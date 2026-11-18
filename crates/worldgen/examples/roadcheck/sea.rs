use crate::prelude::*;

/// The sea ferries (`t0::ferries`): how many, their water (miles), the settled islands they serve
/// against those with bigger land with roads within `WATER_MAX_MI` (cell centres less two cells);
/// with `FERRY_MAP=1`, the battlemap chunks out from each of the first few landings: their ferry
/// shapes, decks and squares of road bed on the water (`FERRY_PROFILE=1`: each line's water at L10).
pub(crate) fn sea_ferries(world: &worldgen::world::World, t0: &T0) {
    use worldgen::t0::biome::Biome;
    use worldgen::t0::{ferries, islands};
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let b = |k: usize| Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
    let land: Vec<bool> = (0..w * h).map(|k| b(k) != Biome::Ocean).collect();
    let lake: Vec<u32> = (0..w * h).map(|k| if b(k) == Biome::Lake { 0 } else { worldgen::t0::hydro::NO_LAKE }).collect();
    let isl = islands::label(w, h, cell, &land, &lake, &t0.biome.data);
    let mut settled = vec![false; isl.parts.len()];
    for s in t0.settlements.iter().filter(|s| !s.isolated) {
        if isl.of[s.cell] != islands::NONE {
            settled[isl.of[s.cell] as usize] = true;
        }
    }
    let at_cell = |p: P| (((p[1] / cell).round() as usize).min(h - 1)) * w + ((p[0] / cell).round() as usize).min(w - 1);
    let ferried: std::collections::BTreeSet<u32> = t0.ferries.iter().map(|f| isl.of[at_cell(f.a)]).collect();
    let reach = (ferries::WATER_MAX_MI * 5280.0 / cell + 2.2).ceil() as i64;
    let mut within = 0;
    for (m, part) in isl.parts.iter().enumerate() {
        if m as u32 == isl.mainland || !settled[m] {
            continue;
        }
        let near = (0..w * h).filter(|&k| isl.of[k] == m as u32).any(|k| {
            let (x, y) = ((k % w) as i64, (k / w) as i64);
            (-reach..=reach).any(|dy| (-reach..=reach).any(|dx| {
                let (i, j) = (x + dx, y + dy);
                if i < 0 || j < 0 || i >= w as i64 || j >= h as i64 || ((dx * dx + dy * dy) as f64).sqrt() - 2.2 > ferries::WATER_MAX_MI * 5280.0 / cell {
                    return false;
                }
                let o = isl.of[j as usize * w + i as usize];
                o != islands::NONE && o != m as u32 && settled[o as usize] && isl.parts[o as usize].area_mi2 > part.area_mi2
            }))
        });
        within += near as usize;
    }
    let mut mi: Vec<f64> = t0.ferries.iter().map(|f| f.mi()).collect();
    mi.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| mi.get(((mi.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(0.0);
    say!("  ferries: {}, water min/median/max {:.1}/{:.1}/{:.1} mi; islands ferried {} / settled islands by bigger land with roads (cell centres within the reach + 2.2 cells) {within}", t0.ferries.len(), q(0.0), q(0.5), q(1.0), ferried.len());
    let fer = worldgen::t0::ferries::overlay_entries(t0);
    if !fer.is_empty() {
        let o = t0.overlay_json();
        say!("      overlay ferries (c 3): {} in sea_lanes, {}", o.matches("{\"c\":3,").count(), fer.split("],\"p\"").map(|v| v.rsplit("\"e\":[").next().unwrap_or("")).filter(|v| !v.is_empty() && !v.contains(']')).map(|v| format!("e [{v}]")).collect::<Vec<_>>().join(", "));
    }
    for f in &t0.ferries {
        say!("      ferry ({:.0}, {:.0}) -> ({:.0}, {:.0}), {:.2} mi", f.a[0], f.a[1], f.b[0], f.b[1], f.mi());
    }
    // Each landing's track (the road ending at it): real miles, climb and range of its bed (ft).
    for f in &t0.ferries {
        let t = std::time::Instant::now();
        let sh = f.shores(world, t0);
        say!("        stages ({:.0}, {:.0}) {:.0} ft out, ({:.0}, {:.0}) {:.0} ft out ({} ms)", sh[0][0], sh[0][1], dist(sh[0], f.a), sh[1][0], sh[1][1], dist(sh[1], f.b), t.elapsed().as_millis());
        for p in [f.a, f.b] {
            let near = |q: &P| dist(*q, p) < ferries::INLAND_FT + 30.0;
            let Some(r) = t0.roads.roads.iter().find(|r| r.pts.first().is_some_and(near) || r.pts.last().is_some_and(near)) else {
                say!("        landing ({:.0}, {:.0}): no track ends here", p[0], p[1]);
                continue;
            };
            let len: f64 = r.pts.windows(2).map(|s| dist(s[0], s[1])).sum();
            let climb: f32 = r.z.windows(2).map(|z| (z[1] - z[0]).abs()).sum();
            let (lo, hi) = r.z.iter().fold((f32::MAX, f32::MIN), |(a, b), &z| (a.min(z), b.max(z)));
            say!("        landing ({:.0}, {:.0}): track {:.1} mi, climb {:.0} ft, bed {:.0}..{:.0} ft", p[0], p[1], len / 5280.0, climb, lo, hi);
        }
    }
    if !std::env::var("FERRY_MAP").is_ok_and(|v| v == "1") {
        return;
    }
    let g = &world.geom;
    if std::env::var("FERRY_PROBE").is_ok() {
        // Out from each landing every 25 ft: the plan's ground and the finest tile's, less the water.
        use worldgen::core::tile::{HALO, PADDED, TileKey};
        let level = g.max_level;
        let (s, size) = (g.spacing_ft(level), g.tile_size_ft(level));
        let lattice = g.spacing_ft(g.first_refine_level.saturating_sub(1));
        let mut cache: std::collections::BTreeMap<(u32, u32), Vec<f32>> = Default::default();
        for f in &t0.ferries {
            for (p, q) in [(f.a, f.b), (f.b, f.a)] {
                let l = dist(p, q);
                let mut row = String::new();
                for k in 0..=48 {
                    let t = k as f64 * 25.0 - 100.0;
                    let (x, y) = (p[0] + (q[0] - p[0]) * t / l, p[1] + (q[1] - p[1]) * t / l);
                    let (tx, ty) = ((x / size) as u32, (y / size) as u32);
                    let pad = cache.entry((tx, ty)).or_insert_with(|| {
                        let mut parent: Option<Vec<f32>> = None;
                        for lv in 0..=level {
                            let pk = TileKey::surface(lv, tx >> (level - lv), ty >> (level - lv));
                            parent = Some(worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref()).padded.to_vec());
                        }
                        parent.unwrap()
                    });
                    let (ox, oy) = (tx as f64 * size, ty as f64 * size);
                    let (i, j) = (((x - ox) / s).round() as usize + HALO, ((y - oy) / s).round() as usize + HALO);
                    let wv = t0.sample_water(x, y) as f64;
                    let gp = t0.rivers.valley(t0.ground_at(x, y, lattice), x, y);
                    row += &format!(" {:.0}:{:.1}/{:.1}", t, gp - wv.max(-1e3), pad[j * PADDED + i] as f64 - wv.max(-1e3));
                }
                say!("        probe from ({:.0}, {:.0}) t:plan-water/tile-water:{row}", p[0], p[1]);
            }
        }
    }
    if std::env::var("FERRY_PROFILE").is_ok() {
        use worldgen::core::tile::{HALO, PADDED, TileKey};
        let level = 10u8;
        let s = g.spacing_ft(level);
        let size = g.tile_size_ft(level);
        let sea = world.params().sea_level_ft;
        let mut cache: std::collections::BTreeMap<(u32, u32), Vec<f32>> = Default::default();
        for f in &t0.ferries {
            let l = dist(f.a, f.b);
            let row: String = (0..=(l / 100.0) as usize).map(|k| {
                let t = k as f64 * 100.0;
                let (x, y) = (f.a[0] + (f.b[0] - f.a[0]) * t / l, f.a[1] + (f.b[1] - f.a[1]) * t / l);
                let (tx, ty) = ((x / size) as u32, (y / size) as u32);
                let pad = cache.entry((tx, ty)).or_insert_with(|| {
                    let mut parent: Option<Vec<f32>> = None;
                    for lv in 0..=level {
                        let pk = TileKey::surface(lv, tx >> (level - lv), ty >> (level - lv));
                        parent = Some(worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref()).padded.to_vec());
                    }
                    parent.unwrap()
                });
                let (ox, oy) = (tx as f64 * size, ty as f64 * size);
                let (i, j) = (((x - ox) / s).round() as usize + HALO, ((y - oy) / s).round() as usize + HALO);
                let z = pad[j * PADDED + i];
                if (z as f64) < sea as f64 { '~' } else { '#' }
            }).collect();
            say!("        line L10 every 100 ft: {row}");
        }
    }
    for f in t0.ferries.iter().take(3) {
        // (Each chunk along the line out to where its stage may stand.)
        let level = g.max_level;
        let size = g.tile_size_ft(level);
        let mut keys: Vec<(P, worldgen::core::tile::TileKey)> = Vec::new();
        for (p, q) in [(f.a, f.b), (f.b, f.a)] {
            let l = dist(p, q);
            for m in 0..=13 {
                let t = m as f64 * 100.0 - 60.0;
                let c = [p[0] + (q[0] - p[0]) * t / l, p[1] + (q[1] - p[1]) * t / l];
                let key = worldgen::core::tile::TileKey::surface(level, (c[0] / size) as u32, (c[1] / size) as u32);
                if !keys.iter().any(|k| k.1 == key) {
                    keys.push((p, key));
                }
            }
        }
        for (p, key) in keys {
            let mut parent: Option<Vec<f32>> = None;
            let mut tile = None;
            for l in 0..=level {
                let pk = worldgen::core::tile::TileKey::surface(l, key.x >> (level - l), key.y >> (level - l));
                let t = worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref());
                parent = Some(t.padded.clone());
                tile = Some(t);
            }
            let tile = tile.expect("a tile");
            let chunk = worldgen::battlemap::generate(world, t0, &key, &tile);
            let shapes = chunk.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Ferry).count();
            let hs = (chunk.road_halo.len() as f64).sqrt() as usize;
            let (mut bed, mut deck, mut wet) = (0, 0, 0);
            for i in 0..hs * hs {
                if chunk.halo_water[i] > chunk.halo_height[i] - 0.01 || chunk.road_halo[i] & 0x80 != 0 {
                    wet += 1;
                    if chunk.road_halo[i] & 0x80 != 0 {
                        deck += 1;
                    } else if chunk.road_halo[i] != 0 {
                        bed += 1;
                    }
                }
            }
            let roads = chunk.road_halo.iter().filter(|&&r| r != 0 && r & 0x80 == 0).count();
            say!("      landing ({:.0}, {:.0}) chunk L{level} ({}, {}): ferry shapes {shapes}, water squares {wet}, decks {deck}, road bed on water {bed}, road squares {roads}", p[0], p[1], key.x, key.y);
        }
    }
}

