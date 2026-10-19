//! Parks laid out (`town::Park`): what the battlemap and the tiles draw on a park's block.
//!
//! - A formal garden, on ground level enough for a terrace (laid at the mean of its dry ground):
//!   cross axes through its middle (to the streets on the sides that face one, else to the walk), a
//!   walk round it, a basin (a fountain raised a tier) in a gravel round where
//!   the axes meet, and in the quarters hedged parterres sunk a tier inside their hedges (grass banks
//!   down from the hedge), open at the middle of every long side, topiary at their corners, opposite
//!   quarters alike: a bosquet (trees in rows, rides from the gaps to a raised statue or basin in a
//!   clearing) or beds (gravel walks from the gaps to a raised planter, statue or basin, knot beds and
//!   topiary in a checkerboard round it); statues in pairs at the axes' ends, raised planters
//!   halfway along the axes, topiary round the basin's round, benches along the walks, a big one's
//!   avenue of trees inside its walk, a city's lamps.
//! - A common: winding gravel paths (a line pushed about by low noise, smoothed by Chaikin's corner
//!   cutting) from its street sides toward a hub (a fountain or a statue in a gravel round, benches
//!   round it), a later path joining an earlier one where they come near; a big one's walk round it;
//!   on fairly level ground a pond in the lowest hollow that holds one (its level from the ground round
//!   its rim, dug in 6 ft below the ground further round where that is level, its banks eased no
//!   steeper than one in two) with reeds along its far side and a walk round its near side joined to
//!   the paths, now and then a stream from a spring down to it (only where the ground falls all the
//!   way; its spring off the paths) with a footbridge square across it wherever a path crosses, and a
//!   gazebo or a bandstand on a raised tier on its high ground (on a slope, the gazebo stands on the
//!   top with a bench beside it looking downhill, no pond; a small one has the bench alone, a path
//!   climbing to it across the slope); broad low swells in the lawn, some long, some low; clumps of
//!   the biome's trees with shrubs, glades between, a few specimen trees, a loose belt of trees along
//!   its edge (all on the world's grid, each cell with its own rolls), benches along the paths, a
//!   city's lamps.
//!
//! Nothing gravel lies by the water (paths are cut back to the dry, rounds there dropped).
//!
//! A pure function of the park (its outline, sides, ground and key), its biome and the seed, so
//! every chunk and tile that asks lays out the same.

use crate::battlemap::{Kind, PlazaFeature};
use crate::core::hash::FastMap;
use crate::core::noise::fbm;
use crate::core::rng::{Pcg32, hash2, hash3, unit};
use crate::t0::biome::Biome;
use crate::town::geom::*;
use crate::town::{Park, ParkKind};

/// How wide (ft) a common's stream runs.
pub const STREAM_FT: f64 = 8.0;
/// How far past its edge (ft) a stream's banks are cut back, three in ten (`stream_bank`).
pub const STREAM_BANK: f64 = 40.0;
/// How far (ft) a pond's banks ease out to the ground round it, and past that how far (ft) the
/// ground's slope toward it is capped (`pond_bank`).
pub const POND_BAND: f64 = 24.0;
pub const POND_CONE: f64 = 30.0;

/// The ground `e` ft out from a pond's rim (its water at `level`, its band `band`) where it stands
/// at `h`: eased down over the band to its banks (0.3 ft over the water, rising a tenth of a foot a
/// foot), and no steeper than 0.35 from the rim out to `POND_CONE` past the band.
pub(crate) fn pond_bank(h: f64, e: f64, level: f64, band: f64) -> f64 {
    if e >= band + POND_CONE {
        return h;
    }
    let bank = level + 0.3 + 0.1 * e;
    let t = (e / band).min(1.0);
    let h = h + (bank - h) * (1.0 - t * t * (3.0 - 2.0 * t));
    h.clamp(bank - 0.35 * e, bank + 0.35 * e)
}

/// The highest a stream's bank stands `d` ft past its edge (its water at `wl`): three in ten.
pub(crate) fn stream_bank(wl: f64, d: f64) -> f64 {
    wl + 0.3 + 0.3 * d
}

/// How far (ft) a formal parterre sinks inside its hedge, and over what (ft in from its edge) its
/// bank comes down.
pub const SUNK_FT: f64 = 5.5;
pub const SUNK_BANK: (f64, f64) = (4.0, 22.0);

/// A common's pond: its outline, its middle (the outline is star-shaped about it), its water's
/// level (ft) and how far (ft) its banks ease out to the ground round it.
#[derive(Clone, Debug)]
pub struct Pond {
    pub pts: Vec<P>,
    pub centre: P,
    pub level: f32,
    pub band: f64,
}

/// A common's broad low swell: its middle, radius across its narrow way (ft), how far it rises (ft),
/// the way it runs long (a unit vector) and how much longer that way (the radius times `el`; the
/// other way, divided by it).
#[derive(Clone, Copy, Debug)]
pub struct Swell {
    pub at: P,
    pub r: f64,
    pub rise: f32,
    pub axis: P,
    pub el: f64,
}

impl Swell {
    /// How far it reaches from its middle (ft).
    pub fn reach(&self) -> f64 {
        self.r * self.el
    }

    /// How far it lifts the ground at `p` (ft): smoothly from its rise at the middle to 0 at its edge.
    pub fn lift(&self, p: P) -> f32 {
        let d = sub(p, self.at);
        let (a, b) = (dot(d, self.axis) / self.el, (d[1] * self.axis[0] - d[0] * self.axis[1]) * self.el);
        let e = crate::core::sqrt(a * a + b * b) / self.r;
        if e >= 1.0 { 0.0 } else { self.rise * (1.0 - (e * e * (3.0 - 2.0 * e)) as f32) }
    }
}

/// A park as laid out (world ft).
#[derive(Clone, Debug, Default)]
pub struct ParkLayout {
    /// Gravel: paths (centre line, width) and rounds (centre, radius).
    pub paths: Vec<(Vec<P>, f64)>,
    pub rounds: Vec<(P, f64)>,
    /// Ground kept off the gravel (a gazebo's floor): centre, radius.
    pub reserved: Vec<(P, f64)>,
    pub pond: Option<Pond>,
    /// A stream down to the pond: its centre line from the spring, with its water's level (ft) at
    /// each point (`STREAM_FT` wide; empty: none).
    pub stream: Vec<(P, f32)>,
    /// The raised tier: its middle and the radius of its top (ft); the ground rises 6 ft there.
    pub mound: Option<(P, f64)>,
    /// A common's broad low swells.
    pub swells: Vec<Swell>,
    /// A formal garden's main axis (the turn its axis-aligned things take).
    pub axis: Option<f32>,
    /// A formal garden's terrace: the level (ft) its ground is laid at, eased to the streets round it.
    pub level: Option<f32>,
    /// Ground the battlemap's guarantee keeps clear (a bosquet's rides, a hedge's gaps): segments and
    /// how far either side (ft).
    pub keep: Vec<(P, P, f64)>,
    /// A formal garden's parterres sunk a tier (`SUNK_FT`, banked over `SUNK_BANK`).
    pub sunk: Vec<Vec<P>>,
    /// Round features raised a tier (a basin, planters, daises under statues): centre, radius, kind.
    pub features: Vec<(P, f64, PlazaFeature)>,
    /// What is set out: where, turn (rad), kind, look, size.
    pub props: Vec<(P, f32, Kind, u8, f32)>,
    /// A formal garden's parterres (inside their hedges), for the tiles.
    pub parterres: Vec<Vec<P>>,
    /// The park's benches' look.
    pub bench: u8,
}

/// How much a prop's footprint (ft) may reach into the cells next to its own (`Clear`).
const REACH: f64 = 16.0;
const CELL: f64 = 24.0;

/// What is laid out so far, for keeping things apart: path segments (half width) and discs, by
/// `CELL`-ft cell, each listed in every cell its reach (`REACH` beyond it) touches.
struct Clear<'a> {
    poly: &'a [P],
    segs: Vec<(P, P, f64)>,
    discs: Vec<(P, f64)>,
    grid: FastMap<(i64, i64), (Vec<u32>, Vec<u32>)>,
}

impl<'a> Clear<'a> {
    fn new(poly: &'a [P]) -> Self {
        Clear { poly, segs: Vec::new(), discs: Vec::new(), grid: FastMap::default() }
    }

    fn cells(lo: P, hi: P, mut f: impl FnMut((i64, i64))) {
        let c = |v: f64| crate::core::floor(v / CELL) as i64;
        for y in c(lo[1] - REACH)..=c(hi[1] + REACH) {
            for x in c(lo[0] - REACH)..=c(hi[0] + REACH) {
                f((x, y));
            }
        }
    }

    fn seg(&mut self, a: P, b: P, half: f64) {
        let k = self.segs.len() as u32;
        self.segs.push((a, b, half));
        let (lo, hi) = ([a[0].min(b[0]) - half, a[1].min(b[1]) - half], [a[0].max(b[0]) + half, a[1].max(b[1]) + half]);
        Self::cells(lo, hi, |c| self.grid.entry(c).or_default().0.push(k));
    }

    fn path(&mut self, pts: &[P], width: f64) {
        for s in pts.windows(2) {
            self.seg(s[0], s[1], 0.5 * width);
        }
    }

    fn disc(&mut self, p: P, r: f64) {
        let k = self.discs.len() as u32;
        self.discs.push((p, r));
        Self::cells([p[0] - r, p[1] - r], [p[0] + r, p[1] + r], |c| self.grid.entry(c).or_default().1.push(k));
    }

    /// Whether something `r` ft round (at most `REACH`) fits at `p`: `edge` ft inside the outline,
    /// a foot clear of the paths and of what is there.
    fn free(&self, p: P, r: f64, edge: f64) -> bool {
        if depth(self.poly, p) < r + edge {
            return false;
        }
        let Some((segs, discs)) = self.grid.get(&(crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64)) else { return true };
        segs.iter().all(|&k| {
            let (a, b, h) = self.segs[k as usize];
            seg_dist(p, a, b) >= h + r + 1.0
        }) && discs.iter().all(|&k| {
            let (q, s) = self.discs[k as usize];
            dist(p, q) >= r + s + 1.0
        })
    }

    /// How far `p` is from the paths' edges (ft; negative on one), within `REACH`.
    fn path_gap(&self, p: P) -> f64 {
        let Some((segs, _)) = self.grid.get(&(crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64)) else { return REACH };
        segs.iter()
            .map(|&k| {
                let (a, b, h) = self.segs[k as usize];
                seg_dist(p, a, b) - h
            })
            .fold(REACH, f64::min)
    }
}

