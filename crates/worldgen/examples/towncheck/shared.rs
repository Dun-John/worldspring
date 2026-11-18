use crate::prelude::*;

// ---- Reports: counts with their worst cases, and measures (a least or greatest value). ----

pub(crate) enum Row {
    Count { n: usize, worst: Vec<(f64, String)> },
    Measure { lowest: bool, v: Option<(f64, String)> },
    /// Every value seen (its least, median and greatest printed).
    Values { v: Vec<(f64, String)> },
}

#[derive(Default)]
pub(crate) struct Report {
    pub(crate) rows: Vec<(String, Row)>,
}

impl Report {
    pub(crate) fn row(&mut self, label: &str, make: impl FnOnce() -> Row) -> &mut Row {
        let i = match self.rows.iter().position(|r| r.0 == label) {
            Some(i) => i,
            None => {
                self.rows.push((label.to_string(), make()));
                self.rows.len() - 1
            }
        };
        &mut self.rows[i].1
    }
    /// Counts listed (as zero until something is added), in this order.
    pub(crate) fn counts(&mut self, labels: &[&str]) {
        for l in labels {
            self.row(l, || Row::Count { n: 0, worst: Vec::new() });
        }
    }
    pub(crate) fn add(&mut self, label: &str, size: f64, what: String) {
        if let Row::Count { n, worst } = self.row(label, || Row::Count { n: 0, worst: Vec::new() }) {
            *n += 1;
            worst.push((size, what));
        }
    }
    /// Adds `k` to a count with nothing to list (how many were looked at).
    pub(crate) fn tally(&mut self, label: &str, k: usize) {
        if let Row::Count { n, .. } = self.row(label, || Row::Count { n: 0, worst: Vec::new() }) {
            *n += k;
        }
    }
    /// A least (`lowest`) or greatest value and where.
    pub(crate) fn measure(&mut self, label: &str, lowest: bool, v: f64, at: String) {
        if let Row::Measure { v: cur, .. } = self.row(label, || Row::Measure { lowest, v: None }) {
            if cur.as_ref().is_none_or(|c| if lowest { v < c.0 } else { v > c.0 }) {
                *cur = Some((v, at));
            }
        }
    }
    /// A value toward a spread (least, median, greatest).
    pub(crate) fn value(&mut self, label: &str, v: f64, at: String) {
        if let Row::Values { v: vals } = self.row(label, || Row::Values { v: Vec::new() }) {
            vals.push((v, at));
        }
    }
    pub(crate) fn print(&mut self) {
        let top = top();
        for (label, row) in &mut self.rows {
            match row {
                Row::Count { n, worst } => {
                    say!("    {label}: {n}");
                    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
                    for (_, w) in worst.iter().take(top) {
                        say!("        {w}");
                    }
                }
                Row::Measure { v: Some((v, at)), .. } => say!("    {label}: {v:.1} ({at})"),
                Row::Measure { v: None, .. } => say!("    {label}: -"),
                Row::Values { v } if v.is_empty() => say!("    {label}: -"),
                Row::Values { v } => {
                    v.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let (lo, mid, hi) = (&v[0], &v[v.len() / 2], &v[v.len() - 1]);
                    say!("    {label}: least {:.1} ({}), median {:.1}, greatest {:.1} ({}), of {}", lo.0, lo.1, mid.0, hi.0, hi.1, v.len());
                    // (A curtain's bow, slice 2c: its 90th percentile too.)
                    if label.contains("curtain bow") {
                        say!("      90th percentile {:.1}", v[(v.len() * 9 / 10).min(v.len() - 1)].0);
                    }
                }
            }
        }
    }
    /// Sums counts (and keeps the best measures) into `all`, the report over every world.
    pub(crate) fn merge_into(&self, all: &mut Report, world: &str) {
        for (label, row) in &self.rows {
            match row {
                Row::Count { n, .. } => all.tally(label, *n),
                Row::Measure { lowest, v: Some((v, at)) } => all.measure(label, *lowest, *v, format!("{world} {at}")),
                Row::Measure { lowest, v: None } => {
                    all.row(label, || Row::Measure { lowest: *lowest, v: None });
                }
                Row::Values { v } => {
                    all.row(label, || Row::Values { v: Vec::new() });
                    for (x, at) in v {
                        all.value(label, *x, format!("{world} {at}"));
                    }
                }
            }
        }
    }
}

// ---- Geometry. ----

/// Distance from `p` to a closed polygon's edges.
pub(crate) fn edge_dist(poly: &[P], p: P) -> f64 {
    let n = poly.len();
    (0..n).map(|k| seg_dist(p, poly[k], poly[(k + 1) % n])).fold(f64::MAX, f64::min)
}

/// How far inside a polygon `p` lies (negative outside).
pub(crate) fn depth(poly: &[P], p: P) -> f64 {
    let d = edge_dist(poly, p);
    if contains(poly, p) { d } else { -d }
}

