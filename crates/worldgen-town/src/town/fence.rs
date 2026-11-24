//! (Roadmap 6 R6) Fences and hedges: a town's generated runs changed by hand (`TownEdit::fences`),
//! lines drawn by hand (`Edits::fences`) and gaps cut by the eraser (`Edits::fence_gaps`), and the
//! ids they go by:
//! - `fr:<layout>:<8 hex>` a run: every straight `(a, b)` a source feeds `rural::battle` (a field's
//!   side `f`, a field's row `r`, a croft's or yard's bound or the churchyard's wall `c`, a
//!   cemetery's `m`, a horse park's corral `h`, a yurt's pen `y`, a walled manor's yard wall `w`),
//!   keyed by its source and its two ends (to 1 ft, either way round): a shared field side is one
//!   run. Its canonical start is the smaller rounded end (gates are ft from it). The same layout
//!   gives the same ids; a ward edit moving its ends gives it another (the change lapses).
//! - `fh:<layout>:<spot>:<j>` a park's hedge or row of topiary: `<spot>` the park's (`cut::park_spot`),
//!   `<j>` its place in `park::hedge_runs` of its battlemap layout.
//!
//! Drawn, never laid: only the battlemap and the plan's marks of corrals, pens and the churchyard's
//! wall read them; the layout, fields, plots, gates, haystacks and horses stay as generated. Out of
//! whole-run ids: a palace forecourt's walls, a castle kitchen's wattle, lone topiary and a temple
//! close's bushes (the eraser cuts the walls and the wattle; Objects › Clear takes any of them).

use super::*;
use super::rural::{Battle, RunKind};
use crate::core::hash::{FastMap, FastSet};
use crate::battle::{Kind, KindInfo, Object, SQUARE_FT};
use crate::model::fences::{Fence, FenceChange, FenceGap, FenceKind, GATE_OPEN_FT};
use crate::world::{Edits, TownEdit};

/// The kinds laid in 5-ft sections and a gate's open leaf (the R6 lane's ids, 150-152 and 155),
/// with their rules: the picket and the railing hard going, half cover, seen over; a stockade blocks
/// the way and sight. (A low stone wall is drawn as a dry-stone wall, a high wall raised as a
/// garden wall: `drawn_kind`.)
pub(crate) const KINDS: [(Kind, KindInfo); 4] = [
    (Kind::Picket, crate::battle::k(Kind::Picket, "picket fence", 0.5, false, false, 1, true, 3.5, true)),
    (Kind::IronRailing, crate::battle::k(Kind::IronRailing, "iron railing", 0.5, false, false, 1, true, 4.0, true)),
    (Kind::Stockade, crate::battle::k(Kind::Stockade, "stockade", 0.55, true, true, 3, false, 10.0, true)),
    (Kind::Gate, crate::battle::k(Kind::Gate, "gate", 0.5, false, false, 0, false, 4.0, false)),
];

/// Sources' tags (the letter in a run's key).
pub const FIELD: u8 = b'f';
pub const ROW: u8 = b'r';
pub const CROFT: u8 = b'c';
pub const CEMETERY: u8 = b'm';
pub const LOT: u8 = b'l';
pub const CORRAL: u8 = b'h';
pub const PEN: u8 = b'y';
pub const GARDEN: u8 = b'w';

/// A piece of a kind laid in sections (ft along its line).
const SECTION_FT: f64 = 5.0;
/// A drawn fence's pieces as water cuts it (ft).
const DRAWN_PIECE_FT: f64 = 4.0;
/// What is left of a run between cuts shorter than this (ft) goes.
const SLIVER_FT: f64 = 0.5;
/// What a gate's opening is wider than the square it frees (ft, a side).
const GATE_MARGIN_FT: f64 = 0.5;
/// The most runs hand-made fences (a changed run, a drawn line) add to a chunk's generated ones.
const MAX_HAND_RUNS: usize = 600;

#[inline]
fn q(p: P) -> (i64, i64) {
    (crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64)
}

