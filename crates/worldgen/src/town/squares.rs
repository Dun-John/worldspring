//! A town's other open spaces, besides its main square and its quays: a widened crossroads, a
//! well square, a fountain square, a small green, a temple's close. Each town draws a mix, each
//! a different kind: a town sometimes one, a city one or two, a metropolis two or three.
//!
//! A square opens where three or more streets meet between blocks built on lots: each patch round
//! the junction gives up its corner inside a chord square to the corner's bisector, at most 28 to
//! 64 ft out by kind, its ends no further along a street than 1.25 times that or 0.4 of the street
//! (so where three streets meet evenly the chord stands about 0.6 of it out), streets and all; the
//! lots there are trimmed back to the chord once laid out, so nothing else in the blocks moves.
//! A temple's close opens ground round the temple and its churchyard in the middle of its block,
//! inside a ring of houses a lot deep along its streets, cut back to face it (none where houses
//! would line under 65% of them).
//!
//! `plan_town` picks them on the plan (`plan`); `build_town` opens them on the corners as they
//! stand, moved by the ward editor or not (`cuts`), with the editor's own squares and closes
//! (`spots`), so a town laid out anew by hand has them as well.

use std::collections::BTreeMap;

use super::catalog::Ward;
use super::geom::*;
use super::mesh::Mesh;
use super::wards::Changes;
use super::{Building, Layout, Plaza, PlazaKind, Site, Structure, TownPlan};
use crate::t0::settle::Tier;

/// The kinds a square where streets meet can be, as named by hand (`world::SQUARE_KINDS`).
pub const CORNER_KINDS: [PlazaKind; 4] = [PlazaKind::Crossroads, PlazaKind::WellSquare, PlazaKind::FountainSquare, PlazaKind::Green];

/// The corner kind with this name.
pub fn corner_kind(name: &str) -> Option<PlazaKind> {
    CORNER_KINDS.iter().copied().find(|k| k.name() == name)
}

/// Wards built on lots, whose blocks a square may cut into (not barracks: their halls would lose
/// their corners).
fn built(w: Option<Ward>) -> bool {
    matches!(w, Some(Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Docks | Ward::Temple))
}

/// Whether a square can open at corner `v` with these wards and district groups: inside the
/// town, off the wall, its gates and the water, every patch round it built on lots, and three
/// streets or more meeting there (patches of one district have none between them).
pub(super) fn junction(plan: &TownPlan, ward: &[Option<Ward>], group: &[usize], v: usize) -> bool {
    let Some(faces) = plan.vf.get(v) else { return false };
    if faces.len() < 3 || plan.pinned[v] || plan.gates.contains(&v) || plan.wall_runs.iter().any(|(c, _, _)| c.contains(&v)) {
        return false;
    }
    if !faces.iter().all(|&f| plan.usable(f) && built(ward[f])) {
        return false;
    }
    // Each edge from the corner, once: the one each patch round it leaves the corner by.
    let streets = faces
        .iter()
        .filter(|&&f| {
            let Some(k) = plan.mesh.faces[f].iter().position(|&u| u == v) else { return false };
            let nb = plan.mesh.labels[f][k];
            nb < 0 || group[f] == usize::MAX || group[f] != group[nb as usize]
        })
        .count();
    streets >= 3
}

/// How far out from its corner a square's chords are cut (ft), before `cuts` keeps them short
/// of the next corners.
fn reach(plan: &TownPlan, v: usize, kind: PlazaKind) -> f64 {
    let t = plan.roll(v, 61);
    match kind {
        PlazaKind::Crossroads => 28.0 + 8.0 * t,
        PlazaKind::WellSquare => 34.0 + 10.0 * t,
        PlazaKind::FountainSquare => 38.0 + 10.0 * t,
        _ => 50.0 + 14.0 * t,
    }
}

