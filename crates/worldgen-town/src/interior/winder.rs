//! Roadmap 7 P8: a round tower's winding stair, up the inside of its wall. Its band is the ring of
//! squares within a square of the wall; each floor's run climbs about half a turn of it (clockwise
//! on screen) and comes out on the floor above where that floor's run goes on round, the upper
//! half of the run below open under a stair well there. The ground floor's run starts beside its
//! door. Its squares are `winder` items in climbing order (the well's `stair_well`), so a design
//! keeps them and the app draws the treads round the floor's middle (`InteriorLayer.drawWinders`).

use super::*;

/// A round floor's stair band: its squares in order round the floor's middle, clockwise on screen
/// (grid y down) from the grid's +x side, and where each level's run starts on it.
#[derive(Clone, Debug)]
pub(super) struct Winder {
    pub ring: Vec<usize>,
    /// Where each level's run starts round the ring (from the lowest level; rising, unwrapped).
    pub starts: Vec<usize>,
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

/// A round floor's stair band (`inside`): its squares within a square of the wall, in order round
/// the floor's middle; None for one too small for a ring of four.
pub(super) fn ring_of(inside: &[bool], nx: usize, ny: usize) -> Option<Vec<usize>> {
    let (c, r) = frame(inside, nx, ny)?;
    let at = |k: usize| [(k % nx) as f64 + 0.5 - c[0], (k / nx) as f64 + 0.5 - c[1]];
    let mut ring: Vec<usize> = (0..nx * ny).filter(|&k| inside[k] && geom::len(at(k)) >= r - 1.0 - 1e-9).collect();
    ring.sort_by(|&a, &b| turn(at(a)).total_cmp(&turn(at(b))).then(a.cmp(&b)));
    (ring.len() >= 4).then_some(ring)
}

/// The winding stair of a round floor (`inside`) through `total` levels, `levels` the ones built so
/// far from its lowest (their doorways and links kept clear where the band allows), its run on
/// level `ground` starting beside that level's front door (climbing away from it).
pub(super) fn plan(levels: &[&Level], ground: usize, total: usize, inside: &[bool], nx: usize, ny: usize) -> Option<Winder> {
    let ring = ring_of(inside, nx, ny)?;
    let lvl = levels.get(ground)?;
    let door = lvl.doors.iter().find(|d| d.kind == "front").or_else(|| lvl.doors.iter().find(|d| d.rooms[1] < 0))?;
    let sq = door_squares(door).into_iter().filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).map(|(i, j)| j as usize * nx + i as usize).find(|&k| inside[k])?;
    Some(plan_ring(ring, levels, ground, total, Some(sq), nx, ny).0)
}

