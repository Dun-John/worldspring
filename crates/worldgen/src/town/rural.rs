//! v63 countryside: what stands about a settlement's fields and plots. While the layout is made
//! (`place`, last, in world ft): windmills on rises among the fields (a village with no water mill,
//! some round towns' farmland) and a water mill's wheel on its water side (`Layout::wheels`). From
//! the finished layout alone, so every chunk and the plan agree: hedgerows, dry-stone walls and
//! fences along bounded fields and crofts as straight runs (`battle`), rows of hedge splitting the
//! bigger fields along their furrows, a gate in each now and then (`rows`: the fields' cover, as
//! the guarantee counts it, `cover`), orchard trees in rows along
//! the field's furrows, copses (a small clump of trees on a field out on the belt's rim: `copse`),
//! a windmill's sails.

use super::catalog;
use super::fields::{FieldBound, FieldUse};
use super::geom::*;
use super::village::{Bound, PlotKind};
use super::{Building, Layout, Site, Structure, rect};
use crate::battlemap::{Kind, Object, SQ, SQUARE_FT, ShapeKind, URBAN_FIELD, VectorShape};
use crate::core::rng::{hash3, unit};
use crate::t0::biome::Biome;
use crate::t0::settle::{Settlement, Tier};
use crate::t0::T0;

/// A run's kind (the battlemap's `ShapeKind::Hedge`, `DryWall`, `Fence`; a field's hedge, round it
/// or a row in it (`rows`), is a `Hedge` drawn the cheaper way, long as they run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Hedge,
    HedgeRow,
    DryWall,
    Fence,
    /// A cemetery's gate pier: a dry-stone wall's short, thicker stretch (`cemetery::bounds`).
    Pier,
}

impl RunKind {
    /// Its thickness (ft) as drawn.
    fn ft(self) -> f64 {
        match self {
            RunKind::Hedge | RunKind::HedgeRow => 3.5,
            RunKind::DryWall => 3.75,
            RunKind::Fence => 0.8,
            RunKind::Pier => 6.0,
        }
    }
}

/// What a chunk takes from the layouts round it (world ft), the runs clipped to its square.
#[derive(Default)]
pub struct Battle {
    pub runs: Vec<(P, P, RunKind)>,
    /// Trees (place, kind, scale, look).
    pub trees: Vec<(P, Kind, f32, u8)>,
    /// Windmills' caps and sails (the tower's middle, the way the cap faces (radians), the tower's
    /// radius ft, whether its walls are pale: lime-washed or plastered).
    pub sails: Vec<(P, f64, f64, bool)>,
    /// Water wheels (middle, the way the mill's wall runs, the mill to its left).
    pub wheels: Vec<(P, P)>,
}

/// Runs and trees a chunk holds at most (every layout round it together).
const MAX_RUNS: usize = 900;
const MAX_TREES: usize = 1200;
/// An orchard's rows apart and its trees apart along a row (ft).
const ORCHARD_ROW_FT: f64 = 22.0;
const ORCHARD_TREE_FT: f64 = 20.0;
/// A copse's trees apart (ft, a jittered lattice keyed by world position).
const COPSE_FT: f64 = 9.0;
/// A field's gate (ft wide).
const GATE_FT: f64 = 10.0;
/// How far a gate's gap keeps off its side's corners (ft).
const GATE_CORNER_FT: f64 = 8.0;
/// How far a run stops short of a roof (ft), and the gap it leaves where it meets a lane or a road
/// (ft off its edge).
const ROOF_FT: f64 = 3.0;
const WAY_GAP_FT: f64 = 5.0;
/// A windmill's sails' reach from the hub (ft).
pub const SAIL_FT: f64 = 27.0;
/// Sides of a tower mill's round footprint (as the POI towers').
pub const MILL_SIDES: usize = 14;

/// A copse on field `i`: the clump's middle and radius (ft). Some pasture and fallow out on the
/// belt's rim, keyed by place; never a windmill's field.
pub fn copse(l: &Layout, i: usize) -> Option<(P, f64)> {
    let info = l.field_info.get(i)?;
    if !matches!(info.kind, FieldUse::Pasture | FieldUse::Fallow) {
        return None;
    }
    let f = &l.fields[i];
    let c = centroid(f);
    let far = dist(c, l.center) / l.radius.max(1.0);
    if far < if l.tier == Tier::Village { 1.5 } else { 1.25 } || !contains(f, c) {
        return None;
    }
    if unit(hash3(0xc0b5e, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 1)) >= 0.12 {
        return None;
    }
    let room = outline_dist(f, c) - 10.0;
    let r = (0.3 * obb(f).short).clamp(16.0, 40.0).min(room);
    (r >= 12.0).then_some((c, r))
}

/// A field's rows apart at most (ft): about this, a little less on some fields (keyed by place).
const ROW_FT: f64 = 110.0;
/// A row's gates apart (ft, about: 100 to 150 keyed by the row) and how far a gate's gap reaches
/// either side of its middle (ft: two 4-ft pieces, so a square stays open).
const ROW_GATE_FT: f64 = 100.0;
const ROW_GATE_HALF: f64 = 4.0;

