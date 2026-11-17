//! Which town owns what: town membership (Roadmap 6 R3). The one place that answers "whose is
//! this" for a thing drawn by hand and for a town's generated outskirts.
//!
//! A drawn thing (a created building, castle, wall, plot or Places site: an *item*) is part of a
//! town when it lies within `JOIN_FT` of the town's edge (its buildings and walls; inside one of
//! its quarters or its wall ring is 0 ft), or within `JOIN_FT` of a thing already part of it
//! (joins chain: a joined row is then the town's outermost building), or was joined by hand
//! (`Edits.members[<town>].add`). Never of a town it was taken out of by hand (`.remove`), however
//! near. Of several, the nearest wins: the smallest gap first, then the lower layout, then the
//! lower `c:` number. Joins by distance are worked out here, never stored.
//!
//! A town's generated buildings on its outskirts can be pushed out into a place of their own (a
//! created `place`, whose `add` names them `b:<layout>:<id>`): they stay in their town's layout,
//! drawn as before, and count for the place.
//!
//! Towns are the world's settlements, those added by hand (`t0.made`) and the places. Their edges
//! come from `town::layout` (edits applied, never membership), so no layout reads membership and
//! there is no cycle; every answer is a pure function of the world file: the memos are keyed by
//! content and the greedy order is total, so nothing hangs on what was asked first.

use super::*;
use crate::world::{Created, Members};
use serde_json::{Value, json};
use std::cmp::Reverse;
use std::collections::BTreeSet;

/// How near a town's edge (or one of its members) a drawn thing joins it (ft).
pub const JOIN_FT: f64 = 300.0;

/// Why a thing is part of a town.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Why {
    /// Within this many feet of the town's edge or of one of its members.
    Gap(f32),
    /// Joined by hand (`Edits.members`).
    Hand,
}

/// The town a thing is part of (its layout: a settlement, a town added by hand or a place) and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Owner {
    pub town: usize,
    pub why: Why,
}

/// A change to towns' members (`change`).
#[derive(Clone, Debug, Default)]
pub struct Change {
    /// The `Edits.members` entries changed: key → its new value (empty: to be taken away).
    pub members: BTreeMap<String, Members>,
    /// Created entries to append, in order (a place for the buildings pushed out).
    pub created: Vec<Created>,
    /// Ids not changed, and why.
    pub refused: Vec<(String, &'static str)>,
    pub report: Value,
}

/// The town layout created thing `li` is part of (None: none, or `li` is no drawn thing).
pub fn owner(world: &World, t0: &T0, li: usize) -> Option<Owner> {
    item_at_layout(world, t0, li)?;
    let idx = index(world, t0);
    let &i = idx.of_layout.get(&li)?;
    let c = idx.comp[i as usize];
    let owners = resolve(world, t0, &idx, &town_rects(world, t0, &idx), c);
    owners[idx.comps[c as usize].items.binary_search(&i).ok()?]
}

/// The place generated building `b:<li>:<id>` was pushed out into, if it was.
pub fn building_owner(world: &World, t0: &T0, li: usize, id: u32) -> Option<usize> {
    if world.file.edits.members.is_empty() {
        return None;
    }
    index(world, t0).pushed.get(&(li, id)).copied()
}

/// The drawn things part of town `town` (layouts, ascending), and why.
pub fn members_of(world: &World, t0: &T0, town: usize) -> Vec<(usize, Owner)> {
    if world.file.edits.created.is_empty() {
        return Vec::new();
    }
    let idx = index(world, t0);
    let towns = town_rects(world, t0, &idx);
    let Some(t) = towns.iter().find(|t| t.li == town) else { return Vec::new() };
    let mut out = Vec::new();
    for (c, comp) in idx.comps.iter().enumerate() {
        let hand = comp.items.iter().any(|i| idx.hand.get(i) == Some(&town));
        if !hand && !overlaps(t.rect, grow(comp.bbox, JOIN_FT)) {
            continue;
        }
        let owners = resolve(world, t0, &idx, &towns, c as u32);
        out.extend(comp.items.iter().zip(owners.iter()).filter_map(|(&i, o)| o.filter(|o| o.town == town).map(|o| (idx.items[i as usize].li, o))));
    }
    out.sort_by_key(|o| o.0);
    out
}

/// The places town `town`'s buildings were pushed out into: (place layout, building ids), by layout.
pub fn pushed(world: &World, t0: &T0, town: usize) -> Vec<(usize, Vec<u32>)> {
    if world.file.edits.members.is_empty() {
        return Vec::new();
    }
    let idx = index(world, t0);
    let mut by: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    for (&(li, id), &place) in &idx.pushed {
        if li == town {
            by.entry(place).or_default().push(id);
        }
    }
    by.into_iter()
        .map(|(p, mut ids)| {
            ids.sort_unstable();
            (p, ids)
        })
        .collect()
}

/// Every place's buildings pushed out: (place layout, town layout, building ids), by place, then town.
pub fn all_pushed(world: &World, t0: &T0) -> Vec<(usize, usize, Vec<u32>)> {
    if world.file.edits.members.is_empty() {
        return Vec::new();
    }
    let mut by: BTreeMap<(usize, usize), Vec<u32>> = BTreeMap::new();
    for (&(li, id), &place) in &index(world, t0).pushed {
        by.entry((place, li)).or_default().push(id);
    }
    by.into_iter()
        .map(|((p, t), mut ids)| {
            ids.sort_unstable();
            (p, t, ids)
        })
        .collect()
}

/// Where in its town created thing `li` stands: inside the walls or not (None: unwalled, or its
/// town is a place) and the quarter (`Layout::quarters` index) it counts in: inside, the one
/// holding it (else the nearest not `Sprawl`); outside, the nearest `Sprawl` one; unwalled, the
/// one holding it, else the nearest. None when it is part of no town.
pub fn district(world: &World, t0: &T0, li: usize) -> Option<(Option<bool>, Option<usize>)> {
    let o = owner(world, t0, li)?;
    if !is_settlement(t0, o.town) {
        return Some((None, None));
    }
    let idx = index(world, t0);
    let mid = geom::mean(&idx.items[idx.of_layout[&li] as usize].shape.pts);
    let l = layout(world, t0, o.town);
    let in_quarter = |q: &Quarter| q.patches.iter().any(|p| p.len() > 2 && contains(p, mid));
    let inside = wall_ring(&l).map(|r| contains(&r, mid) || l.quarters.iter().any(|q| q.kind != QuarterKind::Sprawl && in_quarter(q)));
    let nearest = |keep: &dyn Fn(&Quarter) -> bool| {
        let d = |q: &Quarter| q.patches.iter().filter(|p| p.len() > 2).map(|p| if contains(p, mid) { 0.0 } else { outline_dist(p, mid) }).fold(f64::MAX, f64::min);
        l.quarters.iter().enumerate().filter(|(_, q)| keep(q)).map(|(i, q)| (d(q), i)).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|x| x.1)
    };
    let holding = l.quarters.iter().position(|q| in_quarter(q) && (inside != Some(true) || q.kind != QuarterKind::Sprawl));
    let quarter = match inside {
        Some(true) => holding.or_else(|| nearest(&|q| q.kind != QuarterKind::Sprawl)),
        Some(false) => nearest(&|q| q.kind == QuarterKind::Sprawl),
        None => holding.or_else(|| nearest(&|_| true)),
    };
    Some((inside, quarter))
}