/// How far `p` is inside `poly` (ft; 0 outside it).
pub(crate) fn depth(poly: &[P], p: P) -> f64 {
    if !contains(poly, p) {
        return 0.0;
    }
    let m = poly.len();
    (0..m).map(|e| seg_dist(p, poly[e], poly[(e + 1) % m])).fold(f64::MAX, f64::min)
}

/// A footprint radius (ft) of a kind at scale 1.
fn foot(kind: Kind) -> f64 {
    crate::battlemap::info(kind as u16).radius as f64 * crate::battlemap::SQUARE_FT
}

/// A sprite's turn that sets its front (+y unturned) toward `dir`.
pub(crate) fn facing(dir: P) -> f32 {
    (libm::atan2(dir[1], dir[0]) - std::f64::consts::FRAC_PI_2) as f32
}

/// The turn of a thing drawn along x lying along `dir`.
fn along(dir: P) -> f32 {
    libm::atan2(dir[1], dir[0]) as f32
}

fn unit_of(v: P) -> P {
    mul(v, 1.0 / len(v).max(1e-9))
}

fn perp(v: P) -> P {
    [-v[1], v[0]]
}

/// Chaikin's corner cutting, `n` times, its ends kept.
fn chaikin(pts: &[P], n: usize) -> Vec<P> {
    let mut cur = pts.to_vec();
    for _ in 0..n {
        if cur.len() < 3 {
            break;
        }
        let mut out = vec![cur[0]];
        for s in cur.windows(2) {
            out.push(lerp(s[0], s[1], 0.25));
            out.push(lerp(s[0], s[1], 0.75));
        }
        out.push(cur[cur.len() - 1]);
        cur = out;
    }
    cur
}

/// Where segment `a`–`b` crosses `c`–`d` (as a fraction along `a`–`b`), if it does.
fn crossing(a: P, b: P, c: P, d: P) -> Option<f64> {
    let (r, s) = (sub(b, a), sub(d, c));
    let den = cross(r, s);
    if crate::core::fabs(den) < 1e-9 {
        return None;
    }
    let (t, u) = (cross(sub(c, a), s) / den, cross(sub(c, a), r) / den);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// A polyline's length (ft).
fn length(pts: &[P]) -> f64 {
    pts.windows(2).map(|s| dist(s[0], s[1])).sum()
}

/// Points every `step` ft along a polyline (with the way it runs), from `from` ft to `to` ft short of its end.
fn every(pts: &[P], step: f64, from: f64, to: f64) -> Vec<(P, P)> {
    let total = length(pts);
    let mut out = Vec::new();
    let mut at = from;
    while at <= total - to {
        let mut acc = 0.0;
        for s in pts.windows(2) {
            let l = dist(s[0], s[1]);
            if acc + l >= at && l > 1e-9 {
                out.push((lerp(s[0], s[1], (at - acc) / l), mul(sub(s[1], s[0]), 1.0 / l)));
                break;
            }
            acc += l;
        }
        at += step;
    }
    out
}

/// A polygon's bounds: its lowest and highest corner.
fn bbox_of(pts: &[P]) -> (P, P) {
    pts.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1])], [hi[0].max(q[0]), hi[1].max(q[1])]))
}

/// The nearest point of segment `a`–`b` to `p`, and how far along it is (0–1).
pub(crate) fn project(a: P, b: P, p: P) -> (P, f64) {
    let ab = sub(b, a);
    let t = (dot(sub(p, a), ab) / dot(ab, ab).max(1e-9)).clamp(0.0, 1.0);
    (add(a, mul(ab, t)), t)
}

/// The nearest point of a polyline to `p`, and how far it is.
fn nearest_on(pts: &[P], p: P) -> (P, f64) {
    let mut best = (pts[0], dist(pts[0], p));
    for s in pts.windows(2) {
        let q = project(s[0], s[1], p).0;
        let d = dist(q, p);
        if d < best.1 {
            best = (q, d);
        }
    }
    best
}

/// The runs of a polyline (every 5 ft) on dry ground (`wet` false), each at least 15 ft long.
fn dry_runs(pts: &[P], wet: &dyn Fn(P) -> bool) -> Vec<Vec<P>> {
    let mut runs: Vec<Vec<P>> = vec![Vec::new()];
    for s in pts.windows(2) {
        let n = (crate::core::ceil(dist(s[0], s[1]) / 5.0) as usize).max(1);
        for k in 0..=n {
            if k == 0 && runs.last().is_some_and(|r| !r.is_empty()) {
                continue;
            }
            let q = lerp(s[0], s[1], k as f64 / n as f64);
            if wet(q) {
                if runs.last().is_some_and(|r| !r.is_empty()) {
                    runs.push(Vec::new());
                }
            } else if k == 0 || k == n || runs.last().is_some_and(|r| r.is_empty()) {
                runs.last_mut().expect("a run").push(q);
            } else if k < n && wet(lerp(s[0], s[1], (k + 1) as f64 / n as f64)) {
                runs.last_mut().expect("a run").push(q);
            }
        }
    }
    runs.into_iter().filter(|r| r.len() >= 2 && length(r) >= 15.0).collect()
}

/// The span of the line through `c` along `dir` inside convex `poly`: (t0, t1) with the points
/// at `c + t·dir`.
fn chord(poly: &[P], c: P, dir: P) -> (f64, f64) {
    let (mut t0, mut t1) = (f64::MIN, f64::MAX);
    let ccw = area(poly) > 0.0;
    let m = poly.len();
    for e in 0..m {
        let (a, b) = (poly[e], poly[(e + 1) % m]);
        let ev = sub(b, a);
        // Inward normal.
        let n = if ccw { [-ev[1], ev[0]] } else { [ev[1], -ev[0]] };
        let (num, den) = (dot(sub(c, a), n), dot(dir, n));
        if crate::core::fabs(den) < 1e-12 {
            continue;
        }
        let t = -num / den;
        if den > 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
    }
    (t0, t1)
}

/// The edge of `poly` nearest `p`.
fn edge_at(poly: &[P], p: P) -> usize {
    let m = poly.len();
    (0..m).min_by(|&a, &b| seg_dist(p, poly[a], poly[(a + 1) % m]).total_cmp(&seg_dist(p, poly[b], poly[(b + 1) % m]))).unwrap_or(0)
}

/// What grows in a park in a biome: its trees and shrubs (as a town's green has them; something in
/// their place where the biome has none), its small plants, and whether its trees are few.
fn plants(b: Biome) -> (&'static [Kind], &'static [Kind], &'static [Kind], bool) {
    let (trees, shrubs, small, few) = crate::battlemap::greenery(b);
    (if trees.is_empty() { &[Kind::TreeDeciduous] } else { trees }, if shrubs.is_empty() { &[Kind::Bush] } else { shrubs }, small, few)
}

/// Park `p` laid out (`biome`: the land's round it; `city`: a city's or a metropolis's, with
/// lamps).
pub fn layout(p: &Park, biome: Biome, city: bool, seed: u64) -> ParkLayout {
    let key = hash2(seed ^ 0x9a4c, p.key as i64, 0x51);
    let mut rng = Pcg32::new(key, 37);
    let mut out = match p.kind {
        ParkKind::Formal => formal(p, biome, city, &mut rng),
        ParkKind::Common => common(p, biome, city, key, &mut rng),
    };
    // No gravel by the water: paths cut back to the dry, rounds there dropped.
    let wet = |q: P| p.ground.water_near(q, 8.0);
    out.paths = out.paths.iter().flat_map(|(pts, w)| dry_runs(pts, &wet).into_iter().map(move |r| (r, *w))).collect();
    out.rounds.retain(|&(c, r)| !p.ground.water_near(c, r + 4.0));
    // What stands on the lawn stands wholly inside it, a foot to spare (trees and hedges may reach
    // over its edge).
    let whole = |k: Kind| matches!(k, Kind::Fountain | Kind::GardenStatue | Kind::Gazebo | Kind::Bandstand | Kind::Bench | Kind::Flowerbed | Kind::LampPost);
    out.props.retain(|pr| !whole(pr.2) || depth(&p.pts, pr.0) >= foot(pr.2) * pr.4 as f64 + 1.0);
    // Every piece of hedge (a common's too) wholly inside one chunk, as play mode blocks it.
    seam_split(&mut out, 0);
    // No hedge on a path's gravel: a piece blocking a square the battlemap lays as gravel (its middle
    // within half the path's width and 0.6 ft of its line, or inside a round) goes, the hedge stopping
    // short.
    let (paths, rounds) = (&out.paths, &out.rounds);
    let mut sq = Vec::new();
    out.props.retain(|pr| {
        if pr.2 != Kind::Hedge {
            return true;
        }
        sq.clear();
        blocked_any(pr, &mut sq);
        !sq.iter().any(|&(i, j)| {
            let c = [(i as f64 + 0.5) * 5.0, (j as f64 + 0.5) * 5.0];
            paths.iter().any(|(pts, w)| pts.windows(2).any(|s| seg_dist(c, s[0], s[1]) < 0.5 * w + 0.6)) || rounds.iter().any(|&(o, r)| dist(c, o) < r)
        })
    });
    // One of each thing at a place: none within a foot of another of its kind (two parterres' hedges
    // along a shared edge).
    let mut seen: FastMap<(u16, i64, i64), Vec<P>> = FastMap::default();
    let cell = |v: f64| crate::core::floor(v) as i64;
    out.props.retain(|pr| {
        let (k, i, j) = (pr.2 as u16, cell(pr.0[0]), cell(pr.0[1]));
        let near = (-1..=1).any(|dj| (-1..=1).any(|di| seen.get(&(k, i + di, j + dj)).is_some_and(|v| v.iter().any(|&q| dist(q, pr.0) < 1.0))));
        if !near {
            seen.entry((k, i, j)).or_default().push(pr.0);
        }
        !near
    });
    // Nothing shut in by what is set out, as play mode blocks it.
    unseal(&p.pts, &mut out, 0);
    // Every bench in the park's look.
    for pr in &mut out.props {
        if pr.2 == Kind::Bench {
            pr.3 = out.bench;
        }
    }
    out
}

/// A little noise by spot (0–1).
fn jit(key: u64, p: P, salt: i64) -> f64 {
    unit(hash3(key ^ salt as u64, crate::core::round(p[0] * 2.0) as i64, crate::core::round(p[1] * 2.0) as i64, salt))
}

