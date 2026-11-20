//! A building's shell: its outline on the grid, parts, neighbours and insets.

use super::*;

/// A building's grid: along its footprint's long axis, the squares inside it, and which squares
/// just outside are open ground (not inside a neighbouring building): a door must open onto one.
pub struct Shell<'a> {
    pub nx: usize,
    pub ny: usize,
    /// World ft of grid corner (0, 0); grid x runs along `u`, y along `v`.
    pub origin: P,
    pub u: P,
    pub v: P,
    pub inside: Vec<bool>,
    /// The outer wall as drawn (`Interior::outlines`), grid units; empty when the footprint is
    /// the grid's rectangle (or not convex: drawn on the squares).
    pub outline: Vec<P>,
    /// Squares the outer wall as drawn doesn't cut: inside its inner face (all of them without an
    /// outline). Stairs stand on these.
    pub whole: Vec<bool>,
    /// A keep's outer wall as drawn, one convex part per piece of it (`Building::parts`), grid
    /// units; then `outline` is empty.
    pub outline_parts: Vec<Vec<P>>,
    /// A keep's towers: the squares of each one's room (filling the tower: round about its middle,
    /// or square, its wall as thin as any outer wall), whether round, and the square in its middle
    /// (its spiral stair).
    pub towers: Vec<(Vec<usize>, bool, usize)>,
    /// Each tower's ways in (`towers`' order): the squares of the building's under its wall beside
    /// its room, the one way in on each floor kept by the room it opens onto (`layout_level`).
    pub tower_doors: Vec<Vec<usize>>,
    /// A keep's piece each square is drawn inside (index into `Building::parts`; `usize::MAX` none);
    /// empty for every other building.
    pub owner: Vec<usize>,
    neighbours: Vec<&'a Vec<P>>,
    /// A keep's footprint (world ft): the squares in it no room has are its walls, not open ground.
    own: Vec<P>,
    /// A temple's, a shrine's or a monastery's: the battlemap's squares round it (`town::temple::REACH_PAD`
    /// beyond it each way) whose open ground reaches a street (`town::temple::street_reach`), else the box's
    /// edge (`reach_of`); None for another building, or one no open square beside it reaches either from.
    reach: Option<town::temple::Reach>,
    /// A coaching inn's (`town::palace::is_coaching`: its lots merged round a yard).
    pub coaching: bool,
}


/// Half the outer wall's width as drawn (squares): a square is whole when it lies this far inside
/// the outline.
pub const WALL_HALF: f64 = 0.225;

