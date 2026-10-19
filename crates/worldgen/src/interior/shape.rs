//! Room shapes: a room drawn round, octagonal, with an apse or with its corners cut (or, on a
//! built level underground, as a cavern breaking in), while everything played keeps to its
//! squares: one room per square, walls, doors and sight on square edges. Only the drawing is
//! smooth, so a curved wall can stand up to half a square off the squares.
//!
//! A shape's geometry comes from the box round its room's squares (grid units), and the room's
//! squares are those whose middle lies inside it (`shape_cells`). A room keeps its shape only
//! while its squares are exactly those (`fits`); else it is drawn square. The app draws the same
//! geometry (`render/roomShapes.ts`, `shaders/underground.frag`).

/// Square corners (no shape).
pub const SQUARE: u8 = 0;
/// A circle in a square box; a stadium (round ends) in a longer one.
pub const ROUND: u8 = 1;
/// Corners cut at 45 degrees, a regular octagon in a square box.
pub const OCTAGON: u8 = 2;
/// A half-round end on one side (grid north, east, south, west).
pub const APSE_N: u8 = 3;
pub const APSE_E: u8 = 4;
pub const APSE_S: u8 = 5;
pub const APSE_W: u8 = 6;
/// Corners bevelled by three quarters of a square.
pub const CHAMFERED: u8 = 7;
/// Natural rock breaking into a built level underground: drawn as a cave's outline.
pub const CAVERN: u8 = 255;

/// Every shape by its name in the text form and the editor (`a=sanctum (round)`).
pub const NAMES: &[(u8, &str)] = &[
    (ROUND, "round"),
    (OCTAGON, "octagon"),
    (APSE_N, "apse n"),
    (APSE_E, "apse e"),
    (APSE_S, "apse s"),
    (APSE_W, "apse w"),
    (CHAMFERED, "chamfered"),
    (CAVERN, "cavern"),
];

/// A shape's name (none for square corners or an unknown code).
pub fn name(shape: u8) -> Option<&'static str> {
    NAMES.iter().find(|n| n.0 == shape).map(|n| n.1)
}

/// A shape by name ("square" too).
pub fn parse(s: &str) -> Option<u8> {
    let s = s.trim().to_ascii_lowercase();
    let s = s.as_str();
    if s == "square" {
        return Some(SQUARE);
    }
    NAMES.iter().find(|n| n.1 == s).map(|n| n.0)
}

/// A shape with geometry (not square corners, not a cavern).
pub fn geometric(shape: u8) -> bool {
    (ROUND..=CHAMFERED).contains(&shape)
}

/// Octagon corner cut, as a share of the box's shorter side: 1 - 1/sqrt(2).
const OCTAGON_CUT: f64 = 0.292_893_218_813_452_5;

/// Chamfer corner cut (grid units) for a box whose shorter side is `s`.
pub fn chamfer(s: f64) -> f64 {
    if s >= 3.0 { 0.75 } else { 0.5 }
}

/// Apse depth (grid units): half the side it rounds, keeping at least one square of the box
/// straight behind it; 0 (none) when the box is too shallow.
pub fn apse_depth(across: f64, along: f64) -> f64 {
    let r = (0.5 * across).min(along - 1.0);
    if r < 1.0 { 0.0 } else { r }
}

