//! Underground sites designed by hand: a copy of a generated site (`u:` ids, sewers aside) that
//! the user or an agent changed: rooms painted or dug, doors, ways between levels, props, levels
//! added or taken away. `Edits::designs` keeps them by site id, and `interior::generate_id`
//! builds a designed site instead of generating it. A design holds only squares, rooms, doors
//! and items; walls are derived as for a generated site (`Plan::finish`).
//!
//! `check` holds a site to the rules every generated one meets (tests/vital/underground.rs): one way in,
//! under the entrance; each level's way down on the same square as the way up below; every
//! square reached from the way onto the level. The editor and `set_site_design` refuse a design
//! that breaks them (play mode relies on them).
//!
//! The text form (`to_text`/`from_text`) is the plan `examples/under.rs` prints, made two-way:
//! one character per square, rooms by symbol, then doors and items as lists.

use serde::Serialize;
use serde_json::{Value, json};

use super::{BOSS, KEEP_DUNGEON, LEVEL_FT, MAX_LEVELS, PROPS, Plan, SPANS, TERRAIN, THEMES, UNDERCROFT, UnderKind, blocked_squares, furnish_room, item, kit, lava, neighbours};
use crate::World;
use crate::core::rng::{Pcg32, hash3};
use crate::interior::design::{BUILDING, MAX_LOCK, PORTCULLIS, furniture_kind, lock_flags, lock_of};
use crate::interior::{Interior, Item, SQUARE_FT, Sprites, design_kind, sprite_asset};
use crate::t0::T0;
use crate::town;
pub use crate::model::under::{SiteDesign, DesignLevel, DesignRoom, DesignItem};

mod check;
mod text;
pub use check::*;
use check::flood;
pub use text::*;


/// `DesignItem::posed`, here above the model crate (`model::under`).
pub trait DesignItemPose {
    fn posed(&self, it: Item) -> Item;
}

impl DesignItemPose for DesignItem {
    /// The built item with this one's facing and look (a `back` past 3 is read as none).
    fn posed(&self, mut it: Item) -> Item {
        it.back = self.back.filter(|&b| b < 4);
        it.variant = self.variant.map(|v| v.min(crate::interior::item_looks(it.kind) - 1));
        it
    }
}

/// Something wrong with a design: `blocking` ones break play mode (a design with any is not
/// saved); the rest are worth knowing. `level` is the level's index (bottom to top), `at` a
/// square to look at.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Problem {
    pub level: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<[u16; 2]>,
    pub text: String,
    pub blocking: bool,
}

/// Most levels a building's design has (cellar, storeys, an open roof, tower tops).
pub const BUILDING_LEVELS: usize = 16;
/// Largest grid side, most rooms and items a level holds.
pub const MAX_SIDE: u16 = 200;
pub const MAX_ROOMS: usize = 500;
pub const MAX_ITEMS: usize = 3000;
/// The room kind anything unknown becomes.
pub const CHAMBER: &str = "chamber";
/// Room kinds the generators use besides the themes' own.
const EXTRA_ROOMS: &[&str] = &[
    CHAMBER,
    "landing",
    BOSS,
    "floor",
    "haulage way",
    "drift",
    "stope",
    "ore chamber",
    "powder store",
    "pump room",
    "collapsed stope",
    "shaft head",
    "adit",
    "lift landing",
    "winze",
    "tool store",
    "ore bins",
    "foreman's office",
    "bunk room",
    "corridor",
    "cavern",
    "nave",
    "side chapel",
    "crossing",
    "guard post",
    "cell row",
    "cell",
    "solitary cell",
    "junction",
    "stair hall",
    "pillared gallery",
    "chasm hall",
    "cave-in",
    "landing",
    "island",
];
/// Items that are ways: in from the surface, up and down a level.
pub const WAYS: [&str; 3] = ["exit", "up", "down"];

/// Every room kind a site can hold, each once.
pub fn room_kinds() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for t in THEMES.iter().chain([&KEEP_DUNGEON, &UNDERCROFT]) {
        for k in [t.first, t.passage, t.ledge].into_iter().chain(t.kinds.iter().copied()) {
            if !k.is_empty() && !out.contains(&k) {
                out.push(k);
            }
        }
    }
    for k in EXTRA_ROOMS {
        if !out.contains(k) {
            out.push(k);
        }
    }
    out
}

/// A room kind by name.
pub fn room_kind(s: &str) -> Option<&'static str> {
    room_kinds().into_iter().find(|k| *k == s)
}

/// An item kind by key (or a prop's name: "burial urn").
pub fn item_kind(s: &str) -> Option<&'static str> {
    WAYS.iter()
        .copied()
        .chain(["lava"])
        .find(|k| *k == s)
        .or_else(|| PROPS.iter().find(|p| p.0 == s || p.1 == s).map(|p| p.0))
}

/// A prop's usual size (squares): as the first room kit that holds it has it.
pub fn prop_size(kind: &str) -> (usize, usize) {
    room_kinds().iter().flat_map(|r| kit(r).iter()).find(|e| e.0 == kind).map(|e| (e.1, e.2)).unwrap_or((1, 1))
}

