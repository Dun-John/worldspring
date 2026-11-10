//! R5 towns (M16b): ids and names for a town's squares, parks and gates.
//!
//! Ids are keyed by what the thing stands on in the town's plan, never by its place in a list,
//! so editing one patch moves no other's id, and walls set on or off move none:
//! - `sq:<layout>:p<patch>` a square or park on a whole patch (the market square and other
//!   squares on a patch, a park, formal garden, common or cemetery on one);
//! - `sq:<layout>:<corner>` a square where streets meet, by its corner (as planned or set by hand);
//! - `sq:<layout>:r<road>` a cemetery beside a road out of town, by the road's approach;
//! - `sq:<layout>:g<building>` a palace's garden, by the palace's building id;
//! - `gt:<layout>:<corner>` a gate in the town wall, by its corner (none while the town has no
//!   wall; a castle's gates are the castle's).
//!
//! Names are generated from the settlement's seed, culture group and kind, keyed by the id (one
//! changed leaves the others' draws alone): a market square by its culture ("Market Square", "the
//! Torg", "the Souk"), other squares by a temple or an inn beside them, else by their kind and a
//! culture word ("Temple Square", "Fox Cross", "Harl Green"), parks and cemeteries by kind and
//! culture group ("the Commons", "Lord's Garden", "the Old Burying Ground", "City of the Dead"),
//! gates by where their road goes, a bridge, else their direction ("Baldur Gate", "North Gate").
//! No two of a town's share a name. Each square on a patch and each park has a label baseline,
//! straight along its long axis where it is clear of buildings, walls and towers.

use super::catalog::CATALOG;
use super::geom::*;
use super::{Layout, ParkKind, PlazaKind};
use crate::core::rng::{hash3, unit};
use crate::t0::T0;
use crate::t0::names::Namer;
use crate::t0::settle::Tier;

/// What a square's or park's id is keyed by (`Plaza::spot`, `Park::spot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Spot {
    /// No id (a quay, a close, a bailey, a village green, a site's yard).
    #[default]
    None,
    Patch(u32),
    Corner(u32),
    Road(u32),
    Garden(u32),
}

/// A square, park or gate of a town.
#[derive(Clone, Debug)]
pub struct Place {
    pub id: String,
    /// "square", "park" or "gate".
    pub kind: &'static str,
    /// What sort: a square's `PlazaKind::name` (market kinds as "market"), a park's
    /// `ParkKind::name`, "gate".
    pub sort: &'static str,
    /// The town's main square (the one its main streets lead to).
    pub main: bool,
    /// Its generated name.
    pub name: String,
    pub at: P,
    pub size_ft: f64,
    /// The patch it stands on (a corner square: none; a road's cemetery: the outskirts' patch).
    pub patch: Option<u32>,
    /// The district it is in (index in `Layout::quarters`).
    pub district: Option<usize>,
    /// Its label baseline (world ft, left to right), empty when it has none.
    pub label: Vec<P>,
}

