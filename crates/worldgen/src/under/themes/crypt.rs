//! Crypts, ossuaries and catacombs (`crypt`, `ossuary`, `catacombs`) and barrow tombs (`tomb`), grown room by room
//! (`grow`).
//!
//! Crypts, ossuaries, catacombs: narrow galleries (the passage, one square wide; lined with burial niches on both
//! walls when furnished: `Plan::niches`) meeting at small junction chambers in a rough grid from the way in, a few
//! bigger rooms on the grid (bone chapel, ossuary, charnel pit, a crypt's burial hall at the end of its spine) and more
//! off the galleries (sealed tombs behind hidden doors, burial niches, reliquary, embalming room). Crypts are small
//! and ordered (mirrored on the level's axis); ossuaries bigger; catacombs the biggest grids, laid less regularly
//! (columns and rows a square off, some galleries off their crossings' middles, crossings left out, few loops and
//! secret doors), some crossings bare, and sometimes a flooded gallery (shallow, a little deep in the middle, a wading
//! ledge round it). Mostly dry. Big crypts and huge ossuaries reach the level's edges.
//!
//! Tombs (a barrow, after Dyson's Axehead Mound): a run mirrored on the axis through the way in: the entrance passage,
//! an antechamber, the pillared guardian hall with side chambers in pairs (one a false tomb), a short passage on to
//! the burial chamber at the end (on the deepest level the boss chamber, a dead end, behind a hidden door or an iron
//! grate at times; locks fall where `kit::guard` puts them). Deeper levels go on with the run: a landing, a pillared
//! burial hall with its side chambers, a tomb at the end holding the way down. Bigger barrows have more halls down
//! the run, a second run across each passage between them.

use crate::core::rng::Pcg32;
use crate::under::grow::{Grower, ROOMS_GROWING, ROOMS_MAX, Side, Spec, Template, boss_room, colonnade, dead_end, fallback, fill, flood_room, mask, way_down};
use crate::under::*;

/// A small chamber where galleries meet.
const JUNCTION: &str = "junction";

fn pick(lo: usize, hi: usize, rng: &mut Pcg32) -> usize {
    lo + rng.below((hi.max(lo) - lo + 1) as u32) as usize
}

