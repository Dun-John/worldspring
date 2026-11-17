//! Roads, routes and rivers between places, and battlemap summaries.

use super::*;

/// The road graph: nodes (settlement `S(i)` or a junction by rounded position) and edges
/// (the node reached, road length in ft, the stretch in `t0.roads.roads`).
fn road_graph(world: &World, t0: &T0) -> (Vec<P>, Vec<Vec<(usize, f64, usize)>>, Vec<Option<usize>>) {
    let mut nodes: Vec<P> = Vec::new();
    let mut town_of: Vec<Option<usize>> = Vec::new();
    let mut key: std::collections::BTreeMap<(i64, i64), usize> = Default::default();
    let mut node = |p: P, nodes: &mut Vec<P>, town_of: &mut Vec<Option<usize>>| -> usize {
        // A road ending near a settlement ends at that settlement.
        let near = t0
            .settlements
            .iter()
            .enumerate()
            .filter(|(_, s)| geom::dist([s.x, s.y], p) < town::reach(s) + 3_000.0)
            .min_by(|a, b| geom::dist([a.1.x, a.1.y], p).total_cmp(&geom::dist([b.1.x, b.1.y], p)))
            .map(|(i, _)| i);
        let k = match near {
            Some(i) => (i64::MIN, i as i64),
            None => ((p[0] / 400.0).round() as i64, (p[1] / 400.0).round() as i64),
        };
        *key.entry(k).or_insert_with(|| {
            nodes.push(near.map(|i| [t0.settlements[i].x, t0.settlements[i].y]).unwrap_or(p));
            town_of.push(near);
            nodes.len() - 1
        })
    };
    let mut edges: Vec<(usize, usize, f64, usize)> = Vec::new();
    for (i, r) in t0.roads.roads.iter().enumerate() {
        if r.pts.len() < 2 {
            continue;
        }
        let a = node(r.pts[0], &mut nodes, &mut town_of);
        let b = node(*r.pts.last().unwrap(), &mut nodes, &mut town_of);
        if a != b {
            edges.push((a, b, *r.s.last().unwrap_or(&0.0), i));
        }
    }
    let _ = world;
    let mut adj = vec![Vec::new(); nodes.len()];
    for (a, b, w, i) in edges {
        adj[a].push((b, w, i));
        adj[b].push((a, w, i));
    }
    (nodes, adj, town_of)
}

/// The settlements a settlement's roads lead to directly.
pub(super) fn roads_from(world: &World, t0: &T0, li: usize) -> Vec<Value> {
    let (_, adj, town_of) = road_graph(world, t0);
    let Some(n) = town_of.iter().position(|t| *t == Some(li)) else { return Vec::new() };
    let mut out = Vec::new();
    // Follow junctions on to the next settlement.
    let mut seen = vec![false; adj.len()];
    seen[n] = true;
    let mut stack: Vec<(usize, f64)> = adj[n].iter().map(|&(k, w, _)| (k, w)).collect();
    while let Some((m, d)) = stack.pop() {
        if seen[m] {
            continue;
        }
        seen[m] = true;
        if let Some(t) = town_of[m] {
            let fid = feature_of_layout(world, t0, t);
            out.push(json!({ "id": fid, "name": fid.as_deref().and_then(|f| name_of(world, t0, f)), "road_mi": mi(d) }));
        } else {
            stack.extend(adj[m].iter().map(|&(k, w, _)| (k, d + w)));
        }
    }
    out
}

