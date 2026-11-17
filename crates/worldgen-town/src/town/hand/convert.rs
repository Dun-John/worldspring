//! From records to a plan: kept buildings, captures, converting a generated
//! town to hand records.

use super::*;

/// A fill's building moved to world ft (as `generate_in` moves every building).
fn to_world(mut b: Building, origin: P) -> Building {
    b.poly.iter_mut().chain(b.parts.iter_mut().flat_map(|p| p.poly.iter_mut())).for_each(|p| *p = add(*p, origin));
    b
}

/// The home kind a building of no function has in a town (`assign_functions`).
fn home_kind(b: &Building) -> u8 {
    catalog::residential_for(b.ward, area(&b.poly).abs() > 2500.0) as u8
}

/// A fill's building `raw` (world ft) as the town made it: its id and storeys, what `o` sets
/// and, with `o.y`, on the round footprint `yurts::ring` stands a steppe home on in its lot,
/// turned `y` corners to its door (and drawn up `o.u` ft toward it, as `yurts::extra` draws
/// one up beside a pen or a store).
fn rebuild(raw: &Building, id: u32, floors: u8, o: &Over, roofs: &Layout) -> Result<Building, String> {
    let mut b = raw.clone();
    b.id = id;
    b.floors = floors;
    apply(o, &mut b)?;
    if let Some(y) = o.y {
        let mut ring = super::super::yurts::ring(roofs, &b).filter(|r| (y as usize) < r.len()).ok_or("kept block: no yurt on its lot")?;
        ring.rotate_left(y as usize);
        if let Some(t) = o.u {
            let n = super::super::yurts::frame(&ring).2;
            ring.iter_mut().for_each(|p| *p = add(*p, mul(n, t)));
        }
        b.poly = ring;
    }
    Ok(b)
}

/// What the town gave a fill's building `raw` (world ft) to make it `b`, if only what `Over` and
/// the storeys hold differs: `rebuild` makes `b` again exactly (else it is stored whole).
pub(super) fn over(raw: &Building, b: &Building, roofs: &Layout) -> Option<Over> {
    if !(1..=9).contains(&b.floors) {
        return None;
    }
    let mut o = Over { i: b.id, ..Over::default() };
    if b.ward != raw.ward {
        o.w = Some(index_of(&WARDS, &b.ward));
    }
    if b.func != raw.func {
        o.f = Some(b.func.map_or(-1, |f| f as i32));
    }
    if b.name != raw.name {
        o.n = b.name.clone();
    }
    if b.pad_ft.to_bits() != raw.pad_ft.to_bits() {
        o.p = Some(b.pad_ft as f64);
    }
    if b.structure != raw.structure {
        o.s = Some(index_of(&STRUCTURES, &b.structure));
    }
    if b.residential != rebuild(raw, b.id, b.floors, &o, roofs).ok()?.residential {
        o.r = Some(b.residential);
    }
    if raw.poly != b.poly {
        let ring = super::super::yurts::ring(roofs, &rebuild(raw, b.id, b.floors, &o, roofs).ok()?)?;
        let n = ring.len();
        if b.poly.len() != n {
            return None;
        }
        // (Turned to its door; or drawn up toward it too, by a quarter foot or so: the step
        // `yurts::extra` takes, its door's way out from its middle.)
        let (y, t) = (0..n).find_map(|y| {
            let turned: Vec<P> = (0..n).map(|k| ring[(k + y) % n]).collect();
            if turned == b.poly {
                return Some((y, None));
            }
            let dir = super::super::yurts::frame(&turned).2;
            let t = crate::core::round(4.0 * dot(sub(b.poly[0], turned[0]), dir)) / 4.0;
            (t > 0.0 && turned.iter().zip(&b.poly).all(|(p, q)| add(*p, mul(dir, t)) == *q)).then_some((y, Some(t)))
        })?;
        (o.y, o.u) = (Some(y as u8), t);
    }
    differs(&rebuild(raw, b.id, b.floors, &o, roofs).ok()?, b).is_empty().then_some(o)
}

