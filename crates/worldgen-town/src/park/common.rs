//! A common laid out (`park::layout`, `ParkKind::Common`): its pond, paths, stream and footbridges,
//! the dressing round its hub and along its paths, its swells and its trees, as the module's notes
//! in `park` tell.

use super::*;

/// A stream's way down to the pond, as the ground has it: the water's level along `pts` (from the
/// spring to the mouth), a foot below the ground and never rising toward the pond, smoothed; none
/// where the ground doesn't fall to the pond (it would climb out of its bed or cut a trench).
fn stream_levels(pts: &[P], ground: &crate::town::ParkGround, level: f64, band: f64, rim: &[P], mouth: P) -> Option<Vec<f32>> {
    // The ground as the battlemap lays it round the pond: eased down to its banks over its band,
    // no steeper than 0.35 from there out to 30 ft past it.
    let laid = |q: P| {
        let h = ground.at(q);
        let e = (0..rim.len()).map(|k| seg_dist(q, rim[k], rim[(k + 1) % rim.len()])).fold(f64::MAX, f64::min);
        if contains(rim, q) { h } else { pond_bank(h, e, level, band) }
    };
    let side = |k: usize, off: f64| {
        let (a, b) = (pts[k.saturating_sub(1)], pts[(k + 1).min(pts.len() - 1)]);
        let n = mul(perp(unit_of(sub(b, a))), off);
        (laid(add(pts[k], n)), laid(sub(pts[k], n)))
    };
    // (The lowest of the ground on its line and on either bank, 4 and 10 ft past its edge; and the
    // higher of the ground either side 19 ft past its edge.)
    // (At its head, the ground behind the spring too.)
    let behind = |off: f64| laid(sub(pts[0], mul(unit_of(sub(pts[1.min(pts.len() - 1)], pts[0])), off)));
    let g: Vec<f64> = (0..pts.len())
        .map(|k| {
            let ((l, r), (l2, r2)) = (side(k, 0.5 * STREAM_FT + 4.0), side(k, 0.5 * STREAM_FT + 10.0));
            let h = laid(pts[k]).min(l).min(r).min(l2).min(r2);
            if k == 0 { h.min(behind(0.5 * STREAM_FT + 4.0)).min(behind(0.5 * STREAM_FT + 10.0)) } else { h }
        })
        .collect();
    let far: Vec<f64> = (0..pts.len())
        .map(|k| {
            let (l, r) = side(k, 0.5 * STREAM_FT + 19.0);
            l.max(r)
        })
        .collect();
    let mut w: Vec<f64> = Vec::with_capacity(g.len());
    let mut lo = f64::MAX;
    for &h in &g {
        lo = lo.min(h - 0.4);
        w.push(lo.max(level));
    }
    for _ in 0..2 {
        let prev = w.clone();
        for k in 1..w.len().saturating_sub(1) {
            w[k] = (prev[k - 1] + prev[k] + prev[k + 1]) / 3.0;
        }
    }
    // (Smoothed, still under its banks and falling all the way.)
    let mut lo = f64::MAX;
    for k in 0..w.len() {
        lo = lo.min(w[k]).min(g[k] - 0.3);
        w[k] = lo.max(level);
    }
    let n = w.len();
    *w.last_mut()? = level;
    // Falls at all, cuts no trench deeper than 3 ft (its banks, cut back three in ten, rise no more
    // than 2.5 ft more 19 ft out), never stands above its banks (but by the pond, whose banks are eased
    // to the water).
    let ok = w[0] - level >= 0.25
        && (0..n).all(|k| g[k] - w[k] <= 3.5 && far[k] - w[k] <= 7.0 && (dist(pts[k], mouth) < band + 6.0 || g[k] - w[k] >= 0.3));
    ok.then(|| w.iter().map(|&x| x as f32).collect())
}

