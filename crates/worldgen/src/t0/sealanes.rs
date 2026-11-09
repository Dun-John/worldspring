//! Sea lanes: the ways ships take between ports on one sea (`economy::run` links each to its
//! nearest), routed once over the sea cells so a link's trade goes by its real length. A lane
//! keeps a few miles off the shore except where it runs out of and into its ports, and lanes
//! running side by side merge (a cell already on a lane costs less). The routed links, with the
//! goods each carries, are split into segments between ports and the junctions where lanes meet
//! or part, and each is smoothed into a world-ft polyline that stays off the land (`lanes`).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use super::flood::D8;
use super::settle::Settlement;
use crate::core::hash::FastMap;
use crate::core::{ceil, round, sqrt};

/// Within this of land (miles) a mile at sea costs more, up to `1 + SHORE_PENALTY` at the shore.
const OFFSHORE_MI: f64 = 3.0;
const SHORE_PENALTY: f64 = 3.0;
/// A cell already on a lane costs this much of what it would.
const SHARED: f64 = 0.7;
/// The way out of a port to open water is found over the cells within this many of it.
const ACCESS: i64 = 6;
/// A port's water: sea cells within this many cells of it (as `economy::run` finds its sea).
pub const PORT_REACH: i64 = 2;
/// A sea of fewer cells is a pocket: a port beside only such water finds its sea farther off
/// (`port_sea`).
const POCKET: usize = 64;
/// Ports farther apart by sea are not each other's nearest (`Router::nearest`).
const NEAREST_MI: f64 = 1600.0;
/// Segments at or above the upper flow quantile (by length) are busy, under the lower quiet.
const BUSY_Q: f64 = 2.0 / 3.0;
const QUIET_Q: f64 = 1.0 / 3.0;

/// A sea link between two ports (settlement indices): its cells (the first port's, the sea cells,
/// the second port's; empty if it could not be routed), its length (miles) and the goods it
/// carries (`economy::run`'s flow).
#[derive(Clone, Debug, Default)]
pub struct Link {
    pub ports: [u32; 2],
    pub cells: Vec<u32>,
    pub mi: f64,
    pub flow: f64,
}

/// A stretch of sea lane between two ends (a settlement index, or `JUNCTION`): its points (world
/// ft, from the first end to the second), the goods carried along it and its class (0 busy, 1
/// normal, 2 quiet).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeaLane {
    pub pts: Vec<[f64; 2]>,
    pub class: u8,
    pub flow: f32,
    pub ends: [u32; 2],
}

/// A lane end where lanes meet or part (not a port).
pub const JUNCTION: u32 = u32::MAX;

#[derive(PartialEq)]
struct Node(f64, u32);
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Distance (cells) from each cell to the nearest land cell (0 on land): a two-pass chamfer.
fn shore_distance(w: usize, h: usize, land: &[bool]) -> Vec<f64> {
    let s2 = core::f64::consts::SQRT_2;
    let mut d: Vec<f64> = land.iter().map(|&l| if l { 0.0 } else { f64::INFINITY }).collect();
    for y in 0..h {
        for x in 0..w {
            let k = y * w + x;
            let mut v = d[k];
            if x > 0 { v = v.min(d[k - 1] + 1.0); }
            if y > 0 {
                v = v.min(d[k - w] + 1.0);
                if x > 0 { v = v.min(d[k - w - 1] + s2); }
                if x + 1 < w { v = v.min(d[k - w + 1] + s2); }
            }
            d[k] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let k = y * w + x;
            let mut v = d[k];
            if x + 1 < w { v = v.min(d[k + 1] + 1.0); }
            if y + 1 < h {
                v = v.min(d[k + w] + 1.0);
                if x + 1 < w { v = v.min(d[k + w + 1] + s2); }
                if x > 0 { v = v.min(d[k + w - 1] + s2); }
            }
            d[k] = v;
        }
    }
    d
}

/// The sea a port at cell `k` trades on (labels `sea`, cells per label `size`) and the reach
/// (cells) its water lies within: the largest sea within `PORT_REACH`, or where that is a pocket
/// cut off by land, the largest within `ACCESS` (the reach the routing gets out to open water by);
/// `None` where there is only a pocket.
pub fn port_sea(k: usize, w: usize, h: usize, sea: &[u32], size: &[usize]) -> Option<(u32, i64)> {
    let (x, y) = ((k % w) as i64, (k / w) as i64);
    let largest = |r: i64| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (x + dx, y + dy)))
            .filter(|&(i, j)| i >= 0 && j >= 0 && i < w as i64 && j < h as i64)
            .map(|(i, j)| sea[j as usize * w + i as usize])
            .filter(|&s| s != u32::MAX)
            .min_by_key(|&s| (std::cmp::Reverse(size[s as usize]), s))
    };
    let near = largest(PORT_REACH)?;
    if size[near as usize] >= POCKET {
        return Some((near, PORT_REACH));
    }
    // (Only a pocket in reach, a lagoon inland: no port.)
    largest(ACCESS).filter(|&s| size[s as usize] >= POCKET).map(|s| (s, ACCESS))
}

