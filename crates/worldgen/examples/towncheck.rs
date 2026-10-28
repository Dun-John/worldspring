//! QA: castles, play reach, interiors and the street editor, swept over whole worlds. Counts what
//! should never happen, with the worst places (world ft) to go and look (`TOP=n` lists more):
//! - `castles`: every town's castle as generated. Town gates opening into a bailey; streets and
//!   roads inside a bailey (the castle's own way to its gate excepted) or across a curtain; dry
//!   stretches of a bailey's ring with no wall, tower or gate on them; curtains doubling the town
//!   wall (3 to 30 ft from it along 30 ft or more) and 30 to 45 ft strips between them with no
//!   street in them; towers overlapping, castle towers within 12 ft of town towers, castle
//!   buildings on towers, walls, streets or other buildings; gate ways blocked or laid through
//!   buildings; the outer gate's distance to the nearest street or way; the outer bailey's width
//!   and the inner curtain's distance to the outer gate; castles with no keep; mottes with
//!   objects on them (from the battlemap); and a few more (spikes, keep tops left open, ids twice).
//!   Every town's quays with no water beyond them, and walled towns' rings with no wall on dry ground.
//! - `reach`: each castle's battlemap round it, flooded as play mode blocks movement (buildings,
//!   walls, towers, blocking objects, a hedge every square it is drawn across) from the window's
//!   edge and its streets, so from outside the outer gate: castle buildings cut off, outer gates
//!   not reached, bailey squares sealed (and the largest pocket); the same for every park.
//! - `interiors`: vital's `check_interior` over every enterable building and wall or gate tower,
//!   failures grouped by function; and the corridor rule on its own (no room more than two rooms
//!   from a corridor), with the stricter old reading (a hall or corridor only, not the front room).
//!   `--keeps`: keeps only.
//! - `hub`: castle keeps and barracks of 150 squares or more, laid out by zones: floors the zones
//!   fell back on, the hall's floor, the front door's room, the main stair at the dais end, the great
//!   chamber and solar by it (a solar's middle nearer the stair than the hall's low end), the
//!   minstrels' gallery reached along passages, windows and furniture on open tops, thrones in mess
//!   halls, room sizes and proportions (the box and how much of its floor a room fills), storeys
//!   alike, tower rooms, landings, the screens passage, the service and officers' ends, rooms a square
//!   wide, a shell keep's round yard (`hub_rules`).
//! - `halls`: great halls furnished as wholes (`interior::hall_plans`: castles', town and guild
//!   halls', noble estates', mess halls): the dais at the far end on the axis (a throne dais in a
//!   castle, a low dais with a chair of office elsewhere, none in a mess hall), the runner on the
//!   axis, the tables, benches, pillars, banners, tapestries, braziers and high tables mirrored
//!   across it, nothing shut off, an open hall's musicians on its gallery; no throne outside a
//!   castle or a palace; every banner in a castle's buildings of one house (`hall_rules`).
//! - `palaces`: palaces and noble estates laid out by zones (`interior::hub`: the capital's palace
//!   round its court, other cities' older palaces round their great halls, hall houses and manors):
//!   floors the zones fell back on, vital's `check_interior` (every room reached, the corridor rule),
//!   the rooms of each one's programme, rooms of sound proportions (no longer than 2.5:1 but a long
//!   gallery, filling 60% of their box, a hall, a court or a chapel the only rooms over 60 squares,
//!   none a square wide); the capital's palace laid out as one, its front door on its axis, its front
//!   range (the ground floor's) and every floor above mirrored across its axis, its state rooms one into
//!   the next from the landing round to the throne room both ways (the audience chamber's side and the
//!   ballroom's) through doors mirrored across the axis, its throne dais on the axis, its forecourt and
//!   its paved way off the water (`palace_rules`); a chapel's altar at the end away from its doors.
//! - `temples`: temples, shrines and monasteries (`interior::hub`'s `temple` and `monastery`,
//!   `town::temple`): floors fallen back on; a big temple's footprint a cross ending in a round apse,
//!   a small one's a hall ending in one; each programme's rooms (a big one's nave, sanctuary in its
//!   apse, two side chapels, a vestry; a small one's single hall), the front door into the nave, the
//!   altar rails; the god's statue on the axis at the far end, the altar before it, candle stands
//!   mirrored; a big nave's arcades, the crossing's piers, the carved screen with its gate clear; pews
//!   mirrored facing the altar; the font; side altars; the god's look on every piece; a shrine of one
//!   room; a monastery round its cloister on its merged footprint: its church along a long side, the
//!   chapter house (an octagon), refectory, dorter, scriptorium and kitchen round the walk, the garth open
//!   on the map with nothing scattered in it, its well and walks, the arcade, the refectory furnished
//!   (`temple_rules`). Temples too small for a hall with an apse and monasteries with no cloister listed.
//! - `editor`: castles set in the street editor (`wards::change`) on the patches nearest the
//!   town wall (up to 3 a town, `PLACES=n` a world, default 20), with the walls on and off, each
//!   run through the castle checks, and its keep and barracks through the `hub` and `halls` rules;
//!   every non-castle building's id, outline, ward, function, kind, name and floors the same with
//!   the walls off as on; and every town castle's town with its walls taken down. Every monastery's
//!   town edited round it (its patch's lots made small, rerolled; the walls taken down, else a far
//!   patch rerolled): no building on another's ground (`Layout::grounds`), as generated or after the
//!   edit, and a front door on every building within 150 ft of it.
//! - `inns`: inns and caravanserais of `hub::inn::LEAST` squares or more laid out by zones: floors
//!   fallen back on, `check_interior` (the drinking rules: the bar and tables in the taproom, the front
//!   door into it, the stairs in it; every room reached), the programme (taproom, snug, private dining
//!   room, kitchen, scullery, storeroom; above, best chamber, guest rooms, dormitory, innkeeper's
//!   quarters: on one storey reasoned absent; the snug, the scullery and the private dining room on a
//!   service range too narrow for them listed apart), rooms no longer than 2.5:1, rooms per kind;
//!   those under the threshold and those over it no frame fits listed (`PLAN=b:L:id` prints a plan).
//!   Coaching inns (`town::palace::is_coaching`) round their yard: the yard reached from the street
//!   through an arch 2 squares wide, every stables onto it, every room upstairs off the gallery or a
//!   corridor, no other building on their ground, the front door and the arch within 20 ft of a
//!   street; those not round a yard, and every yard listed with where it is (`editor` edits each
//!   one's patch as a monastery's: still round a yard after).
//! - `hash` (not in `all`): a hash per settlement layout (`L:`), per building of it (`B:`, with what it
//!   is) and per building or tower interior (`b:`/`t:`), marked `C` in a settlement with a castle, one
//!   per line on stdout (everything else goes to stderr): run it on two trees and diff to see what
//!   changed.
//!
//! - `under` (not in `all`): every underground site of every layout (`u:` ways down, up to 4 sewer sections a layout,
//!   keep dungeons `k:`): a hash per site on stdout (`U <seed> <id> <hash> <theme> <levels>`; everything else goes
//!   with the other rows) and totals of sites, levels, rooms, doors, secret doors, loops (doors beyond a spanning
//!   tree of a level's rooms), shortcut loops (each saving 4+ hops) and the hops they save, and `u:` sites breaking `under::design::check`;
//!   portcullises and the water squares generators laid. THEME=temple,sunken_temple keeps only sites of those themes (`keep`, `undercroft`: keep dungeons, sewer sections). TERRAIN=1 also lays the terrain tools
//!   (`under::chasm`, `under::water`) on a copy of each site and counts what they laid and what they left unreached.
//!
//! Usage: cargo run --release -p worldgen --example towncheck -- [castles|reach|interiors|hub|halls|palaces|temples|editor|under|hash|all]... [seed|world.json...] [--keeps]
//! Default: `all` (every mode but `hash`), seeds 1 2 3: 30 to 50 s a seed under `--profile fast`, half of
//! it `interiors` (`--keeps` takes a second).
mod common;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use worldgen::battlemap::{Chunk, Kind, SQ, SQUARE_FT, Surface, URBAN_STREET, URBAN_TOWER, URBAN_WALL};
use worldgen::core::tile::TileKey;
use worldgen::pipeline::Executor;
use worldgen::t0::T0;
use worldgen::town::catalog::Ward;
use worldgen::town::geom::{add, area, centroid, contains, cross, dist, dot, len, lerp, mul, seg_dist, sub};
use worldgen::town::{self, Layout, PlazaKind};
use worldgen::{World, interior};

type P = [f64; 2];

/// `hash`: only the hashes go to stdout.
static QUIET: AtomicBool = AtomicBool::new(false);
macro_rules! say {
    ($($t:tt)*) => {
        if QUIET.load(Ordering::Relaxed) { eprintln!($($t)*) } else { println!($($t)*) }
    };
}

fn top() -> usize {
    std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4)
}

// ---- Reports: counts with their worst cases, and measures (a least or greatest value). ----

enum Row {
    Count { n: usize, worst: Vec<(f64, String)> },
    Measure { lowest: bool, v: Option<(f64, String)> },
    /// Every value seen (its least, median and greatest printed).
    Values { v: Vec<(f64, String)> },
}

#[derive(Default)]
struct Report {
    rows: Vec<(String, Row)>,
}

impl Report {
    fn row(&mut self, label: &str, make: impl FnOnce() -> Row) -> &mut Row {
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
    fn counts(&mut self, labels: &[&str]) {
        for l in labels {
            self.row(l, || Row::Count { n: 0, worst: Vec::new() });
        }
    }
    fn add(&mut self, label: &str, size: f64, what: String) {
        if let Row::Count { n, worst } = self.row(label, || Row::Count { n: 0, worst: Vec::new() }) {
            *n += 1;
            worst.push((size, what));
        }
    }
    /// Adds `k` to a count with nothing to list (how many were looked at).
    fn tally(&mut self, label: &str, k: usize) {
        if let Row::Count { n, .. } = self.row(label, || Row::Count { n: 0, worst: Vec::new() }) {
            *n += k;
        }
    }
    /// A least (`lowest`) or greatest value and where.
    fn measure(&mut self, label: &str, lowest: bool, v: f64, at: String) {
        if let Row::Measure { v: cur, .. } = self.row(label, || Row::Measure { lowest, v: None }) {
            if cur.as_ref().is_none_or(|c| if lowest { v < c.0 } else { v > c.0 }) {
                *cur = Some((v, at));
            }
        }
    }
    /// A value toward a spread (least, median, greatest).
    fn value(&mut self, label: &str, v: f64, at: String) {
        if let Row::Values { v: vals } = self.row(label, || Row::Values { v: Vec::new() }) {
            vals.push((v, at));
        }
    }
    fn print(&mut self) {
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
                }
            }
        }
    }
    /// Sums counts (and keeps the best measures) into `all`, the report over every world.
    fn merge_into(&self, all: &mut Report, world: &str) {
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
fn edge_dist(poly: &[P], p: P) -> f64 {
    let n = poly.len();
    (0..n).map(|k| seg_dist(p, poly[k], poly[(k + 1) % n])).fold(f64::MAX, f64::min)
}

/// How far inside a polygon `p` lies (negative outside).
fn depth(poly: &[P], p: P) -> f64 {
    let d = edge_dist(poly, p);
    if contains(poly, p) { d } else { -d }
}

/// Points every ~`step` ft along a polyline, each with the way its piece runs.
fn samples(w: &[P], step: f64) -> Vec<(P, P)> {
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

fn segx(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (o3, o4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

fn seg_seg(a: P, b: P, c: P, d: P) -> f64 {
    if segx(a, b, c, d) {
        return 0.0;
    }
    seg_dist(a, c, d).min(seg_dist(b, c, d)).min(seg_dist(c, a, b)).min(seg_dist(d, a, b))
}

/// Distance from a segment to a closed polygon's edges.
fn seg_poly(a: P, b: P, poly: &[P]) -> f64 {
    let n = poly.len();
    (0..n).map(|k| seg_seg(a, b, poly[k], poly[(k + 1) % n])).fold(f64::MAX, f64::min)
}

fn poly_overlap(a: &[P], b: &[P]) -> bool {
    let (n, m) = (a.len(), b.len());
    a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p)) || (0..n).any(|i| (0..m).any(|j| segx(a[i], a[(i + 1) % n], b[j], b[(j + 1) % m])))
}

/// A footprint shrunk by `d` toward its centroid (cheap; for convex-ish footprints).
fn shrink(a: &[P], d: f64) -> Vec<P> {
    let c = centroid(a);
    a.iter().map(|p| add(c, mul(sub(*p, c), ((len(sub(*p, c)) - d) / len(sub(*p, c)).max(1e-9)).max(0.0)))).collect()
}

fn bbox(pts: &[P]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])])
}

fn at(p: P) -> String {
    format!("({:.0}, {:.0})", p[0], p[1])
}

// ---- A world: its executor, battlemap chunks kept as made. ----

struct Ctx {
    ex: Executor,
    chunks: HashMap<(u32, u32), Rc<Chunk>>,
}

