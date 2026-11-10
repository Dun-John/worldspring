//! Roofs: a material, a shape and a weathering per building, picked from the settlement's culture
//! (`Layout::culture`: 0 north, 1 west, 2 south, 3 dry, 4 steppe), its ward (wealth), its function
//! (civic and sacred stand out, working buildings plain), its size and storeys, and its own hash.
//! A pure function of the layout and the building (`look`), so a function or storeys changed by
//! hand change the roof with them; a material or shape set by hand (`Building::material`,
//! `Building::shape`, or the old `tint`) wins.
//!
//! The ids below are stable once shipped (payload, edits by name): only ever append.
//!
//! Materials (`MATERIALS`, colours as `app/src/gen/protocol.ts` `ROOF_MATERIALS`):
//!   0 thatch, 1 slate, 2 clay_tile (terracotta barrel), 3 shingle (wood shingle/shake), 4 turf,
//!   5 lead, 6 copper (verdigris), 7 glazed_tile, 8 flat_clay (mud plaster), 9 hide (hide/felt),
//!   10 plank, 11 whitewash (lime-washed plaster: the dry lands' domes).
//! Shapes (`SHAPES`): 0 hip, 1 gable, 2 cone, 3 dome, 4 flat (behind a parapet), 5 round (a low
//!   cone, yurt-like).
//! Weathering: 0-255 (moss, missing tiles, repairs; more on poor wards, old towns, sites), and
//!   `Look::smoke`: a smoke-darkened patch (smithies, kilns).

use super::catalog::{CATALOG, Ward};
use super::geom::P;
use super::{Building, Layout, RoofStyle, Structure};
use crate::core::rng::{hash3, unit};

pub const MATERIALS: [&str; 12] = ["thatch", "slate", "clay_tile", "shingle", "turf", "lead", "copper", "glazed_tile", "flat_clay", "hide", "plank", "whitewash"];
pub const SHAPES: [&str; 6] = ["hip", "gable", "cone", "dome", "flat", "round"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Material {
    Thatch = 0,
    Slate,
    ClayTile,
    Shingle,
    Turf,
    Lead,
    Copper,
    GlazedTile,
    FlatClay,
    Hide,
    Plank,
    Whitewash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Shape {
    Hip = 0,
    Gable,
    Cone,
    Dome,
    Flat,
    Round,
}

use Material::*;
use Shape::*;

/// The old hand-set tints (`world::TINTS`: terracotta, slate, thatch, shingle, moss) as materials
/// (their colours are those materials' base colours).
pub const TINT_MATERIAL: [Material; 5] = [ClayTile, Slate, Thatch, Shingle, Turf];

const ALL_MATERIALS: [Material; 12] = [Thatch, Slate, ClayTile, Shingle, Turf, Lead, Copper, GlazedTile, FlatClay, Hide, Plank, Whitewash];
const ALL_SHAPES: [Shape; 6] = [Hip, Gable, Cone, Dome, Flat, Round];

impl Material {
    pub fn from_u8(v: u8) -> Option<Material> {
        ALL_MATERIALS.get(v as usize).copied()
    }
    pub fn by_name(s: &str) -> Option<Material> {
        MATERIALS.iter().position(|m| *m == s).map(|i| ALL_MATERIALS[i])
    }
}

impl Shape {
    pub fn from_u8(v: u8) -> Option<Shape> {
        ALL_SHAPES.get(v as usize).copied()
    }
    pub fn by_name(s: &str) -> Option<Shape> {
        SHAPES.iter().position(|m| *m == s).map(|i| ALL_SHAPES[i])
    }
}

/// A building's roof as picked (or set by hand).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub material: Material,
    pub shape: Shape,
    pub wear: u8,
    pub smoke: bool,
    /// Colour variant 0-4 of the material (`app/src/gen/protocol.ts` `ROOF_SHADES`); 2 the usual.
    pub shade: u8,
}

/// What a building is for, as its roof tells it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Home,
    Civic,
    Sacred,
    Working,
    Trade,
}

fn role(b: &Building) -> Role {
    let Some(f) = b.func else { return Role::Home };
    let f = &CATALOG[f as usize];
    match f.key {
        "temple" | "shrine" | "monastery" | "mausoleum" | "catacombs" | "tomb" => Role::Sacred,
        "library" | "arcane_academy" | "observatory" | "bank" | "auction_house" | "merchant_guild" | "craft_guild" | "mages_guild" | "theater" | "bards_college" | "arena" | "bathhouse" => Role::Civic,
        "blacksmith" | "armorer" | "weaponsmith" | "glassblower" | "potter" | "brewery" | "winery" | "distillery" | "leatherworker" | "mill" | "windmill" | "stables" | "warehouse" | "shipwright" | "carpenter" | "mason" | "cooper" | "cartwright" | "weaver" => Role::Working,
        _ if f.category == "civic & military" => Role::Civic,
        _ => Role::Trade,
    }
}

