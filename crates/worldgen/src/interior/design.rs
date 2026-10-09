//! Building interiors designed by hand: a copy of a building's interior (`b:` ids, generated or
//! drawn buildings) that the user or an agent changed: rooms split or merged, doors (to the
//! outside too), the stair block moved, furniture. `Edits::designs` keeps it under the building's
//! id as a `SiteDesign` of kind `building` (`under::design`), and `generate` builds it in place
//! of the generated interior while it still fits the building (its `fingerprint`: the grid, the
//! footprint and the levels) and keeps the rules (`check`); else the generated interior stands.
//!
//! What a design holds: squares, rooms, doors and items per level, the stair block. Walls and
//! windows are worked out as for a generated interior; the cellar's ways to other sites (a
//! trapdoor to the sewers, stairs down to a keep's deep dungeons) are put back at build time,
//! never stored.

use serde_json::Value;

use super::{Door, FURNITURE, Interior, Item, Level, Room, STOREY_FT, Shell, Sprites, battlements, building_item, cellar_links, design_kind, finish_rooms, furnish_rooms, room_kind, sprite_asset, wall_runs, windows};
use crate::World;
use crate::core::hash::fnv64;
use crate::core::rng::{Pcg32, hash3};
use crate::t0::T0;
use crate::town::{self, Layout, Structure};
use crate::under::design::{DesignItem, DesignLevel, DesignRoom, Problem, SiteDesign, decode, encode};

/// `SiteDesign::kind` of a building's design.
pub const BUILDING: &str = "building";
/// The room kind anything unknown becomes.
pub const CHAMBER: &str = "chamber";
/// Door flags (`DesignLevel::doors[3]`).
pub const SECRET: u16 = 1;
pub const FRONT: u16 = 2;
pub const BACK: u16 = 4;
/// Items that are ways to other sites, put back at build time (never in a design).
const LINKS: [&str; 2] = ["trapdoor", "link_down"];

/// What a building's design can put down, by key or name ("spiral stairs"): furniture or an
/// indoor prop (an uploaded picture, `s:<asset id>`, is `sprite_asset`'s).
pub fn furniture_kind(s: &str) -> Option<&'static str> {
    building_item(s, 0, 0, 1, 1).map(|f| f.kind)
}

/// The sprites' rules (`Edits::sprites`) when there are none.
fn no_sprites() -> &'static std::collections::BTreeMap<String, crate::world::SpriteMeta> {
    static EMPTY: std::sync::OnceLock<std::collections::BTreeMap<String, crate::world::SpriteMeta>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(Default::default)
}

/// What a design is fitted to: the building's grid (frame, size, the squares inside its
/// footprint) and its levels (storeys, an open roof). A design whose fingerprint is not the
/// building's any more is set aside, not built.
pub fn fingerprint(sh: &Shell, b: &town::Building) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let mut put = |v: i64| bytes.extend_from_slice(&v.to_le_bytes());
    put(sh.nx as i64);
    put(sh.ny as i64);
    for c in [sh.origin[0], sh.origin[1]] {
        put(crate::core::round(c * 10.0) as i64);
    }
    for c in [sh.u[0], sh.u[1]] {
        put(crate::core::round(c * 1e4) as i64);
    }
    put(b.floors as i64);
    put(battlements(b).map_or(-1, |t| crate::core::round(t) as i64));
    for k in sh.inside.chunks(64) {
        put(k.iter().enumerate().fold(0i64, |a, (i, &x)| a | ((x as i64) << i)));
    }
    format!("{:016x}", fnv64(&bytes))
}

