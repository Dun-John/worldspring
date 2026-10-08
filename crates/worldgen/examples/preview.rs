//! Dev tool: generate T0 for a world and write a PNG preview plus stats.
//! `cargo run --release -p worldgen --example preview -- <seed|world.json> <out.png> [params-json] [sketch.json] [--json]`
//! With a sketch, also writes the quick sketch preview (`<out>-quick.png`) and lists conflicts.
//! Water is drawn by kind: sea, fresh lakes, salt lakes, and standing water along a drawn river
//! (a river the world maps as a lake). Writes `<out>.stats.json` beside the picture: conflicts, pins,
//! per drawn river how much of it is mapped as river, lake, sea or dry land, the rivers
//! (polylines with discharge) and the lakes (with area). `--json` prints the same on stdout.

mod common;

use std::time::Instant;

use serde_json::json;
use worldgen::World;
use worldgen::t0::T0;
use worldgen::t0::biome::{ALL, Biome};
use worldgen::t0::hydro::{LakeKind, NO_LAKE};
use worldgen::world::SketchTool;

fn main() {
    let args = common::args();
    let src = args.get(0).unwrap_or("1").to_string();
    let out = args.get(1).unwrap_or("preview.png").to_string();
    let mut file = common::world_file(&src);
    if let Some(p) = args.get(2) {
        if !p.is_empty() && p != "{}" {
            // Fields given override the world's (or the defaults, for a seed).
            let mut params = serde_json::to_value(&file.params).unwrap();
            let over: serde_json::Value = serde_json::from_str(p).expect("params json");
            for (k, v) in over.as_object().expect("params json is an object") {
                params[k] = v.clone();
            }
            file.params = serde_json::from_value(params).expect("params json");
        }
    }
    if let Some(path) = args.get(3) {
        file.sketch = serde_json::from_str(&std::fs::read_to_string(path).expect("sketch file")).expect("sketch json");
    }
    let world = World::new(file).expect("valid world");
    if !world.file.sketch.is_empty() {
        let t = Instant::now();
        let q = T0::preview(&world, 256);
        eprintln!("quick preview {}x{} in {:.0} ms", q.w, q.h, t.elapsed().as_secs_f64() * 1000.0);
        let quick = out.replace(".png", "-quick.png");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&quick).unwrap()), q.w as u32, q.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&q.rgba).unwrap();
        for c in &q.conflicts {
            eprintln!("  preview conflict (stroke {}): {} at ({:.0}, {:.0})", c.stroke, c.message, c.x, c.y);
        }
    }

    let start = Instant::now();
    let mut last = Instant::now();
    let mut stage = String::new();
    let t0 = T0::generate_with_progress(&world, &mut |s, _| {
        if s != stage {
            if !stage.is_empty() {
                eprintln!("  {stage:<10} {:>6.0} ms", last.elapsed().as_secs_f64() * 1000.0);
            }
            stage = s.to_string();
            last = Instant::now();
        }
    });
    eprintln!("T0 total {:.2} s", start.elapsed().as_secs_f64());

    let (w, h) = (t0.height.w, t0.height.h);
    let extra = t0.extra.as_ref().unwrap();
    for c in &extra.overlay.conflicts {
        eprintln!("conflict (stroke {}): {} at ({:.0}, {:.0})", c.stroke, c.message, c.x, c.y);
    }
    let towns: Vec<_> = extra.overlay.features.iter().filter(|f| ["metropolis", "city", "town", "village"].contains(&f.kind)).collect();
    let mut pins = Vec::new();
    for (s, f) in t0.settlements.iter().zip(&towns) {
        if let Some(pin) = s.pin {
            eprintln!("pinned (stroke {pin}): {} {} at ({:.0}, {:.0}), {}{}", f.kind, f.name, f.x, f.y, f.detail.as_deref().unwrap_or(""), if s.coastal { ", coastal" } else { "" });
            pins.push(json!({ "stroke": pin, "name": f.name, "tier": f.kind, "kind": s.kind.name(), "coastal": s.coastal, "x": f.x, "y": f.y }));
        }
    }
    let pinned_coastal = t0.settlements.iter().filter(|s| s.pin.is_some() && s.coastal).count();
    eprintln!("pins: {} placed, {pinned_coastal} coastal", pins.len());
    let sea = world.params().sea_level_ft;
    let mut counts = [0usize; ALL.len()];
    for &b in &t0.biome.data {
        counts[(b & 0xff) as usize] += 1;
    }
    let land = t0.height.data.iter().filter(|&&v| v as f64 > sea).count();
    // Below sea level and dry: land (a fault: a river or plain dug under the sea), or a salt
    // flat (a dry basin, as Death Valley).
    let below = |k: usize| t0.water.data[k] <= -29_000.0 && (t0.height.data[k] as f64) < sea;
    let low_land = (0..w * h).filter(|&k| below(k) && extra.hydro.lake_of[k] == NO_LAKE).count();
    let low_flats = (0..w * h).filter(|&k| below(k) && extra.hydro.lake_of[k] != NO_LAKE).count();
    eprintln!("dry cells below sea level: {low_land} (and {low_flats} of salt flats)");
    for k in (0..w * h).filter(|&k| below(k) && extra.hydro.lake_of[k] == NO_LAKE).take(6) {
        let lake = extra.hydro.lake_of[k];
        let what = if lake == NO_LAKE { "dry land".to_string() } else { format!("{:?} lake", extra.hydro.lakes[lake as usize].kind) };
        eprintln!("  at ({:.0}, {:.0}): {:.0} ft, {what}", (k % w) as f64 * t0.cell_ft, (k / w) as f64 * t0.cell_ft, t0.height.data[k]);
    }
    eprintln!("land {:.1}%  max elev {:.0} ft", 100.0 * land as f64 / (w * h) as f64, t0.height.data.iter().fold(f32::MIN, |a, &b| a.max(b)));
    let named = |k: &str| extra.overlay.features.iter().filter(|f| f.kind == k).count();
    eprintln!(
        "rivers {} (named {}, falls {})  lakes {}  ranges {}  peaks {}  passes {}  volcanoes {}  islands {}  bays {}",
        t0.rivers.rivers.len(),
        named("river"),
        named("waterfall"),
        extra.hydro.lakes.len(),
        named("range"),
        named("peak"),
        named("pass"),
        named("volcano"),
        named("island"),
        named("bay")
    );
    let mut bs: Vec<String> = ALL
        .iter()
        .filter(|b| counts[**b as usize] > 0 && !matches!(b, Biome::Ocean))
        .map(|b| format!("{} {:.1}%", b.name(), 100.0 * counts[*b as usize] as f64 / land.max(1) as f64))
        .collect();
    bs.sort();
    eprintln!("biomes: {}", bs.join(", "));
    for f in extra.overlay.features.iter().filter(|f| matches!(f.kind, "continent" | "range" | "volcano" | "ocean")).take(12) {
        eprintln!("  {:<10} {}{}", f.kind, f.name, f.detail.as_ref().map(|d| format!(" ({d})")).unwrap_or_default());
    }

    // Settlements and roads.
    let tiers = ["village", "town", "city", "metropolis"].map(named);
    eprintln!("settlements: {} villages, {} towns, {} cities, {} metropolises; ruins {}, towers {}, waystations {}", tiers[0], tiers[1], tiers[2], tiers[3], named("ruin"), named("tower"), named("waystation"));
    let mut len = [0.0f64; 3];
    let mut worst = [0.0f64; 3];
    let mut pts = 0;
    for r in &t0.roads.roads {
        pts += r.pts.len();
        for k in 1..r.pts.len() {
            let d = ((r.pts[k][0] - r.pts[k - 1][0]).powi(2) + (r.pts[k][1] - r.pts[k - 1][1]).powi(2)).sqrt();
            len[r.class as usize] += d / 5280.0;
            if d > 1.0 {
                worst[r.class as usize] = worst[r.class as usize].max(((r.z[k] - r.z[k - 1]) as f64).abs() / d);
            }
        }
    }
    let mut cross = [0usize; 3];
    for c in &extra.crossings {
        cross[c.kind as usize] += 1;
    }
    eprintln!(
        "roads {} ({} pts): king's {:.0} mi, road {:.0} mi, track {:.0} mi; max grade {:.3}/{:.3}/{:.3}; bridges {} fords {} ferries {}",
        t0.roads.roads.len(), pts, len[0], len[1], len[2], worst[0], worst[1], worst[2], cross[0], cross[1], cross[2]
    );
    // Where a king's road actually crosses the widest river (road curve over a channel).
    {
        let cell = t0.cell_ft;
        let mut best: Option<(f64, [f64; 2])> = None;
        for r in t0.roads.roads.iter().filter(|r| r.class as u8 == 0) {
            for k in 0..r.pts.len() - 1 {
                for s in 0..8 {
                    let p = r.eval(k, s as f64 / 8.0, 2.5, cell).p;
                    for (vi, vk) in t0.rivers.segments_near(p[0] - 300.0, p[1] - 300.0, p[0] + 300.0, p[1] + 300.0, 0.0) {
                        for u in 0..=16 {
                            let cp = t0.rivers.rivers[vi as usize].eval(vk as usize, u as f64 / 16.0, 2.5, cell);
                            let d = ((p[0] - cp.p[0]).powi(2) + (p[1] - cp.p[1]).powi(2)).sqrt();
                            if d < 0.4 * cp.w && best.is_none_or(|b| cp.w > b.0) {
                                best = Some((cp.w, p));
                            }
                        }
                    }
                }
            }
        }
        if let Some((w, p)) = best {
            eprintln!("  king's road bridge over {w:.0} ft river at fx {:.6} fy {:.6}", p[0] / world.geom.map_w_ft, p[1] / world.geom.map_h_ft);
        }
    }
    // A switchback stretch (points with no wander), the longest one.
    if let Some((r, k0, len)) = t0
        .roads
        .roads
        .iter()
        .flat_map(|r| {
            let mut runs = Vec::new();
            let mut k = 0;
            while k < r.pts.len() {
                if r.wander[k] == 0.0 {
                    let s = k;
                    while k < r.pts.len() && r.wander[k] == 0.0 {
                        k += 1;
                    }
                    runs.push((r, s, k - s));
                }
                k += 1;
            }
            runs
        })
        .max_by_key(|x| x.2)
    {
        let p = r.pts[k0 + len / 2];
        eprintln!("  switchbacks: {len} legs on a {:?} at fx {:.6} fy {:.6}", r.class, p[0] / world.geom.map_w_ft, p[1] / world.geom.map_h_ft);
    }
    for f in extra.overlay.features.iter().filter(|f| matches!(f.kind, "metropolis" | "city")).take(8) {
        eprintln!("  {:<10} {} ({}) fx {:.4} fy {:.4}", f.kind, f.name, f.detail.as_deref().unwrap_or(""), f.x / world.geom.map_w_ft, f.y / world.geom.map_h_ft);
    }

    // One sample location (map fractions) per biome, for screenshots.
    for b in ALL {
        let mut hits: Vec<usize> = (0..w * h).filter(|&k| (t0.biome.data[k] & 0xff) as u8 == b as u8).collect();
        if hits.len() > 50 && !matches!(b, Biome::Ocean | Biome::Lake) {
            hits.sort_by_key(|&k| (k * 2654435761) % 1_000_003);
            let k = hits[0];
            eprintln!("  at {:<20} fx {:.4} fy {:.4}", b.name(), (k % w) as f64 / (w - 1) as f64, (k / w) as f64 / (h - 1) as f64);
        }
    }
    if let Some(r) = t0.rivers.rivers.iter().max_by(|a, b| a.q.last().unwrap().total_cmp(b.q.last().unwrap())) {
        let p = r.pts[r.pts.len() / 2];
        eprintln!("largest river mid: fx {:.4} fy {:.4}", p[0] / world.geom.map_w_ft, p[1] / world.geom.map_h_ft);
    }

    // Drawn rivers: how much of each the world maps as a river, a lake, the sea or dry land
    // (cells along the stroke; a river within a cell of it counts).
    let cell = t0.cell_ft;
    let hydro = &extra.hydro;
    let at_cell = |p: [f64; 2]| ((p[0] / cell).round().clamp(0.0, (w - 1) as f64) as usize, (p[1] / cell).round().clamp(0.0, (h - 1) as f64) as usize);
    let mut river_cell = vec![false; w * h];
    for r in &t0.rivers.rivers {
        for p in &r.pts {
            let (i, j) = at_cell(*p);
            river_cell[j * w + i] = true;
        }
    }
    let near = |m: &[bool], i: usize, j: usize| (j.saturating_sub(1)..=(j + 1).min(h - 1)).any(|y| (i.saturating_sub(1)..=(i + 1).min(w - 1)).any(|x| m[y * w + x]));
    let is_sea = |k: usize| hydro.lake_of[k] == NO_LAKE && t0.water.data[k] > -29_000.0;
    // Cells inside drawn lakes (by stroke + 1; 0 outside any): water there is meant.
    let mut in_lake = vec![0u32; w * h];
    for (si, s) in world.file.sketch.strokes.iter().enumerate().filter(|(_, s)| s.tool == SketchTool::Lake) {
        let (x0, x1) = s.pts.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
        let (y0, y1) = s.pts.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        let (i0, j0) = at_cell([x0, y0]);
        let (i1, j1) = at_cell([x1, y1]);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let (x, y) = (i as f64 * cell, j as f64 * cell);
                let mut inside = false;
                for e in 0..s.pts.len() {
                    let (a, b) = (s.pts[e], s.pts[(e + 1) % s.pts.len()]);
                    if (a[1] <= y) != (b[1] <= y) && x < a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                        inside = !inside;
                    }
                }
                if inside {
                    in_lake[j * w + i] = si as u32 + 1;
                }
            }
        }
    }
    let mut along = vec![false; w * h];
    let mut strokes = Vec::new();
    let mut total = [0usize; 4];
    for (si, s) in world.file.sketch.strokes.iter().enumerate().filter(|(_, s)| s.tool == SketchTool::River) {
        let mut cells: Vec<usize> = Vec::new();
        for seg in s.pts.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let steps = (((b[0] - a[0]).hypot(b[1] - a[1])) / (0.4 * cell)).ceil().max(1.0) as usize;
            for t in 0..=steps {
                let f = t as f64 / steps as f64;
                let (i, j) = at_cell([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]);
                if cells.last() != Some(&(j * w + i)) {
                    cells.push(j * w + i);
                }
            }
        }
        // River, lake, sea, dry land.
        let mut c = [0usize; 4];
        let mut first_lake: Option<usize> = None;
        for &k in cells.iter().filter(|&&k| in_lake[k] == 0) {
            let (i, j) = (k % w, k / w);
            for y in j.saturating_sub(1)..=(j + 1).min(h - 1) {
                for x in i.saturating_sub(1)..=(i + 1).min(w - 1) {
                    along[y * w + x] = true;
                }
            }
            let class = if near(&river_cell, i, j) {
                0
            } else if hydro.lake_of[k] != NO_LAKE {
                first_lake.get_or_insert(k);
                1
            } else if is_sea(k) {
                2
            } else {
                3
            };
            c[class] += 1;
        }
        let mapped = c[0] + c[1] + c[3];
        let share = if mapped > 0 { c[0] as f64 / mapped as f64 } else { 0.0 };
        (0..4).for_each(|i| total[i] += c[i]);
        let len_mi = s.pts.windows(2).map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1])).sum::<f64>() / 5280.0;
        let lake_at = first_lake.map(|k| [(k % w) as f64 * cell, (k / w) as f64 * cell]);
        strokes.push(json!({ "stroke": si, "len_mi": len_mi.round(), "cells": cells.len(), "river": c[0], "lake": c[1], "sea": c[2], "dry": c[3], "river_share": share, "first_lake": lake_at }));
    }
    let cell_mi2 = (cell / 5280.0) * (cell / 5280.0);
    let mapped = total[0] + total[1] + total[3];
    let share = if mapped > 0 { total[0] as f64 / mapped as f64 } else { 0.0 };
    let lake_along = (0..w * h).filter(|&k| along[k] && hydro.lake_of[k] != NO_LAKE).count();
    let standing = (0..w * h).filter(|&k| hydro.lake_of[k] != NO_LAKE).count();
    let standing_undrawn = (0..w * h).filter(|&k| hydro.lake_of[k] != NO_LAKE && hydro.lakes[hydro.lake_of[k] as usize].stroke.is_none()).count();
    if !strokes.is_empty() {
        let mut worst: Vec<&serde_json::Value> = strokes.iter().filter(|s| s["lake"].as_u64().unwrap_or(0) + s["dry"].as_u64().unwrap_or(0) > 4).collect();
        worst.sort_by(|a, b| a["river_share"].as_f64().unwrap().total_cmp(&b["river_share"].as_f64().unwrap()));
        for s in worst.iter().take(8) {
            eprintln!(
                "  drawn river (stroke {}, {} mi): river {} lake {} sea {} dry {}{}",
                s["stroke"],
                s["len_mi"],
                s["river"],
                s["lake"],
                s["sea"],
                s["dry"],
                s["first_lake"].as_array().map(|p| format!(", first lake at ({:.0}, {:.0})", p[0].as_f64().unwrap(), p[1].as_f64().unwrap())).unwrap_or_default()
            );
        }
        eprintln!(
            "drawn rivers {}: river share {:.1}% (river {} lake {} sea {} dry {} cells); standing water along them {:.0} sq mi",
            strokes.len(),
            100.0 * share,
            total[0],
            total[1],
            total[2],
            total[3],
            lake_along as f64 * cell_mi2
        );
    }
    eprintln!("standing water (lakes, salt flats) {:.0} sq mi, {:.0} of it not drawn", standing as f64 * cell_mi2, standing_undrawn as f64 * cell_mi2);
    // Drawn lakes: how much of each outline is its lake, and how much of the lake lies outside.
    let mut drawn_lakes = Vec::new();
    for (si, s) in world.file.sketch.strokes.iter().enumerate().filter(|(_, s)| s.tool == SketchTool::Lake) {
        let inside: Vec<usize> = (0..w * h).filter(|&k| in_lake[k] == si as u32 + 1).collect();
        // (A drawn lake trimmed off the sea or another lake can be in parts.)
        let parts: Vec<&worldgen::t0::hydro::Lake> = hydro.lakes.iter().filter(|l| l.stroke == Some(si as u32)).collect();
        let wet = inside.iter().filter(|&&k| hydro.lake_of[k] != NO_LAKE && hydro.lakes[hydro.lake_of[k] as usize].stroke == Some(si as u32)).count();
        let outside = parts.iter().map(|l| l.cells.len()).sum::<usize>().saturating_sub(wet);
        let (level, kind) = parts.first().map_or((None, None), |l| (Some(l.level_ft), Some(format!("{:?}", l.kind))));
        eprintln!(
            "  drawn lake (stroke {si}{}): {wet}/{} cells inside are its water, {outside} outside; level {}{}",
            s.name.as_deref().map(|n| format!(", {n}")).unwrap_or_default(),
            inside.len(),
            level.map_or("-".into(), |l| format!("{l:.0} ft")),
            kind.as_ref().map(|k| format!(", {k}")).unwrap_or_else(|| ", NOT MAPPED".into())
        );
        drawn_lakes.push(json!({ "stroke": si, "name": s.name, "inside": inside.len(), "wet": wet, "outside": outside, "level_ft": level, "kind": kind }));
    }
    let lakes: Vec<serde_json::Value> = hydro
        .lakes
        .iter()
        .map(|l| {
            let n = l.cells.len() as f64;
            let (sx, sy) = l.cells.iter().fold((0.0, 0.0), |(x, y), &c| (x + (c as usize % w) as f64, y + (c as usize / w) as f64));
            let kind = match l.kind {
                LakeKind::Fresh => "fresh",
                LakeKind::Salt => "salt",
                LakeKind::SaltFlat => "salt_flat",
            };
            let on_river = l.cells.iter().filter(|&&c| along[c as usize]).count();
            json!({ "stroke": l.stroke, "level_ft": l.level_ft, "kind": kind, "cells": l.cells.len(), "area_sq_mi": n * cell_mi2, "along_drawn_river": on_river, "x": sx / n * cell, "y": sy / n * cell })
        })
        .collect();
    let rivers: Vec<serde_json::Value> = t0
        .rivers
        .rivers
        .iter()
        .map(|r| json!({ "pts": r.pts.iter().map(|p| [p[0].round(), p[1].round()]).collect::<Vec<_>>(), "q": r.q.iter().map(|q| q.round()).collect::<Vec<_>>() }))
        .collect();
    let report = json!({
        "world": common::label(&src),
        "conflicts": extra.overlay.conflicts.iter().map(|c| json!({ "stroke": c.stroke, "message": c.message, "x": c.x, "y": c.y })).collect::<Vec<_>>(),
        "pins": pins,
        "stats": {
            "dry_below_sea": low_land,
            "salt_flat_below_sea": low_flats,
            "land_pct": 100.0 * land as f64 / (w * h) as f64,
            "standing_water_sq_mi": standing as f64 * cell_mi2,
            "standing_water_not_drawn_sq_mi": standing_undrawn as f64 * cell_mi2,
            "drawn_river_share": share,
            "drawn_river_cells": { "river": total[0], "lake": total[1], "sea": total[2], "dry": total[3] },
            "standing_water_along_drawn_rivers_sq_mi": lake_along as f64 * cell_mi2,
            "pins_coastal": pinned_coastal,
        },
        "drawn_rivers": strokes,
        "drawn_lakes": drawn_lakes,
        "lakes": lakes,
        "rivers": rivers,
    });

    // Render: biome color × hillshade, water by kind (sea, fresh lake, salt lake, standing water
    // along a drawn river), rivers, feature dots.
    let mut img = vec![0u8; w * h * 3];
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            let hgt = t0.height.data[k] as f64;
            let water = t0.water.data[k] as f64;
            let at = |x: usize, y: usize| t0.height.data[y.min(h - 1) * w + x.min(w - 1)] as f64;
            let gx = (at(i + 1, j) - at(i.saturating_sub(1), j)) / (2.0 * cell);
            let gy = (at(i, j + 1) - at(i, j.saturating_sub(1))) / (2.0 * cell);
            let (nx, ny, nz) = (-gx * 12.0, -gy * 12.0, 1.0);
            let nl = (nx * nx + ny * ny + nz * nz).sqrt();
            let shade = ((nx * -0.6 + ny * -0.6 + nz * 0.53) / nl / 0.53).clamp(0.3, 1.4);
            let lake = hydro.lake_of[k];
            let c = if lake != NO_LAKE && along[k] && hgt < water {
                [150, 90, 215]
            } else if lake != NO_LAKE && hgt < water {
                if hydro.lakes[lake as usize].kind == LakeKind::Salt { [120, 175, 200] } else { [60, 135, 215] }
            } else if hgt < water {
                let d = ((water - hgt) / 8000.0).clamp(0.0, 1.0);
                [(150.0 - 60.0 * d) as u8, (185.0 - 60.0 * d) as u8, (200.0 - 40.0 * d) as u8]
            } else {
                let b = Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
                let base = color(b);
                [
                    (base[0] as f64 * shade).min(255.0) as u8,
                    (base[1] as f64 * shade).min(255.0) as u8,
                    (base[2] as f64 * shade).min(255.0) as u8,
                ]
            };
            img[k * 3..k * 3 + 3].copy_from_slice(&c);
        }
    }
    for r in &t0.rivers.rivers {
        let maxq = r.q.iter().fold(0f32, |a, &b| a.max(b));
        let thick = maxq > 2_000_000.0;
        for p in &r.pts {
            let (i, j) = at_cell(*p);
            for (di, dj) in if thick { vec![(0, 0), (1, 0), (0, 1)] } else { vec![(0, 0)] } {
                let (x, y) = ((i + di).min(w - 1), (j + dj).min(h - 1));
                img[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&[40, 80, 170]);
            }
        }
    }
    for r in &t0.roads.roads {
        let col = [[120, 30, 20], [150, 80, 40], [170, 130, 90]][r.class as usize];
        for k in 1..r.pts.len() {
            let (a, b) = (r.pts[k - 1], r.pts[k]);
            let n = ((((b[0] - a[0]).abs()).max((b[1] - a[1]).abs()) / cell * 2.0).ceil() as usize).max(1);
            for s in 0..=n {
                let t = s as f64 / n as f64;
                let (x, y) = (((a[0] + (b[0] - a[0]) * t) / cell) as usize, ((a[1] + (b[1] - a[1]) * t) / cell) as usize);
                let (x, y) = (x.min(w - 1), y.min(h - 1));
                img[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&col);
            }
        }
    }
    for f in &extra.overlay.features {
        let col = match f.kind {
            "metropolis" | "city" => [0, 0, 0],
            "town" => [60, 20, 60],
            "village" => [200, 200, 200],
            "ruin" | "tower" => [255, 0, 255],
            "peak" => [120, 20, 20],
            "volcano" => [255, 60, 0],
            "pass" => [255, 255, 255],
            _ => continue,
        };
        let (i, j) = ((f.x / cell) as usize, (f.y / cell) as usize);
        for d in [(0i64, 0i64), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (x, y) = ((i as i64 + d.0).clamp(0, w as i64 - 1) as usize, (j as i64 + d.1).clamp(0, h as i64 - 1) as usize);
            img[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&col);
        }
    }
    let file = std::fs::File::create(&out).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(&img).unwrap();
    eprintln!("wrote {out}");
    let text = serde_json::to_string(&report).unwrap();
    // (Its own name: never the world or sketch file it was given.)
    let side = format!("{}.stats.json", out.strip_suffix(".png").unwrap_or(&out));
    std::fs::write(&side, &text).unwrap();
    eprintln!("wrote {side}");
    if args.json {
        println!("{text}");
    }
}

fn color(b: Biome) -> [u8; 3] {
    match b {
        Biome::Ocean => [90, 120, 150],
        Biome::Lake => [110, 150, 190],
        Biome::Ice => [240, 245, 250],
        Biome::Tundra => [170, 175, 150],
        Biome::Alpine => [150, 140, 125],
        Biome::Taiga => [70, 105, 80],
        Biome::TemperateForest => [80, 130, 60],
        Biome::TemperateRainforest => [45, 110, 70],
        Biome::Grassland => [165, 185, 100],
        Biome::Steppe => [190, 185, 120],
        Biome::ColdDesert => [185, 170, 140],
        Biome::HotDesert => [225, 200, 140],
        Biome::Savanna => [195, 185, 95],
        Biome::Jungle => [30, 110, 40],
        Biome::Swamp => [90, 115, 80],
        Biome::Volcanic => [80, 60, 55],
        Biome::SaltFlat => [235, 230, 215],
    }
}