/// Wealth 0 poor, 1 middling, 2 rich: the ward's, a home's kind nudging it.
fn wealth(b: &Building) -> u8 {
    if b.func.is_none() {
        match b.residential {
            0 | 5 => return 0,
            2 => return 2.min(ward_wealth(b.ward) + 1),
            4 => return 2,
            _ => {}
        }
    }
    ward_wealth(b.ward)
}

fn ward_wealth(w: Ward) -> u8 {
    match w {
        Ward::Noble | Ward::Castle | Ward::Plaza | Ward::Temple => 2,
        Ward::Merchant | Ward::Common | Ward::Craft | Ward::Military | Ward::Docks | Ward::Park => 1,
        Ward::Slum | Ward::Farm | Ward::Rural => 0,
    }
}

/// Picks one of `opts` by weight with `u` in [0, 1).
fn pick<T: Copy>(u: f64, opts: &[(T, u32)]) -> T {
    let total: u32 = opts.iter().map(|o| o.1).sum();
    let mut t = u * total as f64;
    for o in opts {
        if t < o.1 as f64 {
            return o.0;
        }
        t -= o.1 as f64;
    }
    opts[opts.len() - 1].0
}

/// A building's roof: as set by hand, else as its culture, ward, function and size have it.
pub fn look(l: &Layout, b: &Building) -> Look {
    let h = |k: i64| unit(hash3(l.roof_seed, b.id as i64, k, 0x726f_6f66));
    let culture = l.culture % 5;
    let role = role(b);
    let rich = wealth(b);
    let city = l.tier as u8 >= 2;
    let o = super::geom::obb(&b.poly);
    let area = super::geom::area(&b.poly).abs();
    let compact = o.long <= 1.35 * o.short.max(1.0);
    let small = area < 1000.0 && b.floors <= 1;
    // (A village's manor is a big house, not a grand one: `town::village`.)
    let grand = matches!(role, Role::Civic | Role::Sacred) || (rich == 2 && area > 1600.0 && l.tier != crate::t0::settle::Tier::Village);
    let key = b.func.map(|f| CATALOG[f as usize].key).unwrap_or("");
    // (R5 towns: a cemetery's tombs in the dry lands domed, `cemetery`.)
    let temple = matches!(key, "temple" | "monastery" | "mausoleum") || (key == "tomb" && culture == 3);
    let (u0, u1, u2) = (h(1), h(2), h(3));
    let (material, shape) = match culture {
        // North: steep shake and turf, dark slate on the rich; stave-like shingle on the sacred.
        0 => {
            let m = match role {
                Role::Sacred => pick(u0, &[(Shingle, 5), (Lead, 3), (Slate, 2)]),
                Role::Civic => pick(u0, &[(Slate, 4), (Lead, 3), (Shingle, 3)]),
                Role::Working => pick(u0, &[(Shingle, 4), (Turf, 3), (Plank, 3)]),
                _ => match rich {
                    0 => pick(u0, &[(Turf, 6), (Shingle, 3), (Thatch, 1)]),
                    1 => pick(u0, &[(Shingle, 6), (Turf, 3), (Slate, 1)]),
                    _ => pick(u0, &[(Slate, 5), (Shingle, 5)]),
                },
            };
            (m, if temple && compact { Cone } else { pick(u1, &[(Gable, 7), (Hip, 3)]) })
        }
        // West: thatch on the poor, slate on the rich, red clay tile in the big cities.
        1 => {
            let m = match role {
                Role::Sacred => pick(u0, &[(Lead, 4), (Slate, 4), (Copper, 2)]),
                Role::Civic => pick(u0, &[(Slate, 4), (Lead, 3), (Copper, 2), (ClayTile, 1)]),
                Role::Working => pick(u0, &[(Shingle, 4), (Plank, 2), (Thatch, 3), (ClayTile, if city { 2 } else { 0 })]),
                _ => match (rich, city) {
                    (0, false) => pick(u0, &[(Thatch, 8), (Shingle, 2)]),
                    (0, true) => pick(u0, &[(Thatch, 5), (ClayTile, 3), (Shingle, 2)]),
                    (1, false) => pick(u0, &[(Thatch, 5), (Shingle, 3), (Slate, 2)]),
                    (1, true) => pick(u0, &[(ClayTile, 4), (Slate, 3), (Thatch, 2), (Shingle, 1)]),
                    (_, false) => pick(u0, &[(Slate, 7), (Shingle, 2), (ClayTile, 1)]),
                    (_, true) => pick(u0, &[(Slate, 6), (ClayTile, 4)]),
                },
            };
            (m, pick(u1, &[(Hip, 5), (Gable, 5)]))
        }
        // South: terracotta barrel tile at a low pitch, tiled domes on temples.
        2 => {
            let m = match role {
                Role::Sacred => pick(u0, &[(GlazedTile, 4), (ClayTile, 4), (Copper, 2)]),
                Role::Civic => pick(u0, &[(ClayTile, 5), (GlazedTile, 3), (Copper, 1), (Lead, 1)]),
                Role::Working => pick(u0, &[(ClayTile, 6), (Plank, 3), (Thatch, 1)]),
                _ => match rich {
                    0 => pick(u0, &[(ClayTile, 6), (Thatch, 2), (FlatClay, 2)]),
                    1 => pick(u0, &[(ClayTile, 9), (FlatClay, 1)]),
                    _ => pick(u0, &[(ClayTile, 8), (GlazedTile, 2)]),
                },
            };
            let shape = if m == FlatClay {
                Flat
            } else if (temple || (grand && compact && u2 < 0.4)) && role != Role::Working {
                Dome
            } else {
                pick(u1, &[(Hip, 7), (Gable, 3)])
            };
            (m, shape)
        }
        // Dry: flat clay behind parapets; whitewashed domes on temples, some grand buildings and a
        // few homes.
        3 => {
            let dome = temple || (grand && u2 < 0.5) || (role == Role::Home && compact && u2 < if rich == 2 { 0.25 } else { 0.06 });
            if dome {
                (if grand && u0 < 0.35 { GlazedTile } else { Whitewash }, Dome)
            } else {
                let m = match role {
                    Role::Working => pick(u0, &[(FlatClay, 8), (Plank, 2)]),
                    _ => pick(u0, &[(FlatClay, 9), (Whitewash, 1)]),
                };
                (m, Flat)
            }
        }
        // Steppe: low hide/felt and thatch, round roofs on small homes (on a ring their lot holds:
        // `round_footprint`).
        _ => {
            let ring = || b.poly.len() == ROUND_SIDES || round_footprint(&b.poly).is_some();
            if role == Role::Home && small && (compact || u2 < 0.5) && ring() {
                (pick(u0, &[(Hide, 7), (Thatch, 3)]), Round)
            } else {
                let m = match role {
                    Role::Sacred => pick(u0, &[(GlazedTile, 4), (Copper, 3), (Hide, 3)]),
                    Role::Civic => pick(u0, &[(Shingle, 3), (GlazedTile, 3), (Hide, 2), (ClayTile, 2)]),
                    Role::Working => pick(u0, &[(Plank, 5), (Thatch, 3), (Hide, 2)]),
                    _ => match rich {
                        0 => pick(u0, &[(Hide, 5), (Thatch, 5)]),
                        1 => pick(u0, &[(Thatch, 6), (Hide, 2), (Shingle, 2)]),
                        _ => pick(u0, &[(Shingle, 4), (Thatch, 3), (ClayTile, 3)]),
                    },
                };
                let shape = if temple && compact {
                    Dome
                } else if role == Role::Sacred && u1 < 0.5 && ring() {
                    Round
                } else {
                    pick(u1, &[(Hip, 6), (Gable, 4)])
                };
                (m, shape)
            }
        }
    };
    // R5 towns: a cemetery's tomb outside the dry lands in quiet stone, lead or weathered pale tile.
    let tomb = key == "tomb" && culture != 3;
    let material = if tomb { pick(h(12), &[(Slate, 4), (Lead, 3), (ClayTile, 3)]) } else { material };
    // A tower mill: its battered walls rising to the cap, seen from above as a cone (the cap drawn
    // over it with the sails): lime-washed in the south and the dry lands, else tarred boards,
    // stone or weatherboard.
    let (material, shape) = if key == "windmill" {
        (if matches!(culture, 2 | 3) { Whitewash } else { pick(u0, &[(Plank, 4), (Slate, 3), (Shingle, 3)]) }, Cone)
    } else {
        (material, shape)
    };
    // (Hand-set: a material, else the old tint; a shape, else the old roof style.)
    let material = b.material.and_then(Material::from_u8).or_else(|| b.tint.and_then(|t| TINT_MATERIAL.get(t as usize).copied())).unwrap_or(material);
    let shape = match (b.shape.and_then(Shape::from_u8), b.roof) {
        (Some(s), _) => s,
        (None, Some(RoofStyle::Cone)) => Cone,
        (None, Some(RoofStyle::Hip)) => Hip,
        // (A dome or a flat roof on a thatched or hide roof set by hand: a hip / a round.)
        _ if b.material.is_some() || b.tint.is_some() => match (material, shape) {
            (Thatch | Turf | Shingle | Plank, Dome | Flat) => Hip,
            (Hide, Dome | Flat) => Round,
            (FlatClay, Hip | Gable) => Flat,
            _ => shape,
        },
        _ => shape,
    };
    // Weathering: the town's age, poverty, soft materials, a site's neglect; grand ones kept up.
    let mut wear = l.roof_age as f64 + [70.0, 30.0, 10.0][rich as usize] + 60.0 * h(4);
    wear += match material {
        Thatch | Turf | Hide => 25.0,
        Shingle | Plank => 15.0,
        GlazedTile | Copper | Lead | Whitewash => -20.0,
        _ => 0.0,
    };
    if grand {
        wear -= 30.0;
    }
    if tomb {
        wear += 60.0;
    }
    if l.site || b.structure == Structure::Ruin {
        wear += 70.0;
    }
    let smoke = matches!(key, "blacksmith" | "armorer" | "weaponsmith" | "glassblower" | "potter" | "distillery");
    // Colour: by hand, else (an old tint keeping its colour) the usual one or a hashed variant.
    let shade = match b.shade {
        Some(s) => s.min(4),
        None if b.tint.is_some() => 2,
        // (A tomb's the material's quiet ones: moss or warm grey slate, warm or light lead, brown tile.)
        None if tomb => match material {
            Slate => pick(h(13), &[(1u8, 1), (3, 1)]),
            Lead => pick(h(13), &[(3u8, 1), (4, 1)]),
            _ => 3,
        },
        None => pick(h(11), &[(0u8, 1), (1, 3), (2, 8), (3, 3), (4, 1)]),
    };
    Look { material, shape, wear: wear.clamp(0.0, 255.0) as u8, smoke, shade }
}

