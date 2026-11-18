//! Network cleanup: shortcuts, triangles, twin links, lenses, parallel roads, fillets.

use super::*;
use crate::core::geom::bbox;

pub(super) fn unit2(v: [f64; 2]) -> [f64; 2] {
    let l = crate::core::sqrt(v[0] * v[0] + v[1] * v[1]).max(1e-9);
    [v[0] / l, v[1] / l]
}

pub(super) fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// Where a settlement or junction is.
pub(super) fn end_key(p: [f64; 2], towns: &[([f64; 2], f64)]) -> (i64, i64) {
    match towns.iter().position(|&(c, r)| dist(c, p) < 1.1 * r + 50.0) {
        Some(t) => (i64::MIN, t as i64),
        None => ((p[0] / 10.0).round() as i64, (p[1] / 10.0).round() as i64),
    }
}

pub(super) fn road_len(r: &RoadPath) -> f64 {
    r.pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Two roads into the same town from junctions joined by a third make a loop just outside it:
/// a road from junction X into a town goes where the way through a neighbouring junction Y
/// (X–Y, then Y's road into the same town) is under 30% longer.
pub(super) fn drop_shortcuts(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) {
    let ends: Vec<Option<((i64, i64), (i64, i64))>> = roads.iter().map(|r| (r.pts.len() >= 2).then(|| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns)))).collect();
    let town = |k: (i64, i64)| k.0 == i64::MIN;
    let lens: Vec<f64> = roads.iter().map(road_len).collect();
    // Shortest road between two ends.
    let link = |a: (i64, i64), b: (i64, i64), skip: usize| {
        (0..roads.len()).filter(|&i| i != skip).filter_map(|i| ends[i].filter(|e| (e.0 == a && e.1 == b) || (e.0 == b && e.1 == a)).map(|_| lens[i])).fold(f64::MAX, f64::min)
    };
    let mut gone = vec![false; roads.len()];
    for i in 0..roads.len() {
        let Some((a, b)) = ends[i] else { continue };
        let (x, z) = match (town(a), town(b)) {
            (false, true) => (a, b),
            (true, false) => (b, a),
            _ => continue,
        };
        // Neighbouring junctions Y of X with a road into z.
        let detour = (0..roads.len())
            .filter(|&k| k != i && !gone[k])
            .filter_map(|k| ends[k].and_then(|(p, q)| if p == x && !town(q) { Some((q, lens[k])) } else if q == x && !town(p) { Some((p, lens[k])) } else { None }))
            .map(|(y, xy)| xy + link(y, z, i))
            .fold(f64::MAX, f64::min);
        if detour < 1.3 * lens[i] && roads[i].stroke.is_none() && roads[i].class != RoadClass::KingsRoad {
            gone[i] = true;
        }
    }
    let mut i = 0;
    roads.retain(|_| {
        i += 1;
        !gone[i - 1]
    });
}

/// How round a small loop may be (ft) for one of its lesser roads to go, and how much longer the
/// way round it may then be.
const TRIANGLE_FT: f64 = 4.0 * 5280.0;
const TRIANGLE_SLACK: f64 = 1.6;
/// The same for a link between two roads into one place.
const TRIANGLE_LINK: f64 = 3.0;
/// How round a loop of three roads between junctions alone (no place at its corners) may be: a
/// rung between two roads that meet a third reads as one too many further out (the way round
/// at most `TRIANGLE_OPEN_SLACK` times as long).
const TRIANGLE_OPEN_FT: f64 = 6.0 * 5280.0;
const TRIANGLE_OPEN_SLACK: f64 = 1.5;

