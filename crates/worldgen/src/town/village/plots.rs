//! Plots and yards: clearing crofts, green props, squares, crofts, churchyards,
//! shrines, mills, manors and spare ground.

use super::*;

/// How far a plot's hedge or fence keeps off a building it does not hold (ft): a run nearer reads
/// as through its roof (the eaves and the hedge's own width), across a corner most of all.
pub(super) const CLEAR_FT: f64 = 6.0;

/// A plot's runs as `runs` draws them, the gate's gap aside: a croft's sides and back, a yard's round.
fn plot_edges(p: &Plot) -> impl Iterator<Item = (P, P)> + '_ {
    let n = p.pts.len();
    let first = if p.kind == PlotKind::Croft { 1 } else { 0 };
    (first..n).map(move |k| (p.pts[k], p.pts[(k + 1) % n]))
}

/// A footprint's middle by its corners' mean.
fn corner_mean(q: &[P]) -> P {
    mul(q.iter().fold([0.0, 0.0], |s, p| add(s, *p)), 1.0 / q.len().max(1) as f64)
}

/// Whether plot `p`'s runs come within `clear` ft of building `b`.
fn runs_by(p: &Plot, b: &[P], clear: f64) -> bool {
    let bb = bbox(b);
    plot_edges(p).any(|(a, e)| {
        let sb = bbox(&[a, e]);
        sb[0] < bb[2] + clear && sb[2] > bb[0] - clear && sb[1] < bb[3] + clear && sb[3] > bb[1] - clear && poly_gap(b, &[a, e]) < clear
    })
}

/// A cottage `HOUSE_BOUND_FT` off the yards' bounds (laid out before it: churchyard, shrine's,
/// manor's), as a croft's from its own: a wall or hedge along its side, not one by its corner.
pub(super) fn off_yards(g: &Ground, house: &[P]) -> bool {
    g.plots.iter().filter(|p| p.kind != PlotKind::Croft).all(|p| !runs_by(p, house, HOUSE_BOUND_FT))
}

/// A farmyard's bounds `CLEAR_FT` off the buildings round it.
pub(super) fn yard_clear(l: &Layout, yard: &[P]) -> bool {
    let p = Plot { pts: yard.to_vec(), kind: PlotKind::Farmyard, bound: Bound::Hedge };
    l.buildings.iter().all(|b| !runs_by(&p, &b.poly, CLEAR_FT))
}

/// Every croft's sides and back `CLEAR_FT` off the buildings it does not hold (those in another plot
/// aside: that plot keeps its own off them): one by a cottage left with none of its own, or across a
/// house's corner, cut back (its own cottage still held as `croft` holds it), else given up.
pub(super) fn clear_crofts(g: &mut Ground, l: &Layout) {
    let mids: Vec<P> = l.buildings.iter().map(|b| corner_mean(&b.poly)).collect();
    let mut gone = vec![false; g.plots.len()];
    // (Again while one goes: the cottage it held is one the others must keep off now.)
    let mut again = true;
    while again {
        again = false;
        for i in 0..g.plots.len() {
            if gone[i] || g.plots[i].kind != PlotKind::Croft {
                continue;
            }
            let held_by = |j: usize, bi: usize, plots: &[Plot]| contains(&plots[j].pts, mids[bi]);
            let foreign: Vec<usize> = (0..l.buildings.len()).filter(|&bi| !held_by(i, bi, &g.plots) && !(0..g.plots.len()).any(|j| j != i && !gone[j] && held_by(j, bi, &g.plots))).collect();
            let hit = |p: &Plot| foreign.iter().any(|&bi| runs_by(p, &l.buildings[bi].poly, CLEAR_FT));
            if !hit(&g.plots[i]) {
                continue;
            }
            let own: Vec<usize> = (0..l.buildings.len()).filter(|&bi| held_by(i, bi, &g.plots)).collect();
            let q = g.plots[i].pts.clone();
            let mut done = false;
            for k in 1..10 {
                let f = 1.0 - 0.1 * k as f64;
                let pts = vec![q[0], q[1], lerp(q[1], q[2], f), lerp(q[0], q[3], f)];
                let held = inset(&pts, &[0.0, HOUSE_BOUND_FT, HOUSE_BOUND_FT, HOUSE_BOUND_FT]);
                if held.len() < 3 || !own.iter().all(|&bi| l.buildings[bi].poly.iter().all(|c| contains(&held, *c))) {
                    break;
                }
                let cut = Plot { pts, kind: PlotKind::Croft, bound: g.plots[i].bound };
                if !hit(&cut) {
                    g.plots[i] = cut;
                    done = true;
                    break;
                }
            }
            if !done {
                gone[i] = true;
                again = true;
            }
        }
    }
    let plots = std::mem::take(&mut g.plots);
    g.boxes.clear();
    for (p, gone) in plots.into_iter().zip(gone) {
        if !gone {
            g.add(p);
        }
    }
}

/// How far short of a farmyard's gate its lane ends (ft): its round end reaches the gap.
pub(super) const FARM_GATE_FT: f64 = 4.0;

/// At a farmstead's gate (`farmstead`): a lane's end there on purpose.
pub(super) fn at_gate(g: &Ground, p: P) -> bool {
    g.plots.iter().any(|pl| pl.kind == PlotKind::Farmyard && dist(p, lerp(pl.pts[0], pl.pts[1], 0.5)) <= FARM_GATE_FT + 2.0)
}

