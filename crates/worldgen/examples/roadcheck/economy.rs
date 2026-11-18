use crate::prelude::*;

/// Street grades in towns and up (`town::stairs::survey`, the stair rule's own stations), per tier
/// and kind of street: length, length at or above some grades, grade percentiles (by length), and
/// the flights of steps (count, stepped length) with steps from 0.12, 0.15 (the default) and 0.20,
/// stopping at 80% of that. Settlements listed with their tier for `TOP`.
/// The economy pass: tier moves (with the biggest movers and their wealth's parts), the rank-size
/// curve of the towns and up (log population against log rank: Zipf's rule is a slope of -1), and
/// the roads' traffic per class (and the stretches whose class it changed).
pub(crate) fn economy(t0: &T0) -> Value {
    use worldgen::t0::roads::RoadClass;
    use worldgen::t0::settle::Tier;
    let Some(extra) = t0.extra.as_ref() else { return Value::Null };
    let places = &extra.economy.places;
    let sts = &t0.settlements;
    let top = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let (up, down): (Vec<_>, Vec<_>) = places.iter().filter(|p| sts[p.index].tier != p.was).partition(|p| sts[p.index].tier > p.was);
    // Rank-size: least squares of ln(population) on ln(rank).
    let mut pops: Vec<f64> = sts.iter().filter(|s| s.tier >= Tier::Town).map(|s| s.population as f64).collect();
    pops.sort_by(|a, b| b.total_cmp(a));
    let pts: Vec<(f64, f64)> = pops.iter().enumerate().map(|(r, &p)| (((r + 1) as f64).ln(), p.ln())).collect();
    let nn = pts.len().max(1) as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / nn, pts.iter().map(|p| p.1).sum::<f64>() / nn);
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    let syy: f64 = pts.iter().map(|p| (p.1 - my) * (p.1 - my)).sum();
    let slope = sxy / sxx.max(1e-9);
    let r2 = sxy * sxy / (sxx * syy).max(1e-9);
    say!("  economy: tier moves up {}, down {}; rank-size (towns and up, n {}): slope {slope:.2}, r2 {r2:.2}; populations {}", up.len(), down.len(), pts.len(), [0usize, 1, 4, 9, 19].iter().filter_map(|&r| pops.get(r).map(|p| format!("#{} {:.0}", r + 1, p))).collect::<Vec<_>>().join(", "));
    let line = |what: &str, p: &worldgen::t0::economy::Place| {
        let s = &sts[p.index];
        say!("      {what} #{} at ({:.0}, {:.0}): {:?} -> {:?}, pop {}, wealth {:.2} (food {:.2}, industry {:.2}, gateway {:.2}, trade {:.2}), {}", p.index, s.x, s.y, p.was, s.tier, s.population, p.wealth, p.food, p.industry, p.gateway, p.trade, s.reason.name());
    };
    for p in up.iter().take(top) {
        line("grew", p);
    }
    for p in down.iter().take(top) {
        line("shrank", p);
    }
    // Traffic per class as built, and the stretches whose class it changed.
    let names = ["king's road", "road", "track"];
    let tr = &extra.traffic;
    let mut parts = Vec::new();
    let mut by = Vec::new();
    for c in 0..3u8 {
        let segs: Vec<_> = tr.iter().filter(|t| t.now as u8 == c).collect();
        let len: f64 = segs.iter().map(|t| t.length_ft).sum();
        let mean = segs.iter().map(|t| t.flow as f64 * t.length_ft).sum::<f64>() / len.max(1.0);
        let mut f: Vec<f64> = segs.iter().map(|t| t.flow as f64).collect();
        f.sort_by(|a, b| a.total_cmp(b));
        parts.push(format!("{} {:.0} mi mean {:.2} p90 {:.2}", names[c as usize], len / MI, mean, pct(&f, 0.9)));
        by.push(json!({ "class": names[c as usize], "mi": len / MI, "mean_flow": mean }));
    }
    let changed = |a: RoadClass, b: RoadClass| tr.iter().filter(|t| t.was == a && t.now == b).collect::<Vec<_>>();
    let (r_k, k_r, r_t) = (changed(RoadClass::Road, RoadClass::KingsRoad), changed(RoadClass::KingsRoad, RoadClass::Road), changed(RoadClass::Road, RoadClass::Track));
    let mi_of = |v: &[&worldgen::t0::roads::SegTraffic]| v.iter().map(|t| t.length_ft).sum::<f64>() / MI;
    say!("  traffic per class (flow-weighted mean, p90 per stretch): {}; king's road threshold {:.2}", parts.join(", "), extra.economy.traffic.kings);
    say!("    class from traffic: road -> king's road {} ({:.0} mi), king's road -> road {} ({:.0} mi), road -> track {} ({:.0} mi)", r_k.len(), mi_of(&r_k), k_r.len(), mi_of(&k_r), r_t.len(), mi_of(&r_t));
    let mut big = r_k.clone();
    big.sort_by(|a, b| b.length_ft.total_cmp(&a.length_ft));
    for t in big.iter().take(top) {
        say!("      to king's road at ({:.0}, {:.0}): {:.1} mi, flow {:.2}", t.at[0], t.at[1], t.length_ft / MI, t.flow);
    }
    for t in k_r.iter().chain(&r_t).take(top) {
        say!("      dropped {:?} -> {:?} at ({:.0}, {:.0}): {:.1} mi, flow {:.2}", t.was, t.now, t.at[0], t.at[1], t.length_ft / MI, t.flow);
    }
    json!({ "up": up.len(), "down": down.len(), "rank_size_slope": slope, "rank_size_r2": r2, "traffic": by, "road_to_kings": r_k.len(), "kings_to_road": k_r.len(), "road_to_track": r_t.len() })
}

