//! A row of lots along one side of a street (a hand-laid town's path fill): houses lining a
//! street outside any closed block, as along the roads out of a medieval town's gates.
//!
//! The street (a polyline, town-local ft) is first straightened (`STRAIGHT_FT`) into runs. Each
//! run carries a band on the chosen side, from `setback` to `setback + depth` ft off the street
//! line, cut square to the run at its ends. Where the street bends toward the row (the inside of
//! the turn) the bands of the two runs meet on the bisector; on the outside the wedge between
//! them stays open, unless the bend is gentle (`MITRE_DEG`, a curve drawn as many short runs),
//! where they meet on the bisector too. Lots are cut along each band square to the run, their
//! widths from the district's lot area over the depth; now and then a slot is left empty (a
//! yard) and every so many lots an alley runs back from the street. Lots are kept as
//! `build_block` keeps them (4+ corners, area, short side, fill of their box, a front on the
//! street line) minus a region to avoid; their buildings stand back from the street as
//! `build_block`'s do. Every roll is taken whatever is avoided, so what the region takes away
//! leaves the rest of the row as it was.

use super::catalog::Ward;
use super::clip::{self, Shape};
use super::geom::*;
use super::{Building, District, Ground, Layout, Structure};
use crate::World;
use crate::core::hash::FastMap;
use crate::core::rng::Pcg32;
use crate::t0::T0;
use crate::t0::settle::Tier;

/// How far a straightened run may stray from the street drawn (ft): the setback covers it.
const STRAIGHT_FT: f64 = 1.5;
/// Bends up to this (degrees) are mitred on their outside too: the bands meet on the bisector.
const MITRE_DEG: f64 = 20.0;
/// The narrowest lot kept (`build_block`'s short side).
const MIN_LOT_FT: f64 = 16.0;
/// An alley's width between two lots (ft): narrower on purpose than `build_block`'s (a 16 ft gap,
/// `2 * fill::ALLEY_HALF_FT`, drawn 17 ft wide): a row's alley is a passage back to the yards
/// behind the houses, not a lane through a block.
const ALLEY_FT: f64 = 8.0;
/// The cell of the grid the kept lots are found by when a new lot is tested against them (ft).
const CELL_FT: f64 = 64.0;
/// The share of slots left empty (yards and gardens between houses).
const YARD_SHARE: f64 = 0.08;

/// One side of a street to line: the street's centre line (town-local ft), the side (`left` of
/// its direction from the first point), and the band the lots stand in, `setback` to
/// `setback + depth` ft off the line (`depth` 16 ft or more, or no lot is kept).
pub(super) struct Side<'a> {
    pub(super) line: &'a [P],
    pub(super) left: bool,
    pub(super) setback: f64,
    pub(super) depth: f64,
}

/// A straightened run of the street and its band.
struct Run {
    a: P,
    /// Along the run, and the side's normal (away from the street).
    u: P,
    n: P,
    band: Vec<P>,
    /// The front's span along the run from `a`.
    f0: f64,
    f1: f64,
}

/// The street straightened: its points within `STRAIGHT_FT` of the line drawn (Douglas-Peucker),
/// repeated points dropped.
fn straighten(line: &[P]) -> Vec<P> {
    let mut pts: Vec<P> = Vec::with_capacity(line.len());
    for &p in line {
        if pts.last().is_none_or(|q| dist(*q, p) > 1e-6) {
            pts.push(p);
        }
    }
    let m = pts.len();
    if m < 3 {
        return pts;
    }
    let mut keep = vec![false; m];
    (keep[0], keep[m - 1]) = (true, true);
    let mut todo = vec![(0, m - 1)];
    while let Some((i, j)) = todo.pop() {
        let (mut far, mut at) = (0.0, 0);
        for k in i + 1..j {
            let d = seg_dist(pts[k], pts[i], pts[j]);
            if d > far {
                (far, at) = (d, k);
            }
        }
        if far > STRAIGHT_FT {
            keep[at] = true;
            todo.push((i, at));
            todo.push((at, j));
        }
    }
    (0..m).filter(|&i| keep[i]).map(|i| pts[i]).collect()
}

