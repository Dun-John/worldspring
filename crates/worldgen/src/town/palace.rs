//! Palaces' and monasteries' footprints: a palace (and a monastery, for its cloister round its garth:
//! `interior::hub`'s `monastery`) takes in the lots beside its own for a big footprint (`merge`): the homes of its ward on the same block, wall to wall with it, as long as the
//! hull of them all crosses no street or alley and covers no other building; it then stands on the
//! largest rectangle that hull holds, square to its own lot (a capital's with a side an odd number
//! of squares across for its front, for its axis), on ground fit to build on. The lots taken in are gone (their ids
//! with them: every other building keeps its own, as the ward editor keeps them); the palace keeps
//! its id and name. What the hull holds outside the rectangle is the palace's open ground. The
//! capital's always stands round a court: where the lots beside it give no footprint big enough, it
//! clears more of its block, taking in the homes and the open ground there (`clear_ground`). Every
//! other palace stays smaller than the least a capital's is; a monastery grows to `MONASTERY_FT2`, on a
//! rectangle 12 squares by 12 or more (`MONASTERY_SIDES`; clearing its block, its alleys too, on ground
//! fit for a terrace where the lots beside it give none). The capital's then gets its front and
//! the forecourt before it, walled, its gate on the axis, a paved way on to the street (`approach`).
//! A coaching inn grows likewise round its yard (`interior::hub`'s `inn`, decision 4): every
//! caravanserai and a town's biggest inn on a street (a city's two, a metropolis' three:
//! `coaching_inns`) take in the homes of their ward beside them to `INN_FT2` (`CARAVAN_FT2`) on a
//! rectangle of `interior::hub::YARD` squares or more with a side on a street, else clear their
//! block's homes for one; their lots are their ground (`Layout::grounds`), as a monastery's.
//! A town laid out anew by hand keeps its palace's footprint as generated wherever what stands under
//! it now is its own lot and homes clear of the streets (the capital's and a monastery's whatever its
//! patch is laid out as, open ground too, the capital's front as generated; a coaching inn's on any
//! homes outside a castle's ward, never on open ground: `follow`, `approach`);
//! else it is grown again. Nothing overlaps a palace, by the area two outlines share
//! (`super::overlap`), however little.

use super::geom::{Labeled, add, area, centroid, clip, contains, dist, dot, hull, mul, obb, sub};
use super::{Building, Layout, P, Site, Structure, Ward, catalog, on_lot, overlap, poly_seg_dist};

/// The footprint a palace grows to (sq ft): a capital's round a court, another's round its hall.
const CAPITAL_FT2: f64 = 15000.0;
const CITY_FT2: f64 = 6000.0;
/// The sides a palace's rectangle may have (ft): its short side `least` to `most_short`, its long one
/// `long` to `most_long`; its floor `most_area` at most.
#[derive(Clone, Copy)]
pub(super) struct Sides {
    pub(super) least: f64,
    pub(super) long: f64,
    pub(super) most_short: f64,
    pub(super) most_long: f64,
    pub(super) most_area: f64,
}
/// The capital's: 17 squares by 19 at least, 30 by 40 at most; round a court (`courts`: its ranges,
/// `interior::hub::palace`: a front range deep enough for the long gallery 3 squares wide before its
/// stair's landing, a court 7 squares across or more each way).
const CAPITAL_SIDES: Sides = Sides { least: 85.0, long: 95.0, most_short: 150.0, most_long: 200.0, most_area: f64::MAX };
/// The capital's where no footprint round a court can be had even by clearing its block: a front
/// range and a back range, 11 squares by 13 at least.
const CAPITAL_LEAST: Sides = Sides { least: 55.0, long: 65.0, most_short: 150.0, most_long: 200.0, most_area: f64::MAX };
/// Another's: 6 squares by 6 to 16 by 30, a hall house as big as its zones suit (`interior::hub::keep`),
/// its floor 240 squares at most: well under the least a capital's is (17 by 19, 323).
const CITY_SIDES: Sides = Sides { least: 30.0, long: 30.0, most_short: 80.0, most_long: 150.0, most_area: CITY_FT2 };
/// A monastery's: 12 squares by 12 to 22 by 26, its ranges round its cloister (`interior::hub`'s
/// `monastery`), its floor `MONASTERY_FT2` and a little more at most.
const MONASTERY_SIDES: Sides = Sides { least: 60.0, long: 60.0, most_short: 110.0, most_long: 130.0, most_area: 1.2 * MONASTERY_FT2 };
/// The footprint a monastery grows to (sq ft), and the most lots it takes in.
const MONASTERY_FT2: f64 = 8000.0;
const MONASTERY_MOST: usize = 5;
/// A coaching inn's (`is_coaching`): 11 squares by 12 to 18 by 22, round its yard; the footprint an inn
/// grows to and a caravanserai's (sq ft), and the most lots either takes in.
const INN_SIDES: Sides = Sides { least: 55.0, long: 60.0, most_short: 90.0, most_long: 110.0, most_area: 1.25 * INN_FT2 };
const CARAVAN_SIDES: Sides = Sides { least: 55.0, long: 60.0, most_short: 90.0, most_long: 110.0, most_area: 1.2 * CARAVAN_FT2 };
const INN_FT2: f64 = 4000.0;
const CARAVAN_FT2: f64 = 5500.0;
const INN_MOST: usize = 4;
/// The least floor (sq ft) an inn grows round a yard from: `interior::hub::INN_LEAST` squares.
const INN_FROM: f64 = 1500.0;
/// The area (sq ft) two outlines may share and not overlap: none but rounding's.
const TOUCH: f64 = 0.01;
/// The most lots a palace takes in.
const MOST: usize = 4;

/// Whether a building is a palace.
pub fn is_palace(b: &Building) -> bool {
    b.func.is_some_and(|f| catalog::CATALOG[f as usize].key == "palace")
}

/// Whether a building is a monastery.
pub fn is_monastery(b: &Building) -> bool {
    b.func.is_some_and(|f| catalog::CATALOG[f as usize].key == "monastery")
}

/// Whether a building is an inn or a caravanserai.
pub fn is_inn(b: &Building) -> bool {
    b.func.is_some_and(|f| matches!(catalog::CATALOG[f as usize].key, "inn" | "caravanserai"))
}

/// Whether a building is a coaching inn (`interior::hub`'s `inn`: round a galleried yard): an inn or
/// a caravanserai on a rectangle of `interior::hub::YARD` squares or more (its lots merged, `merge`:
/// every caravanserai, a town's biggest inn, a city's two, a metropolis' three: so only one with its
/// ground in layout `l`, `Layout::grounds`; not one drawn by hand, nor one whose outline was).
pub fn is_coaching(l: &Layout, b: &Building) -> bool {
    if !l.grounds.iter().any(|g| g.0 == b.id) || b.poly.len() != 4 || !b.parts.is_empty() || b.keep.is_some() || b.structure != Structure::Roofed || !is_inn(b) {
        return false;
    }
    let o = obb(&b.poly);
    let (long, short) = crate::interior::hub::YARD;
    o.long >= 5.0 * long as f64 - 1.0 && o.short >= 5.0 * short as f64 - 1.0 && area(&b.poly).abs() >= 0.97 * o.long * o.short
}

/// Whether a building takes in the lots beside it (`merge`) or did: a palace, a monastery, a coaching
/// inn.
pub fn merges(l: &Layout, b: &Building) -> bool {
    is_palace(b) || is_monastery(b) || is_coaching(l, b)
}

