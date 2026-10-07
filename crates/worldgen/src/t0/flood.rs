//! Priority-flood depression filling (Barnes et al. 2014) and D8 flow routing, shared by
//! erosion and hydrology. Deterministic: heap ties break on cell index.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub const SQRT2: f64 = std::f64::consts::SQRT_2;
/// (dx, dy, distance in cells)
pub const D8: [(i32, i32, f64); 8] = [
    (1, 0, 1.0),
    (-1, 0, 1.0),
    (0, 1, 1.0),
    (0, -1, 1.0),
    (1, 1, SQRT2),
    (1, -1, SQRT2),
    (-1, 1, SQRT2),
    (-1, -1, SQRT2),
];

/// A heap entry: the height as an integer key ordered like `f64::total_cmp`, and the cell.
#[derive(PartialEq, Eq)]
struct Node {
    k: u64,
    i: u32,
}

impl Node {
    fn new(h: f64, i: u32) -> Node {
        // total_cmp's order as unsigned integers: flip all bits of negatives, the sign of the rest.
        let b = h.to_bits();
        Node { k: if b >> 63 == 1 { !b } else { b | (1 << 63) }, i }
    }
}

impl Ord for Node {
    // Reversed: BinaryHeap is a max-heap and we want the lowest cell first.
    fn cmp(&self, o: &Self) -> Ordering {
        o.k.cmp(&self.k).then_with(|| o.i.cmp(&self.i))
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[inline]
pub fn neighbors(w: usize, h: usize, i: usize) -> impl Iterator<Item = (usize, f64)> {
    let (x, y) = ((i % w) as i32, (i / w) as i32);
    D8.iter().filter_map(move |&(dx, dy, d)| {
        let (nx, ny) = (x + dx, y + dy);
        (nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h).then(|| (ny as usize * w + nx as usize, d))
    })
}

pub struct Flooded {
    /// Depression-filled surface; strictly increases by at least `eps` away from outlets.
    pub filled: Vec<f64>,
    /// Non-outlet cells in increasing `filled` order; each cell's receiver comes earlier.
    pub order: Vec<u32>,
}

/// Outlets drain the landscape (ocean). If there are none, the map border is used.
pub fn priority_flood(w: usize, h: usize, height: &[f64], outlet: &[bool], eps: f64) -> Flooded {
    let n = w * h;
    let mut filled = height.to_vec();
    let mut closed = vec![false; n];
    let mut heap = BinaryHeap::new();
    let mut any = false;
    // Cells off the grid's edge have all eight neighbours, at fixed offsets (in D8 order).
    let mut inner = vec![false; n];
    for y in 1..h.saturating_sub(1) {
        inner[y * w + 1..y * w + w - 1].iter_mut().for_each(|v| *v = true);
    }
    let offsets: [isize; 8] = std::array::from_fn(|k| D8[k].1 as isize * w as isize + D8[k].0 as isize);
    for i in 0..n {
        if outlet[i] {
            closed[i] = true;
            any = true;
            // Outlets with no open neighbour would pop without effect: only the ones on the
            // edge of the land enter the heap (the pop order is a total order on (height,
            // index), so the result is the same).
            let open = if inner[i] { offsets.iter().any(|&o| !outlet[(i as isize + o) as usize]) } else { neighbors(w, h, i).any(|(nb, _)| !outlet[nb]) };
            if open {
                heap.push(Node::new(height[i], i as u32));
            }
        }
    }
    if !any {
        for i in 0..n {
            let (x, y) = (i % w, i / w);
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                closed[i] = true;
                heap.push(Node::new(height[i], i as u32));
            }
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(Node { i, .. }) = heap.pop() {
        let i = i as usize;
        if !outlet[i] && any {
            order.push(i as u32);
        }
        let mut visit = |nb: usize| {
            if closed[nb] {
                return;
            }
            closed[nb] = true;
            filled[nb] = filled[nb].max(filled[i] + eps);
            heap.push(Node::new(filled[nb], nb as u32));
        };
        if inner[i] {
            for o in offsets {
                visit((i as isize + o) as usize);
            }
        } else {
            for (nb, _) in neighbors(w, h, i) {
                visit(nb);
            }
        }
    }
    Flooded { filled, order }
}

/// Steepest-descent receiver on `surface` for every cell (itself if it has no lower neighbor)
/// and the distance to it in cells.
pub fn receivers(w: usize, h: usize, surface: &[f64]) -> (Vec<u32>, Vec<f64>) {
    let n = w * h;
    let mut rec = vec![0u32; n];
    let mut dist = vec![1.0; n];
    for i in 0..n {
        let mut best = i;
        let mut best_s = 0.0;
        let mut best_d = 1.0;
        for (nb, d) in neighbors(w, h, i) {
            let s = (surface[i] - surface[nb]) / d;
            if s > best_s {
                best_s = s;
                best = nb;
                best_d = d;
            }
        }
        rec[i] = best as u32;
        dist[i] = best_d;
    }
    (rec, dist)
}