/// The road network: named roads (drawn in the sketch) with the settlements along them, then
/// the roads between settlements and junctions (each with its class, length and ends: a
/// settlement's id and name, else the junction's point), filtered to those within `near` (a
/// place and a radius, ft) of the place, ending at `settlement` (a layout), of `class` (king's
/// road, road, track), or drawn only; at most `limit`, longest first.
pub fn roads(world: &World, t0: &T0, near: Option<(P, f64)>, settlement: Option<usize>, class: Option<&str>, drawn_only: bool, limit: usize) -> Value {
    use crate::roadids::{class_name, drawn_road, end_json, town_at};
    let want = class.map(|c| c.to_lowercase().replace('_', " ").replace("kings", "king's"));
    let fs = features(world, t0);
    let ids = crate::roadids::road_ids(t0);
    let rs = &t0.roads.roads;
    let keep = |i: usize| {
        let r = &rs[i];
        r.pts.len() >= 2
            && (!drawn_only || r.stroke.is_some())
            && want.as_deref().is_none_or(|w| w == class_name(r.class))
            && near.is_none_or(|(p, rad)| r.pts.iter().any(|q| geom::dist(*q, p) <= rad))
            && settlement.is_none_or(|li| [r.pts[0], *r.pts.last().unwrap()].iter().any(|&e| town_at(t0, e) == Some(li)))
    };
    // Named roads: drawn in the sketch (their pieces' ends that reach settlements, in order
    // along the line drawn), then stretches renamed alike.
    let mut named = Vec::new();
    let mut seen: Vec<u32> = Vec::new();
    for i in (0..rs.len()).filter(|&i| keep(i)) {
        let r = &rs[i];
        let Some(st) = r.stroke.filter(|s| !seen.contains(s)) else { continue };
        seen.push(st);
        let Some(f) = drawn_road(world, t0, &fs, st).map(|k| &fs[k]) else { continue };
        let line = &world.file.sketch.strokes[st as usize].pts;
        let along = |p: P| (0..line.len()).min_by(|&a, &b| geom::dist(line[a], p).total_cmp(&geom::dist(line[b], p))).unwrap_or(0);
        let pieces: Vec<usize> = (0..rs.len()).filter(|&q| rs[q].stroke == Some(st)).collect();
        let mut towns: Vec<(usize, usize)> = pieces.iter().flat_map(|&q| [rs[q].pts[0], *rs[q].pts.last().unwrap()]).filter_map(|e| town_at(t0, e).map(|t| (along(e), t))).collect();
        towns.sort();
        towns.dedup_by_key(|t| t.1);
        let via: Vec<Value> = towns.iter().filter_map(|&(_, t)| feature_of_layout(world, t0, t)).map(|id| json!({ "id": id, "name": name_of(world, t0, &id) })).collect();
        let mut by_along: Vec<(usize, usize)> = pieces.iter().map(|&q| (along(rs[q].pts[0]).min(along(*rs[q].pts.last().unwrap())), q)).collect();
        by_along.sort();
        named.push(json!({
            "id": f.id,
            "name": name_of(world, t0, &f.id).unwrap_or_else(|| f.name.clone()),
            "class": class_name(pieces.iter().map(|&q| rs[q].class).min().unwrap_or(r.class)),
            "length_mi": mi(pieces.iter().map(|&q| *rs[q].s.last().unwrap_or(&0.0)).sum()),
            "settlements": via,
            "stretches": by_along.iter().map(|&(_, q)| ids[q].clone()).collect::<Vec<_>>(),
        }));
    }
    named.extend(crate::roadids::renamed_groups(world, t0, &ids, &keep));
    let mut list: Vec<usize> = (0..rs.len()).filter(|&i| keep(i)).collect();
    list.sort_by(|&a, &b| rs[b].s.last().unwrap_or(&0.0).total_cmp(rs[a].s.last().unwrap_or(&0.0)).then_with(|| ids[a].cmp(&ids[b])));
    let total = list.len();
    let roads: Vec<Value> = list
        .iter()
        .take(limit)
        .map(|&i| {
            let r = &rs[i];
            let mut v = json!({ "id": ids[i], "class": class_name(r.class), "length_mi": mi(*r.s.last().unwrap_or(&0.0)), "from": end_json(world, t0, r.pts[0]), "to": end_json(world, t0, *r.pts.last().unwrap()) });
            if let Some(n) = world.file.edits.renames.get(&ids[i]) {
                v["name"] = json!(n);
            }
            if world.file.edits.removed_roads.contains(&ids[i]) {
                v["removed"] = json!(true);
            }
            if let Some(f) = r.stroke.and_then(|s| drawn_road(world, t0, &fs, s)).map(|k| &fs[k]) {
                let n = name_of(world, t0, &f.id).unwrap_or_else(|| f.name.clone());
                if v["name"].is_null() {
                    v["name"] = json!(n);
                }
                v["road"] = json!(n);
                v["road_id"] = json!(f.id);
            } else if r.stroke.is_some() {
                v["drawn"] = json!(true);
            }
            v
        })
        .collect();
    json!({ "named": named, "roads": roads, "count": total, "shown": roads.len() })
}

