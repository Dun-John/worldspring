use crate::prelude::*;

pub(crate) type P = [f64; 2];
pub(crate) const CELL: f64 = 100.0;

/// River channel pieces (fine curve, ~20-ft chords) near a rectangle, in a grid.
pub(crate) struct Channel {
    pub(crate) segs: Vec<(P, P, f64)>,
    pub(crate) grid: HashMap<(i64, i64), Vec<usize>>,
}

impl Channel {
    pub(crate) fn new(t0: &T0, r: [f64; 4]) -> Channel {
        let mut segs = Vec::new();
        for (ri, k) in t0.rivers.segments_near(r[0], r[1], r[2], r[3], 0.0) {
            let rc = &t0.rivers.rivers[ri as usize];
            let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(2.0, 2000.0) as usize;
            let mut prev: Option<(P, f64)> = None;
            for j in 0..=n {
                let c = rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft);
                if let Some((q, w)) = prev {
                    segs.push((q, c.p, 0.5 * w.min(c.w)));
                }
                prev = Some((c.p, c.w));
            }
        }
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, &(a, b, hw)) in segs.iter().enumerate() {
            for cy in ((a[1].min(b[1]) - hw) / CELL).floor() as i64..=((a[1].max(b[1]) + hw) / CELL).floor() as i64 {
                for cx in ((a[0].min(b[0]) - hw) / CELL).floor() as i64..=((a[0].max(b[0]) + hw) / CELL).floor() as i64 {
                    grid.entry((cx, cy)).or_default().push(i);
                }
            }
        }
        Channel { segs, grid }
    }
    /// Inside the channel (less `inset` ft from its edge counts as outside).
    pub(crate) fn inside(&self, p: P, inset: f64) -> bool {
        let (cx, cy) = ((p[0] / CELL).floor() as i64, (p[1] / CELL).floor() as i64);
        self.grid.get(&(cx, cy)).into_iter().flatten().any(|&i| {
            let (a, b, hw) = self.segs[i];
            seg_dist(p, a, b) < hw - inset
        })
    }
}

/// Points over a polygon: corners, every ~5 ft along the edges, and the centroid.
pub(crate) fn outline(poly: &[P]) -> Vec<P> {
    let m = poly.len();
    let mut out = vec![centroid(poly)];
    for k in 0..m {
        let (a, b) = (poly[k], poly[(k + 1) % m]);
        let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
        out.extend((0..n).map(|j| lerp(a, b, j as f64 / n as f64)));
    }
    out
}

/// Points along a polyline every ~`step` ft (ends included).
pub(crate) fn along(pts: &[P], step: f64) -> Vec<P> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        let n = (dist(w[0], w[1]) / step).ceil().max(1.0) as usize;
        out.extend((0..n).map(|j| lerp(w[0], w[1], j as f64 / n as f64)));
    }
    if let Some(&l) = pts.last() {
        out.push(l);
    }
    out
}

pub(crate) fn angle(a: P, b: P) -> f64 {
    let (la, lb) = ((a[0] * a[0] + a[1] * a[1]).sqrt(), (b[0] * b[0] + b[1] * b[1]).sqrt());
    if la < 1e-9 || lb < 1e-9 {
        return 0.0;
    }
    ((a[0] * b[0] + a[1] * b[1]) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Sharpest turn (deg) along a polyline between chords of about `chord` ft, and where.
pub(crate) fn sharpest(pts: &[P], chord: f64) -> (f64, P) {
    let s = along(pts, 5.0);
    let k = (chord / 5.0).round().max(1.0) as usize;
    let mut best = (0.0, pts[0]);
    for i in k..s.len().saturating_sub(k) {
        let t = angle(sub(s[i], s[i - k]), sub(s[i + k], s[i]));
        if t > best.0 {
            best = (t, s[i]);
        }
    }
    best
}

#[derive(Default)]
pub(crate) struct Tally {
    pub(crate) n: usize,
    pub(crate) worst: Vec<(f64, P, String)>,
}

impl Tally {
    pub(crate) fn add(&mut self, size: f64, at: P, what: String) {
        self.n += 1;
        self.worst.push((size, at, what));
    }
    /// Prints the count and the worst few, and records them under `key` in `counts` and `worst`.
    pub(crate) fn print(&mut self, label: &str, key: &str, counts: &mut Map<String, Value>, worst: &mut Map<String, Value>) {
        self.worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        let top = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        say!("  {label}: {}", self.n);
        for (_, _, w) in self.worst.iter().take(top) {
            say!("      {w}");
        }
        counts.insert(key.into(), json!(self.n));
        worst.insert(key.into(), self.worst.iter().take(top).map(|(size, at, _)| json!([at[0].round(), at[1].round(), size])).collect());
    }
}

pub(crate) fn seg_cross(a: P, b: P, c: P, d: P) -> bool {
    let side = |p: P, q: P, r: P| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0
}

pub(crate) const MI: f64 = 5280.0;

/// Value at fraction `q` of a sorted list.
pub(crate) fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() { 0.0 } else { v[((q * v.len() as f64) as usize).min(v.len() - 1)] }
}

pub(crate) fn dijkstra(adj: &[Vec<(usize, f64)>], src: usize) -> Vec<f64> {
    use std::cmp::Reverse;
    let mut d = vec![f64::INFINITY; adj.len()];
    let mut heap = std::collections::BinaryHeap::new();
    d[src] = 0.0;
    heap.push(Reverse((0u64, src)));
    while let Some(Reverse((k, u))) = heap.pop() {
        let du = f64::from_bits(k);
        if du > d[u] {
            continue;
        }
        for &(v, w) in &adj[u] {
            let nd = du + w;
            if nd < d[v] {
                d[v] = nd;
                heap.push(Reverse((nd.to_bits(), v)));
            }
        }
    }
    d
}

/// Points within `tol` ft of each other get one id (the first one's place stays).
pub(crate) struct Spots {
    pub(crate) tol: f64,
    pub(crate) pos: Vec<P>,
    pub(crate) grid: HashMap<(i64, i64), Vec<usize>>,
}

impl Spots {
    pub(crate) fn id(&mut self, p: P) -> usize {
        let (cx, cy) = ((p[0] / self.tol).floor() as i64, (p[1] / self.tol).floor() as i64);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &i in self.grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if dist(self.pos[i], p) <= self.tol {
                        return i;
                    }
                }
            }
        }
        self.pos.push(p);
        self.grid.entry((cx, cy)).or_default().push(self.pos.len() - 1);
        self.pos.len() - 1
    }
}

/// The point `reach` ft along `pts` from index `from` towards `to` (the far end if it is shorter).
pub(crate) fn along_from(pts: &[P], from: usize, to: usize, reach: f64) -> P {
    let step: isize = if to > from { 1 } else { -1 };
    let (mut i, mut acc) = (from as isize, 0.0);
    while i != to as isize {
        let (a, b) = (pts[i as usize], pts[(i + step) as usize]);
        let l = dist(a, b);
        if acc + l >= reach {
            return lerp(a, b, (reach - acc) / l.max(1e-9));
        }
        acc += l;
        i += step;
    }
    pts[to]
}
