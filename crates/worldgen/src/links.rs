//! Paired exits put down by hand (`Edits::links`, `j:<id>`): a square on a level of a site
//! underground (`u:`, `k:`), of a sewer section (`w:<layout>:<sx>:<sy>`) or of a building's cellar
//! (`b:`, a level below ground) joined to another such square. Each end shows on its own map as a
//! `link` item with a way (`Level::links`) naming the far site, its level, square and name; the
//! furniture under it gives way. Only places a link names change (`dress`, the one hook, in
//! `interior::generate_id`): a world without links builds as before.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::interior::{Far, Interior, Item, Level, Link as Way};
use crate::t0::T0;
use crate::world::{Edits, World};

/// The item kind each end shows as.
pub const KIND: &str = "link";
/// How a link goes (its item's name; `variant` is its place here): the first is the default.
pub const WAYS: [&str; 6] = ["stairs", "ladder", "passage", "trapdoor", "rope", "portal"];

fn stairs() -> String {
    WAYS[0].into()
}

/// One end: a level (`l:<site>:<level>`, levels counted from the bottom) and a square on it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct End {
    pub level: String,
    pub x: u16,
    pub y: u16,
}

/// A paired exit: two ends, how it goes (`WAYS`) and a name of its own, if it has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub a: End,
    pub b: End,
    #[serde(default = "stairs")]
    pub way: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// The way's own static name (an unknown one reads as stairs) and its place in `WAYS`.
pub fn way_of(way: &str) -> (&'static str, usize) {
    WAYS.iter().position(|w| *w == way).map_or((WAYS[0], 0), |i| (WAYS[i], i))
}

/// The site and level an end's level id names: `l:<site>:<level>` of a site underground, a sewer
/// section or a building (`u:`, `k:`, `w:`, `b:`).
pub fn site_of(level: &str) -> Option<(&str, usize)> {
    let (site, li, room) = crate::agent::split_room_id(level)?;
    let ok = room.is_none() && level.starts_with("l:") && matches!(site.as_bytes(), [b'u' | b'k' | b'w' | b'b', b':', ..]);
    ok.then_some((site, li))
}

/// A level's id.
pub fn level_id(site: &str, li: usize) -> String {
    format!("l:{site}:{li}")
}

/// A site's name as a link's far end gives it: renamed, else its own (a sewer section's: its
/// sewers'; generating nothing), with its level's name after it where that was renamed.
pub fn site_name(world: &World, t0: &T0, level: &str) -> String {
    let Some((site, _)) = site_of(level) else { return level.to_string() };
    let renames = &world.file.edits.renames;
    // (A sewer section by its sewers' name, as `undersearch` gives it.)
    let sewers = || {
        let li: usize = site.split(':').nth(1)?.parse().ok()?;
        let named = renames.get(&format!("w:{li}")).cloned();
        named.or_else(|| crate::town::is_settlement(t0, li).then(|| format!("Sewers of {}", crate::agent::whereabouts(world, t0, li).0["name"].as_str().unwrap_or("the city"))))
    };
    let mut name = match renames.get(site) {
        Some(n) => n.clone(),
        None if site.starts_with("w:") => sewers().unwrap_or_else(|| "Sewers".into()),
        None => crate::agent::name_any(world, t0, site).unwrap_or_else(|| site.to_string()),
    };
    if let Some(n) = renames.get(level) {
        name = format!("{name} › {n}");
    }
    name
}

/// Whether an item stays where a link would go (a way, a key, a picture, a hazard, another link).
fn fixed(f: &Item) -> bool {
    f.kind == KIND || f.hazard.is_some() || crate::sitelinks::KEPT.contains(&f.kind)
}

fn covers(f: &Item, x: u16, y: u16) -> bool {
    x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h
}

/// Why square `x, y` of level `li` of `it` (site `site`) can take no link: off the grid or the
/// level, in the rock, on a building's stairs or on a cellar's ground floor and above, on a way
/// or another link. Furniture that may move gives way.
pub fn spot_problem(it: &Interior, site: &str, li: usize, x: u16, y: u16) -> Option<String> {
    let Some(lv) = it.levels.get(li) else { return Some(format!("{site} has no level {li} (levels 0-{}, from the bottom)", it.levels.len().saturating_sub(1))) };
    let (nx, ny) = (it.nx, it.ny);
    if x as usize >= nx || y as usize >= ny {
        return Some(format!("{x},{y} is off {site}'s grid ({nx} x {ny} squares)"));
    }
    if site.starts_with("b:") {
        if lv.z >= 0 {
            return Some(format!("level {li} of {site} is not a cellar (only a building's cellars take a link)"));
        }
        let s = it.stairs;
        if (x as usize) >= s[0] && (x as usize) < s[0] + s[2] && (y as usize) >= s[1] && (y as usize) < s[1] + s[3] {
            return Some(format!("{x},{y} is on {site}'s stairs"));
        }
    }
    if lv.cells[y as usize * nx + x as usize] < 0 {
        return Some(format!("{x},{y} on level {li} of {site} is not floor"));
    }
    if lv.links.iter().any(|w| w.x == x && w.y == y) {
        return Some(format!("{x},{y} on level {li} of {site} already leads elsewhere"));
    }
    if let Some(f) = lv.furniture.iter().find(|f| covers(f, x, y) && fixed(f)) {
        return Some(format!("{x},{y} on level {li} of {site} holds a {} ({}) that stays", f.kind, f.name));
    }
    None
}