/// One grid lanes are searched over: the T0 cells (`s` 1) or blocks of 2x2 of them (`s` 2), a
/// block at sea when any of its cells is, standing at its first cell at sea (`rep`); a step
/// between two blocks only where the straight way between their cells stays at sea.
struct Level {
    w: usize,
    h: usize,
    s: usize,
    fw: usize,
    rep: Vec<u32>,
    /// Per node: all its cells at sea.
    full: Vec<bool>,
    /// Per node, once worked out (bit 8): the steps (bits 0-7, as `D8`) it may take.
    steps: Vec<u16>,
    /// Per node: what a mile there costs (0 on land); `SHARED` of it once a lane passes.
    factor: Vec<f64>,
    used: Vec<bool>,
    stamp: Vec<u32>,
    goal: Vec<u32>,
    cost: Vec<f64>,
    parent: Vec<u32>,
    round: u32,
}

impl Level {
    fn new(fw: usize, fh: usize, s: usize, land: &[bool], mi: f64) -> Level {
        let (w, h) = (fw.div_ceil(s), fh.div_ceil(s));
        let n = w * h;
        let cells = |k: usize| {
            let (x, y) = (k % w * s, k / w * s);
            (0..s).flat_map(move |dy| (0..s).map(move |dx| (x + dx, y + dy))).filter(|&(i, j)| i < fw && j < fh).map(move |(i, j)| j * fw + i)
        };
        let rep: Vec<u32> = (0..n).map(|k| cells(k).find(|&c| !land[c]).map_or(u32::MAX, |c| c as u32)).collect();
        let wet: Vec<bool> = rep.iter().map(|&r| r != u32::MAX).collect();
        let dry: Vec<bool> = (0..n).map(|k| cells(k).any(|c| land[c])).collect();
        let full: Vec<bool> = (0..n).map(|k| !dry[k] && cells(k).count() == s * s).collect();
        let d = shore_distance(w, h, &dry);
        let node_mi = mi * s as f64;
        // (From the shore, half a node short of the land's.)
        let factor = d
            .iter()
            .zip(&wet)
            .map(|(&d, &wet)| {
                if !wet {
                    0.0
                } else {
                    let t = (1.0 - (d - 0.5) * node_mi / OFFSHORE_MI).max(0.0);
                    1.0 + SHORE_PENALTY * t * t
                }
            })
            .collect();
        // (Two more nodes: the ports a search runs between.)
        Level { w, h, s, fw, rep, full, steps: vec![0; n], factor, used: vec![false; n], stamp: vec![0; n + 2], goal: vec![0; n], cost: vec![0.0; n + 2], parent: vec![u32::MAX; n + 2], round: 0 }
    }

