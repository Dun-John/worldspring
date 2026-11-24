//! Fungi underground (`kit::furnish`, once the rooms' kits are in): patches that gather where it is damp (beside
//! water, along the rock, in dead ends and alcoves), each grown as one clump from a seed square picked by that bias;
//! groves of mycelium carpet with giant caps standing in it in some caves; a few small patches in damp built rooms.
//! Kinds: `fungus` (glowing), `toadstool` (small caps, puffballs), `shelf_fungus` (on the rock wall), `mycelium`
//! (walkable carpet), `mushroom` (giant caps, blocking: placed only where every square stays reached, `Plan::place`).

use crate::core::rng::Pcg32;
use crate::under::*;

/// Ways in and out, kept clear of giant caps by a square.
const WAYS: [&str; 9] = ["up", "down", "exit", "tunnel", "ladder", "grate_ladder", "outfall", "link", "trapdoor"];
/// Built rooms that are damp by their kind.
const DAMP: [&str; 9] = ["cistern", "sump", "overflow chamber", "drain chamber", "ritual pool", "sunken chapel", "well room", "flooded gallery", "flooded hall"];
/// Cave rooms that are all grove, and rooms rich in fungi.
const GROVES: [&str; 2] = ["fungus forest", "mycelium hall"];
const RICH: [&str; 3] = ["fungus grotto", "spore grotto", "mushroom farm"];
/// The chance a cave level (not fungal) has a grove in its biggest room, and the smallest room that takes one.
const GROVE_CHANCE: f64 = 0.25;
const GROVE_MIN: usize = 48;

#[derive(Clone, Copy, PartialEq)]
enum Mix {
    /// A cave's: every kind, a few giants.
    Cave,
    /// A fungus grotto's or a mushroom farm's: more glowing ones and giants.
    Rich,
    /// A damp built room's: small ones and shelves, no giants.
    Built,
}

