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
//!   Every town's quays with no water beyond them, walled towns' rings with no wall on dry ground,
//!   buildings over a river's channel and built blocks centred on one.
//! - `reach`: each castle's battlemap round it, flooded as play mode blocks movement (buildings,
//!   walls, towers, blocking objects, a hedge every square it is drawn across) from the window's
//!   edge and its streets, so from outside the outer gate: castle buildings cut off, outer gates
//!   not reached, bailey squares sealed (and the largest pocket); the same for every park.
//! - `yurts`: round homes' doors (`town::round_off`): how many, the share facing their lane (within 70 ft)
//!   or the centre within 30 degrees, the share still facing south, interior door squares more than 1.5
//!   square from the ring's door side, and up to 8 listed per world with seed, layout, id, x,y and angle.
//!   `cargo run --profile fast -p worldgen --example towncheck -- yurts [seeds]` (default seeds 1-8; `TOP=8`); a door over 1.5 squares off is the interior finding the ring's side shut.
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
//!   Then districts drawn by hand (`district_edits`; also `districts` alone): a patch into a
//!   neighbour's district, a far one, two into a new named one, a small district emptied, walls
//!   as generated and toggled; renames and notes kept, carried or reported when a patch is laid
//!   out anew.
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
//! - `walls` (not in `all`): walls by culture group (item 104, `town::wallstyle`): walled places and
//!   their wall, castle and corner towers by shape; square towers where the style has none; towers whose
//!   battlemap squares don't follow their shape (ring blocked, floor open, door reached); up to 3 places
//!   a group listed with cameras (`fx, fy, zoom`) on the whole wall and on a tower by a gate.
//! - `cemeteries` (not in `all`; R5 towns): cemeteries per tier and culture group, what they hold,
//!   the rules (props or tombs on a walk, over the boundary or outside the plot, tomb doors not
//!   reached from the way in, kurgans overlapping, a lychgate off the way in) and cameras; a park set
//!   to a cemetery by hand, walls on and off.
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
//! - `castlepieces` (not in `all`): a castle drawn by hand, its keep and yard buildings changed (`Created::pieces`),
//!   an earlier site deleted and taken out of the list, the castle in ruins; keeps of generated castles changed.
//!
//! Usage: cargo run --release -p worldgen --example towncheck -- [castlepieces|castles|reach|interiors|hub|halls|palaces|temples|editor|under|hash|all]... [seed|world.json...] [--keeps]
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
use worldgen::town::geom::{add, area, centroid, contains, cross, dist, dot, len, lerp, mean, mul, seg_dist, sub};
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
                    // (A curtain's bow, slice 2c: its 90th percentile too.)
                    if label.contains("curtain bow") {
                        say!("      90th percentile {:.1}", v[(v.len() * 9 / 10).min(v.len() - 1)].0);
                    }
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

struct Win {
    ox: f64,
    oy: f64,
    nx: usize,
    ny: usize,
    block: Vec<bool>,
    street: Vec<bool>,
    /// Every object (world ft) by name.
    objects: Vec<(P, String)>,
    /// (R5 towns) gravel squares, road or street squares, dry-stone walls' and hedges' runs (world ft).
    gravel: Vec<bool>,
    road: Vec<bool>,
    runs: Vec<(P, P)>,
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
    r.counts(&["moats: wet", "moats: dry", "castles without a moat", "wet moats under 90% water", "dry moats under 90% difficult", "drawbridges with no deck"]);
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
        moat_checks(ctx, &l, &tag, r);
    }
}

/// v63 castles: moats by castle (wet, dry, none; by tier) and on the battlemap: a wet moat's squares
/// under water (deep in its middle), a dry ditch's difficult, its drawbridge a deck.
fn moat_checks(ctx: &mut Ctx, l: &Layout, tag: &str, r: &mut Report) {
    let Some(m) = l.moats.first() else {
        r.tally("castles without a moat", 1);
        r.tally(&format!("{:?} castles without a moat", l.tier), 1);
        return;
    };
    let kind = if m.wet { "wet" } else { "dry" };
    let far = add(m.bridge.at, mul(m.bridge.dir, m.bridge.span));
    r.add(&format!("moats: {kind}"), m.width, format!("{tag}: {:.0} ft wide, drawbridge {} to {}", m.width, at(m.bridge.at), at(far)));
    r.tally(&format!("{:?} castles with a {kind} moat", l.tier), 1);
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    let k = |v: f64| (v / size).floor().max(0.0) as u32;
    let b = m.bbox;
    let (mut band, mut good, mut deep, mut deck) = (0, 0, 0, 0);
    for ky in k(b[1])..=k(b[3]) {
        for kx in k(b[0])..=k(b[2]) {
            let c = ctx.chunk(kx, ky);
            for q in 0..SQ * SQ {
                let p = [kx as f64 * size + ((q % SQ) as f64 + 0.5) * SQUARE_FT, ky as f64 * size + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                let Some(s) = m.spot(p) else { continue };
                if !s.inside() {
                    continue;
                }
                if m.bridge.covers(p) {
                    deck += (c.road_halo[(q / SQ + 1) * (SQ + 2) + q % SQ + 1] & 0x80 != 0) as usize;
                    continue;
                }
                band += 1;
                good += if m.wet { (c.water_level[q] > c.height[q]) as usize } else { (c.edges[q] & 16 != 0 || s.outer < town::moat::BANK_FT) as usize };
                deep += (c.surface[q] == Surface::Deep) as usize;
            }
        }
    }
    r.tally("moat squares", band);
    r.tally(&format!("moat squares deep water ({kind} moats)"), deep);
    if good * 10 < band * 9 {
        r.add(&format!("{kind} moats under 90% {}", if m.wet { "water" } else { "difficult" }), (band - good) as f64, format!("{tag}: {good} of {band} at {}", at(m.bridge.at)));
    }
    if deck == 0 {
        r.add("drawbridges with no deck", 1.0, format!("{tag}: at {}", at(m.bridge.at)));
    }
}

const TOWN_RING: [&str; 7] = [
    "buildings over a river channel",
    "built blocks centred on a river channel",
    "walled towns",
    "town-wall ring sides",
    "town-wall ring sides with 15 ft+ of dry ground and no wall (gates aside)",
    "quays",
    "quays with no water beyond them (a paved strip on dry ground)",
];

/// A town's quays all on the water, its buildings and blocks off the rivers' channels; and a walled
/// town's ring (the sides of its patches facing out of town, not those round water the town encloses):
/// every dry stretch of it has the wall on it (a town wall or a castle's curtain there), but at its gates.
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
    let in_channel = |p: P| river.iter().any(|&(a, b, hw)| seg_dist(p, a, b) < hw);
    let over_channel = |poly: &[P]| poly.len() >= 3 && (poly.iter().any(|&p| in_channel(p)) || in_channel(centroid(poly)));
    for b in l.buildings.iter().filter(|b| over_channel(&b.poly)) {
        r.add("buildings over a river channel", area(&b.poly).abs(), format!("{tag}: {} at {}", b.id, at(centroid(&b.poly))));
    }
    // (A block's corner may reach into the channel where the bank bends: its middle may not.)
    for b in l.blocks.iter().filter(|b| b.len() >= 3 && in_channel(centroid(b))) {
        r.add("built blocks centred on a river channel", area(b).abs(), format!("{tag}: block at {}", at(centroid(b))));
    }
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
    // (Ground the town encloses, a river or lake patch amid it: every way out of it, 16 of them,
    // meets a side of the town's patches within 2,000 ft.)
    let town_sides: Vec<(P, P)> = sides.iter().filter_map(|(a, b, _)| Some((*corners.get(a)?, *corners.get(b)?))).collect();
    let crosses = |p: P, q: P, a: P, b: P| {
        let (o1, o2) = (cross(sub(q, p), sub(a, p)), cross(sub(q, p), sub(b, p)));
        let (o3, o4) = (cross(sub(b, a), sub(p, a)), cross(sub(b, a), sub(q, a)));
        o1 * o2 < 0.0 && o3 * o4 < 0.0
    };
    let enclosed = |p: P| {
        (0..16).all(|k| {
            let a = k as f64 * std::f64::consts::TAU / 16.0;
            let q = add(p, [2000.0 * libm::cos(a), 2000.0 * libm::sin(a)]);
            town_sides.iter().any(|&(s, t)| crosses(p, q, s, t))
        })
    };
    for &(a, b, ccw) in sides.iter().filter(|(a, b, _)| !shared.contains(&(*b, *a))) {
        let (Some(&pa), Some(&pb)) = (corners.get(&a), corners.get(&b)) else { continue };
        r.tally("town-wall ring sides", 1);
        let u = mul(sub(pb, pa), 1.0 / dist(pa, pb).max(1e-9));
        let out = if ccw { [u[1], -u[0]] } else { [-u[1], u[0]] };
        if water_beyond(lerp(pa, pb, 0.5), out) {
            continue;
        }
        let (mut bare, mut run, mut first) = (0.0f64, 0.0, None);
        for (p, _) in samples(&[pa, pb], 5.0) {
            // (Behind a bastion: its gorge, the bastion's walls round it.)
            // (A round wall's curtains bowed out by up to 45 ft (Steppe 70), slice 2e: within 75 ft; or,
            // where a notch of the ring was bridged and bowed, the wall met out along the side's outward
            // normal within 200 ft.)
            let beyond = add(p, mul(out, 200.0));
            if by_gate(p) || by_water(p) || walls.iter().any(|&(q, s)| seg_dist(p, q, s) <= 75.0 || crosses(p, beyond, q, s)) || l.bastions.iter().any(|q| depth(q, p) > -8.0) {
                run = 0.0;
            } else {
                run += 5.0;
                bare = bare.max(run);
                first.get_or_insert(p);
            }
        }
        if bare >= 15.0 && !enclosed(add(lerp(pa, pb, 0.5), mul(out, 20.0))) {
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
        r.value("keep area (sq ft)", keep, format!("{tag} at {}", at(mid)));
    }
    // (Keep kinds, and the bailey's trades: stables and a smithy in every castle.)
    for b in l.buildings.iter().filter(|b| b.ward == Ward::Castle && contains(ring, centroid(&b.poly))) {
        if let Some(k) = b.keep {
            r.tally(&format!("keeps: {k:?}"), 1);
        }
    }
    let has = |key: &str| l.buildings.iter().any(|b| b.ward == Ward::Castle && contains(ring, centroid(&b.poly)) && b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == key));
    for key in ["stables", "blacksmith"] {
        if !has(key) {
            r.add(&format!("town castles with no {key}"), area(ring).abs(), format!("{tag}: bailey of {:.0} sq ft at {}", area(ring).abs(), at(mid)));
        }
    }
    let w = ctx.window(bbox(ring));
    if !w.objects.iter().any(|(p, name)| name == "well" && contains(ring, *p)) {
        r.add("town castles with no well", area(ring).abs(), format!("{tag}: bailey of {:.0} sq ft at {}", area(ring).abs(), at(mid)));
    }
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
    // (R5 towns: a cemetery's tomb is one chamber on one storey, `town::cemetery`.)
    let tomb = l.building(id.rsplit(':').next().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX)).is_some_and(|b| b.func == town::catalog::index_of("tomb").map(|f| f as u16));
    must!(it.levels.len() >= 2 || tomb || world.file.edits.designs.contains_key(id), "only {} level", it.levels.len());
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
        // (R5 towns: a steppe town's round felt halls checked by `steppe`.)
        let mut ids: Vec<String> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && (!keeps || is_keep(b)) && town::roof::felt_built(b).is_none()).map(|b| format!("b:{li}:{}", b.id)).collect();
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
                // (Its formal garden beside it: `palace::gardens`.)
                r.tally("palaces", 1);
                let garden = l.parks.iter().find(|pk| pk.kind == town::ParkKind::Formal && pk.pts.iter().any(|p| (0..b.poly.len()).any(|k| town::geom::seg_dist(*p, b.poly[k], b.poly[(k + 1) % b.poly.len()]) < 12.0)));
                if let Some(pk) = garden {
                    r.tally("palaces with a formal garden beside them", 1);
                    r.tally("capital palaces with a formal garden beside them", t0.settlements.get(li).is_some_and(|s| s.capital) as usize);
                    r.value("palace garden area (sq ft)", town::geom::area(&pk.pts).abs(), format!("b:{li}:{}", b.id));
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

/// A hand castle laid as a tower house (no bailey, its one building a small tower keep, on a patch
/// too small for a curtain): off the streets, walls and towers, on no building. False if not one.
fn tower_house(l: &Layout, tag: &str, r: &mut Report) -> bool {
    let keeps: Vec<&town::Building> = l.buildings.iter().filter(|b| b.ward == Ward::Castle).collect();
    if l.plazas.iter().any(|p| p.kind == PlazaKind::Bailey) || keeps.len() != 1 || keeps[0].keep != Some(town::KeepKind::Plain) {
        return false;
    }
    let b = keeps[0];
    let name = format!("{tag}: b:{}:{} at {}", l.index, b.id, at(centroid(&b.poly)));
    r.tally("placements as a tower house (a patch too small for a curtain)", 1);
    let wd = l.walls.iter().flat_map(|w| w.windows(2)).map(|s| if contains(&b.poly, s[0]) { -1.0 } else { seg_poly(s[0], s[1], &b.poly) }).fold(f64::MAX, f64::min);
    if wd < 4.0 {
        r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: {wd:.1} ft from a wall"));
    }
    let towers = l.towers.iter().chain(&l.gate_towers).chain(l.extra_towers.iter().map(|t| &t.0));
    if let Some(t) = towers.filter(|t| -depth(&b.poly, **t) < 12.0).next() {
        r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: a tower at {} within 12 ft", at(*t)));
    }
    if let Some(o) = l.buildings.iter().find(|o| o.ward != Ward::Castle && poly_overlap(&shrink(&b.poly, 0.5), &shrink(&o.poly, 0.5))) {
        r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: overlaps b:{}:{}", l.index, o.id));
    }
    let ways = l.roads.iter().map(|x| (&x.0, x.2)).chain(l.streets.iter().map(|s| (s, 18.0)));
    for (pts, w) in ways {
        let d = pts.windows(2).map(|s| if contains(&b.poly, s[0]) { 0.0 } else { seg_poly(s[0], s[1], &b.poly) }).fold(f64::MAX, f64::min);
        if d < 0.5 * w - 1.0 {
            r.add("tower houses within 4 ft of a wall, on a tower or a building, or on a street", 1.0, format!("{name}: {d:.1} ft from a way {w:.0} ft wide"));
        }
    }
    true
}

/// Castles set in the street editor at the town wall, walls on and off.
fn editor(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&[
        "placements",
        "placements with no castle",
        "placements as a tower house (a patch too small for a curtain)",
        "tower houses within 4 ft of a wall, on a tower or a building, or on a street",
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
                } else if tower_house(&l, &tag, r) {
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
    // R5 towns
    district_edits(base, t0, r);
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

/// R5 towns (103): where a village's lanes (`Layout::streets`) end and cross, both ends of each,
/// anywhere. An end is joined on a way (within 1 ft of another lane's or a road's centre line: the
/// network roads as `villages` samples them, and the layout's; a ring closed on itself); else at a
/// building (15 ft), a yard or croft (8 ft) or the water, or at nothing; one by the green (within 35 ft
/// of its rim) is also counted as not on its ring. Ends not joined 1-40 ft from a way's centre line are
/// near one; those whose last 40 ft cross a way's centre line and stop
/// past its far edge overshoot it. Crossings: a lane's piece across another lane's or a road's, but
/// within 1 ft of the lane's ends or of a node both share. Farmsteads: farmyards by count.
/// `LANE_DUMP=L<n>` prints that village's lanes, roads and what stands by each lane's ends (stderr);
/// `NEAR_LIST=1` every end near a way but not joined (on the ring or not, ft past the green's rim,
/// by water, the village's centre).
fn lane_ends(world: &World, t0: &T0, l: &Layout, tag: &str, r: &mut Report) {
    let off = |p: P| format!("({:.0}, {:.0})", p[0] - l.center[0], p[1] - l.center[1]);
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    let reach = 1.3 * l.radius;
    let c = l.center;
    let mut roads: Vec<Vec<(P, f64)>> = Vec::new();
    for (ri, k) in t0.roads.segments_near([c[0] - reach, c[1] - reach, c[0] + reach, c[1] + reach], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 40.0).ceil().clamp(1.0, 400.0) as usize;
        roads.push((0..=n).map(|j| (rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p, 0.5 * rc.class.width_ft())).collect());
    }
    let mut river: Vec<(P, P, f64)> = Vec::new();
    let rr = reach + 200.0;
    for (ri, k) in t0.rivers.segments_near(c[0] - rr, c[1] - rr, c[0] + rr, c[1] + rr, 0.0) {
        let rc = &t0.rivers.rivers[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(2.0, 1200.0) as usize;
        let pts: Vec<(P, f64)> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft)).map(|c| (c.p, c.w)).collect();
        river.extend(pts.windows(2).map(|s| (s[0].0, s[1].0, 0.5 * s[0].1.max(s[1].1))));
    }
    let by_water = |p: P| river.iter().any(|&(a, b, hw)| seg_dist(p, a, b) <= hw + 30.0) || wet(p) || [12.0, 25.0, 30.0].iter().any(|&d| (0..8).any(|k| wet(add(p, [d * libm::cos(k as f64 * 0.785), d * libm::sin(k as f64 * 0.785)]))));
    // (Every way as segments with half widths, and whose: the lane's index, or none for a road.)
    let mut segs: Vec<(P, P, f64, Option<usize>)> = Vec::new();
    for (si, s) in l.streets.iter().enumerate() {
        segs.extend(s.windows(2).map(|w| (w[0], w[1], 0.5 * town::village::LANE_W, Some(si))));
    }
    for q in &roads {
        segs.extend(q.windows(2).map(|w| (w[0].0, w[1].0, w[0].1, None)));
    }
    let n_cross = segs.len();
    segs.extend(l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w, None))));
    let green = l.plazas.iter().find(|g| g.kind == PlazaKind::Green).map(|g| (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0])));
    if std::env::var("LANE_DUMP").is_ok_and(|v| v == tag) {
        for (si, st) in l.streets.iter().enumerate() {
            eprintln!("lane {si}: {}", st.iter().map(|p| off(*p)).collect::<Vec<_>>().join(" "));
        }
        for q in &roads {
            eprintln!("road: {}", q.iter().map(|p| off(p.0)).collect::<Vec<_>>().join(" "));
        }
        eprintln!("green {:?} centre ({:.0}, {:.0})", green.map(|g| (off(g.0), g.1)), l.center[0], l.center[1]);
        for st in &l.streets {
            for e in [st[0], st[st.len() - 1]] {
                for b in l.buildings.iter().filter(|b| edge_dist(&b.poly, e) < 40.0) {
                    eprintln!("  end {} building {:?} {:?} {:.1} ft, {} pts", off(e), b.ward, b.func, edge_dist(&b.poly, e), b.poly.len());
                }
                for p in l.crofts.iter().filter(|p| edge_dist(&p.pts, e) < 40.0) {
                    eprintln!("  end {} plot {:?} {:.1} ft", off(e), p.kind, edge_dist(&p.pts, e));
                }
            }
        }
    }
    let hit = |a: P, b: P, p: P, q: P| -> Option<P> {
        let (e, f) = (sub(b, a), sub(q, p));
        let den = cross(e, f);
        if den.abs() < 1e-9 {
            return None;
        }
        let (t, u) = (cross(sub(p, a), f) / den, cross(sub(p, a), e) / den);
        (t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0).then(|| add(a, mul(e, t)))
    };
    for (si, s) in l.streets.iter().enumerate() {
        if s.len() < 2 {
            continue;
        }
        for end in [0, 1] {
            r.tally("lane ends", 1);
            let (e, way): (P, Vec<P>) = if end == 0 { (s[0], s.to_vec()) } else { (s[s.len() - 1], s.iter().rev().copied().collect()) };
            let others = || segs.iter().filter(|w| w.3 != Some(si));
            let d = others().map(|w| seg_dist(e, w.0, w.1)).fold(f64::MAX, f64::min);
            // (A ring round the green closed on itself: its ends one node.)
            if d <= 1.0 || dist(s[0], s[s.len() - 1]) < 0.01 {
                r.tally("lane ends joined", 1);
                continue;
            }
            // (Its last 40 ft across a way and past its far edge.)
            let mut back = vec![way[0]];
            let mut acc = 0.0;
            for w in way.windows(2) {
                let k = dist(w[0], w[1]);
                if acc + k >= 40.0 {
                    back.push(lerp(w[0], w[1], (40.0 - acc) / k.max(1e-9)));
                    break;
                }
                back.push(w[1]);
                acc += k;
            }
            let start = way[way.len() - 1];
            let over = others().find(|w| back.windows(2).any(|b| hit(b[0], b[1], w.0, w.1).is_some_and(|x| dist(x, e) > 1.0 && dist(x, start) > 1.0)) && seg_dist(e, w.0, w.1) > w.2);
            if let Some(w) = over {
                r.add("lane ends overshooting a way", 1.0, format!("{tag} {} by {:.0} ft (lane {si}, {} pts, {:.0} ft)", off(e), seg_dist(e, w.0, w.1) - w.2, s.len(), s.windows(2).map(|q| dist(q[0], q[1])).sum::<f64>()));
            }
            // (Within the ring lane's reach of the green: R5 towns 103, lanes there end on the ring.)
            let ring = green.is_some_and(|(gc, gr)| s.len() >= 4 && s[1..s.len() - 1].iter().all(|p| (dist(*p, gc) - gr - 0.5 * town::village::LANE_W).abs() < 2.0));
            if ring {
                r.add("ring lane ends not on a way", 1.0, format!("{tag} {} (lane {si}, way {d:.0} ft{})", off(e), if by_water(e) { ", water" } else { "" }));
            } else if green.is_some_and(|(gc, gr)| dist(e, gc) <= gr + town::village::LANE_W + 25.0) {
                r.add("lane ends at the green's rim but not on the ring", 1.0, format!("{tag} {} (lane {si}, way {d:.0} ft{})", off(e), if by_water(e) { ", water" } else { "" }));
            }
            let at_building = l.buildings.iter().any(|b| contains(&b.poly, e) || edge_dist(&b.poly, e) <= 15.0);
            let at_yard = l.crofts.iter().any(|p| contains(&p.pts, e) || edge_dist(&p.pts, e) <= 8.0);
            if d <= 40.0 && std::env::var("NEAR_LIST").is_ok() {
                eprintln!("NEAR {tag} {} {d:.0} ring {ring} green {:.0} water {} centre ({:.0}, {:.0})", off(e), green.map_or(-1.0, |(gc, gr)| dist(e, gc) - gr), by_water(e), l.center[0], l.center[1]);
            }
            if d <= 40.0 {
                r.add("lane ends near a way but not joined", 1.0, format!("{tag} {} {d:.0} ft off (lane {si}, {} pts, {}{}{})", off(e), s.len(), if at_building { "house " } else { "" }, if at_yard { "yard " } else { "" }, if by_water(e) { "water" } else { "" }));
            }
            if !at_building && !at_yard && !by_water(e) {
                let near_b = l.buildings.iter().map(|b| edge_dist(&b.poly, e)).fold(f64::MAX, f64::min);
                let near_p = l.crofts.iter().map(|p| edge_dist(&p.pts, e)).fold(f64::MAX, f64::min);
                r.add("lane ends not joined, at nothing", 1.0, format!("{tag} {} {} ft out (lane {si}, {} pts, way {d:.0} ft, building {near_b:.0}, plot {near_p:.0})", off(e), dist(e, l.center) as i64, s.len()));
            }
        }
        // (Across a later lane or a road.)
        for w in s.windows(2) {
            for o in segs[..n_cross].iter().filter(|o| o.3.is_none_or(|j| j > si)) {
                let Some(x) = hit(w[0], w[1], o.0, o.1) else { continue };
                if dist(x, s[0]) <= 1.0 || dist(x, s[s.len() - 1]) <= 1.0 {
                    continue;
                }
                // (A road across a lane at a point of the lane's own: a crossroads.)
                if o.3.is_none() && s.iter().any(|p| dist(*p, x) <= 1.0) {
                    continue;
                }
                if let Some(j) = o.3 {
                    let q = &l.streets[j];
                    if dist(x, q[0]) <= 1.0 || dist(x, q[q.len() - 1]) <= 1.0 || (s.iter().any(|p| dist(*p, x) <= 1.0) && q.iter().any(|p| dist(*p, x) <= 1.0)) {
                        continue;
                    }
                }
                r.add("lanes crossing a lane or road away from a shared node", 1.0, format!("{tag} {} {} (lane {si} {} to {})", off(x), if o.3.is_some() { "lane" } else { "road" }, off(s[0]), off(s[s.len() - 1])));
            }
        }
    }
    r.tally("farmsteads", l.crofts.iter().filter(|p| format!("{:?}", p.kind) == "Farmyard").count());
}

/// `NEAR_DUMP=L<n>:<x>,<y>,<r>` prints what stands within `r` ft of the offset (x, y) from that
/// village's middle: buildings, plots, lanes, roads, `rural::battle` runs and fields (stderr).
fn near_dump(l: &Layout, bt: &worldgen::town::rural::Battle, tag: &str) {
    let Ok(v) = std::env::var("NEAR_DUMP") else { return };
    let Some((t, rest)) = v.split_once(':') else { return };
    let n: Vec<f64> = rest.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if t != tag || n.len() < 3 {
        return;
    }
    let (q, rad) = (add(l.center, [n[0], n[1]]), n[2]);
    let off = |p: P| format!("({:.0},{:.0})", p[0] - l.center[0], p[1] - l.center[1]);
    let poly = |ps: &[P]| ps.iter().map(|p| off(*p)).collect::<Vec<_>>().join(" ");
    let near = |ps: &[P]| contains(ps, q) || edge_dist(ps, q) < rad;
    for (i, b) in l.buildings.iter().enumerate().filter(|(_, b)| near(&b.poly)) {
        eprintln!("bldg {i} c {} {:?} {:?} {:?} {}", off(centroid(&b.poly)), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.structure, poly(&b.poly));
    }
    for (i, p) in l.crofts.iter().enumerate().filter(|(_, p)| near(&p.pts)) {
        let held = l.buildings.iter().filter(|b| contains(&p.pts, centroid(&b.poly))).count();
        eprintln!("plot {i} {:?} {:?} holds {held}: {}", p.kind, p.bound, poly(&p.pts));
    }
    for (i, s) in l.streets.iter().enumerate().filter(|(_, s)| s.windows(2).any(|w| seg_dist(q, w[0], w[1]) < rad)) {
        eprintln!("lane {i}: {}", poly(s));
    }
    for (pts, _, w) in l.roads.iter().filter(|(s, _, _)| s.windows(2).any(|w| seg_dist(q, w[0], w[1]) < rad)) {
        eprintln!("layout road w {w:.0}: {}", poly(pts));
    }
    for &(a, b, k) in bt.runs.iter().filter(|r| seg_dist(q, r.0, r.1) < rad) {
        eprintln!("run {k:?} {} {}", off(a), off(b));
    }
    for (i, f) in l.fields.iter().enumerate().filter(|(_, f)| near(f)) {
        let x = &l.field_info[i];
        eprintln!("field {i} {:?} {:?}: {}", x.kind, x.bound, poly(f));
    }
}

