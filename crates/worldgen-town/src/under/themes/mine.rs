//! Mines: an adit in from the hillside and a railed haulage way, drifts along curved ore veins with stopes where a
//! vein runs rich (some fallen in), crosscuts between the veins, side drifts off them (most ending in a stope) and
//! bypasses that leave a vein and rejoin it (loops), a winze down to the next level (the way down) and a lift shaft with its winch and
//! cage on the same squares of every level. By the adit a miners' station, its powder store locked (the key elsewhere
//! on the level); now and then a drift broken into a natural cave (`cave::caverns`), joined by rubble; the deepest level
//! now and then partly flooded (shallow water in the drifts, a deep sump with a way round). The boss in a dead end off
//! the deepest level's farthest vein.

use super::cave::{Caverns, DEEP, DRY, SHALLOW, around, caverns, depths, lake};
use crate::core::rng::Pcg32;
use crate::under::*;

/// The chambers of a natural cave a drift breaks into.
const CAVE: [&str; 4] = ["cavern", "grotto", "pool chamber", "bat roost"];
/// The station's rooms along the adit, the powder store last (locked).
const STATION: [&str; 5] = ["tool store", "ore bins", "foreman's office", "bunk room", "powder store"];

/// A level while it is dug: the plan, and the squares that stay rock (round the powder store and the lift shaft).
struct Dig {
    p: Plan,
    nx: usize,
    ny: usize,
    keep: Vec<bool>,
}

impl Dig {
    /// Square (i, j) if inside the ring of rock round the level.
    fn at(&self, i: isize, j: isize) -> Option<usize> {
        (i >= 1 && j >= 1 && i < self.nx as isize - 1 && j < self.ny as isize - 1).then(|| j as usize * self.nx + i as usize)
    }

    /// Rock square (i, j) becomes floor of `room` (not a kept one).
    fn cut(&mut self, i: isize, j: isize, room: i16) {
        if let Some(k) = self.at(i, j)
            && self.p.cells[k] < 0
            && !self.keep[k]
        {
            self.p.cells[k] = room;
        }
    }

    fn rect(&mut self, x0: isize, y0: isize, w: isize, h: isize, room: i16) {
        for j in y0..y0 + h {
            for i in x0..x0 + w {
                self.cut(i, j, room);
            }
        }
    }

    /// Whether a w x h rectangle (none of it kept) and a ring of `pad` round it are all rock, inside the level.
    fn clear(&self, x0: isize, y0: isize, w: isize, h: isize, pad: isize) -> bool {
        let inside = |i: isize, j: isize| i >= x0 && i < x0 + w && j >= y0 && j < y0 + h;
        (y0 - pad..y0 + h + pad).all(|j| (x0 - pad..x0 + w + pad).all(|i| self.at(i, j).is_some_and(|k| self.p.cells[k] < 0 && !(self.keep[k] && inside(i, j)))))
    }

    /// Keep the rock of the ring round a rectangle, but on rows `skip`.
    fn keep_ring(&mut self, x0: isize, y0: isize, w: isize, h: isize, skip: &[isize]) {
        for j in y0 - 1..=y0 + h {
            for i in x0 - 1..=x0 + w {
                let inside = i >= x0 && i < x0 + w && j >= y0 && j < y0 + h;
                if let Some(k) = self.at(i, j)
                    && !inside
                    && !skip.contains(&j)
                    && self.p.cells[k] < 0
                {
                    self.keep[k] = true;
                }
            }
        }
    }

    /// Floor squares reached from `start` (stepping 4 ways), never onto `stop`.
    fn reach(&self, start: usize, stop: &dyn Fn(usize) -> bool) -> Vec<bool> {
        let (nx, ny) = (self.nx, self.ny);
        let mut seen = vec![false; nx * ny];
        let mut q = std::collections::VecDeque::from([start]);
        seen[start] = true;
        while let Some(k) = q.pop_front() {
            for m in neighbours(k, nx, ny) {
                if !seen[m] && self.p.cells[m] >= 0 && !stop(m) {
                    seen[m] = true;
                    q.push_back(m);
                }
            }
        }
        seen
    }

    /// The haulage way: two squares wide (rows y, y + 1) from column `from` to `to`, jogging a row now and then (never
    /// within `steady` squares of `from`).
    fn way(&mut self, from: isize, to: isize, y0: isize, room: i16, steady: isize, rng: &mut Pcg32) {
        let step = if to >= from { 1 } else { -1 };
        let (mut x, mut y) = (from, y0);
        loop {
            self.cut(x, y, room);
            self.cut(x, y + 1, room);
            if x == to {
                break;
            }
            if (x - from).abs() > steady && rng.next_f64() < 0.08 {
                let y2 = (y + if rng.next_f64() < 0.5 { 1 } else { -1 }).clamp(self.ny as isize / 4, 3 * self.ny as isize / 4);
                self.cut(x, y2, room);
                self.cut(x, y2 + 1, room);
                y = y2;
            }
            x += step;
        }
    }

    /// Join every part of the floor to the part `main` lies in by the shortest drift through rock (none longer than 14
    /// squares, but for the part holding `must`); parts that can't be joined go back to rock.
    fn connect(&mut self, main: usize, must: usize, room: i16) {
        let (nx, ny) = (self.nx, self.ny);
        let n = nx * ny;
        for _ in 0..64 {
            let seen = self.reach(main, &|_| false);
            let Some(stray) = (0..n).find(|&k| self.p.cells[k] >= 0 && !seen[k]) else { return };
            let part = self.reach(stray, &|_| false);
            let longest = if part[must] { usize::MAX } else { 14 };
            // (Out through the rock from the part, to a square beside the main floor.)
            let mut from = vec![usize::MAX; n];
            let mut q = std::collections::VecDeque::new();
            for k in 0..n {
                if part[k] {
                    from[k] = k;
                    q.push_back(k);
                }
            }
            let mut end = None;
            let mut len = vec![0usize; n];
            while let Some(k) = q.pop_front() {
                if !part[k] && neighbours(k, nx, ny).any(|m| seen[m]) {
                    end = Some(k);
                    break;
                }
                if len[k] >= longest {
                    continue;
                }
                for m in neighbours(k, nx, ny) {
                    let (i, j) = ((m % nx) as isize, (m / nx) as isize);
                    if from[m] == usize::MAX && self.at(i, j).is_some() && self.p.cells[m] < 0 && !self.keep[m] {
                        from[m] = k;
                        len[m] = len[k] + 1;
                        q.push_back(m);
                    }
                }
            }
            match end {
                Some(mut k) => {
                    while !part[k] {
                        self.p.cells[k] = room;
                        k = from[k];
                    }
                }
                None => {
                    for k in 0..n {
                        if part[k] {
                            self.p.cells[k] = -1;
                        }
                    }
                }
            }
        }
    }
}

