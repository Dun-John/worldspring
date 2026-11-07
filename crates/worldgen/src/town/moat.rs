//! Moats (v63 castles): a wet moat or a dry ditch round a castle's outer curtain, crossed by a
//! drawbridge at its gate.
//!
//! Which castles: a city's or a metropolis's citadel the town planned, and a castle drawn by hand
//! (a site), never a palace (nor yet one set on a town's ward by hand). About half of them by roll, the odds
//! weighted to older and more defensive castles (`odds`): the town's age (`Layout::roof_age`, its
//! base weathering), a fortress town, a capital's, an inner curtain wanted; a hand-drawn castle's by
//! its ruin, its size and its keep.
//!
//! Wet or dry: wet where water lies near (a river's channel within `NEAR_FT`, a lake or the sea
//! that far out from it: a channel can feed it) on ground level enough to hold it (the castle's
//! ground flat, the moat's line within `WET_RELIEF` ft), its water `LEVEL_DOWN` ft under the lowest
//! bank; else (a hill castle on its brow, a shell keep's
//! motte, dry ground) a ditch `DITCH_FT` deep with steep sides and a low bank along its outer lip.
//!
//! Where: `BERM` ft out from the curtain's line (the wall's foot clear) and `TOWER_BERM` round its
//! towers, `width` wide, along every side of the curtain the castle's own (not along the town wall
//! where that is the castle's); a town's castle keeps it to its own block, the curtain's bound
//! drawn in by the moat's width (`Plan::room`) so its streets and its neighbours' lots stay as they
//! were. Roads crossing it (but the castle's own way from the gate) stand on causeways. The
//! drawbridge spans it straight out from the gate (`Drawbridge`); a town castle's way to its gate
//! leaves straight out over it (`castle::gate_way`).

use super::Ground;
use super::geom::*;

/// The moat's inner edge stands this far out from the curtain's line (ft): as far as the curtain's
/// bound stands in from its block, so a town castle's moat reaches its block's edge.
pub const BERM: f64 = 6.0;
/// And this far clear of a tower's foot (ft).
pub const TOWER_BERM: f64 = 2.0;
/// Water lies within this far of the moat (ft) for it to be wet: a channel from it can feed the moat.
const NEAR_FT: f64 = 480.0;
/// The moat's line rises and falls no more than this (ft) for it to hold water.
const WET_RELIEF: f64 = 8.0;
/// A wet moat's water stands this far under its lowest bank (ft).
const LEVEL_DOWN: f64 = 2.0;
/// A dry ditch's depth (ft) and its outer bank's width (ft) and height (ft).
pub const DITCH_FT: f32 = 8.0;
pub const BANK_FT: f64 = 4.0;
pub const BANK_UP: f32 = 2.0;
/// A wet moat's depth at its edges and in its middle (ft): deep water (`Surface::Deep`, over 4 ft)
/// but at its edges.
pub const EDGE_DEEP: f32 = 1.5;
pub const MID_DEEP: f32 = 6.5;
/// The drawbridge's width (ft): the castle's way's.
pub const BRIDGE_W: f64 = 10.0;

/// A moat rolled for a castle before it is laid out: its width (ft).
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub width: f64,
}

impl Plan {
    /// How much further in a town castle's curtain stands from its block for the moat to fit (ft).
    pub fn room(self) -> f64 {
        self.width
    }
}

/// The drawbridge: from the gate's middle on the curtain's line (`at`) straight out (`dir`, unit)
/// over the moat to the far bank, `span` ft, `width` ft wide.
#[derive(Clone, Copy, Debug)]
pub struct Drawbridge {
    pub at: P,
    pub dir: P,
    pub width: f64,
    pub span: f64,
}

impl Drawbridge {
    /// Whether `p` is on its deck.
    pub fn covers(&self, p: P) -> bool {
        let d = sub(p, self.at);
        let (along, across) = (dot(d, self.dir), cross(self.dir, d));
        (0.0..=self.span).contains(&along) && across.abs() <= 0.5 * self.width
    }
}