/// The sea lanes: segments, miles (all and per class: busy, normal, quiet), the routed links'
/// lengths, the share of link length within 1 mi of land (outside the last 3 mi into each port),
/// lane points on land (but the ports' own), island ports linked, and island vs mainland ports'
/// trade.
/// `SHORE_PROBE=x,y;x,y`: per place, the road end nearest it and along that road's first 1,000
/// ft (every 100 ft) its bed's height, the finest tile's ground on the line, and the tile's ground
/// 250 ft to each side (past the bed's side slopes), less sea level (ft).
pub(crate) fn shore_probe(world: &worldgen::world::World, t0: &T0, spec: &str) {
    use worldgen::core::tile::{HALO, PADDED, TileKey};
    let g = &world.geom;
    let level = g.max_level;
    let (s, size) = (g.spacing_ft(level), g.tile_size_ft(level));
    let sea = world.params().sea_level_ft;
    let mut cache: std::collections::BTreeMap<(u32, u32), Vec<f32>> = Default::default();
    let mut tile = |x: f64, y: f64| -> f64 {
        let (tx, ty) = ((x / size) as u32, (y / size) as u32);
        let pad = cache.entry((tx, ty)).or_insert_with(|| {
            let mut parent: Option<Vec<f32>> = None;
            for lv in 0..=level {
                let pk = TileKey::surface(lv, tx >> (level - lv), ty >> (level - lv));
                parent = Some(worldgen::lod::terrain_refine::terrain_tile(world, t0, &pk, parent.as_deref()).padded.to_vec());
            }
            parent.unwrap()
        });
        let (i, j) = (((x - tx as f64 * size) / s).round() as usize + HALO, ((y - ty as f64 * size) / s).round() as usize + HALO);
        pad[j * PADDED + i] as f64 - sea
    };
    if let Some(c) = std::env::var("SHORE_GRID").ok().map(|v| v.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).collect::<Vec<f64>>()).filter(|v| v.len() == 3) {
        // (A grid of the finest ground, ft above sea, `c[2]` ft apart, 25 x 25 round (c[0], c[1]).)
        for j in -12..=12 {
            let row: Vec<String> = (-12..=12).map(|i| format!("{:4.0}", tile(c[0] + i as f64 * c[2], c[1] + j as f64 * c[2]))).collect();
            say!("    grid {:+3}: {}", j, row.join(""));
        }
        let lattice = g.spacing_ft(g.first_refine_level.saturating_sub(1));
        for j in [-6, -4, -2] {
            let (x, y) = (c[0] + (-(j + 10) as f64 / 2.0 + 0.0) * c[2], c[1] + j as f64 * c[2]);
            let gr = t0.ground_at(x, y, lattice);
            say!("    at ({x:.0}, {y:.0}): ground_at {:.1}, valley {:.1}, water {:.1}, lake {:?}, tile {:.1} (less sea)", gr - sea, t0.rivers.valley(gr, x, y) - sea, t0.sample_water(x, y) as f64 - sea, t0.sample_lake(x, y), tile(x, y));
        }
    }
    for p in spec.split(';').filter_map(|v| { let v: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect(); (v.len() == 2).then(|| [v[0], v[1]]) }) {
        let Some((r, first)) = t0.roads.roads.iter().flat_map(|r| [(r, true), (r, false)]).filter(|(r, _)| r.pts.len() >= 2).min_by(|a, b| {
            let e = |(r, f): &(&worldgen::lod::roads::RoadCurve, bool)| dist(if *f { r.pts[0] } else { r.pts[r.pts.len() - 1] }, p);
            e(a).total_cmp(&e(b))
        }) else { continue };
        // (The drawn curve, every ~20 ft, as the tiles carve it.)
        let (mut pts, mut z): (Vec<P>, Vec<f32>) = (Vec::new(), Vec::new());
        for k in 0..r.pts.len() - 1 {
            let n = ((r.s[k + 1] - r.s[k]) / 20.0).ceil().max(1.0) as usize;
            for j in 0..n {
                let e = r.eval(k, j as f64 / n as f64, s, t0.cell_ft);
                pts.push(e.p);
                z.push(e.z as f32);
            }
        }
        let e = r.eval(r.pts.len() - 2, 1.0, s, t0.cell_ft);
        pts.push(e.p);
        z.push(e.z as f32);
        if !first {
            pts.reverse();
            z.reverse();
        }
        let mut row = String::new();
        let (mut at, mut k, mut worst) = (0.0, 0usize, f64::MIN);
        let far = std::env::var("SHORE_FT").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(1000.0);
        for m in 0..=(far / 100.0) as usize {
            let t = m as f64 * 100.0;
            while k + 2 < pts.len() && at + dist(pts[k], pts[k + 1]) < t {
                at += dist(pts[k], pts[k + 1]);
                k += 1;
            }
            let l = dist(pts[k], pts[k + 1]).max(1e-9);
            let f = ((t - at) / l).clamp(0.0, 1.0);
            let q = [pts[k][0] + (pts[k + 1][0] - pts[k][0]) * f, pts[k][1] + (pts[k + 1][1] - pts[k][1]) * f];
            let bed = z[k] as f64 + (z[k + 1] - z[k]) as f64 * f - sea;
            let n = [-(pts[k + 1][1] - pts[k][1]) / l, (pts[k + 1][0] - pts[k][0]) / l];
            let side = 0.5 * (tile(q[0] + n[0] * 250.0, q[1] + n[1] * 250.0) + tile(q[0] - n[0] * 250.0, q[1] - n[1] * 250.0));
            let on = tile(q[0], q[1]);
            worst = worst.max(side - on);
            let lattice = g.spacing_ft(g.first_refine_level.saturating_sub(1));
            let (lv, mask) = t0.sample_lake(q[0], q[1]);
            let base = T0::shore_bank(t0.ground_at(q[0], q[1], lattice), lv, mask) - sea;
            row += &format!(" {t:.0}:{bed:.1}/{on:.1}/{side:.1}/{base:.1}");
        }
        say!("    shore probe ({:.0}, {:.0}): road end ({:.0}, {:.0}) {:.0} ft away, {:.1} mi; ft along: bed/tile/side-avg/T0 banked (less sea level):{row}; deepest cut below the sides {worst:.1} ft", p[0], p[1], pts[0][0], pts[0][1], dist(pts[0], p), r.s.last().copied().unwrap_or(0.0) / 5280.0);
    }
}

