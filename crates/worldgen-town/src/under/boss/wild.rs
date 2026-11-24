//! A cave's, lava tube's or mine's boss chamber: the natural sets and the half-dug face.

use super::*;

/// A cave's, lava tube's or mine's boss chamber: maybe a second way in, a layout of its set (`NATURAL`, `MINE`) by fit,
/// the theme's dressing and the hoard by the far end, maybe hazards. A cave boss on its own lake's island
/// (`themes::cave`) is that layout already: dressed only, no second way.
pub(crate) fn wild(p: &mut Plan, nx: usize, ny: usize, ri: i16, squares: &[usize], kind: UnderKind, rng: &mut Pcg32) {
    let h = Hall::new(p, nx, ny, ri, squares);
    let theme = p.theme;
    let isle = islet(p, nx, ny, ri, squares);
    let mine = kind == UnderKind::Mine;
    let lava = kind == UnderKind::LavaTube;
    let (names, fit): (&[&str], &[(usize, usize)]) = if mine { (&MINE, &MFIT) } else { (&NATURAL, &NFIT) };
    let mut order: Vec<usize> = (0..names.len())
        .filter(|&i| !isle && h.l >= fit[i].0 && h.w >= fit[i].1 && (mine || names[i] != if lava { "pool island" } else { "lava channel" }))
        .collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i as u32 + 1) as usize);
    }
    // (A flooded cave's boss on an island in a pool.)
    if theme == "flooded" && let Some(m) = order.iter().position(|&i| names[i] == "pool island") {
        order[..=m].rotate_right(1);
    }
    let fallback = far_point(p, &h);
    let (stage, dressed) = if mine {
        choose(p, &h, order, rng, &|p, h, i, rng| mine_layout(p, h, MINE[i], rng), &|p, h, s, rng| mine_dress(p, h, s, rng), fallback)
    } else {
        choose(p, &h, order, rng, &|p, h, i, rng| wild_layout(p, h, NATURAL[i], lava, rng), &|p, h, s, rng| wild_dress(p, h, s, theme, rng), fallback)
    };
    spread(p, &h, stage, cover_kinds(if mine { "mine" } else if lava { "lava_tube" } else { theme }, true), rng);
    let kinds: &[&'static str] = match theme {
        _ if lava => &["steam vent", "steam vent", "cracked floor"],
        _ if mine => &["cracked floor", "flooded pit"],
        "flooded" | "cave" | "fungal" => &["cracked floor", "flooded pit"],
        _ => &["cracked floor"],
    };
    hazards(p, &h, kinds, stage, dressed, rng);
}

/// The room's squares in frame coordinates.
pub(super) fn frame_squares(p: &Plan, h: &Hall) -> Vec<(isize, isize)> {
    (0..h.l as isize).flat_map(|d| (0..h.w as isize).map(move |a| (d, a))).filter(|&(d, a)| h.sq(p, d, a).is_some()).collect()
}

