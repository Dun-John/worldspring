//! The zone engine for big interiors: a castle's keep (and a big barracks) laid out as real ones
//! were, not as a grid of rooms. Its body is cut into zones along its long axis, from the low end
//! (where it is entered) to the high end: the service end (kitchen, buttery, pantry), the screens
//! passage across the body (the front door at one end of it), the great hall (the body's full
//! width when it is 6 to 9 squares across, else a strip of chambers beside it, a mural passage
//! along the outer wall when 4 deep), and the dais end (a parlour, the landing with the main stair
//! behind the high end of the hall, a wardrobe). Upper floors: a cross passage under the landing
//! and a mural passage on from it down one long wall (the other wall on the next storey), the rest
//! divided by cross walls into rooms of sensible proportions, each on the passage or beside a room
//! that is (`partition`); on the lord's floor the great chamber by the landing, about twice as wide
//! as it is deep, and the solar beside the landing or beyond the great chamber's cross wall; the
//! chapel, chambers, guest chambers, a treasury, an armory and a garrison above, no storey on the
//! squares of the one below. Over a hall open two storeys, its height left open (`OPEN_BELOW`) with a minstrels'
//! gallery over the screens end, reached along a mural passage from the landing. Wings (an L or a
//! T) hold chambers off a mural passage when 4 squares deep or more; a forebuilding the entrance
//! hall, a chapel above and a walled platform on top. A great tower (or a small keep) has its hall
//! on the first floor over stores, a well and the way in to the stair; a shell keep is a ring of
//! lean-tos round a round yard. Each piece rises as many storeys as it has (a forebuilding 2, wings
//! one fewer than the body, towers one more) and ends in its top as the battlemap draws it
//! (`town::keep_tops`): open battlements, or a parapet walk round a roof (`ROOF_SLOPE`) that is one
//! box, hipped. Towers are rooms of their own on every floor up to theirs (guard posts and stores
//! below, chambers and closets above, a garrison under the cone), with a spiral stair at the middle
//! (`super::hub_level`); doors go where the zones want them (`Hints`). A tower keep too small for zones
//! along it has a room or two a storey off a landing across its high end (`small_plan`). Each storey
//! above the hall is tidied (`tidy`: rooms over 2.5:1 or 40 squares cut across, tails a square wide
//! given away), its rooms of one size varied (`vary`), its lord's rooms put by the main stair
//! (`great_chamber`, `solar`, a tower's room where the body has none: `lords_in_towers`), and every
//! room given a door within a room of a way about the floor (`near_links`).
//!
//! Zones along an axis, strips into wings and door hints are general: palaces, temples, guild
//! halls and inns are to be laid out with them too.

use super::{Hints, Item, OPEN_BELOW, ROOF_SLOPE, Room, Shell, building_item, is_circulation, shape};
use crate::core::rng::{Pcg32, hash3};
use crate::town::{self, PartKind, geom::{self, P}};

/// The kinds of building laid out by zones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A hall keep: a long body with a forebuilding before its door, its hall on the ground floor.
    Hall,
    /// A great tower with a tower on each corner: its hall on the first floor.
    Great,
    /// A block with a wing at one end (its towers at its outer corners).
    L,
    /// A block with a wing from the middle of a long side.
    T,
    /// A round shell keep on a motte: lean-tos round an open yard.
    ShellKeep,
    /// A small tower keep of one piece, or a castle's keep redrawn by hand: as a great tower.
    Plain,
    /// A barracks of 150 squares or more: a mess hall at the heart, barrack rooms above.
    Barracks,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Hall => "hall keep",
            Kind::Great => "great tower",
            Kind::L => "L keep",
            Kind::T => "T keep",
            Kind::ShellKeep => "shell keep",
            Kind::Plain => "tower keep",
            Kind::Barracks => "barracks",
        }
    }
}

/// What kind of keep a castle's keep built of pieces is (`Building::parts`), as `town::castle`
/// laid it out; None for a building without pieces.
pub fn keep_kind(b: &town::Building) -> Option<Kind> {
    if b.parts.is_empty() {
        return None;
    }
    let (mut towers, mut wing, mut fore) = (0, false, false);
    for p in &b.parts {
        match p.kind {
            PartKind::Tower { .. } => towers += 1,
            PartKind::Wing => wing = true,
            PartKind::Forebuilding => fore = true,
            PartKind::Body => {}
        }
    }
    Some(if fore {
        Kind::Hall
    } else if wing {
        if towers == 3 { Kind::L } else { Kind::T }
    } else if b.parts[0].poly.len() > 4 && towers == 0 {
        Kind::ShellKeep
    } else if towers > 0 {
        Kind::Great
    } else {
        Kind::Plain
    })
}

/// What kind of building `plan` lays out by zones, if any: a castle's keep (of pieces, else as a
/// tower keep) or a barracks of 150 squares or more (`inside`: its squares).
pub fn kind_of(b: &town::Building, key: Option<&str>, inside: usize) -> Option<Kind> {
    let castle = key == Some("castle") || b.keep;
    match keep_kind(b) {
        Some(k) => Some(k),
        None if key == Some("barracks") && !castle && inside >= 150 => Some(Kind::Barracks),
        None if castle => Some(Kind::Plain),
        None => None,
    }
}

/// What a grid square is part of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Piece {
    Out,
    Body,
    Wing(u8),
    Fore,
    Tower,
}

/// The body's frame on the grid: square (s, t), s along its long axis from its low end, t across
/// it from its front side, is grid square `o + s * ds + t * dt`.
#[derive(Clone, Copy, Debug)]
struct Frame {
    o: (isize, isize),
    ds: (isize, isize),
    dt: (isize, isize),
    len: usize,
    wid: usize,
}

impl Frame {
    fn grid(&self, s: isize, t: isize) -> (isize, isize) {
        (self.o.0 + self.ds.0 * s + self.dt.0 * t, self.o.1 + self.ds.1 * s + self.dt.1 * t)
    }

    fn local(&self, i: isize, j: isize) -> (isize, isize) {
        let (di, dj) = (i - self.o.0, j - self.o.1);
        (di * self.ds.0 + dj * self.ds.1, di * self.dt.0 + dj * self.dt.1)
    }
}