impl<'a> Shell<'a> {
    pub fn of(l: &'a Layout, b: &town::Building) -> Shell<'a> {
        if !b.parts.is_empty() {
            return Shell::of_parts(l, b);
        }
        let o = geom::obb(&b.poly);
        let (u, v) = (o.axis, [-o.axis[1], o.axis[0]]);
        let (a0, a1) = geom::extent(&b.poly, u);
        let (b0, b1) = geom::extent(&b.poly, v);
        let nx = (crate::core::round((a1 - a0) / SQUARE_FT) as usize).max(2);
        let ny = (crate::core::round((b1 - b0) / SQUARE_FT) as usize).max(2);
        let (u0, v0) = (0.5 * (a0 + a1) - 0.5 * nx as f64 * SQUARE_FT, 0.5 * (b0 + b1) - 0.5 * ny as f64 * SQUARE_FT);
        let origin = add(mul(u, u0), mul(v, v0));
        let world_at = |x: f64, y: f64| add(origin, add(mul(u, x * SQUARE_FT), mul(v, y * SQUARE_FT)));
        // Squares inside the footprint (a tolerance keeps rectangle edges whole).
        let grown = grow(&b.poly, 1.5);
        let mut inside: Vec<bool> = (0..nx * ny).map(|k| geom::contains(&grown, world_at((k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5))).collect();
        // A round tower's floors are round: its squares those of a circle in the box round them,
        // so its rooms keep their round shape (`shape::fits`); a yurt's floor too.
        if matches!(arch_of(b), Arch::Tower | Arch::House) && round_ish(b) {
            let all: Vec<i16> = inside.iter().map(|&i| if i { 0 } else { -1 }).collect();
            if let Some(bx) = shape::room_box(&all, nx, 0) {
                let round = shape::shape_cells(bx, shape::ROUND);
                for (k, c) in round.iter().enumerate() {
                    let q = (bx[1] + k / bx[2]) * nx + bx[0] + k % bx[2];
                    inside[q] &= *c;
                }
            }
        }
        let neighbours = neighbours_of(l, b);
        // (A yurt's ring is its footprint as built: its lattice stands on it, inside no wider.)
        let outline = outline_of(&b.poly, &inside, nx, origin, u, v, arch_of(b) == Arch::House && round_ish(b));
        let mut sh = Shell { nx, ny, origin, u, v, inside, outline, whole: Vec::new(), outline_parts: Vec::new(), towers: Vec::new(), tower_doors: Vec::new(), owner: Vec::new(), neighbours, own: Vec::new(), reach: None, coaching: town::palace::is_coaching(l, b) };
        sh.whole = sh.whole_by(WALL_HALF);
        sh.reach = sh.reach_of(l, b);
        sh
    }

    /// A keep's grid (`Building::parts`): along its body, its squares whole squares from the
    /// body's first corner (the pieces are laid on them: `town::castle`), reaching over every
    /// piece. Inside: the squares whose middle lies in the body, a wing or a forebuilding, and each
    /// tower's room, filling its box (round in a round tower: the squares whose middle its circle
    /// holds), so its wall is as thin as any outer wall; any square of the building left under a
    /// tower beside its room is a way in (`tower_doors`, one kept on each floor), else wall. Its
    /// outer wall as drawn is a part per piece, each edge pushed out as far as the squares drawn
    /// inside that piece need (`part_outline`).
    fn of_parts(l: &'a Layout, b: &town::Building) -> Shell<'a> {
        let body = &b.parts[0].poly;
        let o = geom::obb(body);
        let (u, v) = (o.axis, [-o.axis[1], o.axis[0]]);
        let (a0, a1) = geom::extent(&b.poly, u);
        let (b0, b1) = geom::extent(&b.poly, v);
        let (ru, rv) = (dot(body[0], u), dot(body[0], v));
        let from = |r: f64, lo: f64| r - crate::core::ceil((r - lo) / SQUARE_FT - 1e-6) * SQUARE_FT;
        let (u0, v0) = (from(ru, a0), from(rv, b0));
        let nx = (crate::core::ceil((a1 - u0) / SQUARE_FT - 1e-6) as usize).max(2);
        let ny = (crate::core::ceil((b1 - v0) / SQUARE_FT - 1e-6) as usize).max(2);
        let origin = add(mul(u, u0), mul(v, v0));
        let grid = |p: P| {
            let d = sub(p, origin);
            [dot(d, u) / SQUARE_FT, dot(d, v) / SQUARE_FT]
        };
        // Each piece in grid units, wound anticlockwise in grid axes.
        let parts: Vec<Vec<P>> = b
            .parts
            .iter()
            .map(|p| {
                let mut g: Vec<P> = p.poly.iter().map(|q| grid(*q)).collect();
                if geom::area(&g) < 0.0 {
                    g.reverse();
                }
                g
            })
            .collect();
        let middle = |k: usize| [(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5];
        let mut inside = vec![false; nx * ny];
        // The piece each square is drawn inside (the one it lies deepest in; a tower's room its
        // tower).
        let mut owner = vec![usize::MAX; nx * ny];
        for k in 0..nx * ny {
            let best = (0..parts.len())
                .filter(|&pi| !matches!(b.parts[pi].kind, town::PartKind::Tower { .. }))
                .map(|pi| (pi, inset_by(&parts[pi], middle(k))))
                .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)));
            if let Some((pi, _)) = best.filter(|x| x.1 > 0.0) {
                inside[k] = true;
                owner[k] = pi;
            }
        }
        let mut towers = Vec::new();
        let mut tower_doors = Vec::new();
        let in_tower = |k: usize| b.parts.iter().enumerate().any(|(pi, p)| matches!(p.kind, town::PartKind::Tower { .. }) && geom::contains(&parts[pi], middle(k)));
        for (pi, part) in b.parts.iter().enumerate() {
            let town::PartKind::Tower { round } = part.kind else { continue };
            let g = &parts[pi];
            let c = mul(g.iter().fold([0.0, 0.0], |a, q| add(a, *q)), 1.0 / g.len() as f64);
            let (x0, x1) = g.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q[0]), a.1.max(q[0])));
            let d = crate::core::round(x1 - x0) as i64;
            let (bx, by) = (crate::core::round(c[0] - 0.5 * d as f64) as i64, crate::core::round(c[1] - 0.5 * d as f64) as i64);
            let w = d.max(1) as usize;
            if bx < 0 || by < 0 || bx as usize + w > nx || by as usize + w > ny {
                continue;
            }
            let rb = [bx as usize, by as usize, w, w];
            let cells = if round { shape::shape_cells(rb, shape::ROUND) } else { vec![true; w * w] };
            let squares: Vec<usize> = (0..w * w).filter(|&k| cells[k]).map(|k| (rb[1] + k / w) * nx + rb[0] + k % w).collect();
            for &k in &squares {
                inside[k] = true;
                owner[k] = pi;
            }
            // Its wall: the building's squares under it, but those beside its room and beside the
            // building outside every tower (its ways in, one kept on each floor).
            let side = |k: usize| {
                let (i, j) = (k % nx, k / nx);
                [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().filter(|&(a, b)| a < nx && b < ny).map(move |(a, b)| b * nx + a)
            };
            let mut doors = Vec::new();
            for k in 0..nx * ny {
                if !inside[k] || squares.contains(&k) || !geom::contains(&parts[pi], middle(k)) {
                    continue;
                }
                if side(k).any(|q| squares.contains(&q)) && side(k).any(|q| inside[q] && !in_tower(q)) {
                    doors.push(k);
                } else {
                    inside[k] = false;
                }
            }
            // Its spiral stair: the square nearest its middle that no door can open beside (none of
            // its sides on a square of the building outside the tower).
            let mid = [rb[0] as f64 + 0.5 * w as f64, rb[1] as f64 + 0.5 * w as f64];
            let inner = |k: usize| {
                let (i, j) = (k % nx, k / nx);
                [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].iter().all(|&(a, b)| a >= nx || b >= ny || !inside[b * nx + a] || squares.contains(&(b * nx + a)))
            };
            let near = |k: usize| geom::dist(middle(k), mid);
            let best = |all: bool| squares.iter().copied().filter(|&k| all || inner(k)).min_by(|&a, &b| near(a).total_cmp(&near(b)).then(a.cmp(&b)));
            let Some(spiral) = best(false).or_else(|| best(true)) else { continue };
            towers.push((squares, round, spiral));
            tower_doors.push(doors);
        }
        let outline_parts: Vec<Vec<P>> = (0..parts.len())
            .filter_map(|pi| {
                let own: Vec<P> = (0..nx * ny).filter(|&k| inside[k] && owner[k] == pi).map(middle).collect();
                let reach: Vec<usize> = (0..nx * ny).filter(|&k| geom::contains(&parts[pi], middle(k))).collect();
                part_outline(&parts[pi], &own, &reach, nx)
            })
            .collect();
        let mut sh = Shell { nx, ny, origin, u, v, inside, outline: Vec::new(), whole: Vec::new(), outline_parts, towers, tower_doors, owner, neighbours: neighbours_of(l, b), own: b.poly.clone(), reach: None, coaching: false };
        sh.whole = sh.whole_by(WALL_HALF);
        sh.reach = sh.reach_of(l, b);
        sh
    }

    /// How far grid point `c` lies inside the outer wall as drawn (its parts' best; f64::MAX
    /// without one).
    fn inset(&self, c: P) -> f64 {
        if !self.outline.is_empty() {
            return inset_by(&self.outline, c);
        }
        self.outline_parts.iter().map(|o| inset_by(o, c)).fold(if self.outline_parts.is_empty() { f64::MAX } else { f64::MIN }, f64::max)
    }

    /// Squares inside, lying at least `t` (squares) inside the outer wall as drawn.
    pub fn whole_by(&self, t: f64) -> Vec<bool> {
        let walled = !self.outline.is_empty() || !self.outline_parts.is_empty();
        (0..self.nx * self.ny)
            .map(|k| {
                let (i, j) = ((k % self.nx) as f64, (k / self.nx) as f64);
                self.inside[k] && (!walled || [[i, j], [i + 1.0, j], [i + 1.0, j + 1.0], [i, j + 1.0]].iter().all(|c| self.inset(*c) >= t))
            })
            .collect()
    }

    /// The outer wall as drawn (`Interior::outlines`): its convex parts, whose union is the
    /// footprint (one, a keep's one per piece, or none for the grid's rectangle), grid units, wound
    /// clockwise on screen.
    pub fn outlines(&self) -> Vec<Vec<[f32; 2]>> {
        if !self.outline_parts.is_empty() {
            return self.outline_parts.iter().map(|o| o.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()).collect();
        }
        if self.outline.is_empty() {
            return Vec::new();
        }
        vec![self.outline.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()]
    }

    /// (Roadmap 7, 164) A U's or a court's yard (`town::wings`): the squares of its ranges' box
    /// outside its footprint (row-major, nx × ny) and the box's corners (grid units, as
    /// `Interior::yard`); None for any other building.
    pub fn yard(&self, b: &town::Building) -> Option<(Vec<bool>, Vec<[f32; 2]>)> {
        let arms = town::wings::ranges(b).filter(|a| a.len() >= 3)?;
        let at = |p: P, d: P| geom::dot(sub(p, self.origin), d) / SQUARE_FT;
        let span = |d: P| arms.iter().flatten().map(|p| at(*p, d)).fold((f64::MAX, f64::MIN), |(a, c), x| (a.min(x), c.max(x)));
        let ((x0, x1), (y0, y1)) = (span(self.u), span(self.v));
        let yard: Vec<bool> = (0..self.nx * self.ny)
            .map(|k| {
                let (x, y) = ((k % self.nx) as f64 + 0.5, (k / self.nx) as f64 + 0.5);
                !self.inside[k] && x > x0 && x < x1 && y > y0 && y < y1
            })
            .collect();
        yard.contains(&true).then(|| (yard, [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].iter().map(|p| [p[0] as f32, p[1] as f32]).collect()))
    }

    /// World position (ft) of a grid point.
    pub fn world_at(&self, x: f64, y: f64) -> P {
        add(self.origin, add(mul(self.u, x * SQUARE_FT), mul(self.v, y * SQUARE_FT)))
    }

    /// Square (i, j) (maybe off the grid) is open ground: inside no neighbouring building (nor, for
    /// a keep, in its own walls).
    /// A temple's, a shrine's or a monastery's: ground that reaches a street (`reach`) where any
    /// beside it does.
    pub fn ext_free(&self, i: isize, j: isize) -> bool {
        self.open_ground(i, j) && self.reach.as_ref().is_none_or(|r| r.at(self.world_at(i as f64 + 0.5, j as f64 + 0.5)))
    }

    /// Square (i, j) is inside no neighbouring building (nor, for a keep, in its own walls): open
    /// ground, whether or not it reaches a street.
    pub fn open_ground(&self, i: isize, j: isize) -> bool {
        let p = self.world_at(i as f64 + 0.5, j as f64 + 0.5);
        !self.neighbours.iter().any(|poly| geom::contains(poly, p)) && (self.own.is_empty() || !geom::contains(&self.own, p))
    }

    /// `reach` for building `b` of layout `l`: a temple's, a shrine's or a monastery's, where an open
    /// square beside it reaches a street; else (no street near: a hamlet's shrine), where one reaches
    /// the open beyond the houses round it (`Reach::to_edge`), so that its door never opens onto a
    /// battlemap square of a neighbour's.
    fn reach_of(&self, l: &Layout, b: &town::Building) -> Option<town::temple::Reach> {
        if !town::temple::is_faith(b) || b.structure != Structure::Roofed {
            return None;
        }
        let (nx, ny) = (self.nx, self.ny);
        let r = town::temple::street_reach(l, b.id, &b.poly, &b.poly, town::temple::REACH_PAD);
        let is_in = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && self.inside[j as usize * nx + i as usize];
        let beside = |r: &town::temple::Reach| {
            (0..nx * ny).filter(|&k| self.inside[k]).any(|k| {
                let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| !is_in(i + di, j + dj) && self.open_ground(i + di, j + dj) && r.at(self.world_at((i + di) as f64 + 0.5, (j + dj) as f64 + 0.5)))
            })
        };
        if beside(&r) {
            return Some(r);
        }
        let r = r.to_edge();
        beside(&r).then_some(r)
    }
}

