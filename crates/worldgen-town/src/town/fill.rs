//! A block's filling: its lots and buildings, a square, a park or farmland, as its town decided
//! (`Fill`). `build_town` decides each patch's fill (its ward, neighbours, slope, park kind, a
//! temple's close) and calls `fill_block` with the patch's block; a hand-laid town calls it with
//! a block of its own. Nothing here knows patches: a block is a polygon (town-local ft) with the
//! street lines along it (outward normal, offset), the rolls are a function of a stage number,
//! and the square's or park's id comes from its `places::Spot`.

use super::catalog::Ward;
use super::clip::{self, Shape};
use super::geom::*;
use super::{Building, Ground, Layout, PARK_GROUND_FT, Park, ParkGround, ParkKind, Plaza, PlazaKind, Site, Structure, cemetery, chamfer, places, rect, rotate, squares};
use crate::core::rng::Pcg32;
use crate::t0::settle::Tier;

/// What a block is filled with.
pub(super) enum Fill {
    /// A market square of its kind (`square_kind`), with a monument on it by roll.
    Square(PlazaKind),
    /// Farmland (`farm`): its plots' class, and the stream that cuts them.
    Farm { class: u8, rng: Pcg32 },
    /// A park of its kind: its ways in on the block's sides along `ways` (street lines, as the
    /// block's), its look's key and, a cemetery's, its style.
    Park { kind: ParkKind, ways: Vec<(P, f64)>, key: u64, cemetery: Option<cemetery::Style> },
    /// A ward's lots (`build_block`): the district's sizes and the stream that cuts them; the
    /// outskirts' thinning (the corners kept by, and how far it reaches); `close`, a temple's close
    /// round them (the block is known to fit one). A temple's block with no lots that would do
    /// keeps its temple, alone.
    Lots { ward: Ward, dp: District, rng: Pcg32, thin: Option<(Vec<P>, f64)>, close: bool },
}

/// What a market square is for, by roll `t`: a market, busy, sparse or for a market day; a
/// city's may be a civic square with no market at all.
pub(super) fn square_kind(t: f64, city: bool) -> PlazaKind {
    let civic = if city { 0.25 } else { 0.0 };
    if t < civic {
        PlazaKind::Civic
    } else if t < civic + 0.4 * (1.0 - civic) {
        PlazaKind::Market
    } else if t < civic + 0.75 * (1.0 - civic) {
        PlazaKind::MarketSparse
    } else {
        PlazaKind::MarketDay
    }
}

/// Fill `block` (town-local ft) as `fill` says: `streets`, the street lines along it (a lot needs
/// frontage on one); `roll`, the block's rolls by stage (a patch's `TownPlan::roll`); `spot`, what a
/// square's or park's id is keyed by; farmland's plots go to `plots` (kept or not once the town's
/// fields are all known). The block itself is not recorded in `l.blocks`: its town does that.
#[allow(clippy::too_many_arguments)]
pub(super) fn fill_block(site: &Site, block: &[P], streets: Vec<(P, f64)>, fill: Fill, roll: &dyn Fn(i64) -> f64, spot: places::Spot, l: &mut Layout, plots: &mut Vec<(Vec<P>, u8)>) {
    let first = l.buildings.len();
    match fill {
        Fill::Square(kind) => {
            // A monument, pushed toward the square's longest side (watabou's market).
            let mr = roll(40);
            if mr < 0.9 {
                let mb = block.len();
                let e = (0..mb).max_by(|&x, &y| dist(block[x], block[(x + 1) % mb]).total_cmp(&dist(block[y], block[(y + 1) % mb]))).unwrap_or(0);
                let (a, b) = (block[e], block[(e + 1) % mb]);
                let cen = centroid(block);
                let at = lerp(cen, lerp(a, b, 0.5), 0.2 + 0.4 * roll(41));
                let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
                let size = 13.0 + 13.0 * roll(42);
                let plinth = if mr < 0.6 {
                    rect(at, u, size, 13.0 + 13.0 * roll(43))
                } else {
                    let o = circle(at, 0.6 * size, 8);
                    o.iter().map(|q| add(at, rotate(sub(*q, at), libm::atan2(u[1], u[0])))).collect()
                };
                l.monuments.push(plinth);
            }
            l.plazas.push(Plaza { pts: block.to_vec(), kind, spot })
        }
        Fill::Farm { class, mut rng } => farm(site, block, &mut rng, l, plots, class, true),
        Fill::Park { kind, ways, key, cemetery } => {
            let nb = block.len();
            let fronts = (0..nb).map(|e| ways.iter().any(|&(n, off)| (dot(lerp(block[e], block[(e + 1) % nb], 0.5), n) - off).abs() < 1.0)).collect();
            let mut pk = park(site, block, kind, fronts, key);
            pk.cemetery = cemetery;
            pk.spot = spot;
            l.parks.push(pk);
        }
        Fill::Lots { ward, dp, mut rng, thin, close } => {
            // A temple's close: the block's lots, then the temple and its churchyard in the
            // middle, the houses round them cut back to face them.
            let alleys = l.roads.len();
            build_block(site, Block::Plain(block), &Front::of(block, &streets), ward, &dp, &mut rng, l, thin.as_ref().map(|(a, reach)| (a.as_slice(), *reach)));
            if close {
                let laid = squares::close(site, block, &streets, roll(65), first, alleys, l);
                debug_assert!(laid, "a close that fits is laid out");
            } else if ward == Ward::Temple && l.buildings.len() == first {
                squares::lone_temple(site, block, roll(65), l);
            }
        }
    }
}

/// Block and lot parameters shared by a district (after watabou's per-district rolls).
pub(super) struct District {
    /// Target lot area (ft²).
    pub(super) min_sq: f64,
    /// 0.2–1: how irregular alley cuts are allowed to be.
    pub(super) grid_chaos: f64,
    /// 0.4–1: how much lot and block sizes vary.
    pub(super) size_chaos: f64,
    /// Lots per block before an alley cuts it (4–14).
    pub(super) block_size: f64,
    /// (R6 P8) A docks ward on a sloping shore (grade over 0.09): a lot too steep for one pad
    /// (`Ground::stepped`) stands on two (`step_pads`).
    pub(super) stepped: bool,
    /// (161) A planned block's lattice heading (unit vector): its alleys cut square to it, its
    /// lots laid back to back in rows along it (`rows`), none left without a street.
    pub(super) frame: Option<P>,
}

