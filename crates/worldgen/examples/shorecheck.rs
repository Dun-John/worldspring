//! Check tool (R7 173): the shore wharf brush on a curved coast. Per seed, traces the shore near
//! each coastal settlement, takes the most curved ~500 ft stretch, brushes a stroke 12 ft inland of
//! it with a ±5 ft wobble (`wharf::shore_line`), and reports how steady the quay is: its width
//! (back line to face) and its face's offset out from the water's edge, as max deviations.
//! `cargo run --release -p worldgen --example shorecheck -- [seeds, default 1-8] [width ft, default 20]`
mod common;

use worldgen::core::geom::{add, dist, dot, mul, seg_dist, sub, unit};
use worldgen::ground::wharf::wetness;
use worldgen::pipeline::Executor;
use worldgen::t0::T0;
use worldgen::wharf;
use worldgen::world::{CrossingMaterial, Wharf, WharfKind};

type P = [f64; 2];

fn wet(t0: &T0, p: P) -> bool {
    wetness(t0, p) > 0.0
}

/// The water's edge across `p` along `n` (toward the water) within ±`reach`, to 0.1 ft: the first
/// dry-to-wet step.
fn edge(t0: &T0, p: P, n: P, reach: f64) -> Option<f64> {
    let at = |t: f64| wet(t0, add(p, mul(n, t)));
    let mut t = -reach;
    if at(t) {
        return None;
    }
    while t < reach {
        if at(t + 1.0) {
            let (mut lo, mut hi) = (t, t + 1.0);
            for _ in 0..8 {
                let m = 0.5 * (lo + hi);
                if at(m) { hi = m } else { lo = m }
            }
            return Some(hi);
        }
        t += 1.0;
    }
    None
}

/// The shore traced from edge point `p` with the water on the right of `u`, every 5 ft for `len` ft.
fn trace(t0: &T0, mut p: P, mut u: P, len: f64) -> Option<Vec<P>> {
    let mut out = vec![p];
    while (out.len() as f64 - 1.0) * 5.0 < len {
        let q = add(p, mul(u, 5.0));
        let water = [u[1], -u[0]];
        let t = edge(t0, q, water, 8.0)?;
        let np = add(q, mul(water, t));
        u = unit(sub(np, p));
        p = np;
        out.push(p);
    }
    Some(out)
}

/// Total turning (deg) along `pts` smoothed over ±4 points, and the tightest radius (ft) over 40 ft.
fn turning(pts: &[P]) -> (f64, f64) {
    let (mut tot, mut tight) = (0.0, f64::MAX);
    for k in 8..pts.len().saturating_sub(8) {
        let (a, b) = (unit(sub(pts[k], pts[k - 8])), unit(sub(pts[k + 8], pts[k])));
        let ang = libm::atan2(a[0] * b[1] - a[1] * b[0], dot(a, b)).abs();
        tot += ang / 8.0;
        if ang > 1e-6 {
            tight = tight.min(40.0 / ang);
        }
    }
    (tot.to_degrees(), tight)
}

fn polyline_dist(p: P, line: &[P]) -> f64 {
    line.windows(2).map(|s| seg_dist(p, s[0], s[1])).fold(f64::MAX, f64::min)
}

