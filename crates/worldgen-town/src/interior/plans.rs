//! Plans of zoned buildings: hub kinds, courts and forecourts, garths, yards, halls and cellars.

use super::*;

/// What a building laid out by zones (`hub`) is: a castle's keep, a barracks of 150 squares or
/// more, a palace, a noble estate big enough, a temple, a monastery, a town hall or a guild big
/// enough (`hub::civic::LEAST`); None for the rest.
pub fn hub_kind(t0: &T0, l: &Layout, b: &town::Building) -> Option<hub::Kind> {
    if b.structure != Structure::Roofed || !zoned_arch(b) {
        return None;
    }
    let key = func_key(b);
    let sh = Shell::of(l, b);
    hub::kind_of(b, key, &sh, capital_of(t0, l.index as usize), city_of(t0, l.index as usize))
}

/// Whether a building may be laid out by zones (`hub::kind_of`): a keep, a palace or an estate, a
/// temple, a town hall or a guild (not the thieves' guild), an inn or a caravanserai.
pub(super) fn zoned_arch(b: &town::Building) -> bool {
    match arch_of(b) {
        Arch::Keep | Arch::Mansion | Arch::Temple | Arch::Inn => true,
        Arch::Hall => matches!(func_key(b), Some("town_hall" | "merchant_guild" | "craft_guild" | "adventurers_guild" | "mages_guild")),
        _ => false,
    }
}

/// Whether layout `settlement` is the capital's.
pub(super) fn capital_of(t0: &T0, settlement: usize) -> bool {
    crate::town::settlement(t0, settlement).is_some_and(|s| s.capital)
}

/// Whether layout `settlement` is a city's or a metropolis's.
pub(super) fn city_of(t0: &T0, settlement: usize) -> bool {
    crate::town::settlement(t0, settlement).is_some_and(|s| s.tier >= crate::t0::settle::Tier::City)
}

/// The floors (z) of a building laid out by zones that the zones didn't hold together on, laid out
/// as other buildings' are instead (every floor of it if it has no plan at all): for the checks,
/// none should be. As generated (no design, no ground shaped by hand).
pub fn hub_fallbacks(world: &World, t0: &T0, settlement: usize, building: usize) -> Vec<i8> {
    let l = town::layout(world, t0, settlement);
    let Some(b) = l.building(building).filter(|b| hub_kind(t0, &l, b).is_some()) else { return Vec::new() };
    let mut fell = Vec::new();
    build_noting(world, t0, &l, settlement, b, &mut fell, &mut Vec::new());
    fell
}