/// The rails' squares, in order along each piece of the adit and haulage way: from one end to the other (the two
/// squares of it farthest apart, but from the mouth on the top level), straight on where the way lets them and
/// turning only where it must (so a two-wide way gets one row, a one-wide stretch its squares, and each square is
/// beside the one before).
fn rail_line(p: &Plan, nx: usize, ny: usize) -> Vec<usize> {
    let n = nx * ny;
    let on = |k: usize| {
        let r = p.cells[k];
        r >= 0 && matches!(p.rooms[r as usize].kind, "adit" | "haulage way")
    };
    // (The square a step from `k` heading `d`: north, east, south, west.)
    let step = |k: usize, d: usize| -> Option<usize> {
        let (x, y) = (k % nx, k / nx);
        let m = match d {
            0 if y > 0 => k - nx,
            1 if x + 1 < nx => k + 1,
            2 if y + 1 < ny => k + nx,
            3 if x > 0 => k - 1,
            _ => return None,
        };
        on(m).then_some(m)
    };
    let far = |from: usize| -> (usize, Vec<u32>) {
        let mut dist = vec![u32::MAX; n];
        dist[from] = 0;
        let mut q = std::collections::VecDeque::from([from]);
        let mut best = from;
        while let Some(k) = q.pop_front() {
            if dist[k] > dist[best] || dist[k] == dist[best] && k < best {
                best = k;
            }
            for d in 0..4 {
                if let Some(m) = step(k, d)
                    && dist[m] == u32::MAX
                {
                    dist[m] = dist[k] + 1;
                    q.push_back(m);
                }
            }
        }
        (best, dist)
    };
    let mut done = vec![false; n];
    let mut line = Vec::new();
    for s in std::iter::once(p.up).chain(0..n) {
        if !on(s) || done[s] {
            continue;
        }
        let (a, _) = if s == p.up { (s, Vec::new()) } else { far(s) };
        let (b, dist) = far(a);
        for k in 0..n {
            done[k] |= dist[k] != u32::MAX;
        }
        if a == b {
            continue;
        }
        // The cheapest way from a to b by (square, heading): 10 a step, 25 more a turn (heading 4: none yet).
        let mut cost = vec![u32::MAX; n * 5];
        let mut from = vec![usize::MAX; n * 5];
        let mut heap = std::collections::BinaryHeap::new();
        cost[a * 5 + 4] = 0;
        heap.push(std::cmp::Reverse((0u32, a * 5 + 4)));
        while let Some(std::cmp::Reverse((c, st))) = heap.pop() {
            if c > cost[st] {
                continue;
            }
            let (k, h) = (st / 5, st % 5);
            for d in 0..4 {
                let Some(m) = step(k, d) else { continue };
                let c2 = c + 10 + if h != 4 && h != d { 25 } else { 0 };
                if c2 < cost[m * 5 + d] {
                    cost[m * 5 + d] = c2;
                    from[m * 5 + d] = st;
                    heap.push(std::cmp::Reverse((c2, m * 5 + d)));
                }
            }
        }
        let Some(mut st) = (0..4).map(|d| b * 5 + d).filter(|&st| cost[st] != u32::MAX).min_by_key(|&st| (cost[st], st)) else { continue };
        let mut path = vec![b];
        while from[st] != usize::MAX {
            st = from[st];
            path.push(st / 5);
        }
        path.reverse();
        // (An end square round the corner of a two-wide way's end: dropped, so the line ends straight.)
        let bent = |a: usize, b: usize, c: usize| b.wrapping_sub(a) != c.wrapping_sub(b);
        let m = path.len();
        if m >= 3 && bent(path[m - 3], path[m - 2], path[m - 1]) {
            path.pop();
        }
        if path.len() >= 3 && a != p.up && bent(path[0], path[1], path[2]) {
            path.remove(0);
        }
        line.extend(path);
    }
    line
}