impl Ctx {
    fn chunk(&mut self, kx: u32, ky: u32) -> Rc<Chunk> {
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
    fn window(&mut self, r: [f64; 4]) -> Win {
        let size = self.ex.world.geom.tile_size_ft(self.ex.world.geom.max_level);
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let (kx0, ky0, kx1, ky1) = (k(r[0]), k(r[1]), k(r[2]), k(r[3]));
        let (cw, ch) = ((kx1 - kx0 + 1) as usize, (ky1 - ky0 + 1) as usize);
        let (nx, ny) = (cw * SQ, ch * SQ);
        let mut w = Win { ox: kx0 as f64 * size, oy: ky0 as f64 * size, nx, ny, block: vec![false; nx * ny], street: vec![false; nx * ny], objects: Vec::new() };
        for cj in 0..ch {
            for ci in 0..cw {
                let c = self.chunk(kx0 + ci as u32, ky0 + cj as u32);
                let idx = |i: usize, j: usize| (cj * SQ + j) * nx + ci * SQ + i;
                for q in 0..SQ * SQ {
                    let a = idx(q % SQ, q / SQ);
                    w.block[a] |= c.building[q] != 0 || matches!(c.urban[q], URBAN_WALL | URBAN_TOWER);
                    w.street[a] = c.urban[q] == URBAN_STREET;
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

struct Win {
    ox: f64,
    oy: f64,
    nx: usize,
    ny: usize,
    block: Vec<bool>,
    street: Vec<bool>,
    /// Every object (world ft) by name.
    objects: Vec<(P, String)>,
}

impl Win {
    fn centre(&self, k: usize) -> P {
        [self.ox + ((k % self.nx) as f64 + 0.5) * SQUARE_FT, self.oy + ((k / self.nx) as f64 + 0.5) * SQUARE_FT]
    }
    fn index(&self, p: P) -> Option<usize> {
        let (i, j) = (((p[0] - self.ox) / SQUARE_FT).floor(), ((p[1] - self.oy) / SQUARE_FT).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.nx && (j as usize) < self.ny).then(|| j as usize * self.nx + i as usize)
    }
    /// Open squares reached, square to square, from the open squares `from` picks.
    fn flood(&self, from: impl Fn(usize) -> bool) -> Vec<bool> {
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
    fn around(&self, k: usize) -> impl Iterator<Item = usize> {
        let (x, y, nx, ny) = (k % self.nx, k / self.nx, self.nx, self.ny);
        [(x > 0).then(|| k - 1), (x + 1 < nx).then(|| k + 1), (y > 0).then(|| k - nx), (y + 1 < ny).then(|| k + nx)].into_iter().flatten()
    }
    fn edge(&self, k: usize) -> bool {
        let (x, y) = (k % self.nx, k / self.nx);
        x == 0 || y == 0 || x == self.nx - 1 || y == self.ny - 1
    }
    /// The sizes of the pockets the squares `sealed` picks make (largest first) and where each starts.
    fn pockets(&self, sealed: &[bool]) -> Vec<(usize, P)> {
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
struct Castle<'a> {
    ring: Vec<P>,
    /// Walls on the ring (the outer curtain), wholly inside it (an inner curtain), and the rest (the town's).
    outer: Vec<&'a [P]>,
    inner: Vec<&'a [P]>,
    town: Vec<&'a [P]>,
    /// The castle's gate towers (a town's own castle's in `gate_towers`, one set in the street editor's in `extra_towers`).
    gate_towers: Vec<P>,
    /// Gate middles: pairs of its gate towers 52 ft apart.
    gates: Vec<P>,
    /// The gate nearest the ring.
    outer_gate: Option<P>,
}

impl<'a> Castle<'a> {
    fn of(l: &'a Layout) -> Option<Castle<'a>> {
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
    fn is_way(&self, r: &(Vec<P>, u8, f64)) -> bool {
        r.1 == 3 && (r.2 - 10.0).abs() < 1e-9 && self.gates.iter().any(|m| dist(r.0[0], *m) < 12.0)
    }
    fn curtains(&self) -> impl Iterator<Item = &&'a [P]> {
        self.outer.iter().chain(self.inner.iter())
    }
}

/// Whether a building is a keep (of pieces, not a temple's: or the castle's catalog entry).
fn is_keep(b: &town::Building) -> bool {
    (!b.parts.is_empty() && !town::temple::shaped(b)) || (b.func.is_some() && b.func.map(|f| f as usize) == town::catalog::index_of("castle"))
}

const CASTLE_COUNTS: [&str; 24] = [
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
const CASTLE_MORE: [&str; 5] = [
    "bailey spikes (corners under 35 deg, sides 40 ft+)",
    "keep pieces of 1,100 sq ft+ with a flat top left open",
    "duplicate building ids",
    "motte paths through a building or wall",
    "objects on a motte above its toe",
];

/// The castle checks on one layout with a castle (`tag` names it).
fn castle_checks(world: &World, t0: &T0, l: &Layout, tag: &str, capital: bool, r: &mut Report) {
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
        let wd = walls.iter().map(|s| if contains(&b.poly, s.0) { -1.0 } else { seg_poly(s.0, s.1, &b.poly) }).fold(f64::MAX, f64::min);
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
fn motte_objects(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
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

fn castles(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["castles", "castles with an inner curtain", "gate ways"]);
    r.counts(&CASTLE_COUNTS);
    r.counts(&CASTLE_MORE);
    r.counts(&["mottes", "objects on a motte's toe (its last 8 ft)"]);
    r.counts(&CASTLE_SPOTS);
    r.counts(&TOWN_RING);
    let n = town::layout_count(&ctx.ex.t0);
    for li in 0..n {
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, li);
        if l.site {
            continue;
        }
        castle_spot(ctx, &l, &format!("L{li} {:?}", l.tier), r);
        town_ring(ctx, &l, li, &format!("L{li} {:?}", l.tier), r);
        if l.castles.is_empty() {
            continue;
        }
        let capital = li < ctx.ex.t0.settlements.len() && ctx.ex.t0.settlements[li].capital;
        let tag = format!("L{li} {:?}", l.tier);
        castle_checks(&ctx.ex.world, &ctx.ex.t0, &l, &tag, capital, r);
        motte_objects(ctx, &l, &tag, r);
    }
}

const TOWN_RING: [&str; 5] = [
    "walled towns",
    "town-wall ring sides",
    "town-wall ring sides with 15 ft+ of dry ground and no wall (gates aside)",
    "quays",
    "quays with no water beyond them (a paved strip on dry ground)",
];

/// A town's quays all on the water; and a walled town's ring (the sides of its patches facing out
/// of town): every dry stretch of it has the wall on it (a town wall or a castle's curtain there),
/// but at its gates.
fn town_ring(ctx: &Ctx, l: &Layout, li: usize, tag: &str, r: &mut Report) {
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    // (The rivers' channels as `town` follows them.)
    let reach = 2.0 * l.radius + 1000.0;
    let mut river: Vec<(P, P, f64)> = Vec::new();
    for (ri, k) in t0.rivers.segments_near(l.center[0] - reach, l.center[1] - reach, l.center[0] + reach, l.center[1] + reach, 0.0) {
        let rc = &t0.rivers.rivers[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(2.0, 1200.0) as usize;
        let pts: Vec<(P, f64)> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft)).map(|c| (c.p, c.w)).collect();
        river.extend(pts.windows(2).map(|s| (s[0].0, s[1].0, 0.5 * s[0].1.max(s[1].1))));
    }
    let by_water = |p: P| {
        river.iter().any(|&(a, b, hw)| seg_dist(p, a, b) <= hw + 25.0)
            || wet(p)
            || [12.0, 25.0].iter().any(|&d| {
                (0..8).any(|k| {
                    let a = k as f64 * 0.785;
                    wet(add(p, [d * libm::cos(a), d * libm::sin(a)]))
                })
            })
    };
    // (Water beyond a side at `mid`, out to 150 ft: a patch of the sea, a lake or a harbour.)
    let water_beyond = |mid: P, out: P| [0.0, 30.0, 60.0, 100.0, 150.0].iter().any(|&d| by_water(add(mid, mul(out, d))));
    for q in l.plazas.iter().filter(|q| q.kind == PlazaKind::Quay && q.pts.len() == 4) {
        r.tally("quays", 1);
        let out = mul(sub(q.pts[0], q.pts[3]), 1.0 / dist(q.pts[0], q.pts[3]).max(1e-9));
        let mid = lerp(q.pts[0], q.pts[1], 0.5);
        if !water_beyond(mid, out) {
            r.add("quays with no water beyond them (a paved strip on dry ground)", dist(q.pts[0], q.pts[1]), format!("{tag}: {:.0} ft at {}", dist(q.pts[0], q.pts[1]), at(mid)));
        }
    }
    if l.walls.is_empty() {
        return;
    }
    let Ok(plan) = town::wards::plan_json(world, t0, li) else { return };
    if plan["walls"]["built"] != true {
        return;
    }
    r.tally("walled towns", 1);
    let pt = |v: &serde_json::Value| -> P { [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)] };
    let corners: HashMap<u64, P> = plan["corners"].as_array().into_iter().flatten().filter_map(|c| Some((c["corner"].as_u64()?, pt(&c["at"])))).collect();
    // (Each patch's sides, and whether its corners run anticlockwise.)
    let mut sides: Vec<(u64, u64, bool)> = Vec::new();
    for p in plan["patches"].as_array().into_iter().flatten().filter(|p| p["in_town"] == true) {
        let cs: Vec<u64> = p["corners"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()).collect();
        let ccw = area(&cs.iter().filter_map(|c| corners.get(c).copied()).collect::<Vec<_>>()) > 0.0;
        sides.extend((0..cs.len()).map(|k| (cs[k], cs[(k + 1) % cs.len()], ccw)));
    }
    let shared: std::collections::HashSet<(u64, u64)> = sides.iter().map(|s| (s.0, s.1)).collect();
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let by_gate = |p: P| l.gates.iter().any(|g| dist(*g, p) < 60.0);
    for &(a, b, ccw) in sides.iter().filter(|(a, b, _)| !shared.contains(&(*b, *a))) {
        let (Some(&pa), Some(&pb)) = (corners.get(&a), corners.get(&b)) else { continue };
        r.tally("town-wall ring sides", 1);
        let u = mul(sub(pb, pa), 1.0 / dist(pa, pb).max(1e-9));
        if water_beyond(lerp(pa, pb, 0.5), if ccw { [u[1], -u[0]] } else { [-u[1], u[0]] }) {
            continue;
        }
        let (mut bare, mut run, mut first) = (0.0f64, 0.0, None);
        for (p, _) in samples(&[pa, pb], 5.0) {
            if by_gate(p) || by_water(p) || walls.iter().any(|&(q, s)| seg_dist(p, q, s) <= 8.0) {
                run = 0.0;
            } else {
                run += 5.0;
                bare = bare.max(run);
                first.get_or_insert(p);
            }
        }
        if bare >= 15.0 {
            r.add("town-wall ring sides with 15 ft+ of dry ground and no wall (gates aside)", bare, format!("{tag}: {bare:.0} ft from {}", at(first.unwrap_or(pa))));
        }
    }
}

const CASTLE_SPOTS: [&str; 15] = [
    "town castles (planned or set)",
    "castle patches cut, a noble block left",
    "cuts skipped: patch too small",
    "cuts skipped: a side on the wall",
    "cuts skipped: leftover too small or narrow",
    "cuts skipped: castle's part too narrow or sharp",
    "cuts undone: no keep or a sharp ring in the castle's part",
    "castles with free-standing yard buildings",
    "free-standing yard buildings",
    "castles on the market square's patch",
    "town castles with no bailey",
    "interior castles (no bailey corner within 50 ft of the town wall)",
    "bailey squares",
    "bailey squares paved (street or cobbles)",
    "baileys over 5% paved",
];

/// A town's castle patch: not the market square's, with a bailey; inside the town or on its wall;
/// its bailey not paved; the bailey's size against its keep's.
fn castle_spot(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
    let Some(cp) = l.castle_patch else { return };
    r.tally("town castles (planned or set)", 1);
    let c = l.center;
    // (Where: the bailey's middle, else the town's.)
    let c = Castle::of(l).map_or(c, |k| centroid(&k.ring));
    let cut = ["", "castle patches cut, a noble block left", "cuts skipped: patch too small", "cuts skipped: a side on the wall", "cuts skipped: leftover too small or narrow", "cuts skipped: castle's part too narrow or sharp", "cuts undone: no keep or a sharp ring in the castle's part"];
    if let Some(k) = cut.get(l.castle_cut as usize).filter(|k| !k.is_empty()) {
        r.add(k, 1.0, format!("{tag}: patch {cp}, castle at {}", at(c)));
    }
    if l.castle_yard > 0 {
        r.add("castles with free-standing yard buildings", l.castle_yard as f64, format!("{tag}: {}, castle at {}", l.castle_yard, at(c)));
        r.tally("free-standing yard buildings", l.castle_yard as usize);
    }
    if l.plaza_patch == Some(cp) {
        r.add("castles on the market square's patch", 1.0, format!("{tag}: patch {cp} (town at {})", at(c)));
    }
    let c = l.center;
    let Some(castle) = Castle::of(l) else {
        r.add("town castles with no bailey", 1.0, format!("{tag}: patch {cp} (town at {})", at(c)));
        return;
    };
    let ring = &castle.ring;
    let mid = centroid(ring);
    let near_wall = ring.iter().any(|p| castle.town.iter().any(|w| w.windows(2).any(|s| seg_dist(*p, s[0], s[1]) < 50.0)));
    if !near_wall {
        r.add("interior castles (no bailey corner within 50 ft of the town wall)", 1.0, format!("{tag}: bailey at {}", at(mid)));
    }
    let keep: f64 = l.buildings.iter().filter(|b| b.ward == Ward::Castle && is_keep(b) && contains(ring, centroid(&b.poly))).map(|b| area(&b.poly).abs()).fold(0.0, f64::max);
    if keep > 0.0 {
        r.value("bailey area / keep area", area(ring).abs() / keep, format!("{tag} at {}", at(mid)));
    }
    let w = ctx.window(bbox(ring));
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    let (kx0, ky0) = ((w.ox / size) as u32, (w.oy / size) as u32);
    let (mut inside, mut paved) = (0, 0);
    for k in 0..w.nx * w.ny {
        let p = w.centre(k);
        if !contains(ring, p) {
            continue;
        }
        let (i, j) = (k % w.nx, k / w.nx);
        let ch = ctx.chunk(kx0 + (i / SQ) as u32, ky0 + (j / SQ) as u32);
        inside += 1;
        let q = (j % SQ) * SQ + i % SQ;
        paved += (ch.urban[q] == URBAN_STREET || ch.surface[q] == Surface::Cobble) as usize;
    }
    r.tally("bailey squares", inside);
    r.tally("bailey squares paved (street or cobbles)", paved);
    if paved * 20 > inside {
        r.add("baileys over 5% paved", paved as f64 / inside.max(1) as f64, format!("{tag}: {paved} of {inside} squares at {}", at(mid)));
    }
}

// ---- Play reach. ----

fn reach(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&[
        "castles flooded",
        "castle buildings checked",
        "castle buildings cut off",
        "outer gates not reached",
        "bailey squares open",
        "bailey squares sealed",
        "baileys over 3% sealed",
    ]);
    r.counts(&["parks flooded", "park squares open", "park squares sealed", "parks with sealed squares"]);
    let n = town::layout_count(&ctx.ex.t0);
    for li in 0..n {
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, li);
        if !l.castles.is_empty() && !l.site {
            castle_reach(ctx, &l, &format!("L{li} {:?}", l.tier), r);
        }
        for (pi, park) in l.parks.iter().enumerate() {
            let b = bbox(&park.pts);
            let w = ctx.window([b[0] - 60.0, b[1] - 60.0, b[2] + 60.0, b[3] + 60.0]);
            let in_park: Vec<bool> = (0..w.nx * w.ny).map(|k| contains(&park.pts, w.centre(k))).collect();
            let seen = w.flood(|k| !in_park[k] || w.edge(k));
            let sealed: Vec<bool> = (0..w.nx * w.ny).map(|k| in_park[k] && !w.block[k] && !seen[k]).collect();
            let n_sealed = sealed.iter().filter(|s| **s).count();
            r.tally("parks flooded", 1);
            r.tally("park squares open", (0..w.nx * w.ny).filter(|&k| in_park[k] && !w.block[k]).count());
            r.tally("park squares sealed", n_sealed);
            if n_sealed > 0 {
                let p = w.pockets(&sealed);
                r.add(
                    "parks with sealed squares",
                    n_sealed as f64,
                    format!("L{li} park {pi} ({:?}): {n_sealed} squares sealed, the largest pocket {} at {}", park.kind, p[0].0, at(p[0].1)),
                );
            }
        }
    }
}

fn castle_reach(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
    let Some(c) = Castle::of(l) else { return };
    let (cc, cr) = l.castles[0];
    let w = ctx.window([cc[0] - cr - 60.0, cc[1] - cr - 60.0, cc[0] + cr + 60.0, cc[1] + cr + 60.0]);
    // From outside: the window's edge and its streets.
    let seen = w.flood(|k| w.street[k] || w.edge(k));
    let bailey: Vec<bool> = (0..w.nx * w.ny).map(|k| contains(&c.ring, w.centre(k))).collect();
    let open = (0..w.nx * w.ny).filter(|&k| bailey[k] && !w.block[k]).count();
    let sealed: Vec<bool> = (0..w.nx * w.ny).map(|k| bailey[k] && !w.block[k] && !seen[k]).collect();
    let n_sealed = sealed.iter().filter(|s| **s).count();
    r.tally("castles flooded", 1);
    r.tally("bailey squares open", open);
    r.tally("bailey squares sealed", n_sealed);
    if n_sealed > 0 {
        let p = w.pockets(&sealed);
        r.measure("largest sealed pocket in a bailey (squares)", false, p[0].0 as f64, format!("{tag} at {}", at(p[0].1)));
        if n_sealed * 100 > 3 * open {
            r.add("baileys over 3% sealed", n_sealed as f64, format!("{tag}: {n_sealed} of {open} squares sealed, the largest pocket {} at {}", p[0].0, at(p[0].1)));
        }
    }
    if let Some(g) = c.outer_gate {
        let reached = w.index(g).is_some_and(|k| seen[k] || w.around(k).any(|m| seen[m]));
        if !reached {
            r.add("outer gates not reached", 1.0, format!("{tag}: the outer gate at {}", at(g)));
        }
    }
    // Castle buildings in the bailey: some open square within 6 ft of the outline reached.
    for b in l.buildings.iter().filter(|b| b.ward == Ward::Castle && b.poly.iter().all(|p| depth(&c.ring, *p) > -10.0)) {
        let bb = bbox(&b.poly);
        let (i0, j0) = (w.index([bb[0] - 6.0, bb[1] - 6.0]), w.index([bb[2] + 6.0, bb[3] + 6.0]));
        let (Some(k0), Some(k1)) = (i0, j0) else { continue };
        let near: Vec<usize> = (k0 / w.nx..=k1 / w.nx)
            .flat_map(|j| (k0 % w.nx..=k1 % w.nx).map(move |i| j * w.nx + i))
            .filter(|&k| {
                let p = w.centre(k);
                !contains(&b.poly, p) && edge_dist(&b.poly, p) < 6.0
            })
            .collect();
        if near.is_empty() {
            continue;
        }
        r.tally("castle buildings checked", 1);
        if !near.iter().any(|&k| seen[k] && !w.block[k]) {
            r.add(
                "castle buildings cut off",
                1.0,
                format!("{tag}: b:{}:{} ({}) at {}", l.index, b.id, b.func.map_or("home", |f| town::catalog::CATALOG[f as usize].name), at(centroid(&b.poly))),
            );
        }
    }
}

// ---- Interiors. ----

/// A layout's roofed buildings by 100-ft bucket, for "does this door open into a neighbour's wall".
fn building_buckets(l: &Layout) -> HashMap<(i64, i64), Vec<usize>> {
    let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (k, b) in l.buildings.iter().enumerate().filter(|(_, b)| b.structure == town::Structure::Roofed) {
        let r = bbox(&b.poly);
        for by in (r[1] / 100.0).floor() as i64..=(r[3] / 100.0).floor() as i64 {
            for bx in (r[0] / 100.0).floor() as i64..=(r[2] / 100.0).floor() as i64 {
                buckets.entry((bx, by)).or_default().push(k);
            }
        }
    }
    buckets
}

/// The corridor rule on a floor of 8 rooms or more with a hall or corridor: the most rooms a room is
/// from one (through the doors; `front`: the room the front door opens into counted as one too),
/// with the way to it. `None` where the rule doesn't apply.
fn corridor_hops(lv: &interior::Level, front: bool) -> Option<(usize, String)> {
    let entry = lv.doors.iter().find(|d| d.kind == "front").and_then(|d| d.rooms.iter().copied().find(|&r| r >= 0));
    let circ = |r: usize| interior::is_way_through(lv.rooms[r].kind);
    if lv.rooms.len() < 8 || !(0..lv.rooms.len()).any(circ) {
        return None;
    }
    let n = lv.rooms.len();
    let (mut hops, mut from) = (vec![usize::MAX; n], vec![usize::MAX; n]);
    let mut q = VecDeque::new();
    for r in (0..n).filter(|&r| circ(r) || (front && entry == Some(r as i16))) {
        hops[r] = 0;
        q.push_back(r);
    }
    while let Some(r) = q.pop_front() {
        for d in &lv.doors {
            let [a, b] = d.rooms;
            let other = if a == r as i16 {
                b
            } else if b == r as i16 {
                a
            } else {
                continue;
            };
            if other >= 0 && hops[other as usize] == usize::MAX {
                hops[other as usize] = hops[r] + 1;
                from[other as usize] = r;
                q.push_back(other as usize);
            }
        }
    }
    let worst = (0..n).filter(|&r| lv.rooms[r].squares > 0 && hops[r] != usize::MAX && interior::walkable(lv.rooms[r].kind)).max_by_key(|&r| (hops[r], std::cmp::Reverse(r)))?;
    let mut path = vec![lv.rooms[worst].kind.to_string()];
    let mut k = worst;
    while from[k] != usize::MAX {
        k = from[k];
        path.push(format!("{}{}", lv.rooms[k].kind, if entry == Some(k as i16) { " (front)" } else { "" }));
    }
    path.reverse();
    Some((hops[worst], path.join(" > ")))
}

/// The rules every interior keeps (vital's `check_interior`), each broken one as a line.
fn check_interior(world: &World, t0: &T0, l: &Layout, si: usize, id: &str, it: &interior::Interior, buckets: &HashMap<(i64, i64), Vec<usize>>) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    macro_rules! must {
        ($c:expr, $($t:tt)*) => {
            if !$c {
                bad.push(format!($($t)*));
            }
        };
    }
    let (nx, ny) = (it.nx, it.ny);
    for lv in &it.levels {
        for (ri, r) in lv.rooms.iter().enumerate() {
            must!(r.shape == 0 || interior::shape::fits(&lv.cells, nx, ri as i16, r.shape), "{}: the {} drawn shaped ({}) off its squares", lv.name, r.kind, r.shape);
        }
    }
    if !it.outlines.is_empty() {
        let inset = |x: f32, y: f32| {
            it.outlines
                .iter()
                .map(|o| {
                    (0..o.len())
                        .map(|k| {
                            let (a, b) = (o[k], o[(k + 1) % o.len()]);
                            ((b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])) / (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9)
                        })
                        .fold(f32::MAX, f32::min)
                })
                .fold(f32::MIN, f32::max)
        };
        let half = interior::WALL_HALF as f32;
        for lv in &it.levels {
            if let Some(k) = (0..nx * ny).find(|&k| lv.cells[k] >= 0 && inset((k % nx) as f32 + 0.5, (k / nx) as f32 + 0.5) < half + 0.1 - 1e-3) {
                bad.push(format!("{}: square {},{} only {:.2} inside its outer wall", lv.name, k % nx, k / nx, inset((k % nx) as f32 + 0.5, (k / nx) as f32 + 0.5)));
            }
            let st = it.stairs;
            let room = (0..nx * ny).any(|k| {
                let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                lv.cells[k] >= 0 && [(i, j), (i + 1.0, j), (i, j + 1.0), (i + 1.0, j + 1.0)].iter().all(|c| inset(c.0, c.1) >= half - 1e-3)
            });
            if lv.has_stairs && room && !world.file.edits.designs.contains_key(id) {
                let d = [(st[0], st[1]), (st[0] + st[2], st[1]), (st[0], st[1] + st[3]), (st[0] + st[2], st[1] + st[3])]
                    .iter()
                    .map(|&(x, y)| inset(x as f32, y as f32))
                    .fold(f32::MAX, f32::min);
                must!(d >= half - 1e-3, "{}: the outer wall cuts the stairs {st:?} ({d:.2} in)", lv.name);
            }
        }
        let g = &it.levels[it.entry_level];
        // (A keep's towers in grid units: a square of wall round each one's room.)
        let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok()).and_then(|k| l.building(k));
        let towers: Vec<Vec<P>> = own
            .map(|b| {
                b.parts
                    .iter()
                    .filter(|p| matches!(p.kind, town::PartKind::Tower { .. }))
                    .map(|p| {
                        p.poly
                            .iter()
                            .map(|q| {
                                let d = sub(*q, it.origin);
                                [dot(d, it.axis) / 5.0, dot(d, it.across) / 5.0]
                            })
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let by_tower = |v: &[f32; 2]| {
            let p = [v[0] as f64, v[1] as f64];
            towers.iter().any(|t| contains(t, p) || (0..t.len()).any(|k| seg_dist(p, t[k], t[(k + 1) % t.len()]) <= 1.0))
        };
        for v in it.outlines.iter().flatten() {
            let d = (0..nx * ny)
                .filter(|&k| g.cells[k] >= 0)
                .map(|k| {
                    let (i, j) = ((k % nx) as f32, (k / nx) as f32);
                    let (dx, dy) = ((i - v[0]).max(v[0] - i - 1.0).max(0.0), (j - v[1]).max(v[1] - j - 1.0).max(0.0));
                    (dx * dx + dy * dy).sqrt()
                })
                .fold(f32::MAX, f32::min);
            if d > if by_tower(v) { 1.5 } else { 1.0 } {
                bad.push(format!("its outer wall stands {d:.2} squares off its floor at {v:?}"));
                break;
            }
        }
    }
    must!(it.levels.len() >= 2 || world.file.edits.designs.contains_key(id), "only {} level", it.levels.len());
    if id.starts_with("b:") {
        match interior::design::design_of(world, t0, id, false) {
            Ok((d, _)) => match interior::design::problems(world, t0, id, &d) {
                Ok((built, problems)) => {
                    must!(problems.is_empty(), "as a design: {problems:?}");
                    must!(serde_json::to_string(&built).unwrap() == serde_json::to_string(it).unwrap(), "built again from its design, it differs");
                }
                Err(e) => bad.push(format!("as a design: {e}")),
            },
            Err(e) => bad.push(format!("no design: {e}")),
        }
    }
    let chapels = it.levels.iter().flat_map(|lv| lv.rooms.iter()).filter(|r| r.kind == "chapel").count();
    must!(chapels <= 1, "{chapels} chapels");
    let spirals: Vec<Vec<(u16, u16)>> = it.levels.iter().map(|lv| lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| (f.x, f.y)).collect()).collect();
    let mut at: Vec<(u16, u16)> = spirals.iter().flatten().copied().collect();
    at.sort();
    at.dedup();
    for s in at {
        let on: Vec<usize> = (0..spirals.len()).filter(|&li| spirals[li].contains(&s)).collect();
        must!(on.len() >= 2, "a spiral stair at {s:?} on one level only");
        must!(on.windows(2).all(|w| w[1] == w[0] + 1), "the spiral stair at {s:?} skips a level");
    }
    for lv in &it.levels {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, r)| r.kind == "cell") {
            let sq: Vec<usize> = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            if sq.is_empty() {
                continue;
            }
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            must!(x1 - x0 < 3 && y1 - y0 < 3, "{}: a {}x{} ft cell", lv.name, 5 * (x1 - x0 + 1), 5 * (y1 - y0 + 1));
            let doors: Vec<i16> = lv.doors.iter().filter(|d| d.rooms.contains(&(ri as i16))).map(|d| if d.rooms[0] == ri as i16 { d.rooms[1] } else { d.rooms[0] }).collect();
            must!(!doors.is_empty(), "{}: a cell with no door", lv.name);
            must!(doors.iter().all(|&o| o < 0 || lv.rooms[o as usize].kind != "cell"), "{}: a cell entered through another cell", lv.name);
        }
    }
    const DRINKING: [&str; 10] = ["Inn", "Tavern", "Alehouse", "Fine dining", "Brewery", "Winery", "Distillery", "Gambling den", "Smugglers' den", "Caravanserai"];
    if DRINKING.contains(&it.function) {
        let g = &it.levels[it.entry_level];
        let bars = g.furniture.iter().filter(|f| f.kind == "bar").count();
        let tables = g.furniture.iter().filter(|f| f.kind == "table" || f.kind == "booth_table").count();
        must!(bars >= 1 && tables >= 2, "{bars} bars and {tables} tables on the ground floor");
        let public = |r: i16| r >= 0 && matches!(g.rooms[r as usize].kind, "common room" | "taproom");
        match g.doors.iter().find(|d| d.kind == "front") {
            Some(front) => must!(public(front.rooms[0]), "the front door opens into the {}", g.rooms.get(front.rooms[0].max(0) as usize).map_or("?", |r| r.kind)),
            None => bad.push("no front door".into()),
        }
        let stair_room = g.cells[it.stairs[1] * nx + it.stairs[0]];
        must!(public(stair_room), "the stairs are in the {}", g.rooms[stair_room.max(0) as usize].kind);
    }
    let st = it.stairs;
    let on_stairs = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
    let door_sq = |d: &interior::Door| -> [(isize, isize); 2] {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
    };
    let inb = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny;
    for lv in &it.levels {
        if lv.has_stairs {
            let out = (st[1]..st[1] + st[3]).any(|j| (st[0]..st[0] + st[2]).any(|i| lv.cells[j * nx + i] < 0));
            must!(!out, "{}: stairs outside the building", lv.name);
        }
        let fronts = lv.doors.iter().filter(|d| d.kind == "front").count();
        must!(fronts == if lv.z == 0 { 1 } else { 0 }, "{}: {fronts} front doors", lv.name);
        if let Some(d) = lv.doors.iter().find(|d| d.kind == "front") {
            match door_sq(d).into_iter().find(|&(i, j)| !inb((i, j)) || lv.cells[j as usize * nx + i as usize] < 0) {
                Some(outside) => {
                    let p = interior::to_world(it, outside.0 as f64 + 0.5, outside.1 as f64 + 0.5);
                    let own = id.strip_prefix(&format!("b:{si}:")).and_then(|k| k.parse::<usize>().ok());
                    let hit = buckets
                        .get(&((p[0] / 100.0).floor() as i64, (p[1] / 100.0).floor() as i64))
                        .into_iter()
                        .flatten()
                        .find(|&&k| Some(l.buildings[k].id as usize) != own && contains(&l.buildings[k].poly, p));
                    must!(hit.is_none(), "the front door opens into building {}", hit.unwrap());
                }
                None => bad.push(format!("{}: the front door isn't on an outside wall", lv.name)),
            }
        }
        let mut blocked = vec![false; nx * ny];
        let mut misplaced = None;
        for f in &lv.furniture {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    if lv.has_stairs && on_stairs(i, j) {
                        misplaced.get_or_insert(format!("{}: {} on the stairs", lv.name, f.name));
                    }
                    if !interior::hall::is_platform(f.kind) && lv.doors.iter().any(|d| door_sq(d).contains(&(i as isize, j as isize))) {
                        misplaced.get_or_insert(format!("{}: {} in a doorway", lv.name, f.name));
                    }
                    if f.blocks_move {
                        blocked[j * nx + i] = true;
                    }
                }
            }
        }
        bad.extend(misplaced);
        let door_between = |a: (usize, usize), b: (usize, usize)| {
            lv.doors.iter().any(|d| {
                let s = door_sq(d);
                s.contains(&(a.0 as isize, a.1 as isize)) && s.contains(&(b.0 as isize, b.1 as isize))
            })
        };
        let mut starts: Vec<usize> =
            if lv.has_stairs { vec![st[1] * nx + st[0]] } else { lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize).collect() };
        if starts.is_empty() && lv.z == 0 {
            starts =
                lv.doors.iter().filter(|d| d.rooms[1] < 0).flat_map(door_sq).filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] >= 0).collect();
        }
        let mut seen = vec![false; nx * ny];
        let mut stack = starts.clone();
        for &k in &starts {
            seen[k] = true;
        }
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i as isize + di, j as isize + dj);
                if !inb((a, b)) {
                    continue;
                }
                let q = b as usize * nx + a as usize;
                if seen[q] || lv.cells[q] < 0 || blocked[q] || (lv.cells[q] != lv.cells[k] && !door_between((i, j), (a as usize, b as usize))) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        for d in &lv.doors {
            must!(door_sq(d).into_iter().filter(|&p| inb(p)).any(|(i, j)| seen[j as usize * nx + i as usize]), "{}: a {} door is cut off", lv.name, d.kind);
        }
        // (A spiral stair reached from the stairs, and from its room's doors with nothing but a rug
        // in the way: the way on up to the rooms above it.)
        let mut occupied = vec![false; nx * ny];
        for f in lv.furniture.iter().filter(|f| f.kind != "rug" && f.kind != "spiral_stair") {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    occupied[j * nx + i] = true;
                }
            }
        }
        for f in lv.furniture.iter().filter(|f| f.kind == "spiral_stair") {
            let k0 = f.y as usize * nx + f.x as usize;
            must!(seen[k0], "{}: the spiral stair at ({}, {}) is cut off", lv.name, f.x, f.y);
            let r = lv.cells[k0];
            let mut near: Vec<usize> = lv.doors.iter().flat_map(door_sq).filter(|&p| inb(p)).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] == r && seen[k]).collect();
            if near.is_empty() {
                continue;
            }
            let mut got = vec![false; nx * ny];
            for &k in &near {
                got[k] = true;
            }
            while let Some(k) = near.pop() {
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    let q = b.wrapping_mul(nx).wrapping_add(a);
                    if a < nx && b < ny && !got[q] && lv.cells[q] == r && !occupied[q] {
                        got[q] = true;
                        near.push(q);
                    }
                }
            }
            must!(got[k0], "{}: furniture shuts the spiral stair at ({}, {}) off from its room's doors", lv.name, f.x, f.y);
        }
        for (ri, room) in lv.rooms.iter().enumerate() {
            let open = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && !blocked[k]);
            let reached = (0..nx * ny).any(|k| lv.cells[k] == ri as i16 && seen[k]);
            if !interior::walkable(room.kind) {
                must!(!reached && !lv.doors.iter().any(|d| d.rooms.contains(&(ri as i16))), "{}: the {} has a way in", lv.name, room.kind);
                continue;
            }
            must!(!open || reached, "{}: the {} is unreachable", lv.name, room.kind);
        }
        if let Some((hops, path)) = corridor_hops(lv, true).filter(|h| h.0 > 2) {
            bad.push(format!("{}: {} rooms from a corridor ({path})", lv.name, hops - 1));
        }
    }
    bad
}

fn interiors(ctx: &mut Ctx, keeps: bool, r: &mut Report) {
    r.counts(&["building interiors checked", "tower interiors checked", "interiors breaking a rule", "functions with interiors breaking a rule"]);
    r.counts(&["floors the corridor rule applies to", "floors with a room over 2 rooms from a corridor", "the same, the front room not counted as a corridor"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    // Per function: checked, failing.
    let mut by_func: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let buckets = building_buckets(&l);
        let mut ids: Vec<String> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && (!keeps || is_keep(b))).map(|b| format!("b:{li}:{}", b.id)).collect();
        if !keeps {
            ids.extend((0..interior::towers(&l).len()).map(|k| format!("t:{li}:{k}")));
        }
        for id in ids {
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            r.tally(if id.starts_with('b') { "building interiors checked" } else { "tower interiors checked" }, 1);
            let bad = check_interior(world, t0, &l, li, &id, &it, &buckets);
            let e = by_func.entry(it.function).or_default();
            e.0 += 1;
            if !bad.is_empty() {
                e.1 += 1;
                r.add("interiors breaking a rule", bad.len() as f64, format!("{id} ({}): {}", it.function, bad.join("; ")));
            }
            for lv in &it.levels {
                if let Some((hops, _)) = corridor_hops(lv, true) {
                    r.tally("floors the corridor rule applies to", 1);
                    if hops > 2 {
                        r.add("floors with a room over 2 rooms from a corridor", hops as f64, format!("{id} ({}) {}", it.function, lv.name));
                    }
                }
                if let Some((hops, path)) = corridor_hops(lv, false).filter(|h| h.0 > 2) {
                    r.add("the same, the front room not counted as a corridor", hops as f64, format!("{id} ({}) {}: {} rooms ({path})", it.function, lv.name, hops - 1));
                }
            }
        }
    }
    for (f, (n, b)) in by_func.iter().filter(|f| f.1.1 > 0) {
        r.add("functions with interiors breaking a rule", *b as f64, format!("{f}: {b} of {n}"));
    }
}

// ---- Buildings laid out by zones. ----