/// A floor laid out by zones, before its towers and doors (`super::hub_level`).
pub struct Laid {
    pub cells: Vec<i16>,
    pub rooms: Vec<Room>,
    pub hints: Hints,
    /// The room kind that serves as circulation besides the passages (the hall on its floor).
    pub public: Option<&'static str>,
    pub name: &'static str,
    /// Open to the sky (the body's top): its outer wall a parapet.
    pub roof: bool,
    /// Pieces put down before the furniture (a shell keep's well).
    pub items: Vec<Item>,
    /// The keep's towers standing on this floor (indices into `Shell::towers`), with their rooms'
    /// kinds.
    pub towers: Vec<(usize, &'static str)>,
    /// The kinds of the rooms in a grid's corner towers (a barracks'), in `super::hub_level`'s order.
    pub corners: Vec<&'static str>,
    /// On the hall's floor, the hall's frame for its furnishing (`super::hall`): from its low end to
    /// the dais end.
    pub hall: Option<super::hall::Frame>,
}

/// A piece of the body's top (s from `s0` to `s1`): its code (`town::keep_tops`: 0 open
/// battlements, else roofed inside a parapet walk) and whether its walk runs along its ends too.
#[derive(Clone, Copy, Debug)]
struct Top {
    s0: usize,
    s1: usize,
    code: u8,
    walk_lo: bool,
    walk_hi: bool,
}

/// A wing: its squares, along the joint `s0..s1` (body frame), outward `b` squares deep, on the
/// body's far side (t past its width) or its front side.
#[derive(Clone, Debug)]
struct Wing {
    squares: Vec<usize>,
    s0: usize,
    s1: usize,
    deep: usize,
    far: bool,
    /// Its rows (t, body frame): `t0..t1`.
    t0: isize,
    t1: isize,
}

/// A shell keep's ring of lean-tos round its round yard: the yard (the squares whose middles lie
/// in the circle of its box, `shape::ROUND`), the gate passage, the lean-tos' outer and inner
/// rows, the stairs' landing.
#[derive(Clone, Debug)]
struct Ring {
    yard: Vec<bool>,
    gate: Vec<bool>,
    outer: Vec<bool>,
    inner: Vec<bool>,
    landing: Vec<bool>,
    /// Each ring square's angle round from the gate (0 to 1).
    turn: Vec<f64>,
    centre: P,
    front: Vec<(usize, usize, usize)>,
}

/// How a building's floors are laid out by zones (`plan`).
pub struct Plan {
    pub kind: Kind,
    nx: usize,
    ny: usize,
    f: Frame,
    piece: Vec<Piece>,
    /// The service end's depth (rows from the low end); the screens passage after it, if any.
    service: usize,
    screens: bool,
    /// The dais end from this row (the landing, the main stair).
    dz: usize,
    hall_t: (usize, usize),
    land_t: (usize, usize),
    /// The main stair block (grid: x, y, w, h).
    pub stairs: [usize; 4],
    /// The floor the hall is on; whether it is open two storeys (a void above it).
    pub hall_z: i8,
    pub open: bool,
    /// The body's storeys; a wing's and a forebuilding's.
    pub floors: i8,
    wing_floors: i8,
    fore_floors: i8,
    /// Built of pieces: the body's top is a level of its own (`floors`), towers have no tops.
    pub pieces: bool,
    top: Vec<Top>,
    wing_tops: Vec<u8>,
    wings: Vec<Wing>,
    fore: Vec<usize>,
    /// Each tower's top floor (`Shell::towers`' order).
    tower_top: Vec<i8>,
    /// Each tower's room's squares (`Shell::towers`' order).
    tower_sq: Vec<Vec<usize>>,
    /// A wing's or the forebuilding's squares in the body's box (where it overlaps the body's
    /// edge), what they are and the piece's top: the body's above it.
    overlap: Vec<(usize, Piece, i8)>,
    ring: Option<Ring>,
    seed: u64,
    /// Rooms each upper floor must still have, in order (a treasury, an armory...).
    pending: Vec<&'static str>,
    chapel: bool,
    /// The low end's and the front side's order of preference for the front door (nearer the
    /// street first).
    ends: [bool; 2],
    /// The long wall the last storey's mural passage ran along (t), -1 none yet.
    side: isize,
    /// A tower room is the oratory.
    oratory: bool,
    /// The last upper storey's body squares by room (`layout_of`): the next is laid out otherwise.
    last: Vec<i16>,
    /// A barracks entered from its low end (its long sides shut in): the mess hall reaches the
    /// end wall in a bay between the kitchen and the pantry, at this column (t), with open ground
    /// before it, that leaves the kitchen the most room.
    bay: Option<usize>,
    /// A small tower keep (`small_plan`): each storey a room or two off the landing at its high end.
    small: bool,
}

/// Room kinds by relative size, for a floor's rooms to take biggest first.
type Kinds = Vec<(&'static str, f64)>;

/// A building's plan, if it is laid out by zones: a castle's keep built of pieces, a castle's keep
/// redrawn by hand (as a tower keep) and a barracks of 150 squares or more. `corner`: the grid's
/// corner towers' side (a keep without pieces, a barracks); `tops`: `town::keep_tops`; `near`: how
/// near the street a grid point is (lower nearer); `ground`: whether a grid square outside is open
/// ground (`Shell::ext_free`).
#[allow(clippy::too_many_arguments)]
pub fn plan(b: &town::Building, sh: &Shell, key: Option<&str>, corner: usize, tops: &[u8], seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let kind = kind_of(b, key, sh.inside.iter().filter(|&&i| i).count())?;
    // Each square's piece.
    let mut piece = vec![Piece::Out; nx * ny];
    let mut wing_of: Vec<usize> = Vec::new();
    for k in 0..nx * ny {
        if !sh.inside[k] {
            continue;
        }
        piece[k] = if b.parts.is_empty() {
            Piece::Body
        } else {
            match b.parts.get(sh.owner[k]).map(|p| p.kind) {
                Some(PartKind::Body) => Piece::Body,
                Some(PartKind::Wing) => {
                    let w = match wing_of.iter().position(|&p| p == sh.owner[k]) {
                        Some(w) => w,
                        None => {
                            wing_of.push(sh.owner[k]);
                            wing_of.len() - 1
                        }
                    };
                    Piece::Wing(w as u8)
                }
                Some(PartKind::Forebuilding) => Piece::Fore,
                Some(PartKind::Tower { .. }) => Piece::Tower,
                None => Piece::Out,
            }
        };
    }
    for (sq, _, _) in &sh.towers {
        for &k in sq {
            piece[k] = Piece::Tower;
        }
    }
    // (A grid's corner towers: theirs.)
    if corner > 0 && nx > 2 * corner && ny > 2 * corner {
        for (cx, cy) in [(0, 0), (nx - corner, 0), (0, ny - corner), (nx - corner, ny - corner)] {
            for j in cy..cy + corner {
                for i in cx..cx + corner {
                    if piece[j * nx + i] != Piece::Out {
                        piece[j * nx + i] = Piece::Tower;
                    }
                }
            }
        }
    }
    let grid_p = |p: P| {
        let d = geom::sub(p, sh.origin);
        [geom::dot(d, sh.u) / super::SQUARE_FT, geom::dot(d, sh.v) / super::SQUARE_FT]
    };
    let mut rng = Pcg32::new(hash3(seed, 0x6b, 0x11, 0), 81);
    if kind == Kind::ShellKeep {
        return shell_plan(b, sh, piece, seed, &grid_p);
    }
    // The body's box.
    let body: Vec<usize> = (0..nx * ny).filter(|&k| piece[k] == Piece::Body).collect();
    if body.is_empty() {
        return None;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
    for &k in &body {
        (x0, x1, y0, y1) = (x0.min(k % nx), x1.max(k % nx), y0.min(k / nx), y1.max(k / nx));
    }
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let long_x = bw >= bh;
    let (len, wid) = if long_x { (bw, bh) } else { (bh, bw) };
    // (A tower keep too small for zones along it (16 squares or more): laid out as a small one,
    // `small_plan`.)
    let small = len < 7 || wid < 5;
    if small && !(kind == Kind::Plain && len >= 4 && wid >= 4 && b.floors >= 3) {
        return None;
    }
    // Where the wings and the forebuilding lie (grid), to set the frame by.
    let mean = |f: &dyn Fn(Piece) -> bool| {
        let sq: Vec<usize> = (0..nx * ny).filter(|&k| f(piece[k])).collect();
        (!sq.is_empty()).then(|| [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len() as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len() as f64])
    };
    let fore_at = mean(&|p| p == Piece::Fore);
    let wing_at = mean(&|p| matches!(p, Piece::Wing(_)));
    let mid = [0.5 * (x0 + x1 + 1) as f64, 0.5 * (y0 + y1 + 1) as f64];
    // (Along the long axis: the coordinate; across: the other.)
    let along = |p: P| if long_x { p[0] } else { p[1] };
    let across = |p: P| if long_x { p[1] } else { p[0] };
    let ends = if long_x { [[x0 as f64, mid[1]], [(x1 + 1) as f64, mid[1]]] } else { [[mid[0], y0 as f64], [mid[0], (y1 + 1) as f64]] };
    let sides = if long_x { [[mid[0], y0 as f64], [mid[0], (y1 + 1) as f64]] } else { [[x0 as f64, mid[1]], [(x1 + 1) as f64, mid[1]]] };
    let street_low = near(ends[0]) <= near(ends[1]);
    let street_front = near(sides[0]) <= near(sides[1]);
    // (A barracks' mess hall opens on its front: the long side with more open ground before it, a
    // range built against a curtain turned from it; with both shut in, the end with more, the
    // kitchen's: `Plan::bay`. Its corners aside, a corner tower's.)
    let in_at = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && sh.inside[j as usize * nx + i as usize];
    let open_along = |long: bool, lo: bool| {
        let along_x = long == long_x;
        let (c, d) = match (along_x, lo) {
            (true, true) => (y0 as isize, -1),
            (true, false) => (y1 as isize, 1),
            (false, true) => (x0 as isize, -1),
            (false, false) => (x1 as isize, 1),
        };
        let (a0, a1) = if along_x { (x0 + 3, x1.saturating_sub(3)) } else { (y0 + 3, y1.saturating_sub(3)) };
        (a0 as isize..=a1 as isize)
            .filter(|&a| {
                let (i, j, oi, oj) = if along_x { (a, c, a, c + d) } else { (c, a, c + d, a) };
                in_at(i, j) && !in_at(oi, oj) && ground(oi, oj)
            })
            .count()
    };
    let mut bay = false;
    let bay_t = |f: &Frame, len: usize, wid: usize, piece: &[Piece]| {
        // (Through every row of the service end, open ground before it; the kitchen the bigger
        // side, as big as can be, then nearer the middle.)
        let service = if len >= 14 { 3 } else { 2 };
        let body = |s: isize, t: isize| {
            let (i, j) = f.grid(s, t);
            in_at(i, j) && piece[j as usize * nx + i as usize] == Piece::Body
        };
        let side = |t0: isize, t1: isize| (0..service).map(|s| (t0..t1).filter(|&t| body(s, t)).count()).sum::<usize>();
        let w = wid as isize;
        (0..w)
            .filter(|&t| {
                let (i, j) = f.grid(-1, t);
                (0..service).all(|s| body(s, t)) && !in_at(i, j) && ground(i, j)
            })
            .max_by_key(|&t| (side(0, t).max(side(t + 1, w)), std::cmp::Reverse((2 * t - w + 1).abs()), std::cmp::Reverse(t)))
            .map(|t| t as usize)
    };
    // The low end (s = 0 at the box's lower edge?) and the front side (t = 0 at its lower edge?).
    let (low_min, front_min) = match kind {
        // (The service end by the forebuilding, on its side.)
        Kind::Hall => {
            let f = fore_at.unwrap_or(mid);
            (along(f) < along(mid), across(f) < across(mid))
        }
        // (The wing at the high end, on the far side.)
        Kind::L => {
            let w = wing_at.unwrap_or(mid);
            (along(w) >= along(mid), across(w) >= across(mid))
        }
        Kind::T => {
            let w = wing_at.unwrap_or(mid);
            (street_low, across(w) >= across(mid))
        }
        Kind::Barracks => {
            let (lo, hi) = (open_along(true, true), open_along(true, false));
            let (el, eh) = (open_along(false, true), open_along(false, false));
            if lo + hi == 0 && el + eh > 0 {
                bay = true;
                (el >= eh, street_front)
            } else {
                (street_low, if lo == hi { street_front } else { lo > hi })
            }
        }
        _ => (street_low, street_front),
    };
    let f = {
        let (lo_a, hi_a, lo_c, hi_c) = if long_x { (x0, x1, y0, y1) } else { (y0, y1, x0, x1) };
        let (oa, da) = if low_min { (lo_a as isize, 1) } else { (hi_a as isize, -1) };
        let (oc, dc) = if front_min { (lo_c as isize, 1) } else { (hi_c as isize, -1) };
        if long_x {
            Frame { o: (oa, oc), ds: (da, 0), dt: (0, dc), len, wid }
        } else {
            Frame { o: (oc, oa), ds: (0, da), dt: (dc, 0), len, wid }
        }
    };
    let at = |s: isize, t: isize| {
        let (i, j) = f.grid(s, t);
        (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    if small {
        return small_plan(b, sh, piece, f, seed, street_front == front_min);
    }
    let body_at = |s: isize, t: isize| at(s, t).filter(|&k| piece[k] == Piece::Body);
    // The landing in the middle of the dais end; the main stair in it, along the axis, a square in
    // from the end wall (clear of the outer wall as drawn), its free side toward the hall's door.
    let lw = 2 + wid % 2;
    let tl0 = (wid - lw) / 2;
    let (l, w) = (len as isize, wid as isize);
    let find = |back: isize| {
        for tc in [tl0 as isize + 1, tl0 as isize + lw as isize - 1, tl0 as isize] {
            let s0 = l - 1 - back - 1;
            if s0 < 4 || tc < 0 || tc >= w {
                continue;
            }
            if [(s0, tc), (s0 + 1, tc)].iter().all(|&(s, t)| body_at(s, t).is_some_and(|k| sh.whole[k])) {
                return Some((s0, tc));
            }
        }
        None
    };
    let mut stair = find(1).or_else(|| find(2));
    // (A body too short for a hall of 3 rows behind its screens and a service end of 2: the stair
    // against the end wall.)
    if kind != Kind::Barracks && stair.is_some_and(|(s0, _)| s0 - 3 < 3) {
        stair = find(0).or(stair);
    }
    let (ss, st) = stair?;
    let dz = ss as usize;
    let (a, bq) = (f.grid(ss, st), f.grid(ss + 1, st));
    let stairs = [a.0.min(bq.0) as usize, a.1.min(bq.1) as usize, (a.0 - bq.0).unsigned_abs() + 1, (a.1 - bq.1).unsigned_abs() + 1];
    // The hall: 4 squares long at least, after the service end and the screens passage.
    let hall_min = 4;
    let (mut service, mut screens) = if kind == Kind::Barracks { (if len >= 14 { 3 } else { 2 }, false) } else { (if len >= 14 { 3 } else { 2 }, true) };
    if kind == Kind::Hall {
        // (The screens passage where the forebuilding meets the body, as near 3 rows in as it can.)
        let fs: Vec<isize> = (0..nx * ny).filter(|&k| piece[k] == Piece::Fore).map(|k| f.local((k % nx) as isize, (k / nx) as isize).0).collect();
        let (lo, hi) = (fs.iter().copied().min().unwrap_or(0).max(2), fs.iter().copied().max().unwrap_or(3));
        if lo <= hi {
            service = (3isize.clamp(lo, hi)) as usize;
        }
    }
    let rows = |service: usize, screens: bool| dz as isize - service as isize - screens as isize;
    if rows(service, screens) < hall_min {
        service = 2;
    }
    // (A hall too short for 4 rows behind its screens and service end keeps its screens along the
    // end wall, the kitchen and buttery elsewhere on its floor (`level`), where that gives it 4; else
    // 3 rows behind both, else 3 behind its screens, else neither.)
    if rows(service, screens) < hall_min && screens {
        if rows(0, true) >= hall_min || rows(service, true) < 3 {
            service = 0;
        }
        screens = rows(service, true) >= 3;
    }
    if rows(service, screens) < 3 {
        (service, screens) = (0, false);
    }
    if rows(service, screens) < 3 {
        return None;
    }
    // (A service end too small for a kitchen and a buttery or pantry (10 squares): a row deeper,
    // where the hall keeps 4 rows.)
    let held = |n: usize| (0..n).map(|s| (0..wid).filter(|&t| body_at(s as isize, t as isize).is_some()).count()).sum::<usize>();
    if kind != Kind::Hall && screens && service >= 2 && held(service) < 10 && rows(service + 1, screens) >= hall_min {
        service += 1;
    }
    let hall_t = if wid <= 9 { (0, wid) } else { (0, (wid - 3).min(9).max(tl0 + lw)) };
    let floors = b.floors.max(1) as i8;
    let hall_z = if matches!(kind, Kind::Great | Kind::Plain) && floors >= 2 { 1 } else { 0 };
    // About half the keeps' halls rise two storeys (where there is a floor over the hall, and one
    // over that for the lord's rooms; not in a small keep).
    let open = kind != Kind::Barracks && hall_z + 2 < floors && len * wid >= 80 && rng.next_f64() < 0.5;
    let pieces = !b.parts.is_empty();
    // The body's top: a long body is a donjon at one end (open or roofed) and a hall range roofed
    // inside its walk; a squarer one roofed all over (as the battlemap draws them).
    let mut top = Vec::new();
    if pieces {
        let code = tops.first().copied().unwrap_or(2);
        let poly = &b.parts[0].poly;
        let long = poly.len() == 4 && {
            let (p, q) = (geom::dist(poly[0], poly[1]), geom::dist(poly[1], poly[2]));
            p.max(q) > 1.35 * p.min(q)
        };
        if long {
            let dl = (crate::core::round((wid as f64).min(0.42 * len as f64)) as usize).clamp(2, len - 2);
            // (Its donjon at its first corner's end, if `DONJON_LOW`.)
            let c0 = grid_p(poly[0]);
            let (s_c0, _) = f.local(crate::core::floor(c0[0]) as isize, crate::core::floor(c0[1]) as isize);
            let at_low = (code & town::DONJON_LOW != 0) == ((s_c0 as f64) < 0.5 * len as f64);
            let (d, r) = if at_low { ((0, dl), (dl, len)) } else { ((len - dl, len), (0, len - dl)) };
            top.push(Top { s0: d.0, s1: d.1, code: code & 3, walk_lo: true, walk_hi: true });
            top.push(Top { s0: r.0, s1: r.1, code: 2, walk_lo: !at_low, walk_hi: at_low });
        } else {
            top.push(Top { s0: 0, s1: len, code: if code & 3 == 0 { 0 } else { code & 3 }, walk_lo: true, walk_hi: true });
        }
    }
    // Wings: their squares and how they meet the body.
    let mut wings = Vec::new();
    let mut wing_tops = Vec::new();
    for (wi, &pi) in wing_of.iter().enumerate() {
        let squares: Vec<usize> = (0..nx * ny).filter(|&k| piece[k] == Piece::Wing(wi as u8)).collect();
        let st: Vec<(isize, isize)> = squares.iter().map(|&k| f.local((k % nx) as isize, (k / nx) as isize)).collect();
        let far = st.iter().map(|q| q.1).sum::<isize>() > 0;
        let (s0, s1) = st.iter().fold((isize::MAX, isize::MIN), |a, q| (a.0.min(q.0), a.1.max(q.0 + 1)));
        let (t0, t1) = st.iter().fold((isize::MAX, isize::MIN), |a, q| (a.0.min(q.1), a.1.max(q.1 + 1)));
        if squares.is_empty() || s0 < 0 {
            continue;
        }
        wings.push(Wing { squares, s0: s0 as usize, s1: s1 as usize, deep: (t1 - t0) as usize, far, t0, t1 });
        wing_tops.push(tops.get(pi).copied().unwrap_or(0) & 3);
    }
    let fore: Vec<usize> = (0..nx * ny).filter(|&k| piece[k] == Piece::Fore).collect();
    let wing_floors = (floors - 1).max(1);
    let fore_floors = floors.min(2);
    let overlap = (0..nx * ny)
        .filter_map(|k| {
            let top = match piece[k] {
                Piece::Wing(_) => wing_floors,
                Piece::Fore => fore_floors,
                _ => return None,
            };
            let (s, t) = f.local((k % nx) as isize, (k / nx) as isize);
            (s >= 0 && t >= 0 && (s as usize) < len && (t as usize) < wid).then_some((k, piece[k], top))
        })
        .collect();
    // Each tower rises a storey above the piece it stands on.
    let tower_top = (0..sh.towers.len())
        .map(|ti| {
            let Some(tp) = b.parts.iter().enumerate().filter(|(_, p)| matches!(p.kind, PartKind::Tower { .. })).nth(ti) else { return floors };
            let c = geom::mul(tp.1.poly.iter().fold([0.0, 0.0], |a, q| geom::add(a, *q)), 1.0 / tp.1.poly.len() as f64);
            let on = b.parts.iter().filter(|p| !matches!(p.kind, PartKind::Tower { .. })).max_by(|p, q| depth(&p.poly, c).total_cmp(&depth(&q.poly, c)));
            if on.is_some_and(|p| p.kind == PartKind::Wing) { wing_floors } else { floors }
        })
        .collect();
    let pending = if kind == Kind::Barracks { vec![] } else { vec!["great chamber", "solar", "treasury", "armory", "garrison"] };
    let bay = if bay { bay_t(&f, len, wid, &piece) } else { None };
    // (The chapel over the forebuilding's entrance, where it has two storeys.)
    let chapel = kind == Kind::Barracks || (!fore.is_empty() && fore_floors >= 2);
    Some(Plan {
        kind,
        nx,
        ny,
        f,
        piece,
        service,
        screens,
        dz,
        hall_t,
        land_t: (tl0, tl0 + lw),
        stairs,
        hall_z,
        open,
        floors,
        wing_floors,
        fore_floors,
        pieces,
        top,
        wing_tops,
        wings,
        fore,
        tower_top,
        tower_sq: sh.towers.iter().map(|t| t.0.clone()).collect(),
        overlap,
        ring: None,
        seed,
        pending,
        chapel,
        ends: [street_low == low_min, kind == Kind::Barracks || street_front == front_min],
        side: -1,
        oratory: false,
        last: Vec::new(),
        bay,
        small: false,
    })
}

/// A shell keep's plan: its ring of lean-tos 3 squares deep (a gallery and rooms 2 deep above)
/// round its round yard, a gate passage through them toward the motte path's gate (the ring's first
/// corner), the main stair in the ring a square in from the wall with a landing from it to the yard.
fn shell_plan(b: &town::Building, sh: &Shell, piece: Vec<Piece>, seed: u64, grid_p: &dyn Fn(P) -> P) -> Option<Plan> {
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
    })
}

/// A small tower keep's plan (a body under 7 squares long or 5 across, 4 by 4 at least, 3 storeys
/// or more): the main stair against its high end wall (or a square in from it) in a landing across
/// the end, the rest of each storey a room or
/// two off it: an entrance hall to the stair over the stores on the ground, the great hall above,
/// then the lord's rooms (a great chamber, a solar) each by the landing with a chapel, a treasury, a
/// garrison or a closet at the low end (`Plan::small_level`).
fn small_plan(b: &town::Building, sh: &Shell, piece: Vec<Piece>, f: Frame, seed: u64, front: bool) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let (len, wid) = (f.len, f.wid);
    let body = |s: isize, t: isize| {
        let (i, j) = f.grid(s, t);
        (i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && piece[j as usize * nx + i as usize] == Piece::Body).then(|| j as usize * nx + i as usize)
    };
    // The stair: two squares across the high end, as near its middle as are whole (a row in from
    // the end wall where the outer wall as drawn cuts the last; the landing then those two rows).
    let mid = (wid as isize - 2) / 2;
    let (s0, tc) = [len as isize - 1, len as isize - 2].into_iter().find_map(|s0| {
        (0..wid as isize - 1).filter(|&t| [t, t + 1].iter().all(|&u| body(s0, u).is_some_and(|k| sh.whole[k]))).min_by_key(|&t| ((t - mid).abs(), t)).map(|t| (s0, t))
    })?;
    let (a, bq) = (f.grid(s0, tc), f.grid(s0, tc + 1));
    let stairs = [a.0.min(bq.0) as usize, a.1.min(bq.1) as usize, (a.0 - bq.0).unsigned_abs() + 1, (a.1 - bq.1).unsigned_abs() + 1];
    Some(Plan {
        kind: Kind::Plain,
        nx,
        ny,
        f,
        piece,
        service: 0,
        screens: false,
        dz: s0 as usize,
        hall_t: (0, wid),
        land_t: (tc as usize, tc as usize + 2),
        stairs,
        hall_z: 1,
        open: false,
        floors: b.floors.max(1) as i8,
        wing_floors: 1,
        fore_floors: 1,
        pieces: false,
        top: Vec::new(),
        wing_tops: Vec::new(),
        wings: Vec::new(),
        fore: Vec::new(),
        tower_top: Vec::new(),
        tower_sq: Vec::new(),
        overlap: Vec::new(),
        ring: None,
        seed,
        pending: vec!["great chamber", "solar", "treasury", "garrison"],
        chapel: false,
        ends: [true, front],
        side: -1,
        oratory: false,
        last: Vec::new(),
        bay: None,
        small: true,
    })
}

/// How far `p` lies inside polygon `poly` (its nearest edge; negative outside).
fn depth(poly: &[P], p: P) -> f64 {
    let n = poly.len();
    let d = (0..n).map(|k| geom::seg_dist(p, poly[k], poly[(k + 1) % n])).fold(f64::MAX, f64::min);
    if geom::contains(poly, p) { d } else { -d }
}

/// Lengths summing to `len`: `n` pieces (fewer where `min` each won't fit) of at least `min`, by
/// heavy-tailed weights, so the rooms cut by them are of very different sizes.
fn cuts(len: usize, n: usize, min: usize, rng: &mut Pcg32) -> Vec<usize> {
    let n = n.min(len / min.max(1)).max(1);
    if len < min * n {
        return vec![len];
    }
    let w: Vec<f64> = (0..n).map(|_| 1.0 / (0.15 + 0.85 * rng.next_f64())).collect();
    let total: f64 = w.iter().sum();
    let spare = len - n * min;
    let raw: Vec<f64> = w.iter().map(|x| spare as f64 * x / total).collect();
    let mut out: Vec<usize> = raw.iter().map(|x| min + crate::core::floor(*x) as usize).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| (raw[b] - crate::core::floor(raw[b])).total_cmp(&(raw[a] - crate::core::floor(raw[a]))).then(a.cmp(&b)));
    let mut k = 0;
    while out.iter().sum::<usize>() < len {
        out[order[k % n]] += 1;
        k += 1;
    }
    out
}

/// A floor in the making.
#[derive(Clone)]
struct Lay {
    cells: Vec<i16>,
    rooms: Vec<Room>,
    hints: Hints,
    items: Vec<Item>,
    /// The ground floor's screens passage and hall (for the front door, once the forebuilding is
    /// laid).
    entry: Option<(i16, i16)>,
}

impl Lay {
    fn room(&mut self, kind: &'static str) -> i16 {
        self.rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        (self.rooms.len() - 1) as i16
    }

    fn count(&self, r: i16) -> usize {
        self.cells.iter().filter(|&&c| c == r).count()
    }
}

/// The door edge between two side-by-side grid squares (as `connect` names edges).
fn edge(a: (isize, isize), b: (isize, isize)) -> Option<(usize, usize, bool)> {
    let (x, y) = (a.0.min(b.0), a.1.min(b.1));
    if x < 0 || y < 0 || (a.0 - b.0).abs() + (a.1 - b.1).abs() != 1 {
        return None;
    }
    Some((x as usize, y as usize, a.1 == b.1))
}

/// The front door's side (0 top, 1 right, 2 bottom, 3 left) out of square `a` to square `b`.
fn side(a: (isize, isize), b: (isize, isize)) -> usize {
    match (b.0 - a.0, b.1 - a.1) {
        (0, -1) => 0,
        (1, 0) => 1,
        (0, 1) => 2,
        _ => 3,
    }
}

impl Plan {
    /// The levels: z, its storeys and its top (a keep of pieces), else its storeys (the top and
    /// tower tops as other buildings have them: `None` from `level`).
    pub fn reaches(&self) -> i8 {
        if self.pieces { self.floors } else { self.floors - 1 }
    }

    fn at(&self, s: isize, t: isize) -> Option<usize> {
        let (i, j) = self.f.grid(s, t);
        (i >= 0 && j >= 0 && (i as usize) < self.nx && (j as usize) < self.ny).then(|| j as usize * self.nx + i as usize)
    }

    fn body(&self, s: isize, t: isize) -> Option<usize> {
        self.at(s, t).filter(|&k| self.piece[k] == Piece::Body)
    }

    /// Body squares s0..s1, t0..t1 to room `r`.
    fn fill(&self, lay: &mut Lay, s0: usize, s1: usize, t0: usize, t1: usize, r: i16) {
        for s in s0..s1 {
            for t in t0..t1 {
                if let Some(k) = self.body(s as isize, t as isize) {
                    lay.cells[k] = r;
                }
            }
        }
    }

    /// How many body squares s0..s1, t0..t1 holds.
    fn squares(&self, s0: isize, s1: isize, t0: isize, t1: isize) -> usize {
        (s0..s1).map(|s| (t0..t1).filter(|&t| self.body(s, t).is_some()).count()).sum()
    }

    /// The body's squares s0..s1, t0..t1 no room has yet but `or`.
    fn open_squares(&self, lay: &Lay, s0: isize, s1: isize, (t0, t1): (isize, isize), or: i16) -> Vec<usize> {
        (s0..s1).flat_map(|s| (t0..t1).map(move |t| (s, t))).filter_map(|(s, t)| self.body(s, t)).filter(|&k| lay.cells[k] < 0 || lay.cells[k] == or).collect()
    }

    /// A hint: a door between room `a`'s square (s, t) and room `b`'s square beside it (body frame).
    fn door(&self, lay: &mut Lay, from: (isize, isize), to: (isize, isize)) {
        let (Some(ka), Some(kb)) = (self.at(from.0, from.1), self.at(to.0, to.1)) else { return };
        let (a, b) = (lay.cells[ka], lay.cells[kb]);
        if a < 0 || b < 0 || a == b {
            return;
        }
        if let Some(e) = edge(self.f.grid(from.0, from.1), self.f.grid(to.0, to.1)) {
            lay.hints.doors.push(((a.min(b), a.max(b)), e));
        }
    }

    /// Rooms cut along s from `s0..s1` (t0..t1 across): `n` of them by heavy-tailed lengths.
    fn strip(&self, lay: &mut Lay, s0: usize, s1: usize, t0: usize, t1: usize, n: usize, rng: &mut Pcg32) -> Vec<i16> {
        if s1 <= s0 || t1 <= t0 {
            return Vec::new();
        }
        let mut s = s0;
        let mut out = Vec::new();
        for l in cuts(s1 - s0, n, 2, rng) {
            let r = lay.room("chamber");
            self.fill(lay, s, s + l, t0, t1, r);
            out.push(r);
            s += l;
        }
        out
    }

    /// How many rooms a run of `len` squares holds (about one per 4 to 5).
    fn rooms_in(len: usize, rng: &mut Pcg32) -> usize {
        (crate::core::round(len as f64 / 4.5) as usize + (rng.next_f64() < 0.3) as usize).max(1)
    }

