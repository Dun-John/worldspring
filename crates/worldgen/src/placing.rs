//! Quick placement (Roadmap 6 P5): a stamp (one building of a kind and size set down fronting the nearest street), a row (drawn
//! buildings lining a path) and the parcel checks. Every result is a pure function of (world, arguments); the tools in `mapd`
//! write them as `Created` sites in one edit.

use crate::agent::{self, Removal};
use crate::core::hash::fnv64;
use crate::core::rng::Pcg32;
use crate::t0::T0;
use crate::t0::settle::Tier;
use crate::town::{self, catalog, geom, hand};
use crate::model::paths::{Path, PathKind};
use crate::paths::PathLine;
use crate::world::{AddedStreet, BUILDING_HOMES, CreatedChecks, TownEdit, World};

type P = [f64; 2];

/// Stamp sizes: a kind's usual footprint times 0.75, 1 or 1.4.
pub const SIZES: [&str; 3] = ["small", "usual", "large"];
/// Most lots one row makes.
pub const ROW_MAX: usize = 100;
/// Row depth limits (ft).
pub const DEPTH_RANGE: (f64, f64) = (15.0, 120.0);

/// A kind's footprint (width along the street, depth) at a size, on the 5-ft grid; `None` for an unknown kind or size.
pub fn stamp_dims(func: &str, size: &str) -> Option<(f64, f64)> {
    let k = match size {
        "small" => 0.75,
        "usual" => 1.0,
        "large" => 1.4,
        _ => return None,
    };
    let (w, d) = if BUILDING_HOMES.contains(&func) {
        match func {
            "hovel" => (16.0, 16.0),
            "house" => (25.0, 25.0),
            "townhouse" => (20.0, 30.0),
            "tenement" => (30.0, 35.0),
            "noble_estate" => (55.0, 45.0),
            "farmhouse" => (35.0, 30.0),
            _ => (20.0, 20.0),
        }
    } else {
        let f = &catalog::CATALOG[catalog::index_of(func)?];
        if f.big {
            (50.0, 40.0)
        } else {
            match f.category {
                "food & lodging" => (30.0, 30.0),
                "arms & gear" => (25.0, 30.0),
                "luxury & finance" => (30.0, 30.0),
                "crafts & transport" => (35.0, 30.0),
                "faith & death" => (35.0, 40.0),
                "civic & military" => (40.0, 40.0),
                "entertainment & oddities" => (35.0, 30.0),
                _ => (25.0, 25.0),
            }
        }
    };
    let snap = |v: f64| (crate::core::round(v * k / 5.0) * 5.0).clamp(10.0, 400.0);
    Some((snap(w), snap(d)))
}

/// Streets (edges of their beds with half widths) near a rectangle: the settlements' roads, else the world's.
fn streets_near(world: &World, t0: &T0, r: [f64; 4]) -> Vec<(P, P, f64)> {
    let mut out: Vec<(P, P, f64)> = Vec::new();
    for l in town::layouts_near(world, t0, r) {
        for (pts, _, w) in &l.roads {
            out.extend(pts.windows(2).map(|s| (s[0], s[1], 0.5 * w)));
        }
    }
    if out.is_empty() {
        for (ri, k) in t0.roads.segments_near(r, 0.0) {
            let rc = &t0.roads.roads[ri as usize];
            let pts: Vec<P> = (0..=8).map(|j| rc.eval(k as usize, j as f64 / 8.0, 5.0, t0.cell_ft).p).collect();
            out.extend(pts.windows(2).map(|s| (s[0], s[1], 0.5 * rc.class.width_ft())));
        }
    }
    out
}

/// The direction (degrees, 0 = +x, 90 = +y, in [0, 360)) from a footprint's middle to the nearest street within 300 ft, and how far
/// its bed is (ft); `None` where there is none.
pub fn facing_of(world: &World, t0: &T0, poly: &[P]) -> Option<(f64, f64)> {
    let c = geom::centroid(poly);
    let streets = streets_near(world, t0, [c[0] - 300.0, c[1] - 300.0, c[0] + 300.0, c[1] + 300.0]);
    let (mut best, mut q): (f64, P) = (f64::MAX, c);
    for &(a, b, half) in &streets {
        let ab = geom::sub(b, a);
        let l2 = geom::dot(ab, ab).max(1e-9);
        let t = (geom::dot(geom::sub(c, a), ab) / l2).clamp(0.0, 1.0);
        let p = geom::add(a, geom::mul(ab, t));
        let d = geom::len(geom::sub(p, c)) - half;
        if d < best {
            (best, q) = (d, p);
        }
    }
    let v = geom::sub(q, c);
    if best >= 300.0 || geom::len(v) < 1e-6 {
        return None;
    }
    let deg = libm::atan2(v[1], v[0]).to_degrees();
    Some((crate::core::round((deg + 360.0) % 360.0 * 10.0) / 10.0, crate::core::round(best.max(0.0))))
}