/// Three roads closing a small loop (two lesser roads into one place and a short link between
/// them, or a lesser road's two arms out to the same road) read as a ladder's rung: one lesser
/// road of it goes, a link between two roads into one place first, else the one whose way round
/// (over roads at least as good) is shortest for its length. Drawn roads and king's roads stay.
/// Returns whether one went.
pub(super) fn drop_triangle(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) -> bool {
    let ends: Vec<((i64, i64), (i64, i64))> = roads.iter().map(|r| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns))).collect();
    let lens: Vec<f64> = roads.iter().map(road_len).collect();
    let mut at: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, e) in ends.iter().enumerate() {
        if e.0 != e.1 {
            at.entry(e.0).or_default().push(i);
            at.entry(e.1).or_default().push(i);
        }
    }
    let far = |i: usize, n: (i64, i64)| if ends[i].0 == n { ends[i].1 } else { ends[i].0 };
    let mut best: Option<(f64, usize)> = None;
    for (i, r) in roads.iter().enumerate() {
        let (a, b) = ends[i];
        if r.stroke.is_some() || r.class == RoadClass::KingsRoad || a == b || lens[i] > TRIANGLE_OPEN_FT {
            continue;
        }
        for &k in &at[&a] {
            let c = far(k, a);
            if k == i || c == b || roads[k].class > r.class {
                continue;
            }
            for &m in at.get(&c).map(|v| v.as_slice()).unwrap_or(&[]) {
                if m == i || m == k || far(m, c) != b || roads[m].class > r.class {
                    continue;
                }
                let round = lens[k] + lens[m];
                let ratio = round / lens[i];
                // (A link between two roads into one place goes first: the place keeps both ways out.)
                let link = c.0 == i64::MIN && a.0 != i64::MIN && b.0 != i64::MIN;
                let (most, rank) = if link { (TRIANGLE_LINK, ratio - TRIANGLE_LINK) } else { (TRIANGLE_SLACK, ratio) };
                let open = a.0 != i64::MIN && b.0 != i64::MIN && c.0 != i64::MIN;
                let small = lens[i] + round <= TRIANGLE_FT;
                if (small || (open && lens[i] + round <= TRIANGLE_OPEN_FT && ratio <= TRIANGLE_OPEN_SLACK)) && ratio <= most && best.is_none_or(|(q, _)| rank < q) {
                    best = Some((rank, i));
                }
            }
        }
    }
    let Some((_, i)) = best else { return false };
    roads.remove(i);
    true
}

/// A village with two lesser roads that both run out to the same better road, at junctions apart (a
/// triangle too big for `drop_triangle`), has one road too many: the worse (then longer) of the two
/// goes when the way round (the other road and the better road between them) is no more than
/// `TRIANGLE_SLACK` times as long. Returns whether one went.
pub(super) fn drop_twin_link(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], small: &[bool]) -> bool {
    type K = (i64, i64);
    let ends: Vec<(K, K)> = roads.iter().map(|r| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns))).collect();
    let lens: Vec<f64> = roads.iter().map(road_len).collect();
    // Where a point lies along a king's road (ft from its start), if it is on it.
    let along = |king: &RoadPath, p: [f64; 2]| -> Option<f64> {
        let mut run = 0.0;
        for w in king.pts.windows(2) {
            let l = dist(w[0], w[1]);
            if l > 0.0 {
                let t = (((p[0] - w[0][0]) * (w[1][0] - w[0][0]) + (p[1] - w[0][1]) * (w[1][1] - w[0][1])) / (l * l)).clamp(0.0, 1.0);
                let q = [w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1])];
                if dist(q, p) < 60.0 {
                    return Some(run + t * l);
                }
            }
            run += l;
        }
        None
    };
    // (Each road's box: a point `along` it lies within 60 ft of that.)
    let boxes: Vec<[f64; 4]> = roads.iter().map(|r| r.pts.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])).collect();
    let in_box = |k: usize, p: [f64; 2]| p[0] >= boxes[k][0] - 60.0 && p[0] <= boxes[k][2] + 60.0 && p[1] >= boxes[k][1] - 60.0 && p[1] <= boxes[k][3] + 60.0;
    // The roads leaving each town (by index), each ending there once.
    let mut by_town: Vec<Vec<usize>> = vec![Vec::new(); towns.len()];
    for (i, r) in roads.iter().enumerate() {
        if r.stroke.is_some() || r.class == RoadClass::KingsRoad || ends[i].0 == ends[i].1 {
            continue;
        }
        for e in [ends[i].0, ends[i].1] {
            if e.0 == i64::MIN && (e.1 as usize) < towns.len() {
                by_town[e.1 as usize].push(i);
            }
        }
    }
    let mut best: Option<(f64, usize)> = None;
    for t in 0..towns.len() {
        if !small.get(t).copied().unwrap_or(false) {
            continue;
        }
        let tk: K = (i64::MIN, t as i64);
        let mine = &by_town[t];
        // The road's far end, the road itself leaving the village at its start.
        let far = |i: usize| if ends[i].0 == tk { *roads[i].pts.last().unwrap() } else { roads[i].pts[0] };
        for &i in mine {
            for &k in mine {
                if k == i || roads[k].class > roads[i].class || (roads[k].class == roads[i].class && (lens[k], k) > (lens[i], i)) {
                    continue;
                }
                let (pb, pa) = (far(i), far(k));
                for (q, king) in roads.iter().enumerate().filter(|(_, r)| r.class < roads[i].class) {
                    if !in_box(q, pa) || !in_box(q, pb) {
                        continue;
                    }
                    let (Some(sa), Some(sb)) = (along(king, pa), along(king, pb)) else { continue };
                    if (sa - sb).abs() < 1.0 {
                        continue;
                    }
                    // The way round over road `k` and the king's road between them.
                    let ratio = (lens[k] + (sa - sb).abs()) / lens[i];
                    if ratio <= TRIANGLE_SLACK && best.is_none_or(|(q, _)| ratio < q) {
                        best = Some((ratio, i));
                    }
                }
            }
        }
    }
    let Some((_, i)) = best else { return false };
    roads.remove(i);
    true
}