/// A castle's moat (world ft once its layout is; see the module).
#[derive(Clone, Debug)]
pub struct Moat {
    /// Water in it (else a dry ditch).
    pub wet: bool,
    /// Its width (ft), past the berm.
    pub width: f64,
    /// A wet moat's water level (ft); a dry ditch's lowest bank.
    pub level: f32,
    /// The outer curtain's line it runs round (anticlockwise) and, by side, whether it runs along
    /// that side (not where the town wall is the castle's).
    pub ring: Vec<P>,
    pub sides: Vec<bool>,
    /// The curtain's towers it keeps clear of (centre, radius).
    pub towers: Vec<(P, f64)>,
    /// The ground it keeps to (a town castle's block, convex; empty: none).
    pub bound: Vec<P>,
    /// Roads over it on causeways (ends, half width).
    pub causeways: Vec<(P, P, f64)>,
    pub bridge: Drawbridge,
    /// Its outline at plan zoom: convex pieces (its inner edge to the far bank, a dry ditch's bank
    /// left out: side by side, round the corners and its towers), cut to `bound`, none where the
    /// drawbridge or a causeway crosses it (`plan_pieces`); and by piece, its inked edges (bit k
    /// the edge from corner k to k + 1; past the eighth, all inked).
    pub pieces: Vec<Vec<P>>,
    pub inks: Vec<u8>,
    /// Bounding box of all of it (x0, y0, x1, y1).
    pub bbox: [f64; 4],
}

/// Where a point lies across a moat: `inner` ft past its inner edge (the berm's end, or a tower's),
/// `outer` ft short of its far edge (or of its bound); in it where both are 0 or more.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub inner: f64,
    pub outer: f64,
}

impl Spot {
    pub fn inside(self) -> bool {
        self.inner >= 0.0 && self.outer >= 0.0
    }
}

impl Moat {
    /// Where `p` lies across the moat, if by it (outside the curtain, off the causeways, within its
    /// bound, beside a side it runs along, within a square of it).
    pub fn spot(&self, p: P) -> Option<Spot> {
        let r = self.width + BERM + 20.0;
        if p[0] < self.bbox[0] - 6.0 || p[1] < self.bbox[1] - 6.0 || p[0] > self.bbox[2] + 6.0 || p[1] > self.bbox[3] + 6.0 || contains(&self.ring, p) {
            return None;
        }
        let n = self.ring.len();
        let (mut d, mut e) = (f64::MAX, 0);
        for k in 0..n {
            let dk = seg_dist(p, self.ring[k], self.ring[(k + 1) % n]);
            if dk < d {
                (d, e) = (dk, k);
            }
        }
        if d > r || !self.sides[e] {
            return None;
        }
        let tower = self.towers.iter().map(|&(t, tr)| dist(p, t) - tr - TOWER_BERM).fold(f64::MAX, f64::min);
        let inner = (d - BERM).min(tower);
        let mut outer = self.width - inner;
        if !self.bound.is_empty() {
            if !contains(&self.bound, p) {
                return None;
            }
            let m = self.bound.len();
            let edge = (0..m).map(|k| seg_dist(p, self.bound[k], self.bound[(k + 1) % m])).fold(f64::MAX, f64::min);
            outer = outer.min(edge);
        }
        if inner < -5.0 || outer < -5.0 || self.causeways.iter().any(|&(a, b, hw)| seg_dist(p, a, b) <= hw) {
            return None;
        }
        Some(Spot { inner, outer })
    }

    /// Moved by `by` (local to world).
    pub fn shift(&mut self, by: P) {
        let mv = |q: &mut P| *q = add(*q, by);
        self.ring.iter_mut().chain(&mut self.bound).chain(self.pieces.iter_mut().flatten()).for_each(mv);
        self.towers.iter_mut().for_each(|t| mv(&mut t.0));
        self.causeways.iter_mut().for_each(|c| {
            mv(&mut c.0);
            mv(&mut c.1);
        });
        mv(&mut self.bridge.at);
        self.bbox = [self.bbox[0] + by[0], self.bbox[1] + by[1], self.bbox[2] + by[0], self.bbox[3] + by[1]];
    }
}

/// The odds of a moat (see the module): `age` 0-1 (older higher), `fortress` a fortress town's or a
/// ruin's, `strong` a capital's, one with an inner curtain or a keep, `great` a metropolis's or a
/// big one's. From 0.38 to 0.9, about half on average (seeds 1-8).
pub fn odds(age: f64, fortress: bool, strong: bool, great: bool) -> f64 {
    (0.38 + 0.25 * age.clamp(0.0, 1.0) + if fortress { 0.15 } else { 0.0 } + if strong { 0.1 } else { 0.0 } + if great { 0.05 } else { 0.0 }).min(0.9)
}

