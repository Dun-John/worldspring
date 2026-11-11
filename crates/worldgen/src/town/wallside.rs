//! Which way a wall faces (item 104 slice 2e): for each of a layout's wall pieces, the side of its
//! line (a to b along it) that is outside the town, or outside the castle it rings. The battlemap
//! puts it on each `Wall` vector (`[flags, side]`), so a palisade's stakes and a curtain's outer face
//! are drawn on the right side. Also the bastions as battlemap vectors (`ShapeKind::Bastion`) and the
//! props on their tops (`bastion_props`: Kinds 108-109 and crates, barrels).

use super::Layout;
use super::geom::*;

/// Per wall piece of `l.walls`: 1 where the outside is to the left of the way along it, -1 to its
/// right, 0 where it can't be told. A castle's curtain faces away from the castle's middle; the
/// town's wall away from the town's patches (by length, its stretches with a patch on one side only),
/// else away from the town's middle.
pub fn outward(l: &Layout) -> Vec<f32> {
    l.walls
        .iter()
        .map(|w| {
            if w.len() < 2 {
                return 0.0;
            }
            let mid = w[w.len() / 2];
            let castle = l.castles.iter().filter(|(c, r)| dist(*c, mid) <= r + 1.0).map(|x| x.0).next();
            let mut vote = 0.0;
            for s in w.windows(2) {
                let (d, m) = (sub(s[1], s[0]), lerp(s[0], s[1], 0.5));
                let k = len(d);
                if k < 1e-6 {
                    continue;
                }
                let n = [-d[1] / k, d[0] / k];
                let left_out = match castle {
                    Some(c) => Some(dot(n, sub(m, c)) > 0.0),
                    None => {
                        let inside = |p: P| l.districts.iter().any(|f| contains(f, p));
                        match (inside(add(m, mul(n, 6.0))), inside(add(m, mul(n, -6.0)))) {
                            (false, true) => Some(true),
                            (true, false) => Some(false),
                            _ => None,
                        }
                    }
                };
                vote += match left_out {
                    Some(true) => k,
                    Some(false) => -k,
                    None => 0.0,
                };
            }
            if vote == 0.0 && castle.is_none() {
                // (Away from the town's middle.)
                vote = w.windows(2).map(|s| cross(sub(s[1], s[0]), sub(lerp(s[0], s[1], 0.5), l.center))).sum::<f64>() * -1.0;
            }
            if vote > 0.0 {
                1.0
            } else if vote < 0.0 {
                -1.0
            } else {
                0.0
            }
        })
        .collect()
}

/// Each of a star-angled wall's bastions (`Layout::bastions`) as a `ShapeKind::Bastion` vector, by the
/// chunk that `owns` its middle: its six outline points (`local`, squares), then `[style, 0]`.
pub(crate) fn bastions(l: &Layout, owns: &dyn Fn(P) -> bool, local: &dyn Fn(&P) -> [f32; 2], shapes: &mut Vec<crate::battlemap::VectorShape>) {
    for q in l.bastions.iter().filter(|q| q.len() == 6) {
        let mid = centroid(q);
        if !owns(mid) {
            continue;
        }
        let ws = l.wall_style_at(mid);
        let mut pts: Vec<[f32; 2]> = q.iter().map(local).collect();
        pts.push([ws.code(super::wallstyle::TowerShape::Round) as f32, 0.0]);
        shapes.push(crate::battlemap::VectorShape { kind: crate::battlemap::ShapeKind::Bastion, size: (ws.thickness() / crate::battlemap::SQUARE_FT) as f32, pts });
    }
}

/// A bastion's props (the user, 2026-11-11): a ballista and a pile of its shot stones (ids 108-109,
/// the towns lane's range), with the crates and barrels already in the catalog.
pub(crate) const KINDS: [(crate::battlemap::Kind, crate::battlemap::KindInfo); 2] = {
    use crate::battlemap::{Kind, KindInfo};
    const fn k(id: Kind, name: &'static str, radius: f32, blocks_move: bool, cover: u8, difficult: bool, height_ft: f32) -> KindInfo {
        KindInfo { id: id as u16, name, radius, blocks_move, blocks_sight: false, cover, difficult, height_ft, hazard: None, feature: true }
    }
    [
        // (A light bolt thrower on a wheeled frame, about 7 ft long along its turn, aimed along +x.)
        (Kind::Ballista, k(Kind::Ballista, "ballista", 0.75, true, 1, false, 5.0)),
        // (A heap of dressed round stones, its own and a trebuchet's.)
        (Kind::ShotStones, k(Kind::ShotStones, "shot stones", 0.45, false, 1, true, 2.0)),
    ]
};