/// What can be designed (for the editor's menus): props with their rules and usual size,
/// room kinds by theme (each theme's own first), the themes.
pub fn catalog_json() -> String {
    let rooms = room_kinds();
    let size = prop_size;
    let props: Vec<Value> = PROPS
        .iter()
        .map(|p| {
            let (w, h) = size(p.0);
            json!({ "kind": p.0, "name": p.1, "cover": p.2, "blocks": p.3, "height_ft": p.4, "hazard": p.5, "w": w, "h": h })
        })
        .chain([json!({ "kind": "lava", "name": "lava channel", "cover": 0, "blocks": false, "height_ft": 0.0, "hazard": "10d10 fire on entering or starting a turn in it", "w": 1, "h": 1 })])
        .collect();
    let themes: Vec<Value> = THEMES
        .iter()
        .map(|t| {
            let mut kinds: Vec<&str> = [t.first, t.passage, t.ledge].into_iter().filter(|k| !k.is_empty()).collect();
            for k in t.kinds {
                if !kinds.contains(k) {
                    kinds.push(k);
                }
            }
            json!({ "key": t.key, "kind": t.kind.key(), "first": t.first, "passage": t.passage, "rooms": kinds })
        })
        .collect();
    json!({ "props": props, "rooms": rooms, "themes": themes, "boss": BOSS, "max_levels": MAX_LEVELS, "building": crate::interior::design::catalog() }).to_string()
}

/// `SiteDesign`'s building, checks and edits, here above the model crate (`model::under`).
pub trait SiteDesignOps {
    fn under_kind(&self) -> Option<UnderKind>;
    fn is_building(&self) -> bool;
    fn below_ground(&self) -> usize;
    fn from_interior(it: &Interior) -> SiteDesign;
    fn shape_problems(&self) -> Vec<Problem>;
    fn build(&self, id: &str, settlement: u32, building: u32, metas: &std::collections::BTreeMap<String, crate::world::SpriteMeta>) -> Interior;
    fn problems(&self, world: &World, t0: &T0, id: &str) -> Result<(Interior, Vec<Problem>), String>;
    fn add_doors(&mut self, level: Option<usize>);
    fn furnish(&mut self, world: &World, t0: &T0, id: &str, level: usize, room: usize, seed: u64);
    fn unfitted(&self, it: &Interior) -> Vec<Problem>;
    fn set_shape_checked(&mut self, world: &World, t0: &T0, id: &str, level: usize, room: usize, shape: u8) -> Result<(), String>;
    fn set_shape(&mut self, level: usize, room: usize, shape: u8) -> Result<(), String>;
}

trait SiteDesignPlan {
    fn plan(&self, lv: &DesignLevel, kind: UnderKind, nx: usize, ny: usize, sprites: &mut Sprites) -> Plan;
}

impl SiteDesignOps for SiteDesign {
    fn under_kind(&self) -> Option<UnderKind> {
        UnderKind::parse(&self.kind)
    }

    /// A building's interior (`interior::design`), not a site underground.
    fn is_building(&self) -> bool {
        self.kind == BUILDING
    }

    /// Its levels below ground (a building's cellars; every level of a site): those dug or
    /// filled in at the bottom renumber the rest (`Edits::shift_levels`).
    fn below_ground(&self) -> usize {
        if self.is_building() { self.levels.iter().filter(|l| l.z < 0).count() } else { self.levels.len() }
    }

    /// A copy of a site as it is: the design that builds it again, exactly.
    fn from_interior(it: &Interior) -> SiteDesign {
        let kind = UnderKind::CREATABLE.into_iter().find(|k| k.name() == it.function).unwrap_or(UnderKind::Dungeon);
        let top = it.levels.last();
        let entry = top.and_then(|l| l.furniture.iter().find(|f| f.kind == "exit")).map(|f| [f.x, f.y]).unwrap_or([0, 0]);
        let levels = it
            .levels
            .iter()
            .map(|lv| {
                let doors = lv
                    .doors
                    .iter()
                    .map(|d| {
                        let flags = (d.kind == "secret") as u16 | ((d.kind == "arch") as u16) << 3 | lock_flags(d.lock) | if d.kind == "portcullis" { PORTCULLIS } else { 0 };
                        let (x, y) = (d.a[0].min(d.b[0]) as u16, d.a[1].min(d.b[1]) as u16);
                        // A horizontal edge lies between the squares above and below it.
                        if d.a[1] == d.b[1] { [x, y - 1, 1, flags] } else { [x - 1, y, 0, flags] }
                    })
                    .collect();
                DesignLevel {
                    name: lv.name.clone(),
                    elevation_ft: lv.elevation_ft,
                    natural: lv.natural,
                    cells: encode(&lv.cells),
                    rooms: lv
                        .rooms
                        .iter()
                        .map(|r| DesignRoom {
                            kind: r.kind.into(),
                            raise_ft: r.raise_ft.max(0.0) as u16,
                            shape: r.shape,
                        })
                        .collect(),
                    doors,
                    items: lv
                        .furniture
                        .iter()
                        .map(|f| DesignItem {
                            kind: design_kind(it, f),
                            x: f.x,
                            y: f.y,
                            w: f.w,
                            h: f.h,
                            back: f.back,
                            variant: f.variant,
                        })
                        .collect(),
                    ..Default::default()
                }
            })
            .collect();
        SiteDesign {
            kind: kind.key().into(),
            theme: it.theme.map(str::to_string),
            look: it.look,
            origin: it.origin,
            axis: it.axis,
            nx: it.nx as u16,
            ny: it.ny as u16,
            entry,
            levels,
            ..Default::default()
        }
    }

