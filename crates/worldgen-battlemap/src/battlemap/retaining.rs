//! Retaining walls: where a house's pad stands above the ground beside it or is cut into the
//! slope (`town::fill`'s seated lots, and any lot whose ground strays from its pad), its yard
//! round it is levelled at its floor's level, `YARD_FT` out, and edged with a stone wall where the
//! ground beyond drops away or climbs past it by more than `RISE_FT`, one line round the yard's
//! mitred outline, stopping short of another house's eaves; a flight of steps likewise along its
//! treads' edges where it is cut below, or raised above, the open ground beside it. A wall stands
//! only along a sustained drop (`MIN_RUN_FT`). The heights step at the wall (its cliff bits follow from them, `cover::compute_edges`;
//! a yard's relief shaded soft) and the square beyond a wall 4 ft or more is hard going
//! (climbing it or down it). The wall is a `ShapeKind::Retaining`: a quay's coping along its top,
//! its face and shadow on the low side. Pure per global square and per piece of wall (each piece
//! drawn by the chunk its middle lies in; a run reaching past the halo counted as going on), so
//! the heights agree across chunks (a wall's run is judged from its own chunk).

use super::*;

/// How far round a house its levelled yard reaches (ft): a square.
pub(super) const YARD_FT: f64 = 5.0;
/// The least step (ft) a wall stands for.
const RISE_FT: f32 = 3.0;
/// The most (ft): past it the ground is a cliff of its own (or a house at a cliff's foot).
const MAX_FT: f32 = 17.0;
/// The coping's width (ft).
const COPING_FT: f64 = 1.8;
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

/// The yard's outline: the house's grown `YARD_FT` out, its corners mitred (each edge with its
/// outward normal), so a wall round it turns its corners as one line.
fn yard_outline(es: &[([f64; 2], [f64; 2], [f64; 2], f64)]) -> Vec<([f64; 2], [f64; 2], [f64; 2])> {
    let n = es.len();
    let corner = |k: usize| {
        let (p, _, n1, _) = es[k];
        let n0 = es[(k + n - 1) % n].2;
        let c = 1.0 + n0[0] * n1[0] + n0[1] * n1[1];
        // (A corner sharper than about 140° keeps its own side's offset.)
        let m = if c > 0.3 { [(n0[0] + n1[0]) / c, (n0[1] + n1[1]) / c] } else { n1 };
        [p[0] + m[0] * YARD_FT, p[1] + m[1] * YARD_FT]
    };
    (0..n).map(|k| (corner(k), corner((k + 1) % n), es[k].2)).collect()
}

/// One piece of a wall's line: its ends, the normal toward the ground beyond, and whether it
/// starts at a corner.
struct Piece {
    a: [f64; 2],
    b: [f64; 2],
    nrm: [f64; 2],
    corner: bool,
}

/// A line of wall: its pieces end to end (closed round a yard), the house it rings (`None`: a
/// flight's side) and how far in its own level is read.
struct Line {
    pieces: Vec<Piece>,
    closed: bool,
    seat: Option<usize>,
    inner: f64,
}

/// A piece judged: its squares out of reach (past the halo), no wall, or the square on its own
/// side, the square beyond and the step (ft, + the ground beyond higher).
#[derive(Clone, Copy, PartialEq)]
enum Judged {
    Unknown,
    No,
    Step(usize, usize, f32),
}

/// A piece walled: its step (`None`: drawn on to a flight's end without one) and whether the
/// ground beyond is the low side.
#[derive(Clone, Copy)]
struct Wall {
    step: Option<(usize, usize, f32)>,
    low: bool,
}

/// The least run (ft) of wall a drop must hold along to stand.
const MIN_RUN_FT: f64 = 15.0;
/// How near another house's outline (ft) a yard's wall stops (its eaves).
const EAVES_FT: f64 = 2.0;
/// `marks` bit: a yard's square (its relief shaded soft: its wall, or the house, stands for the step).
pub(super) const BY_WALL: u8 = 4;