/// A run's ends from its canonical start (the smaller rounded end) on.
#[inline]
pub fn canonical(a: P, b: P) -> (P, P) {
    if q(a) <= q(b) { (a, b) } else { (b, a) }
}

/// A run's key (`<8 hex>` of its `fr:` id) from its source's tag and its ends.
pub fn run_key(tag: u8, a: P, b: P) -> u32 {
    let (a, b) = (q(a), q(b));
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    (crate::core::rng::hash2(crate::core::rng::hash2(0xFE_0C00 + tag as u64, a.0, a.1), b.0, b.1) & 0xffff_ffff) as u32
}

/// A run's id (`fr:<layout>:<8 hex>`).
#[inline]
pub fn fr_id(li: usize, key: u32) -> String {
    format!("fr:{li}:{key:08x}")
}

/// A park's hedge's or topiary row's id (`fh:<layout>:<spot>:<j>`).
#[inline]
pub fn fh_id(li: usize, spot: &str, j: usize) -> String {
    format!("fh:{li}:{spot}:{j}")
}

/// An `fr:` or `fh:` id as (layout, its key in `TownEdit::fences`).
pub fn parse(id: &str) -> Option<(usize, String)> {
    let (head, rest) = id.split_once(':')?;
    let (li, local) = rest.split_once(':')?;
    let li: usize = li.parse().ok()?;
    let ok = match head {
        "fr" => local.len() == 8 && u32::from_str_radix(local, 16).is_ok() && local.bytes().all(|b| !b.is_ascii_uppercase()),
        "fh" => local.split_once(':').is_some_and(|(spot, j)| spot.len() >= 2 && j.parse::<usize>().is_ok()),
        _ => false,
    };
    ok.then(|| (li, local.to_string()))
}

/// The fence changes of layout `li` that apply (`TownEdit::fences`): a settlement's not laid out
/// by hand, still where its edit says it was (as `cut::removed_of`).
pub fn changes_of<'a>(world: &'a World, t0: &T0, li: usize) -> Option<&'a std::collections::BTreeMap<String, FenceChange>> {
    if world.file.edits.towns.is_empty() {
        return None;
    }
    let e = world.file.edits.towns.get(&wards::key(li)).filter(|e| !e.fences.is_empty())?;
    let s = settlement(t0, li)?;
    if hand::is_hand(world, li) || dist(e.at, [s.x, s.y]) > TownEdit::SAME_AT_FT {
        return None;
    }
    Some(&e.fences)
}

/// What of a layout's fences and hedges is changed by hand (`FenceCut::of`).
#[derive(Debug, Default)]
pub struct FenceCut {
    runs: FastMap<u32, FenceChange>,
    parks: FastMap<u32, Vec<(u32, FenceChange)>>,
}

impl FenceCut {
    /// What of `l` is changed (`TownEdit::fences` as it applies: `changes_of`), or None when
    /// nothing is (drawn as generated).
    pub fn of(world: &World, t0: &T0, l: &Layout) -> Option<FenceCut> {
        let changes = changes_of(world, t0, l.index as usize)?;
        let mut cut = FenceCut::default();
        let mut parks: Option<FastMap<String, u32>> = None;
        for (key, ch) in changes.iter().filter(|(_, c)| !c.is_noop()) {
            if let Some((spot, j)) = key.split_once(':') {
                let parks = parks.get_or_insert_with(|| (0..l.parks.len()).map(|k| (cut::park_spot(l, k), k as u32)).collect());
                if let (Some(&pk), Ok(j)) = (parks.get(spot), j.parse::<u32>()) {
                    cut.parks.entry(pk).or_default().push((j, ch.clone()));
                }
            } else if let Ok(n) = u32::from_str_radix(key, 16) {
                cut.runs.insert(n, ch.clone());
            }
        }
        Some(cut)
    }

    /// The change of the run from `a` to `b` of source `tag`, if any.
    #[inline]
    pub fn run(&self, tag: u8, a: P, b: P) -> Option<&FenceChange> {
        if self.runs.is_empty() { None } else { self.runs.get(&run_key(tag, a, b)) }
    }

