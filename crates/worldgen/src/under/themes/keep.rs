//! A large keep's deep dungeons and a city's undercroft, grown room by room (`grow`) with prison and store kits.
//!
//! Keep (`keep`): the landing at the stairs down from the keep, its one way on behind an iron grate; a prison wing
//! (a cell block, a grate at its mouth), stores, armory, guard room and a well room (a shallow pool) on the top
//! level; below, the crypt of the old lords (pillared), a secret vault, oubliettes, torture chamber and shrine. Big
//! keeps sprawl. The deepest level ends in the boss chamber, a dead end. Mostly dry.
//!
//! Undercroft (`undercroft`, below a sewer section): the drain chamber at the way down from the sewers, cisterns
//! (pillared; sometimes flooded, deep water with a wading ledge round it), overflow chambers, a smugglers' cache
//! (sometimes behind a hidden door), an old vault, a rat warren, sometimes a flooded hall crossed by a bridge or
//! stepping stones; culverts among them.

use crate::core::rng::Pcg32;
use crate::under::grow::{Grower, ROOMS_GROWING, ROOMS_MAX, Side, Template, boss_room, grate, rooms, specs, colonnade, dead_end, fallback, flood_room, way_down};
use crate::under::*;

const CELL: &str = "cell";
const ROW: &str = "cell row";

#[allow(clippy::too_many_arguments)]
pub(crate) fn keep(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let cw = theme.corridor;
    let mut target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * area).round() as usize).max(4);
    if theme.sprawl {
        target = target.max(nx * ny / 140).min(40);
    }
    let Some((mut g, core)) = hub(&mut p, passage, theme.first, Template::Rect, arrive, rng) else { return fallback(p, last) };
    let mut boss = None;
    let mut pools: Vec<(i16, (f64, f64), bool)> = Vec::new();
    let mut pillared = Vec::new();
    if depth == 0 {
        // The landing's one way on, behind a grate.
        let s0 = rng.below(4) as usize;
        let mut gate = None;
        for k in 0..4 {
            if let Some(&c) = g.deck(&mut p, core, Side::ALL[(s0 + k) % 4], "", Template::Corridor, cw, pick(2, 4, rng), 0, false).first() {
                gate = Some(c);
                break;
            }
        }
        let mut near = vec![core];
        if let Some(c) = gate {
            grate(&mut p, &g.pieces[c]);
            g.pieces[core].open = false;
            near = vec![c];
        }
        // The prison wing: a cell block, its grate at the mouth.
        let rw = 1 + (rng.next_f64() < 0.4) as usize;
        let cell = if rng.next_f64() < 0.5 { (1, 2) } else { (2, 2) };
        let wing = cell_block(&mut p, &mut g, &near, (ROW, rw, pick(5, 8, rng)), (CELL, cell.0, cell.1), rng);
        near.extend(wing);
        // The stores, guard room and the well room (a shallow pool round its well).
        for (kind, size) in [("guard room", (4, 4)), ("storeroom", (4, 5)), ("armory", (3, 4)), ("well room", (4, 4))] {
            let Some(r) = g.attach(&mut p, &near, kind, Template::Rect, size, cw, rng) else { continue };
            if kind == "well room" {
                pools.push((g.pieces[r].room, (0.45, 2.0), false));
            }
        }
        let specs = specs(&["guard room", "barracks", "storeroom", "armory", "torture chamber"], cw, ((4, 7), (4, 6)));
        let have = rooms(&p, passage);
        g.grow(&mut p, &specs, target.saturating_sub(have), false, rng);
    } else {
        // The crypt of the old lords (pillared), a secret vault, oubliettes.
        let near = vec![core];
        let (a, d) = (7 + 2 * rng.below(2) as usize, pick(7, 10, rng));
        if let Some(c) = g.attach(&mut p, &near, "crypt of the old lords", Template::Rect, (a, d), cw, rng) {
            pillared.push((g.pieces[c].room, (a - 5) / 2));
        }
        if let Some(v) = g.attach(&mut p, &[], "secret vault", Template::Rect, (3, 3), 0, rng) {
            g.pieces[v].open = false;
            if rng.next_f64() < 0.5
                && let Some((x, y)) = g.pieces[v].door
            {
                p.secret.insert((x.min(y), x.max(y)));
            }
        }
        for i in 0..pick(1, 2, rng) {
            let n = pick(3, 4, rng);
            if let Some(o) = g.attach(&mut p, &[], "oubliette", Template::Rect, (n, n), cw, rng) {
                g.pieces[o].open = false;
                if i == 0 && n == 4 && rng.next_f64() < 0.35 {
                    pools.push((g.pieces[o].room, (-1.0, -1.0), true));
                }
            }
        }
        let specs = specs(&["torture chamber", "oubliette", "shrine", "storeroom", "guard room", "cell block"], cw, ((4, 7), (4, 6)));
        let have = rooms(&p, passage);
        g.grow(&mut p, &specs, target.saturating_sub(have).max(2), false, rng);
    }
    if last {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng);
    }
    finish(&mut p, ny, passage, cw, last, boss, &pools, &pillared);
    p
}

