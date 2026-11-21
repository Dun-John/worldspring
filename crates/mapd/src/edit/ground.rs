//! MCP tools for ground shaped by hand at battlemap scale (`worldgen::ground`): brushed blocks
//! (`Edits::ground`: raise, lower, smooth, put back) and plateaus or pits (`Edits::plateaus`),
//! and the ground's heights as generated and as shaped.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use worldgen::battlemap::{SQ, SQUARE_FT};
use worldgen::core::tile::TileKey;
use worldgen::geoquery::Squares;
use worldgen::ground::{self, Brush, Plateau, Stroke};
use worldgen::pipeline::Executor;
use worldgen::town::geom;
use worldgen::world::Edits;
use worldgen::{agent, town};

use crate::Shared;
use crate::edit::notebook::new_id;
use crate::tools::{schema, text};

/// How far `ground_heights` reads each way from its place (ft): half its box's side.
const MAX_READ_FT: f64 = 200.0;

pub fn list() -> Vec<Value> {
    let pt = crate::tools::args::point_schema("{x_ft, y_ft} or [x_ft, y_ft]");
    let at = crate::tools::args::point_schema("A place (id, point or [x, y] in ft)");
    vec![
        json!({ "name": "shape_ground", "title": "Shape the ground", "description": "Shape the battlemap's ground, a round brush: raise, lower, smooth, or restore toward the generated ground (plateaus stay). A dab (at, radius_ft) or a stroke (path, width_ft); buildings, streets and walls follow. At most 48 blocks of 80 ft a stroke, 1500 a world. Returns the blocks changed and the ground's range (ft).", "inputSchema": schema(json!({
            "mode": { "type": "string", "enum": ["raise", "lower", "smooth", "restore"] },
            "at": at,
            "radius_ft": { "type": "number", "minimum": 2.5, "maximum": 200, "default": 15, "description": "A dab's radius" },
            "path": { "type": "array", "minItems": 1, "items": pt, "description": "A stroke's points (world ft)" },
            "width_ft": { "type": "number", "minimum": 5, "maximum": 400, "default": 30, "description": "A stroke's width" },
            "height_ft": { "type": "number", "minimum": 0, "maximum": 50, "default": 5, "description": "Raise and lower by up to this" },
            "hardness": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5, "description": "0 soft to 1 hard" },
            "strength": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5 },
        }), &["mode"]) }),
        json!({ "name": "set_plateau", "title": "Make or change a plateau", "description": "A plateau or pit: ground inside an outline levelled at one height, easing back over edge_ft (0: a cliff). Overlaps stack: the highest holds, a pit digs through (the deepest holds), same levels join. floor: rock draws bare rock at every zoom (auto, default: a pit's floor 15+ ft down). Give poly (at most 48 points, 1280 ft across) or around (a building, tower or site, grown by margin_ft). Height: level_ft above the sea or rise_ft above the ground (default 10; negative digs). With id, changes it. Returns its id (h:...).", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "A plateau to change (h:...)" },
            "poly": { "type": "array", "minItems": 3, "items": pt },
            "around": { "type": "string", "description": "A building, tower or site (its footprint)" },
            "margin_ft": { "type": "number", "minimum": 0, "maximum": 200, "default": 10, "description": "How far round the footprint" },
            "level_ft": { "type": "number", "minimum": -20000, "maximum": 20000, "description": "Its level, ft above the sea" },
            "rise_ft": { "type": "number", "minimum": -ground::MAX_SHIFT_FT, "maximum": ground::MAX_SHIFT_FT, "default": 10 },
            "edge_ft": { "type": "number", "minimum": 0, "maximum": ground::MAX_EDGE_FT, "default": 0 },
            "floor": { "type": "string", "enum": ["auto", "rock", "ground"], "description": "What its top or floor draws as (kept when changing one; default auto)" },
        }), &[]) }),
        json!({ "name": "set_hill", "title": "Raise or change a hill", "description": "A hill on the battlemap's ground: it rises from the ground at its footprint's edge to peak_ft over the ground, with natural slopes and a little shape noise of its own. profile: round (smooth slopes), ridge (a crest along a line: from and to, half-width width_ft), knoll (steeper, with rocky outcrops) or mountain (concave slopes and bands of crag; the outcrops draw as cliff faces with scree). Footprint: at with radius_ft (or rx_ft along angle_deg and ry_ft across), or around (a building, tower or site: centred on it, the crown wider than it). crown_ft flattens its top (a castle's yard). Sizes: round up to 7,920 ft from its middle, knoll 5,280, mountain 13,200 (5 mi across), ridge 26,400 along (10 mi long) and 5,280 across. Peak: up to 300 ft on any hill, a big one up to 0.45 of its narrower half-axis (round and knoll 1,000 ft, ridge 1,500, mountain 2,000). Hills stand on top of the brushed ground (the brush alone moves it at most 300 ft). A hill whose narrower half-axis spans 8 samples of a map level shows there too. With id, changes it (fields left out stay). Returns its id (h:...); remove takes it away.", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "A hill to change (h:...)" },
            "profile": { "type": "string", "enum": ["round", "ridge", "knoll", "mountain"], "default": "round" },
            "at": at,
            "radius_ft": { "type": "number", "minimum": ground::HILL_RADIUS_FT.start(), "maximum": ground::HILL_RADIUS_FT.end(), "description": "The footprint's radius (round)" },
            "rx_ft": { "type": "number", "minimum": ground::HILL_RADIUS_FT.start(), "maximum": ground::HILL_RADIUS_FT.end(), "description": "Half-axis along angle_deg" },
            "ry_ft": { "type": "number", "minimum": ground::HILL_RADIUS_FT.start(), "maximum": ground::HILL_RADIUS_FT.end(), "description": "Half-axis across (default rx_ft)" },
            "angle_deg": { "type": "number", "default": 0, "description": "rx_ft's direction, clockwise from east" },
            "from": at, "to": at,
            "width_ft": { "type": "number", "minimum": ground::HILL_RADIUS_FT.start(), "maximum": 5280, "default": 80, "description": "A ridge's half-width (with from and to)" },
            "around": { "type": "string", "description": "A building, tower or site to raise a round hill under" },
            "peak_ft": { "type": "number", "minimum": 1, "maximum": ground::MAX_PEAK_FT, "default": 60, "description": "Its peak over the ground (over 300 ft only on a big hill)" },
            "crown_ft": { "type": "number", "minimum": 0, "maximum": 512, "default": 0, "description": "A flat crown's radius (at most 0.8 of the narrower half-axis)" },
        }), &[]) }),
        json!({ "name": "delete_plateau", "title": "Take plateaus away", "description": "Take plateaus, pits or the world\'s own generated islets (i:..., see list (what ground); its watchtower goes too: removed_towers) away (the ground goes back as it was), by id (h:...), with their names, notes and hidden marks (no restore; the user's Undo in the app brings them back).", "inputSchema": schema(crate::tools::args::ids_props("plateau id (h:...)"), &[]) }),
        json!({ "name": "restore_islets", "title": "Bring generated islets back", "description": "Bring the world's own islets (i:...) back after delete_plateau took them away. A watchtower removed with one stays removed (restore_building).", "inputSchema": schema(crate::tools::args::ids_props("islet id (i:...)"), &[]) }),
        json!({ "name": "ground_heights", "title": "Ground heights", "description": "The ground's height (ft above the sea) on each 5-ft square within radius_ft (at most 200) of a place, as generated and as shaped by hand (rows north to south, columns west to east from origin), with water squares, plateaus and brushed blocks.", "inputSchema": schema(json!({ "at": at, "radius_ft": { "type": "number", "minimum": 0, "maximum": MAX_READ_FT, "default": 25 } }), &[]) }),
    ]
}