    /// The steps node `k` may take (bits as `D8`): to a node at sea, and between blocks only
    /// where their cells join (`bridge`).
    fn steps_of(&self, k: usize, land: &[bool]) -> u16 {
        let (w, h) = (self.w, self.h);
        let (x, y) = ((k % w) as i32, (k / w) as i32);
        let mut m = 0x100u16;
        for (i, &(dx, dy, _)) in D8.iter().enumerate() {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                continue;
            }
            let u = ny as usize * w + nx as usize;
            if self.rep[u] != u32::MAX && self.bridge(k, u, land).is_some() {
                m |= 1 << i;
            }
        }
        m
    }

    /// How the cells of nodes `k` and `u` (side by side, both at sea) join: straight (no cells
    /// between), through the cells listed (blocks whose straight way would touch land: the way
    /// over their own cells), or not at all.
    fn bridge(&self, k: usize, u: usize, land: &[bool]) -> Option<Vec<u32>> {
        let fw = self.fw;
        // (Two blocks all at sea side by side, or one below and right of the other: the way
        // between their first cells crosses only them.)
        let (dx, dy) = ((u % self.w) as i64 - (k % self.w) as i64, (u / self.w) as i64 - (k / self.w) as i64);
        if self.s == 1 || (self.full[k] && self.full[u] && (dx == 0 || dy == 0 || dx == dy)) {
            return Some(Vec::new());
        }
        let (p, q) = (self.at(self.rep[k] as usize), self.at(self.rep[u] as usize));
        let n = ceil(4.0 * sqrt((q[0] - p[0]) * (q[0] - p[0]) + (q[1] - p[1]) * (q[1] - p[1]))) as usize;
        let clear = (1..n).all(|j| {
            let t = j as f64 / n as f64;
            let (i, j) = (round(p[0] + t * (q[0] - p[0])) as usize, round(p[1] + t * (q[1] - p[1])) as usize);
            !land[j * fw + i]
        });
        if clear {
            return Some(Vec::new());
        }
        // (Breadth first over the two blocks' cells at sea.)
        let s = self.s;
        let block = |k: usize| {
            let (x, y) = (k % self.w * s, k / self.w * s);
            (0..s).flat_map(move |dy| (0..s).map(move |dx| (x + dx, y + dy))).filter(move |&(i, j)| i < fw && j * fw + i < land.len()).map(move |(i, j)| (j * fw + i) as u32).filter(|&c| !land[c as usize])
        };
        let cells: Vec<u32> = block(k).chain(block(u)).collect();
        let (from, to) = (self.rep[k], self.rep[u]);
        let mut prev: Vec<(u32, u32)> = vec![(from, u32::MAX)];
        let mut i = 0;
        while i < prev.len() {
            let c = prev[i].0;
            if c == to {
                let mut way = Vec::new();
                let mut j = i;
                while prev[j].1 != u32::MAX {
                    j = prev[j].1 as usize;
                    way.push(prev[j].0);
                }
                way.pop();
                way.reverse();
                return Some(way);
            }
            let (cx, cy) = ((c as usize % fw) as i64, (c as usize / fw) as i64);
            for &(dx, dy, _) in &D8 {
                let nb = (cy + dy as i64) * fw as i64 + cx + dx as i64;
                if (cx + dx as i64) < 0 || (cx + dx as i64) >= fw as i64 || nb < 0 {
                    continue;
                }
                let nb = nb as u32;
                if cells.contains(&nb) && !prev.iter().any(|v| v.0 == nb) {
                    prev.push((nb, i as u32));
                }
            }
            i += 1;
        }
        None
    }

    /// The node holding cell `c` if it is at sea.
    fn node(&self, c: usize) -> Option<usize> {
        let k = (c / self.fw / self.s) * self.w + c % self.fw / self.s;
        (self.rep[k] != u32::MAX).then_some(k)
    }

    /// The cell node `k` stands at.
    fn cell(&self, k: usize) -> usize {
        self.rep[k] as usize
    }

    fn at(&self, c: usize) -> [f64; 2] {
        [(c % self.fw) as f64, (c / self.fw) as f64]
    }

    /// The cheapest way between port cells `a` and `b` (cells `mi` miles), out through nodes
    /// `from` and in through `into` (each with what the way between it and its port costs): the
    /// nodes it passes.
    fn search(&mut self, land: &[bool], mi: f64, a: usize, from: &[(usize, f64)], b: usize, into: &[(usize, f64)]) -> Option<Vec<usize>> {
        let (w, s) = (self.w, self.s);
        let n = w * self.h;
        let (na, nb) = (n, n + 1);
        self.round += 1;
        let round = self.round;
        for &(k, _) in into {
            self.goal[k] = round;
        }
        let pos = |l: &Level, k: usize| l.at(if k == na { a } else if k == nb { b } else { l.rep[k] as usize });
        let s2 = core::f64::consts::SQRT_2;
        let reach = (ACCESS as f64 + s as f64) * s2;
        let pb = self.at(b);
        let heur = |p: [f64; 2]| {
            let (dx, dy) = (p[0] - pb[0], p[1] - pb[1]);
            (sqrt(dx * dx + dy * dy) - reach).max(0.0) * mi * SHARED
        };
        let mut heap = BinaryHeap::new();
        self.stamp[na] = round;
        self.cost[na] = 0.0;
        self.parent[na] = u32::MAX;
        heap.push(Node(heur(pos(self, na)), na as u32));
        let mut found = false;
        let mut next: Vec<(usize, f64)> = Vec::with_capacity(9);
        while let Some(Node(f, k)) = heap.pop() {
            let k = k as usize;
            let c = self.cost[k];
            let pk = pos(self, k);
            if f > c + heur(pk) {
                continue;
            }
            if k == nb {
                found = true;
                break;
            }
            next.clear();
            if k == na {
                next.extend_from_slice(from);
            } else {
                if self.steps[k] & 0x100 == 0 {
                    self.steps[k] = self.steps_of(k, land);
                }
                let m = self.steps[k];
                let (x, y) = ((k % w) as i32, (k / w) as i32);
                for (i, &(dx, dy, _)) in D8.iter().enumerate() {
                    if m & (1 << i) != 0 {
                        let u = (y + dy) as usize * w + (x + dx) as usize;
                        let q = pos(self, u);
                        let d = sqrt((q[0] - pk[0]) * (q[0] - pk[0]) + (q[1] - pk[1]) * (q[1] - pk[1]));
                        next.push((u, d * mi * self.factor[u]));
                    }
                }
                if self.goal[k] == round {
                    next.push((nb, into.iter().find(|v| v.0 == k).expect("a goal").1));
                }
            }
            for &(u, step) in &next {
                let nc = c + step;
                if self.stamp[u] != round || nc < self.cost[u] {
                    self.stamp[u] = round;
                    self.cost[u] = nc;
                    self.parent[u] = k as u32;
                    heap.push(Node(nc + heur(pos(self, u)), u as u32));
                }
            }
        }
        if !found {
            return None;
        }
        let mut nodes = vec![nb];
        let mut k = nb;
        while self.parent[k] != u32::MAX {
            k = self.parent[k] as usize;
            nodes.push(k);
        }
        nodes.reverse();
        for &k in &nodes[1..nodes.len() - 1] {
            if !self.used[k] {
                self.used[k] = true;
                self.factor[k] *= SHARED;
            }
        }
        Some(nodes[1..nodes.len() - 1].to_vec())
    }
}