/// A moat by roll (`roll` and `wide` uniform 0-1): `p` its odds, the wider for a great castle.
pub fn roll(roll: f64, wide: f64, p: f64, great: bool) -> Option<Plan> {
    (roll < p).then(|| Plan { width: 15.0 + 5.0 * wide + if great { 4.0 } else { 0.0 } })
}

/// Lay the moat round a castle's outer curtain (`ring`, local ft; `walled` its sides that are the
/// town wall), its gate at `gate` (`inward` the way in), within `bound` (empty: none). `flat`: the
/// castle's ground is (the curtain keeps to its block); `motte`: a shell keep's mound stands in it.
/// Roads in `l` crossing it but its gate's way get causeways. Pushed to `l.moats`.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay<G: Ground>(site: &G, l: &mut super::Layout, plan: Plan, ring: &[P], walled: &[bool], bound: &[P], gate: P, inward: P, flat: bool, motte: bool) {
    let n = ring.len();
    if n < 3 || walled.len() != n {
        return;
    }
    let (mut ring, mut sides) = (ring.to_vec(), walled.iter().map(|w| !w).collect::<Vec<bool>>());
    if area(&ring) < 0.0 {
        ring.reverse();
        // (Side k ran from corner k to k + 1; reversed, from n - 1 - k to n - k.)
        sides.reverse();
        sides.rotate_left(1);
    }
    if !sides.iter().any(|&s| s) {
        return;
    }
    let w = plan.width;
    let out_of = |k: usize| {
        let (a, b) = (ring[k], ring[(k + 1) % n]);
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        [u[1], -u[0]]
    };
    // The curtain's towers (on its line).
    let towers: Vec<(P, f64)> = l
        .towers
        .iter()
        .map(|t| (*t, 11.0 * if l.square_tower(*t).is_some() { std::f64::consts::SQRT_2 } else { 1.0 }))
        .chain(l.gate_towers.iter().map(|t| (*t, 15.0)))
        .filter(|&(t, _)| (0..n).any(|k| seg_dist(t, ring[k], ring[(k + 1) % n]) <= 4.0))
        .collect();
    // Its middle line, every 10 ft: the ground it is dug in, and whether water lies near.
    let mut heights: Vec<f64> = Vec::new();
    let mut near = false;
    for k in (0..n).filter(|&k| sides[k]) {
        let (a, b, o) = (ring[k], ring[(k + 1) % n], out_of(k));
        let steps = crate::core::ceil(dist(a, b) / 10.0).max(1.0) as usize;
        for j in 0..=steps {
            let q = add(lerp(a, b, j as f64 / steps as f64), mul(o, BERM + 0.5 * w));
            if !bound.is_empty() && !contains(bound, q) {
                continue;
            }
            heights.push(site.height(q));
            if !near && j % 2 == 0 {
                near = site.near_water(q, NEAR_FT) || [60.0, 150.0, 250.0, 360.0, NEAR_FT].iter().any(|&f| site.wet(add(q, mul(o, f))));
            }
        }
    }
    if heights.is_empty() {
        return;
    }
    let (lo, hi) = heights.iter().fold((f64::MAX, f64::MIN), |r, &h| (r.0.min(h), r.1.max(h)));
    let wet = near && flat && !motte && hi - lo <= WET_RELIEF;
    let level = if wet { lo - LEVEL_DOWN } else { lo } as f32;
    // Roads over it on causeways: all but the gate's own way (the drawbridge carries it).
    let gate_way = |pts: &[P]| pts.first().is_some_and(|p| dist(*p, gate) <= 8.0);
    let causeways: Vec<(P, P, f64)> =
        l.roads.iter().filter(|r| !gate_way(&r.0)).flat_map(|(pts, _, rw)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * rw))).filter(|&(a, b, _)| (0..n).any(|k| sides[k] && seg_dist(ring[k], a, b).min(seg_dist(a, ring[k], ring[(k + 1) % n])) <= BERM + w + 40.0)).collect();
    let mut moat = Moat {
        wet,
        width: w,
        level,
        ring,
        sides,
        towers,
        bound: bound.to_vec(),
        causeways,
        bridge: Drawbridge { at: gate, dir: mul(inward, -1.0 / len(inward).max(1e-9)), width: BRIDGE_W, span: 0.0 },
        pieces: Vec::new(),
        inks: Vec::new(),
        bbox: [f64::MAX, f64::MAX, f64::MIN, f64::MIN],
    };
    let r = BERM + w + 15.0;
    for q in &moat.ring {
        moat.bbox = [moat.bbox[0].min(q[0] - r), moat.bbox[1].min(q[1] - r), moat.bbox[2].max(q[0] + r), moat.bbox[3].max(q[1] + r)];
    }
    // The drawbridge: from the gate out to a foot past the far bank (or the first foot off the moat
    // past its middle).
    let mut span = BERM + w;
    let mut seen = false;
    for f in 0..=((BERM + w + 30.0) as usize) {
        let q = add(gate, mul(moat.bridge.dir, f as f64));
        let inside = moat.spot(q).is_some_and(|s| s.inside());
        if inside {
            seen = true;
        } else if seen {
            span = f as f64 + 1.0;
            break;
        }
    }
    moat.bridge.span = span;
    (moat.pieces, moat.inks) = plan_pieces(&moat);
    l.moats.push(moat);
}

