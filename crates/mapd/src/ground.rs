//! MCP tools for ground shaped by hand at battlemap scale (`worldgen::ground`): brushed blocks
//! (`Edits::ground`: raise, lower, smooth, put back) and plateaus or pits (`Edits::plateaus`),
//! and the ground's heights as generated and as shaped.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use worldgen::battlemap::{SQ, SQUARE_FT};
use worldgen::core::tile::{HALO, PADDED, TileKey};
use worldgen::ground::{self, Brush, Plateau, Shaper, Stroke};
use worldgen::pipeline::Executor;
use worldgen::town::geom;
use worldgen::world::Edits;
use worldgen::{agent, town};

use crate::Shared;
use crate::notebook::new_id;
use crate::tools::{schema, text};

/// How far `ground_heights` reads each way from its place (ft): half its box's side.
const MAX_READ_FT: f64 = 200.0;

pub fn list() -> Vec<Value> {
    let pt = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": "[x_ft, y_ft]" });
    let at = json!({ "description": "A place: an id, {x_ft, y_ft} or [x_ft, y_ft] (or give x_ft and y_ft, or id, beside the other arguments)" });
    vec![
        json!({ "name": "shape_ground", "title": "Shape the ground", "description": "Shape the battlemap's ground by hand, as a round brush: raise or lower it (by up to height_ft in the middle, fading to the rim by hardness), smooth it (each square toward the mean of its neighbours, by strength) or put it back toward the ground as generated (brushed changes only: plateaus stay). A dab at a place (at, radius_ft) or a stroke along a path (path, width_ft); each square takes the stroke's strongest touch, so a path over itself doesn't pile up. Buildings, streets, roads and walls ride on the shaped ground; dry ground lowered never sinks below the water beside it; city streets made to climb steeply enough (0.15, and 0.03 more than before) are drawn as steps. A stroke reaches at most 48 blocks of 80 ft, a world holds 1500. Returns the blocks changed and the ground's lowest and highest under the brush before and after (ft above the sea).", "inputSchema": schema(json!({
            "mode": { "type": "string", "enum": ["raise", "lower", "smooth", "restore"] },
            "at": at,
            "radius_ft": { "type": "number", "description": "A dab's radius, 2.5 to 200 (default 15)" },
            "path": { "type": "array", "items": pt, "description": "A stroke's points (world ft)" },
            "width_ft": { "type": "number", "description": "A stroke's width, 5 to 400 (default 30)" },
            "height_ft": { "type": "number", "description": "Raise and lower: at most this far, 0 to 50 (default 5, one tier)" },
            "hardness": { "type": "number", "minimum": 0, "maximum": 1, "description": "0 soft (fades from the middle) to 1 hard (full to the rim); default 0.5" },
            "strength": { "type": "number", "minimum": 0, "maximum": 1, "description": "Smooth and restore: how far toward their goal; default 0.5" },
        }), &["mode"]) }),
        json!({ "name": "set_plateau", "title": "Make or change a plateau", "description": "A plateau (or a pit): the ground inside an outline levelled at one height, easing back to the ground round it over edge_ft (0: a sheer cliff at the outline). Give the outline (world ft, at most 48 points kept, spanning at most 1280 ft) or around: a building, tower or site whose footprint, grown by margin_ft, is the outline (a castle on a cliff). The level is level_ft above the sea, or rise_ft above the ground at the outline's middle (default 10; below zero digs a pit); a plateau inside another (a terrace) goes on top of it. Buildings on it stand at its level. With id, changes that plateau (fields left out stay). Returns its id (h:...).", "inputSchema": schema(json!({
            "id": { "type": "string", "description": "A plateau to change (h:...)" },
            "outline": { "type": "array", "items": pt },
            "around": { "type": "string", "description": "A building (b:...), tower (t:...) or site whose footprint makes the outline" },
            "margin_ft": { "type": "number", "description": "How far round the footprint (default 10)" },
            "level_ft": { "type": "number", "description": "Its level, ft above the sea" },
            "rise_ft": { "type": "number", "description": "Or its level above the ground at the outline's middle (default 10)" },
            "edge_ft": { "type": "number", "description": "The slope round it, 0 (sheer) to 60 ft (default 0)" },
        }), &[]) }),
        json!({ "name": "delete_plateau", "title": "Take plateaus away", "description": "Take plateaus or pits away (the ground goes back as it was), by id (h:...).", "inputSchema": schema(json!({ "id": { "type": "string" }, "ids": { "type": "array", "items": { "type": "string" } } }), &[]) }),
        json!({ "name": "ground_heights", "title": "Ground heights", "description": "The ground's height (ft above the sea) on every 5-ft square of the box reaching radius_ft (at most 200) east, west, north and south of a place: as generated and as shaped by hand (rows north to south, columns west to east, from origin: the first square's middle); squares under water; the plateaus and brushed blocks there. The battlemap draws a little relief on top (knolls, hollows) and levels road beds.", "inputSchema": schema(json!({ "at": at, "radius_ft": { "type": "number", "description": "Default 25" } }), &[]), "annotations": { "readOnlyHint": true } }),
    ]
}