fn main() {
    let a = common::args();
    let seeds: Vec<u64> = match a.get(0) {
        Some(s) if s.contains('-') => {
            let (x, y) = s.split_once('-').unwrap();
            (x.parse().unwrap()..=y.parse().unwrap()).collect()
        }
        Some(s) => s.split(',').map(|v| v.parse().unwrap()).collect(),
        None => (1..=8).collect(),
    };
    let width = a.num(1, 20.0);
    let mut worst: (f64, f64) = (0.0, 0.0);
    for seed in seeds {
        let ex = Executor::new(common::world(&seed.to_string()));
        let t0 = &ex.t0;
        // (The most curved 500 ft of shore near a coastal settlement: rays out from each to the
        // water, traced along it.)
        let mut best: Option<(f64, f64, Vec<P>, String)> = None;
        for s in t0.settlements.iter().filter(|s| s.coastal) {
            let c = [s.x, s.y];
            for r in 0..12 {
                let ang = r as f64 * std::f64::consts::TAU / 12.0;
                let d = [libm::cos(ang), libm::sin(ang)];
                let Some(t) = edge(t0, c, d, 2000.0).filter(|_| !wet(t0, c)) else { continue };
                let e = add(c, mul(d, t));
                let Some(line) = trace(t0, e, [-d[1], d[0]], 500.0) else { continue };
                let (turn, tight) = turning(&line);
                // (Curved but not torn: no bend tighter than the quay is wide twice over.)
                if tight >= 2.0 * width && best.as_ref().is_none_or(|b| turn > b.0) {
                    best = Some((turn, tight, line, format!("{:?} near ({:.0}, {:.0})", s.tier, e[0], e[1])));
                }
            }
        }
        let Some((turn, tight, line, place)) = best else {
            println!("seed {seed}: no curved coast found");
            continue;
        };
        // (The stroke: 12 ft inland of the shore, wobbling ±5 ft.)
        let normal: Vec<P> = (0..line.len()).map(|k| {
            let u = unit(sub(line[(k + 1).min(line.len() - 1)], line[k.saturating_sub(1)]));
            [-u[1], u[0]]
        }).collect();
        let stroke: Vec<P> = (0..line.len()).step_by(3).map(|k| add(line[k], mul(normal[k], 12.0 + 5.0 * libm::sin(k as f64 * 5.0 / 60.0 * std::f64::consts::TAU)))).collect();
        let pts = match wharf::shore_line(t0, &stroke) {
            Ok(p) => p,
            Err(e) => {
                println!("seed {seed}: {place}: {e}");
                continue;
            }
        };
        let mut w = Wharf::new(WharfKind::Quay, pts, width, CrossingMaterial::Stone);
        w.shore = true;
        w.piers = 80.0;
        if let Some(p) = w.problem(f64::MAX, f64::MAX) {
            println!("seed {seed}: {place}: problem {p}");
            continue;
        }
        let back = wharf::shore_back(&w);
        let normals = wharf::shore_normals(&w.pts);
        let mut along = vec![0.0; w.pts.len()];
        for k in 1..w.pts.len() {
            along[k] = along[k - 1] + dist(w.pts[k - 1], w.pts[k]);
        }
        let run = *along.last().unwrap();
        let reach = w.width.max(10.0).min(0.3 * run);
        let (mut dw, mut folds) = (0.0f64, 0);
        let (mut df, mut sum_f, mut nf) = (0.0f64, 0.0, 0);
        for k in 0..w.pts.len() {
            // (The ends round off over `reach`: the width is measured between them.)
            if along[k] > reach && along[k] < run - reach {
                dw = dw.max((polyline_dist(back[k], &w.pts) - w.width).abs());
                if k > 0 && dot(sub(back[k], back[k - 1]), sub(w.pts[k], w.pts[k - 1])) <= 0.0 {
                    folds += 1;
                }
            }
            // (The face's offset out from the water's edge, along its normal toward the land.)
            if let Some(t) = edge(t0, add(w.pts[k], mul(normals[k], 20.0)), mul(normals[k], -1.0), 20.0) {
                let off = 20.0 - t;
                df = df.max((off - wharf::SHORE_FACE_OUT_FT).abs());
                sum_f += off;
                nf += 1;
            }
        }
        let piers = wharf::shore_piers(&w, t0).len();
        println!(
            "seed {seed}: {place}: turn {turn:.0} deg over 500 ft, tightest r {tight:.0} ft; quay {run:.0} ft, {} pts; width {width:.0}: max dev {dw:.2} ft, folds {folds}; face out {:.2} ft mean, max dev {df:.2} ft ({nf} pts); piers {piers}",
            w.pts.len(),
            sum_f / nf.max(1) as f64
        );
        worst = (worst.0.max(dw), worst.1.max(df));
    }
    println!("worst: width dev {:.2} ft, face offset dev {:.2} ft", worst.0, worst.1);
}