    /// The changes of park `pk`'s hedges and topiary rows (`l.parks[pk]`): (j, change).
    #[inline]
    pub fn park(&self, pk: usize) -> Option<&[(u32, FenceChange)]> {
        self.parks.get(&(pk as u32)).map(Vec::as_slice)
    }

    /// The middles of `l`'s gone runs that the plan marks (corrals, pens, the churchyard's wall).
    pub fn plan_gone(&self, l: &Layout, t0: &T0) -> Vec<P> {
        if !self.runs.values().any(|c| c.gone) {
            return Vec::new();
        }
        sources(l, t0, EVERYWHERE).into_iter().filter(|s| matches!(s.0, CORRAL | PEN | CROFT) && self.run(s.0, s.1, s.2).is_some_and(|c| c.gone)).map(|s| lerp(s.1, s.2, 0.5)).collect()
    }
}

/// Every place (`sources`' `near` reaching the whole layout).
pub const EVERYWHERE: [f64; 4] = [f64::MIN / 4.0, f64::MIN / 4.0, f64::MAX / 4.0, f64::MAX / 4.0];

/// Whether a plan mark (`payload::pack_sites`) is a gone run's: a corral's or pen's fence
/// (`horses::PLAN_FENCE`) or the churchyard's wall (`village::PLAN_WALL`) with its middle at one
/// of `gone` (`FenceCut::plan_gone`).
pub fn plan_mark_gone(gone: &[P], poly: &[P], code: u32) -> bool {
    (code == horses::PLAN_FENCE || code == village::PLAN_WALL) && !poly.is_empty() && {
        let c = centroid(poly);
        gone.iter().any(|&g| dist(g, c) < 0.5)
    }
}

/// Every generated run of `l` reaching `near` (world ft) as `rural::battle` feeds them, each once:
/// (tag, a, b, its kind). For pick and the agent's lists.
pub fn sources(l: &Layout, t0: &T0, near: [f64; 4]) -> Vec<(u8, P, P, FenceKind)> {
    let hits = |b: [f64; 4]| b[0] <= near[2] && b[2] >= near[0] && b[1] <= near[3] && b[3] >= near[1];
    let mut out: Vec<(u8, P, P, FenceKind)> = Vec::new();
    let mut seen: FastSet<u32> = FastSet::default();
    let mut add = |tag: u8, a: P, b: P, kind: FenceKind, out: &mut Vec<(u8, P, P, FenceKind)>| {
        if hits(bbox(&[a, b])) && seen.insert(run_key(tag, a, b)) {
            out.push((tag, a, b, kind));
        }
    };
    let stony = matches!(
        crate::battle::land_biome(t0, l.center),
        crate::t0::biome::Biome::Alpine
            | crate::t0::biome::Biome::Tundra
            | crate::t0::biome::Biome::ColdDesert
            | crate::t0::biome::Biome::Ice
            | crate::t0::biome::Biome::Steppe
            | crate::t0::biome::Biome::HotDesert
            | crate::t0::biome::Biome::SaltFlat
            | crate::t0::biome::Biome::Savanna
    );
    for (i, f) in l.fields.iter().enumerate() {
        let bound = l.field_info.get(i).map(|x| x.bound);
        if !hits(bbox(f)) {
            continue;
        }
        if bound.is_some_and(|b| b != fields::FieldBound::None) {
            let kind = if bound == Some(fields::FieldBound::Wall) { FenceKind::DryStoneWall } else { FenceKind::Hedgerow };
            for k in 0..f.len() {
                add(FIELD, f[k], f[(k + 1) % f.len()], kind, &mut out);
            }
        }
        let kind = match bound {
            Some(fields::FieldBound::Wall) => FenceKind::DryStoneWall,
            Some(fields::FieldBound::None) if stony => FenceKind::DryStoneWall,
            _ => FenceKind::Hedgerow,
        };
        for (a, b, _) in rural::rows(l, i) {
            add(ROW, a, b, kind, &mut out);
        }
    }
    for (a, b, bound, plot) in village::bounds(l) {
        let kind = match bound {
            village::Bound::Hedge => FenceKind::Hedgerow,
            village::Bound::Fence => FenceKind::PostAndRail,
            village::Bound::Wall if plot == village::PlotKind::Churchyard => FenceKind::DryStoneWall,
            village::Bound::Wall => continue,
        };
        add(CROFT, a, b, kind, &mut out);
    }
    if l.parks.iter().any(|p| p.kind == ParkKind::Cemetery) {
        for (a, b, bound) in cemetery::bounds(l) {
            let kind = if matches!(bound, cemetery::Bound::Hedge) { FenceKind::Hedgerow } else { FenceKind::DryStoneWall };
            add(CEMETERY, a, b, kind, &mut out);
        }
    }
    for (a, b, bound) in cemetery::lot_bounds(l, near) {
        let kind = if matches!(bound, cemetery::Bound::Hedge) { FenceKind::Hedgerow } else { FenceKind::DryStoneWall };
        add(LOT, a, b, kind, &mut out);
    }
    for (a, b) in horses::bounds(l, near) {
        add(CORRAL, a, b, FenceKind::PostAndRail, &mut out);
    }
    for (a, b) in yurts::fences(l, near) {
        add(PEN, a, b, FenceKind::PostAndRail, &mut out);
    }
    for (a, b) in village::wall_runs(l) {
        add(GARDEN, a, b, FenceKind::GardenWall, &mut out);
    }
    out
}

