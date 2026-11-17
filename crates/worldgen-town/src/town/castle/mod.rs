//! Castles: a curtain wall along the brow of the ground round its highest point (where the ground
//! falls away from the top, inside the castle's block), towers at its turns, round or (in some
//! castles) square, a gatehouse on the side facing the way in; in a big one an inner curtain round
//! the keep, an inner bailey inside an outer. The keep stands on the high point, its footprint
//! made of pieces (`Building::parts`): a great tower with corner towers, a hall with a
//! forebuilding, an L or a T with towers at its outer corners, or a round shell keep on a motte.
//! Ranges of buildings stand against the inside of the outer curtain (or of the town wall where that
//! is the castle's).
//!
//! The keep's pieces are laid on the 5-ft grid of its body (whole squares from the body's corner),
//! so its interior's squares meet its walls and its towers' rooms come out round about their middle
//! (`interior::Shell::of`).

use super::catalog::Ward;
use super::geom::*;
use super::{Building, District, Ground, Layout, Motte, Part, PartKind, Plaza, PlazaKind, Structure, wall_pieces};
use crate::core::rng::Pcg32;
use super::wallstyle::{TowerShape, WallStyle};

mod bound;
mod walls;
mod keep;
mod yard;

use bound::*;
use walls::*;
pub use keep::*;
pub(super) use yard::*;

/// The curtain stands this far in from the block's edge (ft).
pub(super) const CURTAIN_IN: f64 = 6.0;
/// Ground with less relief than this over the castle is flat: the curtain keeps to the block.
pub(super) const FLAT_FT: f64 = 5.0;
/// The drops below the top (ft) a curtain may follow, nearest the top first.
const DROPS: [f64; 6] = [6.0, 9.0, 12.0, 16.0, 22.0, 30.0];
/// A curtain along the brow stands back this far from a river's channel (ft), as a block's quay
/// does: clear of the river where walls are cut.
const WATER_BACK: f64 = 16.0;
/// The least distance from the high point to the curtain (ft): room for a keep and a yard.
const R_MIN: f64 = 48.0;
/// The least ground inside a castle's curtain (sq ft): a keep and a yard round it.
const MIN_BOUND_SQFT: f64 = 2500.0;
/// A curtain enclosing this much (sq ft) and this wide (ft) may get an inner curtain round the
/// keep: a capital's; else a town's as its patch was sized for one (`Setting::inner`), or enclosing
/// `TOWN_INNER` where it has the whole patch, one drawn by hand `HAND_INNER` and 200 ft across.
const BIG_AREA: f64 = 30_000.0;
const BIG_WIDTH: f64 = 160.0;
const TOWN_INNER: f64 = 170_000.0;
const HAND_INNER: f64 = 80_000.0;
/// The outer bailey's least width between the two curtains (ft), and its width on the gate's
/// side, where it is broad (a yard to use).
const BAILEY_GAP: f64 = 48.0;
const BROAD: f64 = 80.0;
/// The inner gate stands at least this far from the outer one (ft): the way between them runs
/// along the outer bailey, not straight through both.
const GATES_APART: f64 = 90.0;
/// The gate passage's half width between its towers, as `wall_pieces` lays it (ft).
const GATE_HALF: f64 = 26.0;
/// A side of the castle this near a town wall (ft), and running along it, is that wall.
const WALL_SIDE: f64 = 9.0;
/// A gate tower stands this far from any other tower (ft, centre to centre), where a gate fits so.
const GATE_CLEAR: f64 = 34.0;
/// A castle's gate stands this far from a town gate (ft), where it fits so.
const GATE_AWAY: f64 = 90.0;
/// Towers of a castle and of the town stand this far clear of each other (ft, wall to wall).
const TOWERS_CLEAR: f64 = 12.0;
/// A corner of a castle's ring sharper than this (degrees) is cut back where it is this wide (ft):
/// no wedge of yard between two walls closing in on each other.
const SPIKE_DEG: f64 = 40.0;
const SPIKE_WIDE: f64 = 60.0;

/// How a castle is laid out.
#[derive(Clone, Copy, Debug)]
pub(super) struct Setting {
    /// The curtain follows the brow of the ground (a town's castle); else it keeps to the outline
    /// (one drawn by hand: only its keep and an inner curtain go to the high ground).
    pub follow: bool,
    /// A capital's castle: an inner curtain wherever one fits.
    pub capital: bool,
    /// The town wall stands along the sides given as on it (else they get a curtain of their own).
    pub wall_up: bool,
    /// A castle the town planned (not one set by hand in its place): a town tower crowding its
    /// gatehouse goes (a hand castle's leave every `t:` id as it was).
    pub planned: bool,
    /// An inner curtain where it fits (a town's, its patch cut to a size for one: `cut_bound`);
    /// None: by its size (a town's on its whole patch, one drawn by hand).
    pub inner: Option<bool>,
    /// A great castle's (a capital's, a metropolis's): a hall range in its inner bailey (else along
    /// the curtain) before the other ranges.
    pub hall: bool,
    /// A moat round the outer curtain (v63 castles: `moat`): a town's castle's curtain stands its
    /// width further in from the block.
    pub moat: Option<super::moat::Plan>,
}

