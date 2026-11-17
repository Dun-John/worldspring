//! The street graph and its faces (blocks), and conflict checks.

use super::*;

/// Ids of the buildings a hand town's blocks lay out: `HAND_ID + (block × ROLLS + gen mod ROLLS)
/// × PER_PATCH + k` (k in the order the block builds them), above every id a kept block keeps
/// (`wards::ROLLED_ID`'s range); a row's take block number `ROW_BLOCK + row`. Blocks under
/// `ROW_BLOCK` and rows under `MAX_ROWS` have room.
pub const HAND_ID: u32 = 2_000_000_000;
pub const ROW_BLOCK: u32 = 3_000;
pub const MAX_ROWS: u32 = 350;
/// What of a hand town's records could not be laid out: (key, why).
pub type Problems = Vec<(String, &'static str)>;

/// The street graph: corners (junctions and bends, local ft) and the street pieces between them.
pub(in crate::town) struct Graph {
    pub(in crate::town) pts: Vec<P>,
    q: Vec<(i64, i64)>,
    /// (from, to, street id): from `a` toward `b`.
    pub(in crate::town) segs: Vec<(usize, usize, u32)>,
    /// The pieces of streets left out as dead ends (no block beside them), in the same form: drawn
    /// all the same (a blank town's first streets), never a block's side.
    pub(in crate::town) loose: Vec<(usize, usize, u32)>,
}

/// A face of the graph: its ring (corners, anticlockwise), and per side the street piece and
/// whether the face is on its left.
pub(in crate::town) struct Face {
    pub(in crate::town) ring: Vec<usize>,
    pub(in crate::town) side: Vec<(usize, bool)>,
}

pub(super) fn grid(p: P) -> (i64, i64) {
    (crate::core::round(p[0] * 256.0) as i64, crate::core::round(p[1] * 256.0) as i64)
}

fn orient(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> i128 {
    (b.0 - a.0) as i128 * (c.1 - a.1) as i128 - (b.1 - a.1) as i128 * (c.0 - a.0) as i128
}

/// Whether c lies on segment a-b strictly between its ends (exactly, by the grid).
pub(super) fn between(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> bool {
    orient(a, b, c) == 0 && c != a && c != b && (c.0 - a.0) as i128 * (c.0 - b.0) as i128 + (c.1 - a.1) as i128 * (c.1 - b.1) as i128 <= 0
}

impl Graph {
    /// The graph of `recs`' streets (local ft about `origin`), the streets that can't stand in it
    /// left out and said why: a missing junction, no length, crossing a street of a lower id (or
    /// touching it, or running along it), or ending nowhere (a dead end: no block beside it).
    pub(in crate::town) fn of(recs: &HandRecords, origin: P, li: &str, problems: &mut Problems) -> Graph {
        let mut g = Graph { pts: Vec::new(), q: Vec::new(), segs: Vec::new(), loose: Vec::new() };
        let mut at: FastMap<(i64, i64), usize> = FastMap::default();
        let mut corner = |g: &mut Graph, p: P| {
            let k = grid(p);
            *at.entry(k).or_insert_with(|| {
                g.pts.push(p);
                g.q.push(k);
                g.pts.len() - 1
            })
        };
        // Each street's pieces, in id order.
        let mut pieces: BTreeMap<u32, Vec<(usize, usize)>> = BTreeMap::new();
        for (&id, st) in &recs.streets {
            let (Some(a), Some(b)) = (recs.junctions.get(&st.a), recs.junctions.get(&st.b)) else {
                problems.push((format!("{li}/e{id}"), "no such junction"));
                continue;
            };
            let line: Vec<usize> = std::iter::once(a.p).chain(st.bends.iter().copied()).chain([b.p]).map(|p| corner(&mut g, sub(p, origin))).collect();
            let mut ps: Vec<(usize, usize)> = line.windows(2).filter(|w| w[0] != w[1]).map(|w| (w[0], w[1])).collect();
            // (A piece doubling straight back on the last is no street.)
            if ps.is_empty() || ps.windows(2).any(|w| w[0].0 == w[1].1) {
                problems.push((format!("{li}/e{id}"), "no length"));
                ps.clear();
            }
            if !ps.is_empty() {
                pieces.insert(id, ps);
            }
        }
        // Crossings: every pair of pieces of different streets meeting other than at a shared corner,
        // found by a sweep along x; the street of the higher id goes.
        let all: Vec<(usize, usize, u32)> = pieces.iter().flat_map(|(&id, ps)| ps.iter().map(move |&(u, v)| (u, v, id))).collect();
        let bx = |s: &(usize, usize, u32)| (g.q[s.0].0.min(g.q[s.1].0), g.q[s.0].0.max(g.q[s.1].0));
        let mut order: Vec<usize> = (0..all.len()).collect();
        order.sort_by_key(|&k| (bx(&all[k]).0, k));
        let mut meets: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (oi, &i) in order.iter().enumerate() {
            let (x1, (u, v, si)) = (bx(&all[i]).1, all[i]);
            let (a, b) = (g.q[u], g.q[v]);
            for &j in &order[oi + 1..] {
                if bx(&all[j]).0 > x1 {
                    break;
                }
                let (c, d, sj) = all[j];
                if si == sj {
                    continue;
                }
                let (pc, pd) = (g.q[c], g.q[d]);
                if a.1.max(b.1) < pc.1.min(pd.1) || pc.1.max(pd.1) < a.1.min(b.1) {
                    continue;
                }
                if conflict((u, v), (c, d), a, b, pc, pd) {
                    meets.entry(si.max(sj)).or_default().push(si.min(sj));
                }
            }
        }
        let mut kept: BTreeMap<u32, Vec<(usize, usize)>> = BTreeMap::new();
        for (id, ps) in pieces {
            if meets.get(&id).is_some_and(|m| m.iter().any(|o| kept.contains_key(o))) {
                problems.push((format!("{li}/e{id}"), "crosses another street"));
            } else {
                kept.insert(id, ps);
            }
        }
        // Dead ends, until none is left.
        loop {
            let mut deg = vec![0u32; g.pts.len()];
            for ps in kept.values() {
                for &(u, v) in ps {
                    deg[u] += 1;
                    deg[v] += 1;
                }
            }
            let dead: Vec<u32> = kept.iter().filter(|(_, ps)| ps.iter().any(|&(u, v)| deg[u] < 2 || deg[v] < 2)).map(|(&id, _)| id).collect();
            if dead.is_empty() {
                break;
            }
            for id in dead {
                if let Some(ps) = kept.remove(&id) {
                    g.loose.extend(ps.into_iter().map(|(u, v)| (u, v, id)));
                }
                problems.push((format!("{li}/e{id}"), "a dead end: no block beside it"));
            }
        }
        g.segs = kept.iter().flat_map(|(&id, ps)| ps.iter().map(move |&(u, v)| (u, v, id))).collect();
        g
    }

    /// Its faces by a walk round each half piece (each corner's pieces in exact angle order),
    /// the bounded ones (anticlockwise) and each part's outer edge (clockwise).
    fn faces(&self) -> (Vec<Face>, Vec<Face>) {
        let n = self.pts.len();
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        // (Half piece 2k runs from the piece's first corner, 2k + 1 back.)
        let from = |h: usize| if h.is_multiple_of(2) { self.segs[h / 2].0 } else { self.segs[h / 2].1 };
        let to = |h: usize| from(h ^ 1);
        for h in 0..2 * self.segs.len() {
            out[from(h)].push(h);
        }
        let dir = |h: usize| {
            let (a, b) = (self.q[from(h)], self.q[to(h)]);
            (b.0 - a.0, b.1 - a.1)
        };
        let half = |d: (i64, i64)| if d.1 > 0 || (d.1 == 0 && d.0 > 0) { 0 } else { 1 };
        for o in &mut out {
            o.sort_by(|&x, &y| {
                let (dx, dy) = (dir(x), dir(y));
                half(dx).cmp(&half(dy)).then_with(|| 0.cmp(&((dx.0 as i128) * (dy.1 as i128) - (dx.1 as i128) * (dy.0 as i128)))).then(x.cmp(&y))
            });
        }
        let mut pos = vec![0usize; 2 * self.segs.len()];
        for o in &out {
            for (k, &h) in o.iter().enumerate() {
                pos[h] = k;
            }
        }
        let mut seen = vec![false; 2 * self.segs.len()];
        let (mut bounded, mut outer) = (Vec::new(), Vec::new());
        for h0 in 0..2 * self.segs.len() {
            if seen[h0] {
                continue;
            }
            let mut f = Face { ring: Vec::new(), side: Vec::new() };
            let mut h = h0;
            let mut twice = 0i128;
            while !seen[h] {
                seen[h] = true;
                f.ring.push(from(h));
                f.side.push((h / 2, h.is_multiple_of(2)));
                let (a, b) = (self.q[from(h)], self.q[to(h)]);
                twice += a.0 as i128 * b.1 as i128 - b.0 as i128 * a.1 as i128;
                let o = &out[to(h)];
                h = o[(pos[h ^ 1] + o.len() - 1) % o.len()];
            }
            if twice > 0 { bounded.push(f) } else { outer.push(f) }
        }
        (bounded, outer)
    }
}

/// The graph of a town's streets with its faces: their rings (local ft), the parts held inside
/// each as holes (`outers`' indices), and the block each face is (the lowest-id block whose `at` it
/// holds, outside its holes, not taken by another face).
pub(in crate::town) struct Faces {
    pub(in crate::town) graph: Graph,
    pub(in crate::town) faces: Vec<Face>,
    pub(in crate::town) outers: Vec<Face>,
    pub(in crate::town) rings: Vec<Vec<P>>,
    pub(in crate::town) holes: Vec<Vec<usize>>,
    pub(in crate::town) face_block: Vec<Option<u32>>,
}

impl Faces {
    pub(in crate::town) fn of(recs: &HandRecords, origin: P, li: &str, problems: &mut Problems) -> Faces {
        let graph = Graph::of(recs, origin, li, problems);
        let (faces, outers) = graph.faces();
        let ring_pts = |f: &Face| f.ring.iter().map(|&v| graph.pts[v]).collect::<Vec<P>>();
        // Each part's outer edge is a hole in the smallest face of another part round it.
        let mut part = (0..graph.pts.len()).collect::<Vec<usize>>();
        fn root(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for &(u, v, _) in &graph.segs {
            let (a, b) = (root(&mut part, u), root(&mut part, v));
            part[a.max(b)] = a.min(b);
        }
        let rings: Vec<Vec<P>> = faces.iter().map(ring_pts).collect();
        let areas: Vec<f64> = rings.iter().map(|r| area(r)).collect();
        let mut holes: Vec<Vec<usize>> = vec![Vec::new(); faces.len()];
        for (k, o) in outers.iter().enumerate() {
            let p0 = graph.pts[o.ring[0]];
            let me = root(&mut part, o.ring[0]);
            let around = (0..faces.len()).filter(|&f| root(&mut part, faces[f].ring[0]) != me && contains(&rings[f], p0)).min_by(|&x, &y| areas[x].total_cmp(&areas[y]).then(x.cmp(&y)));
            if let Some(f) = around {
                holes[f].push(k);
            }
        }
        // Faces to blocks.
        let mut taken: BTreeMap<u32, usize> = BTreeMap::new();
        let mut face_block: Vec<Option<u32>> = vec![None; faces.len()];
        for (f, ring) in rings.iter().enumerate() {
            let inside = |p: P| contains(ring, p) && holes[f].iter().all(|&h| !contains(&ring_pts(&outers[h]), p));
            if let Some((&id, _)) = recs.blocks.iter().find(|(id, b)| !taken.contains_key(id) && inside(sub(b.at, origin))) {
                taken.insert(id, f);
                face_block[f] = Some(id);
            } else {
                problems.push((format!("{li}/e{}", graph.segs[faces[f].side[0].0].2), "a block with no record beside this street"));
            }
        }
        for &id in recs.blocks.keys() {
            if !taken.contains_key(&id) {
                problems.push((format!("{li}/k{id}"), "no block of the streets holds its point"));
            }
        }
        Faces { graph, faces, outers, rings, holes, face_block }
    }

    /// Face `f`'s ring as points (local ft).
    pub(in crate::town) fn ring_of(&self, f: &Face) -> Vec<P> {
        f.ring.iter().map(|&v| self.graph.pts[v]).collect()
    }
}

/// Whether two pieces meet anywhere but a corner they share.
fn conflict(s: (usize, usize), t: (usize, usize), a: (i64, i64), b: (i64, i64), c: (i64, i64), d: (i64, i64)) -> bool {
    let shared = [s.0, s.1].iter().filter(|x| **x == t.0 || **x == t.1).count();
    if shared == 2 {
        return true;
    }
    if shared == 1 {
        // (Meeting at their shared corner only, unless they run along each other from it.)
        let (p, q, r) = if s.0 == t.0 { (a, b, d) } else if s.0 == t.1 { (a, b, c) } else if s.1 == t.0 { (b, a, d) } else { (b, a, c) };
        return orient(p, q, r) == 0 && (q.0 - p.0) as i128 * (r.0 - p.0) as i128 + (q.1 - p.1) as i128 * (r.1 - p.1) as i128 > 0;
    }
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1.signum() * o2.signum() < 0 && o3.signum() * o4.signum() < 0 {
        return true;
    }
    between(a, b, c) || between(a, b, d) || between(c, d, a) || between(c, d, b) || a == c || a == d || b == c || b == d
}