/// Grow the level's fungi (see the module).
pub(crate) fn grow_fungi(p: &mut Plan, nx: usize, ny: usize, kind: UnderKind, rng: &mut Pcg32) {
    // (Fungi draw on their own stream, one number from the level's: the rest of the site never changes with them.)
    let rng = &mut Pcg32::new(rng.next_u32() as u64, 0xf09);
    if kind == UnderKind::LavaTube || p.theme == "ice" {
        return;
    }
    let n = nx * ny;
    let start = p.items.len();
    let squares = |f: &Item| {
        let (x0, x1) = (f.x as usize, (f.x + f.w) as usize);
        (f.y as usize..(f.y + f.h) as usize).flat_map(move |j| (x0..x1).map(move |i| j * nx + i))
    };
    let mut water = vec![false; n];
    let mut way = vec![false; n];
    for f in &p.items {
        let wet = matches!(f.kind, "water_shallow" | "water_deep" | "flooded_pit");
        let w = WAYS.contains(&f.kind);
        for k in squares(f).filter(|&k| k < n) {
            water[k] |= wet;
            way[k] |= w;
        }
    }
    for &(a, b) in &p.doors {
        way[a] = true;
        way[b] = true;
    }
    let rock = |k: usize, di: isize, dj: isize| {
        let (x, y) = ((k % nx) as isize + di, (k / nx) as isize + dj);
        x < 0 || y < 0 || x >= nx as isize || y >= ny as isize || p.cells[y as usize * nx + x as usize] < 0
    };
    let near = |marks: &[bool], k: usize, d: isize| {
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        (-d..=d).any(|dj| (-d..=d).any(|di| {
            let (x, y) = (i + di, j + dj);
            x >= 0 && y >= 0 && x < nx as isize && y < ny as isize && marks[y as usize * nx + x as usize]
        }))
    };
    // How damp a square is: beside water (3; within two squares 1.5), against the rock (0.8 a side, at most 1.6), in a
    // dead end (2) or an alcove (1).
    let mut score = vec![0.0f64; n];
    let mut walls = vec![0u8; n];
    for k in 0..n {
        if p.cells[k] < 0 {
            continue;
        }
        let r4 = [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().filter(|&&(di, dj)| rock(k, di, dj)).count();
        let r8 = r4 + [(-1, -1), (1, -1), (1, 1), (-1, 1)].iter().filter(|&&(di, dj)| rock(k, di, dj)).count();
        walls[k] = r4 as u8;
        let wet = if near(&water, k, 1) { 3.0 } else if near(&water, k, 2) { 1.5 } else { 0.0 };
        let nook = if r4 >= 3 { 2.0 } else if r8 >= 5 { 1.0 } else { 0.0 };
        score[k] = wet + (0.8 * r4 as f64).min(1.6) + nook;
    }
    // Squares kept bare: water, and the squares at a ledge's sheer edge (where `Plan::ramp_ledges` may lay a slope).
    let mut off = water.clone();
    for k in 0..n {
        let r = p.cells[k];
        if r >= 0 && neighbours(k, nx, ny).any(|m| p.cells[m] >= 0 && p.cells[m] != r && p.rooms[p.cells[m] as usize].raise != p.rooms[r as usize].raise) {
            off[k] = true;
            // (Giants keep a square clear of a ledge's edge too, as of the ways: one there could leave a floor reached
            // only down or up the sheer edge.)
            way[k] = true;
        }
    }
    let fungal = p.theme == "fungal";
    // A grove in the biggest room of some caves (all of a fungal cave's forests and mycelium halls are groves).
    let mut grove = -1i16;
    if p.natural && kind == UnderKind::Cave && !fungal && rng.next_f64() < GROVE_CHANCE {
        let size = |ri: usize| (0..n).filter(|&k| p.cells[k] == ri as i16 && !p.taken[k]).count();
        grove = (0..p.rooms.len()).filter(|&ri| p.rooms[ri].kind != BOSS).map(|ri| (size(ri), ri)).filter(|&(s, _)| s >= GROVE_MIN).max().map_or(-1, |(_, ri)| ri as i16);
    }
    for ri in 0..p.rooms.len() {
        let rk = p.rooms[ri].kind;
        if rk == BOSS || rk == "sewer tunnel" {
            continue;
        }
        let free: Vec<usize> = (0..n).filter(|&k| p.cells[k] == ri as i16 && !p.taken[k] && !off[k]).collect();
        // (A built room's patch fits where a few squares are left; a cave's needs more.)
        if free.len() < 2 || free.len() < 4 && (p.natural || kind == UnderKind::Mine) {
            continue;
        }
        let wet_room = free.iter().any(|&k| near(&water, k, 1));
        let ri = ri as i16;
        if GROVES.contains(&rk) || ri == grove {
            // A fungal cave's forests and halls nearly all grove; another cave's biggest room about half.
            let whole = GROVES.contains(&rk);
            let size = if whole { (free.len() * 7 / 10).clamp(30, 160).min(free.len() * 4 / 5) } else { (free.len() / 2).clamp(24, 90).min(free.len() * 3 / 5) };
            // (A seed shut in a nook grows no grove: the best of a few tries.)
            let mut clump = Vec::new();
            let mut left = free.clone();
            for _ in 0..4 {
                if left.is_empty() {
                    break;
                }
                let seed = pick(&left, &score, rng);
                let c = grow(p, nx, ny, ri, seed, size, &score, &off, rng);
                left.retain(|k| !c.contains(k));
                if c.len() > clump.len() {
                    clump = c;
                }
                if clump.len() * 3 >= size * 2 {
                    break;
                }
            }
            grove_of(p, nx, ny, &clump, &walls, &way, &score, whole, rng);
            continue;
        }
        let (count, lo, hi, mix) = if RICH.contains(&rk) {
            ((2 + free.len() / 40).min(8), 5, 12, Mix::Rich)
        } else if p.natural || kind == UnderKind::Mine {
            let chance = if kind == UnderKind::Mine { 0.2 } else { 0.35 } + if wet_room { 0.3 } else { 0.0 } + if fungal { 0.25 } else { 0.0 } + if p.theme == "flooded" { 0.2 } else { 0.0 };
            if rng.next_f64() >= chance {
                continue;
            }
            ((1 + free.len() / 70).min(4), 4, 10, Mix::Cave)
        } else {
            // Built rooms: only damp ones (by kind, beside water, or under a cave-in's broken walls), and sparsely.
            let broken = p.gaps.iter().any(|&(a, b)| p.cells[a] == ri || p.cells[b] == ri);
            let damp = wet_room || DAMP.contains(&rk) || rk.contains("flooded") || broken;
            if !damp || rng.next_f64() >= if wet_room { 0.9 } else { 0.65 } {
                continue;
            }
            ((1 + free.len() / 80).min(2), 2, 6, Mix::Built)
        };
        for _ in 0..count {
            let free: Vec<usize> = (0..n).filter(|&k| p.cells[k] == ri && !p.taken[k] && !off[k]).collect();
            if free.len() < lo {
                break;
            }
            // (A built room's patch starts in a corner or along a wall, the damper the better.)
            let walled: Vec<usize> = free.iter().copied().filter(|&k| walls[k] > 0).collect();
            let seed = pick(if mix == Mix::Built && !walled.is_empty() { &walled } else { &free }, &score, rng);
            let size = lo.max(3) + rng.below((hi - lo.max(3) + 1) as u32) as usize;
            let clump = grow(p, nx, ny, ri, seed, size, &score, &off, rng);
            patch_of(p, nx, ny, &clump, &walls, &way, mix, fungal, rng);
        }
    }
    // (The carpet first, so that the caps standing beside it are drawn over its soft edge.)
    let mut fungi = p.items.split_off(start);
    fungi.sort_by_key(|f| f.kind != "mycelium");
    p.items.extend(fungi);
}

/// A seed square from `free`, weighted toward the damp (score squared).
fn pick(free: &[usize], score: &[f64], rng: &mut Pcg32) -> usize {
    let w = |k: usize| 0.1 + score[k] * score[k];
    let total: f64 = free.iter().map(|&k| w(k)).sum();
    let mut t = rng.next_f64() * total;
    for &k in free {
        t -= w(k);
        if t <= 0.0 {
            return k;
        }
    }
    free[free.len() - 1]
}

/// A clump of up to `size` free squares of room `ri` joined side to side, grown from `seed` a square at a time, each
/// picked among those beside it weighted toward the damp.
#[allow(clippy::too_many_arguments)]
fn grow(p: &Plan, nx: usize, ny: usize, ri: i16, seed: usize, size: usize, score: &[f64], off: &[bool], rng: &mut Pcg32) -> Vec<usize> {
    let ok = |k: usize| p.cells[k] == ri && !p.taken[k] && !off[k];
    let mut seen = vec![false; nx * ny];
    seen[seed] = true;
    let mut got = vec![seed];
    let mut front: Vec<usize> = Vec::new();
    let add = |k: usize, seen: &mut Vec<bool>, front: &mut Vec<usize>| {
        for m in neighbours(k, nx, ny) {
            if !seen[m] && ok(m) {
                seen[m] = true;
                front.push(m);
            }
        }
    };
    add(seed, &mut seen, &mut front);
    while got.len() < size && !front.is_empty() {
        let total: f64 = front.iter().map(|&k| 0.6 + score[k]).sum();
        let mut t = rng.next_f64() * total;
        let mut i = front.len() - 1;
        for (j, &k) in front.iter().enumerate() {
            t -= 0.6 + score[k];
            if t <= 0.0 {
                i = j;
                break;
            }
        }
        let k = front.swap_remove(i);
        got.push(k);
        add(k, &mut seen, &mut front);
    }
    got
}

/// Whether a giant cap may stand on `k`: off the rock, a square clear of ways in and out and of other giants.
fn giant_ok(p: &Plan, nx: usize, ny: usize, k: usize, walls: &[u8], way: &[bool]) -> bool {
    let (i, j) = ((k % nx) as isize, (k / nx) as isize);
    walls[k] <= 1
        && (-1..=1).all(|dj| (-1..=1).all(|di| {
            let (x, y) = (i + di, j + dj);
            if x < 0 || y < 0 || x >= nx as isize || y >= ny as isize {
                return true;
            }
            let q = y as usize * nx + x as usize;
            !way[q] && !(p.blocked[q] && p.items.iter().rev().any(|f| f.kind == "mushroom" && (f.x as isize..(f.x + f.w) as isize).contains(&x) && (f.y as isize..(f.y + f.h) as isize).contains(&y)))
        }))
}

/// A heart of a clump: its middle (squares, the grid's corner at 0) and how far its growth reaches round it.
type Heart = (f64, f64, f64);

/// A giant cap at `k` (2 x 2 with chance `big`, covering `k` from whichever of its four corners first fits, all four
/// squares in `inside` and clear), if one may stand there; its middle.
#[allow(clippy::too_many_arguments)]
fn giant_at(p: &mut Plan, nx: usize, ny: usize, k: usize, inside: &[bool], walls: &[u8], way: &[bool], big: f64, rng: &mut Pcg32) -> Option<(f64, f64)> {
    if p.taken[k] || !giant_ok(p, nx, ny, k, walls, way) {
        return None;
    }
    let ri = p.cells[k];
    let (x, y) = (k % nx, k / nx);
    if rng.next_f64() < big {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            if x < dx || y < dy || x - dx + 1 >= nx || y - dy + 1 >= ny {
                continue;
            }
            let c = (y - dy) * nx + x - dx;
            let fits = [c, c + 1, c + nx, c + nx + 1].iter().all(|&q| q == k || inside[q] && p.cells[q] == ri && !p.taken[q] && giant_ok(p, nx, ny, q, walls, way));
            if fits && p.place(item("mushroom", x - dx, y - dy, 2, 2), Some(ri), ny) {
                return Some(((x - dx) as f64 + 1.0, (y - dy) as f64 + 1.0));
            }
        }
    }
    p.place(item("mushroom", x, y, 1, 1), Some(ri), ny).then_some((x as f64 + 0.5, y as f64 + 0.5))
}