/// The largest rectangle (axes `u`, `v`) inside convex polygon `poly` (world ft), its sides whole
/// multiples of 5 ft (`odd`: an odd number of them along `u`), its sides within `sides` (`Sides`), as
/// its four corners, anticlockwise in axes with y up when `ccw`.
pub(super) fn largest_rect(poly: &[P], u: P, odd: bool, sides: Sides, ccw: bool) -> Option<(Vec<P>, f64)> {
    let Sides { least, long, most_short, most_long, most_area } = sides;
    let v = [-u[1], u[0]];
    let local: Vec<P> = poly.iter().map(|p| [dot(*p, u), dot(*p, v)]).collect();
    let (y0, y1) = local.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
    // The chord at height y: its ends along u.
    let chord = |y: f64| -> Option<(f64, f64)> {
        let mut xs: Vec<f64> = Vec::new();
        let n = local.len();
        for k in 0..n {
            let (a, b) = (local[k], local[(k + 1) % n]);
            if (a[1] - y) * (b[1] - y) <= 0.0 && (a[1] - b[1]).abs() > 1e-9 {
                xs.push(a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]));
            }
        }
        let lo = xs.iter().copied().fold(f64::MAX, f64::min);
        let hi = xs.iter().copied().fold(f64::MIN, f64::max);
        (hi > lo).then_some((lo, hi))
    };
    // (Its chords' meeting over [ya, yb] is that of its end chords: the polygon is convex.)
    let step = 1.0;
    let n = crate::core::floor((y1 - y0) / step) as usize;
    let ys: Vec<f64> = (0..=n).map(|k| y0 + 0.01 + k as f64 * step).filter(|&y| y < y1 - 0.01).collect();
    let chords: Vec<Option<(f64, f64)>> = ys.iter().map(|&y| chord(y)).collect();
    let mut best: Option<(f64, [f64; 4])> = None;
    for a in 0..ys.len() {
        let Some(ca) = chords[a] else { continue };
        for b in a + 1..ys.len() {
            let Some(cb) = chords[b] else { continue };
            let (lo, hi) = (ca.0.max(cb.0), ca.1.min(cb.1));
            let mut depth = 5.0 * crate::core::floor((ys[b] - ys[a]) / 5.0);
            let mut wide = 5.0 * crate::core::floor((hi - lo) / 5.0);
            // (Cut down to the most it may be: a rectangle within one inside it is inside it too.)
            let (short, longer) = if depth <= wide { (&mut depth, &mut wide) } else { (&mut wide, &mut depth) };
            *short = short.min(most_short);
            *longer = longer.min(most_long);
            // (No bigger than its floor may be: the longer side cut down.)
            while depth * wide > most_area && depth.max(wide) > long {
                if depth >= wide { depth -= 5.0 } else { wide -= 5.0 }
            }
            if odd && (crate::core::round(wide / 5.0) as i64) % 2 == 0 {
                wide -= 5.0;
            }
            if depth.min(wide) < least || depth.max(wide) < long {
                continue;
            }
            let ar = depth * wide;
            if best.is_none_or(|x| ar > x.0) {
                let (mx, my) = (0.5 * (lo + hi), 0.5 * (ys[a] + ys[b]));
                best = Some((ar, [mx - 0.5 * wide, mx + 0.5 * wide, my - 0.5 * depth, my + 0.5 * depth]));
            }
        }
    }
    let (ar, [x0, x1, ya, yb]) = best?;
    let at = |x: f64, y: f64| add(mul(u, x), mul(v, y));
    let mut pts = vec![at(x0, ya), at(x1, ya), at(x1, yb), at(x0, yb)];
    if (area(&pts) > 0.0) != ccw {
        pts.reverse();
    }
    Some((pts, ar))
}

/// The least distance between two polygons' outlines (0 where they cross).
fn gap(a: &[P], b: &[P]) -> f64 {
    (0..b.len()).map(|k| poly_seg_dist(a, b[k], b[(k + 1) % b.len()])).fold(f64::MAX, f64::min)
}

/// Whether `b` overlaps convex polygon `a` (they share more than rounding's area).
fn on(a: &[P], b: &[P]) -> bool {
    overlap(b, a) > TOUCH
}

/// A home a palace may take in: no function, no keep, roofed, of one piece.
fn takes(o: &Building) -> bool {
    o.func.is_none() && o.keep.is_none() && o.parts.is_empty() && o.structure == Structure::Roofed
}

/// The capital's palace `p` of layout `l` with its block cleared (`off`: the site's origin; `gone`:
/// lots already taken in): the largest rectangle 19 squares by 20 or more (up to `CAPITAL_FT2`)
/// square to `u`, an odd number of squares along it, on its own lot or beside it, on squares of its
/// block (the one holding its lot's middle) clear of the streets and roads, the town wall and its
/// towers, the squares, parks and monuments and every building but homes (`takes`: those it covers it
/// takes in), on ground fit to build on, its middle nearest its lot's, then taking in the fewest: the
/// homes it takes in and its corners (anticlockwise in axes with y up when `ccw`). Its own lot's
/// ground it does not cover is open ground.
#[allow(clippy::too_many_arguments)]
fn clear_ground(site: &Site, l: &Layout, p: &Building, u: P, ccw: bool, off: P, gone: &[u32], sizes: &[(usize, usize)], alleys: bool) -> Option<(Vec<usize>, Vec<P>)> {
    const SQ: f64 = 5.0;
    const N: usize = 96;
    let v = [-u[1], u[0]];
    let c0 = centroid(&p.poly);
    let block = l.blocks.iter().find(|b| contains(b, c0))?;
    // (The lattice: N squares each way about its own lot's middle, square (i, j) `o` + i u + j v.)
    let o = sub(c0, add(mul(u, 0.5 * N as f64 * SQ), mul(v, 0.5 * N as f64 * SQ)));
    let at = |x: f64, y: f64| add(o, add(mul(u, x * SQ), mul(v, y * SQ)));
    let cell = |i: usize, j: usize| [at(i as f64, j as f64), at(i as f64 + 1.0, j as f64), at(i as f64 + 1.0, j as f64 + 1.0), at(i as f64, j as f64 + 1.0)];
    let reach = 0.75 * N as f64 * SQ;
    let mut free: Vec<bool> = (0..N * N)
        .map(|k| {
            let q = cell(k % N, k / N);
            q.iter().chain(std::iter::once(&centroid(&q))).all(|pt| contains(block, *pt))
        })
        .collect();
    // (The squares of the box of `pts` on the lattice, `pad` squares more all round.)
    let span = |pts: &[P], pad: f64| {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for q in pts {
            let d = sub(*q, o);
            let (x, y) = (dot(d, u) / SQ, dot(d, v) / SQ);
            (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
        }
        let at_most = |a: f64| (crate::core::floor(a).max(0.0) as usize).min(N);
        (at_most(x0 - pad)..at_most(x1 + 1.0 + pad), at_most(y0 - pad)..at_most(y1 + 1.0 + pad))
    };
    // (The squares a polygon overlaps (of its box on the lattice): not free.)
    let block_out = |poly: &[P], free: &mut Vec<bool>| {
        let (is, js) = span(poly, 0.0);
        for j in js {
            for i in is.clone() {
                if free[j * N + i] && overlap(poly, &cell(i, j)) > TOUCH {
                    free[j * N + i] = false;
                }
            }
        }
    };
    // (Its own lot's squares, for a rectangle beside it.)
    // (Only those about its box can: a square off it overlaps it nowhere.)
    let mut lot = vec![false; N * N];
    let (is, js) = span(&p.poly, 1.0);
    for j in js {
        for i in is.clone() {
            lot[j * N + i] = overlap(&p.poly, &cell(i, j)) > TOUCH;
        }
    }
    let mut homes: Vec<usize> = Vec::new();
    for (k, b) in l.buildings.iter().enumerate() {
        if b.id == p.id || gone.contains(&b.id) || dist(centroid(&b.poly), c0) > reach + 200.0 {
            continue;
        }
        if takes(b) {
            homes.push(k);
        } else {
            block_out(&b.poly, &mut free);
        }
    }
    for poly in l.plazas.iter().map(|q| &q.pts).chain(l.parks.iter().map(|q| &q.pts)).chain(&l.monuments) {
        if poly.iter().any(|q| dist(*q, c0) < reach + 400.0) {
            block_out(poly, &mut free);
        }
    }
    // (Clear of the streets and roads (a monastery's: but the alleys inside its block, `alleys`, which it
    // cuts: `is_alley`), the town wall and its towers.)
    let lanes: Vec<(P, P, f64)> = l
        .roads
        .iter()
        .filter(|r| !(alleys && is_alley(r, block)))
        .flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * w + 1.0)))
        .chain(l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1], 8.0))))
        .filter(|&(a, b, _)| super::geom::seg_dist(c0, a, b) < reach + 100.0)
        .collect();
    // (Each lane tried only on the squares about its box: those further off are clear of it.)
    let mut by_lanes = vec![true; N * N];
    for &(a, b, w) in &lanes {
        let (is, js) = span(&[a, b], w / SQ + 2.0);
        for j in js {
            for i in is.clone() {
                let k = j * N + i;
                if free[k] && by_lanes[k] && poly_seg_dist(&cell(i, j), a, b) < w {
                    by_lanes[k] = false;
                }
            }
        }
    }
    for (k, f) in free.iter_mut().enumerate() {
        if *f {
            let q = cell(k % N, k / N);
            *f = by_lanes[k] && l.towers.iter().all(|t| dist(*t, centroid(&q)) > 20.0);
        }
    }
    // (How many squares of the box from (0, 0) to (i, j) are free.)
    let sums = |of: &[bool]| {
        let mut sum = vec![0usize; (N + 1) * (N + 1)];
        for j in 0..N {
            for i in 0..N {
                sum[(j + 1) * (N + 1) + i + 1] = of[j * N + i] as usize + sum[j * (N + 1) + i + 1] + sum[(j + 1) * (N + 1) + i] - sum[j * (N + 1) + i];
            }
        }
        sum
    };
    let (sum, lots) = (sums(&free), sums(&lot));
    let count = |sum: &[usize], i0: usize, j0: usize, i1: usize, j1: usize| sum[j1 * (N + 1) + i1] + sum[j0 * (N + 1) + i0] - sum[j0 * (N + 1) + i1] - sum[j1 * (N + 1) + i0];
    // (All free, and on its lot or a square from it.)
    let fits_at = |i0: usize, j0: usize, w: usize, d: usize| count(&sum, i0, j0, i0 + w, j0 + d) == w * d && count(&lots, i0.saturating_sub(1), j0.saturating_sub(1), (i0 + w + 1).min(N), (j0 + d + 1).min(N)) > 0;
    // Its sizes, biggest first (to the footprint it grows to), the squarer first.
    // (Either way about: an odd side for its front, round a court behind it.)
    let mid = 0.5 * N as f64;
    let rect = |i0: usize, j0: usize, w: usize, d: usize| vec![at(i0 as f64, j0 as f64), at((i0 + w) as f64, j0 as f64), at((i0 + w) as f64, (j0 + d) as f64), at(i0 as f64, (j0 + d) as f64)];
    let local = |poly: &[P]| -> Vec<P> { poly.iter().map(|p| sub(*p, off)).collect() };
    for &(w, d) in sizes {
        // (Each place it fits: how far its middle is from its lot's.)
        let mut fits: Vec<(f64, usize, usize)> = Vec::new();
        for j0 in 0..=N - d {
            for i0 in 0..=N - w {
                if !fits_at(i0, j0, w, d) {
                    continue;
                }
                let m = [i0 as f64 + 0.5 * w as f64 - mid, j0 as f64 + 0.5 * d as f64 - mid];
                fits.push((m[0] * m[0] + m[1] * m[1], i0, j0));
            }
        }
        fits.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
        // The nearest that will do, then the one taking in the fewest homes (counted only for those
        // that will do: thousands of places, most on ground unfit to build on).
        let mut g = 0;
        while g < fits.len() {
            let e = g + fits[g..].iter().take_while(|f| f.0.total_cmp(&fits[g].0).is_eq()).count();
            let mut pick: Option<((usize, usize, usize), Vec<usize>, Vec<P>)> = None;
            for &(_, i0, j0) in &fits[g..e] {
                let mut r = rect(i0, j0, w, d);
                if !(if alleys { site.terraced(&local(&r)) } else { site.buildable(&local(&r)) }) {
                    continue;
                }
                let fewest = (homes.iter().filter(|&&k| on(&r, &l.buildings[k].poly)).count(), i0, j0);
                if (area(&r) > 0.0) != ccw {
                    r.reverse();
                }
                let taken: Vec<usize> = homes.iter().copied().filter(|&k| on(&r, &l.buildings[k].poly)).collect();
                // (A monastery's on no other building at all, its squares' slivers summed (as `follow`
                // keeps it), nor stranding a neighbour.)
                if alleys && (l.buildings.iter().any(|b| b.id != p.id && !gone.contains(&b.id) && !takes(b) && dist(centroid(&b.poly), c0) < reach + 200.0 && on(&r, &b.poly)) || strands(l, &r, block, p.id, &taken)) {
                    continue;
                }
                if pick.as_ref().is_none_or(|x| fewest < x.0) {
                    pick = Some((fewest, taken, r));
                }
            }
            if let Some((_, taken, r)) = pick {
                return Some((taken, r));
            }
            g = e;
        }
    }
    None
}

