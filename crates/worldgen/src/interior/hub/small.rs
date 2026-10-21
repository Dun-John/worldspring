//! The small tower keep's programme: a body under 7 squares long or 5 across, each storey a room or
//! two off a landing across its high end (`plan`, `Plan::small_level`).

use super::kit::side;
use super::{Frame, Kind, Laid, Lay, Piece, Plan};
use crate::interior::Shell;
use crate::town;

/// A small tower keep's plan (a body under 7 squares long or 5 across, 4 by 4 at least, 3 storeys
/// or more): the main stair against its high end wall (or a square in from it) in a landing across
/// the end, the rest of each storey a room or
/// two off it: an entrance hall to the stair over the stores on the ground, the great hall above,
/// then the lord's rooms (a great chamber, a solar) each by the landing with a chapel, a treasury, a
/// garrison or a closet at the low end (`Plan::small_level`).
pub(super) fn plan(b: &town::Building, sh: &Shell, piece: Vec<Piece>, f: Frame, seed: u64, front: bool) -> Option<Plan> {
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
        pal: None,
        tem: None,
        mon: None,
        rich: false,
    })
}

impl Plan {
    /// A small tower keep's storey (`plan`): the landing across the high end with the main
    /// stair against the end wall; on the ground an entrance hall from it down the body's high half
    /// (the front door on its walls) and the stores below it (an armory and a storeroom side by side
    /// where they fit); the great hall over them; above, the storey's main room by the landing and
    /// a second room at the low end (a band across it where both hold 9 squares, else a corner of 2
    /// by 2, at the other corner on the next storey): the great chamber and the chapel, the solar and
    /// the treasury, a garrison, then chambers and stores.
    pub(super) fn small_level(&mut self, z: i8, mut lay: Lay) -> Laid {
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
        let halls: Vec<crate::interior::hall::Frame> = (z == self.hall_z).then(|| self.hall_frame()).into_iter().collect();
        Laid { cells: lay.cells, rooms: lay.rooms, hints: lay.hints, public, name, roof: false, items: lay.items, towers: Vec::new(), corners: Vec::new(), halls }
    }
}
