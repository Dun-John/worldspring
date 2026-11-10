//! A world remade from a changed sketch keeps its edits on their places. Generated features can
//! move (a town placed a few miles away, a range redrawn) and so get new ids: edits on one that
//! is gone move to the nearest feature of the same kind that took its place. The layout numbers
//! of places shift (more or fewer settlements and sites before them): changes inside a place
//! follow it when it comes out the same, the rest are set aside from `world::PARKED_LAYOUT` up.
//! Settlements taken away on purpose (a pin removed) have theirs set aside at `gone:<n>:<id>`.
//!
//! One copy for the app (through `worldgen-wasm`'s `remake_prints_json` / `follow_json`) and
//! for `mapd` (`remake`, with no app). It was `app/src/editor/reanchor.ts` (`reanchor`,
//! `setAsideRemoved`, `remapLayouts`) and keeps its behaviour, but for two things: a field's
//! entries are taken in sorted order (the app took them as its objects held them), and the
//! villages and towns added by hand (`c:`) never count among the world's own places.

use crate::World;
use crate::core::hash::{FastMap, FastSet};
use crate::t0::T0;
use crate::world::{Edits, PARKED_LAYOUT};
use serde::{Deserialize, Serialize};

/// How an edits field names places by id.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IdsIn {
    /// Its keys.
    Keys,
    /// Its values (a set of ids).
    Values,
    /// Where each NPC is (`location.id`).
    Npcs,
    /// Each plot point's places (`anchors`).
    Plots,
    /// Its keys are bare layout numbers (a town laid out anew), seen as `t:<n>`.
    Towns,
}

/// Every edits field that names a place, and how: layout-local ids (`b:<layout>:…`, `d:`, `t:`,
/// `u:`, `k:`, `w:`, a level's or room's `l:`/`r:` round one, a town's bare layout number) and
/// feature ids. A remade world moves them with their places: a new field that holds such ids must
/// join this list (and `each_id`, `renamed`), or its ids would point at other places. The app's
/// `LAYOUT_ID_FIELDS` (`app/src/sync/ops.ts`) mirrors it for what it still does itself (an
/// upgrade's `upgradeEdits`).
pub const LAYOUT_ID_FIELDS: &[(&str, IdsIn)] = &[
    ("renames", IdsIn::Keys),
    ("notes", IdsIn::Keys),
    ("hidden", IdsIn::Values),
    ("designs", IdsIn::Keys),
    ("buildings", IdsIn::Keys),
    ("towns", IdsIn::Towns),
    ("npcs", IdsIn::Npcs),
    ("plots", IdsIn::Plots),
];

/// Settlement kinds, as the app's labels have them (`SETTLEMENT_KINDS` in `ui/gazetteer.ts`).
const SETTLEMENT_KINDS: [&str; 4] = ["metropolis", "city", "town", "village"];
/// Site kinds, as the app's labels have them (`SITE_KINDS` in `ui/gazetteer.ts`).
const SITE_KINDS: [&str; 11] = ["ruin", "tower", "camp", "waystation", "cave", "mine", "lava_tube", "entrance", "building", "castle", "wall"];

/// A named feature of a world (its overlay's): id, kind, label anchor (ft) and size (ft).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Place {
    pub id: String,
    pub kind: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub extent_ft: f64,
}

/// A world's places as a remake reads them: its features, the layout number of the first created
/// site, and what each layout is drawn from (`town::layout_identity`, folded: `fold`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Places {
    pub features: Vec<Place>,
    #[serde(default)]
    pub first_created: Option<usize>,
    #[serde(default)]
    pub layout_ids: Option<Vec<u32>>,
}

impl Places {
    /// The places of a world as T0 made them (as the app's overlay has them, but for its created
    /// sites: they never count).
    pub fn of(t0: &T0) -> Places {
        let features = t0.extra.as_ref().map_or_else(Vec::new, |e| e.overlay.features.iter().map(|f| Place { id: f.id.clone(), kind: f.kind.into(), x: f.x, y: f.y, extent_ft: f.extent_ft }).collect());
        let ids = (0..crate::town::layout_count(t0)).map(|li| fold(crate::town::layout_identity(t0, li))).collect();
        Places { features, first_created: Some(t0.settlements.len() + t0.base_pois), layout_ids: Some(ids) }
    }
}

