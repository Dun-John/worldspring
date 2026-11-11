//! (R5 towns, item 115) A steppe town's yurts, set apart from one another: each its own size (about
//! 14 to 24 ft across by its household's wealth, as its lot allows: `ring`) and its own felt (off-white
//! to smoke-browned, a few with a coloured band or a sooty crown: `felt`, carried as the roof's colour
//! variant); bare trodden earth round each, wider at its door (`Yurt::trodden`, the battlemap's
//! `URBAN_YARD`); a small post-and-rail pen with a horse or two, or a cart and a haystack, beside about a
//! third of those with room on their lot (the yurt drawn up toward its door to make it, never onto a lane
//! or the door's open ground); and yurts standing close together (a cluster) facing one lane where they
//! all can (`finish`, by `town::yurt_door`'s rule, its swing less).
//!
//! All of it a pure function of the layout: `town::round_off` calls `finish` once every round roof
//! stands, in building order.

use super::geom::*;
use super::{Building, Layout, Prop, Site, Structure, roof};
use crate::battlemap::Kind;
use crate::core::hash::FastMap;
use crate::core::rng::{hash3, unit};

/// A yurt as laid out: its building's id, the lot it stands on (as the lot was before its ring: the
/// ward editor knows a lot laid out anew by it, `wards::inherit`), the bare ground round it, its pen.
#[derive(Clone, Debug)]
pub struct Yurt {
    pub id: u32,
    pub lot: Vec<P>,
    pub trodden: Vec<P>,
    pub pen: Option<Pen>,
}

/// A small post-and-rail pen (its corners, world ft) with a gate gap in the middle of side `gate`
/// (corners `gate`, `gate + 1`), the side toward its yurt.
#[derive(Clone, Copy, Debug)]
pub struct Pen {
    pub pts: [P; 4],
    pub gate: usize,
}

/// A yurt's radius range (ft): 14 to 24 ft across.
pub const LEAST_R: f64 = 7.0;
pub const MOST_R: f64 = 12.0;
/// How far the trodden ground reaches past a yurt's wall (ft), and how much further at its door.
pub const TRODDEN_FT: f64 = 3.0;
pub const TRODDEN_DOOR_FT: f64 = 5.0;
/// A pen's gate (ft wide).
pub const PEN_GATE_FT: f64 = 4.0;
/// Yurts this near (ft between their walls) stand in one cluster.
const CLUSTER_GAP: f64 = 14.0;
/// The share of yurts that look for room for a pen or a cart.
const EXTRA_SHARE: f64 = 0.36;

const KEY: i64 = 0x7975_7274;

/// A home roofed round in a steppe town: a yurt (`town::round_off`).
pub fn is_steppe_home(l: &Layout, b: &Building) -> bool {
    l.culture % 5 == 4 && b.func.is_none()
}

/// A yurt's ring on its lot (`roof::round_footprint` about the lot's widest middle): 14-24 ft across by
/// its household's wealth and a hash of its id, as its lot allows (1.5 ft of yard to the lot's sides,
/// at least `LEAST_R`).
pub fn ring(l: &Layout, b: &Building) -> Option<Vec<P>> {
    let u = unit(hash3(l.roof_seed, b.id as i64, KEY, 1));
    let (lo, hi) = match roof::wealth(b) {
        0 => (LEAST_R, 9.5),
        1 => (8.0, 11.0),
        _ => (9.5, MOST_R),
    };
    let want = lo + (hi - lo) * u;
    roof::round_footprint_sized(&b.poly, &|clear| want.min(clear - 1.5).max(LEAST_R))
}

/// A yurt's felt (`roof::Look::shade` 0-7, `app/src/gen/protocol.ts` `FELT`): 0 cream, 1 off-white,
/// 2 grey, 3 warm cream, 4 dusty grey, 5 smoke-browned, 6 cream with a red band, 7 off-white with a
/// sooty crown; `u` a unit hash.
pub fn felt(u: f64) -> u8 {
    const W: [(u8, u32); 8] = [(0, 3), (1, 3), (2, 2), (3, 2), (4, 2), (5, 2), (6, 1), (7, 1)];
    let total: u32 = W.iter().map(|w| w.1).sum();
    let mut t = u * total as f64;
    for &(s, w) in &W {
        if t < w as f64 {
            return s;
        }
        t -= w as f64;
    }
    0
}

