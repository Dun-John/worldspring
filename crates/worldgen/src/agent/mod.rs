//! Agent-facing queries (for `mapd`'s MCP tools): a world overview, search, a feature's
//! details and relations, its children, what is at a place, routes, and battlemap summaries.
//! Plain JSON values; names carry the world file's renames, notes come with their features.
//!
//! Ids: named features keep their overlay ids (`city:1a2b…`, `range:…`); settlement and site
//! layouts are also reachable by those; buildings are `b:<layout>:<i>`, districts
//! `d:<layout>:<q>`, wall towers `t:<layout>:<k>`, underground sites `u:`/`w:`/`k:`, created
//! sites `c:<n>`; a site's levels and rooms (for renaming) `l:<site>:<level>` and
//! `r:<site>:<level>:<room>`, levels counted from the bottom; road stretches `rd:<8 hex>`
//! (`roadids`) and crossings put down by hand `v:<id>`. NPCs (`n:<id>`) and plot points (`p:<id>`) are authored, never generated;
//! they come with the places they are tied to.

use std::borrow::Cow;

use serde_json::{Value, json};

use crate::World;
use crate::core::hash::FastMap;
use crate::t0::T0;
use crate::t0::features::{Feature, SITE_RADIUS_FT, Shape};
use crate::town::{self, geom};

mod spot;
mod buildings;
mod notes;
mod query;
mod ways;

pub use spot::*;
pub use buildings::*;
pub use notes::*;
pub use query::*;
pub use ways::*;

type P = [f64; 2];

const MI: f64 = 5280.0;
const SETTLEMENT_KINDS: [&str; 4] = ["metropolis", "city", "town", "village"];
const SITE_KINDS: [&str; 12] = ["ruin", "tower", "camp", "waystation", "cave", "mine", "lava_tube", "entrance", "building", "castle", "wall", "plot"];