/// One level of a mine. `lift`: the lift shaft from the level above (None on the top level: laid out here).
pub(crate) fn mine(nx: usize, ny: usize, arrive: usize, d: usize, last: bool, lift: Option<(usize, bool, bool)>, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut g = Dig { p: Plan::new(nx, ny, arrive, true), nx, ny, keep: vec![false; n] };
    let (ax, ay) = ((arrive % nx) as isize, (arrive / nx) as isize);
    let toward: isize = if (ax as usize) < nx / 2 { 1 } else { -1 };
    let top = lift.is_none();
    // The station along the adit (top level): rooms off both sides, behind a square of rock, each with an opening.
    let mut station: Vec<(i16, usize, usize)> = Vec::new();
    let (lx, ly, up, cage_top, y0) = match lift {
        Some((at, up, cage_top)) => {
            let (lx, ly) = ((at % nx) as isize, (at / nx) as isize);
            (lx, ly, up, cage_top, if up { ly + 4 } else { ly - 4 })
        }
        None => {
            let y0 = (ay - 1).clamp(6, ny as isize - 8);
            let up = rng.next_f64() < 0.5;
            let mut cursor = [3isize, 3isize];
            let small = nx < 50;
            let mut sides: Vec<usize> = (0..STATION.len()).map(|s| s % 2).collect();
            if rng.next_f64() < 0.5 {
                sides.iter_mut().for_each(|s| *s = 1 - *s);
            }
            let adit = g.p.room("adit", 0.0);
            for (s, &kind) in STATION.iter().enumerate() {
                let side = sides[s];
                let w = if small { 3 } else { 3 + rng.below(2) as isize };
                let h = 3 + rng.below(2) as isize;
                let s0 = cursor[side];
                cursor[side] += w + 1;
                // (Forward squares s0..s0 + w from the mouth; above the way (side 0) or below it.)
                let (x0, x1) = (ax + toward * s0, ax + toward * (s0 + w - 1));
                let x0 = x0.min(x1);
                let (ry, oy, hy) = if side == 0 { (y0 - 1 - h, y0 - 1, y0) } else { (y0 + 3, y0 + 2, y0 + 1) };
                if !g.clear(x0, ry, w, h, 1) {
                    continue;
                }
                let room = g.p.room(kind, 0.0);
                g.rect(x0, ry, w, h, room);
                let ox = x0 + w / 2;
                g.cut(ox, oy, room);
                let (o, hk) = (oy as usize * nx + ox as usize, hy as usize * nx + ox as usize);
                station.push((room, o, hk));
                // (The rock row between the rooms and the way stays, but at the openings.)
                for i in x0 - 1..=x0 + w {
                    if let Some(k) = g.at(i, oy)
                        && g.p.cells[k] < 0
                    {
                        g.keep[k] = true;
                    }
                }
                if kind == "powder store" {
                    g.keep_ring(x0, ry, w, h, &[]);
                    g.keep[o] = false;
                }
            }
            // The lift beyond the station, on the side `up`.
            let s_l = cursor[0].max(cursor[1]) + 2;
            let lx = (ax + toward * s_l).min(ax + toward * (s_l + 1));
            let ly = if up { y0 - 4 } else { y0 + 4 };
            // The adit from the mouth to the lift's landing.
            let far = ax + toward * (s_l + 3);
            g.way(ax, far, y0, adit, isize::MAX, rng);
            g.cut(ax, ay, adit);
            for j in ay.min(y0)..=ay.max(y0 + 1) {
                g.cut(ax, j, adit);
            }
            (lx, ly, up, rng.next_f64() < 0.5, y0)
        }
    };
    // The lift's landing: the shaft at its far side, an apron round it on three sides, beside the haulage way.
    let landing = g.p.room("lift landing", 0.0);
    let (wy0, wh) = if up { (y0 - 4, 4) } else { (y0 + 2, 4) };
    g.rect(lx - 1, wy0, 4, wh, landing);
    g.keep_ring(lx - 1, wy0, 4, wh, &[y0, y0 + 1]);
    let apron = (if up { y0 - 1 } else { y0 + 2 }) as usize * nx + lx as usize;
    let haul = g.p.room("haulage way", 0.0);
    // The winze's foot (below the top level): the way up from the level above.
    let mut foot: Option<i16> = None;
    if !top {
        let f = g.p.room("winze", 0.0);
        g.rect(ax - 1, ay - 1, 3, 3, f);
        g.p.cells[arrive] = f;
        foot = Some(f);
    }
    // The veins: across the level north to south, curved; where the haulage way runs.
    let span = |a: isize, b: isize| (a.min(b), a.max(b));
    let veins_x: Vec<isize> = if top {
        let start = (lx + 1 - ax).abs() + 8;
        let room = (nx as isize - 6) - start;
        let count = (1 + room / 15).clamp(1, 3);
        (0..count)
            .map(|v| {
                let s = if count == 1 { start + room / 2 } else { start + room * v / (count - 1) };
                ax + toward * (s + rng.below(3) as isize - 1).clamp(start, nx as isize - 6)
            })
            .collect()
    } else {
        let count = (1 + (nx as isize - 13) / 15).clamp(1, 3);
        let mut xs: Vec<isize> = Vec::new();
        for v in 0..count {
            let mut x = 6 + (nx as isize - 13) * v / (count - 1).max(1) + rng.below(3) as isize - 1;
            if (x - lx).abs() < 7 {
                x = if x < lx { lx - 7 } else { lx + 8 };
            }
            if x >= 5 && x <= nx as isize - 6 && xs.iter().all(|&o| (o - x).abs() >= 10) {
                xs.push(x);
            }
        }
        if xs.is_empty() {
            xs.push(if lx > nx as isize / 2 { 6 } else { nx as isize - 7 });
        }
        xs
    };
    // The haulage way: on from the landing to the veins (the top level's from the adit).
    let (lo, hi) = veins_x.iter().fold(span(lx - 1, lx + 2), |(a, b), &x| (a.min(x - 1), b.max(x + 1)));
    if top {
        let from = if toward > 0 { lx + 3 } else { lx - 2 };
        let to = if toward > 0 { hi } else { lo };
        g.way(from, to, y0, haul, 3, rng);
    } else {
        g.way(lx - 1, lo, y0, haul, 3, rng);
        g.way(lx + 2, hi, y0, haul, 3, rng);
        g.rect(lx - 1, y0, 4, 2, haul);
    }
    // Which vein end the winze (or the boss) lies beyond: the vein farthest from the way in, at its far end.
    let far_vein = (0..veins_x.len()).max_by_key(|&v| ((veins_x[v] - ax).abs(), v)).unwrap_or(0);
    let south = if ay * 2 == ny as isize { rng.next_f64() < 0.5 } else { ay * 2 < ny as isize };
    let trim = if last { 10 } else { 5 };
    let mut ends: Vec<(isize, isize, isize)> = Vec::new();
    let mut drifts: Vec<i16> = Vec::new();
    let mut rich: Vec<(i16, isize, isize)> = Vec::new();
    let mut stopes: Vec<(i16, bool)> = Vec::new();
    let mut lines: Vec<(i16, Vec<(isize, isize)>)> = Vec::new();
    let rs = rng.next_u32() as u64;
    for (v, &cx) in veins_x.iter().enumerate() {
        let drift = g.p.room("drift", 0.0);
        drifts.push(drift);
        let (mut j0, mut j1) = (2 + rng.below(3) as isize, ny as isize - 3 - rng.below(3) as isize);
        if v == far_vein {
            if south {
                j1 = j1.min(ny as isize - 1 - trim);
            } else {
                j0 = j0.max(trim);
            }
        }
        let (a1, f1, p1) = (rng.range(1.5, 3.5), rng.range(0.1, 0.18), rng.range(0.0, 6.28));
        let (a2, f2, p2) = (rng.range(0.5, 1.3), rng.range(0.3, 0.45), rng.range(0.0, 6.28));
        let tilt = rng.range(-0.2, 0.2);
        let x_at = |j: isize| {
            let t = j as f64;
            let x = cx as f64 + a1 * libm::sin(t * f1 + p1) + a2 * libm::sin(t * f2 + p2) + tilt * (t - ny as f64 / 2.0);
            (crate::core::round(x) as isize).clamp(2, nx as isize - 3)
        };
        let mut prev = x_at(j0);
        let mut line: Vec<(isize, isize)> = Vec::new();
        for j in j0..=j1 {
            let x = x_at(j);
            for i in prev.min(x)..=prev.max(x) {
                g.cut(i, j, drift);
            }
            line.push((x, j));
            prev = x;
        }
        lines.push((drift, line.clone()));
        ends.push((v as isize, line[0].0, line[0].1));
        ends.push((v as isize, line[line.len() - 1].0, line[line.len() - 1].1));
        // Stopes where the vein runs rich: a ragged chamber off the drift, now and then fallen in.
        let richness = |j: isize| noise(rs ^ (v as u64 * 0x9e37), 3, j.max(0) as usize, 4.0);
        let mut last_stope = -100;
        for j in j0 + 3..=j1 - 3 {
            let r = richness(j);
            if r < 0.45 || r < richness(j - 1) || r < richness(j + 1) || j - last_stope < 5 || (j - y0).abs() <= 2 || (j - y0 - 1).abs() <= 2 {
                continue;
            }
            last_stope = j;
            let x = x_at(j);
            rich.push((drift, x, j));
            // (Now and then rich enough for a stope on both sides.)
            let first: isize = if rng.next_f64() < 0.5 { 1 } else { -1 };
            let both = rng.next_f64() < 0.35;
            for side in [first, -first].into_iter().take(1 + both as usize) {
                let (rx, ry) = (rng.range(2.4, 4.4), rng.range(2.0, 3.4));
                let ccx = x as f64 + side as f64 * (rx * 0.6 + 0.5);
                let fallen = rng.next_f64() < 0.3;
                let kind = if fallen { "collapsed stope" } else if rng.next_f64() < 0.7 { "stope" } else { "ore chamber" };
                let st = g.p.room(kind, 0.0);
                let ns = rng.next_u32() as u64;
                for jj in j - 4..=j + 4 {
                    for ii in (ccx - rx - 1.0) as isize..=(ccx + rx + 1.0) as isize {
                        let (dx, dy) = ((ii as f64 - ccx) / rx, (jj - j) as f64 / ry);
                        let wobble = 0.35 * (noise(ns, ii.max(0) as usize, jj.max(0) as usize, 2.0) - 0.5);
                        if dx * dx + dy * dy <= 1.0 + wobble {
                            g.cut(ii, jj, st);
                        }
                    }
                }
                stopes.push((st, fallen));
            }
        }
    }
    // The winze (not on the deepest level): a little chamber beyond the far vein's far end, out of the lift's way; the
    // way down at its middle.
    let near_lift = |i: isize, j: isize| i >= lx - 6 && i <= lx + 7 && j >= ly - 6 && j <= ly + 7;
    if !last {
        let mut pick: Vec<(isize, isize, isize)> = ends.clone();
        pick.sort_by_key(|&(v, _, j)| (v != far_vein as isize || (j * 2 > ny as isize) != south, (j * 2 - ny as isize).abs() * -1));
        let w = g.p.room("winze", 0.0);
        let mut put = None;
        for &(_, x, j) in &pick {
            let (cy, y_top) = if j * 2 > ny as isize { (j + 2, j + 1) } else { (j - 2, j - 3) };
            if !near_lift(x, cy) && g.clear(x - 1, y_top, 3, 3, 0) && Some(cy as usize * nx + x as usize) != Some(arrive) {
                g.rect(x - 1, y_top, 3, 3, w);
                put = Some(cy as usize * nx + x as usize);
                break;
            }
        }
        g.p.down = put;
    }
    let link = g.p.room("drift", 0.0);
    g.connect(apron, arrive, link);
    if !last && g.p.down.is_none_or(|k| g.p.cells[k] < 0) {
        // (No room beyond a vein end: the floor square farthest from the way in, clear of the lift.)
        let dist = g.p.distances(arrive, ny, None);
        g.p.down = (0..n).filter(|&k| dist[k] > 0 && !near_lift((k % nx) as isize, (k / nx) as isize)).max_by_key(|&k| (dist[k], usize::MAX - k));
        if g.p.down.is_none() {
            g.p.down = (0..n).filter(|&k| dist[k] > 0 && g.p.cells[k] != landing).max_by_key(|&k| (dist[k], usize::MAX - k));
        }
    }
    // Denser workings: crosscuts between neighbouring veins, side drifts off each vein (some ending in a stope),
    // bypasses that leave a vein and come back to it further on (loops), drifts off the haulage way. Dug only through
    // rock clear of the station, the lift, the winzes and (deepest level) the boss's end of the far vein.
    let fe = ends.iter().copied().find(|&(v, _, j)| v == far_vein as isize && (j * 2 > ny as isize) == south);
    let ok = |i: isize, j: isize| !(near_lift(i, j) || (last && fe.is_some_and(|(_, x, e)| (i - x).abs() <= 8 && if south { j >= e - 3 } else { j <= e + 3 })));
    workings(&mut g, &lines, (y0, haul), &ok, &mut drifts, &mut stopes, rng);
    // The boss (deepest level): the deep vein, a chamber off the far vein's far end touching nothing else.
    let mut boss: Option<i16> = None;
    if last {
        let b = g.p.room(BOSS, 0.0);
        let dist = g.p.distances(arrive, ny, None);
        let mut seeds: Vec<(usize, (f64, f64))> = Vec::new();
        for &(v, x, j) in &ends {
            if v == far_vein as isize && (j * 2 > ny as isize) == south {
                let s = if south { 1 } else { -1 };
                if let Some(k) = g.at(x, j + s) {
                    seeds.push((k, (x as f64, (j + 5 * s) as f64)));
                }
            }
        }
        let mut far: Vec<usize> = (0..n).filter(|&k| dist[k] > 0).collect();
        far.sort_by_key(|&k| (std::cmp::Reverse(dist[k]), k));
        for &k in far.iter().take(80) {
            for m in neighbours(k, nx, ny) {
                let (dx, dy) = ((m % nx) as f64 - (k % nx) as f64, (m / nx) as f64 - (k / nx) as f64);
                seeds.push((m, ((m % nx) as f64 + dx * 4.0, (m / nx) as f64 + dy * 4.0)));
            }
        }
        let mut best: Vec<usize> = Vec::new();
        for (seed, (cx, cy)) in seeds.into_iter().take(120) {
            let (si, sj) = ((seed % nx) as isize, (seed / nx) as isize);
            if g.at(si, sj).is_none() || g.p.cells[seed] >= 0 || g.keep[seed] || near_lift(si, sj) {
                continue;
            }
            let grown = grow_boss(&g, seed, (cx, cy));
            if grown.len() > best.len() {
                best = grown;
            }
            if best.len() >= 30 {
                break;
            }
        }
        if best.is_empty() {
            // (Never on a real grid: the stope farthest in.)
            let k = far.iter().copied().find(|&k| stopes.iter().any(|s| s.0 == g.p.cells[k])).or(far.first().copied()).unwrap_or(arrive);
            let r = g.p.cells[k];
            best = (0..n).filter(|&m| g.p.cells[m] == r && m != arrive).collect();
        }
        for &k in &best {
            g.p.cells[k] = b;
        }
        boss = Some(b);
    }
    // Now and then a drift has broken into a natural cave: chambers grown in the rock well away from the workings.
    let mut broke: Vec<usize> = Vec::new();
    if rng.next_f64() < 0.35 {
        let shy: Vec<bool> = (0..n)
            .map(|k| {
                let near = |m: usize| {
                    let r = g.p.cells[m];
                    r >= 0 && (Some(r) == boss || r == landing || Some(r) == foot || station.iter().any(|s| s.0 == r) || Some(m) == g.p.down || g.p.rooms[r as usize].kind == "winze")
                };
                g.keep[k] || near(k) || around(k, nx, ny).any(|m| near(m) || around(m, nx, ny).any(near))
            })
            .collect();
        // (How far each square lies from the floor.)
        let mut far = vec![u32::MAX; n];
        let mut q = std::collections::VecDeque::new();
        for k in 0..n {
            if g.p.cells[k] >= 0 {
                far[k] = 0;
                q.push_back(k);
            }
        }
        while let Some(k) = q.pop_front() {
            for m in around(k, nx, ny) {
                if far[m] == u32::MAX {
                    far[m] = far[k] + 1;
                    q.push_back(m);
                }
            }
        }
        let inner = |k: usize| {
            let (i, j) = (k % nx, k / nx);
            i >= 4 && j >= 4 && i + 4 < nx && j + 4 < ny
        };
        let mut best: Vec<usize> = (0..n).filter(|&k| inner(k) && !shy[k] && far[k] >= 6).collect();
        for least in [5, 4] {
            if best.is_empty() {
                best = (0..n).filter(|&k| inner(k) && !shy[k] && far[k] >= least).collect();
            }
        }
        if !best.is_empty() {
            let c = best[rng.below(best.len() as u32) as usize];
            let r = far[c] as f64 + 2.5 + rng.range(0.0, 2.0);
            let r = r.min(11.0);
            let region: Vec<bool> = (0..n)
                .map(|k| {
                    let (dx, dy) = ((k % nx) as f64 - (c % nx) as f64, (k / nx) as f64 - (c / nx) as f64);
                    !shy[k] && g.p.cells[k] < 0 && dx * dx + dy * dy <= r * r
                })
                .collect();
            let cv = Caverns { kinds: &CAVE, ledge: "", spacing: 8.0, seeds: &[], raise: 0.0, squeeze: 0.3, loops: 0.4, reach: 6, dead_end: false, solid: &[] };
            // (The boss hidden from the growth, so no chamber is reached through it.)
            let hid: Vec<usize> = (0..n).filter(|&k| boss.is_some() && g.p.cells[k] == boss.unwrap_or(-1)).collect();
            for &k in &hid {
                g.p.cells[k] = -1;
            }
            let grown = caverns(&mut g.p, ny, &region, &cv, rng);
            for &k in &hid {
                g.p.cells[k] = boss.unwrap_or(-1);
            }
            for j in &grown.joins {
                if j.a < 0 || j.b < 0 {
                    broke.extend(j.squares.iter().copied());
                }
            }
            if broke.is_empty() && !grown.rooms.is_empty() {
                // (Chambers touching the drift through its wall: the squares beside it.)
                broke.extend((0..n).filter(|&k| g.p.cells[k] >= 0 && grown.rooms.contains(&g.p.cells[k]) && neighbours(k, nx, ny).any(|m| g.p.cells[m] >= 0 && !grown.rooms.contains(&g.p.cells[m]))));
            }
        }
    }
    // Everything reached from the way in; the boss a dead end; the powder store only through its door.
    let powder = station.iter().copied().find(|s| g.p.rooms[s.0 as usize].kind == "powder store");
    let is = |g: &Dig, k: usize, r: Option<i16>| r.is_some_and(|r| g.p.cells[k] == r);
    let pr = powder.map(|s| s.0);
    let seen = g.reach(arrive, &|k| is(&g, k, boss) || is(&g, k, pr));
    for k in 0..n {
        if g.p.cells[k] >= 0 && !seen[k] && !is(&g, k, boss) && !is(&g, k, pr) {
            g.p.cells[k] = -1;
        }
    }
    let seen = g.reach(arrive, &|k| is(&g, k, pr));
    for k in 0..n {
        if is(&g, k, boss) && !seen[k] {
            g.p.cells[k] = -1;
        }
    }
    let mut lock = None;
    if let Some((r, o, hk)) = powder {
        let ok = seen[hk] && g.p.cells[o] == r;
        let inside = if ok { g.reach(o, &|k| g.p.cells[k] != r) } else { vec![false; n] };
        for k in 0..n {
            if g.p.cells[k] == r && !inside[k] {
                g.p.cells[k] = -1;
            }
        }
        if ok {
            lock = Some((o.min(hk), o.max(hk)));
        }
    }
    // Doors on the foreman's office, the bunks and the powder store (locked).
    for &(r, o, hk) in &station {
        let kind = g.p.rooms[r as usize].kind;
        if matches!(kind, "foreman's office" | "bunk room") && g.p.cells[o] == r && g.p.cells[hk] >= 0 && neighbours(o, nx, ny).filter(|&m| g.p.cells[m] >= 0 && g.p.cells[m] != r).count() == 1 {
            g.p.door(o, hk);
        }
    }
    if let Some((a, b)) = lock {
        g.p.door(a, b);
        g.p.locks.insert((a, b), 1);
    }
    if let Some(k) = g.p.down
        && g.p.cells[k] < 0
    {
        g.p.down = None;
    }
    let Dig { mut p, keep: _, .. } = g;
    p.lift = Some(((ly as usize) * nx + lx as usize, up, cage_top));
    // Things in place: the ways and doorways stay clear.
    p.reserve_doors();
    p.taken[arrive] = true;
    if let Some(k) = p.down {
        p.taken[k] = true;
    }
    let shaft: Vec<usize> = (0..4).map(|s| (ly as usize + s / 2) * nx + lx as usize + s % 2).collect();
    let flood = last && d > 0 && rng.next_f64() < 0.4;
    let mut wet = vec![DRY; n];
    if shaft.iter().all(|&k| p.cells[k] == landing) {
        let (sx, sy) = (lx as usize, ly as usize);
        if (cage_top && top) || (!cage_top && last) {
            p.put(item("cage", sx, sy, 2, 2));
        } else if !last {
            // (Items keep their catalogue names: a design copies only kinds and squares.)
            p.put(item("chasm", sx, sy, 2, 2));
        } else if flood {
            for &k in &shaft {
                wet[k] = DEEP;
            }
        } else {
            p.put(item("rubble", sx, sy + if up { 1 } else { 0 }, 1, 1));
            for &k in &shaft {
                p.taken[k] = true;
            }
        }
        if top {
            // The winch at the shaft's head, on the apron beside it.
            let (side_x, side_y) = ([lx - 1, lx + 2], if up { [ly, ly + 1] } else { [ly + 1, ly] });
            'winch: for y in side_y {
                for x in side_x {
                    if p.place(item("winch", x as usize, y as usize, 1, 1), Some(landing), ny) {
                        break 'winch;
                    }
                }
            }
        }
    }
    // The rails: one line down the adit and the haulage way from end to end, ore carts on it; the rails on every
    // square of it left free (a doorway's too: they don't block it).
    let line = rail_line(&p, nx, ny);
    let free: Vec<usize> = line.iter().copied().filter(|&k| !p.taken[k]).collect();
    if !free.is_empty() {
        for _ in 0..1 + rng.below(2) + top as u32 {
            let k = free[rng.below(free.len() as u32) as usize];
            p.place(item("ore_cart", k % nx, k / nx, 1, 1), None, ny);
        }
    }
    // (A doorway's squares are taken only to keep them clear; the ways on and off the level are kept for their own.)
    let mut open = vec![false; n];
    for &(a, b) in &p.doors {
        open[a] = true;
        open[b] = true;
    }
    for it in &p.items {
        for j in it.y as usize..(it.y + it.h) as usize {
            for i in it.x as usize..(it.x + it.w) as usize {
                open[j * nx + i] = false;
            }
        }
    }
    open[arrive] = false;
    if let Some(k) = p.down {
        open[k] = false;
    }
    let mut on = vec![false; n];
    for &k in &line {
        on[k] = !p.taken[k] || open[k];
    }
    let carts: Vec<usize> = p.items.iter().filter(|it| it.kind == "ore_cart").map(|it| it.y as usize * nx + it.x as usize).collect();
    // (None left alone between the ways on and off the level.)
    let rails: Vec<Item> = line
        .iter()
        .filter(|&&k| on[k] && neighbours(k, nx, ny).any(|m| on[m] || carts.contains(&m)))
        .map(|&k| item("rail", k % nx, k / nx, 1, 1))
        .collect();
    p.put_all(rails);
    // The vein shows in the drift's walls where it runs rich.
    for &(r, x, j) in &rich {
        for k in [j as usize * nx + x as usize - 1, j as usize * nx + x as usize + 1] {
            if p.cells[k] == r && neighbours(k, nx, ny).any(|m| p.cells[m] < 0) && p.place(item("ore_vein", k % nx, k / nx, 1, 1), Some(r), ny) {
                break;
            }
        }
    }
    // Fallen stopes: rubble slopes where they meet the drift, a collapsed wall.
    for &(st, fallen) in &stopes {
        let sq: Vec<usize> = (0..n).filter(|&k| p.cells[k] == st).collect();
        if !fallen || sq.is_empty() {
            continue;
        }
        let join: Vec<usize> = sq.iter().copied().filter(|&k| neighbours(k, nx, ny).any(|m| drifts.contains(&p.cells[m]))).collect();
        let slope: Vec<Item> = join.iter().filter(|&&k| !p.taken[k]).map(|&k| item("rubble_slope", k % nx, k / nx, 1, 1)).collect();
        p.put_all(slope);
        let edge: Vec<usize> = sq.iter().copied().filter(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] < 0)).collect();
        for _ in 0..1 + rng.below(2) {
            if !edge.is_empty() {
                let k = edge[rng.below(edge.len() as u32) as usize];
                p.place(item("collapsed_wall", k % nx, k / nx, 1, 1), Some(st), ny);
            }
        }
    }
    // The break into the cave: rubble over the joins, a collapsed wall on the drift's side.
    let mut broke: Vec<usize> = broke.into_iter().filter(|&k| p.cells[k] >= 0 && !p.taken[k]).collect();
    broke.sort_unstable();
    broke.dedup();
    if let Some(&k) = broke.first() {
        let side = neighbours(k, nx, ny).find(|&m| p.cells[m] >= 0 && !p.taken[m] && !CAVE.contains(&p.rooms[p.cells[m] as usize].kind));
        p.put_all(broke.iter().map(|&k| item("rubble_slope", k % nx, k / nx, 1, 1)).collect::<Vec<_>>());
        if let Some(m) = side {
            let r = p.cells[m];
            p.place(item("collapsed_wall", m % nx, m / nx, 1, 1), Some(r), ny);
        }
    }
    // The powder store's key: in the foreman's office, or else at the end of a stope or drift (never behind the lock).
    if lock.is_some() {
        let office = station.iter().find(|s| p.rooms[s.0 as usize].kind == "foreman's office" && p.cells[s.1] == s.0).map(|s| s.0);
        let mut homes: Vec<i16> = Vec::new();
        if let Some(o) = office.filter(|_| rng.next_f64() < 0.6) {
            homes.push(o);
        }
        homes.extend(stopes.iter().filter(|s| !s.1).map(|s| s.0));
        homes.extend(office);
        homes.extend(drifts.iter().copied());
        let mut put = false;
        'key: for r in homes {
            let sq: Vec<usize> = (0..n).filter(|&k| p.cells[k] == r && !p.taken[k]).collect();
            for _ in 0..12 {
                if sq.is_empty() {
                    break;
                }
                let k = sq[rng.below(sq.len() as u32) as usize];
                let mut key = item("key", k % nx, k / nx, 1, 1);
                key.variant = Some(1);
                if p.place(key, Some(r), ny) {
                    put = true;
                    break 'key;
                }
            }
        }
        if !put {
            // (Never on a real level: the door left unlocked.)
            p.locks.clear();
        }
    }
    // The deepest level now and then partly flooded: shallow water in the low drifts, the biggest stope a deep sump
    // (a way round it), the shaft's foot under water.
    if flood {
        let hs = rng.next_u32() as u64;
        for k in 0..n {
            let r = p.cells[k];
            if r < 0 || Some(r) == boss || Some(r) == foot || p.taken[k] {
                continue;
            }
            if noise(hs, k % nx, k / nx, 8.0) < 0.5 {
                wet[k] = SHALLOW;
            }
        }
        let size = |r: i16| p.cells.iter().filter(|&&c| c == r).count();
        // (The biggest stope with deep enough a middle.)
        if let Some(&(r, _)) = stopes.iter().filter(|s| !s.1 && size(s.0) >= 12 && depths(&p, ny, s.0).iter().any(|d| d.1 >= 2)).max_by_key(|s| (size(s.0), -s.0)) {
            p.rooms[r as usize].kind = "sump";
            lake(&p, ny, &mut wet, r, false, false, rng);
        }
    }
    if wet.iter().any(|&w| w != DRY) {
        p.water = wet;
    }
    // (Rooms nothing was dug for go: a design keeps only rooms with squares.)
    let mut used = vec![false; p.rooms.len()];
    for &r in &p.cells {
        if r >= 0 {
            used[r as usize] = true;
        }
    }
    let mut to = vec![-1i16; p.rooms.len()];
    let mut next = 0i16;
    for (r, &u) in used.iter().enumerate() {
        if u {
            to[r] = next;
            next += 1;
        }
    }
    let mut r = 0;
    p.rooms.retain(|_| {
        r += 1;
        used[r - 1]
    });
    for c in p.cells.iter_mut().filter(|c| **c >= 0) {
        *c = to[*c as usize];
    }
    p
}