/// Field `i`'s hedgerows (or a walled field's walls) inside it, along its furrows: each row's ends
/// on the field's outline (world ft) and its gates (ft along it from the first end). Rows split the
/// field across its furrows at most `ROW_FT` apart, so no point of it lies more than about 55 ft
/// from a row or its bound (a field with no bound has its outer rows half a step in). Orchards keep
/// their trees' rows instead, and pieces shorter than 30 ft are left out. A pure function of the
/// field, so every chunk agrees.
pub fn rows(l: &Layout, i: usize) -> Vec<(P, P, Vec<f64>)> {
    let (Some(f), Some(info)) = (l.fields.get(i), l.field_info.get(i)) else { return Vec::new() };
    if info.kind == FieldUse::Orchard || !info.rows || f.len() < 3 {
        return Vec::new();
    }
    let a = info.angle as f64;
    let (u, v) = ([libm::cos(a), libm::sin(a)], [-libm::sin(a), libm::cos(a)]);
    let c = centroid(f);
    let key = |k: i64| hash3(0x4ed6e, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, k);
    let (t0, t1) = f.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        let t = dot(sub(*p, c), v);
        (lo.min(t), hi.max(t))
    });
    let width = t1 - t0;
    let step = ROW_FT * (0.85 + 0.15 * unit(key(0)));
    let n = (width / step).ceil().max(1.0) as usize;
    let ts: Vec<f64> = if info.bound != FieldBound::None {
        (1..n).map(|k| t0 + width * k as f64 / n as f64).collect()
    } else if width < 40.0 {
        Vec::new()
    } else {
        (0..n).map(|k| t0 + width * (k as f64 + 0.5) / n as f64).collect()
    };
    let mut out = Vec::new();
    for (ri, &t) in ts.iter().enumerate() {
        // (Where the line crosses the outline, in order along it: inside between each pair.)
        let mut ss: Vec<f64> = Vec::new();
        for k in 0..f.len() {
            let (p, q) = (f[k], f[(k + 1) % f.len()]);
            let (dp, dq) = (dot(sub(p, c), v) - t, dot(sub(q, c), v) - t);
            if (dp < 0.0) != (dq < 0.0) {
                ss.push(dot(sub(lerp(p, q, dp / (dp - dq)), c), u));
            }
        }
        ss.sort_by(f64::total_cmp);
        for (pi, s) in ss.chunks_exact(2).enumerate() {
            let len_ft = s[1] - s[0];
            if len_ft < 30.0 {
                continue;
            }
            let base = add(c, mul(v, t));
            let (pa, pb) = (add(base, mul(u, s[0])), add(base, mul(u, s[1])));
            let h = key(1 + 64 * ri as i64 + pi as i64);
            let every = ROW_GATE_FT * (1.0 + 0.5 * unit(h));
            let g = if len_ft < 60.0 { 0 } else { crate::core::round(len_ft / every).max(1.0) as usize };
            let gates = (0..g)
                .map(|m| {
                    let span = len_ft / g as f64;
                    (span * (m as f64 + 0.5 + 0.3 * (unit(hash3(h, m as i64, 0, 3)) - 0.5))).clamp(10.0, len_ft - 10.0)
                })
                .collect();
            out.push((pa, pb, gates));
        }
    }
    out
}

/// A copse's clump: a middle and three lobes (centre, radius ft).
fn lobes(c: P, r: f64) -> [(P, f64); 4] {
    let a0 = unit(hash3(0xc0b5e, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 2)) * std::f64::consts::TAU;
    let lobe = |k: f64| {
        let a = a0 + k * 2.1;
        (add(c, mul([libm::cos(a), libm::sin(a)], 0.5 * r)), 0.55 * r)
    };
    [(c, 0.75 * r), lobe(0.0), lobe(1.0), lobe(2.0)]
}

fn outline_dist(poly: &[P], p: P) -> f64 {
    (0..poly.len()).map(|k| seg_dist(p, poly[k], poly[(k + 1) % poly.len()])).fold(f64::MAX, f64::min)
}

fn bbox(poly: &[P]) -> [f64; 4] {
    poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
}

