//! A town wall's outline by its style (item 104): straight curtains as laid, angled bastions at the
//! main corners of a star-angled wall (`bastions`), or a rounded line with no sharp corners
//! (`round`). Both reshape the wall's pieces as `wall_pieces_spaced` lays them, after its towers are
//! set, so the towers keep their number and order (and their `t:` ids) but for the corner towers a
//! bastion stands in place of.

use super::geom::*;

/// A bastion's flanks stand this far from its corner along each curtain, as long as this out from it,
/// and its point this far out from the corner (ft, at full size; `SHRINK` the smaller try): faces
/// about 40-55 ft long (the user, 2026-11-11: half what they were).
pub const FLANK_AT: f64 = 26.0;
pub const FLANK_FT: f64 = 12.0;
pub const POINT_FT: f64 = 38.0;
const SHRINK: f64 = 0.7;
/// A wall turning more than this (degrees) at a corner may take a bastion, but not one turning by more
/// than `BASTION_SPIKE` (a spike: the town's paved strips reach past its point).
const BASTION_TURN: f64 = 20.0;
const BASTION_SPIKE: f64 = 80.0;
/// Bastions stand this far from a gate, and their corners this far apart (ft).
const BASTION_GATE: f64 = 90.0;
const BASTION_APART: f64 = 240.0;
/// A rounded corner cuts in (or bulges out) this far at most from where the corner was (ft): the wall
/// stays within its 18-ft strip and 8 ft of the patches' sides.
const ROUND_DEV: f64 = 7.0;
const ROUND_OUT: f64 = 3.0;

/// What `bastions` did: the bastions placed (each its outline: the flank's foot on the curtain, the
/// flank's tip, the point, the other flank's tip and foot, then the corner it stands in place of), and
/// how many corners it tried and dropped.
#[derive(Default)]
pub struct Bastions {
    pub placed: Vec<Vec<P>>,
    pub dropped: u32,
}

fn unit(v: P) -> P {
    mul(v, 1.0 / len(v).max(1e-9))
}

fn same(a: P, b: P) -> bool {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() < 1e-6
}