const HUB_COUNTS: [&str; 30] = [
    "floors the zones fell back on",
    "breaking an interior rule (as in interiors)",
    "great or mess halls not one, on their floor",
    "front doors into the wrong room",
    "entry route reaches hall at dais end",
    "main stairs or front doors not into the hall through its screens passage",
    "private stairs not from the dais end up to the lord's floor",
    "main stairs standing in a room",
    "great chambers or solars over 2 squares from the lord's stair",
    "solars nearer the hall's low end than the lord's stair (their middles)",
    "great chambers or solars under 9 squares",
    "keeps with no great chamber or solar",
    "minstrels' galleries not reached along passages",
    "windows on open tops",
    "thrones in mess halls",
    "storeys above the hall with one room size for over half their rooms",
    "storeys above the hall with half their rooms on the squares of the storey below",
    "rooms above the hall longer than 2.5:1",
    "rooms above the hall filling under 60% of their box (its floor)",
    "rooms above the hall over 40 squares (a great chamber 60)",
    "keeps missing a room of their programme",
    "roofs that don't fill their box",
    "tower rooms all alike on a floor, or plain 'tower room'",
    "landings over 12 squares",
    "screens passages not one straight row",
    "service ends short of their rooms",
    "barracks with no officers' end",
    "rooms a square wide",
    "shell keeps' yards not round",
    "levels with furniture on an open top",
];

/// The way from room `from` into room `hall` on a floor, fewest doors first: the rooms on it (from
/// `from` to `hall`) and the middle of the door it enters `hall` by (None where it starts in it).
fn way_into(lv: &interior::Level, from: i16, hall: i16) -> Option<(Vec<i16>, Option<P>)> {
    if from < 0 {
        return None;
    }
    let n = lv.rooms.len();
    let mut prev: Vec<Option<(i16, usize)>> = vec![None; n];
    let mut seen = vec![false; n];
    let mut q = VecDeque::from([from]);
    seen[from as usize] = true;
    while let Some(a) = q.pop_front() {
        if a == hall {
            let mut path = vec![a];
            let mut door = None;
            let mut c = a;
            while let Some((p, di)) = prev[c as usize] {
                if door.is_none() {
                    let d = &lv.doors[di];
                    door = Some([0.5 * (d.a[0] + d.b[0]) as f64, 0.5 * (d.a[1] + d.b[1]) as f64]);
                }
                path.push(p);
                c = p;
            }
            path.reverse();
            return Some((path, door));
        }
        for (di, d) in lv.doors.iter().enumerate().filter(|(_, d)| d.rooms.contains(&a) && d.rooms.iter().all(|&o| o >= 0)) {
            let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
            if !seen[o as usize] {
                seen[o as usize] = true;
                prev[o as usize] = Some((a, di));
                q.push_back(o);
            }
        }
    }
    None
}

/// The squares either side of a door's edge.
fn door_squares(d: &interior::Door) -> [(isize, isize); 2] {
    let (x0, y0) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize);
    if d.a[0] == d.b[0] { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
}

/// `hub`: castle keeps and barracks of 150 squares or more, laid out by zones (`interior::hub`),
/// against the rules their layout keeps (the user's calls): no floor fallen back on the shared
/// layout; the hall on its floor (a great tower's and a tower keep's on the first); the front door
/// into the screens passage (or a forebuilding's entrance hall onto it), a great tower's entrance
/// hall, a barracks' mess hall, a shell keep's gate passage; the main stair landing in passages or
/// landings, at the low end: the front door (a hall on the ground floor) and the main stair reach
/// the hall through its screens passage, entering it on the half away from its dais (the user's
/// call, 2026-10-27); the lord's private spiral stair from a room off the hall at the dais end up to
/// the floor above; the great chamber and solar by the lord's stair (the private one, else the main:
/// a square of each within 2 squares of it, a solar's middle nearer it than the hall's low end), 9
/// squares or more; a
/// minstrels' gallery reached from the stair through passages; no windows or furniture on open tops;
/// no throne in a mess hall; rooms of different sizes, no longer than 2.5:1, filling 60% of the floor
/// in their box, no storey laid out as the one below it (a forebuilding's storeys aside); tower rooms
/// of different kinds; passage-sized landings, a straight screens passage, the service end's rooms, a
/// barracks' officers' end; no room a square wide; a shell keep's yard round. Street-editor castles'
/// keeps too (`editor`).
fn hub(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["buildings laid out by zones"]);
    r.counts(&HUB_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        for b in &l.buildings {
            if !interior::hub_kind(t0, &l, b).is_some_and(|k| k.is_keep()) {
                continue;
            }
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            keep_rules(world, t0, &l, li, b, "", buckets, r);
        }
    }
}

/// A building laid out by zones through `check_interior` and `hub_rules` (`tag`: what to name it by
/// beside its id).
#[allow(clippy::too_many_arguments)]
fn keep_rules(world: &World, t0: &T0, l: &Layout, li: usize, b: &town::Building, tag: &str, buckets: &HashMap<(i64, i64), Vec<usize>>, r: &mut Report) {
    let Some(kind) = interior::hub_kind(t0, l, b).filter(|k| k.is_keep()) else { return };
    let id = format!("b:{li}:{}", b.id);
    let Some(it) = interior::generate_id(world, t0, &id) else { return };
    r.tally("buildings laid out by zones", 1);
    r.tally(&format!("  {}{}", kind.name(), if kind.name().ends_with('s') { "" } else { "s" }), 1);
    let named = format!("{tag}{id}");
    // (`PLAN=<tag and id>`: its text plan, one letter a room.)
    if std::env::var("PLAN").is_ok_and(|p| p.trim() == named.trim()) {
        let c = centroid(&b.poly);
        say!("-- {named} ({}) at ({:.0}, {:.0}) ft", kind.name(), c[0], c[1]);
        for lv in &it.levels {
            say!("-- {} (z {}): {}", lv.name, lv.z, lv.rooms.iter().map(|r| format!("{}={}", r.kind, r.squares)).collect::<Vec<_>>().join(", "));
            for j in 0..it.ny {
                let row: String = (0..it.nx)
                    .map(|i| {
                        let c = lv.cells[j * it.nx + i];
                        let st = it.stairs;
                        if lv.has_stairs && i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3] {
                            '#'
                        } else if c < 0 {
                            ' '
                        } else if lv.rooms[c as usize].kind == interior::OPEN_BELOW {
                            ':'
                        } else {
                            (b'a' + (c as u8 % 26)) as char
                        }
                    })
                    .collect();
                say!("  {row}");
            }
        }
    }
    let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
    if !fell.is_empty() {
        r.add("floors the zones fell back on", fell.len() as f64, format!("{named} ({}): z {fell:?}", kind.name()));
    }
    let bad = check_interior(world, t0, l, li, &id, &it, buckets);
    if !bad.is_empty() {
        r.add("breaking an interior rule (as in interiors)", bad.len() as f64, format!("{named} ({}): {}", kind.name(), bad.join("; ")));
    }
    hub_rules(l, b, kind, &named, &it, r);
}

fn hub_rules(l: &Layout, b: &town::Building, kind: interior::hub::Kind, id: &str, it: &interior::Interior, r: &mut Report) {
    use interior::hub::Kind;
    let (nx, ny) = (it.nx, it.ny);
    let tag = format!("{id} ({})", kind.name());
    let sh = interior::Shell::of(l, b);
    let squares = |lv: &interior::Level, ri: usize| -> Vec<usize> { (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect() };
    let mid = |sq: &[usize]| -> P { [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64] };
    let room_at = |lv: &interior::Level, (i, j): (isize, isize)| -> i16 { if i < 0 || j < 0 || i as usize >= nx || j as usize >= ny { -1 } else { lv.cells[j as usize * nx + i as usize] } };
    let between = |lv: &interior::Level, a: i16, c: i16| lv.doors.iter().any(|d| d.rooms == [a.min(c), a.max(c)]);
    let open_top = |k: &str| matches!(k, "battlements" | "wall walk") || k == interior::ROOF_SLOPE;
    let barracks = kind == Kind::Barracks;
    let hall_kind = if barracks { "mess hall" } else { "great hall" };
    let hall_z = if matches!(kind, Kind::Great | Kind::Plain) && b.floors >= 2 { 1 } else { 0 };
    let halls: Vec<(usize, usize)> = it.levels.iter().enumerate().filter(|(_, lv)| lv.z >= 0).flat_map(|(li, lv)| (0..lv.rooms.len()).filter(|&ri| lv.rooms[ri].kind == hall_kind && lv.rooms[ri].squares > 0).map(move |ri| (li, ri))).collect();
    if halls.len() != 1 || it.levels[halls[0].0].z != hall_z {
        r.add("great or mess halls not one, on their floor", halls.len() as f64, format!("{tag}: {:?}", halls.iter().map(|h| it.levels[h.0].z).collect::<Vec<_>>()));
    }
    let st = it.stairs;
    let sc: P = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
    // Every floor: the main stair lands in a passage, a landing, a yard or on a walk.
    for lv in it.levels.iter().filter(|lv| lv.z >= 0 && lv.has_stairs) {
        let ri = lv.cells[st[1] * nx + st[0]];
        if ri >= 0 && !interior::is_way_through(lv.rooms[ri as usize].kind) {
            r.add("main stairs standing in a room", 1.0, format!("{tag} {}: in the {}", lv.name, lv.rooms[ri as usize].kind));
        }
    }
    // The front door.
    if let Some(g0) = it.levels.iter().find(|lv| lv.z == 0)
        && let Some(front) = g0.doors.iter().find(|d| d.kind == "front")
    {
        let fr = front.rooms[0];
        let k = g0.rooms[fr as usize].kind;
        let ok = match kind {
            Kind::Hall | Kind::L | Kind::T => k == "screens passage" || (k == "entrance hall" && g0.rooms.iter().enumerate().any(|(o, ro)| ro.kind == "screens passage" && between(g0, fr, o as i16))),
            Kind::Great | Kind::Plain => k == "entrance hall" || (b.floors < 2 && k == "great hall"),
            Kind::Barracks => k == "mess hall",
            Kind::ShellKeep => k == "gate passage",
            Kind::Palace | Kind::OldPalace | Kind::HallHouse | Kind::Manor | Kind::Church | Kind::TempleHall | Kind::Monastery => true,
            _ => true,
        };
        if !ok {
            r.add("front doors into the wrong room", 1.0, format!("{tag}: into the {k}"));
        }
    }
    // Windows and furniture on open tops.
    for lv in &it.levels {
        for w in &lv.windows {
            let d = interior::Door { a: [w[0], w[1]], b: [w[2], w[3]], kind: "window", rooms: [-1, -1], lock: 0 };
            if door_squares(&d).iter().any(|&s| room_at(lv, s) >= 0 && open_top(lv.rooms[room_at(lv, s) as usize].kind)) {
                r.add("windows on open tops", 1.0, format!("{tag} {}: ({}, {})", lv.name, w[0], w[1]));
            }
        }
        let on_top = lv.furniture.iter().filter(|f| f.kind != "spiral_stair" && lv.cells[f.y as usize * nx + f.x as usize] >= 0 && open_top(lv.rooms[lv.cells[f.y as usize * nx + f.x as usize] as usize].kind)).count();
        if on_top > 0 {
            r.add("levels with furniture on an open top", on_top as f64, format!("{tag} {}: {on_top} pieces", lv.name));
        }
        for f in lv.furniture.iter().filter(|f| f.kind == "throne") {
            let c = lv.cells[f.y as usize * nx + f.x as usize];
            if c >= 0 && lv.rooms[c as usize].kind == "mess hall" {
                r.add("thrones in mess halls", 1.0, format!("{tag} {}: ({}, {})", lv.name, f.x, f.y));
            }
        }
    }
    // Rooms: sizes, slivers, tower rooms, storeys alike, landings.
    let mut below: Option<&interior::Level> = None;
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        let mut tower_rooms: Vec<i16> = sh.towers.iter().map(|t| lv.cells[t.2]).filter(|&c| c >= 0).collect();
        // (A barracks' corner towers: rooms in a 3-square box at a corner of the grid.)
        if barracks {
            for (ri, room) in lv.rooms.iter().enumerate() {
                let sq = squares(lv, ri);
                let corner = |v: usize, n: usize| v < 3 || v + 3 >= n;
                if room.squares > 0 && room.squares <= 9 && sq.iter().all(|&k| corner(k % nx, nx) && corner(k / nx, ny)) {
                    tower_rooms.push(ri as i16);
                }
            }
        }
        let ordinary = |ri: usize| {
            let k = lv.rooms[ri].kind;
            lv.rooms[ri].squares > 0 && interior::walkable(k) && !interior::is_way_through(k) && !tower_rooms.contains(&(ri as i16)) && k != "guard post"
        };
        let sizes: Vec<usize> = (0..lv.rooms.len()).filter(|&ri| ordinary(ri)).map(|ri| lv.rooms[ri].squares).collect();
        if sizes.len() >= 5 && lv.z > hall_z {
            let most = sizes.iter().map(|s| sizes.iter().filter(|t| *t == s).count()).max().unwrap_or(0);
            if most * 2 > sizes.len() {
                r.add("storeys above the hall with one room size for over half their rooms", most as f64 / sizes.len() as f64, format!("{tag} {}: {sizes:?}", lv.name));
            }
        }
        for ri in (0..lv.rooms.len()).filter(|&ri| ordinary(ri) && lv.rooms[ri].squares >= 2) {
            let mine = |i: usize, j: usize| i < nx && j < ny && lv.cells[j * nx + i] == ri as i16;
            if !(0..nx * ny).any(|k| mine(k % nx, k / nx) && mine(k % nx + 1, k / nx) && mine(k % nx, k / nx + 1) && mine(k % nx + 1, k / nx + 1)) {
                r.add("rooms a square wide", lv.rooms[ri].squares as f64, format!("{tag} {}: the {} ({} squares)", lv.name, lv.rooms[ri].kind, lv.rooms[ri].squares));
            }
        }
        let kinds: Vec<&str> = tower_rooms.iter().map(|&c| lv.rooms[c as usize].kind).filter(|&k| k != "tower top").collect();
        if kinds.contains(&"tower room") || (kinds.len() >= 3 && kinds.iter().all(|k| *k == kinds[0])) {
            r.add("tower rooms all alike on a floor, or plain 'tower room'", kinds.len() as f64, format!("{tag} {}: {kinds:?}", lv.name));
        }
        for (ri, room) in lv.rooms.iter().enumerate().filter(|(_, room)| room.kind == "landing" && room.squares > 12) {
            r.add("landings over 12 squares", room.squares as f64, format!("{tag} {}: {} squares (room {ri})", lv.name, room.squares));
        }
        // Above the hall: ordinary rooms (not halls, passages, galleries or towers) no longer than
        // 2.5 times their width and of 40 squares at most (a great chamber 60), and half the rooms
        // or more not on just the squares of a room of the storey below.
        let upper: Vec<usize> = (0..lv.rooms.len()).filter(|&ri| lv.z > hall_z && !lv.roof && ordinary(ri) && !open_top(lv.rooms[ri].kind)).collect();
        for &ri in &upper {
            let (room, sq) = (&lv.rooms[ri], squares(lv, ri));
            let (xs, ys): (Vec<usize>, Vec<usize>) = sq.iter().map(|&k| (k % nx, k / nx)).unzip();
            let (w, h) = (xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1, ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1);
            if 2 * w.max(h) > 5 * w.min(h) {
                r.add("rooms above the hall longer than 2.5:1", w.max(h) as f64 / w.min(h) as f64, format!("{tag} {}: the {} {w} x {h} ({} squares)", lv.name, room.kind, room.squares));
            }
            // (A room with a tail a square wide: the floor in its box mostly other rooms'; a shell
            // keep's lean-tos bend round its ring.)
            let (x0, y0) = (xs.iter().min().unwrap(), ys.iter().min().unwrap());
            let floor = (*y0..y0 + h).flat_map(|j| (*x0..x0 + w).map(move |i| j * nx + i)).filter(|&k| lv.cells[k] >= 0).count();
            if sq.len() * 5 < floor * 3 && kind != Kind::ShellKeep {
                r.add("rooms above the hall filling under 60% of their box (its floor)", floor as f64 / sq.len() as f64, format!("{tag} {}: the {} of {} squares in a {w} x {h} box", lv.name, room.kind, room.squares));
            }
            let cap = if room.kind == "great chamber" { 60 } else { 40 };
            if room.squares > cap {
                r.add("rooms above the hall over 40 squares (a great chamber 60)", room.squares as f64, format!("{tag} {}: the {} of {} squares", lv.name, room.kind, room.squares));
            }
        }
        // (But a forebuilding's: its storeys one room each, the entrance hall and the chapel over it.)
        let fore = |sq: &[usize]| sq.iter().all(|&k| sh.owner.get(k).and_then(|&o| b.parts.get(o)).is_some_and(|p| p.kind == town::PartKind::Forebuilding));
        if let Some(lb) = below.filter(|lb| !lb.roof && lb.z + 1 == lv.z && upper.len() >= 2) {
            let below_sq: Vec<Vec<usize>> = (0..lb.rooms.len()).map(|ri| squares(lb, ri)).collect();
            let same = upper.iter().filter(|&&ri| !fore(&squares(lv, ri)) && below_sq.contains(&squares(lv, ri))).count();
            if 2 * same >= upper.len() {
                r.add("storeys above the hall with half their rooms on the squares of the storey below", same as f64 / upper.len() as f64, format!("{tag} {} over {}: {same} of {} rooms", lv.name, lb.name, upper.len()));
            }
        }
        below = Some(lv);
    }
    // Roofs: hipped over the box they fill (a shell keep's ring round its yard aside).
    for lv in it.levels.iter().filter(|_| kind != Kind::ShellKeep) {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, room)| room.kind == interior::ROOF_SLOPE && room.squares > 0) {
            let sq = squares(lv, ri);
            let (xs, ys): (Vec<usize>, Vec<usize>) = sq.iter().map(|&k| (k % nx, k / nx)).unzip();
            let (w, h) = (xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1, ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1);
            if w * h != sq.len() {
                r.add("roofs that don't fill their box", (w * h - sq.len()) as f64, format!("{tag} {}: {} squares in a {w} x {h} box", lv.name, sq.len()));
            }
        }
    }
    // A keep's programme: the great chamber, the solar, the chapel, a treasury, an armory, a
    // garrison or stores.
    if !barracks {
        let has = |k: &str| it.levels.iter().any(|lv| lv.rooms.iter().any(|ro| ro.kind == k && ro.squares > 0));
        let missing: Vec<&str> = ["great chamber", "solar", "chapel", "treasury", "armory", "garrison"].into_iter().filter(|k| !has(k) && !(*k == "garrison" && has("storeroom"))).collect();
        if !missing.is_empty() {
            r.add("keeps missing a room of their programme", missing.len() as f64, format!("{tag}: no {}", missing.join(", ")));
        }
    }
    if kind == Kind::ShellKeep {
        for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
            for (ri, room) in lv.rooms.iter().enumerate().filter(|(_, room)| room.kind == "keep yard" || room.kind == interior::OPEN_BELOW) {
                if room.shape != interior::shape::ROUND {
                    r.add("shell keeps' yards not round", 1.0, format!("{tag} {}: the {} (room {ri})", lv.name, room.kind));
                }
            }
        }
        return;
    }
    let Some(&(hl, hall)) = halls.first() else { return };
    let g = &it.levels[hl];
    let hall = hall as i16;
    let first = |k: &str| g.rooms.iter().position(|ro| ro.kind == k && ro.squares > 0).map(|ri| ri as i16);
    // The low end: the screens passage, else the kitchen.
    let low = first("screens passage").or_else(|| first("kitchen")).map(|ri| mid(&squares(g, ri as usize)));
    let hc = mid(&squares(g, hall as usize));
    // The way in: from the main stair, and from the front door where the hall is on the ground
    // floor, into the hall through its screens passage (where it has one) on the half away from its
    // dais.
    let dais: Option<P> = g.furniture.iter().find(|f| matches!(f.kind, "throne_dais" | "low_dais") && g.cells[f.y as usize * nx + f.x as usize] == hall).map(|f| [f.x as f64 + 0.5 * f.w as f64, f.y as f64 + 0.5 * f.h as f64]);
    let screens = first("screens passage");
    let mut starts = vec![("main stair", g.cells[st[1] * nx + st[0]], sc)];
    if g.z == 0
        && let Some(front) = g.doors.iter().find(|d| d.kind == "front")
    {
        starts.push(("front door", front.rooms[0], [0.5 * (front.a[0] + front.b[0]) as f64, 0.5 * (front.a[1] + front.b[1]) as f64]));
    }
    for (what, from, at) in starts {
        let Some((path, door)) = way_into(g, from, hall) else {
            r.add("entry route reaches hall at dais end", 1.0, format!("{tag}: no way from the {what} into the hall"));
            continue;
        };
        let into = door.unwrap_or(at);
        if dais.is_some_and(|d| dot(sub(into, hc), sub(d, hc)) >= 0.0) {
            let by: Vec<&str> = path.iter().map(|&ri| g.rooms[ri as usize].kind).collect();
            r.add("entry route reaches hall at dais end", 1.0, format!("{tag}: from the {what} by {}", by.join(" > ")));
        }
        if screens.is_some_and(|s| path.len() < 2 || path[path.len() - 2] != s) {
            r.add("main stairs or front doors not into the hall through its screens passage", 1.0, format!("{tag}: from the {what} by the {}", if path.len() >= 2 { g.rooms[path[path.len() - 2] as usize].kind } else { "hall itself" }));
        }
    }
    // The lord's private stair: a spiral stair on the hall's floor (not a tower's: at a grid corner or
    // a piece tower's middle) in a room the hall opens into at the dais end, up to the floor above.
    let towered = |k: usize| sh.towers.iter().any(|t| t.2 == k) || [0, nx - 1, nx * (ny - 1), nx * ny - 1].contains(&k);
    let private: Option<usize> = g.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize).find(|&k| !towered(k));
    let above = it.levels.iter().find(|lv| lv.z == g.z + 1 && !lv.roof);
    match private {
        Some(k) => {
            let pr = g.cells[k];
            let up = above.is_some_and(|lv| lv.cells[k] >= 0 && lv.furniture.iter().any(|f| f.kind == "spiral_stair" && f.y as usize * nx + f.x as usize == k));
            let at_dais = dais.is_none_or(|d| dot(sub([(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5], hc), sub(d, hc)) > 0.0);
            if pr < 0 || pr == hall || !between(g, pr, hall) || !up || !at_dais {
                r.add("private stairs not from the dais end up to the lord's floor", 1.0, format!("{tag}: in the {}{}{}", if pr >= 0 { g.rooms[pr as usize].kind } else { "-" }, if up { "" } else { ", not up" }, if at_dais { "" } else { ", at the low end" }));
            }
        }
        None if screens.is_some() && above.is_some() => r.add("private stairs not from the dais end up to the lord's floor", 1.0, format!("{tag}: none")),
        None => {}
    }
    // (The stair the lord's rooms go by, as a block: the private one, else the main.)
    let st = private.map_or(st, |k| [k % nx, k / nx, 1, 1]);
    let sc: P = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
    if matches!(kind, Kind::Hall | Kind::L | Kind::T) {
        if let Some(scr) = first("screens passage") {
            let sq = squares(g, scr as usize);
            let (xs, ys): (Vec<usize>, Vec<usize>) = sq.iter().map(|&k| (k % nx, k / nx)).unzip();
            let (w, h) = (xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1, ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1);
            if w.min(h) > 1 {
                r.add("screens passages not one straight row", sq.len() as f64, format!("{tag}: {w} x {h}, {} squares", sq.len()));
            }
        }
        if first("kitchen").is_none() || (first("buttery").is_none() && first("pantry").is_none()) {
            r.add("service ends short of their rooms", 1.0, format!("{tag}: {:?}", g.rooms.iter().filter(|ro| matches!(ro.kind, "kitchen" | "buttery" | "pantry")).map(|ro| (ro.kind, ro.squares)).collect::<Vec<_>>()));
        }
    }
    if barracks {
        if first("kitchen").is_none() || first("officers' quarters").is_none() {
            r.add("barracks with no officers' end", 1.0, format!("{tag}: {:?}", g.rooms.iter().filter(|ro| matches!(ro.kind, "kitchen" | "officers' quarters" | "armory")).map(|ro| (ro.kind, ro.squares)).collect::<Vec<_>>()));
        }
        return;
    }
    // The lord's rooms: by the lord's stair, of a good size; a solar's middle nearer the stair than the
    // hall's low end's (the screens passage's, else the kitchen's).
    let mut lords = 0;
    for lv in it.levels.iter().filter(|lv| lv.z > hall_z) {
        for (ri, room) in lv.rooms.iter().enumerate().filter(|(_, room)| matches!(room.kind, "great chamber" | "solar") && room.squares > 0) {
            lords += 1;
            if room.squares < 9 {
                r.add("great chambers or solars under 9 squares", 9.0 - room.squares as f64, format!("{tag} {}: the {} of {} squares", lv.name, room.kind, room.squares));
            }
            // (Its nearest square's gap to the stair block, in squares.)
            let gap = squares(lv, ri)
                .iter()
                .map(|&k| {
                    let (i, j) = ((k % nx) as f64, (k / nx) as f64);
                    let dx = (st[0] as f64 - i - 1.0).max(i - (st[0] + st[2]) as f64).max(0.0);
                    let dy = (st[1] as f64 - j - 1.0).max(j - (st[1] + st[3]) as f64).max(0.0);
                    dx.max(dy)
                })
                .fold(f64::MAX, f64::min);
            if gap > 2.0 {
                r.add("great chambers or solars over 2 squares from the lord's stair", gap, format!("{tag} {}: the {} {gap:.0} squares from the stair", lv.name, room.kind));
            }
            if let Some(lo) = low.filter(|_| room.kind == "solar") {
                let c = mid(&squares(lv, ri));
                let (ds, dl) = (dist(c, sc), dist(c, lo));
                if ds > dl {
                    r.add("solars nearer the hall's low end than the lord's stair (their middles)", ds - dl, format!("{tag} {}: {ds:.1} from the stair, {dl:.1} from the low end", lv.name));
                }
            }
        }
    }
    if lords == 0 {
        r.add("keeps with no great chamber or solar", 1.0, tag.clone());
    }
    // A hall open two storeys: its gallery reached from the stair through passages only.
    if let Some(up) = it.levels.get(hl + 1).filter(|up| up.rooms.iter().any(|ro| ro.kind == interior::OPEN_BELOW && ro.squares > 0))
        && let Some(gal) = up.rooms.iter().position(|ro| ro.kind == "minstrels' gallery")
    {
        let start = up.cells[st[1] * nx + st[0]];
        let mut seen = vec![false; up.rooms.len()];
        let mut stack = vec![start];
        while let Some(a) = stack.pop() {
            if a < 0 || seen[a as usize] || !interior::is_circulation(up.rooms[a as usize].kind) {
                continue;
            }
            seen[a as usize] = true;
            for d in up.doors.iter().filter(|d| d.rooms.contains(&a)) {
                stack.push(if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] });
            }
        }
        if !seen[gal] {
            r.add("minstrels' galleries not reached along passages", 1.0, format!("{tag} {}: from the {}", up.name, if start >= 0 { up.rooms[start as usize].kind } else { "-" }));
        }
    }
}