/// (Roadmap 6 P7) What a castle drawn by hand asks for (`world::Created`): its gates, its keep's
/// shape and the buildings drawn by hand in its yard.
#[derive(Clone, Debug, Default)]
pub(super) struct Asked {
    /// Where its gates go (local ft: the middles of their sides), the main one first; empty: the
    /// side facing `gate_toward`.
    pub gates: Vec<P>,
    pub keep: KeepShape,
    /// Footprints drawn by hand in its yard (local ft): its own buildings stand clear of them.
    pub drawn: Vec<Vec<P>>,
    /// What came of it: written as the castle is built (`sites::castle_report`).
    pub report: std::cell::RefCell<Report>,
}

/// (Roadmap 6 P7) What of a hand castle's asks could not be built, and why.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Gates not built: (their place in `Asked::gates`, why).
    pub gates: Vec<(usize, String)>,
    /// What a drawn keep stands on that it must not (a tower, a gate, a building drawn in the yard).
    pub keep: Option<String>,
}

/// A hand castle's keep's shape (`world::KEEP_SHAPES`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum KeepShape {
    /// Its kind rolled (now and then a shell keep on a motte).
    #[default]
    Rolled,
    /// Its kind rolled among the keeps of pieces (never a shell keep).
    Square,
    /// A round great tower.
    Round,
    /// Its outline drawn (local ft).
    Drawn(Vec<P>),
}

impl KeepShape {
    /// The shape named ("square", "round"; else rolled).
    pub(super) fn of(name: Option<&str>) -> KeepShape {
        match name {
            Some("square") => KeepShape::Square,
            Some("round") => KeepShape::Round,
            _ => KeepShape::Rolled,
        }
    }
}

/// How far a castle's own buildings stand from one drawn by hand in its yard (ft).
const DRAWN_CLEAR: f64 = 8.0;
/// Gates on a hand castle's curtain this far apart at least (their towers clear of each other).
const HAND_GATES_APART: f64 = 2.0 * GATE_HALF + 24.0;

/// How far apart two polygons are (ft; 0 where they touch or overlap).
fn apart_by(a: &[P], b: &[P]) -> f64 {
    let m = b.len();
    (0..m).map(|k| super::poly_seg_dist(a, b[k], b[(k + 1) % m])).fold(f64::MAX, f64::min).min(if a.iter().any(|p| contains(b, *p)) { 0.0 } else { f64::MAX })
}

/// A concave outline drawn in by `d` (each side its own): its largest piece, anticlockwise, its
/// corners a hair apart or in a line made one.
fn inset_concave(block: &[P], d: &[f64]) -> Vec<P> {
    let Some(s) = super::clip::inset_edges(block, d).into_iter().max_by(|a, b| a.area().abs().total_cmp(&b.area().abs())) else {
        return Vec::new();
    };
    let mut r = simplify_closed(&s.outer, 0.25);
    if area(&r) < 0.0 {
        r.reverse();
    }
    r
}