/// Rays out from the curtain's line this far apart along a side (ft), this far round an outer
/// corner (radians), for the plan pieces.
const PIECE_STEP: f64 = 4.0;
const PIECE_TURN: f64 = 0.25;

/// A moat's plan pieces and their inked edges (`Moat::pieces`, `inks`): quads between rays out from
/// the curtain's line (along each side it runs along, and round each outer corner between two),
/// each from the moat's inner edge to its far bank as `spot` has them (`BERM` out, bent round a
/// tower `TOWER_BERM` clear of its foot, the width beyond); none where the drawbridge or a causeway
/// crosses it (found to the foot between the rays), so the road shows through. Quads on a straight
/// run clear of towers are one. Its banks are inked, and its ends at a gap or where it stops, not
/// the joins between its pieces.
fn plan_pieces(m: &Moat) -> (Vec<Vec<P>>, Vec<u8>) {
    let (ring, sides, n) = (&m.ring, &m.sides, m.ring.len());
    let wide = m.width - if m.wet { 0.0 } else { BANK_FT };
    let out_of = |k: usize| {
        let (a, b) = (ring[k], ring[(k + 1) % n]);
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        [u[1], -u[0]]
    };
    let turn_at = |k: usize| {
        let (o, o2) = (out_of(k), out_of((k + 1) % n));
        libm::atan2(cross(o, o2), dot(o, o2))
    };
    // How far out along a ray from `q` (unit `d`) the moat's inner edge (`extra` 0) or its far bank
    // (`extra` its width) lies: `base` ft, or round the towers.
    let clear = |q: P, d: P, base: f64, extra: f64| {
        m.towers.iter().fold(base, |h, &(t, tr)| {
            let r = tr + TOWER_BERM + extra;
            let rel = sub(t, q);
            let (tp, ta) = (dot(rel, d), cross(d, rel));
            if ta.abs() < r && tp.abs() < r { h.max(tp + crate::core::sqrt(r * r - ta * ta)) } else { h }
        })
    };
    let gap = |p: P| m.bridge.covers(p) || m.causeways.iter().any(|&(a, b, hw)| seg_dist(p, a, b) <= hw);
    let (mut pieces, mut inks) = (Vec::new(), Vec::new());
    // Runs: side k (`None`) or the corner after it (`Some(turn)`).
    let mut runs: Vec<(usize, Option<f64>)> = Vec::new();
    for k in (0..n).filter(|&k| sides[k]) {
        runs.push((k, None));
        let turn = turn_at(k);
        if sides[(k + 1) % n] && turn > 0.01 {
            runs.push((k, Some(turn)));
        }
    }
    for (k, corner) in runs {
        let (a, b, o) = (ring[k], ring[(k + 1) % n], out_of(k));
        let ray = |t: f64| -> (P, P) {
            match corner {
                None => (lerp(a, b, t), o),
                Some(turn) => {
                    let ang = libm::atan2(o[1], o[0]) + turn * t;
                    (b, [libm::cos(ang), libm::sin(ang)])
                }
            }
        };
        // (Its inner edge, its far bank, whether clear of towers.)
        let span = |t: f64| {
            let (q, d) = ray(t);
            let (hi, ho) = (clear(q, d, BERM, 0.0), clear(q, d, BERM + wide, wide));
            (hi, ho, hi <= BERM && ho <= BERM + wide)
        };
        let mid = |t: f64| {
            let (q, d) = ray(t);
            let (hi, ho, _) = span(t);
            add(q, mul(d, 0.5 * (hi + ho)))
        };
        let steps = match corner {
            None => crate::core::ceil(dist(a, b) / PIECE_STEP).max(1.0),
            Some(turn) => crate::core::ceil(turn / PIECE_TURN).max(1.0),
        } as usize;
        // The rays' params, with one more at each gap's edge (to a foot or so).
        let mut ts: Vec<f64> = Vec::with_capacity(steps + 3);
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            if let Some(&t0) = ts.last() {
                if gap(mid(t0)) != gap(mid(t)) {
                    let (mut lo, mut hi) = (t0, t);
                    for _ in 0..10 {
                        let c = 0.5 * (lo + hi);
                        if gap(mid(c)) == gap(mid(t0)) { lo = c } else { hi = c }
                    }
                    ts.push(0.5 * (lo + hi));
                }
            }
            ts.push(t);
        }
        let shut: Vec<bool> = ts.windows(2).map(|w| gap(mid(0.5 * (w[0] + w[1])))).collect();
        let plain: Vec<bool> = ts.iter().map(|&t| span(t).2).collect();
        // Whether the run carries on past its start and its end (into the next side's or corner's).
        let (pk, nk) = ((k + n - 1) % n, (k + 1) % n);
        let (on_start, on_end) = match corner {
            None => (sides[pk], sides[nk]),
            Some(_) => (true, true),
        };
        let mut i = 0;
        while i + 1 < ts.len() {
            if shut[i] {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            if corner.is_none() {
                while j + 1 < ts.len() && !shut[j] && plain[i] && plain[j] && plain[j + 1] {
                    j += 1;
                }
            }
            let ((q0, d0), (q1, d1)) = (ray(ts[i]), ray(ts[j]));
            let ((hi0, ho0, _), (hi1, ho1, _)) = (span(ts[i]), span(ts[j]));
            let mut p = vec![add(q0, mul(d0, hi0)), add(q1, mul(d1, hi1)), add(q1, mul(d1, ho1)), add(q0, mul(d0, ho0))];
            let ink0 = if i == 0 { !on_start } else { shut[i - 1] };
            let ink1 = if j + 1 == ts.len() { !on_end } else { shut[j] };
            // At an inner corner the two sides' quads meet on its bisector.
            let mut joins: Vec<(P, P)> = Vec::new();
            if corner.is_none() {
                if sides[pk] && turn_at(pk) < -0.01 {
                    let nn = sub(o, out_of(pk));
                    p = clip_plain(&p, nn, dot(a, nn));
                    joins.push((a, mul([nn[1], -nn[0]], 1.0 / len(nn).max(1e-9))));
                }
                if sides[nk] && turn_at(k) < -0.01 {
                    let nn = sub(o, out_of(nk));
                    p = clip_plain(&p, nn, dot(b, nn));
                    joins.push((b, mul([nn[1], -nn[0]], 1.0 / len(nn).max(1e-9))));
                }
            }
            if !m.bound.is_empty() && p.len() >= 3 {
                let bound = &m.bound;
                let mb = bound.len();
                let ccw = area(bound) > 0.0;
                for e in 0..mb {
                    let (ea, eb) = (bound[e], bound[(e + 1) % mb]);
                    let u = mul(sub(eb, ea), 1.0 / dist(ea, eb).max(1e-9));
                    let on = if ccw { [u[1], -u[0]] } else { [-u[1], u[0]] };
                    p = clip_plain(&p, on, dot(ea, on));
                    if p.len() < 3 {
                        break;
                    }
                }
            }
            // (Corners the cuts left a hair apart are one.)
            p.dedup_by(|x, y| dist(*x, *y) < 1e-3);
            while p.len() > 1 && dist(p[0], p[p.len() - 1]) < 1e-3 {
                p.pop();
            }
            if p.len() >= 3 && area(&p).abs() > 1.0 {
                let on_line = |x: P, q: P, d: P| cross(d, sub(x, q)).abs() < 1e-6 * (1.0 + len(sub(x, q)));
                let both = |x: P, y: P, q: P, d: P| on_line(x, q, d) && on_line(y, q, d);
                let mut mask = 0u8;
                for e in 0..p.len().min(8) {
                    let (x, y) = (p[e], p[(e + 1) % p.len()]);
                    let join = (both(x, y, q0, d0) && !ink0) || (both(x, y, q1, d1) && !ink1) || joins.iter().any(|&(q, d)| both(x, y, q, d));
                    if !join {
                        mask |= 1 << e;
                    }
                }
                pieces.push(p);
                inks.push(mask);
            }
            i = j;
        }
    }
    (pieces, inks)
}