/// Shortest ways over the road graph from node `a` (stopping at `b`): distance, previous node and
/// the road reached by, per node.
fn dijkstra(adj: &[Vec<(usize, f64, usize)>], n: usize, a: usize, b: usize) -> (Vec<f64>, Vec<usize>, Vec<usize>) {
    let mut dist = vec![f64::MAX; n];
    let mut prev = vec![usize::MAX; n];
    let mut via_road = vec![usize::MAX; n];
    let mut heap = std::collections::BTreeSet::new();
    dist[a] = 0.0;
    heap.insert(((0.0f64).to_bits(), a));
    while let Some((dk, u)) = heap.pop_first() {
        let du = f64::from_bits(dk);
        if du > dist[u] {
            continue;
        }
        if u == b {
            break;
        }
        for &(v, w, ri) in &adj[u] {
            if du + w < dist[v] {
                dist[v] = du + w;
                prev[v] = u;
                via_road[v] = ri;
                heap.insert(((du + w).to_bits(), v));
            }
        }
    }
    (dist, prev, via_road)
}

/// The stretches (indices in `t0.roads.roads`, each with whether it is walked as listed) a route
/// between two points takes along roads, in order; None where roads do not join them (or
/// `route` would go overland).
pub fn route_stretches(world: &World, t0: &T0, from: P, to: P) -> Option<Vec<(usize, bool)>> {
    let (nodes, adj, _) = road_graph(world, t0);
    let nearest = |p: P| (0..nodes.len()).min_by(|&a, &b| geom::dist(nodes[a], p).total_cmp(&geom::dist(nodes[b], p)));
    let (a, b) = (nearest(from)?, nearest(to)?);
    let (dist, prev, via_road) = dijkstra(&adj, nodes.len(), a, b);
    if dist[b] == f64::MAX {
        return None;
    }
    let total = dist[b] + geom::dist(nodes[a], from) + geom::dist(nodes[b], to);
    if total >= geom::dist(from, to) * 2.5 + 2.0 * MI {
        return None;
    }
    let mut out = Vec::new();
    let mut k = b;
    while prev[k] != usize::MAX {
        let r = &t0.roads.roads[via_road[k]];
        let at = nodes[prev[k]];
        out.push((via_road[k], geom::dist(r.pts[0], at) <= geom::dist(*r.pts.last().unwrap(), at)));
        k = prev[k];
    }
    out.reverse();
    Some(out)
}