/// A ring's middle, radius and door (the outward unit at its first side's middle).
pub fn frame(ring: &[P]) -> (P, f64, P) {
    // (The corners' mean: exact far from the origin, where `centroid` loses feet.)
    let c = mean(ring);
    let m = lerp(ring[0], ring[1 % ring.len()], 0.5);
    let d = sub(m, c);
    (c, dist(ring[0], c), mul(d, 1.0 / len(d).max(1e-9)))
}

/// The bare ground round a yurt: `TRODDEN_FT` past its wall, `TRODDEN_DOOR_FT` more before its door,
/// its edge a little uneven (by a hash of its id).
fn trodden(ring: &[P], id: u32, seed: u64) -> Vec<P> {
    let (c, r, n) = frame(ring);
    let a0 = libm::atan2(n[1], n[0]);
    const N: usize = 20;
    let mut out: Vec<P> = (0..N)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / N as f64;
            let front = libm::cos(t).max(0.0);
            let wob = 0.8 * (unit(hash3(seed, id as i64, KEY + 3, k as i64)) - 0.5);
            let rr = r + TRODDEN_FT + TRODDEN_DOOR_FT * front * front + wob;
            let a = a0 + t;
            add(c, [rr * libm::cos(a), rr * libm::sin(a)])
        })
        .collect();
    if area(ring) < 0.0 {
        out.reverse();
    }
    out
}

/// The ground before a yurt's door kept open (a box out from its door side).
fn door_ground(ring: &[P]) -> [P; 4] {
    let (c, r, n) = frame(ring);
    let a = add(c, mul(n, r - 1.0));
    let b = add(c, mul(n, r + super::YURT_CLEAR_FT + 3.0));
    super::horses::seg_box(a, b, 12.0)
}

/// Whether `p` lies in the fan before a yurt's door (within 55 degrees of it, out to its open ground).
fn in_door_fan(ring: &[P], p: P) -> bool {
    let (c, r, n) = frame(ring);
    let v = sub(p, c);
    let d = len(v);
    d < r + super::YURT_CLEAR_FT + 4.0 && dot(v, n) > d * libm::cos(55f64.to_radians())
}

fn boxes_hit(a: &[P], b: &[P], pad: f64) -> bool {
    poly_gap(a, b) < pad || contains(a, b[0]) || contains(b, a[0])
}

/// Finish a layout's yurts (`found`: each building index and the lot it stood on, in building order):
/// clusters facing one lane, then pens and carts, then the trodden ground; into `l.yurts`, their props
/// into `l.props`.
pub(crate) fn finish(l: &mut Layout, site: Option<&Site>, found: Vec<(usize, Vec<P>)>) {
    l.yurts.clear();
    if found.is_empty() {
        return;
    }
    cluster_doors(l, site, &found);
    let mut placed: Vec<[P; 4]> = Vec::new();
    let mut props: Vec<Prop> = Vec::new();
    let mut pens: Vec<Option<Pen>> = vec![None; found.len()];
    // (Each yurt's door ground, for the others' pens and carts.)
    let rings: Vec<Vec<P>> = found.iter().map(|(i, _)| l.buildings[*i].poly.clone()).collect();
    let mut grounds: Vec<[P; 4]> = rings.iter().map(|r| door_ground(r)).collect();
    for (k, (i, lot)) in found.iter().enumerate() {
        let id = l.buildings[*i].id;
        if unit(hash3(l.roof_seed, id as i64, KEY, 2)) >= EXTRA_SHARE {
            continue;
        }
        // (A pen for two horses or one, or a cart: the first that fits, in an order by its hash.)
        let order: [u8; 3] = if unit(hash3(l.roof_seed, id as i64, KEY, 4)) < 0.55 { [2, 1, 0] } else { [0, 2, 1] };
        let Some((ring, got)) = order.iter().find_map(|&what| extra(l, site, *i, lot, what, &placed, &grounds, k)) else {
            continue;
        };
        grounds[k] = door_ground(&ring);
        l.buildings[*i].poly = ring;
        placed.extend(got.boxes);
        props.extend(got.props);
        pens[k] = got.pen;
    }
    for (k, (i, lot)) in found.into_iter().enumerate() {
        let b = &l.buildings[i];
        let trodden = trodden(&b.poly, b.id, l.roof_seed);
        l.yurts.push(Yurt {
            id: b.id,
            lot,
            trodden,
            pen: pens[k],
        });
    }
    l.props.extend(props);
}

