//! Laying out a hand town: people and size, blank towns, works, parks, walls,
//! castle blocks and junction squares.

use super::*;

/// A plot drawn by hand (`Created` kind `plot`, layout `index`): its outline, about its point,
/// filled as a hand town's block is by its fill (`Created::fill`; none: common lots), its lots
/// fronting its own sides, seeded by its id; businesses named as a hand town's are; laid out in
/// world ft. Its buildings number from 0 as a site's do. The nearest settlement gives a
/// cemetery its looks.
pub fn plot_layout(world: &World, t0: &T0, index: usize, c: &crate::world::Created) -> Layout {
    use super::super::fill::{Block as Region, build_block};
    let origin = [c.x, c.y];
    let reach = c.reach();
    let tier = Tier::Town;
    let ground = KeptGround::at(world, t0, index, origin, reach, tier, 0);
    let site = &ground.site;
    let mut l = Layout::empty(index as u32, origin, reach, tier);
    l.set_roofs(world, t0, c.culture.unwrap_or_else(|| crate::t0::made::culture_at(t0, c.x, c.y)));
    let poly: Vec<P> = c.poly.iter().map(|p| sub(*p, origin)).collect();
    let g = super::super::clip::region(&poly);
    let seed = crate::core::hash::fnv64(c.id.as_bytes()) ^ world.seed;
    let near = |s: &&crate::t0::settle::Settlement| dist([s.x, s.y], origin);
    let Some(s) = t0.settlements.iter().min_by(|a, b| near(a).total_cmp(&near(b))) else { return l };
    let mut plots = Vec::new();
    match c.fill.clone().unwrap_or_default() {
        BlockFill::Hand | BlockFill::Castle { .. } => {}
        BlockFill::Generated { ward, lots, height, reroll, .. } => {
            let Some(w) = ward_of(&ward) else { return l };
            let salt = |h: u64| if reroll == 0 { h } else { hash3(h, reroll as i64, 0x7e70, 1) };
            let mut dp = District::roll(w, salt(hash2(seed, 0, 0xd15)));
            if let Some(k) = lots.as_deref().and_then(|n| LOT_SIZES.iter().position(|x| *x == n)) {
                dp.min_sq = District::base(w) * super::super::wards::LOT_FACTOR[k];
            }
            super::super::docks_on_slope(&mut dp, w, if poly.len() >= 3 { site.grade(&poly) } else { 0.0 });
            let mut rng = Pcg32::new(salt(hash2(seed, 0, 0x107)), 47);
            l.blocks.extend(g.iter().map(|x| x.outer.clone()));
            // (Its lots front its own sides, as a block's front its streets: without a front no
            // lot is cut. Outward normals, the outline taken anticlockwise.)
            let ring: Vec<P> = if area(&poly) < 0.0 { poly.iter().rev().copied().collect() } else { poly.clone() };
            let fronts: Vec<super::super::fill::Front> = (0..ring.len())
                .filter_map(|e| {
                    let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
                    let el = dist(a, b);
                    (el > 1e-9).then(|| {
                        let n = [(b[1] - a[1]) / el, -(b[0] - a[0]) / el];
                        super::super::fill::Front { n, c: dot(a, n), a, b }
                    })
                })
                .collect();
            build_block(site, Region::Region(&g), &fronts, w, &dp, &mut rng, &mut l, None);
            // (Its alleys stop at its outline, round ends and all: no street runs on past it.)
            for (pts, _, width) in &mut l.roads {
                if let [a, .., b] = pts.as_mut_slice() {
                    let (d, h) = (dist(*a, *b), 0.5 * *width);
                    if d > 2.0 * h + 1.0 {
                        let u = mul(sub(*b, *a), 1.0 / d);
                        (*a, *b) = (add(*a, mul(u, h)), sub(*b, mul(u, h)));
                    }
                }
            }
            home_storeys(&mut l, 0, w, tier, hash2(salt(seed), 0, 0x5707), &[]);
            businesses(&mut l, 0, w, tier, false, hash2(salt(seed), 0, 0xb05), &[]);
            height_storeys(&mut l, 0, height.as_deref(), &[]);
        }
        BlockFill::Open { open, style } => {
            let mut pieces: Vec<(f64, usize)> = g.iter().enumerate().map(|(k, x)| (x.area(), k)).filter(|x| x.0 >= 200.0).collect();
            pieces.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
            for (pi, &(_, k)) in pieces.iter().enumerate() {
                let salt = |h: u64| if pi == 0 { h } else { hash2(h, pi as i64, 0x91ec) };
                let roll = |j: i64| unit(salt(hash3(seed, 0, j, 0x09e)));
                let cem = || super::super::cemetery::Style::of(s, index, false);
                let fill = open_fill(open, style.as_deref(), &[], salt(hash2(seed, 0, 0x9a4c)), cem, &mut || Pcg32::new(salt(hash2(seed, 0, 0xfa)), 61), tier);
                super::super::fill_block(site, &g[k].outer, Vec::new(), fill, &roll, super::super::places::Spot::None, &mut l, &mut plots);
            }
        }
    }
    // (A farm's fields, off the sand, as a hand town's.)
    l.fields.extend(plots.into_iter().filter_map(|(f, _)| super::super::fields::off_sand(site, &f)));
    // Local → world (as `build_hand` moves its own).
    let tw = |p: &mut P| *p = add(*p, origin);
    for b in &mut l.buildings {
        b.poly.iter_mut().chain(b.parts.iter_mut().flat_map(|p| p.poly.iter_mut())).for_each(tw);
    }
    for (pts, _, _) in &mut l.roads {
        pts.iter_mut().for_each(tw);
    }
    for poly in l.streets.iter_mut().chain(l.plazas.iter_mut().map(|p| &mut p.pts)).chain(&mut l.fields).chain(&mut l.districts).chain(&mut l.blocks).chain(&mut l.monuments).chain(&mut l.bridges) {
        poly.iter_mut().for_each(tw);
    }
    for pk in &mut l.parks {
        pk.pts.iter_mut().chain([&mut pk.ground.origin]).for_each(tw);
    }
    for c in &mut l.crofts {
        c.pts.iter_mut().for_each(tw);
    }
    for (_, g) in &mut l.grounds {
        g.iter_mut().for_each(tw);
    }
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in l.buildings.iter().flat_map(|b| b.poly.iter()).chain(l.blocks.iter().flatten()).chain(l.fields.iter().flatten()).chain(l.plazas.iter().flat_map(|p| p.pts.iter())).chain(l.parks.iter().flat_map(|p| p.pts.iter())) {
        bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
    }
    l.bbox = if bb[0] <= bb[2] { bb } else { [origin[0], origin[1], origin[0], origin[1]] };
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (k, b) in l.buildings.iter_mut().enumerate() {
        b.id = k as u32;
        if b.func.is_some() && b.name.is_none() {
            for salt in 0..8 {
                let mut rng = Pcg32::new(hash3(seed, k as i64, 0x4e + salt, 0xb7), 43);
                b.name = super::super::sites::trade_name(b.func, &mut rng, t0, origin, names.iter().map(String::as_str));
                if b.name.as_ref().is_none_or(|n| !names.contains(n)) {
                    break;
                }
            }
            names.extend(b.name.clone());
        }
    }
    l
}

