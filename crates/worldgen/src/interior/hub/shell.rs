//! The shell keep's programme: a ring of lean-tos round a round yard on a motte (`plan`), its floors
//! (`Plan::shell_level`).

use super::{Frame, Kind, Kinds, Laid, Lay, Piece, Plan, Ring};
use crate::core::rng::{Pcg32, hash3};
use crate::interior::{OPEN_BELOW, ROOF_SLOPE, Shell, building_item, shape};
use crate::town::{self, geom::{self, P}};

/// A shell keep's plan: its ring of lean-tos 3 squares deep (a gallery and rooms 2 deep above)
/// round its round yard, a gate passage through them toward the motte path's gate (the ring's first
/// corner), the main stair in the ring a square in from the wall with a landing from it to the yard.
pub(super) fn plan(b: &town::Building, sh: &Shell, piece: Vec<Piece>, seed: u64, grid_p: &dyn Fn(P) -> P) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let ring_pts: Vec<P> = b.parts[0].poly.iter().map(|p| grid_p(*p)).collect();
    let n = ring_pts.len();
    let centre = geom::mul(ring_pts.iter().fold([0.0, 0.0], |a, q| geom::add(a, *q)), 1.0 / n as f64);
    let r = (0..n).map(|k| geom::seg_dist(centre, ring_pts[k], ring_pts[(k + 1) % n])).fold(f64::MAX, f64::min);
    // (A ring 4.5 squares across its middle or more: a yard of 3 squares across in the smallest.)
    if r < 4.5 {
        return None;
    }
    let depth = 3.0;
    let gate = geom::sub(ring_pts[0], centre);
    let gate = geom::mul(gate, 1.0 / geom::len(gate).max(1e-9));
    let mid = |k: usize| [(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5];
    let inside = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && piece[j as usize * nx + i as usize] == Piece::Body;
    // The yard: round, the squares whose middles lie in the circle of a box about the middle (as a
    // round room is drawn), as wide as leaves the lean-tos their depth, all of it inside.
    let circle = |d: usize| -> Option<Vec<bool>> {
        let x0 = crate::core::round(centre[0] - 0.5 * d as f64) as isize;
        let y0 = crate::core::round(centre[1] - 0.5 * d as f64) as isize;
        if x0 < 0 || y0 < 0 || x0 as usize + d > nx || y0 as usize + d > ny {
            return None;
        }
        let cells = shape::shape_cells([x0 as usize, y0 as usize, d, d], shape::ROUND);
        let mut out = vec![false; nx * ny];
        for q in (0..d * d).filter(|&q| cells[q]) {
            let (i, j) = (x0 + (q % d) as isize, y0 + (q / d) as isize);
            if !inside(i, j) {
                return None;
            }
            out[j as usize * nx + i as usize] = true;
        }
        Some(out)
    };
    let d0 = crate::core::floor(2.0 * (r - depth)) as usize;
    let yard = (3..=d0).rev().find_map(circle)?;
    let ringed = |k: usize| piece[k] == Piece::Body && !yard[k];
    let near8 = |k: usize, f: &dyn Fn(isize, isize) -> bool| {
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        (-1..=1).any(|dj| (-1..=1).any(|di| (di, dj) != (0, 0) && f(i + di, j + dj)))
    };
    let outer: Vec<bool> = (0..nx * ny).map(|k| ringed(k) && near8(k, &|i, j| !inside(i, j))).collect();
    let inner: Vec<bool> = (0..nx * ny).map(|k| ringed(k) && near8(k, &|i, j| inside(i, j) && yard[j as usize * nx + i as usize])).collect();
    let gate_sq: Vec<bool> = (0..nx * ny)
        .map(|k| {
            let d = geom::sub(mid(k), centre);
            ringed(k) && geom::cross(gate, d).abs() < 0.75 && geom::dot(gate, d) > 0.0
        })
        .collect();
    if !gate_sq.iter().any(|&g| g) {
        return None;
    }
    let turn: Vec<f64> = (0..nx * ny)
        .map(|k| {
            let d = geom::sub(mid(k), centre);
            let a = libm::atan2(geom::cross(gate, d), geom::dot(gate, d));
            let a = if a < 0.0 { a + std::f64::consts::TAU } else { a };
            a / std::f64::consts::TAU
        })
        .collect();
    // The front door: out of the gate passage's squares on the wall, toward the gate.
    let mut front = Vec::new();
    for k in (0..nx * ny).filter(|&k| gate_sq[k] && outer[k]) {
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        for (side, (di, dj)) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            if !inside(i + di, j + dj) {
                front.push((side, k % nx, k / nx));
            }
        }
    }
    let dir = |e: &(usize, usize, usize)| {
        let (di, dj) = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)][e.0];
        -(gate[0] * di + gate[1] * dj)
    };
    front.sort_by(|a, b| dir(a).total_cmp(&dir(b)).then(a.cmp(b)));
    // The stair: two squares side by side in the ring, a square in from the wall (beside it), off
    // the yard's edge where the ring is deep enough and off the gate passage; a quarter round from
    // the gate.
    let ok = |k: usize| ringed(k) && !outer[k] && !gate_sq[k] && sh.whole[k] && !inner[k];
    let by_wall = |k: usize| {
        let (i, j) = (k % nx, k / nx);
        [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].iter().any(|&(a, b)| a < nx && b < ny && outer[b * nx + a])
    };
    let mut best: Option<(f64, [usize; 4])> = None;
    for k in (0..nx * ny).filter(|&k| ok(k)) {
        let (i, j) = (k % nx, k / nx);
        for (w, h) in [(2usize, 1usize), (1, 2)] {
            if i + w > nx || j + h > ny {
                continue;
            }
            let q = (j + h - 1) * nx + i + w - 1;
            if !ok(q) || !(by_wall(k) && by_wall(q)) {
                continue;
            }
            let score = (turn[k] - 0.25).abs() + 1e-3 * k as f64;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, [i, j, w, h]));
            }
        }
    }
    let (_, stairs) = best?;
    // Its landing: the stairs and the squares on in from them to the yard.
    let to_yard = |q: usize| {
        let (i, j) = ((q % nx) as isize, (q / nx) as isize);
        [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)].iter().any(|&(di, dj)| inside(i + di, j + dj) && yard[(j + dj) as usize * nx + (i + di) as usize])
    };
    let mut landing = vec![false; nx * ny];
    for j in stairs[1]..stairs[1] + stairs[3] {
        for i in stairs[0]..stairs[0] + stairs[2] {
            let mut k = j * nx + i;
            landing[k] = true;
            for _ in 0..2 {
                if to_yard(k) {
                    break;
                }
                let (a, b) = ((k % nx) as isize, (k / nx) as isize);
                let next = [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .filter(|&&(di, dj)| inside(a + di, b + dj))
                    .map(|&(di, dj)| (b + dj) as usize * nx + (a + di) as usize)
                    .filter(|&q| ringed(q) && !gate_sq[q])
                    .min_by(|&p, &q| geom::dist(mid(p), centre).total_cmp(&geom::dist(mid(q), centre)).then(p.cmp(&q)));
                let Some(q) = next.filter(|&q| geom::dist(mid(q), centre) < geom::dist(mid(k), centre)) else { break };
                landing[q] = true;
                k = q;
            }
        }
    }
    let f = Frame { o: (0, 0), ds: (1, 0), dt: (0, 1), len: nx, wid: ny };
    Some(Plan {
        kind: Kind::ShellKeep,
        nx,
        ny,
        f,
        piece,
        service: 0,
        screens: false,
        dz: 0,
        hall_t: (0, 0),
        land_t: (0, 0),
        stairs,
        hall_z: 0,
        open: false,
        floors: 2,
        wing_floors: 1,
        fore_floors: 1,
        pieces: true,
        top: Vec::new(),
        wing_tops: Vec::new(),
        wings: Vec::new(),
        fore: Vec::new(),
        tower_top: Vec::new(),
        tower_sq: Vec::new(),
        overlap: Vec::new(),
        ring: Some(Ring { yard, gate: gate_sq, outer, inner, landing, turn, centre, front }),
        seed,
        pending: Vec::new(),
        chapel: false,
        ends: [true, true],
        side: -1,
        oratory: false,
        last: Vec::new(),
        bay: None,
        small: false,
        pal: None,
        rich: false,
    })
}