/// A settlement taken away on purpose by the remake: its feature id, its name and its layout
/// number in the world before.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gone {
    pub id: String,
    pub name: String,
    pub li: usize,
}

/// The edits of a remade world, and how many entries followed their places (`kept`), were set
/// aside (`parked`: their places came out differently) or were set aside with a place taken away
/// on purpose (`removed`); `moved`: the features whose edits moved to another (old id, new).
#[derive(Clone, Debug)]
pub struct Followed {
    pub edits: Edits,
    pub kept: usize,
    pub parked: usize,
    pub removed: usize,
    pub moved: Vec<(String, String)>,
}

/// A 64-bit layout hash as the app holds it (32 bits).
pub fn fold(h: u64) -> u32 {
    (h ^ h >> 32) as u32
}

/// A layout-local id's parts: what comes before its layout number, the number, what follows
/// (`^((?:[lr]:)?[bdtukw]:)(\d+)((?::.*)?)$`).
fn local_id(id: &str) -> Option<(&str, u64, &str)> {
    let lead = if id.starts_with("l:") || id.starts_with("r:") { 2 } else { 0 };
    let b = id.as_bytes();
    if b.len() < lead + 2 || !matches!(b[lead], b'b' | b'd' | b't' | b'u' | b'k' | b'w') || b[lead + 1] != b':' {
        return None;
    }
    let head = lead + 2;
    let end = head + b[head..].iter().take_while(|c| c.is_ascii_digit()).count();
    if end == head || (end < b.len() && b[end] != b':') {
        return None;
    }
    Some((&id[..head], id[head..end].parse().ok()?, &id[end..]))
}

/// A town key's layout number (all digits).
fn bare(id: &str) -> Option<u64> {
    if id.is_empty() || !id.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    id.parse().ok()
}

/// Ids of buildings, districts, wall towers, sites underground and created sites: never moved to
/// another feature (`^[bdtuwkc]:`).
fn is_local(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() >= 2 && matches!(b[0], b'b' | b'd' | b't' | b'u' | b'w' | b'k' | b'c') && b[1] == b':'
}

/// Every id `edits` hold where a place is named (`LAYOUT_ID_FIELDS`, in its order; a town's bare
/// layout number as `t:<n>`, which no other id is).
fn each_id(edits: &Edits, mut f: impl FnMut(&str)) {
    for &(field, _) in LAYOUT_ID_FIELDS {
        match field {
            "renames" => edits.renames.keys().for_each(|k| f(k)),
            "notes" => edits.notes.keys().for_each(|k| f(k)),
            "hidden" => edits.hidden.iter().for_each(|k| f(k)),
            "designs" => edits.designs.keys().for_each(|k| f(k)),
            "buildings" => edits.buildings.keys().for_each(|k| f(k)),
            "towns" => edits.towns.keys().for_each(|k| f(&format!("t:{k}"))),
            "npcs" => edits.npcs.values().filter_map(|n| n.location.as_ref()).for_each(|l| f(&l.id)),
            "plots" => edits.plots.values().flat_map(|p| &p.anchors).for_each(|k| f(k)),
            _ => unreachable!("a layout-id field each_id doesn't read"),
        }
    }
}