/// A lesser road that crosses a better one and then runs beside it to where that one ends (round
/// a town, where it may not join it, it crossed over to a join outside) cuts the better road's
/// corner: it ends where it crosses it instead, a junction there (to be squared clear of rivers).
/// So does one that runs beside a better feeder out of a settlement or junction and crosses the
/// road that feeder ends on (a track alongside a town's road to the king's road and over it).
/// Returns those junctions.
pub(super) fn end_at_crossing(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) -> Vec<[f64; 2]> {
    const NEAR: f64 = 5280.0;
    const FEED_FT: f64 = 3.0 * 5280.0;
    let mut cuts: Vec<[f64; 2]> = Vec::new();
    let in_town = |p: [f64; 2]| towns.iter().any(|&(c, r)| dist(c, p) < 1.2 * r + 100.0);
    // Where segment a–b crosses c–d: the fractions along each.
    let meet = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (r, q) = (sub2(b, a), sub2(d, c));
        let den = r[0] * q[1] - r[1] * q[0];
        if den.abs() < 1e-9 {
            return None;
        }
        let w = sub2(c, a);
        let (u, v) = ((w[0] * q[1] - w[1] * q[0]) / den, (w[0] * r[1] - w[1] * r[0]) / den);
        ((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)).then_some((u, v))
    };
    // Per road, its end keys and bounds (kept up to date as roads are cut).
    let key_of = |r: &RoadPath| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns));
    let box_of = |r: &RoadPath| bbox(&r.pts);
    let mut keys: Vec<((i64, i64), (i64, i64))> = roads.iter().map(key_of).collect();
    let mut boxes: Vec<[f64; 4]> = roads.iter().map(box_of).collect();
    for _ in 0..50 {
        let mut cut: Option<(usize, usize, f64, f64, [f64; 2], bool)> = None;
        // Whether road `k` may pass by a point (its bounds round it).
        let by = |k: usize, p: [f64; 2]| p[0] >= boxes[k][0] - 60.0 && p[0] <= boxes[k][2] + 60.0 && p[1] >= boxes[k][1] - 60.0 && p[1] <= boxes[k][3] + 60.0;
        'find: for (i, r) in roads.iter().enumerate() {
            if r.stroke.is_some() || r.class == RoadClass::KingsRoad || r.pts.len() < 3 {
                continue;
            }
            let rs = arc(&r.pts);
            for at_end in [true, false] {
                let e = if at_end { *r.pts.last().unwrap() } else { r.pts[0] };
                let ek = end_key(e, towns);
                let at_town = ek.0 == i64::MIN;
                // (Or a better road from there ends on the one it crosses, `r` beside that one out
                // to it: the feeder, its far end, its length.)
                let head = r.pts.iter().zip(rs.iter()).filter(|&(_, &sr)| (if at_end { rs[rs.len() - 1] - sr } else { sr }) <= FEED_FT + 1_000.0).fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, (p, _)| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
                let feeders: Vec<(usize, [f64; 2], f64)> = (0..roads.len())
                    .filter(|&f| f != i && roads[f].class < r.class && (keys[f].0 == ek) != (keys[f].1 == ek))
                    .map(|f| {
                        let e = (keys[f].0 == ek) as usize;
                        (f, if e == 1 { *roads[f].pts.last().unwrap() } else { roads[f].pts[0] }, road_len(&roads[f]))
                    })
                    .collect();
                for (k, t) in roads.iter().enumerate() {
                    if k == i || t.class >= r.class || t.pts.len() < 2 {
                        continue;
                    }
                    let direct = !at_town && (keys[k].0 == ek || keys[k].1 == ek);
                    let near_t: Vec<&(usize, [f64; 2], f64)> = if direct { Vec::new() } else { feeders.iter().filter(|q| q.0 != k && by(k, q.1)).collect() };
                    if !direct && near_t.is_empty() {
                        continue;
                    }
                    let ts = arc(&t.pts);
                    // (Where each feeder ends on `t`, which, how long.)
                    let feeds: Vec<(f64, usize, f64)> = near_t
                        .iter()
                        .filter_map(|&&(f, far, lf)| {
                            let (s, d) = project(far, &t.pts, &ts);
                            (d < 60.0).then_some((s, f, lf))
                        })
                        .collect();
                    if !direct && feeds.is_empty() {
                        continue;
                    }
                    // The crossing nearest that end.
                    let mut best: Option<(f64, f64, [f64; 2])> = None;
                    // (Beside a feeder, within a few miles of that end: those stretches of `t` by it.)
                    let segs: Vec<usize> = if direct {
                        (0..t.pts.len() - 1).collect()
                    } else {
                        (0..t.pts.len() - 1).filter(|&b| t.pts[b].iter().zip(t.pts[b + 1].iter()).enumerate().all(|(d, (p, q))| p.max(*q) >= head[d] && p.min(*q) <= head[d + 2])).collect()
                    };
                    // (Of all of `t`, only the stretches whose bounds meet the segment's can cross it.)
                    let t_index = direct.then(|| NearLine::new(&t.pts, 0.0, 1_000.0));
                    let mut near_segs: Vec<u32> = Vec::new();
                    for a in 0..r.pts.len() - 1 {
                        if !direct && (if at_end { rs[rs.len() - 1] - rs[a + 1] } else { rs[a] }) > FEED_FT {
                            continue;
                        }
                        let (pa, pb) = (r.pts[a], r.pts[a + 1]);
                        if let Some(ix) = &t_index {
                            ix.cands([pa[0].min(pb[0]), pa[1].min(pb[1])], [pa[0].max(pb[0]), pa[1].max(pb[1])], &mut near_segs);
                        } else {
                            near_segs.clear();
                            near_segs.extend(segs.iter().map(|&b| b as u32));
                        }
                        for b in near_segs.iter().map(|&b| b as usize) {
                            if let Some((u, v)) = meet(r.pts[a], r.pts[a + 1], t.pts[b], t.pts[b + 1]) {
                                let x = [r.pts[a][0] + u * (r.pts[a + 1][0] - r.pts[a][0]), r.pts[a][1] + u * (r.pts[a + 1][1] - r.pts[a][1])];
                                let (sr, st) = (rs[a] + u * (rs[a + 1] - rs[a]), ts[b] + v * (ts[b + 1] - ts[b]));
                                let from_end = if at_end { rs[rs.len() - 1] - sr } else { sr };
                                if from_end >= 100.0 && st >= 100.0 && st <= ts[ts.len() - 1] - 100.0 && best.is_none_or(|q| from_end < q.0) {
                                    best = Some((from_end, st, x));
                                }
                            }
                        }
                    }
                    let Some((from_end, st, x)) = best else { continue };
                    let sr = if at_end { rs[rs.len() - 1] - from_end } else { from_end };
                    let (lo, hi) = if at_end { (sr, rs[rs.len() - 1]) } else { (0.0, sr) };
                    let kept = rs[rs.len() - 1] - from_end;
                    if kept < 1000.0 || in_town(x) {
                        continue;
                    }
                    // The stretch past the crossing keeps beside the better road.
                    let rest = sub_line(&r.pts, &rs, lo, hi);
                    let beside = if direct {
                        let t_rest = if end_key(t.pts[0], towns) == ek { st } else { ts[ts.len() - 1] - st };
                        from_end <= 1.5 * t_rest && rest.iter().all(|p| line_dist(*p, &t.pts) < NEAR)
                    } else {
                        // (Not where the way round over the feeder is much the longer.)
                        feeds.iter().any(|&(s, f, lf)| {
                            let t_rest = lf + (st - s).abs();
                            from_end <= 1.5 * t_rest && t_rest <= 1.6 * from_end + 2_000.0 && rest.iter().all(|p| line_dist(*p, &t.pts).min(line_dist(*p, &roads[f].pts)) < NEAR)
                        })
                    };
                    if beside {
                        cut = Some((i, k, sr, st, x, at_end));
                        break 'find;
                    }
                }
            }
        }
        let Some((i, k, sr, st, x, at_end)) = cut else { break };
        let (r, t) = (roads[i].clone(), roads[k].clone());
        let mut pieces = split_road(&r, &arc(&r.pts), &[(sr, x)]);
        let kept = if at_end { pieces.swap_remove(0) } else { pieces.pop().unwrap() };
        let halves = split_road(&t, &arc(&t.pts), &[(st, x)]);
        let (lo, hi) = (i.min(k), i.max(k));
        roads.remove(hi);
        roads.remove(lo);
        roads.push(kept);
        roads.extend(halves);
        for v in [hi, lo] {
            keys.remove(v);
            boxes.remove(v);
        }
        for r in &roads[keys.len()..] {
            keys.push(key_of(r));
            boxes.push(box_of(r));
        }
        cuts.push(x);
    }
    cuts
}

