//! Changing a town laid out by hand (`hand`): laying it out by hand and back, streets drawn,
//! deleted and set, junctions moved, blocks filled, rows, areas, squares and walls added or taken
//! away. A change gives the records to set or remove (`HandChange`) and what it did; the street
//! graph is kept planar (a change that would cross streets is refused, with the reason). And the
//! town's plan as an editor reads it (`plan_json`).
//!
//! What a change does to the blocks: a block whose face keeps its shape and setbacks keeps its
//! buildings; one whose face changed (or that was asked to change) is laid out anew (its `kept`
//! dropped, its `generation` bumped), with the buildings that carry something (a business, a name
//! or note, people, plots, an edit) pinned to where they stood (`Block::pins`); a face split in two
//! keeps its block on the side of its point and gives the other a new block like it; two faces
//! merged keep the older (lower-id) block. The header's `works` are refreshed with them.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{Value, json};

use super::clip;
use super::geom::*;
use super::hand::{self, Area, Block, BlockFill, Faces, HandRecords, Junction, OpenKind, Pin, Problems, Row, Street, StreetClass, WallLine};
use super::{Building, Layout};
use crate::World;
use crate::core::rng::hash3;
use crate::t0::T0;
use crate::world::{Edits, TownEdit, Wharf};

/// How near (ft) a new street's ends and bends snap to a junction or a street, and a crossing to a
/// junction.
pub const SNAP_FT: f64 = 3.0;

