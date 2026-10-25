//! The hall house's programme: a castle's keep (of pieces, or redrawn by hand as a tower keep) and a
//! big barracks (`barracks`): the body's frame and zones (`plan`), the hall's floor with its service
//! end, screens passage and dais end, the stores under a first-floor hall, the floors above it (the
//! lord's rooms by the main stair, a hall open two storeys), wings, a forebuilding, the tops and the
//! towers' rooms.

use super::{barracks, estate};
use super::kit::{Lay, cuts, depth, side};
use super::{Frame, Kind, Kinds, Laid, Piece, Plan, Top, Wing};
use crate::core::rng::{Pcg32, hash3};
use crate::interior::{OPEN_BELOW, Shell, is_circulation};
use crate::town::{self, PartKind, geom::{self, P}};

/// A hall house's plan (`super::plan`): a keep of pieces or a castle's keep redrawn by hand (as a
/// tower keep), a barracks; its frame along the body, its zones, its tops, its wings and pieces.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan(b: &town::Building, sh: &Shell, kind: Kind, piece: Vec<Piece>, wing_of: Vec<usize>, tops: &[u8], seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool, grid_p: &dyn Fn(P) -> P) -> Option<Plan> {
    // (A manor turned from the street (`plan_as`) where its screens passage then opens onto the
    // ground outside, else not.)
    if kind == Kind::Manor
        && let Some(p) = plan_as(b, sh, kind, piece.clone(), wing_of.clone(), tops, seed, near, ground, grid_p, true)
    {
        return Some(p);
    }
    plan_as(b, sh, kind, piece, wing_of, tops, seed, near, ground, grid_p, false)
}

