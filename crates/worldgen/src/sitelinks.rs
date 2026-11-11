//! An entrance made in a building's cellar (`Created::in_building`, `way`): which building a
//! created entrance is in (when that building still stands with a cellar), which entrances lead
//! down from a building, the dungeon's name (its entrance's) and the way up in the dungeon
//! that leads back to the cellar. The hooks are one call each in `under::generate`,
//! `interior::cellar_links` (`place_way`) and the battlemap's entrance loop.

use crate::interior::{Interior, Item, Level, Link};
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

/// Items `place_way` never takes away for a way: the ways and keys themselves, pictures and hangings.
const KEPT: [&str; 13] = ["sprite", "trapdoor", "hoist_trapdoor", "link_down", "link_up", "exit", "up", "down", "ladder", "key", "spiral_stair", "tapestry", "banner"];

/// Put the way down to a site (item `kind`, `name`, linked `to`) on cellar level `lvl` so it reads at a glance:
/// on a free floor square whose eight neighbours are free floor of its room (the app draws it bigger than its
/// square), the nearest to grid point `at`; else the square of the largest room with the most free neighbours,
/// nearest its middle; else any free square nearest `at`; with none free, the nearest furniture that may move
/// (`KEPT`, links and hazards stay) gives way. Off the stairs `st` and doorways. False only on a level with no floor
/// off them; `designed` (a hand design): free squares only, false when there is none (its `check` then says so).
pub fn place_way(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], at: [f64; 2], kind: &'static str, name: &'static str, to: String, designed: bool) -> bool {
    let mut taken = vec![false; nx * ny];
    let mut stair = vec![false; nx * ny];
    for f in &lvl.furniture {
        for j in f.y as usize..(f.y + f.h) as usize {
            for i in f.x as usize..(f.x + f.w) as usize {
                if i < nx && j < ny {
                    taken[j * nx + i] = true;
                }
            }
        }
    }
    for j in st[1]..(st[1] + st[3]).min(ny) {
        for i in st[0]..(st[0] + st[2]).min(nx) {
            taken[j * nx + i] = true;
            stair[j * nx + i] = true;
        }
    }
    let mut door = vec![false; nx * ny];
    for d in &lvl.doors {
        let (x, y) = (d.a[0].min(d.b[0]), d.a[1].min(d.b[1]));
        let back = (x - if d.a[0] == d.b[0] { 1.0 } else { 0.0 }, y - if d.a[1] == d.b[1] { 1.0 } else { 0.0 });
        for (x, y) in [(x, y), back] {
            if x >= 0.0 && y >= 0.0 && (x as usize) < nx && (y as usize) < ny {
                door[y as usize * nx + x as usize] = true;
            }
        }
    }
    let floor = |k: usize| lvl.cells[k] >= 0;
    let free = |taken: &[bool], k: usize| floor(k) && !taken[k] && !door[k];
    // Free neighbours of square k in its own room (of eight).
    let room_round = |taken: &[bool], k: usize| -> usize {
        let (i, j) = ((k % nx) as i64, (k / nx) as i64);
        let mut n = 0;
        for (di, dj) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (a, b) = (i + di, j + dj);
            if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                let q = b as usize * nx + a as usize;
                if free(taken, q) && lvl.cells[q] == lvl.cells[k] {
                    n += 1;
                }
            }
        }
        n
    };
    let dist = |k: usize, p: [f64; 2]| {
        let (dx, dy) = ((k % nx) as f64 + 0.5 - p[0], (k / nx) as f64 + 0.5 - p[1]);
        dx * dx + dy * dy
    };
    let nearest = |ks: &mut dyn Iterator<Item = usize>, p: [f64; 2]| ks.min_by(|&a, &b| dist(a, p).total_cmp(&dist(b, p)).then(a.cmp(&b)));
    let mut best = nearest(&mut (0..nx * ny).filter(|&k| free(&taken, k) && room_round(&taken, k) == 8), at);
    if best.is_none() {
        // The largest room's middle, as clear as it gets.
        let big = lvl.rooms.iter().enumerate().max_by(|a, b| a.1.squares.cmp(&b.1.squares).then(b.0.cmp(&a.0)));
        if let Some((ri, r)) = big {
            let mid = [r.center[0] as f64, r.center[1] as f64];
            let most = (0..nx * ny).filter(|&k| free(&taken, k) && lvl.cells[k] == ri as i16).map(|k| room_round(&taken, k)).max();
            if let Some(m) = most {
                best = nearest(&mut (0..nx * ny).filter(|&k| free(&taken, k) && lvl.cells[k] == ri as i16 && room_round(&taken, k) == m), mid);
            }
        }
    }
    if best.is_none() {
        best = nearest(&mut (0..nx * ny).filter(|&k| free(&taken, k)), at);
    }
    if best.is_none() && !designed {
        // No free floor: the nearest furniture that may move gives way, the way on one of its squares (never a way, a link, a hazard or a
        // picture; a hand design's items never: `designed` stops at the free squares, `design::room_for_way_down` refuses up front).
        let linked = |f: &Item| lvl.links.iter().any(|l| l.x >= f.x && l.x < f.x + f.w && l.y >= f.y && l.y < f.y + f.h);
        let fixed = |f: &Item| linked(f) || f.hazard.is_some() || KEPT.contains(&f.kind);
        let mut order: Vec<usize> = (0..lvl.furniture.len()).filter(|&n| !fixed(&lvl.furniture[n])).collect();
        let mid = |f: &Item| [f.x as f64 + f.w as f64 * 0.5 - 0.5, f.y as f64 + f.h as f64 * 0.5 - 0.5];
        order.sort_by(|&a, &b| {
            let (pa, pb) = (mid(&lvl.furniture[a]), mid(&lvl.furniture[b]));
            let d = |p: [f64; 2]| (p[0] - at[0] + 0.5) * (p[0] - at[0] + 0.5) + (p[1] - at[1] + 0.5) * (p[1] - at[1] + 0.5);
            d(pa).total_cmp(&d(pb)).then(a.cmp(&b))
        });
        for n in order {
            let f = &lvl.furniture[n];
            let squares: Vec<usize> = (f.y as usize..(f.y + f.h) as usize).flat_map(|j| (f.x as usize..(f.x + f.w) as usize).map(move |i| j * nx + i)).collect();
            if let Some(k) = nearest(&mut squares.into_iter().filter(|&k| k < nx * ny && floor(k) && !stair[k] && !door[k]), at) {
                lvl.furniture.remove(n);
                best = Some(k);
                break;
            }
        }
    }
    // (Last: any floor off the stairs and doorways.)
    if best.is_none() && !designed {
        best = nearest(&mut (0..nx * ny).filter(|&k| floor(k) && !stair[k] && !door[k]), at);
    }
    let Some(k) = best else { return false };
    let (x, y) = ((k % nx) as u16, (k / nx) as u16);
    lvl.furniture.push(Item { kind, name, x, y, w: 1, h: 1, cover: 0, blocks_move: false, height_ft: 0.0, hazard: None, sprite: 0, back: None, variant: None });
    lvl.links.push(Link { x, y, to });
    true
}
