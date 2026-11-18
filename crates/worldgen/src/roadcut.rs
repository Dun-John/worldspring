//! (Roadmap 6 R5) Roads, streets and paths taken off the map by hand, and brought back: what the
//! app's Select › Delete, the Gazetteer's "Bring back" and the MCP tool `remove_roads` ask.
//!
//! `cut` turns a list of ids into the edits that do it (nothing is changed here):
//! - `rd:` a world road's stretch, `mt:` a track to a settlement added by hand: into or out of
//!   `Edits.removed_roads`. A stretch taken away takes the approach street of each generated town
//!   it ends at (the town's entry within `APPROACH_FT` of its end) with it, and brings it back
//!   with it (`also`); the town's gate stays;
//! - `tr:` a generated town's or village's street or lane, `pp:` a park's path: into or out of
//!   its `TownEdit::removed` (kept while the town is where its edit says);
//! - `rw:` a road drawn by hand and `ts:` a street added to a generated town: their own deletes
//!   (`Edits.paths`, `TownEdit::streets`), not brought back here (Undo does);
//! - a hand town's streets are refused (its street plan changes them).
//!
//! `removed` lists what is removed, for the Gazetteer. A pure query: nothing generated reads it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

use crate::World;
use crate::t0::T0;
use crate::t0::features::Feature;
use crate::town::{self, cut, geom};
use crate::world::{Edits, TownEdit};

type P = [f64; 2];

/// How far (ft) round a removed line's half width the map redraws (decks, park paths, halos).
const PAD_FT: f64 = 50.0;
/// How near (ft) a town's approach starts to a stretch's end to go with it.
const APPROACH_FT: f64 = 2.0;
/// The most boxes one line is redrawn as.
const MAX_BOXES: usize = 39;
/// The most ids one MCP call takes (`cut` takes any number: the app's Select box can hold thousands).
pub const MAX_IDS: usize = 500;

#[derive(Clone, Debug, Serialize)]
pub struct Refused {
    pub id: String,
    pub why: String,
}

/// The edits a `cut` makes, and what it did.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CutOut {
    /// `Edits.removed_roads` as it becomes (None: unchanged).
    pub removed_roads: Option<BTreeSet<String>>,
    /// Roads drawn by hand (`rw:`) to delete.
    pub paths: Vec<String>,
    /// Town edits (`Edits.towns`, by key) as they become (None: the entry goes).
    pub towns: BTreeMap<String, Option<TownEdit>>,
    /// Boxes (world ft) whose site-level tiles and battlemaps redraw.
    pub rects: Vec<[f64; 4]>,
    /// Boxes whose tiles of every level redraw (world roads' stretches and tracks).
    pub all_rects: Vec<[f64; 4]>,
    /// The ids asked for that were removed (or restored).
    pub done: Vec<String>,
    /// The towns' approaches taken (or brought back) with a stretch.
    pub also: Vec<String>,
    pub refused: Vec<Refused>,
}

impl CutOut {
    /// `e` with the change made.
    pub fn apply(&self, e: &mut Edits) {
        if let Some(r) = &self.removed_roads {
            e.removed_roads = r.clone();
        }
        for id in &self.paths {
            e.paths.remove(id);
            e.notes.remove(id);
            e.hidden.remove(id);
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
        self.removed_roads.is_none() && self.paths.is_empty() && self.towns.is_empty()
    }
}

/// A town edit that holds nothing but its `at`.
pub(crate) fn holds_nothing(t: &TownEdit) -> bool {
    t.is_noop() && t.districts.is_empty() && t.streets.is_empty() && t.removed.is_empty() && t.fences.is_empty()
}

/// A line's boxes (at most `MAX_BOXES`, each over a run of its points), padded by `pad`.
fn boxes(pts: &[P], pad: f64, out: &mut Vec<[f64; 4]>) {
    let n = pts.len();
    if n == 0 {
        return;
    }
    let runs = (n - 1).clamp(1, MAX_BOXES);
    for j in 0..runs {
        let (a, b) = ((n - 1) * j / runs, ((n - 1) * (j + 1) / runs).max((n - 1) * j / runs));
        let b0 = geom::bbox(&pts[a..=b]);
        out.push([crate::core::floor(b0[0] - pad), crate::core::floor(b0[1] - pad), crate::core::ceil(b0[2] + pad), crate::core::ceil(b0[3] + pad)]);
    }
}

/// A world road's stretch or a made track by id: (line, width ft).
fn world_line<'a>(t0: &'a T0, tracks: &[String], id: &str) -> Option<(&'a [P], f64)> {
    if id.starts_with("rd:") {
        let r = &t0.roads.roads[t0.road_index(id)?];
        return Some((&r.pts, r.class.width_ft()));
    }
    let k = tracks.iter().position(|t| t == id)?;
    let m = &t0.made[k];
    Some((&m.track, m.class.width_ft()))
}

