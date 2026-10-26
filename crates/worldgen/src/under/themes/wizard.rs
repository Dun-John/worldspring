//! Wizards' lairs (`wizard_lair`), a rotunda and spokes, grown room by room (`grow`): a round stair hall at the heart
//! of each level (on the top level down a short hall from the foyer at the way in; below, round the way down from
//! above), the stairs down beside its middle; spokes off it, mirrored on the level's axis, short halls to round or
//! octagon rooms (library, laboratory, scrying room, specimen vault, study, workshop, menagerie), more rooms grown on
//! from them, some in mirrored pairs. A secret door or two between neighbouring spokes. On the deepest level the
//! summoning circle (the boss chamber) opens straight off the stair hall on the axis, a dead end.

use crate::core::rng::Pcg32;
use crate::under::grow::{ROOMS_MAX, Side, Spec, Template, boss_room, dead_end, fallback, fill, first_room, rooms};
use crate::under::*;

const STAIR_HALL: &str = "stair hall";
const SPOKES: [&str; 7] = ["library", "laboratory", "scrying room", "specimen vault", "study", "golem workshop", "menagerie"];

#[allow(clippy::too_many_arguments)]
pub(crate) fn wizard(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let pick = |lo: usize, hi: usize, rng: &mut Pcg32| lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize;
    let odd = |n: usize| n | 1;
    let sc = |n: usize| ((n as f64) * crate::core::sqrt(area)).round() as usize;
    let cw = theme.corridor;
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(4);
    p.secrets = 2 + (area >= 1.3) as usize;
    let dir = if arrive % nx < nx / 2 { Side::E } else { Side::W };
    let back = if dir == Side::E { Side::W } else { Side::E };
    let r = odd(pick(7, sc(9).max(7), rng));
    // The stair hall: down a short hall from the foyer on the top level, round the way down from above below.
    let (mut g, rot, round, turned) = if depth == 0 {
        let (fw, fh) = (pick(3, 4, rng), odd(pick(3, 5, rng)));
        let Some((mut g, foyer)) = first_room(&mut p, passage, theme.first, Template::Rect, (fw, fh), Some(dir), 1) else { return fallback(p, last) };
        // (The stair hall towards the grid's middle: down a hall north or south to it, else along to it.)
        let mut rot = Some(g.to_middle(&mut p, foyer, cw, (STAIR_HALL, Template::Round, r), ny / 6)).filter(|&c| c != foyer);
        let mut from = foyer;
        let f = g.pieces[foyer];
        let to_mid = if dir == Side::E { (nx / 2).saturating_sub(f.x + f.w + r / 2) } else { (f.x).saturating_sub(nx / 2 + r / 2) };
        if rot.is_none()
            && let Some(&c) = g.deck(&mut p, foyer, dir, "", Template::Corridor, cw, to_mid.clamp(2, 4 + nx / 6), 0, true).first()
        {
            from = c;
        }
        for (t, s) in [(Template::Round, r), (Template::Round, 7), (Template::Octagon, 5), (Template::Rect, 5)] {
            if rot.is_some() {
                break;
            }
            if let Some(&c) = g.deck(&mut p, from, dir, STAIR_HALL, t, s, s, 0, true).first() {
                rot = Some(c);
                break;
            }
        }
        let turned = rot.is_some() && from == foyer;
        match rot {
            Some(c) => (g, c, true, turned),
            None => (g, foyer, false, false),
        }
    } else {
        let Some((g, c)) = first_room(&mut p, passage, STAIR_HALL, Template::Round, (r, r), None, 0) else { return fallback(p, last) };
        let round = g.pieces[c].w >= 5;
        (g, c, round, false)
    };
    let rot_room = g.pieces[rot].room;
    // The deepest level: the summoning circle off the stair hall, on the axis if it fits.
    let mut boss = None;
    let mut sides = vec![Side::N, dir];
    // (Below, and where the stair hall was reached from the north or south, a spoke back the other way too.)
    if depth > 0 || turned {
        sides.push(back);
    }
    if last {
        let s = odd(pick(9, sc(11).max(9), rng));
        'boss: for side in [dir, back, Side::N, Side::S] {
            for (t, s) in [(Template::Round, s), (Template::Round, 9), (Template::Octagon, 7), (Template::Rect, 5)] {
                if let Some(&b) = g.deck(&mut p, rot, side, BOSS, t, s, s, 0, !matches!(side, Side::N | Side::S)).first() {
                    g.pieces[b].open = false;
                    boss = Some(g.pieces[b].room);
                    sides.retain(|&q| q != side && !(side == Side::S && q == Side::N));
                    break 'boss;
                }
            }
        }
    }
    // The spokes: a short hall out of each side to a round or octagon room (north and south in a mirrored pair).
    for side in sides {
        let halls = g.deck(&mut p, rot, side, "", Template::Corridor, cw, pick(2, 3, rng), 0, true);
        let Some(&h) = halls.first() else { continue };
        let t = if rng.next_f64() < 0.5 { Template::Round } else { Template::Octagon };
        let k0 = rng.below(SPOKES.len() as u32) as usize;
        let k1 = (k0 + 1 + rng.below(SPOKES.len() as u32 - 1) as usize) % SPOKES.len();
        let mut put = Vec::new();
        for s in [odd(pick(5, sc(7).max(5), rng)), 5] {
            put = g.deck(&mut p, h, side, SPOKES[k0], t, s, s, 0, true);
            if !put.is_empty() {
                break;
            }
        }
        if put.is_empty() {
            for _ in 0..halls.len() {
                g.undo_last(&mut p);
            }
            continue;
        }
        for &c in &halls {
            g.pieces[c].open = false;
        }
        if let Some(&twin) = put.get(1) {
            p.rooms[g.pieces[twin].room as usize].kind = SPOKES[k1];
        }
    }
    // Then more rooms: half in mirrored pairs, the rest freely.
    let specs = specs(theme, cw, area);
    let left = target.saturating_sub(rooms(&p, passage));
    let made = g.grow(&mut p, &specs, left.div_ceil(2), true, rng);
    g.grow(&mut p, &specs, left.saturating_sub(made), false, rng);
    if last && boss.is_none() {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng);
        if boss.is_none() {
            boss = dead_end(&p, ny);
            if let Some(b) = boss {
                p.rooms[b as usize].kind = BOSS;
            }
        }
    }
    p.passage = Some((passage, cw > 1));
    p.taken[arrive] = true;
    // A secret door or two between neighbouring spokes.
    spoke_secrets(&mut p, ny, rot_room, passage, boss, 1 + (area >= 1.3) as usize, rng);
    if !last {
        // The stairs down at the stair hall's middle (beside the way down from above), else the farthest room.
        let f = g.pieces[rot];
        let mid = (f.y + f.h / 2) * nx + f.x + f.w / 2;
        let doors: Vec<usize> = p.doors.iter().flat_map(|&(a, b)| [a, b]).collect();
        let ok = |k: usize| p.cells[k] == rot_room && k != arrive && !doors.contains(&k);
        let near = [mid, mid + 1, mid - 1, mid + nx, mid - nx];
        let d = near.into_iter().filter(|_| round).find(|&k| ok(k)).unwrap_or_else(|| crate::under::grow::way_down(&p, ny, &[]));
        p.down = Some(d);
        p.taken[d] = true;
    }
    p
}

