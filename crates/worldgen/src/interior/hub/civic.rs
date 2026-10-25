//! Town halls and the merchants', craft, adventurers' and mages' guildhalls by zones, on the box of
//! their footprint's squares (those outside the footprint left out), from the street front (s, the
//! frame's depth from it) and across it (t):
//! - a hall on the ground floor (a town's town hall, a city's too small for an arcade, every
//!   guild's): the great hall from its low end by the front to the low dais and chair of office at
//!   its far end (`hall::Style::Civic`), the body's full width up to 9 squares; a wider one has a
//!   side range beyond it (the stair hall at the front, the front door into the hall on its axis,
//!   then the guild's rooms off the hall), a narrower one an entrance hall with the stairs across
//!   the front beside a room of the guild's. Behind the dais the back range: the council chamber
//!   (its back corner the strongroom, reached through it) and the archive; a deep building a second
//!   row behind it (the strongroom behind the council chamber, the archive, the guild's rooms).
//!   Above, a landing over the stairs and a corridor on from it, rooms either side of it (one deep
//!   off it, `run`; a deeper side parted, `partition`): the programme's rooms not yet placed first
//!   (`Plan::pending`), then the guild's own (offices, bunk rooms, studies);
//! - an arcade (a city's or a metropolis's town hall big enough): the ground floor an open market
//!   loggia (`arcade`, a way about the floor) from the street to the back range (a guardroom and
//!   offices, stores behind), the stair hall in its front corner, pillars down it, its street side
//!   arches (`Wall::arcade`: the outer wall along the line of its front door); the council hall over
//!   it from the landing at its front corner to its dais at the back, a side range of offices beside
//!   it, the council chamber (the strongroom in its corner) and the archive behind the dais; offices
//!   above as the hall form's.
//!
//! Every frame keeps its back range's and front range's rows and its side range's columns 80% the
//! footprint's (`fit`: a slanted wall falls across the hall or the arcade), its stairs a square in
//! from the outer wall.
//!
//! A city's town hall's belfry (`town::belfry`: a tower piece on a front corner's 3 by 3 squares)
//! rises over its stair hall (the entrance hall of a narrow one): only a frame with the stair hall
//! on the tower's squares will do (`on_belfry`). The stairs climb the body's floors inside it, the
//! landings over the stair hall; from the top floor's landing its spiral stair (at s 1 by the side
//! wall) goes on up `town::belfry::STOREYS` floors of its own, the last its bell loft with the bell.

use super::kit::{Lay, side};
use super::{Frame, Kind, Kinds, Laid, Piece, Plan};
use crate::core::rng::Pcg32;
use crate::interior::{Shell, building_item, hall};
use crate::town::{self, geom::P};

/// The least ground-floor squares a town hall or a guild is laid out by zones with (below it, as
/// other halls are: `super::super::program`), and the least share of its box its squares fill.
pub const LEAST: usize = 60;
pub const FILL: f64 = 0.8;
/// The least box (long, short) a city's town hall stands on an arcade in (else it has its hall on
/// the ground floor).
pub const ARCADE_BOX: (usize, usize) = (11, 9);
/// The widest front a civic building is laid out by zones on (squares): its hall 9 across beside
/// a side range of 7 at most (an arcade's council hall as wide).
pub const WIDEST: usize = 16;

/// A civic building's zones in its frame.
#[derive(Clone, Debug)]
pub(super) struct Zones {
    /// On an arcade, its council hall above it.
    pub(super) arcade: bool,
    /// The hall's width (t from 0); a side range beyond it to the body's width.
    pub(super) hall: usize,
    /// The front range's depth (the entrance hall and a room beside it), 0 where there is none.
    pub(super) front: usize,
    /// The back range's rows' depths, from the hall's far end (an arcade's from its back) on.
    pub(super) back: Vec<usize>,
}

impl Zones {
    /// The hall's (an arcade's) last row + 1: where the back range begins.
    fn back_at(&self, len: usize) -> usize {
        len - self.back.iter().sum::<usize>()
    }

    fn strip(&self, wid: usize) -> bool {
        self.hall < wid
    }
}

