//! The acceptance city (Roadmap 6 P11): a world built from seed 3 by the same change functions the app and
//! mapd call (`town::handedit::change`, `placing::*_json`, water outlines, plateaus), written as a world file.
//!
//! `cargo run --release -p worldgen --example acceptcity [out.json]` (default
//! `crates/worldgen/tests/fixtures/acceptcity.json`, the vital test's fixture). Deterministic: run twice, the same bytes.
//!
//! What it holds: seed 3's metropolis laid out by hand and grown inland to ~3.5 mi across (streets out from its edge, ring
//! streets, blocks of every fill, districts, rows, painted areas, a wall with gates, squares); a blank town laid out from a
//! street plan in the manner of a walled market town of before 1900 (authored here by hand, not digitised from any real
//! map); a stamp, a building row and a parcel along a generated city's road; a harbour basin joining the sea; a plateau
//! stack. Every refusal is printed; the run fails if any step that should work is refused.

use serde_json::{Value, json};
use worldgen::geoquery::Squares;
use worldgen::pipeline::Executor;
use worldgen::t0::{T0, settle::Tier};
use worldgen::town::{self, handedit};
use worldgen::world::{Created, Edits, PIN_KINDS};
use worldgen::{World, WorldFile};

type P = [f64; 2];

const SEED: u32 = 3;
/// The grown city's reach from its centre (ft): ~3.5 mi across.
const GROWN_R: f64 = 9_800.0;
/// Ring streets this far apart (ft), radial streets every `TAU / RAYS`.
const RING_STEP: f64 = 780.0;
const RAYS: usize = 48;

fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])).sqrt()
}

fn polar(o: P, r: f64, a: f64) -> P {
    [o[0] + r * libm::cos(a), o[1] + r * libm::sin(a)]
}

/// Rounded to 1/8 ft (what records keep near enough; the same bytes on every platform).
fn q(p: P) -> P {
    [(p[0] * 8.0).round() / 8.0, (p[1] * 8.0).round() / 8.0]
}

/// Where segment a-b first meets the circle of radius `r` round `o` (its parameter), if it does.
fn on_circle(a: P, b: P, o: P, r: f64) -> Option<P> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (fx, fy) = (a[0] - o[0], a[1] - o[1]);
    let (qa, qb, qc) = (dx * dx + dy * dy, 2.0 * (fx * dx + fy * dy), fx * fx + fy * fy - r * r);
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return None;
    }
    let t = (-qb + disc.sqrt()) / (2.0 * qa);
    (0.0..=1.0).contains(&t).then(|| [a[0] + t * dx, a[1] + t * dy])
}

/// The world as it is being built, and what the steps came to.
struct Build {
    world: World,
    t0: T0,
    refused: Vec<String>,
    ops: usize,
}

impl Build {
    fn reload(&mut self, file: WorldFile) {
        self.world = World::new(file).expect("a world");
    }

    /// A change to hand town `li` (a request as the app or mapd sends it). Refused: noted, None.
    fn ask(&mut self, li: usize, req: Value, what: &str) -> Option<Value> {
        self.ops += 1;
        let asked: handedit::HandRequest = serde_json::from_value(req.clone()).unwrap_or_else(|e| panic!("{what}: {e}"));
        match handedit::change(&self.world, &self.t0, li, &asked) {
            Ok((ch, report)) => {
                let mut f = self.world.file.clone();
                ch.apply(&mut f.edits);
                self.reload(f);
                Some(report)
            }
            Err(e) => {
                self.refused.push(format!("{what}: {e}"));
                None
            }
        }
    }

    fn plan(&self, li: usize) -> Value {
        handedit::plan_json(&self.world, &self.t0, li).expect("a hand town")
    }

    /// Created entries pushed (as the app's one `applyEdits`), the continent told.
    fn create(&mut self, made: Vec<Created>) {
        let mut f = self.world.file.clone();
        f.edits.created.extend(made);
        self.reload(f);
        self.t0.apply_edits(&self.world);
    }