fn formal(p: &Park, biome: Biome, city: bool, rng: &mut Pcg32) -> ParkLayout {
    let poly = &p.pts;
    let mut out = ParkLayout::default();
    let mut clear = Clear::new(poly);
    let o = obb(poly);
    let (u, v) = (o.axis, perp(o.axis));
    out.axis = Some(along(u));
    let c = centroid(poly);
    let big = o.short >= 220.0;
    // Bands in from the edge: the walk round it, a big one's avenue of trees, the parterres.
    let walk_at = 12.0;
    let avenue_at = 25.0;
    let inner_at = if big { 36.0 } else { 22.0 };
    const AXIS_W: f64 = 10.0;
    const WALK_W: f64 = 8.0;
    // One look each for the garden: hedges (box or yew, hawthorn now and then), topiary, statues,
    // benches, lamps, beds; and what stands in each pair of opposite parterres' middles.
    let hedge = [0, 0, 1, 1, 2][rng.below(5) as usize];
    let topiary = rng.below(4) as u8;
    let statue = [0, 2, 3, 1][rng.below(4) as usize];
    out.bench = [1, 1, 2][rng.below(3) as usize];
    let lamp = [0, 2][rng.below(2) as usize];
    let bed = [3, 3, 0, 1, 2][rng.below(5) as usize];
    let mids = [rng.below(3), rng.below(3)];
    let (trees, _, _, _) = plants(biome);
    let tree = trees[rng.below(trees.len() as u32) as usize];
    // The terrace's level: the mean of the dry ground planned inside it, where that ground lies within
    // 8 ft of level (on a slope, one set by hand, the garden follows the ground).
    let g = &p.ground;
    let inside: Vec<f64> = (0..g.ny)
        .flat_map(|j| (0..g.nx).map(move |i| (i, j)))
        .filter(|&(i, j)| !g.water[j * g.nx + i] && contains(poly, g.point(i, j)))
        .map(|(i, j)| g.h[j * g.nx + i] as f64)
        .collect();
    let (lo, hi) = inside.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    if !inside.is_empty() && hi - lo <= 8.0 {
        out.level = Some((inside.iter().sum::<f64>() / inside.len() as f64) as f32);
    }

    // The basin where the axes meet (a fountain raised a tier on a big one, a fountain on a small
    // one), in a gravel round.
    let (rb, rr) = if o.short >= 150.0 {
        let rb = (0.045 * o.short).clamp(9.0, 15.0);
        out.features.push((c, rb, PlazaFeature::Fountain));
        (rb, rb + 9.0)
    } else {
        out.props.push((c, 0.0, Kind::Fountain, [0, 1, 2][rng.below(3) as usize], 1.3));
        (foot(Kind::Fountain) * 1.3, 13.0)
    };
    out.rounds.push((c, rr));
    clear.disc(c, rb + 2.0);
    // The walk round, and the axes: to the street on a side that faces one, else to the walk.
    let ring = inset(poly, &vec![walk_at; poly.len()]);
    let mut ends: Vec<(P, P)> = Vec::new();
    let mut axes: Vec<Vec<P>> = Vec::new();
    for dir in [u, v] {
        let (t0, t1) = chord(poly, c, dir);
        if t1 - t0 < 40.0 {
            continue;
        }
        let (r0, r1) = if ring.len() >= 3 { chord(&ring, c, dir) } else { (t0, t1) };
        let front = |t: f64| p.fronts.get(edge_at(poly, add(c, mul(dir, t)))).copied().unwrap_or(true);
        let (a, b) = (if front(t0) { t0 } else { r0 }, if front(t1) { t1 } else { r1 });
        axes.push(vec![add(c, mul(dir, a)), add(c, mul(dir, b))]);
        ends.push((add(c, mul(dir, r0)), dir));
        ends.push((add(c, mul(dir, r1)), mul(dir, -1.0)));
    }
    for pts in axes {
        clear.path(&pts, AXIS_W);
        out.paths.push((pts, AXIS_W));
    }
    if ring.len() >= 3 {
        let mut pts = ring.clone();
        pts.push(ring[0]);
        clear.path(&pts, WALK_W);
        out.paths.push((pts, WALK_W));
    }
    // Statues in pairs at the axes' ends, inside the walk, either side of the axis.
    for &(e, inward) in &ends {
        let at = add(e, mul(inward, 0.5 * WALK_W + 7.0));
        let side = perp(inward);
        for s in [-1.0, 1.0] {
            let q = add(at, mul(side, s * (0.5 * AXIS_W + 4.5)));
            if clear.free(q, foot(Kind::GardenStatue), 2.0) {
                clear.disc(q, foot(Kind::GardenStatue));
                out.props.push((q, 0.0, Kind::GardenStatue, statue, 1.0));
            }
        }
    }
    // Lamps round the basin (a city's), and topiary round its gravel (cover by the round).
    if city {
        for k in 0..4 {
            let d = add(mul(u, [1.0, -1.0, -1.0, 1.0][k]), mul(v, [1.0, 1.0, -1.0, -1.0][k]));
            let q = add(c, mul(unit_of(d), rr + 2.5));
            if clear.free(q, foot(Kind::LampPost), 2.0) {
                clear.disc(q, foot(Kind::LampPost));
                out.props.push((q, 0.0, Kind::LampPost, lamp, 1.0));
            }
        }
    }
    for k in 0..8 {
        let t = std::f64::consts::TAU * (k as f64 + 0.5) / 8.0 + libm::atan2(u[1], u[0]);
        let q = add(c, mul([libm::cos(t), libm::sin(t)], rr + 5.0));
        if clear.free(q, foot(Kind::Topiary), 2.0) {
            clear.disc(q, foot(Kind::Topiary));
            out.props.push((q, 0.0, Kind::Topiary, topiary, 1.0));
        }
    }
    // Benches along the axes, facing them (both sides, in step), and along the walk's inner side.
    for (pi, path) in out.paths.clone().into_iter().enumerate() {
        let (pts, w) = (&path.0, path.1);
        let axis = pi + 1 < out.paths.len() || ring.len() < 3;
        for (k, (q, d)) in every(pts, if axis { 32.0 } else { 44.0 }, if axis { 0.0 } else { 20.0 }, 0.0).into_iter().enumerate() {
            let side = perp(d);
            let sides: &[f64] = if axis { &[-1.0, 1.0] } else { &[1.0] };
            for &s in sides {
                // (The walk's inner side: toward the middle.)
                let s = if axis { s } else if dot(side, sub(c, q)) >= 0.0 { 1.0 } else { -1.0 };
                let b = add(q, mul(side, s * (0.5 * w + foot(Kind::Bench) + 1.3)));
                if (axis && dist(q, c) < rr + 12.0) || !clear.free(b, foot(Kind::Bench), 2.0) {
                    continue;
                }
                clear.disc(b, foot(Kind::Bench));
                out.props.push((b, facing(mul(side, -s)), Kind::Bench, out.bench, 1.0));
                // A city's lamp on the other side of an axis now and then.
                if city && axis && k % 2 == 1 && s > 0.0 {
                    let l = add(q, mul(side, -(0.5 * w + foot(Kind::LampPost) + 1.3)));
                    if clear.free(l, foot(Kind::LampPost), 2.0) {
                        clear.disc(l, foot(Kind::LampPost));
                        out.props.push((l, 0.0, Kind::LampPost, lamp, 1.0));
                    }
                }
            }
        }
    }
    // A big one's avenue of trees inside its walk.
    if big {
        let av = inset(poly, &vec![avenue_at; poly.len()]);
        if av.len() >= 3 {
            let mut pts = av.clone();
            pts.push(av[0]);
            for (q, _) in every(&pts, 28.0, 10.0, 10.0) {
                if clear.free(q, 0.7 * foot(tree), 2.0) {
                    clear.disc(q, 0.7 * foot(tree));
                    out.props.push((q, 0.0, tree, rng.below(8) as u8, 1.0));
                }
            }
        }
    }
    // The parterres: the quarters between the axes inside the walk (or the avenue), off the round.
    let inner = inset(poly, &vec![inner_at; poly.len()]);
    let gap = 0.5 * AXIS_W + 15.0;
    let style = rng.below(2);
    for (su, sv) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        if inner.len() < 3 {
            break;
        }
        let mut q = clip_plain(&inner, mul(u, -su), -su * dot(c, u) - gap);
        q = clip_plain(&q, mul(v, -sv), -sv * dot(c, v) - gap);
        let diag = unit_of(add(mul(u, su), mul(v, sv)));
        q = clip_plain(&q, mul(diag, -1.0), -dot(c, diag) - (rr + 8.0));
        let q = clean(&q);
        if q.len() < 3 || area(&q).abs() < 900.0 || obb(&q).short < 28.0 || q.iter().chain([&centroid(&q)]).any(|&x| g.water_near(x, 12.0)) {
            continue;
        }
        // Opposite quarters alike.
        let pair = (su * sv > 0.0) as usize;
        let bosquet = (pair == 1) == (style == 0) && obb(&q).short >= 60.0;
        let look = Looks { hedge, topiary, statue, bed, tree, mid: mids[pair] };
        parterre(&q, bosquet, look, rng, &mut clear, &mut out);
        out.parterres.push(q);
    }
    // A raised planter either side of each half axis, halfway out, clear of the parterres' walks.
    for &(e, inward) in &ends {
        let mid = lerp(c, e, 0.5);
        let side = perp(inward);
        for s in [-1.0, 1.0] {
            let q = add(mid, mul(side, s * (0.5 * AXIS_W + 8.0)));
            if dist(mid, c) > rr + 20.0 && clear.free(q, 6.5, inner_at) {
                clear.disc(q, 6.5);
                out.features.push((q, 4.5, PlazaFeature::Planter));
            }
        }
    }
    out
}

/// A battlemap chunk's side (ft): play mode blocks a chunk's squares by its own objects only.
const CHUNK_FT: f64 = crate::battlemap::SQ as f64 * crate::battlemap::SQUARE_FT;

/// The squares (on the world's 5-ft grid) a prop blocks as play mode has it: a hedge every square its
/// piece is drawn across (`battlePrep` LONG_FOOTPRINT), anything else that blocks the squares within
/// half its footprint; only those of the chunk it stands in (the chunk holding its middle).
fn blocked_squares(pr: &(P, f32, Kind, u8, f32), out: &mut Vec<(i64, i64)>) {
    let start = out.len();
    blocked_any(pr, out);
    let n = crate::battlemap::SQ as i64;
    let chunk = |v: f64| crate::core::floor(v / CHUNK_FT) as i64;
    let (cx, cy) = (chunk(pr.0[0]), chunk(pr.0[1]));
    let mut k = start;
    while k < out.len() {
        if out[k].0.div_euclid(n) != cx || out[k].1.div_euclid(n) != cy {
            out.swap_remove(k);
        } else {
            k += 1;
        }
    }
}