/// The boss chamber grown from rock square `seed` toward (cx, cy): squares in a ragged oval there, each step out
/// beside the chamber, none touching other floor (but the seed, beside the drift it opens off).
fn grow_boss(g: &Dig, seed: usize, (cx, cy): (f64, f64)) -> Vec<usize> {
    let (nx, ny) = (g.nx, g.ny);
    let mut own = vec![false; nx * ny];
    own[seed] = true;
    let mut out = vec![seed];
    let mut q = std::collections::VecDeque::from([seed]);
    let (rx, ry) = (5.5, 4.5);
    while let Some(k) = q.pop_front() {
        for m in neighbours(k, nx, ny) {
            let (i, j) = ((m % nx) as isize, (m / nx) as isize);
            if own[m] || g.at(i, j).is_none() || g.p.cells[m] >= 0 || g.keep[m] {
                continue;
            }
            let (dx, dy) = ((i as f64 - cx) / rx, (j as f64 - cy) / ry);
            if dx * dx + dy * dy > 1.0 || around(m, nx, ny).any(|t| g.p.cells[t] >= 0 && !own[t]) {
                continue;
            }
            own[m] = true;
            out.push(m);
            q.push_back(m);
        }
    }
    out
}

/// Rooms a new working may open into.
fn open(p: &Plan, r: i16) -> bool {
    r >= 0 && matches!(p.rooms[r as usize].kind, "drift" | "stope" | "ore chamber" | "haulage way")
}

