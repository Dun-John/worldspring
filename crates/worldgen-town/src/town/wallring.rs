//! The town wall's runs (`plan_town`'s phase after the shore and the river): the sides of the
//! town's patches facing the outskirts, chained corner to corner.

use super::*;

/// Wall ring: edges of city patches that face the outskirts (open where water guards: not on a
/// side along the river; one the river crosses has its wall cut at the banks, `walls_off_river`).
/// Returns its chains (corners, closed), longest first; every corner on them; and the wall's own
/// sides (corner pairs, lower first).
#[allow(clippy::type_complexity)]
pub(super) fn chains(
    site: &Site,
    mesh: &Mesh,
    n_inner: usize,
    wet: &[bool],
    by_water: &[Vec<bool>],
    usable: impl Fn(usize) -> bool,
    is_river_edge: impl Fn(usize, usize) -> bool,
) -> (Vec<(Vec<usize>, bool)>, std::collections::BTreeSet<usize>, std::collections::BTreeSet<(usize, usize)>) {
    let inner = |j: i32| j >= 0 && (j as usize) < n_inner;
    let along_river = |a: usize, b: usize| is_river_edge(a, b) && [0.25, 0.75].iter().all(|&t| site.near_river(lerp(mesh.pos[a], mesh.pos[b], t), 2.0));
    let mut edge_next: BTreeMap<usize, usize> = BTreeMap::new();
    for i in (0..n_inner).filter(|&i| usable(i)) {
        let (face, lb) = (&mesh.faces[i], &mesh.labels[i]);
        let m = face.len();
        for e in 0..m {
            let nb = lb[e];
            let outside = !inner(nb) || wet[nb as usize];
            let (a, b) = (face[e], face[(e + 1) % m]);
            if outside && !by_water[i][e] && !along_river(a, b) {
                edge_next.insert(a, b);
            }
        }
    }
    let targets: std::collections::BTreeSet<usize> = edge_next.values().copied().collect();
    let mut used: std::collections::BTreeSet<usize> = Default::default();
    let mut chains: Vec<(Vec<usize>, bool)> = Vec::new();
    // Open chains (broken by water) start where nothing leads in; then closed loops.
    let starts: Vec<usize> = edge_next.keys().copied().filter(|k| !targets.contains(k)).chain(edge_next.keys().copied()).collect();
    for start in starts {
        if used.contains(&start) {
            continue;
        }
        let mut chain = vec![start];
        let mut cur = start;
        let mut closed = false;
        while let Some(&nx) = edge_next.get(&cur) {
            if !used.insert(cur) {
                break;
            }
            if nx == start {
                closed = true;
                break;
            }
            chain.push(nx);
            cur = nx;
        }
        // (A run of one side, between the water at both ends, if a long one.)
        if chain.len() >= 3 || (chain.len() == 2 && dist(mesh.pos[chain[0]], mesh.pos[chain[1]]) >= 50.0) {
            chains.push((chain, closed));
        }
    }
    chains.sort_by_key(|(c, _)| std::cmp::Reverse(c.len()));
    let ring: std::collections::BTreeSet<usize> = chains.iter().flat_map(|(c, _)| c.iter().copied()).collect();
    // (The wall's own sides: pairs of corners next along a chain.)
    let ring_edges: std::collections::BTreeSet<(usize, usize)> = chains
        .iter()
        .flat_map(|(c, closed)| {
            let n = c.len();
            (0..if *closed { n } else { n - 1 }).map(move |k| (c[k].min(c[(k + 1) % n]), c[k].max(c[(k + 1) % n])))
        })
        .collect();
    (chains, ring, ring_edges)
}