/// Points every ~`step` ft along a polyline, each with the way its piece runs.
pub(crate) fn samples(w: &[P], step: f64) -> Vec<(P, P)> {
    let mut v = Vec::new();
    for s in w.windows(2) {
        let l = dist(s[0], s[1]);
        if l < 1e-6 {
            continue;
        }
        let u = mul(sub(s[1], s[0]), 1.0 / l);
        let n = (l / step).ceil().max(1.0) as usize;
        v.extend((0..=n).map(|k| (lerp(s[0], s[1], k as f64 / n as f64), u)));
    }
    v
}

pub(crate) fn segx(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (o3, o4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

pub(crate) fn seg_seg(a: P, b: P, c: P, d: P) -> f64 {
    if segx(a, b, c, d) {
        return 0.0;
    }
    seg_dist(a, c, d).min(seg_dist(b, c, d)).min(seg_dist(c, a, b)).min(seg_dist(d, a, b))
}

/// Distance from a segment to a closed polygon's edges.
pub(crate) fn seg_poly(a: P, b: P, poly: &[P]) -> f64 {
    let n = poly.len();
    (0..n).map(|k| seg_seg(a, b, poly[k], poly[(k + 1) % n])).fold(f64::MAX, f64::min)
}

pub(crate) fn poly_overlap(a: &[P], b: &[P]) -> bool {
    let (n, m) = (a.len(), b.len());
    a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p)) || (0..n).any(|i| (0..m).any(|j| segx(a[i], a[(i + 1) % n], b[j], b[(j + 1) % m])))
}

/// A footprint shrunk by `d` toward its centroid (cheap; for convex-ish footprints).
pub(crate) fn shrink(a: &[P], d: f64) -> Vec<P> {
    let c = centroid(a);
    a.iter().map(|p| add(c, mul(sub(*p, c), ((len(sub(*p, c)) - d) / len(sub(*p, c)).max(1e-9)).max(0.0)))).collect()
}

pub(crate) fn bbox(pts: &[P]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])])
}

pub(crate) fn at(p: P) -> String {
    format!("({:.0}, {:.0})", p[0], p[1])
}

// ---- A world: its executor, battlemap chunks kept as made. ----

pub(crate) struct Ctx {
    pub(crate) ex: Executor,
    pub(crate) chunks: HashMap<(u32, u32), Rc<Chunk>>,
}

impl Ctx {
    pub(crate) fn chunk(&mut self, kx: u32, ky: u32) -> Rc<Chunk> {
        if let Some(c) = self.chunks.get(&(kx, ky)) {
            return c.clone();
        }
        let c = Rc::new(self.ex.battlemap(TileKey::surface(self.ex.world.geom.max_level, kx, ky)));
        self.chunks.insert((kx, ky), c.clone());
        c
    }

    /// The battlemap squares of the chunks round a rectangle (world ft), with what blocks
    /// movement as play mode has it: buildings, walls and towers, and blocking objects (a hedge
    /// every square its piece is drawn across, anything else the squares within half its
    /// footprint), each only in its own chunk.
    pub(crate) fn window(&mut self, r: [f64; 4]) -> Win {
        let size = self.ex.world.geom.tile_size_ft(self.ex.world.geom.max_level);
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let (kx0, ky0, kx1, ky1) = (k(r[0]), k(r[1]), k(r[2]), k(r[3]));
        let (cw, ch) = ((kx1 - kx0 + 1) as usize, (ky1 - ky0 + 1) as usize);
        let (nx, ny) = (cw * SQ, ch * SQ);
        let mut w = Win { ox: kx0 as f64 * size, oy: ky0 as f64 * size, nx, ny, block: vec![false; nx * ny], street: vec![false; nx * ny], objects: Vec::new(), gravel: vec![false; nx * ny], road: vec![false; nx * ny], runs: Vec::new() };
        for cj in 0..ch {
            for ci in 0..cw {
                let c = self.chunk(kx0 + ci as u32, ky0 + cj as u32);
                let idx = |i: usize, j: usize| (cj * SQ + j) * nx + ci * SQ + i;
                for q in 0..SQ * SQ {
                    let a = idx(q % SQ, q / SQ);
                    w.block[a] |= c.building[q] != 0 || matches!(c.urban[q], URBAN_WALL | URBAN_TOWER);
                    w.street[a] = c.urban[q] == URBAN_STREET;
                    w.gravel[a] = c.surface[q] == Surface::Gravel;
                    w.road[a] = c.road_halo[(q / SQ + 1) * (SQ + 2) + q % SQ + 1] != 0 || c.urban[q] == URBAN_STREET;
                }
                // (R5 towns: dry-stone walls and hedges, world ft.)
                let (cx, cy) = (w.ox + (ci * SQ) as f64 * SQUARE_FT, w.oy + (cj * SQ) as f64 * SQUARE_FT);
                for s in c.structures.iter().filter(|s| matches!(s.kind, worldgen::battlemap::ShapeKind::DryWall | worldgen::battlemap::ShapeKind::Hedge) && s.pts.len() >= 2) {
                    let f = |q: [f32; 2]| [cx + q[0] as f64 * SQUARE_FT, cy + q[1] as f64 * SQUARE_FT];
                    w.runs.push((f(s.pts[0]), f(s.pts[1])));
                }
                let mut set = |i: i64, j: i64| {
                    if i >= 0 && j >= 0 && i < SQ as i64 && j < SQ as i64 {
                        w.block[idx(i as usize, j as usize)] = true;
                    }
                };
                for o in &c.objects {
                    let inf = c.info(o);
                    let (x, y) = (o.x as f64, o.y as f64);
                    let p = [w.ox + (ci * SQ) as f64 * SQUARE_FT + x * SQUARE_FT, w.oy + (cj * SQ) as f64 * SQUARE_FT + y * SQUARE_FT];
                    w.objects.push((p, c.name(o).to_string()));
                    if !inf.blocks_move {
                        continue;
                    }
                    if o.sprite == 0 && o.kind == Kind::Hedge {
                        let (cs, sn) = (libm::cos(o.rot as f64), libm::sin(o.rot as f64));
                        let (hl, hw) = (0.49 * o.scale as f64, 0.29 * o.scale as f64);
                        let mut u = -hl;
                        while u <= hl + 1e-6 {
                            let mut v = -hw;
                            while v <= hw + 1e-6 {
                                set((x + cs * u - sn * v).floor() as i64, (y + sn * u + cs * v).floor() as i64);
                                v += 0.1f64.min(hw);
                            }
                            u += 0.1f64.min(hl);
                        }
                        continue;
                    }
                    let reach = (inf.radius as f64 * o.scale as f64 * 0.5).max(0.5);
                    for j in (y - reach).floor() as i64..=(y + reach).floor() as i64 {
                        for i in (x - reach).floor() as i64..=(x + reach).floor() as i64 {
                            let (dx, dy) = (i as f64 + 0.5 - x, j as f64 + 0.5 - y);
                            if dx * dx + dy * dy <= reach * reach {
                                set(i, j);
                            }
                        }
                    }
                }
            }
        }
        w
    }
}

