//! (Roadmap 6 R6) Fences and hedges changed by hand: what the app's Build › Fence and Select card,
//! the Gazetteer's "Bring back" and the MCP tools `place_fence` and `remove_fences` ask.
//!
//! `change` turns a list of ids and one op into the edits that do it (nothing is changed here):
//! - `fr:` a generated run, `fh:` a park's hedge or row of topiary (`town::fence`): their
//!   `TownEdit::fences` change: gone (`Remove`), back as generated (`Restore`), another kind
//!   (`Kind`), a gate put in (`Gate`, projected onto the run, ft from its canonical start). A
//!   garden wall keeps its kind, and no run takes `garden_wall`; a hand town's are refused;
//! - `f:` a fence drawn by hand (`Edits.fences`): drawn or changed (`Draw`), deleted (`Remove`),
//!   given a kind or a gate; `fg:` an eraser's gap (`Edits.fence_gaps`): cut (`Gap`), dropped
//!   (`Remove`). Neither is brought back here (Undo does).
//!
//! `changed` lists the towns whose fences are changed, for the Gazetteer; `near` every fence and
//! hedge round a point, for the agent. Pure queries: nothing generated reads them.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

use crate::World;
use crate::model::fences::{Fence, FenceChange, FenceGap, FenceKind, GATE_OPEN_FT, MAX_FENCE_PTS};
use crate::roadcut::Refused;
use crate::t0::T0;
use crate::town::{self, Layout, fence, geom};
use crate::world::{Edits, TownEdit};

type P = [f64; 2];

/// How far (ft) round a changed line the battlemaps redraw.
const PAD_FT: f64 = 10.0;
/// The most ids one MCP call takes (`change` takes any number: the app's Select box can hold thousands).
pub const MAX_IDS: usize = 500;
/// The most fences drawn by hand, and gaps, a world holds.
pub const MAX_FENCES: usize = 5000;
pub const MAX_GAPS: usize = 5000;
/// An eraser stroke's radius (ft), least and most.
pub const MIN_GAP_R_FT: f32 = 0.5;
pub const MAX_GAP_R_FT: f32 = 10.0;
/// The most gates one line holds.
const MAX_GATES: usize = 64;
/// A gate's middle stays this far (ft) from a line's ends.
const GATE_END_FT: f64 = 0.5 * GATE_OPEN_FT;
/// The farthest (ft) from its line a gate's point may be (a gate is put where the line is nearest).
const GATE_REACH_FT: f64 = 10.0;
/// The widest `near` (ft).
pub const MAX_NEAR_FT: f64 = 2000.0;
/// The widest `near` that lists parks' hedges (`fh:`; a park is laid out for each).
pub const PARK_NEAR_FT: f64 = 400.0;

/// What a `change` does to its ids.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Remove,
    Restore,
    /// Another kind (None: as it is) and gates put in at points (each projected onto the line).
    Change(Option<FenceKind>, Vec<P>),
    /// A fence drawn (no id) or one drawn changed (one `f:` id).
    Draw(Fence),
    /// Eraser's gaps cut (each a new `fg:`).
    Gaps(Vec<FenceGap>),
}

impl Op {
    /// From `{op: remove|restore|kind|gate|draw|gap, kind?, at?: [x, y] | [[x, y], ..], fence?,
    /// gap?: FenceGap | [FenceGap, ..]}`.
    pub fn parse(v: &Value) -> Result<Op, String> {
        let kind = || -> Result<FenceKind, String> {
            let k = v["kind"].as_str().ok_or("kind: missing")?;
            FenceKind::parse(k).ok_or_else(|| format!("kind: one of {}", FenceKind::ALL.iter().map(|k| k.name()).collect::<Vec<_>>().join(", ")))
        };
        let at = || -> Result<Vec<P>, String> {
            serde_json::from_value::<P>(v["at"].clone()).map(|p| vec![p]).or_else(|_| serde_json::from_value::<Vec<P>>(v["at"].clone())).map_err(|_| "at: [x, y] or a list of them (world ft)".to_string())
        };
        Ok(match v["op"].as_str().unwrap_or("") {
            "remove" => Op::Remove,
            "restore" => Op::Restore,
            "kind" => Op::Change(Some(kind()?), Vec::new()),
            "gate" => Op::Change(None, at()?),
            "draw" => Op::Draw(serde_json::from_value(v["fence"].clone()).map_err(|e| format!("fence: {e}"))?),
            "gap" => Op::Gaps(match &v["gap"] {
                Value::Array(_) => serde_json::from_value(v["gap"].clone()),
                g => serde_json::from_value(g.clone()).map(|g| vec![g]),
            }
            .map_err(|e| format!("gap: {e}"))?),
            o => return Err(format!("op: remove, restore, kind, gate, draw or gap (got {o:?})")),
        })
    }
}

