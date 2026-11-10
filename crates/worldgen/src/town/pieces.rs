//! (R5 towns) A castle drawn by hand: its pieces (the keep and the yard's buildings) changed by
//! hand. The edits are kept on the castle's own entry (`Created::pieces`), never by its layout
//! number (which moves when an earlier site is deleted), and applied where the castle's layout
//! is built (`sites::generate`). Their ids stay the layout's (`b:<layout>:<id>`).

use super::geom::{centroid, dist};
use super::{Layout, Structure, catalog, sites};
use crate::World;
use crate::core::hash::FastSet;
use crate::core::rng::{Pcg32, hash2};
use crate::t0::T0;
use crate::world::{BuildingEdit, CastlePiece, Created};

/// The keep's place in a castle's buildings (pushed last; the only one laid out as a keep).
pub fn keep_index(l: &Layout) -> Option<usize> {
    l.buildings.iter().rposition(|b| b.keep.is_some())
}

/// The castle `c`'s piece edits made on its layout `l` (built and numbered): a yard building's
/// only while one stands where it was changed, the keep's wherever it stands. A yard building
/// taken away leaves the list (the others keep their ids); a ruin has no storeys.
pub fn apply(world: &World, t0: &T0, c: &Created, l: &mut Layout) {
    if c.pieces.is_empty() {
        return;
    }
    let keep = keep_index(l);
    let mut removed: FastSet<u32> = FastSet::default();
    for (k, p) in &c.pieces {
        let i = if k == CastlePiece::KEEP {
            keep
        } else {
            k.parse::<usize>().ok().and_then(|id| l.building_index(id)).filter(|&i| Some(i) != keep && dist(centroid(&l.buildings[i].poly), p.edit.at) <= BuildingEdit::SAME_AT_FT)
        };
        let Some(i) = i else { continue };
        let e = &p.edit;
        if e.removed {
            if Some(i) != keep {
                removed.insert(l.buildings[i].id);
            }
            continue;
        }
        let others: Vec<String> = if e.func.is_some() && Some(i) != keep { l.buildings.iter().enumerate().filter(|(j, o)| *j != i && o.func.is_some()).filter_map(|(_, o)| o.name.clone()).collect() } else { Vec::new() };
        let b = &mut l.buildings[i];
        if let Some(f) = &e.func {
            let (func, residential) = sites::func_of(f, false);
            if (func, residential) != (b.func, b.residential) {
                b.func = func;
                b.residential = residential;
                // (The keep stays named as the castle; a yard building is named for its new trade.)
                if Some(i) != keep {
                    let mut rng = Pcg32::new(hash2(world.stream("castle.piece"), crate::core::hash::fnv64(c.id.as_bytes()) as i64, b.id as i64), 47);
                    b.name = sites::trade_name(func, &mut rng, t0, l.center, others.iter().map(String::as_str));
                }
            }
        }
        sites::apply_looks(b, e.roof.as_deref(), e.tint.as_deref(), e.structure.as_deref());
        sites::apply_roof(b, e.material.as_deref(), e.shape.as_deref(), e.shade);
        if b.structure == Structure::Ruin {
            if b.func.is_none() && e.func.is_none() {
                b.residential = catalog::RUINED as u8;
            }
        } else if let Some(f) = e.floors {
            b.floors = f.max(1);
        }
        if let Some(n) = p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            b.name = Some(n.to_string());
        }
    }
    if !removed.is_empty() {
        l.buildings.retain(|b| !removed.contains(&b.id));
    }
}

/// A piece of a castle drawn by hand, by its building id (`b:<layout>:<id>`).
pub struct Piece {
    /// Its castle's place in `Edits.created`, and its `c:` id.
    pub created: usize,
    pub castle: String,
    pub castle_name: String,
    pub layout: usize,
    pub id: u32,
    /// Its key in `Created::pieces`: `keep`, or its id.
    pub key: String,
    /// Its middle (none when taken away).
    pub at: Option<[f64; 2]>,
    /// The castle's structure (`roofed` or `ruin`): its pieces' as built.
    pub ruin: bool,
}

impl Piece {
    pub fn keep(&self) -> bool {
        self.key == CastlePiece::KEEP
    }

    /// What it is, for replies: "the keep of Candlekeep (c:4)", "a yard building of …".
    pub fn what(&self) -> String {
        format!("{} of {} ({}), a castle made by hand", if self.keep() { "the keep" } else { "a yard building" }, self.castle_name, self.castle)
    }
}

/// `id`'s layout and building id, if it reads `b:<layout>:<id>`.
fn parse(id: &str) -> Option<(usize, u32)> {
    let mut parts = id.split(':');
    if parts.next()? != "b" {
        return None;
    }
    let li = parts.next()?.parse().ok()?;
    let bi = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((li, bi))
}

/// The castle piece `id` is, if it is one: a building of a castle drawn by hand standing, or one
/// of its yard's taken away (`Created::pieces`).
pub fn piece_of(world: &World, t0: &T0, id: &str) -> Option<Piece> {
    let (li, bi) = parse(id)?;
    let first = t0.settlements.len() + t0.base_pois;
    let k = li.checked_sub(first)?;
    let c = world.file.edits.created.get(k).filter(|c| c.kind == "castle" && !c.removed)?;
    if li >= super::layout_count(t0) {
        return None;
    }
    let l = super::layout(world, t0, li);
    let piece = |key: String, at: Option<[f64; 2]>| Piece { created: k, castle: c.id.clone(), castle_name: c.name.clone(), layout: li, id: bi, key, at, ruin: c.structure.as_deref() == Some("ruin") };
    match l.building_index(bi as usize) {
        Some(i) => Some(piece(if Some(i) == keep_index(&l) { CastlePiece::KEEP.to_string() } else { bi.to_string() }, Some(centroid(&l.buildings[i].poly)))),
        None => c.pieces.get(&bi.to_string()).filter(|p| p.edit.removed).map(|_| piece(bi.to_string(), None)),
    }
}