    fn land(&self, p: P) -> bool {
        let sea = self.world.params().sea_level_ft;
        self.t0.sample_water(p[0], p[1]) <= worldgen::t0::hydro::DRY && self.t0.sample(p[0], p[1], 200.0) > sea + 2.0
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "crates/worldgen/tests/fixtures/acceptcity.json".into());
    // (`inspect <world.json>`: what a written world holds.)
    if out == "inspect" {
        let path = std::env::args().nth(2).expect("a world file");
        let file: WorldFile = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let ex = Executor::new(World::new(file).unwrap());
        for (k, v) in ex.world.file.edits.streets.iter().filter(|(k, _)| k.parse::<usize>().is_ok()) {
            let li: usize = k.parse().unwrap();
            let l = town::base_layout(&ex.world, &ex.t0, li);
            let plan = handedit::plan_json(&ex.world, &ex.t0, li).unwrap();
            let mut why: std::collections::BTreeMap<String, usize> = Default::default();
            for p in plan["problems"].as_array().into_iter().flatten() {
                *why.entry(p["why"].as_str().unwrap_or_default().into()).or_default() += 1;
            }
            println!("L{li} ({}): {} buildings, {} wall pieces, {} towers, {} gates, {} gate towers, {} blocks; problems {why:?}", v["from"], l.buildings.len(), l.walls.len(), l.towers.len(), l.gates.len(), l.gate_towers.len(), plan["blocks"].as_array().map_or(0, |a| a.len()));
        }
        return;
    }
    let start = std::time::Instant::now();
    let world = World::new(WorldFile { seed: SEED, ..Default::default() }).unwrap();
    let t0 = T0::generate(&world);
    let mut b = Build { world, t0, refused: Vec::new(), ops: 0 };
    let li = (0..b.t0.settlements.len()).filter(|&i| b.t0.settlements[i].tier == Tier::Metropolis).max_by_key(|&i| (b.t0.settlements[i].population, std::cmp::Reverse(i))).expect("a metropolis");
    eprintln!("seed {SEED}: metropolis L{li}, T0 {:.1} s", start.elapsed().as_secs_f64());

    grow_city(&mut b, li);
    // P7: concave castle with three gates, round keep, wet moat
    let lb = blank_town(&mut b, li);
    quick_placement(&mut b, li);
    water_and_ground(&mut b, li);

    // What it came to.
    let l = town::base_layout(&b.world, &b.t0, li);
    let plan = b.plan(li);
    let n = |k: &str| plan[k].as_array().map_or(0, |a| a.len());
    let bb = l.buildings.iter().flat_map(|x| x.poly.iter()).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
    println!(
        "city L{li}: {} buildings, {} junctions, {} streets, {} blocks, {} rows, {} areas, {} walls, {} problems; {:.2} x {:.2} mi",
        l.buildings.len(),
        n("junctions"),
        n("streets"),
        n("blocks"),
        n("rows"),
        n("areas"),
        n("walls"),
        n("problems"),
        (bb[2] - bb[0]) / 5280.0,
        (bb[3] - bb[1]) / 5280.0
    );
    for p in plan["problems"].as_array().into_iter().flatten().take(12) {
        println!("  problem {}: {}", p["key"], p["why"]);
    }
    let lt = town::base_layout(&b.world, &b.t0, lb);
    println!("blank town L{lb}: {} buildings, {} walls, {} gates", lt.buildings.len(), lt.walls.len(), lt.gates.len());
    println!("{} changes, {} refused", b.ops, b.refused.len());
    for r in &b.refused {
        println!("  refused {r}");
    }
    let text = serde_json::to_string(&b.world.file).unwrap() + "\n";
    std::fs::write(&out, &text).unwrap_or_else(|e| panic!("{out}: {e}"));
    println!("wrote {out}: {:.0} KB ({:.0} s)", text.len() as f64 / 1024.0, start.elapsed().as_secs_f64());
    if !b.refused.is_empty() {
        std::process::exit(1);
    }
}

