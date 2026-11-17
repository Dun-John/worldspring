//! Inns and caravanserais by zones (from `LEAST` ground-floor squares; below it a small inn keeps
//! its taproom, kitchen and storeroom, `super::super::program`), on the box of their footprint's
//! squares (`civic::frames`), from the street front (s, the frame's depth from it) and across it
//! (t):
//! - the ground floor: the front range the taproom (the front door into it, the bar and the tables
//!   there: the drinking rules), a snug in one of its front corners (2 or 3 squares across, where
//!   the corner is all the footprint's), a private dining room in the other where the front is too
//!   wide for the taproom alone (over 2.5 times its depth); the stairs in the taproom against its
//!   back wall (as every inn's are: guests come in and go up through it). Behind it the service
//!   range, rooms cut across it of very different widths (`run`): the kitchen, the private dining
//!   room (where it is not in front), the storeroom, the snug (where the front had no room), a
//!   caravanserai's stables and a buttery where there is room; the scullery in the kitchen's back
//!   corner (off the kitchen, not the taproom). A service range 5 deep or more has two rows: behind
//!   the kitchen its scullery, behind the private dining room a pantry, behind the stables the tack
//!   room; the rest of the row the room before it's (a storeroom behind one it would make over
//!   2.5:1). A building 14 deep or more and deeper than wide has a passage on from the taproom to
//!   its back instead, the kitchen, scullery, pantry and buttery down one side, the private dining
//!   room, stores and stables down the other (`inn_passage`);
//! - the guest floors: a corridor over the stairs the length of the building (along s where it is
//!   deeper than it is wide, at its middle column, or by its side wall where it is 6 across or
//!   less; else across it, along the taproom's back row), rooms either side of it cut across into
//!   rooms of a few sizes (`run`; a side over 6 deep parted, `partition`), tidied and varied
//!   (`kit`): on the first floor one or two best chambers (a big bed and a hearth), on the top floor
//!   the innkeeper's quarters and the common dormitory of cots, the rest guest rooms;
//! - the cellar as every inn's (the keg cellar).
//!
//!
//! A coaching inn (every caravanserai and the biggest inns, their lots merged by `town::palace` to a
//! rectangle of `YARD` or more: `town::palace::is_coaching`) stands instead round a galleried yard
//! (`Yard`, decision 4): the front range the private dining room in the corner on the arch's side,
//! the carriage arch (2 squares wide, open at both ends: a door of kind "arch" onto the street, no
//! wall onto the yard, the arch's squares the yard's), the taproom (the front door, the bar, the
//! stairs against its back wall by the yard) and the snug in its far corner; the side range by the
//! arch the stables (their stalls opening onto the yard) and the tack room, the other the kitchen,
//! scullery and pantry; the back range the stores (a caravanserai's more stables; a one-storey
//! caravanserai's guest rooms). The yard is cobbled and open to the sky but for its edge squares,
//! which the gallery stands over above (a walkway railed onto the yard, `yard gallery`): every
//! room above opens off it, the best chambers on the front, the dormitory and the innkeeper's
//! quarters on the top floor. Where no frame holds the yard it falls back to the plan above.

use super::civic::{bbox, frames};
use super::kit::{Lay, side};
use super::{Frame, Kind, Kinds, Laid, Piece, Plan};
use crate::core::rng::Pcg32;
use crate::interior::Shell;
use crate::town::{self, geom::P};

/// The least ground-floor squares an inn or a caravanserai is laid out by zones with, and the least
/// share of its box its squares fill.
pub const LEAST: usize = 60;
pub const FILL: f64 = 0.8;
/// The widest and the deepest box an inn is laid out by zones on (squares).
pub const WIDEST: usize = 20;
pub const DEEPEST: usize = 24;

/// An inn's zones in its frame.
#[derive(Clone, Debug)]
pub(super) struct Zones {
    /// The taproom's depth from the front (rows 0..tap).
    pub(super) tap: usize,
    /// The service range's rows' depths behind it (the near row first).
    pub(super) back: Vec<usize>,
    /// The snug in a front corner: t0..t1, rows 0..depth; None where the front has no room for it.
    pub(super) snug: Option<(usize, usize, usize)>,
    /// The private dining room in the other front corner (t0..t1, rows 0..tap), where the front is
    /// too wide for the taproom.
    pub(super) dining: Option<(usize, usize)>,
    /// The guest floors' corridor along s (column `line`), else across (row `line`).
    pub(super) along_s: bool,
    pub(super) line: usize,
    /// The stairs' first square (s, t; the second at s + 1 along s, else t + 1).
    pub(super) stairs: (usize, usize),
    /// A passage on from the taproom down the corridor's line (s `tap` to the back), the service
    /// range's rooms either side of it (a deep building's).
    pub(super) passage: bool,
    /// A coaching inn's galleried yard (`Yard`): the plan laid out round it instead.
    pub(super) yard: Option<Yard>,
}

/// An inn's or a caravanserai's kind, if it is laid out by zones: `LEAST` squares or more filling
/// `FILL` of their box, on a frame its zones and stairs fit (`fit`) with the taproom's front door
/// onto open ground.
pub(super) fn kind(b: &town::Building, key: Option<&str>, sh: &Shell) -> Option<Kind> {
    let kind = match key? {
        "inn" => Kind::Inn,
        "caravanserai" => Kind::Caravanserai,
        _ => return None,
    };
    if !b.parts.is_empty() || b.keep.is_some() || sh.nx == 0 {
        return None;
    }
    let (x0, x1, y0, y1, m) = bbox(&sh.inside, sh.nx)?;
    if m < LEAST || (m as f64) < FILL * ((x1 - x0 + 1) * (y1 - y0 + 1)) as f64 {
        return None;
    }
    let ground = |i: isize, j: isize| sh.ext_free(i, j);
    // (A coaching inn round its yard where its footprint holds one, `Yard`.)
    if sh.coaching && frames(sh).iter().any(|f| [false, true].iter().any(|&flip| yard_fit(sh, f, flip, &ground).is_some())) {
        return Some(kind);
    }
    frames(sh).iter().any(|f| [false, true].iter().any(|&flip| fit(sh, f, flip).is_some_and(|z| door_out(sh, f, &z, &ground)))).then_some(kind)
}