/// A generated settlement's street, lane or park path by id: (layout, local key, line, width
/// ft). Err: why not (a hand town's, no such town).
fn town_line(world: &World, t0: &T0, id: &str) -> Result<Option<(usize, String, Vec<P>, f64)>, String> {
    let Some((li, local)) = cut::parse(id) else { return Ok(None) };
    if li >= town::layout_count(t0) || !town::is_settlement(t0, li) {
        return Ok(None);
    }
    if town::hand::is_hand(world, li) {
        return Err("a town laid out by hand: change its streets in its street plan (Streets tab, edit_streets)".into());
    }
    let l = town::layout(world, t0, li);
    match local.split_once(':') {
        None => {
            let Ok(n) = u32::from_str_radix(&local, 16) else { return Ok(None) };
            let Some(&(tag, k, _)) = cut::street_ids(world, t0, &l).iter().find(|x| x.2 == n) else { return Ok(None) };
            Ok(Some(if tag == cut::ROADS {
                (li, local, l.roads[k].0.clone(), l.roads[k].2)
            } else {
                (li, local, l.streets[k].clone(), town::village::LANE_W)
            }))
        }
        Some((spot, j)) => {
            let (Some(pk), Ok(j)) = ((0..l.parks.len()).find(|&k| cut::park_spot(&l, k) == spot), j.parse::<usize>()) else { return Ok(None) };
            let park = &l.parks[pk];
            if park.kind == town::ParkKind::Horses {
                return Ok(None);
            }
            let lay = crate::battlemap::park_layout(park, crate::battlemap::land_biome(t0, geom::centroid(&park.pts)), l.tier >= crate::t0::settle::Tier::City, world.stream("battlemap"));
            Ok(lay.paths.get(j).map(|(pts, w)| (li, local.clone(), pts.clone(), *w)))
        }
    }
}

/// The approaches (`tr:` ids, with their lines) of the generated towns stretch `ri` ends at.
fn approaches(world: &World, t0: &T0, ri: usize) -> Vec<(String, Vec<P>, f64)> {
    let r = &t0.roads.roads[ri];
    let (Some(&a), Some(&b)) = (r.pts.first(), r.pts.last()) else { return Vec::new() };
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for end in [a, b] {
        let Some(li) = crate::roadids::town_at(t0, end) else { continue };
        if town::added::home(world, t0, li).is_none() {
            continue;
        }
        let l = town::layout(world, t0, li);
        let ids = cut::street_ids(world, t0, &l);
        for &(tag, k, n) in &ids {
            if tag != cut::ROADS {
                continue;
            }
            let (pts, class, w) = &l.roads[k];
            if *class <= 2 && pts.first().is_some_and(|p| geom::dist(*p, end) <= APPROACH_FT) {
                let id = cut::street_id(li, n);
                if !seen.contains(&id) {
                    seen.push(id.clone());
                    out.push((id, pts.clone(), *w));
                }
            }
        }
    }
    out
}