// ---- Great halls furnished as wholes. ----

const HALL_COUNTS: [&str; 25] = [
    "castle halls and throne rooms with no throne dais",
    "town, guild, estate, banqueting halls and audience chambers of 5 x 3 or more with no dais",
    "estates' halls laid out by zones with no dais",
    "daises not at the far end",
    "daises off the axis",
    "halls of 6 x 5 or more with no carpet runner (castle, town, guild, estate)",
    "runners off the axis",
    "hall pieces with no mirror across the axis",
    "halls with a square shut off",
    "chairs of office not on a dais",
    "estates' and banqueting halls' high tables not on their dais",
    "mess halls and ballrooms with a dais, a throne, a high table or a chair of office",
    "open halls with no musicians on the gallery",
    "long galleries under 3 squares across",
    "long gallery pieces off its long walls or across its walk",
    "long galleries with their walk cut",
    "chapels with no altar on the axis at the far end",
    "chapel pews facing other than the altar",
    "chapel pews across the aisle",
    "audience chambers with no chair of state on their dais",
    "thrones outside castles and palaces",
    "castles flying more than one house",
    "banners in a castle not of its house's colours given",
    "halls of 6 x 5 or more with no tables",
    "pillars before the hearth, a hanging or a door, or across the way by a long wall",
];

/// One of a castle's buildings (its keep, or in its ward).
fn of_castle(b: &town::Building) -> bool {
    b.keep.is_some() || b.ward == Ward::Castle || is_keep(b)
}

/// `halls`: the great halls furnished as wholes (`interior::hall_plans`) and the thrones and banners
/// of every building (`hall_rules`), each layout of the world.
fn halls(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["halls furnished as wholes"]);
    r.counts(&HALL_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        layout_halls(world, t0, &l, li, &format!("#{li}"), &|_| true, r);
    }
}

/// The hall rules over a layout's buildings (`only`: those to look at): each interior's thrones,
/// every banner of a castle's buildings one house, and each hall furnished as a whole (`hall_rules`).
fn layout_halls(world: &World, t0: &T0, l: &Layout, li: usize, tag: &str, only: &dyn Fn(&town::Building) -> bool, r: &mut Report) {
    let mut houses: BTreeMap<u8, String> = BTreeMap::new();
    for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && only(b)) {
        let id = format!("b:{li}:{}", b.id);
        let Some(it) = interior::generate_id(world, t0, &id) else { continue };
        let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
        let thrones = of_castle(b) || key == Some("palace");
        for lv in &it.levels {
            for f in lv.furniture.iter().filter(|f| matches!(f.kind, "throne" | "throne_dais")) {
                if !thrones {
                    r.add("thrones outside castles and palaces", 1.0, format!("{tag} {id} ({}) {}: a {} at ({}, {})", it.function, lv.name, f.kind, f.x, f.y));
                }
            }
            if of_castle(b) {
                for f in lv.furniture.iter().filter(|f| matches!(f.kind, "wall_banner" | "standing_banner")) {
                    match f.variant {
                        Some(v) => {
                            houses.entry(v).or_insert_with(|| format!("{id} ({})", it.function));
                        }
                        None => r.add("banners in a castle not of its house's colours given", 1.0, format!("{tag} {id} {}: ({}, {})", lv.name, f.x, f.y)),
                    }
                }
            }
        }
        let zoned = interior::hub_kind(t0, l, b).is_some();
        for plan in interior::hall_plans(world, t0, li, b.id as usize) {
            hall_rules(&it, &plan, &format!("{tag} {id} ({})", it.function), zoned, r);
        }
    }
    if houses.len() > 1 {
        r.add("castles flying more than one house", houses.len() as f64, format!("{tag}: {:?}", houses));
    }
}

/// A hall furnished as a whole keeps its composition: the dais (a castle's throne dais, a low dais
/// elsewhere, none in a mess hall) at the far end of its frame on the axis, a chair of office on it, an
/// estate's high table on it; the runner on the axis; every table, bench, pillar, banner, tapestry,
/// brazier and high table mirrored across the axis (but a tapestry across from the hearth); no square
/// of its floor shut off by what blocks the way; an open hall's gallery with its musicians; an
/// audience chamber's chair of state on its dais. A long gallery's pieces along its long walls, its
/// walk clear; a chapel's altar at its far end on the axis, its pews facing it either side of the
/// aisle. `zoned`: the building is laid out by zones (an estate's hall has its dais there).
fn hall_rules(it: &interior::Interior, plan: &interior::HallPlan, tag: &str, zoned: bool, r: &mut Report) {
    use interior::hall::Style;
    let (nx, ny) = (it.nx, it.ny);
    let lv = &it.levels[plan.level];
    let f = plan.frame;
    let style = plan.style;
    let name = match style {
        Style::Castle { .. } => "castle",
        Style::Civic { .. } => "town or guild",
        Style::Estate { .. } => "estate",
        Style::Mess { .. } => "mess",
        Style::Throne { .. } => "throne room",
        Style::Banquet { .. } => "banqueting",
        Style::Ballroom => "ballroom",
        Style::Presence { .. } => "audience chamber",
        Style::Gallery => "long gallery",
        Style::Chapel => "chapel",
        // (A temple's, a shrine's and a monastery's: the `temples` rules.)
        Style::Nave { .. } | Style::Sanctuary { .. } | Style::SideChapel { .. } | Style::Shrine { .. } | Style::Refectory => return,
    };
    r.tally("halls furnished as wholes", 1);
    r.tally(&format!("  {name} halls"), 1);
    let tag = format!("{tag} {}", lv.name);
    r.add(&format!("  the biggest {name} halls (frame squares)"), (f.len * f.wid) as f64, format!("{tag}: {} x {}", f.len, f.wid));
    // (The rules for a hall big enough for them: a dais in a frame 5 long and 3 across or more, a
    // runner and tables in one 6 by 5.)
    let roomy = f.len >= 5 && f.wid >= 3;
    let big = f.len >= 6 && f.wid >= 5;
    let ri = plan.room;
    let in_room = |x: u16, y: u16| (x as usize) < nx && (y as usize) < ny && lv.cells[y as usize * nx + x as usize] == ri;
    let items: Vec<&interior::Item> = lv.furniture.iter().filter(|it| in_room(it.x, it.y)).collect();
    // An item's box in the frame: s0, s1, t0, t1 (inclusive).
    let local = |it: &interior::Item| {
        let (a, b) = (f.local(it.x as isize, it.y as isize), f.local((it.x + it.w) as isize - 1, (it.y + it.h) as isize - 1));
        (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1))
    };
    let w = f.wid as isize;
    let off_axis = |b: (isize, isize, isize, isize)| ((b.2 + b.3) as f64 * 0.5 - (w - 1) as f64 * 0.5).abs() > 0.01;
    // A long gallery: 3 squares across or more, its pieces a square deep against its long walls, a
    // way down its middle from end to end.
    if style == Style::Gallery {
        if f.wid < 3 {
            r.add("long galleries under 3 squares across", f.wid as f64, format!("{tag}: {} x {}", f.len, f.wid));
        }
        for p in &items {
            let b = local(p);
            if !(b.2 == b.3 && (b.2 == 0 || b.2 == w - 1)) {
                r.add("long gallery pieces off its long walls or across its walk", 1.0, format!("{tag}: the {} at ({}, {})", p.name, p.x, p.y));
            }
        }
        let blocked_at = |s: isize, t: isize| items.iter().any(|p| p.blocks_move && local(p).0 <= s && s <= local(p).1 && local(p).2 <= t && t <= local(p).3);
        let cut = (0..f.len as isize).filter(|&s| (1..w - 1).any(|t| blocked_at(s, t))).count();
        if f.wid >= 3 && cut > 0 {
            r.add("long galleries with their walk cut", cut as f64, format!("{tag}: {cut} rows"));
        }
        return;
    }
    // A chapel: the altar on the axis at its far end (the end away from its way in: the end wall
    // a door opens in, its main door, else its doors on the mean; either where that is midway),
    // the pews facing it (their backs to the way in) and clear of the aisle.
    if style == Style::Chapel {
        let mid = 0.5 * (f.len as f64 - 1.0);
        let m = interior::way_in(lv, nx, ny, ri, it.stairs, f).unwrap_or(mid);
        let far = |b: (isize, isize, isize, isize)| (m <= mid + 0.01 && b.1 >= f.len as isize - 1) || (m >= mid - 0.01 && b.0 <= 0);
        match items.iter().find(|p| p.kind == "altar") {
            Some(a) if far(local(a)) && !off_axis(local(a)) => {}
            _ => r.add("chapels with no altar on the axis at the far end", 1.0, format!("{tag}: a {} x {} frame, entered at {m:.1}", f.len, f.wid)),
        }
        // (The way in: the frame's -s step, as `Item::back` codes it.)
        let (gi, gj) = (-f.ds.0, -f.ds.1);
        let way_in = match (gi, gj) {
            (0, -1) => 0,
            (1, 0) => 1,
            (0, 1) => 2,
            _ => 3,
        };
        let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
        for p in items.iter().filter(|p| p.kind == "pew") {
            if p.back != Some(way_in) {
                r.add("chapel pews facing other than the altar", 1.0, format!("{tag}: the pew at ({}, {})", p.x, p.y));
            }
            let b = local(p);
            if b.2 <= cr && b.3 >= cl {
                r.add("chapel pews across the aisle", 1.0, format!("{tag}: the pew at ({}, {})", p.x, p.y));
            }
        }
        return;
    }
    let mess = matches!(style, Style::Mess { .. } | Style::Ballroom | Style::Gallery | Style::Chapel);
    let throne = matches!(style, Style::Castle { .. } | Style::Throne { .. });
    let dais_kind = if throne { "throne_dais" } else { "low_dais" };
    let daises: Vec<&&interior::Item> = items.iter().filter(|it| it.kind == dais_kind).collect();
    if mess {
        if items.iter().any(|it| matches!(it.kind, "throne_dais" | "low_dais" | "throne" | "high_table" | "chair_of_office")) {
            r.add("mess halls and ballrooms with a dais, a throne, a high table or a chair of office", 1.0, tag.clone());
        }
    } else if let Some(d) = daises.first() {
        let b = local(d);
        if b.1 < f.len as isize - 3 {
            r.add("daises not at the far end", (f.len as isize - 1 - b.1) as f64, format!("{tag}: rows {}-{} of {}", b.0, b.1, f.len));
        }
        if off_axis(b) {
            r.add("daises off the axis", 1.0, format!("{tag}: across {}-{} of {}", b.2, b.3, f.wid));
        }
    } else if roomy || throne {
        r.add(if throne { "castle halls and throne rooms with no throne dais" } else { "town, guild, estate, banqueting halls and audience chambers of 5 x 3 or more with no dais" }, 1.0, format!("{tag}: a {} x {} frame", f.len, f.wid));
        if zoned && matches!(style, Style::Estate { .. }) && roomy {
            r.add("estates' halls laid out by zones with no dais", 1.0, format!("{tag}: a {} x {} frame", f.len, f.wid));
        }
    }
    if matches!(style, Style::Presence { .. }) && !daises.is_empty() && !items.iter().any(|c| c.kind == "chair_of_office" && daises.iter().any(|d| d.x <= c.x && c.x < d.x + d.w && d.y <= c.y && c.y < d.y + d.h)) {
        r.add("audience chambers with no chair of state on their dais", 1.0, tag.clone());
    }
    let on_dais = |it: &interior::Item| {
        daises.iter().any(|d| {
            let (a, b) = (local(d), local(it));
            b.0 >= a.0 && b.1 <= a.1 && b.2 >= a.2 && b.3 <= a.3
        })
    };
    for c in items.iter().filter(|it| it.kind == "chair_of_office") {
        if !on_dais(c) {
            r.add("chairs of office not on a dais", 1.0, format!("{tag}: ({}, {})", c.x, c.y));
        }
    }
    if matches!(style, Style::Estate { .. } | Style::Banquet { .. }) {
        for t in items.iter().filter(|it| it.kind == "high_table") {
            if !on_dais(t) {
                r.add("estates' and banqueting halls' high tables not on their dais", 1.0, format!("{tag}: ({}, {})", t.x, t.y));
            }
        }
    }
    // A ballroom's dance floor and musicians on the axis.
    if style == Style::Ballroom && big {
        for kind in ["dance_floor", "musicians_corner"] {
            match items.iter().find(|it| it.kind == kind) {
                Some(it) if off_axis(local(it)) => r.add("ballrooms' dance floors or musicians off the axis", 1.0, format!("{tag}: the {}", it.name)),
                Some(_) => {}
                None => r.add("ballrooms of 6 x 5 or more with no dance floor or musicians", 1.0, format!("{tag}: no {kind} in a {} x {} frame", f.len, f.wid)),
            }
        }
    }
    if !mess && !daises.is_empty() {
        match items.iter().find(|it| it.kind == "carpet_runner") {
            Some(run) if off_axis(local(run)) => r.add("runners off the axis", 1.0, tag.clone()),
            Some(_) => {}
            None if big => r.add("halls of 6 x 5 or more with no carpet runner (castle, town, guild, estate)", 1.0, format!("{tag}: a {} x {} frame", f.len, f.wid)),
            None => {}
        }
    }
    if big && !matches!(style, Style::Throne { .. } | Style::Ballroom | Style::Presence { .. }) && !items.iter().any(|it| it.kind == "long_table") {
        r.add("halls of 6 x 5 or more with no tables", 1.0, format!("{tag} ({name}): a {} x {} frame", f.len, f.wid));
    }
    // Mirrors.
    let mirrored = ["long_table", "bench", "pillar", "wall_banner", "tapestry", "standing_banner", "brazier", "high_table", "harpsichord", "refreshment_table"];
    let hearths: Vec<(isize, isize, isize, isize)> = items.iter().filter(|it| it.kind == "hearth").map(|it| local(it)).collect();
    for it in items.iter().filter(|it| mirrored.contains(&it.kind) && !(matches!(style, Style::Estate { .. } | Style::Banquet { .. }) && it.kind == "high_table")) {
        let b = local(it);
        let m = (b.0, b.1, w - 1 - b.3, w - 1 - b.2);
        let found = items.iter().any(|o| o.kind == it.kind && local(o) == m) || (it.kind == "tapestry" && hearths.contains(&m));
        if !found {
            r.add("hall pieces with no mirror across the axis", 1.0, format!("{tag}: the {} at ({}, {})", it.name, it.x, it.y));
        }
    }
    // Pillars: the squares between each and its long wall on its row clear (no hearth, nothing hung
    // there, nothing across the way by the wall, no door's square) and none in a door's way (no
    // door's square next to it).
    let held_at = |s: isize, t: isize| items.iter().any(|o| o.kind != "pillar" && local(o).0 <= s && s <= local(o).1 && local(o).2 <= t && t <= local(o).3);
    let mut door = vec![false; nx * ny];
    for d in &lv.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        for (i, j) in if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] } {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && in_room(i as u16, j as u16) {
                door[j as usize * nx + i as usize] = true;
            }
        }
    }
    let door_at = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && door[j as usize * nx + i as usize];
    for p in items.iter().filter(|it| it.kind == "pillar") {
        let (s, t) = (local(p).0, local(p).2);
        let (i, j) = (p.x as isize, p.y as isize);
        if (if 2 * t < w - 1 { 0..t } else { t + 1..w }).any(|q| held_at(s, q) || door_at(f.grid(s, q))) || [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].into_iter().any(door_at) {
            r.add("pillars before the hearth, a hanging or a door, or across the way by a long wall", 1.0, format!("{tag}: the pillar at ({}, {})", p.x, p.y));
        }
    }
    // Nothing shut off: from the hall's ways in (its doors, the stairs), every square nothing blocking
    // stands on.
    let mut blocked = vec![false; nx * ny];
    for it in &lv.furniture {
        if it.blocks_move {
            for j in it.y as usize..(it.y + it.h) as usize {
                for i in it.x as usize..(it.x + it.w) as usize {
                    blocked[j * nx + i] = true;
                }
            }
        }
    }
    let mut starts: Vec<usize> = lv.doors.iter().flat_map(door_squares).filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).filter(|&k| lv.cells[k] == ri).collect();
    let st = it.stairs;
    if lv.has_stairs {
        starts.extend((st[1]..st[1] + st[3]).flat_map(|j| (st[0]..st[0] + st[2]).map(move |i| j * nx + i)).filter(|&k| lv.cells[k] == ri));
    }
    if let Some(&k0) = starts.first() {
        let mut seen = vec![false; nx * ny];
        let mut stack = vec![k0];
        seen[k0] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < nx && b < ny && !seen[b * nx + a] && lv.cells[b * nx + a] == ri && !blocked[b * nx + a] {
                    seen[b * nx + a] = true;
                    stack.push(b * nx + a);
                }
            }
        }
        let shut = (0..nx * ny).filter(|&k| lv.cells[k] == ri && !blocked[k] && !seen[k]).count() + starts.iter().filter(|&&k| !seen[k]).count();
        if shut > 0 {
            r.add("halls with a square shut off", shut as f64, format!("{tag}: {shut} squares"));
        }
    }
    // An open hall's gallery: its musicians (a castle's, a palace's, an estate's).
    if matches!(style, Style::Castle { .. } | Style::Estate { .. })
        && let Some(up) = it.levels.get(plan.level + 1).filter(|up| up.rooms.iter().any(|ro| ro.kind == interior::OPEN_BELOW && ro.squares > 0))
        && let Some(g) = up.rooms.iter().position(|ro| ro.kind == "minstrels' gallery")
        && !up.furniture.iter().any(|it| it.kind == "musicians_corner" && up.cells[it.y as usize * nx + it.x as usize] == g as i16)
    {
        r.add("open halls with no musicians on the gallery", 1.0, format!("{tag}: {}", up.name));
    }
}

// ---- Palaces and noble estates. ----

const PALACE_COUNTS: [&str; 31] = [
    "capital palaces not round a court",
    "capital palaces' courts under 7 squares across or deep",
    "capital palaces' fronts an even number of squares across",
    "capital palaces with no walled forecourt",
    "capital forecourts' gates on no street and with no way on to one",
    "capital forecourts' and their ways' squares over water",
    "capital palaces' royal floors whose great chamber or bedchamber isn't the biggest room but the gallery",
    "capital palaces' royal floors' wardrobes over 15 squares",
    "older palaces' great halls with no high table",
    "city palaces as big as their realm's capital's or bigger",
    "capital palaces' state floor not guard room, audience chamber, throne room",
    "capital palaces' guard rooms under 12 squares or over 40",
    "palaces' rooms of their programme twice on a floor (great chamber, bedchamber)",
    "long galleries under 3 squares across (palaces)",
    "older palaces with a ballroom or a banqueting hall",
    "manors without a kitchen of 12 squares, a buttery and a pantry of 6",
    "manors' great halls over 35% of the ground floor",
    "floors the zones fell back on",
    "breaking an interior rule (as in interiors)",
    "missing a room of their programme",
    "capital palaces laid out as older ones",
    "capital palaces' front door off the axis",
    "capital palaces' floors not mirrored across the axis (the ground floor's front range)",
    "capital palaces' state rooms not one into the next (the landing round to the throne room)",
    "capital palaces' state rooms' doors not mirrored across the axis",
    "throne rooms' dais off the palace's axis",
    "throne rooms with no throne dais",
    "rooms longer than 2.5:1 (but long galleries)",
    "rooms filling under 60% of their box",
    "rooms over 60 squares (but halls, state rooms, courts, galleries, chapels)",
    "rooms a square wide",
];

/// `palaces`: every palace and noble estate laid out by zones (`palace_rules`).
fn palaces(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["palaces and estates laid out by zones"]);
    r.counts(&PALACE_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    // (The capital's palace's floor and the biggest other palace's (sq ft), with where it stands.)
    let (mut capital, mut city) = (None::<f64>, None::<(f64, String)>);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        for b in &l.buildings {
            if town::palace::is_palace(b) && b.structure == town::Structure::Roofed {
                let a = town::geom::area(&b.poly).abs();
                if t0.settlements.get(li).is_some_and(|s| s.capital) {
                    capital = Some(capital.map_or(a, |c| c.max(a)));
                } else if city.as_ref().is_none_or(|c| a > c.0) {
                    city = Some((a, format!("b:{li}:{}", b.id)));
                }
            }
            let Some(kind) = interior::hub_kind(t0, &l, b).filter(|k| !k.is_keep() && !k.is_faith() && !k.is_civic() && !k.is_inn()) else { continue };
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            palace_rules(world, t0, &l, li, b, kind, buckets, r);
        }
    }
    if let (Some(c), Some((a, id))) = (capital, city)
        && a >= c
    {
        r.add("city palaces as big as their realm's capital's or bigger", a / c, format!("{id}: {a:.0} sq ft, the capital's {c:.0}"));
    }
}