/// Dig `path` (4-joined squares in order) as `room` if each square is rock that may be dug (`ok`) and only its first
/// `ends.0` and last `ends.1` squares touch other floor (and only rooms a working may open into). With `part`, the
/// good beginning instead, if at least that long (a dead end). The squares dug.
fn bore(g: &mut Dig, path: &[(isize, isize)], room: i16, ends: (usize, usize), part: Option<usize>, ok: &dyn Fn(isize, isize) -> bool) -> Vec<usize> {
    let (nx, ny) = (g.nx, g.ny);
    let ks: Vec<Option<usize>> = path.iter().map(|&(i, j)| g.at(i, j)).collect();
    let mut good = 0;
    for (t, &k) in ks.iter().enumerate() {
        let Some(k) = k else { break };
        let (i, j) = path[t];
        let edge = t < ends.0 || t + ends.1 >= path.len();
        let fine = g.p.cells[k] < 0
            && !g.keep[k]
            && ok(i, j)
            && around(k, nx, ny).all(|m| g.p.cells[m] < 0 || g.p.cells[m] == room || ks.contains(&Some(m)) || (edge && open(&g.p, g.p.cells[m])));
        if !fine {
            break;
        }
        good = t + 1;
    }
    let take = match part {
        _ if good == path.len() => good,
        Some(least) if good >= least => good,
        _ => return Vec::new(),
    };
    let out: Vec<usize> = ks[..take].iter().flatten().copied().collect();
    for &k in &out {
        g.p.cells[k] = room;
    }
    out
}