/// A castle on `block` (local ft; convex, but a hand castle's, `asked`): its curtain (see the module), gate facing `gate_toward`
/// (in a town, on the side nearest a street, facing the centre among those); where a side runs
/// along a town wall already standing, that wall is the castle's on that side. `laid`: the town
/// wall's stretches and towers as laid out, standing or not (its gate and way keep clear of them
/// either way: the same castle with the wall taken down). With `keep`, the
/// keep (4 storeys, pushed last), and with `yard`, ranges of buildings against the inside of the
/// curtain, clear of the keep and of the way from gate to gate. Its bailey is an open yard
/// (`PlazaKind::Bailey`). Its outer curtain's line.
#[allow(clippy::too_many_arguments)]
pub(super) fn castle<G: Ground>(
    site: &G,
    block_in: &[P],
    gate_toward: P,
    w: Ward,
    _dp: &District,
    set: Setting,
    town_wall: &[(P, P)],
    town_gates: &[P],
    laid: (&[(P, P)], &[(P, f64)]),
    prng: &mut Pcg32,
    l: &mut Layout,
    keep: bool,
    yard: bool,
    asked: Option<&Asked>,
) -> Option<Vec<P>> {
    // The town wall's line where it runs along the block (planned: the castle is the same with the
    // wall taken down, but for a curtain on that side and the ranges by it); a side along it is that
    // wall.
    let town_walls: &[(P, P)] = town_wall;
    // (A stream's water gap in it is the wall's, but where a street crosses into the castle's
    // ground there.)
    let crossing: Vec<(P, P)> = l
        .roads
        .iter()
        .filter(|r| r.1 != 3)
        .flat_map(|r| r.0.windows(2).map(|q| (q[0], q[1])).collect::<Vec<_>>())
        .collect();
    let along = |a: P, b: P| {
        let bridged = crossing.iter().any(|&(q, r)| crosses(a, b, q, r));
        // (A gap the wall as laid has there, by the water: a stretch of it ends near.)
        let gap = |p: P| town_walls.iter().any(|&(q, r)| dist(q, p).min(dist(r, p)) < 30.0);
        along_wall(town_walls, town_gates, &|p| !bridged && gap(p) && site.near_water(p, 6.0), a, b)
    };
    // The curtain's bound: 6 ft in from the block; a side on a town wall's line (`build_town` sets
    // it there) stays so where that wall runs its whole length, else stands well back from it.
    // (Corners a step apart, as an inset leaves them, made one.)
    let mut block: Vec<P> = Vec::new();
    for p in block_in {
        if block.last().is_none_or(|q| dist(*q, *p) > 0.5) {
            block.push(*p);
        }
    }
    while block.len() > 1 && dist(block[0], block[block.len() - 1]) <= 0.5 {
        block.pop();
    }
    if block.len() < 3 {
        return None;
    }
    if area(&block) < 0.0 {
        block.reverse();
    }
    // (A hand castle's outline may turn inward: drawn in by its sides' setbacks, `clip`.)
    let concave = asked.is_some() && !super::clip::is_convex(&block);
    let m = block.len();
    let d: Vec<f64> = (0..m)
        .map(|e| {
            let (a, b) = (block[e], block[(e + 1) % m]);
            let el = dist(a, b).max(1e-9);
            let u = mul(sub(b, a), 1.0 / el);
            let n = mul([-u[1], u[0]], CURTAIN_IN);
            let mid = lerp(a, b, 0.5);
            // (How far in from the block's side the wall's line runs there, at most 3 ft: the side
            // stands back 30 ft from the wall itself, not from the block.)
            let line_in = town_walls
                .iter()
                .filter(|&&(q, r)| dist(q, r) >= 1.0 && cross(u, mul(sub(r, q), 1.0 / dist(q, r))).abs() < 0.12 && seg_dist(mid, q, r) <= 3.0)
                .map(|&(q, r)| [q, r].iter().map(|&p| dot(sub(p, mid), mul(n, 1.0 / CURTAIN_IN))).fold(f64::MIN, f64::max))
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
            match line_in {
                Some(o) if !along(add(a, n), add(b, n)) => 31.0 + o.clamp(0.0, 4.0),
                // (Room for a town castle's moat, but where the town wall is its.)
                _ if set.follow && set.moat.is_some() && !along(add(a, n), add(b, n)) => CURTAIN_IN + set.moat.map_or(0.0, |m| m.room()),
                _ => CURTAIN_IN,
            }
        })
        .collect();
    let mut outer = if concave { inset_concave(&block, &d) } else { inset(&block, &d) };
    // A town gate at the end of a side on the town wall (no curtain of the castle's there), or by
    // any side of one set in the town by hand (on a patch the town didn't plan for it): the
    // castle's corner is cut back from it, square to the way in from the gate and clear of its
    // towers, so the gate opens onto the town, not into the bailey.
    for &g in town_gates {
        let n = outer.len();
        if n < 3 {
            break;
        }
        if !(0..n).any(|e| (!set.planned || along(outer[e], outer[(e + 1) % n])) && seg_dist(g, outer[e], outer[(e + 1) % n]) < 45.0) {
            continue;
        }
        let to = sub(centroid(&outer), g);
        let u = mul(to, 1.0 / len(to).max(1e-9));
        let reach = laid.1.iter().filter(|t| t.1 > 12.0 && dist(t.0, g) < 60.0).map(|t| dot(sub(t.0, g), u) + 25.0).fold(50.0, f64::max);
        outer = clip_plain(&outer, mul(u, -1.0), -(dot(g, u) + reach));
    }
    // Roads across the castle's ground (an approach road to a gate, a street bent to its bridge;
    // not the paved sides of the patches): it stands clear of them, by a little more than a
    // street's strip.
    // (Not a street laid on a town wall's own line, along a side where that wall is the castle's.)
    let on_wall_line = |a: P, b: P| {
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        town_walls
            .iter()
            .any(|&(q, r)| dist(q, r) >= 1.0 && cross(u, mul(sub(r, q), 1.0 / dist(q, r))).abs() < 0.12 && seg_dist(lerp(a, b, 0.5), q, r) <= 3.0)
    };
    for (pts, _, w) in l.roads.iter().filter(|r| r.1 != 3) {
        for s in pts.windows(2) {
            let (a, b) = (s[0], s[1]);
            let (n, el) = (outer.len(), dist(a, b));
            if n < 3 || el < 1e-6 || on_wall_line(a, b) {
                continue;
            }
            let clear = 0.5 * w + 8.0;
            if super::poly_seg_dist(&outer, a, b) >= clear {
                continue;
            }
            let u = mul(sub(b, a), 1.0 / el);
            // (Not a stretch only leading up to a corner, wholly beyond the bound along its way:
            // the cut would shave off the whole side it points along, the town wall's among them.)
            let (lo, hi) = outer.iter().fold((f64::MAX, f64::MIN), |r, p| (r.0.min(dot(*p, u)), r.1.max(dot(*p, u))));
            if dot(b, u).min(hi) - dot(a, u).max(lo) <= 0.0 {
                continue;
            }
            let nrm = [-u[1], u[0]];
            let nrm = if dot(sub(centroid(&outer), a), nrm) >= 0.0 { nrm } else { mul(nrm, -1.0) };
            let cut = clip_plain(&outer, mul(nrm, -1.0), -(dot(a, nrm) + clear));
            if cut.len() >= 3 && area(&cut).abs() >= 0.75 * area(&outer).abs() {
                outer = cut;
            }
        }
    }
    // A stretch of the town wall running beside a side of the bound, outside it and nearer than
    // 30 ft (where the wall steps out round a corner or a tower): the side stands back 30 ft
    // from it, a strip a street can take, not a gap a curtain would leave against the wall.
    // (How far a wall piece runs beside side `e`, nearer than 32 ft and outside the bound; the
    // test sums a side's whole run along the pieces, so a wall of several short pieces counts too.)
    let beside = |outer: &[P], e: usize, q: P, r: P| -> Option<f64> {
        let (ql, n) = (dist(q, r), outer.len());
        if ql < 1.0 || n < 3 {
            return None;
        }
        let uq = mul(sub(r, q), 1.0 / ql);
        let (a, b) = (outer[e], outer[(e + 1) % n]);
        let el = dist(a, b);
        if el < 1.0 || cross(mul(sub(b, a), 1.0 / el), uq).abs() >= 0.26 || along(a, b) {
            return None;
        }
        let ue = mul(sub(b, a), 1.0 / el);
        let (t0, t1) = (dot(sub(q, a), ue), dot(sub(r, a), ue));
        let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(el));
        if hi <= lo {
            return None;
        }
        let on_wall = |t: f64| lerp(q, r, ((t - t0) / (t1 - t0)).clamp(0.0, 1.0));
        let (pl, ph) = (on_wall(lo), on_wall(hi));
        let near = seg_dist(pl, a, b).min(seg_dist(ph, a, b));
        ((2.5..32.0).contains(&near) && !contains(outer, lerp(pl, ph, 0.5))).then_some(hi - lo)
    };
    for &(q, r) in town_walls {
        let (ql, n) = (dist(q, r), outer.len());
        if ql < 1.0 || n < 3 {
            continue;
        }
        let uq = mul(sub(r, q), 1.0 / ql);
        let close = (0..n).any(|e| beside(&outer, e, q, r).is_some() && town_walls.iter().filter_map(|&(q2, r2)| beside(&outer, e, q2, r2)).sum::<f64>() >= 25.0);
        if !close {
            continue;
        }
        let nrm = [-uq[1], uq[0]];
        let nrm = if dot(sub(centroid(&outer), q), nrm) >= 0.0 { nrm } else { mul(nrm, -1.0) };
        let cut = clip_plain(&outer, mul(nrm, -1.0), -(dot(q, nrm) + 32.0));
        if cut.len() >= 3 && area(&cut).abs() >= 0.75 * area(&outer).abs() {
            outer = cut;
        }
    }
    // (Nor on a sliver of ground, as a patch squeezed between the town wall and a street leaves:
    // no room for a keep and a yard, only a gate.)
    if outer.len() < 3 || area(&outer).abs() < MIN_BOUND_SQFT {
        return None;
    }
    // (A square tower reaches further across its corners.)
    let reach = |t: P| if l.square_tower(t).is_some() { 15.6 } else { 11.0 };
    let mut others: Vec<(P, f64)> = l
        .towers
        .iter()
        .map(|t| (*t, reach(*t)))
        .chain(l.gate_towers.iter().map(|t| (*t, 15.0)))
        .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { reach(t.0) })))
        .chain(laid.1.iter().copied())
        .collect();
    // The ring, laid again where a road runs through it (below).
    let mut cuts = 0;
    let (survey, top, flat, ring) = loop {
        let survey = Survey::of(site, &outer);
        // (The top with room round it for a keep and a yard: a high point at the bound's edge has no
        // brow to follow.)
        let (hp, top) = survey
            .best(|p| inset_depth(&outer, p) >= R_MIN)
            .or_else(|| survey.best(|_| true))
            .unwrap_or((centroid(&outer), 0.0));
        let flat = survey.relief() < FLAT_FT;
        let a_out = area(&outer);
        let walled_out: Vec<bool> = (0..outer.len()).map(|e| along(outer[e], outer[(e + 1) % outer.len()])).collect();
        let ring = if flat || !set.follow {
            None
        } else {
            brow_ring(site, &outer, &walled_out, hp, top, a_out, if set.inner.is_some() { 0.8 } else { 0.5 })
        };
        // (On flat ground, or a top too small to hold it, or drawn by hand: the curtain keeps to the
        // block.)
        let ring = ring.unwrap_or_else(|| outer.clone());
        // Its sharp corners cut back (a town's castle: one drawn by hand keeps to its outline).
        // (Its corners slid along the town wall within the bound, and not up to a town gate.)
        let room = |p: P| inset_depth(&outer, p) >= -1.0 && town_gates.iter().all(|g| dist(*g, p) >= 60.0);
        let ring = if set.follow { meet_wall(&cut_spikes(&ring), &along, &room) } else { ring };
        // A road running through it (one the castle's ground lies across: an approach bent through
        // it, a main street) cuts the bound off it, its larger side kept, and the ring is laid
        // again.
        let through = if cuts < 4 { road_through(&l.roads, &ring) } else { None };
        let Some((a, b, w)) = through else { break (survey, top, flat, ring) };
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        let nrm = [-u[1], u[0]];
        let clear = 0.5 * w + 8.0;
        let sides = [clip_plain(&outer, mul(nrm, -1.0), -(dot(a, nrm) + clear)), clip_plain(&outer, nrm, dot(a, nrm) - clear)];
        let Some(cut) = sides.into_iter().filter(|s| s.len() >= 3).max_by(|x, y| area(x).abs().total_cmp(&area(y).abs())) else {
            break (survey, top, flat, ring);
        };
        if area(&cut).abs() < 0.25 * a_out.abs() {
            break (survey, top, flat, ring);
        }
        outer = cut;
        cuts += 1;
    };
    let walled: Vec<bool> = (0..ring.len()).map(|e| along(ring[e], ring[(e + 1) % ring.len()])).collect();
    // Its curtain towers' shapes by the place's walls (item 104: `WallStyle::castle`). Its keep's corner
    // turrets keep their own roll (a share square), as before the styles: the turrets' shape sets which
    // `KEEP_FITS` size a T or L keep fits.
    let square = prng.next_f64() < KEEP_SQUARE_SHARE;
    let style = l.wall_style.castle();
    let short = obb(&hull(ring.clone())).short;
    let want = area(&ring) >= BIG_AREA
        && short >= BIG_WIDTH
        && (set.capital
            || set.inner.unwrap_or(if set.follow {
                area(&ring) >= TOWN_INNER
            } else {
                area(&ring) >= HAND_INNER && short >= 200.0
            }));
    // The outer curtain and its towers. (The streets its gate faces: none inside it.)
    let streets: Vec<(P, P)> = l
        .roads
        .iter()
        .flat_map(|(pts, _, _)| pts.windows(2).map(|s| (s[0], s[1])))
        .filter(|&(a, b)| inset_depth(&ring, lerp(a, b, 0.5)) <= 3.0)
        .collect();
    let street_dist = |p: P| streets.iter().map(|&(a, b)| seg_dist(p, a, b)).fold(f64::MAX, f64::min);
    // (Nor onto the water or the back of a building: ground open before it, a way's width 24 ft out.)
    let rn = ring.len();
    let reach = ring.iter().map(|p| dist(*p, centroid(&ring))).fold(0.0, f64::max) + 60.0;
    let before: Vec<Vec<P>> = l
        .buildings
        .iter()
        .filter(|b| b.poly.iter().any(|p| dist(*p, centroid(&ring)) < reach))
        .map(|b| b.poly.clone())
        .collect();
    let open_before = |p: P| {
        let Some(e) = (0..rn).min_by(|&a, &b| {
            seg_dist(p, ring[a], ring[(a + 1) % rn])
                .total_cmp(&seg_dist(p, ring[b], ring[(b + 1) % rn]))
                .then(a.cmp(&b))
        }) else {
            return true;
        };
        let u = mul(sub(ring[(e + 1) % rn], ring[e]), 1.0 / dist(ring[e], ring[(e + 1) % rn]).max(1e-9));
        let out = [u[1], -u[0]];
        [8.0, 16.0, 24.0].iter().all(|&d| {
            [-6.0, 0.0, 6.0].iter().all(|&s| {
                let q = add(add(p, mul(out, d)), mul(u, s));
                !site.wet(q) && before.iter().all(|b| super::poly_seg_dist(b, q, q) >= 3.0)
            })
        })
    };
    // (Not facing a town gate: the town's way in doesn't lead into the castle's.)
    let by_town_gate = |p: P| town_gates.iter().map(|g| GATE_AWAY - dist(p, *g)).fold(0.0, f64::max);
    let outer_score = |p: P| {
        let near = by_town_gate(p);
        let away = if near > 0.0 { 1_000.0 + 10.0 * near } else { 0.0 } + if open_before(p) { 0.0 } else { 2_000.0 };
        away + if streets.is_empty() {
            dist(p, gate_toward)
        } else {
            street_dist(p).max(15.0) + 0.2 * dist(p, gate_toward)
        }
    };
    let (t0, g0, w0) = (l.towers.len(), l.gate_towers.len(), l.walls.len());
    // (A town's tower beside one of a castle it planned goes, below; else the castle's stands clear.)
    let gap = if set.planned { 4.0 } else { TOWERS_CLEAR };
    // A gate standing back from the streets (up the brow): a way from it to the nearest (`gate_way`);
    // where none can be laid, the gate elsewhere (three tries).
    let mut banned: Vec<P> = Vec::new();
    // (A hand castle's gates past its main one.)
    let mut extra: Vec<(P, P)> = Vec::new();
    let mut dropped: Vec<(usize, String)> = Vec::new();
    let (outer_gate, outer_in) = loop {
        let mark = (l.walls.len(), l.towers.len(), l.gate_towers.len(), l.square_towers.len(), others.len(), l.tower_forms.len());
        let tried = |p: P| banned.iter().any(|b| dist(*b, p) < 60.0);
        let score = |p: P| outer_score(p) + if tried(p) { 5_000.0 } else { 0.0 };
        extra.clear();
        dropped.clear();
        let (g, gin) = curtain(
            l,
            &ring,
            &walled,
            set.wall_up,
            &score,
            &|p| by_town_gate(p) <= 0.0 && open_before(p) && !tried(p),
            &style,
            gap,
            &mut others,
            set.follow,
            asked.map_or(&[][..], |a| &a.gates[..]),
            &mut extra,
            &mut dropped,
        );
        if streets.is_empty() || street_dist(g) <= 20.0 {
            break (g, gin);
        }
        match gate_way(site, l, &ring, &block, &streets, laid, g, gin, set.moat.map_or(0.0, |m| super::moat::BERM + m.width + 4.0)) {
            Some(way) => {
                l.roads.push((way, 3, 10.0));
                break (g, gin);
            }
            None if banned.len() < 3 => {
                l.walls.truncate(mark.0);
                l.towers.truncate(mark.1);
                l.gate_towers.truncate(mark.2);
                l.square_towers.truncate(mark.3);
                others.truncate(mark.4);
                l.tower_forms.truncate(mark.5);
                banned.push(g);
            }
            None => break (g, gin),
        }
    };
    if let Some(a) = asked {
        a.report.borrow_mut().gates = dropped;
    }
    let cc = centroid(&ring);
    l.castles.push((cc, ring.iter().map(|p| dist(*p, cc)).fold(0.0, f64::max)));
    // An inner curtain round the highest ground with room for it (on flat ground, the middle drawn
    // back from the gate), leaving a broad outer bailey before the gate.
    let inner = if want {
        // (On a hill, the highest spots with room, 40 ft apart at least; then, as on flat ground,
        // the middle drawn back from the gate, about halfway out.)
        let mut tries: Vec<(P, bool)> = Vec::new();
        if !flat {
            for (p, _) in survey.ranked(|p| inset_depth(&ring, p) >= R_MIN + BAILEY_GAP) {
                if tries.len() < 6 && tries.iter().all(|t| dist(t.0, p) >= 40.0) {
                    tries.push((p, false));
                }
            }
        }
        let away = sub(cc, outer_gate);
        let u = mul(away, 1.0 / len(away).max(1e-9));
        let reach = ray_exit(&ring, cc, u);
        // (A sized castle's (`inner` set) two more between.)
        let fs: &[f64] = if set.inner.is_some() { &[0.3, 0.2, 0.1, 0.0, 0.15, 0.05] } else { &[0.3, 0.2, 0.1, 0.0] };
        tries.extend(fs.iter().map(|f| (add(cc, mul(u, f * reach)), true)));
        // (A capital's where it fits at all: if no bailey broad across the gate's side leaves room,
        // one broad before the gate.)
        let cones: &[bool] = if set.capital { &[true, false] } else { &[true] };
        cones.iter().find_map(|&cone| {
            tries
                .iter()
                .find_map(|&(o, half)| inner_ring(site, &ring, o, if half { top } else { site.height(o) }, half, outer_gate, cone))
        })
    } else {
        None
    };
    let mut gates = vec![outer_gate];
    let mut gate_ins: Vec<(P, P)> = vec![(outer_gate, outer_in)];
    let mut lanes: Vec<(P, P)> = vec![(outer_gate, add(outer_gate, mul(outer_in, 30.0)))];
    for &(g, gin) in &extra {
        gates.push(g);
        gate_ins.push((g, gin));
        lanes.push((g, add(g, mul(gin, 30.0))));
    }
    let outer_gates: Vec<(P, P)> = gate_ins.clone();
    if let Some(inner) = &inner {
        let walled_in = vec![false; inner.len()];
        let apart = |p: P| outer_gates.iter().all(|o| dist(p, o.0) >= GATES_APART);
        let (g, gin) = curtain(l, inner, &walled_in, true, &|p: P| dist(p, outer_gate), &apart, &style, 4.0, &mut others, set.follow, &[], &mut Vec::new(), &mut Vec::new());
        gates.push(g);
        gate_ins.push((g, gin));
        // The way from gate to gate along the outer bailey.
        let (a, b) = (add(outer_gate, mul(outer_in, 30.0)), sub(g, mul(gin, 30.0)));
        lanes.extend([(a, b), (b, g)]);
        let ci = centroid(inner);
        l.castles.push((ci, inner.iter().map(|p| dist(*p, ci)).fold(0.0, f64::max)));
    }
    // The inner curtain's towers (the ranges keep well clear of them: the outer bailey's way).
    let inner_towers: Vec<(P, f64)> = match &inner {
        Some(q) => l.towers[t0..]
            .iter()
            .map(|t| (*t, 11.0))
            .chain(l.gate_towers[g0..].iter().map(|t| (*t, 15.0)))
            .filter(|(t, _)| inset_depth(q, *t) > -2.0)
            .collect(),
        None => Vec::new(),
    };
    // The towers the keep keeps clear of: the castle's own, but those by a side along the town wall,
    // whose corners stand for them (a tower there whether the wall stands or not): the same keep
    // either way. (The ranges keep clear of every tower round the bailey: `by_ring`.)
    let rn = ring.len();
    let by_wall = |t: P| (0..rn).any(|e| walled[e] && seg_dist(t, ring[e], ring[(e + 1) % rn]) < 30.0);
    let towers: Vec<(P, f64)> = l.towers[t0..]
        .iter()
        .map(|t| (*t, 11.0))
        .chain(l.gate_towers[g0..].iter().map(|t| (*t, 15.0)))
        .filter(|(t, _)| !by_wall(*t))
        .chain((0..rn).filter(|&k| walled[k] != walled[(k + rn - 1) % rn]).map(|k| (ring[k], 11.0)))
        .collect();
    // The keep on the highest ground that holds it, inside the inner curtain if there is one (on flat
    // ground, about the middle).
    let court = inner.as_deref().unwrap_or(&ring);
    let axis = obb(&hull(court.to_vec())).axis;
    let c = centroid(court);
    let mut spots: Vec<(P, f64)> = survey.ranked(|p| inset_depth(court, p) >= KEEP_ROOM);
    if flat {
        spots.sort_by(|a, b| dist(a.0, c).total_cmp(&dist(b.0, c)));
        spots.insert(0, (c, 0.0));
    }
    spots.truncate(16);
    // (A small court: its middle.)
    spots.push((c, 0.0));
    // (A court too narrow for those: a small tower keep anywhere with room round it, the nearest
    // the middle first.)
    let mut more: Vec<(P, f64)> = survey.ranked(|p| inset_depth(court, p) >= SMALL_ROOM);
    more.sort_by(|a, b| dist(a.0, c).total_cmp(&dist(b.0, c)));
    more.truncate(24);
    let court_gate = *gates.last().unwrap_or(&outer_gate);
    // (A bent entrance's block stands in the yard behind its gate: the keep leaves it room, item 104.)
    let bent_gates: Vec<(P, P)> = if style.gatehouse == super::wallstyle::Gatehouse::Bent { gate_ins.clone() } else { Vec::new() };
    let room = |poly: &[P]| bent_gates.iter().all(|&(g, n)| super::gatehouse::bent_room(g, n, poly));
    // (Every tower by the bailey, the town wall's as laid whether standing or not: the same keep
    // either way.)
    let near: Vec<(P, f64)> = laid
        .1
        .iter()
        .copied()
        .filter(|(t, _)| inset_depth(&ring, *t) > -40.0)
        .chain(l.towers[t0..].iter().map(|t| (*t, if l.square_tower(*t).is_some() { 15.6 } else { 11.0 })))
        .chain(l.gate_towers[g0..].iter().map(|t| (*t, 15.0)))
        .chain(towers.iter().copied())
        .collect();
    // (The castle's own curtain round the court: not on a side along the town wall.)
    let own: Vec<bool> = (0..court.len()).map(|e| inner.is_some() || !walled[e]).collect();
    let curtain: Vec<(P, P)> = l.walls[w0..].iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let drawn: &[Vec<P>] = asked.map_or(&[][..], |a| &a.drawn[..]);
    let shape = asked.map_or(KeepShape::Rolled, |a| a.keep.clone());
    let kept = if keep {
        let mut why = None;
        let k = place_keep(site, court, &spots, &more, axis, square, &towers, &near, &own, &curtain, &gates, &lanes, court_gate, &room, (&shape, drawn), prng, &mut why);
        if let Some(a) = asked {
            a.report.borrow_mut().keep = why;
        }
        k
    } else {
        None
    };
    // (A keep set into the curtain: the curtain stops at its walls.)
    if let Some((k, _)) = &kept {
        let cut: Vec<Vec<P>> = l.walls[w0..].iter().flat_map(|w| cut_out(w, &k.poly)).collect();
        l.walls.truncate(w0);
        l.walls.extend(cut);
    }
    if yard {
        let clear: Vec<P> = kept.as_ref().map(|k| k.0.poly.clone()).unwrap_or_default();
        let from_drawn = l.buildings.len();
        let motte = kept.as_ref().and_then(|k| k.1);
        // (Clear of every tower by the bailey: the castle's own, and the town's on the sides where
        // its wall is the castle's.)
        let by_ring: Vec<(P, f64)> = l.towers[..t0]
            .iter()
            .map(|t| (*t, 11.0))
            .chain(l.gate_towers[..g0].iter().map(|t| (*t, 15.0)))
            .chain(laid.1.iter().copied())
            .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { 11.0 })))
            .filter(|(t, _)| inset_depth(&ring, *t) > -40.0)
            .chain(l.towers[t0..].iter().map(|t| (*t, if l.square_tower(*t).is_some() { 15.6 } else { 11.0 })))
            .chain(l.gate_towers[g0..].iter().map(|t| (*t, 15.0)))
            .chain(towers.iter().copied())
            .collect();
        let from = l.buildings.len();
        // A great castle's hall range first: in the inner bailey along its curtain, else along the
        // outer.
        if set.hall {
            match &inner {
                Some(q) => {
                    let mut q = q.clone();
                    if area(&q) < 0.0 {
                        q.reverse();
                    }
                    let open = vec![false; q.len()];
                    ranges(site, &q, &open, None, &gates, &by_ring, &[], &lanes, &clear, motte, w, HALL, from, prng, l);
                }
                None => ranges(site, &ring, &walled, None, &gates, &by_ring, &[], &lanes, &clear, motte, w, HALL, from, prng, l),
            }
        }
        let after_hall = l.buildings.len();
        ranges(site, &ring, &walled, inner.as_deref(), &gates, &by_ring, &inner_towers, &lanes, &clear, motte, w, RANGE, from, prng, l);
        // (A town's castle sized to them, or one it planned on its whole patch: free-standing
        // buildings in the yard as well.)
        if set.follow && (set.inner.is_some() || set.planned) {
            l.castle_yard = yard_buildings(site, &ring, inner.as_deref(), &gates, &by_ring, &inner_towers, &lanes, &clear, motte, w, from, l);
        }
        if set.follow {
            yard_trades(l, from, after_hall);
        }
        // (Clear of the buildings drawn by hand in its yard: theirs the ground.)
        if !drawn.is_empty() {
            let mut k = 0;
            l.buildings.retain(|b| {
                k += 1;
                k <= from_drawn || drawn.iter().all(|d| apart_by(d, &b.poly) >= DRAWN_CLEAR)
            });
        }
    }
    if let Some((mut b, motte)) = kept {
        b.ward = w;
        if let Some(m) = motte {
            l.mottes.push(m);
        }
        l.buildings.push(b);
    }
    // A town tower crowding a gatehouse or a tower of the castle's goes: not three towers in a row,
    // nor two side by side.
    if set.planned {
        let own: Vec<(P, f64)> = l.towers[t0..].iter().map(|t| (*t, if l.square_tower(*t).is_some() { 15.6 } else { 11.0 })).collect();
        let (towers, gates) = (&mut l.towers, &l.gate_towers[g0..]);
        let mut k = 0;
        towers.retain(|t| {
            k += 1;
            k > t0 || (gates.iter().all(|g| dist(*t, *g) >= 11.0 + 15.0 + TOWERS_CLEAR) && own.iter().all(|(o, r)| dist(*t, *o) >= 11.0 + r + TOWERS_CLEAR))
        });
    }
    // The bailey: an open yard of grass, built on only by hand.
    l.plazas.push(Plaza {
        pts: ring.clone(),
        kind: PlazaKind::Bailey,
        spot: super::places::Spot::None,
    });
    // Its moat (v63 castles), a town castle's in its block.
    if let Some(m) = set.moat {
        let motte = l.mottes.iter().any(|mt| contains(&ring, mt.at));
        super::moat::lay(site, l, m, &ring, &walled, if set.follow { &block } else { &[] }, &outer_gates, flat, motte);
    }
    Some(ring)
}

