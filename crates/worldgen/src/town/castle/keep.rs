//! Keeps: kinds, frames, tops and placing the keep.

use super::*;

/// The kinds of keep (`Building::keep`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeepKind {
    /// A great tower, nearly square, with a tower on each corner.
    Great,
    /// A long hall with a forebuilding before its door and towers at its far end.
    Hall,
    /// A block with a wing at one end, towers at its outer corners.
    L,
    /// A block with a wing from the middle of a long side.
    T,
    /// A round shell keep on a motte.
    Shell,
    /// A small tower keep with no towers of its own (a court too small for any other).
    Plain,
}

/// A keep's pieces in its own frame (ft, `s` along its axis, `t` across, its body about the
/// middle), each convex and wound anticlockwise; towers last.
struct Frame {
    parts: Vec<(Vec<P>, PartKind)>,
}

impl Frame {
    fn rect(&mut self, s0: f64, s1: f64, t0: f64, t1: f64, kind: PartKind) {
        self.parts.push((vec![[s0, t0], [s1, t0], [s1, t1], [s0, t1]], kind));
    }

    /// A tower `d` squares across on the corner (`s`, `t`) of a block, standing out from it along
    /// `ds`, `dt` (each 1 or -1): its middle on the corner (a whole number of squares across) or
    /// half a square out (an odd number), so its box lies on the block's squares.
    fn tower(&mut self, s: f64, t: f64, ds: f64, dt: f64, d: u32, round: bool) {
        let r = 2.5 * d as f64;
        let off = if d % 2 == 0 { 0.0 } else { 2.5 };
        let c = [s + ds * off, t + dt * off];
        let poly = if round {
            (0..16)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 16.0;
                    [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)]
                })
                .collect()
        } else {
            vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] - r], [c[0] + r, c[1] + r], [c[0] - r, c[1] + r]]
        };
        self.parts.push((poly, PartKind::Tower { round }));
    }
}

/// Feet rounded to whole squares (5 ft), at least `min`.
pub(super) fn sq(v: f64, min: f64) -> f64 {
    (crate::core::round(v / 5.0) * 5.0).max(min)
}

/// How much longer a keep's sides are than its kinds' sizes below say (v63: a fifth, its area
/// about half as much again, in the same citadel patch).
const KEEP_GROW: f64 = 1.2;
/// The scales a keep is tried at, the biggest first (`place_keep`): down to the sizes the kinds
/// had before `KEEP_GROW`'s smallest, so a tight court still takes the kind it rolled.
/// The share of keeps whose corner turrets are square.
pub(super) const KEEP_SQUARE_SHARE: f64 = 0.35;
pub(in crate::town) const KEEP_FITS: [f64; 7] = [1.0, 0.86, 0.74, 0.64, 0.55, 0.47, 0.4];

