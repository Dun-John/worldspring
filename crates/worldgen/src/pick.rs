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
//! - `road` (`rd:<8 hex>`, a world road stretch; `mt:<8 hex>`, a track to a settlement added by
//!   hand): removable (`roadcut`, `Edits.removed_roads`); `drawn_road` (`rw:<id>`, `Edits.paths`:
//!   removable), `crossing` (`v:<id>`: removable), `water` (`e:<id>`: removable), `harbour`
//!   (`q:<id>`, a quay, pier or breakwater built by hand: removable);
//! - `street` / `lane` of a generated town or village (`tr:<layout>:<8 hex>`, `town::cut`) and
//!   `path` (`pp:<layout>:<spot>:<j>`, a park's, garden's or cemetery's walk; only in picks up to 400 ft out, a click or a small box): removable
//!   (`roadcut`, `TownEdit::removed`); one added to a generated town by hand (`ts:<layout>:<id>`,
//!   `TownEdit::streets`): removable. Roads, streets and paths removed by hand are left out;
//! - `fence` (a generated run, `fr:<layout>:<8 hex>`, `town::fence`; only in picks up to 1,500 ft
//!   out) and a park's hedge or row of topiary (`fh:<layout>:<spot>:<j>`, up to 400 ft out), of a
//!   generated settlement: removable (`fencecut`, `TownEdit::fences`); `drawn_fence` (`f:<n>`,
//!   `Edits.fences`) and `fence_gap` (`fg:<n>`, `Edits.fence_gaps`): removable. `fence` names its
//!   kind's key (`FenceKind`); those removed by hand are left out;
//! - without ids: a hand town's streets (changed in its street plan), a place's lanes, the fences
//!   of a hand town or a site.

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
    /// (R6) A fence's or hedge's kind (`FenceKind` key; None a row of topiary, a gap).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fence: Option<&'static str>,
    /// For a line (road, street, lane, path, drawn road, fence, gap): its points near the pick (at most `LINE_PTS`), so an area can tell
    /// whether the line runs through it. Only `pick_json` gives it.
    #[serde(skip)]
    pub line: Option<Vec<P>>,
}

/// The widest pick (ft from its middle to its corners) that lists parks' paths (`pp:`): a click or a small box.
const PARK_PICK_FT: f64 = 400.0;

/// The widest pick (ft from its middle to its corners) that lists generated fence runs (`fr:`).
const FENCE_PICK_FT: f64 = 1500.0;

/// The most points of a line `Thing::line` keeps.
const LINE_PTS: usize = 160;

/// The part of `pts` that runs within `r` of `at` (the run from the first to the last segment that does), thinned to `LINE_PTS`.
fn clip(pts: &[P], at: P, r: f64) -> Vec<P> {
    let near = |w: &[P]| w[0][0].min(w[1][0]) <= at[0] + r && w[0][0].max(w[1][0]) >= at[0] - r && w[0][1].min(w[1][1]) <= at[1] + r && w[0][1].max(w[1][1]) >= at[1] - r;
    let (Some(a), Some(b)) = (pts.windows(2).position(near), pts.windows(2).rposition(near)) else { return Vec::new() };
    let run = &pts[a..=b + 1];
    if run.len() <= LINE_PTS {
        return run.to_vec();
    }
    let n = run.len() - 1;
    (0..LINE_PTS).map(|i| run[i * n / (LINE_PTS - 1)]).collect()
}

