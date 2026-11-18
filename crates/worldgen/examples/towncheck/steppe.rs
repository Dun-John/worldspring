use crate::prelude::*;

/// R5 towns (114a): a steppe town's roofs (every steppe town and city, not a site): by material and
/// shape, and the rule (0): clay tile or shingle on a home, a shop or a workshop.
pub(crate) fn steppe_roofs(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["steppe towns", "roofed buildings", "clay tile or shingle on homes, shops, workshops"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town || l.culture % 5 != 4 {
            continue;
        }
        r.add("steppe towns", 0.0, format!("seed {} layout {li} ({:?})", world.file.seed, l.tier));
        for b in l.buildings.iter().filter(|b| b.structure == town::Structure::Roofed) {
            let k = town::roof::look(&l, b);
            let (m, s) = (town::roof::MATERIALS[k.material as usize], town::roof::SHAPES[k.shape as usize]);
            r.tally("roofed buildings", 1);
            r.tally(&format!("  roofs {m} {s}"), 1);
            let f = b.func.map(|f| &town::catalog::CATALOG[f as usize]);
            let grand = f.is_some_and(|f| matches!(f.category, "faith & death" | "civic & military" | "guilds & underworld") || matches!(f.key, "library" | "bank" | "auction_house" | "theater" | "arena" | "bathhouse" | "bards_college" | "observatory" | "arcane_academy"));
            if !grand && matches!(m, "clay_tile" | "shingle") {
                r.add("clay tile or shingle on homes, shops, workshops", 0.0, format!("seed {} b:{li}:{} {}", world.file.seed, b.id, f.map_or("home", |f| f.key)));
            }
            if grand {
                r.tally(&format!("  grand {m} {s}"), 1);
            }
        }
    }
}

/// R5 towns (114a): steppe towns' round felt halls (`town::roof::hall_ring`: the town hall,
/// temples, inns): built and fallen back to rectangles (why), their sizes; the rules (each 0): a
/// hall over another building, a road or a wall; a hall's interior with squares cut off from its
/// door or missing a piece its use needs. Doors facing a street within reach (as a yurt's); the
/// roofs (`steppe_roofs`).
pub(crate) fn steppe(ctx: &mut Ctx, r: &mut Report) {
    use town::roof::{FeltUse, HallRefused};
    steppe_roofs(ctx, r);
    steppe_gaps(ctx, r);
    r.counts(&["felt halls built", "  halls", "  temples", "  inns", "fell back: lot too small", "fell back: a castle's or a ruin"]);
    r.counts(&["halls over another building", "halls over a road", "halls over a wall", "doors within 30 degrees of a street or square", "doors toward the centre (no street in reach)", "doors off their street"]);
    r.counts(&["interiors with squares cut off from the door", "interiors missing a piece", "interiors not one room on one storey", "interiors not built again from their design"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let mut listed = 0;
    let cross = |a: P, b: P, c: P, d: P| {
        let o = |p: P, q: P, s: P| (q[0] - p[0]) * (s[1] - p[1]) - (q[1] - p[1]) * (s[0] - p[0]);
        o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
    };
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town || l.culture % 5 != 4 {
            continue;
        }
        for b in l.buildings.iter() {
            let Some(key) = b.func.map(|f| town::catalog::CATALOG[f as usize].key) else { continue };
            let Some(u) = FeltUse::of_key(key) else { continue };
            let id = format!("b:{li}:{}", b.id);
            let tag = format!("seed {} {id} ({})", world.file.seed, u.name());
            if town::roof::felt_built(b).is_none() {
                match town::roof::hall_refused(&l, b) {
                    Some(HallRefused::Small) => {
                        let o = town::geom::obb(&b.poly);
                        r.add("fell back: lot too small", 0.0, format!("{tag}: {:.0} x {:.0} ft", o.long, o.short));
                    }
                    Some(HallRefused::Pieces) => r.add("fell back: a castle's or a ruin", 0.0, tag.clone()),
                    None => {}
                }
                continue;
            }
            r.tally("felt halls built", 1);
            r.tally(["  halls", "  temples", "  inns"][u as usize], 1);
            let n = b.poly.len();
            let c = b.poly.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n as f64, a[1] + p[1] / n as f64]);
            let rad = libm::hypot(b.poly[0][0] - c[0], b.poly[0][1] - c[1]);
            r.value(&format!("{} across (ft)", u.name()), 2.0 * rad, tag.clone());
            // Over another building, a road, a wall.
            let edges = |p: &[P]| (0..p.len()).map(move |k| (p[k], p[(k + 1) % p.len()])).collect::<Vec<_>>();
            let mine = edges(&b.poly);
            let hit = l.buildings.iter().filter(|o| o.id != b.id && o.structure == town::Structure::Roofed && libm::hypot(o.poly[0][0] - c[0], o.poly[0][1] - c[1]) < 400.0).find(|o| {
                o.poly.iter().any(|p| town::geom::contains(&b.poly, *p))
                    || b.poly.iter().any(|p| town::geom::contains(&o.poly, *p))
                    || edges(&o.poly).iter().any(|e| mine.iter().any(|m| cross(m.0, m.1, e.0, e.1)))
            });
            if let Some(o) = hit {
                r.add("halls over another building", 0.0, format!("{tag}: b:{li}:{}", o.id));
            }
            let road = l.roads.iter().find(|(pts, _, w)| pts.windows(2).any(|s| mine.iter().any(|m| cross(m.0, m.1, s[0], s[1])) || b.poly.iter().any(|p| town::geom::seg_dist(*p, s[0], s[1]) < 0.5 * w - 0.5)));
            if let Some((_, class, w)) = road {
                r.add("halls over a road", 0.0, format!("{tag}: class {class}, {w:.0} ft"));
            }
            if l.walls.iter().any(|w| w.windows(2).any(|s| b.poly.iter().any(|p| town::geom::seg_dist(*p, s[0], s[1]) < 3.0) || mine.iter().any(|m| cross(m.0, m.1, s[0], s[1])))) {
                r.add("halls over a wall", 0.0, tag.clone());
            }
            // The door: its street (as `town::yurt_door` looks for it).
            let door = [0.5 * (b.poly[0][0] + b.poly[1][0]), 0.5 * (b.poly[0][1] + b.poly[1][1])];
            let ang = libm::atan2(door[1] - c[1], door[0] - c[0]).to_degrees();
            let reach = 70.0 + (rad - 15.0).max(0.0);
            let mut near: Option<(f64, P)> = None;
            for w in l.roads.iter().map(|w| &w.0).chain(l.streets.iter()).chain(l.plazas.iter().map(|p| &p.pts)) {
                for e in w.windows(2) {
                    let ab = [e[1][0] - e[0][0], e[1][1] - e[0][1]];
                    let t = (((c[0] - e[0][0]) * ab[0] + (c[1] - e[0][1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
                    let q = [e[0][0] + ab[0] * t, e[0][1] + ab[1] * t];
                    let d = libm::hypot(q[0] - c[0], q[1] - c[1]);
                    if d <= reach && near.is_none_or(|b| d < b.0) {
                        near = Some((d, q));
                    }
                }
            }
            let off = |a: f64, b: f64| ((a - b + 540.0) % 360.0 - 180.0).abs();
            match near {
                Some((_, q)) => {
                    let to = libm::atan2(q[1] - c[1], q[0] - c[0]).to_degrees();
                    if off(ang, to) <= 30.0 {
                        r.tally("doors within 30 degrees of a street or square", 1);
                    } else {
                        r.add("doors off their street", off(ang, to), format!("{tag}: {:.0} degrees off", off(ang, to)));
                    }
                }
                None => r.add("doors toward the centre (no street in reach)", 0.0, tag.clone()),
            }
            // Inside: one round room on one storey, every square not under a piece reached from the door.
            let Some(it) = interior::generate_id(world, t0, &id) else {
                r.add("interiors not one room on one storey", 0.0, format!("{tag}: none"));
                continue;
            };
            let room = ["felt hall", "felt sanctuary", "felt common room"][u as usize];
            let lv = &it.levels[it.entry_level.min(it.levels.len() - 1)];
            let Some(ri) = lv.rooms.iter().position(|q| q.kind == room).filter(|_| it.levels.len() == 1 && lv.rooms.iter().filter(|q| q.squares > 0).count() == 1) else {
                r.add("interiors not one room on one storey", 0.0, format!("{tag}: {} levels, rooms {:?}", it.levels.len(), lv.rooms.iter().map(|q| q.kind).collect::<Vec<_>>()));
                continue;
            };
            let (nx, ny) = (it.nx, it.ny);
            let mut blocked = vec![false; nx * ny];
            for f in lv.furniture.iter().filter(|f| f.blocks_move) {
                for j in f.y as usize..(f.y + f.h) as usize {
                    for i in f.x as usize..(f.x + f.w) as usize {
                        blocked[j * nx + i] = true;
                    }
                }
            }
            let walk = |k: usize| lv.cells[k] == ri as i16 && !blocked[k];
            let start = lv.doors.iter().find(|d| d.kind == "front").and_then(|d| {
                let (x, y) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize);
                let pair = if d.a[0] == d.b[0] { [(x - 1, y), (x, y)] } else { [(x, y - 1), (x, y)] };
                pair.into_iter().filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).find(|&k| walk(k))
            });
            let mut seen = vec![false; nx * ny];
            let mut stack: Vec<usize> = start.into_iter().collect();
            while let Some(k) = stack.pop() {
                if seen[k] {
                    continue;
                }
                seen[k] = true;
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny && walk(b * nx + a) && !seen[b * nx + a] {
                        stack.push(b * nx + a);
                    }
                }
            }
            let cut = (0..nx * ny).filter(|&k| walk(k) && !seen[k]).count();
            if cut > 0 || start.is_none() {
                r.add("interiors with squares cut off from the door", cut as f64, format!("{tag}: {cut} squares cut off{}", if start.is_none() { ", no way in" } else { "" }));
            }
            let needs: &[&str] = match u {
                FeltUse::Hall => &["yurt_hearth", "low_dais", "chair_of_office", "bench"],
                FeltUse::Temple => &["yurt_hearth", "altar", "brazier", "floor_cushion"],
                FeltUse::Inn => &["yurt_hearth", "counter", "table", "yurt_pallet"],
            };
            let missing: Vec<&str> = needs.iter().copied().filter(|k| !lv.furniture.iter().any(|f| f.kind == *k)).collect();
            if !missing.is_empty() {
                r.add("interiors missing a piece", 0.0, format!("{tag}: no {}", missing.join(", ")));
            }
            for f in &lv.furniture {
                r.tally(&format!("  {} pieces: {}", u.name(), f.kind), 1);
            }
            // (As a design builds it again: the same, and within the designer's rules.)
            if let Ok((d, _)) = interior::design::design_of(world, t0, &id, false) {
                let (built, problems) = interior::design::problems(world, t0, &id, &d).unwrap();
                let (x, y) = (serde_json::to_string(&built).unwrap(), serde_json::to_string(&it).unwrap());
                if x != y && std::env::var("DIFF").is_ok() {
                    let k = x.bytes().zip(y.bytes()).position(|(a, b)| a != b).unwrap_or(0);
                    eprintln!("{tag}: built {} / generated {}", &x[k.saturating_sub(150)..(k + 80).min(x.len())], &y[k.saturating_sub(150)..(k + 80).min(y.len())]);
                }
                if !problems.is_empty() || x != y {
                    r.add("interiors not built again from their design", 0.0, format!("{tag}: {problems:?}"));
                }
            }
            if listed < 12 || u == FeltUse::Hall {
                listed += 1;
                r.add("felt halls listed", 0.0, format!("{tag} at {:.0},{:.0}, {:.0} ft across, door {ang:.0}, {} pieces", c[0], c[1], 2.0 * rad, lv.furniture.len()));
            }
        }
    }
}