/// The two squares a door lies between: (x, y) and the one on its `side` (0 east, 1 south,
/// 2 west, 3 north), None when that one is off the grid. Its edge (grid units, the end with the
/// lower coordinates first).
pub fn door_at(x: u16, y: u16, side: u16, nx: usize, ny: usize) -> Option<(usize, Option<usize>, [f32; 2], [f32; 2])> {
    let (i, j) = (x as usize, y as usize);
    if i >= nx || j >= ny || side > 3 {
        return None;
    }
    let (fi, fj) = (i as f32, j as f32);
    let a = j * nx + i;
    Some(match side {
        0 => (a, (i + 1 < nx).then(|| a + 1), [fi + 1.0, fj], [fi + 1.0, fj + 1.0]),
        1 => (a, (j + 1 < ny).then(|| a + nx), [fi, fj + 1.0], [fi + 1.0, fj + 1.0]),
        2 => (a, (i > 0).then(|| a - 1), [fi, fj], [fi, fj + 1.0]),
        _ => (a, (j > 0).then(|| a - nx), [fi, fj], [fi + 1.0, fj]),
    })
}

/// A building's interior as a design: the design that builds it again, exactly.
pub fn from_interior(it: &Interior, fingerprint: String) -> SiteDesign {
    let nx = it.nx;
    let levels = it
        .levels
        .iter()
        .map(|lv| {
            let cell = |i: f32, j: f32| if i < 0.0 || j < 0.0 || i as usize >= nx || j as usize >= it.ny { -1 } else { lv.cells[j as usize * nx + i as usize] };
            let doors = lv
                .doors
                .iter()
                .map(|d| {
                    let flags = match d.kind {
                        "secret" => SECRET,
                        "front" => FRONT,
                        "back" => BACK,
                        _ => 0,
                    };
                    let (x, y) = (d.a[0].min(d.b[0]), d.a[1].min(d.b[1]));
                    // A vertical edge on line x lies between squares x-1 and x; a horizontal one on
                    // line y between y-1 and y. A door to the outside is given from its room's square.
                    if d.a[0] == d.b[0] {
                        if cell(x - 1.0, y) >= 0 { [x as u16 - 1, y as u16, 0, flags] } else { [x as u16, y as u16, 2, flags] }
                    } else if cell(x, y - 1.0) >= 0 {
                        [x as u16, y as u16 - 1, 1, flags]
                    } else {
                        [x as u16, y as u16, 3, flags]
                    }
                })
                .collect();
            let link = |f: &Item| LINKS.contains(&f.kind) && lv.links.iter().any(|k| k.x == f.x && k.y == f.y);
            DesignLevel {
                name: lv.name.clone(),
                elevation_ft: lv.elevation_ft,
                natural: false,
                cells: encode(&lv.cells),
                rooms: lv.rooms.iter().map(|r| DesignRoom { kind: r.kind.into(), raise_ft: r.raise_ft.max(0.0) as u16 }).collect(),
                doors,
                items: lv.furniture.iter().filter(|f| !link(f)).map(|f| DesignItem { kind: design_kind(it, f), x: f.x, y: f.y, w: f.w, h: f.h }).collect(),
                z: lv.z,
                roof: lv.roof,
                has_stairs: lv.has_stairs,
            }
        })
        .collect();
    let s = it.stairs;
    SiteDesign {
        kind: BUILDING.into(),
        theme: None,
        origin: it.origin,
        axis: it.axis,
        nx: it.nx as u16,
        ny: it.ny as u16,
        entry: [0, 0],
        levels,
        stairs: Some([s[0] as u16, s[1] as u16, s[2] as u16, s[3] as u16]),
        fingerprint,
    }
}

/// The stair block (x, y, w, h), inside the grid.
pub fn stairs_of(d: &SiteDesign) -> [usize; 4] {
    let (nx, ny) = (d.nx.max(1) as usize, d.ny.max(1) as usize);
    let [x, y, w, h] = d.stairs.unwrap_or([0, 0, 1, 1]).map(|v| v as usize);
    let (x, y) = (x.min(nx - 1), y.min(ny - 1));
    [x, y, w.clamp(1, nx - x), h.clamp(1, ny - y)]
}