thread_local! {
    /// The settlements and sites of the overlay as `(id, kind, name)` in its order, for a T0 read
    /// without the overlay (the browser's workers): `set_place_names`.
    static PLACES: std::cell::RefCell<Vec<(String, &'static str, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Gives a T0 that has no overlay (`t0.extra` None) the names and ids of the settlements and
/// sites, as JSON `[{id, kind, name}]` in the overlay's order (other kinds are ignored). With
/// the overlay at hand these are not used.
pub fn set_place_names(json: &str) {
    let v: Vec<Value> = serde_json::from_str(json).unwrap_or_default();
    let list = v
        .iter()
        .filter_map(|f| {
            let k = f["kind"].as_str()?;
            let kind = SETTLEMENT_KINDS.iter().chain(SITE_KINDS.iter()).copied().find(|x| *x == k)?;
            Some((f["id"].as_str()?.to_string(), kind, f["name"].as_str()?.to_string()))
        })
        .collect();
    PLACES.with(|p| *p.borrow_mut() = list);
}

/// The overlay's settlements and sites as `(id, kind)`: its own, or `set_place_names`'s.
fn place_ids(t0: &T0) -> Vec<(String, &'static str)> {
    match &t0.extra {
        Some(e) => e.overlay.features.iter().filter(|f| SETTLEMENT_KINDS.contains(&f.kind) || SITE_KINDS.contains(&f.kind)).map(|f| (f.id.clone(), f.kind)).collect(),
        None => PLACES.with(|p| p.borrow().iter().map(|(i, k, _)| (i.clone(), *k)).collect()),
    }
}

fn mi(ft: f64) -> f64 {
    (ft / MI * 10.0).round() / 10.0
}

/// Every named feature: the overlay's, then created sites (names as renamed, created sites
/// marked; removed ones left out).
pub fn features(world: &World, t0: &T0) -> Vec<Feature> {
    let edits = &world.file.edits;
    let mut out: Vec<Feature> = t0.extra.as_ref().map(|e| e.overlay.features.clone()).unwrap_or_default();
    for c in edits.created.iter().filter(|c| !c.removed) {
        out.push(Feature {
            id: c.id.clone(),
            // (A place, `town::members`: a village whatever its count.)
            kind: if c.kind == "place" { "village" } else { SITE_KINDS.iter().chain(&SETTLEMENT_KINDS).copied().find(|k| *k == c.kind).unwrap_or("ruin") },
            name: c.name.clone(),
            x: c.x,
            y: c.y,
            angle: 0.0,
            extent_ft: match c.kind.as_str() {
                "building" => 0.5 * MI,
                "castle" | "wall" | "plot" => (2.0 * c.reach()).max(0.5 * MI),
                "village" | "town" | "place" => if c.kind == "town" { 12.0 * MI } else { 5.0 * MI },
                _ => 2.0 * MI,
            },
            elev_ft: Some(t0.sample(c.x, c.y, t0.cell_ft).round()),
            detail: Some(if c.kind == "place" { String::new() } else { created_detail(c) }),
        });
    }
    for (id, kind, extent, people) in town_sizes(world, t0) {
        if let Some(f) = out.iter_mut().find(|f| f.id == id) {
            f.detail = Some(resized_detail(f.detail.as_deref().unwrap_or(""), f.kind, kind, people));
            (f.kind, f.extent_ft) = (kind, extent);
        }
    }
    for f in &mut out {
        if let Some(n) = edits.renames.get(&f.id) {
            f.name = n.clone();
        }
    }
    out
}

/// The towns laid out by hand, sized by the people their homes house (`hand::size`) at once, as
/// their labels, icons and lists show them: (feature id, size class, label extent ft, people).
/// T0 (its roads, its trade) keeps the settlement as it was until the world is next remade.
pub fn hand_sizes(world: &World, t0: &T0) -> Vec<(String, &'static str, f64, u32)> {
    let streets = &world.file.edits.streets;
    if streets.is_empty() {
        return Vec::new();
    }
    let ids = place_ids(t0);
    let settled: Vec<&String> = ids.iter().filter(|g| SETTLEMENT_KINDS.contains(&g.1)).map(|g| &g.0).collect();
    let first = t0.settlements.len() + t0.base_pois;
    let mut out = Vec::new();
    for li in streets.keys().filter_map(|k| k.parse::<usize>().ok()) {
        let id = if li < t0.settlements.len() { settled.get(li).map(|s| s.to_string()) } else { li.checked_sub(first).and_then(|k| world.file.edits.created.get(k)).map(|c| c.id.clone()) };
        let (Some(id), Some((people, tier))) = (id, town::hand::size(world, li)) else { continue };
        out.push((id, tier.name(), tier_extent(tier), people));
    }
    out
}

/// `hand_sizes`, and the towns whose people their members change (`town::members::sizes`: drawn
/// homes joined, generated ones pushed out) at once, and each place (a village, "pop. N").
pub fn town_sizes(world: &World, t0: &T0) -> Vec<(String, &'static str, f64, u32)> {
    let mut out = hand_sizes(world, t0);
    let first = t0.settlements.len() + t0.base_pois;
    for (li, people, tier) in town::members::sizes(world, t0) {
        let Some(id) = feature_of_layout(world, t0, li) else { continue };
        let place = li.checked_sub(first).and_then(|k| world.file.edits.created.get(k)).is_some_and(|c| c.kind == "place");
        let (kind, extent) = if place { ("village", 5.0 * MI) } else { (tier.name(), tier_extent(tier)) };
        out.retain(|o| o.0 != id);
        out.push((id, kind, extent, people));
    }
    out
}

/// A settlement's label reach (ft) by its size class, as its homes make it.
fn tier_extent(tier: crate::t0::settle::Tier) -> f64 {
    use crate::t0::settle::Tier;
    let mi = match tier {
        Tier::Metropolis => 60.0,
        Tier::City => 30.0,
        Tier::Town => 12.0,
        Tier::Village => 5.0,
    };
    mi * MI
}

/// A settlement's detail (`"<what it lives by> <size>, pop. <n>[, capital]"`) with its size
/// class and population as its homes now make them.
pub fn resized_detail(detail: &str, was: &str, now: &str, people: u32) -> String {
    let (head, tail) = match detail.split_once(", pop. ") {
        Some((h, rest)) => (h, rest.split_once(", ").map_or(String::new(), |(_, t)| format!(", {t}"))),
        None => (detail, String::new()),
    };
    // (`"<what it lives by> <size>"`, or a town added by hand's `"<size> (added by hand)"`.)
    let head = match (head.strip_suffix(was), head.strip_prefix(was)) {
        (Some(h), _) if !h.is_empty() => format!("{h}{now}"),
        (_, Some(t)) if t.starts_with(" (") => format!("{now}{t}"),
        _ => head.to_string(),
    };
    let digits = people.to_string();
    let mut n = String::new();
    for (k, ch) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k) % 3 == 0 {
            n.push(',');
        }
        n.push(ch);
    }
    if head.is_empty() { format!("pop. {n}{tail}") } else { format!("{head}, pop. {n}{tail}") }
}

