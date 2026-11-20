//! Building a hand town: fingerprints, kept builds, block fills and businesses.

use super::*;

/// A hand town laid out, with each block's ground (world ft) and what could not be laid out.
pub struct HandBuilt {
    pub layout: Layout,
    pub blocks: Vec<(u32, Vec<super::super::clip::Shape>)>,
    pub problems: Problems,
}

thread_local! {
    /// Hand towns as built, by layout: the fingerprint of all they were built from (`fingerprint`).
    static BUILT: RefCell<FastMap<u32, (u64, Rc<Layout>)>> = RefCell::new(FastMap::default());
    /// Blocks as filled, by a hash of all a fill reads (`block_key`): so a change to one block
    /// fills only that one again, and what comes out never hangs on what was asked for first.
    static FILLED: RefCell<FastMap<u64, Rc<Filled>>> = RefCell::new(FastMap::default());
}

/// Whether layout `index` is a town laid out by hand (it has a header record).
pub fn is_hand(world: &World, index: usize) -> bool {
    let streets = &world.file.edits.streets;
    !streets.is_empty() && streets.contains_key(&index.to_string())
}

fn hash_value(h: &mut crate::core::hash::Fnv64, v: &Value) {
    match v {
        Value::Null => h.write(&[0]),
        Value::Bool(b) => h.write(&[1, *b as u8]),
        Value::Number(n) => {
            h.write(&[2]);
            h.write(&n.as_f64().unwrap_or(0.0).to_bits().to_le_bytes());
        }
        Value::String(t) => {
            h.write(&[3]);
            h.write(t.as_bytes());
            h.write(&[0xff]);
        }
        Value::Array(a) => {
            h.write(&[4]);
            h.write(&(a.len() as u64).to_le_bytes());
            a.iter().for_each(|x| hash_value(h, x));
        }
        Value::Object(o) => {
            // (serde_json's map is ordered by key: the same object hashes the same.)
            h.write(&[5]);
            h.write(&(o.len() as u64).to_le_bytes());
            for (k, x) in o {
                h.write(k.as_bytes());
                h.write(&[0xfe]);
                hash_value(h, x);
            }
        }
    }
}

/// The ground a hand town's blocks are laid out on: the world, the settlement (its seed, size,
/// culture as laid out) and the header's origin, reach, tier and roof seed.
fn ground_key(world: &World, t0: &T0, index: usize, town: &HandTown, reach: f64) -> u64 {
    let mut h = hash3(world.hash, super::super::layout_identity(t0, index) as i64, super::super::culture::mark(world, t0, index), 0x6a4d);
    // (And what a cemetery's look reads of the settlement.)
    if let Some(s) = super::super::settlement(t0, index) {
        h = hash3(h, super::super::wallstyle::wealth(s).to_bits() as i64, s.capital as i64, s.culture as i64);
    }
    h = hash3(h, town.origin[0].to_bits() as i64, town.origin[1].to_bits() as i64, reach.to_bits() as i64);
    hash3(h, town.tier as i64, town.culture as i64, u64::from_str_radix(&town.roofs, 16).unwrap_or(0) as i64)
}