/// `edits` with every id `rename` changes changed (a town's bare key: `bare` true), and how many
/// entries changed.
fn renamed(edits: &Edits, mut rename: impl FnMut(&str, bool) -> String) -> (Edits, usize) {
    let mut n = 0;
    let mut to = |id: &str, bare: bool| {
        let r = rename(id, bare);
        if r != id {
            n += 1;
        }
        r
    };
    fn rekey<V: Clone>(m: &std::collections::BTreeMap<String, V>, bare: bool, to: &mut impl FnMut(&str, bool) -> String) -> std::collections::BTreeMap<String, V> {
        m.iter().map(|(k, v)| (to(k, bare), v.clone())).collect()
    }
    let mut out = edits.clone();
    out.renames = rekey(&edits.renames, false, &mut to);
    out.notes = rekey(&edits.notes, false, &mut to);
    out.hidden = edits.hidden.iter().map(|k| to(k, false)).collect();
    out.designs = rekey(&edits.designs, false, &mut to);
    out.buildings = rekey(&edits.buildings, false, &mut to);
    out.towns = rekey(&edits.towns, true, &mut to);
    for n in out.npcs.values_mut() {
        if let Some(l) = n.location.as_mut() {
            l.id = to(&l.id, false);
        }
    }
    for p in out.plots.values_mut() {
        for a in p.anchors.iter_mut() {
            *a = to(a, false);
        }
    }
    (out, n)
}

/// The layout numbers `edits` name places in (below `PARKED_LAYOUT`), ascending.
pub fn layouts_in(edits: &Edits) -> Vec<usize> {
    let mut out = std::collections::BTreeSet::new();
    each_id(edits, |id| {
        if let Some((_, li, _)) = local_id(id).filter(|&(_, li, _)| li < PARKED_LAYOUT as u64) {
            out.insert(li as usize);
        }
    });
    out.into_iter().collect()
}

/// The first layout number past those `edits` set aside already (`PARKED_LAYOUT` with none), so
/// two remakes never share one.
fn next_parked(edits: &Edits) -> u64 {
    let mut next = PARKED_LAYOUT as u64;
    each_id(edits, |id| {
        if let Some((_, li, _)) = local_id(id).filter(|&(_, li, _)| li >= PARKED_LAYOUT as u64) {
            next = next.max(li + 1);
        }
    });
    next
}

/// How far a feature may have moved and still be the same one (ft): 40 mi, or more for big ones
/// (a range's or forest's anchor sits mid-way along it and moves as its shape does).
fn reach_for(f: &Place) -> f64 {
    (40.0 * 5280.0f64).max(0.6 * f.extent_ft)
}

/// `edits` with ids of vanished features moved to their nearest same-kind successors (in `after`,
/// by their positions in `before`): names, notes, hidden marks, where NPCs are and plot points'
/// places. The edits, what moved (features) and how many entries; None if nothing did.
fn reanchor(edits: &Edits, before: &[Place], after: &[Place]) -> Option<(Edits, Vec<(String, String)>, usize)> {
    let now: FastSet<&str> = after.iter().map(|f| f.id.as_str()).collect();
    let mut was: FastMap<&str, &Place> = FastMap::default();
    for f in before {
        was.insert(&f.id, f);
    }
    let mut keyed: Vec<String> = Vec::new();
    let mut seen: FastSet<String> = FastSet::default();
    each_id(edits, |id| {
        if seen.insert(id.to_string()) {
            keyed.push(id.to_string());
        }
    });
    let mut taken: FastSet<&str> = keyed.iter().map(String::as_str).filter(|id| now.contains(id)).collect();
    let mut moved: Vec<(String, String)> = Vec::new();
    for id in &keyed {
        if now.contains(id.as_str()) || is_local(id) {
            continue;
        }
        let Some(old) = was.get(id.as_str()) else { continue };
        let (mut best, mut best_d) = (None::<&Place>, reach_for(old));
        for f in after {
            if f.kind != old.kind || taken.contains(f.id.as_str()) {
                continue;
            }
            let (dx, dy) = (f.x - old.x, f.y - old.y);
            let d = crate::core::sqrt(dx * dx + dy * dy);
            if d < best_d {
                (best, best_d) = (Some(f), d);
            }
        }
        if let Some(b) = best {
            taken.insert(&b.id);
            moved.push((id.clone(), b.id.clone()));
        }
    }
    if moved.is_empty() {
        return None;
    }
    let to: FastMap<&str, &str> = moved.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    // (A town's bare layout number is no feature id.)
    let (out, n) = renamed(edits, |id, bare| if bare { id.to_string() } else { to.get(id).map_or_else(|| id.to_string(), |t| t.to_string()) });
    Some((out, moved, n))
}

