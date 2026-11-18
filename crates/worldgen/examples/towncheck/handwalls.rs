use crate::prelude::*;

// R5 towns
/// Walls drawn by hand round patches (M18): per tier up to 3 towns, a ring round 3-6 patches
/// near the middle, one round the whole old town (its own walls taken down if it has them), an
/// open line across it. Rules (each should be 0): a road, approach or main street crossing with
/// no gate; a ring with no way in; a building cut by a hand wall; a ring not closed; the town's
/// own walls on or off changing a hand wall. Then gate counts.
pub(crate) fn handwalls(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::t0::settle::Tier;
    use worldgen::town::handwall;
    use worldgen::world::{Created, Edits, WorldFile};
    const RULES: [&str; 5] = ["crossing with no gate", "ring with no way in", "building cut by a hand wall", "ring not closed", "hand wall changed by town walls on/off"];
    r.counts(&RULES);
    r.counts(&["hand walls refused"]);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let n = t0.settlements.len();
    let mut towns: Vec<usize> = Vec::new();
    for tier in [Tier::Town, Tier::City, Tier::Metropolis] {
        towns.extend((0..n).filter(|&i| t0.settlements[i].tier == tier).take(3));
    }
    // (The towns with walls, taken down for the ring round the old town; toggled for the last rule.)
    let toggle = |ci: usize, on: bool| town::wards::change(base, t0, ci, &serde_json::from_str(&format!(r#"{{"walls":{on}}}"#)).unwrap()).ok().and_then(|x| x.0);
    let mut off = base.file.edits.clone();
    for &ci in &towns {
        let walled = town::wards::plan_json(base, t0, ci).is_ok_and(|p| p["walls"]["built"] == true);
        if walled && let Some(e) = toggle(ci, false) {
            off.towns.insert(town::wards::key(ci), e);
        }
    }
    let w1 = World::new(WorldFile { edits: off.clone(), ..base.file.clone() }).unwrap();
    // (case label, town, wall, main crossings, sides with no street)
    let mut cases: Vec<(String, usize, Created, Vec<P>)> = Vec::new();
    let mut bare: HashMap<String, Vec<(P, P)>> = HashMap::new();
    for &ci in &towns {
        let s = &t0.settlements[ci];
        let Ok(plan) = town::wards::plan_json(&w1, t0, ci) else { continue };
        let patches: Vec<&serde_json::Value> = plan["patches"].as_array().unwrap().iter().filter(|p| p["in_town"] == true).collect();
        let num = |p: &serde_json::Value| p["patch"].as_u64().unwrap() as usize;
        let at = |p: &serde_json::Value| [p["at"][0].as_f64().unwrap(), p["at"][1].as_f64().unwrap()];
        let Some(first) = patches.iter().min_by(|a, b| dist(at(a), [s.x, s.y]).total_cmp(&dist(at(b), [s.x, s.y]))) else { continue };
        let want = 3 + ci % 4;
        let mut few = vec![num(first)];
        let mut k = 0;
        while few.len() < want && k < few.len() {
            let p = patches.iter().find(|p| num(p) == few[k]).unwrap();
            for nb in p["neighbours"].as_array().unwrap().iter().filter_map(|v| v.as_u64()).map(|v| v as usize) {
                if few.len() < want && !few.contains(&nb) && patches.iter().any(|q| num(q) == nb) {
                    few.push(nb);
                }
            }
            k += 1;
        }
        let all: Vec<usize> = patches.iter().map(|p| num(p)).collect();
        for (label, set) in [("ring round patches", few), ("ring round the old town", all)] {
            match handwall::around_patches(&w1, t0, ci, &set, false) {
                Ok(ring) => {
                    let c = Created { kind: "wall".into(), pts: ring.pts.clone(), gates: ring.gates.clone(), closed: true, ..Default::default() };
                    let main = ring.crossings.iter().filter(|x| x.main).map(|x| x.at).collect();
                    r.tally(&format!("{label}: side streets closed"), ring.crossings.iter().filter(|x| !x.gate && !x.main).count());
                    let name = format!("{label} #{ci} ({} patches)", set.len());
                    let m = ring.pts.len();
                    bare.insert(name.clone(), ring.bare.iter().map(|&i| (ring.pts[i], ring.pts[(i + 1) % m])).collect());
                    cases.push((name, ci, c, main));
                }
                Err(e) => r.add("hand walls refused", 0.0, format!("{label} #{ci}: {e}")),
            }
        }
        // (An open line across it, the first way round that stays out of the water.)
        let half = 0.6 * town::reach(s).min(1200.0);
        for j in 0..16 {
            let ang = ((ci + j) % 16) as f64 * std::f64::consts::PI / 16.0;
            let d = [ang.cos() * half, ang.sin() * half];
            let mut c = Created { kind: "wall".into(), x: s.x, y: s.y, pts: vec![sub([s.x, s.y], d), add([s.x, s.y], d)], ..Default::default() };
            c.id = "c:0".into();
            if j == 15 || worldgen::agent::works_spot(&w1, t0, &c, "c:0", None).is_ok() {
                cases.push((format!("open line #{ci}"), ci, c, Vec::new()));
                break;
            }
        }
    }
    // Checked like a new one (`works_spot`); those it takes go in the world together.
    let mut kept: Vec<(String, usize, Created, Vec<P>)> = Vec::new();
    for (label, ci, mut c, main) in cases {
        let k = c.pts.len() as f64;
        (c.x, c.y) = (c.pts.iter().map(|q| q[0]).sum::<f64>() / k, c.pts.iter().map(|q| q[1]).sum::<f64>() / k);
        c.id = format!("c:{}", off.created.len() + kept.len());
        match c.check().and_then(|_| worldgen::agent::works_spot(&w1, t0, &c, &c.id, None)) {
            Err(e) => {
                r.add("hand walls refused", 0.0, format!("{label}: {e}"));
                continue;
            }
            // (Taken away with it: `remove_in_way`.)
            Ok(s) => {
                let kind = label.split(" #").next().unwrap();
                r.tally(&format!("{kind}: buildings in its way (remove_in_way)"), s.in_way.len());
                let sides = bare.get(&label).cloned().unwrap_or_default();
                for (id, at) in &s.in_way {
                    if kind != "open line" && !sides.iter().any(|&(a, q)| seg_dist(*at, a, q) < 40.0) {
                        r.add(&format!("{kind}: in its way off a side with no street"), 0.0, format!("{label}: {id} at ({:.0}, {:.0})", at[0], at[1]));
                    }
                }
                for (id, at) in s.in_way {
                    off.buildings.insert(id, worldgen::world::BuildingEdit { at, removed: true, ..Default::default() });
                }
            }
        }
        kept.push((label, ci, c, main));
    }
    let mut e3: Edits = off.clone();
    e3.created.extend(kept.iter().map(|k| k.2.clone()));
    let w3 = World::new(WorldFile { edits: e3.clone(), ..base.file.clone() }).unwrap();
    let mut t3 = T0::from_bytes(&t0.to_bytes()).unwrap();
    t3.apply_edits(&w3);
    let first = n + t3.base_pois + off.created.len();
    let look = |l: &Layout| format!("{:?}", (&l.walls, &l.towers, &l.gate_towers));
    let mut looks: Vec<String> = Vec::new();
    for (k, (label, ci, c, main)) in kept.iter().enumerate() {
        let l = town::layout(&w3, &t3, first + k);
        looks.push(look(&l));
        let gates = handwall::gates_of(&l);
        let kind = label.split(" #").next().unwrap();
        r.tally(&format!("{kind}: walls"), 1);
        r.tally(&format!("{kind}: gates"), gates.len());
        r.value(&format!("{kind}: gates each"), gates.len() as f64, label.clone());
        // (A gate's two towers as far apart as a generated gate's on a straight wall: 52 ft.)
        let gt = &l.gate_towers;
        for k in 1..gt.len() {
            let d = dist(gt[k - 1], gt[k]);
            if d < 60.0 {
                r.measure(&format!("{kind}: gate towers apart (ft), least"), true, d, format!("{label} at ({:.0}, {:.0})", gt[k][0], gt[k][1]));
            }
        }
        let mut crossings = handwall::road_crossings(&w3, &t3, &c.pts, c.closed);
        crossings.extend(main.iter().copied());
        r.tally(&format!("{kind}: road/approach/main crossings"), crossings.len());
        for q in &crossings {
            // (None in the water, where the wall is broken.)
            if !gates.iter().any(|g| dist(*g, *q) < 65.0) && l.walls.iter().any(|w| w.windows(2).any(|s| seg_dist(*q, s[0], s[1]) < 30.0)) {
                r.add("crossing with no gate", 0.0, format!("{label} at ({:.0}, {:.0})", q[0], q[1]));
            }
        }
        if c.closed {
            if gates.is_empty() {
                r.add("ring with no way in", 0.0, label.clone());
            }
            let m = c.pts.len();
            let ring: f64 = (0..m).map(|i| dist(c.pts[i], c.pts[(i + 1) % m])).sum();
            // (Every 10 ft of it on dry ground: on a wall piece or in a gate.)
            let (x0, y0) = c.pts.iter().fold((f64::MAX, f64::MAX), |a, p| (a.0.min(p[0]), a.1.min(p[1])));
            let (x1, y1) = c.pts.iter().fold((f64::MIN, f64::MIN), |a, p| (a.0.max(p[0]), a.1.max(p[1])));
            let shore = town::Shore::new(&w3, &t3, [x0, y0, x1, y1]);
            let mut short = 0.0;
            for i in 0..m {
                let (a, b) = (c.pts[i], c.pts[(i + 1) % m]);
                let k = (dist(a, b) / 10.0).ceil().max(1.0) as usize;
                for j in 0..k {
                    let p = lerp(a, b, (j as f64 + 0.5) / k as f64);
                    let q = worldgen::lod::rivers::clear_of_rivers(&t3.rivers, p[0], p[1], 6.0, t3.cell_ft);
                    let wet = shore.wet(p) || (q.0 - p[0]).abs() + (q.1 - p[1]).abs() > 0.01;
                    // (In a gate: between its towers, round the bend where one stands on a corner.)
                    let gt = &l.gate_towers;
                    let in_gate = (1..gt.len()).any(|k| dist(gt[k - 1], gt[k]) < 60.0 && dist(p, gt[k - 1]) + dist(p, gt[k]) < 128.0);
                    let laid = l.walls.iter().any(|w| w.windows(2).any(|s| seg_dist(p, s[0], s[1]) < 6.0)) || gates.iter().any(|g| dist(*g, p) < 32.0) || in_gate;
                    if !wet && !laid {
                        short += dist(a, b) / k as f64;
                    }
                }
            }
            if short > 40.0 {
                r.add("ring not closed", short, format!("{label}: {short:.0} ft of {ring:.0} missing"));
            }
        }
        // A building of its town the wall stands on (half its width, 4.5 ft, less a little).
        let town_l = town::layout(&w3, &t3, *ci);
        for b in &town_l.buildings {
            let m = b.poly.len();
            let hit = l.walls.iter().any(|w| {
                w.windows(2).any(|s| contains(&b.poly, s[0]) || (0..m).any(|i| seg_dist(b.poly[i], s[0], s[1]) < 4.0 || seg_dist(s[0], b.poly[i], b.poly[(i + 1) % m]) < 4.0 || segs_cross(s[0], s[1], b.poly[i], b.poly[(i + 1) % m])))
            });
            if hit {
                let on_bare = bare.get(label).is_some_and(|s| s.iter().any(|&(a, q)| b.poly.iter().any(|p| seg_dist(*p, a, q) < 8.0)));
                r.add(if on_bare { "building cut (a side with no street)" } else { "building cut by a hand wall" }, 0.0, format!("{label}: b:{ci}:{}", b.id));
            }
        }
    }
    // The towns' own walls toggled: each hand wall the same.
    let mut e4 = e3.clone();
    for &ci in &towns {
        let now = town::wards::plan_json(&w3, &t3, ci).is_ok_and(|p| p["walls"]["built"] == true);
        let w = World::new(WorldFile { edits: e3.clone(), ..base.file.clone() }).unwrap();
        if let Ok((Some(e), _)) = town::wards::change(&w, &t3, ci, &serde_json::from_str(&format!(r#"{{"walls":{}}}"#, !now)).unwrap()) {
            e4.towns.insert(town::wards::key(ci), e);
        }
    }
    let w4 = World::new(WorldFile { edits: e4, ..base.file.clone() }).unwrap();
    for (k, (label, ..)) in kept.iter().enumerate() {
        if look(&town::layout(&w4, &t3, first + k)) != looks[k] {
            r.add("hand wall changed by town walls on/off", 0.0, label.clone());
        }
    }
    let _ = toggle;
}

pub(crate) fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let (d1, d2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (d3, d4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Gates and towers put in a town's own wall by hand (M18b, `edit_town walls: {…}`): per tier, up

/// A refusal's reason whole, for a tally: its ids and corner numbers as `…`/`N`, what it says in
/// brackets left out.
pub(crate) fn reason(why: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut depth = 0i32;
    for w in why.split_whitespace() {
        let open = w.starts_with('(');
        if open {
            depth += 1;
        }
        let shut = w.ends_with(')') || w.ends_with("),") || w.ends_with(").");
        if depth == 0 {
            let bare = w.trim_end_matches([',', '.', ':', ';']);
            let id = bare.split(':').count() == 3 && bare.split(':').skip(1).all(|x| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit()));
            let after_corner = out.last().is_some_and(|p| matches!(p.as_str(), "corner" | "corners" | "and")) && bare.bytes().all(|c| c.is_ascii_digit());
            out.push(if id { format!("{}:…", bare.split(':').next().unwrap()) } else if after_corner { "N".into() } else { w.to_string() });
        }
        if shut && depth > 0 {
            depth -= 1;
        }
    }
    out.join(" ").trim_end_matches([':', ',']).to_string()
}
/// to 3 walled towns each get 2 gates and 3 towers, then lose one generated gate and one tower;
/// one generated gate renamed first. Rules (each 0): a hand gate with no way through (no track
/// from it, or one over a building), a hand gate not in the group's gatehouse form (a bent one may
/// fall back to a square tower, tallied), a hand tower over a building, street or another tower,
/// other `t:`/`gt:` ids moved or renames lost, walls off then on losing them.
pub(crate) fn wallgates(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::t0::settle::Tier;
    use worldgen::town::gatehouse;
    use worldgen::town::wallgates::{GATE_TOWER_ID, HAND_TOWER_ID, OTHER_TOWER_ID};
    const RULES: [&str; 8] = [
        "building by a gate taken out nearer the wall than the town's",
        "hand gate with no way through",
        "hand gate's track along the wall",
        "hand gate not in the group's form",
        "hand tower over a building, street or tower",
        "other ids moved",
        "renames lost",
        "walls off/on losing them",
    ];
    const FORMS: [&str; 5] = ["square tower", "twin drums", "arched", "bent", "log"];
    r.counts(&RULES);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let n = t0.settlements.len();
    let ask = |w: &World, ci: usize, j: &str| town::wards::change(w, t0, ci, &serde_json::from_str(j).unwrap());
    let with = |w: &World, ci: usize, e: Option<worldgen::world::TownEdit>| {
        let mut f = w.file.clone();
        match e {
            Some(e) => f.edits.towns.insert(town::wards::key(ci), e),
            None => f.edits.towns.remove(&town::wards::key(ci)),
        };
        World::new(f).unwrap()
    };
    let label_of = |tn: &str, ci: usize, v: u64| format!("{tn} #{ci} gate {v}");
    let added_pts = |g: &[(String, P, String)], added: &[usize]| -> Vec<P> { g.iter().filter(|x| added.iter().any(|v| x.0.ends_with(&format!(":{v}")))).map(|x| x.1).collect() };
    let towers_of = |l: &town::Layout| -> Vec<(usize, P, bool)> { worldgen::interior::towers(l).into_iter().enumerate().map(|(k, t)| (l.tower_id(k), t.0, t.2)).collect() };
    let gates_of = |w: &World, l: &town::Layout| -> Vec<(String, P, String)> { town::places::of(w, t0, l).into_iter().filter(|p| p.kind == "gate").map(|p| (p.id, p.at, p.name)).collect() };
    for tier in [Tier::Town, Tier::City, Tier::Metropolis] {
        let towns: Vec<usize> = (0..n).filter(|&i| t0.settlements[i].tier == tier && town::wards::plan_json(base, t0, i).is_ok_and(|p| p["walls"]["built"] == true)).take(3).collect();
        let tn = format!("{tier:?}");
        for ci in towns {
            let plan = town::wards::plan_json(base, t0, ci).unwrap();
            let corners = plan["corners"].as_array().unwrap();
            let wall: Vec<(usize, P, bool)> = corners.iter().filter(|c| c["wall"] == true).map(|c| (c["corner"].as_u64().unwrap() as usize, [c["at"][0].as_f64().unwrap(), c["at"][1].as_f64().unwrap()], c["gate"] == true)).collect();
            let gen_gates: Vec<usize> = wall.iter().filter(|c| c.2).map(|c| c.0).collect();
            if gen_gates.len() < 2 {
                r.tally(&format!("{tn}: towns with under 2 gates (skipped)"), 1);
                continue;
            }
            // A generated gate renamed (kept through everything).
            let mut f = base.file.clone();
            let kept = gen_gates[1];
            f.edits.renames.insert(format!("gt:{ci}:{kept}"), "Basilisk Gate".into());
            let mut w = World::new(f).unwrap();
            let l0 = town::layout(&w, t0, ci);
            let (t_before, g_before) = (towers_of(&l0), gates_of(&w, &l0));
            // Two gates in, tried corner by corner, spread round the wall.
            let mut added: Vec<usize> = Vec::new();
            let step = (wall.len() / 10).max(1);
            for k in (0..wall.len()).step_by(step).chain(0..wall.len()) {
                if added.len() >= 2 {
                    break;
                }
                let v = wall[k].0;
                if wall[k].2 || added.contains(&v) {
                    continue;
                }
                let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"gates_add":[{v}]}}}}"#)) else { continue };
                for x in rep["wall"]["refused"].as_array().into_iter().flatten() {
                    let why = x["why"].as_str().unwrap_or("");
                    r.tally(&format!("gates refused: {}", reason(why)), 1);
                }
                if rep["wall"]["gates_added"].as_array().is_some_and(|a| !a.is_empty()) {
                    added.push(v);
                    w = with(&w, ci, e);
                }
            }
            r.tally(&format!("{tn}: hand gates added"), added.len());
            // (How far the wall bends at each gate: generated and by hand.)
            let at_of = |v: u64| corners.iter().find(|c| c["corner"].as_u64() == Some(v)).map(|c| [c["at"][0].as_f64().unwrap(), c["at"][1].as_f64().unwrap()]);
            for c in corners.iter().filter(|c| c["wall"] == true) {
                let v = c["corner"].as_u64().unwrap();
                let hand = added.contains(&(v as usize));
                if !(hand || c["gate"] == true) {
                    continue;
                }
                let nb = &c["wall_next"];
                let (Some(a), Some(p), Some(b)) = (nb[0].as_u64().and_then(at_of), at_of(v), nb[1].as_u64().and_then(at_of)) else { continue };
                let (u, w) = (sub(a, p), sub(b, p));
                let cos = (u[0] * w[0] + u[1] * w[1]) / (dist(a, p) * dist(b, p)).max(1e-9);
                let bend = 180.0 - cos.clamp(-1.0, 1.0).acos().to_degrees();
                let who = if hand { "hand" } else { "generated" };
                r.measure(&format!("{tn}: wall's bend at a {who} gate (deg), most"), false, bend, label_of(&tn, ci, v));
                if bend > 20.0 {
                    r.tally(&format!("{tn}: {who} gates on a bend over 20 deg"), 1);
                }
                r.tally(&format!("{tn}: {who} gates looked at for bends"), 1);
            }
            // Three towers along the longest sides that take one.
            let mut sides: Vec<(f64, usize, usize)> = Vec::new();
            for (chain_k, c) in wall.iter().enumerate() {
                let next = &wall[(chain_k + 1) % wall.len()];
                let d = dist(c.1, next.1);
                sides.push((d, c.0, next.0));
            }
            sides.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut towers_added = 0;
            for &(_, a, b) in &sides {
                if towers_added >= 3 {
                    break;
                }
                let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"towers_add":[[{a},{b},0.5]]}}}}"#)) else { continue };
                for x in rep["wall"]["refused"].as_array().into_iter().flatten() {
                    let why = x["why"].as_str().unwrap_or("");
                    r.tally(&format!("towers refused: {}", reason(why)), 1);
                }
                if rep["wall"]["towers_added"].as_array().is_some_and(|a| !a.is_empty()) {
                    towers_added += 1;
                    w = with(&w, ci, e);
                }
            }
            r.tally(&format!("{tn}: hand towers added"), towers_added);
            // One generated gate out, one generated tower down.
            let gone_gate = gen_gates[0];
            let mut gate_out = false;
            if let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"gates_remove":[{gone_gate}]}}}}"#)) {
                if rep["wall"]["gates_removed"].as_array().is_some_and(|a| !a.is_empty()) {
                    gate_out = true;
                    w = with(&w, ci, e);
                }
            }
            let gone_tower = t_before.iter().find(|t| !t.2 && t.0 < HAND_TOWER_ID as usize && t_before.iter().all(|g| !g.2 || dist(g.1, t.1) > 80.0)).map(|t| t.0);
            if let Some(id) = gone_tower
                && let Ok((e, rep)) = ask(&w, ci, &format!(r#"{{"walls":{{"towers_remove":["t:{ci}:{id}"]}}}}"#))
                && rep["wall"]["towers_removed"].as_array().is_some_and(|a| !a.is_empty())
            {
                w = with(&w, ci, e);
            }
            let l = town::layout(&w, t0, ci);
            let (t_after, g_after) = (towers_of(&l), gates_of(&w, &l));
            let label = |what: &str| format!("{tn} #{ci} {what}");
            // The gate taken out: within 150 ft of it, no building nearer the wall than the town's
            // buildings kept there with the gate (its least), nor on the ground the gate kept clear
            // (40 ft round it; `clear_of`). (Castles aside: their own curtains.)
            if let Some(gone) = wall.iter().find(|c| c.0 == gone_gate).map(|c| c.1)
                && gate_out
            {
                let wall_of = |l: &town::Layout, poly: &[P]| l.walls.iter().flat_map(|w| w.windows(2)).map(|t| seg_poly(t[0], t[1], poly)).fold(f64::MAX, f64::min);
                let near = |b: &&town::Building| b.ward != Ward::Castle && b.poly.iter().any(|p| dist(*p, gone) < 150.0);
                let keep = l0.buildings.iter().filter(near).map(|b| wall_of(&l0, &b.poly)).fold(f64::MAX, f64::min);
                if keep < f64::MAX {
                    r.measure("wall clearance by a generated gate (ft, its buildings within 150 ft), least", true, keep, label(&format!("gate {gone_gate}")));
                }
                for b in l.buildings.iter().filter(near) {
                    let d = wall_of(&l, &b.poly);
                    let held = b.poly.iter().any(|p| dist(*p, gone) < 40.0) || contains(&b.poly, gone);
                    r.measure("building by a gate taken out: off the wall (ft), least", true, d, label(&format!("gate {gone_gate} b:{ci}:{}", b.id)));
                    if d < keep - 0.5 || held {
                        let why = if held { "on the gate's ground".to_string() } else { format!("{d:.0} ft off the wall, the town's {keep:.0}") };
                        r.add("building by a gate taken out nearer the wall than the town's", 1.0, label(&format!("gate {gone_gate} b:{ci}:{}: {why}", b.id)));
                    }
                }
            }
            // Hand gates: a way through, the group's form.
            let houses = gatehouse::houses(&l);
            for &v in &added {
                let Some((_, gp, _)) = g_after.iter().find(|g| g.0 == format!("gt:{ci}:{v}")) else {
                    r.add("hand gate with no way through", 1.0, label(&format!("gate {v} not built")));
                    continue;
                };
                let way = l.roads.iter().find(|x| x.1 == 2 && x.0.first().is_some_and(|p| dist(*p, *gp) < 2.0));
                match way {
                    None => r.add("hand gate with no way through", 1.0, label(&format!("gate {v}: no track out"))),
                    Some((pts, _, wd)) => {
                        r.measure("way out (ft), longest", false, pts.windows(2).map(|s| dist(s[0], s[1])).sum(), label(&format!("gate {v}")));
                        let over = l.buildings.iter().find(|b| {
                            let m = b.poly.len();
                            pts.windows(2).skip(1).any(|s| b.poly.iter().any(|q| seg_dist(*q, s[0], s[1]) < 0.5 * wd - 1.0) || (0..m).any(|i| seg_dist(lerp(s[0], s[1], 0.5), b.poly[i], b.poly[(i + 1) % m]) < 0.5 * wd - 1.0))
                        });
                        if let Some(b) = over {
                            r.add("hand gate with no way through", 1.0, label(&format!("gate {v}: track over b:{ci}:{}", b.id)));
                        }
                        // (Past its first 70 ft straight out: its edge 25 ft or more off the wall's
                        // outer face, but where it joins a road already there.)
                        let half_wall = 0.5 * l.wall_style.thickness();
                        let mut run = 0.0;
                        let mut worst: Option<(f64, P)> = None;
                        for sg in pts.windows(2) {
                            let d = dist(sg[0], sg[1]);
                            let steps = (d / 4.0).ceil().max(1.0) as usize;
                            for j in 0..steps {
                                let q = lerp(sg[0], sg[1], j as f64 / steps as f64);
                                let at = run + d * j as f64 / steps as f64;
                                if at < 72.0 {
                                    continue;
                                }
                                let by_road = l.roads.iter().filter(|x| !std::ptr::eq(&x.0, pts)).any(|x| x.0.windows(2).any(|t| seg_dist(q, t[0], t[1]) < 0.5 * x.2 + 0.5 * wd + 4.0));
                                let gap = l.walls.iter().flat_map(|w| w.windows(2)).map(|t| seg_dist(q, t[0], t[1])).fold(f64::MAX, f64::min) - half_wall - 0.5 * wd;
                                if !by_road && gap < 25.0 && worst.is_none_or(|w| gap < w.0) {
                                    worst = Some((gap, q));
                                }
                            }
                            run += d;
                        }
                        if let Some((gap, q)) = worst {
                            r.add("hand gate's track along the wall", 1.0, label(&format!("gate {v}: {gap:.0} ft off the wall at ({:.0}, {:.0})", q[0], q[1])));
                        }
                    }
                }
                match houses.iter().find(|h| dist(lerp(h.a, h.b, 0.5), *gp) < 20.0) {
                    None => r.add("hand gate not in the group's form", 1.0, label(&format!("gate {v}: no gatehouse"))),
                    Some(h) => {
                        r.measure("hand gate towers apart (ft), least", true, dist(h.a, h.b), label(&format!("gate {v}")));
                        r.tally(&format!("{tn}: hand gatehouses {}", FORMS[h.form as usize]), 1);
                        let want = l.wall_style.gatehouse;
                        if h.form != want {
                            if FORMS[want as usize] == "bent" && FORMS[h.form as usize] == "square tower" {
                                r.tally("bent hand gates fallen back to a square tower", 1);
                            } else {
                                r.add("hand gate not in the group's form", 1.0, label(&format!("gate {v}: {} not {}", FORMS[h.form as usize], FORMS[want as usize])));
                            }
                        }
                    }
                }
            }
            for h in houses.iter().filter(|h| !added_pts(&g_after, &added).iter().any(|p| dist(lerp(h.a, h.b, 0.5), *p) < 20.0)) {
                r.measure("generated gate towers apart (ft), least", true, dist(h.a, h.b), label("generated gate"));
            }
            // Hand towers: clear of buildings, streets crossing the wall and other towers.
            let all = worldgen::interior::towers(&l);
            for (k, &(id, p, _)) in t_after.iter().enumerate() {
                if !(HAND_TOWER_ID as usize..GATE_TOWER_ID as usize).contains(&id) {
                    continue;
                }
                let r0 = all[k].1;
                let over_b = l.buildings.iter().find(|b| {
                    let m = b.poly.len();
                    (0..m).any(|i| seg_dist(p, b.poly[i], b.poly[(i + 1) % m]) < r0 - 1.0)
                });
                let over_t = t_after.iter().enumerate().find(|(j, t)| *j != k && dist(t.1, p) < r0 + all[*j].1);
                let over_s = l.roads.iter().filter(|x| x.1 != 3).find(|x| x.0.windows(2).any(|s| seg_dist(p, s[0], s[1]) < r0 + 0.5 * x.2 - 2.0));
                if let Some(b) = over_b {
                    r.add("hand tower over a building, street or tower", 1.0, label(&format!("t:{ci}:{id} over b:{ci}:{}", b.id)));
                } else if let Some((_, t)) = over_t {
                    r.add("hand tower over a building, street or tower", 1.0, label(&format!("t:{ci}:{id} over t:{ci}:{} ({:.0} ft apart; it stood {:.0} ft off before)", t.0, dist(t.1, p), t_before.iter().find(|b| b.0 == t.0).map_or(-1.0, |b| dist(b.1, t.1)))));
                } else if over_s.is_some() {
                    r.add("hand tower over a building, street or tower", 1.0, label(&format!("t:{ci}:{id} over a road class {}", over_s.unwrap().1)));
                }
            }
            // Ids: every tower but those the edits took, where it stood; every gate but the one taken out.
            let added_at: Vec<P> = added.iter().filter_map(|v| wall.iter().find(|c| c.0 == *v).map(|c| c.1)).collect();
            let gone_at = wall.iter().find(|c| c.0 == gone_gate).map(|c| c.1).unwrap_or([0.0; 2]);
            // (Moved: along the wall's run; a curtain bowed anew may carry a tower out from it.)
            let runs = &l.hand_wall.runs;
            let along = |a: P, b: P| match (town::wallgates::run_place(runs, a), town::wallgates::run_place(runs, b)) {
                (Some(x), Some(y)) if x.0 == y.0 => (x.1 - y.1).abs(),
                _ => dist(a, b),
            };
            for &(id, p, gate) in &t_before {
                let meant = Some(id) == gone_tower || added_at.iter().any(|a| dist(*a, p) < 90.0) || (gate && dist(gone_at, p) < 40.0);
                match t_after.iter().find(|t| t.0 == id) {
                    Some(t) if along(t.1, p) > 60.0 => r.add("other ids moved", 1.0, label(&format!("t:{ci}:{id} moved {:.0} ft along the wall", along(t.1, p)))),
                    Some(t) if dist(t.1, p) > 1.0 => {
                        r.tally("towers shifted (their curtain bowed anew)", 1);
                        r.measure("tower shifted by a curtain bowed anew (ft), most", false, dist(t.1, p), label(&format!("t:{ci}:{id}")));
                    }
                    None if !meant => r.add("other ids moved", 1.0, label(&format!("t:{ci}:{id} gone (gate tower {gate}, {:.0} ft from the gate out; nearest now {:?})", dist(gone_at, p), t_after.iter().map(|t| (t.0, dist(t.1, p).round(), t.2)).min_by(|a, b| a.1.total_cmp(&b.1))))),
                    _ => {}
                }
            }
            for (id, p, _) in &g_before {
                if *id == format!("gt:{ci}:{gone_gate}") {
                    continue;
                }
                match g_after.iter().find(|g| g.0 == *id) {
                    Some(g) if dist(g.1, *p) > 1.0 => r.add("other ids moved", 1.0, label(&format!("{id} moved"))),
                    None => r.add("other ids moved", 1.0, label(&format!("{id} gone"))),
                    _ => {}
                }
            }
            if t_after.iter().any(|t| t.0 >= OTHER_TOWER_ID as usize) {
                r.add("other ids moved", 1.0, label(&format!("{} towers matched none", t_after.iter().filter(|t| t.0 >= OTHER_TOWER_ID as usize).count())));
            }
            if !g_after.iter().any(|g| g.0 == format!("gt:{ci}:{kept}")) || w.file.edits.renames.get(&format!("gt:{ci}:{kept}")).is_none_or(|n| n != "Basilisk Gate") {
                r.add("renames lost", 1.0, label(&format!("gt:{ci}:{kept}")));
            }
            r.tally(&format!("{tn}: towers before"), t_before.len());
            r.tally(&format!("{tn}: towers after"), t_after.len());
            // Walls off, then on: the same towers and gates.
            let w2 = ask(&w, ci, r#"{"walls":false}"#).map(|(e, _)| with(&w, ci, e)).ok();
            let w3 = w2.and_then(|w2| ask(&w2, ci, r#"{"walls":true}"#).map(|(e, _)| with(&w2, ci, e)).ok());
            match w3 {
                None => r.add("walls off/on losing them", 1.0, label("toggle refused")),
                Some(w3) => {
                    let l3 = town::layout(&w3, t0, ci);
                    let (t3, g3) = (towers_of(&l3), gates_of(&w3, &l3));
                    let ids = |t: &[(usize, P, bool)]| t.iter().map(|x| x.0).collect::<Vec<_>>();
                    let gids = |g: &[(String, P, String)]| g.iter().map(|x| (x.0.clone(), x.2.clone())).collect::<Vec<_>>();
                    if ids(&t3) != ids(&t_after) || gids(&g3) != gids(&g_after) {
                        r.add("walls off/on losing them", 1.0, label(&format!("towers {} -> {}, gates {} -> {}", t_after.len(), t3.len(), g_after.len(), g3.len())));
                    }
                }
            }
        }
    }
}

// R5 towns
/// Castle pieces (M6): a castle drawn by hand (`create_feature castle`, 400 x 300 ft) beside a
/// site drawn before it; its keep made a library and renamed, a yard building given a new trade,
/// one taken away, one given storeys; then the earlier site deleted, then taken out of the list
/// (the castle's layout number moves): the edits stay on the same castle. The same castle in
/// ruins: the keep's function and name set, storeys refused. A castle's keep in a generated town
/// takes `update_building` as before. The rules (each should be 0) are the edits not standing.
pub(crate) fn castle_pieces(ctx: &mut Ctx, r: &mut Report) {
    use worldgen::agent::{self, BuildingChange};
    use worldgen::town::pieces;
    use worldgen::world::{Created, WorldFile};
    const RULES: [&str; 8] = [
        "piece edit refused",
        "keep's function or name not as edited",
        "yard building's edit not standing",
        "other pieces changed by the edits",
        "edits not on the same castle after an earlier site went",
        "ruined keep took storeys or lost its function or name",
        "generated castle's keep refused or not changed",
        "id not told apart (wrong refusal)",
    ];
    r.counts(&RULES);
    let (base, t0) = (&ctx.ex.world, &ctx.ex.t0);
    let n = t0.settlements.len();
    let first = n + t0.base_pois;
    let key = |b: &town::Building| b.func.map_or("-", |f| town::catalog::CATALOG[f as usize].key);
    // A spot for the castle: off a town, clear of buildings, roads and water.
    let mut spot: Option<Created> = None;
    'find: for s in t0.settlements.iter().take(40) {
        for j in 0..8 {
            let a = j as f64 * std::f64::consts::FRAC_PI_4;
            let at = [s.x + 2600.0 * a.cos(), s.y + 2600.0 * a.sin()];
            let poly = vec![[at[0] - 200.0, at[1] - 150.0], [at[0] + 200.0, at[1] - 150.0], [at[0] + 200.0, at[1] + 150.0], [at[0] - 200.0, at[1] + 150.0]];
            let c = Created { id: "c:1".into(), kind: "castle".into(), x: at[0], y: at[1], name: "Candlekeep".into(), poly, ..Default::default() };
            if c.check().is_ok() && agent::works_spot(base, t0, &c, "c:1", None).is_ok_and(|w| w.in_way.is_empty()) {
                spot = Some(c);
                break 'find;
            }
        }
    }
    let Some(castle) = spot else {
        r.add("piece edit refused", 0.0, "no spot for a castle".into());
        return;
    };
    let tower = Created { id: "c:0".into(), kind: "tower".into(), x: castle.x + 3000.0, y: castle.y, name: "Earlier Tower".into(), ..Default::default() };
    let world_of = |created: Vec<Created>| {
        let mut e = base.file.edits.clone();
        e.created = created;
        let w = World::new(WorldFile { edits: e, ..base.file.clone() }).unwrap();
        let mut t = T0::from_bytes(&t0.to_bytes()).unwrap();
        t.apply_edits(&w);
        town::forget_from(first);
        (w, t)
    };
    for ruin in [false, true] {
        let label = if ruin { "ruined castle" } else { "castle" };
        let mut c = castle.clone();
        c.structure = ruin.then(|| "ruin".to_string());
        let (w1, t1) = world_of(vec![tower.clone(), c.clone()]);
        let li = first + 1;
        let l1 = town::layout(&w1, &t1, li);
        let Some(ki) = pieces::keep_index(&l1) else {
            r.add("piece edit refused", 0.0, format!("{label}: no keep"));
            continue;
        };
        let yard: Vec<u32> = l1.buildings.iter().enumerate().filter(|(i, _)| *i != ki).map(|(_, b)| b.id).collect();
        r.tally(&format!("{label}s: buildings"), l1.buildings.len());
        let kid = format!("b:{li}:{}", l1.buildings[ki].id);
        let change = |func: Option<&str>, floors: Option<u8>| BuildingChange { func: func.map(str::to_string), floors, ..Default::default() };
        // (keep: a library, renamed; yard 0: a temple; yard 1: taken away; yard 2: 3 storeys.)
        let mut asks: Vec<(String, BuildingChange, Option<&str>, bool)> = vec![(kid.clone(), change(Some("library"), None), Some("The Great Library"), false)];
        if let Some(&y) = yard.first() {
            asks.push((format!("b:{li}:{y}"), change(Some("temple"), None), None, false));
        }
        if let Some(&y) = yard.get(1) {
            asks.push((format!("b:{li}:{y}"), BuildingChange::default(), None, true));
        }
        if let Some(&y) = yard.get(2).filter(|_| !ruin) {
            asks.push((format!("b:{li}:{y}"), change(None, Some(3)), None, false));
        }
        for (id, ch, name, remove) in &asks {
            let Some(p) = pieces::piece_of(&w1, &t1, id) else {
                r.add("piece edit refused", 0.0, format!("{label} {id}: not a piece ({})", pieces::not_generated(&w1, &t1, id)));
                continue;
            };
            let e = if *remove { pieces::piece_removal(&p).map(Some) } else { pieces::piece_edit(&w1, &p, ch, *name) };
            match e {
                Ok(Some(e)) => drop(c.pieces.insert(p.key.clone(), e)),
                Ok(None) => r.add("piece edit refused", 0.0, format!("{label} {id}: no change")),
                Err(e) => r.add("piece edit refused", 0.0, format!("{label} {id}: {e}")),
            }
        }
        // (A ruin's keep takes no storeys; the keep can't be taken away.)
        if let Some(p) = pieces::piece_of(&w1, &t1, &kid) {
            if ruin != pieces::piece_edit(&w1, &p, &change(None, Some(4)), None).is_err() {
                r.add("ruined keep took storeys or lost its function or name", 0.0, format!("{label}: storeys on the keep {}", if ruin { "taken" } else { "refused" }));
            }
            if pieces::piece_removal(&p).is_ok() {
                r.add("piece edit refused", 0.0, format!("{label}: the keep was taken away"));
            }
        }
        let names: Vec<String> = l1.buildings.iter().map(|b| format!("{:?}", (b.id, &b.poly, b.func, &b.name, b.floors, b.structure))).collect();
        let edited = |l: &Layout, li: usize, when: &str, r: &mut Report| {
            let rule = if when.is_empty() { "" } else { "edits not on the same castle after an earlier site went" };
            let k = pieces::keep_index(l).map(|i| &l.buildings[i]);
            if !k.is_some_and(|b| key(b) == "library" && b.name.as_deref() == Some("The Great Library") && (b.structure == town::Structure::Ruin) == ruin) {
                r.add(if rule.is_empty() { "keep's function or name not as edited" } else { rule }, 0.0, format!("{label}{when}: keep {:?}", k.map(|b| (key(b), &b.name, b.structure))));
            }
            if ruin && k.is_some_and(|b| b.floors != l1.buildings[ki].floors) {
                r.add("ruined keep took storeys or lost its function or name", 0.0, format!("{label}{when}: keep storeys {:?}", k.map(|b| b.floors)));
            }
            let b = |y: Option<&u32>| y.and_then(|&y| l.building_index(y as usize)).map(|i| &l.buildings[i]);
            let bad_yard = (yard.first().is_some() && !b(yard.first()).is_some_and(|b| key(b) == "temple"))
                || (yard.get(1).is_some() && b(yard.get(1)).is_some())
                || (!ruin && yard.get(2).is_some() && !b(yard.get(2)).is_some_and(|b| b.floors == 3));
            if bad_yard {
                r.add(if rule.is_empty() { "yard building's edit not standing" } else { rule }, 0.0, format!("{label}{when} (L{li})"));
            }
            for (o, was) in l1.buildings.iter().zip(&names) {
                if o.id as usize == ki || yard.iter().take(3).any(|y| *y == o.id) {
                    continue;
                }
                let now = l.building_index(o.id as usize).map(|i| &l.buildings[i]);
                if now.is_none_or(|b| format!("{:?}", (b.id, &b.poly, b.func, &b.name, b.floors, b.structure)) != *was) {
                    r.add("other pieces changed by the edits", 0.0, format!("{label}{when}: b:{li}:{}", o.id));
                }
            }
            r.tally(&format!("{label}s: pieces checked"), 1);
        };
        let (w2, t2) = world_of(vec![tower.clone(), c.clone()]);
        edited(&town::layout(&w2, &t2, li), li, "", r);
        let gone = Created { removed: true, ..tower.clone() };
        let (w3, t3) = world_of(vec![gone, c.clone()]);
        edited(&town::layout(&w3, &t3, li), li, " (tower deleted)", r);
        // (The earlier site out of the list: the castle is laid out one place earlier.)
        let (w4, t4) = world_of(vec![c.clone()]);
        let l4 = town::layout(&w4, &t4, first);
        edited(&l4, first, " (tower out of the list)", r);
        let k4 = format!("b:{first}:{}", l1.buildings[ki].id);
        if !pieces::piece_of(&w4, &t4, &k4).is_some_and(|p| p.keep() && p.castle == "c:1") {
            r.add("edits not on the same castle after an earlier site went", 0.0, format!("{label}: {k4} not the keep of c:1"));
        }
    }
    // Ids told apart: the tower's, a hand castle's missing building, no such place.
    let (w, t) = world_of(vec![tower.clone(), castle.clone()]);
    for (id, want) in [(format!("b:{first}:0"), "a tower made by hand"), (format!("b:{}:999", first + 1), "has no building 999"), (format!("b:{}:0", town::layout_count(&t) + 5), "there is no place"), ("c:1".to_string(), "a site made by hand")] {
        let got = agent::building_edit(&w, &t, &id, &BuildingChange::default()).err().unwrap_or_default();
        if !got.contains(want) {
            r.add("id not told apart (wrong refusal)", 0.0, format!("{id}: {got}"));
        }
    }
    town::forget_from(first);
    // A generated castle's keep (in a town, else a village or a site) takes update_building.
    let mut tried = 0;
    let mut per: HashMap<&str, usize> = HashMap::new();
    for li in 0..first {
        let tier = if li < n { t0.settlements[li].tier.name() } else { "site" };
        let l = town::layout(base, t0, li);
        let Some(ki) = pieces::keep_index(&l) else { continue };
        *per.entry(tier).or_default() += 1;
        if per[tier] > 2 {
            continue;
        }
        let id = format!("b:{li}:{}", l.buildings[ki].id);
        tried += 1;
        let ok = agent::building_edit(base, t0, &id, &BuildingChange { func: Some("library".into()), ..Default::default() }).and_then(|e| {
            let mut ed = base.file.edits.clone();
            ed.buildings.insert(id.clone(), e.ok_or("no change")?);
            let w = World::new(WorldFile { edits: ed, ..base.file.clone() }).unwrap();
            let l = town::layout(&w, t0, li);
            let now = l.building_index(l.buildings[ki].id as usize).map(|i| key(&l.buildings[i]));
            if now == Some("library") { Ok(()) } else { Err(format!("now {now:?}")) }
        });
        if let Err(e) = ok {
            r.add("generated castle's keep refused or not changed", 0.0, format!("{id} ({}): {e}", if li < n { t0.settlements[li].tier.name() } else { "site" }));
        }
    }
    r.tally("generated castles' keeps changed", tried);
    for (tier, k) in per {
        r.tally(&format!("generated keeps: in a {tier}"), k);
    }
    town::forget_from(0);
}