const MAIN: [&[&str]; 5] = [
    &["Market Square", "the Torg", "Moot Square", "the Wide", "Market Green"],
    &["Market Square", "Market Place", "Cornmarket", "the Wide", "Market Cross"],
    &["the Forum", "Great Square", "Market Square", "the Wide", "Fountain Court"],
    &["the Great Bazaar", "the Souk", "Market Court", "the Maidan", "Caravan Square"],
    &["the Horse Fair", "Gathering Ground", "Market Square", "the Wide", "the Yurt Field"],
];
const MAIN_CIVIC: [&[&str]; 5] = [
    &["Moot Square", "Thing Square", "Town Square", "Hall Square"],
    &["Town Square", "Guildhall Square", "Crown Square", "Moot Square"],
    &["the Forum", "Senate Square", "Crown Square", "Basilica Square"],
    &["the Maidan", "Palace Square", "Court of Justice", "Divan Square"],
    &["Gathering Ground", "Khan's Square", "Town Square", "Kurultai Square"],
];
const OTHER_MARKET: [&[&str]; 5] = [
    &["Haymarket", "Fish Square", "Cattle Market", "Woolmarket", "Horse Fair", "Salt Market", "Shambles"],
    &["Haymarket", "Cornmarket", "Woolmarket", "Cattle Market", "Butter Market", "Poultry Cross", "Shambles", "Horse Fair"],
    &["Oil Market", "Wine Square", "Cloth Market", "Fish Square", "Grain Square", "Cattle Forum"],
    &["Spice Bazaar", "Cloth Bazaar", "Copper Souk", "Goldsmiths' Souk", "Grain Court", "Camel Market"],
    &["Horse Market", "Sheep Market", "Felt Market", "Hide Market", "Cattle Ground"],
];
const OTHER_CIVIC: [&str; 6] = ["Guild Square", "Chapter Square", "Bell Square", "Court Square", "Assize Square", "Exchange Square"];
const COMMON: [&[&str]; 5] = [
    &["the Commons", "the Meadow", "Common", "Green", "the Leas"],
    &["the Commons", "the Meadow", "Common", "Green", "the Bowling Green"],
    &["the Grove", "the Olive Garden", "Common", "the Meadow", "Green"],
    &["the Orchard", "the Palm Garden", "Garden", "the Pleasance", "Green"],
    &["the Commons", "the Pasture", "Common", "Green", "the Horse Meadow"],
];
const FORMAL: [&str; 6] = ["Lord's Garden", "the Queen's Garden", "the Physic Garden", "the Pleasance", "the Knot Garden", "the Rose Garden"];
const CEMETERY: [&[&str]; 5] = [
    &["the Howe Field", "Barrow Yard", "the Old Burying Ground", "Hillside Yard", "Stone Acre"],
    &["the Old Burying Ground", "Hillside Yard", "God's Acre", "Yew Yard", "St Hild's Churchyard"],
    &["the Necropolis", "Cypress Yard", "the Old Tombs", "Hillside Yard", "Field of Rest"],
    &["Field of Tombs", "the Silent Garden", "the Domes", "Hillside Yard", "Garden of Rest"],
    &["the Kurgans", "Mound Field", "the Old Barrows", "the Burial Steppe", "Barrow Field"],
];

/// A word list's pick by roll `t` (0-1).
fn pick<'a>(xs: &[&'a str], t: f64) -> &'a str {
    xs[((t * xs.len() as f64) as usize).min(xs.len() - 1)]
}

/// The four (or eight) ways a point lies from the town's centre.
fn way(c: P, p: P, eight: bool) -> &'static str {
    let d = sub(p, c);
    let a = libm::atan2(-d[1], d[0]);
    let n = if eight { 8.0 } else { 4.0 };
    let k = crate::core::round(a / (2.0 * core::f64::consts::PI) * n).rem_euclid(n) as usize;
    if eight { ["East", "Northeast", "North", "Northwest", "West", "Southwest", "South", "Southeast"][k] } else { ["East", "North", "West", "South"][k] }
}

/// A building's name's root for a square beside it: "Fox" of "The Laughing Fox", "Temple" for
/// a temple.
fn root_of(l: &Layout, at: P, reach: f64) -> Option<String> {
    let mut best: Option<(f64, String)> = None;
    for b in &l.buildings {
        let Some(f) = b.func else { continue };
        let key = CATALOG[f as usize].key;
        let d = dist(centroid(&b.poly), at);
        if d > reach || best.as_ref().is_some_and(|x| x.0 <= d) {
            continue;
        }
        let root = match key {
            "temple" | "monastery" => Some("Temple".to_string()),
            "shrine" => Some("Chapel".to_string()),
            "inn" | "tavern" | "alehouse" | "guildhall" | "bank" | "library" => b.name.as_deref().and_then(|n| {
                let n = n.split(" of ").next().unwrap_or(n);
                let ws: Vec<&str> = n.split(' ').filter(|w| !matches!(*w, "The" | "the" | "Inn" | "Tavern" | "Alehouse" | "House" | "Hall" | "Arms" | "Rest")).collect();
                ws.last().filter(|w| w.len() >= 3 && w.chars().all(|c| c.is_alphabetic())).map(|w| w.to_string())
            }),
            _ => None,
        };
        if let Some(r) = root {
            best = Some((d, r));
        }
    }
    best.map(|b| b.1)
}

