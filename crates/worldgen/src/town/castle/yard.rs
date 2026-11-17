//! The bailey: ranges, yard trades and buildings, and cutting the bound into lots.

use super::*;

/// The ranges' sizes (ft): length least and most, depth least and most, and how many at most.
/// A castle's ranges, and a great castle's hall range (`Setting::hall`).
pub(super) const RANGE: (f64, f64, f64, f64, usize) = (24.0, 44.0, 16.0, 24.0, usize::MAX);
pub(super) const HALL: (f64, f64, f64, f64, usize) = (44.0, 64.0, 22.0, 30.0, 1);

/// Ranges against the inside of the curtain (local ft), or of the town wall where that is the
/// castle's (`walled`): blocks `size` deep and long along each stretch (a hall range along the
/// longest first), clear of the
/// walls, its towers and gate, of the keep (and its motte) by 12 ft, of an
/// inner curtain (20 ft, its towers 10 ft: the outer bailey stays a way round), of the way from
/// gate to gate (`lanes`, 20 ft) and of each other (and of the castle's buildings from `from`),
/// on ground fit to build on; stone ranges under slate, as the castle's own.
#[allow(clippy::too_many_arguments)]
pub(super) fn ranges<G: Ground>(
    site: &G,
    ring: &[P],
    walled: &[bool],
    inner: Option<&[P]>,
    gates: &[P],
    towers: &[(P, f64)],
    inner_towers: &[(P, f64)],
    lanes: &[(P, P)],
    keep: &[P],
    motte: Option<Motte>,
    w: Ward,
    size: (f64, f64, f64, f64, usize),
    from: usize,
    prng: &mut Pcg32,
    l: &mut Layout,
) {
    let n = ring.len();
    let (lo, hi, dlo, dhi, most) = size;
    let first = l.buildings.len();
    // (The side each range stands along; none for the castle's buildings before these.)
    let mut sides: Vec<usize> = vec![usize::MAX; first - from];
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let mut order: Vec<usize> = (0..n).collect();
    if most < usize::MAX {
        order.sort_by(|&x, &y| dist(ring[y], ring[(y + 1) % n]).total_cmp(&dist(ring[x], ring[(x + 1) % n])).then(x.cmp(&y)));
    }
    for e in order {
        if l.buildings.len() - first >= most {
            break;
        }
        let (a, b) = (ring[e], ring[(e + 1) % n]);
        // (6 ft in from the curtain's line; the town wall's runs 6 ft out from the ring, but where a
        // street runs along its foot.)
        let backs: &[f64] = if walled[e] { &[0.0, 6.0] } else { &[6.0] };
        let el = dist(a, b);
        if el < lo + 16.0 {
            continue;
        }
        let u = mul(sub(b, a), 1.0 / el);
        // Inward (the ring runs anticlockwise).
        let nrm = [-u[1], u[0]];
        let mut t = 16.0;
        while t + lo <= el - 16.0 && l.buildings.len() - first < most {
            let length = prng.range(lo, hi).min(el - 16.0 - t);
            let depth = prng.range(dlo, dhi);
            let gap = prng.range(6.0, 12.0);
            let at = |s: f64, d: f64| add(add(a, mul(u, s)), mul(nrm, d));
            let mut placed = None;
            for &back in backs {
                let poly = vec![at(t, back), at(t + length, back), at(t + length, back + depth), at(t, back + depth)];
                let c = centroid(&poly);
                let inside = poly.iter().all(|p| inset_depth(ring, *p) >= back - 1.0)
                    && !ring.iter().any(|p| inset_depth(&poly, *p) > 1.0)
                    && walls.iter().all(|&(p, q)| super::super::poly_seg_dist(&poly, p, q) >= 5.5)
                    // (Its yard side two squares clear of any wall's foot: no neck of the bailey
                    // shut, its end cut off.)
                    && walls.iter().all(|&(p, q)| super::super::poly_seg_dist(&poly[2..], p, q) >= 16.0);
                let clear_inner = inner.is_none_or(|q| poly.iter().all(|p| inset_depth(q, *p) <= -20.0) && !q.iter().any(|p| inset_depth(&poly, *p) > -20.0))
                    && inner_towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 10.0));
                let clear_lanes = lanes.iter().all(|&(p, q)| super::super::poly_seg_dist(&poly, p, q) >= 20.0 && !contains(&poly, p));
                let clear_keep = keep.is_empty() || (poly.iter().all(|p| inset_depth(keep, *p) <= -12.0) && !keep.iter().any(|p| inset_depth(&poly, *p) >= -12.0));
                let clear_motte = motte.is_none_or(|m| poly.iter().all(|p| dist(*p, m.at) >= m.foot_r + 4.0) && dist(c, m.at) >= m.foot_r + 4.0);
                let clear_gate = gates
                    .iter()
                    .all(|g| poly.iter().all(|p| dist(*p, *g) >= GATE_HALF + 18.0) && inset_depth(&poly, *g) < -(GATE_HALF + 12.0));
                let clear_towers = towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 2.0));
                // (Two squares at least from a range along another side, as at a sharp corner: no yard
                // shut in behind them.)
                let clear_others = l.buildings[from..].iter().zip(&sides).all(|(o, &f)| {
                    let gap = if f == e { 4.0 } else { 10.0 };
                    !super::super::overlaps_or_crosses(&o.poly, &poly) && poly.iter().all(|p| inset_depth(&o.poly, *p) <= -gap) && o.poly.iter().all(|p| inset_depth(&poly, *p) <= -gap)
                });
                // (Off the streets and roads: a paved strip by the bailey.)
                let clear_roads = l
                    .roads
                    .iter()
                    .all(|(pts, _, rw)| pts.windows(2).all(|s| super::super::poly_seg_dist(&poly, s[0], s[1]) >= 0.5 * rw + 1.0));
                let rest = inside && clear_inner && clear_lanes && clear_keep && clear_motte && clear_gate && clear_towers && clear_others;
                // (Set back further only where it stands on a street.)
                if !rest || clear_roads {
                    placed = (rest && site.buildable(&poly)).then_some(poly);
                    break;
                }
            }
            if let Some(poly) = placed {
                let pad = site.pad(&poly);
                l.buildings.push(Building {
                    poly,
                    ward: w,
                    func: None,
                    residential: 0,
                    name: None,
                    floors: 1,
                    pad_ft: pad,
                    structure: Structure::Roofed,
                    roof: None,
                    // (Slate: `world::TINTS`.)
                    tint: Some(1),
                    material: None,
                    shape: None,
            shade: None,
                    id: 0,
                    parts: Vec::new(),
                    keep: None,
                    deck: None,
                });
                sides.push(e);
                t += length + gap;
            } else {
                t += 8.0;
            }
        }
    }
}

