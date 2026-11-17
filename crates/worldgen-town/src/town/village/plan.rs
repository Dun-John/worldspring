//! A village’s plan drawing: runs, bounds, stamps, walls, ticks and lanes.

use super::*;

pub(super) fn village_of(l: &Layout) -> bool {
    l.tier == Tier::Village && !l.site
}

/// A plot's boundary as runs (hedge, fence or wall): a croft's sides and back, a yard's whole round
/// but its gate.
pub(super) fn runs(p: &Plot) -> Vec<(P, P)> {
    let n = p.pts.len();
    let mut out = Vec::new();
    for k in 0..n {
        let (a, b) = (p.pts[k], p.pts[(k + 1) % n]);
        if k == 0 {
            if p.kind == PlotKind::Croft {
                continue;
            }
            let e = dist(a, b);
            if e > GATE_FT + 4.0 {
                let h = 0.5 * GATE_FT / e;
                out.push((a, lerp(a, b, 0.5 - h)));
                out.push((lerp(a, b, 0.5 + h), b));
            }
            continue;
        }
        out.push((a, b));
    }
    out
}

/// Each plot's hedges, fences and walls as drawn (`runs`), but where they run along a plot's before it
/// (within 3 ft, pieces of 4 ft or so): one hedge between two crofts, not two side by side.
pub fn bounds(l: &Layout) -> Vec<(P, P, Bound, PlotKind)> {
    let mut out = Vec::new();
    let boxes: Vec<[f64; 4]> = l.crofts.iter().map(|p| bbox(&p.pts)).collect();
    for (k, p) in l.crofts.iter().enumerate() {
        let shared = |q: P| {
            (0..k).any(|j| {
                let (b, o) = (boxes[j], &l.crofts[j].pts);
                q[0] > b[0] - 3.0 && q[0] < b[2] + 3.0 && q[1] > b[1] - 3.0 && q[1] < b[3] + 3.0 && (0..o.len()).any(|i| seg_dist(q, o[i], o[(i + 1) % o.len()]) < 3.0)
            })
        };
        for (a, b) in runs(p) {
            let m = (dist(a, b) / 4.0).ceil().max(1.0) as usize;
            let mut from: Option<usize> = None;
            for j in 0..=m {
                let keep = j < m && !shared(lerp(a, b, (j as f64 + 0.5) / m as f64));
                match (keep, from) {
                    (true, None) => from = Some(j),
                    (false, Some(f)) => {
                        out.push((lerp(a, b, f as f64 / m as f64), lerp(a, b, j as f64 / m as f64), p.bound, p.kind));
                        from = None;
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

/// The church's way from the churchyard's gate to its door (a 6-ft strip), if it stands in `p`.
fn church_path(l: &Layout, p: &Plot) -> Option<Vec<P>> {
    let b = l.buildings.iter().find(|b| temple::shaped(b) && contains(&p.pts, centroid(&b.poly)))?;
    let door = mul(add(b.parts[0].poly[0], b.parts[0].poly[3]), 0.5);
    let gate = lerp(p.pts[0], p.pts[1], 0.5);
    let dl = dist(door, gate);
    (dl > 1.0).then(|| rect(lerp(gate, door, 0.5), sub(door, gate), dl + 3.0, 6.0))
}

/// A village's plots on the battlemap's ground (`battlemap`'s settlement codes): crofts and yards
/// `URBAN_CROFT`, a churchyard `URBAN_GRAVEYARD` (its headstones) with its way to the door trodden
/// bare.
pub fn stamp(l: &Layout, put: &mut dyn FnMut(&[P], u8)) {
    if !village_of(l) {
        return;
    }
    for p in &l.crofts {
        if p.kind == PlotKind::Churchyard {
            // (Its headstones kept off the wall: a band of grass inside it.)
            put(&p.pts, URBAN_CROFT);
            let inner = inset(&p.pts, &[5.0; 4]);
            if inner.len() >= 3 {
                put(&inner, URBAN_GRAVEYARD);
            }
            if let Some(path) = church_path(l, p) {
                put(&path, URBAN_YARD);
            }
        } else {
            put(&p.pts, URBAN_CROFT);
        }
    }
}

/// The low walls of a village's walled yards (the churchyard's, a walled manor's), as the battlemap
/// raises a palace forecourt's.
pub fn wall_runs(l: &Layout) -> Vec<(P, P)> {
    if !village_of(l) {
        return Vec::new();
    }
    // (A churchyard's wall is a low one, seen over: `rural::battle` draws it as a dry-stone run.)
    l.crofts.iter().filter(|p| p.bound == Bound::Wall && p.kind != PlotKind::Churchyard).flat_map(runs).collect()
}

/// A village's plan zoom polygons (`payload::pack_sites`, before its buildings): its green's well
/// and the churchyard's wall. The green's and a churchyard's grass are the land's own (no fill, so
/// they match the biome's ground round them); the crofts and their bounds are seen on the
/// battlemap only.
pub fn plan_polys(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    if !village_of(l) {
        return;
    }
    for pl in l.plazas.iter().filter(|p| p.kind == PlazaKind::Green) {
        push(&circle(centroid(&pl.pts), 3.0, 10), PLAN_STONE);
    }
    for (a, b, bound, _) in bounds(l).into_iter().filter(|b| b.3 == PlotKind::Churchyard) {
        let e = dist(a, b);
        if bound == Bound::Wall && e > 0.5 {
            push(&rect(lerp(a, b, 0.5), sub(b, a), e + WALL_FT, WALL_FT), PLAN_WALL);
        }
    }
}

/// A village's headstones at plan zoom (after its buildings): rows of ticks across each churchyard
/// (off the church and its way to the door) and each graveyard of a shrine's yard.
pub fn plan_ticks(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    if !village_of(l) {
        return;
    }
    let near_roofed = |q: P| {
        l.buildings.iter().filter(|b| b.structure == Structure::Roofed).any(|b| {
            let bb = bbox(&b.poly);
            q[0] > bb[0] - 4.0 && q[0] < bb[2] + 4.0 && q[1] > bb[1] - 4.0 && q[1] < bb[3] + 4.0 && (contains(&b.poly, q) || b.poly.iter().zip(b.poly.iter().cycle().skip(1)).any(|(a, c)| seg_dist(q, *a, *c) < 3.0))
        })
    };
    let mut rows = |frame: &[P], skip_axis: bool, salt: i64| {
        let (f0, t, n) = (frame[0], sub(frame[1], frame[0]), sub(frame[3], frame[0]));
        let (w, d) = (len(t), len(n));
        if w < 6.0 || d < 6.0 {
            return;
        }
        let (tu, nu) = (mul(t, 1.0 / w), mul(n, 1.0 / d));
        // (Rows 7 ft apart, 4.5 ft along them, each stone a foot or so off its place and a little
        // turned; 5 ft in from a churchyard's wall.)
        let m = if skip_axis { 5.0 } else { 2.5 };
        let (mut row, mut s) = (0i64, m + 1.5);
        while s < d - m {
            let (mut col, mut x) = (0i64, m + 1.0);
            while x < w - m {
                let h = |k: i64| unit(hash3(l.index as u64, salt * 4096 + row, col, 0x6e57 + k));
                let q = add(f0, add(mul(tu, x + h(1) - 0.5), mul(nu, s + 1.6 * (h(2) - 0.5))));
                let turn = 0.25 * (h(3) - 0.5);
                let dir = add(tu, mul(nu, turn));
                if h(0) < 0.85 && !(skip_axis && (x - 0.5 * w).abs() < 5.0) && !near_roofed(q) {
                    push(&rect(q, dir, 2.4, 1.0), PLAN_HEADSTONE);
                }
                x += 4.5;
                col += 1;
            }
            s += 7.0;
            row += 1;
        }
    };
    for (k, p) in l.crofts.iter().enumerate() {
        if p.kind == PlotKind::Churchyard {
            rows(&p.pts, true, k as i64);
        }
    }
    // (A shrine's graveyard: the open building in its yard.)
    for (k, b) in l.buildings.iter().enumerate() {
        if b.structure == Structure::Open && b.poly.len() == 4 && l.crofts.iter().any(|p| p.kind == PlotKind::ShrineYard && contains(&p.pts, centroid(&b.poly))) {
            rows(&b.poly, false, 1000 + k as i64);
        }
    }
}

/// A village's lanes off the roads (`Layout::streets`) as plan zoom draws roads (`payload`: x, y,
/// width, class): narrow dirt tracks.
pub fn plan_lanes(l: &Layout) -> Vec<Vec<[f64; 4]>> {
    if !village_of(l) {
        return Vec::new();
    }
    let track = crate::t0::roads::RoadClass::Track as u8 as f64;
    l.streets.iter().map(|s| s.iter().map(|p| [p[0], p[1], LANE_W, track]).collect()).collect()
}

/// How far `p` (world ft) is from a village's lanes (a croft's front, a lane's edge): where a
/// cottage's door faces (`interior`); far for any other layout.
pub fn front_d(l: &Layout, p: P) -> f64 {
    if !village_of(l) {
        return f64::MAX;
    }
    let fronts = l.crofts.iter().map(|c| seg_dist(p, c.pts[0], c.pts[1]));
    let lanes = l.streets.iter().flat_map(|s| s.windows(2).map(move |w| seg_dist(p, w[0], w[1]) - 5.0));
    fronts.chain(lanes).fold(f64::MAX, f64::min)
}