/// The rectangle a hand town covers (world ft: x0, y0, x1, y1): its junctions' and the header's
/// extent round its origin, and a margin. No cap by its size: a town laid by hand reaches as far
/// as its streets.
pub fn bounds(streets: &BTreeMap<String, Value>, index: usize) -> Option<[f64; 4]> {
    const MARGIN: f64 = 300.0;
    let head = index.to_string();
    let h = streets.get(&head)?;
    let num = |v: Option<&Value>| v.and_then(Value::as_f64);
    let o = h.get("origin")?;
    let (ox, oy, e) = (num(o.get(0))?, num(o.get(1))?, num(h.get("extent")).unwrap_or(0.0));
    let mut b = [ox - e, oy - e, ox + e, oy + e];
    let lo = format!("{head}/n");
    for (_, v) in streets.range(lo.clone()..).take_while(|(k, _)| k.starts_with(&lo)) {
        if let (Some(x), Some(y)) = (num(v.get("p").and_then(|p| p.get(0))), num(v.get("p").and_then(|p| p.get(1)))) {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    // (And the bends of its streets.)
    let le = format!("{head}/e");
    for (_, v) in streets.range(le.clone()..).take_while(|(k, _)| k.starts_with(&le)) {
        for p in v.get("bends").and_then(Value::as_array).into_iter().flatten() {
            if let (Some(x), Some(y)) = (num(p.get(0)), num(p.get(1))) {
                b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
    }
    Some([b[0] - MARGIN, b[1] - MARGIN, b[2] + MARGIN, b[3] + MARGIN])
}

/// A hash of all a hand town is built from: its ground (`ground_key`), its records, and the
/// buildings drawn by hand and wharves within its bounds. None for a layout not laid by hand.
pub fn fingerprint(world: &World, t0: &T0, index: usize) -> Option<u64> {
    if !is_hand(world, index) {
        return None;
    }
    let edits = &world.file.edits;
    let b = bounds(&edits.streets, index)?;
    let mut h = crate::core::hash::Fnv64::default();
    h.write(&hash3(world.hash, super::super::layout_identity(t0, index) as i64, super::super::culture::mark(world, t0, index), 0x6a4e).to_le_bytes());
    let head = index.to_string();
    let lo = format!("{head}/");
    // (The header but its count of people, which the builder doesn't read.)
    let mut header = edits.streets[&head].clone();
    if let Some(o) = header.as_object_mut() {
        for k in ["people", "was_people", "was_homes"] {
            o.remove(k);
        }
    }
    for (k, v) in std::iter::once((&head, &header)).chain(edits.streets.range(lo.clone()..).take_while(|(k, _)| k.starts_with(&lo))) {
        h.write(k.as_bytes());
        hash_value(&mut h, v);
    }
    for c in edits.created.iter().filter(|c| drawn_obstacle(c, &b)) {
        h.write(c.id.as_bytes());
        c.poly.iter().for_each(|p| h.write(&[p[0].to_bits().to_le_bytes(), p[1].to_bits().to_le_bytes()].concat()));
    }
    for (k, w) in edits.wharves.iter().filter(|(_, w)| wharf_obstacle(w, &b)) {
        h.write(k.as_bytes());
        h.write(&w.width.to_bits().to_le_bytes());
        w.pts.iter().for_each(|p| h.write(&[p[0].to_bits().to_le_bytes(), p[1].to_bits().to_le_bytes()].concat()));
    }
    Some(h.finish())
}

/// Whether points `pts`, grown by `pad` ft, overlap rectangle `b` (x0, y0, x1, y1).
fn box_meets(pts: &[P], pad: f64, b: &[f64; 4]) -> bool {
    let Some(first) = pts.first() else { return false };
    let r = pts.iter().fold([first[0], first[1], first[0], first[1]], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])]);
    r[0] - pad <= b[2] && r[2] + pad >= b[0] && r[1] - pad <= b[3] && r[3] + pad >= b[1]
}

/// A building or plot drawn by hand that a hand town within `b` builds round (`build_hand`'s
/// obstacles, `fingerprint`'s: the same test, so all the builder reads is hashed).
fn drawn_obstacle(c: &crate::world::Created, b: &[f64; 4]) -> bool {
    (c.kind == "building" || (c.kind == "plot" && !c.removed)) && c.poly.len() >= 3 && box_meets(&c.poly, 4.0, b)
}

/// A wharf a hand town within `b` builds round (as `drawn_obstacle`).
fn wharf_obstacle(w: &crate::world::Wharf, b: &[f64; 4]) -> bool {
    w.pts.len() >= 2 && box_meets(&w.pts, 0.5 * w.width + 4.0, b)
}

/// Layout `index` as its hand records lay it out, if it has a header (`base_layout`; `fp`: its
/// `fingerprint`).
pub(in crate::town) fn built(world: &World, t0: &T0, index: usize, fp: u64) -> Option<Rc<Layout>> {
    let key = index as u32;
    if let Some(l) = BUILT.with(|c| c.borrow().get(&key).filter(|e| e.0 == fp).map(|e| e.1.clone())) {
        return Some(l);
    }
    let l = Rc::new(build_hand(world, t0, index)?.layout);
    BUILT.with(|c| c.borrow_mut().insert(key, (fp, l.clone())));
    Some(l)
}

/// `build_hand`, remembered as `built` would (the change function's after: the app asks for it next),
/// with what could not be laid out.
pub(in crate::town) fn build_kept(world: &World, t0: &T0, index: usize) -> Option<(Rc<Layout>, Problems)> {
    let fp = fingerprint(world, t0, index)?;
    let b = build_hand(world, t0, index)?;
    let l = Rc::new(b.layout);
    BUILT.with(|c| c.borrow_mut().insert(index as u32, (fp, l.clone())));
    Some((l, b.problems))
}

/// What one block's fill laid out (town-local ft but its kept buildings, world ft).
struct Filled {
    layout: Layout,
    kept: Vec<Building>,
    plots: Vec<(Vec<P>, u8)>,
    problems: Problems,
}

/// A hash of all a block's fill reads: the ground (`ground_key`), its record, its ground and the
/// lines it fronts.
fn block_key(ground: u64, id: u32, rec: &Value, g: &[super::super::clip::Shape], fronts: &[super::super::fill::Front]) -> u64 {
    let mut h = crate::core::hash::Fnv64::default();
    h.write(&ground.to_le_bytes());
    h.write(&id.to_le_bytes());
    hash_value(&mut h, rec);
    let mut pts = |v: &[P]| {
        h.write(&(v.len() as u64).to_le_bytes());
        v.iter().for_each(|p| h.write(&[p[0].to_bits().to_le_bytes(), p[1].to_bits().to_le_bytes()].concat()));
    };
    for sh in g {
        pts(&sh.outer);
        sh.holes.iter().for_each(|x| pts(x));
    }
    for f in fronts {
        pts(&[f.n, [f.c, 0.0], f.a, f.b]);
    }
    h.finish()
}

/// What a block is laid out as, once its ground is known.
pub fn ward_of(name: &str) -> Option<Ward> {
    WARDS.iter().copied().find(|w| w.name() == name)
}

/// A ward's usual lot area (ft²).
pub fn lot_area(w: Ward) -> f64 {
    super::super::fill::District::base(w)
}

/// Hand town `index` laid out from its records (`Edits.streets`): the street graph's faces, each
/// the block whose `at` it holds (the lowest id if several), cut back from its streets by their
/// setbacks, from painted areas and from what is drawn or built over it (buildings drawn by hand,
/// wharves); each block filled as its record says (a kept block's buildings as converted),
/// then the rows along streets. None without a header or a settlement.
pub fn build_hand(world: &World, t0: &T0, index: usize) -> Option<HandBuilt> {
    use super::super::clip::{self, Ends, Join, Shape};
    use super::super::fill::{Block as Region, Front, build_block};
    let mut recs = HandRecords::of(&world.file.edits.streets, index);
    let header = recs.town.clone()?;
    let town = &header;
    let s = super::super::culture::settlement(world, t0, index)?;
    let li = index.to_string();
    let origin = town.origin;
    let ground = KeptGround::of(world, t0, index, town);
    let tier = Tier::from_u8(town.tier);
    // (The ground read as far as its streets reach, no cap by its size: kept blocks on the ground
    // as converted, all else on this.)
    let b = bounds(&world.file.edits.streets, index)?;
    let far = [[b[0], b[1]], [b[2], b[1]], [b[0], b[3]], [b[2], b[3]]].iter().map(|p| dist(*p, origin)).fold(0.0, f64::max);
    let wide = (far > town.reach).then(|| KeptGround::at(world, t0, index, origin, far, tier, 0));
    let kept_site = &ground.site;
    let site = wide.as_ref().map_or(kept_site, |g| &g.site);
    let mut l = Layout::empty(index as u32, origin, if wide.is_some() { super::super::urban_radius(tier, s.population).max(far - 300.0) } else { super::super::urban_radius(tier, s.population) }, tier);
    l.set_roofs(world, t0, town.culture);
    l.roof_seed = u64::from_str_radix(&town.roofs, 16).unwrap_or(l.roof_seed);
    l.wall_style = super::super::wallstyle::WallStyle::of(&s);
    l.on_water = s.coastal || s.river || !site.river.is_empty();
    // (What each block's fill starts from.)
    let template = l.clone();
    let gkey = ground_key(world, t0, index, town, far.max(town.reach));
    let mut problems: Problems = Vec::new();
    let Faces { graph, faces, outers, rings, holes, mut face_block } = Faces::of(&recs, origin, &li, &mut problems);
    // A face with no block record takes the header's fill (a blank town's first blocks, before
    // the change function gives them records): numbered on from the records, in face order, and
    // seeded as the change function seeds a new block.
    let mut unrecorded: BTreeMap<u32, Value> = BTreeMap::new();
    if face_block.iter().any(Option::is_none) {
        let hseed = u64::from_str_radix(&town.seed, 16).unwrap_or(0);
        let mut next = recs.blocks.keys().next_back().map_or(0, |k| k + 1).max(town.next_block.unwrap_or(0));
        for f in 0..faces.len() {
            if face_block[f].is_some() {
                continue;
            }
            let at = add(inside_point(&rings[f]), origin);
            let b = Block { at, fill: town.fill.clone(), district: None, seed: hex(hash3(hseed, next as i64, 0xb10c, 1)), generation: 0, pins: Vec::new(), kept: None };
            unrecorded.insert(next, serde_json::to_value(&b).unwrap_or(Value::Null));
            recs.blocks.insert(next, b);
            face_block[f] = Some(next);
            next += 1;
        }
        problems.retain(|p| p.1 != "a block with no record beside this street");
    }
    let recs = recs;
    let street = |k: usize| &recs.streets[&graph.segs[k].2];
    let set = |k: usize, left: bool| if left { street(k).set_l } else { street(k).set_r };
    let ring_pts = |f: &Face| f.ring.iter().map(|&v| graph.pts[v]).collect::<Vec<P>>();

    // What no block is built over: buildings drawn by hand, wharves, painted areas.
    let pad = |poly: &[P], d: f64| clip::offset(&clip::region(poly), d, Join::Miter(2.0));
    let mut obstacles: Vec<Shape> = Vec::new();
    let tb = bounds(&world.file.edits.streets, index)?;
    for c in world.file.edits.created.iter().filter(|c| drawn_obstacle(c, &tb)) {
        let poly: Vec<P> = c.poly.iter().map(|p| sub(*p, origin)).collect();
        obstacles.extend(pad(&poly, 2.0));
    }
    for w in world.file.edits.wharves.values().filter(|w| wharf_obstacle(w, &tb)) {
        let pts: Vec<P> = w.pts.iter().map(|p| sub(*p, origin)).collect();
        obstacles.extend(clip::buffer(&pts, 0.5 * w.width + 2.0, 0.5 * w.width + 2.0, Join::Miter(2.0), Ends::Flat));
    }
    let painted: Vec<(u32, Vec<P>, OpenKind)> = recs.areas.iter().filter(|(_, a)| a.poly.len() >= 3).map(|(&k, a)| (k, a.poly.iter().map(|p| sub(*p, origin)).collect(), a.open)).collect();
    let mut cut: Vec<Shape> = obstacles.clone();
    for (_, poly, _) in &painted {
        cut.extend(clip::region(poly));
    }
    // Squares at junctions (as converted while the junction stands so, else laid anew), cut out
    // of the blocks round them.
    let works = town.works.as_ref();
    let mut corner_squares: Vec<(u32, PlazaKind, Vec<P>)> = Vec::new();
    for (&j, jr) in &recs.junctions {
        let Some(kind) = jr.square.as_deref().and_then(plaza_kind) else { continue };
        let kept = works.and_then(|w| w.corners.iter().find(|c| c.0 == j && c.1 == kind.name())).map(|c| unpk(&c.2)).filter(|v| v.len() >= 4 && v[0] == jr.p);
        let pts = match kept {
            Some(v) => v[1..].iter().map(|p| sub(*p, origin)).collect(),
            None => {
                let reach = recs.streets.values().filter(|st| st.a == j || st.b == j).map(|st| st.set_l.max(st.set_r)).fold(0.0, f64::max);
                junction_square(sub(jr.p, origin), reach)
            }
        };
        cut.extend(clip::region(&pts));
        corner_squares.push((j, kind, pts));
    }
    let bbox_of = |pts: &mut dyn Iterator<Item = P>| pts.fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let cut_boxes: Vec<[f64; 4]> = cut.iter().map(|sh| bbox_of(&mut sh.outer.iter().copied())).collect();

    // Each block's ground: its face cut back by each street's setback on its side (mitred, as a
    // generated town's patch is), less the parts inside it, painted areas and obstacles.
    let mut grounds: Vec<Vec<Shape>> = Vec::with_capacity(faces.len());
    for (f, face) in faces.iter().enumerate() {
        let ring = &rings[f];
        let d: Vec<f64> = face.side.iter().map(|&(k, left)| set(k, left)).collect();
        let simple = {
            let mut v = face.ring.clone();
            v.sort_unstable();
            v.windows(2).all(|w| w[0] != w[1])
        };
        let mut g = if simple {
            clip::inset_edges(ring, &d)
        } else {
            // (A street with the face on both sides: the face less each street's buffer.)
            let mut sub_: Vec<Shape> = Vec::new();
            for e in 0..ring.len() {
                sub_.extend(clip::buffer(&[ring[e], ring[(e + 1) % ring.len()]], d[e], 0.0, Join::Miter(2.0), Ends::Square));
            }
            clip::difference(&clip::region(ring), &sub_)
        };
        // (And each side's own strip: a mitre takes the nearer setback where two sides run straight
        // on with different ones.)
        let mut minus: Vec<Shape> = Vec::new();
        for e in 0..ring.len() {
            if d[e] > 0.0 {
                minus.extend(clip::buffer(&[ring[e], ring[(e + 1) % ring.len()]], d[e], 0.0, Join::Miter(2.0), Ends::Flat));
            }
        }
        for &h in &holes[f] {
            let o = &outers[h];
            let hr = ring_pts(o);
            minus.extend(clip::region(&hr));
            for (e, &(k, left)) in o.side.iter().enumerate() {
                minus.extend(clip::buffer(&[hr[e], hr[(e + 1) % hr.len()]], set(k, left), 0.0, Join::Miter(2.0), Ends::Square));
            }
        }
        let fb = bbox_of(&mut ring.iter().copied());
        for (sh, b) in cut.iter().zip(&cut_boxes) {
            if b[0] <= fb[2] && fb[0] <= b[2] && b[1] <= fb[3] && fb[1] <= b[3] {
                minus.push(sh.clone());
            }
        }
        if !minus.is_empty() {
            g = clip::difference(&g, &minus);
        }
        grounds.push(g);
    }

    // The streets: each side's paved strip (as a generated town paves its patches' sides), main
    // streets drawn whole too.
    let near_water = |a: P, b: P, w: f64| {
        let n = (dist(a, b) / 5.0).ceil() as usize;
        site.near_river(lerp(a, b, 0.5), 2.0) || (0..=n).any(|j| site.near_river(lerp(a, b, j as f64 / n.max(1) as f64), 0.5 * w))
    };
    // (Paved on the sides of blocks built on or laid out as parks, as a generated town paves its
    // patches but squares and farmland.)
    let paves = |b: &Block| match &b.fill {
        BlockFill::Generated { .. } | BlockFill::Castle { .. } => true,
        BlockFill::Open { open, .. } => matches!(open, OpenKind::Park | OpenKind::Garden | OpenKind::Cemetery),
        BlockFill::Hand => false,
    };
    let mut paved_side: crate::core::hash::FastSet<(usize, bool)> = Default::default();
    // (And the sides of blocks as converted, not laid out again since: a street between them
    // stays as the generator left it, a deck or none.)
    let (mut sided, mut as_converted): (crate::core::hash::FastSet<(usize, bool)>, crate::core::hash::FastSet<(usize, bool)>) = Default::default();
    let untouched = |b: &Block| b.generation == 0 && (b.kept.is_some() || !matches!(b.fill, BlockFill::Generated { .. } | BlockFill::Castle { .. }));
    for (f, face) in faces.iter().enumerate() {
        if face_block[f].is_some_and(|id| paves(&recs.blocks[&id])) {
            paved_side.extend(face.side.iter().copied());
        }
        if let Some(id) = face_block[f] {
            sided.extend(face.side.iter().copied());
            if town.works.is_some() && untouched(&recs.blocks[&id]) {
                as_converted.extend(face.side.iter().copied());
            }
        }
    }
    let mut lines: BTreeMap<u32, Vec<P>> = BTreeMap::new();
    for (k, &(u, v, id)) in graph.segs.iter().enumerate() {
        let line = lines.entry(id).or_insert_with(|| vec![graph.pts[u]]);
        line.push(graph.pts[v]);
        let st = &recs.streets[&id];
        let (a, b) = (graph.pts[u], graph.pts[v]);
        let el = dist(a, b);
        if st.class == StreetClass::Joint || el < 1.0 {
            continue;
        }
        let ev = mul(sub(b, a), 1.0 / el);
        for (d, left) in [(st.set_l, true), (st.set_r, false)] {
            if d <= 0.0 || !paved_side.contains(&(k, left)) {
                continue;
            }
            let o = mul(if left { [-ev[1], ev[0]] } else { [ev[1], -ev[0]] }, 0.5 * d);
            let (p, q) = (add(a, o), add(b, o));
            if !near_water(p, q, d) {
                l.roads.push((vec![p, q], 3, d + 1.0));
            }
        }
    }
    for (id, line) in &lines {
        if recs.streets[id].class == StreetClass::Main {
            l.streets.push(line.clone());
            l.roads.push((line.clone(), 4, recs.streets[id].width.clamp(8.0, 18.0)));
        }
    }
    // (Dead ends, with no block beside them to show where they run: each paved as a main street
    // is, whatever its class, but a joint's.)
    let mut loose: BTreeMap<u32, Vec<P>> = BTreeMap::new();
    for &(u, v, id) in &graph.loose {
        loose.entry(id).or_insert_with(|| vec![graph.pts[u]]).push(graph.pts[v]);
    }
    for (id, line) in &loose {
        let st = &recs.streets[id];
        if st.class == StreetClass::Joint {
            continue;
        }
        if st.class == StreetClass::Main {
            l.streets.push(line.clone());
        }
        l.roads.push((line.clone(), 4, st.width.clamp(8.0, 18.0)));
    }

    // Bridges: a deck wherever a street runs over the river, set or not (as converted where a
    // street or a road in from the country still runs over one), as a generated town spans it.
    let local = |v: Vec<P>| v.into_iter().map(|p| sub(p, origin)).collect::<Vec<P>>();
    let kept_decks: Vec<Vec<P>> = works.map(|w| w.bridges.iter().map(|b| local(unpk(b))).collect()).unwrap_or_default();
    let approaches: Vec<(u8, f64, Vec<P>)> = works.map(|w| w.roads.iter().map(|r| (r.0, r.1, local(unpk(&r.2)))).collect()).unwrap_or_default();
    let mut deck_kept = vec![false; kept_decks.len()];
    for (k, d) in kept_decks.iter().enumerate() {
        deck_kept[k] = approaches.iter().any(|r| r.2.windows(2).any(|s| runs_over(d, s[0], s[1])));
    }
    let mut decks: Vec<Vec<P>> = Vec::new();
    for (k, &(u, v, id)) in graph.segs.iter().enumerate() {
        let st = &recs.streets[&id];
        let (a, b) = (graph.pts[u], graph.pts[v]);
        let el = dist(a, b);
        // (A street, not the edge of farmland or of a square: a block beside it built on.)
        if st.class == StreetClass::Joint || el < 1.0 || !(st.class == StreetClass::Main || paved_side.contains(&(k, true)) || paved_side.contains(&(k, false))) {
            continue;
        }
        let mut over = false;
        for (k, d) in kept_decks.iter().enumerate() {
            if runs_over(d, a, b) {
                deck_kept[k] = true;
                over = true;
            }
        }
        let n = (el / 5.0).ceil() as usize;
        let wet = |p: P| site.near_river(p, 0.0);
        let kept_side = [(k, true), (k, false)].iter().filter(|s| sided.contains(s)).all(|s| as_converted.contains(s)) && (sided.contains(&(k, true)) || sided.contains(&(k, false)));
        if over || kept_side || site.river.is_empty() || wet(a) || wet(b) || !(1..n).any(|j| wet(lerp(a, b, j as f64 / n as f64))) {
            continue;
        }
        let m = super::super::river_crossing(site, a, b);
        let ud = mul(sub(b, a), 1.0 / el);
        let (fwd, back) = (site.dry_reach(m, ud, 0.0, 1.0, 0.0) + 14.0, site.dry_reach(m, ud, 0.0, -1.0, 0.0) + 14.0);
        // (Far wider than the river: the street runs along the water, not over it.)
        let hw = site.nearest_river(m).map_or(0.0, |r| r.2);
        // (Or no water across it to speak of.)
        if fwd + back > 4.0 * hw + 80.0 || fwd + back < 28.0 + 2.0 {
            continue;
        }
        let width = if tier >= Tier::City { 24.0 } else if st.class == StreetClass::Main { 16.0 } else { 12.0 };
        let (fwd, back) = (site.dry_reach(m, ud, width, 1.0, fwd - 14.0) + 14.0, site.dry_reach(m, ud, width, -1.0, back - 14.0) + 14.0);
        decks.push(super::super::rect(add(m, mul(ud, 0.5 * (fwd - back))), ud, fwd + back, width));
        if st.class != StreetClass::Main {
            l.roads.push((vec![a, m, b], 4, 12.0));
        }
    }
    let decks: Vec<Vec<P>> = kept_decks.into_iter().zip(&deck_kept).filter(|x| *x.1).map(|x| x.0).chain(decks).collect();
    l.bridges = decks;
    for (class, w, pts) in approaches {
        l.roads.push((pts, class, w));
    }

    // The blocks' fills.
    let meta = (index as u32, origin, 0.0, tier);
    let mut kept_bs: Vec<Building> = Vec::new();
    let mut plots: Vec<(Vec<P>, u8)> = Vec::new();
    let mut built_ground: Vec<Shape> = Vec::new();
    let cem = |id: u32| super::super::cemetery::Style::of(&s, id as usize, false);
    let mut out: Vec<(u32, Vec<Shape>)> = Vec::new();
    let mut named: Vec<(u32, String, Vec<P>)> = Vec::new();
    let seed_of = |hexs: &str| u64::from_str_radix(hexs, 16).unwrap_or_else(|_| hash2(0x4a4d, index as i64, 0));
    let shift = |sh: &[Shape]| -> Vec<Shape> {
        sh.iter().map(|x| Shape { outer: x.outer.iter().map(|p| add(*p, origin)).collect(), holes: x.holes.iter().map(|h| h.iter().map(|p| add(*p, origin)).collect()).collect() }).collect()
    };
    // (Every id pinned in the town, by block: no block's fresh ids take one pinned in another.)
    // (The castles built on blocks, their curtains, towers and moats put in after the walls.)
    let mut castles: Vec<Rc<Filled>> = Vec::new();
    let pinned_all: Vec<(u32, u32)> = recs.blocks.iter().flat_map(|(&b, r)| r.pins.iter().map(move |p| (b, p.id))).collect();
    for (f, face) in faces.iter().enumerate() {
        let Some(id) = face_block[f] else { continue };
        let rec = &recs.blocks[&id];
        let g = &grounds[f];
        out.push((id, shift(g)));
        if let Some(d) = &rec.district {
            named.push((id, d.clone(), rings[f].clone()));
        }
        // (What a row keeps clear of: every block but one left for buildings drawn by hand.)
        if !matches!(rec.fill, BlockFill::Hand) {
            built_ground.extend(g.iter().cloned());
        }
        let spot = super::super::places::Spot::Patch(id);
        let bseed = seed_of(&rec.seed);
        let gn = rec.generation as i64;
        let ring = &rings[f];
        // (Its streets' lines, outward, where a lot may front them.)
        let fronts: Vec<Front> = face
            .side
            .iter()
            .enumerate()
            .filter_map(|(e, &(k, left))| {
                let d = set(k, left);
                let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
                let el = dist(a, b);
                (d > 0.0 && el > 1e-9 && street(k).class != StreetClass::Joint).then(|| {
                    let n = [(b[1] - a[1]) / el, -(b[0] - a[0]) / el];
                    Front { n, c: dot(a, n) - d, a: sub(a, mul(n, d)), b: sub(b, mul(n, d)) }
                })
            })
            .collect();
        let biggest = || g.iter().max_by(|x, y| x.area().total_cmp(&y.area())).map(|x| x.outer.clone());
        let built_on = !matches!(rec.fill, BlockFill::Hand | BlockFill::Open { .. });
        // (Filled once for all it reads: the same block comes out the same whatever else changed.)
        let base = HAND_ID as u64 + ((id as u64 * ROLLS as u64 + gn as u64 % ROLLS as u64) * PER_PATCH as u64);
        let foreign: Vec<u32> = pinned_all.iter().filter(|x| x.0 != id && (x.1 as u64) >= base && (x.1 as u64) < base + PER_PATCH as u64).map(|x| x.1).collect();
        let mut key = block_key(gkey, id, world.file.edits.streets.get(&format!("{li}/k{id}")).or_else(|| unrecorded.get(&id)).unwrap_or(&Value::Null), g, &fronts);
        for &p in &foreign {
            key = hash2(key, p as i64, 0x4e1d);
        }
        let filled = FILLED.with(|c| c.borrow().get(&key).cloned()).unwrap_or_else(|| {
            let mut l = template.clone();
            let mut kept_bs: Vec<Building> = Vec::new();
            let mut plots: Vec<(Vec<P>, u8)> = Vec::new();
            let mut problems: Problems = Vec::new();
            let first = 0;
            let mut pinned: Vec<Option<u32>> = Vec::new();
            'fill: {
                if built_on {
                    l.districts.push(ring.clone());
                }
                if let Some(kept) = &rec.kept {
                    // (Its fill laid out once: its buildings to keep, its open ground, its plots.)
                    let laid = kept.refill.as_ref().map(|r| r.lay(kept_site, meta, Some(cem(id))));
                    let raw = match &laid {
                        Some(Ok((lay, _))) if kept.refill.as_ref().is_some_and(|r| r.k < 2) => lay.buildings.as_slice(),
                        _ => &[],
                    };
                    match kept_from(&ground, kept, raw) {
                        Ok(bs) => kept_bs.extend(bs),
                        Err(_) => problems.push((format!("{li}/k{id}"), "its kept buildings could not be laid out again")),
                    }
                    if let (Some(r), Some(laid)) = (&kept.refill, laid) {
                        match laid {
                            Ok((mut lay, pl)) => {
                                if r.k == 0 {
                                    l.blocks.push(r.block().unwrap_or_default());
                                }
                                let mask = kept.fields.as_bytes();
                                plots.extend(pl.into_iter().enumerate().filter(|(k, _)| mask.get(*k).is_none_or(|c| *c == b'1')).map(|x| x.1));
                                lay.plazas.iter_mut().for_each(|p| p.spot = spot);
                                lay.parks.iter_mut().for_each(|p| p.spot = spot);
                                l.plazas.append(&mut lay.plazas);
                                l.parks.append(&mut lay.parks);
                                l.monuments.append(&mut lay.monuments);
                                l.roads.append(&mut lay.roads);
                            }
                            Err(_) => problems.push((format!("{li}/k{id}"), "its kept fill could not be laid out again")),
                        }
                    } else if built_on {
                        l.blocks.extend(g.iter().map(|x| x.outer.clone()));
                    }
                    break 'fill;
                }
                if g.is_empty() {
                    break 'fill;
                }
                match &rec.fill {
                    BlockFill::Hand => {}
                    BlockFill::Castle { gates, keep_shape, moat, .. } => {
                        if let Some(b) = biggest() {
                            castle_block(site, &b, (bseed, gn), origin, gates, keep_shape.as_deref(), moat.as_deref(), &mut l);
                        }
                    }
                    BlockFill::Generated { ward, lots, height, reroll, close } => {
                        let Some(w) = ward_of(ward) else {
                            problems.push((format!("{li}/k{id}"), "no such ward"));
                            break 'fill;
                        };
                        let salt = |h: u64| if *reroll == 0 { h } else { hash3(h, *reroll as i64, 0x7e70, 1) };
                        let mut dp = District::roll(w, salt(hash2(bseed, gn, 0xd15)));
                        if let Some(k) = lots.as_deref().and_then(|n| LOT_SIZES.iter().position(|x| *x == n)) {
                            dp.min_sq = District::base(w) * super::super::wards::LOT_FACTOR[k];
                        }
                        super::super::docks_on_slope(&mut dp, w, if ring.len() >= 3 { site.grade(ring) } else { 0.0 });
                        let mut rng = Pcg32::new(salt(hash2(bseed, gn, 0x107)), 47);
                        l.blocks.extend(g.iter().map(|x| x.outer.clone()));
                        let alleys = l.roads.len();
                        build_block(site, Region::Region(g), &fronts, w, &dp, &mut rng, &mut l, None);
                        // A temple's close round its temple on its largest piece (where it fits,
                        // unless asked for none), else a temple alone where no lot was cut.
                        if w == Ward::Temple
                            && let Some(b) = biggest()
                        {
                            let lines: Vec<(P, f64)> = fronts.iter().map(|f| (f.n, f.c)).collect();
                            let t = unit(hash2(bseed, gn, 65));
                            let laid = *close != Some(false) && l.buildings.len() > first && super::super::squares::close(site, &b, &lines, t, first, alleys, &mut l);
                            if !laid && *close == Some(true) {
                                problems.push((format!("{li}/k{id}"), "no room for a close round its temple"));
                            }
                            if l.buildings.len() == first {
                                super::super::squares::lone_temple(site, &b, t, &mut l);
                            }
                        }
                        pinned = pin_on(&mut l, first, &rec.pins, origin);
                        home_storeys(&mut l, first, w, tier, hash2(salt(bseed), gn, 0x5707), &pinned);
                        businesses(&mut l, first, w, tier, s.coastal || s.river || !site.river.is_empty(), hash2(salt(bseed), gn, 0xb05), &pinned);
                        height_storeys(&mut l, first, height.as_deref(), &pinned);
                    }
                    BlockFill::Open { open, style } => {
                        // (Every piece of its ground, the largest first: each but the first with
                        // looks of its own.)
                        let mut pieces: Vec<(f64, usize)> = g.iter().enumerate().map(|(k, x)| (x.area(), k)).filter(|x| x.0 >= 200.0).collect();
                        pieces.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
                        for (pi, &(_, k)) in pieces.iter().enumerate() {
                            let salt = |h: u64| if pi == 0 { h } else { hash2(h, pi as i64, 0x91ec) };
                            let roll = |j: i64| unit(salt(hash3(bseed, gn, j, 0x09e)));
                            let fill = open_fill(*open, style.as_deref(), &fronts, salt(hash2(bseed, gn, 0x9a4c)), || cem(id), &mut || Pcg32::new(salt(hash2(bseed, gn, 0xfa)), 61), tier);
                            let mut pl = Vec::new();
                            let (pz, pk) = (l.plazas.len(), l.parks.len());
                            super::super::fill_block(site, &g[k].outer, Vec::new(), fill, &roll, spot, &mut l, &mut pl);
                            l.plazas[pz..].iter_mut().for_each(|p| p.spot = spot);
                            l.parks[pk..].iter_mut().for_each(|p| p.spot = spot);
                            plots.append(&mut pl);
                        }
                    }
                }
                // (Its own ids, by the order it laid them out, but those pinned: those keep theirs,
                // and no other takes one of them, nor one pinned anywhere else in the town. Past
                // its range (PER_PATCH ids, blocks under ROW_BLOCK) the rest are left out.)
                let held: Vec<u32> = pinned.iter().flatten().copied().chain(foreign.iter().copied()).collect();
                let end = if id < ROW_BLOCK { base + PER_PATCH as u64 } else { base };
                let mut next = base;
                let mut keep = vec![true; l.buildings.len() - first];
                for (k, b) in l.buildings[first..].iter_mut().enumerate() {
                    if let Some(Some(pid)) = pinned.get(k) {
                        b.id = *pid;
                        continue;
                    }
                    while next < end && held.contains(&(next as u32)) {
                        next += 1;
                    }
                    if next >= end {
                        keep[k] = false;
                        continue;
                    }
                    b.id = next as u32;
                    next += 1;
                }
                if keep.contains(&false) {
                    let mut k = 0;
                    l.buildings.retain(|_| {
                        k += 1;
                        k <= first || keep[k - 1 - first]
                    });
                    problems.push((format!("{li}/k{id}"), if id >= ROW_BLOCK { "block number too high for its ids" } else { "more buildings than a block has ids for" }));
                }
            }
            let fl = Rc::new(Filled { layout: l, kept: kept_bs, plots, problems });
            FILLED.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() >= 40_000 {
                    c.clear();
                }
                c.insert(key, fl.clone());
            });
            fl
        });
        let fl = &filled.layout;
        if matches!(rec.fill, BlockFill::Castle { .. }) && rec.kept.is_none() {
            castles.push(filled.clone());
        }
        l.buildings.extend(fl.buildings.iter().cloned());
        l.plazas.extend(fl.plazas.iter().cloned());
        l.parks.extend(fl.parks.iter().cloned());
        l.monuments.extend(fl.monuments.iter().cloned());
        l.roads.extend(fl.roads.iter().cloned());
        l.blocks.extend(fl.blocks.iter().cloned());
        l.districts.extend(fl.districts.iter().cloned());
        l.crofts.extend(fl.crofts.iter().cloned());
        l.grounds.extend(fl.grounds.iter().cloned());
        kept_bs.extend(filled.kept.iter().cloned());
        plots.extend(filled.plots.iter().cloned());
        problems.extend(filled.problems.iter().cloned());
    }
    // Painted areas: open ground of their kind.
    for (k, poly, open) in &painted {
        let key = hash2(seed_of(&town.seed), *k as i64, 0xa4ea);
        let fill = open_fill(*open, None, &[], key, || cem(*k + ROW_BLOCK), &mut || Pcg32::new(key, 61), tier);
        let mut pl = Vec::new();
        super::super::fill_block(site, poly, Vec::new(), fill, &|j| unit(hash2(key, j, 3)), super::super::places::Spot::None, &mut l, &mut pl);
        plots.append(&mut pl);
    }
    // Rows of lots along streets, each clear of what stands already.
    let mut avoid: Vec<Shape> = built_ground;
    avoid.extend(obstacles.iter().cloned());
    avoid.extend(painted.iter().flat_map(|a| clip::region(&a.1)));
    avoid.extend(corner_squares.iter().flat_map(|c| clip::region(&c.2)));
    // (And what stands in no block: kept by the header.)
    let loose = match &town.kept {
        Some(k) => kept_on(&ground, k).map_err(|_| problems.push((li.clone(), "its kept buildings could not be laid out again"))).unwrap_or_default(),
        None => Vec::new(),
    };
    for b in &loose {
        avoid.extend(clip::region(&b.poly.iter().map(|p| sub(*p, origin)).collect::<Vec<P>>()));
    }
    // (Every street's way, a dead end's too: a street out of a gate.)
    let mut dead: BTreeMap<u32, Vec<P>> = BTreeMap::new();
    for &(u, v, id) in graph.loose.iter().filter(|g| recs.streets[&g.2].class != StreetClass::Joint) {
        dead.entry(id).or_insert_with(|| vec![graph.pts[u]]).push(graph.pts[v]);
    }
    for (id, line) in lines.iter().chain(&dead) {
        let w = recs.streets[id].width;
        avoid.extend(clip::buffer(line, 0.5 * w, 0.5 * w, Join::Miter(2.0), Ends::Flat));
    }
    for (&r, row) in &recs.rows {
        let (Some(line), Some(w)) = (lines.get(&row.street), ward_of(&row.ward)) else {
            problems.push((format!("{li}/r{r}"), "no such street or ward"));
            continue;
        };
        if r >= MAX_ROWS {
            problems.push((format!("{li}/r{r}"), "row number too high for its ids"));
            continue;
        }
        let rseed = seed_of(&row.seed);
        let mut dp = District::roll(w, hash2(rseed, r as i64, 0xd15));
        if let Some(k) = row.lots.as_deref().and_then(|n| LOT_SIZES.iter().position(|x| *x == n)) {
            dp.min_sq = District::base(w) * super::super::wards::LOT_FACTOR[k];
        }
        let mut rng = Pcg32::new(hash2(rseed, r as i64, 0x107), 1000 + r as u64);
        let side = super::super::rows::Side { line, left: row.side == "l", setback: 0.5 * recs.streets[&row.street].width + 2.0, depth: row.depth, gaps: row.gaps.clamp(0.0, 0.9) };
        let first = l.buildings.len();
        let lots = super::super::rows::build_row(site, &side, w, &dp, &mut rng, &avoid, &mut l);
        l.blocks.extend(super::super::rows::bands(&side));
        for lot in &lots {
            avoid.extend(clip::region(lot));
        }
        let base = HAND_ID as u64 + ((ROW_BLOCK + r) as u64 * ROLLS as u64) * PER_PATCH as u64;
        // (A row's ids run over all ROLLS of its number; past them the rest are left out.)
        let room = (ROLLS * PER_PATCH) as usize;
        if l.buildings.len() - first > room {
            l.buildings.truncate(first + room);
            problems.push((format!("{li}/r{r}"), "more buildings than a row has ids for"));
        }
        for (k, b) in l.buildings[first..].iter_mut().enumerate() {
            b.id = (base + k as u64) as u32;
        }
    }
    // Farmland's plots, off the sand: the town's fields.
    l.fields.extend(plots.into_iter().filter_map(|(f, _)| super::super::fields::off_sand(site, &f)));

    // Local → world, then what was kept (world ft already).
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
    l.buildings.append(&mut kept_bs);
    l.buildings.extend(loose);
    l.buildings.sort_by_key(|b| b.id);
    // Businesses laid out anew named unlike any other in the town, each by its id.
    {
        let mut names: std::collections::BTreeSet<String> = l.buildings.iter().filter(|b| b.func.is_some()).filter_map(|b| b.name.clone()).collect();
        let tseed = seed_of(&town.seed);
        for b in l.buildings.iter_mut().filter(|b| b.id >= HAND_ID && b.func.is_some() && b.name.is_none()) {
            for salt in 0..8 {
                let mut rng = Pcg32::new(hash3(tseed, b.id as i64, 0x4e + salt, 0xb7), 43);
                b.name = super::super::sites::trade_name(b.func, &mut rng, t0, origin, names.iter().map(String::as_str));
                if b.name.as_ref().is_none_or(|n| !names.contains(n)) {
                    break;
                }
            }
            names.extend(b.name.clone());
        }
    }
    super::super::fields::assign(site, s.seed, &mut l);

    // Walls, towers and gates: as converted while the wall lines and castle blocks are as they
    // were, else laid anew from the wall lines (castles' curtains then go with them).
    let wkey = works_of(&recs);
    match works.filter(|w| w.of == wkey) {
        Some(w) => {
            let nums = |s: &str| unpack(s).unwrap_or_default();
            l.walls = w.walls.iter().map(|x| unpk(x)).collect();
            l.towers = unpk(&w.towers);
            l.gates = unpk(&w.gates);
            l.gate_towers = unpk(&w.gate_towers);
            l.gate_spots = w.gate_spots.clone();
            l.bastions = w.bastions.iter().map(|x| unpk(x)).collect();
            l.bastions_dropped = w.dropped;
            l.castles = nums(&w.castles).chunks_exact(3).map(|c| ([c[0], c[1]], c[2])).collect();
            l.square_towers = nums(&w.square).chunks_exact(4).map(|c| ([c[0], c[1]], [c[2], c[3]])).collect();
            l.tower_forms = forms_unpk(&w.forms);
            l.extra_towers = nums(&w.extra).chunks_exact(3).map(|c| ([c[0], c[1]], c[2] != 0.0)).collect();
            l.mottes = nums(&w.mottes).chunks_exact(7).map(|c| super::super::Motte { at: [c[0], c[1]], top_r: c[2], foot_r: c[3], top: c[4] as f32, way: [c[5], c[6]] }).collect();
        }
        None => {
            // (Dead ends too: a street out of a gate to the country has no block beyond the wall.)
            let segs: Vec<(P, P)> = graph.segs.iter().chain(&graph.loose).filter(|g| recs.streets[&g.2].class != StreetClass::Joint).map(|g| (graph.pts[g.0], graph.pts[g.1])).collect();
            let (pieces, towers, gate_towers, gates, bastions) = lay_walls(site, &recs, &segs, origin, l.wall_style.spacing);
            let w = |v: Vec<P>| v.into_iter().map(|p| add(p, origin)).collect::<Vec<P>>();
            l.walls = pieces.into_iter().map(w).collect();
            l.bastions = bastions.into_iter().map(w).collect();
            l.towers = w(towers);
            l.gate_towers = w(gate_towers);
            l.gates = w(gates);
            // (Castles kept as converted: their own works.)
            for c in recs.blocks.values().filter(|b| matches!(b.fill, BlockFill::Castle { .. })).filter_map(|b| b.kept.as_ref()?.curtain.as_ref()) {
                c.lay(&mut l);
            }
            let (style, laid) = (l.wall_style, l.towers.clone());
            super::super::wallstyle::shape_towers(&mut l, &style, &laid, origin, false);
        }
    }
    // The castles built on blocks: their curtains, towers, mottes and moats (local ft → world).
    for fl in &castles {
        let (c, w) = (&fl.layout, |p: &P| add(*p, origin));
        l.walls.extend(c.walls.iter().map(|x| x.iter().map(w).collect()));
        l.towers.extend(c.towers.iter().map(w));
        l.gate_towers.extend(c.gate_towers.iter().map(w));
        l.castles.extend(c.castles.iter().map(|x| (w(&x.0), x.1)));
        l.square_towers.extend(c.square_towers.iter().map(|x| (w(&x.0), x.1)));
        l.tower_forms.extend(c.tower_forms.iter().map(|x| (w(&x.0), x.1, x.2)));
        l.mottes.extend(c.mottes.iter().map(|m| super::super::Motte { at: w(&m.at), way: w(&m.way), ..*m }));
        l.moats.extend(c.moats.iter().cloned().map(|mut m| {
            m.shift(origin);
            m
        }));
    }
    if !castles.is_empty() {
        super::super::wallstyle::shape_rest(&mut l, origin);
    }
    // Squares at junctions, and those of no block as converted (quays).
    for (j, kind, pts) in corner_squares {
        l.plazas.push(super::super::Plaza { pts: pts.into_iter().map(|p| add(p, origin)).collect(), kind, spot: super::super::places::Spot::Corner(j) });
    }
    for (kind, pts) in works.map(|w| w.plazas.as_slice()).unwrap_or_default() {
        if let Some(kind) = plaza_kind(kind) {
            l.plazas.push(super::super::Plaza { pts: unpk(pts), kind, spot: super::super::places::Spot::None });
        }
    }
    for (meta, pts) in works.map(|w| w.parks.as_slice()).unwrap_or_default() {
        l.parks.extend(park_of(kept_site, origin, meta, pts));
    }
    // The main square, and the harbour as generated.
    if let Some(w) = works {
        l.plaza_patch = w.main;
        l.piers = w.piers.iter().map(|p| unpk(p)).filter(|p| p.len() == 4).collect();
        l.moles = w.moles.iter().map(|m| super::super::harbour::Mole { pts: unpk(&m.2), width: m.0, level: m.1 }).filter(|m| m.pts.len() >= 2).collect();
        l.quays = w.quays.iter().map(|q| super::super::harbour::Quay { pts: unpk(&q.2), level: q.0, timber: q.1 }).filter(|q| q.pts.len() == 4).collect();
        l.moored = unpack(&w.moored)
            .unwrap_or_default()
            .chunks_exact(5)
            .map(|c| super::super::harbour::Moored { at: [c[0], c[1]], rot: c[2] as f32, kind: AFLOAT[(c[3] as usize).min(3)], length: c[4] as f32 })
            .collect();
        l.port = w.port.as_deref().map(|why| PORT_WHY.iter().copied().find(|w| *w == why).unwrap_or(super::super::harbour::NO_SHORE));
    }
    // Districts: the blocks of each name (in the order of their lowest block), labelled along them.
    named.sort_by_key(|x| x.0);
    let mut by_name: BTreeMap<String, Vec<Vec<P>>> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    for (_, name, ring) in named {
        if !by_name.contains_key(&name) {
            names.push(name.clone());
        }
        by_name.entry(name).or_default().push(ring);
    }
    for name in names {
        let patches = by_name.remove(&name).unwrap_or_default();
        let label = super::super::label_curve(&patches).0;
        let kind = works.and_then(|w| w.quarters.iter().find(|q| q.0 == name)).and_then(|q| QUARTERS.iter().copied().find(|k| k.name() == q.1)).unwrap_or(QuarterKind::Regular);
        let w = |v: Vec<P>| v.into_iter().map(|p| add(p, origin)).collect::<Vec<P>>();
        l.quarters.push(super::super::Quarter { name, kind, patches: patches.into_iter().map(w).collect(), label: w(label) });
    }
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in l.buildings.iter().flat_map(|b| b.poly.iter()).chain(l.walls.iter().flatten()).chain(l.fields.iter().flatten()).chain(l.plazas.iter().flat_map(|p| p.pts.iter())).chain(l.parks.iter().flat_map(|p| p.pts.iter())).chain(l.districts.iter().flatten()) {
        bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
    }
    l.bbox = if bb[0] <= bb[2] { bb } else { [origin[0], origin[1], origin[0], origin[1]] };
    // Sewers and catacombs under cities, from its streets as laid by hand.
    if tier >= Tier::City {
        l.entrances = crate::under::city_entrances(&l, t0, s.seed);
    }
    l.number_entrances();
    Some(HandBuilt { layout: l, blocks: out, problems })
}