/// A line of squares from (x, y) heading (dx, dy) until the square ahead is floor `stop` allows (the line) or `most`
/// squares were laid (None); a sideways jog now and then (4-joined).
fn toward(g: &Dig, (mut x, mut y): (isize, isize), (dx, dy): (isize, isize), most: usize, jog: f64, stop: &dyn Fn(i16) -> bool, rng: &mut Pcg32) -> Option<Vec<(isize, isize)>> {
    let mut out: Vec<(isize, isize)> = Vec::new();
    let ahead = |x: isize, y: isize| g.at(x + dx, y + dy).is_some_and(|k| stop(g.p.cells[k]));
    let mut since = 0;
    while out.len() < most {
        out.push((x, y));
        if ahead(x, y) {
            return Some(out);
        }
        if since > 1 && rng.next_f64() < jog {
            let s = if rng.next_f64() < 0.5 { 1 } else { -1 };
            (x, y) = (x + dy * s, y + dx * s);
            out.push((x, y));
            since = 0;
            if ahead(x, y) {
                return Some(out);
            }
        }
        (x, y) = (x + dx, y + dy);
        since += 1;
    }
    None
}

/// A stope round (cx, cy) at the end of `drift`: rock squares in a ragged oval touching no floor but its own and the
/// drift's. How many squares it took.
fn stope_at(g: &mut Dig, (cx, cy): (f64, f64), (rx, ry): (f64, f64), drift: i16, st: i16, ok: &dyn Fn(isize, isize) -> bool, ns: u64) -> usize {
    let (nx, ny) = (g.nx, g.ny);
    let mut want: Vec<usize> = Vec::new();
    for jj in (cy - ry - 1.0) as isize..=(cy + ry + 1.0) as isize {
        for ii in (cx - rx - 1.0) as isize..=(cx + rx + 1.0) as isize {
            let (dx, dy) = ((ii as f64 - cx) / rx, (jj as f64 - cy) / ry);
            let wobble = 0.35 * (noise(ns, ii.max(0) as usize, jj.max(0) as usize, 2.0) - 0.5);
            if dx * dx + dy * dy <= 1.0 + wobble
                && let Some(k) = g.at(ii, jj)
                && g.p.cells[k] < 0
                && !g.keep[k]
                && ok(ii, jj)
                && around(k, nx, ny).all(|m| g.p.cells[m] < 0 || g.p.cells[m] == drift || g.p.cells[m] == st)
            {
                want.push(k);
            }
        }
    }
    for &k in &want {
        g.p.cells[k] = st;
    }
    want.len()
}