/// R5 towns (103 fix): plots (crofts, yards) holding no building; farmyards with no lane to their
/// gate; a plot's sides and back (as drawn, a croft's front open) within 6 ft (a yard's 4) of a building it does
/// not hold, one in no other plot (a cottage beside it with no croft of its own, a hedge across its
/// corner); the battlemap's runs
/// (hedges, fences, walls, field rows) within 2 ft of any building, roofed or not; lanes through a
/// building (the lane's band over it).
fn plot_checks(l: &Layout, bt: &worldgen::town::rural::Battle, tag: &str, r: &mut Report) {
    let off = |p: P| format!("({:.0}, {:.0})", p[0] - l.center[0], p[1] - l.center[1]);
    // (A footprint's middle by its corners' mean: `centroid` loses a round home's small area in world ft.)
    let mid = |q: &[P]| mul(q.iter().fold([0.0, 0.0], |s, p| add(s, *p)), 1.0 / q.len().max(1) as f64);
    let holds = |p: &town::village::Plot, b: &town::Building| contains(&p.pts, mid(&b.poly));
    for p in &l.crofts {
        if !l.buildings.iter().any(|b| holds(p, b)) {
            r.add("plots holding no building", 1.0, format!("{tag} {:?} {}", p.kind, off(mid(&p.pts))));
        }
        let n = p.pts.len();
        let first = if p.kind == town::village::PlotKind::Croft { 1 } else { 0 };
        for k in first..n {
            let (a, e) = (p.pts[k], p.pts[(k + 1) % n]);
            for b in l.buildings.iter().filter(|b| !holds(p, b)) {
                // (A croft's 6 ft; a yard's 4, the cottages laid out after it: `village::off_yards`.)
                let clear = if p.kind == town::village::PlotKind::Croft { 6.0 } else { 4.0 };
                if seg_poly(a, e, &b.poly) >= clear && !contains(&b.poly, a) {
                    continue;
                }
                // (In a plot of its own: that plot keeps its bounds off it, along this one's or not.)
                let shared = l.crofts.iter().any(|q| !std::ptr::eq(q, p) && holds(q, b));
                if !shared {
                    r.add("plot bounds by a building not theirs", 1.0, format!("{tag} {:?} side {} {} by {:?} {:?} at {:.1} ft", p.kind, off(a), off(e), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key), seg_poly(a, e, &b.poly)));
                }
            }
        }
    }
    for p in l.crofts.iter().filter(|p| p.kind == town::village::PlotKind::Farmyard) {
        let gate = lerp(p.pts[0], p.pts[1], 0.5);
        if !l.streets.iter().any(|s| s.windows(2).any(|w| seg_dist(gate, w[0], w[1]) < 8.0)) {
            r.add("farmyards with no lane to their gate", 1.0, format!("{tag} {}", off(gate)));
        }
    }
    // (A churchyard's graveyard is a building of its own, open: its wall runs round it.)
    let graveyard = town::catalog::index_of("graveyard").map(|f| f as u16);
    for &(a, e, k) in &bt.runs {
        if let Some(b) = l.buildings.iter().filter(|b| b.func.is_none() || b.func != graveyard).find(|b| contains(&b.poly, a) || contains(&b.poly, e) || seg_poly(a, e, &b.poly) < 2.0) {
            r.add("runs by any building", 1.0, format!("{tag} {k:?} {} {} by {:?} {:?} {:?}", off(a), off(e), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.structure));
        }
    }
    for (si, s) in l.streets.iter().enumerate() {
        for w in s.windows(2) {
            if let Some(b) = l.buildings.iter().find(|b| contains(&b.poly, w[0]) || seg_poly(w[0], w[1], &b.poly) < 0.5 * town::village::LANE_W - 0.5) {
                r.add("lanes through a building", 1.0, format!("{tag} lane {si} {} {} by {:?} {:?}", off(w[0]), off(w[1]), b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key)));
                break;
            }
        }
    }
}

/// `villages`: v63 village layouts (not part of `all`). Per village: its cottages, how many stand
/// along each 100 ft of lane, those not square to the lane they front (> 8°), crofts overlapping
/// another or a building they do not hold or a lane; the battlemap's bounds (hedges, fences, walls:
/// `rural::battle`) within 2 ft of a roof or across a lane; hard kinks (> 50°) in the lanes; the green's
/// props. Places are offsets from the village's middle (ft).
fn villages(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["villages", "cottages", "cottages not square to their lane", "croft overlaps", "crofts over a building", "crofts over a lane", "bounds by a roof", "bounds across a lane", "lane kinks", "lanes ending at nothing inside the village", "greens with props", "greens without their well (of the first 24)"]);
    r.counts(&["cottages by no lane or road", "lane ends", "lane ends joined", "lane ends not joined, at nothing", "lane ends near a way but not joined", "lane ends overshooting a way", "lanes crossing a lane or road away from a shared node", "farmsteads"]);
    r.counts(&["plots holding no building", "farmyards with no lane to their gate", "plot bounds by a building not theirs", "runs by any building", "lanes through a building"]);
    r.counts(&["greens without a ring lane", "lanes across the green", "lane ends at the green's rim but not on the ring", "ring lane ends not on a way"]);
    r.counts(&["cottages facing the ring", "rings with fewer than 3 cottages facing them", "ways ending inside the green"]);
    let mut greens: Vec<(P, String)> = Vec::new();
    let mut gpolys: Vec<Vec<P>> = Vec::new();
    let mut fchunks: Vec<(P, String)> = Vec::new();
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.tier != worldgen::t0::settle::Tier::Village || l.site {
            continue;
        }
        let tag = format!("L{li}");
        let off = |p: P| format!("({:.0}, {:.0})", p[0] - l.center[0], p[1] - l.center[1]);
        r.tally("villages", 1);
        // (Lanes: the village's own and its roads, as segments with their half widths.)
        let mut ways: Vec<(P, P, f64)> = l.streets.iter().flat_map(|s| s.windows(2).map(|w| (w[0], w[1], 0.5 * town::village::LANE_W))).collect();
        ways.extend(l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w))));
        let reach = 1.3 * l.radius;
        let c = l.center;
        for (ri, k) in t0.roads.segments_near([c[0] - reach, c[1] - reach, c[0] + reach, c[1] + reach], 0.0) {
            let rc = &t0.roads.roads[ri as usize];
            let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 40.0).ceil().clamp(1.0, 400.0) as usize;
            let pts: Vec<P> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p).collect();
            ways.extend(pts.windows(2).map(|q| (q[0], q[1], 0.5 * rc.class.width_ft())));
        }
        let lane_ft: f64 = ways.iter().filter(|&&(a, b, _)| dist(lerp(a, b, 0.5), l.center) < l.radius).map(|&(a, b, _)| dist(a, b)).sum();
        let roofed: Vec<&town::Building> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed).collect();
        let cottages: Vec<&&town::Building> = roofed.iter().filter(|b| b.func.is_none() && b.ward == Ward::Rural).collect();
        r.tally("cottages", cottages.len());
        let fronting = roofed.iter().filter(|b| ways.iter().any(|&(a, e, h)| seg_poly(a, e, &b.poly) < h + 14.0)).count();
        r.tally("cottages by no lane or road", cottages.iter().filter(|b| !ways.iter().any(|&(a, e, h)| seg_poly(a, e, &b.poly) < h + 14.0)).count());
        if lane_ft > 150.0 {
            r.measure("houses fronting a lane per 100 ft of lane (fewest)", true, 100.0 * fronting as f64 / lane_ft.max(1.0), format!("{tag} {} houses, {lane_ft:.0} ft", roofed.len()));
            r.value("houses fronting a lane per 100 ft of lane", 100.0 * fronting as f64 / lane_ft, tag.clone());
        }
        r.measure("cottages (fewest)", true, cottages.len() as f64, tag.clone());
        let mill = town::catalog::index_of("mill").map(|f| f as u16);
        for b in &roofed {
            if b.poly.len() != 4 || (b.func.is_some() && b.func == mill) {
                continue;
            }
            let n_lane = l.streets.iter().map(|q| q.len().saturating_sub(1)).sum::<usize>();
            // (Nearest by its gap; a tie, where a house's front runs past a bend in the lane, to the piece by its middle.)
            let key = |a: P, e: P| seg_poly(a, e, &b.poly) + 0.05 * seg_dist(centroid(&b.poly), a, e);
            let Some((wi, &(a, e, h))) = ways.iter().enumerate().filter(|&(_, &(a, e, h))| seg_poly(a, e, &b.poly) < h + 10.0).min_by(|x, y| key(x.1.0, x.1.1).total_cmp(&key(y.1.0, y.1.1))) else { continue };
            let u = sub(e, a);
            let s0 = sub(b.poly[1], b.poly[0]);
            let c = (dot(u, s0) / (len(u) * len(s0)).max(1e-9)).abs();
            // (Either side square to it: the angle off the nearer axis.)
            let deg = libm::acos(c.min(1.0)).to_degrees();
            let deg = deg.min(90.0 - deg);
            if deg > 8.0 {
                let held = l.crofts.iter().any(|p| contains(&p.pts, centroid(&b.poly)));
                r.add("cottages not square to their lane", deg, format!("{tag} {} {deg:.0}° func {:?} croft {held} gap {:.1} by a {}, {:.0} ft from a lane's end", off(centroid(&b.poly)), b.func.map(|f| town::catalog::CATALOG[f as usize].key), seg_poly(a, e, &b.poly) - h, if wi < n_lane { "lane" } else { "road" }, l.streets.iter().flat_map(|q| [q[0], q[q.len() - 1]]).map(|q| dist(q, centroid(&b.poly))).fold(f64::MAX, f64::min)));
            }
        }
        for (i, p) in l.crofts.iter().enumerate() {
            let core = shrink(&p.pts, 1.0);
            for q in l.crofts.iter().skip(i + 1) {
                if poly_overlap(&core, &shrink(&q.pts, 1.0)) {
                    r.add("croft overlaps", 1.0, format!("{tag} {:?} {:?} {}", p.kind, q.kind, off(centroid(&p.pts))));
                }
            }
            for b in &l.buildings {
                if !b.poly.iter().all(|c| contains(&p.pts, *c) || edge_dist(&p.pts, *c) < 0.6) && poly_overlap(&core, &b.poly) {
                    r.add("crofts over a building", 1.0, format!("{tag} {:?} {} func {:?} {:?} {:.0} sq ft", p.kind, off(centroid(&b.poly)), b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.structure, area(&b.poly).abs()));
                }
            }
            if let Some(&(a, e, _)) = ways.iter().find(|&&(a, e, h)| seg_poly(a, e, &core) < h - 0.5 || contains(&core, a)) {
                r.add("crofts over a lane", 1.0, format!("{tag} {:?} {}", p.kind, off(lerp(a, e, 0.5))));
            }
        }
        let mut bt = worldgen::town::rural::Battle::default();
        let bb = l.crofts.iter().flat_map(|p| p.pts.iter()).fold([l.center[0] - 10.0, l.center[1] - 10.0, l.center[0] + 10.0, l.center[1] + 10.0], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        worldgen::town::rural::battle(&l, t0, [bb[0] - 20.0, bb[1] - 20.0, bb[2] + 20.0, bb[3] + 20.0], &|_| false, &mut bt);
        near_dump(&l, &bt, &tag);
        plot_checks(&l, &bt, &tag, r);
        for &(a, e, _) in &bt.runs {
            let pts: Vec<P> = samples(&[a, e], 1.0).into_iter().map(|s| s.0).collect();
            if let Some(p) = pts.iter().find(|p| roofed.iter().any(|b| contains(&b.poly, **p) || edge_dist(&b.poly, **p) < 2.0)) {
                r.add("bounds by a roof", 1.0, format!("{tag} {}", off(*p)));
            }
            if let Some(p) = pts.iter().find(|p| ways.iter().any(|&(s, t, h)| seg_dist(**p, s, t) < h)) {
                r.add("bounds across a lane", 1.0, format!("{tag} {}", off(*p)));
            }
        }
        for (si, s) in l.streets.iter().enumerate() {
            let e = s[s.len() - 1];
            let met = l.streets.iter().enumerate().any(|(j, q)| j != si && q.windows(2).any(|w| seg_dist(e, w[0], w[1]) < 15.0)) || ways.iter().skip(l.streets.iter().map(|q| q.len() - 1).sum()).any(|&(a, b, h)| seg_dist(e, a, b) < h + 15.0);
            // (A ring closed on itself has no end.)
            if !met && dist(s[0], e) > 0.01 && dist(e, l.center) < 0.9 * l.radius && !l.buildings.iter().any(|b| contains(&b.poly, e) || edge_dist(&b.poly, e) < 20.0) && !l.crofts.iter().any(|p| edge_dist(&p.pts, e) < 12.0) {
                r.add("lanes ending at nothing inside the village", 1.0, format!("{tag} {}", off(e)));
            }
            for w in s.windows(3) {
                let (u, v) = (sub(w[1], w[0]), sub(w[2], w[1]));
                if len(u) < 2.0 || len(v) < 2.0 {
                    continue;
                }
                let deg = libm::acos((dot(u, v) / (len(u) * len(v))).clamp(-1.0, 1.0)).to_degrees();
                if deg > 50.0 {
                    r.add("lane kinks", deg, format!("{tag} {} {deg:.0}° legs {:.0} {:.0}", off(w[1]), len(u), len(v)));
                }
            }
        }
        lane_ends(world, t0, &l, &tag, r);
        fields_rows(&l, t0, &tag, r);
        if fchunks.len() < 8 {
            if let Some(f) = l.fields.iter().enumerate().filter(|(i, _)| l.field_info[*i].kind != town::fields::FieldUse::Orchard).max_by(|a, b| area(a.1).abs().total_cmp(&area(b.1).abs())) {
                fchunks.push((centroid(f.1), tag.clone()));
            }
        }
        if let Some(g) = l.plazas.iter().find(|p| p.kind == PlazaKind::Green) {
            let (c, rad) = (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0]));
            gpolys.push(g.pts.clone());
            // (R5 towns, 103: the lane round it, its points but its ends on its centre line; no lane on its grass.)
            let rr = rad + 0.5 * town::village::LANE_W;
            if !l.streets.iter().any(|s| s.len() >= 4 && s[1..s.len() - 1].iter().all(|p| (dist(*p, c) - rr).abs() < 2.0)) {
                r.add("greens without a ring lane", 1.0, format!("{tag} green {} r {rad:.0}, {} roads by", off(c), l.roads.len()));
            }
            for s in &l.streets {
                if let Some(w) = s.windows(2).find(|w| seg_dist(c, w[0], w[1]) < rad - 1.0) {
                    r.add("lanes across the green", 1.0, format!("{tag} {} to {}", off(w[0]), off(w[1])));
                }
            }
            // (Cottages facing the ring: front within 15 ft of a ring lane's edge, square to its nearest piece within 15°, on its outer side.)
            let on_c = |p: P| (dist(p, c) - rr).abs() < 2.0;
            let pieces: Vec<(P, P)> = l.streets.iter().flat_map(|s| s.windows(2).filter(|w| on_c(w[0]) && on_c(w[1])).map(|w| (w[0], w[1]))).collect();
            if !pieces.is_empty() {
                // (Houses: the cottages and those given a trade, an inn or a smithy; not the church, shrine or mill.)
                let house = |b: &&&town::Building| b.func.is_none_or(|f| !["church", "chapel", "shrine", "mill", "graveyard", "watermill", "windmill"].iter().any(|k| town::catalog::CATALOG[f as usize].key.contains(k)));
                let facing = roofed.iter().filter(house).filter(|b| b.poly.len() == 4 && dist(centroid(&b.poly), c) > rr).filter(|b| {
                    let key = |a: P, e: P| seg_poly(a, e, &b.poly) + 0.05 * seg_dist(centroid(&b.poly), a, e);
                    let Some((a, e)) = pieces.iter().copied().min_by(|x, y| key(x.0, x.1).total_cmp(&key(y.0, y.1))) else { return false };
                    let (u, s0) = (sub(e, a), sub(b.poly[1], b.poly[0]));
                    let deg = libm::acos((dot(u, s0) / (len(u) * len(s0)).max(1e-9)).abs().min(1.0)).to_degrees();
                    seg_poly(a, e, &b.poly) - 0.5 * town::village::LANE_W <= 15.0 && deg.min(90.0 - deg) <= 15.0
                }).count();
                r.tally("cottages facing the ring", facing);
                if std::env::var("FACING_LIST").is_ok() {
                    eprintln!("facing {tag} {facing} green {} r {rad:.0} centre {:.0} {:.0}", off(c), l.center[0], l.center[1]);
                }
                r.value("cottages facing a ring", facing as f64, tag.clone());
                if facing < 3 {
                    r.add("rings with fewer than 3 cottages facing them", 1.0, format!("{tag} {facing} facing, green {} r {rad:.0}", off(c)));
                }
            }
            // (Ways ending inside the green, on no other way: lanes, the layout's roads and the network's.)
            let mut ends: Vec<P> = l.streets.iter().flat_map(|s| [s[0], s[s.len() - 1]]).collect();
            ends.extend(l.roads.iter().flat_map(|(q, _, _)| [q[0], q[q.len() - 1]]));
            let mut net: Vec<Vec<P>> = Vec::new();
            for (ri, k) in t0.roads.segments_near([c[0] - rad, c[1] - rad, c[0] + rad, c[1] + rad], 0.0) {
                let rc = &t0.roads.roads[ri as usize];
                let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 10.0).ceil().clamp(1.0, 400.0) as usize;
                net.push((0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p).collect());
                if k == 0 {
                    ends.push(rc.eval(0, 0.0, 2.5, t0.cell_ft).p);
                }
                if k as usize + 2 == rc.pts.len() {
                    ends.push(rc.eval(k as usize, 1.0, 2.5, t0.cell_ft).p);
                }
            }
            let all: Vec<(P, P)> = l.streets.iter().chain(l.roads.iter().map(|q| &q.0)).chain(net.iter()).flat_map(|s| s.windows(2).map(|w| (w[0], w[1]))).collect();
            for e in ends.into_iter().filter(|e| dist(*e, c) < rad - 2.0) {
                // (An end shared with another way's piece passing it is a joint, not a dead end.)
                let by = all.iter().filter(|&&(a, b)| seg_dist(e, a, b) < 1.0).count();
                let mine = all.iter().filter(|&&(a, b)| dist(a, e) < 1.0 || dist(b, e) < 1.0).count();
                if by <= mine.max(1) && mine <= 1 {
                    r.add("ways ending inside the green", 1.0, format!("{tag} {} green {} r {rad:.0}", off(e), off(c)));
                }
            }
            if l.props.iter().any(|p| dist(p.at, c) < rad) {
                r.tally("greens with props", 1);
            }
            if greens.len() < 24 {
                greens.push((c, tag.clone()));
            }
        }
    }
    // (The first two dozen greens' wells on the battlemap.)
    let size = ctx.ex.world.geom.tile_size_ft(ctx.ex.world.geom.max_level);
    // (Field cover: in the chunk under each of the first villages' biggest field, the 32x32 windows
    // mostly of field squares, every 4 squares.)
    for (c, tag) in fchunks {
        let (kx, ky) = ((c[0] / size).floor() as u32, (c[1] / size).floor() as u32);
        let ch = ctx.chunk(kx, ky);
        let field = |k: usize| ch.urban[k] == worldgen::battlemap::URBAN_FIELD;
        let hedge = |k: usize| ch.yard.get(k).is_some_and(|&y| y & 8 != 0);
        for o in ch.objects.iter().filter(|o| worldgen::battlemap::info(o.kind as u16).cover >= 1) {
            let (x, y) = ((o.x as usize).min(SQ - 1), (o.y as usize).min(SQ - 1));
            if !field(y * SQ + x) {
                continue;
            }
            let edge = (-2i64..=2).any(|dy| (-2i64..=2).any(|dx| {
                let (i, j) = (x as i64 + dx, y as i64 + dy);
                i < 0 || j < 0 || i >= SQ as i64 || j >= SQ as i64 || { let q = j as usize * SQ + i as usize; !field(q) || hedge(q) }
            }));
            // (Orchards' and copses' trees aside.)
            if !matches!(o.kind, Kind::Bush | Kind::RockPile | Kind::Thicket | Kind::WaterTrough) {
                continue;
            }
            r.tally(if edge { "field cover by an edge or hedge" } else { "field cover mid-field" }, 1);
        }
        let dist = worldgen::battlemap::cover_distance(&ch);
        let far = (0..SQ * SQ).filter(|&k| field(k) && ch.water_level[k] <= ch.height[k]).map(|k| dist[k]).max().unwrap_or(0);
        r.measure("field square farthest from cover in a chunk (squares)", false, far as f64, tag.clone());
        for wy in (0..=SQ - 32).step_by(4) {
            for wx in (0..=SQ - 32).step_by(4) {
                let n = (0..32 * 32).filter(|&q| field((wy + q / 32) * SQ + wx + q % 32)).count();
                if n * 2 < 32 * 32 {
                    continue;
                }
                let inw = |o: &worldgen::battlemap::Object| o.x >= wx as f32 && o.x < (wx + 32) as f32 && o.y >= wy as f32 && o.y < (wy + 32) as f32;
                let cover = ch.objects.iter().filter(|o| worldgen::battlemap::info(o.kind as u16).cover >= 1 && inw(o)).count();
                r.value("cover objects per 32x32 field window", cover as f64, format!("{tag} ({wx},{wy})"));
            }
        }
    }
    // (What stands on the greens: objects by kind, of the first two dozen.)
    for g in gpolys.iter().take(24) {
        let c = centroid(g);
        let (kx, ky) = ((c[0] / size).floor() as u32, (c[1] / size).floor() as u32);
        let ch = ctx.chunk(kx, ky);
        let (ox, oy) = (kx as f64 * size, ky as f64 * size);
        r.tally("hedge or wall squares on greens (of 24)", (0..SQ * SQ).filter(|&k| ch.yard.get(k).is_some_and(|&y| y & 24 == 24)).count());
        for o in &ch.objects {
            if contains(g, [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT]) {
                r.tally(&format!("on greens: {:?}", o.kind), 1);
            }
        }
    }
    for (c, tag) in greens {
        let (kx, ky) = ((c[0] / size).floor() as u32, (c[1] / size).floor() as u32);
        let ch = ctx.chunk(kx, ky);
        let (ox, oy) = (kx as f64 * size, ky as f64 * size);
        if !ch.objects.iter().any(|o| o.kind == Kind::Well && dist([ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT], c) < 6.0) {
            r.add("greens without their well (of the first 24)", 1.0, tag);
        }
    }
}

/// The fields' rows (`rural::rows`) and how far their squares lie from a hedgerow or wall (bounds,
/// rows, crofts' hedges: `rural::battle` over the layout's fields, chunk by chunk, roads not cut).
fn fields_rows(l: &Layout, t0: &T0, tag: &str, r: &mut Report) {
    use worldgen::town::fields::{FieldBound, FieldUse};
    use worldgen::town::rural;
    let live: Vec<usize> = (0..l.fields.len()).filter(|&i| l.field_info.get(i).is_some_and(|x| x.kind != FieldUse::Orchard)).collect();
    if live.is_empty() {
        return;
    }
    let bb = live.iter().flat_map(|&i| l.fields[i].iter()).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let g0 = [(bb[0] / 5.0).floor() * 5.0 - 100.0, (bb[1] / 5.0).floor() * 5.0 - 100.0];
    let (nx, ny) = (((bb[2] - g0[0]) / 5.0) as usize + 21, ((bb[3] - g0[1]) / 5.0) as usize + 21);
    let mut d = vec![i32::MAX; nx * ny];
    let mut q = VecDeque::new();
    let blk = 640.0;
    let (bx0, by0) = ((g0[0] / blk).floor() as i64, (g0[1] / blk).floor() as i64);
    let (bx1, by1) = (((g0[0] + nx as f64 * 5.0) / blk).floor() as i64, ((g0[1] + ny as f64 * 5.0) / blk).floor() as i64);
    for by in by0..=by1 {
        for bx in bx0..=bx1 {
            let mut bt = rural::Battle::default();
            let rect = [bx as f64 * blk, by as f64 * blk, (bx + 1) as f64 * blk, (by + 1) as f64 * blk];
            rural::battle(l, t0, rect, &|_| false, &mut bt);
            r.measure("runs in a chunk (most, of the 900 a chunk holds)", false, bt.runs.len() as f64, format!("{tag} block ({bx},{by})"));
            for &(a, e, kind) in &bt.runs {
                if kind == rural::RunKind::Fence {
                    continue;
                }
                let m = (dist(a, e) / 1.25).ceil().max(1.0) as usize;
                for j in 0..=m {
                    let p = lerp(a, e, j as f64 / m as f64);
                    let (i, k) = (((p[0] - g0[0]) / 5.0).floor() as i64, ((p[1] - g0[1]) / 5.0).floor() as i64);
                    if i >= 0 && k >= 0 && (i as usize) < nx && (k as usize) < ny && d[k as usize * nx + i as usize] != 0 {
                        d[k as usize * nx + i as usize] = 0;
                        q.push_back(k as usize * nx + i as usize);
                    }
                }
            }
        }
    }
    while let Some(k) = q.pop_front() {
        let (x, y) = ((k % nx) as i64, (k / nx) as i64);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (i, j) = (x + dx, y + dy);
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && d[j as usize * nx + i as usize] > d[k] + 1 {
                d[j as usize * nx + i as usize] = d[k] + 1;
                q.push_back(j as usize * nx + i as usize);
            }
        }
    }
    for &i in &live {
        let f = &l.fields[i];
        let info = &l.field_info[i];
        let rows = rural::rows(l, i);
        let a = info.angle as f64;
        let v = [-libm::sin(a), libm::cos(a)];
        let (lo, hi) = f.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(dot(*p, v)), hi.max(dot(*p, v))));
        r.value("field width across furrows (ft)", hi - lo, format!("{tag} f{i}"));
        r.value("field area (sq ft)", area(f).abs(), format!("{tag} f{i}"));
        r.value("rows per field", rows.len() as f64, format!("{tag} f{i} {:.0} ft wide", hi - lo));
        r.tally(if info.bound == FieldBound::None { "fields unbounded" } else { "fields bounded" }, 1);
        for (pa, pb, gates) in &rows {
            r.tally("rows", 1);
            r.tally("row gates", gates.len());
            r.value("row ft per gate", dist(*pa, *pb) / gates.len().max(1) as f64, format!("{tag} f{i}"));
        }
        let copse = rural::copse(l, i);
        let fb = f.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        let mut worst = 0;
        let (i0, j0) = (((fb[0] - g0[0]) / 5.0) as usize, ((fb[1] - g0[1]) / 5.0) as usize);
        let (i1, j1) = (((fb[2] - g0[0]) / 5.0) as usize, ((fb[3] - g0[1]) / 5.0) as usize);
        for j in j0..=j1.min(ny - 1) {
            for ii in i0..=i1.min(nx - 1) {
                let p = [g0[0] + (ii as f64 + 0.5) * 5.0, g0[1] + (j as f64 + 0.5) * 5.0];
                if !contains(f, p) || copse.is_some_and(|(c, rr)| dist(c, p) < 1.2 * rr + 10.0) {
                    continue;
                }
                let dd = d[j * nx + ii];
                worst = worst.max(dd);
                r.tally("field squares", 1);
                if dd > 12 {
                    r.tally("field squares over 60 ft from a hedge or wall", 1);
                }
            }
        }
        r.value("farthest square from a hedge or wall per field (ft)", 5.0 * worst as f64, format!("{tag} f{i} {:?} {:?} {:.0} ft wide", info.kind, info.bound, hi - lo));
    }
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

