//! Layouts for points of interest: ruins (broken shells around a courtyard, sometimes a
//! fallen hall, and a stair down to a dungeon or crypt), wizard's towers (a round tower with
//! an outbuilding), roadside inns (inn, stables and a yard beside the road), and the mouths
//! of caves, mines and lava tubes. Same `Layout` as settlements, so battlemaps, site tiles
//! and the gazetteer treat them alike.

use super::catalog::{self, Ward};
use super::geom::*;
use super::{Building, District, Ground, Layout, Structure, castle, clip_walls, rect, wall_pieces};
use crate::World;
use crate::core::rng::{Pcg32, hash2};
use crate::t0::T0;
use crate::t0::settle::{PoiKind, Tier};
use crate::under::UnderKind;

/// How far a POI layout reaches from its point (ft).
pub const SITE_REACH_FT: f64 = 260.0;

pub fn generate(world: &World, t0: &T0, poi: usize, index: u32) -> Layout {
    let p = &t0.pois[poi];
    let center = [p.x, p.y];
    let mut rng = Pcg32::new(hash2(world.stream("site"), poi as i64, p.seed as i64), 41);
    let ground = |q: P| t0.sample(center[0] + q[0], center[1] + q[1], 20.0);
    let dry = |poly: &[P]| {
        poly.iter().all(|q| {
            let w = [center[0] + q[0], center[1] + q[1]];
            (t0.sample_water(w[0], w[1]) as f64) < ground(*q)
        })
    };
    let pad = |poly: &[P]| {
        let n = poly.len() as f64;
        (poly.iter().map(|q| ground(*q)).sum::<f64>() / n) as f32
    };
    let mut buildings: Vec<Building> = Vec::new();
    let mut plazas: Vec<Vec<P>> = Vec::new();
    let mut entrances: Vec<super::Entrance> = Vec::new();
    let mut yards: Vec<(Vec<P>, f32)> = Vec::new();
    let mut props: Vec<super::Prop> = Vec::new();
    // A castle's or wall's works (local): built on a layout of their own, then taken from it.
    let mut works = scratch(index);
    let created = poi.checked_sub(t0.base_pois).and_then(|k| world.file.edits.created.get(k));
    // Uphill (unit, local): passages run into the hill.
    let uphill = |q: P| {
        let e = 60.0;
        let g = [ground(add(q, [e, 0.0])) - ground(add(q, [-e, 0.0])), ground(add(q, [0.0, e])) - ground(add(q, [0.0, -e]))];
        let l = len(g);
        if l < 1e-6 { None } else { Some(mul(g, 1.0 / l)) }
    };
    let any_dir = |rng: &mut Pcg32| {
        let a = rng.range(0.0, std::f64::consts::TAU);
        [libm::cos(a), libm::sin(a)]
    };
    let push = |poly: Vec<P>, structure: Structure, func: Option<&str>, residential: usize, floors: u8, buildings: &mut Vec<Building>| {
        if poly.len() < 3 || !dry(&poly) || buildings.iter().any(|b| b.poly.iter().any(|q| contains(&poly, *q)) || poly.iter().any(|q| contains(&b.poly, *q))) {
            return;
        }
        let pad_ft = pad(&poly);
        buildings.push(Building {
            poly,
            ward: Ward::Rural,
            func: func.and_then(catalog::index_of).map(|i| i as u16),
            residential: residential as u8,
            name: None,
            floors,
            pad_ft,
            structure,
            roof: None,
            tint: None,
            id: 0,
        });
    };
    match p.kind {
        PoiKind::Ruin => {
            // A courtyard ringed by broken shells; sometimes a great hall in the middle.
            let yard = rng.range(35.0, 70.0);
            let n = 3 + rng.below(5) as usize;
            let a0 = rng.range(0.0, std::f64::consts::TAU);
            let mut dir = [libm::cos(a0), libm::sin(a0)];
            for k in 0..n {
                let a = a0 + std::f64::consts::TAU * (k as f64 + rng.range(-0.2, 0.2)) / n as f64;
                let (w, d) = (rng.range(24.0, 48.0), rng.range(18.0, 30.0));
                let c = [(yard + 0.5 * d) * libm::cos(a), (yard + 0.5 * d) * libm::sin(a)];
                push(rect(c, [-libm::sin(a), libm::cos(a)], w, d), Structure::Ruin, None, catalog::RUINED, 1, &mut buildings);
            }
            if rng.next_f64() < 0.5 {
                let a = rng.range(0.0, std::f64::consts::PI);
                dir = [libm::cos(a), libm::sin(a)];
                push(rect([0.0, 0.0], dir, yard * 1.1, yard * 0.7), Structure::Ruin, None, catalog::RUINED, 1, &mut buildings);
            } else {
                plazas.push(circle([0.0, 0.0], yard * 0.6, 10));
            }
            // A stair down in the middle, square to the old hall (or facing the first shell): a
            // dungeon under the hold, or its crypt.
            let mut kind = if rng.next_f64() < 0.5 { UnderKind::Dungeon } else { UnderKind::Crypt };
            if let Some(under) = t0.created_site(poi).and_then(|c| c.under) {
                kind = under;
            }
            entrances.push(super::Entrance { at: mul(dir, -7.5), dir, kind, id: 0 });
        }
        PoiKind::Cave => {
            let dir = uphill([0.0, 0.0]).unwrap_or_else(|| any_dir(&mut rng));
            entrances.push(super::Entrance { at: [0.0, 0.0], dir, kind: UnderKind::Cave, id: 0 });
        }
        PoiKind::Mine => {
            // The adit runs into the hill; the miners' shed and the yard stand below it.
            let dir = uphill([0.0, 0.0]).unwrap_or_else(|| any_dir(&mut rng));
            let side = [-dir[1], dir[0]];
            entrances.push(super::Entrance { at: [0.0, 0.0], dir, kind: UnderKind::Mine, id: 0 });
            push(rect(add(mul(dir, -40.0), mul(side, 28.0)), dir, 26.0, 18.0), Structure::Roofed, None, 1, 1, &mut buildings);
            plazas.push(rect(mul(dir, -30.0), dir, 40.0, 30.0));
        }
        PoiKind::LavaTube => {
            // Tubes run down the flank, the way the lava flowed.
            let dir = uphill([0.0, 0.0]).map(|u| mul(u, -1.0)).unwrap_or_else(|| any_dir(&mut rng));
            entrances.push(super::Entrance { at: [0.0, 0.0], dir, kind: UnderKind::LavaTube, id: 0 });
        }
        PoiKind::Entrance => {
            // Just the way down: into the hill for caves and mines, down the flank for lava
            // tubes; stairs square to the battlemap's grid, on a square's middle.
            let kind = t0.created_site(poi).and_then(|c| c.under).unwrap_or(UnderKind::Dungeon);
            let (at, dir) = match kind {
                UnderKind::Cave | UnderKind::Mine => ([0.0, 0.0], uphill([0.0, 0.0]).unwrap_or_else(|| any_dir(&mut rng))),
                UnderKind::LavaTube => ([0.0, 0.0], uphill([0.0, 0.0]).map(|u| mul(u, -1.0)).unwrap_or_else(|| any_dir(&mut rng))),
                _ => {
                    let sq = crate::battlemap::SQUARE_FT;
                    let snap = |v: f64| crate::core::floor(v / sq) * sq + 0.5 * sq - v;
                    ([snap(center[0]), snap(center[1])], [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]][rng.below(4) as usize])
                }
            };
            entrances.push(super::Entrance { at, dir, kind, id: 0 });
        }
        PoiKind::Tower => {
            let r = rng.range(16.0, 22.0);
            push(circle([0.0, 0.0], r, 14), Structure::Roofed, None, catalog::WIZARD_TOWER, 4 + rng.below(3) as u8, &mut buildings);
            let a = rng.range(0.0, std::f64::consts::TAU);
            let c = [(r + 24.0) * libm::cos(a), (r + 24.0) * libm::sin(a)];
            push(rect(c, [-libm::sin(a), libm::cos(a)], 26.0, 18.0), Structure::Roofed, None, 1, 1, &mut buildings);
        }
        PoiKind::Camp => {
            // A clearing trodden bare round a stone-ringed fire; tents round it with their
            // doors to the fire (the leader's pavilion a little farther out); bedrolls and the
            // firewood by the fire in the gaps between tents, a cart with its stores at the edge.
            use crate::battlemap::Kind;
            use std::f64::consts::{PI, TAU};
            let r = rng.range(32.0, 42.0);
            let a0 = rng.range(0.0, TAU);
            let clearing: Vec<P> = (0..16)
                .map(|k| {
                    let a = a0 + TAU * k as f64 / 16.0;
                    let rr = r * rng.range(0.9, 1.1);
                    [rr * libm::cos(a), rr * libm::sin(a)]
                })
                .collect();
            let level = (pad(&clearing) + ground([0.0, 0.0]) as f32) * 0.5;
            yards.push((clearing, level));
            let polar = |d: f64, a: f64| [d * libm::cos(a), d * libm::sin(a)];
            let mut prop = |at: P, rot: f64, kind: Kind, variant: u8, scale: f64| props.push(super::Prop { at, rot: rot as f32, kind, variant, scale: scale as f32 });
            prop([0.0, 0.0], rng.range(0.0, TAU), Kind::Campfire, rng.below(4) as u8, 1.0);
            let tents = 4 + rng.below(4) as usize;
            let lead = rng.below(tents as u32) as usize;
            let ring = r * 0.62;
            for t in 0..tents {
                let a = a0 + TAU * (t as f64 + rng.range(-0.12, 0.12)) / tents as f64;
                // (A tent is drawn with its door along +x: turned to face the fire.)
                if t == lead {
                    prop(polar(ring + 4.0, a), a + PI, Kind::Tent, 3, 1.45);
                } else {
                    prop(polar(ring + rng.range(-2.5, 2.5), a), a + PI, Kind::Tent, rng.below(3) as u8, rng.range(0.9, 1.05));
                }
            }
            // The gaps between tents: the cart in one, the firewood in the next, bedrolls
            // (feet to the fire) in the rest, up to three.
            let gap = |g: usize| a0 + TAU * (g as f64 + 0.5) / tents as f64;
            let g0 = rng.below(tents as u32) as usize;
            let ca = gap(g0);
            let tangent = ca + 0.5 * PI;
            prop(polar(r - 5.0, ca), tangent, Kind::Cart, rng.below(4) as u8, 1.0);
            for (k, kind) in [Kind::Barrel, Kind::Crate].into_iter().enumerate() {
                let side = if k == 0 { 1.0 } else { -1.0 };
                prop(add(polar(r - 7.0, ca), polar(side * 10.0, tangent)), rng.range(0.0, TAU), kind, rng.below(4) as u8, 1.0);
            }
            prop(polar(14.0, gap(g0 + 1)), gap(g0 + 1) + 0.5 * PI, Kind::Firewood, rng.below(4) as u8, 1.0);
            for g in (g0 + 2..g0 + tents).take(3) {
                let a = gap(g) + rng.range(-0.1, 0.1);
                prop(polar(rng.range(9.0, 11.0), a), a, Kind::Bedroll, rng.below(4) as u8, 1.0);
            }
        }
        PoiKind::Castle => {
            if let Some(c) = created {
                castle_site(world, t0, c, center, &mut rng, &mut works);
                buildings = std::mem::take(&mut works.buildings);
                plazas.append(&mut works.plazas);
            }
        }
        PoiKind::Wall => {
            if let Some(c) = created {
                wall_site(world, t0, c, center, &mut works);
            }
        }
        PoiKind::Building => {
            // Drawn by hand: one building, as the world file has it.
            if let Some(c) = created {
                let poly: Vec<P> = c.poly.iter().map(|q| sub(*q, center)).collect();
                if poly.len() >= 3 {
                    let pad_ft = pad(&poly);
                    buildings.push(drawn(c, poly, pad_ft, &mut rng, t0, center));
                }
            }
        }
        PoiKind::Waystation => {
            // Beside the road: find its direction here and set the buildings back from it.
            let mut dir = [1.0, 0.0];
            let mut best = f64::MAX;
            for (ri, k) in t0.roads.segments_near([p.x - 400.0, p.y - 400.0, p.x + 400.0, p.y + 400.0], 0.0) {
                let rc = &t0.roads.roads[ri as usize];
                for j in 0..16 {
                    let a = rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p;
                    let b = rc.eval(k as usize, (j + 1) as f64 / 16.0, 5.0, t0.cell_ft).p;
                    let d = seg_dist(center, a, b);
                    if d < best {
                        best = d;
                        dir = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
                    }
                }
            }
            let side = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
            let nrm = mul([-dir[1], dir[0]], side);
            // The site stands on the road; buildings face it from one side.
            let at = |along: f64, out: f64| add(mul(dir, along), mul(nrm, out));
            push(rect(at(0.0, 40.0), dir, 56.0, 34.0), Structure::Roofed, Some("inn"), 0, 2, &mut buildings);
            push(rect(at(48.0, 44.0), dir, 34.0, 26.0), Structure::Roofed, Some("stables"), 0, 1, &mut buildings);
            plazas.push(rect(at(20.0, 88.0), dir, 70.0, 40.0));
        }
    }
    let super::Layout { mut walls, mut towers, mut gate_towers, mut castles, .. } = works;
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let reach = |q: &P, r: f64| [[q[0] - r, q[1] - r], [q[0] + r, q[1] + r]];
    let tower_reach = towers.iter().flat_map(|t| reach(t, 11.0)).chain(gate_towers.iter().flat_map(|t| reach(t, 15.0)));
    for q in buildings.iter().flat_map(|b| b.poly.iter()).chain(plazas.iter().flatten()).chain(yards.iter().flat_map(|y| y.0.iter())).chain(walls.iter().flatten()).copied().chain(tower_reach) {
        let q = &q;
        let w = add(*q, center);
        bb = [bb[0].min(w[0]), bb[1].min(w[1]), bb[2].max(w[0]), bb[3].max(w[1])];
    }
    for b in &mut buildings {
        b.poly.iter_mut().for_each(|q| *q = add(*q, center));
    }
    for poly in &mut plazas {
        poly.iter_mut().for_each(|q| *q = add(*q, center));
    }
    for e in &mut entrances {
        e.at = add(e.at, center);
    }
    for (poly, _) in &mut yards {
        poly.iter_mut().for_each(|q| *q = add(*q, center));
    }
    for p in &mut props {
        p.at = add(p.at, center);
    }
    walls.iter_mut().flatten().chain(&mut towers).chain(&mut gate_towers).for_each(|q| *q = add(*q, center));
    for c in &mut castles {
        c.0 = add(c.0, center);
    }
    // A castle is picked anywhere in its walls; a wall on its line (`gazetteer::query`).
    let radius = match p.kind {
        PoiKind::Castle => created.map_or(80.0, |c| c.poly.iter().map(|q| dist(*q, center)).fold(0.0, f64::max)),
        PoiKind::Wall => 30.0,
        _ => 80.0,
    };
    let mut l = Layout {
        index,
        center,
        radius,
        tier: Tier::Village,
        on_water: false,
        walls,
        towers,
        gates: Vec::new(),
        streets: Vec::new(),
        roads: Vec::new(),
        plazas,
        fields: Vec::new(),
        districts: Vec::new(),
        blocks: Vec::new(),
        buildings,
        bridges: Vec::new(),
        piers: Vec::new(),
        gate_towers,
        extra_towers: Vec::new(),
        monuments: Vec::new(),
        quarters: Vec::new(),
        castles,
        bbox: if bb[0] <= bb[2] { bb } else { [center[0] - 40.0, center[1] - 40.0, center[0] + 40.0, center[1] + 40.0] },
        site: true,
        entrances,
        yards,
        props,
    };
    l.number();
    l
}