/// How high a kind lies (lower first).
fn rank(kind: &str) -> u8 {
    match kind {
        "entrance" => 0,
        "crossing" | "harbour" => 1,
        "building" | "drawn_building" | "tower" => 2,
        "wall" | "drawn_fence" => 3,
        "road" | "drawn_road" | "street" | "lane" | "path" => 4,
        "fence" | "fence_gap" => 5,
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

/// `s` with its first letter upper case.
fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
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
    // (A line's points, set just before the `put` of it.)
    let line_now: Cell<Option<Vec<P>>> = Cell::new(None);
    // (A fence's kind, set just before the `put` of it.)
    let fence_now: Cell<Option<&'static str>> = Cell::new(None);
    let mut put = |d: f64, kind: &'static str, id: Option<String>, name: Option<String>, town: Option<String>, removable: bool, bbox: [f64; 4]| {
        let line = line_now.take();
        let fence = fence_now.take();
        if d <= r {
            let seq = out.len();
            let (why, gap_ft) = if town.is_some() { why_now.get() } else { (None, None) };
            out.push(Found { d: d.max(0.0), seq, thing: Thing { kind, id, name, town, why, gap_ft, removable, fence, bbox, line } });
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
        // Streets and lanes: a generated settlement's with their `tr:` ids (R5), those removed by
        // hand left out; a town's `l.streets` (its main streets' paths, never drawn) then not listed.
        let lane = if l.tier == Tier::Village { "lane" } else { "street" };
        let half = if l.tier == Tier::Village { 6.0 } else { 10.0 };
        let generated = town::added::home(world, t0, li).is_some();
        let cut = if generated { town::cut::Cut::of(world, t0, l) } else { None };
        // (The ids are worked out once, when a line in reach asks for its own.)
        let trs: std::cell::OnceCell<Vec<(u8, usize, u32)>> = std::cell::OnceCell::new();
        let tr = |tag: u8, k: usize| {
            if !generated {
                return None;
            }
            trs.get_or_init(|| town::cut::street_ids(world, t0, l)).iter().find(|x| x.0 == tag && x.1 == k).map(|x| town::cut::street_id(li, x.2))
        };
        let gone = |tag: u8, k: usize| cut.as_ref().is_some_and(|c| c.line(tag, k));
        for (k, s) in l.streets.iter().enumerate() {
            if (generated && l.tier != Tier::Village) || gone(town::cut::STREETS, k) {
                continue;
            }
            if near_box(s, at, r + half) {
                let id = tr(town::cut::STREETS, k);
                line_now.set(Some(clip(s, at, r + half)));
                put(line_d(s, false, at) - half, lane, id.clone(), None, home.clone(), id.is_some(), bbox(s, half));
            }
        }
        // (R4: a street added to a generated town has an id, `ts:<layout>:<id>`, and goes with its `TownEdit` street.)
        let added = town::added::ids(world, t0, l);
        for (k, (s, _, w)) in l.roads.iter().enumerate() {
            if gone(town::cut::ROADS, k) {
                continue;
            }
            if near_box(s, at, r + 0.5 * w) {
                let id = added.iter().find(|x| x.0 == k).map(|x| format!("ts:{li}:{}", x.1)).or_else(|| tr(town::cut::ROADS, k));
                let removable = id.is_some();
                line_now.set(Some(clip(s, at, r + 0.5 * w)));
                put(line_d(s, false, at) - 0.5 * w, lane, id, None, home.clone(), removable, bbox(s, 0.5 * w));
            }
        }
        // (R5) A generated settlement's parks' paths (`pp:`), as their battlemaps lay them out.
        // (A park is laid out for each one in reach, so only for a click or a box of a few parks: `PARK_PICK_FT`.)
        if generated && r <= PARK_PICK_FT {
            for (pk, park) in l.parks.iter().enumerate() {
                if park.kind == town::ParkKind::Horses || !near_box(&park.pts, at, r) {
                    continue;
                }
                let pc = geom::centroid(&park.pts);
                let lay = crate::battlemap::park_layout(park, crate::battlemap::land_biome(t0, pc), l.tier >= Tier::City, world.stream("battlemap"));
                let spot = town::cut::park_spot(l, pk);
                let gone = cut.as_ref().and_then(|c| c.paths(pk));
                for (j, (pts, w)) in lay.paths.iter().enumerate() {
                    if gone.is_some_and(|g| g.contains(&(j as u32))) || !near_box(pts, at, r + 0.5 * w) {
                        continue;
                    }
                    line_now.set(Some(clip(pts, at, r + 0.5 * w)));
                    put(line_d(pts, false, at) - 0.5 * w, "path", Some(town::cut::path_id(li, &spot, j)), None, home.clone(), true, bbox(pts, 0.5 * w));
                }
            }
        }
        // (R6) Fences and hedges: a generated settlement's runs with their `fr:` ids, its parks'
        // hedges and topiary rows (`fh:`, a click or a small box); those removed by hand left out.
        let ids = generated && !town::hand::is_hand(world, li);
        let fences = if ids { town::fence::changes_of(world, t0, li) } else { None };
        let change = |local: &str| fences.and_then(|c| c.get(local));
        if r <= FENCE_PICK_FT {
            for (tag, a, b, kind) in town::fence::sources(l, t0, [at[0] - r, at[1] - r, at[0] + r, at[1] + r]) {
                let (a, b) = town::fence::canonical(a, b);
                let key = town::fence::run_key(tag, a, b);
                let ch = change(&format!("{key:08x}"));
                if ch.is_some_and(|c| c.gone) {
                    continue;
                }
                let kind = ch.and_then(|c| c.kind).unwrap_or(kind);
                line_now.set(Some(vec![a, b]));
                fence_now.set(Some(kind.name()));
                let id = ids.then(|| town::fence::fr_id(li, key));
                put(geom::seg_dist(at, a, b) - BOUND_HALF_FT, "fence", id, Some(cap(kind.label())), home.clone(), ids, bbox(&[a, b], BOUND_HALF_FT));
            }
        }
        if ids && r <= PARK_PICK_FT {
            for (pk, park) in l.parks.iter().enumerate() {
                if !near_box(&park.pts, at, r) {
                    continue;
                }
                let spot = town::cut::park_spot(l, pk);
                for (j, a, b, kind) in crate::fencecut::park_runs(world, t0, l, pk) {
                    let ch = change(&format!("{spot}:{j}"));
                    if ch.is_some_and(|c| c.gone) {
                        continue;
                    }
                    let kind = ch.and_then(|c| c.kind).or(kind);
                    line_now.set(Some(vec![a, b]));
                    fence_now.set(kind.map(|k| k.name()));
                    let name = kind.map_or_else(|| "Row of topiary".to_string(), |k| cap(k.label()));
                    put(geom::seg_dist(at, a, b) - BOUND_HALF_FT, "fence", Some(town::fence::fh_id(li, &spot, j)), Some(name), home.clone(), true, bbox(&[a, b], BOUND_HALF_FT));
                }
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
            if edits.removed_roads.contains(id) {
                continue;
            }
            let name = match (edits.renames.get(id), rc.stroke) {
                (Some(n), _) => Some(n.clone()),
                (None, Some(_)) => crate::roadids::stretch_name(world, t0, fs.get_or_init(|| crate::agent::features(world, t0)), id, rc),
                (None, None) => None,
            };
            if d <= r {
                line_now.set(Some(clip(&rc.pts, at, r + 0.5 * rc.class.width_ft())));
            }
            // (Its class first, then its name if it has one: "King's road (Old Salt Way)".)
            let class = crate::roadids::class_name(rc.class);
            let class = format!("{}{}", class[..1].to_uppercase(), &class[1..]);
            let name = Some(match name {
                Some(n) => format!("{class} ({n})"),
                None => class,
            });
            put(d, "road", Some(id.clone()), name, None, true, bbox(&rc.pts, 0.5 * rc.class.width_ft()));
        }
    }
    // Tracks to settlements added by hand (`mt:`).
    let tracks: OnceCell<Vec<String>> = OnceCell::new();
    for (k, m) in t0.made.iter().enumerate() {
        let half = 0.5 * m.class.width_ft();
        if m.track.len() < 2 || !near_box(&m.track, at, r + half) {
            continue;
        }
        let id = &tracks.get_or_init(|| crate::t0::made::track_ids(t0))[k];
        if !edits.removed_roads.contains(id) {
            line_now.set(Some(clip(&m.track, at, r + half)));
            put(line_d(&m.track, false, at) - half, "road", Some(id.clone()), Some("Track".into()), None, true, bbox(&m.track, half));
        }
    }
    // Drawn by hand: roads, crossings, water, harbour pieces.
    for (id, p) in &edits.paths {
        let half = 0.5 * p.width.unwrap_or_else(|| p.kind.width_ft());
        if near_box(&p.pts, at, r + half) {
            line_now.set(Some(clip(&p.pts, at, r + half)));
            put(line_d(&p.pts, false, at) - half, "drawn_road", Some(id.clone()), renamed(id).or_else(|| p.name.clone()), mid(&p.pts).and_then(town_at), true, bbox(&p.pts, half));
        }
    }
    // (R6) Fences drawn by hand and the eraser's gaps.
    for (id, f) in &edits.fences {
        if near_box(&f.pts, at, r + BOUND_HALF_FT) {
            line_now.set(Some(clip(&f.pts, at, r + BOUND_HALF_FT)));
            fence_now.set(Some(f.kind.name()));
            put(line_d(&f.pts, f.closed, at) - BOUND_HALF_FT, "drawn_fence", Some(id.clone()), renamed(id).or_else(|| Some(cap(f.kind.label()))), mid(&f.pts).and_then(town_at), true, bbox(&f.pts, BOUND_HALF_FT));
        }
    }
    for (id, g) in &edits.fence_gaps {
        let half = g.r as f64;
        if near_box(&g.pts, at, r + half) {
            line_now.set(Some(clip(&g.pts, at, r + half)));
            put(line_d(&g.pts, false, at) - half, "fence_gap", Some(id.clone()), Some("Gap".into()), mid(&g.pts).and_then(town_at), true, bbox(&g.pts, half));
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
    let things = pick(world, t0, [x, y], radius_ft);
    let mut v = serde_json::to_value(&things).expect("serializable");
    for (t, j) in things.iter().zip(v.as_array_mut().into_iter().flatten()) {
        if let Some(l) = t.line.as_ref().filter(|l| !l.is_empty()) {
            j["line"] = serde_json::json!(l);
        }
    }
    v.to_string()
}