/// Roads that only meet each other (two ends at a point outside settlements) become one road,
/// the bend where they met rounded (a curve from ~0.3 cell before it to as far after, keeping
/// each side's heading; left as it is where the curve would cross water).
pub(super) fn join_through(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], cell: f64, dry: &dyn Fn(f64, f64) -> bool) {
    // Each road's end keys (a join keeps the two far ends, so theirs carry over).
    let mut keys: Vec<Option<((i64, i64), (i64, i64))>> = roads.iter().map(|r| (r.pts.len() >= 2).then(|| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns)))).collect();
    loop {
        let mut at: std::collections::BTreeMap<(i64, i64), Vec<(usize, bool)>> = Default::default();
        for (i, k) in keys.iter().enumerate() {
            if let Some((k0, k1)) = *k {
                at.entry(k0).or_default().push((i, true));
                at.entry(k1).or_default().push((i, false));
            }
        }
        // (A drawn road and another keep their own names: they stay two.)
        let Some((&(i, si), &(k, sk))) = at.iter().filter(|(key, v)| key.0 != i64::MIN && v.len() == 2 && v[0].0 != v[1].0 && roads[v[0].0].stroke == roads[v[1].0].stroke).map(|(_, v)| (&v[0], &v[1])).next() else { break };
        // Road i ends at the point, road k starts there.
        let a = if si { flipped(&roads[i]) } else { roads[i].clone() };
        let b = if sk { roads[k].clone() } else { flipped(&roads[k]) };
        let joint = a.pts.len() - 1;
        let mut joined = RoadPath { class: a.class.min(b.class), pts: a.pts, z: a.z, wander: a.wander, stroke: a.stroke, spine: a.spine && b.spine };
        joined.pts.extend_from_slice(&b.pts[1..]);
        joined.z.extend_from_slice(&b.z[1..]);
        joined.wander.extend_from_slice(&b.wander[1..]);
        // The ends' wander was tapered away by the curve's ends; mid-road it would jog.
        joined.wander[joint] = 0.0;
        fillet(&mut joined, joint, cell, dry);
        let (ka, kb) = (keys[i].unwrap(), keys[k].unwrap());
        let key = (if si { ka.1 } else { ka.0 }, if sk { kb.1 } else { kb.0 });
        let (lo, hi) = (i.min(k), i.max(k));
        roads.remove(hi);
        roads.remove(lo);
        keys.remove(hi);
        keys.remove(lo);
        keys.push((joined.pts.len() >= 2).then_some(key));
        roads.push(joined);
    }
}