fn hits(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

fn grow(b: [f64; 4], m: f64) -> [f64; 4] {
    [b[0] - m, b[1] - m, b[2] + m, b[3] + m]
}

fn is_func(b: &Building, key: &str) -> bool {
    b.func.is_some_and(|f| catalog::CATALOG.get(f as usize).is_some_and(|c| c.key == key))
}

/// Whether `b` is a windmill (`catalog` "windmill").
pub fn windmill(b: &Building) -> bool {
    is_func(b, "windmill")
}

/// The way a windmill's cap faces, its sails in front (radians, world axes): into the prevailing
/// wind, from the south-west, a little either way by its place.
pub fn sail_angle(b: &Building) -> f64 {
    let c = centroid(&b.poly);
    let u = unit(hash3(0x5a11, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 3));
    0.75 * std::f64::consts::PI + 0.9 * (u - 0.5)
}

/// A windmill's middle and its tower's radius (ft): the corners' mean, measured from the first
/// corner (world feet are millions).
pub fn mill_ring(b: &Building) -> (P, f64) {
    let p0 = b.poly[0];
    let n = b.poly.len().max(1) as f64;
    let m = b.poly.iter().fold([0.0, 0.0], |a, &q| add(a, mul(sub(q, p0), 1.0 / n)));
    let r = b.poly.iter().map(|&q| len(sub(sub(q, p0), m))).sum::<f64>() / n;
    (add(p0, m), r)
}

/// Last in making a layout (world ft): water wheels, then windmills.
pub(super) fn place(site: &Site, s: &Settlement, l: &mut Layout) {
    if l.tier == Tier::Village {
        wheels(site, l);
    }
    windmills(site, s, l);
}

/// A village's water mill's wheel: off the middle of the side nearest water (within 24 ft), half in
/// the water at the bank, if no lane or road runs between (V3b: a short timber race to the wall).
fn wheels(site: &Site, l: &mut Layout) {
    let center = l.center;
    let mut out = Vec::new();
    for b in l.buildings.iter().filter(|b| is_func(b, "mill")) {
        let p = &b.poly;
        let n = p.len();
        let mut best: Option<(f64, P, P, P)> = None;
        for k in 0..n {
            let (a, e) = (p[k], p[(k + 1) % n]);
            let el = dist(a, e);
            if el < 8.0 {
                continue;
            }
            let u = mul(sub(e, a), 1.0 / el);
            let mid = lerp(a, e, 0.5);
            let o = if dot([u[1], -u[0]], sub(mid, centroid(p))) > 0.0 { [u[1], -u[0]] } else { [-u[1], u[0]] };
            let reach = (1..=12).map(|s| 2.0 * s as f64).find(|&d| [0.3, 0.5, 0.7].iter().any(|&t| site.wet(sub(add(lerp(a, e, t), mul(o, d)), center))));
            if let Some(d) = reach
                && best.is_none_or(|x| d < x.0)
            {
                best = Some((d, mid, u, o));
            }
        }
        // (Half in the water at the bank, no lane or road between it and the mill.)
        if let Some((d, mid, u, o)) = best {
            // (Its way along the wall turned so the mill lies to its left, `[-u.y, u.x]`: the sprite's +y.)
            let u = if dot([-u[1], u[0]], o) > 0.0 { mul(u, -1.0) } else { u };
            let c = add(mid, mul(o, d + 4.0));
            let clear = |q: P| {
                l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |x| seg_dist(q, x[0], x[1]) - 0.5 * w)).chain(l.streets.iter().flat_map(|st| st.windows(2).map(|x| seg_dist(q, x[0], x[1]) - 0.5 * super::village::LANE_W))).fold(f64::MAX, f64::min) > 1.0
                    && !site.near_network_road(sub(q, center), 1.0)
            };
            let pit = add(mid, mul(o, 3.0));
            if [0.15, 0.5, 0.85, 1.0].iter().all(|&t| clear(lerp(mid, c, t))) {
                out.push((c, u));
            } else if [-0.3, 0.0, 0.3].iter().all(|&t| clear(add(add(pit, mul(o, 1.5)), mul(u, t * 14.0)))) {
                // (Else in its pit against the wall, the leat under the lane.)
                out.push((pit, u));
            }
        }
    }
    l.wheels = out;
}