fn inside(sh: &Shell, (i, j): (isize, isize)) -> bool {
    i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny && sh.inside[j as usize * sh.nx + i as usize]
}

/// The taproom's front squares (t) that open onto open ground (`ground`), the middle first.
fn door_ts(sh: &Shell, f: &Frame, z: &Zones, ground: &dyn Fn(isize, isize) -> bool) -> Vec<isize> {
    let (lo, hi) = tap_ts(z, f.wid, 0);
    let mid = (lo + hi) as isize / 2;
    let mut ts: Vec<isize> = (lo as isize..hi as isize).collect();
    ts.sort_by_key(|&t| ((t - mid).abs(), t));
    ts.into_iter().filter(|&t| inside(sh, f.grid(0, t)) && ground(f.grid(-1, t).0, f.grid(-1, t).1)).collect()
}

fn door_out(sh: &Shell, f: &Frame, z: &Zones, ground: &dyn Fn(isize, isize) -> bool) -> bool {
    !door_ts(sh, f, z, ground).is_empty()
}

/// The taproom's columns on row `s` (t0..t1): the front less the snug (on its rows) and the
/// private dining room.
fn tap_ts(z: &Zones, w: usize, s: usize) -> (usize, usize) {
    let (mut lo, mut hi) = (0, w);
    for (t0, t1) in z.dining.into_iter().chain(z.snug.filter(|sn| s < sn.2).map(|sn| (sn.0, sn.1))) {
        if t0 == 0 {
            lo = lo.max(t1);
        } else {
            hi = hi.min(t0);
        }
    }
    (lo, hi)
}

/// The zones on frame `f` (`zones`) where they fit its footprint: every row of the service range
/// and the private dining room in front 80% the footprint's, the snug all inside (else behind),
/// the guest floors' corridor all inside, the stairs on whole squares.
fn fit(sh: &Shell, f: &Frame, flip: bool) -> Option<Zones> {
    let at = |s: usize, t: usize| inside(sh, f.grid(s as isize, t as isize));
    // (The snug in the front corner where all its squares are the footprint's, else behind.)
    let z = zones(f.len, f.wid, flip, true).filter(|z| z.snug.is_none_or(|(t0, t1, d)| (0..d).all(|s| (t0..t1).all(|t| at(s, t)))));
    let z = z.or_else(|| zones(f.len, f.wid, flip, false))?;
    // (The private dining room in front 80% the footprint's: else the other corner.)
    if let Some((t0, t1)) = z.dining {
        let n = (0..z.tap).map(|s| (t0..t1).filter(|&t| at(s, t)).count()).sum::<usize>();
        if 5 * n < 4 * z.tap * (t1 - t0) {
            return None;
        }
    }
    let whole = |s: usize, t: usize| {
        let (i, j) = f.grid(s as isize, t as isize);
        i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny && sh.whole[j as usize * sh.nx + i as usize]
    };
    let full = |n: usize, of: usize| 5 * n >= 4 * of;
    let rows = (z.tap..f.len).all(|s| full((0..f.wid).filter(|&t| at(s, t)).count(), f.wid));
    // (The corridor's squares inside one run, 80% of its line or more: a slanted wall may cut its
    // ends off.)
    let on: Vec<bool> = if z.along_s { (0..f.len).map(|s| at(s, z.line)).collect() } else { (0..f.wid).map(|t| at(z.line, t)).collect() };
    let (first, last) = (on.iter().position(|&x| x), on.iter().rposition(|&x| x));
    let line = first.zip(last).is_some_and(|(a, b)| on[a..=b].iter().all(|&x| x) && full(b + 1 - a, on.len()));
    let (s, t) = z.stairs;
    let second = if z.along_s { (s + 1, t) } else { (s, t + 1) };
    (rows && line && whole(s, t) && whole(second.0, second.1)).then_some(z)
}

/// The zones for a frame `l` deep from the front and `w` across: the service range about 0.45 of
/// the depth (3 to 6 rows, two rows from 5), the taproom the rest (3 deep or more); a building 14
/// deep or more, deeper than it is wide, its taproom 0.4 of it (5 to 8 deep) and a passage on down
/// the guest floors' corridor's line through the service range, its rooms either side of it; the
/// snug (`snug`) 3 across from a front of 8 (2 from 6), 3 deep (the taproom's depth where it is 4 or
/// less); the private dining room beside the taproom where the front is over 2.5 times its depth;
/// the taproom's box at most 2.5 to 1. None where they don't fit.
fn zones(l: usize, w: usize, flip: bool, snug: bool) -> Option<Zones> {
    if !(5..=WIDEST).contains(&w) || !(6..=DEEPEST).contains(&l) {
        return None;
    }
    let passage = l >= w && l >= 14;
    let b = if passage { l - ((2 * l + 2) / 5).clamp(5, 8) } else { ((9 * l + 10) / 20).clamp(3, 6) };
    let tap = l - b;
    if tap < 3 {
        return None;
    }
    let back = if b >= 5 && !passage { vec![3, b - 3] } else { vec![b] };
    let sw = if !snug { 0 } else if w >= 8 { 3 } else if w >= 6 { 2 } else { 0 };
    // (The snug the front range's depth where the front is too wide for the taproom beside it.)
    let wide = 2 * w > 5 * tap;
    let sd = if tap <= 4 || (wide && 2 * tap <= 5 * sw) { tap } else { 3 };
    let snug = (sw > 0).then_some(if flip { (0, sw, sd) } else { (w - sw, w, sd) });
    let mut bw = if sd < tap { w } else { w - sw };
    let mut dining = None;
    if 2 * bw > 5 * tap {
        let dw = (bw - (5 * tap) / 2).max(3);
        if dw > 7 || 2 * tap > 5 * dw || bw < dw + 4 {
            return None;
        }
        bw -= dw;
        dining = Some(if flip { (w - dw, w) } else { (0, dw) });
    }
    if 2 * bw.max(tap) > 5 * bw.min(tap) {
        return None;
    }
    let mut z = Zones { tap, back, snug, dining, along_s: l >= w, line: 0, stairs: (0, 0), passage, yard: None };
    if z.along_s {
        z.line = if w <= 6 { if flip { w - 1 } else { 0 } } else { (w - 1) / 2 };
        // (Clear of the passage's way in where there is one.)
        let s0 = tap - if passage { 3 } else { 2 };
        z.stairs = (s0, z.line);
        // (Both stairs' squares the taproom's.)
        for s in s0..s0 + 2 {
            let (lo, hi) = tap_ts(&z, w, s);
            if z.line < lo || z.line >= hi {
                return None;
            }
        }
    } else {
        let (lo, hi) = tap_ts(&z, w, tap - 1);
        if hi < lo + 2 || tap < 3 {
            return None;
        }
        z.line = tap - 1;
        z.stairs = (tap - 1, (lo + hi) / 2 - 1);
    }
    Some(z)
}

