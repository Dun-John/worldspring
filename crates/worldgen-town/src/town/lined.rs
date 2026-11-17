//! Bridge-lined rows of houses, bridge houses, islet towers, round-offs and yurt doors.

use super::*;

/// Round roofs on round footprints: each building roofed round as generated (`roof::look`: a steppe
/// home, a shrine; of one piece, its shape not set by hand) stands on the ring its lot holds
/// (`roof::round_footprint`), if it is still roofed round there; the rest of its lot is open ground.
/// Last, once every other building stands, so theirs are as they were.
/// R5 M20: a lined bridge's span (ft): its width (56 for a narrow water, up to 80 over the widest: `lined_width`), rows
/// of houses 16 ft deep (up to 24 on the widest) either side of a lane of at least 24; the least water it spans; how
/// far it reaches onto each bank.
const LINED_WIDTH_FT: f64 = 56.0;
const LINED_WIDTH_MAX_FT: f64 = 80.0;
const LINED_WIDEST_WATER_FT: f64 = 480.0;
const LINED_MIN_WATER_FT: f64 = 120.0;
const LINED_BANK_FT: f64 = 0.0;

/// A lined bridge's width (ft, whole) over `water` ft of water in settlement `seed`: from `LINED_WIDTH_FT` at the least
/// span to `LINED_WIDTH_MAX_FT` at `LINED_WIDEST_WATER_FT` and over, a quarter of it by the town's hash.
fn lined_width(water: f64, seed: i64) -> f64 {
    let t = ((water - LINED_MIN_WATER_FT) / (LINED_WIDEST_WATER_FT - LINED_MIN_WATER_FT)).clamp(0.0, 1.0);
    let t = 0.75 * t + 0.25 * unit(hash2(0x11e3_b51e, seed, 1));
    crate::core::round(LINED_WIDTH_FT + (LINED_WIDTH_MAX_FT - LINED_WIDTH_FT) * t)
}

/// The depth (ft) of a lined bridge's rows at its full depth, by its width: 16 ft at `LINED_WIDTH_FT` to 24 at the widest.
fn lined_row(width: f64) -> f64 {
    16.0 + (width - LINED_WIDTH_FT) / 3.0
}

/// R5 M20: a metropolis's lined bridge. In 1 of 2 metropolises with a main street's bridge over
/// at least `LINED_MIN_WATER_FT` of water, the longest such gets a second deck over the water
/// part only (`lined_width` wide, each side `LINED_BANK_FT` onto its bank, so the landings and the bank
/// streets stay as they are), at one flat level: the highest water under it + 4 ft
/// (`battlemap::deck_top`). It goes in `bridges` (local, after the street's own) and `lined`.
/// None over one of the world's islets. `over`: (water spanned, the crossing, the street's way,
/// water ahead, water behind) per main street's bridge.
pub(super) fn lined_bridge(site: &Site, s: &Settlement, over: &[(f64, P, P, f64, f64)], l: &mut Layout) {
    if s.tier != Tier::Metropolis || unit(hash2(0x11e3_b51d, s.seed as i64, 0)) >= 0.5 {
        return;
    }
    let Some(&(_, p, u, wf, wb)) = over.iter().filter(|o| o.0 >= LINED_MIN_WATER_FT).max_by(|x, y| x.0.total_cmp(&y.0)) else { return };
    let span = wf + wb + 2.0 * LINED_BANK_FT;
    let width = lined_width(wf + wb, s.seed as i64);
    let c = add(p, mul(u, 0.5 * (wf - wb)));
    // (Each long side ends where its own bank begins, 1 ft onto it, so a bank met at a slant
    // takes the deck's end slanting with it: no corner stands out over the land.)
    let v = [-u[1], u[0]];
    let wet = |q: P| site.wet_standing(q) || site.near_river(q, 2.0);
    let reach = |e: P, sign: f64, centre: f64| {
        let mut t = site.dry_reach(e, u, 0.0, sign, 0.0);
        while t > 0.0 && !wet(add(e, mul(u, sign * (t - 1.0)))) {
            t -= 1.0;
        }
        (t + LINED_BANK_FT).clamp(0.6 * centre, centre + 0.5 * width)
    };
    let side = |k: f64| add(p, mul(v, k * 0.5 * width));
    let (l0, l1) = (side(-1.0), side(1.0));
    let deck = vec![
        add(l0, mul(u, -reach(l0, -1.0, wb))),
        add(l0, mul(u, reach(l0, 1.0, wf))),
        add(l1, mul(u, reach(l1, 1.0, wf))),
        add(l1, mul(u, -reach(l1, -1.0, wb))),
    ];
    let deck = if area(&deck) * area(&rect(c, u, span, width)) > 0.0 { deck } else { rect(c, u, span, width) };
    let (a, b) = (add(add(c, mul(u, -0.5 * span)), site.center), add(add(c, mul(u, 0.5 * span)), site.center));
    let world_deck: Vec<P> = deck.iter().map(|&q| add(q, site.center)).collect();
    let bb = deck.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, q| [r[0].min(q[0] + site.center[0]), r[1].min(q[1] + site.center[1]), r[2].max(q[0] + site.center[0]), r[3].max(q[1] + site.center[1])]);
    if crate::t0::islets::near(site.t0, bb).any(|i| poly_gap(&world_deck, &i.pts) < 10.0) {
        return;
    }
    let level = crate::battle::deck_top(a, b, site.t0);
    // (The street's own plain deck over the span, which ran 5 ft past each bank, goes: the lined deck carries the street on, ending at the banks.)
    l.bridges.retain(|d| !geom::contains(&deck, geom::centroid(d)));
    l.bridges.push(deck.clone());
    l.lined = Some((deck, level));
}

