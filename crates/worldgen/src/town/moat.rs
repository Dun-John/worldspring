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
    /// Its outline at plan zoom: convex pieces (the berm to the far bank, side by side and round
    /// the corners), cut to `bound`.
    pub pieces: Vec<Vec<P>>,
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
    // Plan pieces: along each side, and round each outer corner between two sides it runs along.
    let mut pieces: Vec<Vec<P>> = Vec::new();
    let (i0, i1) = (BERM, BERM + w - if wet { 0.0 } else { BANK_FT });
    for k in (0..n).filter(|&k| sides[k]) {
        let (a, b, o) = (ring[k], ring[(k + 1) % n], out_of(k));
        pieces.push(vec![add(a, mul(o, i0)), add(b, mul(o, i0)), add(b, mul(o, i1)), add(a, mul(o, i1))]);
        let nk = (k + 1) % n;
        if !sides[nk] {
            continue;
        }
        let o2 = out_of(nk);
        let turn = libm::atan2(cross(o, o2), dot(o, o2));
        if turn <= 0.01 {
            continue;
        }
        let steps = crate::core::ceil(turn / 0.5) as usize;
        let at = |t: f64| {
            let ang = libm::atan2(o[1], o[0]) + turn * t;
            [libm::cos(ang), libm::sin(ang)]
        };
        for s in 0..steps {
            let (u0, u1) = (at(s as f64 / steps as f64), at((s + 1) as f64 / steps as f64));
            pieces.push(vec![add(b, mul(u0, i0)), add(b, mul(u0, i1)), add(b, mul(u1, i1)), add(b, mul(u1, i0))]);
        }
    }
    if !bound.is_empty() {
        let m = bound.len();
        let ccw = area(bound) > 0.0;
        for p in &mut pieces {
            for e in 0..m {
                let (a, b) = (bound[e], bound[(e + 1) % m]);
                let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
                let o = if ccw { [u[1], -u[0]] } else { [-u[1], u[0]] };
                *p = clip_plain(p, o, dot(a, o));
                if p.len() < 3 {
                    break;
                }
            }
        }
        pieces.retain(|p| p.len() >= 3 && area(p).abs() > 1.0);
    }
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
        pieces,
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
    l.moats.push(moat);
}
