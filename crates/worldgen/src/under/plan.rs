//! A level while it is laid out: the `Plan` the planners fill (rooms, doors, items) and `finish`, which turns it into an
//! `interior::Level` (walls from the room borders, doors from the door pairs).

use crate::core::hash::{FastMap, FastSet};
use crate::interior::{Door, Item, Level, Link, Room, Wall};
use crate::under::*;

pub(crate) struct RoomDef {
    pub(crate) kind: &'static str,
    pub(crate) raise: f32,
    /// Drawn round, octagonal… (`interior::shape`); kept only while its squares fit it.
    pub(crate) shape: u8,
}

pub(crate) struct Plan {
    pub(crate) nx: usize,
    /// Room per square, -1 rock.
    pub(crate) cells: Vec<i16>,
    pub(crate) rooms: Vec<RoomDef>,
    /// Doors between neighbouring squares (smaller index first).
    pub(crate) doors: FastSet<(usize, usize)>,
    pub(crate) natural: bool,
    /// Arrival square (from the surface or the level above) and the way down, if any.
    pub(crate) up: usize,
    pub(crate) down: Option<usize>,
    pub(crate) items: Vec<Item>,
    /// Squares with an item on them, and squares an item may not take (doorways, ways in
    /// and out).
    pub(crate) taken: Vec<bool>,
    pub(crate) blocked: Vec<bool>,
    /// Lines the floor follows (`Level::paths`).
    pub(crate) paths: Vec<[f32; 5]>,
    /// Ways to other sites (`Level::links`).
    pub(crate) links: Vec<Link>,
    /// Doors hidden in the wall (square pairs).
    pub(crate) secret: FastSet<(usize, usize)>,
    /// Portcullises (square pairs): iron grates in the doorway (a design's, or a temple's sanctum gate).
    pub(crate) portcullis: FastSet<(usize, usize)>,
    /// Locked doors (square pairs): the lock's colour, 1-7 (`Door::lock`).
    pub(crate) locks: FastMap<(usize, usize), u8>,
    /// A built level's passage room and whether its corridors are wide (for loops dug later,
    /// `kit::guard`), with the room pairs a loop may join.
    pub(crate) passage: Option<(i16, bool)>,
    pub(crate) shortcuts: Vec<(i16, i16)>,
    /// Water to lay once the locks and keys are in (`kit::water_at`): per square the noise heights over which it is
    /// shallow and deep (`grow::flood_room`); empty: a dry level.
    pub(crate) water: Vec<[f32; 2]>,
    /// Line the passage's narrow galleries with burial niches when furnished (`kit::furnish`).
    pub(crate) niches: bool,
    /// The share of its usual loops `kit::guard` adds (doors through shared walls, shortcuts): 1 but on catacombs.
    pub(crate) loops: f64,
    /// Most secret doors the level gets once its planner's are in (`kit::guard` adds none past it).
    pub(crate) secrets: usize,
    /// A chasm to lay across a room once the locks and keys are in (`kit::chasm_in`): the room, and whether rope bridges
    /// are strung over it (else it is crossed where its planner kept squares `taken`: a stone bridge).
    pub(crate) chasm: Option<(i16, bool)>,
    /// Doors its planner would have locked first (a bandit captain's): `kit::guard` takes them before any other while
    /// the site has locks to give, even with one room beyond.
    pub(crate) lock_first: Vec<(usize, usize)>,
    /// Open gaps (square pairs, also in `doors`): where a cave-in broke through a wall (`themes::breach`), drawn as
    /// openings with no leaves (`Door` kind "arch"), never locked nor hidden.
    pub(crate) gaps: FastSet<(usize, usize)>,
}

impl Plan {
    pub(crate) fn new(nx: usize, ny: usize, up: usize, natural: bool) -> Plan {
        Plan {
            nx,
            cells: vec![-1; nx * ny],
            rooms: Vec::new(),
            doors: FastSet::default(),
            natural,
            up,
            down: None,
            items: Vec::new(),
            taken: vec![false; nx * ny],
            blocked: vec![false; nx * ny],
            paths: Vec::new(),
            links: Vec::new(),
            secret: FastSet::default(),
            portcullis: FastSet::default(),
            locks: FastMap::default(),
            passage: None,
            shortcuts: Vec::new(),
            water: Vec::new(),
            niches: false,
            loops: 1.0,
            secrets: usize::MAX,
            chasm: None,
            lock_first: Vec::new(),
            gaps: FastSet::default(),
        }
    }

