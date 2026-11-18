use crate::prelude::*;

/// The share on water per tier (realms left out) and the count per reason.
/// Islands per size band: how many, how many settled, their settlements by tier; the share of
/// all settlements on islands; road points out at open sea (the cell and all round it sea);
/// island settlements with no road while others share their island, and islands whose
/// settlements the roads split into more than one group.
pub(crate) fn islands(world: &worldgen::World, t0: &T0) {
    use worldgen::t0::biome::Biome;
    use worldgen::t0::islands;
    let (w, h, cell) = (t0.biome.w, t0.biome.h, t0.cell_ft);
    let b = |k: usize| Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
    let land: Vec<bool> = (0..w * h).map(|k| b(k) != Biome::Ocean).collect();
    let lake: Vec<u32> = (0..w * h).map(|k| if b(k) == Biome::Lake { 0 } else { worldgen::t0::hydro::NO_LAKE }).collect();
    let isl = islands::label(w, h, cell, &land, &lake, &t0.biome.data);
    let bands = [(0.0, 10.0, "<10"), (10.0, 60.0, "10-60"), (60.0, 400.0, "60-400"), (400.0, 1500.0, "400-1500"), (1500.0, f64::INFINITY, ">1500")];
    let sts = &t0.settlements;
    let on: Vec<Option<u32>> = sts.iter().map(|s| isl.island_at(s.cell)).collect();
    let mut lines = Vec::new();
    for (lo, hi, name) in bands {
        let ids: Vec<u32> = (0..isl.parts.len() as u32).filter(|&c| c != isl.mainland && isl.parts[c as usize].area_mi2 >= lo && isl.parts[c as usize].area_mi2 < hi).collect();
        let settled = ids.iter().filter(|&&c| on.contains(&Some(c))).count();
        let mut tiers = [0usize; 4];
        for (i, s) in sts.iter().enumerate() {
            if on[i].is_some_and(|c| ids.contains(&c)) {
                tiers[s.tier as usize] += 1;
            }
        }
        lines.push(format!("{name}: {} islands, {settled} settled, v/t/c/m {}/{}/{}/{}", ids.len(), tiers[0], tiers[1], tiers[2], tiers[3]));
    }
    let n_on = on.iter().filter(|c| c.is_some()).count();
    // (`AT=x,y`: the land mass there.)
    if let Some(at) = std::env::var("AT").ok().and_then(|v| v.split_once(',').and_then(|(x, y)| Some([x.parse::<f64>().ok()?, y.parse::<f64>().ok()?]))) {
        let k = (at[1] / cell).round() as usize * w + (at[0] / cell).round() as usize;
        let c = isl.of[k];
        say!("  at ({:.0}, {:.0}): land mass {c}{}", at[0], at[1], if c == isl.mainland { " (mainland)".to_string() } else if c == islands::NONE { " (sea)".to_string() } else { format!(" ({:.0} sq mi)", isl.parts[c as usize].area_mi2) });
    }
    // (Those of 10 sq mi or more left empty: why.)
    let seed = world.stream("t0.islands");
    let mut empty = Vec::new();
    for c in 0..isl.parts.len() as u32 {
        let p = &isl.parts[c as usize];
        if c != isl.mainland && p.area_mi2 >= 10.0 && !on.contains(&Some(c)) {
            let o = isl.owed(c, seed, (0.0, 0.0));
            empty.push(format!("{:.0} sq mi {:.0}% barren owed {} at ({:.0}, {:.0})", p.area_mi2, 100.0 * p.barren, o.villages, (p.first % w) as f64 * cell, (p.first / w) as f64 * cell));
        }
    }
    if !empty.is_empty() {
        say!("  islands left empty (10 sq mi and up): {}", empty.join("; "));
    }
    let shore: Vec<&worldgen::t0::settle::Settlement> = sts.iter().zip(&on).filter(|(s, c)| c.is_some() && s.coastal && s.tier == worldgen::t0::settle::Tier::Village).map(|(s, _)| s).collect();
    let fishing = shore.iter().filter(|s| s.kind == worldgen::t0::settle::SettleKind::Fishing).count();
    say!("  islands: {}; on islands {n_on}/{} settlements ({:.1}%), shore villages fishing {fishing}/{}", lines.join("; "), sts.len(), 100.0 * n_on as f64 / sts.len().max(1) as f64, shore.len());
    // Roads: out at open sea, and on the islands.
    let cell_of = |p: P| {
        let (i, j) = ((p[0] / cell).round() as i64, (p[1] / cell).round() as i64);
        (i >= 0 && j >= 0 && i < w as i64 && j < h as i64).then(|| j as usize * w + i as usize)
    };
    let open_sea = |k: usize| !land[k] && worldgen::t0::flood::neighbors(w, h, k).all(|(nb, _)| !land[nb]);
    let (mut sea_pts, mut sea_roads) = (0usize, 0usize);
    let mut island_roads: Vec<(usize, Vec<P>)> = Vec::new();
    for (ri, r) in t0.roads.roads.iter().enumerate() {
        let pts = along(&r.pts, 0.25 * cell);
        let wet = pts.iter().filter(|&&p| cell_of(p).is_some_and(open_sea)).count();
        sea_pts += wet;
        sea_roads += (wet > 0) as usize;
        if pts.iter().any(|&p| cell_of(p).is_some_and(|k| isl.island_at(k).is_some())) {
            island_roads.push((ri, pts));
        }
    }
    // (Union-find over the island settlements and roads: a settlement on a road within a mile of
    // it; two roads joined where one's end lies within 300 ft of the other.)
    let ns = sts.len();
    let mut parent: Vec<usize> = (0..ns + island_roads.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let near = |a: &[P], q: P, r: f64| a.iter().any(|p| dist(*p, q) <= r);
    let mut roadless = 0;
    for (i, s) in sts.iter().enumerate() {
        let Some(c) = on[i] else { continue };
        let mut any = false;
        for (j, (_, pts)) in island_roads.iter().enumerate() {
            if near(pts, [s.x, s.y], 5280.0) {
                let (a, b) = (find(&mut parent, i), find(&mut parent, ns + j));
                parent[a] = b;
                any = true;
            }
        }
        if !any && on.iter().filter(|&&o| o == Some(c)).count() > 1 {
            roadless += 1;
            let nearest = sts.iter().enumerate().filter(|&(j, _)| j != i && on[j] == Some(c)).map(|(_, o)| dist([o.x, o.y], [s.x, s.y]) / 5280.0).fold(f64::INFINITY, f64::min);
            say!("      no road: {} at ({:.0}, {:.0}){}, nearest on its island {nearest:.1} mi", s.tier.name(), s.x, s.y, if s.isolated { " (realm)" } else { "" });
        }
    }
    for a in 0..island_roads.len() {
        for b in 0..island_roads.len() {
            let (pa, pb) = (&island_roads[a].1, &island_roads[b].1);
            if a != b && !pa.is_empty() && (near(pb, pa[0], 300.0) || near(pb, pa[pa.len() - 1], 300.0)) {
                let (x, y) = (find(&mut parent, a + ns), find(&mut parent, b + ns));
                parent[x] = y;
            }
        }
    }
    let mut split = 0;
    for c in 0..isl.parts.len() as u32 {
        let mut groups: Vec<usize> = (0..ns).filter(|&i| on[i] == Some(c)).map(|i| find(&mut parent, i)).collect();
        groups.sort();
        groups.dedup();
        if groups.len() > 1 {
            split += 1;
            let members: Vec<String> = (0..ns).filter(|&i| on[i] == Some(c)).map(|i| format!("{}{}", find(&mut parent, i) % 1000, &sts[i].tier.name()[..1])).collect();
            say!("      split: {:.0} sq mi island, {} groups: {}", isl.parts[c as usize].area_mi2, groups.len(), members.join(" "));
        }
    }
    say!("  island roads: {} roads on islands; road points at open sea {sea_pts} on {sea_roads} roads; island settlements with no road (others on the island) {roadless}; islands split by roads {split}", island_roads.len());
}