/// Run a ground tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "shape_ground" => shape(app, a).await.map(text),
        "set_plateau" => set_plateau(app, a).await.map(text),
        "set_hill" => set_hill(app, a).await.map(text),
        "delete_plateau" => delete(app, a).await.map(text),
        "restore_islets" => restore_islets(app, a).await.map(text),
        "ground_heights" => heights(app, a).await.map(text),
        _ => return None,
    })
}

/// An optional number: absent (or null) is `None`, anything else must be a finite number.
fn opt_num(a: &Value, k: &str) -> Result<Option<f64>, String> {
    match &a[k] {
        Value::Null => Ok(None),
        v => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| format!("{k}: a number")),
    }
}

fn num(a: &Value, k: &str, default: f64) -> Result<f64, String> {
    match &a[k] {
        Value::Null => Ok(default),
        v => v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| format!("{k}: a number")),
    }
}

fn extent(v: &[f32], sea: f32) -> Value {
    let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
    if v.is_empty() { Value::Null } else { json!({ "min_ft": round1(lo - sea), "max_ft": round1(hi - sea) }) }
}

fn round1(v: f32) -> f64 {
    (v as f64 * 10.0).round() / 10.0
}

async fn shape(app: &Shared, a: &Value) -> Result<Value, String> {
    let mode = match a["mode"].as_str() {
        Some("raise") => Brush::Raise,
        Some("lower") => Brush::Lower,
        Some("smooth") => Brush::Smooth,
        Some("restore") => Brush::Restore,
        m => return Err(format!("mode: raise, lower, smooth or restore, not {}", m.unwrap_or("nothing"))),
    };
    if !a["path"].is_null() && !a["at"].is_null() {
        return Err("at and path: give one".into());
    }
    let (path, radius) = if a["path"].is_null() {
        (vec![crate::tools::args::place(app, a).await?], num(a, "radius_ft", 15.0)?)
    } else {
        let mut path = Vec::new();
        for (i, p) in a["path"].as_array().ok_or("path: a list of [x_ft, y_ft]")?.iter().enumerate() {
            path.push(crate::tools::args::point(app, &format!("path[{i}]"), p).await?);
        }
        if path.is_empty() {
            return Err("path: at least one point".into());
        }
        (path, num(a, "width_ft", 30.0)? * 0.5)
    };
    let (height, hardness, strength) = (num(a, "height_ft", 5.0)?, num(a, "hardness", 0.5)?, num(a, "strength", 0.5)?);
    let b = path.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - radius), b[1].min(p[1] - radius), b[2].max(p[0] + radius), b[3].max(p[1] + radius)]);
    // The squares the brush reaches (their middles within it), as `stamp_brush` finds them: too
    // many blocks are refused before any ground is read.
    let span = ((b[2] - b[0]) / SQUARE_FT + 1.0) * ((b[3] - b[1]) / SQUARE_FT + 1.0);
    let too_many = || format!("a stroke reaches at most {} blocks of 80 ft", ground::MAX_STROKE_BLOCKS);
    if span > 4.0e6 {
        return Err(too_many());
    }
    let sq = |v: f64| (v / SQUARE_FT).floor() as i64;
    let mut under: Vec<(i64, i64)> = Vec::new();
    for gy in sq(b[1]).max(0)..=sq(b[3]) {
        for gx in sq(b[0]).max(0)..=sq(b[2]) {
            let c = [(gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT];
            let d = if path.len() == 1 { geom::dist(c, path[0]) } else { path.windows(2).map(|w| geom::seg_dist(c, w[0], w[1])).fold(f64::MAX, f64::min) };
            if d < radius {
                under.push((gx, gy));
            }
        }
    }
    let blocks: std::collections::BTreeSet<(i64, i64)> = under.iter().map(|&(x, y)| (x / ground::BLOCK, y / ground::BLOCK)).collect();
    if blocks.len() > ground::MAX_STROKE_BLOCKS {
        return Err(too_many());
    }
    // The ground under the brush as generated; smoothing eases toward the ground as drawn.
    let smooth = mode == Brush::Smooth;
    let (under2, path2) = (under.clone(), path.clone());
    let (squares, drawn, sea) = app
        .worker
        .with_mut(move |ex| {
            let (mw, mh) = (ex.world.geom.map_w_ft, ex.world.geom.map_h_ft);
            if path.iter().any(|p| !(0.0..mw).contains(&p[0]) || !(0.0..mh).contains(&p[1])) {
                return Err("the brush is off the map".to_string());
            }
            let squares = Squares::round(ex, b, 1);
            let drawn = if smooth { drawn_ground(ex, &under2) } else { BTreeMap::new() };
            Ok::<_, String>((squares, drawn, ex.world.params().sea_level_ft as f32))
        })
        .await?;
    let reply = app
        .edit("agent", 0, move |e| {
            let under: Vec<usize> = under.iter().filter_map(|&(gx, gy)| squares.at(gx, gy)).collect();
            let pick = |v: Vec<f32>| under.iter().map(|&k| v[k]).collect::<Vec<f32>>();
            let before = pick(squares.shaped(e, None));
            // (A neighbour off the map reads the nearest square on it: `squares` holds the
            // brush's box and a square round it, clamped to the map.)
            let height_of = |gx: i64, gy: i64| -> f64 {
                let (gx, gy) = (gx.clamp(squares.gx0, squares.gx0 + squares.w - 1), gy.clamp(squares.gy0, squares.gy0 + squares.h - 1));
                drawn.get(&(gx, gy)).map(|&h| h as f64).or_else(|| squares.at(gx, gy).map(|k| squares.g[k] as f64)).unwrap_or(0.0)
            };
            let stroke = Stroke { mode, path: &path2, radius_ft: radius, height_ft: height, hardness, strength };
            let changed = ground::stamp_brush(&mut e.ground, &stroke, &height_of)?;
            let after = pick(squares.shaped(e, None));
            let name = match mode {
                Brush::Raise => "raise",
                Brush::Lower => "lower",
                Brush::Smooth => "smooth",
                Brush::Restore => "restore",
            };
            Ok(json!({ "tool": "shape_ground", "mode": name, "count": changed.len(), "blocks": changed, "squares": under.len(), "before": extent(&before, sea), "after": extent(&after, sea) }))
        })
        .await?;
    Ok(reply)
}

