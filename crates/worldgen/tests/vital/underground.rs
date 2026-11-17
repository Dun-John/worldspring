//! Underground sites: the guarantee, paired links, cellars, designed sites, climbing,
//! the site index and cellar entrances.

use super::*;

/// M6: every underground site opens where its entrance is on the surface, has at least two
/// levels joined by a way down and a way up on the same square, can be walked end to end
/// (around blocking furniture), and ends in one boss chamber on its deepest level that is a
/// dead end (nothing else on the level is reached through it) apart from at most one side way in
/// (a door, secret or not, or an arch on a natural level). Sewers match the streets above:
/// their tunnels lie under paved streets, and the streets over dry ground have sewer beneath.
/// Each site generates within 300 ms.
#[test]
fn underground_guarantee() {
    use worldgen::interior::{self, Level};
    use worldgen::town::{self, geom};
    use worldgen::under::{SiteSize, THEMES};
    let base = World::from_json(&world_json(1)).unwrap();
    let mut t0 = worldgen::t0::T0::generate(&base);
    // Every theme at every size, as created sites (bare entrances on dry land clear of other
    // layouts); the huge ones as deep as a site goes, the rest 1 to 6 levels.
    let sea = base.params().sea_level_ft;
    let g = &base.geom;
    let spots: Vec<[f64; 2]> = (0..90 * 60)
        .map(|k| [(k % 90) as f64 / 90.0 * g.map_w_ft + 1_000.0, (k / 90) as f64 / 60.0 * g.map_h_ft + 1_000.0])
        .filter(|p| {
            let h = t0.sample(p[0], p[1], 20.0);
            h > sea + 20.0 && (t0.sample_water(p[0], p[1]) as f64) < h && town::layouts_near(&base, &t0, [p[0] - 300.0, p[1] - 300.0, p[0] + 300.0, p[1] + 300.0]).is_empty()
        })
        .collect();
    let mut file = base.file.clone();
    for (i, (t, size)) in THEMES.iter().flat_map(|t| SiteSize::ALL.map(|s| (t, s))).enumerate() {
        let p = spots[i * spots.len() / (THEMES.len() * 4)];
        let levels = if size == SiteSize::Huge { worldgen::under::MAX_LEVELS } else { 1 + (i % 6) as u8 };
        file.edits.created.push(worldgen::world::Created {
            id: format!("c:{i}"),
            kind: "entrance".into(),
            x: p[0],
            y: p[1],
            name: format!("{} {}", t.key, size.name()),
            under: Some(t.kind.key().into()),
            size: Some(size.name().into()),
            levels: Some(levels),
            theme: Some(t.key.into()),
            ..Default::default()
        });
    }
    let world = World::new(file).unwrap();
    t0.apply_edits(&world);
    let t0 = t0;
    let (mut worst_ms, mut kinds) = (0.0f64, std::collections::BTreeMap::<&str, usize>::new());
    let (mut designed, mut shaped_rooms) = (0, 0);
    // Squares reached from `start` over floor, stepping between rooms only through doors (any
    // step on a natural level), never onto `blocked`.
    let flood = |lv: &Level, nx: usize, ny: usize, start: usize, blocked: &dyn Fn(usize) -> bool| {
        let door = |a: usize, b: usize| {
            let (ia, ja, ib, jb) = ((a % nx) as f32, (a / nx) as f32, (b % nx) as f32, (b / nx) as f32);
            // The shared edge of two neighbouring squares.
            let (p, q) = if ja == jb { ([ia.max(ib), ja], [ia.max(ib), ja + 1.0]) } else { ([ia, ja.max(jb)], [ia + 1.0, ja.max(jb)]) };
            lv.doors.iter().any(|d| (d.a == p && d.b == q) || (d.a == q && d.b == p))
        };
        let mut seen = vec![false; nx * ny];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for q in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten() {
                if seen[q] || lv.cells[q] < 0 || blocked(q) {
                    continue;
                }
                if !lv.natural && lv.cells[q] != lv.cells[k] && !door(k, q) {
                    continue;
                }
                seen[q] = true;
                stack.push(q);
            }
        }
        seen
    };
    // Every way to another site leads to one that has a way straight back.
    let links_back = |tag: &str, it: &interior::Interior| {
        for lv in &it.levels {
            for k in &lv.links {
                let there = interior::generate_id(&world, &t0, &k.to).unwrap_or_else(|| panic!("{tag}: a way to {} that doesn't exist", k.to));
                assert!(there.levels.iter().any(|l2| l2.links.iter().any(|b| b.to == it.id)), "{tag}: {} has no way back", k.to);
            }
        }
    };
    let check = |tag: &str, it: &interior::Interior, l: &town::Layout, entrance: Option<[f64; 2]>| {
        links_back(tag, it);
        let (nx, ny) = (it.nx, it.ny);
        let n = it.levels.len();
        assert!(n >= 1, "{tag}: no levels");
        assert_eq!(it.entry_level, n - 1, "{tag}: the way in is not on the top level");
        let at = |lv: &Level, kind: &str| lv.furniture.iter().filter(|f| f.kind == kind).map(|f| f.y as usize * nx + f.x as usize).collect::<Vec<_>>();
        // The way in sits on the surface entrance (a sewer section without a grate is entered
        // from its neighbours: walk it from any square).
        let exit = at(&it.levels[n - 1], "exit");
        let mut start = match entrance {
            Some(e_at) => {
                // (A sewer section has a way up at each of its grates.)
                assert!(exit.len() == 1 || (it.function == "sewer" && !exit.is_empty()), "{tag}: {} ways in", exit.len());
                let near = |k: usize| geom::dist(interior::to_world(it, (k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5), e_at);
                let k = *exit.iter().min_by(|&&a, &&b| near(a).total_cmp(&near(b))).unwrap();
                assert!(near(k) < 8.0, "{tag}: the way in is {:.0} ft from the entrance", near(k));
                k
            }
            None => {
                assert!(exit.is_empty(), "{tag}: a way in with no entrance above");
                // From the way up (deep dungeons), else any square no furniture stands on.
                let top = &it.levels[n - 1];
                let held = |q: usize| top.furniture.iter().any(|f| f.blocks_move && (f.x as usize..(f.x + f.w) as usize).contains(&(q % nx)) && (f.y as usize..(f.y + f.h) as usize).contains(&(q / nx)));
                at(top, "up").first().copied().or_else(|| (0..nx * ny).find(|&q| top.cells[q] >= 0 && !held(q))).unwrap_or_else(|| panic!("{tag}: no floor"))
            }
        };
        // Levels bottom to top: each but the deepest has a way down onto the way up below.
        for li2 in (0..n).rev() {
            let lv = &it.levels[li2];
            let down = at(lv, "down");
            if li2 == 0 {
                assert!(down.is_empty(), "{tag}: a way down from the deepest level");
            } else {
                assert_eq!(down.len(), 1, "{tag} {}: {} ways down", lv.name, down.len());
                assert_eq!(at(&it.levels[li2 - 1], "up"), down, "{tag} {}: the way down and the way up below differ", lv.name);
            }
            // Everything is reached, around blocking furniture too.
            // (Deep water and chasms block but where a rope bridge or stepping stones cross them.)
            let blocks: Vec<bool> = worldgen::under::blocked_squares(&lv.furniture, nx, ny);
            assert!(!blocks[start] && lv.cells[start] >= 0, "{tag} {}: the way in is not open floor", lv.name);
            for f in &lv.furniture {
                for j in f.y..f.y + f.h {
                    for i in f.x..f.x + f.w {
                        assert!(lv.cells[j as usize * nx + i as usize] >= 0, "{tag} {}: {} in the rock", lv.name, f.name);
                    }
                }
            }
            let seen = flood(lv, nx, ny, start, &|q| blocks[q]);
            let lost = (0..nx * ny).filter(|&q| lv.cells[q] >= 0 && !blocks[q] && !seen[q]).count();
            assert_eq!(lost, 0, "{tag} {}: {lost} squares unreachable", lv.name);
            r5_no_climb(tag, lv, nx, ny, start);
            if li2 > 0 {
                start = at(lv, "down")[0];
            }
        }
        // One boss chamber, on the deepest level, a dead end apart from one side way at most.
        let boss: Vec<(usize, usize)> = it.levels.iter().enumerate().flat_map(|(li2, lv)| lv.rooms.iter().enumerate().filter(|(_, r)| r.kind == worldgen::under::BOSS && r.squares > 0).map(move |(ri, _)| (li2, ri))).collect();
        assert_eq!(boss.len(), 1, "{tag}: {} boss chambers", boss.len());
        let (bl, br) = boss[0];
        assert_eq!(bl, 0, "{tag}: the boss chamber is not on the deepest level");
        let deep = &it.levels[0];
        // (From the way up; a site of one level from its way in.)
        let up = at(deep, "up").first().or(at(deep, "exit").first()).copied().unwrap_or_else(|| panic!("{tag}: no way onto the deepest level"));
        let seen = flood(deep, nx, ny, up, &|q| deep.cells[q] == br as i16);
        let cut = (0..nx * ny).filter(|&q| deep.cells[q] >= 0 && deep.cells[q] != br as i16 && !seen[q]).count();
        assert_eq!(cut, 0, "{tag}: {cut} squares are reached only through the boss chamber");
        // (Its doors: a built level's way in and one more; a natural level's way in is open floor, so one.)
        let ways = deep.doors.iter().filter(|d| d.rooms.contains(&(br as i16))).count();
        assert!(ways <= if deep.natural { 1 } else { 2 }, "{tag}: {ways} doors into the boss chamber");
        // Locked doors open in order: each key reached without its lock, every square in the end.
        let order = worldgen::under::design::key_order(it);
        assert!(order.is_empty(), "{tag}: keys out of order: {order:?}");
        if it.function == "sewer" {
            let top = &it.levels[n - 1];
            let streets = worldgen::under::street_segments(l);
            let reach = worldgen::under::SEWER_REACH_FT + 0.01;
            for q in 0..nx * ny {
                let r = top.cells[q];
                if r >= 0 && top.rooms[r as usize].kind == "sewer tunnel" {
                    let c = interior::to_world(it, (q % nx) as f64 + 0.5, (q / nx) as f64 + 0.5);
                    assert!(streets.iter().any(|&(a, b, _)| geom::seg_dist(c, a, b) <= reach), "{tag}: a sewer tunnel under no street at {c:?}");
                }
            }
            let wet = |p: [f64; 2]| t0.sample_water(p[0], p[1]) as f64 > t0.sample(p[0], p[1], 5.0);
            let (mut points, mut missed) = (0, 0);
            for &(a, b, _) in &streets {
                let steps = (geom::dist(a, b) / 2.5).ceil().max(1.0) as usize;
                for s in 0..=steps {
                    let p = geom::lerp(a, b, s as f64 / steps as f64);
                    let g = interior::to_grid(it, p);
                    // The whole section, edges included: neighbouring sections line up only
                    // if neither drops its edge runs.
                    if g[0] < 0.0 || g[1] < 0.0 || g[0] >= nx as f64 || g[1] >= ny as f64 || wet(p) {
                        continue;
                    }
                    points += 1;
                    if top.cells[g[1] as usize * nx + g[0] as usize] < 0 {
                        missed += 1;
                    }
                }
            }
            // (One in fifty, or one in a section with fewer: a corner square cut off by water.)
            assert!(missed * 50 <= points.max(50), "{tag}: {missed} of {points} street points have no sewer beneath");
        }
    };
    for li in 0..town::layout_count(&t0) {
        let l = town::layout(&world, &t0, li);
        for (k, e) in l.entrances.iter().enumerate() {
            let tag = format!("u:{li}:{k} ({})", e.kind.name());
            let t = std::time::Instant::now();
            let it = interior::generate_id(&world, &t0, &format!("u:{li}:{k}")).unwrap_or_else(|| panic!("{tag}: no site"));
            worst_ms = worst_ms.max(t.elapsed().as_secs_f64() * 1e3);
            *kinds.entry(e.kind.name()).or_default() += 1;
            check(&tag, &it, &l, Some(e.at));
            // The designer holds sites to the same rules, and copies one exactly (as a design,
            // and through the text form agents write).
            if e.kind != worldgen::under::UnderKind::Sewer {
                use worldgen::under::design;
                let d = design::SiteDesign::from_interior(&it);
                let problems = design::check(&it, Some(d.entry));
                assert!(problems.is_empty(), "{tag}: the designer finds {problems:?}");
                let mut again = d.build(&it.id, it.settlement, it.building, &Default::default());
                worldgen::sitelinks::dress(&world, &t0, li, &mut again);
                assert_eq!(serde_json::to_string(&again).unwrap(), serde_json::to_string(&it).unwrap(), "{tag}: its design builds something else");
                let text = design::to_text(&d, &|_, _| None).unwrap_or_else(|e| panic!("{tag}: {e}"));
                // (Over the same site emptied: the text says it all.)
                let mut blank = d.clone();
                for lv in &mut blank.levels {
                    *lv = design::DesignLevel { elevation_ft: lv.elevation_ft, natural: lv.natural, cells: vec![-1, (it.nx * it.ny) as i32], ..Default::default() };
                }
                let (back, names) = design::from_text(&text, &blank).unwrap_or_else(|e| panic!("{tag}: {e}\n{text}"));
                // (Rooms left with no squares are not in the text: chambers, after the last one used.)
                let mut same = d.clone();
                for lv in &mut same.levels {
                    let cells = design::decode(&lv.cells, it.nx * it.ny).unwrap();
                    let used = |r: usize| cells.contains(&(r as i16));
                    while lv.rooms.len() > 0 && !used(lv.rooms.len() - 1) {
                        lv.rooms.pop();
                    }
                    for (r, room) in lv.rooms.iter_mut().enumerate() {
                        if !used(r) {
                            *room = design::DesignRoom { kind: design::CHAMBER.into(), ..Default::default() };
                        }
                    }
                }
                assert!(names.is_empty() && back == same, "{tag}: its text reads back as something else");
                designed += 1;
                // Shaped rooms come back as well: every third room given a shape in turn (its squares
                // fitted to it; one whose squares can't be is built square), as built and as text.
                use worldgen::interior::shape;
                let mut shaped = d.clone();
                let mut turn = 0;
                for li2 in 0..shaped.levels.len() {
                    for ri in (0..shaped.levels[li2].rooms.len()).step_by(3) {
                        let code = [shape::ROUND, shape::OCTAGON, shape::APSE_N, shape::APSE_E, shape::APSE_S, shape::APSE_W, shape::CHAMFERED, shape::CAVERN][turn % 8];
                        turn += 1;
                        // (Refused where another room or a rule is in the way: nothing changes; taken,
                        // the site keeps every rule.)
                        // (Checked the cheap way round: the change alone, then held to the rules.)
                        let mut next = shaped.clone();
                        if next.set_shape(li2, ri, code).is_err() {
                            assert!(next == shaped, "{tag}: a shape refused, yet the design changed");
                            continue;
                        }
                        if next.problems(&world, &t0, &it.id).unwrap().1.iter().all(|p| !p.blocking) {
                            shaped = next;
                        } else {
                            let before = shaped.clone();
                            assert!(shaped.set_shape_checked(&world, &t0, &it.id, li2, ri, code).is_err() && shaped == before, "{tag}: a shape that breaks a rule is taken");
                        }
                    }
                }
                let built = shaped.build(&it.id, it.settlement, it.building, &Default::default());
                let mut kept = shaped.clone();
                for (lv, b) in kept.levels.iter_mut().zip(&built.levels) {
                    for (ri, (r, br)) in lv.rooms.iter_mut().zip(&b.rooms).enumerate() {
                        if br.shape != r.shape {
                            assert!(br.squares == 0 || !shape::fits(&b.cells, it.nx, ri as i16, r.shape), "{tag}: a room fitting its shape built square");
                            r.shape = 0;
                        } else if r.shape != 0 {
                            shaped_rooms += 1;
                        }
                    }
                }
                // (Doors in any order: a doorway moved round a shape's corner comes last in the design.)
                let doors_sorted = |mut d: design::SiteDesign| {
                    d.levels.iter_mut().for_each(|lv| lv.doors.sort());
                    d
                };
                let (fa, fb) = (doors_sorted(design::SiteDesign::from_interior(&built)), doors_sorted(kept.clone()));
                assert!(fa == fb, "{tag}: its shaped rooms don't come back from what it builds");
                let text = design::to_text(&kept, &|_, _| None).unwrap_or_else(|e| panic!("{tag}: {e}"));
                assert!(design::from_text(&text, &kept).unwrap_or_else(|e| panic!("{tag}: {e}\n{text}")).0 == kept, "{tag}: its shaped rooms' text reads back as something else");
            }
            // A created site is what it was asked to be.
            if let Some(c) = li.checked_sub(t0.settlements.len() + t0.base_pois).and_then(|c| world.file.edits.created.get(c)) {
                assert_eq!(it.levels.len(), c.levels.unwrap() as usize, "{tag} ({}): levels", c.name);
                assert_eq!(it.theme, c.theme.as_deref(), "{tag} ({}): theme", c.name);
            }
        }
        // Every section of a city's sewers, grate or not.
        let streets = worldgen::under::street_segments(&l);
        if !l.entrances.iter().any(|e| e.kind == worldgen::under::UnderKind::Sewer) {
            continue;
        }
        let s = worldgen::under::SEWER_SECTION_FT;
        let sec = |v: f64| (v / s).floor() as i64;
        let (x0, y0) = streets.iter().fold((i64::MAX, i64::MAX), |m, (a, b, _)| (m.0.min(sec(a[0].min(b[0]))), m.1.min(sec(a[1].min(b[1])))));
        let (x1, y1) = streets.iter().fold((i64::MIN, i64::MIN), |m, (a, b, _)| (m.0.max(sec(a[0].max(b[0]))), m.1.max(sec(a[1].max(b[1])))));
        let mut n_sec = 0;
        for sy in y0..=y1 {
            for sx in x0..=x1 {
                let id = format!("w:{li}:{sx}:{sy}");
                let t = std::time::Instant::now();
                let Some(it) = interior::generate_id(&world, &t0, &id) else { continue };
                worst_ms = worst_ms.max(t.elapsed().as_secs_f64() * 1e3);
                let grate = l.entrances.iter().find(|e| e.kind == worldgen::under::UnderKind::Sewer && worldgen::under::sewer_section(e.at) == [sx as f64 * s, sy as f64 * s]).map(|e| e.at);
                check(&format!("{id} (sewer section)"), &it, &l, grate);
                n_sec += 1;
            }
        }
        assert!(n_sec > 0, "layout {li}: no sewer sections");
        // Keeps' deep dungeons, and the cellars that open into the sewers.
        for (k, b) in l.buildings.iter().enumerate() {
            let bi = b.id;
            if interior::has_deep_dungeon(&l.buildings[k]) {
                let id = format!("k:{li}:{bi}");
                let t = std::time::Instant::now();
                let it = interior::generate_id(&world, &t0, &id).unwrap_or_else(|| panic!("{id}: no deep dungeons"));
                worst_ms = worst_ms.max(t.elapsed().as_secs_f64() * 1e3);
                *kinds.entry("deep dungeons").or_default() += 1;
                check(&format!("{id} (deep dungeons)"), &it, &l, None);
            }
            if worldgen::under::sewer_link_of(&t0, &l, &l.buildings[k]).is_some() {
                let id = format!("b:{li}:{bi}");
                let it = interior::generate_id(&world, &t0, &id).unwrap();
                assert!(it.levels[0].links.iter().any(|k| k.to.starts_with("w:")), "{id}: no way into the sewers");
                links_back(&id, &it);
                *kinds.entry("sewer cellars").or_default() += 1;
            }
        }
    }
    for kind in ["dungeon", "crypt", "cave", "mine", "lava tube", "sewer", "catacombs", "deep dungeons", "sewer cellars"] {
        assert!(kinds.get(kind).copied().unwrap_or(0) > 0, "no {kind} in the world: {kinds:?}");
    }
    assert!(worst_ms < 300.0, "slowest underground site took {worst_ms:.1} ms");
    assert!(designed > 0, "no site to design");
    assert!(shaped_rooms > 100, "too few shaped rooms built ({shaped_rooms})");
    designed_site(&world, &t0);
    designed_cellars(&world, &t0);
    paired_links(&world, &t0);
}

