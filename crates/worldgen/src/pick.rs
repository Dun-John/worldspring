//! Everything under or near a point, for a Select tool and agents: `pick` lists each thing there
//! (drawn by hand or generated) with its id, name, owning town and whether a delete path exists for
//! it today. Topmost first (what lies under the point before what is only near it; then a way
//! underground over a crossing or a building over a road, over water, over a district, over its
//! town), then nearest. A pure query: nothing generated depends on it.
//!
//! Kinds and ids (removable: the existing edit that deletes it):
//! - `entrance` (`u:<layout>:<k>`, a sewer grate `w:<layout>:<sx>:<sy>`);
//! - `building` (`b:<layout>:<id>`): removable when it is the world's own (`remove_buildings`,
//!   `Edits.buildings`), not a castle's or a plot's piece; `tower` (`t:<layout>:<k>`);
//! - `drawn_building`, `castle`, `plot`, `wall` (`c:<n>`, drawn by hand): removable
//!   (`delete_feature`, `Created::removed`);
//! - `settlement`, `site` (the feature id; a created one's `c:<n>`): removable when created;
//! - `district` (`d:<layout>:<q>`);
//! - `road` (`rd:<8 hex>`, a world road stretch), `drawn_road` (`rw:<id>`, `Edits.paths`:
//!   removable), `crossing` (`v:<id>`: removable), `water` (`e:<id>`: removable), `harbour`
//!   (`q:<id>`, a quay, pier or breakwater built by hand: removable);
//! - without ids yet: `lane` (a village's), `street` (a town's), `hedge` / `field_wall` (round a
//!   field), `fence` (a pen's).

use std::cell::{Cell, OnceCell};

use serde::Serialize;

use crate::World;
use crate::t0::T0;
use crate::t0::features::Feature;
use crate::t0::settle::Tier;
use crate::town::{self, geom};

type P = [f64; 2];

/// How near (ft) a point must be to a way underground for it to lie under the point.
const ENTRANCE_FT: f64 = 12.0;
/// Half a drawn wall's width (ft) as picked.
const WALL_HALF_FT: f64 = 6.0;
/// Half a hedge's or a field wall's width (ft) as picked.
const BOUND_HALF_FT: f64 = 1.5;
/// Within this (ft) a thing counts as under the point.
const UNDER_FT: f64 = 0.5;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Thing {
    pub kind: &'static str,
    pub id: Option<String>,
    pub name: Option<String>,
    /// The owning settlement's feature id, if any (a drawn thing's by `town::members`; a town's
    /// building pushed out into a place, the place's).
    pub town: Option<String>,
    /// Why it is `town`'s: `gap` (a drawn thing within `gap_ft` of the town or its members),
    /// `hand` (joined, or pushed out, by hand), `inside` (a site of the world's own in its radius).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_ft: Option<f32>,
    pub removable: bool,
    /// World ft: x0, y0, x1, y1.
    pub bbox: [f64; 4],
}

/// How high a kind lies (lower first).
fn rank(kind: &str) -> u8 {
    match kind {
        "entrance" => 0,
        "crossing" | "harbour" => 1,
        "building" | "drawn_building" | "tower" => 2,
        "wall" | "fence" => 3,
        "road" | "drawn_road" | "street" | "lane" => 4,
        "hedge" | "field_wall" => 5,
        "water" => 6,
        "castle" | "plot" => 7,
        "district" => 8,
        _ => 9,
    }
}

fn bbox(pts: &[P], pad: f64) -> [f64; 4] {
    let b = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    [crate::core::floor(b[0] - pad), crate::core::floor(b[1] - pad), crate::core::ceil(b[2] + pad), crate::core::ceil(b[3] + pad)]
}

/// Distance from `p` to a polygon (0 inside).
fn poly_d(poly: &[P], p: P) -> f64 {
    if poly.len() < 3 {
        return line_d(poly, false, p);
    }
    if geom::contains(poly, p) { 0.0 } else { line_d(poly, true, p) }
}

/// Distance from `p` to a line (closed: back to its start).
fn line_d(pts: &[P], closed: bool, p: P) -> f64 {
    match pts.len() {
        0 => f64::MAX,
        1 => geom::dist(pts[0], p),
        n => {
            let open = pts.windows(2).map(|s| geom::seg_dist(p, s[0], s[1])).fold(f64::MAX, f64::min);
            if closed { open.min(geom::seg_dist(p, pts[n - 1], pts[0])) } else { open }
        }
    }
}