/// The edits a `change` makes, and what it did.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ChangeOut {
    /// `Edits.fences` entries as they become (None: the entry goes).
    pub fences: BTreeMap<String, Option<Fence>>,
    /// `Edits.fence_gaps` entries as they become (None: the entry goes).
    pub fence_gaps: BTreeMap<String, Option<FenceGap>>,
    /// Town edits (`Edits.towns`, by key) as they become (None: the entry goes).
    pub towns: BTreeMap<String, Option<TownEdit>>,
    /// Boxes (world ft) whose battlemaps redraw.
    pub rects: Vec<[f64; 4]>,
    /// Boxes whose site-level tiles redraw too (a corral's, a pen's, a churchyard's marks on the plan).
    pub site_rects: Vec<[f64; 4]>,
    /// The ids asked for that were changed (a drawn fence or gap: its id, new or not).
    pub done: Vec<String>,
    pub refused: Vec<Refused>,
}

impl ChangeOut {
    /// `e` with the change made.
    pub fn apply(&self, e: &mut Edits) {
        for (k, v) in &self.fences {
            match v {
                Some(f) => drop(e.fences.insert(k.clone(), f.clone())),
                None => {
                    e.fences.remove(k);
                    e.notes.remove(k);
                    e.hidden.remove(k);
                }
            }
        }
        for (k, v) in &self.fence_gaps {
            match v {
                Some(g) => drop(e.fence_gaps.insert(k.clone(), g.clone())),
                None => drop(e.fence_gaps.remove(k)),
            }
        }
        for (k, v) in &self.towns {
            match v {
                Some(t) => drop(e.towns.insert(k.clone(), t.clone())),
                None => drop(e.towns.remove(k)),
            }
        }
    }

    /// Whether it changes nothing.
    pub fn is_empty(&self) -> bool {
        self.fences.is_empty() && self.fence_gaps.is_empty() && self.towns.is_empty()
    }
}

fn rect(pts: &[P], pad: f64) -> [f64; 4] {
    let b = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    [crate::core::floor(b[0] - pad), crate::core::floor(b[1] - pad), crate::core::ceil(b[2] + pad), crate::core::ceil(b[3] + pad)]
}

/// A generated run by id.
#[derive(Clone, Debug)]
pub struct Run {
    pub li: usize,
    /// Its key in `TownEdit::fences`.
    pub local: String,
    /// Its ends, from its canonical start.
    pub a: P,
    pub b: P,
    /// Its source's tag (`town::fence::FIELD`…; 0 a park's hedge or topiary row).
    pub tag: u8,
    /// Its kind as generated (None: a row of topiary).
    pub kind: Option<FenceKind>,
}

impl Run {
    pub fn id(&self) -> String {
        if self.tag == 0 { format!("fh:{}:{}", self.li, self.local) } else { format!("fr:{}:{}", self.li, self.local) }
    }

    /// Whether the plan marks it (a corral's or pen's fence, a croft's bound or the churchyard's wall).
    fn on_plan(&self) -> bool {
        matches!(self.tag, fence::CORRAL | fence::PEN | fence::CROFT)
    }
}

