//! T0: the authoritative continent-scale grids covering the map rectangle.
//!
//! Pipeline: plates → uplift → stream-power erosion (half resolution) → heights in feet +
//! bathymetry → volcanoes → climate → hydrology (lakes, rivers) → biomes → named features.
//! Tiles sample height, water surface, biome and coast distance from here at any level.

pub mod biome;
pub mod climate;
pub mod erosion;
pub mod features;
pub mod flood;
pub mod hydro;
pub mod names;
pub mod plates;
pub mod roads;
pub mod settle;
pub mod sketch;
pub mod volcano;

use crate::World;
use crate::core::grid::Grid;
use crate::lod::rivers::{RiverCurve, RiverNet, river_seed};
use crate::lod::roads::{RoadCurve, RoadNet, road_seed};
use crate::core::noise::{Fbm, fbm, smoothstep};
use features::Overlay;
use volcano::Activity;

const MAGIC: u32 = 0x5430_4734; // "T0G4"
/// Height of the bank at the dry edge of a shore cell above the water (ft).
const SHORE_BANK_FT: f64 = 6.0;
/// Ocean distance encoded in the biome texture saturates here (ft).
pub const COAST_ENCODE_FT: f64 = 250_000.0;

pub struct T0 {
    pub height: Grid<f32>,
    /// Water surface elevation (sea / lake level) or `hydro::DRY`.
    pub water: Grid<f32>,
    /// Distance from land for ocean cells (ft); 0 on land.
    pub coast: Grid<f32>,
    /// primary | secondary << 8 | blend << 16 (see `biome::classify`).
    pub biome: Grid<u32>,
    pub cell_ft: f64,
    /// River curves (shared by every zoom level) with a spatial index.
    pub rivers: RiverNet,
    /// Road polylines (grade-profiled) with a spatial index.
    pub roads: RoadNet,
    /// Settlements (their layouts are generated on demand, see `crate::town`).
    pub settlements: Vec<settle::Settlement>,
    /// Points of interest (ruins, towers, roadside inns), same order as their features; then
    /// the sites created by edits (`apply_edits`), from `base_pois` on.
    pub pois: Vec<settle::Poi>,
    /// How many points of interest were generated (created sites follow).
    pub base_pois: usize,
    /// Per created site (index `poi - base_pois`): removed, and what lies beneath a ruin.
    pub created: Vec<CreatedSite>,
    biome_seed: u64,
    /// mips[0] is `height`; each next level is a 2x2 box downsample.
    mips: Vec<Grid<f32>>,
    /// `ground_nodes` kept for one lattice while generating (they depend on `height` and
    /// `water` only, which never change after `from_grids`); empty otherwise.
    ground_cache: std::sync::OnceLock<(f64, std::sync::Arc<(Vec<f64>, i64, i64)>)>,
    /// Generation-time products; `None` when loaded from bytes in another worker.
    pub extra: Option<T0Extra>,
}

/// A created site's generation options (see `world::Created`).
#[derive(Clone, Copy, Debug, Default)]
pub struct CreatedSite {
    pub removed: bool,
    pub under: Option<crate::under::UnderKind>,
    /// Its site underground: size, levels, theme.
    pub spec: crate::under::SiteSpec,
}

/// A coarse picture of a world (see `T0::preview`).
pub struct Preview {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
    pub conflicts: Vec<sketch::Conflict>,
}

/// What the terrain stages produce (see `T0::shape`).
struct Shaped {
    height: Vec<f64>,
    land: Vec<bool>,
    clim: climate::Climate,
    hydro: hydro::Hydro,
    biome: Vec<u32>,
    volcanoes: Vec<volcano::Volcano>,
    conflicts: Vec<sketch::Conflict>,
    pins: Vec<sketch::Pin>,
}

pub struct T0Extra {
    pub temp: Vec<f32>,
    pub precip: Vec<f32>,
    pub hydro: hydro::Hydro,
    pub volcanoes: Vec<volcano::Volcano>,
    pub overlay: Overlay,
    pub settlements: Vec<settle::Settlement>,
    pub pois: Vec<settle::Poi>,
    pub crossings: Vec<roads::Crossing>,
}

impl T0 {
    pub fn generate(world: &World) -> T0 {
        Self::generate_with_progress(world, &mut |_, _| {})
    }