/// R5 M20: the houses along both sides of a lined bridge (`Layout::lined`, world ft by now): lots
/// 14-32 ft along it and 60-100% of `lined_row` deep against its outer edges (the back line ragged), 4 ft short of each
/// bank, 1-3 storeys, an open landing now and then, shops (each its own trade, spaced) and homes, their ground floors at the deck's level (`Building::deck`: no
/// cellar, the door on the lane). Lots meeting another building are left empty. Ids follow
/// every other building's.
pub(super) fn bridge_houses(index: usize, l: &mut Layout) {
    let Some((deck, level)) = l.lined.clone() else { return };
    if deck.len() != 4 {
        return;
    }
    const SHOPS: [&str; 8] = ["tailor", "jeweler", "cobbler", "apothecary", "general_store", "fishmonger", "butcher", "weaver"];
    // (Shops at least this far apart along the bridge, either side, so their pins stand apart as a
    // street's do.)
    const SHOP_GAP_FT: f64 = 55.0;
    let (a, b) = (lerp(deck[0], deck[3], 0.5), lerp(deck[1], deck[2], 0.5));
    let span = dist(a, b);
    let u = mul(sub(b, a), 1.0 / span.max(1e-9));
    let v = [-u[1], u[0]];
    let width = crate::core::round(dot(sub(deck[3], deck[0]), v).abs());
    let row = lined_row(width);
    // (Where each end crosses `s` ft across the line: the ends follow the banks, `lined_bridge`.)
    let end_at = |p: P, q: P, s: f64| {
        let (sp, sq) = (dot(sub(p, a), v), dot(sub(q, a), v));
        let k = if (sq - sp).abs() < 1e-9 { 0.5 } else { ((s - sp) / (sq - sp)).clamp(0.0, 1.0) };
        dot(sub(lerp(p, q, k), a), u)
    };
    let mut id = l.buildings.iter().map(|b| b.id + 1).max().unwrap_or(0);
    // (Each lot: its footprint, its middle along the line, its hash.)
    let mut lots: Vec<(Vec<P>, f64, u64)> = Vec::new();
    for (side, sign) in [(0i64, 1.0), (1, -1.0)] {
        let (inner, outer) = (sign * (0.5 * width - row), sign * 0.5 * width);
        // (The row clear of the deck's ends at both its edges, 4 ft short of the banks.)
        let lo = end_at(deck[0], deck[3], inner).max(end_at(deck[0], deck[3], outer)) + 4.0;
        let hi = end_at(deck[1], deck[2], inner).min(end_at(deck[1], deck[2], outer)) - 4.0;
        let mut t = lo;
        let mut k = 0i64;
        while t < hi {
            // (Side by side along the row: they share their walls.)
            let h = hash3(0x11e3_40a5, index as i64, side, k);
            let front = (14.0 + 18.0 * unit(h)).min(hi - t);
            k += 1;
            if front < 14.0 {
                break;
            }
            // (An open landing now and then, 8-12 ft along, never at the first lot.)
            if k > 1 && unit(h ^ 7) < 0.125 {
                t += (0.5 * front).max(8.0);
                continue;
            }
            // (Each lot's depth 60-100% of the row, its inner face ragged against the lane's side, the outer flush with the edge.)
            let depth = crate::core::round(row * (0.6 + 0.4 * unit(h ^ 9)));
            let m = add(add(a, mul(u, t + 0.5 * front)), mul(v, sign * (0.5 * width - 0.5 * depth)));
            t += front;
            let mut poly = rect(m, u, front, depth);
            if area(&poly) < 0.0 {
                poly.reverse();
            }
            if l.buildings.iter().any(|o| poly_gap(&o.poly, &poly) < 0.5) {
                continue;
            }
            lots.push((poly, t - 0.5 * front, h));
        }
    }
    // Shops along the bridge in order, both rows together: one in two or so, each its own trade (the
    // first picked by the town, the rest after it in turn), kept `SHOP_GAP_FT` apart; homes between.
    let mut order: Vec<usize> = (0..lots.len()).collect();
    order.sort_by(|&x, &y| lots[x].1.total_cmp(&lots[y].1));
    let start = (unit(hash2(0x11e3_40a7, index as i64, 0)) * SHOPS.len() as f64) as usize;
    let (mut trades, mut last) = (0usize, f64::MIN);
    let mut func: Vec<Option<u16>> = vec![None; lots.len()];
    for &i in &order {
        let (_, at, h) = lots[i];
        if unit(h ^ 5) < 0.6 && at - last >= SHOP_GAP_FT && trades < SHOPS.len() {
            func[i] = catalog::index_of(SHOPS[(start + trades) % SHOPS.len()]).map(|f| f as u16);
            trades += 1;
            last = at;
        }
    }
    let mut out: Vec<Building> = Vec::new();
    for (i, (poly, _, h)) in lots.into_iter().enumerate() {
        let r = unit(h ^ 3);
        let floors = if r < 0.2 { 1 } else if r < 0.65 { 2 } else { 3 };
        out.push(Building { poly, ward: Ward::Merchant, func: func[i], residential: 1, name: None, floors, pad_ft: level, structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id, parts: Vec::new(), keep: None, deck: Some([a, b]) });
        id += 1;
    }
    l.buildings.extend(out);
}