    /// What can't be read at all (sizes, kinds, cells): a design with such a problem is built
    /// as well as it can be, but never saved.
    fn shape_problems(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        let bad = |level: usize, text: String| Problem {
            level,
            at: None,
            text,
            blocking: true,
        };
        let building = self.is_building();
        if self.under_kind().is_none() && !building {
            out.push(bad(0, format!("{} sites can't be designed", self.kind)));
        }
        if self.nx == 0 || self.ny == 0 || self.nx > MAX_SIDE || self.ny > MAX_SIDE {
            out.push(bad(0, format!("the grid must be 1 to {MAX_SIDE} squares a side")));
            return out;
        }
        let most = if building { BUILDING_LEVELS } else { MAX_LEVELS as usize };
        if self.levels.is_empty() || self.levels.len() > most {
            out.push(bad(0, format!("a site has 1 to {most} levels")));
        }
        // A building's levels go up a storey at a time, from at most three below ground.
        if building && let Some(lv) = self.levels.first() {
            let most = crate::interior::design::MAX_CELLARS as i8;
            if lv.z < -most || self.levels.windows(2).any(|w| w[1].z != w[0].z + 1) {
                out.push(bad(0, format!("a building's levels are its storeys from the bottom, one each, at most {most} below ground")));
            }
        }
        let n = self.nx as usize * self.ny as usize;
        for (li, lv) in self.levels.iter().enumerate() {
            let tag = if building { lv.name.clone() } else { depth_tag(self.levels.len(), li) };
            match decode(&lv.cells, n) {
                None => out.push(bad(li, format!("{tag}: its squares don't add up to {} x {}", self.nx, self.ny))),
                Some(c) if c.iter().any(|&r| r >= lv.rooms.len() as i16) => out.push(bad(li, format!("{tag}: a square of a room that isn't in its rooms list"))),
                _ => {}
            }
            if lv.rooms.len() > MAX_ROOMS || lv.items.len() > MAX_ITEMS || lv.doors.len() > MAX_ITEMS {
                out.push(bad(li, format!("{tag}: at most {MAX_ROOMS} rooms and {MAX_ITEMS} items and doors")));
            }
            for f in &lv.items {
                let known = sprite_asset(&f.kind).is_some() || if building { furniture_kind(&f.kind).is_some() } else { item_kind(&f.kind).is_some() };
                if !known {
                    out.push(bad(li, format!("{tag}: no such item: {}", f.kind)));
                } else if f.w == 0 || f.h == 0 || f.x as usize + f.w as usize > self.nx as usize || f.y as usize + f.h as usize > self.ny as usize {
                    out.push(Problem {
                        level: li,
                        at: Some([f.x, f.y]),
                        text: format!("{tag}: the {} at {},{} is off the grid", f.kind, f.x, f.y),
                        blocking: true,
                    });
                }
            }
        }
        out
    }

    /// The site it builds (`id` `u:<layout>:<k>`; uploaded pictures' rules `metas`).
    fn build(&self, id: &str, settlement: u32, building: u32, metas: &std::collections::BTreeMap<String, crate::world::SpriteMeta>) -> Interior {
        let kind = self.under_kind().unwrap_or(UnderKind::Dungeon);
        let (nx, ny) = (self.nx.clamp(1, MAX_SIDE) as usize, self.ny.clamp(1, MAX_SIDE) as usize);
        let n = self.levels.len();
        let mut sprites = Sprites::new(metas);
        let levels = self
            .levels
            .iter()
            .enumerate()
            .map(|(li, lv)| {
                let p = self.plan(lv, kind, nx, ny, &mut sprites);
                let mut level = p.finish(nx, ny, -((n - li) as i8), lv.name.clone(), lv.elevation_ft);
                level.landing = lv.landing.filter(|l| (l[0] as usize) < nx && (l[1] as usize) < ny);
                level
            })
            .collect();
        Interior {
            id: id.into(),
            settlement,
            building,
            name: None,
            function: kind.name(),
            theme: self.theme.as_deref().and_then(|t| THEMES.iter().find(|x| x.key == t)).map(|t| t.key),
            stone: 0,
            look: self.look,
            origin: self.origin,
            axis: self.axis,
            across: [-self.axis[1], self.axis[0]],
            nx,
            ny,
            levels,
            entry_level: n.saturating_sub(1),
            stairs: [0; 4],
            sprites: sprites.table(),
            outlines: Vec::new(),
        }
    }

    /// Everything wrong with it (shape first; then the rules of `check`, a building's of
    /// `interior::design::check`), with what it builds.
    fn problems(&self, world: &World, t0: &T0, id: &str) -> Result<(Interior, Vec<Problem>), String> {
        if self.is_building() {
            return crate::interior::design::problems(world, t0, id, self);
        }
        let mut out = self.shape_problems();
        let it = self.build(id, 0, 0, &world.file.edits.sprites);
        out.extend(check(&it, Some(self.entry)));
        out.extend(self.unfitted(&it));
        Ok((it, out))
    }

