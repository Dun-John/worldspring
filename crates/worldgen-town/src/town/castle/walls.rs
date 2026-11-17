//! Rings and curtain walls: brow and inner rings, the curtain, gate ways and ways round.

use super::*;

/// The curtain along the brow: round the high point `o` (its ground `top` ft), out along each ray
/// to where the ground first falls `drop` ft below the top (or the water), never past the block's
/// bound (`outer`) nor nearer the top than `R_MIN`; smoothed and tidied. The nearest the top that
/// holds at least `least` of the bound (half; a town's castle on a patch cut to its size, 80%), and
/// room for a keep and a yard, else None (the bound).
pub(super) fn brow_ring<G: Ground>(site: &G, outer: &[P], walled: &[bool], o: P, top: f64, a_out: f64, least: f64) -> Option<Vec<P>> {
    let a = rays(outer, o);
    let hits: Vec<(f64, usize)> = a.iter().map(|&t| ray_hit(outer, o, [libm::cos(t), libm::sin(t)])).collect();
    let max: Vec<f64> = hits.iter().map(|h| h.0).collect();
    // (Out to a side along a town wall: that wall is the castle's there, its corners kept.)
    let n = outer.len();
    let fixed: Vec<P> = (0..n).filter(|&k| walled[k] || walled[(k + n - 1) % n]).map(|k| outer[k]).collect();
    let forced: Vec<bool> = hits
        .iter()
        .zip(&a)
        .map(|(h, &t)| walled.get(h.1).copied().unwrap_or(false) || fixed.iter().any(|f| dist(*f, add(o, mul([libm::cos(t), libm::sin(t)], h.0))) < 0.5))
        .collect();
    // Each ray's ground every 5 ft (none on the water).
    let profiles: Vec<Vec<Option<f64>>> = a
        .iter()
        .zip(&max)
        .map(|(&t, &m)| {
            let d = [libm::cos(t), libm::sin(t)];
            let n = crate::core::floor(m / 5.0) as usize;
            (1..=n)
                .map(|k| add(o, mul(d, k as f64 * 5.0)))
                .map(|p| (!site.near_water(p, WATER_BACK)).then(|| site.height(p)))
                .collect()
        })
        .collect();
    let want = (least * a_out).max((0.9 * a_out).min(14_000.0));
    for drop in DROPS {
        let r: Vec<f64> = profiles
            .iter()
            .zip(&max)
            .zip(&forced)
            .map(|((pr, &m), &forced)| {
                if forced {
                    return m;
                }
                let fall = pr.iter().position(|h| h.is_none_or(|h| h < top - drop));
                let r = fall.map_or(m, |k| k as f64 * 5.0 + 2.5);
                r.max(R_MIN.min(m)).min(m)
            })
            .collect();
        // (Along a town wall, on it: not smoothed.)
        let r: Vec<f64> = smooth(&r, &max).iter().zip(&max).zip(&forced).map(|((&s, &m), &f)| if f { m } else { s }).collect();
        let ring = polar(o, &a, &r);
        if let Some(ring) = tidy(&ring, &fixed)
            && area(&ring) >= want
            && contains(&ring, o)
        {
            return Some(ring);
        }
    }
    None
}