/// What kind of civic building `key` is laid out by zones as, if any: a town hall (on an arcade in
/// a city, `city`, of two storeys or more, wherever one fits), a guild;
/// `LEAST` squares or more filling `FILL` of their box, on a frame its zones and stairs fit
/// (`fit`).
pub(super) fn kind(b: &town::Building, key: Option<&str>, sh: &Shell, city: bool) -> Option<Kind> {
    let kind = match key? {
        "town_hall" => Kind::TownHall,
        "merchant_guild" => Kind::MerchantGuild,
        "craft_guild" => Kind::CraftGuild,
        "adventurers_guild" => Kind::AdventurersGuild,
        "mages_guild" => Kind::MagesGuild,
        _ => return None,
    };
    if (!b.parts.is_empty() && town::belfry::of(b).is_none()) || b.keep.is_some() || sh.nx == 0 {
        return None;
    }
    let (x0, x1, y0, y1, m) = bbox(&body_of(sh), sh.nx)?;
    if m < LEAST || (m as f64) < FILL * ((x1 - x0 + 1) * (y1 - y0 + 1)) as f64 {
        return None;
    }
    // (A belfry's frame with its front door onto open ground too: else another corner is tried.)
    let body = body_of(sh);
    let inside = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny && body[j as usize * sh.nx + i as usize];
    let ground = |i: isize, j: isize| sh.ext_free(i, j);
    let fits = |k: Kind| frames(sh).iter().any(|f| on_belfry(sh, f) && fit(sh, k, f).is_some_and(|(z, _)| sh.towers.is_empty() || door_out(f, &z, &inside, &ground)));
    if kind == Kind::TownHall && city && b.floors >= 2 && fits(Kind::ArcadeHall) {
        return Some(Kind::ArcadeHall);
    }
    fits(kind).then_some(kind)
}

/// Whether frame `f`'s front door (zones `z`) has a square that opens onto open ground (`ground`):
/// an arcade's middle, the entrance hall's way in, the hall's axis.
fn door_out(f: &Frame, z: &Zones, inside: &dyn Fn((isize, isize)) -> bool, ground: &dyn Fn(isize, isize) -> bool) -> bool {
    let w = f.wid as isize;
    let doors: Vec<isize> = if z.arcade {
        vec![(w - 1) / 2, w / 2]
    } else if z.front > 0 {
        (w - 3..w).collect()
    } else {
        vec![(z.hall as isize - 1) / 2, z.hall as isize / 2]
    };
    doors.iter().any(|&t| inside(f.grid(0, t)) && ground(f.grid(-1, t).0, f.grid(-1, t).1))
}

/// The body's squares (a belfry's are its stair hall's).

fn body_of(sh: &Shell) -> Vec<bool> {
    sh.inside.clone()
}

/// Whether frame `f`'s stair hall (s 0..3, t wid - 3..wid) is the belfry's squares, if there is one.
fn on_belfry(sh: &Shell, f: &Frame) -> bool {
    let Some(t) = sh.towers.first() else { return true };
    let w = f.wid as isize;
    let mut hall: Vec<usize> = (0..3).flat_map(|s| (w - 3..w).map(move |t| f.grid(s, t))).filter(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny).map(|(i, j)| j as usize * sh.nx + i as usize).collect();
    let mut tower = t.0.clone();
    hall.sort_unstable();
    tower.sort_unstable();
    hall == tower
}

/// The box of the grid's inside squares (x0, x1, y0, y1 inclusive) and how many there are.
pub(super) fn bbox(inside: &[bool], nx: usize) -> Option<(usize, usize, usize, usize, usize)> {
    let (mut x0, mut x1, mut y0, mut y1, mut m) = (usize::MAX, 0, usize::MAX, 0, 0);
    for (k, _) in inside.iter().enumerate().filter(|(_, i)| **i) {
        (x0, x1, y0, y1) = (x0.min(k % nx), x1.max(k % nx), y0.min(k / nx), y1.max(k / nx));
        m += 1;
    }
    (m > 0).then_some((x0, x1, y0, y1, m))
}

/// The frames on the box of the grid's inside squares: a front on each side, t either way along it.
pub(super) fn frames(sh: &Shell) -> Vec<Frame> {
    let Some((x0, x1, y0, y1, _)) = bbox(&body_of(sh), sh.nx) else { return Vec::new() };
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let (xi, yi) = ((x0 as isize, x1 as isize), (y0 as isize, y1 as isize));
    vec![
        Frame { o: (xi.0, yi.0), ds: (0, 1), dt: (1, 0), len: bh, wid: bw },
        Frame { o: (xi.1, yi.0), ds: (0, 1), dt: (-1, 0), len: bh, wid: bw },
        Frame { o: (xi.0, yi.1), ds: (0, -1), dt: (1, 0), len: bh, wid: bw },
        Frame { o: (xi.1, yi.1), ds: (0, -1), dt: (-1, 0), len: bh, wid: bw },
        Frame { o: (xi.0, yi.0), ds: (1, 0), dt: (0, 1), len: bw, wid: bh },
        Frame { o: (xi.0, yi.1), ds: (1, 0), dt: (0, -1), len: bw, wid: bh },
        Frame { o: (xi.1, yi.0), ds: (-1, 0), dt: (0, 1), len: bw, wid: bh },
        Frame { o: (xi.1, yi.1), ds: (-1, 0), dt: (0, -1), len: bw, wid: bh },
    ]
}