/// Routes sea links over one world's sea, in the order given (each sees the lanes before it):
/// over 2x2 blocks, or the cells themselves where the blocks find no way (narrow water).
pub struct Router<'a> {
    w: usize,
    h: usize,
    mi: f64,
    land: &'a [bool],
    coarse: Level,
    fine: Option<Level>,
}

impl<'a> Router<'a> {
    pub fn new(w: usize, h: usize, cell_ft: f64, land: &'a [bool]) -> Router<'a> {
        let mi = cell_ft / 5280.0;
        Router { w, h, mi, land, coarse: Level::new(w, h, 2, land, mi), fine: None }
    }

    /// The sea cells within `r` (`port_sea`) of port cell `k` whose sea is `sea[..] == label`:
    /// all within `PORT_REACH`, the nearest beyond it.
    pub fn water(&self, k: usize, sea: &[u32], label: u32, r: i64) -> Vec<u32> {
        let (w, h) = (self.w as i64, self.h as i64);
        let (x, y) = ((k % self.w) as i64, (k / self.w) as i64);
        let all: Vec<(i64, u32)> = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter(|&(dx, dy)| x + dx >= 0 && y + dy >= 0 && x + dx < w && y + dy < h).map(|(dx, dy)| (dx * dx + dy * dy, ((y + dy) * w + x + dx) as u32)).filter(|&(_, c)| sea[c as usize] == label).collect();
        let near = all.iter().map(|v| v.0).min().unwrap_or(0).max(PORT_REACH * PORT_REACH * 2);
        all.into_iter().filter(|v| v.0 <= near).map(|v| v.1).collect()
    }

    /// Per port (its cell, its water as `water` gives it, its sea's label), up to `k` of the others
    /// on its sea nearest by sea, nearest first: (miles over the 2x2 blocks, plain length from
    /// port to port, other port's index). One search per port, out to `NEAREST_MI`.
    pub fn nearest(&mut self, ports: &[(usize, Vec<u32>, u32)], k: usize) -> Vec<Vec<(f64, usize)>> {
        let mi = self.mi;
        let ends: Vec<Vec<(usize, f64)>> = ports
            .iter()
            .map(|(c, water, _)| {
                let tree = self.access(*c, water);
                let mut v: Vec<(usize, f64)> = tree.iter().filter_map(|(&q, &(d, _))| self.coarse.node(q as usize).filter(|&n| self.coarse.cell(n) == q as usize).map(|n| (n, d))).collect();
                v.sort_by(|x, y| x.0.cmp(&y.0));
                v
            })
            .collect();
        // (Per node, the ports whose ways out reach it and what that costs them.)
        let mut at: std::collections::BTreeMap<usize, Vec<(usize, f64)>> = BTreeMap::new();
        for (p, e) in ends.iter().enumerate() {
            for &(n, d) in e {
                at.entry(n).or_default().push((p, d));
            }
        }
        let l = &mut self.coarse;
        let s2 = core::f64::consts::SQRT_2;
        let mut out = Vec::with_capacity(ports.len());
        for (p, e) in ends.iter().enumerate() {
            l.round += 1;
            let round = l.round;
            let mut heap = BinaryHeap::new();
            for &(n, d) in e {
                if l.stamp[n] != round || d < l.cost[n] {
                    l.stamp[n] = round;
                    l.cost[n] = d;
                    heap.push(Node(d, n as u32));
                }
            }
            let mut best: BTreeMap<usize, f64> = BTreeMap::new();
            let mut kth = NEAREST_MI;
            while let Some(Node(c, n)) = heap.pop() {
                let n = n as usize;
                if c > l.cost[n] {
                    continue;
                }
                if c > kth {
                    break;
                }
                let mut changed = false;
                for &(q, d) in at.get(&n).map(|v| &v[..]).unwrap_or(&[]) {
                    if q != p && ports[q].2 == ports[p].2 {
                        let e = best.entry(q).or_insert(f64::INFINITY);
                        changed |= c + d < *e;
                        *e = e.min(c + d);
                    }
                }
                if changed && best.len() >= k {
                    let mut v: Vec<f64> = best.values().copied().collect();
                    v.sort_by(|a, b| a.total_cmp(b));
                    kth = kth.min(v[k - 1]);
                }
                if l.steps[n] & 0x100 == 0 {
                    l.steps[n] = l.steps_of(n, self.land);
                }
                let m = l.steps[n];
                let (x, y) = ((n % l.w) as i32, (n / l.w) as i32);
                for (i, &(dx, dy, _)) in D8.iter().enumerate() {
                    if m & (1 << i) != 0 {
                        let u = (y + dy) as usize * l.w + (x + dx) as usize;
                        let nc = c + if dx != 0 && dy != 0 { s2 } else { 1.0 } * mi * l.s as f64;
                        if l.stamp[u] != round || nc < l.cost[u] {
                            l.stamp[u] = round;
                            l.cost[u] = nc;
                            heap.push(Node(nc, u as u32));
                        }
                    }
                }
            }
            let mut v: Vec<(f64, usize)> = best.into_iter().filter(|v| v.1 <= NEAREST_MI).map(|(q, d)| (d, q)).collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            v.truncate(k);
            out.push(v);
        }
        out
    }

    /// The ways out of port cell `k` (through its water `from`) over the cells within `ACCESS`
    /// of it: per cell reached, its cost (miles) and the cell it is reached from.
    fn access(&self, k: usize, from: &[u32]) -> BTreeMap<u32, (f64, u32)> {
        let (w, h) = (self.w as i64, self.h as i64);
        let (px, py) = ((k % self.w) as i64, (k / self.w) as i64);
        let at = |c: usize| [(c % self.w) as f64, (c / self.w) as f64];
        let dist = |p: usize, q: usize| { let (p, q) = (at(p), at(q)); sqrt((p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1])) * self.mi };
        let mut best: BTreeMap<u32, (f64, u32)> = BTreeMap::new();
        let mut heap = BinaryHeap::new();
        for &c in from {
            let d = dist(k, c as usize);
            if best.get(&c).is_none_or(|v| d < v.0) {
                best.insert(c, (d, k as u32));
                heap.push(Node(d, c));
            }
        }
        while let Some(Node(d, c)) = heap.pop() {
            if d > best[&c].0 {
                continue;
            }
            let (x, y) = ((c as usize % self.w) as i64, (c as usize / self.w) as i64);
            for &(dx, dy, step) in &D8 {
                let (nx, ny) = (x + dx as i64, y + dy as i64);
                if nx < 0 || ny < 0 || nx >= w || ny >= h || (nx - px).abs() > ACCESS || (ny - py).abs() > ACCESS {
                    continue;
                }
                let u = (ny * w + nx) as u32;
                let nd = d + step * self.mi;
                if !self.land[u as usize] && best.get(&u).is_none_or(|v| nd < v.0) {
                    best.insert(u, (nd, c));
                    heap.push(Node(nd, u));
                }
            }
        }
        best
    }

