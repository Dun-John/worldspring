//! Layouts for points of interest: ruins (broken shells around a courtyard, sometimes a
//! fallen hall, and a stair down to a dungeon or crypt), wizard's towers (a round tower with
//! an outbuilding), roadside inns (inn, stables and a yard beside the road), and the mouths
//! of caves, mines and lava tubes. Same `Layout` as settlements, so battlemaps, site tiles
//! and the gazetteer treat them alike.

use super::catalog::{self, Ward};
use super::geom::*;
use super::{Building, Layout, Structure, rect};
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
        PoiKind::Building => {
            // Drawn by hand: one building, as the world file has it.
            if let Some(c) = poi.checked_sub(t0.base_pois).and_then(|k| world.file.edits.created.get(k)) {
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
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for q in buildings.iter().flat_map(|b| b.poly.iter()).chain(plazas.iter().flatten()).chain(yards.iter().flat_map(|y| y.0.iter())) {
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
    let mut l = Layout {
        index,
        center,
        radius: 80.0,
        tier: Tier::Village,
        on_water: false,
        walls: Vec::new(),
        towers: Vec::new(),
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
        gate_towers: Vec::new(),
        monuments: Vec::new(),
        quarters: Vec::new(),
        castles: Vec::new(),
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