/// The name of the settlement the road through `g` (a gate) goes to, one road on.
fn destination(t0: &T0, l: &Layout, g: P) -> Option<String> {
    let me = l.index as usize;
    let near = |pts: &[P]| dist(pts[0], g).min(dist(pts[pts.len() - 1], g));
    let ap = l.roads.iter().filter(|r| r.1 < 3 && r.0.len() >= 2).min_by(|a, b| near(&a.0).total_cmp(&near(&b.0)))?;
    if near(&ap.0) > 150.0 {
        return None;
    }
    let (a, b) = (ap.0[0], ap.0[ap.0.len() - 1]);
    let end = if dist(a, g) < dist(b, g) { b } else { a };
    let (mut best, mut far) = (60.0, None);
    for r in t0.roads.roads.iter().filter(|r| r.pts.len() >= 2) {
        let (p, q) = (r.pts[0], r.pts[r.pts.len() - 1]);
        for (x, y) in [(p, q), (q, p)] {
            if dist(x, end) < best {
                best = dist(x, end);
                far = Some(y);
            }
        }
    }
    let far = far?;
    let j = t0
        .settlements
        .iter()
        .enumerate()
        .filter(|(i, s)| *i != me && dist([s.x, s.y], far) < super::reach(s) + 3_000.0)
        .min_by(|a, b| dist([a.1.x, a.1.y], far).total_cmp(&dist([b.1.x, b.1.y], far)).then(a.0.cmp(&b.0)))?
        .0;
    let fs = &t0.extra.as_ref()?.overlay.features;
    fs.iter().filter(|f| matches!(f.kind, "metropolis" | "city" | "town" | "village")).nth(j).map(|f| f.name.clone())
}

