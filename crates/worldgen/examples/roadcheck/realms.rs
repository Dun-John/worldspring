use crate::prelude::*;

/// The road network as a traveller and a surveyor see it: trips between neighbouring settlements
/// against the straight line, road length against a minimum spanning tree, junctions outside
/// settlements (arms, shape, stubs, crossings of two roads), river crossings, roads running beside
/// roads, gates per town and up, settlements by tier.
/// Isolated realms: kind, tier, centre, its tracks, villages, the nearest road not its own, walls and gates.
pub(crate) fn realms(world: &worldgen::World, t0: &T0) {
    use worldgen::t0::settle::Tier;
    let sts = &t0.settlements;
    let d = |a: P, b: P| ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])).sqrt();
    let realms: Vec<usize> = (0..sts.len()).filter(|&i| sts[i].isolated && sts[i].tier >= Tier::Town).collect();
    say!("  realms: {}", realms.len());
    for &i in &realms {
        let s = &sts[i];
        let c = [s.x, s.y];
        let villages: Vec<usize> = (0..sts.len()).filter(|&v| sts[v].isolated && sts[v].tier == Tier::Village && d([sts[v].x, sts[v].y], c) < 12.0 * MI).collect();
        let trim = worldgen::town::road_trim_radius(s.tier, s.population) + 0.3 * MI;
        let at = |p: P| d(p, c) < trim || villages.iter().any(|&v| d(p, [sts[v].x, sts[v].y]) < 0.3 * MI);
        let rs = &t0.roads.roads;
        // Its own roads: those mostly along its routes as found (its track and its villages').
        let mine: Vec<P> = t0.extra.as_ref().map(|e| e.realm_paths.iter().filter(|p| p.0 == i).flat_map(|p| p.1.iter().copied()).collect()).unwrap_or_default();
        let own: Vec<bool> = rs.iter().map(|r| at(r.pts[0]) || at(r.pts[r.pts.len() - 1]) || 2 * r.pts.iter().filter(|&&p| mine.iter().any(|&q| d(p, q) < 1.5 * t0.cell_ft)).count() >= r.pts.len()).collect();
        let tracks: Vec<usize> = (0..rs.len()).filter(|&r| rs[r].pts.len() >= 2 && (d(rs[r].pts[0], c) < trim || d(rs[r].pts[rs[r].pts.len() - 1], c) < trim)).collect();
        let ends: Vec<String> = tracks
            .iter()
            .map(|&r| {
                let p = &rs[r].pts;
                let far = if d(p[0], c) < trim { p[p.len() - 1] } else { p[0] };
                format!("{:?} to ({:.0}, {:.0}) {:.1} mi", rs[r].class, far[0], far[1], d(far, c) / MI)
            })
            .collect();
        let (other, by) = (0..rs.len()).filter(|&r| !own[r]).flat_map(|r| rs[r].pts.iter().map(move |&p| (d(p, c), r))).fold((f64::INFINITY, 0), |a, b| if b.0 < a.0 { b } else { a });
        let by = rs.get(by).map(|r| format!("{:?} ({:.0}, {:.0})-({:.0}, {:.0})", r.class, r.pts[0][0], r.pts[0][1], r.pts[r.pts.len() - 1][0], r.pts[r.pts.len() - 1][1])).unwrap_or_default();
        let l = worldgen::town::layout(world, t0, i);
        let v: Vec<String> = villages.iter().map(|&v| format!("({:.0}, {:.0}) {:.1} mi", sts[v].x, sts[v].y, d([sts[v].x, sts[v].y], c) / MI)).collect();
        // The gap round it: from the town or any of its villages to the nearest town and up not
        // its own, and to the nearest king's road; and the roads not its own within 2 miles of
        // the walls.
        let parts: Vec<P> = std::iter::once(c).chain(villages.iter().map(|&v| [sts[v].x, sts[v].y])).collect();
        let gap = |p: P| parts.iter().map(|&q| d(p, q)).fold(f64::INFINITY, f64::min);
        let town_gap = sts.iter().filter(|o| !o.isolated && o.tier >= Tier::Town).map(|o| gap([o.x, o.y])).fold(f64::INFINITY, f64::min);
        let any_gap = sts.iter().filter(|o| !o.isolated).map(|o| gap([o.x, o.y])).fold(f64::INFINITY, f64::min);
        let king_gap = rs.iter().filter(|r| r.class == worldgen::t0::roads::RoadClass::KingsRoad).flat_map(|r| r.pts.iter().map(|&p| gap(p))).fold(f64::INFINITY, f64::min);
        let close = (0..rs.len()).filter(|&r| !own[r] && rs[r].pts.iter().any(|&p| d(p, c) < trim + 2.0 * MI)).count();
        let b = t0.sample_biome(c[0], c[1], t0.biome_warp(c[0], c[1]));
        // Its own roads in standing water, and along a river (within 600 ft of its line).
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level - 1);
        let own_pts: Vec<P> = (0..rs.len()).filter(|&r| own[r]).flat_map(|r| rs[r].pts.iter().copied()).collect();
        let wet = own_pts.iter().filter(|p| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice)).count();
        // (`WET=1`: where.)
        if std::env::var("WET").is_ok() {
            for p in own_pts.iter().filter(|p| t0.sample_water(p[0], p[1]) as f64 > t0.ground_at(p[0], p[1], lattice)) {
                say!("        wet ({:.0}, {:.0}) water {:.0} ground {:.0}", p[0], p[1], t0.sample_water(p[0], p[1]), t0.ground_at(p[0], p[1], lattice));
            }
        }
        let river_pts: Vec<P> = t0.rivers.rivers.iter().flat_map(|r| r.pts.iter().copied()).filter(|&p| d(p, c) < 20.0 * MI).collect();
        let along = own_pts.iter().filter(|&&p| river_pts.iter().any(|&q| d(p, q) < 600.0)).count();
        // Each village's bearing off the track's way out (its first route, 5 miles from the town).
        let out_at = t0.extra.as_ref().and_then(|e| e.realm_paths.iter().find(|p| p.0 == i)).and_then(|p| p.1.iter().find(|&&q| d(q, c) >= 5.0 * MI).copied());
        let bearings: Vec<String> = villages
            .iter()
            .map(|&v| match out_at {
                Some(o) => {
                    let (a, b) = ([o[0] - c[0], o[1] - c[1]], [sts[v].x - c[0], sts[v].y - c[1]]);
                    format!("{:.0}", ((a[0] * b[0] + a[1] * b[1]) / (d(o, c) * d([sts[v].x, sts[v].y], c))).clamp(-1.0, 1.0).acos().to_degrees())
                }
                None => "-".into(),
            })
            .collect();
        say!("      own road points {}: in water {wet}, along a river {along}; villages off the way out (deg) [{}]", own_pts.len(), bearings.join(" "));
        say!(
            "    realm #{i} {:?} {} at ({:.0}, {:.0}), {:.0} ft up, biome {}: gap to town+ {:.1} mi, any {:.1} mi, king's road {:.1} mi, other roads within 2 mi {close}",
            s.tier,
            s.kind.name(),
            c[0],
            c[1],
            t0.sample(c[0], c[1], 0.0),
            b[0],
            town_gap / MI,
            any_gap / MI,
            king_gap / MI
        );
        say!(
            "      pop {}: {} track(s) [{}]; nearest other road {:.1} mi ({by}); walls {} gates {}; villages {} [{}]",
            s.population,
            tracks.len(),
            ends.join("; "),
            other / MI,
            l.walls.len(),
            l.gates.len(),
            villages.len(),
            v.join(", ")
        );
        // Its track (the route from the town) and its villages' spurs: the nearest road not its own
        // to the track but its outer end (20 miles round where it joins), the spurs, and those
        // joining anything but the track (cross-links).
        let rp: Vec<&(usize, Vec<P>)> = t0.extra.as_ref().map(|e| e.realm_paths.iter().filter(|p| p.0 == i).collect()).unwrap_or_default();
        let main = rp.iter().find(|p| p.1.first().is_some_and(|&q| d(q, c) < t0.cell_ft)).map(|p| p.1.clone()).unwrap_or_default();
        let spurs: Vec<&Vec<P>> = rp.iter().filter(|p| p.1.first().is_some_and(|&q| d(q, c) >= t0.cell_ft)).map(|p| &p.1).collect();
        let outer = main.last().copied().unwrap_or(c);
        let far: Vec<P> = main.iter().copied().filter(|&q| d(q, outer) > 20.0 * MI).collect();
        let others: Vec<P> = (0..rs.len()).filter(|&r| !own[r]).flat_map(|r| rs[r].pts.iter().copied()).collect();
        let to_track = far.iter().flat_map(|&q| others.iter().map(move |&o| d(o, q))).fold(f64::INFINITY, f64::min);
        let on = |q: P, path: &[P]| path.iter().any(|&m| d(m, q) < 1.5 * t0.cell_ft);
        let cross = spurs
            .iter()
            .enumerate()
            .filter(|&(si, sp)| {
                let end = sp[sp.len() - 1];
                let body = &sp[..sp.len().saturating_sub(2)];
                !on(end, &main) || spurs.iter().enumerate().any(|(sj, o)| sj != si && body.iter().any(|&q| o.iter().any(|&m| d(m, q) < 0.5 * t0.cell_ft)))
            })
            .count();
        say!(
            "      clearance: town+ {:.1} mi, king's road {:.1} mi, other road to the track (but its end) {:.1} mi; track {:.1} mi to ({:.0}, {:.0}); spurs {}, cross-links {cross}",
            town_gap / MI,
            king_gap / MI,
            to_track / MI,
            d(outer, c) / MI,
            outer[0],
            outer[1],
            spurs.len()
        );
        // Each spur: length, straightness, its join's place along the track, its angle there, and the
        // distance (along the track) to the nearest bend of the track over 45 degrees.
        {
            let mut cum = vec![0.0f64];
            for q in main.windows(2) {
                cum.push(cum[cum.len() - 1] + d(q[0], q[1]));
            }
            let at = |s: f64| -> P {
                let k = cum.partition_point(|&v| v < s).clamp(1, main.len().max(2) - 1);
                let (a, b) = (main[k - 1], main[k]);
                let t = if cum[k] > cum[k - 1] { ((s - cum[k - 1]) / (cum[k] - cum[k - 1])).clamp(0.0, 1.0) } else { 0.0 };
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            };
            let total = cum.last().copied().unwrap_or(0.0);
            let turn = |s: f64| -> f64 {
                let (a, b, c2) = (at((s - MI).max(0.0)), at(s), at((s + MI).min(total)));
                let (h1, h2) = ((b[1] - a[1]).atan2(b[0] - a[0]), (c2[1] - b[1]).atan2(c2[0] - b[0]));
                let mut t = (h2 - h1).abs().to_degrees();
                if t > 180.0 {
                    t = 360.0 - t;
                }
                t
            };
            let mut bends: Vec<f64> = Vec::new();
            let mut s = MI;
            while s < total.min(60.0 * MI) {
                let t = turn(s);
                if t > 45.0 && turn(s - 0.25 * MI) <= t && turn(s + 0.25 * MI) < t {
                    bends.push(s);
                }
                s += 0.25 * MI;
            }
            let mut rows: Vec<(f64, String)> = Vec::new();
            for sp in &spurs {
                let (a, e) = (sp[0], sp[sp.len() - 1]);
                let len: f64 = sp.windows(2).map(|q| d(q[0], q[1])).sum();
                let k = (0..main.len()).min_by(|&x, &y| d(main[x], e).total_cmp(&d(main[y], e))).unwrap_or(0);
                let js = cum[k];
                let nb = bends.iter().map(|&b| (b - js).abs()).fold(f64::INFINITY, f64::min);
                let (p0, p1) = (sp[sp.len().saturating_sub(4)], e);
                let (m0, m1) = (main[k.saturating_sub(3)], main[(k + 3).min(main.len() - 1)]);
                let (h1, h2) = ((p1[1] - p0[1]).atan2(p1[0] - p0[0]), (m1[1] - m0[1]).atan2(m1[0] - m0[0]));
                let mut ang = (h1 - h2).abs().to_degrees() % 180.0;
                if ang > 90.0 {
                    ang = 180.0 - ang;
                }
                rows.push((js, format!("spur from ({:.0}, {:.0}) len {:.1} mi straight-dist {:.1} mi ratio {:.2}; join at {:.1} mi along track, angle {:.0} deg, nearest bend {} mi", a[0], a[1], len / MI, d(a, e) / MI, len / d(a, e).max(1.0), js / MI, ang, if nb.is_finite() { format!("{:.1}", nb / MI) } else { "none".into() })));
            }
            rows.sort_by(|x, y| x.0.total_cmp(&y.0));
            let gaps: Vec<String> = rows.windows(2).map(|r| format!("{:.1}", (r[1].0 - r[0].0) / MI)).collect();
            say!("      cell {:.0} ft", t0.cell_ft);
            say!("      SPURS bends(>45deg, first 60 mi) at {:?} mi; join gaps {:?} mi", bends.iter().map(|b| (b / MI * 10.0).round() / 10.0).collect::<Vec<_>>(), gaps);
            for r in &rows {
                say!("      SPUR {}", r.1);
            }
            // Each spur as built: its road from the village, length against the chord and the furthest it strays from it.
            for sp in &spurs {
                let a = sp[0];
                let built = t0.roads.roads.iter().filter(|r| r.pts.len() >= 2 && (d(r.pts[0], a) < 0.3 * t0.cell_ft || d(r.pts[r.pts.len() - 1], a) < 0.3 * t0.cell_ft)).max_by(|x, y| x.pts.len().cmp(&y.pts.len()));
                if let Some(r) = built {
                    let pts: Vec<P> = if d(r.pts[0], a) < 0.3 * t0.cell_ft { r.pts.clone() } else { r.pts.iter().rev().copied().collect() };
                    let (p0, p1) = (pts[0], pts[pts.len() - 1]);
                    let len: f64 = pts.windows(2).map(|q| d(q[0], q[1])).sum();
                    let ch = d(p0, p1).max(1.0);
                    let dev = pts.iter().map(|q| ((p1[0] - p0[0]) * (p0[1] - q[1]) - (p0[0] - q[0]) * (p1[1] - p0[1])).abs() / ch).fold(0.0f64, f64::max);
                    say!("      SPUR-BUILT from ({:.0}, {:.0}) len {:.1} mi chord {:.1} mi ratio {:.3} max stray {:.2} mi", a[0], a[1], len / MI, ch / MI, len / ch, dev / MI);
                }
            }
        }
    }
}