/// A stamp set down.
pub struct Stamp {
    pub poly: Vec<P>,
    pub mid: P,
    pub name: String,
    /// The direction its front looks (degrees), toward the nearest street; `None` with no street within 300 ft.
    pub facing: Option<f64>,
    pub removed: Vec<Removal>,
    /// How far the spot is from the point asked for (ft).
    pub moved_ft: f64,
}

/// A `w` × `d` ft building of kind `func` at `at`: with `facing` (degrees, the way its front looks) exactly there if it is clear,
/// else on the nearest clear spot (moved, not turned, to 400 ft); without it fronting the nearest street, turned square to it and
/// set back 2-12 ft from its bed (`agent::find_spot_clearing`). `clear` lets it take away the world's own buildings in the way.
pub fn stamp(world: &World, t0: &T0, at: P, func: &str, w: f64, d: f64, facing: Option<f64>, clear: bool, id: &str) -> Result<Stamp, String> {
    let r2 = |p: P| [crate::core::round(p[0] * 100.0) / 100.0, crate::core::round(p[1] * 100.0) / 100.0];
    let (poly, name, removed, facing) = match facing {
        Some(f) => {
            let a = f.to_radians();
            let (n, u) = ([libm::cos(a), libm::sin(a)], [-libm::sin(a), libm::cos(a)]);
            let _ = n;
            let poly: Vec<P> = town::rect(at, u, w, d).into_iter().map(r2).collect();
            let try_at = |poly: &[P]| -> Result<(String, Vec<Removal>), String> {
                if clear {
                    agent::building_spot_clearing(world, t0, poly, Some(func), id, None).map(|s| (s.1, s.2))
                } else {
                    agent::building_spot(world, t0, poly, Some(func), id, None).map(|s| (s.1, Vec::new()))
                }
            };
            match try_at(&poly) {
                Ok((name, gone)) => (poly, name, gone, Some(f)),
                Err(e) => match agent::nearest_clear(world, t0, &poly, Some(func), id, None, clear) {
                    Some((moved, _, gone)) => {
                        let name = agent::spot_name(world, t0, &moved, Some(func), id);
                        (moved, name, gone, Some(f))
                    }
                    None => return Err(e),
                },
            }
        }
        None => {
            let (poly, _, name, gone) = agent::find_spot_clearing(world, t0, at, w, d, Some(func), id, clear)?;
            let f = facing_of(world, t0, &poly).map(|f| f.0);
            (poly, name, gone, f)
        }
    };
    let mid = geom::centroid(&poly);
    Ok(Stamp { moved_ft: crate::core::round(geom::len(geom::sub(mid, at))), poly, mid, name, facing, removed })
}

/// What a row is asked to be.
pub struct RowAsk<'a> {
    /// The path along which it runs (world ft), at least two points apart.
    pub line: &'a [P],
    /// The sides to build on: `true` left of the direction of travel (the normal `[-uy, ux]`), `false` right.
    pub sides: Vec<bool>,
    /// Lot depth (ft); `None`: by the ward's lot area over the lot's width.
    pub depth: Option<f64>,
    /// The kinds to mix (functions or homes); empty: homes of the ward with now and then a business.
    pub kinds: Vec<String>,
    pub ward: &'a str,
    /// The share of slots left empty as alleys and yards, 0 to 0.5.
    pub gaps: f64,
    pub clear: bool,
    pub seed: u64,
    /// (R4) A road laid along it too: the houses set back half its width and facing it.
    pub road: Option<RowRoadAsk>,
}

/// (R4) The road a row asks for along it: its kind and width (ft; else the kind's).
#[derive(Clone, Copy)]
pub struct RowRoadAsk {
    pub kind: PathKind,
    pub width: Option<f64>,
}

impl RowRoadAsk {
    pub fn width_ft(&self) -> f64 {
        self.width.unwrap_or_else(|| self.kind.width_ft())
    }
}

/// (R4) What a row's road writes: a road drawn by hand (`Edits.paths`, its id the writer's), the
/// street records of a town laid out by hand, or a generated town's edit with the street added
/// (`Edits.towns` key, the whole edit, the street's id); `Kept`: the same road is there already.
pub enum RowRoadEdit {
    Path(Path),
    Hand(town::handedit::HandChange, Vec<u32>),
    Town(String, TownEdit, u32),
    Kept(String),
}

/// (R4) A row's road: the town it is part of (layout), its line as laid (ends joined), the edit
/// and the town's buildings taken off its band (`b:<layout>:<id>`).
pub struct RowRoad {
    pub town: Option<usize>,
    pub line: Vec<P>,
    pub edit: RowRoadEdit,
    pub removed: Vec<String>,
}