/// Rounds the bend of a road at point `at`: the points within ~0.3 cell (less on a short
/// side) either side become a cubic keeping the headings it arrives and leaves with.
pub(super) fn fillet(r: &mut RoadPath, at: usize, cell: f64, dry: &dyn Fn(f64, f64) -> bool) {
    let n = r.pts.len();
    if at == 0 || at + 1 >= n {
        return;
    }
    let (u, v) = (unit2([r.pts[at][0] - r.pts[at - 1][0], r.pts[at][1] - r.pts[at - 1][1]]), unit2([r.pts[at + 1][0] - r.pts[at][0], r.pts[at + 1][1] - r.pts[at][1]]));
    if dot2(u, v) > libm::cos(30f64.to_radians()) {
        return;
    }
    // The points `reach` along the road either side (at most halfway to its ends).
    let walk = |dir: i64| {
        let mut i = at as i64;
        let mut acc = 0.0;
        let limit = if dir < 0 { (0..at).map(|k| dist(r.pts[k], r.pts[k + 1])).sum::<f64>() } else { (at..n - 1).map(|k| dist(r.pts[k], r.pts[k + 1])).sum::<f64>() };
        let want = (0.3 * cell).min(0.45 * limit);
        while i + dir >= 0 && ((i + dir) as usize) < n && acc < want {
            acc += dist(r.pts[i as usize], r.pts[(i + dir) as usize]);
            i += dir;
        }
        i as usize
    };
    let (i0, i1) = (walk(-1), walk(1));
    if i0 == 0 && i1 == n - 1 || i0 >= at || i1 <= at {
        return;
    }
    let (p0, p3) = (r.pts[i0], r.pts[i1]);
    // Headings there: along the road into p0, out of p3.
    let h0 = if i0 > 0 { unit2([p0[0] - r.pts[i0 - 1][0], p0[1] - r.pts[i0 - 1][1]]) } else { u };
    let h1 = if i1 + 1 < n { unit2([r.pts[i1 + 1][0] - p3[0], r.pts[i1 + 1][1] - p3[1]]) } else { v };
    let span = dist(p0, p3);
    let (c1, c2) = ([p0[0] + h0[0] * 0.4 * span, p0[1] + h0[1] * 0.4 * span], [p3[0] - h1[0] * 0.4 * span, p3[1] - h1[1] * 0.4 * span]);
    let m = (span / (0.05 * cell).max(60.0)).ceil().clamp(3.0, 30.0) as usize;
    let curve: Vec<[f64; 2]> = (1..m)
        .map(|i| {
            let t = i as f64 / m as f64;
            let w = 1.0 - t;
            let (a, b, c, d) = (w * w * w, 3.0 * w * w * t, 3.0 * w * t * t, t * t * t);
            [a * p0[0] + b * c1[0] + c * c2[0] + d * p3[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p3[1]]
        })
        .collect();
    if curve.iter().any(|p| !dry(p[0], p[1])) {
        return;
    }
    let (z0, z1) = (r.z[i0], r.z[i1]);
    let z: Vec<f32> = (1..m).map(|i| z0 + (z1 - z0) * i as f32 / m as f32).collect();
    r.pts.splice(i0 + 1..i1, curve);
    r.z.splice(i0 + 1..i1, z);
    r.wander.splice(i0 + 1..i1, std::iter::repeat_n(0.0, m - 1));
}

/// A road that runs within a mile of a way at least as good between its two ends, no more
/// than 35% longer, only makes a lens of two roads: it goes. So do two roads of one class
/// running straight on through a crossroads (the other two roads keep it), or through a
/// junction where a side road joins them (a lesser road beside a better one between the same
/// two junctions): the side road goes on along as little of one of the two as it needs, then
/// straight to the way (within `BESIDE`) on a ramp in its grade, dry, crossing no road, its
/// junction clear of river crossings (else the two stay). The best roads are kept first; drawn
/// roads always. Returns whether any went.
pub(super) fn drop_lenses(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)], ground: &dyn Fn([f64; 2]) -> f64, dry: &dyn Fn(f64, f64) -> bool, crossings: &dyn Fn(&[[f64; 2]]) -> Vec<([f64; 2], f64)>) -> bool {
    const NEAR: f64 = 5280.0;
    const BESIDE: f64 = 2_000.0;
    let mut any = false;
    // (Ones that kept their road once keep it: the network only loses roads.)
    let mut tried: std::collections::BTreeSet<Vec<u64>> = Default::default();
    loop {
        let ends: Vec<((i64, i64), (i64, i64))> = roads.iter().map(|r| (end_key(r.pts[0], towns), end_key(*r.pts.last().unwrap(), towns))).collect();
        let lens: Vec<f64> = roads.iter().map(road_len).collect();
        let mut at: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
        for (i, e) in ends.iter().enumerate() {
            at.entry(e.0).or_default().push(i);
            at.entry(e.1).or_default().push(i);
        }
        // Candidates, worst first: single roads, then straight pairs through a crossroads or a
        // side road's junction.
        // (With the side road joining a pair, and that junction.)
        let mut cands: Vec<(RoadClass, Vec<usize>, (i64, i64), (i64, i64), Option<(usize, (i64, i64))>)> = Vec::new();
        for (i, r) in roads.iter().enumerate() {
            if r.stroke.is_none() && ends[i].0 != ends[i].1 {
                cands.push((r.class, vec![i], ends[i].0, ends[i].1, None));
            }
        }
        for (k, list) in &at {
            if list.len() < 3 || k.0 == i64::MIN {
                continue;
            }
            for (u, &a) in list.iter().enumerate() {
                for &b in &list[u + 1..] {
                    let (ra, rb) = (&roads[a], &roads[b]);
                    if a == b || ra.class != rb.class || ra.stroke.is_some() || rb.stroke.is_some() {
                        continue;
                    }
                    let side = if list.len() == 3 {
                        let s = list.iter().copied().find(|&s| s != a && s != b);
                        match s {
                            Some(s) if roads[s].class >= ra.class && roads[s].stroke.is_none() && ends[s].0 != ends[s].1 => Some((s, *k)),
                            _ => continue,
                        }
                    } else {
                        None
                    };
                    let far = |i: usize| if ends[i].0 == *k { ends[i].1 } else { ends[i].0 };
                    let heading = |i: usize| {
                        let r = &roads[i];
                        let (p, q) = if ends[i].0 == *k { (r.pts[0], r.pts[1.min(r.pts.len() - 1)]) } else { (r.pts[r.pts.len() - 1], r.pts[r.pts.len() - 2]) };
                        unit2(sub2(q, p))
                    };
                    if far(a) != far(b) && dot2(heading(a), heading(b)) <= THROUGH {
                        let mut set = vec![a, b];
                        set.sort_unstable();
                        cands.push((ra.class, set, far(a), far(b), side));
                    }
                }
            }
        }
        cands.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
        // (The side road's new way on: its index, at its start, the points and heights out to
        // the way, and the way's road cut where it meets it.)
        let mut gone: Option<(Vec<usize>, Option<(usize, bool, Vec<[f64; 2]>, Vec<f32>, usize, Option<f64>)>)> = None;
        for (class, set, p, q, side) in cands {
            if !tried.insert(set.iter().map(|&i| lens[i].to_bits()).collect()) {
                continue;
            }
            let len: f64 = set.iter().map(|&i| lens[i]).sum();
            // Shortest way from p to q on roads at least as good, not through the set.
            let mut best: std::collections::BTreeMap<(i64, i64), (f64, Option<usize>)> = Default::default();
            let mut heap = std::collections::BinaryHeap::new();
            best.insert(p, (0.0, None));
            heap.push((std::cmp::Reverse(0u64), p));
            while let Some((std::cmp::Reverse(d), n)) = heap.pop() {
                let d = f64::from_bits(d);
                if n == q || d > 1.35 * len || best.get(&n).is_some_and(|b| b.0 < d) {
                    continue;
                }
                for &e in at.get(&n).map(|v| v.as_slice()).unwrap_or(&[]) {
                    if set.contains(&e) || roads[e].class > class {
                        continue;
                    }
                    let m = if ends[e].0 == n { ends[e].1 } else { ends[e].0 };
                    let nd = d + lens[e];
                    if best.get(&m).is_none_or(|b| nd < b.0) {
                        best.insert(m, (nd, Some(e)));
                        heap.push((std::cmp::Reverse(nd.to_bits()), m));
                    }
                }
            }
            let Some(&(dq, _)) = best.get(&q) else { continue };
            if dq > 1.35 * len {
                continue;
            }
            // The way's roads, back from q.
            let mut way: Vec<usize> = Vec::new();
            let mut n = q;
            while let Some(&(_, Some(e))) = best.get(&n) {
                way.push(e);
                n = if ends[e].0 == n { ends[e].1 } else { ends[e].0 };
                if way.len() > roads.len() {
                    break;
                }
            }
            let near = |x: [f64; 2]| way.iter().any(|&e| roads[e].pts.windows(2).any(|w| seg_dist_or_start(x, w[0], w[1]) < NEAR));
            let along = set.iter().all(|&i| {
                let r = &roads[i];
                let s = arc(&r.pts);
                let n = (s[s.len() - 1] / 300.0).ceil().max(1.0) as usize;
                (0..=n).all(|k| near(point_at(&r.pts, &s, s[s.len() - 1] * k as f64 / n as f64)))
            });
            if !along {
                continue;
            }
            let Some((sd, k)) = side else {
                gone = Some((set, None));
                break;
            };
            if way.contains(&sd) {
                continue;
            }
            // The side road on along one of the two from the junction (as little as it can) and
            // then straight to the nearest point of the way.
            let r = &roads[sd];
            let start = ends[sd].0 == k;
            let gmax = r.class.max_grade();
            let attach = |pp: [f64; 2], zp: f64| -> Option<(usize, Option<f64>, f64, Vec<[f64; 2]>, Vec<f32>)> {
                let mut best_e: Option<(usize, f64, f64)> = None;
                for &e in &way {
                    let (t, d) = project(pp, &roads[e].pts, &arc(&roads[e].pts));
                    if best_e.is_none_or(|b| d < b.2) {
                        best_e = Some((e, t, d));
                    }
                }
                let (e, mut t, d) = best_e?;
                if d > BESIDE {
                    return None;
                }
                let ep = &roads[e].pts;
                let es = arc(ep);
                let elen = es[es.len() - 1];
                // (Near an end of that road, its end: the junction there.)
                let mid = t > 300.0 && t < elen - 300.0;
                if !mid {
                    t = if t <= 300.0 { 0.0 } else { elen };
                }
                let qp = if mid { point_at(ep, &es, t) } else if t == 0.0 { ep[0] } else { ep[ep.len() - 1] };
                let zq = val_at(&roads[e].z, &es, t) as f64;
                let dq = dist(pp, qp);
                if dq < 1.0 {
                    return Some((e, mid.then_some(t), 0.0, vec![qp], vec![zq as f32]));
                }
                let n = (dq / 100.0).ceil().max(1.0) as usize;
                let ramp: Vec<[f64; 2]> = (0..=n).map(|i| [pp[0] + (qp[0] - pp[0]) * i as f64 / n as f64, pp[1] + (qp[1] - pp[1]) * i as f64 / n as f64]).collect();
                let fits = (zq - zp).abs() <= 0.85 * gmax * dq
                    && ramp.iter().enumerate().all(|(i, p)| {
                        let u = dq * i as f64 / n as f64;
                        dry(p[0], p[1]) && (ground(*p) - (zp + (zq - zp) * u / dq)).abs() <= 25.0 + gmax * u.min(dq - u)
                    })
                    && crossings(&ramp).is_empty()
                    && (0..roads.len()).filter(|i| !set.contains(i) && *i != sd).all(|i| !ramp[..n].windows(2).any(|w| crosses(w[0], w[1], &roads[i].pts)))
                    && (!mid || crossings(ep).iter().all(|&(x, hw)| dist(x, qp) >= JUNCTION_WET + WET_STRAY * hw));
                let (z0, z1) = (zp as f32, zq as f32);
                fits.then(|| (e, mid.then_some(t), dq, ramp[1..].to_vec(), (1..=n).map(|i| z0 + (z1 - z0) * i as f32 / n as f32).collect()))
            };
            // (Each of the two from the junction, in 100-ft steps: the first that reaches; the
            // shorter way of the two.)
            let mut pick: Option<(f64, Vec<[f64; 2]>, Vec<f32>, usize, Option<f64>)> = None;
            for &i in &set {
                let l = if end_key(roads[i].pts[0], towns) == k { roads[i].clone() } else { flipped(&roads[i]) };
                let s = arc(&l.pts);
                let len = s[s.len() - 1];
                let mut tp: f64 = 0.0;
                loop {
                    let tp_c = tp.min(len);
                    let pp = if tp_c >= len { l.pts[l.pts.len() - 1] } else { point_at(&l.pts, &s, tp_c) };
                    let zp = val_at(&l.z, &s, tp_c);
                    if let Some((e, cut, dq, ramp, zs)) = attach(pp, zp as f64) {
                        if pick.as_ref().is_none_or(|p| tp_c + dq < p.0) {
                            let keep: Vec<usize> = (1..l.pts.len()).filter(|&j| s[j] < tp_c - 5.0).collect();
                            let mut pts: Vec<[f64; 2]> = keep.iter().map(|&j| l.pts[j]).collect();
                            let mut z: Vec<f32> = keep.iter().map(|&j| l.z[j]).collect();
                            if tp_c > 5.0 && dq >= 1.0 {
                                pts.push(pp);
                                z.push(zp);
                            }
                            pts.extend(ramp);
                            z.extend(zs);
                            pick = Some((tp_c + dq, pts, z, e, cut));
                        }
                        break;
                    }
                    if tp_c >= len {
                        break;
                    }
                    tp += 100.0;
                }
            }
            let Some((_, pts, zs, e, cut)) = pick else { continue };
            gone = Some((set, Some((sd, start, pts, zs, e, cut))));
            break;
        }
        let Some((mut set, side)) = gone else { break };
        if let Some((sd, start, pts, zs, e, cut)) = side {
            let r = &mut roads[sd];
            if start {
                r.pts.splice(0..0, pts.iter().rev().copied());
                r.z.splice(0..0, zs.iter().rev().copied());
                r.wander.splice(0..0, std::iter::repeat_n(0.0, pts.len()));
            } else {
                r.pts.extend(pts.iter().copied());
                r.z.extend(zs.iter().copied());
                r.wander.extend(std::iter::repeat_n(0.0, pts.len()));
            }
            if let Some(t) = cut {
                let s = arc(&roads[e].pts);
                let q = point_at(&roads[e].pts, &s, t);
                let mut pieces = split_road(&roads[e], &s, &[(t, q)]);
                roads[e] = pieces.remove(0);
                roads.extend(pieces);
            }
        }
        set.sort_unstable_by(|x, y| y.cmp(x));
        for i in set {
            roads.remove(i);
        }
        any = true;
    }
    any
}