fn mid(pts: &[P]) -> Option<P> {
    pts.get(pts.len() / 2).copied()
}

fn near_box(pts: &[P], p: P, reach: f64) -> bool {
    let b = bbox(pts, reach + 1.0);
    p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3]
}

struct Found {
    d: f64,
    seq: usize,
    thing: Thing,
}

/// The widest ring asked over (ft): the app's largest box. `describe_location` asks no more than
/// its schema's 500.
pub const MAX_RADIUS_FT: f64 = 6000.0;

/// Everything within `radius_ft` of `at`: topmost first, then nearest (see the module doc).
pub fn pick(world: &World, t0: &T0, at: P, radius_ft: f64) -> Vec<Thing> {
    let r = radius_ft.clamp(0.0, MAX_RADIUS_FT);
    let edits = &world.file.edits;
    let layouts = town::layouts_near(world, t0, [at[0] - r, at[1] - r, at[0] + r, at[1] + r]);
    let mut out: Vec<Found> = Vec::new();
    // (Why the things put next are their town's: set per layout.)
    let why_now: Cell<(Option<&'static str>, Option<f32>)> = Cell::new((None, None));
    let mut put = |d: f64, kind: &'static str, id: Option<String>, name: Option<String>, town: Option<String>, removable: bool, bbox: [f64; 4]| {
        if d <= r {
            let seq = out.len();
            let (why, gap_ft) = if town.is_some() { why_now.get() } else { (None, None) };
            out.push(Found { d: d.max(0.0), seq, thing: Thing { kind, id, name, town, why, gap_ft, removable, bbox } });
        }
    };
    let feature = |li: usize| crate::agent::feature_of_layout(world, t0, li);
    // A layout's town and why: its own; a drawn thing's by membership; a site's of the world's
    // own the settlement it stands in.
    let first = t0.settlements.len() + t0.base_pois;
    let owner = |li: usize| -> (Option<String>, (Option<&'static str>, Option<f32>)) {
        if town::is_settlement(t0, li) {
            (feature(li), (None, None))
        } else if li >= first {
            match town::members::owner(world, t0, li) {
                Some(o) => (feature(o.town), match o.why {
                    town::Why::Gap(g) => (Some("gap"), Some(g)),
                    town::Why::Hand => (Some("hand"), None),
                    town::Why::Inside => (Some("inside"), None),
                }),
                None => (None, (None, None)),
            }
        } else {
            (town::parent_of(t0, li).and_then(feature), (Some("inside"), None))
        }
    };
    let renamed = |id: &str| edits.renames.get(id).cloned();
    // The settlement a drawn thing's middle lies in.
    let town_at = |p: P| {
        layouts
            .iter()
            .filter(|l| town::is_settlement(t0, l.index as usize) && geom::dist(l.center, p) < l.radius * 1.2)
            .min_by(|a, b| geom::dist(a.center, p).total_cmp(&geom::dist(b.center, p)).then(a.index.cmp(&b.index)))
            .and_then(|l| feature(l.index as usize))
    };
    for l in &layouts {
        let li = l.index as usize;
        let made = crate::sitelinks::created_of(world, t0, li).filter(|c| !c.removed);
        let drawn = made.filter(|c| matches!(c.kind.as_str(), "building" | "castle" | "wall" | "plot"));
        let (home, why) = owner(li);
        why_now.set(why);
        for e in &l.entrances {
            let d = geom::dist(e.at, at) - ENTRANCE_FT;
            if d > r {
                continue;
            }
            let (id, name) = if e.kind == crate::under::UnderKind::Sewer {
                (crate::gazetteer::sewer_id(l.index, e.at), None)
            } else {
                let id = format!("u:{}:{}", l.index, e.id);
                let name = renamed(&id).or_else(|| crate::sitelinks::dungeon_name(world, t0, li));
                (id, name)
            };
            put(d, "entrance", Some(id), name, home.clone(), false, bbox(&[e.at], ENTRANCE_FT));
        }
        for b in &l.buildings {
            if !near_box(&b.poly, at, r) {
                continue;
            }
            let d = poly_d(&b.poly, at);
            let id = format!("b:{li}:{}", b.id);
            match drawn {
                Some(c) if c.kind == "building" => put(d, "drawn_building", Some(c.id.clone()), renamed(&c.id).or_else(|| Some(c.name.clone())), home.clone(), true, bbox(&b.poly, 0.0)),
                _ => {
                    let removable = drawn.is_none() && crate::agent::generated_building(t0, &id).is_some();
                    // (Pushed out of its town into a place: the place's, by hand.)
                    let place = if drawn.is_none() { town::members::building_owner(world, t0, li, b.id) } else { None };
                    match place {
                        Some(p) => {
                            why_now.set((Some("hand"), None));
                            put(d, "building", Some(id.clone()), renamed(&id).or_else(|| b.name.clone()), feature(p), removable, bbox(&b.poly, 0.0));
                            why_now.set(why);
                        }
                        None => put(d, "building", Some(id.clone()), renamed(&id).or_else(|| b.name.clone()), home.clone(), removable, bbox(&b.poly, 0.0)),
                    }
                }
            }
        }
        for (k, (p, rad, _)) in crate::interior::towers(l).into_iter().enumerate() {
            put(geom::dist(p, at) - rad, "tower", Some(format!("t:{li}:{}", l.tower_id(k))), None, home.clone(), false, bbox(&[p], rad));
        }
        match drawn {
            Some(c) if c.kind == "wall" => {
                put(line_d(&c.pts, c.closed, at) - WALL_HALF_FT, "wall", Some(c.id.clone()), renamed(&c.id).or_else(|| Some(c.name.clone())), home.clone(), true, bbox(&c.pts, WALL_HALF_FT));
            }
            Some(c) if c.kind == "castle" || c.kind == "plot" => {
                let kind = if c.kind == "castle" { "castle" } else { "plot" };
                put(poly_d(&c.poly, at), kind, Some(c.id.clone()), renamed(&c.id).or_else(|| Some(c.name.clone())), home.clone(), true, bbox(&c.poly, 0.0));
            }
            Some(_) => {}
            None => {
                let d = (geom::dist(l.center, at) - l.radius * 1.2).max(0.0);
                if d <= r {
                    let id = feature(li);
                    let name = id.as_deref().and_then(|id| crate::agent::name_of(world, t0, id));
                    let kind = if town::is_settlement(t0, li) { "settlement" } else { "site" };
                    let town = if kind == "site" { home.clone() } else { None };
                    put(d, kind, id, name, town, made.is_some(), bbox(&[l.center], l.radius * 1.2));
                }
            }
        }
        for (qi, q) in l.quarters.iter().enumerate() {
            let d = q.patches.iter().filter(|poly| near_box(poly, at, r)).map(|poly| poly_d(poly, at)).fold(f64::MAX, f64::min);
            if d <= r {
                let pts: Vec<P> = q.patches.iter().flatten().copied().collect();
                let id = format!("d:{li}:{qi}");
                put(d, "district", Some(id.clone()), renamed(&id).or_else(|| Some(q.name.clone())), home.clone(), false, bbox(&pts, 0.0));
            }
        }
        // Streets and lanes (no ids yet).
        let lane = if l.tier == Tier::Village { "lane" } else { "street" };
        let half = if l.tier == Tier::Village { 6.0 } else { 10.0 };
        for s in &l.streets {
            if near_box(s, at, r + half) {
                put(line_d(s, false, at) - half, lane, None, None, home.clone(), false, bbox(s, half));
            }
        }
        for (s, _, w) in &l.roads {
            if near_box(s, at, r + 0.5 * w) {
                put(line_d(s, false, at) - 0.5 * w, lane, None, None, home.clone(), false, bbox(s, 0.5 * w));
            }
        }
        // Hedges and dry-stone walls round fields, pens' fences.
        for (f, info) in l.fields.iter().zip(&l.field_info) {
            let kind = match info.bound {
                town::fields::FieldBound::Hedge => "hedge",
                town::fields::FieldBound::Wall => "field_wall",
                town::fields::FieldBound::None => continue,
            };
            if near_box(f, at, r) {
                put(line_d(f, true, at) - BOUND_HALF_FT, kind, None, None, home.clone(), false, bbox(f, BOUND_HALF_FT));
            }
        }
        if !l.yurts.is_empty() {
            let pens = town::yurts::fences(l, [at[0] - r, at[1] - r, at[0] + r, at[1] + r]);
            if let Some((d, s)) = pens.iter().map(|s| (geom::seg_dist(at, s.0, s.1), s)).min_by(|a, b| a.0.total_cmp(&b.0)) {
                put(d - BOUND_HALF_FT, "fence", None, None, home.clone(), false, bbox(&[s.0, s.1], BOUND_HALF_FT));
            }
        }
    }
    why_now.set((None, None));
    // World road stretches.
    let net = &t0.roads;
    let mut roads: Vec<(usize, f64)> = Vec::new();
    for (ri, k) in net.segments_near([at[0] - r, at[1] - r, at[0] + r, at[1] + r], 40.0) {
        let rc = &net.roads[ri as usize];
        let k = k as usize;
        if k + 1 >= rc.pts.len() {
            continue;
        }
        let d = geom::seg_dist(at, rc.pts[k], rc.pts[k + 1]) - 0.5 * rc.class.width_ft();
        match roads.iter_mut().find(|(i, _)| *i == ri as usize) {
            Some(e) => e.1 = e.1.min(d),
            None => roads.push((ri as usize, d)),
        }
    }
    if roads.iter().any(|(_, d)| *d <= r) {
        let ids = crate::roadids::road_ids(t0);
        let fs: OnceCell<Vec<Feature>> = OnceCell::new();
        for (ri, d) in roads {
            let rc = &net.roads[ri];
            let id = &ids[ri];
            let name = match (edits.renames.get(id), rc.stroke) {
                (Some(n), _) => Some(n.clone()),
                (None, Some(_)) => crate::roadids::stretch_name(world, t0, fs.get_or_init(|| crate::agent::features(world, t0)), id, rc),
                (None, None) => None,
            };
            put(d, "road", Some(id.clone()), name, None, false, bbox(&rc.pts, 0.5 * rc.class.width_ft()));
        }
    }
    // Drawn by hand: roads, crossings, water, harbour pieces.
    for (id, p) in &edits.paths {
        let half = 0.5 * p.width.unwrap_or_else(|| p.kind.width_ft());
        if near_box(&p.pts, at, r + half) {
            put(line_d(&p.pts, false, at) - half, "drawn_road", Some(id.clone()), renamed(id).or_else(|| p.name.clone()), mid(&p.pts).and_then(town_at), true, bbox(&p.pts, half));
        }
    }
    for (id, c) in &edits.crossings {
        let half = 0.5 * c.width;
        let d = geom::seg_dist(at, c.a, c.b) - half;
        if d <= r {
            let name = renamed(id).unwrap_or_else(|| crate::roadids::crossing_generated(c));
            put(d, "crossing", Some(id.clone()), Some(name), town_at(crate::roadids::crossing_mid(c)), true, bbox(&[c.a, c.b], half));
        }
    }
    for (id, w) in &edits.waters {
        let half = if w.is_line() { 0.5 * w.width_ft() } else { 0.0 };
        if !near_box(&w.pts, at, r + half) {
            continue;
        }
        let d = if w.is_line() { line_d(&w.pts, w.is_ring(), at) - half } else { poly_d(&w.pts, at) };
        put(d, "water", Some(id.clone()), renamed(id), mid(&w.pts).and_then(town_at), true, bbox(&w.pts, half));
    }
    for (id, q) in &edits.wharves {
        let half = 0.5 * q.width;
        if near_box(&q.pts, at, r + half) {
            put(line_d(&q.pts, false, at) - half, "harbour", Some(id.clone()), renamed(id), mid(&q.pts).and_then(town_at), true, bbox(&q.pts, half));
        }
    }
    out.sort_by(|a, b| {
        (a.d > UNDER_FT)
            .cmp(&(b.d > UNDER_FT))
            .then(rank(a.thing.kind).cmp(&rank(b.thing.kind)))
            .then(a.d.total_cmp(&b.d))
            .then(a.seq.cmp(&b.seq))
    });
    out.into_iter().map(|f| f.thing).collect()
}

pub fn pick_json(world: &World, t0: &T0, x: f64, y: f64, radius_ft: f64) -> String {
    serde_json::to_string(&pick(world, t0, [x, y], radius_ft)).expect("serializable")
}