/// Links put down by hand (`Edits::links`): between a site's deepest level and a sewer section,
/// the section and a cellar, and two levels of one site; each end shows on its own map as a
/// `link` item on floor, reached from the level's way in, its way naming the far end; places no
/// link names build as before.
fn paired_links(world: &World, t0: &worldgen::t0::T0) {
    use worldgen::interior::{self, Interior, Level};
    use worldgen::links::{self, End, Link};
    use worldgen::town;
    use worldgen::under::UnderKind;
    // (A created site of two levels: `underground_guarantee`'s second.)
    let site = format!("u:{}:0", t0.settlements.len() + t0.base_pois + 1);
    let (sewer, cellar, other) = (0..t0.settlements.len())
        .find_map(|li| {
            let l = town::layout(world, t0, li);
            let e = l.entrances.iter().find(|e| e.kind == UnderKind::Sewer)?;
            let q = worldgen::under::sewer_section(e.at);
            let s = worldgen::under::SEWER_SECTION_FT;
            let b = l.buildings.iter().find(|b| worldgen::under::sewer_link_of(t0, &l, b).is_some())?;
            let other = l.entrances.iter().find(|e| e.kind != UnderKind::Sewer).map(|e| format!("u:{li}:{}", e.id))?;
            Some((format!("w:{li}:{}:{}", (q[0] / s) as i64, (q[1] / s) as i64), format!("b:{li}:{}", b.id), other))
        })
        .expect("a city with sewers, a cellar on them and a site underground");
    let built = |w: &World, id: &str| interior::generate_id(w, t0, id).unwrap_or_else(|| panic!("{id}: no interior"));
    // A free floor square of a level (one with furniture that may move on it, if there is one).
    let spot = |id: &str, li: usize, avoid: &[(u16, u16)]| -> End {
        let it = built(world, id);
        let ok = |x: u16, y: u16| !avoid.contains(&(x, y)) && links::spot_problem(&it, id, li, x, y).is_none();
        let lv = &it.levels[li];
        let under = lv.furniture.iter().filter(|f| !f.blocks_move && f.w == 1 && f.h == 1).map(|f| (f.x, f.y)).find(|&(x, y)| ok(x, y));
        let any = (0..it.nx * it.ny).map(|k| ((k % it.nx) as u16, (k / it.nx) as u16)).find(|&(x, y)| ok(x, y));
        let (x, y) = under.or(any).unwrap_or_else(|| panic!("{id} level {li}: no square for a link"));
        End { level: links::level_id(id, li), x, y }
    };
    let top = built(world, &site).levels.len() - 1;
    assert!(top > 0, "{site}: one level");
    let cellar_li = built(world, &cellar).levels.iter().position(|l| l.z < 0).expect("a cellar");
    let (deep, up_top) = (spot(&site, 0, &[]), spot(&site, top, &[]));
    let (deep2, up_top2) = (spot(&site, 0, &[(deep.x, deep.y)]), spot(&site, top, &[(up_top.x, up_top.y)]));
    let in_sewer = spot(&sewer, 0, &[]);
    let in_sewer2 = spot(&sewer, 0, &[(in_sewer.x, in_sewer.y)]);
    let mut file = world.file.clone();
    let mut put = |k: &str, a: &End, b: &End, way: &str| {
        file.edits.links.insert(k.into(), Link { a: a.clone(), b: b.clone(), way: way.into(), name: None });
    };
    put("j:a", &deep, &in_sewer, "ladder");
    put("j:b", &in_sewer2, &spot(&cellar, cellar_li, &[]), "trapdoor");
    put("j:c", &up_top, &deep2, "passage");
    // (An end on j:a's square: the second link there is not shown.)
    put("j:d", &up_top2, &deep, "rope");
    let w2 = World::new(file).unwrap();
    let edits = &w2.file.edits;
    let blocked_ok = ["j:d"];
    // Squares reached from `start` over floor, between rooms only through doors (any step on a natural level), round blocking furniture.
    let reached = |it: &Interior, lv: &Level, start: usize| {
        let (nx, ny) = (it.nx, it.ny);
        let blocks = worldgen::under::blocked_squares(&lv.furniture, nx, ny);
        let door = |a: usize, b: usize| {
            let (ia, ja, ib, jb) = ((a % nx) as f32, (a / nx) as f32, (b % nx) as f32, (b / nx) as f32);
            let (p, q) = if ja == jb { ([ia.max(ib), ja], [ia.max(ib), ja + 1.0]) } else { ([ia, ja.max(jb)], [ia + 1.0, ja.max(jb)]) };
            lv.doors.iter().any(|d| (d.a == p && d.b == q) || (d.a == q && d.b == p))
        };
        let mut seen = vec![false; nx * ny];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for q in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten() {
                if !seen[q] && lv.cells[q] >= 0 && !blocks[q] && (lv.natural || lv.cells[q] == lv.cells[k] || door(k, q)) {
                    seen[q] = true;
                    stack.push(q);
                }
            }
        }
        seen
    };
    for (key, l) in &edits.links {
        let problems = links::problems(&w2, t0, key, l);
        if blocked_ok.contains(&key.as_str()) {
            assert!(problems.len() == 1 && problems[0].contains("already leads elsewhere"), "{key}: {problems:?}");
            continue;
        }
        assert!(problems.is_empty(), "{key}: {problems:?}");
        for (here, there) in [(&l.a, &l.b), (&l.b, &l.a)] {
            let (id, li) = links::site_of(&here.level).unwrap();
            let it = built(&w2, id);
            let lv = &it.levels[li];
            let (x, y) = (here.x, here.y);
            let item: Vec<_> = lv.furniture.iter().filter(|f| x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h).collect();
            assert!(item.len() == 1 && item[0].kind == links::KIND && !item[0].blocks_move, "{key} at {}: {:?} on its square", here.level, item.iter().map(|f| f.kind).collect::<Vec<_>>());
            let way = lv.links.iter().find(|w| w.id == *key && (w.x, w.y) == (x, y)).unwrap_or_else(|| panic!("{key}: no way at {}", here.level));
            let far = way.far.as_ref().unwrap();
            assert_eq!((way.to.as_str(), links::level_id(&way.to, far.level), far.x, far.y), (links::site_of(&there.level).unwrap().0, there.level.clone(), there.x, there.y), "{key}: the way leads elsewhere");
            // (From the way onto the level: the way in or up, else a cellar's stairs.)
            let at = |kind: &str| lv.furniture.iter().find(|f| f.kind == kind).map(|f| f.y as usize * it.nx + f.x as usize);
            let start = at("exit").or_else(|| at("up")).unwrap_or(it.stairs[1] * it.nx + it.stairs[0]);
            assert!(reached(&it, lv, start)[y as usize * it.nx + x as usize], "{key} at {}: not reached from the way in", here.level);
        }
    }
    // A place no link names builds as before.
    assert_eq!(serde_json::to_string(&built(&w2, &other)).unwrap(), serde_json::to_string(&built(world, &other)).unwrap(), "{other}: changed by links elsewhere");
}