/// The town's other open spaces as planned: squares where streets meet (corner, kind) and the
/// temple patches kept as a close. How many by tier, each a different kind drawn by weight (a
/// town's are mostly crossroads and wells, a metropolis's fountains); each kind goes where it
/// suits (crossroads and wells on the main streets, fountains among the rich, greens among the
/// common folk), on level ground clear of the main square, the river and one another, where the
/// streets from the corner are long enough to leave room (at least 100 ft), never at the corner of
/// a close.
pub(super) fn plan(plan: &TownPlan, site: &Site, tier: Tier) -> (Vec<(usize, PlazaKind)>, Vec<usize>) {
    use PlazaKind as K;
    let more = plan.roll(0, 60) < 0.5;
    let want = match tier {
        Tier::Village => 0,
        Tier::Town => more as usize,
        Tier::City => 1 + more as usize,
        Tier::Metropolis => 2 + more as usize,
    };
    let mut deck: Vec<(PlazaKind, f64)> = match tier {
        Tier::Town => vec![(K::Crossroads, 3.0), (K::WellSquare, 3.0), (K::Green, 2.0), (K::FountainSquare, 1.0), (K::TempleClose, 1.0)],
        Tier::City => vec![(K::Crossroads, 2.0), (K::WellSquare, 2.0), (K::FountainSquare, 2.0), (K::Green, 2.0), (K::TempleClose, 2.0)],
        _ => vec![(K::Crossroads, 1.0), (K::WellSquare, 2.0), (K::FountainSquare, 3.0), (K::Green, 2.0), (K::TempleClose, 2.0)],
    };
    let sp = plan.spacing;
    let plaza: Vec<P> = plan.plaza.map(|pz| plan.mesh.face_pts(pz)).unwrap_or_default();
    // Level enough for a square: the ground 40 ft round the corner within 12 in 100.
    let level = |p: P| {
        let hs: Vec<f64> = [[0.0, 0.0], [40.0, 0.0], [-40.0, 0.0], [0.0, 40.0], [0.0, -40.0]].iter().map(|d| site.height(add(p, *d))).collect();
        let (lo, hi) = hs.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
        (hi - lo) / 80.0 < 0.12
    };
    let corners: Vec<usize> = (0..plan.mesh.pos.len())
        .filter(|&v| {
            let p = plan.mesh.pos[v];
            junction(plan, &plan.ward, &plan.group, v)
                && plaza.iter().all(|q| dist(*q, p) > 0.6 * sp)
                && !site.wet(p)
                && !site.near_river(p, 60.0)
                && level(p)
                && shortest(plan, &plan.mesh, v) >= 100.0
        })
        .collect();
    let temples: Vec<usize> = (0..plan.n_inner)
        .filter(|&i| plan.usable(i) && plan.ward[i] == Some(Ward::Temple) && area(&plan.mesh.face_pts(i)).abs() > 15_000.0 && site.grade(&plan.mesh.face_pts(i)) < 0.15)
        .collect();
    let main = |v: usize| plan.vf[v].iter().any(|&f| {
        let face = &plan.mesh.faces[f];
        let m = face.len();
        (0..m).any(|k| (face[k] == v || face[(k + 1) % m] == v) && plan.main_edge(face[k], face[(k + 1) % m]))
    });
    let among = |v: usize, ws: &[Ward]| plan.vf[v].iter().any(|&f| plan.ward[f].is_some_and(|w| ws.contains(&w)));
    let (mut squares, mut closes): (Vec<(usize, PlazaKind)>, Vec<usize>) = (Vec::new(), Vec::new());
    let mut draw = 0;
    while squares.len() + closes.len() < want && !deck.is_empty() {
        // A kind by weight, not drawn before.
        let total: f64 = deck.iter().map(|d| d.1).sum();
        let mut pick = plan.roll(draw, 62) * total;
        draw += 1;
        let mut k = deck.len() - 1;
        for (j, d) in deck.iter().enumerate() {
            if pick < d.1 {
                k = j;
                break;
            }
            pick -= d.1;
        }
        let (kind, _) = deck.remove(k);
        if kind == K::TempleClose {
            let free = |p: usize| plan.mesh.faces[p].iter().all(|v| !squares.iter().any(|s| s.0 == *v));
            if let Some(&p) = temples.iter().filter(|&&p| free(p)).max_by(|&&a, &&b| plan.roll(a, 63).total_cmp(&plan.roll(b, 63)).then(b.cmp(&a))) {
                closes.push(p);
            }
            continue;
        }
        // Where it suits, a little nearer the middle of the town, clear of the others.
        let score = |v: usize| {
            let p = plan.mesh.pos[v];
            let fit = match kind {
                K::Crossroads | K::WellSquare => 0.6 * main(v) as u8 as f64,
                K::FountainSquare => 0.6 * among(v, &[Ward::Noble, Ward::Merchant, Ward::Temple]) as u8 as f64,
                _ => 0.6 * among(v, &[Ward::Common, Ward::Craft, Ward::Slum]) as u8 as f64 - 0.4 * main(v) as u8 as f64,
            };
            plan.roll(v, 64 + kind as i64) + fit - 0.25 * (len(p) / (3.0 * sp)).min(1.5)
        };
        let best = corners
            .iter()
            .copied()
            .filter(|&v| squares.iter().all(|&(u, _)| dist(plan.mesh.pos[u], plan.mesh.pos[v]) > 1.4 * sp) && !at_close(plan, v, &closes))
            .max_by(|&a, &b| score(a).total_cmp(&score(b)).then(b.cmp(&a)));
        if let Some(v) = best {
            squares.push((v, kind));
        }
    }
    (squares, closes)
}

