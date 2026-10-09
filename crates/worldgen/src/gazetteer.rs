//! Gazetteer lookups over generated content: what is at a point (a building, a settlement)
//! and business search. Named T0 features are already in the overlay; this covers what only
//! exists once settlement layouts are generated. Results are JSON for the app and agents.
//!
//! Building ids are `b:<settlement>:<building>`, district ids `d:<settlement>:<district>`,
//! underground sites `u:<layout>:<entrance>`: stable for an unchanged world file.

use serde::Serialize;

use crate::World;
use crate::t0::T0;
use crate::town::{self, catalog::CATALOG, geom};

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Hit {
    Building {
        id: String,
        settlement: u32,
        name: Option<String>,
        function: &'static str,
        category: Option<&'static str>,
        ward: &'static str,
        /// The named district it stands in.
        district: Option<String>,
        floors: u8,
        /// Footprint centre (ft).
        x: f64,
        y: f64,
        /// Footprint size (ft), for framing.
        size_ft: f64,
    },
    District {
        id: String,
        settlement: u32,
        name: String,
        /// What it grew from (`town::QuarterKind::name`).
        district_kind: &'static str,
        x: f64,
        y: f64,
        size_ft: f64,
    },
    Settlement {
        settlement: u32,
        x: f64,
        y: f64,
        radius_ft: f64,
    },
}

fn building_hit(l: &town::Layout, b: &town::Building) -> Hit {
    let c = geom::centroid(&b.poly);
    let size = b.poly.iter().map(|p| geom::dist(*p, c)).fold(0.0, f64::max) * 2.0;
    Hit::Building {
        id: format!("b:{}:{}", l.index, b.id),
        settlement: l.index,
        name: b.name.clone(),
        function: b.label(),
        category: b.func.map(|f| CATALOG[f as usize].category),
        ward: b.ward.name(),
        district: quarter_at(l, c).map(|qi| l.quarters[qi].name.clone()),
        floors: b.floors,
        x: c[0],
        y: c[1],
        size_ft: size,
    }
}

fn quarter_at(l: &town::Layout, p: [f64; 2]) -> Option<usize> {
    l.quarters.iter().position(|q| q.patches.iter().any(|poly| geom::contains(poly, p)))
}

fn district_hit(l: &town::Layout, qi: usize) -> Hit {
    let q = &l.quarters[qi];
    let pts: Vec<[f64; 2]> = q.patches.iter().flatten().copied().collect();
    let (x0, y0, x1, y1) = pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| (b.0.min(p[0]), b.1.min(p[1]), b.2.max(p[0]), b.3.max(p[1])));
    Hit::District {
        id: format!("d:{}:{}", l.index, qi),
        settlement: l.index,
        name: q.name.clone(),
        district_kind: q.kind.name(),
        x: q.label[q.label.len() / 2][0],
        y: q.label[q.label.len() / 2][1],
        size_ft: (x1 - x0).max(y1 - y0),
    }
}

/// How near (ft) a point must be to an underground entrance to pick it.
const ENTRANCE_PICK_FT: f64 = 12.0;

/// The way underground at a point, else the building, else the district, else the
/// settlement whose built-up area contains it.
pub fn query(world: &World, t0: &T0, x: f64, y: f64) -> Option<Hit> {
    let layouts = town::layouts_near(world, t0, [x, y, x, y]);
    for l in &layouts {
        for e in &l.entrances {
            if geom::dist(e.at, [x, y]) <= ENTRANCE_PICK_FT {
                // A sewer grate opens onto its section of the sewers.
                let id = if e.kind == crate::under::UnderKind::Sewer {
                    let o = crate::under::sewer_section(e.at);
                    format!("w:{}:{}:{}", l.index, (o[0] / crate::under::SEWER_SECTION_FT) as i64, (o[1] / crate::under::SEWER_SECTION_FT) as i64)
                } else {
                    format!("u:{}:{}", l.index, e.id)
                };
                return Some(Hit::Building {
                    id,
                    settlement: l.index,
                    name: None,
                    function: e.kind.entrance_name(),
                    category: Some("underground"),
                    ward: "underground",
                    district: None,
                    floors: 0,
                    x: e.at[0],
                    y: e.at[1],
                    size_ft: 2.0 * ENTRANCE_PICK_FT,
                });
            }
        }
    }
    for l in &layouts {
        for b in &l.buildings {
            if geom::contains(&b.poly, [x, y]) {
                return Some(building_hit(l, b));
            }
        }
    }
    // Wall and gate towers.
    for l in &layouts {
        for (k, (at, r, gate)) in crate::interior::towers(l).into_iter().enumerate() {
            if geom::dist(at, [x, y]) <= r {
                return Some(Hit::Building {
                    id: format!("t:{}:{}", l.index, k),
                    settlement: l.index,
                    name: None,
                    function: if gate { "Gate tower" } else { "Wall tower" },
                    category: Some("fortification"),
                    ward: "wall",
                    district: quarter_at(l, at).map(|qi| l.quarters[qi].name.clone()),
                    floors: 3,
                    x: at[0],
                    y: at[1],
                    size_ft: 2.0 * r,
                });
            }
        }
    }
    // A wall drawn by hand (or a castle's curtain) is picked on its line.
    for l in layouts.iter().filter(|l| l.site) {
        if l.walls.iter().any(|w| w.windows(2).any(|s| geom::seg_dist([x, y], s[0], s[1]) <= 6.0)) {
            return Some(Hit::Settlement { settlement: l.index, x: l.center[0], y: l.center[1], radius_ft: l.radius });
        }
    }
    for l in &layouts {
        if let Some(qi) = quarter_at(l, [x, y]) {
            return Some(district_hit(l, qi));
        }
    }
    layouts
        .iter()
        .filter(|l| geom::dist(l.center, [x, y]) < l.radius * 1.2)
        .min_by(|a, b| geom::dist(a.center, [x, y]).total_cmp(&geom::dist(b.center, [x, y])))
        .map(|l| Hit::Settlement { settlement: l.index, x: l.center[0], y: l.center[1], radius_ft: l.radius })
}