/// Pins on a block laid out anew (`Block::pins`, world ft; `l` local about `origin`): the building
/// standing on each pin's point takes its id, and its business with the name it had (each building
/// one pin, the pins in order); a pin no building stands on then goes to the nearest building left
/// within `PIN_REACH` ft (a lot moved a little), else it is lost. Per building laid out from
/// `first`: the id pinned to it.
fn pin_on(l: &mut Layout, first: usize, pins: &[Pin], origin: P) -> Vec<Option<u32>> {
    const PIN_REACH: f64 = 12.0;
    let mut out = vec![None; l.buildings.len() - first];
    let mut on: Vec<Option<usize>> = vec![None; pins.len()];
    for (i, pin) in pins.iter().enumerate() {
        let at = sub(pin.p, origin);
        on[i] = (0..out.len()).find(|&k| out[k].is_none() && contains(&l.buildings[first + k].poly, at));
        if let Some(k) = on[i] {
            out[k] = Some(pin.id);
        }
    }
    for (i, pin) in pins.iter().enumerate() {
        if on[i].is_some() {
            continue;
        }
        let at = sub(pin.p, origin);
        let edge = |poly: &[P]| (0..poly.len()).map(|e| seg_dist(at, poly[e], poly[(e + 1) % poly.len()])).fold(f64::MAX, f64::min);
        on[i] = (0..out.len()).filter(|&k| out[k].is_none()).map(|k| (edge(&l.buildings[first + k].poly), k)).filter(|x| x.0 <= PIN_REACH).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|x| x.1);
        if let Some(k) = on[i] {
            out[k] = Some(pin.id);
        }
    }
    for (pin, k) in pins.iter().zip(on) {
        let Some(k) = k else { continue };
        if let Some(fi) = pin.func.as_deref().and_then(catalog::index_of) {
            let b = &mut l.buildings[first + k];
            (b.func, b.name) = (Some(fi as u16), pin.name.clone());
            if CATALOG[fi].key == "graveyard" {
                b.structure = Structure::Open;
            }
        }
    }
    out
}

