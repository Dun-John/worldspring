//! Which way a wall faces (item 104 slice 2e): for each of a layout's wall pieces, the side of its
//! line (a to b along it) that is outside the town, or outside the castle it rings. The battlemap
//! puts it on each `Wall` vector (`[flags, side]`), so a palisade's stakes and a curtain's outer face
//! are drawn on the right side. Also the bastions as battlemap vectors (`ShapeKind::Bastion`).

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