pub(super) fn wild_layout(p: &mut Plan, h: &Hall, name: &str, lava: bool, rng: &mut Pcg32) -> Option<Stage> {
    let (l, w) = (h.l as isize, h.w as isize);
    let stage = far_point(p, h);
    let all = frame_squares(p, h);
    let floor = |p: &Plan, d: isize, a: isize| h.sq(p, d, a).is_some_and(|k| !p.taken[k] && !h.kept(k));
    let (dc, ac) = (all.iter().map(|s| s.0).sum::<isize>() / all.len().max(1) as isize, all.iter().map(|s| s.1).sum::<isize>() / all.len().max(1) as isize);
    match name {
        "rock columns" => {
            // Columns standing clear of the walls, two or more apart, a stalagmite or two among them.
            let kind = if lava { "basalt" } else { "rock_column" };
            let inner: Vec<(isize, isize)> = all.iter().copied().filter(|&(d, a)| d >= 2 && (-1..=1).all(|i| (-1..=1).all(|j| h.sq(p, d + i, a + j).is_some()))).collect();
            if inner.is_empty() {
                return None;
            }
            let want = (all.len() / 14).clamp(3, 6);
            let mut at: Vec<(isize, isize)> = Vec::new();
            for _ in 0..want * 6 {
                if at.len() >= want {
                    break;
                }
                let (d, a) = inner[rng.below(inner.len() as u32) as usize];
                if at.iter().all(|&(e, b)| (e - d).abs().max((b - a).abs()) >= 2) && set(p, h, h.item(kind, d, a, 1, 1, None), false) {
                    at.push((d, a));
                }
            }
            for _ in 0..1 + rng.below(2) {
                let (d, a) = inner[rng.below(inner.len() as u32) as usize];
                set(p, h, h.item(if lava { "obsidian" } else { "stalagmite" }, d, a, 1, 1, None), false);
            }
            (at.len() >= 3).then_some(stage)
        }
        "ledges" => {
            // Raised shelves along the side walls (5 or 10 ft), up a rubble slope at one end or from the floor beside.
            let kind = if rng.below(2) == 0 { "platform" } else { "balcony" };
            let first = rng.below(2) as u8;
            let two = h.w >= 9 && rng.below(5) < 2;
            let mut items = Vec::new();
            let mut tops = Vec::new();
            for s in 0..1 + two as u8 {
                let Some((rects, mut ups)) = shelf(p, h, (first + s) % 2, 3, (1, l - 1)) else {
                    if s == 0 {
                        return None;
                    }
                    break;
                };
                items.extend(rects.iter().map(|&(d, a, dl, aw)| h.item(kind, d, a, dl, aw, None)));
                let turn = rng.below(ups.len().max(1) as u32) as usize;
                ups.rotate_left(turn);
                let &((ud, ua), _) = ups.first()?;
                items.push(h.item("rubble_slope", ud, ua, 1, 1, None));
                tops.push(rects[rects.len() - 1]);
            }
            if !lay(p, h, items) {
                return None;
            }
            // (A cache up on the shelf's far end.)
            for (d, a, _, _) in tops {
                set(p, h, h.item("chest", d, a, 1, 1, None), true);
            }
            Some(stage)
        }
        "rope-bridge chasm" => {
            // A chasm two rows wide across the chamber, a rope bridge over it in line with the middle.
            let dm = (l / 2 - 1).clamp(2, l - 4);
            if all.iter().filter(|s| s.0 > dm + 1).count() < 6 {
                return None;
            }
            let mut cols: Vec<isize> = (0..w).collect();
            cols.sort_by_key(|&a| ((a - ac).abs(), a));
            let ba = cols.into_iter().find(|&a| (dm - 1..=dm + 2).all(|d| floor(p, d, a)))?;
            let mut items = Vec::new();
            for d in [dm, dm + 1] {
                let mut a = 0;
                while a < w {
                    if h.sq(p, d, a).is_none() {
                        a += 1;
                        continue;
                    }
                    let mut b = a;
                    while b + 1 < w && h.sq(p, d, b + 1).is_some() {
                        b += 1;
                    }
                    items.push(h.item("chasm", d, a, 1, (b - a + 1) as usize, None));
                    a = b + 1;
                }
            }
            items.push(h.item("rope_bridge", dm, ba, 2, 1, None));
            lay(p, h, items).then_some(stage)
        }
        "rubble field" => {
            // A collapsed ceiling: rubble heaped round the middle, a boulder or two, the unstable roof over it.
            let r = if all.len() >= 60 { 3 } else { 2 };
            set(p, h, h.item("cave_in", dc, ac, 1, 1, None), false);
            let mut n = 0;
            for &(d, a) in &all {
                let (dd, da) = (d - dc, a - ac);
                if dd * dd + da * da > r * r + 1 || d < 2 || rng.below(20) >= 11 {
                    continue;
                }
                n += set(p, h, h.item("rubble", d, a, 1, 1, None), false) as usize;
            }
            for _ in 0..1 + rng.below(2) {
                let (d, a) = (dc - r + rng.below(2 * r as u32 + 1) as isize, ac - r + rng.below(2 * r as u32 + 1) as isize);
                near(p, h, false, "boulder", d, a, 2, 2, None);
            }
            (n >= 5).then_some(stage)
        }
        "pool island" => {
            // Deep water round an island (its far side may be the chamber's wall), stepping stones out to it from the
            // near side, shallows here and there round the pool.
            let il: isize = if all.len() >= 70 { 3 } else { 2 };
            let (fd, _) = (stage.far(), stage.mid());
            let (td, ta) = ((dc + fd) / 2, ac);
            let mut spots: Vec<(isize, isize)> = (2..l - il + 1).flat_map(|d| (0..w - il + 1).map(move |a| (d, a))).collect();
            spots.sort_by_key(|&(d, a)| ((d + il / 2 - td).abs() + (a + il / 2 - ta).abs(), d, a));
            for (di, ai) in spots.into_iter().take(40) {
                let isle = |d: isize, a: isize| d >= di && d < di + il && a >= ai && a < ai + il;
                if !(di..di + il).all(|d| (ai..ai + il).all(|a| floor(p, d, a))) {
                    continue;
                }
                let (cd, ca) = (di - 1, ai + il / 2);
                if !floor(p, cd, ca) || !floor(p, cd - 1, ca) {
                    continue;
                }
                let mut items = Vec::new();
                let mut ok = true;
                for d in di - 1..=di + il {
                    for a in ai - 1..=ai + il {
                        if isle(d, a) {
                            continue;
                        }
                        match h.sq(p, d, a) {
                            Some(_) if floor(p, d, a) => items.push(h.item("water_deep", d, a, 1, 1, None)),
                            Some(_) => ok = false,
                            // (Off the room: rock only, or the island is reached from there.)
                            None => ok &= h.cell(d, a).is_none_or(|k| p.cells[k] < 0),
                        }
                    }
                }
                if !ok {
                    continue;
                }
                items.push(h.item("stepping_stone", cd, ca, 1, 1, None));
                for d in di - 2..=di + il + 1 {
                    for a in ai - 2..=ai + il + 1 {
                        let ring = d == di - 2 || d == di + il + 1 || a == ai - 2 || a == ai + il + 1;
                        if ring && (d, a) != (cd - 1, ca) && floor(p, d, a) && h.sq(p, d, a) != Some(h.entry) && rng.below(2) == 0 {
                            items.push(h.item("water_shallow", d, a, 1, 1, None));
                        }
                    }
                }
                let s = snap(p);
                if lay(p, h, items) {
                    return Some(Stage { d0: di, a0: ai, dl: il as usize, aw: il as usize, raised: false });
                }
                undo(p, s);
                h.clear.borrow_mut().clear();
            }
            None
        }
        "lava channel" => {
            // Lava across the chamber, a row or two wide, crossed by a rock bridge or basalt stepping stones.
            let rows = if h.l >= 10 { 2 } else { 1 };
            let dm = (l / 2 - 1).clamp(2, l - 2 - rows);
            if all.iter().filter(|s| s.0 >= dm + rows).count() < 5 {
                return None;
            }
            let mut cols: Vec<isize> = (0..w).collect();
            cols.sort_by_key(|&a| ((a - ac).abs(), a));
            let ba = cols.into_iter().find(|&a| (dm - 1..=dm + rows).all(|d| floor(p, d, a)))?;
            let mut items = Vec::new();
            for d in dm..dm + rows {
                for a in 0..w {
                    if a != ba && h.sq(p, d, a).is_some() {
                        items.push(h.rect(d, a, 1, 1).map(|(x, y, _, _)| super::kit::lava(x, y)));
                    }
                }
            }
            let span = if rng.below(2) == 0 { "stone_bridge" } else { "stepping_stone" };
            items.push(h.item(span, dm, ba, rows as usize, 1, None));
            lay(p, h, items).then_some(stage)
        }
        _ => None,
    }
}

