//! Agent-facing queries (for `mapd`'s MCP tools): a world overview, search, a feature's
//! details and relations, its children, what is at a place, routes, and battlemap summaries.
//! Plain JSON values; names carry the world file's renames, notes come with their features.
//!
//! Ids: named features keep their overlay ids (`city:1a2b…`, `range:…`); settlement and site
//! layouts are also reachable by those; buildings are `b:<layout>:<i>`, districts
//! `d:<layout>:<q>`, wall towers `t:<layout>:<k>`, underground sites `u:`/`w:`/`k:`, created
//! sites `c:<n>`; a site's levels and rooms (for renaming) `l:<site>:<level>` and
//! `r:<site>:<level>:<room>`, levels counted from the bottom. NPCs (`n:<id>`) and plot points (`p:<id>`) are authored, never generated;
//! they come with the places they are tied to.

use std::borrow::Cow;

use serde_json::{Value, json};

use crate::World;
use crate::t0::T0;
use crate::t0::features::{Feature, SITE_RADIUS_FT, Shape};
use crate::town::{self, geom};

type P = [f64; 2];

const MI: f64 = 5280.0;
const SETTLEMENT_KINDS: [&str; 4] = ["metropolis", "city", "town", "village"];
const SITE_KINDS: [&str; 9] = ["ruin", "tower", "camp", "waystation", "cave", "mine", "lava_tube", "entrance", "building"];

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
            kind: SITE_KINDS.iter().copied().find(|k| *k == c.kind).unwrap_or("ruin"),
            name: c.name.clone(),
            x: c.x,
            y: c.y,
            angle: 0.0,
            extent_ft: if c.kind == "building" { 0.5 * MI } else { 2.0 * MI },
            elev_ft: Some(t0.sample(c.x, c.y, t0.cell_ft).round()),
            detail: Some(created_detail(c)),
        });
    }
    for f in &mut out {
        if let Some(n) = edits.renames.get(&f.id) {
            f.name = n.clone();
        }
    }
    out
}

/// Each feature's shape, in `features` order: the overlay's own; a created site is a point (a
/// building reaches to its footprint's farthest corner).
fn shapes<'a>(world: &World, t0: &'a T0) -> Vec<Cow<'a, Shape>> {
    let mut out: Vec<Cow<Shape>> = match t0.extra.as_ref().map(|e| &e.overlay) {
        Some(o) => o.features.iter().enumerate().map(|(i, f)| o.shapes.get(i).map(Cow::Borrowed).unwrap_or(Cow::Owned(Shape::Point { at: [f.x, f.y], radius_ft: 0.0 }))).collect(),
        None => Vec::new(),
    };
    for c in world.file.edits.created.iter().filter(|c| !c.removed) {
        let r = if c.kind == "building" { c.poly.iter().map(|q| geom::dist(*q, [c.x, c.y])).fold(0.0, f64::max) } else { SITE_RADIUS_FT };
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

/// The extent of a polygon (or of several).
fn poly_extent(polys: &[&[P]]) -> Value {
    let b = polys.iter().flat_map(|p| p.iter()).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    json!({ "bbox_ft": b.map(f64::round), "area_sq_mi": sq_mi(polys.iter().map(|p| geom::area(p).abs()).sum()) })
}

/// The extent of anything with an id: a feature, a settlement, a building, a district.
fn extent_of(world: &World, t0: &T0, id: &str) -> Option<Value> {
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    match (id.split(':').next()?, nums.as_slice()) {
        ("b", [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).buildings.get(*bi).map(|b| poly_extent(&[&b.poly])),
        ("d", [li, qi]) if *li < town::layout_count(t0) => {
            town::layout(world, t0, *li).quarters.get(*qi).map(|q| poly_extent(&q.patches.iter().map(Vec::as_slice).collect::<Vec<_>>()))
        }
        _ => {
            let i = features(world, t0).iter().position(|f| f.id == id)?;
            shapes(world, t0).get(i).map(|s| extent(s))
        }
    }
}

/// How near a feature must be to count as nearby: a tenth of its size, 2 to 10 miles.
fn reach_ft(f: &Feature) -> f64 {
    (0.1 * f.extent_ft).clamp(2.0 * MI, 10.0 * MI)
}

/// What a created site is: "created ruin over a crypt (tomb, large, 3 levels)".
pub fn created_detail(c: &crate::world::Created) -> String {
    if c.kind == "building" {
        // "created building (inn, 2 floors, battlements, slate roof)"
        let what = c.func.as_deref().map(|f| town::catalog::index_of(f).map_or(f.replace('_', " "), |i| town::catalog::CATALOG[i].name.to_string()));
        let floors = c.floors.map(|f| format!("{f} floor{}", if f == 1 { "" } else { "s" }));
        let roof = c.roof.as_ref().map(|r| if r == "hip" { "hip roof".to_string() } else { r.clone() });
        let tint = c.tint.as_ref().map(|t| format!("{t} roof"));
        let ruin = (c.structure.as_deref() == Some("ruin")).then(|| "ruined".to_string());
        let opts: Vec<String> = [what, floors, ruin, roof, tint].into_iter().flatten().collect();
        return if opts.is_empty() { "created building".into() } else { format!("created building ({})", opts.join(", ")) };
    }
    let mut s = format!("created {}", c.kind.replace('_', " "));
    if let Some(u) = &c.under {
        s += &format!(" over a {}", u.replace('_', " "));
    }
    let opts: Vec<String> = [c.theme.as_ref().map(|t| t.replace('_', " ")), c.size.clone(), c.levels.map(|l| format!("{l} level{}", if l == 1 { "" } else { "s" }))].into_iter().flatten().collect();
    if !opts.is_empty() {
        s += &format!(" ({})", opts.join(", "));
    }
    s
}

/// Where a site of `kind` can be created near `p`: on dry land inside the map, stepped out of
/// river channels (as generated sites are); with a name for it (the nearest settlement's naming
/// culture's), seeded by its id. The app and mapd both place sites through this.
pub fn creation_spot(world: &World, t0: &T0, kind: &str, under: Option<&str>, id: &str, p: [f64; 2]) -> Result<([f64; 2], String), String> {
    use crate::t0::names::{NameKind, Namer};
    use crate::under::UnderKind;
    if p[0] < 0.0 || p[1] < 0.0 || p[0] > world.geom.map_w_ft || p[1] > world.geom.map_h_ft {
        return Err("that place is off the map".to_string());
    }
    let (x, y) = crate::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 150.0, t0.cell_ft);
    let mut p = [x, y];
    if kind == "camp" {
        // A camp's clearing lies within one battlemap chunk (so it is levelled to the ground
        // there): kept 70 ft from the chunk's edges.
        let size = world.geom.tile_size_ft(world.geom.max_level);
        p = p.map(|v| crate::core::floor(v / size) * size + (v - crate::core::floor(v / size) * size).clamp(70.0, size - 70.0));
    }
    let ground = t0.sample(p[0], p[1], 20.0);
    if (t0.sample_water(p[0], p[1]) as f64) >= ground || ground <= world.params().sea_level_ft {
        return Err("that place is under water: pick dry land".to_string());
    }
    let culture = t0.settlements.iter().min_by(|a, b| (a.x - p[0]).hypot(a.y - p[1]).total_cmp(&(b.x - p[0]).hypot(b.y - p[1]))).map(|s| s.culture as usize).unwrap_or(0);
    let nk = match (kind, under.and_then(UnderKind::parse)) {
        ("tower", _) => NameKind::Tower,
        ("camp", _) => NameKind::Camp,
        ("waystation", _) => NameKind::Settlement,
        ("cave", _) | ("entrance", Some(UnderKind::Cave)) => NameKind::Cave,
        ("mine", _) | ("entrance", Some(UnderKind::Mine)) => NameKind::Mine,
        ("lava_tube", _) | ("entrance", Some(UnderKind::LavaTube)) => NameKind::LavaTube,
        _ => NameKind::Ruin,
    };
    let n = Namer::new(crate::core::hash::fnv64(id.as_bytes()) ^ world.seed).name(nk, culture);
    Ok((p, if kind == "waystation" { format!("{n} Inn") } else { n }))
}

/// Where a building drawn by hand may stand: on the map, on dry land out of river channels,
/// clear of other buildings (sharing a wall is fine), roads, streets and town walls. `skip`:
/// the layout it already has (reshaping one). With its point (the footprint's middle) and a
/// name for it: its trade's in the nearest settlement's culture, seeded by its id (else what
/// it is: "House").
pub fn building_spot(world: &World, t0: &T0, poly: &[P], func: Option<&str>, id: &str, skip: Option<usize>) -> Result<(P, String), String> {
    use crate::core::rng::Pcg32;
    if poly.len() < 3 {
        return Err("a building needs at least 3 corners".to_string());
    }
    if poly.iter().any(|p| p[0] < 0.0 || p[1] < 0.0 || p[0] > world.geom.map_w_ft || p[1] > world.geom.map_h_ft) {
        return Err("that place is off the map".to_string());
    }
    let c = geom::centroid(poly);
    // Corners, the middle and every 10 ft along the walls must be dry.
    let mut probes = vec![c];
    for (k, &a) in poly.iter().enumerate() {
        let b = poly[(k + 1) % poly.len()];
        let n = (geom::dist(a, b) / 10.0).ceil().max(1.0) as usize;
        probes.extend((0..n).map(|s| geom::lerp(a, b, s as f64 / n as f64)));
    }
    let sea = world.params().sea_level_ft;
    for p in &probes {
        let ground = t0.sample(p[0], p[1], 5.0);
        if (t0.sample_water(p[0], p[1]) as f64) >= ground || ground <= sea {
            return Err("that is in the water: draw it on dry land".to_string());
        }
        let q = crate::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 2.0, t0.cell_ft);
        if (q.0 - p[0]).abs() + (q.1 - p[1]).abs() > 0.01 {
            return Err("that is in a river: draw it on the bank".to_string());
        }
    }
    let (x0, y0) = poly.iter().fold((f64::MAX, f64::MAX), |a, p| (a.0.min(p[0]), a.1.min(p[1])));
    let (x1, y1) = poly.iter().fold((f64::MIN, f64::MIN), |a, p| (a.0.max(p[0]), a.1.max(p[1])));
    // A foot in from every side, so walls may touch.
    let inner: Vec<P> = poly.iter().map(|p| geom::lerp(*p, c, 1.0 / geom::dist(*p, c).max(1.0))).collect();
    for l in town::layouts_near(world, t0, [x0, y0, x1, y1]) {
        if Some(l.index as usize) == skip {
            continue;
        }
        if let Some(bi) = l.buildings.iter().position(|b| polys_overlap(&inner, &b.poly)) {
            return Err(format!("that overlaps {}", building_name(world, &l, bi)));
        }
        let streets = l.roads.iter().map(|(pts, _, w)| (pts, 0.5 * w)).chain(l.walls.iter().map(|w| (w, 4.5)));
        for (pts, half) in streets {
            if pts.windows(2).any(|s| poly_seg_dist(&inner, s[0], s[1]) < half) {
                return Err("that is on a street or a wall".to_string());
            }
        }
    }
    for (ri, k) in t0.roads.segments_near([x0, y0, x1, y1], 40.0) {
        let rc = &t0.roads.roads[ri as usize];
        let half = 0.5 * rc.class.width_ft();
        let pts: Vec<P> = (0..=16).map(|j| rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p).collect();
        if pts.windows(2).any(|s| poly_seg_dist(&inner, s[0], s[1]) < half) {
            return Err("that is on a road".to_string());
        }
    }
    let name = match func.and_then(town::catalog::index_of) {
        Some(fi) => {
            let f = &town::catalog::CATALOG[fi];
            let culture = t0.settlements.iter().min_by(|a, b| (a.x - c[0]).hypot(a.y - c[1]).total_cmp(&(b.x - c[0]).hypot(b.y - c[1]))).map_or(0, |s| s.culture as usize);
            let mut rng = Pcg32::new(crate::core::hash::fnv64(id.as_bytes()) ^ world.seed, 43);
            let mut namer = crate::t0::names::Namer::new(rng.next_u32() as u64);
            town::business_name(f, &mut rng, &mut namer, culture).unwrap_or_else(|| f.name.to_string())
        }
        None => {
            let home = func.and_then(|f| crate::world::BUILDING_HOMES.iter().position(|h| *h == f)).unwrap_or(1);
            town::catalog::RESIDENTIAL[home].to_string()
        }
    };
    let mut cs = name.chars();
    let name = cs.next().map(|f| f.to_uppercase().chain(cs).collect()).unwrap_or(name);
    Ok((c, name))
}

