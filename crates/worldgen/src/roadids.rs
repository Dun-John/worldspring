//! Road stretches as places (`rd:<8 hex>`) and the names given to them; crossings put down by
//! hand (`v:<id>`) as named places.
//!
//! A stretch is one of `t0.roads.roads` (a road between settlements and junctions, as
//! `list_roads` lists them). Its id is a pure function of its two ends (rounded to 10 ft,
//! either way round), so it stays the same across every edit that leaves the roads as
//! generated (renames, sites, buildings, objects, crossings, town edits); a remake, or a sketch
//! change that moves one of its ends, gives it another. Stretches with the same ends are told
//! apart by their length and middle, never by their place in the list. Names live in
//! `Edits.renames` and notes in `Edits.notes` under these ids, like any feature's.

use serde_json::{Value, json};

use crate::World;
use crate::agent::{feature_of_layout, name_of};
use crate::core::hash::FastSet;
use crate::core::rng::hash2;
use crate::lod::roads::RoadCurve;
use crate::t0::T0;
use crate::t0::features::Feature;
use crate::town::{self, geom};
use crate::world::{Crossing, CrossingKind};

type P = [f64; 2];

const MI: f64 = 5280.0;
/// How finely a stretch's ends are told apart (ft).
const END_FT: f64 = 10.0;
/// The most points a stretch's line is given as.
const LINE_POINTS: usize = 40;

fn mi(ft: f64) -> f64 {
    crate::core::round(ft / MI * 10.0) / 10.0
}

fn q(p: P) -> (i64, i64) {
    (crate::core::round(p[0] / END_FT) as i64, crate::core::round(p[1] / END_FT) as i64)
}

/// A stretch's id before stretches with the same ends are told apart: its two ends, either way
/// round, hashed (32 bits).
pub fn ends_key(pts: &[P]) -> u32 {
    let (a, b) = (q(pts.first().copied().unwrap_or_default()), q(pts.last().copied().unwrap_or_default()));
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    (hash2(hash2(0x52D1_D5, a.0, a.1), b.0, b.1) & 0xffff_ffff) as u32
}

/// Each stretch's id (`rd:<8 hex>`), in `t0.roads.roads` order. Stretches with the same key
/// (the same ends) are ordered by length and middle point and the later ones re-hashed, so the
/// ids depend on the stretches there, not on the order they are listed in.
pub fn road_ids(t0: &T0) -> Vec<String> {
    let rs = &t0.roads.roads;
    let tie = |r: &RoadCurve| {
        let n = r.pts.len();
        let m = if n == 0 { (0, 0) } else { q(r.pts[n / 2]).min(q(r.pts[(n - 1) / 2])) };
        (crate::core::round(*r.s.last().unwrap_or(&0.0)) as i64, m)
    };
    let mut order: Vec<(u32, (i64, (i64, i64)), usize)> = rs.iter().enumerate().map(|(i, r)| (ends_key(&r.pts), tie(r), i)).collect();
    order.sort_unstable();
    let mut taken: FastSet<u32> = FastSet::default();
    let mut out = vec![String::new(); rs.len()];
    for (key, _, i) in order {
        let mut id = key;
        let mut k = 1;
        while !taken.insert(id) {
            id = (hash2(key as u64, k, 0x7D) & 0xffff_ffff) as u32;
            k += 1;
        }
        out[i] = format!("rd:{id:08x}");
    }
    out
}

/// The stretch an `rd:` id names (its index in `t0.roads.roads`).
pub fn road_index(t0: &T0, id: &str) -> Option<usize> {
    if !id.starts_with("rd:") {
        return None;
    }
    road_ids(t0).iter().position(|r| r == id)
}

pub fn class_name(c: crate::t0::roads::RoadClass) -> &'static str {
    ["king's road", "road", "track"][c as usize]
}