/// R5 towns (114-115 fix): steppe towns' buildings against their neighbours: footprints overlapping
/// another's (rule, 0), those within 1 ft of another building (against the base's count), and round
/// ones (a yurt, a felt hall, a shrine) within 2 ft of another (a felt hall or yurt keeps a yard).
pub(crate) fn steppe_gaps(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&["footprints overlapping another building's", "buildings within 1 ft of another", "round buildings within 2 ft of another"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town || l.culture % 5 != 4 {
            continue;
        }
        let bs: Vec<&town::Building> = l.buildings.iter().filter(|b| b.poly.len() >= 3).collect();
        let boxes: Vec<[f64; 4]> = bs.iter().map(|b| bbox(&b.poly)).collect();
        for i in 0..bs.len() {
            let (mut over, mut near) = (None, None);
            for j in 0..bs.len() {
                let (a, b) = (boxes[i], boxes[j]);
                if i == j || a[0] > b[2] + 1.0 || b[0] > a[2] + 1.0 || a[1] > b[3] + 1.0 || b[1] > a[3] + 1.0 {
                    continue;
                }
                if poly_overlap(&shrink(&bs[i].poly, 0.15), &shrink(&bs[j].poly, 0.15)) {
                    over = Some(j);
                }
                let n = bs[j].poly.len();
                let d = (0..n).map(|k| seg_poly(bs[j].poly[k], bs[j].poly[(k + 1) % n], &bs[i].poly)).fold(f64::MAX, f64::min);
                if d < 2.0 && near.is_none_or(|(_, e)| d < e) {
                    near = Some((j, d));
                }
            }
            let round = bs[i].poly.len() == town::roof::ROUND_SIDES || bs[i].poly.len() == town::roof::HALL_SIDES;
            let tag = |j: usize| format!("seed {} b:{li}:{} ({} sides) by b:{li}:{} ({} sides)", world.file.seed, bs[i].id, bs[i].poly.len(), bs[j].id, bs[j].poly.len());
            if let Some(j) = over.filter(|&j| j > i) {
                r.add("footprints overlapping another building's", 0.0, tag(j));
            }
            if let Some((j, d)) = near {
                if d < 1.0 {
                    r.tally("buildings within 1 ft of another", 1);
                }
                if round {
                    let (c, w) = (centroid(&bs[i].poly), world.params().width_mi * 5280.0);
                    let h = world.params().height_mi * 5280.0;
                    r.add("round buildings within 2 ft of another", d, format!("{}: {d:.2} ft (fx {:.5} fy {:.5})", tag(j), c[0] / w, c[1] / h));
                }
            }
        }
    }
}

// R5 towns (114)
/// A prop's footprint (ft along its turn, across) as this check reads it, apart from the generator's.
pub(crate) fn horse_foot(kind: Kind, at: P, rot: f32, scale: f32) -> Option<[P; 4]> {
    let (l, w) = match kind {
        // (As drawn: its sprite 2 squares nose to tail, its middle a foot ahead of where it stands.)
        Kind::Horse => (9.8, 2.7),
        Kind::HitchingRail => (10.0, 1.2),
        Kind::WaterTrough => (6.8, 2.8),
        Kind::Haystack => (7.2, 7.2),
        Kind::Cart => (11.5, 5.5),
        _ => return None,
    };
    let (l, w) = (l * scale as f64, w * scale as f64);
    let u = [libm::cos(rot as f64), libm::sin(rot as f64)];
    let at = if kind == Kind::Horse { add(at, mul(u, scale as f64)) } else { at };
    let (a, b) = (mul(u, 0.5 * l), mul([-u[1], u[0]], 0.5 * w));
    Some([sub(sub(at, a), b), sub(add(at, a), b), add(add(at, a), b), add(sub(at, a), b)])
}

/// Whether two convex outlines overlap (separating axes; touching is not).
pub(crate) fn boxes_cross(a: &[P], b: &[P]) -> bool {
    for q in [a, b] {
        for k in 0..q.len() {
            let e = sub(q[(k + 1) % q.len()], q[k]);
            let ax = [-e[1], e[0]];
            let pr = |r: &[P]| r.iter().fold((f64::MAX, f64::MIN), |s, p| (s.0.min(dot(*p, ax)), s.1.max(dot(*p, ax))));
            let ((a0, a1), (b0, b1)) = (pr(a), pr(b));
            if a0 >= b1 || b0 >= a1 {
                return false;
            }
        }
    }
    true
}

pub(crate) fn seg_rect(a: P, b: P, w: f64) -> [P; 4] {
    let l = dist(a, b).max(1e-9);
    let u = mul(sub(b, a), 1.0 / l);
    let n = mul([-u[1], u[0]], 0.5 * w);
    [sub(a, n), sub(b, n), add(b, n), add(a, n)]
}

pub(crate) const HORSE_RULES: [&str; 9] = [
    "R Formal parks left in steppe towns",
    "R horse parks not by hand outside steppe towns",
    "R props overlapping props",
    "R props over a fence",
    "R props on a way in or in a gate",
    "R fences across a way in",
    "R horses outside a corral or the market ground",
    "R props outside the park",
    "R battlemap park squares beyond cover",
];

