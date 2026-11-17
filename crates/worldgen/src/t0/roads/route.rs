//! Routing links across the cost grid: drawn routes, the router and its preview.

use super::*;

/// The drawn roads' routes: through waypoints along each line, each leg the cheapest way
/// within `CORRIDOR` cells of its stretch of the line (costing more the farther from it); where
/// water closes that, within `CORRIDOR_WIDE`, else round it (a conflict says so); else (no way
/// by land) the road breaks there (a conflict). Then the cells the `none` lines close.
pub(super) fn drawn_routes(inp: &Inputs) -> DrawnRoutes {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let mut out = DrawnRoutes::default();
    if inp.drawn.is_empty() {
        return out;
    }
    let open: Vec<bool> = (0..w * h).map(|k| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE).collect();
    let costs = Costs::new(inp);
    let mut search = Search::new(w * h);
    // The open cell nearest a point within `r` cells.
    let nearest_open = |p: [f64; 2], r: f64| {
        let mut best: Option<(f64, usize)> = None;
        let (i0, i1) = ((p[0] - r).floor().max(0.0) as usize, ((p[0] + r).ceil() as usize).min(w - 1));
        let (j0, j1) = ((p[1] - r).floor().max(0.0) as usize, ((p[1] + r).ceil() as usize).min(h - 1));
        for j in j0..=j1 {
            for i in i0..=i1 {
                let (k, d) = (j * w + i, dist(p, [i as f64, j as f64]));
                if open[k] && d <= r && best.is_none_or(|b| d < b.0) {
                    best = Some((d, k));
                }
            }
        }
        best.map(|b| b.1)
    };
    for d in &inp.drawn {
        let Some(class) = d.class else { continue };
        let s = arc(&d.pts);
        let total = *s.last().unwrap();
        let m = (total / WAYPOINT).ceil().max(1.0) as usize;
        // Waypoints: (place along the line, cell); the ends may look farther for land.
        let mut ways: Vec<(f64, usize)> = Vec::new();
        for i in 0..=m {
            let t = total * i as f64 / m as f64;
            let r = if i == 0 || i == m { 4.0 } else { 2.0 };
            if let Some(k) = nearest_open(point_at(&d.pts, &s, t), r)
                && ways.last().is_none_or(|l| l.1 != k)
            {
                ways.push((t, k));
            }
        }
        let mut piece: Vec<u32> = Vec::new();
        for pair in ways.windows(2) {
            let ((t0, a), (t1, b)) = (pair[0], pair[1]);
            let line = sub_line(&d.pts, &s, t0 - 1.0, t1 + 1.0);
            let mut leg = None;
            for (attempt, (reach, deviate)) in [(CORRIDOR, DEVIATE), (CORRIDOR_WIDE, 0.25 * DEVIATE), (CORRIDOR_ROUND, 0.01 * DEVIATE)].into_iter().enumerate() {
                let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for p in &line {
                    (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
                }
                let bx = [(x0 - reach).floor().max(0.0) as i64, ((x1 + reach).ceil() as i64).min(w as i64 - 1), (y0 - reach).floor().max(0.0) as i64, ((y1 + reach).ceil() as i64).min(h as i64 - 1)];
                // Distance to the line over the box.
                let bw = (bx[1] - bx[0] + 1) as usize;
                let off: Vec<f64> = (0..bw * (bx[3] - bx[2] + 1) as usize).map(|q| line_dist([(bx[0] + (q % bw) as i64) as f64, (bx[2] + (q / bw) as i64) as f64], &line)).collect();
                let off_at = |k: usize| off[((k / w) as i64 - bx[2]) as usize * bw + ((k % w) as i64 - bx[0]) as usize];
                let ok = |k: usize| open[k] && off_at(k) <= reach;
                leg = search.run(w, a, b, bx, ok, |i, nb, mids, run| costs.ground(inp, i, nb, run) + costs.river(inp, i, nb, mids) + deviate * off_at(nb) * off_at(nb));
                if leg.is_some() {
                    if attempt == 2 {
                        let p = point_at(&d.pts, &s, 0.5 * (t0 + t1));
                        out.conflicts.push(super::super::sketch::Conflict { stroke: d.stroke as usize, message: "This road crosses water here: it goes round".into(), x: p[0] * cell, y: p[1] * cell });
                    }
                    break;
                }
            }
            match leg {
                Some(leg) => piece.extend_from_slice(if piece.is_empty() { &leg } else { &leg[1..] }),
                None => {
                    let p = point_at(&d.pts, &s, 0.5 * (t0 + t1));
                    out.conflicts.push(super::super::sketch::Conflict { stroke: d.stroke as usize, message: "No way by land along this road here: it stops and starts again".into(), x: p[0] * cell, y: p[1] * cell });
                    if piece.len() >= 2 {
                        out.routes.push((class, std::mem::take(&mut piece), d.stroke));
                    }
                    piece.clear();
                }
            }
        }
        if piece.len() >= 2 {
            out.routes.push((class, piece, d.stroke));
        }
    }
    // A `none` line closes the cells within about a cell of it: no step crosses that band.
    if inp.drawn.iter().any(|d| d.class.is_none()) {
        let mut blocked = vec![false; w * h];
        for d in inp.drawn.iter().filter(|d| d.class.is_none()) {
            for seg in d.pts.windows(2) {
                let (i0, i1) = ((seg[0][0].min(seg[1][0]) - 2.0).floor().max(0.0) as usize, ((seg[0][0].max(seg[1][0]) + 2.0).ceil() as usize).min(w - 1));
                let (j0, j1) = ((seg[0][1].min(seg[1][1]) - 2.0).floor().max(0.0) as usize, ((seg[0][1].max(seg[1][1]) + 2.0).ceil() as usize).min(h - 1));
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        if seg_dist([i as f64, j as f64], seg[0], seg[1]) <= 1.1 {
                            blocked[j * w + i] = true;
                        }
                    }
                }
            }
        }
        out.blocked = blocked;
    }
    out
}