/// Two segments cross (not just touch).
fn crosses(p: P, q: P, r: P, t: P) -> bool {
    let c = |a: P, b: P, x: P| cross(sub(b, a), sub(x, a));
    let (d1, d2, d3, d4) = (c(p, q, r), c(p, q, t), c(r, t, p), c(r, t, q));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// The wall's pieces as laid, tidied: a piece ending on another's line (where the two meet, a
/// corner of one dropped or rounded off the other, or exactly on it) stopped half a foot short of it
/// instead of a hair across; a tower stood twice where a run passes one corner twice, once.
pub(super) fn tidy(pieces: &mut [Vec<P>], towers: &mut Vec<P>) {
    for i in 0..pieces.len() {
        for first in [true, false] {
            let n = pieces[i].len();
            if n < 2 {
                continue;
            }
            let (e, q) = if first { (pieces[i][0], pieces[i][1]) } else { (pieces[i][n - 1], pieces[i][n - 2]) };
            let mut hit: Option<P> = None;
            for (j, w) in pieces.iter().enumerate() {
                for (k, sg) in w.windows(2).enumerate() {
                    let own = j == i && (if first { k == 0 } else { k + 2 == n });
                    // (Or ending on it, off its corners: a hair either way as it is moved.)
                    let on = seg_dist(e, sg[0], sg[1]) < 0.25 && dist(e, sg[0]) > 0.25 && dist(e, sg[1]) > 0.25;
                    if own || !(on || crosses(e, q, sg[0], sg[1])) {
                        continue;
                    }
                    // (Where the end's stretch meets that line.)
                    let (u, v) = (sub(q, e), sub(sg[1], sg[0]));
                    let x = if on || cross(u, v).abs() < 1e-9 { e } else { add(e, mul(u, cross(sub(sg[0], e), v) / cross(u, v))) };
                    if dist(x, e) <= 3.0 && hit.is_none_or(|h| dist(h, q) < dist(x, q)) {
                        hit = Some(x);
                    }
                }
            }
            if let Some(x) = hit {
                let to = add(x, mul(unit(sub(q, x)), 0.5));
                if first { pieces[i][0] = to } else { pieces[i][n - 1] = to }
            }
        }
    }
    let mut kept: Vec<P> = Vec::new();
    towers.retain(|t| {
        let new = !kept.iter().any(|k| dist(*k, *t) < 0.5);
        if new {
            kept.push(*t);
        }
        new
    });
}

/// A corner of the wall's pieces: where piece `p`'s point `k` stands, and the points either side of it
/// (`a` before, `b` after), joining two pieces where one ends where the other starts (`seam`: the
/// piece that starts there and its first point).
struct Joint {
    p: usize,
    k: usize,
    a: P,
    v: P,
    b: P,
    seam: Option<usize>,
}

fn joints(pieces: &[Vec<P>]) -> Vec<Joint> {
    let mut out = Vec::new();
    for (p, w) in pieces.iter().enumerate() {
        for k in 1..w.len().saturating_sub(1) {
            out.push(Joint { p, k, a: w[k - 1], v: w[k], b: w[k + 1], seam: None });
        }
        // (A piece's end where another, or its own start (a ring with no gate), starts.)
        let n = w.len();
        if n < 2 {
            continue;
        }
        if let Some(q) = pieces.iter().position(|o| o.len() >= 2 && same(o[0], w[n - 1]) && !same(o[1], w[n - 2])) {
            out.push(Joint { p, k: n - 1, a: w[n - 2], v: w[n - 1], b: pieces[q][1], seam: Some(q) });
        }
    }
    out
}

/// The corner's turn (radians) and its outward direction (away from the side it turns to).
fn turn(j: &Joint) -> (f64, P, f64) {
    let (u1, u2) = (unit(sub(j.v, j.a)), unit(sub(j.b, j.v)));
    let th = libm::acos(dot(u1, u2).clamp(-1.0, 1.0));
    (th, unit(sub(u1, u2)), cross(u1, u2))
}

/// Angled bastions at up to `most` of the wall's sharpest outward corners (turning by more than
/// `BASTION_TURN`), away from the gates and from each other, not at a corner `keep` holds; each tried
/// at full size and then smaller, dropped where `clear` refuses its outline (roads, water, the map's
/// edge) or it would come near another stretch of wall or a tower. The corner's tower goes.
pub fn bastions(pieces: &mut [Vec<P>], towers: &mut Vec<P>, gates: &[P], most: usize, keep: &dyn Fn(P) -> bool, clear: &dyn Fn(&[P]) -> bool) -> Bastions {
    let mut out = Bastions::default();
    let mut cands: Vec<(f64, Joint)> = joints(pieces)
        .into_iter()
        .filter(|j| j.seam.is_none())
        .filter_map(|j| {
            let (th, o, _) = turn(&j);
            (th > BASTION_TURN.to_radians() && th < BASTION_SPIKE.to_radians() && dot(o, j.v) > 0.0 && !keep(j.v) && gates.iter().all(|g| dist(*g, j.v) >= BASTION_GATE)).then_some((th, j))
        })
        .collect();
    cands.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.v[0].total_cmp(&y.1.v[0])).then(x.1.v[1].total_cmp(&y.1.v[1])));
    let mut tried = 0;
    let mut laid: Vec<(usize, usize, Vec<P>)> = Vec::new();
    for (_, j) in cands {
        if tried >= most {
            break;
        }
        if laid.iter().any(|(_, _, q)| dist(q[5], j.v) < BASTION_APART) {
            continue;
        }
        tried += 1;
        let (u1, u2) = (unit(sub(j.v, j.a)), unit(sub(j.b, j.v)));
        let (_, o, side) = turn(&j);
        // (Out: the side away from the one it turns to.)
        let out_of = |u: P| if side > 0.0 { [u[1], -u[0]] } else { [-u[1], u[0]] };
        let (n1, n2) = (out_of(u1), out_of(u2));
        let room = 0.4 * dist(j.a, j.v).min(dist(j.v, j.b));
        let fits = [1.0, SHRINK].into_iter().find_map(|k| {
            let s = FLANK_AT * k;
            if s > room || towers.iter().any(|t| !same(*t, j.v) && dist(*t, j.v) < s + 14.0) {
                return None;
            }
            let (f1, f2) = (sub(j.v, mul(u1, s)), add(j.v, mul(u2, s)));
            let (g1, g2) = (add(f1, mul(n1, FLANK_FT * k)), add(f2, mul(n2, FLANK_FT * k)));
            let tip = add(j.v, mul(o, POINT_FT * k));
            let poly = vec![f1, g1, tip, g2, f2, j.v];
            // (Clear of the rest of the wall, its towers and the bastions already laid.)
            let edges = [(f1, g1), (g1, tip), (tip, g2), (g2, f2)];
            let near_wall = pieces.iter().enumerate().any(|(p, w)| {
                w.windows(2).enumerate().any(|(k, sg)| {
                    let own = p == j.p && (k + 1 == j.k || k == j.k);
                    !own && edges.iter().any(|&(a, b)| crosses(a, b, sg[0], sg[1]) || seg_dist(sg[0], a, b) < 12.0 || seg_dist(a, sg[0], sg[1]) < 12.0)
                })
            });
            let near_tower = towers.iter().any(|t| !same(*t, j.v) && edges.iter().any(|&(a, b)| seg_dist(*t, a, b) < 16.0));
            let near_bastion = laid.iter().any(|(_, _, q)| q.iter().any(|x| dist(*x, tip) < 60.0 || contains(q, tip)));
            (!near_wall && !near_tower && !near_bastion && clear(&poly)).then_some(poly)
        });
        match fits {
            Some(poly) => laid.push((j.p, j.k, poly)),
            None => out.dropped += 1,
        }
    }
    // (Each piece's corners replaced from the last, so the earlier stay where they are.)
    laid.sort_by(|x, y| (x.0, std::cmp::Reverse(x.1)).cmp(&(y.0, std::cmp::Reverse(y.1))));
    for (p, k, poly) in &laid {
        pieces[*p].splice(*k..*k + 1, poly[..5].iter().copied());
        towers.retain(|t| !same(*t, poly[5]));
    }
    laid.sort_by(|x, y| x.2[5][0].total_cmp(&y.2[5][0]).then(x.2[5][1].total_cmp(&y.2[5][1])));
    out.placed = laid.into_iter().map(|x| x.2).collect();
    out
}

