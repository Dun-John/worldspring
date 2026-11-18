//! A built site's boss chamber (item 117): a tactical layout picked per site by the rng and by fit (cover: a pillar
//! grid, broken walls, a statue gallery, a rubble ring; obstacles: pits, a chasm with its bridge, a moat round an
//! island; height: a raised platform, balconies, a sunken arena), then the theme's centrepiece (a throne only where a
//! ruler fits, a ritual circle only in temples and cult sanctums) and dressing. Heights go through the floor model
//! (`interior::height`: `platform`/`balcony` raise, `steps`/`climb_ladder` are ramps). Caves and lava tubes get the
//! natural set instead, mines a half-dug face with scaffolding (`wild`). Any of them may get hazards (`hazards`) and a
//! second way in (`side_door`, picked before the level is furnished).

use crate::core::rng::Pcg32;
use crate::interior::Item;
use crate::interior::height::{RAISING, RAMPS};
use crate::under::*;

/// The built layouts, as the `under` example's metrics name them.
pub const LAYOUTS: [&str; 10] = ["pillar grid", "broken walls", "statue gallery", "rubble ring", "pits", "chasm and bridge", "moat", "raised platform", "balconies", "sunken arena"];

/// The least room each layout needs: (depth from the way in, width across).
const FIT: [(usize, usize); 10] = [(5, 5), (5, 5), (6, 5), (8, 7), (6, 5), (7, 4), (7, 5), (6, 5), (6, 5), (8, 7)];

mod built;
mod hall;
mod wild;

pub(crate) use built::{built, side_door};
use built::*;
use hall::*;
pub(crate) use wild::wild;
use wild::*;

// Item 117 slice 2: caves', lava tubes' and mines' boss chambers, hazards, a second way in.

/// The natural sets' layouts (caves: a pool round an island; lava tubes: a lava channel), as the `under` example names
/// them.
pub const NATURAL: [&str; 6] = ["rock columns", "ledges", "rope-bridge chasm", "rubble field", "pool island", "lava channel"];
const NFIT: [(usize, usize); 6] = [(5, 5), (6, 5), (7, 4), (5, 5), (7, 5), (6, 4)];

/// A mine's: the half-dug face with scaffolding up ladders (along one side wall, both, or across the face).
pub const MINE: [&str; 3] = ["side walkway", "twin walkways", "face gallery"];
const MFIT: [(usize, usize); 3] = [(6, 4), (6, 6), (5, 6)];

/// The hazards a boss chamber may get (`hazards`), by item name; the hidden ones' rules start "hidden" (DM-only until
/// found, as traps).
pub const PERILS: [&str; 4] = ["steam vent", "cracked floor", "flooded pit", "pressure plate"];

/// Grid steps by an item's `back` code: 0 north, 1 east, 2 south, 3 west.
const DIRS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

fn step(k: usize, dir: usize, nx: usize, ny: usize) -> Option<usize> {
    let (x, y) = ((k % nx) as isize + DIRS[dir].0, (k / nx) as isize + DIRS[dir].1);
    (x >= 0 && y >= 0 && (x as usize) < nx && (y as usize) < ny).then(|| y as usize * nx + x as usize)
}

/// The side square `b` lies on from its neighbour `a` (an item's `back`: steps' way up).
fn toward(a: usize, b: usize, nx: usize) -> u8 {
    if b + nx == a {
        0
    } else if b == a + 1 {
        1
    } else if b == a + nx {
        2
    } else {
        3
    }
}

fn peril(name: &str, x: usize, y: usize) -> Item {
    let kind = match name {
        "steam vent" => "vent",
        "flooded pit" => "flooded_pit",
        "pressure plate" => "trap",
        _ => "cracked_floor",
    };
    item(kind, x, y, 1, 1)
}


// Item 117 fix round 2: dressing through the whole chamber, hazards kept off the centrepiece, the room's label spot.