/// Distance along a polyline at each of its points.
pub(super) fn arc(pts: &[[f64; 2]]) -> Vec<f64> {
    let mut s = vec![0.0; pts.len()];
    for i in 1..pts.len() {
        s[i] = s[i - 1] + dist(pts[i - 1], pts[i]);
    }
    s
}

/// The point `t` along a polyline (`s`: its `arc`), clamped to its ends.
pub(super) fn point_at(pts: &[[f64; 2]], s: &[f64], t: f64) -> [f64; 2] {
    let k = s.partition_point(|&v| v < t).clamp(1, pts.len() - 1);
    let (a, b) = (pts[k - 1], pts[k]);
    let f = ((t - s[k - 1]) / (s[k] - s[k - 1]).max(1e-9)).clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
}

/// The stretch of a polyline from `t0` to `t1` along it (clamped to its ends).
pub(super) fn sub_line(pts: &[[f64; 2]], s: &[f64], t0: f64, t1: f64) -> Vec<[f64; 2]> {
    let mut out = vec![point_at(pts, s, t0)];
    out.extend(pts.iter().zip(s).filter(|(_, v)| **v > t0 && **v < t1).map(|(p, _)| *p));
    out.push(point_at(pts, s, t1));
    out
}

/// Where a point falls on a polyline: the distance along it of the nearest point, and how far
/// off it is.
pub(super) fn project(p: [f64; 2], pts: &[[f64; 2]], s: &[f64]) -> (f64, f64) {
    let mut best = (0.0, f64::INFINITY);
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let d = dist(p, [a[0] + t * dx, a[1] + t * dy]);
        if d < best.1 {
            best = (s[k - 1] + t * (s[k] - s[k - 1]), d);
        }
    }
    best
}

/// A* routes for the links, in order (later routes reuse earlier ones), after the drawn
/// roads. Returns the best (lowest) class per cell (`u8::MAX` = none) and every route.
/// Round a Town+ end, roads and tracks (not king's roads) keep clear of each other: the road
/// trim radius of the biggest possible metropolis, so a change of tier can't move a junction
/// into the town.
const ZONE_FT: f64 = 1.1 * 1.5 * 12.0 * 282.842712474619;
/// The classes that count as a major road to join (those up to this one).
const MAJOR: RoadClass = RoadClass::Road;
/// What stepping off the open ground onto a major road inside the zone costs (cells): large, not infinite.
const JOIN_BAN: f64 = 200.0;
/// Whether segments a-b and c-d cross (properly: not where they only touch).
fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let side = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// How far (miles) the other roads keep off an isolated realm's track, but at its outer end.
pub const REALM_TRACK_APART_MI: f64 = 20.0;
/// What crossing the zone edge off a major road costs (cells): the long way round that is worth a road of its own.
const GATE_ROAD: f64 = 2.0;
const GATE_TRACK: f64 = 1.0;