/// The ground the battlemaps draw (roofs and decks left out) on the squares given and round
/// them (the chunks holding them and their neighbours).
fn drawn_ground(ex: &mut Executor, under: &[(i64, i64)]) -> BTreeMap<(i64, i64), f32> {
    use worldgen::battlemap::HS;
    let lvl = ex.world.geom.max_level;
    let (sq, hs) = (SQ as i64, HS);
    let size = ex.world.geom.tile_size_ft(lvl);
    let (nx, ny) = ((ex.world.geom.map_w_ft / size).ceil() as i64, (ex.world.geom.map_h_ft / size).ceil() as i64);
    let chunks: std::collections::BTreeSet<(i64, i64)> =
        under.iter().flat_map(|&(x, y)| [(-1, -1), (1, -1), (-1, 1), (1, 1)].map(|(dx, dy)| ((x + dx).max(0).div_euclid(sq), (y + dy).max(0).div_euclid(sq)))).collect();
    let mut out = BTreeMap::new();
    for (tx, ty) in chunks.into_iter().filter(|&(x, y)| x < nx && y < ny) {
        let c = ex.battlemap(TileKey::surface(lvl, tx as u32, ty as u32));
        for j in 0..SQ {
            for i in 0..SQ {
                let h = (j + 1) * hs + i + 1;
                out.insert((tx * sq + i as i64, ty * sq + j as i64), c.halo_height[h] - c.roof_lift[h]);
            }
        }
    }
    out
}