/// Whether grid point (x, y) lies inside `shape` drawn in box `b` (x0, y0, x1, y1, grid units).
pub fn contains(b: [f64; 4], shape: u8, x: f64, y: f64) -> bool {
    let [x0, y0, x1, y1] = b;
    if x < x0 || x > x1 || y < y0 || y > y1 {
        return false;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let (cx, cy) = (x0 + 0.5 * w, y0 + 0.5 * h);
    let (dx, dy) = (crate::core::fabs(x - cx), crate::core::fabs(y - cy));
    match shape {
        ROUND => {
            let r = 0.5 * w.min(h);
            let qx = (dx - (0.5 * w - r)).max(0.0);
            let qy = (dy - (0.5 * h - r)).max(0.0);
            qx * qx + qy * qy <= r * r
        }
        OCTAGON | CHAMFERED => {
            let c = if shape == OCTAGON { OCTAGON_CUT * w.min(h) } else { chamfer(w.min(h)) };
            dx + dy <= 0.5 * (w + h) - c
        }
        APSE_N | APSE_E | APSE_S | APSE_W => {
            // In the apse's own frame: across the apse (u, about its middle) and in from the
            // apse's side (v).
            let (u, v, across, along) = match shape {
                APSE_N => (x - cx, y - y0, w, h),
                APSE_S => (x - cx, y1 - y, w, h),
                APSE_W => (y - cy, x - x0, h, w),
                _ => (y - cy, x1 - x, h, w),
            };
            let r = apse_depth(across, along);
            if r <= 0.0 || v >= r {
                return true;
            }
            let (a, t) = (u / (0.5 * across), (v - r) / r);
            a * a + t * t <= 1.0
        }
        _ => true,
    }
}

/// The squares of box `b` (x, y, w, h in squares) whose middle lies inside `shape`, row by row.
pub fn shape_cells(b: [usize; 4], shape: u8) -> Vec<bool> {
    let [x, y, w, h] = b;
    let bf = [x as f64, y as f64, (x + w) as f64, (y + h) as f64];
    (0..w * h).map(|k| contains(bf, shape, (x + k % w) as f64 + 0.5, (y + k / w) as f64 + 0.5)).collect()
}

/// The box round room `room`'s squares (x, y, w, h), if it has any.
pub fn room_box(cells: &[i16], nx: usize, room: i16) -> Option<[usize; 4]> {
    let mut b: Option<[usize; 4]> = None;
    for (k, _) in cells.iter().enumerate().filter(|(_, c)| **c == room) {
        let (i, j) = (k % nx, k / nx);
        b = Some(match b {
            None => [i, j, i, j],
            Some([a, c, e, f]) => [a.min(i), c.min(j), e.max(i), f.max(j)],
        });
    }
    b.map(|[a, c, e, f]| [a, c, e - a + 1, f - c + 1])
}

/// Room `room` keeps `shape`: a cavern or square corners always; a shape with geometry while
/// the room's squares are exactly those inside it.
pub fn fits(cells: &[i16], nx: usize, room: i16, shape: u8) -> bool {
    if !geometric(shape) {
        return true;
    }
    let Some(b) = room_box(cells, nx, room) else { return false };
    let want = shape_cells(b, shape);
    (0..b[2] * b[3]).all(|k| want[k] == (cells[(b[1] + k / b[2]) * nx + b[0] + k % b[2]] == room))
}

/// Room `room`'s squares made those of `shape` in the box round them: squares outside it go to
/// `spare(square)` (a neighbouring room, or none: -1); squares inside it that are no room's
/// join it where `take(square)` allows. Whatever another room holds stays that room's.
pub fn fit(cells: &mut [i16], nx: usize, room: i16, shape: u8, spare: &dyn Fn(&[i16], usize) -> i16, take: &dyn Fn(usize) -> bool) {
    if !geometric(shape) {
        return;
    }
    let Some(b) = room_box(cells, nx, room) else { return };
    let want = shape_cells(b, shape);
    let squares: Vec<usize> = (0..b[2] * b[3]).map(|k| (b[1] + k / b[2]) * nx + b[0] + k % b[2]).collect();
    for (&k, &w) in squares.iter().zip(&want) {
        if w && cells[k] < 0 && take(k) {
            cells[k] = room;
        }
    }
    // (Each square given away as the room's other squares stand.)
    for (&k, &w) in squares.iter().zip(&want) {
        if !w && cells[k] == room {
            cells[k] = -1;
            cells[k] = spare(cells, k);
        }
    }
}

/// The room a square next to `k` belongs to (other than `room`, none that `skip`s), the one with
/// most sides on it (the lower index on a tie); -1 for none.
pub fn neighbour_room(cells: &[i16], nx: usize, ny: usize, k: usize, room: i16, skip: &dyn Fn(i16) -> bool) -> i16 {
    let (i, j) = (k % nx, k / nx);
    let mut seen: Vec<(i16, usize)> = Vec::new();
    for q in [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)]
        .into_iter()
        .flatten()
    {
        let r = cells[q];
        if r < 0 || r == room || skip(r) {
            continue;
        }
        match seen.iter_mut().find(|s| s.0 == r) {
            Some(s) => s.1 += 1,
            None => seen.push((r, 1)),
        }
    }
    seen.iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map_or(-1, |s| s.0)
}
