//! Final per-road passes: tidy, swept turns, eased turns, keeping dry, cutting spikes.

use super::*;

/// Drops control points (ft) that only make the road jog: closer than 40 ft to the point
/// before or after, or where it doubles back (turns over 100°) within 250 ft; never an end,
/// and only where the chord left keeps the road's grade.
pub fn tidy(r: &mut RoadPath) {
    cut_spikes(r);
    let gmax = r.class.max_grade();
    // The first and last stretch stay as shaped (a branch's way into a junction, the way into a gate).
    let mut k = 2;
    while k + 2 < r.pts.len() {
        let (a, b, c) = (r.pts[k - 1], r.pts[k], r.pts[k + 1]);
        let (ab, bc) = (dist(a, b), dist(b, c));
        let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
        let back = u[0] * v[0] + u[1] * v[1] < -0.17 * ab * bc;
        let jog = ab.min(bc) < 40.0 || (back && ab.min(bc) < 250.0);
        if jog && ((r.z[k + 1] - r.z[k - 1]).abs() as f64) <= gmax * dist(a, c) {
            r.pts.remove(k);
            r.z.remove(k);
            r.wander.remove(k);
            k = k.saturating_sub(1).max(2);
        } else {
            k += 1;
        }
    }
}

/// Sweeps a road's turns round (ft) at map scale: each point moves toward the average of the
/// road within `sweep_radius` of it along the way (a biweight over distance along the road), so
/// a turn bends over hundreds of feet instead of at a point, and the map line, the battlemaps
/// and the town approaches (all drawn from these points) follow one curve. The reach shrinks to
/// 0.6 where the road climbs near its class's grade (it keeps to the contours it was routed on;
/// its graded bed takes the rest, item 168) and to 0.35
/// round hairpins (switchbacks keep their legs apart), and fades out over the first and last
/// stretch (a junction's or a gate's way in stays; a short road sweeps its middle third) and at
/// river crossings (still square over the water). Tried at full and half reach; left as it is
/// where a point would go wet or the road would cross rivers (`rivers`: where, with the belt's
/// half width there) a different number of times. Drawn roads keep their line. (The profile is
/// fitted again afterwards.)
pub fn sweep_turns(r: &mut RoadPath, cell: f64, dry: &dyn Fn(f64, f64) -> bool, rivers: &dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>) {
    let n = r.pts.len();
    if r.stroke.is_some() || n < 5 {
        return;
    }
    let mut s = vec![0.0; n];
    for k in 1..n {
        s[k] = s[k - 1] + dist(r.pts[k - 1], r.pts[k]);
    }
    let total = s[n - 1];
    // (A short road sweeps less: its middle third.)
    let reach = sweep_radius(r.class, cell).min(total / 6.0);
    if reach < 100.0 {
        return;
    }
    // Each point's share of the road (half the segments either side).
    let ds: Vec<f64> = (0..n).map(|k| 0.5 * (s[(k + 1).min(n - 1)] - s[k.saturating_sub(1)])).collect();
    // The point `d` ft along from `k` (dir ±1), clamped to the road.
    let at = |k: usize, d: f64| {
        let mut j = k;
        if d >= 0.0 {
            while j + 1 < n && s[j] - s[k] < d {
                j += 1;
            }
        } else {
            while j > 0 && s[k] - s[j] < -d {
                j -= 1;
            }
        }
        j
    };
    let gmax = r.class.max_grade();
    // Per point: the share of `reach` it takes.
    let local: Vec<f64> = (0..n)
        .map(|k| {
            let (a, b) = (at(k, -300.0), at(k, 300.0));
            let grade = if s[b] - s[a] > 1.0 { (r.z[b] - r.z[a]).abs() as f64 / (s[b] - s[a]) } else { 0.0 };
            let mut f = (1.6 - grade / gmax).clamp(0.6, 1.0);
            // (A hairpin: turning over 120° between the headings half a reach either side.)
            let (a, b) = (at(k, -0.5 * reach), at(k, 0.5 * reach));
            if a < k && b > k {
                let (u, v) = (unit2([r.pts[k][0] - r.pts[a][0], r.pts[k][1] - r.pts[a][1]]), unit2([r.pts[b][0] - r.pts[k][0], r.pts[b][1] - r.pts[k][1]]));
                if dot2(u, v) < -0.5 {
                    f = f.min(0.35);
                }
            }
            f
        })
        .collect();
    // Where it crosses rivers (each with the belt's half width there): it keeps its line within
    // a quarter of the belt and 100 ft, eased over half a reach, so it still crosses square
    // (wider protection keeps most of the bends a crossing makes; narrower skews the bridges).
    let wet = rivers(&r.pts);
    let by_river = |p: [f64; 2]| wet.iter().map(|&(c, hw)| smooth01((dist(p, c) - 0.25 * hw - 100.0) / (0.5 * reach))).fold(1.0, f64::min);
    // The way in at either end: past the second point, and a reach (half of it as shaped).
    let (e0, e1) = (s[2].max(reach), (total - s[n - 3]).max(reach));
    for scale in [1.0, 0.5] {
        let moved: Vec<[f64; 2]> = (0..n)
            .map(|k| {
                let fade = smooth01((s[k] - 0.5 * e0) / (0.5 * e0)) * smooth01((total - s[k] - 0.5 * e1) / (0.5 * e1)) * by_river(r.pts[k]);
                let rr = scale * reach * local[k];
                if fade <= 0.0 || rr < 1.0 {
                    return r.pts[k];
                }
                let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
                for j in at(k, -rr)..=at(k, rr) {
                    let u = (s[j] - s[k]) / rr;
                    let w = (1.0 - u * u).max(0.0);
                    let w = w * w * ds[j];
                    (sx, sy, sw) = (sx + w * r.pts[j][0], sy + w * r.pts[j][1], sw + w);
                }
                if sw <= 0.0 {
                    return r.pts[k];
                }
                let p = r.pts[k];
                [p[0] + fade * (sx / sw - p[0]), p[1] + fade * (sy / sw - p[1])]
            })
            .collect();
        if moved.iter().all(|p| dry(p[0], p[1])) && rivers(&moved).len() == wet.len() {
            r.pts = moved;
            return;
        }
    }
}