impl District {
    /// The ward's usual lot area (ft²).
    pub(super) fn base(w: Ward) -> f64 {
        match w {
            Ward::Merchant => 1400.0,
            Ward::Craft => 1600.0,
            Ward::Noble => 4200.0,
            Ward::Slum => 650.0,
            Ward::Docks => 2600.0,
            Ward::Military => 2400.0,
            Ward::Temple => 2200.0,
            Ward::Castle => 2600.0,
            Ward::Park => 9000.0,
            _ => 1100.0,
        }
    }

    pub(super) fn roll(w: Ward, seed: u64) -> District {
        let mut r = Pcg32::new(seed, 53);
        let base = District::base(w);
        let spread = r.next_f64();
        let mut d = District {
            min_sq: base * (0.75 + 0.9 * spread),
            grid_chaos: 0.2 + 0.8 * r.next_f64(),
            size_chaos: 0.4 + 0.6 * r.next_f64(),
            block_size: 4.0 + 10.0 * r.next_f64(),
            stepped: false,
            frame: None,
        };
        if w == Ward::Slum {
            d.grid_chaos *= 0.5;
            d.block_size *= 2.0;
        }
        d
    }
}

/// A street front: the street's line as a block's street lines always were (outward normal `n`,
/// offset `c`: the block lies where `dot(p, n) <= c`) and the stretch of it from `a` to `b` (both on
/// the line) that the street runs along. A lot fronts it when one of its edges, longer than 6 ft,
/// lies on the line within 0.5 ft and overlaps the stretch by more than 6 ft, so a zero-length
/// stretch (`a == b`, as `Front::of` gives a line the block does not touch) means no frontage at
/// all, never an unbounded one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Front {
    pub n: P,
    pub c: f64,
    pub a: P,
    pub b: P,
}

impl Front {
    /// Fronts from street lines (as `street_lines` gives them) and the block they run along: each
    /// line's stretch is where the block's outline lies on it (none: an empty stretch, at the point
    /// of the line nearest the block's first corner). The lines keep their order and their floats.
    pub(super) fn of(block: &[P], lines: &[(P, f64)]) -> Vec<Front> {
        let m = block.len();
        lines
            .iter()
            .map(|&(n, c)| {
                let t = [-n[1], n[0]];
                let on = |p: P| (dot(p, n) - c).abs() < 0.5;
                let (mut lo, mut hi) = (f64::MAX, f64::MIN);
                for i in 0..m {
                    let (p, q) = (block[i], block[(i + 1) % m]);
                    if on(p) && on(q) {
                        lo = lo.min(dot(p, t)).min(dot(q, t));
                        hi = hi.max(dot(p, t)).max(dot(q, t));
                    }
                }
                if lo > hi {
                    let s = block.first().map_or(0.0, |p| dot(*p, t));
                    (lo, hi) = (s, s);
                }
                Front { n, c, a: add(mul(n, c), mul(t, lo)), b: add(mul(n, c), mul(t, hi)) }
            })
            .collect()
    }

    /// Whether `poly` fronts this street: one of its edges longer than 6 ft lies on the line within
    /// 0.5 ft and overlaps the stretch by more than 6 ft.
    pub fn fronted_by(&self, poly: &[P]) -> bool {
        let t = [-self.n[1], self.n[0]];
        let (s0, s1) = (dot(self.a, t).min(dot(self.b, t)), dot(self.a, t).max(dot(self.b, t)));
        let m = poly.len();
        (0..m).any(|i| {
            let (p, q) = (poly[i], poly[(i + 1) % m]);
            on_line(p, q, self.n, self.c) && dist(p, q) > 6.0 && dot(p, t).max(dot(q, t)).min(s1) - dot(p, t).min(dot(q, t)).max(s0) > 6.0
        })
    }
}

/// A block to fill with lots (`build_block`).
pub(super) enum Block<'a> {
    /// A block as `build_town` makes them (convex or nearly): filled as it always was, a lot
    /// fronting a street when it has an edge on the street's line (stretches not looked at).
    Plain(&'a [P]),
    /// A hand-laid block, convex, concave or with holes (courtyards): cut into convex pieces
    /// (`clip::convex_pieces`; a convex ring is its own one piece), each filled in turn as a plain
    /// block is, but with its lots fronting only along the fronts' stretches and the alleys cut in
    /// that piece. A piece fronting no street stays open.
    Region(&'a [Shape]),
}

/// Alleys, lots and buildings for one block (after watabou's Bisector), fronting `fronts`. Big
/// blocks are cut by 16 ft alleys into blocks of about `block_size` lots; blocks are cut into lots
/// square to their street frontage; lots without frontage stay open (yards and gardens); buildings
/// fill their lots wall to wall, some standing back from the street. A region's pieces are filled
/// in `convex_pieces`' order, one stream through them all.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_block<G: Ground>(site: &G, block: Block, fronts: &[Front], ward: Ward, dp: &District, rng: &mut Pcg32, l: &mut Layout, thin: Option<(&[P], f64)>) {
    // (The outskirts' thinning runs through the whole block, a region's pieces and all: one
    // running share, and "until a house stands" counted from the block's start.)
    let mut run = (l.buildings.len(), None);
    match block {
        Block::Plain(ring) => lay_block(site, ring, &mut fronts.to_vec(), false, ward, dp, rng, l, thin, &mut run),
        Block::Region(set) => {
            for piece in clip::convex_pieces(set) {
                if fronts.iter().any(|f| f.fronted_by(&piece)) {
                    lay_block(site, &piece, &mut fronts.to_vec(), true, ward, dp, rng, l, thin, &mut run);
                }
            }
        }
    }
}