fn blocked_any(pr: &(P, f32, Kind, u8, f32), out: &mut Vec<(i64, i64)>) {
    let (at, rot, kind, _, scale) = *pr;
    let inf = crate::battlemap::info(kind as u16);
    if !inf.blocks_move {
        return;
    }
    let (x, y) = (at[0] / 5.0, at[1] / 5.0);
    if kind == Kind::Hedge {
        let (cs, sn) = (libm::cos(rot as f64), libm::sin(rot as f64));
        let (hl, hw) = (0.49 * scale as f64, 0.29 * scale as f64);
        let mut u = -hl;
        while u <= hl + 1e-6 {
            let mut w = -hw;
            while w <= hw + 1e-6 {
                out.push((crate::core::floor(x + cs * u - sn * w) as i64, crate::core::floor(y + sn * u + cs * w) as i64));
                w += 0.1f64.min(hw);
            }
            u += 0.1f64.min(hl);
        }
        return;
    }
    let reach = (inf.radius as f64 * scale as f64 * 0.5).max(0.5);
    for j in crate::core::floor(y - reach) as i64..=crate::core::floor(y + reach) as i64 {
        for i in crate::core::floor(x - reach) as i64..=crate::core::floor(x + reach) as i64 {
            let (dx, dy) = (i as f64 + 0.5 - x, j as f64 + 0.5 - y);
            if dx * dx + dy * dy <= reach * reach {
                out.push((i, j));
            }
        }
    }
}

/// Hedge pieces (from `from` on in `out.props`) each wholly inside one chunk, as play mode blocks
/// them, every piece kept on its run's line: a piece across a chunk's edge becomes the part of it on
/// each side, each ending a little short of the edge (a part 85% of a piece long or more drawn at its
/// length; a shorter one drawn at 85% of a piece, set back along the run from the edge, so it overlaps
/// the piece before it; under 2.5 ft, left out); a piece over an edge by its side (lying along it, or
/// by a corner) is shortened from its end nearer the edge, or left out.
fn seam_split(out: &mut ParkLayout, from: usize) {
    let mut k = from;
    while k < out.props.len() {
        let pr = out.props[k];
        if pr.2 != Kind::Hedge || seam_inside(&pr) {
            k += 1;
            continue;
        }
        let parts = seam_fit(&pr, 3);
        let n = parts.len();
        out.props.splice(k..k + 1, parts);
        k += n;
    }
}

/// Whether a hedge piece's footprint (play's, 2.45 by 1.45 ft each way at full size, and 0.1 ft to
/// spare: the chunk stores its place as a float) lies wholly in the chunk holding its middle.
pub(crate) fn seam_inside(pr: &(P, f32, Kind, u8, f32)) -> bool {
    let (d, s) = ([libm::cos(pr.1 as f64), libm::sin(pr.1 as f64)], pr.4 as f64);
    let (l, w) = (mul(d, 2.45 * s + 0.1), mul(perp(d), 1.45 * s + 0.1));
    let chunk = |p: P| (crate::core::floor(p[0] / CHUNK_FT) as i64, crate::core::floor(p[1] / CHUNK_FT) as i64);
    [add(add(pr.0, l), w), add(sub(pr.0, l), w), sub(add(pr.0, l), w), sub(sub(pr.0, l), w)].iter().all(|&c| chunk(c) == chunk(pr.0))
}

/// One hedge piece made pieces each wholly inside a chunk, on its line (`seam_split`), `depth` times
/// over at most (a chunk's corner).
fn seam_fit(pr: &(P, f32, Kind, u8, f32), depth: u32) -> Vec<(P, f32, Kind, u8, f32)> {
    if seam_inside(pr) {
        return vec![*pr];
    }
    if depth == 0 {
        return Vec::new();
    }
    const FULL: f64 = 4.9;
    let d = [libm::cos(pr.1 as f64), libm::sin(pr.1 as f64)];
    let l = 0.5 * FULL * pr.4 as f64;
    // The piece spanning `a`..`b` along its line (from its old middle), drawn to that length.
    let piece = |a: f64, b: f64| (add(pr.0, mul(d, 0.5 * (a + b))), pr.1, pr.2, pr.3, ((b - a) / FULL) as f32);
    // The longest part from `end` toward `toward` (the old piece's other end) wholly in one chunk.
    let part = |end: f64, toward: f64| -> Option<(P, f32, Kind, u8, f32)> {
        let sgn = if toward > end { 1.0 } else { -1.0 };
        let mut len = (toward - end).abs();
        let at = |len: f64| if sgn > 0.0 { piece(end, end + len) } else { piece(end - len, end) };
        while len >= 2.5 && !seam_inside(&at(len)) {
            len -= 0.25;
        }
        if len < 2.5 {
            return None;
        }
        if len >= 0.85 * FULL {
            return Some(at(len));
        }
        // (Too short to draw as itself: a piece 85% long, its near end where the part's was.)
        let near = end + sgn * len;
        let p = if sgn > 0.0 { piece(near - 0.85 * FULL, near) } else { piece(near, near + 0.85 * FULL) };
        seam_inside(&p).then_some(p)
    };
    let home = |p: P| (crate::core::floor(p[0] / CHUNK_FT) as i64, crate::core::floor(p[1] / CHUNK_FT) as i64);
    // Where the line meets a chunk's edge inside the piece, if it does.
    let (ca, cb) = (home(add(pr.0, mul(d, -l))), home(add(pr.0, mul(d, l))));
    let mut parts = Vec::new();
    if ca != cb {
        let ax = if ca.0 != cb.0 { 0 } else { 1 };
        let line = (if ax == 0 { ca.0.max(cb.0) } else { ca.1.max(cb.1) }) as f64 * CHUNK_FT;
        let t = (line - pr.0[ax]) / d[ax];
        // Each side's part, from the old end to a little short of the edge.
        parts.extend(part(-l, t - 0.1));
        parts.extend(part(l, t + 0.1));
    } else {
        // Over the edge by its side: shortened from the end nearer the edge.
        let a = part(-l, l);
        let b = part(l, -l);
        let longer = match (a, b) {
            (Some(a), Some(b)) => Some(if b.4 > a.4 { b } else { a }),
            (a, b) => a.or(b),
        };
        parts.extend(longer);
    }
    parts.into_iter().flat_map(|p| seam_fit(&p, depth - 1)).collect()
}

/// No pocket of `region` walled in by what is set out there (from `from` on in `out.props`): its
/// squares walked from round it, square to square, as play mode blocks them; where one is shut in,
/// the piece nearest it goes (a hedge's, a tree's), until none is.
fn unseal(region: &[P], out: &mut ParkLayout, from: usize) {
    let ([x0, y0], [x1, y1]) = bbox_of(region);
    let (i0, j0) = (crate::core::floor(x0 / 5.0) as i64 - 3, crate::core::floor(y0 / 5.0) as i64 - 3);
    let (w, h) = ((crate::core::floor(x1 / 5.0) as i64 + 4 - i0) as usize, (crate::core::floor(y1 / 5.0) as i64 + 4 - j0) as usize);
    let centre = |i: usize, j: usize| [(i0 as f64 + i as f64 + 0.5) * 5.0, (j0 as f64 + j as f64 + 0.5) * 5.0];
    for _ in 0..40 {
        let mut block = vec![false; w * h];
        let mut sq = Vec::new();
        for pr in &out.props[from..] {
            sq.clear();
            blocked_squares(pr, &mut sq);
            for &(i, j) in &sq {
                let (a, b) = (i - i0, j - j0);
                if a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h {
                    block[b as usize * w + a as usize] = true;
                }
            }
        }
        let mut seen = vec![false; w * h];
        let mut queue = std::collections::VecDeque::new();
        for j in 0..h {
            for i in 0..w {
                if (i == 0 || j == 0 || i == w - 1 || j == h - 1 || !contains(region, centre(i, j))) && !block[j * w + i] {
                    seen[j * w + i] = true;
                    queue.push_back(j * w + i);
                }
            }
        }
        while let Some(k) = queue.pop_front() {
            let (i, j) = ((k % w) as i64, (k / w) as i64);
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i + di, j + dj);
                if a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h {
                    let n = b as usize * w + a as usize;
                    if !seen[n] && !block[n] {
                        seen[n] = true;
                        queue.push_back(n);
                    }
                }
            }
        }
        let Some(k) = (0..w * h).find(|&k| !seen[k] && !block[k]) else { return };
        let p = centre(k % w, k / w);
        let near = (from..out.props.len())
            .filter(|&q| crate::battlemap::info(out.props[q].2 as u16).blocks_move && out.props[q].2 != Kind::GardenStatue)
            .min_by(|&a, &b| dist(out.props[a].0, p).total_cmp(&dist(out.props[b].0, p)).then(a.cmp(&b)));
        match near {
            Some(q) => {
                out.props.remove(q);
            }
            None => return,
        }
    }
}

/// One garden's looks for its parterres, and what stands in the middle of this one's pair.
#[derive(Clone, Copy)]
struct Looks {
    hedge: u8,
    topiary: u8,
    statue: u8,
    bed: u8,
    tree: Kind,
    mid: u32,
}