/// `over` applied to a fill's building (its storeys already set), but for its footprint.
fn apply(o: &Over, b: &mut Building) -> Result<(), String> {
    if let Some(w) = o.w {
        b.ward = nth(&WARDS, w, "ward")?;
    }
    if let Some(f) = o.f {
        b.func = (f >= 0).then_some(f as u16);
    }
    if let Some(n) = &o.n {
        b.name = Some(n.clone());
    }
    if let Some(p) = o.p {
        b.pad_ft = p as f32;
    }
    if let Some(s) = o.s {
        b.structure = nth(&STRUCTURES, s, "structure")?;
    }
    b.residential = match (o.r, b.func) {
        (Some(r), _) => r,
        (None, None) => home_kind(b),
        (None, Some(_)) => b.residential,
    };
    Ok(())
}

pub(super) fn roofs_of(meta: (u32, P, f64, Tier), roof_seed: u64) -> Layout {
    let mut l = Layout::empty(meta.0, meta.1, meta.2, meta.3);
    l.roof_seed = roof_seed;
    l
}

/// A kept block's buildings (world ft), in id order, from its record alone.
pub fn kept_on(ground: &KeptGround, kept: &Kept) -> Result<Vec<Building>, String> {
    let raw = match &kept.refill {
        Some(r) => r.run(&ground.site, ground.meta)?,
        None => Vec::new(),
    };
    kept_from(ground, kept, &raw)
}

/// `kept_on` with its fill's buildings laid out already (`raw`, local ft).
pub(super) fn kept_from(ground: &KeptGround, kept: &Kept, raw: &[Building]) -> Result<Vec<Building>, String> {
    let origin = ground.meta.1;
    let overs: FastMap<u32, &Over> = kept.set.iter().map(|o| (o.i, o)).collect();
    let mut floors = kept.floors.bytes();
    let mut out = Vec::new();
    for &[k, id, n] in &kept.runs {
        for j in 0..n {
            let b = raw.get((k + j) as usize).ok_or_else(|| format!("kept block: its fill laid out {} buildings, not {}", raw.len(), k + j + 1))?;
            let fl = floors.next().filter(u8::is_ascii_digit).ok_or("kept block: storeys cut short")? - b'0';
            let none = Over::default();
            let o = overs.get(&(id + j)).copied().unwrap_or(&none);
            out.push(rebuild(&to_world(b.clone(), origin), id + j, fl, o, &ground.roofs)?);
        }
    }
    for f in &kept.full {
        out.push(f.building()?);
    }
    out.sort_by_key(|b| b.id);
    Ok(out)
}

/// Block `block` of hand town `index`: its kept buildings (world ft, id order), from its record
/// alone (none if it keeps none).
pub fn kept_buildings(world: &World, t0: &T0, index: usize, block: u32) -> Result<Vec<Building>, String> {
    let recs = HandRecords::of(&world.file.edits.streets, index);
    let town = recs.town.as_ref().ok_or("not a hand town")?;
    let b = recs.blocks.get(&block).ok_or("no such block")?;
    match &b.kept {
        Some(k) => kept_on(&KeptGround::of(world, t0, index, town), k),
        None => Ok(Vec::new()),
    }
}

/// The fields two buildings differ in (exactly: every float by its bits).
pub fn differs(a: &Building, b: &Building) -> Vec<&'static str> {
    let mut out = Vec::new();
    let d = |x: &dyn std::fmt::Debug, y: &dyn std::fmt::Debug| format!("{x:?}") != format!("{y:?}");
    for (name, diff) in [
        ("poly", d(&a.poly, &b.poly)),
        ("ward", a.ward != b.ward),
        ("func", a.func != b.func),
        ("residential", a.residential != b.residential),
        ("name", a.name != b.name),
        ("floors", a.floors != b.floors),
        ("pad_ft", a.pad_ft.to_bits() != b.pad_ft.to_bits()),
        ("structure", a.structure != b.structure),
        ("roof", a.roof != b.roof),
        ("look", (a.tint, a.material, a.shape, a.shade) != (b.tint, b.material, b.shape, b.shade)),
        ("id", a.id != b.id),
        ("parts", d(&a.parts, &b.parts)),
        ("keep", a.keep != b.keep),
        ("deck", d(&a.deck, &b.deck)),
    ] {
        if diff {
            out.push(name);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Capture: what `build_town` decided per patch, read once by a conversion.

#[derive(Default)]
struct Capture {
    index: usize,
    plan: Option<Rc<TownPlan>>,
    towns: u32,
    faces: Vec<Vec<usize>>,
    cells: Vec<Vec<P>>,
    labels: Vec<Vec<i32>>,
    insets: Vec<Vec<f64>>,
    ward: Vec<Option<Ward>>,
    group: Vec<usize>,
    walled: bool,
    gates: Vec<P>,
    pending: Option<(usize, Refill)>,
    fills: BTreeMap<usize, (Refill, Vec<Building>)>,
    squares: Vec<(usize, PlazaKind)>,
}

thread_local! {
    static CAP: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

fn with_cap(f: impl FnOnce(&mut Capture)) {
    CAP.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            f(c)
        }
    })
}