/// The zones on frame `f` and the stairs' first square (s, t; the second at t + 1): across the stair
/// hall a square in from the outer wall, clear of it as drawn (the entrance hall's, the side range's
/// at the front, the arcade's in its front corner), its way on past them by the wall. None where
/// they don't fit.
fn fit(sh: &Shell, kind: Kind, f: &Frame) -> Option<(Zones, (isize, isize))> {
    let z = zones(kind, f.len, f.wid)?;
    let w = f.wid as isize;
    let s = 1;
    let body = body_of(sh);
    let whole = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny && sh.whole[j as usize * sh.nx + i as usize];
    let inside = |s: usize, t: usize| {
        let (i, j) = f.grid(s as isize, t as isize);
        i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny && body[j as usize * sh.nx + i as usize]
    };
    // (Every row of the back range and the front range, every column of the side range, 80% the
    // footprint's: a slanted wall falls across the hall, not across a range of small rooms.)
    let full = |n: usize, of: usize| 5 * n >= 4 * of;
    let a = z.back_at(f.len);
    let rows = (a..f.len).chain(0..z.front).all(|s| full((0..f.wid).filter(|&t| inside(s, t)).count(), f.wid));
    let cols = (z.hall..f.wid).all(|t| full((0..f.len).filter(|&s| inside(s, t)).count(), f.len));
    // (An arcade's front straight along 80% of it and 4 squares or more, its squares within the
    // outer wall but for a slanted wall's: its piers on the line.)
    let straight = !z.arcade || {
        let flush = sh.whole_by(-0.3);
        let on = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < sh.nx && (j as usize) < sh.ny && flush[j as usize * sh.nx + i as usize];
        let front = (0..w - 3).filter(|&t| on(f.grid(0, t))).count();
        front >= 4 && full(front, (w - 3) as usize)
    };
    (rows && cols && straight && whole(f.grid(s, w - 3)) && whole(f.grid(s, w - 2))).then_some((z, (s, w - 3)))
}

/// The zones for a frame `l` deep from the front and `w` across (`WIDEST` at most): an arcade 11
/// deep and 9 across or more, its council hall 3 narrower (the landing and offices beside it); a hall
/// 5 across or more, its side range from 10 across (3 to 7 wide, the hall 9 across, narrower where it
/// is short), its front range (none with a side range) 3 deep; the back range's first row 3 deep (an
/// arcade's 4 from 16), a second (2, 3 from 18) in a hall form 15 deep or more; the hall 5 long or
/// more, its sides at most 2.5 to 1, the rest rows of 2 to 4 behind. None where it doesn't fit.
fn zones(kind: Kind, l: usize, w: usize) -> Option<Zones> {
    let arcade = kind == Kind::ArcadeHall;
    if (arcade && (l < ARCADE_BOX.0 || w < ARCADE_BOX.1)) || w < 5 || w > WIDEST {
        return None;
    }
    let strip = arcade || w >= 10;
    let mut hall = if arcade { w - 3 } else if strip { w - (w - 9).clamp(3, 7) } else { w };
    let front = if arcade || strip { 0 } else { 3 };
    let mut b0 = if arcade && l >= 16 { 4 } else { 3 };
    let rest = l.checked_sub(front + b0)?;
    if rest < 5 {
        return None;
    }
    let mut extra = if !arcade && l >= 15 && rest >= 7 { if l >= 18 && rest >= 8 { 3 } else { 2 } } else { 0 };
    let most = (5 * hall) / 2;
    if rest - extra > most {
        extra = rest - most;
    }
    if extra == 1 {
        b0 += 1;
        extra = 0;
    }
    // (The hall no wider than it is long where a side range up to 7 wide can take the rest, and no
    // wider than 2.5 times its length.)
    let hl = l - front - b0 - extra;
    if strip && !arcade && hall > hl {
        hall = hall.min(hl.max(w - 7));
    }
    if 2 * hall > 5 * hl {
        return None;
    }
    let mut back = vec![b0];
    let n = extra.div_ceil(4);
    for k in 0..n {
        back.push(extra / n + usize::from(k < extra % n));
    }
    Some(Zones { arcade, hall, front, back })
}