/// `plan`; `turn`: a manor's service end at the end away from the street where the street's end of
/// its lot is cut short and the other is whole (None where it is not, or where its screens passage
/// would then open onto no open ground).
#[allow(clippy::too_many_arguments)]
fn plan_as(b: &town::Building, sh: &Shell, kind: Kind, piece: Vec<Piece>, wing_of: Vec<usize>, tops: &[u8], seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool, grid_p: &dyn Fn(P) -> P, turn: bool) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let mut rng = Pcg32::new(hash3(seed, 0x6b, 0x11, 0), 81);
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
    // `small`.)
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
    let mut bay = false;
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
        // (A manor's service end by the street, unless that end of its lot is cut short (a slanted
        // lot's, by a sixth or more: no room for a kitchen, a buttery and a pantry) and the other is
        // whole: then at the other.)
        Kind::Manor => {
            let (lo_a, hi_a) = if long_x { (x0, x1) } else { (y0, y1) };
            let end = |low: bool| {
                body.iter()
                    .filter(|&&k| {
                        let a = if long_x { k % nx } else { k / nx };
                        if low { a < lo_a + 3 } else { a + 3 > hi_a }
                    })
                    .count()
            };
            let (near, far) = if street_low { (end(true), end(false)) } else { (end(false), end(true)) };
            if turn && !(6 * near < 5 * far && far + 1 >= 3 * wid) {
                return None;
            }
            (street_low != turn, street_front)
        }
        // (A barracks' mess hall on its front, else entered from an end: `barracks::ends`.)
        Kind::Barracks => {
            let (low, front, by_end) = barracks::ends(sh, [x0, x1, y0, y1], long_x, ground, street_low, street_front);
            bay = by_end;
            (low, front)
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
        return super::small::plan(b, sh, piece, f, seed, street_front == front_min);
    }
    let body_at = |s: isize, t: isize| at(s, t).filter(|&k| piece[k] == Piece::Body);
    // The landing in the middle of the dais end; the main stair in it, along the axis, a square in
    // from the end wall (clear of the outer wall as drawn), its free side toward the hall's door.
    // (A narrow estate's landing against its far wall, its parlour all the rest of the end.)
    let estate = matches!(kind, Kind::OldPalace | Kind::HallHouse | Kind::Manor);
    let (lw, tl0) = if estate && wid < 7 { (2, wid - 2) } else { (2 + wid % 2, (wid - 2 - wid % 2) / 2) };
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
    // (A manor's cross wing deeper: `estate::dais_back`.)
    let back = estate::dais_back(kind, len);
    // (A manor not turned whose dais end its lot cuts slant: its cross wing as far in as the landing
    // finds whole squares, up to 3 rows more.)
    let mut stair = find(back).or_else(|| find(back + 1)).or_else(|| (kind == Kind::Manor && !turn).then(|| (back + 2..=back + 4).find_map(find)).flatten());
    // (A body too short for a hall of 3 rows behind its screens and a service end of 2: the stair
    // against the end wall.)
    if kind != Kind::Barracks && stair.is_some_and(|(s0, _)| s0 - 3 < 3) {
        stair = find(0).or(stair);
    }
    let (ss, st) = stair?;
    let dz = ss as usize;
    // The hall: 4 squares long at least, after the service end and the screens passage.
    // (An estate's 3 rows at least, its service end kept; 3 rows deep where it is 10 squares across or
    // more, so that its rooms are not long and thin.)
    let hall_min = if estate { 3 } else { 4 };
    let deep_service = len >= 14 || (estate && wid >= 10 && len >= 11);
    let (mut service, mut screens) = if kind == Kind::Barracks { (if len >= 14 { 3 } else { 2 }, false) } else { (if deep_service { 3 } else { 2 }, true) };
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
    // (A manor turned from the street: its screens passage open to the ground outside at an end.)
    if turn && screens && ![-1, wid as isize].iter().any(|&t| {
        let (i, j) = f.grid(service as isize, t);
        ground(i, j)
    }) {
        return None;
    }
    let hall_t = if wid <= 9 { (0, wid) } else { (0, (wid - 3).min(9).max(tl0 + lw)) };
    // (A manor's service wing deep enough for a kitchen of 12 squares and a buttery and a pantry of
    // 6, and for a near-square kitchen (3 to 5 rows), where the hall keeps its least rows; its hall
    // 30% of its ground floor at most, the rows past that to its cross wing.)
    let (mut ss, mut st, mut dz) = (ss, st, dz);
    if kind == Kind::Manor && screens {
        // (The hall no longer across than 2.5 times its length.)
        let ht = hall_t.1 - hall_t.0;
        let least = hall_min.max((2 * ht).div_ceil(5) as isize);
        let deep = (27usize.div_ceil(wid)).max(crate::core::round(0.4 * wid as f64) as usize).clamp(3, 5);
        while service < 6 && (service < deep || held(service) < 27) && dz as isize - service as isize - 2 >= least {
            service += 1;
        }
        let most = ((0.3 * held(len) as f64) / ht as f64) as isize;
        let over = dz as isize - service as isize - 1 - most.max(least);
        if let Some((s0, t0)) = (1..=over).rev().find_map(|x| find(back + x)) {
            (ss, st, dz) = (s0, t0, s0 as usize);
        }
    }
    let (a, bq) = (f.grid(ss, st), f.grid(ss + 1, st));
    let stairs = [a.0.min(bq.0) as usize, a.1.min(bq.1) as usize, (a.0 - bq.0).unsigned_abs() + 1, (a.1 - bq.1).unsigned_abs() + 1];
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
    let pending = if kind == Kind::Barracks {
        vec![]
    } else if estate {
        estate::pending(kind)
    } else {
        vec!["great chamber", "solar", "treasury", "armory", "garrison"]
    };
    let bay = if bay { barracks::bay(sh, &piece, &f, ground) } else { None };
    // (The chapel over the forebuilding's entrance, where it has two storeys.)
    // (An estate's in its dais end, if any: `estate::dais_rooms`; an older palace's there, else on the
    // first floor above with room for it.)
    let chapel = kind == Kind::Barracks || (estate && kind != Kind::OldPalace) || (!fore.is_empty() && fore_floors >= 2);
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
        pal: None,
        tem: None,
        mon: None,
        civ: None,
        inn: None,
        spiral: None,
        rich: kind == Kind::Manor && sh.inside.iter().filter(|&&i| i).count() >= estate::RICH,
    })
}