/// The roofed buildings near `b` (a door must not open into one), and the ground of another's lots
/// (`Layout::grounds`: a temple's or a monastery's own) but one `b` stands on.
fn neighbours_of<'a>(l: &'a Layout, b: &town::Building) -> Vec<&'a Vec<P>> {
    let c0 = geom::centroid(&b.poly);
    let reach = b.poly.iter().map(|p| geom::dist(*p, c0)).fold(0.0, f64::max) + 40.0;
    let near = |poly: &Vec<P>| poly.iter().any(|p| geom::dist(*p, c0) < reach + 60.0);
    let roofed = |id: u32| l.building(id as usize).is_some_and(|o| o.structure == Structure::Roofed);
    l.buildings
        .iter()
        .filter(|o| o.id != b.id && o.structure == Structure::Roofed && near(&o.poly))
        .map(|o| &o.poly)
        .chain(l.grounds.iter().filter(|g| g.0 != b.id && near(&g.1) && roofed(g.0) && !town::on_lot(&g.1, &b.poly)).map(|g| &g.1))
        .collect()
}

/// A piece of a keep's outer wall as drawn (`pts`: convex, grid units, anticlockwise in grid axes):
/// each edge pushed out as far as the squares drawn inside this piece (`own`: their middles) need
/// to lie `WALL_HALF` + 0.1 in from it, cut back to within half a square of the squares whose middle
/// it holds (`reach`), as `outline_of` does for a whole footprint. None for a piece holding none.
fn part_outline(pts: &[P], own: &[P], reach: &[usize], nx: usize) -> Option<Vec<P>> {
    if reach.is_empty() {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
    }
    let mut out: Vec<P> = vec![[x0 - 5.0, y0 - 5.0], [x1 + 5.0, y0 - 5.0], [x1 + 5.0, y1 + 5.0], [x0 - 5.0, y1 + 5.0]];
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        let e = sub(b, a);
        let l = geom::len(e);
        if l < 1e-9 {
            continue;
        }
        let o = [e[1] / l, -e[0] / l];
        let least = own.iter().map(|c| geom::cross(e, sub(*c, a)) / l).fold(f64::MAX, f64::min);
        let push = (WALL_HALF + 0.1 - least).max(0.0);
        out = geom::clip_plain(&out, o, dot(a, o) + push);
    }
    let mut corners: Vec<P> = Vec::new();
    for &k in reach {
        let (i, j) = ((k % nx) as f64, (k / nx) as f64);
        corners.extend([[i - 0.5, j - 0.5], [i + 1.5, j - 0.5], [i + 1.5, j + 1.5], [i - 0.5, j + 1.5]]);
    }
    let hull = geom::hull(corners);
    for k in 0..hull.len() {
        let (a, b) = (hull[k], hull[(k + 1) % hull.len()]);
        let e = sub(b, a);
        out = geom::clip_plain(&out, [e[1], -e[0]], dot(a, [e[1], -e[0]]));
    }
    let mut pts: Vec<P> = Vec::with_capacity(out.len());
    for p in out {
        if pts.last().is_none_or(|l| (l[0] - p[0]).abs() + (l[1] - p[1]).abs() > 1e-4) {
            pts.push(p);
        }
    }
    while pts.len() > 1 && (pts[0][0] - pts[pts.len() - 1][0]).abs() + (pts[0][1] - pts[pts.len() - 1][1]).abs() <= 1e-4 {
        pts.pop();
    }
    (pts.len() >= 3).then_some(pts)
}