/// Where the bands of runs `u0` and `u1` (unit, in order) meet at their joint: on the bisector
/// (the direction it cuts across, unit), or `None` for square ends with the wedge open: the
/// outside of a bend sharper than `MITRE_DEG`, and a street doubling back on itself.
fn joint(u0: P, u1: P, left: bool) -> Option<P> {
    let k = add(u0, u1);
    let kl = len(k);
    if kl < 0.1 {
        return None;
    }
    let inside = (cross(u0, u1) > 0.0) == left;
    (inside || dot(u0, u1) >= libm::cos(MITRE_DEG.to_radians())).then(|| mul(k, 1.0 / kl))
}

/// The side's runs with their bands, in order along the street.
fn runs(side: &Side) -> Vec<Run> {
    let pts = straighten(side.line);
    let (s, d) = (side.setback, side.depth);
    let us: Vec<P> = pts.windows(2).map(|w| mul(sub(w[1], w[0]), 1.0 / dist(w[0], w[1]))).collect();
    let mut out = Vec::with_capacity(us.len());
    for (i, &u) in us.iter().enumerate() {
        let (a, b) = (pts[i], pts[i + 1]);
        let l = dist(a, b);
        let n = if side.left { [-u[1], u[0]] } else { [u[1], -u[0]] };
        let ks = if i > 0 { joint(us[i - 1], u, side.left) } else { None }.unwrap_or(u);
        let ke = us.get(i + 1).and_then(|&un| joint(u, un, side.left)).unwrap_or(u);
        // A strip past both ends, cut down to the run by its end lines.
        let at = |t: f64, o: f64| add(add(a, mul(u, t)), mul(n, o));
        let reach = s + d + 1.0;
        let mut band = vec![at(-reach, s), at(l + reach, s), at(l + reach, s + d), at(-reach, s + d)];
        band = clip_plain(&band, mul(ks, -1.0), -dot(a, ks));
        band = clean(&clip_plain(&band, ke, dot(b, ke)));
        if area(&band) < 0.0 {
            band.reverse();
        }
        // The front line meets the end lines here (along the run from `a`).
        let f0 = -s * dot(n, ks) / dot(u, ks);
        let f1 = l - s * dot(n, ke) / dot(u, ke);
        out.push(Run { a, u, n, band, f0, f1 });
    }
    out
}

/// The bands of `side`, one per straightened run, in order along the street (anticlockwise):
/// the ground its row is laid in.
pub(super) fn bands(side: &Side) -> Vec<Vec<P>> {
    runs(side).into_iter().map(|r| r.band).filter(|b| b.len() >= 3).collect()
}

/// A lot edge longer than 6 ft lies on the street line (outward normal `n`, offset `c`), as
/// `build_block` asks of its lots.
fn fronts(lot: &[P], n: P, c: f64) -> bool {
    let m = lot.len();
    (0..m).any(|i| {
        let (a, b) = (lot[i], lot[(i + 1) % m]);
        (dot(a, n) - c).abs() < 0.5 && (dot(b, n) - c).abs() < 0.5 && dist(a, b) > 6.0
    })
}

/// `build_block`'s test of a lot: no triangles, slivers or scraps, and a front on the street.
fn good(lot: &[P], dp: &District, n: P, c: f64) -> bool {
    if lot.len() < 4 {
        return false;
    }
    let la = area(lot).abs();
    let o = obb(lot);
    la >= dp.min_sq / 4.0 && o.short >= MIN_LOT_FT && la >= 0.62 * o.long * o.short && fronts(lot, n, c)
}

fn bbox(poly: &[P]) -> [f64; 4] {
    poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])])
}

