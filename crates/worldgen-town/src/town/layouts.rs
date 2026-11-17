//! Which sites hold settlements, and their layouts: building edits applied, kept parts,
//! fingerprints, prints and identities.

use super::*;
pub use crate::t0::radii::{holds_radius, pad_extent, pad_weight, reach, road_trim_radius, urban_radius};
pub use crate::t0::layout_index::{layout_count, layout_identity, rng_key, settlement};

/// Whether layout `li` is a settlement (`settlement`): the world's own or one added by hand.
pub fn is_settlement(t0: &T0, li: usize) -> bool {
    li < t0.settlements.len() || t0.made.iter().any(|m| t0.settlements.len() + m.poi == li)
}

/// The settlement site layout `li` stands in: of those whose `holds_radius` takes in its point,
/// the nearest for its size (then the lowest index). `None` for settlements, removed created
/// sites and sites out of town. From T0 alone (no layout is read).
pub fn parent_of(t0: &T0, li: usize) -> Option<usize> {
    let p = site_point(t0, li)?;
    best_holder(t0, p, (0..t0.settlements.len()).chain(made_layouts(t0)))
}

/// The layouts of the villages and towns added by hand (`t0::made`), in order.
fn made_layouts(t0: &T0) -> impl Iterator<Item = usize> + '_ {
    t0.made.iter().map(|m| t0.settlements.len() + m.poi)
}

/// Every site layout in a settlement: (site layout, settlement), by layout.
pub fn parents(t0: &T0) -> Vec<(usize, usize)> {
    let n = t0.settlements.len();
    let mut by_x: Vec<u32> = (0..n as u32).collect();
    by_x.sort_by(|&a, &b| t0.settlements[a as usize].x.total_cmp(&t0.settlements[b as usize].x).then(a.cmp(&b)));
    let widest = t0.settlements.iter().map(holds_radius).fold(0.0, f64::max);
    let mut out = Vec::new();
    for li in n..layout_count(t0) {
        let Some(p) = site_point(t0, li) else { continue };
        let from = by_x.partition_point(|&i| t0.settlements[i as usize].x < p[0] - widest);
        let to = by_x.partition_point(|&i| t0.settlements[i as usize].x <= p[0] + widest);
        if let Some(si) = best_holder(t0, p, by_x[from..to].iter().map(|&i| i as usize).chain(made_layouts(t0))) {
            out.push((li, si));
        }
    }
    out
}

/// The site layouts in settlement `si`, by layout.
pub fn sites_in(t0: &T0, si: usize) -> Vec<usize> {
    let Some(s) = settlement(t0, si) else { return Vec::new() };
    let (c, r) = ([s.x, s.y], holds_radius(s));
    let n = t0.settlements.len();
    (n..layout_count(t0)).filter(|&li| site_point(t0, li).is_some_and(|p| dist(p, c) < r) && parent_of(t0, li) == Some(si)).collect()
}

/// A site layout's point (a created site's `x`, `y`), unless it is a settlement or removed.
fn site_point(t0: &T0, li: usize) -> Option<P> {
    let k = li.checked_sub(t0.settlements.len())?;
    let p = t0.pois.get(k)?;
    (!t0.created_site(k).is_some_and(|c| c.removed) && p.kind != crate::t0::settle::PoiKind::Settlement).then_some([p.x, p.y])
}

/// Of the settlements `among` (layouts), the one holding point `p` (nearest for its size, then by index).
fn best_holder(t0: &T0, p: P, among: impl Iterator<Item = usize>) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for si in among {
        let Some(s) = settlement(t0, si) else { continue };
        let r = holds_radius(s);
        let d = dist(p, [s.x, s.y]);
        if d < r && best.is_none_or(|(b, bi)| (d / r, si) < (b, bi)) {
            best = Some((d / r, si));
        }
    }
    best.map(|b| b.1)
}

// ---------------------------------------------------------------------------------------
// Memo: layouts are pure, so each worker builds a settlement once and keeps it, and keeps
// the edited copies by a fingerprint of that layout's edits: a town laid out anew by hand
// (`Edits.towns`, `wards`), and that with its buildings edited (`Edits.buildings`).