/// Horse parks (114): how many per steppe town, what they hold, the rules (each 0) on the plan; on the
/// battlemap, props and fence laid against planned and the ground; a park patch set to one by hand.
pub(crate) fn horseparks(ctx: &mut Ctx, r: &mut Report) {
    r.counts(&HORSE_RULES);
    r.counts(&["parks with under 2 corrals", "battlemap: horse props planned", "battlemap: horse props laid", "hand horse parks refused", "hand horse parks not laid"]);
    let (world, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let size = world.geom.tile_size_ft(world.geom.max_level);
    let mut cams = 0;
    let (mw, mh) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let mut look: Vec<(String, town::Park, worldgen::town::horses::Plan)> = Vec::new();
    for li in 0..town::layout_count(t0) {
        let l = town::layout(world, t0, li);
        if l.site || l.tier < worldgen::t0::settle::Tier::Town {
            continue;
        }
        let steppe = l.culture % 5 == 4;
        let tier = format!("{:?}", l.tier);
        if steppe {
            r.tally(&format!("steppe towns ({tier})"), 1);
        }
        for pk in &l.parks {
            let here = format!("L{li} at {}", at(centroid(&pk.pts)));
            if steppe && pk.kind == town::ParkKind::Formal {
                r.add(HORSE_RULES[0], 1.0, here.clone());
            }
            if pk.kind != town::ParkKind::Horses {
                if steppe {
                    r.tally(&format!("steppe parks kept: {}", pk.kind.name()), 1);
                }
                continue;
            }
            if !steppe {
                r.add(HORSE_RULES[1], 1.0, here.clone());
            }
            r.tally(&format!("horse parks ({tier})"), 1);
            let Some(pl) = town::horses::plan(pk) else { continue };
            check_horse_park(pk, &pl, &here, r);
            if look.len() < 10 {
                look.push((here.clone(), pk.clone(), pl.clone()));
            }
            if cams < 4 {
                let c = centroid(&pk.pts);
                cams += 1;
                say!("    camera {here}: fx {:.6} fy {:.6}", c[0] / mw, c[1] / mh);
            }
        }
    }
    // A park patch set to horses by hand (not a steppe town), walls as they are.
    let mut tried = 0;
    for ci in (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town && t0.settlements[i].culture % 5 != 4) {
        if tried >= 2 {
            break;
        }
        let Ok(plan) = town::wards::plan_json(world, t0, ci) else { continue };
        let pick = plan["patches"].as_array().unwrap().iter().find(|p| p["in_town"] == true && p.get("park_refused").is_none() && !matches!(p["ward"].as_str(), Some("plaza" | "castle" | "temple"))).and_then(|p| p["patch"].as_u64());
        let Some(p) = pick else { continue };
        tried += 1;
        let json = format!(r#"{{"patches":[{{"patch":{p},"ward":"park","park":"horses"}}]}}"#);
        let Ok((Some(edit), _)) = town::wards::change(world, t0, ci, &serde_json::from_str(&json).unwrap()) else {
            r.add("hand horse parks refused", 1.0, format!("#{ci} patch {p}"));
            continue;
        };
        let mut file = world.file.clone();
        file.edits.towns.insert(town::wards::key(ci), edit);
        let w2 = World::new(file).unwrap();
        let l = town::layout(&w2, t0, ci);
        let at_p = plan["patches"].as_array().unwrap().iter().find(|q| q["patch"].as_u64() == Some(p)).map(|q| [q["at"][0].as_f64().unwrap(), q["at"][1].as_f64().unwrap()]).unwrap();
        match l.parks.iter().find(|pk| pk.kind == town::ParkKind::Horses && dist(centroid(&pk.pts), at_p) < 200.0) {
            Some(pk) => {
                r.tally("hand horse parks", 1);
                let here = format!("hand #{ci} patch {p}");
                if let Some(pl) = town::horses::plan(pk) {
                    check_horse_park(pk, &pl, &here, r);
                }
                let c = centroid(&pk.pts);
                say!("    camera {here}: fx {:.6} fy {:.6}", c[0] / mw, c[1] / mh);
            }
            None => r.add("hand horse parks not laid", 1.0, format!("#{ci} patch {p}")),
        }
    }
    for (here, pk, pl) in &look {
        let b = pk.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
        let k = |v: f64| (v / size).floor().max(0.0) as u32;
        let (mut laid, mut fence, mut dirt, mut squares, mut far) = (0usize, 0.0f64, 0usize, 0usize, 0usize);
        for ky in k(b[1])..=k(b[3]) {
            for kx in k(b[0])..=k(b[2]) {
                let c = ctx.chunk(kx, ky);
                let (ox, oy) = (kx as f64 * size, ky as f64 * size);
                for o in &c.objects {
                    let p = [ox + o.x as f64 * SQUARE_FT, oy + o.y as f64 * SQUARE_FT];
                    if contains(&pk.pts, p) && matches!(o.kind, Kind::Horse | Kind::HitchingRail | Kind::WaterTrough | Kind::Haystack | Kind::Cart) {
                        laid += 1;
                    }
                    // (Anything else, or more than planned: what the battlemap's guarantee added.)
                    if contains(&pk.pts, p) && !pl.props.iter().any(|q| q.2 == o.kind && dist(q.0, p) < 0.5) {
                        let f = horse_foot(o.kind, p, o.rot, o.scale).unwrap_or_else(|| {
                            let r = c.info(o).radius as f64 * o.scale as f64 * SQUARE_FT;
                            [add(p, [-r, -r]), add(p, [r, -r]), add(p, [r, r]), add(p, [-r, r])]
                        });
                        let over = pl.props.iter().filter_map(|q| horse_foot(q.2, q.0, q.1, q.4)).any(|g| boxes_cross(&f, &g));
                        let fenced = pl.fences().iter().any(|&(a, e)| boxes_cross(&f, &seg_rect(a, e, 0.8)));
                        r.add("battlemap: props added on a horse park", 0.0, format!("{here}: {} at {}{}{}", c.name(o), at(p), if over { ", over a planned prop" } else { "" }, if fenced { ", over a fence" } else { "" }));
                    }
                }
                for s in c.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Fence && s.pts.len() >= 2) {
                    let f = |q: [f32; 2]| [ox + q[0] as f64 * SQUARE_FT, oy + q[1] as f64 * SQUARE_FT];
                    let (a, e) = (f(s.pts[0]), f(s.pts[1]));
                    if contains(&pk.pts, lerp(a, e, 0.5)) {
                        fence += dist(a, e);
                    }
                }
                // (The guarantee's rise in each 16-square cell: an auction block (a dais), else a knoll.)
                for s in c.structures.iter().filter(|s| s.kind == worldgen::battlemap::ShapeKind::Dais && !s.pts.is_empty()) {
                    if contains(&pk.pts, [ox + s.pts[0][0] as f64 * SQUARE_FT, oy + s.pts[0][1] as f64 * SQUARE_FT]) {
                        r.tally("battlemap: daises (auction blocks)", 1);
                    }
                }
                let inside = |q: usize| contains(&pk.pts, [ox + ((q % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((q / SQ) as f64 + 0.5) * SQUARE_FT]) && c.building[q] == 0;
                if let Some(low) = (0..SQ * SQ).filter(|&q| inside(q)).map(|q| c.tier[q]).min() {
                    r.tally("battlemap: park squares raised", (0..SQ * SQ).filter(|&q| inside(q) && c.tier[q] > low).count());
                }
                let gap = worldgen::battlemap::cover_distance(&c);
                for q in 0..SQ * SQ {
                    let p = [ox + ((q % SQ) as f64 + 0.5) * SQUARE_FT, oy + ((q / SQ) as f64 + 0.5) * SQUARE_FT];
                    if contains(&pk.pts, p) && c.water_level[q] <= c.height[q] && gap[q] > worldgen::battlemap::cover_gap(&c, q) {
                        far += 1;
                    }
                    if contains(&pk.pts, p) && c.building[q] == 0 {
                        squares += 1;
                        dirt += (c.surface[q] == Surface::Dirt) as usize;
                    }
                }
            }
        }
        let planned = pl.props.len();
        let want: f64 = pl.fences().iter().map(|(a, e)| dist(*a, *e)).sum();
        r.tally("battlemap: horse props planned", planned);
        r.tally("battlemap: horse props laid", laid);
        r.tally("battlemap: fence ft planned", want as usize);
        r.tally("battlemap: fence ft laid", fence as usize);
        r.tally("battlemap: park squares", squares);
        r.tally("battlemap: dirt squares", dirt);
        if far > 0 {
            r.add("R battlemap park squares beyond cover", far as f64, format!("{here}: {far}"));
        }
        if laid < planned || fence + 1.0 < want {
            r.add("battlemap: parks short of props or fence", 0.0, format!("{here}: props {laid}/{planned}, fence {fence:.0}/{want:.0} ft"));
        }
    }
}

pub(crate) fn check_horse_park(pk: &town::Park, pl: &town::horses::Plan, here: &str, r: &mut Report) {
    use town::horses::{GATE_FT, WAY_HALF};
    let nc = pl.corrals.len();
    r.tally(&format!("parks with {nc} corrals"), 1);
    if nc < 2 {
        r.add("parks with under 2 corrals", 0.0, format!("{here}: {nc}, {:.0} sq ft", area(&pk.pts).abs()));
    }
    let fences = pl.fences();
    let fence_boxes: Vec<[P; 4]> = fences.iter().map(|&(a, b)| seg_rect(a, b, 0.8)).collect();
    let way_boxes: Vec<[P; 4]> = pl.ways.iter().filter(|w| dist(w.0, w.1) > 0.5).map(|&(a, b, _)| seg_rect(a, b, 2.0 * WAY_HALF)).collect();
    // (A gate's opening: its gap, 2 ft either side of the fence.)
    let gate_boxes: Vec<[P; 4]> = pl
        .corrals
        .iter()
        .map(|c| {
            let (a, b) = (c.pts[c.side], c.pts[(c.side + 1) % 4]);
            let u = mul(sub(b, a), 1.0 / dist(a, b));
            seg_rect(sub(c.gate, mul(u, 0.5 * GATE_FT)), add(c.gate, mul(u, 0.5 * GATE_FT)), 4.0)
        })
        .collect();
    let props: Vec<(Kind, [P; 4])> = pl.props.iter().filter_map(|&(at, rot, kind, _, sc)| horse_foot(kind, at, rot, sc).map(|f| (kind, f))).collect();
    for (kind, _) in &props {
        r.tally(&format!("props: {}", format!("{kind:?}").to_lowercase()), 1);
    }
    for (i, (ka, fa)) in props.iter().enumerate() {
        for (kb, fb) in props.iter().skip(i + 1) {
            if boxes_cross(fa, fb) {
                r.add(HORSE_RULES[2], 1.0, format!("{here}: {ka:?} and {kb:?} at {}", at(centroid(fa))));
            }
        }
        if fence_boxes.iter().any(|f| boxes_cross(f, fa)) {
            r.add(HORSE_RULES[3], 1.0, format!("{here}: {ka:?} at {}", at(centroid(fa))));
        }
        if way_boxes.iter().chain(&gate_boxes).any(|f| boxes_cross(f, fa)) {
            r.add(HORSE_RULES[4], 1.0, format!("{here}: {ka:?} at {}", at(centroid(fa))));
        }
        if !fa.iter().all(|&p| contains(&pk.pts, p)) {
            r.add(HORSE_RULES[7], 1.0, format!("{here}: {ka:?} at {}", at(centroid(fa))));
        }
        if *ka == Kind::Horse {
            let pen = pl.corrals.iter().position(|c| fa.iter().all(|&p| contains(&c.pts, p)));
            let market = fa.iter().all(|&p| contains(&pk.pts, p)) && !pl.corrals.iter().any(|c| boxes_cross(&c.pts, fa));
            match (pen, market) {
                (Some(_), _) => r.tally("horses in corrals", 1),
                (None, true) => r.tally("horses on the market ground (tied, led)", 1),
                _ => r.add(HORSE_RULES[6], 1.0, format!("{here}: at {}", at(centroid(fa)))),
            }
        }
    }
    for (k, c) in pl.corrals.iter().enumerate() {
        let n = props.iter().filter(|(kd, f)| *kd == Kind::Horse && f.iter().all(|&p| contains(&c.pts, p))).count();
        r.value("horses a corral", n as f64, format!("{here} corral {k}"));
        let (w, d) = (dist(c.pts[0], c.pts[1]), dist(c.pts[0], c.pts[3]));
        r.value("corral ft (along x in)", w * d, format!("{here} corral {k}: {w:.0} x {d:.0}"));
        for kd in [Kind::WaterTrough, Kind::Haystack] {
            if !props.iter().any(|(x, f)| *x == kd && f.iter().all(|&p| contains(&c.pts, p))) {
                r.add(&format!("corrals with no {}", format!("{kd:?}").to_lowercase()), 0.0, format!("{here} corral {k}"));
            }
        }
    }
    for (a, b) in &fences {
        if way_boxes.iter().any(|w| boxes_cross(w, &seg_rect(*a, *b, 0.8))) {
            r.add(HORSE_RULES[5], 1.0, format!("{here}: fence at {}", at(lerp(*a, *b, 0.5))));
        }
    }
}

pub(crate) const ROW_COUNTS: [&str; 9] = [
    "lots off their band",
    "lots overlapping on one side (bends)",
    "lots overlapping another row's",
    "lots over the region avoided",
    "lots with no front on their street",
    "lots on the wrong side",
    "lots or buildings not convex",
    "lots narrower than 16 ft or filling under 62% of their box",
    "rows not repeating bit for bit",
];

/// `rows`: both sides of each town's streets (`Layout::roads` but alleys and paved strips) lined
/// through `town::qa_rows`, round the town's buildings, walls, other streets and their bands.
pub(crate) fn rows(base: &World, t0: &T0, label: &str, r: &mut Report) {
    use std::sync::atomic::AtomicUsize;
    use worldgen::town::clip::{self, Ends, Join, Shape};
    use worldgen::town::geom::obb;
    use worldgen::town::{QaRow, qa_bands, qa_rows};
    static SVGS: AtomicUsize = AtomicUsize::new(0);
    r.counts(&ROW_COUNTS);
    let most: usize = std::env::var("ROWS").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let wards = [Ward::Craft, Ward::Merchant, Ward::Common, Ward::Slum, Ward::Noble];
    let bbox = |p: &[P]| p.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
    let meets = |a: &[f64; 4], b: &[f64; 4]| a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3];
    let towns = (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier >= worldgen::t0::settle::Tier::Town);
    let mut svg_done = false;
    for (done, ci) in towns.take(most).enumerate() {
        let l = town::layout(base, t0, ci);
        let streets: Vec<(Vec<P>, f64)> = l.roads.iter().filter(|s| s.1 != 3 && s.0.len() >= 2).map(|s| (s.0.clone(), 0.5 * s.2)).collect();
        let mut rows: Vec<QaRow> = Vec::new();
        let mut bands: Vec<Vec<Vec<P>>> = Vec::new();
        let mut avoid: Vec<(u32, Shape)> = l.buildings.iter().map(|b| (u32::MAX, Shape::ring(b.poly.clone()))).collect();
        for w in &l.walls {
            avoid.extend(clip::buffer(w, 4.0, 4.0, Join::Miter(2.0), Ends::Flat).into_iter().map(|s| (u32::MAX, s)));
        }
        for (k, (line, hw)) in streets.iter().enumerate() {
            let depth = 30.0 + 8.0 * (k % 3) as f64;
            avoid.extend(clip::buffer(line, *hw, *hw, Join::Miter(2.0), Ends::Square).into_iter().map(|s| (k as u32, s)));
            for left in [true, false] {
                let row = QaRow { line: line.clone(), left, setback: hw + 2.0, depth, tag: k as u32 };
                let b = qa_bands(&row.line, left, row.setback, depth);
                avoid.extend(b.iter().map(|q| (k as u32, Shape::ring(q.clone()))));
                bands.push(b);
                rows.push(row);
            }
        }
        let (ward, seed) = (wards[ci % wards.len()], ci as u64 + 7);
        let out = qa_rows(base, t0, ci, ward, seed, &rows, &avoid);
        if done < 2 {
            let again = qa_rows(base, t0, ci, ward, seed, &rows, &avoid);
            if out.iter().zip(&again).any(|(a, b)| a.lots != b.lots || a.alleys != b.alleys) {
                r.add(ROW_COUNTS[8], 1.0, format!("{label} town {ci}"));
            }
        }
        let tag = format!("{label} town {ci} ({})", ward.name());
        let (mut lots_n, mut feet) = (0, 0.0);
        let mut every: Vec<(usize, &Vec<P>, [f64; 4])> = Vec::new();
        for (i, (row, o)) in rows.iter().zip(&out).enumerate() {
            let run: f64 = row.line.windows(2).map(|w| dist(w[0], w[1])).sum();
            lots_n += o.lots.len();
            feet += run;
            if run >= 200.0 {
                r.value("lots per 100 ft of street (a side, 200 ft or longer)", 100.0 * o.lots.len() as f64 / run, format!("{tag} street {}", row.tag));
            }
            r.tally("alleys", o.alleys.len());
            let off = |p: P| !bands[i].iter().any(|b| contains(b, p) || (0..b.len()).any(|e| seg_dist(p, b[e], b[(e + 1) % b.len()]) < 0.05));
            let near_line = |p: P| row.line.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f64::MAX, f64::min);
            for (lot, house) in &o.lots {
                let c = centroid(lot);
                let at = format!("{tag} street {} at {:.0},{:.0}", row.tag, c[0], c[1]);
                if lot.iter().chain(house).any(|p| off(*p)) {
                    r.add(ROW_COUNTS[0], 1.0, at.clone());
                }
                let bb = bbox(lot);
                let close: Vec<Shape> = avoid.iter().filter(|(t, s)| *t != row.tag && meets(&bbox(&s.outer), &bb)).map(|(_, s)| s.clone()).collect();
                let over = if close.is_empty() { 0.0 } else { clip::area(&clip::intersection(&[Shape::ring(lot.clone())], &close)) };
                if over > 1.0 {
                    r.add(ROW_COUNTS[3], over, format!("{at}: {over:.1} ft²"));
                }
                let m = lot.len();
                let band = |p: P| (near_line(p) - row.setback).abs() < 2.0;
                if !(0..m).any(|e| dist(lot[e], lot[(e + 1) % m]) > 6.0 && band(lot[e]) && band(lot[(e + 1) % m]) && band(lerp(lot[e], lot[(e + 1) % m], 0.5))) {
                    r.add(ROW_COUNTS[4], 1.0, at.clone());
                }
                let seg = row.line.windows(2).min_by(|a, b| seg_dist(c, a[0], a[1]).total_cmp(&seg_dist(c, b[0], b[1]))).unwrap();
                if (cross(sub(seg[1], seg[0]), sub(c, seg[0])) > 0.0) != row.left {
                    r.add(ROW_COUNTS[5], 1.0, at.clone());
                }
                if !clip::is_convex(lot) || !clip::is_convex(house) {
                    r.add(ROW_COUNTS[6], 1.0, at.clone());
                }
                let ob = obb(lot);
                if ob.short < 16.0 - 1e-6 || area(lot).abs() < 0.62 * ob.long * ob.short {
                    r.add(ROW_COUNTS[7], 1.0, at.clone());
                }
                every.push((i, lot, bb));
            }
        }
        for (x, &(i, a, ba)) in every.iter().enumerate() {
            for &(j, b, bb) in &every[x + 1..] {
                if !meets(&ba, &bb) {
                    continue;
                }
                let o = clip::area(&clip::intersection(&[Shape::ring(a.clone())], &[Shape::ring(b.clone())]));
                if o > 1.0 {
                    let c = centroid(a);
                    r.add(if i == j { ROW_COUNTS[1] } else { ROW_COUNTS[2] }, o, format!("{tag} at {:.0},{:.0}: {o:.1} ft²", c[0], c[1]));
                }
            }
        }
        r.tally("lots laid", lots_n);
        r.tally("street sides, 100 ft", (feet / 100.0).round() as usize);
        r.tally("towns", 1);
        say!("  {tag}: {} streets, {lots_n} lots along {:.0} ft of street sides", streets.len(), feet);
        // One picture a world (`SVG_DIR`, up to 6): the street whose rows hold the most lots, a
        // road out of the town (class 0-2) before a street in it.
        let Some(dir) = std::env::var("SVG_DIR").ok() else { continue };
        let class: Vec<u8> = l.roads.iter().filter(|s| s.1 != 3 && s.0.len() >= 2).map(|s| s.1).collect();
        let Some(k) = (0..streets.len()).max_by_key(|&k| (class[k] <= 2, out[2 * k].lots.len() + out[2 * k + 1].lots.len())) else { continue };
        if svg_done || out[2 * k].lots.len() + out[2 * k + 1].lots.len() < 6 || SVGS.load(Ordering::Relaxed) >= 6 {
            continue;
        }
        svg_done = true;
        SVGS.fetch_add(1, Ordering::Relaxed);
        let pts: Vec<P> = out[2 * k..2 * k + 2].iter().flat_map(|o| o.lots.iter().flat_map(|l| l.0.iter().copied())).collect();
        let b = bbox(&pts);
        let c = [0.5 * (b[0] + b[2]), 0.5 * (b[1] + b[3])];
        let half = (0.5 * (b[2] - b[0]).max(b[3] - b[1]) + 60.0).min(600.0);
        let win = [c[0] - half, c[1] - half, c[0] + half, c[1] + half];
        let path = |p: &[P], close: bool| {
            let mut s = String::new();
            for (n, q) in p.iter().enumerate() {
                s += &format!("{}{:.1},{:.1} ", if n == 0 { 'M' } else { 'L' }, q[0] - win[0], win[3] - q[1]);
            }
            if close {
                s += "Z";
            }
            s
        };
        let mut svg = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {0:.0} {0:.0}' width='900' height='900'><rect width='100%' height='100%' fill='#f4efe2'/>", 2.0 * half);
        for (line, hw) in &streets {
            if meets(&bbox(line), &win) {
                svg += &format!("<path d='{}' fill='none' stroke='#cbbf9f' stroke-width='{:.1}' stroke-linejoin='round'/>", path(line, false), 2.0 * hw);
            }
        }
        for bl in l.buildings.iter().filter(|bl| meets(&bbox(&bl.poly), &win)) {
            svg += &format!("<path d='{}' fill='#9a948a'/>", path(&bl.poly, true));
        }
        for i in [2 * k, 2 * k + 1] {
            for band in &bands[i] {
                svg += &format!("<path d='{}' fill='none' stroke='#6a8fc0' stroke-width='0.6' stroke-dasharray='3 2'/>", path(band, true));
            }
            for (lot, house) in &out[i].lots {
                svg += &format!("<path d='{}' fill='#e9d3a8' stroke='#7a5a30' stroke-width='0.5'/>", path(lot, true));
                svg += &format!("<path d='{}' fill='#c46e4a' stroke='#5a2d1a' stroke-width='0.7'/>", path(house, true));
            }
            for (a, b2) in &out[i].alleys {
                svg += &format!("<path d='{}' stroke='#8a6a3a' stroke-width='2'/>", path(&[*a, *b2], false));
            }
        }
        svg += "</svg>";
        let file = format!("{dir}/rows-{label}-{ci}.svg");
        if std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&file, svg)).is_ok() {
            say!("    wrote {file}");
        }
    }
}

