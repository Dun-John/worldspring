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

/// (Roadmap 7, 162) Generated buildings that are wedges or slivers, in every town and village
/// laid out by the generator (hand-laid towns skipped): a corner under 60 deg, or an aspect
/// (fitted box's short side over its long one) under 0.25. Buildings with pieces (keeps,
/// temples) or on decks are left out.
pub(crate) fn wedges(world: &World, t0: &T0, label: &str, r: &mut Report) {
    for li in (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= Tier::Village && town::hand::size(world, li).is_none()) {
        let l = town::layout(world, t0, li);
        let t = if t0.settlements[li].tier >= Tier::Town { "towns" } else { "villages" };
        let (mut n, mut sharp, mut thin) = (0, 0, 0);
        for b in l.buildings.iter().filter(|b| b.parts.is_empty() && b.deck.is_none() && b.keep.is_none()) {
            let (ang, asp) = wedge_of(&b.poly);
            n += 1;
            let at = format!("{label} L{li} b{} at {:.0},{:.0}", b.id, centroid(&b.poly)[0], centroid(&b.poly)[1]);
            if ang < 60.0 {
                sharp += 1;
                r.add(&format!("corner < 60 ({t})"), 60.0 - ang, format!("{at}: {ang:.0} deg"));
            }
            if asp < 0.25 {
                thin += 1;
                r.add(&format!("aspect < 0.25 ({t})"), 0.25 - asp, format!("{at}: {asp:.2}"));
            }
        }
        r.tally(&format!("buildings ({t})"), n);
        say!("    {label} L{li} {t}: {n} buildings, {sharp} corner < 60, {thin} aspect < 0.25");
    }
}

/// (Roadmap 7, 163) L, U and courtyard buildings (`town::wings`) in every town laid out by the
/// generator: counts by kind and form, and the eligible ones (a kind that may, on a lot that fits,
/// before the roll) left rectangles; each shaped one's place for QA (`-v`).
pub(crate) fn wings(world: &World, t0: &T0, label: &str, r: &mut Report) {
    for li in (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= Tier::Village && town::hand::size(world, li).is_none()) {
        let l = town::layout(world, t0, li);
        for b in &l.buildings {
            let Some(arms) = town::wings::ranges(b) else { continue };
            let kind = b.func.map_or(town::catalog::RESIDENTIAL[b.residential as usize], |f| town::catalog::CATALOG[f as usize].key);
            let form = match (arms.len(), b.poly.len()) {
                (2, _) => "L",
                (3, _) => "U",
                (_, 12) => "court",
                _ => "other",
            };
            r.tally(&format!("{form} {kind}"), 1);
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
    }
}