/// Whether layout `index` is being converted (`build_town` then records its patches).
pub(in crate::town) fn capturing(index: u32) -> bool {
    CAP.with(|c| c.borrow().as_ref().is_some_and(|c| c.index == index as usize))
}

pub(in crate::town) fn cap_plan(index: usize, plan: &Rc<TownPlan>) {
    with_cap(|c| {
        if c.index == index {
            c.plan = Some(plan.clone());
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::town) fn cap_town(faces: &[Vec<usize>], cells: &[Labeled], insets: Vec<Vec<f64>>, ward: &[Option<Ward>], group: &[usize], walled: bool, gates: &[P]) {
    with_cap(|c| {
        c.towns += 1;
        c.faces = faces.to_vec();
        c.cells = cells.iter().map(|l| l.pts.clone()).collect();
        c.labels = cells.iter().map(|l| l.labels.clone()).collect();
        c.insets = insets;
        c.ward = ward.to_vec();
        c.group = group.to_vec();
        c.walled = walled;
        c.gates = gates.to_vec();
    })
}

pub(in crate::town) fn cap_fill(i: usize, fill: &Fill, block: &[P], lines: &[(P, f64)], roll: &dyn Fn(i64) -> f64) {
    with_cap(|c| c.pending = Refill::of(fill, block, lines, roll).map(|r| (i, r)))
}

pub(in crate::town) fn cap_built(i: usize, built: &[Building]) {
    with_cap(|c| {
        if let Some((p, r)) = c.pending.take()
            && p == i
        {
            c.fills.insert(i, (r, built.to_vec()));
        }
    })
}

pub(in crate::town) fn cap_square(v: usize, kind: PlazaKind) {
    with_cap(|c| c.squares.push((v, kind)))
}

// ---------------------------------------------------------------------------------------
// Conversion.

/// A point on a 1/64-ft grid (written short, read back the same).
fn q64(p: P) -> P {
    [crate::core::round(p[0] * 64.0) / 64.0, crate::core::round(p[1] * 64.0) / 64.0]
}

pub(in crate::town) fn hex(v: u64) -> String {
    format!("{v:016x}")
}

/// A point well inside a polygon: its centroid if inside, else the middle of the widest chord
/// across it from one of its corners (its mean, failing those).
pub(in crate::town) fn inside_point(poly: &[P]) -> P {
    let c = centroid(poly);
    if contains(poly, c) {
        return c;
    }
    let n = poly.len();
    let mut best: Option<(f64, P)> = None;
    for i in 0..n {
        for j in i + 2..n {
            let m = lerp(poly[i], poly[j], 0.5);
            if contains(poly, m) && best.is_none_or(|(d, _)| dist(poly[i], poly[j]) > d) {
                best = Some((dist(poly[i], poly[j]), m));
            }
        }
    }
    best.map_or_else(|| mean(poly), |b| b.1)
}

/// Town `index` as laid out now (generated, or by its ward edit), as hand town records (key,
/// value), in key order. A village has no streets plan to convert.
pub fn convert(world: &World, t0: &T0, index: usize) -> Result<Vec<(String, Value)>, String> {
    let s = super::super::culture::settlement(world, t0, index).ok_or("not a settlement")?;
    if s.tier == Tier::Village {
        return Err("a village has no streets plan to lay out by hand".into());
    }
    if HandRecords::of(&world.file.edits.streets, index).town.is_some() {
        return Err("already laid out by hand".into());
    }
    let edit = super::super::wards::edit_of(world, t0, index);
    // (The layouts it reads made first, so the one run here is the only one recorded.)
    let base = super::super::base_layout(world, t0, index);
    super::super::generated_layout(world, t0, index);
    CAP.with(|c| *c.borrow_mut() = Some(Capture { index, ..Capture::default() }));
    let again = super::super::generate_with(world, t0, index, edit, None);
    let cap = CAP.with(|c| c.borrow_mut().take()).expect("the capture just set");
    let plan = cap.plan.clone().ok_or("no plan laid out")?;
    if cap.towns != 1 {
        return Err(format!("the town was built {} times converting it", cap.towns));
    }
    if again.buildings.len() != base.buildings.len() {
        return Err("the town came out otherwise laid out again".into());
    }
    let center = [s.x, s.y];
    let ch = edit.map(|e| super::super::wards::changes(&plan, e, center).0).unwrap_or_default();
    let li = index.to_string();
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut put = |key: String, v: Value| out.push((key, v));

    // Junctions: every corner of a patch.
    let mut corner: BTreeMap<usize, P> = BTreeMap::new();
    for (f, pts) in cap.faces.iter().zip(&cap.cells) {
        for (&v, &p) in f.iter().zip(pts) {
            corner.entry(v).or_insert(add(p, center));
        }
    }
    let squares: BTreeMap<usize, PlazaKind> = cap.squares.iter().copied().collect();
    for (&v, &p) in &corner {
        let j = Junction { p, square: squares.get(&v).map(|k| k.name().to_string()) };
        put(format!("{li}/n{v}"), serde_json::to_value(j).map_err(|e| e.to_string())?);
    }

    // Streets: every patch side, the setback of the patch on each side of it.
    let bridged: Vec<(usize, usize)> = plan.bridged.iter().flat_map(|&((a, r, b), _)| [(a.min(r), a.max(r)), (r.min(b), r.max(b))]).collect();
    let mut sides: BTreeMap<(usize, usize), (Option<(usize, f64)>, Option<(usize, f64)>)> = BTreeMap::new();
    for (i, f) in cap.faces.iter().enumerate() {
        let m = f.len();
        if m < 3 {
            continue;
        }
        let ccw = area(&cap.cells[i]) > 0.0;
        for e in 0..m {
            let (va, vb) = (f[e], f[(e + 1) % m]);
            let key = (va.min(vb), va.max(vb));
            let set = (i, cap.insets[i].get(e).copied().unwrap_or(0.0));
            let slot = sides.entry(key).or_default();
            // (A patch is on its side's left looking from the lower corner when it runs round
            // anticlockwise that way.)
            if ccw == (va == key.0) { slot.0 = Some(set) } else { slot.1 = Some(set) }
        }
    }
    for (k, (&(a, b), &(l, r))) in sides.iter().enumerate() {
        let (set_l, set_r) = (l.map_or(0.0, |x| x.1), r.map_or(0.0, |x| x.1));
        let joint = matches!((l, r), (Some((p, _)), Some((q, _))) if cap.group[p] != usize::MAX && cap.group[p] == cap.group[q]) && set_l == 0.0 && set_r == 0.0;
        let class = if joint {
            StreetClass::Joint
        } else if plan.main_edge(a, b) {
            StreetClass::Main
        } else if plan.shore.contains(&(a, b)) {
            StreetClass::Quay
        } else {
            StreetClass::Street
        };
        let st = Street { a: a as u32, b: b as u32, bends: Vec::new(), width: set_l + set_r, set_l, set_r, class, bridge: bridged.contains(&(a, b)) };
        put(format!("{li}/e{k}"), serde_json::to_value(st).map_err(|e| e.to_string())?);
    }

    // Blocks: every patch, its fill as its ward and settings have it, its buildings kept.
    let quarters = super::super::districts::of(&plan, &ch);
    let district = |i: usize| quarters.iter().position(|q| q.1.contains(&i)).and_then(|q| base.quarters.get(q)).map(|q| q.name.clone());
    let lot_name = |i: usize| ch.lots.get(&i).and_then(|f| super::super::wards::LOT_FACTOR.iter().position(|x| x == f)).map(|k| LOT_SIZES[k].to_string());
    let park_of = |i: usize| ch.park.get(&i).copied().or_else(|| base.parks.iter().find(|pk| pk.spot == super::super::places::Spot::Patch(i as u32)).map(|pk| pk.kind));
    // Which block each building stands in: those a fill laid out by the fill, the rest by where
    // they stand (else the town's own).
    let mut from_fill: FastMap<Vec<u64>, (usize, usize)> = FastMap::default();
    for (&i, (_, raw)) in &cap.fills {
        for (k, b) in raw.iter().enumerate() {
            from_fill.entry(to_world(b.clone(), center).poly.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]).collect()).or_insert((i, k));
        }
    }
    let roofs = roofs_of((index as u32, center, 0.0, s.tier), base.roof_seed);
    let patch_at = |p: P| (0..cap.cells.len()).find(|&i| cap.cells[i].len() >= 3 && contains(&cap.cells[i], sub(p, center)));
    let mut kept: BTreeMap<usize, Kept> = BTreeMap::new();
    let mut runs: BTreeMap<usize, Vec<(u32, u32)>> = BTreeMap::new();
    let mut used: crate::core::hash::FastSet<(usize, usize)> = Default::default();
    for b in &base.buildings {
        let bits: Vec<u64> = b.poly.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]).collect();
        let try_one = |i: usize, k: usize| (!used.contains(&(i, k))).then(|| over(&to_world(cap.fills[&i].1[k].clone(), center), b, &roofs).map(|o| (i, k, o))).flatten();
        // (Else a yurt's ring on the lot it stands in.)
        let c = mean(&b.poly);
        let matched = from_fill.get(&bits).and_then(|&(i, k)| try_one(i, k)).or_else(|| {
            let i = patch_at(c)?;
            let raw = &cap.fills.get(&i)?.1;
            (0..raw.len()).filter(|&k| contains(&raw[k].poly, sub(c, center))).find_map(|k| try_one(i, k))
        });
        match matched {
            Some((i, k, o)) => {
                used.insert((i, k));
                let kp = kept.entry(i).or_default();
                kp.floors.push((b'0' + b.floors) as char);
                if o != (Over { i: b.id, ..Over::default() }) {
                    kp.set.push(o);
                }
                runs.entry(i).or_default().push((k as u32, b.id));
            }
            None => {
                kept.entry(patch_at(c).unwrap_or(usize::MAX)).or_default().full.push(Full::of(b));
            }
        }
    }
    for (i, list) in runs {
        let kp = kept.entry(i).or_default();
        kp.refill = Some(cap.fills[&i].0.clone());
        for (k, id) in list {
            match kp.runs.last_mut() {
                Some(r) if r[0] + r[2] == k && r[1] + r[2] == id => r[2] += 1,
                _ => kp.runs.push([k, id, 1]),
            }
        }
    }
    // (Every fill, though it keeps no building: a square's ground and monument, a park's, which of
    // farmland's plots are fields, laid out again.)
    let ground = KeptGround::at(world, t0, index, center, super::super::reach(&s) + 500.0, s.tier, base.roof_seed);
    let mut fields: FastMap<Vec<u64>, u32> = FastMap::default();
    for f in &base.fields {
        *fields.entry(f.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]).collect()).or_default() += 1;
    }
    for (&i, (r, _)) in &cap.fills {
        let kp = kept.entry(i).or_default();
        kp.refill = Some(r.clone());
        if r.k == 1 {
            for p in ground.plots(r) {
                let bits: Option<Vec<u64>> = p.map(|p| p.iter().flat_map(|q| { let w = add(*q, center); [w[0].to_bits(), w[1].to_bits()] }).collect());
                let hit = bits.and_then(|b| fields.get_mut(&b)).filter(|n| **n > 0).map(|n| *n -= 1).is_some();
                kp.fields.push(if hit { '1' } else { '0' });
            }
            if !kp.fields.contains('0') {
                kp.fields.clear();
            }
        }
    }
    // (A castle's own works, for its walls laid anew.)
    for i in (0..cap.cells.len()).filter(|&i| cap.ward[i] == Some(Ward::Castle) && cap.cells[i].len() >= 3) {
        let cell: Vec<P> = cap.cells[i].iter().map(|p| add(*p, center)).collect();
        if let Some(c) = CastleLook::of(&base, &cell) {
            kept.entry(i).or_default().curtain = Some(c.clone());
        }
    }
    for i in 0..cap.cells.len() {
        if cap.cells[i].len() < 3 {
            continue;
        }
        let w = cap.ward[i];
        let fill = match w {
            None => BlockFill::Hand,
            Some(Ward::Plaza) => {
                let k = super::super::fill::square_kind(plan.roll(i, 44), s.tier >= Tier::City);
                BlockFill::Open { open: if k.is_market() { OpenKind::Market } else { OpenKind::Plaza }, style: Some(k.name().into()) }
            }
            Some(Ward::Farm) => BlockFill::Open { open: OpenKind::Farmland, style: None },
            Some(Ward::Park) => {
                let k = park_of(i).unwrap_or(ParkKind::Common);
                let open = match k {
                    ParkKind::Formal => OpenKind::Garden,
                    ParkKind::Cemetery => OpenKind::Cemetery,
                    ParkKind::Common | ParkKind::Horses => OpenKind::Park,
                };
                BlockFill::Open { open, style: Some(k.name().into()) }
            }
            Some(Ward::Castle) => BlockFill::Castle { hand: plan.ward[i] != Some(Ward::Castle), gates: Vec::new(), keep_shape: None, moat: None },
            Some(w) => BlockFill::Generated { ward: w.name().into(), lots: lot_name(i), height: None, reroll: ch.reroll.get(&i).copied().unwrap_or(0), close: ch.close.get(&i).copied() },
        };
        let open = !matches!(fill, BlockFill::Hand);
        let block = Block {
            at: q64(add(inside_point(&cap.cells[i]), center)),
            fill,
            district: district(i),
            seed: hex(hash3(s.seed, index as i64, i as i64, 0x4a4d)),
            generation: 0,
            pins: Vec::new(),
            // (Nothing kept but what its fill made: it stays so until its lots next change.)
            kept: kept.remove(&i).or_else(|| open.then(Kept::default)),
        };
        put(format!("{li}/k{i}"), serde_json::to_value(block).map_err(|e| e.to_string())?);
    }

    // Wall lines: each run of the town's wall, where it stands.
    if cap.walled {
        let pos = |v: usize| corner.get(&v).copied().unwrap_or_else(|| add(plan.mesh.pos[v], center));
        // (Each run's look as laid: re-laid from its line, it keeps it while the line is as here.)
        let castled: Vec<Vec<P>> = (0..cap.cells.len()).filter(|&i| cap.ward[i] == Some(Ward::Castle) && cap.cells[i].len() >= 3).map(|i| cap.cells[i].iter().map(|p| add(*p, center)).collect()).collect();
        let runs: Vec<Vec<P>> = plan
            .wall_runs
            .iter()
            .map(|(chain, closed, _)| chain.iter().chain(chain.first().filter(|_| *closed)).map(|&v| pos(v)).collect())
            .collect();
        let mut looks = wall_looks(&base, &runs, &castled);
        for (k, (chain, closed, towers)) in plan.wall_runs.iter().enumerate() {
            let pts: Vec<P> = chain.iter().map(|&v| pos(v)).collect();
            let gates: Vec<P> = cap.gates.iter().map(|g| add(*g, center)).filter(|g| chain.windows(2).any(|w| seg_dist(*g, pos(w[0]), pos(w[1])) < 1.0) || pts.iter().any(|p| dist(*p, *g) < 1.0)).collect();
            let mut wall = WallLine { pts, closed: *closed, spacing: base.wall_style.spacing, towers: towers.iter().map(|&t| t as u32).collect(), gates, name: None, look: None };
            wall.look = looks[k].take().map(|lk| WallLook { of: line_of(&wall), ..lk });
            put(format!("{li}/m{k}"), serde_json::to_value(wall).map_err(|e| e.to_string())?);
        }
    }

    // The header, with what stands in no block.
    let extent = base.buildings.iter().flat_map(|b| b.poly.iter()).chain(corner.values()).map(|p| dist(*p, center)).fold(0.0, f64::max);
    let town = HandTown {
        from: "generated".into(),
        // (A streets-only entry is no edit to plan with, yet going back brings its streets again.)
        was: edit.cloned().or_else(|| world.file.edits.towns.get(&super::super::wards::key(index)).filter(|e| !e.streets.is_empty() && dist(e.at, center) <= TownEdit::SAME_AT_FT).cloned()),
        seed: hex(s.seed),
        culture: s.culture,
        fill: BlockFill::default(),
        extent,
        origin: center,
        tier: s.tier as u8,
        reach: super::super::reach(&s) + 500.0,
        roofs: hex(base.roof_seed),
        kept: kept.remove(&usize::MAX),
        works: None,
        people: Some(s.population),
        was_people: Some(s.population),
        was_homes: Some(people_of(&base)),
        next_block: None,
    };
    debug_assert!(kept.is_empty(), "every block's buildings go with its block");
    // (And its walls, gates, bridges and squares as they are, while what they stand on stands.)
    let recs: BTreeMap<String, Value> = out.iter().cloned().collect();
    let town = HandTown { works: Some(works_from(&base, works_of(&HandRecords::of(&recs, index)), &corner)), ..town };
    out.push((li.clone(), serde_json::to_value(town).map_err(|e| e.to_string())?));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