/// A building drawn by hand (footprint local to its point): its function (a business, else a
/// home; a house by default), storeys, structure, roof and tint as chosen; named as created,
/// else as its trade would be.
fn drawn(c: &crate::world::Created, poly: Vec<P>, pad_ft: f32, rng: &mut Pcg32, t0: &T0, center: P) -> Building {
    let ruin = c.structure.as_deref() == Some("ruin");
    let (func, residential) = func_of(c.func.as_deref().unwrap_or(""), ruin);
    let name = Some(c.name.trim().to_string()).filter(|n| !n.is_empty()).or_else(|| trade_name(func, rng, t0, center));
    let tower = func.is_none() && residential as usize == catalog::WIZARD_TOWER;
    let mut b = Building {
        poly,
        ward: Ward::Rural,
        func,
        residential,
        name,
        floors: c.floors.unwrap_or(if tower { 4 } else { 1 }).max(1),
        pad_ft,
        structure: Structure::Roofed,
        roof: None,
        tint: None,
        id: 0,
    };
    apply_looks(&mut b, c.roof.as_deref(), c.tint.as_deref(), c.structure.as_deref());
    b
}

/// A building's function from a catalog key or a home (`BUILDING_HOMES`); else a house, or a
/// ruined building for a ruin. (`func`, `residential`)
pub fn func_of(key: &str, ruin: bool) -> (Option<u16>, u8) {
    use crate::world::BUILDING_HOMES;
    let func = catalog::index_of(key);
    let home = BUILDING_HOMES.iter().position(|h| *h == key);
    let residential = if ruin && func.is_none() && home.is_none() { catalog::RUINED } else { home.unwrap_or(1) };
    debug_assert!(residential < catalog::RESIDENTIAL.len());
    (func.map(|i| i as u16), residential as u8)
}