/// Level `li` as the design has it: squares (none outside `inside`, if given), rooms, doors,
/// walls, windows (not on an open roof) and items (pictures through `sprites`); no ways to
/// other sites yet.
pub fn level_of(d: &SiteDesign, li: usize, inside: Option<&[bool]>, sprites: &mut Sprites) -> Level {
    let (nx, ny) = (d.nx as usize, d.ny as usize);
    let lv = &d.levels[li];
    let mut cells = decode(&lv.cells, nx * ny).unwrap_or_else(|| vec![-1; nx * ny]);
    for (k, c) in cells.iter_mut().enumerate() {
        if *c >= lv.rooms.len() as i16 || inside.is_some_and(|m| !m[k]) {
            *c = -1;
        }
    }
    let mut rooms: Vec<Room> = lv
        .rooms
        .iter()
        .map(|r| Room { kind: room_kind(&r.kind).unwrap_or(CHAMBER), squares: 0, raise_ft: r.raise_ft as f32, center: [0.0; 2] })
        .collect();
    finish_rooms(&mut rooms, &cells, nx);
    let mut doors: Vec<Door> = Vec::new();
    for &[x, y, side, flags] in &lv.doors {
        let Some((a, b, p, q)) = door_at(x, y, side, nx, ny) else { continue };
        let (ra, rb) = (cells[a], b.map_or(-1, |b| cells[b]));
        if ra < 0 || ra == rb || doors.iter().any(|o| o.a == p && o.b == q) {
            continue;
        }
        doors.push(if rb >= 0 {
            Door { a: p, b: q, kind: if flags & SECRET != 0 { "secret" } else { "door" }, rooms: [ra.min(rb), ra.max(rb)] }
        } else {
            Door { a: p, b: q, kind: if flags & FRONT != 0 { "front" } else { "back" }, rooms: [ra, -1] }
        });
    }
    let walls = wall_runs(&cells, nx, ny, &doors);
    let furniture = lv
        .items
        .iter()
        .filter_map(|f| {
            let (x, y, w, h) = (f.x as usize, f.y as usize, f.w.max(1) as usize, f.h.max(1) as usize);
            if x + w > nx || y + h > ny {
                return None;
            }
            match sprite_asset(&f.kind) {
                Some(a) => Some(sprites.item(a, x, y, w, h)),
                None => building_item(&f.kind, x, y, w, h),
            }
        })
        .collect();
    let mut level = Level {
        z: lv.z,
        name: lv.name.clone(),
        elevation_ft: lv.elevation_ft,
        cells,
        rooms,
        walls,
        doors,
        windows: Vec::new(),
        furniture,
        roof: lv.roof,
        has_stairs: lv.has_stairs,
        natural: false,
        paths: Vec::new(),
        links: Vec::new(),
    };
    if !lv.roof {
        windows(&mut level, nx, ny, lv.z == 0);
    }
    level
}

/// The deepest level below ground (index), if the design has one.
pub fn deepest_cellar(d: &SiteDesign) -> Option<usize> {
    (0..d.levels.len()).filter(|&li| d.levels[li].z < 0).min_by_key(|&li| d.levels[li].z)
}

/// The interior a design builds for building `b` of layout `l` (its grid `sh`; uploaded
/// pictures' rules `metas`), with the ways to other sites its deepest cellar should have.
pub fn build(d: &SiteDesign, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, sh: &Shell, metas: &std::collections::BTreeMap<String, crate::world::SpriteMeta>) -> (Interior, Vec<&'static str>) {
    let stairs = stairs_of(d);
    let deepest = deepest_cellar(d);
    let mut want = Vec::new();
    let mut sprites = Sprites::new(metas);
    let levels: Vec<Level> = (0..d.levels.len())
        .map(|li| {
            let mut lvl = level_of(d, li, Some(&sh.inside), &mut sprites);
            lvl.elevation_ft = b.pad_ft + STOREY_FT * lvl.z as f32;
            if Some(li) == deepest {
                want = cellar_links(Some(&mut lvl), t0, l, b, sh, stairs);
            }
            lvl
        })
        .collect();
    if deepest.is_none() {
        want = cellar_links(None, t0, l, b, sh, stairs);
    }
    let it = Interior {
        id: format!("b:{}:{}", l.index, b.id),
        settlement: settlement as u32,
        building: b.id,
        name: b.name.clone(),
        function: b.label(),
        theme: None,
        origin: sh.origin,
        axis: sh.u,
        across: sh.v,
        nx: sh.nx,
        ny: sh.ny,
        entry_level: levels.iter().position(|lv| lv.z == 0).unwrap_or(0),
        levels,
        stairs,
        sprites: sprites.table(),
    };
    (it, want)
}

