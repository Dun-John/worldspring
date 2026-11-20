//! (Roadmap 7, 161) How planned a town's streets are, 0-1: rolled from its seed (mostly grown;
//! some with a planned quarter beside a grown core; a few planned throughout), or set by hand
//! in `Edits.planning` (`<layout>` → 0-100). `seeds::plant` lays the planned part's patches on a
//! lattice, the main streets keep straight there and its blocks are cut regularly (`fill`).
//! Villages and towns laid out by hand ignore it.

use crate::World;
use crate::core::rng::{hash2, unit};
use crate::t0::T0;
use crate::t0::settle::Settlement;
use crate::world::Edits;

/// Below this: grown (organic streets throughout, as towns always were).
pub const MIXED: f64 = 0.3;
/// From this on: planned throughout (no grown core).
pub const PLANNED: f64 = 0.7;

/// The planning settlement `s` rolls: ~60% grown (0-0.3), ~25% a planned quarter (0.3-0.7),
/// ~15% planned (0.7-1); a steppe town (yurts) half as planned.
pub fn rolled(s: &Settlement) -> f64 {
    let u = unit(hash2(s.seed, 0x91a7, 161));
    let p = if u < 0.6 {
        MIXED * u / 0.6
    } else if u < 0.85 {
        MIXED + (PLANNED - MIXED) * (u - 0.6) / 0.25
    } else {
        PLANNED + (1.0 - PLANNED) * (u - 0.85) / 0.15
    };
    if s.culture % 5 == 4 { 0.5 * p } else { p }
}

/// The planning `edits` set for one of the world's own settlements (0-1), if any.
pub fn override_in(edits: &Edits, t0: &T0, index: usize) -> Option<f64> {
    if edits.planning.is_empty() || index >= t0.settlements.len() {
        return None;
    }
    edits.planning.get(&index.to_string()).map(|&v| v.min(100) as f64 / 100.0)
}

/// The planning settlement layout `index` (`s`, as laid out) is planned with: as set, else as
/// rolled.
pub fn of(world: &World, t0: &T0, index: usize, s: &Settlement) -> f64 {
    override_in(&world.file.edits, t0, index).unwrap_or_else(|| rolled(s))
}

/// A mark of the planning set for layout `index` (0: none), mixed into the caches' hashes.
pub fn mark(world: &World, t0: &T0, index: usize) -> u64 {
    override_in(&world.file.edits, t0, index).map_or(0, |p| (p * 100.0) as u64 + 1)
}

/// The class a planning value is in: "grown", "mixed" (a planned quarter beside a grown core) or
/// "planned".
pub fn class(p: f64) -> &'static str {
    if p < MIXED {
        "grown"
    } else if p < PLANNED {
        "mixed"
    } else {
        "planned"
    }
}

/// Where the grown core ends, along the planned quarter's way out from the town's middle (a
/// share of its radius): `None` grown throughout, `Some(-1)` planned throughout; between, the
/// planned quarter is the side of the town beyond a line that far out (from 0.4 out, about a
/// quarter of the town, to 0.08, nearly half), the market square kept in the core.
pub(super) fn core_line(p: f64) -> Option<f64> {
    if p < MIXED {
        None
    } else if p < PLANNED {
        Some(0.4 - 0.32 * (p - MIXED) / (PLANNED - MIXED))
    } else {
        Some(-1.0)
    }
}

/// What `get_town_plan` and `get_feature` say of town layout `index`'s planning: its value
/// (0-1, two places), class, the rolled one and whether it was set by hand.
pub fn to_json(world: &World, t0: &T0, index: usize) -> Option<serde_json::Value> {
    let s = super::culture::settlement(world, t0, index)?;
    let rolled = rolled(&s);
    let set = override_in(&world.file.edits, t0, index);
    let p = set.unwrap_or(rolled);
    let r2 = |x: f64| crate::core::round(x * 100.0) / 100.0;
    Some(serde_json::json!({ "value": r2(p), "class": class(p), "rolled": r2(rolled), "set": set.is_some() }))
}

/// What setting a town's planning did (`change_own`).
#[derive(Debug, Clone, PartialEq)]
pub struct Changed {
    /// The planning it was laid out with, and is now (0-100).
    pub from: u8,
    pub to: u8,
    /// Nothing changed.
    pub unchanged: bool,
    /// The edits inside it set aside, by field (those with any).
    pub aside: Vec<(&'static str, usize)>,
}

impl Changed {
    pub fn to_json(&self) -> serde_json::Value {
        let by: serde_json::Map<String, serde_json::Value> = self.aside.iter().map(|&(f, n)| (f.to_string(), n.into())).collect();
        let n: usize = self.aside.iter().map(|a| a.1).sum();
        let f = |p: u8| p as f64 / 100.0;
        serde_json::json!({ "from": f(self.from), "to": f(self.to), "class": class(f(self.to)), "unchanged": self.unchanged, "set_aside": n, "set_aside_by": by })
    }
}

/// Plan one of the world's own towns, layout `index` (`rolled` its rolled planning, 0-1), with
/// planning `to` (0-100; `None`: as rolled) in `edits`. It is laid out anew, so the edits keyed
/// to its layout are set aside (`t0::made::set_aside`), as a culture change does.
pub fn change_own(edits: &mut Edits, index: usize, rolled: f64, to: Option<u8>) -> Changed {
    let key = index.to_string();
    let pct = |p: f64| crate::core::round(p * 100.0) as u8;
    let from = edits.planning.get(&key).map_or(pct(rolled), |&v| v.min(100));
    let now = to.map_or(pct(rolled), |v| v.min(100));
    match to {
        Some(v) => edits.planning.insert(key, v.min(100)),
        None => edits.planning.remove(&key),
    };
    if from == now {
        return Changed { from, to: now, unchanged: true, aside: Vec::new() };
    }
    let aside = crate::t0::made::set_aside_by_field(edits, index).into_iter().filter(|a| a.1 > 0).collect();
    Changed { from, to: now, unchanged: false, aside }
}