/// The name a business would have here (in the nearest settlement's culture); none for a home.
pub fn trade_name(func: Option<u16>, rng: &mut Pcg32, t0: &T0, center: P) -> Option<String> {
    let f = &catalog::CATALOG[func? as usize];
    let culture = t0.settlements.iter().min_by(|a, b| (a.x - center[0]).hypot(a.y - center[1]).total_cmp(&(b.x - center[0]).hypot(b.y - center[1]))).map_or(0, |s| s.culture as usize);
    let mut namer = crate::t0::names::Namer::new(rng.next_u32() as u64);
    super::business_name(f, rng, &mut namer, culture)
}

/// A roof (`ROOFS`), tint (`TINTS`) and structure (`STRUCTURES`) chosen by hand; each not
/// given stays as it is.
pub fn apply_looks(b: &mut Building, roof: Option<&str>, tint: Option<&str>, structure: Option<&str>) {
    use super::RoofStyle;
    use crate::world::TINTS;
    match roof {
        Some("hip") => b.roof = Some(RoofStyle::Hip),
        Some("battlements") => b.roof = Some(RoofStyle::Battlements),
        Some("cone") => b.roof = Some(RoofStyle::Cone),
        _ => {}
    }
    if let Some(t) = tint.and_then(|t| TINTS.iter().position(|x| *x == t)) {
        b.tint = Some(t as u8);
    }
    match structure {
        Some("ruin") => b.structure = Structure::Ruin,
        Some("roofed") => b.structure = Structure::Roofed,
        _ => {}
    }
}