impl RowRoad {
    /// "road" (drawn by hand) or "street" (a town's).
    pub fn as_str(&self) -> &'static str {
        match &self.edit {
            RowRoadEdit::Path(_) => "road",
            RowRoadEdit::Kept(id) if id.starts_with("rw") => "road",
            _ => "street",
        }
    }
}

/// One building of a row.
pub struct RowLot {
    pub poly: Vec<P>,
    pub mid: P,
    pub func: String,
    pub floors: u8,
    /// The building of the same footprint a row made earlier (`c:<n>`): nothing is added for it.
    pub existing: Option<String>,
    /// The chord of the line it fronts (`Created::faces`).
    pub faces: [P; 2],
}

/// A row laid: the lots, the removals they need (each building once), and the slots refused (`[x, y, why]`, at most 12 kept).
pub struct RowOut {
    pub lots: Vec<RowLot>,
    pub removed: Vec<Removal>,
    pub refused: Vec<(P, String)>,
    pub refused_count: usize,
    pub length_ft: f64,
    pub road: Option<RowRoad>,
}

fn home_for(ward: &str) -> &'static str {
    match ward {
        "slum" => "hovel",
        "noble" => "noble_estate",
        "merchant" | "craft" | "docks" => "townhouse",
        "farm" => "farmhouse",
        _ => "house",
    }
}

fn floors_for(ward: &str, rng: &mut Pcg32) -> u8 {
    let (lo, hi): (u32, u32) = match ward {
        "slum" | "farm" => (1, 2),
        "merchant" | "noble" | "plaza" => (2, 3),
        "craft" | "docks" | "temple" => (1, 3),
        _ => (1, 2),
    };
    (lo + rng.below(hi - lo + 1)) as u8
}

fn point_at(line: &[P], cum: &[f64], s: f64) -> P {
    let k = cum.iter().position(|&c| c >= s).unwrap_or(cum.len() - 1).max(1);
    let (a, b) = (line[k - 1], line[k]);
    let t = ((s - cum[k - 1]) / (cum[k] - cum[k - 1]).max(1e-9)).clamp(0.0, 1.0);
    geom::add(a, geom::mul(geom::sub(b, a), t))
}

