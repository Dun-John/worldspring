//! Roadmap 7 P8: a round tower's winding stair, up the inside of its wall. Its band is the ring of
//! squares within a square of the wall; each floor's run climbs about half a turn of it (clockwise
//! on screen) and comes out on the floor above where that floor's run goes on round, the upper
//! half of the run below open under a stair well there. The ground floor's run starts beside its
//! door. Its squares are `winder` items in climbing order (the well's `stair_well`), so a design
//! keeps them and the app draws the treads round the floor's middle (`InteriorLayer.drawWinders`).

use super::*;

/// A round floor's stair band: its squares in order round the floor's middle, clockwise on screen
/// (grid y down) from the grid's +x side, and the first run's foot (an index into them).
#[derive(Clone, Debug)]
pub(super) struct Winder {
    pub ring: Vec<usize>,
    pub foot: usize,
}

/// The middle (grid units) and half the width of a level's floor: the box round its squares, as the
/// app finds it (`InteriorLayer.winderFrame`).
pub(super) fn frame(inside: &[bool], nx: usize, ny: usize) -> Option<([f64; 2], f64)> {
    let sq: Vec<usize> = (0..nx * ny).filter(|&k| inside[k]).collect();
    let (i0, i1) = (sq.iter().map(|k| k % nx).min()?, sq.iter().map(|k| k % nx).max()?);
    let (j0, j1) = (sq.iter().map(|k| k / nx).min()?, sq.iter().map(|k| k / nx).max()?);
    let c = [0.5 * (i0 + i1 + 1) as f64, 0.5 * (j0 + j1 + 1) as f64];
    Some((c, 0.25 * ((i1 - i0 + 1) + (j1 - j0 + 1)) as f64))
}

/// An angle's order round a point without trigonometry: 0 along +x, rising toward +y, below 4.
fn turn(d: [f64; 2]) -> f64 {
    let r = d[0].abs() + d[1].abs();
    if r <= 0.0 {
        return 0.0;
    }
    let p = d[1] / r;
    if d[0] >= 0.0 {
        if d[1] >= 0.0 { p } else { 4.0 + p }
    } else {
        2.0 - p
    }
}

/// The stair band of a round floor (`inside`) and its foot beside the ground floor's door (`levels[0]`'s
/// front door: the band square after the one nearest the door's inside square, climbing away from
/// it; with the levels above laid out too, as far on as keeps their landings and the most of its
/// squares off their doorways); None for a floor too small for a ring of four squares.
pub(super) fn plan(levels: &[&Level], inside: &[bool], nx: usize, ny: usize) -> Option<Winder> {
    let (c, r) = frame(inside, nx, ny)?;
    let at = |k: usize| [(k % nx) as f64 + 0.5 - c[0], (k / nx) as f64 + 0.5 - c[1]];
    let mut ring: Vec<usize> = (0..nx * ny).filter(|&k| inside[k] && geom::len(at(k)) >= r - 1.0 - 1e-9).collect();
    ring.sort_by(|&a, &b| turn(at(a)).total_cmp(&turn(at(b))).then(a.cmp(&b)));
    if ring.len() < 4 {
        return None;
    }
    let lvl = levels.first()?;
    let door = lvl.doors.iter().find(|d| d.kind == "front").or_else(|| lvl.doors.iter().find(|d| d.rooms[1] < 0))?;
    let sq = door_squares(door).into_iter().filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).find(|&k| inside[k])?;
    let d = at(sq);
    let near = (0..ring.len()).min_by(|&a, &b| geom::dist(at(ring[a]), d).total_cmp(&geom::dist(at(ring[b]), d)).then(a.cmp(&b)))?;
    let n = ring.len();
    // (The ground floor's run clear of the door's square: its foot from beside it to a run short of it.)
    let feet = n - n / 2 - 1;
    let mut w = Winder { ring, foot: (near + 1) % n };
    let mut best: Option<(usize, usize)> = None;
    for f in 0..feet {
        w.foot = (near + 1 + f) % n;
        let mut bad = 0;
        for (li, lv) in levels.iter().enumerate() {
            let doorway = doorways(lv, nx, ny);
            // (A landing in a doorway counts as many as the rest.)
            if li > 0 && doorway.contains(&w.run(li, levels.len().max(li + 1))[0]) {
                bad += 100;
            }
            bad += w.run(li, levels.len()).iter().chain(&w.well(li)).filter(|k| doorway.contains(k)).count();
        }
        if best.is_none_or(|b| bad < b.0) {
            best = Some((bad, w.foot));
        }
    }
    w.foot = best?.1;
    Some(w)
}

/// The squares either side of a level's doors, on the grid.
pub(super) fn doorways(lvl: &Level, nx: usize, ny: usize) -> Vec<usize> {
    lvl.doors.iter().flat_map(door_squares).filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).collect()
}

/// The two squares a door lies between (grid indices as (i, j); one may be off the grid).
pub(super) fn door_squares(d: &Door) -> [(isize, isize); 2] {
    let (x0, y0) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize);
    if d.a[0] == d.b[0] { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
}

impl Winder {
    /// Squares the stair climbs a storey by (half a turn), and a run's length (its top square the
    /// next run's foot).
    fn step(&self) -> usize {
        self.ring.len() / 2
    }

    /// Level `li`'s run (from the ground floor, 0) of `n` levels: on the top level, the square the
    /// run below comes out on alone (kept clear).
    pub fn run(&self, li: usize, n: usize) -> Vec<usize> {
        let s = self.foot + li * self.step();
        let len = if li + 1 >= n { 0 } else { self.step() };
        (0..=len).map(|t| self.ring[(s + t) % self.ring.len()]).collect()
    }

    /// Level `li`'s stair well: over the upper half of the run below, short of its top.
    pub fn well(&self, li: usize) -> Vec<usize> {
        if li == 0 {
            return Vec::new();
        }
        let s = self.foot + (li - 1) * self.step();
        let h = self.step() / 2;
        (self.step() - h..self.step()).map(|t| self.ring[(s + t) % self.ring.len()]).collect()
    }

    /// Lay level `li`'s run and well (of `n` levels) on `lvl`: on its floor, off its doorways.
    pub fn lay(&self, lvl: &mut Level, nx: usize, ny: usize, li: usize, n: usize) {
        let doorway = doorways(lvl, nx, ny);
        let ok = |k: usize| lvl.cells[k] >= 0 && !doorway.contains(&k);
        let (run, well): (Vec<usize>, Vec<usize>) = (self.run(li, n).into_iter().filter(|&k| ok(k)).collect(), self.well(li).into_iter().filter(|&k| ok(k)).collect());
        // (Nothing else on them: what stood there goes.)
        let on = |f: &Item, k: usize| (f.x as usize..(f.x + f.w) as usize).contains(&(k % nx)) && (f.y as usize..(f.y + f.h) as usize).contains(&(k / nx));
        lvl.furniture.retain(|f| !run.iter().chain(&well).any(|&k| on(f, k)));
        for k in run {
            push_item(&mut lvl.furniture, "winder", k % nx, k / nx);
        }
        for k in well {
            push_item(&mut lvl.furniture, "stair_well", k % nx, k / nx);
        }
    }
}
