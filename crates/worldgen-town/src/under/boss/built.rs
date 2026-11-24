//! A built site's boss chamber: layouts, centrepiece, hazards and the second way in.

use super::*;

/// The boss chamber of a built site (not a mine): maybe a second way in, a layout, the theme's centrepiece and its
/// dressing, maybe hazards.
pub(crate) fn built(p: &mut Plan, nx: usize, ny: usize, ri: i16, squares: &[usize], rng: &mut Pcg32) {
    let h = Hall::new(p, nx, ny, ri, squares);
    let theme = p.theme;
    let mut order: Vec<usize> = (0..LAYOUTS.len()).filter(|&i| h.l >= FIT[i].0 && h.w >= FIT[i].1).collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i as u32 + 1) as usize);
    }
    // (A flooded vault's strongroom stands on an island; a fire temple's altar on one in a lava moat.)
    if matches!(theme, "flooded_vault" | FIRE_TEMPLE) && let Some(m) = order.iter().position(|&i| LAYOUTS[i] == "moat") {
        order[..=m].rotate_right(1);
    }
    let fallback = far_stage(&h);
    let (stage, dressed) = choose(p, &h, order, rng, &|p, h, i, rng| layout(p, h, LAYOUTS[i], theme, rng), &|p, h, s, rng| centrepiece(p, h, s, theme, rng), fallback);
    spread(p, &h, stage, cover_kinds(theme, false), rng);
    let mut kinds = vec!["cracked floor", "pressure plate"];
    match theme {
        "dwarven_hall" | FORGE | FIRE_TEMPLE => kinds.push("steam vent"),
        "flooded_vault" | "sunken_temple" => kinds.push("flooded pit"),
        _ => {}
    }
    hazards(p, &h, &kinds, stage, dressed, rng);
}

/// The layouts in `order` tried in turn (`lay_out`, then `dress`), each kept if the chamber is all reached (`reached`),
/// else undone; last the `fallback` stage, dressed. The stage used and the index of the dressing's first item.
#[allow(clippy::type_complexity)]
pub(super) fn choose(
    p: &mut Plan,
    h: &Hall,
    order: Vec<usize>,
    rng: &mut Pcg32,
    lay_out: &dyn Fn(&mut Plan, &Hall, usize, &mut Pcg32) -> Option<Stage>,
    dress: &dyn Fn(&mut Plan, &Hall, Stage, &mut Pcg32),
    fallback: Stage,
) -> (Stage, usize) {
    for li in order.into_iter().map(Some).chain([None]) {
        let s = snap(p);
        h.clear.borrow_mut().clear();
        let stage = match li {
            Some(i) => lay_out(p, h, i, rng),
            None => Some(fallback),
        };
        if let Some(stage) = stage {
            let first = p.items.len();
            dress(p, h, stage, rng);
            if li.is_none() || reached(p, h) {
                return (stage, first);
            }
        }
        undo(p, s);
    }
    (fallback, p.items.len())
}