/// A row of drawn buildings along `ask.line`: lots cut one after another along each side (widths from the ward's lot area, a
/// seeded mix of kinds and storeys, empty slots between), each a rectangle square to its chord, its front 7-26 ft off the line
/// (the first distance that is clear: past the bed and its setback), kept when `agent::building_spot` allows it and it meets no
/// lot of the row; the rest are refused with the reason. Deterministic from `ask.seed` and the line.
pub fn row(world: &World, t0: &T0, ask: &RowAsk, first_id: usize, existing: &dyn Fn(&[P], &str) -> Option<String>) -> Result<RowOut, String> {
    let line: Vec<P> = {
        let mut v: Vec<P> = Vec::new();
        for p in ask.line {
            if v.last().is_none_or(|q| geom::len(geom::sub(*p, *q)) > 1.0) {
                v.push(*p);
            }
        }
        v
    };
    if line.len() < 2 {
        return Err("along: at least two points apart".into());
    }
    // (With a road, the lots are cut along its line: the curve through the points.)
    let raw = line.clone();
    let line = if ask.road.is_some() { crate::paths::curve(&line) } else { line };
    let half = ask.road.map(|r| 0.5 * r.width_ft());
    let offsets = match half {
        Some(h) => [h, h + 4.0, h + 10.0, h + 18.0],
        None => [7.0, 12.0, 18.0, 26.0],
    };
    let band = ask.road.map(|r| vec![crate::paths::Near { kind: r.kind, half: 0.5 * r.width_ft() - 0.1, segs: line.windows(2).map(|s| (s[0], s[1])).collect() }]);
    let mut cum = vec![0.0];
    for w in line.windows(2) {
        cum.push(cum.last().unwrap() + geom::len(geom::sub(w[1], w[0])));
    }
    let total = *cum.last().unwrap();
    if total < 20.0 {
        return Err("along: the path is under 20 ft long".into());
    }
    let ward = hand::ward_of(ask.ward).ok_or_else(|| format!("ward: one of {}", crate::world::TOWN_WARDS.join(", ")))?;
    let area = hand::lot_area(ward);
    let mut key = format!("{}|{}", world.seed, ask.seed);
    for p in &line {
        key.push_str(&format!("|{:.0},{:.0}", p[0], p[1]));
    }
    // The kinds to draw from without a list: businesses the ward can have in a town (not the big ones).
    let shops: Vec<&str> = catalog::CATALOG.iter().filter(|f| f.wards.contains(&ward) && !f.big && f.min_tier <= Tier::Town).map(|f| f.key).collect();
    let mut out = RowOut { lots: Vec::new(), removed: Vec::new(), refused: Vec::new(), refused_count: 0, length_ft: total, road: None };
    let mut seen: Vec<String> = Vec::new();
    for &left in &ask.sides {
        // (Keyed on the side, so "right" alone lays what "both" lays on its right.)
        let mut rng = Pcg32::new(fnv64(key.as_bytes()), if left { 71 } else { 72 });
        let mut s = 0.0;
        while s < total - 8.0 && out.lots.len() < ROW_MAX {
            let width = (crate::core::sqrt(area * rng.range(0.6, 1.4)) * rng.range(0.8, 1.2)).clamp(14.0, 60.0);
            let gap = rng.next_f64() < ask.gaps;
            let func = match ask.kinds.len() {
                0 if !shops.is_empty() && rng.below(4) == 0 => shops[rng.below(shops.len() as u32) as usize].to_string(),
                0 => home_for(ask.ward).to_string(),
                n => ask.kinds[rng.below(n as u32) as usize].clone(),
            };
            let floors = floors_for(ask.ward, &mut rng);
            let e = (s + width).min(total);
            if gap || e - s < 10.0 {
                s = e + if gap { rng.range(8.0, 14.0) } else { 0.0 };
                continue;
            }
            let (a, b) = (point_at(&line, &cum, s), point_at(&line, &cum, e));
            let chord = geom::sub(b, a);
            let cl = geom::len(chord);
            s = e;
            if cl < 10.0 {
                continue;
            }
            let u = geom::mul(chord, 1.0 / cl);
            let nrm = if left { [-u[1], u[0]] } else { [u[1], -u[0]] };
            let depth = ask.depth.unwrap_or_else(|| (area / cl).clamp(22.0, 60.0));
            let mut why = String::new();
            let mut done = false;
            let mut spot_why = false;
            let q = |p: P, o: f64| {
                let v = geom::add(p, geom::mul(nrm, o));
                [crate::core::round(v[0] * 100.0) / 100.0, crate::core::round(v[1] * 100.0) / 100.0]
            };
            let faces = [q(a, 0.0), q(b, 0.0)];
            for off in offsets {
                let poly = vec![q(a, off), q(b, off), q(b, off + depth), q(a, off + depth)];
                if out.lots.iter().any(|l| agent::polys_overlap_pub(&l.poly, &poly)) {
                    if why.is_empty() {
                        why = "meets another house of the row".into();
                    }
                    continue;
                }
                if band.as_ref().is_some_and(|n| crate::paths::meets(n, &poly)) {
                    if why.is_empty() {
                        why = "meets the road".into();
                    }
                    continue;
                }
                if let Some(id) = existing(&poly, &func) {
                    let mid = geom::centroid(&poly);
                    out.lots.push(RowLot { poly, mid, func: func.clone(), floors, existing: Some(id), faces });
                    done = true;
                    break;
                }
                let tid = format!("c:{}", first_id + out.lots.iter().filter(|l| l.existing.is_none()).count());
                let spot = if ask.clear { agent::building_spot_clearing(world, t0, &poly, Some(&func), &tid, None).map(|s| (s.0, s.2)) } else { agent::building_spot(world, t0, &poly, Some(&func), &tid, None).map(|s| (s.0, Vec::new())) };
                match spot {
                    Ok((mid, gone)) => {
                        for r in gone {
                            if !seen.contains(&r.id) {
                                seen.push(r.id.clone());
                                out.removed.push(r);
                            }
                        }
                        out.lots.push(RowLot { poly, mid, func: func.clone(), floors, existing: None, faces });
                        done = true;
                        break;
                    }
                    Err(e) => {
                        // (What the ground or the town says beats the row's own overlaps: the
                        // nearer setbacks often only meet the lot before, at a bend.)
                        if why.is_empty() || !spot_why {
                            why = plain_why(&e);
                            spot_why = true;
                        }
                    }
                }
            }
            if !done {
                out.refused_count += 1;
                if out.refused.len() < 12 {
                    let m = geom::mul(geom::add(a, b), 0.5);
                    out.refused.push(([crate::core::round(m[0]), crate::core::round(m[1])], why));
                }
            }
        }
    }
    if out.lots.is_empty() {
        let why = out.refused.first().map(|r| format!(" (first refusal: {})", r.1)).unwrap_or_default();
        return Err(format!("no building fits along that path{why}"));
    }
    if let Some(r) = ask.road {
        out.road = Some(row_road(world, t0, r, &raw, &line, &out.lots, first_id)?);
    }
    Ok(out)
}

/// A refusal for a person: no ids in brackets, no world-ft points ("that overlaps Inn (b:1:7) in Rizul, at (1, 2)"
/// becomes "that stands on Inn in Rizul").
fn plain_why(e: &str) -> String {
    let mut t = e.to_string();
    if let Some(i) = t.find(", at (").or_else(|| t.find(" at (")) {
        t.truncate(i);
    }
    while let Some(i) = t.find(" (") {
        let Some(j) = t[i..].find(')') else { break };
        if t[i + 2..i + j].contains(' ') {
            break;
        }
        t.replace_range(i..i + j + 1, "");
    }
    match t.strip_prefix("that overlaps ") {
        Some(r) => format!("that stands on {r}"),
        None => t,
    }
}