/// A route between two ids or points: by road where the road network joins them, else
/// overland; distances and 5e travel days (normal 24 mi/day, fast 30, slow 18).
pub fn route(world: &World, t0: &T0, from: P, to: P) -> Value {
    let straight = geom::dist(from, to);
    let (nodes, adj, town_of) = road_graph(world, t0);
    let nearest = |p: P| (0..nodes.len()).min_by(|&a, &b| geom::dist(nodes[a], p).total_cmp(&geom::dist(nodes[b], p)));
    let mut road = None;
    if let (Some(a), Some(b)) = (nearest(from), nearest(to)) {
        let (da, db) = (geom::dist(nodes[a], from), geom::dist(nodes[b], to));
        let (dist, prev, via_road) = dijkstra(&adj, nodes.len(), a, b);
        if dist[b] < f64::MAX {
            let mut via = Vec::new();
            let mut used = Vec::new();
            let mut k = b;
            while k != usize::MAX {
                if let Some(t) = town_of[k]
                    && let Some(fid) = feature_of_layout(world, t0, t)
                {
                    via.push(json!({ "id": fid, "name": name_of(world, t0, &fid) }));
                }
                if via_road[k] != usize::MAX {
                    used.push(via_road[k]);
                }
                k = prev[k];
            }
            via.reverse();
            used.reverse();
            // The stretches in order (id, name if any, class, length), and the named roads
            // followed (runs of stretches of one name joined).
            let ids = crate::roadids::road_ids(t0);
            let fs = features(world, t0);
            let mut stretches = Vec::new();
            let mut follows: Vec<Value> = Vec::new();
            for &ri in &used {
                let r = &t0.roads.roads[ri];
                let len = *r.s.last().unwrap_or(&0.0);
                let name = crate::roadids::stretch_name(world, t0, &fs, &ids[ri], r);
                let mut sv = json!({ "id": ids[ri], "class": crate::roadids::class_name(r.class), "length_mi": mi(len) });
                // (Roadmap 6 R5: a stretch removed by hand is still walked; traffic is unchanged.)
                if world.file.edits.removed_roads.contains(&ids[ri]) {
                    sv["removed"] = json!(true);
                }
                if let Some(n) = &name {
                    sv["name"] = json!(n);
                    match follows.last_mut().filter(|f| f["name"] == json!(n) && f["last"] == json!(stretches.len() - 1)) {
                        Some(f) => {
                            f["ft"] = json!(f["ft"].as_f64().unwrap_or(0.0) + len);
                            f["last"] = json!(stretches.len());
                        }
                        None => follows.push(json!({ "name": n, "ft": len, "last": stretches.len() })),
                    }
                }
                stretches.push(sv);
            }
            let follows: Vec<Value> = follows.into_iter().map(|f| json!({ "name": f["name"], "length_mi": mi(f["ft"].as_f64().unwrap_or(0.0)) })).collect();
            let total = dist[b] + da + db;
            // Worth it only if not far longer than going straight across.
            if total < straight * 2.5 + 2.0 * MI {
                // A long way lists its first 50 stretches (`follows` stays whole).
                let found = stretches.len();
                stretches.truncate(50);
                let mut r = json!({ "distance_mi": mi(total), "off_road_mi": mi(da + db), "via": via, "stretches": stretches, "follows": follows });
                if found > 50 {
                    r["stretches_found"] = json!(found);
                    r["truncated"] = json!(true);
                }
                if total > 1.5 * straight && straight > 0.0 {
                    r["note"] = json!(format!("the road way is {:.1} times the straight line ({} mi): a long detour", total / straight, mi(straight)));
                }
                road = Some(r);
            }
        }
    }
    let travel = |ft: f64| json!({ "normal_days": ((ft / MI / 24.0) * 10.0).ceil() / 10.0, "fast_days": ((ft / MI / 30.0) * 10.0).ceil() / 10.0, "slow_days": ((ft / MI / 18.0) * 10.0).ceil() / 10.0 });
    let by = road.as_ref().and_then(|r| r["distance_mi"].as_f64()).map(|m| m * MI).unwrap_or(straight);
    json!({
        "straight_mi": mi(straight),
        "by_road": road,
        "travel": travel(by),
        "direction": compass(from, to),
    })
}

// ---------------------------------------------------------------------------------------
// Rivers' banks.

/// A river near a point: the nearest point of its finest curve, its width and banks there,
/// and its feature (a river big enough to be named).
pub struct RiverAt {
    pub id: Option<String>,
    pub name: Option<String>,
    pub width_ft: f64,
    /// The middle of the channel nearest the point, and the unit normal across it there.
    pub center: P,
    pub across: P,
    /// From the point to the middle of the channel (ft).
    pub dist_ft: f64,
}

impl RiverAt {
    /// Whether the point asked about is in the water.
    pub fn in_water(&self) -> bool {
        self.dist_ft < 0.5 * self.width_ft
    }

