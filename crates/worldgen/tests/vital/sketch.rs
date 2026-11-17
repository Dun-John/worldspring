//! The sketch guarantee: rivers and ports, relief and lakes, names and sites.

use super::*;

/// Sketch guarantee (M8): a drawn coastline, ranges, a river, a painted biome and settlement
/// pins make a coherent world that follows them. Land well inside the outline and sea well
/// outside it; high ground along the ranges; a mapped river along the drawn one, flowing
/// downhill; the desert painted; pinned settlements of the right tier and name, near their
/// pins and (towns and up) on the road network; a pin far out at sea reported, not placed.
#[test]
fn sketch_guarantee() {
    const MI: f64 = 5280.0;
    let mi = |x: f64, y: f64| serde_json::json!([(x * MI).round(), (y * MI).round()]);
    let c = (600.0, 450.0);
    let outline: Vec<(f64, f64)> = (0..48)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / 48.0;
            let r = 1.0 + 0.12 * (3.0 * a).sin() + 0.08 * (5.0 * a + 1.0).cos();
            (c.0 + 430.0 * r * a.cos(), c.1 + 300.0 * r * a.sin())
        })
        .collect();
    let range = [(330.0, 260.0), (450.0, 280.0), (560.0, 330.0), (700.0, 300.0)];
    let river = [(250.0, 330.0), (300.0, 420.0), (360.0, 520.0), (380.0, 640.0), (380.0, 800.0)];
    let desert = [(930.0, 330.0), (1000.0, 380.0), (990.0, 520.0), (900.0, 500.0), (880.0, 400.0)];
    let pins = [("metropolis", Some("Kingsreach"), (585.0, 690.0)), ("city", Some("Highmoor"), (450.0, 340.0)), ("town", None, (780.0, 420.0)), ("village", Some("Little Ford"), (300.0, 520.0))];
    let mut strokes = vec![
        serde_json::json!({ "tool": "land", "closed": true, "pts": outline.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
        serde_json::json!({ "tool": "range", "radius_ft": 18.0 * MI, "strength": 0.9, "pts": range.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
        serde_json::json!({ "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.8, "pts": river.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
        serde_json::json!({ "tool": "biome", "biome": "hot_desert", "closed": true, "pts": desert.iter().map(|p| mi(p.0, p.1)).collect::<Vec<_>>() }),
    ];
    for (tier, name, p) in pins {
        strokes.push(serde_json::json!({ "tool": "pin", "tier": tier, "name": name, "pts": [mi(p.0, p.1)] }));
    }
    strokes.push(serde_json::json!({ "tool": "pin", "tier": "town", "name": "Lost", "pts": [mi(60.0, 60.0)] }));
    let file: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 5, "sketch": { "strokes": strokes } })).unwrap();
    let world = World::new(file).unwrap();
    let start = std::time::Instant::now();
    let t0 = worldgen::t0::T0::generate(&world);
    println!("sketched T0 in {:.1} s", start.elapsed().as_secs_f64());
    let extra = t0.extra.as_ref().unwrap();
    let sea = world.params().sea_level_ft;
    let cell = t0.cell_ft;
    let idx = |x: f64, y: f64| ((y / cell).round() as usize).min(t0.height.h - 1) * t0.height.w + ((x / cell).round() as usize).min(t0.height.w - 1);
    let dry = |x: f64, y: f64| t0.height.data[idx(x * MI, y * MI)] as f64 > sea && t0.water.data[idx(x * MI, y * MI)] <= worldgen::t0::hydro::DRY;

    // Coastline: 60+ mi inside the outline is land (lakes aside), 60+ mi outside is sea.
    let (mut inside_ok, mut outside_ok) = (0, 0);
    for p in &outline {
        let (dx, dy) = (p.0 - c.0, p.1 - c.1);
        let l = (dx * dx + dy * dy).sqrt();
        let (ix, iy) = (p.0 - dx / l * 60.0, p.1 - dy / l * 60.0);
        inside_ok += usize::from(t0.height.data[idx(ix * MI, iy * MI)] as f64 > sea);
        outside_ok += usize::from(!dry(p.0 + dx / l * 60.0, p.1 + dy / l * 60.0));
    }
    assert!(inside_ok >= 46 && outside_ok >= 46, "coastline not followed: {inside_ok}/48 inside land, {outside_ok}/48 outside sea");

    // Range: high ground all along it.
    let mut high = 0;
    let mut samples = 0;
    for w in range.windows(2) {
        for t in 0..10 {
            let f = t as f64 / 10.0;
            let (x, y) = (w[0].0 + (w[1].0 - w[0].0) * f, w[0].1 + (w[1].1 - w[0].1) * f);
            let mut top = f64::MIN;
            for dy in -6..=6 {
                for dx in -6..=6 {
                    top = top.max(t0.height.data[idx((x + dx as f64) * MI, (y + dy as f64) * MI)] as f64 - sea);
                }
            }
            high += usize::from(top > 4_000.0);
            samples += 1;
        }
    }
    assert!(high * 10 >= samples * 8, "range too low along its line: {high}/{samples} samples above 4,000 ft");

    // River: a mapped river passes every drawn waypoint on land, and all rivers flow downhill.
    let hydro = &extra.hydro;
    let near_river = |x: f64, y: f64| {
        let k = idx(x * MI, y * MI);
        let (i, j) = ((k % t0.height.w) as i64, (k / t0.height.w) as i64);
        hydro.rivers.iter().any(|r| r.cells.iter().any(|&c| ((c as usize % t0.height.w) as i64 - i).abs() <= 2 && ((c as usize / t0.height.w) as i64 - j).abs() <= 2))
    };
    for p in river.iter().filter(|p| dry(p.0, p.1)) {
        assert!(near_river(p.0, p.1), "no mapped river near the drawn river at ({}, {}) mi", p.0, p.1);
    }
    for r in &hydro.rivers {
        for pair in r.cells.windows(2) {
            let (a, b) = (pair[0] as usize, pair[1] as usize);
            let into_water = t0.height.data[b] as f64 <= sea || hydro.lake_of[b] != worldgen::t0::hydro::NO_LAKE;
            assert!(into_water || t0.height.data[b] < t0.height.data[a], "a river climbs in the sketched world");
        }
    }

    // Biome paint: the desert's interior is desert.
    let mut desert_cells = 0;
    let mut tried = 0;
    for dy in -40..=40 {
        for dx in -30..=30 {
            let (x, y) = (940.0 + dx as f64, 430.0 + dy as f64);
            if dry(x, y) {
                tried += 1;
                desert_cells += usize::from(t0.biome.data[idx(x * MI, y * MI)] & 0xff == worldgen::t0::biome::Biome::HotDesert as u32);
            }
        }
    }
    assert!(tried > 100 && desert_cells * 10 >= tried * 9, "painted desert not desert: {desert_cells}/{tried}");

    // Pins: placed, named, near their pins, on the roads; the one at sea is reported.
    let towns: Vec<_> = extra.overlay.features.iter().filter(|f| ["metropolis", "city", "town", "village"].contains(&f.kind)).collect();
    assert_eq!(towns.len(), t0.settlements.len());
    for (k, (tier, name, p)) in pins.iter().enumerate() {
        let stroke = 4 + k as u32;
        let i = t0.settlements.iter().position(|s| s.pin == Some(stroke)).unwrap_or_else(|| panic!("pin {stroke} placed no settlement"));
        let (s, f) = (&t0.settlements[i], towns[i]);
        assert_eq!(f.kind, *tier, "pin {stroke} has the wrong tier");
        // (The economy ranks pins but neither re-tiers nor resizes them.)
        assert!(s.tier == s.planned && s.population == s.planned_pop, "the economy changed pin {stroke}");
        if let Some(n) = name {
            assert_eq!(f.name, *n, "pin {stroke} lost its name");
        }
        let d = ((s.x / MI - p.0).powi(2) + (s.y / MI - p.1).powi(2)).sqrt();
        assert!(d < 15.0, "pin {stroke} settled {d:.1} mi from where it was drawn");
        let road = t0.roads.roads.iter().any(|r| r.pts.iter().any(|q| ((q[0] - s.x).powi(2) + (q[1] - s.y).powi(2)).sqrt() < 3.0 * MI));
        // (Villages may be off the network, as generated ones are; towns and up never.)
        assert!(road || f.kind == "village", "pinned {} {} has no road", f.kind, f.name);
    }
    assert!(!t0.settlements.iter().any(|s| s.pin == Some(8)), "a settlement was placed far out at sea");
    assert!(extra.overlay.conflicts.iter().any(|c| c.stroke == 8), "the pin at sea was not reported");

    sketch_rivers_and_ports();
    sketch_relief_and_lakes();
    sketch_names_and_sites();
}

/// Sketch guarantee (R2): drawn rivers stay rivers and coasts keep their ports. On a low plain
/// with a hard coast, a long river drawn to stop a few miles short of the sea and a tributary
/// drawn into it (drawn first: the order strokes are drawn in doesn't matter) both flow on to
/// the sea, are mapped as rivers over at least 90% of their drawn courses with hardly any lake
/// along them, no dry land lies below sea level, and a town pinned on the coast over barren
/// ground is a port.
fn sketch_rivers_and_ports() {
    const MI: f64 = 5280.0;
    let mi = |p: &(f64, f64)| serde_json::json!([(p.0 * MI).round(), (p.1 * MI).round()]);
    let coast = [(30.0, 30.0), (390.0, 30.0), (390.0, 270.0), (30.0, 270.0)];
    let main = [(60.0, 140.0), (140.0, 160.0), (220.0, 140.0), (300.0, 160.0), (384.0, 150.0)];
    let tributary = [(150.0, 60.0), (190.0, 100.0), (220.0, 140.0)];
    let port = (389.6, 220.0);
    let barren = [(370.0, 205.0), (400.0, 205.0), (400.0, 235.0), (370.0, 235.0)];
    let strokes = serde_json::json!([
        { "tool": "land", "closed": true, "hard": true, "pts": coast.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "river", "radius_ft": 2.0 * MI, "strength": 0.5, "pts": tributary.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.7, "pts": main.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "biome", "biome": "volcanic", "closed": true, "hard": true, "pts": barren.iter().map(mi).collect::<Vec<_>>() },
        { "tool": "pin", "tier": "town", "name": "Saltmouth", "pts": [mi(&port)] },
    ]);
    let params = serde_json::json!({ "width_mi": 420.0, "height_mi": 300.0, "max_elev_ft": 1000.0, "ruggedness": 0.1, "settlement_density": 0.0 });
    let file: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 7, "params": params, "sketch": { "strokes": strokes } })).unwrap();
    let world = World::new(file).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let extra = t0.extra.as_ref().unwrap();
    let hydro = &extra.hydro;
    let sea = world.params().sea_level_ft;
    let (w, h, cell) = (t0.height.w, t0.height.h, t0.cell_ft);
    let no_lake = worldgen::t0::hydro::NO_LAKE;

    // Each drawn river: its cells on land, a mapped river within a cell of each.
    let mut river_cell = vec![false; w * h];
    for r in &hydro.rivers {
        for &c in &r.cells {
            river_cell[c as usize] = true;
        }
    }
    for (name, pts) in [("long river", &main[..]), ("tributary", &tributary[..])] {
        let (mut on, mut all, mut lake) = (0, 0, 0);
        let mut last = usize::MAX;
        for seg in pts.windows(2) {
            for t in 0..=200 {
                let f = t as f64 / 200.0;
                let (x, y) = ((seg[0].0 + (seg[1].0 - seg[0].0) * f) * MI, (seg[0].1 + (seg[1].1 - seg[0].1) * f) * MI);
                let (i, j) = (((x / cell).round() as usize).min(w - 1), ((y / cell).round() as usize).min(h - 1));
                let k = j * w + i;
                if k == last || (t0.height.data[k] as f64 <= sea && hydro.lake_of[k] == no_lake && t0.water.data[k] > worldgen::t0::hydro::DRY) {
                    continue;
                }
                last = k;
                all += 1;
                lake += usize::from(hydro.lake_of[k] != no_lake);
                on += usize::from((j.saturating_sub(1)..=(j + 1).min(h - 1)).any(|y| (i.saturating_sub(1)..=(i + 1).min(w - 1)).any(|x| river_cell[y * w + x])));
            }
        }
        assert!(on * 10 >= all * 9, "the drawn {name} is mapped as a river over only {on}/{all} cells ({lake} in lakes)");
        assert!(lake * 20 <= all, "the drawn {name} runs through lakes over {lake}/{all} cells");
    }
    // (Both reach the sea: neither is left ending inland.)
    let stuck: Vec<&str> = extra.overlay.conflicts.iter().filter(|c| c.stroke <= 2 && c.message.contains("inland")).map(|c| c.message.as_str()).collect();
    assert!(stuck.is_empty(), "a drawn river did not flow on to the sea: {stuck:?}");

    // No dry land below sea level (a river or plain dug under the sea).
    let low = (0..w * h).filter(|&k| (t0.height.data[k] as f64) < sea && t0.water.data[k] <= worldgen::t0::hydro::DRY && hydro.lake_of[k] == no_lake).count();
    assert_eq!(low, 0, "{low} cells of dry land below sea level");

    // The pin on the barren coast is a port.
    let s = t0.settlements.iter().find(|s| s.pin == Some(4)).expect("the coastal pin placed no settlement");
    assert!(s.coastal && s.kind == worldgen::t0::settle::SettleKind::Port, "the town pinned on the coast is not a port ({:?}, coastal {})", s.kind, s.coastal);
}

