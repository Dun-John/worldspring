//! A town's plan alone, as a quick stand-in outline while its layout is still being built: its
//! wall, gates, main streets, bridges and ward patches as `generate_in` plans them (the ward
//! edit's wards, corners, gates and walls applied), none of it built. Read by the app alone:
//! nothing that makes output reads it.

use super::*;

/// What town `index` is planned as (world ft).
#[derive(Clone, Debug, Default)]
pub struct Standin {
    pub center: P,
    /// Wall runs (corners; whether closed). None for a town without walls.
    pub walls: Vec<(Vec<P>, bool)>,
    pub gates: Vec<P>,
    /// Main streets: from each gate to the market.
    pub streets: Vec<Vec<P>>,
    /// Ways over the river (bank, river, bank) bridged for a main street.
    pub bridges: Vec<Vec<P>>,
    /// The town's patches on dry land, and each one's ward (`Ward as u8`, 255 none).
    pub patches: Vec<(Vec<P>, u8)>,
    /// Water within the patches (the sea, a lake and the river, as the layout cuts its
    /// blocks by them): runs `[x0, x1, y]` (world ft) of cells `water_cell` ft tall.
    pub water: Vec<[f64; 3]>,
    pub water_cell: f64,
}

thread_local! {
    static MEMO: RefCell<(u64, FastMap<u32, (u64, Rc<Standin>)>)> = RefCell::new((0, FastMap::default()));
}

/// Town `index`'s plan as a stand-in (`Standin`), kept per world and its edits; `None` for a
/// village, a site, a created settlement or a town laid out by hand.
pub fn standin(world: &World, t0: &T0, index: usize) -> Option<Rc<Standin>> {
    if index >= t0.settlements.len() || hand::is_hand(world, index) {
        return None;
    }
    let s = culture::settlement(world, t0, index).filter(|s| s.tier >= Tier::Town)?;
    let key = index as u32;
    let mut fp = crate::core::hash::Fnv64::default();
    for x in [wards::fingerprint(world, t0, index).unwrap_or(0), plan::frozen_fp(world, t0, index), culture::mark(world, t0, index) as u64, planning::mark(world, t0, index), pattern::mark(world, t0, index)] {
        fp.write(&x.to_le_bytes());
    }
    let fp = fp.finish();
    if let Some(o) = MEMO.with(|m| {
        let m = m.borrow();
        if m.0 == world.hash { m.1.get(&key).filter(|e| e.0 == fp).map(|e| e.1.clone()) } else { None }
    }) {
        return Some(o);
    }
    let o = Rc::new(make(world, t0, index, &s));
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.0 != world.hash {
            m.0 = world.hash;
            m.1.clear();
        }
        m.1.insert(key, (fp, o.clone()));
    });
    Some(o)
}

/// `generate_in`'s planning steps for a town, up to `build_town`.
fn make(world: &World, t0: &T0, index: usize, s: &Settlement) -> Standin {
    let center = [s.x, s.y];
    let r = urban_radius(s.tier, s.population);
    let edit = wards::edit_of(world, t0, index);
    let (site, road_ends, mut rng) = plan::prepare(world, t0, index);
    let plan = wards::plan(world, t0, index, s.culture, || plan::plan_for(world, t0, index, &site, s, r, &road_ends, &mut rng));
    let plan = if plan::frozen_fp(world, t0, index) != 0 { Rc::new(plan::rejoined(&plan, &site, &road_ends, edit.map(|e| &e.wall), plan::frozen_ends(world, t0, index).map_or(0, |f| f.0.len()))) } else { plan };
    let ch = edit.map(|e| wards::changes(&plan, e, center).0).unwrap_or_default();
    let mesh = wards::moved_mesh(&plan, &ch.moves);
    let (ward, _) = ch.wards(&plan);
    let at = |v: usize| add(mesh.pos[v], center);
    let walled = ch.walls.unwrap_or(plan.walled);
    let walls = if walled { plan.wall_runs.iter().map(|(c, closed, _)| (c.iter().map(|&v| at(v)).collect(), *closed)).collect() } else { Vec::new() };
    let gates = if walled { plan.gates.iter().filter(|g| !ch.wall.remove.contains(g)).chain(ch.wall.add.iter()).map(|&v| at(v)).collect() } else { Vec::new() };
    let streets = plan.paths.iter().filter(|p| p.len() > 1).map(|p| p.iter().map(|&v| at(v)).collect()).collect();
    let bridges = plan.bridged.iter().filter(|b| b.1).map(|&((a, b, c), _)| vec![at(a), at(b), at(c)]).collect();
    let patches = (0..plan.n_inner).filter(|&i| plan.usable(i)).map(|i| (mesh.faces[i].iter().map(|&v| at(v)).collect(), ward[i].map_or(255, |w| w as u8))).collect();
    // Water by the site's own test (`Site::wet`: standing water in the drawn shore, and the river
    // channel), sampled where a patch is and joined into runs along x.
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    let mut polys: Vec<([f64; 4], Vec<P>)> = Vec::new();
    for i in (0..plan.n_inner).filter(|&i| plan.usable(i)) {
        let poly: Vec<P> = mesh.faces[i].iter().map(|&v| mesh.pos[v]).collect();
        let bb = poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        for k in 0..2 {
            lo[k] = lo[k].min(bb[k]);
            hi[k] = hi[k].max(bb[k + 2]);
        }
        polys.push((bb, poly));
    }
    let mut water = Vec::new();
    let mut water_cell = 0.0;
    if lo[0] <= hi[0] {
        water_cell = (((hi[0] - lo[0]).max(hi[1] - lo[1])) / 220.0).clamp(10.0, 40.0).round();
        let (i0, i1) = (crate::core::floor(lo[0] / water_cell) as i64, crate::core::ceil(hi[0] / water_cell) as i64);
        let (j0, j1) = (crate::core::floor(lo[1] / water_cell) as i64, crate::core::ceil(hi[1] / water_cell) as i64);
        for j in j0..j1 {
            let y = (j as f64 + 0.5) * water_cell;
            let mut run: Option<i64> = None;
            for i in i0..=i1 {
                let q = [(i as f64 + 0.5) * water_cell, y];
                let wet = i < i1 && polys.iter().any(|(bb, poly)| q[0] >= bb[0] && q[0] <= bb[2] && q[1] >= bb[1] && q[1] <= bb[3] && contains(poly, q)) && site.wet(q);
                match (wet, run) {
                    (true, None) => run = Some(i),
                    (false, Some(a)) => {
                        water.push([a as f64 * water_cell + center[0], i as f64 * water_cell + center[0], j as f64 * water_cell + center[1]]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
    }
    Standin { center, walls, gates, streets, bridges, patches, water, water_cell }
}
