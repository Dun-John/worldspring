//! A settlement laid out in another culture (R5 121): the world's own through `Edits.cultures`
//! (`<layout>` → culture), applied where it is laid out, so no road, neighbour or T0 artifact
//! moves; one added by hand through its `Created::culture`. Its streets, buildings, roofs,
//! walls, yurts, parks and generated names follow (everything that reads `Settlement::culture`
//! or `Layout::culture`); its own name, notes, NPCs and plot points (on its feature id) stay,
//! and the edits inside it are set aside (`t0::made::set_aside`), as a tier change does.

use std::borrow::Cow;

use crate::World;
use crate::t0::T0;
use crate::t0::settle::Settlement;
use crate::world::Edits;

/// The cultures' names, by number (0 north, 1 west, 2 south, 3 dry, 4 steppe).
pub const NAMES: [&str; 5] = ["north", "west", "south", "dry", "steppe"];

/// The culture `edits` lay one of the world's own settlements out in (`Edits.cultures`), if
/// changed; `None` for any other layout.
pub fn override_in(edits: &Edits, t0: &T0, index: usize) -> Option<u8> {
    if edits.cultures.is_empty() || index >= t0.settlements.len() {
        return None;
    }
    edits.cultures.get(&index.to_string()).copied().filter(|&c| (c as usize) < NAMES.len())
}

/// `override_in` the world's edits.
pub fn override_of(world: &World, t0: &T0, index: usize) -> Option<u8> {
    override_in(&world.file.edits, t0, index)
}

/// The culture settlement layout `index` is laid out in: as changed, else as generated (or as
/// added by hand).
pub fn of(world: &World, t0: &T0, index: usize) -> Option<u8> {
    override_of(world, t0, index).or_else(|| super::settlement(t0, index).map(|s| s.culture))
}

/// Settlement layout `index` as it is laid out: `town::settlement` with its culture changed.
pub fn settlement<'a>(world: &World, t0: &'a T0, index: usize) -> Option<Cow<'a, Settlement>> {
    let s = super::settlement(t0, index)?;
    Some(match override_of(world, t0, index) {
        Some(c) if c != s.culture => Cow::Owned(Settlement { culture: c, ..s.clone() }),
        _ => Cow::Borrowed(s),
    })
}

/// A mark of the culture changed for layout `index` (0: none), mixed into the caches' hashes of
/// a layout with its edits.
pub fn mark(world: &World, t0: &T0, index: usize) -> i64 {
    override_of(world, t0, index).map_or(0, |c| c as i64 + 1)
}

/// What changing a settlement's culture did (`change`).
#[derive(Debug, Clone, PartialEq)]
pub struct Changed {
    /// The culture it was laid out in, and is now.
    pub from: u8,
    pub to: u8,
    /// Nothing changed: it was in that culture already.
    pub unchanged: bool,
    /// The edits inside it set aside, by field (those with any).
    pub aside: Vec<(&'static str, usize)>,
}

impl Changed {
    /// How many edits were set aside.
    pub fn set_aside(&self) -> usize {
        self.aside.iter().map(|a| a.1).sum()
    }

    pub fn to_json(&self) -> serde_json::Value {
        let by: serde_json::Map<String, serde_json::Value> = self.aside.iter().map(|&(f, n)| (f.to_string(), n.into())).collect();
        let name = |c: u8| NAMES[c as usize % NAMES.len()];
        serde_json::json!({ "from": name(self.from), "to": name(self.to), "unchanged": self.unchanged, "set_aside": self.set_aside(), "set_aside_by": by })
    }
}

/// Lay settlement layout `index` out in culture `to` (0-4) in `edits`: for one of the world's
/// own, its `Edits.cultures` entry (taken out when `to` is the culture it was generated in); for
/// one added by hand, its `Created::culture`. The edits keyed to its layout are set aside
/// (`t0::made::set_aside`). `t0`: the world's, its edits applied.
pub fn change(edits: &mut Edits, t0: &T0, index: usize, to: u8) -> Result<Changed, String> {
    if to as usize >= NAMES.len() {
        return Err(format!("culture: 0 to {} ({}), got {to}", NAMES.len() - 1, NAMES.join(", ")));
    }
    let n = t0.settlements.len();
    if let Some(s) = t0.settlements.get(index) {
        return Ok(change_own(edits, index, s.culture, to));
    }
    let from = {
        let m = t0.made.iter().find(|m| n + m.poi == index).ok_or_else(|| format!("layout {index}: not a settlement"))?;
        let c = edits.created.get_mut(m.poi - t0.base_pois).filter(|c| c.is_settlement() && !c.removed).ok_or_else(|| format!("layout {index}: not a settlement"))?;
        let from = c.culture.unwrap_or(m.s.culture);
        if from == to {
            return Ok(Changed { from, to, unchanged: true, aside: Vec::new() });
        }
        c.culture = Some(to);
        from
    };
    Ok(Changed { from, to, unchanged: false, aside: aside(edits, index) })
}

/// `change` for one of the world's own settlements, layout `index`, generated in culture
/// `generated` (`to` checked).
pub fn change_own(edits: &mut Edits, index: usize, generated: u8, to: u8) -> Changed {
    let from = edits.cultures.get(&index.to_string()).copied().filter(|&c| (c as usize) < NAMES.len()).unwrap_or(generated);
    if from == to {
        return Changed { from, to, unchanged: true, aside: Vec::new() };
    }
    if to == generated {
        edits.cultures.remove(&index.to_string());
    } else {
        edits.cultures.insert(index.to_string(), to);
    }
    Changed { from, to, unchanged: false, aside: aside(edits, index) }
}

/// Sets aside the edits keyed to layout `index`: by field, those with any.
fn aside(edits: &mut Edits, index: usize) -> Vec<(&'static str, usize)> {
    crate::t0::made::set_aside_by_field(edits, index).into_iter().filter(|a| a.1 > 0).collect()
}

/// A culture as a tool gives it: its name (`NAMES`) or number.
pub fn parse(v: &serde_json::Value) -> Result<u8, String> {
    let bad = || format!("culture: one of {} or 0 to {}, got {v}", NAMES.join(", "), NAMES.len() - 1);
    match v {
        serde_json::Value::String(w) => NAMES.iter().position(|n| n.eq_ignore_ascii_case(w.trim())).map(|c| c as u8).ok_or_else(bad),
        serde_json::Value::Number(n) => n.as_u64().filter(|&c| (c as usize) < NAMES.len()).map(|c| c as u8).ok_or_else(bad),
        _ => Err(bad()),
    }
}