pub fn route(inp: &Inputs, settlements: &[Settlement], links: &[Link]) -> (Vec<u8>, Vec<Routed>) {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let n = w * h;
    let pre = inp.pre();
    // --- 2. Routing.
    let RouteFixed { costs, free, open, wobble } = inp.fixed.get_or_init(|| {
        let costs = Costs::new(inp);
        let free: Vec<bool> = (0..n).map(|k| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE).collect();
        let open: Vec<bool> = (0..n).map(|k| free[k] && !pre.blocked.get(k).copied().unwrap_or(false)).collect();
        let mut noise = crate::core::noise::Fbm::new(inp.world.stream("t0.road.cost"), 2, 2.0, 0.5);
        let wobble: Vec<f32> = (0..n).map(|k| (1.0 + 0.25 * noise.at((k % w) as f64 / 6.0, (k / w) as f64 / 6.0)) as f32).collect();
        RouteFixed { costs, free, open, wobble }
    });
    let mut road_class: Vec<u8> = vec![u8::MAX; n];
    // Cells on a drawn road only (routes share them at half cost, not a fifth: a planned road
    // takes a drawn one where it goes its way, but no long way round for it).
    let mut drawn_only: Vec<bool> = vec![false; if pre.routes.is_empty() { 0 } else { n }];
    let mut paths: Vec<Routed> = Vec::new();
    // The steps major roads take: a step between two major roads' cells is no ride along one.
    let mut major_steps: crate::core::hash::FastSet<(u32, u32)> = Default::default();
    for (class, cells, stroke) in &pre.routes {
        for &c in cells {
            drawn_only[c as usize] = true;
        }
        for &c in cells {
            road_class[c as usize] = road_class[c as usize].min(*class as u8);
        }
        if *class <= MAJOR {
            major_steps.extend(cells.windows(2).map(|p| (p[0].min(p[1]), p[0].max(p[1]))));
        }
        paths.push((*class, cells.clone(), Some(*stroke)));
    }
    let mut search = Search::new(n);
    let mut memo = inp.routes.borrow_mut();
    let mut same = true;
    // Isolated realms' towns (cell x, y, the radius other roads pay extra inside), and per cell
    // whether a road from outside any realm runs there; each realm's track.
    let avoid = super::super::settle::REALM_AVOID_MI * 5280.0 / cell;
    let avoid_track = super::super::settle::REALM_AVOID_TRACK_MI * 5280.0 / cell;
    // (How far other roads keep off a realm's track but where it joins them.)
    let apart_ft = REALM_TRACK_APART_MI * 5280.0;
    let realms: Vec<(f64, f64)> = settlements.iter().filter(|s| s.isolated && s.planned >= Tier::Town).map(|s| ((s.cell % w) as f64, (s.cell / w) as f64)).collect();
    let mut outside: Vec<bool> = vec![false; if realms.is_empty() { 0 } else { n }];
    for (_, cells, _) in &pre.routes {
        for &c in cells {
            if let Some(o) = outside.get_mut(c as usize) {
                *o = true;
            }
        }
    }
    let mut tracks: std::collections::BTreeMap<usize, Vec<u32>> = Default::default();
    // Where each realm's villages' tracks join it.
    let mut joins: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    // Each realm's villages' tracks so far (cells).
    let mut spurs: std::collections::BTreeMap<usize, Vec<Vec<u32>>> = Default::default();
    for (e, link) in links.iter().enumerate() {
        let class = link.class;
        let realm_link = [link.a, link.b].iter().any(|&end| match end {
            End::Town(i) => settlements[i].isolated,
            End::Net | End::Track(_) => true,
            End::Cell(_) => false,
        });
        let from = match link.a {
            End::Town(i) => settlements[i].cell,
            End::Cell(k) => k,
            _ => continue,
        };
        let near = |cells: &mut dyn Iterator<Item = usize>| {
            let (fx, fy) = ((from % w) as f64, (from / w) as f64);
            cells.map(|k| (((k % w) as f64 - fx) * ((k % w) as f64 - fx) + ((k / w) as f64 - fy) * ((k / w) as f64 - fy), k)).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|m| m.1)
        };
        let goal = match link.b {
            End::Town(i) => Some(settlements[i].cell),
            End::Cell(k) => Some(k),
            // (The nearest king's road, the nearest road where that is much nearer; nearer still
            // the way its villages lie, so that it leaves past them: `settle::place_realms`.)
            End::Net => {
                let (fx, fy) = ((from % w) as f64, (from / w) as f64);
                let ri = match link.a {
                    End::Town(i) => i,
                    _ => usize::MAX,
                };
                let (mut ex, mut ey) = (0.0, 0.0);
                for (vi, v) in settlements.iter().enumerate() {
                    if v.isolated && v.planned == Tier::Village && realm_of(settlements, vi) == Some(ri) {
                        let (vx, vy) = ((v.cell % w) as f64 - fx, (v.cell / w) as f64 - fy);
                        let m = crate::core::sqrt(vx * vx + vy * vy).max(1e-9);
                        (ex, ey) = (ex + vx / m, ey + vy / m);
                    }
                }
                let em = crate::core::sqrt(ex * ex + ey * ey);
                let d = |k: usize| {
                    let (dx, dy) = ((k % w) as f64 - fx, (k / w) as f64 - fy);
                    let r = crate::core::sqrt(dx * dx + dy * dy);
                    let off = if em > 1e-6 && r > 1e-9 { 1.0 - (dx * ex + dy * ey) / (em * r) } else { 0.0 };
                    r * (1.0 + 0.4 * off)
                };
                let toward = |cells: &mut dyn Iterator<Item = usize>| cells.map(|k| (d(k), k)).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|m| m.1);
                // (On the realm's own land: not to an island's roads across the sea.)
                let mass = &inp.masses().of;
                let ours = |k: usize| outside[k] && mass[k] == mass[from];
                let any = toward(&mut (0..outside.len()).filter(|&k| ours(k)));
                let king = toward(&mut (0..outside.len()).filter(|&k| ours(k) && road_class[k] == RoadClass::KingsRoad as u8));
                let natural = match (king, any) {
                    (Some(kc), Some(ac)) if d(kc) <= d(ac) + 15.0 * 5280.0 / cell => Some(kc),
                    _ => any,
                };
                // Near a junction it ends at a node instead (a settlement on the roads there, else
                // the junction), a branch of its own rather than a way on past it.
                natural.map(|g| {
                    let reach = apart_ft / cell;
                    let by = |k: usize| ((k % w) as f64 - (g % w) as f64) * ((k % w) as f64 - (g % w) as f64) + ((k / w) as f64 - (g / w) as f64) * ((k / w) as f64 - (g / w) as f64) <= reach * reach;
                    let mut arms: std::collections::BTreeMap<u32, Vec<u32>> = Default::default();
                    for (_, path, _) in &paths {
                        for p in path.windows(2) {
                            if p[0] != p[1] && outside[p[0] as usize] && outside[p[1] as usize] && (by(p[0] as usize) || by(p[1] as usize)) {
                                for (a, b) in [(p[0], p[1]), (p[1], p[0])] {
                                    let v = arms.entry(a).or_default();
                                    if !v.contains(&b) {
                                        v.push(b);
                                    }
                                }
                            }
                        }
                    }
                    let junctions: Vec<usize> = arms.iter().filter(|(k, v)| v.len() >= 3 && by(**k as usize)).map(|(&k, _)| k as usize).collect();
                    if junctions.is_empty() {
                        return g;
                    }
                    let towns: Vec<usize> = settlements.iter().filter(|s| !s.isolated && outside[s.cell] && by(s.cell)).map(|s| s.cell).collect();
                    let pick = if towns.is_empty() { junctions } else { towns };
                    near(&mut pick.into_iter()).unwrap_or(g)
                })
            }
            End::Track(ri) => {
                // (Joining it outside the town, so only the track reaches its gate.)
                let s = &settlements[ri];
                let r = realm_keep_out(s, cell);
                let (sx, sy) = ((s.cell % w) as f64, (s.cell / w) as f64);
                let far = |k: usize| ((k % w) as f64 - sx) * ((k % w) as f64 - sx) + ((k / w) as f64 - sy) * ((k / w) as f64 - sy) > r * r;
                // (Not beside where another of its villages joined: no stub between two junctions.)
                let join_apart = JOIN_APART_FT / cell;
                let bends = tracks.get(&ri).map(|t| track_bends(t, w, cell)).unwrap_or_default();
                let apart = |k: usize| {
                    joins.get(&ri).is_none_or(|js: &Vec<usize>| js.iter().all(|&j| ((k % w) as f64 - (j % w) as f64) * ((k % w) as f64 - (j % w) as f64) + ((k / w) as f64 - (j / w) as f64) * ((k / w) as f64 - (j / w) as f64) >= join_apart * join_apart))
                };
                // (Nor across another of its villages' tracks: each straight to its own join.)
                let xy = |c: usize| ((c % w) as f64, (c / w) as f64);
                let clear = |k: usize| {
                    let (a, b) = (xy(from), xy(k));
                    spurs.get(&ri).is_none_or(|ss| ss.iter().all(|sp| sp.windows(2).all(|q| !segments_cross(a, b, xy(q[0] as usize), xy(q[1] as usize)))))
                };
                // (But a clear line only where it is not much farther: rather a short spur round the
                // other's join than one alongside the track for miles to its far end.)
                let (fx, fy) = xy(from);
                let gap = |k: usize| crate::core::sqrt((xy(k).0 - fx) * (xy(k).0 - fx) + (xy(k).1 - fy) * (xy(k).1 - fy));
                tracks.get(&ri).and_then(|t| {
                    // (Off a bend of the track where there is a join that is, else anywhere.)
                    let pick = |bent: bool| {
                        let ok = |k: usize| far(k) && apart(k) && (!bent || !bends.contains(&k));
                        let best = near(&mut t.iter().map(|&c| c as usize).filter(|&k| ok(k)));
                        let straight = near(&mut t.iter().map(|&c| c as usize).filter(|&k| ok(k) && clear(k)));
                        match (straight, best) {
                            (Some(s), Some(b)) if gap(s) <= 1.5 * gap(b) + 2.0 * 5280.0 / cell => Some(s),
                            (_, b) => b,
                        }
                    };
                    pick(true).or_else(|| pick(false)).or_else(|| near(&mut t.iter().map(|&c| c as usize).filter(|&k| far(k))))
                })
            }
        };
        let Some(goal) = goal else { continue };
        // (The joins of its realm's other villages so far.)
        let others: Vec<usize> = match link.b {
            End::Track(ri) => joins.get(&ri).cloned().unwrap_or_default(),
            _ => Vec::new(),
        };
        if let End::Track(ri) = link.b {
            joins.entry(ri).or_default().push(goal);
        }
        let start = from;
        // A hidden village's track keeps off the ground round its realm's town but the town's own
        // track (one gate).
        let keep_out = match link.b {
            End::Track(ri) => {
                let s = &settlements[ri];
                Some(((s.cell % w) as f64, (s.cell / w) as f64, realm_keep_out(s, cell)))
            }
            _ => None,
        };
        // Near a town or city at either end, the king's roads keep to themselves (no shared trunk),
        // each arriving at its own gate. Roads and tracks keep to a fixed zone round the town
        // instead: inside it they share only a major road, and leaving the open ground for the
        // zone costs a gate, so they join a major road outside (see the cost below).
        let kings = class == RoadClass::KingsRoad;
        let town_ends: Vec<(f64, f64, f64)> = [link.a, link.b]
            .iter()
            .filter_map(|&end| match end {
                End::Town(i) if settlements[i].planned >= Tier::Town => Some(i),
                _ => None,
            })
            .map(|i| {
                let r = if kings { crate::t0::radii::urban_radius(settlements[i].planned, settlements[i].planned_pop) / cell + 1.5 } else { ZONE_FT / cell + 1.5 };
                ((settlements[i].cell % w) as f64, (settlements[i].cell / w) as f64, r)
            })
            .collect();
        let gate = if class == RoadClass::Road { GATE_ROAD } else { GATE_TRACK };
        // A town no major road reaches yet: its first way out may step onto one passing it (the
        // town's own way to it); once one does, the others ride that.
        let lone = road_class[start] > MAJOR as u8;
        let key = RouteKey(start, goal, class, town_ends.iter().map(|t| (t.0.to_bits(), t.1.to_bits(), t.2.to_bits())).collect());
        // The same links so far as the last call: the same routes.
        same = same && memo.get(e).is_some_and(|m| m.0 == key);
        let found = if same {
            memo[e].1.clone()
        } else {
            let (gx, gy) = ((goal % w) as f64, (goal / w) as f64);
            // Search box: endpoints' bounding box grown by 40% + margin.
            let (sx, sy) = ((start % w) as f64, (start / w) as f64);
            let pad = 0.4 * (sx - gx).abs().max((sy - gy).abs()) + 25.0;
            let (bx0, bx1) = ((sx.min(gx) - pad).max(0.0), (sx.max(gx) + pad).min((w - 1) as f64));
            let (by0, by1) = ((sy.min(gy) - pad).max(0.0), (sy.max(gy) + pad).min((h - 1) as f64));
            // The box in whole cells (a cell index is inside exactly when it is between these).
            let b = [crate::core::ceil(bx0) as i64, crate::core::floor(bx1) as i64, crate::core::ceil(by0) as i64, crate::core::floor(by1) as i64];
            // A realm's track keeps off the other roads (cells from them) but near where it joins
            // them; its villages' tracks off every road but where they join its track (each a spur
            // of its own, straight to it).
            let off: Vec<f64> = if matches!(link.b, End::Net) { super::super::climate::distance_to(w, h, &outside) } else { Vec::new() };
            let apart = apart_ft / cell;
            // (A margin over that, so none comes closer just beyond the end.)
            let keep = 1.25 * apart;
            let spur = matches!(link.b, End::Track(_));
            // (And a cell's berth off its realm's other villages' tracks, so none slips across one.)
            let berth: crate::core::hash::FastSet<usize> = match link.b {
                End::Track(ri) => spurs
                    .get(&ri)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .flat_map(|&c| {
                        let (x, y) = ((c as usize % w) as i64, (c as usize / w) as i64);
                        (-1i64..=1).flat_map(move |dy| (-1i64..=1).map(move |dx| (x + dx, y + dy)))
                    })
                    .filter(|&(x, y)| x >= 0 && y >= 0 && x < w as i64 && y < h as i64)
                    .map(|(x, y)| y as usize * w + x as usize)
                    .filter(|&k| k != goal)
                    .collect(),
                _ => Default::default(),
            };
            let zone = if class == RoadClass::Track { avoid_track } else { avoid };
            let path = search.run(w, start, goal, b, |k| open[k] || (k == goal && free[k]), |i, nb, mids, run| {
                let mut c = costs.ground(inp, i, nb, run) * wobble[nb] as f64;
                c += costs.river(inp, i, nb, mids);
                let (nx, ny) = ((nb % w) as f64, (nb / w) as f64);
                let near = |x: f64, y: f64| town_ends.iter().any(|&(tx, ty, tr)| (x - tx) * (x - tx) + (y - ty) * (y - ty) <= tr * tr);
                // Round an isolated realm the others go the long way.
                if !realm_link {
                    for &(rx, ry) in &realms {
                        let d2 = ((nx - rx) * (nx - rx) + (ny - ry) * (ny - ry)) / (zone * zone);
                        if d2 < 1.0 {
                            c *= 1.0 + 12.0 * (1.0 - d2);
                        }
                    }
                }
                if let Some(&d) = off.get(nb)
                    && d < keep
                    && (nx - gx) * (nx - gx) + (ny - gy) * (ny - gy) > apart * apart
                {
                    c *= 1.0 + 8.0 * (1.0 - d / keep);
                }
                if spur && ((road_class[nb] != u8::MAX && nb != goal) || berth.contains(&nb)) {
                    c += JOIN_BAN / run;
                }
                if let Some((kx, ky, kr)) = keep_out
                    && road_class[nb] == u8::MAX
                    && (nx - kx) * (nx - kx) + (ny - ky) * (ny - ky) <= kr * kr
                {
                    c += JOIN_BAN / run;
                }
                // (A hidden village's track keeps off its realm's roads beside where the others
                // joined them, so that their junctions stay apart.)
                if road_class[nb] != u8::MAX && nb != goal && !others.is_empty() && others.iter().any(|&j| ((j % w) as f64 - nx) * ((j % w) as f64 - nx) + ((j / w) as f64 - ny) * ((j / w) as f64 - ny) < (JOIN_APART_FT / cell) * (JOIN_APART_FT / cell)) {
                    c += JOIN_BAN / run;
                }
                if kings {
                    if road_class[nb] != u8::MAX && !near(nx, ny) {
                        c *= if drawn_only.get(nb).copied().unwrap_or(false) { 0.5 } else { 0.2 };
                    }
                } else {
                    let (ix, iy) = ((i % w) as f64, (i / w) as f64);
                    let major = |k: usize| road_class[k] <= MAJOR as u8;
                    let (zi, zn) = (near(ix, iy), near(nx, ny));
                    let edge = || major_steps.contains(&((i as u32).min(nb as u32), (i as u32).max(nb as u32)));
                    // (At its ends a road rides only a major road's own step: a town's roads leave on the
                    // one that reaches it, not across to another passing by.)
                    let on = if i != start && nb != goal { major(nb) && major(i) } else { edge() };
                    if zn {
                        if on {
                            c *= 0.2;
                        } else if major(nb) && nb != goal && nb != start && !(lone && i == start) {
                            c += JOIN_BAN / run;
                        }
                    } else if road_class[nb] != u8::MAX {
                        c *= if drawn_only.get(nb).copied().unwrap_or(false) { 0.5 } else { 0.2 };
                    }
                    // Crossing the zone edge off a major road, or leaving a major road inside the zone
                    // for a way in of its own, costs a gate.
                    if !on && i != start && nb != goal && (zi != zn || (zi && major(i))) {
                        c += gate / run;
                    }
                }
                c
            });
            memo.truncate(e);
            memo.push((key, path.clone()));
            path
        };
        let Some(path) = found else { continue };
        if !realm_link {
            for &c in &path {
                if let Some(o) = outside.get_mut(c as usize) {
                    *o = true;
                }
            }
        } else if let (End::Town(i), End::Net) = (link.a, link.b) {
            tracks.insert(i, path.clone());
        } else if let End::Track(ri) = link.b {
            // (Each realm village's track a spur of its own to the realm's: no village's joins another's.)
            spurs.entry(ri).or_default().push(path.clone());
        }
        if class <= MAJOR {
            major_steps.extend(path.windows(2).map(|p| (p[0].min(p[1]), p[0].max(p[1]))));
        }
        for &c in &path {
            let c = c as usize;
            road_class[c] = road_class[c].min(class as u8);
            if let Some(d) = drawn_only.get_mut(c) {
                *d = false;
            }
        }
        paths.push((class, path, link.stroke));
    }
    (road_class, paths)
}