pub(crate) fn sea_lanes(t0: &T0, world_lattice: f64) {
    use worldgen::t0::biome::Biome;
    use worldgen::t0::{islands, sealanes};
    let Some(extra) = t0.extra.as_ref() else { return };
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let mi = cell / 5280.0;
    let b = |k: usize| Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
    let land: Vec<bool> = (0..w * h).map(|k| b(k) != Biome::Ocean).collect();
    let lake: Vec<u32> = (0..w * h).map(|k| if b(k) == Biome::Lake { 0 } else { worldgen::t0::hydro::NO_LAKE }).collect();
    let isl = islands::label(w, h, cell, &land, &lake, &t0.biome.data);
    let lanes = &t0.sea_lanes;
    let length = |p: &[P]| p.windows(2).map(|q| ((q[1][0] - q[0][0]).powi(2) + (q[1][1] - q[0][1]).powi(2)).sqrt()).sum::<f64>() / 5280.0;
    let mut by = [(0usize, 0.0f64); 3];
    for l in lanes {
        by[l.class as usize].0 += 1;
        by[l.class as usize].1 += length(&l.pts);
    }
    let total: f64 = by.iter().map(|v| v.1).sum();
    let on_land = lanes
        .iter()
        .map(|l| {
            let n = l.pts.len();
            (0..n)
                .filter(|&i| !((i == 0 && l.ends[0] != sealanes::JUNCTION) || (i + 1 == n && l.ends[1] != sealanes::JUNCTION)))
                .filter(|&i| {
                    let p = l.pts[i];
                    let (x, y) = ((p[0] / cell).round() as usize, (p[1] / cell).round() as usize);
                    land[y.min(h - 1) * w + x.min(w - 1)]
                })
                .count()
        })
        .sum::<usize>();
    let links: Vec<_> = extra.economy.sea_links.iter().filter(|l| !l.cells.is_empty()).collect();
    let mut lens: Vec<f64> = links.iter().map(|l| l.mi).collect();
    lens.sort_by(|a, b| a.total_cmp(b));
    // (Each link's cells walked: the miles within 1 mi of the shore, half a cell short of the
    // nearest land cell's centre, past 3 mi from either end.)
    let (mut near, mut open) = (0.0, 0.0);
    for l in &links {
        let c = &l.cells;
        let at = |k: u32| [(k as usize % w) as f64, (k as usize / w) as f64];
        let steps: Vec<f64> = c.windows(2).map(|q| { let (a, b) = (at(q[0]), at(q[1])); ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() * mi }).collect();
        let mut from = 0.0;
        for (i, &s) in steps.iter().enumerate() {
            let mid = from + 0.5 * s;
            from += s;
            if mid < 3.0 || l.mi - mid < 3.0 {
                continue;
            }
            let k = c[i + 1] as usize;
            open += s;
            if t0.coast.data[k] as f64 - 0.5 * cell <= 5280.0 || land[k] {
                near += s;
            }
        }
    }
    let ports: Vec<usize> = (0..t0.settlements.len()).filter(|&i| { let s = &t0.settlements[i]; s.coastal && s.tier >= worldgen::t0::settle::Tier::Town }).collect();
    let linked = |i: usize| lanes.iter().any(|l| l.ends.contains(&(i as u32)));
    let island_ports: Vec<usize> = ports.iter().copied().filter(|&i| isl.island_at(t0.settlements[i].cell).is_some()).collect();
    let trade = |set: &[usize]| {
        let v: Vec<f32> = extra.economy.places.iter().filter(|p| set.contains(&p.index)).map(|p| p.trade).collect();
        if v.is_empty() { f32::NAN } else { v.iter().sum::<f32>() / v.len() as f32 }
    };
    let mainland: Vec<usize> = ports.iter().copied().filter(|i| !island_ports.contains(i)).collect();
    let q = |f: f64| lens.get(((lens.len() as f64 - 1.0) * f).round() as usize).copied().unwrap_or(0.0);
    let straight: f64 = links.iter().map(|l| { let (a, b) = (&t0.settlements[l.ports[0] as usize], &t0.settlements[l.ports[1] as usize]); ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt() / 5280.0 }).sum();
    let points: usize = lanes.iter().map(|l| l.pts.len()).sum();
    {
        let shortest = |l: &[sealanes::SeaLane]| l.iter().map(|l| length(&l.pts)).fold(f64::MAX, f64::min);
        let mut before = sealanes::lanes_with(&extra.economy.sea_links, &extra.settlements, &land, w, h, cell, false);
        sealanes::clip_ports(&mut before, &|x, y| t0.sample_water(x, y) as f64 > t0.ground_at(x, y, world_lattice));
        for l in lanes.iter().filter(|l| length(&l.pts) < 5.0) {
            say!("    short {:.1} mi ends {:?} pts {:?}", length(&l.pts), l.ends.map(|e| e as i64), l.pts);
        }
        say!("  sea lanes shortest segment {:.1} mi before stub merge ({} segments) -> {:.1} mi after ({})", shortest(&before), before.len(), shortest(lanes), lanes.len());
    }
    say!("  sea lanes JSON: {} points, {:.1} KB", points, sealanes::overlay_json(lanes).len() as f64 / 1024.0);
    say!("  sea lanes: {} segments, {total:.0} mi (busy {} / {:.0} mi, normal {} / {:.0} mi, quiet {} / {:.0} mi); links {} (of {}), length min/median/max {:.1}/{:.1}/{:.1} mi, routed/straight {:.2}; within 1 mi of land (past 3 mi from the ports) {:.1}%; points on land {on_land}; island ports linked {}/{}; trade island ports {:.2}, mainland ports {:.2}",
        lanes.len(), by[0].0, by[0].1, by[1].0, by[1].1, by[2].0, by[2].1, links.len(), extra.economy.sea_links.len(), q(0.0), q(0.5), q(1.0), lens.iter().sum::<f64>() / straight.max(1e-9), 100.0 * near / open.max(1e-9),
        island_ports.iter().filter(|&&i| linked(i)).count(), island_ports.len(), trade(&island_ports), trade(&mainland));
    lane_berth(t0, &land);
    for &i in island_ports.iter().filter(|&&i| !linked(i)) {
        let s = &t0.settlements[i];
        let others = ports.iter().filter(|&&j| j != i && isl.island_at(t0.settlements[j].cell) == isl.island_at(s.cell)).count();
        say!("      island port with no lane: {} at ({:.0}, {:.0}){}, {others} other ports on its island", s.tier.name(), s.x, s.y, if s.isolated { " (realm)" } else { "" });
    }
}