/// Whether corner `v` is a corner of one of these closes' patches.
pub(super) fn at_close(plan: &TownPlan, v: usize, closes: &[usize]) -> bool {
    plan.vf.get(v).is_some_and(|fs| fs.iter().any(|f| closes.contains(f)))
}

/// The shortest street from corner `v` (ft) on the mesh as it stands.
fn shortest(plan: &TownPlan, mesh: &Mesh, v: usize) -> f64 {
    let p = mesh.pos[v];
    plan.vf[v]
        .iter()
        .flat_map(|&f| {
            let face = &mesh.faces[f];
            let m = face.len();
            face.iter().position(|&u| u == v).map(|k| dist(mesh.pos[face[(k + m - 1) % m]], p).min(dist(mesh.pos[face[(k + 1) % m]], p)))
        })
        .fold(f64::MAX, f64::min)
}

/// The squares and closes a town is built with: as planned, with those set or taken away by hand
/// (`ch`), each where it still fits with the wards as they now are (a square at a junction, not
/// at a close's corner; a close on a temple's patch).
/// `fits`: whether a close is laid out on a patch (its temple and churchyard fit there, ringed by
/// houses along at least 65% of the block's streets).
pub(super) fn spots(plan: &TownPlan, ch: &Changes, ward: &[Option<Ward>], group: &[usize], fits: &dyn Fn(usize) -> bool) -> (Vec<(usize, PlazaKind)>, Vec<usize>) {
    let mut sq: BTreeMap<usize, PlazaKind> = plan.squares.iter().copied().collect();
    for (&v, &k) in &ch.squares {
        match k {
            Some(k) => sq.insert(v, k),
            None => sq.remove(&v),
        };
    }
    let mut cl: Vec<usize> = plan.closes.iter().copied().filter(|p| ch.close.get(p) != Some(&false)).collect();
    cl.extend(ch.close.iter().filter(|(p, on)| **on && !plan.closes.contains(p)).map(|(&p, _)| p));
    cl.sort();
    cl.retain(|&p| ward.get(p) == Some(&Some(Ward::Temple)) && fits(p));
    let squares = sq.into_iter().filter(|&(v, _)| junction(plan, ward, group, v) && !at_close(plan, v, &cl)).collect();
    (squares, cl)
}

/// How far (ft) the open ground of a temple's close reaches past its temple and churchyard.
const CLOSE_OPEN_FT: f64 = 30.0;

/// How deep (ft) the ring of houses round a close is left along the block's streets: about a lot
/// deep, less where nothing fits so; none shallower.
const CLOSE_RING_FT: [f64; 3] = [38.0, 31.0, 24.0];

/// A temple's close as it would be laid out on `block` (local ft): the temple (a hall along the
/// block's axes, its front toward the middle of the town, room for a cross before it), its
/// churchyard behind it (square to the hall), and the open ground `CLOSE_OPEN_FT` round them, all
/// inside a ring of houses along the block's streets (`streets`: lines as `build_block` takes them;
/// `CLOSE_RING_FT`). `t` (0–1) sizes the hall. None when they don't fit.
pub(super) fn close_plan(site: &Site, block: &[P], streets: &[(P, f64)], t: f64) -> Option<(Vec<P>, Vec<P>, Vec<P>)> {
    CLOSE_RING_FT.iter().find_map(|&ring| close_in(site, block, streets, t, ring, true))
}

