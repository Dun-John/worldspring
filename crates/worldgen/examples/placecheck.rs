//! Scratch: how far settlements sit from the coast, rivers, mouths and road junctions.
//! `cargo run --release -p worldgen --example placecheck -- [seed|world.json] [--json]`   (default 1)
//! `--json`: the numbers as one JSON object on stdout (the text report goes to stderr).
mod common;

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};
use worldgen::t0::T0;

/// `--json`: the text report goes to stderr, stdout carries only the JSON.
static JSON: AtomicBool = AtomicBool::new(false);
macro_rules! say {
    ($($t:tt)*) => {
        if JSON.load(Ordering::Relaxed) { eprintln!($($t)*) } else { println!($($t)*) }
    };
}

type Row = (String, String, f64, f64, f64, usize, bool);

/// A distance for the JSON: `null` when nothing was found (`f64::MAX`).
fn ft(d: f64) -> Value {
    if d < f64::MAX { json!(d.round()) } else { Value::Null }
}

fn row_json(r: &Row) -> Value {
    json!({ "name": r.0, "kind": r.1, "water_ft": ft(r.2), "river_ft": ft(r.3), "mouth_ft": ft(r.4), "roads": r.5, "coastal": r.6 })
}

fn main() {
    let args = common::args();
    JSON.store(args.json, Ordering::Relaxed);
    let arg = args.get(0).unwrap_or("1");
    let world = common::world(arg);
    let t0 = T0::generate(&world);
    let cell = t0.cell_ft;
    let names = t0.extra.as_ref().map(|e| e.overlay.features.as_slice()).unwrap_or(&[]);
    // Distance (ft) from a point to the nearest standing water (sea/lake), by scanning cells.
    // Distance (ft) from a point to the real shoreline (sea or lake), by ray marching.
    let water_d = |x: f64, y: f64| {
        let mut best = f64::MAX;
        for a in 0..64 {
            let ang = std::f64::consts::TAU * a as f64 / 64.0;
            let mut t = 0.0;
            while t < 20_000.0 {
                if t0.sample_water(x + libm::cos(ang) * t, y + libm::sin(ang) * t) > -29_000.0 {
                    best = best.min(t);
                    break;
                }
                t += 25.0;
            }
        }
        best
    };
    // Nearest river curve point and nearest river mouth (last control point).
    let river_d = |x: f64, y: f64| {
        let mut best = f64::MAX;
        for (ri, k) in t0.rivers.segments_near(x - 20_000.0, y - 20_000.0, x + 20_000.0, y + 20_000.0, 0.0) {
            let r = &t0.rivers.rivers[ri as usize];
            for j in 0..=32 {
                let p = r.eval(k as usize, j as f64 / 32.0, 40.0, cell).p;
                best = best.min(((p[0] - x).powi(2) + (p[1] - y).powi(2)).sqrt());
            }
        }
        best
    };
    let mouths: Vec<[f64; 2]> = t0.rivers.rivers.iter().filter(|r| r.q.last().is_some_and(|q| *q > 2_000_000.0)).map(|r| *r.pts.last().unwrap()).collect();
    let mouth_d = |x: f64, y: f64| mouths.iter().map(|m| ((m[0] - x).powi(2) + (m[1] - y).powi(2)).sqrt()).fold(f64::MAX, f64::min);
    // Road junction degree near a point: distinct road curves within 1.5 cells.
    let roads_near = |x: f64, y: f64| {
        let mut set = std::collections::BTreeSet::new();
        for (ri, _) in t0.roads.segments_near([x - 1.5 * cell, y - 1.5 * cell, x + 1.5 * cell, y + 1.5 * cell], 0.0) {
            set.insert(ri);
        }
        set.len()
    };
    let mut rows: Vec<Row> = Vec::new();
    for s in &t0.settlements {
        let name = names.iter().find(|f| (f.x - s.x).abs() < 1.0 && (f.y - s.y).abs() < 1.0).map(|f| f.name.clone()).unwrap_or_default();
        rows.push((name, format!("{:?} {:?}", s.tier, s.kind), water_d(s.x, s.y), river_d(s.x, s.y), mouth_d(s.x, s.y), roads_near(s.x, s.y), s.coastal));
    }
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    say!("cell = {cell:.0} ft; {} mouths of big rivers", mouths.len());
    let cities: Vec<&Row> = rows.iter().filter(|r| r.1.contains("Metropolis") || r.1.contains("City")).collect();
    for r in &cities {
        say!("{:<14} {:<22} water {:>7.0} river {:>7.0} mouth {:>8.0} roads {} coastal {}", r.0, r.1, r.2, r.3, r.4, r.5, r.6);
    }
    let port_like: Vec<&Row> = rows.iter().filter(|r| r.1.contains("Port") || r.1.contains("Fishing")).collect();
    let far: Vec<&Row> = port_like.iter().copied().filter(|r| r.2 > 1_000.0 && r.3 > 500.0).collect();
    say!("port/fishing settlements: {}, of which {} are > 1000 ft from sea/lake and > 500 ft from a river", port_like.len(), far.len());
    for r in far.iter().take(8) {
        say!("  {:<14} {:<22} water {:>7.0} river {:>7.0} coastal {}", r.0, r.1, r.2, r.3, r.6);
    }
    let mut ws: Vec<f64> = port_like.iter().map(|r| r.2.min(r.3)).collect();
    ws.sort_by(|a, b| a.total_cmp(b));
    if !ws.is_empty() {
        say!("port/fishing distance to water: median {:.0} ft, max {:.0} ft", ws[ws.len() / 2], ws[ws.len() - 1]);
    }
    // Roads vs gates for the capital (the most populous settlement): where each road ends and the nearest gate.
    let capital = match (0..t0.settlements.len()).max_by_key(|&i| t0.settlements[i].population) {
        None => {
            say!("no settlements, so no capital");
            Value::Null
        }
        Some(si) => {
            let s = &t0.settlements[si];
            let l = worldgen::town::generate(&world, &t0, si);
            let reach = l.radius * 2.5;
            say!("capital radius {:.0} ft; gates {}; bridges {}; piers {}", l.radius, l.gates.len(), l.bridges.len(), l.piers.len());
            let mut roads = Vec::new();
            for (ri, k) in t0.roads.segments_near([s.x - reach, s.y - reach, s.x + reach, s.y + reach], 0.0) {
                let r = &t0.roads.roads[ri as usize];
                let n = r.pts.len();
                if k as usize != 0 && k as usize != n - 2 {
                    continue;
                }
                let end = if k == 0 { r.eval(0, 0.0, 5.0, cell).p } else { r.eval(n - 2, 1.0, 5.0, cell).p };
                let dc = ((end[0] - s.x).powi(2) + (end[1] - s.y).powi(2)).sqrt();
                if dc > reach {
                    continue;
                }
                let dg = l.gates.iter().map(|g| ((g[0] - end[0]).powi(2) + (g[1] - end[1]).powi(2)).sqrt()).fold(f64::MAX, f64::min);
                say!("  road {ri} ({:?}) ends {dc:.0} ft from centre, nearest gate {dg:.0} ft away", r.class);
                roads.push(json!({ "road": ri, "class": format!("{:?}", r.class), "end_ft": ft(dc), "gate_ft": ft(dg) }));
            }
            let first = |polys: &[Vec<[f64; 2]>], what: &str| {
                let c = worldgen::town::geom::centroid(polys.first()?);
                say!("first {what} at fx {:.6} fy {:.6}", c[0] / mw, c[1] / mh);
                Some([c[0] / mw, c[1] / mh])
            };
            let (pier, bridge) = (first(&l.piers, "pier"), first(&l.bridges, "bridge"));
            json!({ "index": si, "name": rows[si].0, "kind": rows[si].1, "radius_ft": l.radius.round(), "gates": l.gates.len(), "bridges": l.bridges.len(), "piers": l.piers.len(), "roads": roads, "first_pier": pier, "first_bridge": bridge })
        }
    };
    let mut fishing = Vec::new();
    for (i, st) in t0.settlements.iter().enumerate() {
        if fishing.len() >= 4 || !matches!(st.kind, worldgen::t0::settle::SettleKind::Fishing) || st.tier != worldgen::t0::settle::Tier::Village {
            continue;
        }
        let vl = worldgen::town::generate(&world, &t0, i);
        say!("fishing village #{i} at fx {:.6} fy {:.6}: {} houses, {} piers, coastal {}", st.x / mw, st.y / mh, vl.buildings.len(), vl.piers.len(), st.coastal);
        fishing.push(json!({ "index": i, "fx": st.x / mw, "fy": st.y / mh, "houses": vl.buildings.len(), "piers": vl.piers.len(), "coastal": st.coastal }));
    }
    let with_piers = t0.settlements.iter().enumerate().filter(|(_, st)| st.tier == worldgen::t0::settle::Tier::Village && (st.coastal || st.river)).filter(|(i, _)| !worldgen::town::generate(&world, &t0, *i).piers.is_empty()).count();
    let waterside = t0.settlements.iter().filter(|st| st.tier == worldgen::t0::settle::Tier::Village && (st.coastal || st.river)).count();
    say!("waterside villages with piers: {with_piers}/{waterside}");
    let towns: Vec<&Row> = rows.iter().filter(|r| !r.1.contains("Village")).collect();
    let towns_jn = towns.iter().filter(|r| r.5 >= 3).count();
    say!("towns and up at a junction of 3+ roads: {towns_jn}/{}", towns.len());
    let jn = rows.iter().filter(|r| r.5 >= 3).count();
    say!("settlements at a junction of 3+ roads: {}/{}", jn, rows.len());
    if args.json {
        let out = json!({
            "world": common::label(arg),
            "cell_ft": cell,
            "settlements": rows.len(),
            "big_river_mouths": mouths.len(),
            "cities": cities.iter().map(|r| row_json(r)).collect::<Vec<_>>(),
            "port_fishing": { "count": port_like.len(), "far_from_water": far.len(), "far": far.iter().take(8).map(|r| row_json(r)).collect::<Vec<_>>(), "median_water_ft": ws.get(ws.len() / 2).copied().map_or(Value::Null, ft), "max_water_ft": ws.last().copied().map_or(Value::Null, ft) },
            "capital": capital,
            "fishing_villages": fishing,
            "waterside_villages": { "with_piers": with_piers, "total": waterside },
            "junctions_3plus": { "towns": towns_jn, "towns_total": towns.len(), "settlements": jn, "settlements_total": rows.len() },
        });
        println!("{out}");
    }
}
