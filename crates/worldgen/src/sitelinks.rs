//! An entrance made in a building's cellar (`Created::in_building`, `way`): which building a
//! created entrance is in (when that building still stands with a cellar), which entrances lead
//! down from a building, the dungeon's name (its entrance's) and the way up in the dungeon
//! that leads back to the cellar. The hooks are one call each in `under::generate`,
//! `interior::cellar_links` and the battlemap's entrance loop.

use crate::interior::{Interior, Link};
use crate::t0::T0;
use crate::town::{self, Structure};
use crate::under::UnderKind;
use crate::world::{Created, World};

/// The first layout of the created sites (the points of interest follow the settlements, the
/// created ones follow the generated points).
fn first_layout(t0: &T0) -> usize {
    t0.settlements.len() + t0.base_pois
}

/// The created site that is layout `layout`, if it is one.
pub fn created_of<'a>(world: &'a World, t0: &T0, layout: usize) -> Option<&'a Created> {
    world.file.edits.created.get(layout.checked_sub(first_layout(t0))?)
}

/// `b:<layout>:<building>` as numbers.
pub fn parse_building(id: &str) -> Option<(usize, usize)> {
    let mut it = id.strip_prefix("b:")?.split(':');
    let (l, b) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    it.next().is_none().then_some((l, b))
}

/// The building a created entrance is in, if it stands: live, roofed, with a cellar.
pub fn host_of(world: &World, t0: &T0, c: &Created) -> Option<(usize, usize)> {
    if c.kind != "entrance" || c.removed {
        return None;
    }
    let (li, bi) = parse_building(c.in_building.as_deref()?)?;
    if li >= town::layout_count(t0) {
        return None;
    }
    let l = town::layout(world, t0, li);
    let b = l.building(bi)?;
    (b.structure == Structure::Roofed && crate::interior::has_cellar(b)).then_some((li, bi))
}

/// The building that layout `layout` (an entrance) is in, if it is one in a building that stands.
pub fn building_of(world: &World, t0: &T0, layout: usize) -> Option<(usize, usize)> {
    host_of(world, t0, created_of(world, t0, layout)?)
}

/// The entrances that lead down from building `bi` of layout `li` (the layout, the entrance), in
/// the order they were made.
pub fn entrances_in<'a>(world: &'a World, t0: &T0, li: usize, bi: usize) -> Vec<(usize, &'a Created)> {
    // (Filtered by the `in_building` string first: `host_of` lays out a town, so it runs only for the few entrances that name this building.)
    let mine = format!("b:{li}:{bi}");
    let first = first_layout(t0);
    world
        .file
        .edits
        .created
        .iter()
        .enumerate()
        .filter(|(_, c)| c.kind == "entrance" && !c.removed && c.in_building.as_deref() == Some(mine.as_str()))
        .filter(|(_, c)| host_of(world, t0, c) == Some((li, bi)))
        .map(|(k, c)| (first + k, c))
        .collect()
}

/// Whether created entrance `c` takes a trapdoor (else stairs).
pub fn trapdoor(c: &Created) -> bool {
    c.way.as_deref() == Some("trapdoor")
}

/// The dungeon's name: its entrance's own, if it has one.
pub fn dungeon_name(world: &World, t0: &T0, layout: usize) -> Option<String> {
    let c = created_of(world, t0, layout)?;
    let n = c.name.trim();
    (c.kind == "entrance" && !c.removed && !n.is_empty()).then(|| n.to_string())
}

/// A cellar's way down into site `kind`: the item kind, its name and what the cellar is wanted
/// to have (static strings).
pub fn way_down(kind: UnderKind, trap: bool) -> (&'static str, &'static str, &'static str) {
    macro_rules! w {
        ($n:literal) => {
            if trap {
                ("trapdoor", concat!("trapdoor down to the ", $n), concat!("the trapdoor down to the ", $n))
            } else {
                ("link_down", concat!("stairs down to the ", $n), concat!("the stairs down to the ", $n))
            }
        };
    }
    match kind {
        UnderKind::Dungeon => w!("dungeon"),
        UnderKind::Crypt => w!("crypt"),
        UnderKind::Cave => w!("cave"),
        UnderKind::Mine => w!("mine"),
        UnderKind::LavaTube => w!("lava tube"),
        UnderKind::Sewer => w!("sewer"),
        UnderKind::Catacombs => w!("catacombs"),
        UnderKind::KeepDungeon => w!("deep dungeons"),
    }
}

/// Dress the generated site `it` of layout `layout`: its entrance's name, and for an entrance in
/// a building the way up (stairs or a ladder to the trapdoor; still an `exit` item) linked to that building.
pub fn dress(world: &World, t0: &T0, layout: usize, it: &mut Interior) {
    if it.name.is_none() {
        it.name = dungeon_name(world, t0, layout);
    }
    let Some(c) = created_of(world, t0, layout) else { return };
    let Some((li, bi)) = host_of(world, t0, c) else { return };
    // (The way in stays an `exit` item, its name now the way up: the link takes priority, and a design reads its entry from it.)
    let name = if trapdoor(c) { "ladder up to the trapdoor" } else { "stairs up to the cellar" };
    for lv in it.levels.iter_mut() {
        if let Some(x) = lv.furniture.iter_mut().find(|f| f.kind == "exit") {
            x.name = name;
            let link = Link { x: x.x, y: x.y, to: format!("b:{li}:{bi}") };
            lv.links.push(link);
            return;
        }
    }
}