/// Sides of a round roof's footprint (`round_footprint`).
pub const ROUND_SIDES: usize = 16;

/// A round roof's footprint (`Shape::Round`: a steppe home's, a shrine's) on its lot: a ring of
/// `ROUND_SIDES` about the lot's widest middle, a yard inside its sides (half a foot on a tight lot),
/// wound as the lot is; `None` on a lot under about 19 ft across (no round roof there: `look`).
/// The walls, the interior and the roof all follow it (`super::round_off`).
pub fn round_footprint(lot: &[P]) -> Option<Vec<P>> {
    if lot.len() < 3 {
        return None;
    }
    let (c, clear) = widest(lot);
    if clear - 0.5 < 9.0 {
        return None;
    }
    // (A yard round it where the lot allows: about 6 ft to the lot's sides, at least 18 ft across,
    // at most 30.)
    let r = (clear - 6.0).clamp(9.0, 15.0);
    let u = super::geom::obb(lot).axis;
    let mut out: Vec<P> = (0..ROUND_SIDES)
        .map(|k| {
            let a = std::f64::consts::TAU * (k as f64 + 0.5) / ROUND_SIDES as f64;
            let (co, si) = (libm::cos(a), libm::sin(a));
            [c[0] + r * (co * u[0] - si * u[1]), c[1] + r * (co * u[1] + si * u[0])]
        })
        .collect();
    if super::geom::area(lot) < 0.0 {
        out.reverse();
    }
    Some(out)
}