/// What a building drawn by hand can be: businesses (catalog key, name, category) and homes.
pub fn building_funcs_json() -> String {
    let businesses: Vec<Value> = town::catalog::CATALOG.iter().map(|f| json!({ "key": f.key, "name": f.name, "category": f.category })).collect();
    let homes: Vec<Value> = crate::world::BUILDING_HOMES.iter().zip(town::catalog::RESIDENTIAL).map(|(k, n)| json!({ "key": k, "name": n })).collect();
    json!({ "businesses": businesses, "homes": homes }).to_string()
}

/// `building_spot` as JSON: `{x, y, name}` or `{error}`.
pub fn building_spot_json(world: &World, t0: &T0, poly: &[P], func: Option<&str>, id: &str) -> String {
    let skip = layout_of(world, t0, id);
    match building_spot(world, t0, poly, func, id, skip) {
        Ok((q, name)) => json!({ "x": q[0], "y": q[1], "name": name }),
        Err(e) => json!({ "error": e }),
    }
    .to_string()
}

/// Whether two polygons share any area (or one holds the other).
fn polys_overlap(a: &[P], b: &[P]) -> bool {
    a.iter().any(|p| geom::contains(b, *p)) || b.iter().any(|p| geom::contains(a, *p)) || (0..a.len()).any(|i| (0..b.len()).any(|j| segs_cross(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()])))
}

fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (d1, d2) = (geom::cross(geom::sub(b, a), geom::sub(c, a)), geom::cross(geom::sub(b, a), geom::sub(d, a)));
    let (d3, d4) = (geom::cross(geom::sub(d, c), geom::sub(a, c)), geom::cross(geom::sub(d, c), geom::sub(b, c)));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Distance from a segment to a polygon (0 when it crosses or lies inside).
fn poly_seg_dist(poly: &[P], a: P, b: P) -> f64 {
    let n = poly.len();
    if geom::contains(poly, a) || geom::contains(poly, b) || (0..n).any(|i| segs_cross(poly[i], poly[(i + 1) % n], a, b)) {
        return 0.0;
    }
    let to_edges = (0..n).flat_map(|i| [geom::seg_dist(a, poly[i], poly[(i + 1) % n]), geom::seg_dist(b, poly[i], poly[(i + 1) % n])]);
    poly.iter().map(|p| geom::seg_dist(*p, a, b)).chain(to_edges).fold(f64::MAX, f64::min)
}