/// The green's pond (on some) and its tree beside the well, each on dry ground off the roads across
/// it (world ft props); their ways round the well keyed by the village's seed.
pub(super) fn green_props(site: &Site, seed: u64, well: P, r: f64, roads: &[(Vec<P>, f64)], l: &mut Layout) {
    let h = |k: i64| unit(hash3(seed, k, 0, 0x9e3d));
    let off_roads = |p: P, room: f64| {
        !site.near_network_road(p, room + 2.0) && roads.iter().all(|(q, w)| q.windows(2).all(|s| seg_dist(p, s[0], s[1]) > 0.5 * w + room + 2.0))
    };
    let spot = |a0: f64, rad: f64, room: f64| {
        (0..8).map(|k| polar_pt(rad, a0 + std::f64::consts::TAU * k as f64 / 8.0)).map(|p| add(well, p)).find(|&p| {
            (0..8).all(|j| !site.wet(add(p, polar_pt(room + 3.0, std::f64::consts::TAU * j as f64 / 8.0)))) && !site.near_river(p, room + 6.0) && off_roads(p, room)
        })
    };
    let a0 = std::f64::consts::TAU * h(0);
    let mut put = |at: P, kind: crate::battlemap::Kind, variant: u8, scale: f64, rot: f64| {
        l.props.push(super::super::Prop { at: add(at, site.center), rot: rot as f32, kind, variant, scale: scale as f32 });
    };
    // (A pond 11-14 ft across the radius, half way out; a tree on the far side, nearer the edge.)
    let pond_r = 11.0 + 3.0 * h(1);
    let pond = (h(2) < 0.55).then(|| spot(a0, 0.5 * r, pond_r)).flatten();
    if let Some(p) = pond {
        put(p, crate::battlemap::Kind::Pond, 0, pond_r / 10.0, std::f64::consts::TAU * h(3));
    }
    let tree_r = 6.0 + 2.0 * h(4);
    if let Some(p) = spot(a0 + std::f64::consts::PI, (0.62 * r).min(r - tree_r - 2.0), tree_r) {
        let kind = if pond.is_some() && h(5) < 0.5 { crate::battlemap::Kind::TreeWillow } else { crate::battlemap::Kind::TreeDeciduous };
        put(p, kind, (h(6) * 4.0) as u8, 0.9 + 0.35 * h(7), std::f64::consts::TAU * h(8));
    }
}

/// Whether a house along `u` stands square to every lane and road within `SQUARE_FT` of it (but by
/// `SQUARE_DEG`): none runs past it at a slant.
/// (By `JUNCTION_DEG` but along `own`, the lane it fronts, by `own_deg`: a house by a junction square
/// to both ways; along the ring round the green, square to its chord across the house, any.)
pub(super) fn square_to(lanes: &[(Vec<P>, f64)], house: &[P], u: P, own: Option<&[P]>, own_deg: f64) -> bool {
    let (c, r) = (centroid(house), house.iter().map(|q| dist(*q, centroid(house))).fold(0.0, f64::max));
    let (cos, sin) = (libm::cos(own_deg.to_radians()), libm::sin(own_deg.to_radians()));
    let (cos_j, sin_j) = (libm::cos(JUNCTION_DEG.to_radians()), libm::sin(JUNCTION_DEG.to_radians()));
    let mine = |s: &[P]| own.is_some_and(|o| o.windows(2).any(|w| seg_dist(lerp(s[0], s[1], 0.5), w[0], w[1]) < 0.01));
    lanes.iter().all(|(pts, w)| {
        pts.windows(2).all(|s| {
            let e = sub(s[1], s[0]);
            let el = len(e);
            if el < 1e-6 || seg_dist(c, s[0], s[1]) > r + 0.5 * w + SQUARE_FT || poly_gap(house, s) > 0.5 * w + SQUARE_FT {
                return true;
            }
            // (Along it or across it.)
            let k = (dot(e, u) / el).abs();
            if mine(s) { k > cos || k < sin } else { k > cos_j || k < sin_j }
        })
    })
}

/// A house along `u` square by `JUNCTION_DEG` to the piece of `pts` nearest it (along the ring round
/// the green, curved: the pieces beside that one turn away from it).
pub(super) fn square_to_nearest(pts: &[P], house: &[P], u: P) -> bool {
    // (Nearest by its gap, a tie, where its front runs past a bend, to the piece by its middle.)
    let c = centroid(house);
    let key = |s: &[P]| poly_gap(house, s) + 0.05 * seg_dist(c, s[0], s[1]);
    let Some(s) = pts.windows(2).filter(|s| dist(s[0], s[1]) > 1e-6).min_by(|a, b| key(a).total_cmp(&key(b))) else { return true };
    let e = sub(s[1], s[0]);
    let k = (dot(e, u) / len(e)).abs();
    k > libm::cos(JUNCTION_DEG.to_radians()) || k < libm::sin(JUNCTION_DEG.to_radians())
}