/// `close_plan` with a ring of houses `ring` ft deep: none where half the ring's ground along the
/// streets won't take a house, or there are no streets (with `houses`).
fn close_in(site: &Site, block: &[P], streets: &[(P, f64)], t: f64, ring: f64, houses: bool) -> Option<(Vec<P>, Vec<P>, Vec<P>)> {
    let inner = inset(block, &vec![ring; block.len()]);
    if inner.len() < 3 {
        return None;
    }
    let (m, ccw) = (block.len(), area(block) > 0.0);
    let (mut lots, mut ok) = (0, 0);
    for e in 0..m {
        let (a, b) = (block[e], block[(e + 1) % m]);
        let el = dist(a, b);
        let u = mul(sub(b, a), 1.0 / el.max(1e-9));
        let inward = if ccw { [-u[1], u[0]] } else { [u[1], -u[0]] };
        let mid = lerp(a, b, 0.5);
        let n = if streets.iter().any(|&(nr, c)| (dot(mid, nr) - c).abs() < 1.0) { (el / 40.0) as usize } else { 0 };
        for j in 0..n {
            let c = add(add(a, mul(u, (j as f64 + 0.5) * el / n as f64)), mul(inward, 0.5 * ring));
            lots += 1;
            ok += (contains(block, c) && site.buildable(&super::rect(c, u, 24.0, ring - 6.0))) as usize;
        }
    }
    if houses && (ok * 2 < lots || lots == 0) {
        return None;
    }
    let o = obb(&inner);
    let (u, v) = (o.axis, [-o.axis[1], o.axis[0]]);
    let c0 = centroid(&inner);
    let toward = if len(c0) > 1.0 { mul(c0, -1.0 / len(c0)) } else { u };
    let front = [u, mul(u, -1.0), v, mul(v, -1.0)].into_iter().max_by(|a, b| dot(*a, toward).total_cmp(&dot(*b, toward))).unwrap_or(u);
    let lengthwise = dot(front, u).abs() > 0.5;
    let (hl, hw) = ((70.0 + 30.0 * t).min(0.45 * o.long), (40.0 + 14.0 * t).min(0.45 * o.short));
    let inside = |poly: &[P]| poly.iter().all(|p| contains(&inner, *p));
    for f in [1.0, 0.85, 0.72] {
        // Depth along the front, width across it: the hall, then the churchyard behind it.
        let (hd, hs) = if lengthwise { (hl * f, hw * f) } else { (hw * f, hl * f) };
        if hd < 30.0 || hs < 26.0 {
            break;
        }
        let (gd, gs) = ((45.0 * f).max(24.0), (hs + 16.0).min(70.0));
        let total = 14.0 + hd + 8.0 + gd;
        for shift in [0.0, 0.15, -0.15, 0.3, -0.3] {
            let base = add(c0, mul(front, shift * total));
            let hall = super::rect(add(base, mul(front, 0.5 * total - 14.0 - 0.5 * hd)), front, hd, hs);
            let ch = super::rect(add(base, mul(front, -0.5 * total + 0.5 * gd)), front, gd, gs);
            let cross = add(base, mul(front, 0.5 * total - 6.0));
            if inside(&hall) && inside(&ch) && contains(&inner, cross) && site.buildable(&hall) && site.dry_outline(&ch, 10.0) {
                let mut open = super::rect(base, front, total + 2.0 * CLOSE_OPEN_FT, hs.max(gs) + 2.0 * CLOSE_OPEN_FT);
                for (n, c) in super::edge_lines(&inner) {
                    open = clip_plain(&open, n, c);
                }
                let open = clean(&open);
                return (open.len() >= 3).then_some((hall, ch, open));
            }
        }
    }
    None
}