/// Town `town`'s people and size class with its members: its own (T0's, a hand town's as last
/// counted, a place's buildings') plus its members' homes, less the homes pushed out of it, given
/// them. None when membership changes nothing (no members, none pushed out, not a place).
fn people_with(world: &World, t0: &T0, town: usize, joined: &[(usize, Owner)], out: &[(usize, Vec<u32>)]) -> Option<(u32, Tier)> {
    let place = place_of(world, t0, town).is_some();
    if !place && joined.is_empty() && out.is_empty() {
        return None;
    }
    let idx = index(world, t0);
    let own = if place {
        idx.pushed.iter().filter(|e| *e.1 == town).filter_map(|(&(li, id), _)| layout(world, t0, li).building(id as usize).map(hand::people_in)).sum()
    } else {
        hand::size(world, town).map(|s| s.0).or_else(|| settlement(t0, town).map(|s| s.population)).unwrap_or(0)
    };
    let more: u32 = joined.iter().map(|&(li, _)| hand::people_of(&layout(world, t0, li))).sum();
    let l = layout(world, t0, town);
    let less: u32 = out.iter().flat_map(|(_, ids)| ids).filter_map(|&id| l.building(id as usize)).map(hand::people_in).sum();
    let n = (own + more).saturating_sub(less);
    Some((n, hand::tier_of_people(n)))
}

/// Every drawn thing's owner, by layout (those part of no town left out): `owner` for all at once.
pub fn owners(world: &World, t0: &T0) -> Vec<(usize, Owner)> {
    if world.file.edits.created.is_empty() {
        return Vec::new();
    }
    let idx = index(world, t0);
    let towns = town_rects(world, t0, &idx);
    let mut out = Vec::new();
    for (c, comp) in idx.comps.iter().enumerate() {
        let hand = comp.items.iter().any(|i| idx.hand.contains_key(i));
        let reach = grow(comp.bbox, JOIN_FT);
        if !hand && !towns.iter().any(|t| overlaps(t.rect, reach)) {
            continue;
        }
        let owners = resolve(world, t0, &idx, &towns, c as u32);
        out.extend(comp.items.iter().zip(owners.iter()).filter_map(|(&i, o)| o.map(|o| (idx.items[i as usize].li, o))));
    }
    out.sort_by_key(|o| o.0);
    out
}

/// The towns whose people membership changes (`people`), by layout: (town, people, size class).
/// From `owners` and the buildings pushed out, each town's members read once.
pub fn sizes(world: &World, t0: &T0) -> Vec<(usize, u32, Tier)> {
    if world.file.edits.created.is_empty() {
        return Vec::new();
    }
    let mut joined: BTreeMap<usize, Vec<(usize, Owner)>> = BTreeMap::new();
    for o in owners(world, t0) {
        joined.entry(o.1.town).or_default().push(o);
    }
    let idx = index(world, t0);
    let mut out: BTreeMap<usize, Vec<(usize, Vec<u32>)>> = BTreeMap::new();
    for (&(li, id), &place) in &idx.pushed {
        let e = out.entry(li).or_default();
        match e.iter_mut().find(|p| p.0 == place) {
            Some(p) => p.1.push(id),
            None => e.push((place, vec![id])),
        }
    }
    let first = t0.settlements.len() + t0.base_pois;
    let places = world.file.edits.created.iter().enumerate().filter(|(_, c)| c.kind == "place" && !c.removed).map(|(k, _)| first + k);
    let towns: BTreeSet<usize> = joined.keys().chain(out.keys()).copied().chain(places).collect();
    towns.into_iter().filter_map(|t| people_with(world, t0, t, joined.get(&t).map_or(&[][..], |v| v), out.get(&t).map_or(&[][..], |v| v)).map(|(n, tier)| (t, n, tier))).collect()
}

/// How a drawn thing part of a town looks the town's (`look`): its culture, its roofs' age, its
/// own roofs' seed (the town's salted by the thing: every drawn building is building 0) and, for
/// a drawn building, the ward of the town's nearest building.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub town: usize,
    /// None: its own (a plot's culture chosen by hand).
    pub culture: Option<u8>,
    pub roof_seed: u64,
    pub roof_age: u8,
    pub ward: Option<Ward>,
}