/// A footprint to raise a plateau round: a building's or keep's outline, a tower's circle, or a
/// site's buildings, towers and walls.
fn footprint(ex: &Executor, id: &str) -> Result<Vec<[f64; 2]>, String> {
    let (world, t0) = (&ex.world, &ex.t0);
    let mut parts = id.split(':');
    let head = parts.next().unwrap_or("");
    let nums: Vec<usize> = parts.filter_map(|p| p.parse().ok()).collect();
    let n = town::layout_count(t0);
    match (head, nums.as_slice()) {
        ("b" | "k", [li, bi]) if *li < n => town::layout(world, t0, *li).building(*bi).map(|b| b.poly.clone()).ok_or_else(|| format!("no building {id}")),
        ("t", [li, k]) if *li < n => {
            let l = town::layout(world, t0, *li);
            let (c, r, _) = l.tower_index(*k).and_then(|i| worldgen::interior::towers(&l).get(i).copied()).ok_or_else(|| format!("no tower {id}"))?;
            Ok(geom::circle(c, r, 16))
        }
        _ => {
            let li = agent::layout_of(world, t0, id).ok_or_else(|| format!("{id}: give a building (b:...), a tower (t:...) or a site, or an outline"))?;
            if town::is_settlement(t0, li) {
                return Err(format!("{id} is a settlement: a plateau spans at most {} ft (give an outline, or a building in it)", ground::MAX_PLATEAU_SPAN_FT));
            }
            let l = town::layout(world, t0, li);
            let mut pts: Vec<[f64; 2]> = l.buildings.iter().flat_map(|b| b.poly.iter().copied()).chain(l.walls.iter().flatten().copied()).collect();
            for (c, r, _) in worldgen::interior::towers(&l) {
                pts.extend(geom::circle(c, r, 16));
            }
            if pts.len() < 3 {
                return Err(format!("{id} has nothing standing to go round: give an outline"));
            }
            Ok(pts)
        }
    }
}