/// Windmills among the fields, the highest ground against the land round (150 ft out) first: a
/// village's always when it has no water mill, else one time in three; one round most towns, one or
/// two round a city, two or three round a metropolis. A tower mill: a round tower (`MILL_SIDES`,
/// 22-28 ft across) of three or four floors in a field's middle, clear of its hedges, the roads,
/// lanes and buildings, 600 ft apart.
fn windmills(site: &Site, s: &Settlement, l: &mut Layout) {
    let Some(func) = catalog::index_of("windmill") else { return };
    // (Keyed by its place, not its number: a town added elsewhere leaves it as it was.)
    let roll = |k: i64| unit(hash3(s.seed ^ 0x3d11, crate::core::round(s.x) as i64, crate::core::round(s.y) as i64, k));
    let village = l.tier == Tier::Village;
    let want = match l.tier {
        Tier::Village if l.buildings.iter().any(|b| is_func(b, "mill")) => (roll(2) < 1.0 / 3.0) as usize,
        Tier::Village => 1,
        Tier::Town => (roll(1) < 0.8) as usize,
        Tier::City => 1 + (roll(1) < 0.5) as usize,
        Tier::Metropolis => 2 + (roll(1) < 0.5) as usize,
    };
    if want == 0 || l.fields.is_empty() || l.field_info.len() != l.fields.len() {
        return;
    }
    let r = l.radius.max(1.0);
    let center = l.center;
    let mut cands: Vec<(f64, usize)> = Vec::new();
    for (i, f) in l.fields.iter().enumerate() {
        if l.field_info[i].kind != FieldUse::Arable || copse(l, i).is_some() || area(f).abs() < 3000.0 {
            continue;
        }
        let c = centroid(f);
        if !contains(f, c) {
            continue;
        }
        let lc = sub(c, center);
        let ring: f64 = (0..8).map(|k| {
            let a = k as f64 * std::f64::consts::FRAC_PI_4;
            site.height(add(lc, [150.0 * libm::cos(a), 150.0 * libm::sin(a)]))
        }).sum::<f64>() / 8.0;
        // (A rise preferred, not needed: flat land scores by nearness alone.)
        let rise = (site.height(lc) - ring).clamp(-6.0, 12.0);
        let far = len(lc) / r;
        cands.push((rise - 4.0 * (far - if village { 1.0 } else { 1.3 }).max(0.0), i));
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut placed: Vec<P> = Vec::new();
    let mut id = l.buildings.iter().map(|b| b.id + 1).max().unwrap_or(0);
    for &(_, i) in cands.iter().take(200) {
        if placed.len() >= want {
            break;
        }
        let f = &l.fields[i];
        let c = centroid(f);
        let h = hash3(s.seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 0x3d12);
        let rad = 11.0 + 3.0 * unit(h ^ 1);
        if placed.iter().any(|&q| dist(q, c) < 600.0) || outline_dist(f, c) < rad + 12.0 {
            continue;
        }
        let a0 = unit(h) * std::f64::consts::TAU;
        let mut poly: Vec<P> = (0..MILL_SIDES).map(|k| {
            let a = a0 + std::f64::consts::TAU * k as f64 / MILL_SIDES as f64;
            [c[0] + rad * libm::cos(a), c[1] + rad * libm::sin(a)]
        }).collect();
        if area(&poly) < 0.0 {
            poly.reverse();
        }
        let local: Vec<P> = poly.iter().map(|&p| sub(p, center)).collect();
        let lc = sub(c, center);
        let near_way = l.roads.iter().flat_map(|(pts, _, w)| pts.windows(2).map(move |q| seg_dist(c, q[0], q[1]) - 0.5 * w)).chain(l.streets.iter().flat_map(|st| st.windows(2).map(|q| seg_dist(c, q[0], q[1]) - 6.0))).any(|d| d < rad + 14.0);
        if near_way || site.near_network_road(lc, rad + 19.0) || !site.buildable(&local) || l.buildings.iter().any(|b| dist(centroid(&b.poly), c) < rad + 30.0) {
            continue;
        }
        let ward = if village { catalog::Ward::Rural } else { catalog::Ward::Farm };
        let floors = 3 + (unit(h ^ 3) < 0.5) as u8;
        l.buildings.push(Building { poly, ward, func: Some(func as u16), residential: 1, name: None, floors, pad_ft: site.pad(&local), structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id, parts: Vec::new(), keep: None });
        id += 1;
        placed.push(c);
    }
}

/// The part of the segment a-b inside `r` (Liang-Barsky).
fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (o3, o4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    o1 * o2 < 0.0 && o3 * o4 < 0.0
}

fn clip_seg(a: P, b: P, r: [f64; 4]) -> Option<(P, P)> {
    let d = sub(b, a);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-d[0], a[0] - r[0]), (d[0], r[2] - a[0]), (-d[1], a[1] - r[1]), (d[1], r[3] - a[1])] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t1 - t0 > 1e-6).then(|| (lerp(a, b, t0), lerp(a, b, t1)))
}

/// A bounded field's gate: where its outline comes nearest a lane or the layout's road (within 45
/// ft), else a place along its longest side keyed by the field. Its gap stays `GATE_CORNER_FT` off
/// the side's corners (no stub of hedge left between it and a corner); sides too short for that
/// take none but the longest.
fn gate(f: &[P], ways: &[(P, P, f64)]) -> P {
    let n = f.len();
    let fb = grow(bbox(f), 50.0);
    let ways: Vec<(P, P, f64)> = ways.iter().copied().filter(|&(a, b, half)| hits(grow(bbox(&[a, b]), half), fb)).collect();
    let room = 0.5 * GATE_FT + GATE_CORNER_FT;
    let mut best = (45.0, None);
    for k in 0..n {
        let (a, b) = (f[k], f[(k + 1) % n]);
        let e = dist(a, b);
        if e < 2.0 * room {
            continue;
        }
        let m = ((e - 2.0 * room) / 8.0).ceil().max(1.0) as usize;
        for j in 0..=m {
            let p = lerp(a, b, (room + (e - 2.0 * room) * j as f64 / m as f64) / e);
            for &(s, e, half) in &ways {
                let d = seg_dist(p, s, e) - half;
                if d < best.0 {
                    best = (d, Some(p));
                }
            }
        }
    }
    if let Some(p) = best.1 {
        return p;
    }
    let k = (0..n).max_by(|&x, &y| dist(f[x], f[(x + 1) % n]).total_cmp(&dist(f[y], f[(y + 1) % n])).then(y.cmp(&x))).unwrap_or(0);
    let c = centroid(f);
    let e = dist(f[k], f[(k + 1) % n]).max(1e-9);
    let span = (0.4 * e).min((e - 2.0 * room).max(0.0));
    lerp(f[k], f[(k + 1) % n], 0.5 + span / e * (unit(hash3(0x9a7e, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64, 4)) - 0.5))
}