/// Yurts standing close together (walls within `CLUSTER_GAP`) facing one lane: the lane or street
/// every one of them has in reach (`town::yurt_door`'s), nearest to them all; each then turned to its
/// nearest point on it (by the door rule: open ground before it, else the next side that has it).
fn cluster_doors(l: &mut Layout, site: Option<&Site>, found: &[(usize, Vec<P>)]) {
    let n = found.len();
    let frames: Vec<(P, f64)> = found
        .iter()
        .map(|(i, _)| {
            let (c, r, _) = frame(&l.buildings[*i].poly);
            (c, r)
        })
        .collect();
    let mut up: Vec<usize> = (0..n).collect();
    fn root(up: &mut [usize], mut a: usize) -> usize {
        while up[a] != a {
            up[a] = up[up[a]];
            a = up[a];
        }
        a
    }
    const CELL: f64 = 60.0;
    let cell = |p: P| (crate::core::floor(p[0] / CELL) as i64, crate::core::floor(p[1] / CELL) as i64);
    let mut grid: FastMap<(i64, i64), Vec<usize>> = FastMap::default();
    for (k, f) in frames.iter().enumerate() {
        grid.entry(cell(f.0)).or_default().push(k);
    }
    for a in 0..n {
        let (cx, cy) = cell(frames[a].0);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &b in grid.get(&(cx + dx, cy + dy)).map(|v| v.as_slice()).unwrap_or(&[]) {
                    if b > a && dist(frames[a].0, frames[b].0) < frames[a].1 + frames[b].1 + CLUSTER_GAP {
                        let (ra, rb) = (root(&mut up, a), root(&mut up, b));
                        if ra != rb {
                            up[ra.max(rb)] = ra.min(rb);
                        }
                    }
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); n];
    for a in 0..n {
        let r = root(&mut up, a);
        groups[r].push(a);
    }
    let ways: Vec<Vec<P>> = l.roads.iter().map(|r| r.0.clone()).chain(l.streets.iter().cloned()).collect();
    let near = |w: &[P], c: P| -> (f64, P) {
        let mut best = (f64::MAX, c);
        for e in w.windows(2) {
            let ab = sub(e[1], e[0]);
            let t = (dot(sub(c, e[0]), ab) / dot(ab, ab).max(1e-12)).clamp(0.0, 1.0);
            let q = add(e[0], mul(ab, t));
            let d = dist(c, q);
            if d < best.0 {
                best = (d, q);
            }
        }
        best
    };
    for g in groups.iter().filter(|g| g.len() >= 2) {
        // (The way every one has in reach, the least far from them all.)
        let mut best: Option<(f64, usize)> = None;
        for (w, way) in ways.iter().enumerate() {
            let mut sum = 0.0;
            let mut all = true;
            for &k in g {
                let d = near(way, frames[k].0).0;
                if d > super::YURT_LANE_REACH {
                    all = false;
                    break;
                }
                sum += d;
            }
            if all && best.is_none_or(|b| sum < b.0) {
                best = Some((sum, w));
            }
        }
        let Some((_, w)) = best else { continue };
        for &k in g {
            let (i, lot) = (&found[k].0, &found[k].1);
            let aim = near(&ways[w], frames[k].0).1;
            let ring = l.buildings[*i].poly.clone();
            let ring = super::yurt_door(l, *i, ring, lot, site, Some(aim));
            l.buildings[*i].poly = ring;
        }
    }
}

