//! Roofs QA (`town::roof`): per culture, the share of each roof material and shape over its 4
//! biggest settlements in each of seeds 1-8 (roofed buildings; battlemented ones counted apart), and
//! the counts of flat roofs, domes and smoke-darkened ones, with the mean weathering.
//! `cargo run --profile fast -p worldgen --example roofcheck`
use worldgen::World;
use worldgen::pipeline::Executor;
use worldgen::town::roof::{self, MATERIALS, SHAPES};
use worldgen::town::{self, Structure};
use worldgen::world::WorldFile;

const CULTURES: [&str; 5] = ["north", "west", "south", "dry", "steppe"];

fn main() {
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
