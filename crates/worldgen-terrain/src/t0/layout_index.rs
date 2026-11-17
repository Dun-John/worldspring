//! Layout indices (settlements first, then sites): a layout's settlement, the count, its rng key
//! and identity.

use crate::core::rng::{hash2, hash3};
use crate::t0::T0;
use crate::t0::settle::Settlement;

/// The settlement layout `li` is: one of the world's own, or a village or town added by hand
/// (`t0::made`; not once removed). `None` for every other site.
pub fn settlement(t0: &T0, li: usize) -> Option<&Settlement> {
    match li.checked_sub(t0.settlements.len()) {
        None => t0.settlements.get(li),
        Some(poi) => t0.made.iter().find(|m| m.poi == poi).map(|m| &m.s),
    }
}

/// What layout `index`'s randomness is keyed by, never its number: the world's own settlements
/// and sites by their seeds (a settlement's from its cell or where its pin was drawn, a site's
/// from its place), so a world remade with a settlement or site more or fewer lays the others out
/// as before, their dungeons and interiors too; a created site's id (through its seed) for one
/// added by hand, so it is laid out the same wherever it falls in the list.
pub fn rng_key(t0: &T0, index: usize) -> i64 {
    let n = t0.settlements.len();
    let Some(poi) = index.checked_sub(n) else { return hash2(0x7365_7474_6c65_6b00, 0, t0.settlements[index].seed as i64) as i64 };
    match t0.pois.get(poi) {
        Some(p) if poi >= t0.base_pois => (p.seed ^ 0x6372_6561_7465_6400) as i64,
        Some(p) => hash2(0x7369_7465_706f_6900, 0, p.seed as i64) as i64,
        None => index as i64,
    }
}

/// What layout `index` is drawn from, as a hash: the inputs of its randomness (`rng_key` and the
/// seed) with its tier, kind, size and waters; never its number. Two worlds whose
/// layouts at an index (each) hash the same lay them out the same, and everything in them, given
/// the same ground and roads round them: the app keeps changes on a place across a remade world
/// only then (`t0::remap`); 0 past the last layout.
pub fn layout_identity(t0: &T0, index: usize) -> u64 {
    let n = t0.settlements.len();
    if let Some(s) = settlement(t0, index) {
        let what = (s.coastal as i64) << 49 | (s.river as i64) << 48 | (s.tier as i64) << 40 | (s.kind as i64) << 32 | s.population as i64;
        return hash3(0x7365_7474_6c65_0000, rng_key(t0, index), s.seed as i64, what);
    }
    match t0.pois.get(index - n) {
        Some(p) => hash3(0x7369_7465_0000_0000, rng_key(t0, index), p.seed as i64, p.kind as i64),
        None => 0,
    }
}

/// Number of layouts (settlements then sites).
pub fn layout_count(t0: &T0) -> usize {
    t0.settlements.len() + t0.pois.len()
}