/// The town's squares, parks and gates, with their ids, generated names and labels.
pub fn of(t0: &T0, l: &Layout) -> Vec<Place> {
    let li = l.index;
    let Some(s) = t0.settlements.get(li as usize).filter(|_| !l.site) else { return Vec::new() };
    let group = (s.culture % 5) as usize;
    let culture = s.culture as usize;
    let roll = |tag: i64, n: i64, k: i64| unit(hash3(s.seed ^ 0x5a1e_7c3d, tag, n, k));
    let namer = |tag: i64, n: i64| Namer::new(hash3(s.seed, tag, n, 0x91ac));
    let district = |p: P| l.quarters.iter().position(|q| q.patches.iter().any(|poly| contains(poly, p)));
    let mut used: std::collections::BTreeSet<String> = Default::default();
    // The first of these not yet taken, else the last with a culture word before it.
    let mut take = |cands: Vec<String>, namer: &mut Namer, noun: &str| -> String {
        for c in &cands {
            if !used.contains(c) {
                used.insert(c.clone());
                return c.clone();
            }
        }
        loop {
            let n = format!("{} {noun}", namer.word(culture, 1, 2));
            if used.insert(n.clone()) {
                return n;
            }
        }
    };
    let mut out = Vec::new();
    // Squares: the main one first, then those on patches, then those where streets meet.
    let mut squares: Vec<(u8, u32, usize)> = l
        .plazas
        .iter()
        .enumerate()
        .filter_map(|(k, p)| match p.spot {
            Spot::Patch(i) => Some((if l.plaza_patch == Some(i) { 0 } else { 1 }, i, k)),
            Spot::Corner(v) => Some((2, v, k)),
            _ => None,
        })
        .collect();
    squares.sort();
    for (rank, n, k) in squares {
        let p = &l.plazas[k];
        let tag = rank as i64;
        let t = roll(tag, n as i64, 1);
        let mut nm = namer(tag, n as i64);
        let at = centroid(&p.pts);
        let civic = p.kind == PlazaKind::Civic;
        let name = match rank {
            0 => {
                let mut c: Vec<String> = Vec::new();
                if s.tier == Tier::Metropolis && t < 0.3 {
                    c.push(if group == 3 { "the Great Maidan".into() } else { "the Wide".into() });
                }
                if s.capital && t > 0.7 {
                    c.push("Crown Square".into());
                }
                c.push(pick(if civic { MAIN_CIVIC[group] } else { MAIN[group] }, t).to_string());
                c.extend((if civic { MAIN_CIVIC[group] } else { MAIN[group] }).iter().map(|x| x.to_string()));
                take(c, &mut nm, "Square")
            }
            1 => {
                let mut c: Vec<String> = Vec::new();
                if civic {
                    c.push(pick(&OTHER_CIVIC, t).to_string());
                } else {
                    let wet = !l.piers.is_empty() && l.piers.iter().any(|q| dist(centroid(q), at) < 600.0);
                    if wet {
                        c.push(if group == 3 { "Fish Souk".into() } else { "Fish Square".into() });
                    }
                    c.push(pick(OTHER_MARKET[group], t).to_string());
                    c.extend(OTHER_MARKET[group].iter().map(|x| x.to_string()));
                }
                take(c, &mut nm, if group == 3 { "Bazaar" } else { "Market" })
            }
            _ => {
                let noun = match p.kind {
                    PlazaKind::FountainSquare => if t < 0.5 { "Fountain" } else { "Square" },
                    PlazaKind::WellSquare => if t < 0.5 { "Well" } else { "Square" },
                    PlazaKind::Crossroads => if t < 0.6 { "Cross" } else { "Corner" },
                    PlazaKind::Green => "Green",
                    _ => "Square",
                };
                let mut c: Vec<String> = Vec::new();
                if let Some(r) = root_of(l, at, 160.0) {
                    c.push(format!("{r} {noun}"));
                    if noun != "Square" && p.kind != PlazaKind::Green {
                        c.push(format!("{r} Square"));
                    }
                }
                c.push(match p.kind {
                    PlazaKind::FountainSquare => "Fountain Square".into(),
                    PlazaKind::WellSquare => if t < 0.3 { "Oldwell".into() } else { "Well Square".into() },
                    PlazaKind::Crossroads => if t < 0.5 { "Five Ways".into() } else { "the Crossways".into() },
                    PlazaKind::Green => "Little Green".into(),
                    _ => "Little Square".into(),
                });
                // (A culture word first, mostly: "Harl Green".)
                if roll(tag, n as i64, 2) < 0.6 {
                    let w = nm.word(culture, 1, 2);
                    c.insert(c.len() - 1, format!("{w} {noun}"));
                }
                take(c, &mut nm, noun)
            }
        };
        let o = obb(&p.pts);
        let sort = if p.kind.is_market() { "market" } else { p.kind.name() };
        let label = if rank < 2 { clear_line(l, &p.pts) } else { Vec::new() };
        let patch = if let Spot::Patch(i) = p.spot { Some(i) } else { None };
        let id = if rank < 2 { format!("sq:{li}:p{n}") } else { format!("sq:{li}:{n}") };
        out.push(Place { id, kind: "square", sort, main: rank == 0, name, at, size_ft: o.long, patch, district: district(at), label });
    }
    // Parks: on patches, a palace's gardens, then the cemeteries by the roads.
    let mut parks: Vec<(u8, u32, usize)> = l
        .parks
        .iter()
        .enumerate()
        .filter_map(|(k, p)| match p.spot {
            Spot::Patch(i) => Some((3, i, k)),
            Spot::Garden(b) => Some((4, b, k)),
            Spot::Road(r) => Some((5, r, k)),
            _ => None,
        })
        .collect();
    parks.sort();
    let mut first_cemetery = true;
    for (rank, n, k) in parks {
        let p = &l.parks[k];
        let tag = rank as i64;
        let t = roll(tag, n as i64, 1);
        let mut nm = namer(tag, n as i64);
        let at = centroid(&p.pts);
        let mut c: Vec<String> = Vec::new();
        let noun = match p.kind {
            ParkKind::Cemetery => {
                if s.tier == Tier::Metropolis && first_cemetery {
                    c.push("City of the Dead".into());
                }
                first_cemetery = false;
                c.push(pick(CEMETERY[group], t).to_string());
                c.extend(CEMETERY[group].iter().map(|x| x.to_string()));
                if group == 1 { "Churchyard" } else { "Yard" }
            }
            ParkKind::Formal => {
                if rank == 4 {
                    c.push(if t < 0.5 { "Palace Gardens".into() } else { "the Royal Gardens".into() });
                }
                if t < 0.35 {
                    c.push(format!("{}'s Garden", nm.person(culture)));
                }
                c.push(pick(&FORMAL, t).to_string());
                c.extend(FORMAL.iter().map(|x| x.to_string()));
                "Garden"
            }
            ParkKind::Common => {
                let w = pick(COMMON[group], t);
                // (A bare noun takes a culture word: "Harl Common".)
                if w.starts_with("the ") {
                    c.push(w.to_string());
                }
                c.extend(COMMON[group].iter().filter(|x| x.starts_with("the ")).map(|x| x.to_string()));
                if group == 3 { "Garden" } else if t < 0.5 { "Common" } else { "Green" }
            }
        };
        if p.kind == ParkKind::Common && !c.is_empty() && roll(tag, n as i64, 2) < 0.4 {
            c.insert(0, format!("{} {noun}", nm.word(culture, 1, 2)));
        }
        let name = take(c, &mut nm, noun);
        let patch = match p.spot {
            Spot::Patch(i) => Some(i),
            _ => p.cemetery.map(|c| c.patch),
        };
        let id = match p.spot {
            Spot::Patch(i) => format!("sq:{li}:p{i}"),
            Spot::Garden(b) => format!("sq:{li}:g{b}"),
            _ => format!("sq:{li}:r{n}"),
        };
        out.push(Place { id, kind: "park", sort: p.kind.name(), main: false, name, at, size_ft: obb(&p.pts).long, patch, district: district(at), label: clear_line(l, &p.pts) });
    }
    // Gates in the town wall, by their corner.
    if l.gate_spots.len() == l.gates.len() {
        let mut gates: Vec<(u32, P)> = l.gate_spots.iter().copied().zip(l.gates.iter().copied()).collect();
        gates.sort_by_key(|g| g.0);
        let towns: Vec<Option<String>> = gates.iter().map(|g| destination(t0, l, g.1)).collect();
        for ((v, g), to) in gates.into_iter().zip(towns) {
            let t = roll(6, v as i64, 1);
            let mut nm = namer(6, v as i64);
            let mut c: Vec<String> = Vec::new();
            if let Some(to) = to.filter(|_| t < 0.6) {
                c.push(format!("{to} Gate"));
            }
            if l.bridges.iter().any(|b| dist(centroid(b), g) < 250.0) {
                c.push(if t < 0.5 { "Bridge Gate".into() } else { "River Gate".into() });
            }
            if l.piers.iter().any(|q| dist(centroid(q), g) < 300.0) {
                c.push("Water Gate".into());
            }
            c.push(format!("{} Gate", way(l.center, g, false)));
            c.push(format!("{} Gate", way(l.center, g, true)));
            let name = take(c, &mut nm, "Gate");
            let inward = mul(sub(l.center, g), 1.0 / dist(l.center, g).max(1e-9));
            let box_ = super::rect(add(g, mul(inward, 50.0)), [-inward[1], inward[0]], 140.0, 36.0);
            out.push(Place { id: format!("gt:{li}:{v}"), kind: "gate", sort: "gate", main: false, name, at: g, size_ft: 30.0, patch: None, district: district(add(g, mul(inward, 30.0))), label: clear_line(l, &box_) });
        }
    }
    out
}