pub(super) fn layout(p: &mut Plan, h: &Hall, name: &str, theme: &str, rng: &mut Pcg32) -> Option<Stage> {
    let (l, w) = (h.l as isize, h.w as isize);
    let mid = w / 2;
    let stage = far_stage(h);
    // (The rows before the stage: what the cover and obstacles fill.)
    let front = stage.d0 - 1;
    match name {
        "pillar grid" => {
            // (Rows and columns two apart in a small hall, three in a big one, an aisle down the middle.)
            let (sd, sa) = (if h.l >= 10 { 3 } else { 2 }, if h.w >= 9 { 3 } else { 2 });
            let mut n = 0;
            let mut d = 1 + (h.l >= 10) as isize;
            while d <= front {
                let mut a = 1;
                while mid - a >= 0 {
                    for x in [mid - a, mid + a] {
                        n += set(p, h, h.item("pillar", d, x, 1, 1, None), false) as usize;
                    }
                    a += sa;
                }
                d += sd;
            }
            (n >= 4).then_some(stage)
        }
        "broken walls" => {
            // Stubs of wall across the way in, a row apart, from each side by turns, a gap in the middle.
            let len = ((w - 1) / 2).clamp(1, 3) as usize;
            let first = rng.below(2) as isize;
            let (mut n, mut i) = (0, 0);
            let gap = if front < 4 { 1 } else { 2 };
            let mut d = if front < 4 { 1 } else { 1 + rng.below(2) as isize };
            while d <= front {
                let left = (i + first) % 2 == 0;
                i += 1;
                let a = if left { mid - len as isize } else { mid + 1 };
                if set(p, h, h.item("wall_stub", d, a, 1, len, None), false) {
                    n += 1;
                    // (Its tumbled blocks on the far side, at its inner end.)
                    let end = if left { a + len as isize - 1 } else { a };
                    set(p, h, h.item("collapsed_wall", d + 1, end, 1, 1, Some(h.side)), false);
                }
                d += gap + (h.l >= 10 && rng.below(2) == 0) as isize;
            }
            // A fallen column, lying along the way in or across it.
            let along = h.l >= 8 && rng.below(2) == 0;
            let (dl, aw) = if along { (3, 1) } else { (1, 3.min(h.w - 2)) };
            let d = 1 + rng.below(front.max(1) as u32) as isize;
            near(p, h, false, "fallen_column", d, mid - aw as isize / 2, dl, aw, None);
            (n >= 2).then_some(stage)
        }
        "statue gallery" => {
            let mut n = 0;
            if h.l >= 9 && h.w >= 9 && rng.below(5) < 2 {
                // Ringing the stage.
                let (f, m) = (stage.far(), stage.mid());
                for (d, a) in [(f, m - 3), (f, m + 3), (f - 2, m - 3), (f - 2, m + 3), (f - 4, m - 2), (f - 4, m + 2)] {
                    n += set(p, h, h.item("statue", d, a, 1, 1, None), false) as usize;
                }
            } else {
                // Lining the nave, against the walls in a narrow hall.
                let off = if h.w >= 9 { 3 } else if h.w >= 7 { 2 } else { mid };
                let mut d = 1;
                while d <= front {
                    for a in [mid - off, mid + off] {
                        n += set(p, h, h.item("statue", d, a.clamp(0, w - 1), 1, 1, None), false) as usize;
                    }
                    d += 2;
                }
            }
            (n >= 4).then_some(stage)
        }
        "rubble ring" => {
            let r = ((front.min(w - 1) - 1) / 2).clamp(1, 3);
            let dc = (front + 1) / 2;
            let mut n = 0;
            let mut boulders = 0;
            for dd in -r..=r {
                for da in -r..=r {
                    if dd.abs().max(da.abs()) != r || rng.below(10) < 3 {
                        continue;
                    }
                    if boulders < 2 && rng.below(8) == 0 && set(p, h, h.item("boulder", dc + dd, mid + da, 1, 1, None), false) {
                        boulders += 1;
                    } else {
                        n += set(p, h, h.item("rubble", dc + dd, mid + da, 1, 1, None), false) as usize;
                    }
                }
            }
            (n >= 6).then_some(stage)
        }
        "pits" => {
            // Two or three across the approach, staggered, a way between them.
            let ps = if h.l >= 9 && h.w >= 7 { 2 } else { 1 };
            let at: &[isize] = if h.w >= 9 { &[-4, -1, 2] } else if ps == 2 { &[-3, 1] } else { &[-2, 0, 2] };
            // (Two rows short of the stage at least, so none opens beside the centrepiece.)
            let band = (front - ps as isize - 1).max(1);
            let mut n = 0;
            for (i, &a) in at.iter().enumerate() {
                let d = 1 + (band / 2 + if i % 2 == 1 { 1 } else { 0 } + rng.below(2) as isize - 1).clamp(0, band - 1);
                n += set(p, h, h.item("pit", d, (mid + a).clamp(0, w - ps as isize), ps, ps, None), false) as usize;
            }
            (n >= 2).then_some(stage)
        }
        "chasm and bridge" => {
            let dm = ((l - 2) / 2).clamp(2, l - 4);
            // (Across the room, row by row, crossed by one stone bridge in line with the way in, or two wide.)
            let bw = if h.w >= 10 { 2 } else { 1 };
            let ba = (h.ea as isize).clamp(1, w - 1 - bw as isize).max(0);
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
                    // (A forge's or fire temple's: a lava channel.)
                    let n = (b - a + 1) as usize;
                    items.push(if matches!(theme, FORGE | FIRE_TEMPLE) {
                        h.rect(d, a, 1, n).map(|(x, y, rw, rh)| Item { w: rw as u16, h: rh as u16, ..super::kit::lava(x, y) })
                    } else {
                        h.item("chasm", d, a, 1, n, None)
                    });
                    a = b + 1;
                }
            }
            items.push(h.item("stone_bridge", dm, ba, 2, bw, None));
            // (The bridge's ends on floor.)
            if h.sq(p, dm - 1, ba).is_none() || h.sq(p, dm + 2, ba).is_none() {
                return None;
            }
            lay(p, h, items).then_some(stage)
        }
        "moat" => {
            // An island of 3 (4 in a big hall) in a ring of water 2 wide (1 in a small hall), a floor row before it.
            let il = if h.l >= 13 && h.w >= 11 { 4 } else { 3 };
            // (Against the far wall, so no pocket of floor behind it is cut off; the ring 1 wide where 2 won't fit.
            // Else a row or two of floor behind it, joined round its sides: `reached` says.)
            let fits = |(mw, gap): (isize, isize)| {
                let (il, di, ai) = (il as isize, l - il as isize - mw - gap, (w - il as isize) / 2);
                let bridge = (di - mw, ai + il / 2);
                let ok = di - mw >= 2 && ai >= mw && h.free(p, di - mw, ai - mw, (il + 2 * mw) as usize, (il + 2 * mw) as usize) && h.sq(p, bridge.0 - 1, bridge.1).is_some();
                ok.then_some((mw, di, ai, bridge))
            };
            let (mw, di, ai, bridge) = [(2, 0), (1, 0), (2, 1), (1, 1), (1, 2)].into_iter().find_map(fits)?;
            let il = il as isize;
            let molten = matches!(theme, "dwarven_hall" | FORGE | FIRE_TEMPLE);
            let mut items = Vec::new();
            for d in di - mw..di + il + mw {
                for a in ai - mw..ai + il + mw {
                    if d >= di && d < di + il && a >= ai && a < ai + il {
                        continue;
                    }
                    if molten && (d, a) != bridge && (mw < 2 || (d, a) != (bridge.0 + 1, bridge.1)) {
                        items.push(h.rect(d, a, 1, 1).map(|(x, y, _, _)| super::kit::lava(x, y)));
                    } else if !molten {
                        items.push(h.item("water_deep", d, a, 1, 1, None));
                    }
                }
            }
            let span = if molten || rng.below(2) == 0 { "stone_bridge" } else { "stepping_stone" };
            items.push(h.item(span, bridge.0, bridge.1, mw as usize, 1, None));
            lay(p, h, items).then_some(Stage { d0: di, a0: ai, dl: il as usize, aw: il as usize, raised: false })
        }
        "raised platform" => {
            let pd = if h.l >= 10 { 3 } else { 2 };
            let (mut a0, mut a1) = (0, w - 1);
            while a1 - a0 + 1 >= 3 && !h.free(p, l - pd as isize, a0, pd, (a1 - a0 + 1) as usize) {
                a0 += 1;
                a1 -= 1;
            }
            let aw = (a1 - a0 + 1) as usize;
            if aw < 3 {
                return None;
            }
            let sd = l - pd as isize - 1;
            let sa = a0 + aw as isize / 2;
            let mut items = vec![h.item("platform", l - pd as isize, a0, pd, aw, None), h.item("steps", sd, sa, 1, 1, Some(h.far()))];
            // (A second flight at one end, now and then.)
            if aw >= 7 && rng.below(2) == 0 {
                let end = if rng.below(2) == 0 { a0 } else { a1 };
                items.push(h.item("steps", sd, end, 1, 1, Some(h.far())));
            }
            lay(p, h, items).then_some(Stage { d0: l - pd as isize, a0, dl: pd, aw, raised: true })
        }
        "balconies" => {
            let two = h.w >= 8 && rng.below(5) < 3;
            let bw = if h.w >= 10 { 2 } else { 1 };
            let first = rng.below(2) as usize;
            let mut items = Vec::new();
            let mut sides = Vec::new();
            for s in 0..if two { 2 } else { 1 } {
                let right = (first + s) % 2 == 1;
                let a = if right { w - bw as isize } else { 0 };
                let (mut d0, mut d1) = (2, l - 1);
                while d1 - d0 + 1 >= 4 && !h.free(p, d0, a, (d1 - d0 + 1) as usize, bw) {
                    if h.free(p, d0, a, 1, bw) { d1 -= 1 } else { d0 += 1 }
                }
                if d1 - d0 + 1 < 4 {
                    return None;
                }
                let up = if rng.below(5) < 3 { "climb_ladder" } else { "steps" };
                let back = Some(if right { h.walls().1 } else { h.walls().0 });
                items.push(h.item("balcony", d0, a, (d1 - d0 + 1) as usize, bw, back));
                items.push(h.item(up, d0 - 1, if right { w - 1 } else { 0 }, 1, 1, Some(h.far())));
                sides.push((right, d0, d1));
            }
            if !lay(p, h, items) {
                return None;
            }
            // Columns under the gallery's edge, holding it up, every second square (third in a long hall), before the stage.
            let gap = if h.l >= 12 { 3 } else { 2 };
            for &(right, d0, d1) in &sides {
                let a = if right { w - 1 - bw as isize } else { bw as isize };
                let mut d = d0 + 1;
                while d <= d1.min(front) {
                    set(p, h, h.item("pillar", d, a, 1, 1, None), false);
                    d += gap;
                }
            }
            // Cover in the hall between them: a broken wall or a statue a third and two thirds of the way in, on
            // alternate sides of the middle (where the hall is wide enough to walk round them).
            let (lo, hi) = (bw as isize + 1, w - 2 - bw as isize);
            if hi - lo >= 3 && front >= 4 {
                let left = rng.below(2) == 0;
                for (i, d) in [(front + 1) / 3, (2 * front + 1) / 3].into_iter().enumerate() {
                    let west = left == (i == 0);
                    let stub = rng.below(2) == 0;
                    let aw = if stub { 2 } else { 1 };
                    let a = if west { lo + rng.below(2) as isize } else { hi - aw as isize + 1 - rng.below(2) as isize };
                    let kind = if stub { "wall_stub" } else { "statue" };
                    let _ = [d, d + 1, d - 1].into_iter().filter(|&d| d >= 1 && d <= front).any(|d| set(p, h, h.item(kind, d, a, 1, aw, None), false));
                }
            }
            Some(stage)
        }
        "sunken arena" => {
            // The chamber's floor raised by `rise` round a fighting floor at the level's own height (heights stand
            // above the level's floor): the doorway's square stays down, steps up inside it, steps down into the
            // arena off to one side of the way in, and up again at its far side toward the stage.
            let rise = if rng.below(2) == 0 { 5.0 } else { 10.0 };
            let (ad0, ad1, aa0, aa1) = (3, l - 4, 2, w - 3);
            let (ed, ea) = (h.ed as isize, h.ea as isize);
            let up = (ed + 1, ea);
            if ed + 1 >= ad0 || h.sq(p, up.0, up.1).is_none_or(|k| p.taken[k]) {
                return None;
            }
            let side = if ea + 2 <= aa1 && (ea - 2 < aa0 || rng.below(2) == 0) { ea + 2 } else { ea - 2 };
            let down = (ad0 - 1, side.clamp(aa0, aa1));
            let back = (ad1 + 1, mid.clamp(aa0, aa1));
            let arena = |d: isize, a: isize| d >= ad0 && d <= ad1 && a >= aa0 && a <= aa1;
            // (A second way in stays down too, its own steps up inside it unless it opens on the arena.)
            let side_steps = h.door.and_then(|(k, iw)| {
                let (d, a) = h.frame(iw);
                (!arena(d as isize, a as isize)).then_some((k, iw, (d as isize, a as isize)))
            });
            if side_steps.is_some_and(|(_, _, at)| [up, down, back].contains(&at)) {
                return None;
            }
            let mut raise = vec![false; h.l * h.w];
            for d in 0..l {
                for a in 0..w {
                    let Some(k) = h.sq(p, d, a) else { continue };
                    if arena(d, a) || k == h.entry || h.door.is_some_and(|(door, _)| door == k) || [up, down, back].contains(&(d, a)) || side_steps.is_some_and(|s| s.2 == (d, a)) {
                        continue;
                    }
                    if p.taken[k] {
                        return None;
                    }
                    raise[d as usize * h.w + a as usize] = true;
                }
            }
            let mut items = Vec::new();
            for (d, a) in [up, down, back] {
                items.push(h.item("steps", d, a, 1, 1, Some(if (d, a) == up { h.far() } else { h.side })));
            }
            if let Some((k, iw, (d, a))) = side_steps {
                items.push(h.item("steps", d, a, 1, 1, Some(toward(k, iw, h.nx))));
            }
            // (The raised floor as rects: a run along a row, as deep as it goes.)
            for d in 0..h.l {
                let mut a = 0;
                while a < h.w {
                    if !raise[d * h.w + a] {
                        a += 1;
                        continue;
                    }
                    let mut b = a;
                    while b + 1 < h.w && raise[d * h.w + b + 1] {
                        b += 1;
                    }
                    let mut e = d;
                    while e + 1 < h.l && (a..=b).all(|x| raise[(e + 1) * h.w + x]) {
                        e += 1;
                    }
                    for dd in d..=e {
                        for x in a..=b {
                            raise[dd * h.w + x] = false;
                        }
                    }
                    // (A 10-ft surround is a gallery round the pit: the kinds carry their heights, as designs do.)
                    items.push(h.item(if rise > 5.0 { "balcony" } else { "platform" }, d as isize, a as isize, e - d + 1, b - a + 1, None));
                    a = b + 1;
                }
            }
            if !lay(p, h, items) {
                return None;
            }
            // Cover down in the pit, one to three pieces (by its size) standing apart, off its edge and its steps.
            let size = (ad1 - ad0 + 1) * (aa1 - aa0 + 1);
            let want = 1 + (size >= 20) as usize + (size >= 40) as usize;
            let mut spots: Vec<(isize, isize)> = (ad0 + 1..ad1).flat_map(|d| (aa0 + 1..aa1).map(move |a| (d, a))).collect();
            let mut put = 0;
            for _ in 0..want * 6 {
                if put >= want || spots.is_empty() {
                    break;
                }
                let i = rng.below(spots.len() as u32) as usize;
                let (d, a) = spots[i];
                let kind = ["pillar", "rubble", "weapon_rack", "boulder"][rng.below(4) as usize];
                let s = snap(p);
                if set(p, h, h.item(kind, d, a, 1, 1, None), false) && reached(p, h) {
                    put += 1;
                    spots.retain(|&(d2, a2)| (d2 - d).abs().max((a2 - a).abs()) >= 3);
                } else {
                    undo(p, s);
                    spots.swap_remove(i);
                }
            }
            Some(Stage { raised: true, ..stage })
        }
        _ => None,
    }
}