/// Roadmap 6 P3: every town converted to a hand town (`town::hand::convert`), and the first 4
/// with the street editor's scripted edit (`scripted_edit`) too. Every block's kept buildings
/// (`kept_on`, from its record alone) with the header's must be the town's buildings exactly,
/// field by field; converting again (the towns in the other order) must give the same bytes.
/// Sizes: each town's records as stored (compact JSON), per tier (`HAND_WHY=<layout>`: what
/// that one's bytes and buildings stored whole are).
pub(crate) fn hand_towns(world: &World, t0: &T0, label: &str, r: &mut Report) {
    use std::collections::BTreeMap as Map;
    use worldgen::town::hand;
    const FIELDS: [&str; 14] = ["poly", "ward", "func", "residential", "name", "floors", "pad_ft", "structure", "roof", "look", "id", "parts", "keep", "deck"];
    let fields: Vec<String> = FIELDS.iter().map(|f| format!("buildings differing: {f}")).collect();
    r.counts(&["towns converted", "edited towns converted", "towns refused", "buildings", "buildings kept by their fill", "buildings stored whole", "buildings missing", "buildings extra", "towns not the same converted again", "size class changed by conversion"]);
    r.counts(&fields.iter().map(String::as_str).collect::<Vec<_>>());
    let lis: Vec<usize> = (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= worldgen::t0::settle::Tier::Town).collect();
    let mut first: Map<usize, String> = Map::new();
    for &li in &lis {
        match hand::convert(world, t0, li) {
            Ok(recs) => drop(first.insert(li, serde_json::to_string(&recs).unwrap())),
            Err(e) => r.add("towns refused", 0.0, format!("{label} L{li}: {e}")),
        }
    }
    for &li in lis.iter().rev() {
        let Some(json) = first.get(&li) else { continue };
        let recs = hand::convert(world, t0, li).unwrap_or_default();
        if serde_json::to_string(&recs).unwrap() != *json {
            r.add("towns not the same converted again", 0.0, format!("{label} L{li}"));
        }
        r.tally("towns converted", 1);
        // (Its people as its homes count them, against its population as generated.)
        let people = recs.iter().find(|(k, _)| *k == li.to_string()).and_then(|(_, v)| v["people"].as_u64()).unwrap_or(0) as u32;
        let s = &t0.settlements[li];
        r.value("people counted / population", people as f64 / s.population.max(1) as f64, format!("{label} L{li}"));
        if hand::tier_of_people(people) != s.tier {
            r.add("size class changed by conversion", 0.0, format!("{label} L{li}: {} {} counted {people}", s.tier.name(), s.population));
        }
        hand_one(world, t0, li, &recs, label, true, r);
    }
    for &li in lis.iter().take(4) {
        let Some(Ok(edit)) = scripted_edit(world, t0, li) else { continue };
        let mut file = world.file.clone();
        file.edits.towns.insert(town::wards::key(li), edit);
        let w = World::new(file).unwrap();
        match hand::convert(&w, t0, li) {
            Ok(recs) => {
                r.tally("edited towns converted", 1);
                hand_one(&w, t0, li, &recs, &format!("{label} edited"), false, r);
            }
            Err(e) => r.add("towns refused", 0.0, format!("{label} L{li} edited: {e}")),
        }
    }
    hand_built(world, t0, label, &first, r);
    hand_edits(world, t0, label, &lis, r);
}