/// The capital's palace's sizes (squares: across its front, deep) when it clears its block
/// (`clear_ground`): round a court, an odd number of squares across its front, up to `CAPITAL_FT2`,
/// biggest first, the squarer first.
fn capital_sizes() -> Vec<(usize, usize)> {
    const SQ: f64 = 5.0;
    let front = |a: usize, b: usize| a % 2 == 1 && courted(a, b);
    let mut sizes: Vec<(usize, usize)> = (17..=31).flat_map(|w| (17..=31).map(move |d| (w, d))).filter(|&(w, d)| (front(w, d) || front(d, w)) && (w * d) as f64 * SQ * SQ <= CAPITAL_FT2).collect();
    sizes.sort_by_key(|&(w, d)| (std::cmp::Reverse(w * d), w.abs_diff(d), w));
    sizes
}

/// A monastery's sizes (squares) when it clears its block (`clear_ground`, where the lots beside it
/// give no footprint round a cloister): `MONASTERY_SIDES`, nearest `MONASTERY_FT2` first, the squarer
/// first.
fn monastery_sizes() -> Vec<(usize, usize)> {
    sizes_of(MONASTERY_SIDES, MONASTERY_FT2)
}

/// The sizes (squares) a footprint of sides `sides` may clear its block for, nearest `ft2` first, the
/// squarer first (`monastery_sizes`; a coaching inn's).
fn sizes_of(sides: Sides, ft2: f64) -> Vec<(usize, usize)> {
    const SQ: f64 = 5.0;
    let n = |x: f64| crate::core::round(x / SQ) as usize;
    let (lo, hi, short, long) = (n(sides.least), n(sides.most_long), n(sides.most_short), n(sides.long));
    let mut sizes: Vec<(usize, usize)> =
        (lo..=hi).flat_map(|w| (lo..=hi).map(move |d| (w, d))).filter(|&(w, d)| w.max(d) >= long && w.min(d) <= short && (w * d) as f64 * SQ * SQ <= sides.most_area).collect();
    let target = ft2 / (SQ * SQ);
    sizes.sort_by_key(|&(w, d)| (crate::core::round(((w * d) as f64 - target).abs()) as i64, w.abs_diff(d), w, d));
    sizes
}

/// The capital's palace's forecourt (`approach`): paved ground walled round before its front door,
/// the gate in its outer wall on the palace's axis, `GATE_SQ` squares wide (world ft); with no yard
/// where none could be had, its front all the same.
#[derive(Clone, Debug)]
pub struct Forecourt {
    /// The palace's id.
    pub palace: u32,
    /// The palace's front from end to end (an odd number of squares long), its door in the middle.
    pub front: [P; 2],
    /// The way out from the front (unit).
    pub out: P,
    /// How deep it is out from the front, and how far in from each end of the front its sides stand
    /// (squares).
    pub depth: f64,
    pub inset: f64,
    /// Its ground: its corner out from the front's first end, the front's there and at its other
    /// end, and the one out from that.
    pub yard: Vec<P>,
    /// The paved way on from its gate to the nearest street (`way`, a street of its own): its centre
    /// line; empty where the gate opens onto a street.
    pub way: Vec<P>,
}

/// Whether a capital's palace `w` squares across its front and `d` deep behind it stands round a
/// court 7 squares across or more each way (`interior::hub::palace`).
fn courted(w: usize, d: usize) -> bool {
    crate::interior::hub::courted(w, d)
}

/// Whether rectangle `r` has a side its front (an odd number of squares, an even side a square
/// short) round a court behind it (`courted`).
fn courts(r: &[P]) -> bool {
    (0..4).any(|k| {
        let (w, d) = (crate::core::round(dist(r[k], r[(k + 1) % 4]) / 5.0) as usize, crate::core::round(dist(r[(k + 1) % 4], r[(k + 2) % 4]) / 5.0) as usize);
        courted(w - (w % 2 == 0) as usize, d)
    })
}
/// A forecourt's gate (squares).
pub const GATE_SQ: f64 = 3.0;
/// The depth a forecourt is wanted (squares): a shallower one only where drawing the palace back
/// from its front gives none so deep.
const FORE_WANT: usize = 4;

/// The forecourt `depth` squares deep before front `a` (along `e` for `wide` squares; out: `out`),
/// its sides `inset` squares in from the front's ends (`Forecourt::yard`).
fn yard_of(a: P, e: P, out: P, wide: usize, depth: f64, inset: f64) -> Vec<P> {
    let at = |o: f64, t: f64| add(a, add(mul(e, 5.0 * t), mul(out, 5.0 * o)));
    vec![at(depth, inset), at(0.0, inset), at(0.0, wide as f64 - inset), at(depth, wide as f64 - inset)]
}

/// A road's cut where a forecourt takes its end (`yard_clear`): its index and its points then
/// (none: gone).
type Cut = (usize, Option<Vec<P>>);