impl Look {
    /// A hash of it, for a layout's cache fingerprint.
    pub fn fingerprint(&self) -> u64 {
        let w = self.ward.map_or(0xff, |w| w as i64);
        hash3(self.roof_seed, self.town as i64, (self.culture.map_or(0xff, |c| c as i64)) << 16 | (self.roof_age as i64) << 8 | w, 0x100c)
    }

    /// Layout `l` (the drawn thing's) in it.
    pub fn apply(&self, l: &mut Layout) {
        if let Some(c) = self.culture {
            l.culture = c;
        }
        l.roof_seed = self.roof_seed;
        l.roof_age = self.roof_age;
        if let Some(w) = self.ward {
            l.buildings.iter_mut().for_each(|b| b.ward = w);
        }
    }
}

thread_local! {
    /// By drawn thing's layout: the index key and town layouts its look was read from, and it.
    static LOOKS: RefCell<FastMap<usize, (u64, Vec<Rc<Layout>>, Look)>> = RefCell::new(FastMap::default());
}

/// How created thing `li` looks as part of its town (None: part of none, or no drawn thing).
pub fn look(world: &World, t0: &T0, li: usize) -> Option<Look> {
    let c = item_at_layout(world, t0, li)?;
    let o = owner(world, t0, li)?;
    let idx = index(world, t0);
    let ls = edge_layouts(world, t0, &idx, o.town);
    let tl = ls.first()?;
    if let Some(l) = LOOKS.with(|m| m.borrow().get(&li).filter(|e| e.0 == idx.key && e.2.town == o.town && e.1.len() == ls.len() && e.1.iter().zip(&ls).all(|(a, b)| Rc::ptr_eq(a, b))).map(|e| e.2)) {
        return Some(l);
    }
    let ward = (c.kind == "building").then(|| {
        let mid = geom::mean(&idx.items[idx.of_layout[&li] as usize].shape.pts);
        let mine = |l: &Layout, b: &Building| match place_of(world, t0, o.town) {
            Some(_) => idx.pushed.get(&(l.index as usize, b.id)) == Some(&o.town),
            None => !idx.pushed.contains_key(&(l.index as usize, b.id)),
        };
        ls.iter().flat_map(|l| l.buildings.iter().filter(move |b| mine(l, b)).map(move |b| (geom::dist(geom::centroid(&b.poly), mid), l.index, b.id, b.ward))).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2))).map(|x| x.3)
    });
    let look = Look { town: o.town, culture: (c.culture.is_none() || c.kind != "plot").then_some(tl.culture), roof_seed: hash2(tl.roof_seed, rng_key(t0, li), 0x100c), roof_age: tl.roof_age, ward: ward.flatten() };
    LOOKS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 4096 {
            m.clear();
        }
        m.insert(li, (idx.key, ls, look));
    });
    Some(look)
}

/// Layout `li`'s created entry if it is a drawn thing standing (an item), from the file alone.
fn item_at_layout<'a>(world: &'a World, t0: &T0, li: usize) -> Option<&'a Created> {
    let k = li.checked_sub(t0.settlements.len() + t0.base_pois)?;
    world.file.edits.created.get(k).filter(|c| !c.removed && crate::world::CREATABLE.contains(&c.kind.as_str()))
}