/// A temple's close on `block` (local ft; `close_plan`), whose lots are laid out from `first` in
/// `l.buildings` and alleys from `alleys` in `l.roads`. The houses round it are cut back to face it
/// (to the ring's depth, their street fronts kept; no slivers or wedges) or go; the alleys into it
/// go, and those that then lead nowhere. False, with nothing changed, when it doesn't fit.
pub(super) fn close(site: &Site, block: &[P], streets: &[(P, f64)], t: f64, first: usize, alleys: usize, l: &mut Layout) -> bool {
    let (Some(temple), Some(yard)) = (super::catalog::index_of("temple"), super::catalog::index_of("graveyard")) else { return false };
    let Some((hall, ch, open)) = close_plan(site, block, streets, t) else { return false };
    let clear = inset(&open, &vec![0.1; open.len()]);
    // The open ground's edges as lines to cut back to (outward normal, offset 1 ft out).
    let lines: Vec<(P, f64)> = super::edge_lines(&open).into_iter().map(|(n, c)| (n, c + 1.0)).collect();
    let mut k = first;
    while k < l.buildings.len() {
        let poly = &l.buildings[k].poly;
        if clear.len() < 3 || !super::overlaps_or_crosses(poly, &clear) {
            k += 1;
            continue;
        }
        let whole = area(poly).abs();
        let best = lines
            .iter()
            .map(|&(n, c)| clean(&clip_plain(poly, mul(n, -1.0), -c)))
            .filter(|kept| kept.len() >= 3 && !super::overlaps_or_crosses(kept, &clear))
            .max_by(|a, b| area(a).abs().total_cmp(&area(b).abs()));
        // (A lot deeper than the ring is cut back to it, its street front kept: no less than a
        // small house, and no sliver.)
        match best.filter(|kept| area(kept).abs() >= 150.0_f64.max(0.2 * whole) && obb(kept).short >= 16.0 && !wedge(kept)) {
            Some(kept) => {
                l.buildings[k].poly = kept;
                l.buildings[k].parts.clear();
                k += 1;
            }
            None => {
                l.buildings.remove(k);
            }
        }
    }
    // (No wedge faces the close, cut back or not.)
    let mut k = first;
    while k < l.buildings.len() {
        if wedge(&l.buildings[k].poly) {
            l.buildings.remove(k);
        } else {
            k += 1;
        }
    }
    // (An alley ends on a street or on the side of one it branched from, half an alley away.)
    let mut kept: Vec<(Vec<P>, u8, f64)> = l.roads.drain(alleys..).filter(|(pts, _, _)| pts.windows(2).all(|s| !touches(s[0], s[1], &open))).collect();
    let tol = super::ALLEY_HALF_FT + 1.5;
    loop {
        let ends = |me: usize, p: P, kept: &[(Vec<P>, u8, f64)]| {
            (0..block.len()).any(|e| seg_dist(p, block[e], block[(e + 1) % block.len()]) < tol)
                || kept.iter().enumerate().any(|(j, (q, _, _))| j != me && q.windows(2).any(|s| seg_dist(p, s[0], s[1]) < tol))
        };
        let led: Vec<bool> = (0..kept.len()).map(|j| ends(j, kept[j].0[0], &kept) && ends(j, kept[j].0[kept[j].0.len() - 1], &kept)).collect();
        if led.iter().all(|&x| x) {
            break;
        }
        let mut j = 0;
        kept.retain(|_| {
            j += 1;
            led[j - 1]
        });
    }
    l.roads.extend(kept);
    l.buildings.push(temple_part(site, hall, temple, Structure::Roofed));
    l.buildings.push(temple_part(site, ch, yard, Structure::Open));
    l.plazas.push(Plaza { pts: open, kind: PlazaKind::TempleClose });
    true
}

/// A temple alone on `block` (local ft), where `close_plan` would put a close's, houses round it
/// or not (none where it doesn't fit even 14 ft from the streets).
pub(super) fn lone_temple(site: &Site, block: &[P], t: f64, l: &mut Layout) {
    let hall = CLOSE_RING_FT.iter().chain(&[14.0]).find_map(|&ring| close_in(site, block, &[], t, ring, false)).map(|x| x.0);
    let (Some(temple), Some(hall)) = (super::catalog::index_of("temple"), hall) else { return };
    l.buildings.push(temple_part(site, hall, temple, Structure::Roofed));
}

/// A temple's own building on `poly`: its hall or churchyard (function `func`), roofed or open.
fn temple_part(site: &Site, poly: Vec<P>, func: usize, structure: Structure) -> Building {
    let pad_ft = site.pad(&poly);
    Building { poly, ward: Ward::Temple, func: Some(func as u16), residential: 0, name: None, floors: 1, pad_ft, structure, roof: None, tint: None, id: 0, parts: Vec::new(), keep: None }
}