/// What forecourt `yard` before the front from `a` (along `e`, `wide` squares; out: `out`, `depth`
/// squares) of palace `id` asks of the streets: none reaches into it but those that run up to the
/// front (square to it, ending before it or at it), each cut back to its outer wall, where the
/// street stops at its gate (`Cut`s); None where something else stands in it: a building, a square,
/// a park or a monument, the town wall or a tower, a street running past.
#[allow(clippy::too_many_arguments)]
fn yard_clear(l: &Layout, id: u32, yard: &[P], a: P, e: P, out: P, wide: usize, depth: f64) -> Option<Vec<Cut>> {
    let c0 = centroid(yard);
    if l.buildings.iter().any(|o| o.id != id && dist(centroid(&o.poly), c0) < 400.0 && overlap(&o.poly, yard) > TOUCH) {
        return None;
    }
    if l.plazas.iter().map(|q| &q.pts).chain(l.parks.iter().map(|q| &q.pts)).chain(&l.monuments).any(|q| overlap(q, yard) > TOUCH) {
        return None;
    }
    if l.walls.iter().any(|w| w.windows(2).any(|s| poly_seg_dist(yard, s[0], s[1]) < 6.0)) {
        return None;
    }
    if l.towers.iter().chain(&l.gate_towers).chain(l.extra_towers.iter().map(|t| &t.0)).any(|t| contains(yard, *t) || (0..4).any(|k| super::geom::seg_dist(*t, yard[k], yard[(k + 1) % 4]) <= 16.0)) {
        return None;
    }
    let s_of = |x: P| dot(sub(x, a), out);
    let t_of = |x: P| dot(sub(x, a), e);
    let (deep, long) = (5.0 * depth, 5.0 * wide as f64);
    let mut cuts = Vec::new();
    for (ri, (pts, class, w)) in l.roads.iter().enumerate() {
        if pts.windows(2).all(|s| super::geom::seg_dist(c0, s[0], s[1]) > 400.0 || poly_seg_dist(yard, s[0], s[1]) >= 0.5 * w) {
            continue;
        }
        // (A street's end running up to the front: square to it, its near end before the front
        // or at it, its far end beyond the forecourt.)
        let d = sub(pts[pts.len() - 1], pts[0]);
        let square = pts.len() == 2 && *class == 3 && dot(d, e).abs() <= 0.5 * super::geom::len(d);
        let (near, far) = if s_of(pts[0]) <= s_of(pts[1]) { (0, 1) } else { (1, 0) };
        if !square {
            return None;
        }
        let (sn, sf, tn) = (s_of(pts[near]), s_of(pts[far]), t_of(pts[near]));
        if sn < -(0.5 * w + 5.0) || !(0.0..=long).contains(&tn) {
            return None;
        }
        if sf <= deep + 0.5 * w + 5.0 {
            cuts.push((ri, None));
            continue;
        }
        let k = (deep + 0.5 * w + 0.05 - sn) / (sf - sn);
        let mut cut = pts.clone();
        cut[near] = add(pts[near], mul(sub(pts[far], pts[near]), k.max(0.0)));
        cuts.push((ri, Some(cut)));
    }
    Some(cuts)
}

/// Streets `roads` with `cuts` made.
fn cut_roads(roads: &[(Vec<P>, u8, f64)], cuts: &[Cut]) -> Vec<(Vec<P>, u8, f64)> {
    roads
        .iter()
        .enumerate()
        .filter_map(|(k, r)| match cuts.iter().find(|c| c.0 == k) {
            Some((_, Some(pts))) => Some((pts.clone(), r.1, r.2)),
            Some((_, None)) => None,
            None => Some(r.clone()),
        })
        .collect()
}

/// The least distance between segments a-b and c-d (0 where they cross).
fn seg_seg(a: P, b: P, c: P, d: P) -> f64 {
    let o = |p: P, q: P, r: P| super::geom::cross(sub(q, p), sub(r, p));
    if o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0 {
        return 0.0;
    }
    let sd = super::geom::seg_dist;
    sd(a, c, d).min(sd(b, c, d)).min(sd(c, a, b)).min(sd(d, a, b))
}

/// Whether the strip `half` ft either side of segment a-b (world ft) is dry ground (`Site::wet`: off
/// the water and a river's channel; points every 5 ft or less, on its centre line and its edges), but
/// where it lies on one of streets `segs` (their ground, a bridge's too).
fn dry(site: &Site, off: P, a: P, b: P, half: f64, segs: &[(P, P, f64)]) -> bool {
    let d = sub(b, a);
    let n = mul([-d[1], d[0]], 1.0 / dist(a, b).max(1e-9));
    let steps = crate::core::ceil(dist(a, b) / 5.0).max(1.0) as usize;
    (0..=steps).all(|j| {
        let c = add(a, mul(d, j as f64 / steps as f64));
        [-half, 0.0, half].iter().all(|&t| {
            let q = add(c, mul(n, t));
            !site.wet(sub(q, off)) || segs.iter().any(|&(s0, s1, w)| super::geom::seg_dist(q, s0, s1) <= 0.5 * w)
        })
    })
}

/// Whether forecourt `fc` (its yard) or the ground out from its gate (3 squares, as wide as the
/// gate) is on the water (`Site::wet`): a forecourt facing the water.
fn wet_face(site: &Site, off: P, fc: &Forecourt) -> bool {
    let half = 2.5 * GATE_SQ;
    let m = mul(add(fc.yard[0], fc.yard[3]), 0.5);
    let y = &fc.yard;
    !(dry(site, off, y[0], y[1], 0.0, &[]) && dry(site, off, y[1], y[2], 0.0, &[]) && dry(site, off, y[2], y[3], 0.0, &[]) && dry(site, off, mul(add(y[0], y[1]), 0.5), mul(add(y[2], y[3]), 0.5), 0.0, &[]))
        || !dry(site, off, m, add(m, mul(fc.out, 15.0)), half, &[])
}

/// A paved way as wide as forecourt `fc`'s gate from the gate to the nearest of streets `roads`,
/// where it doesn't open onto one: straight out from it where that is about as short, else out a
/// square or two and on to the nearest point of a street; on dry ground (`dry`), clear of every
/// building, the town wall and its towers, the monuments and the forecourt (the palace's own:
/// `rect`; `homes`: but the homes, which it takes in). Its centre line (world ft); empty where the
/// gate opens onto a street, None where no way is clear.
#[allow(clippy::too_many_arguments)]
fn way(site: &Site, off: P, l: &Layout, roads: &[(Vec<P>, u8, f64)], fc: &Forecourt, rect: &[P], homes: bool) -> Option<Vec<P>> {
    let half = 2.5 * GATE_SQ;
    let m = mul(add(fc.yard[0], fc.yard[3]), 0.5);
    let segs: Vec<(P, P, f64)> = roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], *w))).filter(|&(a, b, _)| super::geom::seg_dist(m, a, b) < 300.0).collect();
    if segs.iter().any(|&(a, b, w)| super::geom::seg_dist(m, a, b) <= 0.5 * w + 1.0) {
        return Some(Vec::new());
    }
    let near: Vec<&[P]> = l.buildings.iter().filter(|o| o.id != fc.palace && !(homes && takes(o)) && dist(centroid(&o.poly), m) < 500.0).map(|o| &o.poly[..]).chain([rect]).collect();
    let clear = |pts: &[P]| {
        pts.windows(2).enumerate().all(|(k, s)| {
            near.iter().all(|o| poly_seg_dist(o, s[0], s[1]) >= half - 0.5)
                && l.walls.iter().all(|w| w.windows(2).all(|q| seg_seg(q[0], q[1], s[0], s[1]) >= half + 4.0))
                && l.towers.iter().chain(&l.gate_towers).chain(l.extra_towers.iter().map(|t| &t.0)).all(|t| super::geom::seg_dist(*t, s[0], s[1]) >= half + 15.0)
                && l.monuments.iter().all(|q| poly_seg_dist(q, s[0], s[1]) >= half)
                && (k == 0 || poly_seg_dist(&fc.yard, s[0], s[1]) >= half - 0.5)
                && dry(site, off, s[0], s[1], half, &segs)
        })
    };
    // (The nearest point of segment a-b to p.)
    let foot = |p: P, a: P, b: P| {
        let ab = sub(b, a);
        let t = (dot(sub(p, a), ab) / dot(ab, ab).max(1e-9)).clamp(0.0, 1.0);
        add(a, mul(ab, t))
    };
    let length = |pts: &[P]| pts.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>();
    // Straight out: where the line out from the gate first meets a street's centre line.
    let ray = segs
        .iter()
        .filter_map(|&(a, b, _)| {
            let (d, ab) = (fc.out, sub(b, a));
            let den = super::geom::cross(d, ab);
            if den.abs() < 1e-9 {
                return None;
            }
            let (t, u) = (super::geom::cross(sub(a, m), ab) / den, super::geom::cross(sub(a, m), d) / den);
            (t > 0.0 && t <= 250.0 && (0.0..=1.0).contains(&u)).then_some(t)
        })
        .fold(f64::MAX, f64::min);
    let straight = (ray < f64::MAX).then(|| vec![m, add(m, mul(fc.out, ray))]).filter(|w| clear(w));
    let mut bent: Option<Vec<P>> = None;
    for j in [1.0, 2.0] {
        let kn = add(m, mul(fc.out, 5.0 * j));
        for &(a, b, _) in &segs {
            let q = foot(kn, a, b);
            let w = vec![m, kn, q];
            if dist(kn, q) > 1.0 && bent.as_ref().is_none_or(|x| length(&w) < length(x) - 1e-9) && clear(&w) {
                bent = Some(w);
            }
        }
    }
    match (straight, bent) {
        (Some(s), Some(b)) => Some(if length(&s) <= 1.25 * length(&b) { s } else { b }),
        (s, b) => s.or(b),
    }
}