/// One convex block's alleys, lots and buildings (`build_block`). The alleys' sides join
/// `fronts` as they are cut; `spans`: whether a lot must front a street along its stretch (a
/// region's piece) or only on its line (a plain block, as always).
#[allow(clippy::too_many_arguments)]
fn lay_block<G: Ground>(site: &G, block: &[P], fronts: &mut Vec<Front>, spans: bool, ward: Ward, dp: &District, rng: &mut Pcg32, l: &mut Layout, thin: Option<(&[P], f64)>, run: &mut (usize, Option<f64>)) {
    let mut parts: Vec<Vec<P>> = Vec::new();
    let alley_area = dp.min_sq * dp.block_size;
    if area(block).abs() > alley_area * libm::pow(2.0, dp.size_chaos * (2.0 * rng.next_f64() - 1.0)) {
        let mut chords: Vec<(P, P)> = Vec::new();
        bisect(block, alley_area, (16.0 * dp.grid_chaos).max(2.0), ALLEY_HALF_FT, dp.frame, rng, &mut parts, fronts, &mut chords, 0);
        // Alleys in towns are paved like their fronts (the sprawl outside keeps dirt lanes).
        if thin.is_none() && l.tier >= Tier::Town {
            for (a, b) in chords {
                l.roads.push((vec![a, b], 3, 2.0 * ALLEY_HALF_FT + 1.0));
            }
        }
    } else {
        parts.push(block.to_vec());
    }
    let lot_variance = (4.0 * dp.size_chaos).max(1.2);
    // (The sprawl thinning out away from the city a block between alleys at a time, nearest
    // first: each block is kept with a share that falls with its distance from the corners the
    // patch shares with the city (to a tenth at the far side), the shares added up block by block
    // rather than each rolled alone, so the far blocks still stand here and there, one in so many,
    // never all lost to a run of bad rolls; and blocks are kept until one has a house standing.)
    let (order, share): (Vec<usize>, Vec<f64>) = match thin {
        None => ((0..parts.len()).collect(), vec![1.0; parts.len()]),
        Some((anchors, reach)) => {
            let ds: Vec<f64> = parts.iter().map(|part| anchors.iter().map(|a| dist(*a, centroid(part))).fold(f64::MAX, f64::min)).collect();
            let mut order: Vec<usize> = (0..parts.len()).collect();
            order.sort_by(|&a, &b| ds[a].total_cmp(&ds[b]).then(a.cmp(&b)));
            (order, ds.iter().map(|&d| if anchors.is_empty() { 0.35 } else { (1.0 - smoothstep_f(0.0, 0.9 * reach, d)).max(0.1) }).collect())
        }
    };
    if thin.is_some() && run.1.is_none() {
        run.1 = Some(rng.next_f64());
    }
    let (first, mut acc) = (run.0, run.1.unwrap_or(0.0));
    for pi in order {
        let part = &parts[pi];
        acc += share[pi];
        if acc >= 1.0 {
            acc -= 1.0;
        } else if l.buildings.len() > first {
            continue;
        }
        let mut lots: Vec<Vec<P>> = Vec::new();
        match dp.frame {
            Some(u) => rows(part, u, dp.min_sq, rng, &mut lots),
            None => bisect(part, dp.min_sq, lot_variance, 0.0, None, rng, &mut lots, &mut Vec::new(), &mut Vec::new(), 0),
        }
        merge_wedges(&mut lots, 5.0 * dp.min_sq);
        for lot in lots {
            let lot = clean(&blunt(&clean(&lot), 60.0, 10.0));
            // No triangles, slivers or scraps: those corners stay open ground.
            if lot.len() < 4 {
                continue;
            }
            let la = area(&lot).abs();
            let o = obb(&lot);
            if la < dp.min_sq / 4.0 || o.short < 16.0 || la < 0.62 * o.long * o.short {
                continue;
            }
            // Frontage: a street or alley one of the lot's edges lies on (along its stretch, in a piece).
            let Some(&Front { n, c, a: fa, b: fb }) = fronts.iter().find(|f| if spans { f.fronted_by(&lot) } else { on_line_lot(&lot, f.n, f.c) }) else { continue };
            // (A terraced garden on a steep block: a house here and there.)
            if ward == Ward::Park && rng.next_f64() > 0.15 {
                continue;
            }
            // Irregular street fronts: some buildings stand back from the street line.
            let indent = rng.next_f64() * (crate::core::sqrt(la) / 3.0).min(16.0);
            // (Blunted again: a corner cut off at the street may come back once set back. Not
            // where that leaves a wedge or a sliver, a shallow lot along its street (under 14 ft
            // deep), or no front.)
            let set = if indent > 6.5 { clean(&blunt(&clean(&clip_plain(&lot, n, c - indent)), 60.0, 10.0)) } else { Vec::new() };
            // (`d`: how far the footprint stands back.)
            let fronted = |p: &[P], d: f64| if spans { Front { n, c: c - d, a: sub(fa, mul(n, d)), b: sub(fb, mul(n, d)) }.fronted_by(p) } else { on_line_lot(p, n, c - d) };
            let (poly, d) = if set.len() >= 3 && fronted(&set, indent) && shapeliness(&set) >= shapeliness(&lot).min(1.0) && obb(&set).short >= 14.0 { (set, indent) } else { (lot, 0.0) };
            // (P4) A footprint that still reads as a wedge (`eye_corner`: a trapezoid's narrow end, a
            // chord across a sharp corner): squared off on its front inside its lot, or the lot left
            // as yard.
            let poly = if eye_corner(&poly) < 70.0 {
                match square_off(&poly, Some((n, c - d))).filter(|r| fronted(r, d)) {
                    Some(r) => r,
                    None => continue,
                }
            } else {
                poly
            };
            if poly.len() < 3 || area(&poly).abs() < 150.0 {
                continue;
            }
            let pads = if site.buildable(&poly) {
                vec![poly]
            } else if dp.stepped && site.stepped(&poly) {
                // ((P4) A half that reads as a wedge squared off, or left as yard.)
                step_pads(site, &poly).into_iter().filter_map(|h| if eye_corner(&h) < 70.0 { square_off(&h, poly_on(&h, n, c - d).then_some((n, c - d))).filter(|r| area(r).abs() >= 150.0 && site.buildable(r)) } else { Some(h) }).collect()
            } else {
                continue;
            };
            for poly in pads {
                let pad = site.pad(&poly);
                l.buildings.push(Building { poly, ward, func: None, residential: 0, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id: 0, parts: Vec::new(), keep: None, deck: None });
            }
        }
    }
    if thin.is_some() {
        run.1 = Some(acc);
    }
}