/// `gone:<n>:<id>`'s `n`.
fn gone_number(id: &str) -> Option<u64> {
    let rest = id.strip_prefix("gone:")?;
    let (n, tail) = rest.split_once(':')?;
    if n.is_empty() || !n.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // (`<kind>:…`, the kind lower-case letters and `_`.)
    let (kind, _) = tail.split_once(':')?;
    if kind.is_empty() || !kind.bytes().all(|c| c.is_ascii_lowercase() || c == b'_') {
        return None;
    }
    n.parse().ok()
}

/// `edits` with the ids of places taken away on purpose (`gone`, not in `after`) set aside at
/// `gone:<n>:<id>` (n past those in use, so a place taken away twice keeps both): notes, NPCs'
/// places and plot points' places. Its hidden mark goes (nothing to hide). Its name stays (as
/// renamed, else the name it had) when anything is left on it, so the entries say what they were
/// on; else it goes too. The edits and how many entries were set aside; None if nothing was.
fn set_aside_removed(edits: &Edits, removed: &[Gone], after: &[Place]) -> Option<(Edits, usize)> {
    let now: FastSet<&str> = after.iter().map(|f| f.id.as_str()).collect();
    // (Still on the map: nothing to set aside.)
    let gone: Vec<&Gone> = removed.iter().filter(|r| !now.contains(r.id.as_str())).collect();
    if gone.is_empty() {
        return None;
    }
    let mut k = 1u64;
    each_id(edits, |id| {
        if let Some(n) = gone_number(id) {
            k = k.max(n + 1);
        }
    });
    let mut to: FastMap<&str, String> = FastMap::default();
    for r in &gone {
        to.insert(&r.id, format!("gone:{k}:{}", r.id));
        k += 1;
    }
    let (mut out, _) = renamed(edits, |id, bare| if bare { id.to_string() } else { to.get(id).cloned().unwrap_or_else(|| id.to_string()) });
    let ids: FastSet<&str> = to.values().map(String::as_str).collect();
    out.hidden.retain(|h| !ids.contains(h.as_str()));
    let mut n = 0;
    for r in &gone {
        let g = &to[r.id.as_str()];
        let used = out.notes.contains_key(g) as usize + out.npcs.values().filter(|x| x.location.as_ref().is_some_and(|l| &l.id == g)).count() + out.plots.values().filter(|p| p.anchors.contains(g)).count();
        n += used;
        if used > 0 {
            out.renames.entry(g.clone()).or_insert_with(|| r.name.clone());
        } else {
            out.renames.remove(g);
        }
    }
    let hid = edits.hidden.iter().any(|h| to.contains_key(h.as_str()));
    (n > 0 || hid || gone.iter().any(|r| edits.renames.contains_key(&r.id))).then_some((out, n))
}

/// A world's layouts as the app counts them: the settlements, then the world's own sites (their
/// features, in the generator's order); the first created one's number.
fn layouts_of(p: &Places) -> (Vec<&Place>, usize) {
    let own = |f: &&Place| !f.id.starts_with("c:");
    let settlements = p.features.iter().filter(|f| SETTLEMENT_KINDS.contains(&f.kind.as_str())).filter(own);
    let sites = p.features.iter().filter(|f| SITE_KINDS.contains(&f.kind.as_str())).filter(own);
    let list: Vec<&Place> = settlements.chain(sites).collect();
    let first = p.first_created.unwrap_or(list.len());
    (list, first)
}