/// The programme's rooms every floor laid out takes from until each is placed (`Plan::pending`),
/// with their weights for `Plan::assign` on the floors above.
fn programme(kind: Kind) -> Vec<(&'static str, f64)> {
    let mut out = vec![("council chamber", 1.8), ("archive", 1.0), ("strongroom", 0.6)];
    out.extend(match kind {
        Kind::MerchantGuild => vec![("counting house", 1.5), ("sample room", 1.0)],
        Kind::CraftGuild => vec![("masterpiece gallery", 1.5), ("workshop", 1.2)],
        Kind::AdventurersGuild => vec![("job-board room", 1.2), ("trophy hall", 1.4), ("armory", 1.0), ("bunk room", 1.2)],
        Kind::MagesGuild => vec![("library", 1.4), ("laboratory", 1.2), ("warded vault", 0.6)],
        _ => Vec::new(),
    });
    out
}

/// The guild's rooms on the ground floor where there is room, in order (the first beside the
/// entrance hall, else nearest the front in the side range).
fn ground_kinds(kind: Kind) -> Vec<(&'static str, f64)> {
    match kind {
        Kind::MerchantGuild => vec![("counting house", 1.4), ("sample room", 1.0)],
        Kind::CraftGuild => vec![("masterpiece gallery", 1.4), ("workshop", 1.0)],
        Kind::AdventurersGuild => vec![("job-board room", 1.0), ("trophy hall", 1.4), ("armory", 1.0)],
        // (A mages' library and laboratory as long as the range lets them be: `run`.)
        Kind::MagesGuild => vec![("library", 3.0), ("laboratory", 2.6)],
        _ => vec![("office", 1.0)],
    }
}

/// The rooms the floors above hold besides the programme's still to place, and what fills the rest.
fn upper_kinds(kind: Kind) -> (Kinds, &'static [&'static str]) {
    match kind {
        Kind::MerchantGuild => (vec![("office", 1.0), ("storeroom", 0.8)], &["office", "storeroom"]),
        Kind::CraftGuild => (vec![("meeting room", 1.2), ("storeroom", 0.8)], &["office", "storeroom"]),
        Kind::AdventurersGuild => (vec![("bunk room", 1.1), ("storeroom", 0.7)], &["bunk room"]),
        Kind::MagesGuild => (vec![("study", 1.0), ("library", 0.9)], &["study"]),
        _ => (vec![("meeting room", 1.4), ("office", 1.0), ("storeroom", 0.6)], &["office"]),
    }
}

/// A civic building's plan: on the frame that fits (`fit`) with its front nearest a street, open
/// ground before its front door first; its zones, its stairs. None where no frame fits.
pub(super) fn plan(sh: &Shell, kind: Kind, floors: i8, seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let body = body_of(sh);
    let inside = |(i, j): (isize, isize)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && body[j as usize * nx + i as usize];

    let pt = |f: &Frame, s: f64, t: f64| {
        let (i, j) = f.grid(0, 0);
        let c = [i as f64 + 0.5 - 0.5 * (f.ds.0 + f.dt.0) as f64, j as f64 + 0.5 - 0.5 * (f.ds.1 + f.dt.1) as f64];
        [c[0] + f.ds.0 as f64 * s + f.dt.0 as f64 * t, c[1] + f.ds.1 as f64 * s + f.dt.1 as f64 * t]
    };
    let flip = (seed >> 7) & 1;
    let (_, f, z, (s, t)) = frames(sh)
        .into_iter()
        .enumerate()
        .filter_map(|(k, f)| {
            if !on_belfry(sh, &f) {
                return None;
            }
            let (z, st) = fit(sh, kind, &f)?;
            let w = f.wid as isize;
            // (The front door's squares: an arcade's middle, the entrance hall's way in, the hall's
            // axis.)
            let door = door_out(&f, &z, &inside, ground);
            let d = near(pt(&f, 0.0, 0.5 * w as f64));
            Some(((!door, d >= 80.0, crate::core::round(d) as i64, (k as u64 & 1) ^ flip, k), f, z, st))
        })
        .min_by_key(|x| x.0)?;
    let (a, e) = (f.grid(s, t), f.grid(s, t + 1));
    let stairs = [a.0.min(e.0) as usize, a.1.min(e.1) as usize, a.0.abs_diff(e.0) + 1, a.1.abs_diff(e.1) + 1];
    let piece = body.iter().map(|&i| if i { Piece::Body } else { Piece::Out }).collect();
    let mut p = Plan::bare(kind, nx, ny, f, piece, stairs, floors, seed);
    p.pending = programme(kind).into_iter().map(|k| k.0).collect();
    p.civ = Some(z);
    if let Some(t) = sh.towers.first() {
        p.tower_sq = vec![t.0.clone()];
        p.tower_top = vec![floors - 1 + town::belfry::STOREYS];
        let (i, j) = f.grid(1, f.wid as isize - 1);
        p.spiral = (i >= 0 && j >= 0).then(|| j as usize * nx + i as usize).or(Some(t.2));
    }
    Some(p)
}