    /// The two banks across the channel (ft): on the `across` side, then the other.
    pub fn banks(&self) -> [P; 2] {
        let h = 0.5 * self.width_ft;
        [[self.center[0] + self.across[0] * h, self.center[1] + self.across[1] * h], [self.center[0] - self.across[0] * h, self.center[1] - self.across[1] * h]]
    }

    /// The bank on `p`'s side, `beyond` ft out of the water, and how far `p` is from the bank
    /// (ft; negative: in the water).
    pub fn bank_of(&self, p: P, beyond: f64) -> (P, f64) {
        let side = if (p[0] - self.center[0]) * self.across[0] + (p[1] - self.center[1]) * self.across[1] >= 0.0 { 1.0 } else { -1.0 };
        let h = 0.5 * self.width_ft + beyond;
        let b = [self.center[0] + side * self.across[0] * h, self.center[1] + side * self.across[1] * h];
        let along = (p[0] - self.center[0]) * self.across[0] * side + (p[1] - self.center[1]) * self.across[1] * side;
        (b, along - 0.5 * self.width_ft)
    }

    /// {id, name, width_ft, banks}: with `from`, the bank on its side first.
    pub fn json(&self, from: Option<P>) -> Value {
        let mut b = self.banks();
        if let Some(p) = from
            && (p[0] - self.center[0]) * self.across[0] + (p[1] - self.center[1]) * self.across[1] < 0.0
        {
            b.swap(0, 1);
        }
        let r = |p: P| [p[0].round(), p[1].round()];
        json!({ "id": self.id, "name": self.name, "width_ft": self.width_ft.round(), "banks": [r(b[0]), r(b[1])] })
    }
}

/// The river nearest `p` whose water comes within `range_ft` of it (by its finest curve, as
/// battlemaps draw it), else None.
pub fn river_at(world: &World, t0: &T0, p: P, range_ft: f64) -> Option<RiverAt> {
    let net = &t0.rivers;
    let cell = t0.cell_ft;
    // (Distance to the water's edge, river, segment, t, steps.)
    let mut best: Option<(f64, usize, usize, f64, usize)> = None;
    for (ri, k) in net.segments_near(p[0] - range_ft, p[1] - range_ft, p[0] + range_ft, p[1] + range_ft, 0.0) {
        let (r, k) = (&net.rivers[ri as usize], k as usize);
        let n = ((r.s[k + 1] - r.s[k]) / 20.0).ceil().clamp(8.0, 2000.0) as usize;
        for j in 0..=n {
            let t = j as f64 / n as f64;
            let c = r.eval(k, t, 2.5, cell);
            let d = geom::dist(c.p, p) - 0.5 * c.w;
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, ri as usize, k, t, n));
            }
        }
    }
    let (_, ri, k, t0_, n) = best?;
    let r = &net.rivers[ri];
    // Finer about the best sample.
    let (mut bt, mut bd) = (t0_, f64::MAX);
    for j in -10..=10 {
        let t = (t0_ + j as f64 / (10.0 * n as f64)).clamp(0.0, 1.0);
        let d = geom::dist(r.eval(k, t, 2.5, cell).p, p);
        if d < bd {
            (bt, bd) = (t, d);
        }
    }
    let c = r.eval(k, bt, 2.5, cell);
    if bd - 0.5 * c.w > range_ft {
        return None;
    }
    let eps = (2.0 / (r.s[k + 1] - r.s[k]).max(1.0)).min(0.05);
    let (a, b) = (r.eval(k, (bt - eps).max(0.0), 2.5, cell).p, r.eval(k, (bt + eps).min(1.0), 2.5, cell).p);
    let tl = geom::dist(a, b).max(1e-9);
    let across = [-(b[1] - a[1]) / tl, (b[0] - a[0]) / tl];
    // Its feature: the named river whose course passes nearest.
    let feature = t0.extra.as_ref().and_then(|e| {
        let o = &e.overlay;
        (0..o.features.len().min(o.shapes.len())).filter(|&i| o.features[i].kind == "river").map(|i| (o.shapes[i].distance(c.p).0, i)).filter(|&(d, _)| d <= (2.0 * cell).max(c.w)).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, i)| &o.features[i])
    });
    Some(RiverAt {
        id: feature.map(|f| f.id.clone()),
        name: feature.map(|f| world.file.edits.renames.get(&f.id).cloned().unwrap_or_else(|| f.name.clone())),
        width_ft: c.w,
        center: c.p,
        across,
        dist_ft: bd,
    })
}

