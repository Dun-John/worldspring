//! Quick placement (Roadmap 6 P5): a stamp (one building of a kind and size set down fronting the nearest street), a row (drawn
//! buildings lining a path) and the parcel checks. Every result is a pure function of (world, arguments); the tools in `mapd`
//! write them as `Created` sites in one edit.

use crate::agent::{self, Removal};
use crate::core::hash::fnv64;
use crate::core::rng::Pcg32;
use crate::t0::T0;
use crate::t0::settle::Tier;
use crate::town::{self, catalog, geom, hand};
use crate::world::{BUILDING_HOMES, World};

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
}

/// One building of a row.
pub struct RowLot {
    pub poly: Vec<P>,
    pub mid: P,
    pub func: String,
    pub floors: u8,
    /// The building of the same footprint a row made earlier (`c:<n>`): nothing is added for it.
    pub existing: Option<String>,
}

/// A row laid: the lots, the removals they need (each building once), and the slots refused (`[x, y, why]`, at most 12 kept).
pub struct RowOut {
    pub lots: Vec<RowLot>,
    pub removed: Vec<Removal>,
    pub refused: Vec<(P, String)>,
    pub refused_count: usize,
    pub length_ft: f64,
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
    let mut out = RowOut { lots: Vec::new(), removed: Vec::new(), refused: Vec::new(), refused_count: 0, length_ft: total };
    let mut seen: Vec<String> = Vec::new();
    for (si, &left) in ask.sides.iter().enumerate() {
        let mut rng = Pcg32::new(fnv64(key.as_bytes()), 71 + si as u64);
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
            for off in [7.0, 12.0, 18.0, 26.0] {
                let q = |p: P, o: f64| {
                    let v = geom::add(p, geom::mul(nrm, o));
                    [crate::core::round(v[0] * 100.0) / 100.0, crate::core::round(v[1] * 100.0) / 100.0]
                };
                let poly = vec![q(a, off), q(b, off), q(b, off + depth), q(a, off + depth)];
                if out.lots.iter().any(|l| agent::polys_overlap_pub(&l.poly, &poly)) {
                    why = "meets another building of the row".into();
                    continue;
                }
                if let Some(id) = existing(&poly, &func) {
                    let mid = geom::centroid(&poly);
                    out.lots.push(RowLot { poly, mid, func: func.clone(), floors, existing: Some(id) });
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
                        out.lots.push(RowLot { poly, mid, func: func.clone(), floors, existing: None });
                        done = true;
                        break;
                    }
                    Err(e) => why = e,
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
    Ok(out)
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

/// A row as the app asks for it, as JSON. Input `{line, side?: "left"|"right"|"both", ward?, kinds?, depth?, gaps?, clear?, seed?, first_id}`;
/// output `{lots: [{poly, x, y, func, floors, name, existing?}], removed, refused: [{x, y, why}], refused_count, length_ft}` or `{error}`.
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
        let ask = RowAsk { line: &line, sides, depth, kinds, ward, gaps: a["gaps"].as_f64().unwrap_or(0.15).clamp(0.0, 0.5), clear: a["clear"].as_bool().unwrap_or(false), seed: a["seed"].as_u64().unwrap_or(0) };
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
                serde_json::json!({ "poly": l.poly, "x": l.mid[0], "y": l.mid[1], "func": l.func, "floors": l.floors, "name": name, "existing": l.existing })
            })
            .collect();
        Ok(serde_json::json!({ "lots": lots, "removed": out.removed.iter().map(removal_json).collect::<Vec<_>>(),
            "refused": out.refused.iter().map(|(p, why)| serde_json::json!({ "x": p[0], "y": p[1], "why": why })).collect::<Vec<_>>(),
            "refused_count": out.refused_count, "length_ft": out.length_ft }))
    };
    go().unwrap_or_else(|e| serde_json::json!({ "error": e })).to_string()
}

/// Whether an outline can be a plot (`create_parcel`'s checks), as JSON: input `{poly}`; output `{x, y}` (its middle) or
/// `{error, over?: [[x, y], ..]}` (`over`: the live plot it meets).
pub fn parcel_check_json(world: &World, input: &str) -> String {
    let go = || -> Result<serde_json::Value, String> {
        let a: serde_json::Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let poly: Vec<P> = a["poly"].as_array().ok_or("poly: needed")?.iter().filter_map(|q| Some([q.get(0)?.as_f64()?, q.get(1)?.as_f64()?])).collect();
        let n = poly.len().max(1) as f64;
        let (x, y) = (poly.iter().map(|q| q[0]).sum::<f64>() / n, poly.iter().map(|q| q[1]).sum::<f64>() / n);
        let c = crate::world::Created { kind: "plot".into(), x, y, poly, fill: Some(hand::BlockFill::Generated { ward: "common".into(), lots: None, reroll: 0, close: None }), ..Default::default() };
        c.check()?;
        if let Some(o) = world.file.edits.created.iter().find(|o| o.kind == "plot" && !o.removed && !c.same_site(o, [c.x, c.y]) && agent::polys_overlap_pub(&o.poly, &c.poly)) {
            return Err(serde_json::json!({ "over": o.poly }).to_string());
        }
        Ok(serde_json::json!({ "x": x, "y": y }))
    };
    match go() {
        Ok(v) => v.to_string(),
        Err(e) => match serde_json::from_str::<serde_json::Value>(&e) {
            Ok(v) if v.get("over").is_some() => serde_json::json!({ "error": "That outline meets another plot", "over": v["over"] }).to_string(),
            _ => serde_json::json!({ "error": e }).to_string(),
        },
    }
}