//// An inner curtain round `o` in a big castle: along the brow of a higher drop (on flat ground
/// about halfway out), at least `BAILEY_GAP` inside the outer `ring` all round and `BROAD` on the
/// side of its gate (`gate`), so the outer bailey is a yard to use there; None if that doesn't fit,
/// or what is left is too small for a keep and a yard or too big to leave an outer bailey.
pub(super) fn inner_ring<G: Ground>(site: &G, ring: &[P], o: P, top: f64, flat: bool, gate: P, cone: bool) -> Option<Vec<P>> {
    let a = rays(ring, o);
    let to_gate = sub(gate, o);
    let g = mul(to_gate, 1.0 / len(to_gate).max(1e-9));
    let exit: Vec<f64> = a.iter().map(|&t| ray_exit(ring, o, [libm::cos(t), libm::sin(t)])).collect();
    // (A little more than each: the ring's sides between its rays come nearer.)
    const MORE: f64 = 6.0;
    let max: Vec<f64> = a
        .iter()
        .zip(&exit)
        .map(|(&t, &e)| {
            let d = [libm::cos(t), libm::sin(t)];
            // (With `cone`, the outer bailey broad all across the gate's side, not only before it.)
            let facing = cone && dot(d, g) > 0.64;
            let by_wall = e - if facing { BROAD } else { BAILEY_GAP } - MORE;
            // Out to where it comes `BROAD` from the gate.
            let (along, off) = (dot(d, to_gate), cross(d, to_gate).abs());
            let reach = BROAD + MORE;
            let by_gate = if along > 0.0 && off < reach {
                along - crate::core::sqrt(reach * reach - off * off)
            } else {
                f64::MAX
            };
            by_wall.min(by_gate)
        })
        .collect();
    if max.iter().any(|&m| m < R_MIN) {
        return None;
    }
    let r: Vec<f64> = a
        .iter()
        .zip(&max)
        .zip(&exit)
        .map(|((&t, &m), &e)| {
            let d = [libm::cos(t), libm::sin(t)];
            let half = 0.55 * e;
            if flat {
                return half.clamp(R_MIN, m);
            }
            let n = crate::core::floor(m / 5.0) as usize;
            let fall = (1..=n).find(|&k| {
                let p = add(o, mul(d, k as f64 * 5.0));
                site.wet(p) || site.height(p) < top - 5.0
            });
            fall.map_or(half, |k| (k as f64 * 5.0 + 2.5).min(half)).clamp(R_MIN, m)
        })
        .collect();
    let inner = tidy(&polar(o, &a, &smooth(&r, &max)), &[])?;
    let (ai, ao) = (area(&inner), area(ring));
    // Every stretch of it (not only its corners) the outer bailey's width from the outer curtain
    // and `BROAD` from the outer gate, and broad somewhere.
    let n = inner.len();
    let mut broad = 0.0;
    for k in 0..n {
        let (p, q) = (inner[k], inner[(k + 1) % n]);
        let steps = crate::core::ceil(dist(p, q) / 5.0).max(1.0) as usize;
        for j in 0..steps {
            let at = lerp(p, q, j as f64 / steps as f64);
            let d = inset_depth(ring, at);
            if d < BAILEY_GAP || dist(at, gate) < BROAD {
                return None;
            }
            if d >= BROAD - 10.0 {
                broad += dist(p, q) / steps as f64;
            }
        }
    }
    (ai >= 7_000.0 && ai <= 0.55 * ao && broad >= 60.0 && contains(&inner, o)).then_some(inner)
}