/// How well the close `open` on `block` is ringed: the share of the block's street fronts
/// (`streets`, as `build_block` takes them; a point every 4 ft) with one of the homes of `houses`
/// behind them within 45 ft, before the open ground.
pub(super) fn ringed(block: &[P], streets: &[(P, f64)], open: &[P], houses: &[Building]) -> f64 {
    let near: Vec<&Building> = houses.iter().filter(|b| b.structure == Structure::Roofed && b.func.is_none()).collect();
    let (m, ccw) = (block.len(), area(block) > 0.0);
    let (mut all, mut hit) = (0, 0);
    for e in 0..m {
        let (a, b) = (block[e], block[(e + 1) % m]);
        let el = dist(a, b);
        if el < 12.0 || !streets.iter().any(|&(n, c)| (dot(lerp(a, b, 0.5), n) - c).abs() < 1.0) {
            continue;
        }
        let u = mul(sub(b, a), 1.0 / el);
        let inward = if ccw { [-u[1], u[0]] } else { [u[1], -u[0]] };
        let mut t = 6.0;
        while t <= el - 6.0 {
            let s = add(a, mul(u, t));
            all += 1;
            let mut d = 1.0;
            while d <= 45.0 {
                let q = add(s, mul(inward, d));
                if contains(open, q) {
                    break;
                }
                if near.iter().any(|h| contains(&h.poly, q)) {
                    hit += 1;
                    break;
                }
                d += 1.5;
            }
            t += 4.0;
        }
    }
    if all == 0 { 0.0 } else { hit as f64 / all as f64 }
}

/// Whether a building's footprint is a wedge: a corner sharper than 60° (edges under 4 ft taken
/// as part of their corner), or filling less of its box than a lot may (a wedge blunted at its
/// point, as `build_block` keeps none).
pub(super) fn wedge(poly: &[P]) -> bool {
    let o = obb(poly);
    sharp(poly) || area(poly).abs() < 0.62 * o.long * o.short
}

/// Whether a polygon has a corner sharper than 60° (edges under 4 ft taken as part of their
/// corner).
fn sharp(poly: &[P]) -> bool {
    let mut pts: Vec<P> = Vec::with_capacity(poly.len());
    for &p in poly {
        if pts.last().is_none_or(|q| dist(*q, p) >= 4.0) {
            pts.push(p);
        }
    }
    while pts.len() > 3 && dist(pts[0], pts[pts.len() - 1]) < 4.0 {
        pts.pop();
    }
    let m = pts.len();
    m < 3
        || (0..m).any(|i| {
            let (a, b) = (sub(pts[(i + m - 1) % m], pts[i]), sub(pts[(i + 1) % m], pts[i]));
            let (la, lb) = (len(a), len(b));
            la > 1e-6 && lb > 1e-6 && dot(a, b) / (la * lb) > 0.5
        })
}

/// Whether segment `a`–`b` passes into convex polygon `poly`.
fn touches(a: P, b: P, poly: &[P]) -> bool {
    let ccw = area(poly) > 0.0;
    let m = poly.len();
    let d = sub(b, a);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for e in 0..m {
        let (p, q) = (poly[e], poly[(e + 1) % m]);
        let ev = sub(q, p);
        // Inward normal.
        let n = if ccw { [-ev[1], ev[0]] } else { [ev[1], -ev[0]] };
        let (num, den) = (dot(sub(a, p), n), dot(d, n));
        if den.abs() < 1e-12 {
            if num < 0.0 {
                return false;
            }
            continue;
        }
        let t = -num / den;
        if den > 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 >= t1 {
            return false;
        }
    }
    true
}

/// A square being opened at a corner: its kind, and per patch round the corner
/// (in turn round the corner) the corner's inward bisector `n`, the chord's offset (the line
/// dot(p, n) = c) and its ends on the patch's two edges at the corner. The square is the patches'
/// corners inside their chords, streets and all; the lots there are trimmed back to the chords.
pub(super) struct Cut {
    pub kind: PlazaKind,
    faces: Vec<(usize, P, f64, P, P)>,
}

