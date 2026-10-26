//! Temples underground (`temple`, `sunken_temple`), grown room by room (`grow`). The top level mirrored on its axis:
//! a narthex at the way in, a pillared nave, side chapels in pairs, sometimes a crossing with its transept arms, the
//! sanctum at the far end rounded in an apse (sometimes behind a portcullis). Lower levels loosely ordered round a
//! mirrored core (a hall at the way down from above, cells, reliquary, ossuary, ritual pool in pairs), the rest grown
//! freely, corridors among them; the deepest ends in the boss chamber, a dead end. A sunken temple is flooded: deep
//! water in its big halls (ledges along their walls, or bridges and stepping stones), shallow elsewhere, more the
//! deeper it goes.

use crate::core::rng::Pcg32;
use crate::under::grow::{Grower, Side, Spec, Template, boss_room, colonnade, dead_end, fallback, flood_room, way_down};
use crate::under::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn temple(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let wide = theme.corridor > 1;
    let (ax, ay) = (arrive % nx, arrive / nx);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi - lo + 1) as u32) as usize;
    let odd = |n: usize| n | 1;
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * area).round() as usize).max(4);
    let mut boss: Option<i16> = None;
    // The halls to flood deep in a sunken temple, and the rooms kept dry.
    let mut halls: Vec<i16> = Vec::new();
    let mut g;
    if depth == 0 {
        // Narthex, nave, (crossing), sanctum along the axis through the way in.
        g = Grower::new(nx, ny, passage, Some(ay));
        let (nw, nh) = (pick(4, 5, rng), odd(pick(5, 7, rng)).min(odd(ny.saturating_sub(4)).max(3)));
        let x0 = ax.saturating_sub(1).max(1);
        let y0 = ay.saturating_sub(nh / 2).max(1);
        let narthex = g.seed(&mut p, theme.first, Template::Rect, x0, y0, nw, nh).or_else(|| g.seed(&mut p, theme.first, Template::Rect, ax, ay, 1, 1));
        let Some(narthex) = narthex else { return fallback(p, last) };
        let cross = rng.next_f64() < 0.45;
        // (Bigger sites: a longer, wider nave, a bigger sanctum and crossing.)
        let sc = |n: usize| ((n as f64) * area.sqrt()).round() as usize;
        let (sd, sa) = (pick(5, sc(7), rng), odd(pick(5, sc(7), rng)));
        let cw = if cross { odd(pick(5, sc(7), rng)) } else { 0 };
        let room_left = nx.saturating_sub(x0 + nw + 2 + sd + cw);
        let nave_h = if ny >= 30 { odd(pick(7, sc(9).min(11), rng)) } else { 7 };
        let nave_len = pick(sc(10), sc(18), rng).min(room_left).max(6);
        let nave = g.deck(&mut p, narthex, Side::E, "nave", Template::Rect, nave_h, nave_len, 0, true);
        let mut end = narthex;
        if let Some(&nave) = nave.first() {
            end = nave;
            halls.push(g.pieces[nave].room);
            // Side chapels in pairs down the nave's long walls.
            let pairs = pick(1, sc(3), rng).min(nave_len / 5).max(1);
            for c in 0..pairs {
                let along = ((c + 1) * nave_len / (pairs + 1)) as isize - (nave_len / 2) as isize;
                let t = [Template::Rect, Template::Round, Template::Octagon, Template::Apse][rng.below(4) as usize];
                let (a, d) = (odd(pick(3, sc(4), rng)), pick(3, sc(4), rng));
                for i in g.deck(&mut p, nave, Side::N, "side chapel", t, a, d, along, true) {
                    g.pieces[i].open = false;
                }
            }
        }
        if cross && let Some(&c) = g.deck(&mut p, end, Side::E, "crossing", Template::Cross, odd(nave_h + 4).min(odd(ny.saturating_sub(6))), cw, 0, true).first() {
            end = c;
            halls.push(g.pieces[c].room);
        }
        // Vestries in a pair off the last hall before the sanctum: the transept's ends, or beside the sanctum.
        let (va, vd) = (pick(3, 4, rng), pick(3, 4, rng));
        let along = if end == narthex || p.rooms[g.pieces[end].room as usize].kind == "crossing" { 0 } else { (g.pieces[end].w / 2) as isize - va as isize / 2 - 1 };
        g.deck(&mut p, end, Side::N, "vestry", Template::Rect, va, vd, along, true);
        let kind = if last { BOSS } else { "sanctum" };
        let mut gate = None;
        for (t, a, d) in [(Template::Apse, sa, sd), (Template::Apse, 5, 5), (Template::Rect, 3, 3)] {
            if let Some(&s) = g.deck(&mut p, end, Side::E, kind, t, a, d, 0, true).first() {
                g.pieces[s].open = false;
                gate = g.pieces[s].door;
                if last {
                    boss = Some(g.pieces[s].room);
                }
                break;
            }
        }
        // Sometimes an iron grate at the sanctum's gate.
        if let Some((a, b)) = gate
            && rng.next_f64() < 0.35
        {
            p.portcullis.insert((a.min(b), a.max(b)));
        }
        // Then more rooms in pairs, to the theme's count.
        let have = p.rooms.iter().enumerate().filter(|&(r, _)| r as i16 != passage).count();
        let specs = specs(theme, &["sanctum", "ossuary"], wide);
        g.grow(&mut p, &specs, target.saturating_sub(have), true, rng);
    } else {
        // A hall round the way down from above, its axis through it; a mirrored core grown off it, then the rest.
        let t = [Template::Octagon, Template::Cross, Template::Round, Template::Rect][rng.below(4) as usize];
        let s = odd(pick(5, 7, rng));
        let x = ax.saturating_sub(s / 2).clamp(1, nx.saturating_sub(s + 1).max(1));
        let y = ay.saturating_sub(s / 2).clamp(1, ny.saturating_sub(s + 1).max(1));
        let (m, _) = crate::under::grow::mask(t, x, y, s, s, Side::E);
        let holds = (x..x + s).contains(&ax) && (y..y + s).contains(&ay) && m[(ay - y) * s + ax - x];
        let t = if holds { t } else { Template::Rect };
        g = Grower::new(nx, ny, passage, Some(y + s / 2));
        let Some(core) = g.seed(&mut p, "landing", t, x, y, s, s).or_else(|| g.seed(&mut p, "landing", Template::Rect, ax, ay, 1, 1)) else {
            return fallback(p, last);
        };
        if s >= 7 {
            halls.push(g.pieces[core].room);
        }
        let specs = specs(theme, &["sanctum"], wide);
        let rooms = target.saturating_sub(1);
        let made = g.grow(&mut p, &specs, rooms.div_ceil(2), true, rng);
        g.grow(&mut p, &specs, rooms.saturating_sub(made + 1), false, rng);
        if last {
            boss = boss_room(&mut p, &mut g, ny, rng);
        }
    }
    // A boss chamber that found no room: the farthest dead end becomes it.
    if last && boss.is_none() {
        boss = dead_end(&p, ny);
        if let Some(b) = boss {
            p.rooms[b as usize].kind = BOSS;
        }
    }
    p.passage = Some((passage, wide));
    p.taken[arrive] = true;
    if !last {
        let d = way_down(&p, ny, &[]);
        p.down = Some(d);
        p.taken[d] = true;
    }
    // Colonnades down the nave and the big halls.
    for r in 0..p.rooms.len() as i16 {
        let Some([_, _, w, h]) = crate::interior::shape::room_box(&p.cells, nx, r) else { continue };
        let (len, wide) = (w.max(h), w.min(h));
        if r == passage || Some(r) == boss || p.rooms[r as usize].shape != 0 || wide < 7 || len < 9 {
            continue;
        }
        colonnade(&mut p, ny, r, (wide - 5) / 2, 2);
        if !halls.contains(&r) {
            halls.push(r);
        }
    }
    if theme.water != Wetness::Dry {
        flood(&mut p, ny, depth, &halls, boss, rng);
    }
    p
}