/// The undercroft below a sewer section (always its deepest level).
pub(crate) fn undercroft(nx: usize, ny: usize, arrive: usize, last: bool, theme: &Theme, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let cw = theme.corridor;
    let target = (theme.rooms.0 + rng.below(theme.rooms.1) as usize).max(4);
    let t = if rng.next_f64() < 0.5 { Template::Round } else { Template::Rect };
    let Some((mut g, core)) = hub(&mut p, passage, theme.first, t, arrive, rng) else { return fallback(p, last) };
    let (mut pools, mut pillared): (Vec<(i16, (f64, f64), bool)>, Vec<(i16, usize)>) = (Vec::new(), Vec::new());
    let near = vec![core];
    // Cisterns: pillared halls, sometimes flooded (deep, a wading ledge round), else puddled.
    for i in 0..pick(1, 2, rng) {
        let (a, d) = (7 + 2 * rng.below(2) as usize, pick(7, 11, rng));
        let Some(c) = g.attach(&mut p, &near, "cistern", Template::Rect, (a, d), cw, rng) else { continue };
        let r = g.pieces[c].room;
        pillared.push((r, 1));
        if i == 0 && rng.next_f64() < 0.45 {
            pools.push((r, (-1.0, -1.0), true));
        } else {
            pools.push((r, (0.5, 2.0), false));
        }
    }
    // Overflow chambers (puddled), the smugglers' cache (sometimes behind a hidden door), an old vault, a rat warren.
    for (kind, t, size) in [
        ("overflow chamber", Template::Rect, (5, 4)),
        ("smugglers' cache", Template::Rect, (3, 4)),
        ("old vault", Template::Rect, (4, 4)),
        ("rat warren", Template::Cross, (7, 7)),
        ("overflow chamber", Template::Rect, (4, 5)),
    ] {
        let Some(c) = g.attach(&mut p, &[], kind, t, size, cw, rng) else { continue };
        let r = g.pieces[c].room;
        match kind {
            "overflow chamber" => pools.push((r, (0.4, 2.0), false)),
            "smugglers' cache" if rng.next_f64() < 0.4 => {
                g.pieces[c].open = false;
                if let Some((x, y)) = g.pieces[c].door {
                    p.secret.insert((x.min(y), x.max(y)));
                }
            }
            _ => {}
        }
    }
    // Sometimes a flooded hall: a long gallery of deep water, crossed by a bridge or stepping stones.
    if rng.next_f64() < 0.3
        && let Some(c) = g.attach(&mut p, &[], "flooded hall", Template::Rect, (3, pick(8, 12, rng)), cw, rng)
    {
        pools.push((g.pieces[c].room, (-1.0, 0.2), false));
    }
    let specs = specs(&["overflow chamber", "old vault", "rat warren", "cistern", "smugglers' cache"], cw, ((4, 7), (4, 6)));
    let have = rooms(&p, passage);
    g.grow(&mut p, &specs, target.saturating_sub(have), false, rng);
    g.cap = ROOMS_MAX;
    let boss = if last { boss_room(&mut p, &mut g, ny, rng) } else { None };
    finish(&mut p, ny, passage, cw, last, boss, &pools, &pillared);
    p
}