/// The ramp up onto a bastion's terreplein from the town: it starts where the town meets the
/// bastion's open back (the corner it replaced) and runs this far out along its middle line, this
/// wide (ft; `render/walls.ts` `drawBastion` draws the same).
pub const RAMP_FT: f64 = 10.0;
pub const RAMP_HALF: f64 = 5.0;

/// A bastion's frame (`Layout::bastions`: f1, g1, tip, g2, f2, the corner `v`): `v`, the way out to
/// its point (unit), and square to it.
fn frame(q: &[P]) -> (P, P, P) {
    let o = sub(q[2], q[5]);
    let o = mul(o, 1.0 / len(o).max(1e-9));
    (q[5], o, [-o[1], o[0]])
}

/// What a bastion holds on its terreplein (world ft, as `Layout::props`): a ballista as near its point
/// as it fits, aimed out over it, a pile of shot stones beside it, a crate or barrels in a flank's angle (one or
/// two). Each stands clear of the walls (half `thick`, their width, and a margin), the open back, the
/// ramp and its head, and the others (the ballista, the one that blocks a step, blocks only the
/// squares under its middle: every square of the top can still be walked to from the ramp, as
/// `towncheck walls` checks). A pure function of the outline.
pub fn bastion_props(q: &[P], thick: f64) -> Vec<super::Prop> {
    use crate::battlemap::{Kind, SQUARE_FT, info};
    let mut out: Vec<super::Prop> = Vec::new();
    if q.len() != 6 {
        return out;
    }
    let (v, o, s) = frame(q);
    let half = (0.5 * thick).max(2.1);
    let walls = [(q[0], q[1]), (q[1], q[2]), (q[2], q[3]), (q[3], q[4])];
    let back = [(q[4], q[5]), (q[5], q[0])];
    let reach = |kind: Kind, scale: f64| info(kind as u16).radius as f64 * scale * SQUARE_FT;
    // (The walls' squares on the battlemap: those whose middles lie within half their width and 1.5 ft
    // of their line, `battlemap` `raise`; the curtain on past the flanks' feet too. Nothing may reach
    // into one: `battlemap::on_wall`.)
    let curtain = [(q[0], add(q[0], mul(sub(q[0], q[5]), 20.0 / dist(q[0], q[5]).max(1e-9)))), (q[4], add(q[4], mul(sub(q[4], q[5]), 20.0 / dist(q[4], q[5]).max(1e-9))))];
    let on_wall = |p: P, r: f64| {
        let (i0, i1) = (crate::core::floor((p[0] - r) / SQUARE_FT) as i64, crate::core::floor((p[0] + r) / SQUARE_FT) as i64);
        let (j0, j1) = (crate::core::floor((p[1] - r) / SQUARE_FT) as i64, crate::core::floor((p[1] + r) / SQUARE_FT) as i64);
        (j0..=j1).any(|j| {
            (i0..=i1).any(|i| {
                let (x0, y0) = (i as f64 * SQUARE_FT, j as f64 * SQUARE_FT);
                let near = [p[0].clamp(x0, x0 + SQUARE_FT), p[1].clamp(y0, y0 + SQUARE_FT)];
                let c = [x0 + 0.5 * SQUARE_FT, y0 + 0.5 * SQUARE_FT];
                dist(near, p) < r && walls.iter().chain(&curtain).any(|&(a, b)| seg_dist(c, a, b) <= half + 1.5)
            })
        })
    };
    // (Whether a ballista at `p` shuts off any square of the top the town could otherwise walk to: the
    // squares it blocks are those whose middles lie within half its reach, a half square at least, as
    // play mode has it; walked square to square from the town's side of the open back.)
    let lo = [crate::core::floor((q.iter().map(|p| p[0]).fold(f64::MAX, f64::min) - 10.0) / SQUARE_FT) as i64, crate::core::floor((q.iter().map(|p| p[1]).fold(f64::MAX, f64::min) - 10.0) / SQUARE_FT) as i64];
    let hi = [crate::core::ceil((q.iter().map(|p| p[0]).fold(f64::MIN, f64::max) + 10.0) / SQUARE_FT) as i64, crate::core::ceil((q.iter().map(|p| p[1]).fold(f64::MIN, f64::max) + 10.0) / SQUARE_FT) as i64];
    let (nx, ny) = ((hi[0] - lo[0]) as usize, (hi[1] - lo[1]) as usize);
    let mid = |k: usize| [((lo[0] + (k % nx) as i64) as f64 + 0.5) * SQUARE_FT, ((lo[1] + (k / nx) as i64) as f64 + 0.5) * SQUARE_FT];
    let walled: Vec<bool> = (0..nx * ny).map(|k| walls.iter().chain(&curtain).any(|&(a, b)| seg_dist(mid(k), a, b) <= half + 1.5)).collect();
    let top: Vec<bool> = (0..nx * ny).map(|k| contains(q, mid(k))).collect();
    let walk = |blocked: &dyn Fn(usize) -> bool| {
        let mut seen: Vec<bool> = (0..nx * ny).map(|k| !top[k] && !blocked(k) && dot(sub(mid(k), v), o) < 0.0).collect();
        let mut todo: Vec<usize> = (0..nx * ny).filter(|&k| seen[k]).collect();
        while let Some(k) = todo.pop() {
            let (x, y) = (k % nx, k / nx);
            for n in [(x > 0).then(|| k - 1), (x + 1 < nx).then(|| k + 1), (y > 0).then(|| k - nx), (y + 1 < ny).then(|| k + nx)].into_iter().flatten() {
                if !seen[n] && !blocked(n) {
                    seen[n] = true;
                    todo.push(n);
                }
            }
        }
        seen
    };
    let free = walk(&|k| walled[k]);
    let cuts_off = |p: P, r: f64| {
        let under = (0.5 * r).max(0.5 * SQUARE_FT);
        let seen = walk(&|k| walled[k] || dist(mid(k), p) <= under);
        (0..nx * ny).any(|k| top[k] && free[k] && !seen[k] && dist(mid(k), p) > under)
    };
    // (Where it stands, how far it reaches, and the margin past the walls' faces.)
    let fits = |p: P, r: f64, margin: f64, laid: &[(P, f64)]| {
        let (ax, lat) = (dot(sub(p, v), o), dot(sub(p, v), s).abs());
        contains(q, p)
            && walls.iter().all(|&(a, b)| seg_dist(p, a, b) >= half + r + margin)
            && back.iter().all(|&(a, b)| seg_dist(p, a, b) >= r + 2.0)
            && !(ax < RAMP_FT + 1.5 + r && lat < RAMP_HALF + 1.5 + r)
            && laid.iter().all(|&(c, rc)| dist(p, c) >= r + rc + 2.5)
            && !on_wall(p, r)
    };
    let mut laid: Vec<(P, f64)> = Vec::new();
    let roll = crate::core::rng::hash2(0xba57_1015, crate::core::round(v[0] * 4.0) as i64, crate::core::round(v[1] * 4.0) as i64);
    let rot = libm::atan2(o[1], o[0]) as f32;
    // (Candidate spots: every foot of the top along and across its middle line, in a fixed order.)
    let depth = dist(q[2], v);
    let wide = q.iter().map(|p| dot(sub(*p, v), s).abs()).fold(0.0, f64::max);
    let spots: Vec<(f64, f64, P)> = (0..=crate::core::ceil(depth) as i64)
        .flat_map(|ax| (-(crate::core::ceil(wide) as i64)..=crate::core::ceil(wide) as i64).map(move |lt| (ax as f64, lt as f64)))
        .map(|(ax, lt)| (ax, lt, add(add(v, mul(o, ax)), mul(s, lt))))
        .filter(|t| contains(q, t.2))
        .collect();
    let best = |r: f64, margin: f64, laid: &[(P, f64)], score: &dyn Fn(f64, f64, P) -> f64| {
        spots.iter().filter(|t| fits(t.2, r, margin, laid)).map(|t| (score(t.0, t.1, t.2), t.2)).fold(None, |m: Option<(f64, P)>, c| if m.is_none_or(|m| c.0 > m.0) { Some(c) } else { m }).map(|m| m.1)
    };
    // The ballista: as near the point as it fits, on its middle line rather than off it, shutting off
    // no square of the top (the best few tried in turn).
    let rb = reach(Kind::Ballista, 1.0);
    let mut tries: Vec<(f64, P)> = spots.iter().filter(|t| fits(t.2, rb, 1.0, &laid)).map(|t| (t.0 - 0.7 * t.1.abs(), t.2)).collect();
    tries.sort_by(|x, y| y.0.total_cmp(&x.0));
    if let Some(p) = tries.iter().take(60).map(|t| t.1).find(|&p| !cuts_off(p, rb)) {
        out.push(super::Prop { at: p, rot, kind: Kind::Ballista, variant: (roll & 3) as u8, scale: 1.0 });
        laid.push((p, rb));
        // Its shot as near beside it as fits (the side by the roll).
        let rs = reach(Kind::ShotStones, 1.0);
        let side = if roll >> 8 & 1 == 0 { 1.0 } else { -1.0 };
        if let Some(c) = best(rs, 1.0, &laid, &|_, lt, c| -dist(c, p) + 0.3 * side * lt.signum()) {
            if dist(c, p) < rb + rs + 10.0 {
                out.push(super::Prop { at: c, rot: rot + 0.7, kind: Kind::ShotStones, variant: (roll >> 4 & 3) as u8, scale: 1.0 });
                laid.push((c, rs));
            }
        }
    }
    // A crate or barrels in each flank's angle (the second not always), as deep in it as they fit.
    let n = if roll >> 12 & 3 == 0 { 1 } else { 2 };
    let corners = if roll >> 9 & 1 == 0 { [q[1], q[3]] } else { [q[3], q[1]] };
    for (i, g) in corners.into_iter().take(n).enumerate() {
        let kind = if roll >> (14 + i) & 1 == 0 { Kind::Crate } else { Kind::Barrel };
        let r = reach(kind, 1.0);
        if let Some(p) = best(r, 0.5, &laid, &|_, _, c| -dist(c, g)) {
            if dist(p, g) < 0.5 * depth + 10.0 {
                let turn = (roll >> (16 + 4 * i) & 15) as f32 * 0.39;
                out.push(super::Prop { at: p, rot: turn, kind, variant: (roll >> (24 + 2 * i) & 3) as u8, scale: 1.0 });
                laid.push((p, r));
            }
        }
    }
    out
}