/// An inn's plan: on the frame that fits (`fit`) with its front nearest a street, the taproom's
/// front door onto open ground; its zones, its stairs. None where no frame fits.
pub(super) fn plan(sh: &Shell, kind: Kind, floors: i8, seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let pt = |f: &Frame, s: f64, t: f64| {
        let (i, j) = f.grid(0, 0);
        let c = [i as f64 + 0.5 - 0.5 * (f.ds.0 + f.dt.0) as f64, j as f64 + 0.5 - 0.5 * (f.ds.1 + f.dt.1) as f64];
        [c[0] + f.ds.0 as f64 * s + f.dt.0 as f64 * t, c[1] + f.ds.1 as f64 * s + f.dt.1 as f64 * t]
    };
    // (A coaching inn round its yard: its front nearest a street, the arch's side by the seed.)
    if sh.coaching {
        let best = frames(sh)
            .into_iter()
            .enumerate()
            .flat_map(|(k, f)| {
                let first = ((seed >> 9) ^ k as u64) & 1 == 1;
                [first, !first].into_iter().enumerate().filter_map(move |(n, flip)| {
                    let y = yard_fit(sh, &f, flip, ground)?;
                    let d = near(pt(&f, 0.0, 0.5 * f.wid as f64));
                    Some(((d >= 80.0, crate::core::round(d) as i64, (k as u64 & 1) ^ ((seed >> 7) & 1), k, n), f, y))
                })
            })
            .min_by_key(|x| x.0);
        if let Some((_, f, y)) = best {
            let w = f.wid;
            let st = y.stairs(w) as isize;
            let (a, e) = (f.grid(y.front as isize - 1, st), f.grid(y.front as isize - 1, st + 1));
            let stairs = [a.0.min(e.0) as usize, a.1.min(e.1) as usize, a.0.abs_diff(e.0) + 1, a.1.abs_diff(e.1) + 1];
            let piece = sh.inside.iter().map(|&i| if i { Piece::Body } else { Piece::Out }).collect();
            let mut p = Plan::bare(kind, nx, ny, f, piece, stairs, floors, seed);
            let (sw, sd) = y.snug;
            let snug = (sw > 0).then(|| {
                let (t0, t1) = y.cols(w, w - sw, w);
                (t0, t1, sd)
            });
            let (d0, d1) = y.cols(w, 0, y.side);
            let line = y.front - 1;
            p.inn = Some(Zones { tap: y.front, back: vec![f.len - y.front], snug, dining: Some((d0, d1)), along_s: false, line, stairs: (line, st as usize), passage: false, yard: Some(y) });
            return Some(p);
        }
    }
    let (_, f, z) = frames(sh)
        .into_iter()
        .enumerate()
        .flat_map(|(k, f)| {
            // (The snug in the corner the seed picks first, else the other.)
            let first = ((seed >> 9) ^ k as u64) & 1 == 1;
            [first, !first].into_iter().enumerate().filter_map(move |(n, flip)| {
                let z = fit(sh, &f, flip)?;
                let door = door_out(sh, &f, &z, ground);
                let d = near(pt(&f, 0.0, 0.5 * f.wid as f64));
                Some(((!door, d >= 80.0, crate::core::round(d) as i64, (k as u64 & 1) ^ ((seed >> 7) & 1), k, n), f, z))
            })
        })
        .min_by_key(|x| x.0)?;
    let (s, t) = (z.stairs.0 as isize, z.stairs.1 as isize);
    let (a, e) = (f.grid(s, t), if z.along_s { f.grid(s + 1, t) } else { f.grid(s, t + 1) });
    let stairs = [a.0.min(e.0) as usize, a.1.min(e.1) as usize, a.0.abs_diff(e.0) + 1, a.1.abs_diff(e.1) + 1];
    let piece = sh.inside.iter().map(|&i| if i { Piece::Body } else { Piece::Out }).collect();
    let mut p = Plan::bare(kind, nx, ny, f, piece, stairs, floors, seed);
    p.inn = Some(z);
    Some(p)
}

/// Where each kind of the service range's near row goes along it (t from the end away from the
/// snug): stables, storeroom, scullery, kitchen, buttery, private dining room, snug.
fn order(kind: &str) -> usize {
    ["stables", "storeroom", "scullery", "kitchen", "buttery", "private dining room", "snug"].iter().position(|k| *k == kind).unwrap_or(1)
}

impl Plan {
    /// An inn's zones: the taproom's depth, the service range's rows, the snug (t0, t1, depth), the
    /// private dining room in front (t0, t1), the corridor (along s, its line).
    #[allow(clippy::type_complexity)]
    pub fn inn(&self) -> Option<(usize, Vec<usize>, Option<(usize, usize, usize)>, Option<(usize, usize)>, bool, usize)> {
        self.inn.as_ref().map(|z| (z.tap, z.back.clone(), z.snug, z.dining, z.along_s, z.line))
    }