/// Whether `b` is an islet's watchtower (`catalog` "watchtower")./// Whether `b` is an islet's watchtower (`catalog` "watchtower").
pub fn is_watchtower(b: &Building) -> bool {
    b.func.is_some_and(|f| CATALOG[f as usize].key == "watchtower")
}

/// R5 M20: a round watchtower (2-3 storeys and its battlements, 14-18 ft across) on the middle of
/// each of settlement `index`'s islets that has one (`t0::islets`), its ground floor at the
/// islet's top; none where a bridge's deck runs over it. Ids follow every other building's.
pub(super) fn islet_towers(t0: &T0, index: usize, l: &mut Layout) {
    let Some(func) = catalog::index_of("watchtower") else { return };
    let mut id = l.buildings.iter().map(|b| b.id + 1).max().unwrap_or(0);
    for isl in crate::t0::islets::of(t0, index).filter(|i| i.tower) {
        let h = hash3(0x3a7c_70e5, crate::core::round(isl.at[0]) as i64, crate::core::round(isl.at[1]) as i64, index as i64);
        let rad = 7.0 + 2.0 * unit(h);
        let a0 = unit(h ^ 1) * std::f64::consts::TAU;
        let mut poly: Vec<P> = (0..rural::MILL_SIDES)
            .map(|k| {
                let a = a0 + std::f64::consts::TAU * k as f64 / rural::MILL_SIDES as f64;
                [isl.at[0] + rad * libm::cos(a), isl.at[1] + rad * libm::sin(a)]
            })
            .collect();
        if area(&poly) < 0.0 {
            poly.reverse();
        }
        if l.bridges.iter().any(|d| poly_gap(d, &poly) < 4.0) {
            continue;
        }
        let floors = 2 + (unit(h ^ 3) < 0.5) as u8;
        l.buildings.push(Building { poly, ward: Ward::Docks, func: Some(func as u16), residential: 1, name: None, floors, pad_ft: isl.level, structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id, parts: Vec::new(), keep: None, deck: None });
        id += 1;
    }
}

pub(crate) fn round_off(l: &mut Layout, site: Option<&Site>) {
    let mut found: Vec<(usize, Vec<P>)> = Vec::new();
    for i in 0..l.buildings.len() {
        let b = &l.buildings[i];
        let hand = b.shape.is_some() || b.roof.is_some();
        if b.structure != Structure::Roofed || hand || !b.parts.is_empty() || b.keep.is_some() || roof::look(l, b).shape != roof::Shape::Round {
            continue;
        }
        // (R5 towns: a steppe town's hall, temple or inn a round felt hall of one storey, `roof::hall_ring`.)
        let hall = roof::hall_ring(l, b);
        // (R5 towns, 115: a steppe home's yurt sized by its wealth and lot, `yurts::ring`.)
        let yurt = hall.is_none() && yurts::is_steppe_home(l, b);
        let Some(poly) = hall.clone().or_else(|| if yurt { yurts::ring(l, b) } else { roof::round_footprint(&b.poly) }) else { continue };
        let mut round = b.clone();
        round.poly = poly;
        if roof::look(l, &round).shape == roof::Shape::Round {
            let lot = l.buildings[i].poly.clone();
            let ring = yurt_door(l, i, round.poly, &lot, site, None);
            l.buildings[i].poly = ring;
            if hall.is_some() {
                l.buildings[i].floors = 1;
            }
            if yurt {
                found.push((i, lot));
            }
        }
    }
    yurts::finish(l, site, found);
}