/// How far along the road (ft) `sweep_turns` averages for a road of `class`: a king's road
/// sweeps widest, a track bends tightest.
pub fn sweep_radius(class: RoadClass, cell: f64) -> f64 {
    [0.32, 0.26, 0.18][class as usize] * cell
}

/// Eases turns still too sharp for a cart (ft): where the road's heading over 40 ft before a
/// point and 40 ft after it differ by over 70°, the stretch from ~0.05 cell before to as far
/// after becomes a cubic keeping the headings it arrives and leaves with (left as it is where
/// that would cross water). Not where that stretch would reach the first or last two points.
pub fn ease_turns(r: &mut RoadPath, cell: f64, dry: &dyn Fn(f64, f64) -> bool) {
    const CHORD: f64 = 40.0;
    const HAIRPIN: f64 = 420.0;
    let reach = 0.05 * cell;
    let mut i = 2;
    while i + 2 < r.pts.len() {
        let n = r.pts.len();
        // The point at least `d` ft along the road from `i` (dir ±1), within [1, n - 2].
        let walk = |i: usize, dir: i64, d: f64| {
            let (mut k, mut acc) = (i as i64, 0.0);
            while acc < d && k + dir >= 1 && k + dir <= n as i64 - 2 {
                acc += dist(r.pts[k as usize], r.pts[(k + dir) as usize]);
                k += dir;
            }
            k as usize
        };
        let (a, b) = (walk(i, -1, CHORD), walk(i, 1, CHORD));
        let (u, v) = (unit2([r.pts[i][0] - r.pts[a][0], r.pts[i][1] - r.pts[a][1]]), unit2([r.pts[b][0] - r.pts[i][0], r.pts[b][1] - r.pts[i][1]]));
        // (A hairpin, turning over 120° between the headings 150 ft either side, already rounded
        // too tight for a cart.)
        let (a2, b2) = (walk(i, -1, 150.0), walk(i, 1, 150.0));
        let (u2, v2) = (unit2([r.pts[i][0] - r.pts[a2][0], r.pts[i][1] - r.pts[a2][1]]), unit2([r.pts[b2][0] - r.pts[i][0], r.pts[b2][1] - r.pts[i][1]]));
        let hairpin = a2 != i && b2 != i && dot2(u2, v2) < -0.5;
        if a == i || b == i || (dot2(u, v) >= libm::cos(70f64.to_radians()) && !hairpin) {
            i += 1;
            continue;
        }
        let (mut i0, mut i1) = (walk(i, -1, reach), walk(i, 1, reach));
        // (A hairpin takes a wider stretch: its legs at least `HAIRPIN` ft apart where it turns,
        // else it turns on the spot.)
        if hairpin {
            for m in [1.5, 2.0, 3.0] {
                if dist(r.pts[i0], r.pts[i1]) >= HAIRPIN {
                    break;
                }
                (i0, i1) = (walk(i, -1, m * reach), walk(i, 1, m * reach));
            }
        }
        // (Near an end the road keeps its way in: a junction's or a gate's.)
        let room = |a: usize, b: usize| (a.min(b)..a.max(b)).map(|k| dist(r.pts[k], r.pts[k + 1])).sum::<f64>() >= reach;
        if !room(i0, i) || !room(i, i1) {
            i += 1;
            continue;
        }
        let (p0, p3) = (r.pts[i0], r.pts[i1]);
        let h0 = unit2([p0[0] - r.pts[i0 - 1][0], p0[1] - r.pts[i0 - 1][1]]);
        let h1 = unit2([r.pts[i1 + 1][0] - p3[0], r.pts[i1 + 1][1] - p3[1]]);
        let span = dist(p0, p3);
        // (A hairpin's turn is round: its handles about two thirds of the span, a half circle.)
        let pull = if hairpin { 0.65 } else { 0.4 } * span;
        let (c1, c2) = ([p0[0] + h0[0] * pull, p0[1] + h0[1] * pull], [p3[0] - h1[0] * pull, p3[1] - h1[1] * pull]);
        let m = (span / 30.0).ceil().clamp(3.0, 40.0) as usize;
        let curve: Vec<[f64; 2]> = (1..m)
            .map(|k| {
                let t = k as f64 / m as f64;
                let w = 1.0 - t;
                let (a, b, c, d) = (w * w * w, 3.0 * w * w * t, 3.0 * w * t * t, t * t * t);
                [a * p0[0] + b * c1[0] + c * c2[0] + d * p3[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p3[1]]
            })
            .collect();
        if i1 <= i0 + 1 || curve.iter().any(|p| !dry(p[0], p[1])) {
            i += 1;
            continue;
        }
        let (z0, z1) = (r.z[i0], r.z[i1]);
        r.pts.splice(i0 + 1..i1, curve);
        r.z.splice(i0 + 1..i1, (1..m).map(|k| z0 + (z1 - z0) * k as f32 / m as f32));
        r.wander.splice(i0 + 1..i1, std::iter::repeat_n(0.0, m - 1));
        i = i0 + m;
    }
}

/// Moves a road (ft) off standing water it still runs into (a lake's water between the cells it
/// was routed on): each run of wet points (not an end) slides sideways, to the side that needs
/// less, by as much as its wettest point needs, eased in and out over `EASE` ft; left as it is
/// where no move within `REACH` ft keeps it dry. One run per call: whether one moved. (The
/// profile is fitted again afterwards.)
pub fn keep_dry(r: &mut RoadPath, dry: &dyn Fn(f64, f64) -> bool) -> bool {
    const REACH: f64 = 1500.0;
    const EASE: f64 = 600.0;
    let n = r.pts.len();
    let wet: Vec<bool> = r.pts.iter().map(|p| !dry(p[0], p[1])).collect();
    let mut k = 1;
    while k + 1 < n {
        if !wet[k] {
            k += 1;
            continue;
        }
        let i0 = k;
        while k + 1 < n && wet[k] {
            k += 1;
        }
        let i1 = k - 1;
        if i1 + 1 >= n - 1 && wet[n - 1] {
            break;
        }
        let normal = |i: usize| {
            let (a, b) = (r.pts[i - 1], r.pts[(i + 1).min(n - 1)]);
            let t = unit2([b[0] - a[0], b[1] - a[1]]);
            [-t[1], t[0]]
        };
        // Per side: the offset the wettest point needs.
        let need = |side: f64| {
            let mut most = 0.0f64;
            for i in i0..=i1 {
                let nrm = normal(i);
                let d = (1..=(REACH / 50.0) as usize).map(|m| m as f64 * 50.0).find(|&d| dry(r.pts[i][0] + side * nrm[0] * d, r.pts[i][1] + side * nrm[1] * d))?;
                most = most.max(d);
            }
            Some(most + 60.0)
        };
        let pick = match (need(1.0), need(-1.0)) {
            (Some(a), Some(b)) => Some(if a <= b { (1.0, a) } else { (-1.0, b) }),
            (Some(a), None) => Some((1.0, a)),
            (None, Some(b)) => Some((-1.0, b)),
            (None, None) => None,
        };
        let Some((side, off)) = pick else {
            continue;
        };
        // Distance along the road from the run, for the easing.
        let mut s = vec![0.0; n];
        for i in 1..n {
            s[i] = s[i - 1] + dist(r.pts[i - 1], r.pts[i]);
        }
        let moved: Vec<(usize, f64, [f64; 2])> = (1..n - 1)
            .filter_map(|i| {
                let gap = if i < i0 { s[i0] - s[i] } else if i > i1 { s[i] - s[i1] } else { 0.0 };
                (gap < EASE).then(|| {
                    // (A point the planned offset leaves wet takes the nearest dry one to it.)
                    let (f, nrm, p) = (1.0 - smooth01(gap / EASE), normal(i), r.pts[i]);
                    let at = |d: f64| [p[0] + side * nrm[0] * d, p[1] + side * nrm[1] * d];
                    let d = (0..=(REACH / 50.0) as usize).flat_map(|m| [off * f + m as f64 * 50.0, off * f - m as f64 * 50.0]).filter(|&d| (0.0..=REACH).contains(&d)).find(|&d| dry(at(d)[0], at(d)[1]));
                    (i, d.unwrap_or(off * f), nrm)
                })
            })
            .collect();
        // Offsets smoothed (raised to their neighbours' mean) where that stays dry: no kinks.
        let place = |m: &[(usize, f64, [f64; 2])]| m.iter().map(|&(i, d, nrm)| (i, [r.pts[i][0] + side * nrm[0] * d, r.pts[i][1] + side * nrm[1] * d])).collect::<Vec<(usize, [f64; 2])>>();
        let mut smooth = moved.clone();
        for _ in 0..3 {
            let prev = smooth.clone();
            for m in 1..smooth.len().saturating_sub(1) {
                smooth[m].1 = prev[m].1.max(0.5 * (prev[m - 1].1 + prev[m + 1].1));
            }
        }
        let (rough, smooth) = (place(&moved), place(&smooth));
        let moved = if smooth.iter().all(|(_, q)| dry(q[0], q[1])) { smooth } else { rough };
        if moved.iter().all(|(_, q)| dry(q[0], q[1])) {
            let (lo, hi) = (moved[0].0, moved[moved.len() - 1].0);
            for (i, q) in moved {
                r.pts[i] = q;
            }
            // Points left jogging there (turning over 45°) go where the chord past them stays dry.
            let chord_dry = |a: [f64; 2], b: [f64; 2]| {
                let m = (dist(a, b) / 50.0).ceil().max(1.0) as usize;
                (0..=m).all(|j| dry(a[0] + (b[0] - a[0]) * j as f64 / m as f64, a[1] + (b[1] - a[1]) * j as f64 / m as f64))
            };
            let mut drop = vec![false; n];
            let mut last = lo - 1;
            for i in lo..=hi {
                let (a, b, c) = (r.pts[last], r.pts[i], r.pts[i + 1]);
                if dot2(unit2(sub2(b, a)), unit2(sub2(c, b))) < libm::cos(45f64.to_radians()) && chord_dry(a, c) {
                    drop[i] = true;
                } else {
                    last = i;
                }
            }
            let keep: Vec<usize> = (0..n).filter(|&i| !drop[i]).collect();
            r.pts = keep.iter().map(|&i| r.pts[i]).collect();
            r.z = keep.iter().map(|&i| r.z[i]).collect();
            r.wander = keep.iter().map(|&i| r.wander[i]).collect();
            return true;
        }
    }
    false
}

fn smooth01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Cuts out-and-back spikes (ft): where a road turns back (over ~155°) and retraces its way
/// within 60 ft (two roads that climbed the same switchbacks, joined into one), the stretch
/// both ways is dropped (and `EASE` ft more each way) and the road goes on from where they
/// part; never at an end. (The profile is fitted again afterwards.)
fn cut_spikes(r: &mut RoadPath) {
    const TOL: f64 = 60.0;
    const EASE: f64 = 200.0;
    let mut k = 1;
    while k + 1 < r.pts.len() {
        let (u, v) = (unit2([r.pts[k][0] - r.pts[k - 1][0], r.pts[k][1] - r.pts[k - 1][1]]), unit2([r.pts[k + 1][0] - r.pts[k][0], r.pts[k + 1][1] - r.pts[k][1]]));
        // (A hairpin's two legs part at once: not a spike.)
        if dot2(u, v) > -0.9 || dist(r.pts[k - 1], r.pts[k + 1]) >= TOL {
            k += 1;
            continue;
        }
        let (mut i, mut j) = (k - 1, k + 1);
        loop {
            if i >= 1 && j + 1 < r.pts.len() && seg_dist_or_start(r.pts[i - 1], r.pts[j], r.pts[j + 1]) < TOL {
                i -= 1;
            } else if i >= 1 && j + 1 < r.pts.len() && seg_dist_or_start(r.pts[j + 1], r.pts[i - 1], r.pts[i]) < TOL {
                j += 1;
            } else {
                break;
            }
        }
        if i == 0 || j + 1 >= r.pts.len() {
            k += 1;
            continue;
        }
        // A little farther each way (the stubs left would turn too tightly to round).
        let (mut back, mut on) = (0.0, 0.0);
        while i > 1 && back < EASE {
            back += dist(r.pts[i - 1], r.pts[i]);
            i -= 1;
        }
        while j + 2 < r.pts.len() && on < EASE {
            on += dist(r.pts[j], r.pts[j + 1]);
            j += 1;
        }
        r.pts.drain(i + 1..j);
        r.z.drain(i + 1..j);
        r.wander.drain(i + 1..j);
        k = i.max(1);
    }
}

pub(super) use crate::core::geom::dist;
