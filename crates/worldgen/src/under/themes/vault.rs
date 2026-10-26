//! Flooded vaults (`flooded_vault`), grown room by room (`grow`): the flooded stair at the way down from above, one
//! or two great flooded halls (pillared) and a cistern off it, deep water in them crossed by bridges, stepping stones
//! or a ledge along the walls; old vaults, a treasury (sometimes behind an iron grate), a sunken chapel and drowned
//! barracks grown among flooded passages, shallow water there; more water the deeper it goes. The deepest level ends
//! in the boss chamber, a dead end, kept dry.

use super::keep::hub;
use crate::core::rng::Pcg32;
use crate::under::grow::{ROOMS_MAX, Spec, Template, boss_room, colonnade, dead_end, fallback, fill, flood_room, grate, rooms, way_down};
use crate::under::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn vault(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let odd = |n: usize| n | 1;
    let cw = theme.corridor;
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(4);
    let Some((mut g, core)) = hub(&mut p, passage, theme.first, Template::Rect, arrive, rng) else { return fallback(p, last) };
    g.spread = true;
    // The great halls (pillared) and a cistern: deep water; more and longer halls on bigger sites.
    let mut halls: Vec<i16> = Vec::new();
    let mut near = vec![core];
    let sc = |n: usize| ((n as f64) * crate::core::sqrt(area).max(1.0)).round() as usize;
    for _ in 0..1 + (area >= 1.0 && rng.next_f64() < 0.6) as usize + (area >= 1.3) as usize + (area >= 2.0) as usize {
        let size = (odd(pick(7, 9, rng)), pick(9, sc(13), rng));
        if let Some(h) = g.attach(&mut p, &near, "flooded hall", Template::Rect, size, cw, rng).or_else(|| g.attach(&mut p, &near, "flooded hall", Template::Rect, (7, 8), cw, rng)) {
            halls.push(g.pieces[h].room);
            near.push(h);
        }
    }
    let t = if rng.next_f64() < 0.5 { Template::Round } else { Template::Rect };
    let mut cistern = None;
    if let Some(c) = g.attach(&mut p, &near, "cistern", t, (7, 7), cw, rng) {
        cistern = Some(g.pieces[c].room);
    }
    // A treasury or old vault behind an iron grate at times.
    let kind = if rng.next_f64() < 0.5 { "treasury" } else { "old vault" };
    if let Some(v) = g.attach(&mut p, &[], kind, Template::Rect, (pick(3, 4, rng), pick(3, 4, rng)), cw, rng) {
        g.pieces[v].open = false;
        if rng.next_f64() < 0.4 {
            grate(&mut p, &g.pieces[v]);
        }
    }
    let s = |kind, t, across, deep| Spec { kind, t, across, deep };
    let specs = [
        s("old vault", Template::Rect, (3, 5), (3, 5)),
        s("treasury", Template::Rect, (3, 4), (3, 4)),
        s("sunken chapel", Template::Apse, (5, 5), (4, 6)),
        s("drowned barracks", Template::Rect, (4, 6), (4, 5)),
        s("drowned barracks", Template::Rect, (3, 4), (3, 4)),
        s("cistern", Template::Round, (5, 5), (5, 5)),
        s("flooded hall", Template::Rect, (5, 7), (6, 9)),
        s(theme.passage, Template::Corridor, (cw, cw), (3, sc(6))),
        s(theme.passage, Template::Corridor, (cw, cw), (2, 4)),
    ];
    let have = rooms(&p, passage);
    g.grow(&mut p, &specs, target.saturating_sub(have), false, rng);
    let mut boss = None;
    if last {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng).or_else(|| {
            let b = dead_end(&p, ny)?;
            p.rooms[b as usize].kind = BOSS;
            Some(b)
        });
    }
    p.passage = Some((passage, cw > 1));
    p.taken[arrive] = true;
    if !last {
        let d = way_down(&p, ny, &[]);
        p.down = Some(d);
        p.taken[d] = true;
    }
    // Pillars down the great halls.
    let mut squares = vec![0usize; p.rooms.len()];
    for &c in p.cells.iter().filter(|&&c| c >= 0) {
        squares[c as usize] += 1;
    }
    for r in 0..p.rooms.len() as i16 {
        let Some([_, _, w, h]) = crate::interior::shape::room_box(&p.cells, nx, r) else { continue };
        let (len, wide) = (w.max(h), w.min(h));
        if r == passage || Some(r) == boss || p.rooms[r as usize].shape != 0 || wide < 7 || len < 8 {
            continue;
        }
        colonnade(&mut p, ny, r, (wide - 5) / 2, 2);
        if !halls.contains(&r) {
            halls.push(r);
        }
    }
    // The water: deep in the halls and cisterns (ledges along the walls of some), shallow elsewhere, more deeper down;
    // the vaults and treasuries mostly dry, the boss chamber dry.
    let more = 0.06 * depth.min(4) as f64;
    for r in 0..p.rooms.len() as i16 {
        if Some(r) == boss {
            continue;
        }
        let kind = p.rooms[r as usize].kind;
        if Some(r) == cistern || kind == "cistern" {
            flood_room(&mut p, ny, r, (-1.0, 0.2 - more), true);
        } else if halls.contains(&r) || squares[r as usize] >= 40 {
            let ledge = rng.next_f64() < 0.5;
            flood_room(&mut p, ny, r, (-1.0, 0.34 - more), ledge);
        } else if matches!(kind, "treasury" | "old vault") {
            flood_room(&mut p, ny, r, (0.62 - more, 2.0), false);
        } else {
            flood_room(&mut p, ny, r, (0.3 - more, 2.0), false);
        }
    }
    p
}