/// How the battlemap draws a fence of `kind` drawn by hand (or a park's run changed to it).
pub fn drawn_kind(kind: FenceKind) -> RunKind {
    match kind {
        FenceKind::Hedgerow => RunKind::Hedge,
        FenceKind::PostAndRail => RunKind::Fence,
        FenceKind::Hedge => RunKind::Pieces(Kind::Hedge),
        FenceKind::Wattle => RunKind::Pieces(Kind::WattleFence),
        // (A low stone wall's rules are a dry-stone wall's: drawn as one.)
        FenceKind::DryStoneWall | FenceKind::LowWall => RunKind::DryWall,
        FenceKind::Picket => RunKind::Pieces(Kind::Picket),
        FenceKind::IronRailing => RunKind::Pieces(Kind::IronRailing),
        FenceKind::Stockade => RunKind::Pieces(Kind::Stockade),
        // (A garden wall is generated only: one asked for elsewhere stands as a high wall, raised
        // and drawn as a garden wall.)
        FenceKind::GardenWall | FenceKind::HighWall => RunKind::Wall,
    }
}

/// How a generated run drawn as `was` is drawn once it is changed to `kind` (a field's hedge keeps
/// its cheaper drawing, a corral its bolder fence, a pier its thickness).
pub fn run_kind(kind: FenceKind, was: RunKind) -> RunKind {
    match (kind, was) {
        (FenceKind::Hedgerow, RunKind::HedgeRow) => RunKind::HedgeRow,
        (FenceKind::PostAndRail, RunKind::Corral) => RunKind::Corral,
        (FenceKind::DryStoneWall, RunKind::Pier) => RunKind::Pier,
        _ => drawn_kind(kind),
    }
}

/// A gate's leaf's look by the run it stands in: 0 timber, 1 iron, 2 plank.
#[inline]
pub fn gate_look(kind: RunKind) -> u8 {
    match kind {
        RunKind::Pieces(Kind::IronRailing) | RunKind::Wall => 1,
        RunKind::Pieces(Kind::Stockade) => 2,
        _ => 0,
    }
}

