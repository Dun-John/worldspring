//! Dev tool: height profile across a line at a given level, with road and water context.
//! `cargo run --release -p worldgen --example probe -- <seed> <fx> <fy> <dx-ft> <dy-ft> [level] [steps]`
//! Samples from (fx, fy) - (dx, dy) to (fx, fy) + (dx, dy). An 8th argument `river` or `road`
//! centres it on the nearest river or road point instead, across it.
use worldgen::core::tile::{HALO, PADDED, TileKey};
use worldgen::pipeline::Executor;
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |i: usize, d: f64| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let world = World::new(WorldFile { seed: f(1, 1.0) as u32, ..Default::default() }).unwrap();
    let g = world.geom.clone();
    let (cx, cy) = (f(2, 0.5) * g.map_w_ft, f(3, 0.5) * g.map_h_ft);
    let (dx, dy) = (f(4, 0.0), f(5, 300.0));
    let level = f(6, 12.0) as u8;
    let steps = f(7, 24.0) as usize;
    let mut ex = Executor::new(world);
    // `river` as the 8th argument: centre on the nearest river curve point, probe across it.
    let (mut cx, mut cy, mut dx, mut dy) = (cx, cy, dx, dy);
    if a.get(8).is_some_and(|s| s == "river") {
        let mut best = (f64::MAX, [0.0; 2], [0.0; 2], 0.0, 0.0);
        for (ri, k) in ex.t0.rivers.segments_near(cx - 4000.0, cy - 4000.0, cx + 4000.0, cy + 4000.0, 0.0) {
            let r = &ex.t0.rivers.rivers[ri as usize];
            for j in 0..64 {
                let p = r.eval(k as usize, j as f64 / 64.0, 2.5, ex.t0.cell_ft);
                let q = r.eval(k as usize, (j + 1) as f64 / 64.0, 2.5, ex.t0.cell_ft);
                let d = ((p.p[0] - cx).powi(2) + (p.p[1] - cy).powi(2)).sqrt();
                if d < best.0 {
                    best = (d, p.p, [q.p[0] - p.p[0], q.p[1] - p.p[1]], p.z, p.w);
                }
            }
        }
        let l = (best.2[0].powi(2) + best.2[1].powi(2)).sqrt().max(1e-9);
        let half = (dx * dx + dy * dy).sqrt();
        (cx, cy, dx, dy) = (best.1[0], best.1[1], -best.2[1] / l * half, best.2[0] / l * half);
        println!("river point {:.0} ft away at fx {:.6} fy {:.6}: surface z {:.1} ft, width {:.0} ft", best.0, cx / g.map_w_ft, cy / g.map_h_ft, best.3, best.4);
    }
    // `road`: the same across the nearest road.
    if a.get(8).is_some_and(|s| s == "road") {
        let mut best = (f64::MAX, [0.0; 2], [0.0; 2], 0.0, 0usize);
        for (ri, k) in ex.t0.roads.segments_near([cx - 4000.0, cy - 4000.0, cx + 4000.0, cy + 4000.0], 0.0) {
            let r = &ex.t0.roads.roads[ri as usize];
            for j in 0..64 {
                let p = r.eval(k as usize, j as f64 / 64.0, 2.5, ex.t0.cell_ft);
                let q = r.eval(k as usize, (j + 1) as f64 / 64.0, 2.5, ex.t0.cell_ft);
                let d = ((p.p[0] - cx).powi(2) + (p.p[1] - cy).powi(2)).sqrt();
                if d < best.0 {
                    best = (d, p.p, [q.p[0] - p.p[0], q.p[1] - p.p[1]], p.z, ri as usize);
                }
            }
        }
        let l = (best.2[0].powi(2) + best.2[1].powi(2)).sqrt().max(1e-9);
        let half = (dx * dx + dy * dy).sqrt();
        (cx, cy, dx, dy) = (best.1[0], best.1[1], -best.2[1] / l * half, best.2[0] / l * half);
        let lattice = g.spacing_ft(g.first_refine_level - 1);
        let ga = ex.t0.ground_at(best.1[0], best.1[1], lattice);
        println!("road {} point {:.0} ft away at fx {:.6} fy {:.6}: surface z {:.1} ft (planned on {:.1}, ground_at {ga:.1})", best.4, best.0, cx / g.map_w_ft, cy / g.map_h_ft, best.3, ex.t0.rivers.valley(ga, best.1[0], best.1[1]));
    }
    let s = g.spacing_ft(level);
    let size = g.tile_size_ft(level);
    for k in 0..=steps {
        let t = -1.0 + 2.0 * k as f64 / steps as f64;
        let (x, y) = (cx + dx * t, cy + dy * t);
        let key = TileKey::surface(level, (x / size) as u32, (y / size) as u32);
        let (ox, oy) = g.tile_origin_ft(&key);
        let (i, j) = (((x - ox) / s).round() as usize, ((y - oy) / s).round() as usize);
        let tile = ex.terrain(key);
        let h = tile.padded[(j + HALO) * PADDED + i + HALO];
        let rw = tile.river_water[(j + HALO) * PADDED + i + HALO];
        let road = tile.road_mask.get((j + HALO) * PADDED + i + HALO).copied().unwrap_or(0);
        let t0h = ex.t0.sample(x, y, s);
        let (lvl, mask) = ex.t0.sample_lake(x, y);
        println!("{:+7.0} ft  h {h:8.1}  t0 {t0h:8.1}  road {road}  water {lvl:9.1} mask {mask:.2} river {rw:9.1}", t * (dx * dx + dy * dy).sqrt());
    }
}
