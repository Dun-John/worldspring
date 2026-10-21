//! The zone engine's toolkit, shared by every programme: rooms cut along a run by heavy-tailed
//! lengths (`cuts`, `strip`), rooms of sound proportions (`sound`, `cut`), a storey tidied and varied
//! (`tidy`, `vary`, `trim_landings`), squares no zone took given away (`leftovers`, `absorb`,
//! `near_ways`), a box divided off a passage (`partition`), the doors that keep every room within
//! a room of a way about the floor (`links`, `near_links`), a roof's box (`roofed`) and room kinds
//! dealt out by size (`assign`).

use super::{Kinds, Piece, Plan};
use crate::core::rng::Pcg32;
use crate::interior::{Hints, Item, ROOF_SLOPE, Room, is_circulation};
use crate::town::geom::{self, P};

/// How far `p` lies inside polygon `poly` (its nearest edge; negative outside).
pub(super) fn depth(poly: &[P], p: P) -> f64 {
    let n = poly.len();
    let d = (0..n).map(|k| geom::seg_dist(p, poly[k], poly[(k + 1) % n])).fold(f64::MAX, f64::min);
    if geom::contains(poly, p) { d } else { -d }
}

/// Lengths summing to `len`: `n` pieces (fewer where `min` each won't fit) of at least `min`, by
/// heavy-tailed weights, so the rooms cut by them are of very different sizes.
pub(super) fn cuts(len: usize, n: usize, min: usize, rng: &mut Pcg32) -> Vec<usize> {
    let n = n.min(len / min.max(1)).max(1);
    if len < min * n {
        return vec![len];
    }
    let w: Vec<f64> = (0..n).map(|_| 1.0 / (0.15 + 0.85 * rng.next_f64())).collect();
    let total: f64 = w.iter().sum();
    let spare = len - n * min;
    let raw: Vec<f64> = w.iter().map(|x| spare as f64 * x / total).collect();
    let mut out: Vec<usize> = raw.iter().map(|x| min + crate::core::floor(*x) as usize).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| (raw[b] - crate::core::floor(raw[b])).total_cmp(&(raw[a] - crate::core::floor(raw[a]))).then(a.cmp(&b)));
    let mut k = 0;
    while out.iter().sum::<usize>() < len {
        out[order[k % n]] += 1;
        k += 1;
    }
    out
}

/// A floor in the making.
#[derive(Clone)]
pub(super) struct Lay {
    pub(super) cells: Vec<i16>,
    pub(super) rooms: Vec<Room>,
    pub(super) hints: Hints,
    pub(super) items: Vec<Item>,
    /// The ground floor's screens passage and hall (for the front door, once the forebuilding is
    /// laid).
    pub(super) entry: Option<(i16, i16)>,
}

impl Lay {
    pub(super) fn room(&mut self, kind: &'static str) -> i16 {
        self.rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        (self.rooms.len() - 1) as i16
    }

    pub(super) fn count(&self, r: i16) -> usize {
        self.cells.iter().filter(|&&c| c == r).count()
    }
}

/// The door edge between two side-by-side grid squares (as `connect` names edges).
pub(super) fn edge(a: (isize, isize), b: (isize, isize)) -> Option<(usize, usize, bool)> {
    let (x, y) = (a.0.min(b.0), a.1.min(b.1));
    if x < 0 || y < 0 || (a.0 - b.0).abs() + (a.1 - b.1).abs() != 1 {
        return None;
    }
    Some((x as usize, y as usize, a.1 == b.1))
}

/// The front door's side (0 top, 1 right, 2 bottom, 3 left) out of square `a` to square `b`.
pub(super) fn side(a: (isize, isize), b: (isize, isize)) -> usize {
    match (b.0 - a.0, b.1 - a.1) {
        (0, -1) => 0,
        (1, 0) => 1,
        (0, 1) => 2,
        _ => 3,
    }
}

impl Plan {
    pub(super) fn at(&self, s: isize, t: isize) -> Option<usize> {
        let (i, j) = self.f.grid(s, t);
        (i >= 0 && j >= 0 && (i as usize) < self.nx && (j as usize) < self.ny).then(|| j as usize * self.nx + i as usize)
    }

    pub(super) fn body(&self, s: isize, t: isize) -> Option<usize> {
        self.at(s, t).filter(|&k| self.piece[k] == Piece::Body)
    }

    /// Body squares s0..s1, t0..t1 to room `r`.
    pub(super) fn fill(&self, lay: &mut Lay, s0: usize, s1: usize, t0: usize, t1: usize, r: i16) {
        for s in s0..s1 {
            for t in t0..t1 {
                if let Some(k) = self.body(s as isize, t as isize) {
                    lay.cells[k] = r;
                }
            }
        }
    }

    /// How many body squares s0..s1, t0..t1 holds.
    pub(super) fn squares(&self, s0: isize, s1: isize, t0: isize, t1: isize) -> usize {
        (s0..s1).map(|s| (t0..t1).filter(|&t| self.body(s, t).is_some()).count()).sum()
    }

    /// The body's squares s0..s1, t0..t1 no room has yet but `or`.
    pub(super) fn open_squares(&self, lay: &Lay, s0: isize, s1: isize, (t0, t1): (isize, isize), or: i16) -> Vec<usize> {
        (s0..s1).flat_map(|s| (t0..t1).map(move |t| (s, t))).filter_map(|(s, t)| self.body(s, t)).filter(|&k| lay.cells[k] < 0 || lay.cells[k] == or).collect()
    }

    /// A hint: a door between room `a`'s square (s, t) and room `b`'s square beside it (body frame).
    pub(super) fn door(&self, lay: &mut Lay, from: (isize, isize), to: (isize, isize)) {
        let (Some(ka), Some(kb)) = (self.at(from.0, from.1), self.at(to.0, to.1)) else { return };
        let (a, b) = (lay.cells[ka], lay.cells[kb]);
        if a < 0 || b < 0 || a == b {
            return;
        }
        if let Some(e) = edge(self.f.grid(from.0, from.1), self.f.grid(to.0, to.1)) {
            lay.hints.doors.push(((a.min(b), a.max(b)), e));
        }
    }

    /// Rooms cut along s from `s0..s1` (t0..t1 across): `n` of them by heavy-tailed lengths.
    pub(super) fn strip(&self, lay: &mut Lay, s0: usize, s1: usize, t0: usize, t1: usize, n: usize, rng: &mut Pcg32) -> Vec<i16> {
        if s1 <= s0 || t1 <= t0 {
            return Vec::new();
        }
        let mut s = s0;
        let mut out = Vec::new();
        for l in cuts(s1 - s0, n, 2, rng) {
            let r = lay.room("chamber");
            self.fill(lay, s, s + l, t0, t1, r);
            out.push(r);
            s += l;
        }
        out
    }