/// A curtain on `ring` (local ft) with its gatehouse at the place on a side with room for it (its
/// towers clear of the corners) where `ok` holds that `score` likes best (lowest), and towers on
/// every other corner and along long runs (square in a castle built so), each clear of the towers
/// already standing (`others`, which gets them: `TOWERS_CLEAR` from a gate tower, `gap` from any
/// other). The sides `walled` (along a town wall) get no
/// curtain while that wall stands (`up`): the rest runs from that wall round to it again; no gate
/// is on one either way (a stub left by a gap in it dropped where `stubs`). `want`: where a hand
/// castle's gates go (the main one first; each on the nearest side off the town wall, along it as
/// far as its towers keep clear of the corners, `HAND_GATES_APART`), its gates past the main one
/// pushed to `extra` (each with its way in). Returns the gate and the way in there (unit).
#[allow(clippy::too_many_arguments)]
pub(super) fn curtain(
    l: &mut Layout,
    ring: &[P],
    walled: &[bool],
    up: bool,
    score: &dyn Fn(P) -> f64,
    ok: &dyn Fn(P) -> bool,
    style: &WallStyle,
    gap: f64,
    others: &mut Vec<(P, f64)>,
    stubs: bool,
    want: &[P],
    extra: &mut Vec<(P, P)>,
    dropped: &mut Vec<(usize, String)>,
) -> (P, P) {
    let n = ring.len();
    let edge = |e: usize| (ring[e], ring[(e + 1) % n]);
    // (A town wall all round: none is.)
    let walled: Vec<bool> = if walled.iter().all(|&w| w) { vec![false; n] } else { walled.to_vec() };
    // (Its towers clear of the corners' towers: 30 ft from the passage to a corner.)
    let room = 2.0 * (GATE_HALF + 30.0);
    let mut spots: Vec<(usize, f64, P)> = Vec::new();
    for e in (0..n).filter(|&e| !walled[e]) {
        let (a, b) = edge(e);
        let el = dist(a, b);
        if el < room {
            continue;
        }
        let k = crate::core::floor((el - room) / 5.0) as usize;
        for j in 0..=k {
            let t = (GATE_HALF + 30.0 + 5.0 * j as f64) / el;
            spots.push((e, t, lerp(a, b, t)));
        }
    }
    // (Its towers clear of the towers already standing, a town wall's among them: not one beside
    // another.)
    let clear = |s: &(usize, f64, P)| {
        let (a, b) = edge(s.0);
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        [-GATE_HALF, GATE_HALF].iter().all(|&o| {
            others
                .iter()
                .all(|(q, r)| dist(add(s.2, mul(u, o)), *q) >= GATE_CLEAR.max(r + 15.0 + if *r >= 15.0 { TOWERS_CLEAR } else { gap }))
        })
    };
    let best = |spots: &[(usize, f64, P)], need: bool, apart: bool| {
        spots
            .iter()
            .filter(|s| (!need || ok(s.2)) && (!apart || clear(s)))
            .min_by(|x, y| score(x.2).total_cmp(&score(y.2)).then(x.0.cmp(&y.0)).then(x.1.total_cmp(&y.1)))
            .copied()
    };
    // (Where none of those will do, the middle of a side too short for corner towers clear of it, as
    // a small castle's: theirs go.)
    let short: Vec<(usize, f64, P)> = (0..n)
        .filter(|&e| !walled[e] && (2.0 * GATE_HALF + 20.0..room).contains(&dist(edge(e).0, edge(e).1)))
        .map(|e| (e, 0.5, lerp(edge(e).0, edge(e).1, 0.5)))
        .collect();
    // (A gate asked for: on the side nearest it, as near it as the side's towers leave room.)
    let snap = |p: P| -> Option<(usize, f64)> {
        (0..n)
            .filter(|&e| !walled[e] && dist(edge(e).0, edge(e).1) >= 2.0 * GATE_HALF + 4.0)
            .map(|e| {
                let (a, b) = edge(e);
                let el = dist(a, b);
                let t = (dot(sub(p, a), sub(b, a)) / (el * el)).clamp(0.0, 1.0);
                let m = ((GATE_HALF + 30.0).min(0.5 * el)) / el;
                let t = if m >= 0.5 { 0.5 } else { t.clamp(m, 1.0 - m) };
                (e, t, dist(p, lerp(a, b, t)))
            })
            .min_by(|x, y| x.2.total_cmp(&y.2).then(x.0.cmp(&y.0)))
            .map(|s| (s.0, s.1))
    };
    // (Vetted as a rolled one: open ground before it (`ok`), its towers clear of those standing,
    // `HAND_GATES_APART` from the gates asked before it; where its spot will not do, the nearest on
    // its side that will; else it is not built, and why: `dropped`.)
    let asked: Vec<(usize, f64)> = {
        let mut out: Vec<(usize, f64)> = Vec::new();
        for (i, p) in want.iter().enumerate() {
            let Some((e, t)) = snap(*p) else {
                dropped.push((i, "no side there is long enough for a gate".into()));
                continue;
            };
            let at = lerp(edge(e).0, edge(e).1, t);
            let apart = |q: P| out.iter().all(|&(f, u)| dist(lerp(edge(f).0, edge(f).1, u), q) >= HAND_GATES_APART);
            let mut on_side: Vec<(usize, f64, P)> = spots.iter().chain(&short).filter(|s| s.0 == e).copied().collect();
            on_side.sort_by(|x, y| dist(x.2, at).total_cmp(&dist(y.2, at)).then(x.1.total_cmp(&y.1)));
            let tries: Vec<(usize, f64, P)> = std::iter::once((e, t, at)).chain(on_side).collect();
            match tries.iter().find(|s| apart(s.2) && ok(s.2) && clear(s)) {
                Some(s) => out.push((s.0, s.1)),
                None => {
                    let why = if !tries.iter().any(|s| apart(s.2)) {
                        "too near another gate (they stand 76 ft apart at least)"
                    } else if !tries.iter().any(|s| apart(s.2) && ok(s.2)) {
                        "no open ground before it (water, a building or a town gate), or no way from it to the streets"
                    } else {
                        "its towers would stand against the towers there"
                    };
                    dropped.push((i, why.into()));
                }
            }
        }
        out
    };
    let (gate_e, t) = match asked.first().map(|&(e, t)| (e, t, [0.0, 0.0])).or_else(|| best(&spots, true, true))
        .or_else(|| best(&spots, true, false))
        .or_else(|| best(&short, true, true))
        .or_else(|| best(&short, true, false))
        .or_else(|| best(&spots, false, false))
    {
        Some(s) => (s.0, s.1),
        None => {
            let e = (0..n)
                .filter(|&e| !walled[e])
                .max_by(|&x, &y| dist(edge(x).0, edge(x).1).total_cmp(&dist(edge(y).0, edge(y).1)).then(y.cmp(&x)))
                .unwrap_or(0);
            (e, 0.5)
        }
    };
    let (a, b) = edge(gate_e);
    let gate = lerp(a, b, t);
    let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
    // (The ring runs anticlockwise: in is to the left.)
    let inward = [-u[1], u[0]];
    // (With the town wall taken down, its sides get a curtain too; the gate stays where it was.)
    let walled: Vec<bool> = if up { walled } else { vec![false; n] };
    // The lines the curtain runs along: all round through the gate, or from the town wall round to
    // it again along each run of sides off it.
    let mut lines: Vec<(Vec<(P, bool)>, bool)> = Vec::new();
    // (The gates past the main one, by side and along it.)
    let more: Vec<(usize, f64)> = asked.iter().skip(1).copied().collect();
    let on_side = |e: usize, lo: f64, hi: f64| {
        let mut v: Vec<f64> = more.iter().filter(|g| g.0 == e && g.1 > lo && g.1 < hi).map(|g| g.1).collect();
        v.sort_by(f64::total_cmp);
        v
    };
    for &(e, u) in &more {
        let (a, b) = edge(e);
        let d = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        extra.push((lerp(a, b, u), [-d[1], d[0]]));
    }
    if walled.iter().all(|&w| !w) {
        let mut line: Vec<(P, bool)> = vec![(gate, true)];
        for k in 1..=n {
            let e = (gate_e + k - 1) % n;
            for u in on_side(e, if k == 1 { t } else { -1.0 }, 2.0) {
                line.push((lerp(edge(e).0, edge(e).1, u), true));
            }
            line.push((ring[(gate_e + k) % n], false));
        }
        for u in on_side(gate_e, -1.0, t) {
            line.push((lerp(edge(gate_e).0, edge(gate_e).1, u), true));
        }
        line.push((gate, true));
        lines.push((line, false));
    } else {
        let start = (0..n).find(|&e| walled[e] && !walled[(e + 1) % n]).unwrap_or(0);
        let mut e = (start + 1) % n;
        let mut line: Vec<(P, bool)> = Vec::new();
        for _ in 0..n {
            if walled[e] {
                if !line.is_empty() {
                    line.push((ring[e], false));
                    lines.push((std::mem::take(&mut line), true));
                }
            } else {
                line.push((ring[e], false));
                if e == gate_e {
                    line.push((gate, true));
                }
            }
            e = (e + 1) % n;
        }
        if !line.is_empty() {
            line.push((ring[e], false));
            lines.push((line, true));
        }
    }
    let mut pieces = Vec::new();
    let mut towers = Vec::new();
    let mut gate_towers = Vec::new();
    // (A town's castle (`stubs`): not a stub under 30 ft from the town wall ending in the open, as by a
    // water gap in it.)
    let standing: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).collect();
    let open_end = |p: P| !standing.iter().any(|&(a, b)| seg_dist(p, a, b) < 12.0);
    for (line, ends) in lines {
        let span: f64 = line.windows(2).map(|s| dist(s[0].0, s[1].0)).sum();
        if stubs && ends && span < 30.0 && !line.iter().any(|q| q.1) && line.len() >= 2 && (open_end(line[0].0) || open_end(line[line.len() - 1].0)) {
            continue;
        }
        let (mut p, t, g) = wall_pieces(&line, false, style.spacing);
        // (A line from the town wall: its ends carried on to that wall's line, its towers where
        // they stand on the ring, as with no town wall there.)
        if ends && line.len() >= 2 {
            let m = line.len();
            if let Some(first) = p.first_mut().filter(|w| w.len() >= 2 && dist(w[0], line[0].0) < 1e-9) {
                first[0] = to_wall(l, line[0].0, line[1].0);
            }
            if let Some(last) = p.last_mut().filter(|w| w.len() >= 2 && dist(w[w.len() - 1], line[m - 1].0) < 1e-9) {
                let k = last.len() - 1;
                last[k] = to_wall(l, line[m - 1].0, line[m - 2].0);
            }
        }
        pieces.extend(p);
        towers.extend(t);
        gate_towers.extend(g);
    }
    // Towers clear of the gatehouse, of each other and of every tower already standing (a square
    // one reaches further across its corners).
    // (Each its shape by the style: at a corner or a line's end its corner shape; a D facing out.)
    let r = 11.0;
    let middle = centroid(ring);
    let mut kept: Vec<(P, f64, TowerShape, P)> = Vec::new();
    // (In an order of their own, not the line's: the same towers whether a town wall's side has a
    // curtain or not.)
    towers.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    // (A hand castle's ring turning inward: a D faces out of the ring, not away from its middle.)
    let bent = !stubs && !super::super::clip::is_convex(ring);
    for t in towers {
        let (shape, mut dir) = super::super::wallstyle::form_of(style, &pieces, t, middle);
        if bent && shape == TowerShape::D && contains(ring, add(t, mul(dir, 3.0))) {
            dir = mul(dir, -1.0);
        }
        let reach = match shape {
            TowerShape::Square => 15.6,
            TowerShape::Octagon => 11.9,
            _ => r,
        };
        let clear = gate_towers.iter().all(|g| dist(t, *g) >= reach + 15.0 + 4.0)
            && kept.iter().all(|k| dist(t, k.0) >= reach + k.1 + 2.0)
            && others.iter().all(|(o, ro)| dist(t, *o) >= reach + ro + if *ro >= 15.0 { TOWERS_CLEAR } else { gap });
        if clear {
            kept.push((t, reach, shape, dir));
        }
    }
    for &(t, _, shape, dir) in &kept {
        l.tower_forms.push((t, shape as u8 | 16, dir));
        if shape == TowerShape::Square {
            l.square_towers.push((t, dir));
        }
    }
    others.extend(kept.iter().map(|t| (t.0, t.1)));
    others.extend(gate_towers.iter().map(|t| (*t, 15.0)));
    l.walls.extend(pieces);
    l.towers.extend(kept.iter().map(|t| t.0));
    l.gate_towers.extend(gate_towers);
    (gate, inward)
}

