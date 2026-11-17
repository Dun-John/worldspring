//! Where a hand islet or a wharf's root can go: the water at a point (the rivers and lakes as the
//! agent finds them) and the squares there (the executor's). `wharf` re-exports them.

use crate::t0::T0;
use crate::core::geom::P;
use crate::ground::wharf::unit;
use crate::town::geom;
use crate::wharf::WaterAt;
use crate::world::{Wharf, WharfKind};

pub fn water_at(world: &crate::world::World, t0: &T0, p: P) -> Option<WaterAt> {
    water_over(world, t0, p).filter(|w| w.depth_ft > 0.0 || w.kind == "river")
}

/// `water_at` where T0's water level is above its ground or not (T0's shore is coarser than a battlemap's: its water may
/// lie a little under its ground at a square the battlemap draws wet).
pub fn water_over(world: &crate::world::World, t0: &T0, p: P) -> Option<WaterAt> {
    let still = t0.sample_water(p[0], p[1]);
    let river = t0.rivers.crossing_level(p, p, 0.0, t0.cell_ft, &crate::lod::rivers::CrossingCache::default()).map_or(crate::t0::hydro::DRY, |(z, _)| z as f32);
    let level = still.max(river);
    if level <= crate::t0::hydro::DRY {
        return None;
    }
    let depth_ft = (level as f64 - t0.ground_at(p[0], p[1], 5.0)).max(0.0);
    if river > still {
        let r = crate::agent::river_at(world, t0, p, 50.0);
        return Some(WaterAt { kind: "river", id: r.as_ref().and_then(|r| r.id.clone()), name: r.and_then(|r| r.name), depth_ft, level });
    }
    let body = crate::agent::water_body_at(world, t0, p);
    let kind = if (still as f64) <= world.params().sea_level_ft + 1.0 { "sea" } else { "lake" };
    Some(WaterAt { kind, id: body.as_ref().map(|b| b.0.clone()), name: body.map(|b| b.1), depth_ft, level })
}

/// A hand islet for `key` (`h:<id>`) round `at`: `ground::islet_for`, with the water it stands in.
pub fn islet_at(world: &crate::world::World, t0: &T0, key: &str, at: P, radius_ft: f64, rise_ft: f64) -> Result<(crate::ground::Plateau, WaterAt), String> {
    let p = crate::ground::islet_for(key, at, radius_ft, rise_ft, t0)?;
    let w = water_at(world, t0, at).ok_or_else(|| format!("at: ({:.0}, {:.0}) is on dry land: an islet stands in a river, lake or the sea", at[0], at[1]))?;
    Ok((p, w))
}

/// `islet_at` by the fine shore (as a battlemap draws the water at `at`: `ground::islet_for_fine`), for the agent's tool.
pub fn islet_at_fine(ex: &mut crate::pipeline::Executor, key: &str, at: P, radius_ft: f64, rise_ft: f64) -> Result<(crate::ground::Plateau, WaterAt), String> {
    let sq = crate::geoquery::Squares::round(ex, [at[0] - 3.0, at[1] - 3.0, at[0] + 3.0, at[1] + 3.0], 0);
    let fine = sq.at(crate::core::floor(at[0] / crate::battlemap::SQUARE_FT) as i64, crate::core::floor(at[1] / crate::battlemap::SQUARE_FT) as i64).map(|k| (sq.hw[k], sq.g[k]));
    let (world, t0) = (&ex.world, &ex.t0);
    let p = crate::ground::islet_for_fine(key, at, radius_ft, rise_ft, t0, fine)?;
    let w = water_over(world, t0, at).or_else(|| {
        fine.filter(|f| f.0 > f.1).map(|(hw, g)| WaterAt { kind: if (hw as f64) <= world.params().sea_level_ft + 1.0 { "sea" } else { "lake" }, id: None, name: None, depth_ft: (hw - g) as f64, level: hw })
    });
    let w = w.ok_or_else(|| format!("at: ({:.0}, {:.0}) is on dry land: an islet stands in a river, lake or the sea", at[0], at[1]))?;
    Ok((p, w))
}

/// Why a pier or breakwater can't start where it does: `pts[0]` and the point 5 ft behind it both wet, as a battlemap
/// draws the water (more than 5 ft into it). Names the bank to start from; a quay has no root.
pub fn root_problem(ex: &mut crate::pipeline::Executor, w: &Wharf) -> Option<String> {
    if w.kind == WharfKind::Quay || w.pts.len() < 2 {
        return None;
    }
    let (p, u) = (w.pts[0], unit(geom::sub(w.pts[1], w.pts[0])));
    let back = geom::sub(p, geom::mul(u, 5.0));
    let mut sq = crate::geoquery::Squares::round(ex, [p[0].min(back[0]) - 1.0, p[1].min(back[1]) - 1.0, p[0].max(back[0]) + 1.0, p[1].max(back[1]) + 1.0], 0);
    // (As the battlemap draws it: an islet's top, a plateau over the water and water drawn by hand count, so a pier may start on an islet.)
    sq.add_waters(&ex.world.file.edits);
    let ground = sq.shaped(&ex.world.file.edits, Some(&ex.t0));
    let wet_sq = |q: P| {
        let k = sq.at(crate::core::floor(q[0] / crate::battlemap::SQUARE_FT) as i64, crate::core::floor(q[1] / crate::battlemap::SQUARE_FT) as i64);
        k.is_some_and(|k| sq.hw[k] > ground[k])
    };
    if !wet_sq(p) || !wet_sq(back) {
        return None;
    }
    let (world, t0) = (&ex.world, &ex.t0);
    let r = |q: P| [crate::core::round(q[0]), crate::core::round(q[1])];
    let where_ = |q: P| format!("({}, {})", r(q)[0], r(q)[1]);
    if let Some(rv) = crate::agent::river_at(world, t0, p, 50.0).filter(|rv| rv.in_water()) {
        let (b, _) = rv.bank_of(p, 5.0);
        let what = rv.name.clone().map_or_else(|| "a river".to_string(), |n| format!("{n} ({})", rv.id.clone().unwrap_or_default()));
        return Some(format!("path[0]: {} is in the water of {what}, {:.0} ft wide there: a {} starts on the bank, at about {}", where_(p), rv.width_ft, w.kind.name(), where_(b)));
    }
    let what = water_over(world, t0, p).map_or_else(|| "the water".to_string(), |a| match (a.name, a.id) {
        (Some(n), Some(i)) => format!("the water of {n} ({i})"),
        _ => format!("a {}", a.kind),
    });
    // The nearest dry square within 600 ft (rings of 40 ft over a box).
    let big = crate::geoquery::Squares::round(ex, [p[0] - 600.0, p[1] - 600.0, p[0] + 600.0, p[1] + 600.0], 0);
    let dry = (1..=120).flat_map(|k| (0..16).map(move |a| (k as f64 * 5.0, a))).find_map(|(d, a)| {
        let t = std::f64::consts::TAU * a as f64 / 16.0;
        let q = [p[0] + d * libm::cos(t), p[1] + d * libm::sin(t)];
        let k = big.at(crate::core::floor(q[0] / crate::battlemap::SQUARE_FT) as i64, crate::core::floor(q[1] / crate::battlemap::SQUARE_FT) as i64)?;
        (big.hw[k] <= big.g[k]).then_some(q)
    });
    let near = dry.map_or_else(String::new, |q| format!(": the nearest dry ground is at about {}", where_(q)));
    Some(format!("path[0]: {} is in {what}: a {} starts on the bank{near}", where_(p), w.kind.name()))
}