/// Splits `a`→`b` into pieces at most `PIECE_FT` long.
fn split(out: &mut Vec<Piece>, a: [f64; 2], b: [f64; 2], nrm: [f64; 2]) {
    let n = crate::core::ceil(crate::town::geom::dist(a, b) / PIECE_FT).max(1.0) as usize;
    let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    for k in 0..n {
        out.push(Piece { a: at(k as f64 / n as f64), b: at((k + 1) as f64 / n as f64), nrm, corner: k == 0 });
    }
}

/// Levels the yards round `seats` (in `hh`), pushes the walls the chunk draws and returns the
/// marks per halo square (`HARD_SQ`, `BY_WALL`). `hx0, hy0`: the halo's corner (ft); the chunk is
/// `[ox, ox + size) × [oy, oy + size)`. `flights`: each flight's axis, its ends and width.
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
    let boxes: Vec<_> = seats.iter().map(|(poly, _)| poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, q| (a.0.min(q[0]), a.1.min(q[1]), a.2.max(q[0]), a.3.max(q[1])))).collect();
    for (k, (poly, pad)) in seats.iter().enumerate() {
        let b = boxes[k];
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
    // The lines: round each yard's outline, and along each side of a flight at its treads' edge.
    let mut lines: Vec<Line> = Vec::new();
    for k in 0..seats.len() {
        let mut pieces = Vec::new();
        for (a, b, nrm) in yard_outline(&es[k]) {
            split(&mut pieces, a, b, nrm);
        }
        lines.push(Line { pieces, closed: true, seat: Some(k), inner: 3.0 });
    }
    for &(a, b, width) in flights {
        let l = crate::town::geom::dist(a, b);
        if l < 1e-6 {
            continue;
        }
        let half = 0.5 * width;
        let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
        for s in [1.0, -1.0] {
            let nrm = [-u[1] * s, u[0] * s];
            let mut pieces = Vec::new();
            split(&mut pieces, [a[0] + nrm[0] * half, a[1] + nrm[1] * half], [b[0] + nrm[0] * half, b[1] + nrm[1] * half], nrm);
            lines.push(Line { pieces, closed: false, seat: None, inner: (0.5 * half).min(3.0) });
        }
    }
    // Under another house's eaves (or inside it).
    let eaves = |k: usize, p: [f64; 2]| {
        seats.iter().enumerate().any(|(j, (poly, _))| {
            let b = boxes[j];
            j != k
                && p[0] >= b.0 - EAVES_FT
                && p[0] <= b.2 + EAVES_FT
                && p[1] >= b.1 - EAVES_FT
                && p[1] <= b.3 + EAVES_FT
                && (crate::town::geom::contains(poly, p) || (0..poly.len()).any(|e| crate::town::geom::seg_dist(p, poly[e], poly[(e + 1) % poly.len()]) < EAVES_FT))
        })
    };
    let judge = |line: &Line, pc: &Piece, hh: &[f32], owner: &[u32]| -> Judged {
        let m = [0.5 * (pc.a[0] + pc.b[0]), 0.5 * (pc.a[1] + pc.b[1])];
        let off = |d: f64| [m[0] + pc.nrm[0] * d, m[1] + pc.nrm[1] * d];
        let Some(hi) = cell(off(-line.inner)) else { return Judged::Unknown };
        let (hb, lv) = match line.seat {
            Some(k) => {
                if [pc.a, m, pc.b].iter().any(|&p| eaves(k, p)) {
                    return Judged::No;
                }
                // (Never along a street or by a building: just past the line no flight's square,
                // nor a street's or a building's, and none of a building at its ends.)
                let near = [off(0.0), off(1.5), pc.a, pc.b].map(cell);
                for (e, h) in near.into_iter().enumerate() {
                    let Some(h) = h else { return Judged::Unknown };
                    if g.bld_h[h] != 0 || (e < 2 && (g.road_h[h] != 0 || g.stair(h) || matches!(g.urb_h[h], URBAN_STREET | URBAN_PLAZA))) {
                        return Judged::No;
                    }
                }
                let Some(hb) = cell(off(4.0)) else { return Judged::Unknown };
                let pad = seats[k].1;
                // (Its own side the house's yard, or open ground about its level no other yard holds.)
                let mine = owner[hi] == k as u32 || (owner[hi] == u32::MAX && g.open(hi, hh) && (hh[hi] - pad).abs() <= 2.0);
                if !mine || owner[hb] == k as u32 {
                    return Judged::No;
                }
                (hb, pad)
            }
            None => {
                if !g.stair(hi) {
                    return Judged::No;
                }
                // (The ground past the treads' squares, which may reach a little past their edge.)
                let mut hb = None;
                for d in [3.5, 6.0] {
                    let Some(h) = cell(off(d)) else { return Judged::Unknown };
                    if !g.stair(h) {
                        hb = Some(h);
                        break;
                    }
                }
                let Some(hb) = hb else { return Judged::No };
                (hb, hh[hi])
            }
        };
        let rise = hh[hb] - lv;
        if hi == hb || !g.open(hb, hh) || rise.abs() > MAX_FT {
            return Judged::No;
        }
        Judged::Step(hi, hb, rise)
    };
    // The pieces walled along a line: past `RISE_FT`, the gaps of up to `gap` pieces between two
    // walled the same way filled where the ground still steps, then only runs `MIN_RUN_FT` long or
    // more (a piece out of reach counted as going on); a flight's side drawn on to its end from the
    // piece before it.
    let walled = |line: &Line, hh: &[f32], owner: &[u32]| -> Vec<Option<Wall>> {
        let js: Vec<Judged> = line.pieces.iter().map(|pc| judge(line, pc, hh, owner)).collect();
        let n = js.len();
        let gap: isize = if line.seat.is_some() { 1 } else { 2 };
        let at = |k: isize| -> Option<usize> {
            if line.closed {
                Some(k.rem_euclid(n as isize) as usize)
            } else {
                (k >= 0 && (k as usize) < n).then_some(k as usize)
            }
        };
        let over = |k: usize| match js[k] {
            Judged::Step(_, _, r) if r.abs() > RISE_FT => Some(r < 0.0),
            _ => None,
        };
        let stepping = |k: usize, low: bool| matches!(js[k], Judged::Step(_, _, r) if r.abs() > 1.0 && (r < 0.0) == low);
        // Per piece: walled (`Some(Some(low beyond))`), out of reach (`Some(None)`) or neither.
        let mut st: Vec<Option<Option<bool>>> = (0..n).map(|k| over(k).map(Some).or((js[k] == Judged::Unknown).then_some(None))).collect();
        for k in 0..n {
            if st[k].is_some() {
                continue;
            }
            // (The nearest walled piece each way within `gap`, every piece between still stepping.)
            let find = |dir: isize| -> Option<bool> {
                let mut between = Vec::new();
                for d in 1..=gap {
                    let q = at(k as isize + dir * d)?;
                    if let Some(l) = over(q) {
                        return between.iter().all(|&b| stepping(b, l)).then_some(l);
                    }
                    between.push(q);
                }
                None
            };
            if let (Some(a), Some(b)) = (find(-1), find(1)) {
                if a == b && stepping(k, a) {
                    st[k] = Some(Some(a));
                }
            }
        }
        let mut out: Vec<Option<Wall>> = vec![None; n];
        // (Round a yard, from a piece walled not at all; a yard walled all round keeps its ring.)
        let s = if line.closed { (0..n).find(|&k| st[k].is_none()).unwrap_or(0) } else { 0 };
        let order: Vec<usize> = (0..n).map(|d| (s + d) % n).collect();
        let len = |k: usize| crate::town::geom::dist(line.pieces[k].a, line.pieces[k].b);
        let mut i = 0;
        while i < n {
            if st[order[i]].is_none() {
                i += 1;
                continue;
            }
            // One run: walled or out-of-reach pieces, the walled all one way.
            let (mut j, mut low, mut ft, mut any) = (i, None, 0.0, false);
            while j < n {
                match st[order[j]] {
                    Some(Some(l)) if low.is_some_and(|w| w != l) => break,
                    Some(Some(l)) => {
                        low = Some(l);
                        any = true;
                    }
                    Some(None) => {}
                    None => break,
                }
                ft += len(order[j]);
                j += 1;
            }
            if any && ft >= MIN_RUN_FT - 1e-6 {
                for &k in &order[i..j] {
                    if let (Some(Some(l)), Judged::Step(hi, hb, r)) = (st[k], js[k]) {
                        out[k] = Some(Wall { step: Some((hi, hb, r)), low: l });
                    }
                }
                if !line.closed {
                    let low = low.unwrap_or(false);
                    for by in [(i == 1).then_some(0), (j + 1 == n).then_some(n - 1)].into_iter().flatten() {
                        if out[by].is_none() && js[by] != Judged::Unknown && !matches!(js[by], Judged::Step(_, _, r) if r.abs() > 1.0 && (r < 0.0) != low) {
                            out[by] = Some(Wall { step: None, low });
                        }
                    }
                }
            }
            i = j.max(i + 1);
        }
        out
    };
    // (A yard's squares, level by their house, shade their relief soft: never dark rock by it.)
    marks = (0..HS * HS).map(|h| if owner[h] != u32::MAX { BY_WALL } else { 0 }).collect();
    let owned = |m: [f64; 2]| m[0] >= ox && m[0] < ox + size && m[1] >= oy && m[1] < oy + size;
    let local = |p: [f64; 2]| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32];
    for line in &lines {
        let ws = walled(line, hh, &owner);
        let n = ws.len();
        for w in ws.iter().flatten() {
            if let Some((_, hb, r)) = w.step {
                if r.abs() >= 4.0 {
                    marks[hb] |= HARD_SQ;
                }
            }
        }
        // The chunk's pieces in runs (each drawn by the chunk its middle lies in), round a corner
        // as one line; a yard walled all round from a piece mid-side.
        let draw = |k: usize| {
            let pc = &line.pieces[k];
            ws[k].filter(|_| owned([0.5 * (pc.a[0] + pc.b[0]), 0.5 * (pc.a[1] + pc.b[1])]))
        };
        let s = if !line.closed {
            0
        } else if let Some(s) = (0..n).find(|&k| draw(k).is_none_or(|w| ws[0].is_some_and(|w0| w0.low != w.low))) {
            s
        } else {
            (0..n).find(|&k| !line.pieces[k].corner).unwrap_or(0)
        };
        let mut k = 0;
        while k < n {
            let Some(w0) = draw((s + k) % n) else {
                k += 1;
                continue;
            };
            let mut pts = vec![line.pieces[(s + k) % n].a];
            let mut rise = 0f32;
            let mut q = k;
            while q < n {
                match draw((s + q) % n) {
                    Some(w) if w.low == w0.low => {
                        pts.push(line.pieces[(s + q) % n].b);
                        rise = rise.max(w.step.map_or(0.0, |s| s.2.abs()));
                    }
                    _ => break,
                }
                q += 1;
            }
            // (The low side on the left-hand normal (-uy, ux) of the line as drawn.)
            let pc = &line.pieces[(s + k) % n];
            let u = [pc.b[0] - pc.a[0], pc.b[1] - pc.a[1]];
            let toward = if w0.low { pc.nrm } else { [-pc.nrm[0], -pc.nrm[1]] };
            if -u[1] * toward[0] + u[0] * toward[1] < 0.0 {
                pts.reverse();
            }
            let mut out: Vec<[f32; 2]> = pts.iter().map(|&p| local(p)).collect();
            out.push([rise.max(RISE_FT), 0.0]);
            shapes.push(VectorShape { kind: ShapeKind::Retaining, size: (COPING_FT / SQUARE_FT) as f32, pts: out });
            k = q;
        }
    }
    marks
}