/// The rules a palace or a noble estate laid out by zones keeps (`palaces`).
#[allow(clippy::too_many_arguments)]
fn palace_rules(world: &World, t0: &T0, l: &Layout, li: usize, b: &town::Building, kind: interior::hub::Kind, buckets: &HashMap<(i64, i64), Vec<usize>>, r: &mut Report) {
    use interior::hub::Kind;
    let id = format!("b:{li}:{}", b.id);
    let Some(it) = interior::generate_id(world, t0, &id) else { return };
    r.tally("palaces and estates laid out by zones", 1);
    r.tally(&format!("  {}s", kind.name()), 1);
    let tag = format!("{id} ({})", kind.name());
    let (nx, ny) = (it.nx, it.ny);
    let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
    if !fell.is_empty() {
        r.add("floors the zones fell back on", fell.len() as f64, format!("{tag}: z {fell:?}"));
    }
    let bad = check_interior(world, t0, l, li, &id, &it, buckets);
    if !bad.is_empty() {
        r.add("breaking an interior rule (as in interiors)", bad.len() as f64, format!("{tag}: {}", bad.join("; ")));
    }
    // The programme.
    let has = |k: &str| it.levels.iter().any(|lv| lv.rooms.iter().any(|ro| ro.kind == k && ro.squares > 0));
    let plan = interior::zone_plan_of(world, t0, li, b.id as usize);
    let court = plan.as_ref().and_then(|p| p.palace()).is_some_and(|z| z.3);
    let squares = interior::Shell::of(l, b).inside.iter().filter(|&&i| i).count();
    let want: Vec<&str> = match kind {
        Kind::Palace => {
            let mut w = vec!["entrance hall", "guardroom", "throne room", "audience chamber", "ballroom", "banqueting hall", "royal chapel", "kitchen", "antechamber"];
            if b.floors >= 3 {
                w.push("long gallery");
            }
            if court {
                w.push("courtyard");
            }
            w
        }
        Kind::OldPalace => {
            let mut w = vec!["great hall", "guardroom", "chapel", "audience chamber", "solar", "kitchen"];
            if b.floors >= 3 {
                w.push("long gallery");
            }
            w
        }
        Kind::Manor => {
            let mut w = vec!["great hall", "kitchen", "parlor", "stair hall", "great chamber", "solar"];
            if squares >= 260 {
                w.push("chapel");
            }
            w
        }
        _ => vec!["great hall", "kitchen", "parlor", "solar"],
    };
    let missing: Vec<&str> = want.iter().copied().filter(|k| !has(k)).collect();
    if !missing.is_empty() {
        r.add("missing a room of their programme", missing.len() as f64, format!("{tag}: no {}", missing.join(", ")));
    }
    // Rooms of sound proportions.
    let big = |k: &str| matches!(k, "great hall" | "banqueting hall" | "throne room" | "ballroom" | "audience chamber" | "courtyard" | "royal chapel" | "chapel" | "entrance hall" | "long gallery");
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        for (ri, ro) in lv.rooms.iter().enumerate() {
            if ro.squares == 0 || !interior::walkable(ro.kind) || (interior::is_circulation(ro.kind) && ro.kind != "long gallery") {
                continue;
            }
            let sq: Vec<usize> = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let named = format!("{tag} {}: the {} ({} squares, {w} x {h})", lv.name, ro.kind, sq.len());
            if ro.kind != "long gallery" && 2 * w.max(h) > 5 * w.min(h) {
                r.add("rooms longer than 2.5:1 (but long galleries)", w.max(h) as f64 / w.min(h) as f64, named.clone());
            }
            if sq.len() * 5 < w * h * 3 {
                r.add("rooms filling under 60% of their box", sq.len() as f64 / (w * h) as f64, named.clone());
            }
            if sq.len() > 60 && !big(ro.kind) {
                r.add("rooms over 60 squares (but halls, state rooms, courts, galleries, chapels)", sq.len() as f64, named.clone());
            }
            let wide = sq.iter().any(|&k| k % nx + 1 < nx && [k + 1, k + nx, k + nx + 1].iter().all(|q| sq.contains(q)));
            if !wide && sq.len() >= 2 {
                r.add("rooms a square wide", sq.len() as f64, named);
            }
        }
    }
    // Long galleries 3 squares across or more; a palace's great chamber and bedchamber once a floor.
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, ro)| ro.kind == "long gallery" && ro.squares > 0) {
            let sq: Vec<usize> = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16).collect();
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            let across = (x1 - x0 + 1).min(y1 - y0 + 1);
            if across < 3 {
                r.add("long galleries under 3 squares across (palaces)", across as f64, format!("{tag} {}: {} x {}", lv.name, x1 - x0 + 1, y1 - y0 + 1));
            }
        }
        if matches!(kind, Kind::Palace | Kind::OldPalace) {
            for k in ["great chamber", "bedchamber"] {
                let n = lv.rooms.iter().filter(|ro| ro.kind == k && ro.squares > 0).count();
                if n > 1 {
                    r.add("palaces' rooms of their programme twice on a floor (great chamber, bedchamber)", n as f64, format!("{tag} {}: {n} {k}s", lv.name));
                }
            }
        }
    }
    match kind {
        Kind::OldPalace => {
            if has("ballroom") || has("banqueting hall") {
                r.add("older palaces with a ballroom or a banqueting hall", 1.0, tag.clone());
            }
            // (Its great hall serves as the banqueting hall: its high table.)
            for lv in &it.levels {
                for (ri, _) in lv.rooms.iter().enumerate().filter(|(_, ro)| ro.kind == "great hall" && ro.squares > 0) {
                    if !lv.furniture.iter().any(|f| f.kind == "high_table" && lv.cells[f.y as usize * nx + f.x as usize] == ri as i16) {
                        r.add("older palaces' great halls with no high table", 1.0, format!("{tag} {}", lv.name));
                    }
                }
            }
        }
        Kind::Manor => {
            // A hall with a service wing: kitchen, buttery and pantry of sound size, the hall a third
            // of the ground floor or so.
            if let Some(g) = it.levels.iter().find(|lv| lv.z == 0) {
                let size = |k: &str| g.rooms.iter().filter(|ro| ro.kind == k).map(|ro| ro.squares as usize).max().unwrap_or(0);
                if size("kitchen") < 12 || size("buttery") < 6 || size("pantry") < 6 {
                    r.add("manors without a kitchen of 12 squares, a buttery and a pantry of 6", 1.0, format!("{tag}: kitchen {}, buttery {}, pantry {}", size("kitchen"), size("buttery"), size("pantry")));
                }
                let all: usize = g.rooms.iter().map(|ro| ro.squares as usize).sum();
                let hall = size("great hall");
                if hall * 100 > 35 * all {
                    r.add("manors' great halls over 35% of the ground floor", hall as f64 / all as f64, format!("{tag}: {hall} of {all} squares"));
                }
            }
        }
        _ => {}
    }
    if kind != Kind::Palace {
        return;
    }
    // The capital's palace: laid out as one, mirrored across its axis.
    let Some((f, (front, _, _, _))) = plan.as_ref().and_then(|p| p.palace().map(|z| (p.frame(), z))) else {
        r.add("capital palaces laid out as older ones", 1.0, format!("{tag}: {}", plan.as_ref().map_or("no plan", |p| p.kind().name())));
        return;
    };
    if plan.as_ref().and_then(|p| p.palace()).is_some_and(|z| z.3) {
        r.tally("  capital palaces round a court", 1);
    } else {
        r.add("capital palaces not round a court", 1.0, tag.clone());
    }
    if let Some((_, back, side, true)) = plan.as_ref().and_then(|p| p.palace()) {
        let (across, deep) = (f.wid - 2 * side, back - front);
        if across < 7 || deep < 7 {
            r.add("capital palaces' courts under 7 squares across or deep", across.min(deep) as f64, format!("{tag}: {across} x {deep}"));
        }
    }
    if f.wid % 2 == 0 {
        r.add("capital palaces' fronts an even number of squares across", f.wid as f64, tag.clone());
    }
    if interior::court_plan(world, t0, l, b).is_none_or(|c| c.forecourt.is_none()) {
        r.add("capital palaces with no walled forecourt", 1.0, tag.clone());
    }
    // (Its gate on a street, else a paved way on to one.)
    if let Some(fc) = l.forecourts.iter().find(|fc| fc.palace == b.id && fc.yard.len() == 4) {
        let m = [0.5 * (fc.yard[0][0] + fc.yard[3][0]), 0.5 * (fc.yard[0][1] + fc.yard[3][1])];
        let on = l.roads.iter().any(|(pts, _, w)| pts.windows(2).any(|s| town::geom::seg_dist(m, s[0], s[1]) <= 0.5 * w + 1.0));
        if fc.way.is_empty() && !on {
            r.add("capital forecourts' gates on no street and with no way on to one", 1.0, tag.clone());
        }
        // (Its yard and its paved way on dry ground: the squares whose middles are in the yard or
        // within 6 ft of the way's centre line, on the water.)
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
        let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
        let bb = bbox(&fc.yard.iter().chain(&fc.way).copied().collect::<Vec<P>>());
        let (mut yard, mut way) = (0usize, 0usize);
        for j in (libm::floor(bb[1] / 5.0) as i64 - 2)..=(libm::floor(bb[3] / 5.0) as i64 + 2) {
            for i in (libm::floor(bb[0] / 5.0) as i64 - 2)..=(libm::floor(bb[2] / 5.0) as i64 + 2) {
                let c = [i as f64 * 5.0 + 2.5, j as f64 * 5.0 + 2.5];
                if town::geom::contains(&fc.yard, c) {
                    yard += wet(c) as usize;
                } else if fc.way.windows(2).any(|s| town::geom::seg_dist(c, s[0], s[1]) < 6.0) {
                    way += wet(c) as usize;
                }
            }
        }
        if yard + way > 0 {
            r.add("capital forecourts' and their ways' squares over water", (yard + way) as f64, format!("{tag}: yard {yard}, way {way}"));
        }
    }
    // The royal floor: the great chamber and the bedchamber its biggest rooms but the gallery, the
    // wardrobes small.
    if let Some(lv) = it.levels.iter().find(|lv| lv.name == "Royal floor") {
        let size = |k: &str| lv.rooms.iter().filter(|ro| ro.kind == k).map(|ro| ro.squares as usize).max().unwrap_or(0);
        let royal = size("great chamber").min(size("bedchamber"));
        let other = lv.rooms.iter().filter(|ro| !matches!(ro.kind, "great chamber" | "bedchamber" | "long gallery") && !interior::is_circulation(ro.kind) && interior::walkable(ro.kind)).map(|ro| ro.squares as usize).max().unwrap_or(0);
        if royal <= other {
            r.add("capital palaces' royal floors whose great chamber or bedchamber isn't the biggest room but the gallery", royal as f64, format!("{tag}: {royal} squares, another {other}"));
        }
        if let Some(w) = lv.rooms.iter().filter(|ro| ro.kind == "wardrobe" && ro.squares > 15).map(|ro| ro.squares).max() {
            r.add("capital palaces' royal floors' wardrobes over 15 squares", w as f64, format!("{tag}: {w} squares"));
        }
    }
    // Its guard rooms of sound size; on the state floor the throne room reached from the landing
    // only through a guard room, then the audience chamber (or the ballroom across the axis), then a
    // guard room again.
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        for ro in lv.rooms.iter().filter(|ro| ro.kind == "guardroom" && ro.squares > 0) {
            if ro.squares < 12 || ro.squares > 40 {
                r.add("capital palaces' guard rooms under 12 squares or over 40", ro.squares as f64, format!("{tag} {}", lv.name));
            }
        }
    }
    if let Some(lv) = it.levels.iter().find(|lv| lv.z == 1) {
        let room_of = |k: &str| (0..lv.rooms.len()).find(|&ri| lv.rooms[ri].kind == k && lv.rooms[ri].squares > 0).map(|ri| ri as i16);
        let state = |k: &str| matches!(k, "landing" | "guardroom" | "antechamber" | "audience chamber" | "ballroom" | "throne room");
        // (The rooms reached from the landing through state rooms, but those of kinds `skip`.)
        let reach = |skip: &[&str]| {
            let mut seen = vec![false; lv.rooms.len()];
            let mut stack: Vec<i16> = room_of("landing").into_iter().collect();
            for &s in &stack {
                seen[s as usize] = true;
            }
            while let Some(a) = stack.pop() {
                for d in lv.doors.iter().filter(|d| d.rooms.contains(&a)) {
                    let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
                    if o >= 0 && !seen[o as usize] && state(lv.rooms[o as usize].kind) && !skip.contains(&lv.rooms[o as usize].kind) {
                        seen[o as usize] = true;
                        stack.push(o);
                    }
                }
            }
            seen
        };
        let throne = room_of("throne room");
        let guarded = throne.is_some_and(|t| lv.doors.iter().any(|d| d.rooms.contains(&t) && d.rooms.iter().any(|&o| o >= 0 && lv.rooms[o as usize].kind == "guardroom")));
        let ok = guarded
            && room_of("audience chamber").is_some_and(|a| !reach(&["guardroom"])[a as usize])
            && throne.is_some_and(|t| !reach(&["audience chamber", "ballroom"])[t as usize] && reach(&[])[t as usize]);
        if !ok {
            r.add("capital palaces' state floor not guard room, audience chamber, throne room", 1.0, tag.clone());
        }
    }
    let w = f.wid as isize;
    let mirror = |(i, j): (isize, isize)| {
        let (s, t) = f.local(i, j);
        f.grid(s, w - 1 - t)
    };
    let cell = |lv: &interior::Level, (i, j): (isize, isize)| if i < 0 || j < 0 || i as usize >= nx || j as usize >= ny { -1 } else { lv.cells[j as usize * nx + i as usize] };
    if let Some(g0) = it.levels.iter().find(|lv| lv.z == 0)
        && let Some(d) = g0.doors.iter().find(|d| d.kind == "front")
    {
        let sq = door_squares(d);
        let inner = sq.iter().copied().find(|&p| cell(g0, p) >= 0).unwrap_or(sq[0]);
        let (_, t) = f.local(inner.0, inner.1);
        if (2 * t - (w - 1)).abs() > 1 {
            r.add("capital palaces' front door off the axis", 1.0, format!("{tag}: across {t} of {w}"));
        }
    }
    for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
        // (Each square's room and its mirror's: one room to one room.)
        let (mut to, mut from): (HashMap<i16, i16>, HashMap<i16, i16>) = (HashMap::new(), HashMap::new());
        let mut broken = 0;
        for k in 0..nx * ny {
            let p = ((k % nx) as isize, (k / nx) as isize);
            let (s, _) = f.local(p.0, p.1);
            if lv.z == 0 && s >= front as isize {
                continue;
            }
            let (a, m) = (lv.cells[k], cell(lv, mirror(p)));
            if (a < 0) != (m < 0) {
                broken += 1;
                continue;
            }
            if a < 0 {
                continue;
            }
            if *to.entry(a).or_insert(m) != m || *from.entry(m).or_insert(a) != a {
                broken += 1;
            }
        }
        if broken > 0 {
            r.add("capital palaces' floors not mirrored across the axis (the ground floor's front range)", broken as f64, format!("{tag} {}: {broken} squares", lv.name));
        }
    }
    // The state floor: from the landing round to the throne room through the state rooms, both ways.
    if let Some(lv) = it.levels.iter().find(|lv| lv.z == 1) {
        let state = |k: &str| matches!(k, "landing" | "guardroom" | "antechamber" | "audience chamber" | "ballroom" | "throne room");
        let room_of = |k: &str| (0..lv.rooms.len()).find(|&ri| lv.rooms[ri].kind == k && lv.rooms[ri].squares > 0).map(|ri| ri as i16);
        let reach = |skip: Option<i16>| {
            let mut seen = vec![false; lv.rooms.len()];
            let mut stack: Vec<i16> = room_of("landing").into_iter().collect();
            for &s in &stack {
                seen[s as usize] = true;
            }
            while let Some(a) = stack.pop() {
                for d in lv.doors.iter().filter(|d| d.rooms.contains(&a)) {
                    let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
                    if o >= 0 && !seen[o as usize] && Some(o) != skip && state(lv.rooms[o as usize].kind) {
                        seen[o as usize] = true;
                        stack.push(o);
                    }
                }
            }
            seen
        };
        let throne = room_of("throne room");
        let ok = throne.is_some_and(|t| ["audience chamber", "ballroom"].iter().all(|side| room_of(side).is_some_and(|s| reach(room_of(if *side == "ballroom" { "audience chamber" } else { "ballroom" }))[t as usize] && reach(None)[s as usize])));
        if !ok {
            r.add("capital palaces' state rooms not one into the next (the landing round to the throne room)", 1.0, format!("{tag} {}", lv.name));
        }
        for d in lv.doors.iter().filter(|d| d.rooms.iter().all(|&ro| ro >= 0 && state(lv.rooms[ro as usize].kind))) {
            let sq = door_squares(d);
            let m = [mirror(sq[0]), mirror(sq[1])];
            if !lv.doors.iter().any(|o| {
                let q = door_squares(o);
                (q[0] == m[0] && q[1] == m[1]) || (q[0] == m[1] && q[1] == m[0])
            }) {
                r.add("capital palaces' state rooms' doors not mirrored across the axis", 1.0, format!("{tag} {}: the door at ({}, {})", lv.name, d.a[0], d.a[1]));
            }
        }
        match lv.furniture.iter().find(|it| it.kind == "throne_dais") {
            Some(dais) => {
                let (a, c) = (f.local(dais.x as isize, dais.y as isize), f.local((dais.x + dais.w) as isize - 1, (dais.y + dais.h) as isize - 1));
                if a.1 + c.1 != w - 1 {
                    r.add("throne rooms' dais off the palace's axis", 1.0, format!("{tag}: across {}-{} of {w}", a.1.min(c.1), a.1.max(c.1)));
                }
            }
            None => r.add("throne rooms with no throne dais", 1.0, tag.clone()),
        }
    }
}

// ---- The street editor. ----

/// Castles set in the street editor at the town wall, walls on and off.
fn editor(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&[
        "placements",
        "placements with no castle",
        "placements with any non-castle building changed",
        "non-castle buildings changed or gone, walls off against on",
        "new non-castle ids with the walls off",
        "towns with a castle, walls taken down",
        "their non-castle buildings changed or gone",
    ]);
    r.counts(&["castles", "castles with an inner curtain", "gate ways"]);
    r.counts(&CASTLE_COUNTS);
    r.counts(&CASTLE_MORE[..4]);
    r.counts(&["buildings laid out by zones"]);
    r.counts(&HUB_COUNTS);
    r.counts(&["halls furnished as wholes"]);
    r.counts(&HALL_COUNTS);
    r.counts(&MONASTERY_EDIT_COUNTS);
    let places: usize = std::env::var("PLACES").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let with = |ci: usize, edit: worldgen::world::TownEdit| {
        let mut file = base.file.clone();
        file.edits.towns.insert(town::wards::key(ci), edit);
        World::new(file).unwrap()
    };
    let n = t0.settlements.len();
    let mut placed = 0;
    for ci in (0..n).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        let g = town::layout(base, t0, ci);
        // Every town castle's town with its walls taken down: no other building changes.
        if !g.castles.is_empty() && !g.walls.is_empty() {
            if let Ok((Some(edit), _)) = town::wards::change(base, t0, ci, &serde_json::from_str(r#"{"walls":false}"#).unwrap()) {
                let off = town::layout(&with(ci, edit), t0, ci);
                r.tally("towns with a castle, walls taken down", 1);
                same_buildings(&g, &off, &format!("#{ci} (its castle, walls down)"), "their non-castle buildings changed or gone", r);
            }
        }
        if placed >= places || !g.castles.is_empty() || g.walls.is_empty() {
            continue;
        }
        let Ok(plan) = town::wards::plan_json(base, t0, ci) else { continue };
        // The patches in town nearest the town wall (by their middle), up to 3, 250 ft apart.
        let pt = |v: &serde_json::Value| [v["at"][0].as_f64().unwrap(), v["at"][1].as_f64().unwrap()];
        let to_wall = |p: P| g.walls.iter().flat_map(|w| w.windows(2)).map(|s| seg_dist(p, s[0], s[1])).fold(f64::MAX, f64::min);
        let mut near: Vec<&serde_json::Value> = plan["patches"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["in_town"] == true && matches!(p["ward"].as_str(), Some("common" | "craft" | "merchant" | "patriciate" | "slum")))
            .collect();
        near.sort_by(|a, b| to_wall(pt(a)).total_cmp(&to_wall(pt(b))));
        let mut picked: Vec<&serde_json::Value> = Vec::new();
        for p in near {
            if picked.len() < 3 && picked.iter().all(|q| dist(pt(q), pt(p)) > 250.0) {
                picked.push(p);
            }
        }
        for patch in picked {
            let mut both = Vec::new();
            for walls in ["", r#","walls":false"#] {
                let req = format!(r#"{{"patches":[{{"patch":{},"ward":"castle"}}]{walls}}}"#, patch["patch"]);
                let Ok((Some(edit), _)) = town::wards::change(base, t0, ci, &serde_json::from_str(&req).unwrap()) else { continue };
                let world = with(ci, edit);
                let l = town::layout(&world, t0, ci);
                let tag = format!("#{ci} p{} walls {}", patch["patch"], if walls.is_empty() { "on" } else { "off" });
                if l.castles.is_empty() {
                    r.add("placements with no castle", 1.0, format!("{tag}: no castle"));
                } else {
                    castle_checks(&world, t0, &l, &tag, false, r);
                    // Its keep and barracks as the castle's laid out by zones, and their halls.
                    let buckets = building_buckets(&l);
                    for b in l.buildings.iter().filter(|b| of_castle(b)) {
                        keep_rules(&world, t0, &l, ci, b, &format!("{tag} "), &buckets, r);
                    }
                    layout_halls(&world, t0, &l, ci, &tag, &of_castle, r);
                }
                both.push(l);
            }
            if both.len() == 2 {
                placed += 1;
                r.tally("placements", 1);
                let tag = format!("#{ci} p{}", patch["patch"]);
                let changed = same_buildings(&both[0], &both[1], &tag, "non-castle buildings changed or gone, walls off against on", r);
                let new = both[1].buildings.iter().filter(|y| y.ward != Ward::Castle && !both[0].buildings.iter().any(|x| x.id == y.id)).count();
                if new > 0 {
                    r.add("new non-castle ids with the walls off", new as f64, format!("{tag}: {new} new ids"));
                }
                if changed + new > 0 {
                    r.add("placements with any non-castle building changed", (changed + new) as f64, format!("{tag}: {changed} changed or gone, {new} new"));
                }
            }
        }
    }
    monastery_edits(base, t0, r);
}

/// `monastery_edits`' counts, in order.
const MONASTERY_EDIT_COUNTS: [&str; 6] = [
    "monastery edits",
    "coaching inn edits",
    "coaching inns no longer round a yard after an edit of their own patch or a far one",
    "buildings on another's ground (`Layout::grounds`), as generated",
    "buildings on another's ground after an edit",
    "buildings by a monastery with no front door after an edit",
];