/// The winding stair round `ring` (`ring_of`) through `total` levels (`levels` the ones built so
/// far), its foot on level `ground` beside square `door` if given: each run about half a turn,
/// shorter (down to a square) where that keeps every doorway and link off the runs and wells; and
/// whether it does (none may, as round a keep tower with a door on every floor).
pub(super) fn plan_ring(ring: Vec<usize>, levels: &[&Level], ground: usize, total: usize, door: Option<usize>, nx: usize, ny: usize) -> (Winder, bool) {
    let n = ring.len();
    let step = n / 2;
    let total = total.max(1);
    // (Per level, each ring square: 0 free, 1 a doorway, 2 a link's.)
    let avoid: Vec<Vec<u8>> = (0..total)
        .map(|li| {
            let mut a = vec![0u8; n];
            if let Some(lv) = levels.get(li) {
                let (door, link): (Vec<usize>, Vec<usize>) = (doorways(lv, nx, ny), lv.links.iter().map(|q| q.y as usize * nx + q.x as usize).collect());
                for (t, k) in ring.iter().enumerate() {
                    a[t] = if link.contains(k) { 2 } else { door.contains(k) as u8 };
                }
            }
            a
        })
        .collect();
    // (The ring square nearest the door: the ground floor's foot best the one after it.)
    let at = |k: usize| [(k % nx) as f64, (k / nx) as f64];
    let near = door.and_then(|d| (0..n).min_by(|&a, &b| geom::dist(at(ring[a]), at(d)).total_cmp(&geom::dist(at(ring[b]), at(d))).then(a.cmp(&b))));
    const HIT: u64 = 1_000_000;
    // (A link's square never: what stands there would go.)
    let hits = |li: usize, from: usize, len: usize| (from..=from + len).map(|p| [0, HIT, 10 * HIT][avoid[li][p % n] as usize]).sum::<u64>();

    let pref = |li: usize, p: usize| if li == ground { near.map_or(0, |q| ((p + 2 * n - q - 1) % n) as u64) } else { 0 };
    // Cheapest runs level by level: a state is where the last run came out and its length (the well
    // above it over its upper half, short of its top); a run short of half a turn costs the more
    // the shorter, so the shortening is shared between levels.
    let lens = step.max(2) + 1;
    let idx = |p: usize, len: usize| p * lens + len;
    // (Each level's table: cost, the state it came from, where its run started.)
    let mut tables: Vec<Vec<Option<(u64, usize, usize)>>> = Vec::new();
    let mut best: Option<(u64, usize)> = None;
    for li in 0..total {
        let top = li + 1 == total;
        let mut t: Vec<Option<(u64, usize, usize)>> = vec![None; n * lens];
        let froms: Vec<(usize, u64, usize, usize)> = if li == 0 {
            (0..n).map(|p| (p, 0, 0, usize::MAX)).collect()
        } else {
            tables[li - 1].iter().enumerate().filter_map(|(s, e)| e.map(|(c, _, _)| (s / lens, c, s % lens, s))).collect()
        };
        for (p, c0, plen, s) in froms {
            let h = plen / 2;
            let well = if h > 0 { hits(li, p + n - h, h - 1) } else { 0 };
            let c = c0 + well * 2 + pref(li, p);
            if top {
                let c = c + hits(li, p, 0);
                if best.is_none_or(|b| c < b.0) {
                    best = Some((c, s));
                }
                continue;
            }
            for len in 1..lens.min(n) {
                if len + 1 + h > n {
                    break;
                }
                let c = c + hits(li, p, len) + (step.abs_diff(len) as u64).pow(2) * 10;
                let e = idx((p + len) % n, len);
                if t[e].is_none_or(|q| c < q.0) {
                    t[e] = Some((c, s, p));
                }
            }
        }
        tables.push(t);
    }
    // (Back from the cheapest top: each level's run's start.)
    let mut starts = vec![0usize; total];
    let mut s = best.map_or(usize::MAX, |b| b.1);
    if total == 1 {
        starts[0] = near.map_or(0, |q| (q + 1) % n);
    } else {
        starts[total - 1] = s / lens;
        for li in (0..total - 1).rev() {
            let Some((_, prev, p)) = tables[li][s] else { break };
            starts[li] = p;
            s = prev;
        }
    }
    // (Unwrapped: each start on from the one before.)
    for li in 1..total {
        while starts[li] <= starts[li - 1] {
            starts[li] += n;
        }
    }
    (Winder { ring, starts }, best.is_some_and(|b| b.0 < HIT))
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
    /// Level `li`'s run (from the lowest, 0): from its start to the next level's (its top that run's
    /// foot); on the top level, the square the run below comes out on alone (kept clear).
    pub fn run(&self, li: usize) -> Vec<usize> {
        let Some(&s) = self.starts.get(li) else { return Vec::new() };
        let e = self.starts.get(li + 1).copied().unwrap_or(s);
        (s..=e).map(|t| self.ring[t % self.ring.len()]).collect()
    }

    /// Level `li`'s stair well: over the upper half of the run below, short of its top.
    pub fn well(&self, li: usize) -> Vec<usize> {
        if li == 0 || li >= self.starts.len() {
            return Vec::new();
        }
        let s = self.starts[li];
        let h = (s - self.starts[li - 1]) / 2;
        (s - h..s).map(|t| self.ring[t % self.ring.len()]).collect()
    }

    /// Lay level `li`'s run and well on `lvl`: on its floor, the well off its doorways (a run kept
    /// whole, through a doorway where `plan_ring` found no way round one).
    pub fn lay(&self, lvl: &mut Level, nx: usize, ny: usize, li: usize) {
        let doorway = doorways(lvl, nx, ny);
        let (run, well): (Vec<usize>, Vec<usize>) = (self.run(li).into_iter().filter(|&k| lvl.cells[k] >= 0).collect(), self.well(li).into_iter().filter(|&k| lvl.cells[k] >= 0 && !doorway.contains(&k)).collect());
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