/// The metropolis laid out by hand, then grown: streets out from its edge every 7.5 degrees where the land
/// reaches, ring streets between them, the new blocks given every fill, districts, rows both sides of a
/// long street, painted areas, squares, a wall round the grown part with gates where streets cross it.
fn grow_city(b: &mut Build, li: usize) {
    use std::f64::consts::TAU;
    b.ask(li, json!({ "op": "lay_out_by_hand" }), "lay out by hand").expect("converted");
    let plan = b.plan(li);
    let o: P = serde_json::from_value(plan["origin"].clone()).unwrap();
    let first_new = plan["blocks"].as_array().unwrap().iter().filter_map(|x| x["id"].as_u64()).max().unwrap_or(0) + 1;
    let junctions: Vec<(u64, P)> = plan["junctions"].as_array().unwrap().iter().map(|j| (j["id"].as_u64().unwrap(), serde_json::from_value(j["p"].clone()).unwrap())).collect();
    let ang = |p: P| libm::atan2(p[1] - o[1], p[0] - o[0]).rem_euclid(TAU);
    // Per ray: the outermost junction within half a step of it, where the street out starts.
    let step = TAU / RAYS as f64;
    let starts: Vec<Option<P>> = (0..RAYS)
        .map(|k| {
            let a = k as f64 * step;
            let off = |p: P| {
                let d = (ang(p) - a).rem_euclid(TAU);
                d.min(TAU - d)
            };
            junctions.iter().filter(|(_, p)| off(*p) <= 0.5 * step).max_by(|x, y| dist(x.1, o).total_cmp(&dist(y.1, o)).then(y.0.cmp(&x.0))).map(|x| x.1)
        })
        .collect();
    let r0 = starts.iter().flatten().map(|p| dist(*p, o)).fold(0.0, f64::max);
    // How far dry land runs out along each ray (a river up to 450 ft across is bridged).
    let dry: Vec<f64> = (0..RAYS)
        .map(|k| {
            let a = k as f64 * step;
            let (mut last, mut wet) = (0.0, 0.0);
            let mut r = 1_000.0;
            while r <= GROWN_R + 600.0 {
                if b.land(polar(o, r, a)) {
                    last = r;
                    wet = 0.0;
                } else {
                    wet += 150.0;
                    if wet > 450.0 {
                        break;
                    }
                }
                r += 150.0;
            }
            last
        })
        .collect();
    let ring0 = r0 + 450.0;
    let rings: Vec<f64> = (0..).map(|j| ring0 + j as f64 * RING_STEP).take_while(|r| *r <= GROWN_R).collect();
    // Each ray's outermost ring (None: the land runs out before the first block's far side).
    let outer: Vec<Option<usize>> = (0..RAYS).map(|k| rings.iter().rposition(|r| *r + 150.0 <= dry[k]).filter(|&j| j >= 1 && starts[k].is_some())).collect();
    eprintln!("city L{li}: edge {r0:.0} ft, {} rings {:.0}..{:.0} ft, {} of {RAYS} rays grown", rings.len(), ring0, rings.last().unwrap_or(&0.0), outer.iter().flatten().count());
    // The streets out (every third, where the land goes on, past the wall as a road out of a gate).
    let mut rays: Vec<Option<(P, P)>> = vec![None; RAYS];
    for k in 0..RAYS {
        let (Some(j), Some(s)) = (outer[k], starts[k]) else { continue };
        let a = k as f64 * step;
        let past = k % 3 == 0 && dry[k] >= rings[j] + 450.0;
        let end = q(polar(o, rings[j] + if past { 360.0 } else { 0.0 }, a));
        if b.ask(li, json!({ "op": "add_street", "points": [s, end], "class": if k % 6 == 0 { "main" } else { "street" } }), &format!("street out {k}")).is_some() {
            rays[k] = Some((s, end));
        }
    }
    // Ring streets over each run of rays that reach them, each bend on a street out.
    for (j, &r) in rings.iter().enumerate() {
        let on = |k: usize| outer[k].is_some_and(|o| o >= j) && rays[k].is_some();
        let first = (0..RAYS).find(|&k| !on(k)).unwrap_or(0);
        let mut run: Vec<P> = Vec::new();
        for i in 1..=RAYS {
            let k = (first + i) % RAYS;
            if on(k) {
                let (s, e) = rays[k].unwrap();
                if let Some(p) = on_circle(s, e, o, r) {
                    run.push(q(p));
                }
            }
            if !on(k) || i == RAYS {
                if run.len() >= 2 {
                    let mut req = json!({ "op": "add_street", "points": run });
                    if j == 0 {
                        req["width"] = json!(24.0);
                        (req["set_l"], req["set_r"]) = (json!(12.0), json!(12.0));
                    }
                    b.ask(li, req, &format!("ring {j}"));
                }
                run.clear();
            }
        }
    }
    // The new blocks: every fill, districts.
    let plan = b.plan(li);
    let mut new: Vec<(u64, P)> = plan["blocks"].as_array().unwrap().iter().filter(|x| x["id"].as_u64().unwrap() >= first_new).map(|x| (x["id"].as_u64().unwrap(), serde_json::from_value(x["at"].clone()).unwrap())).collect();
    new.sort_by(|x, y| x.0.cmp(&y.0));
    eprintln!("city L{li}: {} new blocks", new.len());
    let wards = ["merchant", "craft", "noble", "common", "slum", "docks", "military", "temple", "plaza", "farm", "park", "village"];
    let mut fills: Vec<Value> = Vec::new();
    for (i, w) in wards.iter().enumerate() {
        let mut f = json!({ "kind": "generated", "ward": w });
        if i % 4 != 3 {
            f["lots"] = json!(worldgen::world::LOT_SIZES[i % 4]);
        }
        if i % 3 != 1 {
            f["height"] = json!(town::hand::HEIGHTS[i % 3]);
        }
        fills.push(f);
    }
    fills.push(json!({ "kind": "generated", "ward": "common", "reroll": 2 }));
    fills.push(json!({ "kind": "hand" }));
    for (open, style) in [("plaza", Some("market")), ("lawn", None), ("park", Some("formal")), ("garden", None), ("market", None), ("cemetery", None), ("farmland", None)] {
        let mut f = json!({ "kind": "open", "open": open });
        if let Some(s) = style {
            f["style"] = json!(s);
        }
        fills.push(f);
    }
    // (Spread round the city: every fill on blocks a stride apart; the outer blocks farms and parks.)
    let stride = (new.len() / fills.len()).max(1);
    for (i, f) in fills.iter().enumerate() {
        if let Some(&(id, _)) = new.get(i * stride) {
            b.ask(li, json!({ "op": "set_block", "block": id, "fill": f }), &format!("fill k{id} {f}"));
        }
    }
    let far = rings.last().copied().unwrap_or(0.0) - RING_STEP;
    for (n, &(id, at)) in new.iter().enumerate().filter(|(_, x)| dist(x.1, o) > far) {
        if n % stride != 0 {
            let f = match n % 4 {
                0 => json!({ "kind": "generated", "ward": "farm", "lots": "huge" }),
                2 => json!({ "kind": "open", "open": "park", "style": "common" }),
                _ => json!({ "kind": "open", "open": "farmland" }),
            };
            b.ask(li, json!({ "op": "set_block", "block": id, "fill": f }), &format!("outer k{id} at {at:?}"));
        }
    }
    for (i, name) in ["Newmarket", "Weavers' End", "Ropewalk", "Highfields", "Saltgate", "Orchard Ward"].iter().enumerate() {
        if let Some(&(id, _)) = new.get(i * new.len() / 6 + 1) {
            b.ask(li, json!({ "op": "set_block", "block": id, "district": name }), &format!("district {name}"));
        }
    }
    // Squares where streets out meet the first and middle rings.
    let plan = b.plan(li);
    let js: Vec<(u64, P, u64)> = plan["junctions"].as_array().unwrap().iter().map(|j| (j["id"].as_u64().unwrap(), serde_json::from_value(j["p"].clone()).unwrap(), j["streets"].as_u64().unwrap())).collect();
    for (k, kind) in [(3usize, "well"), (9, "fountain"), (15, "green"), (21, "crossroads")] {
        let Some((s, e)) = rays.iter().skip(k).flatten().next().copied() else { continue };
        let Some(p) = on_circle(s, e, o, rings[(k / 6).min(rings.len() - 1)]) else { continue };
        if let Some(j) = js.iter().filter(|j| j.2 >= 4).min_by(|x, y| dist(x.1, p).total_cmp(&dist(y.1, p))).filter(|j| dist(j.1, p) < 10.0) {
            b.ask(li, json!({ "op": "set_square", "junction": j.0, "square": kind }), &format!("square {kind}"));
        }
    }
    // Rows both sides of a long street (the longest new one).
    let streets: Vec<(u64, f64)> = plan["streets"].as_array().unwrap().iter().filter_map(|s| {
        let (a, z) = (s["a"].as_u64()?, s["b"].as_u64()?);
        let p = |id: u64| js.iter().find(|j| j.0 == id).map(|j| j.1);
        let (pa, pz) = (p(a)?, p(z)?);
        (dist(pa, o) > ring0 && dist(pz, o) > ring0).then(|| (s["id"].as_u64().unwrap(), dist(pa, pz)))
    }).collect();
    if let Some(&(st, _)) = streets.iter().max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0))) {
        b.ask(li, json!({ "op": "add_row", "street": st, "side": "l", "depth": 45, "ward": "merchant" }), "row l");
        b.ask(li, json!({ "op": "add_row", "street": st, "side": "r", "depth": 40, "ward": "craft", "lots": "small", "gaps": 0.12 }), "row r");
    }
    // Painted areas in two generated blocks: one paved, one open.
    let plan = b.plan(li);
    let commons: Vec<P> = plan["blocks"].as_array().unwrap().iter().filter(|x| x["id"].as_u64().unwrap() >= first_new && x["fill"]["kind"] == "generated" && x["fill"]["ward"] == "common").filter_map(|x| serde_json::from_value(x["at"].clone()).ok()).collect();
    for (i, open) in [(commons.len() / 3, "plaza"), (2 * commons.len() / 3, "lawn")] {
        if let Some(&c) = commons.get(i) {
            let poly: Vec<P> = (0..6).map(|k| q(polar(c, 55.0, k as f64 * TAU / 6.0))).collect();
            b.ask(li, json!({ "op": "add_area", "poly": poly, "open": open }), &format!("area {open}"));
        }
    }
    // The wall round the grown part: just outside each ray's outer ring, over the longest run of grown rays;
    // a gate where each street out runs past it.
    let grown: Vec<usize> = (0..RAYS).filter(|&k| rays[k].is_some()).collect();
    let first = (0..RAYS).find(|&k| rays[k].is_none()).unwrap_or(0);
    let mut best: Vec<usize> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    for i in 1..=RAYS {
        let k = (first + i) % RAYS;
        if rays[k].is_some() {
            cur.push(k);
        }
        if rays[k].is_none() || i == RAYS {
            if cur.len() > best.len() {
                best = std::mem::take(&mut cur);
            }
            cur.clear();
        }
    }
    let wall: Vec<P> = best.iter().map(|&k| q(polar(o, rings[outer[k].unwrap()] + 70.0, k as f64 * step))).collect();
    let gates: Vec<P> = best.iter().filter_map(|&k| {
        let (s, e) = rays[k]?;
        on_circle(s, e, o, rings[outer[k].unwrap()] + 70.0).map(q)
    }).collect();
    eprintln!("city L{li}: {} rays grown, wall over {} with {} gates", grown.len(), best.len(), gates.len());
    if wall.len() >= 2 {
        b.ask(li, json!({ "op": "add_wall", "pts": wall, "gates": gates, "name": "New Wall" }), "wall");
    }
}