/// An empty layout to build a castle's or wall's works on (local coordinates).
fn scratch(index: u32) -> Layout {
    Layout {
        index,
        center: [0.0, 0.0],
        radius: 80.0,
        tier: Tier::Village,
        on_water: false,
        walls: Vec::new(),
        towers: Vec::new(),
        gates: Vec::new(),
        streets: Vec::new(),
        roads: Vec::new(),
        plazas: Vec::new(),
        fields: Vec::new(),
        districts: Vec::new(),
        blocks: Vec::new(),
        buildings: Vec::new(),
        bridges: Vec::new(),
        piers: Vec::new(),
        gate_towers: Vec::new(),
        extra_towers: Vec::new(),
        monuments: Vec::new(),
        quarters: Vec::new(),
        castles: Vec::new(),
        bbox: [0.0; 4],
        site: true,
        entrances: Vec::new(),
        yards: Vec::new(),
        props: Vec::new(),
    }
}

/// The open ground round a site, as its buildings stand on it (local coordinates): dry
/// everywhere along a footprint's outline, built at the mean ground under it.
struct Open<'a> {
    t0: &'a T0,
    center: P,
}

impl Open<'_> {
    fn ground(&self, q: P) -> f64 {
        self.t0.sample(self.center[0] + q[0], self.center[1] + q[1], 20.0)
    }
    /// Standing water or a river channel at `q` (local).
    fn wet(&self, q: P) -> bool {
        let w = add(q, self.center);
        if (self.t0.sample_water(w[0], w[1]) as f64) >= self.ground(q) {
            return true;
        }
        let c = crate::lod::rivers::clear_of_rivers(&self.t0.rivers, w[0], w[1], 2.0, self.t0.cell_ft);
        (c.0 - w[0]).abs() + (c.1 - w[1]).abs() > 0.01
    }
}