/// A docks lot on a slope too steep for one pad (`Ground::stepped`): cut across the fall
/// through its middle into an upper and a lower half, a pair of houses a step apart down the
/// shore, each on its own pad and each kept only where it stands as any lot does
/// (`Ground::buildable`), so their walls meet the ground as the rest of the town's do.
fn step_pads<G: Ground>(site: &G, poly: &[P]) -> Vec<Vec<P>> {
    let c = centroid(poly);
    let hs: Vec<f64> = poly.iter().map(|p| site.height(*p)).collect();
    let (lo, hi) = (0..poly.len()).fold((0, 0), |(lo, hi), k| (if hs[k] < hs[lo] { k } else { lo }, if hs[k] > hs[hi] { k } else { hi }));
    let fall = sub(poly[hi], poly[lo]);
    if len(fall) < 1.0 {
        return Vec::new();
    }
    let n = mul(fall, 1.0 / len(fall));
    let k = dot(c, n);
    [clip_plain(poly, n, k), clip_plain(poly, mul(n, -1.0), -k)].into_iter().map(|h| clean(&h)).filter(|h| h.len() >= 3 && area(h).abs() >= 150.0 && site.buildable(h)).collect()
}

/// How far a lot is from a wedge or a sliver, 0-1 (1: fine): the least of its sharpest corner
/// over 60 deg (edges under 4 ft taken as part of their corner), its fitted box's short side over
/// a quarter of its long one, and its area over 0.62 of that box's.
pub(super) fn shapeliness(lot: &[P]) -> f64 {
    let pts = corners(lot);
    let m = pts.len();
    if m < 3 {
        return 0.0;
    }
    let mut cos_max = -1.0f64;
    for i in 0..m {
        let (a, b) = (sub(pts[(i + m - 1) % m], pts[i]), sub(pts[(i + 1) % m], pts[i]));
        let (la, lb) = (len(a), len(b));
        if la > 1e-6 && lb > 1e-6 {
            cos_max = cos_max.max(dot(a, b) / (la * lb));
        }
    }
    let corner = libm::acos(cos_max.clamp(-1.0, 1.0)).to_degrees() / 60.0;
    let o = obb(lot);
    let aspect = if o.long > 1e-6 { o.short / (0.25 * o.long) } else { 0.0 };
    let fill = if o.long * o.short > 1e-6 { area(lot).abs() / (0.62 * o.long * o.short) } else { 0.0 };
    corner.min(aspect).min(fill).min(1.0)
}

/// (P4) A footprint's sharpest corner as the eye reads it (deg): each convex corner, and across
/// each short edge (under 14 ft, a `blunt` chord, and under half of both edges beside it) between
/// two convex corners the corner the two edges beside it would meet at ahead (a trapezoid's narrow
/// end, a corner cut by a chord; not where they run (nearly) parallel: a strip, a width's matter).
/// Vertices under 1 ft apart count as one.
pub(super) fn eye_corner(poly: &[P]) -> f64 {
    let mut p: Vec<P> = Vec::with_capacity(poly.len());
    for q in poly {
        if p.last().is_none_or(|l| dist(*l, *q) >= 1.0) {
            p.push(*q);
        }
    }
    while p.len() > 3 && dist(p[0], p[p.len() - 1]) < 1.0 {
        p.pop();
    }
    let m = p.len();
    if m < 3 {
        return 0.0;
    }
    let ccw = area(&p) > 0.0;
    let ang = |k: usize| -> Option<f64> {
        let (a, b, c) = (p[(k + m - 1) % m], p[k], p[(k + 1) % m]);
        if (cross(sub(b, a), sub(c, b)) > 0.0) != ccw {
            return None;
        }
        let (u, v) = (sub(a, b), sub(c, b));
        Some(libm::acos((dot(u, v) / (len(u) * len(v)).max(1e-9)).clamp(-1.0, 1.0)).to_degrees())
    };
    let mut sharp = 180.0f64;
    for k in 0..m {
        let Some(a0) = ang(k) else { continue };
        sharp = sharp.min(a0);
        let e = dist(p[k], p[(k + 1) % m]);
        let (before, after) = (dist(p[(k + m - 1) % m], p[k]), dist(p[(k + 1) % m], p[(k + 2) % m]));
        if e < 14.0 && e < 0.5 * before.min(after) {
            if let Some(a1) = ang((k + 1) % m).filter(|a1| a0 + a1 > 185.0) {
                sharp = sharp.min(a0 + a1 - 180.0);
            }
        }
    }
    sharp
}