// ---------------------------------------------------------------------------------------
// Battlemaps.

/// A battlemap chunk (one 640-ft square, 128 × 128 five-foot squares) summarised for an
/// agent: its origin, surfaces, elevation, buildings, and the objects with their tactical
/// rules, those within `radius_sq` squares of the focus listed one by one.
pub fn battlemap_summary(world: &World, chunk: &crate::battlemap::Chunk, origin: P, focus: P, radius_sq: f64) -> Value {
    use crate::battlemap::SQ;
    let sea = world.params().sea_level_ft as f32;
    let mut surfaces = std::collections::BTreeMap::<String, usize>::new();
    for s in &chunk.surface {
        *surfaces.entry(format!("{s:?}").to_lowercase()).or_default() += 1;
    }
    let (lo, hi) = chunk.height.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
    let mut kinds = std::collections::BTreeMap::<&str, (usize, Value)>::new();
    let mut near = Vec::new();
    let (fx, fy) = ((focus[0] - origin[0]) / 5.0, (focus[1] - origin[1]) / 5.0);
    for o in &chunk.objects {
        let (k, name) = (chunk.info(o), chunk.name(o));
        let e = kinds.entry(name).or_insert_with(|| {
            let cover = ["none", "half", "three-quarters", "full"][k.cover.min(3) as usize];
            (0, json!({ "cover": cover, "blocks_movement": k.blocks_move, "blocks_sight": k.blocks_sight, "difficult": k.difficult, "height_ft": k.height_ft, "hazard": k.hazard.map(|h| format!("{}: {}", h.name, h.effect)) }))
        });
        e.0 += 1;
        if (o.x as f64 - fx).hypot(o.y as f64 - fy) <= radius_sq && near.len() < 200 {
            let (x, y) = (origin[0] + o.x as f64 * 5.0, origin[1] + o.y as f64 * 5.0);
            let mut v = json!({ "object": name, "square": [o.x as i32, o.y as i32], "x_ft": (x * 100.0).round() / 100.0, "y_ft": (y * 100.0).round() / 100.0 });
            // Put down by hand: its id (to move or remove it) and its turn if drawn turned;
            // generated: its kind (to clear it).
            match world.file.edits.objects.iter().find(|(_, p)| (p.x - x).abs() < 0.01 && (p.y - y).abs() < 0.01) {
                Some((id, _)) => {
                    v["id"] = json!(id);
                    if o.turned {
                        v["rot_deg"] = json!((o.rot as f64 * 180.0 / std::f64::consts::PI * 10.0).round() / 10.0);
                    }
                }
                None => v["kind"] = json!(o.kind as u16),
            }
            near.push(v);
        }
    }
    // Generated objects taken away by hand here (restore_objects brings them back).
    let size = SQ as f64 * 5.0;
    let cleared: Vec<Value> = world
        .file
        .edits
        .cleared
        .iter()
        .filter(|(_, c)| c.x >= origin[0] && c.y >= origin[1] && c.x < origin[0] + size && c.y < origin[1] + size)
        .map(|(id, c)| json!({ "id": id, "x_ft": c.x, "y_ft": c.y, "kind": c.kind, "radius_ft": c.r }))
        .collect();
    json!({
        "origin_ft": [origin[0], origin[1]],
        "squares": [SQ, SQ],
        "square_ft": 5,
        "elevation_ft": [(lo - sea).round(), (hi - sea).round()],
        "surfaces": surfaces,
        "buildings": chunk.buildings.len(),
        "objects": kinds.into_iter().map(|(name, (count, rules))| json!({ "object": name, "count": count, "rules": rules })).collect::<Vec<_>>(),
        "near_focus": near,
        "cleared": cleared,
    })
}
