use crate::prelude::*;
use worldgen::t0::settle::Tier;

/// (Roadmap 7, 189) How a town's patches' sides (its streets' lines, inside radius `r` but
/// beyond a sixth of it) run round its middle `c`: the share of their length within 15 deg of
/// the way round (on a ring) and within 15 deg of the way out (a spoke).
fn round_and_out(l: &Layout, c: P, r: f64) -> (f64, f64) {
    let (mut ring, mut spoke, mut total) = (0.0, 0.0, 0.0);
    for (a, b) in l.districts.iter().filter(|p| p.len() >= 3).flat_map(|p| (0..p.len()).map(move |k| (p[k], p[(k + 1) % p.len()]))) {
        let (m, d) = (lerp(a, b, 0.5), dist(a, b));
        let out = sub(m, c);
        if d < 5.0 || len(out) > r || len(out) < r / 6.0 {
            continue;
        }
        let t = (dot(sub(b, a), out) / (d * len(out))).abs().min(1.0);
        let off_out = libm::acos(t).to_degrees();
        total += d;
        if off_out <= 15.0 {
            spoke += d;
        } else if off_out >= 75.0 {
            ring += d;
        }
    }
    if total > 0.0 { (ring / total, spoke / total) } else { (0.0, 0.0) }
}

/// (189) How much of a straight high street town `l` (urban radius `r`) has: the most main-street
/// length on one line (within 10 deg of one of 36 headings, within 60 ft across), over its width
/// (2 `r`).
fn straight(l: &Layout, r: f64) -> f64 {
    let segs: Vec<(P, P)> = l.roads.iter().filter(|x| x.1 == 4).flat_map(|x| x.0.windows(2).map(|w| (sub(w[0], l.center), sub(w[1], l.center))).collect::<Vec<_>>()).collect();
    let mut best = 0.0f64;
    for k in 0..36 {
        let a = k as f64 * std::f64::consts::PI / 36.0;
        let u = [libm::cos(a), libm::sin(a)];
        let along: Vec<&(P, P)> = segs.iter().filter(|(p, q)| dist(*p, *q) > 1.0 && (dot(sub(*q, *p), u) / dist(*p, *q)).abs() > libm::cos(10f64.to_radians())).collect();
        // (Lines: each aligned piece's offset across; the best 60 ft band's length.)
        for (p, q) in &along {
            let off = cross(u, lerp(*p, *q, 0.5));
            let t: f64 = along.iter().filter(|(a, b)| (cross(u, lerp(*a, *b, 0.5)) - off).abs() < 60.0).map(|(a, b)| dist(*a, *b)).sum();
            best = best.max(t);
        }
    }
    best / (2.0 * r)
}

/// (189) Per town and city (as generated, not by hand): the street pattern it is laid out on
/// (`Layout::pattern`; every town's set with `TC_PATTERN`), how its streets run round its middle
/// (`round_and_out`), and its buildings' wedges (`wedges::wedge_of`, `eye_corner`) and overlaps,
/// by pattern.
pub(crate) fn patterns(world: &World, t0: &T0, label: &str, r: &mut Report) {
    use town::clip::{self, Shape};
    for li in (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= Tier::Town && town::hand::size(world, li).is_none()) {
        let s = &t0.settlements[li];
        let l = town::layout(world, t0, li);
        let p = town::pattern::Pattern::from_code(l.pattern).name();
        let ur = worldgen::t0::radii::urban_radius(s.tier, s.population);
        let (ring, spoke) = round_and_out(&l, l.center, ur);
        crate::planning::svg_dump(label, li, &l, [world.geom.map_w_ft, world.geom.map_h_ft], t0);
        let (mut sharp, mut eye, mut over, mut n) = (0, 0, 0, 0);
        let bs: Vec<&town::Building> = l.buildings.iter().filter(|b| b.parts.is_empty() && b.deck.is_none() && b.keep.is_none()).collect();
        let boxes: Vec<[f64; 4]> = bs.iter().map(|b| bbox(&b.poly)).collect();
        for (i, b) in bs.iter().enumerate() {
            n += 1;
            if crate::wedges::wedge_of(&b.poly).0 < 60.0 {
                sharp += 1;
            }
            if crate::wedges::eye_corner(&b.poly) < 70.0 {
                eye += 1;
            }
            for j in i + 1..bs.len() {
                let (a, c) = (boxes[i], boxes[j]);
                if a[0] > c[2] || c[0] > a[2] || a[1] > c[3] || c[1] > a[3] {
                    continue;
                }
                let x = clip::area(&clip::intersection(&[Shape::ring(b.poly.clone())], &[Shape::ring(bs[j].poly.clone())]));
                if x > 1.0 {
                    over += 1;
                    r.add(&format!("buildings overlapping ({p})"), x, format!("{label} L{li} b{} {:?}/{:?} b{} {:?}/{:?}: {x:.0} ft2 at {:.0},{:.0}", b.id, b.func, b.ward, bs[j].id, bs[j].func, bs[j].ward, centroid(&b.poly)[0] - l.center[0], centroid(&b.poly)[1] - l.center[1]));
                }
            }
        }
        let hs = straight(&l, ur);
        let wide: f64 = l.roads.iter().filter(|x| x.1 == 4 && x.2 > 20.0).map(|x| x.0.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>()).sum();
        r.value(&format!("high street % ({p})"), 100.0 * hs, format!("{label} L{li}"));
        say!("    {label} L{li} {} {p} (planning {}): round {:.0}%, out {:.0}%, high street {:.0}% ({wide:.0} ft wide); {n} buildings (r {ur:.0} ft), {sharp} corner < 60, {eye} eye < 70, {over} overlapping; fx {:.4} fy {:.4}", s.tier.name(), l.planning, 100.0 * ring, 100.0 * spoke, 100.0 * hs, l.center[0] / world.geom.map_w_ft, l.center[1] / world.geom.map_h_ft);
        r.tally(&format!("towns {p}"), 1);
        r.value(&format!("round % ({p})"), 100.0 * ring, format!("{label} L{li}"));
        r.value(&format!("out % ({p})"), 100.0 * spoke, format!("{label} L{li}"));
        r.tally(&format!("buildings ({p})"), n);
        r.tally(&format!("corner < 60 ({p})"), sharp);
        r.tally(&format!("eye corner < 70 ({p})"), eye);
    }
}