/// Put the ends of the links that name site `id` on `it` (in key order; an end whose square can't
/// take one is left off: `problems` says so).
pub fn dress(world: &World, t0: &T0, id: &str, it: &mut Interior) {
    for (key, l) in &world.file.edits.links {
        for (here, there) in [(&l.a, &l.b), (&l.b, &l.a)] {
            let (Some((site, li)), Some((far, fli))) = (site_of(&here.level), site_of(&there.level)) else { continue };
            if site != id || spot_problem(it, site, li, here.x, here.y).is_some() {
                continue;
            }
            let (way, look) = way_of(&l.way);
            let to = Far { level: fli, x: there.x, y: there.y, name: site_name(world, t0, &there.level), label: l.name.clone().filter(|n| !n.trim().is_empty()), way };
            put(&mut it.levels[li], here.x, here.y, way, look, Way { x: here.x, y: here.y, to: far.to_string(), id: key.clone(), far: Some(to) });
        }
    }
}

fn put(lv: &mut Level, x: u16, y: u16, way: &'static str, look: usize, link: Way) {
    lv.furniture.retain(|f| !covers(f, x, y));
    lv.furniture.push(Item { kind: KIND, name: way, x, y, w: 1, h: 1, cover: 0, blocks_move: false, height_ft: 0.0, hazard: None, sprite: 0, back: None, variant: Some(look as u8) });
    lv.links.push(link);
}

/// The links with an end at `id`: a site (`u:`, `k:`, `w:`, `b:`) or one of its levels (`l:`).
pub fn of<'a>(edits: &'a Edits, id: &'a str) -> impl Iterator<Item = (&'a String, &'a Link)> + 'a {
    let at = move |e: &End| e.level == id || site_of(&e.level).is_some_and(|(s, _)| s == id);
    edits.links.iter().filter(move |(_, l)| at(&l.a) || at(&l.b))
}

/// A link as the agent tools give it: id, way, name, and each end's level, site, square and name.
pub fn link_json(world: &World, t0: &T0, key: &str, l: &Link) -> Value {
    let end = |e: &End| json!({ "level": e.level, "site": site_of(&e.level).map(|s| s.0), "x": e.x, "y": e.y, "name": site_name(world, t0, &e.level) });
    let mut v = json!({ "id": key, "way": way_of(&l.way).0, "a": end(&l.a), "b": end(&l.b) });
    if let Some(n) = &l.name {
        v["name"] = json!(n);
    }
    v
}

/// What keeps link `key` from showing at either end (empty: both maps show it, on floor,
/// leading to each other).
pub fn problems(world: &World, t0: &T0, key: &str, l: &Link) -> Vec<String> {
    let mut out = Vec::new();
    for (here, there) in [(&l.a, &l.b), (&l.b, &l.a)] {
        let Some((site, li)) = site_of(&here.level) else {
            out.push(format!("{}: not a level of a site underground, a sewer section or a cellar", here.level));
            continue;
        };
        let Some(it) = crate::interior::generate_id(world, t0, site) else {
            out.push(format!("{site} is not there"));
            continue;
        };
        let shown = it.levels.get(li).and_then(|lv| lv.links.iter().find(|w| w.id == key && (w.x, w.y) == (here.x, here.y)).map(|w| (lv, w)));
        match shown {
            Some((lv, w)) => {
                let far = site_of(&there.level).map(|f| f.0);
                let item = lv.furniture.iter().any(|f| f.kind == KIND && (f.x, f.y) == (w.x, w.y));
                if !item || lv.cells[w.y as usize * it.nx + w.x as usize] < 0 || far != Some(w.to.as_str()) {
                    out.push(format!("{}: shown at {},{} leading to {}", here.level, w.x, w.y, w.to));
                }
            }
            // (Not put down: why.)
            None => out.push(format!("{}: {}", here.level, spot_problem(&it, site, li, here.x, here.y).unwrap_or_else(|| "not shown".into()))),
        }
    }
    out
}