/// Roadmap 6 P3c1: a scripted run of changes on a few towns (laid out by hand, a block split, two
/// merged, a junction dragged, a block filled anew with a renamed house on it, a row added, back
/// to generated), each through `handedit::change`: every change made, the graph unbroken, no new
/// building outside its block or over another, ids one each, what carries something kept or told
/// lost, and back to generated the town as generated again.
pub(crate) fn hand_edits(world: &World, t0: &T0, label: &str, lis: &[usize], r: &mut Report) {
    use serde_json::json;
    use worldgen::town::clip;
    use worldgen::town::hand;
    use worldgen::town::handedit::{self, HandRequest};
    const STEPS: [&str; 8] = ["lay out", "split", "merge", "drag", "refill", "row", "square", "back"];
    r.counts(&["edits: towns", "edits: steps", "edits: refused", "edits: problems", "edits: outside the block", "edits: overlapping", "edits: ids twice", "edits: carried", "edits: told lost", "edits: lost untold", "edits: not as generated again", "edits: blocks relaid", "edits: blocks added", "edits: blocks removed", "edits: businesses new", "edits: closes"]);
    let per = worldgen::town::wards::PER_PATCH * worldgen::town::wards::ROLLS;
    for &li in lis.iter().take(4) {
        let tag = format!("{label} L{li}");
        let mut w = World::new(world.file.clone()).unwrap();
        let mut renamed: Vec<u32> = Vec::new();
        let mut ok = true;
        r.tally("edits: towns", 1);
        for step in STEPS {
            if !ok {
                break;
            }
            let recs = hand::HandRecords::of(&w.file.edits.streets, li);
            let plan = handedit::plan_json(&w, t0, li).ok();
            let blocks = plan.as_ref().map(|p| p["blocks"].as_array().cloned().unwrap_or_default()).unwrap_or_default();
            let mid = |id: u32| -> Option<[f64; 2]> {
                let st = recs.streets.get(&id)?;
                let line: Vec<[f64; 2]> = std::iter::once(recs.junctions.get(&st.a)?.p).chain(st.bends.iter().copied()).chain([recs.junctions.get(&st.b)?.p]).collect();
                Some(lerp(line[0], line[1], 0.5))
            };
            let ids = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect::<Vec<u32>>()).unwrap_or_default();
            // (The biggest kept block built on lots, by its streets round it.)
            let lots: Vec<&serde_json::Value> = blocks.iter().filter(|b| b["kept"] == json!(true) && b["fill"]["kind"] == json!("generated") && ids(&b["streets"]).len() >= 4).collect();
            let req: Option<HandRequest> = match step {
                "lay out" => Some(HandRequest::LayOutByHand),
                "split" => lots.first().and_then(|b| {
                    let round = ids(&b["streets"]);
                    let m0 = mid(round[0])?;
                    let far = round.iter().copied().skip(1).filter_map(|e| Some((dist(mid(e)?, m0), e))).max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))?;
                    Some(HandRequest::AddStreet { points: vec![m0, mid(far.1)?], width: None, set_l: None, set_r: None, class: None })
                }),
                "merge" => {
                    let mut seen: BTreeMap<u32, u32> = BTreeMap::new();
                    for b in &lots {
                        for e in ids(&b["streets"]) {
                            *seen.entry(e).or_default() += 1;
                        }
                    }
                    seen.iter().find(|(e, n)| **n >= 2 && recs.streets.get(e).is_some_and(|s| s.class == hand::StreetClass::Street)).map(|(e, _)| HandRequest::DeleteStreet { street: *e })
                }
                "drag" => recs.junctions.iter().find(|(id, _)| recs.streets.values().filter(|s| s.a == **id || s.b == **id).count() >= 4).map(|(id, _)| HandRequest::MoveJunctions { moves: vec![handedit::JunctionMove { junction: *id, to: None, by: Some([6.0, 4.0]) }] }),
                "refill" => lots.get(1).or(lots.first()).map(|b| {
                    let id = b["id"].as_u64().unwrap() as u32;
                    // (Two of its houses renamed first: they stay, pinned, or are told lost.)
                    if let Some(hb) = hand::build_hand(&w, t0, li) {
                        let ground = hb.blocks.iter().find(|x| x.0 == id).map(|x| x.1.clone()).unwrap_or_default();
                        let mut file = w.file.clone();
                        for bd in hb.layout.buildings.iter().filter(|bd| bd.func.is_none() && clip::contains(&ground, centroid(&bd.poly))).take(2) {
                            file.edits.renames.insert(format!("b:{li}:{}", bd.id), format!("Renamed {}", bd.id));
                            renamed.push(bd.id);
                        }
                        w = World::new(file).unwrap();
                    }
                    HandRequest::SetBlock { block: id, fill: Some(hand::BlockFill::Generated { ward: "merchant".into(), lots: None, height: None, reroll: 0, close: None }), district: None, seed: None, reroll: false }
                }),
                "row" => recs.streets.iter().filter_map(|(id, st)| Some((dist(recs.junctions.get(&st.a)?.p, recs.junctions.get(&st.b)?.p), *id))).max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1))).map(|(_, e)| HandRequest::AddRow { street: e, side: "l".into(), depth: 40.0, ward: "common".into(), lots: None, gaps: None, seed: None }),
                "square" => recs.junctions.iter().find(|(id, j)| j.square.is_none() && recs.streets.values().filter(|s| s.a == **id || s.b == **id).count() >= 3).map(|(id, _)| HandRequest::SetSquare { junction: *id, square: "market".into() }),
                _ => Some(HandRequest::BackToGenerated),
            };
            let Some(req) = req else {
                r.add("edits: refused", 0.0, format!("{tag} {step}: nothing to do it on"));
                continue;
            };
            let before = town::base_layout(&w, t0, li);
            let clock = Instant::now();
            // (One change sent as JSON, as the app and the agent server send it: the same change.)
            if step == "row"
                && let HandRequest::AddRow { street, .. } = &req
            {
                let text = handedit::change_json(&w, t0, li, &format!(r#"{{"op": "add_row", "street": {street}, "side": "l", "depth": 40, "ward": "common"}}"#));
                let got: serde_json::Value = serde_json::from_str(&text).unwrap();
                let want = handedit::change(&w, t0, li, &req).map(|x| x.0.to_json()).unwrap_or_default();
                if got["change"] != want {
                    r.add("edits: refused", 0.0, format!("{tag} {step}: as JSON otherwise: {}", &text[..text.len().min(200)]));
                }
            }
            match handedit::change(&w, t0, li, &req) {
                Err(e) => {
                    r.add("edits: refused", 0.0, format!("{tag} {step}: {e}"));
                    ok = step != "lay out";
                }
                Ok((ch, rep)) => {
                    r.tally("edits: steps", 1);
                    let mut file = w.file.clone();
                    ch.apply(&mut file.edits);
                    w = World::new(file).unwrap();
                    let after = town::base_layout(&w, t0, li);
                    if std::env::var("HAND_EDITS").is_ok() {
                        let pins: usize = hand::HandRecords::of(&w.file.edits.streets, li).blocks.values().map(|b| b.pins.len()).sum();
                        say!("      {tag} {step} ({} ms): blocks {} buildings {} -> {} rects {} lost {} pins {} renamed {:?} kept {:?}", clock.elapsed().as_millis(), rep["blocks"], before.buildings.len(), after.buildings.len(), rep["rects"].as_array().map_or(0, |a| a.len()), rep["lost"].as_array().map_or(0, |a| a.len()), pins, renamed, renamed.iter().map(|id| after.buildings.iter().any(|b| b.id == *id)).collect::<Vec<_>>());
                    }
                    for (k, n) in [("relaid", "edits: blocks relaid"), ("added", "edits: blocks added"), ("removed", "edits: blocks removed")] {
                        r.tally(n, rep["blocks"][k].as_array().map_or(0, |a| a.len()));
                    }
                    for p in rep["problems"].as_array().into_iter().flatten() {
                        r.add("edits: problems", 0.0, format!("{tag} {step}: {} {}", p["key"], p["why"]));
                    }
                    // Ids one each; what carries something kept or told lost.
                    let mut seen = std::collections::BTreeSet::new();
                    for b in &after.buildings {
                        if !seen.insert(b.id) {
                            r.add("edits: ids twice", 0.0, format!("{tag} {step}: b{}", b.id));
                        }
                    }
                    let told: std::collections::BTreeSet<String> = rep["lost"].as_array().into_iter().flatten().filter_map(|x| x["id"].as_str().map(String::from)).collect();
                    r.tally("edits: told lost", told.len());
                    for b in before.buildings.iter().filter(|b| b.func.is_some() || renamed.contains(&b.id)) {
                        if seen.contains(&b.id) {
                            r.tally("edits: carried", 1);
                        } else if !told.contains(&format!("b:{li}:{}", b.id)) {
                            r.add("edits: lost untold", 0.0, format!("{tag} {step}: b{}", b.id));
                        }
                    }
                    r.tally("edits: businesses new", after.buildings.iter().filter(|b| b.func.is_some() && b.id >= hand::HAND_ID).count().saturating_sub(before.buildings.iter().filter(|b| b.func.is_some() && b.id >= hand::HAND_ID).count()));
                    if step == "back" {
                        let g = town::generated_layout(&w, t0, li);
                        let same = g.buildings.len() == after.buildings.len() && g.buildings.iter().zip(after.buildings.iter()).all(|(a, b)| a.id == b.id && a.poly == b.poly);
                        if !same || hand::is_hand(&w, li) {
                            let edited = w.file.edits.towns.contains_key(&town::wards::key(li));
                            if !edited || !same {
                                // (With a ward edit restored: against the town as that edit lays it out.)
                                let e = w.file.edits.towns.get(&town::wards::key(li)).cloned();
                                let want = e.map(|e| town::wards::with_edit(&w, t0, li, &e)).unwrap_or(g);
                                if want.buildings.len() != after.buildings.len() || want.buildings.iter().zip(after.buildings.iter()).any(|(a, b)| a.id != b.id || a.poly != b.poly) {
                                    r.add("edits: not as generated again", 0.0, format!("{tag}: {} buildings against {}", after.buildings.len(), want.buildings.len()));
                                }
                            }
                        }
                        continue;
                    }
                    // New buildings of the blocks laid out anew: in their block, over no other.
                    let Some(hb) = hand::build_hand(&w, t0, li) else { continue };
                    let relaid: Vec<u32> = ids(&rep["blocks"]["relaid"]).into_iter().chain(ids(&rep["blocks"]["added"])).collect();
                    r.tally("edits: closes", hb.layout.plazas.iter().filter(|p| p.kind == worldgen::town::PlazaKind::TempleClose).count().saturating_sub(before.plazas.iter().filter(|p| p.kind == worldgen::town::PlazaKind::TempleClose).count()));
                    let fresh: Vec<&town::Building> = hb.layout.buildings.iter().filter(|b| b.id >= hand::HAND_ID && relaid.contains(&((b.id - hand::HAND_ID) / per))).collect();
                    for bd in &fresh {
                        let blk = (bd.id - hand::HAND_ID) / per;
                        let poly = clip::region(&bd.poly);
                        let out = hb.blocks.iter().find(|x| x.0 == blk).map_or(clip::area(&poly), |g| clip::area(&clip::difference(&poly, &g.1)));
                        if out > 1.0 {
                            r.add("edits: outside the block", out, format!("{tag} {step} b{}: {out:.1} sq ft", bd.id));
                        }
                    }
                    for (k, a) in fresh.iter().enumerate() {
                        for b in &fresh[k + 1..] {
                            if dist(centroid(&a.poly), centroid(&b.poly)) > 200.0 {
                                continue;
                            }
                            let o = clip::area(&clip::intersection(&clip::region(&a.poly), &clip::region(&b.poly)));
                            if o > 1.0 {
                                r.add("edits: overlapping", o, format!("{tag} {step} b{} b{}: {o:.1} sq ft", a.id, b.id));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Roadmap 6 P3b2: each converted town's walls, towers, gates, bridges, squares at junctions,
/// districts and ways underground against the town's (positions within 1 ft, the same counts);
/// then laid anew from the wall lines alone (`works` dropped): towers and gates against the
/// town's (how many more or fewer, told, not a rule).
pub(crate) fn hand_works(world: &World, t0: &T0, label: &str, first: &std::collections::BTreeMap<usize, String>, hw: &World, originals: &std::collections::BTreeMap<usize, std::rc::Rc<Layout>>, r: &mut Report) {
    use worldgen::town::geom::dist;
    use worldgen::town::hand;
    const WHAT: [&str; 7] = ["walls", "towers", "gates", "gate towers", "bridges missing", "squares at junctions", "quarters"];
    r.counts(&WHAT.iter().map(|w| format!("hand: {w}")).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>());
    r.counts(&WHAT.iter().map(|w| format!("hand: {w} differing")).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>());
    r.counts(&["hand: bridges", "hand: bridges new (a street over water the town left unbridged)", "hand: entrances", "hand: entrances otherwise many (told)", "laid anew: walled towns", "laid anew: towers more", "laid anew: towers fewer", "laid anew: gates more", "laid anew: gates fewer", "laid anew: castles' towers (generated)", "laid anew: castles' towers"]);
    let near = |a: &[[f64; 2]], b: &[[f64; 2]]| a.len() == b.len() && a.iter().all(|p| b.iter().any(|q| dist(*p, *q) <= 1.0)) && b.iter().all(|p| a.iter().any(|q| dist(*p, *q) <= 1.0));
    let corners = |l: &Layout| l.plazas.iter().filter(|p| matches!(p.spot, worldgen::town::places::Spot::Corner(_))).map(|p| (format!("{:?} {:?}", p.spot, p.kind), p.pts.clone())).collect::<std::collections::BTreeMap<_, _>>();
    for (&li, want) in originals {
        let got = town::base_layout(hw, t0, li);
        let lists = |l: &Layout| -> [Vec<Vec<[f64; 2]>>; 4] { [l.walls.clone(), vec![l.towers.clone()], vec![l.gates.clone()], vec![l.gate_towers.clone()]] };
        let (lw, lg) = (lists(want), lists(&got));
        for (k, (a, b)) in lw.iter().zip(&lg).enumerate() {
            let n: usize = a.iter().map(|x| x.len()).sum();
            r.tally(&format!("hand: {}", WHAT[k]), n);
            if a.len() != b.len() || !a.iter().zip(b).all(|(x, y)| near(x, y)) {
                r.add(&format!("hand: {} differing", WHAT[k]), 0.0, format!("{label} L{li}: {} against {}", b.iter().map(|x| x.len()).sum::<usize>(), n));
            }
        }
        r.tally("hand: bridges", want.bridges.len());
        let lost = want.bridges.iter().filter(|d| !got.bridges.iter().any(|e| near(d, e))).count();
        if lost > 0 {
            r.add("hand: bridges missing", lost as f64, format!("{label} L{li}: {lost} of {}", want.bridges.len()));
        }
        if want.bridges.len() != got.bridges.len() || lost > 0 {
            let odd: Vec<String> = got.bridges.iter().filter(|e| !want.bridges.iter().any(|d| near(d, e))).map(|e| {
                let c = worldgen::town::geom::centroid(e);
                format!("({:.0},{:.0}) {:.0} ft", c[0] - want.center[0], c[1] - want.center[1], dist(e[0], e[1]).max(dist(e[1], e[2])))
            }).collect();
            r.add("hand: bridges new (a street over water the town left unbridged)", 0.0, format!("{label} L{li}: {} against {}; new {}", got.bridges.len(), want.bridges.len(), odd.join(" ")));
        }
        let (cw, cg) = (corners(want), corners(&got));
        r.tally("hand: squares at junctions", cw.len());
        for (k, v) in &cw {
            if cg.get(k).is_none_or(|g| !near(g, v)) {
                r.add("hand: squares at junctions differing", 0.0, format!("{label} L{li} {k}"));
            }
        }
        let qn = |l: &Layout| l.quarters.iter().map(|q| format!("{} {:?}", q.name, q.kind)).collect::<std::collections::BTreeSet<_>>();
        r.tally("hand: quarters", want.quarters.len());
        if qn(want) != qn(&got) {
            r.add("hand: quarters differing", 0.0, format!("{label} L{li}: {:?} against {:?}", qn(&got).difference(&qn(want)).collect::<Vec<_>>(), qn(want).difference(&qn(&got)).collect::<Vec<_>>()));
        }
        r.tally("hand: entrances", want.entrances.len());
        if want.entrances.len() != got.entrances.len() {
            r.add("hand: entrances otherwise many (told)", 0.0, format!("{label} L{li}: {} against {}", got.entrances.len(), want.entrances.len()));
        }
    }
    // Laid anew from the wall lines.
    let mut file = hw.file.clone();
    for li in first.keys() {
        if let Some(o) = file.edits.streets.get_mut(&li.to_string()).and_then(|v| v.as_object_mut()) {
            o.remove("works");
        }
    }
    let fw = World::new(file).unwrap();
    for (&li, want) in originals.iter().filter(|(_, l)| !l.walls.is_empty() && !l.towers.is_empty()) {
        let Some(b) = hand::build_hand(&fw, t0, li) else { continue };
        r.tally("laid anew: walled towns", 1);
        let (t, g) = (b.layout.towers.len() as i64 - want.towers.len() as i64, b.layout.gates.len() as i64 - want.gates.len() as i64);
        r.tally(if t >= 0 { "laid anew: towers more" } else { "laid anew: towers fewer" }, t.unsigned_abs() as usize);
        r.tally(if g >= 0 { "laid anew: gates more" } else { "laid anew: gates fewer" }, g.unsigned_abs() as usize);
        // (Of them, a castle's own: within its curtain's reach.)
        let castled = |l: &town::Layout| l.towers.iter().filter(|t| want.castles.iter().any(|c| (t[0] - c.0[0]).hypot(t[1] - c.0[1]) < c.1 + 12.0)).count();
        r.tally("laid anew: castles' towers (generated)", castled(want));
        r.tally("laid anew: castles' towers", castled(&b.layout));
    }
    hand_remade(world, t0, label, hw, originals, r);
}

/// Roadmap 6 P3b2: the converted towns kept through a remake (`remap::remake`, the world remade
/// with a ruin drawn far from every town): each hand town, found by its header's origin, lays out
/// the same (buildings, walls, towers, gates, bridges, squares). Then the battlemap guarantee on
/// chunks of the three largest converted towns (every dry square near cover).
pub(crate) fn hand_remade(world: &World, t0: &T0, label: &str, hw: &World, originals: &std::collections::BTreeMap<usize, std::rc::Rc<Layout>>, r: &mut Report) {
    use worldgen::t0::remap;
    r.counts(&["remake: hand towns", "remake: hand towns laid otherwise", "remake: hand towns lost", "hand battlemap: chunks", "hand battlemap: chunks with no building", "hand battlemap: squares far from cover"]);
    let (w, h) = (world.geom.map_w_ft, world.geom.map_h_ft);
    let sea = world.params().sea_level_ft;
    let far = (0..400)
        .map(|k| [(0.1 + 0.8 * (k % 20) as f64 / 19.0) * w, (0.1 + 0.8 * (k / 20) as f64 / 19.0) * h])
        .filter(|p| t0.sample(p[0], p[1], 5.0) > sea + 20.0)
        .map(|p| (t0.settlements.iter().map(|s| dist(p, [s.x, s.y])).fold(f64::MAX, f64::min), p))
        .max_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, p)) = far {
        let mut b = hw.file.clone();
        b.sketch.strokes.push(serde_json::from_value(serde_json::json!({ "tool": "site", "kind": "ruin", "pts": [p] })).unwrap());
        let new = World::new(b).unwrap();
        let (t1, followed) = remap::remake(hw, t0, &new, &[]);
        let mut nf = new.file.clone();
        if let Some(f) = followed {
            nf.edits = f.edits;
        }
        let nw = World::new(nf).unwrap();
        let laid = |l: &Layout| format!("{:?} {:?} {:?} {:?} {:?} {:?}", l.buildings, l.walls, l.towers, l.gates, l.bridges, l.plazas);
        for &li in originals.keys() {
            r.tally("remake: hand towns", 1);
            let origin = &hw.file.edits.streets[&li.to_string()]["origin"];
            let to = nw.file.edits.streets.iter().filter(|(k, _)| !k.contains('/')).find(|(_, v)| v["origin"] == *origin).and_then(|(k, _)| k.parse::<usize>().ok());
            match to {
                None => r.add("remake: hand towns lost", 0.0, format!("{label} L{li}")),
                Some(to) => {
                    let (a, b) = (town::base_layout(hw, t0, li), town::base_layout(&nw, &t1, to));
                    if laid(&a) != laid(&b) {
                        r.add("remake: hand towns laid otherwise", 0.0, format!("{label} L{li} (now L{to}): {} buildings against {}", b.buildings.len(), a.buildings.len()));
                    }
                }
            }
        }
    }
    // The battlemap guarantee on hand towns.
    let mut ex = Executor::new(World::new(hw.file.clone()).unwrap());
    let mut big: Vec<(usize, usize)> = originals.iter().map(|(&li, l)| (l.buildings.len(), li)).collect();
    big.sort_by(|a, b| b.cmp(a));
    let size = ex.world.geom.tile_size_ft(ex.world.geom.max_level);
    for &(_, li) in big.iter().take(3) {
        let l = town::base_layout(&ex.world, &ex.t0, li);
        let mut at: Vec<[f64; 2]> = vec![l.center];
        at.extend(l.buildings.iter().step_by((l.buildings.len() / 3).max(1)).take(3).map(|b| centroid(&b.poly)));
        at.extend(l.towers.first().copied());
        for p in at {
            let key = TileKey::surface(ex.world.geom.max_level, (p[0] / size) as u32, (p[1] / size) as u32);
            let c = ex.battlemap(key);
            r.tally("hand battlemap: chunks", 1);
            if c.building.iter().all(|&b| b == 0) {
                r.add("hand battlemap: chunks with no building", 0.0, format!("{label} L{li} {key:?}"));
            }
            let gap = worldgen::battlemap::cover_distance(&c);
            let far = (0..SQ * SQ).filter(|&k| c.water_level[k] <= c.height[k] && c.surface[k] != Surface::Planks && gap[k] > worldgen::battlemap::cover_gap(&c, k)).count();
            if far > 0 {
                r.add("hand battlemap: squares far from cover", far as f64, format!("{label} L{li} {key:?}: {far}"));
            }
        }
    }
}

/// Roadmap 6 P3b1: every converted town (`first`: its records) built from its records alone
/// (`hand::build_hand`, all in one world with their wharves): its buildings against the town's
/// (missing, extra, differing by field), its blocks' squares, parks, monuments and fields
/// against the town's by kind. Then every block filled anew (its kept buildings dropped, a
/// generated fill as its ward): nothing outside its block, overlapping, or over a street.
pub(crate) fn hand_built(world: &World, t0: &T0, label: &str, first: &std::collections::BTreeMap<usize, String>, r: &mut Report) {
    use std::collections::BTreeMap as Map;
    use worldgen::town::clip::{self, Ends, Join, Shape};
    use worldgen::town::hand;
    r.counts(&["hand towns built", "hand: buildings missing", "hand: buildings extra", "hand: buildings differing", "hand: problems"]);
    r.counts(&["hand: squares", "hand: squares differing", "hand: parks", "hand: parks differing", "hand: parks of no block (not rebuilt)", "hand: monuments", "hand: monuments missing", "hand: monuments extra", "hand: fields", "hand: fields missing", "hand: fields extra"]);
    r.counts(&["hand: harbour fields", "hand: harbour fields differing", "hand: props differing (not kept yet)", "hand: places", "hand: places differing"]);
    r.counts(&["refill: blocks", "refill: buildings", "refill: row buildings", "refill: outside the block", "refill: overlapping", "refill: over a street"]);
    let key = |pts: &[[f64; 2]]| format!("{pts:?}");
    let mut file = world.file.clone();
    for json in first.values() {
        let recs: Vec<(String, serde_json::Value)> = serde_json::from_str(json).unwrap();
        file.edits.streets.extend(recs);
    }
    let hw = World::new(file.clone()).unwrap();
    let originals: Map<usize, std::rc::Rc<Layout>> = first.keys().map(|&li| (li, town::base_layout(world, t0, li))).collect();
    for (&li, want) in &originals {
        let Some(b) = hand::build_hand(&hw, t0, li) else {
            r.add("hand: problems", 0.0, format!("{label} L{li}: not built"));
            continue;
        };
        r.tally("hand towns built", 1);
        for (k, why) in &b.problems {
            r.add("hand: problems", 0.0, format!("{label} {k}: {why}"));
        }
        let got = &b.layout;
        let have: Map<u32, &town::Building> = got.buildings.iter().map(|b| (b.id, b)).collect();
        for w in &want.buildings {
            match have.get(&w.id) {
                None => r.add("hand: buildings missing", 0.0, format!("{label} L{li} b{}", w.id)),
                Some(h) => {
                    let d = hand::differs(h, w);
                    if !d.is_empty() {
                        r.add("hand: buildings differing", 0.0, format!("{label} L{li} b{}: {}", w.id, d.join(" ")));
                    }
                }
            }
        }
        let ids: std::collections::BTreeSet<u32> = want.buildings.iter().map(|b| b.id).collect();
        for b in got.buildings.iter().filter(|b| !ids.contains(&b.id)) {
            r.add("hand: buildings extra", 0.0, format!("{label} L{li} b{}", b.id));
        }
        // Squares and parks by their block.
        let patch = |s: &worldgen::town::places::Spot| matches!(s, worldgen::town::places::Spot::Patch(_));
        let sq = |l: &Layout| l.plazas.iter().filter(|p| patch(&p.spot)).map(|p| (format!("{:?}", p.spot), format!("{:?} {}", p.kind, key(&p.pts)))).collect::<Map<String, String>>();
        let (sw, sg) = (sq(want), sq(got));
        r.tally("hand: squares", sw.len());
        for (spot, v) in &sw {
            if sg.get(spot) != Some(v) {
                r.add("hand: squares differing", 0.0, format!("{label} L{li} {spot}: {}", if sg.contains_key(spot) { "otherwise" } else { "missing" }));
            }
        }
        let pk = |l: &Layout| l.parks.iter().filter(|p| patch(&p.spot)).map(|p| (format!("{:?}", p.spot), format!("{p:?}"))).collect::<Map<String, String>>();
        let (pw, pg) = (pk(want), pk(got));
        r.tally("hand: parks", pw.len());
        for p in want.parks.iter().filter(|p| !patch(&p.spot)) {
            if !got.parks.iter().any(|q| format!("{q:?}") == format!("{p:?}")) {
                r.add("hand: parks of no block (not rebuilt)", 0.0, format!("{label} L{li} {:?} {:?}", p.spot, p.kind));
            }
        }
        for (spot, v) in &pw {
            if pg.get(spot) != Some(v) {
                r.add("hand: parks differing", 0.0, format!("{label} L{li} {spot}: {}", if pg.contains_key(spot) { "otherwise" } else { "missing" }));
            }
        }
        for (what, w, g) in [("monuments", &want.monuments, &got.monuments), ("fields", &want.fields, &got.fields)] {
            let bag = |v: &Vec<Vec<[f64; 2]>>| {
                let mut m: Map<String, i32> = Map::new();
                v.iter().for_each(|p| *m.entry(key(p)).or_default() += 1);
                m
            };
            let (bw, bg) = (bag(w), bag(g));
            r.tally(&format!("hand: {what}"), w.len());
            let missing: i32 = bw.iter().map(|(k, n)| (n - bg.get(k).copied().unwrap_or(0)).max(0)).sum();
            let extra: i32 = bg.iter().map(|(k, n)| (n - bw.get(k).copied().unwrap_or(0)).max(0)).sum();
            if missing > 0 {
                r.add(&format!("hand: {what} missing"), missing as f64, format!("{label} L{li}: {missing} of {}", w.len()));
            }
            if extra > 0 {
                r.add(&format!("hand: {what} extra"), extra as f64, format!("{label} L{li}: {extra} of {}", g.len()));
            }
        }
        // P4: the harbour (piers, breakwaters, boats, port, props) and the places' names and labels
        // as the town drew them before it was laid out by hand (the layouts the map draws).
        let (lw, lg) = (town::layout(world, t0, li), town::layout(&hw, t0, li));
        let props = |l: &Layout| l.props.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>();
        for (what, same) in [
            ("piers", lw.piers == lg.piers),
            ("breakwaters", lw.moles == lg.moles),
            ("boats", lw.moored == lg.moored),
            ("port", lw.port == lg.port),
        ] {
            r.tally("hand: harbour fields", 1);
            if !same {
                r.add("hand: harbour fields differing", 0.0, format!("{label} L{li} {what}"));
            }
        }
        // (Props: yurts' and sites' are not kept by conversion yet, P3's known limit.)
        if props(&lw) != props(&lg) {
            r.add("hand: props differing (not kept yet)", 0.0, format!("{label} L{li}: {} before, {} after", lw.props.len(), lg.props.len()));
        }
        // (Bridges within 1 ft, corner by corner: kept decks are stored to 1/64 ft.)
        let near = |a: &Vec<[f64; 2]>, b: &Vec<[f64; 2]>| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p[0] - q[0]).abs() < 1.0 && (p[1] - q[1]).abs() < 1.0);
        let gone = lw.bridges.iter().filter(|d| !lg.bridges.iter().any(|e| near(d, e))).count();
        let new = lg.bridges.iter().filter(|d| !lw.bridges.iter().any(|e| near(d, e))).count();
        r.tally("hand: harbour fields", 1);
        if gone + new > 0 {
            r.add("hand: harbour fields differing", 0.0, format!("{label} L{li} bridges: {gone} of {} gone, {new} new", lw.bridges.len()));
        }
        let pl = |w: &World, l: &Layout| worldgen::town::places::of(w, t0, l).into_iter().map(|p| (p.id.clone(), format!("{} {:?}", p.name, p.label))).collect::<Map<String, String>>();
        let (pw, pg) = (pl(world, &lw), pl(&hw, &lg));
        r.tally("hand: places", pw.len());
        for (id, v) in &pw {
            if pg.get(id) != Some(v) {
                r.add("hand: places differing", 0.0, format!("{label} L{li} {id}: {}", if pg.contains_key(id) { "otherwise" } else { "missing" }));
            }
        }
    }
    hand_works(world, t0, label, first, &hw, &originals, r);
    // Every block filled anew, and a row of lots along each side of each town's longest street
    // (clear of the blocks).
    for (_, v) in file.edits.streets.iter_mut() {
        if let Some(o) = v.as_object_mut() {
            o.remove("kept");
        }
    }
    for &li in first.keys() {
        let recs = hand::HandRecords::of(&file.edits.streets, li);
        let long = recs.streets.iter().filter_map(|(id, st)| Some((*id, worldgen::town::geom::dist(recs.junctions.get(&st.a)?.p, recs.junctions.get(&st.b)?.p)))).max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)));
        if let Some((id, _)) = long {
            for (k, side) in ["l", "r"].iter().enumerate() {
                let row = serde_json::json!({"street": id, "side": side, "depth": 40.0, "ward": "common", "gaps": 0.08, "seed": format!("{:016x}", 0x5eed + li * 2 + k)});
                file.edits.streets.insert(format!("{li}/r{k}"), row);
            }
        }
    }
    let rw = World::new(file).unwrap();
    for &li in first.keys() {
        let Some(b) = hand::build_hand(&rw, t0, li) else { continue };
        let blocks: Map<u32, &Vec<Shape>> = b.blocks.iter().map(|(id, g)| (*id, g)).collect();
        r.tally("refill: blocks", blocks.len());
        if let Ok(dbg) = std::env::var("HAND_DBG")
            && let Some((l0, k0)) = dbg.split_once(':')
            && l0 == li.to_string()
        {
            let k0: u32 = k0.parse().unwrap();
            let o = b.layout.center;
            let rel = |p: &[f64; 2]| format!("({:.1},{:.1})", p[0] - o[0], p[1] - o[1]);
            for sh in blocks.get(&k0).into_iter().flat_map(|g| g.iter()) {
                say!("      ground outer {}", sh.outer.iter().map(rel).collect::<Vec<_>>().join(" "));
            }
            let recs = hand::HandRecords::of(&rw.file.edits.streets, li);
            for (id, st) in &recs.streets {
                let (a, z) = (&recs.junctions[&st.a], &recs.junctions[&st.b]);
                if std::env::var("HAND_E").is_ok_and(|e| e == id.to_string()) {
                    say!("      e{id} {} -> {} set {} {} {:?}", rel(&a.p), rel(&z.p), st.set_l, st.set_r, st.class);
                }
            }
            for bd in b.layout.buildings.iter().filter(|x| x.id >= hand::HAND_ID && (x.id - hand::HAND_ID) / (worldgen::town::wards::PER_PATCH * worldgen::town::wards::ROLLS) == k0).take(12) {
                say!("      b{} {}", bd.id, bd.poly.iter().map(rel).collect::<Vec<_>>().join(" "));
            }
        }
        let recs = hand::HandRecords::of(&rw.file.edits.streets, li);
        let per = worldgen::town::wards::PER_PATCH * worldgen::town::wards::ROLLS;
        let new: Vec<&town::Building> = b.layout.buildings.iter().filter(|b| b.id >= hand::HAND_ID).collect();
        r.tally("refill: buildings", new.len());
        r.tally("refill: row buildings", new.iter().filter(|b| (b.id - hand::HAND_ID) / per >= hand::ROW_BLOCK).count());
        // (A 64-ft grid of the buildings, for the pairs to look at.)
        let cell = |p: [f64; 2]| ((p[0] / 64.0).floor() as i64, (p[1] / 64.0).floor() as i64);
        let bbox = |pts: &[[f64; 2]]| pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]);
        let mut grid: Map<(i64, i64), Vec<usize>> = Map::new();
        for (k, bd) in new.iter().enumerate() {
            let bb = bbox(&bd.poly);
            let (c0, c1) = (cell([bb[0], bb[1]]), cell([bb[2], bb[3]]));
            for x in c0.0..=c1.0 {
                for y in c0.1..=c1.1 {
                    grid.entry((x, y)).or_default().push(k);
                }
            }
        }
        for (k, bd) in new.iter().enumerate() {
            let blk = (bd.id - hand::HAND_ID) / per;
            let poly = clip::region(&bd.poly);
            let out = if blk >= hand::ROW_BLOCK { 0.0 } else { blocks.get(&blk).map_or(clip::area(&poly), |g| clip::area(&clip::difference(&poly, g))) };
            if out > 1.0 {
                r.add("refill: outside the block", out, format!("{label} L{li} b{} (block {blk}): {out:.1} sq ft", bd.id));
            }
            let bb = bbox(&bd.poly);
            let (c0, c1) = (cell([bb[0], bb[1]]), cell([bb[2], bb[3]]));
            let mut near: Vec<usize> = (c0.0..=c1.0).flat_map(|x| (c0.1..=c1.1).map(move |y| (x, y))).filter_map(|c| grid.get(&c)).flatten().copied().filter(|&j| j > k).collect();
            near.sort_unstable();
            near.dedup();
            for j in near {
                let o = clip::area(&clip::intersection(&poly, &clip::region(&new[j].poly)));
                if o > 1.0 {
                    r.add("refill: overlapping", o, format!("{label} L{li} b{} b{}: {o:.1} sq ft", bd.id, new[j].id));
                }
            }
        }
        for (id, st) in &recs.streets {
            let (Some(a), Some(z)) = (recs.junctions.get(&st.a), recs.junctions.get(&st.b)) else { continue };
            let line: Vec<[f64; 2]> = std::iter::once(a.p).chain(st.bends.iter().copied()).chain([z.p]).collect();
            if st.set_l + st.set_r <= 0.0 {
                continue;
            }
            // (Its paved way: up to 10 ft each side of its line, as far as the side stands back; not
            // the last 12 ft to each junction, where the blocks of other streets meeting there come
            // within that of its line.)
            let mut line = line;
            let trim = |line: &mut Vec<[f64; 2]>| {
                let (a, b) = (line[0], line[1]);
                let d = ((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1])).sqrt();
                if d <= 24.0 {
                    return false;
                }
                line[0] = [a[0] + (b[0] - a[0]) * 12.0 / d, a[1] + (b[1] - a[1]) * 12.0 / d];
                true
            };
            if !trim(&mut line) {
                continue;
            }
            line.reverse();
            let n = line.len();
            if n < 2 || !trim(&mut line) {
                continue;
            }
            let buf = clip::buffer(&line, st.set_r.min(10.0), st.set_l.min(10.0), Join::Miter(2.0), Ends::Flat);
            let bb = bbox(&line);
            let (c0, c1) = (cell([bb[0] - 20.0, bb[1] - 20.0]), cell([bb[2] + 20.0, bb[3] + 20.0]));
            let mut near: Vec<usize> = (c0.0..=c1.0).flat_map(|x| (c0.1..=c1.1).map(move |y| (x, y))).filter_map(|c| grid.get(&c)).flatten().copied().collect();
            near.sort_unstable();
            near.dedup();
            for j in near.into_iter().filter(|&j| (new[j].id - hand::HAND_ID) / per < hand::ROW_BLOCK) {
                let o = clip::area(&clip::intersection(&buf, &clip::region(&new[j].poly)));
                if o > 1.0 {
                    r.add("refill: over a street", o, format!("{label} L{li} b{} e{id}: {o:.1} sq ft", new[j].id));
                }
            }
        }
    }
}