/// The piece of ring run `pts` whose middle is nearest `a` ft along it, but not short of `min` ft:
/// that middle, the piece's direction and how far along the run the middle is.
pub(super) fn ring_spot(pts: &[P], a: f64, min: f64) -> Option<(P, P, f64)> {
    let mut s = 0.0;
    let mut best: Option<(P, P, f64)> = None;
    for w in pts.windows(2) {
        let pl = dist(w[0], w[1]);
        let m = s + 0.5 * pl;
        s += pl;
        if pl > 1e-6 && m >= min && best.is_none_or(|b| (m - a).abs() < (b.2 - a).abs()) {
            best = Some((lerp(w[0], w[1], 0.5), mul(sub(w[1], w[0]), 1.0 / pl), m));
        }
    }
    best
}

/// The width of a cottage on the ring (ft; off it 22-36).
pub(super) const RING_HOUSE_W: (f64, f64) = (20.0, 30.0);

/// How far on along the ring a spot that took no house is tried again (ft).
pub(super) const RING_STEP_FT: f64 = 6.0;

/// How near a lane's edge a house must stand square to it (ft), and by how much (degrees).
pub(super) const SQUARE_FT: f64 = 12.0;
pub(super) const SQUARE_DEG: f64 = 12.0;
const JUNCTION_DEG: f64 = 8.0;

/// A cottage's toft and croft on lane `pts`, side `side`, from `a0` to `a1` along it: its front
/// `front` ft out from the lane's middle, its sides square to the lane where they leave it (so the
/// crofts beside it share theirs), running back `depth` ft or until they meet water or a shore's
/// beach, a lane or a road (`CROFT_LANE_FT` off it; its front half a foot off its own), the green,
/// another plot or building, or ground too steep to work; at least to `house` ft (the cottage's back)
/// and 10 more, else none; deeper than wide and not a wedge. `own`: the cottage's place in the
/// buildings.
#[allow(clippy::too_many_arguments)]
pub(super) fn croft(g: &Ground, l: &Layout, pts: &[P], side: f64, a0: f64, a1: f64, front: f64, house: f64, depth: f64, own: usize) -> Option<Vec<P>> {
    let (p0, u0) = point_along(pts, a0);
    let (p1, u1) = point_along(pts, a1);
    let (n0, n1) = (mul([-u0[1], u0[0]], side), mul([-u1[1], u1[0]], side));
    let (f0, f1) = (add(p0, mul(n0, front)), add(p1, mul(n1, front)));
    if len(add(n0, n1)) < 1e-6 {
        return None;
    }
    let own_poly = &l.buildings[own].poly;
    let need = house - front + 10.0;
    // (Lines back from the front across it, sampled every 10 ft, the stop found to a foot or so: the
    // depth it reaches. Its sides 1.5 ft in, off the side a croft beside it shares.)
    let mut reach = depth;
    let inward = mul(sub(f1, f0), 1.5 / dist(f0, f1).max(1.0));
    let (s0, s1) = (add(f0, inward), sub(f1, inward));
    for k in 0..=4 {
        let t = k as f64 / 4.0;
        let (f, n) = (lerp(s0, s1, t), {
            let m = lerp(n0, n1, t);
            mul(m, 1.0 / len(m).max(1e-9))
        });
        let ok = |p: P| contains(own_poly, p) || g.open(p);
        let mut prev = g.site.height(f);
        let mut t = 10.0;
        while t <= reach {
            let p = add(f, mul(n, t));
            let h = g.site.height(p);
            if !ok(p) || (h - prev).abs() > CROFT_STEP_FT {
                let (mut lo, mut hi) = (t - 10.0, t);
                if ok(p) {
                    // (Too steep: stopped at the last sample.)
                    hi = lo;
                }
                while hi - lo > 1.5 {
                    let m = 0.5 * (lo + hi);
                    if ok(add(f, mul(n, m))) { lo = m } else { hi = m }
                }
                reach = lo;
                break;
            }
            prev = h;
            t += 10.0;
        }
    }
    let mut d = reach;
    while d >= need {
        let poly = vec![f0, f1, add(f1, mul(n1, d)), add(f0, mul(n0, d))];
        let (front_e, back_e) = (sub(poly[1], poly[0]), sub(poly[2], poly[3]));
        if dot(front_e, back_e) <= 0.0 || len(back_e) < 12.0 || area(&poly).abs() < 1.0 {
            d -= 20.0;
            continue;
        }
        // (Deeper than wide, and most of the rectangle round it: no wide wedge at a bend.)
        let (wide, deep) = frame(&poly);
        if wide > deep {
            return None;
        }
        if area(&poly).abs() < 0.5 * wide * deep {
            d = (0.75 * d).min(d - 15.0);
            continue;
        }
        let core = inset(&poly, &[0.3; 4]);
        // (Its cottage inside it, its sides and back 3 ft off the house: no hedge under a roof.)
        let held = inset(&poly, &[0.0, HOUSE_BOUND_FT, HOUSE_BOUND_FT, HOUSE_BOUND_FT]);
        if held.len() < 3 || !own_poly.iter().all(|c| contains(&held, *c)) {
            return None;
        }
        // (Its sides and back clear of the lanes and the roads, and dry, every 8 ft round: behind its
        // front strip, which is on the lane.)
        let behind = inset(&poly, &[CROFT_LANE_FT + 4.0, 0.5, 0.5, 0.5]);
        let every = |q: &[P], f: &dyn Fn(P) -> bool| {
            (0..q.len()).all(|k| {
                let (a, b) = (q[k], q[(k + 1) % q.len()]);
                let m = (dist(a, b) / 8.0).ceil().max(1.0) as usize;
                (0..m).all(|j| f(lerp(a, b, j as f64 / m as f64)))
            })
        };
        let round_ok = behind.len() >= 3 && every(&behind, &|q| !g.site.near_network_road(q, CROFT_LANE_FT)) && every(&poly, &|q| g.dry(q));
        // (And dry across it, every 8 ft or so.)
        let (nw, nd) = ((dist(f0, f1) / 8.0).ceil() as usize, (d / 8.0).ceil() as usize);
        let inside_ok = || (1..nd).all(|j| (1..nw).all(|i| g.dry(lerp(lerp(poly[0], poly[1], i as f64 / nw as f64), lerp(poly[3], poly[2], i as f64 / nw as f64), j as f64 / nd as f64))));
        if core.len() >= 3 && behind.len() >= 3 && !g.plot_hit(&core) && !building_hit(l, &core, Some(own)) && !g.lanes.touches(&behind, CROFT_LANE_FT) && !g.lanes.touches(&poly, 0.5) && g.off_green(&poly, 1.0) && round_ok && inside_ok() && g.plot_overlap(&poly) < 1.0 {
            return Some(poly);
        }
        d = (0.75 * d).min(d - 15.0);
    }
    None
}