/// The squares either side of a door's edge (some maybe off the grid).
fn door_squares(d: &Door) -> [(isize, isize); 2] {
    let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
    if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
}

/// The rules every building interior keeps (generated ones are held to them in tests/vital.rs):
/// one ground floor with one front door; doors to the outside only there, onto open ground; the
/// stair block on floor on every level it reaches; squares only inside the footprint; no item
/// off the floor, on another, on the stairs or in a doorway; every room and door reached from
/// the stairs (above them, from the spiral stairs; a ground floor without either, from its doors
/// to the outside) around whatever blocks movement; the ways to other sites (`want`) in place on
/// the deepest cellar (a building with such a way has a cellar).
pub fn check(it: &Interior, sh: &Shell, want: &[&str]) -> Vec<Problem> {
    let (nx, ny) = (it.nx, it.ny);
    let mut out = Vec::new();
    let mut put = |level: usize, at: Option<(usize, usize)>, text: String, blocking: bool| {
        let name = it.levels.get(level).map_or("", |l| l.name.as_str());
        out.push(Problem { level, at: at.map(|(i, j)| [i as u16, j as u16]), text: format!("{name}: {text}"), blocking });
    };
    let deepest = (0..it.levels.len()).filter(|&li| it.levels[li].z < 0).min_by_key(|&li| it.levels[li].z);
    let grounds = it.levels.iter().filter(|l| l.z == 0).count();
    if grounds != 1 {
        put(0, None, format!("{grounds} ground floors (one only)"), true);
    }
    let st = it.stairs;
    let on_stairs = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
    if st[2] > 3 || st[3] > 3 || st[0] + st[2] > nx || st[1] + st[3] > ny {
        put(0, Some((st[0], st[1])), "the stairs are 1 to 3 squares each way, on the grid".into(), true);
        return out;
    }
    for (li, lv) in it.levels.iter().enumerate() {
        let inb = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny;
        if let Some(k) = (0..nx * ny).find(|&k| lv.cells[k] >= 0 && !sh.inside[k]) {
            put(li, Some((k % nx, k / nx)), "a square outside the walls".into(), true);
        }
        if lv.has_stairs
            && let Some((i, j)) = (st[1]..st[1] + st[3]).flat_map(|j| (st[0]..st[0] + st[2]).map(move |i| (i, j))).find(|&(i, j)| lv.cells[j * nx + i] < 0)
        {
            put(li, Some((i, j)), "the stairs are not on the floor".into(), true);
        }
        // Doors to the outside: on the ground floor only, onto open ground; one front door.
        let fronts = lv.doors.iter().filter(|d| d.kind == "front").count();
        if lv.z == 0 && fronts != 1 {
            put(li, None, if fronts == 0 { "no front door".into() } else { format!("{fronts} front doors (one only)") }, true);
        }
        for d in lv.doors.iter().filter(|d| d.rooms[1] < 0) {
            let [p, q] = door_squares(d);
            let out_sq = if inb(p) && lv.cells[p.1 as usize * nx + p.0 as usize] >= 0 { q } else { p };
            let inn = if out_sq == p { q } else { p };
            let at = Some((inn.0.max(0) as usize, inn.1.max(0) as usize));
            if lv.z != 0 {
                put(li, at, "a door to the outside above or below the ground floor".into(), true);
            } else if !sh.ext_free(out_sq.0, out_sq.1) {
                put(li, at, format!("the {} door opens into the building next door", d.kind), true);
            }
        }
        // Items on the floor, apart, off the stairs and doorways.
        let mut held = vec![false; nx * ny];
        let mut blocked = vec![false; nx * ny];
        let doorway: Vec<(isize, isize)> = lv.doors.iter().flat_map(door_squares).collect();
        let mut bad: Option<((usize, usize), String)> = None;
        for f in &lv.furniture {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    if i >= nx || j >= ny {
                        continue;
                    }
                    let k = j * nx + i;
                    let why = if lv.cells[k] < 0 {
                        Some("is not on the floor")
                    } else if held[k] {
                        Some("stands on something else")
                    } else if lv.has_stairs && on_stairs(i, j) {
                        Some("is on the stairs")
                    } else if doorway.contains(&(i as isize, j as isize)) {
                        Some("is in a doorway")
                    } else {
                        None
                    };
                    if let Some(w) = why
                        && bad.is_none()
                    {
                        bad = Some(((i, j), format!("the {} at {i},{j} {w}", f.name)));
                    }
                    held[k] = true;
                    blocked[k] |= f.blocks_move;
                }
            }
        }
        if let Some((at, text)) = bad {
            put(li, Some(at), text, true);
        }
        // Everything reached from the stairs (else the spiral stairs, else the doors to the
        // outside), between rooms through doors.
        let mut starts: Vec<usize> = if lv.has_stairs {
            vec![st[1] * nx + st[0]]
        } else {
            lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize).collect()
        };
        if starts.is_empty() && lv.z == 0 {
            for d in lv.doors.iter().filter(|d| d.rooms[1] < 0) {
                starts.extend(door_squares(d).into_iter().filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize));
            }
        }
        let starts: Vec<usize> = starts.into_iter().filter(|&k| lv.cells[k] >= 0).collect();
        if starts.is_empty() {
            if lv.cells.iter().any(|&c| c >= 0) {
                put(li, None, "no stairs reach it".into(), true);
            }
            continue;
        }
        let door_between = |a: (usize, usize), b: (usize, usize)| {
            lv.doors.iter().any(|d| {
                let s = door_squares(d);
                s.contains(&(a.0 as isize, a.1 as isize)) && s.contains(&(b.0 as isize, b.1 as isize))
            })
        };
        let mut seen = vec![false; nx * ny];
        let mut stack = starts.clone();
        for &k in &starts {
            seen[k] = true;
        }
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i as isize + di, j as isize + dj);
                if !inb((a, b)) {
                    continue;
                }
                let q = b as usize * nx + a as usize;
                if seen[q] || lv.cells[q] < 0 || blocked[q] || (lv.cells[q] != lv.cells[k] && !door_between((i, j), (a as usize, b as usize))) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        for d in &lv.doors {
            if !door_squares(d).into_iter().filter(|&p| inb(p)).any(|(i, j)| seen[j as usize * nx + i as usize]) {
                let [p, _] = door_squares(d);
                put(li, Some((p.0.max(0) as usize, p.1.max(0) as usize)), format!("a {} can't be reached", if d.kind == "door" { "door" } else { d.kind }), true);
            }
        }
        for (ri, room) in lv.rooms.iter().enumerate() {
            let open = (0..nx * ny).find(|&k| lv.cells[k] == ri as i16 && !blocked[k]);
            if let Some(k) = open
                && !(0..nx * ny).any(|k| lv.cells[k] == ri as i16 && seen[k])
            {
                put(li, Some((k % nx, k / nx)), format!("the {} at {},{} can't be reached", room.kind, k % nx, k / nx), true);
            }
        }
        if Some(li) == deepest && lv.links.len() < want.len() {
            put(li, None, "no free floor for the way down to the sewers or dungeons".into(), true);
        }
    }
    if deepest.is_none() && !want.is_empty() {
        put(it.entry_level, None, format!("it needs a cellar, for {}", want.join(" and ")), true);
    }
    out
}