/// How a theme's galleries are laid: the grid's pitch (squares between crossings), how far it reaches (columns in all,
/// rows either side of the way in; 0 as far as the level goes), crossings kept, extra galleries (loops), crossings
/// with no chamber, mirrored, big rooms on the grid, jittered (columns and rows a square off their places, some
/// galleries a square off their crossings' middles).
struct Grid {
    pitch: usize,
    cols: usize,
    rows: usize,
    keep: f64,
    loops: f64,
    bare: f64,
    mirror: bool,
    bigs: usize,
    jitter: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn crypt(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    p.niches = true;
    let (ax, ay) = (arrive % nx, arrive / nx);
    let big = area >= 1.0;
    // (Big crypts and huge ossuaries: out to the level's edges.)
    let grid = match theme.key {
        "crypt" if area > 1.0 => Grid { pitch: pick(6, 7, rng), cols: 0, rows: 1 + (area >= 2.0) as usize, keep: 1.0, loops: 0.5, bare: 0.0, mirror: true, bigs: 1, jitter: false },
        "crypt" => Grid { pitch: pick(6, 7, rng), cols: 3 + big as usize + depth.min(1), rows: 1, keep: 1.0, loops: 0.5, bare: 0.0, mirror: true, bigs: 1, jitter: false },
        "ossuary" => {
            let edge = area >= 2.0;
            Grid { pitch: pick(6, 7, rng), cols: if edge { 0 } else { 4 + big as usize }, rows: if edge { 0 } else { 1 + big as usize }, keep: 0.9, loops: 0.35, bare: 0.15, mirror: false, bigs: 1 + (area >= 1.3) as usize, jitter: false }
        }
        // (Catacombs: fewer loops, less regular: crossings and galleries a square off, more crossings left out.)
        _ => Grid { pitch: pick(6, 8, rng), cols: 0, rows: 0, keep: 0.8, loops: 0.08, bare: 0.3, mirror: false, bigs: 1 + big as usize + (area >= 1.6) as usize, jitter: true },
    };
    if grid.jitter {
        // (About half the loops `kit::guard` would add, and few secret doors.)
        p.loops = 0.5;
        p.secrets = 2 + (area >= 1.3) as usize;
    }
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(3);
    // The way in: a room round the arrival (the grid's first crossing).
    let s = if depth == 0 && theme.key == "crypt" { 5 } else { 3 };
    let x0 = ax.saturating_sub(s / 2).clamp(1, nx.saturating_sub(s + 1).max(1));
    let y0 = ay.saturating_sub(s / 2).clamp(1, ny.saturating_sub(s + 1).max(1));
    let (cx0, cy0) = (x0 + s / 2, y0 + s / 2);
    let mut g = Grower::new(nx, ny, passage, grid.mirror.then_some(cy0));
    g.cap = ROOMS_GROWING;
    let first = if depth == 0 { theme.first } else { "stair foot" };
    let Some(core) = g.seed(&mut p, first, Template::Rect, x0, y0, s, s).or_else(|| g.seed(&mut p, first, Template::Rect, ax, ay, 1, 1)) else {
        return fallback(p, last);
    };
    let mut junctions = vec![g.pieces[core].room];
    if g.pieces[core].w == s {
        galleries(&mut p, &mut g, &grid, theme, (cx0, cy0, s / 2), &mut junctions, rng);
    }
    // A catacomb level sometimes has a flooded gallery.
    let mut pools = Vec::new();
    if theme.key == "catacombs"
        && rng.next_f64() < 0.35
        && let Some(c) = g.attach(&mut p, &[], "flooded gallery", Template::Rect, (3, pick(8, 12, rng)), 1, rng)
    {
        pools.push(g.pieces[c].room);
    }
    // Then the rooms off the galleries and crossings: in pairs in a crypt, the rest grown freely.
    let specs = specs(theme.key);
    let rooms = |p: &Plan| (0..p.rooms.len()).filter(|&r| r as i16 != passage && p.rooms[r].kind != JUNCTION).count();
    if grid.mirror {
        let want = target.saturating_sub(rooms(&p));
        g.grow(&mut p, &specs, want, true, rng);
    }
    let want = target.saturating_sub(rooms(&p));
    g.grow(&mut p, &specs, want, false, rng);
    // Sealed tombs: most behind a hidden door.
    // (Those behind one are closed to growth: nothing may hang off a tomb only a hidden door leads to.)
    for f in g.pieces.iter_mut().filter(|f| f.room >= 0 && p.rooms[f.room as usize].kind == "sealed tomb") {
        if let Some((a, b)) = f.door
            && rng.next_f64() < 0.6
            && p.secret.len() < p.secrets
        {
            p.secret.insert((a.min(b), a.max(b)));
            f.open = false;
        }
    }
    let mut boss = None;
    if last {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng);
        if boss.is_none() {
            boss = dead_end(&p, ny);
            if let Some(b) = boss {
                p.rooms[b as usize].kind = BOSS;
            }
        }
    }
    p.passage = Some((passage, false));
    p.taken[arrive] = true;
    if !last {
        let skip: Vec<i16> = (0..p.rooms.len() as i16).filter(|&r| matches!(p.rooms[r as usize].kind, JUNCTION | "sealed tomb" | "flooded gallery")).collect();
        let d = way_down(&p, ny, &skip);
        p.down = Some(d);
        p.taken[d] = true;
    }
    pillars(&mut p, ny, passage, boss);
    for r in pools.into_iter().filter(|&r| Some(r) != boss) {
        flood_room(&mut p, ny, r, (-1.0, 0.55), true);
    }
    p
}