/// A croft's width along its front and its depth square to it (the rectangle round it).
pub(super) fn frame(poly: &[P]) -> (f64, f64) {
    let t = sub(poly[1], poly[0]);
    let tu = mul(t, 1.0 / len(t).max(1e-9));
    let span = |u: P| poly.iter().map(|q| dot(sub(*q, poly[0]), u)).fold((f64::MAX, f64::MIN), |a, v| (a.0.min(v), a.1.max(v)));
    let ((a0, a1), (b0, b1)) = (span(tu), span([-tu[1], tu[0]]));
    (a1 - a0, b1 - b0)
}

/// The pieces of lane `pts` outside the circle round `c` of radius `r`, each at least 25 ft long.
pub(super) fn outside(pts: &[P], c: P, r: f64) -> Vec<Vec<P>> {
    let mut out: Vec<Vec<P>> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    let r2 = r * r;
    let inside = |p: P| dot(sub(p, c), sub(p, c)) < r2;
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        let (da, ab) = (sub(a, c), sub(b, a));
        // (Where the segment crosses the circle: |da + t ab| = r.)
        let (qa, qb, qc) = (dot(ab, ab), 2.0 * dot(da, ab), dot(da, da) - r2);
        let disc = qb * qb - 4.0 * qa * qc;
        let (t0, t1) = if qa > 1e-12 && disc > 0.0 {
            let sq = crate::core::sqrt(disc);
            ((-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa))
        } else {
            (2.0, 2.0)
        };
        if !inside(a) && cur.is_empty() {
            cur.push(a);
        }
        if t0 > 0.0 && t0 < 1.0 {
            cur.push(add(a, mul(ab, t0)));
            out.push(std::mem::take(&mut cur));
        }
        if t1 > 0.0 && t1 < 1.0 {
            cur.push(add(a, mul(ab, t1)));
        }
        if !inside(b) {
            cur.push(b);
        }
    }
    out.push(cur);
    out.retain(|q| q.len() >= 2 && q.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>() >= 25.0);
    out
}

/// The church on its lot inside a walled churchyard on the green's edge (east of it first: the
/// church's door toward the green, its apse away, `temple::shape`), its graveyard behind it; a cross
/// plan in a big village, else a hall. Whether there was room.
pub(super) fn churchyard(g: &mut Ground, l: &mut Layout, green_r: f64, big: bool, rng: &mut Pcg32) -> bool {
    let (Some(temple_f), Some(grave_f)) = (catalog::index_of("temple"), catalog::index_of("graveyard")) else { return false };
    let (cl, cw) = if big { (rng.range(68.0, 80.0), rng.range(46.0, 52.0)) } else { (rng.range(46.0, 60.0), rng.range(27.0, 34.0)) };
    let (front, back, side) = (12.0, rng.range(26.0, 40.0), rng.range(14.0, 22.0));
    let (depth, width) = (front + cl + back, cw + 2.0 * side);
    let a0 = ring_at(g, rng.range(-0.3, 0.3));
    // (Its gate on a lane first, else on the green; on level ground first: a churchyard is not
    // laid over a knoll.)
    let level = |yard: &[P]| {
        let (lo, hi) = g.site.relief(yard);
        hi - lo < 6.0
    };
    let by = green_r + if g.green.1 > 0.0 { LANE_W + 2.0 } else { 6.0 };
    for (ring, lane, flat) in [(by, true, true), (by, false, true), (by, true, false), (by, false, false), (green_r + 50.0, false, false)] {
        for step in ROUND {
            let a = a0 + step * std::f64::consts::PI / 6.0;
            let n = [libm::cos(a), libm::sin(a)];
            let m = add(g.green.0, mul(n, ring));
            let yard = quad(m, n, width, depth);
            if (lane && g.lanes.dist(m) > GATE_LANE_FT) || (flat && !level(&yard)) || !yard_ok(g, l, &yard) {
                continue;
            }
            let lot = rect(add(m, mul(n, front + 0.5 * cl)), n, cl, cw);
            if !g.site.buildable(&lot) {
                continue;
            }
            // (The door's end nearer the gate.)
            let door = |q: &[super::super::Part]| mul(add(q[0].poly[0], q[0].poly[3]), 0.5);
            let shaped = [false, true].map(|b| temple::shape(&lot, b, false));
            let best = shaped.into_iter().flatten().min_by(|x, y| dist(door(&x.1), m).total_cmp(&dist(door(&y.1), m)));
            let Some((outline, parts)) = best else { continue };
            let grave = rect(add(m, mul(n, front + cl + 0.5 * back + 1.0)), n, back - 6.0, width - 8.0);
            let mut b = new_building(g.site, outline, Ward::Temple, Some(temple_f), Structure::Roofed);
            b.parts = parts;
            l.grounds.push((l.buildings.len() as u32, lot));
            l.buildings.push(b);
            l.buildings.push(new_building(g.site, grave, Ward::Rural, Some(grave_f), Structure::Open));
            g.add(Plot { pts: yard, kind: PlotKind::Churchyard, bound: Bound::Wall });
            return true;
        }
    }
    false
}