/// Cellars designed where they lead on: a large keep's and a cellar on the sewers dug two deep
/// keep their ways on (the stairs down to the deep dungeons, the trapdoor to the sewers) on the
/// deepest, each with its way back, the deep dungeons below it; without a cellar, refused.
fn designed_cellars(world: &World, t0: &worldgen::t0::T0) {
    use worldgen::interior::{self, design as bd};
    use worldgen::town;
    let (li, keep, sewer) = (0..t0.settlements.len())
        .find_map(|li| {
            let l = town::layout(world, t0, li);
            let keep = l.buildings.iter().find(|b| interior::has_deep_dungeon(b))?.id;
            let sewer = l.buildings.iter().find(|b| worldgen::under::sewer_link_of(t0, &l, b).is_some() && !interior::has_deep_dungeon(b))?.id;
            Some((li, keep, sewer))
        })
        .expect("a city with a large keep and a cellar on the sewers");
    let mut file = world.file.clone();
    for bi in [keep, sewer] {
        let id = format!("b:{li}:{bi}");
        let (d, _) = bd::design_of(world, t0, &id, false).unwrap();
        let mut none = d.clone();
        assert!(bd::set_cellars(&mut none, 0));
        let (_, problems) = bd::problems(world, t0, &id, &none).unwrap();
        assert!(problems.iter().any(|p| p.blocking && p.text.contains("needs a cellar")), "{id}: no cellar, its way on lost: {problems:?}");
        let mut two = d.clone();
        assert!(bd::set_cellars(&mut two, 2));
        let (_, problems) = bd::problems(world, t0, &id, &two).unwrap();
        assert!(problems.iter().all(|p| !p.blocking), "{id}: two cellars: {problems:?}");
        file.edits.designs.insert(id, two);
    }
    let w = World::new(file).unwrap();
    let l = town::layout(&w, t0, li);
    for bi in [keep, sewer] {
        let id = format!("b:{li}:{bi}");
        let it = interior::generate_id(&w, t0, &id).unwrap();
        assert!(it.levels[0].z == -2 && !it.levels[0].links.is_empty() && it.levels[1].links.is_empty(), "{id}: its ways on not on the deepest cellar");
        for k in &it.levels[0].links {
            let there = interior::generate_id(&w, t0, &k.to).unwrap_or_else(|| panic!("{id}: a way to {} that doesn't exist", k.to));
            assert!(there.levels.iter().any(|lv| lv.links.iter().any(|b| b.to == id)), "{id}: {} has no way back", k.to);
            if k.to.starts_with("k:") {
                assert!(there.levels.iter().all(|lv| lv.elevation_ft < it.levels[0].elevation_ft), "{id}: the deep dungeons are not below its deepest cellar");
            }
        }
        check_interior(&w, t0, &l, li, &id, &building_buckets(&l));
    }
}