/// What a block laid out anew may hold as businesses: shops, workshops, inns, taverns and the like
/// (not what a town has one of: its hall, castle, temples, guilds, mills).
pub(super) fn trade(f: &catalog::Func) -> bool {
    let shops = ["alchemist", "apothecary", "magic_shop", "scribe", "bookseller", "cartographer", "fortune_teller", "bathhouse", "curiosities", "gambling_den", "undertaker", "guard_post", "pawnbroker"];
    (matches!(f.category, "food & lodging" | "arms & gear" | "luxury & finance" | "crafts & transport") && !matches!(f.key, "mill" | "caravanserai" | "auction_house" | "bank")) || shops.contains(&f.key)
}

/// About this share of a block's homes are businesses when it is laid out anew, by its ward.
fn business_share(w: Ward) -> f64 {
    match w {
        Ward::Merchant | Ward::Plaza => 0.14,
        Ward::Craft => 0.10,
        Ward::Docks => 0.08,
        Ward::Military => 0.06,
        Ward::Common => 0.04,
        Ward::Noble | Ward::Slum => 0.03,
        _ => 0.0,
    }
}

/// What a generated fill's storeys can be set to (`BlockFill::Generated::height`): one storey
/// each, the ward's usual, one more than usual.
pub const HEIGHTS: [&str; 3] = ["low", "usual", "tall"];