pub(crate) fn stairs(world: &worldgen::World, t0: &T0) -> Value {
    use worldgen::t0::settle::Tier;
    use worldgen::town::stairs::{self, Kind, StairParams};
    const GRADES: [f64; 6] = [0.10, 0.12, 0.15, 0.20, 0.25, 0.33];
    let sets: Vec<StairParams> = [0.12, 0.15, 0.20].iter().map(|&g| StairParams { g_on: g, g_off: 0.8 * g, ..StairParams::DEFAULT }).collect();
    #[derive(Default)]
    struct Row {
        len: f64,
        at: [f64; 6],
        grades: Vec<(f64, f64)>,
        flights: [usize; 3],
        stepped: [f64; 3],
        towns: [usize; 3],
    }
    let kinds = [("strip", Kind::Edge, 3u8), ("main", Kind::Edge, 4), ("alley", Kind::Alley, 3), ("approach", Kind::Approach, 4)];
    let mut rows: std::collections::BTreeMap<(u8, usize), Row> = Default::default();
    // Steepest flights at the default: (grade, middle, rise, length, settlement).
    let mut steep: Vec<(f64, P, f64, f64, usize)> = Vec::new();
    let t = std::time::Instant::now();
    for (si, st) in t0.settlements.iter().enumerate().filter(|(_, st)| st.tier >= Tier::Town) {
        let profiles = stairs::survey(world, t0, si);
        let mut any = [[false; 3]; 4];
        for p in &profiles {
            let ki = kinds.iter().position(|k| k.1 == p.kind && k.2 == p.class).unwrap_or(0);
            let row = rows.entry((st.tier as u8, ki)).or_default();
            row.len += p.len;
            for k in 1..p.s.len() {
                let ds = p.s[k] - p.s[k - 1];
                let g = (p.h[k] - p.h[k - 1]).abs() / ds.max(1e-9);
                row.grades.push((g, ds));
                for (j, &lim) in GRADES.iter().enumerate() {
                    if g >= lim {
                        row.at[j] += ds;
                    }
                }
            }
            for (j, prm) in sets.iter().enumerate() {
                let runs = stairs::runs(p, prm);
                row.flights[j] += runs.len();
                row.stepped[j] += runs.iter().map(|r| r.1 - r.0).sum::<f64>();
                any[ki][j] |= !runs.is_empty();
            }
        }
        for (ki, a) in any.iter().enumerate() {
            let row = rows.entry((st.tier as u8, ki)).or_default();
            for j in 0..3 {
                row.towns[j] += a[j] as usize;
            }
        }
        for f in stairs::flights(&profiles, &StairParams::DEFAULT).iter().filter(|f| f.pts.len() >= 2) {
            let length: f64 = f.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
            if steep.last().is_some_and(|l| l.4 == si && l.2 == f.rise as f64 && (l.3 - length).abs() < 1e-6) {
                continue;
            }
            steep.push((f.rise as f64 / length.max(1.0), lerp(f.pts[0], f.pts[f.pts.len() - 1], 0.5), f.rise as f64, length, si));
        }
    }
    say!("  steep streets (towns and up, {:.1} s): length, at grade >= {GRADES:?}; p50 p90 p99; flights / stepped ft / settlements with any, steps from 0.12 | 0.15 | 0.20", t.elapsed().as_secs_f64());
    let mut out = Vec::new();
    for ((tier, ki), row) in &mut rows {
        if row.len <= 0.0 {
            continue;
        }
        row.grades.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total: f64 = row.grades.iter().map(|g| g.1).sum();
        let pct = |q: f64| {
            let mut acc = 0.0;
            row.grades.iter().find(|g| {
                acc += g.1;
                acc >= q * total
            }).map_or(0.0, |g| g.0)
        };
        let tier_name = [Tier::Village, Tier::Town, Tier::City, Tier::Metropolis].iter().find(|t| **t as u8 == *tier).map_or("?".to_string(), |t| format!("{t:?}"));
        let at: Vec<String> = row.at.iter().map(|a| format!("{:.1}%", 100.0 * a / row.len)).collect();
        let fl: Vec<String> = (0..3).map(|j| format!("{} / {:.0} / {}", row.flights[j], row.stepped[j], row.towns[j])).collect();
        say!(
            "    {tier_name:<10} {:<8} {:>8.0} ft  {}  p {:.3} {:.3} {:.3}  {}",
            kinds[*ki].0,
            row.len,
            at.join(" "),
            pct(0.5),
            pct(0.9),
            pct(0.99),
            fl.join(" | ")
        );
        out.push(json!({ "tier": tier_name, "street": kinds[*ki].0, "length_ft": row.len.round(), "at_grade": GRADES.iter().zip(&row.at).map(|(g, a)| json!([g, a.round()])).collect::<Vec<_>>(), "p50": pct(0.5), "p90": pct(0.9), "p99": pct(0.99), "flights": row.flights, "stepped_ft": row.stepped.map(|x| x.round()), "settlements": row.towns }));
    }
    steep.sort_by(|a, b| b.0.total_cmp(&a.0));
    let top = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    for (g, at, rise, length, si) in steep.iter().take(top) {
        say!("      flight at ({:.0}, {:.0}) in settlement {si} ({:?}): rise {rise:.1} ft over {length:.0} ft (grade {g:.2})", at[0], at[1], t0.settlements[*si].tier);
    }
    Value::Array(out)
}