    /// Doors wherever a room can't be reached otherwise: from the way onto each level, the
    /// middle of the wall to the nearest room cut off, until every room is reached (or nothing
    /// cut off touches anything reached).
    fn add_doors(&mut self, level: Option<usize>) {
        let (nx, ny) = (self.nx as usize, self.ny as usize);
        let building = self.is_building();
        let stairs = building.then(|| crate::interior::design::stairs_of(self)).map(|s| s[1] * nx + s[0]);
        for (li, lv) in self.levels.iter_mut().enumerate() {
            if level.is_some_and(|l| l != li) || lv.natural {
                continue;
            }
            let Some(cells) = decode(&lv.cells, nx * ny) else { continue };
            let items: Vec<(String, usize)> = lv.items.iter().map(|f| (f.kind.clone(), f.y as usize * nx + f.x as usize)).collect();
            // (A building's floors are reached from the stairs, else the spiral stairs.)
            let ways: &[&str] = if building { &["spiral_stair"] } else { &["exit", "up"] };
            let start = stairs
                .filter(|_| lv.has_stairs)
                .or_else(|| items.iter().find(|(k, _)| ways.contains(&k.as_str())).map(|e| e.1))
                .or_else(|| (0..nx * ny).find(|&k| cells[k] >= 0));
            let Some(start) = start.filter(|&s| s < nx * ny && cells[s] >= 0) else { continue };
            let mut doors: Vec<(usize, usize)> = lv.doors.iter().filter_map(|d| door_squares(d[0], d[1], d[2], nx, ny)).collect();
            loop {
                let seen = flood(&cells, nx, ny, start, false, &doors, &|_| false);
                // Walls between a reached room and one cut off, by pair of rooms.
                let mut pairs: std::collections::BTreeMap<(i16, i16), Vec<(usize, usize)>> = Default::default();
                for k in 0..nx * ny {
                    if !seen[k] {
                        continue;
                    }
                    for m in neighbours(k, nx, ny) {
                        if cells[m] >= 0 && !seen[m] && cells[m] != cells[k] {
                            pairs.entry((cells[k], cells[m])).or_default().push((k.min(m), k.max(m)));
                        }
                    }
                }
                let Some((_, edges)) = pairs.into_iter().next() else { break };
                let mut edges = edges;
                edges.sort();
                edges.dedup();
                doors.push(edges[edges.len() / 2]);
            }
            let known: Vec<(usize, usize)> = lv.doors.iter().filter_map(|d| door_squares(d[0], d[1], d[2], nx, ny)).collect();
            for &(a, b) in doors.iter().filter(|d| !known.contains(d)) {
                lv.doors.push([(a % nx) as u16, (a / nx) as u16, (b == a + nx) as u16, 0]);
            }
        }
    }

    /// Room `room` of level `level` of site `id` furnished from its kit (a boss chamber's set
    /// piece; a building's, `interior::design::furnish`) where that fits round what is already
    /// there; `seed` picks the arrangement.
    fn furnish(&mut self, world: &World, t0: &T0, id: &str, level: usize, room: usize, seed: u64) {
        if self.is_building() {
            return crate::interior::design::furnish(self, world, t0, id, level, room, seed);
        }
        let (nx, ny) = (self.nx as usize, self.ny as usize);
        let kind = self.under_kind().unwrap_or(UnderKind::Dungeon);
        let Some(lv) = self.levels.get(level) else { return };
        let metas = Default::default();
        let mut p = self.plan(lv, kind, nx, ny, &mut Sprites::new(&metas));
        p.theme = if kind == UnderKind::KeepDungeon { KEEP_DUNGEON.key } else { self.theme.as_deref().and_then(|t| THEMES.iter().find(|x| x.key == t)).map_or("", |t| t.key) };
        let Some(rkind) = p.rooms.get(room).map(|r| r.kind) else { return };
        p.reserve_doors();
        let had = p.items.len();
        let mut rng = Pcg32::new(hash3(seed, level as i64, room as i64, 7), 91);
        furnish_room(&mut p, nx, ny, room as i16, rkind, kind, &mut rng);
        let lv = &mut self.levels[level];
        for f in &p.items[had..] {
            lv.items.push(DesignItem {
                kind: f.kind.into(),
                x: f.x,
                y: f.y,
                w: f.w,
                h: f.h,
                ..Default::default()
            });
        }
    }

    /// Rooms given a shape that `it` (what the design builds) draws square: their squares don't
    /// fill it (or a building's cavern). Noted, not refused.
    fn unfitted(&self, it: &Interior) -> Vec<Problem> {
        let building = self.is_building();
        let n = self.levels.len();
        let mut out = Vec::new();
        for (li, (lv, built)) in self.levels.iter().zip(&it.levels).enumerate() {
            for (ri, r) in lv.rooms.iter().enumerate() {
                // (A cavern on natural rock: nothing to break into.)
                let natural_cavern = !building && lv.natural && r.shape == crate::interior::shape::CAVERN;
                let Some(room) = built.rooms.get(ri).filter(|b| b.squares > 0 && r.shape != 0 && (b.shape != r.shape || natural_cavern)) else { continue };
                let k = built.cells.iter().position(|&c| c == ri as i16).unwrap_or(0);
                let (x, y) = (k % it.nx, k / it.nx);
                let tag = if building { lv.name.clone() } else { depth_tag(n, li) };
                let shape = crate::interior::shape::name(r.shape).unwrap_or("shaped");
                if natural_cavern {
                    let text = format!("{tag}: the {} at {x},{y} is natural rock already: a cavern breaking in shows only on a built level", room.kind);
                    out.push(Problem { level: li, at: Some([x as u16, y as u16]), text, blocking: false });
                    continue;
                }
                let why = if building && r.shape == crate::interior::shape::CAVERN { "a building's rooms can't be caverns".to_string() } else { format!("its squares don't fill its shape ({shape}); to fit them, choose the shape again, or give the room just the squares whose middles lie inside the shape") };
                out.push(Problem { level: li, at: Some([x as u16, y as u16]), text: format!("{tag}: the {} at {x},{y} is drawn square: {why}", room.kind), blocking: false });
            }
        }
        out
    }