    /// The terrain stages on a `w` × `h` grid of `cell` ft (the T0 grid, or a coarse one for
    /// sketch previews): plates and the sketch's land, sea and ranges → erosion → heights →
    /// volcanoes → the sketch's rivers → climate → hydrology → biomes and the sketch's paint,
    /// and where its pins go.
    fn shape(world: &World, w: usize, h: usize, cell: f64, progress: &mut dyn FnMut(&str, f64)) -> Shaped {
        let p = world.params();
        let sea = p.sea_level_ft;
        let n = w * h;

        progress("plates", 0.0);
        let mut tect = plates::build(world, w, h);
        let mut thr = plates::threshold_for_fraction(&tect.crust, p.land_fraction);
        // Drawn land and sea decide the land mask; drawn ranges add uplift.
        let raster = sketch::Raster { w, h, cell };
        if let Some(crust) = sketch::land_crust(world, raster, &tect.crust, thr) {
            tect.crust = crust;
            thr = 0.0;
            // Rift valleys follow plate boundaries, which drawn land doesn't: no straight
            // channels through it.
            tect.rift.iter_mut().for_each(|r| *r = 0.0);
        }
        sketch::add_ranges(world, raster, &mut tect.uplift);
        let mut land: Vec<bool> = tect.crust.iter().map(|&c| c > thr).collect();

        // Erosion at half resolution.
        let (ew, eh) = (w.div_ceil(2), h.div_ceil(2));
        let e_crust = downsample(w, h, &tect.crust);
        let e_land: Vec<bool> = e_crust.iter().map(|&c| c > thr).collect();
        let e_up = downsample(w, h, &tect.uplift);
        let iterations = (20.0 + 50.0 * p.erosion).round() as usize;
        let z = erosion::erode(ew, eh, &e_land, &e_up, iterations, world.stream("t0.erosion"), |f| progress("erosion", f));

        // Model units → feet: the 99.5th percentile of land maps to 85% of max elevation.
        let mut land_z: Vec<f64> = z.iter().zip(&e_land).filter(|(_, l)| **l).map(|(v, _)| *v).collect();
        land_z.sort_by(|a, b| a.total_cmp(b));
        let p995 = land_z.get(land_z.len() * 995 / 1000).copied().unwrap_or(1.0).max(1e-9);
        // Lowlands fill with sediment (which stream-power alone lacks): blend weakly uplifted
        // land toward a heavily smoothed surface so plains read as plains, ranges stay rugged.
        let mut smooth = z.clone();
        for _ in 0..4 {
            smooth = blur_land(ew, eh, &smooth, &e_land, 3);
        }
        let mut up_sorted: Vec<f64> = e_up.iter().zip(&e_land).filter(|(_, l)| **l).map(|(u, _)| *u).collect();
        up_sorted.sort_by(|a, b| a.total_cmp(b));
        let up_ref = up_sorted.get(up_sorted.len() * 95 / 100).copied().unwrap_or(1.0).max(1e-9);
        let z: Vec<f64> = (0..ew * eh)
            .map(|k| {
                let rugged = smoothstep(0.12, 0.45, e_up[k] / up_ref);
                smooth[k] + (z[k] - smooth[k]) * (0.25 + 0.75 * rugged)
            })
            .collect();
        let zg = Grid::from_vec(ew, eh, z.iter().map(|&v| (v / p995 * 0.85 * p.max_elev_ft) as f32).collect());

        progress("terrain", 0.0);
        let dist_land = climate::distance_to(w, h, &land);
        let (s_detail, s_kettle, s_bathy) = (world.stream("t0.detail"), world.stream("t0.kettle"), world.stream("t0.bathy"));
        let cap = 0.9 * p.max_elev_ft;
        let (mut detail, mut kettle, mut bathy) = (Fbm::new(s_detail, 3, 2.0, 0.5), Fbm::new(s_kettle, 2, 2.0, 0.5), Fbm::new(s_bathy, 4, 2.0, 0.5));
        let mut height = vec![0.0f64; n];
        for j in 0..h {
            let lat = climate::latitude(world, j, h);
            for i in 0..w {
                let k = j * w + i;
                let (fi, fj) = (i as f64, j as f64);
                if land[k] {
                    let base = zg.sample_cubic((fi - 0.5) / 2.0, (fj - 0.5) / 2.0).max(0.0);
                    let mut v = base * (1.0 + 0.12 * detail.at(fi / 3.0, fj / 3.0));
                    if v > cap {
                        // Soft ceiling: approach max elevation, never exceed it.
                        let room = p.max_elev_ft - cap;
                        v = cap + room * libm::tanh((v - cap) / room);
                    }
                    // Rift valleys subside (rift lakes); glaciated lowlands get kettle hollows.
                    v -= tect.rift[k] * 2_200.0;
                    if crate::core::fabs(lat) > 50.0 {
                        v -= 160.0 * smoothstep(0.85, 0.95, kettle.at(fi / 5.0, fj / 5.0) + 0.5);
                    }
                    height[k] = sea + 5.0 + v;
                } else {
                    let d_mi = dist_land[k] * cell / 5280.0;
                    let depth = 60.0 + 540.0 * smoothstep(0.0, 50.0, d_mi) + 11_000.0 * smoothstep(40.0, 220.0, d_mi)
                        + 600.0 * bathy.at(fi / 10.0, fj / 10.0)
                        - 6_500.0 * tect.ridge[k];
                    height[k] = sea - depth.max(20.0);
                }
            }
        }

        progress("volcanoes", 0.0);
        let volcanoes = volcano::place(world, w, h, cell, &tect.arc, &height);
        volcano::apply(&volcanoes, w, h, cell, sea, p.max_elev_ft, &mut height);
        for k in 0..n {
            land[k] = land[k] || height[k] > sea;
        }
        // Land that subsided below sea level (rifts, kettles) and touches the ocean floods:
        // it becomes a gulf or sound instead of dry ground below the waterline.
        let mut queue: Vec<usize> = (0..n).filter(|&k| !land[k]).collect();
        while let Some(k) = queue.pop() {
            for (nb, _) in flood::neighbors(w, h, k) {
                if land[nb] && height[nb] < sea {
                    land[nb] = false;
                    queue.push(nb);
                }
            }
        }

        // Drawn rivers are carved downhill along their lines (and fed at their sources).
        let mut conflicts = Vec::new();
        sketch::check_ranges(world, raster, &land, &mut conflicts);
        let feed = sketch::carve_rivers(world, raster, &mut height, &land, &mut conflicts);

        progress("climate", 0.0);
        let clim = climate::build(world, w, h, cell, &height, &land);
        progress("rivers", 0.0);
        let hydro = hydro::build(w, h, &mut height, &land, &clim, sea, p.river_density, &feed);

        progress("biomes", 0.0);
        let vents = volcanoes
            .iter()
            .filter(|v| v.activity == Activity::Active)
            .map(|v| (v.cx, v.cy, 1.3 * v.radius_ft / cell))
            .collect();
        let mut biome = biome::classify(world, w, h, cell, &height, &land, &clim, &hydro, &biome::Volcanic { vents });
        sketch::paint_biomes(world, raster, &land, &hydro.lake_of, &mut biome);
        let pins = sketch::pins(world, raster, &land, &mut conflicts);
        Shaped { height, land, clim, hydro, biome, volcanoes, conflicts, pins }
    }