    /// How many rooms a run of `len` squares holds (about one per 4 to 5).
    pub(super) fn rooms_in(len: usize, rng: &mut Pcg32) -> usize {
        (crate::core::round(len as f64 / 4.5) as usize + (rng.next_f64() < 0.3) as usize).max(1)
    }

    /// Squares sound as a room: `least` or more, a 2 x 2 block among them, in one piece, no longer
    /// than 2.5 times their width by their box.
    pub(super) fn sound(&self, sq: &[usize], least: usize) -> bool {
        let nx = self.nx;
        if sq.len() < least || sq.is_empty() {
            return false;
        }
        let has = |k: usize| sq.contains(&k);
        let block = sq.iter().any(|&k| k % nx + 1 < nx && has(k + 1) && has(k + nx) && has(k + nx + 1));
        let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
        let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut seen = vec![sq[0]];
        let mut i = 0;
        while i < seen.len() {
            let k = seen[i];
            i += 1;
            for q in [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx] {
                let adj = (q == k + 1 && k % nx + 1 < nx) || (q.wrapping_add(1) == k && k % nx > 0) || q == k + nx || q.wrapping_add(nx) == k;
                if adj && has(q) && !seen.contains(&q) {
                    seen.push(q);
                }
            }
        }
        block && seen.len() == sq.len() && 2 * w.max(h) <= 5 * w.min(h)
    }

    /// Room `r` cut across its longer side in two sound rooms (`sound`: the part keeping its kind
    /// `least` squares or more, the other, of kind `kind`, `other`), each a room from a way about the
    /// floor at most (on one, beside a room on one, or beside the other part on one); both on a way
    /// first, then the most even. The new room.
    pub(super) fn cut(&self, lay: &mut Lay, r: i16, least: usize, other: usize, kind: &'static str) -> Option<i16> {
        self.cut_avoiding(lay, r, least, other, kind, 0)
    }