/// People a home of each kind (`catalog::RESIDENTIAL`: hovel, house, townhouse, tenement, noble
/// estate, farmhouse, wizard's tower) houses, counting servants, lodgers and the trades that live
/// over their shops.
const PEOPLE_PER_HOME: [u32; 7] = [4, 5, 8, 23, 14, 5, 2];

/// The people a layout's homes house (a hand town's population: `HandTown::people`).
pub fn people_of(l: &Layout) -> u32 {
    l.buildings.iter().map(people_in).sum()
}

/// The people a building houses: a home's (`PEOPLE_PER_HOME`), else none.
pub fn people_in(b: &Building) -> u32 {
    if b.func.is_some() { 0 } else { PEOPLE_PER_HOME.get(b.residential as usize).copied().unwrap_or(5) }
}

/// A hand town's people as layout `l` (its homes now) houses them: a converted town's population
/// as generated scaled by its homes now against its homes then; a blank town's per home.
pub fn count_people(town: &HandTown, l: &Layout) -> u32 {
    let now = people_of(l);
    match (town.was_people, town.was_homes) {
        (Some(p), Some(h)) if h > 0 => ((p as u64 * now as u64 + h as u64 / 2) / h as u64).min(u32::MAX as u64) as u32,
        _ => now,
    }
}

