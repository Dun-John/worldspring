//! QA: waterfalls (`lod::rivers::Fall`). For seeds 1-8 (or the seeds given): how many falls, their
//! drops, how many hide a cave, how many lie within a settlement's reach; then at the finest level
//! for the first few falls (`N=`, default 3): the river's water and the ground just above the lip,
//! at the face's foot and below it, and the ground at the cave's mouth (dry, not under the water).
//! `SCARP=<fall id>`: instead, the ground beside that fall by distance from the bank and along the flow.
//! `cargo run --release -p worldgen --example fallcheck -- [seeds]`

use worldgen::World;
use worldgen::core::tile::TileKey;
use worldgen::lod::terrain_refine::padded_index;
use worldgen::pipeline::Executor;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let seeds: Vec<u32> = if a.is_empty() { (1..=8).collect() } else { a.iter().filter_map(|s| s.parse().ok()).collect() };
    let shown: usize = std::env::var("N").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    for seed in seeds {
        let world = World::new(worldgen::WorldFile { seed, ..Default::default() }).unwrap();
        let t0 = std::time::Instant::now();
        let mut ex = Executor::new(world);
        let ms = t0.elapsed().as_millis();
        let falls = ex.t0.rivers.falls.clone();
        let drops: Vec<f32> = falls.iter().map(|f| f.drop_ft()).collect();
        let (lo, hi) = drops.iter().fold((f32::MAX, 0f32), |(l, h), &d| (l.min(d), h.max(d)));
        let caves = falls.iter().filter(|f| f.cave.is_some()).count();
        let near_town = falls.iter().filter(|f| ex.t0.settlements.iter().any(|s| (s.x - f.at[0]).hypot(s.y - f.at[1]) < worldgen::t0::radii::pad_extent(worldgen::t0::radii::reach(s)))).count();
        let cave_pois = (0..ex.t0.pois.len()).filter(|&i| ex.t0.fall_of_poi(i).is_some()).count();
        let mut sorted = drops.clone();
        sorted.sort_by(f32::total_cmp);
        let med = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
        let (dropped, caveless) = (ex.t0.rivers.dropped_falls.len(), ex.t0.rivers.caveless_falls.len());
        println!("seed {seed}: {} falls (drop {lo:.0}-{hi:.0} ft, median {med:.0}), {caves} caves ({cave_pois} cave sites), {near_town} within a town's pad; {dropped} dropped by towns and roads, {caveless} caves drowned | T0 {ms} ms", falls.len());
        if std::env::var("ALL").is_ok() {
            // Every cave: walked to from its chunk's edge (or a big stretch of ground)?
            let mut bad = Vec::new();
            let mut least = usize::MAX;
            for f in falls.iter().filter(|f| f.cave.is_some()) {
                let (reached, edge, wet, _) = reach(&mut ex, f.cave.unwrap());
                least = least.min(reached);
                if !edge || wet {
                    bad.push(format!("fl:{} ({reached} sq, wet {wet})", f.id));
                }
            }
            println!("  all {caves} caves: least reached {least} squares; not to the edge: {}", if bad.is_empty() { "none".into() } else { bad.join(", ") });
        }
        let g = ex.world.geom.clone();
        let level = g.max_level;
        let s = g.spacing_ft(level);
        let size = g.tile_size_ft(level);
        let sample = |ex: &mut Executor, x: f64, y: f64| {
            let key = TileKey::surface(level, (x / size) as u32, (y / size) as u32);
            let (ox, oy) = g.tile_origin_ft(&key);
            let t = ex.terrain(key);
            let k = padded_index(((x - ox) / s).round() as i64, ((y - oy) / s).round() as i64);
            (t.padded[k], t.river_water[k])
        };
        if let Ok(id) = std::env::var("SCARP") {
            // The ground across the land beside a fall (left side), by distance from the bank and
            // along the flow: a straight scarp shows as a step at every distance.
            let f = falls[id.parse::<usize>().unwrap_or(0)];
            let (face, half) = (f.face_ft as f64, 0.5 * f.width as f64);
            println!("  fall {} drop {:.0} face {face:.0} width {:.0}; rows: ft beyond the bank, cols: ft along the flow from the lip", f.id, f.drop_ft(), f.width);
            let us = [-200.0, -60.0, -20.0, 0.0, face, face + 20.0, face + 60.0, face + 200.0];
            println!("  {:>6}{}", "", us.iter().map(|u| format!("{u:>7.0}")).collect::<String>());
            for e in [5.0, 20.0, 50.0, 100.0, 200.0, 400.0, 700.0] {
                let mut row = String::new();
                for &u in &us {
                    let o = half + e;
                    let (x, y) = (f.at[0] + f.dir[0] * u - f.dir[1] * o, f.at[1] + f.dir[1] * u + f.dir[0] * o);
                    row += &format!("{:>7.0}", sample(&mut ex, x, y).0);
                }
                println!("  {e:>6.0}{row}");
            }
            continue;
        }
        let mut picks: Vec<&worldgen::lod::rivers::Fall> = falls.iter().filter(|f| f.cave.is_some()).take(shown.div_ceil(2)).collect();
        picks.extend(falls.iter().filter(|f| f.cave.is_none()).take(shown / 2));
        for f in picks {
            let at = |d: f64| [f.at[0] + f.dir[0] * d, f.at[1] + f.dir[1] * d];
            let face = f.face_ft as f64;
            let mut row = String::new();
            for (name, d) in [("above", -15.0), ("lip", -2.0), ("foot", face + 3.0), ("below", face + 30.0)] {
                let p = at(d);
                let (h, w) = sample(&mut ex, p[0], p[1]);
                row += &format!(" {name} ground {h:.0} water {}", if w > -1.0e4 { format!("{w:.0}") } else { "-".into() });
            }
            let side = [-f.dir[1], f.dir[0]];
            for (name, d) in [("bank above", -15.0), ("bank below", face + 15.0)] {
                let p = at(d);
                let off = 0.5 * f.width as f64 + 8.0;
                let (l, _) = sample(&mut ex, p[0] + side[0] * off, p[1] + side[1] * off);
                let (r, _) = sample(&mut ex, p[0] - side[0] * off, p[1] - side[1] * off);
                row += &format!(" {name} {l:.0}/{r:.0}");
            }
            if let Some(c) = f.cave {
                let (reached, edge, wet, steep) = reach(&mut ex, c);
                row += &format!(" | cave squares reached {reached} edge {edge} (mouth wet {wet}, hard {steep})");
                let mut sample = |x: f64, y: f64| {
                    let key = TileKey::surface(level, (x / size) as u32, (y / size) as u32);
                    let (ox, oy) = g.tile_origin_ft(&key);
                    let t = ex.terrain(key);
                    let k = padded_index(((x - ox) / s).round() as i64, ((y - oy) / s).round() as i64);
                    (t.padded[k], t.river_water[k])
                };
                let (h, w) = sample(c[0], c[1]);
                row += &format!(" | cave ({:.0}, {:.0}) ground {h:.0} water {}", c[0], c[1], if w > -1.0e4 { format!("{w:.0}") } else { "-".into() });
            }
            println!("  fall {} at ({:.0}, {:.0}) dir ({:.2}, {:.2}) top {:.0} bottom {:.0} width {:.0} face {:.0}:{row}", f.id, f.at[0], f.at[1], f.dir[0], f.dir[1], f.top, f.bottom, f.width, f.face_ft);
        }
    }
}

