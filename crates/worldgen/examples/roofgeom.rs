//! Roof geometry QA (dry-culture towns): dumps, for seeds 1-8 (the 4 biggest dry towns of each, and
//! the default world's Ain Keshir), every roofed building's outline (ft), roof shape and the domes
//! its roof terrace keeps (`interior::terrace`), as JSON for `scripts/roofgeom.mjs`, which draws them
//! with the app's roof maths and counts the faults. `-- find <name>`: where a settlement of the
//! default world is (map fractions). `-- all`: the 2 biggest towns of every other culture instead.
//! `cargo run --profile fast -p worldgen --example roofgeom > out.json`, then `node scripts/roofgeom.mjs out.json`.
use worldgen::town::{self, Structure};
use worldgen::world::WorldFile;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let find = args.iter().position(|a| a == "find").map(|i| args[i + 1].clone());
    let mut out = Vec::new();
    for seed in 1..=8u32 {
        if find.is_some() && seed > 1 {
            break;
        }
        let world = worldgen::World::new(WorldFile { seed, ..Default::default() }).unwrap();
        let t0 = worldgen::t0::T0::generate(&world);
        let (w, h) = (world.geom.map_w_ft, world.geom.map_h_ft);
        let feats = &t0.extra.as_ref().expect("generated").overlay.features;
        let near = |x: f64, y: f64| (0..t0.settlements.len()).min_by(|&a, &b| {
            let d = |i: usize| (t0.settlements[i].x - x).powi(2) + (t0.settlements[i].y - y).powi(2);
            d(a).total_cmp(&d(b))
        });
        let ain = feats.iter().find(|f| f.name == "Ain Keshir").and_then(|f| near(f.x, f.y));
        if let Some(name) = &find {
            for f in feats.iter().filter(|f| &f.name == name) {
                let si = near(f.x, f.y).unwrap();
                let s = &t0.settlements[si];
                println!("{} {} settlement {si} culture {} pop {} fx {:.6} fy {:.6}", f.kind, f.name, s.culture % 5, s.population, s.x / w, s.y / h);
            }
            return;
        }
        let all = args.iter().any(|a| a == "all");
        let mut dry: Vec<usize> = (0..t0.settlements.len()).filter(|&i| all || t0.settlements[i].culture % 5 == 3).collect();
        dry.sort_by_key(|&i| (std::cmp::Reverse(t0.settlements[i].population), i));
        if all {
            let mut taken = [0; 5];
            dry.retain(|&i| {
                let c = (t0.settlements[i].culture % 5) as usize;
                taken[c] += 1;
                taken[c] <= 2 && c != 3
            });
        } else {
            dry.truncate(4);
        }
        if let Some(a) = ain.filter(|a| seed == 1 && !dry.contains(a)) {
            dry.push(a);
        }
        for si in dry {
            let l = town::layout(&world, &t0, si);
            let mut bs = Vec::new();
            for b in l.buildings.iter().filter(|b| b.structure == Structure::Roofed && b.poly.len() >= 3) {
                let k = town::roof::look(&l, b);
                let bat = worldgen::interior::battlements(b).is_some();
                let domes: Vec<[f64; 3]> = worldgen::interior::terrace(&l, b).unwrap_or_default().iter().map(|(p, r)| [p[0], p[1], *r]).collect();
                let poly: Vec<[f64; 2]> = b.poly.iter().map(|p| [p[0], p[1]]).collect();
                bs.push(serde_json::json!({ "id": b.id, "shape": k.shape as u8, "bat": bat, "cone": b.roof == Some(town::RoofStyle::Cone), "parts": b.parts.len(), "poly": poly, "domes": domes }));
            }
            out.push(serde_json::json!({ "seed": seed, "si": si, "ain": Some(si) == ain && seed == 1, "w": w, "h": h, "b": bs }));
        }
    }
    println!("{}", serde_json::Value::Array(out));
}