/// A change asked of a hand town (`{"op": …}`; positions world ft, ids the records' own numbers).
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum HandRequest {
    /// Convert the town as laid out now into records (a village is refused).
    LayOutByHand,
    /// Drop the records: the town as generated again (with its ward edit as it was).
    BackToGenerated,
    /// A street along `points` (ends and bends within `SNAP_FT` of a junction or street meet it
    /// there; where it crosses a street both get a junction), split into streets at each junction.
    AddStreet {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        set_l: Option<f64>,
        #[serde(default)]
        set_r: Option<f64>,
        #[serde(default)]
        class: Option<StreetClass>,
    },
    /// A street taken away (the blocks beside it merge; its rows go; junctions left with no street go).
    DeleteStreet { street: u32 },
    /// A street's width, setbacks, class or bridge.
    SetStreet {
        street: u32,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        set_l: Option<f64>,
        #[serde(default)]
        set_r: Option<f64>,
        #[serde(default)]
        class: Option<StreetClass>,
        #[serde(default)]
        bridge: Option<bool>,
    },
    /// Junctions moved (`to`, or `by` from where each stands), all at once where that keeps the
    /// streets apart, else each that does.
    MoveJunctions { moves: Vec<JunctionMove> },
    /// A square where streets meet (`PlazaKind::name`), or "none".
    SetSquare { junction: u32, square: String },
    /// A block's fill, district ("" none), seed (hex), or its lots rolled again.
    SetBlock {
        block: u32,
        #[serde(default)]
        fill: Option<BlockFill>,
        #[serde(default)]
        district: Option<String>,
        #[serde(default)]
        seed: Option<String>,
        #[serde(default)]
        reroll: bool,
    },
    AddRow {
        street: u32,
        side: String,
        depth: f64,
        ward: String,
        #[serde(default)]
        lots: Option<String>,
        #[serde(default)]
        gaps: Option<f64>,
        #[serde(default)]
        seed: Option<String>,
    },
    RemoveRow { row: u32 },
    AddArea { poly: Vec<[f64; 2]>, open: OpenKind },
    RemoveArea { area: u32 },
    AddWall {
        pts: Vec<[f64; 2]>,
        #[serde(default)]
        closed: bool,
        #[serde(default)]
        spacing: Option<f64>,
        #[serde(default)]
        towers: Vec<u32>,
        #[serde(default)]
        gates: Vec<[f64; 2]>,
        #[serde(default)]
        name: Option<String>,
    },
    RemoveWall { wall: u32 },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct JunctionMove {
    pub junction: u32,
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    #[serde(default)]
    pub by: Option<[f64; 2]>,
}

/// What a change sets in the world's edits: per key, the record (`None`: removed).
#[derive(Clone, Debug, Default)]
pub struct HandChange {
    pub streets: BTreeMap<String, Option<Value>>,
    pub wharves: BTreeMap<String, Option<Wharf>>,
    pub towns: BTreeMap<String, Option<TownEdit>>,
}

impl HandChange {
    /// `e` with the change made.
    pub fn apply(&self, e: &mut Edits) {
        for (k, v) in &self.streets {
            match v {
                Some(v) => drop(e.streets.insert(k.clone(), v.clone())),
                None => drop(e.streets.remove(k)),
            }
        }
        for (k, v) in &self.wharves {
            match v {
                Some(v) => drop(e.wharves.insert(k.clone(), v.clone())),
                None => drop(e.wharves.remove(k)),
            }
        }
        for (k, v) in &self.towns {
            match v {
                Some(v) => drop(e.towns.insert(k.clone(), v.clone())),
                None => drop(e.towns.remove(k)),
            }
        }
    }

    /// As JSON: each map `{key: record or null}`, left out when empty.
    pub fn to_json(&self) -> Value {
        let mut out = json!({});
        if !self.streets.is_empty() {
            out["streets"] = json!(self.streets);
        }
        if !self.wharves.is_empty() {
            out["wharves"] = json!(self.wharves);
        }
        if !self.towns.is_empty() {
            out["towns"] = json!(self.towns);
        }
        out
    }
}

/// A point on a 1/64-ft grid (the same number read back from JSON).
fn q(p: P) -> P {
    [crate::core::round(p[0] * 64.0) / 64.0, crate::core::round(p[1] * 64.0) / 64.0]
}

fn next_id<T>(m: &BTreeMap<u32, T>) -> u32 {
    m.keys().next_back().map_or(0, |k| k + 1)
}

/// A street's line, world ft.
fn line_of(recs: &HandRecords, st: &Street) -> Option<Vec<P>> {
    let (a, b) = (recs.junctions.get(&st.a)?, recs.junctions.get(&st.b)?);
    Some(std::iter::once(a.p).chain(st.bends.iter().copied()).chain([b.p]).collect())
}

fn project(p: P, a: P, b: P) -> P {
    let d = sub(b, a);
    let l2 = dot(d, d);
    if l2 <= 1e-12 {
        return a;
    }
    lerp(a, b, (dot(sub(p, a), d) / l2).clamp(0.0, 1.0))
}

fn cross2(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// Where segments a-b and c-d cross (the share along a-b), if they do.
fn crossing(a: P, b: P, c: P, d: P) -> Option<f64> {
    let (r, s) = (sub(b, a), sub(d, c));
    let den = cross2(r, s);
    if den.abs() < 1e-12 {
        return None;
    }
    let (t, u) = (cross2(sub(c, a), s) / den, cross2(sub(c, a), r) / den);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// The nearest junction within `r` ft of `p` (the lowest id of equals).
fn near_junction(recs: &HandRecords, p: P, r: f64) -> Option<u32> {
    recs.junctions.iter().map(|(&id, j)| (dist(j.p, p), id)).filter(|x| x.0 <= r).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|x| x.1)
}

/// The nearest street within `r` ft of `p`, and the nearest point on it.
fn near_street(recs: &HandRecords, p: P, r: f64) -> Option<(u32, P)> {
    let mut best: Option<(f64, u32, P)> = None;
    for (&id, st) in &recs.streets {
        let Some(line) = line_of(recs, st) else { continue };
        for w in line.windows(2) {
            let x = project(p, w[0], w[1]);
            let d = dist(x, p);
            if d <= r && best.is_none_or(|b| d < b.0) {
                best = Some((d, id, x));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

/// Street `e` split at `x` (on it): a junction there, `e` up to it and a new street on.
fn split(recs: &mut HandRecords, e: u32, x: P) -> u32 {
    let st = recs.streets[&e].clone();
    let line = line_of(recs, &st).unwrap_or_default();
    let i = (0..line.len().saturating_sub(1)).min_by(|&i, &k| seg_dist(x, line[i], line[i + 1]).total_cmp(&seg_dist(x, line[k], line[k + 1])).then(i.cmp(&k))).unwrap_or(0);
    let j = next_id(&recs.junctions);
    recs.junctions.insert(j, Junction { p: x, square: None });
    let keep = |p: &P| dist(*p, x) > 0.01;
    let first = Street { b: j, bends: st.bends[..i.min(st.bends.len())].iter().copied().filter(keep).collect(), ..st.clone() };
    let rest = Street { a: j, bends: st.bends[i.min(st.bends.len())..].iter().copied().filter(keep).collect(), ..st };
    let n = next_id(&recs.streets);
    recs.streets.insert(e, first);
    recs.streets.insert(n, rest);
    j
}

/// The setbacks and width a new street takes unless told: the town's usual (the middle of its
/// streets'), else 8 ft each side.
fn usual(recs: &HandRecords) -> (f64, f64) {
    let mut v: Vec<f64> = recs.streets.values().filter(|s| s.class == StreetClass::Street).map(|s| 0.5 * (s.set_l + s.set_r)).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    let set = v.get(v.len() / 2).copied().unwrap_or(8.0);
    (set, 2.0 * set)
}

#[derive(Clone, Copy)]
enum It {
    Node(u32),
    Bend(P),
}

/// `add_street`: the new streets' ids.
fn add_street(recs: &mut HandRecords, points: &[P], proto: &Street) -> Result<Vec<u32>, String> {
    if points.len() < 2 {
        return Err("a street needs two points at least".into());
    }
    let pos = |recs: &HandRecords, it: It| match it {
        It::Node(j) => recs.junctions[&j].p,
        It::Bend(p) => p,
    };
    let mut items: Vec<It> = Vec::new();
    for (k, &p) in points.iter().enumerate() {
        let end = k == 0 || k + 1 == points.len();
        if let Some(j) = near_junction(recs, p, SNAP_FT) {
            items.push(It::Node(j));
        } else if let Some((e, x)) = near_street(recs, p, SNAP_FT) {
            items.push(It::Node(split(recs, e, q(x))));
        } else if end {
            let j = next_id(&recs.junctions);
            recs.junctions.insert(j, Junction { p: q(p), square: None });
            items.push(It::Node(j));
        } else {
            items.push(It::Bend(q(p)));
        }
    }
    // Where it crosses a street, a junction on both.
    let mut out: Vec<It> = vec![items[0]];
    for w in items.windows(2) {
        let (a, b) = (pos(recs, w[0]), pos(recs, w[1]));
        let mut xs: Vec<(f64, P)> = Vec::new();
        for st in recs.streets.values() {
            let Some(line) = line_of(recs, st) else { continue };
            for s in line.windows(2) {
                if let Some(t) = crossing(a, b, s[0], s[1]) {
                    let x = lerp(a, b, t);
                    if dist(x, a) > 0.5 && dist(x, b) > 0.5 {
                        xs.push((t, x));
                    }
                }
            }
        }
        xs.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1[0].total_cmp(&y.1[0])).then(x.1[1].total_cmp(&y.1[1])));
        for (_, x) in xs {
            let node = match near_junction(recs, x, SNAP_FT) {
                Some(j) => j,
                None => match near_street(recs, x, 0.5) {
                    Some((e, _)) => split(recs, e, q(x)),
                    None => continue,
                },
            };
            if !matches!(out.last(), Some(It::Node(n)) if *n == node) {
                out.push(It::Node(node));
            }
        }
        if !matches!((out.last(), w[1]), (Some(It::Node(n)), It::Node(m)) if *n == m) {
            out.push(w[1]);
        }
    }
    // Cut into streets at each junction.
    let It::Node(mut from) = out[0] else { unreachable!("a street starts at a junction") };
    let mut bends: Vec<P> = Vec::new();
    let mut made = Vec::new();
    for it in &out[1..] {
        match *it {
            It::Bend(p) => bends.push(p),
            It::Node(j) => {
                if j == from {
                    return Err("a street must run between two junctions (it comes back to where it began)".into());
                }
                if bends.is_empty() && recs.streets.values().any(|s| s.bends.is_empty() && ((s.a, s.b) == (from, j) || (s.a, s.b) == (j, from))) {
                    return Err(format!("a street joins junctions n{from} and n{j} already"));
                }
                let id = next_id(&recs.streets);
                recs.streets.insert(id, Street { a: from, b: j, bends: std::mem::take(&mut bends), ..proto.clone() });
                made.push(id);
                from = j;
            }
        }
    }
    Ok(made)
}

/// What breaks the graph (a street crossing another, of no length, to no junction) that `after`
/// has and `before` hadn't.
fn broken(before: &Problems, after: &Problems) -> Option<String> {
    const BAD: [&str; 3] = ["crosses another street", "no length", "no such junction"];
    let mut had: BTreeMap<(&str, &str), i32> = BTreeMap::new();
    for (k, why) in before.iter().filter(|p| BAD.contains(&p.1)) {
        *had.entry((k.as_str(), why)).or_default() += 1;
    }
    for (k, why) in after.iter().filter(|p| BAD.contains(&p.1)) {
        let n = had.entry((k.as_str(), why)).or_default();
        if *n == 0 {
            return Some(format!("{k}: {why}"));
        }
        *n -= 1;
    }
    None
}

fn graph_problems(recs: &HandRecords, origin: P, li: &str) -> Problems {
    let mut p = Problems::new();
    hand::Graph::of(recs, origin, li, &mut p);
    p
}

/// A face as the change function compares them: its ring and holes (local ft), its sides (street,
/// left), a hash of its shape and setbacks, its box.
struct FaceInfo {
    ring: Vec<P>,
    holes: Vec<Vec<P>>,
    sides: Vec<(u32, bool)>,
    sig: u64,
    bbox: [f64; 4],
    block: Option<u32>,
}

impl FaceInfo {
    fn holds(&self, p: P) -> bool {
        contains(&self.ring, p) && self.holes.iter().all(|h| !contains(h, p))
    }
}

fn bbox(pts: &[P]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
}

fn boxes_meet(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

fn face_infos(recs: &HandRecords, origin: P, li: &str) -> Vec<FaceInfo> {
    let mut problems = Problems::new();
    let fs = Faces::of(recs, origin, li, &mut problems);
    let mut out = Vec::new();
    for (f, face) in fs.faces.iter().enumerate() {
        let ring = fs.rings[f].clone();
        let set: Vec<f64> = face
            .side
            .iter()
            .map(|&(k, left)| {
                let st = &recs.streets[&fs.graph.segs[k].2];
                if left { st.set_l } else { st.set_r }
            })
            .collect();
        // (Corners on a straight run between equal setbacks don't count: a street split there
        // changes nothing of the block.)
        let n = ring.len();
        let mut corners: Vec<(i64, i64, u64)> = Vec::new();
        for i in 0..n {
            let (a, p, b) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            let straight = seg_dist(p, a, b) < 0.05 && dot(sub(p, a), sub(b, p)) > 0.0;
            if straight && set[(i + n - 1) % n] == set[i] {
                continue;
            }
            corners.push((crate::core::round(p[0] * 8.0) as i64, crate::core::round(p[1] * 8.0) as i64, set[i].to_bits()));
        }
        let start = (0..corners.len()).min_by_key(|&k| (corners[k].0, corners[k].1, k)).unwrap_or(0);
        corners.rotate_left(start);
        let mut h = crate::core::hash::Fnv64::default();
        for c in &corners {
            h.write(&c.0.to_le_bytes());
            h.write(&c.1.to_le_bytes());
            h.write(&c.2.to_le_bytes());
        }
        let holes: Vec<Vec<P>> = fs.holes[f].iter().map(|&o| fs.ring_of(&fs.outers[o])).collect();
        let mut hq: Vec<(i64, i64)> = holes.iter().flatten().map(|p| (crate::core::round(p[0] * 8.0) as i64, crate::core::round(p[1] * 8.0) as i64)).collect();
        hq.sort_unstable();
        for c in &hq {
            h.write(&c.0.to_le_bytes());
            h.write(&c.1.to_le_bytes());
        }
        let mut sides: Vec<(u32, bool)> = face.side.iter().map(|&(k, left)| (fs.graph.segs[k].2, left)).collect();
        sides.sort_unstable();
        sides.dedup();
        out.push(FaceInfo { bbox: bbox(&ring), ring, holes, sides, sig: h.finish(), block: fs.face_block[f] });
    }
    out
}

/// What reconciling the blocks with the faces did.
#[derive(Default)]
struct Blocks {
    /// Laid out anew (their face changed, or asked), added (a face split off), merged away.
    relaid: BTreeSet<u32>,
    added: BTreeSet<u32>,
    removed: BTreeSet<u32>,
    /// The faces (local ft) of those, before and after: where the works may have changed.
    rings: Vec<Vec<P>>,
}

/// The ids of a `Kept`'s buildings.
fn kept_ids(k: &hand::Kept) -> impl Iterator<Item = u32> + '_ {
    k.runs.iter().flat_map(|r| r[1]..r[1] + r[2]).chain(k.full.iter().map(|f| f.i))
}

/// Each face of `after` given its block (as `before`'s faces had them), the blocks whose faces
/// changed (or in `asked`) laid out anew with what carries something in `l` (the town as it was)
/// pinned where it stood.
fn reconcile(world: &World, index: usize, l: &Layout, before: &HandRecords, after: &mut HandRecords, asked: &BTreeSet<u32>) -> Blocks {
    let li = index.to_string();
    let Some(town) = after.town.clone() else { return Blocks::default() };
    let origin = town.origin;
    let fb = face_infos(before, origin, &li);
    let fa = face_infos(after, origin, &li);
    let mut assign: Vec<Option<u32>> = vec![None; fa.len()];
    let mut same = vec![false; fa.len()];
    let mut taken: BTreeSet<u32> = BTreeSet::new();
    let live = |b: Option<u32>, taken: &BTreeSet<u32>, after: &HandRecords| b.filter(|b| after.blocks.contains_key(b) && !taken.contains(b));
    // The same face (shape and setbacks), then the same sides (moved).
    let mut by_sig: BTreeMap<u64, usize> = BTreeMap::new();
    let mut by_sides: BTreeMap<&[(u32, bool)], usize> = BTreeMap::new();
    for (g, f) in fb.iter().enumerate().filter(|(_, f)| f.block.is_some()) {
        by_sig.entry(f.sig).or_insert(g);
        by_sides.entry(&f.sides).or_insert(g);
    }
    for f in 0..fa.len() {
        if let Some(b) = by_sig.get(&fa[f].sig).and_then(|&g| live(fb[g].block, &taken, after)) {
            (assign[f], same[f]) = (Some(b), true);
            taken.insert(b);
        }
    }
    for f in 0..fa.len() {
        if assign[f].is_none()
            && let Some(b) = by_sides.get(fa[f].sides.as_slice()).and_then(|&g| live(fb[g].block, &taken, after))
        {
            assign[f] = Some(b);
            taken.insert(b);
        }
    }
    // Faces new or changed in shape: the blocks of the faces they overlap.
    let overlaps: Vec<Vec<(f64, u32)>> = (0..fa.len())
        .map(|f| {
            if assign[f].is_some() {
                return Vec::new();
            }
            let reg = clip::region(&fa[f].ring);
            let mut v: Vec<(f64, u32)> = fb.iter().filter(|g| g.block.is_some() && boxes_meet(&g.bbox, &fa[f].bbox)).map(|g| (clip::area(&clip::intersection(&reg, &clip::region(&g.ring))), g.block.unwrap_or(0))).filter(|x| x.0 > 1.0).collect();
            v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            v
        })
        .collect();
    for f in 0..fa.len() {
        if assign[f].is_some() {
            continue;
        }
        let mut ids: Vec<u32> = overlaps[f].iter().map(|x| x.1).filter(|b| !taken.contains(b) && after.blocks.get(b).is_some_and(|r| fa[f].holds(sub(r.at, origin)))).collect();
        ids.sort_unstable();
        if let Some(&b) = ids.first() {
            assign[f] = Some(b);
            taken.insert(b);
        }
    }
    for f in 0..fa.len() {
        if assign[f].is_none()
            && let Some(b) = overlaps[f].iter().map(|x| x.1).find(|b| !taken.contains(b) && after.blocks.contains_key(b))
        {
            assign[f] = Some(b);
            taken.insert(b);
        }
    }
    let mut out = Blocks::default();
    let seed = u64::from_str_radix(&town.seed, 16).unwrap_or(0);
    for f in 0..fa.len() {
        if assign[f].is_some() {
            continue;
        }
        let like = overlaps[f].first().and_then(|x| after.blocks.get(&x.1).or_else(|| before.blocks.get(&x.1))).cloned();
        // (Numbered past every block the town has had: a number is never given out twice.)
        let id = next_id(&after.blocks).max(next_id(&before.blocks)).max(town.next_block.unwrap_or(0));
        let (fill, district) = like.map_or((town.fill.clone(), None), |b| (b.fill, b.district));
        after.blocks.insert(id, Block { at: [0.0, 0.0], fill, district, seed: hand::hex(hash3(seed, id as i64, 0xb10c, 1)), generation: 0, pins: Vec::new(), kept: None });
        assign[f] = Some(id);
        taken.insert(id);
        out.added.insert(id);
    }
    // Blocks of faces gone: merged away.
    for g in &fb {
        if let Some(b) = g.block
            && !taken.contains(&b)
            && after.blocks.remove(&b).is_some()
        {
            out.removed.insert(b);
            out.rings.push(g.ring.clone());
        }
    }
    // (The highest number given out, kept in the header once it is more than the blocks show.)
    let high = next_id(&after.blocks).max(next_id(&before.blocks)).max(town.next_block.unwrap_or(0));
    if let Some(t) = after.town.as_mut()
        && (t.next_block.is_some() || high > next_id(&after.blocks))
    {
        t.next_block = Some(high);
    }
    let mut relaid_face: BTreeMap<u32, usize> = BTreeMap::new();
    for f in 0..fa.len() {
        let Some(b) = assign[f] else { continue };
        if same[f] && !asked.contains(&b) {
            continue;
        }
        relaid_face.insert(b, f);
        out.rings.push(fa[f].ring.clone());
        let r = after.blocks.get_mut(&b).expect("a block just assigned");
        if !out.added.contains(&b) {
            r.generation += 1;
        }
        r.kept = None;
        if !fa[f].holds(sub(r.at, origin)) {
            r.at = q(add(hand::inside_point(&fa[f].ring), origin));
        }
    }
    for g in fb.iter().filter(|g| g.block.is_some_and(|b| relaid_face.contains_key(&b))) {
        out.rings.push(g.ring.clone());
    }
    out.relaid = relaid_face.keys().copied().collect();
    // Pins: what carries something on a block laid out anew or merged away, where it stood.
    let marked = marked(world, index);
    let loose: BTreeSet<u32> = town.kept.as_ref().map(|k| kept_ids(k).collect()).unwrap_or_default();
    let per = super::wards::PER_PATCH * super::wards::ROLLS;
    let gone: Vec<&FaceInfo> = fb.iter().filter(|g| g.block.is_some_and(|b| relaid_face.contains_key(&b) || out.removed.contains(&b))).collect();
    let mut pins: BTreeMap<u32, Vec<Pin>> = BTreeMap::new();
    for bd in &l.buildings {
        let row = bd.id >= hand::HAND_ID && (bd.id - hand::HAND_ID) / per >= hand::ROW_BLOCK;
        if row || loose.contains(&bd.id) || (bd.func.is_none() && !marked.contains(&bd.id)) {
            continue;
        }
        let p = hand::inside_point(&bd.poly);
        let lp = sub(p, origin);
        if !gone.iter().any(|g| contains(&g.ring, lp) && g.holds(lp)) {
            continue;
        }
        let Some((&b, _)) = relaid_face.iter().find(|(_, f)| fa[**f].holds(lp)) else { continue };
        pins.entry(b).or_default().push(Pin { p: q(p), id: bd.id, func: bd.func.map(|f| super::catalog::CATALOG[f as usize].key.to_string()), name: bd.func.and(bd.name.clone()) });
    }
    for &b in &out.relaid {
        if let Some(r) = after.blocks.get_mut(&b) {
            r.pins = pins.remove(&b).unwrap_or_default();
        }
    }
    out
}

/// The ids of town `index`'s buildings that carry something said by hand: a rename or note, hidden,
/// an edit, people there, a plot anchored to it.
fn marked(world: &World, index: usize) -> BTreeSet<u32> {
    let e = &world.file.edits;
    let prefix = format!("b:{index}:");
    let id = |k: &str| k.strip_prefix(&prefix).and_then(|r| r.parse::<u32>().ok());
    let mut out: BTreeSet<u32> = BTreeSet::new();
    for k in e.renames.keys().chain(e.notes.keys()).chain(e.hidden.iter()).chain(e.buildings.keys()) {
        out.extend(id(k));
    }
    for n in e.npcs.values() {
        out.extend(n.location.as_ref().and_then(|l| id(&l.id)));
    }
    for p in e.plots.values() {
        out.extend(p.anchors.iter().filter_map(|a| id(a)));
    }
    out
}

/// The header's works refreshed after a change: walls dropped once the wall lines or castles
/// changed, squares at junctions gone or moved dropped, roads in from the country following the
/// junctions they ran to (gone with them), quays and loose parks on changed blocks dropped, bridges
/// no street runs over dropped, district kinds of districts gone dropped.
fn refresh_works(after: &mut HandRecords, before: &HandRecords, rings: &[Vec<P>]) {
    let of = hand::works_of(after);
    let Some(origin) = after.town.as_ref().map(|t| t.origin) else { return };
    let Some(mut works) = after.town.as_mut().and_then(|t| t.works.take()) else { return };
    let w = &mut works;
    if w.of != of && !w.of.is_empty() {
        let keep = hand::Works { roads: std::mem::take(&mut w.roads), corners: std::mem::take(&mut w.corners), plazas: std::mem::take(&mut w.plazas), parks: std::mem::take(&mut w.parks), quarters: std::mem::take(&mut w.quarters), bridges: std::mem::take(&mut w.bridges), ..Default::default() };
        *w = keep;
    }
    w.corners.retain(|c| {
        let pts = hand::unpk(&c.2);
        after.junctions.get(&c.0).is_some_and(|j| pts.first() == Some(&j.p) && j.square.as_deref() == Some(c.1.as_str()))
    });
    // Roads in: an end where a junction stood follows it (or goes with it).
    let mut roads = Vec::new();
    for (class, width, line) in std::mem::take(&mut w.roads) {
        let mut pts = hand::unpk(&line);
        let mut ok = true;
        for p in pts.iter_mut() {
            if let Some((id, _)) = before.junctions.iter().find(|(_, j)| dist(j.p, *p) <= 1.0) {
                match after.junctions.get(id) {
                    Some(j) => *p = add(*p, sub(j.p, before.junctions[id].p)),
                    None => ok = false,
                }
            }
        }
        if ok {
            roads.push((class, width, hand::pk(&pts)));
        }
    }
    w.roads = roads;
    let world_rings: Vec<(Vec<clip::Shape>, [f64; 4])> = rings
        .iter()
        .map(|r| {
            let r: Vec<P> = r.iter().map(|p| add(*p, origin)).collect();
            (clip::region(&r), bbox(&r))
        })
        .collect();
    let touched = |pts: &[P]| {
        let b = bbox(pts);
        let reg = clip::region(pts);
        world_rings.iter().any(|(r, rb)| boxes_meet(rb, &b) && clip::area(&clip::intersection(&reg, r)) > 1.0)
    };
    w.plazas.retain(|x| !touched(&hand::unpk(&x.1)));
    w.parks.retain(|x| !touched(&hand::unpk(&x.1)));
    let lines: Vec<Vec<P>> = after.streets.values().filter_map(|s| line_of(after, s)).chain(w.roads.iter().map(|r| hand::unpk(&r.2))).collect();
    w.bridges.retain(|d| {
        let d = hand::unpk(d);
        lines.iter().any(|l| l.windows(2).any(|s| hand::runs_over(&d, s[0], s[1])))
    });
    let names: BTreeSet<&str> = after.blocks.values().filter_map(|b| b.district.as_deref()).collect();
    w.quarters.retain(|x| names.contains(x.0.as_str()));
    if let Some(t) = after.town.as_mut() {
        t.works = Some(works);
    }
}

/// Every record of a town as stored (key, JSON).
fn keyed(li: &str, r: &HandRecords) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if let Some(t) = &r.town {
        out.insert(li.to_string(), v(t));
    }
    for (id, x) in &r.junctions {
        out.insert(format!("{li}/n{id}"), v(x));
    }
    for (id, x) in &r.streets {
        out.insert(format!("{li}/e{id}"), v(x));
    }
    for (id, x) in &r.blocks {
        out.insert(format!("{li}/k{id}"), v(x));
    }
    for (id, x) in &r.rows {
        out.insert(format!("{li}/r{id}"), v(x));
    }
    for (id, x) in &r.areas {
        out.insert(format!("{li}/a{id}"), v(x));
    }
    for (id, x) in &r.walls {
        out.insert(format!("{li}/m{id}"), v(x));
    }
    out
}

fn v<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).unwrap_or(Value::Null)
}

/// A world with `ch` made (the town laid out by it).
fn changed_world(world: &World, ch: &HandChange) -> Result<World, String> {
    let mut file = world.file.clone();
    ch.apply(&mut file.edits);
    World::new(file)
}

/// What carries something in `before` that `after` hasn't any more (by id): `[{id, name, what}]`.
fn lost(world: &World, index: usize, before: &Layout, after: &Layout) -> Vec<Value> {
    let marked = marked(world, index);
    let now: BTreeSet<u32> = after.buildings.iter().map(|b| b.id).collect();
    let e = &world.file.edits;
    before
        .buildings
        .iter()
        .filter(|b| !now.contains(&b.id) && (b.func.is_some() || marked.contains(&b.id)))
        .map(|b: &Building| {
            let k = format!("b:{index}:{}", b.id);
            let name = e.renames.get(&k).cloned().or_else(|| b.name.clone()).unwrap_or_else(|| b.label().to_string());
            let mut what: Vec<&str> = Vec::new();
            if b.func.is_some() {
                what.push("business");
            }
            for (has, w) in [(e.renames.contains_key(&k), "rename"), (e.notes.contains_key(&k), "note"), (e.buildings.contains_key(&k), "edit"), (e.npcs.values().any(|n| n.location.as_ref().is_some_and(|l| l.id == k)), "people"), (e.plots.values().any(|p| p.anchors.contains(&k)), "plot")] {
                if has {
                    what.push(w);
                }
            }
            json!({ "id": k, "name": name, "what": what })
        })
        .collect()
}

/// Town `index` changed as asked: what to set in the world's edits, and what the change did
/// (`{op, records: {set, removed}, blocks: {relaid, added, removed}, rects, lost, problems, …}`).
pub fn change(world: &World, t0: &T0, index: usize, req: &HandRequest) -> Result<(HandChange, Value), String> {
    let li = index.to_string();
    let streets = &world.file.edits.streets;
    let before_l = super::base_layout(world, t0, index);
    let mut ch = HandChange::default();
    let mut report = json!({});
    let tkey = super::wards::key(index);
    match req {
        HandRequest::LayOutByHand => {
            let recs = hand::convert(world, t0, index)?;
            let lo = format!("{li}/");
            for (k, _) in streets.range(lo.clone()..).take_while(|(k, _)| k.starts_with(&lo)) {
                ch.streets.insert(k.clone(), None);
            }
            for (k, v) in recs {
                ch.streets.insert(k, Some(v));
            }
            for (k, w) in hand::wharves(world, t0, index) {
                ch.wharves.insert(k, Some(w));
            }
            if world.file.edits.towns.contains_key(&tkey) {
                ch.towns.insert(tkey, None);
            }
        }
        HandRequest::BackToGenerated => {
            let recs = HandRecords::of(streets, index);
            let town = recs.town.ok_or("not laid out by hand")?;
            ch.streets.insert(li.clone(), None);
            let lo = format!("{li}/");
            for (k, _) in streets.range(lo.clone()..).take_while(|(k, _)| k.starts_with(&lo)) {
                ch.streets.insert(k.clone(), None);
            }
            let wp = format!("q:h{index}-");
            for (k, _) in world.file.edits.wharves.range(wp.clone()..).take_while(|(k, _)| k.starts_with(&wp)) {
                ch.wharves.insert(k.clone(), None);
            }
            match town.was {
                Some(e) => drop(ch.towns.insert(tkey, Some(e))),
                None if world.file.edits.towns.contains_key(&tkey) => drop(ch.towns.insert(tkey, None)),
                None => {}
            }
        }
        _ => {
            let before = HandRecords::of(streets, index);
            let town = before.town.clone().ok_or("not laid out by hand: lay_out_by_hand first")?;
            let origin = town.origin;
            let mut after = before.clone();
            let mut asked: BTreeSet<u32> = BTreeSet::new();
            let pt = |p: &[f64; 2]| q(*p);
            match req {
                HandRequest::AddStreet { points, width, set_l, set_r, class } => {
                    let (set, w) = usual(&before);
                    let (sl, sr) = (set_l.unwrap_or(set), set_r.unwrap_or(set));
                    let proto = Street { a: 0, b: 0, bends: Vec::new(), width: width.unwrap_or(if set_l.is_some() || set_r.is_some() { sl + sr } else { w }), set_l: sl, set_r: sr, class: class.unwrap_or_default(), bridge: false };
                    if proto.width < 0.0 || proto.set_l < 0.0 || proto.set_r < 0.0 {
                        return Err("width and setbacks are 0 ft or more".into());
                    }
                    let pts: Vec<P> = points.iter().map(pt).collect();
                    let made = add_street(&mut after, &pts, &proto)?;
                    report["streets_added"] = json!(made);
                }
                HandRequest::DeleteStreet { street } => {
                    let st = after.streets.remove(street).ok_or_else(|| format!("street e{street}: no such street"))?;
                    let rows: Vec<u32> = after.rows.iter().filter(|(_, r)| r.street == *street).map(|(k, _)| *k).collect();
                    for r in &rows {
                        after.rows.remove(r);
                    }
                    for j in [st.a, st.b] {
                        if !after.streets.values().any(|s| s.a == j || s.b == j) {
                            after.junctions.remove(&j);
                        }
                    }
                    if !rows.is_empty() {
                        report["rows_removed"] = json!(rows);
                    }
                }
                HandRequest::SetStreet { street, width, set_l, set_r, class, bridge } => {
                    let st = after.streets.get_mut(street).ok_or_else(|| format!("street e{street}: no such street"))?;
                    if [width, set_l, set_r].iter().any(|v| v.is_some_and(|v| !(0.0..=200.0).contains(&v))) {
                        return Err("width and setbacks are 0 to 200 ft".into());
                    }
                    st.width = width.unwrap_or(st.width);
                    st.set_l = set_l.unwrap_or(st.set_l);
                    st.set_r = set_r.unwrap_or(st.set_r);
                    st.class = class.unwrap_or(st.class);
                    st.bridge = bridge.unwrap_or(st.bridge);
                }
                HandRequest::MoveJunctions { moves } => {
                    let mut to: Vec<(u32, P)> = Vec::new();
                    for m in moves {
                        let j = after.junctions.get(&m.junction).ok_or_else(|| format!("junction n{}: no such junction", m.junction))?;
                        let p = match (m.to, m.by) {
                            (Some(t), None) => q(t),
                            (None, Some(b)) => q(add(j.p, b)),
                            _ => return Err(format!("junction n{}: give to or by", m.junction)),
                        };
                        to.push((m.junction, p));
                    }
                    let base = graph_problems(&before, origin, &li);
                    let mut all = after.clone();
                    for (j, p) in &to {
                        all.junctions.get_mut(j).expect("checked").p = *p;
                    }
                    if broken(&base, &graph_problems(&all, origin, &li)).is_none() {
                        after = all;
                    } else {
                        // Each that keeps the streets apart, in the order asked.
                        let mut stayed = Vec::new();
                        for (j, p) in &to {
                            let mut one = after.clone();
                            one.junctions.get_mut(j).expect("checked").p = *p;
                            match broken(&base, &graph_problems(&one, origin, &li)) {
                                None => after = one,
                                Some(why) => stayed.push(json!({ "junction": j, "why": why })),
                            }
                        }
                        if stayed.len() == to.len() && to.len() == 1 {
                            return Err(format!("junction n{} can't go there: {}", to[0].0, stayed[0]["why"].as_str().unwrap_or("")));
                        }
                        report["stayed"] = json!(stayed);
                    }
                }
                HandRequest::SetSquare { junction, square } => {
                    let j = after.junctions.get_mut(junction).ok_or_else(|| format!("junction n{junction}: no such junction"))?;
                    j.square = match square.as_str() {
                        "none" => None,
                        k if hand::plaza_kind(k).is_some() => Some(k.to_string()),
                        k => return Err(format!("no square of kind {k}")),
                    };
                    // (The blocks round it give it room.)
                    let lp = sub(j.p, origin);
                    for f in face_infos(&before, origin, &li) {
                        if f.ring.iter().any(|p| dist(*p, lp) < 0.5) {
                            asked.extend(f.block);
                        }
                    }
                }
                HandRequest::SetBlock { block, fill, district, seed, reroll } => {
                    let b = after.blocks.get_mut(block).ok_or_else(|| format!("block k{block}: no such block"))?;
                    if let Some(f) = fill {
                        if let BlockFill::Generated { ward, lots, .. } = f {
                            if hand::ward_of(ward).is_none() {
                                return Err(format!("no ward {ward}"));
                            }
                            if lots.as_deref().is_some_and(|l| !crate::world::LOT_SIZES.contains(&l)) {
                                return Err(format!("lots: one of {}", crate::world::LOT_SIZES.join(", ")));
                            }
                        }
                        b.fill = f.clone();
                        asked.insert(*block);
                    }
                    if let Some(s) = seed {
                        u64::from_str_radix(s, 16).map_err(|_| "seed: hex digits".to_string())?;
                        b.seed = s.clone();
                        asked.insert(*block);
                    }
                    if *reroll {
                        match &mut b.fill {
                            BlockFill::Generated { reroll, .. } => *reroll += 1,
                            _ => return Err(format!("block k{block}: only lots are rolled again")),
                        }
                        asked.insert(*block);
                    }
                    if let Some(d) = district {
                        b.district = (!d.is_empty()).then(|| d.clone());
                    }
                }
                HandRequest::AddRow { street, side, depth, ward, lots, gaps, seed } => {
                    if !after.streets.contains_key(street) {
                        return Err(format!("street e{street}: no such street"));
                    }
                    if side != "l" && side != "r" {
                        return Err("side: l or r".into());
                    }
                    if hand::ward_of(ward).is_none() {
                        return Err(format!("no ward {ward}"));
                    }
                    if !(10.0..=200.0).contains(depth) {
                        return Err("depth: 10 to 200 ft".into());
                    }
                    let id = next_id(&after.rows);
                    if id >= hand::MAX_ROWS {
                        return Err(format!("no more than {} rows", hand::MAX_ROWS));
                    }
                    let seed = seed.clone().unwrap_or_else(|| hand::hex(hash3(u64::from_str_radix(&town.seed, 16).unwrap_or(0), id as i64, 0x7a0, 1)));
                    after.rows.insert(id, Row { street: *street, side: side.clone(), depth: *depth, ward: ward.clone(), lots: lots.clone(), gaps: gaps.unwrap_or(0.08), seed });
                    report["row"] = json!(id);
                }
                HandRequest::RemoveRow { row } => {
                    after.rows.remove(row).ok_or_else(|| format!("row r{row}: no such row"))?;
                }
                HandRequest::AddArea { poly, open } => {
                    if poly.len() < 3 {
                        return Err("an area needs three corners at least".into());
                    }
                    let poly: Vec<P> = poly.iter().map(pt).collect();
                    let id = next_id(&after.areas);
                    asked.extend(blocks_under(&before, origin, &li, &poly));
                    after.areas.insert(id, Area { poly, open: *open });
                    report["area"] = json!(id);
                }
                HandRequest::RemoveArea { area } => {
                    let a = after.areas.remove(area).ok_or_else(|| format!("area a{area}: no such area"))?;
                    asked.extend(blocks_under(&before, origin, &li, &a.poly));
                }
                HandRequest::AddWall { pts, closed, spacing, towers, gates, name } => {
                    if pts.len() < 2 {
                        return Err("a wall needs two points at least".into());
                    }
                    let id = next_id(&after.walls);
                    let spacing = spacing.unwrap_or_else(|| before.walls.values().next().map_or(90.0, |w| w.spacing));
                    after.walls.insert(id, WallLine { pts: pts.iter().map(pt).collect(), closed: *closed, spacing, towers: towers.clone(), gates: gates.iter().map(pt).collect(), name: name.clone() });
                    report["wall"] = json!(id);
                }
                HandRequest::RemoveWall { wall } => {
                    after.walls.remove(wall).ok_or_else(|| format!("wall m{wall}: no such wall"))?;
                }
                HandRequest::LayOutByHand | HandRequest::BackToGenerated => unreachable!("handled above"),
            }
            if let Some(why) = broken(&graph_problems(&before, origin, &li), &graph_problems(&after, origin, &li)) {
                return Err(format!("that would break the streets: {why}"));
            }
            let blocks = reconcile(world, index, &before_l, &before, &mut after, &asked);
            if after.blocks.keys().next_back().is_some_and(|&k| k >= hand::ROW_BLOCK) {
                return Err(format!("no more than {} blocks in a town (a block's number is never used again)", hand::ROW_BLOCK));
            }
            refresh_works(&mut after, &before, &blocks.rings);
            let (kb, ka) = (keyed(&li, &before), keyed(&li, &after));
            for (k, v) in &ka {
                if kb.get(k) != Some(v) {
                    ch.streets.insert(k.clone(), Some(v.clone()));
                }
            }
            for k in kb.keys().filter(|k| !ka.contains_key(*k)) {
                ch.streets.insert(k.clone(), None);
            }
            report["blocks"] = json!({ "relaid": blocks.relaid, "added": blocks.added, "removed": blocks.removed });
        }
    }
    let w2 = changed_world(world, &ch)?;
    let (after_l, problems) = match hand::build_kept(&w2, t0, index) {
        Some((l, p)) => (l, p),
        None => (super::base_layout(&w2, t0, index), Vec::new()),
    };
    // No building left without an id of its own: refused (a block or row with more buildings
    // than its range of ids holds, not so before).
    const NO_IDS: [&str; 3] = ["more buildings than a block has ids for", "more buildings than a row has ids for", "block number too high for its ids"];
    let old = hand::build_kept(world, t0, index).map(|(_, b)| b).unwrap_or_default();
    if let Some(p) = problems.iter().find(|p| NO_IDS.contains(&p.1) && !old.contains(p)) {
        return Err(format!("{}: {} (make it smaller)", p.0, p.1));
    }
    // Its population counted from its homes as they now stand, and the size class it makes.
    if let Some(h) = w2.file.edits.streets.get(&li) {
        let n = HandRecords::of(&w2.file.edits.streets, index).town.map_or_else(|| hand::people_of(&after_l), |t| hand::count_people(&t, &after_l));
        if h.get("people").and_then(Value::as_u64) != Some(n as u64) {
            let mut h = h.clone();
            h["people"] = json!(n);
            ch.streets.insert(li.clone(), Some(h));
        }
        report["population"] = json!(n);
        report["class"] = json!(hand::tier_of_people(n).name());
    }
    let set: Vec<&String> = ch.streets.iter().filter(|x| x.1.is_some()).map(|x| x.0).collect();
    let removed: Vec<&String> = ch.streets.iter().filter(|x| x.1.is_none()).map(|x| x.0).collect();
    let few = |v: &[&String]| if v.len() > 40 { json!({ "count": v.len() }) } else { json!(v) };
    report["op"] = json!(op_name(req));
    report["records"] = json!({ "set": few(&set), "removed": few(&removed) });
    report["buildings"] = json!({ "before": before_l.buildings.len(), "after": after_l.buildings.len() });
    report["rects"] = json!(super::wards::changed_rects(&before_l, &after_l).iter().map(|r| r.map(crate::core::round)).collect::<Vec<_>>());
    let lost = lost(world, index, &before_l, &after_l);
    if !lost.is_empty() {
        report["lost"] = json!(lost);
    }
    if !problems.is_empty() {
        report["problems"] = json!(problems.iter().take(40).map(|(k, w)| json!({ "key": k, "why": w })).collect::<Vec<_>>());
    }
    if matches!(req, HandRequest::LayOutByHand) {
        report["size"] = json!(ch.streets.iter().map(|(k, v)| k.len() + 4 + v.as_ref().map_or(0, |v| v.to_string().len())).sum::<usize>());
    }
    Ok((ch, report))
}

fn op_name(r: &HandRequest) -> &'static str {
    match r {
        HandRequest::LayOutByHand => "lay_out_by_hand",
        HandRequest::BackToGenerated => "back_to_generated",
        HandRequest::AddStreet { .. } => "add_street",
        HandRequest::DeleteStreet { .. } => "delete_street",
        HandRequest::SetStreet { .. } => "set_street",
        HandRequest::MoveJunctions { .. } => "move_junctions",
        HandRequest::SetSquare { .. } => "set_square",
        HandRequest::SetBlock { .. } => "set_block",
        HandRequest::AddRow { .. } => "add_row",
        HandRequest::RemoveRow { .. } => "remove_row",
        HandRequest::AddArea { .. } => "add_area",
        HandRequest::RemoveArea { .. } => "remove_area",
        HandRequest::AddWall { .. } => "add_wall",
        HandRequest::RemoveWall { .. } => "remove_wall",
    }
}

