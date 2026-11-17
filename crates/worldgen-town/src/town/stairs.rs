//! Flights of steps on steep paved streets (cities and up). Every street is judged once per
//! layout from the ground it is planned on (`Site::height`): stations every 5 ft, the grade by a
//! central difference over 10 ft, steps where it climbs past `g_on` until it falls below `g_off`.
//! A mesh edge is judged once for all its pieces (both side strips and the main street), so a
//! street's whole width steps together. Flights stop at landings clear of every street meeting
//! at the corner, short of plazas, bridges and the water; alleys stop at their ends and either
//! side of an alley meeting them.

use super::geom::*;
use super::{Layout, Site, Stair};
use crate::World;
use crate::core::hash::FastMap;
use crate::t0::T0;
use crate::t0::settle::Tier;

/// Steps start where the grade reaches `g_on` and stop where it falls below `g_off`; a flight is
/// at least `min_run` ft long and rises at least `min_rise` ft.
#[derive(Clone, Copy, Debug)]
pub struct StairParams {
    pub g_on: f64,
    pub g_off: f64,
    pub min_run: f64,
    pub min_rise: f64,
}

impl StairParams {
    pub const DEFAULT: StairParams = StairParams { g_on: 0.15, g_off: 0.12, min_run: 15.0, min_rise: 2.5 };
}

/// The least grade the survey looks at further (its gentlest `g_off`).
pub const SURVEY_FROM: f64 = 0.09;
/// The smallest settlement with stepped streets.
pub const STAIR_TIER: Tier = Tier::City;
const STATION: f64 = 5.0;
/// The shortest landing at a flight's end (ft), and the room left past the streets that meet it:
/// the steps come down to the street crossing them.
const LANDING_MIN: f64 = 2.0;
const CLEAR: f64 = 0.5;
/// How far steps keep off a plaza (ft).
const PLAZA_CLEAR: f64 = 6.0;
/// An alley's landing either side of an alley meeting it: half its width and `CLEAR` (its own ends
/// lie on the edge of the street they meet: `LANDING_MIN` there).
const ALLEY_LANDING: f64 = CLEAR + super::ALLEY_HALF_FT;
/// A flight goes on down the foot of its slope (and up over its brow) while the grade is this.
const TOE: f64 = 0.08;
const GRID: f64 = 80.0;

/// What a street is, for the survey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A patch edge: its side strips, and the main street when it runs along it.
    Edge,
    Alley,
    /// The town end of a road's approach, to its gate.
    Approach,
}

/// One street's stations, judged once. `line` is the edge (lower corner first) or the street's own
/// polyline; `pieces` the strips and main street along an edge (same direction as `line`).
#[derive(Clone, Debug)]
pub struct Profile {
    pub kind: Kind,
    /// 4 when a main street or an approach, else 3.
    pub class: u8,
    pub line: Vec<P>,
    pub len: f64,
    pub pieces: Vec<(P, P, f64)>,
    /// The street's width when it has no pieces.
    pub width: f64,
    /// Station positions along `line` (every 5 ft from its start, and its end), ground height,
    /// grade, and level (where steps never go: water and ground below it, bridges, plazas, alleys
    /// meeting it).
    pub s: Vec<f64>,
    pub h: Vec<f64>,
    pub g: Vec<f64>,
    pub level: Vec<bool>,
    /// Landing lengths at the start and the end.
    pub ends: [f64; 2],
}

impl Profile {
    fn h_at(&self, s: f64) -> f64 {
        let k = (crate::core::floor(s / STATION) as usize).min(self.s.len() - 2);
        let t = ((s - self.s[k]) / (self.s[k + 1] - self.s[k]).max(1e-9)).clamp(0.0, 1.0);
        self.h[k] + (self.h[k + 1] - self.h[k]) * t
    }
}