/// What grows round the spokes: the theme's rooms (the summoning circle is the deepest level's), small and square or
/// in a shape that suits them; short halls.
fn specs(theme: &Theme, cw: usize, area: f64) -> Vec<Spec> {
    let up = |n: usize| ((n as f64) * crate::core::sqrt(area).max(1.0)).round() as usize;
    let mut out = Vec::new();
    for &kind in theme.kinds.iter().filter(|&&k| k != "summoning circle") {
        out.push(Spec { kind, t: Template::Rect, across: (3, up(5)), deep: (3, up(5)) });
        let t = match kind {
            "scrying room" | "laboratory" => Template::Round,
            "library" | "specimen vault" => Template::Octagon,
            _ => Template::Rect,
        };
        out.push(Spec { kind, t, across: (5, up(7)), deep: (5, up(7)) });
    }
    out.push(Spec { kind: "", t: Template::Corridor, across: (cw, cw), deep: (2, 3) });
    out.push(Spec { kind: "", t: Template::Corridor, across: (cw, cw), deep: (2, 4) });
    out
}

/// Up to `want` secret doors (within the level's cap) through walls rooms of different spokes share: the spokes are
/// what one reaches from each side of the stair hall without crossing it; never the passage's, the boss chamber's, nor
/// the stair hall's walls; each at the middle of the wall two rooms share.
fn spoke_secrets(p: &mut Plan, ny: usize, hub: i16, passage: i16, boss: Option<i16>, want: usize, rng: &mut Pcg32) {
    let nx = p.nx;
    let n = p.cells.len();
    let mut spoke = vec![usize::MAX; n];
    let mut next = 0;
    for s in 0..n {
        if p.cells[s] < 0 || p.cells[s] == hub || spoke[s] != usize::MAX {
            continue;
        }
        spoke[s] = next;
        let mut stack = vec![s];
        while let Some(k) = stack.pop() {
            for m in neighbours(k, nx, ny) {
                if spoke[m] == usize::MAX && p.cells[m] != hub && p.passable(k, m) {
                    spoke[m] = next;
                    stack.push(m);
                }
            }
        }
        next += 1;
    }
    let skip = |r: i16| r < 0 || r == hub || r == passage || Some(r) == boss;
    // Shared walls by room pair, in scan order.
    let mut walls: Vec<((i16, i16), Vec<(usize, usize)>)> = Vec::new();
    for k in 0..n {
        for m in [k + 1, k + nx] {
            if m >= n || (m == k + 1 && m % nx == 0) {
                continue;
            }
            let (a, b) = (p.cells[k], p.cells[m]);
            if a == b || skip(a) || skip(b) || spoke[k] == spoke[m] || p.doors.contains(&(k, m)) {
                continue;
            }
            let key = (a.min(b), a.max(b));
            match walls.iter_mut().find(|w| w.0 == key) {
                Some(w) => w.1.push((k, m)),
                None => walls.push((key, vec![(k, m)])),
            }
        }
    }
    let mut joined: Vec<(usize, usize)> = Vec::new();
    for _ in 0..want {
        if walls.is_empty() || p.secret.len() >= p.secrets {
            break;
        }
        let i = rng.below(walls.len() as u32) as usize;
        let (_, run) = walls.swap_remove(i);
        let (k, m) = run[run.len() / 2];
        let pair = (spoke[k].min(spoke[m]), spoke[k].max(spoke[m]));
        if joined.contains(&pair) {
            continue;
        }
        joined.push(pair);
        p.door(k, m);
        p.secret.insert((k, m));
    }
    // (No walls shared, as on small sites: a secret passage through the rock between the two nearest rooms of
    // different spokes, if they lie within a few squares.)
    if joined.is_empty() && p.secret.len() < p.secrets {
        let mut squares: Vec<Vec<usize>> = vec![Vec::new(); p.rooms.len()];
        for k in 0..n {
            if !skip(p.cells[k]) {
                squares[p.cells[k] as usize].push(k);
            }
        }
        let gap = |a: usize, b: usize| (a % nx).abs_diff(b % nx) + (a / nx).abs_diff(b / nx);
        let mut best: Option<(usize, i16, i16)> = None;
        for a in 0..squares.len() {
            for b in a + 1..squares.len() {
                let (Some(&ka), Some(&kb)) = (squares[a].first(), squares[b].first()) else { continue };
                if spoke[ka] == spoke[kb] {
                    continue;
                }
                let d = squares[a].iter().flat_map(|&x| squares[b].iter().map(move |&y| gap(x, y))).min().unwrap_or(usize::MAX);
                if d <= 12 && best.is_none_or(|q| d < q.0) {
                    best = Some((d, a as i16, b as i16));
                }
            }
        }
        if let Some((_, a, b)) = best {
            let before = p.doors.clone();
            let dug: Vec<(usize, usize)> = dig(p, nx, ny, a, b, passage, false).into_iter().filter(|e| !before.contains(e)).collect();
            if let Some(&e) = dug.first() {
                p.secret.insert(e);
            }
        }
    }
}