/// Battlemap squares walked to from a cave's mouth in its chunk (square to square, no water, no
/// step over 5 ft between squares), whether that reaches the chunk's edge, and the mouth's own.
fn reach(ex: &mut Executor, c: [f64; 2]) -> (usize, bool, bool, bool) {
    let g = ex.world.geom.clone();
    let size = g.tile_size_ft(g.max_level);
    let key = TileKey::surface(g.max_level, (c[0] / size) as u32, (c[1] / size) as u32);
    let (ox, oy) = g.tile_origin_ft(&key);
    let ch = ex.battlemap(key);
    let sq = worldgen::battlemap::SQ;
    let ft = size / sq as f64;
    let (i0, j0) = (((c[0] - ox) / ft) as usize, ((c[1] - oy) / ft) as usize);
    let start = j0.min(sq - 1) * sq + i0.min(sq - 1);
    let wet = |k: usize| ch.water_level[k] > ch.height[k];
    let open = |k: usize| !wet(k);
    let mut seen = vec![false; sq * sq];
    let mut stack = vec![start];
    seen[start] = true;
    let (mut n, mut edge) = (0, false);
    while let Some(k) = stack.pop() {
        n += 1;
        let (i, j) = (k % sq, k / sq);
        edge |= i == 0 || j == 0 || i + 1 == sq || j + 1 == sq;
        let h = ch.height[k];
        let mut go = |q: usize, _: bool| {
            if !seen[q] && (ch.height[q] - h).abs() <= 5.0 && open(q) {
                seen[q] = true;
                stack.push(q);
            }
        };
        if i + 1 < sq {
            go(k + 1, ch.edges[k] & 1 != 0);
        }
        if i > 0 {
            go(k - 1, ch.edges[k - 1] & 1 != 0);
        }
        if j + 1 < sq {
            go(k + sq, ch.edges[k] & 2 != 0);
        }
        if j > 0 {
            go(k - sq, ch.edges[k - sq] & 2 != 0);
        }
    }
    (n, edge, wet(start), ch.edges[start] & 16 != 0)
}