/// (P4) The largest rectangle inside a convex footprint standing on one of its edges (12 ft or
/// more each way, no more than three times as long as wide): on its street front where `front`
/// (the street line: normal, offset) is given, else on whichever edge does best. None where no
/// rectangle keeps 40% of the footprint (a sliver: left as yard).
pub(super) fn square_off(poly: &[P], front: Option<(P, f64)>) -> Option<Vec<P>> {
    let m = poly.len();
    let pa = area(poly).abs();
    if m < 3 || pa < 1.0 {
        return None;
    }
    let c = centroid(poly);
    let mut best: Option<(f64, Vec<P>)> = None;
    for i in 0..m {
        let (a, b) = (poly[i], poly[(i + 1) % m]);
        let el = dist(a, b);
        if el < 12.0 {
            continue;
        }
        let u = mul(sub(b, a), 1.0 / el);
        let mut w = [-u[1], u[0]];
        if dot(sub(c, a), w) < 0.0 {
            w = mul(w, -1.0);
        }
        let on_front = front.is_some_and(|(n, c)| dot(u, n).abs() < 0.05 && (dot(a, n) - c).abs() < 0.5);
        if front.is_some() && !on_front {
            continue;
        }
        let vmax = poly.iter().map(|p| dot(sub(*p, a), w)).fold(0.0, f64::max);
        // The footprint's extent along `u` at height `v` off the edge.
        let chord = |v: f64| -> (f64, f64) {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for k in 0..m {
                let (p, q) = (poly[k], poly[(k + 1) % m]);
                let (vp, vq) = (dot(sub(p, a), w), dot(sub(q, a), w));
                if (vp - v) * (vq - v) > 0.0 || (vp - vq).abs() < 1e-9 && (vp - v).abs() > 1e-9 {
                    continue;
                }
                let t = if (vq - vp).abs() < 1e-9 { 0.0 } else { (v - vp) / (vq - vp) };
                let (up, uq) = (dot(sub(p, a), u), dot(sub(q, a), u));
                let x = up + t * (uq - up);
                lo = lo.min(x);
                hi = hi.max(x);
                if (vq - vp).abs() < 1e-9 {
                    lo = lo.min(uq);
                    hi = hi.max(uq);
                }
            }
            (lo, hi)
        };
        let base = chord(0.05);
        for s in 0..=24 {
            let d = 12.1 + (vmax - 12.1) * s as f64 / 24.0;
            if d < 12.1 {
                break;
            }
            let top = chord(d - 0.05);
            let (lo, hi) = (base.0.max(top.0), base.1.min(top.1));
            if hi - lo < 12.0 {
                continue;
            }
            // (No sliver either: its short side a third of its long one or more, kept in the middle.)
            let dd = (d - 0.05).min(3.0 * (hi - lo));
            let mid = 0.5 * (lo + hi);
            let half = 0.5 * (hi - lo).min(3.0 * dd);
            let (lo, hi) = (mid - half, mid + half);
            let ra = (hi - lo) * dd;
            let score = ra;
            if best.as_ref().is_none_or(|bb| score > bb.0) {
                let mut r = vec![add(a, mul(u, lo)), add(a, mul(u, hi)), add(add(a, mul(u, hi)), mul(w, dd)), add(add(a, mul(u, lo)), mul(w, dd))];
                if (area(&r) > 0.0) != (area(poly) > 0.0) {
                    r.reverse();
                }
                best = Some((score, r));
            }
        }
    }
    best.filter(|bb| area(&bb.1).abs() >= 0.4 * pa).map(|bb| bb.1)
}

/// A polygon's corners: its vertices, each edge under 4 ft taken as part of the corner it leads to.
fn corners(poly: &[P]) -> Vec<P> {
    let mut pts: Vec<P> = Vec::with_capacity(poly.len());
    for &p in poly {
        if pts.last().is_none_or(|q| dist(*q, p) >= 4.0) {
            pts.push(p);
        }
    }
    while pts.len() > 3 && dist(pts[0], pts[pts.len() - 1]) < 4.0 {
        pts.pop();
    }
    pts
}

/// (Roadmap 7, 162) Wedge and sliver lots (`shapeliness` under 1) merged into a neighbour, the
/// one they share the longest edge with whose union is convex, no bigger than `cap` sq ft, and
/// shaped better than the wedge was; a wedge at the block's own sharp corner (no merge helps) is
/// left to `blunt`. Worst first, until none is left to merge.
fn merge_wedges(lots: &mut Vec<Vec<P>>, cap: f64) {
    let mut score: Vec<f64> = lots.iter().map(|l| shapeliness(l)).collect();
    let mut stuck = vec![false; lots.len()];
    loop {
        let Some(i) = (0..lots.len()).filter(|&i| score[i] < 1.0 && !stuck[i]).min_by(|&a, &b| score[a].total_cmp(&score[b]).then(a.cmp(&b))) else { break };
        let ai = area(&lots[i]).abs();
        let mut cands: Vec<(f64, usize)> = (0..lots.len()).filter(|&j| j != i).map(|j| (shared(&lots[i], &lots[j]), j)).filter(|c| c.0 > 1.0).collect();
        cands.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
        let mut done = false;
        for (_, j) in cands {
            let aj = area(&lots[j]).abs();
            if ai + aj > cap {
                continue;
            }
            let mut h = hull(lots[i].iter().chain(lots[j].iter()).copied().collect());
            if h.len() < 3 || area(&h).abs() > ai + aj + 0.5 {
                continue;
            }
            let sh = shapeliness(&h);
            if sh <= score[i] + 0.05 {
                continue;
            }
            if (area(&h) > 0.0) != (area(&lots[j]) > 0.0) {
                h.reverse();
            }
            lots[j] = clean(&h);
            score[j] = sh;
            stuck[j] = false;
            lots.remove(i);
            score.remove(i);
            stuck.remove(i);
            done = true;
            break;
        }
        if !done {
            stuck[i] = true;
        }
    }
}

/// The length two lots' edges run together (collinear within half a foot, either way round).
fn shared(a: &[P], b: &[P]) -> f64 {
    let mut total = 0.0;
    for i in 0..a.len() {
        let (p, q) = (a[i], a[(i + 1) % a.len()]);
        let l = dist(p, q);
        if l < 1e-6 {
            continue;
        }
        let u = mul(sub(q, p), 1.0 / l);
        let nrm = [-u[1], u[0]];
        for k in 0..b.len() {
            let (r, s) = (b[k], b[(k + 1) % b.len()]);
            if dot(sub(r, p), nrm).abs() > 0.5 || dot(sub(s, p), nrm).abs() > 0.5 {
                continue;
            }
            let (t0, t1) = (dot(sub(r, p), u), dot(sub(s, p), u));
            total += (t0.max(t1).min(l) - t0.min(t1).max(0.0)).max(0.0);
        }
    }
    total
}

