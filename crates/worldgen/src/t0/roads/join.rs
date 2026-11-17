//! Joining near-parallel roads: realm keep-outs, near-line tests, crossings that join.

use super::*;

/// How far apart (feet) two of a realm's villages' tracks join its track, and how far a join keeps off a
/// bend of the track sharper than `JOIN_BEND_DEG` (a spur at a bend reads as a fork of the track).
pub(super) const JOIN_APART_FT: f64 = 1.5 * 5280.0;
const JOIN_BEND_DEG: f64 = 20.0;
const JOIN_BEND_FT: f64 = 5280.0;

/// The cells of a realm's track (in order) within `JOIN_BEND_FT` of a bend over `JOIN_BEND_DEG`, the turn
/// measured between chords of that length either side.
pub(super) fn track_bends(track: &[u32], w: usize, cell: f64) -> crate::core::hash::FastSet<usize> {
    let xy = |c: u32| ((c as usize % w) as f64, (c as usize / w) as f64);
    let mut cum = vec![0.0f64];
    for q in track.windows(2) {
        let (a, b) = (xy(q[0]), xy(q[1]));
        cum.push(cum[cum.len() - 1] + crate::core::sqrt((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)) * cell);
    }
    let at = |s: f64| track[cum.partition_point(|&v| v < s).min(track.len() - 1)];
    let total = cum[cum.len() - 1];
    let mut bends: Vec<(f64, f64)> = Vec::new();
    for (i, &c) in track.iter().enumerate() {
        let (s0, p) = (cum[i], xy(c));
        if s0 < JOIN_BEND_FT || s0 + JOIN_BEND_FT > total {
            continue;
        }
        let (a, b) = (xy(at(s0 - JOIN_BEND_FT)), xy(at(s0 + JOIN_BEND_FT)));
        let (ux, uy, vx, vy) = (p.0 - a.0, p.1 - a.1, b.0 - p.0, b.1 - p.1);
        let m = crate::core::sqrt(ux * ux + uy * uy) * crate::core::sqrt(vx * vx + vy * vy);
        if m > 1e-9 && (ux * vx + uy * vy) / m < libm::cos(JOIN_BEND_DEG.to_radians()) {
            bends.push(p);
        }
    }
    let r = JOIN_BEND_FT / cell;
    track.iter().filter(|&&c| bends.iter().any(|b| (xy(c).0 - b.0) * (xy(c).0 - b.0) + (xy(c).1 - b.1) * (xy(c).1 - b.1) < r * r)).map(|&c| c as usize).collect()
}

/// How far (cells) round a realm's town its villages' tracks keep off, joining its track beyond.
pub(super) fn realm_keep_out(s: &Settlement, cell: f64) -> f64 {
    (crate::t0::radii::road_trim_radius(s.planned, s.planned_pop) + 1.5 * 5280.0) / cell + 2.0
}

/// The realm (its town's index) an isolated village belongs to: the nearest isolated town.
pub fn realm_of(settlements: &[Settlement], i: usize) -> Option<usize> {
    let s = &settlements[i];
    if !s.isolated {
        return None;
    }
    if s.planned >= Tier::Town {
        return Some(i);
    }
    let d = |r: &Settlement| (r.x - s.x) * (r.x - s.x) + (r.y - s.y) * (r.y - s.y);
    (0..settlements.len()).filter(|&r| settlements[r].isolated && settlements[r].planned >= Tier::Town).min_by(|&a, &b| d(&settlements[a]).total_cmp(&d(&settlements[b])).then(a.cmp(&b)))
}

/// Distance (cells) from a point to a polyline.
pub(super) fn line_dist(p: [f64; 2], line: &[[f64; 2]]) -> f64 {
    line.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}

/// Whether `p` is within `tol` of the polyline (`line_dist(p, line) < tol`, skipping segments
/// whose box is farther off).
pub(super) fn near_line(p: [f64; 2], line: &[[f64; 2]], tol: f64) -> bool {
    line.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        p[0] >= a[0].min(b[0]) - tol && p[0] <= a[0].max(b[0]) + tol && p[1] >= a[1].min(b[1]) - tol && p[1] <= a[1].max(b[1]) + tol && seg_dist(p, a, b) < tol
    })
}