/// Every monastery's town edited round it (`wards::change`: its patch's lots made small, its patch
/// rerolled, and a far edit: the walls taken down, else the patch farthest from it rerolled): no
/// building on another's ground (`Layout::grounds`, by more than a foot), and every building within
/// 150 ft of it with a front door.
fn monastery_edits(base: &World, t0: &T0, r: &mut Report) {
    let on_ground = |l: &Layout, tag: &str, label: &str, r: &mut Report| {
        for (id, g) in &l.grounds {
            let (small, gb) = (shrink(g, 1.0), bbox(g));
            for b in l.buildings.iter().filter(|b| b.id != *id) {
                let bb = bbox(&b.poly);
                if bb[0] > gb[2] || bb[2] < gb[0] || bb[1] > gb[3] || bb[3] < gb[1] || !poly_overlap(&shrink(&b.poly, 1.0), &small) {
                    continue;
                }
                r.add(label, 1.0, format!("{tag}: building {} on the ground of {id} at {}", b.id, at(centroid(&b.poly))));
            }
        }
    };
    let patch_at = |plan: &serde_json::Value, p: P, far: bool| -> Option<u64> {
        let at = |v: &serde_json::Value| [v["at"][0].as_f64().unwrap(), v["at"][1].as_f64().unwrap()];
        let by = plan["patches"].as_array()?.iter().filter(|q| q["in_town"] == true);
        let q = if far { by.max_by(|a, b| dist(at(a), p).total_cmp(&dist(at(b), p))) } else { by.min_by(|a, b| dist(at(a), p).total_cmp(&dist(at(b), p))) };
        q?["patch"].as_u64()
    };
    for li in 0..town::layout_count(t0) {
        let g = town::layout(base, t0, li);
        on_ground(&g, &format!("#{li}"), "buildings on another's ground (`Layout::grounds`), as generated", r);
        let mons: Vec<&town::Building> = g.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && (b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == "monastery") || town::palace::is_coaching(&g, b))).collect();
        if mons.is_empty() {
            continue;
        }
        let Ok(plan) = town::wards::plan_json(base, t0, li) else { continue };
        for m in mons {
            let c = centroid(&m.poly);
            let Some(own) = patch_at(&plan, c, false) else { continue };
            let mut reqs = vec![format!(r#"{{"patches":[{{"patch":{own},"lots":"small"}}]}}"#), format!(r#"{{"patches":[{{"patch":{own},"reroll":true}}]}}"#)];
            if !g.walls.is_empty() {
                reqs.push(r#"{"walls":false}"#.to_string());
            } else if let Some(far) = patch_at(&plan, c, true).filter(|&q| q != own) {
                reqs.push(format!(r#"{{"patches":[{{"patch":{far},"reroll":true}}]}}"#));
            }
            for req in reqs {
                let Ok((Some(edit), _)) = town::wards::change(base, t0, li, &serde_json::from_str(&req).unwrap()) else { continue };
                let mut file = base.file.clone();
                file.edits.towns.insert(town::wards::key(li), edit);
                let world = World::new(file).unwrap();
                let l = town::layout(&world, t0, li);
                let tag = format!("b:{li}:{} {req}", m.id);
                if town::palace::is_coaching(&g, m) {
                    r.tally("coaching inn edits", 1);
                    if !l.buildings.iter().any(|b| b.id == m.id && town::palace::is_coaching(&l, b) && interior::zone_plan_of(&world, t0, li, b.id as usize).and_then(|p| p.yard()).is_some()) {
                        let now = l.buildings.iter().find(|b| b.id == m.id);
                        let why = match now {
                            None => "gone".to_string(),
                            Some(b) if !town::palace::is_coaching(&l, b) => format!("not merged ({:?}, {} corners, as generated: {})", b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.poly.len(), b.poly == m.poly),
                            Some(b) => format!("no yard frame (as generated: {})", b.poly == m.poly),
                        };
                        r.add("coaching inns no longer round a yard after an edit of their own patch or a far one", 1.0, format!("{tag} at {}: {why}", at(c)));
                    }
                } else {
                    r.tally("monastery edits", 1);
                }
                on_ground(&l, &tag, "buildings on another's ground after an edit", r);
                for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && dist(centroid(&b.poly), c) < 150.0) {
                    let Some(it) = interior::generate(&world, t0, li, b.id as usize) else { continue };
                    if !it.levels.iter().any(|lv| lv.doors.iter().any(|d| d.kind == "front")) {
                        r.add("buildings by a monastery with no front door after an edit", 1.0, format!("{tag}: b:{li}:{} at {}", b.id, at(centroid(&b.poly))));
                    }
                }
            }
        }
    }
}

/// Non-castle buildings of `a` by id in `b`: the same outline, ward, function, kind, name and floors.
/// Returns how many changed or went.
fn same_buildings(a: &Layout, b: &Layout, tag: &str, label: &str, r: &mut Report) -> usize {
    let by_id: HashMap<u32, &town::Building> = b.buildings.iter().map(|y| (y.id, y)).collect();
    let mut n = 0;
    for x in a.buildings.iter().filter(|x| x.ward != Ward::Castle) {
        let what = match by_id.get(&x.id) {
            Some(y) if y.poly.len() == x.poly.len() && y.poly.iter().zip(&x.poly).all(|(p, q)| dist(*p, *q) < 1e-3) && y.ward == x.ward && y.func == x.func && y.residential == x.residential && y.name == x.name && y.floors == x.floors => continue,
            Some(y) => format!("{:?}/{:?} -> {:?}/{:?}, moved {:.1} ft", x.ward, x.func, y.ward, y.func, dist(centroid(&x.poly), centroid(&y.poly))),
            None => "gone".into(),
        };
        n += 1;
        r.add(label, 1.0, format!("{tag}: id {} {what} at {}", x.id, at(centroid(&x.poly))));
    }
    n
}

// ---- Temples, shrines and monasteries. ----

/// The `temples` rules' counts, in order.
const TEMPLE_COUNTS: [&str; 34] = [
    "temples left on their lots (too small for a hall with an apse)",
    "temples, shrines and monasteries laid out by zones: floors fallen back on",
    "big temples whose footprint is no cross with a round apse",
    "small temples whose footprint is no hall with a round apse",
    "temples missing a room of their programme (or its apse)",
    "temples' front doors not into the nave",
    "temples with no altar rail between the nave and the sanctuary or a side chapel",
    "temples' god's statues not on the axis at the far end",
    "temples with no altar before the god's statue",
    "temples whose candle stands by the altar are missing or not mirrored",
    "big temples whose arcades are missing or not mirrored",
    "big temples whose crossing lacks its four piers",
    "big temples with no carved screen across the nave from pier to gate before the crossing, or its gate shut or missing",
    "temples whose pews are missing, not mirrored (but by the stairs) or not facing the altar",
    "temples with no font",
    "side chapels with no side altar against the far wall",
    "vestries with nothing in them",
    "temples' pieces not in their god's look",
    "shrines not of one room",
    "shrines with no god's statue or altar",
    "monasteries not round a cloister (kept their lot)",
    "monasteries missing a room round the walk",
    "monasteries' rooms opening neither onto the walk nor onto a room that does",
    "monasteries whose chapter house is not drawn as an octagon",
    "monasteries whose church is not along a long side",
    "monasteries whose garth is not open to the sky on the map",
    "monasteries with objects scattered in the garth",
    "monasteries with no cloister well or walks in the garth",
    "monasteries whose refectory has no high table or long tables",
    "monasteries with another building on their footprint, or no arcade round the garth",
    "temple, shrine and monastery rooms the designer's Furnish lays out otherwise than generation",
    "temples, shrines and monasteries whose front door opens onto ground that reaches no street (in 50 ft)",
    "temple, shrine and monastery rooms with squares their furniture shuts off",
    "monasteries with a crypt beyond their church (and the foot of its stairs)",
];

/// `temples`: temples, shrines and monasteries as laid out (`interior::hub`'s `temple` and
/// `monastery`, `town::temple`): no floor fallen back on the shared layout; a big temple's
/// footprint a cross (its transept wider than its body both ways) ending in an apse beyond the body,
/// a small one's a hall ending in one; the rooms of each programme (a big one's nave, sanctuary in
/// its apse, two side chapels and a vestry; a small one's single hall in its apse), the front door
/// into the nave, an altar rail between the nave and the sanctuary and each side chapel; the god's
/// statue on the axis at the far end, the altar before it, candle stands mirrored by it; a big nave's
/// arcades mirrored, the crossing's four piers, the carved screen on the row before the crossing
/// mirrored with its gate on the axis clear; pews mirrored, facing the altar; the font; each side
/// chapel's altar against its far wall; the vestry furnished; every temple piece in the god's look
/// (`town::temple::god_look`); a shrine of one room with the god's statue and an altar; a monastery
/// round its cloister on its merged footprint (nothing else on it), its church along a long side, the
/// chapter house (drawn as an octagon), refectory, dorter, scriptorium and kitchen round the walk (each
/// opening onto it or onto a room that does), the garth open to the sky on the map with nothing
/// scattered in it but its well, its cloister well and walks inside, the arcade round it, the refectory
/// furnished as a whole (its high table and long tables); every front door onto ground that reaches a
/// street on the battlemap (within 50 ft), every square of every room no piece blocks reached from
/// its doors and stairs; a monastery's crypt under its church only.
fn temples(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&TEMPLE_COUNTS);
    let n = town::layout_count(&ctx.ex.t0);
    for li in 0..n {
        let l = town::layout(&ctx.ex.world, &ctx.ex.t0, li);
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
            if !matches!(key, Some("temple" | "shrine" | "monastery")) {
                continue;
            }
            temple_rules(ctx, &l, li, b, key.unwrap_or_default(), r);
        }
    }
}

#[allow(clippy::too_many_lines)]
fn temple_rules(ctx: &mut Ctx, l: &Layout, li: usize, b: &town::Building, key: &str, r: &mut Report) {
    use interior::hall::Style;
    use interior::hub::Kind;
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let id = format!("b:{li}:{}", b.id);
    let kind = interior::hub_kind(t0, l, b);
    let tag = format!("{id} ({})", kind.map_or(key, |k| k.name()));
    let at_b = at(centroid(&b.poly));
    r.tally(&format!("  {}s", if key == "shrine" && kind.is_some() { "shrine hall" } else { kind.map_or(key, |k| k.name()) }), 1);
    if key == "temple" && kind.is_none() {
        r.add(TEMPLE_COUNTS[0], area(&b.poly).abs(), format!("{tag} at {at_b}: {:.0} sq ft", area(&b.poly).abs()));
    }
    if key == "monastery" && kind != Some(Kind::Monastery) {
        r.add(TEMPLE_COUNTS[20], area(&b.poly).abs(), format!("{tag} at {at_b}: {:.0} sq ft", area(&b.poly).abs()));
    }
    let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
    if !fell.is_empty() {
        r.add(TEMPLE_COUNTS[1], fell.len() as f64, format!("{tag}: z {fell:?}"));
    }
    let Some(it) = interior::generate_id(world, t0, &id) else { return };
    let (nx, ny) = (it.nx, it.ny);
    let Some(gi) = it.levels.iter().position(|lv| lv.z == 0) else { return };
    let g = &it.levels[gi];
    let room_at = |lv: &interior::Level, (i, j): (isize, isize)| if i < 0 || j < 0 || i as usize >= nx || j as usize >= ny { -1 } else { lv.cells[j as usize * nx + i as usize] };
    // The front door's outside on the battlemap: open ground reached from a street (or 50 ft off).
    if let Some(d) = g.doors.iter().find(|d| d.kind == "front")
        && let Some(o) = door_squares(d).into_iter().find(|&p| room_at(g, p) < 0)
    {
        let p = interior::to_world(&it, o.0 as f64 + 0.5, o.1 as f64 + 0.5);
        let bb = bbox(&b.poly);
        let w = ctx.window([bb[0] - 50.0, bb[1] - 50.0, bb[2] + 50.0, bb[3] + 50.0]);
        let seen = w.flood(|k| w.street[k] || w.edge(k));
        let ok = w.index(p).is_some_and(|k| (!w.block[k] && seen[k]) || (w.block[k] && w.around(k).any(|m| seen[m] && !contains(&b.poly, w.centre(m)))));
        if !ok {
            r.add(TEMPLE_COUNTS[31], 1.0, format!("{tag}: its door's outside at {}", at(p)));
        }
    }
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    // Every square of every room no piece blocks reached from the stairs and the front door, through
    // doors.
    for (li, lv) in it.levels.iter().enumerate() {
        let mut blocked = vec![false; nx * ny];
        for f in lv.furniture.iter().filter(|f| f.blocks_move) {
            for j in f.y as usize..(f.y + f.h) as usize {
                for i in f.x as usize..(f.x + f.w) as usize {
                    blocked[j * nx + i] = true;
                }
            }
        }
        let st = it.stairs;
        let mut starts: Vec<usize> = Vec::new();
        if lv.has_stairs {
            starts.extend((st[1]..st[1] + st[3]).flat_map(|j| (st[0]..st[0] + st[2]).map(move |i| j * nx + i)).filter(|&k| lv.cells[k] >= 0));
        }
        if li == it.entry_level {
            for d in lv.doors.iter().filter(|d| d.kind == "front") {
                starts.extend(door_squares(d).into_iter().filter(|&p| room_at(lv, p) >= 0).map(|p| p.1 as usize * nx + p.0 as usize));
            }
        }
        starts.extend(lv.furniture.iter().filter(|f| f.kind == "spiral_stair").map(|f| f.y as usize * nx + f.x as usize));
        let door_between = |a: usize, c: usize| lv.doors.iter().any(|d| {
            let s = door_squares(d);
            let sq = |k: usize| ((k % nx) as isize, (k / nx) as isize);
            s.contains(&sq(a)) && s.contains(&sq(c))
        });
        let mut seen = vec![false; nx * ny];
        let mut stack = starts.clone();
        starts.iter().for_each(|&k| seen[k] = true);
        while let Some(k) = stack.pop() {
            let (i, j) = ((k % nx) as isize, (k / nx) as isize);
            for (a, c) in [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)] {
                if a < 0 || c < 0 || a as usize >= nx || c as usize >= ny {
                    continue;
                }
                let q = c as usize * nx + a as usize;
                if seen[q] || lv.cells[q] < 0 || blocked[q] || (lv.cells[q] != lv.cells[k] && !door_between(k, q)) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        for (ri, rm) in lv.rooms.iter().enumerate().filter(|(_, rm)| interior::walkable(rm.kind) && rm.squares > 0) {
            let shut = (0..nx * ny).filter(|&k| lv.cells[k] == ri as i16 && !blocked[k] && !seen[k]).count();
            if shut > 0 {
                r.add(TEMPLE_COUNTS[32], shut as f64, format!("{tag} {}: the {} has {shut} squares shut off", lv.name, rm.kind));
            }
        }
    }
    let kinds: Vec<&str> = g.rooms.iter().filter(|rm| rm.squares > 0).map(|rm| rm.kind).collect();
    let count = |k: &str| kinds.iter().filter(|&&x| x == k).count();
    let plans = interior::hall_plans(world, t0, li, b.id as usize);
    let god = town::temple::god_look(b);
    // The designer's Furnish on each room furnished as a whole, cleared: what generation gave it (the
    // pieces, their places, backs and the god's looks; the looks rolled aside).
    if let Ok((d, _)) = interior::design::design_of(world, t0, &id, false) {
        let faith = |s: Style| matches!(s, Style::Nave { .. } | Style::Sanctuary { .. } | Style::SideChapel { .. } | Style::Shrine { .. } | Style::Refectory);
        for p in plans.iter().filter(|p| faith(p.style)) {
            let Some(cells) = worldgen::under::design::decode(&d.levels[p.level].cells, nx * ny) else { continue };
            let mine = |x: u16, y: u16| cells[y as usize * nx + x as usize] == p.room;
            let mut dd = d.clone();
            dd.levels[p.level].items.retain(|f| !mine(f.x, f.y));
            dd.furnish(world, t0, &id, p.level, p.room as usize, 7);
            let look = |k: &str, v: Option<u8>| if matches!(k, "god_statue" | "candle_stand" | "carved_screen" | "screen_gate" | "side_altar" | "low_dais") { v } else { None };
            let mut got: Vec<(String, u16, u16, u16, u16, Option<u8>, Option<u8>)> = dd.levels[p.level].items.iter().filter(|f| mine(f.x, f.y)).map(|f| (f.kind.clone(), f.x, f.y, f.w, f.h, f.back, look(&f.kind, f.variant))).collect();
            let mut want: Vec<(String, u16, u16, u16, u16, Option<u8>, Option<u8>)> = it.levels[p.level].furniture.iter().filter(|f| mine(f.x, f.y)).map(|f| (f.kind.to_string(), f.x, f.y, f.w, f.h, f.back, look(f.kind, f.variant))).collect();
            got.sort();
            want.sort();
            if got != want {
                let kind = it.levels[p.level].rooms[p.room as usize].kind;
                r.add(TEMPLE_COUNTS[30], 1.0, format!("{tag} {}: the {kind}: {} pieces, generated {}", it.levels[p.level].name, got.len(), want.len()));
            }
        }
    }
    // (Every temple piece in the god's look.)
    let wrong: Vec<String> = it.levels.iter().flat_map(|lv| lv.furniture.iter()).filter(|f| matches!(f.kind, "god_statue" | "candle_stand" | "carved_screen" | "screen_gate" | "side_altar") && f.variant != Some(god)).map(|f| format!("{} v{:?}", f.kind, f.variant)).collect();
    if !wrong.is_empty() {
        r.add(TEMPLE_COUNTS[17], wrong.len() as f64, format!("{tag}: god {god}: {}", wrong.join(", ")));
    }
    // (A piece's box in a frame: s0..s1, t0..t1.)
    let local = |f: &interior::hall::Frame, it: &interior::Item| {
        let (a, c) = (f.local(it.x as isize, it.y as isize), f.local((it.x + it.w) as isize - 1, (it.y + it.h) as isize - 1));
        (a.0.min(c.0), a.0.max(c.0), a.1.min(c.1), a.1.max(c.1))
    };
    let in_room = |lv: &interior::Level, it: &interior::Item, ri: i16| lv.cells[it.y as usize * nx + it.x as usize] == ri;
    // (Pieces of `kind` in room `ri` mirrored across frame `f`'s axis, every one: their boxes.)
    let mirrored = |lv: &interior::Level, f: &interior::hall::Frame, ri: i16, kind: &str| -> (usize, bool) {
        let boxes: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == kind && in_room(lv, it, ri)).map(|it| local(f, it)).collect();
        let w = f.wid as isize;
        let ok = boxes.iter().all(|&(s0, s1, a, c)| boxes.contains(&(s0, s1, w - 1 - c, w - 1 - a)));
        (boxes.len(), ok)
    };
    if key == "shrine" {
        if kinds.len() != 1 {
            r.add(TEMPLE_COUNTS[18], kinds.len() as f64, format!("{tag} at {at_b}: {}", kinds.join(", ")));
        }
        let has = |k: &str| g.furniture.iter().any(|f| f.kind == k);
        if !has("god_statue") || !has("altar") {
            r.add(TEMPLE_COUNTS[19], 1.0, format!("{tag} at {at_b}: {} squares", g.rooms.iter().map(|rm| rm.squares).sum::<usize>()));
        }
        return;
    }
    if key == "temple" {
        let Some(kind) = kind else { return };
        let big = kind == Kind::Church;
        // The footprint: a cross ending in an apse (a hall ending in one).
        let body = b.parts.iter().find(|p| p.kind == town::PartKind::Body);
        let apse = b.parts.iter().find(|p| p.kind == town::PartKind::Apse);
        let wing = b.parts.iter().find(|p| p.kind == town::PartKind::Wing);
        let shape_ok = match (body, apse) {
            (Some(bd), Some(ap)) if bd.poly.len() == 4 => {
                let (es, et) = (sub(bd.poly[1], bd.poly[0]), sub(bd.poly[3], bd.poly[0]));
                let (le, lt) = (len(es), len(et));
                let proj = |p: P| (dot(sub(p, bd.poly[0]), es) / le, dot(sub(p, bd.poly[0]), et) / lt);
                // (The apse beyond the body's end; the transept past the body both ways.)
                let tip = ap.poly.iter().map(|q| proj(*q).0).fold(f64::MIN, f64::max);
                let round = tip >= le + 0.4 * lt - 1.0 && ap.poly.len() >= 10;
                let cross = wing.is_some_and(|w| {
                    let ts: Vec<f64> = w.poly.iter().map(|q| proj(*q).1).collect();
                    ts.iter().copied().fold(f64::MAX, f64::min) < -4.0 && ts.iter().copied().fold(f64::MIN, f64::max) > lt + 4.0
                });
                round && (cross == big) && b.poly.len() >= if big { 20 } else { 12 }
            }
            _ => false,
        };
        if !shape_ok {
            r.add(TEMPLE_COUNTS[if big { 2 } else { 3 }], 1.0, format!("{tag} at {at_b}: {} pieces, outline of {}", b.parts.len(), b.poly.len()));
        }
        // The programme.
        let apse_kept = |k: &str| g.rooms.iter().any(|rm| rm.kind == k && (3..=6).contains(&rm.shape));
        let missing: Vec<&str> = if big {
            [("nave", 1), ("sanctuary", 1), ("side chapel", 2), ("vestry", 1)].iter().filter(|(k, n)| count(k) != *n).map(|(k, _)| *k).chain((!apse_kept("sanctuary")).then_some("the sanctuary's apse")).collect()
        } else {
            [("nave", 1)].iter().filter(|(k, n)| count(k) != *n || kinds.len() != 1).map(|(k, _)| *k).chain((!apse_kept("nave")).then_some("the hall's apse")).collect()
        };
        if !missing.is_empty() {
            r.add(TEMPLE_COUNTS[4], missing.len() as f64, format!("{tag} at {at_b}: {} (has {})", missing.join(", "), kinds.join(", ")));
        }
        if let Some(fr) = g.doors.iter().find(|d| d.kind == "front")
            && g.rooms[fr.rooms[0] as usize].kind != "nave"
        {
            r.add(TEMPLE_COUNTS[5], 1.0, format!("{tag}: into the {}", g.rooms[fr.rooms[0] as usize].kind));
        }
        if big {
            // (A rail on the edge between the nave and each of them.)
            let railed = |k: &str| {
                g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == k && rm.squares > 0).all(|(ri, _)| {
                    g.walls.iter().any(|w| {
                        let d = interior::Door { a: w.a, b: [if w.a[0] == w.b[0] { w.a[0] } else { w.a[0] + 1.0 }, if w.a[1] == w.b[1] { w.a[1] } else { w.a[1] + 1.0 }], kind: "rail", rooms: [-1, -1], lock: 0 };
                        let [p, q] = door_squares(&d);
                        let (rp, rq) = (room_at(g, p), room_at(g, q));
                        w.rail && ((rp == ri as i16 && rq >= 0 && g.rooms[rq as usize].kind == "nave") || (rq == ri as i16 && rp >= 0 && g.rooms[rp as usize].kind == "nave"))
                    })
                })
            };
            if !railed("sanctuary") || !railed("side chapel") {
                r.add(TEMPLE_COUNTS[6], 1.0, format!("{tag} at {at_b}"));
            }
        }
        // The god's end: in the sanctuary's frame (a hall's in its nave's).
        let god_plan = plans.iter().find(|p| matches!(p.style, Style::Sanctuary { .. }) || matches!(p.style, Style::Nave { cross: None, .. }));
        match god_plan {
            Some(p) => {
                let lv = &it.levels[p.level];
                let f = &p.frame;
                let w = f.wid as isize;
                let statue = lv.furniture.iter().find(|it| it.kind == "god_statue" && in_room(lv, it, p.room)).map(|it| local(f, it));
                match statue {
                    Some((s0, _, a, c)) if a + c == w - 1 && s0 >= f.len as isize - 3 => {
                        let altar = lv.furniture.iter().filter(|it| it.kind == "altar" && in_room(lv, it, p.room)).map(|it| local(f, it)).any(|(as0, _, aa, ac)| aa + ac == w - 1 && as0 < s0 && as0 >= s0 - 2);
                        if !altar {
                            r.add(TEMPLE_COUNTS[8], 1.0, format!("{tag} at {at_b}"));
                        }
                    }
                    other => r.add(TEMPLE_COUNTS[7], 1.0, format!("{tag} at {at_b}: {other:?} in a {} x {} frame", f.len, f.wid)),
                }
                let (n, ok) = mirrored(lv, f, p.room, "candle_stand");
                if n < 2 || !ok {
                    r.add(TEMPLE_COUNTS[9], 1.0, format!("{tag} at {at_b}: {n} candle stands"));
                }
            }
            None => r.add(TEMPLE_COUNTS[7], 1.0, format!("{tag} at {at_b}: no frame furnished")),
        }
        // The nave.
        if let Some(p) = plans.iter().find(|p| matches!(p.style, Style::Nave { .. })) {
            let lv = &it.levels[p.level];
            let f = &p.frame;
            let w = f.wid as isize;
            if let Style::Nave { cross: Some((c0, c1)), .. } = p.style {
                let (c0, c1) = (c0 as isize, c1 as isize);
                let pillars: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == "pillar" && in_room(lv, it, p.room)).map(|it| local(f, it)).collect();
                let arcade = pillars.iter().filter(|q| q.0 < c0).count();
                let (_, mirror) = mirrored(lv, f, p.room, "pillar");
                if arcade < 4 || !mirror {
                    r.add(TEMPLE_COUNTS[10], 1.0, format!("{tag} at {at_b}: {arcade} pillars before the crossing"));
                }
                if pillars.iter().filter(|q| q.0 == c0 || q.0 == c1).count() < 4 {
                    r.add(TEMPLE_COUNTS[11], 1.0, format!("{tag} at {at_b}"));
                }
                // (The screen on the row before the crossing, from the piers' line to the gate on the
                // axis, mirrored; its gate clear.)
                let (n, ok) = mirrored(lv, f, p.room, "carved_screen");
                let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
                let pier = if w >= 11 { 2 } else { 1 };
                let boxes: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == "carved_screen" && in_room(lv, it, p.room)).map(|it| local(f, it)).collect();
                let on_row = boxes.iter().all(|q| q.0 == c0 - 1) && boxes.iter().any(|q| q.2 == pier && q.3 == cl - 1);
                let gate = (cl..=cr).all(|t| {
                    let (i, j) = f.grid(c0 - 1, t);
                    let on = |it: &&interior::Item| (i as u16) >= it.x && (i as u16) < it.x + it.w && (j as u16) >= it.y && (j as u16) < it.y + it.h;
                    !lv.furniture.iter().filter(on).any(|it| it.blocks_move) && lv.furniture.iter().filter(on).any(|it| it.kind == "screen_gate")
                });
                if n < 2 || !ok || !on_row || !gate {
                    r.add(TEMPLE_COUNTS[12], 1.0, format!("{tag} at {at_b}: {n} pieces"));
                }
            }
            // Pews, mirrored, their backs to the door's end.
            let back = {
                let d = (-f.ds.0, -f.ds.1);
                match d {
                    (0, -1) => 0,
                    (1, 0) => 1,
                    (0, 1) => 2,
                    _ => 3,
                }
            };
            // (Mirrored but beside the stairs and their foot, which may take one side.)
            let st = it.stairs;
            let by_stairs: Vec<isize> = (st[1] as isize - 1..=(st[1] + st[3]) as isize).flat_map(|j| (st[0] as isize - 1..=(st[0] + st[2]) as isize).map(move |i| f.local(i, j).0)).collect();
            let pews: Vec<(isize, isize, isize, isize)> = lv.furniture.iter().filter(|it| it.kind == "pew" && in_room(lv, it, p.room)).map(|it| local(f, it)).collect();
            let n = pews.len();
            let ok = pews.iter().all(|&(s0, s1, a, c)| by_stairs.contains(&s0) || pews.contains(&(s0, s1, w - 1 - c, w - 1 - a)));
            let facing = lv.furniture.iter().filter(|it| it.kind == "pew" && in_room(lv, it, p.room)).all(|it| it.back == Some(back));
            if n < 2 || !ok || !facing {
                r.add(TEMPLE_COUNTS[13], 1.0, format!("{tag} at {at_b}: {n} pews"));
            }
            if !lv.furniture.iter().any(|it| it.kind == "font" && in_room(lv, it, p.room)) {
                r.add(TEMPLE_COUNTS[14], 1.0, format!("{tag} at {at_b}"));
            }
        }
        if big {
            for p in plans.iter().filter(|p| matches!(p.style, Style::SideChapel { .. })) {
                let lv = &it.levels[p.level];
                let ok = lv.furniture.iter().filter(|it| it.kind == "side_altar" && in_room(lv, it, p.room)).any(|it| local(&p.frame, it).1 == p.frame.len as isize - 1);
                if !ok {
                    r.add(TEMPLE_COUNTS[15], 1.0, format!("{tag} at {at_b}"));
                }
            }
            if count("side chapel") != plans.iter().filter(|p| matches!(p.style, Style::SideChapel { .. })).count() {
                r.add(TEMPLE_COUNTS[15], 1.0, format!("{tag} at {at_b}: a chapel not furnished as a whole"));
            }
            for (ri, _) in g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == "vestry" && rm.squares > 0) {
                if !g.furniture.iter().any(|it| in_room(g, it, ri as i16)) {
                    r.add(TEMPLE_COUNTS[16], 1.0, format!("{tag} at {at_b}"));
                }
            }
        }
        return;
    }
    // A monastery round its cloister.
    if kind != Some(Kind::Monastery) {
        return;
    }
    // (The crypt: under the church, the stairs' foot and the way beside them; no more squares than
    // those and four.)
    if let Some(ri) = g.rooms.iter().position(|rm| rm.kind == "nave") {
        let church = (0..nx * ny).filter(|&k| g.cells[k] == ri as i16).count();
        for lv in it.levels.iter().filter(|lv| lv.z < 0) {
            let under = (0..nx * ny).filter(|&k| lv.cells[k] >= 0).count();
            let off = (0..nx * ny).filter(|&k| lv.cells[k] >= 0 && g.cells[k] != ri as i16).count();
            if off > 4 {
                r.add(TEMPLE_COUNTS[33], 1.0, format!("{tag} at {at_b}: {under} squares under it, {off} not under the church ({church})"));
            }
        }
    }
    let need = ["nave", "entrance hall", "cloister walk", "garth", "chapter house", "refectory", "dorter", "scriptorium", "kitchen"];
    let missing: Vec<&str> = need.iter().copied().filter(|k| count(k) == 0).collect();
    if !missing.is_empty() {
        r.add(TEMPLE_COUNTS[21], missing.len() as f64, format!("{tag} at {at_b}: {}", missing.join(", ")));
    }
    // (The rooms each one has a door to.)
    let doors_of = |ri: i16| -> Vec<i16> { g.doors.iter().filter(|d| d.rooms.contains(&ri)).map(|d| if d.rooms[0] == ri { d.rooms[1] } else { d.rooms[0] }).filter(|&o| o >= 0).collect() };
    let is = |ri: i16, k: &str| ri >= 0 && g.rooms[ri as usize].kind == k;
    for k in ["chapter house", "refectory", "dorter", "scriptorium", "kitchen"] {
        for (ri, _) in g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == k && rm.squares > 0) {
            let ok = doors_of(ri as i16).iter().any(|&o| is(o, "cloister walk") || doors_of(o).iter().any(|&q| is(q, "cloister walk")));
            if !ok {
                r.add(TEMPLE_COUNTS[22], 1.0, format!("{tag} at {at_b}: the {k}"));
            }
        }
    }
    if !g.rooms.iter().any(|rm| rm.kind == "chapter house" && matches!(rm.shape, interior::shape::OCTAGON | interior::shape::CHAMFERED)) {
        r.add(TEMPLE_COUNTS[23], 1.0, format!("{tag} at {at_b}"));
    }
    // (The church: the nave's squares a band the footprint's whole length along one side.)
    if let Some(ri) = g.rooms.iter().position(|rm| rm.kind == "nave") {
        let sq: Vec<usize> = (0..nx * ny).filter(|&k| g.cells[k] == ri as i16).collect();
        let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
        let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
        let long = nx.max(ny);
        let along = (x1 - x0 + 1 == long && (y0 == 0 || y1 == ny - 1) && nx >= ny) || (y1 - y0 + 1 == long && (x0 == 0 || x1 == nx - 1) && ny >= nx);
        if !along || sq.len() != (x1 - x0 + 1) * (y1 - y0 + 1) {
            r.add(TEMPLE_COUNTS[24], 1.0, format!("{tag} at {at_b}"));
        }
    }
    match interior::court_plan(world, t0, l, b) {
        None => r.add(TEMPLE_COUNTS[25], 1.0, format!("{tag} at {at_b}")),
        Some(c) => {
            let bb = bbox(&c.court);
            let w = ctx.window([bb[0], bb[1], bb[2], bb[3]]);
            let inside: Vec<String> = w.objects.iter().filter(|(p, name)| contains(&c.court, *p) && name.as_str() != "well").map(|(p, name)| format!("{name} at {}", at(*p))).collect();
            if !inside.is_empty() {
                r.add(TEMPLE_COUNTS[26], inside.len() as f64, format!("{tag}: {}", inside.join(", ")));
            }
        }
    }
    let garth = g.rooms.iter().position(|rm| rm.kind == "garth").map(|ri| ri as i16);
    let pieces = |k: &str| garth.is_some_and(|ri| g.furniture.iter().any(|it| it.kind == k && in_room(g, it, ri)));
    if !pieces("cloister_well") || !pieces("garden_path") {
        r.add(TEMPLE_COUNTS[27], 1.0, format!("{tag} at {at_b}"));
    }
    let refectory = plans.iter().find(|p| p.style == Style::Refectory);
    let fed = refectory.is_some_and(|p| {
        let lv = &it.levels[p.level];
        lv.furniture.iter().any(|it| it.kind == "high_table" && in_room(lv, it, p.room)) && lv.furniture.iter().any(|it| it.kind == "long_table" && in_room(lv, it, p.room))
    });
    if !fed {
        r.add(TEMPLE_COUNTS[28], 1.0, format!("{tag} at {at_b}"));
    }
    let crowded = l.buildings.iter().filter(|o| o.id != b.id && dist(centroid(&o.poly), centroid(&b.poly)) < 200.0 && poly_overlap(&o.poly, &shrink(&b.poly, 1.0))).count();
    let arcade = garth.is_some_and(|gr| g.walls.iter().any(|w| w.rail && {
        let d = interior::Door { a: w.a, b: [if w.a[0] == w.b[0] { w.a[0] } else { w.a[0] + 1.0 }, if w.a[1] == w.b[1] { w.a[1] } else { w.a[1] + 1.0 }], kind: "rail", rooms: [-1, -1], lock: 0 };
        let [p, q] = door_squares(&d);
        room_at(g, p) == gr || room_at(g, q) == gr
    }));
    if crowded > 0 || !arcade {
        r.add(TEMPLE_COUNTS[29], crowded as f64, format!("{tag} at {at_b}: {crowded} buildings on it{}", if arcade { "" } else { ", no arcade" }));
    }
}