pub(crate) struct Win {
    pub(crate) ox: f64,
    pub(crate) oy: f64,
    pub(crate) nx: usize,
    pub(crate) ny: usize,
    pub(crate) block: Vec<bool>,
    pub(crate) street: Vec<bool>,
    /// Every object (world ft) by name.
    pub(crate) objects: Vec<(P, String)>,
    /// (R5 towns) gravel squares, road or street squares, dry-stone walls' and hedges' runs (world ft).
    pub(crate) gravel: Vec<bool>,
    pub(crate) road: Vec<bool>,
    pub(crate) runs: Vec<(P, P)>,
}

impl Win {
    pub(crate) fn centre(&self, k: usize) -> P {
        [self.ox + ((k % self.nx) as f64 + 0.5) * SQUARE_FT, self.oy + ((k / self.nx) as f64 + 0.5) * SQUARE_FT]
    }
    pub(crate) fn index(&self, p: P) -> Option<usize> {
        let (i, j) = (((p[0] - self.ox) / SQUARE_FT).floor(), ((p[1] - self.oy) / SQUARE_FT).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.nx && (j as usize) < self.ny).then(|| j as usize * self.nx + i as usize)
    }
    /// Open squares reached, square to square, from the open squares `from` picks.
    pub(crate) fn flood(&self, from: impl Fn(usize) -> bool) -> Vec<bool> {
        let mut seen = vec![false; self.nx * self.ny];
        let mut q = VecDeque::new();
        for k in (0..self.nx * self.ny).filter(|&k| !self.block[k] && from(k)) {
            seen[k] = true;
            q.push_back(k);
        }
        while let Some(k) = q.pop_front() {
            for n in self.around(k) {
                if !seen[n] && !self.block[n] {
                    seen[n] = true;
                    q.push_back(n);
                }
            }
        }
        seen
    }
    pub(crate) fn around(&self, k: usize) -> impl Iterator<Item = usize> {
        let (x, y, nx, ny) = (k % self.nx, k / self.nx, self.nx, self.ny);
        [(x > 0).then(|| k - 1), (x + 1 < nx).then(|| k + 1), (y > 0).then(|| k - nx), (y + 1 < ny).then(|| k + nx)].into_iter().flatten()
    }
    pub(crate) fn edge(&self, k: usize) -> bool {
        let (x, y) = (k % self.nx, k / self.nx);
        x == 0 || y == 0 || x == self.nx - 1 || y == self.ny - 1
    }
    /// The sizes of the pockets the squares `sealed` picks make (largest first) and where each starts.
    pub(crate) fn pockets(&self, sealed: &[bool]) -> Vec<(usize, P)> {
        let mut seen = vec![false; sealed.len()];
        let mut out = Vec::new();
        for s in 0..sealed.len() {
            if !sealed[s] || seen[s] {
                continue;
            }
            seen[s] = true;
            let (mut stack, mut n) = (vec![s], 0);
            while let Some(k) = stack.pop() {
                n += 1;
                for m in self.around(k) {
                    if sealed[m] && !seen[m] {
                        seen[m] = true;
                        stack.push(m);
                    }
                }
            }
            out.push((n, self.centre(s)));
        }
        out.sort_by(|a, b| b.0.cmp(&a.0));
        out
    }
}

// ---- Castles. ----