    /// Squares sound as a room: `least` or more, a 2 x 2 block among them, in one piece, no longer
    /// than 2.5 times their width by their box.
    fn sound(&self, sq: &[usize], least: usize) -> bool {
        let nx = self.nx;
        if sq.len() < least || sq.is_empty() {
            return false;
        }
        let has = |k: usize| sq.contains(&k);
        let block = sq.iter().any(|&k| k % nx + 1 < nx && has(k + 1) && has(k + nx) && has(k + nx + 1));
        let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
        let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut seen = vec![sq[0]];
        let mut i = 0;
        while i < seen.len() {
            let k = seen[i];
            i += 1;
            for q in [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx] {
                let adj = (q == k + 1 && k % nx + 1 < nx) || (q.wrapping_add(1) == k && k % nx > 0) || q == k + nx || q.wrapping_add(nx) == k;
                if adj && has(q) && !seen.contains(&q) {
                    seen.push(q);
                }
            }
        }
        block && seen.len() == sq.len() && 2 * w.max(h) <= 5 * w.min(h)
    }

    /// Room `r` cut across its longer side in two sound rooms (`sound`: the part keeping its kind
    /// `least` squares or more, the other, of kind `kind`, `other`), each a room from a way about the
    /// floor at most (on one, beside a room on one, or beside the other part on one); both on a way
    /// first, then the most even. The new room.
    fn cut(&self, lay: &mut Lay, r: i16, least: usize, other: usize, kind: &'static str) -> Option<i16> {
        self.cut_avoiding(lay, r, least, other, kind, 0)
    }

    /// `cut`, neither part of `avoid` squares (0: any).
    fn cut_avoiding(&self, lay: &mut Lay, r: i16, least: usize, other: usize, kind: &'static str, avoid: usize) -> Option<i16> {
        let (nx, ny) = (self.nx, self.ny);
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        let st = self.stairs;
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        let on_way = |lay: &Lay, sq: &[usize]| sq.iter().any(|&k| nbrs(k).any(|q| on_stairs(q) || (lay.cells[q] >= 0 && is_circulation(lay.rooms[lay.cells[q] as usize].kind))));
        // (Beside a room other than `r` that lies on a way: a room from one.)
        let by_way = |lay: &Lay, sq: &[usize]| {
            let mut seen: Vec<i16> = Vec::new();
            sq.iter().any(|&k| {
                nbrs(k).any(|q| {
                    let o = lay.cells[q];
                    if o < 0 || o == r || seen.contains(&o) || is_circulation(lay.rooms[o as usize].kind) || !super::walkable(lay.rooms[o as usize].kind) {
                        return false;
                    }
                    seen.push(o);
                    let osq: Vec<usize> = (0..nx * ny).filter(|&p| lay.cells[p] == o).collect();
                    super::is_way_through(lay.rooms[o as usize].kind) || on_way(lay, &osq)
                })
            })
        };
        let sq: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == r).collect();
        let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
        let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
        let along_x = x1 - x0 > y1 - y0;
        let at = |k: usize| if along_x { k % nx - x0 } else { k / nx - y0 };
        let long = if along_x { x1 - x0 + 1 } else { y1 - y0 + 1 };
        let best = (1..long)
            .filter_map(|c| {
                let (a, b): (Vec<usize>, Vec<usize>) = sq.iter().partition(|&&k| at(k) < c);
                // (The part keeping its kind: the one on a way, else the bigger.)
                let keep_a = on_way(lay, &a) && (!on_way(lay, &b) || a.len() >= b.len());
                let (k, o) = if keep_a { (&a, &b) } else { (&b, &a) };
                let near = |p: &Vec<usize>, q: &Vec<usize>| on_way(lay, p) || by_way(lay, p) || on_way(lay, q);
                (self.sound(k, least) && self.sound(o, other) && near(k, o) && near(o, k) && a.len() != avoid && b.len() != avoid).then(|| ((on_way(lay, k) && on_way(lay, o)) as usize, std::cmp::Reverse(a.len().abs_diff(b.len())), c, keep_a))
            })
            .max_by(|p, q| (p.0, p.1).cmp(&(q.0, q.1)).then(q.2.cmp(&p.2)))?;
        let (_, _, c, keep_a) = best;
        let id = lay.room(kind);
        for &k in &sq {
            if (at(k) < c) != keep_a {
                lay.cells[k] = id;
            }
        }
        Some(id)
    }