/// The storeys of the homes of a block of ward `w` laid out anew (`l.buildings[first..]`, but
/// those pinned), as a town of that size rolls them per ward (`assign_functions`).
pub(super) fn home_storeys(l: &mut Layout, first: usize, w: Ward, tier: Tier, key: u64, pinned: &[Option<u32>]) {
    for k in first..l.buildings.len() {
        let b = &mut l.buildings[k];
        if pinned.get(k - first).is_some_and(|p| p.is_some()) || b.func.is_some() || b.keep.is_some() || b.structure != Structure::Roofed {
            continue;
        }
        let r = unit(hash2(key, k as i64, 1));
        b.floors = match w {
            Ward::Noble | Ward::Merchant | Ward::Castle => 2 + (r < 0.4) as u8,
            Ward::Slum | Ward::Common | Ward::Craft | Ward::Docks => 1 + (tier >= Tier::City && r < 0.5) as u8,
            _ => 1,
        };
    }
}

/// A block's storeys as its fill's `height` asks (after `businesses`): "low" one storey each,
/// "tall" one more than rolled (to 4); none or "usual" as rolled. Those pinned are left be.
pub(super) fn height_storeys(l: &mut Layout, first: usize, height: Option<&str>, pinned: &[Option<u32>]) {
    let set = |f: u8| match height {
        Some("low") => 1,
        Some("tall") => (f + 1).min(4),
        _ => f,
    };
    for k in first..l.buildings.len() {
        let b = &mut l.buildings[k];
        if pinned.get(k - first).is_none_or(|p| p.is_none()) && b.keep.is_none() && b.structure == Structure::Roofed {
            b.floors = set(b.floors);
        }
    }
}