/// (R4) The road along a row whose lots are laid: in the town the lots would be part of
/// (`members::would_own`) a street of it (a hand town's street graph, joined at its ends and
/// split where it crosses; a generated town's added street), else a road drawn by hand (`raw`:
/// the points asked). Refused: a town street over water or through the town wall, a town with
/// `added::MAX_ADDED` streets added, a street the hand town's graph refuses.
fn row_road(world: &World, t0: &T0, r: RowRoadAsk, raw: &[P], line: &[P], lots: &[RowLot], first_id: usize) -> Result<RowRoad, String> {
    use town::{added, handedit};
    let polys: Vec<Vec<P>> = lots.iter().map(|l| l.poly.clone()).collect();
    let owner = town::members::would_own(world, t0, &polys).map(|o| o.0);
    let laid = added::thin(&crate::paths::rounded(line), 0.5);
    let w = r.width_ft();
    let band = |pts: &[P]| vec![crate::paths::Near { kind: r.kind, half: 0.5 * w, segs: pts.windows(2).map(|s| (s[0], s[1])).collect() }];
    match owner {
        Some(li) if hand::is_hand(world, li) => {
            // (Its lots in the world first, so the blocks laid again keep off them.)
            let mut file = world.file.clone();
            for (k, l) in lots.iter().filter(|l| l.existing.is_none()).enumerate() {
                let c = crate::world::Created { id: format!("c:{}", first_id + k), kind: "building".into(), x: l.mid[0], y: l.mid[1], poly: l.poly.clone(), func: Some(l.func.clone()), floors: Some(l.floors), faces: Some(l.faces), ..Default::default() };
                file.edits.created.push(c);
            }
            let w2 = World::new(file)?;
            let points = handedit::join_ends(&w2, li, &laid, added::JOIN_END_FT);
            let req = handedit::HandRequest::AddStreet { points: points.clone(), width: Some(w), set_l: None, set_r: None, class: Some(hand::StreetClass::Street) };
            let (ch, report) = handedit::change(&w2, t0, li, &req).map_err(|e| format!("road: the town's streets refuse it: {e}"))?;
            let made: Vec<u32> = report["streets_added"].as_array().map(|v| v.iter().filter_map(|x| x.as_u64().map(|x| x as u32)).collect()).unwrap_or_default();
            Ok(RowRoad { town: Some(li), line: points, edit: RowRoadEdit::Hand(ch, made), removed: Vec::new() })
        }
        Some(li) if added::home(world, t0, li).is_some() => {
            let at = added::home(world, t0, li).unwrap_or_default();
            let key = town::wards::key(li);
            let mut e = match world.file.edits.towns.get(&key) {
                Some(e) if geom::dist(e.at, at) > TownEdit::SAME_AT_FT => return Err("road: the town was laid out elsewhere since its streets were edited: open it in Streets first".into()),
                Some(e) => e.clone(),
                None => TownEdit { at, ..Default::default() },
            };
            let st = AddedStreet { pts: laid, kind: r.kind, width: r.width };
            if let Some((&id, _)) = e.streets.iter().find(|(_, o)| **o == st) {
                let line = added::laid(world, t0, li).into_iter().find(|x| x.0 == id).map(|x| x.1).unwrap_or_else(|| st.pts.clone());
                return Ok(RowRoad { town: Some(li), line, edit: RowRoadEdit::Kept(format!("ts:{li}:{id}")), removed: Vec::new() });
            }
            if e.streets.len() >= added::MAX_ADDED {
                return Err(format!("road: a town takes at most {} streets added by hand", added::MAX_ADDED));
            }
            let id = e.streets.keys().next_back().map_or(1, |k| k + 1);
            let l = town::layout(world, t0, li);
            let joined = added::lines(&l, &std::collections::BTreeMap::from([(id, st.clone())])).pop().map_or_else(|| st.pts.clone(), |x| x.1);
            if added::crosses_wall(&l, &joined) {
                return Err("road: the street would run through the town wall: open a gate there first (Streets › gates)".into());
            }
            let probe = Path { pts: added::thin(&crate::paths::rounded(&joined), 0.5), kind: r.kind, width: r.width, name: None };
            if probe.problem().is_none() && !crate::paths::crossings(world, t0, &probe).is_empty() {
                return Err("road: a street along a row can't cross water: put a crossing down first".into());
            }
            let near = band(&joined);
            let removed = l.buildings.iter().filter(|b| crate::paths::meets(&near, &b.poly)).map(|b| format!("b:{li}:{}", b.id)).collect();
            e.streets.insert(id, st);
            Ok(RowRoad { town: Some(li), line: joined, edit: RowRoadEdit::Town(key, e, id), removed })
        }
        _ => {
            let pts = crate::paths::rounded(raw);
            let pts = if pts.len() > crate::paths::MAX_PATH_PTS { added::thin(&pts, 0.5) } else { pts };
            let p = Path { pts, kind: r.kind, width: r.width, name: None };
            if let Some(why) = p.problem() {
                return Err(format!("road: {why}"));
            }
            if !crate::paths::crossings(world, t0, &p).is_empty() {
                return Err("road: a road along a row can't cross water: put a crossing down first".into());
            }
            if let Some((id, _)) = world.file.edits.paths.iter().find(|(_, o)| o.pts == p.pts && o.kind == p.kind && o.width == p.width) {
                return Ok(RowRoad { town: owner, line: p.line().to_vec(), edit: RowRoadEdit::Kept(id.clone()), removed: Vec::new() });
            }
            Ok(RowRoad { town: owner, line: p.line().to_vec(), edit: RowRoadEdit::Path(p), removed: Vec::new() })
        }
    }
}