/// The working copy of the town edits a cut changes.
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
        let v = (!holds_nothing(&t)).then_some(t);
        if v.as_ref() == self.edits.towns.get(&key) {
            self.towns.remove(&key);
        } else {
            self.towns.insert(key, v);
        }
    }

    /// Local key `local` of layout `li` taken away (Err: why not).
    fn remove(&mut self, world: &World, t0: &T0, li: usize, local: &str) -> Result<(), String> {
        let home = town::added::home(world, t0, li).ok_or("not a generated town")?;
        let key = town::wards::key(li);
        let mut t = self.get(&key).cloned().unwrap_or_else(|| TownEdit { at: home, ..Default::default() });
        if geom::dist(t.at, home) > TownEdit::SAME_AT_FT {
            return Err("the town's edits are set aside (it moved): open it in Streets first".into());
        }
        if !t.removed.insert(local.to_string()) {
            return Err("already removed".into());
        }
        self.put(key, t);
        Ok(())
    }

    /// Local key `local` of layout `li` brought back (false: it was not removed).
    fn restore(&mut self, li: usize, local: &str) -> bool {
        let key = town::wards::key(li);
        let Some(mut t) = self.get(&key).cloned() else { return false };
        if !t.removed.remove(local) {
            return false;
        }
        self.put(key, t);
        true
    }
}