/// A footprint's outer wall as drawn (grid units, the wall's middle on the footprint, each edge
/// pushed out as far as the squares inside need to lie `WALL_HALF` + 0.1 in from it), for one that
/// isn't the grid's rectangle: the footprint when it is convex (else none: drawn on the squares),
/// cut back to within half a square of the squares inside it (an acute corner holds none), wound
/// anticlockwise in grid axes (clockwise on screen), points the cuts left together merged.
/// `exact`: the footprint as it is, not pushed out nor cut (a yurt's ring).
fn outline_of(poly: &[P], inside: &[bool], nx: usize, origin: P, u: P, v: P, exact: bool) -> Vec<P> {
    if inside.iter().all(|&i| i) || !inside.iter().any(|&i| i) {
        return Vec::new();
    }
    let mut pts: Vec<P> = poly
        .iter()
        .map(|p| {
            let d = sub(*p, origin);
            [dot(d, u) / SQUARE_FT, dot(d, v) / SQUARE_FT]
        })
        .collect();
    let n = pts.len();
    let turns: Vec<f64> = (0..n).map(|k| geom::cross(sub(pts[(k + 1) % n], pts[k]), sub(pts[(k + 2) % n], pts[(k + 1) % n]))).collect();
    if n < 3 || !(turns.iter().all(|&t| t >= -1e-6) || turns.iter().all(|&t| t <= 1e-6)) {
        return Vec::new();
    }
    if turns.iter().sum::<f64>() < 0.0 {
        pts.reverse();
    }
    if exact {
        return pts;
    }
    // Each edge pushed out as far as its squares need: every square's middle at least a tenth of
    // a square inside the wall's inner face (squares count as inside up to 1.5 ft past the
    // footprint); an edge no square reaches past stays on the footprint.
    let middles: Vec<P> = inside.iter().enumerate().filter(|(_, i)| **i).map(|(k, _)| [(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5]).collect();
    let ny = inside.len() / nx;
    let mut grown: Vec<P> = vec![[-5.0, -5.0], [nx as f64 + 5.0, -5.0], [nx as f64 + 5.0, ny as f64 + 5.0], [-5.0, ny as f64 + 5.0]];
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        let e = sub(b, a);
        let l = geom::len(e);
        if l < 1e-9 {
            continue;
        }
        let out = [e[1] / l, -e[0] / l];
        let least = middles.iter().map(|c| geom::cross(e, sub(*c, a)) / l).fold(f64::MAX, f64::min);
        let push = (WALL_HALF + 0.1 - least).max(0.0);
        grown = geom::clip_plain(&grown, out, dot(a, out) + push);
    }
    pts = grown;
    // Within the squares grown by half a square each way.
    let mut corners: Vec<P> = Vec::new();
    for (k, _) in inside.iter().enumerate().filter(|(_, i)| **i) {
        let (i, j) = ((k % nx) as f64, (k / nx) as f64);
        corners.extend([[i - 0.5, j - 0.5], [i + 1.5, j - 0.5], [i + 1.5, j + 1.5], [i - 0.5, j + 1.5]]);
    }
    let reach = geom::hull(corners);
    for k in 0..reach.len() {
        let (a, b) = (reach[k], reach[(k + 1) % reach.len()]);
        let e = sub(b, a);
        let out = [e[1], -e[0]];
        pts = geom::clip_plain(&pts, out, dot(a, out));
    }
    let mut out: Vec<P> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_none_or(|l| (l[0] - p[0]).abs() + (l[1] - p[1]).abs() > 1e-4) {
            out.push(p);
        }
    }
    while out.len() > 1 && (out[0][0] - out[out.len() - 1][0]).abs() + (out[0][1] - out[out.len() - 1][1]).abs() <= 1e-4 {
        out.pop();
    }
    if out.len() < 3 { Vec::new() } else { out }
}

/// How far point `p` lies inside convex polygon `poly` (wound anticlockwise in grid axes): its
/// least distance to an edge's line, negative outside.
fn inset_by(poly: &[P], p: P) -> f64 {
    (0..poly.len())
        .map(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            let e = sub(b, a);
            geom::cross(e, sub(p, a)) / geom::len(e).max(1e-12)
        })
        .fold(f64::MAX, f64::min)
}
