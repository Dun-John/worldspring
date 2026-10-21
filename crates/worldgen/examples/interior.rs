//! Dev tool: print building interiors as text (one letter per room, # stairs, @ spiral stairs, *
//! furniture, a great hall's pieces by their own capitals (`MARKS`, listed); a hall's height left
//! open (`open to below`) `:`, a roof's slopes `^`; the railings where a gallery or passage looks
//! down into an open hall listed).
//! `cargo run --release -p worldgen --example interior -- [seed] [settlement] [function...]`
use worldgen::interior;
use worldgen::{World, WorldFile};

/// A great hall's pieces drawn by their own marks (`*` for the rest; a piece on a low dais over it).
const MARKS: &[(&str, char)] = &[
    ("throne_dais", 'T'),
    ("low_dais", 'D'),
    ("chair_of_office", 'C'),
    ("high_table", 'H'),
    ("carpet_runner", '='),
    ("long_table", 'L'),
    ("bench", 'B'),
    ("hearth", 'F'),
    ("pillar", 'O'),
    ("standing_banner", 'S'),
    ("brazier", 'X'),
    ("wall_banner", 'W'),
    ("tapestry", 'Y'),
    ("musicians_corner", 'M'),
    ("throne", '!'),
];

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let t0 = worldgen::t0::T0::generate(&world);
    let si: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| (0..t0.settlements.len()).max_by_key(|&i| t0.settlements[i].population).unwrap());
    let wanted: Vec<String> = a.iter().skip(3).cloned().collect();
    println!("marks: {}", MARKS.iter().map(|m| format!("{} {}", m.1, m.0)).collect::<Vec<_>>().join(", "));
    let l = worldgen::town::layout(&world, &t0, si);
    for want in &wanted {
        let Some(bi) = want.parse::<usize>().ok().or_else(|| l.buildings.iter().find(|b| b.label().to_lowercase() == *want).map(|b| b.id as usize)) else {
            println!("no {want}");
            continue;
        };
        let t = std::time::Instant::now();
        let it = interior::generate(&world, &t0, si, bi).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let outline = if it.outlines.is_empty() { String::new() } else { format!(", drawn inside a {}-sided outline", it.outlines.iter().map(|o| o.len().to_string()).collect::<Vec<_>>().join("+")) };
        let kind = l.building(bi).and_then(|b| interior::hub_kind(&t0, &l, b)).map(|k| format!(", a {}", k.name())).unwrap_or_default();
        println!("== {} {:?} ({}x{} squares, {} levels{outline}{kind}, {ms:.2} ms)", it.function, it.name, it.nx, it.ny, it.levels.len());
        for lv in &it.levels {
            let room = |r: &interior::Room| format!("{}{}={}", r.kind, interior::shape::name(r.shape).map(|s| format!(" ({s})")).unwrap_or_default(), r.squares);
            println!("-- {} (z {}): {}", lv.name, lv.z, lv.rooms.iter().map(room).collect::<Vec<_>>().join(", "));
            let mut grid: Vec<Vec<char>> = (0..it.ny).map(|j| (0..it.nx).map(|i| {
                let c = lv.cells[j * it.nx + i];
                match lv.rooms.get(c.max(0) as usize).map(|r| r.kind) {
                    _ if c < 0 => ' ',
                    Some(interior::OPEN_BELOW) => ':',
                    Some(interior::ROOF_SLOPE) => '^',
                    _ => (b'a' + (c as u8 % 26)) as char,
                }
            }).collect()).collect();
            if std::env::var("NOFURN").is_err() {
                // (A low dais first, what stands on it over it.)
                let mut pieces: Vec<&interior::Item> = lv.furniture.iter().filter(|f| f.kind != "spiral_stair").collect();
                pieces.sort_by_key(|f| f.kind != "low_dais");
                for f in pieces {
                    let mark = MARKS.iter().find(|m| m.0 == f.kind).map_or('*', |m| m.1);
                    for j in f.y..f.y + f.h {
                        for i in f.x..f.x + f.w {
                            grid[j as usize][i as usize] = mark;
                        }
                    }
                }
            }
            let look = |f: &interior::Item| f.variant.map(|v| format!(" v{v}")).unwrap_or_default() + &f.back.map(|b| format!(" {}", ["N", "E", "S", "W"][b as usize % 4])).unwrap_or_default();
            println!("  furniture: {}", lv.furniture.iter().map(|f| format!("{}@{},{} {}x{}{}", f.kind, f.x, f.y, f.w, f.h, look(f))).collect::<Vec<_>>().join(" "));
            for j in it.stairs[1]..it.stairs[1] + it.stairs[3] {
                for i in it.stairs[0]..it.stairs[0] + it.stairs[2] {
                    if lv.has_stairs {
                        grid[j][i] = '#';
                    }
                }
            }
            for f in lv.furniture.iter().filter(|f| f.kind == "spiral_stair") {
                grid[f.y as usize][f.x as usize] = '@';
            }
            for row in grid {
                println!("  {}", row.into_iter().collect::<String>());
            }
            println!("  doors: {}", lv.doors.iter().map(|d| format!("{}@({},{})", d.kind, d.a[0], d.a[1])).collect::<Vec<_>>().join(" "));
            let rails: Vec<String> = lv.walls.iter().filter(|w| w.rail).map(|w| format!("({},{})-({},{})", w.a[0], w.a[1], w.b[0], w.b[1])).collect();
            if !rails.is_empty() {
                println!("  railings: {}", rails.join(" "));
            }
        }
    }
}