thread_local! {
    static CACHE: RefCell<(u64, FastMap<u32, Rc<Layout>>)> = RefCell::new((0, FastMap::default()));
    static EDITED: RefCell<FastMap<u32, (u64, Rc<Layout>)>> = RefCell::new(FastMap::default());
    /// What each wall drawn by hand read when its layout was built (`WallDeps`), by layout index.
    static WALLS: RefCell<FastMap<u32, WallDeps>> = RefCell::new(FastMap::default());
    /// The islets a created site's layout read (`raised_fp`) when it was built, by layout index.
    static RAISED: RefCell<FastMap<u32, u64>> = RefCell::new(FastMap::default());
}

/// What a cached wall layout was built from: the towns it reached, the fingerprint of their
/// edits (`towns_fp`) and a hash of the lanes that can cross it (`lanes_fp`). A wall is read
/// again only when the first changes, and kept when the second still holds.
struct WallDeps {
    reached: Vec<u32>,
    towns_fp: u64,
    lanes_fp: u64,
}

/// A layout with its buildings as edited (`Edits.buildings`).
pub fn layout(world: &World, t0: &T0, index: usize) -> Rc<Layout> {
    let base = base_layout(world, t0, index);
    // (Roads drawn by hand take the world's own buildings off their band: a settlement's, not a
    // town laid out by hand's nor a building drawn by hand.)
    let roads = if world.file.edits.paths.is_empty() || !is_settlement(t0, index) || hand::is_hand(world, index) { None } else { crate::paths::fingerprint(&world.file.edits, base.bbox, 0.0) };
    if world.file.edits.buildings.is_empty() && roads.is_none() {
        return base;
    }
    let key = index as u32;
    let fp = match (edits_fingerprint(world, key), roads) {
        (Some(f), Some(r)) => crate::core::rng::hash2(f, r as i64, 0x9a75),
        (Some(f), None) => f,
        (None, Some(r)) => crate::core::rng::hash2(world.hash, r as i64, 0x9a76),
        (None, None) => return base,
    };
    let fp = match hand::fingerprint(world, t0, index).or_else(|| wards::fingerprint(world, t0, index)) {
        Some(t) => crate::core::rng::hash2(fp, t as i64, 0x70e5),
        None => fp,
    };
    // (R5 121: laid out in another culture, its buildings' edits are applied anew.)
    let fp = match culture::mark(world, t0, index) {
        0 => fp,
        c => hash2(fp, c, 0xc017),
    };
    if let Some(l) = EDITED.with(|c| c.borrow().get(&key).filter(|e| e.0 == fp).map(|e| e.1.clone())) {
        return l;
    }
    let l = Rc::new(apply_building_edits(world, t0, &base));
    EDITED.with(|c| c.borrow_mut().insert(key, (fp, l.clone())));
    l
}

/// A layout's building edits (`b:<layout>:<id>`), by id.
pub fn building_edits(world: &World, index: u32) -> impl Iterator<Item = (u32, &crate::world::BuildingEdit)> {
    let prefix = format!("b:{index}:");
    world.file.edits.buildings.range(prefix.clone()..).take_while(move |(k, _)| k.starts_with(&prefix)).filter_map(|(k, e)| Some((k.rsplit(':').next()?.parse().ok()?, e)))
}

/// A hash of the world and the layout's building edits, or `None` without any.
fn edits_fingerprint(world: &World, index: u32) -> Option<u64> {
    let mut h = crate::core::hash::Fnv64::default();
    h.write(&world.hash.to_le_bytes());
    let mut any = false;
    for (id, e) in building_edits(world, index) {
        any = true;
        h.write(&id.to_le_bytes());
        for v in e.at.iter().chain(e.poly.iter().flatten()) {
            h.write(&v.to_le_bytes());
        }
        h.write(&[e.removed as u8, e.floors.map_or(0, |f| f.saturating_add(1)), e.shade.map_or(0, |s| s + 1), 0xfe]);
        for t in [&e.func, &e.roof, &e.tint, &e.material, &e.shape, &e.structure] {
            h.write(t.as_deref().unwrap_or("-").as_bytes());
            h.write(&[0xff]);
        }
    }
    any.then(|| h.finish())
}