/// Park `pk` of `l`'s hedges and topiary rows (`park::hedge_runs`): (j, a, b, kind; None topiary).
pub fn park_runs(world: &World, t0: &T0, l: &Layout, pk: usize) -> Vec<(usize, P, P, Option<FenceKind>)> {
    let park = &l.parks[pk];
    if park.kind == town::ParkKind::Horses {
        return Vec::new();
    }
    let lay = crate::battlemap::park_layout(park, crate::battlemap::land_biome(t0, geom::centroid(&park.pts)), l.tier >= crate::t0::settle::Tier::City, world.stream("battlemap"));
    let mut out = Vec::new();
    for (j, run) in crate::park::hedge_runs(&lay).iter().enumerate() {
        let Some((a, b)) = fence::park_run_ends(&lay, run) else { continue };
        let hedge = run.first().and_then(|&k| lay.props.get(k)).is_some_and(|p| p.2 == crate::battlemap::Kind::Hedge);
        out.push((j, a, b, hedge.then_some(FenceKind::Hedge)));
    }
    out
}

/// What a batch of ids looks up once: a layout's runs by key, a layout's parks by spot, a park's
/// hedges and rows.
#[derive(Default)]
struct Cache {
    runs: BTreeMap<usize, BTreeMap<u32, (u8, P, P, FenceKind)>>,
    spots: BTreeMap<usize, BTreeMap<String, usize>>,
    parks: BTreeMap<(usize, usize), Vec<(usize, P, P, Option<FenceKind>)>>,
}

impl Cache {
    /// A generated run by id (Ok(None): no such run; Err: why not).
    fn run(&mut self, world: &World, t0: &T0, id: &str) -> Result<Option<Run>, String> {
        let Some((li, local)) = fence::parse(id) else { return Ok(None) };
        if li >= town::layout_count(t0) || !town::is_settlement(t0, li) {
            return Ok(None);
        }
        if town::hand::is_hand(world, li) {
            return Err("a town laid out by hand: change it in its street plan (Streets tab, edit_streets)".into());
        }
        let l = town::layout(world, t0, li);
        match local.split_once(':') {
            None => {
                let Ok(key) = u32::from_str_radix(&local, 16) else { return Ok(None) };
                let runs = self.runs.entry(li).or_insert_with(|| {
                    let mut m = BTreeMap::new();
                    for s in fence::sources(&l, t0, fence::EVERYWHERE) {
                        m.entry(fence::run_key(s.0, s.1, s.2)).or_insert(s);
                    }
                    m
                });
                Ok(runs.get(&key).map(|&(tag, a, b, kind)| {
                    let (a, b) = fence::canonical(a, b);
                    Run { li, local, a, b, tag, kind: Some(kind) }
                }))
            }
            Some((spot, j)) => {
                let spots = self.spots.entry(li).or_insert_with(|| {
                    let mut m = BTreeMap::new();
                    for k in 0..l.parks.len() {
                        m.entry(town::cut::park_spot(&l, k)).or_insert(k);
                    }
                    m
                });
                let (Some(&pk), Ok(j)) = (spots.get(spot), j.parse::<usize>()) else { return Ok(None) };
                let runs = self.parks.entry((li, pk)).or_insert_with(|| park_runs(world, t0, &l, pk));
                Ok(runs.iter().find(|r| r.0 == j).map(|&(_, a, b, kind)| Run { li, local, a, b, tag: 0, kind }))
            }
        }
    }
}

/// The working copy of the town edits a change makes.
struct Towns<'a> {
    edits: &'a Edits,
    towns: BTreeMap<String, Option<TownEdit>>,
}

