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
//! stack. And what Roadmap 6 R3-R7 added, away from those (`roadmap6`): a row with its street joined to a generated town,
//! another town's outskirts pushed out into a place, a road stretch, a village lane and a common's path removed, a croft
//! hedge removed, a field's side cut by the eraser and a picket fence drawn with a gate; the largest generated walled
//! town's main streets straight in (found, not made). Every refusal is printed; the run fails if any step that should
//! work is refused.

use serde_json::{Value, json};
use worldgen::geoquery::Squares;
use worldgen::pipeline::Executor;
use worldgen::t0::{T0, settle::Tier};
use worldgen::town::{self, handedit};
use worldgen::world::{Created, CreatedChecks, Edits, PIN_KINDS};
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

    let first_new = grow_city(&mut b, li);
    castles(&mut b, li, first_new);
    let lb = blank_town(&mut b, li);
    let lc = quick_placement(&mut b, li);
    water_and_ground(&mut b, li);
    roadmap6(&mut b, &[li, lb, lc]);

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
fn grow_city(b: &mut Build, li: usize) -> u64 {
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
    // (Every grown block on dry land: the streets out run only as far as the land does.)
    let plan = b.plan(li);
    for x in plan["blocks"].as_array().unwrap().iter().filter(|x| x["id"].as_u64().unwrap() >= first_new) {
        let at: P = serde_json::from_value(x["at"].clone()).unwrap();
        if !b.land(at) {
            b.refused.push(format!("grown block k{} at {at:?} off the land", x["id"]));
        }
    }
    first_new
}