pub fn query_json(world: &World, t0: &T0, x: f64, y: f64) -> String {
    serde_json::to_string(&query(world, t0, x, y)).expect("serializable")
}

/// Districts whose name contains `q`, then named buildings whose name or function does
/// (case-insensitive), name matches first. Names are matched as they are now (renamed or
/// generated); hidden places are left out unless `include_hidden`. Hits carry the generated
/// names (callers show the renames). With `rect` (world ft x0, y0, x1, y1) only settlements
/// reaching into it are searched and only hits inside it kept; without, every settlement
/// layout is generated on first use.
pub fn search_buildings(world: &World, t0: &T0, q: &str, limit: usize, rect: Option<[f64; 4]>, include_hidden: bool) -> Vec<Hit> {
    let q = q.trim().to_lowercase();
    if q.len() < 2 {
        return Vec::new();
    }
    let edits = &world.file.edits;
    let mut districts = Vec::new();
    let mut by_name = Vec::new();
    let mut by_function = Vec::new();
    let layouts: Vec<std::rc::Rc<town::Layout>> = match rect {
        Some(r) => town::layouts_near(world, t0, r),
        None => (0..town::layout_count(t0)).map(|i| town::layout(world, t0, i)).collect(),
    };
    let inside = |x: f64, y: f64| rect.is_none_or(|r| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]);
    // A place's name as it is now (None if hidden and not wanted).
    let current = |id: &str, generated: Option<&str>| -> Option<Option<String>> {
        if !include_hidden && edits.hidden.contains(id) {
            return None;
        }
        Some(edits.renames.get(id).map(String::as_str).or(generated).map(str::to_lowercase))
    };
    for l in layouts {
        for qi in 0..l.quarters.len() {
            let at = l.quarters[qi].label[l.quarters[qi].label.len() / 2];
            let name = current(&format!("d:{}:{qi}", l.index), Some(&l.quarters[qi].name)).flatten();
            if name.is_some_and(|n| n.contains(&q)) && districts.len() < limit && inside(at[0], at[1]) {
                districts.push(district_hit(&l, qi));
            }
        }
        for b in &l.buildings {
            let id = format!("b:{}:{}", l.index, b.id);
            let renamed = edits.renames.contains_key(&id);
            if b.func.is_none() && !renamed {
                continue;
            }
            let c = geom::centroid(&b.poly);
            if !inside(c[0], c[1]) {
                continue;
            }
            let Some(name) = current(&id, b.name.as_deref()) else { continue };
            if name.is_some_and(|n| n.contains(&q)) {
                by_name.push(building_hit(&l, b));
            } else if b.func.is_some() && b.label().to_lowercase().contains(&q) && by_function.len() < limit {
                by_function.push(building_hit(&l, b));
            }
        }
    }
    districts.extend(by_name);
    districts.extend(by_function);
    districts.truncate(limit);
    districts
}

/// A settlement's named districts for map labels: id, name, kind and label baseline.
#[derive(Serialize)]
pub struct DistrictLabel {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub path: Vec<[f64; 2]>,
}

pub fn districts_json(world: &World, t0: &T0, settlement: usize) -> String {
    if settlement >= t0.settlements.len() {
        return "[]".into();
    }
    let l = town::layout(world, t0, settlement);
    let out: Vec<DistrictLabel> = l
        .quarters
        .iter()
        .enumerate()
        .map(|(qi, q)| DistrictLabel { id: format!("d:{}:{}", l.index, qi), name: q.name.clone(), kind: q.kind.name(), path: q.label.clone() })
        .collect();
    serde_json::to_string(&out).expect("serializable")
}

pub fn search_json(world: &World, t0: &T0, q: &str, rect: Option<[f64; 4]>) -> String {
    serde_json::to_string(&search_buildings(world, t0, q, 30, rect, false)).expect("serializable")
}

/// Everything named inside a rectangle (world ft): districts, then businesses (buildings
/// with a function), up to `limit`. For "what's here" lists.
pub fn in_view(world: &World, t0: &T0, rect: [f64; 4], limit: usize) -> Vec<Hit> {
    let inside = |p: [f64; 2]| p[0] >= rect[0] && p[0] <= rect[2] && p[1] >= rect[1] && p[1] <= rect[3];
    let (mut districts, mut shops) = (Vec::new(), Vec::new());
    for l in town::layouts_near(world, t0, rect) {
        for qi in 0..l.quarters.len() {
            let q = &l.quarters[qi];
            if inside(q.label[q.label.len() / 2]) {
                districts.push(district_hit(&l, qi));
            }
        }
        for b in &l.buildings {
            if b.func.is_some() && inside(geom::centroid(&b.poly)) && shops.len() < limit {
                shops.push(building_hit(&l, b));
            }
        }
    }
    districts.extend(shops);
    districts
}

pub fn in_view_json(world: &World, t0: &T0, rect: [f64; 4]) -> String {
    // (A metropolis core holds hundreds of businesses; the map pins them all.)
    serde_json::to_string(&in_view(world, t0, rect, 1_500)).expect("serializable")
}
