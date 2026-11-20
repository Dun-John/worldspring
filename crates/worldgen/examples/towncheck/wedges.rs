use crate::prelude::*;
use worldgen::t0::settle::Tier;

/// A footprint's sharpest corner (deg; vertices nearer than 1 ft to the last one kept are
/// merged into it) and its aspect (the short side of its fitted box over the long one).
pub(crate) fn wedge_of(poly: &[P]) -> (f64, f64) {
    let mut p: Vec<P> = Vec::new();
    for q in poly {
        if p.last().is_none_or(|l| dist(*l, *q) >= 1.0) {
            p.push(*q);
        }
    }
    while p.len() > 3 && dist(p[0], *p.last().unwrap()) < 1.0 {
        p.pop();
    }
    let m = p.len();
    if m < 3 {
        return (0.0, 0.0);
    }
    let ccw = area(&p) > 0.0;
    let mut sharp = 180.0f64;
    for k in 0..m {
        let (a, b, c) = (p[(k + m - 1) % m], p[k], p[(k + 1) % m]);
        let (u, v) = (sub(a, b), sub(c, b));
        // (Only convex corners: a reflex one is no point.)
        let turn = cross(sub(b, a), sub(c, b));
        if (turn > 0.0) != ccw {
            continue;
        }
        let ang = libm::acos((dot(u, v) / (len(u) * len(v)).max(1e-9)).clamp(-1.0, 1.0)).to_degrees();
        sharp = sharp.min(ang);
    }
    let o = worldgen::town::geom::obb(&p);
    (sharp, if o.long > 0.0 { o.short / o.long } else { 0.0 })
}