/// The natural set's dressing by theme round the stage (a beast's nest and bone mounds, a fungus grove, a crystal
/// cluster, ice columns, obsidian and sulfur in a lava tube), the hoard and a chest by it.
pub(super) fn wild_dress(p: &mut Plan, h: &Hall, s: Stage, theme: &str, rng: &mut Pcg32) {
    let (f, m, top) = (s.far(), s.mid(), s.raised);
    let put = |p: &mut Plan, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize| near(p, h, top, kind, d, a, dl, aw, None);
    let scatter = |p: &mut Plan, kind: &'static str, n: u32, rng: &mut Pcg32| {
        for _ in 0..n {
            let (d, a) = (f - 1 - rng.below(3) as isize, m - 3 + rng.below(7) as isize);
            put(p, kind, d, a, 1, 1);
        }
    };
    match theme {
        "beast_den" => {
            put(p, "nest", f - 1, m - 1, 2, 2);
            put(p, "bone_mound", f - 2, m - 4, 2, 2);
            if rng.below(2) == 0 {
                put(p, "bone_mound", f - 2, m + 2, 2, 2);
            }
            scatter(p, "bones", 3, rng);
        }
        "fungal" => {
            scatter(p, "mushroom", 3, rng);
            scatter(p, "fungus", 3, rng);
        }
        "crystal" => scatter(p, "crystal", 5, rng),
        "ice" => scatter(p, "ice", 4, rng),
        "lava_tube" | LAVA_CAVE => {
            scatter(p, "obsidian", 2, rng);
            scatter(p, "sulfur", 2, rng);
        }
        _ => {
            scatter(p, if theme == "flooded" { "bones" } else { "stalagmite" }, 3, rng);
            put(p, "skeleton", f - 2, m + 2, 2, 1);
        }
    }
    put(p, "hoard", f, m, 1, 1);
    put(p, "chest", f, m + 1, 1, 1);
}

