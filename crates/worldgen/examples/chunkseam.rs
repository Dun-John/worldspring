//! Dev tool: compare battlemap heights across a chunk boundary (seam diagnosis).
//! `cargo run --release -p worldgen --example chunkseam -- <seed> <fx> <fy>`
//! `... -- <seed> at <x ft> <y ft> [r]`: every shared edge of the (2r+1)² chunks round the spot,
//! each side's halo against the neighbour's own squares (height, road cover, road marks).
//! `RUG=r`: the world's ruggedness. `ROW=y [COL=tx]`: also one row of that chunk (square:surface:height:road mark).

use worldgen::battlemap::SQ;
use worldgen::core::tile::TileKey;
use worldgen::pipeline::Executor;
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(99);
    if a.get(2).map(|s| s.as_str()) == Some("at") {
        let num = |i: usize, d: f64| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
        return around(seed, num(3, 0.0), num(4, 0.0), num(5, 1.0) as i64);
    }
    let fx: f64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.3666);
    let fy: f64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.8357);
    let world = World::new(file(seed)).unwrap();
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let size = g.tile_size_ft(g.max_level);
    let (x, y) = ((fx * g.map_w_ft / size) as u32, (fy * g.map_h_ft / size) as u32);
    let top = ex.battlemap(TileKey::surface(g.max_level, x, y));
    let bot = ex.battlemap(TileKey::surface(g.max_level, x, y + 1));
    let (mut worst, mut worst_i, mut inner) = (0f32, 0, 0f32);
    for i in 0..SQ {
        let d = (top.height[(SQ - 1) * SQ + i] - bot.height[i]).abs();
        if d > worst {
            worst = d;
            worst_i = i;
        }
        inner = inner.max((top.height[(SQ - 2) * SQ + i] - top.height[(SQ - 1) * SQ + i]).abs());
    }
    println!("max jump across boundary {worst:.2} ft at column {worst_i}; max jump between the two rows inside {inner:.2} ft");
    let i = worst_i;
    for r in (SQ - 4)..SQ {
        print!("{:.1} ", top.height[r * SQ + i]);
    }
    print!("| ");
    for r in 0..4 {
        print!("{:.1} ", bot.height[r * SQ + i]);
    }
    println!();
}

/// The halo ring of each chunk against its neighbours' own squares round (x, y) ft.
fn around(seed: u32, x: f64, y: f64, r: i64) {
    use worldgen::battlemap::HS;
    let world = World::new(file(seed)).unwrap();
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let size = g.tile_size_ft(g.max_level);
    let (cx, cy) = ((x / size) as i64, (y / size) as i64);
    let mut chunks = std::collections::HashMap::new();
    for ty in cy - r..=cy + r {
        for tx in cx - r..=cx + r {
            chunks.insert((tx, ty), ex.battlemap(TileKey::surface(g.max_level, tx as u32, ty as u32)));
        }
    }
    // (`ROW=y`: the centre chunk's squares along that row, surface and height.)
    if let Some(ry) = std::env::var("ROW").ok().and_then(|v| v.parse::<f64>().ok()) {
        let ty = (ry / size) as i64;
        let tx = std::env::var("COL").ok().and_then(|v| v.parse::<i64>().ok()).unwrap_or(cx);
        let c = &chunks[&(tx, ty)];
        let j = ((ry - ty as f64 * size) / 5.0) as usize;
        for i in 0..SQ {
            print!("{}:{:?}:{:.1}:{} ", i, c.surface[j * SQ + i], c.height[j * SQ + i], c.road_halo[(j + 1) * HS + i + 1]);
        }
        println!();
    }
    // (Halo square (i, j) of the HS grid is square (i - 1, j - 1) of the chunk.)
    let own = |c: &worldgen::battlemap::Chunk, i: usize, j: usize| c.height[j * SQ + i];
    let (mut dh, mut dhin, mut dc, mut dm, mut n) = (0f32, 0f32, 0i32, 0usize, 0usize);
    let mut at = String::new();
    for (&(tx, ty), a) in &chunks {
        // Own squares in the halo grid match `height`.
        for j in 0..SQ {
            for i in 0..SQ {
                dhin = dhin.max((a.halo_height[(j + 1) * HS + i + 1] - own(a, i, j)).abs());
            }
        }
        for (ox, oy) in [(1i64, 0i64), (0, 1)] {
            let Some(b) = chunks.get(&(tx + ox, ty + oy)) else { continue };
            for k in 0..SQ {
                // A's halo past its east/south edge against B's first column/row, and B's halo
                // before its west/north edge against A's last.
                let pairs = if ox == 1 {
                    [((k + 1) * HS + HS - 1, own(b, 0, k), (k + 1) * HS + 1), ((k + 1) * HS, own(a, SQ - 1, k), (k + 1) * HS + HS - 2)]
                } else {
                    [((HS - 1) * HS + k + 1, own(b, k, 0), HS + k + 1), (k + 1, own(a, k, SQ - 1), (HS - 2) * HS + k + 1)]
                };
                for (side, (h_halo, h_own, h_twin)) in pairs.into_iter().enumerate() {
                    let (halo, other) = if side == 0 { (a, b) } else { (b, a) };
                    n += 1;
                    let d = (halo.halo_height[h_halo] - h_own).abs();
                    let dcv = (halo.road_cover[h_halo] as i32 - other.road_cover[h_twin] as i32).abs();
                    let dmk = halo.road_halo[h_halo] != other.road_halo[h_twin];
                    if d > dh {
                        dh = d;
                        at = format!("chunk ({tx}, {ty}) {} k {k} side {side}: halo {:.2} own {:.2}", if ox == 1 { "east" } else { "south" }, halo.halo_height[h_halo], h_own);
                    }
                    dc = dc.max(dcv);
                    dm += dmk as usize;
                }
            }
        }
    }
    println!("(map {:.0} x {:.0} ft, chunk {size:.0} ft)", g.map_w_ft, g.map_h_ft);
    println!("{n} shared squares: height halo vs own max {dh:.3} ft ({at}); road cover max diff {dc}; road marks differing {dm}; own squares in the halo grid vs height max {dhin:.3}");
}

fn file(seed: u32) -> WorldFile {
    let mut f = WorldFile { seed, ..Default::default() };
    if let Some(r) = std::env::var("RUG").ok().and_then(|v| v.parse().ok()) {
        f.params.ruggedness = r;
    }
    f
}