fn meets(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// The shapes to avoid whose bounds meet `bb`, cloned only when there are any: `cand` holds
/// places in `avoid` with their bounds.
fn near(avoid: &[Shape], cand: &[(usize, [f64; 4])], bb: &[f64; 4]) -> Vec<Shape> {
    cand.iter().filter(|(_, b)| meets(b, bb)).map(|&(i, _)| avoid[i].clone()).collect()
}

/// `lot` less the avoided shapes near it: the lot itself when they take nothing, else the
/// largest convex piece of what is left that still makes a lot, if any.
fn spare(lot: Vec<P>, avoid: &[Shape], cand: &[(usize, [f64; 4])], dp: &District, n: P, c: f64) -> Option<Vec<P>> {
    let close = near(avoid, cand, &bbox(&lot));
    if close.is_empty() {
        return Some(lot);
    }
    let la = area(&lot).abs();
    let rest = clip::difference(&[Shape::ring(lot.clone())], &close);
    if clip::area(&rest) >= la - 0.5 {
        return Some(lot);
    }
    let mut best: Option<(f64, Vec<P>)> = None;
    for piece in clip::convex_pieces(&rest) {
        let piece = clean(&piece);
        let pa = area(&piece);
        if good(&piece, dp, n, c) && best.as_ref().is_none_or(|b| pa > b.0) {
            best = Some((pa, piece));
        }
    }
    best.map(|b| b.1)
}

/// A lot kept and its building's footprint.
type Lot = (Vec<P>, Vec<P>);

/// The lots laid along `side`, each with its building's footprint, and the alleys' centre lines
/// (front to back); see the module comment.
fn lay<G: Ground>(site: &G, side: &Side, dp: &District, rng: &mut Pcg32, avoid: &[Shape]) -> (Vec<Lot>, Vec<(P, P)>) {
    let (s, d) = (side.setback, side.depth);
    let runs = runs(side);
    let all = runs.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, run| {
        let b = bbox(&run.band);
        [r[0].min(b[0]), r[1].min(b[1]), r[2].max(b[2]), r[3].max(b[3])]
    });
    // The avoided shapes near the row, by place and bounds; each run looks only at those near its
    // own band (a diagonal street's whole row can span the town).
    let row_avoid: Vec<(usize, [f64; 4])> = avoid.iter().enumerate().map(|(i, sh)| (i, bbox(&sh.outer))).filter(|(_, b)| meets(b, &all)).collect();
    let alley_every = |rng: &mut Pcg32| crate::core::round(dp.block_size * (0.5 + rng.next_f64())).max(2.0) as u32;
    let mut every = alley_every(rng);
    let mut since = 0u32;
    let (mut kept, mut alleys): (Vec<(Vec<P>, Vec<P>, [f64; 4])>, Vec<(P, P)>) = (Vec::new(), Vec::new());
    // The kept lots by grid cell (lookup only), to test a new lot against those near it alone.
    let mut grid: FastMap<(i64, i64), Vec<usize>> = FastMap::default();
    let cells = |b: [f64; 4]| {
        let c = |x: f64| crate::core::floor(x / CELL_FT) as i64;
        (c(b[0])..=c(b[2])).flat_map(move |x| (c(b[1])..=c(b[3])).map(move |y| (x, y)))
    };
    let mut close_kept: Vec<usize> = Vec::new();
    for run in &runs {
        if run.band.len() < 3 {
            continue;
        }
        let band_bb = bbox(&run.band);
        let cand: Vec<(usize, [f64; 4])> = row_avoid.iter().filter(|(_, b)| meets(b, &band_bb)).copied().collect();
        let (a, u, n) = (run.a, run.u, run.n);
        // The street line as `build_block` has its lines: normal toward the street, the lot on
        // the side where `dot(p, sn) <= sc`.
        let (sn, sc) = (mul(n, -1.0), -(dot(a, n) + s));
        let ua = dot(a, u);
        let mut t = run.f0;
        // An alley is kept only between two lots kept: it waits for the lot after it.
        let (mut prev_kept, mut pending): (bool, Option<(P, P)>) = (false, None);
        while run.f1 - t >= MIN_LOT_FT {
            if since >= every && run.f1 - t >= ALLEY_FT + MIN_LOT_FT {
                let m = t + 0.5 * ALLEY_FT;
                let lane = [add(add(a, mul(u, m)), mul(n, s)), add(add(a, mul(u, m)), mul(n, s + d))];
                let strip = vec![add(lane[0], mul(u, -0.5 * ALLEY_FT)), add(lane[0], mul(u, 0.5 * ALLEY_FT)), add(lane[1], mul(u, 0.5 * ALLEY_FT)), add(lane[1], mul(u, -0.5 * ALLEY_FT))];
                let close = near(avoid, &cand, &bbox(&strip));
                let clear = close.is_empty() || clip::area(&clip::intersection(&[Shape::ring(strip)], &close)) < 1.0;
                pending = (prev_kept && clear).then_some((lane[0], lane[1]));
                (t, since, every) = (t + ALLEY_FT, 0, alley_every(rng));
                continue;
            }
            let (r1, r2, yard, indent) = (rng.next_f64(), rng.next_f64(), rng.next_f64(), rng.next_f64());
            prev_kept = false;
            let want = dp.min_sq * libm::pow(1.0 + dp.size_chaos, r1 + r2 - 1.0);
            let mut w = (want / d).clamp(MIN_LOT_FT + 0.5, (2.5 * d).max(24.0));
            since += 1;
            if yard < YARD_SHARE {
                // An alley waiting goes on waiting over a yard, for the next lot, so alleys keep
                // their spacing.
                t += (0.6 * w).clamp(8.0, 30.0);
                continue;
            }
            let alley = pending.take();
            // The last lot of a run takes what would be too narrow for another.
            let last = run.f1 - (t + w) < MIN_LOT_FT;
            if last {
                w = run.f1 - t;
            }
            // The first and last lots reach the band's end lines (square or on the bisector).
            let mut lot = run.band.clone();
            if t > run.f0 {
                lot = clip_plain(&lot, mul(u, -1.0), -(ua + t));
            }
            if !last {
                lot = clip_plain(&lot, u, ua + t + w);
            }
            t += w;
            let lot = clean(&lot);
            if !good(&lot, dp, sn, sc) {
                continue;
            }
            let Some(lot) = spare(lot, avoid, &cand, dp, sn, sc) else { continue };
            // (A street doubling back or coming round again: its rows don't overlap.)
            let bb = bbox(&lot);
            close_kept.clear();
            for cell in cells(bb) {
                close_kept.extend(grid.get(&cell).into_iter().flatten().copied());
            }
            close_kept.sort_unstable();
            close_kept.dedup();
            let lot_shape = [Shape::ring(lot.clone())];
            let over = close_kept.iter().any(|&i| meets(&kept[i].2, &bb) && clip::area(&clip::intersection(&lot_shape, &[Shape::ring(kept[i].0.clone())])) > 1.0);
            if over {
                continue;
            }
            // Some buildings stand back from the street, as in `build_block`.
            let la = area(&lot).abs();
            let back = indent * (crate::core::sqrt(la) / 3.0).min(16.0);
            let poly = if back > 6.5 { clean(&clip_plain(&lot, sn, sc - back)) } else { lot.clone() };
            if poly.len() < 3 || area(&poly).abs() < 150.0 || !site.buildable(&poly) {
                continue;
            }
            for cell in cells(bb) {
                grid.entry(cell).or_default().push(kept.len());
            }
            kept.push((lot, poly, bb));
            alleys.extend(alley);
            prev_kept = true;
        }
    }
    (kept.into_iter().map(|(lot, poly, _)| (lot, poly)).collect(), alleys)
}

