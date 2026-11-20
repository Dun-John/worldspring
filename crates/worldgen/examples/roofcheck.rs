//! Roofs QA (`town::roof`): per culture, the share of each roof material and shape over its 4
//! biggest settlements in each of seeds 1-8 (roofed buildings; battlemented ones counted apart), and
//! the counts of flat roofs, domes and smoke-darkened ones, with the mean weathering.
//! `cargo run --profile fast -p worldgen --example roofcheck` (`-- sample`: per culture, the seed
//! of its biggest town and, per roof shape and material, where one of them stands as map fractions for a battlemap
//! view, `fx`/`fy`).
use worldgen::World;
use worldgen::pipeline::Executor;
use worldgen::town::roof::{self, MATERIALS, SHAPES};
use worldgen::town::{self, Structure};
use worldgen::world::WorldFile;

const CULTURES: [&str; 5] = ["north", "west", "south", "dry", "steppe"];

fn main() {
    if std::env::args().any(|a| a == "sample") {
        return sample();
    }
    if std::env::args().any(|a| a == "taverns") {
        return taverns();
    }
    let mut mat = [[0u32; MATERIALS.len()]; 5];
    let mut shp = [[0u32; SHAPES.len()]; 5];
    let (mut n, mut bat, mut smoke, mut wear, mut towns) = ([0u32; 5], [0u32; 5], [0u32; 5], [0u64; 5], [0u32; 5]);
    for seed in 1..=8 {
        let ex = Executor::new(World::new(WorldFile { seed, ..Default::default() }).unwrap());
        let (world, t0) = (&ex.world, &ex.t0);
        let mut by_pop: Vec<usize> = (0..t0.settlements.len()).collect();
        by_pop.sort_by_key(|&i| (std::cmp::Reverse(t0.settlements[i].population), i));
        let mut taken = [0u32; 5];
        for &si in &by_pop {
            let c = (t0.settlements[si].culture % 5) as usize;
            if taken[c] == 4 {
                continue;
            }
            taken[c] += 1;
            let l = town::layout(world, t0, si);
            towns[c] += 1;
            for b in l.buildings.iter().filter(|b| b.structure == Structure::Roofed) {
                if worldgen::interior::battlements(b).is_some() {
                    bat[c] += 1;
                    continue;
                }
                let k = roof::look(&l, b);
                n[c] += 1;
                mat[c][k.material as usize] += 1;
                shp[c][k.shape as usize] += 1;
                smoke[c] += k.smoke as u32;
                wear[c] += k.wear as u64;
            }
        }
    }
    let pct = |a: u32, t: u32| if t == 0 { 0.0 } else { 100.0 * a as f64 / t as f64 };
    for c in 0..5 {
        if n[c] == 0 {
            println!("{:<7} (no towns)", CULTURES[c]);
            continue;
        }
        let mats: Vec<String> = (0..MATERIALS.len()).filter(|&m| mat[c][m] > 0).map(|m| format!("{} {:.0}%", MATERIALS[m], pct(mat[c][m], n[c]))).collect();
        let shps: Vec<String> = (0..SHAPES.len()).filter(|&s| shp[c][s] > 0).map(|s| format!("{} {:.0}%", SHAPES[s], pct(shp[c][s], n[c]))).collect();
        println!("{:<7} towns {:>2} roofs {:>5} (+{} battlements)  wear {:.0}  smoke {}", CULTURES[c], towns[c], n[c], bat[c], wear[c] as f64 / n[c] as f64, smoke[c]);
        println!("        material: {}", mats.join(", "));
        println!("        shape:    {}  (flat {}, domes {})", shps.join(", "), shp[c][4], shp[c][3]);
    }
}

fn sample() {
    let mut done = [false; 5];
    for seed in 1..=8 {
        let ex = Executor::new(World::new(WorldFile { seed, ..Default::default() }).unwrap());
        let (world, t0) = (&ex.world, &ex.t0);
        let (w, h) = (world.geom.map_w_ft, world.geom.map_h_ft);
        let mut by_pop: Vec<usize> = (0..t0.settlements.len()).collect();
        by_pop.sort_by_key(|&i| (std::cmp::Reverse(t0.settlements[i].population), i));
        for &si in &by_pop {
            let c = (t0.settlements[si].culture % 5) as usize;
            if done[c] {
                continue;
            }
            done[c] = true;
            let l = town::layout(world, t0, si);
            let s = &t0.settlements[si];
            println!("{:<7} seed {} pop {} centre fx {:.6} fy {:.6}", CULTURES[c], seed, s.population, s.x / w, s.y / h);
            let (mut seen, mut seen_m, mut seen_smoke) = ([false; SHAPES.len()], [false; MATERIALS.len()], false);
            for b in l.buildings.iter().filter(|b| b.structure == Structure::Roofed && worldgen::interior::battlements(b).is_none()) {
                let k = roof::look(&l, b);
                let new_smoke = k.smoke && !seen_smoke;
                if (seen[k.shape as usize] && seen_m[k.material as usize] && !new_smoke) || b.poly.is_empty() {
                    continue;
                }
                (seen[k.shape as usize], seen_m[k.material as usize], seen_smoke) = (true, true, seen_smoke || k.smoke);
                let n = b.poly.len() as f64;
                let (x, y) = (b.poly.iter().map(|p| p[0]).sum::<f64>() / n, b.poly.iter().map(|p| p[1]).sum::<f64>() / n);
                println!("        {:<6} {:<12} wear {:>3}{} fx {:.6} fy {:.6}", SHAPES[k.shape as usize], MATERIALS[k.material as usize], k.wear, if k.smoke { " smoke" } else { "" }, x / w, y / h);
            }
        }
        if done.iter().all(|&d| d) {
            break;
        }
    }
}