/// Yurts (round homes on `town::round_off`'s rings): their doors face the nearest lane (within 70 ft) or
/// the layout's centre, and the interior's door lies on the ring's door side (its first side). Up to 8
/// per world are listed (`TOP=8`): seed, layout, building id, world ft x,y, the door's angle (degrees,
/// 0 east, 90 south: y runs down) and the aim's.
fn yurts(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["yurts", "doors within 30 degrees of their lane or centre", "doors facing south, within 22.5 degrees", "interior doors over 1.5 squares from the ring's door", "doors with their first 10 ft out under another roof", "yurts with no clear side (10 ft out, 10 ft wide, any side)", "yurts listed"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut listed = 0;
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.poly.len() == town::roof::ROUND_SIDES && b.parts.is_empty()) {
            let id = format!("b:{li}:{}", b.id);
            let Some(it) = interior::generate_id(world, t0, &id) else { continue };
            let Some(lv) = it.levels.get(it.entry_level) else { continue };
            let Some(yi) = lv.rooms.iter().position(|q| q.kind == "yurt") else { continue };
            r.tally("yurts", 1);
            let n = b.poly.len() as f64;
            let c = b.poly.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
            let door = [0.5 * (b.poly[0][0] + b.poly[1][0]), 0.5 * (b.poly[0][1] + b.poly[1][1])];
            let ang = libm::atan2(door[1] - c[1], door[0] - c[0]).to_degrees();
            let mut near: Option<(f64, [f64; 2])> = None;
            for w in l.roads.iter().map(|w| &w.0).chain(l.streets.iter()) {
                for e in w.windows(2) {
                    let ab = [e[1][0] - e[0][0], e[1][1] - e[0][1]];
                    let t = (((c[0] - e[0][0]) * ab[0] + (c[1] - e[0][1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
                    let q = [e[0][0] + ab[0] * t, e[0][1] + ab[1] * t];
                    let d = libm::hypot(q[0] - c[0], q[1] - c[1]);
                    if d <= 70.0 && near.is_none_or(|b| d < b.0) {
                        near = Some((d, q));
                    }
                }
            }
            let aim = near.map(|n| n.1).unwrap_or(l.center);
            let aim_ang = libm::atan2(aim[1] - c[1], aim[0] - c[0]).to_degrees();
            let off = |a: f64, b: f64| ((a - b + 540.0) % 360.0 - 180.0).abs();
            if off(ang, aim_ang) <= 30.0 {
                r.tally("doors within 30 degrees of their lane or centre", 1);
            }
            if off(ang, 90.0) <= 22.5 {
                r.tally("doors facing south, within 22.5 degrees", 1);
            }
            let n = b.poly.len();
            let clear = |e: usize| {
                let m = [0.5 * (b.poly[e][0] + b.poly[(e + 1) % n][0]), 0.5 * (b.poly[e][1] + b.poly[(e + 1) % n][1])];
                let cc = town::geom::centroid(&b.poly);
                let d = [m[0] - cc[0], m[1] - cc[1]];
                let k = 1.0 / libm::hypot(d[0], d[1]).max(1e-9);
                [0.25, 0.5, 0.75, 1.0].iter().flat_map(|&t| [-5.0, -2.5, 0.0, 2.5, 5.0].map(|w| (t, w))).all(|(t, w)| {
                    let p = [m[0] + d[0] * k * town::YURT_CLEAR_FT * t - d[1] * k * w, m[1] + d[1] * k * town::YURT_CLEAR_FT * t + d[0] * k * w];
                    !l.buildings.iter().any(|o| o.id != b.id && o.structure == town::Structure::Roofed && libm::hypot(o.poly[0][0] - cc[0], o.poly[0][1] - cc[1]) < 400.0 && town::geom::contains(&o.poly, p))
                })
            };
            if !clear(0) {
                r.tally(if (0..n).any(clear) { "doors with their first 10 ft out under another roof" } else { "yurts with no clear side (10 ft out, 10 ft wide, any side)" }, 1);
            }
            if let Some(d) = lv.doors.iter().find(|d| d.kind == "front" && d.rooms.contains(&(yi as i16))) {
                let g = [0.5 * (d.a[0] + d.b[0]) as f64 * interior::SQUARE_FT, 0.5 * (d.a[1] + d.b[1]) as f64 * interior::SQUARE_FT];
                let w = [it.origin[0] + it.axis[0] * g[0] + it.across[0] * g[1], it.origin[1] + it.axis[1] * g[0] + it.across[1] * g[1]];
                let gap = libm::hypot(w[0] - door[0], w[1] - door[1]);
                if gap > 1.5 * interior::SQUARE_FT {
                    r.add("interior doors over 1.5 squares from the ring's door", gap, format!("{id} seed {}: {gap:.1} ft off, at {:.0},{:.0}", world.file.seed, c[0], c[1]));
                }
            }
            if listed < 8 {
                listed += 1;
                r.add("yurts listed", 0.0, format!("seed {} layout {li} {id} at {:.0},{:.0} door {ang:.0} aim {aim_ang:.0}", world.file.seed, c[0], c[1]));
            }
        }
    }
    yurt_looks(ctx, r);
    yurt_yards(ctx, r);
}

/// R5 towns (115): a steppe town's yurts (homes roofed round on 16-sided rings) by size (ft across) and
/// felt (`roof::Look::shade`), and their clusters (walls within 14 ft of one another): those whose doors
/// all lie within 60 degrees of one another, and those whose yurts all have one lane in reach (70 ft).
fn yurt_looks(ctx: &mut Ctx, r: &mut Report) {
    let bands = ["steppe yurts", "  under 14 ft across", "  14-16 ft", "  16-19 ft", "  19-22 ft", "  22-24 ft", "  over 24 ft"];
    r.counts(&bands);
    r.counts(&["  felt 0 cream", "  felt 1 off-white", "  felt 2 grey", "  felt 3 warm cream", "  felt 4 dusty grey", "  felt 5 smoke-browned", "  felt 6 red band", "  felt 7 sooty crown"]);
    r.counts(&["yurt clusters (2 or more)", "  clusters with one lane in reach of all", "  clusters whose doors share a side (all within 60 degrees)", "  of those with one lane: doors sharing a side"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let felts = ["cream", "off-white", "grey", "warm cream", "dusty grey", "smoke-browned", "red band", "sooty crown"];
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.culture % 5 != 4 {
            continue;
        }
        let ys: Vec<&town::Building> = l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed && b.poly.len() == town::roof::ROUND_SIDES && b.parts.is_empty() && b.func.is_none() && town::roof::look(&l, b).shape == town::roof::Shape::Round).collect();
        let fr: Vec<(P, f64, f64)> = ys
            .iter()
            .map(|b| {
                let c = mean(&b.poly);
                let m = lerp(b.poly[0], b.poly[1], 0.5);
                (c, dist(b.poly[0], c), libm::atan2(m[1] - c[1], m[0] - c[0]).to_degrees())
            })
            .collect();
        for (k, b) in ys.iter().enumerate() {
            r.tally("steppe yurts", 1);
            let d = 2.0 * fr[k].1;
            r.tally(bands[if d < 13.9 { 1 } else if d < 16.0 { 2 } else if d < 19.0 { 3 } else if d < 22.0 { 4 } else if d <= 24.1 { 5 } else { 6 }], 1);
            let s = town::roof::look(&l, b).shade as usize;
            r.tally(&format!("  felt {s} {}", felts.get(s).unwrap_or(&"?")), 1);
        }
        let n = ys.len();
        let mut up: Vec<usize> = (0..n).collect();
        fn root(up: &mut [usize], mut a: usize) -> usize {
            while up[a] != a {
                a = up[a];
            }
            a
        }
        for a in 0..n {
            for b in a + 1..n {
                if dist(fr[a].0, fr[b].0) < fr[a].1 + fr[b].1 + 14.0 {
                    let (ra, rb) = (root(&mut up, a), root(&mut up, b));
                    if ra != rb {
                        up[ra.max(rb)] = ra.min(rb);
                    }
                }
            }
        }
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for a in 0..n {
            let ra = root(&mut up, a);
            groups.entry(ra).or_default().push(a);
        }
        let ways: Vec<&Vec<P>> = l.roads.iter().map(|w| &w.0).chain(l.streets.iter()).collect();
        let off = |a: f64, b: f64| ((a - b + 540.0) % 360.0 - 180.0).abs();
        for g in groups.values().filter(|g| g.len() >= 2) {
            r.tally("yurt clusters (2 or more)", 1);
            let lane = ways.iter().any(|w| g.iter().all(|&k| w.windows(2).any(|e| seg_dist(fr[k].0, e[0], e[1]) <= 70.0)));
            let share = g.iter().all(|&a| g.iter().all(|&b| off(fr[a].2, fr[b].2) <= 60.0));
            if lane {
                r.tally("  clusters with one lane in reach of all", 1);
            }
            if share {
                r.tally("  clusters whose doors share a side (all within 60 degrees)", 1);
            }
            if share && lane {
                r.tally("  of those with one lane: doors sharing a side", 1);
            }
        }
    }
}

/// Whether two outlines touch or overlap (a corner of one in the other, or their edges crossing).
fn outlines_touch(a: &[P], b: &[P]) -> bool {
    let o = |p: P, q: P, s: P| cross(sub(q, p), sub(s, p));
    a.iter().any(|&p| contains(b, p))
        || b.iter().any(|&p| contains(a, p))
        || (0..a.len()).any(|i| {
            let (p, q) = (a[i], a[(i + 1) % a.len()]);
            (0..b.len()).any(|j| {
                let (s, t) = (b[j], b[(j + 1) % b.len()]);
                o(p, q, s) * o(p, q, t) < 0.0 && o(s, t, p) * o(s, t, q) < 0.0
            })
        })
}

/// R5 towns (115): a steppe town's yurts' trodden ground, pens and carts (`town::yurts`): counted, their
/// rules (none over a building, a lane, another's pen or cart, a door's open ground, or off its lot), and
/// on the battlemap of the first 40 yurts with a pen or cart and 40 without per world: the trodden ring
/// drawn as dirt, the props and fences laid, nothing else over a pen or a cart.
fn yurt_yards(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["yurts with trodden ground", "yurts with a pen", "  horses in pens", "yurts with a cart", "  with a haystack too", "yurts drawn up toward their door"]);
    r.counts(&["R pens and carts over a building", "R pens and carts over another's pen, cart or prop", "R pens and carts on a lane", "R pens and carts in a door's open ground", "R pens and carts off their lot"]);
    r.counts(&["battlemap: yurts looked at", "battlemap: trodden rings drawn (dirt by the wall)", "battlemap: yurt props planned", "battlemap: yurt props laid", "battlemap: pen fence ft planned", "battlemap: pen fence ft laid", "R battlemap: objects over a pen or cart"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let size = world.geom.tile_size_ft(world.geom.max_level);
    let mut look: Vec<(String, P, f64, Vec<[P; 4]>, Vec<(P, Kind)>, f64)> = Vec::new();
    let (mut with, mut without, mut listed) = (0, 0, 0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.yurts.is_empty() {
            continue;
        }
        let byid: HashMap<u32, &town::Building> = l.buildings.iter().map(|b| (b.id, b)).collect();
        // (Every yurt's door ground: 10 ft out from its door side, 10 ft wide.)
        let doors: Vec<(u32, Vec<P>)> = l
            .yurts
            .iter()
            .filter_map(|y| byid.get(&y.id).map(|b| (y.id, b)))
            .map(|(id, b)| {
                let c = mean(&b.poly);
                let m = lerp(b.poly[0], b.poly[1], 0.5);
                let n = mul(sub(m, c), 1.0 / dist(m, c).max(1e-9));
                (id, seg_rect(m, add(m, mul(n, town::YURT_CLEAR_FT)), 10.0).to_vec())
            })
            .collect();
        let mut all: Vec<(u32, Vec<P>)> = Vec::new();
        for y in &l.yurts {
            r.tally("yurts with trodden ground", (y.trodden.len() >= 3) as usize);
            let Some(b) = byid.get(&y.id) else { continue };
            let c = mean(&b.poly);
            let rr = dist(b.poly[0], c);
            let mut boxes: Vec<[P; 4]> = Vec::new();
            let mut planned: Vec<(P, Kind)> = Vec::new();
            let mut fence = 0.0;
            if let Some(p) = y.pen {
                r.tally("yurts with a pen", 1);
                if listed < 4 {
                    listed += 1;
                    r.add("  pens listed (fx, fy: the map's)", 0.0, format!("seed {} b:{li}:{} at {} fx {:.6} fy {:.6}", world.file.seed, y.id, at(c), c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft));
                }
                boxes.push(p.pts);
                fence = (0..4).map(|e| dist(p.pts[e], p.pts[(e + 1) % 4])).sum::<f64>() - town::yurts::PEN_GATE_FT;
            }
            let mine: Vec<&town::Prop> = l.props.iter().filter(|p| matches!(p.kind, Kind::Horse | Kind::Cart | Kind::Haystack) && contains(&y.lot, p.at)).collect();
            for p in &mine {
                planned.push((p.at, p.kind));
                match p.kind {
                    Kind::Horse => r.tally("  horses in pens", 1),
                    Kind::Cart => {
                        r.tally("yurts with a cart", 1);
                        boxes.push(horse_foot(p.kind, p.at, p.rot, p.scale).unwrap());
                    }
                    _ => {
                        r.tally("  with a haystack too", 1);
                        boxes.push(horse_foot(p.kind, p.at, p.rot, p.scale).unwrap());
                    }
                }
            }
            let ring = town::roof::round_footprint_sized(&y.lot, &|_| rr);
            if ring.is_some_and(|g| dist(mean(&g), c) > 0.25) {
                r.tally("yurts drawn up toward their door", 1);
            }
            let here = format!("seed {} b:{li}:{} at {}", world.file.seed, y.id, at(c));
            for bx in &boxes {
                if l.buildings.iter().any(|o| o.id != y.id && o.structure == town::Structure::Roofed && dist(o.poly[0], c) < 150.0 && outlines_touch(bx, &o.poly)) {
                    r.add("R pens and carts over a building", 0.0, here.clone());
                }
                if all.iter().any(|(id, o)| *id != y.id && outlines_touch(bx, o)) || l.props.iter().any(|p| !mine.iter().any(|q| std::ptr::eq(*q, p)) && contains(bx, p.at)) {
                    r.add("R pens and carts over another's pen, cart or prop", 0.0, here.clone());
                }
                let lane = l.roads.iter().any(|w| w.0.windows(2).any(|s| bx.iter().any(|&p| seg_dist(p, s[0], s[1]) < 0.5 * w.2) || boxes_cross(bx, &seg_rect(s[0], s[1], w.2))))
                    || l.streets.iter().any(|w| w.windows(2).any(|s| boxes_cross(bx, &seg_rect(s[0], s[1], 10.0))));
                if lane {
                    r.add("R pens and carts on a lane", 0.0, here.clone());
                }
                if doors.iter().any(|(_, d)| boxes_cross(bx, d)) {
                    r.add("R pens and carts in a door's open ground", 0.0, here.clone());
                }
                if !bx.iter().all(|&p| contains(&y.lot, p)) {
                    r.add("R pens and carts off their lot", 0.0, here.clone());
                }
            }
            for bx in &boxes {
                all.push((y.id, bx.to_vec()));
            }
            let extra = !boxes.is_empty();
            if (extra && with < 40) || (!extra && without < 40) {
                if extra {
                    with += 1;
                } else {
                    without += 1;
                }
                look.push((here, c, rr, if y.pen.is_some() { vec![y.pen.unwrap().pts] } else { boxes.clone() }, planned, fence));
            }
        }
    }
    for (here, c, rr, boxes, planned, fence) in look {
        r.tally("battlemap: yurts looked at", 1);
        let reach = rr + 30.0;
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let (mut dirt, mut laid, mut flen, mut over) = (0usize, 0usize, 0.0f64, 0usize);
        for ky in k(c[1] - reach)..=k(c[1] + reach) {
            for kx in k(c[0] - reach)..=k(c[0] + reach) {
                let ch = ctx.chunk(kx, ky);
                let (ox, oy) = (kx as f64 * size, ky as f64 * size);
                for q in 0..SQ * SQ {
                    let p = [ox + ((q % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    let d = dist(p, c);
                    if d > rr && d < rr + town::yurts::TRODDEN_FT + 2.5 && ch.building[q] == 0 && ch.surface[q] == Surface::Dirt {
                        dirt += 1;
                    }
                }
                for o in &ch.objects {
                    let p = [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
                    if planned.iter().any(|q| q.1 == o.kind && dist(q.0, p) < 0.5) {
                        laid += 1;
                    } else if boxes.iter().any(|b| contains(b, p)) {
                        over += 1;
                        r.add("R battlemap: objects over a pen or cart", 0.0, format!("{here}: {} at {}", ch.name(o), at(p)));
                    }
                }
                for s in ch.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Fence && s.pts.len() >= 2) {
                    let f = |q: [f32; 2]| [ox + q[0] as f64 * SQUARE_FT, oy + q[1] as f64 * SQUARE_FT];
                    let (a, e) = (f(s.pts[0]), f(s.pts[1]));
                    if boxes.iter().any(|b| (0..4).any(|j| seg_dist(lerp(a, e, 0.5), b[j], b[(j + 1) % 4]) < 1.0)) {
                        flen += dist(a, e);
                    }
                }
            }
        }
        let _ = over;
        r.tally("battlemap: trodden rings drawn (dirt by the wall)", (dirt > 0) as usize);
        r.tally("battlemap: yurt props planned", planned.len());
        r.tally("battlemap: yurt props laid", laid);
        r.tally("battlemap: pen fence ft planned", fence as usize);
        r.tally("battlemap: pen fence ft laid", flen as usize);
        if dirt == 0 || laid < planned.len() || flen + 1.0 < fence {
            r.add("battlemap: yurts short of dirt, props or fence", 0.0, format!("{here}: dirt squares {dirt}, props {laid}/{}, fence {flen:.0}/{fence:.0} ft", planned.len()));
        }
    }
}

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
        let mut text = format!("{l:?}").replace(", grounds: [] }", " }").replace(", keep: false }", " }").replace(", keep: true }", " }").replace(", keep: None }", " }").replace(", forecourts: [] }", " }").replace(", wheels: [] }", " }").replace(", crofts: [] }", " }");
        for k in ["Great", "Hall", "L", "T", "Shell", "Plain"] {
            text = text.replace(&format!(", keep: Some({k}) }}"), " }");
        }
        // (Nor the fields' info, v63: their furrows, use and bounds.)
        if let Some(a) = text.find(", field_info: [") {
            if let Some(b) = text[a..].find(']') {
                text.replace_range(a..a + b + 1, "");
            }
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

// ---- Walls by culture group (item 104). ----

/// `walls`: per culture group (`l.culture % 5`), walled places and their towers by shape (wall towers,
/// castle towers, corner towers); square towers where the style has none; towers whose battlemap squares
/// don't follow their shape (the ring of wall blocking, the floor open, the door reached from outside);
/// and up to 3 walled settlements per group listed with a camera on the whole wall and one on a tower
/// by a gate (`fx, fy` as `scripts/shot.mjs` views take them). Towers of the first listed place per
/// group are checked on the battlemap (`WALLS_ALL=1`: every listed place's).
/// The bow of each long curtain (slice 2c): per side of the town's ring 150 ft and longer, the town
/// wall along it (5-ft steps within 80 ft of it, beside the side), its greatest distance off the chord
/// between its points nearest the side's two corners. Also the buildings centred in the town's patches
/// that the wall leaves outside (a line to the town's middle crosses it an odd number of times), and
/// how many it has inside.
fn curtain_bows(ctx: &Ctx, l: &Layout, li: usize) -> (Vec<(f64, String)>, Vec<String>, usize) {
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut out = (Vec::new(), Vec::new(), 0);
    let Ok(plan) = town::wards::plan_json(world, t0, li) else { return out };
    if plan["walls"]["built"] != true {
        return out;
    }
    let pt = |v: &serde_json::Value| -> P { [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)] };
    let corners: HashMap<u64, P> = plan["corners"].as_array().into_iter().flatten().filter_map(|c| Some((c["corner"].as_u64()?, pt(&c["at"])))).collect();
    let mut sides: Vec<(u64, u64)> = Vec::new();
    let mut patches: Vec<Vec<P>> = Vec::new();
    for p in plan["patches"].as_array().into_iter().flatten().filter(|p| p["in_town"] == true) {
        let cs: Vec<u64> = p["corners"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()).collect();
        patches.push(cs.iter().filter_map(|c| corners.get(c).copied()).collect());
        sides.extend((0..cs.len()).map(|k| (cs[k], cs[(k + 1) % cs.len()])));
    }
    let shared: std::collections::HashSet<(u64, u64)> = sides.iter().copied().collect();
    let town: Vec<&Vec<P>> = l.walls.iter().filter(|w| w.len() >= 2 && !l.castles.iter().any(|(c, rad)| dist(*c, w[w.len() / 2]) <= rad + 10.0)).collect();
    let pts: Vec<P> = town.iter().flat_map(|w| samples(w, 5.0).into_iter().map(|s| s.0).chain(std::iter::once(w[w.len() - 1]))).collect();
    // (Each wall point counts for the ring side nearest it, slice 2e: bows reach 80 ft out.)
    let ring: Vec<(P, P)> = sides.iter().filter(|(a, b)| !shared.contains(&(*b, *a))).filter_map(|(a, b)| Some((*corners.get(a)?, *corners.get(b)?))).collect();
    let side_of = |q: P| (0..ring.len()).min_by(|&x, &y| seg_dist(q, ring[x].0, ring[x].1).total_cmp(&seg_dist(q, ring[y].0, ring[y].1)));
    let owner: Vec<Option<usize>> = pts.iter().map(|q| side_of(*q)).collect();
    for (k, &(pa, pb)) in ring.iter().enumerate() {
        let d = dist(pa, pb);
        if d < 150.0 {
            continue;
        }
        let u = mul(sub(pb, pa), 1.0 / d);
        let near: Vec<P> = pts.iter().zip(&owner).filter(|(q, o)| **o == Some(k) && seg_dist(**q, pa, pb) < 80.0 && (0.0..=d).contains(&dot(sub(**q, pa), u))).map(|x| *x.0).collect();
        let nearest = |c: P| near.iter().copied().min_by(|x, y| dist(*x, c).total_cmp(&dist(*y, c)));
        let (Some(q0), Some(q1)) = (nearest(pa), nearest(pb)) else { continue };
        if dist(q0, q1) < 100.0 {
            continue;
        }
        let bow = near.iter().map(|q| seg_dist(*q, q0, q1)).fold(0.0, f64::max);
        let m = lerp(pa, pb, 0.5);
        out.0.push((bow, format!("L{li} side at {} fx {:.6} fy {:.6}", at(m), m[0] / world.geom.map_w_ft, m[1] / world.geom.map_h_ft)));
    }
    let segs: Vec<(P, P)> = town.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let crosses = |p: P, q: P, a: P, b: P| {
        let (o1, o2) = (cross(sub(q, p), sub(a, p)), cross(sub(q, p), sub(b, p)));
        let (o3, o4) = (cross(sub(b, a), sub(p, a)), cross(sub(b, a), sub(q, a)));
        o1 * o2 < 0.0 && o3 * o4 < 0.0
    };
    for bd in l.buildings.iter().filter(|bd| bd.poly.len() >= 3 && patches.iter().any(|f| contains(f, centroid(&bd.poly)))) {
        let c = centroid(&bd.poly);
        if segs.iter().filter(|&&(a, b)| crosses(c, l.center, a, b)).count() % 2 == 1 {
            out.1.push(format!("L{li} b:{li}:{} at {}", bd.id, at(c)));
        } else {
            out.2 += 1;
        }
    }
    out
}

/// Town wall towers (a castle's apart) and gate towers under 22 ft apart, centre to centre: two
/// wall runs' corners meeting, each with its own tower (a gate's own pair apart).
fn close_towers(l: &Layout, li: usize, r: &mut Report) {
    let castle = |t: P| l.tower_forms.iter().any(|f| (f.0[0] - t[0]).abs() + (f.0[1] - t[1]).abs() < 1e-6 && f.1 & 16 != 0);
    let ts: Vec<(P, bool)> = l.towers.iter().filter(|t| !castle(**t)).map(|t| (*t, false)).chain(l.gate_towers.iter().map(|t| (*t, true))).collect();
    for i in 0..ts.len() {
        for j in i + 1..ts.len() {
            let d = dist(ts[i].0, ts[j].0);
            // (A gate's pair: across its passage, both gate towers.)
            if d < 22.0 && !(ts[i].1 && ts[j].1 && d > 8.0) {
                let kind = match (ts[i].1, ts[j].1) {
                    (false, false) => "wall towers",
                    (true, true) => "gate towers",
                    _ => "a gate tower and a wall tower",
                };
                r.add("town towers under 22 ft apart", 1.0, format!("L{li} {kind} {d:.1} ft apart at {} and {}", at(ts[i].0), at(ts[j].0)));
            }
        }
    }
}

fn walls(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::battlemap::URBAN_TOWER_FLOOR;
    use worldgen::town::wallstyle::{self, TowerShape};
    const GROUPS: [&str; 5] = ["North", "West", "South", "Dry", "Steppe"];
    const SHAPES: [&str; 4] = ["round", "square", "D", "octagon"];
    r.counts(&[
        "square towers where the style has none",
        "towers not of their style's shapes",
        "towers checked on the battlemap",
        "towers whose battlemap squares don't follow their shape",
        "towers by the water with their door in it (not counted above)",
        "buildings under or within 4 ft of a town wall",
        "town walls crossing a road or street off a gate",
        "town walls crossing themselves",
        "town walls on the water (5-ft steps)",
        "gate passages blocked on the battlemap",
        "bastions over a road, a building or the water",
        "gate passages not walkable end to end on the battlemap",
        "gatehouses over a road, a building or the water",
        "castle gates not of their group's form",
        "town towers under 22 ft apart",
        "buildings in the town's patches the wall leaves outside",
        "places listed",
    ]);
    const OUTLINES: [&str; 3] = ["straight", "star", "round"];
    const MATERIALS: [&str; 3] = ["stone", "palisade", "rammed earth"];
    let lattice = ctx.ex.world.geom.spacing_ft(ctx.ex.world.geom.first_refine_level - 1);
    let all = std::env::var("WALLS_ALL").is_ok_and(|v| v == "1");
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let ns = t0.settlements.len();
    let mut listed = [0usize; 5];
    let mut to_check: Vec<(usize, Rc<Layout>)> = Vec::new();
    let mut to_walk: Vec<(usize, Rc<Layout>)> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut order: Vec<usize> = (0..town::layout_count(t0)).collect();
    // (The biggest places first: a group's cities before its towns.)
    order.sort_by_key(|&li| (std::cmp::Reverse(t0.settlements.get(li).map_or(0, |s| s.population)), li));
    for li in order {
        let l = town::layout(world, t0, li);
        let g = (l.wall_style.group % 5) as usize;
        if li < ns && t0.settlements[li].tier >= worldgen::t0::settle::Tier::Town {
            r.tally(&format!("{}: towns and up", GROUPS[g]), 1);
        }
        if l.walls.is_empty() {
            continue;
        }
        r.tally(&format!("{}: walled places", GROUPS[g]), 1);
        for t in l.towers.iter().copied().chain(l.extra_towers.iter().filter(|e| !e.1).map(|e| e.0)) {
            let (shape, _) = l.tower_form(t);
            let castle = l.tower_forms.iter().any(|f| (f.0[0] - t[0]).abs() + (f.0[1] - t[1]).abs() < 1e-6 && f.1 & 16 != 0);
            let style = l.wall_style_at(t);
            r.tally(&format!("{}: {} towers {}", GROUPS[g], if castle { "castle" } else { "wall" }, SHAPES[shape as usize]), 1);
            if wallstyle::is_corner(&l.walls, t) {
                r.tally(&format!("{}: corner towers {}", GROUPS[g], SHAPES[shape as usize]), 1);
            }
            if shape == TowerShape::Square && !style.has_square() {
                r.add("square towers where the style has none", 1.0, format!("L{li} {}", at(t)));
            }
            if shape != style.tower && shape != style.corner {
                r.add("towers not of their style's shapes", 1.0, format!("L{li} {} {}", SHAPES[shape as usize], at(t)));
            }
        }
        close_towers(&l, li, r);
        wall_rules(&l, li, lattice, t0, r);
        gate_rules(&l, li, lattice, t0, r);
        // How round the curtains run (castles' apart): the turning at each corner; runs of gentle steps
        // (12 degrees or less, one way) turning 15 degrees or more are rounded corners, each with its radius
        // (its steps' shorter sides over their turning, weighted by turning).
        for w in l.walls.iter().filter(|w| w.len() >= 3 && !l.castles.iter().any(|(c, rad)| dist(*c, w[w.len() / 2]) <= rad + 10.0)) {
            let mut run = (0.0f64, 0.0f64, 0.0f64);
            let flush = |run: &mut (f64, f64, f64), r: &mut Report| {
                if run.1.abs() >= 15.0 {
                    r.tally(&format!("{}: rounded corners", GROUPS[g]), 1);
                    r.tally(&format!("{}: rounded corners' radius ft (sum)", GROUPS[g]), (run.2 / run.1.abs()).round() as usize);
                    r.tally(&format!("{}: curtain turning deg in rounded corners", GROUPS[g]), run.1.abs().round() as usize);
                }
                *run = (0.0, 0.0, 0.0);
            };
            for k in 1..w.len() - 1 {
                let (u, v) = (sub(w[k], w[k - 1]), sub(w[k + 1], w[k]));
                let turn = libm::atan2(cross(u, v), dot(u, v)).to_degrees();
                r.tally(&format!("{}: curtain turning deg", GROUPS[g]), turn.abs().round() as usize);
                if turn.abs() > 12.0 || turn.abs() < 0.5 || run.0 * turn < 0.0 {
                    flush(&mut run, r);
                }
                if turn.abs() <= 12.0 && turn.abs() >= 0.5 {
                    let rad = len(u).min(len(v)) / turn.abs().to_radians();
                    run = (turn.signum(), run.1 + turn, run.2 + rad * turn.abs());
                }
            }
            flush(&mut run, r);
        }
        // How far the long curtains bow (slice 2c), and the town's buildings the wall leaves outside.
        let (bows, outside, inside) = curtain_bows(ctx, &l, li);
        for b in bows {
            r.value(&format!("{}: curtain bow ft (runs 150 ft and longer)", GROUPS[g]), b.0, b.1);
        }
        r.tally("buildings in the town's patches inside its wall", inside);
        for w in outside {
            r.add("buildings in the town's patches the wall leaves outside", 1.0, w);
        }
        let st = l.wall_style;
        r.tally(&format!("{}: outline {}", GROUPS[g], OUTLINES[st.outline as usize]), 1);
        r.tally(&format!("{}: material {}", GROUPS[g], MATERIALS[st.material as usize]), 1);
        r.tally(&format!("{}: bastions placed", GROUPS[g]), l.bastions.len());
        r.tally(&format!("{}: bastions dropped", GROUPS[g]), l.bastions_dropped as usize);
        // Listed: settlements, up to 3 a group.
        if li >= ns || listed[g] >= 3 {
            continue;
        }
        listed[g] += 1;
        r.tally("places listed", 1);
        let s = &t0.settlements[li];
        let bb = bbox(&l.walls.iter().flatten().copied().collect::<Vec<_>>());
        let (cx, cy) = (0.5 * (bb[0] + bb[2]), 0.5 * (bb[1] + bb[3]));
        let span = (bb[2] - bb[0]).max(bb[3] - bb[1]).max(100.0);
        let whole = libm::log2(1100.0 / span);
        // (A gate tower of the town's, not a castle's: the first in order.)
        let town_gate = l.gate_towers.iter().find(|p| !l.castles.iter().any(|(c, rad)| dist(*c, **p) <= rad + 1.0));
        let gate = town_gate.or(l.gate_towers.first()).copied().unwrap_or(l.towers.first().copied().unwrap_or([cx, cy]));
        let st = l.wall_style;
        lines.push(format!(
            "    {} L{li} {} wealth {:.2}{}{}: wall fx {:.6} fy {:.6} zoom {whole:.1}; gate tower fx {:.6} fy {:.6} zoom 1.5 ({} towers, {} castle)",
            GROUPS[g],
            s.tier.name(),
            wallstyle::wealth(s),
            if st.rich { " rich" } else { "" },
            if st.poor { " poor" } else { "" },
            cx / mw,
            cy / mh,
            gate[0] / mw,
            gate[1] / mh,
            l.towers.len() + l.gate_towers.len(),
            if l.castles.is_empty() { "no" } else { "a" },
        ));
        if let Some(q) = l.bastions.first() {
            lines.push(format!("      bastion fx {:.6} fy {:.6} zoom 0.5", q[5][0] / mw, q[5][1] / mh));
        }
        if all || listed[g] == 1 {
            to_check.push((li, l.clone()));
        }
        to_walk.push((li, l.clone()));
    }
    // Every gate of the places listed walked through on the battlemap.
    for (li, l) in to_walk {
        gate_walks(ctx, &l, li, r);
    }
    // The battlemap round each tower of the places checked.
    for (li, l) in to_check {
        // (Each town gate's way through: open squares from 15 ft out to 15 ft in.)
        let houses = town::gatehouse::houses(&l);
        for g in l.gates.iter().copied() {
            let pair: Vec<P> = l.gate_towers.iter().copied().filter(|t| dist(*t, g) < 32.0).collect();
            // (A bent entrance's way turns: walked through below.)
            if pair.len() < 2 || town::gatehouse::of_tower(&houses, pair[0]).is_some_and(|h| h.0.form == wallstyle::Gatehouse::Bent) {
                continue;
            }
            let u = sub(pair[1], pair[0]);
            let n = mul([-u[1], u[0]], 1.0 / len(u).max(1e-9));
            let w = ctx.window([g[0] - 30.0, g[1] - 30.0, g[0] + 30.0, g[1] + 30.0]);
            let shut = (-3..=3).map(|k| add(g, mul(n, 5.0 * k as f64))).filter(|p| {
                let (i, j) = (((p[0] - w.ox) / SQUARE_FT).floor() as i64, ((p[1] - w.oy) / SQUARE_FT).floor() as i64);
                i >= 0 && j >= 0 && (i as usize) < w.nx && (j as usize) < w.ny && w.block[j as usize * w.nx + i as usize]
            }).count();
            if shut > 0 {
                r.add("gate passages blocked on the battlemap", shut as f64, format!("L{li} gate {} ({shut} squares)", at(g)));
            }
        }
        for (k, &(t, rad, gate)) in interior::towers(&l).iter().enumerate() {
            let (shape, ras) = wallstyle::tower_raster(&l, t, rad, gate, SQUARE_FT);
            let x0 = ras.i0 as f64 * SQUARE_FT - 40.0;
            let y0 = ras.j0 as f64 * SQUARE_FT - 40.0;
            let w = ctx.window([x0, y0, x0 + ras.nx as f64 * SQUARE_FT + 80.0, y0 + ras.ny as f64 * SQUARE_FT + 80.0]);
            r.tally("towers checked on the battlemap", 1);
            let wi = |gi: i64, gj: i64| -> Option<usize> {
                let (i, j) = (gi - (w.ox / SQUARE_FT).round() as i64, gj - (w.oy / SQUARE_FT).round() as i64);
                (i >= 0 && j >= 0 && (i as usize) < w.nx && (j as usize) < w.ny).then(|| j as usize * w.nx + i as usize)
            };
            let mut bad: Vec<String> = Vec::new();
            let mut door: Option<usize> = None;
            let mut floors = 0;
            let mut wet = false;
            for q in 0..ras.nx * ras.ny {
                let v = ras.cells[q];
                if v == 0 {
                    continue;
                }
                let (gi, gj) = (ras.i0 + (q % ras.nx) as i64, ras.j0 + (q / ras.nx) as i64);
                let Some(a) = wi(gi, gj) else { continue };
                let (kx, ky) = (gi.div_euclid(SQ as i64), gj.div_euclid(SQ as i64));
                let c = ctx.chunk(kx as u32, ky as u32);
                let k = (gj - ky * SQ as i64) as usize * SQ + (gi - kx * SQ as i64) as usize;
                let (b, u) = (c.building[k], c.urban[k]);
                // (A building on it, the water, a deck: theirs.)
                // (A way underground at its foot: its.)
                if b != 0 || c.water_level[k] > c.height[k] || matches!(u, worldgen::battlemap::URBAN_DECK | worldgen::battlemap::URBAN_ENTRANCE) {
                    if v == wallstyle::DOOR && b != 0 {
                        bad.push("door under a building".into());
                    }
                    wet |= v == wallstyle::DOOR && c.water_level[k] > c.height[k];
                    continue;
                }
                match v {
                    wallstyle::RING if u != URBAN_TOWER => bad.push(format!("ring square open ({u})")),
                    wallstyle::FLOOR | wallstyle::DOOR if u != URBAN_TOWER_FLOOR && u != URBAN_TOWER => bad.push(format!("floor square {u}")),
                    _ => {}
                }
                if v != wallstyle::RING && u == URBAN_TOWER_FLOOR {
                    floors += 1;
                }
                if v == wallstyle::DOOR {
                    door = Some(a);
                }
            }
            if floors == 0 {
                bad.push("no open floor".into());
            }
            // The door reached from the ground 30 ft out (square to square past what blocks).
            match door {
                None if !ras.cells.contains(&wallstyle::DOOR) => bad.push("no door".into()),
                None => {}
                Some(d) => {
                    let mut seen = vec![false; w.nx * w.ny];
                    let mut q = VecDeque::from([d]);
                    seen[d] = true;
                    let mut out = false;
                    while let Some(a) = q.pop_front() {
                        let p = [w.ox + ((a % w.nx) as f64 + 0.5) * SQUARE_FT, w.oy + ((a / w.nx) as f64 + 0.5) * SQUARE_FT];
                        if dist(p, t) > rad + 30.0 {
                            out = true;
                            break;
                        }
                        let (x, y) = ((a % w.nx) as i64, (a / w.nx) as i64);
                        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            let (nx, ny) = (x + dx, y + dy);
                            if nx < 0 || ny < 0 || nx >= w.nx as i64 || ny >= w.ny as i64 {
                                continue;
                            }
                            let n = ny as usize * w.nx + nx as usize;
                            if !seen[n] && !w.block[n] {
                                seen[n] = true;
                                q.push_back(n);
                            }
                        }
                    }
                    if !out {
                        bad.push("door not reached from outside".into());
                    }
                }
            }
            if wet {
                r.add("towers by the water with their door in it (not counted above)", 1.0, format!("t:{li}:{k} {} {}", SHAPES[shape as usize], at(t)));
                continue;
            }
            if !bad.is_empty() {
                bad.dedup();
                r.add("towers whose battlemap squares don't follow their shape", bad.len() as f64, format!("t:{li}:{k} {} {}: {}", SHAPES[shape as usize], at(t), bad.join(", ")));
            }
        }
    }
    for line in lines {
        say!("{line}");
    }
}

/// The town wall's rules (item 104) on one layout: buildings off it, roads and streets through it only
/// at a gate, never crossing itself or standing on the water, its bastions clear.
fn wall_rules(l: &Layout, li: usize, lattice: f64, t0: &T0, r: &mut Report) {
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    let half = 0.5 * l.wall_style.thickness();
    let segs: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let segx = |p: P, q: P, a: P, b: P| {
        let c = |o: P, x: P, y: P| cross(sub(x, o), sub(y, o));
        c(p, q, a) * c(p, q, b) < 0.0 && c(a, b, p) * c(a, b, q) < 0.0
    };
    let poly_seg = |poly: &[P], a: P, b: P| {
        let n = poly.len();
        let d = (0..n).map(|k| seg_dist(poly[k], a, b).min(seg_dist(a, poly[k], poly[(k + 1) % n])).min(seg_dist(b, poly[k], poly[(k + 1) % n]))).fold(f64::MAX, f64::min);
        if contains(poly, a) || (0..n).any(|k| segx(poly[k], poly[(k + 1) % n], a, b)) { 0.0 } else { d }
    };
    let bb = bbox(&l.walls.iter().flatten().copied().collect::<Vec<_>>());
    for b in l.buildings.iter().filter(|b| b.poly.iter().any(|p| p[0] > bb[0] - 30.0 && p[0] < bb[2] + 30.0 && p[1] > bb[1] - 30.0 && p[1] < bb[3] + 30.0)) {
        // (Castles' own curtains have their rule.)
        if l.castles.iter().any(|(c, rad)| dist(*c, centroid(&b.poly)) <= rad + 20.0) {
            continue;
        }
        let d = segs.iter().map(|&(a, q)| poly_seg(&b.poly, a, q)).fold(f64::MAX, f64::min) - half;
        // (The sprawl outside stands 7 ft off the wall's line, as its patches are laid: counted apart.)
        if d < 4.0 && d > -half && matches!(b.ward, Ward::Slum | Ward::Farm) {
            r.add("sprawl outside within 4 ft of a town wall's face (not under it)", 4.0 - d, format!("L{li} b:{li}:{} {:.1} ft", b.id, d));
        } else if d < 4.0 {
            r.add("buildings under or within 4 ft of a town wall", 4.0 - d, format!("L{li} b:{li}:{} {:?} {:?} {:.1} ft at {}", b.id, b.ward, b.structure, d, at(centroid(&b.poly))));
        }
    }
    let gate_near = |p: P| l.gates.iter().chain(&l.gate_towers).any(|g| dist(*g, p) < 32.0) || l.castles.iter().any(|(c, rad)| dist(*c, p) <= rad + 30.0);
    for (pts, what) in l.streets.iter().map(|s| (s, 9)).chain(l.roads.iter().filter(|r| r.1 != 3).map(|r| (&r.0, r.1))) {
        for q in pts.windows(2) {
            for &(a, b) in &segs {
                if segx(q[0], q[1], a, b) {
                    let m = lerp(a, b, 0.5);
                    if !gate_near(q[0]) && !gate_near(q[1]) && !gate_near(m) {
                        r.add("town walls crossing a road or street off a gate", 1.0, format!("L{li} {} at {}", if what == 9 { "street".to_string() } else { format!("road {what}") }, at(q[0])));
                    }
                }
            }
        }
    }
    for (i, &(a, b)) in segs.iter().enumerate() {
        for &(c, d) in &segs[i + 1..] {
            if segx(a, b, c, d) && !l.castles.iter().any(|(o, rad)| dist(*o, a) <= rad + 40.0) {
                r.add("town walls crossing themselves", 1.0, format!("L{li} at {} and {}", at(a), at(c)));
            }
        }
        let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
        let k = (0..n).filter(|&j| wet(lerp(a, b, (j as f64 + 0.5) / n as f64))).count();
        if k > 0 && !l.castles.iter().any(|(c, rad)| dist(*c, a) <= rad + 10.0) {
            r.add("town walls on the water (5-ft steps)", k as f64, format!("L{li} at {}", at(a)));
        }
    }
    for q in &l.bastions {
        let on_b = l.buildings.iter().find(|b| b.poly.iter().any(|p| contains(q, *p)) || q.iter().any(|p| contains(&b.poly, *p)));
        // (A road's line more than 3 ft into it, or its paving more than 6 ft: the town's paved strips
        // run along the wall's line, and those ending at the corner spread their paving into the gorge.)
        let paved = |pts: &[P], w: f64| {
            pts.windows(2).any(|s| {
                let (u, n) = (sub(s[1], s[0]), (dist(s[0], s[1]) / 2.5).ceil().max(1.0) as usize);
                let nrm = mul([-u[1], u[0]], 1.0 / len(u).max(1e-9));
                (0..=n).any(|j| [-0.5 * w + 1.0, 0.0, 0.5 * w - 1.0].iter().any(|o| {
                    let p = add(lerp(s[0], s[1], j as f64 / n as f64), mul(nrm, *o));
                    contains(q, p) && depth(q, p) > if *o == 0.0 { 3.0 } else { 6.0 }
                }))
            })
        };
        let on_r = l.streets.iter().map(|s| (s.as_slice(), 9, 4.0)).chain(l.roads.iter().map(|r| (r.0.as_slice(), r.1, r.2))).find(|s| paved(s.0, s.2));
        let on_w = wet(centroid(q)) || q.iter().any(|p| wet(*p));
        if on_b.is_some() || on_r.is_some() || on_w {
            let what = on_b.map(|b| format!("b:{li}:{} {:?}", b.id, b.ward)).or(on_r.map(|s| format!("road {}", s.1))).unwrap_or("water".into());
            r.add("bastions over a road, a building or the water", 1.0, format!("L{li} at {}: {what}", at(q[2])));
        }
    }
}

const FORMS: [&str; 5] = ["square tower", "twin drums", "arched", "bent", "log"];

/// The gatehouses' rules (item 104) on one layout: their forms counted, none over a road (but its own
/// way through), a building or the water, a castle's gate of its group's form.
fn gate_rules(l: &Layout, li: usize, lattice: f64, t0: &T0, r: &mut Report) {
    use worldgen::town::gatehouse;
    const GROUPS: [&str; 5] = ["North", "West", "South", "Dry", "Steppe"];
    let g = GROUPS[(l.wall_style.group % 5) as usize];
    let wet = |p: P| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice);
    let segx = |p: P, q: P, a: P, b: P| {
        let c = |o: P, x: P, y: P| cross(sub(x, o), sub(y, o));
        c(p, q, a) * c(p, q, b) < 0.0 && c(a, b, p) * c(a, b, q) < 0.0
    };
    let cuts = |poly: &[P], a: P, b: P| contains(poly, a) || contains(poly, b) || (0..poly.len()).any(|k| segx(poly[k], poly[(k + 1) % poly.len()], a, b));
    for h in gatehouse::houses(l) {
        r.tally(&format!("{g}: {}gatehouses {}", if h.castle { "castle " } else { "" }, FORMS[h.form as usize]), 1);
        if h.castle && h.form != l.wall_style.castle().gatehouse {
            r.add("castle gates not of their group's form", 1.0, format!("L{li} {} at {}", FORMS[h.form as usize], at(h.a)));
        }
        let solids = h.solids();
        let way = h.passage();
        let near = |p: &P| dist(*p, h.a) < 140.0;
        let on_b = l.buildings.iter().filter(|b| b.poly.iter().any(near)).find(|b| solids.iter().any(|q| b.poly.iter().any(|p| contains(q, *p)) || q.iter().any(|p| contains(&b.poly, *p))));
        // (A road's line more than 3 ft into a tower or the block: not the gate's own way, through it.)
        let into = |p: P| solids.iter().any(|z| contains(z, p) && (0..z.len()).all(|k| seg_dist(p, z[k], z[(k + 1) % z.len()]) > 3.0));
        let on_r = l.streets.iter().map(|s| (s, 9)).chain(l.roads.iter().filter(|r| r.1 != 3).map(|r| (&r.0, r.1))).find(|(pts, _)| {
            !pts.windows(2).any(|q| way.iter().any(|w| cuts(w, q[0], q[1]))) && pts.windows(2).any(|q| {
                let n = (dist(q[0], q[1]) / 2.5).ceil().max(1.0) as usize;
                (near(&q[0]) || near(&q[1])) && solids.iter().any(|z| cuts(z, q[0], q[1])) && (0..=n).any(|j| into(lerp(q[0], q[1], j as f64 / n as f64)))
            })
        });
        let on_w = solids.iter().any(|q| wet(centroid(q)) || q.iter().any(|p| wet(*p)));
        if on_b.is_some() || on_r.is_some() || on_w {
            let hit = on_r.and_then(|(pts, _)| pts.windows(2).flat_map(|q| (0..=8).map(move |j| lerp(q[0], q[1], j as f64 / 8.0))).find(|p| into(*p)));
            let fr = hit.map(|p| { let u = mul(sub(h.b, h.a), 1.0 / h.len()); let d = sub(p, h.a); format!(" s {:.1} c {:.1} len {:.1}", dot(d, u), dot(d, h.n), h.len()) }).unwrap_or_default();
            let what = on_b.map(|b| format!("b:{li}:{} {:?}", b.id, b.ward)).or(on_r.map(|s| format!("road {}{fr}", s.1))).unwrap_or("water".into());
            r.add("gatehouses over a road, a building or the water", 1.0, format!("L{li} {} at {}: {what}", FORMS[h.form as usize], at(h.a)));
        }
    }
}

/// Each gate of `l` walked through on the battlemap: square to square (not corner to corner) from 8 ft
/// out past its outer mouth to 8 ft past its inner one, within 40 ft of it.
fn gate_walks(ctx: &mut Ctx, l: &Layout, li: usize, r: &mut Report) {
    for h in town::gatehouse::houses(l) {
        let (o, i) = h.ends();
        let m = lerp(h.a, h.b, 0.5);
        let w = ctx.window([m[0] - 70.0, m[1] - 70.0, m[0] + 70.0, m[1] + 70.0]);
        let ix = |p: P| {
            let (i, j) = (((p[0] - w.ox) / SQUARE_FT).floor() as i64, ((p[1] - w.oy) / SQUARE_FT).floor() as i64);
            (i >= 0 && j >= 0 && (i as usize) < w.nx && (j as usize) < w.ny).then(|| j as usize * w.nx + i as usize)
        };
        // (From any open square within 5 ft of each end: the ground past a mouth may be built on.)
        let open_near = |p: P| (0..w.nx * w.ny).filter(|&k| !w.block[k]).find(|&k| dist(p, [w.ox + ((k % w.nx) as f64 + 0.5) * SQUARE_FT, w.oy + ((k / w.nx) as f64 + 0.5) * SQUARE_FT]) <= 5.0);
        let (Some(_), Some(_)) = (ix(o), ix(i)) else { continue };
        let (a, b) = (open_near(o), open_near(i));
        let why = if a.is_none() || b.is_none() {
            Some(if a.is_none() { "its way in shut" } else { "its way out shut" })
        } else {
            let (a, b) = (a.unwrap(), b.unwrap());
            let mut seen = vec![false; w.nx * w.ny];
            let mut q = VecDeque::from([a]);
            seen[a] = true;
            while let Some(k) = q.pop_front() {
                let (x, y) = ((k % w.nx) as i64, (k / w.nx) as i64);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w.nx as i64 || ny >= w.ny as i64 {
                        continue;
                    }
                    let n = ny as usize * w.nx + nx as usize;
                    let p = [w.ox + (nx as f64 + 0.5) * SQUARE_FT, w.oy + (ny as f64 + 0.5) * SQUARE_FT];
                    if !seen[n] && !w.block[n] && dist(p, m) <= h.len() + 40.0 {
                        seen[n] = true;
                        q.push_back(n);
                    }
                }
            }
            (!seen[b]).then_some("no way through")
        };
        r.tally("gates walked through on the battlemap", 1);
        if let Some(why) = why {
            r.add("gate passages not walkable end to end on the battlemap", 1.0, format!("L{li} {} at {}: {why}", FORMS[h.form as usize], at(m)));
        }
    }
}

fn main() {
    const MODES: [&str; 25] = ["horseparks", "steppe", "castlepieces", "wallgates", "handwalls", "districts", "places", "walls", "yurts", "villages", "castles", "reach", "interiors", "hub", "halls", "palaces", "temples", "civic", "inns", "editor", "under", "hash", "piers", "cemeteries", "all"];
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
    let worlds = if worlds.is_empty() && (modes == ["yurts"] || modes == ["steppe"] || modes == ["horseparks"]) {
        (1..=8).map(|s| s.to_string()).collect()
    } else if worlds.is_empty() {
        vec!["1".into(), "2".into(), "3".into()]
    } else {
        worlds
    };
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
                "yurts" => yurts(&mut ctx, &mut r),
                "villages" => villages(&mut ctx, &mut r),
                "walls" => walls(&mut ctx, &mut r),
                "steppe" => steppe(&mut ctx, &mut r),
                "horseparks" => horseparks(&mut ctx, &mut r),
                "cemeteries" => cemeteries(&mut ctx, &mut r),
                "handwalls" => handwalls(&mut ctx, &mut r),
                "wallgates" => wallgates(&mut ctx, &mut r),
                "castlepieces" => castle_pieces(&mut ctx, &mut r),
                "editor" => editor(&mut ctx, &mut r),
                "districts" => district_edits(&ctx.ex.world, &ctx.ex.t0, &mut r),
                "places" => places(&ctx.ex.world, &ctx.ex.t0, &mut r),
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

// R5 towns
/// Cemeteries (M21): per tier and culture group, how many and what they hold; the rules (each
/// should be 0): props or tombs on a walk, over the boundary or outside the plot, tomb doors not
/// reached from the way in, kurgans overlapping, a North or West lychgate off the way in, a tomb
/// planned and not built. Then the editor: a park patch set to a cemetery by hand, walls on and off.
fn cemeteries(ctx: &mut Ctx, r: &mut Report) {
    const GROUPS: [&str; 5] = ["North", "West", "South", "Dry", "Steppe"];
    r.counts(&CEM_RULES);
    r.counts(&["hand cemeteries refused", "hand cemeteries not laid"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let mut cams: Vec<String> = Vec::new();
    let mut shown = [0usize; 5];
    // (Up to 2 a group looked at on the battlemap: plot, props planned.)
    let mut look: Vec<(usize, Vec<P>, usize)> = Vec::new();
    let mut ground: Vec<(usize, town::Park)> = Vec::new();
    r.counts(&CEM_GROUND);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town {
            continue;
        }
        let g = (l.culture % 5) as usize;
        let n = l.parks.iter().filter(|p| p.kind == town::ParkKind::Cemetery).count();
        r.tally(&format!("{} {}: places", l.tier.name(), GROUPS[g]), 1);
        r.tally(&format!("{} {}: cemeteries", l.tier.name(), GROUPS[g]), n);
        let ps = if n > 0 { town::places::of(world, t0, &l) } else { Vec::new() };
        for pk in l.parks.iter().filter(|p| p.kind == town::ParkKind::Cemetery) {
            let by_road = pk.cemetery.is_some_and(|c| c.by_road);
            cemetery_label(&ps, li, pk, &GROUPS, r);
            r.tally(&format!("{}: cemeteries {}", GROUPS[g], if by_road { "by a road" } else { "in town" }), 1);
            check_cemetery(world, t0, &l, li, pk, &GROUPS, r);
            ground.push((li, pk.clone()));
            if shown[g] < 2 {
                shown[g] += 1;
                look.push((g, pk.pts.clone(), town::cemetery::plan(pk).map_or(0, |pl| pl.props.len())));
                let c = centroid(&pk.pts);
                let span = pk.pts.iter().map(|q| dist(*q, c)).fold(0.0, f64::max) * 2.0;
                cams.push(format!("    {} L{li} {}{}: fx {:.6} fy {:.6} zoom {:.1} (battlemap zoom 1.5)", GROUPS[g], l.tier.name(), if by_road { " by a road" } else { " in town" }, c[0] / mw, c[1] / mh, libm::log2(1100.0 / span.max(100.0))));
            }
        }
    }
    // The editor: up to 3 towns' first park-able patch set to a cemetery by hand, as walled and not.
    let with = |ci: usize, edit: worldgen::world::TownEdit| {
        let mut file = world.file.clone();
        file.edits.towns.insert(town::wards::key(ci), edit);
        World::new(file).unwrap()
    };
    let mut tried = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        if tried >= 3 {
            break;
        }
        let Ok(plan) = town::wards::plan_json(world, t0, ci) else { continue };
        let pick = plan["patches"].as_array().unwrap().iter().find(|p| p["in_town"] == true && p.get("park_refused").is_none() && !matches!(p["ward"].as_str(), Some("plaza" | "castle" | "temple"))).and_then(|p| p["patch"].as_u64());
        let Some(p) = pick else { continue };
        tried += 1;
        let walled = !town::layout(world, t0, ci).walls.is_empty();
        for walls in [walled, !walled] {
            let json = format!(r#"{{"patches":[{{"patch":{p},"ward":"park","park":"cemetery"}}],"walls":{walls}}}"#);
            let Ok((Some(edit), _)) = town::wards::change(world, t0, ci, &serde_json::from_str(&json).unwrap()) else {
                r.add("hand cemeteries refused", 1.0, format!("#{ci} patch {p} walls {walls}"));
                continue;
            };
            let w2 = with(ci, edit);
            let l = town::layout(&w2, t0, ci);
            let at = plan["patches"].as_array().unwrap().iter().find(|q| q["patch"].as_u64() == Some(p)).map(|q| [q["at"][0].as_f64().unwrap(), q["at"][1].as_f64().unwrap()]).unwrap();
            let hand = l.parks.iter().find(|pk| pk.kind == town::ParkKind::Cemetery && !pk.cemetery.is_some_and(|c| c.by_road) && dist(centroid(&pk.pts), at) < 200.0);
            r.tally(&format!("hand cemeteries (walls {})", if walls { "on" } else { "off" }), 1);
            match hand {
                Some(pk) => {
                    check_cemetery(&w2, t0, &l, ci, pk, &GROUPS, r);
                    let fi = town::catalog::index_of("tomb").map(|f| f as u16);
                    let tombs = l.buildings.iter().filter(|b| b.func == fi && contains(&pk.pts, centroid(&b.poly))).count();
                    r.tally("hand cemeteries' tombs", tombs);
                    if shown.iter().sum::<usize>() < 12 {
                        let c = centroid(&pk.pts);
                        cams.push(format!("    by hand #{ci} patch {p} walls {walls}: fx {:.6} fy {:.6} zoom 1.5", c[0] / mw, c[1] / mh));
                    }
                }
                None => r.add("hand cemeteries not laid", 1.0, format!("#{ci} patch {p} walls {walls}")),
            }
        }
    }
    for (g, pts, planned) in look {
        let b = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
        let w = ctx.window(b);
        const NAMES: [&str; 7] = ["headstone", "cairn", "standing stone", "yew", "lychgate", "kurgan", "tomb slab"];
        let laid = w.objects.iter().filter(|(p, n)| contains(&pts, *p) && NAMES.contains(&n.as_str())).count();
        r.tally(&format!("{}: battlemap props planned (looked at)", GROUPS[g]), planned);
        r.tally(&format!("{}: battlemap props laid (looked at)", GROUPS[g]), laid);
    }
    for (li, pk) in &ground {
        cemetery_ground(ctx, *li, pk, &GROUPS, r);
    }
    say!("  cemetery cameras:");
    for c in cams {
        say!("{c}");
    }
}

const CEM_RULES: [&str; 12] = [
    "R props or tombs on a walk",
    "R props or tombs over the boundary",
    "R props or tombs outside the plot",
    "R tomb doors not reached from the way in",
    "R kurgans overlapping",
    "R lychgates not on the way in (North, West)",
    "R tombs planned and not built",
    "R tombs with no tomb chamber",
    "R plots by a road within 25 ft of a road as drawn",
    "R tombs within 5 ft of the boundary's face",
    "R yews within 10 ft of the gate or 2 ft of a walk",
    "R cemetery labels on a walk, a tomb, a big prop or the boundary",
];

// R5 towns (M21 fixes): a cemetery as the battlemap lays it.
const CEM_GROUND: [&str; 4] = ["R cemetery squares on a road", "R walk samples off the gravel", "R boundary samples with no wall or hedge", "R ways in not facing the road beside"];

/// A cemetery's battlemap: no road square inside its boundary; its walks (and the link from the road)
/// gravel all along (as the ground shader smooths the squares' gravel: a tent 1.5 squares round, at
/// least half); its wall or hedge all round but for the gate; a way in by a road faces it.
fn cemetery_ground(ctx: &mut Ctx, li: usize, pk: &town::Park, groups: &[&str; 5], r: &mut Report) {
    let Some(pl) = town::cemetery::plan(pk) else { return };
    let g = pk.cemetery.map_or(1, |c| c.group as usize % 5);
    let here = format!("L{li} {} at {}", groups[g], at(town::geom::mean(&pk.pts)));
    let b = pk.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
    let w = ctx.window([b[0] - 60.0, b[1] - 60.0, b[2] + 60.0, b[3] + 60.0]);
    let on_road = (0..w.nx * w.ny).filter(|&k| w.road[k] && contains(&pl.boundary, w.centre(k))).count();
    if on_road > 0 {
        r.add(CEM_GROUND[0], on_road as f64, format!("{here}: {on_road} squares"));
    }
    let cover = |p: P| -> f64 {
        let gx = (p[0] - w.ox) / SQUARE_FT - 0.5;
        let gy = (p[1] - w.oy) / SQUARE_FT - 0.5;
        let (ix, iy) = (gx.floor() as i64, gy.floor() as i64);
        let (mut sum, mut ws) = (0.0, 0.0);
        for j in -1..=2i64 {
            for i in -1..=2i64 {
                let (dx, dy) = (gx - (ix + i) as f64, gy - (iy + j) as f64);
                let wt = (1.5 - dx.abs()).max(0.0) * (1.5 - dy.abs()).max(0.0);
                let (a, c) = (ix + i, iy + j);
                let on = a >= 0 && c >= 0 && (a as usize) < w.nx && (c as usize) < w.ny && w.gravel[c as usize * w.nx + a as usize];
                sum += wt * on as u8 as f64;
                ws += wt;
            }
        }
        sum / ws.max(1e-9)
    };
    let mut walks: Vec<(P, P)> = pl.walks.iter().map(|&(a, e, _)| (a, e)).collect();
    walks.extend(pl.link);
    let (mut bad, mut all) = (0, 0);
    for (a, e) in walks {
        let l = dist(a, e);
        let n = (l / 2.5) as usize;
        for k in 1..n {
            let p = lerp(a, e, k as f64 * 2.5 / l);
            // (Where the link meets the road, the road's.)
            let by_road = (-1..=1).any(|j| (-1..=1).any(|i| w.index(add(p, [i as f64 * SQUARE_FT, j as f64 * SQUARE_FT])).is_some_and(|k| w.road[k])));
            if by_road {
                continue;
            }
            all += 1;
            if cover(p) < 0.5 {
                bad += 1;
            }
        }
    }
    r.tally(&format!("{}: walk samples", groups[g]), all);
    if bad > 0 {
        r.add(CEM_GROUND[1], bad as f64, format!("{here}: {bad}/{all}"));
    }
    if pl.bound != town::cemetery::Bound::None {
        let m = pl.boundary.len();
        let (mut miss, mut all) = (0, 0);
        for e in 0..m {
            let (a, c) = (pl.boundary[e], pl.boundary[(e + 1) % m]);
            let n = (dist(a, c) / 5.0) as usize;
            for k in 1..n {
                let p = lerp(a, c, k as f64 / n as f64);
                if dist(p, pl.gate) < 0.5 * pl.gap + 2.0 {
                    continue;
                }
                all += 1;
                if !w.runs.iter().any(|&(u, v)| seg_dist(p, u, v) < 1.5) {
                    miss += 1;
                }
            }
        }
        r.tally(&format!("{}: boundary samples", groups[g]), all);
        if miss > 0 {
            r.add(CEM_GROUND[2], miss as f64, format!("{here}: {miss}/{all}"));
        }
    }
    if pk.cemetery.is_some_and(|c| c.by_road) {
        let near = |p: P| (0..w.nx * w.ny).filter(|&k| w.road[k]).map(|k| dist(w.centre(k), p)).fold(f64::MAX, f64::min);
        let side = |p: P| (0..w.nx * w.ny).filter(|&k| w.road[k]).map(|k| w.centre(k)).map(|q| if contains(&pk.pts, q) { 0.0 } else { (0..pk.pts.len()).map(|e| seg_dist(q, pk.pts[e], pk.pts[(e + 1) % pk.pts.len()])).fold(f64::MAX, f64::min) }).fold(p[0] * 0.0 + f64::MAX, f64::min);
        let (dg, dm) = (near(pl.gate), side(pl.gate));
        if dm < 40.0 && dg > dm + BOUND_SLACK {
            r.add(CEM_GROUND[3], 1.0, format!("{here}: gate {dg:.0} ft from a road, plot {dm:.0}"));
        }
    }
}

/// How much farther (ft) than the plot's nearest edge a way in by a road may be from it.
const BOUND_SLACK: f64 = 20.0;

/// A cemetery's label (`places::of`): its baseline's every 2 ft off the walks, the tombs, the yews,
/// kurgans and lychgate and 2 ft inside the boundary.
fn cemetery_label(ps: &[town::places::Place], li: usize, pk: &town::Park, groups: &[&str; 5], r: &mut Report) {
    let Some(pl) = town::cemetery::plan(pk) else { return };
    let g = pk.cemetery.map_or(1, |c| c.group as usize % 5);
    let Some(p) = ps.iter().find(|p| p.sort == "cemetery" && p.label.len() >= 2 && contains(&pk.pts, p.label[p.label.len() / 2])) else {
        r.tally(&format!("{}: cemeteries with no label line", groups[g]), 1);
        return;
    };
    r.tally(&format!("{}: cemetery labels", groups[g]), 1);
    let len: f64 = p.label.windows(2).map(|q| dist(q[0], q[1])).sum();
    r.tally(&format!("{}: cemetery label ft", groups[g]), len as usize);
    if len < 36.0 {
        r.tally(&format!("{}: cemetery labels under 36 ft", groups[g]), 1);
    }
    let m = pl.boundary.len();
    let bad = p.label.windows(2).any(|q| {
        let n = (dist(q[0], q[1]) / 2.0).ceil().max(1.0) as usize;
        (0..=n).any(|k| {
            let x = lerp(q[0], q[1], k as f64 / n as f64);
            !contains(&pl.boundary, x)
                || (0..m).any(|e| seg_dist(x, pl.boundary[e], pl.boundary[(e + 1) % m]) < 2.0)
                || pl.walks.iter().any(|&(a, b, w)| seg_dist(x, a, b) < 0.5 * w + 1.0)
                || pl.tombs.iter().any(|t| contains(t, x) || (0..t.len()).any(|e| seg_dist(x, t[e], t[(e + 1) % t.len()]) < 1.0))
                || pl.props.iter().any(|q| match q.2 {
                    Kind::Kurgan => dist(x, q.0) < town::cemetery::KURGAN_R * q.4 as f64,
                    Kind::Yew => dist(x, q.0) < town::cemetery::YEW_R * q.4 as f64,
                    Kind::Lychgate => dist(x, q.0) < 6.0,
                    _ => false,
                })
        })
    });
    if bad {
        r.add(CEM_RULES[11], 1.0, format!("L{li} {} at {}: {}", groups[g], at(town::geom::mean(&pk.pts)), p.name));
    }
}

fn check_cemetery(world: &World, t0: &T0, l: &Layout, li: usize, pk: &town::Park, groups: &[&str; 5], r: &mut Report) {
    let Some(pl) = town::cemetery::plan(pk) else {
        r.add("cemeteries too small to lay out", 1.0, format!("L{li} at {}", at(town::geom::mean(&pk.pts))));
        return;
    };
    let g = pk.cemetery.map_or(1, |c| c.group as usize % 5);
    let here = format!("L{li} {} at {}", groups[g], at(town::geom::mean(&pk.pts)));
    for (k, name) in [(Kind::Headstone, "headstones"), (Kind::Cairn, "cairns"), (Kind::StandingStone, "standing stones"), (Kind::Yew, "yews"), (Kind::Lychgate, "lychgates"), (Kind::Kurgan, "kurgans"), (Kind::TombSlab, "tomb slabs")] {
        let c = pl.props.iter().filter(|p| p.2 == k).count();
        if c > 0 {
            r.tally(&format!("{}: {name}", groups[g]), c);
        }
    }
    r.tally(&format!("{}: tomb buildings", groups[g]), pl.tombs.len());
    let depth = |poly: &[P], p: P| if contains(poly, p) { (0..poly.len()).map(|e| seg_dist(p, poly[e], poly[(e + 1) % poly.len()])).fold(f64::MAX, f64::min) } else { 0.0 };
    let foot = |k: Kind, s: f32| match k {
        Kind::Yew => 2.0,
        Kind::Kurgan => town::cemetery::KURGAN_R * s as f64,
        Kind::TombSlab => 1.5,
        Kind::Cairn => 2.0,
        Kind::StandingStone => 1.2,
        _ => 1.0,
    };
    for &(p, _, k, _, s) in &pl.props {
        if k != Kind::Lychgate && pl.walks.iter().any(|&(a, b, w)| seg_dist(p, a, b) < 0.5 * w + 1.0) {
            r.add(CEM_RULES[0], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
        if k != Kind::Lychgate && depth(&pl.boundary, p) < foot(k, s) {
            r.add(CEM_RULES[1], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
        if !contains(&pk.pts, p) {
            r.add(CEM_RULES[2], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
    }
    // R5 towns (last fix): a plot by a road clear of every road as the map draws it at every level
    // the plot is seen at (the tiles' pieces, `lod::roads::pieces`) and of the layout's roads.
    {
        let b = pk.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
        let geo = &world.geom;
        let mut worst: Option<(f64, String)> = None;
        let mut note = |gap: f64, what: String| {
            if worst.as_ref().is_none_or(|w| gap < w.0) {
                worst = Some((gap, what));
            }
        };
        for lv in 0..=geo.max_level {
            let sp = geo.spacing_ft(lv);
            if sp > worldgen::lod::roads::CARVE_MAX_SPACING_FT {
                continue;
            }
            for pc in worldgen::lod::roads::pieces(&t0.roads, b, 0.0, sp.max(20.0)) {
                let hw = 0.5 * pc.class.width_ft();
                for q in pc.pts.windows(2) {
                    note(town::geom::poly_gap(&pk.pts, &[q[0].p, q[1].p]) - hw, format!("level {lv} ({sp:.0} ft)"));
                }
            }
        }
        for (k, (pts, c, w)) in l.roads.iter().enumerate() {
            for q in pts.windows(2) {
                note(town::geom::poly_gap(&pk.pts, &[q[0], q[1]]) - 0.5 * w, format!("layout road {k} (class {c}, {w:.0} ft)"));
            }
        }
        if let Some((gap, what)) = worst.filter(|_| pk.cemetery.is_some_and(|c| c.by_road)) {
            if gap < town::cemetery::VERGE - 5.0 {
                r.add(CEM_RULES[8], 1.0, format!("{here}: {gap:.1} ft from the road at {what}"));
            }
        }
    }
    let face = |pl: &town::cemetery::Plan| if pl.bound == town::cemetery::Bound::None { 0.0 } else { 1.875 };
    for t in &pl.tombs {
        if t.iter().any(|&q| depth(&pl.boundary, q) < 5.0 + face(&pl)) {
            r.add(CEM_RULES[9], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
    }
    let gap_seg = (sub(pl.gate, mul(pl.along, 0.5 * pl.gap)), add(pl.gate, mul(pl.along, 0.5 * pl.gap)));
    for &(p, _, k, _, s) in pl.props.iter().filter(|p| p.2 == Kind::Yew) {
        let rr = town::cemetery::YEW_R * s as f64;
        if seg_dist(p, gap_seg.0, gap_seg.1) < rr + 10.0 || pl.walks.iter().any(|&(a, b, w)| seg_dist(p, a, b) < 0.5 * w + rr + 2.0) {
            r.add(CEM_RULES[10], 1.0, format!("{here}: {:?} at {}", k, at(p)));
        }
    }
    let seg_poly = |t: &[P], a: P, b: P| town::geom::poly_gap(t, &[a, b]);
    for t in &pl.tombs {
        if pl.walks.iter().any(|&(a, b, w)| seg_poly(t, a, b) < 0.5 * w + 1.0 || contains(t, a) || contains(t, b)) {
            r.add(CEM_RULES[0], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
        if t.iter().any(|&q| depth(&pl.boundary, q) <= 0.0) {
            r.add(CEM_RULES[1], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
        if t.iter().any(|&q| !contains(&pk.pts, q)) {
            r.add(CEM_RULES[2], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
    }
    // Doors reached: a 2.5-ft grid inside the boundary (and the gate's gap), off the tombs and what
    // blocks the way, from just inside the gate.
    let (x0, y0, x1, y1) = pl.boundary.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    const C: f64 = 2.5;
    let (nx, ny) = (((x1 - x0) / C) as usize + 3, ((y1 - y0) / C) as usize + 3);
    let cell = |p: P| -> Option<usize> {
        let (i, j) = (((p[0] - x0) / C).floor() as i64 + 1, ((p[1] - y0) / C).floor() as i64 + 1);
        (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    let mid = |k: usize| [x0 + ((k % nx) as f64 - 0.5) * C, y0 + ((k / nx) as f64 - 0.5) * C];
    let blockers: Vec<(P, f64)> = pl.props.iter().filter(|p| worldgen::battlemap::info(p.2 as u16).blocks_move).map(|p| (p.0, foot(p.2, p.4))).collect();
    let open: Vec<bool> = (0..nx * ny)
        .map(|k| {
            let p = mid(k);
            (contains(&pl.boundary, p) || dist(p, pl.gate) < 0.5 * pl.gap)
                && !pl.tombs.iter().any(|t| contains(t, p) || depth(t, p) > 0.0)
                && !blockers.iter().any(|&(q, rr)| dist(p, q) < rr)
        })
        .collect();
    let mut seen = vec![false; nx * ny];
    if let Some(s0) = cell(add(pl.gate, mul(pl.inward, 1.5))).filter(|&k| open[k]) {
        let mut q = VecDeque::from([s0]);
        seen[s0] = true;
        while let Some(k) = q.pop_front() {
            let (x, y) = ((k % nx) as i64, (k / nx) as i64);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (x + dx, y + dy);
                if a < 0 || b < 0 || a >= nx as i64 || b >= ny as i64 {
                    continue;
                }
                let n = b as usize * nx + a as usize;
                if !seen[n] && open[n] {
                    seen[n] = true;
                    q.push_back(n);
                }
            }
        }
    }
    for t in &pl.tombs {
        let door = lerp(t[0], t[1], 0.5);
        let out = sub(door, town::geom::mean(t));
        let front = add(door, mul(out, 2.0 / len(out).max(1e-9)));
        if !cell(front).is_some_and(|k| seen[k]) {
            r.add(CEM_RULES[3], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
        }
    }
    let ks: Vec<(P, f64)> = pl.props.iter().filter(|p| p.2 == Kind::Kurgan).map(|p| (p.0, town::cemetery::KURGAN_R * p.4 as f64)).collect();
    for a in 0..ks.len() {
        for b in a + 1..ks.len() {
            if dist(ks[a].0, ks[b].0) < ks[a].1 + ks[b].1 {
                r.add(CEM_RULES[4], 1.0, format!("{here}: at {}", at(ks[a].0)));
            }
        }
    }
    if g <= 1 && !pl.walks.is_empty() && !pl.props.iter().any(|p| p.2 == Kind::Lychgate && dist(p.0, pl.gate) < 2.0) {
        r.add(CEM_RULES[5], 1.0, here.clone());
    }
    let fi = town::catalog::index_of("tomb").map(|f| f as u16);
    for t in &pl.tombs {
        let Some(b) = l.buildings.iter().find(|b| b.func == fi && b.poly.len() == t.len() && b.poly.iter().zip(t).all(|(p, q)| dist(*p, *q) < 0.01)) else {
            r.add(CEM_RULES[6], 1.0, format!("{here}: tomb at {}", at(town::geom::mean(t))));
            continue;
        };
        let id = format!("b:{li}:{}", b.id);
        let lv = interior::generate_id(world, t0, &id).and_then(|it| it.levels.get(it.entry_level).cloned());
        let chamber = lv.as_ref().is_some_and(|lv| lv.rooms.iter().any(|q| q.kind == "tomb chamber"));
        if !chamber {
            r.add(CEM_RULES[7], 1.0, format!("{here}: {id}"));
        } else if lv.is_some_and(|lv| lv.furniture.iter().any(|f| f.kind == "sarcophagus")) {
            r.tally(&format!("{}: tombs with a sarcophagus", groups[g]), 1);
        }
    }
}

// R5 towns
/// `district_edits`' counts, in order.
const DISTRICT_COUNTS: [&str; 16] = [
    "district edits",
    "district edits refused",
    "patches in no district or two after a district edit",
    "patches not in the district they were put in",
    "district labels outside their district",
    "districts emptied with a label left, or with patches and none",
    "untouched districts with their id, name, kind or patches changed",
    "district edits giving other districts with the walls off",
    "new districts not named as asked",
    "renamed buildings laid out anew",
    "renames kept with their id",
    "renames carried",
    "renames carried to another function",
    "renames reported lost",
    "renames lost without a report",
    "district ids reused after one was emptied",
];

/// Districts drawn by hand (`patches[].district`), in up to `PLACES` towns (default 8) of four
/// districts or more: a patch into a neighbour's district, into a far one, two into a new named
/// one, and every patch of a small district moved out; each with the walls as generated and the
/// other way. A patch is in one district after, where it was put; the label of a changed one is on
/// it, an emptied one has none; the others are as generated (id, name, kind, patches) and the walls
/// change none. Then a renamed business's patch rerolled, the castle's keep renamed and its patch
/// rerolled, and a noted home's patch made empty: each rename and note is kept with its id, carried
/// (`renames_carried`, to the same function) or reported (`renames_lost`), never lost silently.
fn district_edits(base: &World, t0: &T0, r: &mut Report) {
    use serde_json::{Value, json};
    r.counts(&DISTRICT_COUNTS);
    let places: usize = std::env::var("PLACES").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let with = |w: &World, ci: usize, edit: Option<worldgen::world::TownEdit>| {
        let mut file = w.file.clone();
        if let Some(e) = edit {
            file.edits.towns.insert(town::wards::key(ci), e);
        }
        World::new(file).unwrap()
    };
    let qi_of = |d: &Value| d["id"].as_str().and_then(|s| s.rsplit(':').next()).and_then(|q| q.parse::<usize>().ok());
    let summary = |plan: &Value| -> Vec<(String, String, String, Vec<u64>)> {
        plan["districts"].as_array().into_iter().flatten().map(|d| (d["id"].as_str().unwrap_or("").to_string(), d["name"].as_str().unwrap_or("").to_string(), d["kind"].as_str().unwrap_or("").to_string(), d["patches"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect())).collect()
    };
    let mut done = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        if done >= places {
            break;
        }
        let Ok(plan) = town::wards::plan_json(base, t0, ci) else { continue };
        let ds = summary(&plan);
        if ds.len() < 4 {
            continue;
        }
        done += 1;
        let g = town::layout(base, t0, ci);
        let n = ds.len();
        let pats: Vec<&Value> = plan["patches"].as_array().unwrap().iter().collect();
        let dq = |p: u64| pats.iter().find(|x| x["patch"].as_u64() == Some(p)).and_then(|x| qi_of(&x["district"]));
        let nbs = |p: u64| pats.iter().find(|x| x["patch"].as_u64() == Some(p)).map(|x| x["neighbours"].as_array().unwrap().iter().filter_map(Value::as_u64).collect::<Vec<_>>()).unwrap_or_default();
        let did = |q: usize| format!("d:{ci}:{q}");
        // (name, request patches, where each should be after, a district to come out empty)
        let mut cases: Vec<(&str, Vec<Value>, Vec<(u64, usize)>, Option<usize>)> = Vec::new();
        let placed: Vec<(u64, usize)> = pats.iter().filter_map(|x| Some((x["patch"].as_u64()?, dq(x["patch"].as_u64()?)?))).filter(|(_, q)| ds[*q].3.len() >= 2).collect();
        if let Some((p, b)) = placed.iter().find_map(|&(p, a)| nbs(p).into_iter().filter_map(dq).find(|&b| b != a).map(|b| (p, b))) {
            cases.push(("into a neighbour's", vec![json!({ "patch": p, "district": did(b) })], vec![(p, b)], None));
        }
        if let Some((p, c)) = placed.iter().find_map(|&(p, a)| {
            let near: Vec<usize> = nbs(p).into_iter().filter_map(dq).collect();
            (0..n).rev().find(|&c| c != a && !near.contains(&c) && !ds[c].3.is_empty()).map(|c| (p, c))
        }) {
            cases.push(("into a far one", vec![json!({ "patch": p, "district": did(c) })], vec![(p, c)], None));
        }
        if let (Some(&(p1, a)), Some(&(p2, _))) = (placed.first(), placed.iter().rev().find(|x| x.1 != placed[0].1)) {
            let _ = a;
            cases.push(("two into a new one", vec![json!({ "patch": p1, "district": "Checked Ward" }), json!({ "patch": p2, "district": "checked ward" })], vec![(p1, n), (p2, n)], None));
        }
        if let Some(e) = (0..n).find(|&e| (1..=4).contains(&ds[e].3.len()) && ds[e].3.iter().all(|&m| nbs(m).into_iter().filter_map(dq).any(|q| q != e))) {
            let to: Vec<(u64, usize)> = ds[e].3.iter().map(|&m| (m, nbs(m).into_iter().filter_map(dq).find(|&q| q != e).unwrap())).collect();
            cases.push(("every patch moved out", to.iter().map(|&(m, q)| json!({ "patch": m, "district": did(q) })).collect(), to, Some(e)));
        }
        // (R5 towns: a new district emptied, then another made: a new id, not the emptied one's.)
        if let Some(&(p1, _)) = placed.first() {
            let step = |w: &World, d: &str| -> Option<Option<worldgen::world::TownEdit>> {
                let req: town::wards::TownRequest = serde_json::from_value(json!({ "patches": [{ "patch": p1, "district": d }] })).ok()?;
                town::wards::change(w, t0, ci, &req).ok().map(|x| x.0)
            };
            let got = step(base, "Checked Ward").flatten().and_then(|e1| {
                let w1 = with(base, ci, Some(e1));
                let w2 = with(base, ci, step(&w1, "auto")?);
                let w3 = with(base, ci, step(&w2, "Later Ward")??.into());
                let plan3 = town::wards::plan_json(&w3, t0, ci).ok()?;
                plan3["patches"].as_array()?.iter().find(|x| x["patch"].as_u64() == Some(p1)).and_then(|x| x["district"]["id"].as_str().map(str::to_string))
            });
            r.tally("district ids checked after one was emptied", 1);
            if got.as_deref() != Some(did(n + 1).as_str()) {
                r.add("district ids reused after one was emptied", 1.0, format!("#{ci}: patch {p1} in {got:?}, not {}", did(n + 1)));
            }
        }
        let walled = plan["walls"]["built"].as_bool().unwrap_or(false);
        for (name, req, want, emptied) in cases {
            let tag = format!("#{ci} {name}");
            let mut outs: Vec<Vec<(String, String, String, Vec<u64>)>> = Vec::new();
            for walls in [None, Some(!walled)] {
                let mut rq = json!({ "patches": req });
                if let Some(w) = walls {
                    rq["walls"] = json!(w);
                }
                r.tally("district edits", 1);
                let req: town::wards::TownRequest = serde_json::from_value(rq).unwrap();
                let edit = match town::wards::change(base, t0, ci, &req) {
                    Ok((Some(e), _)) => e,
                    Ok((None, _)) => {
                        r.add("district edits refused", 1.0, format!("{tag}: no edit"));
                        continue;
                    }
                    Err(why) => {
                        r.add("district edits refused", 1.0, format!("{tag}: {why}"));
                        continue;
                    }
                };
                let w = with(base, ci, Some(edit));
                let (plan2, l) = (town::wards::plan_json(&w, t0, ci).unwrap(), town::layout(&w, t0, ci));
                let d2 = summary(&plan2);
                let tag = format!("{tag} walls {}", if walls.is_some() { "toggled" } else { "as generated" });
                // One district each.
                let mut count: BTreeMap<u64, usize> = BTreeMap::new();
                for (_, _, _, m) in &d2 {
                    for &p in m {
                        *count.entry(p).or_default() += 1;
                    }
                }
                for (p, _) in placed.iter().chain(&want) {
                    let k = count.get(p).copied().unwrap_or(0);
                    if k != 1 {
                        r.add("patches in no district or two after a district edit", 1.0, format!("{tag}: patch {p} in {k}"));
                    }
                }
                if count.values().any(|&k| k > 1) {
                    r.add("patches in no district or two after a district edit", 1.0, format!("{tag}: a patch in two"));
                }
                for &(p, q) in &want {
                    if !d2.get(q).is_some_and(|d| d.3.contains(&p)) {
                        r.add("patches not in the district they were put in", 1.0, format!("{tag}: patch {p} not in d:{ci}:{q}"));
                    }
                }
                if name == "two into a new one" && (d2.get(n).is_none_or(|d| d.1 != "Checked Ward") || l.quarters.get(n).is_none_or(|q| q.name != "Checked Ward") || d2.len() != n + 1) {
                    r.add("new districts not named as asked", 1.0, format!("{tag}: {:?}", d2.get(n).map(|d| &d.1)));
                }
                // Labels: on a changed district, none on an emptied one.
                for (qi, q) in l.quarters.iter().enumerate() {
                    let changed = plan2["districts"][qi]["changed"] == true || plan2["districts"][qi]["by_hand"] == true;
                    if q.patches.is_empty() != q.label.is_empty() || (Some(qi) == emptied && !q.patches.is_empty()) {
                        r.add("districts emptied with a label left, or with patches and none", 1.0, format!("{tag}: d:{ci}:{qi} {} patches, {} label points", q.patches.len(), q.label.len()));
                    }
                    if changed && !q.label.is_empty() {
                        let mid = q.label[q.label.len() / 2];
                        let off = q.patches.iter().map(|poly| if contains(poly, mid) { 0.0 } else { poly.iter().zip(poly.iter().cycle().skip(1)).map(|(a, b)| seg_dist(mid, *a, *b)).fold(f64::MAX, f64::min) }).fold(f64::MAX, f64::min);
                        if off > 15.0 {
                            r.add("district labels outside their district", off, format!("{tag}: d:{ci}:{qi} label {off:.0} ft out at {}", at(mid)));
                        }
                    }
                    // The rest as generated (their labels too, with the walls as generated: one
                    // over a park moves off its middle as the park is laid out).
                    if qi < n && !changed && (d2[qi] != ds[qi] || g.quarters.get(qi).is_none_or(|o| o.name != q.name || o.kind != q.kind || (walls.is_none() && o.label != q.label))) {
                        r.add("untouched districts with their id, name, kind or patches changed", 1.0, format!("{tag}: d:{ci}:{qi} {:?} -> {:?}", ds[qi], d2[qi]));
                    }
                }
                outs.push(d2);
            }
            if outs.len() == 2 && outs[0] != outs[1] {
                r.add("district edits giving other districts with the walls off", 1.0, format!("{tag}: districts differ"));
            }
        }
        // Renames and notes kept when a patch is laid out anew.
        let corner: BTreeMap<u64, P> = plan["corners"].as_array().unwrap().iter().map(|c| (c["corner"].as_u64().unwrap(), [c["at"][0].as_f64().unwrap(), c["at"][1].as_f64().unwrap()])).collect();
        let patch_of = |p: P| pats.iter().find(|x| contains(&x["corners"].as_array().unwrap().iter().filter_map(|v| corner.get(&v.as_u64()?).copied()).collect::<Vec<_>>(), p)).and_then(|x| x["patch"].as_u64());
        let castle = town::catalog::index_of("castle").map(|c| c as u16);
        let business = g.buildings.iter().find(|b| b.func.is_some() && b.func != castle && b.keep.is_none() && patch_of(centroid(&b.poly)).is_some_and(|p| pats.iter().any(|x| x["patch"].as_u64() == Some(p) && x["in_town"] == true)));
        let keep = g.buildings.iter().find(|b| b.keep.is_some());
        let home = g.buildings.iter().find(|b| b.func.is_none() && b.structure == town::Structure::Roofed && patch_of(centroid(&b.poly)).is_some_and(|p| pats.iter().any(|x| x["patch"].as_u64() == Some(p) && x["in_town"] == true && x["ward"] == "common")));
        let mut f2: Vec<(&str, &town::Building, Value, bool)> = Vec::new();
        for (what, b, note) in [("a renamed business rerolled", business, false), ("the castle's keep renamed, rerolled", keep, false), ("a noted home's patch made empty", home, true)] {
            let Some(b) = b else { continue };
            let Some(p) = patch_of(centroid(&b.poly)) else { continue };
            let req = if note { json!({ "patches": [{ "patch": p, "ward": "empty" }] }) } else { json!({ "patches": [{ "patch": p, "reroll": true }] }) };
            f2.push((what, b, req, note));
        }
        for (what, b, req, note) in f2 {
            let tag = format!("#{ci} {what}");
            let key = format!("b:{ci}:{}", b.id);
            let mut file = base.file.clone();
            if note {
                file.edits.notes.insert(key.clone(), worldgen::world::Note { text: "Checked note".into(), tags: Vec::new() });
            } else {
                file.edits.renames.insert(key.clone(), "Checked Name".into());
            }
            let w1 = World::new(file).unwrap();
            let req: town::wards::TownRequest = serde_json::from_value(req).unwrap();
            let Ok((edit, rep)) = town::wards::change(&w1, t0, ci, &req) else { continue };
            let after = town::layout(&with(&w1, ci, edit), t0, ci);
            r.tally("renamed buildings laid out anew", 1);
            let listed = |k: &str, field: &str| rep[k].as_array().into_iter().flatten().find(|x| x[field] == key.as_str()).cloned();
            if after.buildings.iter().any(|x| x.id == b.id) {
                r.tally("renames kept with their id", 1);
            } else if let Some(c) = listed("renames_carried", "from") {
                r.tally("renames carried", 1);
                let to: u32 = c["to"].as_str().and_then(|s| s.rsplit(':').next()).and_then(|s| s.parse().ok()).unwrap_or(u32::MAX);
                let nb = after.buildings.iter().find(|x| x.id == to);
                let ok = nb.is_some_and(|x| x.func == b.func || (b.keep.is_some() && x.keep.is_some()));
                if !ok {
                    r.add("renames carried to another function", 1.0, format!("{tag}: {key} -> {to}"));
                }
            } else if listed("renames_lost", "id").is_some() {
                r.add("renames reported lost", 1.0, format!("{tag}: {key}"));
            } else {
                r.add("renames lost without a report", 1.0, format!("{tag}: {key} at {}", at(centroid(&b.poly))));
            }
        }
    }
}

// R5 towns (M16b): squares, parks and gates (`town::places`).

const PLACE_COUNTS: [&str; 8] = [
    "places sharing an id",
    "places unnamed",
    "place labels over a wall or building",
    "place ids moved or gone after an unrelated patch edit",
    "place ids moved or gone with the walls toggled",
    "generated place names changed by an edit elsewhere",
    "gates listed while the town is unwalled",
    "unrelated patch edits refused",
];

/// `places`: per tier, the squares, parks and gates with ids and names; that no two share an id,
/// every one is named, no label lies over a wall or building, an edit to a patch none of them
/// stands on moves none of their ids, nor do the walls toggled (gates aside: only while walled).
/// Towns and up, `PLACES` of them (default 12); a few towns' places printed per tier.
fn places(base: &World, t0: &T0, r: &mut Report) {
    use serde_json::json;
    use worldgen::town::places::{Place, label_blocked, of};
    r.counts(&PLACE_COUNTS);
    let most: usize = std::env::var("PLACES").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let mut shown: HashMap<&str, usize> = HashMap::new();
    let tiers = ["village", "town", "city", "metropolis"];
    let mut done = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town) {
        if done >= most {
            break;
        }
        done += 1;
        let tier = tiers[t0.settlements[ci].tier as usize];
        let l = town::layout(base, t0, ci);
        let ps = of(base, t0, &l);
        for k in ["square", "park", "gate"] {
            r.tally(&format!("{tier}: {k}s"), ps.iter().filter(|p| p.kind == k).count());
        }
        r.tally(&format!("{tier}: labels"), ps.iter().filter(|p| !p.label.is_empty()).count());
        r.tally(&format!("{tier}: towns"), 1);
        let n = shown.entry(tier).or_default();
        if *n < 2 {
            *n += 1;
            let list: Vec<String> = ps.iter().map(|p| format!("{} {} ({})", p.id, p.name, p.sort)).collect();
            say!("    {tier} {ci}: {}", list.join("; "));
        }
        let mut ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        for w in ids.windows(2).filter(|w| w[0] == w[1]) {
            r.add("places sharing an id", 1.0, format!("{ci}: {}", w[0]));
        }
        for p in &ps {
            if p.name.trim().is_empty() {
                r.add("places unnamed", 1.0, format!("{ci}: {}", p.id));
            }
            if label_blocked(&l, &p.label) {
                r.add("place labels over a wall or building", 1.0, format!("{ci}: {} {}", p.id, p.name));
            }
        }
        let gates_off = |l: &Layout, ps: &[Place]| l.walls.is_empty() && ps.iter().any(|p| p.kind == "gate");
        if gates_off(&l, &ps) {
            r.add("gates listed while the town is unwalled", 1.0, format!("{ci}"));
        }
        // Each of `ps` (but those `skip` keeps out) by its id in `now`, where it was.
        let kept = |now: &[Place], skip: &dyn Fn(&Place) -> bool, label: &str, tag: &str, r: &mut Report| {
            for p in ps.iter().filter(|p| !skip(p)) {
                match now.iter().find(|q| q.id == p.id) {
                    Some(q) if dist(q.at, p.at) <= 40.0 && q.kind == p.kind => {
                        if q.name != p.name {
                            r.add("generated place names changed by an edit elsewhere", 1.0, format!("{ci} {tag}: {} {} -> {}", p.id, p.name, q.name));
                        }
                    }
                    Some(q) => r.add(label, dist(q.at, p.at), format!("{ci} {tag}: {} moved {:.0} ft", p.id, dist(q.at, p.at))),
                    None => r.add(label, 1.0, format!("{ci} {tag}: {} {} gone", p.id, p.name)),
                }
            }
        };
        let Ok(plan) = town::wards::plan_json(base, t0, ci) else { continue };
        // A patch built on lots none of them stands on, nor any square at its corners, farthest
        // from the centre among the inner ones.
        let corner_of = |p: &Place| p.id.rsplit(':').next().and_then(|s| s.parse::<u64>().ok());
        let used_corners: Vec<u64> = ps.iter().filter(|p| p.kind == "square").filter_map(corner_of).collect();
        let far = plan["patches"].as_array().into_iter().flatten().filter(|x| {
            let p = x["patch"].as_u64().unwrap_or(0) as u32;
            matches!(x["ward"].as_str(), Some("common" | "craft" | "merchant" | "noble" | "slum")) && x["in_town"] == true
                && !ps.iter().any(|q| q.patch == Some(p))
                && !x["corners"].as_array().into_iter().flatten().filter_map(|c| c.as_u64()).any(|c| used_corners.contains(&c))
        }).max_by(|a, b| {
            let c0 = [plan["center"][0].as_f64().unwrap_or(0.0), plan["center"][1].as_f64().unwrap_or(0.0)];
            let d = |x: &serde_json::Value| dist([x["at"][0].as_f64().unwrap_or(0.0), x["at"][1].as_f64().unwrap_or(0.0)], c0);
            d(a).total_cmp(&d(b))
        }).and_then(|x| Some((x["patch"].as_u64()?, if x["ward"] == "noble" { "common" } else { "noble" })));
        if let Some((p, to)) = far {
            let req: town::wards::TownRequest = serde_json::from_value(json!({ "patches": [{ "patch": p, "ward": to }] })).unwrap();
            match town::wards::change(base, t0, ci, &req) {
                Ok((Some(e), _)) => {
                    let l2 = town::wards::with_edit(base, t0, ci, &e);
                    // (A road's cemetery may come or go as the town's commons change.)
                    kept(&of(base, t0, &l2), &|q| q.id.contains(":r"), "place ids moved or gone after an unrelated patch edit", &format!("patch {p} to {to}"), r);
                }
                Ok((None, _)) => r.add("unrelated patch edits refused", 1.0, format!("{ci}: patch {p}: no change")),
                Err(e) => r.add("unrelated patch edits refused", 1.0, format!("{ci}: patch {p}: {e}")),
            }
        }
        let walled = plan["walls"]["built"].as_bool().unwrap_or(false);
        let req: town::wards::TownRequest = serde_json::from_value(json!({ "walls": !walled })).unwrap();
        if let Ok((Some(e), _)) = town::wards::change(base, t0, ci, &req) {
            let l2 = town::wards::with_edit(base, t0, ci, &e);
            let now = of(base, t0, &l2);
            if gates_off(&l2, &now) {
                r.add("gates listed while the town is unwalled", 1.0, format!("{ci} walls toggled"));
            }
            kept(&now, &|q| q.kind == "gate" || q.id.contains(":r"), "place ids moved or gone with the walls toggled", "walls toggled", r);
        }
    }
}

// R5 towns
/// Walls drawn by hand round patches (M18): per tier up to 3 towns, a ring round 3-6 patches
/// near the middle, one round the whole old town (its own walls taken down if it has them), an
/// open line across it. Rules (each should be 0): a road, approach or main street crossing with
/// no gate; a ring with no way in; a building cut by a hand wall; a ring not closed; the town's
/// own walls on or off changing a hand wall. Then gate counts.
fn handwalls(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::t0::settle::Tier;
    use worldgen::town::handwall;
    use worldgen::world::{Created, Edits, WorldFile};
    const RULES: [&str; 5] = ["crossing with no gate", "ring with no way in", "building cut by a hand wall", "ring not closed", "hand wall changed by town walls on/off"];
    r.counts(&RULES);
    r.counts(&["hand walls refused"]);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let n = t0.settlements.len();
    let mut towns: Vec<usize> = Vec::new();
    for tier in [Tier::Town, Tier::City, Tier::Metropolis] {
        towns.extend((0..n).filter(|&i| t0.settlements[i].tier == tier).take(3));
    }
    // (The towns with walls, taken down for the ring round the old town; toggled for the last rule.)
    let toggle = |ci: usize, on: bool| town::wards::change(base, t0, ci, &serde_json::from_str(&format!(r#"{{"walls":{on}}}"#)).unwrap()).ok().and_then(|x| x.0);
    let mut off = base.file.edits.clone();
    for &ci in &towns {
        let walled = town::wards::plan_json(base, t0, ci).is_ok_and(|p| p["walls"]["built"] == true);
        if walled && let Some(e) = toggle(ci, false) {
            off.towns.insert(town::wards::key(ci), e);
        }
    }
    let w1 = World::new(WorldFile { edits: off.clone(), ..base.file.clone() }).unwrap();
    // (case label, town, wall, main crossings, sides with no street)
    let mut cases: Vec<(String, usize, Created, Vec<P>)> = Vec::new();
    let mut bare: HashMap<String, Vec<(P, P)>> = HashMap::new();
    for &ci in &towns {
        let s = &t0.settlements[ci];
        let Ok(plan) = town::wards::plan_json(&w1, t0, ci) else { continue };
        let patches: Vec<&serde_json::Value> = plan["patches"].as_array().unwrap().iter().filter(|p| p["in_town"] == true).collect();
        let num = |p: &serde_json::Value| p["patch"].as_u64().unwrap() as usize;
        let at = |p: &serde_json::Value| [p["at"][0].as_f64().unwrap(), p["at"][1].as_f64().unwrap()];
        let Some(first) = patches.iter().min_by(|a, b| dist(at(a), [s.x, s.y]).total_cmp(&dist(at(b), [s.x, s.y]))) else { continue };
        let want = 3 + ci % 4;
        let mut few = vec![num(first)];
        let mut k = 0;
        while few.len() < want && k < few.len() {
            let p = patches.iter().find(|p| num(p) == few[k]).unwrap();
            for nb in p["neighbours"].as_array().unwrap().iter().filter_map(|v| v.as_u64()).map(|v| v as usize) {
                if few.len() < want && !few.contains(&nb) && patches.iter().any(|q| num(q) == nb) {
                    few.push(nb);
                }
            }
            k += 1;
        }
        let all: Vec<usize> = patches.iter().map(|p| num(p)).collect();
        for (label, set) in [("ring round patches", few), ("ring round the old town", all)] {
            match handwall::around_patches(&w1, t0, ci, &set, false) {
                Ok(ring) => {
                    let c = Created { kind: "wall".into(), pts: ring.pts.clone(), gates: ring.gates.clone(), closed: true, ..Default::default() };
                    let main = ring.crossings.iter().filter(|x| x.main).map(|x| x.at).collect();
                    r.tally(&format!("{label}: side streets closed"), ring.crossings.iter().filter(|x| !x.gate && !x.main).count());
                    let name = format!("{label} #{ci} ({} patches)", set.len());
                    let m = ring.pts.len();
                    bare.insert(name.clone(), ring.bare.iter().map(|&i| (ring.pts[i], ring.pts[(i + 1) % m])).collect());
                    cases.push((name, ci, c, main));
                }
                Err(e) => r.add("hand walls refused", 0.0, format!("{label} #{ci}: {e}")),
            }
        }
        // (An open line across it, the first way round that stays out of the water.)
        let half = 0.6 * town::reach(s).min(1200.0);
        for j in 0..16 {
            let ang = ((ci + j) % 16) as f64 * std::f64::consts::PI / 16.0;
            let d = [ang.cos() * half, ang.sin() * half];
            let mut c = Created { kind: "wall".into(), x: s.x, y: s.y, pts: vec![sub([s.x, s.y], d), add([s.x, s.y], d)], ..Default::default() };
            c.id = "c:0".into();
            if j == 15 || worldgen::agent::works_spot(&w1, t0, &c, "c:0", None).is_ok() {
                cases.push((format!("open line #{ci}"), ci, c, Vec::new()));
                break;
            }
        }
    }
    // Checked like a new one (`works_spot`); those it takes go in the world together.
    let mut kept: Vec<(String, usize, Created, Vec<P>)> = Vec::new();
    for (label, ci, mut c, main) in cases {
        let k = c.pts.len() as f64;
        (c.x, c.y) = (c.pts.iter().map(|q| q[0]).sum::<f64>() / k, c.pts.iter().map(|q| q[1]).sum::<f64>() / k);
        c.id = format!("c:{}", off.created.len() + kept.len());
        match c.check().and_then(|_| worldgen::agent::works_spot(&w1, t0, &c, &c.id, None)) {
            Err(e) => {
                r.add("hand walls refused", 0.0, format!("{label}: {e}"));
                continue;
            }
            // (Taken away with it: `remove_in_way`.)
            Ok(s) => {
                let kind = label.split(" #").next().unwrap();
                r.tally(&format!("{kind}: buildings in its way (remove_in_way)"), s.in_way.len());
                let sides = bare.get(&label).cloned().unwrap_or_default();
                for (id, at) in &s.in_way {
                    if kind != "open line" && !sides.iter().any(|&(a, q)| seg_dist(*at, a, q) < 40.0) {
                        r.add(&format!("{kind}: in its way off a side with no street"), 0.0, format!("{label}: {id} at ({:.0}, {:.0})", at[0], at[1]));
                    }
                }
                for (id, at) in s.in_way {
                    off.buildings.insert(id, worldgen::world::BuildingEdit { at, removed: true, ..Default::default() });
                }
            }
        }
        kept.push((label, ci, c, main));
    }
    let mut e3: Edits = off.clone();
    e3.created.extend(kept.iter().map(|k| k.2.clone()));
    let w3 = World::new(WorldFile { edits: e3.clone(), ..base.file.clone() }).unwrap();
    let mut t3 = T0::from_bytes(&t0.to_bytes()).unwrap();
    t3.apply_edits(&w3);
    let first = n + t3.base_pois + off.created.len();
    let look = |l: &Layout| format!("{:?}", (&l.walls, &l.towers, &l.gate_towers));
    let mut looks: Vec<String> = Vec::new();
    for (k, (label, ci, c, main)) in kept.iter().enumerate() {
        let l = town::layout(&w3, &t3, first + k);
        looks.push(look(&l));
        let gates = handwall::gates_of(&l);
        let kind = label.split(" #").next().unwrap();
        r.tally(&format!("{kind}: walls"), 1);
        r.tally(&format!("{kind}: gates"), gates.len());
        r.value(&format!("{kind}: gates each"), gates.len() as f64, label.clone());
        // (A gate's two towers as far apart as a generated gate's on a straight wall: 52 ft.)
        let gt = &l.gate_towers;
        for k in 1..gt.len() {
            let d = dist(gt[k - 1], gt[k]);
            if d < 60.0 {
                r.measure(&format!("{kind}: gate towers apart (ft), least"), true, d, format!("{label} at ({:.0}, {:.0})", gt[k][0], gt[k][1]));
            }
        }
        let mut crossings = handwall::road_crossings(&w3, &t3, &c.pts, c.closed);
        crossings.extend(main.iter().copied());
        r.tally(&format!("{kind}: road/approach/main crossings"), crossings.len());
        for q in &crossings {
            // (None in the water, where the wall is broken.)
            if !gates.iter().any(|g| dist(*g, *q) < 65.0) && l.walls.iter().any(|w| w.windows(2).any(|s| seg_dist(*q, s[0], s[1]) < 30.0)) {
                r.add("crossing with no gate", 0.0, format!("{label} at ({:.0}, {:.0})", q[0], q[1]));
            }
        }
        if c.closed {
            if gates.is_empty() {
                r.add("ring with no way in", 0.0, label.clone());
            }
            let m = c.pts.len();
            let ring: f64 = (0..m).map(|i| dist(c.pts[i], c.pts[(i + 1) % m])).sum();
            // (Every 10 ft of it on dry ground: on a wall piece or in a gate.)
            let (x0, y0) = c.pts.iter().fold((f64::MAX, f64::MAX), |a, p| (a.0.min(p[0]), a.1.min(p[1])));
            let (x1, y1) = c.pts.iter().fold((f64::MIN, f64::MIN), |a, p| (a.0.max(p[0]), a.1.max(p[1])));
            let shore = town::Shore::new(&w3, &t3, [x0, y0, x1, y1]);
            let mut short = 0.0;
            for i in 0..m {
                let (a, b) = (c.pts[i], c.pts[(i + 1) % m]);
                let k = (dist(a, b) / 10.0).ceil().max(1.0) as usize;
                for j in 0..k {
                    let p = lerp(a, b, (j as f64 + 0.5) / k as f64);
                    let q = worldgen::lod::rivers::clear_of_rivers(&t3.rivers, p[0], p[1], 6.0, t3.cell_ft);
                    let wet = shore.wet(p) || (q.0 - p[0]).abs() + (q.1 - p[1]).abs() > 0.01;
                    // (In a gate: between its towers, round the bend where one stands on a corner.)
                    let gt = &l.gate_towers;
                    let in_gate = (1..gt.len()).any(|k| dist(gt[k - 1], gt[k]) < 60.0 && dist(p, gt[k - 1]) + dist(p, gt[k]) < 128.0);
                    let laid = l.walls.iter().any(|w| w.windows(2).any(|s| seg_dist(p, s[0], s[1]) < 6.0)) || gates.iter().any(|g| dist(*g, p) < 32.0) || in_gate;
                    if !wet && !laid {
                        short += dist(a, b) / k as f64;
                    }
                }
            }
            if short > 40.0 {
                r.add("ring not closed", short, format!("{label}: {short:.0} ft of {ring:.0} missing"));
            }
        }
        // A building of its town the wall stands on (half its width, 4.5 ft, less a little).
        let town_l = town::layout(&w3, &t3, *ci);
        for b in &town_l.buildings {
            let m = b.poly.len();
            let hit = l.walls.iter().any(|w| {
                w.windows(2).any(|s| contains(&b.poly, s[0]) || (0..m).any(|i| seg_dist(b.poly[i], s[0], s[1]) < 4.0 || seg_dist(s[0], b.poly[i], b.poly[(i + 1) % m]) < 4.0 || segs_cross(s[0], s[1], b.poly[i], b.poly[(i + 1) % m])))
            });
            if hit {
                let on_bare = bare.get(label).is_some_and(|s| s.iter().any(|&(a, q)| b.poly.iter().any(|p| seg_dist(*p, a, q) < 8.0)));
                r.add(if on_bare { "building cut (a side with no street)" } else { "building cut by a hand wall" }, 0.0, format!("{label}: b:{ci}:{}", b.id));
            }
        }
    }
    // The towns' own walls toggled: each hand wall the same.
    let mut e4 = e3.clone();
    for &ci in &towns {
        let now = town::wards::plan_json(&w3, &t3, ci).is_ok_and(|p| p["walls"]["built"] == true);
        let w = World::new(WorldFile { edits: e3.clone(), ..base.file.clone() }).unwrap();
        if let Ok((Some(e), _)) = town::wards::change(&w, &t3, ci, &serde_json::from_str(&format!(r#"{{"walls":{}}}"#, !now)).unwrap()) {
            e4.towns.insert(town::wards::key(ci), e);
        }
    }
    let w4 = World::new(WorldFile { edits: e4, ..base.file.clone() }).unwrap();
    for (k, (label, ..)) in kept.iter().enumerate() {
        if look(&town::layout(&w4, &t3, first + k)) != looks[k] {
            r.add("hand wall changed by town walls on/off", 0.0, label.clone());
        }
    }
    let _ = toggle;
}

fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (d1, d2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (d3, d4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Gates and towers put in a town's own wall by hand (M18b, `edit_town walls: {…}`): per tier, up

/// A refusal's reason whole, for a tally: its ids and corner numbers as `…`/`N`, what it says in
/// brackets left out.
fn reason(why: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut depth = 0i32;
    for w in why.split_whitespace() {
        let open = w.starts_with('(');
        if open {
            depth += 1;
        }
        let shut = w.ends_with(')') || w.ends_with("),") || w.ends_with(").");
        if depth == 0 {
            let bare = w.trim_end_matches([',', '.', ':', ';']);
            let id = bare.split(':').count() == 3 && bare.split(':').skip(1).all(|x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()));
            let after_corner = out.last().is_some_and(|p| matches!(p.as_str(), "corner" | "corners" | "and")) && bare.bytes().all(|c| c.is_ascii_digit());
            out.push(if id { format!("{}:…", bare.split(':').next().unwrap()) } else if after_corner { "N".into() } else { w.to_string() });
        }
        if shut && depth > 0 {
            depth -= 1;
        }
    }
    out.join(" ").trim_end_matches([':', ',']).to_string()
}
/// to 3 walled towns each get 2 gates and 3 towers, then lose one generated gate and one tower;
/// one generated gate renamed first. Rules (each 0): a hand gate with no way through (no track
/// from it, or one over a building), a hand gate not in the group's gatehouse form (a bent one may
/// fall back to a square tower, tallied), a hand tower over a building, street or another tower,
/// other `t:`/`gt:` ids moved or renames lost, walls off then on losing them.
fn wallgates(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::t0::settle::Tier;
    use worldgen::town::gatehouse;
    use worldgen::town::wallgates::{GATE_TOWER_ID, HAND_TOWER_ID, OTHER_TOWER_ID};
    const RULES: [&str; 8] = [
        "building by a gate taken out nearer the wall than the town's",
        "hand gate with no way through",
        "hand gate's track along the wall",
        "hand gate not in the group's form",
        "hand tower over a building, street or tower",
        "other ids moved",
        "renames lost",
        "walls off/on losing them",
    ];
    const FORMS: [&str; 5] = ["square tower", "twin drums", "arched", "bent", "log"];
    r.counts(&RULES);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let n = t0.settlements.len();
    let ask = |w: &World, ci: usize, j: &str| town::wards::change(w, t0, ci, &serde_json::from_str(j).unwrap());
    let with = |w: &World, ci: usize, e: Option<worldgen::world::TownEdit>| {
        let mut f = w.file.clone();
        match e {
            Some(e) => f.edits.towns.insert(town::wards::key(ci), e),
            None => f.edits.towns.remove(&town::wards::key(ci)),
        };
        World::new(f).unwrap()
    };
    let label_of = |tn: &str, ci: usize, v: u64| format!("{tn} #{ci} gate {v}");
    let added_pts = |g: &[(String, P, String)], added: &[usize]| -> Vec<P> { g.iter().filter(|x| added.iter().any(|v| x.0.ends_with(&format!(":{v}")))).map(|x| x.1).collect() };
    let towers_of = |l: &town::Layout| -> Vec<(usize, P, bool)> { worldgen::interior::towers(l).into_iter().enumerate().map(|(k, t)| (l.tower_id(k), t.0, t.2)).collect() };
    let gates_of = |w: &World, l: &town::Layout| -> Vec<(String, P, String)> { town::places::of(w, t0, l).into_iter().filter(|p| p.kind == "gate").map(|p| (p.id, p.at, p.name)).collect() };
    for tier in [Tier::Town, Tier::City, Tier::Metropolis] {
        let towns: Vec<usize> = (0..n).filter(|&i| t0.settlements[i].tier == tier && town::wards::plan_json(base, t0, i).is_ok_and(|p| p["walls"]["built"] == true)).take(3).collect();
        let tn = format!("{tier:?}");
        for ci in towns {
            let plan = town::wards::plan_json(base, t0, ci).unwrap();
            let corners = plan["corners"].as_array().unwrap();
            let wall: Vec<(usize, P, bool)> = corners.iter().filter(|c| c["wall"] == true).map(|c| (c["corner"].as_u64().unwrap() as usize, [c["at"][0].as_f64().unwrap(), c["at"][1].as_f64().unwrap()], c["gate"] == true)).collect();
            let gen_gates: Vec<usize> = wall.iter().filter(|c| c.2).map(|c| c.0).collect();
            if gen_gates.len() < 2 {
                r.tally(&format!("{tn}: towns with under 2 gates (skipped)"), 1);
                continue;
            }
            // A generated gate renamed (kept through everything).
            let mut f = base.file.clone();
            let kept = gen_gates[1];
            f.edits.renames.insert(format!("gt:{ci}:{kept}"), "Basilisk Gate".into());
            let mut w = World::new(f).unwrap();
            let l0 = town::layout(&w, t0, ci);
            let (t_before, g_before) = (towers_of(&l0), gates_of(&w, &l0));
            // Two gates in, tried corner by corner, spread round the wall.
            let mut added: Vec<usize> = Vec::new();
            let step = (wall.len() / 10).max(1);
            for k in (0..wall.len()).step_by(step).chain(0..wall.len()) {
                if added.len() >= 2 {
                    break;
                }
                let v = wall[k].0;
                if wall[k].2 || added.contains(&v) {
                    continue;
                }
                let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"gates_add":[{v}]}}}}"#)) else { continue };
                for x in rep["wall"]["refused"].as_array().into_iter().flatten() {
                    let why = x["why"].as_str().unwrap_or("");
                    r.tally(&format!("gates refused: {}", reason(why)), 1);
                }
                if rep["wall"]["gates_added"].as_array().is_some_and(|a| !a.is_empty()) {
                    added.push(v);
                    w = with(&w, ci, e);
                }
            }
            r.tally(&format!("{tn}: hand gates added"), added.len());
            // (How far the wall bends at each gate: generated and by hand.)
            let at_of = |v: u64| corners.iter().find(|c| c["corner"].as_u64() == Some(v)).map(|c| [c["at"][0].as_f64().unwrap(), c["at"][1].as_f64().unwrap()]);
            for c in corners.iter().filter(|c| c["wall"] == true) {
                let v = c["corner"].as_u64().unwrap();
                let hand = added.contains(&(v as usize));
                if !(hand || c["gate"] == true) {
                    continue;
                }
                let nb = &c["wall_next"];
                let (Some(a), Some(p), Some(b)) = (nb[0].as_u64().and_then(at_of), at_of(v), nb[1].as_u64().and_then(at_of)) else { continue };
                let (u, w) = (sub(a, p), sub(b, p));
                let cos = (u[0] * w[0] + u[1] * w[1]) / (dist(a, p) * dist(b, p)).max(1e-9);
                let bend = 180.0 - cos.clamp(-1.0, 1.0).acos().to_degrees();
                let who = if hand { "hand" } else { "generated" };
                r.measure(&format!("{tn}: wall's bend at a {who} gate (deg), most"), false, bend, label_of(&tn, ci, v));
                if bend > 20.0 {
                    r.tally(&format!("{tn}: {who} gates on a bend over 20 deg"), 1);
                }
                r.tally(&format!("{tn}: {who} gates looked at for bends"), 1);
            }
            // Three towers along the longest sides that take one.
            let mut sides: Vec<(f64, usize, usize)> = Vec::new();
            for (chain_k, c) in wall.iter().enumerate() {
                let next = &wall[(chain_k + 1) % wall.len()];
                let d = dist(c.1, next.1);
                sides.push((d, c.0, next.0));
            }
            sides.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut towers_added = 0;
            for &(_, a, b) in &sides {
                if towers_added >= 3 {
                    break;
                }
                let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"towers_add":[[{a},{b},0.5]]}}}}"#)) else { continue };
                for x in rep["wall"]["refused"].as_array().into_iter().flatten() {
                    let why = x["why"].as_str().unwrap_or("");
                    r.tally(&format!("towers refused: {}", reason(why)), 1);
                }
                if rep["wall"]["towers_added"].as_array().is_some_and(|a| !a.is_empty()) {
                    towers_added += 1;
                    w = with(&w, ci, e);
                }
            }
            r.tally(&format!("{tn}: hand towers added"), towers_added);
            // One generated gate out, one generated tower down.
            let gone_gate = gen_gates[0];
            let mut gate_out = false;
            if let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"gates_remove":[{gone_gate}]}}}}"#)) {
                if rep["wall"]["gates_removed"].as_array().is_some_and(|a| !a.is_empty()) {
                    gate_out = true;
                    w = with(&w, ci, e);
                }
            }
            let gone_tower = t_before.iter().find(|t| !t.2 && t.0 < HAND_TOWER_ID as usize && t_before.iter().all(|g| !g.2 || dist(g.1, t.1) > 80.0)).map(|t| t.0);
            if let Some(id) = gone_tower
                && let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"towers_remove":["t:{ci}:{id}"]}}}}"#))
                && rep["wall"]["towers_removed"].as_array().is_some_and(|a| !a.is_empty())
            {
                w = with(&w, ci, e);
            }
            let l = town::layout(&w, t0, ci);
            let (t_after, g_after) = (towers_of(&l), gates_of(&w, &l));
            let label = |what: &str| format!("{tn} #{ci} {what}");
            // The gate taken out: within 150 ft of it, no building nearer the wall than the town's
            // buildings kept there with the gate (its least), nor on the ground the gate kept clear
            // (40 ft round it; `clear_of`). (Castles aside: their own curtains.)
            if let Some(gone) = wall.iter().find(|c| c.0 == gone_gate).map(|c| c.1)
                && gate_out
            {
                let wall_of = |l: &town::Layout, poly: &[P]| l.walls.iter().flat_map(|w| w.windows(2)).map(|t| seg_poly(t[0], t[1], poly)).fold(f64::MAX, f64::min);
                let near = |b: &&town::Building| b.ward != Ward::Castle && b.poly.iter().any(|p| dist(*p, gone) < 150.0);
                let keep = l0.buildings.iter().filter(near).map(|b| wall_of(&l0, &b.poly)).fold(f64::MAX, f64::min);
                if keep < f64::MAX {
                    r.measure("wall clearance by a generated gate (ft, its buildings within 150 ft), least", true, keep, label(&format!("gate {gone_gate}")));
                }
                for b in l.buildings.iter().filter(near) {
                    let d = wall_of(&l, &b.poly);
                    let held = b.poly.iter().any(|p| dist(*p, gone) < 40.0) || contains(&b.poly, gone);
                    r.measure("building by a gate taken out: off the wall (ft), least", true, d, label(&format!("gate {gone_gate} b:{ci}:{}", b.id)));
                    if d < keep - 0.5 || held {
                        let why = if held { "on the gate's ground".to_string() } else { format!("{d:.0} ft off the wall, the town's {keep:.0}") };
                        r.add("building by a gate taken out nearer the wall than the town's", 1.0, label(&format!("gate {gone_gate} b:{ci}:{}: {why}", b.id)));
                    }
                }
            }
            // Hand gates: a way through, the group's form.
            let houses = gatehouse::houses(&l);
            for &v in &added {
                let Some((_, gp, _)) = g_after.iter().find(|g| g.0 == format!("gt:{ci}:{v}")) else {
                    r.add("hand gate with no way through", 1.0, label(&format!("gate {v} not built")));
                    continue;
                };
                let way = l.roads.iter().find(|x| x.1 == 2 && x.0.first().is_some_and(|p| dist(*p, *gp) < 2.0));
                match way {
                    None => r.add("hand gate with no way through", 1.0, label(&format!("gate {v}: no track out"))),
                    Some((pts, _, wd)) => {
                        r.measure("way out (ft), longest", false, pts.windows(2).map(|s| dist(s[0], s[1])).sum(), label(&format!("gate {v}")));
                        let over = l.buildings.iter().find(|b| {
                            let m = b.poly.len();
                            pts.windows(2).skip(1).any(|s| b.poly.iter().any(|q| seg_dist(*q, s[0], s[1]) < 0.5 * wd - 1.0) || (0..m).any(|i| seg_dist(lerp(s[0], s[1], 0.5), b.poly[i], b.poly[(i + 1) % m]) < 0.5 * wd - 1.0))
                        });
                        if let Some(b) = over {
                            r.add("hand gate with no way through", 1.0, label(&format!("gate {v}: track over b:{ci}:{}", b.id)));
                        }
                        // (Past its first 70 ft straight out: its edge 25 ft or more off the wall's
                        // outer face, but where it joins a road already there.)
                        let half_wall = 0.5 * l.wall_style.thickness();
                        let mut run = 0.0;
                        let mut worst: Option<(f64, P)> = None;
                        for sg in pts.windows(2) {
                            let d = dist(sg[0], sg[1]);
                            let steps = (d / 4.0).ceil().max(1.0) as usize;
                            for j in 0..steps {
                                let q = lerp(sg[0], sg[1], j as f64 / steps as f64);
                                let at = run + d * j as f64 / steps as f64;
                                if at < 72.0 {
                                    continue;
                                }
                                let by_road = l.roads.iter().filter(|x| !std::ptr::eq(&x.0, pts)).any(|x| x.0.windows(2).any(|t| seg_dist(q, t[0], t[1]) < 0.5 * x.2 + 0.5 * wd + 4.0));
                                let gap = l.walls.iter().flat_map(|w| w.windows(2)).map(|t| seg_dist(q, t[0], t[1])).fold(f64::MAX, f64::min) - half_wall - 0.5 * wd;
                                if !by_road && gap < 25.0 && worst.is_none_or(|w| gap < w.0) {
                                    worst = Some((gap, q));
                                }
                            }
                            run += d;
                        }
                        if let Some((gap, q)) = worst {
                            r.add("hand gate's track along the wall", 1.0, label(&format!("gate {v}: {gap:.0} ft off the wall at ({:.0}, {:.0})", q[0], q[1])));
                        }
                    }
                }
                match houses.iter().find(|h| dist(lerp(h.a, h.b, 0.5), *gp) < 20.0) {
                    None => r.add("hand gate not in the group's form", 1.0, label(&format!("gate {v}: no gatehouse"))),
                    Some(h) => {
                        r.measure("hand gate towers apart (ft), least", true, dist(h.a, h.b), label(&format!("gate {v}")));
                        r.tally(&format!("{tn}: hand gatehouses {}", FORMS[h.form as usize]), 1);
                        let want = l.wall_style.gatehouse;
                        if h.form != want {
                            if FORMS[want as usize] == "bent" && FORMS[h.form as usize] == "square tower" {
                                r.tally("bent hand gates fallen back to a square tower", 1);
                            } else {
                                r.add("hand gate not in the group's form", 1.0, label(&format!("gate {v}: {} not {}", FORMS[h.form as usize], FORMS[want as usize])));
                            }
                        }
                    }
                }
            }
            for h in houses.iter().filter(|h| !added_pts(&g_after, &added).iter().any(|p| dist(lerp(h.a, h.b, 0.5), *p) < 20.0)) {
                r.measure("generated gate towers apart (ft), least", true, dist(h.a, h.b), label("generated gate"));
            }
            // Hand towers: clear of buildings, streets crossing the wall and other towers.
            let all = worldgen::interior::towers(&l);
            for (k, &(id, p, _)) in t_after.iter().enumerate() {
                if !(HAND_TOWER_ID as usize..GATE_TOWER_ID as usize).contains(&id) {
                    continue;
                }
                let r0 = all[k].1;
                let over_b = l.buildings.iter().find(|b| {
                    let m = b.poly.len();
                    (0..m).any(|i| seg_dist(p, b.poly[i], b.poly[(i + 1) % m]) < r0 - 1.0)
                });
                let over_t = t_after.iter().enumerate().find(|(j, t)| *j != k && dist(t.1, p) < r0 + all[*j].1);
                let over_s = l.roads.iter().filter(|x| x.1 != 3).find(|x| x.0.windows(2).any(|s| seg_dist(p, s[0], s[1]) < r0 + 0.5 * x.2 - 2.0));
                if let Some(b) = over_b {
                    r.add("hand tower over a building, street or tower", 1.0, label(&format!("t:{ci}:{id} over b:{ci}:{}", b.id)));
                } else if let Some((_, t)) = over_t {
                    r.add("hand tower over a building, street or tower", 1.0, label(&format!("t:{ci}:{id} over t:{ci}:{} ({:.0} ft apart; it stood {:.0} ft off before)", t.0, dist(t.1, p), t_before.iter().find(|b| b.0 == t.0).map_or(-1.0, |b| dist(b.1, t.1)))));
                } else if over_s.is_some() {
                    r.add("hand tower over a building, street or tower", 1.0, label(&format!("t:{ci}:{id} over a road class {}", over_s.unwrap().1)));
                }
            }
            // Ids: every tower but those the edits took, where it stood; every gate but the one taken out.
            let added_at: Vec<P> = added.iter().filter_map(|v| wall.iter().find(|c| c.0 == *v).map(|c| c.1)).collect();
            let gone_at = wall.iter().find(|c| c.0 == gone_gate).map(|c| c.1).unwrap_or([0.0; 2]);
            // (Moved: along the wall's run; a curtain bowed anew may carry a tower out from it.)
            let runs = &l.hand_wall.runs;
            let along = |a: P, b: P| match (town::wallgates::run_place(runs, a), town::wallgates::run_place(runs, b)) {
                (Some(x), Some(y)) if x.0 == y.0 => (x.1 - y.1).abs(),
                _ => dist(a, b),
            };
            for &(id, p, gate) in &t_before {
                let meant = Some(id) == gone_tower || added_at.iter().any(|a| dist(*a, p) < 90.0) || (gate && dist(gone_at, p) < 40.0);
                match t_after.iter().find(|t| t.0 == id) {
                    Some(t) if along(t.1, p) > 60.0 => r.add("other ids moved", 1.0, label(&format!("t:{ci}:{id} moved {:.0} ft along the wall", along(t.1, p)))),
                    Some(t) if dist(t.1, p) > 1.0 => {
                        r.tally("towers shifted (their curtain bowed anew)", 1);
                        r.measure("tower shifted by a curtain bowed anew (ft), most", false, dist(t.1, p), label(&format!("t:{ci}:{id}")));
                    }
                    None if !meant => r.add("other ids moved", 1.0, label(&format!("t:{ci}:{id} gone (gate tower {gate}, {:.0} ft from the gate out; nearest now {:?})", dist(gone_at, p), t_after.iter().map(|t| (t.0, dist(t.1, p).round(), t.2)).min_by(|a, b| a.1.total_cmp(&b.1))))),
                    _ => {}
                }
            }
            for (id, p, _) in &g_before {
                if *id == format!("gt:{ci}:{gone_gate}") {
                    continue;
                }
                match g_after.iter().find(|g| g.0 == *id) {
                    Some(g) if dist(g.1, *p) > 1.0 => r.add("other ids moved", 1.0, label(&format!("{id} moved"))),
                    None => r.add("other ids moved", 1.0, label(&format!("{id} gone"))),
                    _ => {}
                }
            }
            if t_after.iter().any(|t| t.0 >= OTHER_TOWER_ID as usize) {
                r.add("other ids moved", 1.0, label(&format!("{} towers matched none", t_after.iter().filter(|t| t.0 >= OTHER_TOWER_ID as usize).count())));
            }
            if !g_after.iter().any(|g| g.0 == format!("gt:{ci}:{kept}")) || w.file.edits.renames.get(&format!("gt:{ci}:{kept}")).is_none_or(|n| n != "Basilisk Gate") {
                r.add("renames lost", 1.0, label(&format!("gt:{ci}:{kept}")));
            }
            r.tally(&format!("{tn}: towers before"), t_before.len());
            r.tally(&format!("{tn}: towers after"), t_after.len());
            // Walls off, then on: the same towers and gates.
            let w2 = ask(&w, ci, r#"{"walls":false}"#).map(|(e, _)| with(&w, ci, e)).ok();
            let w3 = w2.and_then(|w2| ask(&w2, ci, r#"{"walls":true}"#).map(|(e, _)| with(&w2, ci, e)).ok());
            match w3 {
                None => r.add("walls off/on losing them", 1.0, label("toggle refused")),
                Some(w3) => {
                    let l3 = town::layout(&w3, t0, ci);
                    let (t3, g3) = (towers_of(&l3), gates_of(&w3, &l3));
                    let ids = |t: &[(usize, P, bool)]| t.iter().map(|x| x.0).collect::<Vec<_>>();
                    let gids = |g: &[(String, P, String)]| g.iter().map(|x| (x.0.clone(), x.2.clone())).collect::<Vec<_>>();
                    if ids(&t3) != ids(&t_after) || gids(&g3) != gids(&g_after) {
                        r.add("walls off/on losing them", 1.0, label(&format!("towers {} -> {}, gates {} -> {}", t_after.len(), t3.len(), g_after.len(), g3.len())));
                    }
                }
            }
        }
    }
}

// R5 towns
/// Castle pieces (M6): a castle drawn by hand (`create_feature castle`, 400 x 300 ft) beside a
/// site drawn before it; its keep made a library and renamed, a yard building given a new trade,
/// one taken away, one given storeys; then the earlier site deleted, then taken out of the list
/// (the castle's layout number moves): the edits stay on the same castle. The same castle in
/// ruins: the keep's function and name set, storeys refused. A castle's keep in a generated town
/// takes `update_building` as before. The rules (each should be 0) are the edits not standing.
fn castle_pieces(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::agent::{self, BuildingChange};
    use worldgen::town::pieces;
    use worldgen::world::{Created, WorldFile};
    const RULES: [&str; 8] = [
        "piece edit refused",
        "keep's function or name not as edited",
        "yard building's edit not standing",
        "other pieces changed by the edits",
        "edits not on the same castle after an earlier site went",
        "ruined keep took storeys or lost its function or name",
        "generated castle's keep refused or not changed",
        "id not told apart (wrong refusal)",
    ];
    r.counts(&RULES);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let n = t0.settlements.len();
    let first = n + t0.base_pois;
    let key = |b: &town::Building| b.func.map_or("-", |f| town::catalog::CATALOG[f as usize].key);
    // A spot for the castle: off a town, clear of buildings, roads and water.
    let mut spot: Option<Created> = None;
    'find: for s in t0.settlements.iter().take(40) {
        for j in 0..8 {
            let a = j as f64 * std::f64::consts::FRAC_PI_4;
            let at = [s.x + 2600.0 * a.cos(), s.y + 2600.0 * a.sin()];
            let poly = vec![[at[0] - 200.0, at[1] - 150.0], [at[0] + 200.0, at[1] - 150.0], [at[0] + 200.0, at[1] + 150.0], [at[0] - 200.0, at[1] + 150.0]];
            let c = Created { id: "c:1".into(), kind: "castle".into(), x: at[0], y: at[1], name: "Candlekeep".into(), poly, ..Default::default() };
            if c.check().is_ok() && agent::works_spot(base, t0, &c, "c:1", None).is_ok_and(|w| w.in_way.is_empty()) {
                spot = Some(c);
                break 'find;
            }
        }
    }
    let Some(castle) = spot else {
        r.add("piece edit refused", 0.0, "no spot for a castle".into());
        return;
    };
    let tower = Created { id: "c:0".into(), kind: "tower".into(), x: castle.x + 3000.0, y: castle.y, name: "Earlier Tower".into(), ..Default::default() };
    let world_of = |created: Vec<Created>| {
        let mut e = base.file.edits.clone();
        e.created = created;
        let w = World::new(WorldFile { edits: e, ..base.file.clone() }).unwrap();
        let mut t = T0::from_bytes(&t0.to_bytes()).unwrap();
        t.apply_edits(&w);
        town::forget_from(first);
        (w, t)
    };
    for ruin in [false, true] {
        let label = if ruin { "ruined castle" } else { "castle" };
        let mut c = castle.clone();
        c.structure = ruin.then(|| "ruin".to_string());
        let (w1, t1) = world_of(vec![tower.clone(), c.clone()]);
        let li = first + 1;
        let l1 = town::layout(&w1, &t1, li);
        let Some(ki) = pieces::keep_index(&l1) else {
            r.add("piece edit refused", 0.0, format!("{label}: no keep"));
            continue;
        };
        let yard: Vec<u32> = l1.buildings.iter().enumerate().filter(|(i, _)| *i != ki).map(|(_, b)| b.id).collect();
        r.tally(&format!("{label}s: buildings"), l1.buildings.len());
        let kid = format!("b:{li}:{}", l1.buildings[ki].id);
        let change = |func: Option<&str>, floors: Option<u8>| BuildingChange { func: func.map(str::to_string), floors, ..Default::default() };
        // (keep: a library, renamed; yard 0: a temple; yard 1: taken away; yard 2: 3 storeys.)
        let mut asks: Vec<(String, BuildingChange, Option<&str>, bool)> = vec![(kid.clone(), change(Some("library"), None), Some("The Great Library"), false)];
        if let Some(&y) = yard.first() {
            asks.push((format!("b:{li}:{y}"), change(Some("temple"), None), None, false));
        }
        if let Some(&y) = yard.get(1) {
            asks.push((format!("b:{li}:{y}"), BuildingChange::default(), None, true));
        }
        if let Some(&y) = yard.get(2).filter(|_| !ruin) {
            asks.push((format!("b:{li}:{y}"), change(None, Some(3)), None, false));
        }
        for (id, ch, name, remove) in &asks {
            let Some(p) = pieces::piece_of(&w1, &t1, id) else {
                r.add("piece edit refused", 0.0, format!("{label} {id}: not a piece ({})", pieces::not_generated(&w1, &t1, id)));
                continue;
            };
            let e = if *remove { pieces::piece_removal(&p).map(Some) } else { pieces::piece_edit(&w1, &p, ch, *name) };
            match e {
                Ok(Some(e)) => drop(c.pieces.insert(p.key.clone(), e)),
                Ok(None) => r.add("piece edit refused", 0.0, format!("{label} {id}: no change")),
                Err(e) => r.add("piece edit refused", 0.0, format!("{label} {id}: {e}")),
            }
        }
        // (A ruin's keep takes no storeys; the keep can't be taken away.)
        if let Some(p) = pieces::piece_of(&w1, &t1, &kid) {
            if ruin != pieces::piece_edit(&w1, &p, &change(None, Some(4)), None).is_err() {
                r.add("ruined keep took storeys or lost its function or name", 0.0, format!("{label}: storeys on the keep {}", if ruin { "taken" } else { "refused" }));
            }
            if pieces::piece_removal(&p).is_ok() {
                r.add("piece edit refused", 0.0, format!("{label}: the keep was taken away"));
            }
        }
        let names: Vec<String> = l1.buildings.iter().map(|b| format!("{:?}", (b.id, &b.poly, b.func, &b.name, b.floors, b.structure))).collect();
        let edited = |l: &Layout, li: usize, when: &str, r: &mut Report| {
            let rule = if when.is_empty() { "" } else { "edits not on the same castle after an earlier site went" };
            let k = pieces::keep_index(l).map(|i| &l.buildings[i]);
            if !k.is_some_and(|b| key(b) == "library" && b.name.as_deref() == Some("The Great Library") && (b.structure == town::Structure::Ruin) == ruin) {
                r.add(if rule.is_empty() { "keep's function or name not as edited" } else { rule }, 0.0, format!("{label}{when}: keep {:?}", k.map(|b| (key(b), &b.name, b.structure))));
            }
            if ruin && k.is_some_and(|b| b.floors != l1.buildings[ki].floors) {
                r.add("ruined keep took storeys or lost its function or name", 0.0, format!("{label}{when}: keep storeys {:?}", k.map(|b| b.floors)));
            }
            let b = |y: Option<&u32>| y.and_then(|&y| l.building_index(y as usize)).map(|i| &l.buildings[i]);
            let bad_yard = (yard.first().is_some() && !b(yard.first()).is_some_and(|b| key(b) == "temple"))
                || (yard.get(1).is_some() && b(yard.get(1)).is_some())
                || (!ruin && yard.get(2).is_some() && !b(yard.get(2)).is_some_and(|b| b.floors == 3));
            if bad_yard {
                r.add(if rule.is_empty() { "yard building's edit not standing" } else { rule }, 0.0, format!("{label}{when} (L{li})"));
            }
            for (o, was) in l1.buildings.iter().zip(&names) {
                if o.id as usize == ki || yard.iter().take(3).any(|y| *y == o.id) {
                    continue;
                }
                let now = l.building_index(o.id as usize).map(|i| &l.buildings[i]);
                if now.is_none_or(|b| format!("{:?}", (b.id, &b.poly, b.func, &b.name, b.floors, b.structure)) != *was) {
                    r.add("other pieces changed by the edits", 0.0, format!("{label}{when}: b:{li}:{}", o.id));
                }
            }
            r.tally(&format!("{label}s: pieces checked"), 1);
        };
        let (w2, t2) = world_of(vec![tower.clone(), c.clone()]);
        edited(&town::layout(&w2, &t2, li), li, "", r);
        let gone = Created { removed: true, ..tower.clone() };
        let (w3, t3) = world_of(vec![gone, c.clone()]);
        edited(&town::layout(&w3, &t3, li), li, " (tower deleted)", r);
        // (The earlier site out of the list: the castle is laid out one place earlier.)
        let (w4, t4) = world_of(vec![c.clone()]);
        let l4 = town::layout(&w4, &t4, first);
        edited(&l4, first, " (tower out of the list)", r);
        let k4 = format!("b:{first}:{}", l1.buildings[ki].id);
        if !pieces::piece_of(&w4, &t4, &k4).is_some_and(|p| p.keep() && p.castle == "c:1") {
            r.add("edits not on the same castle after an earlier site went", 0.0, format!("{label}: {k4} not the keep of c:1"));
        }
    }
    // Ids told apart: the tower's, a hand castle's missing building, no such place.
    let (w, t) = world_of(vec![tower.clone(), castle.clone()]);
    for (id, want) in [(format!("b:{first}:0"), "a tower made by hand"), (format!("b:{}:999", first + 1), "has no building 999"), (format!("b:{}:0", town::layout_count(&t) + 5), "there is no place"), ("c:1".to_string(), "a site made by hand")] {
        let got = agent::building_edit(&w, &t, &id, &BuildingChange::default()).err().unwrap_or_default();
        if !got.contains(want) {
            r.add("id not told apart (wrong refusal)", 0.0, format!("{id}: {got}"));
        }
    }
    town::forget_from(first);
    // A generated castle's keep (in a town, else a village or a site) takes update_building.
    let mut tried = 0;
    let mut per: HashMap<&str, usize> = HashMap::new();
    for li in 0..first {
        let tier = if li < n { t0.settlements[li].tier.name() } else { "site" };
        let l = town::layout(base, t0, li);
        let Some(ki) = pieces::keep_index(&l) else { continue };
        *per.entry(tier).or_default() += 1;
        if per[tier] > 2 {
            continue;
        }
        let id = format!("b:{li}:{}", l.buildings[ki].id);
        tried += 1;
        let ok = agent::building_edit(base, t0, &id, &BuildingChange { func: Some("library".into()), ..Default::default() }).and_then(|e| {
            let mut ed = base.file.edits.clone();
            ed.buildings.insert(id.clone(), e.ok_or("no change")?);
            let w = World::new(WorldFile { edits: ed, ..base.file.clone() }).unwrap();
            let l = town::layout(&w, t0, li);
            let now = l.building_index(l.buildings[ki].id as usize).map(|i| key(&l.buildings[i]));
            if now == Some("library") { Ok(()) } else { Err(format!("now {now:?}")) }
        });
        if let Err(e) = ok {
            r.add("generated castle's keep refused or not changed", 0.0, format!("{id} ({}): {e}", if li < n { t0.settlements[li].tier.name() } else { "site" }));
        }
    }
    r.tally("generated castles' keeps changed", tried);
    for (tier, k) in per {
        r.tally(&format!("generated keeps: in a {tier}"), k);
    }
    town::forget_from(0);
}

/// R5 towns (114a): a steppe town's roofs (every steppe town and city, not a site): by material and
/// shape, and the rule (0): clay tile or shingle on a home, a shop or a workshop.
fn steppe_roofs(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["steppe towns", "roofed buildings", "clay tile or shingle on homes, shops, workshops"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town || l.culture % 5 != 4 {
            continue;
        }
        r.add("steppe towns", 0.0, format!("seed {} layout {li} ({:?})", world.file.seed, l.tier));
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let k = town::roof::look(&l, b);
            let (m, s) = (town::roof::MATERIALS[k.material as usize], town::roof::SHAPES[k.shape as usize]);
            r.tally("roofed buildings", 1);
            r.tally(&format!("  roofs {m} {s}"), 1);
            let f = b.func.map(|f| &town::catalog::CATALOG[f as usize]);
            let grand = f.is_some_and(|f| matches!(f.category, "faith & death" | "civic & military" | "guilds & underworld") || matches!(f.key, "library" | "bank" | "auction_house" | "theater" | "arena" | "bathhouse" | "bards_college" | "observatory" | "arcane_academy"));
            if !grand && matches!(m, "clay_tile" | "shingle") {
                r.add("clay tile or shingle on homes, shops, workshops", 0.0, format!("seed {} b:{li}:{} {}", world.file.seed, b.id, f.map_or("home", |f| f.key)));
            }
            if grand {
                r.tally(&format!("  grand {m} {s}"), 1);
            }
        }
    }
}

/// R5 towns (114a): steppe towns' round felt halls (`town::roof::hall_ring`: the town hall,
/// temples, inns): built and fallen back to rectangles (why), their sizes; the rules (each 0): a
/// hall over another building, a road or a wall; a hall's interior with squares cut off from its
/// door or missing a piece its use needs. Doors facing a street within reach (as a yurt's); the
/// roofs (`steppe_roofs`).
fn steppe(ctx: &mut Ctx, r: &mut Report) {
    use town::roof::{FeltUse, HallRefused};
    steppe_roofs(ctx, r);
    steppe_gaps(ctx, r);
    r.counts(&["felt halls built", "  halls", "  temples", "  inns", "fell back: lot too small", "fell back: a castle's or a ruin"]);
    r.counts(&["halls over another building", "halls over a road", "halls over a wall", "doors within 30 degrees of a street or square", "doors toward the centre (no street in reach)", "doors off their street"]);
    r.counts(&["interiors with squares cut off from the door", "interiors missing a piece", "interiors not one room on one storey", "interiors not built again from their design"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut listed = 0;
    let cross = |a: P, b: P, c: P, d: P| {
        let o = |p: P, q: P, s: P| (q[0] - p[0]) * (s[1] - p[1]) - (q[1] - p[1]) * (s[0] - p[0]);
        o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
    };
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town || l.culture % 5 != 4 {
            continue;
        }
        for b in l.buildings.iter() {
            let Some(key) = b.func.map(|f| town::catalog::CATALOG[f as usize].key) else { continue };
            let Some(u) = FeltUse::of_key(key) else { continue };
            let id = format!("b:{li}:{}", b.id);
            let tag = format!("seed {} {id} ({})", world.file.seed, u.name());
            if town::roof::felt_built(b).is_none() {
                match town::roof::hall_refused(&l, b) {
                    Some(HallRefused::Small) => {
                        let o = town::geom::obb(&b.poly);
                        r.add("fell back: lot too small", 0.0, format!("{tag}: {:.0} x {:.0} ft", o.long, o.short));
                    }
                    Some(HallRefused::Pieces) => r.add("fell back: a castle's or a ruin", 0.0, tag.clone()),
                    None => {}
                }
                continue;
            }
            r.tally("felt halls built", 1);
            r.tally(["  halls", "  temples", "  inns"][u as usize], 1);
            let n = b.poly.len();
            let c = b.poly.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n as f64, a[1] + p[1] / n as f64]);
            let rad = libm::hypot(b.poly[0][0] - c[0], b.poly[0][1] - c[1]);
            r.value(&format!("{} across (ft)", u.name()), 2.0 * rad, tag.clone());
            // Over another building, a road, a wall.
            let edges = |p: &[P]| (0..p.len()).map(move |k| (p[k], p[(k + 1) % p.len()])).collect::<Vec<_>>();
            let mine = edges(&b.poly);
            let hit = l.buildings.iter().filter(|o| o.id != b.id && o.structure == town::Structure::Roofed && libm::hypot(o.poly[0][0] - c[0], o.poly[0][1] - c[1]) < 400.0).find(|o| {
                o.poly.iter().any(|p| town::geom::contains(&b.poly, *p))
                    || b.poly.iter().any(|p| town::geom::contains(&o.poly, *p))
                    || edges(&o.poly).iter().any(|e| mine.iter().any(|m| cross(m.0, m.1, e.0, e.1)))
            });
            if let Some(o) = hit {
                r.add("halls over another building", 0.0, format!("{tag}: b:{li}:{}", o.id));
            }
            let road = l.roads.iter().find(|(pts, _, w)| pts.windows(2).any(|s| mine.iter().any(|m| cross(m.0, m.1, s[0], s[1])) || b.poly.iter().any(|p| town::geom::seg_dist(*p, s[0], s[1]) < 0.5 * w - 0.5)));
            if let Some((_, class, w)) = road {
                r.add("halls over a road", 0.0, format!("{tag}: class {class}, {w:.0} ft"));
            }
            if l.walls.iter().any(|w| w.windows(2).any(|s| b.poly.iter().any(|p| town::geom::seg_dist(*p, s[0], s[1]) < 3.0) || mine.iter().any(|m| cross(m.0, m.1, s[0], s[1])))) {
                r.add("halls over a wall", 0.0, tag.clone());
            }
            // The door: its street (as `town::yurt_door` looks for it).
            let door = [0.5 * (b.poly[0][0] + b.poly[1][0]), 0.5 * (b.poly[0][1] + b.poly[1][1])];
            let ang = libm::atan2(door[1] - c[1], door[0] - c[0]).to_degrees();
            let reach = 70.0 + (rad - 15.0).max(0.0);
            let mut near: Option<(f64, P)> = None;
            for w in l.roads.iter().map(|w| &w.0).chain(l.streets.iter()).chain(l.plazas.iter().map(|p| &p.pts)) {
                for e in w.windows(2) {
                    let ab = [e[1][0] - e[0][0], e[1][1] - e[0][1]];
                    let t = (((c[0] - e[0][0]) * ab[0] + (c[1] - e[0][1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
                    let q = [e[0][0] + ab[0] * t, e[0][1] + ab[1] * t];
                    let d = libm::hypot(q[0] - c[0], q[1] - c[1]);
                    if d <= reach && near.is_none_or(|b| d < b.0) {
                        near = Some((d, q));
                    }
                }
            }
            let off = |a: f64, b: f64| ((a - b + 540.0) % 360.0 - 180.0).abs();
            match near {
                Some((_, q)) => {
                    let to = libm::atan2(q[1] - c[1], q[0] - c[0]).to_degrees();
                    if off(ang, to) <= 30.0 {
                        r.tally("doors within 30 degrees of a street or square", 1);
                    } else {
                        r.add("doors off their street", off(ang, to), format!("{tag}: {:.0} degrees off", off(ang, to)));
                    }
                }
                None => r.add("doors toward the centre (no street in reach)", 0.0, tag.clone()),
            }
            // Inside: one round room on one storey, every square not under a piece reached from the door.
            let Some(it) = interior::generate_id(world, t0, &id) else {
                r.add("interiors not one room on one storey", 0.0, format!("{tag}: none"));
                continue;
            };
            let room = ["felt hall", "felt sanctuary", "felt common room"][u as usize];
            let lv = &it.levels[it.entry_level.min(it.levels.len() - 1)];
            let Some(ri) = lv.rooms.iter().position(|q| q.kind == room).filter(|_| it.levels.len() == 1 && lv.rooms.iter().filter(|q| q.squares > 0).count() == 1) else {
                r.add("interiors not one room on one storey", 0.0, format!("{tag}: {} levels, rooms {:?}", it.levels.len(), lv.rooms.iter().map(|q| q.kind).collect::<Vec<_>>()));
                continue;
            };
            let (nx, ny) = (it.nx, it.ny);
            let mut blocked = vec![false; nx * ny];
            for f in lv.furniture.iter().filter(|f| f.blocks_move) {
                for j in f.y as usize..(f.y + f.h) as usize {
                    for i in f.x as usize..(f.x + f.w) as usize {
                        blocked[j * nx + i] = true;
                    }
                }
            }
            let walk = |k: usize| lv.cells[k] == ri as i16 && !blocked[k];
            let start = lv.doors.iter().find(|d| d.kind == "front").and_then(|d| {
                let (x, y) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize);
                let pair = if d.a[0] == d.b[0] { [(x - 1, y), (x, y)] } else { [(x, y - 1), (x, y)] };
                pair.into_iter().filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).find(|&k| walk(k))
            });
            let mut seen = vec![false; nx * ny];
            let mut stack: Vec<usize> = start.into_iter().collect();
            while let Some(k) = stack.pop() {
                if seen[k] {
                    continue;
                }
                seen[k] = true;
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny && walk(b * nx + a) && !seen[b * nx + a] {
                        stack.push(b * nx + a);
                    }
                }
            }
            let cut = (0..nx * ny).filter(|&k| walk(k) && !seen[k]).count();
            if cut > 0 || start.is_none() {
                r.add("interiors with squares cut off from the door", cut as f64, format!("{tag}: {cut} squares cut off{}", if start.is_none() { ", no way in" } else { "" }));
            }
            let needs: &[&str] = match u {
                FeltUse::Hall => &["yurt_hearth", "low_dais", "chair_of_office", "bench"],
                FeltUse::Temple => &["yurt_hearth", "altar", "brazier", "floor_cushion"],
                FeltUse::Inn => &["yurt_hearth", "counter", "table", "yurt_pallet"],
            };
            let missing: Vec<&str> = needs.iter().copied().filter(|k| !lv.furniture.iter().any(|f| f.kind == *k)).collect();
            if !missing.is_empty() {
                r.add("interiors missing a piece", 0.0, format!("{tag}: no {}", missing.join(", ")));
            }
            for f in &lv.furniture {
                r.tally(&format!("  {} pieces: {}", u.name(), f.kind), 1);
            }
            // (As a design builds it again: the same, and within the designer's rules.)
            if let Ok((d, _)) = interior::design::design_of(world, t0, &id, false) {
                let (built, problems) = interior::design::problems(world, t0, &id, &d).unwrap();
                let (x, y) = (serde_json::to_string(&built).unwrap(), serde_json::to_string(&it).unwrap());
                if x != y && std::env::var("DIFF").is_ok() {
                    let k = x.bytes().zip(y.bytes()).position(|(a, b)| a != b).unwrap_or(0);
                    eprintln!("{tag}: built {} / generated {}", &x[k.saturating_sub(150)..(k + 80).min(x.len())], &y[k.saturating_sub(150)..(k + 80).min(y.len())]);
                }
                if !problems.is_empty() || x != y {
                    r.add("interiors not built again from their design", 0.0, format!("{tag}: {problems:?}"));
                }
            }
            if listed < 12 || u == FeltUse::Hall {
                listed += 1;
                r.add("felt halls listed", 0.0, format!("{tag} at {:.0},{:.0}, {:.0} ft across, door {ang:.0}, {} pieces", c[0], c[1], 2.0 * rad, lv.furniture.len()));
            }
        }
    }
}

/// R5 towns (114-115 fix): steppe towns' buildings against their neighbours: footprints overlapping
/// another's (rule, 0), those within 1 ft of another building (against the base's count), and round
/// ones (a yurt, a felt hall, a shrine) within 2 ft of another (a felt hall or yurt keeps a yard).
fn steppe_gaps(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["footprints overlapping another building's", "buildings within 1 ft of another", "round buildings within 2 ft of another"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town || l.culture % 5 != 4 {
            continue;
        }
        let bs: Vec<&town::Building> = l.buildings.iter().filter(|b| b.poly.len() >= 3).collect();
        let boxes: Vec<[f64; 4]> = bs.iter().map(|b| bbox(&b.poly)).collect();
        for i in 0..bs.len() {
            let (mut over, mut near) = (None, None);
            for j in 0..bs.len() {
                let (a, b) = (boxes[i], boxes[j]);
                if i == j || a[0] > b[2] + 1.0 || b[0] > a[2] + 1.0 || a[1] > b[3] + 1.0 || b[1] > a[3] + 1.0 {
                    continue;
                }
                if poly_overlap(&shrink(&bs[i].poly, 0.15), &shrink(&bs[j].poly, 0.15)) {
                    over = Some(j);
                }
                let n = bs[j].poly.len();
                let d = (0..n).map(|k| seg_poly(bs[j].poly[k], bs[j].poly[(k + 1) % n], &bs[i].poly)).fold(f64::MAX, f64::min);
                if d < 2.0 && near.is_none_or(|(_, e)| d < e) {
                    near = Some((j, d));
                }
            }
            let round = bs[i].poly.len() == town::roof::ROUND_SIDES || bs[i].poly.len() == town::roof::HALL_SIDES;
            let tag = |j: usize| format!("seed {} b:{li}:{} ({} sides) by b:{li}:{} ({} sides)", world.file.seed, bs[i].id, bs[i].poly.len(), bs[j].id, bs[j].poly.len());
            if let Some(j) = over.filter(|&j| j > i) {
                r.add("footprints overlapping another building's", 0.0, tag(j));
            }
            if let Some((j, d)) = near {
                if d < 1.0 {
                    r.tally("buildings within 1 ft of another", 1);
                }
                if round {
                    let (c, w) = (centroid(&bs[i].poly), world.params().width_mi * 5280.0);
                    let h = world.params().height_mi * 5280.0;
                    r.add("round buildings within 2 ft of another", d, format!("{}: {d:.2} ft (fx {:.5} fy {:.5})", tag(j), c[0] / w, c[1] / h));
                }
            }
        }
    }
}

// R5 towns (114)
/// A prop's footprint (ft along its turn, across) as this check reads it, apart from the generator's.
fn horse_foot(kind: Kind, at: P, rot: f32, scale: f32) -> Option<[P; 4]> {
    let (l, w) = match kind {
        // (As drawn: its sprite 2 squares nose to tail, its middle a foot ahead of where it stands.)
        Kind::Horse => (9.8, 2.7),
        Kind::HitchingRail => (10.0, 1.2),
        Kind::WaterTrough => (6.8, 2.8),
        Kind::Haystack => (7.2, 7.2),
        Kind::Cart => (11.5, 5.5),
        _ => return None,
    };
    let (l, w) = (l * scale as f64, w * scale as f64);
    let u = [libm::cos(rot as f64), libm::sin(rot as f64)];
    let at = if kind == Kind::Horse { add(at, mul(u, scale as f64)) } else { at };
    let (a, b) = (mul(u, 0.5 * l), mul([-u[1], u[0]], 0.5 * w));
    Some([sub(sub(at, a), b), sub(add(at, a), b), add(add(at, a), b), add(sub(at, a), b)])
}

/// Whether two convex outlines overlap (separating axes; touching is not).
fn boxes_cross(a: &[P], b: &[P]) -> bool {
    for q in [a, b] {
        for k in 0..q.len() {
            let e = sub(q[(k + 1) % q.len()], q[k]);
            let ax = [-e[1], e[0]];
            let pr = |r: &[P]| r.iter().fold((f64::MAX, f64::MIN), |s, p| (s.0.min(dot(*p, ax)), s.1.max(dot(*p, ax))));
            let ((a0, a1), (b0, b1)) = (pr(a), pr(b));
            if a0 >= b1 || b0 >= a1 {
                return false;
            }
        }
    }
    true
}

fn seg_rect(a: P, b: P, w: f64) -> [P; 4] {
    let l = dist(a, b).max(1e-9);
    let u = mul(sub(b, a), 1.0 / l);
    let n = mul([-u[1], u[0]], 0.5 * w);
    [sub(a, n), sub(b, n), add(b, n), add(a, n)]
}

const HORSE_RULES: [&str; 9] = [
    "R Formal parks left in steppe towns",
    "R horse parks not by hand outside steppe towns",
    "R props overlapping props",
    "R props over a fence",
    "R props on a way in or in a gate",
    "R fences across a way in",
    "R horses outside a corral or the market ground",
    "R props outside the park",
    "R battlemap park squares beyond cover",
];

/// Horse parks (114): how many per steppe town, what they hold, the rules (each 0) on the plan; on the
/// battlemap, props and fence laid against planned and the ground; a park patch set to one by hand.
fn horseparks(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&HORSE_RULES);
    r.counts(&["parks with under 2 corrals", "battlemap: horse props planned", "battlemap: horse props laid", "hand horse parks refused", "hand horse parks not laid"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let size = world.geom.tile_size_ft(world.geom.max_level);
    let mut cams = 0;
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let mut look: Vec<(String, town::Park, worldgen::town::horses::Plan)> = Vec::new();
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town {
            continue;
        }
        let steppe = l.culture % 5 == 4;
        let tier = format!("{:?}", l.tier);
        if steppe {
            r.tally(&format!("steppe towns ({tier})"), 1);
        }
        for pk in &l.parks {
            let here = format!("L{li} at {}", at(centroid(&pk.pts)));
            if steppe && pk.kind == town::ParkKind::Formal {
                r.add(HORSE_RULES[0], 1.0, here.clone());
            }
            if pk.kind != town::ParkKind::Horses {
                if steppe {
                    r.tally(&format!("steppe parks kept: {}", pk.kind.name()), 1);
                }
                continue;
            }
            if !steppe {
                r.add(HORSE_RULES[1], 1.0, here.clone());
            }
            r.tally(&format!("horse parks ({tier})"), 1);
            let Some(pl) = town::horses::plan(pk) else { continue };
            check_horse_park(pk, &pl, &here, r);
            if look.len() < 10 {
                look.push((here.clone(), pk.clone(), pl.clone()));
            }
            if cams < 4 {
                let c = centroid(&pk.pts);
                cams += 1;
                say!("    camera {here}: fx {:.6} fy {:.6}", c[0] / mw, c[1] / mh);
            }
        }
    }
    // A park patch set to horses by hand (not a steppe town), walls as they are.
    let mut tried = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town && t0.settlements[i].culture % 5 != 4) {
        if tried >= 2 {
            break;
        }
        let Ok(plan) = town::wards::plan_json(world, t0, ci) else { continue };
        let pick = plan["patches"].as_array().unwrap().iter().find(|p| p["in_town"] == true && p.get("park_refused").is_none() && !matches!(p["ward"].as_str(), Some("plaza" | "castle" | "temple"))).and_then(|p| p["patch"].as_u64());
        let Some(p) = pick else { continue };
        tried += 1;
        let json = format!(r#"{{"patches":[{{"patch":{p},"ward":"park","park":"horses"}}]}}"#);
        let Ok((Some(edit), _)) = town::wards::change(world, t0, ci, &serde_json::from_str(&json).unwrap()) else {
            r.add("hand horse parks refused", 1.0, format!("#{ci} patch {p}"));
            continue;
        };
        let mut file = world.file.clone();
        file.edits.towns.insert(town::wards::key(ci), edit);
        let w2 = World::new(file).unwrap();
        let l = town::layout(&w2, t0, ci);
        let at_p = plan["patches"].as_array().unwrap().iter().find(|q| q["patch"].as_u64() == Some(p)).map(|q| [q["at"][0].as_f64().unwrap(), q["at"][1].as_f64().unwrap()]).unwrap();
        match l.parks.iter().find(|pk| pk.kind == town::ParkKind::Horses && dist(centroid(&pk.pts), at_p) < 200.0) {
            Some(pk) => {
                r.tally("hand horse parks", 1);
                let here = format!("hand #{ci} patch {p}");
                if let Some(pl) = town::horses::plan(pk) {
                    check_horse_park(pk, &pl, &here, r);
                }
                let c = centroid(&pk.pts);
                say!("    camera {here}: fx {:.6} fy {:.6}", c[0] / mw, c[1] / mh);
            }
            None => r.add("hand horse parks not laid", 1.0, format!("#{ci} patch {p}")),
        }
    }
    for (here, pk, pl) in &look {
        let b = pk.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let (mut laid, mut fence, mut dirt, mut squares, mut far) = (0usize, 0.0f64, 0usize, 0usize, 0usize);
        for ky in k(b[1])..=k(b[3]) {
            for kx in k(b[0])..=k(b[2]) {
                let c = ctx.chunk(kx, ky);
                let (ox, oy) = (kx as f64 * size, ky as f64 * size);
                for o in &c.objects {
                    let p = [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
                    if contains(&pk.pts, p) && matches!(o.kind, Kind::Horse | Kind::HitchingRail | Kind::WaterTrough | Kind::Haystack | Kind::Cart) {
                        laid += 1;
                    }
                    // (Anything else, or more than planned: what the battlemap's guarantee added.)
                    if contains(&pk.pts, p) && !pl.props.iter().any(|q| q.2 == o.kind && dist(q.0, p) < 0.5) {
                        let f = horse_foot(o.kind, p, o.rot, o.scale).unwrap_or_else(|| {
                            let r = c.info(o).radius as f64 * o.scale as f64 * SQUARE_FT;
                            [add(p, [-r, -r]), add(p, [r, -r]), add(p, [r, r]), add(p, [-r, r])]
                        });
                        let over = pl.props.iter().filter_map(|q| horse_foot(q.2, q.0, q.1, q.4)).any(|g| boxes_cross(&f, &g));
                        let fenced = pl.fences().iter().any(|&(a, e)| boxes_cross(&f, &seg_rect(a, e, 0.8)));
                        r.add("battlemap: props added on a horse park", 0.0, format!("{here}: {} at {}{}{}", c.name(o), at(p), if over { ", over a planned prop" } else { "" }, if fenced { ", over a fence" } else { "" }));
                    }
                }
                for s in c.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Fence && s.pts.len() >= 2) {
                    let f = |q: [f32; 2]| [ox + q[0] as f64 * SQUARE_FT, oy + q[1] as f64 * SQUARE_FT];
                    let (a, e) = (f(s.pts[0]), f(s.pts[1]));
                    if contains(&pk.pts, lerp(a, e, 0.5)) {
                        fence += dist(a, e);
                    }
                }
                // (The guarantee's rise in each 16-square cell: an auction block (a dais), else a knoll.)
                for s in c.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Dais && !s.pts.is_empty()) {
                    if contains(&pk.pts, [ox + s.pts[0][0] as f64 * SQUARE_FT, oy + s.pts[0][1] as f64 * SQUARE_FT]) {
                        r.tally("battlemap: daises (auction blocks)", 1);
                    }
                }
                let inside = |q: usize| contains(&pk.pts, [ox + ((q % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((q / SQ) as f64 + 0.5) * SQUARE_FT]) && c.building[q] == 0;
                if let Some(low) = (0..SQ * SQ).filter(|&q| inside(q)).map(|q| c.tier[q]).min() {
                    r.tally("battlemap: park squares raised", (0..SQ * SQ).filter(|&q| inside(q) && c.tier[q] > low).count());
                }
                let gap = worldgen::battlemap::cover_distance(&c);
                for q in 0..SQ * SQ {
                    let p = [ox + ((q % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    if contains(&pk.pts, p) && c.water_level[q] <= c.height[q] && gap[q] > worldgen::battlemap::cover_gap(&c, q) {
                        far += 1;
                    }
                    if contains(&pk.pts, p) && c.building[q] == 0 {
                        squares += 1;
                        dirt += (c.surface[q] == Surface::Dirt) as usize;
                    }
                }
            }
        }
        let planned = pl.props.len();
        let want: f64 = pl.fences().iter().map(|(a, e)| dist(*a, *e)).sum();
        r.tally("battlemap: horse props planned", planned);
        r.tally("battlemap: horse props laid", laid);
        r.tally("battlemap: fence ft planned", want as usize);
        r.tally("battlemap: fence ft laid", fence as usize);
        r.tally("battlemap: park squares", squares);
        r.tally("battlemap: dirt squares", dirt);
        if far > 0 {
            r.add("R battlemap park squares beyond cover", far as f64, format!("{here}: {far}"));
        }
        if laid < planned || fence + 1.0 < want {
            r.add("battlemap: parks short of props or fence", 0.0, format!("{here}: props {laid}/{planned}, fence {fence:.0}/{want:.0} ft"));
        }
    }
}

fn check_horse_park(pk: &town::Park, pl: &town::horses::Plan, here: &str, r: &mut Report) {
    use town::horses::{GATE_FT, WAY_HALF};
    let nc = pl.corrals.len();
    r.tally(&format!("parks with {nc} corrals"), 1);
    if nc < 2 {
        r.add("parks with under 2 corrals", 0.0, format!("{here}: {nc}, {:.0} sq ft", area(&pk.pts).abs()));
    }
    let fences = pl.fences();
    let fence_boxes: Vec<[P; 4]> = fences.iter().map(|&(a, b)| seg_rect(a, b, 0.8)).collect();
    let way_boxes: Vec<[P; 4]> = pl.ways.iter().filter(|w| dist(w.0, w.1) > 0.5).map(|&(a, b, _)| seg_rect(a, b, 2.0 * WAY_HALF)).collect();
    // (A gate's opening: its gap, 2 ft either side of the fence.)
    let gate_boxes: Vec<[P; 4]> = pl
        .corrals
        .iter()
        .map(|c| {
            let (a, b) = (c.pts[c.side], c.pts[(c.side + 1) % 4]);
            let u = mul(sub(b, a), 1.0 / dist(a, b));
            seg_rect(sub(c.gate, mul(u, 0.5 * GATE_FT)), add(c.gate, mul(u, 0.5 * GATE_FT)), 4.0)
        })
        .collect();
    let props: Vec<(Kind, [P; 4])> = pl.props.iter().filter_map(|&(at, rot, kind, _, sc)| horse_foot(kind, at, rot, sc).map(|f| (kind, f))).collect();
    for (kind, _) in &props {
        r.tally(&format!("props: {}", format!("{kind:?}").to_lowercase()), 1);
    }
    for (i, (ka, fa)) in props.iter().enumerate() {
        for (kb, fb) in props.iter().skip(i + 1) {
            if boxes_cross(fa, fb) {
                r.add(HORSE_RULES[2], 1.0, format!("{here}: {ka:?} and {kb:?} at {}", at(centroid(fa))));
            }
        }
        if fence_boxes.iter().any(|f| boxes_cross(f, fa)) {
            r.add(HORSE_RULES[3], 1.0, format!("{here}: {ka:?} at {}", at(centroid(fa))));
        }
        if way_boxes.iter().chain(&gate_boxes).any(|f| boxes_cross(f, fa)) {
            r.add(HORSE_RULES[4], 1.0, format!("{here}: {ka:?} at {}", at(centroid(fa))));
        }
        if !fa.iter().all(|&p| contains(&pk.pts, p)) {
            r.add(HORSE_RULES[7], 1.0, format!("{here}: {ka:?} at {}", at(centroid(fa))));
        }
        if *ka == Kind::Horse {
            let pen = pl.corrals.iter().position(|c| fa.iter().all(|&p| contains(&c.pts, p)));
            let market = fa.iter().all(|&p| contains(&pk.pts, p)) && !pl.corrals.iter().any(|c| boxes_cross(&c.pts, fa));
            match (pen, market) {
                (Some(_), _) => r.tally("horses in corrals", 1),
                (None, true) => r.tally("horses on the market ground (tied, led)", 1),
                _ => r.add(HORSE_RULES[6], 1.0, format!("{here}: at {}", at(centroid(fa)))),
            }
        }
    }
    for (k, c) in pl.corrals.iter().enumerate() {
        let n = props.iter().filter(|(kd, f)| *kd == Kind::Horse && f.iter().all(|&p| contains(&c.pts, p))).count();
        r.value("horses a corral", n as f64, format!("{here} corral {k}"));
        let (w, d) = (dist(c.pts[0], c.pts[1]), dist(c.pts[0], c.pts[3]));
        r.value("corral ft (along x in)", w * d, format!("{here} corral {k}: {w:.0} x {d:.0}"));
        for kd in [Kind::WaterTrough, Kind::Haystack] {
            if !props.iter().any(|(x, f)| *x == kd && f.iter().all(|&p| contains(&c.pts, p))) {
                r.add(&format!("corrals with no {}", format!("{kd:?}").to_lowercase()), 0.0, format!("{here} corral {k}"));
            }
        }
    }
    for (a, b) in &fences {
        if way_boxes.iter().any(|w| boxes_cross(w, &seg_rect(*a, *b, 0.8))) {
            r.add(HORSE_RULES[5], 1.0, format!("{here}: fence at {}", at(lerp(*a, *b, 0.5))));
        }
    }
}