/// Whether the building edited still stands where it was edited (else the edit is set aside):
/// there (`edit_stands`), or it is the building generated there (`generated`: the one generated
/// with its id), cut back (a lot trimmed by a square or a close: half of it, or of what is left,
/// on it), whose outline drawn by hand is then set aside (the rest of the edit holds).
pub fn edit_applies(b: &Building, generated: Option<&Building>, e: &crate::world::BuildingEdit) -> bool {
    edit_stands(b, e) || generated.is_some_and(|g| near_edit(&g.poly, e) && overlap(&b.poly, &g.poly) >= 0.5 * area(&g.poly).abs().min(area(&b.poly).abs()))
}

/// Whether the building edited stands just where it was edited (its outline drawn by hand, if
/// any, holds).
pub fn edit_stands(b: &Building, e: &crate::world::BuildingEdit) -> bool {
    near_edit(&b.poly, e)
}

fn near_edit(poly: &[P], e: &crate::world::BuildingEdit) -> bool {
    dist(centroid(poly), e.at) <= crate::world::BuildingEdit::SAME_AT_FT
}

/// The area (sq ft) of `a` lying on convex polygon `b`.
pub(crate) fn overlap(a: &[P], b: &[P]) -> f64 {
    let mut part = a.to_vec();
    for (n, c) in edge_lines(b) {
        part = clip_plain(&part, n, c);
    }
    if part.len() < 3 { 0.0 } else { area(&part).abs() }
}

/// Whether outline `b` stands on lot `a` (world ft), either of them other than convex too: the more of
/// the two ways `overlap` measures what they share, more than a square foot.
pub(crate) fn on_lot(a: &[P], b: &[P]) -> bool {
    overlap(b, a).max(overlap(a, b)) > 1.0
}

/// `to` as `from` moved and turned (the same corners in the same order, each within half a foot of
/// where the move puts it): the move, for any point.
fn rigid(from: &[P], to: &[P]) -> Option<impl Fn(P) -> P> {
    if from.len() != to.len() || from.is_empty() {
        return None;
    }
    let n = from.len() as f64;
    let mean = |v: &[P]| mul(v.iter().fold([0.0, 0.0], |a, p| add(a, *p)), 1.0 / n);
    let (cf, ct) = (mean(from), mean(to));
    let (mut c, mut s) = (0.0, 0.0);
    for (f, t) in from.iter().zip(to) {
        let (a, b) = (sub(*f, cf), sub(*t, ct));
        c += dot(a, b);
        s += cross(a, b);
    }
    let h = crate::core::sqrt(c * c + s * s);
    let (c, s) = if h > 1e-9 { (c / h, s / h) } else { (1.0, 0.0) };
    let f = move |p: P| {
        let d = sub(p, cf);
        add(ct, [c * d[0] - s * d[1], s * d[0] + c * d[1]])
    };
    from.iter().zip(to).all(|(p, t)| dist(f(*p), *t) <= 0.5).then_some(f)
}