/// A site designed by hand (as an agent writes one, in text): a level dug below the deepest,
/// joined by a way down, doors added where needed, a chest turned and in another look (read back
/// from the text plan), a room furnished; the world then builds the
/// design instead of the generated site. Breaking a rule (a room shut off, the way in moved, a
/// way up not under the way down) is caught.
/// A JSON value with every number that isn't a whole one rounded to 9 significant digits.
fn rounded(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Number(n) if !(n.is_i64() || n.is_u64()) => {
            let x = n.as_f64().unwrap();
            Value::String(format!("{x:.8e}"))
        }
        Value::Array(a) => Value::Array(a.into_iter().map(rounded).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, rounded(v))).collect()),
        v => v,
    }
}

fn designed_site(world: &World, t0: &worldgen::t0::T0) {
    use worldgen::under::design;
    let (li, k) = (0..worldgen::town::layout_count(t0))
        .find_map(|li| {
            let l = worldgen::town::layout(world, t0, li);
            l.entrances.iter().position(|e| e.kind == worldgen::under::UnderKind::Dungeon).map(|k| (li, k))
        })
        .expect("a dungeon");
    let id = format!("u:{li}:{k}");
    let base = design::design_of(world, t0, &id, false).unwrap();
    let (nx, ny, n) = (base.nx as usize, base.ny as usize, base.levels.len());
    // A free square of the deepest level's floor, clear of the edge, for the way down.
    let deep = &base.levels[0];
    let cells = design::decode(&deep.cells, nx * ny).unwrap();
    let held = |i: usize, j: usize| deep.items.iter().any(|f| (f.x as usize..(f.x + f.w) as usize).contains(&i) && (f.y as usize..(f.y + f.h) as usize).contains(&j));
    let (x, y) = (3..ny - 6)
        .flat_map(|j| (3..nx - 8).map(move |i| (i, j)))
        .find(|&(i, j)| cells[j * nx + i] >= 0 && !held(i, j) && !deep.doors.iter().any(|d| (d[0] as usize, d[1] as usize) == (i, j)))
        .expect("free floor");
    let items: Vec<String> = deep.items.iter().map(|f| format!("{} {},{} {}x{}", f.kind, f.x, f.y, f.w, f.h)).collect();
    // The new level: a landing round the way up, a vault east of it.
    let grid: String = (0..ny)
        .map(|j| (0..nx).map(|i| if i + 1 >= x && i <= x + 1 && j + 1 >= y && j <= y + 1 { 'a' } else if i >= x + 2 && i <= x + 6 && j + 2 >= y && j <= y + 2 { 'b' } else { '.' }).collect::<String>() + "\n")
        .collect();
    let level = |doors: &str| {
        format!(
            "site dungeon · {} levels\nlevel {n}\nitems: {}; down {x},{y}\nlevel {}: Level {} · the vault\nrooms: a=landing (cavern); b=secret vault (round) \"The Hoard\"\ngrid:\n{grid}doors: {doors}\nitems: chest {},{} variant 2 back e; chasm {},{} 1x5; rope_bridge {},{}; water_shallow {},{}; water_deep {},{}\n",
            n + 1,
            items.join("; "),
            n + 1,
            n + 1,
            x + 5,
            y,
            x + 4,
            y - 2,
            x + 4,
            y + 1,
            x + 3,
            y + 2,
            x + 3,
            y - 2
        )
    };
    let (d, names) = design::from_text(&level("auto"), &base).unwrap_or_else(|e| panic!("{id}: {e}"));
    assert_eq!(d.levels.len(), n + 1, "{id}: a level added");
    assert_eq!(names, vec![(0, 1, "The Hoard".to_string())], "{id}: an unknown kind of room is a chamber by that name");
    assert_eq!(d.levels[0].items.iter().filter(|f| f.kind == "up").map(|f| (f.x as usize, f.y as usize)).collect::<Vec<_>>(), vec![(x, y)], "{id}: the way up is put under the way down");
    let (it, problems) = d.problems(world, t0, &id).unwrap();
    assert!(problems.iter().all(|p| !p.blocking), "{id}: {problems:?}");
    assert_eq!(it.levels[0].doors.len(), 1, "{id}: one door from the landing into the vault");
    // A lock of colour 12 (door flags bits 8-11) on that door builds and reads back; the first seven keep bits 4-6.
    let mut locked = d.clone();
    locked.levels[0].doors[0][3] |= worldgen::interior::design::lock_flags(12);
    assert_eq!(locked.levels[0].doors[0][3] >> 4 & 7, 4, "{id}: the low bits of lock 12");
    assert_eq!(worldgen::interior::design::lock_flags(5), 5 << 4, "{id}: an old lock colour's flags changed");
    let (lit, lp) = locked.problems(world, t0, &id).unwrap();
    assert!(lit.levels[0].doors[0].lock == 12 && lp.iter().any(|p| p.text.contains("lock 12") && p.text.contains("no key")), "{id}: lock 12: {lp:?}");
    assert!(design::from_text(&design::to_text(&locked, &|_, _| None).unwrap(), &base).unwrap().0 == locked, "{id}: the lock 12 doesn't read back");
    // A landing set by hand: the way up must sit on it.
    let mut lands = d.clone();
    lands.levels[0].landing = Some([x as u16 - 1, y as u16]);
    assert!(lands.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.text.contains("not at the landing")), "{id}: a way up off its landing");
    lands.levels[0].items.iter_mut().filter(|f| f.kind == "up").for_each(|f| f.x -= 1);
    assert!(lands.problems(world, t0, &id).unwrap().1.iter().all(|p| !p.blocking), "{id}: the way up on its landing refused");
    assert!(design::from_text(&design::to_text(&lands, &|_, _| None).unwrap(), &base).unwrap().0 == lands, "{id}: the landing doesn't read back");
    let chest = it.levels[0].furniture.iter().find(|f| f.kind == "chest").expect("the chest");
    assert!((chest.back, chest.variant) == (Some(1), Some(2)), "{id}: the chest's side and look: {chest:?}");
    let text = design::to_text(&d, &|_, _| None).unwrap();
    assert!(text.contains(&format!("chest {},{} back e variant 2", x + 5, y)), "{id}: the chest in the text plan:\n{text}");
    assert!(design::from_text(&text, &base).unwrap().0 == d, "{id}: the text plan doesn't read back:\n{text}");
    // Terrain: a chasm across the vault (the chest beyond it) crossed by a rope bridge; without the bridge, cut off.
    assert!(text.contains(&format!("chasm {},{} 1x5; rope_bridge {},{}", x + 4, y - 2, x + 4, y + 1)), "{id}: the chasm and bridge in the text plan:\n{text}");
    // A stone bridge is a span like it: laid over the chasm, no clash, and the chest still reached.
    let mut stone = d.clone();
    stone.levels[0].items.iter_mut().filter(|f| f.kind == "rope_bridge").for_each(|f| f.kind = "stone_bridge".to_string());
    assert!(stone.problems(world, t0, &id).unwrap().1.iter().all(|p| !p.blocking), "{id}: a stone bridge over the chasm clashes or cuts off");
    let mut fallen = d.clone();
    fallen.levels[0].items.retain(|f| f.kind != "rope_bridge");
    assert!(fallen.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.text.contains("can't be reached")), "{id}: the chasm cut nothing off");
    // Shapes: the landing a cavern breaking in; the vault round in the plan but drawn square (its
    // squares fill their box: noted), until the designer fits it to an octagon (its corners rock).
    use worldgen::interior::shape;
    assert_eq!((it.levels[0].rooms[0].shape, it.levels[0].rooms[1].shape), (shape::CAVERN, 0), "{id}: the landing's and vault's shapes as built");
    assert!(problems.iter().any(|p| !p.blocking && p.text.contains("drawn square")), "{id}: the vault drawn square, not noted: {problems:?}");
    assert!(text.contains("a=landing (cavern); b=secret vault (round)"), "{id}: the shapes in the text plan:\n{text}");
    let v: serde_json::Value = serde_json::from_str(&design::design_json(world, t0, &id, Some(&serde_json::to_string(&d).unwrap()), Some(r#"{"shape":{"level":0,"room":1,"shape":"octagon"}}"#))).unwrap();
    let d: design::SiteDesign = serde_json::from_value(v["design"].clone()).unwrap();
    assert!(v["problems"].as_array().unwrap().iter().all(|p| p["blocking"] == false), "{id}: the octagonal vault: {}", v["problems"]);
    assert_eq!(v["interior"]["levels"][0]["rooms"][1]["shape"], shape::OCTAGON, "{id}: the vault not drawn octagonal");
    let vault = design::decode(&d.levels[0].cells, nx * ny).unwrap().iter().filter(|&&c| c == 1).count();
    assert_eq!(vault, 21, "{id}: the octagonal vault's squares");
    let text = design::to_text(&d, &|_, _| None).unwrap();
    assert!(text.contains("b=secret vault (octagon)") && design::from_text(&text, &base).unwrap().0 == d, "{id}: the octagonal vault's text plan doesn't read back:\n{text}");
    let built = d.problems(world, t0, &id).unwrap().0;
    assert!(design::SiteDesign::from_interior(&built).levels[0].rooms == d.levels[0].rooms, "{id}: the shapes don't come back from what the design builds");
    // Broken: the vault shut off; the way in moved; the way up off the way down.
    let (shut, _) = design::from_text(&level(""), &base).unwrap();
    assert!(shut.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.level == 0 && p.text.contains("can't be reached")), "{id}: a room shut off");
    let mut moved = d.clone();
    let top = moved.levels.last_mut().unwrap();
    let e = top.items.iter_mut().find(|f| f.kind == "exit").unwrap();
    e.x += 1;
    assert!(moved.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.text.contains("under the entrance")), "{id}: the way in moved");
    let mut off = d.clone();
    off.levels[0].items.iter_mut().find(|f| f.kind == "up").unwrap().y += 1;
    assert!(off.problems(world, t0, &id).unwrap().1.iter().any(|p| p.blocking && p.level == 0), "{id}: the way up off the way down");
    // Furnished: props go where they leave everything reachable.
    let mut f = d.clone();
    let had = f.levels[0].items.len();
    f.furnish(world, t0, &id, 0, 0, 7);
    f.levels[0].rooms[0].kind = "armory".into();
    f.furnish(world, t0, &id, 0, 0, 7);
    assert!(f.levels[0].items.len() > had, "{id}: the armory got nothing");
    assert!(f.problems(world, t0, &id).unwrap().1.iter().all(|p| !p.blocking), "{id}: furnishing broke it");
    // A design travels as an edit op (the app's, an agent's) and comes back as it was.
    let mut file = world.file.clone();
    let v = serde_json::to_value(&f).unwrap();
    let undo = file.edits.apply(&worldgen::world::EditOp::Set { field: "designs".into(), key: id.clone(), value: v.clone() }).unwrap();
    assert_eq!(serde_json::to_value(&file.edits.designs[&id]).unwrap(), v, "{id}: the design changed on the round trip");
    assert!(matches!(undo, worldgen::world::EditOp::Unset { .. }), "{id}: undoing a new design unsets it");
    // Live edits reach the generators field by field, or as patches of keyed fields: the same
    // edits as sent whole.
    let mut changed = file.edits.clone();
    changed.renames.insert(format!("r:{id}:0:1"), "The Hoard".into());
    changed.designs.remove(&id);
    let mut by_field = file.edits.clone();
    let fields = serde_json::json!({ "renames": changed.renames, "designs": changed.designs });
    assert!(by_field.set_fields(&fields.to_string()).unwrap().is_none(), "{id}: created sites untouched");
    assert!(by_field == changed, "{id}: edits set field by field differ");
    let mut patched = file.edits.clone();
    let patch = serde_json::json!({ "renames": { "set": { format!("r:{id}:0:1"): "The Hoard" } }, "designs": { "unset": [id] } });
    patched.patch_fields(&patch.to_string()).unwrap();
    assert!(patched == changed, "{id}: patched edits differ");
    // The world builds the design (its hash unchanged), the same in a world read back from JSON.
    let w2 = World::new(file).unwrap();
    assert_eq!(w2.hash, world.hash, "designs are edits: the world hash stays");
    let it2 = worldgen::interior::generate_id(&w2, t0, &id).unwrap();
    assert_eq!(it2.levels.len(), n + 1, "{id}: the world builds the design");
    assert_eq!(it2.levels[0].rooms[1].shape, shape::OCTAGON, "{id}: the world doesn't build the vault octagonal");
    let w3 = World::from_json(&serde_json::to_string(&w2.file).unwrap()).unwrap();
    // (serde_json reads a long float back to within a unit in its last place, the design's origin
    // among them: the same but for that.)
    let it3 = serde_json::to_value(worldgen::interior::generate_id(&w3, t0, &id).unwrap()).unwrap();
    assert_eq!(rounded(it3), rounded(serde_json::to_value(&it2).unwrap()), "{id}: read back");
    assert!(design::design_of(&w2, t0, &id, true).unwrap() == base, "{id}: the generated site is still there underneath");
    // The design under another site's key (edits kept for another sketch or generator): set
    // aside, that site built as generated; its own site keeps it.
    let other = (0..worldgen::town::layout_count(t0)).filter(|&l| l != li).map(|l| format!("u:{l}:0")).find(|o| design::editable(world, t0, o).is_ok()).expect("another site");
    let mut f4 = w2.file.clone();
    let d = f4.edits.designs.remove(&id).unwrap();
    f4.edits.designs.insert(other.clone(), d);
    let w4 = World::new(f4).unwrap();
    assert!(!design::misplaced(&w2, t0, &id), "{id}: its own design is built");
    assert!(design::misplaced(&w4, t0, &other), "{other}: a design made for {id} is not set aside");
    let (ol, _) = design::editable(world, t0, &other).unwrap();
    let generated = serde_json::to_value(worldgen::under::generate(world, t0, ol, 0).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(worldgen::interior::generate_id(&w4, t0, &other).unwrap()).unwrap(), generated, "{other}: not built as generated");
    assert!(design::design_of(&w4, t0, &other, false).unwrap() == design::design_of(&w4, t0, &other, true).unwrap(), "{other}: the misplaced design is read back");
    assert!(worldgen::interior::design::set_aside(&w4, t0).iter().any(|(i, _)| *i == other), "{other}: not listed as set aside");
}