/// The part of the line `a + u t` (t from 0 to `l`, `u` a unit vector) within `r` of the segment
/// from `s` to `e`, as (t0, t1), if any.
fn capsule_span(a: P, u: P, l: f64, s: P, e: P, r: f64) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    let mut take = |span: Option<(f64, f64)>| {
        if let Some((x, y)) = span.filter(|(x, y)| y > x) {
            lo = lo.min(x);
            hi = hi.max(y);
        }
    };
    let disc = |c: P| {
        let d = sub(a, c);
        let (bb, cc) = (dot(u, d), dot(d, d) - r * r);
        let disc = bb * bb - cc;
        (disc > 0.0).then(|| {
            let h = crate::core::sqrt(disc);
            (-bb - h, -bb + h)
        })
    };
    take(disc(s));
    take(disc(e));
    let sl = dist(s, e);
    if sl > 1e-9 {
        let w = mul(sub(e, s), 1.0 / sl);
        let n = [-w[1], w[0]];
        // (Within the slab 0 ≤ (p - s)·w ≤ sl and |(p - s)·n| ≤ r.)
        let (mut t0, mut t1) = (f64::MIN, f64::MAX);
        for (axis, from, to) in [(w, 0.0, sl), (n, -r, r)] {
            let (p0, dp) = (dot(sub(a, s), axis), dot(u, axis));
            if crate::core::fabs(dp) < 1e-12 {
                if p0 < from || p0 > to {
                    t1 = f64::MIN;
                }
            } else {
                let (x, y) = ((from - p0) / dp, (to - p0) / dp);
                t0 = t0.max(x.min(y));
                t1 = t1.min(x.max(y));
            }
        }
        take(Some((t0, t1)));
    }
    let (lo, hi) = (lo.max(0.0), hi.min(l));
    (hi > lo).then_some((lo, hi))
}

/// The spans (ft from `a`) of the segment from `a` to `b` that gaps' strokes cut.
fn gap_spans(a: P, b: P, gaps: &[FenceGap], out: &mut Vec<(f64, f64)>) {
    let l = dist(a, b);
    if l < 1e-9 {
        return;
    }
    let u = mul(sub(b, a), 1.0 / l);
    let (x0, y0, x1, y1) = (a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1]));
    for g in gaps {
        let r = g.r.max(0.0) as f64;
        let strokes: Vec<(P, P)> = if g.pts.len() == 1 { vec![(g.pts[0], g.pts[0])] } else { g.pts.windows(2).map(|w| (w[0], w[1])).collect() };
        for (s, e) in strokes {
            if s[0].min(e[0]) - r > x1 || s[0].max(e[0]) + r < x0 || s[1].min(e[1]) - r > y1 || s[1].max(e[1]) + r < y0 {
                continue;
            }
            if let Some(span) = capsule_span(a, u, l, s, e, r) {
                out.push(span);
            }
        }
    }
}

/// Where the opening of a gate asked for `g` ft from `a` on the run to `b` lies: (its middle ft from
/// `a`, half its width). A gate is open passage: the opening frees the whole battlemap square (the
/// lattice's, 5 ft) the gate falls in, so the middle moves to that square's (at most ~3.5 ft along
/// the line) and the opening is a square's reach along the line wide (5 ft straight, 7 ft slanted).
fn gate_span(a: P, b: P, g: f64) -> (f64, f64) {
    let l = dist(a, b).max(1e-9);
    let u = mul(sub(b, a), 1.0 / l);
    let p = lerp(a, b, g / l);
    let c = [(crate::core::floor(p[0] / SQUARE_FT) + 0.5) * SQUARE_FT, (crate::core::floor(p[1] / SQUARE_FT) + 0.5) * SQUARE_FT];
    let t = dot(sub(c, a), u).clamp(0.0, l);
    (t, 0.5 * GATE_OPEN_FT * (crate::core::fabs(u[0]) + crate::core::fabs(u[1])) + GATE_MARGIN_FT)
}