/// The layout with its building edits applied: buildings taken away leave the list (the others
/// keep their ids); a new function is named as a drawn building's would be (seeded by id).
fn apply_building_edits(world: &World, t0: &T0, base: &Layout) -> Layout {
    let mut l = base.clone();
    let mut removed: FastSet<u32> = FastSet::default();
    // (A town laid out by hand is its own: its buildings keep their ids by its records.)
    let made;
    let generated: &Layout = if hand::is_hand(world, l.index as usize) {
        base
    } else {
        made = generated_layout(world, t0, l.index as usize);
        &made
    };
    for (id, e) in building_edits(world, l.index) {
        let Some(i) = l.building_index(id as usize) else { continue };
        if !edit_applies(&l.buildings[i], generated.building(id as usize), e) {
            continue;
        }
        if e.removed {
            removed.insert(id);
            continue;
        }
        // (A business made by the edit is named unlike the town's others.)
        let others: Vec<String> = if e.func.is_some() { l.buildings.iter().enumerate().filter(|(j, o)| *j != i && o.func.is_some()).filter_map(|(_, o)| o.name.clone()).collect() } else { Vec::new() };
        let b = &mut l.buildings[i];
        if let Some(f) = &e.func {
            let (func, residential) = sites::func_of(f, false);
            if (func, residential) != (b.func, b.residential) {
                // (A town hall's belfry gone with its function: the hall's footprint as it was.)
                if belfry::of(b).is_some() && func != b.func {
                    b.poly = b.parts[0].poly.clone();
                    b.parts.clear();
                }
                b.func = func;
                b.residential = residential;
                let mut rng = Pcg32::new(hash2(world.stream("town.edit"), rng_key(t0, l.index as usize), id as i64), 43);
                b.name = sites::trade_name(func, &mut rng, t0, l.center, others.iter().map(String::as_str));
            }
        }
        if let Some(f) = e.floors {
            b.floors = f.max(1);
        }
        if e.poly.len() >= 3 && edit_stands(b, e) {
            // (Its lots' ground as generated no longer its own.)
            l.grounds.retain(|g| g.0 != id);
            // (Moved or turned, a keep keeps its pieces with it; drawn otherwise, it has none.)
            match rigid(&b.poly, &e.poly) {
                Some(f) => b.parts.iter_mut().flat_map(|p| p.poly.iter_mut()).for_each(|q| *q = f(*q)),
                None => b.parts.clear(),
            }
            b.poly = e.poly.clone();
        }
        sites::apply_looks(b, e.roof.as_deref(), e.tint.as_deref(), e.structure.as_deref());
        sites::apply_roof(b, e.material.as_deref(), e.shape.as_deref(), e.shade);
        if b.structure == Structure::Ruin && b.func.is_none() && e.func.is_none() {
            b.residential = catalog::RUINED as u8;
        }
        for q in &b.poly {
            l.bbox = [l.bbox[0].min(q[0]), l.bbox[1].min(q[1]), l.bbox[2].max(q[0]), l.bbox[3].max(q[1])];
        }
    }
    if !removed.is_empty() {
        l.buildings.retain(|b| !removed.contains(&b.id));
    }
    // (The world's own buildings a road drawn by hand meets: gone from its band.)
    if !world.file.edits.paths.is_empty() && is_settlement(t0, l.index as usize) && !hand::is_hand(world, l.index as usize) {
        let roads = crate::paths::near(&world.file.edits, l.bbox, 0.0);
        if !roads.is_empty() {
            l.buildings.retain(|b| !crate::paths::meets(&roads, &b.poly));
        }
    }
    l
}

/// Building edits set aside: the building at their id no longer stands where it was edited, or
/// is gone. (key, why)
pub fn set_aside(world: &World, t0: &T0) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for (k, e) in &world.file.edits.buildings {
        let mut parts = k.split(':').skip(1).map(|v| v.parse::<usize>().ok());
        let (Some(Some(li)), Some(Some(id))) = (parts.next(), parts.next()) else {
            out.push((k.clone(), "not a building id"));
            continue;
        };
        if li >= layout_count(t0) {
            out.push((k.clone(), if li >= crate::world::PARKED_LAYOUT { "set aside when the world was remade or upgraded" } else { "no such place" }));
            continue;
        }
        let base = base_layout(world, t0, li);
        // (A town laid out by hand: against itself, as `apply_building_edits` holds it.)
        let by_hand = hand::is_hand(world, li);
        match base.building(id) {
            None => out.push((k.clone(), "no such building")),
            Some(b) if !(if by_hand { edit_applies(b, Some(b), e) } else { edit_applies(b, generated_layout(world, t0, li).building(id), e) }) => out.push((k.clone(), "the town was laid out anew: another building stands at this id")),
            Some(b) if e.poly.len() >= 3 && !edit_stands(b, e) => out.push((k.clone(), "a square or close cut the building back: its outline drawn by hand is set aside")),
            _ => {}
        }
    }
    out
}

/// A layout as laid out, before its building edits: as generated, or the town laid out anew by
/// hand (`Edits.towns`). Building edits are made against it.
pub fn base_layout(world: &World, t0: &T0, index: usize) -> Rc<Layout> {
    // (A town laid out by hand: its street graph, `hand::build_hand`.)
    if let Some(l) = hand::built(world, t0, index) {
        return l;
    }
    wards::edited(world, t0, index).unwrap_or_else(|| generated_layout(world, t0, index))
}