impl Plan {
    /// A hall house's floor `z` (`level`): laid out by its zones, again with more rooms where it
    /// repeats the storey below.
    pub(super) fn hall_level(&mut self, z: i8, blank: Lay, mut rng: Pcg32) -> Option<Laid> {
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
                name = if barracks { "Mess" } else if self.estate() { estate::names(self.kind)[0] } else { "Hall" };
                kinds = if barracks {
                    barracks::kinds(barracks::HALL_KINDS)
                } else if self.estate() {
                    estate::hall_kinds(self.kind).0
                } else {
                    vec![("chamber", 1.4), ("guest chamber", 1.1), ("guardroom", 1.0), ("wardrobe", 0.6)]
                };
                // (No service end: its rooms the floor's first; one too small for more than its
                // kitchen: a pantry the first.)
                if self.service == 0 && !barracks {
                    kinds.splice(0..0, [("kitchen", 2.0), ("buttery", 1.8)]);
                } else if !barracks && !lay.rooms.iter().any(|r| r.kind == "buttery" || r.kind == "pantry") {
                    kinds.insert(0, ("pantry", 1.9));
                }
                fill = if barracks {
                    barracks::HALL_FILL
                } else if self.estate() {
                    estate::hall_kinds(self.kind).1
                } else {
                    &["chamber", "storeroom", "guest chamber"]
                };
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
                    // (An older palace's long gallery down the top storey's long wall.)
                    let gallery = self.kind == Kind::OldPalace && n >= 2 && z + 1 == self.floors;
                    free = self.upper_floor(&mut lay, &mut rng, lord, need + more, more, gallery);
                    name = match (self.estate(), lord) {
                        (true, l) => estate::names(self.kind)[if l { 1 } else { 2 }],
                        (false, true) => "Lord's floor",
                        (false, false) => "Upper floor",
                    };
                }
                upper = true;
                if barracks {
                    kinds = barracks::kinds(barracks::UPPER_KINDS);
                    fill = barracks::UPPER_FILL;
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
        if self.estate() && z == self.hall_z {
            self.hall_to_zone(&mut lay);
        }
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
        // (An estate's rooms the lord's rooms' places left more than a room from a way about the floor:
        // made part of the room beside them nearest one, their doors found again.)
        if self.estate() && z > self.hall_z {
            let mut rest: Vec<i16> = (0..lay.rooms.len() as i16).filter(|&r| lay.count(r) > 0 && !matches!(lay.rooms[r as usize].kind, "great chamber" | "solar" | "chapel")).collect();
            self.near_ways(&mut lay, &mut rest);
            self.near_links(&mut lay);
        }
        self.last = lay.cells.clone();
        let halls: Vec<crate::interior::hall::Frame> = (z == self.hall_z).then(|| self.hall_frame()).into_iter().collect();
        if self.estate() {
            self.estate_names(&mut lay.rooms, z == 0);
        }
        Some(Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public, name, roof, items: lay.items, towers, corners, halls })
    }

    /// The great chamber of good size by the main stair (9 to 60 squares, a square within 2 of it,
    /// of sound proportions): one too small made one with a room beside it where the two make one
    /// such, else changing places with the room of the storey nearest the stair that is one, of a
    /// kind any room may be.
    pub(super) fn great_chamber(&mut self, lay: &mut Lay) {
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
    pub(super) fn lords_in_towers(&mut self, lay: &mut Lay, towers: &mut [(usize, &'static str)]) {
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

    /// The hall's low end (grid): the middle of the screens passage, else of the service end; None
    /// without either.
    pub(super) fn low_end(&self) -> Option<P> {
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
    pub(super) fn solar(&mut self, lay: &mut Lay) {
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

    /// An estate's hall kept to its zone: what went to it past the dais end's line (a slanted lot's
    /// tail of a room there) goes to the room beside it there, the landing first: the hall's far
    /// wall whole behind its dais.
    pub(super) fn hall_to_zone(&self, lay: &mut Lay) {
        let Some(hall) = (0..lay.rooms.len() as i16).find(|&r| lay.rooms[r as usize].kind == "great hall") else { return };
        loop {
            let mut moved = false;
            for s in self.dz as isize..self.f.len as isize {
                for t in 0..self.f.wid as isize {
                    let Some(k) = self.body(s, t).filter(|&k| lay.cells[k] == hall) else { continue };
                    let by = [(1, 0), (0, -1), (0, 1)].iter().filter_map(|&(ds, dt)| self.body(s + ds, t + dt).map(|q| lay.cells[q])).filter(|&r| r >= 0 && r != hall).min_by_key(|&r| (lay.rooms[r as usize].kind != "landing", r));
                    if let Some(r) = by {
                        lay.cells[k] = r;
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// The hall's frame (`crate::interior::hall::Frame`): from the screens' side (its low end) to the dais end,
    /// across its width.
    pub(super) fn hall_frame(&self) -> crate::interior::hall::Frame {
        let h0 = self.service + self.screens as usize;
        let (ht0, ht1) = self.hall_t;
        crate::interior::hall::Frame { o: self.f.grid(h0 as isize, ht0 as isize), ds: self.f.ds, dt: self.f.dt, len: self.dz - h0, wid: ht1 - ht0 }
    }

    /// The rooms an upper floor (`n` above the hall's) takes, besides the lord's (laid where they
    /// go: `upper_floor`): first what must be somewhere and is due here (the chapel on the first
    /// floor with room for it; on the top storey all that is left, else a treasury from the second
    /// floor up, an armory and a garrison from the third), then chambers and guest chambers, a
    /// study, stores... With how many come first.
    pub(super) fn upper_kinds(&self, n: usize, last: bool) -> (Kinds, usize) {
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
        let own: Kinds = if self.estate() {
            estate::upper_kinds(self.kind, n, last)
        } else {
            match n {
                1 => vec![("chamber", 1.4), ("wardrobe", 0.7), ("guest chamber", 1.1), ("study", 0.9)],
                2 => vec![("chamber", 1.8), ("guest chamber", 1.5), ("study", 1.0), ("guest chamber", 1.1), ("wardrobe", 0.6)],
                3 => vec![("storeroom", 1.0), ("chamber", 0.9), ("map room", 1.2), ("guest chamber", 1.1)],
                _ => vec![("guest chamber", 1.4), ("servants' quarters", 1.6), ("storeroom", 1.0), ("library", 1.2), ("chamber", 0.8)],
            }
        };
        k.extend(own);
        (k, musts)
    }

    /// The dais end: the landing in its middle (the main stair in it), a room each side (`akind` on
    /// the front side, `bkind` on the far one) where it is 2 squares across. Returns the rooms each
    /// side and the landing.
    pub(super) fn dais(&self, lay: &mut Lay, akind: &'static str, bkind: &'static str) -> (Option<i16>, Option<i16>, i16) {
        let (l, w) = (self.f.len, self.f.wid);
        let (tl0, tl1) = self.land_t;
        let landing = lay.room("landing");
        self.fill(lay, self.dz, l, tl0, tl1, landing);
        // (A manor's cross wing deeper than 2.5 times a side's width: that side two rooms, the second
        // beyond the first; its chapel whole.)
        let manor = self.kind == Kind::Manor;
        let mut side = |kind: &'static str, t0: usize, t1: usize| {
            (t1 >= t0 + 2).then(|| {
                let r = lay.room(kind);
                let deep = l - self.dz;
                if manor && kind != "chapel" && 2 * deep > 5 * (t1 - t0) {
                    let mid = self.dz + deep / 2;
                    self.fill(lay, self.dz, mid, t0, t1, r);
                    let more = lay.room(match kind {
                        "parlor" => "study",
                        "study" => "chamber",
                        _ => "storeroom",
                    });
                    self.fill(lay, mid, l, t0, t1, more);
                } else {
                    self.fill(lay, self.dz, l, t0, t1, r);
                }
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
    pub(super) fn service_cols(&self, kinds: &mut Vec<(&'static str, f64)>) -> Vec<usize> {
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
    pub(super) fn hall_floor(&self, lay: &mut Lay, ground: bool, rng: &mut Pcg32) -> Vec<i16> {
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
            self.bay_service(lay, hall, m);
        } else if self.service > 0 {
            let mut kinds: Vec<(&'static str, f64)> = if barracks {
                barracks::SERVICE.to_vec()
            } else if self.estate() {
                estate::service(self.kind, w)
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
            self.officers(lay, hall, h0);
        } else {
            // (An estate's main room there on the side with more floor: a slanted lot cuts the other.)
            let (l, (tl0, tl1)) = (self.f.len as isize, self.land_t);
            let (mut a, mut b) = if self.estate() { estate::dais_rooms(self) } else { ("parlor", "wardrobe") };
            if self.estate() && self.squares(self.dz as isize, l, 0, tl0 as isize) < self.squares(self.dz as isize, l, tl1 as isize, w as isize) {
                (a, b) = (b, a);
            }
            self.dais(lay, a, b);
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
    pub(super) fn front(&self, lay: &mut Lay, screens: i16, hall: i16) {
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
    pub(super) fn stores_floor(&self, lay: &mut Lay, rng: &mut Pcg32) -> Vec<i16> {
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
    pub(super) fn passage_side(&self, sides: &[isize], s1: isize, low: isize, turn: &dyn Fn(isize) -> (isize, isize), flip: bool) -> Option<(isize, isize)> {
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
    #[allow(clippy::too_many_arguments)]
    pub(super) fn upper_floor(&mut self, lay: &mut Lay, rng: &mut Pcg32, lord: bool, need: usize, more: usize, gallery: bool) -> Vec<i16> {
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
        // (A long gallery: the passage down the long wall 2 or 3 squares wide.)
        let gw = if !gallery { 1 } else if w >= 14 { 4 } else { 3 };
        if gw > 1 {
            let g = lay.room("long gallery");
            let (c0, c1) = if pt == 0 { (0, gw) } else { (w - gw, w) };
            self.fill(lay, e as usize, row as usize, c0 as usize, c1 as usize, g);
        }
        // (The landing's sides: by the passage's wall, and across from it from the cross passage's
        // row; the rooms' columns beside the passage.)
        let (near, far) = if pt == 0 { ((0, tl0), (tl1, w)) } else { ((tl1, w), (0, tl0)) };
        let (t0, t1) = if pt == 0 { (gw, w) } else { (0, w - gw) };
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

    /// An upper floor whose long walls are both broken by the dais end: the landing at the dais
    /// end, a corridor on from it down the axis (ending at rooms across the low end in a long
    /// body), rooms each side divided by cross walls (`partition`); the lord's rooms the biggest.
    pub(super) fn corridor_floor(&mut self, lay: &mut Lay, rng: &mut Pcg32, need: usize) -> Vec<i16> {
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
    pub(super) fn void_floor(&mut self, lay: &mut Lay, rng: &mut Pcg32, more: usize) -> Option<Vec<i16>> {
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
    pub(super) fn top_level(&self, lay: &mut Lay) {
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

    /// A wing's storey: chambers off a mural passage along its outer wall from the body when it is
    /// 4 squares across or more (on its other side, `flip`ped), else one or two rooms opening off
    /// each other from the body.
    pub(super) fn wing_storey(&self, lay: &mut Lay, wi: usize, flip: bool, rng: &mut Pcg32) -> Vec<i16> {
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
    pub(super) fn wing_top(&self, lay: &mut Lay, wi: usize) {
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
    pub(super) fn fore_level(&mut self, lay: &mut Lay, z: i8) {
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
    pub(super) fn tower_kinds(&mut self, z: i8) -> (Vec<(usize, &'static str)>, Vec<&'static str>) {
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
            shuffled(&barracks::TOWER_ROOMS)
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
}