/// What a chunk (`rect`, world ft) takes from layout `l`: runs (pieces of 4 ft or so: not on a road
/// or water, `blocked`, not in a building, not where a lower field already runs one, a gap at each
/// field's gate), its orchards' and copses' trees, sails and wheels (each in the chunk holding its
/// middle).
pub fn battle(l: &Layout, t0: &T0, rect: [f64; 4], blocked: &dyn Fn(P) -> bool, out: &mut Battle) {
    let near = grow(rect, 8.0);
    if !hits(l.bbox, near) && !l.crofts.iter().any(|p| hits(bbox(&p.pts), near)) {
        return;
    }
    let inside = |p: P| p[0] >= rect[0] && p[0] < rect[2] && p[1] >= rect[1] && p[1] < rect[3];
    // (A piece's middle within 2.5 ft of the chunk: `blocked` reads the squares round it.)
    let mids = grow(rect, 2.5);
    // The ways near the fields here: every field's gate is found from all of its own.
    let bounded: Vec<(usize, [f64; 4])> = l
        .fields
        .iter()
        .enumerate()
        .filter(|&(i, _)| l.field_info.get(i).is_some_and(|x| x.bound != FieldBound::None))
        .map(|(i, f)| (i, bbox(f)))
        .filter(|&(_, b)| hits(b, near))
        .collect();
    let wide = bounded.iter().fold(grow(rect, 30.0), |a, &(_, b)| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]);
    let wide = grow(wide, 80.0);
    let mut ways: Vec<(P, P, f64)> = l
        .roads
        .iter()
        .flat_map(|(pts, _, w)| pts.windows(2).map(move |q| (q[0], q[1], 0.5 * w)))
        .chain(l.streets.iter().flat_map(|st| st.windows(2).map(|q| (q[0], q[1], 0.5 * super::village::LANE_W))))
        .filter(|&(a, b, _)| hits(bbox(&[a, b]), wide))
        .collect();
    // (And the world's roads by it, as the village took them: `village`.)
    for (ri, k) in t0.roads.segments_near(wide, 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 40.0).ceil().clamp(1.0, 400.0) as usize;
        let pts: Vec<P> = (0..=n).map(|j| rc.eval(k as usize, j as f64 / n as f64, 2.5, t0.cell_ft).p).collect();
        ways.extend(pts.windows(2).map(|q| (q[0], q[1], 0.5 * rc.class.width_ft())).filter(|&(a, b, _)| hits(bbox(&[a, b]), wide)));
    }
    // (Its roofs: a run stops `ROOF_FT` short of one.)
    let blds: Vec<&Vec<P>> = l.buildings.iter().filter(|b| b.structure == Structure::Roofed).map(|b| &b.poly).filter(|p| hits(grow(bbox(p), ROOF_FT + 4.0), near)).collect();
    let by_roof = |a: P, b: P| {
        let piece = [a, b];
        blds.iter().any(|q| contains(q, a) || contains(q, b) || poly_gap(q, &piece) < ROOF_FT)
    };
    // (Off a way by 2.5 ft along it, and a gate's gap where it meets one: `WAY_GAP_FT` off its edge
    // where it runs at it.)
    let on_way = |a: P, b: P| {
        let (u, ul) = (sub(b, a), dist(a, b).max(1e-9));
        ways.iter().any(|&(s, e, half)| {
            let d = [seg_dist(a, s, e), seg_dist(b, s, e), seg_dist(s, a, b), seg_dist(e, a, b)].into_iter().fold(f64::MAX, f64::min);
            if d < half + 2.5 || segs_cross(a, b, s, e) {
                return true;
            }
            let (v, vl) = (sub(e, s), dist(s, e).max(1e-9));
            d < half + WAY_GAP_FT && (dot(u, v) / (ul * vl)).abs() < 0.7
        })
    };
    // (Nothing runs over a village's green.)
    let greens: Vec<&Vec<P>> = l.plazas.iter().filter(|p| p.kind == super::PlazaKind::Green && hits(grow(bbox(&p.pts), 4.0), near)).map(|p| &p.pts).collect();
    let piece_runs = |a: P, b: P, kind: RunKind, keep: &dyn Fn(P) -> bool, out: &mut Battle| {
        if !hits(bbox(&[a, b]), near) {
            return;
        }
        let m = (dist(a, b) / 4.0).ceil().max(1.0) as usize;
        let mut from: Option<usize> = None;
        for j in 0..=m {
            let mid = lerp(a, b, (j as f64 + 0.5) / m as f64);
            let ok = j < m && hits([mid[0], mid[1], mid[0], mid[1]], mids) && keep(mid) && !blocked(mid) && !greens.iter().any(|g| contains(g, mid)) && {
                let (p, q) = (lerp(a, b, j as f64 / m as f64), lerp(a, b, (j + 1) as f64 / m as f64));
                !on_way(p, q) && !by_roof(p, q)
            };
            match (ok, from) {
                (true, None) => from = Some(j),
                (false, Some(f)) => {
                    if let Some((p, q)) = clip_seg(lerp(a, b, f as f64 / m as f64), lerp(a, b, j as f64 / m as f64), rect)
                        && out.runs.len() < MAX_RUNS
                    {
                        out.runs.push((p, q, kind));
                    }
                    from = None;
                }
                _ => {}
            }
        }
    };
    // Fields: hedged or walled, each shared side once (the lower field's), a gate each.
    for (bi, &(i, _)) in bounded.iter().enumerate() {
        let f = &l.fields[i];
        let kind = if l.field_info[i].bound == FieldBound::Wall { RunKind::DryWall } else { RunKind::HedgeRow };
        let g = gate(f, &ways);
        let keep = |p: P| {
            dist(p, g) > 0.5 * GATE_FT
                && !bounded[..bi].iter().any(|&(j, b)| {
                    p[0] > b[0] - 3.0 && p[0] < b[2] + 3.0 && p[1] > b[1] - 3.0 && p[1] < b[3] + 3.0 && {
                        let o = &l.fields[j];
                        (0..o.len()).any(|k| seg_dist(p, o[k], o[(k + 1) % o.len()]) < 3.0)
                    }
                })
        };
        for k in 0..f.len() {
            piece_runs(f[k], f[(k + 1) % f.len()], kind, &keep, out);
        }
    }
    // Fields' rows inside them along their furrows (`rows`), a gap at each of their gates; none
    // through a copse or under a windmill's sails. A walled field's are walls, an unbounded field's
    // hedges but on cold or dry ground (walls).
    let stony = matches!(
        crate::battlemap::land_biome(t0, l.center),
        Biome::Alpine | Biome::Tundra | Biome::ColdDesert | Biome::Ice | Biome::Steppe | Biome::HotDesert | Biome::SaltFlat | Biome::Savanna
    );
    // (The sails turn in front of the tower, `sail_front`: their tips reach ~31 ft from its middle.)
    let mills: Vec<P> = l.buildings.iter().filter(|b| windmill(b)).map(|b| centroid(&b.poly)).collect();
    for (i, f) in l.fields.iter().enumerate() {
        if !hits(bbox(f), near) {
            continue;
        }
        let kind = match l.field_info.get(i).map(|x| x.bound) {
            Some(FieldBound::Wall) => RunKind::DryWall,
            Some(FieldBound::None) if stony => RunKind::DryWall,
            _ => RunKind::HedgeRow,
        };
        let clump = copse(l, i);
        for (a, b, gates) in rows(l, i) {
            let keep = |p: P| {
                let s = dist(a, p);
                gates.iter().all(|&g| (s - g).abs() > ROW_GATE_HALF) && clump.is_none_or(|(c, r)| dist(p, c) > 1.15 * r + 6.0) && mills.iter().all(|&m| dist(p, m) > SAIL_FT + 8.0)
            };
            piece_runs(a, b, kind, &keep, out);
        }
    }
    // A village's crofts' and yards' hedges and fences, and its churchyard's low wall (the manor's
    // yard wall is raised as a garden wall's).
    if l.crofts.iter().any(|p| hits(bbox(&p.pts), near)) {
        for (a, b, bound, plot) in super::village::bounds(l) {
            let kind = match bound {
                Bound::Hedge => RunKind::Hedge,
                Bound::Fence => RunKind::Fence,
                Bound::Wall if plot == PlotKind::Churchyard => RunKind::DryWall,
                Bound::Wall => continue,
            };
            piece_runs(a, b, kind, &|_| true, out);
        }
    }
    // R5 towns: a cemetery's low wall or hedge, a gap at its gate (`cemetery::bounds`).
    if l.parks.iter().any(|p| p.kind == super::ParkKind::Cemetery && hits(bbox(&p.pts), near)) {
        for (a, b, bound) in super::cemetery::bounds(l) {
            let kind = match bound {
                super::cemetery::Bound::Hedge => RunKind::Hedge,
                super::cemetery::Bound::Pier => RunKind::Pier,
                _ => RunKind::DryWall,
            };
            piece_runs(a, b, kind, &|_| true, out);
        }
    }
    // R5 towns (114): a horse park's corrals, post and rail, a gap at each gate (`horses::bounds`).
    if l.parks.iter().any(|p| p.kind == super::ParkKind::Horses && hits(bbox(&p.pts), near)) {
        for (a, b) in super::horses::bounds(l, near) {
            piece_runs(a, b, RunKind::Fence, &|_| true, out);
        }
    }
    // R5 towns (115): the small pens beside some yurts, post and rail, a gap at each gate.
    if l.yurts.iter().any(|y| y.pen.is_some()) && hits(l.bbox, near) {
        for (a, b) in super::yurts::fences(l, near) {
            piece_runs(a, b, RunKind::Fence, &|_| true, out);
        }
    }
    // Orchards in rows along their furrows; copses.
    if hits(l.bbox, near) {
        let (trees, _, _, _) = crate::battlemap::greenery(crate::battlemap::land_biome(t0, l.center));
        for (i, f) in l.fields.iter().enumerate() {
            let fb = bbox(f);
            if !hits(fb, rect) {
                continue;
            }
            let Some(info) = l.field_info.get(i) else { continue };
            if info.kind == FieldUse::Orchard {
                orchard(f, info.angle as f64, rect, out);
            } else if let Some((c, r)) = copse(l, i) {
                let lb = lobes(c, r);
                let (x0, y0) = ((rect[0].max(c[0] - r * 1.1) / COPSE_FT).floor() as i64, (rect[1].max(c[1] - r * 1.1) / COPSE_FT).floor() as i64);
                let (x1, y1) = ((rect[2].min(c[0] + r * 1.1) / COPSE_FT).ceil() as i64, (rect[3].min(c[1] + r * 1.1) / COPSE_FT).ceil() as i64);
                for gy in y0..=y1 {
                    for gx in x0..=x1 {
                        let h = hash3(0xc0b5e, gx, gy, 5);
                        let p = [(gx as f64 + 0.15 + 0.7 * unit(h)) * COPSE_FT, (gy as f64 + 0.15 + 0.7 * unit(h ^ 7)) * COPSE_FT];
                        let edge = lb.iter().map(|&(q, rr)| rr - dist(p, q)).fold(f64::MIN, f64::max);
                        if edge < 0.0 || !inside(p) || !contains(f, p) || outline_dist(f, p) < 8.0 || out.trees.len() >= MAX_TREES {
                            continue;
                        }
                        let roll = unit(h ^ 11);
                        // (Bushes round its edge, a tree now and then left out.)
                        let kind = if edge < 5.0 && roll < 0.45 { Kind::Bush } else if roll > 0.93 { continue } else { trees[(h >> 20) as usize % trees.len()] };
                        out.trees.push((p, kind, (0.75 + 0.3 * unit(h ^ 13)) as f32, (h >> 32) as u8));
                    }
                }
            }
        }
        for b in l.buildings.iter().filter(|b| windmill(b) && b.poly.len() >= 3) {
            let (c, r) = mill_ring(b);
            if inside(c) {
                let pale = matches!(super::roof::look(l, b).material, super::roof::Material::Whitewash | super::roof::Material::FlatClay);
                out.sails.push((c, sail_angle(b), r, pale));
            }
        }
        for &(c, u) in &l.wheels {
            if inside(c) {
                out.wheels.push((c, u));
            }
        }
    }
}