/// Sketch guarantee (R2.4): relief and water drawn as areas and points. With the plates'
/// mountains off, the land is high only where drawn: a massif is mountains across its outline,
/// a plateau stands its height above the land round it without damming the streams that cross
/// it, and the plains stay low. A lake drawn on the plain fills its outline (and little more),
/// keeps its name and, with a river drawn through it, is fresh and the river runs on out of it.
/// Drawn volcanoes stand where drawn, as drawn, and no others are placed.
fn sketch_relief_and_lakes() {
    const MI: f64 = 5280.0;
    let p = |x: f64, y: f64| serde_json::json!([(x * MI).round(), (y * MI).round()]);
    let ring = |cx: f64, cy: f64, rx: f64, ry: f64, n: usize, wob: f64| -> Vec<(f64, f64)> {
        (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                let r = 1.0 + wob * (3.0 * a).sin() + wob * 0.6 * (5.0 * a + 1.0).cos();
                (cx + rx * r * a.cos(), cy + ry * r * a.sin())
            })
            .collect()
    };
    let pts = |v: &[(f64, f64)]| v.iter().map(|q| p(q.0, q.1)).collect::<Vec<_>>();
    let massif = ring(170.0, 130.0, 90.0, 45.0, 20, 0.08);
    let plateau = ring(390.0, 285.0, 80.0, 40.0, 20, 0.08);
    let lake = ring(350.0, 150.0, 18.0, 10.0, 16, 0.08);
    let river = [(210.0, 160.0), (280.0, 165.0), (330.0, 152.0), (370.0, 148.0), (450.0, 130.0), (520.0, 110.0), (575.0, 100.0)];
    let strokes = serde_json::json!([
        { "tool": "land", "closed": true, "pts": pts(&ring(300.0, 200.0, 260.0, 165.0, 48, 0.05)) },
        { "tool": "massif", "closed": true, "radius_ft": 12.0 * MI, "strength": 0.8, "pts": pts(&massif) },
        { "tool": "elevation", "closed": true, "radius_ft": 10.0 * MI, "delta_ft": 1200.0, "pts": pts(&plateau) },
        { "tool": "lake", "closed": true, "name": "Mirrormere", "pts": pts(&lake) },
        { "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.6, "pts": pts(&river) },
        { "tool": "volcano", "kind": "caldera", "activity": "dormant", "name": "Mount Ash", "strength": 0.7, "pts": [p(450.0, 230.0)] },
        { "tool": "volcano", "kind": "strato", "activity": "active", "name": "Emberhorn", "strength": 0.8, "pts": [p(120.0, 280.0)] },
    ]);
    let params = serde_json::json!({ "width_mi": 600.0, "height_mi": 400.0, "procedural_mountains": 0.0, "volcanoes": 0 });
    let file: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 5, "params": params, "sketch": { "strokes": strokes } })).unwrap();
    let world = World::new(file).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let extra = t0.extra.as_ref().unwrap();
    let hydro = &extra.hydro;
    let sea = world.params().sea_level_ft;
    let (w, h, cell) = (t0.height.w, t0.height.h, t0.cell_ft);
    let no_lake = worldgen::t0::hydro::NO_LAKE;
    let inside = |poly: &[(f64, f64)], x: f64, y: f64| {
        let mut ins = false;
        for e in 0..poly.len() {
            let (a, b) = (poly[e], poly[(e + 1) % poly.len()]);
            if (a.1 <= y) != (b.1 <= y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                ins = !ins;
            }
        }
        ins
    };
    // Distance (mi) from a point to an outline's edge.
    let edge = |poly: &[(f64, f64)], x: f64, y: f64| {
        (0..poly.len())
            .map(|e| {
                let (a, b) = (poly[e], poly[(e + 1) % poly.len()]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                ((a.0 + t * dx - x).powi(2) + (a.1 + t * dy - y).powi(2)).sqrt()
            })
            .fold(f64::MAX, f64::min)
    };
    let at = |k: usize| ((k % w) as f64 * cell / MI, (k / w) as f64 * cell / MI);
    let above = |k: usize| t0.height.data[k] as f64 - sea;
    let land = |k: usize| t0.height.data[k] as f64 > sea || hydro.lake_of[k] != no_lake;

    // The massif: mountains across it (10+ mi inside, the 80th percentile over 4,000 ft).
    let mut core: Vec<f64> = (0..w * h).filter(|&k| { let (x, y) = at(k); inside(&massif, x, y) && edge(&massif, x, y) > 10.0 }).map(above).collect();
    core.sort_by(|a, b| a.total_cmp(b));
    let m80 = core[core.len() * 8 / 10];
    assert!(m80 > 4_000.0, "the massif is not mountains across its outline: 80th percentile {m80:.0} ft");

    // The plains (land 40+ mi from anything drawn): low, with the plates' mountains off.
    let mut plains: Vec<f64> = (0..w * h)
        .filter(|&k| {
            let (x, y) = at(k);
            land(k) && edge(&massif, x, y) > 40.0 && !inside(&massif, x, y) && edge(&plateau, x, y) > 40.0 && !inside(&plateau, x, y) && ((x - 450.0).powi(2) + (y - 230.0).powi(2)).sqrt() > 40.0 && ((x - 120.0).powi(2) + (y - 280.0).powi(2)).sqrt() > 40.0
        })
        .map(above)
        .collect();
    plains.sort_by(|a, b| a.total_cmp(b));
    let p95 = plains[plains.len() * 95 / 100];
    assert!(p95 < 2_500.0, "the plains rise to {p95:.0} ft (95th percentile) with the plates' mountains off");

    // The plateau: 1,200 ft drawn, at least 900 ft over the land round it (median inside 10+ mi
    // from its edge vs a ring 15–35 mi out), and no undrawn lakes dammed round it.
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    };
    let top = median((0..w * h).filter(|&k| { let (x, y) = at(k); land(k) && inside(&plateau, x, y) && edge(&plateau, x, y) > 10.0 }).map(above).collect());
    let round = median((0..w * h).filter(|&k| { let (x, y) = at(k); land(k) && !inside(&plateau, x, y) && (15.0..35.0).contains(&edge(&plateau, x, y)) }).map(above).collect());
    assert!(top - round > 900.0, "the plateau stands only {:.0} ft over the land round it", top - round);
    let near: Vec<usize> = (0..w * h).filter(|&k| { let (x, y) = at(k); land(k) && (inside(&plateau, x, y) || edge(&plateau, x, y) < 30.0) }).collect();
    let dammed = near.iter().filter(|&&k| hydro.lake_of[k] != no_lake && hydro.lakes[hydro.lake_of[k] as usize].stroke.is_none()).count();
    assert!(dammed * 50 <= near.len(), "lakes dammed round the plateau: {dammed}/{} cells", near.len());

    // The lake: its outline's cells are its water, hardly any beyond; named, fresh.
    let li = hydro.lakes.iter().position(|l| l.stroke == Some(3)).expect("the drawn lake is not a lake");
    let lk = &hydro.lakes[li];
    let ins = (0..w * h).filter(|&k| { let (x, y) = at(k); inside(&lake, x, y) }).count();
    let wet = lk.cells.iter().filter(|&&c| { let (x, y) = at(c as usize); inside(&lake, x, y) }).count();
    assert!(wet * 10 >= ins * 9 && (lk.cells.len() - wet) * 10 <= ins, "the drawn lake: {wet}/{ins} cells of its outline wet, {} outside", lk.cells.len() - wet);
    assert_eq!(lk.kind, worldgen::t0::hydro::LakeKind::Fresh, "a lake with a river drawn out of it is not fresh");
    let feature = extra.overlay.features.iter().find(|f| f.name == "Mirrormere").expect("the drawn lake lost its name");
    assert!(feature.kind == "lake" && edge(&lake, feature.x / MI, feature.y / MI) < 20.0, "Mirrormere is a {} at ({:.0}, {:.0}) mi", feature.kind, feature.x / MI, feature.y / MI);

    // The river through it: mapped as a river (lake and sea aside) over 90%+ of its course.
    let mut river_cell = vec![false; w * h];
    for r in &hydro.rivers {
        for &c in &r.cells {
            river_cell[c as usize] = true;
        }
    }
    let (mut on, mut all, mut last) = (0, 0, usize::MAX);
    for seg in river.windows(2) {
        for t in 0..=200 {
            let f = t as f64 / 200.0;
            let (x, y) = (seg[0].0 + (seg[1].0 - seg[0].0) * f, seg[0].1 + (seg[1].1 - seg[0].1) * f);
            let (i, j) = (((x * MI / cell).round() as usize).min(w - 1), ((y * MI / cell).round() as usize).min(h - 1));
            let k = j * w + i;
            if k == last || !land(k) || hydro.lake_of[k] != no_lake || inside(&lake, x, y) {
                continue;
            }
            last = k;
            all += 1;
            on += usize::from((j.saturating_sub(1)..=(j + 1).min(h - 1)).any(|y| (i.saturating_sub(1)..=(i + 1).min(w - 1)).any(|x| river_cell[y * w + x])));
        }
    }
    assert!(on * 10 >= all * 9, "the river drawn through the lake is mapped over only {on}/{all} cells");

    // Volcanoes: the two drawn, where drawn, as drawn; none placed.
    let volcanoes: Vec<_> = extra.overlay.features.iter().filter(|f| f.kind == "volcano").collect();
    assert_eq!(volcanoes.len(), 2, "volcanoes: {:?}", volcanoes.iter().map(|f| &f.name).collect::<Vec<_>>());
    for (name, x, y, what) in [("Mount Ash", 450.0, 230.0, "dormant caldera"), ("Emberhorn", 120.0, 280.0, "active stratovolcano")] {
        let f = volcanoes.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("the volcano {name} lost its name"));
        let d = ((f.x / MI - x).powi(2) + (f.y / MI - y).powi(2)).sqrt();
        assert!(d < 2.0 && f.detail.as_deref().is_some_and(|s| s.starts_with(what)), "{name}: {:?}, {d:.1} mi from where drawn", f.detail);
    }
    println!("relief and lakes: massif 80th pct {m80:.0} ft, plains 95th pct {p95:.0} ft, plateau +{:.0} ft ({dammed} dammed cells), lake {wet}/{ins} (+{}), river {on}/{all}", top - round, lk.cells.len() - wet);
}