/// A small village's shrine at the front of a small yard by the green, its graveyard behind it.
pub(super) fn shrine_yard(g: &mut Ground, l: &mut Layout, green_r: f64, bound: Bound, rng: &mut Pcg32) -> bool {
    let (Some(shrine_f), Some(grave_f)) = (catalog::index_of("shrine"), catalog::index_of("graveyard")) else { return false };
    let (width, depth) = (rng.range(38.0, 48.0), rng.range(52.0, 66.0));
    let (sw, sd) = (rng.range(22.0, 28.0), rng.range(16.0, 20.0));
    let a0 = ring_at(g, rng.range(0.0, std::f64::consts::TAU));
    let by = green_r + if g.green.1 > 0.0 { LANE_W + 2.0 } else { 6.0 };
    for (ring, lane) in [(by, true), (by, false), (green_r + 50.0, false)] {
        for step in ROUND {
            let a = a0 + step * std::f64::consts::PI / 6.0;
            let n = [libm::cos(a), libm::sin(a)];
            let m = add(g.green.0, mul(n, ring));
            let yard = quad(m, n, width, depth);
            let shrine = rect(add(m, mul(n, 6.0 + 0.5 * sd)), [n[1], -n[0]], sw, sd);
            if (lane && g.lanes.dist(m) > GATE_LANE_FT) || !yard_ok(g, l, &yard) || !g.site.buildable(&shrine) {
                continue;
            }
            let g0 = 6.0 + sd + 4.0;
            let grave = rect(add(m, mul(n, 0.5 * (g0 + depth - 3.0))), n, depth - 3.0 - g0, width - 6.0);
            l.buildings.push(new_building(g.site, shrine, Ward::Rural, Some(shrine_f), Structure::Roofed));
            l.buildings.push(new_building(g.site, grave, Ward::Rural, Some(grave_f), Structure::Open));
            g.add(Plot { pts: yard, kind: PlotKind::ShrineYard, bound });
            return true;
        }
    }
    false
}