/// (R4) A row's road as JSON: `{as: "road"|"street", town?, line, removed, kept?, path? (a road
/// drawn by hand: the writer gives it its `rw:` id), streets?/wharves? (a hand town's records to
/// set, null: removed), towns? (`{key: TownEdit}`), street_id? (`ts:<layout>:<id>`), street_ids?
/// (a hand town's new `e` numbers)}`.
pub fn row_road_json(r: &RowRoad) -> serde_json::Value {
    let mut v = serde_json::json!({ "as": r.as_str(), "line": r.line, "removed": r.removed });
    if let Some(t) = r.town {
        v["town"] = serde_json::json!(t);
    }
    match &r.edit {
        RowRoadEdit::Path(p) => v["path"] = serde_json::json!(p),
        RowRoadEdit::Hand(ch, made) => {
            for (k, x) in ch.to_json().as_object().cloned().unwrap_or_default() {
                v[k] = x;
            }
            v["street_ids"] = serde_json::json!(made);
        }
        RowRoadEdit::Town(key, e, id) => {
            v["towns"] = serde_json::json!({ key.clone(): e });
            v["street_id"] = serde_json::json!(format!("ts:{}:{id}", r.town.unwrap_or_default()));
        }
        RowRoadEdit::Kept(id) => v["kept"] = serde_json::json!(id),
    }
    v
}

/// (R4) A row's road from JSON (`{kind, width?}`; absent or null: none): a path kind, 3-40 ft.
pub fn road_of(v: &serde_json::Value) -> Result<Option<RowRoadAsk>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let names = || PathKind::ALL.iter().map(|k| k.name()).collect::<Vec<_>>().join(", ");
    let kind = v["kind"].as_str().and_then(PathKind::parse).ok_or_else(|| format!("road.kind: one of {}", names()))?;
    let width = match &v["width"] {
        serde_json::Value::Null => None,
        w => Some(w.as_f64().filter(|w| (3.0..=40.0).contains(w)).ok_or("road.width: 3 to 40 ft")?),
    };
    Ok(Some(RowRoadAsk { kind, width }))
}

/// A row's seed from JSON: absent is 0; a negative or fractional number is refused.
pub fn seed_of(v: &serde_json::Value) -> Result<u64, String> {
    if v.is_null() {
        return Ok(0);
    }
    v.as_u64().ok_or_else(|| "seed: a whole number, 0 or more".to_string())
}

/// What stops an outline from being a plot, in words: a building drawn by hand, water, a river or a road (streets too) under it; the
/// world's own buildings and squares may be replaced by its fill. `None` when it is free. `own`: the plot being changed, whose
/// fill's own lots, alleys and squares never stand in its way.
pub fn parcel_blocker(world: &World, t0: &T0, poly: &[P], own: Option<&str>) -> Option<String> {
    let first = t0.settlements.len() + t0.base_pois;
    let skip = own.and_then(|id| world.file.edits.created.iter().position(|c| c.id == id && c.kind == "plot")).map(|k| (first + k, None));
    let (found, _) = match agent::spot_blockers(world, t0, poly, skip, true) {
        Ok(f) => f,
        Err(e) => return Some(e),
    };
    let hit = found.iter().find(|b| match b.what {
        "building" => !b.removable,
        "water" | "river" | "road" | "street" | "square" => true,
        _ => false,
    });
    let (what, kind) = match hit {
        Some(b) => (b.what, b.kind),
        None => hand_water(world, poly).or_else(|| wet_inside(world, t0, poly))?,
    };
    let what = match what {
        "building" => "a building drawn by hand".to_string(),
        "water" => kind.map_or("water", |k| if k == "sea" { "the sea" } else { "a lake" }).to_string(),
        "river" => "a river".to_string(),
        "square" => "a square".to_string(),
        w => format!("a {w}"),
    };
    Some(format!("that outline has {what} in it: a plot is free ground, so draw it clear of that"))
}