/// The free-standing buildings of a town castle's yard: stables and a smithy (`TRADES`, in every
/// bailey they fit in), then a barracks and a chapel (a shrine) as the open yard holds them (one
/// building in 12,000 sq ft of it, 4 at most).
const YARD: [(&str, f64, f64, f64, f64); 4] = [("stables", 34.0, 44.0, 20.0, 24.0), ("blacksmith", 30.0, 38.0, 18.0, 22.0), ("barracks", 44.0, 56.0, 22.0, 26.0), ("shrine", 26.0, 32.0, 16.0, 20.0)];
/// The first of `YARD` every bailey gets (stables and a smithy): where the yard is tight, at
/// their least size, else smaller again (`TRADES_LEAST` of it), else in a range (`yard_trades`).
const TRADES: usize = 2;
const TRADES_LEAST: f64 = 0.8;

/// Stables and a smithy in every town castle's bailey (`TRADES`): those its yard had no room for
/// (or no yard buildings at all) go in its ranges (from `first`, past a great castle's hall
/// range), the stables in the biggest, the smithy in the smallest left. (The castle's buildings
/// from `from`.)
pub(super) fn yard_trades(l: &mut Layout, from: usize, first: usize) {
    for (key, biggest) in YARD[..TRADES].iter().map(|y| y.0).zip([true, false]) {
        let Some(fi) = super::super::catalog::index_of(key) else { continue };
        if l.buildings[from..].iter().any(|b| b.func == Some(fi as u16)) {
            continue;
        }
        let size = |i: usize| area(&l.buildings[i].poly).abs();
        let pick = (first..l.buildings.len())
            .filter(|&i| l.buildings[i].func.is_none() && l.buildings[i].keep.is_none())
            .max_by(|&a, &b| if biggest { size(a).total_cmp(&size(b)) } else { size(b).total_cmp(&size(a)) }.then(b.cmp(&a)));
        if let Some(i) = pick {
            l.buildings[i].func = Some(fi as u16);
        }
    }
}

