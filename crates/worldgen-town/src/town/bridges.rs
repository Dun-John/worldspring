//! The streets' bridges (`build_town`'s first phase, before the patches are materialised).

use super::*;

/// Bridges: the river vertex back on the bank-to-bank line, and the deck along it; then the
/// lined bridge over the main streets' (`lined_bridge`).
pub(super) fn lay(site: &Site, s: &Settlement, plan: &TownPlan, mesh: &mut Mesh, pieces: &mut stairs::Pieces, l: &mut Layout) {
    let mut bridge_streets: Vec<(Vec<P>, bool)> = Vec::new();
    // (R5 M20: the main streets' bridges with their water's extent, for the lined bridge.)
    let mut over_water: Vec<(f64, P, P, f64, f64)> = Vec::new();
    for &((a, v, b), main) in &plan.bridged {
        mesh.pos[v] = river_crossing(site, mesh.pos[a], mesh.pos[b]);
        bridge_streets.push((vec![mesh.pos[a], mesh.pos[v], mesh.pos[b]], main));
    }
    for (&((a, v, b), _), (pts, main)) in plan.bridged.iter().zip(&bridge_streets) {
        let dir = sub(pts[2], pts[0]);
        let dl = len(dir).max(1e-9);
        let u = mul(dir, 1.0 / dl);
        // Span from bank to bank along the street, plus a landing on each side.
        let (fwd, back) = (site.dry_reach(pts[1], u, 0.0, 1.0, 0.0) + 14.0, site.dry_reach(pts[1], u, 0.0, -1.0, 0.0) + 14.0);
        // A span far wider than the river means the street runs along the water, not over it.
        let hw = site.nearest_river(pts[1]).map_or(0.0, |r| r.2);
        if fwd + back > 4.0 * hw + 80.0 {
            continue;
        }
        // Cities build in stone, as wide as the street's gap between blocks (12 ft a side).
        let width = if s.tier >= Tier::City { 24.0 } else if *main { 16.0 } else { 12.0 };
        // Long enough that every corner stands on the bank, crossing at a slant too.
        let (fwd, back) = (site.dry_reach(pts[1], u, width, 1.0, fwd - 14.0) + 14.0, site.dry_reach(pts[1], u, width, -1.0, back - 14.0) + 14.0);
        let c = add(pts[1], mul(u, 0.5 * (fwd - back)));
        l.bridges.push(rect(c, u, fwd + back, width));
        if *main {
            let (wf, wb) = (site.dry_reach(pts[1], u, 0.0, 1.0, 0.0), site.dry_reach(pts[1], u, 0.0, -1.0, 0.0));
            over_water.push((wf + wb, pts[1], u, wf, wb));
        }
        pieces.bridge((a, v, b), pts, if *main { 18.0 } else { 12.0 }, (!main).then_some(l.roads.len()));
        if !main {
            l.roads.push((pts.clone(), 4, 12.0));
        }
    }

    lined_bridge(site, s, &over_water, l);
}
