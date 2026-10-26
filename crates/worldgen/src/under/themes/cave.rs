//! Caves (every cave theme): chambers and tunnels. `caverns` seeds chambers across a region (Poisson-ish darts, each
//! seed a growth weight so their sizes come out uneven), grows them together over the squares (least cost from every
//! seed at once, the cost noisy so the edges wander) and leaves a square of rock wherever two would meet; then joins
//! neighbouring chambers (a relative-neighbourhood graph over their middles, with a spanning tree under it so all
//! join up) through the shared wall, or by a short tunnel dug through the rock where none touch: some joins tight
//! 1-square squeezes, the rest two or three wide; a few loops, the rest dead ends. `cave` finishes a level from them:
//! raised ledges reached by rubble slopes, the way down or the boss chamber (a dead end) at the far end, water by
//! theme (flooded caves mostly shallow with deep channels and a lake; plain and fungal caves a pool or two and
//! sometimes a lake on the low ground; crystal, ice and beast-den caves mostly dry), islands in lakes (treasure, a
//! shrine or the boss), rock columns and stalagmites standing in the big chambers, sometimes a chasm across the
//! level. `natural_level` turns open squares into chambers (lava tubes).

use crate::core::rng::Pcg32;
use crate::under::*;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// The squares round square `k` (8 ways), inside the grid.
pub(crate) fn around(k: usize, nx: usize, ny: usize) -> impl Iterator<Item = usize> {
    let (i, j) = ((k % nx) as isize, (k / nx) as isize);
    (-1..=1isize)
        .flat_map(move |dj| (-1..=1isize).map(move |di| (i + di, j + dj)))
        .filter(move |&(x, y)| (x, y) != (i, j) && x >= 0 && y >= 0 && x < nx as isize && y < ny as isize)
        .map(move |(x, y)| y as usize * nx + x as usize)
}

/// What `caverns` grows.
pub(crate) struct Caverns<'a> {
    /// Chamber kinds, picked at random.
    pub(crate) kinds: &'a [&'static str],
    /// A raised chamber's kind ("" none raised).
    pub(crate) ledge: &'static str,
    /// About how many squares apart the chambers' seeds lie.
    pub(crate) spacing: f64,
    /// Squares that seed a chamber first (the arrival): never raised nor the dead end.
    pub(crate) seeds: &'a [usize],
    /// The chance a chamber is raised (5 or 10 ft).
    pub(crate) raise: f64,
    /// The chance a join through a shared wall stays a 1-square squeeze.
    pub(crate) squeeze: f64,
    /// The share of the extra joins (beyond the spanning tree) kept: the loops.
    pub(crate) loops: f64,
    /// The longest tunnel dug through rock between chambers that don't touch.
    pub(crate) reach: u32,
    /// Leave the chamber farthest (in joins) from the first seed's (or the floor there before) a dead end with one
    /// join: `Grown::end`.
    pub(crate) dead_end: bool,
}

/// A join between two chambers (`Grown::rooms`; -1: the floor there before): the squares dug through the rock.
pub(crate) struct Join {
    pub(crate) a: i16,
    pub(crate) b: i16,
    pub(crate) squares: Vec<usize>,
}

/// What `caverns` grew: its chambers' rooms, their joins, the dead end (`Caverns::dead_end`).
#[derive(Default)]
pub(crate) struct Grown {
    pub(crate) rooms: Vec<i16>,
    pub(crate) joins: Vec<Join>,
    pub(crate) end: Option<i16>,
}

/// One possible join: the nodes (chambers; the floor there before last), its tunnel's length and squares, the squares
/// each end starts from, and (a 1-square wall) every square of that wall between the two.
struct Cand {
    a: usize,
    b: usize,
    len: u32,
    path: Vec<usize>,
    ends: (usize, usize),
}