impl Ground for Open<'_> {
    fn buildable(&self, poly: &[P]) -> bool {
        let m = poly.len();
        (0..m).all(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            let n = (dist(a, b) / 10.0).ceil().max(1.0) as usize;
            (0..n).all(|j| !self.wet(lerp(a, b, j as f64 / n as f64)))
        }) && !self.wet(centroid(poly))
    }
    fn pad(&self, poly: &[P]) -> f32 {
        (poly.iter().map(|q| self.ground(*q)).sum::<f64>() / poly.len() as f64) as f32
    }
}

/// Where a castle's gate faces when none was chosen (local): the nearest road within half a
/// mile, else the nearest settlement.
fn gate_toward(t0: &T0, center: P) -> P {
    let r = 2640.0;
    let mut best: Option<(f64, P)> = None;
    for (ri, k) in t0.roads.segments_near([center[0] - r, center[1] - r, center[0] + r, center[1] + r], 0.0) {
        let rc = &t0.roads.roads[ri as usize];
        for j in 0..=16 {
            let q = rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p;
            let d = dist(q, center);
            if d <= r && best.is_none_or(|b| d < b.0) {
                best = Some((d, q));
            }
        }
    }
    if let Some((_, q)) = best {
        return sub(q, center);
    }
    let town = t0.settlements.iter().min_by(|a, b| dist([a.x, a.y], center).total_cmp(&dist([b.x, b.y], center)));
    town.map_or([0.0, -1000.0], |s| sub([s.x, s.y], center))
}