    pub(crate) fn room(&mut self, kind: &'static str, raise: f32) -> i16 {
        self.rooms.push(RoomDef { kind, raise, shape: 0 });
        (self.rooms.len() - 1) as i16
    }

    pub(crate) fn door(&mut self, a: usize, b: usize) {
        self.doors.insert((a.min(b), a.max(b)));
    }

    /// Whether one can step between neighbouring squares a and b.
    pub(crate) fn passable(&self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.cells[a], self.cells[b]);
        ra >= 0 && rb >= 0 && (ra == rb || self.natural || self.doors.contains(&(a.min(b), a.max(b))))
    }

    /// Step distances from `start` (-1 unreachable), around `blocked` squares.
    pub(crate) fn distances(&self, start: usize, ny: usize, blocked: Option<&[bool]>) -> Vec<i32> {
        let nx = self.nx;
        let mut dist = vec![-1; nx * ny];
        let mut q = std::collections::VecDeque::new();
        dist[start] = 0;
        q.push_back(start);
        while let Some(k) = q.pop_front() {
            for n in neighbours(k, nx, ny) {
                if dist[n] < 0 && self.passable(k, n) && blocked.is_none_or(|b| !b[n]) {
                    dist[n] = dist[k] + 1;
                    q.push_back(n);
                }
            }
        }
        dist
    }

    /// Put an item without checks (ways in and out, lava, terrain): its squares are reserved.
    /// A span (rope bridge, stepping stones) clears the deep water or chasm under it.
    pub(crate) fn put(&mut self, it: Item) {
        self.put_all(std::iter::once(it));
    }

    /// `put` for many at once (what the terrain tools return: `water`, `chasm`, `rubble_slope`):
    /// the squares under terrain and spans are worked out again in one pass at the end.
    pub(crate) fn put_all(&mut self, items: impl IntoIterator<Item = Item>) {
        let mut over: Vec<(usize, usize, usize, usize)> = Vec::new();
        for it in items {
            let (x, y, w, h) = (it.x as usize, it.y as usize, it.w as usize, it.h as usize);
            if SPANS.contains(&it.kind) || it.blocks_move && TERRAIN.contains(&it.kind) {
                over.push((x, y, w, h));
            }
            for j in y..y + h {
                for i in x..x + w {
                    self.taken[j * self.nx + i] = true;
                    if it.blocks_move {
                        self.blocked[j * self.nx + i] = true;
                    }
                }
            }
            self.items.push(it);
        }
        if !over.is_empty() {
            let b = blocked_squares(&self.items, self.nx, self.cells.len() / self.nx);
            for (x, y, w, h) in over {
                for j in y..y + h {
                    for i in x..x + w {
                        self.blocked[j * self.nx + i] = b[j * self.nx + i];
                    }
                }
            }
        }
    }

    /// Place an item if its squares are free floor (of `room`, if given) and, if it blocks,
    /// every other floor square stays reachable from the arrival.
    pub(crate) fn place(&mut self, it: Item, room: Option<i16>, ny: usize) -> bool {
        let nx = self.nx;
        let (x, y, w, h) = (it.x as usize, it.y as usize, it.w as usize, it.h as usize);
        if x + w > nx || y + h > ny {
            return false;
        }
        let squares: Vec<usize> = (y..y + h).flat_map(|j| (x..x + w).map(move |i| j * nx + i)).collect();
        if squares.iter().any(|&k| self.cells[k] < 0 || self.taken[k] || room.is_some_and(|r| self.cells[k] != r)) {
            return false;
        }
        if it.blocks_move {
            let mut blocked = self.blocked.clone();
            for &k in &squares {
                blocked[k] = true;
            }
            let dist = self.distances(self.up, ny, Some(&blocked));
            if (0..nx * ny).any(|k| self.cells[k] >= 0 && !blocked[k] && dist[k] < 0) {
                return false;
            }
            self.blocked = blocked;
        }
        for &k in &squares {
            self.taken[k] = true;
        }
        self.items.push(it);
        true
    }

    /// Doorways stay clear.
    pub(crate) fn reserve_doors(&mut self) {
        let ds: Vec<(usize, usize)> = self.doors.iter().copied().collect();
        for (a, b) in ds {
            self.taken[a] = true;
            self.taken[b] = true;
        }
    }

    pub(crate) fn finish(self, nx: usize, ny: usize, z: i8, name: String, elevation_ft: f32) -> Level {
        let mut rooms: Vec<Room> = self
            .rooms
            .iter()
            .enumerate()
            .map(|(ri, r)| {
                let shape = if r.shape != 0 && crate::interior::shape::fits(&self.cells, nx, ri as i16, r.shape) { r.shape } else { 0 };
                Room { kind: r.kind, squares: 0, raise_ft: r.raise, center: [0.0; 2], shape }
            })
            .collect();
        let mut sum = vec![[0.0f64; 2]; rooms.len()];
        for k in 0..nx * ny {
            let r = self.cells[k];
            if r >= 0 {
                rooms[r as usize].squares += 1;
                sum[r as usize][0] += (k % nx) as f64 + 0.5;
                sum[r as usize][1] += (k / nx) as f64 + 0.5;
            }
        }
        for (r, s) in rooms.iter_mut().zip(&sum) {
            if r.squares > 0 {
                r.center = [(s[0] / r.squares as f64) as f32, (s[1] / r.squares as f64) as f32];
            }
        }
        // Walls on square edges: rock against floor (exterior), and between rooms where no
        // door is (built levels only). Unit edges, merged into runs.
        let cell = |i: isize, j: isize| {
            if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { self.cells[j as usize * nx + i as usize] }
        };
        let mut hs: Vec<(isize, isize, bool)> = Vec::new();
        let mut vs: Vec<(isize, isize, bool)> = Vec::new();
        let mut doors = Vec::new();
        for j in 0..=ny as isize {
            for i in 0..=nx as isize {
                // Edge above square (i, j): between (i, j-1) and (i, j).
                for (horizontal, a, b) in [(true, (i, j - 1), (i, j)), (false, (i - 1, j), (i, j))] {
                    if (horizontal && i >= nx as isize) || (!horizontal && j >= ny as isize) {
                        continue;
                    }
                    let (ra, rb) = (cell(a.0, a.1), cell(b.0, b.1));
                    if ra < 0 && rb < 0 || ra == rb {
                        continue;
                    }
                    let edge = (j, i, ra < 0 || rb < 0);
                    if !edge.2 {
                        let (ka, kb) = (a.1 as usize * nx + a.0 as usize, b.1 as usize * nx + b.0 as usize);
                        if self.doors.contains(&(ka.min(kb), ka.max(kb))) {
                            let (p0, p1) = if horizontal { ([i as f32, j as f32], [i as f32 + 1.0, j as f32]) } else { ([i as f32, j as f32], [i as f32, j as f32 + 1.0]) };
                            let e = (ka.min(kb), ka.max(kb));
                            let kind = if self.gaps.contains(&e) { "arch" } else if self.secret.contains(&e) { "secret" } else if self.portcullis.contains(&e) { "portcullis" } else { "door" };
                            let lock = self.locks.get(&(ka.min(kb), ka.max(kb))).copied().unwrap_or(0);
                            doors.push(Door { a: p0, b: p1, kind, rooms: [ra, rb], lock });
                            continue;
                        }
                        if self.natural {
                            continue;
                        }
                    }
                    if horizontal {
                        hs.push(edge);
                    } else {
                        vs.push((i, j, edge.2));
                    }
                }
            }
        }
        let mut walls = Vec::new();
        for (runs, horizontal) in [(hs, true), (vs, false)] {
            // (line, position along it, exterior), sorted by line then position.
            let mut runs = runs;
            runs.sort();
            let mut k = 0;
            while k < runs.len() {
                let (line, start, ext) = runs[k];
                let mut end = start + 1;
                while k + 1 < runs.len() && runs[k + 1] == (line, end, ext) {
                    end += 1;
                    k += 1;
                }
                let (a, b) = if horizontal { ([start as f32, line as f32], [end as f32, line as f32]) } else { ([line as f32, start as f32], [line as f32, end as f32]) };
                walls.push(Wall { a, b, exterior: ext, rail: false, arcade: false });
                k += 1;
            }
        }
        Level { z, name, elevation_ft, cells: self.cells, rooms, walls, doors, windows: Vec::new(), furniture: self.items, roof: false, has_stairs: false, natural: self.natural, paths: self.paths, links: self.links }
    }
}