/// What grows on a temple's levels: the theme's rooms (but `skip`), each as a room of a few shapes, and corridors.
fn specs(theme: &Theme, skip: &[&str], wide: bool) -> Vec<Spec> {
    let mut out = Vec::new();
    for &kind in theme.kinds.iter().filter(|k| !skip.contains(k)) {
        out.push(Spec { kind, t: Template::Rect, across: (3, 5), deep: (3, 5) });
        let t = match kind {
            "ritual pool" | "shrine" => Template::Round,
            "reliquary" | "ossuary" => Template::Octagon,
            "chapel of rest" | "sunken chapel" => Template::Apse,
            _ => Template::Rect,
        };
        out.push(Spec { kind, t, across: (4, 6), deep: (4, 5) });
    }
    let w = if wide { 2 } else { 1 };
    out.push(Spec { kind: theme.passage, t: Template::Corridor, across: (w, w), deep: (3, 6) });
    out.push(Spec { kind: theme.passage, t: Template::Corridor, across: (w, w), deep: (2, 4) });
    out
}

/// A sunken temple's water: deep in the big halls (half of them with dry ledges along their walls), shallow pools
/// elsewhere, more on each level down; the boss chamber dry.
fn flood(p: &mut Plan, ny: usize, depth: usize, halls: &[i16], boss: Option<i16>, rng: &mut Pcg32) {
    let more = 0.05 * depth.min(4) as f64;
    let mut squares = vec![0usize; p.rooms.len()];
    for &c in p.cells.iter().filter(|&&c| c >= 0) {
        squares[c as usize] += 1;
    }
    for r in 0..p.rooms.len() as i16 {
        if Some(r) == boss {
            continue;
        }
        if halls.contains(&r) || squares[r as usize] >= 35 {
            let ledge = rng.next_f64() < 0.5;
            flood_room(p, ny, r, (0.30 - more, 0.42 - more), ledge);
        } else {
            flood_room(p, ny, r, (0.52 - more, 2.0), false);
        }
    }
}