/// Every floor square of a site's level is reached from `start` without climbing a sheer edge (`interior::height`: up
/// by ramps or steps, down by anything), around blocking furniture, between rooms through doors (any step on a natural
/// level). Called by `underground_guarantee` on every level.
fn r5_no_climb(tag: &str, lv: &worldgen::interior::Level, nx: usize, ny: usize, start: usize) {
    use worldgen::interior::height;
    assert!(lv.floor.is_empty() || lv.floor.len() == nx * ny, "{tag} {}: {} floor heights for {} squares", lv.name, lv.floor.len(), nx * ny);
    let f = height::of_level(lv, nx);
    let blocks = worldgen::under::blocked_squares(&lv.furniture, nx, ny);
    let door = |a: usize, b: usize| {
        let (ia, ja, ib, jb) = ((a % nx) as f32, (a / nx) as f32, (b % nx) as f32, (b / nx) as f32);
        let (p, q) = if ja == jb { ([ia.max(ib), ja], [ia.max(ib), ja + 1.0]) } else { ([ia, ja.max(jb)], [ia + 1.0, ja.max(jb)]) };
        lv.doors.iter().any(|d| (d.a == p && d.b == q) || (d.a == q && d.b == p))
    };
    let mut seen = vec![false; nx * ny];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(k) = stack.pop() {
        let (i, j) = (k % nx, k / nx);
        for q in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten() {
            if seen[q] || lv.cells[q] < 0 || blocks[q] || f.sheer(k, q) > 0.0 || (!lv.natural && lv.cells[q] != lv.cells[k] && !door(k, q)) {
                continue;
            }
            seen[q] = true;
            stack.push(q);
        }
    }
    let lost: Vec<usize> = (0..nx * ny).filter(|&q| lv.cells[q] >= 0 && !blocks[q] && !seen[q]).collect();
    assert!(lost.is_empty(), "{tag} {}: {} squares reached only by a climb (first at {}, {})", lv.name, lost.len(), lost[0] % nx, lost[0] / nx);
}