/// The yard's buildings (`YARD`) in the bailey (the outer one, where there is an inner), standing
/// free: 24 ft in from the curtain at least, 16 ft clear of the ranges and of each other and of the
/// keep (its motte 12), 20 of an inner curtain, of the way from gate to gate and of the gates, on
/// ground fit to build on; each along the curtain nearest it, as near it as it goes (the middle
/// of the yard left open). How many were placed.
#[allow(clippy::too_many_arguments)]
pub(super) fn yard_buildings<G: Ground>(
    site: &G,
    ring: &[P],
    inner: Option<&[P]>,
    gates: &[P],
    towers: &[(P, f64)],
    inner_towers: &[(P, f64)],
    lanes: &[(P, P)],
    keep: &[P],
    motte: Option<Motte>,
    w: Ward,
    from: usize,
    l: &mut Layout,
) -> u8 {
    let n = ring.len();
    if n < 3 {
        return 0;
    }
    // (The yard left open: the bailey but the inner one, the keep and the ranges.)
    let built: f64 = l.buildings[from..].iter().map(|b| area(&b.poly).abs()).sum::<f64>() + area(keep).abs();
    let open = area(ring).abs() - inner.map_or(0.0, |q| area(q).abs()) - built;
    let want = ((open / 12_000.0).max(0.0) as usize).clamp(TRADES, YARD.len());
    let (x0, y0, x1, y1) = ring.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let step = 6.0;
    let (nx, ny) = (crate::core::ceil((x1 - x0) / step) as usize, crate::core::ceil((y1 - y0) / step) as usize);
    // (The roads and the town's main streets, 18 ft wide.)
    let roads: Vec<(P, P, f64)> = l
        .roads
        .iter()
        .flat_map(|(pts, _, rw)| pts.windows(2).map(move |s| (s[0], s[1], *rw)))
        .chain(l.streets.iter().flat_map(|pts| pts.windows(2).map(|s| (s[0], s[1], 18.0))))
        .collect();
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let mut placed = 0;
    for (yi, &(key, l0, l1, d0, d1)) in YARD.iter().enumerate().take(want) {
        // (Sized by the yard: the more room, the longer.)
        let f = (open / 60_000.0).clamp(0.0, 1.0);
        let (long, deep) = (l0 + f * (l1 - l0), d0 + f * (d1 - d0));
        let mut best: Option<(f64, Vec<P>)> = None;
        // (Stables and a smithy smaller where the yard is tight: `TRADES`.)
        let sizes = if yi < TRADES { vec![(long, deep), (l0, d0), (TRADES_LEAST * l0, TRADES_LEAST * d0), (0.6 * l0, 0.6 * d0)] } else { vec![(long, deep)] };
        for (long, deep) in sizes {
            if best.is_some() {
                break;
            }
            for j in 0..=ny {
                for i in 0..=nx {
                    let c = [x0 + i as f64 * step, y0 + j as f64 * step];
                    let depth = inset_depth(ring, c);
                    if depth < 24.0 + 0.5 * deep || best.as_ref().is_some_and(|b| b.0 <= depth) {
                        continue;
                    }
                    // (Along the curtain nearest it.)
                    let Some(e) = (0..n).min_by(|&a, &b| seg_dist(c, ring[a], ring[(a + 1) % n]).total_cmp(&seg_dist(c, ring[b], ring[(b + 1) % n])).then(a.cmp(&b))) else {
                        continue;
                    };
                    let poly = super::super::rect(c, sub(ring[(e + 1) % n], ring[e]), long, deep);
                    let fits = poly.iter().all(|p| inset_depth(ring, *p) >= 24.0)
                        && walls.iter().all(|&(p, q)| super::super::poly_seg_dist(&poly, p, q) >= 20.0)
                        && inner.is_none_or(|q| poly.iter().all(|p| inset_depth(q, *p) <= -20.0) && !q.iter().any(|p| inset_depth(&poly, *p) > -20.0))
                        && inner_towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 10.0))
                        && towers.iter().all(|(q, r)| inset_depth(&poly, *q) < -(r + 12.0))
                        && lanes.iter().all(|&(p, q)| super::super::poly_seg_dist(&poly, p, q) >= 20.0 && !contains(&poly, p))
                        && gates.iter().all(|g| poly.iter().all(|p| dist(*p, *g) >= GATE_HALF + 24.0) && inset_depth(&poly, *g) < -(GATE_HALF + 18.0))
                        && (keep.is_empty() || (poly.iter().all(|p| inset_depth(keep, *p) <= -16.0) && !keep.iter().any(|p| inset_depth(&poly, *p) >= -16.0)))
                        && motte.is_none_or(|m| poly.iter().all(|p| dist(*p, m.at) >= m.foot_r + 12.0) && dist(c, m.at) >= m.foot_r + 12.0)
                        && l.buildings[from..].iter().all(|o| {
                            !super::super::overlaps_or_crosses(&o.poly, &poly) && poly.iter().all(|p| inset_depth(&o.poly, *p) <= -16.0) && o.poly.iter().all(|p| inset_depth(&poly, *p) <= -16.0)
                        })
                        && roads.iter().all(|&(p, q, rw)| super::super::poly_seg_dist(&poly, p, q) >= 0.5 * rw + 2.0)
                        && site.buildable(&poly);
                    if fits {
                        best = Some((depth, poly));
                    }
                }
            }
        }
        let Some((_, poly)) = best else { continue };
        let pad = site.pad(&poly);
        l.buildings.push(Building {
            poly,
            ward: w,
            func: super::super::catalog::index_of(key).map(|f| f as u16),
            residential: 0,
            name: None,
            floors: 1,
            pad_ft: pad,
            structure: Structure::Roofed,
            roof: None,
            tint: Some(1),
            material: None,
            shape: None,
            shade: None,
            id: 0,
            parts: Vec::new(),
            keep: None,
            deck: None,
        });
        placed += 1;
    }
    placed
}