/// A raised strip along a wall (`wall`: 0 the side at a = 0, 1 the side at a = w - 1, 2 the far wall) over `span` of
/// the rows along it (d for a side wall, a for the far wall): on each row, from the wall's square out to one straight
/// inner line (at most `most` wide), all free floor clear of the way in, with floor beside it; the longest such run, of
/// 4 rows or more. Its rects (d, a, dl, aw), and the free squares a way up may take with the shelf's square each climbs
/// to (at the near end, the far end, beside its middle).
#[allow(clippy::type_complexity)]
pub(super) fn shelf(p: &Plan, h: &Hall, wall: u8, most: isize, span: (isize, isize)) -> Option<(Vec<(isize, isize, usize, usize)>, Vec<((isize, isize), (isize, isize))>)> {
    let (l, w) = (h.l as isize, h.w as isize);
    let deep = if wall == 2 { l } else { w };
    let to = |u: isize, v: isize| match wall {
        0 => (u, v),
        1 => (u, w - 1 - v),
        _ => (l - 1 - v, u),
    };
    let (ex, ey) = (h.entry % h.nx, h.entry / h.nx);
    let free = |(d, a): (isize, isize)| h.sq(p, d, a).is_some_and(|k| !p.taken[k] && !h.kept(k) && (k % h.nx).abs_diff(ex).max((k / h.nx).abs_diff(ey)) >= 2);
    let outer = |u: isize| (0..deep).find(|&v| {
        let (d, a) = to(u, v);
        h.sq(p, d, a).is_some()
    });
    let row_ok = |u: isize, line: isize| {
        outer(u).is_some_and(|o| o <= line && line - o < most && (o..=line).all(|v| free(to(u, v))) && {
            let (d, a) = to(u, line + 1);
            h.sq(p, d, a).is_some()
        })
    };
    let mut best: Option<(isize, isize, isize)> = None;
    for line in 0..=(deep / 3).min(most + 1) {
        let mut u = span.0;
        while u <= span.1 {
            if !row_ok(u, line) {
                u += 1;
                continue;
            }
            let mut e = u;
            while e < span.1 && row_ok(e + 1, line) {
                e += 1;
            }
            if best.is_none_or(|b| e - u > b.2 - b.1) {
                best = Some((line, u, e));
            }
            u = e + 1;
        }
    }
    let (line, u0, u1) = best.filter(|b| b.2 - b.1 >= 3)?;
    let rects = (u0..=u1)
        .map(|u| {
            let (a, b) = (to(u, outer(u).unwrap_or(line)), to(u, line));
            (a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).unsigned_abs() + 1, (a.1 - b.1).unsigned_abs() + 1)
        })
        .collect();
    let mid = (u0 + u1) / 2;
    let ups = [((u0 - 1, line), (u0, line)), ((u1 + 1, line), (u1, line)), ((mid, line + 1), (mid, line))].into_iter().map(|(a, b)| (to(a.0, a.1), to(b.0, b.1))).filter(|&(a, _)| free(a)).collect();
    Some((rects, ups))
}

