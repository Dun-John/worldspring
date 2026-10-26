//! Cult sanctums (`cult_sanctum`): rings round a ritual chamber. Each level has a round chamber at its heart and ring
//! passages round it (rectangular loops a square wide) with rooms between them (cells, dormitories, the robing room,
//! the sacrifice pit, shrines, stores), each opening on one ring only; each ring's gate (a short passage across the
//! rooms between) is set off from the next one's, so the way in winds round. A few rooms grow off the outer ring. The
//! top level looks like a plain cellar (or a crypt's front) round the way in, a secret door into the cult; on the
//! deepest level the chamber is the boss chamber, a dead end; on the levels above, the stairs down lie elsewhere.

use crate::core::rng::Pcg32;
use crate::under::grow::{Grower, Piece, ROOMS_GROWING, Side, Spec, Template, fill, first_room, mask, rooms, way_down};
use crate::under::themes::keep::CELL;
use crate::under::*;

/// The chamber at a level's heart (the boss chamber on the deepest).
const CHAMBER_KIND: &str = "ritual chamber";

/// Unit steps by side: north, east, south, west.
const DIRS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Rooms the rings may hold (some left for the locks' split passages and the rooms grown off them).
const RING_ROOMS: usize = ROOMS_GROWING - 8;

/// Squares not to grow into (round a part kept apart while another grows): rock for `deck`, not for `dig`.
const HALO: i16 = i16::MAX;

#[allow(clippy::too_many_arguments)]
pub(crate) fn cult(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = p.room(theme.passage, 0.0);
    p.secrets = 2 + (area >= 1.3) as usize;
    // (Few loops: they'd cut the winding way in short; none on the top level, round its secret door.)
    p.loops = if depth == 0 { 0.0 } else { 0.34 };
    let (ax, ay) = ((arrive % nx) as isize, (arrive / nx) as isize);
    let (nxi, nyi) = (nx as isize, ny as isize);
    let s: isize = if area >= 1.3 { 9 } else { 7 };
    let h = s / 2;
    let (bx, by) = (3 + rng.below(2 + (area >= 1.3) as u32) as isize, 3 + rng.below(2) as isize);
    // Ring k's half-sizes (from the centre to the ring, across x and y).
    let half = |k: isize| (h + 1 + k * (bx + 1), h + 1 + k * (by + 1));
    let most = 2 + (area >= 1.0) as isize;
    // Where it goes: the most rings that fit with the arrival outside them (on the top level with room for the cellar),
    // else on the levels below among them (off the chamber), the centre as near the grid's middle as that allows.
    let tries: Vec<(isize, bool)> = if depth == 0 { (1..=most).rev().map(|n| (n, true)).collect() } else { vec![(most, true), (most, false), (most - 1, true), (most - 1, false), (1, false)] };
    let mut at = None;
    'fit: for (n, strict) in tries.into_iter().filter(|t| t.0 >= 1) {
        let (hx, hy) = half(n - 1);
        if 2 * hx + 3 > nxi || 2 * hy + 3 > nyi {
            continue;
        }
        let (lo, hi) = ((hx + 1, hy + 1), (nxi - 2 - hx, nyi - 2 - hy));
        let mid = ((nxi / 2).clamp(lo.0, hi.0), (nyi / 2).clamp(lo.1, hi.1));
        let far = (if ax < nxi / 2 { hi.0 } else { lo.0 }, if ay < nyi / 2 { hi.1 } else { lo.1 });
        for c in [mid, (far.0, mid.1), (mid.0, far.1), far] {
            let (ox, oy) = ((ax - c.0).abs(), (ay - c.1).abs());
            let fine = match (depth, strict) {
                (0, _) => ox > hx + 6 || oy > hy + 6,
                (_, true) => ox > hx + 3 || oy > hy + 3,
                _ => ox > h + 1 || oy > h + 1,
            };
            if fine {
                at = Some((n, c));
                break 'fit;
            }
        }
    }
    let Some((n, (cx, cy))) = at else { return mixed(nx, ny, arrive, depth, last, theme, area, rng) };
    let sq = |x: isize, y: isize| y as usize * nx + x as usize;
    // The rings.
    for k in 0..n {
        let (hx, hy) = half(k);
        for x in -hx..=hx {
            p.cells[sq(cx + x, cy - hy)] = passage;
            p.cells[sq(cx + x, cy + hy)] = passage;
        }
        for y in -hy..=hy {
            p.cells[sq(cx - hx, cy + y)] = passage;
            p.cells[sq(cx + hx, cy + y)] = passage;
        }
    }
    // The chamber.
    let (m, code) = mask(Template::Round, (cx - h) as usize, (cy - h) as usize, s as usize, s as usize, Side::E);
    let chamber = p.room(if last { BOSS } else { CHAMBER_KIND }, 0.0);
    p.rooms[chamber as usize].shape = code;
    for k in (0..(s * s) as usize).filter(|&k| m[k]) {
        p.cells[sq(cx - h + (k as isize % s), cy - h + (k as isize / s))] = chamber;
    }
    // The gates, from the side facing the arrival inwards: each on the far side of the ring from the last (now and then
    // a quarter round), the chamber's door opposite the last.
    let (hx, hy) = half(n - 1);
    let mut side = if (ax - cx).abs() - hx >= (ay - cy).abs() - hy { if ax > cx { 1 } else { 3 } } else if ay > cy { 2 } else { 0 };
    let mut gates = vec![(0usize, 0isize); n as usize];
    for k in (0..n - 1).rev() {
        side = if rng.next_f64() < 0.7 { (side + 2) % 4 } else { (side + 1 + 2 * rng.below(2) as usize) % 4 };
        let (ix, iy) = half(k);
        let reach = (if side % 2 == 0 { ix } else { iy }) / 2;
        gates[k as usize] = (side, rng.below(2 * reach as u32 + 1) as isize - reach);
    }
    let cs = (side + 2) % 4;
    let (r0, _) = half(0);
    let (dx, dy) = DIRS[cs];
    p.door(sq(cx + dx * r0, cy + dy * r0), sq(cx + dx * h, cy + dy * h));
    // The rooms between the rings, each strip cut into rooms (a cell two squares wide, others three to five, longer
    // farther out) round its gate, a corner block a room; each opening on the inner ring or the outer.
    let mut band: Vec<(i16, isize, usize)> = Vec::new();
    let mut doors: Vec<(i16, usize, usize, usize, usize)> = Vec::new();
    for k in 0..n - 1 {
        let ((ix, iy), (ox, oy)) = (half(k), half(k + 1));
        for side in 0..4 {
            let (dx, dy) = DIRS[side];
            let (along, deep) = if side % 2 == 0 { (ix, by) } else { (iy, bx) };
            let (inner, outer) = if side % 2 == 0 { (iy, oy) } else { (ix, ox) };
            // Strip square at (t along, d out from the inner ring); the along axis runs across the side.
            let at = |t: isize, d: isize| if side % 2 == 0 { sq(cx + t, cy + dy * (inner + 1 + d)) } else { sq(cx + dx * (inner + 1 + d), cy + t) };
            let ring = |t: isize, outward: bool| if side % 2 == 0 { sq(cx + t, cy + dy * if outward { outer } else { inner }) } else { sq(cx + dx * if outward { outer } else { inner }, cy + t) };
            let gate = (gates[k as usize].0 == side).then_some(gates[k as usize].1);
            if let Some(t) = gate {
                for d in 0..deep {
                    p.cells[at(t, d)] = passage;
                }
            }
            let runs: Vec<(isize, isize)> = match gate {
                Some(t) => vec![(-along, t - 1), (t + 1, along)],
                None => vec![(-along, along)],
            };
            for (a, b) in runs.into_iter().filter(|r| r.0 <= r.1) {
                let mut t = a;
                while t <= b {
                    let left = b - t + 1;
                    // (Longer farther out; cells only in the inner bands.)
                    let mut len = if k < 2 && rng.next_f64() < 0.3 { 2 } else { 3 + k + rng.below(3 + k as u32) as isize };
                    if left - len < 2 || p.rooms.len() >= RING_ROOMS {
                        len = left;
                    }
                    let r = p.room(if len == 2 { CELL } else { "dormitory" }, 0.0);
                    for u in t..t + len {
                        for d in 0..deep {
                            p.cells[at(u, d)] = r;
                        }
                    }
                    let mid = t + len / 2;
                    let out = rng.next_f64() < 0.5;
                    let (a, b) = if out { (at(mid, deep - 1), ring(mid, true)) } else { (at(mid, 0), ring(mid, false)) };
                    let (a2, b2) = if out { (at(mid, 0), ring(mid, false)) } else { (at(mid, deep - 1), ring(mid, true)) };
                    p.door(a, b);
                    band.push((r, k, (len * deep) as usize));
                    doors.push((r, a, b, a2, b2));
                    t += len;
                }
            }
            // The corner block clockwise of this strip: a room opening on the outer ring.
            let (ex, ey) = (DIRS[(side + 1) % 4].0 + dx, DIRS[(side + 1) % 4].1 + dy);
            let (x0, x1) = if ex > 0 { (cx + ix + 1, cx + ox - 1) } else { (cx - ox + 1, cx - ix - 1) };
            let (y0, y1) = if ey > 0 { (cy + iy + 1, cy + oy - 1) } else { (cy - oy + 1, cy - iy - 1) };
            let r = p.room("storeroom", 0.0);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    p.cells[sq(x, y)] = r;
                }
            }
            let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
            let (a, b) = if side % 2 == 0 { (sq(mx, if dy < 0 { y0 } else { y1 }), sq(mx, cy + dy * oy)) } else { (sq(if dx < 0 { x0 } else { x1 }, my), sq(cx + dx * ox, my)) };
            p.door(a, b);
            band.push((r, k, ((x1 - x0 + 1) * (y1 - y0 + 1)) as usize));
        }
    }
    // Their kinds: by the chamber the sacrifice pit (the biggest there) and the robing room; dormitories, stores and
    // shrines farther out; cells where narrow.
    let mut inner: Vec<(usize, i16)> = band.iter().filter(|b| b.1 == 0 && p.rooms[b.0 as usize].kind != CELL).map(|b| (b.2, b.0)).collect();
    inner.sort_by_key(|&(a, r)| (std::cmp::Reverse(a), r));
    let rest: Vec<i16> = band.iter().filter(|b| p.rooms[b.0 as usize].kind != CELL).map(|b| b.0).collect();
    for r in rest {
        p.rooms[r as usize].kind = ["dormitory", "dormitory", "storeroom", "shrine", "dormitory"][rng.below(5) as usize];
    }
    for (i, kind) in ["sacrifice pit", "robing room"].into_iter().enumerate() {
        if let Some(&(_, r)) = inner.get(i) {
            p.rooms[r as usize].kind = kind;
        }
    }
    // A secret door: one room opening on both its rings (a way round a gate).
    if rng.next_f64() < 0.7 && !doors.is_empty() && p.secret.len() < p.secrets {
        let (_, _, _, a, b) = doors[rng.below(doors.len() as u32) as usize];
        p.door(a, b);
        p.secret.insert((a.min(b), a.max(b)));
    }
    let ring_pieces = || {
        let (hx, hy) = half(n - 1);
        let (x0, y0) = ((cx - hx) as usize, (cy - hy) as usize);
        let (w, hh) = ((2 * hx + 1) as usize, (2 * hy + 1) as usize);
        [(x0, y0, w, 1), (x0, y0 + hh - 1, w, 1), (x0, y0, 1, hh), (x0 + w - 1, y0, 1, hh)].map(|(x, y, w, h)| Piece { room: passage, x, y, w, h, door: None, open: true })
    };
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).saturating_sub(rooms(&p, passage)).max(2);
    let specs = specs(area);
    let cap = ROOMS_GROWING - 4;
    let inside = |k: usize| {
        let (hx, hy) = half(n - 1);
        let (x, y) = ((k % nx) as isize, (k / nx) as isize);
        (x - cx).abs() <= hx && (y - cy).abs() <= hy
    };
    if inside(arrive) {
        // (Arrived among the rings: rooms off the outer ring only.)
        let mut g = Grower::new(nx, ny, passage, None);
        (g.cap, g.spread) = (cap, area > 1.0);
        g.pieces.extend(ring_pieces());
        g.grow(&mut p, &specs, target, false, rng);
    } else if depth > 0 {
        // A landing at the stairs, a passage dug to the outer ring; rooms off both.
        let Some((mut g, _)) = first_room(&mut p, passage, "landing", Template::Rect, (3, 3), None, 0) else { return mixed(nx, ny, arrive, depth, last, theme, area, rng) };
        let landing = p.cells[arrive];
        dig(&mut p, nx, ny, landing, passage, passage, false);
        (g.cap, g.spread) = (cap, area > 1.0);
        g.pieces.extend(ring_pieces());
        g.grow(&mut p, &specs, target, false, rng);
    } else {
        // The top level: a plain cellar (or a crypt's front) round the way in, kept apart from the cult.
        let crypt = rng.next_f64() < 0.4;
        let (first, kinds): (&'static str, &[&'static str]) = if crypt { ("antechamber", &["burial hall", "ossuary"]) } else { (theme.first, &["storeroom", "storeroom", "cellar"]) };
        let ring = halo(&mut p, ny, &|_| true);
        let dir = if ax < cx { Side::E } else { Side::W };
        let Some((mut g, _)) = first_room(&mut p, passage, first, Template::Rect, (4, 5), Some(dir), 1) else {
            unhalo(&mut p, &ring);
            return mixed(nx, ny, arrive, depth, last, theme, area, rng);
        };
        g.cap = cap;
        let front: Vec<Spec> = kinds.iter().map(|&kind| Spec { kind, t: Template::Rect, across: (3, 5), deep: (3, 5) }).collect();
        g.grow(&mut p, &front, 1 + rng.below(3) as usize, false, rng);
        unhalo(&mut p, &ring);
        let fronts: Vec<i16> = g.pieces.iter().map(|f| f.room).filter(|&r| r != passage).collect();
        // The secret door: a passage dug from the front room nearest the rings to the outer ring.
        let (hx, hy) = half(n - 1);
        let gap = |r: i16| (0..nx * ny).filter(|&k| p.cells[k] == r).map(|k| ((k % nx) as isize - cx).abs().saturating_sub(hx) + ((k / nx) as isize - cy).abs().saturating_sub(hy)).min().unwrap_or(isize::MAX);
        let mut order: Vec<(isize, i16)> = fronts.iter().map(|&r| (gap(r), r)).collect();
        order.sort_unstable();
        for (_, r) in order {
            let made = dig(&mut p, nx, ny, r, passage, passage, false);
            if let Some(&e) = made.iter().find(|e| p.cells[e.0] == r || p.cells[e.1] == r) {
                p.secret.insert(e);
                break;
            }
        }
        // Rooms off the outer ring, kept apart from the front.
        let shut = halo(&mut p, ny, &|r| fronts.contains(&r));
        let mut g = Grower::new(nx, ny, passage, None);
        (g.cap, g.spread) = (cap, area > 1.0);
        g.pieces.extend(ring_pieces());
        g.grow(&mut p, &specs, target, false, rng);
        unhalo(&mut p, &shut);
    }
    p.passage = Some((passage, false));
    p.taken[p.up] = true;
    if !last {
        let skip: Vec<i16> = (0..p.rooms.len() as i16).filter(|&r| matches!(p.rooms[r as usize].kind, CHAMBER_KIND | CELL)).collect();
        let d = way_down(&p, ny, &skip);
        p.down = Some(d);
        p.taken[d] = true;
    }
    p
}