/// A layout as generated (no edits). A wall drawn by hand is the exception: its gates follow the
/// streets of the towns it reaches as laid out (`Edits.towns`), so its cached layout is kept
/// only while those streets are where it was built.
pub fn generated_layout(world: &World, t0: &T0, index: usize) -> Rc<Layout> {
    let key = index as u32;
    if let Some(l) = CACHE.with(|c| {
        let c = c.borrow();
        if c.0 == world.hash { c.1.get(&key).cloned() } else { None }
    }) && wall_kept(world, t0, index)
        && raised_kept(world, t0, index)
        && culture_kept(world, t0, index, &l)
    {
        return l;
    }
    let n = t0.settlements.len();
    let l = Rc::new(if is_settlement(t0, index) { generate(world, t0, index) } else { sites::generate(world, t0, index - n, key) });
    let deps = wall_of(world, t0, index).map(|c| {
        let w = sites::wall_lanes(world, t0, c);
        WallDeps { towns_fp: towns_fp(world, t0, &w.reached), lanes_fp: w.fp, reached: w.reached }
    });
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != world.hash {
            c.0 = world.hash;
            c.1.clear();
            WALLS.with(|w| w.borrow_mut().clear());
            RAISED.with(|r| r.borrow_mut().clear());
        }
        c.1.insert(key, l.clone());
    });
    WALLS.with(|w| match deps {
        Some(d) => drop(w.borrow_mut().insert(key, d)),
        None => drop(w.borrow_mut().remove(&key)),
    });
    let raised = raised_fp(world, t0, index);
    RAISED.with(|r| match raised {
        Some(fp) => drop(r.borrow_mut().insert(key, fp)),
        None => drop(r.borrow_mut().remove(&key)),
    });
    l
}

/// A hash of the islets drawn by hand round a created site (its reach and what it was drawn
/// with: `town::Shore` reads them as dry land), and of the deck a drawn building stands on
/// (`Created::on`); `None` for any other layout, which reads none.
fn raised_fp(world: &World, t0: &T0, index: usize) -> Option<u64> {
    let poi = index.checked_sub(t0.settlements.len()).filter(|&p| p >= t0.base_pois && p < t0.pois.len())?;
    let p = &t0.pois[poi];
    let r = sites::SITE_REACH_FT + 50.0;
    let mut rect = [p.x - r, p.y - r, p.x + r, p.y + r];
    if let Some(c) = world.file.edits.created.get(poi - t0.base_pois) {
        for q in c.pts.iter().chain(&c.poly) {
            rect = [rect[0].min(q[0] - 50.0), rect[1].min(q[1] - 50.0), rect[2].max(q[0] + 50.0), rect[3].max(q[1] + 50.0)];
        }
    }
    let mut h = 0;
    // (R5 M20: and the deck a drawn building stands on: its level and line.)
    if let Some(on) = world.file.edits.created.get(poi - t0.base_pois).and_then(|c| c.on.as_deref()) {
        match crate::wharf::deck_of(world, t0, on) {
            Some((top, line)) => {
                h = hash2(h, top.to_bits() as i64, 0x0d3c);
                for t in [0.0, 1.0] {
                    h = hash2(h, crate::wharf::deck_level_at(world, t0, on, line[t as usize]).unwrap_or(top).to_bits() as i64, 0x0d3d);
                }
                for q in line {
                    h = hash2(h, q[0].to_bits() as i64, q[1].to_bits() as i64);
                }
            }
            None => h = hash2(h, 0x0d3c, 1),
        }
    }
    for (pts, level, _) in crate::ground::islets_in(&world.file.edits, rect) {
        h = hash2(h, level.to_bits() as i64, pts.len() as i64);
        for q in pts {
            h = hash2(h, q[0].to_bits() as i64, q[1].to_bits() as i64);
        }
    }
    // (Roadmap 6 P7: a castle drawn by hand with its moat rolled reads the ground as shaped round
    // it, `sites::shaped_level`.)
    if world.file.edits.created.get(poi - t0.base_pois).is_some_and(|c| c.kind == "castle" && c.moat.is_none()) && crate::ground::shapes(&world.file.edits) {
        h = hash2(h, crate::ground::shaped_fp(&world.file.edits, rect) as i64, 0x5a9e);
    }
    Some(h)
}