/// The pieces of a keep of `kind` scaled by `f` (its rolls in `rolls`, 0..1 each): its frame.
fn keep_frame(kind: KeepKind, f: f64, rolls: &[f64; 6], round: bool) -> Frame {
    let f = f * KEEP_GROW;
    let mut fr = Frame { parts: Vec::new() };
    let sgn = |r: f64| if r < 0.5 { -1.0 } else { 1.0 };
    match kind {
        KeepKind::Great => {
            let l = sq(f * (50.0 + 25.0 * rolls[0]), 35.0);
            let w = sq(l * (0.8 + 0.2 * rolls[1]), 30.0);
            let d = if w < 45.0 {
                4
            } else if w < 60.0 {
                5
            } else {
                6
            };
            let (hl, hw) = (0.5 * l, 0.5 * w);
            fr.rect(-hl, hl, -hw, hw, PartKind::Body);
            for (ds, dt) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                fr.tower(ds * hl, dt * hw, ds, dt, d, round);
            }
        }
        KeepKind::Hall => {
            let l = sq(f * (75.0 + 35.0 * rolls[0]), 50.0);
            let w = sq(f * (35.0 + 12.0 * rolls[1]), 30.0);
            let (hl, hw) = (0.5 * l, 0.5 * w);
            fr.rect(-hl, hl, -hw, hw, PartKind::Body);
            // The forebuilding before one end of a long side, the towers at the other end.
            let (end, side) = (sgn(rolls[2]), sgn(rolls[3]));
            let fw = sq(f * (20.0 + 10.0 * rolls[4]), 15.0);
            let fd = sq(f * (15.0 + 10.0 * rolls[5]), 10.0);
            let s0 = if end > 0.0 { hl - 10.0 - fw } else { -hl + 10.0 };
            let (t0, t1) = if side > 0.0 { (hw - 5.0, hw + fd) } else { (-hw - fd, -hw + 5.0) };
            fr.rect(s0, s0 + fw, t0, t1, PartKind::Forebuilding);
            let d = if w < 40.0 { 4 } else { 5 };
            for dt in [-1.0, 1.0] {
                fr.tower(-end * hl, dt * hw, -end, dt, d, round);
            }
        }
        KeepKind::L | KeepKind::T => {
            let l = sq(f * (55.0 + 30.0 * rolls[0]), 40.0);
            let w = sq(f * (30.0 + 12.0 * rolls[1]), 30.0);
            let (hl, hw) = (0.5 * l, 0.5 * w);
            fr.rect(-hl, hl, -hw, hw, PartKind::Body);
            let side = sgn(rolls[3]);
            let wl = sq(f * (20.0 + 20.0 * rolls[4]), 15.0);
            let d = if w < 40.0 { 4 } else { 5 };
            let (t0, t1, tip) = if side > 0.0 { (hw - 5.0, hw + wl, hw + wl) } else { (-hw - wl, -hw + 5.0, -hw - wl) };
            if kind == KeepKind::L {
                // The wing at one end, its outer side on with the block's end.
                let end = sgn(rolls[2]);
                let ww = sq(w * (0.7 + 0.3 * rolls[5]), 20.0).min(l - 15.0);
                let (s0, s1) = if end > 0.0 { (hl - ww, hl) } else { (-hl, -hl + ww) };
                fr.rect(s0, s1, t0, t1, PartKind::Wing);
                fr.tower(end * hl, tip, end, side, d, round);
                fr.tower(-end * hl, -side * hw, -end, -side, d, round);
                fr.tower(end * hl, -side * hw, end, -side, d, round);
            } else {
                // The wing from the middle of a long side, on the block's squares.
                let ww = sq(w * (0.6 + 0.3 * rolls[5]), 15.0).min(l - 20.0);
                let s0 = -hl + crate::core::round((hl - 0.5 * ww) / 5.0) * 5.0;
                fr.rect(s0, s0 + ww, t0, t1, PartKind::Wing);
                fr.tower(s0, tip, -1.0, side, d, round);
                fr.tower(s0 + ww, tip, 1.0, side, d, round);
                fr.tower(-hl, -side * hw, -1.0, -side, d, round);
                fr.tower(hl, -side * hw, 1.0, -side, d, round);
            }
        }
        KeepKind::Plain => {
            let l = sq(f * (40.0 + 10.0 * rolls[0]), 20.0);
            let w = sq(l * (0.75 + 0.25 * rolls[1]), 20.0);
            fr.rect(-0.5 * l, 0.5 * l, -0.5 * w, 0.5 * w, PartKind::Body);
        }
        KeepKind::Shell => {
            let r = 2.5 * crate::core::round(f * (12.0 + 5.0 * rolls[0])).max(10.0);
            let ring: Vec<P> = (0..20)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 20.0;
                    [r * libm::cos(a), r * libm::sin(a)]
                })
                .collect();
            fr.parts.push((ring, PartKind::Body));
        }
    }
    fr
}

/// A small tower keep's body (`KeepKind::Plain`, local ft round its middle) scaled by `f`: a tower
/// house's (`towerhouse`).
pub(in crate::town) fn plain_body(f: f64, rolls: &[f64; 6]) -> Vec<P> {
    keep_frame(KeepKind::Plain, f, rolls, true).parts.swap_remove(0).0
}

/// A keep's flat top this big (sq ft) or more is roofed.
const BIG_TOP: f64 = 1_100.0;