/// Why a town castle's patch was not cut to its size (`cut_bound`; `Layout::castle_cut`): cut (1);
/// the patch too small to leave lots (2); a cut past the sides on the town wall leaves too little,
/// or keeps half as much again as wanted (3); what is left too small or narrow for lots (4); the
/// castle's part too narrow or sharp (5); its part held no keep, or its ring came out with a sharp
/// corner, so the castle has the whole patch after all (6).
pub(in crate::town) const CUT_DONE: u8 = 1;
pub(in crate::town) const CUT_SMALL: u8 = 2;
pub(in crate::town) const CUT_WALL: u8 = 3;
pub(in crate::town) const CUT_LEFT: u8 = 4;
pub(in crate::town) const CUT_NARROW: u8 = 5;
pub(in crate::town) const CUT_KEEP: u8 = 6;
/// The lane between the castle and the lots left (ft), and those lots' least area and width.
pub(in crate::town) const LANE: f64 = 14.0;
const LEFT_AREA: f64 = 9_000.0;
const LEFT_WIDE: f64 = 60.0;

/// A town castle's block cut with one chord, parallel to one of its sides: the castle's part
/// (`target` sq ft), a lane `LANE` wide, and the rest of the patch back to the town.
pub(in crate::town) struct Cut {
    pub kept: Vec<P>,
    pub left: Vec<P>,
    /// The lane's middle across the patch, and the street line of the lots' side on it (outward
    /// normal, offset: as `street_lines`).
    pub lane: (P, P),
    pub line: (P, f64),
}