/// The segment from `a` to `b` minus its gates (ft from `a`, each an opening: `gate_span`) and the
/// gaps' strokes: what stands, exact (stubs under `SLIVER_FT` left out).
pub fn subtract(a: P, b: P, gates: &[f32], gaps: &[FenceGap]) -> Vec<(P, P)> {
    let l = dist(a, b);
    let mut cuts: Vec<(f64, f64)> = gates
        .iter()
        .map(|&g| {
            let (t, h) = gate_span(a, b, g as f64);
            (t - h, t + h)
        })
        .collect();
    gap_spans(a, b, gaps, &mut cuts);
    if cuts.is_empty() {
        return vec![(a, b)];
    }
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
    let mut out = Vec::new();
    let mut at = 0.0f64;
    let at_t = |t: f64| lerp(a, b, t / l.max(1e-9));
    for (x, y) in cuts {
        if x - at >= SLIVER_FT && x > 0.0 {
            out.push((at_t(at), at_t(x.min(l))));
        }
        at = at.max(y);
        if at >= l {
            break;
        }
    }
    if l - at >= SLIVER_FT {
        out.push((at_t(at.max(0.0)), b));
    }
    out
}

/// Where the gates of a run from `a` (its canonical start) to `b` stand: (middle, the line's turn).
pub fn gate_points(a: P, b: P, gates: &[f32]) -> Vec<(P, f64)> {
    let l = dist(a, b);
    let turn = libm::atan2(b[1] - a[1], b[0] - a[0]);
    gates.iter().map(|&g| g as f64).filter(|&g| g >= 0.0 && g < l).map(|g| (lerp(a, b, gate_span(a, b, g).0 / l.max(1e-9)), turn)).collect()
}

/// A park's hedge's or topiary row's ends (`park::hedge_runs`, its places in `lay.props`): a hedge
/// from its first piece's end to its last's, a row of topiary from its first to its last; in
/// canonical order.
pub fn park_run_ends(lay: &crate::park::ParkLayout, run: &[usize]) -> Option<(P, P)> {
    let (first, last) = (lay.props.get(*run.first()?)?, lay.props.get(*run.last()?)?);
    let along = sub(last.0, first.0);
    let u = if len(along) > 1e-6 { mul(along, 1.0 / len(along)) } else { [libm::cos(first.1 as f64), libm::sin(first.1 as f64)] };
    let (ra, rb) = if first.2 == Kind::Hedge { (0.5 * SECTION_FT * first.4 as f64, 0.5 * SECTION_FT * last.4 as f64) } else { (0.0, 0.0) };
    Some(canonical(sub(first.0, mul(u, ra)), add(last.0, mul(u, rb))))
}

/// A park's hedges and topiary rows changed by hand (`FenceCut::park`): the props of each gone or
/// changed run taken out of `lay.props`; a changed hedge (another kind, or gates) laid again as a
/// straight run of its kind, from its first piece's end to its last's, into `runs` (world ft, its
/// gates cut out, their leaves in `runs.gates`); a topiary row given gates loses the topiary in
/// them, one given another kind stands as a run of that kind.
pub fn park_change(lay: &mut crate::park::ParkLayout, changes: &[(u32, FenceChange)], runs: &mut Battle) {
    let all = crate::park::hedge_runs(lay);
    let mut drop: FastSet<usize> = FastSet::default();
    for (j, ch) in changes {
        let Some(run) = all.get(*j as usize) else { continue };
        let hedge = lay.props[run[0]].2 == Kind::Hedge;
        if ch.gone {
            drop.extend(run.iter().copied());
            continue;
        }
        let Some((a, b)) = park_run_ends(lay, run) else { continue };
        if !hedge && ch.kind.is_none_or(|k| k == FenceKind::Hedge) {
            let gates = gate_points(a, b, &ch.gates);
            drop.extend(run.iter().copied().filter(|&k| gates.iter().any(|g| dist(g.0, lay.props[k].0) < 0.5 * GATE_OPEN_FT + 1.5)));
            continue;
        }
        drop.extend(run.iter().copied());
        let kind = drawn_kind(ch.kind.unwrap_or(FenceKind::Hedge));
        for (p, e) in subtract(a, b, &ch.gates, &[]) {
            runs.runs.push((p, e, kind));
        }
        runs.gates.extend(gate_points(a, b, &ch.gates).into_iter().map(|(p, t)| (p, t, gate_look(kind))));
    }
    if !drop.is_empty() {
        let mut k = 0;
        lay.props.retain(|_| {
            k += 1;
            !drop.contains(&(k - 1))
        });
    }
}