/// Grow cave chambers over the rock squares of `region` (inside the ring of rock round the level, but for `seeds`),
/// a square of rock kept between them and round any floor already there; join them as the module says (each chamber
/// reached from the first seed's chamber, or from the floor already there: then that floor is a node joined to the
/// nearest chambers by tunnels too). Chambers no join reaches are left rock. Laid on `p` as rooms (kinds from
/// `kinds`, some raised `ledge`s); the joins' squares belong to one of their two rooms (never the dead end's).
pub(crate) fn caverns(p: &mut Plan, ny: usize, region: &[bool], c: &Caverns, rng: &mut Pcg32) -> Grown {
    let nx = p.nx;
    let n = nx * ny;
    let inner = |k: usize| {
        let (i, j) = (k % nx, k / nx);
        i >= 1 && j >= 1 && i + 1 < nx && j + 1 < ny
    };
    let old: Vec<bool> = p.cells.iter().map(|&r| r >= 0).collect();
    let any_old = old.iter().any(|&o| o);
    let near_old: Vec<bool> = (0..n).map(|k| !old[k] && around(k, nx, ny).any(|m| old[m])).collect();
    // (Squares a chamber or tunnel may take; chambers keep off the floor already there.)
    let open: Vec<bool> = (0..n).map(|k| region[k] && !old[k] && (inner(k) || c.seeds.contains(&k))).collect();
    let free: Vec<bool> = (0..n).map(|k| open[k] && !near_old[k]).collect();
    let pool: Vec<usize> = (0..n).filter(|&k| free[k]).collect();
    if pool.is_empty() {
        return Grown::default();
    }
    // Seeds: the given ones, then darts at least `spacing` * 0.8 apart; each a growth weight (uneven sizes).
    let mut seeds: Vec<(usize, f64)> = Vec::new();
    for &k in c.seeds {
        if k < n && free[k] && !seeds.iter().any(|s| s.0 == k) {
            seeds.push((k, rng.range(1.0, 1.4)));
        }
    }
    let given = seeds.len();
    let gap = c.spacing * 0.8;
    let d2 = |a: usize, b: usize| {
        let (dx, dy) = ((a % nx) as f64 - (b % nx) as f64, (a / nx) as f64 - (b / nx) as f64);
        dx * dx + dy * dy
    };
    for _ in 0..pool.len() / 3 + 20 {
        let k = pool[rng.below(pool.len() as u32) as usize];
        if seeds.iter().all(|s| d2(s.0, k) >= gap * gap) {
            let u = rng.next_f64();
            seeds.push((k, if u < 0.4 { 0.45 + u } else if u < 0.85 { 0.85 + (u - 0.4) } else { 1.3 + 3.0 * (u - 0.85) }));
        }
    }
    let m = seeds.len();
    // Growth: out from every seed at once, nearest first (the distance noisy, over the seed's weight, up to a budget),
    // square by square; a square touching (8 ways) another chamber's stays rock: the wall between them.
    let ns = rng.next_u32() as u64;
    let budget = (c.spacing * 0.6 * 1000.0) as u32;
    let mut own = vec![-1i32; n];
    let mut best = vec![u32::MAX; n];
    let mut heap = BinaryHeap::new();
    for (s, &(k, _)) in seeds.iter().enumerate() {
        best[k] = 0;
        heap.push(Reverse((0u32, k, s)));
    }
    while let Some(Reverse((cost, k, s))) = heap.pop() {
        if own[k] != -1 || cost > best[k] {
            continue;
        }
        if around(k, nx, ny).any(|q| own[q] >= 0 && own[q] as usize != s) {
            own[k] = -2;
            continue;
        }
        own[k] = s as i32;
        for q in neighbours(k, nx, ny) {
            if own[q] != -1 || !free[q] {
                continue;
            }
            // (The distance from the seed over its weight, stretched by the noise: round chambers with lobes.)
            let reach = (crate::core::sqrt(d2(seeds[s].0, q)) + 4.0 * (noise(ns, q % nx, q / nx, 3.5) - 0.5)).max(0.0) / seeds[s].1;
            let next = (reach * 1000.0) as u32;
            if next <= budget && next < best[q] {
                best[q] = next;
                heap.push(Reverse((next, q, s)));
            }
        }
    }
    // Clean: spurs off (a square with one neighbour of its chamber or none), holes filled (rock with three sides of one
    // chamber and nothing else round it), crumbs (under 8 squares) back to rock.
    let seed_at = |k: usize| seeds[..given].iter().any(|s| s.0 == k);
    for _ in 0..2 {
        for k in 0..n {
            if own[k] >= 0 && !seed_at(k) && neighbours(k, nx, ny).filter(|&q| own[q] == own[k]).count() <= 1 {
                own[k] = -2;
            }
        }
    }
    for k in 0..n {
        if own[k] >= 0 || !free[k] {
            continue;
        }
        let Some(s) = neighbours(k, nx, ny).map(|q| own[q]).find(|&r| r >= 0) else { continue };
        if neighbours(k, nx, ny).filter(|&q| own[q] == s).count() >= 3 && around(k, nx, ny).all(|q| own[q] < 0 || own[q] == s) {
            own[k] = s;
        }
    }
    let mut size = vec![0usize; m];
    for k in 0..n {
        if own[k] >= 0 {
            size[own[k] as usize] += 1;
        }
    }
    for k in 0..n {
        if own[k] >= 0 && size[own[k] as usize] < 8 && own[k] as usize >= given {
            own[k] = -2;
        }
    }
    // Possible joins: from every chamber (and the floor there before, node `m`) out through the rock, nearest first,
    // up to `reach`; where two fronts meet, a tunnel of their summed lengths. Kept only if it runs beside no other.
    let node = |k: usize| if own[k] >= 0 { own[k] as usize } else if old[k] { m } else { usize::MAX };
    let mut lab = vec![usize::MAX; n];
    let mut dist = vec![0u32; n];
    let mut par = vec![usize::MAX; n];
    let mut q = VecDeque::new();
    for k in 0..n {
        if node(k) != usize::MAX {
            lab[k] = node(k);
            q.push_back(k);
        }
    }
    while let Some(k) = q.pop_front() {
        if dist[k] >= c.reach {
            continue;
        }
        for t in neighbours(k, nx, ny) {
            if lab[t] == usize::MAX && open[t] && inner(t) {
                lab[t] = lab[k];
                dist[t] = dist[k] + 1;
                par[t] = k;
                q.push_back(t);
            }
        }
    }
    let chain = |mut k: usize, out: &mut Vec<usize>| {
        while dist[k] > 0 {
            out.push(k);
            k = par[k];
        }
        k
    };
    let mut raw: Vec<(usize, usize, u32, usize, usize)> = Vec::new();
    for k in 0..n {
        for t in [(k % nx + 1 < nx).then(|| k + 1), (k + nx < n).then(|| k + nx)].into_iter().flatten() {
            let (la, lb) = (lab[k], lab[t]);
            if la == usize::MAX || lb == usize::MAX || la == lb || dist[k] + dist[t] == 0 || dist[k] + dist[t] > c.reach {
                continue;
            }
            let (x, y) = if la < lb { (k, t) } else { (t, k) };
            raw.push((la.min(lb), la.max(lb), dist[k] + dist[t], x, y));
        }
    }
    raw.sort_unstable();
    let mut cands: Vec<Cand> = Vec::new();
    let mut walls: Vec<(usize, usize, usize)> = Vec::new();
    for &(a, b, len, x, y) in &raw {
        let mut path = Vec::new();
        let ea = chain(x, &mut path);
        let eb = chain(y, &mut path);
        let ok = path.iter().all(|&s| neighbours(s, nx, ny).all(|t| path.contains(&t) || node(t) == usize::MAX || node(t) == a || node(t) == b));
        if !ok {
            continue;
        }
        if len == 1 {
            walls.push((a, b, path[0]));
        }
        if cands.last().is_none_or(|e| (e.a, e.b) != (a, b)) {
            cands.push(Cand { a, b, len, path, ends: (ea, eb) });
        }
    }
    // The graph: a spanning tree of the shortest joins, and the relative-neighbourhood ones over the chambers' middles
    // beyond it by `loops` (the floor there before joins by the tree, or as a loop at half that).
    let mut mid = vec![[0.0f64; 2]; m];
    for k in 0..n {
        if own[k] >= 0 {
            let s = own[k] as usize;
            mid[s][0] += (k % nx) as f64;
            mid[s][1] += (k / nx) as f64;
        }
    }
    let mut size = vec![0usize; m];
    for k in 0..n {
        if own[k] >= 0 {
            size[own[k] as usize] += 1;
        }
    }
    for s in 0..m {
        if size[s] > 0 {
            mid[s] = [mid[s][0] / size[s] as f64, mid[s][1] / size[s] as f64];
        }
    }
    let far = |a: usize, b: usize| {
        let (dx, dy) = (mid[a][0] - mid[b][0], mid[a][1] - mid[b][1]);
        crate::core::sqrt(dx * dx + dy * dy)
    };
    let weight = |e: &Cand| if e.b == m { e.len as f64 } else { far(e.a, e.b) + 0.5 * e.len as f64 };
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|&i, &j| weight(&cands[i]).total_cmp(&weight(&cands[j])).then(i.cmp(&j)));
    let mut up: Vec<usize> = (0..=m).collect();
    fn top(up: &mut [usize], mut x: usize) -> usize {
        while up[x] != x {
            up[x] = up[up[x]];
            x = up[x];
        }
        x
    }
    let mut keep = vec![false; cands.len()];
    for &i in &order {
        let (ra, rb) = (top(&mut up, cands[i].a), top(&mut up, cands[i].b));
        if ra != rb {
            up[ra] = rb;
            keep[i] = true;
        }
    }
    let present: Vec<usize> = (0..m).filter(|&s| size[s] > 0).collect();
    for &i in &order {
        if keep[i] {
            continue;
        }
        let e = &cands[i];
        let chance = if e.b == m {
            c.loops * 0.5
        } else {
            let dab = far(e.a, e.b);
            if present.iter().any(|&o| o != e.a && o != e.b && far(e.a, o).max(far(e.b, o)) < dab) {
                continue;
            }
            c.loops
        };
        if rng.next_f64() < chance {
            keep[i] = true;
        }
    }
    // Chambers the joins leave apart from the root's: a tunnel dug from the root's side to the nearest of them, as long
    // as it must be, running beside nothing but its two ends; the rest are left rock.
    let root = if any_old { m } else if given > 0 && size[0] > 0 { 0 } else { present.iter().copied().max_by_key(|&s| (size[s], usize::MAX - s)).unwrap_or(0) };
    loop {
        let r = top(&mut up, root);
        let mut side = vec![false; m + 1];
        for x in 0..=m {
            side[x] = top(&mut up, x) == r;
        }
        if present.iter().all(|&s| side[s]) {
            break;
        }
        let mut src = vec![usize::MAX; n];
        let mut back = vec![usize::MAX; n];
        let mut q = VecDeque::new();
        for k in 0..n {
            if node(k) != usize::MAX && side[node(k)] {
                src[k] = node(k);
                q.push_back(k);
            }
        }
        let mut found = None;
        'search: while let Some(k) = q.pop_front() {
            for t in neighbours(k, nx, ny) {
                if src[t] != usize::MAX || !open[t] || !inner(t) || node(t) != usize::MAX {
                    continue;
                }
                let by: Vec<usize> = neighbours(t, nx, ny).map(node).filter(|&x| x != usize::MAX && x != src[k]).collect();
                if by.iter().any(|&x| side[x]) {
                    continue;
                }
                src[t] = src[k];
                back[t] = k;
                if let Some(&x) = by.first() {
                    if by.iter().all(|&y| y == x) {
                        found = Some((t, x));
                        break 'search;
                    }
                    continue;
                }
                q.push_back(t);
            }
        }
        let Some((t, x)) = found else { break };
        let mut path = Vec::new();
        let mut k = t;
        while node(k) == usize::MAX {
            path.push(k);
            k = back[k];
        }
        let into = neighbours(t, nx, ny).find(|&u| node(u) == x).unwrap();
        let (a, b) = (src[t].min(x), src[t].max(x));
        let ends = if src[t] < x { (k, into) } else { (into, k) };
        let (ra, rb) = (top(&mut up, a), top(&mut up, b));
        up[ra] = rb;
        keep.push(true);
        order.push(cands.len());
        cands.push(Cand { a, b, len: path.len() as u32, path, ends });
    }
    // What the root reaches (hops), the dead end.
    let mut hop = vec![u32::MAX; m + 1];
    hop[root] = 0;
    let mut q = VecDeque::from([root]);
    while let Some(x) = q.pop_front() {
        for (i, e) in cands.iter().enumerate() {
            if keep[i] && (e.a == x || e.b == x) {
                let o = if e.a == x { e.b } else { e.a };
                if hop[o] == u32::MAX {
                    hop[o] = hop[x] + 1;
                    q.push_back(o);
                }
            }
        }
    }
    let mut end = None;
    if c.dead_end {
        let deepest = present.iter().filter(|&&s| s >= given && hop[s] != u32::MAX).map(|&s| hop[s]).max();
        if let Some(h) = deepest.filter(|&h| h > 0) {
            let e = present.iter().copied().filter(|&s| s >= given && hop[s] == h).max_by_key(|&s| (size[s], usize::MAX - s)).unwrap();
            // Its one join: to a chamber a hop nearer, the shortest.
            let one = (0..cands.len()).filter(|&i| keep[i] && (cands[i].a == e || cands[i].b == e)).filter(|&i| hop[if cands[i].a == e { cands[i].b } else { cands[i].a }] + 1 == h).min_by_key(|&i| (cands[i].len, i));
            for i in 0..cands.len() {
                if keep[i] && (cands[i].a == e || cands[i].b == e) && Some(i) != one {
                    keep[i] = false;
                }
            }
            end = Some(e);
        }
    }
    // Lay the chambers the root reaches as rooms.
    let mut room = vec![-1i16; m];
    let mut out = Grown::default();
    for &s in &present {
        if hop[s] == u32::MAX {
            continue;
        }
        let raised = s >= given && Some(s) != end && !c.ledge.is_empty() && rng.next_f64() < c.raise;
        let (kind, raise) = if raised { (c.ledge, if rng.next_f64() < 0.7 { 5.0 } else { 10.0 }) } else { (c.kinds[rng.below(c.kinds.len() as u32) as usize], 0.0) };
        room[s] = p.room(kind, raise);
        out.rooms.push(room[s]);
    }
    for k in 0..n {
        if own[k] >= 0 && room[own[k] as usize] >= 0 {
            p.cells[k] = room[own[k] as usize];
        }
    }
    out.end = end.map(|e| room[e]);
    // Dig the joins: a wall's squeeze or two or three squares of it, a tunnel (sometimes two wide).
    for &i in &order {
        if !keep[i] {
            continue;
        }
        let e = &cands[i];
        if (e.a < m && room[e.a] < 0) || (e.b < m && room[e.b] < 0) {
            continue;
        }
        let mut sq = e.path.clone();
        if e.len == 1 {
            if rng.next_f64() >= c.squeeze {
                let width = 2 + rng.below(2) as usize;
                let wall: Vec<usize> = walls.iter().filter(|w| (w.0, w.1) == (e.a, e.b)).map(|w| w.2).collect();
                // (Along the wall the two share, else into the rock beside the opening where that touches neither.)
                let beside = |t: usize, sq: &[usize]| {
                    open[t] && inner(t) && own[t] < 0 && !sq.contains(&t) && neighbours(t, nx, ny).any(|u| sq.contains(&u)) && neighbours(t, nx, ny).all(|u| sq.contains(&u) || own[u] < 0 && !old[u] && p.cells[u] < 0 || node(u) == e.a || node(u) == e.b)
                };
                while sq.len() < width {
                    let more = wall.iter().copied().find(|&t| !sq.contains(&t) && neighbours(t, nx, ny).any(|u| sq.contains(&u)));
                    let Some(t) = more.or_else(|| sq.iter().flat_map(|&u| neighbours(u, nx, ny)).find(|&t| beside(t, &sq))) else { break };
                    sq.push(t);
                }
            }
        } else if rng.next_f64() < 0.4 {
            let (ax, ay) = ((e.ends.0 % nx) as isize - (e.ends.1 % nx) as isize, (e.ends.0 / nx) as isize - (e.ends.1 / nx) as isize);
            let side = if ax.abs() >= ay.abs() { nx } else { 1 };
            for &s in &e.path.clone() {
                let t = s + side;
                if t < n && open[t] && inner(t) && node(t) == usize::MAX && p.cells[t] < 0 && !sq.contains(&t) {
                    let ok = neighbours(t, nx, ny).all(|u| sq.contains(&u) || p.cells[u] < 0 && !old[u] || node(u) == e.a || node(u) == e.b);
                    if ok {
                        sq.push(t);
                    }
                }
            }
        }
        // (Its squares to the end that is not the dead end; the floor there before keeps its own rooms.)
        let (ra, rb) = (p.cells[e.ends.0], p.cells[e.ends.1]);
        let owner = if Some(e.a) == end || (e.a == m) { rb } else { ra };
        for &s in &sq {
            if p.cells[s] < 0 {
                p.cells[s] = owner;
            }
        }
        let id = |x: usize| if x == m { -1 } else { room[x] };
        out.joins.push(Join { a: id(e.a), b: id(e.b), squares: sq });
    }
    out
}