/// The theme's freestanding cover, 1x1 (the kinds `spread` puts down): none that tells a layout apart in the `under`
/// example's metrics (statues, pillars, broken walls, rubble, rock columns).
fn cover_kinds(theme: &str, natural: bool) -> &'static [&'static str] {
    match theme {
        "keep" | "dungeon" | FORTRESS => &["weapon_rack", "barrel", "crate"],
        "dwarven_hall" => &["barrel", "anvil", "keg_rack"],
        "temple" | "sunken_temple" | "cult_sanctum" => &["urn", "effigy"],
        "wizard_lair" => &["bookshelf", "crate", "table"],
        "prison" => &["rack", "stocks", "iron_maiden"],
        "crypt" | "tomb" | "catacombs" => &["coffin", "urn", "effigy"],
        "ossuary" => &["skulls", "urn", "coffin"],
        "bandit_hideout" => &["crate", "barrel", "sacks"],
        "goblin_warren" => &["sacks", "crate", "skulls"],
        "flooded_vault" => &["crate", "barrel", "urn"],
        "labyrinth" => &["skulls", "boulder", "urn"],
        "mine" => &["crate", "barrel", "timber", "ore_cart"],
        "lava_tube" => &["boulder"],
        "fungal" => &["mushroom", "boulder"],
        "crystal" => &["crystal", "boulder"],
        "ice" => &["ice", "boulder"],
        _ if natural => &["stalagmite", "boulder"],
        _ => &["crate", "barrel"],
    }
}

/// Kinds that stand against a wall, their back to it.
const WALLED: [&str; 4] = ["bookshelf", "weapon_rack", "keg_rack", "rack"];

/// The squares kept for the centrepiece: the stage and a square round it, and round what the dressing put down (the
/// items from `dressed` on, but what hangs on the walls).
fn held(p: &Plan, h: &Hall, s: Stage, dressed: usize) -> Vec<bool> {
    let (nx, ny) = (h.nx, h.ny);
    let mut out = vec![false; nx * ny];
    for d in s.d0 - 1..=s.far() + 1 {
        for a in s.a0 - 1..s.a0 + s.aw as isize + 1 {
            if let Some(k) = h.cell(d, a) {
                out[k] = true;
            }
        }
    }
    for it in p.items.iter().skip(dressed).filter(|it| !matches!(it.kind, "banner" | "chains" | "sconce" | "lantern")) {
        for j in (it.y as usize).saturating_sub(1)..((it.y + it.h) as usize + 1).min(ny) {
            for i in (it.x as usize).saturating_sub(1)..((it.x + it.w) as usize + 1).min(nx) {
                out[j * nx + i] = true;
            }
        }
    }
    out
}