/// The theme's centrepiece on the stage, the hoard and chest by it, braziers and banners where they fit.
pub(super) fn centrepiece(p: &mut Plan, h: &Hall, s: Stage, theme: &str, rng: &mut Pcg32) {
    let (f, back, top) = (s.far(), Some(h.far()), s.raised);
    // (The middle of the far row's floor where the room narrows there (an apse, a round room), else the stage's.)
    let row: Vec<isize> = (s.a0..s.a0 + s.aw as isize).filter(|&a| h.sq(p, f, a).is_some()).collect();
    let m = match (row.first(), row.last()) {
        (Some(&a), Some(&b)) if b - a + 1 < s.aw as isize => (a + b) / 2,
        _ => s.mid(),
    };
    let (wa, wb) = h.walls();
    let wl = h.w as isize - 1;
    let put = |p: &mut Plan, kind: &'static str, d: isize, a: isize, dl: usize, aw: usize, back: Option<u8>| near(p, h, top, kind, d, a, dl, aw, back);
    // (What goes round it, braziers, candles, the loot: by its spot or a square off, else left out, so a small stage
    // or an island is not heaped.)
    let extra = |p: &mut Plan, kind: &'static str, d: isize, a: isize, back: Option<u8>| near_in(p, h, top, kind, d, a, 1, 1, back, 1);
    // (On a level stage: a dais (a bier) first, what it bears on it.)
    let dais = |p: &mut Plan| if top { Some((f - 1, m - 1)) } else { near(p, h, false, "dais", f - 1, m - 1, 2, 3, back) };
    let mut loot = true;
    match theme {
        // A throne on its dais; a dwarven hall's forge-throne with its forge and anvil off to one side, two squares
        // clear of the dais, the hoard on the other (no chest or braziers: the forge is the fire).
        "keep" | "dungeon" | "dwarven_hall" | FORGE => {
            let (dd, da) = dais(p).unwrap_or((f - 1, m - 1));
            near(p, h, true, "throne", dd + 1, da + 1, 1, 1, back);
            if theme != "keep" && theme != "dungeon" {
                let west = rng.below(2) == 0;
                let side = if west { (da - 3).max(0) } else { (da + 5).min(wl) };
                put(p, "forge", dd + 1, side, 1, 1, back);
                put(p, "anvil", dd, side, 1, 1, None);
                put(p, "hoard", dd + 1, if west { da + 4 } else { da - 2 }, 1, 1, None);
                loot = false;
            }
        }
        // An altar, and the ritual circle before it.
        "temple" | "sunken_temple" | "cult_sanctum" | FIRE_TEMPLE => {
            put(p, "altar", f, m - 1, 1, 2, back);
            // (Before the altar: on a raised stage 3 deep, on it.)
            put(p, "glyph", if top && s.dl >= 3 { f - 2 } else { f - 3 }, m - 1, 2, 2, None);
            for a in [m - 2, m + 2] {
                extra(p, "candles", f, a, None);
            }
        }
        // An arcane workshop: the workbench, bookshelves on the side walls, a focus before them (no circle).
        "wizard_lair" => {
            put(p, "arcane_workbench", f, m - 1, 1, 2, back);
            if !set(p, h, h.item("bookshelf", f - 2, 0, 2, 1, Some(wa)), top) {
                put(p, "bookshelf", f, m - 3, 1, 1, back);
            }
            if !set(p, h, h.item("bookshelf", f - 2, wl, 2, 1, Some(wb)), top) {
                put(p, "bookshelf", f, m + 3, 1, 1, back);
            }
            put(p, "arcane_focus", f - 2, m, 1, 1, None);
            extra(p, "candles", f, m + 2, None);
        }
        // A cell block: cages along the far wall, shackles on the walls, the warden's desk.
        "prison" => {
            let first = p.items.len();
            put(p, "cage", f - 1, 0, 2, 2, None);
            put(p, "cage", f - 1, wl - 1, 2, 2, None);
            if h.l >= 9 {
                put(p, "cage", h.l as isize / 2, 0, 2, 2, None);
            }
            // (A cage's door is on its right as drawn, its top its back: turned a quarter past the wall it stands
            // against, the far wall if none, the door faces the room.)
            let cages: Vec<usize> = (first..p.items.len()).filter(|&i| p.items[i].kind == "cage").collect();
            for i in cages {
                let it = p.items[i];
                let (x0, y0, x1, y1) = (it.x as isize, it.y as isize, (it.x + it.w) as isize, (it.y + it.h) as isize);
                let out = |x: isize, y: isize| x < 0 || y < 0 || x >= h.nx as isize || y >= h.ny as isize || p.cells[y as usize * h.nx + x as usize] != h.ri;
                let sides = [(x0..x1).filter(|&x| out(x, y0 - 1)).count(), (y0..y1).filter(|&y| out(x1, y)).count(), (x0..x1).filter(|&x| out(x, y1)).count(), (y0..y1).filter(|&y| out(x0 - 1, y)).count()];
                let wall = (0..4u8).filter(|&d| sides[d as usize] > 0).max_by_key(|&d| (sides[d as usize], d == h.far())).unwrap_or(h.far());
                p.items[i].back = Some((wall + 1) % 4);
            }
            edge(p, h, "chains", 3, rng);
            put(p, "desk", f - 3, m - 1, 1, 2, None);
        }
        // A great sarcophagus on its bier, candles at its head; an ossuary's skull piles round about.
        "crypt" | "tomb" | "ossuary" | "catacombs" => {
            let (dd, da) = dais(p).unwrap_or((f - 1, m - 1));
            near(p, h, true, "sarcophagus", dd, da + 1, 2, 1, back);
            for a in [da - 1, da + 3] {
                extra(p, "candles", dd, a, None);
            }
            if theme == "ossuary" {
                edge(p, h, "skulls", 4, rng);
            }
        }
        // The chief's seat and the loot heaped round it.
        "bandit_hideout" => {
            put(p, "chair_of_office", f, m, 1, 1, back);
            put(p, "hoard", f, m + 1, 1, 1, None);
            for (k, a) in [("chest", m - 2), ("chest", m + 2), ("crate", m - 3), ("sacks", m + 3)] {
                put(p, k, f, a, 1, 1, None);
            }
            put(p, "table", f - 2, m - 1, 1, 2, None);
            loot = false;
        }
        // Bone mounds round a totem.
        "goblin_warren" => {
            put(p, "totem", f - 1, m, 1, 1, None);
            put(p, "bone_mound", f - 1, m - 3, 2, 2, None);
            put(p, "bone_mound", f - 1, m + 2, 2, 2, None);
            put(p, "skulls", f, m - 1, 1, 1, None);
        }
        // The strongroom's hoard (on its island when the moat fits).
        "flooded_vault" => {
            put(p, "hoard", f - s.dl as isize / 2, m, 1, 1, None);
            put(p, "chest", f, m - 1, 1, 1, back);
            put(p, "chest", f, m + 1, 1, 1, back);
            loot = false;
        }
        // A beast's lair: its nest, gnawed bones and an adventurer who came before.
        "labyrinth" => {
            put(p, "nest", f - 1, m - 1, 2, 2, None);
            for _ in 0..5 {
                let (d, a) = (f - 1 - rng.below(4) as isize, m - 3 + rng.below(7) as isize);
                put(p, "bones", d, a, 1, 1, None);
            }
            put(p, "skeleton", f - 3, m + 2, 2, 1, None);
        }
        // A fallen commander's war table, racks on the walls (no throne: not a ruler's seat).
        FORTRESS => {
            put(p, "long_table", f - 2, m - 1, 1, 3, None);
            if !set(p, h, h.item("weapon_rack", f - 1, 0, 1, 1, Some(wa)), top) {
                extra(p, "weapon_rack", f, m - 3, back);
            }
            if !set(p, h, h.item("weapon_rack", f - 1, wl, 1, 1, Some(wb)), top) {
                extra(p, "weapon_rack", f, m + 3, back);
            }
        }
        _ => {}
    }
    if loot {
        // (The hoard somewhere: beside the centrepiece, else wherever near.)
        if extra(p, "hoard", f, m + 2, None).is_none() {
            put(p, "hoard", f, m + 2, 1, 1, None);
        }
        extra(p, "chest", f, m - 2, back);
    }
    if matches!(theme, "keep" | "dungeon" | "temple" | "sunken_temple" | "cult_sanctum" | FORTRESS | FIRE_TEMPLE) {
        for a in [m - 3, m + 3] {
            extra(p, "brazier", f - 1, a, None);
        }
    }
    match theme {
        "dwarven_hall" | FORGE => edge(p, h, "banner", 2, rng),
        "keep" | "dungeon" | "cult_sanctum" | "temple" | FORTRESS => edge(p, h, "banner", 2 + rng.below(2) as usize, rng),
        "prison" => edge(p, h, "sconce", 2, rng),
        "bandit_hideout" => edge(p, h, "lantern", 1, rng),
        _ => {}
    }
}