/// Whether the cached layout of `index` read the islets drawn by hand that are there now
/// (`raised_fp`; edits that change only them don't forget sites, so the cache checks).
fn raised_kept(world: &World, t0: &T0, index: usize) -> bool {
    let key = index as u32;
    let Some(was) = RAISED.with(|r| r.borrow().get(&key).copied()) else { return true };
    raised_fp(world, t0, index) == Some(was)
}

/// The wall drawn by hand that layout `index` is, if it is one.
fn wall_of<'a>(world: &'a World, t0: &T0, index: usize) -> Option<&'a crate::world::Created> {
    let c = index.checked_sub(t0.settlements.len() + t0.base_pois).and_then(|k| world.file.edits.created.get(k))?;
    (c.kind == "wall" && !c.pts.is_empty()).then_some(c)
}

/// A hash of the edits of the towns (settlement indices) a wall reaches; 0 with none.
fn towns_fp(world: &World, t0: &T0, reached: &[u32]) -> u64 {
    if world.file.edits.towns.is_empty() && world.file.edits.streets.is_empty() {
        return 0;
    }
    let mut h = 0;
    for &i in reached {
        // (A town laid out by hand by all it is built from.)
        if let Some(fp) = hand::fingerprint(world, t0, i as usize).or_else(|| wards::fingerprint(world, t0, i as usize)) {
            h = hash2(h ^ fp, i as i64, 0x3a11);
        }
    }
    h
}

/// Whether the cached layout of `index` is still what would be built: anything but a wall drawn
/// by hand always is. A wall is when the edits of the towns it reached are as they were, or
/// when they changed and the lanes that can cross the wall did not (the wall then reads the
/// same, so its layout is the same).
fn wall_kept(world: &World, t0: &T0, index: usize) -> bool {
    let key = index as u32;
    let Some((reached, was)) = WALLS.with(|w| w.borrow().get(&key).map(|d| (d.reached.clone(), d.towns_fp))) else { return true };
    let now = towns_fp(world, t0, &reached);
    if now == was {
        return true;
    }
    let Some(c) = wall_of(world, t0, index) else { return false };
    let lanes = sites::wall_lanes(world, t0, c).fp;
    WALLS.with(|w| {
        let mut w = w.borrow_mut();
        match w.get_mut(&key) {
            Some(d) if d.lanes_fp == lanes => {
                d.towns_fp = now;
                true
            }
            _ => false,
        }
    })
}

/// Whether a cached layout `l` of one of the world's own settlements is in the culture it is laid
/// out in now (`Edits.cultures`, R5 121: changed, or changed back).
fn culture_kept(world: &World, t0: &T0, index: usize, l: &Layout) -> bool {
    index >= t0.settlements.len() || culture::of(world, t0, index) == Some(l.culture)
}

/// Forget cached layouts from `index` on (`forget_from(0)`: all of them).
pub fn forget_from(index: usize) {
    CACHE.with(|c| c.borrow_mut().1.retain(|&k, _| (k as usize) < index));
    WALLS.with(|w| w.borrow_mut().retain(|&k, _| (k as usize) < index));
    RAISED.with(|r| r.borrow_mut().retain(|&k, _| (k as usize) < index));
    EDITED.with(|c| c.borrow_mut().retain(|&k, _| (k as usize) < index));
    wards::forget_from(index);
}