/// Rounds every corner of the wall's pieces but those `keep` holds (gates are pieces' ends, and stay
/// put): an arc tangent to both stretches, cutting the corner by `ROUND_DEV` ft at most and taking up
/// to 45% of each, kept only where `ok` takes its middle and its points (no water). A tower on a
/// corner moves to its arc's middle, one along a stretch onto the arc where the arc takes it in.
pub fn round(pieces: &mut [Vec<P>], towers: &mut [P], keep: &dyn Fn(P) -> bool, ok: &dyn Fn(P) -> bool, inside: &dyn Fn(P) -> bool) {
    // (Arcs from the corners as laid; then put in, each piece's from its last.)
    let mut arcs: Vec<(usize, usize, Option<usize>, P, Vec<P>, P)> = Vec::new();
    for j in joints(pieces) {
        let (th, _, side) = turn(&j);
        if th < 2f64.to_radians() || keep(j.v) {
            continue;
        }
        let u1 = unit(sub(j.v, j.a));
        let half = 0.5 * th;
        let (tn, dev) = (libm::tan(half), 1.0 / libm::cos(half) - 1.0);
        let room = 0.45 * dist(j.a, j.v).min(dist(j.v, j.b));
        // (Bulging out of the town, toward the sprawl beyond it: by `ROUND_OUT` at most.)
        let toward_in = inside(add(j.v, mul(turn(&j).1, -2.0)));
        let most = if toward_in { ROUND_DEV } else { ROUND_OUT };
        let arc = [most, 0.5 * most].into_iter().find_map(|cut| {
            let rad = (room / tn).min(cut / dev.max(1e-9));
            if rad < 6.0 {
                return None;
            }
            let t = rad * tn;
            let t1 = sub(j.v, mul(u1, t));
            let toward = if side > 0.0 { [-u1[1], u1[0]] } else { [u1[1], -u1[0]] };
            let c = add(t1, mul(toward, rad));
            let a0 = libm::atan2(t1[1] - c[1], t1[0] - c[0]);
            let m = crate::core::ceil(th / 10f64.to_radians()).max(2.0) as usize;
            let sweep = if side > 0.0 { th } else { -th };
            let pts: Vec<P> = (0..=m).map(|i| {
                let a = a0 + sweep * i as f64 / m as f64;
                [c[0] + rad * libm::cos(a), c[1] + rad * libm::sin(a)]
            }).collect();
            let mid = add(c, mul(unit(sub(j.v, c)), rad));
            (ok(mid) && pts.iter().all(|p| ok(*p))).then_some((pts, mid))
        });
        if let Some((pts, mid)) = arc {
            arcs.push((j.p, j.k, j.seam, j.v, pts, mid));
        }
    }
    // (A corner's tower to its arc's middle; one along a stretch the arc takes in, onto the arc.)
    for (_, _, _, v, pts, mid) in &arcs {
        let (first, last) = (pts[0], pts[pts.len() - 1]);
        for t in towers.iter_mut() {
            if same(*t, *v) {
                *t = *mid;
            } else if seg_dist(*t, first, *v) < 1e-3 || seg_dist(*t, *v, last) < 1e-3 {
                let on = |s: &[P]| {
                    let d = sub(s[1], s[0]);
                    let f = (dot(sub(*t, s[0]), d) / dot(d, d).max(1e-9)).clamp(0.0, 1.0);
                    add(s[0], mul(d, f))
                };
                *t = pts.windows(2).map(on).min_by(|a, b| dist(*a, *t).total_cmp(&dist(*b, *t))).unwrap_or(*mid);
            }
        }
    }
    // (Each piece's corners put in from its last, so the earlier stay where they are; the starts of
    // pieces a seam's arc runs on into last of all.)
    arcs.sort_by(|x, y| (x.0, std::cmp::Reverse(x.1)).cmp(&(y.0, std::cmp::Reverse(y.1))));
    let mut starts: Vec<(usize, Vec<P>)> = Vec::new();
    for (p, k, seam, _, pts, mid) in arcs {
        match seam {
            None => {
                pieces[p].splice(k..k + 1, pts);
            }
            Some(q) => {
                // (Its first half ends this piece, the second starts the next.)
                let h = pts.len() / 2;
                let mut a: Vec<P> = pts[..h].to_vec();
                a.push(mid);
                let mut b = vec![mid];
                b.extend_from_slice(&pts[pts.len() - h..]);
                pieces[p].splice(k..k + 1, a);
                starts.push((q, b));
            }
        }
    }
    for (q, b) in starts {
        pieces[q].splice(0..1, b);
    }
}