/// One parterre `q` (convex): a hedge round it 2.5 ft in, open (wide enough that the hedge's squares
/// leave two free) at the middle of every side 24 ft or longer (else of its longest), topiary at its
/// corners; sunk a tier inside its hedge if there is room for a floor. In the middle a raised
/// feature (a planter, a statue on a dais or a basin; a bosquet's statue, basin or pool), reached
/// from every gap by a walk (gravel through beds, a grass ride through a bosquet); then trees in rows
/// (a bosquet) or knot beds and topiary in a checkerboard on its floor.
fn parterre(q: &[P], bosquet: bool, look: Looks, rng: &mut Pcg32, clear: &mut Clear, out: &mut ParkLayout) {
    let first = out.props.len();
    let m = q.len();
    let ccw = area(q) > 0.0;
    let qc = centroid(q);
    let o = obb(q);
    let (u, v) = (o.axis, perp(o.axis));
    let sunk = o.short >= 70.0;
    if sunk {
        out.sunk.push(q.to_vec());
    }
    let longest = (0..m).max_by(|&a, &b| dist(q[a], q[(a + 1) % m]).total_cmp(&dist(q[b], q[(b + 1) % m]))).unwrap_or(0);
    let mut gaps: Vec<P> = Vec::new();
    for e in 0..m {
        let (a, b) = (q[e], q[(e + 1) % m]);
        let l = dist(a, b);
        if l < 8.0 {
            continue;
        }
        let d = mul(sub(b, a), 1.0 / l);
        let n_in = if ccw { [-d[1], d[0]] } else { [d[1], -d[0]] };
        let open = l >= 24.0 || e == longest;
        let half = 8.0f64.min(0.5 * l - 2.0);
        if open {
            gaps.push(add(lerp(a, b, 0.5), mul(n_in, 2.5)));
        }
        // (Open at a sharp corner too: the hedges meeting there would shut its tip in.)
        let sharp = |k: usize| {
            let (pa, at, pb) = (q[(k + m - 1) % m], q[k], q[(k + 1) % m]);
            dot(unit_of(sub(pa, at)), unit_of(sub(pb, at))) > 0.3
        };
        // (Far enough back from a sharp corner that the pieces' ends, 1.45 ft thick, leave 6 ft between
        // them at its tip.)
        let skip = |k: usize| {
            if !sharp(k) {
                return 0.0;
            }
            let (pa, at, pb) = (q[(k + m - 1) % m], q[k], q[(k + 1) % m]);
            let half = 0.5 * libm::acos(dot(unit_of(sub(pa, at)), unit_of(sub(pb, at))).clamp(-1.0, 1.0));
            (2.5 + 8.9 / (2.0 * libm::sin(half)).max(0.2)).max(7.0)
        };
        let (skip_a, skip_b) = (skip(e), skip((e + 1) % m));
        let mut t = 2.5;
        while t <= l - 2.5 {
            let h = add(add(a, mul(d, t)), mul(n_in, 2.5));
            // (Not across a path: the hedge stops at it.)
            if (!open || crate::core::fabs(t - 0.5 * l) >= half) && t >= skip_a && t <= l - skip_b && clear.path_gap(h) > 2.0 {
                out.props.push((h, along(d), Kind::Hedge, look.hedge, 1.0));
                clear.disc(h, 2.5);
            }
            t += 5.0;
        }
    }
    // Topiary at its corners.
    for k in 0..m {
        let (prev, at, next) = (q[(k + m - 1) % m], q[k], q[(k + 1) % m]);
        let bis = add(unit_of(sub(prev, at)), unit_of(sub(next, at)));
        if len(bis) < 0.2 {
            continue;
        }
        let t = add(at, mul(unit_of(bis), 8.0));
        if depth(q, t) >= 4.0 && clear.free(t, foot(Kind::Topiary), 0.0) {
            clear.disc(t, foot(Kind::Topiary));
            out.props.push((t, 0.0, Kind::Topiary, look.topiary, 1.0));
        }
    }
    // The middle, raised a tier.
    let room = depth(q, qc) >= 16.0;
    let clearing = if bosquet { 16.0 } else { 10.0 };
    if room {
        match (bosquet, look.mid) {
            (false, 0) => out.features.push((qc, 5.5, PlazaFeature::Planter)),
            (_, 1) => out.features.push((qc, 6.5, PlazaFeature::Fountain)),
            (true, 2) => out.props.push((qc, along(u), Kind::Pond, 2, 1.0)),
            _ => {
                out.features.push((qc, 4.5, PlazaFeature::Dais));
                out.props.push((qc, 0.0, Kind::GardenStatue, look.statue, 1.0));
            }
        }
        clear.disc(qc, clearing);
    }
    // Walks from the gaps to the middle (gravel through beds; a ride kept clear through a bosquet).
    for &g in &gaps {
        let end = if room { add(qc, mul(unit_of(sub(g, qc)), clearing - 1.0)) } else { qc };
        out.keep.push((g, g, 6.0));
        if bosquet {
            clear.seg(g, end, 3.5);
            out.keep.push((g, end, 3.5));
        } else {
            let pts = vec![add(g, mul(unit_of(sub(qc, g)), 1.5)), end];
            clear.path(&pts, 7.0);
            out.paths.push((pts, 7.0));
        }
    }
    // Trees in rows (a bosquet), else knot beds and topiary in a checkerboard, on its floor.
    let floor = if sunk { SUNK_BANK.1 } else { 9.0 };
    let (step, from) = if bosquet { (24.0, 14.0) } else { (13.0, floor) };
    let (a0, a1) = extent(q, u);
    let (b0, b1) = extent(q, v);
    let (cu, cv) = (dot(qc, u), dot(qc, v));
    let (nu, nv) = (((a1 - a0) / step) as i64 / 2 + 1, ((b1 - b0) / step) as i64 / 2 + 1);
    for i in -nu..=nu {
        for j in -nv..=nv {
            let p = add(mul(u, cu + i as f64 * step), mul(v, cv + j as f64 * step));
            if depth(q, p) < from {
                continue;
            }
            let (kind, look_v, r) = if bosquet {
                (look.tree, rng.below(8) as u8, 0.7 * foot(look.tree))
            } else if (i + j).rem_euclid(2) == 0 {
                (Kind::Flowerbed, look.bed, 0.8 * foot(Kind::Flowerbed))
            } else {
                (Kind::Topiary, look.topiary, foot(Kind::Topiary))
            };
            if clear.free(p, r, 0.0) {
                clear.disc(p, r);
                let rot = if kind == Kind::Flowerbed { along(u) } else { 0.0 };
                out.props.push((p, rot, kind, look_v, 1.0));
            }
        }
    }
    seam_split(out, first);
    unseal(q, out, first);
}

/// A stream's way down to the pond, as the ground has it: the water's level along `pts` (from the
/// spring to the mouth), a foot below the ground and never rising toward the pond, smoothed; none
/// where the ground doesn't fall to the pond (it would climb out of its bed or cut a trench).
fn stream_levels(pts: &[P], ground: &crate::town::ParkGround, level: f64, band: f64, rim: &[P], mouth: P) -> Option<Vec<f32>> {
    // The ground as the battlemap lays it round the pond: eased down to its banks over its band,
    // no steeper than 0.35 from there out to 30 ft past it.
    let laid = |q: P| {
        let h = ground.at(q);
        let e = (0..rim.len()).map(|k| seg_dist(q, rim[k], rim[(k + 1) % rim.len()])).fold(f64::MAX, f64::min);
        if contains(rim, q) { h } else { pond_bank(h, e, level, band) }
    };
    let side = |k: usize, off: f64| {
        let (a, b) = (pts[k.saturating_sub(1)], pts[(k + 1).min(pts.len() - 1)]);
        let n = mul(perp(unit_of(sub(b, a))), off);
        (laid(add(pts[k], n)), laid(sub(pts[k], n)))
    };
    // (The lowest of the ground on its line and on either bank, 4 and 10 ft past its edge; and the
    // higher of the ground either side 19 ft past its edge.)
    // (At its head, the ground behind the spring too.)
    let behind = |off: f64| laid(sub(pts[0], mul(unit_of(sub(pts[1.min(pts.len() - 1)], pts[0])), off)));
    let g: Vec<f64> = (0..pts.len())
        .map(|k| {
            let ((l, r), (l2, r2)) = (side(k, 0.5 * STREAM_FT + 4.0), side(k, 0.5 * STREAM_FT + 10.0));
            let h = laid(pts[k]).min(l).min(r).min(l2).min(r2);
            if k == 0 { h.min(behind(0.5 * STREAM_FT + 4.0)).min(behind(0.5 * STREAM_FT + 10.0)) } else { h }
        })
        .collect();
    let far: Vec<f64> = (0..pts.len())
        .map(|k| {
            let (l, r) = side(k, 0.5 * STREAM_FT + 19.0);
            l.max(r)
        })
        .collect();
    let mut w: Vec<f64> = Vec::with_capacity(g.len());
    let mut lo = f64::MAX;
    for &h in &g {
        lo = lo.min(h - 0.4);
        w.push(lo.max(level));
    }
    for _ in 0..2 {
        let prev = w.clone();
        for k in 1..w.len().saturating_sub(1) {
            w[k] = (prev[k - 1] + prev[k] + prev[k + 1]) / 3.0;
        }
    }
    // (Smoothed, still under its banks and falling all the way.)
    let mut lo = f64::MAX;
    for k in 0..w.len() {
        lo = lo.min(w[k]).min(g[k] - 0.3);
        w[k] = lo.max(level);
    }
    let n = w.len();
    *w.last_mut()? = level;
    // Falls at all, cuts no trench deeper than 3 ft (its banks, cut back three in ten, rise no more
    // than 2.5 ft more 19 ft out), never stands above its banks (but by the pond, whose banks are eased
    // to the water).
    let ok = w[0] - level >= 0.25
        && (0..n).all(|k| g[k] - w[k] <= 3.5 && far[k] - w[k] <= 7.0 && (dist(pts[k], mouth) < band + 6.0 || g[k] - w[k] >= 0.3));
    ok.then(|| w.iter().map(|&x| x as f32).collect())
}

/// A path crossing the stream goes straight and square across it there: each crossing (where it
/// is, and the way across) and the path with its points near it put in a line across.
/// A crossing within 35 ft of a bridge already there (`bridges`) isn't made: the path ends on that
/// bridge's end on its own bank instead (a Y onto the path that crosses there).
fn square_crossings(pts: &[P], stream: &[P], bridges: &[(P, P)]) -> (Vec<P>, Vec<(P, P)>) {
    let mut out: Vec<P> = Vec::new();
    let mut found: Vec<(P, P)> = Vec::new();
    let mut i = 0;
    while i < pts.len() {
        let hit = (i + 1 < pts.len())
            .then(|| stream.windows(2).find_map(|w| crossing(pts[i], pts[i + 1], w[0], w[1]).map(|t| (lerp(pts[i], pts[i + 1], t), unit_of(sub(w[1], w[0]))))))
            .flatten();
        let Some((q, sd)) = hit.filter(|(q, _)| found.iter().all(|f| dist(f.0, *q) > 30.0)) else {
            out.push(pts[i]);
            i += 1;
            continue;
        };
        if let Some(&(bq, bn)) = bridges.iter().find(|b| dist(b.0, q) < 35.0) {
            let side = if dot(sub(pts[i], bq), bn) >= 0.0 { 1.0 } else { -1.0 };
            let end = add(bq, mul(bn, 12.0 * side));
            out.push(pts[i]);
            while out.len() > 1 && dist(*out.last().unwrap(), end) < 8.0 {
                out.pop();
            }
            out.push(end);
            return (out, found);
        }
        let way = sub(pts[i + 1], pts[i]);
        let mut n = perp(sd);
        if dot(n, way) < 0.0 {
            n = mul(n, -1.0);
        }
        out.push(pts[i]);
        while out.len() > 1 && dist(*out.last().unwrap(), q) < 14.0 {
            out.pop();
        }
        out.push(sub(q, mul(n, 12.0)));
        out.push(add(q, mul(n, 12.0)));
        found.push((q, n));
        i += 1;
        while i + 1 < pts.len() && dist(pts[i], q) < 14.0 {
            i += 1;
        }
    }
    (out, found)
}