/// Cover and dressing in every quarter of the chamber (near and far, either side of its middle) that has room and none:
/// one or two of `kinds` each, standing apart, off the main lane (a square either side of the line from the way in to
/// the stage), the landings, the second way in and the stage; each kept only if the chamber is still all reached.
fn spread(p: &mut Plan, h: &Hall, s: Stage, kinds: &[&'static str], rng: &mut Pcg32) {
    let (l, w) = (h.l as isize, h.w as isize);
    let (nx, ny) = (h.nx, h.ny);
    let (ed, ea) = (h.ed as isize, h.ea as isize);
    let (td, ta) = (s.d0.max(ed + 1), s.mid());
    // (|a - the lane's a at d| <= 1, in whole numbers.)
    let lane = |d: isize, a: isize| d <= td && (a * (td - ed) - ea * (td - ed) - (ta - ea) * (d - ed).max(0)).abs() <= td - ed;
    let (ex, ey) = (h.entry % nx, h.entry / nx);
    let held = held(p, h, s, p.items.len());
    let quarter = |d: isize, a: isize| (d * 2 >= l) as usize * 2 + (a * 2 >= w) as usize;
    let cover = |it: &Item| PROPS.iter().any(|q| q.0 == it.kind && q.2 >= 1) && !TERRAIN.contains(&it.kind) && !SPANS.contains(&it.kind);
    let mut has = [false; 4];
    for it in p.items.iter().filter(|it| cover(it)) {
        let k = it.y as usize * nx + it.x as usize;
        if p.cells[k] == h.ri {
            let (d, a) = h.frame(k);
            has[quarter(d as isize, a as isize)] = true;
        }
    }
    for q in 0..4 {
        let squares = frame_squares(p, h).into_iter().filter(|&(d, a)| quarter(d, a) == q).count();
        if has[q] || squares < 4 {
            continue;
        }
        let cells = &p.cells;
        let wall = |k: usize| (0..4u8).find(|&dir| step(k, dir as usize, nx, ny).is_none_or(|m| cells[m] != h.ri));
        // (Standing alone where it can, else beside something.)
        let find = |alone: bool| -> Vec<(isize, isize, usize, Option<u8>)> {
            frame_squares(p, h)
                .into_iter()
                .filter(|&(d, a)| quarter(d, a) == q && !lane(d, a))
                .filter_map(|(d, a)| h.sq(p, d, a).map(|k| (d, a, k)))
                .filter(|&(_, _, k)| {
                    !p.taken[k] && !held[k] && !h.kept(k) && !h.clear.borrow().contains(&k) && (k % nx).abs_diff(ex).max((k / nx).abs_diff(ey)) >= 2 && (!alone || neighbours(k, nx, ny).all(|m| !p.taken[m] || cells[m] != h.ri))
                })
                .map(|(d, a, k)| (d, a, k, wall(k)))
                .collect()
        };
        let mut spots = find(true);
        if spots.is_empty() {
            spots = find(false);
        }
        let want = 1 + (spots.len() >= 16) as usize;
        let mut put = 0;
        for _ in 0..want * 6 {
            if put >= want || spots.is_empty() {
                break;
            }
            let kind = kinds[rng.below(kinds.len() as u32) as usize];
            let walled = WALLED.contains(&kind);
            let fit: Vec<usize> = (0..spots.len()).filter(|&i| spots[i].3.is_some() == walled).collect();
            if fit.is_empty() {
                continue;
            }
            let (d, a, k, back) = spots[fit[rng.below(fit.len() as u32) as usize]];
            let snap = snap(p);
            if set(p, h, h.item(kind, d, a, 1, 1, back), false) && reached(p, h) {
                put += 1;
                spots.retain(|&(d2, a2, _, _)| (d2 - d).abs().max((a2 - a).abs()) >= 3);
            } else {
                undo(p, snap);
                spots.retain(|&(_, _, m, _)| m != k);
            }
        }
    }
}

/// Where a boss chamber's name goes (`Room::center`): on open floor, away from its props, bridges, steps and doors: a
/// square of the room with no item on or round it (else the most open squares round it, five at least), the most open
/// floor along its row (the name runs across), nearest the room's middle; else the middle as before (None).
pub(crate) fn label(cells: &[i16], items: &[Item], doors: &crate::core::hash::FastSet<(usize, usize)>, nx: usize, ny: usize, ri: i16, middle: [f32; 2]) -> Option<usize> {
    let n = nx * ny;
    let mut busy = vec![false; n];
    for it in items {
        // (A raised floor is floor: its edge is what the name should keep off.)
        let edge_only = RAISING.contains(&it.kind);
        for j in it.y as usize..(it.y + it.h) as usize {
            for i in it.x as usize..(it.x + it.w) as usize {
                let inner = i > it.x as usize && j > it.y as usize && i + 1 < (it.x + it.w) as usize && j + 1 < (it.y + it.h) as usize;
                if !edge_only || !inner {
                    busy[j * nx + i] = true;
                }
            }
        }
    }
    for &(a, b) in doors {
        busy[a] = true;
        busy[b] = true;
    }
    let open = |k: usize| cells[k] == ri && !busy[k];
    // (Open squares round it, of the eight.)
    let clear = |k: usize| {
        let (x, y) = (k % nx, k / nx);
        if x == 0 || y == 0 || x + 1 >= nx || y + 1 >= ny {
            return 0;
        }
        (y - 1..=y + 1).flat_map(|j| (x - 1..=x + 1).map(move |i| j * nx + i)).filter(|&m| m != k && open(m)).count()
    };
    let run = |k: usize| {
        let (x, y) = (k % nx, k / nx);
        (x.saturating_sub(2)..=(x + 2).min(nx - 1)).filter(|&i| open(y * nx + i)).count()
    };
    let dist = |k: usize| {
        let (dx, dy) = ((k % nx) as f32 + 0.5 - middle[0], (k / nx) as f32 + 0.5 - middle[1]);
        dx * dx + dy * dy
    };
    (0..n).filter(|&k| open(k) && clear(k) >= 5).max_by(|&a, &b| (clear(a) == 8).cmp(&(clear(b) == 8)).then(run(a).cmp(&run(b))).then(clear(a).cmp(&clear(b))).then(dist(b).total_cmp(&dist(a))).then(b.cmp(&a)))
}
