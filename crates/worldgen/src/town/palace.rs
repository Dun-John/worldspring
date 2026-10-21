//! Palaces' footprints: a palace (and only a palace) takes in the lots beside its own for a big
//! footprint (`merge`): the homes of its ward on the same block, wall to wall with it, as long as the
//! hull of them all crosses no street or alley and covers no other building; it then stands on the
//! largest rectangle that hull holds, square to its own lot (a capital's with a side an odd number
//! of squares across for its front, for its axis), on ground fit to build on. The lots taken in are gone (their ids
//! with them: every other building keeps its own, as the ward editor keeps them); the palace keeps
//! its id and name. What the hull holds outside the rectangle is the palace's open ground. The
//! capital's always stands round a court: where the lots beside it give no footprint big enough, it
//! clears more of its block, taking in the homes and the open ground there (`clear_ground`). Every
//! other palace stays smaller than the least a capital's is. The capital's then gets its front and
//! the forecourt before it, walled, its gate on the axis, a paved way on to the street (`approach`).
//! A town laid out anew by hand keeps its palace's footprint as generated wherever what stands under
//! it now is its own lot and homes clear of the streets (the capital's whatever its patch is laid out
//! as, open ground too, its front as generated: `follow`, `approach`); else it is grown again. Nothing overlaps a palace, by the area two outlines share
//! (`super::overlap`), however little.

use super::geom::{Labeled, add, area, centroid, clip, contains, dist, dot, hull, mul, obb, sub};
use super::{Building, Layout, P, Site, Structure, Ward, catalog, overlap, poly_seg_dist};