    pub fn generate_with_progress(world: &World, progress: &mut dyn FnMut(&str, f64)) -> T0 {
        let g = &world.geom;
        let (w, h, cell) = (g.t0_w, g.t0_h, g.t0_cell_ft);
        let n = w * h;
        let Shaped { height, land, clim, hydro, biome, volcanoes, conflicts, pins } = Self::shape(world, w, h, cell, progress);
        let dist_land = climate::distance_to(w, h, &land);
        let coast: Vec<f32> = (0..n).map(|k| if land[k] { 0.0 } else { (dist_land[k] * cell) as f32 }).collect();

        let mut rivers = build_river_net(world, w, cell, &height, &land, &hydro);
        let hgrid = Grid::from_vec(w, h, height.iter().map(|&v| v as f32).collect());
        let base = Self::from_grids(hgrid, Grid::from_vec(w, h, hydro.water.clone()), Grid::from_vec(w, h, coast.clone()), Grid::from_vec(w, h, biome.clone()), cell, world.stream("t0.biome.warp"), RiverNet::default(), RoadNet::new(Vec::new(), (w - 1) as f64 * cell, (h - 1) as f64 * cell, cell));
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
        // The ground's lattice nodes, computed once for every sampler and grid below.
        let _ = base.ground_cache.set((lattice, std::sync::Arc::new(base.ground_nodes_uncached(lattice))));
        let ground = base.ground_sampler(lattice);
        let plan = {
            // Levels that agree with the ground the terrain is built on (before anything is
            // placed against the curves, whose meanders follow the levels).
            let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
            // Standing water: the lookup gives a level near any wet cell; it is water only
            // where that level is above the ground.
            rivers.settle_levels(&ground, &|x, y| base.sample_lake(x, y).0 as f64 > ground(x, y), cell);
            rivers.settle(&ground, cell);
            rivers.settled_on = Some(lattice);
            // The ground roads are planned on: that ground, in the river valleys, every half cell.
            let s = roads::Plan::SCALE;
            let (pw, ph) = (((w - 1) as f64 * s) as usize + 1, ((h - 1) as f64 * s) as usize + 1);
            let mut z = base.ground_grid(lattice, pw, ph, &|i| i as f64 * cell / s);
            rivers.valley_grid(&mut z, pw, [0.0, 0.0], cell / s);
            Grid::from_vec(pw, ph, z)
        };
        let plan = roads::Plan { grid: plan, cw: w, ch: h };
        progress("settlements", 0.0);
        // Staged, so later tiers grow on the roads the earlier ones made: cities, then king's
        // roads between them; towns (favouring those roads and their junctions), then roads;
        // villages (favouring any road).
        let sinp = settle::Inputs { world, w, h, cell_ft: cell, height: &height, land: &land, biome: &biome, hydro: &hydro, pins: &pins };
        // The ground roads are planned on, at any point (the plan's grid every half cell is
        // too coarse for switchbacks to see a narrow valley).
        let planned = |x: f64, y: f64| rivers.valley(ground(x, y), x, y);
        let rinp = roads::Inputs { world, plan: &plan, ground: &planned, w, h, cell_ft: cell, height: &height, land: &land, biome: &biome, hydro: &hydro, routes: Default::default() };
        use settle::Tier as T;
        let mut settlements = settle::place(&sinp, Vec::new(), &[T::Metropolis, T::City], None);
        let usage = roads::preview(&rinp, &settlements, &[roads::RoadClass::KingsRoad]);
        settlements = settle::place(&sinp, settlements, &[T::Town], Some(&usage));
        let usage = roads::preview(&rinp, &settlements, &[roads::RoadClass::KingsRoad, roads::RoadClass::Road]);
        settlements = settle::place(&sinp, settlements, &[T::Village], Some(&usage));
        // Waterside settlements stand on their water, not at the centre of their map cell.
        let water_grid = Grid::from_vec(w, h, hydro.water.clone());
        let biome_seed = world.stream("t0.biome.warp");
        // Water where its level is above the ground the terrain draws (T0::ground_at, which
        // needs the height mips: a road-less T0 serves for it).
        let ground_t0 = Self::from_grids(
            Grid::from_vec(w, h, height.iter().map(|&v| v as f32).collect()),
            water_grid.clone(),
            Grid::from_vec(w, h, vec![0.0; n]),
            Grid::from_vec(w, h, vec![0u32; n]),
            cell,
            biome_seed,
            RiverNet::default(),
            RoadNet::new(Vec::new(), (w - 1) as f64 * cell, (h - 1) as f64 * cell, cell),
        );
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
        // (Its ground nodes are base's: the same height and water grids.)
        if let Some(c) = base.ground_cache.get() {
            let _ = ground_t0.ground_cache.set(c.clone());
        }
        let ground_wet = ground_t0.ground_sampler(lattice);
        let wet = |x: f64, y: f64| {
            let level = ground_t0.sample_water(x, y);
            level > hydro::DRY && level as f64 > ground_wet(x, y)
        };
        let nearest_river = |x: f64, y: f64| {
            let mut best: Option<(f64, [f64; 2], f64)> = None;
            for (ri, k) in rivers.segments_near(x - 1.5 * cell, y - 1.5 * cell, x + 1.5 * cell, y + 1.5 * cell, 0.0) {
                let r = &rivers.rivers[ri as usize];
                for j in 0..=24 {
                    let c = r.eval(k as usize, j as f64 / 24.0, 20.0, cell);
                    let d = crate::core::sqrt((c.p[0] - x) * (c.p[0] - x) + (c.p[1] - y) * (c.p[1] - y));
                    if d < 1.5 * cell && best.is_none_or(|b| d < b.0) {
                        best = Some((d, c.p, 0.5 * c.w));
                    }
                }
            }
            best.map(|(_, p, hw)| (p, hw))
        };
        settle::snap_to_water(&mut settlements, cell, &wet, &nearest_river);
        let vents_at: Vec<(f64, f64, f64)> = volcanoes.iter().map(|v| (v.cx, v.cy, v.radius_ft)).collect();
        let mut pois = settle::place_pois(&sinp, &settlements, &vents_at);
        // Towns sit beside rivers, not in them (the fine channel meanders through T0 cells).
        let clear = crate::lod::rivers::ClearCache::default();
        for s in &mut settlements {
            // Villages sit on the bank (fishing villages work the water); towns stand back.
            let margin = if s.tier == settle::Tier::Village { 40.0 } else { 300.0 };
            (s.x, s.y) = crate::lod::rivers::clear_of_rivers_with(&rivers, s.x, s.y, margin, cell, &clear);
        }
        for p in &mut pois {
            (p.x, p.y) = crate::lod::rivers::clear_of_rivers_with(&rivers, p.x, p.y, 150.0, cell, &clear);
        }
        progress("roads", 0.0);
        let mut network = roads::build(&rinp, &settlements);
        // Roads keep out of the belts the rivers meander in, crossing each one once.
        let belts = crate::lod::roads::BeltCache::default();
        for (i, r) in network.roads.iter_mut().enumerate() {
            let curve = RoadCurve::new(r.class, r.pts.clone(), r.z.clone(), r.wander.clone(), road_seed(world.seed, i));
            if let Some((pts, z, wander)) = crate::lod::roads::unweave(&curve, &rivers, cell, &|x, y| wet(x, y), &belts) {
                (r.pts, r.z, r.wander) = (pts, z, wander);
            }
            roads::tidy(r);
            // Corners left by pushing runs out of meander belts turn on arcs too (the profile
            // is fitted again below).
            let z = roads::round_corners(&r.pts, &r.z, 0.06 * cell, 0.05 * cell).1;
            (r.pts, r.wander) = roads::round_corners(&r.pts, &r.wander, 0.06 * cell, 0.05 * cell);
            r.z = z;
            // The bends are in the points (`roads::follow_terrain`): no wander on top, which
            // would ignore the rivers and the profile (and, where two roads were joined into one,
            // swing at full strength round the joint).
            r.wander.iter_mut().for_each(|w| *w = 0.0);
        }
        let (map_w, map_h) = ((w - 1) as f64 * cell, (h - 1) as f64 * cell);
        let road_net = RoadNet::new(
            network
                .roads
                .iter()
                .enumerate()
                .map(|(i, r)| RoadCurve::new(r.class, r.pts.clone(), r.z.clone(), r.wander.clone(), road_seed(world.seed, i)))
                .collect(),
            map_w,
            map_h,
            cell,
        );
        // Roadside inns stand on the road as drawn (the curve wanders off its control points).
        for ws in &network.waystations {
            let mut ws = ws.clone();
            (ws.x, ws.y) = road_net.nearest_point(ws.x, ws.y, 1.5 * cell).unwrap_or((ws.x, ws.y));
            pois.push(ws);
        }

        progress("names", 0.0);
        let mut overlay = features::extract(&features::Inputs {
            world,
            w,
            h,
            cell_ft: cell,
            height: &height,
            land: &land,
            biome: &biome,
            clim: &clim,
            hydro: &hydro,
            volcanoes: &volcanoes,
            settlements: &settlements,
            pois: &pois,
        });
        overlay.conflicts = conflicts;
        progress("done", 1.0);

        let mut t0 = Self::from_grids(
            Grid::from_vec(w, h, height.iter().map(|&v| v as f32).collect()),
            Grid::from_vec(w, h, hydro.water.clone()),
            Grid::from_vec(w, h, coast),
            Grid::from_vec(w, h, biome),
            cell,
            world.stream("t0.biome.warp"),
            rivers,
            road_net,
        );
        // Its ground nodes are base's (the same height and water grids).
        if let Some(c) = base.ground_cache.get() {
            let _ = t0.ground_cache.set(c.clone());
        }
        // Road beds follow the ground the terrain actually builds along them (road corridors
        // keep refinement detail off it), so they cut and fill a few feet, not trenches.
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
        let curves: Vec<RoadCurve> = {
            let ground = t0.ground_sampler(lattice);
            let crossings = crate::lod::rivers::CrossingCache::default();
            let rs = &t0.roads.roads;
            // Where roads meet or cross, a point on each (their beds meet at one level there).
            let mut pts: Vec<Vec<[f64; 2]>> = rs.iter().map(|r| r.pts.clone()).collect();
            let mut wander: Vec<Vec<f32>> = rs.iter().map(|r| r.wander.clone()).collect();
            let meets = roads::meeting_points(&mut pts, &mut wander);
            let class: Vec<roads::RoadClass> = rs.iter().map(|r| r.class).collect();
            // In the river valleys the terrain carves (a road along a valley side runs on it, not
            // on a dike above it), and over every river it crosses: clear of the water by a
            // bridge's clearance at both ends of the crossing (the profile ramps up to it).
            let terrain: Vec<Vec<f64>> = pts.iter().map(|q| q.iter().map(|p| t0.rivers.valley(ground(p[0], p[1]), p[0], p[1])).collect()).collect();
            let floor: Vec<Vec<f64>> = pts
                .iter()
                .zip(&class)
                .map(|(q, &c)| {
                    let mut floor = vec![f64::MIN; q.len()];
                    for k in 0..q.len().saturating_sub(1) {
                        // (A ford wades through and a ferry lands at the water: no clearance.)
                        if let Some((z, w)) = t0.rivers.crossing_level(q[k], q[k + 1], 0.5 * c.width_ft() + 2.0, cell, &crossings)
                            && roads::crossing_kind(c, w) == roads::CrossingKind::Bridge
                        {
                            let deck = z + crate::battlemap::DECK_CLEARANCE_FT as f64;
                            floor[k] = floor[k].max(deck);
                            floor[k + 1] = floor[k + 1].max(deck);
                        }
                    }
                    floor
                })
                .collect();
            let z = roads::level_profiles(&pts, &class, &terrain, &floor, &meets);
            rs.iter().zip(pts).zip(z).zip(wander).map(|(((r, p), z), w)| RoadCurve::new(r.class, p, z, w, r.seed)).collect()
        };
        t0.roads = RoadNet::new(curves, map_w, map_h, cell);
        t0.ground_cache = Default::default();
        for (s, &c) in settlements.iter_mut().zip(&overlay.settlement_cultures) {
            s.culture = c;
        }
        t0.settlements = settlements.clone();
        t0.base_pois = pois.len();
        t0.pois = pois.clone();
        t0.extra = Some(T0Extra { temp: clim.temp, precip: clim.precip, hydro, volcanoes, overlay, settlements, pois, crossings: network.crossings });
        t0
    }