/// Cut off corners sharper than `min_deg` with a chord square to the corner's bisector,
/// `depth` ft in from the tip, deeper at a sharper corner so the chord is 12 ft or more (keeps the
/// polygon convex; houses don't come to a point).
pub(super) fn blunt(poly: &[P], min_deg: f64, depth: f64) -> Vec<P> {
    // (Corners as `corners` has them: a corner already cut a foot across is still a point.)
    let pts = corners(poly);
    let m = pts.len();
    if m < 3 {
        return poly.to_vec();
    }
    let cos_min = libm::cos(min_deg.to_radians());
    let mut out = poly.to_vec();
    for i in 0..m {
        let (p, a, b) = (pts[i], pts[(i + m - 1) % m], pts[(i + 1) % m]);
        let (u, v) = (sub(a, p), sub(b, p));
        let (lu, lv) = (len(u), len(v));
        if lu < 1e-9 || lv < 1e-9 {
            continue;
        }
        let (u, v) = (mul(u, 1.0 / lu), mul(v, 1.0 / lv));
        if dot(u, v) <= cos_min {
            continue;
        }
        // Inward bisector; keep the part at least `depth` along it from the tip.
        let bis = add(u, v);
        let bl = len(bis).max(1e-9);
        let n = mul(bis, -1.0 / bl);
        // (The chord at depth d is 2 d tan(half the corner) long.)
        let half = 0.5 * libm::acos(dot(u, v).clamp(-1.0, 1.0));
        let d = depth.max(6.0 / libm::tan(half).max(0.05));
        out = clip_plain(&out, n, dot(p, n) - d);
        if out.len() < 3 {
            return out;
        }
    }
    out
}

/// Half the width of an alley cut through a block (ft).
pub(super) const ALLEY_HALF_FT: f64 = 8.0;

/// Some edge of `poly` lies on the line (normal `n`, offset `c`) within 0.5 ft.
fn poly_on(poly: &[P], n: P, c: f64) -> bool {
    (0..poly.len()).any(|i| on_line(poly[i], poly[(i + 1) % poly.len()], n, c))
}

/// A lot edge of useful length lies on the line (normal `n`, offset `c`).
fn on_line_lot(lot: &[P], n: P, c: f64) -> bool {
    let m = lot.len();
    (0..m).any(|i| {
        let (a, b) = (lot[i], lot[(i + 1) % m]);
        on_line(a, b, n, c) && dist(a, b) > 6.0
    })
}

/// Both ends of an edge lie on the line (normal `n`, offset `c`) within 0.5 ft.
fn on_line(a: P, b: P, n: P, c: f64) -> bool {
    (dot(a, n) - c).abs() < 0.5 && (dot(b, n) - c).abs() < 0.5
}

/// Recursive straight-cut bisection of a convex polygon (watabou's Bisector, straight cuts
/// only so pieces stay convex). The cut crosses the long axis of the minimum bounding box
/// near the middle; it runs square to the crossed edge that is most parallel to that axis,
/// so pieces meet their frontage at right angles. Cuts that split too unevenly are retried
/// in frames rotated by 36°. `gap` > 0 leaves a lane of width 2·gap along each cut, whose
/// sides are recorded in `cuts` as fronts, as far as the lane runs.
#[allow(clippy::too_many_arguments)]
/// (161) A planned block's lots (`District::frame`, `u` its lattice heading): one row of lots, or
/// two back to back (a yard between them where the block is deeper than two lots), along its
/// longer way on the lattice, every lot reaching a street or alley on the block's side.
fn rows(part: &[P], u: P, min_sq: f64, rng: &mut Pcg32, out: &mut Vec<Vec<P>>) {
    let v = [-u[1], u[0]];
    let ((ulo, uhi), (vlo, vhi)) = (extent(part, u), extent(part, v));
    let (a, b, (blo, bhi)) = if uhi - ulo >= vhi - vlo { (u, v, (vlo, vhi)) } else { (v, u, (ulo, uhi)) };
    let deep = bhi - blo;
    let dt = crate::core::sqrt(1.5 * min_sq).clamp(22.0, 70.0);
    // (Lots about twice as deep as wide at most: a house fills its lot.)
    let strips: Vec<(Vec<P>, f64)> = if deep <= 1.3 * dt {
        vec![(part.to_vec(), deep)]
    } else {
        let d = (0.5 * deep).min(1.15 * dt);
        vec![(clean(&clip_plain(part, b, blo + d)), d), (clean(&clip_plain(part, mul(b, -1.0), -(bhi - d))), d)]
    };
    for (strip, d) in strips {
        if strip.len() < 3 {
            continue;
        }
        let (alo, ahi) = extent(&strip, a);
        let w = (min_sq / d.max(1.0)).max(14.0);
        let mut x = alo;
        while ahi - x > 1.0 {
            let mut step = w * (0.85 + 0.3 * rng.next_f64());
            if ahi - (x + step) < 0.6 * w {
                step = ahi - x;
            }
            let lot = clean(&clip_plain(&clip_plain(&strip, a, x + step), mul(a, -1.0), -x));
            if lot.len() >= 3 {
                out.push(lot);
            }
            x += step;
        }
    }
}