// R5 mcp

/// The underground search index (`undersearch::sites`, built without laying any site out) says
/// what the sites are when laid out: each `u:` site's levels, theme and size, and each keep's
/// deep dungeons' levels. A sample over seed 1 (T0 and the index cost seconds a seed) that takes the first temple (the sunken
/// rule's path) and keep dungeon, the first of each kind of `u:` site, and a spread.
#[test]
fn underground_index_matches_sites() {
    for (seed, spread, all_kinds) in [(1, 2, true)] {
        let world = World::from_json(&world_json(seed)).unwrap();
        let t0 = worldgen::t0::T0::generate(&world);
        let sites = worldgen::undersearch::sites(&world, &t0);
        assert!(sites.iter().any(|s| s.id.starts_with("u:")), "seed {seed}: no site underground in the index");
        if seed == 1 {
            assert!(sites.iter().any(|s| s.id.starts_with("w:") && !s.grates.is_empty()), "seed {seed}: no sewers in the index");
        }
        let mut pick: Vec<usize> = Vec::new();
        let mut kinds: Vec<String> = Vec::new();
        for (i, s) in sites.iter().enumerate() {
            let first_kind = all_kinds && s.id.starts_with("u:") && !kinds.contains(&format!("{:?}", s.kind));
            if first_kind {
                kinds.push(format!("{:?}", s.kind));
            }
            let rare = (s.theme.ends_with("temple") || s.id.starts_with("k:")) && !pick.iter().any(|&j| sites[j].theme == s.theme);
            if first_kind || rare {
                pick.push(i);
            }
        }
        pick.extend((0..spread).map(|k| k * sites.len() / spread));
        pick.sort_unstable();
        pick.dedup();
        for s in pick.iter().map(|&i| &sites[i]) {
            let n: Vec<usize> = s.id.split(':').skip(1).filter_map(|x| x.parse().ok()).collect();
            let it = match s.id.as_bytes()[0] {
                b'u' => worldgen::under::generate(&world, &t0, n[0], n[1]),
                b'k' => worldgen::under::keep_dungeon(&world, &t0, n[0], n[1]),
                _ => continue,
            };
            let it = it.unwrap_or_else(|| panic!("seed {seed}: {} in the index but not generated", s.id));
            assert_eq!(it.levels.len(), s.levels as usize, "seed {seed}: {} levels", s.id);
            if s.id.starts_with("u:") {
                assert_eq!(it.theme, Some(s.theme), "seed {seed}: {} theme", s.id);
                assert_eq!(worldgen::under::site_size(&it).map(|z| z.name()), Some(s.size), "seed {seed}: {} size", s.id);
            }
        }
    }
}