impl Plan {
    /// A shell keep's floor: lean-tos round the round yard (a gate passage through them, a well in
    /// the yard, the stairs' landing) on the ground; rooms round a gallery over the yard above (the
    /// lean-tos' inner row; the stair's landing off it): the great chamber and solar, a treasury,
    /// an armory, a garrison; the wall walk round the lean-tos' roof on top.
    pub(super) fn shell_level(&mut self, z: i8, mut lay: Lay) -> Option<Laid> {
        let ring = self.ring.clone()?;
        let (nx, ny) = (self.nx, self.ny);
        let in_ring = |k: usize| self.piece[k] == Piece::Body && !ring.yard[k];
        let st = self.stairs;
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        // Sectors round from the gate (turned by `off`), by weight.
        let sectors = |lay: &mut Lay, kinds: &[(&'static str, f64)], off: f64| {
            let total: f64 = kinds.iter().map(|k| k.1).sum();
            let ids: Vec<i16> = kinds.iter().map(|k| lay.room(k.0)).collect();
            for k in 0..nx * ny {
                if !in_ring(k) || lay.cells[k] >= 0 {
                    continue;
                }
                let turn = ring.turn[k] - off - crate::core::floor(ring.turn[k] - off);
                let mut acc = 0.0;
                let mut at = kinds.len() - 1;
                for (n, kd) in kinds.iter().enumerate() {
                    acc += kd.1 / total;
                    if turn < acc {
                        at = n;
                        break;
                    }
                }
                lay.cells[k] = ids[at];
            }
            ids
        };
        let name: &'static str;
        let mut roof = false;
        let (mut free, mut kinds): (Vec<i16>, Kinds) = (Vec::new(), Vec::new());
        let open = |lay: &mut Lay| {
            let void = lay.room(OPEN_BELOW);
            for k in (0..nx * ny).filter(|&k| ring.yard[k]) {
                lay.cells[k] = void;
            }
            void
        };
        let round: i16;
        match z {
            0 => {
                let yard = lay.room("keep yard");
                let gate = lay.room("gate passage");
                let landing = lay.room("landing");
                for k in 0..nx * ny {
                    if self.piece[k] == Piece::Body && ring.yard[k] {
                        lay.cells[k] = yard;
                    } else if ring.landing[k] {
                        lay.cells[k] = landing;
                    } else if ring.gate[k] {
                        lay.cells[k] = gate;
                    }
                }
                sectors(&mut lay, &[("storeroom", 1.0), ("kitchen", 1.4), ("great hall", 3.0), ("chapel", 1.2), ("chamber", 1.2), ("garrison", 1.4)], 0.0);
                self.chapel = true;
                lay.hints.front = Some((gate, ring.front.clone()));
                // The well in the middle of the yard.
                let c = ring.centre;
                if let Some(k) = (0..nx * ny).filter(|&k| lay.cells[k] == yard).min_by(|&a, &b| {
                    let d = |k: usize| geom::dist([(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5], c);
                    d(a).total_cmp(&d(b)).then(a.cmp(&b))
                }) && let Some(well) = building_item("well", k % nx, k / nx, 1, 1)
                {
                    lay.items.push(well);
                }
                round = yard;
                name = "Yard";
            }
            1 => {
                round = open(&mut lay);
                let gallery = lay.room("corridor");
                let landing = lay.room("landing");
                // (The stair's landing out to the wall beside it.)
                let by_stair = |k: usize| ring.outer[k] && [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx].iter().any(|&q| q < nx * ny && on_stairs(q));
                for k in 0..nx * ny {
                    if lay.cells[k] < 0 && self.piece[k] == Piece::Body && (ring.inner[k] || ring.landing[k] || by_stair(k)) {
                        lay.cells[k] = if ring.inner[k] { gallery } else { landing };
                    }
                }
                // (Of about 8 squares each, given their kinds once those too small are made part of
                // the room beside them: the programme's first, the biggest to the biggest.)
                let n = (0..nx * ny).filter(|&k| in_ring(k) && lay.cells[k] < 0).count();
                let m = (crate::core::round(n as f64 / 8.0) as usize).clamp(6, 14);
                let mut rng = Pcg32::new(hash3(self.seed, z as i64, 0x5b, 0), 87);
                let weights: Vec<(&'static str, f64)> = (0..m).map(|_| ("chamber", 0.6 + 0.8 * rng.next_f64())).collect();
                free = sectors(&mut lay, &weights, 0.0);
                kinds = vec![
                    ("great chamber", 1.6),
                    ("solar", 1.4),
                    ("garrison", 1.3),
                    ("armory", 1.1),
                    ("treasury", 0.6),
                    ("wardrobe", 0.5),
                    ("chamber", 1.0),
                    ("guest chamber", 0.9),
                    ("storeroom", 0.8),
                    ("study", 0.7),
                ];
                name = "Upper floor";
            }
            _ => {
                round = open(&mut lay);
                let walk = lay.room("wall walk");
                let slope = lay.room(ROOF_SLOPE);
                for k in 0..nx * ny {
                    if self.piece[k] != Piece::Body || lay.cells[k] >= 0 {
                        continue;
                    }
                    lay.cells[k] = if ring.outer[k] || on_stairs(k) { walk } else { slope };
                }
                roof = true;
                name = "Wall walk";
            }
        }
        // (The yard, and what is open over it, drawn round.)
        if shape::fits(&lay.cells, nx, round, shape::ROUND) {
            lay.rooms[round as usize].shape = shape::ROUND;
        }
        self.absorb(&mut lay, &mut free, 4);
        // (A lean-to longer than 2.5 times its width: halved round the ring, where both halves hold
        // 4 squares in a 2 x 2 block or more.)
        for _ in 0..free.len() {
            let Some(half) = free.iter().find_map(|&r| {
                let sq: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == r).collect();
                let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
                let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
                let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
                if 2 * w.max(h) <= 5 * w.min(h) {
                    return None;
                }
                let mut turns: Vec<f64> = sq.iter().map(|&k| ring.turn[k]).collect();
                turns.sort_by(f64::total_cmp);
                // (Round from the gate: a room across it is turned half round first.)
                let wraps = turns.last().unwrap_or(&0.0) - turns.first().unwrap_or(&0.0) > 0.5;
                let at = |k: usize| if wraps { (ring.turn[k] + 0.5) % 1.0 } else { ring.turn[k] };
                let mut by: Vec<f64> = sq.iter().map(|&k| at(k)).collect();
                by.sort_by(f64::total_cmp);
                let mid = by[by.len() / 2];
                let half: Vec<usize> = sq.iter().copied().filter(|&k| at(k) >= mid).collect();
                let solid = |part: &[usize]| part.len() >= 4 && part.iter().any(|&k| [k + 1, k + nx, k + nx + 1].iter().all(|q| part.contains(q)) && k % nx + 1 < nx);
                let rest: Vec<usize> = sq.iter().copied().filter(|k| !half.contains(k)).collect();
                (solid(&half) && solid(&rest)).then_some(half)
            }) else { break };
            let id = lay.room("chamber");
            for k in half {
                lay.cells[k] = id;
            }
            free.push(id);
        }
        Self::assign(&mut lay, &free, kinds, &["chamber", "storeroom", "guest chamber"]);
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name, roof, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls: Vec::new() })
    }
}