/// `creation_spot` as JSON: `{x, y, name}` or `{error}`.
pub fn creation_spot_json(world: &World, t0: &T0, kind: &str, under: Option<&str>, id: &str, p: [f64; 2]) -> String {
    match creation_spot(world, t0, kind, under, id, p) {
        Ok((q, name)) => json!({ "x": q[0], "y": q[1], "name": name }),
        Err(e) => json!({ "error": e }),
    }
    .to_string()
}

/// The layout index of a settlement or site feature (by overlay order; created sites follow
/// the generated ones).
pub fn layout_of(world: &World, t0: &T0, id: &str) -> Option<usize> {
    if let Some(k) = world.file.edits.created.iter().position(|c| c.id == id) {
        return Some(t0.settlements.len() + t0.base_pois + k);
    }
    let fs = &t0.extra.as_ref()?.overlay.features;
    let f = fs.iter().find(|f| f.id == id)?;
    if SETTLEMENT_KINDS.contains(&f.kind) {
        fs.iter().filter(|g| SETTLEMENT_KINDS.contains(&g.kind)).position(|g| g.id == id)
    } else if SITE_KINDS.contains(&f.kind) {
        fs.iter().filter(|g| SITE_KINDS.contains(&g.kind)).position(|g| g.id == id).map(|k| t0.settlements.len() + k)
    } else {
        None
    }
}

/// The feature id of layout `li` (a settlement or site).
pub fn feature_of_layout(world: &World, t0: &T0, li: usize) -> Option<String> {
    let n = t0.settlements.len();
    if li >= n + t0.base_pois {
        return world.file.edits.created.get(li - n - t0.base_pois).map(|c| c.id.clone());
    }
    let fs = &t0.extra.as_ref()?.overlay.features;
    if li < n {
        fs.iter().filter(|g| SETTLEMENT_KINDS.contains(&g.kind)).nth(li).map(|f| f.id.clone())
    } else {
        fs.iter().filter(|g| SITE_KINDS.contains(&g.kind)).nth(li - n).map(|f| f.id.clone())
    }
}

fn name_of(world: &World, t0: &T0, id: &str) -> Option<String> {
    if let Some(n) = world.file.edits.renames.get(id) {
        return Some(n.clone());
    }
    features(world, t0).into_iter().find(|f| f.id == id).map(|f| f.name)
}

/// A building's display name (renamed, its own, or its trade).
fn building_name(world: &World, l: &town::Layout, bi: usize) -> String {
    let id = format!("b:{}:{bi}", l.index);
    world.file.edits.renames.get(&id).cloned().or_else(|| l.buildings[bi].name.clone()).unwrap_or_else(|| l.buildings[bi].label().to_string())
}