/// How far (ft) a label baseline keeps from buildings, the place's edge, walls and towers.
const CLEAR_FT: f64 = 6.0;
const WALL_CLEAR_FT: f64 = 10.0;
const TOWER_CLEAR_FT: f64 = 22.0;
/// The shortest label baseline (ft).
const LABEL_MIN_FT: f64 = 36.0;

/// Whether a point is clear of the layout's buildings, walls and towers (those near `bb`).
fn clear_at(l: &Layout, near: &[usize], walls: &[(P, P)], p: P) -> bool {
    near.iter().all(|&k| {
        let b = &l.buildings[k].poly;
        !contains(b, p) && (0..b.len()).all(|e| seg_dist(p, b[e], b[(e + 1) % b.len()]) >= CLEAR_FT)
    }) && walls.iter().all(|&(a, b)| seg_dist(p, a, b) >= WALL_CLEAR_FT)
        && l.towers.iter().chain(&l.gate_towers).chain(&l.gates).all(|t| dist(*t, p) >= TOWER_CLEAR_FT)
}

/// A straight label baseline inside `poly` along its long axis (left to right), the longest run
/// clear of buildings, walls and towers on a few lines across it; empty when none is long enough.
fn clear_line(l: &Layout, poly: &[P]) -> Vec<P> {
    if poly.len() < 3 {
        return Vec::new();
    }
    let o = obb(poly);
    let mut u = o.axis;
    if u[0] < 0.0 || (u[0] == 0.0 && u[1] < 0.0) {
        u = mul(u, -1.0);
    }
    let n = [-u[1], u[0]];
    let c = centroid(poly);
    let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let pad = 30.0;
    let inb = |q: &[P]| q.iter().any(|p| p[0] >= x0 - pad && p[0] <= x1 + pad && p[1] >= y0 - pad && p[1] <= y1 + pad);
    let near: Vec<usize> = (0..l.buildings.len()).filter(|&k| inb(&l.buildings[k].poly)).collect();
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).filter(|s| inb(&[s.0, s.1]) || seg_dist(c, s.0, s.1) < o.long).collect();
    let edge_ok = |p: P| contains(poly, p) && (0..poly.len()).all(|e| seg_dist(p, poly[e], poly[(e + 1) % poly.len()]) >= CLEAR_FT);
    let (t0, t1) = extent(poly, u);
    let tc = dot(c, u);
    let mut best: Option<(f64, P, P)> = None;
    for f in [0.0, 0.15, -0.15, 0.3, -0.3] {
        let base = add(c, mul(n, f * o.short));
        let step = 3.0;
        let (mut run0, mut run_len): (Option<f64>, f64) = (None, 0.0);
        let mut s = t0 - tc;
        while s <= t1 - tc + step {
            let p = add(base, mul(u, s));
            let ok = s <= t1 - tc && edge_ok(p) && clear_at(l, &near, &walls, p);
            match (ok, run0) {
                (true, None) => run0 = Some(s),
                (false, Some(a)) => {
                    let len = s - step - a;
                    if len > run_len {
                        run_len = len;
                        let (pa, pb) = (add(base, mul(u, a)), add(base, mul(u, s - step)));
                        if best.as_ref().is_none_or(|b| len > b.0 + 1e-9) {
                            best = Some((len, pa, pb));
                        }
                    }
                    run0 = None;
                }
                _ => {}
            }
            s += step;
        }
    }
    match best {
        Some((len, a, b)) if len >= LABEL_MIN_FT => {
            // (No longer than nine tenths of the place: the name sits in it, not along its edge.)
            let keep = len.min(0.9 * o.long);
            let m = lerp(a, b, 0.5);
            let h = mul(u, 0.5 * keep);
            vec![sub(m, h), m, add(m, h)]
        }
        _ => Vec::new(),
    }
}

