//! The ground under a town plan: height and river columns, shores, and river samples.

use super::*;

/// The ground settlements are planned on (`Site::height`), which the terrain is also reset
/// to under their pads (`lod::terrain_refine`): `T0::ground_at` cut by the river valleys
/// (`RiverNet::valley`), the valleys' depth interpolated over a world lattice. Subdividing
/// the coarse levels alone drifts from it by tens of feet on coasts, enough to move the
/// shore under piers and buildings.
pub struct PlanGround<'a> {
    t0: &'a T0,
    /// Spacing (ft) of the last coarse terrain level (`T0::ground_at`'s lattice) and its
    /// nodes over the area (first node index on each axis, row width, values).
    lattice_ft: f64,
    nodes: (i64, i64, usize, Vec<f64>),
    /// The valley lattice's spacing (ft); its nodes' depth (≤ 0) over the area, filled as
    /// asked (NaN until then; first node index on each axis, row width, values), and any
    /// beyond it.
    cell: f64,
    depth: RefCell<(i64, i64, usize, Vec<f32>, FastMap<(i64, i64), f32>)>,
    /// The river chords that can cut the area's valley nodes (`RiverNet::valley_chords`).
    chords: Vec<([f32; 4], [f32; 4])>,
}

/// The valley lattice's spacing (ft) under settlements: layouts and the finest terrain
/// levels interpolate the same nodes.
pub const VALLEY_CELL: f64 = 50.0;

