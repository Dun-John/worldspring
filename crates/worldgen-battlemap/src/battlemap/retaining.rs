//! Retaining walls: where a house's pad stands above the ground beside it or is cut into the
//! slope (`town::fill`'s seated lots, and any lot whose ground strays from its pad), its yard
//! round it is levelled at its floor's level, `YARD_FT` out, and edged with a stone wall where the
//! ground beyond drops away or climbs past it by more than `RISE_FT`; a flight of steps likewise
//! along its sides where it is cut below, or raised above, the open ground beside it. The heights
//! step at the wall (its cliff bits follow from them, `cover::compute_edges`) and the square beyond
//! a wall 4 ft or more is hard going (climbing it or down it). The wall is a `ShapeKind::Retaining`:
//! a quay's coping along its top, its face and shadow on the low side. Pure per global square and per piece of wall (each piece
//! drawn by the chunk its middle lies in), so chunks agree.

use super::*;

/// How far round a house its levelled yard reaches (ft): a square.
pub(super) const YARD_FT: f64 = 5.0;
/// The least step (ft) a wall stands for.
const RISE_FT: f32 = 3.0;
/// The most (ft): past it the ground is a cliff of its own (or a house at a cliff's foot).
const MAX_FT: f32 = 17.0;
/// The coping's width (ft).
const COPING_FT: f64 = 2.2;
/// A wall's pieces, at most this long (ft): each judged on its own squares.
const PIECE_FT: f64 = 5.0;
/// `marks` bit: hard going.
pub(super) const HARD_SQ: u8 = 2;

/// A building whose pad a yard and wall may edge: a plain roofed one on the ground (no deck, no
/// pieces; a palace's court has its own ground).
pub(super) fn seated(b: &crate::town::Building) -> bool {
    b.structure == crate::town::Structure::Roofed && b.deck.is_none() && b.parts.is_empty()
}

/// A box grown by what the yard and the samples beside a wall reach.
pub(super) fn grow(b: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let r = YARD_FT + 2.0 * SQUARE_FT;
    (b.0 - r, b.1 - r, b.2 + r, b.3 + r)
}

/// What a wall may stand by (halo squares).
pub(super) struct Open<'a> {
    pub bld_h: &'a [u16],
    pub road_h: &'a [u8],
    pub urb_h: &'a [u8],
    pub ent_h: &'a [bool],
    pub hw: &'a [f32],
    pub stair: Option<&'a [bool]>,
    /// Ground shaped by hand (empty: none here): its own heights stand, no yard levelled on it.
    pub shaped: &'a [bool],
}

impl Open<'_> {
    fn stair(&self, h: usize) -> bool {
        self.stair.is_some_and(|s| s[h])
    }
    /// Open ground: no building, road, street, square, wall or water, nor ground shaped by hand.
    fn open(&self, h: usize, hh: &[f32]) -> bool {
        !self.shaped.get(h).is_some_and(|&s| s)
            && self.bld_h[h] == 0
            && self.road_h[h] == 0
            && !self.ent_h[h]
            && !self.stair(h)
            && self.hw[h] <= hh[h]
            && !matches!(self.urb_h[h], URBAN_STREET | URBAN_PLAZA | URBAN_DECK | URBAN_WALL | URBAN_TOWER | URBAN_ENTRANCE | URBAN_TOWER_FLOOR | URBAN_FIELD)
    }
}

/// Each edge of an outline with its outward normal and length.
fn edges(poly: &[[f64; 2]]) -> Vec<([f64; 2], [f64; 2], [f64; 2], f64)> {
    let s = if crate::town::geom::area(poly) >= 0.0 { 1.0 } else { -1.0 };
    let n = poly.len();
    (0..n)
        .filter_map(|k| {
            let (p, q) = (poly[k], poly[(k + 1) % n]);
            let l = crate::town::geom::dist(p, q);
            (l > 1e-6).then(|| {
                let u = [(q[0] - p[0]) / l, (q[1] - p[1]) / l];
                // (Counter-clockwise, y down or up alike: outward is the right-hand normal.)
                (p, u, [s * u[1], -s * u[0]], l)
            })
        })
        .collect()
}