/// `hand_towns` for one town converted (`recs`): its kept buildings against its layout's, and
/// (`sizes`) its records' sizes.
pub(crate) fn hand_one(world: &World, t0: &T0, li: usize, recs: &[(String, serde_json::Value)], label: &str, sizes: bool, r: &mut Report) {
    use std::collections::BTreeMap as Map;
    use worldgen::town::hand;
    let map: Map<String, serde_json::Value> = recs.iter().cloned().collect();
    let hr = hand::HandRecords::of(&map, li);
    let town = hr.town.clone().expect("a header");
    let ground = hand::KeptGround::of(world, t0, li, &town);
    let l = town::base_layout(world, t0, li);
    let mut got = Vec::new();
    let (mut by_fill, mut whole) = (0, 0);
    for kept in hr.blocks.values().filter_map(|b| b.kept.as_ref()).chain(town.kept.as_ref()) {
        by_fill += kept.runs.iter().map(|r| r[2] as usize).sum::<usize>();
        whole += kept.full.len();
        match hand::kept_on(&ground, kept) {
            Ok(bs) => got.extend(bs),
            Err(e) => r.add("towns refused", 0.0, format!("{label} L{li}: kept: {e}")),
        }
    }
    r.tally("buildings kept by their fill", by_fill);
    r.tally("buildings stored whole", whole);
    r.tally("buildings", l.buildings.len());
    let want: Map<u32, &town::Building> = l.buildings.iter().map(|b| (b.id, b)).collect();
    let have: Map<u32, &town::Building> = got.iter().map(|b| (b.id, b)).collect();
    for (id, b) in &want {
        match have.get(id) {
            None => r.add("buildings missing", 0.0, format!("{label} L{li} b{id}")),
            Some(k) => {
                for f in hand::differs(k, b) {
                    r.add(&format!("buildings differing: {f}"), 0.0, format!("{label} L{li} b{id}"));
                }
            }
        }
    }
    if got.len() != have.len() {
        r.add("buildings extra", 0.0, format!("{label} L{li}: {} ids twice", got.len() - have.len()));
    }
    for id in have.keys().filter(|id| !want.contains_key(id)) {
        r.add("buildings extra", 0.0, format!("{label} L{li} b{id}"));
    }
    if std::env::var("HAND_WHY").is_ok_and(|w| w == li.to_string()) {
        let mut by: Map<String, (usize, usize)> = Map::new();
        for (k, v) in recs {
            let kind = k.split('/').nth(1).map_or("header".to_string(), |r| r[..1].to_string());
            let all = serde_json::to_string(v).unwrap().len() + k.len() + 4;
            let kept = v.get("kept").map_or(0, |x| serde_json::to_string(x).unwrap().len());
            for (what, n) in [(kind.clone(), all - kept), (format!("{kind} kept"), kept)] {
                let e = by.entry(what).or_default();
                e.0 += 1;
                e.1 += n;
            }
        }
        for (what, (n, bytes)) in &by {
            say!("      bytes {what}: {bytes} in {n}");
        }
        if let Some(w) = recs.iter().find(|(k, _)| *k == li.to_string()).and_then(|(_, v)| v.get("works")).and_then(|w| w.as_object()) {
            say!("      works: {}", w.iter().map(|(k, v)| format!("{k} {}", v.to_string().len())).collect::<Vec<_>>().join(", "));
        }
        let kepts: Vec<&hand::Kept> = hr.blocks.values().filter_map(|b| b.kept.as_ref()).chain(town.kept.as_ref()).collect();
        let full: usize = kepts.iter().flat_map(|k| &k.full).map(|f| serde_json::to_string(f).unwrap().len()).sum();
        let refill: usize = kepts.iter().filter_map(|k| k.refill.as_ref()).map(|f| serde_json::to_string(f).unwrap().len()).sum();
        let set: usize = kepts.iter().flat_map(|k| &k.set).map(|f| serde_json::to_string(f).unwrap().len()).sum();
        say!("      kept bytes: whole {full}, refills {refill}, set {set}");
        let mut why: Map<String, usize> = Map::new();
        for f in kepts.iter().flat_map(|k| &k.full) {
            let b = want[&f.i];
            *why.entry(format!("{:?} func {:?} parts {} {:?}", b.ward, b.func.map(|f| town::catalog::CATALOG[f as usize].key), b.parts.len(), b.structure)).or_default() += 1;
        }
        let mut v: Vec<_> = why.into_iter().collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.1));
        for (w, n) in v.iter().take(12) {
            say!("      {n} stored whole: {w}");
        }
    }
    if sizes {
        let size = hand::size_of(recs) as f64 / 1024.0;
        let kept_kb = recs.iter().filter_map(|(k, v)| v.get("kept").map(|x| k.len() + serde_json::to_string(x).unwrap().len())).sum::<usize>() as f64 / 1024.0;
        let tier = ["village", "town", "city", "metropolis"][t0.settlements[li].tier as usize];
        let at = format!("{label} L{li}, {} buildings, {} records", l.buildings.len(), recs.len());
        r.value(&format!("{tier}: KB of records"), size, at.clone());
        r.value(&format!("{tier}: KB of it kept buildings"), kept_kb, at);
        r.value(&format!("{tier}: bytes a building"), size * 1024.0 / l.buildings.len().max(1) as f64, format!("{label} L{li}"));
    }
}