/// The looks of the plan's site lines `lines` from `looks.len()` on (`payload::pack_sites`): for each,
/// its points with `[x, y, a, b]`. A wall line (kind 0): a its style's code. A tower (kind 1 or 3, a
/// tower of `l`'s at its middle): a its style's code with its shape, b the way it faces (radians); a
/// gate tower its gatehouse's (`gatehouse::House::tower`: square, or a drum round). Anything else: 0, 0.
pub(crate) fn plan_looks(l: &Layout, lines: &[Vec<[f64; 4]>], looks: &mut Vec<Vec<[f64; 4]>>) {
    use super::wallstyle::TowerShape;
    let houses = if l.gate_towers.is_empty() && !l.extra_towers.iter().any(|e| e.1) { Vec::new() } else { super::gatehouse::houses(l) };
    for w in &lines[looks.len().min(lines.len())..] {
        let (a, b) = match (w.first(), w.last()) {
            (Some(p), Some(q)) => {
                let mid = [0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1])];
                match p[3] as u8 {
                    0 if w.len() >= 2 => (l.wall_style_at([w[w.len() / 2][0], w[w.len() / 2][1]]).code(TowerShape::Round) as f64, 0.0),
                    1 | 3 => {
                        let at = |t: &P| (t[0] - mid[0]).abs() + (t[1] - mid[1]).abs() < 0.1;
                        let t = l.towers.iter().chain(&l.gate_towers).chain(l.extra_towers.iter().map(|e| &e.0)).find(|t| at(t)).copied();
                        match t {
                            Some(t) => {
                                let gate = super::gatehouse::of_tower(&houses, t).map(|(h, second)| h.tower(second));
                                let (shape, u) = gate.map_or_else(|| l.tower_form(t), |g| (g.0, g.1));
                                (l.wall_style_at(t).code(shape) as f64, libm::atan2(u[1], u[0]))
                            }
                            None => (0.0, 0.0),
                        }
                    }
                    _ => (0.0, 0.0),
                }
            }
            _ => (0.0, 0.0),
        };
        looks.push(w.iter().map(|p| [p[0], p[1], a, b]).collect());
    }
}
