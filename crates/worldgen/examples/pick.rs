//! Dev tool: what `pick` finds at a point (one line a thing: kind, id, name, town, removable).
//! `cargo run --profile fast -p worldgen --example pick -- <seed|world.json> [x-ft y-ft] [radius-ft]`
//! Without a point: at the first drawn building, the first settlement's centre and one of its buildings, and a point on the
//! first world road stretch.
mod common;

use worldgen::pipeline::Executor;

fn main() {
    let a = common::args();
    let ex = Executor::new(common::world(a.get(0).unwrap_or("1")));
    let (world, t0) = (&ex.world, &ex.t0);
    let radius = a.num(3, 15.0);
    let mut at: Vec<(String, [f64; 2])> = Vec::new();
    if a.get(2).is_some() {
        at.push(("point".into(), [a.num(1, 0.0), a.num(2, 0.0)]));
    } else {
        if let Some(c) = world.file.edits.created.iter().find(|c| c.kind == "building" && !c.removed) {
            at.push((format!("drawn building {}", c.id), [c.x, c.y]));
        }
        if let Some(s) = t0.settlements.first() {
            at.push(("settlement 0's centre".into(), [s.x, s.y]));
            let l = worldgen::town::layout(world, t0, 0);
            if let Some(b) = l.buildings.get(l.buildings.len() / 2) {
                at.push((format!("building b:0:{}", b.id), worldgen::town::geom::centroid(&b.poly)));
            }
        }
        if let Some(r) = t0.roads.roads.iter().find(|r| r.pts.len() > 8) {
            at.push(("a road".into(), r.pts[r.pts.len() / 2]));
        }
    }
    for (what, p) in at {
        let things = worldgen::pick::pick(world, t0, p, radius);
        println!("{what} at ({:.0}, {:.0}): {} things", p[0], p[1], things.len());
        for t in things.iter().take(8) {
            println!("  {} {} {:?} town={} removable={}", t.kind, t.id.as_deref().unwrap_or("-"), t.name.as_deref().unwrap_or(""), t.town.as_deref().unwrap_or("-"), t.removable);
        }
    }
}