/// The castles (Roadmap 6 P7): a concave castle beside the grown city, a U open toward it (~700 x 500 ft,
/// its notch 240 ft wide), three gates, a round keep, a wet moat, put down as the app's Castle tool does
/// (`works_spot`: on dry land, nothing in its way); and a grown block of the city given the Castle fill
/// (three gates, one toward each of three of its streets; a round keep, a wet moat).
fn castles(b: &mut Build, li: usize, first_new: u64) {
    use std::f64::consts::TAU;
    let plan = b.plan(li);
    let o: P = serde_json::from_value(plan["origin"].clone()).unwrap();
    // The U: corners about its middle, its open side toward `+y` before it is turned to face the city.
    let u = [[-350.0, -250.0], [350.0, -250.0], [350.0, 250.0], [120.0, 250.0], [120.0, 20.0], [-120.0, 20.0], [-120.0, 250.0], [-350.0, 250.0]];
    let id = format!("c:{}", b.world.file.edits.created.len());
    let mut put = None;
    'find: for k in 0..RAYS {
        let a = (k as f64 + 0.5) * TAU / RAYS as f64;
        let mut r = GROWN_R + 1_000.0;
        while r <= GROWN_R + 5_000.0 {
            let c = q(polar(o, r, a));
            // (Turned so its notch opens toward the city: local +y along -radial.)
            let (ey, ex) = ([-libm::cos(a), -libm::sin(a)], [libm::sin(a), -libm::cos(a)]);
            let poly: Vec<P> = u.iter().map(|d| q([c[0] + ex[0] * d[0] + ey[0] * d[1], c[1] + ex[1] * d[0] + ey[1] * d[1]])).collect();
            r += 250.0;
            if !poly.iter().chain([&c]).all(|p| b.land(*p)) {
                continue;
            }
            let cr = Created { id: id.clone(), kind: "castle".into(), x: c[0], y: c[1], poly, name: "Castle Ward".into(), gates: vec![0, 1, 7], keep_shape: Some("round".into()), moat: Some("wet".into()), ..Default::default() };
            if cr.check().is_err() {
                continue;
            }
            match worldgen::agent::works_spot(&b.world, &b.t0, &cr, &id, None) {
                Ok(spot) if spot.in_way.is_empty() && spot.gates_not_built.is_empty() => {
                    put = Some(Created { x: spot.at[0], y: spot.at[1], ..cr });
                    break 'find;
                }
                _ => {}
            }
        }
    }
    match put {
        Some(c) => {
            eprintln!("castle {id} at ({:.0}, {:.0})", c.x, c.y);
            b.create(vec![c]);
        }
        None => b.refused.push("castle: no open ground beside the city".into()),
    }
    // The castle block: a grown block left as it came (a common ward), on dry land, at ~7,000 ft out.
    let js: std::collections::BTreeMap<u64, P> = plan["junctions"].as_array().unwrap().iter().map(|j| (j["id"].as_u64().unwrap(), serde_json::from_value(j["p"].clone()).unwrap())).collect();
    let sts: std::collections::BTreeMap<u64, (u64, u64)> = plan["streets"].as_array().unwrap().iter().map(|s| (s["id"].as_u64().unwrap(), (s["a"].as_u64().unwrap(), s["b"].as_u64().unwrap()))).collect();
    let pick = plan["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["id"].as_u64().unwrap() >= first_new && x["fill"] == json!({ "kind": "generated", "ward": "common" }) && x["district"].is_null())
        .filter_map(|x| {
            let at: P = serde_json::from_value(x["at"].clone()).ok()?;
            let round: Vec<u64> = x["streets"].as_array()?.iter().filter_map(|s| s.as_u64()).collect();
            (round.len() >= 3 && b.land(at)).then(|| (x["id"].as_u64().unwrap(), at, round))
        })
        .min_by(|x, y| (dist(x.1, o) - 7_000.0).abs().total_cmp(&(dist(y.1, o) - 7_000.0).abs()).then(x.0.cmp(&y.0)));
    let Some((bk, at, round)) = pick else {
        b.refused.push("castle block: no grown block".into());
        return;
    };
    // (A gate toward each of its first three streets, the main one toward the city's middle first.)
    let mut gates: Vec<P> = round.iter().filter_map(|s| sts.get(s)).map(|&(a, z)| q([0.5 * (js[&a][0] + js[&z][0]), 0.5 * (js[&a][1] + js[&z][1])])).collect();
    gates.sort_by(|p, r| dist(*p, o).total_cmp(&dist(*r, o)));
    gates.truncate(3);
    eprintln!("castle block k{bk} at ({:.0}, {:.0}), {} gates", at[0], at[1], gates.len());
    b.ask(li, json!({ "op": "set_block", "block": bk, "fill": { "kind": "castle", "gates": gates, "keep_shape": "round", "moat": "wet" } }), &format!("castle block k{bk}"));
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
fn quick_placement(b: &mut Build, city: usize) -> usize {
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
        return lc;
    }
    b.refused.push("parcel: no free outline".into());
    lc
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
    let basin = worldgen::waters::Water::outline(vec![corner(-260.0, -180.0), corner(260.0, -200.0), corner(260.0, 200.0), corner(-260.0, 180.0), corner(-330.0, 0.0)], worldgen::waters::WaterKind::Basin, sea as f32);
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

/// (R7, as `towncheck streetsin` measures it) Each main street of a walled layout: its length, its straight run
/// from the gate (ft, up to the first piece 3 ft or longer heading over 20 degrees off the gate's inward normal)
/// and its first 100 ft's heading off that normal (degrees). The normal: across the line best fitting the wall
/// within 50 ft of the gate, turned toward the walls' mean point. None: no wall at the gate.
fn straight_in(l: &town::Layout) -> Vec<Option<(f64, f64, f64)>> {
    use worldgen::town::geom::{add, dot, lerp, mul, seg_dist, sub};
    let segs: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).filter(|s| dist(s.0, s.1) > 1e-6).collect();
    let pts: Vec<P> = l.walls.iter().flatten().copied().collect();
    let centre = mul(pts.iter().fold([0.0, 0.0], |a, &p| add(a, p)), 1.0 / pts.len().max(1) as f64);
    l.streets
        .iter()
        .map(|st| {
            let total: f64 = st.windows(2).map(|w| dist(w[0], w[1])).sum();
            let g = st[0];
            let near: Vec<P> = segs
                .iter()
                .filter(|(a, b)| seg_dist(g, *a, *b) < 50.0)
                .flat_map(|&(a, b)| {
                    let n = (dist(a, b) / 2.0).ceil().max(1.0) as usize;
                    (0..=n).map(move |j| lerp(a, b, j as f64 / n as f64))
                })
                .filter(|p| dist(*p, g) < 50.0)
                .collect();
            let m = mul(near.iter().fold([0.0, 0.0], |a, &p| add(a, p)), 1.0 / near.len().max(1) as f64);
            let (sxx, syy, sxy) = near.iter().fold((0.0, 0.0, 0.0), |c, p| {
                let d = sub(*p, m);
                (c.0 + d[0] * d[0], c.1 + d[1] * d[1], c.2 + d[0] * d[1])
            });
            if near.len() < 2 || sxx + syy < 1e-6 || total < 1e-6 {
                return None;
            }
            let th = 0.5 * libm::atan2(2.0 * sxy, sxx - syy);
            let mut nrm = [-libm::sin(th), libm::cos(th)];
            if dot(nrm, sub(centre, g)) < 0.0 {
                nrm = mul(nrm, -1.0);
            }
            let (mut s, mut p100) = (0.0f64, st[st.len() - 1]);
            for w in st.windows(2) {
                let d = dist(w[0], w[1]);
                if s + d >= 100.0 {
                    p100 = lerp(w[0], w[1], (100.0 - s) / d.max(1e-9));
                    break;
                }
                s += d;
            }
            let off = libm::acos(dot(mul(sub(p100, g), 1.0 / dist(p100, g).max(1e-9)), nrm).clamp(-1.0, 1.0)).to_degrees();
            let mut run = 0.0;
            for w in st.windows(2) {
                let d = dist(w[0], w[1]);
                if d >= 3.0 && dot(mul(sub(w[1], w[0]), 1.0 / d), nrm) < libm::cos(20f64.to_radians()) {
                    break;
                }
                run += d;
            }
            Some((total, run, off))
        })
        .collect()
}