/// Town `town`'s members changed by hand: `add` joins drawn things (`c:<n>`) to it, or into a
/// place generated buildings (`b:<layout>:<id>`, or one back into its own town); `remove` keeps
/// drawn things out of it, or pushes its generated outskirts out into one new place (named in its
/// culture), or takes a place's buildings back to their town. In normal form: an id in at most one
/// town's `add`, `remove` holding only `c:` ids, no empty entries. Err when `town` is no town.
pub fn change(world: &World, t0: &T0, town: usize, add: &[String], remove: &[String]) -> Result<Change, String> {
    let place = place_of(world, t0, town).is_some();
    if !is_settlement(t0, town) && !place {
        return Err(format!("layout {town} is no town"));
    }
    let edits = &world.file.edits;
    let was = &edits.members;
    let mut m = was.clone();
    let key = town.to_string();
    let mut refused: Vec<(String, &'static str)> = Vec::new();
    let (mut joined, mut left, mut out): (Vec<&str>, Vec<&str>, Vec<(u32, P)>) = Default::default();
    // (Into this town's `add`, out of every other's.)
    let claim = |m: &mut BTreeMap<String, Members>, key: &str, id: &str| {
        m.values_mut().for_each(|e| {
            e.add.remove(id);
        });
        let e = m.entry(key.to_string()).or_default();
        e.remove.remove(id);
        e.add.insert(id.to_string());
    };
    let building = |id: &str| -> Result<(usize, u32, Rc<Layout>), &'static str> {
        let (li, b) = building_id(id).ok_or("Not a drawn thing nor a generated building")?;
        if !is_settlement(t0, li) {
            return Err("Not a town's building");
        }
        let l = layout(world, t0, li);
        if l.building(b as usize).is_none() {
            return Err("No such building");
        }
        Ok((li, b, l))
    };
    for id in add {
        if item_of(world, id).is_some() {
            claim(&mut m, &key, id);
            joined.push(id);
            continue;
        }
        match building(id) {
            Err(why) => refused.push((id.clone(), why)),
            Ok((li, _, _)) if li == town => {
                // (Back into its own town.)
                m.values_mut().for_each(|e| {
                    e.add.remove(id.as_str());
                });
                joined.push(id);
            }
            Ok((_, b, l)) if place => {
                if may_push(&l, l.building(b as usize).unwrap()) {
                    claim(&mut m, &key, id);
                    joined.push(id);
                } else {
                    refused.push((id.clone(), INSIDE));
                }
            }
            Ok(_) => refused.push((id.clone(), "A generated building stays with its own town")),
        }
    }
    for id in remove {
        if item_of(world, id).is_some() {
            // (A `remove` only where distance would join it: else dropping the `add` is all.)
            let near = by_distance(world, t0, town, id);
            let e = m.entry(key.clone()).or_default();
            e.add.remove(id.as_str());
            if near {
                e.remove.insert(id.clone());
            }
            left.push(id);
            continue;
        }
        match building(id) {
            Err(why) => refused.push((id.clone(), why)),
            Ok(_) if place => {
                if m.get_mut(&key).is_some_and(|e| e.add.remove(id.as_str())) {
                    left.push(id);
                } else {
                    refused.push((id.clone(), "Not in this place"));
                }
            }
            Ok((li, b, _)) if li == town && index(world, t0).pushed.contains_key(&(li, b)) => refused.push((id.clone(), "Already out in a place")),
            Ok((li, b, l)) if li == town => {
                let bd = l.building(b as usize).unwrap();
                if may_push(&l, bd) {
                    out.push((b, geom::centroid(&bd.poly)));
                    left.push(id);
                } else {
                    refused.push((id.clone(), INSIDE));
                }
            }
            Ok(_) => refused.push((id.clone(), "Not this town's building")),
        }
    }
    // The buildings pushed out: one new place, at their middle.
    let mut created = Vec::new();
    if !out.is_empty() {
        let k = edits.created.len();
        let id = format!("c:{k}");
        let place_key = (t0.settlements.len() + t0.base_pois + k).to_string();
        for &(b, _) in &out {
            claim(&mut m, &place_key, &format!("b:{town}:{b}"));
        }
        let n = out.len() as f64;
        let (x, y) = (out.iter().map(|o| o.1[0]).sum::<f64>() / n, out.iter().map(|o| o.1[1]).sum::<f64>() / n);
        let name = crate::t0::made::name(world, t0, &id, culture::of(world, t0, town).unwrap_or(0));
        created.push(Created { id, kind: "place".into(), x, y, name, ..Default::default() });
    }
    m.retain(|_, e| !e.is_empty());
    let keys: BTreeSet<&String> = m.keys().chain(was.keys()).collect();
    let members: BTreeMap<String, Members> = keys.into_iter().filter(|k| m.get(*k) != was.get(*k)).map(|k| (k.clone(), m.get(k).cloned().unwrap_or_default())).collect();
    let report = json!({
        "town": town,
        "joined": joined,
        "left": left,
        "place": created.first().map(|c| json!({ "id": c.id, "name": c.name, "buildings": out.len() })),
        "refused": refused.iter().map(|(id, why)| json!({ "id": id, "why": why })).collect::<Vec<_>>(),
    });
    Ok(Change { members, created, refused, report })
}

/// What thing `id` (`c:<n>` or `b:<layout>:<id>`) could join: the towns whose edge lies within
/// `within` ft of it (layout, gap ft; nearest first, ties by layout), and, for a generated
/// building, why it may not be pushed out of its town (None: it may, or it is out already).
/// Both from the file and the layouts alone, as `owner` is.
pub fn join_reach(world: &World, t0: &T0, id: &str, within: f64) -> (Vec<(usize, f64)>, Option<&'static str>) {
    let idx = index(world, t0);
    let first = t0.settlements.len() + t0.base_pois;
    let (shape, building) = if let Some(k) = world.file.edits.created.iter().position(|c| c.id == id).filter(|_| item_of(world, id).is_some()) {
        let Some(&i) = idx.of_layout.get(&(first + k)) else { return (Vec::new(), None) };
        (idx.items[i as usize].shape.clone(), None)
    } else if let Some((li, b)) = building_id(id).filter(|b| is_settlement(t0, b.0)) {
        let l = layout(world, t0, li);
        let Some(bd) = l.building(b as usize) else { return (Vec::new(), None) };
        let push = if idx.pushed.contains_key(&(li, b)) || may_push(&l, bd) { None } else { Some(INSIDE) };
        (Shape::new(bd.poly.clone(), true), Some((li, push)))
    } else {
        return (Vec::new(), None);
    };
    let towns = town_rects(world, t0, &idx);
    let mut out: Vec<(usize, f64)> = Vec::new();
    for t in towns.iter().filter(|t| overlaps(t.rect, grow(shape.bbox, within))) {
        if building.is_some() && is_settlement(t0, t.li) {
            continue;
        }
        let ls = edge_layouts(world, t0, &idx, t.li);
        let g = edge_gap(&shape, &edge(t0, &idx, t.li, &ls));
        if g <= within {
            out.push((t.li, g));
        }
    }
    if let Some((li, _)) = building {
        out.push((li, 0.0));
    }
    out.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    (out, building.and_then(|b| b.1))
}

const INSIDE: &str = "Inside the town: draw the town's edge in Streets";

/// Whether distance alone would join drawn thing `id` to town `town`: within `JOIN_FT` of its
/// edge, or of a member that is.
fn by_distance(world: &World, t0: &T0, town: usize, id: &str) -> bool {
    let idx = index(world, t0);
    let Some(k) = world.file.edits.created.iter().position(|c| c.id == id) else { return false };
    let Some(&i) = idx.of_layout.get(&(t0.settlements.len() + t0.base_pois + k)) else { return false };
    let e = edge(t0, &idx, town, &edge_layouts(world, t0, &idx, town));
    if edge_gap(&idx.items[i as usize].shape, &e) <= JOIN_FT {
        return true;
    }
    idx.near[i as usize].iter().any(|&(j, _)| owner(world, t0, idx.items[j as usize].li).is_some_and(|o| o.town == town))
}