// ---- Hashes. ----

/// The `civic` rules' counts, listed even when none.
const CIVIC_COUNTS: [&str; 11] = [
    "civic buildings laid out by zones: floors fallen back on",
    "civic buildings breaking an interior rule (as in interiors)",
    "civic buildings missing a room of their programme",
    "civic halls with no low dais and chair of office at the far end",
    "civic rooms over 2.5:1 (ways about the floor aside)",
    "arcades not on the street front (no open side, or the front door not into it)",
    "civic buildings missing a room of their programme, one storey (reasoned absent)",
    "civic buildings below the threshold (as other halls)",
    "city town halls with no belfry (why)",
    "belfries breaking a rule (zones, a room and its spiral stair on every floor to the top, the bell)",
    "civic rooms without their piece (council table, strongbox, counting desk, job board, stalls)",
];

/// The pieces a room kind must hold (A2): in a council chamber its council table (else the long
/// table stands in), in a strongroom and a warded vault a strongbox, in a counting house the counting
/// desk, in a job-board room the job board, in stables the stalls.
const ROOM_PIECES: [(&str, &str); 6] = [
    ("council chamber", "council_table"),
    ("strongroom", "strongbox"),
    ("warded vault", "strongbox"),
    ("counting house", "counting_desk"),
    ("job-board room", "job_board"),
    ("stables", "stable_stall"),
];

/// Interior `it`'s pieces by the rooms that must hold them (`ROOM_PIECES`): each room of those kinds
/// tallied, a room without its piece listed; every guild's arms tallied (on the hall's dais apart).
fn room_pieces(it: &worldgen::interior::Interior, tag: &str, r: &mut Report) {
    for lv in &it.levels {
        for (ri, room) in lv.rooms.iter().enumerate() {
            if room.squares == 0 {
                continue;
            }
            let held = |kind: &str| lv.furniture.iter().filter(|f| f.kind == kind && lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16).count();
            for (rk, piece) in ROOM_PIECES {
                if room.kind != rk {
                    continue;
                }
                let n = held(piece);
                r.tally(&format!("  rooms: {rk}"), 1);
                r.tally(&format!("    {rk}: {piece} pieces"), n);
                if n == 0 {
                    r.tally(&format!("    {rk}: without one"), 1);
                    let stand = if rk == "council chamber" && held("long_table") > 0 { ", a long table stands in" } else { "" };
                    r.add(CIVIC_COUNTS[10], room.squares as f64, format!("{tag} {}: {rk} of {} squares{stand}", lv.name, room.squares));
                }
            }
            let arms = held("guild_arms");
            if arms > 0 {
                r.tally(if room.kind == "great hall" { "  guild's arms in halls" } else { "  guild's arms in other rooms" }, arms);
            }
            if room.kind == "great hall" && held("job_board") > 0 {
                r.tally("  job boards in halls", 1);
            }
        }
    }
}

/// A belfried town hall's interior `it` (its body's top floor `floors - 1`): the spiral stair on the
/// top floor's landing, then the tower's room on each floor above to `floors - 1 + STOREYS` (a
/// belfry stair, at the top the belfry), the spiral stair in it, the bell at the top; the problems.
fn belfry_rules(it: &worldgen::interior::Interior, floors: i8) -> Vec<String> {
    let top = floors - 1 + town::belfry::STOREYS;
    let mut bad = Vec::new();
    if !it.levels.iter().find(|lv| lv.z == floors - 1).is_some_and(|lv| lv.furniture.iter().any(|f| f.kind == "spiral_stair")) {
        bad.push(format!("z {}: no spiral stair up", floors - 1));
    }
    for z in floors..=top {
        let Some(lv) = it.levels.iter().find(|lv| lv.z == z) else {
            bad.push(format!("no level {z}"));
            continue;
        };
        let want = if z == top { "belfry" } else { "belfry stair" };
        let Some(ri) = lv.rooms.iter().position(|r| r.kind == want && r.squares > 0) else {
            bad.push(format!("z {z}: no {want}"));
            continue;
        };
        let inr = |f: &worldgen::interior::Item| lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16;
        if !lv.furniture.iter().any(|f| f.kind == "spiral_stair" && inr(f)) {
            bad.push(format!("z {z}: no spiral stair"));
        }
        if z == top && !lv.furniture.iter().any(|f| f.kind == "bell" && inr(f)) {
            bad.push(format!("z {z}: no bell"));
        }
        if lv.has_stairs || lv.rooms.iter().filter(|r| r.squares > 0).count() != 1 {
            bad.push(format!("z {z}: not the tower's alone"));
        }

    }
    if it.levels.iter().any(|lv| lv.z > top) {
        bad.push("levels above the bell loft".to_string());
    }
    bad
}

/// `piers`: dock piers (not part of `all`). Per town with piers: how many, how many are shorter than
/// the 105 ft a sea pier gets (cut back on a river), and the narrowest open channel between piers that
/// face each other across the water (outline to outline); pairs closer than 40 ft, piers overlapping
/// another, and piers within 30 ft of a bridge deck.
fn piers(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["towns with piers", "piers", "piers shorter than 105 ft", "facing pairs", "facing pairs under 40 ft", "piers overlapping another", "piers within 30 ft of a bridge"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.piers.is_empty() {
            continue;
        }
        let tag = format!("L{li} {:?}", l.tier);
        r.tally("towns with piers", 1);
        r.tally("piers", l.piers.len());
        let axis = |p: &Vec<[f64; 2]>| {
            let d = [p[1][0] - p[0][0], p[1][1] - p[0][1]];
            let n = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
            ([d[0] / n, d[1] / n], n)
        };
        for (i, a) in l.piers.iter().enumerate() {
            let (ua, la) = axis(a);
            if la < 104.0 {
                r.tally("piers shorter than 105 ft", 1);
            }
            if l.bridges.iter().any(|b| town::geom::poly_gap(a, b) < 30.0) {
                r.add("piers within 30 ft of a bridge", 1.0, format!("{tag} pier {i}, {la:.0} ft long"));
            }
            for (j, b) in l.piers.iter().enumerate().skip(i + 1) {
                let (ca, cb) = (centroid(a), centroid(b));
                let d = [cb[0] - ca[0], cb[1] - ca[1]];
                let dn = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                let (ub, _) = axis(b);
                let gap = town::geom::poly_gap(a, b);
                if gap == 0.0 {
                    r.add("piers overlapping another", 1.0, format!("{tag} piers {i} {j} at {:.0} {:.0}, {la:.0} and {:.0} ft long", ca[0], ca[1], axis(b).1));
                }
                // Facing: roughly end to end along their length, not side by side.
                if (ua[0] * ub[0] + ua[1] * ub[1]).abs() > 0.5 && ((d[0] * ua[0] + d[1] * ua[1]) / dn).abs() > 0.7 && dn < 400.0 {
                    r.tally("facing pairs", 1);
                    if gap < 40.0 {
                        r.add("facing pairs under 40 ft", gap, format!("{tag} piers {i} {j} gap {gap:.0}"));
                    }
                    r.measure("narrowest open channel between facing piers (ft)", true, gap, format!("{tag} piers {i} {j}"));
                    // (River piers are cut short of 105 ft; sea, lake and harbour ones are not.)
                    if la < 104.0 && axis(b).1 < 104.0 {
                        r.measure("  of those, both cut short (river, wharf): narrowest (ft)", true, gap, format!("{tag} piers {i} {j}"));
                    }
                }
            }
        }
    }
}

/// Town halls and the four guilds laid out by zones (`interior::hub::civic`): no floor fallen back
/// on, every interior rule, every room of the programme (the council chamber, the strongroom, the
/// archive, the guild's own; on a single storey some may be absent), the hall's low dais and chair
/// of office at the far end of its frame (with the `halls` rules over it), rooms no longer than
/// 2.5:1, an arcade open along the street front with the front door into it.
fn civic(ctx: &mut Ctx, r: &mut Report) {
    use interior::hub::Kind;
    r.counts(&["civic buildings", "  laid out by zones"]);
    r.counts(&CIVIC_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    const KEYS: [&str; 5] = ["town_hall", "merchant_guild", "craft_guild", "adventurers_guild", "mages_guild"];
    let key_of = |b: &town::Building| b.func.map(|f| town::catalog::CATALOG[f as usize].key).filter(|k| KEYS.contains(k));
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        // (Belfries: on every city's or metropolis's town hall one goes on, else why not.)
        if l.tier >= worldgen::t0::settle::Tier::City {
            for b in l.buildings.iter().filter(|b| town::belfry::is_town_hall(b)) {
                let id = format!("b:{li}:{}", b.id);
                if town::belfry::of(b).is_some() {
                    r.tally("belfries", 1);
                    // (BELFRY_AT=1: where each stands, world ft.)
                    if std::env::var("BELFRY_AT").is_ok() {
                        let c = centroid(&b.poly);
                        say!("belfry {id} at {:.0} {:.0} ({}, {} floors)", c[0], c[1], interior::hub_kind(t0, &l, b).map_or("-", |k| k.name()), b.floors);
                    }
                    let fell = interior::hub_kind(t0, &l, b).is_none();
                    let bad = if fell { vec!["not laid out by zones".to_string()] } else { interior::generate_id(world, t0, &id).map(|it| belfry_rules(&it, b.floors.max(1) as i8)).unwrap_or_default() };
                    if !bad.is_empty() {
                        r.add(CIVIC_COUNTS[9], bad.len() as f64, format!("{id}: {}", bad.join("; ")));
                    }
                    continue;
                }
                let why = if !town::belfry::eligible(b) { "not roofed, a castle's or in pieces".to_string() } else { format!("{:?}", town::belfry::raise(&l, b).err()) };
                r.tally(&format!("  no belfry: {why}"), 1);
                r.add(CIVIC_COUNTS[8], 1.0, format!("{id}: {why}"));
            }
        }
        // (Stables' stalls.)
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == "stables")) {
            let id = format!("b:{li}:{}", b.id);
            if let Some(it) = interior::generate_id(world, t0, &id) {
                room_pieces(&it, &format!("{id} (stables)"), r);
            }
        }
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let Some(key) = key_of(b) else { continue };
            r.tally("civic buildings", 1);
            r.tally(&format!("  {key}s"), 1);
            let id = format!("b:{li}:{}", b.id);
            let Some(kind) = interior::hub_kind(t0, &l, b) else {
                let n = interior::Shell::of(&l, b).inside.iter().filter(|&&i| i).count();
                r.add(CIVIC_COUNTS[7], n as f64, format!("{id} ({key}): {n} squares"));
                continue;
            };
            r.tally("  laid out by zones", 1);
            r.tally(&format!("    {}s", kind.name()), 1);
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            let tag = format!("{id} ({})", kind.name());
            room_pieces(&it, &tag, r);

            if std::env::var("PLAN").is_ok_and(|p| p.trim() == id) {
                for lv in &it.levels {
                    say!("-- {} (z {}): {}", lv.name, lv.z, lv.rooms.iter().map(|r| format!("{}={}", r.kind, r.squares)).collect::<Vec<_>>().join(", "));
                    for j in 0..it.ny {
                        let row: String = (0..it.nx)
                            .map(|i| {
                                let c = lv.cells[j * it.nx + i];
                                let st = it.stairs;
                                if lv.has_stairs && i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3] {
                                    '#'
                                } else if c < 0 {
                                    ' '
                                } else {
                                    (b'a' + (c as u8 % 26)) as char
                                }
                            })
                            .collect();
                        say!("  {row}");
                    }
                    let pieces: Vec<String> = lv.furniture.iter().filter(|f| std::env::var("ALL").is_ok() || matches!(f.kind, "low_dais" | "chair_of_office" | "pillar")).map(|f| format!("{}@{},{}", f.kind, f.x, f.y)).collect();
                    say!("  pieces: {}", pieces.join(" "));
                }
                for p in interior::hall_plans(world, t0, li, b.id as usize) {
                    say!("  hall frame: level {} room {} o {:?} ds {:?} dt {:?} {}x{}", p.level, p.room, p.frame.o, p.frame.ds, p.frame.dt, p.frame.len, p.frame.wid);
                }
            }
            let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
            if !fell.is_empty() {
                r.add(CIVIC_COUNTS[0], fell.len() as f64, format!("{tag}: z {fell:?}"));
            }
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            let bad = check_interior(world, t0, &l, li, &id, &it, buckets);
            if !bad.is_empty() {
                r.add(CIVIC_COUNTS[1], bad.len() as f64, format!("{tag}: {}", bad.join("; ")));
            }
            // (The programme.)
            let mut want = vec!["great hall", "council chamber", "strongroom", "archive"];
            want.extend(match kind {
                Kind::MerchantGuild => vec!["counting house", "sample room"],
                Kind::CraftGuild => vec!["masterpiece gallery", "workshop"],
                Kind::AdventurersGuild => vec!["job-board room", "trophy hall", "armory", "bunk room"],
                Kind::MagesGuild => vec!["library", "laboratory", "warded vault"],
                Kind::ArcadeHall => vec!["arcade"],
                _ => vec![],
            });
            let above = it.levels.iter().filter(|lv| lv.z >= 0 && !lv.roof).count();
            let missing: Vec<&str> = want.iter().copied().filter(|k| !it.levels.iter().any(|lv| lv.rooms.iter().any(|r| r.kind == *k && r.squares > 0))).collect();
            if !missing.is_empty() {
                let row = if above <= 1 && !missing.contains(&"great hall") { CIVIC_COUNTS[6] } else { CIVIC_COUNTS[2] };
                r.add(row, missing.len() as f64, format!("{tag}, {above} storeys: no {}", missing.join(", ")));
            }
            // (The hall's dais and chair of office in its frame's last two rows.)
            let plans = interior::hall_plans(world, t0, li, b.id as usize);
            let dais = plans.iter().any(|p| {
                let lv = &it.levels[p.level];
                let f = p.frame;
                let (i0, j0) = f.grid(f.len as isize - 2, 0);
                let (i1, j1) = f.grid(f.len as isize - 1, f.wid as isize - 1);
                let far = |x: u16, y: u16| {
                    let (x, y) = (x as isize, y as isize);
                    x >= i0.min(i1) && x <= i0.max(i1) && y >= j0.min(j1) && y <= j0.max(j1)
                };
                lv.rooms.get(p.room as usize).is_some_and(|r| r.kind == "great hall")
                    && lv.furniture.iter().any(|f| f.kind == "low_dais" && far(f.x, f.y))
                    && lv.furniture.iter().any(|f| f.kind == "chair_of_office" && far(f.x, f.y))
            });
            if !dais {
                r.add(CIVIC_COUNTS[3], 1.0, tag.clone());
            }
            // (Proportions: every room one stands in but a way about the floor, by its box.)
            for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
                for (ri, room) in lv.rooms.iter().enumerate() {
                    if room.squares < 4 || interior::is_circulation(room.kind) || !interior::walkable(room.kind) {
                        continue;
                    }
                    let sq: Vec<usize> = (0..it.nx * it.ny).filter(|&k| lv.cells[k] == ri as i16).collect();
                    let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % it.nx), a.1.max(k % it.nx)));
                    let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / it.nx), a.1.max(k / it.nx)));
                    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
                    if 2 * w.max(h) > 5 * w.min(h) {
                        r.add(CIVIC_COUNTS[4], w.max(h) as f64 / w.min(h) as f64, format!("{tag} {}: {} {w}x{h}", lv.name, room.kind));
                    }
                }
            }
            // (An arcade: open along the street front, the front door into it.)
            if kind == Kind::ArcadeHall {
                let ok = it.levels.iter().find(|lv| lv.z == 0).is_some_and(|lv| {
                    let arc = lv.rooms.iter().position(|r| r.kind == "arcade").map(|a| a as i16);
                    // (Open along 4 squares or more of the front.)
                    let open: f32 = lv.walls.iter().filter(|w| w.arcade).map(|w| (w.b[0] - w.a[0]).abs() + (w.b[1] - w.a[1]).abs()).sum();
                    r.tally("  arcade fronts' open squares", open as usize);
                    arc.is_some_and(|a| open >= 4.0 && lv.doors.iter().any(|d| d.kind == "front" && d.rooms.contains(&a)))
                });
                if !ok {
                    r.add(CIVIC_COUNTS[5], 1.0, tag.clone());
                }
            }
        }
        // (The `halls` rules over their halls.)
        layout_halls(world, t0, &l, li, &format!("#{li}"), &|b| key_of(b).is_some() && interior::hub_kind(t0, &l, b).is_some(), r);
    }
}

const INN_COUNTS: [&str; 15] = [
    "inns laid out by zones: floors fallen back on",
    "inns breaking an interior rule (as in interiors)",
    "inns missing a room of their programme",
    "inns missing a guest floor's rooms, one storey (reasoned absent)",
    "inns missing a snug, scullery or private dining room, a front 9 or less (reasoned absent)",
    "inn rooms over 2.5:1 (ways about the floor aside)",
    "inns over the threshold no frame fits (as small inns)",
    "inns below the threshold (as small inns)",
    "coaching inns whose yard no arch 2 squares wide opens onto from the street",
    "coaching inns with stables not opening onto the yard",
    "coaching inn rooms upstairs off neither the gallery nor a corridor",
    "coaching inns with another building on their ground",
    "coaching inns whose front door or arch opens 20 ft or more from a street",
    "coaching inns (merged, `town::palace::is_coaching`) not round a yard",
    "coaching inns round a yard (where, by the yard open to the sky: squares)",
];