    /// `set_shape`, held to the rules (`problems`, as site or building `id` of `world`): refused,
    /// with nothing changed, when the change breaks one more than the design already did (each
    /// kind of problem counted, whatever its numbers: a stretch already cut off may change size).
    fn set_shape_checked(&mut self, world: &World, t0: &T0, id: &str, level: usize, room: usize, shape: u8) -> Result<(), String> {
        let blocking = |d: &SiteDesign| -> Result<Vec<(String, String)>, String> {
            Ok(d.problems(world, t0, id)?.1.into_iter().filter(|p| p.blocking).map(|p| (p.text.chars().filter(|c| !c.is_ascii_digit()).collect(), p.text)).collect())
        };
        let had = blocking(self)?;
        let mut next = self.clone();
        next.set_shape(level, room, shape)?;
        let now = blocking(&next)?;
        let count = |v: &[(String, String)], k: &str| v.iter().filter(|p| p.0 == k).count();
        if let Some(p) = now.iter().find(|p| count(&now, &p.0) > count(&had, &p.0)) {
            let kind = &self.levels[level].rooms[room].kind;
            return Err(format!("the {kind} can't take that shape: {}", p.1));
        }
        *self = next;
        Ok(())
    }

    /// Room `room` of level `level` given `shape` (`interior::shape`), its squares made the
    /// shape's in the box round them: squares outside it go back to rock (a building's to the
    /// room beside them); rock inside it joins the room (a building's only within its walls: where
    /// another level has floor). Other rooms keep theirs. Refused, with nothing changed, when the
    /// squares can't be made the shape's (another room or the walls in the way).
    fn set_shape(&mut self, level: usize, room: usize, shape: u8) -> Result<(), String> {
        use crate::interior::shape;
        let (nx, ny) = (self.nx as usize, self.ny as usize);
        let building = self.is_building();
        if building && shape == shape::CAVERN {
            return Err("a building's rooms can't be caverns".into());
        }
        if shape != shape::SQUARE && shape::name(shape).is_none() {
            return Err(format!("no such shape: {shape}"));
        }
        // (A building's squares within its walls: floor on some level.)
        let walls: Vec<bool> = if building {
            let mut w = vec![false; nx * ny];
            for lv in &self.levels {
                for (k, c) in decode(&lv.cells, nx * ny).unwrap_or_default().iter().enumerate() {
                    w[k] |= *c >= 0;
                }
            }
            w
        } else {
            Vec::new()
        };
        let lv = self.levels.get_mut(level).ok_or_else(|| format!("no level {level}"))?;
        let kind = lv.rooms.get(room).ok_or_else(|| format!("no room {room} on that level"))?.kind.clone();
        let mut cells = decode(&lv.cells, nx * ny).ok_or("the level's squares don't add up")?;
        let before = cells.clone();
        // Squares given away: underground, a doorway goes to the room across its door (the way in
        // reaching on to the shape), the rest to rock; a building's to a room beside it with no
        // shape of its own (one with a shape would no longer fit it).
        let shaped: Vec<bool> = lv.rooms.iter().map(|r| shape::geometric(r.shape)).collect();
        let skip = |r: i16| shaped.get(r as usize).copied().unwrap_or(false);
        let spare = |c: &[i16], k: usize| if building { shape::neighbour_room(c, nx, ny, k, room as i16, &skip) } else { -1 };
        shape::fit(&mut cells, nx, room as i16, shape, &spare, &|k| !building || walls[k]);
        let name = shape::name(shape).unwrap_or("shaped");
        let made = match shape {
            shape::OCTAGON => "octagonal".to_string(),
            shape::CAVERN => "a cavern".to_string(),
            _ if name.starts_with("apse") => "to end in an apse".to_string(),
            _ => name.to_string(),
        };
        if !shape::fits(&cells, nx, room as i16, shape) {
            let why = if building { "the walls or another room are in the way" } else { "another room is in the way" };
            return Err(format!("the {kind} can't be made {made}: {why}"));
        }
        // Underground, a way in through a corner gone to rock is carried on to the shape: the
        // room beyond the door takes the squares between (the fewest), the door moves to its end.
        let mut moved: Vec<[u16; 4]> = Vec::new();
        let mut doorways: Vec<usize> = Vec::new();
        if !building {
            let me = room as i16;
            for d in &lv.doors {
                let Some((a, b)) = door_squares(d[0], d[1], d[2], nx, ny) else { continue };
                let (k, m) = if before[a] == me { (a, b) } else if before[b] == me { (b, a) } else { continue };
                let other = cells[m];
                if cells[k] >= 0 || other < 0 || other == me || skip(other) {
                    continue;
                }
                // Through the room's old squares now rock, to one beside the shape.
                let mut from = vec![usize::MAX; nx * ny];
                from[k] = k;
                let mut queue = std::collections::VecDeque::from([k]);
                let mut end = None;
                while let Some(q) = queue.pop_front() {
                    if let Some(u) = neighbours(q, nx, ny).find(|&u| cells[u] == me) {
                        end = Some((q, u));
                        break;
                    }
                    for u in neighbours(q, nx, ny) {
                        if from[u] == usize::MAX && before[u] == me && cells[u] < 0 {
                            from[u] = q;
                            queue.push_back(u);
                        }
                    }
                }
                let Some((t, u)) = end else { continue };
                let mut q = t;
                loop {
                    cells[q] = other;
                    if q == k {
                        break;
                    }
                    q = from[q];
                }
                doorways.push(u);
                let (p, r) = (t.min(u), t.max(u));
                let door = [(p % nx) as u16, (p / nx) as u16, (r == p + nx) as u16, d[3]];
                if !moved.iter().chain(&lv.doors).any(|o| o[..3] == door[..3]) {
                    moved.push(door);
                }
            }
        }
        // The ways stay on floor; props left in the rock, in a doorway moved, or across two rooms
        // now, go.
        let rock = |k: usize| (cells[k] < 0 && before[k] >= 0) || doorways.contains(&k);
        let on = |f: &DesignItem| {
            let squares: Vec<usize> = (f.y as usize..(f.y + f.h) as usize).flat_map(|j| (f.x as usize..(f.x + f.w) as usize).map(move |i| (i, j))).filter(|&(i, j)| i < nx && j < ny).map(|(i, j)| j * nx + i).collect();
            let touched = squares.iter().any(|&k| cells[k] != before[k] || doorways.contains(&k));
            touched && (squares.iter().any(|&k| rock(k)) || squares.iter().any(|&k| cells[k] != cells[squares[0]]))
        };
        if let Some(f) = lv.items.iter().find(|f| WAYS.contains(&f.kind.as_str()) && on(f)) {
            return Err(format!("the {kind} can't be made {made}: the way {} at {},{} would be in the rock or a doorway", f.kind, f.x, f.y));
        }
        lv.items.retain(|f| !on(f));
        lv.rooms[room].shape = shape;
        lv.cells = encode(&cells);
        lv.doors.extend(moved);
        // Doors that no longer lie between two rooms go.
        lv.doors.retain(|d| {
            if !building {
                return door_squares(d[0], d[1], d[2], nx, ny).is_some_and(|(a, b)| cells[a] >= 0 && cells[b] >= 0 && cells[a] != cells[b]);
            }
            // (A building's door to the outside stays one, given from its room's square.)
            crate::interior::design::door_at(d[0], d[1], d[2], nx, ny).is_some_and(|(a, b, _, _)| {
                let other = b.map_or(-1, |b| cells[b]);
                cells[a] >= 0 && other != cells[a] && (other >= 0 || d[3] & 6 != 0)
            })
        });
        Ok(())
    }
}