/// Drawn roads against the roads made from them: per road stroke, the line sampled every
/// 500 ft (on land, and outside towns, where streets take over) and each sample's distance to the nearest
/// point of a road following it; per `none` line, the roads crossing it.
pub(crate) fn drawn_roads(world: &worldgen::World, t0: &T0, wet: &dyn Fn(P) -> bool) -> Vec<Value> {
    const MI: f64 = 5280.0;
    let mut out = Vec::new();
    for (si, s) in world.file.sketch.strokes.iter().enumerate().filter(|(_, s)| s.tool == worldgen::world::SketchTool::Road) {
        let name = s.name.clone().unwrap_or_else(|| format!("stroke {si}"));
        let curve = |rc: &worldgen::lod::roads::RoadCurve| {
            let mut pts: Vec<P> = Vec::new();
            for k in 0..rc.pts.len() - 1 {
                let n = ((rc.s[k + 1] - rc.s[k]) / 100.0).ceil().max(1.0) as usize;
                for j in 0..n {
                    pts.push(rc.eval(k, j as f64 / n as f64, 5.0, t0.cell_ft).p);
                }
            }
            pts
        };
        if s.kind.as_deref() == Some("none") {
            let mut crossings = 0;
            for rc in &t0.roads.roads {
                let pts = curve(rc);
                for w in pts.windows(2) {
                    if s.pts.windows(2).any(|l| seg_cross(w[0], w[1], l[0], l[1])) {
                        crossings += 1;
                    }
                }
            }
            say!("  drawn {name} (no road): {crossings} road crossings");
            out.push(json!({ "stroke": si, "name": name, "none": true, "crossings": crossings }));
            continue;
        }
        // The roads following it, as points in 1-mi buckets.
        let mut grid: HashMap<(i64, i64), Vec<P>> = HashMap::new();
        let mut pieces = 0;
        for rc in t0.roads.roads.iter().filter(|rc| rc.stroke == Some(si as u32)) {
            pieces += 1;
            for p in curve(rc) {
                grid.entry(((p[0] / MI).floor() as i64, (p[1] / MI).floor() as i64)).or_default().push(p);
            }
        }
        let nearest = |p: P| {
            let (ci, cj) = ((p[0] / MI).floor() as i64, (p[1] / MI).floor() as i64);
            let mut best = f64::INFINITY;
            for r in 0..12i64 {
                for j in cj - r..=cj + r {
                    for i in ci - r..=ci + r {
                        if (i - ci).abs() != r && (j - cj).abs() != r {
                            continue;
                        }
                        for q in grid.get(&(i, j)).into_iter().flatten() {
                            best = best.min(dist(p, *q));
                        }
                    }
                }
                if best < (r as f64) * MI {
                    break;
                }
            }
            best
        };
        // (Inside a town the streets take over: roads end at its edge.)
        let in_town = |p: P| t0.settlements.iter().any(|st| dist([st.x, st.y], p) < worldgen::town::road_trim_radius(st.tier, st.population));
        let samples: Vec<P> = along(&s.pts, 500.0).into_iter().filter(|p| !in_town(*p) && !wet(*p)).collect();
        let mut off: Vec<f64> = samples.iter().map(|p| nearest(*p)).collect();
        let len: f64 = s.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        let covered = off.iter().filter(|d| **d <= MI).count() as f64 / off.len().max(1) as f64;
        off.sort_by(|a, b| a.total_cmp(b));
        let mean = off.iter().filter(|d| d.is_finite()).sum::<f64>() / off.iter().filter(|d| d.is_finite()).count().max(1) as f64;
        let p95 = off[((off.len() as f64 * 0.95) as usize).min(off.len() - 1)];
        let max = *off.last().unwrap();
        say!("  drawn {name}: {:.0} mi in {pieces} pieces, {:.1}% within a mile, offset mean {:.2} p95 {:.2} max {:.2} mi", len / MI, covered * 100.0, mean / MI, p95 / MI, max / MI);
        out.push(json!({ "stroke": si, "name": name, "length_mi": len / MI, "pieces": pieces, "within_mi": covered, "mean_mi": mean / MI, "p95_mi": p95 / MI, "max_mi": max / MI }));
    }
    out
}
