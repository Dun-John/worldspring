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
    match block {
        Block::Plain(ring) => lay_block(site, ring, &mut fronts.to_vec(), false, ward, dp, rng, l, thin),
        Block::Region(set) => {
            for piece in clip::convex_pieces(set) {
                if fronts.iter().any(|f| f.fronted_by(&piece)) {
                    lay_block(site, &piece, &mut fronts.to_vec(), true, ward, dp, rng, l, thin);
                }
            }
        }
    }
}

/// One convex block's alleys, lots and buildings (`build_block`). The alleys' sides join
/// `fronts` as they are cut; `spans`: whether a lot must front a street along its stretch (a
/// region's piece) or only on its line (a plain block, as always).
#[allow(clippy::too_many_arguments)]
fn lay_block<G: Ground>(site: &G, block: &[P], fronts: &mut Vec<Front>, spans: bool, ward: Ward, dp: &District, rng: &mut Pcg32, l: &mut Layout, thin: Option<(&[P], f64)>) {
    let mut parts: Vec<Vec<P>> = Vec::new();
    let alley_area = dp.min_sq * dp.block_size;
    if area(block).abs() > alley_area * libm::pow(2.0, dp.size_chaos * (2.0 * rng.next_f64() - 1.0)) {
        let mut chords: Vec<(P, P)> = Vec::new();
        bisect(block, alley_area, (16.0 * dp.grid_chaos).max(2.0), ALLEY_HALF_FT, rng, &mut parts, fronts, &mut chords, 0);
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
    for part in parts {
        // (The sprawl thinning out: a block between alleys at a time.)
        if let Some((anchors, reach)) = thin {
            let c = centroid(&part);
            let d = anchors.iter().map(|a| dist(*a, c)).fold(f64::MAX, f64::min);
            let keep = if anchors.is_empty() { 0.35 } else { 1.0 - smoothstep_f(0.0, 0.9 * reach, d) };
            if rng.next_f64() > keep {
                continue;
            }
        }
        let mut lots: Vec<Vec<P>> = Vec::new();
        bisect(&part, dp.min_sq, lot_variance, 0.0, rng, &mut lots, &mut Vec::new(), &mut Vec::new(), 0);
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
            let Some(&Front { n, c, .. }) = fronts.iter().find(|f| if spans { f.fronted_by(&lot) } else { on_line_lot(&lot, f.n, f.c) }) else { continue };
            // (A terraced garden on a steep block: a house here and there.)
            if ward == Ward::Park && rng.next_f64() > 0.15 {
                continue;
            }
            // Irregular street fronts: some buildings stand back from the street line.
            let indent = rng.next_f64() * (crate::core::sqrt(la) / 3.0).min(16.0);
            let poly = if indent > 6.5 { clean(&clip_plain(&lot, n, c - indent)) } else { lot };
            if poly.len() < 3 || area(&poly).abs() < 150.0 || !(site.buildable(&poly) || (ward == Ward::Docks && site.stepped(&poly))) {
                continue;
            }
            let pad = site.pad(&poly);
            l.buildings.push(Building { poly, ward, func: None, residential: 0, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id: 0, parts: Vec::new(), keep: None, deck: None });
        }
    }
}

/// Cut off corners sharper than `min_deg` with a chord square to the corner's bisector,
/// `depth` ft in from the tip (keeps the polygon convex; houses don't come to a point).
fn blunt(poly: &[P], min_deg: f64, depth: f64) -> Vec<P> {
    let m = poly.len();
    if m < 3 {
        return poly.to_vec();
    }
    let cos_min = libm::cos(min_deg.to_radians());
    let mut out = poly.to_vec();
    for i in 0..m {
        let (p, a, b) = (poly[i], poly[(i + m - 1) % m], poly[(i + 1) % m]);
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
        out = clip_plain(&out, n, dot(p, n) - depth);
        if out.len() < 3 {
            return out;
        }
    }
    out
}

/// Half the width of an alley cut through a block (ft).
pub(super) const ALLEY_HALF_FT: f64 = 8.0;

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
fn bisect(poly: &[P], min_area: f64, variance: f64, gap: f64, rng: &mut Pcg32, out: &mut Vec<Vec<P>>, cuts: &mut Vec<Front>, chords: &mut Vec<(P, P)>, depth: u32) {
    let a = area(poly).abs();
    let avg4 = (rng.next_f64() + rng.next_f64() + rng.next_f64() + rng.next_f64()) / 4.0;
    if depth > 18 || a < min_area * libm::pow(variance, (avg4 * 2.0 - 1.0).abs()) {
        out.push(poly.to_vec());
        return;
    }
    let frame = obb(poly).axis;
    let c = centroid(poly);
    let m = poly.len();
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
        let cc = dot(x, nrm);
        let p1 = clean(&clip_plain(poly, nrm, cc - gap));
        let p2 = clean(&clip_plain(poly, mul(nrm, -1.0), -(cc + gap)));
        if p1.len() < 3 || p2.len() < 3 {
            continue;
        }
        let (a1, a2) = (area(&p1).abs(), area(&p2).abs());
        if a1.max(a2) > 2.0 * variance * a1.min(a2).max(1e-9) {
            continue;
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
        bisect(&p1, min_area, variance, gap, rng, out, cuts, chords, depth + 1);
        bisect(&p2, min_area, variance, gap, rng, out, cuts, chords, depth + 1);
        return;
    }
    out.push(poly.to_vec());
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