/// Whether a walled town's main streets all go straight in (R7): two or more, each 100 ft or longer straight
/// for 100 ft from its gate, its first 100 ft within 10 degrees of square to the wall.
fn goes_straight_in(l: &town::Layout) -> bool {
    let s = straight_in(l);
    s.len() >= 2 && s.iter().all(|x| x.is_some_and(|(total, run, off)| total < 100.0 || (run >= 100.0 && off <= 10.0)))
}

/// Roadmap 6 R3-R7, away from the hand towns and the quick placement's city (`avoid`: their layouts, the city
/// first): a row with a street laid along it, joined to a generated town (R3, R4); another town's outskirts
/// pushed out into a place of their own (R3); a stretch of the world's roads, a village's lane and a common's
/// path removed (R5); in that village a croft hedge removed whole, a field's side cut by the eraser and a
/// picket fence drawn with a gate (R6). And the walled town whose main streets go straight in (R7), found.
fn roadmap6(b: &mut Build, avoid: &[usize]) {
    use std::f64::consts::TAU;
    use worldgen::battlemap as bm;
    use worldgen::fencecut::{self, Op};
    use worldgen::model::fences::{Fence, FenceGap, FenceKind};
    use worldgen::town::geom::{add, centroid, lerp, mul, sub};
    use worldgen::town::{ParkKind, cut, fence, members, places::Spot};
    let len = |v: &[P]| v.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>();
    let mut keep_off: Vec<(P, f64)> = avoid.iter().enumerate().map(|(k, &li)| (town::layout(&b.world, &b.t0, li).center, if k == 0 { GROWN_R + 10_000.0 } else { 8_000.0 })).collect();
    let n = b.t0.settlements.len();
    let at = |b: &Build, i: usize| [b.t0.settlements[i].x, b.t0.settlements[i].y];
    let by_size = |b: &Build, keep_off: &[(P, f64)], tier: Tier| {
        let mut v: Vec<usize> = (0..n).filter(|&i| b.t0.settlements[i].tier == tier && !town::hand::is_hand(&b.world, i) && keep_off.iter().all(|&(c, r)| dist(at(b, i), c) > r)).collect();
        v.sort_by_key(|&i| (std::cmp::Reverse(b.t0.settlements[i].population), i));
        v
    };
    let seed = b.world.stream("battlemap");
    let reload = |b: &mut Build, f: WorldFile| {
        b.ops += 1;
        b.reload(f);
        b.t0.apply_edits(&b.world);
    };

    // A row with its street (paved) out from an unwalled town's edge, its houses joined to the town; that town's
    // common's longest path (R5, below).
    let mut row_town = None;
    'town: for ti in by_size(b, &keep_off, Tier::Town) {
        let l = town::layout(&b.world, &b.t0, ti);
        if !l.walls.is_empty() {
            continue;
        }
        let path = l.parks.iter().enumerate().filter(|(_, p)| p.kind == ParkKind::Common && p.spot != Spot::None).find_map(|(k, pk)| {
            let lay = bm::park_layout(pk, bm::land_biome(&b.t0, centroid(&pk.pts)), l.tier >= Tier::City, seed);
            let j = (0..lay.paths.len()).max_by(|&x, &y| len(&lay.paths[x].0).total_cmp(&len(&lay.paths[y].0)))?;
            (len(&lay.paths[j].0) >= 40.0).then(|| cut::path_id(ti, &cut::park_spot(&l, k), j))
        });
        let Some(path) = path else { continue };
        let c = at(b, ti);
        for k in 0..16 {
            let a = k as f64 * TAU / 16.0;
            let u = [libm::cos(a), libm::sin(a)];
            let Some(e) = l.buildings.iter().flat_map(|x| x.poly.iter()).copied().max_by(|p, q| worldgen::town::geom::dot(sub(*p, c), u).total_cmp(&worldgen::town::geom::dot(sub(*q, c), u))) else { continue 'town };
            let line = [q(add(e, mul(u, 40.0))), q(add(e, mul(u, 440.0)))];
            if !(0..=8).all(|t| b.land(lerp(line[0], line[1], t as f64 / 8.0))) {
                continue;
            }
            let first = b.world.file.edits.created.len();
            let r: Value = serde_json::from_str(&worldgen::placing::row_json(&b.world, &b.t0, &json!({ "line": line, "side": "both", "ward": "common", "gaps": 0.1, "seed": 5, "first_id": first, "road": { "kind": "paved" } }).to_string())).unwrap();
            if let Some(e) = r["error"].as_str() {
                eprintln!("roadmap 6: a row out of L{ti} heading {k}: {e}");
                continue;
            }
            let lots: Vec<&Value> = r["lots"].as_array().into_iter().flatten().filter(|x| x["existing"].is_null()).collect();
            let road = &r["road"];
            if lots.len() < 6 || road["as"] != "street" || road["town"].as_u64() != Some(ti as u64) {
                eprintln!("roadmap 6: a row out of L{ti} heading {k}: {} houses, its road {} of {}", lots.len(), road["as"], road["town"]);
                continue;
            }
            let mut f = b.world.file.clone();
            for (key, te) in road["towns"].as_object().into_iter().flatten() {
                f.edits.towns.insert(key.clone(), serde_json::from_value(te.clone()).unwrap());
            }
            for (k, x) in lots.iter().enumerate() {
                let c = Created {
                    id: format!("c:{}", first + k),
                    kind: "building".into(),
                    x: x["x"].as_f64().unwrap(),
                    y: x["y"].as_f64().unwrap(),
                    name: x["name"].as_str().unwrap_or_default().into(),
                    poly: serde_json::from_value(x["poly"].clone()).unwrap(),
                    func: x["func"].as_str().map(str::to_string),
                    floors: x["floors"].as_u64().map(|f| f as u8),
                    faces: serde_json::from_value(x["faces"].clone()).ok(),
                    ..Default::default()
                };
                c.check().expect("a row's house");
                f.edits.created.push(c);
            }
            eprintln!("roadmap 6: a row of {} houses out of L{ti} heading {k}, its street {}", lots.len(), road["street_id"]);
            reload(b, f);
            row_town = Some((ti, path));
            break 'town;
        }
    }
    let Some((ti, path)) = row_town else {
        b.refused.push("roadmap 6: no unwalled town with a common took a row with its street".into());
        return;
    };
    keep_off.push((at(b, ti), 8_000.0));

    // Another town's outskirts pushed out: the first building that may go and up to three more within 200 ft.
    let mut pushed = None;
    for t2 in by_size(b, &keep_off, Tier::Town) {
        let l = town::layout(&b.world, &b.t0, t2);
        let ok = |b: &Build, id: &str| members::change(&b.world, &b.t0, t2, &[], &[id.to_string()]).is_ok_and(|c| c.refused.is_empty() && c.created.len() == 1);
        let Some(one) = l.buildings.iter().find(|x| ok(b, &format!("b:{t2}:{}", x.id))) else { continue };
        let m0 = centroid(&one.poly);
        let mut near: Vec<&town::Building> = l.buildings.iter().filter(|x| x.id != one.id && dist(centroid(&x.poly), m0) < 200.0).collect();
        near.sort_by(|x, y| dist(centroid(&x.poly), m0).total_cmp(&dist(centroid(&y.poly), m0)).then(x.id.cmp(&y.id)));
        let mut ids = vec![format!("b:{t2}:{}", one.id)];
        for x in near {
            let id = format!("b:{t2}:{}", x.id);
            if ids.len() < 4 && ok(b, &id) {
                ids.push(id);
            }
        }
        b.ops += 1;
        match members::change(&b.world, &b.t0, t2, &[], &ids) {
            Ok(c) if c.refused.is_empty() && c.created.len() == 1 => {
                let mut f = b.world.file.clone();
                for (k, v) in c.members {
                    if v.is_empty() {
                        f.edits.members.remove(&k);
                    } else {
                        f.edits.members.insert(k, v);
                    }
                }
                eprintln!("roadmap 6: {} of L{t2}'s outskirts pushed out into {} ({})", ids.len(), c.created[0].id, c.created[0].name);
                f.edits.created.extend(c.created);
                reload(b, f);
            }
            Ok(c) => b.refused.push(format!("roadmap 6: pushing out {ids:?}: {:?}, {} made", c.refused, c.created.len())),
            Err(e) => b.refused.push(format!("roadmap 6: pushing out {ids:?}: {e}")),
        }
        pushed = Some(t2);
        break;
    }
    let Some(t2) = pushed else {
        b.refused.push("roadmap 6: no town's outskirts may be pushed out".into());
        return;
    };
    keep_off.push((at(b, t2), 8_000.0));

    // A village: a lane (150 ft or longer) to remove, a croft hedge (30 ft or longer) to remove whole and a
    // field's side (40 ft or longer, 60 ft or more from it) to cut.
    // (Each drawn on its battlemap as generated: its shape within 1 ft of its middle, and of its points 30% and
    // 70% along for the hedge.)
    let mut ex = Executor::new(World::new(b.world.file.clone()).unwrap());
    let g = ex.world.geom.clone();
    let drawn = |ex: &mut Executor, a: P, z: P, ts: &[f64]| {
        let size = g.tile_size_ft(g.max_level);
        let m = lerp(a, z, 0.5);
        let c = ex.battlemap(worldgen::core::tile::TileKey::surface(g.max_level, (m[0] / size) as u32, (m[1] / size) as u32));
        let (ox, oy) = g.tile_origin_ft(&c.key);
        let w = |q: [f32; 2]| [ox + q[0] as f64 * bm::SQUARE_FT, oy + q[1] as f64 * bm::SQUARE_FT];
        let near = |p: P| c.structures.iter().any(|s| matches!(s.kind, bm::ShapeKind::Hedge | bm::ShapeKind::DryWall | bm::ShapeKind::Fence) && s.pts.len() >= 2 && worldgen::town::geom::seg_dist(p, w(s.pts[0]), w(s.pts[1])) < 1.0);
        ts.iter().all(|&t| near(lerp(a, z, t)))
    };
    let mut village = None;
    'village: for vi in by_size(b, &keep_off, Tier::Village) {
        let l = town::layout(&b.world, &b.t0, vi);
        let Some(lane) = cut::street_ids(&b.world, &b.t0, &l).into_iter().find(|x| x.0 == cut::STREETS && len(&l.streets[x.1]) >= 150.0) else { continue };
        let srcs = fence::sources(&l, &b.t0, fence::EVERYWHERE);
        for &hedge in srcs.iter().filter(|s| s.0 == fence::CROFT && s.3 == FenceKind::Hedgerow && dist(s.1, s.2) >= 30.0) {
            let hm = lerp(hedge.1, hedge.2, 0.5);
            if !drawn(&mut ex, hedge.1, hedge.2, &[0.3, 0.5, 0.7]) {
                continue;
            }
            let Some(side) = srcs.iter().find(|s| s.0 == fence::FIELD && dist(s.1, s.2) >= 40.0 && dist(lerp(s.1, s.2, 0.5), hm) > 60.0 && drawn(&mut ex, s.1, s.2, &[0.5])).copied() else { continue 'village };
            village = Some((vi, cut::street_id(vi, lane.2), hedge, side));
            break 'village;
        }
    }
    drop(ex);
    let Some((vi, lane, hedge, side)) = village else {
        b.refused.push("roadmap 6: no village with a lane, a croft hedge and a field's side".into());
        return;
    };
    keep_off.push((at(b, vi), 8_000.0));

    // A stretch of the world's roads 2 mi or longer, away from all of these.
    let ids = worldgen::roadids::road_ids(&b.t0);
    let Some(ri) = (0..ids.len()).find(|&ri| {
        let r = &b.t0.roads.roads[ri];
        len(&r.pts) >= 10_560.0 && r.pts.iter().all(|&p| keep_off.iter().all(|&(c, rr)| dist(p, c) > rr))
    }) else {
        b.refused.push("roadmap 6: no stretch of road away from the towns used".into());
        return;
    };
    // The removals, one at a time (as Select › Delete makes them).
    for id in [ids[ri].clone(), lane.clone(), path.clone()] {
        b.ops += 1;
        let out = worldgen::roadcut::cut(&b.world, &b.t0, std::slice::from_ref(&id), false, false);
        if !out.refused.is_empty() || out.done != [id.clone()] {
            b.refused.push(format!("roadmap 6: removing {id}: {:?}", out.refused));
            continue;
        }
        eprintln!("roadmap 6: removed {id}{}", if out.also.is_empty() { String::new() } else { format!(" (and {:?})", out.also) });
        let mut f = b.world.file.clone();
        out.apply(&mut f.edits);
        reload(b, f);
    }

    // The village's fences: the hedge gone, the eraser over the field's side, a picket fence drawn and a gate put in it.
    let fences = |b: &mut Build, ids: &[String], op: Op, what: &str| -> Vec<String> {
        b.ops += 1;
        let out = fencecut::change(&b.world, &b.t0, ids, &op, false);
        if !out.refused.is_empty() || out.done.is_empty() {
            b.refused.push(format!("roadmap 6: {what}: {:?}", out.refused));
            return Vec::new();
        }
        let mut f = b.world.file.clone();
        out.apply(&mut f.edits);
        reload(b, f);
        out.done
    };
    let (a, z) = fence::canonical(hedge.1, hedge.2);
    fences(b, &[fence::fr_id(vi, fence::run_key(hedge.0, a, z))], Op::Remove, "removing a croft hedge");
    let (sm, d) = (lerp(side.1, side.2, 0.5), sub(side.2, side.1));
    let sn = mul([-d[1], d[0]], 1.0 / dist(side.1, side.2));
    fences(b, &[], Op::Gaps(vec![FenceGap { pts: vec![q(sub(sm, mul(sn, 8.0))), q(add(sm, mul(sn, 8.0)))], r: 4.0 }]), "the eraser over a field's side");
    // (On dry open ground off the roads 400 ft to 2,000 ft from the village's middle.)
    let vc = at(b, vi);
    let spot = (2..=10).flat_map(|ring| (0..16).map(move |k| polar(vc, ring as f64 * 200.0, k as f64 * TAU / 16.0))).find_map(|p| {
        let pts = [q(p), q(add(p, [80.0, 0.0])), q(add(p, [80.0, 50.0]))];
        let r = [p[0] - 40.0, p[1] - 40.0, p[0] + 120.0, p[1] + 90.0];
        let dry = (0..=8).all(|t| b.land(lerp(pts[0], pts[1], t as f64 / 8.0)) && b.land(lerp(pts[1], pts[2], t as f64 / 8.0)));
        let line_d = |v: &[P]| v.windows(2).flat_map(|w| pts.iter().map(move |&p| worldgen::town::geom::seg_dist(p, w[0], w[1]))).fold(f64::MAX, f64::min);
        let open = town::layouts_near(&b.world, &b.t0, r).iter().all(|l| {
            l.buildings.iter().flat_map(|x| x.poly.iter()).all(|p| p[0] < r[0] || p[0] > r[2] || p[1] < r[1] || p[1] > r[3]) && l.roads.iter().all(|x| line_d(&x.0) > 40.0) && l.streets.iter().all(|x| line_d(x) > 40.0)
        });
        let clear = b.t0.roads.segments_near(r, 200.0).iter().all(|&(ri, k)| {
            let v = &b.t0.roads.roads[ri as usize].pts;
            line_d(&v[k as usize..(k as usize + 2).min(v.len())]) > 80.0
        });
        (dry && open && clear).then_some(pts)
    });
    let Some(pts) = spot else {
        b.refused.push("roadmap 6: no open ground for a fence".into());
        return;
    };
    let drawn = fences(b, &[], Op::Draw(Fence { kind: FenceKind::Picket, pts: pts.to_vec(), closed: false, gates: Vec::new() }), "drawing a picket fence");
    if !drawn.is_empty() {
        fences(b, &drawn, Op::Change(None, vec![lerp(pts[0], pts[1], 0.5)]), "a gate in the picket fence");
    }
    eprintln!("roadmap 6: village L{vi}: {lane} and its fences; {:?} drawn", drawn);

    // R7: the largest generated walled town whose main streets all go straight in.
    let mut tried = 0;
    let mut walled: Vec<usize> = (0..n).filter(|&i| b.t0.settlements[i].tier >= Tier::Town && !town::hand::is_hand(&b.world, i)).collect();
    walled.sort_by_key(|&i| (std::cmp::Reverse(b.t0.settlements[i].population), i));
    let found = walled.into_iter().find(|&i| {
        let l = town::layout(&b.world, &b.t0, i);
        if l.walls.is_empty() || l.streets.is_empty() {
            return false;
        }
        tried += 1;
        goes_straight_in(&l)
    });
    match found {
        Some(i) => {
            let s = straight_in(&town::layout(&b.world, &b.t0, i));
            let runs: Vec<String> = s.iter().flatten().map(|x| format!("{:.0} ft straight of {:.0}, {:.1} deg off", x.1, x.0, x.2)).collect();
            eprintln!("roadmap 6: L{i} is walled and its main streets go straight in ({tried} walled towns looked at): {}", runs.join("; "));
        }
        None => b.refused.push(format!("roadmap 6: none of {tried} walled towns has its main streets all straight in")),
    }
}