/// The streets of a town as `build_town` laid them out (local ft), recorded as they are pushed.
#[derive(Default)]
pub(super) struct Pieces {
    /// Per mesh edge (lower corner first), in the order first met: its pieces from the lower
    /// corner's end, and whether one is a main street.
    edges: Vec<((usize, usize), Vec<(P, P, f64)>, bool)>,
    at: FastMap<(usize, usize), usize>,
    /// `l.roads` entries that are edge pieces or streets over bridges (the rest of classes 3 and
    /// 4 are alleys and approaches).
    covered: Vec<usize>,
    /// Edges carrying a street over a bridge: never stepped.
    bridged: Vec<(usize, usize)>,
    /// Approaches (`l.roads` index) and the corner of their gate.
    approaches: Vec<(usize, usize)>,
    /// Streets over bridges at their bank corners (corner, centreline, width): never stepped,
    /// but flights stop short of them.
    banks: Vec<(usize, P, P, f64)>,
}

impl Pieces {
    /// A street piece along mesh edge (`va`, `vb`), from `a` (at `va`'s end) to `b`; `road` its
    /// `l.roads` index.
    pub fn edge(&mut self, va: usize, vb: usize, a: P, b: P, width: f64, main: bool, road: usize) {
        let (e, a, b) = if va <= vb { ((va, vb), a, b) } else { ((vb, va), b, a) };
        let n = self.edges.len();
        let k = *self.at.entry(e).or_insert(n);
        if k == n {
            self.edges.push((e, Vec::new(), false));
        }
        let entry = &mut self.edges[k];
        // Main streets from several gates share their last edges.
        if !(main && entry.2) {
            entry.1.push((a, b, width));
        }
        entry.2 |= main;
        self.covered.push(road);
    }
    /// A street over a bridge from bank corner `a` by river corner `v` to bank corner `b`, along
    /// `pts` (`width` wide); `road` its `l.roads` index when it is its own street.
    pub fn bridge(&mut self, (a, v, b): (usize, usize, usize), pts: &[P], width: f64, road: Option<usize>) {
        self.bridged.extend([(a.min(v), a.max(v)), (v.min(b), v.max(b))]);
        self.covered.extend(road);
        let n = pts.len();
        if n >= 2 {
            self.banks.push((a, pts[0], pts[1], width));
            self.banks.push((b, pts[n - 2], pts[n - 1], width));
        }
    }
    /// A main street (`l.roads` index): judged along its edges, never as a whole.
    pub fn main_street(&mut self, road: usize) {
        self.covered.push(road);
    }
    pub fn approach(&mut self, road: usize, gate: usize) {
        self.approaches.push((road, gate));
    }
}

/// Who a street footprint at a corner belongs to (a street's landing ignores its own).
#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Edge(usize),
    Road(usize),
    Bridge,
}