impl<'a> PlanGround<'a> {
    /// Ground nodes over `ground_rect` and valley nodes every `cell` ft over `valley_rect`
    /// (x0, y0, x1, y1 world ft).
    pub fn new(t0: &'a T0, ground_rect: [f64; 4], valley_rect: [f64; 4], lattice_ft: f64, cell: f64) -> PlanGround<'a> {
        let r = ground_rect;
        let (nx0, ny0) = (crate::core::floor(r[0] / lattice_ft) as i64 - 2, crate::core::floor(r[1] / lattice_ft) as i64 - 2);
        let (nx1, ny1) = (crate::core::floor(r[2] / lattice_ft) as i64 + 3, crate::core::floor(r[3] / lattice_ft) as i64 + 3);
        let nw = (nx1 - nx0 + 1) as usize;
        let values: Vec<f64> = (ny0..=ny1).flat_map(|j| (nx0..=nx1).map(move |i| (i, j))).map(|(i, j)| t0.ground_node(i, j, lattice_ft)).collect();
        let r = valley_rect;
        let (ci0, cj0) = (crate::core::floor(r[0] / cell) as i64 - 1, crate::core::floor(r[1] / cell) as i64 - 1);
        let (ci1, cj1) = (crate::core::floor(r[2] / cell) as i64 + 2, crate::core::floor(r[3] / cell) as i64 + 2);
        let (cw, n) = ((ci1 - ci0 + 1) as usize, ((ci1 - ci0 + 1) * (cj1 - cj0 + 1)) as usize);
        let chords = t0.rivers.valley_chords([ci0 as f64 * cell, cj0 as f64 * cell, ci1 as f64 * cell, cj1 as f64 * cell]);
        // Where no chord reaches the area its nodes have no depth.
        let depth = vec![if chords.is_empty() { 0.0 } else { f32::NAN }; n];
        PlanGround { t0, lattice_ft, nodes: (nx0, ny0, nw, values), cell, depth: RefCell::new((ci0, cj0, cw, depth, FastMap::default())), chords }
    }

    /// The T0 surface (`T0::ground_at`; `bank` false leaves out the shore bank at the point,
    /// for callers that know no water is near or bank it themselves).
    pub fn ground(&self, x: f64, y: f64, bank: bool) -> f64 {
        self.t0.ground_with_nodes(x, y, self.lattice_ft, bank, |i, j| self.node(i, j))
    }

    /// Ground lattice node (i, j).
    fn node(&self, i: i64, j: i64) -> f64 {
        let (x0, y0, nw, values) = &self.nodes;
        let (a, b) = (i - x0, j - y0);
        if a >= 0 && b >= 0 && (a as usize) < *nw && ((b as usize) * nw + a as usize) < values.len() {
            values[b as usize * nw + a as usize]
        } else {
            self.t0.ground_node(i, j, self.lattice_ft)
        }
    }

    /// How far the river valleys lower the ground at (x, y) (≤ 0): smooth, interpolated
    /// between lattice nodes.
    pub fn depth(&self, x: f64, y: f64) -> f64 {
        let (u, v) = (x / self.cell, y / self.cell);
        let (i0, j0) = (crate::core::floor(u), crate::core::floor(v));
        let (fu, fv) = (u - i0, v - j0);
        let (i, j) = (i0 as i64, j0 as i64);
        let (a, b, c, d) = (self.depth_node(i, j), self.depth_node(i + 1, j), self.depth_node(i, j + 1), self.depth_node(i + 1, j + 1));
        let top = a + (b - a) * fu;
        let bottom = c + (d - c) * fu;
        top + (bottom - top) * fv
    }

    /// The valleys' depth at valley node (i, j), worked out once.
    fn depth_node(&self, i: i64, j: i64) -> f64 {
        let mut cache = self.depth.borrow_mut();
        let (ci0, cj0, cw) = (cache.0, cache.1, cache.2);
        let node = |inside: bool| {
            let (qx, qy) = (i as f64 * self.cell, j as f64 * self.cell);
            let h = self.ground(qx, qy, true);
            let v = if inside { crate::lod::rivers::RiverNet::valley_with(&self.chords, h, qx, qy) } else { self.t0.rivers.valley(h, qx, qy) };
            (v - h) as f32
        };
        let (a, b) = (i - ci0, j - cj0);
        if a >= 0 && b >= 0 && (a as usize) < cw && (b as usize) * cw + (a as usize) < cache.3.len() {
            let k = b as usize * cw + a as usize;
            if cache.3[k].is_nan() {
                cache.3[k] = node(true);
            }
            cache.3[k] as f64
        } else {
            *cache.4.entry((i, j)).or_insert_with(|| node(false)) as f64
        }
    }

    /// What `row` needs of each of `xs` (ascending), worked out once for every row.
    pub fn columns(&self, xs: &[f64]) -> PlanColumns {
        let (Some(&lo), Some(&hi)) = (xs.first(), xs.last()) else { return PlanColumns::default() };
        let (l, c) = (self.lattice_ft, self.cell);
        let (a0, c0) = (crate::core::floor(lo / l) as i64 - 1, crate::core::floor(lo / c) as i64);
        let at = xs
            .iter()
            .map(|&x| {
                let (u, w) = (x / l, x / c);
                let (i0, k0) = (crate::core::floor(u), crate::core::floor(w));
                ((i0 as i64 - 1 - a0) as usize, crate::t0::catmull_rom(u - i0), (k0 as i64 - c0) as usize, w - k0)
            })
            .collect();
        PlanColumns { nodes: (a0, crate::core::floor(hi / l) as i64 + 2), cells: (c0, crate::core::floor(hi / c) as i64 + 1), at }
    }

    /// (`ground(x, y, false)`, `depth(x, y)`) at each of the columns' xs along the row at `y`:
    /// the same interpolation, down each node column once for the whole row.
    pub fn row(&self, y: f64, cols: &PlanColumns) -> Vec<(f64, f64)> {
        let v = y / self.lattice_ft;
        let j0 = crate::core::floor(v);
        let wv = crate::t0::catmull_rom(v - j0);
        let col: Vec<f64> = (cols.nodes.0..=cols.nodes.1).map(|i| (0..4).map(|b| wv[b] * self.node(i, j0 as i64 + b as i64 - 1)).sum()).collect();
        let v = y / self.cell;
        let cj = crate::core::floor(v);
        let fv = v - cj;
        let dcol: Vec<f64> = (cols.cells.0..=cols.cells.1)
            .map(|i| {
                let (a, b) = (self.depth_node(i, cj as i64), self.depth_node(i, cj as i64 + 1));
                a + (b - a) * fv
            })
            .collect();
        cols.at.iter().map(|&(k, wu, d, fu)| (wu[0] * col[k] + wu[1] * col[k + 1] + wu[2] * col[k + 2] + wu[3] * col[k + 3], dcol[d] + (dcol[d + 1] - dcol[d]) * fu)).collect()
    }
}

/// Sea and lake over an area as the map draws them (for things placed by hand and built on
/// open ground): the water mask's level above the drawn ground. That is the smooth T0 surface
/// refinement starts from (`T0::ground_at`), in a settlement's river valleys under its pad
/// (`PlanGround`, as `lod::terrain_refine` resets it), not the T0 grid, which on a harbour's
/// waterfront can lie tens of feet (on steep coasts hundreds) below it. Worked out once, on
/// the first point asked about.
pub struct Shore<'a> {
    t0: &'a T0,
    /// Pads reaching the area: (x, y, reach).
    pads: Vec<(f64, f64, f64)>,
    rect: [f64; 4],
    lattice_ft: f64,
    plan: std::cell::OnceCell<PlanGround<'a>>,
    /// The rivers' curve samples, shared by every `in_river`.
    rivers: crate::lod::rivers::ClearCache,
    /// Islets meeting the area, drawn by hand (`ground::islets_in`) and the world's (`t0::islets`):
    /// dry land in the water.
    islets: Vec<(Vec<P>, f32, [f64; 4])>,
    /// The hand plateaus and brushed ground meeting the area (`ground::Shaper`): dry where they
    /// raise the ground over the water.
    shaped: Option<crate::ground::Shaper<'a>>,
}