    /// Rooms of sound proportions on a storey above the hall: a room (not a way about the floor, a
    /// hall or a top) longer than 2.5 times its width, or over 40 squares (a great chamber 60), cut
    /// across into rooms that are not, each of 4
    /// squares or more with a 2 x 2 block, in one piece, the part by a way about the floor (or the
    /// stairs) keeping its kind (a great chamber or a solar 9 squares at least), the rest chambers
    /// (`free`); a room filling under 60% of its box (a tail a square wide off it) giving the squares
    /// outside its largest box to a way about the floor beside them, else to a room beside them that
    /// stays sound. Cuts first where both parts lie on a way.
    fn tidy(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        let fixed = |kind: &str| is_circulation(kind) || !super::walkable(kind) || matches!(kind, "great hall" | "mess hall" | "battlements" | "wall walk");
        let lord = |kind: &str| matches!(kind, "great chamber" | "solar");
        let boxed = |sq: &[usize]| {
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
        };
        for _ in 0..3 * lay.rooms.len() {
            let mut changed = false;
            for r in 0..lay.rooms.len() as i16 {
                let kind = lay.rooms[r as usize].kind;
                let sq: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == r).collect();
                if sq.len() < 5 || fixed(kind) {
                    continue;
                }
                let (x0, y0, w, h) = boxed(&sq);
                let least = if lord(kind) { 9 } else { 4 };
                let cap = if kind == "great chamber" { 60 } else { 40 };
                if (2 * w.max(h) > 5 * w.min(h) && sq.len() >= 8) || sq.len() > cap {
                    if let Some(id) = self.cut(lay, r, least, 4, "chamber") {
                        free.push(id);
                        changed = true;
                    }
                    continue;
                }
                let sound = |sq: &[usize], lmin: usize| self.sound(sq, lmin);
                if sq.len() * 5 >= w * h * 3 {
                    continue;
                }
                // A tail: the squares outside the room's largest box.
                let has = |i: usize, j: usize| i >= x0 && j >= y0 && i < x0 + w && j < y0 + h && lay.cells[j * nx + i] == r;
                let mut big = (0, [0usize; 4]);
                for b0 in 0..h {
                    for b1 in b0 + 1..=h {
                        for a0 in 0..w {
                            for a1 in a0 + 1..=w {
                                let n = (a1 - a0) * (b1 - b0);
                                if n > big.0 && (b0..b1).all(|j| (a0..a1).all(|i| has(x0 + i, y0 + j))) {
                                    big = (n, [a0, a1, b0, b1]);
                                }
                            }
                        }
                    }
                }
                let [a0, a1, b0, b1] = big.1;
                let core: Vec<usize> = sq.iter().copied().filter(|&k| (a0..a1).contains(&(k % nx - x0)) && (b0..b1).contains(&(k / nx - y0))).collect();
                if !sound(&core, least) {
                    continue;
                }
                // (Each piece of the tail to a way beside it, else the room beside it that stays
                // sound.)
                let mut tail: Vec<usize> = sq.iter().copied().filter(|k| !core.contains(k)).collect();
                while let Some(&k0) = tail.first() {
                    let mut part = vec![k0];
                    let mut i = 0;
                    while i < part.len() {
                        let k = part[i];
                        i += 1;
                        for q in nbrs(k) {
                            if tail.contains(&q) && !part.contains(&q) {
                                part.push(q);
                            }
                        }
                    }
                    tail.retain(|k| !part.contains(k));
                    let mut by: Vec<(usize, i16)> = Vec::new();
                    for &k in &part {
                        for q in nbrs(k) {
                            let o = lay.cells[q];
                            if o >= 0 && o != r && super::walkable(lay.rooms[o as usize].kind) && !matches!(lay.rooms[o as usize].kind, "great hall" | "mess hall") {
                                match by.iter_mut().find(|x| x.1 == o) {
                                    Some(x) => x.0 += 1,
                                    None => by.push((1, o)),
                                }
                            }
                        }
                    }
                    by.sort_by(|a, b| {
                        let way = |o: i16| is_circulation(lay.rooms[o as usize].kind) && lay.rooms[o as usize].kind != "landing";
                        (way(b.1), b.0, std::cmp::Reverse(b.1)).cmp(&(way(a.1), a.0, std::cmp::Reverse(a.1)))
                    });
                    let to = by.iter().map(|x| x.1).find(|&o| {
                        let k = lay.rooms[o as usize].kind;
                        if is_circulation(k) && k != "landing" {
                            return true;
                        }
                        let mut grown: Vec<usize> = (0..nx * ny).filter(|&q| lay.cells[q] == o).collect();
                        grown.extend(part.iter().copied());
                        let (_, _, gw, gh) = boxed(&grown);
                        !fixed(k) && sound(&grown, 4) && grown.len() * 5 >= gw * gh * 3
                    });
                    // (Nowhere: a tail a square wide, a tower's way in or the like, a mural passage of its
                    // own.)
                    let wide = part.iter().any(|&k| k % nx + 1 < nx && [k + 1, k + nx, k + nx + 1].iter().all(|q| part.contains(q)));
                    let to = to.or_else(|| (!wide && part.len() >= 2).then(|| lay.room("mural passage")));
                    if let Some(o) = to {
                        for &k in &part {
                            lay.cells[k] = o;
                        }
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// The great chamber of good size by the main stair (9 to 60 squares, a square within 2 of it,
    /// of sound proportions): one too small made one with a room beside it where the two make one
    /// such, else changing places with the room of the storey nearest the stair that is one, of a
    /// kind any room may be.
    fn great_chamber(&mut self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let st = self.stairs;
        let stc = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
        let gap = |sq: &[usize]| {
            sq.iter()
                .map(|&k| {
                    let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                    let dx = (st[0] as isize - i - 1).max(i - (st[0] + st[2]) as isize).max(0);
                    let dy = (st[1] as isize - j - 1).max(j - (st[1] + st[3]) as isize).max(0);
                    dx.max(dy)
                })
                .min()
                .unwrap_or(isize::MAX)
        };
        let fill = |sq: &[usize]| {
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            sq.len() * 5 >= (x1 - x0 + 1) * (y1 - y0 + 1) * 3
        };
        let good = |sq: &[usize]| sq.len() <= 60 && self.sound(sq, 9) && fill(sq) && gap(sq) <= 2;
        let squares = |lay: &Lay, r: i16| -> Vec<usize> { (0..nx * ny).filter(|&k| lay.cells[k] == r).collect() };
        let Some(g) = (0..lay.rooms.len() as i16).find(|&r| lay.rooms[r as usize].kind == "great chamber" && lay.count(r) > 0) else { return };
        let gsq = squares(lay, g);
        if good(&gsq) {
            return;
        }
        let swappable = |kind: &str| matches!(kind, "chamber" | "guest chamber" | "study" | "wardrobe" | "storeroom" | "servants' quarters" | "map room" | "library");
        let mid = |sq: &[usize]| [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64];
        // (Made one with a room beside it.)
        let beside: Vec<i16> = (0..lay.rooms.len() as i16)
            .filter(|&o| o != g && swappable(lay.rooms[o as usize].kind) && gsq.iter().any(|&k| [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx].iter().any(|&q| q < nx * ny && lay.cells[q] == o)))
            .collect();
        let merged = beside
            .iter()
            .filter_map(|&o| {
                let mut u = gsq.clone();
                u.extend(squares(lay, o));
                good(&u).then(|| (u.len(), o))
            })
            .min();
        if let Some((_, o)) = merged {
            for c in lay.cells.iter_mut() {
                if *c == o {
                    *c = g;
                }
            }
            return;
        }
        let best = (0..lay.rooms.len() as i16)
            .filter(|&o| swappable(lay.rooms[o as usize].kind) && good(&squares(lay, o)))
            .min_by(|&a, &b| geom::dist(mid(&squares(lay, a)), stc).total_cmp(&geom::dist(mid(&squares(lay, b)), stc)).then(a.cmp(&b)));
        if let Some(o) = best {
            let was = lay.rooms[o as usize].kind;
            lay.rooms[o as usize].kind = "great chamber";
            lay.rooms[g as usize].kind = was;
        }
    }

    /// A great chamber or a solar the body's rooms don't hold where it is due (none still to be
    /// placed, or one under 9 squares): the room of a tower standing on the storey, 9 squares or
    /// more by the main stair (a square within 2 of it; a solar's middle nearer the stair than the
    /// hall's low end), the nearest; the body's small one a chamber.
    fn lords_in_towers(&mut self, lay: &mut Lay, towers: &mut [(usize, &'static str)]) {
        let nx = self.nx;
        let st = self.stairs;
        let stc = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
        let low = self.low_end();
        let gap = |sq: &[usize]| {
            sq.iter()
                .map(|&k| {
                    let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                    let dx = (st[0] as isize - i - 1).max(i - (st[0] + st[2]) as isize).max(0);
                    let dy = (st[1] as isize - j - 1).max(j - (st[1] + st[3]) as isize).max(0);
                    dx.max(dy)
                })
                .min()
                .unwrap_or(isize::MAX)
        };
        let mid = |sq: &[usize]| [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64];
        for kind in ["great chamber", "solar"] {
            let room = (0..lay.rooms.len() as i16).find(|&r| lay.rooms[r as usize].kind == kind && lay.count(r) > 0);
            // (Too small, or with a tail its box under 60% full.)
            let poor = |r: i16| {
                let sq: Vec<usize> = (0..lay.cells.len()).filter(|&k| lay.cells[k] == r).collect();
                let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
                let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
                sq.len() < 9 || sq.len() * 5 < (x1 - x0 + 1) * (y1 - y0 + 1) * 3
            };
            if !(room.is_some_and(poor) || (room.is_none() && self.pending.contains(&kind))) {
                continue;
            }
            let best = (0..towers.len())
                .filter(|&i| {
                    let sq = &self.tower_sq[towers[i].0];
                    !matches!(towers[i].1, "great chamber" | "solar" | "oratory") && sq.len() >= 9 && gap(sq) <= 2 && (kind != "solar" || low.is_none_or(|lo| geom::dist(mid(sq), stc) <= geom::dist(mid(sq), lo)))
                })
                .min_by(|&a, &b| geom::dist(mid(&self.tower_sq[towers[a].0]), stc).total_cmp(&geom::dist(mid(&self.tower_sq[towers[b].0]), stc)).then(a.cmp(&b)));
            if let Some(i) = best {
                towers[i].1 = kind;
                if let Some(r) = room {
                    lay.rooms[r as usize].kind = "chamber";
                    // (Its tail, now a chamber's, given away.)
                    self.tidy(lay, &mut Vec::new());
                }
                self.pending.retain(|k| *k != kind);
            }
        }
    }

    /// A storey of five rooms or more (a floor's free ones, `free`) over half of them of one size:
    /// two of that size side by side made one where that is sound (40 squares at most, no longer
    /// than 2.5:1, its box 60% full), else cut again where neither is of that size, until it isn't
    /// so.
    fn vary(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        for _ in 0..free.len() {
            let live: Vec<i16> = free.iter().copied().filter(|&r| lay.count(r) > 0).collect();
            let sizes: Vec<usize> = live.iter().map(|&r| lay.count(r)).collect();
            let Some(&most) = sizes.iter().max_by_key(|&&s| (sizes.iter().filter(|&&t| t == s).count(), s)) else { return };
            if sizes.len() < 5 || 2 * sizes.iter().filter(|&&t| t == most).count() <= sizes.len() {
                return;
            }
            let same: Vec<i16> = live.iter().copied().filter(|&r| lay.count(r) == most).collect();
            let pair = same.iter().flat_map(|&a| same.iter().map(move |&b| (a, b))).filter(|&(a, b)| a < b).find(|&(a, b)| {
                let mut u: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == a || lay.cells[k] == b).collect();
                u.sort();
                let (x0, x1) = u.iter().fold((usize::MAX, 0), |m, &k| (m.0.min(k % nx), m.1.max(k % nx)));
                let (y0, y1) = u.iter().fold((usize::MAX, 0), |m, &k| (m.0.min(k / nx), m.1.max(k / nx)));
                u.len() <= 40 && self.sound(&u, 4) && u.len() * 5 >= (x1 - x0 + 1) * (y1 - y0 + 1) * 3
            });
            if let Some((a, b)) = pair {
                for c in lay.cells.iter_mut() {
                    if *c == b {
                        *c = a;
                    }
                }
                free.retain(|&f| f != b);
                continue;
            }
            // (Else two side by side cut again otherwise, neither of that size.)
            let adjacent = |lay: &Lay, a: i16, b: i16| (0..nx * ny).any(|k| lay.cells[k] == a && [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx].iter().any(|&q| q < nx * ny && lay.cells[q] == b));
            let mut done = false;
            for (a, b) in same.iter().flat_map(|&a| same.iter().map(move |&b| (a, b))).filter(|&(a, b)| a < b) {
                if !adjacent(lay, a, b) {
                    continue;
                }
                let (cells, kind) = (lay.cells.clone(), lay.rooms[b as usize].kind);
                for c in lay.cells.iter_mut() {
                    if *c == b {
                        *c = a;
                    }
                }
                match self.cut_avoiding(lay, a, 4, 4, kind, most) {
                    Some(id) => {
                        // (The new part takes `b`'s place among the floor's rooms.)
                        for c in lay.cells.iter_mut() {
                            if *c == id {
                                *c = b;
                            }
                        }
                        lay.rooms.pop();
                        done = true;
                        break;
                    }
                    None => lay.cells = cells,
                }
            }
            if !done {
                return;
            }
        }
    }

    /// A landing of more than 12 squares (squares no zone took made part of it): those outside its
    /// own (from the dais end's row across its columns) to the room beside them that stays sound,
    /// else a passage beside them.
    fn trim_landings(&self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        for r in 0..lay.rooms.len() as i16 {
            if lay.rooms[r as usize].kind != "landing" || lay.count(r) <= 12 {
                continue;
            }
            let (tl0, tl1) = if self.small { (0, self.f.wid as isize) } else { (self.land_t.0 as isize, self.land_t.1 as isize) };
            let own = |k: usize| {
                let (s, t) = self.f.local((k % nx) as isize, (k / nx) as isize);
                s >= self.dz as isize && t >= tl0 && t < tl1
            };
            let mut extra: Vec<usize> = (0..nx * ny).filter(|&k| lay.cells[k] == r && !own(k)).collect();
            while let Some(&k0) = extra.first() {
                let mut part = vec![k0];
                let mut i = 0;
                while i < part.len() {
                    let k = part[i];
                    i += 1;
                    for q in nbrs(k) {
                        if extra.contains(&q) && !part.contains(&q) {
                            part.push(q);
                        }
                    }
                }
                extra.retain(|k| !part.contains(k));
                let mut by: Vec<(usize, i16)> = Vec::new();
                for &k in &part {
                    for q in nbrs(k) {
                        let o = lay.cells[q];
                        if o >= 0 && o != r && super::walkable(lay.rooms[o as usize].kind) && !matches!(lay.rooms[o as usize].kind, "great hall" | "mess hall" | "landing") {
                            match by.iter_mut().find(|x| x.1 == o) {
                                Some(x) => x.0 += 1,
                                None => by.push((1, o)),
                            }
                        }
                    }
                }
                by.sort_by(|a, b| (b.0, std::cmp::Reverse(b.1)).cmp(&(a.0, std::cmp::Reverse(a.1))));
                let to = by.iter().map(|x| x.1).find(|&o| {
                    if is_circulation(lay.rooms[o as usize].kind) {
                        return true;
                    }
                    let mut grown: Vec<usize> = (0..nx * ny).filter(|&q| lay.cells[q] == o).collect();
                    grown.extend(part.iter().copied());
                    let (x0, x1) = grown.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
                    let (y0, y1) = grown.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
                    self.sound(&grown, 4) && grown.len() <= 40 && grown.len() * 5 >= (x1 - x0 + 1) * (y1 - y0 + 1) * 3
                });
                // (Nowhere, a strip a square wide: a mural passage of its own.)
                let wide = part.iter().any(|&k| k % nx + 1 < nx && [k + 1, k + nx, k + nx + 1].iter().all(|q| part.contains(q)));
                let to = to.or_else(|| (!wide).then(|| lay.room("mural passage")));
                if let Some(o) = to.filter(|_| lay.count(r) > 12) {
                    for &k in &part {
                        lay.cells[k] = o;
                    }
                }
            }
        }
    }

    /// Doors that keep every room within a room of a way about the floor (a passage, a landing, a
    /// hall, a walk, a guardroom; the corridor rule): a room beside one has a door onto it; one
    /// beside none a door into the room beside it that is beside one (the one they share the most
    /// wall with).
    fn near_links(&self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let n = lay.rooms.len();
        let mut shared: Vec<Vec<(i16, usize)>> = vec![Vec::new(); n];
        for k in 0..nx * ny {
            let a = lay.cells[k];
            if a < 0 || !super::walkable(lay.rooms[a as usize].kind) {
                continue;
            }
            for q in [(k % nx + 1 < nx).then_some(k + 1), (k / nx + 1 < ny).then_some(k + nx)].into_iter().flatten() {
                let b = lay.cells[q];
                if b < 0 || b == a || !super::walkable(lay.rooms[b as usize].kind) {
                    continue;
                }
                for (x, y) in [(a, b), (b, a)] {
                    match shared[x as usize].iter_mut().find(|e| e.0 == y) {
                        Some(e) => e.1 += 1,
                        None => shared[x as usize].push((y, 1)),
                    }
                }
            }
        }
        let st = self.stairs;
        let start = lay.cells[st[1] * nx + st[0]];
        let way = |r: i16| super::is_way_through(lay.rooms[r as usize].kind) || r == start;
        let near: Vec<bool> = (0..n as i16).map(|r| !way(r) && shared[r as usize].iter().any(|e| way(e.0))).collect();
        let mut add = Vec::new();
        for r in (0..n as i16).filter(|&r| lay.count(r) > 0 && !way(r)) {
            let pick = if near[r as usize] {
                // (Onto a way that isn't a passage, which every room beside one opens onto anyway.)
                shared[r as usize].iter().filter(|e| way(e.0) && !is_circulation(lay.rooms[e.0 as usize].kind)).max_by_key(|e| (e.1, std::cmp::Reverse(e.0))).filter(|_| !shared[r as usize].iter().any(|e| is_circulation(lay.rooms[e.0 as usize].kind)))
            } else {
                shared[r as usize].iter().filter(|e| near[e.0 as usize]).max_by_key(|e| (e.1, std::cmp::Reverse(e.0)))
            };
            if let Some(e) = pick {
                add.push((r.min(e.0), r.max(e.0)));
            }
        }
        for p in add {
            if !lay.hints.link.contains(&p) {
                lay.hints.link.push(p);
            }
        }
    }

    /// The hall's low end (grid): the middle of the screens passage, else of the service end; None
    /// without either.
    fn low_end(&self) -> Option<P> {
        let rows: Vec<isize> = if self.screens { vec![self.service as isize] } else { (0..self.service as isize).collect() };
        let sq: Vec<(isize, isize)> = rows.iter().flat_map(|&s| (0..self.f.wid as isize).filter(move |&t| self.body(s, t).is_some()).map(move |t| self.f.grid(s, t))).collect();
        (!sq.is_empty()).then(|| [sq.iter().map(|q| q.0 as f64 + 0.5).sum::<f64>() / sq.len() as f64, sq.iter().map(|q| q.1 as f64 + 0.5).sum::<f64>() / sq.len() as f64])
    }

    /// The solar where the lord's rooms go: by the main stair at the dais end (a square within 2 of
    /// it, its middle nearer the stair than the hall's low end), 9 squares or more, of sound
    /// proportions. A solar not so (or none yet, where one is due on this storey: it is still to be
    /// placed) changes places with the room of the storey nearest the stair that is, of a kind any
    /// room may be (a great chamber too, where the solar's room will do for one; the chapel but over
    /// a forebuilding); with none, a great chamber big enough for both is cut in two.
    fn solar(&mut self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let st = self.stairs;
        let stc = [st[0] as f64 + 0.5 * st[2] as f64, st[1] as f64 + 0.5 * st[3] as f64];
        let low = self.low_end();
        let shape = |sq: &[usize]| {
            let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            2 * w.max(h) <= 5 * w.min(h) && sq.len() * 5 >= w * h * 3
        };
        let gap = |sq: &[usize]| {
            sq.iter()
                .map(|&k| {
                    let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                    let dx = (st[0] as isize - i - 1).max(i - (st[0] + st[2]) as isize).max(0);
                    let dy = (st[1] as isize - j - 1).max(j - (st[1] + st[3]) as isize).max(0);
                    dx.max(dy)
                })
                .min()
                .unwrap_or(isize::MAX)
        };
        let mid = |sq: &[usize]| [sq.iter().map(|&k| (k % nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64, sq.iter().map(|&k| (k / nx) as f64 + 0.5).sum::<f64>() / sq.len().max(1) as f64];
        let near = |sq: &[usize]| low.is_none_or(|lo| geom::dist(mid(sq), stc) <= geom::dist(mid(sq), lo));
        let good = |sq: &[usize]| sq.len() >= 9 && shape(sq) && gap(sq) <= 2 && near(sq);
        let squares = |lay: &Lay, r: i16| -> Vec<usize> { (0..nx * ny).filter(|&k| lay.cells[k] == r).collect() };
        let rooms: Vec<(i16, Vec<usize>)> = (0..lay.rooms.len() as i16).map(|r| (r, squares(lay, r))).filter(|x| !x.1.is_empty()).collect();
        let solar = rooms.iter().find(|x| lay.rooms[x.0 as usize].kind == "solar").cloned();
        if solar.as_ref().is_some_and(|s| good(&s.1)) || (solar.is_none() && !self.pending.contains(&"solar")) {
            return;
        }
        // (The chapel too, but over a forebuilding's entrance.)
        let swappable = |kind: &str, sq: &[usize]| {
            matches!(kind, "chamber" | "guest chamber" | "study" | "wardrobe" | "storeroom" | "servants' quarters" | "map room" | "library" | "great chamber")
                || (kind == "chapel" && !sq.iter().any(|k| self.fore.contains(k)))
        };
        let best = rooms
            .iter()
            .filter(|x| swappable(lay.rooms[x.0 as usize].kind, &x.1) && good(&x.1))
            .filter(|x| {
                // (A great chamber gives its place only for one that will do for it.)
                lay.rooms[x.0 as usize].kind != "great chamber" || solar.as_ref().is_some_and(|s| s.1.len() >= 9 && s.1.len() <= 60 && gap(&s.1) <= 2 && shape(&s.1))
            })
            .min_by(|a, b| geom::dist(mid(&a.1), stc).total_cmp(&geom::dist(mid(&b.1), stc)).then(a.0.cmp(&b.0)));
        let Some((o, _)) = best.cloned() else {
            // (None will do: a smaller room by the stair grown by the squares of a room beside it
            // that border it, where both stay sound (a great chamber 9 squares, by the stair); else a
            // great chamber of 18 squares or more cut in two, the part that will do for it the solar,
            // a solar that wouldn't a chamber.)
            let fill = |sq: &[usize]| {
                let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
                let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
                sq.len() * 5 >= (x1 - x0 + 1) * (y1 - y0 + 1) * 3
            };
            let mut grown: Option<(f64, i16, i16, Vec<usize>)> = None;
            for (r, rsq) in rooms.iter().filter(|x| (swappable(lay.rooms[x.0 as usize].kind, &x.1) || lay.rooms[x.0 as usize].kind == "solar") && lay.rooms[x.0 as usize].kind != "great chamber" && x.1.len() >= 4 && x.1.len() < 9) {
                for (o, osq) in rooms.iter().filter(|x| x.0 != *r && (swappable(lay.rooms[x.0 as usize].kind, &x.1) || lay.rooms[x.0 as usize].kind == "great chamber")) {
                    let strip: Vec<usize> = osq.iter().copied().filter(|&k| [k.wrapping_sub(1), k + 1, k.wrapping_sub(nx), k + nx].iter().any(|q| rsq.contains(q))).collect();
                    let mut u = rsq.clone();
                    u.extend(strip.iter().copied());
                    let rest: Vec<usize> = osq.iter().copied().filter(|k| !strip.contains(k)).collect();
                    let gc = lay.rooms[*o as usize].kind == "great chamber";
                    let rest_ok = self.sound(&rest, if gc { 9 } else { 4 }) && fill(&rest) && (!gc || gap(&rest) <= 2);
                    if !strip.is_empty() && good(&u) && self.sound(&u, 9) && rest_ok {
                        let d = geom::dist(mid(&u), stc);
                        if grown.as_ref().is_none_or(|x| d < x.0) {
                            grown = Some((d, *r, *o, strip));
                        }
                    }
                }
            }
            if let Some((_, r, _, strip)) = grown {
                for k in strip {
                    lay.cells[k] = r;
                }
                let was = lay.rooms[r as usize].kind;
                lay.rooms[r as usize].kind = "solar";
                if let Some((s, _)) = solar.filter(|x| x.0 != r) {
                    lay.rooms[s as usize].kind = was;
                }
                return;
            }
            let Some((g, gsq)) = rooms.iter().find(|x| lay.rooms[x.0 as usize].kind == "great chamber" && x.1.len() >= 18).cloned() else { return };
            let (x0, x1) = gsq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (y0, y1) = gsq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            let mut cut: Option<(f64, Vec<usize>)> = None;
            for along_x in [true, false] {
                let (lo, hi) = if along_x { (x0, x1) } else { (y0, y1) };
                for c in lo + 1..=hi {
                    let (a, b): (Vec<usize>, Vec<usize>) = gsq.iter().partition(|&&k| (if along_x { k % nx } else { k / nx }) < c);
                    for (sol, rest) in [(&a, &b), (&b, &a)] {
                        if good(sol) && rest.len() >= 9 && shape(rest) && gap(rest) <= 2 {
                            let d = geom::dist(mid(sol), stc);
                            if cut.as_ref().is_none_or(|x| d < x.0) {
                                cut = Some((d, sol.clone()));
                            }
                        }
                    }
                }
            }
            if let Some((_, sol)) = cut {
                let id = lay.room("solar");
                for k in sol {
                    lay.cells[k] = id;
                }
                if let Some((s, _)) = solar {
                    lay.rooms[s as usize].kind = "chamber";
                }
                let _ = g;
            }
            return;
        };
        let was = lay.rooms[o as usize].kind;
        lay.rooms[o as usize].kind = "solar";
        if let Some((s, _)) = solar {
            lay.rooms[s as usize].kind = was;
        }
    }

    /// Room kinds to the floor's free rooms, the biggest kinds to the biggest rooms; rooms left
    /// over take `fill` in turn.
    fn assign(lay: &mut Lay, free: &[i16], mut kinds: Kinds, fill: &[&'static str]) {
        let mut rooms: Vec<(usize, i16)> = free.iter().map(|&r| (lay.count(r), r)).filter(|x| x.0 > 0).collect();
        rooms.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        kinds.truncate(rooms.len());
        let mut by_weight: Vec<usize> = (0..kinds.len()).collect();
        by_weight.sort_by(|&a, &b| kinds[b].1.total_cmp(&kinds[a].1).then(a.cmp(&b)));
        for (n, &(_, r)) in rooms.iter().enumerate() {
            lay.rooms[r as usize].kind = match by_weight.get(n) {
                Some(&k) => kinds[k].0,
                None => fill[(n - by_weight.len()) % fill.len().max(1)],
            };
        }
    }

    /// Floor `z` laid out by zones, or None where the shared code lays it out (the cellar; a
    /// barracks' and a redrawn keep's roof).
    pub fn level(&mut self, z: i8) -> Option<Laid> {
        if z < 0 || z > self.reaches() {
            return None;
        }
        let mut rng = Pcg32::new(hash3(self.seed, z as i64, 0x5a, 0), 83);
        // (Above a wing's or the forebuilding's top, its squares on the body's edge are the body's.)
        for &(k, p, top) in &self.overlap {
            self.piece[k] = if z > top { Piece::Body } else { p };
        }
        let blank = Lay { cells: vec![-1; self.nx * self.ny], rooms: Vec::new(), hints: Hints::default(), items: Vec::new(), entry: None };
        if self.kind == Kind::ShellKeep {
            return self.shell_level(z, blank);
        }
        if self.small {
            return Some(self.small_level(z, blank));
        }
        let barracks = self.kind == Kind::Barracks;
        // (An upper floor with half its rooms or more on the squares of the storey below's: laid
        // out again, divided into more rooms.)
        let (pending, side) = (self.pending.clone(), self.side);
        let mut more = 0;
        let (mut lay, free, kinds, fill, public, roof, name) = loop {
            (self.pending, self.side) = (pending.clone(), side);
            let mut lay = blank.clone();
            let mut free: Vec<i16> = Vec::new();
            let mut kinds: Kinds = Vec::new();
            let mut fill: &[&'static str] = &["chamber", "guest chamber", "storeroom", "servants' quarters"];
            let mut public = None;
            let mut roof = false;
            let mut upper = false;
            let name: &'static str;
            if self.pieces && z == self.floors {
                self.top_level(&mut lay);
                roof = true;
                name = if self.top.iter().any(|t| t.code != 0) { "Roof" } else { "Battlements" };
            } else if z == self.hall_z {
                free = self.hall_floor(&mut lay, z == 0, &mut rng);
                public = Some(if barracks { "mess hall" } else { "great hall" });
                name = if barracks { "Mess" } else { "Hall" };
                kinds = if barracks { vec![("barrack room", 1.2), ("storeroom", 1.0)] } else { vec![("chamber", 1.4), ("guest chamber", 1.1), ("guardroom", 1.0), ("wardrobe", 0.6)] };
                // (No service end: its rooms the floor's first; one too small for more than its
                // kitchen: a pantry the first.)
                if self.service == 0 && !barracks {
                    kinds.splice(0..0, [("kitchen", 2.0), ("buttery", 1.8)]);
                } else if !barracks && !lay.rooms.iter().any(|r| r.kind == "buttery" || r.kind == "pantry") {
                    kinds.insert(0, ("pantry", 1.9));
                }
                fill = if barracks { &["barrack room", "storeroom"] } else { &["chamber", "storeroom", "guest chamber"] };
            } else if z < self.hall_z {
                free = self.stores_floor(&mut lay, &mut rng);
                name = "Stores";
                kinds = vec![("storeroom", 1.6), ("armory", 1.2), ("well room", 0.7), ("guardroom", 1.0), ("wine cellar", 0.9), ("storeroom", 1.1)];
                fill = &["storeroom"];
                self.pending.retain(|k| *k != "armory");
            } else {
                let n = (z - self.hall_z) as usize;
                // (Over a hall open two storeys, its open height, unless no long wall runs on from
                // the gallery to the dais end for the passage to it.)
                let gallery = if self.open && n == 1 { self.void_floor(&mut lay, &mut rng, more) } else { None };
                let lord = !barracks && self.pending.iter().any(|k| matches!(*k, "great chamber" | "solar"));
                let need = if barracks { 4 } else { self.upper_kinds(n, z + 1 == self.floors).1 + 1 };
                if let Some(g) = gallery {
                    free = g;
                    name = "Gallery";
                } else {
                    free = self.upper_floor(&mut lay, &mut rng, lord, need + more, more);
                    name = if lord { "Lord's floor" } else { "Upper floor" };
                }
                upper = true;
                if barracks {
                    kinds = vec![("barrack room", 2.0), ("barrack room", 1.7), ("armory", 1.0), ("officers' quarters", 1.2), ("barrack room", 1.4), ("storeroom", 0.8)];
                    fill = &["barrack room", "storeroom"];
                } else {
                    kinds = self.upper_kinds(n, z + 1 == self.floors).0;
                }
            }
            // Wings and the forebuilding, as high as each rises.
            for wi in 0..self.wings.len() {
                if z < self.wing_floors {
                    free.extend(self.wing_storey(&mut lay, wi, more % 2 == 1, &mut rng));
                } else if z == self.wing_floors {
                    self.wing_top(&mut lay, wi);
                }
            }
            if !self.fore.is_empty() {
                self.fore_level(&mut lay, z);
            }
            if let Some((sc, hall)) = lay.entry {
                self.front(&mut lay, sc, hall);
            }
            // (Squares no zone took, and rooms the towers left too small or a square wide: the room
            // beside them, a passage only where no room is; rooms no door could bring within a room
            // of a way about the floor: the room beside them too.)
            self.leftovers(&mut lay, &mut free);
            self.absorb(&mut lay, &mut free, 4);
            self.near_ways(&mut lay, &mut free);
            if upper {
                self.tidy(&mut lay, &mut free);
                self.vary(&mut lay, &mut free);
            }
            self.trim_landings(&mut lay);
            if !upper || more == 5 || !self.repeats(&lay) {
                break (lay, free, kinds, fill, public, roof, name);
            }
            more += 1;
        };
        Self::assign(&mut lay, &free, kinds, fill);
        // (A hall with no service end: its kitchen, where it has room, cut for a buttery too.)
        if z == self.hall_z && self.service == 0 && !barracks && !lay.rooms.iter().any(|r| r.kind == "buttery" || r.kind == "pantry") {
            if let Some(k) = (0..lay.rooms.len() as i16).find(|&r| lay.rooms[r as usize].kind == "kitchen" && lay.count(r) >= 9) {
                self.cut(&mut lay, k, 5, 4, "buttery");
            }
        }
        // (Above the hall, the great chamber and the solar where the lord's rooms go.)
        if z > self.hall_z && !(self.pieces && z == self.floors) && !barracks {
            self.great_chamber(&mut lay);
            self.solar(&mut lay);
            // (A room that gave up a lord's kind with a tail: the tail given away.)
            self.tidy(&mut lay, &mut Vec::new());
        }
        self.links(&mut lay);
        // (The chapel and the rooms that must be somewhere, once placed, are placed.)
        let here: Vec<&'static str> = (0..lay.rooms.len()).filter(|&r| lay.count(r as i16) > 0).map(|r| lay.rooms[r].kind).collect();
        self.chapel |= here.contains(&"chapel");
        self.pending.retain(|k| !here.contains(k));
        let (mut towers, corners) = self.tower_kinds(z);
        if z > self.hall_z && !(self.pieces && z == self.floors) && !barracks {
            self.lords_in_towers(&mut lay, &mut towers);
        }
        self.near_links(&mut lay);
        self.last = lay.cells.clone();
        let hall = (z == self.hall_z).then(|| self.hall_frame());
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public, name, roof, items: lay.items, towers, corners, hall })
    }

    /// The hall's frame (`super::hall::Frame`): from the screens' side (its low end) to the dais end,
    /// across its width.
    fn hall_frame(&self) -> super::hall::Frame {
        let h0 = self.service + self.screens as usize;
        let (ht0, ht1) = self.hall_t;
        super::hall::Frame { o: self.f.grid(h0 as isize, ht0 as isize), ds: self.f.ds, dt: self.f.dt, len: self.dz - h0, wid: ht1 - ht0 }
    }

    /// Whether half the floor's rooms or more (passages, halls and open tops aside) lie on just the
    /// squares of a room of the storey below (`last`), or over half of five or more are of one size.
    fn repeats(&self, lay: &Lay) -> bool {
        let (mut sizes, mut same) = (Vec::new(), 0);
        for r in 0..lay.rooms.len() as i16 {
            let kind = lay.rooms[r as usize].kind;
            let Some(k0) = lay.cells.iter().position(|&c| c == r) else { continue };
            if super::is_way_through(kind) || !super::walkable(kind) {
                continue;
            }
            sizes.push(lay.count(r));
            let below = self.last.get(k0).copied().unwrap_or(-1);
            if below >= 0 && self.last.iter().zip(&lay.cells).all(|(&b, &c)| (b == below) == (c == r)) {
                same += 1;
            }
        }
        let most = sizes.iter().map(|s| sizes.iter().filter(|t| *t == s).count()).max().unwrap_or(0);
        (!sizes.is_empty() && 2 * same >= sizes.len()) || (sizes.len() >= 5 && 2 * most > sizes.len())
    }

    /// A passage that meets none of the floor's others reached from the stairs (a wing's, beside a
    /// body whose passage runs along its other wall): doors through the rooms between it and them
    /// (`Hints::link`), the fewest.
    fn links(&self, lay: &mut Lay) {
        let (nx, ny) = (self.nx, self.ny);
        let n = lay.rooms.len();
        let st = self.stairs;
        let start = lay.cells[st[1] * nx + st[0]];
        if start < 0 {
            return;
        }
        let mut adj: Vec<Vec<i16>> = vec![Vec::new(); n];
        for k in 0..nx * ny {
            let a = lay.cells[k];
            if a < 0 || !super::walkable(lay.rooms[a as usize].kind) {
                continue;
            }
            for q in [(k % nx + 1 < nx).then_some(k + 1), (k / nx + 1 < ny).then_some(k + nx)].into_iter().flatten() {
                let b = lay.cells[q];
                if b >= 0 && b != a && super::walkable(lay.rooms[b as usize].kind) && !adj[a as usize].contains(&b) {
                    adj[a as usize].push(b);
                    adj[b as usize].push(a);
                }
            }
        }
        let circ = |lay: &Lay, r: i16| is_circulation(lay.rooms[r as usize].kind) || r == start;
        for _ in 0..n {
            // (Reached: the stairs' passages, through each other and the links made.)
            let mut seen = vec![false; n];
            let mut stack = vec![start];
            seen[start as usize] = true;
            while let Some(a) = stack.pop() {
                for &b in &adj[a as usize] {
                    let linked = lay.hints.link.contains(&(a.min(b), a.max(b)));
                    if !seen[b as usize] && ((circ(lay, a) && circ(lay, b)) || linked) {
                        seen[b as usize] = true;
                        stack.push(b);
                    }
                }
            }
            let Some(c) = (0..n as i16).find(|&r| !seen[r as usize] && circ(lay, r) && lay.count(r) > 0) else { return };
            // (The fewest rooms from it to a passage reached.)
            let mut from = vec![-1i16; n];
            let mut q = std::collections::VecDeque::from([c]);
            from[c as usize] = c;
            let mut end = None;
            while let Some(a) = q.pop_front() {
                if seen[a as usize] && circ(lay, a) {
                    end = Some(a);
                    break;
                }
                for &b in &adj[a as usize] {
                    if from[b as usize] < 0 {
                        from[b as usize] = a;
                        q.push_back(b);
                    }
                }
            }
            let Some(mut b) = end else { return };
            while b != c {
                let a = from[b as usize];
                lay.hints.link.push((a.min(b), a.max(b)));
                b = a;
            }
        }
    }

    /// The rooms an upper floor (`n` above the hall's) takes, besides the lord's (laid where they
    /// go: `upper_floor`): first what must be somewhere and is due here (the chapel on the first
    /// floor with room for it; on the top storey all that is left, else a treasury from the second
    /// floor up, an armory and a garrison from the third), then chambers and guest chambers, a
    /// study, stores... With how many come first.
    fn upper_kinds(&self, n: usize, last: bool) -> (Kinds, usize) {
        let mut k: Kinds = Vec::new();
        if !self.chapel {
            k.push(("chapel", 1.5));
        }
        for &p in &self.pending {
            if last || matches!((p, n), ("treasury", 2..) | ("armory" | "garrison", 3..)) {
                k.push((p, match p {
                    "great chamber" => 2.2,
                    "solar" => 1.8,
                    "garrison" => 1.9,
                    "armory" => 1.2,
                    _ => 0.5,
                }));
            }
        }
        let musts = k.len();
        let own: Kinds = match n {
            1 => vec![("chamber", 1.4), ("wardrobe", 0.7), ("guest chamber", 1.1), ("study", 0.9)],
            2 => vec![("chamber", 1.8), ("guest chamber", 1.5), ("study", 1.0), ("guest chamber", 1.1), ("wardrobe", 0.6)],
            3 => vec![("storeroom", 1.0), ("chamber", 0.9), ("map room", 1.2), ("guest chamber", 1.1)],
            _ => vec![("guest chamber", 1.4), ("servants' quarters", 1.6), ("storeroom", 1.0), ("library", 1.2), ("chamber", 0.8)],
        };
        k.extend(own);
        (k, musts)
    }

    /// Rooms of fewer than `min` squares, or a square wide (but passages, a hall's open height and
    /// roofs), made part of the room beside them they share the most wall with: another room
    /// first, a hall next, a passage last.
    fn absorb(&self, lay: &mut Lay, free: &mut Vec<i16>, min: usize) {
        let (nx, ny) = (self.nx, self.ny);
        for r in 0..lay.rooms.len() as i16 {
            let kind = lay.rooms[r as usize].kind;
            let n = lay.count(r);
            if n == 0 || is_circulation(kind) || !super::walkable(kind) || matches!(kind, "great hall" | "mess hall" | "battlements" | "wall walk") {
                continue;
            }
            let mine = |i: usize, j: usize| i < nx && j < ny && lay.cells[j * nx + i] == r;
            let wide = (0..nx * ny).any(|k| mine(k % nx, k / nx) && mine(k % nx + 1, k / nx) && mine(k % nx, k / nx + 1) && mine(k % nx + 1, k / nx + 1));
            if n >= min && wide {
                continue;
            }
            let Some(to) = self.beside(lay, &|k| lay.cells[k] == r) else { continue };
            for c in lay.cells.iter_mut() {
                if *c == r {
                    *c = to;
                }
            }
            free.retain(|&f| f != r);
        }
    }

    /// The floor's rooms (`free`) more than one room from a way about it (a passage, a hall): each
    /// made part of the room beside it nearest one (the floor's own first), as no door could bring
    /// it nearer.
    fn near_ways(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        loop {
            let n = lay.rooms.len();
            let mut adj: Vec<Vec<i16>> = vec![Vec::new(); n];
            for k in 0..nx * ny {
                let a = lay.cells[k];
                if a < 0 || !super::walkable(lay.rooms[a as usize].kind) {
                    continue;
                }
                for q in [(k % nx + 1 < nx).then_some(k + 1), (k / nx + 1 < ny).then_some(k + nx)].into_iter().flatten() {
                    let b = lay.cells[q];
                    if b >= 0 && b != a && super::walkable(lay.rooms[b as usize].kind) && !adj[a as usize].contains(&b) {
                        adj[a as usize].push(b);
                        adj[b as usize].push(a);
                    }
                }
            }
            let mut hops = vec![usize::MAX; n];
            let mut q = std::collections::VecDeque::new();
            for r in (0..n).filter(|&r| super::is_way_through(lay.rooms[r].kind)) {
                hops[r] = 0;
                q.push_back(r);
            }
            while let Some(a) = q.pop_front() {
                for &b in &adj[a] {
                    if hops[b as usize] == usize::MAX {
                        hops[b as usize] = hops[a] + 1;
                        q.push_back(b as usize);
                    }
                }
            }
            let merge = free.iter().filter(|&&r| hops[r as usize] != usize::MAX && hops[r as usize] >= 3).find_map(|&r| {
                adj[r as usize].iter().copied().filter(|&o| !super::is_way_through(lay.rooms[o as usize].kind)).min_by_key(|&o| (hops[o as usize], !free.contains(&o), o)).map(|o| (r, o))
            });
            let Some((r, to)) = merge else { return };
            for c in lay.cells.iter_mut() {
                if *c == r {
                    *c = to;
                }
            }
            free.retain(|&f| f != r);
        }
    }

    /// The room beside the squares `of` that they share the most wall with, a room first, a hall
    /// next, then a passage, a landing last (none of them open to below or a roof).
    fn beside(&self, lay: &Lay, of: &dyn Fn(usize) -> bool) -> Option<i16> {
        let (nx, ny) = (self.nx, self.ny);
        let mut shared: Vec<(i16, usize)> = Vec::new();
        for k in (0..nx * ny).filter(|&k| of(k)) {
            let (i, j) = (k % nx, k / nx);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a >= nx || b >= ny || of(b * nx + a) {
                    continue;
                }
                let o = lay.cells[b * nx + a];
                if o >= 0 && super::walkable(lay.rooms[o as usize].kind) {
                    match shared.iter_mut().find(|x| x.0 == o) {
                        Some(x) => x.1 += 1,
                        None => shared.push((o, 1)),
                    }
                }
            }
        }
        let rank = |o: i16| {
            let kind = lay.rooms[o as usize].kind;
            if matches!(kind, "great hall" | "mess hall") {
                2
            } else if kind == "landing" {
                0
            } else if is_circulation(kind) || matches!(kind, "battlements" | "wall walk") {
                1
            } else {
                3
            }
        };
        shared.iter().max_by(|a, b| rank(a.0).cmp(&rank(b.0)).then(a.1.cmp(&b.1)).then(b.0.cmp(&a.0))).map(|x| x.0)
    }

    /// Body squares no zone took: a room of their own where they make one (4 squares, two wide),
    /// else part of the room beside them (`beside`).
    fn leftovers(&self, lay: &mut Lay, free: &mut Vec<i16>) {
        let (nx, ny) = (self.nx, self.ny);
        let mut seen = vec![false; nx * ny];
        for k0 in 0..nx * ny {
            if seen[k0] || lay.cells[k0] >= 0 || self.piece[k0] != Piece::Body {
                continue;
            }
            let mut part = vec![k0];
            seen[k0] = true;
            let mut n = 0;
            while n < part.len() {
                let (i, j) = (part[n] % nx, part[n] / nx);
                n += 1;
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny && !seen[b * nx + a] && lay.cells[b * nx + a] < 0 && self.piece[b * nx + a] == Piece::Body {
                        seen[b * nx + a] = true;
                        part.push(b * nx + a);
                    }
                }
            }
            let has = |i: usize, j: usize| i < nx && j < ny && part.contains(&(j * nx + i));
            let wide = part.iter().any(|&k| has(k % nx + 1, k / nx) && has(k % nx, k / nx + 1) && has(k % nx + 1, k / nx + 1));
            let to = if part.len() >= 4 && wide { None } else { self.beside(lay, &|k| part.contains(&k)) };
            let r = to.unwrap_or_else(|| {
                let r = lay.room("chamber");
                free.push(r);
                r
            });
            for &k in &part {
                lay.cells[k] = r;
            }
        }
    }

    /// The dais end: the landing in its middle (the main stair in it), a room each side (`akind` on
    /// the front side, `bkind` on the far one) where it is 2 squares across. Returns the rooms each
    /// side and the landing.
    fn dais(&self, lay: &mut Lay, akind: &'static str, bkind: &'static str) -> (Option<i16>, Option<i16>, i16) {
        let (l, w) = (self.f.len, self.f.wid);
        let (tl0, tl1) = self.land_t;
        let landing = lay.room("landing");
        self.fill(lay, self.dz, l, tl0, tl1, landing);
        let mut side = |kind: &'static str, t0: usize, t1: usize| {
            (t1 >= t0 + 2).then(|| {
                let r = lay.room(kind);
                self.fill(lay, self.dz, l, t0, t1, r);
                r
            })
        };
        let a = side(akind, 0, tl0);
        let b = side(bkind, tl1, w);
        (a, b, landing)
    }

    /// The service end's rooms across its columns (`kinds`, by weight): each 4 squares or more (the
    /// kitchen 6) and 2 columns across, as near their weights as that allows; fewer kinds where they
    /// won't fit (the lightest dropped first). The columns where each ends.
    fn service_cols(&self, kinds: &mut Vec<(&'static str, f64)>) -> Vec<usize> {
        let w = self.f.wid;
        let col: Vec<usize> = (0..w).map(|t| (0..self.service).filter(|&s| self.body(s as isize, t as isize).is_some()).count()).collect();
        let sum = |a: usize, b: usize| col[a..b].iter().sum::<usize>();
        // (The kitchen 6 squares, 5 where the end holds fewer than 10.)
        let small = sum(0, w) < 10;
        let least = |k: &(&'static str, f64)| if k.0 == "kitchen" { 6 - small as usize } else { 4 };
        loop {
            let total: f64 = kinds.iter().map(|k| k.1).sum();
            let all = sum(0, w);
            let n = kinds.len();
            let mut ends = Vec::new();
            let mut t = 0;
            let mut acc = 0.0;
            for (m, k) in kinds.iter().enumerate() {
                acc += k.1;
                if m + 1 == n {
                    ends.push(w);
                    break;
                }
                let left = n - m - 1;
                let rest: usize = kinds[m + 1..].iter().map(least).sum();
                let want = all as f64 * acc / total;
                let t1 = (t + 2..=w.saturating_sub(2 * left))
                    .filter(|&t1| sum(t, t1) >= least(k) && sum(t1, w) >= rest)
                    .min_by(|&a, &b| (sum(0, a) as f64 - want).abs().total_cmp(&(sum(0, b) as f64 - want).abs()).then(a.cmp(&b)));
                let Some(t1) = t1 else { break };
                ends.push(t1);
                t = t1;
            }
            if ends.len() == n && sum(t, w) >= least(&kinds[n - 1]).min(all) {
                return ends;
            }
            if n == 1 {
                return vec![w];
            }
            // (The lightest goes first: a buttery, then a pantry.)
            let drop = (0..n).min_by(|&a, &b| kinds[a].1.total_cmp(&kinds[b].1).then(b.cmp(&a))).unwrap_or(n - 1);
            kinds.remove(drop);
        }
    }

    /// The hall's floor: service end, screens passage, hall (a strip of chambers beside a hall not
    /// as wide as the body), the dais end; the front door (on the ground floor) at the screens
    /// passage's end, the forebuilding's entrance or the mess hall's side. A barracks' officers'
    /// quarters, at the end away from the kitchen, run down the mess hall's side where a corner
    /// tower takes most of the end.
    fn hall_floor(&self, lay: &mut Lay, ground: bool, rng: &mut Pcg32) -> Vec<i16> {
        let w = self.f.wid;
        let barracks = self.kind == Kind::Barracks;
        let (ht0, ht1) = self.hall_t;
        let h0 = self.service + self.screens as usize;
        let sc = if self.screens { lay.room("screens passage") } else { -1 };
        let hall = lay.room(if barracks { "mess hall" } else { "great hall" });
        if self.screens {
            self.fill(lay, self.service, self.service + 1, 0, w, sc);
        }
        self.fill(lay, h0, self.dz, ht0, ht1, hall);
        // The service end: kitchen, buttery and pantry across it, each with its door in the screens
        // passage's wall as near its middle as the towers allow.
        if let Some(m) = self.bay.filter(|_| self.service > 0) {
            // (Entered from the end: the kitchen and the pantry each side of the mess hall's bay.)
            self.fill(lay, 0, self.service, m, m + 1, hall);
            let big = self.squares(0, self.service as isize, 0, m as isize) >= self.squares(0, self.service as isize, m as isize + 1, w as isize);
            let (k, p) = if big { ((0, m), (m + 1, w)) } else { ((m + 1, w), (0, m)) };
            for (kind, (t0, t1)) in [("kitchen", k), ("pantry", p)] {
                let r = lay.room(kind);
                self.fill(lay, 0, self.service, t0, t1, r);
                let c = ((t0 + t1) / 2) as isize;
                self.door(lay, (self.service as isize - 1, c), (self.service as isize, c));
            }
        } else if self.service > 0 {
            let mut kinds: Vec<(&'static str, f64)> = if barracks {
                vec![("kitchen", 1.6), ("pantry", 1.0)]
            } else if w >= 7 {
                vec![("buttery", 1.0), ("kitchen", 1.6), ("pantry", 1.0)]
            } else {
                vec![("kitchen", 1.6), ("buttery", 1.0)]
            };
            let ends = self.service_cols(&mut kinds);
            let next = if self.screens { self.service as isize } else { h0 as isize };
            let mut t = 0;
            for (&(kind, _), &t1) in kinds.iter().zip(&ends) {
                let r = lay.room(kind);
                self.fill(lay, 0, self.service, t, t1, r);
                let m = (t + t1) as isize / 2;
                let open = |c: isize| self.body(self.service as isize - 1, c).is_some() && self.body(next, c).is_some();
                if let Some(c) = (t as isize..t1 as isize).filter(|&c| open(c)).min_by_key(|&c| ((c - m).abs(), c)) {
                    self.door(lay, (self.service as isize - 1, c), (next, c));
                }
                t = t1;
            }
        }
        // The hall's door on its axis.
        let axis = ((ht0 + ht1) / 2) as isize;
        if self.screens {
            self.door(lay, (self.service as isize, axis), (h0 as isize, axis));
        }
        let mut free = Vec::new();
        // A strip of chambers beside a narrower hall; a mural passage along its outer wall when
        // deep enough, from the screens passage to the dais end.
        if ht1 < w {
            let deep = w - ht1;
            let t1 = if deep >= 4 {
                let p = lay.room("mural passage");
                self.fill(lay, h0, self.dz, w - 1, w, p);
                w - 1
            } else {
                w
            };
            free.extend(self.strip(lay, h0, self.dz, ht1, t1, Self::rooms_in(self.dz - h0, rng), rng));
        }
        if barracks {
            let (a, b, _) = self.dais(lay, "officers' quarters", "armory");
            // (On the front side if there is room, else the far one; 6 squares at least.)
            let (r, t0, t1) = match (a, b) {
                (Some(r), _) => (r, 0, self.land_t.0),
                (None, Some(r)) => (r, self.land_t.1, w),
                _ => (-1, 0, 0),
            };
            if r >= 0 {
                lay.rooms[r as usize].kind = "officers' quarters";
                if let Some(b) = b.filter(|&b| b != r) {
                    lay.rooms[b as usize].kind = "armory";
                }
                let mut s0 = self.dz;
                let wide = |lay: &Lay| {
                    let mine = |s: isize, t: isize| self.body(s, t).is_some_and(|k| lay.cells[k] == r);
                    (0..self.f.len as isize).any(|s| (0..w as isize).any(|t| mine(s, t) && mine(s + 1, t) && mine(s, t + 1) && mine(s + 1, t + 1)))
                };
                while (lay.count(r) < 6 || !wide(lay)) && s0 > h0 + 3 && self.dz - s0 < 3 {
                    s0 -= 1;
                    for t in t0..t1 {
                        if let Some(k) = self.body(s0 as isize, t as isize).filter(|&k| lay.cells[k] == hall) {
                            lay.cells[k] = r;
                        }
                    }
                }
            }
        } else {
            self.dais(lay, "parlor", "wardrobe");
        }
        let mut tl0 = self.land_t.0 as isize;
        // (A hall of 3 rows: its door to the landing beside the middle columns, the dais's, the
        // landing a square wider there.)
        let (dz, l) = (self.dz as isize, self.f.len as isize);
        if !barracks && self.dz - h0 <= 3 && tl0 >= 1 && w % 2 == 0 && l - dz >= 3 {
            if let Some(k) = self.body(dz, tl0 - 1) {
                let landing = lay.cells[self.body(dz, tl0).unwrap_or(k)];
                if landing >= 0 && lay.rooms[landing as usize].kind == "landing" {
                    lay.cells[k] = landing;
                    tl0 -= 1;
                }
            }
        }
        self.door(lay, (self.dz as isize - 1, tl0), (self.dz as isize, tl0));
        if ground {
            lay.entry = Some((sc, hall));
        }
        free
    }

    /// The front door's hint: into the forebuilding's entrance hall, the screens passage's ends, or
    /// the mess hall's side (nearer the street first).
    fn front(&self, lay: &mut Lay, screens: i16, hall: i16) {
        let w = self.f.wid as isize;
        if self.kind == Kind::Hall && !self.fore.is_empty() {
            // (The entrance hall's walls out, the far side's middle first.)
            let Some(&r) = self.fore.iter().map(|&k| &lay.cells[k]).find(|&&c| c >= 0) else { return };
            let local: Vec<(isize, isize)> = self.fore.iter().map(|&k| self.f.local((k % self.nx) as isize, (k / self.nx) as isize)).collect();
            let far = local.iter().map(|q| q.1).min().unwrap_or(0);
            let ms = local.iter().map(|q| q.0).sum::<isize>() / local.len().max(1) as isize;
            let mut edges: Vec<(isize, (usize, usize, usize))> = Vec::new();
            for &(s, t) in &local {
                for (ds, dt) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                    if self.at(s + ds, t + dt).is_none_or(|k| self.piece[k] == Piece::Out) {
                        let (a, b) = (self.f.grid(s, t), self.f.grid(s + ds, t + dt));
                        let rank = if t == far && dt == -1 { 0 } else { 100 } + (s - ms).abs();
                        edges.push((rank, (side(a, b), a.0 as usize, a.1 as usize)));
                    }
                }
            }
            edges.sort();
            lay.hints.front = Some((r, edges.into_iter().map(|e| e.1).collect()));
            // (Its door into the screens passage.)
            for &(s, t) in &local {
                if self.body(s, t + 1).is_some_and(|k| lay.cells[k] == screens) {
                    self.door(lay, (s, t), (s, t + 1));
                    break;
                }
            }
            return;
        }
        let order = |a: (isize, isize, isize, isize), b: (isize, isize, isize, isize)| if self.ends[1] { vec![a, b] } else { vec![b, a] };
        if screens >= 0 {
            let s = self.service as isize;
            let e = order((s, 0, s, -1), (s, w - 1, s, w));
            lay.hints.front = Some((screens, e.into_iter().map(|(s0, t0, s1, t1)| {
                let (a, b) = (self.f.grid(s0, t0), self.f.grid(s1, t1));
                (side(a, b), a.0 as usize, a.1 as usize)
            }).collect()));
        } else {
            let (h0, h1) = (self.service as isize, self.dz as isize);
            let m = (h0 + h1) / 2;
            let mut e = Vec::new();
            if let Some(t) = self.bay {
                e.push((0, t as isize, -1, t as isize));
            }
            for d in 0..(h1 - h0) {
                let s = if d % 2 == 0 { m + d / 2 } else { m - d / 2 - 1 };
                e.extend(order((s, 0, s, -1), (s, w - 1, s, w)));
            }
            lay.hints.front = Some((hall, e.into_iter().map(|(s0, t0, s1, t1)| {
                let (a, b) = (self.f.grid(s0, t0), self.f.grid(s1, t1));
                (side(a, b), a.0 as usize, a.1 as usize)
            }).collect()));
        }
    }

    /// A great tower's ground floor: the entrance hall down the axis from the door at the low end
    /// to the main stair at the far end, stores, a well room and an armory each side.
    fn stores_floor(&self, lay: &mut Lay, rng: &mut Pcg32) -> Vec<i16> {
        let (l, w) = (self.f.len, self.f.wid);
        let (tl0, tl1) = self.land_t;
        let hall = lay.room("entrance hall");
        self.fill(lay, 0, l, tl0, tl1, hall);
        let mut free = Vec::new();
        for (t0, t1) in [(0, tl0), (tl1, w)] {
            if t1 - t0 >= 2 {
                free.extend(self.strip(lay, 0, l, t0, t1, Self::rooms_in(l, rng).min(3), rng));
            } else {
                self.fill(lay, 0, l, t0, t1, hall);
            }
        }
        let mut e = Vec::new();
        for t in [tl0 as isize, tl0 as isize + 1, tl1 as isize - 1] {
            let (a, b) = (self.f.grid(0, t), self.f.grid(-1, t));
            e.push((side(a, b), a.0 as usize, a.1 as usize));
        }
        lay.hints.front = Some((hall, e));
        free
    }

    /// The long wall (t) a passage runs along from row `s1` (included) down as far as it runs
    /// unbroken (to `low` at most): the one it runs further along, else the other from the last
    /// storey's; with how far down it runs (`flip`: the other, where there is one). `turn`: the
    /// cross passage to the landing on row `s1` (its columns from the wall), which must be whole too.
    fn passage_side(&self, sides: &[isize], s1: isize, low: isize, turn: &dyn Fn(isize) -> (isize, isize), flip: bool) -> Option<(isize, isize)> {
        let mut c: Vec<(isize, isize)> = sides
            .iter()
            .filter_map(|&pt| {
                let (a, b) = turn(pt);
                if !(a..b).all(|t| self.body(s1, t).is_some()) || self.body(s1, pt).is_none() {
                    return None;
                }
                let mut e = s1;
                while e > low && self.body(e - 1, pt).is_some() {
                    e -= 1;
                }
                Some((pt, e))
            })
            .collect();
        c.sort_by_key(|&(pt, e)| (e, pt == self.side));
        c.get(if flip && c.len() > 1 { 1 } else { 0 }).copied()
    }

    /// An upper floor: the landing at the dais end; a cross passage under it from one long wall
    /// (the other from the storey below's where both will do, or on the `more`th try at it the
    /// other) and a mural passage on down that wall; rooms each side of the landing; the rest of the body divided by cross walls into rooms
    /// of sensible proportions (`partition`), as many as the floor must hold (`need`) at least. The
    /// lord's floor has the great chamber across the body off the cross passage by the passage's
    /// wall, about twice as wide as it is deep (ended at a cross wall in a wide body, a wardrobe or
    /// closet beyond it), and the solar beside the landing (down the great chamber's side too
    /// where that is narrow). A body whose long walls are both broken at the dais end gets a
    /// corridor down its axis.
    fn upper_floor(&mut self, lay: &mut Lay, rng: &mut Pcg32, lord: bool, need: usize, more: usize) -> Vec<i16> {
        let flip = more % 2 == 1;
        let (l, w, dz) = (self.f.len as isize, self.f.wid as isize, self.dz as isize);
        let (tl0, tl1) = (self.land_t.0 as isize, self.land_t.1 as isize);
        let row = dz - 1;
        let turn = |pt: isize| if pt == 0 { (0, tl1) } else { (tl0, w) };
        let Some((pt, e)) = self.passage_side(&[w - 1, 0], row, 0, &turn, flip) else { return self.corridor_floor(lay, rng, need) };
        // (A long body whose long walls are broken soon below the cross passage: the corridor.)
        if row >= 10 && e > row - 4 {
            return self.corridor_floor(lay, rng, need);
        }
        self.side = pt;
        let landing = lay.room("landing");
        self.fill(lay, dz as usize, l as usize, tl0 as usize, tl1 as usize, landing);
        let passage = lay.room("mural passage");
        let (a, b) = turn(pt);
        self.fill(lay, row as usize, dz as usize, a as usize, b as usize, passage);
        self.fill(lay, e as usize, row as usize, pt as usize, pt as usize + 1, passage);
        // (The landing's sides: by the passage's wall, and across from it from the cross passage's
        // row; the rooms' columns beside the passage.)
        let (near, far) = if pt == 0 { ((0, tl0), (tl1, w)) } else { ((tl1, w), (0, tl0)) };
        let (t0, t1) = if pt == 0 { (1, w) } else { (0, w - 1) };
        let mut free = Vec::new();
        let mut lines: Vec<(bool, isize)> = Vec::new();
        let want_gc = lord && self.pending.contains(&"great chamber");
        // A column of the landing (one the stair isn't in) to a room beside it: the solar's where
        // the far side is too narrow for it, else now and then either side's.
        let st = self.stairs;
        let on_stairs = |k: usize| (k % self.nx) >= st[0] && (k % self.nx) < st[0] + st[2] && (k / self.nx) >= st[1] && (k / self.nx) < st[1] + st[3];
        let open_col = |c: isize| tl1 - tl0 >= 2 && !(dz..l).any(|s| self.at(s, c).is_some_and(on_stairs));
        let (far_c, near_c) = if pt == 0 { (tl1 - 1, tl0) } else { (tl0, tl1 - 1) };
        let mut solar = !(lord && self.pending.contains(&"solar"));
        let roll = rng.below(3);
        let take_far = open_col(far_c) && ((!solar && far.1 - far.0 < 3) || roll == 1);
        let take_near = !take_far && open_col(near_c) && roll == 2;
        let widen = |(c0, c1): (isize, isize), c: isize, take: bool| if !take { (c0, c1) } else { (c0.min(c), c1.max(c + 1)) };
        let (far, near) = (widen(far, far_c, take_far), widen(near, near_c, take_near));
        // The solar beside the landing, where there is room for it (9 squares, 2 across or more, no
        // longer than 2.5 times its width: on down a row or two past the cross passage's where it
        // needs them).
        let across = far.1 - far.0;
        for k in 0..=2isize {
            let sq = self.open_squares(lay, row - k, l, far, landing);
            if solar || across < 2 || row - k < 1 {
                break;
            }
            let deep = l - (row - k);
            if sq.len() >= 9 && 2 * deep.max(across) <= 5 * deep.min(across) {
                let r = lay.room("solar");
                for q in sq {
                    lay.cells[q] = r;
                }
                solar = true;
                self.pending.retain(|k| *k != "solar");
            }
        }
        // The band under the cross passage: the great chamber by the passage's wall, about twice as
        // wide as it is deep; beyond its cross wall in a wide body the solar (where it isn't beside
        // the landing) or a room or two.
        let wm = t1 - t0;
        let band_solar = !solar && (!want_gc || wm >= 5);
        let mut top = row;
        if want_gc || band_solar {
            let sw = if !band_solar {
                0
            } else if !want_gc {
                wm
            } else if wm >= 6 {
                (crate::core::round(wm as f64 * (0.35 + 0.1 * rng.next_f64())) as isize).clamp(3, wm - 3)
            } else {
                2
            };
            let gw = if !want_gc {
                0
            } else if band_solar {
                wm - sw
            } else if wm <= 10 {
                wm
            } else {
                (crate::core::round(wm as f64 * (0.5 + 0.15 * rng.next_f64())) as isize).clamp(6, wm - 3)
            };
            // (The band's depth: the great chamber's width over 1.3 to 1.9, 60 squares at most, 3
            // rows at least (beside the solar 4, 5 by a narrow great chamber); a solar alone, 16
            // squares or so.)
            let gd = if want_gc { crate::core::round(gw as f64 / (1.3 + 0.6 * rng.next_f64())) as isize } else { crate::core::round(16.0 / sw as f64) as isize };
            let mut gd = gd.min((60 / gw.max(1)).max(2)).max(if !band_solar { 3 } else if gw <= 4 { 5 } else { 4 }).max((2 * sw + 4) / 5).min(row);
            // (Too small for both of 9 squares: the solar's, the great chamber a floor up; a great
            // chamber alone, all the body below the cross passage.)
            let (sw, gw) = if band_solar && gw > 0 && (sw.min(gw) * gd < 9) { (wm, 0) } else { (sw, gw) };
            if gw > 0 && sw == 0 && gw * gd < 9 {
                gd = row;
            }
            // (Leaving no strip under 3 rows below it.)
            if row - gd > 0 && row - gd < 3 {
                gd = if gd - (3 - (row - gd)) >= 3 { row - 3 } else if (gw + sw) * row <= 60 { row } else { gd };
            }
            top = row - gd;
            // (The great chamber's columns, from the passage's wall; the solar's beyond.)
            let (g0, g1) = if pt == 0 { (t0, t0 + gw) } else { (t1 - gw, t1) };
            let (o0, o1) = if pt == 0 { (g1, t1) } else { (t0, g0) };
            // (The solar beyond the cross wall opening off the great chamber.)
            let mut lords = Vec::new();
            for (kind, c0, c1) in [("great chamber", g0, g1), ("solar", o0, o0 + sw)] {
                if c1 <= c0 {
                    continue;
                }
                let r = lay.room(kind);
                for s in top..row {
                    for t in c0..c1 {
                        if let Some(k) = self.body(s, t).filter(|&k| lay.cells[k] < 0) {
                            lay.cells[k] = r;
                        }
                    }
                }
                self.keep_whole(lay, r);
                self.pending.retain(|k| *k != kind);
                lords.push(r);
            }
            if let [gc, sol] = lords[..] {
                lay.hints.link.push((gc.min(sol), gc.max(sol)));
            }
            solar |= band_solar;
            lines.push((true, top));
            lines.push((false, if pt == 0 { g1 } else { g0 }));
            if gw + sw < wm {
                free.extend(self.partition(lay, [top, row, o0, o1], 1, 1, &mut lines, rng));
            }
        }
        for (s0, side) in [(dz, near), (row, far)] {
            let sq = self.open_squares(lay, s0, l, side, landing);
            if side.1 - side.0 >= 2 && !sq.is_empty() {
                let r = lay.room("chamber");
                for k in sq {
                    lay.cells[k] = r;
                }
                free.push(r);
            }
        }
        // The rest of the body, down from the great chamber (or the cross passage) to the low end
        // (past where the passage ends, a tower's notch, beside it too), divided off the passage (or,
        // laid out again, off its end by the cross passage).
        let way = if more >= 2 { 1 } else if pt == 0 { 2 } else { 3 };
        let mut rest = self.partition(lay, [0, top, 0, w], way, need.saturating_sub(free.len()).max(1), &mut lines, rng);
        // (No room for the solar by the landing: the room of 9 squares or more nearest it.)
        if !solar {
            let high = |r: i16| (0..self.nx * self.ny).filter(|&k| lay.cells[k] == r).map(|k| self.f.local((k % self.nx) as isize, (k / self.nx) as isize).0).max().unwrap_or(-1);
            if let Some(n) = (0..rest.len()).filter(|&n| lay.count(rest[n]) >= 9).max_by_key(|&n| (high(rest[n]), std::cmp::Reverse(n))) {
                lay.rooms[rest[n] as usize].kind = "solar";
                self.pending.retain(|k| *k != "solar");
                rest.remove(n);
            }
        }
        free.extend(rest);
        free
    }

    /// Rooms over the body's free squares in box `b` (s0, s1, t0, t1), divided as a keep's storeys
    /// were, off its side `way` runs along (0 its low s, 1 its high s, 2 its low t, 3 its high t: a
    /// passage's): a box over 7 squares deep from it (6 or 7, now and then, where nothing opens off
    /// it) parted by a cross wall along it into a near part and a far one, each cut across into
    /// rooms at most 2.5 times as long as they are wide and of 40 squares at most, of lengths
    /// rolled (more of them where the box must hold `want` rooms), the far part's cross walls off
    /// the near part's (`lines`: walls across s or t, and where): every room on the way or beside
    /// a room that is. A tower, a wing or the forebuilding that would open only off the far part
    /// has a mural passage to it across that part. A room of 20 squares or more on the way and an
    /// outer wall may have a closet off its corner there. The rooms.
    #[allow(clippy::too_many_arguments)]
    fn partition(&self, lay: &mut Lay, b: [isize; 4], way: usize, want: usize, lines: &mut Vec<(bool, isize)>, rng: &mut Pcg32) -> Vec<i16> {
        let (nx, ny) = (self.nx, self.ny);
        let free = |lay: &Lay, s: isize, t: isize| self.body(s, t).filter(|&k| lay.cells[k] < 0);
        let mut bb = [isize::MAX, isize::MIN, isize::MAX, isize::MIN];
        for s in b[0]..b[1] {
            for t in b[2]..b[3] {
                if free(lay, s, t).is_some() {
                    bb = [bb[0].min(s), bb[1].max(s + 1), bb[2].min(t), bb[3].max(t + 1)];
                }
            }
        }
        if bb[0] >= bb[1] {
            return Vec::new();
        }
        // (Depth from the way along s or t; `hi`: the way on the box's high side. A square by its
        // depth and place along the way.)
        let (deep_s, hi) = (way < 2, way % 2 == 1);
        let (d0, d1, a0, a1) = if deep_s { (bb[0], bb[1], bb[2], bb[3]) } else { (bb[2], bb[3], bb[0], bb[1]) };
        let at = |d: isize, a: isize| if deep_s { (d, a) } else { (a, d) };
        let depth = d1 - d0;
        // (Rooms already laid that are ways about the floor or open onto one; what lies beyond the
        // body: a tower, a wing, the forebuilding.)
        let nbrs = |k: usize| [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1), (k >= nx).then(|| k - nx), (k + nx < nx * ny).then_some(k + nx)].into_iter().flatten();
        let circ = |lay: &Lay, k: usize| lay.cells[k] >= 0 && is_circulation(lay.rooms[lay.cells[k] as usize].kind);
        let mut opens = vec![false; lay.rooms.len()];
        for k in (0..nx * ny).filter(|&k| lay.cells[k] >= 0 && (circ(lay, k) || nbrs(k).any(|q| circ(lay, q)))) {
            opens[lay.cells[k] as usize] = true;
        }
        let beyond = |k: usize| matches!(self.piece[k], Piece::Tower | Piece::Wing(_) | Piece::Fore);
        let touches_beyond = (d0..d1).any(|d| {
            (a0..a1).any(|a| {
                let (s, t) = at(d, a);
                free(lay, s, t).is_some_and(|k| nbrs(k).any(beyond))
            })
        });
        let parted = depth >= 8 || (depth >= 6 && !touches_beyond && rng.next_f64() < 0.5);
        // (A depth whose rooms can fill the box's length: some number of them between their least
        // and most lengths.)
        let fits = |d: isize| {
            let len = a1 - a0;
            let (lmin, lmax) = (((2 * d + 4) / 5).max(2).min(len), ((5 * d) / 2).min(40 / d.max(1)));
            (1..=len).any(|n| n * lmin <= len && len <= n * lmax)
        };
        let near = if parted {
            let aim = crate::core::round(depth as f64 * (0.4 + 0.2 * rng.next_f64())) as isize;
            (3..=depth - 3).filter(|&d| fits(d) && fits(depth - d)).min_by_key(|&d| ((d - aim).abs(), d)).unwrap_or(aim.clamp(3, depth - 3))
        } else {
            depth
        };
        // (Zones: (from, to) in depth, the near one first.)
        let zones = if !parted {
            vec![(d0, d1)]
        } else if hi {
            vec![(d1 - near, d1), (d0, d1 - near)]
        } else {
            vec![(d0, d0 + near), (d0 + near, d1)]
        };
        if parted {
            lines.push((deep_s, if hi { d1 - near } else { d0 + near }));
        }
        let in_zone = |lay: &Lay, z: (isize, isize), k: usize| {
            let (s, t) = self.f.local((k % nx) as isize, (k / nx) as isize);
            let (d, a) = if deep_s { (s, t) } else { (t, s) };
            d >= z.0 && d < z.1 && a >= a0 && a < a1 && lay.cells[k] < 0 && self.piece[k] == Piece::Body
        };
        // The far part's mural passages: to each tower, wing or forebuilding beside it that opens
        // off nothing else (no square of the near part, no way, no room opening onto one), across
        // it at its end where they meet there, else in the middle of where they meet.
        let mut stubs: Vec<isize> = Vec::new();
        if let Some(&far) = zones.get(1) {
            let mut seen = vec![false; nx * ny];
            for k0 in 0..nx * ny {
                if seen[k0] || !beyond(k0) || !nbrs(k0).any(|q| in_zone(lay, far, q)) {
                    continue;
                }
                let mut part = vec![k0];
                seen[k0] = true;
                let mut i = 0;
                while i < part.len() {
                    let k = part[i];
                    i += 1;
                    for q in nbrs(k) {
                        if !seen[q] && self.piece[q] == self.piece[k0] {
                            seen[q] = true;
                            part.push(q);
                        }
                    }
                }
                if part.iter().any(|&k| nbrs(k).any(|q| in_zone(lay, zones[0], q) || (lay.cells[q] >= 0 && opens[lay.cells[q] as usize]))) {
                    continue;
                }
                let mut along: Vec<isize> = part
                    .iter()
                    .flat_map(|&k| nbrs(k).filter(|&q| in_zone(lay, far, q)).collect::<Vec<_>>())
                    .map(|q| {
                        let (s, t) = self.f.local((q % nx) as isize, (q / nx) as isize);
                        if deep_s { t } else { s }
                    })
                    .collect();
                along.sort();
                along.dedup();
                let a = if along.contains(&a0) { a0 } else if along.contains(&(a1 - 1)) { a1 - 1 } else { along[along.len() / 2] };
                if !stubs.contains(&a) {
                    stubs.push(a);
                }
            }
            stubs.sort();
            if !stubs.is_empty() {
                let p = lay.room("mural passage");
                for &a in &stubs {
                    for d in far.0..far.1 {
                        let (s, t) = at(d, a);
                        if let Some(k) = free(lay, s, t) {
                            lay.cells[k] = p;
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut extra = want.saturating_sub(zones.len());
        for (zi, &(z0, z1)) in zones.iter().enumerate() {
            let d = z1 - z0;
            // (Its runs along the way, between the mural passages across it.)
            let mut runs: Vec<(isize, isize)> = Vec::new();
            let mut from = a0;
            for &c in stubs.iter().filter(|_| zi > 0) {
                if c > from {
                    runs.push((from, c));
                }
                from = c + 1;
            }
            if from < a1 {
                runs.push((from, a1));
            }
            for (r0, r1) in runs {
                let len = r1 - r0;
                let held: Vec<usize> = (r0..r1)
                    .map(|a| {
                        (z0..z1)
                            .filter(|&d| {
                                let (s, t) = at(d, a);
                                free(lay, s, t).is_some()
                            })
                            .count()
                    })
                    .collect();
                // (A room's length along the way: no more than 2.5 times its depth (as most of it is
                // deep: a notch, a passage taking some) or 40 squares, no less than its depth over 2.5,
                // 2 squares.)
                let mut deep: Vec<usize> = held.iter().copied().filter(|&h| h > 0).collect();
                deep.sort();
                let d = deep.get(deep.len() / 2).map_or(d, |&h| h as isize);
                let lmin = ((2 * d + 4) / 5).max(2).min(len);
                let lmax = ((5 * d) / 2).min(40 / d.max(1)).max(lmin);
                let lo = (len + lmax - 1) / lmax;
                let most = (len / lmin).max(lo);
                let add = extra.min((most - lo) as usize) as isize;
                extra -= add as usize;
                let roomy = len >= (lo + add + 1) * (lmin + 1);
                let n = (lo + add + (roomy && rng.next_f64() < 0.35) as isize).min(most).max(1);
                // (Lengths of very different sizes, within those bounds.)
                let mut ls: Vec<isize> = Vec::new();
                for _ in 0..8 {
                    ls = cuts(len as usize, n as usize, lmin as usize, rng).into_iter().map(|x| x as isize).collect();
                    if ls.iter().all(|&x| x <= lmax) {
                        break;
                    }
                }
                if ls.iter().any(|&x| x > lmax) {
                    ls = (0..n).map(|k| len * (k + 1) / n - len * k / n).collect();
                }
                // (Each wall where the rooms before it have their share of the squares (a room by a
                // notch runs on further), each room as long as it may be and with a 2 x 2 block of
                // squares where it can be, off the lines of the walls across of the part before.)
                let total: usize = held.iter().sum();
                let mut walls: Vec<isize> = Vec::new();
                let mut share = 0;
                let upto = |c: isize| held[..(c - r0) as usize].iter().sum::<usize>();
                for (i, &x) in ls[..ls.len() - 1].iter().enumerate() {
                    share += x;
                    let aim = total as f64 * share as f64 / len as f64;
                    let lo = walls.last().map_or(r0 + 1, |&c| c + 1);
                    let before = walls.last().map_or(0, |&c| upto(c));
                    let left = ls.len() - 1 - i;
                    let prev = walls.last().copied().unwrap_or(r0);
                    let solid = |a0: isize, a1: isize| {
                        (a0..a1 - 1).any(|a| {
                            (z0..z1 - 1).any(|x| {
                                [(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|&(da, dx)| {
                                    let (s, t) = at(x + dx, a + da);
                                    free(lay, s, t).is_some()
                                })
                            })
                        })
                    };
                    let bad = |c: isize| {
                        (upto(c) < before + 4 || !solid(prev, c)) as u8 + (c - prev < lmin || c - prev > lmax || r1 - c < lmin * left as isize) as u8
                    };
                    let off = |c: isize| (upto(c) as f64 - aim).abs() + if lines.contains(&(!deep_s, c)) { d as f64 } else { 0.0 };
                    let best = (lo..r1).min_by(|&a, &b| bad(a).cmp(&bad(b)).then(off(a).total_cmp(&off(b))).then(a.cmp(&b)));
                    if let Some(c) = best {
                        walls.push(c);
                    }
                }
                // (In the near part, where the way runs along most of it, each room on it: one past
                // the way's end (a tower's notch) runs on to it.)
                if zi == 0 {
                    let on: Vec<bool> = (r0..r1)
                        .map(|a| {
                            (z0..z1).any(|d| {
                                let (s, t) = at(d, a);
                                free(lay, s, t).is_some_and(|k| nbrs(k).any(|q| circ(lay, q)))
                            })
                        })
                        .collect();
                    if 2 * on.iter().filter(|&&o| o).count() > on.len() {
                        let first = r0 + on.iter().position(|&o| o).unwrap_or(0) as isize;
                        let last = r0 + on.iter().rposition(|&o| o).unwrap_or(0) as isize;
                        walls.retain(|&c| c > first && c <= last);
                        // (Split again where that left an end room too long.)
                        let c = walls.first().copied().unwrap_or(r1);
                        if c - r0 > lmax && (first + 1).max(r0 + lmin).max(c - lmax) <= c - 2 {
                            walls.insert(0, (first + 1).max(r0 + lmin).max(c - lmax));
                        }
                        let c = walls.last().copied().unwrap_or(r0);
                        if r1 - c > lmax && last.min(r1 - lmin).min(c + lmax) >= c + 2 {
                            walls.push(last.min(r1 - lmin).min(c + lmax));
                        }
                    }
                }
                lines.extend(walls.iter().map(|&c| (!deep_s, c)));
                let mut from = r0;
                for c in walls.into_iter().chain([r1]) {
                    let r = lay.room("chamber");
                    let mut n = 0;
                    for x in z0..z1 {
                        for y in from..c {
                            let (s, t) = at(x, y);
                            if let Some(k) = free(lay, s, t) {
                                lay.cells[k] = r;
                                n += 1;
                            }
                        }
                    }
                    out.push(r);
                    if zi == 0 && n >= 20 && rng.next_f64() < 0.5 {
                        let bx = if deep_s { [z0, z1, from, c] } else { [from, c, z0, z1] };
                        out.extend(self.closet(lay, r, bx, rng));
                    }
                    from = c;
                }
            }
        }
        for &r in &out {
            self.keep_whole(lay, r);
        }
        out
    }

    /// A closet off a corner of room `r` (its box `bx`) on an outer wall: 2 squares by 2 or 3
    /// along the box's longer side, all the room's.
    fn closet(&self, lay: &mut Lay, r: i16, bx: [isize; 4], rng: &mut Pcg32) -> Option<i16> {
        let (ls, lt) = if bx[1] - bx[0] >= bx[3] - bx[2] { (2 + rng.below(2) as isize, 2) } else { (2, 2 + rng.below(2) as isize) };
        let c0 = rng.below(4) as usize;
        for c in (0..4).map(|m| (c0 + m) % 4) {
            let (s0, t0) = (if c & 1 == 0 { bx[0] } else { bx[1] - ls }, if c & 2 == 0 { bx[2] } else { bx[3] - lt });
            let sq: Vec<(isize, isize)> = (s0..s0 + ls).flat_map(|s| (t0..t0 + lt).map(move |t| (s, t))).collect();
            let out = |s: isize, t: isize| self.at(s, t).is_none_or(|k| self.piece[k] == Piece::Out);
            if !sq.iter().all(|&(s, t)| self.body(s, t).is_some_and(|k| lay.cells[k] == r)) || !sq.iter().any(|&(s, t)| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(ds, dt)| out(s + ds, t + dt))) {
                continue;
            }
            let id = lay.room("chamber");
            for &(s, t) in &sq {
                if let Some(k) = self.body(s, t) {
                    lay.cells[k] = id;
                }
            }
            return Some(id);
        }
        None
    }

    /// Room `r` in one piece: its biggest (the rest left to the room beside it, `leftovers`).
    fn keep_whole(&self, lay: &mut Lay, r: i16) {
        let (nx, ny) = (self.nx, self.ny);
        let mut part = vec![usize::MAX; nx * ny];
        let mut sizes: Vec<usize> = Vec::new();
        for k0 in 0..nx * ny {
            if lay.cells[k0] != r || part[k0] != usize::MAX {
                continue;
            }
            let p = sizes.len();
            let mut stack = vec![k0];
            part[k0] = p;
            let mut n = 0;
            while let Some(k) = stack.pop() {
                n += 1;
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny && lay.cells[b * nx + a] == r && part[b * nx + a] == usize::MAX {
                        part[b * nx + a] = p;
                        stack.push(b * nx + a);
                    }
                }
            }
            sizes.push(n);
        }
        if sizes.len() > 1 {
            let keep = (0..sizes.len()).max_by_key(|&p| (sizes[p], std::cmp::Reverse(p))).unwrap_or(0);
            for k in 0..nx * ny {
                if lay.cells[k] == r && part[k] != keep {
                    lay.cells[k] = -1;
                }
            }
        }
    }

    /// An upper floor whose long walls are both broken by the dais end: the landing at the dais
    /// end, a corridor on from it down the axis (ending at rooms across the low end in a long
    /// body), rooms each side divided by cross walls (`partition`); the lord's rooms the biggest.
    fn corridor_floor(&mut self, lay: &mut Lay, rng: &mut Pcg32, need: usize) -> Vec<i16> {
        let (w, dz) = (self.f.wid, self.dz);
        let (tl0, _) = self.land_t;
        let mut free = Vec::new();
        let mut lines = Vec::new();
        let e = if dz >= 11 { 3 + rng.below(2) as usize } else { 0 };
        let corridor = lay.room("corridor");
        self.fill(lay, e, dz, tl0, tl0 + 1, corridor);
        for (t0, t1) in [(0, tl0), (tl0 + 1, w)] {
            if t1 - t0 >= 2 {
                free.extend(self.partition(lay, [e as isize, dz as isize, t0 as isize, t1 as isize], if t0 == 0 { 3 } else { 2 }, need.div_ceil(2), &mut lines, rng));
            } else {
                self.fill(lay, e, dz, t0, t1, corridor);
            }
        }
        if e > 0 {
            free.extend(self.partition(lay, [0, e as isize, 0, w as isize], 1, 1, &mut lines, rng));
        }
        let (a, b, _) = self.dais(lay, "chamber", "chamber");
        free.extend(a.into_iter().chain(b));
        // (The lord's rooms, where they are still to be placed: the biggest by the stair.)
        let mut lords: Vec<&'static str> = self.pending.iter().copied().filter(|k| matches!(*k, "great chamber" | "solar")).collect();
        let stc = [self.stairs[0] as f64 + 0.5 * self.stairs[2] as f64, self.stairs[1] as f64 + 0.5 * self.stairs[3] as f64];
        let mut near: Vec<(f64, i16)> = free
            .iter()
            .filter(|&&r| lay.count(r) >= 9)
            .map(|&r| {
                let sq: Vec<usize> = (0..self.nx * self.ny).filter(|&k| lay.cells[k] == r).collect();
                let c = [sq.iter().map(|&k| (k % self.nx) as f64 + 0.5).sum::<f64>() / sq.len() as f64, sq.iter().map(|&k| (k / self.nx) as f64 + 0.5).sum::<f64>() / sq.len() as f64];
                (geom::dist(c, stc), r)
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, r) in near {
            let Some(kind) = lords.pop() else { break };
            lay.rooms[r as usize].kind = kind;
            free.retain(|&f| f != r);
            self.pending.retain(|k| *k != kind);
        }
        free
    }

    /// The floor over a hall open two storeys: its height left open (`OPEN_BELOW`), a minstrels'
    /// gallery over the screens end, a mural passage from it along the hall's side to the dais end
    /// (between the hall and the strip of chambers beside a narrower hall, else along the long wall
    /// that runs unbroken), on along the end to the landing; rooms over the service end and beside
    /// the landing. None where no wall runs unbroken (the floor is laid as any other).
    fn void_floor(&mut self, lay: &mut Lay, rng: &mut Pcg32, more: usize) -> Option<Vec<i16>> {
        let (l, w, dz) = (self.f.len as isize, self.f.wid as isize, self.dz as isize);
        let (ht0, ht1) = (self.hall_t.0 as isize, self.hall_t.1 as isize);
        let (tl0, tl1) = (self.land_t.0 as isize, self.land_t.1 as isize);
        let g0 = if self.screens { self.service } else { self.service.min(1) } as isize;
        let h0 = (self.service + self.screens as usize) as isize;
        let gd = if dz - h0 >= 7 { 2 } else { 1 };
        let g1 = (h0 + gd - 1).max(g0 + 1).min(dz - 2);
        let strip = ht1 < w;
        let sides: Vec<isize> = if strip { vec![ht1] } else { vec![w - 1, 0] };
        let turn = |pt: isize| if pt >= tl1 { (tl1, pt + 1) } else if pt < tl0 { (pt, tl0) } else { (pt, pt + 1) };
        let (pt, e) = self.passage_side(&sides, dz, g1 - 1, &turn, false)?;
        if e > g1 - 1 {
            return None;
        }
        self.side = pt;
        let gallery = lay.room("minstrels' gallery");
        self.fill(lay, g0 as usize, g1 as usize, 0, w as usize, gallery);
        let mut free = Vec::new();
        let mut lines = Vec::new();
        if g0 >= 2 {
            // (Their walls off those of the service end below; each opening onto the gallery in the
            // middle of its side of it. Laid out again (as the rooms below), one room over it all
            // where that is sound.)
            let mut kinds = if w >= 7 { vec![("buttery", 1.0), ("kitchen", 1.6), ("pantry", 1.0)] } else { vec![("kitchen", 1.6), ("buttery", 1.0)] };
            lines.extend(self.service_cols(&mut kinds).into_iter().map(|t| (false, t as isize)));
            let whole: Vec<usize> = self.open_squares(lay, 0, g0, (0, w), -1);
            let rooms = if more > 0 && whole.len() <= 40 && self.sound(&whole, 4) {
                let r = lay.room("chamber");
                for &k in &whole {
                    lay.cells[k] = r;
                }
                vec![r]
            } else {
                self.partition(lay, [0, g0, 0, w], 1, 1, &mut lines, rng)
            };
            for r in rooms {
                let on: Vec<isize> = (0..w).filter(|&t| self.body(g0 - 1, t).is_some_and(|k| lay.cells[k] == r)).collect();
                if let Some(&t) = on.get(on.len() / 2) {
                    self.door(lay, (g0 - 1, t), (g0, t));
                }
                free.push(r);
            }
        } else {
            self.fill(lay, 0, g0 as usize, 0, w as usize, gallery);
        }
        let passage = lay.room("mural passage");
        self.fill(lay, g1 as usize, dz as usize, pt as usize, pt as usize + 1, passage);
        let (a, b) = turn(pt);
        self.fill(lay, dz as usize, dz as usize + 1, a as usize, b as usize, passage);
        // (A wing joining the hall's other side: a passage along that wall too, its way in.)
        let other = if pt == 0 { w - 1 } else { 0 };
        if !strip && self.wings.iter().any(|wg| wg.far == (other == w - 1) && (wg.s0 as isize) < dz && (wg.s1 as isize) > g1) {
            let p = lay.room("mural passage");
            self.fill(lay, g1 as usize, dz as usize, other as usize, other as usize + 1, p);
        }
        let void = lay.room(OPEN_BELOW);
        for s in g1..dz {
            for t in ht0..ht1 {
                if let Some(k) = self.body(s, t).filter(|&k| lay.cells[k] < 0) {
                    lay.cells[k] = void;
                }
            }
        }
        if strip && w - pt - 1 >= 2 {
            free.extend(self.partition(lay, [g1, l, pt + 1, w], 2, 1, &mut lines, rng));
        }
        let landing = lay.room("landing");
        self.fill(lay, dz as usize, l as usize, tl0 as usize, tl1 as usize, landing);
        // (The landing's side away from the passage: a chamber; part of the landing where that is
        // no more than 12 squares, not the room under it again.)
        let (c0, c1) = if pt < tl0 { (tl1, w) } else { (0, tl0) };
        if c1 - c0 >= 2 {
            let side = self.squares(dz, l, c0, c1) as isize;
            let r = if lay.count(landing) as isize + side <= 12 { landing } else { lay.room("chamber") };
            self.fill(lay, dz as usize, l as usize, c0 as usize, c1 as usize, r);
            if r != landing {
                free.push(r);
            }
        }
        Some(free)
    }

    /// The body's top: open battlements where its code is 0, else a parapet walk a square wide
    /// inside its walls (widening round a tower's notch and the main stair's head) round the roof,
    /// one box (hipped, as the map draws it).
    fn top_level(&self, lay: &mut Lay) {
        let (nx, w) = (self.nx, self.f.wid as isize);
        let mut deck = -1;
        let walk = lay.room("wall walk");
        let st = self.stairs;
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        for tp in self.top.clone() {
            if tp.code == 0 {
                if deck < 0 {
                    deck = lay.room("battlements");
                }
                self.fill(lay, tp.s0, tp.s1, 0, w as usize, deck);
                continue;
            }
            let (s0, s1) = (tp.s0 as isize, tp.s1 as isize);
            let near = |s: isize, t: isize, f: &dyn Fn(Option<usize>) -> bool| (-1..=1).any(|dt| (-1..=1).any(|ds| (ds, dt) != (0, 0) && f(self.at(s + ds, t + dt).filter(|&k| self.piece[k] == Piece::Body))));
            let rim = |k: usize| {
                let (s, t) = self.f.local((k % nx) as isize, (k / nx) as isize);
                near(s, t, &|k| k.is_none()) || near(s, t, &|k| k.is_some_and(on_stairs)) || on_stairs(k) || (tp.walk_lo && s == s0) || (tp.walk_hi && s + 1 == s1)
            };
            let sq: Vec<usize> = (s0..s1).flat_map(|s| (0..w).filter_map(move |t| self.body(s, t))).collect();
            self.roofed(lay, &sq, &rim, walk);
        }
    }

    /// A roofed top's squares `sq`: its roof the largest box of them clear of the walk (`rim`), 2
    /// squares across at least (hipped, as the map draws it); the rest (all of them where no such
    /// box is) the walk.
    fn roofed(&self, lay: &mut Lay, sq: &[usize], rim: &dyn Fn(usize) -> bool, walk: i16) {
        let nx = self.nx;
        let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
        for &k in sq {
            (x0, x1, y0, y1) = (x0.min(k % nx), x1.max(k % nx), y0.min(k / nx), y1.max(k / nx));
        }
        if sq.is_empty() {
            return;
        }
        let (rows, cols) = (y1 - y0 + 1, x1 - x0 + 1);
        let mut clear = vec![false; rows * cols];
        for &k in sq {
            clear[(k / nx - y0) * cols + k % nx - x0] = !rim(k);
        }
        let mut sum = vec![0usize; (rows + 1) * (cols + 1)];
        for r in 0..rows {
            for c in 0..cols {
                sum[(r + 1) * (cols + 1) + c + 1] = sum[r * (cols + 1) + c + 1] + sum[(r + 1) * (cols + 1) + c] - sum[r * (cols + 1) + c] + clear[r * cols + c] as usize;
            }
        }
        let full = |r0: usize, r1: usize, c0: usize, c1: usize| sum[r1 * (cols + 1) + c1] + sum[r0 * (cols + 1) + c0] - sum[r0 * (cols + 1) + c1] - sum[r1 * (cols + 1) + c0] == (r1 - r0) * (c1 - c0);
        let mut best = (0, [0usize; 4]);
        for r0 in 0..rows {
            for r1 in r0 + 2..=rows {
                for c0 in 0..cols {
                    for c1 in c0 + 2..=cols {
                        let a = (r1 - r0) * (c1 - c0);
                        if a > best.0 && full(r0, r1, c0, c1) {
                            best = (a, [r0, r1, c0, c1]);
                        }
                    }
                }
            }
        }
        let [r0, r1, c0, c1] = best.1;
        let roof = if best.0 >= 4 { lay.room(ROOF_SLOPE) } else { walk };
        for &k in sq {
            let (r, c) = (k / nx - y0, k % nx - x0);
            lay.cells[k] = if r >= r0 && r < r1 && c >= c0 && c < c1 { roof } else { walk };
        }
    }

    /// A wing's storey: chambers off a mural passage along its outer wall from the body when it is
    /// 4 squares across or more (on its other side, `flip`ped), else one or two rooms opening off
    /// each other from the body.
    fn wing_storey(&self, lay: &mut Lay, wi: usize, flip: bool, rng: &mut Pcg32) -> Vec<i16> {
        let wg = self.wings[wi].clone();
        let local = |k: usize| self.f.local((k % self.nx) as isize, (k / self.nx) as isize);
        let w = self.f.wid as isize;
        // (Outward from the body, from its first row.)
        let out = |t: isize| if wg.far { (t - wg.t0) as usize } else { (wg.t1 - 1 - t) as usize };
        let across = wg.s1 - wg.s0;
        let mut free = Vec::new();
        let mut cols: (usize, usize) = (wg.s0, wg.s1);
        if across >= 4 {
            // (Its passage on the side whose square of the body is circulation, if either is.)
            let joint = |s: usize| self.body(s as isize, if wg.far { w - 1 } else { 0 }).map(|k| lay.cells[k]).filter(|&r| r >= 0);
            let circ = |s: usize| joint(s).is_some_and(|r| is_circulation(lay.rooms[r as usize].kind) || matches!(lay.rooms[r as usize].kind, "great hall" | "mess hall"));
            let ps = if !circ(wg.s0) && circ(wg.s1 - 1) { wg.s1 - 1 } else { wg.s0 };
            let ps = if flip { wg.s0 + wg.s1 - 1 - ps } else { ps };
            let p = lay.room("mural passage");
            for &k in &wg.squares {
                if local(k).0 as usize == ps {
                    lay.cells[k] = p;
                }
            }
            cols = if ps == wg.s0 { (wg.s0 + 1, wg.s1) } else { (wg.s0, wg.s1 - 1) };
        }
        // (Rooms across it, each as deep as 2 squares and its width over 2.5, as 2.5 times its width
        // or 40 squares at most; one too wide for its depth in two.)
        let width = (cols.1 - cols.0).max(1);
        let lmin = ((2 * width + 4) / 5).max(2).min(wg.deep);
        let lmax = ((5 * width) / 2).min(40 / width).max(lmin);
        let n = if across >= 4 { (crate::core::round(wg.deep as f64 / 3.5) as usize).clamp(1, 3) } else { 2.min(wg.deep / 2).max(1) } + (wg.deep >= 4 && rng.next_f64() < 0.5) as usize;
        let n = n.max(wg.deep.div_ceil(lmax)).min((wg.deep / lmin).max(wg.deep.div_ceil(lmax)).max(1));
        let mut ls = Vec::new();
        for _ in 0..8 {
            ls = cuts(wg.deep, n, lmin, rng);
            if ls.iter().all(|&x| x <= lmax) {
                break;
            }
        }
        let mut b = 0;
        for l in ls {
            let halves = if 2 * width > 5 * l { vec![(cols.0, cols.0 + width / 2), (cols.0 + width / 2, cols.1)] } else { vec![cols] };
            for (c0, c1) in halves {
                let r = lay.room("chamber");
                for &k in &wg.squares {
                    let (s, t) = local(k);
                    let o = out(t);
                    if (s as usize) >= c0 && (s as usize) < c1 && o >= b && o < b + l && lay.cells[k] < 0 {
                        lay.cells[k] = r;
                    }
                }
                free.push(r);
            }
            b += l;
        }
        // (Any square left: the room beside it.)
        for &k in &wg.squares {
            if lay.cells[k] < 0
                && let Some(&r) = free.last()
            {
                lay.cells[k] = r;
            }
        }
        free
    }

    /// A wing's top: open battlements, or a parapet walk along its outer walls round a roof
    /// (`roofed`).
    fn wing_top(&self, lay: &mut Lay, wi: usize) {
        let wg = &self.wings[wi];
        let code = self.wing_tops.get(wi).copied().unwrap_or(0);
        let (nx, ny) = (self.nx, self.ny);
        let own = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && matches!(self.piece[j as usize * nx + i as usize], Piece::Body | Piece::Wing(_) | Piece::Fore);
        if code == 0 {
            let d = lay.room("battlements");
            for &k in &wg.squares {
                lay.cells[k] = d;
            }
            return;
        }
        let walk = lay.room("wall walk");
        let edge = |k: usize| {
            let (i, j) = ((k % nx) as isize, (k / nx) as isize);
            (-1..=1).any(|dj| (-1..=1).any(|di| (di, dj) != (0, 0) && !own(i + di, j + dj)))
        };
        self.roofed(lay, &wg.squares, &edge, walk);
    }

    /// The forebuilding: its entrance hall, the chapel above it, a walled platform on top.
    fn fore_level(&mut self, lay: &mut Lay, z: i8) {
        let kind = if z >= self.fore_floors {
            if z > self.fore_floors {
                return;
            }
            "battlements"
        } else if z == 0 {
            "entrance hall"
        } else if z == 1 {
            "chapel"
        } else {
            "chamber"
        };
        let r = lay.room(kind);
        for &k in &self.fore {
            lay.cells[k] = r;
        }
    }

    /// The kinds of the tower rooms standing on floor `z` (the keep's towers, then a grid's corner
    /// towers): guard posts, stores and an armory on the ground; chambers, closets, a study and an
    /// oratory (one) above (a barracks', barrack rooms and stores); a garrison or guardroom in the
    /// storey under each tower's cone. Shuffled per floor, so that no two towers on it are alike
    /// but where there are more towers than kinds.
    fn tower_kinds(&mut self, z: i8) -> (Vec<(usize, &'static str)>, Vec<&'static str>) {
        let mut rng = Pcg32::new(hash3(self.seed, z as i64, 0x7c, 0), 85);
        let mut shuffled = |pool: &[&'static str]| {
            let mut p = pool.to_vec();
            for k in (1..p.len()).rev() {
                p.swap(k, rng.below(k as u32 + 1) as usize);
            }
            p
        };
        let low = shuffled(&["guard post", "storeroom", "armory", "guard post"]);
        let mid = if self.kind == Kind::Barracks {
            shuffled(&["barrack room", "storeroom", "armory", "barrack room"])
        } else {
            let mut p = shuffled(&["chamber", "wardrobe", "study", "guest chamber", "storeroom", "oratory", "chamber"]);
            p.retain(|k| *k != "oratory" || !self.oratory);
            p
        };
        let top = shuffled(&["garrison", "guardroom", "storeroom", "armory"]);
        let (mut a, mut b, mut c) = (0, 0, 0);
        let mut towers = Vec::new();
        for (ti, tt) in self.tower_top.clone().into_iter().enumerate() {
            if z > tt {
                continue;
            }
            let kind = if z == 0 {
                a += 1;
                low[(a - 1) % low.len()]
            } else if z == tt {
                c += 1;
                top[(c - 1) % top.len()]
            } else {
                b += 1;
                mid[(b - 1) % mid.len()]
            };
            self.oratory |= kind == "oratory";
            towers.push((ti, kind));
        }
        let corners = (0..4).map(|k| if z == 0 { low[k % low.len()] } else { mid[k % mid.len()] }).collect();
        (towers, corners)
    }

    /// A small tower keep's storey (`small_plan`): the landing across the high end with the main
    /// stair against the end wall; on the ground an entrance hall from it down the body's high half
    /// (the front door on its walls) and the stores below it (an armory and a storeroom side by side
    /// where they fit); the great hall over them; above, the storey's main room by the landing and
    /// a second room at the low end (a band across it where both hold 9 squares, else a corner of 2
    /// by 2, at the other corner on the next storey): the great chamber and the chapel, the solar and
    /// the treasury, a garrison, then chambers and stores.
    fn small_level(&mut self, z: i8, mut lay: Lay) -> Laid {
        let (l, w, dz) = (self.f.len as isize, self.f.wid as isize, self.dz as isize);
        let (tl0, tl1) = (self.land_t.0 as isize, self.land_t.1 as isize);
        let mut name = "Upper floor";
        let mut public = None;
        let way = lay.room(if z == 0 { "entrance hall" } else { "landing" });
        self.fill(&mut lay, dz as usize, l as usize, 0, w as usize, way);
        // (A door from the room in front of the landing onto it, off the stair's columns.)
        let door_col = [tl0 - 1, tl1, tl0].into_iter().find(|&t| t >= 0 && t < w && self.body(dz - 1, t).is_some()).unwrap_or(0);
        let rest = dz as usize;
        if z == 0 {
            name = "Stores";
            // The entrance hall down the high half, the stores below.
            let a = (rest / 2).max(2).min(rest);
            self.fill(&mut lay, a, rest, 0, w as usize, way);
            if a >= 2 && w >= 4 {
                let half = w as usize / 2;
                for (kind, t0, t1) in [("armory", 0, half), ("storeroom", half, w as usize)] {
                    let r = lay.room(kind);
                    self.fill(&mut lay, 0, a, t0, t1, r);
                    self.door(&mut lay, (a as isize - 1, ((t0 + t1) / 2) as isize), (a as isize, ((t0 + t1) / 2) as isize));
                }
            } else if a >= 1 {
                let r = lay.room("armory");
                self.fill(&mut lay, 0, a, 0, w as usize, r);
            }
            self.pending.retain(|k| *k != "armory");
            let mut e = Vec::new();
            for s in (a as isize..l).rev() {
                for (t, dt) in [(0isize, -1isize), (w - 1, 1)] {
                    if self.body(s, t).is_some() {
                        let (p, q) = (self.f.grid(s, t), self.f.grid(s, t + dt));
                        e.push((side(p, q), p.0 as usize, p.1 as usize));
                    }
                }
            }
            if !self.ends[1] {
                e.reverse();
            }
            lay.hints.front = Some((way, e));
        } else if z == self.hall_z {
            name = "Hall";
            let hall = lay.room("great hall");
            self.fill(&mut lay, 0, rest, 0, w as usize, hall);
            self.door(&mut lay, (dz - 1, door_col), (dz, door_col));
            public = Some("great hall");
        } else {
            let n = (z - self.hall_z) as usize;
            if n == 1 {
                name = "Lord's floor";
            }
            // (The storey's two kinds: the first of the lord's rooms still due (a garrison, then
            // chambers after them) by the landing; at the low end the chapel, a treasury, a garrison,
            // then stores.)
            let due = |k: &str| self.pending.contains(&k);
            let main = ["great chamber", "solar", "garrison"].into_iter().find(|&k| due(k)).unwrap_or("chamber");
            let low = [("chapel", !self.chapel), ("treasury", due("treasury")), ("garrison", due("garrison") && main != "garrison")].into_iter().find(|x| x.1).map_or(if n % 2 == 0 { "storeroom" } else { "wardrobe" }, |x| x.0);
            let big = lay.room(main);
            self.fill(&mut lay, 0, rest, 0, w as usize, big);
            let total = self.squares(0, rest as isize, 0, w);
            // The second room: a band across the low end where both are sound (9 squares, no
            // longer than 2.5:1), else a corner of 2 by 2 leaving the main room 9 squares; else none
            // (its kind on a later storey). The band on even storeys, the corner at the other end
            // of the low wall each odd one.
            let fine = |n: usize, a: usize, b: usize| n >= 9 && 2 * a.max(b) <= 5 * a.min(b);
            let band = (1..rest).rev().find(|&k| fine(self.squares(0, k as isize, 0, w), k, w as usize) && fine(self.squares(k as isize, rest as isize, 0, w), rest - k, w as usize));
            let corner = (total >= 13).then(|| if (n / 2) % 2 == 0 { 0 } else { w as usize - 2 });
            let placed = match (band.filter(|_| n % 2 == 0 || corner.is_none()), corner) {
                (Some(k), _) => {
                    let second = lay.room(low);
                    self.fill(&mut lay, 0, k, 0, w as usize, second);
                    true
                }
                (None, Some(c0)) => {
                    let second = lay.room(low);
                    self.fill(&mut lay, 0, 2, c0, c0 + 2, second);
                    true
                }
                _ => false,
            };
            let low = if placed { low } else { main };
            self.door(&mut lay, (dz - 1, door_col), (dz, door_col));
            self.chapel |= low == "chapel";
            self.pending.retain(|k| *k != main && *k != low);
        }
        // (Squares no room took: the room beside them.)
        let mut free = Vec::new();
        self.leftovers(&mut lay, &mut free);
        let here: Vec<&'static str> = (0..lay.rooms.len()).filter(|&r| lay.count(r as i16) > 0).map(|r| lay.rooms[r].kind).collect();
        self.chapel |= here.contains(&"chapel");
        self.pending.retain(|k| !here.contains(k));
        let hall = (z == self.hall_z).then(|| self.hall_frame());
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public, name, roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), hall }
    }

    /// A shell keep's floor: lean-tos round the round yard (a gate passage through them, a well in
    /// the yard, the stairs' landing) on the ground; rooms round a gallery over the yard above (the
    /// lean-tos' inner row; the stair's landing off it): the great chamber and solar, a treasury,
    /// an armory, a garrison; the wall walk round the lean-tos' roof on top.
    fn shell_level(&mut self, z: i8, mut lay: Lay) -> Option<Laid> {
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
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name, roof, items: lay.items, towers: Vec::new(), corners: Vec::new(), hall: None })
    }
}