/// The water mill: on the river's bank where a river runs by (the bank nearer the green), its
/// water-side wall a foot or few from the water: a lane along the bank there goes round its back
/// (`round_mill`). Else across that lane from the water, else by the wharf; its long side along
/// the bank, its footprint within `MILL_WATER_FT` of the water. Else none (`rural::place`: a
/// windmill on a rise).
pub(super) fn mill(g: &mut Ground, l: &mut Layout, r: f64, wharf: Option<&[P]>, lanes: &mut Vec<(Vec<P>, f64)>, n_roads: usize, rng: &mut Pcg32) -> bool {
    let Some(mill_f) = catalog::index_of("mill") else { return false };
    let (w, d) = (rng.range(30.0, 40.0), rng.range(20.0, 26.0));
    let site = g.site;
    // (Its wheel's room too: off the piers and bridges by 8 ft.)
    let decks = |l: &Layout, poly: &[P]| {
        let (c, u) = (centroid(poly), sub(poly[1], poly[0]));
        let room = rect(c, u, len(u) + 16.0, dist(poly[1], poly[2]) + 16.0);
        !l.piers.iter().chain(&l.bridges).any(|q| overlaps_or_crosses(q, &room))
    };
    let near = |g: &Ground, l: &Layout, poly: &[P]| g.off_green(poly, 6.0) && site.buildable(poly) && !building_hit(l, poly, None) && !g.plot_hit(poly) && off_yards(g, poly) && g.off_roads(poly, 2.0) && decks(l, poly) && waterside(site, poly, MILL_WATER_FT);
    let fits = |g: &Ground, l: &Layout, poly: &[P]| near(g, l, poly) && !g.lanes.touches(poly, 2.0);
    // (River pieces by their distance from the green, nearer first: the mill's middle, its way
    // along the bank, and its way from the water.)
    let mut pieces: Vec<(f64, usize)> = site.river.iter().enumerate().map(|(i, &(a, b, _))| (len(lerp(a, b, 0.5)), i)).filter(|&(dd, _)| dd > 0.3 * r && dd < 1.1 * r).collect();
    pieces.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let mut banks: Vec<(P, P, P, f64)> = Vec::new();
    for &(_, i) in pieces.iter().take(24) {
        let (a, b, hw) = site.river[i];
        let ab = sub(b, a);
        let al = len(ab);
        if al < 1e-6 {
            continue;
        }
        let q = lerp(a, b, 0.5);
        let t = mul(ab, 1.0 / al);
        let mut nrm = [-t[1], t[0]];
        if dot(nrm, q) > 0.0 {
            nrm = mul(nrm, -1.0);
        }
        banks.push((q, t, nrm, hw));
    }
    // (Else the wharf's shore: where the water starts out from points along it.)
    let mut shore: Vec<(P, P, P)> = Vec::new();
    if banks.is_empty()
        && let Some(wh) = wharf
    {
        let total: f64 = wh.windows(2).map(|s| dist(s[0], s[1])).sum();
        for at in [0.2, 0.8, 0.35, 0.65, 0.1, 0.9, 0.5] {
            let (p, u) = point_along(wh, at * total);
            let mut n = [-u[1], u[0]];
            if dot(n, p) > 0.0 {
                n = mul(n, -1.0);
            }
            if let Some(s) = (0..=40).find(|&s| site.wet(sub(p, mul(n, s as f64)))) {
                shore.push((sub(p, mul(n, s as f64 - 0.5)), u, n));
            }
        }
    }
    let put = |l: &mut Layout, poly: Vec<P>| l.buildings.push(new_building(site, poly, Ward::Rural, Some(mill_f), Structure::Roofed));
    // On the bank: its middle 12.5 ft off the channel at least (`buildable`).
    let off0 = (12.5 - 0.5 * d).max(1.0);
    let edges = banks.iter().map(|&(q, t, nrm, hw)| (add(q, mul(nrm, hw)), t, nrm, off0)).chain(shore.iter().map(|&(e, t, nrm)| (e, t, nrm, 2.0)));
    for (e, t, nrm, o) in edges {
        for off in [o, o + 3.0] {
            let c = add(e, mul(nrm, 0.5 * d + off));
            let poly = rect(c, t, w, d);
            if fits(g, l, &poly) {
                put(l, poly);
                return true;
            }
            if !near(g, l, &poly) {
                continue;
            }
            // (A lane runs there: round the mill's back, or ending at it.)
            let mut moved = lanes.clone();
            let mut ok = true;
            for (li, (pts, lw)) in moved.iter_mut().enumerate() {
                if !LaneGrid::new(&[(pts.clone(), *lw)]).touches(&poly, 2.0) {
                    continue;
                }
                match (li >= n_roads).then(|| round_mill(g, l, pts, *lw, &poly, c, t, nrm, w, d)).flatten() {
                    Some(np) => *pts = np,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            let grid = LaneGrid::new(&moved);
            if grid.touches(&poly, 2.0) || crossings(&moved, n_roads) > crossings(lanes, n_roads) {
                continue;
            }
            *lanes = moved;
            g.lanes = grid;
            put(l, poly);
            return true;
        }
    }
    // (Across the riverside lane from the water.)
    let mut spots: Vec<Vec<P>> = Vec::new();
    for &(q, t, nrm, hw) in &banks {
        for off in [4.0, 8.0, 14.0, 22.0] {
            spots.push(rect(add(q, mul(nrm, hw + 0.5 * d + off)), t, w, d));
        }
    }
    if spots.is_empty()
        && let Some(wh) = wharf
    {
        let total: f64 = wh.windows(2).map(|s| dist(s[0], s[1])).sum();
        for at in [0.2, 0.8, 0.35, 0.65] {
            let (p, u) = point_along(wh, at * total);
            let mut n = [-u[1], u[0]];
            if dot(n, p) > 0.0 {
                n = mul(n, -1.0);
            }
            spots.push(rect(add(p, mul(n, 6.0 + 0.5 * d + 3.0)), u, w, d));
        }
    }
    let Some(poly) = spots.into_iter().find(|p| fits(g, l, p)) else { return false };
    put(l, poly);
    true
}

/// A lane `lw` wide that meets the mill `poly` (middle `c`, `t` along the bank, `nrm` from the
/// water, `w` x `d`) taken round its back, clear of it by 3 ft, or cut short of it where the lane
/// ends there; the new way checked (dry, off the green, the plots, the buildings and the roads).
#[allow(clippy::too_many_arguments)]
fn round_mill(g: &Ground, l: &Layout, pts: &[P], lw: f64, poly: &[P], c: P, t: P, nrm: P, w: f64, d: f64) -> Option<Vec<P>> {
    let hw = 0.5 * lw;
    let m = poly.len();
    let hits = |p: P| contains(poly, p) || (0..m).any(|k| seg_dist(p, poly[k], poly[(k + 1) % m]) < hw + 3.0);
    // (The lane every 2 ft: which piece each sample lies on.)
    let mut samples: Vec<(usize, P)> = Vec::new();
    for (j, s) in pts.windows(2).enumerate() {
        let n = (dist(s[0], s[1]) / 2.0).ceil().max(1.0) as usize;
        samples.extend((0..n).map(|k| (j, lerp(s[0], s[1], k as f64 / n as f64))));
    }
    samples.push((pts.len() - 1, pts[pts.len() - 1]));
    let first = samples.iter().position(|&(_, p)| hits(p))?;
    let last = samples.iter().rposition(|&(_, p)| hits(p))?;
    let at = |p: P| [dot(sub(p, c), t), dot(sub(p, c), nrm)];
    // (Its corners out past the clearance: the bend rounded off below cuts into them.)
    let (w2, d2) = (0.5 * w + hw + 7.0, 0.5 * d + hw + 7.0);
    let corner = |p: P| add(c, add(mul(t, if at(p)[0] < 0.0 { -w2 } else { w2 }), mul(nrm, d2)));
    let mut head: Vec<P> = Vec::new();
    let mut fresh: Vec<P> = Vec::new();
    let mut tail: Vec<P> = Vec::new();
    if first > 0 {
        let (ja, pa) = samples[first - 1];
        head.extend_from_slice(&pts[..=ja]);
        fresh.push(pa);
        if last + 1 < samples.len() {
            fresh.push(corner(pa));
        }
    }
    if last + 1 < samples.len() {
        let (jb, pb) = samples[last + 1];
        if first > 0 {
            let b = corner(pb);
            if dist(b, fresh[fresh.len() - 1]) > 0.5 {
                fresh.push(b);
            }
        }
        fresh.push(pb);
        tail.extend_from_slice(&pts[(jb + 1).min(pts.len())..]);
    }
    // (Rounded: the corners cut twice, from the lane's last point before to its first after, those
    // two kept.)
    let mut way: Vec<P> = head.last().copied().into_iter().chain(fresh.iter().copied()).chain(tail.first().copied()).collect();
    way.dedup_by(|a, b| dist(*a, *b) < 0.5);
    for _ in 0..2 {
        if way.len() < 3 {
            break;
        }
        let mut cut = vec![way[0]];
        for s in way.windows(2) {
            cut.push(lerp(s[0], s[1], 0.25));
            cut.push(lerp(s[0], s[1], 0.75));
        }
        cut.push(way[way.len() - 1]);
        cut.remove(1);
        cut.remove(cut.len() - 2);
        way = cut;
    }
    let mut out: Vec<P> = head.iter().copied().take(head.len().saturating_sub(1)).chain(way.iter().copied()).chain(tail.iter().copied().skip(1)).collect();
    if head.is_empty() || tail.is_empty() {
        // (Cut short of it: the way is what is left.)
        out = head.iter().copied().chain(fresh.iter().copied()).chain(tail.iter().copied()).collect();
        way = fresh.clone();
    }
    out.dedup_by(|a, b| dist(*a, *b) < 0.5);
    if out.len() < 2 || out.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>() < 25.0 {
        return None;
    }
    // (No hard turn left in it: else the mill goes elsewhere.)
    if out.windows(3).any(|q| {
        let (u, v) = (sub(q[1], q[0]), sub(q[2], q[1]));
        len(u) > 1e-6 && len(v) > 1e-6 && dot(u, v) < libm::cos(MILL_TURN_DEG.to_radians()) * len(u) * len(v)
    }) {
        return None;
    }
    // (The new way: its band clear of what is there and of the mill, its ground dry.)
    let (pa, pb) = (way[0], way[way.len() - 1]);
    for s in way.windows(2) {
        let e = sub(s[1], s[0]);
        let el = len(e);
        if el < 0.5 {
            continue;
        }
        let band = quad(s[0], mul(e, 1.0 / el), lw, el);
        if building_hit(l, &band, None) || g.plot_hit(&band) || !g.off_green(&band, 1.0) || poly_gap(&band, poly) < 2.0 || overlaps_or_crosses(&band, poly) {
            return None;
        }
        // (Off the channel; off the roads but where it leaves the old way.)
        let n = (el / 3.0).ceil() as usize;
        if (0..=n).any(|k| {
            let p = lerp(s[0], s[1], k as f64 / n as f64);
            g.site.near_river(p, 1.0) || g.site.wet(p) || (dist(p, pa).min(dist(p, pb)) > lw && g.site.near_network_road(p, hw + 1.0))
        }) {
            return None;
        }
    }
    Some(out)
}

/// Water within `reach` ft of `poly`'s outline, looking straight out from each side (every 5 ft).
fn waterside(site: &Site, poly: &[P], reach: f64) -> bool {
    let n = poly.len();
    let ccw = area(poly) > 0.0;
    (0..n).any(|k| {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        let e = sub(b, a);
        let el = len(e).max(1e-9);
        let out = if ccw { [e[1] / el, -e[0] / el] } else { [-e[1] / el, e[0] / el] };
        (0..=4).any(|j| {
            let p = lerp(a, b, j as f64 / 4.0);
            let steps = (reach / 5.0) as usize;
            (1..=steps).any(|s| site.wet(add(p, mul(out, 5.0 * s as f64))))
        })
    })
}

/// The manor house in its yard on a lane toward the village's edge (or by the church), hedged or
/// walled, its gate on the lane.
pub(super) fn manor_yard(g: &mut Ground, l: &mut Layout, r: f64, lanes: &[(Vec<P>, f64)], rng: &mut Pcg32) -> bool {
    let (yw, yd) = (rng.range(80.0, 110.0), rng.range(70.0, 100.0));
    let (hw_, hd) = (rng.range(48.0, 62.0), rng.range(30.0, 38.0));
    let bound = if rng.next_f64() < 0.4 { Bound::Wall } else { Bound::Hedge };
    let by_church = rng.next_f64() < 0.5;
    let church = l.buildings.iter().find(|b| temple::is_temple(b)).map(|b| centroid(&b.poly));
    let jit = hash3(rng.next_f64().to_bits(), 0, 0, 0x3a40);
    // (Spots every 40 ft along the lanes, either side, 0.45-1.0 of the radius out; the nearest the
    // church first, else the farthest out.)
    let mut spots: Vec<(f64, P, P)> = Vec::new();
    for (li, (pts, w)) in lanes.iter().enumerate() {
        let total: f64 = pts.windows(2).map(|s| dist(s[0], s[1])).sum();
        let mut a = 20.0;
        let mut k = 0;
        while a < total {
            let (p, u) = point_along(pts, a);
            let rr = len(p);
            if rr > 0.45 * r && rr < r {
                for side in [-1.0, 1.0] {
                    let n = mul([-u[1], u[0]], side);
                    let m = add(p, mul(n, 0.5 * w + 2.0));
                    let score = match church.filter(|_| by_church) {
                        Some(c) => dist(m, c),
                        None => -rr,
                    } + 30.0 * unit(hash3(jit, li as i64, k, side as i64));
                    spots.push((score, m, n));
                }
            }
            a += 40.0;
            k += 1;
        }
    }
    spots.sort_by(|x, y| x.0.total_cmp(&y.0));
    for &(_, m, n) in spots.iter().take(60) {
        let yard = quad(m, n, yw, yd);
        let house = rect(add(m, mul(n, 0.55 * yd)), [n[1], -n[0]], hw_, hd);
        if !g.site.buildable(&house) || !yard_ok(g, l, &yard) {
            continue;
        }
        l.buildings.push(new_building(g.site, house, Ward::Noble, None, Structure::Roofed));
        g.add(Plot { pts: yard, kind: PlotKind::ManorYard, bound });
        return true;
    }
    false
}

/// What a village's layout set out for a purpose `assign_functions` leaves be: its church stands for
/// its shrine; its mill is the layout's or none; its manor (the noble estate, `Ward::Noble`) is no
/// one's business.
pub(in crate::town) fn reserve(l: &Layout, needs: &mut Vec<usize>, taken: &mut [bool]) {
    if l.buildings.iter().any(temple::is_temple)
        && let Some(sh) = catalog::index_of("shrine")
        && let Some(k) = needs.iter().position(|&f| f == sh)
    {
        needs.remove(k);
    }
    // (A village's mill stands only by the water, where its layout put it: no mill on a dry lot.)
    if let Some(mill) = catalog::index_of("mill") {
        needs.retain(|&f| f != mill);
    }
    // (Nor a farmstead's farmhouse: a farmer's.)
    for (i, b) in l.buildings.iter().enumerate() {
        if b.ward == Ward::Noble || b.ward == Ward::Farm {
            taken[i] = true;
        }
    }
}

/// Whether a house `assign_functions` adds to make up the numbers (local ft) keeps off a village's
/// lanes, roads, green and plots; anywhere in any other layout.
pub(in crate::town) fn spare_ok(site: &Site, l: &Layout, poly: &[P]) -> bool {
    if !village_of(l) {
        return true;
    }
    let m = poly.len();
    let edges = || (0..m).map(|k| [poly[k], poly[(k + 1) % m]]);
    let lanes_ok = l.streets.iter().all(|s| s.windows(2).all(|w| !contains(poly, w[0]) && edges().all(|e| poly_gap(&e, w) > 7.0)));
    let roads_ok = edges().all(|e| (0..=8).all(|j| !site.near_network_road(lerp(e[0], e[1], j as f64 / 8.0), 1.0)));
    let green_ok = l.plazas.iter().filter(|p| p.kind == PlazaKind::Green).all(|g| {
        let (c, r) = (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0]));
        !contains(poly, c) && edges().all(|e| seg_dist(c, e[0], e[1]) > r + 2.0)
    });
    lanes_ok && roads_ok && green_ok && l.crofts.iter().all(|p| !overlaps_or_crosses(&p.pts, poly) && poly_gap(&p.pts, poly) >= CLEAR_FT)
}