/// Where roads of the given classes would run, for placing settlements beside them.
pub struct Preview {
    /// Per cell: routes passing through (saturating).
    pub usage: Vec<u8>,
    /// Per cell: a junction (3+ distinct route edges meet there).
    pub junction: Vec<bool>,
    /// Per cell: the top of a pass, where a route climbs over high ground between valleys.
    pub pass: Vec<bool>,
    /// The routes (the economy's trade runs along them).
    pub paths: Vec<Routed>,
}

pub fn preview(inp: &Inputs, settlements: &[Settlement], classes: &[RoadClass]) -> Preview {
    let edges = links(inp, settlements, classes);
    let (_, paths) = route(inp, settlements, &edges);
    let n = inp.w * inp.h;
    let mut usage = vec![0u8; n];
    let mut pass = vec![false; n];
    let mut links: std::collections::BTreeSet<(u32, u32)> = Default::default();
    // A pass: where a king's route tops a saddle (`settle::saddle_at`, the test a pass town is
    // held to where it stands), the route's way through there from two cells either side.
    let sea = inp.world.params().sea_level_ft;
    let (w, cell) = (inp.w, inp.cell_ft);
    let at = |c: u32| [(c as usize % w) as f64 * cell, (c as usize / w) as f64 * cell];
    for (class, path, _) in &paths {
        let m = path.len();
        for i in 1..m.saturating_sub(1) {
            if *class != RoadClass::KingsRoad || inp.height[path[i] as usize] - sea < 1_000.0 {
                continue;
            }
            let (a, b) = (at(path[i.saturating_sub(2)]), at(path[(i + 2).min(m - 1)]));
            let u = unit2(sub2(b, a));
            if crate::t0::settle::saddle_at(inp.ground, at(path[i]), u, sea) {
                pass[path[i] as usize] = true;
            }
        }
        for &c in path {
            usage[c as usize] = usage[c as usize].saturating_add(1);
        }
        for pair in path.windows(2) {
            links.insert((pair[0].min(pair[1]), pair[0].max(pair[1])));
        }
    }
    let mut degree = vec![0u8; n];
    for &(a, b) in &links {
        degree[a as usize] = degree[a as usize].saturating_add(1);
        degree[b as usize] = degree[b as usize].saturating_add(1);
    }
    Preview { usage, junction: degree.iter().map(|&d| d >= 3).collect(), pass, paths }
}