/// Sketch guarantee (R3): what the sketch names keeps its name, and what it places stays put.
/// Two named mountain strokes drawn into one range name its two parts; a named river, a region
/// outline, two region names in one plain (each naming its own part), a named blighted wood and
/// named ashlands (both making battlemaps) name what they were drawn on; a pinned capital is the capital, a pinned fortress a fortress,
/// a pin's wards name its districts (the central one first); a site drawn as a ruin with a crypt
/// is one, named, and stays where it was, with its crypt, when the sketch changes elsewhere.
fn sketch_names_and_sites() {
    const MI: f64 = 5280.0;
    let p = |x: f64, y: f64| serde_json::json!([(x * MI).round(), (y * MI).round()]);
    let ring = |cx: f64, cy: f64, rx: f64, ry: f64, n: usize| -> Vec<serde_json::Value> {
        (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                p(cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect()
    };
    let wards = ["Old Market", "Crown Hill", "Tanner's Row"];
    let mut strokes = vec![
        serde_json::json!({ "tool": "land", "closed": true, "pts": ring(300.0, 200.0, 270.0, 175.0, 48) }),
        serde_json::json!({ "tool": "massif", "closed": true, "radius_ft": 10.0 * MI, "strength": 0.9, "name": "Greyspine Massif", "pts": ring(150.0, 110.0, 70.0, 40.0, 20) }),
        serde_json::json!({ "tool": "range", "radius_ft": 15.0 * MI, "strength": 0.9, "name": "Silberquel Ridge", "pts": [p(215.0, 110.0), p(280.0, 100.0), p(340.0, 90.0)] }),
        serde_json::json!({ "tool": "river", "radius_ft": 3.0 * MI, "strength": 0.8, "name": "Glory Run", "pts": [p(330.0, 140.0), p(380.0, 200.0), p(450.0, 260.0), p(520.0, 330.0), p(560.0, 380.0)] }),
        serde_json::json!({ "tool": "biome", "biome": "grassland", "closed": true, "hard": true, "pts": [p(60.0, 190.0), p(300.0, 190.0), p(300.0, 330.0), p(60.0, 330.0)] }),
        serde_json::json!({ "tool": "region", "name": "Zemni Fields", "pts": [p(110.0, 250.0)] }),
        serde_json::json!({ "tool": "region", "name": "Marrow Valley", "pts": [p(250.0, 260.0)] }),
        serde_json::json!({ "tool": "region", "name": "Truscan Vale", "closed": true, "pts": [p(420.0, 160.0), p(470.0, 160.0), p(470.0, 200.0), p(420.0, 200.0)] }),
        serde_json::json!({ "tool": "biome", "biome": "blighted_woods", "closed": true, "name": "The Pallid Grove", "pts": ring(470.0, 110.0, 30.0, 22.0, 16) }),
        serde_json::json!({ "tool": "biome", "biome": "ashlands", "closed": true, "name": "The Cinderwaste", "pts": ring(150.0, 290.0, 30.0, 20.0, 16) }),
        serde_json::json!({ "tool": "site", "kind": "ruin", "under": "crypt", "name": "Barrow of Kings", "pts": [p(350.0, 300.0)] }),
        serde_json::json!({ "tool": "pin", "tier": "town", "capital": true, "name": "Rexxentrum", "pts": [p(200.0, 230.0)] }),
        serde_json::json!({ "tool": "pin", "tier": "city", "kind": "fortress", "name": "Bladegarden", "wards": wards, "pts": [p(380.0, 280.0)] }),
        // A king's road bending south between the two pins, and a line no planned road crosses.
        serde_json::json!({ "tool": "road", "kind": "kings_road", "name": "The Gilded Roadway", "pts": [p(201.0, 232.0), p(225.0, 275.0), p(270.0, 305.0), p(330.0, 305.0), p(378.0, 282.0)] }),
        serde_json::json!({ "tool": "road", "kind": "none", "pts": [p(150.0, 150.0), p(150.0, 215.0), p(140.0, 260.0)] }),
    ];
    let params = serde_json::json!({ "width_mi": 600.0, "height_mi": 400.0, "volcanoes": 0 });
    let file = |strokes: &[serde_json::Value]| -> World {
        let f: WorldFile = serde_json::from_value(serde_json::json!({ "gen_version": worldgen::world::GEN_VERSION, "seed": 9, "params": params, "sketch": { "strokes": strokes } })).unwrap();
        World::new(f).unwrap()
    };
    let mut ex = Executor::new(file(&strokes));
    // The painted blighted woods and ashlands make battlemaps, with their objects.
    for (x, y) in [(470.0, 110.0), (150.0, 290.0)] {
        let size = ex.world.geom.tile_size_ft(ex.world.geom.max_level);
        let key = TileKey::surface(ex.world.geom.max_level, (x * MI / size) as u32, (y * MI / size) as u32);
        assert!(!ex.battlemap(key).objects.is_empty(), "no battlemap objects in the painted biome at ({x}, {y}) mi");
    }
    let (world, t0) = (&ex.world, &ex.t0);
    let extra = t0.extra.as_ref().unwrap();
    let feats = &extra.overlay.features;
    let named = |name: &str| feats.iter().filter(|f| f.name == name).collect::<Vec<_>>();
    let conflicts: Vec<&str> = extra.overlay.conflicts.iter().map(|c| c.message.as_str()).collect();

    for (name, kind) in [("Greyspine Massif", "range"), ("Silberquel Ridge", "range"), ("Glory Run", "river"), ("Zemni Fields", "plains"), ("Marrow Valley", "plains"), ("The Pallid Grove", "blight"), ("The Cinderwaste", "ashlands")] {
        let fs = named(name);
        assert!(fs.iter().any(|f| f.kind == kind), "no {kind} named {name} (named so: {:?}; conflicts {conflicts:?})", fs.iter().map(|f| f.kind).collect::<Vec<_>>());
    }
    let vale = named("Truscan Vale");
    assert_eq!(vale.len(), 1, "the region outline named {} features", vale.len());
    // The two plains names name two parts of the plain, each holding its own name's point.
    let part = |name: &str| extra.overlay.shapes[feats.iter().position(|f| f.name == name && f.kind == "plains").unwrap()].clone();
    assert!(part("Zemni Fields").distance([110.0 * MI, 250.0 * MI]).0 == 0.0 && part("Marrow Valley").distance([250.0 * MI, 260.0 * MI]).0 == 0.0, "the plain is not split between its names");

    // Pins: the capital, the fortress and its wards.
    let at = |name: &str| t0.settlements.iter().position(|s| feats.iter().any(|f| f.name == name && (f.x - s.x).abs() < 1.0 && (f.y - s.y).abs() < 1.0)).unwrap_or_else(|| panic!("no settlement {name}"));
    let (capital, fort) = (at("Rexxentrum"), at("Bladegarden"));
    assert!(t0.settlements[capital].capital && t0.settlements.iter().filter(|s| s.capital).count() == 1, "the pinned capital is not the only capital");
    assert_eq!(t0.settlements[fort].kind, worldgen::t0::settle::SettleKind::Fortress, "the pinned fortress is not a fortress");
    let layout = worldgen::town::generate(&world, &t0, fort);
    let centre = layout.quarters.iter().find(|q| q.kind == worldgen::town::QuarterKind::Center).expect("a centre");
    assert_eq!(centre.name, wards[0], "the first ward doesn't name the central district");
    assert!(wards[1..].iter().all(|w| layout.quarters.iter().any(|q| q.name == *w)), "the wards don't name districts");

    // The drawn road: followed (every half mile of its line within a mile of the road made from
    // it), named, joining the pins at its ends; no planned road crosses the `none` line.
    let seg = |q: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let t = (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
        ((a[0] + t * dx - q[0]).powi(2) + (a[1] + t * dy - q[1]).powi(2)).sqrt()
    };
    let strokes_of = |tool_kind: Option<&str>| world.file.sketch.strokes.iter().position(|s| s.tool == worldgen::world::SketchTool::Road && s.kind.as_deref() == tool_kind).unwrap();
    let (gilded, none) = (strokes_of(Some("kings_road")), strokes_of(Some("none")));
    let made: Vec<&worldgen::lod::roads::RoadCurve> = t0.roads.roads.iter().filter(|r| r.stroke == Some(gilded as u32)).collect();
    let line = &world.file.sketch.strokes[gilded].pts;
    let (mut n, mut far) = (0, Vec::new());
    for w in line.windows(2) {
        let k = (((w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])) / (0.5 * MI)).ceil() as usize;
        for j in 0..k {
            let q = [w[0][0] + (w[1][0] - w[0][0]) * j as f64 / k as f64, w[0][1] + (w[1][1] - w[0][1]) * j as f64 / k as f64];
            let d = made.iter().flat_map(|r| r.pts.windows(2).map(|v| seg(q, v[0], v[1]))).fold(f64::INFINITY, f64::min);
            n += 1;
            if d > MI {
                far.push((q[0] / MI, q[1] / MI, d / MI));
            }
        }
    }
    assert!(far.len() * 20 <= n, "the drawn road strays over a mile from its line at {} of {n} points: {:?}", far.len(), &far[..far.len().min(6)]);
    assert!(made.iter().all(|r| r.class == worldgen::t0::roads::RoadClass::KingsRoad), "the drawn king's road isn't one");
    assert!(feats.iter().any(|f| f.kind == "road" && f.name == "The Gilded Roadway"), "the named road is not a feature");
    let near_pin = |name: &str| {
        let s = &t0.settlements[at(name)];
        let r = worldgen::town::road_trim_radius(s.tier, s.population) + 3_000.0;
        made.iter().any(|c| [c.pts[0], *c.pts.last().unwrap()].iter().any(|e| (e[0] - s.x).hypot(e[1] - s.y) < r))
    };
    assert!(near_pin("Rexxentrum") && near_pin("Bladegarden"), "the drawn road doesn't reach the pins at its ends");
    let cut = &world.file.sketch.strokes[none].pts;
    let crossing = t0.roads.roads.iter().filter(|r| r.stroke.is_none()).find_map(|r| {
        r.pts.windows(2).find_map(|v| {
            let side = |p: [f64; 2], q: [f64; 2], o: [f64; 2]| (q[0] - p[0]) * (o[1] - p[1]) - (q[1] - p[1]) * (o[0] - p[0]);
            cut.windows(2).find(|c| side(v[0], v[1], c[0]) * side(v[0], v[1], c[1]) < 0.0 && side(c[0], c[1], v[0]) * side(c[0], c[1], v[1]) < 0.0).map(|_| (v[0][0] / MI, v[0][1] / MI))
        })
    });
    assert!(crossing.is_none(), "a planned road crosses the no-road line at {crossing:?} mi");

    // The site: a named ruin over a crypt, where it was drawn; still there when the sketch changes
    // elsewhere.
    let site = |t0: &worldgen::t0::T0| {
        let i = (0..t0.base_pois).find(|&i| t0.pois[i].stroke.is_some()).expect("the site was not placed");
        (i, t0.pois[i].kind, t0.pois[i].x, t0.pois[i].y, t0.created_site(i).and_then(|c| c.under), t0.pois[i].seed)
    };
    let (i, kind, x, y, under, seed) = site(&t0);
    assert!(kind == worldgen::t0::settle::PoiKind::Ruin && under == Some(worldgen::under::UnderKind::Crypt), "the site is not a ruin over a crypt");
    assert!(((x / MI - 350.0).powi(2) + (y / MI - 300.0).powi(2)).sqrt() < 2.0, "the site moved from where it was drawn");
    let id = feats.iter().find(|f| f.name == "Barrow of Kings").expect("the site lost its name").id.clone();
    strokes.push(serde_json::json!({ "tool": "pin", "tier": "village", "name": "Elsewhere", "pts": [p(520.0, 120.0)] }));
    let world2 = file(&strokes);
    let mut t2 = worldgen::t0::T0::generate(&world2);
    t2.apply_edits(&world2);
    let (_, kind2, x2, y2, under2, seed2) = site(&t2);
    assert!(kind2 == kind && under2 == under && (x2 - x).abs() < 1.0 && (y2 - y).abs() < 1.0 && seed2 == seed, "the site changed with the sketch elsewhere");
    let f2 = t2.extra.as_ref().unwrap().overlay.features.iter().find(|f| f.name == "Barrow of Kings").expect("the site lost its name");
    assert_eq!(f2.id, id, "the site's id changed with the sketch elsewhere");
    let _ = i;

    // Generated roads off: only the drawn road and short spurs from it to the settlements beside
    // it, no roadside inns; the settlements stand where they would without the road.
    let off = |strokes: &[serde_json::Value]| {
        let mut f = file(strokes).file;
        f.params.generated_roads = false;
        let w = World::new(f).unwrap();
        let t = worldgen::t0::T0::generate(&w);
        (w, t)
    };
    let roads = &strokes[strokes.len() - 3..strokes.len() - 1];
    assert!(roads.iter().all(|s| s["tool"] == "road"), "the road strokes are not where this test expects them");
    let (w_road, t_road) = off(&strokes[..strokes.len() - 1]);
    let (_, t_none) = off(&strokes[..strokes.len() - 3]);
    let length = |r: &worldgen::lod::roads::RoadCurve| r.pts.windows(2).map(|v| (v[1][0] - v[0][0]).hypot(v[1][1] - v[0][1])).sum::<f64>();
    let gilded = w_road.file.sketch.strokes.iter().position(|s| s.kind.as_deref() == Some("kings_road")).unwrap() as u32;
    assert!(t_road.roads.roads.iter().any(|r| r.stroke == Some(gilded)), "the drawn road is not made with generated roads off");
    let long: Vec<f64> = t_road.roads.roads.iter().filter(|r| r.stroke.is_none()).map(length).filter(|&l| l > 10.0 * MI).map(|l| l / MI).collect();
    assert!(long.is_empty(), "with generated roads off, roads not drawn run {long:?} mi");
    assert!(t_none.roads.roads.is_empty(), "with generated roads off and none drawn, there are roads");
    assert!(!t_road.pois.iter().any(|p| p.kind == worldgen::t0::settle::PoiKind::Waystation), "roadside inns made with generated roads off");
    let places = |t: &worldgen::t0::T0| t.settlements.iter().map(|s| (s.x.round() as i64, s.y.round() as i64, s.tier as u8, s.population)).collect::<Vec<_>>();
    assert_eq!(places(&t_road), places(&t_none), "drawing a road moved or resized settlements with generated roads off");
}

// v63 castles