impl SiteDesignPlan for SiteDesign {
    /// A level as the generators lay one out (rooms, doors, items), for `Plan::finish`.
    fn plan(&self, lv: &DesignLevel, kind: UnderKind, nx: usize, ny: usize, sprites: &mut Sprites) -> Plan {
        let mut p = Plan::new(nx, ny, 0, lv.natural);
        for r in &lv.rooms {
            let ri = p.room(room_kind(&r.kind).unwrap_or(CHAMBER), r.raise_ft as f32);
            p.rooms[ri as usize].shape = r.shape;
        }
        let rooms = p.rooms.len() as i16;
        p.cells = decode(&lv.cells, nx * ny).unwrap_or_else(|| vec![-1; nx * ny]);
        for c in p.cells.iter_mut() {
            if *c >= rooms {
                *c = -1;
            }
        }
        for &[x, y, side, flags] in &lv.doors {
            let Some((a, b)) = door_squares(x, y, side, nx, ny) else { continue };
            let (ra, rb) = (p.cells[a], p.cells[b]);
            if ra < 0 || rb < 0 || ra == rb {
                continue;
            }
            p.door(a, b);
            if flags & 1 != 0 {
                p.secret.insert((a, b));
            } else if flags & 8 != 0 {
                p.gaps.insert((a.min(b), a.max(b)));
            } else if flags & PORTCULLIS != 0 {
                p.portcullis.insert((a, b));
            }
            if lock_of(flags) != 0 {
                p.locks.insert((a, b), lock_of(flags));
            }
        }
        let (down_name, up_name) = kind.ways();
        for f in &lv.items {
            let (x, y, w, h) = (f.x as usize, f.y as usize, f.w.max(1) as usize, f.h.max(1) as usize);
            if x + w > nx || y + h > ny {
                continue;
            }
            if let Some(a) = sprite_asset(&f.kind) {
                p.put(f.posed(sprites.item(a, x, y, w, h)));
                continue;
            }
            let Some(k) = item_kind(&f.kind) else { continue };
            let it = match k {
                "exit" => Item::new("exit", kind.entrance_name(), x, y, 1, 1, 0, false, 0.0, None),
                "up" => Item::new("up", up_name, x, y, 1, 1, 0, false, 0.0, None),
                "down" => Item::new("down", down_name, x, y, 1, 1, 0, false, 0.0, None),
                "lava" => lava(x, y),
                _ => item(k, x, y, w, h),
            };
            p.put(f.posed(it));
        }
        // The way onto the level, for furnishing's reachability.
        p.up = arrival(&p.items, nx).unwrap_or(0);
        p
    }
}