/// Where a domed roof's domes stand (world ft): each its middle and its drum's radius, all inside the
/// footprint with a foot of roof round them: one over its widest middle, two along a long one (if
/// each half holds one near as big); none if it can't hold a dome 5 ft across. The battlemap draws
/// the same (`app/src/gen/roofGeom.ts` `domeSpots`, its maths step for step) and the roof terrace
/// keeps them clear (`interior::terrace`).
pub fn domes(poly: &[P]) -> Vec<(P, f64)> {
    use super::geom::{add, clip_plain, mul};
    let m = poly.len();
    if m < 3 {
        return Vec::new();
    }
    // (`frame`: the corners' mean, the longest side's way, the extent along and across it.)
    let c = poly.iter().fold([0.0, 0.0], |a, p| add(a, mul(*p, 1.0 / m as f64)));
    let (mut ud, mut best) = ([1.0, 0.0], 0.0);
    for k in 0..m {
        let e = [poly[(k + 1) % m][0] - poly[k][0], poly[(k + 1) % m][1] - poly[k][1]];
        let len = crate::core::sqrt(e[0] * e[0] + e[1] * e[1]);
        if len > best {
            (ud, best) = ([e[0] / len, e[1] / len], len);
        }
    }
    let (mut lo, mut hi, mut plo, mut phi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in poly {
        let d = [p[0] - c[0], p[1] - c[1]];
        let (a, q) = (d[0] * ud[0] + d[1] * ud[1], -d[0] * ud[1] + d[1] * ud[0]);
        (lo, hi, plo, phi) = (lo.min(a), hi.max(a), plo.min(q), phi.max(q));
    }
    let (mid, half_w, len) = (0.5 * (plo + phi), 0.5 * (phi - plo), hi - lo);
    let r0 = [c[0] - ud[1] * mid, c[1] + ud[0] * mid];
    let size = |w: f64, c: f64| (0.448 * w).min(c - 1.0);
    let (p, cl) = widest(poly);
    let mut out = vec![(p, size(len.min(2.0 * half_w), cl))];
    if len > 1.75 * 2.0 * half_w {
        let t = (lo + hi) * 0.5;
        let k = (r0[0] + ud[0] * t) * ud[0] + (r0[1] + ud[1] * t) * ud[1];
        let (a, b) = (clip_plain(poly, ud, k), clip_plain(poly, mul(ud, -1.0), -k));
        if a.len() >= 3 && b.len() >= 3 {
            let ((pa, ca), (pb, cb)) = (widest(&a), widest(&b));
            let two = size((len * 0.5).min(2.0 * half_w), ca).min(size((len * 0.5).min(2.0 * half_w), cb));
            if two >= 0.7 * out[0].1 {
                out = vec![(pa, two), (pb, two)];
            }
        }
    }
    if out[0].1 >= 2.5 { out } else { Vec::new() }
}

/// How far a point is from a polygon's nearest side, negative outside it.
fn clearance(poly: &[P], x: f64, y: f64) -> f64 {
    let m = poly.len();
    let (mut d, mut inside) = (f64::MAX, false);
    let mut j = m - 1;
    for i in 0..m {
        let ([ax, ay], [bx, by]) = (poly[i], poly[j]);
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
        let (ex, ey) = (bx - ax, by - ay);
        let t = (((x - ax) * ex + (y - ay) * ey) / (ex * ex + ey * ey).max(1e-12)).clamp(0.0, 1.0);
        let (dx, dy) = (x - ax - ex * t, y - ay - ey * t);
        d = d.min(crate::core::sqrt(dx * dx + dy * dy));
        j = i;
    }
    if inside { d } else { -d }
}

/// The middle of the biggest circle in a polygon (the nearest such to its corners' mean) and its
/// radius: a coarse grid over it, then steps that halve.
fn widest(poly: &[P]) -> (P, f64) {
    const STEPS: [f64; 16] = [1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, -1.0];
    let m = poly.len() as f64;
    let (mut cx, mut cy, mut x0, mut y0, mut x1, mut y1) = (0.0, 0.0, f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &[x, y] in poly {
        (cx, cy) = (cx + x / m, cy + y / m);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    let size = (x1 - x0).max(y1 - y0);
    let score = |x: f64, y: f64| clearance(poly, x, y) - 0.02 * crate::core::sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
    let (mut bx, mut by, mut bs) = (cx, cy, score(cx, cy));
    for i in 0..=8 {
        for j in 0..=8 {
            let (x, y) = (x0 + (x1 - x0) * i as f64 / 8.0, y0 + (y1 - y0) * j as f64 / 8.0);
            let s = score(x, y);
            if s > bs {
                (bx, by, bs) = (x, y, s);
            }
        }
    }
    let mut step = size / 16.0;
    let mut it = 0;
    while it < 200 && step > size * 1e-4 {
        let mut moved = false;
        for k in (0..16).step_by(2) {
            let (x, y) = (bx + STEPS[k] * step, by + STEPS[k + 1] * step);
            let s = score(x, y);
            if s > bs {
                (bx, by, bs, moved) = (x, y, s, true);
            }
        }
        if !moved {
            step /= 2.0;
        }
        it += 1;
    }
    ([bx, by], clearance(poly, bx, by))
}