/// Line `side` of a street with a row of the ward's houses (see the module comment): its
/// district's lot sizes and alley spacing (`dp`), cut by `rng`, nothing built over `avoid`
/// (other streets, blocks, buildings drawn by hand: any shapes, overlapping or not). The
/// buildings go to `l.buildings` (ids left to the caller), a town's alleys to `l.roads` (paved,
/// as `build_block`'s are). Returns the lots, in step with the buildings pushed (a building is its
/// lot, or the lot's back part when it stands back from the street). The band (`bands`) is not
/// recorded: the caller does that if it wants.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_row<G: Ground>(site: &G, side: &Side, ward: Ward, dp: &District, rng: &mut Pcg32, avoid: &[Shape], l: &mut Layout) -> Vec<Vec<P>> {
    let (lots, alleys) = lay(site, side, dp, rng, avoid);
    if l.tier >= Tier::Town {
        for (a, b) in alleys {
            l.roads.push((vec![a, b], 3, ALLEY_FT));
        }
    }
    lots.into_iter()
        .map(|(lot, poly)| {
            l.buildings.push(house(site, poly, ward));
            lot
        })
        .collect()
}

fn house<G: Ground>(site: &G, poly: Vec<P>, ward: Ward) -> Building {
    let pad = site.pad(&poly);
    Building { poly, ward, func: None, residential: 0, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id: 0, parts: Vec::new(), keep: None, deck: None }
}