/// `n` of `kind` on the room's wall squares in its far half.
pub(super) fn edge(p: &mut Plan, h: &Hall, kind: &'static str, n: usize, rng: &mut Pcg32) {
    let (nx, ny) = (h.nx, h.ny);
    let ws: Vec<usize> = (0..nx * ny)
        .filter(|&k| p.cells[k] == h.ri && !p.taken[k] && h.frame(k).0 * 2 >= h.l && neighbours(k, nx, ny).any(|m| p.cells[m] != h.ri))
        .collect();
    let mut put = 0;
    for _ in 0..n * 6 {
        if put >= n || ws.is_empty() {
            break;
        }
        let k = ws[rng.below(ws.len() as u32) as usize];
        if set(p, h, Some(item(kind, k % nx, k / nx, 1, 1)), false) {
            put += 1;
        }
    }
}

/// About half the chambers: one or two of `kinds` (`PERILS`) on free floor in the approach: off the way in, the second
/// way in, every landing (`Hall::clear`), the stage and the centrepiece's squares and round them (the items from
/// `dressed` on), each kept only if the chamber is still all reached round them.
pub(super) fn hazards(p: &mut Plan, h: &Hall, kinds: &[&'static str], stage: Stage, dressed: usize, rng: &mut Pcg32) {
    if kinds.is_empty() || rng.below(2) != 0 {
        return;
    }
    let want = 1 + rng.below(2) as usize;
    let (nx, ny) = (h.nx, h.ny);
    let (ex, ey) = (h.entry % nx, h.entry / nx);
    let held = held(p, h, stage, dressed);
    let spots: Vec<usize> = (0..nx * ny)
        .filter(|&k| p.cells[k] == h.ri && !p.taken[k] && !h.kept(k) && !held[k] && !h.clear.borrow().contains(&k) && (k % nx).abs_diff(ex).max((k / nx).abs_diff(ey)) >= 2)
        .collect();
    let mut put = 0;
    for _ in 0..want * 8 {
        if put >= want || spots.is_empty() {
            break;
        }
        let k = spots[rng.below(spots.len() as u32) as usize];
        let kind = kinds[rng.below(kinds.len() as u32) as usize];
        if p.taken[k] {
            continue;
        }
        let s = snap(p);
        if p.place(peril(kind, k % nx, k / nx), Some(h.ri), ny) && reached(p, h) {
            put += 1;
        } else {
            undo(p, s);
        }
    }
}

/// About half the chambers: a second way in from a nearby room on the level, two times in three a secret door, else a
/// plain door (an open arch on a natural level). Through a wall the two rooms share, or a passage dug through a little
/// rock (that room's floor, rock either side), from a square of that room as far from the main way in as the walk goes
/// round, and (on a built level) one the same locks shut off as the square before the main door, so no key is skipped;
/// a locked main door's lock goes on it too. Never into a raised room. Its doorway square and the square inside it:
/// `Hall::door`.
pub(super) fn side_way(p: &mut Plan, h: &Hall, rng: &mut Pcg32) -> Option<(usize, usize)> {
    let secret = rng.below(3) < 2;
    let (nx, ny, ri) = (h.nx, h.ny, h.ri);
    let n = nx * ny;
    let built = !p.natural;
    let boss: Vec<bool> = p.cells.iter().map(|&c| c == ri).collect();
    let walk = p.distances(p.up, ny, Some(&boss));
    let out = neighbours(h.entry, nx, ny).filter(|&m| p.cells[m] >= 0 && p.cells[m] != ri && p.passable(h.entry, m) && walk[m] >= 0).min_by_key(|&m| (walk[m], m))?;
    let zone = built.then(|| super::kit::parts(p, ny, &|q| p.locks.contains_key(&q)).0);
    let lock = p.locks.get(&(out.min(h.entry), out.max(h.entry))).copied();
    let raise = p.rooms[ri as usize].raise;
    let dig = if built { 5 } else { 8 };
    let apart = |a: usize, b: usize| (a % nx).abs_diff(b % nx) + (a / nx).abs_diff(b / nx);
    // (Rock dug, the boss's square, the way out, the far room's square.)
    let mut cands: Vec<(usize, usize, usize, usize)> = Vec::new();
    for k in 0..n {
        if p.cells[k] != ri || p.taken[k] || apart(k, h.entry) < 4 {
            continue;
        }
        for dir in 0..4 {
            if step(k, (dir + 2) % 4, nx, ny).is_none_or(|iw| p.cells[iw] != ri || p.taken[iw]) {
                continue;
            }
            let mut q = k;
            for t in 0..=dig {
                let Some(s) = step(q, dir, nx, ny) else { break };
                q = s;
                let c = p.cells[q];
                if c < 0 {
                    // (Rock either side of the passage.)
                    if t == dig || [(dir + 1) % 4, (dir + 3) % 4].iter().any(|&sd| step(q, sd, nx, ny).is_none_or(|m| p.cells[m] >= 0)) {
                        break;
                    }
                    continue;
                }
                let room = &p.rooms[c as usize];
                let ok = c != ri
                    && room.kind != BOSS
                    && (built || t > 0)
                    && !p.taken[q]
                    && walk[q] >= 0
                    && apart(q, out) >= 4
                    && (room.raise - raise).abs() <= crate::interior::height::STEP_FT
                    && (t == 0 || room.shape == 0)
                    && !p.doors.contains(&(k.min(q), k.max(q)))
                    && zone.as_ref().is_none_or(|z| z[q] == z[out]);
                if ok {
                    cands.push((t, k, dir, q));
                }
                break;
            }
        }
    }
    // (Into the chamber's far half where one can, through the least rock.)
    if cands.iter().any(|c| h.frame(c.1).0 * 2 >= h.l) {
        cands.retain(|c| h.frame(c.1).0 * 2 >= h.l);
    }
    let least = cands.iter().map(|c| c.0).min()?;
    cands.retain(|c| c.0 <= least + 1);
    let (t, k, dir, q) = cands[rng.below(cands.len() as u32) as usize];
    let r = p.cells[q];
    let mut s = k;
    let mut first = q;
    for i in 0..t {
        s = step(s, dir, nx, ny)?;
        if i == 0 {
            first = s;
        }
        p.cells[s] = r;
        p.taken[s] = true;
    }
    let e = (k.min(first), k.max(first));
    p.door(k, first);
    if secret {
        p.secret.insert(e);
    } else if !built {
        p.gaps.insert(e);
    }
    if let Some(c) = lock.filter(|_| built) {
        p.locks.insert(e, c);
    }
    p.taken[k] = true;
    p.taken[q] = true;
    Some((k, step(k, (dir + 2) % 4, nx, ny)?))
}

/// A boss chamber on an island in its own lake (`themes::cave`): deep water round it, in another room.
pub(super) fn islet(p: &Plan, nx: usize, ny: usize, ri: i16, squares: &[usize]) -> bool {
    let deep = |m: usize| p.items.iter().any(|f| f.kind == "water_deep" && (f.x as usize..(f.x + f.w) as usize).contains(&(m % nx)) && (f.y as usize..(f.y + f.h) as usize).contains(&(m / nx)));
    squares.iter().any(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] >= 0 && p.cells[m] != ri && deep(m)))
}