/// The squares of room `r` and each one's steps in from its edge (1 on a square beside another room or rock).
pub(crate) fn depths(p: &Plan, ny: usize, r: i16) -> Vec<(usize, u32)> {
    let nx = p.nx;
    let mut e = vec![0u32; nx * ny];
    let mut q = VecDeque::new();
    let sq: Vec<usize> = (0..nx * ny).filter(|&k| p.cells[k] == r).collect();
    for &k in &sq {
        if neighbours(k, nx, ny).count() < 4 || neighbours(k, nx, ny).any(|t| p.cells[t] != r) {
            e[k] = 1;
            q.push_back(k);
        }
    }
    while let Some(k) = q.pop_front() {
        for t in neighbours(k, nx, ny) {
            if p.cells[t] == r && e[t] == 0 {
                e[t] = e[k] + 1;
                q.push_back(t);
            }
        }
    }
    sq.into_iter().map(|k| (k, e[k])).collect()
}

/// Water bands (`Plan::water`): dry, shallow, deep whatever the noise.
pub(crate) const DRY: [f32; 2] = [2.0, 2.0];
pub(crate) const SHALLOW: [f32; 2] = [0.0, 2.0];
pub(crate) const DEEP: [f32; 2] = [0.0, 0.0];

/// A lake over room `r`: deep in the middle, round it a ring of wading shallows or, on one side, a dry shore ledge
/// with shallows inside it (the deep reaching the rock on the other side); with `island`, its middle left dry (for the
/// `boss`: a bigger one, the rest deep). The island's squares, if any.
pub(crate) fn lake(p: &Plan, ny: usize, w: &mut [[f32; 2]], r: i16, island: bool, boss: bool, rng: &mut Pcg32) -> Vec<usize> {
    let nx = p.nx;
    let sq = depths(p, ny, r);
    let deepest = sq.iter().map(|s| s.1).max().unwrap_or(0);
    let (mut cx, mut cy) = (0.0, 0.0);
    for &(k, _) in &sq {
        cx += (k % nx) as f64;
        cy += (k / nx) as f64;
    }
    let count = sq.len().max(1) as f64;
    let (cx, cy) = (cx / count, cy / count);
    let ledge = rng.next_f64() < 0.5;
    let a = rng.range(0.0, std::f64::consts::TAU);
    let (dx, dy) = (libm::cos(a), libm::sin(a));
    // (An island stands three squares of deep water or more off the shore: crossed by stones or a bridge, never waded.)
    // (The boss's lake is deep to the rock all round: its chamber is a dead end, the way in a tunnel off another.)
    let isle = island && deepest >= 5;
    let core = deepest - (boss || deepest >= 7) as u32;
    let mut out = Vec::new();
    for &(k, e) in &sq {
        let side = ((k % nx) as f64 - cx) * dx + ((k / nx) as f64 - cy) * dy > 0.0;
        w[k] = if isle && e >= core {
            out.push(k);
            DRY
        } else if boss {
            DEEP
        } else if ledge && side {
            if e <= 1 { DRY } else if e == 2 && !isle { SHALLOW } else { DEEP }
        } else if ledge {
            DEEP
        } else if e <= 1 {
            SHALLOW
        } else {
            DEEP
        };
    }
    out
}