/// A castle's bailey and its walls as the checks see them.
pub(crate) struct Castle<'a> {
    pub(crate) ring: Vec<P>,
    /// Walls on the ring (the outer curtain), wholly inside it (an inner curtain), and the rest (the town's).
    pub(crate) outer: Vec<&'a [P]>,
    pub(crate) inner: Vec<&'a [P]>,
    pub(crate) town: Vec<&'a [P]>,
    /// The castle's gate towers (a town's own castle's in `gate_towers`, one set in the street editor's in `extra_towers`).
    pub(crate) gate_towers: Vec<P>,
    /// Gate middles: pairs of its gate towers 52 ft apart.
    pub(crate) gates: Vec<P>,
    /// The gate nearest the ring.
    pub(crate) outer_gate: Option<P>,
}

impl<'a> Castle<'a> {
    pub(crate) fn of(l: &'a Layout) -> Option<Castle<'a>> {
        let ring = l.plazas.iter().find(|p| p.kind == PlazaKind::Bailey)?.pts.clone();
        let (mut outer, mut inner, mut town) = (Vec::new(), Vec::new(), Vec::new());
        for w in &l.walls {
            let s = samples(w, 5.0);
            let on = s.iter().filter(|x| edge_dist(&ring, x.0) < 1.5).count();
            if w.iter().all(|p| depth(&ring, *p) > 20.0) {
                inner.push(w.as_slice());
            } else if on * 10 >= s.len() * 8 {
                outer.push(w.as_slice());
            } else {
                town.push(w.as_slice());
            }
        }
        let gate_towers: Vec<P> = l.gate_towers.iter().copied().chain(l.extra_towers.iter().filter(|t| t.1).map(|t| t.0)).filter(|p| depth(&ring, *p) > -20.0).collect();
        let mut gates = Vec::new();
        for a in 0..gate_towers.len() {
            for b in a + 1..gate_towers.len() {
                if (dist(gate_towers[a], gate_towers[b]) - 52.0).abs() < 0.5 {
                    gates.push(lerp(gate_towers[a], gate_towers[b], 0.5));
                }
            }
        }
        let outer_gate = gates.iter().copied().min_by(|a, b| depth(&ring, *a).abs().total_cmp(&depth(&ring, *b).abs()));
        Some(Castle { ring, outer, inner, town, gate_towers, gates, outer_gate })
    }
    /// The castle's own way from its gate to a street (class 3, 10 ft wide, from a gate).
    pub(crate) fn is_way(&self, r: &(Vec<P>, u8, f64)) -> bool {
        r.1 == 3 && (r.2 - 10.0).abs() < 1e-9 && self.gates.iter().any(|m| dist(r.0[0], *m) < 12.0)
    }
    pub(crate) fn curtains(&self) -> impl Iterator<Item = &&'a [P]> {
        self.outer.iter().chain(self.inner.iter())
    }
}

/// Whether a building is a keep (of pieces, not a temple's: or the castle's catalog entry).
pub(crate) fn is_keep(b: &town::Building) -> bool {
    (!b.parts.is_empty() && !town::temple::shaped(b)) || (b.func.is_some() && b.func.map(|f| f as usize) == town::catalog::index_of("castle"))
}

pub(crate) const CASTLE_COUNTS: [&str; 24] = [
    "castles with no bailey",
    "castles with no outer gate (no pair of gate towers)",
    "castles with no keep",
    "capitals with no inner curtain",
    "town gates opening into a bailey",
    "town gates by a castle with fewer than 2 towers",
    "main streets inside a bailey",
    "roads inside a bailey (its own way excepted)",
    "streets or roads crossing a curtain",
    "baileys open on dry ground (a run of more than 15 ft)",
    "castles with a curtain 3-30 ft from the town wall along 30 ft+",
    "castles with a 30-45 ft strip by the town wall",
    "castles with a 30-45 ft strip by the town wall and no street in it",
    "castles with towers within 2 ft of each other",
    "castles with a castle tower within 12 ft of a town tower",
    "castle buildings on towers",
    "castle buildings within 4 ft of a wall",
    "castle buildings overlapping a building",
    "castle buildings on a street or road",
    "town buildings in a bailey",
    "gate ways through or within 5 ft of buildings",
    "gate ways across walls, towers or water",
    "outer gates over 20 ft from a street or way",
    "outer baileys under 48 ft wide, or inner curtains under 80 ft from the outer gate",
];
pub(crate) const CASTLE_MORE: [&str; 5] = [
    "bailey spikes (corners under 35 deg, sides 40 ft+)",
    "keep pieces of 1,100 sq ft+ with a flat top left open",
    "duplicate building ids",
    "motte paths through a building or wall",
    "objects on a motte above its toe",
];