/// Businesses for a block of ward `w` laid out anew (`l.buildings[first..]`, `pinned` as `pin_on`
/// gave): about `business_share` of its homes become a business of that ward the town's size has
/// (more likely the more of it a town of that size has), on the lot that suits it best (as
/// `assign_functions` picks), with its storeys; a pinned building is left as pinned. Named after,
/// with the town's others (`build_hand`).
pub(super) fn businesses(l: &mut Layout, first: usize, w: Ward, tier: Tier, on_water: bool, key: u64, pinned: &[Option<u32>]) {
    let free = |k: usize| pinned.get(k - first).is_none_or(|p| p.is_none());
    let homes: Vec<usize> = (first..l.buildings.len()).filter(|&k| free(k) && l.buildings[k].func.is_none() && l.buildings[k].keep.is_none() && l.buildings[k].structure == Structure::Roofed).collect();
    let have = l.buildings[first..].iter().filter(|b| b.func.is_some()).count();
    let want = (crate::core::floor(homes.len() as f64 * business_share(w) + unit(hash2(key, 0, 1))) as usize).saturating_sub(have);
    let menu: Vec<(usize, f64)> = CATALOG
        .iter()
        .enumerate()
        .filter(|(_, f)| tier >= f.min_tier && (!f.water || on_water) && trade(f))
        .filter_map(|(fi, f)| f.wards.iter().position(|x| *x == w).map(|pos| (fi, (1 + f.counts[tier as usize]) as f64 / (1 + pos) as f64)))
        .collect();
    if menu.is_empty() || want == 0 {
        return;
    }
    let mut rng = Pcg32::new(key, 53);
    let areas: Vec<f64> = homes.iter().map(|&k| area(&l.buildings[k].poly).abs()).collect();
    let max_area = areas.iter().cloned().fold(1.0, f64::max);
    let mut taken = vec![false; homes.len()];
    let total: f64 = menu.iter().map(|m| m.1).sum();
    for _ in 0..want.min(homes.len()) {
        let mut r = rng.next_f64() * total;
        let fi = menu.iter().find(|m| {
            r -= m.1;
            r < 0.0
        });
        let fi = fi.map_or(menu[menu.len() - 1].0, |m| m.0);
        let f = &CATALOG[fi];
        let mut best: Option<(f64, usize)> = None;
        for j in (0..homes.len()).filter(|&j| !taken[j]) {
            let size = areas[j] / max_area;
            let score = if f.big { 6.0 * size } else { -2.0 * size } - if areas[j] < 500.0 { 100.0 } else { 0.0 } + 1.5 * rng.next_f64();
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, j));
            }
        }
        let Some((_, j)) = best else { break };
        taken[j] = true;
        let b = &mut l.buildings[homes[j]];
        b.func = Some(fi as u16);
        b.floors = match f.key {
            "inn" | "tavern" => 2 + (rng.next_f64() < 0.5) as u8,
            "warehouse" | "shrine" => 1,
            _ => 1 + (tier >= Tier::Town && rng.next_f64() < 0.6) as u8,
        };
    }
}