/// "level 2" (counted from the top, as players go down).
fn depth_tag(n: usize, li: usize) -> String {
    format!("level {}", n - li)
}

/// The two squares a door lies between.
fn door_squares(x: u16, y: u16, side: u16, nx: usize, ny: usize) -> Option<(usize, usize)> {
    let (x, y) = (x as usize, y as usize);
    if x >= nx || y >= ny {
        return None;
    }
    let a = y * nx + x;
    match side {
        0 if x + 1 < nx => Some((a, a + 1)),
        1 if y + 1 < ny => Some((a, a + nx)),
        _ => None,
    }
}

/// Where one arrives on a level: the way in from the surface, else the way up.
fn arrival(items: &[Item], nx: usize) -> Option<usize> {
    items
        .iter()
        .find(|f| f.kind == "exit")
        .or_else(|| items.iter().find(|f| f.kind == "up"))
        .map(|f| f.y as usize * nx + f.x as usize)
}

/// Room per square, run-length encoded.
pub fn encode(cells: &[i16]) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::new();
    for &c in cells {
        match out.len() {
            n if n >= 2 && out[n - 2] == c as i32 => out[n - 1] += 1,
            _ => out.extend([c as i32, 1]),
        }
    }
    out
}

/// Room per square from runs; None unless they hold exactly `n` squares.
pub fn decode(runs: &[i32], n: usize) -> Option<Vec<i16>> {
    if !runs.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for r in runs.chunks(2) {
        let (c, k) = (r[0], r[1]);
        if !(-1..=i16::MAX as i32).contains(&c) || k < 0 || out.len() + k as usize > n {
            return None;
        }
        out.extend(std::iter::repeat_n(c as i16, k as usize));
    }
    (out.len() == n).then_some(out)
}

/// The site `id` names, if it can be designed: (layout, entrance).
pub fn editable(world: &World, t0: &T0, id: &str) -> Result<(usize, usize), String> {
    let mut parts = id.split(':');
    let (Some("u"), Some(l), Some(k), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(format!("{id}: only underground sites (u:<layout>:<k>) can be designed"));
    };
    let (Ok(l), Ok(k)) = (l.parse::<usize>(), k.parse::<usize>()) else {
        return Err(format!("{id}: not a site id"));
    };
    if l >= town::layout_count(t0) {
        return Err(format!("no such site: {id}"));
    }
    let layout = town::layout(world, t0, l);
    match layout.entrance(k) {
        None => Err(format!("no such site: {id}")),
        Some(e) if e.kind == UnderKind::Sewer => Err(format!("{id}: a city's sewers can't be designed")),
        Some(_) => Ok((l, k)),
    }
}

/// Whether design `d` was made for site `l`/`k` as the world has it: its grid holds the site's
/// entrance (2 squares' margin). A key that came to name another site (edits kept for another
/// sketch or generator) would build the design far from that site's way in: it is set aside.
pub fn fits(world: &World, t0: &T0, l: usize, k: usize, d: &SiteDesign) -> bool {
    let layout = town::layout(world, t0, l);
    let Some(e) = layout.entrance(k) else { return false };
    let (dx, dy) = (e.at[0] - d.origin[0], e.at[1] - d.origin[1]);
    let (gx, gy) = ((dx * d.axis[0] + dy * d.axis[1]) / SQUARE_FT, (dy * d.axis[0] - dx * d.axis[1]) / SQUARE_FT);
    let m = 2.0;
    gx >= -m && gy >= -m && gx <= d.nx as f64 + m && gy <= d.ny as f64 + m
}

/// The design saved for site `id` was made for a site that stood elsewhere (`fits`): the
/// generated site stands in its place.
pub fn misplaced(world: &World, t0: &T0, id: &str) -> bool {
    let Some(d) = world.file.edits.designs.get(id) else { return false };
    editable(world, t0, id).is_ok_and(|(l, k)| !fits(world, t0, l, k, d))
}

/// The designed site `id`, if the world still has its entrance (the generated one when the
/// design was made for a site that stood elsewhere).
pub fn site(world: &World, t0: &T0, id: &str, d: &SiteDesign) -> Option<Interior> {
    let (l, k) = editable(world, t0, id).ok()?;
    if !fits(world, t0, l, k, d) {
        return super::generate(world, t0, l, k);
    }
    let mut it = d.build(id, l as u32, k as u32, &world.file.edits.sprites);
    crate::sitelinks::dress(world, t0, l, &mut it);
    Some(it)
}

/// Site `id` as a design: the one saved (if made for this site, `fits`), else (or with
/// `original`) a copy of what the generator makes.
pub fn design_of(world: &World, t0: &T0, id: &str, original: bool) -> Result<SiteDesign, String> {
    if id.starts_with("b:") {
        return crate::interior::design::design_of(world, t0, id, original).map(|d| d.0);
    }
    let (l, k) = editable(world, t0, id)?;
    match world.file.edits.designs.get(id) {
        Some(d) if !original && fits(world, t0, l, k, d) => Ok(d.clone()),
        _ => super::generate(world, t0, l, k)
            .map(|it| SiteDesign::from_interior(&it))
            .ok_or_else(|| format!("no such site: {id}")),
    }
}