/// A side drift from (x, y) heading (dx, dy) into the rock, `len` squares or as far as it may go (4 at least), now and
/// then opening into a stope at its end; it goes in `drifts`, the stope in `stopes`. The drift's squares.
#[allow(clippy::too_many_arguments)]
fn spur(g: &mut Dig, (x, y): (isize, isize), (dx, dy): (isize, isize), len: u32, stope: f64, ok: &dyn Fn(isize, isize) -> bool, drifts: &mut Vec<i16>, stopes: &mut Vec<(i16, bool)>, rng: &mut Pcg32) -> Vec<usize> {
    let mut path: Vec<(isize, isize)> = Vec::new();
    let (mut px, mut py) = (x, y);
    for _ in 0..len {
        path.push((px, py));
        if path.len() > 2 && rng.next_f64() < 0.15 {
            let s = if rng.next_f64() < 0.5 { 1 } else { -1 };
            (px, py) = (px + dy * s, py + dx * s);
            path.push((px, py));
        }
        (px, py) = (px + dx, py + dy);
    }
    let room = g.p.room("drift", 0.0);
    let dug = bore(g, &path, room, (2, 0), Some(4), ok);
    if dug.is_empty() {
        return dug;
    }
    drifts.push(room);
    if rng.next_f64() < stope {
        let end = *dug.last().unwrap();
        let (ex, ey) = ((end % g.nx) as f64, (end / g.nx) as f64);
        let (rx, ry) = if dx != 0 { (rng.range(2.2, 3.8), rng.range(1.8, 3.0)) } else { (rng.range(2.0, 3.4), rng.range(2.0, 3.4)) };
        let fallen = rng.next_f64() < 0.2;
        let st = g.p.room(if fallen { "collapsed stope" } else if rng.next_f64() < 0.7 { "stope" } else { "ore chamber" }, 0.0);
        let ns = rng.next_u32() as u64;
        let at = (ex + dx as f64 * (rx + 0.5), ey + dy as f64 * (ry + 0.5));
        if stope_at(g, at, (rx, ry), room, st, ok, ns) >= 4 {
            stopes.push((st, fallen));
        }
    }
    dug
}