    /// The cheapest way from port cell `a` (out through its water `from`) to port cell `b` (in
    /// through `into`): its cells (`a`, the sea cells, `b`) and length (miles); `None` if none.
    pub fn route(&mut self, a: usize, from: &[u32], b: usize, into: &[u32]) -> Option<(Vec<u32>, f64)> {
        let (w, mi) = (self.w, self.mi);
        let (out, inn) = (self.access(a, from), self.access(b, into));
        // (A level's nodes whose first cell the ways out reach, at what reaching it costs.)
        let ends = |l: &Level, tree: &BTreeMap<u32, (f64, u32)>| {
            let mut v: Vec<(usize, f64)> = tree.iter().filter_map(|(&c, &(d, _))| l.node(c as usize).filter(|&k| l.cell(k) == c as usize).map(|k| (k, d))).collect();
            v.sort_by(|x, y| x.0.cmp(&y.0));
            v
        };
        let (cf, ci) = (ends(&self.coarse, &out), ends(&self.coarse, &inn));
        let mut found = if cf.is_empty() || ci.is_empty() { None } else { self.coarse.search(self.land, mi, a, &cf, b, &ci).map(|p| (p, true)) };
        if found.is_none() {
            let (land, h) = (self.land, self.h);
            let fine = self.fine.get_or_insert_with(|| Level::new(w, h, 1, land, mi));
            let (ff, fi) = (ends(fine, &out), ends(fine, &inn));
            found = fine.search(land, mi, a, &ff, b, &fi).map(|p| (p, false));
        }
        let (nodes, coarse) = found?;
        let level = if coarse { &self.coarse } else { self.fine.as_ref().expect("searched") };
        let mut mid: Vec<u32> = vec![level.cell(nodes[0]) as u32];
        for p in nodes.windows(2) {
            mid.extend(level.bridge(p[0], p[1], self.land).expect("a step taken"));
            mid.push(level.cell(p[1]) as u32);
        }
        // The ways out of `a` to the first node and in from the last to `b`, cell by cell.
        let chain = |tree: &BTreeMap<u32, (f64, u32)>, port: usize, from: u32| {
            let mut v = vec![from];
            let mut c = from;
            while c as usize != port {
                c = tree[&c].1;
                v.push(c);
            }
            v
        };
        let mut cells = chain(&out, a, mid[0]);
        cells.reverse();
        cells.extend_from_slice(&mid[1..]);
        let tail = chain(&inn, b, *mid.last().expect("a node"));
        cells.extend_from_slice(&tail[1..]);
        let at = |c: u32| [(c as usize % w) as f64, (c as usize / w) as f64];
        let len = cells.windows(2).map(|p| { let (p, q) = (at(p[0]), at(p[1])); sqrt((p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1])) }).sum::<f64>() * mi;
        Some((cells, len))
    }
}