/// QA only (`towncheck rows`), not used by generation: one side of a street to line (world ft),
/// and `tag`: it is built round shapes of `qa_rows`' `avoid` tagged otherwise.
pub struct QaRow {
    pub line: Vec<P>,
    pub left: bool,
    pub setback: f64,
    pub depth: f64,
    pub tag: u32,
}

/// What `qa_rows` laid along one side (world ft): each lot kept with its building's footprint, and
/// the alleys' centre lines (front to back).
pub struct QaRowOut {
    pub lots: Vec<(Vec<P>, Vec<P>)>,
    pub alleys: Vec<(P, P)>,
}

/// QA only (`towncheck rows`), not used by generation: `rows` laid by `build_row` on layout
/// `index`'s ground, all as `ward`'s with its district rolled from `seed`, each row with its own
/// stream (`seed`, its number), round the shapes of `avoid` (world ft) not tagged with its own tag.
pub fn qa_rows(world: &World, t0: &T0, index: usize, ward: Ward, seed: u64, rows: &[QaRow], avoid: &[(u32, Shape)]) -> Vec<QaRowOut> {
    let (site, _, _) = super::prepare(world, t0, index);
    let c = site.center;
    let local = |p: &P| sub(*p, c);
    let ring = |r: &[P]| r.iter().map(local).collect::<Vec<P>>();
    let dp = District::roll(ward, seed);
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let line = ring(&row.line);
            let side = Side { line: &line, left: row.left, setback: row.setback, depth: row.depth };
            let others: Vec<Shape> = avoid.iter().filter(|(t, _)| *t != row.tag).map(|(_, s)| Shape { outer: ring(&s.outer), holes: s.holes.iter().map(|h| ring(h)).collect() }).collect();
            let mut rng = Pcg32::new(seed, 1000 + i as u64);
            let mut l = Layout::empty(index as u32, c, 0.0, Tier::Town);
            let lots = build_row(&site, &side, ward, &dp, &mut rng, &others, &mut l);
            let world_ft = |r: &[P]| r.iter().map(|p| add(*p, c)).collect::<Vec<P>>();
            let lots = lots.iter().zip(&l.buildings).map(|(a, b)| (world_ft(a), world_ft(&b.poly))).collect();
            QaRowOut { lots, alleys: l.roads.iter().map(|r| (add(r.0[0], c), add(r.0[1], c))).collect() }
        })
        .collect()
}

/// QA only (`towncheck rows`): the bands `qa_rows` lays a row in (world ft or any frame: pure
/// geometry), one per straightened run.
pub fn qa_bands(line: &[P], left: bool, setback: f64, depth: f64) -> Vec<Vec<P>> {
    bands(&Side { line, left, setback, depth })
}