/// Water drawn by hand (`Edits.waters`) under an outline: a corner or a grid sample of the plot in an outline, or an outline's corner in the plot.
fn hand_water(world: &World, poly: &[P]) -> Option<(&'static str, Option<&'static str>)> {
    let edits = &world.file.edits;
    let grouped = crate::waters::grouped(edits);
    if grouped.is_empty() {
        return None;
    }
    let [x0, y0, x1, y1] = [
        poly.iter().map(|p| p[0]).fold(f64::MAX, f64::min),
        poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min),
        poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max),
        poly.iter().map(|p| p[1]).fold(f64::MIN, f64::max),
    ];
    let hit = poly.iter().any(|&p| crate::waters::level_in(&grouped, p).is_some())
        || grouped.iter().any(|g| g.pts().iter().any(|&q| geom::contains(poly, q)));
    let step = 10.0_f64.max((x1 - x0).max(y1 - y0) / 80.0);
    let mut y = y0 + step * 0.5;
    while !hit && y < y1 {
        let mut x = x0 + step * 0.5;
        while x < x1 {
            if geom::contains(poly, [x, y]) && crate::waters::level_in(&grouped, [x, y]).is_some() {
                return Some(("water", Some("lake")));
            }
            x += step;
        }
        y += step;
    }
    hit.then_some(("water", Some("lake")))
}

/// Water inside an outline that the walls' probes miss (a pond or a river bend wholly within): its area on a grid of at most 10 ft.
fn wet_inside(world: &World, t0: &T0, poly: &[P]) -> Option<(&'static str, Option<&'static str>)> {
    let [x0, y0, x1, y1] = [
        poly.iter().map(|p| p[0]).fold(f64::MAX, f64::min),
        poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min),
        poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max),
        poly.iter().map(|p| p[1]).fold(f64::MIN, f64::max),
    ];
    let step = 10.0_f64.max((x1 - x0).max(y1 - y0) / 80.0);
    let shore = crate::town::Shore::new(world, t0, [x0, y0, x1, y1]);
    let sea = world.params().sea_level_ft + 1.0;
    let mut y = y0 + step * 0.5;
    while y < y1 {
        let mut x = x0 + step * 0.5;
        while x < x1 {
            let p = [x, y];
            if geom::contains(poly, p) {
                if shore.wet(p) {
                    return Some(("water", Some(if (t0.sample_water(x, y) as f64) <= sea { "sea" } else { "lake" })));
                }
                if shore.in_river(p) {
                    return Some(("river", None));
                }
            }
            x += step;
        }
        y += step;
    }
    None
}

fn removal_json(r: &Removal) -> serde_json::Value {
    serde_json::json!({ "id": r.id, "name": r.name, "function": r.function, "at": r.at })
}

/// A stamp as the app asks for it, as JSON. Input `{x, y, func, size?: "small"|"usual"|"large", width?, depth?, facing?, clear?, id}`;
/// output `{poly, x, y, name, facing?, width, depth, moved_ft, removed}` or `{error}` (the one model `place_stamp` uses).
pub fn stamp_json(world: &World, t0: &T0, input: &str) -> String {
    let go = || -> Result<serde_json::Value, String> {
        let a: serde_json::Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let func = a["func"].as_str().ok_or("func: needed")?;
        let (w, d) = match (a["width"].as_f64(), a["depth"].as_f64()) {
            (Some(w), Some(d)) => (w, d),
            _ => stamp_dims(func, a["size"].as_str().unwrap_or("usual")).ok_or("that kind has no stamp size")?,
        };
        if !(10.0..=400.0).contains(&w) || !(10.0..=400.0).contains(&d) {
            return Err("a stamp is 10 to 400 ft a side".into());
        }
        let at = [a["x"].as_f64().ok_or("x: needed")?, a["y"].as_f64().ok_or("y: needed")?];
        let s = stamp(world, t0, at, func, w, d, a["facing"].as_f64(), a["clear"].as_bool().unwrap_or(false), a["id"].as_str().unwrap_or("c:new"))?;
        Ok(serde_json::json!({ "poly": s.poly, "x": s.mid[0], "y": s.mid[1], "name": s.name, "facing": s.facing, "width": w, "depth": d,
            "moved_ft": s.moved_ft, "removed": s.removed.iter().map(removal_json).collect::<Vec<_>>() }))
    };
    go().unwrap_or_else(|e| serde_json::json!({ "error": e })).to_string()
}