/// The lanes' berth and approaches: the share of lane length within 5 mi of land outside 10 mi of
/// every port, and the near-parallel strands (two segments 0.2-1.5 mi apart for over 2 mi, sampled
/// every 0.1 mi along the first: closer, they are drawn as one; not by an end they share).
pub(crate) fn lane_berth(t0: &T0, land: &[bool]) {
    use worldgen::t0::sealanes::JUNCTION;
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let lanes = &t0.sea_lanes;
    let d = |a: P, b: P| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let ports: Vec<P> = lanes.iter().flat_map(|l| l.ends.iter().filter(|&&e| e != JUNCTION).map(|&e| [t0.settlements[e as usize].x, t0.settlements[e as usize].y])).collect();
    let samples = |pts: &[P], step: f64| {
        let mut v = Vec::new();
        for q in pts.windows(2) {
            let l = d(q[0], q[1]);
            let n = (l / step).ceil().max(1.0) as usize;
            for k in 0..n {
                let t = k as f64 / n as f64;
                v.push(([q[0][0] + (q[1][0] - q[0][0]) * t, q[0][1] + (q[1][1] - q[0][1]) * t], l / n as f64));
            }
        }
        v
    };
    let (mut near, mut open) = (0.0, 0.0);
    for l in lanes {
        for (p, s) in samples(&l.pts, 264.0) {
            if ports.iter().any(|&q| d(p, q) < 10.0 * 5280.0) {
                continue;
            }
            let k = ((p[1] / cell).round() as usize).min(h - 1) * w + ((p[0] / cell).round() as usize).min(w - 1);
            open += s;
            if land[k] || t0.coast.data[k] as f64 - 0.5 * cell <= 5.0 * 5280.0 {
                near += s;
            }
        }
    }
    let seg_d = |p: P, pts: &[P]| {
        pts.windows(2)
            .map(|q| {
                let (dx, dy) = (q[1][0] - q[0][0], q[1][1] - q[0][1]);
                let t = (((p[0] - q[0][0]) * dx + (p[1] - q[0][1]) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
                let at = [q[0][0] + dx * t, q[0][1] + dy * t];
                (d(p, at), at)
            })
            .fold((f64::MAX, [0.0; 2]), |a, b| if b.0 < a.0 { b } else { a })
    };
    if let Some(port) = std::env::var("LINK_DUMP").ok().and_then(|v| v.parse::<u32>().ok()) {
        for (li, l) in t0.extra.as_ref().map(|e| &e.economy.sea_links[..]).unwrap_or(&[]).iter().enumerate().filter(|(_, l)| l.ports.contains(&port)) {
            let mut c = l.cells.clone();
            if l.ports[1] == port {
                c.reverse();
            }
            let v: Vec<String> = c.iter().take(14).map(|&k| format!("{},{}", k as usize % w, k as usize / w)).collect();
            say!("    link {li} {:?}: {}", l.ports, v.join(" "));
        }
    }
    if std::env::var("LANE_DUMP").is_ok() {
        for (i, l) in lanes.iter().enumerate() {
            let n = l.pts.len();
            say!("    lane {i}: ends {:?} class {} {:.1} mi ({:.0}, {:.0}) -> ({:.0}, {:.0})", l.ends.map(|e| e as i64), l.class, l.pts.windows(2).map(|q| d(q[0], q[1])).sum::<f64>() / 5280.0, l.pts[0][0], l.pts[0][1], l.pts[n - 1][0], l.pts[n - 1][1]);
        }
    }
    let mut strands = Vec::new();
    for i in 0..lanes.len() {
        let si = samples(&lanes[i].pts, 528.0);
        for j in i + 1..lanes.len() {
            let pj = &lanes[j].pts;
            // (Not where they leave one end together: a fork, if no sharper than ~25 degrees: both
            // the sample and the nearest point on the other over 1.5 mi from it.)
            // (A port end by its settlement: the lane was cut at the waterline short of it.)
            let ends = |l: &worldgen::t0::sealanes::SeaLane| {
                let n = l.pts.len();
                [(l.ends[0], l.pts[0]), (l.ends[1], l.pts[n - 1])].map(|(e, p)| if e == JUNCTION { (e, p) } else { (e, [t0.settlements[e as usize].x, t0.settlements[e as usize].y]) })
            };
            let shared: Vec<P> = ends(&lanes[i]).into_iter().filter(|&(e, p)| ends(&lanes[j]).iter().any(|&(f, q)| e == f && d(p, q) < 100.0)).map(|v| v.1).collect();
            let (mut run, mut best, mut at) = (0.0, 0.0, [0.0; 2]);
            for &(p, s) in &si {
                let (dq, q) = seg_d(p, pj);
                if (0.2 * 5280.0..1.5 * 5280.0).contains(&dq) && !shared.iter().any(|&e| d(p, e) < 1.5 * 5280.0 || d(q, e) < 1.5 * 5280.0) {
                    run += s;
                    if run > best {
                        (best, at) = (run, p);
                    }
                } else {
                    run = 0.0;
                }
            }
            if best > 2.0 * 5280.0 {
                strands.push(format!("{i}/{j} {:.1} mi to ({:.0}, {:.0})", best / 5280.0, at[0], at[1]));
            }
        }
    }
    say!("  sea lane berth: within 5 mi of land past 10 mi from ports {:.1}% of {:.0} mi; near-parallel strands {}: {}", 100.0 * near / open.max(1e-9), open / 5280.0, strands.len(), strands.join("; "));
}