/// The castle's ground on a 10-ft lattice (local ft, whole multiples so a block moved a little
/// samples the same points): the dry points inside its outer bound and their heights.
struct Survey {
    i0: i64,
    j0: i64,
    nx: usize,
    ny: usize,
    h: Vec<Option<f64>>,
}

const SURVEY_FT: f64 = 10.0;

impl Survey {
    fn of<G: Ground>(site: &G, poly: &[P]) -> Survey {
        let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| {
            (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1]))
        });
        let f = |v: f64| crate::core::floor(v / SURVEY_FT) as i64;
        let (i0, j0, i1, j1) = (f(x0), f(y0), f(x1), f(y1));
        let (nx, ny) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
        let h = (0..nx * ny)
            .map(|k| {
                let p = [((i0 + (k % nx) as i64) as f64 + 0.5) * SURVEY_FT, ((j0 + (k / nx) as i64) as f64 + 0.5) * SURVEY_FT];
                (contains(poly, p) && !site.wet(p)).then(|| site.height(p))
            })
            .collect();
        Survey { i0, j0, nx, ny, h }
    }

    fn point(&self, k: usize) -> P {
        [
            ((self.i0 + (k % self.nx) as i64) as f64 + 0.5) * SURVEY_FT,
            ((self.j0 + (k / self.nx) as i64) as f64 + 0.5) * SURVEY_FT,
        ]
    }

    /// Highest and lowest dry ground minus each other.
    fn relief(&self) -> f64 {
        let (lo, hi) = self.h.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), &h| (a.min(h), b.max(h)));
        if lo <= hi { hi - lo } else { 0.0 }
    }

    /// A dry point's ground with its surroundings (20 ft round): their mean height.
    fn smoothed(&self, k: usize) -> Option<f64> {
        self.h[k]?;
        let (i, j) = ((k % self.nx) as i64, (k / self.nx) as i64);
        let (mut s, mut n) = (0.0, 0.0);
        for dj in -2..=2i64 {
            for di in -2..=2i64 {
                let (a, b) = (i + di, j + dj);
                if a >= 0 && b >= 0 && (a as usize) < self.nx && (b as usize) < self.ny && di * di + dj * dj <= 5 {
                    if let Some(h) = self.h[b as usize * self.nx + a as usize] {
                        s += h;
                        n += 1.0;
                    }
                }
            }
        }
        Some(s / n)
    }

    /// The dry points where `ok` holds, highest ground first (`smoothed`; equals row by row).
    fn ranked(&self, ok: impl Fn(P) -> bool) -> Vec<(P, f64)> {
        let mut v: Vec<(usize, P, f64)> = (0..self.nx * self.ny)
            .filter_map(|k| self.smoothed(k).map(|m| (k, self.point(k), m)))
            .filter(|x| ok(x.1))
            .collect();
        v.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        v.into_iter().map(|x| (x.1, x.2)).collect()
    }

    /// The top of the ground where `ok` holds: its point and height (`smoothed`).
    fn best(&self, ok: impl Fn(P) -> bool) -> Option<(P, f64)> {
        self.ranked(ok).into_iter().next()
    }
}