impl Towns<'_> {
    fn get(&self, key: &str) -> Option<&TownEdit> {
        match self.towns.get(key) {
            Some(t) => t.as_ref(),
            None => self.edits.towns.get(key),
        }
    }

    fn put(&mut self, key: String, t: TownEdit) {
        let v = (!crate::roadcut::holds_nothing(&t)).then_some(t);
        if v.as_ref() == self.edits.towns.get(&key) {
            self.towns.remove(&key);
        } else {
            self.towns.insert(key, v);
        }
    }

    /// The change recorded for run `local` of layout `li` dropped, though no run answers to it any
    /// more (a ward edit moved its ends): whether there was one.
    fn forget(&mut self, li: usize, local: &str) -> bool {
        let key = town::wards::key(li);
        let Some(mut t) = self.get(&key).cloned() else { return false };
        if t.fences.remove(local).is_none() {
            return false;
        }
        self.put(key, t);
        true
    }

    /// Run `r`'s change (None: as generated) changed by `f` (Err: why not).
    fn change(&mut self, world: &World, t0: &T0, r: &Run, f: impl FnOnce(Option<FenceChange>) -> Result<Option<FenceChange>, String>) -> Result<(), String> {
        let home = town::added::home(world, t0, r.li).ok_or("not a generated town")?;
        let key = town::wards::key(r.li);
        let mut t = self.get(&key).cloned().unwrap_or_else(|| TownEdit { at: home, ..Default::default() });
        if geom::dist(t.at, home) > TownEdit::SAME_AT_FT {
            return Err("the town's edits are set aside (it moved): open it in Streets first".into());
        }
        match f(t.fences.get(&r.local).cloned())?.filter(|c| !c.is_noop()) {
            Some(c) => drop(t.fences.insert(r.local.clone(), c)),
            None => drop(t.fences.remove(&r.local)),
        }
        self.put(key, t);
        Ok(())
    }
}

/// Where along a line (ft from its first point) `p` falls, kept `GATE_END_FT` from an open line's
/// ends (Err: why not: too short for a gate, or `p` farther than `GATE_REACH_FT` from the line).
pub fn along(pts: &[P], closed: bool, p: P) -> Result<f64, String> {
    let mut line: Vec<P> = pts.to_vec();
    if closed && pts.len() >= 3 {
        line.push(pts[0]);
    }
    let total: f64 = line.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
    if total < 2.0 * GATE_END_FT + 0.1 {
        return Err("too short for a gate".into());
    }
    let (mut best, mut s0, mut at) = (f64::MAX, 0.0, 0.0);
    for w in line.windows(2) {
        let l = geom::dist(w[0], w[1]);
        if l > 1e-9 {
            let t = (((p[0] - w[0][0]) * (w[1][0] - w[0][0]) + (p[1] - w[0][1]) * (w[1][1] - w[0][1])) / (l * l)).clamp(0.0, 1.0);
            let d = geom::dist(p, geom::lerp(w[0], w[1], t));
            if d < best {
                (best, at) = (d, s0 + t * l);
            }
        }
        s0 += l;
    }
    if best > GATE_REACH_FT {
        return Err(format!("the gate's point is {} ft from the line (at most {GATE_REACH_FT})", crate::core::round(best)));
    }
    let at = if closed { at } else { at.clamp(GATE_END_FT, total - GATE_END_FT) };
    Ok(crate::core::round(at * 10.0) / 10.0)
}

/// `gates` with one more at `g` (Err: one is there already, or too many).
fn with_gate(gates: &[f32], g: f64) -> Result<Vec<f32>, String> {
    if gates.iter().any(|&x| (x as f64 - g).abs() < GATE_OPEN_FT) {
        return Err("a gate is there already".into());
    }
    if gates.len() >= MAX_GATES {
        return Err(format!("at most {MAX_GATES} gates a line"));
    }
    let mut out = gates.to_vec();
    out.push(g as f32);
    out.sort_by(f32::total_cmp);
    Ok(out)
}

/// What is wrong with a fence drawn by hand, if anything.
pub fn problem(f: &Fence) -> Option<String> {
    if f.pts.len() < 2 || f.pts.len() > MAX_FENCE_PTS {
        return Some(format!("points: 2 to {MAX_FENCE_PTS}"));
    }
    if f.pts.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
        return Some("points: numbers (world ft)".into());
    }
    if f.closed && f.pts.len() < 3 {
        return Some("closed: 3 or more points".into());
    }
    if f.kind == FenceKind::GardenWall {
        return Some("garden_wall is generated only: low_wall or high_wall".into());
    }
    let total: f64 = f.pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum::<f64>() + if f.closed { geom::dist(f.pts[f.pts.len() - 1], f.pts[0]) } else { 0.0 };
    if total < 1.0 {
        return Some("the line is under 1 ft".into());
    }
    if f.gates.len() > MAX_GATES || f.gates.iter().any(|&g| !g.is_finite() || g < 0.0 || g as f64 > total) {
        return Some(format!("gates: at most {MAX_GATES}, each ft along the line (0 to {})", crate::core::round(total)));
    }
    None
}