/// Inns and caravanserais (`interior::hub::inn`): the rules in the header's `inns`.
fn inns(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["inns and caravanserais", "  laid out by zones"]);
    r.counts(&INN_COUNTS);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let letter = |x: i16| if x < 0 { '-' } else { (b'a' + (x as u8 % 26)) as char };
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut buckets = None;
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let Some(key) = b.func.map(|f| town::catalog::CATALOG[f as usize].key).filter(|k| matches!(*k, "inn" | "caravanserai")) else { continue };
            r.tally("inns and caravanserais", 1);
            r.tally(&format!("  {key}s"), 1);
            let id = format!("b:{li}:{}", b.id);
            let Some(kind) = interior::hub_kind(t0, &l, b) else {
                let sh = interior::Shell::of(&l, b);
                let n = sh.inside.iter().filter(|&&i| i).count();
                let row = if n >= interior::hub::INN_LEAST { 6 } else { 7 };
                // (Its box and how full it is: a slanted or ragged lot's under 80%.)
                let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
                for k in (0..sh.nx * sh.ny).filter(|&k| sh.inside[k]) {
                    (x0, x1, y0, y1) = (x0.min(k % sh.nx), x1.max(k % sh.nx), y0.min(k / sh.nx), y1.max(k / sh.nx));
                }
                let (bw, bh) = (x1.wrapping_sub(x0).wrapping_add(1), y1.wrapping_sub(y0).wrapping_add(1));
                let full = 100 * n / (bw * bh).max(1);
                r.add(INN_COUNTS[row], n as f64, format!("{id} ({key}): {n} squares, box {bw}x{bh} {full}% full, {} floors", b.floors));
                continue;
            };
            r.tally("  laid out by zones", 1);
            r.tally(&format!("    {}s", kind.name()), 1);
            r.tally(&format!("    {} storeys", b.floors.max(1)), 1);
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            let tag = format!("{id} ({})", kind.name());
            if std::env::var("PLAN").is_ok_and(|p| p.trim() == id) {
                for lv in &it.levels {
                    say!("-- {} (z {}): {}", lv.name, lv.z, lv.rooms.iter().enumerate().map(|(k, r)| format!("{}={}{}", letter(k as i16), r.kind, r.squares)).collect::<Vec<_>>().join(", "));
                    for j in 0..it.ny {
                        let row: String = (0..it.nx)
                            .map(|i| {
                                let st = it.stairs;
                                if lv.has_stairs && i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3] { '#' } else if lv.cells[j * it.nx + i] < 0 { ' ' } else { letter(lv.cells[j * it.nx + i]) }
                            })
                            .collect();
                        say!("  {row}");
                    }
                    let doors: Vec<String> = lv.doors.iter().map(|d| format!("{}{}{}", letter(d.rooms[0]), letter(d.rooms[1]), if d.kind == "front" { "!" } else { "" })).collect();
                    say!("  doors: {}", doors.join(" "));
                    let pieces: Vec<String> = lv.furniture.iter().map(|f| format!("{}@{},{}", f.kind, f.x, f.y)).collect();
                    say!("  pieces: {}", pieces.join(" "));
                }
            }
            let fell = interior::hub_fallbacks(world, t0, li, b.id as usize);
            if !fell.is_empty() {
                r.add(INN_COUNTS[0], fell.len() as f64, format!("{tag}: z {fell:?}"));
            }
            let buckets = buckets.get_or_insert_with(|| building_buckets(&l));
            let bad = check_interior(world, t0, &l, li, &id, &it, buckets);
            if !bad.is_empty() {
                r.add(INN_COUNTS[1], bad.len() as f64, format!("{tag}: {}", bad.join("; ")));
            }
            // (The programme: the ground floor's, then the guest floors'.)
            let has = |k: &str| it.levels.iter().any(|lv| lv.rooms.iter().any(|r| r.kind == k && r.squares > 0));
            let ground = ["taproom", "snug", "private dining room", "kitchen", "scullery", "storeroom"];
            let upper = ["best chamber", "guest room", "dormitory", "innkeeper's quarters"];
            let above = it.levels.iter().filter(|lv| lv.z >= 0 && !lv.roof).count();
            let wid = interior::zone_plan_of(world, t0, li, b.id as usize).map(|p| p.frame().wid);
            let (mut miss, mut narrow, mut one) = (Vec::new(), Vec::new(), Vec::new());
            for k in ground.iter().chain(&upper).copied().filter(|k| !has(k)) {
                if upper.contains(&k) && above <= 1 {
                    one.push(k);
                } else if matches!(k, "snug" | "scullery" | "private dining room") && wid.is_some_and(|w| w <= 9) {
                    narrow.push(k);
                } else {
                    miss.push(k);
                }
            }
            for (row, list) in [(2, &miss), (3, &one), (4, &narrow)] {
                if !list.is_empty() {
                    r.add(INN_COUNTS[row], list.len() as f64, format!("{tag}, {above} storeys, front {wid:?}: no {}", list.join(", ")));
                }
            }
            if kind == interior::hub::Kind::Caravanserai {
                r.tally(if has("stables") { "  caravanserais with stables" } else { "  caravanserais without stables (no room)" }, 1);
            }
            // (A coaching inn's yard: reached from the street through its arch, its stables onto it,
            // every room upstairs off the gallery, nothing on its ground, its doors on a street.)
            if town::palace::is_coaching(&l, b) {
                let yard = interior::zone_plan_of(world, t0, li, b.id as usize).and_then(|p| p.yard());
                let Some(((ys0, ys1, yt0, yt1), _)) = yard else {
                    r.add(INN_COUNTS[13], 1.0, format!("{tag}: {} floors at {}", b.floors, at(centroid(&b.poly))));
                    continue;
                };
                r.tally(&format!("  coaching inns round a yard: {:?} {key}s", l.tier), 1);
                r.add(INN_COUNTS[14], (ys1 - ys0) as f64 * (yt1 - yt0) as f64, format!("{tag}: {:?}, yard open {}x{}, {} floors at {}", l.tier, ys1 - ys0, yt1 - yt0, b.floors, at(centroid(&b.poly))));
                r.tally(&format!("    yards open {}x{}", ys1 - ys0, yt1 - yt0), 1);
                let g = it.levels.iter().find(|lv| lv.z == 0).expect("ground floor");
                let yr = g.rooms.iter().position(|rm| rm.kind == "inn yard").map(|k| k as i16);
                let arches = g.doors.iter().filter(|d| d.kind == "arch" && Some(d.rooms[0]) == yr).count();
                if arches < 2 {
                    r.add(INN_COUNTS[8], 1.0, format!("{tag}: {arches} arch edges"));
                }
                for (ri, _) in g.rooms.iter().enumerate().filter(|(_, rm)| rm.kind == "stables" && rm.squares > 0) {
                    if !g.doors.iter().any(|d| d.rooms.contains(&(ri as i16)) && yr.is_some_and(|y| d.rooms.contains(&y))) {
                        r.add(INN_COUNTS[9], 1.0, format!("{tag}: stables {ri}"));
                    }
                }
                for lv in it.levels.iter().filter(|lv| lv.z >= 1) {
                    let way = |x: i16| x >= 0 && matches!(lv.rooms[x as usize].kind, "yard gallery" | "corridor");
                    for (ri, rm) in lv.rooms.iter().enumerate().filter(|(_, rm)| rm.squares > 0 && matches!(rm.kind, "best chamber" | "guest room" | "dormitory" | "innkeeper's quarters")) {
                        if !lv.doors.iter().any(|d| d.rooms.contains(&(ri as i16)) && (way(d.rooms[0]) || way(d.rooms[1]))) {
                            r.add(INN_COUNTS[10], 1.0, format!("{tag} {}: {} {ri}", lv.name, rm.kind));
                        }
                    }
                }
                for (_, gr) in l.grounds.iter().filter(|g| g.0 == b.id) {
                    for o in l.buildings.iter().filter(|o| o.id != b.id && poly_overlap(&shrink(&o.poly, 1.0), &shrink(gr, 1.0))) {
                        r.add(INN_COUNTS[11], 1.0, format!("{tag}: b:{li}:{} at {}", o.id, at(centroid(&o.poly))));
                    }
                }
                let sh = interior::Shell::of(&l, b);
                let street = |p: [f64; 2]| l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| seg_dist(p, q[0], q[1]) - 0.5 * w)).fold(f64::MAX, f64::min);
                for d in g.doors.iter().filter(|d| matches!(d.kind, "front" | "arch")) {
                    let m = sh.world_at(0.5 * (d.a[0] + d.b[0]) as f64, 0.5 * (d.a[1] + d.b[1]) as f64);
                    if street(m) >= 20.0 {
                        r.add(INN_COUNTS[12], street(m), format!("{tag}: {} door {:.0} ft from a street at {}", d.kind, street(m), at(m)));
                    }
                }
            }
            // (Rooms per kind, their proportions by their boxes, and the pieces that make them what
            // they are.)
            for lv in it.levels.iter().filter(|lv| lv.z >= 0) {
                for (ri, room) in lv.rooms.iter().enumerate() {
                    if room.squares == 0 {
                        continue;
                    }
                    r.tally(&format!("  rooms: {}", room.kind), 1);
                    let piece = match room.kind {
                        "best chamber" => Some("bed"),
                        "dormitory" => Some("cot"),
                        "snug" => Some("table"),
                        "private dining room" => Some("long_table"),
                        "stables" => Some("stable_stall"),
                        _ => None,
                    };
                    if let Some(piece) = piece
                        && !lv.furniture.iter().any(|f| f.kind == piece && lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16)
                    {
                        r.tally(&format!("    {} without a {piece}", room.kind), 1);
                    }
                    if room.squares < 4 || interior::is_circulation(room.kind) || !interior::walkable(room.kind) {
                        continue;
                    }
                    let sq: Vec<usize> = (0..it.nx * it.ny).filter(|&k| lv.cells[k] == ri as i16).collect();
                    let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % it.nx), a.1.max(k % it.nx)));
                    let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / it.nx), a.1.max(k / it.nx)));
                    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
                    if 2 * w.max(h) > 5 * w.min(h) {
                        r.add(INN_COUNTS[5], w.max(h) as f64 / w.min(h) as f64, format!("{tag} {}: {} {w}x{h}", lv.name, room.kind));
                    }
                }
            }
        }
    }
}

fn hashes(ctx: &mut Ctx, label: &str, r: &mut Report) {
    use worldgen::core::hash::fnv64;
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let castle = if !l.castles.is_empty() || l.buildings.iter().any(|b| b.ward == Ward::Castle) { "C" } else { "-" };
        // (Without the keep's mark, nor an empty list of forecourts or grounds, so that the hashes
        // compare with trees from before them.)
        let mut text = format!("{l:?}").replace(", grounds: [] }", " }").replace(", keep: false }", " }").replace(", keep: true }", " }").replace(", keep: None }", " }").replace(", forecourts: [] }", " }");
        for k in ["Great", "Hall", "L", "T", "Shell", "Plain"] {
            text = text.replace(&format!(", keep: Some({k}) }}"), " }");
        }
        // (Nor the castle's and the market's patches, recorded later.)
        if let Some(a) = text.find(", castle_patch: ")
            && let Some(b) = text[a..].find(", bbox: ")
        {
            text.replace_range(a..a + b, "");
        }
        println!("{label} L:{li} {:016x} {castle} {}", fnv64(text.as_bytes()), l.buildings.len());
        r.tally("layouts hashed", 1);
        // (Each building's own, with what it is: which buildings a change of the layout's is in.)
        for b in &l.buildings {
            println!("{label} B:{li}:{} {:016x} {castle} {}", b.id, fnv64(format!("{b:?}").as_bytes()), b.label().replace(' ', "_"));
        }
        let mut ids: Vec<String> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed).map(|b| format!("b:{li}:{}", b.id)).collect();
        ids.extend((0..interior::towers(&l).len()).map(|k| format!("t:{li}:{k}")));
        for id in ids {
            match interior::generate_id(world, t0, &id) {
                Some(it) => println!("{label} {id} {:016x} {castle} {}", fnv64(serde_json::to_string(&it).unwrap().as_bytes()), it.function.replace(' ', "_")),
                None => println!("{label} {id} none {castle}"),
            }
            r.tally("interiors hashed", 1);
        }
    }
}

/// `under`: every underground site of every layout (`u:` ways down, a few sewer sections each, keep dungeons
/// `k:`) with a hash of its serialized interior on stdout (`U <label> <id> <hash> <theme> <levels>`), and the
/// totals: sites, levels, rooms, doors, secret doors, loops (doors beyond a spanning tree of each level's
/// rooms), shortcut loops (each saving 4 or more hops: `shortcut_loops`) and the hops they save, and the sites failing `under::design::check`; locks, keys (and those on a level above their lock's), sites
/// whose keys are out of order (`under::design::key_order`), and built sites and their locks by size (`keep`, `sewer` for `k:`, `w:`).
fn under(ctx: &mut Ctx, r: &mut Report, label: &str) {
    use worldgen::core::hash::fnv64;
    use worldgen::under::{self, UnderKind};
    r.counts(&["sites", "levels", "rooms", "doors", "secret doors", "loops", "shortcut loops", "hops they save", "sites breaking a rule", "locks", "keys", "keys on an earlier level", "sites with keys out of order"]);
    for s in ["small", "medium", "large", "huge", "keep", "sewer"] {
        r.counts(&[&format!("sites {s}"), &format!("locks {s}")]);
    }
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        let mut ids: Vec<String> = Vec::new();
        let mut sections: Vec<[i64; 2]> = Vec::new();
        for (k, e) in l.entrances.iter().enumerate() {
            if e.kind == UnderKind::Sewer {
                let o = under::sewer_section(e.at);
                let q = [(o[0] / under::SEWER_SECTION_FT) as i64, (o[1] / under::SEWER_SECTION_FT) as i64];
                if sections.len() < 4 && !sections.contains(&q) {
                    sections.push(q);
                    ids.push(format!("w:{li}:{}:{}", q[0], q[1]));
                }
            } else {
                ids.push(format!("u:{li}:{k}"));
            }
        }
        ids.extend(l.buildings.iter().filter(|b| interior::has_deep_dungeon(b)).map(|b| format!("k:{li}:{}", b.id)));
        for id in ids {
            let t = std::time::Instant::now();
            let Some(it) = interior::generate_id(world, t0, &id) else {
                println!("U {label} {id} none");
                continue;
            };
            // THEME=a,b: only sites of those themes (`keep` keep dungeons, `undercroft` sewer sections).
            let theme = it.theme.unwrap_or(if id.starts_with('k') { "keep" } else if id.starts_with('w') { "undercroft" } else { "-" });
            if let Ok(th) = std::env::var("THEME")
                && !th.split(',').any(|t| t == theme)
            {
                continue;
            }
            r.measure("slowest site (ms)", false, t.elapsed().as_secs_f64() * 1e3, id.clone());
            // (A sewer section's own level too: the undercroft below it may change alone.)
            let top = if id.starts_with('w') { format!(" top {:016x}", fnv64(serde_json::to_string(it.levels.last().unwrap()).unwrap().as_bytes())) } else { String::new() };
            println!("U {label} {id} {:016x} {} {}{top}", fnv64(serde_json::to_string(&it).unwrap().as_bytes()), it.theme.unwrap_or("-").replace(' ', "_"), it.levels.len());
            r.tally("sites", 1);
            r.tally("levels", it.levels.len());
            // Cave-ins by the age of the place above (levels with open gaps; a collapsed fortress's own left out).
            if theme != "collapsed_fortress" && it.levels.iter().all(|lv| !lv.natural) {
                let class = if id.starts_with('u') && li >= t0.settlements.len() { "ruin" } else { "living" };
                let caved = it.levels.iter().filter(|lv| lv.doors.iter().any(|d| d.kind == "arch")).count();
                r.tally(&format!("built sites {class}"), 1);
                r.tally(&format!("built levels {class}"), it.levels.len());
                r.tally(&format!("caved sites {class}"), (caved > 0) as usize);
                r.tally(&format!("caved levels {class}"), caved);
            }
            for lv in &it.levels {
                r.tally("rooms", lv.rooms.len());
                r.tally("doors", lv.doors.len());
                r.tally("secret doors", lv.doors.iter().filter(|d| d.kind == "secret").count());
                r.tally("portcullises", lv.doors.iter().filter(|d| d.kind == "portcullis").count());
                for f in lv.furniture.iter().filter(|f| matches!(f.kind, "water_shallow" | "water_deep" | "rope_bridge" | "stepping_stone")) {
                    r.tally(&format!("{} squares", f.kind), (f.w * f.h) as usize);
                }
                // (Union-find over the rooms: a door joining rooms already joined is a loop.)
                let mut up: Vec<usize> = (0..lv.rooms.len()).collect();
                fn root(up: &mut [usize], mut i: usize) -> usize {
                    while up[i] != i {
                        up[i] = up[up[i]];
                        i = up[i];
                    }
                    i
                }
                for d in &lv.doors {
                    let [a, b] = d.rooms;
                    if a < 0 || b < 0 || a as usize >= up.len() || b as usize >= up.len() {
                        continue;
                    }
                    let (ra, rb) = (root(&mut up, a as usize), root(&mut up, b as usize));
                    if ra == rb {
                        r.tally("loops", 1);
                    } else {
                        up[ra] = rb;
                    }
                }
                if !lv.natural {
                    let (n, saved) = shortcut_loops(lv, it.nx, it.ny);
                    r.tally("shortcut loops", n);
                    r.tally("hops they save", saved);
                }
            }
            // Locks and keys (levels run bottom to top: an earlier level is a later index).
            let locks: Vec<(usize, u8)> = it.levels.iter().enumerate().flat_map(|(li, lv)| lv.doors.iter().filter(|d| d.lock != 0).map(move |d| (li, d.lock))).collect();
            let keys: Vec<(usize, u8)> = it.levels.iter().enumerate().flat_map(|(li, lv)| lv.furniture.iter().filter(|f| f.kind == "key").map(move |f| (li, f.variant.unwrap_or(0)))).collect();
            r.tally("locks", locks.len());
            r.tally("keys", keys.len());
            r.tally("keys on an earlier level", keys.iter().filter(|k| locks.iter().any(|l| l.1 == k.1 && l.0 < k.0)).count());
            let size = under::site_size(&it).map(|s| s.name()).unwrap_or(if id.starts_with('k') { "keep" } else { "sewer" });
            if it.levels.iter().all(|lv| !lv.natural) {
                // (Built sites only: caves, mines and lava tubes have no doors.)
                r.tally(&format!("sites {size}"), 1);
                r.tally(&format!("locks {size}"), locks.len());
            }
            r.measure("most locks in a site", false, locks.len() as f64, id.clone());
            let order = under::design::key_order(&it);
            if !order.is_empty() {
                r.add("sites with keys out of order", order.len() as f64, format!("{id} ({}): {}", it.theme.unwrap_or("-"), order.into_iter().map(|p| p.text).collect::<Vec<_>>().join("; ")));
            }
            // (Its rules are a designed `u:` site's. A keep dungeon is entered by the stairs up to the keep, an undercroft
            // by its way up to the sewer section above: each checked as if that were its way in from the surface, the
            // sewer section's own level left out.)
            let bad: Vec<String> = if id.starts_with('u') {
                under::design::check(&it, None).into_iter().filter(|p| p.blocking).map(|p| p.text).collect()
            } else {
                let mut c = it.clone();
                if id.starts_with('w') {
                    c.levels.pop();
                    c.entry_level = c.levels.len().saturating_sub(1);
                }
                if let Some(top) = c.levels.last_mut() {
                    for f in top.furniture.iter_mut().filter(|f| f.kind == "up") {
                        f.kind = "exit";
                    }
                }
                if c.levels.is_empty() { Vec::new() } else { under::design::check(&c, None).into_iter().filter(|p| p.blocking).map(|p| p.text).collect() }
            };
            if !bad.is_empty() {
                r.add("sites breaking a rule", bad.len() as f64, format!("{id} ({}): {}", it.theme.unwrap_or("-"), bad.join("; ")));
            }
            if std::env::var("TERRAIN").is_ok() {
                under_terrain(it, &id, r);
            }
        }
    }
}

/// A built level's shortcut loops and the hops they save: doors in turn, each counted (and then taken away, so a
/// loop counts once) if the way round without it is 4 or more hops longer (on foot: a square 1, a door 8, a hop 8).
fn shortcut_loops(lv: &interior::Level, nx: usize, ny: usize) -> (usize, usize) {
    let pair = |d: &interior::Door| {
        let (x, y) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize);
        if d.a[1] == d.b[1] { (y.saturating_sub(1) * nx + x, y * nx + x) } else { (y * nx + x.saturating_sub(1), y * nx + x) }
    };
    let mut open: std::collections::HashSet<(usize, usize)> = lv.doors.iter().map(pair).collect();
    let walk = |open: &std::collections::HashSet<(usize, usize)>, from: usize, to: usize| {
        let mut cost = vec![u32::MAX; nx * ny];
        let mut heap = std::collections::BinaryHeap::new();
        cost[from] = 0;
        heap.push(std::cmp::Reverse((0u32, from)));
        while let Some(std::cmp::Reverse((c, k))) = heap.pop() {
            if k == to {
                return Some(c);
            }
            if c > cost[k] {
                continue;
            }
            let (i, j) = (k % nx, k / nx);
            for m in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten() {
                let (a, b) = (lv.cells[k], lv.cells[m]);
                if b < 0 || (a != b && !open.contains(&(k.min(m), k.max(m)))) {
                    continue;
                }
                let nc = c + if a == b { 1 } else { 8 };
                if nc < cost[m] {
                    cost[m] = nc;
                    heap.push(std::cmp::Reverse((nc, m)));
                }
            }
        }
        None
    };
    let (mut n, mut saved) = (0, 0);
    for d in &lv.doors {
        let e = pair(d);
        if e.1 >= nx * ny || lv.cells[e.0] < 0 || lv.cells[e.1] < 0 {
            continue;
        }
        open.remove(&e);
        match walk(&open, e.0, e.1) {
            Some(c) if c >= 8 + 32 => {
                n += 1;
                saved += (c - 8) as usize / 8;
            }
            _ => {
                open.insert(e);
            }
        }
    }
    (n, saved)
}

/// `under` with TERRAIN set: the terrain tools on a copy of the site, every level (a chasm across the biggest room, or
/// the whole level if natural; then water, wet and flooded by turns; rope bridges and stepping stones by turns): what
/// they laid, the squares left unreached (0 expected) and the rules the site then breaks.
fn under_terrain(mut it: interior::Interior, id: &str, r: &mut Report) {
    use worldgen::under::{self, Ground, Span, Wetness};
    r.counts(&["terrain: levels", "terrain: unreached squares", "terrain: sites breaking a rule", "terrain: sites with keys out of order"]);
    let (nx, ny) = (it.nx, it.ny);
    let seed = worldgen::core::hash::fnv64(id.as_bytes());
    let t = std::time::Instant::now();
    for (li, lv) in it.levels.iter_mut().enumerate() {
        let s = worldgen::core::rng::hash2(seed, li as i64, 0);
        let biggest = (!lv.natural).then(|| (0..lv.rooms.len()).max_by_key(|&q| (lv.rooms[q].squares, std::cmp::Reverse(q))).map(|q| q as i16)).flatten();
        let span = if li % 2 == 0 { Span::Bridge } else { Span::Stones };
        let items = under::chasm(&Ground::of_level(lv, nx, ny), biggest, span, s ^ 1);
        lv.furniture.extend(items);
        let items = under::water(&Ground::of_level(lv, nx, ny), if (li + seed as usize).is_multiple_of(2) { Wetness::Wet } else { Wetness::Flooded }, s);
        lv.furniture.extend(items);
        r.tally("terrain: levels", 1);
        r.tally("terrain: unreached squares", Ground::of_level(lv, nx, ny).unreached());
        for f in &lv.furniture {
            let sq = (f.w * f.h) as usize;
            match f.kind {
                "water_shallow" | "water_deep" | "chasm" | "rope_bridge" | "stepping_stone" => r.tally(&format!("terrain: {} squares", f.kind), sq),
                _ => {}
            }
            if f.kind == "rope_bridge" {
                r.tally("terrain: rope bridges", 1);
            }
        }
    }
    r.measure("terrain: slowest site (ms)", false, t.elapsed().as_secs_f64() * 1e3, id.to_string());
    let bad: Vec<String> = if id.starts_with('u') { under::design::check(&it, None).into_iter().filter(|p| p.blocking).map(|p| p.text).collect() } else { Vec::new() };
    if !bad.is_empty() {
        r.add("terrain: sites breaking a rule", bad.len() as f64, format!("{id}: {}", bad.join("; ")));
    }
    let order = under::design::key_order(&it);
    if !order.is_empty() {
        r.add("terrain: sites with keys out of order", order.len() as f64, format!("{id}: {}", order.into_iter().map(|p| p.text).collect::<Vec<_>>().join("; ")));
    }
}

fn main() {
    const MODES: [&str; 14] = ["castles", "reach", "interiors", "hub", "halls", "palaces", "temples", "civic", "inns", "editor", "under", "hash", "piers", "all"];
    let args = common::args();
    let keeps = args.pos.iter().any(|a| a == "--keeps");
    let mut modes: Vec<&str> = args.pos.iter().filter_map(|a| MODES.iter().find(|m| **m == a.as_str()).copied()).collect();
    for a in args.pos.iter().filter(|a| !MODES.contains(&a.as_str()) && *a != "--keeps" && !common::is_world(a)) {
        eprintln!("{a}: not a mode, a seed or a world file, ignored");
    }
    if modes.is_empty() || modes.contains(&"all") {
        modes.retain(|m| *m != "all");
        for m in ["castles", "reach", "interiors", "hub", "halls", "palaces", "temples", "civic", "inns", "editor"] {
            if !modes.contains(&m) {
                modes.push(m);
            }
        }
    }
    QUIET.store(modes.contains(&"hash"), Ordering::Relaxed);
    let worlds: Vec<String> = args.pos.iter().filter(|a| common::is_world(a)).cloned().collect();
    let worlds = if worlds.is_empty() { vec!["1".into(), "2".into(), "3".into()] } else { worlds };
    let mut all: BTreeMap<&str, Report> = BTreeMap::new();
    let start = Instant::now();
    for arg in &worlds {
        let t = Instant::now();
        let mut ctx = Ctx { ex: Executor::new(common::world(arg)), chunks: HashMap::new() };
        let label = common::label(arg);
        say!("{label}: {} settlements, T0 {:.1} s", ctx.ex.t0.settlements.len(), t.elapsed().as_secs_f64());
        for &mode in &modes {
            let t = Instant::now();
            let mut r = Report::default();
            match mode {
                "castles" => castles(&mut ctx, &mut r),
                "reach" => reach(&mut ctx, &mut r),
                "interiors" => interiors(&mut ctx, keeps, &mut r),
                "hub" => hub(&mut ctx, &mut r),
                "halls" => halls(&mut ctx, &mut r),
                "palaces" => palaces(&mut ctx, &mut r),
                "temples" => temples(&mut ctx, &mut r),
                "civic" => civic(&mut ctx, &mut r),
                "inns" => inns(&mut ctx, &mut r),
                "piers" => piers(&mut ctx, &mut r),
                "editor" => editor(&mut ctx, &mut r),
                "under" => under(&mut ctx, &mut r, &label.replace(' ', "")),
                _ => hashes(&mut ctx, &label.replace(' ', ""), &mut r),
            }
            say!("  {mode} ({:.1} s)", t.elapsed().as_secs_f64());
            r.print();
            r.merge_into(all.entry(mode).or_default(), &label);
        }
    }
    if worlds.len() > 1 {
        say!("all {} worlds ({:.1} s)", worlds.len(), start.elapsed().as_secs_f64());
        for &mode in &modes {
            if mode != "hash" {
                say!("  {mode}");
                all.get_mut(mode).unwrap().print();
            }
        }
    }
}
