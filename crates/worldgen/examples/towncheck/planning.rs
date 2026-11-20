use crate::prelude::*;
use worldgen::t0::settle::Tier;

/// (Roadmap 7, 161) How regular a town's streets are: the share of its streets' and alleys' length
/// within 10 deg of the town's two main axes, the mean rectangularity of its
/// patches (area over their box along those axes) and the share of four-cornered patches.
fn regularity(l: &Layout) -> (f64, f64, f64) {
    let polys: Vec<&Vec<P>> = l.districts.iter().filter(|p| p.len() >= 3).collect();
    // (The town's streets and alleys, as drawn: class 3 and 4 road pieces.)
    let edges: Vec<(f64, f64)> = l.roads.iter().filter(|r| r.1 >= 3).flat_map(|r| r.0.windows(2).map(|w| (w[0], w[1]))).filter_map(|(a, b)| (dist(a, b) > 10.0).then(|| (libm::atan2(b[1] - a[1], b[0] - a[0]), dist(a, b)))).collect();
    // (The axes: the heading, of 90 tried a degree apart, with the most length within 10 deg.)
    let quarter = std::f64::consts::FRAC_PI_2;
    let near = |t: f64, axis: f64| {
        let d = (t - axis).rem_euclid(quarter);
        d.min(quarter - d) <= 10f64.to_radians()
    };
    let total: f64 = edges.iter().map(|e| e.1).sum();
    let share = |axis: f64| edges.iter().filter(|e| near(e.0, axis)).map(|e| e.1).sum::<f64>();
    let axis = (0..90).map(|k| (k as f64).to_radians()).max_by(|a, b| share(*a).total_cmp(&share(*b))).unwrap();
    let on = share(axis);
    let (u, v) = ([libm::cos(axis), libm::sin(axis)], [-libm::sin(axis), libm::cos(axis)]);
    let rect = polys
        .iter()
        .map(|p| {
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for q in p.iter() {
                let x = [dot(*q, u), dot(*q, v)];
                for k in 0..2 {
                    lo[k] = lo[k].min(x[k]);
                    hi[k] = hi[k].max(x[k]);
                }
            }
            area(p).abs() / ((hi[0] - lo[0]) * (hi[1] - lo[1])).max(1.0)
        })
        .sum::<f64>()
        / polys.len().max(1) as f64;
    // (Corners turning under 15 deg are not counted.)
    let corners = |p: &[P]| {
        (0..p.len())
            .filter(|&k| {
                let (a, b, c) = (p[(k + p.len() - 1) % p.len()], p[k], p[(k + 1) % p.len()]);
                let (d1, d2) = (sub(b, a), sub(c, b));
                libm::atan2(cross(d1, d2), dot(d1, d2)).abs() > 15f64.to_radians()
            })
            .count()
    };
    let quads = polys.iter().filter(|p| corners(p) == 4).count() as f64 / polys.len().max(1) as f64;
    (if total > 0.0 { on / total } else { 0.0 }, rect, quads)
}