/// Forget the cached layouts of the created sites that changed between `old` and `new` (the
/// entries that differ, or are in one list only; `first`: the first created layout index). The
/// rest stay: no generated layout reads `Edits.created`, and a created one reads only its own
/// entry (a wall drawn by hand reads the settlements it reaches, kept by `wall_kept`).
pub fn forget_created(old: &[crate::world::Created], new: &[crate::world::Created], first: usize) {
    let mut gone: FastSet<u32> = (0..old.len().max(new.len())).filter(|&k| old.get(k) != new.get(k)).map(|k| (first + k) as u32).collect();
    if gone.is_empty() {
        return;
    }
    // A settlement added, moved or removed by hand takes its dependents with it: the walls drawn
    // by hand (they may reach it) and its edited copies (its buildings' and wards' edits, by its
    // index, above).
    // (Roadmap 6 P7) A building drawn by hand, added, moved or removed, takes the castles drawn by
    // hand round it with it: their yards make room for it (`sites::castle_site`).
    for k in (0..old.len().max(new.len())).filter(|&k| old.get(k) != new.get(k)) {
        for b in [old.get(k), new.get(k)].into_iter().flatten().filter(|b| b.kind == "building") {
            gone.extend(new.iter().enumerate().filter(|(_, c)| c.kind == "castle" && !c.removed && b.poly.iter().any(|q| (q[0] - c.x).hypot(q[1] - c.y) <= c.reach())).map(|(j, _)| (first + j) as u32));
        }
    }
    let town = |c: Option<&crate::world::Created>| c.is_some_and(|c| c.is_settlement());
    if (0..old.len().max(new.len())).any(|k| old.get(k) != new.get(k) && (town(old.get(k)) || town(new.get(k)))) {
        gone.extend(new.iter().enumerate().filter(|(_, c)| c.kind == "wall").map(|(k, _)| (first + k) as u32));
    }
    CACHE.with(|c| c.borrow_mut().1.retain(|k, _| !gone.contains(k)));
    WALLS.with(|w| w.borrow_mut().retain(|k, _| !gone.contains(k)));
    RAISED.with(|r| r.borrow_mut().retain(|k, _| !gone.contains(k)));
    EDITED.with(|c| c.borrow_mut().retain(|k, _| !gone.contains(k)));
    wards::forget(&gone);
}

/// A fingerprint of layout `index` as generated (before any edits: its buildings, districts,
/// streets, walls and ways underground), its own number left out; 0 past the last layout. Two
/// worlds' layouts with the same print and `layout_identity` are the same place inside: the app
/// keeps changes on a world's own settlement or site across a remade world only then (a road or
/// river redrawn past a town lays it out anew though it is drawn from the same seed).
pub fn layout_print(world: &World, t0: &T0, index: usize) -> u64 {
    if index >= layout_count(t0) {
        return 0;
    }
    // (In the culture it was generated in: a culture changed by hand follows its place to the
    // remade world's number, as its other edits do, so it is no part of what the place is.)
    let mut l = match culture::override_of(world, t0, index) {
        Some(_) => generate_in(world, t0, index, &t0.settlements[index], None, None),
        None => (*generated_layout(world, t0, index)).clone(),
    };
    l.index = 0;
    crate::core::hash::fnv64(format!("{l:?}").as_bytes())
}

/// Settlement and site layouts whose reach intersects the rectangle (world ft). Layout index
/// `i` is settlement `i`, then POI `i - settlements.len()`.
pub fn layouts_near(world: &World, t0: &T0, rect: [f64; 4]) -> Vec<Rc<Layout>> {
    let hit = |x: f64, y: f64, r: f64| x + r >= rect[0] && x - r <= rect[2] && y + r >= rect[1] && y - r <= rect[3];
    let n = t0.settlements.len();
    let streets = &world.file.edits.streets;
    // (A town laid out by hand reaches as far as its streets: `hand::bounds`.)
    let by_hand = |i: usize| if streets.is_empty() { None } else { hand::bounds(streets, i) };
    let towns = t0
        .settlements
        .iter()
        .enumerate()
        .filter(|&(i, s)| match by_hand(i) {
            Some(b) => b[2] >= rect[0] && b[0] <= rect[2] && b[3] >= rect[1] && b[1] <= rect[3],
            None => hit(s.x, s.y, reach(s)),
        })
        .map(|(i, _)| i);
    let pois = t0
        .pois
        .iter()
        .enumerate()
        .filter(|&(i, p)| {
            let c = t0.created_site(i);
            hit(p.x, p.y, c.map_or(sites::SITE_REACH_FT, |c| c.reach)) && !c.is_some_and(|c| c.removed)
        })
        .map(|(i, _)| n + i);
    towns.chain(pois).map(|i| layout(world, t0, i)).collect()
}

// ---------------------------------------------------------------------------------------
// Terrain context.