/// A piece's split into parts of about `min_area` (`variance` how far they may differ), cut by
/// lines across it (`gap` either side of each kept open as an alley, its sides `cuts` and its centre
/// line `chords`). (161) With `frame` (a planned block's lattice heading) the cuts run square to it.
#[allow(clippy::too_many_arguments)]
fn bisect(poly: &[P], min_area: f64, variance: f64, gap: f64, frame_to: Option<P>, rng: &mut Pcg32, out: &mut Vec<Vec<P>>, cuts: &mut Vec<Front>, chords: &mut Vec<(P, P)>, depth: u32) {
    let a = area(poly).abs();
    let avg4 = (rng.next_f64() + rng.next_f64() + rng.next_f64() + rng.next_f64()) / 4.0;
    if depth > 18 || a < min_area * libm::pow(variance, (avg4 * 2.0 - 1.0).abs()) {
        out.push(poly.to_vec());
        return;
    }
    let c = centroid(poly);
    let m = poly.len();
    if let Some(u) = frame_to {
        // (Across the longer way first, then the other.)
        let v = [-u[1], u[0]];
        let ((ulo, uhi), (vlo, vhi)) = (extent(poly, u), extent(poly, v));
        let order = if uhi - ulo >= vhi - vlo { [u, v] } else { [v, u] };
        for attempt in 0..6 {
            let nrm = order[attempt % 2];
            let (lo, hi) = extent(poly, nrm);
            if hi - lo < 1e-6 {
                continue;
            }
            let avg3 = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0;
            let cf = (dot(c, nrm) - lo) / (hi - lo);
            let cc = lo + (hi - lo) * (0.5 * (cf + 0.5) + 0.2 * (avg3 - 0.5));
            if split_at(poly, nrm, add(c, mul(nrm, cc - dot(c, nrm))), min_area, variance, gap, frame_to, rng, out, cuts, chords, depth) {
                return;
            }
        }
        out.push(poly.to_vec());
        return;
    }
    let frame = obb(poly).axis;
    for attempt in 0..10 {
        let ang = attempt as f64 * std::f64::consts::PI / 5.0;
        let (ca, sa) = (libm::cos(ang), libm::sin(ang));
        let axis = [frame[0] * ca - frame[1] * sa, frame[0] * sa + frame[1] * ca];
        let (lo, hi) = extent(poly, axis);
        if hi - lo < 1e-6 {
            continue;
        }
        let avg3 = (rng.next_f64() + rng.next_f64() + rng.next_f64()) / 3.0;
        let cf = (dot(c, axis) - lo) / (hi - lo);
        let pos = lo + (hi - lo) * (cf + avg3) / 2.0;
        // The crossed edge most parallel to the axis: the cut leaves it at a right angle.
        let mut best: Option<(f64, P, P)> = None;
        for i in 0..m {
            let (p, q) = (poly[i], poly[(i + 1) % m]);
            let (dp_, dq) = (dot(p, axis) - pos, dot(q, axis) - pos);
            if (dp_ <= 0.0) == (dq <= 0.0) {
                continue;
            }
            let e = sub(q, p);
            let el = len(e);
            if el < 1e-9 {
                continue;
            }
            let u = mul(e, 1.0 / el);
            let cs = dot(u, axis).abs();
            if best.is_none_or(|b| cs > b.0) {
                best = Some((cs, lerp(p, q, dp_ / (dp_ - dq)), u));
            }
        }
        let Some((_, x, nrm)) = best else { continue };
        if split_at(poly, nrm, x, min_area, variance, gap, None, rng, out, cuts, chords, depth) {
            return;
        }
    }
    out.push(poly.to_vec());
}

/// `bisect`'s cut of `poly` along the line through `x` square to `nrm`, if it leaves two parts near
/// enough in size; then each part's own.
#[allow(clippy::too_many_arguments)]
fn split_at(poly: &[P], nrm: P, x: P, min_area: f64, variance: f64, gap: f64, frame_to: Option<P>, rng: &mut Pcg32, out: &mut Vec<Vec<P>>, cuts: &mut Vec<Front>, chords: &mut Vec<(P, P)>, depth: u32) -> bool {
    let cc = dot(x, nrm);
    let p1 = clean(&clip_plain(poly, nrm, cc - gap));
    let p2 = clean(&clip_plain(poly, mul(nrm, -1.0), -(cc + gap)));
    if p1.len() < 3 || p2.len() < 3 {
        return false;
    }
    let (a1, a2) = (area(&p1).abs(), area(&p2).abs());
    if a1.max(a2) > 2.0 * variance * a1.min(a2).max(1e-9) {
        return false;
    }
    if gap > 0.0 {
        // The alley's centre line across this piece; its sides, as far as it runs, are fronts.
        let dir = [-nrm[1], nrm[0]];
        let m = poly.len();
        let hits: Vec<P> = (0..m)
            .filter_map(|k| {
                let (a, b) = (poly[k], poly[(k + 1) % m]);
                let (da, db) = (dot(a, nrm) - cc, dot(b, nrm) - cc);
                (da * db < 0.0).then(|| lerp(a, b, da / (da - db)))
            })
            .collect();
        let lo = hits.iter().min_by(|a, b| dot(**a, dir).total_cmp(&dot(**b, dir)));
        let hi = hits.iter().max_by(|a, b| dot(**a, dir).total_cmp(&dot(**b, dir)));
        let (a, b) = match (lo, hi) {
            (Some(&a), Some(&b)) => {
                chords.push((a, b));
                (a, b)
            }
            _ => (x, x),
        };
        let side = |s: f64| (add(a, mul(nrm, s)), add(b, mul(nrm, s)));
        let ((a1, b1), (a2, b2)) = (side(-gap), side(gap));
        cuts.push(Front { n: nrm, c: cc - gap, a: a1, b: b1 });
        cuts.push(Front { n: mul(nrm, -1.0), c: -(cc + gap), a: a2, b: b2 });
    }
    bisect(&p1, min_area, variance, gap, frame_to, rng, out, cuts, chords, depth + 1);
    bisect(&p2, min_area, variance, gap, frame_to, rng, out, cuts, chords, depth + 1);
    true
}