/// Whether a label baseline crosses a building, wall or tower of the layout (towncheck).
pub fn label_blocked(l: &Layout, label: &[P]) -> bool {
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    label.windows(2).any(|s| {
        let steps = crate::core::ceil(dist(s[0], s[1]) / 2.0).max(1.0) as usize;
        (0..=steps).any(|k| {
            let p = lerp(s[0], s[1], k as f64 / steps as f64);
            l.buildings.iter().any(|b| contains(&b.poly, p)) || walls.iter().any(|&(a, b)| seg_dist(p, a, b) < 2.0)
        })
    })
}

// ---------------------------------------------------------------------------------------
// For agents and the map (`agent`, `gazetteer`, `get_town_plan`).

/// The layout and place an id (`sq:`/`gt:`) names.
pub fn find(world: &crate::World, t0: &T0, id: &str) -> Option<(std::rc::Rc<Layout>, Place)> {
    let mut parts = id.split(':');
    if !matches!(parts.next(), Some("sq" | "gt")) {
        return None;
    }
    let li = parts.next()?.parse::<usize>().ok().filter(|&li| li < t0.settlements.len())?;
    let l = super::layout(world, t0, li);
    let p = of(t0, &l).into_iter().find(|p| p.id == id)?;
    Some((l, p))
}