/// Fill what is left of `clump` round its `hearts`: shelves on the rock; close round a heart a ring of small caps and
/// glowing ones (`glow` of them glowing) with carpet between; farther out (to `spread` reaches) carpet, ragged at its edge (each square
/// by its own chance); beyond, bare floor. Carpet in look `carpet` (one look a patch, so its squares join), glowing
/// ones mostly in look `lamp`.
#[allow(clippy::too_many_arguments)]
fn fill(p: &mut Plan, nx: usize, ny: usize, clump: &[usize], walls: &[u8], hearts: &[Heart], spread: f64, glow: f64, carpet: u8, lamp: u8, rng: &mut Pcg32) {
    let ri = p.cells[clump[0]];
    for &k in clump {
        if p.taken[k] {
            continue;
        }
        let (x, y) = ((k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5);
        // How far out, as a share of the nearest heart's reach.
        let d = hearts.iter().map(|&(hx, hy, r)| crate::core::sqrt((x - hx) * (x - hx) + (y - hy) * (y - hy)) / r).fold(f64::MAX, f64::min);
        let (u, j) = (rng.next_f64(), rng.next_f64());
        let what = if d > spread + 0.5 * j {
            continue;
        } else if walls[k] > 0 && u < 0.45 {
            "shelf_fungus"
        } else if d < 0.6 && u < 0.72 || d < 1.0 && u < 0.3 || u < 0.06 {
            if rng.next_f64() < glow { "fungus" } else { "toadstool" }
        } else if d > spread && j > 0.6 {
            continue;
        } else {
            "mycelium"
        };
        let mut it = item(what, k % nx, k / nx, 1, 1);
        match what {
            "mycelium" => it.variant = Some(carpet),
            "fungus" if rng.next_f64() < 0.75 => it.variant = Some(lamp),
            _ => {}
        }
        p.place(it, Some(ri), ny);
    }
}

/// A patch over `clump`, round its seed (the first square): a giant there now and then (never in built rooms), then
/// `fill`.
#[allow(clippy::too_many_arguments)]
fn patch_of(p: &mut Plan, nx: usize, ny: usize, clump: &[usize], walls: &[u8], way: &[bool], mix: Mix, fungal: bool, rng: &mut Pcg32) {
    let mut inside = vec![false; nx * ny];
    for &k in clump {
        inside[k] = true;
    }
    let s = clump[0];
    let mut heart: Heart = ((s % nx) as f64 + 0.5, (s / nx) as f64 + 0.5, 0.0);
    let giant = match mix {
        Mix::Built => 0.0,
        Mix::Cave if fungal => 0.4,
        Mix::Cave => 0.25,
        Mix::Rich => 0.55,
    };
    if clump.len() >= 5 && rng.next_f64() < giant {
        if let Some((x, y)) = giant_at(p, nx, ny, s, &inside, walls, way, 0.15, rng) {
            heart = (x, y, 0.0);
        }
    }
    // (Reach: about the round patch of the clump's size, the farthest square at most a little beyond.)
    heart.2 = 0.6 + crate::core::sqrt(clump.len() as f64 / 2.4);
    let glow = match mix {
        Mix::Built => 0.3,
        Mix::Cave => 0.45,
        Mix::Rich => 0.6,
    };
    let (carpet, lamp) = (rng.below(4) as u8, rng.below(4) as u8);
    fill(p, nx, ny, clump, walls, &[heart], 1.0, glow, carpet, lamp, rng);
}

/// A grove over `clump`: several hearts of different sizes well apart (picked toward the damp), most with a giant cap
/// or two (some 2 x 2, a square clear round each, so a way always winds between them), the rest a knot of glowing
/// ones; then `fill` round them. `whole`: a fungal cave's forest or hall, with more and bigger hearts.
#[allow(clippy::too_many_arguments)]
fn grove_of(p: &mut Plan, nx: usize, ny: usize, clump: &[usize], walls: &[u8], way: &[bool], score: &[f64], whole: bool, rng: &mut Pcg32) {
    let mut inside = vec![false; nx * ny];
    for &k in clump {
        inside[k] = true;
    }
    let want = 1 + clump.len() / if whole { 12 } else { 20 };
    let mut hearts: Vec<Heart> = Vec::new();
    let mut tries = 0;
    while hearts.len() < want && tries < want * 8 {
        tries += 1;
        let k = pick(clump, score, rng);
        let (x, y) = ((k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5);
        // (The first heart the biggest; the rest from small knots to big stands.)
        let r = if hearts.is_empty() { 3.0 + rng.next_f64() * if whole { 2.0 } else { 0.8 } } else { 1.5 + rng.next_f64() * if whole { 2.5 } else { 1.4 } };
        if hearts.iter().any(|&(hx, hy, hr)| crate::core::sqrt((x - hx) * (x - hx) + (y - hy) * (y - hy)) < 0.8 * (r + hr)) {
            continue;
        }
        let mut at = (x, y);
        // (The giant on the heart's square or, where it can't stand there, the first square beside it where it can.)
        let near = |q: isize| {
            let (i, j) = ((k % nx) as isize + q % 3 - 1, (k / nx) as isize + q / 3 - 1);
            (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && inside[j as usize * nx + i as usize]).then(|| j as usize * nx + i as usize)
        };
        if r >= 2.0 && rng.next_f64() < 0.85 {
            let big = if whole { 0.3 } else { 0.2 };
            let mut got = None;
            for q in [4, 1, 3, 5, 7, 0, 2, 6, 8] {
                if let Some(c) = near(q).and_then(|m| giant_at(p, nx, ny, m, &inside, walls, way, big, rng)) {
                    got = Some(c);
                    break;
                }
            }
            if let Some(c) = got {
                at = c;
                // A big stand: one or two more giants at its edge, two squares out.
                let more = if r >= 2.8 { 1 + rng.below(2) } else { rng.below(2) };
                for _ in 0..more {
                    let (i, jj) = ((k % nx) as isize + rng.below(5) as isize - 2, (k / nx) as isize + rng.below(5) as isize - 2);
                    if i >= 0 && jj >= 0 && (i as usize) < nx && (jj as usize) < ny && inside[jj as usize * nx + i as usize] {
                        giant_at(p, nx, ny, jj as usize * nx + i as usize, &inside, walls, way, if whole { 0.15 } else { 0.1 }, rng);
                    }
                }
            }
        }
        hearts.push((at.0, at.1, r));
    }
    let (carpet, lamp) = (rng.below(4) as u8, rng.below(4) as u8);
    fill(p, nx, ny, clump, walls, &hearts, if whole { 2.0 } else { 1.5 }, if whole { 0.5 } else { 0.45 }, carpet, lamp, rng);
}