/// Every street judged for steps (local ft). Stations at or above grade `g_min` are looked at
/// further (level, landings); below it steps never start.
pub(super) fn profiles(site: &Site, pos: &[P], pieces: &Pieces, l: &Layout, g_min: f64) -> Vec<Profile> {
    // Street footprints at each corner: (owner, centreline segment, half width).
    let mut meet: FastMap<usize, Vec<(Owner, P, P, f64)>> = FastMap::default();
    for (k, (e, ps, _)) in pieces.edges.iter().enumerate() {
        for &(a, b, w) in ps {
            for v in [e.0, e.1] {
                meet.entry(v).or_default().push((Owner::Edge(k), a, b, 0.5 * w));
            }
        }
    }
    for &(v, a, b, w) in &pieces.banks {
        meet.entry(v).or_default().push((Owner::Bridge, a, b, 0.5 * w));
    }
    for &(r, g) in &pieces.approaches {
        let (pts, _, w) = &l.roads[r];
        for s in pts.windows(2).filter(|s| seg_dist(pos[g], s[0], s[1]) < 60.0) {
            meet.entry(g).or_default().push((Owner::Road(r), s[0], s[1], 0.5 * w));
        }
    }
    let plazas: Vec<([f64; 4], &Vec<P>)> = l.plazas.iter().map(|p| (bbox(&p.pts, PLAZA_CLEAR), &p.pts)).collect();
    let bridges: Vec<([f64; 4], &Vec<P>)> = l.bridges.iter().map(|p| (bbox(p, 0.0), p)).collect();
    let level_at = |p: P| {
        site.wet(p)
            || site.below_shore(p)
            || site.near_river(p, 10.0)
            || bridges.iter().any(|(bb, poly)| inside(bb, p) && contains(poly, p))
            || plazas.iter().any(|(bb, poly)| inside(bb, p) && (contains(poly, p) || (0..poly.len()).any(|k| seg_dist(p, poly[k], poly[(k + 1) % poly.len()]) < PLAZA_CLEAR)))
    };
    let mut out = Vec::new();

    for (k, (e, ps, main)) in pieces.edges.iter().enumerate() {
        if pieces.bridged.contains(e) {
            continue;
        }
        let mut p = stations(site, vec![pos[e.0], pos[e.1]], Kind::Edge, if *main { 4 } else { 3 }, 0.0);
        p.pieces = ps.clone();
        if p.g.iter().any(|&g| g >= g_min) {
            // Where it may be steep, every piece is measured (a strip beside a shore bank can lie
            // flat while the edge climbs): the street steps where all of it climbs, as steeply
            // as its gentlest piece.
            let len = p.len;
            let per: Vec<(Vec<f64>, Vec<f64>)> = ps
                .iter()
                .map(|&(a, b, _)| {
                    let h: Vec<f64> = p.s.iter().map(|&s| site.height(lerp(a, b, s / len.max(1e-9)))).collect();
                    let g = grades(&p.s, &h);
                    (h, g)
                })
                .collect();
            for j in 0..p.s.len() {
                p.h[j] = per.iter().map(|q| q.0[j]).sum::<f64>() / per.len() as f64;
                p.g[j] = per.iter().map(|q| q.1[j]).fold(f64::MAX, f64::min);
            }
        }
        if p.g.iter().any(|&g| g >= g_min) {
            for j in 0..p.s.len() {
                let t = p.s[j] / p.len.max(1e-9);
                p.level[j] = p.g[j] >= g_min && ps.iter().any(|&(a, b, _)| level_at(lerp(a, b, t)));
            }
            let len = p.len;
            for (end, v) in [(0, e.0), (1, e.1)] {
                let at = meet.get(&v).map_or(&[][..], |m| &m[..]);
                p.ends[end] = landing(at, Owner::Edge(k), len, &|s, out| {
                    let t = if end == 0 { s / len } else { 1.0 - s / len };
                    for &(a, b, w) in ps {
                        out.push(section(lerp(a, b, t), sub(b, a), 0.5 * w));
                    }
                });
            }
        }
        out.push(p);
    }

    // Alleys and approaches: the class 3 and 4 streets not along an edge.
    let mut covered = vec![false; l.roads.len()];
    for &r in &pieces.covered {
        covered[r] = true;
    }
    let gate_of: FastMap<usize, usize> = pieces.approaches.iter().copied().collect();
    let alleys: Vec<usize> = (0..l.roads.len()).filter(|&r| !covered[r] && l.roads[r].1 == 3).collect();
    let mut ends_grid: FastMap<(i64, i64), Vec<P>> = FastMap::default();
    for &r in &alleys {
        let pts = &l.roads[r].0;
        for q in [pts[0], pts[pts.len() - 1]] {
            ends_grid.entry(cell(q)).or_default().push(q);
        }
    }
    for r in 0..l.roads.len() {
        let (pts, class, w) = &l.roads[r];
        if covered[r] || *class < 3 || pts.len() < 2 {
            continue;
        }
        let alley = *class == 3;
        let mut p = stations(site, pts.clone(), if alley { Kind::Alley } else { Kind::Approach }, *class, *w);
        if !p.g.iter().any(|&g| g >= g_min) {
            out.push(p);
            continue;
        }
        let cum = cumulative(&p.line);
        for j in 0..p.s.len() {
            p.level[j] = p.g[j] >= g_min && level_at(along(&p.line, &cum, p.s[j]).0);
        }
        if alley {
            p.ends = [LANDING_MIN; 2];
            // Either side of an alley ending on this one.
            let (a, b) = (pts[0], pts[pts.len() - 1]);
            let (c0, c1) = (cell([a[0].min(b[0]) - 1.0, a[1].min(b[1]) - 1.0]), cell([a[0].max(b[0]) + 1.0, a[1].max(b[1]) + 1.0]));
            for cy in c0.1..=c1.1 {
                for cx in c0.0..=c1.0 {
                    for &q in ends_grid.get(&(cx, cy)).into_iter().flatten() {
                        if dist(q, a) > 1.0 && dist(q, b) > 1.0 && seg_dist(q, a, b) < 1.0 {
                            let at = dot(sub(q, a), sub(b, a)) / dist(a, b).max(1e-9);
                            for j in 0..p.s.len() {
                                if (p.s[j] - at).abs() <= ALLEY_LANDING {
                                    p.level[j] = true;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // An approach starts where the road comes into town and ends at its gate.
            let len = p.len;
            let gate = gate_of.get(&r).copied();
            let at = gate.and_then(|g| meet.get(&g)).map_or(&[][..], |m| &m[..]);
            let (line, hw) = (&p.line, 0.5 * w);
            p.ends = [LANDING_MIN, landing(at, Owner::Road(r), len, &|s, out| {
                let (c, u) = along(line, &cum, len - s);
                out.push(section(c, u, hw));
            })];
        }
        out.push(p);
    }
    out
}

/// A street's stations: heights and grades (level and landings left to the caller).
fn stations(site: &Site, line: Vec<P>, kind: Kind, class: u8, width: f64) -> Profile {
    let len: f64 = line.windows(2).map(|w| dist(w[0], w[1])).sum();
    let n = (crate::core::ceil(len / STATION) as usize).max(1) + 1;
    let s: Vec<f64> = (0..n).map(|k| if k + 1 == n { len } else { k as f64 * STATION }).collect();
    let cum = cumulative(&line);
    let h: Vec<f64> = s.iter().map(|&s| site.height(along(&line, &cum, s).0)).collect();
    let g = grades(&s, &h);
    Profile { kind, class, line, len, pieces: Vec::new(), width, s, h, g, level: vec![false; n], ends: [0.0; 2] }
}

/// The grade at each station: a central difference over its neighbours (one-sided at the ends).
fn grades(s: &[f64], h: &[f64]) -> Vec<f64> {
    let n = s.len();
    (0..n)
        .map(|k| {
            let (a, b) = (k.saturating_sub(1), (k + 1).min(n - 1));
            (h[b] - h[a]).abs() / (s[b] - s[a]).max(1e-9)
        })
        .collect()
}

/// Where steps stop short of a corner: the first cross-section (each foot from the corner) of
/// the street clear of every other street meeting there, plus `CLEAR`; at least `LANDING_MIN`.
fn landing(meet: &[(Owner, P, P, f64)], own: Owner, len: f64, sections: &dyn Fn(f64, &mut Vec<(P, P)>)) -> f64 {
    let mut cross = Vec::new();
    let mut s = 0.0;
    while s < len {
        cross.clear();
        sections(s, &mut cross);
        if cross.iter().all(|&(a, b)| meet.iter().all(|&(o, c, d, hw)| o == own || seg_seg_dist(a, b, c, d) >= hw)) {
            return (s + CLEAR).max(LANDING_MIN);
        }
        s += 1.0;
    }
    len
}

/// The flights along a street: (start, end, rise) along its line, oriented as the line.
pub fn runs(p: &Profile, prm: &StairParams) -> Vec<(f64, f64, f64)> {
    let n = p.s.len();
    let mut spans: Vec<(f64, f64)> = Vec::new();
    let mut cur: Option<(usize, usize)> = None;
    for k in 0..=n {
        let on = k < n && !p.level[k] && p.g[k] >= if cur.is_some() { prm.g_off } else { prm.g_on };
        let stepped = |j: usize, g: f64| !p.level[j] && p.g[j] >= g;
        if on {
            // Steps start at `g_on` from either side: back down to `g_off` behind.
            cur = Some(cur.map_or_else(
                || {
                    let mut a = k;
                    while a > 0 && stepped(a - 1, prm.g_off) {
                        a -= 1;
                    }
                    (a, k)
                },
                |c| (c.0, k),
            ));
        } else if let Some((mut a, mut b)) = cur.take() {
            // Down the foot of the slope and up over its brow.
            while a > 0 && stepped(a - 1, TOE) {
                a -= 1;
            }
            while b + 1 < n && stepped(b + 1, TOE) {
                b += 1;
            }
            let (s0, s1) = ((p.s[a] - 0.5 * STATION).max(0.0), (p.s[b] + 0.5 * STATION).min(p.len));
            // A level gap of a station or less joins two runs.
            match spans.last_mut() {
                Some(last) if s0 - last.1 <= STATION => last.1 = last.1.max(s1),
                _ => spans.push((s0, s1)),
            }
        }
    }
    spans
        .into_iter()
        .filter_map(|(s0, s1)| {
            let (s0, s1) = (s0.max(p.ends[0]), s1.min(p.len - p.ends[1]));
            let rise = (p.h_at(s1) - p.h_at(s0)).abs();
            (s1 - s0 >= prm.min_run && rise >= prm.min_rise).then_some((s0, s1, rise))
        })
        .collect()
}

/// The depth of a tread drawn (ft): two to a square, whatever the rise (bold enough to read at
/// a battlemap's scale).
pub const TREAD: f64 = 2.5;

/// A stretch of a flight as it is drawn: its axis from the bottom up (world ft), the width it
/// covers, how far up the flight its bottom lies, and the depth of a tread (ft). A street's
/// pieces side by side (one stretch of a mesh edge, straight and parallel) make one axis over
/// their whole width; an alley or approach makes one per segment of its line.
#[derive(Clone, Copy, Debug)]
pub struct Axis {
    pub a: P,
    pub b: P,
    pub width: f64,
    pub s0: f64,
    pub tread: f64,
}

/// The axes of every flight: treads square across them, each as near `TREAD` deep as fits.
pub fn axes(stairs: &[Stair]) -> Vec<Axis> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < stairs.len() {
        let n = stairs[k..].iter().take_while(|s| s.flight == stairs[k].flight).count();
        let flight = &stairs[k..k + n];
        k += n;
        let first = &flight[0];
        let tread = |len: f64| len / crate::core::round(len / TREAD).max(1.0);
        if n == 1 {
            let len: f64 = first.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
            let t = tread(len);
            let mut s0 = 0.0;
            for w in first.pts.windows(2) {
                out.push(Axis { a: w[0], b: w[1], width: first.width, s0, tread: t });
                s0 += dist(w[0], w[1]);
            }
            continue;
        }
        let (a, b) = (first.pts[0], *first.pts.last().unwrap());
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        let nrm = [-u[1], u[0]];
        let (mut t0, mut t1, mut o0, mut o1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for s in flight {
            for p in &s.pts {
                let (t, o) = (dot(sub(*p, a), u), dot(sub(*p, a), nrm));
                (t0, t1, o0, o1) = (t0.min(t), t1.max(t), o0.min(o - 0.5 * s.width), o1.max(o + 0.5 * s.width));
            }
        }
        let mid = mul(nrm, 0.5 * (o0 + o1));
        out.push(Axis { a: add(add(a, mul(u, t0)), mid), b: add(add(a, mul(u, t1)), mid), width: o1 - o0, s0: 0.0, tread: tread(t1 - t0) });
    }
    out
}

/// The flights of steps on these streets, each piece from the bottom up; pieces of one flight
/// (the strips and main street of one stretch of edge) share its number.
pub fn flights(profiles: &[Profile], prm: &StairParams) -> Vec<Stair> {
    let mut out = Vec::new();
    let mut flight = 0u32;
    for p in profiles {
        for (s0, s1, rise) in runs(p, prm) {
            let up = p.h_at(s1) >= p.h_at(s0);
            let mut push = |mut pts: Vec<P>, width: f64| {
                if !up {
                    pts.reverse();
                }
                out.push(Stair { pts, width, rise: rise as f32, flight });
            };
            if p.pieces.is_empty() {
                let cum = cumulative(&p.line);
                let mut pts = vec![along(&p.line, &cum, s0).0];
                pts.extend((1..p.line.len() - 1).filter(|&k| cum[k] > s0 && cum[k] < s1).map(|k| p.line[k]));
                pts.push(along(&p.line, &cum, s1).0);
                push(pts, p.width);
            } else {
                for &(a, b, w) in &p.pieces {
                    push(vec![lerp(a, b, s0 / p.len), lerp(a, b, s1 / p.len)], w);
                }
            }
            flight += 1;
        }
    }
    out
}

/// Every street of settlement `index` judged for steps (world ft), a town's too: the generated
/// layout built once more, uncached. Villages have none.
pub fn survey(world: &World, t0: &T0, index: usize) -> Vec<Profile> {
    let mut out = Vec::new();
    let l = super::generate_with(world, t0, index, None, Some(&mut out));
    let tw = |p: &mut P| *p = add(*p, l.center);
    for p in &mut out {
        p.line.iter_mut().for_each(tw);
        for piece in &mut p.pieces {
            tw(&mut piece.0);
            tw(&mut piece.1);
        }
    }
    out
}

fn section(c: P, dir: P, hw: f64) -> (P, P) {
    let u = mul(dir, 1.0 / len(dir).max(1e-9));
    let n = [-u[1] * hw, u[0] * hw];
    (add(c, n), sub(c, n))
}

fn seg_seg_dist(a: P, b: P, c: P, d: P) -> f64 {
    let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (o3, o4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
        return 0.0;
    }
    seg_dist(a, c, d).min(seg_dist(b, c, d)).min(seg_dist(c, a, b)).min(seg_dist(d, a, b))
}

fn cumulative(line: &[P]) -> Vec<f64> {
    let mut acc = 0.0;
    let mut out = vec![0.0];
    for w in line.windows(2) {
        acc += dist(w[0], w[1]);
        out.push(acc);
    }
    out
}

/// The point `s` ft along a polyline and the way it runs there.
fn along(line: &[P], cum: &[f64], s: f64) -> (P, P) {
    let k = cum.partition_point(|&c| c <= s).clamp(1, line.len() - 1);
    let (a, b) = (line[k - 1], line[k]);
    let d = cum[k] - cum[k - 1];
    (lerp(a, b, ((s - cum[k - 1]) / d.max(1e-9)).clamp(0.0, 1.0)), sub(b, a))
}

fn bbox(poly: &[P], pad: f64) -> [f64; 4] {
    let r = poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])]);
    [r[0] - pad, r[1] - pad, r[2] + pad, r[3] + pad]
}

fn inside(bb: &[f64; 4], p: P) -> bool {
    p[0] >= bb[0] && p[0] <= bb[2] && p[1] >= bb[1] && p[1] <= bb[3]
}

fn cell(p: P) -> (i64, i64) {
    (crate::core::floor(p[0] / GRID) as i64, crate::core::floor(p[1] / GRID) as i64)
}