/// What grows off the outer ring: dormitories, cells, stores, a shrine; short passages.
fn specs(area: f64) -> Vec<Spec> {
    let up = |n: usize| ((n as f64) * crate::core::sqrt(area).max(1.0)).round() as usize;
    let mut out = Vec::new();
    for kind in ["dormitory", "dormitory", "storeroom", "shrine", "robing room", "cell"] {
        let t = if kind == "shrine" { Template::Apse } else { Template::Rect };
        let (a, d) = if kind == CELL { ((2, 3), (2, 3)) } else { ((3, up(5)), (3, up(5))) };
        out.push(Spec { kind, t, across: a, deep: d });
    }
    out.push(Spec { kind: "", t: Template::Corridor, across: (1, 1), deep: (2, 4) });
    out
}

/// Mark the rock round the floor of rooms `of` picks (8 ways) as `HALO`: what grows next keeps a square off them.
/// The squares marked.
fn halo(p: &mut Plan, ny: usize, of: &dyn Fn(i16) -> bool) -> Vec<usize> {
    let nx = p.nx;
    let mut out = Vec::new();
    for k in 0..nx * ny {
        if p.cells[k] >= 0 {
            continue;
        }
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        let near = (-1..=1isize).any(|b| (-1..=1isize).any(|a| {
            let (x, y) = (i + a, j + b);
            x >= 0 && y >= 0 && x < nx as isize && y < ny as isize && {
                let c = p.cells[y as usize * nx + x as usize];
                c >= 0 && c != HALO && of(c)
            }
        }));
        if near {
            out.push(k);
        }
    }
    for &k in &out {
        p.cells[k] = HALO;
    }
    out
}

/// Back to rock.
fn unhalo(p: &mut Plan, squares: &[usize]) {
    for &k in squares {
        p.cells[k] = -1;
    }
}