/// How each of a keep's pieces is topped, as the battlemap and tiles draw it (`seed`: the world's
/// stream for it): 0 open battlements (a wall walk of stone flags in its merlons, a broad one's
/// middle flat dark lead), 1 a hipped roof of lead, 2 of slate. A wing's top, and the donjon's at
/// one end of a long body (a squarer body is all hall roofs: 2); a big top (`BIG_TOP`) is roofed,
/// a smaller one open but in some keeps (rolled per keep), lead or slate rolled per keep too; a
/// long body's code has `DONJON_LOW` set when its donjon stands at the end nearer its first corner
/// (along its longer side from there, as the battlemap measures it), its hall range under hipped
/// roofs inside a wall walk either way. A forebuilding's top is a walled platform: 0. Towers: 2
/// (their cones and pyramids).
pub fn keep_tops(b: &Building, seed: u64) -> Vec<u8> {
    let c = centroid(&b.poly);
    let h = crate::core::rng::hash2(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64);
    let roofed = crate::core::rng::unit(h) < 0.35;
    let roof = if crate::core::rng::unit(crate::core::rng::hash2(h, 1, 2)) < 0.5 { 1 } else { 2 };
    b.parts
        .iter()
        .map(|p| {
            let top = match p.kind {
                PartKind::Tower { .. } => return 2,
                PartKind::Body if p.poly.len() == 4 => {
                    // (The donjon as drawn: a square at one end of a long body.)
                    let (a, w) = (dist(p.poly[0], p.poly[1]), dist(p.poly[1], p.poly[2]));
                    let (l, w) = (a.max(w), a.min(w));
                    if l <= 1.35 * w {
                        return 2;
                    }
                    let low = if crate::core::rng::unit(crate::core::rng::hash2(h, 3, 4)) < 0.5 { DONJON_LOW } else { 0 };
                    return (if w * w.min(0.42 * l) >= BIG_TOP || roofed { roof } else { 0 }) | low;
                }
                PartKind::Body => return 2,
                PartKind::Forebuilding => return 0,
                _ => area(&p.poly).abs(),
            };
            if top >= BIG_TOP || roofed { roof } else { 0 }
        })
        .collect()
}

/// Set in a long keep body's top code (`keep_tops`): its donjon stands at its first corner's end.
pub const DONJON_LOW: u8 = 4;

/// How a castle's keep with no pieces (reshaped or drawn by hand) is topped, as `keep_tops` tops a
/// body: its donjon (a square at one end of a long keep, as the battlemap draws it), else the whole
/// top if it has four corners; 0 for any other building.
pub fn plain_keep_top(b: &Building, seed: u64) -> u8 {
    let castle = b.ward == Ward::Castle || b.func.is_some_and(|f| super::super::catalog::CATALOG[f as usize].key == "castle");
    if !b.parts.is_empty() || b.poly.len() != 4 || !castle {
        return 0;
    }
    let c = centroid(&b.poly);
    let h = crate::core::rng::hash2(seed, crate::core::round(c[0]) as i64, crate::core::round(c[1]) as i64);
    let roofed = crate::core::rng::unit(h) < 0.35;
    let roof = if crate::core::rng::unit(crate::core::rng::hash2(h, 1, 2)) < 0.5 { 1 } else { 2 };
    let (a, w) = (dist(b.poly[0], b.poly[1]), dist(b.poly[1], b.poly[2]));
    let (l, w) = (a.max(w), a.min(w));
    let top = if l > 1.35 * w { w * w.min(0.42 * l) } else { l * w };
    if top >= BIG_TOP || roofed { roof } else { 0 }
}

/// Where a shell keep's motte reaches past its wall (ft), and how far its slope runs: a mound
/// eased up from the ground, nowhere steep enough for a cliff (its rise 10 to 12 ft).
const MOTTE_TOP: f64 = 8.0;
const MOTTE_SLOPE: f64 = 36.0;

/// Where a keep can stand: this far in from its curtain (ft).
pub(super) const KEEP_ROOM: f64 = 36.0;
/// Where a small tower keep can stand when no spot `KEEP_ROOM` in holds a keep (ft).
pub(super) const SMALL_ROOM: f64 = 18.0;
/// A keep built against the curtain: this far in from it (ft), and the lane kept open before its
/// face (ft).
const AGAINST: f64 = 6.0;
const KEEP_LANE: f64 = 10.0;
/// A keep set into the curtain: its outer face this far past the curtain's line (ft).
const SUNK: f64 = 2.0;

