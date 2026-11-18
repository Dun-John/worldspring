//! The per-building marks of `pack_sites`: one building's polygons (through `push`) and lines.

use std::cell::Cell;

use super::bell_cote;
use crate::World;
use crate::t0::T0;

/// One building's marks at a tile: `whole` is what `push` tests ownership by (see `pack_sites`), `owns` whether the
/// tile holds a point, `drawn` whether the layout is one building drawn by hand.
#[allow(clippy::too_many_arguments)]
pub(super) fn building_marks(world: &World, t0: &T0, l: &crate::town::Layout, b: &crate::town::Building, drawn: bool, owns: &impl Fn([f64; 2]) -> bool, whole: &Cell<Option<bool>>, push: &mut impl FnMut(&[[f64; 2]], u32), lines: &mut Vec<Vec<[f64; 4]>>) {
        // v63 underground: a ruin as its broken walls, rubble and floor (`battlemap::ruin_plan`).
        if b.structure == crate::town::Structure::Ruin {
            whole.set(Some(owns(crate::town::geom::centroid(&b.poly))));
            for (kind, poly) in crate::battlemap::ruin_plan::footprint(world.stream("battlemap"), &b.poly) {
                push(&poly, kind | (b.ward as u32) << 8);
            }
            whole.set(None);
            return;
        }
        // (A building whose middle is in another tile draws nothing here (every branch below
        // pushes only in the tile holding its middle), unless it can draw lines, owned by
        // their own middles: a palace's forecourt walls and piers, a keep's corner towers.)
        if !owns(crate::town::geom::centroid(&b.poly)) && !crate::town::palace::is_palace(b) && !crate::interior::battlements(b).is_some_and(|t| t > 0.0 && b.poly.len() == 4) {
            return;
        }
        let kind = match b.structure {
            crate::town::Structure::Roofed => 0,
            crate::town::Structure::Ruin => 4,
            crate::town::Structure::Open => 5,
        };
        // (Its roof's material in bits 20-23, its storeys capped at 15 below them.)
        let k = crate::town::roof::look(&l, b);
        // (A building drawn by hand: bit 7 of its ward byte, so the map shows its roof's own colour.)
        let attr = kind | (b.ward as u32 | (drawn as u32) << 7) << 8 | (b.floors.min(15) as u32) << 16 | (k.material as u32 & 15) << 20 | (b.func.is_some() as u32) << 24;
        // (A plain building's colour variant in bits 29-31; a keep's pieces use them for their tops.)
        let plain = attr | (k.shade as u32) << 29;
        // (R5 towns, 115: a yurt's felt (piece code 11; its shade the felt, `yurts::felt`).)
        let plain = if k.shape == crate::town::roof::Shape::Round && b.poly.len() == crate::town::roof::ROUND_SIDES && crate::town::yurts::is_steppe_home(&l, b) { plain | 11 << 25 } else { plain };
        // (R5 towns, 114: a felt hall's too, in a light felt: cream, off-white or warm cream by its id.)
        let plain = if k.shape == crate::town::roof::Shape::Round && crate::town::roof::felt_built(b).is_some() { (plain & !(7 << 29)) | 11 << 25 | [0u32, 1, 3][b.id as usize % 3] << 29 } else { plain };
        // A keep of pieces: each piece (convex, outlined), its towers over its body;
        // each with its kind + 1 (bits 25-28: 1 body, 2 round tower, 3 square tower, 4
        // forebuilding, 5 wing) and how it is topped (bits 29-31: `town::keep_tops`, its
        // donjon's end in bit 31).
        // A temple of pieces (`town::temple`): its body, transept and apse, each outlined
        // (all in the tile holding its middle).
        if crate::town::temple::shaped(b) {
            whole.set(Some(owns(crate::town::geom::centroid(&b.poly))));
            for p in &b.parts {
                push(&p.poly, plain);
            }
            whole.set(None);
            return;
        }
        // A town hall's belfry (`town::belfry`): the hall as any building, then the tower's
        // long shadow down its roof (piece 10, the tower two storeys over it and its spire's
        // point beyond), the tower under its verdigris spire (piece 8), the open bell lantern
        // on top (piece 9).
        if let Some(t) = crate::town::belfry::of(b) {
            use crate::town::geom::{add, hull, mean, mul, sub};
            whole.set(Some(owns(crate::town::geom::centroid(&b.poly))));
            push(&b.parts[0].poly, plain);
            let c = mean(&t.poly);
            let mut sh: Vec<[f64; 2]> = t.poly.iter().flat_map(|&p| [p, add(p, [11.0, 11.0])]).collect();
            sh.push(add(c, [21.0, 21.0]));
            push(&hull(sh), attr | 10 << 25);
            push(&t.poly, attr | 8 << 25);
            push(&t.poly.iter().map(|&p| add(c, mul(sub(p, c), 0.3))).collect::<Vec<[f64; 2]>>(), attr | 9 << 25);
            whole.set(None);
            return;
        }
        if !b.parts.is_empty() {
            let c = crate::town::geom::centroid(&b.poly);

            whole.set(Some(owns(c)));
            let tops = crate::town::keep_tops(b, world.stream("keep.tops"));
            for (p, &top) in b.parts.iter().zip(&tops) {
                use crate::town::PartKind as K;
                let code = match p.kind {
                    K::Body => 1u32,
                    K::Tower { round: true } => 2,
                    K::Tower { round: false } => 3,
                    K::Forebuilding => 4,
                    K::Wing => 5,
                    K::Apse => 1,
                };
                push(&p.poly, attr | code << 25 | (top as u32) << 29);
            }
            whole.set(None);
            return;
        }
        // The capital's palace round its court: a range along each side, the court a lawn
        // with its gravel walks and its fountain, as on the battlemap (all in the tile
        // holding its middle).
        // (A monastery's cloister likewise: its garth, its walks and its well.)
        if let Some(court) = crate::town::palace::merges(&l, b).then(|| crate::interior::court_plan(world, t0, &l, b)).flatten() {
            whole.set(Some(owns(crate::town::geom::centroid(&b.poly))));
            let q = &court.court;
            // (A coaching inn's yard cobbled, its trough, barrels and cart left to the
            // battlemap.)
            push(q, if court.paved { 1 } else { 13 });
            // Its forecourt paved, its low wall round it, the gate between two piers.
            if let Some(fc) = &court.forecourt {
                push(&fc.yard, 1);
                let (wall, pier) = (crate::interior::FORE_WALL_FT, crate::interior::FORE_PIER_FT);
                for &(a, e) in &fc.walls {
                    lines.push(vec![[a[0], a[1], wall, 0.0], [e[0], e[1], wall, 0.0]]);
                }
                for p in &fc.piers {
                    let (a, e) = (crate::town::geom::sub(*p, crate::town::geom::mul(fc.along, 0.5 * pier)), crate::town::geom::add(*p, crate::town::geom::mul(fc.along, 0.5 * pier)));
                    lines.push(vec![[a[0], a[1], pier, 3.0], [e[0], e[1], pier, 3.0]]);
                }
            }
            for (kind, sq, _) in &court.pieces {
                match *kind {
                    "garden_path" => push(sq, 12),
                    "fountain" => push(&crate::town::geom::circle(crate::town::geom::mean(sq), 0.5 * crate::town::geom::dist(sq[0], sq[1]), 12), 7),
                    "cloister_well" => push(&crate::town::geom::circle(crate::town::geom::mean(sq), 0.35 * crate::town::geom::dist(sq[0], sq[1]).min(crate::town::geom::dist(sq[1], sq[2])), 10), 7),
                    _ => {}
                }
            }
            for k in 0..4 {
                push(&[b.poly[k], b.poly[(k + 1) % 4], q[(k + 1) % 4], q[k]], plain);
            }
            // (A coaching inn's carriage arch: the yard's cobbles out through its mouth to
            // the street.)
            for (_, sq, _) in court.pieces.iter().filter(|p| p.0 == "arch") {
                push(sq, 1);
            }
            // A monastery's church over its range (piece 6: roofed higher, darker), its
            // bell-cote on its west end (piece 7).
            if let Some(c) = &court.church {
                push(c, attr | 6 << 25);
                push(&bell_cote(c), attr | 7 << 25);
            }
            whole.set(None);
            return;
        }
        // A manor: its main range and its cross wing over the family end, each outlined
        // (all in the tile holding its middle).
        if let Some((main, wing)) = crate::interior::cross_wing_of(world, t0, &l, b) {
            whole.set(Some(owns(crate::town::geom::centroid(&b.poly))));
            push(&main, plain);
            push(&wing, plain);
            whole.set(None);
            return;
        }
        // (R5 towns, 114: a felt hall's roof as a yurt's: its felt, an inner ring where the
        // girth rope runs (outlined in the same felt) and the crown over the smoke hole
        // (piece code 12).)
        if k.shape == crate::town::roof::Shape::Round && crate::town::roof::felt_built(b).is_some() {
            use crate::town::geom::{add, centroid, mul, sub};
            let c = centroid(&b.poly);
            whole.set(Some(owns(c)));
            push(&b.poly, plain);
            push(&b.poly.iter().map(|&p| add(c, mul(sub(p, c), 0.62))).collect::<Vec<[f64; 2]>>(), plain);
            push(&b.poly.iter().map(|&p| add(c, mul(sub(p, c), 0.2))).collect::<Vec<[f64; 2]>>(), (plain & !(15 << 25)) | 12 << 25);
            whole.set(None);
            return;
        }
        // (R5 M20: an islet's watchtower or a breakwater's lighthouse as a tower mark, drawn a
        // little over its size: its long shadow to the south-east (piece 10), its stone wall
        // head (a keep's open top, piece 4), the pale walk inside its parapet (piece 9), and on
        // a lighthouse its lamp, a warm disc (piece 13).)
        let lighthouse = crate::town::harbour::is_lighthouse(b);
        if lighthouse || crate::town::is_watchtower(b) {
            use crate::town::geom::{add, centroid, hull, mul, sub};
            let c = centroid(&b.poly);
            let at = |k: f64| b.poly.iter().map(|&p| add(c, mul(sub(p, c), k))).collect::<Vec<[f64; 2]>>();
            let base = attr & !(15 << 25);
            whole.set(Some(owns(c)));
            let tower = at(1.2);
            let reach = 0.9 * crate::town::geom::dist(c, tower[0]);
            push(&hull(tower.iter().flat_map(|&p| [p, add(p, [reach, reach])]).collect()), base | 10 << 25);
            push(&tower, base | 4 << 25);
            push(&at(0.78), base | 9 << 25);
            if lighthouse {
                push(&at(0.38), base | 13 << 25);
            }
            whole.set(None);
            return;
        }
        push(&b.poly, plain);
        // A keep's round corner towers (as the battlemap draws them), standing a
        // little proud of its walls; each tile keeps the ones it owns.
        if let Some(t) = crate::interior::battlements(b)
            && t > 0.0
            && b.poly.len() == 4
        {
            use crate::town::geom::{add, dist, mul, sub};
            let p = &b.poly;
            let (la, lb) = (dist(p[0], p[1]), dist(p[0], p[3]));
            let rt = (t * 0.6).max((la.min(lb) * 0.11).min(13.0));
            for k in 0..4 {
                let (c, n, q) = (p[k], p[(k + 1) % 4], p[(k + 3) % 4]);
                let into = add(mul(sub(n, c), 0.55 * rt / dist(c, n).max(1e-9)), mul(sub(q, c), 0.55 * rt / dist(c, q).max(1e-9)));
                let at = add(c, into);
                lines.push(vec![[at[0], at[1], 2.0 * rt, 1.0], [at[0] + 0.01, at[1], 2.0 * rt, 1.0]]);
            }
        }
}