/// (Roadmap 7, 164) Taverns, inns and alehouses per culture over the same towns: their roof shapes and
/// materials and their forms (a coaching inn round its yard, an L / U / court, a felt hall, a plain one),
/// and any not hipped or gabled listed (`-- taverns`).
fn taverns() {
    let mut shp = [[0u32; SHAPES.len()]; 5];
    let mut mat = [[0u32; MATERIALS.len()]; 5];
    let mut form = [[0u32; 6]; 5];
    let mut odd: Vec<String> = Vec::new();
    let mut first: [Option<String>; 5] = Default::default();
    for seed in 1..=8 {
        let ex = Executor::new(World::new(WorldFile { seed, ..Default::default() }).unwrap());
        let (world, t0) = (&ex.world, &ex.t0);
        let mut by_pop: Vec<usize> = (0..t0.settlements.len()).collect();
        by_pop.sort_by_key(|&i| (std::cmp::Reverse(t0.settlements[i].population), i));
        let mut taken = [0u32; 5];
        for &si in &by_pop {
            let c = (t0.settlements[si].culture % 5) as usize;
            if taken[c] == 4 {
                continue;
            }
            taken[c] += 1;
            let l = town::layout(world, t0, si);
            for b in l.buildings.iter().filter(|b| b.structure == Structure::Roofed) {
                let Some(key) = b.func.map(|f| town::catalog::CATALOG[f as usize].key) else { continue };
                if !matches!(key, "inn" | "tavern" | "alehouse") {
                    continue;
                }
                let k = roof::look(&l, b);
                shp[c][k.shape as usize] += 1;
                mat[c][k.material as usize] += 1;
                let f = if town::palace::is_coaching(&l, b) {
                    0
                } else if let Some(r) = town::wings::ranges(&l, b) {
                    r.len().min(4) - 1
                } else if roof::hall_ring(&l, b).is_some() {
                    4
                } else {
                    5
                };
                form[c][f] += 1;
                if first[c].is_none() && f != 4 && key == "inn" {
                    let m = town::geom::centroid(&b.poly);
                    first[c] = Some(format!("s{seed} L{si} b{} {} {} fx {:.6} fy {:.6}", b.id, SHAPES[k.shape as usize], MATERIALS[k.material as usize], m[0] / world.geom.map_w_ft, m[1] / world.geom.map_h_ft));
                }
                if !matches!(k.shape, roof::Shape::Hip | roof::Shape::Gable) && f != 4 && odd.len() < 12 {
                    let o = town::geom::obb(&b.poly);
                    odd.push(format!("s{seed} L{si} b{} {key} {} {} {}x{:.0}x{:.0}", b.id, SHAPES[k.shape as usize], MATERIALS[k.material as usize], b.poly.len(), o.long, o.short));
                }
            }
        }
    }
    for c in 0..5 {
        let n: u32 = shp[c].iter().sum();
        let shps: Vec<String> = (0..SHAPES.len()).filter(|&s| shp[c][s] > 0).map(|s| format!("{} {}", SHAPES[s], shp[c][s])).collect();
        let mats: Vec<String> = (0..MATERIALS.len()).filter(|&m| mat[c][m] > 0).map(|m| format!("{} {}", MATERIALS[m], mat[c][m])).collect();
        let f = form[c];
        println!("{:<7} {:>3}  shape: {}", CULTURES[c], n, shps.join(", "));
        println!("             material: {}", mats.join(", "));
        println!("             coaching {} L {} U {} court {} felt hall {} plain {}", f[0], f[1], f[2], f[3], f[4], f[5]);
    }
    for (c, f) in first.iter().enumerate() {
        println!("  first inn, {}: {}", CULTURES[c], f.as_deref().unwrap_or("-"));
    }
    for o in &odd {
        println!("  {o}");
    }
}
