//! Underground sites designed by hand: a copy of a generated site (`u:` ids, sewers aside) that
//! the user or an agent changed: rooms painted or dug, doors, ways between levels, props, levels
//! added or taken away. `Edits::designs` keeps them by site id, and `interior::generate_id`
//! builds a designed site instead of generating it. A design holds only squares, rooms, doors
//! and items; walls are derived as for a generated site (`Plan::finish`).
//!
//! `check` holds a site to the rules every generated one meets (tests/vital.rs): one way in,
//! under the entrance; each level's way down on the same square as the way up below; every
//! square reached from the way onto the level. The editor and `set_site_design` refuse a design
//! that breaks them (play mode relies on them).
//!
//! The text form (`to_text`/`from_text`) is the plan `examples/under.rs` prints, made two-way:
//! one character per square, rooms by symbol, then doors and items as lists.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{BOSS, KEEP_DUNGEON, LEVEL_FT, MAX_LEVELS, PROPS, Plan, THEMES, UNDERCROFT, UnderKind, furnish_room, item, kit, lava, neighbours};
use crate::World;
use crate::core::rng::{Pcg32, hash3};
use crate::interior::{Interior, Item};
use crate::t0::T0;
use crate::town;

/// A designed site.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SiteDesign {
    /// What it is (`UnderKind::key`): dungeon, crypt, catacombs, cave, mine, lava_tube.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// World ft of grid corner (0, 0) and the grid's x axis (as `Interior`).
    pub origin: [f64; 2],
    pub axis: [f64; 2],
    pub nx: u16,
    pub ny: u16,
    /// The way in on the top level: the square under the entrance on the surface.
    pub entry: [u16; 2],
    /// Levels bottom to top (as `Interior::levels`, so `l:<site>:<level>` ids hold).
    pub levels: Vec<DesignLevel>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DesignLevel {
    pub name: String,
    pub elevation_ft: f32,
    /// Natural rock (caves, mines, lava tubes): rooms open into each other, no doors needed.
    pub natural: bool,
    /// Room per square (row-major), run-length encoded: room (-1 rock), count, room, count…
    pub cells: Vec<i32>,
    /// Rooms keep their index (`r:<site>:<level>:<room>` names hold); one with no squares is
    /// simply not there.
    pub rooms: Vec<DesignRoom>,
    /// Doors: x, y, side (0: between the square and the one east of it, 1: south), secret (0/1).
    pub doors: Vec<[u16; 4]>,
    /// Everything standing on the floor, the ways in, up and down too (kinds `exit`, `up`,
    /// `down`), in order.
    pub items: Vec<DesignItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DesignRoom {
    pub kind: String,
    /// Raised floor (a ledge), ft.
    pub raise_ft: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DesignItem {
    pub kind: String,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
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
    "corridor",
    "cavern",
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

/// What can be designed (for the editor's menus): props with their rules and usual size,
/// room kinds by theme (each theme's own first), the themes.
pub fn catalog_json() -> String {
    let rooms = room_kinds();
    // A prop's usual size: as the first room kit that holds it has it.
    let size = |kind: &str| rooms.iter().flat_map(|r| kit(r).iter()).find(|e| e.0 == kind).map(|e| (e.1, e.2)).unwrap_or((1, 1));
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
    json!({ "props": props, "rooms": rooms, "themes": themes, "boss": BOSS, "max_levels": MAX_LEVELS }).to_string()
}

impl SiteDesign {
    pub fn under_kind(&self) -> Option<UnderKind> {
        UnderKind::parse(&self.kind)
    }

    /// A copy of a site as it is: the design that builds it again, exactly.
    pub fn from_interior(it: &Interior) -> SiteDesign {
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
                        let secret = (d.kind == "secret") as u16;
                        let (x, y) = (d.a[0].min(d.b[0]) as u16, d.a[1].min(d.b[1]) as u16);
                        // A horizontal edge lies between the squares above and below it.
                        if d.a[1] == d.b[1] { [x, y - 1, 1, secret] } else { [x - 1, y, 0, secret] }
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
                        })
                        .collect(),
                    doors,
                    items: lv
                        .furniture
                        .iter()
                        .map(|f| DesignItem {
                            kind: f.kind.into(),
                            x: f.x,
                            y: f.y,
                            w: f.w,
                            h: f.h,
                        })
                        .collect(),
                }
            })
            .collect();
        SiteDesign {
            kind: kind.key().into(),
            theme: it.theme.map(str::to_string),
            origin: it.origin,
            axis: it.axis,
            nx: it.nx as u16,
            ny: it.ny as u16,
            entry,
            levels,
        }
    }

    /// What can't be read at all (sizes, kinds, cells): a design with such a problem is built
    /// as well as it can be, but never saved.
    pub fn shape_problems(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        let bad = |level: usize, text: String| Problem {
            level,
            at: None,
            text,
            blocking: true,
        };
        if self.under_kind().is_none() {
            out.push(bad(0, format!("{} sites can't be designed", self.kind)));
        }
        if self.nx == 0 || self.ny == 0 || self.nx > MAX_SIDE || self.ny > MAX_SIDE {
            out.push(bad(0, format!("the grid must be 1 to {MAX_SIDE} squares a side")));
            return out;
        }
        if self.levels.is_empty() || self.levels.len() > MAX_LEVELS as usize {
            out.push(bad(0, format!("a site has 1 to {MAX_LEVELS} levels")));
        }
        let n = self.nx as usize * self.ny as usize;
        for (li, lv) in self.levels.iter().enumerate() {
            let tag = depth_tag(self.levels.len(), li);
            match decode(&lv.cells, n) {
                None => out.push(bad(li, format!("{tag}: its squares don't add up to {} x {}", self.nx, self.ny))),
                Some(c) if c.iter().any(|&r| r >= lv.rooms.len() as i16) => out.push(bad(li, format!("{tag}: a square of a room that isn't in its rooms list"))),
                _ => {}
            }
            if lv.rooms.len() > MAX_ROOMS || lv.items.len() > MAX_ITEMS || lv.doors.len() > MAX_ITEMS {
                out.push(bad(li, format!("{tag}: at most {MAX_ROOMS} rooms and {MAX_ITEMS} items and doors")));
            }
            for f in &lv.items {
                if item_kind(&f.kind).is_none() {
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

    /// The site it builds (`id` `u:<layout>:<k>`).
    pub fn build(&self, id: &str, settlement: u32, building: u32) -> Interior {
        let kind = self.under_kind().unwrap_or(UnderKind::Dungeon);
        let (nx, ny) = (self.nx.clamp(1, MAX_SIDE) as usize, self.ny.clamp(1, MAX_SIDE) as usize);
        let n = self.levels.len();
        let levels = self
            .levels
            .iter()
            .enumerate()
            .map(|(li, lv)| {
                let p = self.plan(lv, kind, nx, ny);
                p.finish(nx, ny, -((n - li) as i8), lv.name.clone(), lv.elevation_ft)
            })
            .collect();
        Interior {
            id: id.into(),
            settlement,
            building,
            name: None,
            function: kind.name(),
            theme: self.theme.as_deref().and_then(|t| THEMES.iter().find(|x| x.key == t)).map(|t| t.key),
            origin: self.origin,
            axis: self.axis,
            across: [-self.axis[1], self.axis[0]],
            nx,
            ny,
            levels,
            entry_level: n.saturating_sub(1),
            stairs: [0; 4],
        }
    }

    /// A level as the generators lay one out (rooms, doors, items), for `Plan::finish`.
    fn plan(&self, lv: &DesignLevel, kind: UnderKind, nx: usize, ny: usize) -> Plan {
        let mut p = Plan::new(nx, ny, 0, lv.natural);
        for r in &lv.rooms {
            p.room(room_kind(&r.kind).unwrap_or(CHAMBER), r.raise_ft as f32);
        }
        let rooms = p.rooms.len() as i16;
        p.cells = decode(&lv.cells, nx * ny).unwrap_or_else(|| vec![-1; nx * ny]);
        for c in p.cells.iter_mut() {
            if *c >= rooms {
                *c = -1;
            }
        }
        for &[x, y, side, secret] in &lv.doors {
            let Some((a, b)) = door_squares(x, y, side, nx, ny) else { continue };
            let (ra, rb) = (p.cells[a], p.cells[b]);
            if ra < 0 || rb < 0 || ra == rb {
                continue;
            }
            p.door(a, b);
            if secret != 0 {
                p.secret.insert((a, b));
            }
        }
        let (down_name, up_name) = kind.ways();
        for f in &lv.items {
            let Some(k) = item_kind(&f.kind) else { continue };
            let (x, y, w, h) = (f.x as usize, f.y as usize, f.w.max(1) as usize, f.h.max(1) as usize);
            if x + w > nx || y + h > ny {
                continue;
            }
            let it = match k {
                "exit" => Item::new("exit", kind.entrance_name(), x, y, 1, 1, 0, false, 0.0, None),
                "up" => Item::new("up", up_name, x, y, 1, 1, 0, false, 0.0, None),
                "down" => Item::new("down", down_name, x, y, 1, 1, 0, false, 0.0, None),
                "lava" => lava(x, y),
                _ => item(k, x, y, w, h),
            };
            p.put(it);
        }
        // The way onto the level, for furnishing's reachability.
        p.up = arrival(&p.items, nx).unwrap_or(0);
        p
    }

    /// Everything wrong with it (shape first; then the rules of `check`).
    pub fn problems(&self, id: &str) -> (Interior, Vec<Problem>) {
        let mut out = self.shape_problems();
        let it = self.build(id, 0, 0);
        out.extend(check(&it, Some(self.entry)));
        (it, out)
    }

    /// Doors wherever a room can't be reached otherwise: from the way onto each level, the
    /// middle of the wall to the nearest room cut off, until every room is reached (or nothing
    /// cut off touches anything reached).
    pub fn add_doors(&mut self, level: Option<usize>) {
        let (nx, ny) = (self.nx as usize, self.ny as usize);
        for (li, lv) in self.levels.iter_mut().enumerate() {
            if level.is_some_and(|l| l != li) || lv.natural {
                continue;
            }
            let Some(cells) = decode(&lv.cells, nx * ny) else { continue };
            let items: Vec<(String, usize)> = lv.items.iter().map(|f| (f.kind.clone(), f.y as usize * nx + f.x as usize)).collect();
            let start = items
                .iter()
                .find(|(k, _)| k == "exit" || k == "up")
                .map(|e| e.1)
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

    /// Room `room` of level `level` furnished from its kit (a boss chamber's set piece) where
    /// that fits round what is already there; `seed` picks the arrangement.
    pub fn furnish(&mut self, level: usize, room: usize, seed: u64) {
        let (nx, ny) = (self.nx as usize, self.ny as usize);
        let kind = self.under_kind().unwrap_or(UnderKind::Dungeon);
        let Some(lv) = self.levels.get(level) else { return };
        let mut p = self.plan(lv, kind, nx, ny);
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
            });
        }
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

/// Squares reached from `start` over floor: between rooms only through `doors` (any step on a
/// natural level), never onto `blocked` ones.
fn flood(cells: &[i16], nx: usize, ny: usize, start: usize, natural: bool, doors: &[(usize, usize)], blocked: &dyn Fn(usize) -> bool) -> Vec<bool> {
    let mut seen = vec![false; nx * ny];
    if cells[start] < 0 {
        return seen;
    }
    let door: crate::core::hash::FastSet<(usize, usize)> = doors.iter().copied().collect();
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(k) = stack.pop() {
        for q in neighbours(k, nx, ny) {
            if seen[q] || cells[q] < 0 || blocked(q) {
                continue;
            }
            if !natural && cells[q] != cells[k] && !door.contains(&(k.min(q), k.max(q))) {
                continue;
            }
            seen[q] = true;
            stack.push(q);
        }
    }
    seen
}

/// The rules every site keeps (generated ones are held to them in tests/vital.rs): the top
/// level's one way in (under the entrance, `entry`, if given); one way down from each level
/// but the deepest, on the square of the one way up on the level below; every item on floor,
/// none on another; every square reached from the way onto its level, around whatever blocks
/// movement. A site without one boss chamber on its deepest level is noted, not refused.
pub fn check(it: &Interior, entry: Option<[u16; 2]>) -> Vec<Problem> {
    let (nx, ny) = (it.nx, it.ny);
    let n = it.levels.len();
    let mut out = Vec::new();
    let mut put = |level: usize, at: Option<usize>, text: String, blocking: bool| {
        out.push(Problem {
            level,
            at: at.map(|k| [(k % nx) as u16, (k / nx) as u16]),
            text: format!("{}: {text}", depth_tag(n, level)),
            blocking,
        });
    };
    if n == 0 {
        put(0, None, "no levels".into(), true);
        return out;
    }
    let at = |li: usize, kind: &str| {
        it.levels[li]
            .furniture
            .iter()
            .filter(|f| f.kind == kind)
            .map(|f| f.y as usize * nx + f.x as usize)
            .collect::<Vec<_>>()
    };
    for li in 0..n {
        let lv = &it.levels[li];
        let top = li + 1 == n;
        // The ways.
        let (exit, up, down) = (at(li, "exit"), at(li, "up"), at(li, "down"));
        if top {
            match exit.as_slice() {
                [] => put(li, None, "no way in from the surface".into(), true),
                [k] => {
                    if let Some(e) = entry
                        && [(k % nx) as u16, (k / nx) as u16] != e
                    {
                        put(li, Some(*k), format!("the way in must stay under the entrance, at {},{}", e[0], e[1]), true);
                    }
                }
                _ => put(li, Some(exit[1]), format!("{} ways in from the surface (one only)", exit.len()), true),
            }
            if !up.is_empty() {
                put(li, Some(up[0]), "a way up on the top level".into(), true);
            }
        } else {
            if !exit.is_empty() {
                put(li, Some(exit[0]), "a way in from the surface below the top level".into(), true);
            }
            let above = at(li + 1, "down");
            match up.as_slice() {
                [] => put(li, above.first().copied(), "no way up".into(), true),
                [k] if above.len() == 1 && above[0] != *k => put(li, Some(*k), "the way up is not under the way down from the level above".into(), true),
                [_] => {}
                _ => put(li, Some(up[1]), format!("{} ways up (one only)", up.len()), true),
            }
        }
        if li == 0 {
            if !down.is_empty() {
                put(li, Some(down[0]), "a way down from the deepest level".into(), true);
            }
        } else if down.is_empty() {
            put(li, None, "no way down".into(), true);
        } else if down.len() > 1 {
            put(li, Some(down[1]), format!("{} ways down (one only)", down.len()), true);
        }
        // Items on floor, none on another.
        let mut held = vec![false; nx * ny];
        let mut blocks = vec![false; nx * ny];
        let mut clash = None;
        for f in &lv.furniture {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    if i >= nx || j >= ny {
                        continue;
                    }
                    let k = j * nx + i;
                    if lv.cells[k] < 0 {
                        put(li, Some(k), format!("the {} at {i},{j} is in the rock", f.name), true);
                    }
                    if held[k] && clash.is_none() {
                        clash = Some((k, f.name));
                    }
                    held[k] = true;
                    blocks[k] |= f.blocks_move;
                }
            }
        }
        if let Some((k, name)) = clash {
            put(li, Some(k), format!("the {name} at {},{} stands on something else", k % nx, k / nx), true);
        }
        // Everything reached from the way onto the level.
        let Some(start) = (if top { exit.first() } else { up.first() }).copied() else {
            continue;
        };
        if lv.cells[start] < 0 || blocks[start] {
            put(li, Some(start), "the way onto the level is not open floor".into(), true);
            continue;
        }
        let doors: Vec<(usize, usize)> = lv
            .doors
            .iter()
            .map(|d| {
                let (x, y) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize);
                if d.a[1] == d.b[1] {
                    (y.saturating_sub(1) * nx + x, y * nx + x)
                } else {
                    (y * nx + x.saturating_sub(1), y * nx + x)
                }
            })
            .collect();
        let seen = flood(&lv.cells, nx, ny, start, lv.natural, &doors, &|q| blocks[q]);
        // Each stretch cut off, once.
        let mut lost = vec![false; nx * ny];
        let mut cut = 0;
        for k in 0..nx * ny {
            if lv.cells[k] < 0 || blocks[k] || seen[k] || lost[k] {
                continue;
            }
            let part = flood(&lv.cells, nx, ny, k, lv.natural, &doors, &|q| blocks[q]);
            let size = part.iter().filter(|&&s| s).count();
            for (q, s) in part.iter().enumerate() {
                lost[q] |= *s;
            }
            cut += 1;
            if cut <= 8 {
                let room = it.levels[li].rooms[lv.cells[k] as usize].kind;
                put(
                    li,
                    Some(k),
                    format!("{size} square{} of the {room} at {},{} can't be reached", if size == 1 { "" } else { "s" }, k % nx, k / nx),
                    true,
                );
            }
        }
        if cut > 8 {
            put(li, None, format!("{} more stretches can't be reached", cut - 8), true);
        }
        if lv.cells.iter().all(|&c| c < 0) {
            put(li, None, "no floor".into(), true);
        }
    }
    let boss: Vec<usize> = it
        .levels
        .iter()
        .enumerate()
        .flat_map(|(li, lv)| lv.rooms.iter().filter(|r| r.kind == BOSS && r.squares > 0).map(move |_| li))
        .collect();
    match boss.as_slice() {
        [] => put(0, None, "no boss chamber".into(), false),
        [0] => {}
        [li] => put(*li, None, "the boss chamber is not on the deepest level".into(), false),
        _ => put(boss[1], None, format!("{} boss chambers", boss.len()), false),
    }
    out
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

/// The designed site `id`, if the world still has its entrance.
pub fn site(world: &World, t0: &T0, id: &str, d: &SiteDesign) -> Option<Interior> {
    let (l, k) = editable(world, t0, id).ok()?;
    Some(d.build(id, l as u32, k as u32))
}

/// Site `id` as a design: the one saved, else (or with `original`) a copy of what the
/// generator makes.
pub fn design_of(world: &World, t0: &T0, id: &str, original: bool) -> Result<SiteDesign, String> {
    let (l, k) = editable(world, t0, id)?;
    match world.file.edits.designs.get(id) {
        Some(d) if !original => Ok(d.clone()),
        _ => super::generate(world, t0, l, k)
            .map(|it| SiteDesign::from_interior(&it))
            .ok_or_else(|| format!("no such site: {id}")),
    }
}

/// For the editor (WASM): the design given (else the site's own, `design_of`), changed by
/// `action` (`{"doors": level|null}`, `{"furnish": {level, room, seed}}`, `{"original": true}`),
/// with the site it builds and its problems: `{design, interior, problems}` or `{error}`.
pub fn design_json(world: &World, t0: &T0, id: &str, design: Option<&str>, action: Option<&str>) -> String {
    let run = || -> Result<Value, String> {
        let (l, k) = editable(world, t0, id)?;
        let action: Value = action.map(serde_json::from_str).transpose().map_err(|e| format!("bad action: {e}"))?.unwrap_or(Value::Null);
        let original = action["original"].as_bool().unwrap_or(false);
        let mut d = match design.filter(|_| !original) {
            Some(s) => serde_json::from_str::<SiteDesign>(s).map_err(|e| format!("bad design: {e}"))?,
            None => design_of(world, t0, id, original)?,
        };
        if action.get("doors").is_some() {
            d.add_doors(action["doors"].as_u64().map(|l| l as usize));
        }
        if let Some(f) = action.get("furnish") {
            d.furnish(
                f["level"].as_u64().unwrap_or(0) as usize,
                f["room"].as_u64().unwrap_or(0) as usize,
                f["seed"].as_u64().unwrap_or(0),
            );
        }
        let (mut it, problems) = d.problems(id);
        (it.settlement, it.building) = (l as u32, k as u32);
        Ok(json!({ "design": d, "interior": it, "problems": problems }))
    };
    run().unwrap_or_else(|e| json!({ "error": e })).to_string()
}

// ---------------------------------------------------------------------------------------
// The text form.

/// Room symbols, in room order: room `i` is `SYMBOLS[i]`.
pub const SYMBOLS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#$%&+?@<>~^*!";

/// The design as text, levels from the top. `name(level, room)`: a room's name, if renamed.
pub fn to_text(d: &SiteDesign, name: &dyn Fn(usize, usize) -> Option<String>) -> Result<String, String> {
    let (nx, ny) = (d.nx as usize, d.ny as usize);
    let n = d.levels.len();
    let mut s = format!(
        "site {}{} · {} x {} squares · {} level{} · entry {},{}\n",
        d.kind,
        d.theme.as_deref().map(|t| format!(" · theme {t}")).unwrap_or_default(),
        nx,
        ny,
        n,
        if n == 1 { "" } else { "s" },
        d.entry[0],
        d.entry[1]
    );
    for (li, lv) in d.levels.iter().enumerate().rev() {
        let cells = decode(&lv.cells, nx * ny).ok_or_else(|| format!("{}: bad squares", depth_tag(n, li)))?;
        let used: Vec<bool> = (0..lv.rooms.len()).map(|r| cells.contains(&(r as i16))).collect();
        if lv.rooms.len() > SYMBOLS.len() && used.iter().skip(SYMBOLS.len()).any(|&u| u) {
            return Err(format!("{} has more than {} rooms: too many to show as text", depth_tag(n, li), SYMBOLS.len()));
        }
        s.push_str(&format!("\nlevel {}: {}\n", n - li, lv.name));
        let legend: Vec<String> = lv
            .rooms
            .iter()
            .enumerate()
            .filter(|(r, _)| used[*r])
            .map(|(r, room)| {
                let raise = if room.raise_ft > 0 { format!("+{}", room.raise_ft) } else { String::new() };
                let named = name(li, r).map(|t| format!(" \"{t}\"")).unwrap_or_default();
                format!("{}={}{raise}{named}", SYMBOLS[r] as char, room.kind)
            })
            .collect();
        s.push_str(&format!("rooms: {}\ngrid:\n", legend.join("; ")));
        for j in 0..ny {
            let row: String = (0..nx)
                .map(|i| if cells[j * nx + i] < 0 { '.' } else { SYMBOLS[cells[j * nx + i] as usize] as char })
                .collect();
            s.push_str(&row);
            s.push('\n');
        }
        let doors: Vec<String> = lv
            .doors
            .iter()
            .map(|d| format!("{},{} {}{}", d[0], d[1], if d[2] == 0 { "e" } else { "s" }, if d[3] != 0 { " secret" } else { "" }))
            .collect();
        if !lv.natural || !doors.is_empty() {
            s.push_str(&format!("doors: {}\n", doors.join("; ")));
        }
        let items: Vec<String> = lv
            .items
            .iter()
            .map(|f| format!("{} {},{}{}", f.kind, f.x, f.y, if f.w > 1 || f.h > 1 { format!(" {}x{}", f.w, f.h) } else { String::new() }))
            .collect();
        s.push_str(&format!("items: {}\n", items.join("; ")));
    }
    Ok(s)
}

/// A room name given in the text: (level index, room, name).
pub type TextName = (usize, usize, String);

/// A design from text over `base` (what the text leaves out stays as it is): levels by their
/// number from the top, each with any of `rooms:`, `grid:`, `doors:` (`auto`: doors where
/// needed) and `items:`. A header `site … · N levels` sets how many levels there are (new ones
/// start as solid rock). Missing ways are filled in where they can only go one place: the way
/// in at the entry, a way up under the way down from the level above.
pub fn from_text(text: &str, base: &SiteDesign) -> Result<(SiteDesign, Vec<TextName>), String> {
    let mut d = base.clone();
    let (nx, ny) = (d.nx as usize, d.ny as usize);
    let mut names: Vec<TextName> = Vec::new();
    // Blocks by level number, in order.
    struct Block<'a> {
        depth: usize,
        name: Option<&'a str>,
        rooms: Option<&'a str>,
        grid: Option<Vec<&'a str>>,
        doors: Option<&'a str>,
        items: Option<&'a str>,
    }
    let mut blocks: Vec<Block> = Vec::new();
    let mut levels: Option<usize> = None;
    let mut in_grid = false;
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        let t = line.trim_start();
        let keyword = |k: &str| t.strip_prefix(k).map(str::trim);
        if t.is_empty() || t.starts_with("//") {
            in_grid = false;
            continue;
        }
        if let Some(h) = keyword("site ") {
            // "… · 3 levels · …"
            for part in h.split('·') {
                let p = part.trim();
                if let Some(v) = p.strip_suffix(" levels").or_else(|| p.strip_suffix(" level")) {
                    levels = Some(v.trim().parse().map_err(|_| format!("line {}: '{p}': how many levels?", ln + 1))?);
                }
            }
            in_grid = false;
            continue;
        }
        if let Some(h) = keyword("level ") {
            let (num, rest) = h.split_once(':').map(|(a, b)| (a.trim(), Some(b.trim()))).unwrap_or((h, None));
            let depth: usize = num.parse().map_err(|_| format!("line {}: 'level {num}': levels are numbered from 1 at the top", ln + 1))?;
            if depth == 0 || depth > MAX_LEVELS as usize {
                return Err(format!("line {}: levels are 1 to {MAX_LEVELS}", ln + 1));
            }
            blocks.push(Block {
                depth,
                name: rest.filter(|r| !r.is_empty()),
                rooms: None,
                grid: None,
                doors: None,
                items: None,
            });
            in_grid = false;
            continue;
        }
        let b = blocks.last_mut().ok_or_else(|| format!("line {}: start with 'level <n>' (1 is the top)", ln + 1))?;
        if let Some(v) = keyword("rooms:") {
            b.rooms = Some(v);
            in_grid = false;
        } else if let Some(v) = keyword("doors:") {
            b.doors = Some(v);
            in_grid = false;
        } else if let Some(v) = keyword("items:") {
            b.items = Some(v);
            in_grid = false;
        } else if keyword("grid:").is_some() {
            b.grid = Some(Vec::new());
            in_grid = true;
        } else if in_grid {
            b.grid.as_mut().expect("grid open").push(t);
        } else {
            return Err(format!("line {}: expected rooms:, grid:, doors: or items:, not '{t}'", ln + 1));
        }
    }
    let count = levels.unwrap_or_else(|| d.levels.len().max(blocks.iter().map(|b| b.depth).max().unwrap_or(0)));
    if count == 0 || count > MAX_LEVELS as usize {
        return Err(format!("a site has 1 to {MAX_LEVELS} levels"));
    }
    if let Some(b) = blocks.iter().find(|b| b.depth > count) {
        return Err(format!("level {} given, but the site has {count} levels", b.depth));
    }
    // Levels added below or taken away from the bottom (index 0 is the deepest).
    let n0 = d.levels.len();
    if count < n0 {
        d.levels.drain(0..n0 - count);
    }
    while d.levels.len() < count {
        let deep = d.levels.first().cloned().unwrap_or_default();
        let depth = d.levels.len() + 1;
        d.levels.insert(
            0,
            DesignLevel {
                name: format!("Level {depth} · {} ft down", depth * LEVEL_FT as usize),
                elevation_ft: deep.elevation_ft - LEVEL_FT,
                natural: deep.natural,
                cells: vec![-1, (nx * ny) as i32],
                ..Default::default()
            },
        );
        if !blocks.iter().any(|b| b.depth == depth && b.grid.is_some()) {
            return Err(format!("level {depth} is new: give its grid"));
        }
    }
    for b in &blocks {
        let li = count - b.depth;
        let lv = &mut d.levels[li];
        let tag = format!("level {}", b.depth);
        if let Some(nm) = b.name {
            lv.name = nm.to_string();
        }
        if let Some(legend) = b.rooms {
            let mut rooms: Vec<Option<DesignRoom>> = Vec::new();
            for entry in legend.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                let (sym, rest) = entry.split_once('=').ok_or_else(|| format!("{tag}: room '{entry}': write it as a=kind"))?;
                let sym = sym.trim();
                let r = (sym.len() == 1).then(|| SYMBOLS.iter().position(|&c| c == sym.as_bytes()[0])).flatten().ok_or_else(|| {
                    format!(
                        "{tag}: '{sym}' is not a room symbol (a-z, A-Z, 0-9, then {})",
                        std::str::from_utf8(&SYMBOLS[62..]).unwrap_or("")
                    )
                })?;
                let (body, named) = match rest.split_once('"') {
                    Some((k, n)) => (k.trim(), Some(n.trim_end_matches('"').trim().to_string())),
                    None => (rest.trim(), None),
                };
                let (kind, raise) = match body.rsplit_once('+') {
                    Some((k, v)) if v.trim().parse::<u16>().is_ok() => (k.trim(), v.trim().parse::<u16>().unwrap_or(0)),
                    _ => (body, 0),
                };
                let known = room_kind(kind);
                if rooms.len() <= r {
                    rooms.resize(r + 1, None);
                }
                rooms[r] = Some(DesignRoom {
                    kind: known.unwrap_or(CHAMBER).into(),
                    raise_ft: raise.min(30),
                });
                // A kind of room the generators don't know becomes a chamber by that name.
                if let Some(nm) = named.filter(|n| !n.is_empty()).or_else(|| known.is_none().then(|| kind.to_string())) {
                    names.push((li, r, nm));
                }
            }
            let old = std::mem::take(&mut lv.rooms);
            lv.rooms = rooms
                .into_iter()
                .enumerate()
                .map(|(r, x)| {
                    x.or_else(|| old.get(r).cloned()).unwrap_or(DesignRoom {
                        kind: CHAMBER.into(),
                        raise_ft: 0,
                    })
                })
                .collect();
        }
        if let Some(rows) = &b.grid {
            if rows.len() > ny {
                return Err(format!("{tag}: {} rows; the grid has {ny}", rows.len()));
            }
            let mut cells = vec![-1i16; nx * ny];
            for (j, row) in rows.iter().enumerate() {
                if row.chars().count() > nx {
                    return Err(format!("{tag}: row {j} has {} squares; the grid has {nx}", row.chars().count()));
                }
                for (i, c) in row.chars().enumerate() {
                    if c == '.' || c == ' ' {
                        continue;
                    }
                    let r = SYMBOLS
                        .iter()
                        .position(|&s| s as char == c)
                        .filter(|&r| r < lv.rooms.len())
                        .ok_or_else(|| format!("{tag}: '{c}' at {i},{j} is not in its rooms list"))?;
                    cells[j * nx + i] = r as i16;
                }
            }
            lv.cells = encode(&cells);
        }
        if let Some(list) = b.doors {
            lv.doors.clear();
            if list.trim() != "auto" {
                for e in list.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                    let mut w = e.split_whitespace();
                    let (xy, side, secret) = (w.next().unwrap_or(""), w.next().unwrap_or(""), w.next());
                    let (x, y) = coords(xy).ok_or_else(|| format!("{tag}: door '{e}': write it as x,y e|s [secret]"))?;
                    let side = match side {
                        "e" => 0,
                        "s" => 1,
                        _ => return Err(format!("{tag}: door '{e}': the side is e (to the square east) or s (south)")),
                    };
                    if door_squares(x, y, side, nx, ny).is_none() {
                        return Err(format!("{tag}: door '{e}' is off the grid"));
                    }
                    lv.doors.push([x, y, side, matches!(secret, Some("secret")) as u16]);
                }
            }
        }
        if let Some(list) = b.items {
            lv.items.clear();
            for e in list.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                let words: Vec<&str> = e.split_whitespace().collect();
                let size = words
                    .last()
                    .and_then(|s| s.split_once('x'))
                    .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)));
                let at = words.len() - 1 - size.is_some() as usize;
                let (Some(&xy), true) = (words.get(at), at > 0) else {
                    return Err(format!("{tag}: item '{e}': write it as kind x,y [wxh]"));
                };
                let (x, y) = coords(xy).ok_or_else(|| format!("{tag}: item '{e}': write it as kind x,y [wxh]"))?;
                let name = words[..at].join(" ");
                let kind = item_kind(&name).ok_or_else(|| format!("{tag}: no such item: {name}"))?;
                let (w, h) = if WAYS.contains(&kind) || kind == "lava" { (1, 1) } else { size.unwrap_or((1, 1)) };
                lv.items.push(DesignItem {
                    kind: kind.into(),
                    x,
                    y,
                    w: w.max(1),
                    h: h.max(1),
                });
            }
        }
    }
    d.settle_ways();
    for b in blocks.iter().filter(|b| b.doors.is_some_and(|l| l.trim() == "auto")) {
        d.add_doors(Some(count - b.depth));
    }
    Ok((d, names))
}

fn coords(s: &str) -> Option<(u16, u16)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

impl SiteDesign {
    /// Fill in ways that can go only one place: the way in at the entry (top level), a way up
    /// under the way down from the level above; none down from the deepest level.
    pub fn settle_ways(&mut self) {
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
                },
            );
        }
        for li in 0..n.saturating_sub(1) {
            let downs: Vec<[u16; 2]> = self.levels[li + 1].items.iter().filter(|f| f.kind == "down").map(|f| [f.x, f.y]).collect();
            let lv = &mut self.levels[li];
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
                    },
                );
            }
        }
    }
}