/// Each capital's palace of layout `l` (world ft; `off`: the site's origin; `capital`: whether it
/// is the capital's) given its front and its forecourt (`Forecourt`). An even front loses a square
/// at one end (an odd number of squares across, for its axis); the palace is drawn back from its
/// front by up to 4 squares (keeping its court behind it) where that gives a forecourt `FORE_WANT`
/// squares deep or a deeper one. The forecourt is 6 squares deep down to 2, a square in from the
/// front's ends, else 2 or 3, on ground clear of everything but the ends of streets that run up to
/// the front, which then stop at its outer wall. A paved way as wide as its gate runs on to the
/// nearest street where the gate doesn't open onto one (`way`: a street of its own, on dry ground).
/// Its front is the side it stands round its court from, then one with a forecourt before it, then
/// one whose forecourt doesn't face the water (`wet_face`) with a way on to a street, else one with a
/// way, the shortest, then a long side, then one not drawn back, nearest a street (on one: within
/// 80 ft). Palaces `kept` (their footprint as generated, the town laid out anew by hand) keep it:
/// only an odd side is their front, none drawn back, their front as generated (in `generated`)
/// first where a forecourt is had there. One not round a court has its front, no forecourt (a
/// forecourt is walled round as its court's palace is: `interior::court_plan`).
pub(super) fn approach(site: &Site, l: &mut Layout, off: P, capital: bool, kept: &[u32], generated: Option<&Layout>) {
    if !capital {
        return;
    }
    let local = |poly: &[P]| -> Vec<P> { poly.iter().map(|p| sub(*p, off)).collect() };
    let sq = |x: f64| crate::core::round(x / 5.0) as usize;
    let palaces: Vec<usize> = (0..l.buildings.len())
        .filter(|&i| {
            let b = &l.buildings[i];
            is_palace(b) && b.structure == Structure::Roofed && b.ward != Ward::Castle && b.parts.is_empty() && b.poly.len() == 4
        })
        .collect();
    for pi in palaces {
        let (id, r) = (l.buildings[pi].id, l.buildings[pi].poly.clone());
        let reshape = !kept.contains(&id);
        let gap = |m: P| l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |s| super::geom::seg_dist(m, s[0], s[1]) - 0.5 * w)).fold(f64::MAX, f64::min);
        // (A kept palace's front as generated: its middle.)
        let was = generated.filter(|_| !reshape).and_then(|g| g.forecourts.iter().find(|f| f.palace == id)).map(|f| mul(add(f.front[0], f.front[1]), 0.5));
        // Each side as its front: its rank, the footprint, the forecourt and the streets' cuts.
        type Pick = (((bool, bool, bool, bool, bool, i64), bool, bool, bool, bool, i64, usize), Vec<P>, Forecourt, Vec<Cut>, Option<(Vec<P>, bool)>);
        let mut best: Option<Pick> = None;
        for k in 0..4 {
            let (a, mut b, mut c, d) = (r[k], r[(k + 1) % 4], r[(k + 2) % 4], r[(k + 3) % 4]);
            let e = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
            let n = mul(sub(d, a), 1.0 / dist(a, d).max(1e-9));
            let out = mul(n, -1.0);
            let (mut wide, deep) = (sq(dist(a, b)), sq(dist(a, d)));
            if wide % 2 == 0 {
                if !reshape {
                    continue;
                }
                (b, c, wide) = (sub(b, mul(e, 5.0)), sub(c, mul(e, 5.0)), wide - 1);
            }
            let court = courted(wide, deep);
            // (Drawn back by `back` squares: its deepest forecourt then.)
            let mut pick: Option<(usize, usize, f64, Vec<P>, Vec<Cut>)> = None;
            for back in [0usize, 2, 3, 4] {
                if back > 0 && (!reshape || deep < 11 + back || court && !courted(wide, deep - back)) {
                    break;
                }
                let fa = add(a, mul(n, 5.0 * back as f64));
                let found = (2..=6usize).rev().find_map(|fd| {
                    [1.0, 2.0, 3.0].into_iter().find_map(|i| {
                        let y = yard_of(fa, e, out, wide, fd as f64, i);
                        yard_clear(l, id, &y, fa, e, out, wide, fd as f64).map(|cuts| (fd, i, y, cuts))
                    })
                });
                let Some((fd, i, y, cuts)) = found else { continue };
                if pick.as_ref().is_none_or(|p| fd > p.1) {
                    pick = Some((back, fd, i, y, cuts));
                }
                if fd >= FORE_WANT {
                    break;
                }
            }
            let back = pick.as_ref().map_or(0, |p| p.0);
            let (fa, fb) = (add(a, mul(n, 5.0 * back as f64)), add(b, mul(n, 5.0 * back as f64)));
            let mut rect = r.clone();
            (rect[k], rect[(k + 1) % 4], rect[(k + 2) % 4]) = (fa, fb, c);
            let g = gap(mul(add(r[k], r[(k + 1) % 4]), 0.5));
            let (fc, cuts) = match pick {
                Some((_, fd, i, y, cuts)) => (Forecourt { palace: id, front: [fa, fb], out, depth: fd as f64, inset: i, yard: y, way: Vec::new() }, cuts),
                None => (Forecourt { palace: id, front: [fa, fb], out, depth: 0.0, inset: 0.0, yard: Vec::new(), way: Vec::new() }, Vec::new()),
            };
            // (Its way on to a street, with the streets it takes the ends of cut.)
            // (Its way on to a street, with the streets it takes the ends of cut; else one through
            // homes, which it takes in.)
            let path = (!fc.yard.is_empty())
                .then(|| {
                    let roads = cut_roads(&l.roads, &cuts);
                    way(site, off, l, &roads, &fc, &rect, false).map(|w| (w, false)).or_else(|| way(site, off, l, &roads, &fc, &rect, true).map(|w| (w, true)))
                })
                .flatten();
            let same = was.is_some_and(|m| dist(m, mul(add(r[k], r[(k + 1) % 4]), 0.5)) < 1.0);
            // (Facing the water, or with no dry way on to a street: only where no other side has
            // neither, then the shortest way.)
            let wet = !fc.yard.is_empty() && wet_face(site, off, &fc);
            let long = if wet { path.as_ref().map_or(0, |p| crate::core::round(p.0.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>()) as i64) } else { 0 };
            let none = path.is_none();
            let rank = ((!court, fc.yard.is_empty(), was.is_some() && !same, wet || none, none, long), wide < deep, path.as_ref().is_some_and(|p| p.1), back > 0, g >= 80.0, crate::core::round(g) as i64, k);
            if best.as_ref().is_none_or(|x| rank < x.0) {
                best = Some((rank, rect, fc, cuts, path));
            }
        }
        let Some((rank, rect, mut fc, cuts, path)) = best else { continue };
        if reshape {
            let b = &mut l.buildings[pi];
            b.pad_ft = site.pad(&local(&rect));
            b.poly = rect;
        }
        // (Not round a court: no forecourt, as none would be walled round.)
        if fc.yard.is_empty() || rank.0.0 {
            (fc.depth, fc.inset, fc.yard) = (0.0, 0.0, Vec::new());
            l.forecourts.push(fc);
            continue;
        }
        l.roads = cut_roads(&l.roads, &cuts);
        if let Some((w, _)) = path.filter(|w| w.0.len() >= 2) {
            // (The homes it runs through taken in.)
            let half = 2.5 * GATE_SQ - 0.5;
            l.buildings.retain(|o| o.id == id || !takes(o) || w.windows(2).all(|s| poly_seg_dist(&o.poly, s[0], s[1]) >= half));
            l.roads.push((w.clone(), 3, 5.0 * GATE_SQ));
            fc.way = w;
        }
        l.forecourts.push(fc);
    }
}