/// The lane network from the routed `links`: segments between ports and junctions, each with
/// its summed flow and class, smoothed into world-ft polylines (a port's end at its settlement's
/// own place, which `settlements` give) whose every point lies at sea but for the ports' own.
pub fn lanes(links: &[Link], settlements: &[Settlement], land: &[bool], w: usize, h: usize, cell_ft: f64) -> Vec<SeaLane> {
    lanes_with(links, settlements, land, w, h, cell_ft, true)
}

/// Segments shorter than this (mi) between junctions are merged away.
const STUB_MI: f64 = 3.0;
/// The same for a stub to a port (a diagonal step is 3.7 cells' worth of miles on some seeds).
const PORT_STUB_MI: f64 = 6.0;

/// `lanes`, with `merge_stubs` off for comparing (a fork a few miles short of a port stays).
pub fn lanes_with(links: &[Link], settlements: &[Settlement], land: &[bool], w: usize, h: usize, cell_ft: f64, merge_stubs: bool) -> Vec<SeaLane> {
    // The steps every link takes, with the goods over each.
    let mut step: FastMap<(u32, u32), f64> = Default::default();
    let mut port_of: BTreeMap<u32, u32> = BTreeMap::new();
    for l in links.iter().filter(|l| l.cells.len() >= 2) {
        port_of.insert(l.cells[0], l.ports[0]);
        port_of.insert(*l.cells.last().expect("routed"), l.ports[1]);
        for p in l.cells.windows(2) {
            *step.entry((p[0].min(p[1]), p[0].max(p[1]))).or_default() += l.flow;
        }
    }
    let mut steps: Vec<((u32, u32), f64)> = step.into_iter().collect();
    steps.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut adj: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &((a, b), _) in &steps {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    let flow_of = |a: u32, b: u32| steps.binary_search_by(|s| s.0.cmp(&(a.min(b), a.max(b)))).map(|i| steps[i].1).unwrap_or(0.0);
    let is_end = |c: u32| port_of.contains_key(&c) || adj.get(&c).is_some_and(|v| v.len() != 2);
    // Walk from each end along each of its steps to the next end.
    let mut done: crate::core::hash::FastSet<(u32, u32)> = Default::default();
    let mut segs: Vec<(Vec<u32>, f64)> = Vec::new();
    let starts: Vec<u32> = adj.keys().copied().filter(|&c| is_end(c)).collect();
    for &s in &starts {
        for &first in &adj[&s] {
            if done.contains(&(s.min(first), s.max(first))) {
                continue;
            }
            let mut cells = vec![s, first];
            done.insert((s.min(first), s.max(first)));
            let (mut prev, mut at) = (s, first);
            while !is_end(at) {
                let next = adj[&at].iter().copied().find(|&c| c != prev).expect("a degree-2 cell");
                if !done.insert((at.min(next), at.max(next))) {
                    break;
                }
                cells.push(next);
                (prev, at) = (at, next);
            }
            let flow = flow_of(cells[0], cells[1]);
            segs.push((cells, flow));
        }
    }
    if segs.is_empty() {
        return Vec::new();
    }
    let len = |c: &[u32]| c.windows(2).map(|p| { let (a, b) = (p[0] as usize, p[1] as usize); let (dx, dy) = ((a % w) as f64 - (b % w) as f64, (a / w) as f64 - (b / w) as f64); sqrt(dx * dx + dy * dy) }).sum::<f64>();
    // Stubs: a short segment with a junction end is contracted. Its junction moves to the other
    // end: the segments meeting there run on along the stub (their last cells), the heaviest
    // taking the stub's flow too. Repeated until no stub is left.
    let mut kept: Vec<(u32, u32)> = Vec::new();
    while merge_stubs {
        let mi = cell_ft / 5280.0;
        let ends_of = |c: &Vec<u32>| (c[0], *c.last().expect("cells"));
        // (Lengths are of the cell path: a port's last stretch over land is cut at the waterline later.)
        let drawn = |c: &Vec<u32>| len(c);
        let Some(si) = (0..segs.len())
            .filter(|&i| {
                let (a, b) = ends_of(&segs[i].0);
                a != b && !kept.contains(&(a, b)) && drawn(&segs[i].0) * mi < if port_of.contains_key(&a) || port_of.contains_key(&b) { PORT_STUB_MI } else { STUB_MI } && !(port_of.contains_key(&a) && port_of.contains_key(&b))
            })
            .min_by(|&x, &y| drawn(&segs[x].0).total_cmp(&drawn(&segs[y].0)).then(x.cmp(&y)))
        else {
            break;
        };
        let (stub, sflow) = segs.remove(si);
        let (a, b) = ends_of(&stub);
        // The junction to drop: the one that is not a port; of two junctions, the higher cell.
        let gone = if port_of.contains_key(&a) { b } else if port_of.contains_key(&b) { a } else { a.max(b) };
        let mut path: Vec<u32> = if stub[0] == gone { stub.clone() } else { stub.iter().rev().copied().collect() };
        path.remove(0);
        let keep = if gone == a { b } else { a };
        let meets = |c: &Vec<u32>| ends_of(c).0 == gone || ends_of(c).1 == gone;
        let heaviest = (0..segs.len()).filter(|&i| meets(&segs[i].0)).max_by(|&x, &y| segs[x].1.total_cmp(&segs[y].1).then(y.cmp(&x)));
        // (Not where it would leave a lane starting and ending at the same place.)
        let hook = segs.iter().any(|c| meets(&c.0) && (ends_of(&c.0).0 == keep || ends_of(&c.0).1 == keep));
        if heaviest.is_none() || hook {
            kept.push((a, b));
            segs.insert(si, (stub, sflow));
            continue;
        }
        for i in 0..segs.len() {
            if !meets(&segs[i].0) {
                continue;
            }
            if ends_of(&segs[i].0).1 != gone {
                segs[i].0.reverse();
            }
            segs[i].0.extend_from_slice(&path);
            if Some(i) == heaviest {
                segs[i].1 += sflow;
            }
        }
    }
    // Classes by flow quantiles (by length).
    let mut by: Vec<(f64, f64)> = segs.iter().map(|(c, f)| (*f, len(c))).collect();
    by.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let total: f64 = by.iter().map(|v| v.1).sum();
    let quantile = |q: f64| {
        let mut acc = 0.0;
        for &(f, l) in &by {
            acc += l;
            if acc >= q * total {
                return f;
            }
        }
        by.last().map_or(0.0, |v| v.0)
    };
    let (busy, quiet) = (quantile(BUSY_Q), quantile(QUIET_Q));
    let sea_at = |p: [f64; 2]| {
        let (i, j) = ((p[0] / cell_ft).round(), (p[1] / cell_ft).round());
        i >= 0.0 && j >= 0.0 && (i as usize) < w && (j as usize) < h && !land[j as usize * w + i as usize]
    };
    segs.into_iter()
        .map(|(cells, flow)| {
            let end = |c: u32| port_of.get(&c).copied().unwrap_or(JUNCTION);
            let ends = [end(cells[0]), end(*cells.last().expect("a step"))];
            let mut pts: Vec<[f64; 2]> = cells.iter().map(|&c| [(c as usize % w) as f64 * cell_ft, (c as usize / w) as f64 * cell_ft]).collect();
            let n = pts.len();
            for (e, at) in [(ends[0], 0), (ends[1], n - 1)] {
                if let Some(s) = settlements.get(e as usize) {
                    pts[at] = [s.x, s.y];
                }
            }
            let pts = smooth(&pts, [ends[0] != JUNCTION, ends[1] != JUNCTION], cell_ft, &sea_at);
            let class = if flow >= busy && busy > quiet { 0 } else if flow < quiet { 2 } else { 1 };
            SeaLane { pts, class, flow: flow as f32, ends }
        })
        .collect()
}

/// Ends each port end at the waterline: from the last point of the lane that stands in water
/// (`wet`) the way on toward the port is walked in ~100 ft steps and cut at the first dry one.
/// A pure function of the lanes and `wet`; a few hundred samples an end.
pub fn clip_ports(lanes: &mut [SeaLane], wet: &dyn Fn(f64, f64) -> bool) {
    const STEP: f64 = 100.0;
    for l in lanes.iter_mut() {
        for end in 0..2 {
            if l.ends[end] == JUNCTION || l.pts.len() < 2 {
                continue;
            }
            if end == 0 {
                l.pts.reverse();
            }
            // (The port is at the back.) The last wet point within the final few.
            let n = l.pts.len();
            let lo = n.saturating_sub(8);
            if let Some(i) = (lo..n - 1).rev().find(|&i| wet(l.pts[i][0], l.pts[i][1])) {
                let (a, b) = (l.pts[i], l.pts[i + 1]);
                let d = sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]));
                let steps = (d / STEP).ceil().max(1.0) as usize;
                let mut last = a;
                for k in 1..=steps {
                    let t = k as f64 / steps as f64;
                    let q = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
                    if !wet(q[0], q[1]) {
                        break;
                    }
                    last = q;
                }
                l.pts.truncate(i + 1);
                if last != a || l.pts.len() < 2 {
                    l.pts.push(last);
                }
            }
            if end == 0 {
                l.pts.reverse();
            }
        }
    }
}