/// Levels the yards round `seats` (in `hh`), pushes the walls the chunk draws and returns the
/// marks per halo square (`HARD_SQ`). `hx0, hy0`: the halo's corner (ft); the chunk is
/// `[ox, ox + size) × [oy, oy + size)`.
pub(super) fn build(seats: &[(Vec<[f64; 2]>, f32)], flights: &[([f64; 2], [f64; 2], f64)], g: &Open, hh: &mut [f32], (hx0, hy0): (f64, f64), (ox, oy, size): (f64, f64, f64), shapes: &mut Vec<VectorShape>) -> Vec<u8> {
    let mut marks = Vec::new();
    if seats.is_empty() && flights.is_empty() {
        return marks;
    }
    let cell = |p: [f64; 2]| -> Option<usize> {
        let (i, j) = (crate::core::floor((p[0] - hx0) / SQUARE_FT), crate::core::floor((p[1] - hy0) / SQUARE_FT));
        (i >= 0.0 && j >= 0.0 && i < HS as f64 && j < HS as f64).then(|| j as usize * HS + i as usize)
    };
    let center = |h: usize| [hx0 + ((h % HS) as f64 + 0.5) * SQUARE_FT, hy0 + ((h / HS) as f64 + 0.5) * SQUARE_FT];
    // The yards: each open square within `YARD_FT` of a house's side (square at its corners) whose
    // ground strays from the pad, levelled to the nearest house's pad (the lower pad on a tie).
    let mut owner = vec![u32::MAX; HS * HS];
    let mut best = vec![(f64::MAX, f32::MAX); HS * HS];
    let es: Vec<_> = seats.iter().map(|(poly, _)| edges(poly)).collect();
    for (k, (poly, pad)) in seats.iter().enumerate() {
        let b = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, q| (a.0.min(q[0]), a.1.min(q[1]), a.2.max(q[0]), a.3.max(q[1])));
        let (i0, j0) = (crate::core::floor((b.0 - YARD_FT - hx0) / SQUARE_FT).max(0.0) as usize, crate::core::floor((b.1 - YARD_FT - hy0) / SQUARE_FT).max(0.0) as usize);
        let (i1, j1) = (crate::core::floor((b.2 + YARD_FT - hx0) / SQUARE_FT), crate::core::floor((b.3 + YARD_FT - hy0) / SQUARE_FT));
        if i1 < 0.0 || j1 < 0.0 {
            continue;
        }
        for j in j0..=(j1 as usize).min(HS - 1) {
            for i in i0..=(i1 as usize).min(HS - 1) {
                let h = j * HS + i;
                if !g.open(h, hh) {
                    continue;
                }
                let dh = (hh[h] - pad).abs();
                if dh <= 2.0 || dh > MAX_FT {
                    continue;
                }
                let c = center(h);
                if crate::town::geom::contains(poly, c) {
                    continue;
                }
                let d = es[k]
                    .iter()
                    .filter_map(|&(p, u, nrm, l)| {
                        let v = crate::town::geom::sub(c, p);
                        let (t, o) = (crate::town::geom::dot(v, u), crate::town::geom::dot(v, nrm));
                        (t >= -YARD_FT && t <= l + YARD_FT && o > 0.0 && o <= YARD_FT).then_some(o)
                    })
                    .fold(f64::MAX, f64::min);
                if d < f64::MAX && (d, *pad) < best[h] {
                    best[h] = (d, *pad);
                    owner[h] = k as u32;
                }
            }
        }
    }
    for h in 0..HS * HS {
        if owner[h] != u32::MAX {
            hh[h] = seats[owner[h] as usize].1;
        }
    }
    marks = vec![0u8; HS * HS];
    let owned = |m: [f64; 2]| m[0] >= ox && m[0] < ox + size && m[1] >= oy && m[1] < oy + size;
    let local = |p: [f64; 2]| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32];
    // One line of wall, `a` along `u` for `l` ft, `nrm` toward the ground beyond: per piece, the
    // level on its own side (`level`, from the square there; `None` no wall) against the ground
    // beyond; the walled pieces drawn in runs, the low side on the line's left.
    let mut line = |a: [f64; 2], u: [f64; 2], nrm: [f64; 2], l: f64, inner: f64, flight: bool, level: &dyn Fn(usize, &[f32]) -> Option<f32>, not_beyond: &dyn Fn(usize) -> bool, hh: &[f32], marks: &mut [u8]| {
        let n = crate::core::ceil(l / PIECE_FT).max(1.0) as usize;
        let at = |t: f64| [a[0] + u[0] * t, a[1] + u[1] * t];
        let mut run: Vec<[f64; 2]> = Vec::new();
        let (mut run_low, mut run_rise) = (false, 0f32);
        let mut flush = |run: &mut Vec<[f64; 2]>, low_beyond: bool, rise: f32| {
            if run.len() >= 2 {
                // (The low side on the left-hand normal (-uy, ux) of the line as drawn.)
                let left = [-u[1], u[0]];
                let toward = if low_beyond { nrm } else { [-nrm[0], -nrm[1]] };
                if crate::town::geom::dot(left, toward) < 0.0 {
                    run.reverse();
                }
                let mut pts: Vec<[f32; 2]> = run.iter().map(|&p| local(p)).collect();
                pts.push([rise, 0.0]);
                shapes.push(VectorShape { kind: ShapeKind::Retaining, size: (COPING_FT / SQUARE_FT) as f32, pts });
            }
            run.clear();
        };
        // (Each piece's squares and step, then the walled ones: past `RISE_FT`, a gap of one piece
        // between two walled the same way filled, a lone piece under 5 ft left out.)
        let raw: Vec<Option<(usize, usize, f32)>> = (0..n)
            .map(|k| {
                let m = at(l * (k as f64 + 0.5) / n as f64);
                let off = |d: f64| [m[0] + nrm[0] * d, m[1] + nrm[1] * d];
                // (Never along a street or over a flight's treads: just past the line is no flight's,
                // nor a street's or a building's.)
                let clear = |d: f64| cell(off(d)).is_some_and(|h| g.bld_h[h] == 0 && g.road_h[h] == 0 && !g.stair(h) && !matches!(g.urb_h[h], URBAN_STREET | URBAN_PLAZA));
                let past = clear(1.5) && (flight || clear(0.0));
                match (cell(off(-inner)), cell(off(4.0))) {
                    (Some(hi), Some(hb)) if past && hi != hb && g.open(hb, hh) && !not_beyond(hb) => level(hi, hh).map(|lv| (hi, hb, hh[hb] - lv)).filter(|w| w.2.abs() <= MAX_FT),
                    _ => None,
                }
            })
            .collect();
        let over = |k: usize| raw[k].filter(|w| w.2.abs() > RISE_FT);
        let walls: Vec<Option<(usize, usize, f32)>> = (0..n)
            .map(|k| {
                let (before, after) = (if k > 0 { over(k - 1) } else { None }, if k + 1 < n { over(k + 1) } else { None });
                match (over(k), before, after) {
                    (Some(w), None, None) if w.2.abs() < 5.0 => None,
                    (Some(w), _, _) => Some(w),
                    (None, Some(a), Some(b)) => raw[k].filter(|w| w.2.abs() > 1.0 && (w.2 > 0.0) == (a.2 > 0.0) && (a.2 > 0.0) == (b.2 > 0.0)),
                    _ => None,
                }
            })
            .collect();
        for k in 0..n {
            let (t0, t1) = (l * k as f64 / n as f64, l * (k + 1) as f64 / n as f64);
            let m = at(0.5 * (t0 + t1));
            let wall = walls[k];
            let low_beyond = wall.is_some_and(|w| w.2 < 0.0);
            if let Some((_, hb, _)) = wall.filter(|w| w.2.abs() >= 4.0) {
                marks[hb] |= HARD_SQ;
            }
            let draw = wall.is_some() && owned(m);
            if !draw || (!run.is_empty() && low_beyond != run_low) {
                flush(&mut run, run_low, run_rise);
                run_rise = 0.0;
            }
            if draw {
                if run.is_empty() {
                    run.push(at(t0));
                }
                run.push(at(t1));
                run_low = low_beyond;
                run_rise = run_rise.max(wall.map_or(0.0, |w| w.2.abs()));
            }
        }
        flush(&mut run, run_low, run_rise);
    };
    for (k, (_, pad)) in seats.iter().enumerate() {
        let level = |h: usize, hh: &[f32]| (owner[h] == k as u32 || (g.open(h, hh) && (hh[h] - pad).abs() <= 2.0)).then_some(*pad);
        let not_beyond = |h: usize| owner[h] == k as u32;
        for &(p, u, nrm, l) in &es[k] {
            let a = [p[0] + nrm[0] * YARD_FT - u[0] * YARD_FT, p[1] + nrm[1] * YARD_FT - u[1] * YARD_FT];
            line(a, u, nrm, l + 2.0 * YARD_FT, 3.0, false, &level, &not_beyond, hh, &mut marks);
        }
    }
    for &(a, b, width) in flights {
        let l = crate::town::geom::dist(a, b);
        if l < 1e-6 {
            continue;
        }
        let half = 0.5 * width;
        let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
        let level = |h: usize, hh: &[f32]| g.stair(h).then(|| hh[h]);
        for s in [1.0, -1.0] {
            let nrm = [-u[1] * s, u[0] * s];
            line([a[0] + nrm[0] * half, a[1] + nrm[1] * half], u, nrm, l, (0.5 * half).min(3.0), true, &level, &|_| false, hh, &mut marks);
        }
    }
    marks
}