/// Building `id` (`b:<layout>:<id>`, roofed): its layout, the building and its grid.
pub fn building_of(world: &World, t0: &T0, id: &str) -> Result<(usize, std::rc::Rc<Layout>, u32), String> {
    let mut parts = id.split(':');
    let (Some("b"), Some(l), Some(k), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(format!("{id}: not a building id (b:<layout>:<id>)"));
    };
    let (Ok(li), Ok(bi)) = (l.parse::<usize>(), k.parse::<u32>()) else {
        return Err(format!("{id}: not a building id"));
    };
    if li >= town::layout_count(t0) {
        return Err(format!("no such building: {id}"));
    }
    let layout = town::layout(world, t0, li);
    match layout.building(bi as usize) {
        None => Err(format!("no such building: {id}")),
        Some(b) if b.structure != Structure::Roofed => Err(format!("{id}: a ruin or a yard has no inside to design")),
        Some(_) => Ok((li, layout, bi)),
    }
}

/// The interior a building's design builds, if it still fits the building and keeps the rules.
pub fn designed(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, d: &SiteDesign) -> Option<Interior> {
    let sh = Shell::of(l, b);
    if d.kind != BUILDING || d.fingerprint != fingerprint(&sh, b) || d.nx as usize != sh.nx || d.ny as usize != sh.ny {
        return None;
    }
    let (it, want) = build(d, t0, l, settlement, b, &sh, &world.file.edits.sprites);
    check(&it, &sh, &want).iter().all(|p| !p.blocking).then_some(it)
}