/// A path crossing the stream goes straight and square across it there: each crossing (where it
/// is, and the way across) and the path with its points near it put in a line across.
/// A crossing within 35 ft of a bridge already there (`bridges`) isn't made: the path ends on that
/// bridge's end on its own bank instead (a Y onto the path that crosses there).
fn square_crossings(pts: &[P], stream: &[P], bridges: &[(P, P)]) -> (Vec<P>, Vec<(P, P)>) {
    let mut out: Vec<P> = Vec::new();
    let mut found: Vec<(P, P)> = Vec::new();
    let mut i = 0;
    while i < pts.len() {
        let hit = (i + 1 < pts.len())
            .then(|| stream.windows(2).find_map(|w| crossing(pts[i], pts[i + 1], w[0], w[1]).map(|t| (lerp(pts[i], pts[i + 1], t), unit_of(sub(w[1], w[0]))))))
            .flatten();
        let Some((q, sd)) = hit.filter(|(q, _)| found.iter().all(|f| dist(f.0, *q) > 30.0)) else {
            out.push(pts[i]);
            i += 1;
            continue;
        };
        if let Some(&(bq, bn)) = bridges.iter().find(|b| dist(b.0, q) < 35.0) {
            let side = if dot(sub(pts[i], bq), bn) >= 0.0 { 1.0 } else { -1.0 };
            let end = add(bq, mul(bn, 12.0 * side));
            out.push(pts[i]);
            while out.len() > 1 && dist(*out.last().unwrap(), end) < 8.0 {
                out.pop();
            }
            out.push(end);
            return (out, found);
        }
        let way = sub(pts[i + 1], pts[i]);
        let mut n = perp(sd);
        if dot(n, way) < 0.0 {
            n = mul(n, -1.0);
        }
        out.push(pts[i]);
        while out.len() > 1 && dist(*out.last().unwrap(), q) < 14.0 {
            out.pop();
        }
        out.push(sub(q, mul(n, 12.0)));
        out.push(add(q, mul(n, 12.0)));
        found.push((q, n));
        i += 1;
        while i + 1 < pts.len() && dist(pts[i], q) < 14.0 {
            i += 1;
        }
    }
    (out, found)
}

/// A common's pond (`out.pond`) in the lowest hollow that holds one, and its hub (`hub`) moved
/// clear of it or deeper in: the hub, and the pond as its middle, its water's level and its band.
fn lay_pond(
    p: &Park,
    o: &Obb,
    inside: &[(P, f64)],
    steep: bool,
    rim: impl Fn(f64) -> f64,
    rmax: f64,
    mut hub: P,
    rng: &mut Pcg32,
    out: &mut ParkLayout,
) -> (P, Option<(P, f64, f64)>) {
    let poly = &p.pts;
    let g = &p.ground;
    let ring = |pc: P, f: f64| -> Vec<f64> {
        (0..16)
            .map(|k| {
                let t = std::f64::consts::TAU * k as f64 / 16.0;
                g.at(add(pc, mul([libm::cos(t), libm::sin(t)], f * rim(t))))
            })
            .collect()
    };
    let mut pond: Option<(P, f64, f64)> = None;
    if !steep && o.short >= 140.0 && rng.next_f64() < 0.85 {
        let mut cands: Vec<(f64, P)> = inside.iter().filter(|&&(q, _)| depth(poly, q) >= rmax + 34.0).map(|&(q, _)| (ring(q, 1.2).iter().sum::<f64>() / 16.0, q)).collect();
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1[0].total_cmp(&b.1[0])).then(a.1[1].total_cmp(&b.1[1])));
        for &(_, q) in cands.iter().take(24) {
            // (A little below the lowest ground round it, near or further out: no water stands over
            // ground that falls away past a thin bank.)
            // (On level ground it is dug in, the ground round it 6 ft above its water: its banks then
            // rise a tier, where no knoll may. The ground at its band's edge and 20 ft past it no more
            // than 7 ft over the water: no pond in a pit.)
            let past = |off: f64| -> Vec<f64> {
                (0..16)
                    .map(|k| {
                        let t = std::f64::consts::TAU * k as f64 / 16.0;
                        g.at(add(q, mul([libm::cos(t), libm::sin(t)], rim(t) + off)))
                    })
                    .collect()
            };
            let band = POND_BAND;
            // (And every point of the planned ground out to there: the rings alone may miss a rise.)
            let rmin = (0..32).map(|k| rim(std::f64::consts::TAU * k as f64 / 32.0)).fold(f64::MAX, f64::min);
            let round = (0..g.ny).flat_map(|j| (0..g.nx).map(move |i| (i, j))).filter(|&(i, j)| (rmin..=rmax + band + 20.0).contains(&dist(g.point(i, j), q))).map(|(i, j)| g.h[j * g.nx + i] as f64);
            let hi = past(band).iter().chain(past(band + 20.0).iter()).copied().chain(round).fold(f64::MIN, f64::max);
            let level = ([1.05, 1.5, 2.0, 2.6].iter().flat_map(|&f| ring(q, f)).fold(f64::MAX, f64::min) - 0.3).min(hi - 6.0);
            let above = hi - level;
            // (Its banks well in from the park's edge: past it the drawn ground may stand otherwise.)
            if above <= 7.0 && depth(poly, q) >= rmax + band + 24.0 && !g.wet_near(q, rmax + band + 10.0) {
                pond = Some((q, level, band));
                break;
            }
        }
    }
    // (The hub's round wholly inside, 4 ft to spare: else the deepest point inside.)
    if depth(poly, hub) < 17.0
        && let Some(&(q, _)) = inside.iter().max_by(|a, b| depth(poly, a.0).total_cmp(&depth(poly, b.0)).then(b.0[0].total_cmp(&a.0[0])).then(b.0[1].total_cmp(&a.0[1])))
    {
        hub = q;
    }
    // The hub stands clear of it, its banks and 18 ft more: moved straight away from it, else round it
    // (the nearest way first) to where it is 20 ft or more inside; with nowhere, no pond.
    if let Some((pc, _, band)) = pond
        && dist(hub, pc) < rmax + band + 18.0
    {
        let away = sub(hub, pc);
        let a0 = if len(away) > 1e-6 { libm::atan2(away[1], away[0]) } else { 0.0 };
        let reach = rmax + band + 18.0;
        let moved = (0..16).map(|k| a0 + std::f64::consts::TAU / 16.0 * if k % 2 == 0 { (k / 2) as f64 } else { -((k + 1) / 2) as f64 });
        match moved.map(|t| add(pc, mul([libm::cos(t), libm::sin(t)], reach))).find(|&q| depth(poly, q) >= 20.0) {
            Some(q) => hub = q,
            None => pond = None,
        }
    }
    if let Some((pc, level, band)) = pond {
        let pts: Vec<P> = (0..28)
            .map(|k| {
                let t = std::f64::consts::TAU * k as f64 / 28.0;
                add(pc, mul([libm::cos(t), libm::sin(t)], rim(t)))
            })
            .collect();
        out.pond = Some(Pond { pts, centre: pc, level: level as f32, band });
    }
    (hub, pond)
}