/// A pool in room `r`: its lower squares (the noise height under the room's 45th percentile) shallow, its lowest
/// seventh deep (two in from the edge or more) if `deep`.
fn pool(p: &Plan, ny: usize, w: &mut [[f32; 2]], r: i16, h: &dyn Fn(usize) -> f64, deep: bool) {
    let sq = depths(p, ny, r);
    let mut hs: Vec<f64> = sq.iter().map(|s| h(s.0)).collect();
    hs.sort_by(|a, b| a.total_cmp(b));
    if hs.is_empty() {
        return;
    }
    let (lo, hi) = (hs[hs.len() * 9 / 20], hs[hs.len() / 7]);
    for &(k, e) in &sq {
        let v = h(k);
        if v <= hi && e >= 2 && deep {
            w[k] = DEEP;
        } else if v <= lo {
            w[k] = SHALLOW;
        }
    }
}

/// A cave level: chambers and tunnels (`caverns`), then as the module says.
pub(crate) fn cave(nx: usize, ny: usize, arrive: usize, last: bool, theme: &Theme, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut p = Plan::new(nx, ny, arrive, true);
    // The region: in from the grid's edge by one to four squares (a ragged outline), less a few masses of rock; all
    // round the arrival.
    let rs = rng.next_u32() as u64;
    let region: Vec<bool> = (0..n)
        .map(|k| {
            let (i, j) = (k % nx, k / nx);
            let edge = i.min(j).min(nx - 1 - i).min(ny - 1 - j) as f64;
            let (di, dj) = (i.abs_diff(arrive % nx), j.abs_diff(arrive / nx));
            di * di + dj * dj <= 20 || (edge >= 1.0 + 3.5 * noise(rs, i, j, 6.0) && noise(rs ^ 0x9e37, i, j, 7.0) < 0.74)
        })
        .collect();
    let raise = match theme.key {
        "flooded" => 0.2,
        "beast_den" => 0.3,
        _ => 0.25,
    };
    let c = Caverns { kinds: theme.kinds, ledge: theme.ledge, spacing: 11.0, seeds: &[arrive], raise, squeeze: 0.3, loops: 0.45, reach: 7, dead_end: last };
    let g = caverns(&mut p, ny, &region, &c, rng);
    if p.cells[arrive] < 0 || (last && g.end.is_none()) {
        // (Never on a grid big enough for a few chambers: the open squares as a natural level.)
        let open: Vec<bool> = (0..n).map(|k| k == arrive || p.cells[k] >= 0).collect();
        return natural_level(nx, ny, arrive, last, open, theme.kinds, theme.ledge, rng);
    }
    let a = p.cells[arrive] as usize;
    p.rooms[a].raise = 0.0;
    p.taken[arrive] = true;
    let dist = p.distances(arrive, ny, None);
    let boss = g.end;
    // (Now and then the boss on an island in its own lake, in a wet cave.)
    let boss_isle = boss.is_some() && matches!(theme.key, "flooded" | "cave" | "fungal") && rng.next_f64() < if theme.key == "flooded" { 0.6 } else { 0.3 };
    if let Some(b) = boss {
        p.rooms[b as usize].kind = BOSS;
        p.rooms[b as usize].raise = 0.0;
        // A small boss chamber widened into the rock where that touches nothing else (a big one for an island).
        let (want, rounds) = if boss_isle { (150, 10) } else { (36, 3) };
        for _ in 0..rounds {
            if (0..n).filter(|&k| p.cells[k] == b).count() >= want {
                break;
            }
            let grow: Vec<usize> = (0..n)
                .filter(|&k| {
                    let (i, j) = (k % nx, k / nx);
                    p.cells[k] < 0 && i >= 1 && j >= 1 && i + 1 < nx && j + 1 < ny && neighbours(k, nx, ny).any(|m| p.cells[m] == b) && around(k, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == b)
                })
                .collect();
            for k in grow {
                p.cells[k] = b;
            }
        }
    } else {
        let far = (0..n).filter(|&k| dist[k] >= 0).max_by_key(|&k| (dist[k], usize::MAX - k)).unwrap_or(arrive);
        p.down = Some(far);
        p.taken[far] = true;
        let r = p.cells[far] as usize;
        p.rooms[r].raise = 0.0;
    }
    // Raised ledges: rubble slopes up the joins into them.
    for j in &g.joins {
        if j.a < 0 || j.b < 0 || p.rooms[j.a as usize].raise == p.rooms[j.b as usize].raise {
            continue;
        }
        for &k in &j.squares {
            if !p.taken[k] && Some(k) != p.down {
                p.put(item("rubble_slope", k % nx, k / nx, 1, 1));
            }
        }
    }
    // Water by theme.
    let hs = rng.next_u32() as u64;
    let h = |k: usize| noise(hs, k % nx, k / nx, 7.0);
    let mut w = vec![DRY; n];
    let down_room = p.down.map(|k| p.cells[k]);
    let size = |p: &Plan, r: i16| p.cells.iter().filter(|&&c| c == r).count();
    let low = |p: &Plan, r: i16| {
        let sq: Vec<usize> = (0..n).filter(|&k| p.cells[k] == r).collect();
        sq.iter().map(|&k| h(k)).sum::<f64>() / sq.len().max(1) as f64
    };
    // (Chambers water may fill: on the floor, not the arrival's, the boss's nor the way down's.)
    let mut wet: Vec<i16> = g.rooms.iter().copied().filter(|&r| p.rooms[r as usize].raise == 0.0 && r as usize != a && Some(r) != boss && Some(r) != down_room).collect();
    let mut lakes: Vec<i16> = Vec::new();
    let mut islands: Vec<(Vec<usize>, bool)> = Vec::new();
    let mut lay_lake = |p: &Plan, w: &mut Vec<[f32; 2]>, r: i16, island: f64, boss: bool, rng: &mut Pcg32| {
        let isle = rng.next_f64() < island;
        let sq = lake(p, ny, w, r, isle, boss, rng);
        if sq.len() >= 3 {
            islands.push((sq, boss));
        }
    };
    match theme.key {
        "flooded" => {
            // Most of the floor shallow, deep channels along stretches of a noise contour (open-ended: few pockets cut off),
            // dry patches on the high ground.
            for k in 0..n {
                let r = p.cells[k];
                if r < 0 || p.rooms[r as usize].raise > 0.0 || Some(r) == boss {
                    continue;
                }
                let v = h(k);
                w[k] = if v > 0.7 {
                    DRY
                } else if (v - 0.52).abs() < 0.05 && noise(hs ^ 0x51ed, k % nx, k / nx, 9.0) > 0.5 && r as usize != a {
                    DEEP
                } else {
                    SHALLOW
                };
            }
            if let Some(&r) = wet.iter().max_by_key(|&&r| (size(&p, r), -r)) {
                lakes.push(r);
                lay_lake(&p, &mut w, r, 0.6, false, rng);
            }
        }
        "cave" | "fungal" => {
            if rng.next_f64() < 0.35 && !wet.is_empty() {
                // A lake on the low ground: of the bigger chambers, the lowest.
                let mut big = wet.clone();
                big.sort_by_key(|&r| (std::cmp::Reverse(size(&p, r)), r));
                big.truncate(big.len().div_ceil(3));
                let r = *big.iter().min_by(|&&x, &&y| low(&p, x).total_cmp(&low(&p, y)).then(x.cmp(&y))).unwrap();
                lakes.push(r);
                lay_lake(&p, &mut w, r, 0.5, false, rng);
                wet.retain(|&x| x != r);
            }
            wet.sort_by(|&x, &y| low(&p, x).total_cmp(&low(&p, y)).then(x.cmp(&y)));
            for &r in wet.iter().take(1 + rng.below(2) as usize) {
                pool(&p, ny, &mut w, r, &h, true);
            }
        }
        _ => {
            if rng.next_f64() < 0.3 && !wet.is_empty() {
                let r = wet[rng.below(wet.len() as u32) as usize];
                pool(&p, ny, &mut w, r, &h, false);
            }
        }
    }
    // The boss's island (if its chamber grew big enough).
    if let Some(b) = boss
        && boss_isle
        && depths(&p, ny, b).iter().any(|s| s.1 >= 5)
    {
        lay_lake(&p, &mut w, b, 1.0, true, rng);
        lakes.push(b);
    }
    // Islands: a room of their own (the boss's, the lake round it then a chamber of the theme's).
    for (sq, is_boss) in islands {
        let r = p.cells[sq[0]];
        if is_boss {
            let lake_room = p.room(theme.kinds[0], 0.0);
            for k in 0..n {
                if p.cells[k] == r && !sq.contains(&k) {
                    p.cells[k] = lake_room;
                }
            }
        } else {
            let isle = p.room("island", 0.0);
            for &k in &sq {
                p.cells[k] = isle;
            }
        }
    }
    // Cover: rock columns and stalagmites (crystal clusters, ice) standing in the big chambers, off the edges.
    let pillar = match theme.key {
        "crystal" => "crystal",
        "ice" => "ice",
        _ => "rock_column",
    };
    for r in 0..p.rooms.len() as i16 {
        if p.rooms[r as usize].kind == BOSS || lakes.contains(&r) {
            continue;
        }
        let inner: Vec<usize> = depths(&p, ny, r).into_iter().filter(|s| s.1 >= 3).map(|s| s.0).collect();
        let squares = size(&p, r);
        if squares < 50 || inner.is_empty() {
            continue;
        }
        let want = (squares / 45).clamp(1, 4);
        let mut put = 0;
        for _ in 0..want * 6 {
            if put >= want {
                break;
            }
            let k = inner[rng.below(inner.len() as u32) as usize];
            let what = if rng.next_f64() < 0.5 { pillar } else { "stalagmite" };
            if w[k] != DEEP && p.place(item(what, k % nx, k / nx, 1, 1), Some(r), ny) {
                put += 1;
            }
        }
    }
    if w.iter().any(|&x| x != DRY) {
        p.water = w;
    }
    // Sometimes a chasm across the level (not the deepest, not a flooded cave), crossed by rope bridges.
    if !last && theme.key != "flooded" && rng.next_f64() < 0.3 {
        p.chasm = Some((-1, true));
    }
    p
}