/// The edits that take `ids` off the map (`restore`: bring them back); see the module doc.
/// `dry`: the boxes to redraw for each id that resolves, whatever its state, and nothing else
/// (the app asks it after an Undo or a change from elsewhere).
pub fn cut(world: &World, t0: &T0, ids: &[String], restore: bool, dry: bool) -> CutOut {
    let e = &world.file.edits;
    let mut out = CutOut::default();
    let mut gone = e.removed_roads.clone();
    let mut towns = Towns { edits: e, towns: BTreeMap::new() };
    let tracks = if ids.iter().any(|id| id.starts_with("mt:")) { crate::t0::made::track_ids(t0) } else { Vec::new() };
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let refuse = |out: &mut CutOut, id: &str, why: &str| out.refused.push(Refused { id: id.to_string(), why: why.to_string() });
    for id in ids {
        if !seen.insert(id.as_str()) {
            continue;
        }
        let head = id.split(':').next().unwrap_or("");
        match head {
            "rd" | "mt" => {
                let line = world_line(t0, &tracks, id);
                let ok_id = if head == "rd" { crate::lod::roads::parse_id(id).is_some() } else { id.len() == 11 && id[3..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) };
                if !ok_id {
                    refuse(&mut out, id, "not a road id");
                    continue;
                }
                if dry {
                    if let Some((pts, w)) = line {
                        boxes(pts, 0.5 * w + PAD_FT, &mut out.all_rects);
                    }
                } else if restore {
                    if !gone.remove(id) {
                        refuse(&mut out, id, "not removed");
                        continue;
                    }
                    if let Some((pts, w)) = line {
                        boxes(pts, 0.5 * w + PAD_FT, &mut out.all_rects);
                    }
                } else {
                    let Some((pts, w)) = line else {
                        refuse(&mut out, id, "no such road");
                        continue;
                    };
                    if !gone.insert(id.clone()) {
                        refuse(&mut out, id, "already removed");
                        continue;
                    }
                    boxes(pts, 0.5 * w + PAD_FT, &mut out.all_rects);
                }
                out.done.push(id.clone());
                // (Answer 7) The towns' approaches at a stretch's ends go and come back with it.
                if let Some(ri) = (head == "rd").then(|| t0.road_index(id)).flatten() {
                    for (aid, pts, w) in approaches(world, t0, ri) {
                        let Some((li, local)) = cut::parse(&aid) else { continue };
                        let changed = dry || if restore { towns.restore(li, &local) } else { towns.remove(world, t0, li, &local).is_ok() };
                        if changed {
                            boxes(&pts, 0.5 * w + PAD_FT, &mut out.rects);
                            if !dry {
                                out.also.push(aid);
                            }
                        }
                    }
                }
            }
            "tr" | "pp" => {
                let line = match town_line(world, t0, id) {
                    Ok(l) => l,
                    Err(why) => {
                        refuse(&mut out, id, &why);
                        continue;
                    }
                };
                if dry {
                    if let Some((_, _, pts, w)) = &line {
                        boxes(pts, 0.5 * w + PAD_FT, &mut out.rects);
                    }
                    out.done.push(id.clone());
                    continue;
                }
                if restore {
                    let Some((li, local)) = cut::parse(id) else {
                        refuse(&mut out, id, "not a street or path id");
                        continue;
                    };
                    if !towns.restore(li, &local) {
                        refuse(&mut out, id, "not removed");
                        continue;
                    }
                    if let Some((_, _, pts, w)) = &line {
                        boxes(pts, 0.5 * w + PAD_FT, &mut out.rects);
                    }
                    out.done.push(id.clone());
                    continue;
                }
                let Some((li, local, pts, w)) = line else {
                    refuse(&mut out, id, if head == "tr" { "no such street" } else { "no such path" });
                    continue;
                };
                match towns.remove(world, t0, li, &local) {
                    Ok(()) => {
                        boxes(&pts, 0.5 * w + PAD_FT, &mut out.rects);
                        out.done.push(id.clone());
                    }
                    Err(why) => refuse(&mut out, id, &why),
                }
            }
            "rw" => {
                let Some(p) = e.paths.get(id) else {
                    refuse(&mut out, id, "no such road");
                    continue;
                };
                if restore && !dry {
                    refuse(&mut out, id, "a road drawn by hand: Undo brings it back");
                    continue;
                }
                use crate::paths::PathLine;
                boxes(&p.line(), 0.5 * p.width_ft() + PAD_FT, &mut out.rects);
                if !dry {
                    out.paths.push(id.clone());
                }
                out.done.push(id.clone());
            }
            "ts" => {
                let parsed = id.split(':').nth(1).and_then(|l| l.parse::<usize>().ok()).zip(id.split(':').nth(2).and_then(|n| n.parse::<u32>().ok()));
                let key = parsed.map(|(li, _)| town::wards::key(li));
                let Some(((_, n), key)) = parsed.zip(key) else {
                    refuse(&mut out, id, "not a street id");
                    continue;
                };
                let Some(s) = towns.get(&key).and_then(|t| t.streets.get(&n)).cloned() else {
                    refuse(&mut out, id, "no such street");
                    continue;
                };
                if restore && !dry {
                    refuse(&mut out, id, "a street added by hand: Undo brings it back");
                    continue;
                }
                boxes(&s.pts, 0.5 * s.width_ft() + PAD_FT, &mut out.rects);
                if !dry {
                    let mut t = towns.get(&key).cloned().unwrap_or_default();
                    t.streets.remove(&n);
                    towns.put(key, t);
                }
                out.done.push(id.clone());
            }
            _ if id.split_once('/').is_some_and(|(l, r)| l.parse::<usize>().is_ok() && r.starts_with('e')) => refuse(&mut out, id, "a hand town's street: change it in its street plan (Streets tab, edit_streets)"),
            _ => refuse(&mut out, id, "not a road, street or path id (rd:, mt:, tr:, pp:, rw:, ts:)"),
        }
    }
    if !dry {
        if gone != e.removed_roads {
            out.removed_roads = Some(gone);
        }
        out.towns = towns.towns;
    }
    out
}

pub fn cut_json(world: &World, t0: &T0, ids: &[String], restore: bool, dry: bool) -> Value {
    serde_json::to_value(cut(world, t0, ids, restore, dry)).unwrap_or(Value::Null)
}