/// The workings between and beside the veins (see `mine`): new drifts go in `drifts`, new stopes in `stopes`.
fn workings(g: &mut Dig, lines: &[(i16, Vec<(isize, isize)>)], (y0, haul): (isize, i16), ok: &dyn Fn(isize, isize) -> bool, drifts: &mut Vec<i16>, stopes: &mut Vec<(i16, bool)>, rng: &mut Pcg32) {
    let (nx, ny) = (g.nx, g.ny as isize);
    let clear_of_haul = |j: isize| j < y0 - 3 || j > y0 + 4;
    let x_of = |line: &[(isize, isize)], j: isize| line.iter().find(|q| q.1 == j).map(|q| q.0);
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by_key(|&v| lines[v].1[lines[v].1.len() / 2].0);
    // Bypasses: out of a vein a few squares, along it and back in (a loop or two a vein).
    for (_, line) in lines {
        let len = line.len() as isize;
        let (mut made, want) = (0, 1 + (len > 30) as usize);
        for _ in 0..10 {
            if made >= want {
                break;
            }
            let (x, j) = line[3 + rng.below((len - 6).max(1) as u32) as usize];
            let s: isize = if rng.next_f64() < 0.5 { 1 } else { -1 };
            let (out, along) = (2 + rng.below(4) as isize, 6 + rng.below(6) as isize);
            let t: isize = if j * 2 < ny { 1 } else { -1 };
            if !clear_of_haul(j) || !clear_of_haul(j + t * along) {
                continue;
            }
            let mut path: Vec<(isize, isize)> = (1..=out).map(|d| (x + s * d, j)).collect();
            let (mut px, mut py) = (x + s * out, j);
            for _ in 0..along {
                py += t;
                if rng.next_f64() < 0.15 {
                    let qx = px + if rng.next_f64() < 0.5 { 1 } else { -1 };
                    if (qx - x) * s >= 2 {
                        path.push((px, py));
                        px = qx;
                    }
                }
                path.push((px, py));
            }
            let Some(back) = toward(g, (px - s, py), (-s, 0), 10, 0.0, &|r| open(&g.p, r), rng) else { continue };
            path.extend(back);
            let room = g.p.room("drift", 0.0);
            if !bore(g, &path, room, (2, 2), None, ok).is_empty() {
                drifts.push(room);
                made += 1;
            }
        }
    }
    // Crosscuts: between each two neighbouring veins, at a few heights off the haulage way (loops with it); a spur or
    // two up or down off each.
    let mut cuts: Vec<Vec<usize>> = Vec::new();
    for w in order.windows(2) {
        let (la, (db, lb)) = (&lines[w[0]].1, (lines[w[1]].0, &lines[w[1]].1));
        let want = 2 + rng.below(2) as usize + (ny > 45) as usize;
        let mut made = 0;
        for _ in 0..14 {
            if made >= want {
                break;
            }
            let j = 3 + rng.below((ny - 6) as u32) as isize;
            let (Some(xa), Some(_)) = (x_of(la, j), x_of(lb, j)) else { continue };
            if !clear_of_haul(j) {
                continue;
            }
            let Some(path) = toward(g, (xa + 1, j), (1, 0), 60, 0.12, &|r| r == db, rng) else { continue };
            let room = g.p.room("drift", 0.0);
            let dug = bore(g, &path, room, (2, 2), None, ok);
            if !dug.is_empty() {
                drifts.push(room);
                cuts.push(dug);
                made += 1;
            }
        }
    }
    for cut in &cuts {
        for _ in 0..1 + (cut.len() > 12) as usize + rng.below(2) as usize {
            if cut.len() < 8 {
                break;
            }
            let k = cut[3 + rng.below(cut.len() as u32 - 6) as usize];
            let t: isize = if rng.next_f64() < 0.5 { 1 } else { -1 };
            let (x, j) = ((k % nx) as isize, (k / nx) as isize);
            spur(g, (x, j + t), (0, t), 4 + rng.below(7), 0.7, ok, drifts, stopes, rng);
        }
    }
    // Off each vein: side drifts (dead ends, most opening into a stope).
    for (_, line) in lines {
        let len = line.len() as isize;
        for _ in 0..3 + (len / 5) as usize {
            let (x, j) = line[3 + rng.below((len - 6).max(1) as u32) as usize];
            let s: isize = if rng.next_f64() < 0.5 { 1 } else { -1 };
            if !clear_of_haul(j) {
                continue;
            }
            spur(g, (x + s, j), (s, 0), 5 + rng.below(9), 0.7, ok, drifts, stopes, rng);
        }
    }
    // Off the haulage way: drifts up or down into the rock between the veins.
    let row: Vec<usize> = (0..g.p.cells.len()).filter(|&k| g.p.cells[k] == haul).collect();
    for _ in 0..2 + rng.below(2) + (nx / 40) as u32 {
        if row.is_empty() {
            break;
        }
        let k = row[rng.below(row.len() as u32) as usize];
        let (x, j) = ((k % nx) as isize, (k / nx) as isize);
        let t: isize = if g.p.cells[k - nx] != haul { -1 } else if g.p.cells[k + nx] != haul { 1 } else { continue };
        spur(g, (x, j + t), (0, t), 6 + rng.below(7), 0.6, ok, drifts, stopes, rng);
    }
}