pub(super) fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    dist(p, [a[0] + t * dx, a[1] + t * dy])
}

/// Whether segment a–b crosses the polyline.
/// Two roads that cross away from settlements (routes laid across each other between grid
/// cells, never sharing one) meet there: both are cut at the crossing, a junction of four arms
/// that the passes after square (a crossroads, or Ts) or drop. Drawn roads are left as they are.
/// Segments are bucketed by `cell` squares, O(segments).
pub(super) fn join_crossings(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], cell: f64) {
    let mut keyed: Vec<((i64, i64), usize, usize)> = Vec::new();
    for (ri, r) in roads.iter().enumerate().filter(|(_, r)| r.stroke.is_none() && r.pts.len() >= 2) {
        for k in 0..r.pts.len() - 1 {
            let (a, b) = (r.pts[k], r.pts[k + 1]);
            let (x0, x1) = (crate::core::floor(a[0].min(b[0]) / cell) as i64, crate::core::floor(a[0].max(b[0]) / cell) as i64);
            let (y0, y1) = (crate::core::floor(a[1].min(b[1]) / cell) as i64, crate::core::floor(a[1].max(b[1]) / cell) as i64);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    keyed.push(((x, y), ri, k));
                }
            }
        }
    }
    keyed.sort_unstable();
    let mut cuts: Vec<Vec<(f64, [f64; 2])>> = vec![Vec::new(); roads.len()];
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let mut i = 0;
    while i < keyed.len() {
        let mut e = i;
        while e < keyed.len() && keyed[e].0 == keyed[i].0 {
            e += 1;
        }
        for u in i..e {
            for v in u + 1..e {
                let ((_, ra, ka), (_, rb, kb)) = (keyed[u], keyed[v]);
                if ra == rb {
                    continue;
                }
                let (a, b, c, d) = (roads[ra].pts[ka], roads[ra].pts[ka + 1], roads[rb].pts[kb], roads[rb].pts[kb + 1]);
                let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
                if d1 * d2 >= 0.0 || d3 * d4 >= 0.0 {
                    continue;
                }
                let t = d3 / (d3 - d4);
                let x = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                // (Not in a settlement, nor at a road's end: a junction is there already.)
                let at_end = |r: &RoadPath| dist(x, r.pts[0]) < 30.0 || dist(x, r.pts[r.pts.len() - 1]) < 30.0;
                if towns.iter().any(|&(tc, tr)| dist(x, tc) < tr) || at_end(&roads[ra]) || at_end(&roads[rb]) {
                    continue;
                }
                let along = |r: &RoadPath, k: usize| r.pts[..=k].windows(2).map(|w| dist(w[0], w[1])).sum::<f64>() + dist(r.pts[k], x);
                cuts[ra].push((along(&roads[ra], ka), x));
                cuts[rb].push((along(&roads[rb], kb), x));
            }
        }
        i = e;
    }
    if cuts.iter().all(|c| c.is_empty()) {
        return;
    }
    let mut out = Vec::with_capacity(roads.len());
    for (ri, r) in roads.drain(..).enumerate() {
        let mut c = std::mem::take(&mut cuts[ri]);
        if c.is_empty() {
            out.push(r);
            continue;
        }
        c.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1[0].total_cmp(&q.1[0])).then(p.1[1].total_cmp(&q.1[1])));
        c.dedup_by(|p, q| p.1 == q.1);
        let s = arc(&r.pts);
        for mut piece in split_road(&r, &s, &c) {
            // (Where they meet is fixed: no wander.)
            let n = piece.pts.len();
            if c.iter().any(|q| q.1 == piece.pts[0]) {
                piece.wander[0] = 0.0;
            }
            if c.iter().any(|q| q.1 == piece.pts[n - 1]) {
                piece.wander[n - 1] = 0.0;
            }
            out.push(piece);
        }
    }
    *roads = out;
}

pub(super) fn crosses(a: [f64; 2], b: [f64; 2], line: &[[f64; 2]]) -> bool {
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    line.windows(2).any(|w| side(a, b, w[0]) * side(a, b, w[1]) < 0.0 && side(w[0], w[1], a) * side(w[0], w[1], b) < 0.0)
}