/// The smallest `<head>:<n>` above every one `keys` holds.
fn next_id<'a>(head: &str, keys: impl Iterator<Item = &'a String>) -> String {
    let n = keys.filter_map(|k| k.strip_prefix(head).and_then(|r| r.strip_prefix(':')).and_then(|n| n.parse::<u64>().ok())).max().map_or(1, |n| n + 1);
    format!("{head}:{n}")
}

/// The edits that make `op` on `ids` (see the module doc). `dry`: the boxes to redraw for each id
/// that resolves, whatever its state, and nothing else (the app asks it after an Undo).
pub fn change(world: &World, t0: &T0, ids: &[String], op: &Op, dry: bool) -> ChangeOut {
    let e = &world.file.edits;
    let mut out = ChangeOut::default();
    let mut towns = Towns { edits: e, towns: BTreeMap::new() };
    let refuse = |out: &mut ChangeOut, id: &str, why: &str| out.refused.push(Refused { id: id.to_string(), why: why.to_string() });
    let fence_now = |out: &ChangeOut, id: &str| match out.fences.get(id) {
        Some(f) => f.clone(),
        None => e.fences.get(id).cloned(),
    };
    // A new line: drawn or cut.
    match op {
        Op::Draw(f) if ids.is_empty() => {
            if let Some(why) = problem(f) {
                refuse(&mut out, "", &why);
                return out;
            }
            if e.fences.len() >= MAX_FENCES {
                refuse(&mut out, "", &format!("a world holds at most {MAX_FENCES} fences drawn by hand"));
                return out;
            }
            out.rects.push(rect(&f.pts, PAD_FT));
            let id = next_id("f", e.fences.keys());
            if !dry {
                out.fences.insert(id.clone(), Some(f.clone()));
            }
            out.done.push(id);
            return out;
        }
        Op::Gaps(gs) => {
            let mut n = e.fence_gaps.len();
            let mut next = next_id("fg", e.fence_gaps.keys());
            for g in gs {
                if g.pts.is_empty() || g.pts.len() > MAX_FENCE_PTS || g.pts.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) || !(MIN_GAP_R_FT..=MAX_GAP_R_FT).contains(&g.r) {
                    refuse(&mut out, "", &format!("gap: 1 to {MAX_FENCE_PTS} points, radius {MIN_GAP_R_FT} to {MAX_GAP_R_FT} ft"));
                    continue;
                }
                if n >= MAX_GAPS {
                    refuse(&mut out, "", &format!("a world holds at most {MAX_GAPS} gaps"));
                    break;
                }
                n += 1;
                out.rects.push(rect(&g.pts, g.r as f64 + PAD_FT));
                let id = next.clone();
                let k: u64 = id[3..].parse().unwrap_or(0);
                next = format!("fg:{}", k + 1);
                if !dry {
                    out.fence_gaps.insert(id.clone(), Some(g.clone()));
                }
                out.done.push(id);
            }
            return out;
        }
        _ => {}
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut cache = Cache::default();
    for id in ids {
        if !seen.insert(id.as_str()) {
            continue;
        }
        let head = id.split(':').next().unwrap_or("");
        match head {
            "fr" | "fh" => {
                let run = match cache.run(world, t0, id) {
                    Ok(Some(r)) => r,
                    Ok(None) => {
                        // (A change whose run lapsed, its ends moved by a ward edit: Restore forgets it.)
                        if *op == Op::Restore
                            && !dry
                            && let Some((li, local)) = fence::parse(id)
                            && towns.forget(li, &local)
                        {
                            out.done.push(id.clone());
                            continue;
                        }
                        refuse(&mut out, id, if fence::parse(id).is_some() { "no such fence or hedge" } else { "not a fence id" });
                        continue;
                    }
                    Err(why) => {
                        refuse(&mut out, id, &why);
                        continue;
                    }
                };
                let r = rect(&[run.a, run.b], PAD_FT);
                // A run with a plan mark redraws its site's tiles too, besides the battlemaps.
                let plan = run.on_plan() && matches!(op, Op::Remove | Op::Restore);
                if dry {
                    out.rects.push(r);
                    if plan {
                        out.site_rects.push(r);
                    }
                    out.done.push(id.clone());
                    continue;
                }
                let garden = run.kind == Some(FenceKind::GardenWall);
                let res = towns.change(world, t0, &run, |c| match op {
                    Op::Remove => match c {
                        Some(c) if c.gone => Err("already removed".into()),
                        c => Ok(Some(FenceChange { gone: true, ..c.unwrap_or_default() })),
                    },
                    Op::Restore => match c {
                        Some(_) => Ok(None),
                        None => Err("not changed".into()),
                    },
                    _ if c.as_ref().is_some_and(|c| c.gone) => Err("removed: restore it first".into()),
                    Op::Change(Some(_), _) if garden => Err("a garden wall keeps its kind (remove it, or cut a gap)".into()),
                    Op::Change(Some(FenceKind::GardenWall), _) => Err("garden_wall is generated only: low_wall or high_wall".into()),
                    Op::Change(k, gates) => {
                        let mut c = c.unwrap_or_default();
                        if let Some(k) = k {
                            c.kind = (Some(*k) != run.kind).then_some(*k);
                        }
                        for p in gates {
                            c.gates = with_gate(&c.gates, along(&[run.a, run.b], false, *p)?)?;
                        }
                        Ok(Some(c))
                    }
                    Op::Draw(_) | Op::Gaps(_) => Err("a generated run: give kind or gates, not points".into()),
                });
                match res {
                    Ok(()) => {
                        out.rects.push(r);
                        if plan {
                            out.site_rects.push(r);
                        }
                        out.done.push(id.clone());
                    }
                    Err(why) => refuse(&mut out, id, &why),
                }
            }
            "f" => {
                let Some(f) = fence_now(&out, id) else {
                    refuse(&mut out, id, "no such fence");
                    continue;
                };
                let r = rect(&f.pts, PAD_FT);
                if dry {
                    out.rects.push(r);
                    out.done.push(id.clone());
                    continue;
                }
                let new = match op {
                    Op::Remove => Ok(None),
                    Op::Restore => Err("a fence drawn by hand: Undo brings it back".to_string()),
                    Op::Change(Some(FenceKind::GardenWall), _) => Err("garden_wall is generated only: low_wall or high_wall".into()),
                    Op::Change(k, gates) => {
                        let mut n = Fence { kind: k.unwrap_or(f.kind), ..f.clone() };
                        let mut res = Ok(());
                        for p in gates {
                            match along(&n.pts, n.closed, *p).and_then(|g| with_gate(&n.gates, g)) {
                                Ok(g) => n.gates = g,
                                Err(e) => {
                                    res = Err(e);
                                    break;
                                }
                            }
                        }
                        res.map(|()| Some(n))
                    }
                    Op::Draw(n) => match problem(n) {
                        Some(why) => Err(why),
                        None => Ok(Some(n.clone())),
                    },
                    Op::Gaps(_) => Err("not a gap".into()),
                };
                match new {
                    Ok(n) => {
                        out.rects.push(r);
                        if let Some(n) = &n {
                            out.rects.push(rect(&n.pts, PAD_FT));
                        }
                        out.fences.insert(id.clone(), n);
                        out.done.push(id.clone());
                    }
                    Err(why) => refuse(&mut out, id, &why),
                }
            }
            "fg" => {
                let Some(g) = e.fence_gaps.get(id) else {
                    refuse(&mut out, id, "no such gap");
                    continue;
                };
                if !dry && *op != Op::Remove {
                    refuse(&mut out, id, if *op == Op::Restore { "a gap: Undo brings it back" } else { "a gap can only be removed" });
                    continue;
                }
                out.rects.push(rect(&g.pts, g.r as f64 + PAD_FT));
                if !dry {
                    out.fence_gaps.insert(id.clone(), None);
                }
                out.done.push(id.clone());
            }
            _ => refuse(&mut out, id, "not a fence, hedge or gap id (fr:, fh:, f:, fg:)"),
        }
    }
    if !dry {
        out.towns = towns.towns;
    }
    out
}