/// Each palace and monastery of layout `l` (world ft; `off`: the settlement's centre, the site's
/// origin) given its footprint: the lots beside it taken in (`merge`'s rules), the largest rectangle
/// the whole holds. `capital`: the capital's (its palace round a court: bigger, an odd number of
/// squares across). A monastery with no rectangle big enough for its cloister keeps its lot. A town
/// laid out anew by hand (`generated`: as generated) takes in no lots for an inn that took in none
/// as generated (on lots laid out again a hair apart, it might find some).
pub(super) fn merge(site: &Site, l: &mut Layout, off: P, capital: bool, tier: crate::t0::settle::Tier, skip: &[u32], generated: Option<&Layout>) {
    let local = |poly: &[P]| -> Vec<P> { poly.iter().map(|p| sub(*p, off)).collect() };
    let mut palaces: Vec<usize> = (0..l.buildings.len())
        .filter(|&i| (is_palace(&l.buildings[i]) || is_monastery(&l.buildings[i])) && l.buildings[i].structure == Structure::Roofed && l.buildings[i].ward != Ward::Castle && l.buildings[i].parts.is_empty() && !skip.contains(&l.buildings[i].id))
        .collect();
    let plain = |i: &usize| generated.is_some_and(|g| g.buildings.iter().any(|b| b.id == l.buildings[*i].id && is_inn(b) && !g.grounds.iter().any(|x| x.0 == b.id)));
    let inns: Vec<usize> = coaching_inns(l, tier, skip).into_iter().filter(|i| !plain(i)).collect();
    palaces.extend(inns);
    let mut gone: Vec<u32> = Vec::new();
    for pi in palaces {
        let p = l.buildings[pi].clone();
        let monk = is_monastery(&p);
        // (A coaching inn: its ward's homes as a palace's, its ground kept as a monastery's.)
        let inn = is_inn(&p);
        let cara = inn && p.func.is_some_and(|f| catalog::CATALOG[f as usize].key == "caravanserai");
        let capital = capital && !monk && !inn;
        let most = if monk {
            MONASTERY_MOST
        } else if inn {
            INN_MOST
        } else {
            MOST
        };
        // (Ground fit to build on: a monastery's on a terrace, `Site::terraced`.)
        let fit = |r: &[P]| if monk { site.terraced(&local(r)) } else { site.buildable(&local(r)) };
        let target = if monk {
            MONASTERY_FT2
        } else if cara {
            CARAVAN_FT2
        } else if inn {
            INN_FT2
        } else if capital {
            CAPITAL_FT2
        } else {
            CITY_FT2
        };
        let ccw = area(&p.poly) > 0.0;
        let u = obb(&p.poly).axis;
        // (The streets and alleys near it: nothing it takes in may reach across one.)
        let c0 = centroid(&p.poly);
        let lanes: Vec<(P, P, f64)> = l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], *w))).filter(|&(a, b, _)| super::geom::seg_dist(c0, a, b) < 400.0).collect();
        let clear = |h: &[P]| lanes.iter().all(|&(a, b, w)| poly_seg_dist(h, a, b) >= 0.5 * w - 1.0);
        // (A side's middle within 8 ft of a street's edge.)
        let fronts = |r: &[P]| {
            (0..r.len()).any(|k| {
                let m = mul(add(r[k], r[(k + 1) % r.len()]), 0.5);
                lanes.iter().any(|&(a, b, w)| super::geom::seg_dist(m, a, b) < 0.5 * w + 8.0)
            })
        };
        // (A home of its ward it may take in; a monastery any home.)
        let home = |o: &Building| takes(o) && (monk || o.ward == p.ward) && !gone.contains(&o.id);
        let rect_of = |set: &[usize], l: &Layout, sides: Sides| -> Option<(Vec<P>, f64, Vec<P>)> {
            let mut pts: Vec<P> = p.poly.clone();
            for &k in set {
                pts.extend(l.buildings[k].poly.iter().copied());
            }
            let h = hull(pts);
            let (r, ar) = largest_rect(&h, u, capital, sides, ccw)?;
            Some((r, ar, h))
        };
        // Lot `k` taken in, with every other the hull of them all then covers (homes all, `MOST` at
        // most), where that hull reaches across no street: the set, its rectangle and its floor.
        let grow = |set: &[usize], k: usize, l: &Layout, sides: Sides| -> Option<(Vec<usize>, Vec<P>, f64)> {
            let mut with = set.to_vec();
            with.push(k);
            loop {
                let (r, ar, h) = rect_of(&with, l, sides)?;
                if !clear(&h) {
                    return None;
                }
                let more: Vec<usize> = (0..l.buildings.len())
                    // (A monastery stands on no graveyard either.)
                    .filter(|&j| j != pi && !with.contains(&j) && !gone.contains(&l.buildings[j].id) && (monk || l.buildings[j].structure != Structure::Open))
                    .filter(|&j| dist(centroid(&l.buildings[j].poly), c0) < 400.0 && on(&h, &l.buildings[j].poly))
                    .collect();
                if more.is_empty() {
                    return Some((with, r, ar));
                }
                if with.len() + more.len() > most || !more.iter().all(|&j| home(&l.buildings[j])) {
                    return None;
                }
                with.extend(more);
            }
        };
        // The lots it takes in and the rectangle they give, for sides `sides`.
        let attempt = |sides: Sides| -> Option<(Vec<usize>, Vec<P>)> {
            let mut set: Vec<usize> = Vec::new();
            // (A capital's own lot may be too small for its ranges: no rectangle until it takes in more.)
            let (mut rect, mut best) = rect_of(&set, l, sides).map_or((None, 0.0), |(r, ar, _)| (Some(r), ar));
            while best < target && set.len() < most {
                let members: Vec<Vec<P>> = std::iter::once(p.poly.clone()).chain(set.iter().map(|&k| l.buildings[k].poly.clone())).collect();
                let mut pick: Option<(f64, Vec<usize>, Vec<P>)> = None;
                for (k, o) in l.buildings.iter().enumerate() {
                    if k == pi || set.contains(&k) || !home(o) || dist(centroid(&o.poly), c0) > 250.0 || !members.iter().any(|m| gap(m, &o.poly) < 2.0) {
                        continue;
                    }
                    let Some((with, r, ar)) = grow(&set, k, l, sides) else { continue };
                    // (Worth taking: the rectangle grows by much of the lots' own floor.)
                    let added: f64 = with.iter().filter(|j| !set.contains(j)).map(|&j| area(&l.buildings[j].poly).abs()).sum();
                    if ar - best < 0.4 * added || !fit(&r) {
                        continue;
                    }
                    // (The smallest that reaches the footprint it grows to, else the biggest.)
                    let better = |x: f64| if (ar >= target) != (x >= target) { ar >= target } else if ar >= target { ar < x } else { ar > x };
                    if pick.as_ref().is_none_or(|x| better(x.0)) {
                        pick = Some((ar, with, r));
                    }
                }
                let Some((ar, with, r)) = pick else { break };
                set = with;
                (best, rect) = (ar, Some(r));
            }
            let rect = rect?;
            // (Its own lot's rectangle, where it takes in none: square to its axis all the same, where
            // that keeps most of its floor; a capital's where it keeps 60%, for its symmetry; another's
            // whatever it keeps where its lot is bigger than it may be.)
            if !monk && set.is_empty() && best < if capital { 0.6 } else { 0.8 } * area(&p.poly).abs() && (capital || inn || area(&p.poly).abs() <= CITY_FT2) {
                return None;
            }
            // (A coaching inn's front on a street, for its carriage arch and its door.)
            if inn && !fronts(&rect) {
                return None;
            }
            fit(&rect).then_some((set, rect))
        };
        // (A capital's round a court: on the lots beside it, else clearing its block; else, where
        // nothing round a court can be had at all, a front and a back range.)
        // (A monastery that clears its block stands across its alleys: `cut_alleys`.)
        let mut cleared = false;
        let found = if monk {
            attempt(MONASTERY_SIDES).or_else(|| {
                cleared = true;
                clear_ground(site, l, &p, u, ccw, off, &gone, &monastery_sizes(), true)
            })
        } else if inn {
            // (Where the lots beside it give no yard, clearing the homes of its block as a monastery
            // does: on a street all the same.)
            let (sides, ft2) = if cara { (CARAVAN_SIDES, CARAVAN_FT2) } else { (INN_SIDES, INN_FT2) };
            attempt(sides).or_else(|| clear_ground(site, l, &p, u, ccw, off, &gone, &sizes_of(sides, ft2), false).filter(|f| fronts(&f.1)))
        } else if capital {
            attempt(CAPITAL_SIDES).filter(|f| courts(&f.1)).or_else(|| clear_ground(site, l, &p, u, ccw, off, &gone, &capital_sizes(), false)).or_else(|| attempt(CAPITAL_LEAST))
        } else {
            attempt(CITY_SIDES)
        };
        let Some((set, rect)) = found else { continue };
        // (A monastery's: the alleys of its block it stands across cut back to it.)
        // Its own lot and those it took in its ground (`Layout::grounds`).
        if monk || inn {
            if cleared {
                cut_alleys(l, &rect);
            }
            let id = p.id;
            l.grounds.push((id, p.poly.clone()));
            for &k in &set {
                l.grounds.push((id, l.buildings[k].poly.clone()));
            }
        }
        let b = &mut l.buildings[pi];
        b.pad_ft = site.pad(&local(&rect));
        b.poly = rect;
        gone.extend(set.iter().map(|&k| l.buildings[k].id));
    }
    if !gone.is_empty() {
        l.buildings.retain(|b| !gone.contains(&b.id));
    }
}