/// Run a ground tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "shape_ground" => shape(app, a).await.map(text),
        "set_plateau" => set_plateau(app, a).await.map(text),
        "delete_plateau" => delete(app, a).await.map(text),
        "ground_heights" => heights(app, a).await.map(text),
        _ => return None,
    })
}

/// The ground as generated over a box of squares (global indices, inclusive), with the water
/// over each: read as a battlemap reads them (square centres are odd samples of the finest tiles).
struct Squares {
    gx0: i64,
    gy0: i64,
    w: i64,
    h: i64,
    g: Vec<f32>,
    hw: Vec<f32>,
}

impl Squares {
    fn read(ex: &mut Executor, gx0: i64, gy0: i64, gx1: i64, gy1: i64) -> Squares {
        let geom = ex.world.geom.clone();
        let lvl = geom.max_level;
        let sq = SQ as i64;
        let (w, h) = (gx1 - gx0 + 1, gy1 - gy0 + 1);
        let (mut g, mut hw) = (vec![0f32; (w * h) as usize], vec![0f32; (w * h) as usize]);
        for ty in gy0.div_euclid(sq)..=gy1.div_euclid(sq) {
            for tx in gx0.div_euclid(sq)..=gx1.div_euclid(sq) {
                let tile = ex.terrain(TileKey::surface(lvl, tx as u32, ty as u32));
                for gy in (ty * sq).max(gy0)..=((ty + 1) * sq - 1).min(gy1) {
                    for gx in (tx * sq).max(gx0)..=((tx + 1) * sq - 1).min(gx1) {
                        let (i, j) = ((gx - tx * sq) as usize, (gy - ty * sq) as usize);
                        let c = (2 * j + 1 + HALO) * PADDED + 2 * i + 1 + HALO;
                        let (x, y) = ((gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT);
                        let k = ((gy - gy0) * w + gx - gx0) as usize;
                        g[k] = tile.padded[c];
                        hw[k] = ex.t0.sample_water(x, y).max(tile.river_water[c]);
                    }
                }
            }
        }
        Squares { gx0, gy0, w, h, g, hw }
    }

    /// The squares round a box of world ft (and `pad` squares more), clamped to the map.
    fn round(ex: &mut Executor, b: [f64; 4], pad: i64) -> Squares {
        let geom = &ex.world.geom;
        let (mx, my) = ((geom.map_w_ft / SQUARE_FT).ceil() as i64, (geom.map_h_ft / SQUARE_FT).ceil() as i64);
        let sq = |v: f64| (v / SQUARE_FT).floor() as i64;
        let (gx0, gy0) = ((sq(b[0]) - pad).clamp(0, mx - 1), (sq(b[1]) - pad).clamp(0, my - 1));
        let (gx1, gy1) = ((sq(b[2]) + pad).clamp(gx0, mx - 1), (sq(b[3]) + pad).clamp(gy0, my - 1));
        Squares::read(ex, gx0, gy0, gx1, gy1)
    }

    fn at(&self, gx: i64, gy: i64) -> Option<usize> {
        let (i, j) = (gx - self.gx0, gy - self.gy0);
        (i >= 0 && j >= 0 && i < self.w && j < self.h).then_some((j * self.w + i) as usize)
    }

    fn rect(&self) -> [f64; 4] {
        [self.gx0 as f64 * SQUARE_FT, self.gy0 as f64 * SQUARE_FT, (self.gx0 + self.w) as f64 * SQUARE_FT, (self.gy0 + self.h) as f64 * SQUARE_FT]
    }

    /// Each square's ground shaped by `edits`.
    fn shaped(&self, edits: &Edits) -> Vec<f32> {
        match Shaper::new(edits, self.rect(), SQUARE_FT) {
            None => self.g.clone(),
            Some(s) => (0..self.g.len()).map(|k| s.at_square(self.gx0 + k as i64 % self.w, self.gy0 + k as i64 / self.w, self.g[k], self.hw[k]).0).collect(),
        }
    }
}

/// A point from an array, or a place (an id or {x_ft, y_ft}).
async fn point_of(app: &Shared, v: &Value) -> Result<[f64; 2], String> {
    if let Some(p) = v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()) {
        return match p[..] {
            [x, y] if x.is_finite() && y.is_finite() => Ok([x, y]),
            _ => Err("a point is [x_ft, y_ft]".into()),
        };
    }
    crate::tools::place(app, v).await
}