/// The keep: of a kind rolled (a shell keep in a small court now and then), as big as the court
/// holds clear of its curtain (16 ft) and its towers, on the first of `spots` it fits (the highest
/// ground first), on ground level enough to build on. Its building (4 storeys, the castle's
/// pieces) and a shell keep's motte, its way up and the shell's gate toward the court's gate
/// (`gate`).
#[allow(clippy::too_many_arguments)]
pub(super) fn place_keep<G: Ground>(
    site: &G,
    court: &[P],
    spots: &[(P, f64)],
    more: &[(P, f64)],
    axis: P,
    square: bool,
    towers: &[(P, f64)],
    by_ring: &[(P, f64)],
    own: &[bool],
    curtain: &[(P, P)],
    gates: &[P],
    lanes: &[(P, P)],
    gate: P,
    room: &dyn Fn(&[P]) -> bool,
    (shape, drawn): (&KeepShape, &[Vec<P>]),
    prng: &mut Pcg32,
    report: &mut Option<String>,
) -> Option<(Building, Option<Motte>)> {
    const CLEAR: f64 = 16.0;
    let roll = prng.next_f64();
    let rolls: [f64; 6] = std::array::from_fn(|_| prng.next_f64());
    // (A keep drawn by hand, or a round great tower: one piece, drawn as a keep of pieces is: a
    // round one a great round tower (walled, roofed, its door), one of four corners a keep's body under its
    // roof or battlements; any other shape its flat top behind a parapet. 4 storeys, as any.)
    let plain = |mut poly: Vec<P>| {
        if area(&poly) < 0.0 {
            poly.reverse();
        }
        let parts = match (shape, poly.len()) {
            (KeepShape::Round, _) | (_, 4) => vec![Part { poly: poly.clone(), kind: PartKind::Body }],
            _ => Vec::new(),
        };
        Building {
            parts,
            pad_ft: site.pad(&poly),
            poly,
            ward: Ward::Castle,
            func: None,
            residential: 4,
            name: None,
            floors: 4,
            structure: Structure::Roofed,
            roof: None,
            tint: None,
            material: None,
            shape: None,
            shade: None,
            id: 0,
            keep: Some(KeepKind::Plain),
            deck: None,
        }
    };
    // (Clear of the buildings drawn by hand in the yard.)
    let off_drawn = |poly: &[P]| drawn.iter().all(|d| apart_by(d, poly) >= DRAWN_CLEAR);
    // A drawn keep stands as drawn; what it stands on that it must not (a tower, a gate or its way
    // in, a building drawn in the yard) is told (`Report::keep`: `agent::works_spot` refuses it).
    if let KeepShape::Drawn(p) = shape {
        let k = plain(p.clone());
        let at = |q: P| format!("({:.0}, {:.0})", q[0], q[1]);
        let why = if let Some((t, _)) = by_ring.iter().chain(towers).find(|(t, r)| inset_depth(&k.poly, *t) > -(r + 1.0)) {
            Some(format!("on a tower of the curtain at {}", at(*t)))
        } else if let Some(g) = gates.iter().find(|g| inset_depth(&k.poly, **g) > -GATE_HALF) {
            Some(format!("on a gate at {}", at(*g)))
        } else if let Some((q, _)) = lanes.iter().find(|&&(q, r)| super::super::poly_seg_dist(&k.poly, q, r) < 4.0 || contains(&k.poly, q)) {
            Some(format!("across the way in from the gate at {}", at(*q)))
        } else {
            drawn.iter().find(|d| apart_by(d, &k.poly) < 1.0).map(|d| format!("on a building drawn in the yard at {}", at(centroid(d))))
        };
        return Some((k, None)).inspect(|_| *report = why);
    }
    // A round great tower: as big as the court holds clear of its curtain and towers, on the first
    // spot it fits.
    if *shape == KeepShape::Round {
        for f in KEEP_FITS {
            let r = 2.5 * crate::core::round(f * (10.0 + 4.0 * rolls[0])).max(4.0);
            for &(c, _) in spots.iter().chain(more) {
                let poly: Vec<P> = (0..24)
                    .map(|k| {
                        let a = std::f64::consts::TAU * k as f64 / 24.0;
                        [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)]
                    })
                    .collect();
                let fits = poly.iter().all(|p| inset_depth(court, *p) >= CLEAR) && !court.iter().any(|p| contains(&poly, *p)) && room(&poly);
                let off_towers = towers.iter().all(|(t, tr)| inset_depth(&poly, *t) < -(tr + CLEAR));
                if fits && off_towers && off_drawn(&poly) && site.buildable(&poly) {
                    return Some((plain(poly), None));
                }
            }
        }
    }
    let small = obb(&hull(court.to_vec())).short < 150.0;
    let first = if roll < if small { 0.2 } else { 0.08 } {
        if *shape == KeepShape::Square { KeepKind::Great } else { KeepKind::Shell }
    } else if roll < 0.45 {
        KeepKind::Great
    } else if roll < 0.65 {
        KeepKind::Hall
    } else if roll < 0.85 {
        KeepKind::L
    } else {
        KeepKind::T
    };
    // Square towers on a square-towered castle's keep, else round.
    let round = !square;
    let v = [-axis[1], axis[0]];
    let world = |c: P, q: P| add(c, add(mul(axis, q[0]), mul(v, q[1])));
    // (Else a smaller great tower; in a court too small for one, a tower keep without corner towers,
    // last off the middle, between the curtain's towers: `more`.)
    for (kind, clear_by, spots) in [(first, CLEAR, spots), (KeepKind::Great, CLEAR, spots), (KeepKind::Plain, 8.0, spots), (KeepKind::Plain, 8.0, more)] {
        for f in KEEP_FITS {
            let fr = keep_frame(kind, f, &rolls, round);
            for &(c, _) in spots {
                let mut parts: Vec<Part> = fr
                    .parts
                    .iter()
                    .map(|(p, k)| Part {
                        poly: p.iter().map(|q| world(c, *q)).collect(),
                        kind: *k,
                    })
                    .collect();
                // (A shell keep's ring from its gate round: its first corner toward the court's gate.)
                let way = sub(gate, c);
                let way = if len(way) > 1e-6 { mul(way, 1.0 / len(way)) } else { axis };
                if kind == KeepKind::Shell {
                    let r = dist(parts[0].poly[0], c);
                    let a0 = libm::atan2(way[1], way[0]);
                    parts[0].poly = (0..20)
                        .map(|k| {
                            let a = a0 + std::f64::consts::TAU * k as f64 / 20.0;
                            [c[0] + r * libm::cos(a), c[1] + r * libm::sin(a)]
                        })
                        .collect();
                }
                let Some(poly) = union_outline(&parts.iter().map(|p| p.poly.clone()).collect::<Vec<_>>()) else {
                    continue;
                };
                let reach = if kind == KeepKind::Shell { MOTTE_TOP + MOTTE_SLOPE } else { 0.0 };
                let fits = poly.iter().all(|p| inset_depth(court, *p) >= clear_by + reach) && !court.iter().any(|p| contains(&poly, *p)) && room(&poly);
                // (Off the curtain's towers.)
                let off_towers = towers.iter().all(|(t, r)| inset_depth(&poly, *t) < -(r + clear_by + reach));
                if !fits || !off_towers || !off_drawn(&poly) {
                    continue;
                }
                let (pad, motte) = if kind == KeepKind::Shell {
                    if poly.iter().any(|p| site.wet(*p)) || site.wet(c) {
                        continue;
                    }
                    let r = poly.iter().map(|p| dist(*p, c)).fold(0.0, f64::max);
                    let top = site.height(c) as f32 + 10.0 + 2.0 * rolls[1] as f32;
                    (
                        top,
                        Some(Motte {
                            at: c,
                            top_r: r + MOTTE_TOP,
                            foot_r: r + MOTTE_TOP + MOTTE_SLOPE,
                            top,
                            way,
                        }),
                    )
                } else {
                    if !site.buildable(&poly) {
                        continue;
                    }
                    (site.pad(&poly), None)
                };
                let b = Building {
                    poly,
                    ward: Ward::Castle,
                    func: None,
                    residential: 4,
                    name: None,
                    floors: 4,
                    pad_ft: pad,
                    structure: Structure::Roofed,
                    roof: None,
                    tint: None,
                    material: None,
                    shape: None,
            shade: None,
                    id: 0,
                    parts,
                    keep: Some(kind),
                    deck: None,
                };
                return Some((b, motte));
            }
        }
    }
    // (Last, a court too narrow for one standing free: a tower keep built against the curtain, else
    // set into it (on a side of the castle's own curtain, `own`, its outer face just past the
    // curtain's line: `castle` cuts the curtain at it), on the sides farthest from the gate first,
    // the middle of a side first, clear of every tower by the bailey (`by_ring`: the town wall's as
    // laid too, standing or not) and of the gates and their ways, with a lane before its face.)
    let n = court.len();
    let ccw = area(court) > 0.0;
    let mut sides: Vec<usize> = (0..n).filter(|&e| dist(court[e], court[(e + 1) % n]) >= 30.0).collect();
    let far = |e: usize| dist(gate, lerp(court[e], court[(e + 1) % n], 0.5));
    sides.sort_by(|&a, &b| far(b).total_cmp(&far(a)).then(a.cmp(&b)));
    for (sunk, f) in [false, true].into_iter().flat_map(|k| KEEP_FITS.map(|f| (k, f))) {
        let fr = keep_frame(KeepKind::Plain, f, &rolls, round);
        let body = &fr.parts[0].0;
        let hl = body.iter().map(|q| q[0]).fold(0.0, f64::max);
        let hw = body.iter().map(|q| q[1]).fold(0.0, f64::max);
        for &e in sides.iter().filter(|&&e| !sunk || own[e]) {
            let (a, b) = (court[e], court[(e + 1) % n]);
            let el = dist(a, b);
            let d = mul(sub(b, a), 1.0 / el);
            let inward = if ccw { [-d[1], d[0]] } else { [d[1], -d[0]] };
            let steps = crate::core::floor((0.5 * el - hl) / 5.0) as i64;
            for k in 0..=(2 * steps).max(-1) {
                let off = 5.0 * ((k + 1) / 2) as f64 * if k % 2 == 1 { 1.0 } else { -1.0 };
                let c = add(lerp(a, b, 0.5), add(mul(d, off), mul(inward, if sunk { hw - SUNK } else { AGAINST + hw })));
                let at = |q: P| add(c, add(mul(d, q[0]), mul(inward, q[1])));
                let mut poly: Vec<P> = body.iter().map(|q| at(*q)).collect();
                if area(&poly) < 0.0 {
                    poly.reverse();
                }
                let inside = poly.iter().all(|p| inset_depth(court, *p) >= if sunk { -SUNK - 0.5 } else { AGAINST - 0.5 }) && !court.iter().any(|p| contains(&poly, *p));
                // (Set into the curtain where it stands all along the keep's outer face.)
                let walled = !sunk || (0..=4).all(|j| {
                    let p = add(lerp(at([-hl, -hw]), at([hl, -hw]), j as f64 / 4.0), mul(inward, SUNK));
                    curtain.iter().any(|&(q, r)| seg_dist(p, q, r) < 1.5)
                });
                let off_towers = by_ring.iter().all(|(t, r)| inset_depth(&poly, *t) < -(r + 1.0));
                let off_gates = gates.iter().all(|g| inset_depth(&poly, *g) < -GATE_HALF) && room(&poly) && lanes.iter().all(|&(p, q)| super::super::poly_seg_dist(&poly, p, q) >= 10.0 && !contains(&poly, p));
                // (The lane before its face: inside the court, off the towers.)
                let (f0, f1) = (at([-hl, hw]), at([hl, hw]));
                let mut lane = vec![f0, f1, add(f1, mul(inward, KEEP_LANE)), add(f0, mul(inward, KEEP_LANE))];
                if area(&lane) < 0.0 {
                    lane.reverse();
                }
                let open = lane.iter().all(|p| inset_depth(court, *p) >= 4.0) && by_ring.iter().all(|(t, r)| inset_depth(&lane, *t) < -(r - 3.0));
                if !inside || !walled || !off_towers || !off_gates || !open || !off_drawn(&poly) || !site.buildable(&poly) {
                    continue;
                }
                let parts = vec![Part { poly: poly.clone(), kind: PartKind::Body }];
                let b = Building {
                    pad_ft: site.pad(&poly),
                    poly,
                    ward: Ward::Castle,
                    func: None,
                    residential: 4,
                    name: None,
                    floors: 4,
                    structure: Structure::Roofed,
                    roof: None,
                    tint: None,
                    material: None,
                    shape: None,
            shade: None,
                    id: 0,
                    parts,
                    keep: Some(KeepKind::Plain),
                    deck: None,
                };
                return Some((b, None));
            }
        }
    }
    None
}