pub fn change_json(world: &World, t0: &T0, ids: &[String], op: &Op, dry: bool) -> Value {
    serde_json::to_value(change(world, t0, ids, op, dry)).unwrap_or(Value::Null)
}

/// A town whose generated fences and hedges are changed by hand, for the Gazetteer.
#[derive(Clone, Debug, Serialize)]
pub struct Changed {
    /// Its key in `Edits.towns`.
    pub town: String,
    pub name: Option<String>,
    /// Its changed runs' ids (`fr:`, `fh:`), to bring back with `change(.., Op::Restore)`.
    pub ids: Vec<String>,
    /// False when its changes are set aside (the town moved, or is laid out by hand now), or when
    /// every one of them has lapsed.
    pub found: bool,
    /// Of `ids`, those no run answers to any more (a ward edit moved its ends): Bring back drops them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stale: Vec<String>,
}

/// Every town with fence changes, in key order.
pub fn changed(world: &World, t0: &T0) -> Vec<Changed> {
    let mut out = Vec::new();
    let mut cache = Cache::default();
    for (key, t) in &world.file.edits.towns {
        let Ok(li) = key.parse::<usize>() else { continue };
        if t.fences.is_empty() || li >= town::layout_count(t0) || !town::is_settlement(t0, li) {
            continue;
        }
        let ids: Vec<String> = t.fences.keys().map(|k| if k.contains(':') { format!("fh:{li}:{k}") } else { format!("fr:{li}:{k}") }).collect();
        let name = crate::agent::feature_of_layout(world, t0, li).and_then(|f| crate::agent::name_of(world, t0, &f));
        let at = fence::changes_of(world, t0, li).is_some();
        let stale: Vec<String> = if at { ids.iter().filter(|id| matches!(cache.run(world, t0, id), Ok(None))).cloned().collect() } else { Vec::new() };
        out.push(Changed { town: key.clone(), name, found: at && stale.len() < ids.len(), ids, stale });
    }
    out
}