struct Got {
    boxes: Vec<[P; 4]>,
    props: Vec<Prop>,
    pen: Option<Pen>,
}

/// A pen (`pen`) or a cart and a haystack beside yurt `i` on its lot: the ring as it stands or drawn up
/// toward its door (its door still on open ground), the spot furthest from the door and nearest the
/// wall that fits: inside the lot a foot in, clear of its wall by 2.5 ft, of every door's open ground,
/// of other roofs, lanes, walls, water and what the others set out. `None` where none fits.
#[allow(clippy::too_many_arguments)]
fn extra(l: &Layout, site: Option<&Site>, i: usize, lot: &[P], what: u8, placed: &[[P; 4]], grounds: &[[P; 4]], me: usize) -> Option<(Vec<P>, Got)> {
    let pen = what > 0;
    let ring0 = l.buildings[i].poly.clone();
    let (c0, r, n) = frame(&ring0);
    let id = l.buildings[i].id;
    let h = |k: i64| unit(hash3(l.roof_seed, id as i64, KEY, k));
    let edge_gap = |p: P| (0..lot.len()).map(|e| seg_dist(p, lot[e], lot[(e + 1) % lot.len()])).fold(f64::MAX, f64::min);
    // (How far the ring may be drawn up toward its door, still 1.5 ft inside its lot.)
    let mut slack = 0.0;
    while slack < 10.0 {
        let t = slack + 0.5;
        if ring0.iter().all(|&p| {
            let q = add(p, mul(n, t));
            contains(lot, q) && edge_gap(q) >= 1.5
        }) {
            slack = t;
        } else {
            break;
        }
    }
    let ax = obb(lot).axis;
    let axes = [ax, [-ax[1], ax[0]]];
    // (Two horses side by side, or one: a horse about 8.4 by 3.2 ft.)
    let (pl, pw) = if what == 2 {
        (12.5 + 1.5 * h(5), 11.5 + 1.5 * h(6))
    } else {
        (9.5 + 1.0 * h(5), 7.5 + 1.0 * h(6))
    };
    // (A pen's fence stops short of a roof or a way, `rural::battle`: kept clear of them.)
    let (wall, roof_pad, way_pad) = if pen { (3.5, 3.5, 5.5) } else { (2.5, 1.0, 1.5) };
    let near_roofs: Vec<&Building> = l
        .buildings
        .iter()
        .enumerate()
        .filter(|(k, o)| *k != i && o.structure == Structure::Roofed && dist(o.poly[0], c0) < 120.0)
        .map(|(_, o)| o)
        .collect();
    // (The ways, walls, squares and parks near it only.)
    let close = |a: P, b: P| seg_dist(c0, a, b) < 120.0;
    let ways: Vec<(P, P, f64)> = l
        .roads
        .iter()
        .flat_map(|r| r.0.windows(2).map(move |s| (s[0], s[1], 0.5 * r.2 + way_pad)))
        .chain(l.streets.iter().flat_map(|w| w.windows(2).map(move |s| (s[0], s[1], 5.0 + way_pad.max(3.0)))))
        .filter(|s| close(s.0, s.1))
        .collect();
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2).map(|s| (s[0], s[1]))).filter(|s| close(s.0, s.1)).collect();
    let opens: Vec<&Vec<P>> = l
        .plazas
        .iter()
        .map(|p| &p.pts)
        .chain(l.parks.iter().map(|p| &p.pts))
        .filter(|q| {
            let (x0, y0, x1, y1) = q.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| {
                (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1]))
            });
            c0[0] > x0 - 120.0 && c0[0] < x1 + 120.0 && c0[1] > y0 - 120.0 && c0[1] < y1 + 120.0
        })
        .collect();
    let others: Vec<usize> = (0..grounds.len()).filter(|&k| k != me && dist(grounds[k][0], c0) < 120.0).collect();
    for t in [0.0, 0.5 * slack, slack] {
        let ring: Vec<P> = ring0.iter().map(|&p| add(p, mul(n, t))).collect();
        if t > 0.0 {
            // (Its door still facing the same way onto open ground.)
            let turned = super::yurt_door(l, i, ring.clone(), lot, site, Some(add(frame(&ring).0, mul(n, 60.0))));
            if dot(frame(&turned).2, n) < 0.97 {
                continue;
            }
        }
        let (c, _, _) = frame(&ring);
        let mine = door_ground(&ring);
        // (Drawn up, its door ground clear of what the others set out.)
        if t > 0.0 && placed.iter().any(|q| super::horses::hit(q, &mine, 0.5)) {
            continue;
        }
        let fits = |b: &[P; 4]| -> bool {
            let pts = [b[0], b[1], b[2], b[3], lerp(b[0], b[2], 0.5)];
            pts[..4].iter().all(|&p| contains(lot, p) && edge_gap(p) >= 1.0)
                && !lot.iter().any(|&q| contains(b, q))
                && (0..4).all(|e| seg_dist(c, b[e], b[(e + 1) % 4]) >= r + wall)
                && !contains(b, c)
                && !pts.iter().any(|&p| in_door_fan(&ring, p))
                && !super::horses::hit(b, &mine, 0.5)
                && !others.iter().any(|&k| super::horses::hit(b, &grounds[k], 0.5))
                && !near_roofs.iter().any(|o| boxes_hit(b, &o.poly, roof_pad))
                && !placed.iter().any(|q| super::horses::hit(b, q, 1.0))
                && !l.props.iter().any(|p| dist(p.at, pts[4]) < 12.0)
                && !ways.iter().any(|&(a, e, half)| poly_gap(b, &[a, e]) < half)
                && !walls.iter().any(|&(a, e)| poly_gap(b, &[a, e]) < 5.0)
                && !opens.iter().any(|p| boxes_hit(b, p, 1.0))
                && !site.is_some_and(|s| pts.iter().any(|&p| s.wet(sub(p, s.center))))
        };
        // (Spots on a grid over the lot, furthest from the door and nearest the wall first.)
        let (x0, y0, x1, y1) = lot.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| {
            (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1]))
        });
        let mut best: Option<(f64, [P; 4], P)> = None;
        let (l0, w0) = if pen { (pl, pw) } else { (12.0, 6.0) };
        let mut y = y0;
        while y <= y1 {
            let mut x = x0;
            while x <= x1 {
                let p = [x, y];
                for u in axes {
                    let b = boxed(p, u, l0, w0);
                    let score = dot(sub(p, c), n) / r.max(1.0) + 0.1 * dist(p, c);
                    if best.as_ref().is_none_or(|bb| score < bb.0 - 1e-9) && fits(&b) {
                        best = Some((score, b, u));
                    }
                }
                x += 1.5;
            }
            y += 1.5;
        }
        let Some((_, b, u)) = best else { continue };
        let mid = lerp(b[0], b[2], 0.5);
        let mut items: Vec<([P; 4], Prop)> = Vec::new();
        let rot = libm::atan2(u[1], u[0]) as f32;
        if pen {
            // (A horse or two along it, nose either way.)
            let two = what == 2;
            let across = [-u[1], u[0]];
            let offs: &[f64] = if two { &[-0.22, 0.22] } else { &[0.0] };
            for (q, &o) in offs.iter().enumerate() {
                let at = add(mid, mul(across, o * pw));
                let flip = if h(8 + q as i64) < 0.5 { 0.0 } else { std::f32::consts::PI };
                let prop = Prop {
                    at,
                    rot: rot + flip,
                    kind: Kind::Horse,
                    variant: (h(10 + q as i64) * 4.0) as u8 % 4,
                    scale: super::horses::HORSE_SCALE,
                };
                items.push((super::horses::foot(Kind::Horse, at, prop.rot, prop.scale), prop));
            }
            // (Its gate on the side nearest the yurt; the whole pen kept, so nothing else lands in it.)
            let gate = (0..4)
                .min_by(|&a, &e| {
                    dist(lerp(b[a], b[(a + 1) % 4], 0.5), c)
                        .total_cmp(&dist(lerp(b[e], b[(e + 1) % 4], 0.5), c))
                        .then(a.cmp(&e))
                })
                .unwrap_or(0);
            return Some((
                ring,
                Got {
                    boxes: vec![b],
                    props: items.into_iter().map(|i| i.1).collect(),
                    pen: Some(Pen { pts: b, gate }),
                },
            ));
        }
        let flip = if h(8) < 0.5 { 0.0 } else { std::f32::consts::PI };
        let cart = Prop {
            at: mid,
            rot: rot + flip,
            kind: Kind::Cart,
            variant: (h(10) * 4.0) as u8 % 4,
            scale: 1.0,
        };
        items.push((super::horses::foot(Kind::Cart, mid, cart.rot, 1.0), cart));
        // (A haystack beside it where one fits, full size or smaller: off either end or either side.)
        let across = [-u[1], u[0]];
        let (hl, _) = super::horses::size(Kind::Haystack);
        'hay: for sc in [1.0, 0.75] {
            let hs = hl * sc;
            for (dir, off) in [
                (u, 6.0 + 0.5 * hs + 1.0),
                (mul(u, -1.0), 6.0 + 0.5 * hs + 1.0),
                (across, 3.0 + 0.5 * hs + 1.0),
                (mul(across, -1.0), 3.0 + 0.5 * hs + 1.0),
            ] {
                let at = add(mid, mul(dir, off));
                let hb = super::horses::foot(Kind::Haystack, at, rot, sc as f32);
                if fits(&hb) && !super::horses::hit(&hb, &b, 0.5) {
                    items.push((
                        hb,
                        Prop {
                            at,
                            rot,
                            kind: Kind::Haystack,
                            variant: (h(11) * 4.0) as u8 % 4,
                            scale: sc as f32,
                        },
                    ));
                    break 'hay;
                }
            }
        }
        return Some((
            ring,
            Got {
                boxes: items.iter().map(|i| i.0).collect(),
                props: items.into_iter().map(|i| i.1).collect(),
                pen: None,
            },
        ));
    }
    None
}