/// A mine's: scaffolding (a raised walkway, 10 ft, up a ladder) along a side wall, both, or across the dug half of the
/// face; the face's other half an ore vein; rails from the way in to the face, an ore cart on them.
pub(super) fn mine_layout(p: &mut Plan, h: &Hall, name: &str, rng: &mut Pcg32) -> Option<Stage> {
    let (l, w) = (h.l as isize, h.w as isize);
    let vein_left = rng.below(2) == 0;
    let mut items = Vec::new();
    let walkway = |p: &Plan, wall: u8, span: (isize, isize), items: &mut Vec<Option<Item>>| {
        let Some((rects, ups)) = shelf(p, h, wall, 2, span) else { return false };
        let Some(&(foot, top)) = ups.first() else { return false };
        items.extend(rects.iter().map(|&(d, a, dl, aw)| h.item("scaffold", d, a, dl, aw, None)));
        let (Some(k), Some(m)) = (h.sq(p, foot.0, foot.1), h.sq(p, top.0, top.1)) else { return false };
        items.push(h.item("climb_ladder", foot.0, foot.1, 1, 1, Some(toward(k, m, h.nx))));
        true
    };
    // (Along the side walls toward the face only, about half their length: the rest of the walls stay bare.)
    let run = (l / 2).max(4);
    let near_face = ((l - 1 - run).max(1), l - 2);
    let ok = match name {
        "side walkway" => walkway(p, rng.below(2) as u8, near_face, &mut items),
        "twin walkways" => walkway(p, 0, near_face, &mut items) && walkway(p, 1, near_face, &mut items),
        // (Across the dug half of the face.)
        "face gallery" => walkway(p, 2, if vein_left { (w / 2, w - 1) } else { (0, (w - 1) / 2) }, &mut items),
        _ => false,
    };
    if !ok || !lay(p, h, items) {
        return None;
    }
    // The face: the vein in its half of the far wall, rubble and tools where the other half is dug back.
    let half = |a: isize| if vein_left { a < w / 2 } else { a >= w / 2 };
    let mut face: Vec<(isize, isize)> = frame_squares(p, h)
        .into_iter()
        .filter(|&(d, a)| d >= l - 3 && half(a) && h.sq(p, d, a).is_some_and(|k| neighbours(k, h.nx, h.ny).count() < 4 || neighbours(k, h.nx, h.ny).any(|m| p.cells[m] < 0)))
        .collect();
    face.sort_by_key(|&(d, a)| (-d, a));
    let mut veins = 0;
    for (d, a) in face {
        if veins < 4 {
            veins += set(p, h, h.item("ore_vein", d, a, 1, 1, None), false) as usize;
        }
    }
    let dug = if vein_left { w * 3 / 4 } else { w / 4 };
    near(p, h, false, "rubble", l - 2, dug, 1, 1, None);
    near(p, h, false, "tools", l - 2, if vein_left { w / 4 } else { w * 3 / 4 }, 1, 1, None);
    let ar = (h.ea as isize).clamp(1, (w - 2).max(1));
    near(p, h, false, "ore_cart", l - 3, ar, 1, 1, None);
    let rails: Vec<Option<Item>> = (1..l - 1).filter(|&d| h.free(p, d, ar, 1, 1)).map(|d| h.item("rail", d, ar, 1, 1, None)).collect();
    if rails.len() >= 3 {
        lay(p, h, rails);
    }
    (veins >= 2).then(|| far_point(p, h))
}

/// A mine's dressing: tools and lanterns, the hoard and a chest by the face.
pub(super) fn mine_dress(p: &mut Plan, h: &Hall, s: Stage, rng: &mut Pcg32) {
    let (f, m) = (s.far(), s.mid());
    near(p, h, false, "hoard", f - 1, m, 1, 1, None);
    near(p, h, false, "chest", f - 1, m + 1, 1, 1, None);
    near(p, h, false, "tools", f - 2, m - 2, 1, 1, None);
    edge(p, h, "lantern", 2, rng);
    // (One prop against a wall; the rest of the dressing, crates, barrels, a cart, a short stack, goes round the
    // chamber by `spread`.)
    edge(p, h, "timber", 1, rng);
}