fn common(p: &Park, biome: Biome, city: bool, key: u64, rng: &mut Pcg32) -> ParkLayout {
    let poly = &p.pts;
    let g = &p.ground;
    let mut out = ParkLayout::default();
    let mut clear = Clear::new(poly);
    let o = obb(poly);
    let c = centroid(poly);
    let m = poly.len();
    let size = crate::core::sqrt(area(poly).abs());
    let (trees, shrubs, small, few) = plants(biome);
    out.bench = [0, 1, 1, 3][rng.below(4) as usize];
    let lamp = [0, 1, 2][rng.below(3) as usize];
    const PATH_W: f64 = 7.0;
    // The ground inside it (dry points of its grid at least 20 ft in): low, high, and how steep.
    let inside: Vec<(P, f64)> = (0..g.ny)
        .flat_map(|j| (0..g.nx).map(move |i| (i, j)))
        .filter(|&(i, j)| !g.wet[j * g.nx + i] && depth(poly, g.point(i, j)) >= 20.0)
        .map(|(i, j)| (g.point(i, j), g.h[j * g.nx + i] as f64))
        .collect();
    let lowest = inside.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
    let highest = inside.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1));
    let slope = match (lowest, highest) {
        (Some(lo), Some(hi)) if dist(lo.0, hi.0) > 20.0 => (hi.1 - lo.1) / dist(lo.0, hi.0),
        _ => 0.0,
    };
    let steep = slope > 0.1;

    // The hub, a little off the middle.
    let mut hub = add(c, [(rng.next_f64() - 0.5) * 0.15 * o.short, (rng.next_f64() - 0.5) * 0.15 * o.short]);
    // The pond: in the lowest hollow that holds one (its level a little below the lowest ground
    // round its rim; the ground further round no more than 7 ft above it, its banks eased out far
    // enough to be no steeper than one in two), clear of the water.
    let rp = (0.13 * size).clamp(16.0, 40.0);
    let shape = (rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::PI), rng.range(1.1, 1.4));
    // The pond's rim radius at angle `t`: an ellipse (`el` to 1 / `el` across, along `ax`), wobbled.
    let rim = move |t: f64| {
        let (a, b, ax, el) = shape;
        let (cs, sn) = (libm::cos(t - ax), libm::sin(t - ax));
        let ellipse = 1.0 / crate::core::sqrt(cs * cs / (el * el) + sn * sn * el * el);
        rp * (1.0 + 0.14 * libm::sin(2.0 * t + a) + 0.08 * libm::sin(3.0 * t + b)) * ellipse
    };
    let rmax = (0..32).map(|k| rim(std::f64::consts::TAU * k as f64 / 32.0)).fold(0.0, f64::max);
    let ring = |pc: P, f: f64| -> Vec<f64> {
        (0..16)
            .map(|k| {
                let t = std::f64::consts::TAU * k as f64 / 16.0;
                g.at(add(pc, mul([libm::cos(t), libm::sin(t)], f * rim(t))))
            })
            .collect()
    };
    let mut pond: Option<(P, f64, f64)> = None;
    if !steep && o.short >= 140.0 && rng.next_f64() < 0.85 {
        let mut cands: Vec<(f64, P)> = inside.iter().filter(|&&(q, _)| depth(poly, q) >= rmax + 34.0).map(|&(q, _)| (ring(q, 1.2).iter().sum::<f64>() / 16.0, q)).collect();
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1[0].total_cmp(&b.1[0])).then(a.1[1].total_cmp(&b.1[1])));
        for &(_, q) in cands.iter().take(24) {
            // (A little below the lowest ground round it, near or further out: no water stands over
            // ground that falls away past a thin bank.)
            // (On level ground it is dug in, the ground round it 6 ft above its water: its banks then
            // rise a tier, where no knoll may. The ground at its band's edge and 20 ft past it no more
            // than 7 ft over the water: no pond in a pit.)
            let past = |off: f64| -> Vec<f64> {
                (0..16)
                    .map(|k| {
                        let t = std::f64::consts::TAU * k as f64 / 16.0;
                        g.at(add(q, mul([libm::cos(t), libm::sin(t)], rim(t) + off)))
                    })
                    .collect()
            };
            let band = POND_BAND;
            // (And every point of the planned ground out to there: the rings alone may miss a rise.)
            let rmin = (0..32).map(|k| rim(std::f64::consts::TAU * k as f64 / 32.0)).fold(f64::MAX, f64::min);
            let round = (0..g.ny).flat_map(|j| (0..g.nx).map(move |i| (i, j))).filter(|&(i, j)| (rmin..=rmax + band + 20.0).contains(&dist(g.point(i, j), q))).map(|(i, j)| g.h[j * g.nx + i] as f64);
            let hi = past(band).iter().chain(past(band + 20.0).iter()).copied().chain(round).fold(f64::MIN, f64::max);
            let level = ([1.05, 1.5, 2.0, 2.6].iter().flat_map(|&f| ring(q, f)).fold(f64::MAX, f64::min) - 0.3).min(hi - 6.0);
            let above = hi - level;
            // (Its banks well in from the park's edge: past it the drawn ground may stand otherwise.)
            if above <= 7.0 && depth(poly, q) >= rmax + band + 24.0 && !g.wet_near(q, rmax + band + 10.0) {
                pond = Some((q, level, band));
                break;
            }
        }
    }
    // (The hub's round wholly inside, 4 ft to spare: else the deepest point inside.)
    if depth(poly, hub) < 17.0
        && let Some(&(q, _)) = inside.iter().max_by(|a, b| depth(poly, a.0).total_cmp(&depth(poly, b.0)).then(b.0[0].total_cmp(&a.0[0])).then(b.0[1].total_cmp(&a.0[1])))
    {
        hub = q;
    }
    // The hub stands clear of it, its banks and 18 ft more: moved straight away from it, else round it
    // (the nearest way first) to where it is 20 ft or more inside; with nowhere, no pond.
    if let Some((pc, _, band)) = pond
        && dist(hub, pc) < rmax + band + 18.0
    {
        let away = sub(hub, pc);
        let a0 = if len(away) > 1e-6 { libm::atan2(away[1], away[0]) } else { 0.0 };
        let reach = rmax + band + 18.0;
        let moved = (0..16).map(|k| a0 + std::f64::consts::TAU / 16.0 * if k % 2 == 0 { (k / 2) as f64 } else { -((k + 1) / 2) as f64 });
        match moved.map(|t| add(pc, mul([libm::cos(t), libm::sin(t)], reach))).find(|&q| depth(poly, q) >= 20.0) {
            Some(q) => hub = q,
            None => pond = None,
        }
    }
    let pond_c = pond.map(|x| x.0);
    if let Some((pc, level, band)) = pond {
        let pts: Vec<P> = (0..28)
            .map(|k| {
                let t = std::f64::consts::TAU * k as f64 / 28.0;
                add(pc, mul([libm::cos(t), libm::sin(t)], rim(t)))
            })
            .collect();
        out.pond = Some(Pond { pts, centre: pc, level: level as f32, band });
    }
    let band = pond.map_or(0.0, |x| x.2);
    // Out of the pond: a point inside it (or within `margin` of its rim) pushed out to the rim and
    // the margin.
    let off_pond = |q: P, margin: f64| -> P {
        let Some(pc) = pond_c else { return q };
        let d = sub(q, pc);
        let t = libm::atan2(d[1], d[0]);
        let r = rim(t) + margin;
        if len(d) < r { add(pc, mul(unit_of(d), r)) } else { q }
    };
    // The gazebo, or a city's bandstand now and then: on a raised tier on the high ground, or on a
    // slope at the top, no tier.
    let stand = if rng.next_f64() < if city { 0.45 } else { 0.2 } { Kind::Bandstand } else { Kind::Gazebo };
    let rm = foot(stand) + 5.0;
    let mut tops: Vec<(P, f64)> = inside.clone();
    tops.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0[0].total_cmp(&b.0[0])).then(a.0[1].total_cmp(&b.0[1])));
    let stand_at = (o.short >= 120.0)
        .then(|| {
            tops.iter().map(|t| t.0).find(|&q| {
                depth(poly, q) >= rm + if steep { 12.0 } else { 22.0 } && dist(q, hub) >= rm + 30.0 && pond_c.is_none_or(|pc| dist(q, pc) >= rmax + band + rm + 20.0)
            })
        })
        .flatten();
    // (A raised tier only on gentle ground: on a slope it would stand out of the hill as a cliff.)
    let gentle = |q: P| {
        let hs: Vec<f64> = (0..12).map(|k| std::f64::consts::TAU * k as f64 / 12.0).map(|t| g.at(add(q, mul([libm::cos(t), libm::sin(t)], rm + 20.0)))).collect();
        hs.iter().copied().fold(f64::MIN, f64::max) - hs.iter().copied().fold(f64::MAX, f64::min) <= 5.0
    };
    let mound = stand_at.filter(|&q| !steep && gentle(q));
    let off_mound = |q: P, margin: f64| -> P {
        let Some(mc) = stand_at else { return q };
        let d = sub(q, mc);
        if len(d) < rm + margin { add(mc, mul(unit_of(d), rm + margin)) } else { q }
    };
    // A winding path from `a` to `b`: a line every 16 ft pushed across by low noise (most in the
    // middle), out of the pond and the tier, kept inside, then smoothed.
    let nseed = key ^ 0x7a17;
    let wind = |a: P, b: P, amp: f64, margin: f64| -> Vec<P> {
        let l = dist(a, b);
        let n = (crate::core::ceil(l / 16.0) as usize).max(2);
        let nrm = perp(unit_of(sub(b, a)));
        let pts: Vec<P> = (0..=n)
            .map(|k| {
                let t = k as f64 / n as f64;
                let base = lerp(a, b, t);
                if k == 0 || k == n {
                    return base;
                }
                let push = amp * libm::sin(std::f64::consts::PI * t) * fbm(nseed, base[0] / 150.0, base[1] / 150.0, 2, 2.0, 0.5) * 1.6;
                let mut q = off_mound(off_pond(add(base, mul(nrm, push)), margin), margin + 8.0);
                if depth(poly, q) < 10.0 {
                    q = lerp(q, c, 0.15);
                }
                q
            })
            .collect();
        chaikin(&pts, 2).iter().map(|&q| off_pond(q, margin - 2.0)).collect()
    };
    // Ways in from the street sides only: the middle of the longest (up to four).
    let mut sides: Vec<(f64, usize)> = (0..m).map(|e| (dist(poly[e], poly[(e + 1) % m]), e)).filter(|s| s.0 >= 40.0 && p.fronts.get(s.1).copied().unwrap_or(false)).collect();
    sides.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let ways = (2 + rng.below(3) as usize).min(sides.len());
    let mut entrances: Vec<P> = Vec::new();
    for &(_, e) in sides.iter().take(ways) {
        let at = lerp(poly[e], poly[(e + 1) % m], rng.range(0.35, 0.65));
        if entrances.iter().all(|q| dist(*q, at) > 60.0) {
            entrances.push(at);
        }
    }
    let mut paths: Vec<Vec<P>> = Vec::new();
    // A big one's walk round, a little in from its edge.
    if o.short >= 220.0 {
        let belt = inset(poly, &vec![26.0; m]);
        if belt.len() >= 3 {
            let mut pts: Vec<P> = Vec::new();
            for e in 0..belt.len() {
                let (a, b) = (belt[e], belt[(e + 1) % belt.len()]);
                let n = (crate::core::ceil(dist(a, b) / 18.0) as usize).max(1);
                for k in 0..n {
                    let q = lerp(a, b, k as f64 / n as f64);
                    let w = fbm(nseed ^ 0xbe17, q[0] / 120.0, q[1] / 120.0, 2, 2.0, 0.5) * 14.0;
                    let toward = unit_of(sub(c, q));
                    pts.push(off_mound(off_pond(add(q, mul(toward, w)), 9.0), 10.0));
                }
            }
            pts.push(pts[0]);
            paths.push(chaikin(&pts, 2));
        }
    }
    // From each way in toward the hub, joining an earlier path where it comes within 40 ft of one
    // (away from the hub): a Y, not a star.
    let mut spokes: Vec<usize> = Vec::new();
    for &e in &entrances {
        let full = wind(e, hub, (0.35 * dist(e, hub)).min(60.0), 9.0);
        let mut cut = None;
        let mut run = 0.0;
        for k in 1..full.len() {
            run += dist(full[k - 1], full[k]);
            if run < 30.0 || dist(full[k], hub) < 50.0 {
                continue;
            }
            let near = spokes.iter().map(|&s| nearest_on(&paths[s], full[k])).filter(|x| x.1 < 40.0).min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((q, _)) = near {
                cut = Some((k, q));
                break;
            }
        }
        let pts = match cut {
            Some((k, q)) => {
                let mut pts = full[..=k].to_vec();
                pts.push(q);
                pts
            }
            None => full,
        };
        spokes.push(paths.len());
        paths.push(pts);
    }
    // The way up to the stand, coming to its steps (on its south side) from the south.
    if let Some(mc) = stand_at {
        let steps = add(mc, [0.0, foot(stand) + 2.5]);
        let below = add(mc, [0.0, rm + 12.0]);
        let mut pts = wind(hub, if depth(poly, below) > 8.0 { below } else { steps }, 20.0, 9.0);
        pts.push(steps);
        paths.push(pts);
        out.reserved.push((mc, foot(stand) + 1.0));
    }
    // A steep common with no stand: a viewpoint at its top (a bench looking downhill), the path to it
    // climbing across the slope by a point to one side.
    let view = (steep && stand_at.is_none()).then(|| tops.iter().map(|t| t.0).find(|&q| depth(poly, q) >= 14.0 && dist(q, hub) >= 40.0)).flatten();
    if let Some(v) = view {
        let down = lowest.map_or([0.0, 1.0], |lo| unit_of(sub(lo.0, v)));
        let end = add(v, mul(down, 7.0));
        let side = add(lerp(hub, end, 0.5), mul(perp(unit_of(sub(end, hub))), 0.3 * dist(hub, end)));
        let side = if depth(poly, side) > 14.0 { side } else { lerp(hub, end, 0.5) };
        let mut pts = wind(hub, side, 8.0, 9.0);
        pts.extend(wind(side, end, 8.0, 9.0).into_iter().skip(1));
        paths.push(pts);
        clear.disc(v, foot(Kind::Bench));
        out.props.push((v, facing(down), Kind::Bench, out.bench, 1.0));
    }
    // A walk round the pond's near side, both ends joined to the nearest path.
    if let Some(pc) = pond_c {
        let toward = sub(hub, pc);
        let t0 = libm::atan2(toward[1], toward[0]);
        let walk: Vec<P> = (0..=22)
            .map(|k| {
                let t = t0 + (k as f64 / 22.0 - 0.5) * 3.6;
                add(pc, mul([libm::cos(t), libm::sin(t)], rim(t) + 10.0))
            })
            .filter(|&q| depth(poly, q) >= 8.0)
            .collect();
        if walk.len() >= 6 {
            let walk = chaikin(&walk, 1);
            let ends = [walk[0], walk[walk.len() - 1]];
            let others: Vec<Vec<P>> = paths.clone();
            paths.push(walk);
            for end in ends {
                match others.iter().map(|pts| nearest_on(pts, end)).min_by(|a, b| a.1.total_cmp(&b.1)) {
                    Some((_, d)) if d <= 3.0 => {}
                    Some((q, d)) if d < 90.0 => paths.push(wind(end, q, 6.0, 7.0)),
                    _ => paths.push(wind(end, hub, 12.0, 9.0)),
                }
            }
        }
    }
    // A stream down to the pond now and then, from a spring at least 30 ft in from the edge, the
    // way (of seven, away from the hub) whose ground falls most; its head cut back until its spring
    // stands 3 ft or more off the paths and a path crossing it does so 12 ft or more below its head.
    let path_gap = |q: P| paths.iter().map(|pts| nearest_on(pts, q).1).fold(f64::MAX, f64::min) - 0.5 * PATH_W;
    if let Some((pc, level, band)) = pond
        && rng.next_f64() < 0.6
    {
        let away = sub(pc, hub);
        let a0 = libm::atan2(away[1], away[0]);
        let mut best: Option<(f64, Vec<(P, f32)>)> = None;
        for k in 0..7 {
            let t = a0 + (k as f64 - 3.0) * 0.32;
            let d = [libm::cos(t), libm::sin(t)];
            let mouth = add(pc, mul(d, rim(t) - 4.0));
            let mut head = mouth;
            while depth(poly, add(head, mul(d, 5.0))) >= 30.0 && dist(head, mouth) < 260.0 {
                head = add(head, mul(d, 5.0));
            }
            if dist(head, mouth) < 50.0 || g.wet_near(head, 30.0) {
                continue;
            }
            let n = (crate::core::ceil(dist(head, mouth) / 14.0) as usize).max(2);
            let nrm = perp(d);
            let raw: Vec<P> = (0..=n)
                .map(|j| {
                    let f = j as f64 / n as f64;
                    let base = lerp(head, mouth, f);
                    off_mound(add(base, mul(nrm, 10.0 * libm::sin(std::f64::consts::PI * f) * fbm(nseed ^ 0x51, base[0] / 70.0, base[1] / 70.0, 2, 2.0, 0.5))), 12.0)
                })
                .collect();
            let mut pts = chaikin(&raw, 2);
            // (And until it rises at the highest ground along it: nothing upstream of its spring climbs
            // above it.)
            while pts.len() > 2 {
                let spring = sub(pts[0], mul(unit_of(sub(pts[1], pts[0])), 0.5 * STREAM_FT + 4.0));
                let top = pts[1..].iter().map(|&q| g.at(q)).fold(f64::MIN, f64::max);
                if path_gap(pts[0]) >= 12.0 && path_gap(spring) >= 6.0 + 3.0 && top <= g.at(pts[0]) + 0.2 {
                    break;
                }
                pts.remove(0);
            }
            if length(&pts) < 40.0 {
                continue;
            }
            let Some(w) = stream_levels(&pts, g, level, band, &out.pond.as_ref().map_or(Vec::new(), |pd| pd.pts.clone()), mouth) else { continue };
            let fall = w[0] as f64 - level;
            if best.as_ref().is_none_or(|b| fall > b.0) {
                best = Some((fall, pts.into_iter().zip(w).collect()));
            }
        }
        if let Some((_, s)) = best {
            out.stream = s;
            // Its spring: stones round where it rises, reeds by them.
            // (Just behind its head, on the dry.)
            let (head, next) = (out.stream[0].0, out.stream[1].0);
            let spring = sub(head, mul(unit_of(sub(next, head)), 0.5 * STREAM_FT + 4.0));
            out.props.push((spring, 0.0, Kind::RockPile, rng.below(4) as u8, 1.1));
            clear.disc(spring, 6.0);
        }
    }
    // Where a path crosses the stream it goes straight across on a footbridge, square to the water.
    let stream: Vec<P> = out.stream.iter().map(|s| s.0).collect();
    if stream.len() >= 2 {
        let mut bridges: Vec<(P, P)> = Vec::new();
        for pts in &mut paths {
            let (squared, found) = square_crossings(pts, &stream, &bridges);
            *pts = squared;
            for (q, n) in found {
                out.props.push((q, along(n), Kind::Footbridge, [0, 0, 2, 3][rng.below(4) as usize], 1.05));
                clear.disc(q, 3.0);
                bridges.push((q, n));
            }
        }
        for s in stream.windows(2) {
            clear.seg(s[0], s[1], 0.5 * STREAM_FT + 2.0);
        }
    }
    for pts in &paths {
        clear.path(pts, PATH_W);
    }
    out.paths = paths.into_iter().map(|pts| (pts, PATH_W)).collect();
    // The pond and the stand, kept clear.
    if let Some(pc) = pond_c {
        let r = (0..16).map(|k| rim(std::f64::consts::TAU * k as f64 / 16.0)).fold(0.0, f64::max);
        clear.disc(pc, r + 2.0);
    }
    if let Some(mc) = stand_at {
        out.mound = mound.map(|mc| (mc, rm));
        out.props.push((mc, 0.0, stand, rng.below(4) as u8, 1.0));
        clear.disc(mc, rm + 3.0);
        // On a slope's top, a bench beside it looking downhill.
        if steep {
            let down = lowest.map_or([0.0, 1.0], |lo| unit_of(sub(lo.0, mc)));
            let b = add(mc, mul(down, rm + 4.0));
            if depth(poly, b) >= 6.0 && clear.free(b, foot(Kind::Bench), 1.0) {
                clear.disc(b, foot(Kind::Bench));
                out.props.push((b, facing(down), Kind::Bench, out.bench, 1.0));
            }
        }
    }
    // The hub: a fountain or a statue in a gravel round, benches round it.
    let rh = 13.0;
    out.rounds.push((hub, rh));
    if rng.next_f64() < 0.55 {
        out.props.push((hub, 0.0, Kind::Fountain, [0, 1, 3][rng.below(3) as usize], 1.1));
    } else {
        out.props.push((hub, 0.0, Kind::GardenStatue, [1, 3, 0][rng.below(3) as usize], 1.0));
    }
    clear.disc(hub, rh - 1.0);
    let mut round_benches = 0;
    for k in 0..6 {
        let t = std::f64::consts::TAU * (k as f64 + rng.next_f64() * 0.3) / 6.0;
        let d = [libm::cos(t), libm::sin(t)];
        let b = add(hub, mul(d, rh + 3.5));
        if round_benches < 4 && clear.path_gap(b) > foot(Kind::Bench) + 1.0 && clear.free(b, foot(Kind::Bench), 2.0) {
            clear.disc(b, foot(Kind::Bench));
            out.props.push((b, facing(mul(d, -1.0)), Kind::Bench, out.bench, 1.0));
            round_benches += 1;
        }
    }
    // Reeds along the pond's far side, in the shallows.
    if let Some(pc) = pond_c {
        let away = sub(pc, hub);
        let t0 = libm::atan2(away[1], away[0]) + rng.range(-0.8, 0.8);
        let span = rng.range(1.6, 2.6);
        let n = (span * rp / 6.0) as usize;
        for k in 0..=n {
            let t = t0 + (k as f64 / n.max(1) as f64 - 0.5) * span;
            let q = add(pc, mul([libm::cos(t), libm::sin(t)], rim(t) - 2.5 - 2.0 * jit(key, [t, 0.0], 5)));
            out.props.push((q, rng.range(0.0, std::f64::consts::TAU) as f32, Kind::Reeds, rng.below(4) as u8, rng.range(0.8, 1.2) as f32));
        }
    }
    // Benches along the paths, by turns either side, facing them; a city's lamps between.
    for (pi, (pts, w)) in out.paths.clone().iter().enumerate() {
        for (k, (q, d)) in every(pts, rng.range(40.0, 55.0), 22.0, 14.0).into_iter().enumerate() {
            if dist(q, hub) < rh + 14.0 {
                continue;
            }
            let side = perp(d);
            let s = if (k + pi) % 2 == 0 { 1.0 } else { -1.0 };
            let b = add(q, mul(side, s * (0.5 * w + foot(Kind::Bench) + 1.3)));
            if clear.free(b, foot(Kind::Bench), 3.0) {
                clear.disc(b, foot(Kind::Bench));
                out.props.push((b, facing(mul(side, -s)), Kind::Bench, out.bench, 1.0));
            }
            if city && k % 2 == 1 {
                let l = add(q, mul(side, -s * (0.5 * w + foot(Kind::LampPost) + 1.3)));
                if clear.free(l, foot(Kind::LampPost), 3.0) {
                    clear.disc(l, foot(Kind::LampPost));
                    out.props.push((l, 0.0, Kind::LampPost, lamp, 1.0));
                }
            }
        }
    }
    // A few beds round the hub's round.
    for k in 0..2 + rng.below(3) as usize {
        let t = std::f64::consts::TAU * (k as f64 + 0.5) / 4.0 + rng.range(-0.3, 0.3);
        let d = [libm::cos(t), libm::sin(t)];
        let f = add(hub, mul(d, rh + 9.0));
        if clear.free(f, 0.8 * foot(Kind::Flowerbed), 3.0) {
            clear.disc(f, 0.8 * foot(Kind::Flowerbed));
            out.props.push((f, along(perp(d)), Kind::Flowerbed, [0, 1, 2][rng.below(3) as usize], 1.0));
        }
    }
    // Broad low swells, two or more by its size (none on a slope: it has its own), clear of the pond's
    // banks, the stream, the stand and the hub: the common rolls rather than lies flat.
    if !steep {
        let (lo, hi) = bbox_of(poly);
        let want = ((area(poly).abs() / 40000.0) as usize).clamp(2, 7) + rng.below(2) as usize;
        for _ in 0..8 * want {
            if out.swells.len() >= want {
                break;
            }
            // (Some longer one way than the other, now and then one lower, under a tier: not all alike.)
            let t = rng.range(0.0, std::f64::consts::PI);
            let sw = Swell {
                at: [rng.range(lo[0], hi[0]), rng.range(lo[1], hi[1])],
                r: rng.range(26.0, 44.0),
                rise: if rng.next_f64() < 0.25 { rng.range(2.0, 4.0) } else { rng.range(4.5, 6.5) } as f32,
                axis: [libm::cos(t), libm::sin(t)],
                el: rng.range(1.0, 1.5),
            };
            let (q, r) = (sw.at, sw.reach());
            // (Wholly inside, 4 ft to spare: the verge by the street keeps its ground.)
            let ok = depth(poly, q) >= r + 4.0
                && dist(q, hub) >= r + 16.0
                && pond_c.is_none_or(|pc| dist(q, pc) >= rmax + band + r + 20.0)
                && stand_at.is_none_or(|mc| dist(q, mc) >= rm + r + 14.0)
                && (stream.len() < 2 || nearest_on(&stream, q).1 >= r + 12.0)
                && out.swells.iter().all(|s| dist(s.at, q) >= 0.8 * (s.reach() + r))
                && !g.water_near(q, r + 10.0);
            if ok {
                out.swells.push(sw);
            }
        }
    }
    // Trees: clumps of four to eight on a jittered grid, glades between, shrubs at their edges, a
    // few specimen trees in the open; a loose belt along the edge. Where the biome's trees are few,
    // a handful in all.
    let pick = |rng: &mut Pcg32, kinds: &[Kind]| kinds[rng.below(kinds.len() as u32) as usize];
    let mut budget = if few { ((area(poly).abs() / 20000.0) as usize).clamp(1, 4) } else { usize::MAX };
    let ([x0, y0], [x1, y1]) = bbox_of(poly);
    // (On the world's grid of clump cells, each with rolls of its own: a corner moved keeps the
    // clumps away from it.)
    const CLUMP: f64 = 75.0;
    let cell = |v: f64| crate::core::floor(v / CLUMP) as i64;
    for j in cell(y0)..=cell(y1) {
        for i in cell(x0)..=cell(x1) {
            let rng = &mut Pcg32::new(hash3(key ^ 0xc1a3, i, j, 7), 41);
            let at = [(i as f64 + rng.next_f64()) * CLUMP, (j as f64 + rng.next_f64()) * CLUMP];
            if rng.next_f64() > 0.5 || depth(poly, at) < 14.0 {
                continue;
            }
            let kind = pick(rng, trees);
            for _ in 0..4 + rng.below(5) {
                let q = add(at, [rng.range(-14.0, 14.0), rng.range(-14.0, 14.0)]);
                let r = 0.45 * foot(kind);
                if budget > 0 && clear.free(q, r, 0.8 * foot(kind)) {
                    clear.disc(q, r);
                    out.props.push((q, 0.0, kind, rng.below(8) as u8, rng.range(0.9, 1.2) as f32));
                    budget -= 1;
                }
            }
            for _ in 0..1 + rng.below(3) {
                let kind = pick(rng, shrubs);
                let t = rng.range(0.0, std::f64::consts::TAU);
                let q = add(at, mul([libm::cos(t), libm::sin(t)], rng.range(15.0, 21.0)));
                if clear.free(q, foot(kind), 3.0) {
                    clear.disc(q, foot(kind));
                    out.props.push((q, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8, 1.0));
                }
            }
            if !small.is_empty() && rng.next_f64() < 0.5 {
                let kind = pick(rng, small);
                let q = add(at, [rng.range(-24.0, 24.0), rng.range(-24.0, 24.0)]);
                if clear.free(q, foot(kind), 3.0) {
                    clear.disc(q, foot(kind));
                    out.props.push((q, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8, 1.0));
                }
            }
        }
    }
    // A few specimen trees in the open, on the world's grid of 120-ft cells (one in four holds one),
    // each with rolls of its own.
    const SPECIMEN: f64 = 120.0;
    let scell = |v: f64| crate::core::floor(v / SPECIMEN) as i64;
    for j in scell(y0)..=scell(y1) {
        for i in scell(x0)..=scell(x1) {
            let rng = &mut Pcg32::new(hash3(key ^ 0x5be7, i, j, 11), 43);
            let q = [(i as f64 + rng.next_f64()) * SPECIMEN, (j as f64 + rng.next_f64()) * SPECIMEN];
            let kind = pick(rng, trees);
            let r = foot(kind) * 1.25;
            if rng.next_f64() < 0.25 && budget > 0 && depth(poly, q) >= r && clear.free(q, r + 6.0, 4.0) {
                clear.disc(q, r);
                out.props.push((q, 0.0, kind, rng.below(8) as u8, 1.3));
                budget -= 1;
            }
        }
    }
    // A loose belt of trees 11 ft in from the edge: where it passes through each 26-ft cell of the
    // world, at its point nearest the cell's own mark, by the cell's own rolls (a corner moved keeps
    // the belt's trees elsewhere).
    let belt = inset(poly, &vec![11.0; m]);
    if belt.len() >= 3 && !few {
        let mut pts = belt.clone();
        pts.push(belt[0]);
        const BELT: f64 = 26.0;
        let mark = |c: (i64, i64)| {
            let h = hash3(key ^ 0xbe17, c.0, c.1, 13);
            [(c.0 as f64 + 0.15 + 0.7 * unit(h)) * BELT, (c.1 as f64 + 0.15 + 0.7 * unit(h >> 21)) * BELT]
        };
        let mut at: std::collections::BTreeMap<(i64, i64), P> = std::collections::BTreeMap::new();
        for (q, _) in every(&pts, 2.0, 0.0, 0.0) {
            let c = (crate::core::floor(q[0] / BELT) as i64, crate::core::floor(q[1] / BELT) as i64);
            let e = at.entry(c).or_insert(q);
            if dist(q, mark(c)) < dist(*e, mark(c)) {
                *e = q;
            }
        }
        for (&(ci, cj), &q) in &at {
            let rng = &mut Pcg32::new(hash3(key ^ 0xbe18, ci, cj, 17), 47);
            let kind = pick(rng, trees);
            let r = 0.75 * foot(kind);
            if rng.next_f64() < 0.3 && clear.free(q, r, 3.0) {
                clear.disc(q, r);
                out.props.push((q, 0.0, kind, rng.below(8) as u8, rng.range(0.9, 1.15) as f32));
            }
        }
    }
    unseal(poly, &mut out, 0);
    out
}