/// The town wall as `build_town` lays it along its runs (`wall_pieces_spaced`), shaped by `style`'s
/// outline: its pieces, towers, gate towers, and the bastions (a star-angled wall's). The corners of a
/// castle's patch and those on the water or a river stay as they are.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(
    site: &super::Site,
    plan: &super::TownPlan,
    mesh: &super::mesh::Mesh,
    ward: &[Option<super::Ward>],
    style: &super::wallstyle::WallStyle,
    gate_vs: &[usize],
    hand: &super::wallgates::WallChanges,
) -> (Vec<Vec<P>>, Vec<P>, Vec<P>, Bastions) {
    use super::wallstyle::Outline;
    // (With the gates put in or taken out by hand: `wallgates`.)
    let (mut pieces, mut towers, gate_towers) = super::wallgates::pieces(plan, mesh, gate_vs, hand);
    let mut held: Vec<P> = (0..mesh.pos.len()).filter(|&v| plan.pinned[v]).map(|v| mesh.pos[v]).collect();
    for (i, f) in mesh.faces.iter().enumerate() {
        if ward.get(i).copied().flatten() == Some(super::Ward::Castle) {
            held.extend(f.iter().map(|&v| mesh.pos[v]));
        }
    }
    // (Nor where a main street meets the wall or runs along it.)
    for &(a, b) in &plan.main_edges {
        held.push(mesh.pos[a]);
        held.push(mesh.pos[b]);
    }
    let keep = |p: P| held.iter().any(|q| same(*q, p));
    let inner: Vec<Vec<P>> = (0..plan.n_inner).map(|i| mesh.face_pts(i)).collect();
    let out_of_town = |p: P| !inner.iter().any(|f| contains(f, p));
    let mut out = Bastions::default();
    match style.outline {
        Outline::Straight => {}
        Outline::Round => round(&mut pieces, &mut towers, &keep, &|p| !super::Ground::near_water(site, p, 3.0), &|p| !out_of_town(p)),
        Outline::Star => {
            let gates: Vec<P> = gate_vs.iter().map(|&g| mesh.pos[g]).collect();
            // (The ways in: from where each road arrives to its gate.)
            let ways: Vec<(P, P)> = plan.ends.iter().zip(&plan.gate_of).filter_map(|(e, g)| g.map(|g| (e.0, mesh.pos[g]))).collect();
            let t0 = site.t0;
            let (mw, mh) = (t0.height.w as f64 * t0.cell_ft, t0.height.h as f64 * t0.cell_ft);
            let ok = |p: P| {
                let w = add(p, site.center);
                !super::Ground::near_water(site, p, 6.0)
                    && !site.near_network_road(p, 8.0)
                    && !ways.iter().any(|&(a, b)| seg_dist(p, a, b) < 22.0)
                    && w[0] > 40.0
                    && w[1] > 40.0
                    && w[0] < mw - 40.0
                    && w[1] < mh - 40.0
                    && len(p) < site.reach - 20.0
            };
            // (Outside the town: off its patches, but for where the flanks meet the curtain.)
            let sprawl: Vec<Vec<P>> = (plan.n_inner..mesh.faces.len()).filter(|&i| !matches!(ward.get(i).copied().flatten(), None | Some(super::Ward::Farm))).map(|i| mesh.face_pts(i)).collect();
            let clear = |poly: &[P]| {
                let n = poly.len();
                // (Nor over the sprawl's patches: only open country or farmland.)
                !sprawl.iter().any(|f| poly[1..4].iter().chain([centroid(poly)].iter()).any(|p| contains(f, *p)))
                    && out_of_town(centroid(poly))
                    && poly[1..4].iter().all(|p| out_of_town(*p))
                    && ok(centroid(poly))
                    && (0..n).all(|k| {
                        let (a, b) = (poly[k], poly[(k + 1) % n]);
                        let m = crate::core::ceil(dist(a, b) / 4.0).max(1.0) as usize;
                        (0..=m).all(|j| ok(lerp(a, b, j as f64 / m as f64)))
                    })
            };
            let run: f64 = pieces.iter().flat_map(|w| w.windows(2)).map(|s| dist(s[0], s[1])).sum();
            let most = (crate::core::round(run / 450.0) as usize).clamp(3, 8);
            out = bastions(&mut pieces, &mut towers, &gates, most, &keep, &clear);
        }
    }
    (pieces, towers, gate_towers, out)
}

/// A building's outline stands off every bastion: none of it inside one, and 4 ft and more from the
/// face of its walls (`STONE_FT` thick).
pub fn off_bastions(bastions: &[Vec<P>], poly: &[P]) -> bool {
    let reach = 0.5 * super::wallstyle::STONE_FT + 4.0;
    bastions.iter().all(|q| {
        let n = q.len();
        !poly.iter().any(|p| contains(q, *p))
            && !q.iter().any(|p| contains(poly, *p))
            && (0..n).all(|k| {
                let (a, b) = (q[k], q[(k + 1) % n]);
                let m = poly.len();
                (0..m).all(|j| seg_dist(poly[j], a, b) >= reach && seg_dist(a, poly[j], poly[(j + 1) % m]) >= reach && !crosses(a, b, poly[j], poly[(j + 1) % m]))
            })
    })
}