/// A feature other than `id` that has the name `name` (as renamed), if any: its id.
pub fn same_named(world: &World, t0: &T0, name: &str, id: &str) -> Option<String> {
    let n = name.trim();
    (!n.is_empty()).then(|| features(world, t0).into_iter().find(|f| f.id != id && f.name.trim().eq_ignore_ascii_case(n)).map(|f| f.id)).flatten()
}

/// Each feature's shape, in `features` order: the overlay's own; a created site is a point (a
/// building reaches to its footprint's farthest corner).
pub(crate) fn shapes<'a>(world: &World, t0: &'a T0) -> Vec<Cow<'a, Shape>> {
    let mut out: Vec<Cow<Shape>> = match t0.extra.as_ref().map(|e| &e.overlay) {
        Some(o) => o.features.iter().enumerate().map(|(i, f)| o.shapes.get(i).map(Cow::Borrowed).unwrap_or(Cow::Owned(Shape::Point { at: [f.x, f.y], radius_ft: 0.0 }))).collect(),
        None => Vec::new(),
    };
    for c in world.file.edits.created.iter().filter(|c| !c.removed) {
        let r = match c.kind.as_str() {
            "building" => c.poly.iter().map(|q| geom::dist(*q, [c.x, c.y])).fold(0.0, f64::max),
            "castle" | "wall" => c.reach() - crate::world::WORKS_MARGIN_FT,
            "village" | "town" => crate::t0::made::built_radius(world, t0, &c.id),
            _ => SITE_RADIUS_FT,
        };
        out.push(Cow::Owned(Shape::Point { at: [c.x, c.y], radius_ft: r }));
    }
    out
}

fn sq_mi(sq_ft: f64) -> f64 {
    let a = sq_ft / (MI * MI);
    if a < 100.0 { (a * 100.0).round() / 100.0 } else { a.round() }
}

/// How much ground a shape covers: its bounding box (ft), and its area or length.
fn extent(s: &Shape) -> Value {
    let mut v = json!({ "bbox_ft": s.bbox().map(f64::round) });
    if let Shape::Line(_) = s {
        v["length_mi"] = json!(mi(s.length_ft()));
    } else {
        v["area_sq_mi"] = json!(sq_mi(s.area_sq_ft()));
    }
    v
}

/// A feature's extent: a drawn building's own footprint (its shape is a point reaching to the
/// farthest corner, which is not the footprint's box), the shape's box otherwise.
fn feature_extent(world: &World, f: &Feature, s: &Shape) -> Value {
    if f.kind == "building" {
        if let Some(c) = world.file.edits.created.iter().find(|c| !c.removed && c.id == f.id && c.poly.len() >= 3) {
            return poly_extent(&[&c.poly]);
        }
    }
    extent(s)
}

/// The extent of a polygon (or of several).
fn poly_extent(polys: &[&[P]]) -> Value {
    let b = polys.iter().flat_map(|p| p.iter()).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    json!({ "bbox_ft": b.map(f64::round), "area_sq_mi": sq_mi(polys.iter().map(|p| geom::area(p).abs()).sum()) })
}

/// The extent of anything with an id: a feature, a settlement, a building, a district.
fn extent_of(world: &World, t0: &T0, id: &str) -> Option<Value> {
    if deleted_with(world, t0, id).is_some() {
        return None;
    }
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    match (id.split(':').next()?, nums.as_slice()) {
        ("b", [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).building(*bi).map(|b| poly_extent(&[&b.poly])),
        ("d", [li, qi]) if *li < town::layout_count(t0) => {
            town::layout(world, t0, *li).quarters.get(*qi).map(|q| poly_extent(&q.patches.iter().map(Vec::as_slice).collect::<Vec<_>>()))
        }
        _ => {
            let i = features(world, t0).iter().position(|f| f.id == id)?;
            shapes(world, t0).get(i).map(|s| feature_extent(world, &features(world, t0)[i], s))
        }
    }
}

/// How near a feature must be to count as nearby: a tenth of its size, 2 to 10 miles.
fn reach_ft(f: &Feature) -> f64 {
    (0.1 * f.extent_ft).clamp(2.0 * MI, 10.0 * MI)
}