/// An entrance made in a building's cellar (`Created::in_building`): the cellar has the way down
/// to the site and the site the way up to the building (both resolve and lead back), the
/// battlemap draws no opening on the surface for it, and the site takes its entrance's name.
#[test]
fn entrance_in_a_cellar_links_both_ways() {
    use worldgen::battlemap::ShapeKind;
    use worldgen::world::Created;
    use worldgen::{interior, sitelinks, town};
    let file: WorldFile = serde_json::from_str(&world_json(1)).unwrap();
    // (The towns are the same with or without the entrance: pick the building in a world that has the entrance far off.)
    let (bid, bi, li, mid, el) = {
        let ex = Executor::new(World::new(file.clone()).unwrap());
        let t0 = &ex.t0;
        let li = (0..t0.settlements.len()).max_by_key(|&i| (t0.settlements[i].tier as u8, t0.settlements[i].population)).unwrap();
        let l = town::layout(&ex.world, t0, li);
        let b = l.buildings.iter().find(|b| b.structure == town::Structure::Roofed && interior::has_cellar(b)).expect("a building with a cellar");
        (format!("b:{li}:{}", b.id), b.id as usize, li, town::geom::centroid(&b.poly), t0.settlements.len() + t0.base_pois)
    };
    let made = |in_building: Option<String>| {
        let mut f = file.clone();
        let name = "Undermountain".to_string();
        f.edits.created.push(Created { id: "c:0".into(), kind: "entrance".into(), x: mid[0], y: mid[1], name, in_building, way: Some("stairs".into()), ..Default::default() });
        Executor::new(World::new(f).unwrap())
    };
    let (mut ex, mut plain) = (made(Some(bid.clone())), made(None));
    assert!(ex.world.file.edits.created[0].check().is_ok());
    assert_eq!(sitelinks::building_of(&ex.world, &ex.t0, el), Some((li, bi)), "the entrance is not found in its building");
    let (cellar, site) = (interior::generate_id(&ex.world, &ex.t0, &bid).expect("the building"), interior::generate_id(&ex.world, &ex.t0, &format!("u:{el}:0")).expect("the site"));
    let to = format!("u:{el}:0");
    assert!(cellar.levels.iter().any(|lv| lv.z < 0 && lv.links.iter().any(|k| k.to == to)), "the cellar has no way down to {to}");
    assert!(site.levels.iter().any(|lv| lv.links.iter().any(|k| k.to == bid)), "{to} has no way up to {bid}");
    let up = site.levels.iter().flat_map(|lv| lv.furniture.iter()).any(|f| f.kind == "exit" && f.name == "stairs up to the cellar");
    assert!(up, "the way in is not the stairs up to the cellar");
    assert_eq!(site.name.as_deref(), Some("Undermountain"), "the site does not take its entrance's name");
    // Both ways resolve and lead back.
    for it in [&cellar, &site] {
        for k in it.levels.iter().flat_map(|lv| lv.links.iter()).filter(|k| k.to == bid || k.to == to) {
            let there = interior::generate_id(&ex.world, &ex.t0, &k.to).unwrap_or_else(|| panic!("a way to {} that doesn't exist", k.to));
            assert!(there.levels.iter().any(|lv| lv.links.iter().any(|b| b.to == it.id)), "{} has no way back", k.to);
        }
    }
    // The chunk holding the point draws no opening for it (the same entrance in no building does).
    let (level, size) = (ex.world.geom.max_level, ex.world.geom.tile_size_ft(ex.world.geom.max_level));
    let key = TileKey::surface(level, (mid[0] / size) as u32, (mid[1] / size) as u32);
    let openings = |ex: &mut Executor| ex.battlemap(key).structures.iter().filter(|s| s.kind == ShapeKind::Entrance).count();
    let (a, p) = (openings(&mut ex), openings(&mut plain));
    assert_eq!(a + 1, p, "the chunk draws {a} openings with the entrance in the building, {p} without");
}

// R5 world
