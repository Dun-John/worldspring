//! Shared-vertex patch mesh: Voronoi patches whose corners are shared between neighbours,
//! so moving a corner (smoothing a wall or a main street) bends every patch that meets it.
//! After watabou's patch model. All edits keep every face convex, which the block and lot
//! code (half-plane insets and cuts) relies on.

use crate::core::hash::{FastMap, FastSet};

use super::geom::*;

#[derive(Clone)]
pub struct Mesh {
    pub pos: Vec<P>,
    /// Vertex ids per face; edge k runs from `faces[f][k]` to `faces[f][k + 1]`.
    pub faces: Vec<Vec<usize>>,
    /// Neighbour face across edge k (−1 = none).
    pub labels: Vec<Vec<i32>>,
}

#[inline]
fn qkey(p: P) -> (i64, i64) {
    (crate::core::round(p[0] * 2.0) as i64, crate::core::round(p[1] * 2.0) as i64)
}

impl Mesh {
    /// Faces from clipped Voronoi cells; corners within half a foot are one vertex.
    pub fn from_cells(cells: &[Labeled]) -> Mesh {
        let mut ids: FastMap<(i64, i64), usize> = FastMap::default();
        let mut pos: Vec<P> = Vec::new();
        let mut faces = Vec::with_capacity(cells.len());
        let mut labels = Vec::with_capacity(cells.len());
        for c in cells {
            let mut f: Vec<usize> = Vec::with_capacity(c.pts.len());
            let mut lb: Vec<i32> = Vec::with_capacity(c.pts.len());
            for (k, p) in c.pts.iter().enumerate() {
                let id = *ids.entry(qkey(*p)).or_insert_with(|| {
                    pos.push(*p);
                    pos.len() - 1
                });
                f.push(id);
                lb.push(c.labels[k]);
            }
            let (f, lb) = dedupe(f, lb);
            faces.push(if f.len() >= 3 { f } else { Vec::new() });
            labels.push(if lb.len() >= 3 { lb } else { Vec::new() });
        }
        Mesh { pos, faces, labels }
    }

    pub fn face_pts(&self, f: usize) -> Vec<P> {
        self.faces[f].iter().map(|&v| self.pos[v]).collect()
    }

    /// Faces around each vertex.
    pub fn vertex_faces(&self) -> Vec<Vec<usize>> {
        let mut vf = vec![Vec::new(); self.pos.len()];
        for (f, face) in self.faces.iter().enumerate() {
            for &v in face {
                vf[v].push(f);
            }
        }
        vf
    }

    fn face_convex(&self, f: usize, moved: Option<(usize, P)>) -> bool {
        let at = |v: usize| match moved {
            Some((mv, p)) if mv == v => p,
            _ => self.pos[v],
        };
        let face = &self.faces[f];
        let m = face.len();
        if m < 3 {
            return true;
        }
        let mut sign = 0.0f64;
        for k in 0..m {
            let (a, b, c) = (at(face[k]), at(face[(k + 1) % m]), at(face[(k + 2) % m]));
            let cr = cross(sub(b, a), sub(c, b));
            if cr.abs() < 1e-6 {
                continue;
            }
            if sign == 0.0 {
                sign = cr.signum();
            } else if cr.signum() != sign {
                return false;
            }
        }
        sign != 0.0
    }

    /// Collapse edges shorter than `min_len` into their midpoint while both faces keep at
    /// least 4 corners and every face stays convex (shortest first; deterministic).
    pub fn collapse_short_edges(&mut self, min_len: f64) {
        let mut rejected: FastSet<(usize, usize)> = Default::default();
        loop {
            let mut best: Option<(f64, usize, usize)> = None;
            for face in &self.faces {
                let m = face.len();
                for k in 0..m {
                    let (a, b) = (face[k], face[(k + 1) % m]);
                    let key = (a.min(b), a.max(b));
                    let l = dist(self.pos[a], self.pos[b]);
                    if l < min_len && !rejected.contains(&key) && best.is_none_or(|x| l < x.0 || (l == x.0 && key < (x.1, x.2))) {
                        best = Some((l, key.0, key.1));
                    }
                }
            }
            let Some((_, u, v)) = best else { break };
            // Faces that have the edge must keep 4+ corners.
            let has_edge = |face: &Vec<usize>| {
                let m = face.len();
                (0..m).any(|k| (face[k] == u && face[(k + 1) % m] == v) || (face[k] == v && face[(k + 1) % m] == u))
            };
            if self.faces.iter().any(|f| has_edge(f) && f.len() < 5) {
                rejected.insert((u, v));
                continue;
            }
            let mid = lerp(self.pos[u], self.pos[v], 0.5);
            let (old_u, old_v) = (self.pos[u], self.pos[v]);
            let old_faces = self.faces.clone();
            let old_labels = self.labels.clone();
            self.pos[u] = mid;
            self.pos[v] = mid;
            for f in 0..self.faces.len() {
                if !self.faces[f].contains(&v) {
                    continue;
                }
                let face: Vec<usize> = self.faces[f].iter().map(|&x| if x == v { u } else { x }).collect();
                let (face, lb) = dedupe(face, self.labels[f].clone());
                self.faces[f] = face;
                self.labels[f] = lb;
            }
            let touched: Vec<usize> = (0..self.faces.len()).filter(|&f| self.faces[f].contains(&u)).collect();
            if touched.iter().any(|&f| self.faces[f].len() < 3 || !self.face_convex(f, None)) {
                self.faces = old_faces;
                self.labels = old_labels;
                self.pos[u] = old_u;
                self.pos[v] = old_v;
                rejected.insert((u, v));
            }
        }
    }