/// An orchard's trees in `rect`: rows along `angle` from the field's middle, 6 ft in from its edge.
fn orchard(f: &[P], angle: f64, rect: [f64; 4], out: &mut Battle) {
    let (u, v) = ([libm::cos(angle), libm::sin(angle)], [-libm::sin(angle), libm::cos(angle)]);
    let c = centroid(f);
    let o = [crate::core::round(c[0]), crate::core::round(c[1])];
    let fb = bbox(f);
    let b = [rect[0].max(fb[0]), rect[1].max(fb[1]), rect[2].min(fb[2]), rect[3].min(fb[3])];
    let corners = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]];
    let (mut s0, mut s1, mut t0, mut t1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in corners {
        let d = sub(p, o);
        let (s, t) = (dot(d, u), dot(d, v));
        (s0, s1, t0, t1) = (s0.min(s), s1.max(s), t0.min(t), t1.max(t));
    }
    for j in (t0 / ORCHARD_ROW_FT).floor() as i64..=(t1 / ORCHARD_ROW_FT).ceil() as i64 {
        for i in (s0 / ORCHARD_TREE_FT).floor() as i64..=(s1 / ORCHARD_TREE_FT).ceil() as i64 {
            let p = add(o, add(mul(u, i as f64 * ORCHARD_TREE_FT), mul(v, j as f64 * ORCHARD_ROW_FT)));
            if !(p[0] >= rect[0] && p[0] < rect[2] && p[1] >= rect[1] && p[1] < rect[3]) || !contains(f, p) || outline_dist(f, p) < 6.0 || out.trees.len() >= MAX_TREES {
                continue;
            }
            let h = hash3(0x0c4a2d, crate::core::round(p[0] * 4.0) as i64, crate::core::round(p[1] * 4.0) as i64, 6);
            if unit(h) < 0.04 {
                continue;
            }
            out.trees.push((p, Kind::FruitTree, (0.85 + 0.2 * unit(h ^ 3)) as f32, (h >> 32) as u8));
        }
    }
}