/// For the editor (WASM): the design given (else the site's own, `design_of`), changed by
/// `action` (`{"doors": level|null}`, `{"furnish": {level, room, seed}}`, `{"original": true}`,
/// `{"shape": {level, room, shape}}`: a room's shape by name or code, its squares fitted to it;
/// a building's `{"refit": true}`: fitted to its storeys as they are now, `{"refit": n}`: to n
/// storeys, `{"cellars": n}`: n levels below ground), with the site it
/// builds and its problems: `{design, interior, problems}` (a saved design that no longer fits
/// the building, or was made for a site that stood elsewhere: `set_aside`) or `{error}`.
pub fn design_json(world: &World, t0: &T0, id: &str, design: Option<&str>, action: Option<&str>) -> String {
    let run = || -> Result<Value, String> {
        let building = id.starts_with("b:");
        let (l, k) = if building { (0, 0) } else { editable(world, t0, id)? };
        let action: Value = action.map(serde_json::from_str).transpose().map_err(|e| format!("bad action: {e}"))?.unwrap_or(Value::Null);
        let original = action["original"].as_bool().unwrap_or(false);
        let mut set_aside = false;
        let mut d = match design.filter(|_| !original) {
            Some(s) => serde_json::from_str::<SiteDesign>(s).map_err(|e| format!("bad design: {e}"))?,
            None if building => {
                let (d, aside) = crate::interior::design::design_of(world, t0, id, original)?;
                set_aside = aside;
                d
            }
            None => {
                set_aside = !original && misplaced(world, t0, id);
                design_of(world, t0, id, original)?
            }
        };
        // (Fitted ahead of a change of storeys: checked as the building will be.)
        let mut floors = None;
        if building && (action["refit"].as_bool() == Some(true) || action["refit"].is_u64()) {
            floors = action["refit"].as_u64().map(|f| f.min(12) as u8);
            d = crate::interior::design::refit(world, t0, id, &d, floors)?.ok_or_else(|| format!("{id}: its footprint changed: the design no longer fits"))?;
        }
        if building && let Some(n) = action["cellars"].as_u64() {
            let most = crate::interior::design::MAX_CELLARS;
            if !crate::interior::design::set_cellars(&mut d, n as usize) {
                return Err(format!("a building has 0 to {most} levels below ground"));
            }
        }
        if action.get("doors").is_some() {
            d.add_doors(action["doors"].as_u64().map(|l| l as usize));
        }
        if let Some(f) = action.get("shape") {
            let shape = match &f["shape"] {
                Value::String(n) => crate::interior::shape::parse(n).ok_or_else(|| format!("no such shape: {n}"))?,
                v => v.as_u64().filter(|&c| c <= 255).ok_or("a shape is a name or its code")? as u8,
            };
            d.set_shape_checked(world, t0, id, f["level"].as_u64().unwrap_or(0) as usize, f["room"].as_u64().unwrap_or(0) as usize, shape)?;
        }
        if let Some(f) = action.get("furnish") {
            d.furnish(
                world,
                t0,
                id,
                f["level"].as_u64().unwrap_or(0) as usize,
                f["room"].as_u64().unwrap_or(0) as usize,
                f["seed"].as_u64().unwrap_or(0),
            );
        }
        let (mut it, problems) = if building { crate::interior::design::problems_as(world, t0, id, &d, floors)? } else { d.problems(world, t0, id)? };
        if !building {
            (it.settlement, it.building) = (l as u32, k as u32);
        }
        let mut out = json!({ "design": d, "interior": it, "problems": problems });
        if set_aside {
            out["set_aside"] = json!(true);
        }
        Ok(out)
    };
    run().unwrap_or_else(|e| json!({ "error": e })).to_string()
}
/// `SiteDesign::settle_ways`, here above the model crate (`model::under`).
pub trait SiteDesignWays {
    fn settle_ways(&mut self);
}

impl SiteDesignWays for SiteDesign {
    /// Fill in ways that can go only one place: the way in at the entry (top level), a way up
    /// under the way down from the level above; none down from the deepest level.
    fn settle_ways(&mut self) {
        let n = self.levels.len();
        if let Some(deep) = self.levels.first_mut() {
            deep.items.retain(|f| f.kind != "down");
        }
        let entry = self.entry;
        if let Some(top) = self.levels.last_mut()
            && !top.items.iter().any(|f| f.kind == "exit")
        {
            top.items.insert(
                0,
                DesignItem {
                    kind: "exit".into(),
                    x: entry[0],
                    y: entry[1],
                    w: 1,
                    h: 1,
                    ..Default::default()
                },
            );
        }
        for li in 0..n.saturating_sub(1) {
            let mut downs: Vec<[u16; 2]> = self.levels[li + 1].items.iter().filter(|f| f.kind == "down").map(|f| [f.x, f.y]).collect();
            let lv = &mut self.levels[li];
            if let Some(l) = lv.landing {
                downs = vec![l];
            }
            if let [k] = downs.as_slice()
                && !lv.items.iter().any(|f| f.kind == "up")
            {
                lv.items.insert(
                    0,
                    DesignItem {
                        kind: "up".into(),
                        x: k[0],
                        y: k[1],
                        w: 1,
                        h: 1,
                        ..Default::default()
                    },
                );
            }
        }
    }
}
