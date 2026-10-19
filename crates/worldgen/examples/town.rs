//! Dev tool: generate settlement layouts and write SVG previews plus stats.
//! `cargo run --release -p worldgen --example town -- <seed> <out-prefix> [settlement index...]`
//! Without indices: the metropolis, the largest city, a town and a village.
use std::fmt::Write as _;
use std::time::Instant;

use worldgen::t0::T0;
use worldgen::t0::settle::Tier;
use worldgen::town::{self, catalog::CATALOG};
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let prefix = a.get(2).cloned().unwrap_or_else(|| "town".into());
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let t0 = T0::generate(&world);
    let names = &t0.extra.as_ref().unwrap().overlay.features;
    let mut picks: Vec<usize> = a.iter().skip(3).filter_map(|s| s.parse().ok()).collect();
    if picks.is_empty() {
        let by = |t: Tier| t0.settlements.iter().enumerate().filter(|(_, s)| s.tier == t).max_by_key(|(_, s)| s.population).map(|(i, _)| i);
        picks = [Tier::Metropolis, Tier::City, Tier::Town, Tier::Village].into_iter().filter_map(by).collect();
    }
    // One site of each kind, and a village graveyard.
    let n = t0.settlements.len();
    for kind in [worldgen::t0::settle::PoiKind::Ruin, worldgen::t0::settle::PoiKind::Tower, worldgen::t0::settle::PoiKind::Waystation] {
        if let Some((pi, p)) = t0.pois.iter().enumerate().find(|(_, p)| p.kind == kind) {
            let l = town::layout(&world, &t0, n + pi);
            println!("site {kind:?} #{} at fx {:.6} fy {:.6}: {} structures", n + pi, p.x / world.geom.map_w_ft, p.y / world.geom.map_h_ft, l.buildings.len());
        }
    }
    if let Some((l, b)) = (0..n).map(|i| town::layout(&world, &t0, i)).find_map(|l| {
        let b = l.buildings.iter().find(|b| b.structure == town::Structure::Open)?.poly.clone();
        Some((l, b))
    }) {
        let c = town::geom::centroid(&b);
        println!("graveyard in #{} at fx {:.6} fy {:.6}", l.index, c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
    }
    // Open spaces of every settlement by kind, and where the first few of each are; squares where
    // streets meet that reach past their bound (90 ft from their middle) or under a building, and
    // closes with a building on them besides the temple and its churchyard.
    let mut kinds: std::collections::BTreeMap<&str, Vec<(usize, [f64; 2])>> = Default::default();
    let (mut wide, mut under) = (0, 0);
    for i in 0..n {
        let l = town::layout(&world, &t0, i);
        for p in &l.plazas {
            let c = town::geom::centroid(&p.pts);
            kinds.entry(p.kind.name()).or_default().push((i, c));
            if l.tier >= Tier::Town && (town::squares::CORNER_KINDS.contains(&p.kind) || p.kind == town::PlazaKind::TempleClose) {
                let corner = p.kind != town::PlazaKind::TempleClose;
                if corner && p.pts.iter().any(|q| town::geom::dist(*q, c) > 90.0) {
                    wide += 1;
                    println!("  wide {} in #{i} at fx {:.6} fy {:.6}", p.kind.name(), c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
                }
                let on = |b: &town::Building| !corner && town::geom::contains(&p.pts, town::geom::centroid(&b.poly));
                // (Half a foot in from its walls: one sharing a patch edge with the square touches it.)
                if l.buildings.iter().any(|b| !on(b) && overlap(&town::geom::inset(&b.poly, &vec![0.5; b.poly.len()]), &p.pts)) {
                    under += 1;
                    println!("  {} over a building in #{i} at fx {:.6} fy {:.6}", p.kind.name(), c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
                }
            }
        }
    }
    println!("squares too wide: {wide}; over buildings: {under}");
    // Parks by tier (settlements, parks, formal, common) and the biggest of each kind.
    let mut parks: std::collections::BTreeMap<String, [usize; 4]> = Default::default();
    let mut biggest: [Option<(f64, usize, [f64; 2])>; 2] = [None, None];
    let mut planned = 0;
    for i in 0..n {
        let l = town::layout(&world, &t0, i);
        let row = parks.entry(format!("{:?}", l.tier)).or_default();
        row[0] += 1;
        // (Patches planned as parks, those on slopes too steep to build on among them.)
        if l.tier >= Tier::Town {
            planned += town::wards::plan_json(&world, &t0, i).map_or(0, |v| v["patches"].as_array().map_or(0, |ps| ps.iter().filter(|p| p.get("ward_generated").unwrap_or(&p["ward"]) == "park").count()));
        }
        for p in &l.parks {
            let formal = p.kind == town::ParkKind::Formal;
            row[1] += 1;
            row[if formal { 2 } else { 3 }] += 1;
            let a = town::geom::area(&p.pts).abs();
            let slot = &mut biggest[(!formal) as usize];
            if slot.is_none_or(|b| a > b.0) {
                *slot = Some((a, i, town::geom::centroid(&p.pts)));
            }
        }
    }
    for (tier, [s, p, f, c]) in &parks {
        println!("parks in {tier}: {s} settlements, {p} parks ({f} formal, {c} common)");
    }
    println!("patches planned as parks: {planned}");
    for (kind, b) in ["formal", "common"].iter().zip(&biggest) {
        if let Some((a, i, c)) = b {
            println!("biggest {kind} park: #{i} {a:.0} sq ft at fx {:.6} fy {:.6}", c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
        }
    }
    // Castles: their curtains (an inner one in a big castle), square towers, and their keeps by the
    // pieces they are made of (B body, o round tower, # square tower, F forebuilding, W wing).
    let (mut castles, mut inner, mut square, mut mottes) = (0, 0, 0, 0);
    let mut keeps: std::collections::BTreeMap<String, usize> = Default::default();
    for i in 0..n {
        let l = town::layout(&world, &t0, i);
        if l.castles.is_empty() || l.buildings.iter().all(|b| b.ward != town::catalog::Ward::Castle) {
            continue;
        }
        castles += 1;
        inner += (l.castles.len() > 1) as usize;
        square += (!l.square_towers.is_empty()) as usize;
        mottes += l.mottes.len();
        let keep = l.buildings.iter().find(|b| !b.parts.is_empty());
        let sig: String = keep.map_or("none".into(), |k| {
            k.parts
                .iter()
                .map(|p| match p.kind {
                    town::PartKind::Body => 'B',
                    town::PartKind::Tower { round: true } => 'o',
                    town::PartKind::Tower { round: false } => '#',
                    town::PartKind::Forebuilding => 'F',
                    town::PartKind::Wing => 'W',
                })
                .collect()
        });
        *keeps.entry(sig).or_default() += 1;
        if let Some(k) = keep {
            let c = town::geom::centroid(&k.poly);
            println!("  castle #{i}: keep b:{i}:{} at fx {:.6} fy {:.6}", k.id, c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
        }
    }
    println!("castles: {castles} ({inner} with an inner curtain, {square} with square towers, {mottes} mottes); keeps: {keeps:?}");
    for (kind, at) in &kinds {
        let first: Vec<String> = at.iter().take(3).map(|(i, c)| format!("#{i} fx {:.6} fy {:.6}", c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft)).collect();
        println!("{kind}: {} ({})", at.len(), first.join("; "));
    }
    for i in picks {
        let s = &t0.settlements[i];
        let start = Instant::now();
        let l = town::generate(&world, &t0, i);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let name = names.iter().find(|f| (f.x - s.x).abs() < 1.0 && (f.y - s.y).abs() < 1.0).map(|f| f.name.clone()).unwrap_or_default();
        let funcs = l.buildings.iter().filter(|b| b.func.is_some()).count();
        let distinct: std::collections::BTreeSet<u16> = l.buildings.iter().filter_map(|b| b.func).collect();
        println!(
            "#{i} {name} ({:?}, pop {}) at fx {:.5} fy {:.5}: {} buildings ({} with a function, {} of {} kinds), {} gates, {} walls, {} fields — {ms:.1} ms",
            s.tier,
            s.population,
            s.x / world.geom.map_w_ft,
            s.y / world.geom.map_h_ft,
            l.buildings.len(),
            funcs,
            distinct.len(),
            CATALOG.len(),
            l.gates.len(),
            l.walls.len(),
            l.fields.len()
        );
        for g in l.gate_towers.iter().take(1).chain(l.monuments.iter().map(|m| &m[0]).take(1)) {
            println!("    at fx {:.6} fy {:.6}", g[0] / world.geom.map_w_ft, g[1] / world.geom.map_h_ft);
        }
        for (what, d) in l.bridges.iter().take(2).map(|d| ("bridge", d)).chain(l.piers.iter().take(1).map(|d| ("pier", d))) {
            let c = town::geom::centroid(d);
            println!("    {what} at fx {:.6} fy {:.6}", c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
        }
        // Its open spaces (quays aside), the biggest first.
        let mut plazas: Vec<&town::Plaza> = l.plazas.iter().filter(|p| p.kind != town::PlazaKind::Quay).collect();
        plazas.sort_by(|a, b| town::geom::area(&b.pts).abs().total_cmp(&town::geom::area(&a.pts).abs()));
        for p in plazas.iter().take(6) {
            let c = town::geom::centroid(&p.pts);
            println!("    plaza {:?} ({:.0} sq ft) at fx {:.6} fy {:.6}", p.kind, town::geom::area(&p.pts).abs(), c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
        }
        for b in l.buildings.iter().filter(|b| b.name.is_some()).take(6) {
            println!("    {} — {}", b.name.as_ref().unwrap(), b.label());
        }
        RIVERS.with(|rv| {
            let mut rv = rv.borrow_mut();
            rv.clear();
            let reach = l.radius * 3.0;
            for (ri, k) in t0.rivers.segments_near(s.x - reach, s.y - reach, s.x + reach, s.y + reach, 0.0) {
                let rc = &t0.rivers.rivers[ri as usize];
                for j in 0..16 {
                    let a = rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft);
                    let b = rc.eval(k as usize, (j + 1) as f64 / 16.0, 5.0, t0.cell_ft);
                    rv.push((a.p, b.p, 0.5 * a.w));
                }
            }
        });
        std::fs::write(format!("{prefix}-{i}.svg"), svg(&l)).unwrap();
    }
}

fn svg(l: &town::Layout) -> String {
    let [x0, y0, x1, y1] = l.bbox;
    let pad = 100.0;
    let (w, h) = (x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad);
    let mut s = String::new();
    let _ = write!(s, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {w} {h}" width="1400" height="{}" style="background:#efe6cf">"#, x0 - pad, y0 - pad, (1400.0 * h / w) as u32);
    let path = |pts: &[[f64; 2]], close: bool| {
        let mut d = String::new();
        for (k, p) in pts.iter().enumerate() {
            let _ = write!(d, "{}{:.1},{:.1} ", if k == 0 { "M" } else { "L" }, p[0], p[1]);
        }
        if close {
            d.push('Z');
        }
        d
    };
    for (a, bb, hw) in river_lines(&l) {
        let _ = write!(s, r##"<path d="M{:.1},{:.1} L{:.1},{:.1}" stroke="#9fb8c4" stroke-width="{:.1}" stroke-linecap="round"/>"##, a[0], a[1], bb[0], bb[1], 2.0 * hw);
    }
    for f in &l.fields {
        let _ = write!(s, r##"<path d="{}" fill="#d9d2a6" stroke="#b8ad7c" stroke-width="2"/>"##, path(f, true));
    }
    for p in &l.plazas {
        let fill = if p.kind.is_lawn() { "#b9cf98" } else { "#e2d9c0" };
        let _ = write!(s, r##"<path d="{}" fill="{fill}" stroke="none"/>"##, path(&p.pts, true));
    }
    for st in &l.streets {
        let _ = write!(s, r##"<path d="{}" fill="none" stroke="#c9b894" stroke-width="10" stroke-linecap="round"/>"##, path(st, false));
    }
    for b in &l.buildings {
        let fill = if b.func.is_some() { "#8a6f5a" } else { "#a8998a" };
        let _ = write!(s, r##"<path d="{}" fill="{fill}" stroke="#2e2620" stroke-width="1.5"/>"##, path(&b.poly, true));
    }
    for d in l.bridges.iter().chain(&l.piers) {
        let _ = write!(s, r##"<path d="{}" fill="#8a6440" stroke="#2e2620" stroke-width="2"/>"##, path(d, true));
    }
    for wl in &l.walls {
        let _ = write!(s, r##"<path d="{}" fill="none" stroke="#2e2620" stroke-width="10"/>"##, path(wl, false));
    }
    for t in &l.towers {
        let _ = write!(s, r##"<circle cx="{:.1}" cy="{:.1}" r="12" fill="#2e2620"/>"##, t[0], t[1]);
    }
    for m in &l.monuments {
        let _ = write!(s, r##"<path d="{}" fill="#b8b0a0" stroke="#2e2620" stroke-width="2"/>"##, path(m, true));
    }
    for t in &l.gate_towers {
        let _ = write!(s, r##"<circle cx="{:.1}" cy="{:.1}" r="16" fill="#2e2620"/>"##, t[0], t[1]);
    }
    for g in &l.gates {
        let _ = write!(s, r##"<circle cx="{:.1}" cy="{:.1}" r="16" fill="#c0392b"/>"##, g[0], g[1]);
    }
    s.push_str("</svg>");
    s
}

thread_local! {
    static RIVERS: std::cell::RefCell<Vec<([f64; 2], [f64; 2], f64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn river_lines(_l: &town::Layout) -> Vec<([f64; 2], [f64; 2], f64)> {
    RIVERS.with(|r| r.borrow().clone())
}

/// Two polygons overlap: a corner of one inside the other, or edges crossing.
fn overlap(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    use town::geom::{contains, cross, sub};
    let crosses = |p: [f64; 2], q: [f64; 2], r: [f64; 2], s: [f64; 2]| {
        let (d1, d2) = (cross(sub(q, p), sub(r, p)), cross(sub(q, p), sub(s, p)));
        let (d3, d4) = (cross(sub(s, r), sub(p, r)), cross(sub(s, r), sub(q, r)));
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    };
    a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p)) || (0..a.len()).any(|i| (0..b.len()).any(|j| crosses(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()])))
}