/// The inns of layout `l` that grow round a yard (`merge`, by index): every caravanserai; of the inns
/// of `INN_FROM` sq ft or more, roofed, of one piece, on a street (a side within 8 ft of one), the
/// biggest (the most floor, then the lowest id): a town's one, a city's two, a metropolis' three, less
/// those kept as generated (`skip`, `follow`). None in a village.
fn coaching_inns(l: &Layout, tier: crate::t0::settle::Tier, skip: &[u32]) -> Vec<usize> {
    use crate::t0::settle::Tier;
    let n: usize = match tier {
        Tier::Village => 0,
        Tier::Town => 1,
        Tier::City => 2,
        Tier::Metropolis => 3,
    };
    let ok = |b: &Building| b.structure == Structure::Roofed && b.ward != Ward::Castle && b.parts.is_empty() && b.keep.is_none() && !skip.contains(&b.id);
    let key = |b: &Building| b.func.map(|f| catalog::CATALOG[f as usize].key);
    let mut out: Vec<usize> = (0..l.buildings.len()).filter(|&i| ok(&l.buildings[i]) && key(&l.buildings[i]) == Some("caravanserai")).collect();
    let kept = l.buildings.iter().filter(|b| skip.contains(&b.id) && key(b) == Some("inn")).count();
    let street = |b: &Building| {
        let c = centroid(&b.poly);
        l.roads.iter().any(|(pts, _, w)| pts.windows(2).any(|s| super::geom::seg_dist(c, s[0], s[1]) < 300.0 && poly_seg_dist(&b.poly, s[0], s[1]) < 0.5 * w + 8.0))
    };
    let mut inns: Vec<usize> = (0..l.buildings.len()).filter(|&i| ok(&l.buildings[i]) && key(&l.buildings[i]) == Some("inn") && area(&l.buildings[i].poly).abs() >= INN_FROM && street(&l.buildings[i])).collect();
    inns.sort_by(|&a, &b| area(&l.buildings[b].poly).abs().total_cmp(&area(&l.buildings[a].poly).abs()).then(l.buildings[a].id.cmp(&l.buildings[b].id)));
    out.extend(inns.into_iter().take(n.saturating_sub(kept)));
    out
}

/// Whether a monastery on rectangle `rect` (world ft; `id`, taking in the homes `taken`), cutting the
/// alleys of its block `block` it stands across (`cut_alleys`), leaves another building that stood by
/// a lane (the middle of a side of it within 15 ft of its side, where its door goes) farther from every
/// lane than it was (by 2 ft).
fn strands(l: &Layout, rect: &[P], block: &[P], id: u32, taken: &[usize]) -> bool {
    let c = centroid(rect);
    let reach = rect.iter().map(|q| dist(*q, c)).fold(0.0, f64::max) + 60.0;
    let cut: Vec<usize> = (0..l.roads.len()).filter(|&k| is_alley(&l.roads[k], block) && l.roads[k].0.windows(2).any(|s| poly_seg_dist(rect, s[0], s[1]) < 0.5 * l.roads[k].2 - 1.0)).collect();
    if cut.is_empty() {
        return false;
    }
    let pieces: Vec<(Vec<P>, f64)> = cut.iter().flat_map(|&k| cut_out(&l.roads[k].0, rect, 0.5 * l.roads[k].2 + 0.05).into_iter().map(move |p| (p, l.roads[k].2))).collect();
    // (How near a lane the middle of a side of `poly` comes: where a door goes, `interior`'s front.)
    let near = |poly: &[P], pts: &[P], w: f64| {
        let n = poly.len();
        (0..n).map(|k| super::geom::lerp(poly[k], poly[(k + 1) % n], 0.5)).flat_map(|m| pts.windows(2).map(move |s| super::geom::seg_dist(m, s[0], s[1]) - 0.5 * w)).fold(f64::MAX, f64::min)
    };
    let lanes: Vec<usize> = (0..l.roads.len()).filter(|&k| l.roads[k].0.iter().any(|q| dist(*q, c) < reach + 200.0)).collect();
    l.buildings.iter().enumerate().filter(|(k, o)| o.id != id && !taken.contains(k) && o.structure == Structure::Roofed && dist(centroid(&o.poly), c) < reach).any(|(_, o)| {
        let before = lanes.iter().map(|&k| near(&o.poly, &l.roads[k].0, l.roads[k].2)).fold(f64::MAX, f64::min);
        if before > 15.0 {
            return false;
        }
        let after = lanes.iter().filter(|k| !cut.contains(k)).map(|&k| near(&o.poly, &l.roads[k].0, l.roads[k].2)).chain(pieces.iter().map(|(p, w)| near(&o.poly, p, *w))).fold(f64::MAX, f64::min);
        after > before + 2.0
    })
}

/// Whether road `r` is an alley inside block `block` (`build_block`'s, between its lots): a narrow
/// lane every point of which lies inside the block, off its edge.
fn is_alley(r: &(Vec<P>, u8, f64), block: &[P]) -> bool {
    let (pts, class, w) = r;
    *class == 3 && *w <= 2.0 * super::ALLEY_HALF_FT + 1.5 && pts.windows(2).all(|s| {
        let m = super::geom::lerp(s[0], s[1], 0.5);
        contains(block, m) && (0..block.len()).all(|k| super::geom::seg_dist(m, block[k], block[(k + 1) % block.len()]) > 2.0)
    })
}

/// The alleys of layout `l` that reach across rectangle `rect` (a monastery's footprint where it cleared
/// its block: `is_alley` in the block holding its middle; more than a foot into it, as no lane may lie
/// by a building) cut back to it, the pieces left a foot long or more.
fn cut_alleys(l: &mut Layout, rect: &[P]) {
    let c = centroid(rect);
    let Some(block) = l.blocks.iter().find(|b| contains(b, c)).cloned() else { return };
    let mut out: Vec<(Vec<P>, u8, f64)> = Vec::new();
    for r in std::mem::take(&mut l.roads) {
        if is_alley(&r, &block) && r.0.windows(2).any(|s| poly_seg_dist(rect, s[0], s[1]) < 0.5 * r.2 - 1.0) {
            out.extend(cut_out(&r.0, rect, 0.5 * r.2 + 0.05).into_iter().map(|p| (p, r.1, r.2)));
        } else {
            out.push(r);
        }
    }
    l.roads = out;
}

/// Polyline `pts` with what of it a street `half` ft either side of it lays on convex polygon
/// `poly` taken out: the pieces left (each more than a foot long).
fn cut_out(pts: &[P], poly: &[P], half: f64) -> Vec<Vec<P>> {
    let ccw = area(poly) > 0.0;
    let n = poly.len();
    let mut out: Vec<Vec<P>> = Vec::new();
    let mut cur: Vec<P> = vec![pts[0]];
    for s in pts.windows(2) {
        let (p, q) = (s[0], s[1]);
        let d = sub(q, p);
        // (Where the segment lies inside the polygon grown by `half`: t0..t1.)
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for k in 0..n {
            let (a, b) = (poly[k], poly[(k + 1) % n]);
            let e = sub(b, a);
            let mut nrm = mul([e[1], -e[0]], 1.0 / super::geom::len(e).max(1e-9));
            if !ccw {
                nrm = mul(nrm, -1.0);
            }
            // (Inside: dot(x - a, nrm) <= half.)
            let (num, den) = (half - dot(sub(p, a), nrm), dot(d, nrm));
            if den.abs() < 1e-12 {
                if num < 0.0 {
                    (t0, t1) = (1.0, 0.0);
                }
            } else if den > 0.0 {
                t1 = t1.min(num / den);
            } else {
                t0 = t0.max(num / den);
            }
        }
        if t0 < t1 {
            cur.push(add(p, mul(d, t0)));
            out.push(std::mem::take(&mut cur));
            cur.push(add(p, mul(d, t1)));
        }
        cur.push(q);
    }
    out.push(cur);
    out.into_iter()
        .map(|mut piece| {
            piece.dedup_by(|a, b| dist(*a, *b) < 1e-6);
            piece
        })
        .filter(|piece| piece.len() >= 2 && piece.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>() > 1.0)
        .collect()
}