/// The level's second way into its boss chamber (`side_way`; about half the chambers get one, none to an island), picked
/// before the level's rooms are furnished so its doorway squares are still free: `Plan::side_door`.
pub(crate) fn side_door(p: &mut Plan, nx: usize, ny: usize, rng: &mut Pcg32) {
    let Some(ri) = p.rooms.iter().position(|r| r.kind == BOSS).map(|r| r as i16) else { return };
    let squares: Vec<usize> = (0..nx * ny).filter(|&k| p.cells[k] == ri).collect();
    // (Tried in four chambers of five: about three in five have room for one.)
    if squares.is_empty() || rng.below(5) == 0 || islet(p, nx, ny, ri, &squares) {
        return;
    }
    let h = Hall::new(p, nx, ny, ri, &squares);
    let entry = h.entry;
    p.side_door = side_way(p, &h, rng).map(|(k, iw)| (k, iw, entry));
}

/// The stage round the chamber's farthest square from its way in (walking its floor): its far row that square's.
pub(super) fn far_point(p: &Plan, h: &Hall) -> Stage {
    let (nx, ny) = (h.nx, h.ny);
    let mut dist = vec![-1i32; nx * ny];
    dist[h.entry] = 0;
    let mut q = std::collections::VecDeque::from([h.entry]);
    let mut far = h.entry;
    while let Some(k) = q.pop_front() {
        if dist[k] > dist[far] || dist[k] == dist[far] && k < far {
            far = k;
        }
        for m in neighbours(k, nx, ny) {
            if dist[m] < 0 && p.cells[m] == h.ri {
                dist[m] = dist[k] + 1;
                q.push_back(m);
            }
        }
    }
    let (d, a) = h.frame(far);
    Stage { d0: d as isize - 1, a0: a as isize - 2, dl: 2, aw: 5, raised: false }
}

