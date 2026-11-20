//! (Roadmap 7, 189) Setting a town's street pattern by hand (`Edits.patterns`): `grid`, `rings`,
//! `radial` or `linear`; absent, the pattern is chosen from the town's seed and site.

use crate::world::Edits;

/// The patterns a town can be laid out with, as `Edits.patterns` names them.
pub const NAMES: [&str; 4] = ["grid", "rings", "radial", "linear"];

/// What setting a town's pattern did (`change_own`).
#[derive(Debug, Clone, PartialEq)]
pub struct Changed {
    /// The pattern set before and now (`None`: chosen from the seed and site).
    pub from: Option<String>,
    pub to: Option<String>,
    pub unchanged: bool,
    /// The edits inside it set aside, by field (those with any).
    pub aside: Vec<(&'static str, usize)>,
}

impl Changed {
    pub fn to_json(&self) -> serde_json::Value {
        let by: serde_json::Map<String, serde_json::Value> = self.aside.iter().map(|&(f, n)| (f.to_string(), n.into())).collect();
        let n: usize = self.aside.iter().map(|a| a.1).sum();
        let name = |p: &Option<String>| p.clone().unwrap_or_else(|| "auto".into());
        serde_json::json!({ "from": name(&self.from), "to": name(&self.to), "unchanged": self.unchanged, "set_aside": n, "set_aside_by": by })
    }
}

/// The pattern `edits` set for layout `index`, if any.
pub fn override_in(edits: &Edits, index: usize) -> Option<&str> {
    edits.patterns.get(&index.to_string()).map(String::as_str)
}

/// Give one of the world's own towns, layout `index`, pattern `to` (one of `NAMES`; `None`: as
/// chosen) in `edits`. It is laid out anew, so the edits keyed to its layout are set aside
/// (`t0::made::set_aside`), as a planning change does.
pub fn change_own(edits: &mut Edits, index: usize, to: Option<&str>) -> Changed {
    let key = index.to_string();
    let from = edits.patterns.get(&key).cloned();
    let to = to.map(str::to_string);
    match &to {
        Some(v) => edits.patterns.insert(key, v.clone()),
        None => edits.patterns.remove(&key),
    };
    if from == to {
        return Changed { from, to, unchanged: true, aside: Vec::new() };
    }
    let aside = crate::t0::made::set_aside_by_field(edits, index).into_iter().filter(|a| a.1 > 0).collect();
    Changed { from, to, unchanged: false, aside }
}