/// A footprint's least width (ft): the narrowest of its hull's calipers (each hull edge's
/// farthest point off it).
pub(crate) fn min_width(poly: &[P]) -> f64 {
    let h = worldgen::town::geom::hull(poly.to_vec());
    let m = h.len();
    let mut w = f64::MAX;
    for i in 0..m {
        let (a, b) = (h[i], h[(i + 1) % m]);
        let l = dist(a, b);
        if l < 1e-6 {
            continue;
        }
        let n = [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
        w = w.min(h.iter().map(|p| dot(sub(*p, a), n).abs()).fold(0.0, f64::max));
    }
    if w == f64::MAX { 0.0 } else { w }
}

/// A footprint's sharpest corner as the eye reads it (deg; the generator's own `fill::eye_corner`,
/// which is private to its crate): each convex corner, and across each short edge (under 14 ft, a
/// `blunt` chord, and under half of both edges beside it) between two convex corners the corner the
/// two edges beside it would meet at ahead (a trapezoid's narrow end, a corner cut by a chord).
pub(crate) fn eye_corner(poly: &[P]) -> f64 {
    let mut p: Vec<P> = Vec::new();
    for q in poly {
        if p.last().is_none_or(|l| dist(*l, *q) >= 1.0) {
            p.push(*q);
        }
    }
    while p.len() > 3 && dist(p[0], *p.last().unwrap()) < 1.0 {
        p.pop();
    }
    let m = p.len();
    if m < 3 {
        return 0.0;
    }
    let ccw = area(&p) > 0.0;
    let ang = |k: usize| -> Option<f64> {
        let (a, b, c) = (p[(k + m - 1) % m], p[k], p[(k + 1) % m]);
        if (cross(sub(b, a), sub(c, b)) > 0.0) != ccw {
            return None;
        }
        let (u, v) = (sub(a, b), sub(c, b));
        Some(libm::acos((dot(u, v) / (len(u) * len(v)).max(1e-9)).clamp(-1.0, 1.0)).to_degrees())
    };
    let mut sharp = 180.0f64;
    for k in 0..m {
        let Some(a0) = ang(k) else { continue };
        sharp = sharp.min(a0);
        let e = dist(p[k], p[(k + 1) % m]);
        let (before, after) = (dist(p[(k + m - 1) % m], p[k]), dist(p[(k + 1) % m], p[(k + 2) % m]));
        if e < 14.0 && e < 0.5 * before.min(after) {
            // (Only where they meet ahead, not (nearly) parallel: a strip, which the width finds.)
            if let Some(a1) = ang((k + 1) % m).filter(|a1| a0 + a1 > 185.0) {
                sharp = sharp.min(a0 + a1 - 180.0);
            }
        }
    }
    sharp
}

/// Footprint corners as drawn: vertices under 1 ft from the last kept merged into it.
fn verts(poly: &[P]) -> usize {
    let mut p: Vec<P> = Vec::new();
    for q in poly {
        if p.last().is_none_or(|l| dist(*l, *q) >= 1.0) {
            p.push(*q);
        }
    }
    while p.len() > 3 && dist(p[0], *p.last().unwrap()) < 1.0 {
        p.pop();
    }
    p.len()
}

/// (Roadmap 7, 162) Generated buildings that are wedges or slivers, in every town and village
/// laid out by the generator (hand-laid towns skipped): a corner under 60 deg, or an aspect
/// (fitted box's short side over its long one) under 0.25; and (P4) the wider net QA's eye
/// catches: three corners, a corner under 70 deg as the eye reads it (`eye_corner`), a least width (a winged building's narrowest range's) under 12 ft. Buildings with
/// pieces (keeps, temples) or on decks are left out. `NEAR=fx,fy,r` lists every building within
/// r ft of that map point with its numbers.
pub(crate) fn wedges(world: &World, t0: &T0, label: &str, r: &mut Report) {
    let near: Option<(P, f64)> = std::env::var("NEAR").ok().and_then(|v| {
        let f: Vec<f64> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        (f.len() == 3).then(|| ([f[0] * world.geom.map_w_ft, f[1] * world.geom.map_h_ft], f[2]))
    });
    for li in (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= Tier::Village && town::hand::size(world, li).is_none()) {
        if let Some((c, rr)) = near {
            if dist([t0.settlements[li].x, t0.settlements[li].y], c) > rr + 3000.0 {
                continue;
            }
        }
        let l = town::layout(world, t0, li);
        let t = if t0.settlements[li].tier >= Tier::Town { "towns" } else { "villages" };
        let (mut n, mut sharp, mut thin, mut wide) = (0, 0, 0, 0);
        for b in l.buildings.iter().filter(|b| b.parts.is_empty() && b.deck.is_none() && b.keep.is_none()) {
            let (ang, asp) = wedge_of(&b.poly);
            let nv = verts(&b.poly);
            // (An L's, a U's or a court's width is its narrowest range's.)
            let w = town::wings::ranges(b).map_or_else(|| min_width(&b.poly), |arms| arms.iter().map(|a| min_width(a)).fold(f64::MAX, f64::min));
            let eye = eye_corner(&b.poly);
            n += 1;
            let c0 = centroid(&b.poly);
            let at = format!("{label} L{li} b{} at {:.0},{:.0}", b.id, c0[0], c0[1]);
            if let Some((c, rr)) = near {
                if dist(c0, c) < rr {
                    say!("    {at} ({:+.0},{:+.0}) v{nv} corner {ang:.0} eye {eye:.0} aspect {asp:.2} width {w:.1} area {:.0} func {:?} name {:?} wings {}", c0[0] - c[0], c0[1] - c[1], area(&b.poly).abs(), b.func, b.name, town::wings::ranges(b).is_some());
                    if std::env::var("POLY").is_ok() {
                        say!("      {:?}", b.poly.iter().map(|p| ((p[0] - c0[0]) as i32, (p[1] - c0[1]) as i32)).collect::<Vec<_>>());
                    }
                }
            }
            if ang < 60.0 {
                sharp += 1;
                r.add(&format!("corner < 60 ({t})"), 60.0 - ang, format!("{at}: {ang:.0} deg"));
            }
            if asp < 0.25 {
                thin += 1;
                r.add(&format!("aspect < 0.25 ({t})"), 0.25 - asp, format!("{at}: {asp:.2}"));
            }
            if nv <= 3 {
                r.add(&format!("three corners ({t})"), 1.0, at.clone());
            }
            if eye < 70.0 {
                r.add(&format!("eye corner < 70 ({t})"), 70.0 - eye, format!("{at}: {eye:.0} deg"));
                r.tally(&format!("eye corner {}0s ({t})", (eye / 10.0) as i32), 1);
                if std::env::var("POLY").is_ok() {
                    say!("    SHARP {at} eye {eye:.0} v{nv} area {:.0} func {:?} ward {:?} name {:?} {:?}", area(&b.poly).abs(), b.func, b.ward, b.name, b.poly.iter().map(|p| ((p[0] - c0[0]) as i32, (p[1] - c0[1]) as i32)).collect::<Vec<_>>());
                }
            }
            if w < 12.0 {
                let winged = if town::wings::ranges(b).is_some() { "winged " } else { "" };
                r.add(&format!("width < 12 ft ({winged}{t})"), 12.0 - w, format!("{at}: {w:.1} ft"));
                if std::env::var("POLY").is_ok() {
                    say!("    THIN {at} w {w:.1} v{nv} area {:.0} func {:?} ward {:?} floors {} {:?}", area(&b.poly).abs(), b.func, b.ward, b.floors, b.poly.iter().map(|p| ((p[0] - c0[0]) as i32, (p[1] - c0[1]) as i32)).collect::<Vec<_>>());
                }
            }
            if nv <= 3 || eye < 70.0 || w < 12.0 {
                wide += 1;
            }
        }
        r.tally(&format!("buildings ({t})"), n);
        r.tally(&format!("wide-net bad ({t})"), wide);
        say!("    {label} L{li} {t}: {n} buildings, {sharp} corner < 60, {thin} aspect < 0.25, {wide} wide-net bad");
    }
}

/// (Roadmap 7, 163) L, U and courtyard buildings (`town::wings`) in every town laid out by the
/// generator: counts by kind and form, and the eligible ones (a kind that may, on a lot that fits,
/// before the roll) left rectangles; each shaped one's place for QA (`-v`).
pub(crate) fn wings(world: &World, t0: &T0, label: &str, r: &mut Report) {
    for li in (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= Tier::Village && town::hand::size(world, li).is_none()) {
        let l = town::layout(world, t0, li);
        for b in &l.buildings {
            let Some(arms) = town::wings::ranges(&l, b) else { continue };
            let kind = b.func.map_or(town::catalog::RESIDENTIAL[b.residential as usize], |f| town::catalog::CATALOG[f as usize].key);
            let form = match (arms.len(), b.poly.len()) {
                (2, _) => "L",
                (3, _) => "U",
                (_, 12) => "court",
                _ => "other",
            };
            r.tally(&format!("{form} {kind}"), 1);
            // (Fix B) Its narrowest range (ft): under 25 flagged.
            let narrow = arms.iter().map(|q| len(sub(q[1], q[0])).min(len(sub(q[3], q[0])))).fold(f64::MAX, f64::min);
            r.tally(&format!("{form}: narrowest range ft {}", (narrow / 5.0).round() * 5.0), 1);
            if narrow < 23.0 {
                r.add(&format!("{form}s with a range under 23 ft"), narrow, format!("{label} L{li} b{} {kind}", b.id));
            }
            // Its ground floor's doors out, and those onto its yard (the ground inside its ranges' box).
            if let Some(it) = worldgen::interior::generate_id(world, t0, &format!("b:{li}:{}", b.id)) {
                let lv = &it.levels[it.entry_level];
                let world_at = |p: [f32; 2]| add(it.origin, add(mul(it.axis, p[0] as f64 * 5.0), mul(it.across, p[1] as f64 * 5.0)));
                let (u, v) = (it.axis, it.across);
                let box_of = |d: P| (arms.iter().flatten().map(|p| dot(*p, d)).fold(f64::MAX, f64::min), arms.iter().flatten().map(|p| dot(*p, d)).fold(f64::MIN, f64::max));
                let ((s0, s1), (t0_, t1)) = (box_of(u), box_of(v));
                let (mut out, mut yard) = (0, 0);
                for d in lv.doors.iter().filter(|d| d.rooms.contains(&-1)) {
                    out += 1;
                    let (a, e) = (world_at(d.a), world_at(d.b));
                    let m = mul(add(a, e), 0.5);
                    let n = { let t = sub(e, a); let l = len(t).max(1e-9); [-t[1] / l, t[0] / l] };
                    let o = [add(m, mul(n, 2.0)), sub(m, mul(n, 2.0))].into_iter().find(|p| !worldgen::town::geom::contains(&b.poly, *p));
                    if o.is_some_and(|p| { let (a, c) = (dot(p, u), dot(p, v)); a > s0 && a < s1 && c > t0_ && c < t1 }) {
                        yard += 1;
                    }
                }
                // (164) Its squares as its footprint (the battlemap's and the roof's outline) has them, on every
                // floor above ground; and its yard box (`Interior::yard`) on a U's and a court's.
                let off = it.levels.iter().filter(|lv| lv.z >= 0).map(|lv| (0..it.nx * it.ny).filter(|&k| (lv.cells[k] >= 0) != worldgen::town::geom::contains(&b.poly, world_at([(k % it.nx) as f32 + 0.5, (k / it.nx) as f32 + 0.5]))).count()).max().unwrap_or(0);
                if off > 0 {
                    r.add(&format!("{form}s whose squares miss their footprint"), off as f64, format!("{label} L{li} b{} {kind}", b.id));
                }
                if form != "L" && it.yard.len() != 4 {
                    r.add(&format!("{form}s with no yard box"), 1.0, format!("{label} L{li} b{} {kind}", b.id));
                }
                r.tally(&format!("{form}: doors out"), out);
                r.tally(&format!("{form}: doors onto the yard"), yard);
                if yard == 0 {
                    r.add(&format!("{form}s with no door onto their yard"), 1.0, format!("{label} L{li} b{} {kind}", b.id));
                }
            }
            r.tally(&format!("{form} (all)"), 1);
            let c = centroid(&b.poly);
            say!("    {label} L{li} b{} {form} {kind} at {:.0},{:.0} fx {:.6} fy {:.6}", b.id, c[0], c[1], c[0] / world.geom.map_w_ft, c[1] / world.geom.map_h_ft);
        }
        r.tally("buildings", l.buildings.len());
        // (Fix B) Shaped ones within 30 ft of each other (their lots: footprints within 30 ft), and blocks with more than one U or court.
        let shaped: Vec<&town::Building> = l.buildings.iter().filter(|b| town::wings::ranges(&l, b).is_some()).collect();
        let lot = |b: &town::Building| l.grounds.iter().find(|g| g.0 == b.id).map_or(b.poly.clone(), |g| g.1.clone());
        let gap = |a: &[P], b: &[P]| a.iter().flat_map(|p| (0..b.len()).map(move |k| worldgen::town::geom::seg_dist(*p, b[k], b[(k + 1) % b.len()]))).fold(f64::MAX, f64::min);
        for i in 0..shaped.len() {
            for j in i + 1..shaped.len() {
                let (a, c) = (lot(shaped[i]), lot(shaped[j]));
                let d = gap(&a, &c).min(gap(&c, &a));
                if d < 29.0 {
                    r.add("shaped pairs within 30 ft", d, format!("{label} L{li} b{} b{}", shaped[i].id, shaped[j].id));
                }
            }
        }
        for blk in &l.blocks {
            let big = shaped.iter().filter(|b| b.poly.len() > 6 && worldgen::town::geom::contains(blk, centroid(&lot(b)))).count();
            if big > 1 {
                r.add("blocks with 2+ U/courts", big as f64, format!("{label} L{li}"));
            }
        }
    }
}