/// The footprint a palace grows to (sq ft): a capital's round a court, another's round its hall.
const CAPITAL_FT2: f64 = 15000.0;
const CITY_FT2: f64 = 6000.0;
/// The sides a palace's rectangle may have (ft): its short side `least` to `most_short`, its long one
/// `long` to `most_long`; its floor `most_area` at most.
#[derive(Clone, Copy)]
struct Sides {
    least: f64,
    long: f64,
    most_short: f64,
    most_long: f64,
    most_area: f64,
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
/// The area (sq ft) two outlines may share and not overlap: none but rounding's.
const TOUCH: f64 = 0.01;
/// The most lots a palace takes in.
const MOST: usize = 4;

/// Whether a building is a palace.
pub fn is_palace(b: &Building) -> bool {
    b.func.is_some_and(|f| catalog::CATALOG[f as usize].key == "palace")
}

/// The largest rectangle (axes `u`, `v`) inside convex polygon `poly` (world ft), its sides whole
/// multiples of 5 ft (`odd`: an odd number of them along `u`), its sides within `sides` (`Sides`), as
/// its four corners, anticlockwise in axes with y up when `ccw`.
fn largest_rect(poly: &[P], u: P, odd: bool, sides: Sides, ccw: bool) -> Option<(Vec<P>, f64)> {
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
fn clear_ground(site: &Site, l: &Layout, p: &Building, u: P, ccw: bool, off: P, gone: &[u32]) -> Option<(Vec<usize>, Vec<P>)> {
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
    // (The squares a polygon overlaps (of its box on the lattice): not free.)
    let block_out = |poly: &[P], free: &mut Vec<bool>| {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for q in poly {
            let d = sub(*q, o);
            let (x, y) = (dot(d, u) / SQ, dot(d, v) / SQ);
            (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
        }
        let at_most = |a: f64| (crate::core::floor(a).max(0.0) as usize).min(N);
        for j in at_most(y0)..at_most(y1 + 1.0) {
            for i in at_most(x0)..at_most(x1 + 1.0) {
                if free[j * N + i] && overlap(poly, &cell(i, j)) > TOUCH {
                    free[j * N + i] = false;
                }
            }
        }
    };
    // (Its own lot's squares, for a rectangle beside it.)
    let lot: Vec<bool> = (0..N * N).map(|k| overlap(&p.poly, &cell(k % N, k / N)) > TOUCH).collect();
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
    // (Clear of the streets and roads, the town wall and its towers.)
    let lanes: Vec<(P, P, f64)> = l
        .roads
        .iter()
        .flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * w + 1.0)))
        .chain(l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1], 8.0))))
        .filter(|&(a, b, _)| super::geom::seg_dist(c0, a, b) < reach + 100.0)
        .collect();
    for (k, f) in free.iter_mut().enumerate() {
        if *f {
            let q = cell(k % N, k / N);
            *f = lanes.iter().all(|&(a, b, w)| poly_seg_dist(&q, a, b) >= w) && l.towers.iter().all(|t| dist(*t, centroid(&q)) > 20.0);
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
    let front = |a: usize, b: usize| a % 2 == 1 && courted(a, b);
    let mut sizes: Vec<(usize, usize)> = (17..=31).flat_map(|w| (17..=31).map(move |d| (w, d))).filter(|&(w, d)| (front(w, d) || front(d, w)) && (w * d) as f64 * SQ * SQ <= CAPITAL_FT2).collect();
    sizes.sort_by_key(|&(w, d)| (std::cmp::Reverse(w * d), w.abs_diff(d), w));
    let mid = 0.5 * N as f64;
    let rect = |i0: usize, j0: usize, w: usize, d: usize| vec![at(i0 as f64, j0 as f64), at((i0 + w) as f64, j0 as f64), at((i0 + w) as f64, (j0 + d) as f64), at(i0 as f64, (j0 + d) as f64)];
    let local = |poly: &[P]| -> Vec<P> { poly.iter().map(|p| sub(*p, off)).collect() };
    for (w, d) in sizes {
        // (Each place it fits: how far its middle is from its lot's, how many homes it takes in.)
        let mut fits: Vec<(f64, usize, usize, usize)> = Vec::new();
        for j0 in 0..=N - d {
            for i0 in 0..=N - w {
                if !fits_at(i0, j0, w, d) {
                    continue;
                }
                let r = rect(i0, j0, w, d);
                let taken = homes.iter().filter(|&&k| on(&r, &l.buildings[k].poly)).count();
                let m = [i0 as f64 + 0.5 * w as f64 - mid, j0 as f64 + 0.5 * d as f64 - mid];
                fits.push((m[0] * m[0] + m[1] * m[1], taken, i0, j0));
            }
        }
        fits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then((a.2, a.3).cmp(&(b.2, b.3))));
        for &(_, _, i0, j0) in &fits {
            let mut r = rect(i0, j0, w, d);
            if !site.buildable(&local(&r)) {
                continue;
            }
            if (area(&r) > 0.0) != ccw {
                r.reverse();
            }
            let taken: Vec<usize> = homes.iter().copied().filter(|&k| on(&r, &l.buildings[k].poly)).collect();
            return Some((taken, r));
        }
    }
    None
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

/// Each palace of layout `l` (world ft; `off`: the settlement's centre, the site's origin) given its
/// footprint: the lots beside it taken in (`merge`'s rules), the largest rectangle the whole holds.
/// `capital`: the capital's (round a court: bigger, an odd number of squares across).
pub(super) fn merge(site: &Site, l: &mut Layout, off: P, capital: bool, skip: &[u32]) {
    let local = |poly: &[P]| -> Vec<P> { poly.iter().map(|p| sub(*p, off)).collect() };
    let palaces: Vec<usize> = (0..l.buildings.len())
        .filter(|&i| is_palace(&l.buildings[i]) && l.buildings[i].structure == Structure::Roofed && l.buildings[i].ward != Ward::Castle && l.buildings[i].parts.is_empty() && !skip.contains(&l.buildings[i].id))
        .collect();
    let mut gone: Vec<u32> = Vec::new();
    for pi in palaces {
        let p = l.buildings[pi].clone();
        let target = if capital { CAPITAL_FT2 } else { CITY_FT2 };
        let ccw = area(&p.poly) > 0.0;
        let u = obb(&p.poly).axis;
        // (The streets and alleys near it: nothing it takes in may reach across one.)
        let c0 = centroid(&p.poly);
        let lanes: Vec<(P, P, f64)> = l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], *w))).filter(|&(a, b, _)| super::geom::seg_dist(c0, a, b) < 400.0).collect();
        let clear = |h: &[P]| lanes.iter().all(|&(a, b, w)| poly_seg_dist(h, a, b) >= 0.5 * w - 1.0);
        // (A home of its ward it may take in.)
        let home = |o: &Building| takes(o) && o.ward == p.ward && !gone.contains(&o.id);
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
                    .filter(|&j| j != pi && !with.contains(&j) && !gone.contains(&l.buildings[j].id) && l.buildings[j].structure != Structure::Open)
                    .filter(|&j| dist(centroid(&l.buildings[j].poly), c0) < 400.0 && on(&h, &l.buildings[j].poly))
                    .collect();
                if more.is_empty() {
                    return Some((with, r, ar));
                }
                if with.len() + more.len() > MOST || !more.iter().all(|&j| home(&l.buildings[j])) {
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
            while best < target && set.len() < MOST {
                let members: Vec<Vec<P>> = std::iter::once(p.poly.clone()).chain(set.iter().map(|&k| l.buildings[k].poly.clone())).collect();
                let mut pick: Option<(f64, Vec<usize>, Vec<P>)> = None;
                for (k, o) in l.buildings.iter().enumerate() {
                    if k == pi || set.contains(&k) || !home(o) || dist(centroid(&o.poly), c0) > 250.0 || !members.iter().any(|m| gap(m, &o.poly) < 2.0) {
                        continue;
                    }
                    let Some((with, r, ar)) = grow(&set, k, l, sides) else { continue };
                    // (Worth taking: the rectangle grows by much of the lots' own floor.)
                    let added: f64 = with.iter().filter(|j| !set.contains(j)).map(|&j| area(&l.buildings[j].poly).abs()).sum();
                    if ar - best < 0.4 * added || !site.buildable(&local(&r)) {
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
            if set.is_empty() && best < if capital { 0.6 } else { 0.8 } * area(&p.poly).abs() && (capital || area(&p.poly).abs() <= CITY_FT2) {
                return None;
            }
            site.buildable(&local(&rect)).then_some((set, rect))
        };
        // (A capital's round a court: on the lots beside it, else clearing its block; else, where
        // nothing round a court can be had at all, a front and a back range.)
        let found = if capital {
            attempt(CAPITAL_SIDES).filter(|f| courts(&f.1)).or_else(|| clear_ground(site, l, &p, u, ccw, off, &gone)).or_else(|| attempt(CAPITAL_LEAST))
        } else {
            attempt(CITY_SIDES)
        };
        let Some((set, rect)) = found else { continue };
        let b = &mut l.buildings[pi];
        b.pad_ft = site.pad(&local(&rect));
        b.poly = rect;
        gone.extend(set.iter().map(|&k| l.buildings[k].id));
    }
    if !gone.is_empty() {
        l.buildings.retain(|b| !gone.contains(&b.id));
    }
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
/// `wards::inherit`; `off`: the site's origin): each palace of the generated layout `generated`
/// stands again on its footprint as generated where everything under that now is a home of its
/// ward (the capital's: any home, as it clears its block; the lots it took in, laid out again) and
/// it reaches across no street (the capital's whatever its patch is laid out as: the streets of its
/// patch that would cross it, or its forecourt as generated, stop at them; the homes on its
/// forecourt gone; on open ground too, the squares, parks, fields and monuments there cut back:
/// `clear_open`): the first of them given its footprint (so that it takes the palace's id,
/// function and name; on open ground, the palace as generated), the rest gone. The palaces' ids
/// kept so.
pub(super) fn follow(site: &Site, l: &mut Layout, generated: &Layout, off: P, capital: bool) -> Vec<u32> {
    let mut kept = Vec::new();
    for gp in generated.buildings.iter().filter(|b| is_palace(b) && b.structure == Structure::Roofed && b.ward != Ward::Castle && b.parts.is_empty()) {
        let under: Vec<usize> = (0..l.buildings.len()).filter(|&k| on(&gp.poly, &l.buildings[k].poly)).collect();
        let home = |o: &Building| takes(o) && (capital || o.ward == gp.ward);
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
        if (under.is_empty() && !capital) || !clear || !under.iter().all(|&k| home(&l.buildings[k])) || !site.buildable(&local) {
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
        let mut k = 0;
        l.buildings.retain(|_| {
            k += 1;
            !gone.contains(&(k - 1))
        });
        kept.push(gp.id);
    }
    kept
}