pub(super) fn common(p: &Park, biome: Biome, city: bool, key: u64, rng: &mut Pcg32) -> ParkLayout {
    let poly = &p.pts;
    let g = &p.ground;
    let mut out = ParkLayout::default();
    let mut clear = Clear::new(poly);
    let o = obb(poly);
    let c = centroid(poly);
    let m = poly.len();
    let size = crate::core::sqrt(area(poly).abs());
    let (trees, shrubs, small, few) = plants(biome);
    out.bench = [0, 1, 1, 3][rng.below(4) as usize];
    let lamp = [0, 1, 2][rng.below(3) as usize];
    const PATH_W: f64 = 7.0;
    // The ground inside it (dry points of its grid at least 20 ft in): low, high, and how steep.
    let inside: Vec<(P, f64)> = (0..g.ny)
        .flat_map(|j| (0..g.nx).map(move |i| (i, j)))
        .filter(|&(i, j)| !g.wet[j * g.nx + i] && depth(poly, g.point(i, j)) >= 20.0)
        .map(|(i, j)| (g.point(i, j), g.h[j * g.nx + i] as f64))
        .collect();
    let lowest = inside.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
    let highest = inside.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1));
    let slope = match (lowest, highest) {
        (Some(lo), Some(hi)) if dist(lo.0, hi.0) > 20.0 => (hi.1 - lo.1) / dist(lo.0, hi.0),
        _ => 0.0,
    };
    let steep = slope > 0.1;

    // The hub, a little off the middle.
    let hub = add(c, [(rng.next_f64() - 0.5) * 0.15 * o.short, (rng.next_f64() - 0.5) * 0.15 * o.short]);
    // The pond: in the lowest hollow that holds one (its level a little below the lowest ground
    // round its rim; the ground further round no more than 7 ft above it, its banks eased out far
    // enough to be no steeper than one in two), clear of the water.
    let rp = (0.13 * size).clamp(16.0, 40.0);
    let shape = (rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::PI), rng.range(1.1, 1.4));
    // The pond's rim radius at angle `t`: an ellipse (`el` to 1 / `el` across, along `ax`), wobbled.
    let rim = move |t: f64| {
        let (a, b, ax, el) = shape;
        let (cs, sn) = (libm::cos(t - ax), libm::sin(t - ax));
        let ellipse = 1.0 / crate::core::sqrt(cs * cs / (el * el) + sn * sn * el * el);
        rp * (1.0 + 0.14 * libm::sin(2.0 * t + a) + 0.08 * libm::sin(3.0 * t + b)) * ellipse
    };
    let rmax = (0..32).map(|k| rim(std::f64::consts::TAU * k as f64 / 32.0)).fold(0.0, f64::max);
    let (hub, pond) = lay_pond(p, &o, &inside, steep, rim, rmax, hub, rng, &mut out);
    let pond_c = pond.map(|x| x.0);
    let band = pond.map_or(0.0, |x| x.2);
    // Out of the pond: a point inside it (or within `margin` of its rim) pushed out to the rim and
    // the margin.
    let off_pond = |q: P, margin: f64| -> P {
        let Some(pc) = pond_c else { return q };
        let d = sub(q, pc);
        let t = libm::atan2(d[1], d[0]);
        let r = rim(t) + margin;
        if len(d) < r { add(pc, mul(unit_of(d), r)) } else { q }
    };
    // The gazebo, or a city's bandstand now and then: on a raised tier on the high ground, or on a
    // slope at the top, no tier.
    let stand = if rng.next_f64() < if city { 0.45 } else { 0.2 } { Kind::Bandstand } else { Kind::Gazebo };
    let rm = foot(stand) + 5.0;
    let mut tops: Vec<(P, f64)> = inside.clone();
    tops.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0[0].total_cmp(&b.0[0])).then(a.0[1].total_cmp(&b.0[1])));
    let stand_at = (o.short >= 120.0)
        .then(|| {
            tops.iter().map(|t| t.0).find(|&q| {
                depth(poly, q) >= rm + if steep { 12.0 } else { 22.0 } && dist(q, hub) >= rm + 30.0 && pond_c.is_none_or(|pc| dist(q, pc) >= rmax + band + rm + 20.0)
            })
        })
        .flatten();
    // (A raised tier only on gentle ground: on a slope it would stand out of the hill as a cliff.)
    let gentle = |q: P| {
        let hs: Vec<f64> = (0..12).map(|k| std::f64::consts::TAU * k as f64 / 12.0).map(|t| g.at(add(q, mul([libm::cos(t), libm::sin(t)], rm + 20.0)))).collect();
        hs.iter().copied().fold(f64::MIN, f64::max) - hs.iter().copied().fold(f64::MAX, f64::min) <= 5.0
    };
    let mound = stand_at.filter(|&q| !steep && gentle(q));
    let off_mound = |q: P, margin: f64| -> P {
        let Some(mc) = stand_at else { return q };
        let d = sub(q, mc);
        if len(d) < rm + margin { add(mc, mul(unit_of(d), rm + margin)) } else { q }
    };
    // A winding path from `a` to `b`: a line every 16 ft pushed across by low noise (most in the
    // middle), out of the pond and the tier, kept inside, then smoothed.
    let nseed = key ^ 0x7a17;
    let wind = |a: P, b: P, amp: f64, margin: f64| -> Vec<P> {
        let l = dist(a, b);
        let n = (crate::core::ceil(l / 16.0) as usize).max(2);
        let nrm = perp(unit_of(sub(b, a)));
        let pts: Vec<P> = (0..=n)
            .map(|k| {
                let t = k as f64 / n as f64;
                let base = lerp(a, b, t);
                if k == 0 || k == n {
                    return base;
                }
                let push = amp * libm::sin(std::f64::consts::PI * t) * fbm(nseed, base[0] / 150.0, base[1] / 150.0, 2, 2.0, 0.5) * 1.6;
                let mut q = off_mound(off_pond(add(base, mul(nrm, push)), margin), margin + 8.0);
                if depth(poly, q) < 10.0 {
                    q = lerp(q, c, 0.15);
                }
                q
            })
            .collect();
        chaikin(&pts, 2).iter().map(|&q| off_pond(q, margin - 2.0)).collect()
    };
    // Ways in from the street sides only: the middle of the longest (up to four).
    let mut sides: Vec<(f64, usize)> = (0..m).map(|e| (dist(poly[e], poly[(e + 1) % m]), e)).filter(|s| s.0 >= 40.0 && p.fronts.get(s.1).copied().unwrap_or(false)).collect();
    sides.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let ways = (2 + rng.below(3) as usize).min(sides.len());
    let mut entrances: Vec<P> = Vec::new();
    for &(_, e) in sides.iter().take(ways) {
        let at = lerp(poly[e], poly[(e + 1) % m], rng.range(0.35, 0.65));
        if entrances.iter().all(|q| dist(*q, at) > 60.0) {
            entrances.push(at);
        }
    }
    let mut paths: Vec<Vec<P>> = Vec::new();
    // A big one's walk round, a little in from its edge.
    if o.short >= 220.0 {
        let belt = inset(poly, &vec![26.0; m]);
        if belt.len() >= 3 {
            let mut pts: Vec<P> = Vec::new();
            for e in 0..belt.len() {
                let (a, b) = (belt[e], belt[(e + 1) % belt.len()]);
                let n = (crate::core::ceil(dist(a, b) / 18.0) as usize).max(1);
                for k in 0..n {
                    let q = lerp(a, b, k as f64 / n as f64);
                    let w = fbm(nseed ^ 0xbe17, q[0] / 120.0, q[1] / 120.0, 2, 2.0, 0.5) * 14.0;
                    let toward = unit_of(sub(c, q));
                    pts.push(off_mound(off_pond(add(q, mul(toward, w)), 9.0), 10.0));
                }
            }
            pts.push(pts[0]);
            paths.push(chaikin(&pts, 2));
        }
    }
    // From each way in toward the hub, joining an earlier path where it comes within 40 ft of one
    // (away from the hub): a Y, not a star.
    let mut spokes: Vec<usize> = Vec::new();
    for &e in &entrances {
        let full = wind(e, hub, (0.35 * dist(e, hub)).min(60.0), 9.0);
        let mut cut = None;
        let mut run = 0.0;
        for k in 1..full.len() {
            run += dist(full[k - 1], full[k]);
            if run < 30.0 || dist(full[k], hub) < 50.0 {
                continue;
            }
            let near = spokes.iter().map(|&s| nearest_on(&paths[s], full[k])).filter(|x| x.1 < 40.0).min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((q, _)) = near {
                cut = Some((k, q));
                break;
            }
        }
        let pts = match cut {
            Some((k, q)) => {
                let mut pts = full[..=k].to_vec();
                pts.push(q);
                pts
            }
            None => full,
        };
        spokes.push(paths.len());
        paths.push(pts);
    }
    // The way up to the stand, coming to its steps (on its south side) from the south.
    if let Some(mc) = stand_at {
        let steps = add(mc, [0.0, foot(stand) + 2.5]);
        let below = add(mc, [0.0, rm + 12.0]);
        let mut pts = wind(hub, if depth(poly, below) > 8.0 { below } else { steps }, 20.0, 9.0);
        pts.push(steps);
        paths.push(pts);
        out.reserved.push((mc, foot(stand) + 1.0));
    }
    // A steep common with no stand: a viewpoint at its top (a bench looking downhill), the path to it
    // climbing across the slope by a point to one side.
    let view = (steep && stand_at.is_none()).then(|| tops.iter().map(|t| t.0).find(|&q| depth(poly, q) >= 14.0 && dist(q, hub) >= 40.0)).flatten();
    if let Some(v) = view {
        let down = lowest.map_or([0.0, 1.0], |lo| unit_of(sub(lo.0, v)));
        let end = add(v, mul(down, 7.0));
        let side = add(lerp(hub, end, 0.5), mul(perp(unit_of(sub(end, hub))), 0.3 * dist(hub, end)));
        let side = if depth(poly, side) > 14.0 { side } else { lerp(hub, end, 0.5) };
        let mut pts = wind(hub, side, 8.0, 9.0);
        pts.extend(wind(side, end, 8.0, 9.0).into_iter().skip(1));
        paths.push(pts);
        clear.disc(v, foot(Kind::Bench));
        out.props.push((v, facing(down), Kind::Bench, out.bench, 1.0));
    }
    // A walk round the pond's near side, both ends joined to the nearest path.
    if let Some(pc) = pond_c {
        let toward = sub(hub, pc);
        let t0 = libm::atan2(toward[1], toward[0]);
        let walk: Vec<P> = (0..=22)
            .map(|k| {
                let t = t0 + (k as f64 / 22.0 - 0.5) * 3.6;
                add(pc, mul([libm::cos(t), libm::sin(t)], rim(t) + 10.0))
            })
            .filter(|&q| depth(poly, q) >= 8.0)
            .collect();
        if walk.len() >= 6 {
            let walk = chaikin(&walk, 1);
            let ends = [walk[0], walk[walk.len() - 1]];
            let others: Vec<Vec<P>> = paths.clone();
            paths.push(walk);
            for end in ends {
                match others.iter().map(|pts| nearest_on(pts, end)).min_by(|a, b| a.1.total_cmp(&b.1)) {
                    Some((_, d)) if d <= 3.0 => {}
                    Some((q, d)) if d < 90.0 => paths.push(wind(end, q, 6.0, 7.0)),
                    _ => paths.push(wind(end, hub, 12.0, 9.0)),
                }
            }
        }
    }
    // A stream down to the pond now and then, from a spring at least 30 ft in from the edge, the
    // way (of seven, away from the hub) whose ground falls most; its head cut back until its spring
    // stands 3 ft or more off the paths and a path crossing it does so 12 ft or more below its head.
    let path_gap = |q: P| paths.iter().map(|pts| nearest_on(pts, q).1).fold(f64::MAX, f64::min) - 0.5 * PATH_W;
    if let Some((pc, level, band)) = pond
        && rng.next_f64() < 0.6
    {
        let away = sub(pc, hub);
        let a0 = libm::atan2(away[1], away[0]);
        let mut best: Option<(f64, Vec<(P, f32)>)> = None;
        for k in 0..7 {
            let t = a0 + (k as f64 - 3.0) * 0.32;
            let d = [libm::cos(t), libm::sin(t)];
            let mouth = add(pc, mul(d, rim(t) - 4.0));
            let mut head = mouth;
            while depth(poly, add(head, mul(d, 5.0))) >= 30.0 && dist(head, mouth) < 260.0 {
                head = add(head, mul(d, 5.0));
            }
            if dist(head, mouth) < 50.0 || g.wet_near(head, 30.0) {
                continue;
            }
            let n = (crate::core::ceil(dist(head, mouth) / 14.0) as usize).max(2);
            let nrm = perp(d);
            let raw: Vec<P> = (0..=n)
                .map(|j| {
                    let f = j as f64 / n as f64;
                    let base = lerp(head, mouth, f);
                    off_mound(add(base, mul(nrm, 10.0 * libm::sin(std::f64::consts::PI * f) * fbm(nseed ^ 0x51, base[0] / 70.0, base[1] / 70.0, 2, 2.0, 0.5))), 12.0)
                })
                .collect();
            let mut pts = chaikin(&raw, 2);
            // (And until it rises at the highest ground along it: nothing upstream of its spring climbs
            // above it.)
            while pts.len() > 2 {
                let spring = sub(pts[0], mul(unit_of(sub(pts[1], pts[0])), 0.5 * STREAM_FT + 4.0));
                let top = pts[1..].iter().map(|&q| g.at(q)).fold(f64::MIN, f64::max);
                if path_gap(pts[0]) >= 12.0 && path_gap(spring) >= 6.0 + 3.0 && top <= g.at(pts[0]) + 0.2 {
                    break;
                }
                pts.remove(0);
            }
            if length(&pts) < 40.0 {
                continue;
            }
            let Some(w) = stream_levels(&pts, g, level, band, &out.pond.as_ref().map_or(Vec::new(), |pd| pd.pts.clone()), mouth) else { continue };
            let fall = w[0] as f64 - level;
            if best.as_ref().is_none_or(|b| fall > b.0) {
                best = Some((fall, pts.into_iter().zip(w).collect()));
            }
        }
        if let Some((_, s)) = best {
            out.stream = s;
            // Its spring: stones round where it rises, reeds by them.
            // (Just behind its head, on the dry.)
            let (head, next) = (out.stream[0].0, out.stream[1].0);
            let spring = sub(head, mul(unit_of(sub(next, head)), 0.5 * STREAM_FT + 4.0));
            out.props.push((spring, 0.0, Kind::RockPile, rng.below(4) as u8, 1.1));
            clear.disc(spring, 6.0);
        }
    }
    // Where a path crosses the stream it goes straight across on a footbridge, square to the water.
    let stream: Vec<P> = out.stream.iter().map(|s| s.0).collect();
    if stream.len() >= 2 {
        let mut bridges: Vec<(P, P)> = Vec::new();
        for pts in &mut paths {
            let (squared, found) = square_crossings(pts, &stream, &bridges);
            *pts = squared;
            for (q, n) in found {
                out.props.push((q, along(n), Kind::Footbridge, [0, 0, 2, 3][rng.below(4) as usize], 1.05));
                clear.disc(q, 3.0);
                bridges.push((q, n));
            }
        }
        for s in stream.windows(2) {
            clear.seg(s[0], s[1], 0.5 * STREAM_FT + 2.0);
        }
    }
    for pts in &paths {
        clear.path(pts, PATH_W);
    }
    out.paths = paths.into_iter().map(|pts| (pts, PATH_W)).collect();
    // The pond and the stand, kept clear.
    if let Some(pc) = pond_c {
        let r = (0..16).map(|k| rim(std::f64::consts::TAU * k as f64 / 16.0)).fold(0.0, f64::max);
        clear.disc(pc, r + 2.0);
    }
    if let Some(mc) = stand_at {
        out.mound = mound.map(|mc| (mc, rm));
        out.props.push((mc, 0.0, stand, rng.below(4) as u8, 1.0));
        clear.disc(mc, rm + 3.0);
        // On a slope's top, a bench beside it looking downhill.
        if steep {
            let down = lowest.map_or([0.0, 1.0], |lo| unit_of(sub(lo.0, mc)));
            let b = add(mc, mul(down, rm + 4.0));
            if depth(poly, b) >= 6.0 && clear.free(b, foot(Kind::Bench), 1.0) {
                clear.disc(b, foot(Kind::Bench));
                out.props.push((b, facing(down), Kind::Bench, out.bench, 1.0));
            }
        }
    }
    // The hub: a fountain or a statue in a gravel round, benches round it.
    let rh = 13.0;
    out.rounds.push((hub, rh));
    if rng.next_f64() < 0.55 {
        out.props.push((hub, 0.0, Kind::Fountain, [0, 1, 3][rng.below(3) as usize], 1.1));
    } else {
        out.props.push((hub, 0.0, Kind::GardenStatue, [1, 3, 0][rng.below(3) as usize], 1.0));
    }
    clear.disc(hub, rh - 1.0);
    let mut round_benches = 0;
    for k in 0..6 {
        let t = std::f64::consts::TAU * (k as f64 + rng.next_f64() * 0.3) / 6.0;
        let d = [libm::cos(t), libm::sin(t)];
        let b = add(hub, mul(d, rh + 3.5));
        if round_benches < 4 && clear.path_gap(b) > foot(Kind::Bench) + 1.0 && clear.free(b, foot(Kind::Bench), 2.0) {
            clear.disc(b, foot(Kind::Bench));
            out.props.push((b, facing(mul(d, -1.0)), Kind::Bench, out.bench, 1.0));
            round_benches += 1;
        }
    }
    // Reeds along the pond's far side, in the shallows.
    if let Some(pc) = pond_c {
        let away = sub(pc, hub);
        let t0 = libm::atan2(away[1], away[0]) + rng.range(-0.8, 0.8);
        let span = rng.range(1.6, 2.6);
        let n = (span * rp / 6.0) as usize;
        for k in 0..=n {
            let t = t0 + (k as f64 / n.max(1) as f64 - 0.5) * span;
            let q = add(pc, mul([libm::cos(t), libm::sin(t)], rim(t) - 2.5 - 2.0 * jit(key, [t, 0.0], 5)));
            out.props.push((q, rng.range(0.0, std::f64::consts::TAU) as f32, Kind::Reeds, rng.below(4) as u8, rng.range(0.8, 1.2) as f32));
        }
    }
    // Benches along the paths, by turns either side, facing them; a city's lamps between.
    for (pi, (pts, w)) in out.paths.clone().iter().enumerate() {
        for (k, (q, d)) in every(pts, rng.range(40.0, 55.0), 22.0, 14.0).into_iter().enumerate() {
            if dist(q, hub) < rh + 14.0 {
                continue;
            }
            let side = perp(d);
            let s = if (k + pi) % 2 == 0 { 1.0 } else { -1.0 };
            let b = add(q, mul(side, s * (0.5 * w + foot(Kind::Bench) + 1.3)));
            if clear.free(b, foot(Kind::Bench), 3.0) {
                clear.disc(b, foot(Kind::Bench));
                out.props.push((b, facing(mul(side, -s)), Kind::Bench, out.bench, 1.0));
            }
            if city && k % 2 == 1 {
                let l = add(q, mul(side, -s * (0.5 * w + foot(Kind::LampPost) + 1.3)));
                if clear.free(l, foot(Kind::LampPost), 3.0) {
                    clear.disc(l, foot(Kind::LampPost));
                    out.props.push((l, 0.0, Kind::LampPost, lamp, 1.0));
                }
            }
        }
    }
    // A few beds round the hub's round.
    for k in 0..2 + rng.below(3) as usize {
        let t = std::f64::consts::TAU * (k as f64 + 0.5) / 4.0 + rng.range(-0.3, 0.3);
        let d = [libm::cos(t), libm::sin(t)];
        let f = add(hub, mul(d, rh + 9.0));
        if clear.free(f, 0.8 * foot(Kind::Flowerbed), 3.0) {
            clear.disc(f, 0.8 * foot(Kind::Flowerbed));
            out.props.push((f, along(perp(d)), Kind::Flowerbed, [0, 1, 2][rng.below(3) as usize], 1.0));
        }
    }
    // Broad low swells, two or more by its size (none on a slope: it has its own), clear of the pond's
    // banks, the stream, the stand and the hub: the common rolls rather than lies flat.
    if !steep {
        let (lo, hi) = bbox_of(poly);
        let want = ((area(poly).abs() / 40000.0) as usize).clamp(2, 7) + rng.below(2) as usize;
        for _ in 0..8 * want {
            if out.swells.len() >= want {
                break;
            }
            // (Some longer one way than the other, now and then one lower, under a tier: not all alike.)
            let t = rng.range(0.0, std::f64::consts::PI);
            let sw = Swell {
                at: [rng.range(lo[0], hi[0]), rng.range(lo[1], hi[1])],
                r: rng.range(26.0, 44.0),
                rise: if rng.next_f64() < 0.25 { rng.range(2.0, 4.0) } else { rng.range(4.5, 6.5) } as f32,
                axis: [libm::cos(t), libm::sin(t)],
                el: rng.range(1.0, 1.5),
            };
            let (q, r) = (sw.at, sw.reach());
            // (Wholly inside, 4 ft to spare: the verge by the street keeps its ground.)
            let ok = depth(poly, q) >= r + 4.0
                && dist(q, hub) >= r + 16.0
                && pond_c.is_none_or(|pc| dist(q, pc) >= rmax + band + r + 20.0)
                && stand_at.is_none_or(|mc| dist(q, mc) >= rm + r + 14.0)
                && (stream.len() < 2 || nearest_on(&stream, q).1 >= r + 12.0)
                && out.swells.iter().all(|s| dist(s.at, q) >= 0.8 * (s.reach() + r))
                && !g.water_near(q, r + 10.0);
            if ok {
                out.swells.push(sw);
            }
        }
    }
    // Trees: clumps of four to eight on a jittered grid, glades between, shrubs at their edges, a
    // few specimen trees in the open; a loose belt along the edge. Where the biome's trees are few,
    // a handful in all.
    let pick = |rng: &mut Pcg32, kinds: &[Kind]| kinds[rng.below(kinds.len() as u32) as usize];
    let mut budget = if few { ((area(poly).abs() / 20000.0) as usize).clamp(1, 4) } else { usize::MAX };
    let ([x0, y0], [x1, y1]) = bbox_of(poly);
    // (On the world's grid of clump cells, each with rolls of its own: a corner moved keeps the
    // clumps away from it.)
    const CLUMP: f64 = 75.0;
    let cell = |v: f64| crate::core::floor(v / CLUMP) as i64;
    for j in cell(y0)..=cell(y1) {
        for i in cell(x0)..=cell(x1) {
            let rng = &mut Pcg32::new(hash3(key ^ 0xc1a3, i, j, 7), 41);
            let at = [(i as f64 + rng.next_f64()) * CLUMP, (j as f64 + rng.next_f64()) * CLUMP];
            if rng.next_f64() > 0.5 || depth(poly, at) < 14.0 {
                continue;
            }
            let kind = pick(rng, trees);
            for _ in 0..4 + rng.below(5) {
                let q = add(at, [rng.range(-14.0, 14.0), rng.range(-14.0, 14.0)]);
                let r = 0.45 * foot(kind);
                if budget > 0 && clear.free(q, r, 0.8 * foot(kind)) {
                    clear.disc(q, r);
                    out.props.push((q, 0.0, kind, rng.below(8) as u8, rng.range(0.9, 1.2) as f32));
                    budget -= 1;
                }
            }
            for _ in 0..1 + rng.below(3) {
                let kind = pick(rng, shrubs);
                let t = rng.range(0.0, std::f64::consts::TAU);
                let q = add(at, mul([libm::cos(t), libm::sin(t)], rng.range(15.0, 21.0)));
                if clear.free(q, foot(kind), 3.0) {
                    clear.disc(q, foot(kind));
                    out.props.push((q, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8, 1.0));
                }
            }
            if !small.is_empty() && rng.next_f64() < 0.5 {
                let kind = pick(rng, small);
                let q = add(at, [rng.range(-24.0, 24.0), rng.range(-24.0, 24.0)]);
                if clear.free(q, foot(kind), 3.0) {
                    clear.disc(q, foot(kind));
                    out.props.push((q, rng.range(0.0, std::f64::consts::TAU) as f32, kind, rng.below(4) as u8, 1.0));
                }
            }
        }
    }
    // A few specimen trees in the open, on the world's grid of 120-ft cells (one in four holds one),
    // each with rolls of its own.
    const SPECIMEN: f64 = 120.0;
    let scell = |v: f64| crate::core::floor(v / SPECIMEN) as i64;
    for j in scell(y0)..=scell(y1) {
        for i in scell(x0)..=scell(x1) {
            let rng = &mut Pcg32::new(hash3(key ^ 0x5be7, i, j, 11), 43);
            let q = [(i as f64 + rng.next_f64()) * SPECIMEN, (j as f64 + rng.next_f64()) * SPECIMEN];
            let kind = pick(rng, trees);
            let r = foot(kind) * 1.25;
            if rng.next_f64() < 0.25 && budget > 0 && depth(poly, q) >= r && clear.free(q, r + 6.0, 4.0) {
                clear.disc(q, r);
                out.props.push((q, 0.0, kind, rng.below(8) as u8, 1.3));
                budget -= 1;
            }
        }
    }
    // A loose belt of trees 11 ft in from the edge: where it passes through each 26-ft cell of the
    // world, at its point nearest the cell's own mark, by the cell's own rolls (a corner moved keeps
    // the belt's trees elsewhere).
    let belt = inset(poly, &vec![11.0; m]);
    if belt.len() >= 3 && !few {
        let mut pts = belt.clone();
        pts.push(belt[0]);
        const BELT: f64 = 26.0;
        let mark = |c: (i64, i64)| {
            let h = hash3(key ^ 0xbe17, c.0, c.1, 13);
            [(c.0 as f64 + 0.15 + 0.7 * unit(h)) * BELT, (c.1 as f64 + 0.15 + 0.7 * unit(h >> 21)) * BELT]
        };
        let mut at: std::collections::BTreeMap<(i64, i64), P> = std::collections::BTreeMap::new();
        for (q, _) in every(&pts, 2.0, 0.0, 0.0) {
            let c = (crate::core::floor(q[0] / BELT) as i64, crate::core::floor(q[1] / BELT) as i64);
            let e = at.entry(c).or_insert(q);
            if dist(q, mark(c)) < dist(*e, mark(c)) {
                *e = q;
            }
        }
        for (&(ci, cj), &q) in &at {
            let rng = &mut Pcg32::new(hash3(key ^ 0xbe18, ci, cj, 17), 47);
            let kind = pick(rng, trees);
            let r = 0.75 * foot(kind);
            if rng.next_f64() < 0.3 && clear.free(q, r, 3.0) {
                clear.disc(q, r);
                out.props.push((q, 0.0, kind, rng.below(8) as u8, rng.range(0.9, 1.15) as f32));
            }
        }
    }
    unseal(poly, &mut out, 0);
    out
}
