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
    /// A chasm to lay across a room once the locks and keys are in (`kit::chasm_in`): the room (-1: across the level),
    /// and whether rope bridges are strung over it (else it is crossed where its planner kept squares `taken`: a stone
    /// bridge).
    pub(crate) chasm: Option<(i16, bool)>,
    /// The stone bridge's squares (kept off the chasm): a `stone_bridge` is laid over those the chasm runs past.
    pub(crate) bridge: Vec<usize>,
    /// Doors its planner would have locked first (a bandit captain's): `kit::guard` takes them before any other while
    /// the site has locks to give, even with one room beyond.
    pub(crate) lock_first: Vec<(usize, usize)>,
    /// Open gaps (square pairs, also in `doors`): where a cave-in broke through a wall (`themes::breach`), drawn as
    /// openings with no leaves (`Door` kind "arch"), never locked nor hidden.
    pub(crate) gaps: FastSet<(usize, usize)>,
    /// A mine's lift shaft, the same on every level (`themes::mine`): its 2 x 2 squares' top-left corner, whether it
    /// lies north of the haulage way (its landing between them), and whether the cage stands at the top (else the foot).
    pub(crate) lift: Option<(usize, bool, bool)>,
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
            bridge: Vec::new(),
            lock_first: Vec::new(),
            gaps: FastSet::default(),
            lift: None,
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
    /// every other floor square stays reachable from the arrival, and every square outside the boss
    /// chamber stays reachable without passing through it (the boss stays a dead end).
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
            // (Round the boss chamber too, unless the item stands in it.)
            let boss = self.rooms.iter().position(|r| r.kind == BOSS).map_or(-1, |b| b as i16);
            let boss = if squares.iter().any(|&k| self.cells[k] == boss) { -1 } else { boss };
            let safe = |avoid: i16| if squares.len() == 1 && self.ring_safe(squares[0], ny, avoid) { true } else { self.joined(&squares, ny, avoid) };
            if !safe(-1) || boss >= 0 && !safe(boss) {
                return false;
            }
            for &k in &squares {
                self.blocked[k] = true;
            }
        }
        for &k in &squares {
            self.taken[k] = true;
        }
        self.items.push(it);
        true
    }

    /// Whether blocking square `k` keeps every square reached (the quick way): the open squares one steps to from `k`
    /// all join up round the ring of eight about it. Squares of room `avoid` count as rock.
    fn ring_safe(&self, k: usize, ny: usize, avoid: i16) -> bool {
        let nx = self.nx;
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        // (The ring from north, clockwise; the steps to `k` at 0, 2, 4, 6.)
        const RING: [(isize, isize); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
        let at = |d: usize| {
            let (x, y) = (i + RING[d].0, j + RING[d].1);
            (x >= 0 && y >= 0 && x < nx as isize && y < ny as isize).then(|| y as usize * nx + x as usize)
        };
        let open = |d: usize| at(d).filter(|&q| self.cells[q] >= 0 && self.cells[q] != avoid && !self.blocked[q]);
        let goes = |d: usize| d % 2 == 0 && open(d).is_some_and(|q| self.passable(k, q));
        let n = (0..8).filter(|&d| goes(d)).count();
        if n <= 1 {
            return true;
        }
        // The arcs of open squares round the ring, each step between neighbours passable: one holds every way out.
        let link = |d: usize| matches!((open(d), open((d + 1) % 8)), (Some(a), Some(b)) if self.passable(a, b));
        let Some(start) = (0..8).find(|&d| !link(d)) else { return true };
        let mut arcs = 0;
        let mut inside = false;
        for s in 1..=8 {
            let d = (start + s) % 8;
            if goes(d) && !inside {
                arcs += 1;
                inside = true;
            }
            if !link(d) {
                inside = false;
            }
        }
        arcs <= 1
    }

    /// Whether blocking `squares` keeps every square reached that was (squares of room `avoid` counted as rock): the open
    /// squares one steps to from them still join up round about. Searched out from each at once, square by square; it
    /// stops when they have all met, or when one's search runs out first (cut off).
    fn joined(&self, squares: &[usize], ny: usize, avoid: i16) -> bool {
        let nx = self.nx;
        let open = |q: usize| self.cells[q] >= 0 && self.cells[q] != avoid && !self.blocked[q] && !squares.contains(&q);
        let mut lab = vec![u16::MAX; nx * ny];
        let mut up: Vec<u16> = Vec::new();
        let mut q = std::collections::VecDeque::new();
        for &s in squares {
            for t in neighbours(s, nx, ny) {
                if open(t) && self.passable(s, t) && lab[t] == u16::MAX {
                    lab[t] = up.len() as u16;
                    up.push(up.len() as u16);
                    q.push_back(t);
                }
            }
        }
        fn top(up: &mut [u16], mut x: u16) -> u16 {
            while up[x as usize] != x {
                up[x as usize] = up[up[x as usize] as usize];
                x = up[x as usize];
            }
            x
        }
        // (Squares waiting per search; a search with none left while others remain is cut off.)
        let mut left: Vec<u32> = vec![1; up.len()];
        let mut groups = up.len();
        while let Some(k) = q.pop_front() {
            if groups <= 1 {
                return true;
            }
            let g = top(&mut up, lab[k]);
            left[g as usize] -= 1;
            for t in neighbours(k, nx, ny) {
                if !open(t) || !self.passable(k, t) {
                    continue;
                }
                if lab[t] == u16::MAX {
                    lab[t] = g;
                    left[g as usize] += 1;
                    q.push_back(t);
                } else {
                    let o = top(&mut up, lab[t]);
                    if o != g {
                        up[o as usize] = g;
                        left[g as usize] += left[o as usize];
                        groups -= 1;
                    }
                }
            }
            if left[g as usize] == 0 && groups > 1 {
                return false;
            }
        }
        groups <= 1
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