/// A blank town some miles off, laid out from a street plan in the manner of a walled market town of
/// before 1900 (authored here, not traced from a real map): a main street from the west gate to the east,
/// widening at a market place; the church in its close behind the market; back lanes, cross lanes, a north
/// and a south street out to their gates; a wall round it. Returns its layout.
fn blank_town(b: &mut Build, near: usize) -> usize {
    let s = &b.t0.settlements[near];
    let (sx, sy) = (s.x, s.y);
    // A spot on dry land 4 to 9 mi from the city where a town may stand, with room round it.
    let mut at = None;
    'find: for ring in 4..=9 {
        for k in 0..24 {
            let a = k as f64 * std::f64::consts::TAU / 24.0;
            let c = q(polar([sx, sy], ring as f64 * 5280.0, a));
            if (0..16).any(|i| !b.land(polar(c, 1_300.0, i as f64 * std::f64::consts::TAU / 16.0))) || !b.land(c) {
                continue;
            }
            if worldgen::t0::made::spot(&b.world, &b.t0, "town", "c:0", c).is_ok() {
                at = Some(c);
                break 'find;
            }
        }
    }
    let c = at.expect("a spot for the blank town");
    // (As create_settlement makes it: the culture and living from the ground there, its name.)
    let id = format!("c:{}", b.world.file.edits.created.len());
    let culture = worldgen::t0::made::culture_at(&b.t0, c[0], c[1]);
    let kind = worldgen::t0::made::kind_at(&b.world, &b.t0, c[0], c[1], worldgen::t0::made::tier_of("town"));
    let name = worldgen::t0::made::name(&b.world, &b.t0, &id, culture);
    let town = Created { id, kind: "town".into(), x: c[0], y: c[1], name: name.clone(), culture: Some(culture), lives_by: Some(PIN_KINDS[kind as usize].into()), ..Default::default() };
    town.check().expect("a town");
    let lb = b.t0.settlements.len() + b.t0.base_pois + b.world.file.edits.created.len();
    b.create(vec![town]);
    let recs = town::hand::blank(&b.world, &b.t0, lb).expect("blank records");
    let mut f = b.world.file.clone();
    f.edits.streets.extend(recs);
    b.reload(f);
    // Its roads in run to the centre: taken away (their ends kept), the plan drawn, then each end joined
    // to the nearest gate road.
    let plan = b.plan(lb);
    let pj = |plan: &Value, id: u64| -> P { serde_json::from_value(plan["junctions"].as_array().unwrap().iter().find(|j| j["id"].as_u64() == Some(id)).unwrap()["p"].clone()).unwrap() };
    let mut ends: Vec<P> = Vec::new();
    for st in plan["streets"].as_array().unwrap().clone() {
        let (a, z) = (st["a"].as_u64().unwrap(), st["b"].as_u64().unwrap());
        ends.push(pj(&plan, if a == 0 { z } else { a }));
        b.ask(lb, json!({ "op": "delete_street", "street": st["id"] }), "road in taken away");
    }
    let at = |x: f64, y: f64| q([c[0] + x, c[1] + y]);
    // (Town-local ft, y down the map; the market place is the main street widened between two side lanes.)
    let plan_lines: Vec<(Vec<P>, f64, &str)> = vec![
        // The main street, west gate to east gate, bending a little as old main streets do.
        (vec![at(-1_050.0, 30.0), at(-640.0, 10.0), at(-230.0, 0.0), at(230.0, 0.0), at(640.0, -20.0), at(1_050.0, -60.0)], 30.0, "main"),
        // The north back lane, behind the market and the church.
        (vec![at(-640.0, -270.0), at(-230.0, -270.0), at(230.0, -270.0), at(640.0, -290.0)], 16.0, "street"),
        // The south back lane.
        (vec![at(-640.0, 300.0), at(-230.0, 300.0), at(230.0, 300.0), at(640.0, 280.0)], 16.0, "street"),
        // The market's two side lanes, on to the church lane.
        (vec![at(-230.0, 300.0), at(-230.0, 0.0), at(-230.0, -270.0), at(-200.0, -560.0)], 18.0, "street"),
        (vec![at(230.0, 300.0), at(230.0, 0.0), at(230.0, -270.0), at(200.0, -560.0)], 18.0, "street"),
        // The church lane north of the close.
        (vec![at(-200.0, -560.0), at(200.0, -560.0)], 14.0, "street"),
        // The north street, out of the north gate.
        (vec![at(200.0, -560.0), at(260.0, -900.0), at(280.0, -1_150.0)], 22.0, "main"),
        // The south street, out of the south gate.
        (vec![at(-230.0, 300.0), at(-260.0, 700.0), at(-280.0, 1_100.0)], 22.0, "main"),
        // The cross lanes.
        (vec![at(-640.0, 300.0), at(-640.0, 10.0), at(-640.0, -270.0)], 14.0, "street"),
        (vec![at(640.0, 280.0), at(640.0, -20.0), at(640.0, -290.0)], 14.0, "street"),
    ];
    for (i, (pts, w, class)) in plan_lines.iter().enumerate() {
        b.ask(lb, json!({ "op": "add_street", "points": pts, "width": w, "set_l": 0.5 * w, "set_r": 0.5 * w, "class": class }), &format!("blank street {i}"));
    }
    // Roads in to the nearest gate street's outer end.
    let gate_ends = [at(-1_050.0, 30.0), at(1_050.0, -60.0), at(280.0, -1_150.0), at(-280.0, 1_100.0)];
    for (i, e) in ends.iter().enumerate() {
        let g = *gate_ends.iter().min_by(|x, y| dist(**x, *e).total_cmp(&dist(**y, *e))).unwrap();
        if dist(g, *e) > 30.0 {
            b.ask(lb, json!({ "op": "add_street", "points": [g, q(*e)], "width": 14.0, "set_l": 7.0, "set_r": 7.0, "class": "main" }), &format!("blank road in {i}"));
        }
    }
    // The blocks: the market place, the church close, the merchants on the main street, crafts behind.
    let plan = b.plan(lb);
    let blocks: Vec<(u64, P)> = plan["blocks"].as_array().unwrap().iter().map(|x| (x["id"].as_u64().unwrap(), serde_json::from_value(x["at"].clone()).unwrap())).collect();
    let block_at = |p: P| -> Option<u64> {
        // (The block whose `at` is nearest: faces here are a few hundred ft across.)
        blocks.iter().min_by(|x, y| dist(x.1, p).total_cmp(&dist(y.1, p)).then(x.0.cmp(&y.0))).map(|x| x.0)
    };
    let fills = [
        ((0.0, -130.0), json!({ "kind": "open", "open": "market" })),
        ((0.0, 150.0), json!({ "kind": "generated", "ward": "merchant", "height": "tall" })),
        ((0.0, -420.0), json!({ "kind": "generated", "ward": "temple", "close": true })),
        ((-430.0, -130.0), json!({ "kind": "generated", "ward": "merchant" })),
        ((430.0, -140.0), json!({ "kind": "generated", "ward": "craft", "lots": "small" })),
        ((-430.0, 150.0), json!({ "kind": "generated", "ward": "common" })),
        ((430.0, 140.0), json!({ "kind": "generated", "ward": "noble", "lots": "large" })),
    ];
    for ((x, y), f) in fills {
        if let Some(id) = block_at(at(x, y)) {
            b.ask(lb, json!({ "op": "set_block", "block": id, "fill": f }), &format!("blank fill {f}"));
        }
    }
    // The town wall: an oval round the plan, gates where the four streets out cross it.
    let wall: Vec<P> = (0..20).map(|k| {
        let a = k as f64 * std::f64::consts::TAU / 20.0;
        at(880.0 * libm::cos(a), 760.0 * libm::sin(a))
    }).collect();
    b.ask(lb, json!({ "op": "add_wall", "pts": wall, "closed": true, "name": "Town Wall" }), "blank wall");
    lb
}