/// A way from a castle's gate (`gate`, `inward` the way in) to the nearest of `streets` (outside its
/// `ring`): straight where that is clear of the towers, walls (the town's as laid, `laid`, standing
/// or not) and buildings, else out from the gate first, else round what stands in the way
/// (`way_round`).
#[allow(clippy::too_many_arguments)]
pub(super) fn gate_way<G: Ground>(site: &G, l: &Layout, ring: &[P], block: &[P], streets: &[(P, P)], laid: (&[(P, P)], &[(P, f64)]), gate: P, inward: P, out: f64) -> Option<Vec<P>> {
    let from = sub(gate, mul(inward, 6.0));
    let nearest = |from: P| {
        let mut near: Vec<P> = streets
            .iter()
            .map(|&(a, b)| {
                let e = sub(b, a);
                let t = (dot(sub(from, a), e) / dot(e, e).max(1e-9)).clamp(0.0, 1.0);
                add(a, mul(e, t))
            })
            .collect();
        near.sort_by(|x, y| dist(*x, from).total_cmp(&dist(*y, from)).then(x[0].total_cmp(&y[0])).then(x[1].total_cmp(&y[1])));
        near.truncate(40);
        near
    };
    let towers: Vec<(P, f64)> = l
        .towers
        .iter()
        .map(|t| (*t, 11.0 * if l.square_tower(*t).is_some() { std::f64::consts::SQRT_2 } else { 1.0 }))
        .chain(l.gate_towers.iter().map(|t| (*t, 15.0)))
        .chain(l.extra_towers.iter().map(|t| (t.0, if t.1 { 15.0 } else { 11.0 })))
        .chain(laid.1.iter().copied())
        .collect();
    // (Clear of the buildings standing too.)
    let reach = ring.iter().map(|p| dist(*p, from)).fold(0.0, f64::max) + 260.0;
    let near: Vec<&Building> = l.buildings.iter().filter(|b| b.poly.iter().any(|p| dist(*p, from) < reach)).collect();
    let clear = |p: P, q: P| {
        towers.iter().all(|&(t, r)| seg_dist(t, p, q) >= r + 6.0)
            && l.walls
                .iter()
                .flat_map(|w| w.windows(2))
                .all(|s| !crosses(p, q, s[0], s[1]) && seg_dist(q, s[0], s[1]) >= 6.0 && seg_dist(p, s[0], s[1]) >= 5.0)
            && near.iter().all(|b| super::super::poly_seg_dist(&b.poly, p, q) >= 6.0)
            && laid.0.iter().all(|&(a, b)| !crosses(p, q, a, b) && seg_dist(q, a, b) >= 6.0 && seg_dist(p, a, b) >= 5.0)
    };
    // (Over a moat, `out` ft from the gate: straight out across it first, v63 castles.)
    let over = (out > 0.0).then(|| sub(from, mul(inward, out))).filter(|&mid| clear(from, mid)).and_then(|mid| {
        nearest(mid).into_iter().find(|q| dist(*q, mid) <= 160.0 && clear(mid, *q)).map(|q| if dist(q, mid) < 1.0 { vec![from, q] } else { vec![from, mid, q] })
    });
    let way = over
        .or_else(|| nearest(from).into_iter().find(|q| dist(*q, from) <= 160.0 && clear(from, *q)).map(|q| vec![from, q]))
        .or_else(|| {
            [15.0, 25.0, 35.0, 50.0].iter().find_map(|&out| {
                let mid = sub(from, mul(inward, out));
                (inset_depth(ring, mid) < -5.0 && clear(from, mid))
                    .then(|| nearest(mid).into_iter().find(|q| dist(*q, mid) <= 120.0 && clear(mid, *q)).map(|q| vec![from, mid, q]))
                    .flatten()
            })
        })
        // (Else round what stands in the way: `way_round`.)
        .or_else(|| {
            let walls: Vec<Vec<P>> = l.walls.iter().cloned().chain(laid.0.iter().map(|&(a, b)| vec![a, b])).collect();
            way_round(site, ring, block, from, streets, &towers, &walls, &near, &clear)
        });
    way
}