/// A castle drawn by hand (`Created` kind `castle`): its curtain on the outline drawn, the gate
/// on the side chosen (else facing the nearest road), the keep (the castle, named as the site)
/// and the buildings round the yard (the biggest the barracks, then the stables and the smithy);
/// in ruins, broken walls and roofless shells.
fn castle_site(world: &World, t0: &T0, c: &crate::world::Created, center: P, rng: &mut Pcg32, l: &mut Layout) {
    let mut block: Vec<P> = c.poly.iter().map(|q| sub(*q, center)).collect();
    if block.len() < 3 {
        return;
    }
    let m = block.len();
    let toward = match c.gate.map(|g| g as usize).filter(|&g| g < m) {
        Some(g) => lerp(block[g], block[(g + 1) % m], 0.5),
        None => gate_toward(t0, center),
    };
    if area(&block) < 0.0 {
        block.reverse();
    }
    let ground = Open { t0, center };
    let dp = District::roll(Ward::Castle, hash2(world.stream("castle"), rng.next_u32() as i64, 0));
    let keep = c.keep.unwrap_or(true);
    castle(&ground, &block, toward, Ward::Castle, &dp, rng, l, keep, c.yard_buildings.unwrap_or(true));
    // The bailey inside the curtain: open ground (no trees or boulders), built on only by hand.
    let bailey = inset(&block, &vec![10.0; block.len()]);
    if bailey.len() >= 3 {
        l.plazas.push(bailey);
    }
    // The keep (pushed last) is the castle; the yard's buildings by size.
    let n = l.buildings.len();
    let kept = keep && l.buildings.last().is_some_and(|b| b.floors == 4 && b.residential == 4 && b.func.is_none());
    if kept && let Some(fi) = catalog::index_of("castle") {
        let b = &mut l.buildings[n - 1];
        b.func = Some(fi as u16);
        b.name = Some(c.name.trim().to_string()).filter(|s| !s.is_empty()).or_else(|| trade_name(b.func, rng, t0, center));
    }
    let yard = if kept { n - 1 } else { n };
    let mut by_size: Vec<usize> = (0..yard).collect();
    by_size.sort_by(|&a, &b| area(&l.buildings[b].poly).abs().total_cmp(&area(&l.buildings[a].poly).abs()).then(a.cmp(&b)));
    for (rank, &bi) in by_size.iter().enumerate() {
        let big = area(&l.buildings[bi].poly).abs() > 2500.0;
        let func = ["barracks", "stables", "blacksmith"].get(rank).and_then(|k| catalog::index_of(k));
        let b = &mut l.buildings[bi];
        b.func = func.map(|f| f as u16);
        b.residential = catalog::residential_for(Ward::Castle, big) as u8;
        b.floors = if func.is_some() { 1 + (rank == 0) as u8 } else { 2 + (rng.next_f64() < 0.4) as u8 };
        b.name = trade_name(b.func, rng, t0, center);
    }
    if c.structure.as_deref() == Some("ruin") {
        ruin_works(l, rng);
    }
}