impl<'a> Shore<'a> {
    /// Over `rect` (x0, y0, x1, y1 world ft).
    pub fn new(world: &'a World, t0: &'a T0, rect: [f64; 4]) -> Shore<'a> {
        let lattice_ft = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
        let mut shore = Self::of_t0(t0, rect, lattice_ft);
        shore.islets = crate::ground::islets_in(&world.file.edits, rect);
        shore.shaped = crate::ground::Shaper::build(&world.file.edits, None, rect, crate::battlemap::SQUARE_FT);
        // (R5 M20: and the world's own islets, `t0::islets`.)
        shore.islets.extend(crate::t0::islets::near_in(t0, &world.file.edits, rect).map(|i| (i.pts.clone(), i.level, i.bbox)));
        shore
    }

    /// `new` from T0 alone: no islets (`t0::islets` places the world's own with it).
    pub(crate) fn of_t0(t0: &'a T0, rect: [f64; 4], lattice_ft: f64) -> Shore<'a> {
        let pads = t0
            .settlements
            .iter()
            .map(|s| (s.x, s.y, reach(s)))
            .filter(|&(x, y, r)| {
                let e = pad_extent(r);
                x + e >= rect[0] && x - e <= rect[2] && y + e >= rect[1] && y - e <= rect[3]
            })
            .collect();
        Shore { t0, pads, rect, lattice_ft, plan: std::cell::OnceCell::new(), rivers: Default::default(), islets: Vec::new(), shaped: None }
    }

    /// The drawn ground at `p` (world ft), without its fine detail.
    pub fn ground(&self, p: P) -> f64 {
        let r = self.rect;
        let plan = self.plan.get_or_init(|| PlanGround::new(self.t0, r, r, self.lattice_ft, VALLEY_CELL));
        let pad = self.pads.iter().fold(0.0f64, |m, &(x, y, r)| m.max(pad_weight(crate::core::sqrt((p[0] - x) * (p[0] - x) + (p[1] - y) * (p[1] - y)), r)));
        let g = plan.ground(p[0], p[1], true);
        if pad > 0.0 { g + plan.depth(p[0], p[1]) * pad } else { g }
    }

    /// (Roadmap 6 P7) The ground at `p` as shaped by hand (plateaus, the brush), where shaping
    /// meets the area (else None).
    pub fn shaped_ground(&self, p: P) -> Option<f64> {
        let s = self.shaped.as_ref()?;
        let g = self.ground(p);
        Some(s.at_point(p[0], p[1], g as f32, self.t0.sample_water(p[0], p[1])) as f64)
    }

    /// Whether `p` is on an islet drawn by hand, above the water there.
    pub fn on_islet(&self, p: P) -> bool {
        self.islets.iter().any(|(pts, level, b)| {
            p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3] && geom::contains(pts, p) && *level > self.t0.sample_water(p[0], p[1]) + 0.5
        })
    }

    /// Whether `p` is in a river channel or within 2 ft of its water (not on an islet in it, nor
    /// a plateau over the water: `raised`).
    pub fn in_river(&self, p: P) -> bool {
        let q = crate::lod::rivers::clear_of_rivers_with(&self.t0.rivers, p[0], p[1], 2.0, self.t0.cell_ft, &self.rivers);
        (q.0 - p[0]).abs() + (q.1 - p[1]).abs() > 0.01 && !(!self.islets.is_empty() && self.on_islet(p) || self.raised(p))
    }

    /// Standing water at `p` (world ft), water drawn by hand too (`waters`); an islet drawn by
    /// hand is dry, and so is ground a plateau (or the brush) raises above the water.
    pub fn wet(&self, p: P) -> bool {
        if !self.islets.is_empty() && self.on_islet(p) {
            return false;
        }
        let level = self.t0.sample_water(p[0], p[1]);
        if let Some(s) = self.shaped.as_ref().filter(|s| s.has_waters())
            && let Some(l) = s.outline_level(p[0], p[1])
        {
            let l = l.max(level);
            return s.at_point(p[0], p[1], self.ground(p) as f32, level) < l;
        }
        level > crate::t0::hydro::DRY && level as f64 > self.ground(p) && !self.raised(p)
    }

    /// Whether a plateau (or the brush) holds the ground at `p` above the sea, lake or river
    /// there (a river's: its surface over the channel's bed, as battlemaps shape it).
    pub fn raised(&self, p: P) -> bool {
        let Some(s) = self.shaped.as_ref().filter(|s| {
            let r = s.reach();
            p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3]
        }) else {
            return false;
        };
        let level = self.t0.sample_water(p[0], p[1]);
        // (Water drawn by hand: its surface, as `wet` reads it.)
        let level = s.outline_level(p[0], p[1]).map_or(level, |l| l.max(level));
        if level > crate::t0::hydro::DRY {
            return s.at_point(p[0], p[1], self.ground(p) as f32, level) > level;
        }
        let Some((z, depth)) = crate::lod::rivers::surface_near(&self.t0.rivers, p[0], p[1], 2.5, self.t0.cell_ft) else {
            return false;
        };
        let bed = self.ground(p).min(z - depth);
        s.at_point(p[0], p[1], bed as f32, z as f32) as f64 > z
    }
}