/// What of polygon `poly` lies beyond one side of convex polygon `hull` by `gap` ft or more (the
/// side that leaves the most of it): its corners, each edge labelled by the edge of `poly` it lies
/// along (-1: along that side); None where nothing does.
fn beyond(poly: &[P], hull: &[P], gap: f64) -> Option<Labeled> {
    let ccw = area(hull) > 0.0;
    let whole = Labeled { pts: poly.to_vec(), labels: (0..poly.len() as i32).collect() };
    let n = hull.len();
    (0..n)
        .filter_map(|k| {
            let (a, b) = (hull[k], hull[(k + 1) % n]);
            let e = sub(b, a);
            let mut out = mul([e[1], -e[0]], 1.0 / super::geom::len(e).max(1e-9));
            if !ccw {
                out = mul(out, -1.0);
            }
            // (Keep dot(p, out) >= dot(a, out) + gap.)
            let part = clip(&whole, mul(out, -1.0), -(dot(a, out) + gap), -1);
            (part.pts.len() >= 3).then(|| (area(&part.pts).abs(), part))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|x| x.1)
}

/// The capital's palace kept on open ground (`follow`): the squares, parks, fields and monuments
/// of layout `l` on `hull` (its footprint and its forecourt) cut back beyond a side of it, a square
/// clear of it (gone where too little is left).
fn clear_open(l: &mut Layout, hull: &[P]) {
    let on_it = |q: &[P]| overlap(q, hull) > TOUCH;
    let mut monuments_off: Vec<P> = Vec::new();
    let mut k = 0;
    while k < l.plazas.len() {
        if !on_it(&l.plazas[k].pts) {
            k += 1;
            continue;
        }
        let old = l.plazas[k].pts.clone();
        match beyond(&old, hull, 5.0).filter(|q| area(&q.pts).abs() >= 1500.0) {
            Some(q) => {
                monuments_off.extend(l.monuments.iter().map(|m| centroid(m)).filter(|c| contains(&old, *c) && !contains(&q.pts, *c)));
                l.plazas[k].pts = q.pts;
                k += 1;
            }
            None => {
                monuments_off.extend(l.monuments.iter().map(|m| centroid(m)).filter(|c| contains(&old, *c)));
                l.plazas.remove(k);
            }
        }
    }
    l.monuments.retain(|m| !on_it(m) && !monuments_off.iter().any(|c| dist(*c, centroid(m)) < 1e-6));
    let mut k = 0;
    while k < l.parks.len() {
        if !on_it(&l.parks[k].pts) {
            k += 1;
            continue;
        }
        // (A park left too small for one, as `build_town` has it: gone.)
        match beyond(&l.parks[k].pts, hull, 5.0).filter(|q| area(&q.pts).abs() >= 6000.0 && obb(&q.pts).short >= 40.0) {
            Some(q) => {
                let pk = &mut l.parks[k];
                pk.fronts = q.labels.iter().map(|&e| e >= 0 && pk.fronts.get(e as usize).copied().unwrap_or(false)).collect();
                pk.pts = q.pts;
                k += 1;
            }
            None => drop(l.parks.remove(k)),
        }
    }
    let fields = std::mem::take(&mut l.fields);
    l.fields = fields.into_iter().filter_map(|f| if on_it(&f) { beyond(&f, hull, 5.0).map(|q| q.pts).filter(|q| area(q).abs() >= 2000.0) } else { Some(f) }).collect();
}

/// A town laid out anew by hand (`l`, world ft, before it takes the generated layout's ids:
/// `wards::inherit`; `off`: the site's origin): each palace (and monastery) of the generated layout `generated`
/// stands again on its footprint as generated where everything under that now is a home of its
/// ward (the capital's: any home, as it clears its block; the lots it took in, laid out again) and
/// it reaches across no street (the capital's whatever its patch is laid out as: the streets of its
/// patch that would cross it, or its forecourt as generated, stop at them; the homes on its
/// forecourt gone; on open ground too, the squares, parks, fields and monuments there cut back:
/// `clear_open`; a monastery's as the capital's, on ground fit for a terrace): the first of them given
/// its footprint (so that it takes the palace's id, function and name; on open ground, the palace as
/// generated), the rest gone. The palaces' (and monasteries') ids kept so.
pub(super) fn follow(site: &Site, l: &mut Layout, generated: &Layout, off: P, capital: bool) -> Vec<u32> {
    let mut kept = Vec::new();
    for gp in generated.buildings.iter().filter(|b| merges(generated, b) && b.structure == Structure::Roofed && b.ward != Ward::Castle && b.parts.is_empty()) {
        // (A monastery kept as the capital's palace is, whatever its patch is laid out as; a coaching
        // inn likewise, but on homes outside a castle's ward, never on open ground.)
        let coach = is_coaching(generated, gp);
        let capital = capital || is_monastery(gp) || coach;
        // (What stands under it, measured about the site's origin as it was grown: no rounding of
        // world ft taken for a sliver.)
        let at_origin = |poly: &[P]| -> Vec<P> { poly.iter().map(|p| sub(*p, off)).collect() };
        let own = at_origin(&gp.poly);
        let under: Vec<usize> = (0..l.buildings.len()).filter(|&k| on(&own, &at_origin(&l.buildings[k].poly))).collect();
        let home = |o: &Building| takes(o) && ((capital && !(coach && o.ward == Ward::Castle)) || o.ward == gp.ward);
        let c0 = centroid(&gp.poly);
        let crossing: Vec<usize> = (0..l.roads.len())
            .filter(|&ri| {
                let (pts, _, w) = &l.roads[ri];
                pts.windows(2).any(|s| super::geom::seg_dist(c0, s[0], s[1]) < 400.0 && poly_seg_dist(&gp.poly, s[0], s[1]) < 0.5 * w - 1.0)
            })
            .collect();
        // (The capital's: across its patch's streets, not a main street or a road.)
        let clear = crossing.is_empty() || capital && gp.poly.len() == 4 && crossing.iter().all(|&ri| l.roads[ri].1 == 3);
        let local: Vec<P> = gp.poly.iter().map(|p| sub(*p, off)).collect();
        if (under.is_empty() && (!capital || coach)) || !clear || !under.iter().all(|&k| home(&l.buildings[k])) || !(if is_monastery(gp) { site.terraced(&local) } else { site.buildable(&local) }) {
            continue;
        }
        let mut pieces: Vec<(Vec<P>, u8, f64)> = Vec::new();
        for &ri in &crossing {
            let (pts, class, w) = l.roads[ri].clone();
            pieces.extend(cut_out(&pts, &gp.poly, 0.5 * w + 0.05).into_iter().map(|p| (p, class, w)));
        }
        let mut k = 0;
        l.roads.retain(|_| {
            k += 1;
            !crossing.contains(&(k - 1))
        });
        l.roads.extend(pieces);
        let fc = generated.forecourts.iter().find(|f| f.palace == gp.id && f.yard.len() == 4);
        // (The capital's on open ground: what lies there cut back from it and its forecourt.)
        if capital {
            let mut pts = gp.poly.clone();
            pts.extend(fc.iter().flat_map(|f| f.yard.iter().copied()));
            clear_open(l, &hull(pts));
        }
        // (On open ground the palace as generated: it takes its own id, `wards::inherit`, its
        // patch's number for the while the nearest building's.)
        let k0 = match under.first() {
            Some(&k) => k,
            None => {
                let mut b = gp.clone();
                b.id = l.buildings.iter().min_by(|a, b| dist(centroid(&a.poly), c0).total_cmp(&dist(centroid(&b.poly), c0))).map_or(0, |o| o.id);
                l.buildings.push(b);
                l.buildings.len() - 1
            }
        };
        let mut gone = under.iter().skip(1).copied().collect::<Vec<usize>>();

        // (The capital's forecourt as generated cleared again: the homes on it and its way gone, its
        // patch's streets stopping at it.)
        if let Some(fc) = fc {
            let yard = fc.yard.clone();
            let way = |o: &Building| fc.way.windows(2).any(|s| poly_seg_dist(&o.poly, s[0], s[1]) < 2.5 * GATE_SQ);
            let more: Vec<usize> = (0..l.buildings.len()).filter(|&k| k != k0 && !gone.contains(&k) && home(&l.buildings[k]) && (on(&yard, &l.buildings[k].poly) || way(&l.buildings[k]))).collect();
            gone.extend(more);
            let cross: Vec<usize> = (0..l.roads.len()).filter(|&ri| l.roads[ri].1 == 3 && l.roads[ri].0.windows(2).any(|s| poly_seg_dist(&yard, s[0], s[1]) < 0.5 * l.roads[ri].2)).collect();
            let mut pieces: Vec<(Vec<P>, u8, f64)> = Vec::new();
            for &ri in &cross {
                let (pts, class, w) = l.roads[ri].clone();
                pieces.extend(cut_out(&pts, &yard, 0.5 * w + 0.05).into_iter().map(|p| (p, class, w)));
            }
            let mut k = 0;
            l.roads.retain(|_| {
                k += 1;
                !cross.contains(&(k - 1))
            });
            l.roads.extend(pieces);
        }
        (l.buildings[k0].poly, l.buildings[k0].pad_ft) = (gp.poly.clone(), site.pad(&local));
        // (A monastery's ground as generated, `Layout::grounds`: the homes on it gone, as `merge` took
        // them in; a piece another building stands on left out.)
        let ground: Vec<Vec<P>> = generated.grounds.iter().filter(|g| g.0 == gp.id && (is_monastery(gp) || coach)).map(|g| g.1.clone()).collect();
        let on_ground = |o: &Building, g: &[P]| on_lot(&at_origin(g), &at_origin(&o.poly));
        let more: Vec<usize> = (0..l.buildings.len()).filter(|&k| k != k0 && !gone.contains(&k) && takes(&l.buildings[k]) && ground.iter().any(|g| on_ground(&l.buildings[k], g))).collect();
        gone.extend(more);
        let mut k = 0;
        l.buildings.retain(|_| {
            k += 1;
            !gone.contains(&(k - 1))
        });
        let k0 = k0 - gone.iter().filter(|&&k| k < k0).count();
        l.grounds.extend(ground.into_iter().filter(|g| !l.buildings.iter().enumerate().any(|(k, o)| k != k0 && on_ground(o, g))).map(|g| (gp.id, g)));
        // (A coaching inn's footprint its ground at the least: `is_coaching`.)
        if coach && !l.grounds.iter().any(|g| g.0 == gp.id) {
            l.grounds.push((gp.id, gp.poly.clone()));
        }
        kept.push(gp.id);
    }
    kept
}