/// Whether generated building `b` of town layout `l` may be pushed out of it: outside its wall
/// ring when walled; else on its outskirts (a `Sprawl` quarter or none, or a farm's or the
/// countryside's).
fn may_push(l: &Layout, b: &Building) -> bool {
    let mid = geom::mean(&b.poly);
    match wall_ring(l) {
        Some(r) => !contains(&r, mid),
        None => matches!(b.ward, Ward::Farm | Ward::Rural) || l.quarters.iter().find(|q| q.patches.iter().any(|p| p.len() > 2 && contains(p, mid))).is_none_or(|q| q.kind == QuarterKind::Sprawl),
    }
}

/// A town's wall ring: its wall runs but castle curtains, their points in order round its centre
/// (gaps closed by chords; towns' walls are star-shaped from the centre). None when unwalled.
fn wall_ring(l: &Layout) -> Option<Vec<P>> {
    let curtain = |w: &Vec<P>| l.castles.iter().any(|&(c, r)| w.iter().all(|&p| dist(p, c) <= r * 1.3 + 30.0));
    let mut pts: Vec<(f64, P)> = l.walls.iter().filter(|w| !curtain(w)).flatten().map(|&p| (libm::atan2(p[1] - l.center[1], p[0] - l.center[0]), p)).collect();
    if pts.len() < 3 {
        return None;
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1[0].total_cmp(&b.1[0])).then(a.1[1].total_cmp(&b.1[1])));
    Some(pts.into_iter().map(|p| p.1).collect())
}

// ---------------------------------------------------------------------------------------
// Shapes and gaps.

/// A thing's outline (world ft): a polygon (`closed`), a line or a point; and its box.
#[derive(Clone)]
struct Shape {
    pts: Vec<P>,
    closed: bool,
    bbox: [f64; 4],
}

impl Shape {
    fn new(pts: Vec<P>, closed: bool) -> Shape {
        let bbox = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        Shape { closed: closed && pts.len() > 2, pts, bbox }
    }

    /// Its sides (a point: one of no length).
    fn segs(&self) -> impl Iterator<Item = (P, P)> + '_ {
        let n = self.pts.len();
        let m = if self.closed { n } else { n.saturating_sub(1).max(1) };
        (0..m).map(move |i| (self.pts[i], self.pts[if self.closed { (i + 1) % n } else { (i + 1).min(n - 1) }]))
    }
}

/// A created entry's shape: a building's, castle's or plot's outline, a wall's line, else its point.
fn shape_of(c: &Created) -> Shape {
    match c.kind.as_str() {
        "building" | "castle" | "plot" if c.poly.len() > 2 => Shape::new(c.poly.clone(), true),
        "wall" if c.pts.len() > 1 => Shape::new(c.pts.clone(), false),
        _ => Shape::new(vec![[c.x, c.y]], false),
    }
}

/// The least distance between two shapes (0 where they cross or one holds the other).
fn gap(a: &Shape, b: &Shape) -> f64 {
    let holds = |s: &Shape, o: &Shape| s.closed && o.pts.iter().any(|&p| contains(&s.pts, p));
    if holds(a, b) || holds(b, a) {
        return 0.0;
    }
    let o = |p: P, q: P, r: P| cross(sub(q, p), sub(r, p));
    let mut best = f64::MAX;
    for (p, q) in a.segs() {
        for (r, t) in b.segs() {
            if o(p, q, r) * o(p, q, t) < 0.0 && o(r, t, p) * o(r, t, q) < 0.0 {
                return 0.0;
            }
            best = best.min(seg_dist(p, r, t)).min(seg_dist(q, r, t)).min(seg_dist(r, p, q)).min(seg_dist(t, p, q));
        }
    }
    best
}

fn outline_dist(poly: &[P], p: P) -> f64 {
    (0..poly.len()).map(|i| seg_dist(p, poly[i], poly[(i + 1) % poly.len()])).fold(f64::MAX, f64::min)
}

fn grow(b: [f64; 4], d: f64) -> [f64; 4] {
    [b[0] - d, b[1] - d, b[2] + d, b[3] + d]
}

fn overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

// ---------------------------------------------------------------------------------------
// The index: items, their chains, towns, hand edits (from the world file and T0 alone).

/// A drawn thing: its layout, created entry and shape.
struct Item {
    li: usize,
    shape: Shape,
}

/// Items chained within `JOIN_FT` of each other (by item, ascending), and their box.
struct Comp {
    items: Vec<u32>,
    bbox: [f64; 4],
}

/// A town: its layout and the box it may reach (a settlement's reach, a hand town's streets, a
/// place's towns'), worked out on each ask (a hand town's streets are no part of the index).
struct Town {
    li: usize,
    rect: [f64; 4],
}

struct Index {
    key: u64,
    items: Vec<Item>,
    of_layout: FastMap<usize, u32>,
    /// Items within `JOIN_FT` of each item, and how far.
    near: Vec<Vec<(u32, f64)>>,
    comp: Vec<u32>,
    comps: Vec<Comp>,
    /// Joined by hand: item → town.
    hand: FastMap<u32, usize>,
    /// Kept out by hand: (item, town).
    kept_out: FastSet<(u32, usize)>,
    /// Generated buildings pushed out: (town layout, id) → place layout.
    pushed: FastMap<(usize, u32), usize>,
}

/// What a component's owners were worked out from: its candidate towns and the layouts read.
struct Resolved {
    towns: Vec<usize>,
    layouts: Vec<Rc<Layout>>,
    owners: Rc<[Option<Owner>]>,
}