/// The cuts for squares at these corners, on the mesh as it stands: each chord square to its
/// corner's bisector, as far out as the kind has it, its ends no further along either street
/// than 1.25 times that (an open corner is cut shallower) nor than two fifths of the street.
pub(super) fn cuts(plan: &TownPlan, mesh: &Mesh, spots: &[(usize, PlazaKind)]) -> Vec<Cut> {
    spots
        .iter()
        .map(|&(v, kind)| {
            let p = mesh.pos[v];
            let r = reach(plan, v, kind);
            let mut faces: Vec<(f64, (usize, P, f64, P, P))> = Vec::new();
            for &f in &plan.vf[v] {
                let face = &mesh.faces[f];
                let m = face.len();
                let Some(k) = face.iter().position(|&u| u == v) else { continue };
                let (ea, eb) = (sub(mesh.pos[face[(k + m - 1) % m]], p), sub(mesh.pos[face[(k + 1) % m]], p));
                let (la, lb) = (len(ea), len(eb));
                if la < 1e-6 || lb < 1e-6 {
                    continue;
                }
                let (ua, ub) = (mul(ea, 1.0 / la), mul(eb, 1.0 / lb));
                let bis = add(ua, ub);
                let half_cos = 0.5 * len(bis);
                if half_cos < 1e-3 {
                    continue;
                }
                let n = mul(bis, 0.5 / half_cos);
                let t = (r / half_cos.max(0.3)).min(1.25 * r).min(0.4 * la.min(lb));
                let (a, b) = (add(p, mul(ua, t)), add(p, mul(ub, t)));
                // Its ends in turn round the corner.
                let (a, b) = if cross(sub(a, p), sub(b, p)) >= 0.0 { (a, b) } else { (b, a) };
                faces.push((libm::atan2(n[1], n[0]), (f, n, dot(a, n), a, b)));
            }
            faces.sort_by(|x, y| x.0.total_cmp(&y.0));
            Cut { kind, faces: faces.into_iter().map(|x| x.1).collect() }
        })
        .collect()
}

/// How far out from corner `v` a square of this kind is cut (ft) on the mesh as it stands: its
/// chords' mean distance from the corner.
pub(super) fn cut_reach(plan: &TownPlan, mesh: &Mesh, v: usize, kind: PlazaKind) -> f64 {
    let p = mesh.pos[v];
    let cut = &cuts(plan, mesh, &[(v, kind)])[0];
    if cut.faces.is_empty() {
        return 0.0;
    }
    cut.faces.iter().map(|&(_, n, c, _, _)| c - dot(p, n)).sum::<f64>() / cut.faces.len() as f64
}

impl Cut {
    /// Patch `f`'s buildings (from `first` on) trimmed back to the chord, 2 ft clear of it: those
    /// across it cut along it (if half or more is left of them, not a wedge: `wedge`), those
    /// inside it taken away.
    pub fn trim(&self, f: usize, bs: &mut Vec<Building>, first: usize) {
        for &(g, n, c, _, _) in &self.faces {
            if g != f {
                continue;
            }
            let line = c + 2.0;
            let mut k = first;
            while k < bs.len() {
                let poly = &bs[k].poly;
                let (lo, hi) = extent(poly, n);
                if lo >= line {
                    k += 1;
                    continue;
                }
                let kept = if hi <= line { Vec::new() } else { clean(&clip_plain(poly, mul(n, -1.0), -line)) };
                if kept.len() >= 3 && area(&kept).abs() >= (0.5 * area(poly).abs()).max(150.0) && obb(&kept).short >= 12.0 && !wedge(&kept) {
                    bs[k].poly = kept;
                    bs[k].parts.clear();
                    k += 1;
                } else {
                    bs.remove(k);
                }
            }
        }
    }

    /// A path along each chord, on the square's side of it (centre line, width ft): a green's
    /// edge before the houses that front it.
    pub fn edge_paths(&self, width: f64) -> Vec<(Vec<P>, f64)> {
        self.faces.iter().map(|&(_, n, _, a, b)| (vec![sub(a, mul(n, 0.5 * width)), sub(b, mul(n, 0.5 * width))], width)).collect()
    }

    /// The square: the chords' ends in turn round the corner (a polygon star-shaped about it).
    pub fn outline(&self) -> Option<Vec<P>> {
        let mut pts: Vec<P> = Vec::new();
        for &(_, _, _, a, b) in &self.faces {
            for q in [a, b] {
                if pts.last().is_none_or(|l| dist(*l, q) > 0.5) {
                    pts.push(q);
                }
            }
        }
        while pts.len() > 1 && dist(pts[0], pts[pts.len() - 1]) <= 0.5 {
            pts.pop();
        }
        (pts.len() >= 3 && area(&pts).abs() > 400.0).then_some(pts)
    }
}