/// A walled manor's yard walls (`village::wall_runs`) as changed by hand: gone and changed-kind
/// runs left out (`rural::battle` draws the latter), gates cut out (their leaves in `gates`).
pub fn garden_walls(walls: Vec<(P, P)>, cut: &FenceCut, gates: &mut Vec<(P, f64, u8)>) -> Vec<(P, P)> {
    let mut out = Vec::new();
    for (a, b) in walls {
        match cut.run(GARDEN, a, b) {
            None => out.push((a, b)),
            Some(ch) if ch.gone || ch.kind.is_some_and(|k| k != FenceKind::GardenWall) => {}
            Some(ch) => {
                let (s, e) = canonical(a, b);
                out.extend(subtract(s, e, &ch.gates, &[]));
                gates.extend(gate_points(s, e, &ch.gates).into_iter().map(|(p, t)| (p, t, gate_look(RunKind::Wall))));
            }
        }
    }
    out
}

/// Runs and gates of `from` (world ft, whole) clipped to the chunk `rect` into `out`.
pub fn take(from: &Battle, rect: [f64; 4], out: &mut Battle) {
    for &(a, b, kind) in &from.runs {
        if let Some((p, e)) = rural::clip_seg(a, b, kind.clip(rect))
            && out.runs.len() < rural::MAX_RUNS + MAX_HAND_RUNS
        {
            out.runs.push((p, e, kind));
        }
    }
    out.gates.extend(from.gates.iter().copied().filter(|g| inside(g.0, rect)));
}

#[inline]
fn inside(p: P, rect: [f64; 4]) -> bool {
    p[0] >= rect[0] && p[0] < rect[2] && p[1] >= rect[1] && p[1] < rect[3]
}

/// The fences drawn by hand reaching the chunk `rect` (world ft) as runs into `out`: each line's
/// segments minus its gates, cut by water only (`wet`: across roads and roofs, as drawn), their
/// gates' leaves.
pub fn drawn(edits: &Edits, rect: [f64; 4], wet: &dyn Fn(P) -> bool, out: &mut Battle) {
    let near = [rect[0] - 8.0, rect[1] - 8.0, rect[2] + 8.0, rect[3] + 8.0];
    for f in edits.fences.values() {
        drawn_one(f, rect, near, wet, out);
    }
}