/// The chunk's shapes from `b` (chunk origin `ox`, `oy` ft): each run, each windmill's sails.
pub fn shapes(b: &Battle, ox: f64, oy: f64, out: &mut Vec<VectorShape>) {
    let local = |p: P| [((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32];
    for &(a, e, kind) in &b.runs {
        let sk = match kind {
            RunKind::Hedge | RunKind::HedgeRow => ShapeKind::Hedge,
            RunKind::DryWall | RunKind::Pier => ShapeKind::DryWall,
            RunKind::Fence => ShapeKind::Fence,
        };
        // (A field's hedge's third point (1, 0): drawn the cheaper way, `countryside.ts`.)
        let mut pts = vec![local(a), local(e)];
        if kind == RunKind::HedgeRow {
            pts.push([1.0, 0.0]);
        }
        out.push(VectorShape { kind: sk, size: (kind.ft() / SQUARE_FT) as f32, pts });
    }
    for &(c, a, r, pale) in &b.sails {
        out.push(VectorShape { kind: ShapeKind::Sails, size: (SAIL_FT / SQUARE_FT) as f32, pts: vec![local(c), [a as f32, (r / SQUARE_FT) as f32], [pale as u8 as f32, 0.0]] });
    }
}

/// The chunk's trees and wheels from `b`: a tree on open field ground (not on a building, a road
/// or water), a wheel wherever it is (on the water by the mill).
pub fn objects(b: &Battle, ox: f64, oy: f64, c: &mut crate::battlemap::Chunk) {
    let at = |p: P| (((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32);
    for &(p, kind, scale, variant) in &b.trees {
        let (x, y) = at(p);
        let k = (y.max(0.0) as usize).min(SQ - 1) * SQ + (x.max(0.0) as usize).min(SQ - 1);
        let open = c.urban[k] == URBAN_FIELD && c.building[k] == 0 && c.water_level[k] <= c.height[k] && c.road_halo[(k / SQ + 1) * (SQ + 2) + k % SQ + 1] == 0;
        if open {
            c.objects.push(Object { kind, variant, x, y, rot: 0.0, scale, sprite: 0, turned: false });
        }
    }
    for &(p, u) in &b.wheels {
        let (x, y) = at(p);
        let k = (y.max(0.0) as usize).min(SQ - 1) * SQ + (x.max(0.0) as usize).min(SQ - 1);
        if c.building[k] == 0 {
            c.objects.push(Object { kind: Kind::WaterWheel, variant: 0, x, y, rot: libm::atan2(u[1], u[0]) as f32, scale: 1.3, sprite: 0, turned: true });
        }
    }
}

/// Each square of the chunk (origin `ox`, `oy` ft) a run crosses, with the run's kind.
fn each_square(b: &Battle, ox: f64, oy: f64, f: &mut dyn FnMut(usize, RunKind)) {
    for &(a, e, kind) in &b.runs {
        let m = (dist(a, e) / (0.25 * SQUARE_FT)).ceil().max(1.0) as usize;
        for j in 0..=m {
            let p = lerp(a, e, j as f64 / m as f64);
            let (i, k) = (((p[0] - ox) / SQUARE_FT).floor() as i64, ((p[1] - oy) / SQUARE_FT).floor() as i64);
            if i >= 0 && k >= 0 && (i as usize) < SQ && (k as usize) < SQ {
                f(k as usize * SQ + i as usize, kind);
            }
        }
    }
}

/// Play: a run's squares are difficult ground (bit 16 of `Chunk::edges`), never sight's walls.
pub fn mark(b: &Battle, ox: f64, oy: f64, edges: &mut [u8]) {
    each_square(b, ox, oy, &mut |k, _| edges[k] |= 16);
}

/// The guarantee: a hedgerow's or dry-stone wall's squares are cover (`Chunk::yard` bit 8; a
/// fence's are not).
pub fn cover(b: &Battle, ox: f64, oy: f64, yard: &mut [u8]) {
    each_square(b, ox, oy, &mut |k, kind| {
        if kind != RunKind::Fence {
            yard[k] |= 8;
        }
    });
}

/// Plan zoom polygons under the buildings (`payload::pack_sites`): each copse's clump as a park's
/// trees' crowns (site kind 13).
pub fn plan_polys(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    for i in 0..l.fields.len() {
        if let Some((c, r)) = copse(l, i) {
            for (q, rr) in lobes(c, r) {
                push(&circle(q, rr, 12), 13);
            }
        }
    }
}

/// How far in front of a windmill's middle its sails turn, by its tower's radius (ft).
pub fn sail_front(r: f64) -> f64 {
    0.95 * r + 2.0
}

/// Plan zoom marks over the buildings: a windmill's sails, edge-on from above, as a bar across the
/// front of its cap (site kind 23, `PLAN_SAILS`).
pub const PLAN_SAILS: u32 = 23;
pub fn plan_ticks(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    for b in l.buildings.iter().filter(|b| windmill(b) && b.poly.len() >= 3) {
        let (c, r) = mill_ring(b);
        let a = sail_angle(b);
        let d = [libm::cos(a), libm::sin(a)];
        push(&rect(add(c, mul(d, sail_front(r))), [-d[1], d[0]], 2.0 * SAIL_FT, 4.0), PLAN_SAILS);
    }
}
