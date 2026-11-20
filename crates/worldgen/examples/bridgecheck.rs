//! QA: a road drawn by hand and a street added to a generated town, laid across a town's river
//! (along one of its generated bridges), each get a bridge. Prints one line per seed.
//! `cargo run --profile fast -p worldgen --example bridgecheck -- [seeds, default 1..8]`
mod common;

use std::collections::BTreeMap;
use worldgen::model::paths::{Path, PathKind};
use worldgen::pipeline::Executor;
use worldgen::town::{self, geom, wards};
use worldgen::world::{AddedStreet, CrossingKind, TownEdit, World};

fn main() {
    let a = common::args();
    let seeds: Vec<String> = if a.get(0).is_some() { (0..8).filter_map(|i| a.get(i).map(String::from)).collect() } else { (1..=8).map(|s| s.to_string()).collect() };
    for seed in seeds {
        let ex = Executor::new(common::world(&seed));
        let (world, t0) = (&ex.world, &ex.t0);
        let Some((li, deck)) = (0..t0.settlements.len()).find_map(|i| town::layout(world, t0, i).bridges.first().map(|d| (i, d.clone()))) else {
            println!("seed {seed}: no town with a bridge");
            continue;
        };
        let (c, d) = (geom::centroid(&deck), geom::main_axis(&deck));
        let pts: Vec<[f64; 2]> = vec![[c[0] - 150.0 * d[0], c[1] - 150.0 * d[1]], [c[0] + 150.0 * d[0], c[1] + 150.0 * d[1]]];
        let mut line = String::new();
        for kind in PathKind::ALL {
            let p = Path { pts: pts.clone(), kind, width: None, name: None };
            let cr = worldgen::paths::crossings(world, t0, &p);
            line += &format!(" {}:{}", kind.name(), cr.iter().filter(|c| c.kind == CrossingKind::Bridge).count());
        }
        let was = town::layout(world, t0, li).bridges.len();
        let s = &t0.settlements[li];
        let mut file = world.file.clone();
        let streets = BTreeMap::from([(1u32, AddedStreet { pts, kind: PathKind::Paved, width: None })]);
        file.edits.towns.insert(wards::key(li), TownEdit { at: [s.x, s.y], streets, ..Default::default() });
        let w2 = World::new(file).unwrap();
        let now = town::layout(&w2, t0, li).bridges.len();
        println!("seed {seed}: town {li}; hand road bridges{line}; town bridges {was} -> {now} with an added street");
    }
}