    #[allow(clippy::too_many_arguments)]
    fn from_grids(height: Grid<f32>, water: Grid<f32>, coast: Grid<f32>, biome: Grid<u32>, cell_ft: f64, biome_seed: u64, rivers: RiverNet, roads: RoadNet) -> T0 {
        let mut mips = vec![height.clone()];
        while mips.last().is_some_and(|m| m.w > 4 && m.h > 4) {
            let next = mips.last().unwrap().downsample();
            mips.push(next);
        }
        let mut t0 = T0 { height, water, coast, biome, cell_ft, rivers: RiverNet::default(), roads, settlements: Vec::new(), pois: Vec::new(), base_pois: 0, created: Vec::new(), biome_seed, mips, ground_cache: Default::default(), extra: None };
        // Water surfaces follow the ground the terrain is built on along each curve
        // (generated and loaded alike). That ground's lattice is the coarsest spacing
        // (2.5 · 2^n ft) at least a T0 cell.
        let mut lattice = 2.5;
        while lattice < cell_ft {
            lattice *= 2.0;
        }
        let mut rivers = rivers;
        // (Generating, they were settled on this ground already: the same grids, lattice.)
        if rivers.settled_on != Some(lattice) {
            rivers.settle(&t0.ground_sampler(lattice), cell_ft);
            rivers.settled_on = Some(lattice);
        }
        t0.rivers = rivers;
        t0
    }

    fn with_settlements(mut self, settlements: Vec<settle::Settlement>, pois: Vec<settle::Poi>) -> T0 {
        self.settlements = settlements;
        self.base_pois = pois.len();
        self.pois = pois;
        self
    }

    /// Put the world file's created sites after the generated points of interest (replacing
    /// any put there before). Their seeds come from their ids, so a site is the same whenever
    /// and wherever it is applied.
    pub fn apply_edits(&mut self, world: &World) {
        use settle::PoiKind;
        self.pois.truncate(self.base_pois);
        self.created.clear();
        for c in &world.file.edits.created {
            let kind = match c.kind.as_str() {
                "tower" => PoiKind::Tower,
                "camp" => PoiKind::Camp,
                "waystation" => PoiKind::Waystation,
                "cave" => PoiKind::Cave,
                "mine" => PoiKind::Mine,
                "lava_tube" => PoiKind::LavaTube,
                "entrance" => PoiKind::Entrance,
                "building" => PoiKind::Building,
                _ => PoiKind::Ruin,
            };
            let under = c.under.as_deref().and_then(crate::under::UnderKind::parse);
            let spec = crate::under::SiteSpec {
                size: c.size.as_deref().and_then(crate::under::SiteSize::parse),
                levels: c.levels,
                theme: c.theme.as_deref().and_then(crate::under::theme).map(|i| i as u8),
            };
            let seed = crate::core::hash::fnv64(c.id.as_bytes()) ^ world.seed;
            self.pois.push(settle::Poi { kind, x: c.x, y: c.y, seed });
            self.created.push(CreatedSite { removed: c.removed, under, spec });
        }
    }

    /// The created-site options for point of interest `poi`, if it is a created one.
    pub fn created_site(&self, poi: usize) -> Option<CreatedSite> {
        poi.checked_sub(self.base_pois).and_then(|k| self.created.get(k).copied())
    }

    /// Height (ft) at a world position, prefiltered for sample spacing `spacing_ft`.
    pub fn sample(&self, x_ft: f64, y_ft: f64, spacing_ft: f64) -> f64 {
        let mut m = 0;
        while m + 1 < self.mips.len() && self.cell_ft * (1u64 << (m + 1)) as f64 <= spacing_ft {
            m += 1;
        }
        let scale = (1u64 << m) as f64;
        let cell = self.cell_ft * scale;
        // Box-filtered mip samples sit at the center of the texels they average.
        let off = (scale - 1.0) * 0.5 * self.cell_ft;
        self.mips[m].sample_cubic((x_ft - off) / cell, (y_ft - off) / cell)
    }

    /// Water surface (ft) at a world position: the wet corners' level where the bilinear
    /// wet/dry mask is at least one half (a smooth contour, not the cell edges), else
    /// `hydro::DRY`. The land side of that contour gets a bank, so the
    /// shoreline sits on it even where the ground is flat (`shore_bank`).
    pub fn sample_water(&self, x_ft: f64, y_ft: f64) -> f32 {
        let (level, mask) = self.sample_lake(x_ft, y_ft);
        if mask >= 0.5 { level } else { hydro::DRY }
    }