/// The convex outline round points, grown by `margin` ft.
fn hull_round(pts: &[[f64; 2]], margin: f64) -> Vec<[f64; 2]> {
    let mut all: Vec<[f64; 2]> = if margin > 0.0 {
        pts.iter().flat_map(|&p| (0..16).map(move |k| {
            let a = std::f64::consts::TAU * k as f64 / 16.0;
            // (The circle's points stand outside it, so the hull clears the margin everywhere.)
            let r = margin / (std::f64::consts::PI / 16.0).cos();
            [p[0] + r * a.cos(), p[1] + r * a.sin()]
        })).collect()
    } else {
        pts.to_vec()
    };
    all.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    all.dedup();
    if all.len() < 3 {
        return all;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut hull: Vec<[f64; 2]> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let seq: Box<dyn Iterator<Item = &[f64; 2]>> = if pass == 0 { Box::new(all.iter()) } else { Box::new(all.iter().rev()) };
        for &p in seq {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

async fn set_plateau(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().map(str::to_string);
    let old = match &id {
        Some(id) => Some(app.with_edits(|e| e.plateaus.get(id).cloned()).flatten().ok_or_else(|| format!("no plateau {id}"))?),
        None => None,
    };
    if old.as_ref().is_some_and(|p| p.hill.is_some()) {
        return Err("that is a hill: set_ground changes it (kind hill)".into());
    }
    let edge = match (&a["edge_ft"], &old) {
        (Value::Null, Some(p)) => p.edge as f64,
        (Value::Null, None) => 0.0,
        (v, _) => v.as_f64().filter(|e| (0.0..=ground::MAX_EDGE_FT as f64).contains(e)).ok_or("edge_ft: 0 to 60")?,
    };
    // The outline: given, round a footprint, or the one it has.
    let outline: Option<Vec<[f64; 2]>> = if let Some(o) = a["poly"].as_array() {
        let mut pts = Vec::new();
        for (i, p) in o.iter().enumerate() {
            pts.push(crate::tools::args::point(app, &format!("poly[{i}]"), p).await?);
        }
        Some(pts)
    } else if let Some(around) = a["around"].as_str() {
        let (around, margin) = (around.to_string(), num(a, "margin_ft", 10.0)?);
        if !(0.0..=200.0).contains(&margin) {
            return Err("margin_ft: 0 to 200".into());
        }
        Some(app.worker.with(move |ex| footprint(ex, &around).map(|f| hull_round(&f, margin))).await?)
    } else {
        None
    };
    if outline.is_none() && old.is_none() {
        return Err("give an outline, or around (a building, tower or site)".into());
    }
    let pts = outline.clone().unwrap_or_else(|| old.as_ref().map(|p| p.pts.clone()).unwrap_or_default());
    let mid = geom::centroid(&pts);
    // The ground at the middle as generated (shaped below, without this plateau).
    let (squares, sea) = app
        .worker
        .with_mut(move |ex| {
            let (mw, mh) = (ex.world.geom.map_w_ft, ex.world.geom.map_h_ft);
            if pts.iter().any(|p| !(0.0..mw).contains(&p[0]) || !(0.0..mh).contains(&p[1])) {
                return Err("the outline is off the map".to_string());
            }
            Ok::<_, String>((Squares::round(ex, [mid[0], mid[1], mid[0], mid[1]], 0), ex.world.params().sea_level_ft))
        })
        .await?;
    let (level_ft, rise) = (opt_num(a, "level_ft")?, opt_num(a, "rise_ft")?);
    if level_ft.is_some_and(|l| !l.is_finite() || l.abs() > 20_000.0) || rise.is_some_and(|r| !r.is_finite() || r.abs() > ground::MAX_SHIFT_FT) {
        return Err(format!("level_ft: within 20,000 ft of the sea; rise_ft: within {} ft", ground::MAX_SHIFT_FT));
    }
    let edge_arg = opt_num(a, "edge_ft")?;
    // Some(None): auto; None: as it was.
    let floor_arg: Option<Option<ground::Floor>> = match &a["floor"] {
        Value::Null => None,
        Value::String(f) if f == "auto" => Some(None),
        Value::String(f) => Some(Some(ground::Floor::parse(f).ok_or("floor: auto, rock or ground")?)),
        _ => return Err("floor: auto, rock or ground".into()),
    };
    app.edit("agent", 0, move |e| {
        let changed = id.is_some();
        // (As it is now: it may have changed, or gone, since it was read above.)
        let old = match &id {
            Some(id) => Some(e.plateaus.get(id).cloned().ok_or_else(|| format!("no plateau {id}"))?),
            None => None,
        };
        let edge = match (edge_arg, &old) {
            (Some(v), _) => v,
            (None, Some(p)) => p.edge as f64,
            (None, None) => edge,
        };
        let id = id.unwrap_or_else(|| new_id("h"));
        if !changed && e.plateaus.len() >= ground::MAX_PLATEAUS {
            return Err(format!("a world holds at most {} plateaus", ground::MAX_PLATEAUS));
        }
        let mut others = Edits { ground: e.ground.clone(), plateaus: e.plateaus.clone(), ..Default::default() };
        others.plateaus.remove(&id);
        let here = squares.shaped(&others, None)[0] as f64;
        let level = match (level_ft, rise, &old) {
            (Some(l), _, _) => l + sea,
            (None, Some(r), _) => here + r,
            (None, None, Some(p)) => p.level as f64,
            (None, None, None) => here + 10.0,
        };
        let p = match (&outline, &old) {
            (Some(o), _) => ground::plateau_for(o, level, edge)?,
            (None, Some(p)) => {
                let p = Plateau { pts: p.pts.clone(), level: ((level * 10.0).round() * 0.1) as f32, edge: edge as f32, kind: p.kind, floor: p.floor, hill: p.hill.clone() };
                if let Some(err) = p.problem() {
                    return Err(err);
                }
                p
            }
            (None, None) => unreachable!("an outline or a plateau"),
        };
        let p = Plateau { floor: floor_arg.unwrap_or(old.as_ref().and_then(|o| o.floor)), ..p };
        // The same outline again (a script run twice): that plateau, nothing added.
        if !changed
            && let Some((eid, ep)) = e.plateaus.iter().find(|(_, q)| q.pts.len() == p.pts.len() && q.pts.iter().zip(&p.pts).all(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= 1.0))
        {
            // A rise is from the ground without that plateau (the new level above took the matched one into account).
            let level = match (level_ft, rise) {
                (Some(_), _) | (None, None) => p.level as f64,
                (None, Some(r)) => {
                    let mut o2 = Edits { ground: e.ground.clone(), plateaus: e.plateaus.clone(), ..Default::default() };
                    o2.plateaus.remove(eid);
                    squares.shaped(&o2, None)[0] as f64 + r
                }
            };
            let tol = if level_ft.is_some() { 0.15 } else { 1.0 };
            if (level_ft.is_some() || rise.is_some()) && (ep.level as f64 - level).abs() > tol || edge_arg.is_some() && (ep.edge - p.edge).abs() > 1e-4 || floor_arg.is_some_and(|f| f != ep.floor) {
                return Err(format!("{eid} has this outline: give its id to change it"));
            }
            return Ok(json!({
                "tool": "set_plateau", "id": eid, "changed": false, "existing": true,
                "note": "a plateau with this outline was already there: nothing was added (give its id to change it)",
                "level_ft": round1(ep.level - sea as f32), "edge_ft": ep.edge, "points": ep.pts.len(),
            }));
        }
        let b = p.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
        let reply = json!({
            "tool": "set_plateau", "id": id, "changed": changed,
            "level_ft": round1(p.level - sea as f32), "ground_ft": round1(here as f32 - sea as f32), "edge_ft": p.edge,
            "points": p.pts.len(), "area_sq_ft": geom::area(&p.pts).abs().round(), "bbox": [b[0], b[1], b[2], b[3]],
            "floor": p.floor.map_or("auto", ground::Floor::name),
        });
        e.plateaus.insert(id, p);
        Ok(reply)
    })
    .await
}

/// `set_hill`: a hill (`ground::hill_for`) made or changed.
async fn set_hill(app: &Shared, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().map(str::to_string);
    let old = match &id {
        Some(id) => {
            let p = app.with_edits(|e| e.plateaus.get(id).cloned()).flatten().ok_or_else(|| format!("no hill {id}"))?;
            Some(p.hill.clone().map(|h| (h, p.level as f64)).ok_or_else(|| format!("{id} is a plateau, not a hill: set_ground changes it (kind plateau)"))?)
        }
        None => None,
    };
    let profile = match &a["profile"] {
        Value::Null => old.as_ref().map_or(ground::HillProfile::Round, |o| o.0.profile),
        Value::String(s) => ground::HillProfile::parse(s).ok_or("profile: round, ridge, knoll or mountain")?,
        _ => return Err("profile: round, ridge, knoll or mountain".into()),
    };
    let mut peak = opt_num(a, "peak_ft")?;
    let mut crown = opt_num(a, "crown_ft")?;
    let (mut at, mut rx, mut ry, mut angle) = (None, opt_num(a, "rx_ft")?.or(opt_num(a, "radius_ft")?), opt_num(a, "ry_ft")?.or(opt_num(a, "radius_ft")?), opt_num(a, "angle_deg")?);
    if !a["from"].is_null() || !a["to"].is_null() {
        let (f, t) = (crate::tools::args::point(app, "from", &a["from"]).await?, crate::tools::args::point(app, "to", &a["to"]).await?);
        let w = num(a, "width_ft", 80.0)?;
        at = Some([(f[0] + t[0]) * 0.5, (f[1] + t[1]) * 0.5]);
        (rx, ry) = (Some((t[0] - f[0]).hypot(t[1] - f[1]) * 0.5 + w), Some(w));
        angle = Some((t[1] - f[1]).atan2(t[0] - f[0]).to_degrees());
    } else if !a["at"].is_null() {
        at = Some(crate::tools::args::point(app, "at", &a["at"]).await?);
    } else if let Some(around) = a["around"].as_str() {
        let around = around.to_string();
        let key = id.clone().unwrap_or_else(|| "h:around".into());
        let p = app.worker.with(move |ex| footprint(ex, &around).and_then(|f| ground::castle_hill(&key, &f))).await?;
        let h = p.hill.as_ref().expect("a hill");
        at = Some(h.at);
        (rx, ry) = (rx.or(Some(h.rx)), ry.or(Some(h.ry)));
        peak = peak.or(Some(p.level as f64));
        crown = crown.or(Some(h.crown as f64));
    }
    let at = match (at, &old) {
        (Some(p), _) => p,
        (None, Some(o)) => o.0.at,
        (None, None) => return Err("give at (with radius_ft, or rx_ft and ry_ft), from and to (a ridge), or around".into()),
    };
    let rx = rx.or(old.as_ref().map(|o| o.0.rx)).ok_or("radius_ft (or rx_ft): how far the hill reaches")?;
    let ry = ry.or(old.as_ref().map(|o| if a["rx_ft"].is_null() && a["radius_ft"].is_null() { o.0.ry } else { rx })).unwrap_or(rx);
    let angle = angle.or(old.as_ref().map(|o| o.0.angle as f64)).unwrap_or(0.0);
    let peak = peak.or(old.as_ref().map(|o| o.1)).unwrap_or(60.0);
    let crown = crown.or(old.as_ref().map(|o| o.0.crown as f64)).unwrap_or(0.0);
    let (mw, mh) = app.worker.with(|ex| Ok::<_, String>((ex.world.geom.map_w_ft, ex.world.geom.map_h_ft))).await?;
    if !(0.0..mw).contains(&at[0]) || !(0.0..mh).contains(&at[1]) {
        return Err("the hill's middle is off the map".into());
    }
    app.edit("agent", 0, move |e| {
        let changed = id.is_some();
        if let Some(id) = &id
            && !e.plateaus.get(id).is_some_and(|p| p.hill.is_some())
        {
            return Err(format!("{id} was taken away meanwhile"));
        }
        if !changed && e.plateaus.len() >= ground::MAX_PLATEAUS {
            return Err(format!("a world holds at most {} plateaus and hills", ground::MAX_PLATEAUS));
        }
        let id = id.unwrap_or_else(|| new_id("h"));
        let mut p = ground::hill_for(&id, profile, at, rx, ry, angle, peak, crown)?;
        // (Its noise stays its own when changed.)
        if let (Some(h), Some(o)) = (p.hill.as_mut(), e.plateaus.get(&id).and_then(|q| q.hill.as_ref())) {
            h.seed = o.seed;
        }
        let h = p.hill.clone().expect("a hill");
        let reply = json!({
            "tool": "set_hill", "id": id, "changed": changed, "profile": h.profile, "peak_ft": p.level,
            "x_ft": h.at[0], "y_ft": h.at[1], "rx_ft": h.rx, "ry_ft": h.ry, "angle_deg": h.angle, "crown_ft": h.crown,
            "bbox": ground::footprint_bounds(&h).map(f64::round),
        });
        e.plateaus.insert(id, p);
        Ok(reply)
    })
    .await
}

/// The generated islets among `ids` (`i:`): each one's watchtower building (`b:<layout>:<id>`, its centre) when it has one.
async fn generated_islets(app: &Shared, ids: Vec<String>, single: bool) -> Result<Vec<(String, Option<(String, [f64; 2])>)>, String> {
    app.worker
        .with(move |ex| {
            let all = worldgen::t0::islets::islets(&ex.t0);
            let mut out = Vec::new();
            for (n, id) in ids.iter().enumerate().filter(|(_, id)| id.starts_with("i:")) {
                let i = all.iter().find(|i| &i.id == id).ok_or_else(|| format!("{}: no generated islet {id}", crate::tools::args::item(single, n)))?;
                let tower = worldgen::t0::islets::tower_building(&ex.world, &ex.t0, i);
                out.push((id.clone(), tower));
            }
            Ok(out)
        })
        .await
}

async fn delete(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::tools::args::ids(a)?;
    let single = a["ids"].is_null();
    let islets = generated_islets(app, ids.clone(), single).await?;
    app.edit("agent", 0, move |e| {
        for (i, id) in ids.iter().enumerate() {
            if !id.starts_with("i:") && !e.plateaus.contains_key(id) {
                return Err(format!("{}: no plateau {id}", crate::tools::args::item(single, i)));
            }
        }
        // (Its name, note and hidden mark go with it: nothing brings a plateau back but Undo.)
        for id in ids.iter().filter(|id| !id.starts_with("i:")) {
            e.plateaus.remove(id);
            e.renames.remove(id);
            e.notes.remove(id);
            e.hidden.remove(id);
        }
        // A generated islet is taken off the map by a hidden mark (`restore_islets` lifts it); so is its watchtower, as a building removed.
        let mut towers = Vec::new();
        for (id, tower) in &islets {
            e.hidden.insert(id.clone());
            if let Some((b, at)) = tower {
                e.buildings.insert(b.clone(), worldgen::world::BuildingEdit { at: *at, removed: true, ..Default::default() });
                towers.push(b.clone());
            }
        }
        let mut reply = json!({ "tool": "delete_plateau", "ids": ids });
        if !towers.is_empty() {
            reply["removed_towers"] = json!(towers);
        }
        Ok(reply)
    })
    .await
}

/// Bring generated islets taken away by `delete_plateau` back (their watchtowers stay removed: `restore_building`).
async fn restore_islets(app: &Shared, a: &Value) -> Result<Value, String> {
    let ids = crate::tools::args::ids(a)?;
    let single = a["ids"].is_null();
    for (i, id) in ids.iter().enumerate() {
        if !id.starts_with("i:") {
            return Err(format!("{}: {id} is not a generated islet (i:...)", crate::tools::args::item(single, i)));
        }
    }
    generated_islets(app, ids.clone(), single).await?;
    app.edit("agent", 0, move |e| {
        for (i, id) in ids.iter().enumerate() {
            if !e.hidden.contains(id) {
                return Err(format!("{}: {id} is not taken away", crate::tools::args::item(single, i)));
            }
        }
        for id in &ids {
            e.hidden.remove(id);
        }
        Ok(json!({ "tool": "restore_islets", "ids": ids }))
    })
    .await
}

async fn heights(app: &Shared, a: &Value) -> Result<Value, String> {
    let c = crate::tools::args::place(app, a).await?;
    let r = num(a, "radius_ft", 25.0)?;
    app.worker
        .with_mut(move |ex| {
            if !(0.0..ex.world.geom.map_w_ft).contains(&c[0]) || !(0.0..ex.world.geom.map_h_ft).contains(&c[1]) {
                return Err("the place is off the map".to_string());
            }
            let sea = ex.world.params().sea_level_ft as f32;
            let s = Squares::round(ex, [c[0] - r, c[1] - r, c[0] + r, c[1] + r], 0);
            let shaped = s.shaped(&ex.world.file.edits, Some(&ex.t0));
            let rows = |v: &[f32]| (0..s.h).map(|j| (0..s.w).map(|i| round1(v[(j * s.w + i) as usize] - sea)).collect::<Vec<_>>()).collect::<Vec<_>>();
            let wet: Vec<[i64; 2]> = (0..s.g.len()).filter(|&k| s.hw[k] > shaped[k]).map(|k| [k as i64 % s.w, k as i64 / s.w]).collect();
            let rect = s.rect();
            let edits = &ex.world.file.edits;
            let plateaus: Vec<Value> = edits
                .plateaus
                .iter()
                .filter_map(|(id, p)| {
                    let band = (p.edge.clamp(0.0, ground::MAX_EDGE_FT) as f64).max(SQUARE_FT);
                    let b = p.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
                    (b[0] - band <= rect[2] && b[2] + band >= rect[0] && b[1] - band <= rect[3] && b[3] + band >= rect[1])
                        .then(|| json!({ "id": id, "level_ft": round1(p.level - sea), "edge_ft": p.edge, "middle": geom::centroid(&p.pts), "bbox": b, "points": p.pts.len() }))
                })
                .collect();
            let block = (ground::BLOCK as f64) * SQUARE_FT;
            let blocks: Vec<&String> = edits
                .ground
                .keys()
                .filter(|k| ground::parse_key(k).is_some_and(|(bx, by)| (bx as f64 + 1.0) * block >= rect[0] && bx as f64 * block <= rect[2] && (by as f64 + 1.0) * block >= rect[1] && by as f64 * block <= rect[3]))
                .collect();
            let moved = (0..s.g.len()).filter(|&k| shaped[k] != s.g[k]).count();
            Ok::<Value, String>(json!({
                "origin": [(s.gx0 as f64 + 0.5) * SQUARE_FT, (s.gy0 as f64 + 0.5) * SQUARE_FT], "square_ft": SQUARE_FT, "columns": s.w, "rows": s.h,
                "generated": extent(&s.g, sea), "shaped": extent(&shaped, sea), "squares_shaped": moved,
                "plateaus": plateaus, "brushed_blocks": blocks,
                "wet": wet,
                "heights_generated": rows(&s.g), "heights_shaped": rows(&shaped),
            }))
        })
        .await
}