/// Farmland (after watabou's fields): the patch is split into plots, square to the long
/// side of each piece's bounding box with some skewed cuts, leaving hedged lanes between
/// plots; corners are chamfered; one plot in five gets a farmhouse on its longest side. v63: the
/// plots go to `out` with `class` (0: always kept; one with its farmhouse on it is too), which
/// fields the belt keeps chosen once all are known (`build_town`); `houses` off: open ground past
/// the farmland, fields only.
pub(super) fn farm(site: &Site, block: &[P], rng: &mut Pcg32, l: &mut Layout, out: &mut Vec<(Vec<P>, u8)>, class: u8, houses: bool) {
    let spread = rng.next_f64();
    let mut plots: Vec<Vec<P>> = Vec::new();
    split_field(block, 67_600.0 * (1.0 + spread), rng, &mut plots, 0);
    for plot in plots {
        let f = clean(&chamfer(&plot, 17.0));
        if f.len() < 3 || area(&f).abs() < 2_000.0 || site.wet(centroid(&f)) || !site.dry_outline(&f, 8.0) || site.grade(&f) > 0.08 {
            continue;
        }
        let mut housed = false;
        if houses && rng.next_f64() < 0.2 {
            let m = f.len();
            let e = (0..m).max_by(|&x, &y| dist(f[x], f[(x + 1) % m]).total_cmp(&dist(f[y], f[(y + 1) % m]))).unwrap_or(0);
            let (a, b) = (f[e], f[(e + 1) % m]);
            let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
            let mut inward = [-u[1], u[0]];
            if dot(inward, sub(centroid(&f), a)) < 0.0 {
                inward = mul(inward, -1.0);
            }
            let (w, dp) = (rng.range(52.0, 65.0), rng.range(26.0, 39.0));
            let house = rect(add(add(a, mul(u, 0.5 * w + 6.0)), mul(inward, 0.5 * dp + 6.0)), u, w, dp);
            if house.iter().all(|p| contains(&f, *p)) && site.buildable(&house) {
                let pad = site.pad(&house);
                l.buildings.push(Building { poly: house, ward: Ward::Farm, func: None, residential: 5, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id: 0, parts: Vec::new(), keep: None, deck: None });
                housed = true;
            }
        }
        out.push((f, if housed { 0 } else { class }));
    }
}

/// A park on `block` (local ft) with the ground it is planned on: every `PARK_GROUND_FT` over its
/// bounds (on the world's grid of that spacing, so a corner moved changes no sample).
pub(super) fn park(site: &Site, block: &[P], kind: ParkKind, fronts: Vec<bool>, key: u64) -> Park {
    let (x0, y0, x1, y1) = block.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let s = PARK_GROUND_FT;
    let snap = |v: f64, c: f64| crate::core::floor((v + c) / s) * s - c;
    let origin = [snap(x0, site.center[0]), snap(y0, site.center[1])];
    let (nx, ny) = (((x1 - origin[0]) / s) as usize + 2, ((y1 - origin[1]) / s) as usize + 2);
    let pts: Vec<P> = (0..ny).flat_map(|j| (0..nx).map(move |i| [origin[0] + i as f64 * s, origin[1] + j as f64 * s])).collect();
    // (Wet: on the water or by a river, whose valley the drawn ground cuts otherwise than planned.)
    let ground = ParkGround {
        origin,
        nx,
        ny,
        h: pts.iter().map(|&p| site.height(p) as f32).collect(),
        wet: pts.iter().map(|&p| site.wet(p) || site.near_river(p, 80.0)).collect(),
        water: pts.iter().map(|&p| site.wet(p)).collect(),
    };
    Park { pts: block.to_vec(), kind, fronts, ground, key, cemetery: None, spot: places::Spot::None }
}

pub(super) fn split_field(poly: &[P], max_area: f64, rng: &mut Pcg32, out: &mut Vec<Vec<P>>, depth: u32) {
    if area(poly).abs() <= max_area || depth > 10 {
        out.push(poly.to_vec());
        return;
    }
    let axis = obb(poly).axis;
    let (lo, hi) = extent(poly, axis);
    let cut = lo + (hi - lo) * (0.5 + rng.range(-0.2, 0.2));
    let mut n = axis;
    if rng.next_f64() < 0.5 {
        let t = rng.range(-std::f64::consts::FRAC_PI_8, std::f64::consts::FRAC_PI_8);
        let (c, s) = (libm::cos(t), libm::sin(t));
        n = [axis[0] * c - axis[1] * s, axis[0] * s + axis[1] * c];
    }
    let cen = centroid(poly);
    let at = add(cen, mul(axis, cut - dot(cen, axis)));
    let cc = dot(at, n);
    const LANE_HALF: f64 = 10.0;
    for half in [clean(&clip_plain(poly, n, cc - LANE_HALF)), clean(&clip_plain(poly, mul(n, -1.0), -(cc + LANE_HALF)))] {
        if half.len() >= 3 && area(&half).abs() > 1.0 {
            split_field(&half, max_area, rng, out, depth + 1);
        }
    }
}

fn smoothstep_f(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0).max(1e-9)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Level dry ground everywhere (`qa_fill`).
struct Level;

impl Ground for Level {
    fn buildable(&self, _: &[P]) -> bool {
        true
    }
    fn pad(&self, _: &[P]) -> f32 {
        0.0
    }
    fn height(&self, _: P) -> f64 {
        0.0
    }
    fn wet(&self, _: P) -> bool {
        false
    }
    fn near_water(&self, _: P, _: f64) -> bool {
        false
    }
}

/// QA entry for `towncheck fills`; generation never calls it. `region` (town-local ft) filled
/// through `Block::Region` as a hand-laid block is: `ward`'s lots fronting `fronts`, its `District`
/// rolled from `seed` and its lots cut by a stream of its own, on level dry ground in a town of
/// `tier`. Returns the buildings' outlines and the alleys cut (centre lines; a town's only).
pub fn qa_fill(region: &[Shape], fronts: &[Front], ward: Ward, seed: u64, tier: Tier) -> (Vec<Vec<P>>, Vec<(P, P)>) {
    let dp = District::roll(ward, seed);
    let mut rng = Pcg32::new(seed, 47);
    let mut l = Layout::empty(0, [0.0, 0.0], 0.0, tier);
    build_block(&Level, Block::Region(region), fronts, ward, &dp, &mut rng, &mut l, None);
    let alleys = l.roads.iter().filter(|r| r.0.len() == 2).map(|r| (r.0[0], r.0[1])).collect();
    (l.buildings.into_iter().map(|b| b.poly).collect(), alleys)
}