/// The court the capital's palace stands round (`hub::Plan::palace`), open to the sky (world ft, its
/// corners each by the footprint's corner in the same place in `Building::poly`), as the battlemap
/// and the map draw it; None for every other building.
pub fn court_of(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Vec<P>> {
    court_plan(world, t0, l, b).map(|c| c.court)
}

/// The capital's palace's court as the battlemap draws it (`court_of`) and what stands in it, as
/// its ground floor has it (`hub::Plan::court_pieces`).
pub struct Court {
    /// Its outline (world ft), each corner by the footprint's corner in the same place.
    pub court: Vec<P>,
    /// Each piece: its kind (the fountain, a gravel walk, a bench), its box's corners (world ft),
    /// the way its back faces (unit, world), if any.
    pub pieces: Vec<(&'static str, [P; 4], Option<P>)>,
    /// The walled forecourt before its front door, where the ground there allows one (`Forecourt`).
    pub forecourt: Option<Forecourt>,
    /// A monastery's church along its side (world ft): its corners from its west end's (the way in's)
    /// along its length, then back along its side by the walk; roofed higher than the ranges, a
    /// bell-cote on its west end.
    pub church: Option<[P; 4]>,
    /// Cobbled (a coaching inn's yard), not a lawn.
    pub paved: bool,
}

/// The capital's palace's forecourt (`town::palace::Forecourt`): paved ground before its front, a
/// low garden wall round it with a gate on the palace's axis between two piers (world ft).
pub struct Forecourt {
    pub yard: Vec<P>,
    /// The wall's centre lines, `FORE_WALL_FT` thick, along the middle of its squares at the edge:
    /// from the palace's front round to the piers.
    pub walls: Vec<(P, P)>,
    /// The gate's piers' middles, `FORE_PIER_FT` square, their sides along `along` (the front,
    /// unit).
    pub piers: Vec<P>,
    pub along: P,
    /// The gate: its opening's ends (the piers' inner faces, `town::palace::GATE_SQ` squares apart)
    /// and a point inside, where its leaves swing open to.
    pub gate: [P; 3],
}

/// A forecourt's wall's thickness and its gate's piers' side (ft).
pub const FORE_WALL_FT: f64 = 2.0;
pub const FORE_PIER_FT: f64 = 7.0;

/// A manor's outline cut across where its dais end begins (`hub::Plan::cross_wing`): its main
/// range's part and its cross wing's over the family end (world ft), as the map and the battlemap roof
/// them (the cross wing's ridge across the main range's: a cross gable). None for every other
/// building.
pub fn cross_wing_of(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<(Vec<P>, Vec<P>)> {
    // (Cheap tests first: a noble estate big enough for a manor, `hub::kind_of`.)
    if b.structure != Structure::Roofed || !b.parts.is_empty() || b.func.is_some() || b.keep.is_some() || town::catalog::RESIDENTIAL[b.residential as usize] != "noble estate" || geom::area(&b.poly).abs() < 120.0 * SQUARE_FT * SQUARE_FT {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (f, dz) = plan.cross_wing()?;
    // (The line between the rows dz - 1 and dz, the wing beyond it.)
    let (gi, gj) = f.grid(dz as isize, 0);
    let (cx, cy) = (gi as f64 + 0.5 - 0.5 * f.ds.0 as f64, gj as f64 + 0.5 - 0.5 * f.ds.1 as f64);
    let n = add(mul(sh.u, f.ds.0 as f64), mul(sh.v, f.ds.1 as f64));
    let c = dot(sh.world_at(cx, cy), n);
    let main = geom::clip_plain(&b.poly, n, c);
    let wing = geom::clip_plain(&b.poly, mul(n, -1.0), -c);
    (main.len() >= 3 && wing.len() >= 3).then_some((main, wing))
}

/// A city's town hall on its arcade (`hub::civic`): the arcade's open front (world ft) from the outer
/// corner of its first square along the front to its last's (the stair hall's corner left out), then
/// a square in; None for every other building.
pub fn arcade_front(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Vec<P>> {
    // (Cheap tests first: a city's town hall of two storeys or more, big enough.)
    if !town::belfry::is_town_hall(b) || b.structure != Structure::Roofed || b.floors < 2 || !city_of(t0, l.index as usize) || geom::area(&b.poly).abs() < 100.0 * SQUARE_FT * SQUARE_FT {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    if !plan.civic()?.0 {
        return None;
    }
    let f = plan.frame();
    let w = f.wid as isize;
    let corner = |t: isize, ds: f64, dt: f64| {
        let (i, j) = f.grid(0, t);
        sh.world_at(i as f64 + 0.5 + ds * f.ds.0 as f64 + dt * f.dt.0 as f64, j as f64 + 0.5 + ds * f.ds.1 as f64 + dt * f.dt.1 as f64)
    };
    Some(vec![corner(0, -0.5, -0.5), corner(w - 4, -0.5, 0.5), corner(w - 4, 0.5, 0.5), corner(0, 0.5, -0.5)])
}

/// A coaching inn's carriage arch's mouth on the street (`hub::inn`'s `Yard`), as a town hall's arcade
/// front is given (`arcade_front`: the outer corner of its first square along the front to its
/// last's, then a square in; the battlemap draws its piers either side); None for every other building.
pub fn arch_front(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Vec<P>> {
    if !town::palace::is_coaching(l, b) {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (_, (a0, a1)) = plan.yard()?;
    let f = plan.frame();
    let corner = |t: isize, ds: f64, dt: f64| {
        let (i, j) = f.grid(0, t);
        sh.world_at(i as f64 + 0.5 + ds * f.ds.0 as f64 + dt * f.dt.0 as f64, j as f64 + 0.5 + ds * f.ds.1 as f64 + dt * f.dt.1 as f64)
    };
    let (a0, a1) = (a0 as isize, a1 as isize - 1);
    Some(vec![corner(a0, -0.5, -0.5), corner(a1, -0.5, 0.5), corner(a1, 0.5, 0.5), corner(a0, 0.5, -0.5)])
}

/// `court_of` with the court's pieces (`Court`); a monastery's garth (`garth_plan`); a coaching inn's
/// yard (`yard_plan`); None for every

/// other building.
pub fn court_plan(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Court> {
    if town::palace::is_monastery(b) {
        return garth_plan(world, t0, l, b);
    }
    if town::palace::is_coaching(l, b) {
        return yard_plan(world, t0, l, b);
    }
    if !town::palace::is_palace(b) || !capital_of(t0, l.index as usize) || b.poly.len() != 4 || b.structure != Structure::Roofed || !b.parts.is_empty() {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (front, back, side, court) = plan.palace()?;
    if !court {
        return None;
    }
    let f = plan.frame();
    // (A box of the frame's squares (s0..s1, t0..t1) in the world.)
    let corners = |s0: usize, s1: usize, t0: usize, t1: usize| {
        let (a, c) = (f.grid(s0 as isize, t0 as isize), f.grid(s1 as isize - 1, t1 as isize - 1));
        let (x0, x1, y0, y1) = (a.0.min(c.0) as f64, a.0.max(c.0) as f64 + 1.0, a.1.min(c.1) as f64, a.1.max(c.1) as f64 + 1.0);
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|g| sh.world_at(g[0], g[1]))
    };
    let pts = corners(front, back, side, f.wid - side);
    let court = b.poly.iter().map(|q| *pts.iter().min_by(|p, r| geom::dist(**p, *q).total_cmp(&geom::dist(**r, *q))).expect("four corners")).collect();
    let pieces = plan
        .court_pieces()
        .into_iter()
        .map(|(kind, (s0, s1, t0, t1), back)| {
            let dir = back.map(|d| {
                let (gi, gj) = (f.ds.0 * d.0 + f.dt.0 * d.1, f.ds.1 * d.0 + f.dt.1 * d.1);
                add(mul(sh.u, gi as f64), mul(sh.v, gj as f64))
            });
            (kind, corners(s0, s1, t0, t1), dir)
        })
        .collect();
    // The forecourt laid out with the town (`town::palace::approach`): its wall along its outer
    // squares' middles, the piers either side of the gate outside its opening.
    let forecourt = l.forecourts.iter().find(|fc| fc.palace == b.id && fc.yard.len() == 4).map(|fc| {
        let w = geom::dist(fc.front[0], fc.front[1]) / SQUARE_FT;
        let along = mul(sub(fc.front[1], fc.front[0]), 1.0 / (w * SQUARE_FT).max(1e-9));
        let at = |o: f64, t: f64| add(fc.front[0], add(mul(along, t * SQUARE_FT), mul(fc.out, o * SQUARE_FT)));
        let (i, e, p) = (fc.inset, fc.depth - 0.5, 0.5 * FORE_PIER_FT / SQUARE_FT);
        let (g0, g1) = (0.5 * (w - town::palace::GATE_SQ), 0.5 * (w + town::palace::GATE_SQ));
        let walls = vec![(at(0.0, i + 0.5), at(e, i + 0.5)), (at(e, i + 0.5), at(e, g0 - 2.0 * p)), (at(e, g1 + 2.0 * p), at(e, w - i - 0.5)), (at(e, w - i - 0.5), at(0.0, w - i - 0.5))];
        Forecourt { yard: fc.yard.clone(), walls, piers: vec![at(e, g0 - p), at(e, g1 + p)], along, gate: [at(e, g0), at(e, g1), at(e - 1.0, 0.5 * w)] }
    });
    Some(Court { court, pieces, forecourt, church: None, paved: false })
}

/// `court_plan` with the building's carriage arch (`arch_front`) where it is a coaching inn round a
/// yard, both from the one plan; None where `court_plan` is (the arch then from `arch_front`).
pub fn court_and_arch(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<(Court, Option<Vec<P>>)> {
    if !town::palace::is_monastery(b) && town::palace::is_coaching(l, b) {
        return yard_and_arch(world, t0, l, b).map(|(c, a)| (c, Some(a)));
    }
    // (Not a coaching inn: no arch.)
    court_plan(world, t0, l, b).map(|c| (c, None))
}

/// A monastery's cloister garth as the battlemap and the map draw it (`Court`: open to the sky, its
/// corners each by the footprint's corner in the same place; its gravel walks and its cloister well as
/// its ground floor has them, `hub::Plan::garth_pieces`); None for a monastery not round a cloister.
fn garth_plan(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Court> {
    if b.structure != Structure::Roofed || hub_kind(t0, l, b) != Some(hub::Kind::Monastery) {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (_, (s0, s1, g0, g1)) = plan.monastery()?;
    let f = plan.frame();
    let corners = |s0: usize, s1: usize, t0: usize, t1: usize| {
        let (a, c) = (f.grid(s0 as isize, t0 as isize), f.grid(s1 as isize - 1, t1 as isize - 1));
        let (x0, x1, y0, y1) = (a.0.min(c.0) as f64, a.0.max(c.0) as f64 + 1.0, a.1.min(c.1) as f64, a.1.max(c.1) as f64 + 1.0);
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|g| sh.world_at(g[0], g[1]))
    };
    let pts = corners(s0, s1, g0, g1);
    let court = b.poly.iter().map(|q| *pts.iter().min_by(|p, r| geom::dist(**p, *q).total_cmp(&geom::dist(**r, *q))).expect("four corners")).collect();
    let pieces = plan.garth_pieces().into_iter().map(|(kind, (s0, s1, t0, t1))| (kind, corners(s0, s1, t0, t1), None)).collect();
    // (The church: the frame's corner (s, t), grid units, to world ft.)
    let ((cw, ..), _) = plan.monastery()?;
    let (i, j) = f.grid(0, 0);
    let o = [i as f64 + 0.5 - 0.5 * (f.ds.0 + f.dt.0) as f64, j as f64 + 0.5 - 0.5 * (f.ds.1 + f.dt.1) as f64];
    let corner = |s: usize, t: usize| sh.world_at(o[0] + (f.ds.0 * s as isize + f.dt.0 * t as isize) as f64, o[1] + (f.ds.1 * s as isize + f.dt.1 * t as isize) as f64);
    let church = Some([corner(0, 0), corner(f.len, 0), corner(f.len, cw), corner(0, cw)]);
    Some(Court { court, pieces, forecourt: None, church, paved: false })
}

/// A coaching inn's yard as the battlemap and the map draw it (`Court`: cobbled, open to the sky but
/// for the squares the gallery stands over, its corners each by the footprint's corner in the same
/// place; a water trough, barrels and a cart as `hub::Plan::yard_pieces` puts them); None for an inn
/// not round a yard.
fn yard_plan(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Court> {
    yard_and_arch(world, t0, l, b).map(|(c, _)| c)
}

/// `yard_plan` with its carriage arch's mouth as `arch_front` gives it, from the same plan.
fn yard_and_arch(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<(Court, Vec<P>)> {
    if !hub_kind(t0, l, b).is_some_and(|k| k.is_inn()) {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let ((s0, s1, g0, g1), (a0, a1)) = plan.yard()?;
    let f = plan.frame();
    let corners = |s0: usize, s1: usize, t0: usize, t1: usize| {
        let (a, c) = (f.grid(s0 as isize, t0 as isize), f.grid(s1 as isize - 1, t1 as isize - 1));
        let (x0, x1, y0, y1) = (a.0.min(c.0) as f64, a.0.max(c.0) as f64 + 1.0, a.1.min(c.1) as f64, a.1.max(c.1) as f64 + 1.0);
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|g| sh.world_at(g[0], g[1]))
    };
    let pts = corners(s0, s1, g0, g1);
    let court = b.poly.iter().map(|q| *pts.iter().min_by(|p, r| geom::dist(**p, *q).total_cmp(&geom::dist(**r, *q))).expect("four corners")).collect();
    let mut pieces: Vec<(&'static str, [P; 4], Option<P>)> = plan.yard_pieces().into_iter().map(|(kind, (s0, s1, t0, t1))| (kind, corners(s0, s1, t0, t1), None)).collect();
    // (The carriage arch's mouth, "arch": from just inside the front wall to a square out on the
    // street, across its squares.)
    let at = |t: isize, ds: f64, dt: f64| {
        let (i, j) = f.grid(0, t);
        sh.world_at(i as f64 + 0.5 + ds * f.ds.0 as f64 + dt * f.dt.0 as f64, j as f64 + 0.5 + ds * f.ds.1 as f64 + dt * f.dt.1 as f64)
    };
    let (a0, a1) = (a0 as isize, a1 as isize - 1);
    pieces.push(("arch", [at(a0, -1.5, -0.5), at(a1, -1.5, 0.5), at(a1, -0.05, 0.5), at(a0, -0.05, -0.5)], None));
    let arch = vec![at(a0, -0.5, -0.5), at(a1, -0.5, 0.5), at(a1, 0.5, 0.5), at(a0, 0.5, -0.5)];
    Some((Court { court, pieces, forecourt: None, church: None, paved: true }, arch))
}

/// The plan a building laid out by zones is laid out on (`hub::plan`): its frame, its zones (a
/// palace's: `hub::Plan::palace`); as generated (no design, no ground shaped by hand). For the checks.
pub fn zone_plan_of(world: &World, t0: &T0, settlement: usize, building: usize) -> Option<hub::Plan> {
    let l = town::layout(world, t0, settlement);
    let b = l.building(building).filter(|b| b.structure == Structure::Roofed && zoned_arch(b))?;
    let sh = Shell::of(&l, b);
    let t = if b.parts.is_empty() && corner_towers(b) { keep_tower_squares(sh.nx, sh.ny) } else { 0 };
    zone_plan(world, t0, &l, settlement, b, &sh, if arch_of(b) == Arch::Keep { t } else { 0 })
}

/// A building's plan by zones (`hub::plan`), its front toward the nearest street (else the town's
/// middle); `corner`: a keep's grid's corner towers' side.
pub(super) fn zone_plan(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, sh: &Shell, corner: usize) -> Option<hub::Plan> {
    let key = func_key(b);
    let street_d = |p: P| {
        l.roads
            .iter()
            .flat_map(|(pts, _, w)| pts.windows(2).map(move |s| geom::seg_dist(p, s[0], s[1]) - 0.5 * w))
            .fold(f64::MAX, f64::min)
    };
    // (A keep built against its castle's curtain or set into it fronts on the bailey, not the
    // curtain: no side within 8 ft of the bailey's edge.)
    let bailey: Option<&Vec<P>> = b.keep.and_then(|_| l.plazas.iter().find(|q| q.kind == town::PlazaKind::Bailey && geom::contains(&q.pts, geom::centroid(&b.poly))).map(|q| &q.pts));
    let by_curtain = |w: P| bailey.is_some_and(|q| (0..q.len()).any(|k| geom::seg_dist(w, q[k], q[(k + 1) % q.len()]) < 8.0));
    // (Roadmap 6 R4: a row's house fronts its `faces` chord, toward the row's road.)
    let faces = town::sites::faces(world, t0, l);
    let near = |p: P| {
        let w = sh.world_at(p[0], p[1]);
        let d = match faces {
            Some([a, e]) => geom::seg_dist(w, a, e),
            None => street_d(w),
        };
        let d = if d < 80.0 || faces.is_some() { d } else { 1e4 + geom::dist(w, l.center) };
        if by_curtain(w) { d + 1e6 } else { d }
    };
    let ext_free = |i: isize, j: isize| sh.ext_free(i, j);
    let seed = hash3(world.stream("interior.hub"), town::rng_key(t0, settlement), b.id as i64, 0x4b);
    // (The capital's palace's front as the town laid it out with its forecourt: its middle, grid
    // units.)
    let front = l.forecourts.iter().find(|f| f.palace == b.id).map(|f| {
        let d = sub(mul(add(f.front[0], f.front[1]), 0.5), sh.origin);
        [dot(d, sh.u) / SQUARE_FT, dot(d, sh.v) / SQUARE_FT]
    });
    hub::plan(b, sh, key, corner, &town::keep_tops(b, world.stream("keep.tops")), seed, &near, &ext_free, (capital_of(t0, settlement), city_of(t0, settlement)), front)
}

/// A great hall furnished as a whole (`hall`): its level (index into `Interior::levels`), its room,
/// the frame it was laid out in, how.
#[derive(Clone, Copy, Debug)]
pub struct HallPlan {
    pub level: usize,
    pub room: i16,
    pub frame: hall::Frame,
    pub style: hall::Style,
}

/// The great halls of a building furnished as wholes, as generated (no design, no ground shaped by
/// hand): for the checks.
pub fn hall_plans(world: &World, t0: &T0, settlement: usize, building: usize) -> Vec<HallPlan> {
    let l = town::layout(world, t0, settlement);
    let Some(b) = l.building(building).filter(|b| b.structure == Structure::Roofed) else { return Vec::new() };
    let mut out = Vec::new();
    build_noting(world, t0, &l, settlement, b, &mut Vec::new(), &mut out);
    out
}

/// A keep big enough for deep dungeons below its cellar (`under::keep_dungeon`).
pub fn has_deep_dungeon(b: &town::Building) -> bool {
    b.structure == Structure::Roofed && arch_of(b) == Arch::Keep && geom::area(&b.poly).abs() >= 150.0 * SQUARE_FT * SQUARE_FT
}

/// Whether a roofed building has a cellar (all do but a yurt, an islet's watchtower and one on a
/// deck over the water).
pub fn has_cellar(b: &town::Building) -> bool {
    // (An islet's watchtower stands on rock in the water; R5 M20: a shop on a deck over it.)
    !(arch_of(b) == Arch::House && b.parts.is_empty() && round_ish(b)) && !b.func.is_some_and(|f| matches!(town::catalog::CATALOG[f as usize].key, "watchtower" | "lighthouse")) && b.deck.is_none()
}

/// The ways from a cellar to other sites: a trapdoor down into the sewers (on the street side),
/// stairs down to a keep's deep dungeons (in the middle of the far end). Which ones it should
/// have (their names), put on the free floor of `lvl` (the deepest level below ground; fewer if
/// there is none; none without a level).
pub(super) fn cellar_links(mut lvl: Option<&mut Level>, world: &World, t0: &T0, l: &Layout, b: &town::Building, sh: &Shell, stairs: [usize; 4], designed: bool) -> Vec<&'static str> {
    let (nx, ny) = (sh.nx, sh.ny);
    let grid = |p: P| {
        let d = sub(p, sh.origin);
        [dot(d, sh.u) / SQUARE_FT, dot(d, sh.v) / SQUARE_FT]
    };
    let mut want = Vec::new();
    // (R3: a building drawn by hand part of a city: on its sewers, `under::member_link_of`.)
    let sewer = if l.site { crate::under::member_link_of(world, t0, l, b) } else { crate::under::sewer_link_of(t0, l, b).map(|q| (q, l.index as usize)) };
    if let Some((q, city)) = sewer {
        want.push("the trapdoor to the sewers");
        if let Some(lvl) = lvl.as_deref_mut() {
            place_link(lvl, nx, ny, stairs, grid(q), "trapdoor", "trapdoor to the sewers", crate::under::sewer_id(city, q));
        }
    }
    if has_deep_dungeon(b) {
        want.push("the stairs down to the deep dungeons");
        if let Some(lvl) = lvl.as_deref_mut() {
            place_link(lvl, nx, ny, stairs, [nx as f64 * 0.5, ny as f64 * 0.5], "link_down", "stairs down to the deep dungeons", format!("k:{}:{}", l.index, b.id));
        }
    }
    // (Entrances made in this building: stairs or a trapdoor down to each one's site.)
    for (el, c) in crate::sitelinks::entrances_in(world, t0, l.index as usize, b.id as usize) {
        let (item, name, wanted) = crate::sitelinks::way_down(c.under_kind().unwrap_or(crate::under::UnderKind::Dungeon), crate::sitelinks::trapdoor(c));
        want.push(wanted);
        if let Some(lvl) = lvl.as_deref_mut() {
            crate::sitelinks::place_way(lvl, nx, ny, stairs, grid([c.x, c.y]), item, name, format!("u:{el}:0"), designed);
        }
    }
    want
}

/// Put a link item on the free floor square of `lvl` nearest grid point `at` (off the stairs
/// and doorways); false if there is none.
fn place_link(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], at: [f64; 2], kind: &'static str, name: &'static str, to: String) -> bool {
    let mut taken = vec![false; nx * ny];
    for f in &lvl.furniture {
        for j in f.y..f.y + f.h {
            for i in f.x..f.x + f.w {
                taken[j as usize * nx + i as usize] = true;
            }
        }
    }
    for j in st[1]..st[1] + st[3] {
        for i in st[0]..st[0] + st[2] {
            taken[j * nx + i] = true;
        }
    }
    for d in &lvl.doors {
        for (x, y) in [(d.a[0].min(d.b[0]), d.a[1].min(d.b[1])), (d.a[0].min(d.b[0]) - if d.a[0] == d.b[0] { 1.0 } else { 0.0 }, d.a[1].min(d.b[1]) - if d.a[1] == d.b[1] { 1.0 } else { 0.0 })] {
            if x >= 0.0 && y >= 0.0 && (x as usize) < nx && (y as usize) < ny {
                taken[y as usize * nx + x as usize] = true;
            }
        }
    }
    let best = (0..nx * ny).filter(|&k| lvl.cells[k] >= 0 && !taken[k]).min_by(|&a, &b| {
        let d = |k: usize| ((k % nx) as f64 + 0.5 - at[0]).powi(2) + ((k / nx) as f64 + 0.5 - at[1]).powi(2);
        d(a).total_cmp(&d(b))
    });
    let Some(k) = best else { return false };
    let (x, y) = (k % nx, k / nx);
    lvl.furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: 1, h: 1, cover: 0, blocks_move: false, height_ft: 0.0, hazard: None, sprite: 0, back: None, variant: None });
    lvl.links.push(Link { x: x as u16, y: y as u16, to, id: String::new(), far: None });
    true
}
