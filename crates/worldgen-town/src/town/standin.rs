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
    Standin { center, walls, gates, streets, bridges, patches }
}
