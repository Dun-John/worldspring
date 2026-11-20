//! Scratch: how sheltered the harbours of coastal cities and metropolises are.
//! `cargo run --profile fast -p worldgen --example shelter -- [seed|world.json]...`   (default 1-8)
//! Per city on the coast (water within half a mile): `open`, the share of 48 ways out that
//! find water 1, 2 and 3 mi out (a straight coast about 0.5, a bay its mouth's share); `land`,
//! the share of ways that meet land within 3 mi after leaving the town's water; and `shelter`,
//! the score `settle::verify` gives a harbour (0-1).
mod common;

use worldgen::t0::settle::Tier;
use worldgen::t0::T0;

fn main() {
    let args = common::args();
    let seeds: Vec<String> = if args.pos.is_empty() { (1..=8).map(|s| s.to_string()).collect() } else { args.pos.clone() };
    let (mut all_open, mut all_shelter) = (Vec::new(), Vec::new());
    let (mut inland, mut p_inland) = (0, 0);
    let (mut p_open, mut p_shelter) = (Vec::new(), Vec::new());
    let mut t_total = 0.0;
    for arg in &seeds {
        let world = common::world(arg);
        let t = std::time::Instant::now();
        let t0 = T0::generate(&world);
        let dt = t.elapsed().as_secs_f64();
        t_total += dt;
        let names = t0.extra.as_ref().map(|e| e.overlay.features.as_slice()).unwrap_or(&[]);
        let wet = |x: f64, y: f64| t0.sample_water(x, y) > -29_000.0;
        let ray = |a: usize| {
            let ang = std::f64::consts::TAU * a as f64 / 48.0;
            [libm::cos(ang), libm::sin(ang)]
        };
        println!("seed {arg}: T0 {dt:.2} s");
        for s in t0.settlements.iter().filter(|s| s.tier >= Tier::City || s.planned >= Tier::City) {
            let name = names.iter().find(|f| (f.x - s.x).abs() < 1.0 && (f.y - s.y).abs() < 1.0).map(|f| f.name.clone()).unwrap_or_default();
            let shore = (0..48).any(|a| {
                let u = ray(a);
                (1..=26).any(|t| wet(s.x + u[0] * t as f64 * 100.0, s.y + u[1] * t as f64 * 100.0))
            });
            if !shore {
                inland += 1;
                p_inland += (s.planned >= Tier::City) as i32;
                println!("  {:<16} {:<10} inland ({:?})", name, format!("{:?}", s.tier), s.reason);
                continue;
            }
            let open = (0..48).filter(|&a| {
                let u = ray(a);
                [1.0, 2.0, 3.0].iter().all(|&m| wet(s.x + u[0] * m * 5_280.0, s.y + u[1] * m * 5_280.0))
            }).count() as f64 / 48.0;
            let mile = (0..48).filter(|&a| {
                let u = ray(a);
                wet(s.x + u[0] * 5_280.0, s.y + u[1] * 5_280.0)
            }).count() as f64 / 48.0;
            // Of the ways that reach water, how many meet land again within 3 mi.
            let (mut out, mut back) = (0, 0);
            for a in 0..48 {
                let u = ray(a);
                let mut t = 100.0;
                let mut seen = false;
                let mut hit = false;
                while t <= 3.0 * 5_280.0 {
                    let w = wet(s.x + u[0] * t, s.y + u[1] * t);
                    if w {
                        seen = true;
                    } else if seen {
                        hit = true;
                        break;
                    }
                    t += 200.0;
                }
                if seen {
                    out += 1;
                    back += hit as i32;
                }
            }
            let land = if out > 0 { back as f64 / out as f64 } else { 0.0 };
            let ss = |a: f64, b: f64, x: f64| {
                let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            };
            let shelter = (1.0 - ss(0.2, 0.34, open)).min(1.0 - ss(0.3, 0.4, mile));
            all_open.push(open);
            all_shelter.push(shelter);
            if s.planned >= Tier::City {
                p_open.push(open);
                p_shelter.push(shelter);
            }
            println!("  {:<16} {:<10} open {:.2} mile {:.2} land {:.2} shelter {:.2} ({:?} {:?} planned {:?}) at {:.0},{:.0}", name, format!("{:?}", s.tier), open, mile, land, shelter, s.reason, s.kind, s.planned, s.x, s.y);
        }
    }
    let q = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        if v.is_empty() {
            return String::from("-");
        }
        let at = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
        format!("n {} p10 {:.2} p25 {:.2} med {:.2} p75 {:.2} p90 {:.2}", v.len(), at(0.1), at(0.25), at(0.5), at(0.75), at(0.9))
    };
    let sheltered = all_shelter.iter().filter(|&&s| s >= 0.5).count();
    let bare = all_open.iter().filter(|&&o| o >= 0.34).count();
    println!("SUMMARY coastal {} inland {inland}; sheltered (>=0.5) {sheltered}; open coast (open>=0.34) {bare}; T0 total {t_total:.1} s", all_open.len());
    println!("SUMMARY open    {}", q(&mut all_open));
    println!("SUMMARY shelter {}", q(&mut all_shelter));
    let (ps, po) = (p_shelter.iter().filter(|&&s| s >= 0.5).count(), p_open.iter().filter(|&&o| o >= 0.34).count());
    println!("PLANNED coastal {} inland {p_inland}; sheltered (>=0.5) {ps}; open coast (open>=0.34) {po}", p_open.len());
    let bays = p_open.iter().filter(|&&o| o <= 0.15).count();
    let coast = p_open.iter().filter(|&&o| o >= 0.25).count();
    println!("PLANNED bays (open<=0.15) {bays}, open coast (open>=0.25) {coast}");
    println!("PLANNED open    {}", q(&mut p_open));
    println!("PLANNED shelter {}", q(&mut p_shelter));
}