/// The size class a population makes (`t0::settle::pop_range`'s floors).
pub fn tier_of_people(people: u32) -> Tier {
    let floor = |t: Tier| crate::t0::settle::pop_range(t).0 as u32;
    [Tier::Metropolis, Tier::City, Tier::Town].into_iter().find(|&t| people >= floor(t)).unwrap_or(Tier::Village)
}

/// A hand town's population (its homes', as last counted) and the size class it makes: what its
/// label, icon and lists show at once (T0's roads and trade catch up at the next remake). None
/// for a layout not laid out by hand or not yet counted.
pub fn size(world: &World, index: usize) -> Option<(u32, Tier)> {
    let people = world.file.edits.streets.get(&index.to_string())?.get("people")?.as_u64()? as u32;
    Some((people, tier_of_people(people)))
}

/// The records of settlement `index` laid out blank (Roadmap 6, the user's call): a header (`from`
/// "blank", common lots for the blocks its streets will make) and the roads that reach it running
/// in as streets to one junction at its centre, nothing else: no blocks, no buildings. Streets
/// drawn later (`add_street`) make its blocks.
pub fn blank(world: &World, t0: &T0, index: usize) -> Result<Vec<(String, Value)>, String> {
    let s = super::super::culture::settlement(world, t0, index).ok_or_else(|| format!("layout {index}: not a settlement"))?;
    let li = index.to_string();
    let center = [s.x, s.y];
    let mut out: Vec<(String, Value)> = Vec::new();
    out.push((format!("{li}/n0"), serde_json::to_value(Junction { p: center, square: None }).map_err(|e| e.to_string())?));
    for (k, (e, class)) in super::super::ends_of(t0, index).into_iter().enumerate() {
        let p = add(e, center);
        if dist(p, center) < 20.0 {
            continue;
        }
        let n = k as u32 + 1;
        // (A road's street as wide as a generated town's main streets run, a track's narrower.)
        let w = if class >= crate::t0::roads::RoadClass::Road as u8 { 14.0 } else { 10.0 };
        out.push((format!("{li}/n{n}"), serde_json::to_value(Junction { p, square: None }).map_err(|e| e.to_string())?));
        let st = Street { a: n, b: 0, bends: Vec::new(), width: w, set_l: 0.5 * w, set_r: 0.5 * w, class: StreetClass::Main, bridge: false };
        out.push((format!("{li}/e{n}"), serde_json::to_value(st).map_err(|e| e.to_string())?));
    }
    let town = HandTown {
        from: "blank".into(),
        was: None,
        seed: hex(s.seed),
        culture: s.culture,
        fill: BlockFill::default(),
        extent: super::super::urban_radius(s.tier, s.population),
        origin: center,
        tier: s.tier as u8,
        reach: super::super::reach(&s) + 500.0,
        roofs: String::new(),
        kept: None,
        works: None,
        people: Some(0),
        was_people: None,
        was_homes: None,
        next_block: None,
    };
    out.push((li, serde_json::to_value(town).map_err(|e| e.to_string())?));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// A town's records as they would be stored (`Edits.streets`, compact JSON): its size in bytes.
pub fn size_of(records: &[(String, Value)]) -> usize {
    records.iter().map(|(k, v)| k.len() + 4 + serde_json::to_string(v).map_or(0, |s| s.len())).sum()
}

// ---------------------------------------------------------------------------------------
// Walls, gates, bridges and squares.

pub(super) const QUARTERS: [QuarterKind; 9] =
    [QuarterKind::Center, QuarterKind::Castle, QuarterKind::Docks, QuarterKind::Bridge, QuarterKind::Gate, QuarterKind::Bank, QuarterKind::Park, QuarterKind::Sprawl, QuarterKind::Regular];

pub(in crate::town) fn pk(pts: &[P]) -> String {
    pack(&pts.iter().flat_map(|p| [p[0], p[1]]).collect::<Vec<f64>>())
}

pub(in crate::town) fn unpk(s: &str) -> Vec<P> {
    if let Some(q) = s.strip_prefix('~') {
        return unpkq(q);
    }
    unpack(s).map(|f| pts_of(&f)).unwrap_or_default()
}

pub(in crate::town) fn plaza_kind(name: &str) -> Option<PlazaKind> {
    PLAZAS.iter().copied().find(|k| k.name() == name)
}

/// A hash (hex) of what a town's walls are laid from: its wall lines and its castle blocks.
pub(in crate::town) fn works_of(recs: &HandRecords) -> String {
    let mut h = crate::core::hash::Fnv64::default();
    for (id, w) in &recs.walls {
        h.write(&id.to_le_bytes());
        h.write(serde_json::to_string(w).unwrap_or_default().as_bytes());
    }
    for (id, b) in recs.blocks.iter().filter(|(_, b)| matches!(b.fill, BlockFill::Castle { .. })) {
        h.write(&id.to_le_bytes());
        h.write(serde_json::to_string(&b.fill).unwrap_or_default().as_bytes());
        h.write(b.seed.as_bytes());
        h.write(&[b.generation as u8, b.kept.is_some() as u8, 0xfe]);
    }
    hex(h.finish())
}

/// A town's works as laid out in `l` (world ft), with junction `corner[v]` where each square at a
/// junction opens.
pub(super) fn works_from(l: &Layout, of: String, corner: &BTreeMap<usize, P>) -> Works {
    use super::super::places::Spot;
    let flat = |it: &mut dyn Iterator<Item = f64>| pack(&it.collect::<Vec<f64>>());
    Works {
        of,
        walls: l.walls.iter().map(|w| pkq(w)).collect(),
        towers: pkq(&l.towers),
        gates: pk(&l.gates),
        gate_towers: pkq(&l.gate_towers),
        gate_spots: l.gate_spots.clone(),
        bastions: l.bastions.iter().map(|b| pk(b)).collect(),
        dropped: l.bastions_dropped,
        castles: flat(&mut l.castles.iter().flat_map(|c| [c.0[0], c.0[1], c.1])),
        square: flat(&mut l.square_towers.iter().flat_map(|t| [t.0[0], t.0[1], t.1[0], t.1[1]])),
        forms: forms_pk(&l.tower_forms),
        extra: flat(&mut l.extra_towers.iter().flat_map(|t| [t.0[0], t.0[1], t.1 as u8 as f64])),
        mottes: flat(&mut l.mottes.iter().flat_map(|m| [m.at[0], m.at[1], m.top_r, m.foot_r, m.top as f64, m.way[0], m.way[1]])),
        bridges: l.bridges.iter().map(|b| pkq(b)).collect(),
        roads: l.roads.iter().filter(|r| r.1 <= 2).map(|r| (r.1, r.2, pkq(&r.0))).collect(),
        corners: l
            .plazas
            .iter()
            .filter_map(|p| match p.spot {
                // (The junction's point first: the square stands while the junction does.)
                Spot::Corner(v) => corner.get(&(v as usize)).map(|j| (v, p.kind.name().to_string(), pk(&[&[*j][..], &p.pts[..]].concat()))),
                _ => None,
            })
            .collect(),
        plazas: l.plazas.iter().filter(|p| !matches!(p.spot, Spot::Patch(_) | Spot::Corner(_))).map(|p| (p.kind.name().to_string(), pk(&p.pts))).collect(),
        parks: l
            .parks
            .iter()
            .filter_map(|p| {
                let spot = match p.spot {
                    Spot::Garden(g) => format!("g{g}"),
                    Spot::Road(r) => format!("r{r}"),
                    Spot::None => "n".into(),
                    _ => return None,
                };
                let fronts: String = p.fronts.iter().map(|f| if *f { '1' } else { '0' }).collect();
                let cem = p.cemetery.map_or("-".into(), |c| format!("{}.{}.{}.{}.{}", c.group, c.rich as u8, c.city as u8, c.patch, c.by_road as u8));
                Some((format!("{} {} {:x} {spot} {cem}", p.kind.name(), if fronts.is_empty() { "-".into() } else { fronts }, p.key), pk(&p.pts)))
            })
            .collect(),
        quarters: l.quarters.iter().map(|q| (q.name.clone(), q.kind.name().to_string())).collect(),
        main: l.plaza_patch,
        piers: l.piers.iter().map(|p| pk(p)).collect(),
        moles: l.moles.iter().map(|m| (m.width, m.level, pk(&m.pts))).collect(),
        moored: flat(&mut l.moored.iter().flat_map(|m| [m.at[0], m.at[1], m.rot as f64, AFLOAT.iter().position(|k| *k == m.kind).unwrap_or(0) as f64, m.length as f64])),
        port: l.port.map(str::to_string),
    }
}

/// Why a big port has no breakwater (`harbour::breakwater`), "" when it has one.
pub(super) const PORT_WHY: [&str; 10] = [
    "",
    super::super::harbour::NO_SHORE,
    super::super::harbour::TOO_DEEP,
    "piers in the way",
    "bridges in the way",
    "buildings in the way",
    "the town wall in the way",
    "islets in the way",
    "roads, sea lanes or ferries in the way",
    "out of the town's reach",
];

pub(super) const AFLOAT: [super::super::harbour::Afloat; 4] = [super::super::harbour::Afloat::Rowboat, super::super::harbour::Afloat::FishingBoat, super::super::harbour::Afloat::Cog, super::super::harbour::Afloat::Buoy];

/// A park of no block as `works_from` wrote it down, planned again on `site` about `origin`.
pub(super) fn park_of(site: &Site, origin: P, meta: &str, pts: &str) -> Option<super::super::Park> {
    use super::super::places::Spot;
    let f: Vec<&str> = meta.split(' ').collect();
    let (kind, fronts, key, spot, cem) = (ParkKind::parse(f.first()?)?, *f.get(1)?, u64::from_str_radix(f.get(2)?, 16).ok()?, *f.get(3)?, *f.get(4)?);
    let fronts: Vec<bool> = if fronts == "-" { Vec::new() } else { fronts.bytes().map(|c| c == b'1').collect() };
    let world_pts = unpk(pts);
    let local: Vec<P> = world_pts.iter().map(|p| sub(*p, origin)).collect();
    let mut pk = super::super::park(site, &local, kind, fronts, key);
    pk.pts = world_pts;
    pk.ground.origin = add(pk.ground.origin, origin);
    pk.spot = match (spot.as_bytes().first(), spot.get(1..).and_then(|n| n.parse::<u32>().ok())) {
        (Some(b'g'), Some(n)) => Spot::Garden(n),
        (Some(b'r'), Some(n)) => Spot::Road(n),
        _ => Spot::None,
    };
    if cem != "-" {
        let c: Vec<u32> = cem.split('.').filter_map(|x| x.parse().ok()).collect();
        if c.len() == 5 {
            pk.cemetery = Some(super::super::cemetery::Style { group: c[0] as u8, rich: c[1] != 0, city: c[2] != 0, patch: c[3], by_road: c[4] != 0 });
        }
    }
    Some(pk)
}

/// Whether the line `a`-`b` runs over `poly` (crossing it, or an end in it).
pub(in crate::town) fn runs_over(poly: &[P], a: P, b: P) -> bool {
    super::super::overlaps_or_crosses(poly, &[a, b])
}

/// Where the street `c`-`d` crosses the wall's stretch `a`-`b` (its share along the stretch), if
/// it crosses it outright.
pub(super) fn crossing(a: P, b: P, c: P, d: P) -> Option<f64> {
    let (d1, d2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (d3, d4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    (d1 * d2 < 0.0 && d3 * d4 < 0.0).then(|| d3 / (d3 - d4))
}

/// The wall lines laid anew (town-local ft): each line's corners, a gate where a street crosses it
/// or one of its `gates` stands (at a corner within 30 ft, else on the stretch), towers by
/// `wall_pieces_spaced` (its record's counts per stretch while they fit, else every `spacing`
/// ft); broken at the river; a line as converted as the generator laid it (`WallLook`). Returns
/// (pieces, towers, gate towers, gates, bastions).
#[allow(clippy::type_complexity)]
pub(super) fn lay_walls(site: &Site, recs: &HandRecords, streets: &[(P, P)], origin: P, spacing: f64) -> (Vec<Vec<P>>, Vec<P>, Vec<P>, Vec<P>, Vec<Vec<P>>) {
    const SNAP: f64 = 30.0;
    let mut tmp = Layout::empty(0, [0.0, 0.0], 0.0, Tier::Town);
    let mut looked: (Vec<Vec<P>>, Vec<P>, Vec<P>, Vec<Vec<P>>) = Default::default();
    let mut gates_at: Vec<P> = Vec::new();
    for w in recs.walls.values() {
        let pts: Vec<P> = w.pts.iter().map(|p| sub(*p, origin)).collect();
        if pts.len() < 2 {
            continue;
        }
        let gates: Vec<P> = w.gates.iter().map(|p| sub(*p, origin)).collect();
        let n = pts.len();
        let stretches = if w.closed { n } else { n - 1 };
        let mut at_corner = vec![false; n];
        let mut on: Vec<Vec<f64>> = vec![Vec::new(); stretches];
        let put = |e: usize, t: f64, at_corner: &mut Vec<bool>, on: &mut Vec<Vec<f64>>| {
            let (a, b) = (pts[e], pts[(e + 1) % n]);
            let el = dist(a, b);
            if t * el < SNAP {
                at_corner[e] = true;
            } else if (1.0 - t) * el < SNAP {
                at_corner[(e + 1) % n] = true;
            } else {
                on[e].push(t);
            }
        };
        for g in &gates {
            if let Some(k) = pts.iter().position(|p| dist(*p, *g) < SNAP) {
                at_corner[k] = true;
            } else if let Some(e) = (0..stretches).find(|&e| seg_dist(*g, pts[e], pts[(e + 1) % n]) < 2.0) {
                let (a, b) = (pts[e], pts[(e + 1) % n]);
                put(e, dot(sub(*g, a), sub(b, a)) / dot(sub(b, a), sub(b, a)).max(1e-9), &mut at_corner, &mut on);
            }
        }
        for e in 0..stretches {
            let (a, b) = (pts[e], pts[(e + 1) % n]);
            for &(c, d) in streets {
                if let Some(t) = crossing(a, b, c, d) {
                    put(e, t, &mut at_corner, &mut on);
                }
            }
        }
        // (A closed ring starts at a plain corner, so no gate falls on its seam.)
        let start = if w.closed { (0..n).find(|&k| !at_corner[k]).unwrap_or(0) } else { 0 };
        let mut corners: Vec<(P, bool)> = Vec::new();
        let mut plain = true;
        for j in 0..n {
            let k = (start + j) % n;
            corners.push((pts[k], at_corner[k]));
            if k < stretches && (w.closed || j + 1 < n) {
                let mut ts = std::mem::take(&mut on[k]);
                ts.sort_by(f64::total_cmp);
                ts.dedup_by(|x, y| (*x - *y) * dist(pts[k], pts[(k + 1) % n]) < SNAP);
                plain &= ts.is_empty();
                corners.extend(ts.iter().map(|&t| (lerp(pts[k], pts[(k + 1) % n], t), true)));
            }
        }
        // (A line as converted, and crossed by no street where it has no gate: as the generator
        // laid it, curtains bowed and bastions put out by its outline.)
        let gates_rec: Vec<P> = w.gates.iter().map(|p| sub(*p, origin)).collect();
        if let Some(lk) = w.look.as_ref().filter(|lk| lk.of == line_of(w))
            && corners.iter().filter(|c| c.1).all(|c| gates_rec.iter().any(|g| dist(*g, c.0) < SNAP))
        {
            let local = |s: &str| unpk(s).into_iter().map(|p| sub(p, origin)).collect::<Vec<P>>();
            looked.0.extend(lk.walls.iter().map(|x| local(x)));
            looked.1.extend(local(&lk.towers));
            looked.2.extend(local(&lk.gate_towers));
            looked.3.extend(lk.bastions.iter().map(|x| local(x)));
            gates_at.extend(gates_rec);
            continue;
        }
        gates_at.extend(corners.iter().filter(|c| c.1).map(|c| c.0));
        let counts: Vec<usize> = w.towers.iter().map(|&t| t as usize).collect();
        let along = (plain && start == 0 && !counts.is_empty()).then_some(counts.as_slice());
        let (pieces, towers, gate_towers, _) = super::super::wall_pieces_spaced(&corners, w.closed, along, if w.spacing > 0.0 { w.spacing } else { spacing });
        tmp.walls.extend(pieces);
        tmp.towers.extend(towers);
        tmp.gate_towers.extend(gate_towers);
    }
    super::super::walls_off_river(site, &mut tmp, &[]);
    tmp.walls.extend(looked.0);
    tmp.towers.extend(looked.1);
    tmp.gate_towers.extend(looked.2);
    (tmp.walls, tmp.towers, tmp.gate_towers, gates_at, looked.3)
}

/// (Roadmap 6 P7) A castle on a block's ground `block` (local ft) of no kept buildings: its
/// curtain on the outline, gates where asked (world ft) else facing the town's middle, its keep's
/// shape and moat as asked (else rolled, keyed to the block's seed and roll; a moat within the
/// block), the keep the castle
/// and the yard's buildings the barracks, stables and smithy, as one drawn by hand
/// (`sites::castle_site`). Into `l` (local ft).
#[allow(clippy::too_many_arguments)]
pub(super) fn castle_block(site: &Site, block: &[P], (bseed, gn): (u64, i64), origin: P, gates: &[P], keep_shape: Option<&str>, moat: Option<&str>, l: &mut Layout) {
    use super::super::castle::{Asked, KeepShape, Setting};
    let gates: Vec<P> = gates.iter().take(8).map(|g| sub(*g, origin)).collect();
    let toward = gates.first().copied().unwrap_or(if contains(block, [0.0, 0.0]) { [0.0, -1000.0] } else { [0.0, 0.0] });
    let roll = |salt: i64| unit(hash3(bseed, gn, 0xca57, salt));
    let keep = KeepShape::of(keep_shape);
    let big = area(block).abs() >= 80_000.0;
    let moat = match moat {
        Some("none") => None,
        Some(k) => super::super::moat::roll(0.0, roll(2), 1.0, false).map(|m| super::super::moat::Plan { wet: Some(k == "wet"), ..m }),
        None => super::super::moat::roll(roll(1), roll(2), super::super::moat::odds(roll(3), false, true, big), false),
    };
    let set = Setting { follow: false, capital: false, wall_up: false, planned: false, inner: None, hall: false, moat };
    // (A moat within the block: the curtain drawn in so the moat's outer bank is the block's edge.)
    let inset: Vec<P>;
    let block = match moat {
        Some(m) => {
            let d = super::super::moat::BERM + m.width - super::super::castle::CURTAIN_IN;
            let pieces = super::super::clip::offset(&super::super::clip::region(block), -d, super::super::clip::Join::Miter(2.0));
            match pieces.into_iter().max_by(|a, b| a.area().total_cmp(&b.area())) {
                Some(x) if x.outer.len() >= 3 => {
                    inset = x.outer;
                    &inset[..]
                }
                _ => return,
            }
        }
        None => block,
    };
    let dp = District::roll(Ward::Castle, hash2(bseed, gn, 0xd15));
    let mut rng = Pcg32::new(hash2(bseed, gn, 0xca5), 53);
    let asked = Asked { gates, keep, drawn: Vec::new(), ..Default::default() };
    super::super::castle::castle(site, block, toward, Ward::Castle, &dp, set, &[], &[], (&[], &[]), &mut rng, l, true, true, Some(&asked));
    // The keep (pushed last) is the castle; the yard's buildings by size.
    let n = l.buildings.len();
    let kept = l.buildings.last().is_some_and(|b| b.keep.is_some() && b.func.is_none());
    if kept && let Some(fi) = catalog::index_of("castle") {
        l.buildings[n - 1].func = Some(fi as u16);
    }
    let yard = if kept { n - 1 } else { n };
    let mut by_size: Vec<usize> = (0..yard).collect();
    by_size.sort_by(|&a, &b| area(&l.buildings[b].poly).abs().total_cmp(&area(&l.buildings[a].poly).abs()).then(a.cmp(&b)));
    let trades: &[&str] = if by_size.len() >= 3 { &["barracks", "stables", "blacksmith"] } else { &["stables", "blacksmith"] };
    for (rank, &bi) in by_size.iter().enumerate() {
        let big = area(&l.buildings[bi].poly).abs() > 2500.0;
        let func = trades.get(rank).and_then(|k| catalog::index_of(k));
        let b = &mut l.buildings[bi];
        b.func = func.map(|f| f as u16);
        b.residential = catalog::residential_for(Ward::Castle, big) as u8;
        b.floors = if func.is_some() { 1 + (rank == 0) as u8 } else { 2 + (rng.next_f64() < 0.4) as u8 };
    }
}

/// A square opening at a junction laid anew (town-local ft): an octagon reaching past the
/// setbacks of the streets that meet there.
pub(super) fn junction_square(at: P, reach: f64) -> Vec<P> {
    let r = reach + 24.0;
    (0..8)
        .map(|k| {
            let a = (k as f64 + 0.5) * std::f64::consts::FRAC_PI_4;
            add(at, [r * libm::cos(a), r * libm::sin(a)])
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// The builder: a hand town laid out from its records.