    /// `cut`, neither part of `avoid` squares (0: any).
    pub(super) fn cut_avoiding(&self, lay: &mut Lay, r: i16, least: usize, other: usize, kind: &'static str, avoid: usize) -> Option<i16> {
        let (nx, ny) = (self.nx, self.ny);
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        let st = self.stairs;
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        let on_way = |lay: &Lay, sq: &[usize]| sq.iter().any(|&k| nbrs(k).any(|q| on_stairs(q) || (lay.cells[q] >= 0 && is_circulation(lay.rooms[lay.cells[q] as usize].kind))));
        // (Beside a room other than `r` that lies on a way: a room from one.)
        let by_way = |lay: &Lay, sq: &[usize]| {
            let mut seen: Vec<i16> = Vec::new();
            sq.iter().any(|&k| {
                nbrs(k).any(|q| {
                    let o = lay.cells[q];
                    if o < 0 || o == r || seen.contains(&o) || is_circulation(lay.rooms[o as usize].kind) || !crate::interior::walkable(lay.rooms[o as usize].kind) {
                        return false;
                    }
                    seen.push(o);
                    let osq: Vec<usize> = (0..nx * ny).filter(|&p| lay.cells[p] == o).collect();
                    crate::interior::is_way_through(lay.rooms[o as usize].kind) || on_way(lay, &osq)
                })
            })
        };
        let sq: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == r).collect();
        let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
        let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
        let along_x = x1 - x0 > y1 - y0;
        let at = |k: usize| if along_x { k % nx - x0 } else { k / nx - y0 };
        let long = if along_x { x1 - x0 + 1 } else { y1 - y0 + 1 };
        let best = (1..long)
            .filter_map(|c| {
                let (a, b): (Vec<usize>, Vec<usize>) = sq.iter().partition(|&&k| at(k) < c);
                // (The part keeping its kind: the one on a way, else the bigger.)
                let keep_a = on_way(lay, &a) && (!on_way(lay, &b) || a.len() >= b.len());
                let (k, o) = if keep_a { (&a, &b) } else { (&b, &a) };
                let near = |p: &Vec<usize>, q: &Vec<usize>| on_way(lay, p) || by_way(lay, p) || on_way(lay, q);
                (self.sound(k, least) && self.sound(o, other) && near(k, o) && near(o, k) && a.len() != avoid && b.len() != avoid).then(|| ((on_way(lay, k) && on_way(lay, o)) as usize, std::cmp::Reverse(a.len().abs_diff(b.len())), c, keep_a))
            })
            .max_by(|p, q| (p.0, p.1).cmp(&(q.0, q.1)).then(q.2.cmp(&p.2)))?;
        let (_, _, c, keep_a) = best;
        let id = lay.room(kind);
        for &k in &sq {
            if (at(k) < c) != keep_a {
                lay.cells[k] = id;
            }
        }
        Some(id)
    }

    /// Rooms of sound proportions on a storey above the hall: a room (not a way about the floor, a
    /// hall or a top) longer than 2.5 times its width, or over 40 squares (a great chamber 60), cut
    /// across into rooms that are not, each of 4
    /// squares or more with a 2 x 2 block, in one piece, the part by a way about the floor (or the
    /// stairs) keeping its kind (a great chamber or a solar 9 squares at least), the rest chambers
    /// (`free`); a room filling under 60% of its box (a tail a square wide off it) giving the squares
    /// outside its largest box to a way about the floor beside them, else to a room beside them that
    /// stays sound. Cuts first where both parts lie on a way.
    pub(super) fn tidy(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        let fixed = |kind: &str| is_circulation(kind) || !crate::interior::walkable(kind) || matches!(kind, "great hall" | "mess hall" | "battlements" | "wall walk");
        let lord = |kind: &str| matches!(kind, "great chamber" | "solar");
        let boxed = |sq: &[usize]| {
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
        };
        for _ in 0..3 * lay.rooms.len() {
            let mut changed = false;
            for r in 0..lay.rooms.len() as i16 {
                let kind = lay.rooms[r as usize].kind;
                let sq: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == r).collect();
                if sq.len() < 5 || fixed(kind) {
                    continue;
                }
                let (x0, y0, w, h) = boxed(&sq);
                let least = if lord(kind) { 9 } else { 4 };
                let cap = if kind == "great chamber" { 60 } else { 40 };
                if (2 * w.max(h) > 5 * w.min(h) && sq.len() >= 8) || sq.len() > cap {
                    if let Some(id) = self.cut(lay, r, least, 4, "chamber") {
                        free.push(id);
                        changed = true;
                    }
                    continue;
                }
                let sound = |sq: &[usize], lmin: usize| self.sound(sq, lmin);
                if sq.len() * 5 >= w * h * 3 {
                    continue;
                }
                // A tail: the squares outside the room's largest box.
                let has = |i: usize, j: usize| i >= x0 && j >= y0 && i < x0 + w && j < y0 + h && lay.cells[j * nx + i] == r;
                let mut big = (0, [0usize; 4]);
                for b0 in 0..h {
                    for b1 in b0 + 1..=h {
                        for a0 in 0..w {
                            for a1 in a0 + 1..=w {
                                let n = (a1 - a0) * (b1 - b0);
                                if n > big.0 && (b0..b1).all(|j| (a0..a1).all(|i| has(x0 + i, y0 + j))) {
                                    big = (n, [a0, a1, b0, b1]);
                                }
                            }
                        }
                    }
                }
                let [a0, a1, b0, b1] = big.1;
                let core: Vec<usize> = sq.iter().copied().filter(|&k| (a0..a1).contains(&(k % nx - x0)) && (b0..b1).contains(&(k / nx - y0))).collect();
                if !sound(&core, least) {
                    continue;
                }
                // (Each piece of the tail to a way beside it, else the room beside it that stays
                // sound.)
                let mut tail: Vec<usize> = sq.iter().copied().filter(|k| !core.contains(k)).collect();
                while let Some(&k0) = tail.first() {
                    let mut part = vec![k0];
                    let mut i = 0;
                    while i < part.len() {
                        let k = part[i];
                        i += 1;
                        for q in nbrs(k) {
                            if tail.contains(&q) && !part.contains(&q) {
                                part.push(q);
                            }
                        }
                    }
                    tail.retain(|k| !part.contains(k));
                    let mut by: Vec<(usize, i16)> = Vec::new();
                    for &k in &part {
                        for q in nbrs(k) {
                            let o = lay.cells[q];
                            if o >= 0 && o != r && crate::interior::walkable(lay.rooms[o as usize].kind) && !matches!(lay.rooms[o as usize].kind, "great hall" | "mess hall") {
                                match by.iter_mut().find(|x| x.1 == o) {
                                    Some(x) => x.0 += 1,
                                    None => by.push((1, o)),
                                }
                            }
                        }
                    }
                    by.sort_by(|a, b| {
                        let way = |o: i16| is_circulation(lay.rooms[o as usize].kind) && lay.rooms[o as usize].kind != "landing";
                        (way(b.1), b.0, std::cmp::Reverse(b.1)).cmp(&(way(a.1), a.0, std::cmp::Reverse(a.1)))
                    });
                    let to = by.iter().map(|x| x.1).find(|&o| {
                        let k = lay.rooms[o as usize].kind;
                        if is_circulation(k) && k != "landing" {
                            return true;
                        }
                        let mut grown: Vec<usize> = (0..nx * ny).filter(|&q| lay.cells[q] == o).collect();
                        grown.extend(part.iter().copied());
                        let (_, _, gw, gh) = boxed(&grown);
                        !fixed(k) && sound(&grown, 4) && grown.len() * 5 >= gw * gh * 3
                    });
                    // (Nowhere: a tail a square wide, a tower's way in or the like, a mural passage of its
                    // own.)
                    let wide = part.iter().any(|&k| k % nx + 1 < nx && [k + 1, k + nx, k + nx + 1].iter().all(|q| part.contains(q)));
                    let to = to.or_else(|| (!wide && part.len() >= 2).then(|| lay.room("mural passage")));
                    if let Some(o) = to {
                        for &k in &part {
                            lay.cells[k] = o;
                        }
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// A storey of five rooms or more (a floor's free ones, `free`) over half of them of one size:
    /// two of that size side by side made one where that is sound (40 squares at most, no longer
    /// than 2.5:1, its box 60% full), else cut again where neither is of that size, until it isn't
    /// so.
    pub(super) fn vary(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        for _ in 0..free.len() {
            let live: Vec<i16> = free.iter().copied().filter(|&r| lay.count(r) > 0).collect();
            let sizes: Vec<usize> = live.iter().map(|&r| lay.count(r)).collect();
            let Some(&most) = sizes.iter().max_by_key(|&&s| (sizes.iter().filter(|&&t| t == s).count(), s)) else { return };
            if sizes.len() < 5 || 2 * sizes.iter().filter(|&&t| t == most).count() <= sizes.len() {
                return;
            }
            let same: Vec<i16> = live.iter().copied().filter(|&r| lay.count(r) == most).collect();
            let pair = same.iter().flat_map(|&a| same.iter().map(move |&b| (a, b))).filter(|&(a, b)| a < b).find(|&(a, b)| {
                let mut u: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == a || lay.cells[k] == b).collect();
                u.sort();
                let (x0, x1) = u.iter().fold((usize::MAX, 0), |m, &k| (m.0.min(k % nx), m.1.max(k % nx)));
                let (y0, y1) = u.iter().fold((usize::MAX, 0), |m, &k| (m.0.min(k / nx), m.1.max(k / nx)));
                u.len() <= 40 && self.sound(&u, 4) && u.len() * 5 >= (x1 - x0 + 1) * (y1 - y0 + 1) * 3
            });
            if let Some((a, b)) = pair {
                for c in lay.cells.iter_mut() {
                    if *c == b {
                        *c = a;
                    }
                }
                free.retain(|&f| f != b);
                continue;
            }
            // (Else two side by side cut again otherwise, neither of that size.)
            let adjacent = |lay: &Lay, a: i16, b: i16| (0..nx * ny).any(|k| lay.cells[k] == a && [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx].iter().any(|&q| q < nx * ny && lay.cells[q] == b));
            let mut done = false;
            for (a, b) in same.iter().flat_map(|&a| same.iter().map(move |&b| (a, b))).filter(|&(a, b)| a < b) {
                if !adjacent(lay, a, b) {
                    continue;
                }
                let (cells, kind) = (lay.cells.clone(), lay.rooms[b as usize].kind);
                for c in lay.cells.iter_mut() {
                    if *c == b {
                        *c = a;
                    }
                }
                match self.cut_avoiding(lay, a, 4, 4, kind, most) {
                    Some(id) => {
                        // (The new part takes `b`'s place among the floor's rooms.)
                        for c in lay.cells.iter_mut() {
                            if *c == id {
                                *c = b;
                            }
                        }
                        lay.rooms.pop();
                        done = true;
                        break;
                    }
                    None => lay.cells = cells,
                }
            }
            if !done {
                return;
            }
        }
    }

    /// A landing of more than 12 squares (squares no zone took made part of it): those outside its
    /// own (from the dais end's row across its columns) to the room beside them that stays sound,
    /// else a passage beside them.
    pub(super) fn trim_landings(&self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        for r in 0..lay.rooms.len() as i16 {
            if lay.rooms[r as usize].kind != "landing" || lay.count(r) <= 12 {
                continue;
            }
            let (tl0, tl1) = if self.small { (0, self.f.wid as isize) } else { (self.land_t.0 as isize, self.land_t.1 as isize) };
            let own = |k: usize| {
                let (s, t) = self.f.local((k % nx) as isize, (k / nx) as isize);
                s >= self.dz as isize && t >= tl0 && t < tl1
            };
            let mut extra: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == r && !own(k)).collect();
            while let Some(&k0) = extra.first() {
                let mut part = vec![k0];
                let mut i = 0;
                while i < part.len() {
                    let k = part[i];
                    i += 1;
                    for q in nbrs(k) {
                        if extra.contains(&q) && !part.contains(&q) {
                            part.push(q);
                        }
                    }
                }
                extra.retain(|k| !part.contains(k));
                let mut by: Vec<(usize, i16)> = Vec::new();
                for &k in &part {
                    for q in nbrs(k) {
                        let o = lay.cells[q];
                        if o >= 0 && o != r && crate::interior::walkable(lay.rooms[o as usize].kind) && !matches!(lay.rooms[o as usize].kind, "great hall" | "mess hall" | "landing") {
                            match by.iter_mut().find(|x| x.1 == o) {
                                Some(x) => x.0 += 1,
                                None => by.push((1, o)),
                            }
                        }
                    }
                }
                by.sort_by(|a, b| (b.0, std::cmp::Reverse(b.1)).cmp(&(a.0, std::cmp::Reverse(a.1))));
                let to = by.iter().map(|x| x.1).find(|&o| {
                    if is_circulation(lay.rooms[o as usize].kind) {
                        return true;
                    }
                    let mut grown: Vec<usize> = (0..nx * ny).filter(|&q| lay.cells[q] == o).collect();
                    grown.extend(part.iter().copied());
                    let (x0, x1) = grown.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
                    let (y0, y1) = grown.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
                    self.sound(&grown, 4) && grown.len() <= 40 && grown.len() * 5 >= (x1 - x0 + 1) * (y1 - y0 + 1) * 3
                });
                // (Nowhere, a strip a square wide: a mural passage of its own.)
                let wide = part.iter().any(|&k| k % nx + 1 < nx && [k + 1, k + nx, k + nx + 1].iter().all(|q| part.contains(q)));
                let to = to.or_else(|| (!wide).then(|| lay.room("mural passage")));
                if let Some(o) = to.filter(|_| lay.count(r) > 12) {
                    for &k in &part {
                        lay.cells[k] = o;
                    }
                }
            }
        }
    }

    /// Doors that keep every room within a room of a way about the floor (a passage, a landing, a
    /// hall, a walk, a guardroom; the corridor rule): a room beside one has a door onto it; one
    /// beside none a door into the room beside it that is beside one (the one they share the most
    /// wall with).
    pub(super) fn near_links(&self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let n = lay.rooms.len();
        let mut shared: Vec<Vec<(i16, usize)>> = vec![Vec::new(); n];
        for k in 0..nx * ny {
            let a = lay.cells[k];
            if a < 0 || !crate::interior::walkable(lay.rooms[a as usize].kind) {
                continue;
            }
            for q in [(k % nx + 1 < nx).then_some(k + 1), (k / nx + 1 < ny).then_some(k + nx)].into_iter().flatten() {
                let b = lay.cells[q];
                if b < 0 || b == a || !crate::interior::walkable(lay.rooms[b as usize].kind) {
                    continue;
                }
                for (x, y) in [(a, b), (b, a)] {
                    match shared[x as usize].iter_mut().find(|e| e.0 == y) {
                        Some(e) => e.1 += 1,
                        None => shared[x as usize].push((y, 1)),
                    }
                }
            }
        }
        let st = self.stairs;
        let start = lay.cells[st[1] * nx + st[0]];
        let way = |r: i16| crate::interior::is_way_through(lay.rooms[r as usize].kind) || r == start;
        let near: Vec<bool> = (0..n as i16).map(|r| !way(r) && shared[r as usize].iter().any(|e| way(e.0))).collect();
        let mut add = Vec::new();
        for r in (0..n as i16).filter(|&r| lay.count(r) > 0 && !way(r)) {
            let pick = if near[r as usize] {
                // (Onto a way that isn't a passage, which every room beside one opens onto anyway.)
                shared[r as usize].iter().filter(|e| way(e.0) && !is_circulation(lay.rooms[e.0 as usize].kind)).max_by_key(|e| (e.1, std::cmp::Reverse(e.0))).filter(|_| !shared[r as usize].iter().any(|e| is_circulation(lay.rooms[e.0 as usize].kind)))
            } else {
                shared[r as usize].iter().filter(|e| near[e.0 as usize]).max_by_key(|e| (e.1, std::cmp::Reverse(e.0)))
            };
            if let Some(e) = pick {
                add.push((r.min(e.0), r.max(e.0)));
            }
        }
        for p in add {
            if !lay.hints.link.contains(&p) {
                lay.hints.link.push(p);
            }
        }
    }

    /// Room kinds to the floor's free rooms, the biggest kinds to the biggest rooms; rooms left
    /// over take `fill` in turn.
    pub(super) fn assign(lay: &mut Lay, free: &[i16], mut kinds: Kinds, fill: &[&'static str]) {
        let mut rooms: Vec<(usize, i16)> = free.iter().map(|&r| (lay.count(r), r)).filter(|x| x.0 > 0).collect();
        rooms.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        kinds.truncate(rooms.len());
        let mut by_weight: Vec<usize> = (0..kinds.len()).collect();
        by_weight.sort_by(|&a, &b| kinds[b].1.total_cmp(&kinds[a].1).then(a.cmp(&b)));
        for (n, &(_, r)) in rooms.iter().enumerate() {
            lay.rooms[r as usize].kind = match by_weight.get(n) {
                Some(&k) => kinds[k].0,
                None => fill[(n - by_weight.len()) % fill.len().max(1)],
            };
        }
    }

    /// Whether half the floor's rooms or more (passages, halls and open tops aside) lie on just the
    /// squares of a room of the storey below (`last`), or over half of five or more are of one size.
    pub(super) fn repeats(&self, lay: &Lay) -> bool {
        let (mut sizes, mut same) = (Vec::new(), 0);
        for r in 0..lay.rooms.len() as i16 {
            let kind = lay.rooms[r as usize].kind;
            let Some(k0) = lay.cells.iter().position(|&c| c == r) else { continue };
            if crate::interior::is_way_through(kind) || !crate::interior::walkable(kind) {
                continue;
            }
            sizes.push(lay.count(r));
            let below = self.last.get(k0).copied().unwrap_or(-1);
            if below >= 0 && self.last.iter().zip(&lay.cells).all(|(&b, &c)| (b == below) == (c == r)) {
                same += 1;
            }
        }
        let most = sizes.iter().map(|s| sizes.iter().filter(|t| *t == s).count()).max().unwrap_or(0);
        (!sizes.is_empty() && 2 * same >= sizes.len()) || (sizes.len() >= 5 && 2 * most > sizes.len())
    }

    /// A passage that meets none of the floor's others reached from the stairs (a wing's, beside a
    /// body whose passage runs along its other wall): doors through the rooms between it and them
    /// (`Hints::link`), the fewest.
    pub(super) fn links(&self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let n = lay.rooms.len();
        let st = self.stairs;
        let start = lay.cells[st[1] * nx + st[0]];
        if start < 0 {
            return;
        }
        let mut adj: Vec<Vec<i16>> = vec![Vec::new(); n];
        for k in 0..nx * ny {
            let a = lay.cells[k];
            if a < 0 || !crate::interior::walkable(lay.rooms[a as usize].kind) {
                continue;
            }
            for q in [(k % nx + 1 < nx).then_some(k + 1), (k / nx + 1 < ny).then_some(k + nx)].into_iter().flatten() {
                let b = lay.cells[q];
                if b >= 0 && b != a && crate::interior::walkable(lay.rooms[b as usize].kind) && !adj[a as usize].contains(&b) {
                    adj[a as usize].push(b);
                    adj[b as usize].push(a);
                }
            }
        }
        let circ = |lay: &Lay, r: i16| is_circulation(lay.rooms[r as usize].kind) || r == start;
        for _ in 0..n {
            // (Reached: the stairs' passages, through each other and the links made.)
            let mut seen = vec![false; n];
            let mut stack = vec![start];
            seen[start as usize] = true;
            while let Some(a) = stack.pop() {
                for &b in &adj[a as usize] {
                    let linked = lay.hints.link.contains(&(a.min(b), a.max(b)));
                    if !seen[b as usize] && ((circ(lay, a) && circ(lay, b)) || linked) {
                        seen[b as usize] = true;
                        stack.push(b);
                    }
                }
            }
            let Some(c) = (0..n as i16).find(|&r| !seen[r as usize] && circ(lay, r) && lay.count(r) > 0) else { return };
            // (The fewest rooms from it to a passage reached.)
            let mut from = vec![-1i16; n];
            let mut q = std::collections::VecDeque::from([c]);
            from[c as usize] = c;
            let mut end = None;
            while let Some(a) = q.pop_front() {
                if seen[a as usize] && circ(lay, a) {
                    end = Some(a);
                    break;
                }
                for &b in &adj[a as usize] {
                    if from[b as usize] < 0 {
                        from[b as usize] = a;
                        q.push_back(b);
                    }
                }
            }
            let Some(mut b) = end else { return };
            while b != c {
                let a = from[b as usize];
                lay.hints.link.push((a.min(b), a.max(b)));
                b = a;
            }
        }
    }

    /// Rooms of fewer than `min` squares, or a square wide (but passages, a hall's open height and
    /// roofs), made part of the room beside them they share the most wall with: another room
    /// first, a hall next, a passage last.
    pub(super) fn absorb(&self, lay: &mut Lay, free: &mut Vec<i16>, min: usize) {
        let (nx, ny) = (self.nx, self.ny);
        for r in 0..lay.rooms.len() as i16 {
            let kind = lay.rooms[r as usize].kind;
            let n = lay.count(r);
            if n == 0 || is_circulation(kind) || !crate::interior::walkable(kind) || matches!(kind, "great hall" | "mess hall" | "battlements" | "wall walk") {
                continue;
            }
            let mine = |i: usize, j: usize| i < nx && j < ny && lay.cells[j * nx + i] == r;
            let wide = (0..nx * ny).any(|k| mine(k % nx, k / nx) && mine(k % nx + 1, k / nx) && mine(k % nx, k / nx + 1) && mine(k % nx + 1, k / nx + 1));
            if n >= min && wide {
                continue;
            }
            let Some(to) = self.beside(lay, &|k| lay.cells[k] == r) else { continue };
            for c in lay.cells.iter_mut() {
                if *c == r {
                    *c = to;
                }
            }
            free.retain(|&f| f != r);
        }
    }

    /// The floor's rooms (`free`) more than one room from a way about it (a passage, a hall): each
    /// made part of the room beside it nearest one (the floor's own first), as no door could bring
    /// it nearer.
    pub(super) fn near_ways(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        loop {
            let n = lay.rooms.len();
            let mut adj: Vec<Vec<i16>> = vec![Vec::new(); n];
            for k in 0..nx * ny {
                let a = lay.cells[k];
                if a < 0 || !crate::interior::walkable(lay.rooms[a as usize].kind) {
                    continue;
                }
                for q in [(k % nx + 1 < nx).then_some(k + 1), (k / nx + 1 < ny).then_some(k + nx)].into_iter().flatten() {
                    let b = lay.cells[q];
                    if b >= 0 && b != a && crate::interior::walkable(lay.rooms[b as usize].kind) && !adj[a as usize].contains(&b) {
                        adj[a as usize].push(b);
                        adj[b as usize].push(a);
                    }
                }
            }
            let mut hops = vec![usize::MAX; n];
            let mut q = std::collections::VecDeque::new();
            for r in (0..n).filter(|&r| crate::interior::is_way_through(lay.rooms[r].kind)) {
                hops[r] = 0;
                q.push_back(r);
            }
            while let Some(a) = q.pop_front() {
                for &b in &adj[a] {
                    if hops[b as usize] == usize::MAX {
                        hops[b as usize] = hops[a] + 1;
                        q.push_back(b as usize);
                    }
                }
            }
            let merge = free.iter().filter(|&&r| hops[r as usize] != usize::MAX && hops[r as usize] >= 3).find_map(|&r| {
                adj[r as usize].iter().copied().filter(|&o| !crate::interior::is_way_through(lay.rooms[o as usize].kind)).min_by_key(|&o| (hops[o as usize], !free.contains(&o), o)).map(|o| (r, o))
            });
            let Some((r, to)) = merge else { return };
            for c in lay.cells.iter_mut() {
                if *c == r {
                    *c = to;
                }
            }
            free.retain(|&f| f != r);
        }
    }

    /// The room beside the squares `of` that they share the most wall with, a room first, a hall
    /// next, then a passage, a landing last (none of them open to below or a roof).
    pub(super) fn beside(&self, lay: &Lay, of: &dyn Fn(usize) -> bool) -> Option<i16> {
        let (nx, ny) = (self.nx, self.ny);
        let mut shared: Vec<(i16, usize)> = Vec::new();
        for k in (0..nx * ny).filter(|&k| of(k)) {
            let (i, j) = (k % nx, k / nx);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a >= nx || b >= ny || of(b * nx + a) {
                    continue;
                }
                let o = lay.cells[b * nx + a];
                if o >= 0 && crate::interior::walkable(lay.rooms[o as usize].kind) {
                    match shared.iter_mut().find(|x| x.0 == o) {
                        Some(x) => x.1 += 1,
                        None => shared.push((o, 1)),
                    }
                }
            }
        }
        let rank = |o: i16| {
            let kind = lay.rooms[o as usize].kind;
            if matches!(kind, "great hall" | "mess hall") {
                2
            } else if kind == "landing" {
                0
            } else if is_circulation(kind) || matches!(kind, "battlements" | "wall walk") {
                1
            } else {
                3
            }
        };
        shared.iter().max_by(|a, b| rank(a.0).cmp(&rank(b.0)).then(a.1.cmp(&b.1)).then(b.0.cmp(&a.0))).map(|x| x.0)
    }

    /// Body squares no zone took: a room of their own where they make one (4 squares, two wide),
    /// else part of the room beside them (`beside`).
    pub(super) fn leftovers(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        let mut seen = vec![false; nx * ny];
        for k0 in 0..nx * ny {
            if seen[k0] || lay.cells[k0] >= 0 || self.piece[k0] != Piece::Body {
                continue;
            }
            let mut part = vec![k0];
            seen[k0] = true;
            let mut n = 0;
            while n < part.len() {
                let (i, j) = (part[n] % nx, part[n] / nx);
                n += 1;
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny && !seen[b * nx + a] && lay.cells[b * nx + a] < 0 && self.piece[b * nx + a] == Piece::Body {
                        seen[b * nx + a] = true;
                        part.push(b * nx + a);
                    }
                }
            }
            let has = |i: usize, j: usize| i < nx && j < ny && part.contains(&(j * nx + i));
            let wide = part.iter().any(|&k| has(k % nx + 1, k / nx) && has(k % nx, k / nx + 1) && has(k % nx + 1, k / nx + 1));
            let to = if part.len() >= 4 && wide { None } else { self.beside(lay, &|k| part.contains(&k)) };
            let r = to.unwrap_or_else(|| {
                let r = lay.room("chamber");
                free.push(r);
                r
            });
            for &k in &part {
                lay.cells[k] = r;
            }
        }
    }

    /// Rooms over the body's free squares in box `b` (s0, s1, t0, t1), divided as a keep's storeys
    /// were, off its side `way` runs along (0 its low s, 1 its high s, 2 its low t, 3 its high t: a
    /// passage's): a box over 7 squares deep from it (6 or 7, now and then, where nothing opens off
    /// it) parted by a cross wall along it into a near part and a far one, each cut across into
    /// rooms at most 2.5 times as long as they are wide and of 40 squares at most, of lengths
    /// rolled (more of them where the box must hold `want` rooms), the far part's cross walls off
    /// the near part's (`lines`: walls across s or t, and where): every room on the way or beside
    /// a room that is. A tower, a wing or the forebuilding that would open only off the far part
    /// has a mural passage to it across that part. A room of 20 squares or more on the way and an
    /// outer wall may have a closet off its corner there. The rooms.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn partition(&self, lay: &mut Lay, b: [isize; 4], way: usize, want: usize, lines: &mut Vec<(bool, isize)>, rng: &mut Pcg32) -> Vec<i16> {
        let (nx, ny) = (self.nx, self.ny);
        let free = |lay: &Lay, s: isize, t: isize| self.body(s, t).filter(|&k| lay.cells[k] < 0);
        let mut bb = [isize::MAX, isize::MIN, isize::MAX, isize::MIN];
        for s in b[0]..b[1] {
            for t in b[2]..b[3] {
                if free(lay, s, t).is_some() {
                    bb = [bb[0].min(s), bb[1].max(s + 1), bb[2].min(t), bb[3].max(t + 1)];
                }
            }
        }
        if bb[0] >= bb[1] {
            return Vec::new();
        }
        // (Depth from the way along s or t; `hi`: the way on the box's high side. A square by its
        // depth and place along the way.)
        let (deep_s, hi) = (way < 2, way % 2 == 1);
        let (d0, d1, a0, a1) = if deep_s { (bb[0], bb[1], bb[2], bb[3]) } else { (bb[2], bb[3], bb[0], bb[1]) };
        let at = |d: isize, a: isize| if deep_s { (d, a) } else { (a, d) };
        let depth = d1 - d0;
        // (Rooms already laid that are ways about the floor or open onto one; what lies beyond the
        // body: a tower, a wing, the forebuilding.)
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        let circ = |lay: &Lay, k: usize| lay.cells[k] >= 0 && is_circulation(lay.rooms[lay.cells[k] as usize].kind);
        let mut opens = vec![false; lay.rooms.len()];
        for k in (0..nx * ny).filter(|&k| lay.cells[k] >= 0 && (circ(lay, k) || nbrs(k).any(|q| circ(lay, q)))) {
            opens[lay.cells[k] as usize] = true;
        }
        let beyond = |k: usize| matches!(self.piece[k], Piece::Tower | Piece::Wing(_) | Piece::Fore);
        let touches_beyond = (d0..d1).any(|d| {
            (a0..a1).any(|a| {
                let (s, t) = at(d, a);
                free(lay, s, t).is_some_and(|k| nbrs(k).any(beyond))
            })
        });
        let parted = depth >= 8 || (depth >= 6 && !touches_beyond && rng.next_f64() < 0.5);
        // (A depth whose rooms can fill the box's length: some number of them between their least
        // and most lengths.)
        let fits = |d: isize| {
            let len = a1 - a0;
            let (lmin, lmax) = (((2 * d + 4) / 5).max(2).min(len), ((5 * d) / 2).min(40 / d.max(1)));
            (1..=len).any(|n| n * lmin <= len && len <= n * lmax)
        };
        let near = if parted {
            let aim = crate::core::round(depth as f64 * (0.4 + 0.2 * rng.next_f64())) as isize;
            (3..=depth - 3).filter(|&d| fits(d) && fits(depth - d)).min_by_key(|&d| ((d - aim).abs(), d)).unwrap_or(aim.clamp(3, depth - 3))
        } else {
            depth
        };
        // (Zones: (from, to) in depth, the near one first.)
        let zones = if !parted {
            vec![(d0, d1)]
        } else if hi {
            vec![(d1 - near, d1), (d0, d1 - near)]
        } else {
            vec![(d0, d0 + near), (d0 + near, d1)]
        };
        if parted {
            lines.push((deep_s, if hi { d1 - near } else { d0 + near }));
        }
        let in_zone = |lay: &Lay, z: (isize, isize), k: usize| {
            let (s, t) = self.f.local((k % nx) as isize, (k / nx) as isize);
            let (d, a) = if deep_s { (s, t) } else { (t, s) };
            d >= z.0 && d < z.1 && a >= a0 && a < a1 && lay.cells[k] < 0 && self.piece[k] == Piece::Body
        };
        // The far part's mural passages: to each tower, wing or forebuilding beside it that opens
        // off nothing else (no square of the near part, no way, no room opening onto one), across
        // it at its end where they meet there, else in the middle of where they meet.
        let mut stubs: Vec<isize> = Vec::new();
        if let Some(&far) = zones.get(1) {
            let mut seen = vec![false; nx * ny];
            for k0 in 0..nx * ny {
                if seen[k0] || !beyond(k0) || !nbrs(k0).any(|q| in_zone(lay, far, q)) {
                    continue;
                }
                let mut part = vec![k0];
                seen[k0] = true;
                let mut i = 0;
                while i < part.len() {
                    let k = part[i];
                    i += 1;
                    for q in nbrs(k) {
                        if !seen[q] && self.piece[q] == self.piece[k0] {
                            seen[q] = true;
                            part.push(q);
                        }
                    }
                }
                if part.iter().any(|&k| nbrs(k).any(|q| in_zone(lay, zones[0], q) || (lay.cells[q] >= 0 && opens[lay.cells[q] as usize]))) {
                    continue;
                }
                let mut along: Vec<isize> = part
                    .iter()
                    .flat_map(|&k| nbrs(k).filter(|&q| in_zone(lay, far, q)).collect::<Vec<_>>())
                    .map(|q| {
                        let (s, t) = self.f.local((q % nx) as isize, (q / nx) as isize);
                        if deep_s { t } else { s }
                    })
                    .collect();
                along.sort();
                along.dedup();
                let a = if along.contains(&a0) { a0 } else if along.contains(&(a1 - 1)) { a1 - 1 } else { along[along.len() / 2] };
                if !stubs.contains(&a) {
                    stubs.push(a);
                }
            }
            stubs.sort();
            if !stubs.is_empty() {
                let p = lay.room("mural passage");
                for &a in &stubs {
                    for d in far.0..far.1 {
                        let (s, t) = at(d, a);
                        if let Some(k) = free(lay, s, t) {
                            lay.cells[k] = p;
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut extra = want.saturating_sub(zones.len());
        for (zi, &(z0, z1)) in zones.iter().enumerate() {
            let d = z1 - z0;
            // (Its runs along the way, between the mural passages across it.)
            let mut runs: Vec<(isize, isize)> = Vec::new();
            let mut from = a0;
            for &c in stubs.iter().filter(|_| zi > 0) {
                if c > from {
                    runs.push((from, c));
                }
                from = c + 1;
            }
            if from < a1 {
                runs.push((from, a1));
            }
            for (r0, r1) in runs {
                let len = r1 - r0;
                let held: Vec<usize> = (r0..r1)
                    .map(|a| {
                        (z0..z1)
                            .filter(|&d| {
                                let (s, t) = at(d, a);
                                free(lay, s, t).is_some()
                            })
                            .count()
                    })
                    .collect();
                // (A room's length along the way: no more than 2.5 times its depth (as most of it is
                // deep: a notch, a passage taking some) or 40 squares, no less than its depth over 2.5,
                // 2 squares.)
                let mut deep: Vec<usize> = held.iter().copied().filter(|&h| h > 0).collect();
                deep.sort();
                let d = deep.get(deep.len() / 2).map_or(d, |&h| h as isize);
                let lmin = ((2 * d + 4) / 5).max(2).min(len);
                let lmax = ((5 * d) / 2).min(40 / d.max(1)).max(lmin);
                let lo = (len + lmax - 1) / lmax;
                let most = (len / lmin).max(lo);
                let add = extra.min((most - lo) as usize) as isize;
                extra -= add as usize;
                let roomy = len >= (lo + add + 1) * (lmin + 1);
                let n = (lo + add + (roomy && rng.next_f64() < 0.35) as isize).min(most).max(1);
                // (Lengths of very different sizes, within those bounds.)
                let mut ls: Vec<isize> = Vec::new();
                for _ in 0..8 {
                    ls = cuts(len as usize, n as usize, lmin as usize, rng).into_iter().map(|x| x as isize).collect();
                    if ls.iter().all(|&x| x <= lmax) {
                        break;
                    }
                }
                if ls.iter().any(|&x| x > lmax) {
                    ls = (0..n).map(|k| len * (k + 1) / n - len * k / n).collect();
                }
                // (Each wall where the rooms before it have their share of the squares (a room by a
                // notch runs on further), each room as long as it may be and with a 2 x 2 block of
                // squares where it can be, off the lines of the walls across of the part before.)
                let total: usize = held.iter().sum();
                let mut walls: Vec<isize> = Vec::new();
                let mut share = 0;
                let upto = |c: isize| held[..(c - r0) as usize].iter().sum::<usize>();
                for (i, &x) in ls[..ls.len() - 1].iter().enumerate() {
                    share += x;
                    let aim = total as f64 * share as f64 / len as f64;
                    let lo = walls.last().map_or(r0 + 1, |&c| c + 1);
                    let before = walls.last().map_or(0, |&c| upto(c));
                    let left = ls.len() - 1 - i;
                    let prev = walls.last().copied().unwrap_or(r0);
                    let solid = |a0: isize, a1: isize| {
                        (a0..a1 - 1).any(|a| {
                            (z0..z1 - 1).any(|x| {
                                [(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|&(da, dx)| {
                                    let (s, t) = at(x + dx, a + da);
                                    free(lay, s, t).is_some()
                                })
                            })
                        })
                    };
                    let bad = |c: isize| {
                        (upto(c) < before + 4 || !solid(prev, c)) as u8 + (c - prev < lmin || c - prev > lmax || r1 - c < lmin * left as isize) as u8
                    };
                    let off = |c: isize| (upto(c) as f64 - aim).abs() + if lines.contains(&(!deep_s, c)) { d as f64 } else { 0.0 };
                    let best = (lo..r1).min_by(|&a, &b| bad(a).cmp(&bad(b)).then(off(a).total_cmp(&off(b))).then(a.cmp(&b)));
                    if let Some(c) = best {
                        walls.push(c);
                    }
                }
                // (In the near part, where the way runs along most of it, each room on it: one past
                // the way's end (a tower's notch) runs on to it.)
                if zi == 0 {
                    let on: Vec<bool> = (r0..r1)
                        .map(|a| {
                            (z0..z1).any(|d| {
                                let (s, t) = at(d, a);
                                free(lay, s, t).is_some_and(|k| nbrs(k).any(|q| circ(lay, q)))
                            })
                        })
                        .collect();
                    if 2 * on.iter().filter(|&&o| o).count() > on.len() {
                        let first = r0 + on.iter().position(|&o| o).unwrap_or(0) as isize;
                        let last = r0 + on.iter().rposition(|&o| o).unwrap_or(0) as isize;
                        walls.retain(|&c| c > first && c <= last);
                        // (Split again where that left an end room too long.)
                        let c = walls.first().copied().unwrap_or(r1);
                        if c - r0 > lmax && (first + 1).max(r0 + lmin).max(c - lmax) <= c - 2 {
                            walls.insert(0, (first + 1).max(r0 + lmin).max(c - lmax));
                        }
                        let c = walls.last().copied().unwrap_or(r0);
                        if r1 - c > lmax && last.min(r1 - lmin).min(c + lmax) >= c + 2 {
                            walls.push(last.min(r1 - lmin).min(c + lmax));
                        }
                    }
                }
                lines.extend(walls.iter().map(|&c| (!deep_s, c)));
                let mut from = r0;
                for c in walls.into_iter().chain([r1]) {
                    let r = lay.room("chamber");
                    let mut n = 0;
                    for x in z0..z1 {
                        for y in from..c {
                            let (s, t) = at(x, y);
                            if let Some(k) = free(lay, s, t) {
                                lay.cells[k] = r;
                                n += 1;
                            }
                        }
                    }
                    out.push(r);
                    if zi == 0 && n >= 20 && rng.next_f64() < 0.5 {
                        let bx = if deep_s { [z0, z1, from, c] } else { [from, c, z0, z1] };
                        out.extend(self.closet(lay, r, bx, rng));
                    }
                    from = c;
                }
            }
        }
        for &r in &out {
            self.keep_whole(lay, r);
        }
        out
    }

    /// A closet off a corner of room `r` (its box `bx`) on an outer wall: 2 squares by 2 or 3
    /// along the box's longer side, all the room's.
    pub(super) fn closet(&self, lay: &mut Lay, r: i16, bx: [isize; 4], rng: &mut Pcg32) -> Option<i16> {
        let (ls, lt) = if bx[1] - bx[0] >= bx[3] - bx[2] { (2 + rng.below(2) as isize, 2) } else { (2, 2 + rng.below(2) as isize) };
        let c0 = rng.below(4) as usize;
        for c in (0..4).map(|m| (c0 + m) % 4) {
            let (s0, t0) = (if c & 1 == 0 { bx[0] } else { bx[1] - ls }, if c & 2 == 0 { bx[2] } else { bx[3] - lt });
            let sq: Vec<(isize, isize)> = (s0..s0 + ls).flat_map(|s| (t0..t0 + lt).map(move |t| (s, t))).collect();
            let out = |s: isize, t: isize| self.at(s, t).is_none_or(|k| self.piece[k] == Piece::Out);
            if !sq.iter().all(|&(s, t)| self.body(s, t).is_some_and(|k| lay.cells[k] == r)) || !sq.iter().any(|&(s, t)| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(ds, dt)| out(s + ds, t + dt))) {
                continue;
            }
            let id = lay.room("chamber");
            for &(s, t) in &sq {
                if let Some(k) = self.body(s, t) {
                    lay.cells[k] = id;
                }
            }
            return Some(id);
        }
        None
    }

    /// Room `r` in one piece: its biggest (the rest left to the room beside it, `leftovers`).
    pub(super) fn keep_whole(&self, lay: &mut Lay, r: i16) {
        let (nx, ny) = (self.nx, self.ny);
        let mut part = vec![usize::MAX; nx * ny];
        let mut sizes: Vec<usize> = Vec::new();
        for k0 in 0..nx * ny {
            if lay.cells[k0] != r || part[k0] != usize::MAX {
                continue;
            }
            let p = sizes.len();
            let mut stack = vec![k0];
            part[k0] = p;
            let mut n = 0;
            while let Some(k) = stack.pop() {
                n += 1;
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny && lay.cells[b * nx + a] == r && part[b * nx + a] == usize::MAX {
                        part[b * nx + a] = p;
                        stack.push(b * nx + a);
                    }
                }
            }
            sizes.push(n);
        }
        if sizes.len() > 1 {
            let keep = (0..sizes.len()).max_by_key(|&p| (sizes[p], std::cmp::Reverse(p))).unwrap_or(0);
            for k in 0..nx * ny {
                if lay.cells[k] == r && part[k] != keep {
                    lay.cells[k] = -1;
                }
            }
        }
    }

    /// A roofed top's squares `sq`: its roof the largest box of them clear of the walk (`rim`), 2
    /// squares across at least (hipped, as the map draws it); the rest (all of them where no such
    /// box is) the walk.
    pub(super) fn roofed(&self, lay: &mut Lay, sq: &[usize], rim: &dyn Fn(usize) -> bool, walk: i16) {
        let nx = self.nx;
        let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
        for &k in sq {
            (x0, x1, y0, y1) = (x0.min(k % nx), x1.max(k % nx), y0.min(k / nx), y1.max(k / nx));
        }
        if sq.is_empty() {
            return;
        }
        let (rows, cols) = (y1 - y0 + 1, x1 - x0 + 1);
        let mut clear = vec![false; rows * cols];
        for &k in sq {
            clear[(k / nx - y0) * cols + k % nx - x0] = !rim(k);
        }
        let mut sum = vec![0usize; (rows + 1) * (cols + 1)];
        for r in 0..rows {
            for c in 0..cols {
                sum[(r + 1) * (cols + 1) + c + 1] = sum[r * (cols + 1) + c + 1] + sum[(r + 1) * (cols + 1) + c] - sum[r * (cols + 1) + c] + clear[r * cols + c] as usize;
            }
        }
        let full = |r0: usize, r1: usize, c0: usize, c1: usize| sum[r1 * (cols + 1) + c1] + sum[r0 * (cols + 1) + c0] - sum[r0 * (cols + 1) + c1] - sum[r1 * (cols + 1) + c0] == (r1 - r0) * (c1 - c0);
        let mut best = (0, [0usize; 4]);
        for r0 in 0..rows {
            for r1 in r0 + 2..=rows {
                for c0 in 0..cols {
                    for c1 in c0 + 2..=cols {
                        let a = (r1 - r0) * (c1 - c0);
                        if a > best.0 && full(r0, r1, c0, c1) {
                            best = (a, [r0, r1, c0, c1]);
                        }
                    }
                }
            }
        }
        let [r0, r1, c0, c1] = best.1;
        let roof = if best.0 >= 4 { lay.room(ROOF_SLOPE) } else { walk };
        for &k in sq {
            let (r, c) = (k / nx - y0, k % nx - x0);
            lay.cells[k] = if r >= r0 && r < r1 && c >= c0 && c < c1 { roof } else { walk };
        }
    }
}