/// Of roads joining the same two ends (the grid's routes can part and meet again; two roads
/// from one junction into the same town, ending at different points of its edge), only the
/// best (class, then shortest) is kept: the other only made a loop.
pub(super) fn drop_parallel(roads: &mut Vec<RoadPath>, towns: &[([f64; 2], f64)]) {
    // An end near a settlement (its road ends are trimmed at its edge) is that settlement.
    let key = |p: [f64; 2]| end_key(p, towns);
    let len = road_len;
    let mut best: std::collections::BTreeMap<((i64, i64), (i64, i64)), usize> = Default::default();
    for (i, r) in roads.iter().enumerate() {
        if r.pts.len() < 2 {
            continue;
        }
        let (a, b) = (key(r.pts[0]), key(*r.pts.last().unwrap()));
        if a == b {
            continue;
        }
        let e = best.entry((a.min(b), a.max(b))).or_insert(i);
        let o = &roads[*e];
        if (r.stroke.is_none(), r.class, len(r)) < (o.stroke.is_none(), o.class, len(o)) {
            *e = i;
        }
    }
    let keep: std::collections::BTreeSet<usize> = best.values().copied().collect();
    let mut i = 0;
    roads.retain(|r| {
        i += 1;
        let ends = r.pts.len() >= 2 && key(r.pts[0]) != key(*r.pts.last().unwrap());
        !ends || keep.contains(&(i - 1)) || r.stroke.is_some()
    });
}