/// The world's own layouts among `lis` (in `before`) whose place the remade world (`after`) has
/// drawn alike: the same feature (id, kind and place) from the same seed (the same `layout_ids`
/// entry). (Old layout number, new), in the order of `lis`.
pub fn match_layouts(lis: &[usize], before: &Places, after: &Places) -> Vec<(usize, usize)> {
    let ((a, a_first), (b, b_first)) = (layouts_of(before), layouts_of(after));
    // (Counted otherwise than the generator counts them, the world's own places can't be matched.)
    if a.len() != a_first || b.len() != b_first {
        return Vec::new();
    }
    let mut index: FastMap<&str, usize> = FastMap::default();
    for (i, f) in b.iter().enumerate() {
        index.insert(&f.id, i);
    }
    let mut out = Vec::new();
    for &li in lis {
        if li >= a_first {
            continue;
        }
        let f = a[li];
        let Some(&i) = index.get(f.id.as_str()) else { continue };
        let g = b[i];
        if g.kind != f.kind || (g.x - f.x).abs() > 1.0 || (g.y - f.y).abs() > 1.0 {
            continue;
        }
        // (Without the generator's word on how they are drawn, only those whose number is the same.)
        let drawn = match (&before.layout_ids, &after.layout_ids) {
            (Some(x), Some(y)) => x.get(li).is_some_and(|v| Some(v) == y.get(i)),
            _ => li == i,
        };
        if drawn {
            out.push((li, i));
        }
    }
    out
}

/// Where the places of layouts `lis` go from one world (`before`) to the same world remade
/// (`after`), in `lis`' order: a created site (one of `created`) keeps its place among the created
/// ones; one of the world's own its new number when `match_layouts` pairs it and `same` says it
/// was laid out the same. None: it came out differently, or is gone.
pub fn layout_targets(lis: &[usize], before: &Places, after: &Places, created: usize, same: &dyn Fn(usize, usize) -> bool) -> Vec<Option<usize>> {
    let (a_first, b_first) = (layouts_of(before).1, layouts_of(after).1);
    let matched: FastMap<usize, usize> = match_layouts(lis, before, after).into_iter().collect();
    lis.iter()
        .map(|&li| {
            if li >= a_first {
                return (li < a_first + created).then(|| b_first + li - a_first);
            }
            matched.get(&li).copied().filter(|&to| same(li, to))
        })
        .collect()
}

/// `edits` with their layout-local ids following their places from one world (`before`) to the
/// same world remade (`after`), as `layout_targets` sends them; every other layout's keys set
/// aside, at `PARKED_LAYOUT` and up (each layout a number of its own), so none lands on another
/// place. The keys of layouts taken away on purpose (`removed`) are set aside too, counted apart.
/// (Edits, moved, parked, removed); None if nothing moved.
fn remap_layouts(edits: &Edits, before: &Places, after: &Places, same: &dyn Fn(usize, usize) -> bool, removed: &[usize]) -> Option<(Edits, usize, usize, usize)> {
    let lis = layouts_in(edits);
    let target = layout_targets(&lis, before, after, edits.created.len(), same);
    let mut next = next_parked(edits);
    let mut to: FastMap<u64, u64> = FastMap::default();
    let (mut parked_at, mut removed_at): (FastSet<u64>, FastSet<u64>) = Default::default();
    for (&li, &t) in lis.iter().zip(&target) {
        let t = if removed.contains(&li) { None } else { t };
        match t {
            Some(t) => {
                to.insert(li as u64, t as u64);
            }
            None => {
                if removed.contains(&li) { &mut removed_at } else { &mut parked_at }.insert(next);
                to.insert(li as u64, next);
                next += 1;
            }
        }
    }
    let (mut parked, mut gone) = (0, 0);
    let (out, n) = renamed(edits, |id, is_bare| {
        let m = if is_bare { bare(id).map(|li| ("", li, "")) } else { local_id(id) };
        let Some((head, li, tail)) = m else { return id.to_string() };
        match to.get(&li) {
            Some(&t) if t != li => {
                if parked_at.contains(&t) {
                    parked += 1;
                } else if removed_at.contains(&t) {
                    gone += 1;
                }
                format!("{head}{t}{tail}")
            }
            _ => id.to_string(),
        }
    });
    (n > 0).then(|| (out, n - parked - gone, parked, gone))
}