/// The galleries: a grid of crossings `grid.pitch` apart round the first one (centre `cx0`, `cy0`, `h0` squares from
/// it to its walls), a random tree of galleries over the crossings kept and some more for loops (mirrored on the
/// axis if asked), each crossing a junction chamber, a bare crossing of the passage or one of the big rooms.
fn galleries(p: &mut Plan, g: &mut Grower, grid: &Grid, theme: &Theme, (cx0, cy0, h0): (usize, usize, usize), junctions: &mut Vec<i16>, rng: &mut Pcg32) {
    let nx = p.nx;
    let ny = p.cells.len() / nx;
    let pp = grid.pitch as isize;
    // Columns and rows of crossings whose junction lies inside the ring of rock round the level.
    let span = |c0: usize, n: usize, limit: usize, both: bool| -> (isize, isize) {
        let (lo, hi) = (-((c0 as isize - 2) / pp), (n as isize - 3 - c0 as isize) / pp);
        if limit == 0 || hi - lo <= limit as isize {
            (lo, hi)
        } else if both {
            (lo.max(-(limit as isize)), hi.min(limit as isize))
        } else if hi >= -lo {
            let l = lo.max(-(limit as isize / 4));
            (l, (l + limit as isize - 1).min(hi))
        } else {
            let h = hi.min(limit as isize / 4);
            ((h - limit as isize + 1).max(lo), h)
        }
    };
    let (c_lo, c_hi) = span(cx0, nx, grid.cols, false);
    let (r_lo, r_hi) = span(cy0, ny, grid.rows, true);
    let (w, h) = ((c_hi - c_lo + 1) as usize, (r_hi - r_lo + 1) as usize);
    let at = |c: isize, r: isize| (r - r_lo) as usize * w + (c - c_lo) as usize;
    // Where each column (row) of crossings lies: `pitch` apart; jittered, each a square off its place or not, the next
    // at most a square further off (neighbours a pitch apart give or take a square), inside the ring of rock.
    let line = |lo: isize, hi: isize, c0: usize, n: usize, rng: &mut Pcg32| -> Vec<usize> {
        let mut o = vec![0isize; (hi - lo + 1) as usize];
        if grid.jitter {
            for dir in [1isize, -1] {
                let (mut prev, mut c) = (0isize, dir);
                while (lo..=hi).contains(&c) {
                    let at0 = c0 as isize + c * pp;
                    let v = (prev + rng.below(3) as isize - 1).clamp(-1, 1).clamp(2 - at0, n as isize - 3 - at0);
                    o[(c - lo) as usize] = v;
                    prev = v;
                    c += dir;
                }
            }
        }
        (lo..=hi).map(|c| (c0 as isize + c * pp + o[(c - lo) as usize]) as usize).collect()
    };
    let xs = line(c_lo, c_hi, cx0, nx, rng);
    let ys = line(r_lo, r_hi, cy0, ny, rng);
    let pos = |(c, r): (isize, isize)| (xs[(c - c_lo) as usize], ys[(r - r_lo) as usize]);
    let inside = |c: isize, r: isize| (c_lo..=c_hi).contains(&c) && (r_lo..=r_hi).contains(&r);
    // (Mirrored: drawn for the rows on and above the axis, copied below.)
    let mirror = |r: isize| if grid.mirror { -r } else { r };
    let kept: Vec<bool> = {
        let mut k = vec![false; w * h];
        for r in r_lo..=r_hi {
            for c in c_lo..=c_hi {
                if grid.mirror && r > 0 {
                    continue;
                }
                let keep = (c, r) == (0, 0) || rng.next_f64() < grid.keep;
                k[at(c, r)] = keep;
                if inside(c, mirror(r)) {
                    k[at(c, mirror(r))] = keep;
                }
            }
        }
        k
    };
    // A random tree from the first crossing, then loops: edges (crossing, its east or south neighbour).
    let mut seen = vec![false; w * h];
    let mut edges: Vec<((isize, isize), (isize, isize))> = Vec::new();
    seen[at(0, 0)] = true;
    let half = |r: isize| !grid.mirror || r <= 0;
    let mut front: Vec<((isize, isize), (isize, isize))> = Vec::new();
    let push = |front: &mut Vec<_>, (c, r): (isize, isize)| {
        for (dc, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            front.push(((c, r), (c + dc, r + dr)));
        }
    };
    push(&mut front, (0, 0));
    while !front.is_empty() {
        let e = front.swap_remove(rng.below(front.len() as u32) as usize);
        let (c, r) = e.1;
        if !inside(c, r) || !half(r) || seen[at(c, r)] {
            continue;
        }
        seen[at(c, r)] = true;
        edges.push(e);
        push(&mut front, (c, r));
    }
    for r in r_lo..=r_hi {
        for c in c_lo..=c_hi {
            for d in [(c + 1, r), (c, r + 1)] {
                let e = ((c, r), d);
                if half(r) && half(d.1) && inside(d.0, d.1) && seen[at(c, r)] && seen[at(d.0, d.1)] && !edges.iter().any(|&(a, b)| (a, b) == e || (b, a) == e) && rng.next_f64() < grid.loops {
                    edges.push(e);
                }
            }
        }
    }
    // Crossings left out at the tree's ends go (again till none is); those it passes through stay, bare.
    loop {
        let mut deg = vec![0usize; w * h];
        for &(a, b) in &edges {
            deg[at(a.0, a.1)] += 1;
            deg[at(b.0, b.1)] += 1;
        }
        let gone: Vec<usize> = (0..w * h).filter(|&k| seen[k] && !kept[k] && deg[k] <= 1 && k != at(0, 0)).collect();
        if gone.is_empty() {
            break;
        }
        for k in gone {
            seen[k] = false;
        }
        edges.retain(|&(a, b)| seen[at(a.0, a.1)] && seen[at(b.0, b.1)]);
    }
    if grid.mirror {
        for r in r_lo..0 {
            for c in c_lo..=c_hi {
                if seen[at(c, r)] && inside(c, -r) {
                    seen[at(c, -r)] = true;
                }
            }
        }
        let below: Vec<_> = edges.iter().filter(|(a, b)| a.1 < 0 || b.1 < 0).map(|&(a, b)| ((a.0, -a.1), (b.0, -b.1))).filter(|&(a, b)| inside(a.0, a.1) && inside(b.0, b.1)).collect();
        edges.extend(below);
    }
    // What stands at each crossing: the way in at the first; the big rooms where no other big room or the way in is
    // next to them (a crypt's at the far end of its spine); the rest junctions or bare.
    let mut bigs: Vec<(isize, isize)> = Vec::new();
    let far = |(c, r): (isize, isize)| c.abs() + r.abs();
    if grid.mirror {
        if let Some(c) = (1..=c_hi).rev().find(|&c| seen[at(c, 0)]) {
            bigs.push((c, 0));
        }
    } else {
        let mut cands: Vec<(isize, isize)> = (r_lo..=r_hi).flat_map(|r| (c_lo..=c_hi).map(move |c| (c, r))).filter(|&q| seen[at(q.0, q.1)] && far(q) >= 2).collect();
        while bigs.len() < grid.bigs && !cands.is_empty() {
            let q = cands.swap_remove(rng.below(cands.len() as u32) as usize);
            if bigs.iter().all(|b| far((b.0 - q.0, b.1 - q.1)) >= 2) {
                bigs.push(q);
            }
        }
    }
    let bigk: &[(&'static str, Template)] = match theme.key {
        "crypt" => &[("burial hall", Template::Rect), ("chapel of rest", Template::Rect)],
        "ossuary" => &[("ossuary", Template::Octagon), ("bone chapel", Template::Cross), ("charnel pit", Template::Round)],
        _ => &[("ossuary", Template::Octagon), ("bone chapel", Template::Cross), ("charnel pit", Template::Round), ("reliquary", Template::Octagon)],
    };
    // The crossings' boxes (half sizes) and pieces.
    let mut halfs = vec![0usize; w * h];
    let mut piece = vec![usize::MAX; w * h];
    halfs[at(0, 0)] = h0;
    piece[at(0, 0)] = 0;
    let bare: Vec<bool> = (0..w * h).map(|_| rng.next_f64() < grid.bare).collect();
    // (Square-cornered crossings: a gallery may meet them a square off their middle.)
    let mut junc = vec![false; w * h];
    junc[at(0, 0)] = true;
    let big_draws: Vec<usize> = (0..bigs.len()).map(|_| rng.below(bigk.len() as u32) as usize).collect();
    for r in r_lo..=r_hi {
        for c in c_lo..=c_hi {
            let k = at(c, r);
            if !seen[k] || (c, r) == (0, 0) {
                continue;
            }
            let (cx, cy) = pos((c, r));
            let mirrored = grid.mirror && r > 0 && inside(c, -r);
            let (kind, t, hb) = if let Some(b) = bigs.iter().position(|&q| q == (c, r)) {
                let (kind, t) = bigk[big_draws[b]];
                // (As big as the pitch and the ring of rock round the level allow.)
                let room = [cx - 1, cy - 1, p.nx - 2 - cx, ny - 2 - cy].into_iter().min().unwrap_or(0);
                (kind, t, (grid.pitch - 4).min(3).min(room))
            } else if bare[if mirrored { at(c, -r) } else { k }] || !kept[k] || p.rooms.len() + 12 >= ROOMS_GROWING {
                ("", Template::Corridor, 0)
            } else {
                (JUNCTION, Template::Rect, 1)
            };
            let n = 2 * hb + 1;
            // (A shape that leaves a side's middle square out is drawn square: the galleries meet it there.)
            let (m, _) = mask(t, cx - hb, cy - hb, n, n, Side::E);
            let t = if [hb, n * hb, n * hb + n - 1, n * n - 1 - hb].iter().all(|&q| m[q]) { t } else { Template::Rect };
            let Some(i) = g.seed(p, kind, t, cx - hb, cy - hb, n, n) else { continue };
            piece[k] = i;
            halfs[k] = hb;
            if kind == JUNCTION {
                junctions.push(g.pieces[i].room);
                junc[k] = true;
            }
        }
    }
    // The galleries between them, a door where one meets a room.
    for &(a, b) in &edges {
        let (ka, kb) = (at(a.0, a.1), at(b.0, b.1));
        if piece[ka] == usize::MAX || piece[kb] == usize::MAX {
            continue;
        }
        // (Its west or north end first.)
        let ((ka, a), (kb, b)) = if a < b { ((ka, a), (kb, b)) } else { ((kb, b), (ka, a)) };
        let ((cx, cy), (bx, by)) = (pos(a), pos(b));
        let across = b.0 == a.0 + 1;
        let (ha, hb) = (halfs[ka], halfs[kb]);
        let len = if across { bx - cx } else { by - cy } - ha - hb - 1;
        // (Jittered: some galleries between two junctions a square off their middles.)
        let off = if grid.jitter && junc[ka] && junc[kb] && ha >= 1 && hb >= 1 && rng.next_f64() < 0.35 { if rng.next_f64() < 0.5 { 1 } else { usize::MAX } } else { 0 };
        let (x, y, gw, gh) = if across { (cx + ha + 1, cy.wrapping_add(off), len, 1) } else { (cx.wrapping_add(off), cy + ha + 1, 1, len) };
        if len == 0 || g.seed(p, "", Template::Corridor, x, y, gw, gh).is_none() {
            continue;
        }
        let (first, end) = if across { (y * nx + x, y * nx + x + len - 1) } else { (y * nx + x, (y + len - 1) * nx + x) };
        let (before, after) = if across { (first - 1, end + 1) } else { (first - nx, end + nx) };
        for (q, e) in [(before, first), (after, end)] {
            if p.cells[q] >= 0 && p.cells[q] != g.passage {
                p.door(q, e);
            }
        }
    }
}

/// What grows off a gallery level's galleries and crossings, by theme.
fn specs(key: &str) -> Vec<Spec> {
    use Template::*;
    let s = |kind, t, across, deep| Spec { kind, t, across, deep };
    let mut out = match key {
        "crypt" => vec![
            s("burial hall", Rect, (5, 7), (4, 6)),
            s("tomb", Rect, (3, 4), (3, 4)),
            s("sealed tomb", Rect, (3, 3), (3, 4)),
            s("chapel of rest", Apse, (5, 5), (4, 6)),
            s("embalming room", Rect, (3, 5), (3, 4)),
            s("reliquary", Octagon, (5, 5), (5, 5)),
            s("ossuary", Octagon, (5, 5), (5, 5)),
        ],
        "ossuary" => vec![
            s("ossuary", Octagon, (5, 7), (5, 7)),
            s("ossuary", Rect, (3, 5), (3, 5)),
            s("bone chapel", Apse, (5, 5), (5, 7)),
            s("charnel pit", Round, (5, 5), (5, 5)),
            s("skull gallery", Rect, (3, 3), (6, 9)),
            s("burial niches", Rect, (3, 4), (3, 5)),
        ],
        _ => vec![
            s("burial niches", Rect, (3, 4), (3, 6)),
            s("burial niches", Rect, (3, 4), (3, 6)),
            s("ossuary", Octagon, (5, 5), (5, 5)),
            s("catacomb gallery", Rect, (3, 3), (6, 10)),
            s("charnel pit", Round, (5, 5), (5, 5)),
            s("bone chapel", Apse, (5, 5), (5, 6)),
            s("reliquary", Octagon, (5, 5), (5, 5)),
            s("sealed tomb", Rect, (3, 3), (3, 3)),
        ],
    };
    out.push(s("", Corridor, (1, 1), (2, 4)));
    out
}

/// Pillars down the big square-cornered rooms (not the passage, junctions nor the boss chamber).
fn pillars(p: &mut Plan, ny: usize, passage: i16, boss: Option<i16>) {
    for r in 0..p.rooms.len() as i16 {
        let Some([_, _, w, h]) = crate::interior::shape::room_box(&p.cells, p.nx, r) else { continue };
        let (len, wide) = (w.max(h), w.min(h));
        if r == passage || Some(r) == boss || p.rooms[r as usize].shape != 0 || p.rooms[r as usize].kind == JUNCTION || wide < 5 || len < 7 {
            continue;
        }
        colonnade(p, ny, r, if wide < 7 { 1 } else { (wide - 5) / 2 }, 2);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn tomb(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    let (ax, ay) = (arrive % nx, arrive / nx);
    let odd = |n: usize| n | 1;
    let sc = |n: usize| ((n as f64) * area.sqrt()).round() as usize;
    // The run goes the way there is more room.
    let fwd = if ax < nx / 2 { Side::E } else { Side::W };
    let mut g = Grower::new(nx, ny, passage, Some(ay));
    g.cap = ROOMS_GROWING;
    g.spread = true;
    let start = if depth == 0 {
        let len = pick(3, 5, rng);
        let x0 = if fwd == Side::E { ax.max(1) } else { (ax + 1).saturating_sub(len).max(1) };
        g.seed(&mut p, "", Template::Corridor, x0, ay, len, 1)
    } else {
        let x0 = ax.saturating_sub(1).clamp(1, nx.saturating_sub(4).max(1));
        let y0 = ay.saturating_sub(1).clamp(1, ny.saturating_sub(4).max(1));
        g.axis = Some(y0 + 1);
        g.seed(&mut p, "landing", Template::Rect, x0, y0, 3, 3)
    };
    let Some(start) = start.or_else(|| g.seed(&mut p, "landing", Template::Rect, ax, ay, 1, 1)) else { return fallback(p, last) };
    // Down the run: the first that fits of each room's sizes.
    let run = |p: &mut Plan, g: &mut Grower, from: usize, kind: &'static str, t: Template, sizes: &[(usize, usize)]| -> Option<usize> {
        sizes.iter().find_map(|&(a, d)| g.deck(p, from, fwd, kind, t, a, d, 0, true).first().copied())
    };
    let mut at = start;
    let mut sides: Vec<(i16, usize)> = Vec::new();
    if depth == 0
        && let Some(a) = run(&mut p, &mut g, at, "antechamber", Template::Rect, &[(odd(pick(3, 5, rng)), pick(3, 4, rng)), (3, 3)])
    {
        at = a;
    }
    // The halls down the run: one, more on bigger sites (a short passage between).
    let many = 1 + (area > 1.0) as usize + (area >= 2.0) as usize;
    let mut leads = Vec::new();
    for h in 0..many {
        let mut lead = None;
        if h > 0 {
            let Some(&c) = g.deck(&mut p, at, fwd, "", Template::Corridor, 1, pick(2, 3, rng), 0, true).first() else { break };
            lead = Some(c);
        }
        let hall_kind = if depth == 0 && h == 0 { "guardian hall" } else { "burial hall" };
        let (hw, hd) = (odd(pick(5, sc(7).max(5), rng)), pick(6, sc(10).max(6), rng));
        let from = lead.unwrap_or(at);
        let Some(hall) = run(&mut p, &mut g, from, hall_kind, Template::Rect, &[(hw, hd), (5, 6), (5, 5)]) else {
            if lead.is_some() {
                g.undo_last(&mut p);
            }
            break;
        };
        if let Some(c) = lead {
            g.pieces[c].open = false;
            leads.push(c);
        }
        at = hall;
        // Side chambers in pairs off the hall's long walls (the antechamber's too on the top level).
        let f = g.pieces[hall];
        let pairs = (f.w.max(f.h) / 5).clamp(1, 2 + (area > 1.0) as usize);
        for c in 0..pairs {
            let along = ((c + 1) * f.w / (pairs + 1)) as isize - (f.w / 2) as isize;
            let kind = ["tomb", "treasury", "embalming room"][rng.below(3) as usize];
            let (a, d) = (pick(3, 4, rng), pick(3, sc(4).max(3), rng));
            for i in g.deck(&mut p, hall, Side::N, kind, Template::Rect, a, d, along, true) {
                g.pieces[i].open = false;
                sides.push((g.pieces[i].room, i));
            }
        }
    }
    // Big sites: a second run across each passage between halls, a passage either way to a burial hall (more grows
    // off them).
    for c in leads {
        let Some(&n) = g.deck(&mut p, c, Side::N, "", Template::Corridor, 1, pick(3, 5, rng), 0, true).first() else { continue };
        let laid = g.pieces.len();
        let (a, d) = (odd(pick(5, sc(7).max(5), rng)), pick(5, sc(6).max(5), rng));
        if [(a, d), (5, 5), (3, 4)].iter().all(|&(a, d)| g.deck(&mut p, n, Side::N, "burial hall", Template::Rect, a, d, 0, true).is_empty()) {
            while g.pieces.len() > laid - 2 {
                g.undo_last(&mut p);
            }
        }
    }
    // One of the side chambers a false tomb.
    if !sides.is_empty() {
        let (r, _) = sides[rng.below(sides.len() as u32) as usize];
        p.rooms[r as usize].kind = "false tomb";
    }
    // On to the burial chamber (the boss chamber on the deepest level; above, a tomb holding the way down).
    let lead = g.deck(&mut p, at, fwd, "", Template::Corridor, 1, pick(2, 3, rng), 0, true);
    if let Some(&c) = lead.first() {
        g.pieces[c].open = false;
        at = c;
    }
    let kind = if last { BOSS } else { "tomb" };
    let (ba, bd) = (odd(pick(5, sc(7).max(5), rng)), pick(5, sc(7).max(5), rng));
    let mut end = None;
    // (Above the deepest, half the time a plain hall rather than an apse, so levels differ.)
    let plain = !last && rng.next_f64() < 0.5;
    let ends = if plain { [(Template::Rect, ba, bd), (Template::Apse, 5, 5), (Template::Rect, 3, 3)] } else { [(Template::Apse, ba, bd), (Template::Apse, 5, 5), (Template::Rect, 3, 3)] };
    for (t, a, d) in ends {
        if let Some(&e) = g.deck(&mut p, at, fwd, kind, t, a, d, 0, true).first() {
            g.pieces[e].open = false;
            end = Some(e);
            break;
        }
    }
    let mut boss = None;
    if let Some(e) = end {
        if last {
            boss = Some(g.pieces[e].room);
        }
        // Sealed behind a hidden door, or an iron grate at times.
        if let Some((a, b)) = g.pieces[e].door {
            let r = rng.next_f64();
            if last && r < 0.5 {
                p.secret.insert((a.min(b), a.max(b)));
            } else if r < if last { 0.75 } else { 0.28 } {
                p.portcullis.insert((a.min(b), a.max(b)));
            }
        }
    }
    // More burial chambers in pairs, to the theme's count.
    let have = (0..p.rooms.len()).filter(|&r| r as i16 != passage).count();
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(4);
    let specs = [
        Spec { kind: "tomb", t: Template::Rect, across: (3, 4), deep: (3, 4) },
        Spec { kind: "sealed tomb", t: Template::Rect, across: (3, 3), deep: (3, 3) },
        Spec { kind: "treasury", t: Template::Rect, across: (3, 4), deep: (3, 3) },
        Spec { kind: "embalming room", t: Template::Rect, across: (3, 5), deep: (3, 4) },
        Spec { kind: "", t: Template::Corridor, across: (1, 1), deep: (2, 3) },
    ];
    g.grow(&mut p, &specs, target.saturating_sub(have), true, rng);
    if last && boss.is_none() {
        g.cap = ROOMS_MAX;
        boss = boss_room(&mut p, &mut g, ny, rng).or_else(|| {
            let b = dead_end(&p, ny)?;
            p.rooms[b as usize].kind = BOSS;
            Some(b)
        });
    }
    p.passage = Some((passage, false));
    p.taken[arrive] = true;
    if !last {
        // The way down in the tomb at the end of the run (else the farthest room).
        let skip: Vec<i16> = match end {
            Some(e) => (0..p.rooms.len() as i16).filter(|&r| r != g.pieces[e].room).collect(),
            None => Vec::new(),
        };
        let d = way_down(&p, ny, &skip);
        p.down = Some(d);
        p.taken[d] = true;
    }
    pillars(&mut p, ny, passage, boss);
    p
}