    /// (level over the wet corners or `DRY`, bilinear wet fraction 0..1).
    pub fn sample_lake(&self, x_ft: f64, y_ft: f64) -> (f32, f64) {
        lake_at(&self.water, self.cell_ft, self.biome_seed, x_ft, y_ft)
    }

    /// Smooth ground height (ft) as the terrain builds it without refinement detail (which
    /// settlement pads and road corridors remove): the banked T0 samples of the last coarse
    /// level (lattice spacing `lattice_ft`), interpolated Catmull-Rom as refinement does, and
    /// banked again at the point.
    pub fn ground_at(&self, x_ft: f64, y_ft: f64, lattice_ft: f64) -> f64 {
        self.ground_with(x_ft, y_ft, lattice_ft, true, &mut |i, j| self.ground_node(i, j, lattice_ft))
    }

    /// `ground_at` over the whole map with its lattice nodes computed once (for callers that
    /// ask many thousands of times).
    pub fn ground_sampler(&self, lattice_ft: f64) -> impl Fn(f64, f64) -> f64 + '_ {
        let cached = self.ground_nodes(lattice_ft);
        let (nw, nh) = (cached.1, cached.2);
        let stride = (nw + 1) as usize;
        move |x, y| {
            self.ground_with_nodes(x, y, lattice_ft, true, |i, j| {
                if i >= -1 && j >= -1 && i < nw && j < nh { cached.0[(j + 1) as usize * stride + (i + 1) as usize] } else { self.ground_node(i, j, lattice_ft) }
            })
        }
    }

    /// The lattice nodes `ground_sampler` caches (row-major from node (-1, -1), `nw + 1` a
    /// row), and the node counts (nw, nh) past which it computes them as asked.
    fn ground_nodes(&self, lattice_ft: f64) -> std::sync::Arc<(Vec<f64>, i64, i64)> {
        match self.ground_cache.get() {
            Some((l, nodes)) if *l == lattice_ft => nodes.clone(),
            _ => std::sync::Arc::new(self.ground_nodes_uncached(lattice_ft)),
        }
    }

    fn ground_nodes_uncached(&self, lattice_ft: f64) -> (Vec<f64>, i64, i64) {
        let (nw, nh) = ((self.height.w as f64 * self.cell_ft / lattice_ft) as i64 + 4, (self.height.h as f64 * self.cell_ft / lattice_ft) as i64 + 4);
        let nodes: Vec<f64> = (-1..nh).flat_map(|j| (-1..nw).map(move |i| (i, j))).map(|(i, j)| self.ground_node(i, j, lattice_ft)).collect();
        (nodes, nw, nh)
    }

    /// `ground_sampler` over a `w` × `h` grid whose point (i, j) is at (`pos(i)`, `pos(j)`) ft,
    /// row-major: the same values (the same sums in the same order), with each column's and
    /// row's interpolation weights worked out once instead of per point.
    pub fn ground_grid(&self, lattice_ft: f64, w: usize, h: usize, pos: &dyn Fn(usize) -> f64) -> Vec<f32> {
        let cached = self.ground_nodes(lattice_ft);
        let (nodes, nw, nh) = (&cached.0, cached.1, cached.2);
        let stride = (nw + 1) as usize;
        let node = |i: i64, j: i64| if i >= -1 && j >= -1 && i < nw && j < nh { nodes[(j + 1) as usize * stride + (i + 1) as usize] } else { self.ground_node(i, j, lattice_ft) };
        let axis = |n: usize| -> Vec<(f64, i64, [f64; 4])> {
            (0..n)
                .map(|i| {
                    let x = pos(i);
                    let u = x / lattice_ft;
                    let i0 = crate::core::floor(u);
                    (x, i0 as i64, catmull_rom(u - i0))
                })
                .collect()
        };
        let (cols, rows) = (axis(w), axis(h));
        let mut out = Vec::with_capacity(w * h);
        for &(y, j0, wv) in &rows {
            for &(x, i0, wu) in &cols {
                let mut g = 0.0;
                if i0 >= 0 && j0 >= 0 && i0 + 2 < nw && j0 + 2 < nh {
                    // All 16 nodes cached: straight from the rows (the same sum, in the same order).
                    let base = j0 as usize * stride + i0 as usize;
                    for (b, wy) in wv.iter().enumerate() {
                        let row = &nodes[base + b * stride..base + b * stride + 4];
                        for (a, wx) in wu.iter().enumerate() {
                            g += wx * wy * row[a];
                        }
                    }
                } else {
                    for (b, wy) in wv.iter().enumerate() {
                        for (a, wx) in wu.iter().enumerate() {
                            g += wx * wy * node(i0 + a as i64 - 1, j0 + b as i64 - 1);
                        }
                    }
                }
                let (lw, m) = self.sample_lake(x, y);
                out.push(Self::shore_bank(g, lw, m) as f32);
            }
        }
        out
    }

    /// Banked T0 sample at lattice node (i, j) of `ground_at`'s lattice.
    pub fn ground_node(&self, i: i64, j: i64, lattice_ft: f64) -> f64 {
        let (px, py) = (i as f64 * lattice_ft, j as f64 * lattice_ft);
        let (lw, m) = self.sample_lake(px, py);
        Self::shore_bank(self.sample(px, py, lattice_ft), lw, m)
    }

    /// `ground_at` with the lattice nodes supplied by `node` (so callers can cache them);
    /// `bank` false skips the shore bank at the point (callers that know no water is near).
    pub fn ground_with(&self, x_ft: f64, y_ft: f64, lattice_ft: f64, bank: bool, node: &mut dyn FnMut(i64, i64) -> f64) -> f64 {
        self.ground_with_nodes(x_ft, y_ft, lattice_ft, bank, node)
    }

    /// `ground_with` for a node source known at compile time (no call through a pointer per
    /// node: the callers that ask most).
    pub fn ground_with_nodes(&self, x_ft: f64, y_ft: f64, lattice_ft: f64, bank: bool, mut node: impl FnMut(i64, i64) -> f64) -> f64 {
        let (u, v) = (x_ft / lattice_ft, y_ft / lattice_ft);
        let (i0, j0) = (crate::core::floor(u), crate::core::floor(v));
        let (wu, wv) = (catmull_rom(u - i0), catmull_rom(v - j0));
        let mut h = 0.0;
        for (b, wy) in wv.iter().enumerate() {
            for (a, wx) in wu.iter().enumerate() {
                h += wx * wy * node(i0 as i64 + a as i64 - 1, j0 as i64 + b as i64 - 1);
            }
        }
        if !bank {
            return h;
        }
        let (w, m) = self.sample_lake(x_ft, y_ft);
        Self::shore_bank(h, w, m)
    }

    /// Apply the shore bank to a ground height, given the sampled (level, mask): on the land
    /// side of the contour the ground is lifted to at least just above the water, rising
    /// toward the dry corners; the lift fades in with the mask so it is continuous across
    /// cell edges (a cell with no wet corner has mask 0 and no lift).
    pub fn shore_bank(h: f64, level: f32, mask: f64) -> f64 {
        if level <= hydro::DRY || mask >= 0.5 {
            return h;
        }
        // A floor (so applying it at every level changes nothing twice) that drops away
        // steeply toward mask 0, where a cell edge with no wet corner has no floor at all.
        let floor = level as f64 + 0.5 + SHORE_BANK_FT * (0.5 - mask) * 2.0 - 3_000.0 * (1.0 - smoothstep(0.0, 0.3, mask));
        h.max(floor)
    }

    /// Domain-warp offset (in T0 cells) that makes biome borders organic instead of blocky.
    pub fn biome_warp(&self, x_ft: f64, y_ft: f64) -> (f64, f64) {
        let (cx, cy) = (x_ft / self.cell_ft, y_ft / self.cell_ft);
        (
            1.3 * fbm(self.biome_seed, cx / 2.0, cy / 2.0, 4, 2.2, 0.6),
            1.3 * fbm(self.biome_seed ^ 0x9e37, cx / 2.0 + 3.1, cy / 2.0 + 7.7, 4, 2.2, 0.6),
        )
    }

    /// Biome texel: [primary, secondary, blend, coast distance (sqrt-encoded)]: the nearest
    /// cell after warping by `warp` (from `biome_warp`, possibly interpolated).
    pub fn sample_biome(&self, x_ft: f64, y_ft: f64, warp: (f64, f64)) -> [u8; 4] {
        let (cx, cy) = (x_ft / self.cell_ft, y_ft / self.cell_ft);
        let (wx, wy) = (cx + warp.0, cy + warp.1);
        let i = (crate::core::round(wx).max(0.0) as usize).min(self.biome.w - 1);
        let j = (crate::core::round(wy).max(0.0) as usize).min(self.biome.h - 1);
        let b = self.biome.get(i, j);
        let coast = bilinear(&self.coast, cx, cy);
        let enc = crate::core::sqrt((coast / COAST_ENCODE_FT).clamp(0.0, 1.0)) * 255.0;
        [(b & 0xff) as u8, ((b >> 8) & 0xff) as u8, ((b >> 16) & 0xff) as u8, enc as u8]
    }

    /// Features + river polylines for labels and vector rendering, as JSON.
    /// A quick look at the world for sketching: the terrain stages on a coarse grid `width`
    /// points across, drawn as relief-shaded biomes, water and rivers (RGBA, row-major), with
    /// the sketch's conflicts. Close to the full world in layout, not in detail.
    pub fn preview(world: &World, width: usize) -> Preview {
        let g = &world.geom;
        let w = width.clamp(64, g.t0_w);
        let cell = g.map_w_ft / (w - 1) as f64;
        let h = ((g.map_h_ft / cell).floor() as usize + 1).max(2);
        let s = Self::shape(world, w, h, cell, &mut |_, _| {});
        let sea = world.params().sea_level_ft;
        const PALETTE: [[f64; 3]; 17] = [
            [0.54, 0.63, 0.66],
            [0.62, 0.72, 0.74],
            [0.95, 0.95, 0.93],
            [0.80, 0.79, 0.70],
            [0.74, 0.71, 0.64],
            [0.64, 0.69, 0.57],
            [0.68, 0.74, 0.54],
            [0.58, 0.68, 0.52],
            [0.82, 0.82, 0.63],
            [0.85, 0.81, 0.64],
            [0.84, 0.79, 0.66],
            [0.91, 0.83, 0.63],
            [0.86, 0.81, 0.59],
            [0.52, 0.65, 0.45],
            [0.66, 0.71, 0.60],
            [0.53, 0.48, 0.45],
            [0.93, 0.91, 0.86],
        ];
        let threshold = hydro::RIVER_Q / world.params().river_density;
        let mut river = vec![false; w * h];
        for r in &s.hydro.rivers {
            for (&c, &q) in r.cells.iter().zip(&r.q) {
                if q as f64 >= threshold {
                    river[c as usize] = true;
                }
            }
        }
        let at = |i: usize, j: usize| s.height[j.min(h - 1) * w + i.min(w - 1)].max(sea);
        let mut rgba = Vec::with_capacity(w * h * 4);
        for j in 0..h {
            for i in 0..w {
                let k = j * w + i;
                let col = if !s.land[k] {
                    let depth = ((sea - s.height[k]) / 12_000.0).clamp(0.0, 1.0);
                    let t = crate::core::sqrt(depth);
                    [0.70 + (0.52 - 0.70) * t, 0.78 + (0.62 - 0.78) * t, 0.77 + (0.66 - 0.77) * t]
                } else if s.hydro.water[k] > hydro::DRY && s.hydro.water[k] as f64 > s.height[k] {
                    PALETTE[1]
                } else if river[k] {
                    [0.42, 0.58, 0.70]
                } else {
                    let b = s.biome[k];
                    let (b1, b2, f) = (PALETTE[(b & 0xff).min(16) as usize], PALETTE[((b >> 8) & 0xff).min(16) as usize], 0.5 * ((b >> 16) & 0xff) as f64 / 255.0);
                    let base = [b1[0] + (b2[0] - b1[0]) * f, b1[1] + (b2[1] - b1[1]) * f, b1[2] + (b2[2] - b1[2]) * f];
                    // North-west light on the relief, exaggerated for the coarse grid.
                    let gx = (at(i + 1, j) - at(i.saturating_sub(1), j)) / (2.0 * cell) * 12.0;
                    let gy = (at(i, j + 1) - at(i, j.saturating_sub(1))) / (2.0 * cell) * 12.0;
                    let nl = crate::core::sqrt(gx * gx + gy * gy + 1.0);
                    let lit = (gx + gy + 1.3) / (nl * crate::core::sqrt(3.69));
                    let shade = (0.45 + 0.75 * lit).clamp(0.4, 1.25);
                    [base[0] * shade, base[1] * shade, base[2] * shade]
                };
                rgba.extend_from_slice(&[(col[0].clamp(0.0, 1.0) * 255.0) as u8, (col[1].clamp(0.0, 1.0) * 255.0) as u8, (col[2].clamp(0.0, 1.0) * 255.0) as u8, 255]);
            }
        }
        Preview { w, h, rgba, conflicts: s.conflicts }
    }

    pub fn overlay_json(&self) -> String {
        match &self.extra {
            Some(e) => serde_json::to_string(&e.overlay).expect("serializable"),
            None => "{\"features\":[]}".into(),
        }
    }

    /// Grids needed by tile generation (not the generation-time extras).
    pub fn to_bytes(&self) -> Vec<u8> {
        let (w, h) = (self.height.w, self.height.h);
        let mut out = Vec::with_capacity(28 + w * h * 16);
        out.extend_from_slice(&MAGIC.to_le_bytes());
        out.extend_from_slice(&(w as u32).to_le_bytes());
        out.extend_from_slice(&(h as u32).to_le_bytes());
        out.extend_from_slice(&self.cell_ft.to_le_bytes());
        out.extend_from_slice(&self.biome_seed.to_le_bytes());
        for g in [&self.height, &self.water, &self.coast] {
            for v in &g.data {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in &self.biome.data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        // Rivers: count, then per river: point count, seed, then (x, y) f64 + (z, q, taper) f32.
        out.extend_from_slice(&(self.rivers.rivers.len() as u32).to_le_bytes());
        for r in &self.rivers.rivers {
            out.extend_from_slice(&(r.pts.len() as u32).to_le_bytes());
            out.extend_from_slice(&r.seed.to_le_bytes());
            for k in 0..r.pts.len() {
                out.extend_from_slice(&r.pts[k][0].to_le_bytes());
                out.extend_from_slice(&r.pts[k][1].to_le_bytes());
                out.extend_from_slice(&r.z[k].to_le_bytes());
                out.extend_from_slice(&r.q[k].to_le_bytes());
                out.extend_from_slice(&r.taper[k].to_le_bytes());
            }
        }
        // Roads: count, then per road: class u8, point count u32, seed u64, then (x, y) f64 +
        // (z, wander) f32.
        out.extend_from_slice(&(self.roads.roads.len() as u32).to_le_bytes());
        for r in &self.roads.roads {
            out.push(r.class as u8);
            out.extend_from_slice(&(r.pts.len() as u32).to_le_bytes());
            out.extend_from_slice(&r.seed.to_le_bytes());
            for k in 0..r.pts.len() {
                out.extend_from_slice(&r.pts[k][0].to_le_bytes());
                out.extend_from_slice(&r.pts[k][1].to_le_bytes());
                out.extend_from_slice(&r.z[k].to_le_bytes());
                out.extend_from_slice(&r.wander[k].to_le_bytes());
            }
        }
        // Settlements: count, then per settlement: tier, kind, flags (coastal | river << 1 |
        // capital << 2), culture as u8; cell u32; x, y f64; population u32; seed u64.
        out.extend_from_slice(&(self.settlements.len() as u32).to_le_bytes());
        for s in &self.settlements {
            out.extend_from_slice(&[s.tier as u8, s.kind as u8, s.coastal as u8 | (s.river as u8) << 1 | (s.capital as u8) << 2, s.culture]);
            out.extend_from_slice(&(s.cell as u32).to_le_bytes());
            out.extend_from_slice(&s.x.to_le_bytes());
            out.extend_from_slice(&s.y.to_le_bytes());
            out.extend_from_slice(&s.population.to_le_bytes());
            out.extend_from_slice(&s.seed.to_le_bytes());
        }
        // POIs: count, then per POI: kind u8, x, y f64, seed u64.
        out.extend_from_slice(&(self.pois.len() as u32).to_le_bytes());
        for p in &self.pois {
            out.push(p.kind as u8);
            out.extend_from_slice(&p.x.to_le_bytes());
            out.extend_from_slice(&p.y.to_le_bytes());
            out.extend_from_slice(&p.seed.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<T0, String> {
        let rd = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        if bytes.len() < 28 || rd(0) != MAGIC {
            return Err("not a T0 blob (or from another generator version)".into());
        }
        let (w, h) = (rd(4) as usize, rd(8) as usize);
        let n = w * h;
        if bytes.len() < 28 + n * 16 + 4 {
            return Err("truncated T0 blob".into());
        }
        let cell_ft = f64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let biome_seed = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
        let f32s = |o: usize| -> Vec<f32> { bytes[o..o + n * 4].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect() };
        let base = 28;
        let biome_end = base + 4 * n * 4;
        let biome = bytes[base + 3 * n * 4..biome_end].chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        let mut o = biome_end;
        let mut take = |len: usize| -> Result<&[u8], String> {
            let slice = bytes.get(o..o + len).ok_or("truncated T0 rivers")?;
            o += len;
            Ok(slice)
        };
        let count = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
        let mut curves = Vec::with_capacity(count);
        for _ in 0..count {
            let npts = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
            let seed = u64::from_le_bytes(take(8)?.try_into().unwrap());
            let (mut pts, mut z, mut q, mut taper) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for _ in 0..npts {
                let x = f64::from_le_bytes(take(8)?.try_into().unwrap());
                let y = f64::from_le_bytes(take(8)?.try_into().unwrap());
                pts.push([x, y]);
                z.push(f32::from_le_bytes(take(4)?.try_into().unwrap()));
                q.push(f32::from_le_bytes(take(4)?.try_into().unwrap()));
                taper.push(f32::from_le_bytes(take(4)?.try_into().unwrap()));
            }
            curves.push(RiverCurve::new(pts, z, q, taper, seed));
        }
        let count = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
        let mut road_curves = Vec::with_capacity(count);
        for _ in 0..count {
            let class = roads::RoadClass::from_u8(take(1)?[0]);
            let npts = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
            let seed = u64::from_le_bytes(take(8)?.try_into().unwrap());
            let (mut pts, mut z, mut wander) = (Vec::with_capacity(npts), Vec::with_capacity(npts), Vec::with_capacity(npts));
            for _ in 0..npts {
                let x = f64::from_le_bytes(take(8)?.try_into().unwrap());
                let y = f64::from_le_bytes(take(8)?.try_into().unwrap());
                pts.push([x, y]);
                z.push(f32::from_le_bytes(take(4)?.try_into().unwrap()));
                wander.push(f32::from_le_bytes(take(4)?.try_into().unwrap()));
            }
            road_curves.push(RoadCurve::new(class, pts, z, wander, seed));
        }
        let count = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
        let mut settlements = Vec::with_capacity(count);
        for _ in 0..count {
            let b = take(4)?;
            let (tier, kind, flags, culture) = (b[0], b[1], b[2], b[3]);
            let cell = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
            let x = f64::from_le_bytes(take(8)?.try_into().unwrap());
            let y = f64::from_le_bytes(take(8)?.try_into().unwrap());
            let population = u32::from_le_bytes(take(4)?.try_into().unwrap());
            let seed = u64::from_le_bytes(take(8)?.try_into().unwrap());
            settlements.push(settle::Settlement {
                tier: settle::Tier::from_u8(tier),
                kind: settle::SettleKind::from_u8(kind),
                cell,
                x,
                y,
                population,
                coastal: flags & 1 != 0,
                river: flags & 2 != 0,
                capital: flags & 4 != 0,
                seed,
                culture,
                pin: None,
            });
        }
        let count = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
        let mut pois = Vec::with_capacity(count);
        for _ in 0..count {
            let kind = settle::PoiKind::from_u8(take(1)?[0]);
            let x = f64::from_le_bytes(take(8)?.try_into().unwrap());
            let y = f64::from_le_bytes(take(8)?.try_into().unwrap());
            let seed = u64::from_le_bytes(take(8)?.try_into().unwrap());
            pois.push(settle::Poi { kind, x, y, seed });
        }
        let map_w = (w - 1) as f64 * cell_ft;
        let map_h = (h - 1) as f64 * cell_ft;
        Ok(Self::from_grids(
            Grid::from_vec(w, h, f32s(base)),
            Grid::from_vec(w, h, f32s(base + n * 4)),
            Grid::from_vec(w, h, f32s(base + 2 * n * 4)),
            Grid::from_vec(w, h, biome),
            cell_ft,
            biome_seed,
            RiverNet::new(curves, map_w, map_h, cell_ft),
            RoadNet::new(road_curves, map_w, map_h, cell_ft),
        )
        .with_settlements(settlements, pois))
    }
}

/// (water level or `DRY`, wet fraction 0..1) at a world position, with the shoreline wobble
/// (see `T0::sample_lake`); usable before the T0 exists.
/// Catmull-Rom weights for the four nodes round fraction `t` (`T0::ground_with`).
pub(crate) fn catmull_rom(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [0.5 * (-t3 + 2.0 * t2 - t), 0.5 * (3.0 * t3 - 5.0 * t2 + 2.0), 0.5 * (-3.0 * t3 + 4.0 * t2 + t), 0.5 * (t3 - t2)]
}

pub fn lake_at(g: &Grid<f32>, cell_ft: f64, biome_seed: u64, x_ft: f64, y_ft: f64) -> (f32, f64) {
    let x = (x_ft / cell_ft).clamp(0.0, (g.w - 1) as f64);
    let y = (y_ft / cell_ft).clamp(0.0, (g.h - 1) as f64);
    let (x0, y0) = (crate::core::floor(x) as usize, crate::core::floor(y) as usize);
    let (x1, y1) = ((x0 + 1).min(g.w - 1), (y0 + 1).min(g.h - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let corners = [
        (g.get(x0, y0), (1.0 - fx) * (1.0 - fy)),
        (g.get(x1, y0), fx * (1.0 - fy)),
        (g.get(x0, y1), (1.0 - fx) * fy),
        (g.get(x1, y1), fx * fy),
    ];
    let (mut sum, mut wsum, mut any) = (0.0f64, 0.0f64, 0.0f64);
    for (v, wt) in corners {
        if v > hydro::DRY {
            sum += v as f64 * wt;
            wsum += wt;
            any += 1.0;
        }
    }
    if any == 0.0 {
        return (hydro::DRY, 0.0);
    }
    // Level from the wet corners present (cell-local weights can vanish at a corner).
    let level = if wsum > 1e-12 {
        sum / wsum
    } else {
        corners.iter().filter(|c| c.0 > hydro::DRY).map(|c| c.0 as f64).sum::<f64>() / any
    };
    if any == 4.0 {
        return (level as f32, 1.0);
    }
    // Wobble the wet fraction so contours between two wet and two dry corners are not
    // ruler-straight cell edges. Only the band around one half matters (and the noise is
    // not evaluated elsewhere); the amplitude tapers to zero at its ends.
    let edge = wsum.min(1.0 - wsum);
    if edge <= 0.15 {
        return (level as f32, wsum);
    }
    let (cx, cy) = (x_ft / cell_ft, y_ft / cell_ft);
    let wobble = 0.3 * fbm(biome_seed ^ 0x51a4e, cx * 2.2, cy * 2.2, 3, 2.1, 0.5);
    (level as f32, (wsum + wobble * (1.0 - (2.0 * wsum - 1.0).abs()) * smoothstep(0.15, 0.3, edge)).clamp(0.0, 1.0))
}

/// River curves from the hydrology chains. Water surface along each river is its cells'
/// (filled, strictly decreasing) heights, ending at the sea or lake level at the mouth.
/// Meanders are tapered to zero at sources, mouths and where tributaries join, so lines meet.
fn build_river_net(world: &World, w: usize, cell: f64, height: &[f64], land: &[bool], hydro: &hydro::Hydro) -> RiverNet {
    let sea = world.params().sea_level_ft;
    let chains = &hydro.rivers;
    let mut tapers: Vec<Vec<f32>> = chains
        .iter()
        .map(|r| {
            let n = r.cells.len();
            (0..n).map(|k| (k.min(n - 1 - k) as f32).min(1.0)).collect()
        })
        .collect();
    for r in chains {
        if let Some(p) = r.into {
            let join = *r.cells.last().unwrap();
            if let Some(m) = chains[p].cells.iter().position(|&c| c == join) {
                for (k, t) in tapers[p].iter_mut().enumerate() {
                    let d = (k as i64 - m as i64).unsigned_abs() as f32;
                    *t = t.min(d.min(1.0));
                }
            }
        }
    }
    let curves = chains
        .iter()
        .enumerate()
        .map(|(ri, r)| {
            let pts = r.cells.iter().map(|&c| [(c as usize % w) as f64 * cell, (c as usize / w) as f64 * cell]).collect();
            let z = r
                .cells
                .iter()
                .map(|&c| {
                    let c = c as usize;
                    if !land[c] {
                        sea as f32
                    } else if hydro.lake_of[c] != hydro::NO_LAKE {
                        hydro.lakes[hydro.lake_of[c] as usize].level_ft as f32
                    } else {
                        height[c] as f32
                    }
                })
                .collect();
            // Water cells (the mouth) have no water balance of their own: the river keeps its
            // discharge (and width) into the lake or sea instead of pinching to nothing.
            let mut q = r.q.clone();
            for k in 1..q.len() {
                let c = r.cells[k] as usize;
                if !land[c] || hydro.lake_of[c] != hydro::NO_LAKE {
                    q[k] = q[k].max(q[k - 1]);
                }
            }
            RiverCurve::new(pts, z, q, tapers[ri].clone(), river_seed(world.seed, ri))
        })
        .collect();
    let h = height.len() / w;
    // Bin extents from the grid (not the world file) so loaded copies index identically.
    RiverNet::new(curves, (w - 1) as f64 * cell, (h - 1) as f64 * cell, cell)
}

fn bilinear(g: &Grid<f32>, x: f64, y: f64) -> f64 {
    let x = x.clamp(0.0, (g.w - 1) as f64);
    let y = y.clamp(0.0, (g.h - 1) as f64);
    let (x0, y0) = (crate::core::floor(x) as usize, crate::core::floor(y) as usize);
    let (x1, y1) = ((x0 + 1).min(g.w - 1), (y0 + 1).min(g.h - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let a = g.get(x0, y0) as f64 * (1.0 - fx) + g.get(x1, y0) as f64 * fx;
    let b = g.get(x0, y1) as f64 * (1.0 - fx) + g.get(x1, y1) as f64 * fx;
    a * (1.0 - fy) + b * fy
}

/// Box blur of radius `r` over land cells only.
fn blur_land(w: usize, h: usize, v: &[f64], land: &[bool], r: i64) -> Vec<f64> {
    let mut out = v.to_vec();
    for j in 0..h as i64 {
        for i in 0..w as i64 {
            let k = j as usize * w + i as usize;
            if !land[k] {
                continue;
            }
            let (mut s, mut c) = (0.0, 0.0);
            for dj in -r..=r {
                for di in -r..=r {
                    let (x, y) = (i + di, j + dj);
                    if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                        let kk = y as usize * w + x as usize;
                        s += v[kk];
                        c += 1.0;
                    }
                }
            }
            out[k] = s / c;
        }
    }
    out
}

/// 2x2 box average to half resolution (odd edges average with themselves).
fn downsample(w: usize, h: usize, v: &[f64]) -> Vec<f64> {
    let (ew, eh) = (w.div_ceil(2), h.div_ceil(2));
    let mut out = vec![0.0; ew * eh];
    for j in 0..eh {
        for i in 0..ew {
            let (x0, x1) = ((2 * i).min(w - 1), (2 * i + 1).min(w - 1));
            let (y0, y1) = ((2 * j).min(h - 1), (2 * j + 1).min(h - 1));
            out[j * ew + i] = 0.25 * (v[y0 * w + x0] + v[y0 * w + x1] + v[y1 * w + x0] + v[y1 * w + x1]);
        }
    }
    out
}
