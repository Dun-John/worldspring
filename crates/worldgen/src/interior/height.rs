//! Floor heights inside a level: per square, how high its floor stands above the level's (ft), from its room's
//! `raise_ft` and the raising items on it (a dais), and which squares are ramps (rubble slopes joining two floors).
//!
//! The rules the rest reads (play, drawing, checks): a rise of up to `STEP_FT` between two squares is a step
//! (difficult terrain); a larger one between two squares neither of which is a ramp is a sheer edge (a climb going
//! up, a drop going down); a ramp is difficult terrain and joins the floors round it without a climb.

use crate::interior::{Item, Level};

/// The highest rise that is a step (a dais), not a climb (ft).
pub const STEP_FT: f32 = 3.0;
/// Items that raise the floor under them by their `height_ft` (stood on, not walked round).
pub const RAISING: [&str; 1] = ["dais"];
/// Items that are ramps where they join floors of two heights (elsewhere a heap on the floor).
pub const RAMPS: [&str; 1] = ["rubble_slope"];
/// `Level::floor`: a square's character is `FLOOR_0` + its height in half feet (`'0'`-`'~'`: 0-39 ft).
pub const FLOOR_0: u8 = b'0';
const MAX_HALF: f32 = (b'~' - FLOOR_0) as f32;

/// A level's floor heights (nx x ny, row-major): `ft` above the level's floor (0 off the floor), whole half feet.
pub struct Floor {
    pub nx: usize,
    pub ft: Vec<f32>,
    pub ramp: Vec<bool>,
}

fn sides(k: usize, nx: usize, ny: usize) -> impl Iterator<Item = usize> {
    let (i, j) = (k % nx, k / nx);
    [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten()
}

fn half(ft: f32) -> f32 {
    crate::core::round(ft as f64 * 2.0).clamp(0.0, MAX_HALF as f64) as f32 * 0.5
}

/// The floor of a level from its room per square (`cells`, -1 off the floor), each room's raise (ft), its items, and
/// whether one steps between two neighbouring floor squares (`step`: the same room, an open natural level, a door).
pub fn floor(nx: usize, ny: usize, cells: &[i16], raise: &dyn Fn(i16) -> f32, items: &[Item], step: &dyn Fn(usize, usize) -> bool) -> Floor {
    let n = nx * ny;
    let base: Vec<f32> = cells.iter().map(|&r| if r >= 0 { half(raise(r)) } else { 0.0 }).collect();
    let mut ft = base.clone();
    let mut slope = vec![false; n];
    let squares = |f: &Item| {
        let (x0, x1, y1) = (f.x as usize, ((f.x + f.w) as usize).min(nx), ((f.y + f.h) as usize).min(ny));
        (f.y as usize..y1).flat_map(move |j| (x0..x1).map(move |i| j * nx + i))
    };
    for f in items {
        if RAISING.contains(&f.kind) {
            for k in squares(f).filter(|&k| cells[k] >= 0) {
                ft[k] = ft[k].max(half(base[k] + f.height_ft));
            }
        } else if RAMPS.contains(&f.kind) {
            for k in squares(f).filter(|&k| cells[k] >= 0) {
                slope[k] = true;
            }
        }
    }
    // Each run of slope squares is a ramp where the floors it joins (its own rooms' and those stepped to from it)
    // differ: halfway between the lowest and the highest.
    let mut ramp = vec![false; n];
    let mut seen = vec![false; n];
    for s in 0..n {
        if !slope[s] || seen[s] {
            continue;
        }
        let mut run = vec![s];
        seen[s] = true;
        let (mut lo, mut hi) = (base[s], base[s]);
        let mut k = 0;
        while k < run.len() {
            let a = run[k];
            k += 1;
            lo = lo.min(base[a]);
            hi = hi.max(base[a]);
            for b in sides(a, nx, ny) {
                if cells[b] < 0 || !step(a, b) {
                    continue;
                }
                if slope[b] {
                    if !seen[b] {
                        seen[b] = true;
                        run.push(b);
                    }
                } else {
                    lo = lo.min(ft[b]);
                    hi = hi.max(ft[b]);
                }
            }
        }
        if hi > lo {
            let mid = half((lo + hi) * 0.5);
            for &a in &run {
                ft[a] = mid;
                ramp[a] = true;
            }
        }
    }
    Floor { nx, ft, ramp }
}

/// `Level::floor` and `Level::ramps` for a floor: empty where it is flat.
pub fn encode(f: &Floor) -> (String, Vec<u32>) {
    if f.ft.iter().all(|&h| h == 0.0) && !f.ramp.contains(&true) {
        return (String::new(), Vec::new());
    }
    let s = f.ft.iter().map(|&h| (FLOOR_0 + (h * 2.0) as u8) as char).collect();
    (s, (0..f.ramp.len()).filter(|&k| f.ramp[k]).map(|k| k as u32).collect())
}

/// A level's floor as it was built (`nx` its grid's width).
pub fn of_level(lv: &Level, nx: usize) -> Floor {
    let n = lv.cells.len();
    let ft = if lv.floor.is_empty() { vec![0.0; n] } else { lv.floor.bytes().map(|c| (c - FLOOR_0) as f32 * 0.5).collect() };
    let mut ramp = vec![false; n];
    for &k in &lv.ramps {
        ramp[k as usize] = true;
    }
    Floor { nx, ft, ramp }
}

impl Floor {
    /// The sheer rise stepping from square `a` to its neighbour `b` (ft; negative a drop), 0 where none: more than
    /// `STEP_FT` with neither a ramp.
    pub fn sheer(&self, a: usize, b: usize) -> f32 {
        let d = self.ft[b] - self.ft[a];
        if d.abs() > STEP_FT && !self.ramp[a] && !self.ramp[b] { d } else { 0.0 }
    }

    /// Stepping from `a` to its neighbour `b` is difficult terrain: onto a ramp, or a step up or down.
    pub fn difficult(&self, a: usize, b: usize) -> bool {
        let d = (self.ft[b] - self.ft[a]).abs();
        self.ramp[b] || (d > 0.0 && d <= STEP_FT)
    }
}