/// `edits` made on a world (`before`) following their places to the same world remade from a
/// changed sketch (`after`): places taken away on purpose (`gone`) have theirs set aside first,
/// so no neighbour takes them; then names, notes and the like on vanished features move to their
/// successors; then layout-local ids follow their layouts (`same(old, new)`: the layout came out
/// the same, its fingerprint as generated). None if nothing moved.
pub fn follow(edits: &Edits, before: &Places, after: &Places, same: &dyn Fn(usize, usize) -> bool, gone: &[Gone]) -> Option<Followed> {
    let off = set_aside_removed(edits, gone, &after.features);
    let named = reanchor(off.as_ref().map_or(edits, |o| &o.0), &before.features, &after.features);
    let base = named.as_ref().map(|n| &n.0).or(off.as_ref().map(|o| &o.0)).unwrap_or(edits);
    let lis: Vec<usize> = gone.iter().map(|g| g.li).collect();
    let placed = remap_layouts(base, before, after, same, &lis);
    if off.is_none() && named.is_none() && placed.is_none() {
        return None;
    }
    let (moved_n, parked, removed) = placed.as_ref().map_or((0, 0, 0), |p| (p.1, p.2, p.3));
    Some(Followed {
        kept: named.as_ref().map_or(0, |n| n.2) + moved_n,
        parked,
        removed: off.as_ref().map_or(0, |o| o.1) + removed,
        moved: named.as_ref().map_or_else(Vec::new, |n| n.1.clone()),
        edits: placed.map_or_else(|| base.clone(), |p| p.0),
    })
}

/// The fingerprints (`town::layout_print`, folded) of the world's own layouts a remake must find
/// alike to keep what is made on them: those `edits` name, and `also` (a play session's), below
/// the first created one. (Layout, print), ascending.
pub fn prints_before(world: &World, t0: &T0, edits: &Edits, also: &[usize]) -> Vec<(usize, u32)> {
    let first = t0.settlements.len() + t0.base_pois;
    let mut lis: Vec<usize> = layouts_in(edits).into_iter().chain(also.iter().copied()).filter(|&li| li < first).collect();
    lis.sort_unstable();
    lis.dedup();
    lis.into_iter().map(|li| (li, fold(crate::town::layout_print(world, t0, li)))).collect()
}

/// `follow` into the world remade (`world`, `t0`: generated, its edits those made before), from
/// the places and prints of the world before (`before`, `was`: `prints_before`); and where the
/// layouts `played` go (`layout_targets`).
pub fn follow_into(world: &World, t0: &T0, edits: &Edits, before: &Places, was: &[(usize, u32)], gone: &[Gone], played: &[usize]) -> (Option<Followed>, Vec<Option<usize>>) {
    let after = Places::of(t0);
    let lis: Vec<usize> = was.iter().map(|&(li, _)| li).collect();
    let prints: FastMap<usize, u32> = was.iter().copied().collect();
    let same: FastMap<(usize, usize), bool> = match_layouts(&lis, before, &after)
        .into_iter()
        .map(|(li, to)| {
            let now = fold(crate::town::layout_print(world, t0, to));
            ((li, to), now != 0 && Some(&now) == prints.get(&li))
        })
        .collect();
    let same = |li: usize, to: usize| same.get(&(li, to)).copied().unwrap_or(false);
    let targets = layout_targets(played, before, &after, edits.created.len(), &same);
    (follow(edits, before, &after, &same, gone), targets)
}

/// The whole remake with no app: the world `old` (its T0 `old_t0`, generated) remade as `new` (its
/// changed sketch; its edits those `old` has). Returns the new T0 (generated, its edits applied)
/// and what `follow` made of the edits (None: they stay as they are).
pub fn remake(old: &World, old_t0: &T0, new: &World, gone: &[Gone]) -> (T0, Option<Followed>) {
    let edits = &old.file.edits;
    let (before, was) = (Places::of(old_t0), prints_before(old, old_t0, edits, &[]));
    let mut t0 = T0::generate(new);
    t0.apply_edits(new);
    let (followed, _) = follow_into(new, &t0, edits, &before, &was, gone, &[]);
    (t0, followed)
}