thread_local! {
    static KEY: RefCell<Option<((u64, usize, usize, usize, usize, u64, usize, usize), u64)>> = const { RefCell::new(None) };
    static INDEX: RefCell<Option<Rc<Index>>> = const { RefCell::new(None) };
    /// By (index key, component).
    static RESOLVED: RefCell<FastMap<(u64, u32), Resolved>> = RefCell::new(FastMap::default());
    /// The towns' reach (`town_rects`), by index key.
    static RECTS: RefCell<Option<((u64, u64), Rc<Vec<Town>>)>> = const { RefCell::new(None) };
}

/// A created entry that is an item: a drawn thing standing (`world::CREATABLE`), by id.
fn item_of<'a>(world: &'a World, id: &str) -> Option<&'a Created> {
    world.file.edits.created.iter().find(|c| c.id == id && !c.removed && crate::world::CREATABLE.contains(&c.kind.as_str()))
}

/// Layout `li`'s created entry if it is a place standing.
fn place_of<'a>(world: &'a World, t0: &T0, li: usize) -> Option<&'a Created> {
    let k = li.checked_sub(t0.settlements.len() + t0.base_pois)?;
    world.file.edits.created.get(k).filter(|c| c.kind == "place" && !c.removed)
}

/// `b:<layout>:<id>`'s parts.
fn building_id(id: &str) -> Option<(usize, u32)> {
    let (li, b) = id.strip_prefix("b:")?.split_once(':')?;
    Some((li.parse().ok()?, b.parse().ok()?))
}

/// What the index is built from: the world, its layout numbering, the created entries and the
/// members set by hand.
fn index_key(world: &World, t0: &T0) -> u64 {
    // (A cheap pass first: the same buffers, lengths and coordinate sums give the key as last time.)
    let edits = &world.file.edits;
    let mut sum = 0u64;
    for (k, c) in edits.created.iter().enumerate() {
        let k = k as u64 + 1;
        sum = sum.wrapping_add(k.wrapping_mul(c.x.to_bits() ^ c.y.to_bits().rotate_left(7) ^ (c.removed as u64) ^ c.kind.len() as u64));
        for p in c.poly.iter().chain(&c.pts) {
            sum = sum.wrapping_add(k.wrapping_mul(p[0].to_bits() ^ p[1].to_bits().rotate_left(13)));
        }
    }
    let mut ids = 0usize;
    for (k, e) in &edits.members {
        let mut h = crate::core::hash::Fnv64::default();
        h.write(k.as_bytes());
        e.add.iter().for_each(|i| h.write(i.as_bytes()));
        h.write(&[0xff]);
        e.remove.iter().for_each(|i| h.write(i.as_bytes()));
        ids = ids.wrapping_add(h.finish() as usize);
    }
    let quick = (world.hash, edits.created.as_ptr() as usize, edits.created.len(), edits.members.len(), ids, sum, t0.settlements.len(), t0.base_pois);
    if let Some(k) = KEY.with(|c| c.borrow().filter(|e| e.0 == quick).map(|e| e.1)) {
        return k;
    }
    let key = index_key_now(world, t0);
    KEY.with(|c| *c.borrow_mut() = Some((quick, key)));
    key
}

fn index_key_now(world: &World, t0: &T0) -> u64 {
    let mut h = crate::core::hash::Fnv64::default();
    let edits = &world.file.edits;
    for v in [world.hash, t0.settlements.len() as u64, t0.base_pois as u64, edits.created.len() as u64] {
        h.write(&v.to_le_bytes());
    }
    for c in &edits.created {
        h.write(c.id.as_bytes());
        h.write(c.kind.as_bytes());
        h.write(&[c.removed as u8, c.poly.len() as u8, c.pts.len() as u8]);
        for p in std::iter::once(&[c.x, c.y]).chain(&c.poly).chain(&c.pts) {
            h.write(&p[0].to_bits().to_le_bytes());
            h.write(&p[1].to_bits().to_le_bytes());
        }
    }
    if !edits.members.is_empty() {
        h.write(&serde_json::to_vec(&edits.members).unwrap_or_default());
    }
    h.finish()
}

fn index(world: &World, t0: &T0) -> Rc<Index> {
    let key = index_key(world, t0);
    if let Some(idx) = INDEX.with(|c| c.borrow().as_ref().filter(|i| i.key == key).cloned()) {
        return idx;
    }
    let idx = Rc::new(build_index(world, t0, key));
    INDEX.with(|c| *c.borrow_mut() = Some(idx.clone()));
    RESOLVED.with(|r| r.borrow_mut().clear());
    idx
}