/// `PlanGround::columns`: the ground node and valley cell column ranges, and per x its first
/// node column (from the range's start) and Catmull-Rom weights, its valley cell column and
/// fraction across it.
#[derive(Default)]
pub struct PlanColumns {
    nodes: (i64, i64),
    cells: (i64, i64),
    at: Vec<(usize, [f64; 4], usize, f64)>,
}

pub(super) const RIVER_CELL: f64 = 250.0;
/// Network road chords near a settlement are binned by this (`Site::near_network_road`, margins
/// up to 10 ft).
pub(super) const ROAD_CELL: f64 = 100.0;

/// Shoreline points of standing water (sea or lake) near the settlement: where the wet test
/// changes between neighbouring points of a `step` grid, refined by bisection. With each:
/// the unit normal pointing into the water.
pub(super) fn shore_points(site: &Site, step: f64, within: f64) -> Vec<(P, P)> {
    let n = (2.0 * within / step).ceil() as i64;
    let at = |i: i64, j: i64| [-within + i as f64 * step, -within + j as f64 * step];
    let wet_grid: Vec<bool> = (0..=n).flat_map(|j| (0..=n).map(move |i| (i, j))).map(|(i, j)| {
        let p = at(i, j);
        len(p) < within && site.wet_standing(p)
    }).collect();
    let idx = |i: i64, j: i64| (j * (n + 1) + i) as usize;
    let mut out: Vec<(P, P)> = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            for (di, dj) in [(1i64, 0i64), (0, 1)] {
                let (i2, j2) = (i + di, j + dj);
                if i2 > n || j2 > n || wet_grid[idx(i, j)] == wet_grid[idx(i2, j2)] {
                    continue;
                }
                let (mut dry, mut wetp) = if wet_grid[idx(i, j)] { (at(i2, j2), at(i, j)) } else { (at(i, j), at(i2, j2)) };
                if len(dry) >= within || len(wetp) >= within {
                    continue;
                }
                for _ in 0..6 {
                    let m = lerp(dry, wetp, 0.5);
                    if site.wet_standing(m) { wetp = m } else { dry = m }
                }
                let q = lerp(dry, wetp, 0.5);
                // Normal from the local wet gradient (8 samples around the point).
                let mut g = [0.0, 0.0];
                for k in 0..8 {
                    let ang = std::f64::consts::TAU * k as f64 / 8.0;
                    let d = [libm::cos(ang), libm::sin(ang)];
                    if site.wet_standing(add(q, mul(d, 0.5 * step))) {
                        g = add(g, d);
                    }
                }
                let gl = len(g);
                if gl > 1e-6 {
                    out.push((q, mul(g, 1.0 / gl)));
                }
            }
        }
    }
    out
}