/// The street editor's scripted edit of town `ci` (`same`, `hand`): three corners moved, a patch
/// made noble with large lots rolled again, its walls turned over. None if it changes nothing;
/// the reason and the request if refused.
pub(crate) fn scripted_edit(world: &World, t0: &T0, ci: usize) -> Option<Result<worldgen::world::TownEdit, (String, String)>> {
    let plan = town::wards::plan_json(world, t0, ci).ok()?;
    let patches = plan["patches"].as_array().cloned().unwrap_or_default();
    let inner: Vec<&serde_json::Value> = patches.iter().filter(|p| p["in_town"] == true).collect();
    // Three corners of patches in town that are no squares (the first that may move), each moved 35 ft
    // east and 25 ft south.
    let mut moves: Vec<String> = Vec::new();
    for v in inner.iter().filter(|p| p["ward"] != "plaza").flat_map(|p| p["corners"].as_array().unwrap().iter()) {
        let m = format!(r#"{{"corner":{v},"by":[35,-25]}}"#);
        if moves.len() < 3 && !moves.contains(&m) && town::wards::change(world, t0, ci, &serde_json::from_str(&format!(r#"{{"moves":[{m}]}}"#)).unwrap()).is_ok() {
            moves.push(m);
        }
    }
    // The first common, craft or merchant patch made a noble one's, its lots large, rolled again.
    let ward = inner.iter().find(|p| matches!(p["ward"].as_str(), Some("common" | "craft" | "merchant"))).map(|p| format!(r#","patches":[{{"patch":{},"ward":"noble","lots":"large","reroll":true}}]"#, p["patch"])).unwrap_or_default();
    let walls = if plan["walls"]["built"] == true { r#","walls":false"# } else { r#","walls":true"# };
    let req = format!(r#"{{"moves":[{}]{ward}{walls}}}"#, moves.join(","));
    match town::wards::change(world, t0, ci, &serde_json::from_str(&req).unwrap()) {
        Ok((Some(edit), _)) => Some(Ok(edit)),
        Ok((None, _)) => None,
        Err(e) => Some(Err((e, req))),
    }
}