/// A level's first room (`kind`, `t`, 3-5 squares) round the arrival, the grower to go on with, and its piece.
fn hub(p: &mut Plan, passage: i16, kind: &'static str, t: Template, arrive: usize, rng: &mut Pcg32) -> Option<(Grower, usize)> {
    let nx = p.nx;
    let ny = p.cells.len() / nx;
    let (ax, ay) = (arrive % nx, arrive / nx);
    let s = 3 + 2 * rng.below(2) as usize;
    let x = ax.saturating_sub(s / 2).clamp(1, nx.saturating_sub(s + 1).max(1));
    let y = ay.saturating_sub(s / 2).clamp(1, ny.saturating_sub(s + 1).max(1));
    let (m, _) = crate::under::grow::mask(t, x, y, s, s, Side::E);
    let holds = (x..x + s).contains(&ax) && (y..y + s).contains(&ay) && m[(ay - y) * s + ax - x];
    let t = if holds { t } else { Template::Rect };
    let mut g = Grower::new(nx, ny, passage, None);
    g.cap = ROOMS_GROWING;
    let core = g.seed(p, kind, t, x, y, s, s).or_else(|| g.seed(p, kind, Template::Rect, ax, ay, 1, 1))?;
    Some((g, core))
}

/// A cell block off one of the `near` pieces (else any open one), on whichever side it fits. Its rows.
fn cell_block(p: &mut Plan, g: &mut Grower, near: &[usize], row: (&'static str, usize, usize), cell: (&'static str, usize, usize), rng: &mut Pcg32) -> Vec<usize> {
    let s0 = rng.below(4) as usize;
    let mut order: Vec<usize> = near.to_vec();
    order.extend((0..g.pieces.len()).filter(|i| !near.contains(i) && g.pieces[*i].open));
    for i in order {
        for k in 0..4 {
            let rows = g.cell_block(p, i, Side::ALL[(s0 + k) % 4], 0, row, cell, true, false);
            if !rows.is_empty() {
                return rows;
            }
        }
    }
    Vec::new()
}

/// A level's end: the boss chamber if none found room (the farthest dead end), the passage, the way down (not in a
/// cell), pillars down the `pillared` halls (room, inset from the long walls), the pools laid (room, shallow and deep
/// heights, ledge).
#[allow(clippy::too_many_arguments)]
fn finish(p: &mut Plan, ny: usize, passage: i16, cw: usize, last: bool, mut boss: Option<i16>, pools: &[(i16, (f64, f64), bool)], pillared: &[(i16, usize)]) {
    if last && boss.is_none() {
        boss = dead_end(p, ny);
        if let Some(b) = boss {
            p.rooms[b as usize].kind = BOSS;
        }
    }
    p.passage = Some((passage, cw > 1));
    p.taken[p.up] = true;
    if !last {
        let skip: Vec<i16> = (0..p.rooms.len() as i16).filter(|&r| matches!(p.rooms[r as usize].kind, CELL | ROW | "secret vault")).collect();
        let d = way_down(p, ny, &skip);
        p.down = Some(d);
        p.taken[d] = true;
    }
    for &(r, inset) in pillared.iter().filter(|q| Some(q.0) != boss) {
        colonnade(p, ny, r, inset, 2);
    }
    for &(r, bands, ledge) in pools.iter().filter(|q| Some(q.0) != boss) {
        flood_room(p, ny, r, bands, ledge);
    }
}