fn build_index(world: &World, t0: &T0, key: u64) -> Index {
    let edits = &world.file.edits;
    let first = t0.settlements.len() + t0.base_pois;
    let items: Vec<Item> = edits.created.iter().enumerate().filter(|(_, c)| !c.removed && crate::world::CREATABLE.contains(&c.kind.as_str())).map(|(k, c)| Item { li: first + k, shape: shape_of(c) }).collect();
    let of_layout: FastMap<usize, u32> = items.iter().enumerate().map(|(i, it)| (it.li, i as u32)).collect();
    // Items within `JOIN_FT` of each other, through a grid of their boxes.
    const CELL: f64 = 600.0;
    let cells = |b: [f64; 4]| {
        let (x0, y0, x1, y1) = (crate::core::floor(b[0] / CELL) as i64, crate::core::floor(b[1] / CELL) as i64, crate::core::floor(b[2] / CELL) as i64, crate::core::floor(b[3] / CELL) as i64);
        (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y)))
    };
    let mut grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
    for (i, it) in items.iter().enumerate() {
        for c in cells(it.shape.bbox) {
            grid.entry(c).or_default().push(i as u32);
        }
    }
    let mut near: Vec<Vec<(u32, f64)>> = vec![Vec::new(); items.len()];
    for (i, it) in items.iter().enumerate() {
        let reach = grow(it.shape.bbox, JOIN_FT);
        let mut seen: FastSet<u32> = FastSet::default();
        for c in cells(reach) {
            for &j in grid.get(&c).into_iter().flatten() {
                if j as usize <= i || !seen.insert(j) || !overlaps(reach, items[j as usize].shape.bbox) {
                    continue;
                }
                let g = gap(&it.shape, &items[j as usize].shape);
                if g <= JOIN_FT {
                    near[i].push((j, g));
                    near[j as usize].push((i as u32, g));
                }
            }
        }
    }
    near.iter_mut().for_each(|n| n.sort_by_key(|e| e.0));
    // Their chains (each numbered by its first item).
    let mut comp: Vec<u32> = vec![u32::MAX; items.len()];
    let mut comps: Vec<Comp> = Vec::new();
    for s in 0..items.len() {
        if comp[s] != u32::MAX {
            continue;
        }
        let c = comps.len() as u32;
        let (mut stack, mut list) = (vec![s as u32], Vec::new());
        comp[s] = c;
        while let Some(i) = stack.pop() {
            list.push(i);
            for &(j, _) in &near[i as usize] {
                if comp[j as usize] == u32::MAX {
                    comp[j as usize] = c;
                    stack.push(j);
                }
            }
        }
        list.sort_unstable();
        let bbox = list.iter().map(|&i| items[i as usize].shape.bbox).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]);
        comps.push(Comp { items: list, bbox });
    }
    // Hand edits: joins, keeps out, buildings pushed out (into places standing only).
    let item_at = |id: &str| edits.created.iter().position(|c| c.id == id).and_then(|k| of_layout.get(&(first + k)).copied());
    let (mut hand, mut kept_out, mut pushed) = (FastMap::default(), FastSet::default(), FastMap::default());
    for (k, e) in &edits.members {
        let Ok(town) = k.parse::<usize>() else { continue };
        let place = place_of(world, t0, town).is_some();
        if !is_settlement(t0, town) && !place {
            continue;
        }
        for id in &e.add {
            if let Some(i) = item_at(id) {
                hand.insert(i, town);
            } else if let Some(b) = building_id(id).filter(|b| place && is_settlement(t0, b.0)) {
                pushed.insert(b, town);
            }
        }
        for i in e.remove.iter().filter_map(|id| item_at(id)) {
            kept_out.insert((i, town));
        }
    }
    Index { key, items, of_layout, near, comp, comps, hand, kept_out, pushed }
}

/// Every town (settlements, those added by hand, places), by layout, and how far each may reach.
/// (Kept while the index and the hand-laid streets' points hold: `streets_key`.)
fn town_rects(world: &World, t0: &T0, idx: &Index) -> Rc<Vec<Town>> {
    let key = (idx.key, streets_key(&world.file.edits.streets));
    if let Some(t) = RECTS.with(|r| r.borrow().as_ref().filter(|e| e.0 == key).map(|e| e.1.clone())) {
        return t;
    }
    let t = Rc::new(town_rects_now(world, t0, idx));
    RECTS.with(|r| *r.borrow_mut() = Some((key, t.clone())));
    t
}

/// What a hand town's reach is read from (`hand::bounds`): its entries' keys, origins, extents,
/// points and bends, hashed without building anything.
fn streets_key(streets: &BTreeMap<String, Value>) -> u64 {
    if streets.is_empty() {
        return 0;
    }
    let mut h = crate::core::hash::Fnv64::default();
    let num = |h: &mut crate::core::hash::Fnv64, v: Option<&Value>| h.write(&v.and_then(Value::as_f64).unwrap_or(f64::NAN).to_bits().to_le_bytes());
    for (k, v) in streets {
        for x in [v.get("origin").and_then(|o| o.get(0)), v.get("origin").and_then(|o| o.get(1)), v.get("extent"), v.get("p").and_then(|p| p.get(0)), v.get("p").and_then(|p| p.get(1))] {
            num(&mut h, x);
        }
        for p in v.get("bends").and_then(Value::as_array).into_iter().flatten() {
            num(&mut h, p.get(0));
            num(&mut h, p.get(1));
        }
        h.write(k.as_bytes());
    }
    h.finish()
}

fn town_rects_now(world: &World, t0: &T0, idx: &Index) -> Vec<Town> {
    let edits = &world.file.edits;
    let first = t0.settlements.len() + t0.base_pois;
    let streets = &edits.streets;
    let circle = |x: f64, y: f64, r: f64| [x - r, y - r, x + r, y + r];
    let rect_of = |li: usize| -> Option<[f64; 4]> {
        let s = settlement(t0, li)?;
        Some(if streets.is_empty() { None } else { hand::bounds(streets, li) }.unwrap_or_else(|| circle(s.x, s.y, reach(s))))
    };
    let mut towns: Vec<Town> = (0..t0.settlements.len()).chain(t0.made.iter().map(|m| t0.settlements.len() + m.poi)).filter_map(|li| Some(Town { li, rect: rect_of(li)? })).collect();
    for (k, c) in edits.created.iter().enumerate().filter(|(_, c)| c.kind == "place" && !c.removed) {
        let li = first + k;
        let rect = idx.pushed.iter().filter(|e| *e.1 == li).filter_map(|(b, _)| rect_of(b.0)).fold(circle(c.x, c.y, 0.0), |a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]);
        towns.push(Town { li, rect });
    }
    towns.sort_by_key(|t| t.li);
    towns
}

// ---------------------------------------------------------------------------------------
// Owners: one chain at a time.