/// Mesh vertices joined to `v` by an edge.
pub(super) fn adj_vertices(mesh: &Mesh, v: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for face in &mesh.faces {
        let m = face.len();
        for k in 0..m {
            if face[k] == v {
                for u in [face[(k + m - 1) % m], face[(k + 1) % m]] {
                    if !out.contains(&u) {
                        out.push(u);
                    }
                }
            }
        }
    }
    out.sort_unstable();
    out
}

/// Points along the river centreline every `every` ft within `within` of the centre:
/// (point, unit tangent, half width).
pub(super) fn river_samples(site: &Site, every: f64, within: f64) -> Vec<(P, P, f64)> {
    let mut out = Vec::new();
    let mut acc = 0.5 * every;
    for &(a, b, hw) in &site.river {
        let l = dist(a, b);
        if l < 1e-9 {
            continue;
        }
        let t = mul(sub(b, a), 1.0 / l);
        while acc <= l {
            let p = add(a, mul(t, acc));
            if len(p) < within {
                out.push((p, t, hw));
            }
            acc += every;
        }
        acc -= l;
    }
    out
}

pub(super) fn river_pieces(t0: &T0, center: P, r: f64) -> Vec<(P, P, f64)> {
    let mut out = Vec::new();
    for (ri, k) in t0.rivers.segments_near(center[0] - r, center[1] - r, center[0] + r, center[1] + r, 0.0) {
        let rc = &t0.rivers.rivers[ri as usize];
        let len = rc.s[k as usize + 1] - rc.s[k as usize];
        // Fine enough to follow meanders and wiggles to a few feet (banks, quays, piers).
        let n = (len / 20.0).ceil().clamp(2.0, 1200.0) as usize;
        let mut prev: Option<(P, f64)> = None;
        for j in 0..=n {
            let cp = rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft);
            let p = sub(cp.p, center);
            if let Some((q, w)) = prev {
                out.push((q, p, 0.5 * w.max(cp.w)));
            }
            prev = Some((p, cp.w));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