/// The castle checks on one layout with a castle (`tag` names it).
pub(crate) fn castle_checks(world: &World, t0: &T0, l: &Layout, tag: &str, capital: bool, r: &mut Report) {
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    r.tally("castles", 1);
    let Some(c) = Castle::of(l) else {
        r.add("castles with no bailey", 1.0, format!("{tag}: no bailey"));
        return;
    };
    let ring = &c.ring;
    let inner = l.castles.len() > 1;
    r.tally("castles with an inner curtain", inner as usize);
    if capital && !inner {
        r.add("capitals with no inner curtain", area(ring).abs(), format!("{tag}: a capital's castle with no inner curtain, bailey {:.0} sq ft", area(ring).abs()));
    }
    if !l.buildings.iter().any(|b| b.ward == Ward::Castle && is_keep(b)) {
        r.add("castles with no keep", area(ring).abs(), format!("{tag}: no keep in a bailey of {:.0} sq ft at {}", area(ring).abs(), at(centroid(ring))));
    }
    let towers = interior::towers(l);
    // How far a tower reaches from its middle (a square one to its corners).
    let reach = |t: &(P, f64, bool)| if !t.2 && l.square_tower(t.0).is_some() { t.1 * std::f64::consts::SQRT_2 } else { t.1 };
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let Some(om) = c.outer_gate else {
        r.add("castles with no outer gate (no pair of gate towers)", 1.0, format!("{tag}: no outer gate, bailey at {}", at(centroid(ring))));
        return;
    };
    // The outer bailey's width and the inner curtain's distance to the outer gate.
    if inner {
        let s: Vec<P> = c.inner.iter().flat_map(|w| samples(w, 2.5)).map(|x| x.0).collect();
        let ring_gap = s.iter().map(|p| depth(ring, *p)).fold(f64::MAX, f64::min);
        // (Also against the outer curtain as laid, and the town wall where it is the castle's.)
        let others: Vec<(P, P)> = l.walls.iter().filter(|w| !c.inner.iter().any(|x| std::ptr::eq(*x, w.as_slice()))).flat_map(|w| w.windows(2).map(|q| (q[0], q[1]))).collect();
        let wall_gap = s.iter().map(|p| others.iter().map(|q| seg_dist(*p, q.0, q.1)).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
        let gate_gap = s.iter().map(|p| dist(*p, om)).fold(f64::MAX, f64::min);
        r.measure("least outer bailey width (to its ring)", true, ring_gap, tag.into());
        r.measure("least outer bailey width (to any other wall)", true, wall_gap, tag.into());
        r.measure("least inner curtain distance to the outer gate", true, gate_gap, tag.into());
        if ring_gap < 48.0 || wall_gap < 46.0 || gate_gap < 80.0 {
            r.add(
                "outer baileys under 48 ft wide, or inner curtains under 80 ft from the outer gate",
                80.0 - gate_gap.min(ring_gap),
                format!("{tag}: outer bailey {ring_gap:.1} ft wide ({wall_gap:.1} to a wall), inner curtain {gate_gap:.1} ft from the outer gate at {}", at(om)),
            );
        }
    }
    // The outer gate to the nearest street or road (its own way included).
    let to_street = l.roads.iter().map(|x| &x.0).chain(l.streets.iter()).flat_map(|s| s.windows(2).map(|w| seg_dist(om, w[0], w[1]))).fold(f64::MAX, f64::min);
    r.measure("greatest outer gate distance to a street or way", false, to_street, tag.into());
    if to_street > 20.0 {
        r.add("outer gates over 20 ft from a street or way", to_street, format!("{tag}: outer gate at {} {to_street:.1} ft from any street", at(om)));
    }
    // Gate ways: clear of buildings, walls, towers and water.
    for way in l.roads.iter().filter(|x| c.is_way(x)) {
        r.tally("gate ways", 1);
        let (mut near, mut other): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for s in way.0.windows(2) {
            let (a, b) = (s[0], s[1]);
            for bd in &l.buildings {
                let d = seg_poly(a, b, &bd.poly);
                if contains(&bd.poly, a) || contains(&bd.poly, b) || d < 5.0 {
                    near.push(format!("b:{}:{} ({:?}) {d:.1} ft", l.index, bd.id, bd.ward));
                }
            }
            if walls.iter().any(|q| segx(a, b, q.0, q.1)) {
                other.push("crosses a wall".into());
            }
            for t in &towers {
                // (Its own gate towers stand either side of its start.)
                if seg_dist(t.0, a, b) < reach(t) + 3.0 && !(t.2 && dist(t.0, way.0[0]) < 27.0) {
                    other.push(format!("tower at {} {:.1} ft off", at(t.0), seg_dist(t.0, a, b)));
                }
            }
            let n = (0..=10).filter(|k| wet(lerp(a, b, *k as f64 / 10.0))).count();
            if n > 0 {
                other.push(format!("{n}/11 points wet"));
            }
        }
        near.dedup();
        let length: f64 = way.0.windows(2).map(|s| dist(s[0], s[1])).sum();
        if !near.is_empty() {
            r.add("gate ways through or within 5 ft of buildings", near.len() as f64, format!("{tag}: the {length:.0}-ft way from {} by {}", at(way.0[0]), near.join(", ")));
        }
        if !other.is_empty() {
            r.add("gate ways across walls, towers or water", other.len() as f64, format!("{tag}: the {length:.0}-ft way from {}: {}", at(way.0[0]), other.join("; ")));
        }
    }
    // Buildings: ids, town buildings in the bailey, castle buildings on walls, towers, buildings, streets.
    let mut ids: Vec<u32> = l.buildings.iter().map(|b| b.id).collect();
    ids.sort();
    let n_ids = ids.len();
    ids.dedup();
    if ids.len() < n_ids {
        r.add("duplicate building ids", (n_ids - ids.len()) as f64, format!("{tag}: {} ids twice", n_ids - ids.len()));
    }
    for b in l.buildings.iter().filter(|b| b.ward != Ward::Castle && contains(ring, centroid(&b.poly))) {
        r.add("town buildings in a bailey", 1.0, format!("{tag}: b:{}:{} ({:?}) at {}", l.index, b.id, b.ward, at(centroid(&b.poly))));
    }
    let ways: Vec<(&Vec<P>, f64, String)> =
        l.roads.iter().filter(|x| !c.is_way(x)).map(|x| (&x.0, x.2, format!("class {} road", x.1))).chain(l.streets.iter().map(|s| (s, 18.0, "main street".to_string()))).collect();
    for (bi, b) in l.buildings.iter().enumerate().filter(|(_, b)| b.ward == Ward::Castle) {
        let name = format!("b:{}:{}", l.index, b.id);
        // (A keep set into the curtain: the curtain stops at its walls.)
        let ends_on = |s: &(P, P)| b.keep.is_some() && [s.0, s.1].iter().any(|p| seg_poly(*p, *p, &b.poly) < 0.5);
        let wd = walls.iter().filter(|s| !ends_on(s)).map(|s| if contains(&b.poly, s.0) { -1.0 } else { seg_poly(s.0, s.1, &b.poly) }).fold(f64::MAX, f64::min);
        r.measure("least castle building distance to a wall", true, wd, format!("{tag} {name}"));
        if wd < 4.0 {
            r.add("castle buildings within 4 ft of a wall", 4.0 - wd, format!("{tag}: {name} {wd:.1} ft from a wall at {}", at(centroid(&b.poly))));
        }
        for t in &towers {
            let hit = match l.square_tower(t.0).filter(|_| !t.2) {
                Some(u) => {
                    let v = [-u[1], u[0]];
                    let sq: Vec<P> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|(x, y)| add(t.0, add(mul(u, x * t.1), mul(v, y * t.1)))).collect();
                    poly_overlap(&sq, &b.poly)
                }
                None => -depth(&b.poly, t.0) < t.1,
            };
            if hit {
                r.add("castle buildings on towers", t.1 + depth(&b.poly, t.0), format!("{tag}: {name} on the tower at {}", at(t.0)));
            }
        }
        // (Each pair once; castle buildings against each other too.)
        for (_, o) in l.buildings.iter().enumerate().filter(|(oi, o)| *oi != bi && (o.ward != Ward::Castle || *oi > bi)) {
            if poly_overlap(&shrink(&b.poly, 0.5), &shrink(&o.poly, 0.5)) {
                r.add("castle buildings overlapping a building", 1.0, format!("{tag}: {name} overlaps b:{}:{} ({:?}) at {}", l.index, o.id, o.ward, at(centroid(&b.poly))));
            }
        }
        for (pts, w, what) in &ways {
            let d = pts.windows(2).map(|s| if contains(&b.poly, s[0]) { 0.0 } else { seg_poly(s[0], s[1], &b.poly) }).fold(f64::MAX, f64::min);
            if d < 0.5 * w - 1.0 {
                r.add("castle buildings on a street or road", 0.5 * w - d, format!("{tag}: {name} under a {what} ({d:.1} ft from its line) at {}", at(centroid(&b.poly))));
            }
        }
    }
    // Curtains alongside the town wall: 3-30 ft from it, or 30-45 ft (a strip), along 30 ft or more.
    let tsegs: Vec<(P, P)> = c.town.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let (mut narrow, mut broad, mut bare): (Option<String>, Option<String>, Option<String>) = (None, None, None);
    for w in c.curtains() {
        let (mut run_n, mut run_b) = (0, 0);
        for (p, u) in samples(w, 5.0) {
            // The nearest town wall running the same way (within 15 deg), beside the point.
            let near = tsegs
                .iter()
                .filter_map(|&(a, b)| {
                    let tl = dist(a, b);
                    let t = dot(sub(p, a), sub(b, a)) / (tl * tl);
                    (tl > 1.0 && cross(u, mul(sub(b, a), 1.0 / tl)).abs() < 0.26 && t > 0.0 && t < 1.0).then(|| (seg_dist(p, a, b), lerp(a, b, t)))
                })
                .min_by(|x, y| x.0.total_cmp(&y.0));
            let d = near.map_or(f64::MAX, |x| x.0);
            run_n = if d > 3.0 && d <= 30.0 { run_n + 1 } else { 0 };
            run_b = if d > 30.0 && d < 45.0 { run_b + 1 } else { 0 };
            if run_n == 7 && narrow.is_none() {
                narrow = Some(format!("{tag}: curtain {d:.0} ft from the town wall along 30 ft+ near {}", at(p)));
            }
            if run_b == 7 {
                let mid = lerp(p, near.unwrap().1, 0.5);
                let street =
                    l.roads.iter().map(|x| (&x.0, x.2)).chain(l.streets.iter().map(|s| (s, 18.0))).any(|(s, w)| s.windows(2).any(|q| seg_dist(mid, q[0], q[1]) < 0.5 * w + 4.0));
                broad.get_or_insert_with(|| format!("{tag}: a {d:.0}-ft strip between curtain and town wall near {}", at(p)));
                if !street {
                    bare.get_or_insert_with(|| format!("{tag}: a {d:.0}-ft strip with no street between curtain and town wall near {}", at(p)));
                }
            }
        }
    }
    for (label, what) in [
        ("castles with a curtain 3-30 ft from the town wall along 30 ft+", narrow),
        ("castles with a 30-45 ft strip by the town wall", broad),
        ("castles with a 30-45 ft strip by the town wall and no street in it", bare),
    ] {
        if let Some(w) = what {
            r.add(label, 1.0, w);
        }
    }
    // The ring closed all round (a wall within 10 ft, a tower or a gate) but on or by water.
    let mut closed = ring.clone();
    closed.push(ring[0]);
    let (mut open, mut run, mut longest, mut first) = (0.0, 0.0f64, 0.0f64, None);
    for (p, _) in samples(&closed, 5.0) {
        let wd = walls.iter().map(|&(a, b)| seg_dist(p, a, b)).fold(f64::MAX, f64::min);
        let shut = wd <= 10.0 || towers.iter().any(|t| dist(t.0, p) < t.1 + 6.0) || c.gates.iter().any(|m| dist(*m, p) < 30.0);
        let by_water = || {
            wet(p)
                || (0..8).any(|k| {
                    let a = k as f64 * 0.785;
                    wet(add(p, [12.0 * libm::cos(a), 12.0 * libm::sin(a)]))
                })
        };
        if shut || by_water() {
            run = 0.0;
        } else {
            open += 5.0;
            run += 5.0;
            longest = longest.max(run);
            first.get_or_insert(p);
        }
    }
    r.measure("greatest dry open length of a bailey's ring", false, open, tag.into());
    if longest > 15.0 {
        r.add("baileys open on dry ground (a run of more than 15 ft)", open, format!("{tag}: open {open:.0} ft on dry ground (longest {longest:.0}), from {}", at(first.unwrap())));
    }
    // Towers on or beside each other near the castle: castle (C: on a curtain, or a castle gate's)
    // against town (T: on the town wall).
    let on = |ws: &[&[P]], p: P| ws.iter().any(|w| w.windows(2).any(|s| seg_dist(p, s[0], s[1]) < 2.0));
    let label = |t: &(P, f64, bool)| {
        let gate = c.gate_towers.iter().any(|q| dist(*q, t.0) < 1e-6);
        let castle = gate || on(&c.outer, t.0) || on(&c.inner, t.0);
        let town = on(&c.town, t.0) && !gate;
        match (castle, town) {
            (true, false) => "C",
            (false, true) => "T",
            (true, true) => "CT",
            _ => "?",
        }
    };
    let near_castle: Vec<&(P, f64, bool)> = towers.iter().filter(|t| depth(ring, t.0) > -60.0).collect();
    let (mut beside, mut crowded): (Option<(f64, String)>, Option<(f64, String)>) = (None, None);
    for (a, ta) in near_castle.iter().enumerate() {
        for tb in &near_castle[a + 1..] {
            let (la, lb) = (label(ta), label(tb));
            let d = dist(ta.0, tb.0);
            if (la == "T" && lb == "T") || (ta.2 && tb.2 && (d - 52.0).abs() < 0.5) {
                continue;
            }
            let clear = d - reach(ta) - reach(tb);
            let what = format!("{tag}: towers at {} ({la}) and {} ({lb}) {clear:.1} ft clear", at(ta.0), at(tb.0));
            if (la == "T") != (lb == "T") && la != "?" && lb != "?" {
                r.measure("least clearance, castle tower to town tower", true, clear, tag.into());
                if clear < 12.0 && beside.as_ref().is_none_or(|b| clear < b.0) {
                    beside = Some((clear, what));
                }
            } else {
                r.measure("least clearance between castle towers", true, clear, tag.into());
                if clear < 2.0 && crowded.as_ref().is_none_or(|b| clear < b.0) {
                    crowded = Some((clear, what));
                }
            }
        }
    }
    if let Some((clear, w)) = beside {
        r.add("castles with a castle tower within 12 ft of a town tower", 12.0 - clear, w);
    }
    if let Some((clear, w)) = crowded {
        r.add("castles with towers within 2 ft of each other", 2.0 - clear, w);
    }
    // Town gates by the castle: their towers (where the town has its wall), and whether they open
    // straight into the bailey.
    for g in l.gates.iter().filter(|g| depth(ring, **g) > -60.0) {
        let n = l.gate_towers.iter().filter(|t| dist(**t, *g) < 30.0).count();
        if n < 2 && !c.town.is_empty() {
            r.add("town gates by a castle with fewer than 2 towers", 1.0, format!("{tag}: the town gate at {} has {n} towers", at(*g)));
        }
        let cd = c.curtains().flat_map(|w| w.windows(2)).map(|q| seg_dist(*g, q[0], q[1])).fold(f64::MAX, f64::min);
        r.measure("least town gate distance to a curtain", true, cd, tag.into());
        let into = (0..36).flat_map(|k| [15.0, 25.0, 35.0, 45.0].map(|rr| (k, rr))).map(|(k, rr)| {
            let a = k as f64 * std::f64::consts::TAU / 36.0;
            add(*g, [rr * libm::cos(a), rr * libm::sin(a)])
        });
        let into = into.into_iter().find(|p| {
            depth(ring, *p) > 4.0 && !walls.iter().any(|s| segx(*g, *p, s.0, s.1) || seg_dist(*p, s.0, s.1) < 4.5) && !towers.iter().any(|t| seg_dist(t.0, *g, *p) < t.1)
        });
        if let Some(p) = into {
            r.add("town gates opening into a bailey", 1.0, format!("{tag}: the town gate at {} opens into the bailey (to {})", at(*g), at(p)));
        }
    }
    // Streets and roads inside the bailey, or across its curtains.
    let csegs: Vec<(P, P)> = c.curtains().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    for (pts, _, what) in &ways {
        let inside = samples(pts, 5.0).iter().filter(|x| depth(ring, x.0) > 3.0).count() as f64 * 5.0;
        if inside > 15.0 {
            let label = if what == "main street" { "main streets inside a bailey" } else { "roads inside a bailey (its own way excepted)" };
            r.add(label, inside, format!("{tag}: a {what} runs {inside:.0} ft inside the bailey, from {}", at(pts[0])));
        }
        for s in pts.windows(2) {
            if let Some(q) = csegs.iter().find(|q| segx(s[0], s[1], q.0, q.1)) {
                r.add("streets or roads crossing a curtain", 1.0, format!("{tag}: a {what} crosses a curtain near {}", at(q.0)));
            }
        }
    }
    // Spikes: a ring corner sharper than 35 deg between sides of 40 ft or more.
    let ccw = area(ring) > 0.0;
    let rn = ring.len();
    for k in 0..rn {
        let (pv, cn, nx) = (ring[(k + rn - 1) % rn], ring[k], ring[(k + 1) % rn]);
        let (a, b) = (sub(pv, cn), sub(nx, cn));
        let (la, lb) = (len(a), len(b));
        if la < 40.0 || lb < 40.0 {
            continue;
        }
        let deg = libm::acos((dot(a, b) / (la * lb)).clamp(-1.0, 1.0)).to_degrees();
        if (cross(sub(cn, pv), sub(nx, cn)) > 0.0) == ccw && deg < 35.0 {
            r.add("bailey spikes (corners under 35 deg, sides 40 ft+)", 35.0 - deg, format!("{tag}: a {deg:.0}-deg corner at {}, sides {la:.0} and {lb:.0} ft", at(cn)));
        }
    }
    // Keep tops: a flat top of 1,100 sq ft or more is roofed.
    for b in l.buildings.iter().filter(|b| b.ward == Ward::Castle && !b.parts.is_empty()) {
        for (p, &top) in b.parts.iter().zip(&town::keep_tops(b, world.stream("keep.tops"))) {
            let flat = match p.kind {
                town::PartKind::Body if p.poly.len() == 4 => {
                    let (x, y) = (dist(p.poly[0], p.poly[1]), dist(p.poly[1], p.poly[2]));
                    let (long, wide) = (x.max(y), x.min(y));
                    (long > 1.35 * wide).then(|| wide * wide.min(0.42 * long))
                }
                town::PartKind::Forebuilding | town::PartKind::Wing => Some(area(&p.poly).abs()),
                _ => None,
            };
            if let Some(f) = flat.filter(|f| *f >= 1100.0 && top & 3 == 0) {
                r.add("keep pieces of 1,100 sq ft+ with a flat top left open", f, format!("{tag}: b:{}:{} a {:?} of {f:.0} sq ft", l.index, b.id, p.kind));
            }
        }
    }
    // Mottes: the path up clear of buildings and walls.
    for m in &l.mottes {
        let (a, b) = (add(m.at, mul(m.way, m.top_r - 8.0)), add(m.at, mul(m.way, m.foot_r + 6.0)));
        let hit =
            l.buildings.iter().filter(|bd| bd.parts.is_empty()).any(|bd| contains(&bd.poly, b) || seg_poly(a, b, &bd.poly) == 0.0) || walls.iter().any(|s| segx(a, b, s.0, s.1));
        if hit {
            r.add("motte paths through a building or wall", 1.0, format!("{tag}: the path up the motte at {}", at(m.at)));
        }
    }
}

/// Objects on a motte above its toe (the last 8 ft of its foot), from the battlemap.
pub(crate) fn motte_objects(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
    for m in &l.mottes {
        let w = ctx.window([m.at[0] - m.foot_r, m.at[1] - m.foot_r, m.at[0] + m.foot_r, m.at[1] + m.foot_r]);
        r.tally("mottes", 1);
        let (mut above, mut toe) = (Vec::new(), 0);
        for (p, name) in &w.objects {
            let d = dist(*p, m.at);
            if d < m.foot_r - 8.0 {
                above.push(format!("{name} at {}", at(*p)));
            } else if d < m.foot_r {
                toe += 1;
            }
        }
        r.tally("objects on a motte's toe (its last 8 ft)", toe);
        if !above.is_empty() {
            r.add("objects on a motte above its toe", above.len() as f64, format!("{tag}: the motte at {}: {}", at(m.at), above.join(", ")));
        }
    }
}
