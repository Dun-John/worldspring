//! Built levels, graph-first: rooms placed on the level, a spanning tree between them (loops and locks after, in
//! `kit::guard`), corridors dug by least-cost paths, doors where a corridor meets a room (dungeons, crypts,
//! catacombs, keep dungeons, undercrofts).

use crate::core::rng::Pcg32;
use crate::under::*;

#[derive(Clone, Copy)]
pub(crate) struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Rect {
    fn center(&self) -> (f64, f64) {
        (self.x as f64 + self.w as f64 / 2.0, self.y as f64 + self.h as f64 / 2.0)
    }
    /// Overlaps `o` grown by one square of rock all round.
    fn touches(&self, o: &Rect) -> bool {
        self.x < o.x + o.w + 1 && o.x < self.x + self.w + 1 && self.y < o.y + o.h + 1 && o.y < self.y + self.h + 1
    }
}

pub(crate) fn dungeon(nx: usize, ny: usize, arrive: usize, depth: usize, last: bool, theme: &Theme, area: f64, rng: &mut Pcg32) -> Plan {
    let mut p = Plan::new(nx, ny, arrive, false);
    let (ax, ay) = (arrive % nx, arrive / nx);
    let fit = |c: usize, len: usize, n: usize, rng: &mut Pcg32| {
        // A start so that [start, start + len) holds c and stays inside [1, n - 1).
        let lo = c.saturating_sub(len - 1).max(1);
        let hi = c.min(n - 1 - len);
        if hi <= lo { lo } else { lo + rng.below((hi - lo + 1) as u32) as usize }
    };
    let (w, h) = (4 + rng.below(3) as usize, 4 + rng.below(3) as usize);
    let mut rects = vec![Rect { x: fit(ax, w, nx, rng), y: fit(ay, h, ny, rng), w, h }];
    // The boss chamber first on the last level: big, as far from the arrival as it fits.
    let mut boss: Option<usize> = None;
    if last {
        let mut best: Option<(f64, Rect)> = None;
        for _ in 0..80 {
            let (w, h) = (8 + rng.below(3) as usize, 7 + rng.below(3) as usize);
            let r = Rect { x: 1 + rng.below((nx - w - 2) as u32) as usize, y: 1 + rng.below((ny - h - 2) as u32) as usize, w, h };
            if rects[0].touches(&r) {
                continue;
            }
            let (cx, cy) = r.center();
            let d = (cx - ax as f64).hypot(cy - ay as f64);
            if best.is_none_or(|(b, _)| d > b) {
                best = Some((d, r));
            }
        }
        if let Some((_, r)) = best {
            rects.push(r);
            boss = Some(1);
        }
    }
    let mut target = (((theme.rooms.0 + rng.below(theme.rooms.1) as usize) as f64 * area).round() as usize).max(3);
    if theme.sprawl {
        target = target.max(nx * ny / 140).min(40);
    }
    for _ in 0..(600.0 * area.max(1.0)) as usize {
        if rects.len() >= target {
            break;
        }
        let (lo, span) = if theme.sprawl { (4, 6) } else { (3, 5) };
        let (w, h) = (lo + rng.below(span) as usize, lo + rng.below(span) as usize);
        let r = Rect { x: 1 + rng.below((nx - w - 2) as u32) as usize, y: 1 + rng.below((ny - h - 2) as u32) as usize, w, h };
        if rects.iter().all(|o| !o.touches(&r)) {
            rects.push(r);
        }
    }
    let kinds = theme.kinds;
    for (ri, r) in rects.iter().enumerate() {
        let kind = if Some(ri) == boss {
            BOSS
        } else if ri == 0 {
            if depth == 0 { theme.first } else { "landing" }
        } else {
            kinds[rng.below(kinds.len() as u32) as usize]
        };
        let id = p.room(kind, 0.0);
        for j in r.y..r.y + r.h {
            for i in r.x..r.x + r.w {
                p.cells[j * nx + i] = id;
            }
        }
    }
    let passage = p.room(theme.passage, 0.0);
    // Spanning tree over the rooms (not the boss chamber), then loops; the boss chamber hangs
    // off its nearest room only.
    let n = rects.len();
    let cdist = |a: usize, b: usize| {
        let (ca, cb) = (rects[a].center(), rects[b].center());
        (ca.0 - cb.0).hypot(ca.1 - cb.1)
    };
    let normal: Vec<usize> = (0..n).filter(|&i| Some(i) != boss).collect();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut inside = vec![false; n];
    inside[0] = true;
    let mut mst_len = 0.0;
    for _ in 1..normal.len() {
        let mut best: Option<(f64, usize, usize)> = None;
        for &a in normal.iter().filter(|&&a| inside[a]) {
            for &b in normal.iter().filter(|&&b| !inside[b]) {
                let d = cdist(a, b);
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, a, b));
                }
            }
        }
        let Some((d, a, b)) = best else { break };
        inside[b] = true;
        mst_len += d;
        edges.push((a, b));
    }
    let avg = mst_len / edges.len().max(1) as f64;
    // (Loops between near rooms are dug later, once the locks are chosen: `kit::guard`.)
    for (ia, &a) in normal.iter().enumerate() {
        for &b in &normal[ia + 1..] {
            if !edges.contains(&(a, b)) && !edges.contains(&(b, a)) && cdist(a, b) < 1.3 * avg && rng.next_f64() < 0.2 {
                p.shortcuts.push((a as i16, b as i16));
            }
        }
    }
    if let Some(bi) = boss {
        let near = normal.iter().copied().filter(|&a| a != 0 || normal.len() == 1).min_by(|&a, &b| cdist(a, bi).total_cmp(&cdist(b, bi))).unwrap_or(0);
        edges.push((near, bi));
    }
    for (a, b) in edges {
        dig(&mut p, nx, ny, a as i16, b as i16, passage, theme.corridor > 1);
    }
    p.passage = Some((passage, theme.corridor > 1));
    // Rooms the corridors failed to reach (boxed in): back to rock.
    let dist = p.distances(arrive, ny, None);
    for k in 0..nx * ny {
        if p.cells[k] >= 0 && dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    let alive = |p: &Plan, r: i16| p.cells.iter().any(|&c| c == r);
    if let Some(bi) = boss
        && !alive(&p, bi as i16)
    {
        // No way into the boss chamber was found: the farthest room takes its place.
        let far = (0..nx * ny).filter(|&k| p.cells[k] >= 0 && p.cells[k] != passage).max_by_key(|&k| dist[k]).map(|k| p.cells[k]);
        if let Some(r) = far {
            p.rooms[r as usize].kind = BOSS;
        }
    }
    if !last {
        // The way down: the middle of the room farthest from the arrival.
        let far = (0..nx * ny).filter(|&k| p.cells[k] >= 0 && p.cells[k] != passage && p.cells[k] != p.cells[arrive]).max_by_key(|&k| dist[k]);
        let room = far.map(|k| p.cells[k]).unwrap_or(p.cells[arrive]);
        let r = &rects[room as usize];
        let c = (r.y + r.h / 2) * nx + r.x + r.w / 2;
        p.down = Some(if p.cells[c] == room && c != arrive { c } else { far.unwrap_or(arrive) });
    }
    p
}