/// Building `id`'s design: the one saved (if it still fits), else (or with `original`) a copy
/// of the generated interior. Whether the saved one was set aside (no longer fits).
pub fn design_of(world: &World, t0: &T0, id: &str, original: bool) -> Result<(SiteDesign, bool), String> {
    let (li, l, bi) = building_of(world, t0, id)?;
    let b = l.building(bi as usize).expect("checked");
    let sh = Shell::of(&l, b);
    let fp = fingerprint(&sh, b);
    let saved = world.file.edits.designs.get(id);
    match saved {
        Some(d) if !original && d.fingerprint == fp => Ok((d.clone(), false)),
        _ => {
            let it = super::build(world, t0, &l, li, b);
            Ok((from_interior(&it, fp), saved.is_some() && !original))
        }
    }
}

/// A design's interior and its problems (as it would be built for building `id` now).
pub fn problems(world: &World, t0: &T0, id: &str, d: &SiteDesign) -> Result<(Interior, Vec<Problem>), String> {
    problems_as(world, t0, id, d, None)
}

/// `problems`, as the building would be with `floors` storeys (a design `refit` to them, before
/// the building is changed).
pub fn problems_as(world: &World, t0: &T0, id: &str, d: &SiteDesign, floors: Option<u8>) -> Result<(Interior, Vec<Problem>), String> {
    let (li, l, bi) = building_of(world, t0, id)?;
    let mut b = l.building(bi as usize).expect("checked").clone();
    if let Some(f) = floors {
        b.floors = f.clamp(1, 12);
    }
    let b = &b;
    let sh = Shell::of(&l, b);
    let mut out = d.shape_problems();
    if d.nx as usize != sh.nx || d.ny as usize != sh.ny || d.fingerprint != fingerprint(&sh, b) {
        out.push(Problem { level: 0, at: None, text: "the design was made for the building as it was: its footprint or storeys have changed since".into(), blocking: true });
    }
    let (it, want) = build(d, t0, &l, li, b, &sh, &world.file.edits.sprites);
    out.extend(check(&it, &sh, &want));
    Ok((it, out))
}

/// Room `room` of level `level` furnished by its kind's rules where that fits round what is
/// already there; `seed` picks the arrangement.
pub fn furnish(d: &mut SiteDesign, level: usize, room: usize, seed: u64) {
    if level >= d.levels.len() {
        return;
    }
    let (nx, ny) = (d.nx as usize, d.ny as usize);
    let mut lvl = level_of(d, level, None, &mut Sprites::new(no_sprites()));
    let had = lvl.furniture.len();
    let mut rng = Pcg32::new(hash3(seed, level as i64, room as i64, 0xf0), 77);
    furnish_rooms(&mut lvl, nx, ny, stairs_of(d), Some(room as i16), &mut rng);
    // (`furnish_rooms` keeps what was there first, in order.)
    let lv = &mut d.levels[level];
    for f in &lvl.furniture[had..] {
        lv.items.push(DesignItem { kind: f.kind.into(), x: f.x, y: f.y, w: f.w, h: f.h });
    }
}

