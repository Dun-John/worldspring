//! Goblin warrens (`goblin_warren`): organic burrows, drawn rounded like caves (dens and crawlways shaped `CAVERN`).
//! From the warren mouth, twisting crawlways a square wide are dug out across the level (each try aimed at a random
//! spot, from the burrow nearest it), most ending in a den of uneven size and roundish shape, some running out (dead
//! ends); crawlways branch off crawlways. Dens take their kinds by how far in they lie: the wolf pen and refuse pit by
//! the mouth, nests and the cook pit in the middle, the shaman's den deepest; the chief's (the boss chamber) a big den
//! at the end of the deepest level. Loops: the few shortcuts `kit::guard` digs between dens near each other.
//! The burrowing tools (`blob`, `burrow`, `den`, `lay`) dig the bandits' cave front too.

use crate::core::rng::Pcg32;
use crate::interior::shape::CAVERN;
use crate::under::grow::{ROOMS_GROWING, fallback, fill};
use crate::under::themes::keep::finish;
use crate::under::*;

/// Unit steps by heading: 0 east, 1 south, 2 west, 3 north.
const STEPS: [(isize, isize); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// The square a step from `k` heading `d`, inside the grid.
pub(super) fn step(k: usize, nx: usize, ny: usize, d: usize) -> Option<usize> {
    let (x, y) = ((k % nx) as isize + STEPS[d].0, (k / nx) as isize + STEPS[d].1);
    (x >= 0 && y >= 0 && x < nx as isize && y < ny as isize).then(|| y as usize * nx + x as usize)
}

/// The squares round square `k` (8 ways), inside the grid.
fn around(k: usize, nx: usize, ny: usize) -> impl Iterator<Item = usize> {
    let (i, j) = ((k % nx) as isize, (k / nx) as isize);
    (-1..=1isize)
        .flat_map(move |dj| (-1..=1isize).map(move |di| (i + di, j + dj)))
        .filter(move |&(x, y)| (x, y) != (i, j) && x >= 0 && y >= 0 && x < nx as isize && y < ny as isize)
        .map(move |(x, y)| y as usize * nx + x as usize)
}

/// Whether square `k` may be dug: rock inside the ring of rock round the level, with no floor round it (8 ways) but
/// where `ok`, and none of the `avoid` squares (dug but not laid yet) round it but those `ok` lets.
fn free(p: &Plan, ny: usize, k: usize, ok: &dyn Fn(usize) -> bool, avoid: &[usize]) -> bool {
    let nx = p.nx;
    let (i, j) = (k % nx, k / nx);
    i >= 1 && j >= 1 && i + 1 < nx && j + 1 < ny && p.cells[k] < 0 && !avoid.contains(&k) && around(k, nx, ny).all(|m| ok(m) || (p.cells[m] < 0 && !avoid.contains(&m)))
}

/// A crawlway dug out of floor square `start` heading `d`: up to `len` squares, turning left or right with chance
/// `turn` a step, never beside other floor (the room it leaves only for its first two steps) nor its own earlier
/// squares; it stops where it can't go on. Its squares (not laid) and its last heading.
pub(super) fn burrow(p: &Plan, ny: usize, start: usize, mut d: usize, len: usize, turn: f64, rng: &mut Pcg32) -> (Vec<usize>, usize) {
    let nx = p.nx;
    let from = p.cells[start];
    let mut path: Vec<usize> = Vec::new();
    let mut at = start;
    for s in 0..len {
        let mut tries = [d, d];
        if s > 0 && rng.next_f64() < turn {
            tries[0] = if rng.next_f64() < 0.5 { (d + 1) % 4 } else { (d + 3) % 4 };
        }
        let prev = path.len().checked_sub(2).map(|i| path[i]);
        let older = &path[..path.len().saturating_sub(2)];
        let next = tries.into_iter().find_map(|t| {
            let m = step(at, nx, ny, t)?;
            let ok = |q: usize| q == at || Some(q) == prev || (s < 2 && p.cells[q] == from);
            free(p, ny, m, &ok, older).then_some((m, t))
        });
        let Some((m, t)) = next else { break };
        path.push(m);
        (at, d) = (m, t);
    }
    (path, d)
}

/// The squares of a rough ellipse round square `c` (half-sizes `r`, across x and across y), ragged at its edge, kept
/// to the part joined (4 ways) to `force[0]`; the `force` squares always in. None if any can't be dug (`free`: beside
/// floor only where `ok`, never beside the `avoid` squares but those) or it has fewer than 4.
#[allow(clippy::too_many_arguments)]
pub(super) fn blob(p: &Plan, ny: usize, c: (isize, isize), r: (f64, f64), force: &[usize], ok: &dyn Fn(usize) -> bool, avoid: &[usize], rng: &mut Pcg32) -> Option<Vec<usize>> {
    let nx = p.nx;
    let (ex, ey) = (crate::core::ceil(r.0) as isize + 1, crate::core::ceil(r.1) as isize + 1);
    let (x0, x1) = ((c.0 - ex).max(1), (c.0 + ex).min(nx as isize - 2));
    let (y0, y1) = ((c.1 - ey).max(1), (c.1 + ey).min(ny as isize - 2));
    if x0 > x1 || y0 > y1 {
        return None;
    }
    let mut inside = vec![false; nx * ny];
    for j in y0..=y1 {
        for i in x0..=x1 {
            let (dx, dy) = ((i - c.0) as f64 / r.0, (j - c.1) as f64 / r.1);
            if dx * dx + dy * dy <= 1.0 + 0.4 * (rng.next_f64() - 0.5) {
                inside[j as usize * nx + i as usize] = true;
            }
        }
    }
    for &k in force {
        inside[k] = true;
    }
    let start = *force.first()?;
    let mut out = vec![start];
    let mut seen = vec![false; nx * ny];
    seen[start] = true;
    let mut i = 0;
    while i < out.len() {
        let k = out[i];
        i += 1;
        for m in neighbours(k, nx, ny) {
            if inside[m] && !seen[m] {
                seen[m] = true;
                out.push(m);
            }
        }
    }
    (out.len() >= 4 && out.iter().all(|&k| free(p, ny, k, &|q| ok(q) || seen[q], avoid))).then_some(out)
}

/// A den beyond the end of crawlway `path` (its last heading `d`): a `blob` (half-sizes `r`) whose near edge is the
/// square past the crawlway's tip, beside nothing but the tip and the square before it. Its squares, the first the one
/// at the tip.
pub(super) fn den(p: &Plan, ny: usize, path: &[usize], d: usize, r: (f64, f64), rng: &mut Pcg32) -> Option<Vec<usize>> {
    let nx = p.nx;
    let tip = *path.last()?;
    let first = step(tip, nx, ny, d)?;
    let reach = crate::core::round(if d % 2 == 0 { r.0 } else { r.1 }).max(1.0) as isize;
    let c = ((first % nx) as isize + STEPS[d].0 * reach, (first / nx) as isize + STEPS[d].1 * reach);
    // (The squares from the first to the middle always in: the den reaches its door.)
    let mut force = vec![first];
    for t in 1..=reach {
        let (x, y) = ((first % nx) as isize + STEPS[d].0 * t, (first / nx) as isize + STEPS[d].1 * t);
        if x < 1 || y < 1 || x + 1 >= nx as isize || y + 1 >= ny as isize {
            return None;
        }
        force.push(y as usize * nx + x as usize);
    }
    let near = &path[path.len().saturating_sub(2)..];
    let older = &path[..path.len().saturating_sub(2)];
    blob(p, ny, c, r, &force, &|q| near.contains(&q), older, rng)
}

/// Lay `squares` as room `r`.
pub(super) fn lay(p: &mut Plan, squares: &[usize], r: i16) {
    for &k in squares {
        p.cells[k] = r;
    }
}

/// From floor square `s`, the last square of its room straight on heading `d` (a den's edge that way).
pub(super) fn edge(p: &Plan, ny: usize, mut s: usize, d: usize) -> usize {
    while let Some(m) = step(s, p.nx, ny, d).filter(|&m| p.cells[m] == p.cells[s]) {
        s = m;
    }
    s
}

/// A new room `kind` drawn as a cavern.
pub(super) fn cavern(p: &mut Plan, kind: &'static str) -> i16 {
    let r = p.room(kind, 0.0);
    p.rooms[r as usize].shape = CAVERN;
    r
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn warren(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let passage = cavern(&mut p, theme.passage);
    p.secrets = 2 + (area >= 1.3) as usize;
    let big = crate::core::sqrt(area).clamp(1.0, 1.3);
    let target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * fill(area)).round() as usize).max(4);
    // The mouth round the arrival (a den on the levels below).
    let r0 = 1.4 + 0.8 * rng.next_f64();
    let c = ((arrive % nx) as isize, (arrive / nx) as isize);
    let Some(sq) = blob(&p, ny, c, (r0 + 0.4, r0), &[arrive], &|_| false, &[], rng) else { return fallback(p, last) };
    let first = cavern(&mut p, if depth == 0 { theme.first } else { "den" });
    lay(&mut p, &sq, first);
    let mut floor = sq;
    let mut dens = vec![first];
    let (mut made, mut ends) = (0, 0);
    for _ in 0..target * 40 {
        if made >= target {
            break;
        }
        // Aimed at a random spot: from one of the burrowed squares nearest it (of a few drawn), out of its room's
        // edge facing that way (a crawlway's side: a branch).
        let t = (1 + rng.below(nx as u32 - 2) as usize, 1 + rng.below(ny as u32 - 2) as usize);
        let mut s = floor[0];
        let mut best = usize::MAX;
        for _ in 0..8 {
            let k = floor[rng.below(floor.len() as u32) as usize];
            let dd = (k % nx).abs_diff(t.0) + (k / nx).abs_diff(t.1);
            if dd < best {
                (best, s) = (dd, k);
            }
        }
        let (dx, dy) = (t.0 as isize - (s % nx) as isize, t.1 as isize - (s / nx) as isize);
        let d = if dx.abs() >= dy.abs() { if dx >= 0 { 0 } else { 2 } } else if dy >= 0 { 1 } else { 3 };
        let s = edge(&p, ny, s, d);
        let len = 3 + rng.below((5.0 * big) as u32 + 1) as usize;
        let (path, h) = burrow(&p, ny, s, d, len, 0.35, rng);
        if path.len() < 2 {
            continue;
        }
        let r = ((1.2 + 2.0 * rng.next_f64()) * big, (1.2 + 2.0 * rng.next_f64()) * big);
        let dug = if p.rooms.len() < ROOMS_GROWING { den(&p, ny, &path, h, r, rng) } else { None };
        if dug.is_none() && (ends > target / 2 || rng.next_f64() >= 0.3) {
            continue;
        }
        lay(&mut p, &path, passage);
        if p.cells[s] != passage {
            p.door(s, path[0]);
        }
        floor.extend_from_slice(&path);
        match dug {
            Some(sq) => {
                let r = cavern(&mut p, "den");
                lay(&mut p, &sq, r);
                p.door(*path.last().unwrap_or(&s), sq[0]);
                floor.extend_from_slice(&sq);
                dens.push(r);
                made += 1;
            }
            None => ends += 1,
        }
    }
    // The chief's den at the end of the deepest level: a big den off the burrows farthest in, a dead end.
    let mut boss = None;
    if last {
        let dist = p.distances(arrive, ny, None);
        let mut far: Vec<usize> = floor.iter().copied().filter(|&k| dist[k] > 0).collect();
        far.sort_by_key(|&k| (std::cmp::Reverse(dist[k]), k));
        'b: for &s in far.iter().take(48) {
            let d0 = rng.below(4) as usize;
            for d in (0..4).map(|i| (d0 + i) % 4) {
                if step(s, nx, ny, d).is_none_or(|m| p.cells[m] >= 0) {
                    continue;
                }
                let (path, h) = burrow(&p, ny, s, d, 2 + rng.below(3) as usize, 0.25, rng);
                if path.len() < 2 {
                    continue;
                }
                let r = (2.6 + 1.2 * rng.next_f64()) * big;
                let Some(sq) = den(&p, ny, &path, h, (r, r * 0.85), rng) else { continue };
                lay(&mut p, &path, passage);
                if p.cells[s] != passage {
                    p.door(s, path[0]);
                }
                let b = cavern(&mut p, BOSS);
                lay(&mut p, &sq, b);
                p.door(*path.last().unwrap_or(&s), sq[0]);
                boss = Some(b);
                break 'b;
            }
        }
    }
    // Den kinds by how far in they lie.
    let dist = p.distances(arrive, ny, None);
    let mut nearest = vec![i32::MAX; p.rooms.len()];
    for k in (0..nx * ny).filter(|&k| p.cells[k] >= 0 && dist[k] >= 0) {
        let r = p.cells[k] as usize;
        nearest[r] = nearest[r].min(dist[k]);
    }
    let mut doors = vec![0; p.rooms.len()];
    for &(a, b) in &p.doors {
        doors[p.cells[a] as usize] += 1;
        doors[p.cells[b] as usize] += 1;
    }
    let mut order: Vec<(i32, i16)> = dens[1..].iter().map(|&r| (nearest[r as usize], r)).collect();
    order.sort_unstable();
    let n = order.len();
    for (i, &(_, r)) in order.iter().enumerate() {
        let f = i as f64 / n as f64;
        let kinds: &[&'static str] = if f < 0.3 {
            &["den", "den", "nest", "storeroom"]
        } else if f < 0.75 {
            &["den", "nest", "nest", "storeroom"]
        } else {
            &["den", "den", "nest"]
        };
        p.rooms[r as usize].kind = kinds[rng.below(kinds.len() as u32) as usize];
    }
    if depth == 0 {
        for (i, kind) in ["wolf pen", "refuse pit"].into_iter().enumerate().filter(|&(i, _)| i < n) {
            p.rooms[order[i].1 as usize].kind = kind;
        }
    }
    if n >= 4 {
        p.rooms[order[n / 2].1 as usize].kind = "cook pit";
    }
    if last && let Some(&(_, r)) = order.iter().rev().find(|&&(_, r)| doors[r as usize] == 1 && p.rooms[r as usize].kind != "cook pit") {
        p.rooms[r as usize].kind = "shaman's hut";
    }
    finish(&mut p, ny, passage, 1, last, boss, &[], &[]);
    p
}