/// What `id` is when it is not one of the world's own buildings, for the refusal: a castle
/// piece, a building drawn by hand (by its `c:` id), a building of another site made by hand,
/// or nothing at all.
pub fn not_generated(world: &World, t0: &T0, id: &str) -> String {
    if let Some(p) = piece_of(world, t0, id) {
        return format!("{id} is {}, not a building of the world's own", p.what());
    }
    let Some((li, bi)) = parse(id) else {
        return if id.starts_with("c:") { format!("{id} is a site made by hand: a building drawn by hand is changed by that id, a castle's pieces by their building ids (b:<layout>:<id>)") } else { format!("{id} is not a building id (b:<layout>:<id>)") };
    };
    let first = t0.settlements.len() + t0.base_pois;
    if li >= super::layout_count(t0) {
        return format!("{id}: there is no place {li} (places run 0 to {})", super::layout_count(t0).saturating_sub(1));
    }
    let Some(c) = li.checked_sub(first).and_then(|k| world.file.edits.created.get(k)) else {
        return format!("no such building: {id}");
    };
    let named = if c.name.trim().is_empty() { c.id.clone() } else { format!("{} ({})", c.name.trim(), c.id) };
    if c.removed {
        return format!("{id} was in {named}, a site made by hand and deleted since");
    }
    match c.kind.as_str() {
        "building" => format!("{id} is {named}, a building drawn by hand: it goes by its id {} (update_building changes it, delete_feature takes it away)", c.id),
        "castle" => {
            let n = super::layout(world, t0, li).buildings.last().map_or(0, |b| b.id + 1);
            format!("{named}, a castle made by hand, has no building {bi}: its pieces are b:{li}:0 to b:{li}:{}", n.saturating_sub(1))
        }
        k => format!("{id} is in {named}, a {k} made by hand: its buildings are laid out with it and are not changed one by one"),
    }
}

/// The edit of castle piece `p` with `change` made (`name`: `Some("")` back to as built), and
/// what it changes into: `None` when it is then as built. Footprints stay the castle's; a
/// ruin has no storeys.
pub fn piece_edit(world: &World, p: &Piece, change: &crate::agent::BuildingChange, name: Option<&str>) -> Result<Option<CastlePiece>, String> {
    let c = &world.file.edits.created[p.created];
    let Some(at) = p.at else { return Err(format!("b:{}:{} was taken away from {}: restore it first (restore_building)", p.layout, p.id, p.castle_name)) };
    if change.poly.is_some() {
        return Err(format!("b:{}:{} is {}: its footprint is the castle's (redraw the castle with update_fortification)", p.layout, p.id, p.what()));
    }
    // (A yard building's edit made where another stood is not its own.)
    let mut e = c.pieces.get(&p.key).filter(|e| p.keep() || dist(e.edit.at, at) <= BuildingEdit::SAME_AT_FT).cloned().unwrap_or_default();
    e.edit.at = at;
    let keep = |v: &Option<String>| v.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "auto");
    for (v, list, what) in [(&change.material, &super::roof::MATERIALS[..], "material"), (&change.shape, &super::roof::SHAPES[..], "shape"), (&change.tint, &crate::world::TINTS[..], "tint"), (&change.roof, &crate::world::ROOFS[..], "roof")] {
        if let Some(v) = keep(v)
            && !list.contains(&v.as_str())
        {
            return Err(format!("{what} must be one of {}, or auto", list.join(", ")));
        }
    }
    for (v, slot) in [(&change.func, &mut e.edit.func), (&change.roof, &mut e.edit.roof), (&change.tint, &mut e.edit.tint), (&change.material, &mut e.edit.material), (&change.shape, &mut e.edit.shape)] {
        if v.is_some() {
            *slot = keep(v);
        }
    }
    if let Some(s) = change.shade {
        if s.is_some_and(|s| s > 4) {
            return Err("shade must be 0 to 4 (2 the usual), or auto".into());
        }
        e.edit.shade = s;
    }
    let built = if p.ruin { "ruin" } else { "roofed" };
    if change.structure.is_some() {
        e.edit.structure = keep(&change.structure).filter(|s| s != built);
    }
    let ruin = e.edit.structure.as_deref().unwrap_or(built) == "ruin";
    if let Some(f) = change.floors {
        if ruin {
            return Err(format!("b:{}:{} ({}) is a ruin: a roofless shell has no storeys (its function, name and looks can change; structure: roofed rebuilds it)", p.layout, p.id, p.what()));
        }
        e.edit.floors = Some(f);
    }
    // (Storeys set before it fell stay with it, for when it is roofed again.)
    if let Some(n) = name {
        e.name = Some(n.trim().to_string()).filter(|n| !n.is_empty());
    }
    e.edit.check()?;
    Ok((!e.is_noop()).then_some(e))
}

/// The edit that takes castle piece `p` (a yard building) away; the keep goes with the castle's
/// `keep: false`.
pub fn piece_removal(p: &Piece) -> Result<CastlePiece, String> {
    if p.keep() {
        return Err(format!("b:{}:{} is {}: a castle goes without its keep by update_fortification keep: false", p.layout, p.id, p.what()));
    }
    let at = p.at.ok_or_else(|| format!("b:{}:{} was taken away already", p.layout, p.id))?;
    Ok(CastlePiece { edit: BuildingEdit { at, removed: true, ..Default::default() }, name: None })
}