/// A place's name as it is now (renamed, else generated).
fn name_now(world: &crate::World, p: &Place) -> String {
    world.file.edits.renames.get(&p.id).cloned().unwrap_or_else(|| p.name.clone())
}

/// A place as `get_town_plan` and `get_feature` give it.
fn json_of(world: &crate::World, l: &Layout, p: &Place) -> serde_json::Value {
    let mut v = serde_json::json!({ "id": p.id, "kind": p.kind, "sort": p.sort, "name": name_now(world, p), "generated": p.name, "x_ft": crate::core::round(p.at[0]), "y_ft": crate::core::round(p.at[1]), "size_ft": crate::core::round(p.size_ft) });
    if p.main {
        v["main"] = serde_json::json!(true);
    }
    if let Some(i) = p.patch {
        v["patch"] = serde_json::json!(i);
    }
    if let Some(qi) = p.district {
        let id = format!("d:{}:{qi}", l.index);
        v["district"] = serde_json::json!({ "name": world.file.edits.renames.get(&id).cloned().unwrap_or_else(|| l.quarters[qi].name.clone()), "id": id });
    }
    v
}

/// `get_feature` of a square, park or gate: what `json_of` gives (no position: `get` has it).
pub fn detail(world: &crate::World, t0: &T0, id: &str) -> Option<serde_json::Value> {
    let (l, p) = find(world, t0, id)?;
    let mut v = json_of(world, &l, &p);
    if let Some(o) = v.as_object_mut() {
        o.remove("id");
        o.remove("x_ft");
        o.remove("y_ft");
    }
    Some(v)
}

/// The town's squares, parks and gates for `get_town_plan`.
pub fn plan_json(world: &crate::World, t0: &T0, index: usize) -> serde_json::Value {
    if index >= t0.settlements.len() {
        return serde_json::json!([]);
    }
    let l = super::layout(world, t0, index);
    serde_json::json!(of(t0, &l).iter().map(|p| json_of(world, &l, p)).collect::<Vec<_>>())
}

/// Squares, parks and gates whose name (as it is now) holds `q` (lower case), for
/// `search_features`: towns and up, up to `limit`.
pub fn search(world: &crate::World, t0: &T0, q: &str, kind: Option<&str>, limit: usize, include_hidden: bool) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if q.len() < 3 || limit == 0 || kind.is_some_and(|k| !matches!(k, "square" | "park" | "gate")) {
        return out;
    }
    for li in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= Tier::Town) {
        let l = super::layout(world, t0, li);
        for p in of(t0, &l) {
            if kind.is_some_and(|k| k != p.kind) || (!include_hidden && world.file.edits.hidden.contains(&p.id)) || !name_now(world, &p).to_lowercase().contains(q) {
                continue;
            }
            let mut v = json_of(world, &l, &p);
            v["settlement"] = serde_json::json!(li);
            out.push(v);
            if out.len() >= limit {
                return out;
            }
        }
    }
    out
}

/// The map's labels for a town's squares on patches, parks and gates (the app shows a gate's once
/// it is renamed), with the district labels (`gazetteer::districts_json`).
pub fn labels(t0: &T0, l: &Layout) -> Vec<crate::gazetteer::DistrictLabel> {
    of(t0, l)
        .into_iter()
        .filter(|p| !p.label.is_empty())
        .map(|p| {
            let kind = match (p.kind, p.sort) {
                (_, "cemetery") => "cemetery",
                ("gate", _) => "gate",
                (k, _) => k,
            };
            crate::gazetteer::DistrictLabel { id: p.id, name: p.name, kind, path: p.label }
        })
        .collect()
}
