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