fn drawn_one(f: &Fence, rect: [f64; 4], near: [f64; 4], wet: &dyn Fn(P) -> bool, out: &mut Battle) {
    let mut pts: Vec<P> = f.pts.iter().copied().take(crate::model::fences::MAX_FENCE_PTS).collect();
    if pts.len() < 2 {
        return;
    }
    if f.closed && pts.len() >= 3 {
        pts.push(pts[0]);
    }
    let b = bbox(&pts);
    if b[0] > near[2] || b[2] < near[0] || b[1] > near[3] || b[3] < near[1] {
        return;
    }
    let kind = drawn_kind(f.kind);
    let mut s0 = 0.0f64;
    for w in pts.windows(2) {
        let (a, e) = (w[0], w[1]);
        let l = dist(a, e);
        if l < 1e-6 {
            continue;
        }
        let gates: Vec<f32> = f.gates.iter().map(|&g| g as f64 - s0).filter(|&g| g > -GATE_OPEN_FT && g < l + GATE_OPEN_FT).map(|g| g as f32).collect();
        for g in gate_points(a, e, &gates) {
            if inside(g.0, rect) {
                out.gates.push((g.0, g.1, gate_look(kind)));
            }
        }
        s0 += l;
        if a[0].min(e[0]) > near[2] || a[0].max(e[0]) < near[0] || a[1].min(e[1]) > near[3] || a[1].max(e[1]) < near[1] {
            continue;
        }
        for (p, r) in subtract(a, e, &gates, &[]) {
            // (In pieces, as the countryside's runs, the ones over water left out.)
            let m = (dist(p, r) / DRAWN_PIECE_FT).ceil().max(1.0) as usize;
            let mut from: Option<usize> = None;
            for j in 0..=m {
                let ok = j < m && !wet(lerp(p, r, (j as f64 + 0.5) / m as f64));
                match (ok, from) {
                    (true, None) => from = Some(j),
                    (false, Some(fj)) => {
                        if let Some((x, y)) = rural::clip_seg(lerp(p, r, fj as f64 / m as f64), lerp(p, r, j as f64 / m as f64), kind.clip(rect))
                            && out.runs.len() < rural::MAX_RUNS + MAX_HAND_RUNS
                        {
                            out.runs.push((x, y, kind));
                        }
                        from = None;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// The chunk's runs and gates' leaves minus the eraser's gaps (exact).
pub fn cut_gaps(gaps: &[FenceGap], b: &mut Battle) {
    if gaps.is_empty() {
        return;
    }
    let runs = std::mem::take(&mut b.runs);
    for (a, e, kind) in runs {
        for (p, q) in subtract(a, e, &[], gaps) {
            b.runs.push((p, q, kind));
        }
    }
    b.gates.retain(|g| !in_gap(gaps, g.0));
}

/// Whether `p` (world ft) lies under one of the eraser's strokes.
pub fn in_gap(gaps: &[FenceGap], p: P) -> bool {
    gaps.iter().any(|g| {
        let r = g.r.max(0.0) as f64;
        match g.pts.len() {
            0 => false,
            1 => dist(p, g.pts[0]) < r,
            _ => g.pts.windows(2).any(|w| seg_dist(p, w[0], w[1]) < r),
        }
    })
}

/// Whether objects of `kind` are a fence's or a hedge's (the eraser takes them).
#[inline]
pub fn is_fence_object(kind: Kind) -> bool {
    matches!(kind, Kind::Hedge | Kind::WattleFence | Kind::Picket | Kind::IronRailing | Kind::Stockade | Kind::Gate)
}

/// The chunk's (origin `ox`, `oy` ft) fence and hedge objects whose middles lie under a gap left out.
pub fn gap_objects(gaps: &[FenceGap], ox: f64, oy: f64, objects: &mut Vec<Object>) {
    if gaps.is_empty() {
        return;
    }
    objects.retain(|o| !(is_fence_object(o.kind) && in_gap(gaps, [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT])));
}

/// The objects of a chunk (origin `ox`, `oy` ft) a run laid in sections stands as: 5-ft sections
/// end to end along it (as many as cover it, evenly), and each gate's open leaf.
pub fn objects(b: &Battle, ox: f64, oy: f64, objects: &mut Vec<Object>) {
    let at = |p: P| (((p[0] - ox) / SQUARE_FT) as f32, ((p[1] - oy) / SQUARE_FT) as f32);
    for &(a, e, kind) in &b.runs {
        let RunKind::Pieces(k) = kind else { continue };
        let l = dist(a, e);
        if l < SLIVER_FT {
            continue;
        }
        let n = (l / SECTION_FT).ceil().max(1.0) as usize;
        let rot = libm::atan2(e[1] - a[1], e[0] - a[0]) as f32;
        let scale = (l / n as f64 / SECTION_FT) as f32;
        for i in 0..n {
            let (x, y) = at(lerp(a, e, (i as f64 + 0.5) / n as f64));
            objects.push(Object { kind: k, variant: 0, x, y, rot, scale, sprite: 0, turned: false });
        }
    }
    for &(p, turn, look) in &b.gates {
        let (x, y) = at(p);
        objects.push(Object { kind: Kind::Gate, variant: look, x, y, rot: turn as f32, scale: 1.0, sprite: 0, turned: false });
    }
}