/// The design fitted to the building as it is now (or with `floors` storeys), after its storeys
/// changed: the floors it keeps as designed, new storeys (and an open roof's levels) as
/// generated. None if its grid changed too.
pub fn refit(world: &World, t0: &T0, id: &str, d: &SiteDesign, floors: Option<u8>) -> Result<Option<SiteDesign>, String> {
    let (li, l, bi) = building_of(world, t0, id)?;
    let mut b = l.building(bi as usize).expect("checked").clone();
    if let Some(f) = floors {
        b.floors = f.clamp(1, 12);
    }
    let b = &b;
    let sh = Shell::of(&l, b);
    if d.nx as usize != sh.nx || d.ny as usize != sh.ny || d.origin != sh.origin {
        return Ok(None);
    }
    let fp = fingerprint(&sh, b);
    if d.fingerprint == fp {
        return Ok(Some(d.clone()));
    }
    let generated = from_interior(&super::build(world, t0, &l, li, b), fp.clone());
    let floors = b.floors.max(1) as i8;
    let st = stairs_of(d);
    let mut out = d.clone();
    out.fingerprint = fp;
    // Kept: the design's cellars (however many) and the storeys both have (open roofs follow the
    // building).
    out.levels = d.levels.iter().filter(|m| m.z < 0).cloned().collect();
    out.levels.extend(generated.levels.iter().filter(|g| g.z >= 0).map(|g| {
        if let Some(mine) = d.levels.iter().find(|m| m.z == g.z && !m.roof && !g.roof && g.z < floors) {
            return mine.clone();
        }
        // New: as generated, with nothing on the stairs where the design has them.
        let mut g = g.clone();
        g.items.retain(|f| !g.has_stairs || !on_block(f, st));
        g
    }));
    settle_stairs(&mut out);
    Ok(Some(out))
}

/// Item `f` stands on the stair block `st`.
fn on_block(f: &DesignItem, st: [usize; 4]) -> bool {
    let (x, y) = (f.x as usize, f.y as usize);
    !(x + f.w as usize <= st[0] || x >= st[0] + st[2] || y + f.h as usize <= st[1] || y >= st[1] + st[3])
}

/// The ground floor's stairs where they lead somewhere: down to a cellar or up to a floor with
/// stairs (a one-storey building without a cellar has none). Furniture on the block goes when
/// they come back.
fn settle_stairs(d: &mut SiteDesign) {
    let Some(g) = d.levels.iter().position(|lv| lv.z == 0) else { return };
    let want = d.levels.iter().any(|lv| lv.z < 0 || (lv.z == 1 && lv.has_stairs));
    if want && !d.levels[g].has_stairs {
        let st = stairs_of(d);
        d.levels[g].items.retain(|f| !on_block(f, st));
    }
    d.levels[g].has_stairs = want;
}

/// Levels a building can have below ground.
pub const MAX_CELLARS: usize = 3;

/// A cellar's name by its storey (-1 the first below ground).
fn cellar_name(z: i8) -> &'static str {
    match z {
        -1 => "Cellar",
        -2 => "Lower cellar",
        _ => "Deep cellar",
    }
}