/// Whether the straight way from `a` to `b` stays at sea (sampled every quarter cell).
fn clear(a: [f64; 2], b: [f64; 2], cell_ft: f64, sea_at: &dyn Fn([f64; 2]) -> bool) -> bool {
    let d = sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]));
    let n = (d / (0.25 * cell_ft)).ceil().max(1.0) as usize;
    (0..=n).all(|i| {
        let t = i as f64 / n as f64;
        sea_at([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])])
    })
}

/// A cell path's points smoothed: thinned where a straight chord stays at sea and within a third
/// of a cell, then corners cut (Chaikin, three times) where the cut stays at sea, then thinned
/// again. A port end (`port`) lies on land: the way from it to the next point is not checked.
fn smooth(pts: &[[f64; 2]], port: [bool; 2], cell_ft: f64, sea_at: &dyn Fn([f64; 2]) -> bool) -> Vec<[f64; 2]> {
    let n = pts.len();
    if n <= 2 {
        return pts.to_vec();
    }
    // (The hops out of a port, its first and last steps, stay as they are, unchecked.)
    let thin = |p: &[[f64; 2]], tol: f64| -> Vec<[f64; 2]> {
        let m = p.len();
        let (lo, hi) = (port[0] as usize, m - 1 - port[1] as usize);
        let mut keep = vec![false; m];
        for k in (0..=lo).chain(hi..m) {
            keep[k] = true;
        }
        let mut stack = if hi > lo { vec![(lo, hi)] } else { Vec::new() };
        while let Some((i, j)) = stack.pop() {
            if j <= i + 1 {
                continue;
            }
            let (a, b) = (p[i], p[j]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = sqrt(dx * dx + dy * dy).max(1e-9);
            let (mut worst, mut at) = (0.0, i + 1);
            for (k, q) in p.iter().enumerate().take(j).skip(i + 1) {
                let off = ((q[0] - a[0]) * dy - (q[1] - a[1]) * dx).abs() / l;
                if off > worst {
                    (worst, at) = (off, k);
                }
            }
            if worst > tol || !clear(a, b, cell_ft, sea_at) {
                keep[at] = true;
                stack.push((i, at));
                stack.push((at, j));
            }
        }
        p.iter().zip(&keep).filter(|v| *v.1).map(|v| *v.0).collect()
    };
    let mut p = thin(pts, cell_ft / 3.0);
    for _ in 0..3 {
        if p.len() <= 2 {
            break;
        }
        let m = p.len();
        let mut out = Vec::with_capacity(2 * m);
        out.push(p[0]);
        for i in 1..m - 1 {
            let (a, b, c) = (p[i - 1], p[i], p[i + 1]);
            let q = [0.75 * b[0] + 0.25 * a[0], 0.75 * b[1] + 0.25 * a[1]];
            let r = [0.75 * b[0] + 0.25 * c[0], 0.75 * b[1] + 0.25 * c[1]];
            // (A corner beside a port's hop, or one whose cut would touch land, stays.)
            let near_port = (i == 1 && port[0]) || (i == m - 2 && port[1]);
            if !near_port && clear(q, r, cell_ft, sea_at) {
                out.push(q);
                out.push(r);
            } else {
                out.push(b);
            }
        }
        out.push(p[m - 1]);
        p = out;
    }
    thin(&p, cell_ft / 20.0)
}

/// The lanes as compact JSON for the app (`T0::overlay_json`'s `sea_lanes`): per lane `c` its
/// class, `f` its flow, `e` its ends (settlement indices, -1 a junction) and `p` its points as a
/// flat list of whole feet (x, y, x, y, ...).
pub fn overlay_json(lanes: &[SeaLane]) -> String {
    use std::fmt::Write;
    let mut out = String::from("[");
    for (i, l) in lanes.iter().enumerate() {
        let end = |e: u32| if e == JUNCTION { -1 } else { e as i64 };
        let _ = write!(out, "{}{{\"c\":{},\"f\":{},\"e\":[{},{}],\"p\":[", if i > 0 { "," } else { "" }, l.class, (l.flow as f64 * 100.0).round() / 100.0, end(l.ends[0]), end(l.ends[1]));
        for (k, p) in l.pts.iter().enumerate() {
            let _ = write!(out, "{}{},{}", if k > 0 { "," } else { "" }, p[0].round() as i64, p[1].round() as i64);
        }
        out.push_str("]}");
    }
    out.push(']');
    out
}