pub fn changed_json(world: &World, t0: &T0) -> Value {
    json!(changed(world, t0))
}

fn r1(v: f64) -> f64 {
    crate::core::round(v * 10.0) / 10.0
}

fn pt(p: P) -> [f64; 2] {
    [crate::core::round(p[0]), crate::core::round(p[1])]
}

/// A changed run's entry (`near`, `list`).
fn change_entry(v: &mut Value, c: Option<&FenceChange>) {
    if let Some(c) = c {
        if c.gone {
            v["gone"] = json!(true);
        }
        if !c.gates.is_empty() {
            v["gates"] = json!(c.gates);
        }
        if let Some(k) = c.kind {
            v["kind"] = json!(k.name());
            v["changed_from"] = v["generated"].take();
        }
    }
    if let Some(o) = v.as_object_mut() {
        o.remove("generated");
    }
}

/// Every fence and hedge within `r` (ft, at most `MAX_NEAR_FT`) of `at`, nearest first: generated
/// runs (`{id, kind, length_ft, ends, town, gone?, gates?, changed_from?}`; parks' hedges only
/// within `PARK_NEAR_FT`), drawn fences (`{id, kind, length_ft, pts, closed?, gates?}`) and gaps
/// (`{id, pts, width_ft}`); at most `limit`.
pub fn near(world: &World, t0: &T0, at: P, r: f64, limit: usize) -> Vec<Value> {
    let r = r.clamp(0.0, MAX_NEAR_FT);
    let b = [at[0] - r, at[1] - r, at[0] + r, at[1] + r];
    let mut out: Vec<(f64, Value)> = Vec::new();
    for l in town::layouts_near(world, t0, b) {
        let li = l.index as usize;
        if !town::is_settlement(t0, li) || town::added::home(world, t0, li).is_none() || town::hand::is_hand(world, li) {
            continue;
        }
        let changes = world.file.edits.towns.get(&town::wards::key(li)).map(|t| &t.fences);
        let name = crate::agent::feature_of_layout(world, t0, li).and_then(|f| crate::agent::name_of(world, t0, &f));
        let mut put = |id: String, local: &str, a: P, b2: P, kind: Option<FenceKind>| {
            let d = geom::seg_dist(at, a, b2);
            if d > r {
                return;
            }
            let k = kind.map_or("topiary", |k| k.name());
            let mut v = json!({ "id": id, "kind": k, "generated": k, "length_ft": r1(geom::dist(a, b2)), "ends": [pt(a), pt(b2)], "town": name });
            change_entry(&mut v, changes.and_then(|c| c.get(local)));
            out.push((d, v));
        };
        for (tag, a, b2, kind) in fence::sources(&l, t0, b) {
            let (a, b2) = fence::canonical(a, b2);
            let key = fence::run_key(tag, a, b2);
            put(fence::fr_id(li, key), &format!("{key:08x}"), a, b2, Some(kind));
        }
        if r <= PARK_NEAR_FT {
            for pk in 0..l.parks.len() {
                let pb = rect(&l.parks[pk].pts, 0.0);
                if pb[0] > b[2] || pb[2] < b[0] || pb[1] > b[3] || pb[3] < b[1] {
                    continue;
                }
                let spot = town::cut::park_spot(&l, pk);
                for (j, a, b2, kind) in park_runs(world, t0, &l, pk) {
                    put(fence::fh_id(li, &spot, j), &format!("{spot}:{j}"), a, b2, kind);
                }
            }
        }
    }
    let line_d = |pts: &[P], closed: bool| {
        let mut d = pts.windows(2).map(|w| geom::seg_dist(at, w[0], w[1])).fold(f64::MAX, f64::min);
        if closed && pts.len() >= 3 {
            d = d.min(geom::seg_dist(at, pts[pts.len() - 1], pts[0]));
        }
        if pts.len() == 1 { geom::dist(at, pts[0]) } else { d }
    };
    for (id, f) in &world.file.edits.fences {
        let d = line_d(&f.pts, f.closed);
        if d <= r {
            out.push((d, drawn_entry(id, f)));
        }
    }
    for (id, g) in &world.file.edits.fence_gaps {
        let d = line_d(&g.pts, false) - g.r as f64;
        if d <= r {
            out.push((d.max(0.0), gap_entry(id, g)));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().take(limit).map(|x| x.1).collect()
}

/// A drawn fence's entry in lists and replies.
pub fn drawn_entry(id: &str, f: &Fence) -> Value {
    let mut len: f64 = f.pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
    if f.closed && f.pts.len() >= 3 {
        len += geom::dist(f.pts[f.pts.len() - 1], f.pts[0]);
    }
    let mut v = json!({ "id": id, "kind": f.kind.name(), "length_ft": r1(len), "pts": f.pts.iter().map(|&p| pt(p)).collect::<Vec<_>>() });
    if f.closed {
        v["closed"] = json!(true);
    }
    if !f.gates.is_empty() {
        v["gates"] = json!(f.gates);
    }
    v
}

/// A gap's entry in lists and replies.
pub fn gap_entry(id: &str, g: &FenceGap) -> Value {
    json!({ "id": id, "pts": g.pts.iter().map(|&p| pt(p)).collect::<Vec<_>>(), "width_ft": r1(2.0 * g.r as f64) })
}

/// Everything changed by hand (no `at`): drawn fences, gaps and each changed run (`{id, town,
/// gone?, kind?, gates?, found}`).
pub fn list(world: &World, t0: &T0) -> Value {
    let e = &world.file.edits;
    let mut runs = Vec::new();
    for c in changed(world, t0) {
        let Some(t) = e.towns.get(&c.town) else { continue };
        for (id, (_, ch)) in c.ids.iter().zip(&t.fences) {
            let mut v = json!({ "id": id, "town": c.name, "found": c.found && !c.stale.contains(id), "generated": null });
            change_entry(&mut v, Some(ch));
            if let Some(o) = v.as_object_mut() {
                o.remove("changed_from");
            }
            runs.push(v);
        }
    }
    json!({
        "drawn": e.fences.iter().map(|(id, f)| drawn_entry(id, f)).collect::<Vec<_>>(),
        "gaps": e.fence_gaps.iter().map(|(id, g)| gap_entry(id, g)).collect::<Vec<_>>(),
        "changed": runs,
    })
}