/// The design with `n` levels below ground (0 to `MAX_CELLARS`): the deepest filled in, or new
/// ones dug below it, each a storeroom under the whole footprint (the ground floor's squares)
/// that the stairs reach. The ways to the sewers or a keep's deep dungeons go on the deepest.
/// False (nothing changed) if `n` is out of range or there is no ground floor.
pub fn set_cellars(d: &mut SiteDesign, n: usize) -> bool {
    let Some(g) = d.levels.iter().position(|lv| lv.z == 0).filter(|_| n <= MAX_CELLARS) else { return false };
    let size = d.nx as usize * d.ny as usize;
    let floor: Vec<i16> = decode(&d.levels[g].cells, size).map_or_else(|| vec![-1; size], |c| c.iter().map(|&r| if r >= 0 { 0 } else { -1 }).collect());
    let ground_ft = d.levels[g].elevation_ft;
    while d.levels.iter().filter(|lv| lv.z < 0).count() > n {
        let k = deepest_cellar(d).expect("a cellar");
        d.levels.remove(k);
    }
    while d.levels.iter().filter(|lv| lv.z < 0).count() < n {
        let at = deepest_cellar(d).unwrap_or_else(|| d.levels.iter().position(|lv| lv.z == 0).expect("the ground floor"));
        let z = d.levels[at].z.min(0) - 1;
        d.levels.insert(
            at,
            DesignLevel {
                name: cellar_name(z).into(),
                elevation_ft: ground_ft + STOREY_FT * z as f32,
                natural: false,
                cells: encode(&floor),
                rooms: vec![DesignRoom { kind: "storeroom".into(), raise_ft: 0 }],
                doors: Vec::new(),
                items: Vec::new(),
                z,
                roof: false,
                has_stairs: true,
            },
        );
    }
    settle_stairs(d);
    true
}

/// Room and item kinds a building's design can be given (for the editor's menus).
pub fn catalog() -> Value {
    // A piece's usual size: as the first room that has it puts it down.
    let size = |kind: &str| {
        super::ROOM_KINDS.iter().flat_map(|r| super::furnishing(r).iter()).find(|e| e.0 == kind).map_or((1, 1), |e| match e.1 {
            super::Place::Wall(l, d) | super::Place::Center(l, d) => (l, d),
            super::Place::Seated | super::Place::Booth => (1, 1),
        })
    };
    let furniture: Vec<Value> = FURNITURE
        .iter()
        .map(|f| {
            let (w, h) = size(f.0);
            serde_json::json!({ "kind": f.0, "name": f.1, "cover": f.2, "blocks": f.3, "height_ft": f.4, "w": w, "h": h })
        })
        .collect();
    // Indoor props: their usual size as underground.
    let props: Vec<Value> = super::INDOOR_PROPS
        .iter()
        .filter_map(|k| crate::under::PROPS.iter().find(|p| p.0 == *k))
        .map(|p| {
            let (w, h) = crate::under::design::prop_size(p.0);
            serde_json::json!({ "kind": p.0, "name": p.1, "cover": p.2, "blocks": p.3, "height_ft": p.4, "hazard": p.5, "w": w, "h": h })
        })
        .collect();
    serde_json::json!({ "furniture": furniture, "props": props, "rooms": super::ROOM_KINDS, "max_floors": crate::world::MAX_FLOORS, "max_cellars": MAX_CELLARS })
}

/// Buildings' designs not built (the generated interior stands), with why: the building is gone,
/// its footprint or storeys changed, or it breaks a rule now (a neighbour built against its door).
pub fn set_aside(world: &World, t0: &T0) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (id, d) in world.file.edits.designs.iter().filter(|(id, _)| id.starts_with("b:")) {
        let why = match building_of(world, t0, id) {
            Err(e) => Some(e),
            Ok((li, l, bi)) => {
                let b = l.building(bi as usize).expect("checked");
                let sh = Shell::of(&l, b);
                if d.fingerprint != fingerprint(&sh, b) {
                    Some("made for the building as it was: its footprint or storeys have changed since".into())
                } else {
                    let (it, want) = build(d, t0, &l, li, b, &sh, &world.file.edits.sprites);
                    check(&it, &sh, &want).into_iter().find(|p| p.blocking).map(|p| format!("it breaks a rule now: {}", p.text))
                }
            }
        };
        if let Some(why) = why {
            out.push((id.clone(), why));
        }
    }
    out
}