/// How far a yurt looks for a lane to face (ft from its middle to the lane's nearest point).
pub(super) const YURT_LANE_REACH: f64 = 70.0;
/// The most a yurt's door swings off its lane or the centre (degrees, either way).
const YURT_SWING: f64 = 20.0;
/// How far out from its door a yurt wants open ground (ft; `interior`'s squares there are then free too).
pub const YURT_CLEAR_FT: f64 = 10.0;

/// A yurt's ring turned so its first side (corners 0 and 1) is its door: facing the nearest lane or
/// street within `YURT_LANE_REACH`, else the layout's centre (a camp's fire, a green), swung a little by
/// a hash of the layout and the building's id; the nearest side whose outside is clear of other roofs
/// if that one is shut in. Carried by the ring's order alone: `interior::furnish_yurt` and the
/// battlemap's roof (`battlePrep.yurtDoor`) both take the door from side 0.
/// `lot`: the ground it stands on; `aim`: a point to face in place of the nearest lane's (R5 towns, 115:
/// a cluster's shared lane, `yurts::finish`), its swing then a quarter.
pub(crate) fn yurt_door(l: &Layout, i: usize, mut ring: Vec<P>, lot: &[P], site: Option<&Site>, aim: Option<P>) -> Vec<P> {
    let m = ring.len();
    let c = centroid(&ring);
    // (R5 towns: a felt hall's reach as far past its wall as a yurt's, its door swung less.)
    let hall = m == roof::HALL_SIDES;
    let reach = if hall { YURT_LANE_REACH + (dist(ring[0], c) - 15.0).max(0.0) } else { YURT_LANE_REACH };
    let mut best: Option<(f64, P)> = None;
    // (A felt hall's: a square's edge too.)
    let ways = l.roads.iter().map(|r| &r.0).chain(l.streets.iter()).chain(l.plazas.iter().filter(|_| hall).map(|p| &p.pts));
    for w in ways {
        for e in w.windows(2) {
            let ab = sub(e[1], e[0]);
            let t = (dot(sub(c, e[0]), ab) / dot(ab, ab).max(1e-12)).clamp(0.0, 1.0);
            let q = add(e[0], mul(ab, t));
            let d = dist(c, q);
            if d <= reach && best.is_none_or(|b| d < b.0) {
                best = Some((d, q));
            }
        }
    }
    let forced = aim.filter(|q| dist(*q, c) > 1.0);
    let aim = forced.or(best.map(|b| b.1).filter(|q| dist(*q, c) > 1.0)).unwrap_or(l.center);
    let to = if dist(aim, c) > 1.0 { sub(aim, c) } else { [0.0, 1.0] };
    let swing = signed(hash2(hash2(l.roof_seed, l.buildings[i].id as i64, 0x7975), 1, 0)) * if hall || forced.is_some() { 0.25 } else { 1.0 } * YURT_SWING.to_radians();
    let a = libm::atan2(to[1], to[0]) + swing;
    let want = [libm::cos(a), libm::sin(a)];
    let mid = |e: usize| lerp(ring[e], ring[(e + 1) % m], 0.5);
    // (Open ground out to `out` ft from the side: under no other roof or ground of another's lot, on no water.)
    let me = l.buildings[i].id;
    let open = |e: usize, out: f64| {
        let d = sub(mid(e), c);
        let n = mul(d, 1.0 / len(d).max(1e-9));
        // (A strip 10 ft wide: the interior's door lands on a square beside the side's middle.)
        [0.25, 0.5, 0.75, 1.0].iter().flat_map(|&t| [-5.0, -2.5, 0.0, 2.5, 5.0].map(|w| (t, w))).all(|(t, w)| {
            let p = add(add(mid(e), mul(n, out * t)), mul([-n[1], n[0]], w));
            !l.buildings.iter().enumerate().any(|(k, o)| k != i && o.structure == Structure::Roofed && dist(o.poly[0], c) < 400.0 && contains(&o.poly, p))
                && !l.grounds.iter().any(|g| g.0 != me && dist(g.1[0], c) < 400.0 && !on_lot(&g.1, lot) && contains(&g.1, p))
                && !site.is_some_and(|s| s.wet(sub(p, s.center)))
        })
    };
    let mut sides: Vec<(f64, usize)> = (0..m)
        .map(|e| {
            let d = sub(mid(e), c);
            (-(dot(d, want) / len(d).max(1e-9)), e)
        })
        .collect();
    sides.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let door = [YURT_CLEAR_FT, 4.0].iter().find_map(|&out| sides.iter().map(|s| s.1).find(|&e| open(e, out))).unwrap_or(sides[0].1);
    ring.rotate_left(door);
    ring
}