/// Quick placement round a generated city (the biggest city not laid out by hand): a stamp and a building row
/// along one of its roads in, and a parcel with a fill beside it, as the app's Build tools put them down.
fn quick_placement(b: &mut Build, city: usize) {
    let lc = (0..b.t0.settlements.len()).filter(|&i| i != city && b.t0.settlements[i].tier == Tier::City).max_by_key(|&i| (b.t0.settlements[i].population, std::cmp::Reverse(i))).expect("a city");
    let l = town::base_layout(&b.world, &b.t0, lc);
    // Its longest road in (an approach from a road's end to its gate).
    let len = |v: &[P]| v.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>();
    let road: Vec<P> = l.roads.iter().filter(|r| r.1 <= 2).max_by(|x, y| len(&x.0).total_cmp(&len(&y.0))).map(|r| r.0.clone()).expect("a road in");
    eprintln!("quick placement: city L{lc}, road in {:.0} ft", len(&road));
    let n = b.world.file.edits.created.len();
    // A row along it, both sides.
    let r: Value = serde_json::from_str(&worldgen::placing::row_json(&b.world, &b.t0, &json!({ "line": road, "side": "both", "ward": "craft", "gaps": 0.15, "seed": 7, "first_id": n }).to_string())).unwrap();
    if let Some(e) = r["error"].as_str() {
        b.refused.push(format!("row: {e}"));
    }
    let made: Vec<Created> = r["lots"].as_array().into_iter().flatten().filter(|x| x["existing"].is_null()).enumerate().map(|(k, x)| Created {
        id: format!("c:{}", n + k),
        kind: "building".into(),
        x: x["x"].as_f64().unwrap(),
        y: x["y"].as_f64().unwrap(),
        name: x["name"].as_str().unwrap_or_default().into(),
        poly: serde_json::from_value(x["poly"].clone()).unwrap(),
        func: x["func"].as_str().map(str::to_string),
        floors: x["floors"].as_u64().map(|f| f as u8),
        ..Default::default()
    }).collect();
    eprintln!("quick placement: row {} buildings, {} refused", made.len(), r["refused_count"]);
    if made.is_empty() {
        b.refused.push("row: no building".into());
    }
    b.create(made);
    // Stamps: an inn, a house, a smithy, a little way along the road in.
    let mid = road[road.len() / 2];
    for (i, (func, size)) in [("inn", "usual"), ("house", "small"), ("blacksmith", "large")].iter().enumerate() {
        let id = format!("c:{}", b.world.file.edits.created.len());
        let p = [mid[0] + 90.0 * (i as f64 - 1.0), mid[1] + 140.0];
        let s: Value = serde_json::from_str(&worldgen::placing::stamp_json(&b.world, &b.t0, &json!({ "func": func, "size": size, "x": p[0], "y": p[1], "id": id }).to_string())).unwrap();
        if let Some(e) = s["error"].as_str() {
            b.refused.push(format!("stamp {func}: {e}"));
            continue;
        }
        let c = Created { id, kind: "building".into(), x: s["x"].as_f64().unwrap(), y: s["y"].as_f64().unwrap(), name: s["name"].as_str().unwrap_or_default().into(), poly: serde_json::from_value(s["poly"].clone()).unwrap(), func: Some(func.to_string()), ..Default::default() };
        b.create(vec![c]);
    }
    // A parcel off the road (the first outline that is free, further out each time), generated crafts.
    let m = road.len() / 2;
    let (u, w) = (road[m.saturating_sub(1)], road[(m + 1).min(road.len() - 1)]);
    let d = dist(u, w).max(1e-9);
    let nrm = [-(w[1] - u[1]) / d, (w[0] - u[0]) / d];
    for k in 0..12 {
        let off = (if k % 2 == 0 { 1.0 } else { -1.0 }) * (240.0 + 90.0 * (k / 2) as f64);
        let c = [mid[0] + nrm[0] * off, mid[1] + nrm[1] * off];
        let poly: Vec<P> = [[-110.0, -80.0], [110.0, -80.0], [110.0, 80.0], [-110.0, 80.0]].iter().map(|d| q([c[0] + d[0], c[1] + d[1]])).collect();
        let r: Value = serde_json::from_str(&worldgen::placing::parcel_check_json(&b.world, &b.t0, &json!({ "poly": poly }).to_string())).unwrap();
        if let Some(e) = r["error"].as_str() {
            eprintln!("quick placement: parcel {k}: {e}");
            continue;
        }
        let id = format!("c:{}", b.world.file.edits.created.len());
        let fill = serde_json::from_value(json!({ "kind": "generated", "ward": "craft", "lots": "small" })).unwrap();
        let p = Created { id, kind: "plot".into(), x: r["x"].as_f64().unwrap(), y: r["y"].as_f64().unwrap(), name: "Tanners' Yard".into(), poly, fill: Some(fill), ..Default::default() };
        p.check().expect("a plot");
        b.create(vec![p]);
        return;
    }
    b.refused.push("parcel: no free outline".into());
}