/// Where an id is (ft), for features, buildings, districts, towers, underground sites.
pub fn position(world: &World, t0: &T0, id: &str) -> Option<P> {
    let edits = &world.file.edits;
    // An NPC is where they were placed; a plot point at its first place.
    if let Some(n) = edits.npcs.get(id) {
        let l = n.location.as_ref()?;
        return match (l.x, l.y) {
            (Some(x), Some(y)) => Some([x, y]),
            _ => position(world, t0, &l.id).filter(|_| !is_authored(&l.id)),
        };
    }
    if let Some(p) = edits.plots.get(id) {
        return p.anchors.iter().filter(|a| !is_authored(a)).find_map(|a| position(world, t0, a));
    }
    // A level is where its site is; a room at its middle.
    if let Some((site, li, ri)) = split_room_id(id) {
        let it = crate::interior::generate_id(world, t0, site)?;
        let lv = it.levels.get(li)?;
        return match ri {
            Some(ri) => lv.rooms.get(ri).map(|r| crate::interior::to_world(&it, r.center[0] as f64, r.center[1] as f64)),
            None => position(world, t0, site),
        };
    }
    let mut parts = id.split(':');
    let head = parts.next()?;
    let nums: Vec<usize> = parts.filter_map(|p| p.parse().ok()).collect();
    match (head, nums.as_slice()) {
        ("b", [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).buildings.get(*bi).map(|b| geom::centroid(&b.poly)),
        ("d", [li, qi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).quarters.get(*qi).map(|q| q.label[q.label.len() / 2]),
        ("t", [li, k]) if *li < town::layout_count(t0) => crate::interior::towers(&town::layout(world, t0, *li)).get(*k).map(|t| t.0),
        ("u", [li, k]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).entrances.get(*k).map(|e| e.at),
        ("k", [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).buildings.get(*bi).map(|b| geom::centroid(&b.poly)),
        ("w", [_, sx, sy]) => Some([(*sx as f64 + 0.5) * crate::under::SEWER_SECTION_FT, (*sy as f64 + 0.5) * crate::under::SEWER_SECTION_FT]),
        _ => features(world, t0).into_iter().find(|f| f.id == id).map(|f| [f.x, f.y]),
    }
}

/// A level's or room's id split into its site, level and room: `l:<site>:<level>`,
/// `r:<site>:<level>:<room>`.
pub fn split_room_id(id: &str) -> Option<(&str, usize, Option<usize>)> {
    if let Some(rest) = id.strip_prefix("l:") {
        let (site, lv) = rest.rsplit_once(':')?;
        return Some((site, lv.parse().ok()?, None));
    }
    let (head, room) = id.strip_prefix("r:")?.rsplit_once(':')?;
    let (site, lv) = head.rsplit_once(':')?;
    Some((site, lv.parse().ok()?, Some(room.parse().ok()?)))
}

/// NPC and plot ids (which are not places themselves).
pub fn is_authored(id: &str) -> bool {
    id.starts_with("n:") || id.starts_with("p:")
}

/// An NPC as agents see it (`brief`: name, stance and where only).
pub fn npc_json(world: &World, t0: &T0, id: &str, brief: bool) -> Option<Value> {
    let n = world.file.edits.npcs.get(id)?;
    let at = n.location.as_ref().map(|l| {
        let mut v = json!({ "id": l.id, "name": name_any(world, t0, &l.id) });
        if let Some(lv) = l.level {
            v["level"] = json!(lv);
        }
        if let (Some(x), Some(y)) = (l.x, l.y) {
            v["x_ft"] = json!(x.round());
            v["y_ft"] = json!(y.round());
        }
        v
    });
    if brief {
        return Some(json!({ "id": id, "name": n.name, "stance": n.attitude.stance, "status": n.status, "location": at, "tags": n.tags }));
    }
    let plots: Vec<Value> = world.file.edits.plots.iter().filter(|(_, p)| p.npcs.iter().any(|k| k == id)).map(|(k, p)| json!({ "id": k, "title": p.title, "status": p.status })).collect();
    Some(json!({
        "id": id,
        "name": n.name,
        "appearance": n.appearance,
        "mannerisms": n.mannerisms,
        "attitude": n.attitude,
        "goals": n.goals,
        "notes": n.notes,
        "tags": n.tags,
        "portrait": n.portrait,
        "status": n.status,
        "location": at,
        "plots": plots,
    }))
}

/// A plot point as agents see it, its places and NPCs named.
pub fn plot_json(world: &World, t0: &T0, id: &str, brief: bool) -> Option<Value> {
    let p = world.file.edits.plots.get(id)?;
    let anchors: Vec<Value> = p.anchors.iter().map(|a| json!({ "id": a, "name": name_any(world, t0, a) })).collect();
    let npcs: Vec<Value> = p.npcs.iter().map(|k| json!({ "id": k, "name": world.file.edits.npcs.get(k).map(|n| n.name.clone()) })).collect();
    let mut v = json!({ "id": id, "title": p.title, "status": p.status, "anchors": anchors, "npcs": npcs, "tags": p.tags });
    if !brief {
        v["text"] = json!(p.text);
    }
    Some(v)
}

/// A place by id, for the app's notebook: its name (as renamed) and where it is.
pub fn place(world: &World, t0: &T0, id: &str) -> Option<Value> {
    let p = position(world, t0, id)?;
    Some(json!({ "id": id, "name": name_any(world, t0, id), "generated": generated_name(world, t0, id), "x": p[0], "y": p[1] }))
}

/// Any id's display name: a feature, building, district, NPC or plot (else null).
fn name_any(world: &World, t0: &T0, id: &str) -> Option<String> {
    let edits = &world.file.edits;
    if let Some(n) = edits.npcs.get(id) {
        return Some(n.name.clone());
    }
    if let Some(p) = edits.plots.get(id) {
        return Some(p.title.clone());
    }
    if let Some(n) = edits.renames.get(id) {
        return Some(n.clone());
    }
    generated_name(world, t0, id)
}

/// A place's name before renames (a building's own name or its trade, a level's or room's
/// generated name, a feature's overlay name).
fn generated_name(world: &World, t0: &T0, id: &str) -> Option<String> {
    if let Some((site, li, ri)) = split_room_id(id) {
        let it = crate::interior::generate_id(world, t0, site)?;
        let lv = it.levels.get(li)?;
        return match ri {
            Some(ri) => lv.rooms.get(ri).map(|r| r.kind.to_string()),
            None => Some(lv.name.clone()),
        };
    }
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    match (id.split(':').next(), nums.as_slice()) {
        (Some("b"), [li, bi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).buildings.get(*bi).map(|b| b.name.clone().unwrap_or_else(|| b.label().to_string())),
        (Some("d"), [li, qi]) if *li < town::layout_count(t0) => town::layout(world, t0, *li).quarters.get(*qi).map(|q| q.name.clone()),
        _ => match world.file.edits.created.iter().find(|c| c.id == id) {
            Some(c) => Some(c.name.clone()),
            None => t0.extra.as_ref()?.overlay.features.iter().find(|f| f.id == id).map(|f| f.name.clone()),
        },
    }
}

/// The NPCs and plot points at the places `ids` (each with the place it is at). A layout's
/// feature id also takes in what is in its buildings, districts, towers and sites.
fn authored_at(world: &World, t0: &T0, ids: &[String]) -> (Vec<Value>, Vec<Value>) {
    let edits = &world.file.edits;
    if edits.npcs.is_empty() && edits.plots.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let prefixes: Vec<String> = ids.iter().filter_map(|id| layout_of(world, t0, id)).flat_map(|li| ["b", "d", "t", "u", "k", "w"].map(|h| format!("{h}:{li}:"))).collect();
    let here = |a: &str| ids.iter().any(|i| i == a) || prefixes.iter().any(|p| a.starts_with(p.as_str()));
    let npcs = edits.npcs.iter().filter(|(_, n)| n.location.as_ref().is_some_and(|l| here(&l.id))).filter_map(|(k, _)| npc_json(world, t0, k, true)).collect();
    let plots = edits.plots.iter().filter(|(_, p)| p.anchors.iter().any(|a| here(a))).filter_map(|(k, _)| plot_json(world, t0, k, true)).collect();
    (npcs, plots)
}

fn note_of(world: &World, id: &str) -> Value {
    world.file.edits.notes.get(id).map(|n| json!({ "text": n.text, "tags": n.tags })).unwrap_or(Value::Null)
}

fn compass(from: P, to: P) -> &'static str {
    let a = libm::atan2(to[1] - from[1], to[0] - from[0]).to_degrees();
    // Screen y points south.
    ["E", "SE", "S", "SW", "W", "NW", "N", "NE"][((((a + 22.5) % 360.0) + 360.0) % 360.0 / 45.0) as usize % 8]
}

/// The areas a point lies in (land or sea, ranges, forests, lakes...), largest first.
fn regions_at(fs: &[Feature], shapes: &[Cow<Shape>], p: P) -> Vec<Value> {
    let mut out: Vec<(f64, &Feature)> =
        fs.iter().zip(shapes).filter(|(_, s)| matches!(***s, Shape::Area { .. }) && s.distance(p).0 == 0.0).map(|(f, s)| (s.area_sq_ft(), f)).collect();
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().map(|(_, f)| json!({ "id": f.id, "kind": f.kind, "name": f.name })).collect()
}

fn nearest_settlements(fs: &[Feature], p: P, skip: &str, n: usize) -> Vec<Value> {
    let mut v: Vec<&Feature> = fs.iter().filter(|f| SETTLEMENT_KINDS.contains(&f.kind) && f.id != skip).collect();
    v.sort_by(|a, b| geom::dist([a.x, a.y], p).total_cmp(&geom::dist([b.x, b.y], p)));
    v.into_iter()
        .take(n)
        .map(|f| json!({ "id": f.id, "name": f.name, "kind": f.kind, "distance_mi": mi(geom::dist([f.x, f.y], p)), "direction": compass(p, [f.x, f.y]) }))
        .collect()
}

/// A world overview: its size, the land, counts of named features, the largest settlements.
pub fn overview(world: &World, t0: &T0) -> Value {
    let fs = features(world, t0);
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for f in &fs {
        *counts.entry(f.kind).or_default() += 1;
    }
    let mut towns: Vec<&Feature> = fs.iter().filter(|f| SETTLEMENT_KINDS.contains(&f.kind)).collect();
    // "…, pop. 12,345, capital": the digits after "pop. " (thousands commas included).
    let pop = |f: &Feature| -> u64 {
        let d = f.detail.as_deref().and_then(|d| d.split("pop. ").nth(1)).unwrap_or("");
        d.chars().take_while(|c| c.is_ascii_digit() || *c == ',').filter(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0)
    };
    towns.sort_by_key(|f| std::cmp::Reverse(pop(f)));
    let p = world.params();
    json!({
        "seed": world.file.seed,
        "size_mi": [p.width_mi, p.height_mi],
        "map_origin": "x east and y south, in feet from the map's top-left corner (5,280 ft a mile)",
        "land": fs.iter().filter(|f| matches!(f.kind, "continent" | "island")).map(|f| json!({ "id": f.id, "kind": f.kind, "name": f.name })).collect::<Vec<_>>(),
        "feature_counts": counts,
        "largest_settlements": towns.iter().take(12).map(|f| json!({ "id": f.id, "name": f.name, "kind": f.kind, "detail": f.detail, "x_ft": f.x.round(), "y_ft": f.y.round() })).collect::<Vec<_>>(),
        "created_sites": world.file.edits.created.iter().filter(|c| !c.removed).count(),
        "notes": world.file.edits.notes.len(),
        "npcs": world.file.edits.npcs.len(),
        "plots": world.file.edits.plots.len(),
    })
}

/// Named features (and, from three letters, districts and businesses) matching a query, by
/// their current names, each with its extent.
pub fn search(world: &World, t0: &T0, q: &str, kind: Option<&str>, limit: usize, include_hidden: bool) -> Value {
    let ql = q.trim().to_lowercase();
    let hidden = &world.file.edits.hidden;
    let shapes = shapes(world, t0);
    let mut out: Vec<Value> = features(world, t0)
        .into_iter()
        .zip(&shapes)
        .filter(|(f, _)| (ql.is_empty() || f.name.to_lowercase().contains(&ql)) && kind.is_none_or(|k| f.kind == k) && (include_hidden || !hidden.contains(&f.id)))
        .take(limit)
        .map(|(f, s)| json!({ "id": f.id, "kind": f.kind, "name": f.name, "detail": f.detail, "x_ft": f.x.round(), "y_ft": f.y.round(), "extent": extent(s) }))
        .collect();
    if ql.len() >= 3 && out.len() < limit && kind.is_none_or(|k| matches!(k, "building" | "district")) {
        for h in crate::gazetteer::search_buildings(world, t0, &ql, limit - out.len(), None, include_hidden) {
            out.push(hit_json(world, t0, &h));
        }
    }
    json!({ "results": out })
}

/// A district or building found in a layout, with its extent.
fn hit_json(world: &World, t0: &T0, h: &crate::gazetteer::Hit) -> Value {
    let mut v = with_rename(world, serde_json::to_value(h).unwrap_or(Value::Null));
    if let Some(e) = v["id"].as_str().and_then(|id| extent_of(world, t0, id)) {
        v["extent"] = e;
    }
    v
}

fn with_rename(world: &World, mut v: Value) -> Value {
    if let Some(id) = v["id"].as_str()
        && let Some(n) = world.file.edits.renames.get(id)
    {
        v["name"] = json!(n);
    }
    v
}

/// Everything named within `radius` ft of a point (inside it, for an area), nearest first:
/// features, and districts and businesses when `kinds` asks for them (or, with no kinds,
/// within a mile). Each with its distance, direction and extent.
pub fn near(world: &World, t0: &T0, p: P, radius: f64, kinds: &[String], limit: usize, include_hidden: bool) -> Value {
    let hidden = &world.file.edits.hidden;
    let wants = |k: &str| kinds.is_empty() || kinds.iter().any(|w| w == k);
    let fs = features(world, t0);
    let mut out: Vec<(f64, Value)> = fs
        .iter()
        .zip(&shapes(world, t0))
        .filter(|(f, _)| wants(f.kind) && (include_hidden || !hidden.contains(&f.id)))
        .filter_map(|(f, s)| {
            let (d, at) = s.distance(p);
            (d <= radius).then(|| {
                let dir = if d > 0.0 { compass(p, at) } else { "here" };
                (d, json!({ "id": f.id, "kind": f.kind, "name": f.name, "detail": f.detail, "distance_mi": mi(d), "direction": dir, "x_ft": f.x.round(), "y_ft": f.y.round(), "extent": extent(s) }))
            })
        })
        .collect();
    let towns = if kinds.is_empty() { radius <= MI } else { wants("district") || wants("building") };
    if towns {
        let r = radius.min(5.0 * MI);
        for h in crate::gazetteer::in_view(world, t0, [p[0] - r, p[1] - r, p[0] + r, p[1] + r], 2_000) {
            let mut v = hit_json(world, t0, &h);
            let (kind, id) = (v["kind"].as_str().unwrap_or("").to_string(), v["id"].as_str().unwrap_or("").to_string());
            let at = [v["x"].as_f64().unwrap_or(0.0), v["y"].as_f64().unwrap_or(0.0)];
            let d = geom::dist(at, p);
            if d <= radius && wants(&kind) && (include_hidden || !hidden.contains(&id)) {
                v["distance_mi"] = json!(mi(d));
                v["direction"] = json!(if d > 0.0 { compass(p, at) } else { "here" });
                out.push((d, v));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total = out.len();
    json!({ "x_ft": p[0].round(), "y_ft": p[1].round(), "radius_mi": mi(radius), "found": total, "results": out.into_iter().take(limit).map(|(_, v)| v).collect::<Vec<_>>() })
}

/// A feature's details: what it is, where, its notes, and how it relates to the world around
/// it (the regions it lies in, the nearest settlements, roads; a settlement's districts and
/// notable buildings; a building's floors; an underground site's levels).
pub fn get(world: &World, t0: &T0, id: &str) -> Option<Value> {
    let fs = features(world, t0);
    let p = position(world, t0, id)?;
    let ground = t0.sample(p[0], p[1], t0.cell_ft);
    let mut v = json!({
        "id": id,
        "x_ft": p[0].round(),
        "y_ft": p[1].round(),
        "elevation_ft": (ground - world.params().sea_level_ft).round(),
        "hidden": world.file.edits.hidden.contains(id),
        "notes": note_of(world, id),
        "in": regions_at(&fs, &shapes(world, t0), p),
        "nearest_settlements": nearest_settlements(&fs, p, id, 3),
    });
    if let Some(e) = extent_of(world, t0, id) {
        v["extent"] = e;
    }
    if let Some(f) = fs.iter().find(|f| f.id == id) {
        v["kind"] = json!(f.kind);
        v["name"] = json!(f.name);
        v["detail"] = json!(f.detail);
    }
    let head = id.split(':').next().unwrap_or("");
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    if let Some(li) = layout_of(world, t0, id) {
        let l = town::layout(world, t0, li);
        v["layout"] = layout_summary(world, t0, &l);
        if li < t0.settlements.len() {
            v["roads_to"] = json!(roads_from(world, t0, li));
        }
    } else if head == "b" && nums.len() == 2 {
        let l = town::layout(world, t0, nums[0]);
        let b = l.buildings.get(nums[1])?;
        v["kind"] = json!("building");
        v["name"] = json!(building_name(world, &l, nums[1]));
        v["function"] = json!(b.label());
        v["floors"] = json!(b.floors);
        v["settlement"] = json!({ "id": feature_of_layout(world, t0, nums[0]), "name": feature_of_layout(world, t0, nums[0]).and_then(|f| name_of(world, t0, &f)) });
        if let Some(it) = crate::interior::generate_id(world, t0, id) {
            v["interior"] = interior_summary(world, &it);
        }
    } else if head == "d" && nums.len() == 2 {
        let l = town::layout(world, t0, nums[0]);
        let q = l.quarters.get(nums[1])?;
        v["kind"] = json!("district");
        v["name"] = json!(world.file.edits.renames.get(id).cloned().unwrap_or_else(|| q.name.clone()));
        v["district_kind"] = json!(q.kind.name());
        v["settlement"] = json!({ "id": feature_of_layout(world, t0, nums[0]) });
    } else if matches!(head, "u" | "w" | "k" | "t") {
        let it = crate::interior::generate_id(world, t0, id)?;
        v["kind"] = json!(it.function);
        v["name"] = json!(site_name(world, &it));
        v["interior"] = interior_summary(world, &it);
    } else if let Some((site, ..)) = split_room_id(id) {
        v["kind"] = json!(if head == "l" { "level" } else { "room" });
        v["name"] = json!(name_any(world, t0, id));
        v["site"] = json!(site);
    }
    let (npcs, plots) = authored_at(world, t0, &[id.to_string()]);
    v["npcs"] = json!(npcs);
    v["plots"] = json!(plots);
    Some(v)
}

/// A settlement's or site's layout: districts, notable buildings, ways underground.
fn layout_summary(world: &World, t0: &T0, l: &town::Layout) -> Value {
    let notable: Vec<Value> = l
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, b)| b.func.is_some())
        .take(60)
        .map(|(bi, b)| json!({ "id": format!("b:{}:{bi}", l.index), "name": building_name(world, l, bi), "function": b.label() }))
        .collect();
    let entrances: Vec<Value> = l
        .entrances
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind != crate::under::UnderKind::Sewer)
        .map(|(k, e)| json!({ "id": format!("u:{}:{k}", l.index), "kind": e.kind.name(), "x_ft": e.at[0].round(), "y_ft": e.at[1].round() }))
        .collect();
    let sewers = l.entrances.iter().filter(|e| e.kind == crate::under::UnderKind::Sewer).count();
    let _ = t0;
    json!({
        "tier": l.tier.name(),
        "buildings": l.buildings.len(),
        "walled": !l.walls.is_empty(),
        "districts": l.quarters.iter().enumerate().map(|(qi, q)| json!({ "id": format!("d:{}:{qi}", l.index), "name": world.file.edits.renames.get(&format!("d:{}:{qi}", l.index)).cloned().unwrap_or_else(|| q.name.clone()), "kind": q.kind.name() })).collect::<Vec<_>>(),
        "notable_buildings": notable,
        "underground": entrances,
        "sewer_grates": sewers,
    })
}

/// A building's or site's name: renamed, its own, or what it is.
fn site_name(world: &World, it: &crate::interior::Interior) -> String {
    world.file.edits.renames.get(&it.id).cloned().or_else(|| it.name.clone()).unwrap_or_else(|| it.function.to_string())
}

/// A building's or site's levels with their rooms (counted by kind; renamed rooms by name).
fn interior_summary(world: &World, it: &crate::interior::Interior) -> Value {
    let renames = &world.file.edits.renames;
    json!({
        "id": it.id,
        "name": site_name(world, it),
        "function": it.function,
        "size_ft": [it.nx * 5, it.ny * 5],
        "levels": it.levels.iter().enumerate().map(|(li, lv)| {
            let mut rooms: std::collections::BTreeMap<&str, usize> = Default::default();
            for r in lv.rooms.iter().filter(|r| r.squares > 0) {
                *rooms.entry(r.kind).or_default() += 1;
            }
            let lid = format!("l:{}:{li}", it.id);
            let named: Vec<Value> = (0..lv.rooms.len())
                .filter_map(|ri| {
                    let rid = format!("r:{}:{li}:{ri}", it.id);
                    renames.get(&rid).map(|n| json!({ "id": rid, "name": n, "kind": lv.rooms[ri].kind }))
                })
                .collect();
            json!({
                "id": lid,
                "name": renames.get(&lid).unwrap_or(&lv.name),
                "rooms": rooms,
                "named_rooms": named,
                "hazards": lv.furniture.iter().filter_map(|f| f.hazard.map(|h| format!("{}: {h}", f.name))).collect::<std::collections::BTreeSet<_>>(),
                "ways_to_other_sites": lv.links.iter().map(|k| &k.to).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    })
}

/// Children of a feature: a settlement's districts and businesses, a district's businesses, a
/// building's or underground site's levels and rooms, a region's settlements and sites.
pub fn children(world: &World, t0: &T0, id: &str) -> Option<Value> {
    let head = id.split(':').next().unwrap_or("");
    let nums: Vec<usize> = id.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    if let Some(li) = layout_of(world, t0, id) {
        let l = town::layout(world, t0, li);
        let businesses: Vec<Value> = l
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.func.is_some())
            .map(|(bi, b)| json!({ "id": format!("b:{}:{bi}", l.index), "name": building_name(world, &l, bi), "function": b.label() }))
            .collect();
        return Some(json!({ "districts": layout_summary(world, t0, &l)["districts"], "businesses": businesses, "underground": layout_summary(world, t0, &l)["underground"] }));
    }
    if head == "d" && nums.len() == 2 {
        let l = town::layout(world, t0, nums[0]);
        let q = l.quarters.get(nums[1])?;
        let businesses: Vec<Value> = l
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.func.is_some() && q.patches.iter().any(|poly| geom::contains(poly, geom::centroid(&b.poly))))
            .map(|(bi, b)| json!({ "id": format!("b:{}:{bi}", l.index), "name": building_name(world, &l, bi), "function": b.label() }))
            .collect();
        return Some(json!({ "businesses": businesses }));
    }
    if matches!(head, "b" | "u" | "w" | "k" | "t") {
        let it = crate::interior::generate_id(world, t0, id)?;
        return Some(interior_summary(world, &it));
    }
    // A region: the settlements and sites within it.
    let fs = features(world, t0);
    let f = fs.iter().find(|f| f.id == id)?;
    let reach = f.extent_ft * if matches!(f.kind, "continent" | "island") { 0.9 } else { 0.6 };
    let inside: Vec<Value> = fs
        .iter()
        .filter(|g| (SETTLEMENT_KINDS.contains(&g.kind) || SITE_KINDS.contains(&g.kind)) && geom::dist([g.x, g.y], [f.x, f.y]) < reach)
        .take(200)
        .map(|g| json!({ "id": g.id, "kind": g.kind, "name": g.name }))
        .collect();
    Some(json!({ "places": inside }))
}

/// What is at a point: the ground, the regions, the nearest settlements, the building or
/// district or site there, and named features nearby (a tenth of their size away, 2 to 10 mi).
pub fn describe(world: &World, t0: &T0, x: f64, y: f64) -> Value {
    let fs = features(world, t0);
    let shapes = shapes(world, t0);
    let p = [x, y];
    let sea = world.params().sea_level_ft;
    let ground = t0.sample(x, y, t0.cell_ft);
    let water = t0.sample_water(x, y) as f64;
    let biome = crate::t0::biome::Biome::from_u8(t0.sample_biome(x, y, t0.biome_warp(x, y))[0]).name();
    let here = crate::gazetteer::query(world, t0, x, y).map(|h| with_rename(world, serde_json::to_value(&h).unwrap_or(Value::Null)));
    let hidden = &world.file.edits.hidden;
    let mut near: Vec<(f64, P, &Feature)> = fs
        .iter()
        .zip(&shapes)
        .filter(|(f, _)| !hidden.contains(&f.id))
        .filter_map(|(f, s)| {
            let (d, at) = s.distance(p);
            // (What it lies in is listed under "in".)
            (d <= reach_ft(f) && !(d == 0.0 && matches!(**s, Shape::Area { .. }))).then_some((d, at, f))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let near: Vec<Value> = near
        .into_iter()
        .take(25)
        .map(|(d, at, f)| json!({ "id": f.id, "kind": f.kind, "name": f.name, "distance_mi": mi(d), "direction": if d > 0.0 { compass(p, at) } else { "here" } }))
        .collect();
    // The NPCs and plot points at what is here, or nearby.
    let mut ids: Vec<String> = near.iter().filter_map(|f| f["id"].as_str().map(str::to_string)).collect();
    if let Some(h) = &here {
        ids.extend(h["id"].as_str().map(str::to_string));
        ids.extend(h["settlement"].as_u64().and_then(|li| feature_of_layout(world, t0, li as usize)));
    }
    let (npcs, plots) = authored_at(world, t0, &ids);
    json!({
        "x_ft": x.round(),
        "y_ft": y.round(),
        "elevation_ft": (ground - sea).round(),
        "under_water": water > ground,
        "npcs": npcs,
        "plots": plots,
        "biome": biome,
        "in": regions_at(&fs, &shapes, p),
        "here": here,
        "nearby": near,
        "nearest_settlements": nearest_settlements(&fs, p, "", 3),
    })
}

// ---------------------------------------------------------------------------------------
// Names: everything that can be renamed, with its generated name.

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A name as the Names menu and `list_names` show it: current and generated, where it is, and
/// whether it can be entered (a building or site with levels).
fn name_entry(world: &World, id: String, kind: &str, generated: String, p: Option<P>, enter: bool) -> Value {
    let edits = &world.file.edits;
    let mut v = json!({ "id": id, "kind": kind, "name": edits.renames.get(&id).unwrap_or(&generated), "generated": generated });
    if let Some(p) = p {
        v["x_ft"] = json!(p[0].round());
        v["y_ft"] = json!(p[1].round());
    }
    if enter {
        v["enter"] = json!(true);
    }
    if edits.hidden.contains(&id) {
        v["hidden"] = json!(true);
    }
    v
}

/// The names in a settlement's or site's layout: its districts, businesses, wall and gate
/// towers, ways underground (sewer grates aside) and keeps' deep dungeons.
pub fn layout_names(world: &World, t0: &T0, li: usize) -> Vec<Value> {
    if li >= town::layout_count(t0) {
        return Vec::new();
    }
    let l = town::layout(world, t0, li);
    let mut out = Vec::new();
    for (qi, q) in l.quarters.iter().enumerate() {
        out.push(name_entry(world, format!("d:{li}:{qi}"), "district", q.name.clone(), Some(q.label[q.label.len() / 2]), false));
    }
    for (bi, b) in l.buildings.iter().enumerate().filter(|(_, b)| b.func.is_some()) {
        let generated = b.name.clone().unwrap_or_else(|| b.label().to_string());
        out.push(name_entry(world, format!("b:{li}:{bi}"), "building", generated, Some(geom::centroid(&b.poly)), true));
    }
    for (k, (at, _, gate)) in crate::interior::towers(&l).into_iter().enumerate() {
        out.push(name_entry(world, format!("t:{li}:{k}"), "tower", format!("{} {}", if gate { "Gate tower" } else { "Wall tower" }, k + 1), Some(at), true));
    }
    for (k, e) in l.entrances.iter().enumerate().filter(|(_, e)| e.kind != crate::under::UnderKind::Sewer) {
        out.push(name_entry(world, format!("u:{li}:{k}"), "underground", cap(e.kind.name()), Some(e.at), true));
    }
    for (bi, b) in l.buildings.iter().enumerate().filter(|(_, b)| crate::interior::has_deep_dungeon(b)) {
        out.push(name_entry(world, format!("k:{li}:{bi}"), "underground", "Deep dungeons".into(), Some(geom::centroid(&b.poly)), true));
    }
    out
}

/// The names in a building or site: each level, then its rooms (ids `l:` and `r:`).
pub fn site_names(world: &World, t0: &T0, site: &str) -> Option<Vec<Value>> {
    let it = crate::interior::generate_id(world, t0, site)?;
    let mut out = Vec::new();
    for (li, lv) in it.levels.iter().enumerate() {
        out.push(name_entry(world, format!("l:{site}:{li}"), "level", lv.name.clone(), None, false));
        for (ri, r) in lv.rooms.iter().enumerate().filter(|(_, r)| r.squares > 0 && r.kind != "floor") {
            let p = crate::interior::to_world(&it, r.center[0] as f64, r.center[1] as f64);
            out.push(name_entry(world, format!("r:{site}:{li}:{ri}"), "room", r.kind.to_string(), Some(p), false));
        }
    }
    Some(out)
}

/// The names in a layout (`scope` its index) or a building or site (`scope` its id), JSON.
pub fn names_json(world: &World, t0: &T0, scope: &str) -> String {
    let v = match scope.parse::<usize>() {
        Ok(li) => Some(layout_names(world, t0, li)),
        Err(_) => site_names(world, t0, scope),
    };
    serde_json::to_string(&v.unwrap_or_default()).expect("serializable")
}

/// Names to rename (`list_names`): with `within` (a building or site) its levels and rooms;
/// with `settlement` (a settlement's or site's id) what is in its layout; else every named
/// feature (natural ones, settlements, sites, created sites). `kind` keeps only that kind.
pub fn names(world: &World, t0: &T0, kind: Option<&str>, within: Option<&str>, settlement: Option<&str>) -> Result<Value, String> {
    let list = if let Some(site) = within {
        site_names(world, t0, site).ok_or_else(|| format!("not a building or site with levels: {site}"))?
    } else if let Some(s) = settlement {
        layout_names(world, t0, layout_of(world, t0, s).ok_or_else(|| format!("not a settlement or site: {s}"))?)
    } else {
        let edits = &world.file.edits;
        let fs = t0.extra.as_ref().map(|e| e.overlay.features.as_slice()).unwrap_or_default();
        let mut v: Vec<Value> = fs.iter().map(|f| name_entry(world, f.id.clone(), f.kind, f.name.clone(), Some([f.x, f.y]), false)).collect();
        v.extend(edits.created.iter().filter(|c| !c.removed).map(|c| name_entry(world, c.id.clone(), &c.kind, c.name.clone(), Some([c.x, c.y]), false)));
        v
    };
    Ok(json!({ "names": list.into_iter().filter(|v| kind.is_none_or(|k| v["kind"] == k)).collect::<Vec<_>>() }))
}

// ---------------------------------------------------------------------------------------
// Routes over the road network.

/// The road graph: nodes (settlement `S(i)` or a junction by rounded position) and edges
/// (road length, ft).
fn road_graph(world: &World, t0: &T0) -> (Vec<P>, Vec<Vec<(usize, f64)>>, Vec<Option<usize>>) {
    let mut nodes: Vec<P> = Vec::new();
    let mut town_of: Vec<Option<usize>> = Vec::new();
    let mut key: std::collections::BTreeMap<(i64, i64), usize> = Default::default();
    let mut node = |p: P, nodes: &mut Vec<P>, town_of: &mut Vec<Option<usize>>| -> usize {
        // A road ending near a settlement ends at that settlement.
        let near = t0
            .settlements
            .iter()
            .enumerate()
            .filter(|(_, s)| geom::dist([s.x, s.y], p) < town::reach(s) + 3_000.0)
            .min_by(|a, b| geom::dist([a.1.x, a.1.y], p).total_cmp(&geom::dist([b.1.x, b.1.y], p)))
            .map(|(i, _)| i);
        let k = match near {
            Some(i) => (i64::MIN, i as i64),
            None => ((p[0] / 400.0).round() as i64, (p[1] / 400.0).round() as i64),
        };
        *key.entry(k).or_insert_with(|| {
            nodes.push(near.map(|i| [t0.settlements[i].x, t0.settlements[i].y]).unwrap_or(p));
            town_of.push(near);
            nodes.len() - 1
        })
    };
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for r in &t0.roads.roads {
        if r.pts.len() < 2 {
            continue;
        }
        let a = node(r.pts[0], &mut nodes, &mut town_of);
        let b = node(*r.pts.last().unwrap(), &mut nodes, &mut town_of);
        if a != b {
            edges.push((a, b, *r.s.last().unwrap_or(&0.0)));
        }
    }
    let _ = world;
    let mut adj = vec![Vec::new(); nodes.len()];
    for (a, b, w) in edges {
        adj[a].push((b, w));
        adj[b].push((a, w));
    }
    (nodes, adj, town_of)
}

/// The settlements a settlement's roads lead to directly.
fn roads_from(world: &World, t0: &T0, li: usize) -> Vec<Value> {
    let (_, adj, town_of) = road_graph(world, t0);
    let Some(n) = town_of.iter().position(|t| *t == Some(li)) else { return Vec::new() };
    let mut out = Vec::new();
    // Follow junctions on to the next settlement.
    let mut seen = vec![false; adj.len()];
    seen[n] = true;
    let mut stack: Vec<(usize, f64)> = adj[n].clone();
    while let Some((m, d)) = stack.pop() {
        if seen[m] {
            continue;
        }
        seen[m] = true;
        if let Some(t) = town_of[m] {
            let fid = feature_of_layout(world, t0, t);
            out.push(json!({ "id": fid, "name": fid.as_deref().and_then(|f| name_of(world, t0, f)), "road_mi": mi(d) }));
        } else {
            stack.extend(adj[m].iter().map(|&(k, w)| (k, d + w)));
        }
    }
    out
}

/// A route between two ids or points: by road where the road network joins them, else
/// overland; distances and 5e travel days (normal 24 mi/day, fast 30, slow 18).
pub fn route(world: &World, t0: &T0, from: P, to: P) -> Value {
    let straight = geom::dist(from, to);
    let (nodes, adj, town_of) = road_graph(world, t0);
    let nearest = |p: P| (0..nodes.len()).min_by(|&a, &b| geom::dist(nodes[a], p).total_cmp(&geom::dist(nodes[b], p)));
    let mut road = None;
    if let (Some(a), Some(b)) = (nearest(from), nearest(to)) {
        let (da, db) = (geom::dist(nodes[a], from), geom::dist(nodes[b], to));
        // Dijkstra.
        let mut dist = vec![f64::MAX; nodes.len()];
        let mut prev = vec![usize::MAX; nodes.len()];
        let mut heap = std::collections::BTreeSet::new();
        dist[a] = 0.0;
        heap.insert(((0.0f64).to_bits(), a));
        while let Some((dk, u)) = heap.pop_first() {
            let du = f64::from_bits(dk);
            if du > dist[u] {
                continue;
            }
            if u == b {
                break;
            }
            for &(v, w) in &adj[u] {
                if du + w < dist[v] {
                    dist[v] = du + w;
                    prev[v] = u;
                    heap.insert(((du + w).to_bits(), v));
                }
            }
        }
        if dist[b] < f64::MAX {
            let mut via = Vec::new();
            let mut k = b;
            while k != usize::MAX {
                if let Some(t) = town_of[k]
                    && let Some(fid) = feature_of_layout(world, t0, t)
                {
                    via.push(json!({ "id": fid, "name": name_of(world, t0, &fid) }));
                }
                k = prev[k];
            }
            via.reverse();
            let total = dist[b] + da + db;
            // Worth it only if not far longer than going straight across.
            if total < straight * 2.5 + 2.0 * MI {
                road = Some(json!({ "distance_mi": mi(total), "off_road_mi": mi(da + db), "via": via }));
            }
        }
    }
    let travel = |ft: f64| json!({ "normal_days": ((ft / MI / 24.0) * 10.0).ceil() / 10.0, "fast_days": ((ft / MI / 30.0) * 10.0).ceil() / 10.0, "slow_days": ((ft / MI / 18.0) * 10.0).ceil() / 10.0 });
    let by = road.as_ref().and_then(|r| r["distance_mi"].as_f64()).map(|m| m * MI).unwrap_or(straight);
    json!({
        "straight_mi": mi(straight),
        "by_road": road,
        "travel": travel(by),
        "direction": compass(from, to),
    })
}

// ---------------------------------------------------------------------------------------
// Battlemaps.

/// A battlemap chunk (one 640-ft square, 128 × 128 five-foot squares) summarised for an
/// agent: its origin, surfaces, elevation, buildings, and the objects with their tactical
/// rules, those within `radius_sq` squares of the focus listed one by one.
pub fn battlemap_summary(world: &World, chunk: &crate::battlemap::Chunk, origin: P, focus: P, radius_sq: f64) -> Value {
    use crate::battlemap::SQ;
    let sea = world.params().sea_level_ft as f32;
    let mut surfaces = std::collections::BTreeMap::<String, usize>::new();
    for s in &chunk.surface {
        *surfaces.entry(format!("{s:?}").to_lowercase()).or_default() += 1;
    }
    let (lo, hi) = chunk.height.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
    let mut kinds = std::collections::BTreeMap::<&str, (usize, Value)>::new();
    let mut near = Vec::new();
    let (fx, fy) = ((focus[0] - origin[0]) / 5.0, (focus[1] - origin[1]) / 5.0);
    for o in &chunk.objects {
        let (k, name) = (chunk.info(o), chunk.name(o));
        let e = kinds.entry(name).or_insert_with(|| {
            let cover = ["none", "half", "three-quarters", "full"][k.cover.min(3) as usize];
            (0, json!({ "cover": cover, "blocks_movement": k.blocks_move, "blocks_sight": k.blocks_sight, "difficult": k.difficult, "height_ft": k.height_ft, "hazard": k.hazard.map(|h| format!("{}: {}", h.name, h.effect)) }))
        });
        e.0 += 1;
        if (o.x as f64 - fx).hypot(o.y as f64 - fy) <= radius_sq && near.len() < 200 {
            let (x, y) = (origin[0] + o.x as f64 * 5.0, origin[1] + o.y as f64 * 5.0);
            let mut v = json!({ "object": name, "square": [o.x as i32, o.y as i32], "x_ft": (x * 100.0).round() / 100.0, "y_ft": (y * 100.0).round() / 100.0 });
            // Put down by hand: its id (to remove it); generated: its kind (to clear it).
            match world.file.edits.objects.iter().find(|(_, p)| (p.x - x).abs() < 0.01 && (p.y - y).abs() < 0.01) {
                Some((id, _)) => v["id"] = json!(id),
                None => v["kind"] = json!(o.kind as u16),
            }
            near.push(v);
        }
    }
    // Generated objects taken away by hand here (restore_objects brings them back).
    let size = SQ as f64 * 5.0;
    let cleared: Vec<Value> = world
        .file
        .edits
        .cleared
        .iter()
        .filter(|(_, c)| c.x >= origin[0] && c.y >= origin[1] && c.x < origin[0] + size && c.y < origin[1] + size)
        .map(|(id, c)| json!({ "id": id, "x_ft": c.x, "y_ft": c.y, "kind": c.kind, "radius_ft": c.r }))
        .collect();
    json!({
        "origin_ft": [origin[0], origin[1]],
        "squares": [SQ, SQ],
        "square_ft": 5,
        "elevation_ft": [(lo - sea).round(), (hi - sea).round()],
        "surfaces": surfaces,
        "buildings": chunk.buildings.len(),
        "objects": kinds.into_iter().map(|(name, (count, rules))| json!({ "object": name, "count": count, "rules": rules })).collect::<Vec<_>>(),
        "near_focus": near,
        "cleared": cleared,
    })
}