/// A ruin's works: every building a roofless shell (the keep still the castle), some stretches
/// of wall and some towers fallen.
fn ruin_works(l: &mut Layout, rng: &mut Pcg32) {
    for b in &mut l.buildings {
        b.structure = Structure::Ruin;
        if b.func.is_none_or(|f| catalog::CATALOG[f as usize].key != "castle") {
            b.func = None;
            b.residential = catalog::RUINED as u8;
        }
    }
    // Stretches of about 40 ft; roughly one in four is down.
    let mut walls: Vec<Vec<P>> = Vec::new();
    for w in std::mem::take(&mut l.walls) {
        let mut cur: Vec<P> = Vec::new();
        for s in w.windows(2) {
            let n = (dist(s[0], s[1]) / 40.0).ceil().max(1.0) as usize;
            for j in 0..n {
                let (a, b) = (lerp(s[0], s[1], j as f64 / n as f64), lerp(s[0], s[1], (j + 1) as f64 / n as f64));
                if rng.next_f64() < 0.25 {
                    if cur.len() >= 2 {
                        walls.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    continue;
                }
                if cur.last() != Some(&a) {
                    if cur.len() >= 2 {
                        walls.push(std::mem::take(&mut cur));
                    }
                    cur = vec![a];
                }
                cur.push(b);
            }
        }
        if cur.len() >= 2 {
            walls.push(cur);
        }
    }
    l.walls = walls;
    l.towers.retain(|_| rng.next_f64() < 0.65);
}

/// A wall drawn by hand (`Created` kind `wall`): towers on its corners and along long runs, a
/// gatehouse on each corner marked a gate and wherever a road or a town's main street crosses it,
/// broken where it would stand in water.
fn wall_site(world: &World, t0: &T0, c: &crate::world::Created, center: P, l: &mut Layout) {
    let n = c.pts.len();
    if n < 2 {
        return;
    }
    let mut corners: Vec<(P, bool)> = c.pts.iter().enumerate().map(|(i, q)| (sub(*q, center), c.gates.contains(&(i as u32)))).collect();
    // Lanes that pass through: the world's roads, and the approaches and main streets of the
    // towns the wall reaches.
    let (x0, y0, x1, y1) = c.pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, q| (b.0.min(q[0]), b.1.min(q[1]), b.2.max(q[0]), b.3.max(q[1])));
    let mut lanes: Vec<(P, P)> = Vec::new();
    for (ri, k) in t0.roads.segments_near([x0, y0, x1, y1], 20.0) {
        let rc = &t0.roads.roads[ri as usize];
        let pts: Vec<P> = (0..=16).map(|j| sub(rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p, center)).collect();
        lanes.extend(pts.windows(2).map(|s| (s[0], s[1])));
    }
    let hit = |x: f64, y: f64, r: f64| x + r >= x0 && x - r <= x1 && y + r >= y0 && y - r <= y1;
    for (i, s) in t0.settlements.iter().enumerate() {
        if !hit(s.x, s.y, super::reach(s)) {
            continue;
        }
        let town = super::generated_layout(world, t0, i);
        // (Its paved side streets and alleys, class 3, the wall closes.)
        for (pts, _, _) in town.roads.iter().filter(|r| r.1 != 3) {
            lanes.extend(pts.windows(2).map(|q| (sub(q[0], center), sub(q[1], center))));
        }
    }
    // Each crossing is a gate (a corner within 20 ft of it is made one instead), at least
    // 60 ft from the gate before it.
    let sides = if c.closed { n } else { n - 1 };
    let mut line: Vec<(P, bool)> = Vec::new();
    for i in 0..sides {
        let (a, b) = (corners[i].0, corners[(i + 1) % n].0);
        let mut cuts: Vec<f64> = lanes.iter().filter_map(|&(p, q)| crossing(a, b, p, q)).collect();
        cuts.sort_by(f64::total_cmp);
        line.push(corners[i]);
        let len_ab = dist(a, b);
        for t in cuts {
            let at = lerp(a, b, t);
            if t * len_ab < 20.0 {
                line.last_mut().expect("a corner").1 = true;
            } else if (1.0 - t) * len_ab < 20.0 {
                corners[(i + 1) % n].1 = true;
            } else if !line.iter().rev().take(2).any(|g| g.1 && dist(g.0, at) < 60.0) {
                line.push((at, true));
            }
        }
    }
    if c.closed {
        line[0].1 = corners[0].1;
    } else {
        line.push(corners[n - 1]);
    }
    // (Of two gates side by side, the second goes.)
    let mut k = 1;
    while k < line.len() {
        if line[k].1 && line[k - 1].1 && dist(line[k].0, line[k - 1].0) < 60.0 {
            line.remove(k);
        } else {
            k += 1;
        }
    }
    let (pieces, towers, gate_towers) = wall_pieces(&line, c.closed);
    l.walls = pieces;
    l.towers = towers;
    l.gate_towers = gate_towers;
    let ground = Open { t0, center };
    clip_walls(l, |p| ground.wet(p), |p| ground.wet(p));
    if c.closed {
        let ring: Vec<P> = line.iter().map(|q| q.0).collect();
        let cc = centroid(&ring);
        l.castles.push((cc, ring.iter().map(|p| dist(*p, cc)).fold(0.0, f64::max)));
    }
}

/// Where the segment a→b crosses p→q, as a fraction along a→b.
fn crossing(a: P, b: P, p: P, q: P) -> Option<f64> {
    let (r, s) = (sub(b, a), sub(q, p));
    let d = cross(r, s);
    if d.abs() < 1e-9 {
        return None;
    }
    let ap = sub(p, a);
    let (t, u) = (cross(ap, s) / d, cross(ap, r) / d);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}