/// The fill of open ground of kind `open` (its style by name, else the kind's usual): its ways in
/// along `fronts`, its looks' key, a cemetery's style, farmland's stream.
pub(super) fn open_fill(open: OpenKind, style: Option<&str>, fronts: &[super::super::fill::Front], key: u64, cem: impl Fn() -> super::super::cemetery::Style, rng: &mut dyn FnMut() -> Pcg32, tier: Tier) -> Fill {
    let plaza = style.and_then(|n| PLAZAS.iter().copied().find(|k| k.name() == n));
    let parkk = style.and_then(ParkKind::parse);
    let ways: Vec<(P, f64)> = fronts.iter().map(|f| (f.n, f.c)).collect();
    let park = |kind: ParkKind| Fill::Park { kind, ways: ways.clone(), key, cemetery: (kind == ParkKind::Cemetery).then(&cem) };
    match open {
        OpenKind::Plaza => Fill::Square(plaza.unwrap_or(if tier >= Tier::City { PlazaKind::Civic } else { PlazaKind::MarketDay })),
        OpenKind::Market => Fill::Square(plaza.filter(|k| k.is_market()).unwrap_or(PlazaKind::Market)),
        OpenKind::Lawn => Fill::Square(PlazaKind::Green),
        OpenKind::Park => park(parkk.unwrap_or(ParkKind::Common)),
        OpenKind::Garden => park(parkk.unwrap_or(ParkKind::Formal)),
        OpenKind::Cemetery => park(ParkKind::Cemetery),
        OpenKind::Farmland => Fill::Farm { class: 0, rng: rng() },
    }
}