/// The place a call gives: `at`, else `x_ft`/`y_ft` or `id` beside its other arguments.
async fn place_of(app: &Shared, a: &Value) -> Result<[f64; 2], String> {
    if a["at"].is_null() {
        crate::tools::place(app, a).await
    } else {
        point_of(app, &a["at"]).await
    }
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
    let (path, radius) = if a["path"].is_null() {
        (vec![place_of(app, a).await?], num(a, "radius_ft", 15.0)?)
    } else {
        let mut path = Vec::new();
        for p in a["path"].as_array().ok_or("path: a list of [x_ft, y_ft]")? {
            path.push(point_of(app, p).await?);
        }
        if path.is_empty() {
            return Err("path: at least one point".into());
        }
        (path, num(a, "width_ft", 30.0)? * 0.5)
    };
    let (height, hardness, strength) = (num(a, "height_ft", 5.0)?, num(a, "hardness", 0.5)?, num(a, "strength", 0.5)?);
    // Checked here as `stamp_brush` checks them, before any ground is read.
    if !(2.5..=200.0).contains(&radius) || !(0.0..=50.0).contains(&height) {
        return Err("radius_ft 2.5 to 200 (width_ft 5 to 400), height_ft 0 to 50".into());
    }
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
    let (squares, drawn) = app
        .worker
        .run(move |g| {
            let ex = g.ex.as_mut().ok_or("no world is open: open the map app (it connects to mapd and shares its world)")?;
            let (mw, mh) = (ex.world.geom.map_w_ft, ex.world.geom.map_h_ft);
            if path.iter().any(|p| !(0.0..mw).contains(&p[0]) || !(0.0..mh).contains(&p[1])) {
                return Err("the brush is off the map".to_string());
            }
            let squares = Squares::round(ex, b, 1);
            let drawn = if smooth { drawn_ground(ex, &under2) } else { BTreeMap::new() };
            Ok::<_, String>((squares, drawn))
        })
        .await?;
    let sea = app.worker.with(|ex| Ok(ex.world.params().sea_level_ft as f32)).await?;
    let reply = app
        .edit("agent", 0, move |e| {
            let under: Vec<usize> = under.iter().filter_map(|&(gx, gy)| squares.at(gx, gy)).collect();
            let pick = |v: Vec<f32>| under.iter().map(|&k| v[k]).collect::<Vec<f32>>();
            let before = pick(squares.shaped(e));
            // (A neighbour off the map reads the nearest square on it: `squares` holds the
            // brush's box and a square round it, clamped to the map.)
            let height_of = |gx: i64, gy: i64| -> f64 {
                let (gx, gy) = (gx.clamp(squares.gx0, squares.gx0 + squares.w - 1), gy.clamp(squares.gy0, squares.gy0 + squares.h - 1));
                drawn.get(&(gx, gy)).map(|&h| h as f64).or_else(|| squares.at(gx, gy).map(|k| squares.g[k] as f64)).unwrap_or(0.0)
            };
            let stroke = Stroke { mode, path: &path2, radius_ft: radius, height_ft: height, hardness, strength };
            let changed = ground::stamp_brush(&mut e.ground, &stroke, &height_of)?;
            let after = pick(squares.shaped(e));
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
            let (c, r, _) = *worldgen::interior::towers(&town::layout(world, t0, *li)).get(*k).ok_or_else(|| format!("no tower {id}"))?;
            Ok(geom::circle(c, r, 16))
        }
        _ => {
            let li = agent::layout_of(world, t0, id).ok_or_else(|| format!("{id}: give a building (b:...), a tower (t:...) or a site, or an outline"))?;
            if li < t0.settlements.len() {
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
    let edge = match (&a["edge_ft"], &old) {
        (Value::Null, Some(p)) => p.edge as f64,
        (Value::Null, None) => 0.0,
        (v, _) => v.as_f64().filter(|e| (0.0..=ground::MAX_EDGE_FT as f64).contains(e)).ok_or("edge_ft: 0 to 60")?,
    };
    // The outline: given, round a footprint, or the one it has.
    let outline: Option<Vec<[f64; 2]>> = if let Some(o) = a["outline"].as_array() {
        let mut pts = Vec::new();
        for p in o {
            pts.push(point_of(app, p).await?);
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
        .run(move |g| {
            let ex = g.ex.as_mut().ok_or("no world is open: open the map app (it connects to mapd and shares its world)")?;
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
        let here = squares.shaped(&others)[0] as f64;
        let level = match (level_ft, rise, &old) {
            (Some(l), _, _) => l + sea,
            (None, Some(r), _) => here + r,
            (None, None, Some(p)) => p.level as f64,
            (None, None, None) => here + 10.0,
        };
        let p = match (&outline, &old) {
            (Some(o), _) => ground::plateau_for(o, level, edge)?,
            (None, Some(p)) => {
                let p = Plateau { pts: p.pts.clone(), level: ((level * 10.0).round() * 0.1) as f32, edge: edge as f32 };
                if let Some(err) = p.problem() {
                    return Err(err);
                }
                p
            }
            (None, None) => unreachable!("an outline or a plateau"),
        };
        let b = p.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, q| [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]);
        let reply = json!({
            "tool": "set_plateau", "id": id, "changed": changed,
            "level_ft": round1(p.level - sea as f32), "ground_ft": round1(here as f32 - sea as f32), "edge_ft": p.edge,
            "points": p.pts.len(), "area_sq_ft": geom::area(&p.pts).abs().round(), "bbox": [b[0], b[1], b[2], b[3]],
        });
        e.plateaus.insert(id, p);
        Ok(reply)
    })
    .await
}

async fn delete(app: &Shared, a: &Value) -> Result<Value, String> {
    let mut ids: Vec<String> = a["ids"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
    ids.extend(a["id"].as_str().map(str::to_string));
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Err("give id or ids (h:...)".into());
    }
    app.edit("agent", 0, move |e| {
        for id in &ids {
            e.plateaus.remove(id).ok_or_else(|| format!("no plateau {id}"))?;
        }
        Ok(json!({ "tool": "delete_plateau", "ids": ids }))
    })
    .await
}

async fn heights(app: &Shared, a: &Value) -> Result<Value, String> {
    let c = place_of(app, a).await?;
    let r = num(a, "radius_ft", 25.0)?;
    if !(0.0..=MAX_READ_FT).contains(&r) {
        return Err(format!("radius_ft: 0 to {MAX_READ_FT}"));
    }
    app.worker
        .run(move |g| {
            let ex = g.ex.as_mut().ok_or("no world is open: open the map app (it connects to mapd and shares its world)")?;
            if !(0.0..ex.world.geom.map_w_ft).contains(&c[0]) || !(0.0..ex.world.geom.map_h_ft).contains(&c[1]) {
                return Err("the place is off the map".to_string());
            }
            let sea = ex.world.params().sea_level_ft as f32;
            let s = Squares::round(ex, [c[0] - r, c[1] - r, c[0] + r, c[1] + r], 0);
            let shaped = s.shaped(&ex.world.file.edits);
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