/// A town's edge: its buildings (but those pushed out) and walls, and what counts as inside it
/// (its quarters' patches, its wall ring).
struct Edge {
    shapes: Vec<Shape>,
    inside: Vec<Shape>,
}

/// The layouts town `town`'s edge is read from: its own, or a place's towns'.
fn edge_layouts(world: &World, t0: &T0, idx: &Index, town: usize) -> Vec<Rc<Layout>> {
    if is_settlement(t0, town) {
        return vec![layout(world, t0, town)];
    }
    let lis: BTreeSet<usize> = idx.pushed.iter().filter(|e| *e.1 == town).map(|(b, _)| b.0).collect();
    lis.into_iter().map(|li| layout(world, t0, li)).collect()
}

fn edge(t0: &T0, idx: &Index, town: usize, layouts: &[Rc<Layout>]) -> Edge {
    if !is_settlement(t0, town) {
        let shapes = layouts.iter().flat_map(|l| l.buildings.iter().filter(|b| idx.pushed.get(&(l.index as usize, b.id)) == Some(&town)).map(|b| Shape::new(b.poly.clone(), true))).collect();
        return Edge { shapes, inside: Vec::new() };
    }
    let l = &layouts[0];
    let mut shapes: Vec<Shape> = l.buildings.iter().filter(|b| !idx.pushed.contains_key(&(town, b.id))).map(|b| Shape::new(b.poly.clone(), true)).collect();
    shapes.extend(l.walls.iter().filter(|w| w.len() > 1).map(|w| Shape::new(w.clone(), false)));
    let mut inside: Vec<Shape> = l.quarters.iter().flat_map(|q| &q.patches).filter(|p| p.len() > 2).map(|p| Shape::new(p.clone(), true)).collect();
    inside.extend(wall_ring(l).map(|r| Shape::new(r, true)));
    Edge { shapes, inside }
}

/// How far item shape `s` lies from a town's edge (0 inside it; MAX beyond `JOIN_FT`'s reach).
fn edge_gap(s: &Shape, e: &Edge) -> f64 {
    if e.inside.iter().any(|q| overlaps(q.bbox, s.bbox) && s.pts.iter().any(|&p| contains(&q.pts, p))) {
        return 0.0;
    }
    let reach = grow(s.bbox, JOIN_FT);
    e.shapes.iter().filter(|o| overlaps(reach, o.bbox)).map(|o| gap(s, o)).fold(f64::MAX, f64::min)
}

/// The owners of component `c`'s items (in its order): a greedy claim, nearest first, over
/// (gap, town, item), all of them totally ordered.
fn resolve(world: &World, t0: &T0, idx: &Index, all: &[Town], c: u32) -> Rc<[Option<Owner>]> {
    let comp = &idx.comps[c as usize];
    let reach = grow(comp.bbox, JOIN_FT);
    let mut towns: Vec<usize> = all.iter().filter(|t| overlaps(t.rect, reach)).map(|t| t.li).collect();
    towns.extend(comp.items.iter().filter_map(|i| idx.hand.get(i)));
    towns.sort_unstable();
    towns.dedup();
    let read: Vec<Vec<Rc<Layout>>> = towns.iter().map(|&t| edge_layouts(world, t0, idx, t)).collect();
    let layouts: Vec<Rc<Layout>> = read.iter().flatten().cloned().collect();
    let kept = RESOLVED.with(|r| {
        r.borrow().get(&(idx.key, c)).filter(|m| m.towns == towns && m.layouts.len() == layouts.len() && m.layouts.iter().zip(&layouts).all(|(a, b)| Rc::ptr_eq(a, b))).map(|m| m.owners.clone())
    });
    if let Some(o) = kept {
        return o;
    }
    let local = |i: u32| comp.items.binary_search(&i).unwrap();
    let mut owners: Vec<Option<Owner>> = vec![None; comp.items.len()];
    let mut heap: BinaryHeap<Reverse<(u64, usize, u32)>> = BinaryHeap::new();
    let push_near = |heap: &mut BinaryHeap<Reverse<(u64, usize, u32)>>, owners: &[Option<Owner>], i: u32, town: usize| {
        for &(j, g) in &idx.near[i as usize] {
            if owners[local(j)].is_none() && !idx.kept_out.contains(&(j, town)) {
                heap.push(Reverse((g.to_bits(), town, j)));
            }
        }
    };
    // Joined by hand first, whatever is nearer.
    for (n, &i) in comp.items.iter().enumerate() {
        if let Some(&t) = idx.hand.get(&i) {
            owners[n] = Some(Owner { town: t, why: Why::Hand });
        }
    }
    for &i in &comp.items {
        if let Some(&t) = idx.hand.get(&i) {
            push_near(&mut heap, &owners, i, t);
        }
    }
    for (&t, ls) in towns.iter().zip(&read) {
        let e = edge(t0, idx, t, ls);
        for (n, &i) in comp.items.iter().enumerate() {
            if owners[n].is_some() || idx.kept_out.contains(&(i, t)) {
                continue;
            }
            let g = edge_gap(&idx.items[i as usize].shape, &e);
            if g <= JOIN_FT {
                heap.push(Reverse((g.to_bits(), t, i)));
            }
        }
    }
    while let Some(Reverse((g, t, i))) = heap.pop() {
        let n = local(i);
        if owners[n].is_some() {
            continue;
        }
        owners[n] = Some(Owner { town: t, why: Why::Gap(f64::from_bits(g) as f32) });
        push_near(&mut heap, &owners, i, t);
    }
    let owners: Rc<[Option<Owner>]> = owners.into();
    RESOLVED.with(|r| {
        let mut r = r.borrow_mut();
        if r.len() > 64 {
            r.clear();
        }
        r.insert((idx.key, c), Resolved { towns, layouts, owners: owners.clone() });
    });
    owners
}