/// The blocks whose faces `poly` (world ft) lies over.
fn blocks_under(recs: &HandRecords, origin: P, li: &str, poly: &[P]) -> Vec<u32> {
    let local: Vec<P> = poly.iter().map(|p| sub(*p, origin)).collect();
    let (reg, b) = (clip::region(&local), bbox(&local));
    face_infos(recs, origin, li).into_iter().filter(|f| boxes_meet(&f.bbox, &b) && clip::area(&clip::intersection(&reg, &clip::region(&f.ring))) > 1.0).filter_map(|f| f.block).collect()
}

/// `change` as JSON: `{change: {streets, wharves, towns}, report}` or `{error}` (for the app's
/// worker and the agent server).
pub fn change_json(world: &World, t0: &T0, index: usize, req: &str) -> String {
    let r = serde_json::from_str::<HandRequest>(req).map_err(|e| e.to_string()).and_then(|req| change(world, t0, index, &req));
    match r {
        Ok((ch, report)) => json!({ "change": ch.to_json(), "report": report }),
        Err(e) => json!({ "error": e }),
    }
    .to_string()
}

/// A hand town's plan as an editor reads it: its header, junctions (with how many streets meet
/// there), streets, blocks (fill, district, kept or laid out anew, the streets round it), rows,
/// areas, walls and what could not be laid out.
pub fn plan_json(world: &World, t0: &T0, index: usize) -> Result<Value, String> {
    let recs = HandRecords::of(&world.file.edits.streets, index);
    let town = recs.town.as_ref().ok_or("not laid out by hand")?;
    let li = index.to_string();
    let mut problems = Problems::new();
    let fs = Faces::of(&recs, town.origin, &li, &mut problems);
    let mut round: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (f, face) in fs.faces.iter().enumerate() {
        let Some(b) = fs.face_block[f] else { continue };
        let mut ids: Vec<u32> = Vec::new();
        for &(k, _) in &face.side {
            let id = fs.graph.segs[k].2;
            if ids.last() != Some(&id) && ids.first() != Some(&id) {
                ids.push(id);
            }
        }
        round.insert(b, ids);
    }
    let built = hand::build_kept(world, t0, index).map(|x| x.1).unwrap_or(problems);
    let degree = |j: u32| recs.streets.values().filter(|s| s.a == j || s.b == j).count();
    Ok(json!({
        "layout": index,
        "from": town.from,
        "origin": town.origin,
        "tier": town.tier,
        "fill": town.fill,
        "works": town.works.is_some(),
        "ward_edit": town.was.is_some(),
        "snap_ft": SNAP_FT,
        "junctions": recs.junctions.iter().map(|(id, j)| json!({ "id": id, "p": j.p, "square": j.square, "streets": degree(*id) })).collect::<Vec<_>>(),
        "streets": recs.streets.iter().map(|(id, s)| {
            let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
            v["id"] = json!(id);
            v["class"] = json!(s.class);
            v
        }).collect::<Vec<_>>(),
        "blocks": recs.blocks.iter().map(|(id, b)| json!({ "id": id, "at": b.at, "fill": b.fill, "district": b.district, "seed": b.seed, "gen": b.generation, "kept": b.kept.is_some(), "pins": b.pins.len(), "streets": round.get(id) })).collect::<Vec<_>>(),
        "rows": recs.rows.iter().map(|(id, r)| json!({ "id": id, "street": r.street, "side": r.side, "depth": r.depth, "ward": r.ward, "lots": r.lots, "gaps": r.gaps })).collect::<Vec<_>>(),
        "areas": recs.areas.iter().map(|(id, a)| json!({ "id": id, "poly": a.poly, "open": a.open })).collect::<Vec<_>>(),
        "walls": recs.walls.iter().map(|(id, w)| json!({ "id": id, "pts": w.pts, "closed": w.closed, "spacing": w.spacing, "gates": w.gates, "name": w.name })).collect::<Vec<_>>(),
        "problems": built.iter().map(|(k, w)| json!({ "key": k, "why": w })).collect::<Vec<_>>(),
    }))
}