    /// Move vertex `v` toward `target`, halving the step up to three times if a face around
    /// it would lose convexity. Returns whether it moved.
    pub fn try_move(&mut self, vf: &[Vec<usize>], v: usize, target: P) -> bool {
        let p = self.pos[v];
        for step in [1.0, 0.5, 0.25] {
            let q = lerp(p, target, step);
            if vf[v].iter().all(|&f| self.face_convex(f, Some((v, q)))) {
                self.pos[v] = q;
                return true;
            }
        }
        false
    }

    /// Laplacian smoothing of a vertex chain: each pass moves every free vertex halfway to
    /// the midpoint of its chain neighbours (targets from the positions at the pass start).
    pub fn smooth_chain(&mut self, vf: &[Vec<usize>], chain: &[usize], closed: bool, fixed: &dyn Fn(usize) -> bool, passes: usize) {
        let n = chain.len();
        if n < 3 {
            return;
        }
        for _ in 0..passes {
            let mut targets: Vec<(usize, P)> = Vec::new();
            for k in 0..n {
                if !closed && (k == 0 || k == n - 1) {
                    continue;
                }
                let v = chain[k];
                if fixed(v) {
                    continue;
                }
                let (a, b) = (self.pos[chain[(k + n - 1) % n]], self.pos[chain[(k + 1) % n]]);
                let mid = lerp(a, b, 0.5);
                targets.push((v, lerp(self.pos[v], mid, 0.5)));
            }
            for (v, t) in targets {
                self.try_move(vf, v, t);
            }
        }
    }
}

impl Mesh {
    /// Pull a face toward the regular polygon that fits it best (watabou's citadel
    /// `equalize`): each pass moves every free corner a fifth of the way to its place on the
    /// fitted n-gon, until the face is round enough (4πA/P² ≥ `target`).
    pub fn equalize(&mut self, vf: &[Vec<usize>], f: usize, fixed: &dyn Fn(usize) -> bool, target: f64) {
        let n = self.faces[f].len();
        if n < 3 {
            return;
        }
        for _ in 0..12 {
            let pts = self.face_pts(f);
            let a = area(&pts);
            let per: f64 = (0..n).map(|i| dist(pts[i], pts[(i + 1) % n])).sum();
            if 4.0 * std::f64::consts::PI * a.abs() / (per * per).max(1e-9) >= target {
                return;
            }
            let c = centroid(&pts);
            let rad = pts.iter().map(|p| dist(*p, c)).sum::<f64>() / n as f64;
            let dir = if a > 0.0 { 1.0 } else { -1.0 };
            let step = std::f64::consts::TAU / n as f64;
            let (mut sn, mut cs) = (0.0, 0.0);
            for (i, p) in pts.iter().enumerate() {
                let th = libm::atan2(p[1] - c[1], p[0] - c[0]) - dir * step * i as f64;
                sn += libm::sin(th);
                cs += libm::cos(th);
            }
            let phi = libm::atan2(sn, cs);
            let face = self.faces[f].clone();
            for (i, &v) in face.iter().enumerate() {
                if fixed(v) {
                    continue;
                }
                let th = phi + dir * step * i as f64;
                let goal = [c[0] + rad * libm::cos(th), c[1] + rad * libm::sin(th)];
                let p = self.pos[v];
                self.try_move(vf, v, lerp(p, goal, 0.2));
            }
        }
    }
}

/// Remove degenerate edges (a vertex repeated next to itself), keeping each remaining edge's
/// label: pair (v_k, l_k) is dropped when v_k equals the next vertex.
fn dedupe(face: Vec<usize>, labels: Vec<i32>) -> (Vec<usize>, Vec<i32>) {
    let m = face.len();
    let mut f = Vec::with_capacity(m);
    let mut l = Vec::with_capacity(m);
    for k in 0..m {
        if face[k] != face[(k + 1) % m] {
            f.push(face[k]);
            l.push(labels[k]);
        }
    }
    (f, l)
}