/// Finish a natural level from its open squares: keep what the arrival reaches, the boss
/// chamber (last level) or the way down at the far end, chambers by nearest seed (some
/// raised as ledges).
pub(super) fn natural_level(nx: usize, ny: usize, arrive: usize, last: bool, open: Vec<bool>, kinds: &[&'static str], ledge: &'static str, rng: &mut Pcg32) -> Plan {
    let n = nx * ny;
    let mut p = Plan::new(nx, ny, arrive, true);
    let floor = p.room("floor", 0.0);
    for k in 0..n {
        if open[k] {
            p.cells[k] = floor;
        }
    }
    let dist = p.distances(arrive, ny, None);
    for k in 0..n {
        if dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    let far = (0..n).filter(|&k| dist[k] >= 0).max_by_key(|&k| dist[k]).unwrap_or(arrive);
    let max_d = dist[far].max(0);
    // Chambers: nearest of a few seeds by walking distance.
    let cells: Vec<usize> = (0..n).filter(|&k| p.cells[k] >= 0).collect();
    let mut seeds = vec![arrive];
    for _ in 0..(cells.len() / 110 + 3) {
        seeds.push(cells[rng.below(cells.len() as u32) as usize]);
    }
    p.rooms.clear();
    let mut owner = vec![-1i16; n];
    let mut q = std::collections::VecDeque::new();
    for (si, &s) in seeds.iter().enumerate() {
        if owner[s] < 0 {
            let raise = if si == 0 { 0.0 } else { [0.0, 0.0, 0.0, 5.0, 5.0, 10.0][rng.below(6) as usize] };
            let kind = if raise > 0.0 { ledge } else { kinds[rng.below(kinds.len() as u32) as usize] };
            let id = p.room(kind, raise);
            owner[s] = id;
            q.push_back(s);
        }
    }
    while let Some(k) = q.pop_front() {
        for m in neighbours(k, nx, ny) {
            if p.cells[m] >= 0 && owner[m] < 0 {
                owner[m] = owner[k];
                q.push_back(m);
            }
        }
    }
    for k in 0..n {
        if p.cells[k] >= 0 {
            p.cells[k] = owner[k];
        }
    }
    // The arrival stands on the floor (not a ledge).
    let a = p.cells[arrive] as usize;
    p.rooms[a].raise = 0.0;
    if last {
        // The boss chamber: the far end. Squares at least `max_d - reach` steps in, joined to
        // the farthest square: every other square's shortest way in avoids them (a dead end).
        let reach = 12.min(max_d / 3);
        let t = max_d - reach;
        let boss = p.room(BOSS, 0.0);
        let mut q = std::collections::VecDeque::from([far]);
        let mut seen = vec![false; n];
        seen[far] = true;
        let mut chamber = Vec::new();
        while let Some(k) = q.pop_front() {
            chamber.push(k);
            for m in neighbours(k, nx, ny) {
                if !seen[m] && p.cells[m] >= 0 && dist[m] >= t {
                    seen[m] = true;
                    q.push_back(m);
                }
            }
        }
        for &k in &chamber {
            p.cells[k] = boss;
        }
        // Widen it into the rock where that touches nothing else.
        let (fx, fy) = ((far % nx) as isize, (far / nx) as isize);
        for _ in 0..3 {
            for j in fy - 5..=fy + 5 {
                for i in fx - 5..=fx + 5 {
                    if i < 1 || j < 1 || i >= nx as isize - 1 || j >= ny as isize - 1 || (i - fx) * (i - fx) + (j - fy) * (j - fy) > 25 {
                        continue;
                    }
                    let k = j as usize * nx + i as usize;
                    if p.cells[k] < 0 && neighbours(k, nx, ny).all(|m| p.cells[m] < 0 || p.cells[m] == boss) && neighbours(k, nx, ny).any(|m| p.cells[m] == boss) {
                        p.cells[k] = boss;
                    }
                }
            }
        }
    } else {
        p.down = Some(far);
        let r = p.cells[far] as usize;
        p.rooms[r].raise = 0.0;
    }
    p
}