/// The point halfway along a stretch.
pub fn midpoint(r: &RoadCurve) -> P {
    let half = 0.5 * r.s.last().copied().unwrap_or(0.0);
    let k = r.s.iter().position(|&s| s >= half).unwrap_or(0);
    if k == 0 {
        return r.pts.first().copied().unwrap_or_default();
    }
    let (a, b) = (r.pts[k - 1], r.pts[k]);
    let t = (half - r.s[k - 1]) / (r.s[k] - r.s[k - 1]).max(1e-9);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// A stretch's line in at most `LINE_POINTS` points (evenly along it, both ends kept), whole ft.
pub fn line(r: &RoadCurve) -> Vec<[f64; 2]> {
    let n = r.pts.len();
    let pick: Vec<usize> = if n <= LINE_POINTS {
        (0..n).collect()
    } else {
        let total = r.s.last().copied().unwrap_or(0.0);
        let mut v: Vec<usize> = (0..LINE_POINTS).map(|j| r.s.partition_point(|&s| s < total * j as f64 / (LINE_POINTS - 1) as f64).min(n - 1)).collect();
        v[LINE_POINTS - 1] = n - 1;
        v.dedup();
        v
    };
    pick.into_iter().map(|i| [crate::core::round(r.pts[i][0]), crate::core::round(r.pts[i][1])]).collect()
}

/// The settlement a road's end reaches (as `list_roads` and routes decide).
pub fn town_at(t0: &T0, p: P) -> Option<usize> {
    t0.settlements
        .iter()
        .enumerate()
        .filter(|(_, s)| geom::dist([s.x, s.y], p) < town::reach(s) + 3_000.0)
        .min_by(|a, b| geom::dist([a.1.x, a.1.y], p).total_cmp(&geom::dist([b.1.x, b.1.y], p)))
        .map(|(i, _)| i)
}

/// A road's end: the settlement it reaches ({id, name}), else its point.
pub fn end_json(world: &World, t0: &T0, p: P) -> Value {
    match town_at(t0, p).and_then(|i| feature_of_layout(world, t0, i)) {
        Some(id) => json!({ "id": id, "name": name_of(world, t0, &id) }),
        None => json!({ "x_ft": crate::core::round(p[0]), "y_ft": crate::core::round(p[1]) }),
    }
}

/// The feature (index in `fs`, `agent::features` order) of the road drawn as sketch stroke
/// `stroke`, if it was named: by its name, else (renamed since) the road feature nearest its
/// first piece.
pub fn drawn_road(world: &World, t0: &T0, fs: &[Feature], stroke: u32) -> Option<usize> {
    let name = world.file.sketch.strokes.get(stroke as usize)?.name.as_deref()?.trim().to_string();
    if let Some(i) = fs.iter().position(|f| f.kind == "road" && f.name == name) {
        return Some(i);
    }
    let first = *t0.roads.roads.iter().find(|r| r.stroke == Some(stroke))?.pts.first()?;
    let shapes = crate::agent::shapes(world, t0);
    (0..fs.len().min(shapes.len())).filter(|&i| fs[i].kind == "road").min_by(|&a, &b| shapes[a].distance(first).0.total_cmp(&shapes[b].distance(first).0))
}

/// A stretch's name: given to it (`rename_feature rd:…`), else the drawn road's it belongs to.
pub fn stretch_name(world: &World, t0: &T0, fs: &[Feature], id: &str, r: &RoadCurve) -> Option<String> {
    if let Some(n) = world.file.edits.renames.get(id) {
        return Some(n.clone());
    }
    r.stroke.and_then(|s| drawn_road(world, t0, fs, s)).map(|i| fs[i].name.clone())
}

/// A stretch's details for `get_feature`.
/// `ids` is `road_ids(t0)` and `fs` the world's features (the caller computes them once).
pub fn road_json(world: &World, t0: &T0, ids: &[String], fs: &[Feature], id: &str) -> Option<Value> {
    if !id.starts_with("rd:") {
        return None;
    }
    let i = ids.iter().position(|r| r == id)?;
    let r = &t0.roads.roads[i];
    let m = midpoint(r);
    let mut v = json!({
        "id": id,
        "kind": "road",
        "name": stretch_name(world, t0, fs, id, r),
        "class": class_name(r.class),
        "length_mi": mi(*r.s.last().unwrap_or(&0.0)),
        "from": end_json(world, t0, r.pts.first().copied().unwrap_or_default()),
        "to": end_json(world, t0, r.pts.last().copied().unwrap_or_default()),
        "x_ft": crate::core::round(m[0]),
        "y_ft": crate::core::round(m[1]),
        "line": line(r),
    });
    if world.file.edits.renames.contains_key(id) {
        v["renamed"] = json!(true);
    }
    if let Some(f) = r.stroke.and_then(|s| drawn_road(world, t0, fs, s)) {
        v["drawn_road"] = json!({ "id": fs[f].id, "name": fs[f].name });
    }
    Some(v)
}

/// A road someone named, as the map letters it: the stretches given one name that meet end to
/// end (at a junction, or a settlement both reach), in order along it.
pub struct NamedRoad {
    pub ids: Vec<String>,
    /// The stretches' indices in `t0.roads.roads`.
    pub idx: Vec<usize>,
    pub name: String,
    /// The whole line (ft), the stretches turned to run on from each other (straight across
    /// a settlement between two).
    pub pts: Vec<[f64; 2]>,
}

/// The roads someone named (`rename_feature rd:…`, stretches still there), by name; hidden
/// stretches left out with `skip_hidden`. What the map letters (only roads someone named).
pub fn named_roads(world: &World, t0: &T0, skip_hidden: bool) -> Vec<NamedRoad> {
    let e = &world.file.edits;
    if !e.renames.keys().any(|k| k.starts_with("rd:")) {
        return Vec::new();
    }
    let ids = road_ids(t0);
    let mut by_name: std::collections::BTreeMap<&String, Vec<usize>> = Default::default();
    for (i, id) in ids.iter().enumerate() {
        if let Some(n) = e.renames.get(id).filter(|n| !n.trim().is_empty() && !(skip_hidden && e.hidden.contains(id)) && t0.roads.roads[i].pts.len() >= 2) {
            by_name.entry(n).or_default().push(i);
        }
    }
    let rs = &t0.roads.roads;
    let mut out = Vec::new();
    for (name, set) in by_name {
        for chain in walk(t0, &set) {
            let mut pts: Vec<P> = Vec::new();
            for &(k, fwd) in &chain {
                let r = &rs[set[k]].pts;
                if fwd {
                    pts.extend(r.iter().copied());
                } else {
                    pts.extend(r.iter().rev().copied());
                }
            }
            out.push(NamedRoad { ids: chain.iter().map(|&(k, _)| ids[set[k]].clone()).collect(), idx: chain.iter().map(|&(k, _)| set[k]).collect(), name: name.clone(), pts });
        }
    }
    out
}

/// Stretches (`set`: indices in `t0.roads.roads`) walked end to end: each run of stretches
/// meeting at a node (a settlement, else a point to 400 ft), from an end met once where there
/// is one, as (index in `set`, walked as listed).
fn walk(t0: &T0, set: &[usize]) -> Vec<Vec<(usize, bool)>> {
    let rs = &t0.roads.roads;
    let node = |p: P| match town_at(t0, p) {
        Some(s) => (i64::MIN, s as i64),
        None => (crate::core::round(p[0] / 400.0) as i64, crate::core::round(p[1] / 400.0) as i64),
    };
    let ends: Vec<((i64, i64), (i64, i64))> = set.iter().map(|&i| (node(rs[i].pts[0]), node(*rs[i].pts.last().unwrap_or(&rs[i].pts[0])))).collect();
    let degree = |n: (i64, i64)| ends.iter().filter(|e| e.0 == n || e.1 == n).count();
    let mut used = vec![false; set.len()];
    let mut chains: Vec<Vec<(usize, bool)>> = Vec::new();
    let mut n = 0;
    while n < set.len() {
        let starts: Vec<(i64, i64)> = (0..set.len()).filter(|&k| !used[k]).flat_map(|k| [ends[k].0, ends[k].1]).collect();
        let Some(mut at) = starts.iter().copied().filter(|&n| degree(n) == 1).min().or_else(|| starts.iter().copied().min()) else { break };
        let mut chain = Vec::new();
        while let Some(k) = (0..set.len()).find(|&k| !used[k] && (ends[k].0 == at || ends[k].1 == at)) {
            used[k] = true;
            let fwd = ends[k].0 == at;
            chain.push((k, fwd));
            at = if fwd { ends[k].1 } else { ends[k].0 };
        }
        n += chain.len();
        chains.push(chain);
    }
    chains
}

/// A line with the points it can do without dropped (Douglas–Peucker to `tol` ft), whole ft.
pub fn simplify(pts: &[P], tol: f64) -> Vec<P> {
    let n = pts.len();
    if n < 3 {
        return pts.iter().map(|p| [crate::core::round(p[0]), crate::core::round(p[1])]).collect();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (pts[a], pts[b]);
        let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
        let len = crate::core::sqrt(dx * dx + dy * dy).max(1e-9);
        let (mut best, mut at) = (tol, 0);
        for (k, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = if len < 1e-6 { geom::dist(*p, pa) } else { crate::core::fabs((p[0] - pa[0]) * dy - (p[1] - pa[1]) * dx) / len };
            if d > best {
                best = d;
                at = k;
            }
        }
        if at > 0 {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    (0..n).filter(|&k| keep[k]).map(|k| [crate::core::round(pts[k][0]), crate::core::round(pts[k][1])]).collect()
}

fn line_len(pts: &[P]) -> f64 {
    pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum()
}

/// For the app (`Ctx::named_json`): the named roads to letter (hidden stretches left out; line
/// to 15 ft), every named road for the lists (`ids`, its box, length, `generated`: its name
/// before renames, a drawn road's or else its class) and the crossings put down
/// by hand (name given, else as generated; `generated`; middle; length).
pub fn named_json(world: &World, t0: &T0) -> Value {
    let named = named_roads(world, t0, false);
    // Hidden stretches are left out of the lettering (and the rest re-chained); the same walk serves when none is hidden.
    let hidden = named.iter().any(|r| r.ids.iter().any(|id| world.file.edits.hidden.contains(id)));
    let lettered = if hidden { named_roads(world, t0, true) } else { Vec::new() };
    let labels: Vec<Value> = (if hidden { &lettered } else { &named })
        .iter()
        .map(|r| json!({ "id": r.ids[0], "ids": r.ids, "name": r.name, "length_ft": crate::core::round(line_len(&r.pts)), "pts": simplify(&r.pts, 15.0) }))
        .collect();
    let rs = &t0.roads.roads;
    let fs = if named.iter().any(|r| rs[r.idx[0]].stroke.is_some()) { crate::agent::features(world, t0) } else { Vec::new() };
    let roads: Vec<Value> = named
        .into_iter()
        .map(|r| {
            let first = &rs[r.idx[0]];
            let generated = first.stroke.and_then(|s| drawn_road(world, t0, &fs, s)).map_or_else(|| class_name(first.class).to_string(), |i| fs[i].name.clone());
            let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for p in &r.pts {
                b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
            }
            json!({ "id": r.ids[0], "ids": r.ids, "name": r.name, "length_ft": crate::core::round(line_len(&r.pts)), "bbox": b.map(crate::core::round), "generated": generated })
        })
        .collect();
    let e = &world.file.edits;
    let crossings: Vec<Value> = e
        .crossings
        .iter()
        .map(|(id, c)| {
            let m = crossing_mid(c);
            let generated = crossing_generated(c);
            json!({ "id": id, "name": e.renames.get(id).cloned().unwrap_or_else(|| generated.clone()), "generated": generated, "x": crate::core::round(m[0]), "y": crate::core::round(m[1]), "length_ft": crate::core::round(c.length()) })
        })
        .collect();
    json!({ "labels": labels, "roads": roads, "crossings": crossings })
}

/// The stretches renamed alike, each name once: its stretches in order along the road (ends
/// joined where they meet), the settlements it passes, its class (the best) and total length.
/// Only names with a stretch that `keep` lets through.
pub fn renamed_groups(world: &World, t0: &T0, ids: &[String], keep: &dyn Fn(usize) -> bool) -> Vec<Value> {
    let renames = &world.file.edits.renames;
    let mut by_name: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
    for (i, id) in ids.iter().enumerate() {
        if let Some(n) = renames.get(id).filter(|n| !n.trim().is_empty() && t0.roads.roads[i].pts.len() >= 2) {
            by_name.entry(n.clone()).or_default().push(i);
        }
    }
    let rs = &t0.roads.roads;
    let mut out = Vec::new();
    for (name, set) in by_name {
        if !set.iter().any(|&i| keep(i)) {
            continue;
        }
        let order: Vec<(usize, bool)> = walk(t0, &set).concat();
        let mut towns: Vec<usize> = Vec::new();
        for &(k, fwd) in &order {
            let r = &rs[set[k]];
            let (a, b) = (r.pts[0], *r.pts.last().unwrap_or(&r.pts[0]));
            for p in if fwd { [a, b] } else { [b, a] } {
                if let Some(t) = town_at(t0, p).filter(|t| !towns.contains(t)) {
                    towns.push(t);
                }
            }
        }
        let via: Vec<Value> = towns.iter().filter_map(|&t| feature_of_layout(world, t0, t)).map(|id| json!({ "id": id, "name": name_of(world, t0, &id) })).collect();
        out.push(json!({
            "name": name,
            "stretches": order.iter().map(|&(k, _)| ids[set[k]].clone()).collect::<Vec<_>>(),
            "class": class_name(set.iter().map(|&i| rs[i].class).min().unwrap_or(rs[set[0]].class)),
            "length_mi": mi(set.iter().map(|&i| *rs[i].s.last().unwrap_or(&0.0)).sum()),
            "settlements": via,
        }));
    }
    out
}

// ---------------------------------------------------------------------------------------
// Crossings put down by hand.

/// A crossing's name before renames ("Stone bridge", "Ford", "Ferry").
pub fn crossing_generated(c: &Crossing) -> String {
    match c.kind {
        CrossingKind::Bridge => format!("{} bridge", if c.material.name() == "stone" { "Stone" } else { "Timber" }),
        CrossingKind::Ford => "Ford".into(),
        CrossingKind::Ferry => "Ferry".into(),
    }
}

pub fn crossing_mid(c: &Crossing) -> P {
    [0.5 * (c.a[0] + c.b[0]), 0.5 * (c.a[1] + c.b[1])]
}

/// A crossing as a list item and in `get_feature`: id, kind, ends, width, length, material,
/// its name (given, else as generated) and notes.
pub fn crossing_json(e: &crate::world::Edits, id: &str, c: &Crossing) -> Value {
    let mut v = json!({ "id": id, "kind": c.kind.name(), "from": c.a, "to": c.b, "width_ft": c.width, "length_ft": crate::core::round(c.length()) });
    if c.kind == CrossingKind::Bridge {
        v["material"] = json!(c.material.name());
    }
    v["name"] = json!(e.renames.get(id).cloned().unwrap_or_else(|| crossing_generated(c)));
    if let Some(n) = e.notes.get(id) {
        v["notes"] = json!({ "text": n.text, "tags": n.tags });
    }
    v
}

/// A renamed stretch as a search hit (kind `road`), from `ids` = `road_ids(t0)` and the world's features.
pub fn road_hit(world: &World, t0: &T0, ids: &[String], fs: &[Feature], id: &str) -> Option<Value> {
    let i = ids.iter().position(|r| r == id)?;
    let r = &t0.roads.roads[i];
    let m = midpoint(r);
    Some(json!({ "id": id, "kind": "road", "name": stretch_name(world, t0, fs, id, r), "class": class_name(r.class), "length_mi": mi(*r.s.last().unwrap_or(&0.0)), "x_ft": crate::core::round(m[0]), "y_ft": crate::core::round(m[1]) }))
}

/// Renamed stretches (kind `road`) and named crossings (kind `crossing`) whose names hold `ql`
/// (lowercase; empty: all), for search.
pub fn search_hits(world: &World, t0: &T0, ql: &str, kind: Option<&str>, include_hidden: bool, limit: usize) -> Vec<Value> {
    let e = &world.file.edits;
    let matches = |id: &String, n: &String| n.to_lowercase().contains(ql) && (include_hidden || !e.hidden.contains(id));
    let mut out = Vec::new();
    if kind.is_none_or(|k| k == "road") && e.renames.keys().any(|k| k.starts_with("rd:")) {
        let ids = road_ids(t0);
        for (i, id) in ids.iter().enumerate() {
            let Some(n) = e.renames.get(id).filter(|n| matches(id, n)) else { continue };
            if out.len() >= limit {
                break;
            }
            let r = &t0.roads.roads[i];
            let m = midpoint(r);
            out.push(json!({ "id": id, "kind": "road", "name": n, "class": class_name(r.class), "length_mi": mi(*r.s.last().unwrap_or(&0.0)), "x_ft": crate::core::round(m[0]), "y_ft": crate::core::round(m[1]) }));
        }
    }
    if kind.is_none_or(|k| k == "crossing") {
        for (id, c) in &e.crossings {
            let Some(n) = e.renames.get(id).filter(|n| matches(id, n)) else { continue };
            if out.len() >= limit {
                break;
            }
            let m = crossing_mid(c);
            out.push(json!({ "id": id, "kind": "crossing", "name": n, "detail": crossing_generated(c), "x_ft": crate::core::round(m[0]), "y_ft": crate::core::round(m[1]) }));
        }
    }
    out
}