fn boxed(c: P, u: P, l: f64, w: f64) -> [P; 4] {
    let (a, b) = (mul(u, 0.5 * l), mul([-u[1], u[0]], 0.5 * w));
    [sub(sub(c, a), b), sub(add(c, a), b), add(add(c, a), b), add(sub(c, a), b)]
}

/// The pens' fences reaching `near` (world ft: x0, y0, x1, y1), a gap at each gate, for `rural::battle`.
pub fn fences(l: &Layout, near: [f64; 4]) -> Vec<(P, P)> {
    let mut out = Vec::new();
    for pen in l.yurts.iter().filter_map(|y| y.pen) {
        let b = pen.pts;
        if !b
            .iter()
            .any(|p| p[0] >= near[0] - 20.0 && p[0] <= near[2] + 20.0 && p[1] >= near[1] - 20.0 && p[1] <= near[3] + 20.0)
        {
            continue;
        }
        for e in 0..4 {
            let (a, q) = (b[e], b[(e + 1) % 4]);
            if e == pen.gate {
                let l = dist(a, q);
                let t = (0.5 * (l - PEN_GATE_FT) / l.max(1e-9)).clamp(0.0, 0.5);
                out.push((a, lerp(a, q, t)));
                out.push((lerp(a, q, 1.0 - t), q));
            } else {
                out.push((a, q));
            }
        }
    }
    out
}

/// The yurts on the town's plan: their trodden ground (a camp clearing's, kind 8), their pens' fences
/// (`horses::PLAN_FENCE`).
pub fn plan_polys(l: &Layout, push: &mut dyn FnMut(&[P], u32)) {
    for y in &l.yurts {
        push(&y.trodden, 8);
    }
    for (a, b) in fences(l, [f64::MIN / 4.0, f64::MIN / 4.0, f64::MAX / 4.0, f64::MAX / 4.0]) {
        if dist(a, b) > 0.5 {
            push(&super::horses::seg_box(a, b, 1.6), super::horses::PLAN_FENCE);
        }
    }
}