/// The way the lane or road nearest `c` runs (within 80 ft) in a village: a house `assign_functions`
/// adds stands square to it.
pub(in crate::town) fn spare_dir(l: &Layout, c: P) -> Option<P> {
    if !village_of(l) {
        return None;
    }
    let segs = l.streets.iter().map(|s| s.as_slice()).chain(l.roads.iter().map(|r| r.0.as_slice())).flat_map(|s| s.windows(2));
    let (d, s) = segs.map(|w| (seg_dist(c, w[0], w[1]), w)).filter(|x| dist(x.1[0], x.1[1]) > 1e-6).min_by(|a, b| a.0.total_cmp(&b.0))?;
    (d < 80.0).then(|| sub(s[1], s[0]))
}

/// What `assign_functions` adds to a village house's score for an inn or a tavern by the green
/// (most on its edge, none 80 ft off it): the inn fronts the green where a house there can be one.
pub(in crate::town) fn green_bonus(l: &Layout, poly: &[P], key: &str) -> f64 {
    if !village_of(l) || !matches!(key, "inn" | "tavern") {
        return 0.0;
    }
    let Some(g) = l.plazas.iter().find(|p| p.kind == PlazaKind::Green) else { return 0.0 };
    let (c, r) = (centroid(&g.pts), dist(centroid(&g.pts), g.pts[0]));
    let off = poly.iter().map(|q| dist(*q, c)).fold(f64::MAX, f64::min) - r;
    8.0 * (1.0 - off / 80.0).clamp(0.0, 1.0)
}