    /// An inn's floor `z`: the ground floor (`inn_ground`), the guest floors (`inn_upper`); its
    /// cellar as every inn's.
    pub(super) fn inn_level(&mut self, z: i8, lay: Lay, mut rng: Pcg32) -> Option<Laid> {
        let zn = self.inn.clone()?;
        if let Some(y) = zn.yard {
            return match z {
                0 => Some(self.yard_ground(lay, y)),
                1.. => Some(self.yard_upper(lay, y, z)),
                _ => None,
            };
        }
        match z {
            0 => Some(self.inn_ground(lay, &zn)),
            1.. => Some(self.inn_upper(lay, &zn, z, &mut rng)),
            _ => None,
        }
    }

    /// The t columns room `r` holds on rows s0..s1 (t0, t1 inclusive).
    fn span_of(&self, lay: &Lay, r: i16, s0: usize, s1: usize) -> Option<(usize, usize)> {
        let mut out: Option<(usize, usize)> = None;
        for s in s0..s1 {
            for t in 0..self.f.wid {
                if self.at(s as isize, t as isize).is_some_and(|k| lay.cells[k] == r) {
                    out = Some(out.map_or((t, t), |(a, b)| (a.min(t), b.max(t))));
                }
            }
        }
        out
    }

    /// The ground floor: the taproom across the front (the snug and the private dining room in its
    /// corners), the service range behind (`Zones`).
    fn inn_ground(&mut self, mut lay: Lay, zn: &Zones) -> Laid {
        let (w, d) = (self.f.wid, zn.tap);
        let tap = self.put_box(&mut lay, "taproom", 0, d, 0, w);
        if let Some((t0, t1, sd)) = zn.snug {
            self.put_box(&mut lay, "snug", 0, sd, t0, t1);
        }
        if let Some((t0, t1)) = zn.dining {
            self.put_box(&mut lay, "private dining room", 0, d, t0, t1);
        }
        if zn.passage {
            self.inn_passage(&mut lay, zn);
        } else {
            self.inn_rows(&mut lay, zn);
        }
        // (The front door into the taproom, onto open ground at its front's middle first.)
        let (lo, hi) = tap_ts(zn, w, 0);
        let mid = (lo + hi) as isize / 2;
        self.front_door(&mut lay, tap, &[mid, mid - 1], lo as isize..hi as isize);
        // (Squares a slanted wall left no zone's: a room of their own where they make one, else the
        // room beside them's; rooms it left under 4 squares or a square wide the room beside them's.)
        let mut free = Vec::new();
        self.leftovers(&mut lay, &mut free);
        self.absorb(&mut lay, &mut free, 4);
        for r in free {
            if lay.count(r) > 0 && lay.rooms[r as usize].kind == "chamber" {
                lay.rooms[r as usize].kind = "storeroom";
            }
        }
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: Some("taproom"), name: "Taproom", roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls: Vec::new() }
    }

    /// The service range's kinds an inn needs, by need: the kitchen, the private dining room and the
    /// snug where the front had no room for them, the storeroom, the scullery, a caravanserai's
    /// stables (and tack room, `tack`), a pantry (`pantry`), a buttery (`buttery`).
    fn service(&self, zn: &Zones, scullery: bool, buttery: bool, pantry: bool, tack: bool) -> Vec<(&'static str, f64)> {
        let mut want: Vec<(&'static str, f64)> = vec![("kitchen", 1.7)];
        if zn.dining.is_none() {
            want.push(("private dining room", 1.4));
        }
        want.push(("storeroom", 1.0));
        if scullery {
            want.push(("scullery", 0.8));
        }
        if zn.snug.is_none() {
            want.push(("snug", 1.0));
        }
        if self.kind == Kind::Caravanserai {
            want.push(("stables", 1.8));
        }
        if pantry {
            want.push(("pantry", 0.8));
        }
        if tack && self.kind == Kind::Caravanserai {
            want.push(("tack room", 0.6));
        }
        if buttery {
            want.push(("buttery", 0.6));
        }
        want
    }

    /// A deep inn's service range: the passage on from the taproom to the back, the kitchen, its
    /// scullery and pantry down one side of it (each opening off the kitchen where it can), the
    /// private dining room, the stores and the stables down the other.
    fn inn_passage(&self, lay: &mut Lay, zn: &Zones) {
        let (l, w, d, c) = (self.f.len, self.f.wid, zn.tap, zn.line);
        self.put_box(lay, "corridor", d, l, c, c + 1);
        let sides: Vec<(usize, usize)> = [(0, c), (c + 1, w)].into_iter().filter(|x| x.1 > x.0).collect();
        let cap = |t0: usize, t1: usize| ((l - d) / (2 * (t1 - t0)).div_ceil(5).max(2)).max(1);
        let rank = |k: &str| ["kitchen", "scullery", "pantry", "buttery", "private dining room", "snug", "storeroom", "stables", "tack room"].iter().position(|x| *x == k).unwrap_or(9);
        let want = self.service(zn, true, true, true, true);
        let mut all = Vec::new();
        if let [(t0, t1)] = sides[..] {
            let mut k = want;
            k.truncate(cap(t0, t1));
            k.sort_by_key(|x| rank(x.0));
            all.extend(self.run(lay, d, l, t0, t1, &k, "storeroom"));
        } else {
            // (The kitchen's side away from the snug.)
            let ks = usize::from(zn.snug.is_some_and(|s| s.0 == 0));
            let cook = |k: &str| matches!(k, "kitchen" | "scullery" | "pantry" | "buttery");
            for (n, &(t0, t1)) in sides.iter().enumerate() {
                let mut k: Vec<(&'static str, f64)> = want.iter().copied().filter(|x| cook(x.0) == (n == ks)).collect();
                k.truncate(cap(t0, t1));
                k.sort_by_key(|x| rank(x.0));
                all.extend(self.run(lay, d, l, t0, t1, &k, "storeroom"));
            }
        }
        let of = |lay: &Lay, kind: &str| all.iter().copied().find(|&r| lay.rooms[r as usize].kind == kind);
        for (a, b) in [("scullery", "kitchen"), ("pantry", "kitchen"), ("tack room", "stables")] {
            if let (Some(x), Some(y)) = (of(lay, a), of(lay, b)) {
                lay.hints.onto.push((x, y));
            }
        }
    }

    /// An inn's service range in rows across it: the near row's kinds by need, as many as it holds
    /// 2 squares or more across each, then in their order along it; the far row behind.
    fn inn_rows(&self, lay: &mut Lay, zn: &Zones) {
        let (l, w, d) = (self.f.len, self.f.wid, zn.tap);
        let near = zn.back[0];
        let two = zn.back.len() > 1;
        let mut want = self.service(zn, false, !two, false, false);
        let least = (2 * near).div_ceil(5).max(2);
        want.truncate((w / least).max(1));
        // (Away from the snug: the stables and stores at the far end, the private dining room and
        // a snug by the front's snug.)
        want.sort_by_key(|k| order(k.0));
        if zn.snug.is_some_and(|s| s.0 == 0) {
            want.reverse();
        }
        let rs = self.run(lay, d, d + near, 0, w, &want, "storeroom");
        let of = |lay: &Lay, kind: &str| rs.iter().copied().find(|&r| lay.rooms[r as usize].kind == kind);
        // (One row: the scullery in the kitchen's back corner, 2 rows deep and 2 or 3 across, the
        // kitchen round it to the taproom; a kitchen under 4 across the room beside it as its
        // scullery, the buttery.)
        if !two && let Some(k) = of(lay, "kitchen") {
            let (a, b) = self.span_of(lay, k, d, d + near).unwrap_or((0, 0));
            let kw = b + 1 - a;
            let sc = if near >= 3 && kw >= 4 {
                let n = if kw >= 6 { 3 } else { 2 };
                let (fa, fb) = if zn.snug.is_some_and(|s| s.0 == 0) { (b + 1 - n, b + 1) } else { (a, a + n) };
                Some(self.put_box(lay, "scullery", d + near - 2, d + near, fa, fb))
            } else {
                of(lay, "buttery").inspect(|&r| lay.rooms[r as usize].kind = "scullery")
            };
            if let Some(sc) = sc.filter(|&sc| lay.count(sc) > 0) {
                lay.hints.onto.push((sc, k));
            }
        }
        if two {
            let far = zn.back[1];
            let s0 = d + near;
            let has_store = |lay: &Lay| lay.rooms.iter().any(|r| r.kind == "storeroom");
            for &r in &rs {
                let Some((a, b)) = self.span_of(lay, r, d, s0) else { continue };
                let behind = match lay.rooms[r as usize].kind {
                    "kitchen" => Some("scullery"),
                    "private dining room" => Some("pantry"),
                    "stables" => Some("tack room"),
                    "snug" if !has_store(lay) => Some("storeroom"),
                    _ => None,
                };
                // (A room the far row would make over 2.5:1 a storeroom behind it.)
                let (x, y) = (b - a + 1, near + far);
                let behind = behind.or_else(|| (2 * x.max(y) > 5 * x.min(y)).then_some("storeroom"));
                // (No longer than 2.5 times its depth: the rest of the row the room before it's.)
                let n = (b - a + 1).min((5 * far) / 2);
                let (fa, fb) = if zn.snug.is_some_and(|s| s.0 == 0) { (b + 1 - n, b + 1) } else { (a, a + n) };
                self.fill(lay, s0, l, a, b + 1, r);
                if let Some(kind) = behind {
                    let v = self.put_box(lay, kind, s0, l, fa, fb);
                    if lay.count(v) > 0 {
                        lay.hints.onto.push((v, r));
                    }
                }
            }
        }
    }

    /// A guest floor: the corridor over the stairs, rooms either side of it of a few sizes (one
    /// deep off it, `run`; a deeper side parted, `partition`), tidied and varied; the first floor's
    /// best chambers, the top floor's innkeeper's quarters and common dormitory, guest rooms the
    /// rest (the biggest kinds to the biggest rooms).
    fn inn_upper(&mut self, mut lay: Lay, zn: &Zones, z: i8, rng: &mut Pcg32) -> Laid {
        let (l, w, c) = (self.f.len as isize, self.f.wid as isize, zn.line as isize);
        let (cor, boxes): ([isize; 4], Vec<([isize; 4], usize)>) = if zn.along_s {
            ([0, l, c, c + 1], vec![([0, l, 0, c], 3), ([0, l, c + 1, w], 2)])
        } else {
            ([c, c + 1, 0, w], vec![([0, c, 0, w], 1), ([c + 1, l, 0, w], 0)])
        };
        self.put_box(&mut lay, "corridor", cor[0] as usize, cor[1] as usize, cor[2] as usize, cor[3] as usize);
        let area: isize = boxes.iter().map(|(b, _)| (b[1] - b[0]).max(0) * (b[3] - b[2]).max(0)).sum::<isize>().max(1);
        let want = (crate::core::round(area as f64 / 11.0) as usize).max(3);
        let sizes = [1.0, 1.6, 0.8, 1.3, 2.0, 0.9];
        let mut free = Vec::new();
        let mut lines = Vec::new();
        for (bx, way) in &boxes {
            let (deep, len) = if *way < 2 { (bx[1] - bx[0], bx[3] - bx[2]) } else { (bx[3] - bx[2], bx[1] - bx[0]) };
            if deep <= 0 || len <= 0 {
                continue;
            }
            if deep <= 6 {
                // (Rooms about 3 squares along the corridor, of a few sizes.)
                let n = (crate::core::round(len as f64 / 3.0) as usize).max(1);
                let k0 = rng.next_u32() as usize % sizes.len();
                let kinds: Vec<(&'static str, f64)> = (0..n).map(|k| ("guest room", sizes[(k0 + k) % sizes.len()])).collect();
                free.extend(self.run(&mut lay, bx[0] as usize, bx[1] as usize, bx[2] as usize, bx[3] as usize, &kinds, "guest room"));
                continue;
            }
            let share = crate::core::round(want as f64 * (deep * len) as f64 / area as f64) as usize;
            free.extend(self.partition(&mut lay, *bx, *way, share.max(1), &mut lines, rng));
        }
        self.leftovers(&mut lay, &mut free);
        self.absorb(&mut lay, &mut free, 4);
        self.near_ways(&mut lay, &mut free);
        self.tidy(&mut lay, &mut free);
        self.vary(&mut lay, &mut free);
        // (By need: the innkeeper's on the top floor, the best chambers on the first, a guest room,
        // the dormitory on the top floor, a second best chamber on a big first floor; by weight the
        // biggest rooms.)
        let (first, top) = (z == 1, z == self.floors - 1);
        let rooms = free.iter().filter(|&&r| lay.count(r) > 0).count();
        let mut kinds: Kinds = Vec::new();
        if top {
            kinds.push(("innkeeper's quarters", 2.0));
        }
        if first {
            kinds.push(("best chamber", 3.0));
        }
        kinds.push(("guest room", 1.0));
        if top {
            kinds.push(("dormitory", 4.0));
        }
        if first && (area >= 90 || rooms >= 7) {
            kinds.push(("best chamber", 2.9));
        }
        Self::assign(&mut lay, &free, kinds, &["guest room"]);
        self.links(&mut lay);
        self.near_links(&mut lay);
        let name = if top && z > 1 { "Top floor" } else { "Guest rooms" };
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name, roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls: Vec::new() }
    }
}

/// The least footprint a coaching inn stands round its yard on (squares: the longer side, the
/// shorter): its ranges 3 deep or more round a yard 5 squares or more each way.
pub const YARD: (usize, usize) = (12, 11);

/// A coaching inn's zones in its frame (s from the street, t across): the front range `front` deep,
/// the back range `back`, a side range `side` wide either side, the yard between them (s front..len
/// - back, t side..wid - side); the carriage arch the 2 columns of the front range beside the side
/// range on the arch's side, low t (high where `flip`). `snug`: the snug's width in the taproom's far
/// corner and its depth (0 wide: none).
#[derive(Clone, Copy, Debug)]
pub(super) struct Yard {
    pub(super) front: usize,
    pub(super) back: usize,
    pub(super) side: usize,
    pub(super) flip: bool,
    pub(super) snug: (usize, usize),
}

impl Yard {
    /// Columns t0..t1 as laid out with the arch on the low side, in the frame.
    pub(super) fn cols(&self, w: usize, t0: usize, t1: usize) -> (usize, usize) {
        if self.flip { (w - t1, w - t0) } else { (t0, t1) }
    }

    /// Column t as laid out with the arch on the low side, in the frame.
    fn col(&self, w: usize, t: usize) -> isize {
        (if self.flip { w - 1 - t } else { t }) as isize
    }

    /// The stairs' first column in the frame (the second the next along t, both on the taproom's back
    /// row): against its back wall by the far side range (or the snug where it runs the front's depth).
    pub(super) fn stairs(&self, w: usize) -> usize {
        let end = if self.snug.0 > 0 && self.snug.1 >= self.front { w - self.snug.0 } else { w - self.side };
        let t = end.min(w - self.side) - 2;
        self.cols(w, t, t + 2).0
    }
}

/// A coaching inn's zones for a frame `l` deep from the street and `w` across: the front range 4
/// deep (5 from 16), the back range 3 (4 from 18), the side ranges 3 wide (4 from 17); the taproom
/// 2.5 times its depth across at most, the snug its far corner (the front's depth where the taproom
/// would be wider, else a row less: 3 across from a taproom 8 across, 2 from 6). None under `YARD`.
fn yard_zones(l: usize, w: usize, flip: bool) -> Option<Yard> {
    if !(YARD.1..=26).contains(&l) || !(YARD.1..=22).contains(&w) {
        return None;
    }
    let front = if l >= 16 { 5 } else { 4 };
    let back = if l >= 18 { 4 } else { 3 };
    let side = if w >= 17 { 4 } else { 3 };
    if l < front + back + 5 || w < 2 * side + 5 {
        return None;
    }
    let tw = w - side - 2;
    let wide = (5 * front) / 2;
    let snug = if tw > wide {
        ((tw - wide).max(3), front)
    } else if tw >= 8 {
        (3, front - 1)
    } else if tw >= 6 {
        (2, front - 1)
    } else {
        (0, 0)
    };
    Some(Yard { front, back, side, flip, snug })
}

/// The yard's zones on frame `f` where its footprint is the whole box (every square whole) and the
/// arch's two squares and one of the taproom's on the front open onto open ground (`ground`).
fn yard_fit(sh: &Shell, f: &Frame, flip: bool, ground: &dyn Fn(isize, isize) -> bool) -> Option<Yard> {
    let y = yard_zones(f.len, f.wid, flip)?;
    let whole = |s: usize, t: usize| {
        let (i, j) = f.grid(s as isize, t as isize);
        i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny && sh.whole[j as usize * sh.nx + i as usize]
    };
    if !(0..f.len).all(|s| (0..f.wid).all(|t| whole(s, t))) {
        return None;
    }
    let out = |t: isize| {
        let (i, j) = f.grid(-1, t);
        ground(i, j)
    };
    let w = f.wid;
    let arch = (y.side..y.side + 2).all(|t| out(y.col(w, t)));
    let tap_end = if y.snug.0 > 0 { w - y.snug.0 } else { w };
    let door = (y.side + 2..tap_end).any(|t| out(y.col(w, t)));
    (arch && door).then_some(y)
}

impl Plan {
    /// A coaching inn's yard open to the sky (s0..s1, t0..t1 in its frame: the yard but the squares
    /// the gallery stands over) and the arch's columns (t0..t1); None for any other building.
    pub fn yard(&self) -> Option<((usize, usize, usize, usize), (usize, usize))> {
        let y = self.inn.as_ref()?.yard?;
        let (l, w) = (self.f.len, self.f.wid);
        Some(((y.front + 1, l - y.back - 1, y.side + 1, w - y.side - 1), y.cols(w, y.side, y.side + 2)))
    }

    /// The yard's dressing, as the battlemap draws it in the yard open to the sky (`yard`): a water
    /// trough by the stables, barrels by the kitchen, a cart in the far corner where the open yard is
    /// 4 squares or more each way. Its kind and its box in the frame (s0..s1, t0..t1).
    pub fn yard_pieces(&self) -> Vec<(&'static str, (usize, usize, usize, usize))> {
        let (Some(((s0, s1, _, _), _)), Some(y)) = (self.yard(), self.inn.as_ref().and_then(|z| z.yard)) else { return Vec::new() };
        let w = self.f.wid;
        // (Columns as laid out with the stables on the low side.)
        let (a0, a1) = (y.side + 1, w - y.side - 1);
        let (p, q) = y.cols(w, a0, a0 + 1);
        let mut out = vec![("water_trough", (s0, s0 + 2, p, q))];
        let (p, q) = y.cols(w, a1 - 1, a1);
        out.push(("barrels", (s0, s0 + 1, p, q)));
        if s1 - s0 >= 4 && a1 - a0 >= 4 {
            let (p, q) = y.cols(w, a1 - 2, a1);
            out.push(("cart", (s1 - 2, s1, p, q)));
        }
        out
    }

    /// Each of `rooms` that meets room `to` given a door onto it at the middle of the wall they share
    /// (and opening onto it, not another, where `onto`: `to` is circulation).
    fn open_onto(&self, lay: &mut Lay, rooms: &[i16], to: i16, onto: bool) {
        let rooms: Vec<i16> = rooms.iter().copied().filter(|&r| lay.count(r) > 0).collect();
        for r in rooms {
            let mut pairs: Vec<((isize, isize), (isize, isize))> = Vec::new();
            for s in 0..self.f.len as isize {
                for t in 0..self.f.wid as isize {
                    if !self.at(s, t).is_some_and(|k| lay.cells[k] == r) {
                        continue;
                    }
                    for (ds, dt) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if self.at(s + ds, t + dt).is_some_and(|k| lay.cells[k] == to) {
                            pairs.push(((s, t), (s + ds, t + dt)));
                        }
                    }
                }
            }
            if let Some(&(a, b)) = pairs.get(pairs.len() / 2) {
                self.join(lay, a, b);
                if onto {
                    lay.hints.onto.push((r, to));
                }
            }
        }
    }

    /// Whether square `k` is in the frame's box (a coaching inn's footprint: every square of it).
    fn in_box(&self, k: usize) -> bool {
        let (i, j) = ((k % self.nx) as isize, (k / self.nx) as isize);
        let (a, c) = (self.f.grid(0, 0), self.f.grid(self.f.len as isize - 1, self.f.wid as isize - 1));
        i >= a.0.min(c.0) && i <= a.0.max(c.0) && j >= a.1.min(c.1) && j <= a.1.max(c.1)
    }

    /// A coaching inn's ground floor round its yard (`Yard`).
    fn yard_ground(&mut self, mut lay: Lay, y: Yard) -> Laid {
        let (l, w) = (self.f.len, self.f.wid);
        for k in 0..self.piece.len() {
            if self.in_box(k) {
                self.piece[k] = Piece::Body;
            }
        }
        let (fr, bk, sd) = (y.front, y.back, y.side);
        let (sw, sdp) = y.snug;
        let bx = |p: &Plan, lay: &mut Lay, kind: &'static str, s0: usize, s1: usize, t0: usize, t1: usize| {
            let (a, b) = y.cols(w, t0, t1);
            p.put_box(lay, kind, s0, s1, a, b)
        };
        let at = |s: usize, t: usize| (s as isize, y.col(w, t));
        // The front range: the taproom, the snug in its far corner, the private dining room in the
        // corner on the arch's side, the arch (the yard's squares).
        let tap = bx(self, &mut lay, "taproom", 0, fr, sd + 2, w);
        if sw > 0 {
            bx(self, &mut lay, "snug", 0, sdp, w - sw, w);
        }
        bx(self, &mut lay, "private dining room", 0, fr, 0, sd);
        let yard = bx(self, &mut lay, "inn yard", fr, l - bk, sd, w - sd);
        let (a, b) = y.cols(w, sd, sd + 2);
        self.fill(&mut lay, 0, fr, a, b, yard);
        // The side range by the arch: the stables along the yard (the tack room in the corner behind).
        // (A long range cut into more rooms of its kind: none over 2.5 times its width long.)
        let yl = l - fr - bk;
        let (a, b) = y.cols(w, 0, sd);
        let mut sides = self.run(&mut lay, fr, l - bk, a, b, &[("stables", 1.0)], "stables");
        // (The back corners rooms of their own: the tack room, a storeroom.)
        bx(self, &mut lay, "tack room", l - bk, l, 0, sd);
        bx(self, &mut lay, "storeroom", l - bk, l, w - sd, w);
        // The other: the kitchen behind the taproom, the scullery off it, a pantry (a storeroom in the
        // corner behind).
        let k = (yl - 2).min(4);
        let kitchen = bx(self, &mut lay, "kitchen", fr, fr + k, w - sd, w);
        let scullery = bx(self, &mut lay, "scullery", fr + k, fr + k + 2, w - sd, w);
        lay.hints.onto.push((scullery, kitchen));
        let (a, b) = y.cols(w, w - sd, w);
        if l - bk >= fr + k + 4 {
            sides.extend(self.run(&mut lay, fr + k + 2, l - bk, a, b, &[("pantry", 1.0)], "pantry"));
        } else {
            self.fill(&mut lay, fr + k + 2, l - bk, a, b, scullery);
        }
        // The back range across the yard's end: the stores (a caravanserai's more stables; a
        // one-storey one's guest rooms).
        let cara = self.kind == Kind::Caravanserai;
        let mut kinds: Vec<(&'static str, f64)> = Vec::new();
        if cara {
            kinds.push(("stables", 2.0));
        }
        kinds.push(("storeroom", 1.0));
        if self.floors <= 1 {
            kinds.extend([("guest room", 1.0), ("guest room", 1.2), ("guest room", 0.9)]);
        } else if !cara {
            kinds.push(("buttery", 0.8));
        }
        let fill = if self.floors <= 1 { "guest room" } else { "storeroom" };
        let (b0, b1) = y.cols(w, sd, w - sd);
        let backs = self.run(&mut lay, l - bk, l, b0, b1, &kinds, fill);
        // Doors: the taproom and the kitchen onto the yard; the kitchen into the taproom where they
        // meet; the private dining room off the arch; the side and back ranges' rooms onto the yard
        // where they meet it (the stables' stalls so open onto it).
        self.join(&mut lay, at(fr - 1, sd + 2), at(fr, sd + 2));
        self.join(&mut lay, at(fr + 1, w - sd), at(fr + 1, w - sd - 1));
        let (ks, kt) = at(fr - 1, w - sd + 1);
        if self.at(ks, kt).is_some_and(|q| lay.cells[q] == tap) {
            self.join(&mut lay, (ks, kt), at(fr, w - sd + 1));
        }
        self.join(&mut lay, at(1, sd - 1), at(1, sd));
        let onto: Vec<i16> = sides.iter().chain(&backs).copied().filter(|&r| r != scullery).collect();
        self.open_onto(&mut lay, &onto, yard, false);
        // The front door into the taproom at its front's middle; the arch open onto the street.
        let tap_end = if sw > 0 { w - sw } else { w };
        let mid = (sd + 2 + tap_end) / 2;
        let mut ts: Vec<usize> = (sd + 2..tap_end).collect();
        ts.sort_by_key(|&t| (t.abs_diff(mid), t));
        let first: Vec<isize> = ts.iter().map(|&t| y.col(w, t)).collect();
        self.front_door(&mut lay, tap, &first, 0..0);
        let mut e = Vec::new();
        for t in sd..sd + 2 {
            let (p, q) = (self.f.grid(0, y.col(w, t)), self.f.grid(-1, y.col(w, t)));
            e.push((side(p, q), p.0 as usize, p.1 as usize));
        }
        lay.hints.arch.push((yard, e));
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: Some("taproom"), name: "Taproom", roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls: Vec::new() }
    }

    /// A coaching inn's floor above the ground (`Yard`): nothing over the yard but the gallery round
    /// its edge (railed onto it), every room off the gallery: the front range's best chambers (on the
    /// first floor) or guest rooms, the side ranges' and the back range's guest rooms, on the top
    /// floor the dormitory (the biggest but the front's) and the innkeeper's quarters (the back
    /// range's biggest left).
    fn yard_upper(&mut self, mut lay: Lay, y: Yard, z: i8) -> Laid {
        let (l, w) = (self.f.len, self.f.wid);
        let (fr, bk, sd) = (y.front, y.back, y.side);
        for k in 0..self.piece.len() {
            if self.in_box(k) {
                self.piece[k] = Piece::Body;
            }
        }
        for s in fr + 1..l - bk - 1 {
            for t in sd + 1..w - sd - 1 {
                if let Some(k) = self.at(s as isize, t as isize) {
                    self.piece[k] = Piece::Out;
                }
            }
        }
        let (first, top) = (z == 1, z == self.floors - 1);
        let guest = |n: usize| -> Vec<(&'static str, f64)> { [1.0, 1.3, 0.9, 1.1].iter().cycle().take(n.max(1)).map(|&x| ("guest room", x)).collect() };
        // The front range over the yard's width: one or two rooms (the best chambers on the first
        // floor).
        let yw = w - 2 * sd;
        let nf = if yw >= 8 { 2 } else { 1 };
        let front = self.run(&mut lay, 0, fr, sd, w - sd, &vec![(if first { "best chamber" } else { "guest room" }, 1.0); nf], "guest room");
        // The side ranges the length of the building, their end rooms round the corners.
        let mut others: Vec<i16> = Vec::new();
        for (t0, t1) in [(0, sd), (w - sd, w)] {
            others.push(self.put_box(&mut lay, "guest room", 0, fr + 1, t0, t1));
            let n = crate::core::round((l - bk - 1 - (fr + 1)) as f64 / 3.0) as usize;
            others.extend(self.run(&mut lay, fr + 1, l - bk - 1, t0, t1, &guest(n), "guest room"));
            others.push(self.put_box(&mut lay, "guest room", l - bk - 1, l, t0, t1));
        }
        let n = crate::core::round(yw as f64 / 3.0) as usize;
        let backs = self.run(&mut lay, l - bk, l, sd, w - sd, &guest(n), "guest room");
        // The gallery over the yard's edge, and the landing at the stairs off it.
        let gal = self.put_box(&mut lay, "yard gallery", fr, l - bk, sd, w - sd);
        let st = y.stairs(w);
        self.fill(&mut lay, fr - 1, fr, st, st + 2, gal);
        let size = |lay: &Lay, r: i16| lay.count(r);
        if top {
            let mut pool: Vec<i16> = others.iter().chain(&backs).copied().filter(|&r| size(&lay, r) > 0).collect();
            pool.sort_by_key(|&r| (std::cmp::Reverse(size(&lay, r)), r));
            if let Some(&d) = pool.first() {
                lay.rooms[d as usize].kind = "dormitory";
                let keeper = backs.iter().copied().filter(|&r| r != d && size(&lay, r) > 0).max_by_key(|&r| (size(&lay, r), std::cmp::Reverse(r))).or_else(|| pool.get(1).copied());
                if let Some(q) = keeper {
                    lay.rooms[q as usize].kind = "innkeeper's quarters";
                }
            }
        }
        // Every room off the gallery.
        let rooms: Vec<i16> = front.iter().chain(&others).chain(&backs).copied().collect();
        self.open_onto(&mut lay, &rooms, gal, true);
        let name = if top && z > 1 { "Top floor" } else { "Guest rooms" };
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name, roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls: Vec::new() }
    }
}