/// A harbour basin joining the sea beyond the grown city's wall, and a plateau stack (a terrace on a
/// terrace) on dry land beside it.
fn water_and_ground(b: &mut Build, li: usize) {
    use std::f64::consts::TAU;
    let s = &b.t0.settlements[li];
    let o = [s.x, s.y];
    let sea = b.world.params().sea_level_ft;
    // The coast: the first land-to-sea step along a ray past the city.
    let mut coast = None;
    'find: for k in 0..RAYS {
        let a = k as f64 * TAU / RAYS as f64;
        let mut r = GROWN_R + 1_200.0;
        while r < GROWN_R + 6_000.0 {
            if b.land(polar(o, r, a)) && !b.land(polar(o, r + 100.0, a)) {
                coast = Some((polar(o, r + 50.0, a), a));
                break 'find;
            }
            r += 100.0;
        }
    }
    let (c, a) = coast.expect("a coast past the city");
    let (u, v) = ([libm::cos(a), libm::sin(a)], [-libm::sin(a), libm::cos(a)]);
    let corner = |x: f64, y: f64| worldgen::waters::rounded(&[[c[0] + u[0] * x + v[0] * y, c[1] + u[1] * x + v[1] * y]])[0];
    let basin = worldgen::waters::Water { pts: vec![corner(-260.0, -180.0), corner(260.0, -200.0), corner(260.0, 200.0), corner(-260.0, 180.0), corner(-330.0, 0.0)], kind: worldgen::waters::WaterKind::Basin, level: sea as f32 };
    if let Some(e) = basin.problem() {
        b.refused.push(format!("basin: {e}"));
    } else {
        let mut f = b.world.file.clone();
        f.edits.waters.insert("e:harbour".into(), basin);
        f.edits.renames.insert("e:harbour".into(), "Newhaven Basin".into());
        b.reload(f);
    }
    // The plateau stack: inland of the basin, a terrace 12 ft up and one 12 ft above that inside it.
    let mid = [c[0] - u[0] * 1_400.0, c[1] - u[1] * 1_400.0];
    let mut ex = Executor::new(World::new(b.world.file.clone()).unwrap());
    let ground = |ex: &mut Executor, p: P, e: &Edits| Squares::round(ex, [p[0], p[1], p[0], p[1]], 0).shaped(e, None)[0] as f64;
    let here = ground(&mut ex, mid, &b.world.file.edits);
    for (i, (r, rise)) in [(320.0, 12.0), (150.0, 24.0)].iter().enumerate() {
        let outline: Vec<P> = (0..9).map(|k| q(polar(mid, *r * if k % 2 == 0 { 1.0 } else { 0.86 }, k as f64 * TAU / 9.0))).collect();
        match worldgen::ground::plateau_for(&outline, here + rise, 8.0) {
            Ok(p) => {
                let mut f = b.world.file.clone();
                f.edits.plateaus.insert(format!("h:stack{i}"), p);
                b.reload(f);
            }
            Err(e) => b.refused.push(format!("plateau {i}: {e}")),
        }
    }
}