/// One road, street or path removed by hand, for the Gazetteer.
#[derive(Clone, Debug, Serialize)]
pub struct Removed {
    pub id: String,
    pub name: Option<String>,
    /// What it is ("king's road, 3.2 mi", "street", "lane", "path").
    pub what: String,
    /// Where ("Ashby to Fenwick", "Ashby", "the Commons").
    pub at: Option<String>,
    /// False when the id no longer names anything (the town or the world changed since).
    pub found: bool,
}

fn mi(ft: f64) -> f64 {
    crate::core::round(ft / 5280.0 * 10.0) / 10.0
}

/// Everything removed by hand (`Edits.removed_roads`, each town edit's `removed`), in id order.
pub fn removed(world: &World, t0: &T0) -> Vec<Removed> {
    let e = &world.file.edits;
    let mut out = Vec::new();
    let town_name = |li: usize| crate::agent::feature_of_layout(world, t0, li).and_then(|f| crate::agent::name_of(world, t0, &f));
    let end_name = |p: P| crate::roadids::town_at(t0, p).and_then(town_name);
    let tracks = if e.removed_roads.iter().any(|id| id.starts_with("mt:")) { crate::t0::made::track_ids(t0) } else { Vec::new() };
    let mut fs: Option<Vec<Feature>> = None;
    for id in &e.removed_roads {
        let mut r = Removed { id: id.clone(), name: e.renames.get(id).cloned(), what: if id.starts_with("mt:") { "track".into() } else { "road".into() }, at: None, found: false };
        if let Some(ri) = t0.road_index(id) {
            let rc = &t0.roads.roads[ri];
            if r.name.is_none() && rc.stroke.is_some() {
                r.name = crate::roadids::stretch_name(world, t0, fs.get_or_insert_with(|| crate::agent::features(world, t0)), id, rc);
            }
            r.what = format!("{}, {} mi", crate::roadids::class_name(rc.class), mi(*rc.s.last().unwrap_or(&0.0)));
            let (a, b) = (rc.pts.first().copied().and_then(end_name), rc.pts.last().copied().and_then(end_name));
            r.at = match (a, b) {
                (Some(a), Some(b)) if a != b => Some(format!("{a} to {b}")),
                (Some(a), _) | (None, Some(a)) => Some(a),
                _ => None,
            };
            r.found = true;
        } else if let Some(k) = tracks.iter().position(|t| t == id) {
            let m = &t0.made[k];
            let len: f64 = m.track.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
            r.what = format!("{}, {} mi", crate::roadids::class_name(m.class), mi(len));
            r.at = town_name(t0.settlements.len() + m.poi).map(|n| format!("to {n}"));
            r.found = true;
        }
        out.push(r);
    }
    for (key, t) in &e.towns {
        let Ok(li) = key.parse::<usize>() else { continue };
        // (A town laid out by hand keeps its `removed` only for taking it back to generated.)
        if town::hand::is_hand(world, li) {
            continue;
        }
        for local in &t.removed {
            let id = if local.contains(':') { format!("pp:{li}:{local}") } else { format!("tr:{li}:{local}") };
            let found = town::cut::removed_of(world, t0, li).is_some_and(|r| r.contains(local)) && matches!(town_line(world, t0, &id), Ok(Some(_)));
            let village = li < town::layout_count(t0) && town::is_settlement(t0, li) && town::settlement(t0, li).is_some_and(|s| s.tier == crate::t0::settle::Tier::Village);
            let what = if local.contains(':') { "path" } else if village { "lane" } else { "street" };
            let mut at = town_name(li);
            if let Some((spot, _)) = local.split_once(':') {
                let sq = format!("sq:{li}:{spot}");
                if let Some(n) = e.renames.get(&sq).cloned().or_else(|| town::places::find(world, t0, &sq).map(|(_, p)| p.name)) {
                    at = Some(match at {
                        Some(t) => format!("{n}, {t}"),
                        None => n,
                    });
                }
            }
            out.push(Removed { id, name: None, what: what.into(), at, found });
        }
    }
    out
}

pub fn removed_json(world: &World, t0: &T0) -> Value {
    json!(removed(world, t0))
}