/// A row as the app asks for it, as JSON. Input `{line, side?: "left"|"right"|"both", ward?, kinds?, depth?, gaps?, clear?, seed?, first_id,
/// road?: {kind, width?}}`; output `{lots: [{poly, x, y, func, floors, name, existing?, faces}], removed, refused: [{x, y, why}],
/// refused_count, length_ft, road: null | row_road_json}` or `{error}`.
pub fn row_json(world: &World, t0: &T0, input: &str) -> String {
    let go = || -> Result<serde_json::Value, String> {
        let a: serde_json::Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let line: Vec<P> = a["line"].as_array().ok_or("line: needed")?.iter().filter_map(|q| Some([q.get(0)?.as_f64()?, q.get(1)?.as_f64()?])).collect();
        let sides = match a["side"].as_str().unwrap_or("both") {
            "left" => vec![true],
            "right" => vec![false],
            _ => vec![true, false],
        };
        let kinds: Vec<String> = a["kinds"].as_array().map(|k| k.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default();
        let ward = a["ward"].as_str().unwrap_or("common");
        let n = a["first_id"].as_u64().unwrap_or(0) as usize;
        let depth = a["depth"].as_f64().map(|d| d.clamp(DEPTH_RANGE.0, DEPTH_RANGE.1));
        let road = road_of(&a["road"])?;
        let ask = RowAsk { line: &line, sides, depth, kinds, ward, gaps: a["gaps"].as_f64().unwrap_or(0.15).clamp(0.0, 0.5), clear: a["clear"].as_bool().unwrap_or(false), seed: seed_of(&a["seed"])?, road };
        let edits = &world.file.edits;
        let existing = |poly: &[P], func: &str| {
            let probe = crate::world::Created { kind: "building".into(), poly: poly.to_vec(), func: Some(func.to_string()), ..Default::default() };
            edits.existing_site(&probe, geom::centroid(poly)).map(|o| o.id.clone())
        };
        let out = row(world, t0, &ask, n, &existing)?;
        let mut k = 0;
        let lots: Vec<serde_json::Value> = out
            .lots
            .iter()
            .map(|l| {
                let name = if l.existing.is_some() {
                    String::new()
                } else {
                    k += 1;
                    agent::spot_name(world, t0, &l.poly, Some(&l.func), &format!("c:{}", n + k - 1))
                };
                serde_json::json!({ "poly": l.poly, "x": l.mid[0], "y": l.mid[1], "func": l.func, "floors": l.floors, "name": name, "existing": l.existing, "faces": l.faces })
            })
            .collect();
        Ok(serde_json::json!({ "lots": lots, "removed": out.removed.iter().map(removal_json).collect::<Vec<_>>(),
            "refused": out.refused.iter().map(|(p, why)| serde_json::json!({ "x": p[0], "y": p[1], "why": why })).collect::<Vec<_>>(),
            "refused_count": out.refused_count, "length_ft": out.length_ft, "road": out.road.as_ref().map(row_road_json) }))
    };
    go().unwrap_or_else(|e| serde_json::json!({ "error": e })).to_string()
}

/// Whether an outline can be a plot (`create_parcel`'s checks), as JSON: input `{poly}`; output `{x, y, removed}` (its middle, the
/// world's own buildings under it, as `stamp_json`'s) or
/// `{error, over?: [[x, y], ..]}` (`over`: the live plot it meets).
pub fn parcel_check_json(world: &World, t0: &T0, input: &str) -> String {
    let go = || -> Result<serde_json::Value, String> {
        let a: serde_json::Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let poly: Vec<P> = a["poly"].as_array().ok_or("poly: needed")?.iter().filter_map(|q| Some([q.get(0)?.as_f64()?, q.get(1)?.as_f64()?])).collect();
        let n = poly.len().max(1) as f64;
        let (x, y) = (poly.iter().map(|q| q[0]).sum::<f64>() / n, poly.iter().map(|q| q[1]).sum::<f64>() / n);
        let c = crate::world::Created { kind: "plot".into(), x, y, poly, fill: Some(hand::BlockFill::Generated { ward: "common".into(), lots: None, height: None, reroll: 0, close: None }), ..Default::default() };
        c.check()?;
        if let Some(o) = world.file.edits.created.iter().find(|o| o.kind == "plot" && !o.removed && !c.same_site(o, [c.x, c.y]) && agent::polys_overlap_pub(&o.poly, &c.poly)) {
            return Err(serde_json::json!({ "over": o.poly }).to_string());
        }
        if let Some(why) = parcel_blocker(world, t0, &c.poly, None) {
            return Err(why);
        }
        // (The world's own buildings under it give way to its fill: the app takes them away in the same change.)
        let found = agent::spot_blockers(world, t0, &c.poly, None, true).map(|f| f.0).unwrap_or_default();
        let removed = agent::removals(world, t0, &found);
        Ok(serde_json::json!({ "x": x, "y": y, "removed": removed.iter().map(removal_json).collect::<Vec<_>>() }))
    };
    match go() {
        Ok(v) => v.to_string(),
        Err(e) => match serde_json::from_str::<serde_json::Value>(&e) {
            Ok(v) if v.get("over").is_some() => serde_json::json!({ "error": "That outline meets another plot", "over": v["over"] }).to_string(),
            _ => serde_json::json!({ "error": e }).to_string(),
        },
    }
}