impl Plan {
    /// A civic building's zones: on an arcade, the hall's width, the front range's depth, the back
    /// range's rows.
    pub fn civic(&self) -> Option<(bool, usize, usize, Vec<usize>)> {
        self.civ.as_ref().map(|z| (z.arcade, z.hall, z.front, z.back.clone()))
    }

    /// Box s0..s1, t0..t1 cut across its longer side into rooms, one of each kind in turn (`kinds`,
    /// by weight) while each stays 2 squares long or more and its depth at most 2.5 times its length,
    /// more (of `fill`) where a room would be over 2.5 times its depth long. The rooms.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run(&self, lay: &mut Lay, s0: usize, s1: usize, t0: usize, t1: usize, kinds: &[(&'static str, f64)], fill: &'static str) -> Vec<i16> {
        if s1 <= s0 || t1 <= t0 {
            return Vec::new();
        }
        let along_t = t1 - t0 >= s1 - s0;
        let (len, dep) = if along_t { (t1 - t0, s1 - s0) } else { (s1 - s0, t1 - t0) };
        let least = (2 * dep).div_ceil(5).max(2);
        let most = ((5 * dep) / 2).max(least);
        let cap = (len / least).max(1);
        let mut n = kinds.len().max(1).min(cap);
        let lengths = loop {
            let w: Vec<f64> = (0..n).map(|k| kinds.get(k).map_or(1.0, |x| x.1)).collect();
            let total: f64 = w.iter().sum();
            let spare = len.saturating_sub(n * least);
            let mut out: Vec<usize> = w.iter().map(|x| if n * least > len { len / n } else { least + crate::core::floor(spare as f64 * x / total) as usize }).collect();
            let mut k = 0;
            while out.iter().sum::<usize>() < len {
                out[k % n] += 1;
                k += 1;
            }
            if n >= cap || out.iter().all(|&x| x <= most) {
                break out;
            }
            n += 1;
        };
        let mut at = 0;
        let mut out = Vec::new();
        for (k, l) in lengths.into_iter().enumerate() {
            let r = lay.room(kinds.get(k).map_or(fill, |x| x.0));
            if along_t {
                self.fill(lay, s0, s1, t0 + at, t0 + at + l, r);
            } else {
                self.fill(lay, s0 + at, s0 + at + l, t0, t1, r);
            }
            at += l;
            out.push(r);
        }
        out
    }

    /// The strongroom in room `r`'s corner s0..s1, t0..t0 + 2 (a council chamber's back corner on its
    /// outer wall), where `r` keeps 6 squares or more and the corner is all its.
    fn strongroom_in(&self, lay: &mut Lay, r: i16, s0: usize, s1: usize, t0: usize) -> bool {
        let sq: Vec<usize> = (s0..s1).flat_map(|s| (t0..t0 + 2).map(move |t| (s, t))).filter_map(|(s, t)| self.body(s as isize, t as isize)).collect();
        if sq.len() != 2 * (s1 - s0) || sq.iter().any(|&k| lay.cells[k] != r) || lay.count(r) < sq.len() + 6 {
            return false;
        }
        let v = lay.room("strongroom");
        for k in sq {
            lay.cells[k] = v;
        }
        true
    }

    /// The kinds of the programme's rooms still to place, by weight, with those of `kinds` not
    /// among them.
    fn still(&self, kinds: &[(&'static str, f64)], here: &[&'static str]) -> Vec<(&'static str, f64)> {
        kinds.iter().copied().filter(|k| self.pending.contains(&k.0) && !here.contains(&k.0)).collect()
    }

    /// A town hall's belfry: its spiral stair's square and its top floor (the bell loft).
    pub fn belfry(&self) -> Option<(usize, i8)> {
        if self.civ.is_none() {
            return None;
        }
        Some((self.spiral?, *self.tower_top.first()?))
    }

    /// A civic building's floor `z` (`civic_floor`); above the hall's top floor a belfry's alone (its
    /// stair hall's squares), its bell loft at its top with the bell (in a corner away from its stair).
    pub(super) fn civic_level(&mut self, z: i8, lay: Lay, rng: Pcg32) -> Option<Laid> {
        let Some((spiral, top)) = self.belfry().filter(|_| z >= self.floors) else { return self.civic_floor(z, lay, rng) };

        let mut items = Vec::new();
        if z == top {
            let sq = &self.tower_sq[0];
            let nx = self.nx;
            let (bx, by) = sq.iter().fold((usize::MAX, usize::MAX), |a, &k| (a.0.min(k % nx), a.1.min(k / nx)));
            let d = crate::core::round(crate::core::sqrt(sq.len() as f64)) as usize;
            let bs = if d >= 4 { 2 } else { 1 };
            let (sx, sy) = (spiral % nx, spiral / nx);
            let x = if sx - bx < d / 2 { bx + d - bs } else { bx };
            let y = if sy - by < d / 2 { by + d - bs } else { by };
            items.extend(building_item("bell", x, y, bs, bs));
        }
        let name = if z == top { "Bell loft" } else { "Belfry" };
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name, roof: false, items, towers: vec![(0, if z == top { "belfry" } else { "belfry stair" })], corners: Vec::new(), halls: Vec::new() })
    }

    /// A civic building's floor `z` (`civic` above); its cellar as other buildings' are.
    fn civic_floor(&mut self, z: i8, mut lay: Lay, mut rng: Pcg32) -> Option<Laid> {
        if z < 0 {
            return None;
        }

        let zn = self.civ.clone()?;
        let (l, w) = (self.f.len, self.f.wid);
        let a = zn.back_at(l);
        let frame = |o: (isize, isize), ds: (isize, isize), len: usize, wid: usize| hall::Frame { o, ds, dt: self.f.dt, len, wid };
        let mut halls = Vec::new();
        let (name, public) = if zn.arcade && z == 0 {
            // The arcade from the street to the back range, the stair hall in its front corner; a
            // guardroom and offices behind it, then stores.
            let arc = self.put_box(&mut lay, "arcade", 0, a, 0, w);
            self.put_box(&mut lay, "stair hall", 0, 3, w - 3, w);
            self.run(&mut lay, a, a + zn.back[0], 0, w, &[("guardroom", 1.0), ("office", 1.0)], "office");
            self.rows(&mut lay, &zn, &[("storeroom", 1.0)], w);
            // Pillars down it, 3 squares apart, clear of its walls, the stair hall's ways in and the
            // doors at its ends.
            let ts = spaced(w, 2);
            let ss = spaced(a.saturating_sub(1), 1);
            for &s in &ss {
                for &t in ts.iter().filter(|&&t| s > 3 || t + 5 < w) {
                    if let Some(k) = self.body(s as isize, t as isize).filter(|&k| lay.cells[k] == arc)
                        && let Some(it) = building_item("pillar", k % self.nx, k / self.nx, 1, 1)
                    {
                        lay.items.push(it);
                    }
                }
            }
            let m = (w as isize - 3) / 2;
            self.front_door(&mut lay, arc, &[m, m - 1], 0..w as isize - 3);
            ("Arcade", None)
        } else if z == 0 || (zn.arcade && z == 1) {
            // The hall from its low end by the front to its dais at the far end (an arcade's council
            // hall over it, from the landing at its front corner), the back range behind the dais.
            let (hw, fr) = (zn.hall, zn.front);
            let hall = self.put_box(&mut lay, "great hall", fr, a, 0, hw);
            halls.push(frame(self.f.grid(fr as isize, 0), self.f.ds, a - fr, hw));
            let gk = if zn.arcade { vec![("office", 1.0)] } else { ground_kinds(self.kind) };
            if fr > 0 {
                let eh = self.put_box(&mut lay, "entrance hall", 0, fr, w - 3, w);
                self.run(&mut lay, 0, fr, 0, w - 3, &gk[..1], gk[0].0);
                self.front_door(&mut lay, eh, &[w as isize - 2, w as isize - 3], (w as isize - 3)..w as isize);
            } else {
                self.put_box(&mut lay, if zn.arcade { "landing" } else { "stair hall" }, 0, 3, hw, w);
                self.run(&mut lay, 3, a, hw, w, &gk, "office");
                if !zn.arcade {
                    self.front_door(&mut lay, hall, &[(hw as isize - 1) / 2, hw as isize / 2], 0..hw as isize);
                }
            }
            self.chambers(&mut lay, &zn, hw, hw);
            // (Behind the side range, its rows' depth: what of the programme is left, else an office;
            // a room from the hall through the side range's last.)
            if zn.strip(w) {
                let here: Vec<&'static str> = lay.rooms.iter().map(|r| r.kind).collect();
                let left: Vec<(&'static str, f64)> = self.still(&programme(self.kind), &here).into_iter().filter(|k| k.0 != "strongroom").take(1).collect();
                self.run(&mut lay, a, l, hw, w, &left, "office");
            }
            (if zn.arcade { "Council floor" } else { "Ground floor" }, Some("great hall"))
        } else {
            return Some(self.civic_upper(lay, &zn, &mut rng));
        };
        // (Rooms a slanted wall left under 4 squares or a square wide: the room beside them's; what
        // of the programme they were, on a floor above.)
        self.absorb(&mut lay, &mut Vec::new(), 4);
        // (A room three or more rooms from a way about the floor, a chain of the back rows, is made
        // part of the room beside it nearest one: `near_ways`.)
        let mut deep: Vec<i16> = (0..lay.rooms.len() as i16)
            .filter(|&r| lay.count(r) > 0 && !matches!(lay.rooms[r as usize].kind, "great hall" | "strongroom" | "council chamber" | "stair hall" | "landing" | "entrance hall" | "arcade"))
            .collect();
        self.near_ways(&mut lay, &mut deep);
        let here: Vec<&'static str> = (0..lay.rooms.len()).filter(|&r| lay.count(r as i16) > 0).map(|r| lay.rooms[r].kind).collect();
        self.pending.retain(|k| !here.contains(k));
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public, name, roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls })
    }

    /// The front door into room `r` through the front wall at the first of `first` (t) onto open
    /// ground, else anywhere along `rest`.
    pub(super) fn front_door(&self, lay: &mut Lay, r: i16, first: &[isize], rest: std::ops::Range<isize>) {
        let mut e = Vec::new();
        for t in first.iter().copied().chain(rest) {
            let (p, q) = (self.f.grid(0, t), self.f.grid(-1, t));
            if self.body(0, t).is_some() {
                e.push((side(p, q), p.0 as usize, p.1 as usize));
            }
        }
        lay.hints.front = Some((r, e));
    }

    /// The back range's first row from `a` (over 0..t1, beside the stair hall or the landing): the
    /// council chamber at t 0 (its back corner the strongroom where no row is behind it) and the
    /// archive; the rows behind (over 0..rows), the strongroom behind the council chamber, then what
    /// of the programme is left (`rows`).
    fn chambers(&self, lay: &mut Lay, zn: &Zones, t1: usize, rows: usize) {
        let a = zn.back_at(self.f.len);
        let b0 = zn.back[0];
        let rs = self.run(lay, a, a + b0, 0, t1, &[("council chamber", 2.0), ("archive", 1.0)], "office");
        if zn.back.len() == 1
            && let Some(&c) = rs.first()
        {
            self.strongroom_in(lay, c, a + 1, a + b0, 0);
        }
        let here: Vec<&'static str> = lay.rooms.iter().map(|r| r.kind).collect();
        let left = self.still(&programme(self.kind), &here);
        self.rows(lay, zn, &left, rows);
    }

    /// The back range's rows behind its first, over t 0..w: the first of them the strongroom at t 0
    /// (2 or 3 squares across) and `kinds` beside it, the others `kinds` left over (offices after
    /// them).
    fn rows(&self, lay: &mut Lay, zn: &Zones, kinds: &[(&'static str, f64)], w: usize) {
        let mut s = zn.back_at(self.f.len) + zn.back[0];
        let mut todo: Vec<(&'static str, f64)> = kinds.to_vec();
        for (n, &d) in zn.back.iter().enumerate().skip(1) {
            let mut t0 = 0;
            if n == 1 && todo.iter().any(|k| k.0 == "strongroom") {
                let sw = if w >= 8 { 3 } else { 2 };
                self.put_box(lay, "strongroom", s, s + d, 0, sw);
                todo.retain(|k| k.0 != "strongroom");
                t0 = sw;
            }
            let rs = self.run(lay, s, s + d, t0, w, &todo, "office");
            todo.drain(..rs.len().min(todo.len()));
            s += d;
        }
    }

    /// A civic building's storey above its hall (or its council hall): the landing over the stair
    /// hall, a corridor on from it the length of the building, rooms either side (`partition`):
    /// those of the programme still to place, then the storey's own.
    fn civic_upper(&mut self, mut lay: Lay, zn: &Zones, rng: &mut Pcg32) -> Laid {
        let (l, w) = (self.f.len as isize, self.f.wid as isize);
        // (Landing, corridor (t, s0..s1), and the boxes off them with the side each is on.)
        let (land, c, cs, boxes): ([isize; 4], isize, (isize, isize), Vec<([isize; 4], usize)>) = if zn.front > 0 {
            let fr = zn.front as isize;
            ([0, fr, w - 3, w], w - 3, (fr, l), vec![([0, l, 0, w - 3], 3), ([fr, l, w - 2, w], 2)])
        } else {
            let hw = zn.hall as isize;
            ([0, 3, hw, w], hw, (3, l), vec![([0, l, 0, hw], 3), ([3, l, hw + 1, w], 2)])
        };
        self.put_box(&mut lay, "landing", land[0] as usize, land[1] as usize, land[2] as usize, land[3] as usize);
        self.put_box(&mut lay, "corridor", cs.0 as usize, cs.1 as usize, c as usize, c as usize + 1);
        let prog = programme(self.kind);
        let todo: Vec<(&'static str, f64)> = prog.iter().copied().filter(|k| self.pending.contains(&k.0)).collect();
        let (own, fill) = upper_kinds(self.kind);
        let area: isize = boxes.iter().map(|(b, _)| (b[1] - b[0]) * (b[3] - b[2])).sum::<isize>().max(1);
        // (A mages' guild's studies fewer and bigger: about 28 squares each.)
        let mage = self.kind == Kind::MagesGuild;
        let per = if mage { 28.0 } else { 16.0 };
        let want = (todo.len() + own.len()).max(crate::core::round(area as f64 / per) as usize);
        let mut free = Vec::new();
        let mut lines = Vec::new();
        for (bx, way) in &boxes {
            // (A box 6 deep or less from its way: rooms one deep across it, `run`; deeper, parted. A
            // mages' guild's to 10 deep, its share of the rooms, every study off the corridor.)
            let deep = if *way < 2 { bx[1] - bx[0] } else { bx[3] - bx[2] };
            if deep <= 0 || bx[1] <= bx[0] {
                continue;
            }
            let share = crate::core::round(want as f64 * ((bx[1] - bx[0]) * (bx[3] - bx[2])) as f64 / area as f64) as usize;
            if deep <= 6 || (mage && deep <= 10) {
                let n = if mage { vec![("chamber", 1.0); share.max(1)] } else { Vec::new() };
                free.extend(self.run(&mut lay, bx[0] as usize, bx[1] as usize, bx[2] as usize, bx[3] as usize, &n, "chamber"));
                continue;
            }
            free.extend(self.partition(&mut lay, *bx, *way, share.max(1), &mut lines, rng));
        }
        self.leftovers(&mut lay, &mut free);
        self.absorb(&mut lay, &mut free, 4);
        self.near_ways(&mut lay, &mut free);
        self.tidy(&mut lay, &mut free);
        self.vary(&mut lay, &mut free);
        // (The programme's rooms first, outweighing the storey's own.)
        let mut kinds: Kinds = todo.iter().map(|&(k, wt)| (k, wt + 2.0)).collect();
        kinds.extend(own);
        Self::assign(&mut lay, &free, kinds, fill);
        self.links(&mut lay);
        self.near_links(&mut lay);
        let here: Vec<&'static str> = (0..lay.rooms.len()).filter(|&r| lay.count(r as i16) > 0).map(|r| lay.rooms[r].kind).collect();
        self.pending.retain(|k| !here.contains(k));
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public: None, name: "Upper floor", roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls: Vec::new() }
    }
}

/// Places 3 squares apart along a run of `n` squares, `m` or more from each end, centred.
fn spaced(n: usize, m: usize) -> Vec<usize> {
    if n < 2 * m + 1 {
        return Vec::new();
    }
    let span = n - 1 - 2 * m;
    let k = span / 3 + 1;
    let start = m + (span - 3 * (k - 1)) / 2;
    (0..k).map(|i| start + 3 * i).collect()
}