/// The cut (`Cut`) of `block` (convex, its patch `patch`) leaving the castle `target` sq ft of the
/// patch: on the low side and towards `toward` (a unit direction: away from the town wall, or
/// towards the market), never leaving out a side on the town wall (`walls`, the patch's sides
/// there), with at least `LEFT_AREA` and `LEFT_WIDE` of the block left and the castle's part of the
/// patch `wide` across. Else why not (`CUT_*`).
pub(in crate::town) fn cut_bound(block: &[P], patch: &[P], walls: &[(P, P)], toward: P, height: &dyn Fn(P) -> f64, target: f64, wide: f64) -> Result<Cut, u8> {
    // (Measured on the patch, not the block: the same cut, and the same lots left, whether the town
    // wall stands or not.)
    let mut pp = patch.to_vec();
    if area(&pp) < 0.0 {
        pp.reverse();
    }
    let n = pp.len();
    if n < 3 || block.len() < 3 || area(&pp) < target + LEFT_AREA + 2_000.0 {
        return Err(CUT_SMALL);
    }
    // (A side on the wall and its block's corners, within 12 ft of its ends along the sides off it:
    // on the castle's side.)
    let fixed: Vec<P> = (0..n)
        .filter(|&k| walls.iter().any(|&(a, b)| dist(a, pp[k]) < 0.5 || dist(b, pp[k]) < 0.5))
        .flat_map(|k| {
            let (prev, next) = (pp[(k + n - 1) % n], pp[(k + 1) % n]);
            let step = |q: P| add(pp[k], mul(sub(q, pp[k]), 12.0_f64.min(dist(q, pp[k])) / dist(q, pp[k]).max(1e-9)));
            [pp[k], step(prev), step(next)]
        })
        .collect();
    let half = 0.5 * LANE;
    let mut why = CUT_SMALL;
    let mut best: Option<(f64, Cut)> = None;
    // (Across from a side: its outward normal, the lots on that side; along a side on the wall: away
    // from it; and `toward` itself.)
    let dirs: Vec<P> = (0..n)
        .filter(|&e| dist(pp[e], pp[(e + 1) % n]) >= LEFT_WIDE)
        .map(|e| {
            let (a, b) = (pp[e], pp[(e + 1) % n]);
            mul([b[1] - a[1], a[0] - b[0]], 1.0 / dist(a, b))
        })
        .chain(walls.iter().filter(|w| dist(w.0, w.1) > 1.0).map(|&(a, b)| {
            let v = mul(sub(b, a), 1.0 / dist(a, b));
            let nrm = [-v[1], v[0]];
            if dot(sub(centroid(&pp), a), nrm) >= 0.0 { nrm } else { mul(nrm, -1.0) }
        }))
        .chain((len(toward) > 0.5).then_some(toward))
        .collect();
    for u in dirs {
        let (lo, hi) = pp.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(dot(*p, u)), m.1.max(dot(*p, u))));
        let kept_at = |s: f64| clip_plain(&pp, u, s - half);
        let (mut s0, mut s1) = (lo + half, hi + half);
        for _ in 0..40 {
            let s = 0.5 * (s0 + s1);
            if area(&kept_at(s)).abs() < target {
                s0 = s;
            } else {
                s1 = s;
            }
        }
        // (Past the wall's side, where it reaches further: what is left is still the town's.)
        let mut s = fixed.iter().map(|p| dot(*p, u) + half + 1.0).fold(0.5 * (s0 + s1), f64::max);
        let pushed = s > 0.5 * (s0 + s1) + 0.5;
        // (And past where the castle's part is `wide` across.)
        while s < hi + half && obb(&kept_at(s)).short < wide {
            s += 5.0;
        }
        let left = clip_plain(block, mul(u, -1.0), -(s + half));
        if left.len() < 3 || area(&left).abs() < LEFT_AREA || obb(&left).short < LEFT_WIDE {
            why = why.max(if pushed { CUT_WALL } else { CUT_LEFT });
            continue;
        }
        let part = kept_at(s);
        // (Not where the wall's side keeps it twice as big: the castle has the whole patch.)
        if pushed && area(&part) > 2.0 * target {
            why = why.max(CUT_WALL);
            continue;
        }
        // (Nor a corner of it sharper than 50 degrees where the cut meets a side: a wedge of yard.)
        let k = part.len();
        let sharp = (0..k).any(|j| {
            let (a, b) = (sub(part[(j + k - 1) % k], part[j]), sub(part[(j + 1) % k], part[j]));
            let (la, lb) = (len(a), len(b));
            la > 1.0 && lb > 1.0 && dot(a, b) / (la * lb) > libm::cos(50.0_f64.to_radians())
        });
        if k < 3 || sharp || obb(&part).short < wide {
            why = why.max(CUT_NARROW);
            continue;
        }
        let kept = clip_plain(block, u, s - half);
        if kept.len() < 3 {
            continue;
        }
        // The lane's middle: the line across the patch.
        let mut ends: Vec<P> = Vec::new();
        let m = patch.len();
        for k in 0..m {
            let (p, q) = (patch[k], patch[(k + 1) % m]);
            let (dp, dq) = (dot(p, u) - s, dot(q, u) - s);
            if (dp < 0.0) != (dq < 0.0) {
                ends.push(lerp(p, q, dp / (dp - dq)));
            }
        }
        let along = [-u[1], u[0]];
        let (Some(ea), Some(eb)) = (
            ends.iter().copied().min_by(|x, y| dot(*x, along).total_cmp(&dot(*y, along))),
            ends.iter().copied().max_by(|x, y| dot(*x, along).total_cmp(&dot(*y, along))),
        ) else {
            continue;
        };
        // (Lower ground left to the town, the side `toward` preferred, and the castle's part the
        // nearest its size.)
        let score = height(centroid(&left)) - height(centroid(&part)) - 8.0 * dot(u, toward) + (area(&part) - target).max(0.0) / 2_000.0;
        if best.as_ref().is_none_or(|x| score < x.0) {
            best = Some((score, Cut { kept, left, lane: (ea, eb), line: (mul(u, -1.0), -(s + half)) }));
        }
    }
    best.map(|b| b.1).ok_or(why)
}