/// A way from a gate (`from`, just outside it) to the nearest of `streets` round what stands in the
/// way: on a 5-ft grid (whole multiples of local ft), off the castle's ring, the water, and 6 ft
/// off walls, towers (`towers`: centre and reach) and buildings, keeping to the castle's own ground
/// round its ring (`block` and the streets round it) where it can; straightened where `clear`
/// allows. None if no street is in reach.
#[allow(clippy::too_many_arguments)]
fn way_round<G: Ground>(
    site: &G,
    ring: &[P],
    block: &[P],
    from: P,
    streets: &[(P, P)],
    towers: &[(P, f64)],
    walls: &[Vec<P>],
    near: &[&Building],
    clear: &dyn Fn(P, P) -> bool,
) -> Option<Vec<P>> {
    const S: f64 = 5.0;
    const REACH: f64 = 240.0;
    let (x0, y0, x1, y1) = ring.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| {
        (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1]))
    });
    let (i0, j0) = (crate::core::floor((x0 - REACH) / S) as i64, crate::core::floor((y0 - REACH) / S) as i64);
    let (nx, ny) = (
        (crate::core::ceil((x1 + REACH) / S) as i64 - i0 + 1) as usize,
        (crate::core::ceil((y1 + REACH) / S) as i64 - j0 + 1) as usize,
    );
    let centre = |k: usize| [((i0 + (k % nx) as i64) as f64 + 0.5) * S, ((j0 + (k / nx) as i64) as f64 + 0.5) * S];
    let cell = |p: P| {
        let (i, j) = (crate::core::floor(p[0] / S) as i64 - i0, crate::core::floor(p[1] / S) as i64 - j0);
        (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    // (Over the cells round a box: `f` for each.)
    let each = |lo: P, hi: P, f: &mut dyn FnMut(usize)| {
        let (a, b) = (crate::core::floor(lo[0] / S) as i64 - i0, crate::core::floor(lo[1] / S) as i64 - j0);
        let (c, d) = (crate::core::floor(hi[0] / S) as i64 - i0, crate::core::floor(hi[1] / S) as i64 - j0);
        for j in b.max(0)..=d.min(ny as i64 - 1) {
            for i in a.max(0)..=c.min(nx as i64 - 1) {
                f(j as usize * nx + i as usize);
            }
        }
    };
    // 1: too near something (6 ft); 2: in it; 4: a street; 8: too near a wall or a tower.
    let mut grid = vec![0u8; nx * ny];
    for (k, g) in grid.iter_mut().enumerate() {
        let p = centre(k);
        if inset_depth(ring, p) > -4.0 || site.wet(p) {
            *g = 3;
        }
    }
    for w in walls {
        for s in w.windows(2) {
            let (lo, hi) = (
                [s[0][0].min(s[1][0]) - 6.0, s[0][1].min(s[1][1]) - 6.0],
                [s[0][0].max(s[1][0]) + 6.0, s[0][1].max(s[1][1]) + 6.0],
            );
            each(lo, hi, &mut |k| {
                let d = seg_dist(centre(k), s[0], s[1]);
                grid[k] |= if d < 3.0 {
                    3
                } else if d < 6.0 {
                    9
                } else {
                    0
                };
            });
        }
    }
    for &(t, r) in towers {
        each([t[0] - r - 6.0, t[1] - r - 6.0], [t[0] + r + 6.0, t[1] + r + 6.0], &mut |k| {
            let d = dist(centre(k), t);
            grid[k] |= if d < r {
                3
            } else if d < r + 6.0 {
                9
            } else {
                0
            };
        });
    }
    for b in near {
        let (lo, hi) = b.poly.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |a, p| {
            ([a.0[0].min(p[0]), a.0[1].min(p[1])], [a.1[0].max(p[0]), a.1[1].max(p[1])])
        });
        each([lo[0] - 6.0, lo[1] - 6.0], [hi[0] + 6.0, hi[1] + 6.0], &mut |k| {
            let p = centre(k);
            let d = super::super::poly_seg_dist(&b.poly, p, p);
            grid[k] |= if d <= 0.0 {
                3
            } else if d < 6.0 {
                1
            } else {
                0
            };
        });
    }
    for &(a, b) in streets {
        let (lo, hi) = ([a[0].min(b[0]) - 4.0, a[1].min(b[1]) - 4.0], [a[0].max(b[0]) + 4.0, a[1].max(b[1]) + 4.0]);
        each(lo, hi, &mut |k| {
            if seg_dist(centre(k), a, b) <= 4.0 {
                grid[k] |= 4;
            }
        });
    }
    let start = cell(from)?;
    // Least cost out (10 a step, 14 across; three times that off the castle's own ground).
    let own: Vec<bool> = (0..nx * ny).map(|k| inset_depth(block, centre(k)) > -35.0).collect();
    let mut cost = vec![u32::MAX; nx * ny];
    let mut back = vec![usize::MAX; nx * ny];
    let mut heap = std::collections::BinaryHeap::new();
    cost[start] = 0;
    heap.push(std::cmp::Reverse((0u32, start)));
    let mut end = None;
    while let Some(std::cmp::Reverse((c, k))) = heap.pop() {
        if c > cost[k] {
            continue;
        }
        if k != start && grid[k] & 4 != 0 {
            end = Some(k);
            break;
        }
        let (i, j) = ((k % nx) as i64, (k / nx) as i64);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (a, b) = (i + di, j + dj);
            if a < 0 || b < 0 || a >= nx as i64 || b >= ny as i64 {
                continue;
            }
            let n = b as usize * nx + a as usize;
            // (Onto a street, anything not in a building nor near a wall or a tower.)
            let open = if grid[n] & 4 != 0 { grid[n] & 10 == 0 } else { grid[n] & 3 == 0 };
            if !open {
                continue;
            }
            let step = if di != 0 && dj != 0 { 14 } else { 10 } * if own[n] { 1 } else { 3 };
            let nc = c + step;
            if nc < cost[n] && nc <= 6_000 {
                cost[n] = nc;
                back[n] = k;
                heap.push(std::cmp::Reverse((nc, n)));
            }
        }
    }
    let end = end?;
    let mut path = vec![centre(end)];
    let mut k = end;
    while back[k] != start && back[k] != usize::MAX {
        k = back[k];
        path.push(centre(k));
    }
    path.push(from);
    path.reverse();
    // Straightened: from each point on to the farthest it reaches clear.
    let mut way = vec![from];
    let mut i = 0;
    while i + 1 < path.len() {
        let j = (i + 2..path.len()).rev().find(|&j| clear(path[i], path[j])).unwrap_or(i + 1);
        way.push(path[j]);
        i = j;
    }
    (way.len() >= 2).then_some(way)
}

/// Where a curtain from `p` (coming from `from`) meets the town wall nearest it, carried on along
/// its line (within 20 ft); else `p`.
fn to_wall(l: &Layout, p: P, from: P) -> P {
    let d = sub(p, from);
    let dl = len(d);
    if dl < 1e-9 {
        return p;
    }
    let d = mul(d, 1.0 / dl);
    let mut best: Option<(f64, P)> = None;
    for w in &l.walls {
        for s in w.windows(2) {
            let e = sub(s[1], s[0]);
            let den = cross(d, e);
            if den.abs() < 1e-9 {
                continue;
            }
            let ao = sub(s[0], p);
            let t = cross(ao, e) / den;
            let u = cross(ao, d) / den;
            if (0.0..=20.0).contains(&t) && (-0.05..=1.05).contains(&u) && best.is_none_or(|b| t < b.0) {
                best = Some((t, add(p, mul(d, t))));
            }
        }
    }
    best.map_or(p, |b| b.1)
}
