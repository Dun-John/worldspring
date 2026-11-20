//! The main streets and the roads in to the gates (`build_town`'s phase after the walls), with
//! what a gate taken out by hand cuts off them.

use super::*;

/// Lays the main streets (`plan.paths`), each road's approach from where it ends to its gate
/// (`approach_curve`; straight over a bridge where it crosses the river, a town street once it
/// is in the town) and a hand gate's way out. Returns the stretches a gate taken out cut off.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(
    site: &Site,
    s: &Settlement,
    r: f64,
    plan: &TownPlan,
    ch: &wards::Changes,
    mesh: &Mesh,
    walled: bool,
    wall_pieces: &[Vec<P>],
    gate_ways: Vec<(usize, Vec<P>)>,
    pieces: &mut stairs::Pieces,
    l: &mut Layout,
) -> Vec<(P, P, f64)> {
    // (R5 towns, M18: the stretches of road and street a gate taken out cut off, kept clear as
    // they were with the gate, and their half widths and 3 ft, as `keep_clear` keeps roads.)
    let mut freed: Vec<(P, P, f64)> = Vec::new();
    for (pi, path) in plan.paths.iter().enumerate() {
        // (189: a Linear town's high street wider, a street market's.)
        let width = if plan.wide.get(pi) == Some(&true) { super::HIGH_STREET_FT } else { 18.0 };
        l.streets.push(path.iter().map(|&v| mesh.pos[v]).collect());
        let mut drawn: Vec<P> = Vec::new();
        for (k, &v) in path.iter().enumerate() {
            if k > 0
                && plan.crossing_links.iter().any(|&(a, _, b)| (a == path[k - 1] && b == v) || (a == v && b == path[k - 1]))
            {
                drawn.push(river_crossing(site, mesh.pos[path[k - 1]], mesh.pos[v]));
            } else if k > 0 {
                pieces.edge(path[k - 1], v, mesh.pos[path[k - 1]], mesh.pos[v], width, true, l.roads.len());
            }
            drawn.push(mesh.pos[v]);
        }
        pieces.main_street(l.roads.len());
        let drawn = if walled {
            let (kept, cut) = wallgates::closed_street(&ch.wall, path, drawn);
            freed.extend(cut.iter().flat_map(|c| c.windows(2).map(|s| (s[0], s[1], 0.5 * width + 3.0))));
            kept
        } else {
            drawn
        };
        l.roads.push((drawn, 4, width));
    }
    // (A road to a gate taken out ends where it can join another: the streets and network roads.)
    let others: Vec<(P, P, f64)> = if walled && !ch.wall.remove.is_empty() {
        let mut o: Vec<(P, P, f64)> = l.roads.iter().filter(|r| r.1 != 3).flat_map(|(pts, _, w)| pts.windows(2).map(move |s| (s[0], s[1], 0.5 * w))).collect();
        let within = OUTSKIRTS * r * 1.1;
        let c = site.center;
        for (ri, k) in site.t0.roads.segments_near([c[0] - within, c[1] - within, c[0] + within, c[1] + within], 0.0) {
            let rc = &site.t0.roads.roads[ri as usize];
            let k = k as usize;
            let m = ((rc.s[k + 1] - rc.s[k]) / 25.0).ceil().clamp(1.0, 400.0) as usize;
            let pts: Vec<P> = (0..=m).map(|j| sub(rc.eval(k, j as f64 / m as f64, 5.0, site.t0.cell_ft).p, c)).collect();
            o.extend(pts.windows(2).filter(|s| len(s[0]) < within).map(|s| (s[0], s[1], 0.5 * rc.class.width_ft())));
        }
        o
    } else {
        Vec::new()
    };
    // Each road continues from where it ends to its gate (`approach_curve`).
    for (k, (e, class)) in plan.ends.iter().enumerate() {
        if let Some(Some(g)) = plan.gate_of.get(k) {
            let gp = mesh.pos[*g];
            let span = dist(*e, gp);
            if span > 1.0 {
                let curve = approach_curve(*e, gp, plan.heading_of[k]);
                let width = [24.0, 16.0, 10.0][(*class as usize).min(2)];
                let curve = if walled {
                    let (kept, cut) = wallgates::closed_road(&ch.wall, *g, curve, wall_pieces, &others);
                    freed.extend(cut.windows(2).map(|s| (s[0], s[1], 0.5 * width + 3.0)));
                    kept
                } else {
                    curve
                };
                if curve.len() < 2 || curve.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>() < 1.0 {
                    continue;
                }
                // Where the approach crosses the river it goes straight over a bridge: the
                // wet stretch of the curve becomes the deck's line, bank to bank.
                let total: f64 = curve.windows(2).map(|w| dist(w[0], w[1])).sum();
                let steps = (total / 5.0).ceil().max(1.0) as usize;
                let samples: Vec<P> = (0..=steps).map(|j| point_along(&curve, total * j as f64 / steps as f64).0).collect();
                let mut straight: Vec<P> = Vec::new();
                // (The last deck laid on this curve: its index in `l.bridges`, where its span starts and that
                // point's index in `straight`.)
                let mut last: Option<(usize, P, usize)> = None;
                let mut j = 0;
                while j < samples.len() {
                    if !site.near_river(samples[j], 0.0) {
                        // One point in eight (40 ft) where it runs on land, and its ends.
                        if j % 8 == 0 || j + 1 == samples.len() || site.near_river(samples[(j + 1).min(samples.len() - 1)], 0.0) {
                            straight.push(samples[j]);
                        }
                        j += 1;
                        continue;
                    }
                    let start = j;
                    while j + 1 < samples.len() && site.near_river(samples[j + 1], 0.0) {
                        j += 1;
                    }
                    let (a, b) = (samples[start.saturating_sub(1)], samples[(j + 1).min(samples.len() - 1)]);
                    if straight.last().is_none_or(|p| dist(*p, a) > 1e-9) {
                        straight.push(a);
                    }
                    straight.push(b);
                    let deck = if s.tier >= Tier::City { f64::max(width + 4.0, 24.0) } else { width + 4.0 };
                    // From bank to bank at every corner, plus a landing each side.
                    let lay = |a: P, b: P| {
                        let span = dist(a, b);
                        let u = mul(sub(b, a), 1.0 / span);
                        let m = lerp(a, b, 0.5);
                        let (fwd, back) = (site.dry_reach(m, u, deck, 1.0, 0.5 * span) + 8.0, site.dry_reach(m, u, deck, -1.0, 0.5 * span) + 8.0);
                        rect(add(m, mul(u, 0.5 * (fwd - back))), u, fwd + back, deck)
                    };
                    if dist(a, b) > 1.0 {
                        let d = lay(a, b);
                        // A second wet stretch so close that its deck would overlap the last one's (the curve
                        // dry for a step between two channels): one deck goes straight over both.
                        let overlaps = |p: &[P], q: &[P]| poly_gap(p, q) == 0.0 || p.iter().any(|v| geom::contains(q, *v)) || q.iter().any(|v| geom::contains(p, *v));
                        match last {
                            Some((k, a0, i0)) if overlaps(&d, &l.bridges[k]) && dist(a0, b) > 1.0 => {
                                l.bridges[k] = lay(a0, b);
                                straight.truncate(i0 + 1);
                                straight.push(b);
                            }
                            _ => {
                                l.bridges.push(d);
                                last = Some((l.bridges.len() - 1, a, straight.len() - 2));
                            }
                        }
                    }
                    j += 2;
                }
                straight.dedup_by(|a, b| dist(*a, *b) < 1e-9);
                let curve = if straight.len() >= 2 { straight } else { curve };
                let total: f64 = curve.windows(2).map(|w| dist(w[0], w[1])).sum();
                let samples: Vec<P> = (0..=steps).map(|j| point_along(&curve, total * j as f64 / steps as f64).0).collect();
                // Once it is in the town (on land, short of the gate itself) the road is a
                // town street, paved like the main streets.
                match samples.iter().position(|q| dist(*q, gp) >= 40.0 && !site.wet(*q) && plan.in_town(*q)) {
                    Some(entry) if entry > 0 => {
                        let at = total * entry as f64 / steps as f64;
                        let mut acc = 0.0;
                        let (mut outside, mut inside) = (vec![curve[0]], vec![samples[entry]]);
                        for w in curve.windows(2) {
                            acc += dist(w[0], w[1]);
                            if acc < at { outside.push(w[1]) } else { inside.push(w[1]) }
                        }
                        outside.push(samples[entry]);
                        l.roads.push((outside, *class, width));
                        pieces.approach(l.roads.len(), *g);
                        l.roads.push((inside, 4, width));
                    }
                    _ => l.roads.push((curve, *class, width)),
                }
            }
        }
    }

    // R5 towns (M18b): a gate put in by hand, its way out to a road or the open country.
    if walled {
        for (_, pts) in gate_ways {
            l.roads.push((pts, 2, wallgates::WAY_WIDTH_FT));
        }
    }
    freed
}