/// (161) Per town and city (as generated, not by hand): its rolled planning, and how regular its
/// patches are grown (planning 0: as before 161), as rolled, and planned (1); then the main
/// streets' rule (`streetsin`) with every town planned.
pub(crate) fn planning(ctx: &mut Ctx, label: &str, r: &mut Report) {
    let towns: Vec<usize> = {
        let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
        (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= Tier::Town && town::hand::size(world, li).is_none()).collect()
    };
    let with = |world: &World, v: u8| {
        let mut file = world.file.clone();
        for &li in &towns {
            file.edits.planning.insert(li.to_string(), v);
        }
        World::new(file).unwrap()
    };
    let (grown, planned) = (with(&ctx.ex.world, 0), with(&ctx.ex.world, 100));
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut by_class: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for &li in &towns {
        let s = town::culture::settlement(world, t0, li).unwrap();
        let p = town::planning::of(world, t0, li, &s);
        let class = town::planning::class(p);
        let name = worldgen::agent::feature_of_layout(world, t0, li).and_then(|id| worldgen::agent::name_of(world, t0, &id)).unwrap_or_else(|| "?".into());
        let m: Vec<(f64, f64, f64)> = [&grown, world, &planned].iter().map(|w| regularity(&town::layout(w, t0, li))).collect();
        say!(
            "    {label} L{li} {} {name}: planning {p:.2} {class}; on axes {:.0}/{:.0}/{:.0}%, rect {:.2}/{:.2}/{:.2}, quads {:.0}/{:.0}/{:.0}% (grown/rolled/planned)",
            s.tier.name(),
            100.0 * m[0].0,
            100.0 * m[1].0,
            100.0 * m[2].0,
            m[0].1,
            m[1].1,
            m[2].1,
            100.0 * m[0].2,
            100.0 * m[1].2,
            100.0 * m[2].2
        );
        svg_dump(label, li, &town::layout(world, t0, li), [world.geom.map_w_ft, world.geom.map_h_ft]);
        r.tally(&format!("towns {class}"), 1);
        for (k, what) in ["grown", "as rolled", "planned"].iter().enumerate() {
            r.value(&format!("on axes % {what}"), 100.0 * m[k].0, format!("L{li}"));
            r.value(&format!("rect {what}"), m[k].1, format!("L{li}"));
        }
        r.value(&format!("on axes % as rolled, {class}"), 100.0 * m[1].0, format!("L{li}"));
        by_class.entry(class).or_default().push(p);
    }
    for (class, v) in &by_class {
        say!("    {label} {class}: {}", v.iter().map(|p| format!("{p:.2}")).collect::<Vec<_>>().join(" "));
    }
    // The main streets' rule (straight in from the gates) with every town grown (as before 161),
    // and planned.
    for (what, w) in [("grown", grown), ("planned", planned)] {
        let was = std::mem::replace(&mut ctx.ex.world, w);
        let mut rs = Report::default();
        streets_in(ctx, label, &mut rs);
        ctx.ex.world = was;
        say!("    streetsin, every town {what}:");
        rs.print();
    }
}

/// (161) With `TC_SVG=<dir>` and `TC_SVG_TOWNS=<label>:L<li>,...` (e.g. `seed 3:L1`): the town's
/// patches, blocks, streets and buildings drawn to `<dir>/<label>-L<li>.svg`, for a look at its plan.
fn svg_dump(label: &str, li: usize, l: &Layout, map: [f64; 2]) {
    let (Ok(dir), Ok(towns)) = (std::env::var("TC_SVG"), std::env::var("TC_SVG_TOWNS")) else { return };
    if !towns.split(',').any(|t| t.trim() == format!("{label}:L{li}")) {
        return;
    }
    let pts: Vec<P> = l.districts.iter().flatten().copied().collect();
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for q in &pts {
        for k in 0..2 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    }
    let w = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0);
    let path = |p: &[P], close: bool| {
        let mut d = String::new();
        for (k, q) in p.iter().enumerate() {
            d += &format!("{}{:.0} {:.0} ", if k == 0 { "M" } else { "L" }, q[0] - lo[0], q[1] - lo[1]);
        }
        if close {
            d += "Z";
        }
        d
    };
    let mut svg = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {w:.0} {w:.0}' width='960' height='960'><rect width='100%' height='100%' fill='#cfe0b8'/>");
    for b in &l.blocks {
        svg += &format!("<path d='{}' fill='#e8dcc0'/>", path(b, true));
    }
    for r in l.roads.iter().filter(|r| r.1 >= 3) {
        svg += &format!("<path d='{}' fill='none' stroke='#9a8f80' stroke-width='{:.0}'/>", path(&r.0, false), r.2);
    }
    for b in &l.buildings {
        svg += &format!("<path d='{}' fill='#a0604a'/>", path(&b.poly, true));
    }
    for d in &l.districts {
        svg += &format!("<path d='{}' fill='none' stroke='#2040c0' stroke-width='{:.1}'/>", path(d, true), w / 500.0);
    }
    svg += "</svg>";
    let file = format!("{dir}/{}-L{li}.svg", label.replace(' ', ""));
    say!("    svg {file}: centre fx {:.6} fy {:.6}", l.center[0] / map[0], l.center[1] / map[1]);
    std::fs::write(&file, svg).unwrap();
}
