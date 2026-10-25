//! A city's sewers: the ground under the paved streets of one section, a sewage channel down the wider runs.

use crate::interior::{Item, SQUARE_FT};
use crate::t0::T0;
use crate::town::{Layout, geom::P};
use crate::under::*;

/// The sewers of a section: the ground under the paved streets, joined where runs meet only
/// outside the section by service passages; a sewage channel down the wider runs, an outfall
/// where a tunnel reaches water; the drain shaft down at the far end from the way in (the
/// grate, else the run nearest the section's middle). None if no street runs here.
pub(crate) fn sewer(t0: &T0, l: &Layout, origin: P, nx: usize, ny: usize, grate: Option<usize>, last: bool) -> Option<Plan> {
    let n = nx * ny;
    let mut p = Plan::new(nx, ny, 0, false);
    let tunnel = p.room("sewer tunnel", 0.0);
    let centre = |k: usize| [origin[0] + ((k % nx) as f64 + 0.5) * SQUARE_FT, origin[1] + ((k / nx) as f64 + 0.5) * SQUARE_FT];
    let mut is_wet: Vec<Option<bool>> = vec![None; n];
    let r = SEWER_REACH_FT;
    for (a, b, _) in street_segments(l) {
        let (x0, y0) = (a[0].min(b[0]) - r - origin[0], a[1].min(b[1]) - r - origin[1]);
        let (x1, y1) = (a[0].max(b[0]) + r - origin[0], a[1].max(b[1]) + r - origin[1]);
        let span = nx as f64 * SQUARE_FT;
        if x1 < 0.0 || y1 < 0.0 || x0 > span || y0 > span {
            continue;
        }
        let g = |p: P| [((p[0] - origin[0]) / SQUARE_FT) as f32, ((p[1] - origin[1]) / SQUARE_FT) as f32];
        let (ga, gb) = (g(a), g(b));
        p.paths.push([ga[0], ga[1], gb[0], gb[1], (r / SQUARE_FT) as f32]);
        let i0 = (x0 / SQUARE_FT).floor().max(0.0) as usize;
        let j0 = (y0 / SQUARE_FT).floor().max(0.0) as usize;
        let i1 = ((x1 / SQUARE_FT).ceil() as usize).min(nx - 1);
        let j1 = ((y1 / SQUARE_FT).ceil() as usize).min(ny - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let k = j * nx + i;
                if p.cells[k] >= 0 || town::geom::seg_dist(centre(k), a, b) > r {
                    continue;
                }
                if !*is_wet[k].get_or_insert_with(|| wet(t0, centre(k))) {
                    p.cells[k] = tunnel;
                }
            }
        }
    }
    // Where cellars and keep tunnels meet this section: their squares are sewer.
    let s_ft = SEWER_SECTION_FT;
    let here = [origin[0] / s_ft, origin[1] / s_ft];
    let mut joins: Vec<(usize, Link, &'static str, &'static str)> = Vec::new();
    let square_of = |q: P| {
        let (i, j) = (((q[0] - origin[0]) / SQUARE_FT).floor(), ((q[1] - origin[1]) / SQUARE_FT).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < nx && (j as usize) < ny).then(|| j as usize * nx + i as usize)
    };
    for b in &l.buildings {
        let (c, bi) = (town::geom::centroid(&b.poly), b.id);
        // (A keep's escape tunnel may reach a street up to 400 ft off: `keep_escape`.)
        let near = if crate::interior::has_deep_dungeon(b) { 0.8 + 400.0 / s_ft } else { 0.8 };
        if (c[0] / s_ft - here[0] - 0.5).abs() > near || (c[1] / s_ft - here[1] - 0.5).abs() > near {
            continue;
        }
        if let Some(q) = sewer_link_of(t0, l, b)
            && let Some(k) = square_of(q)
        {
            joins.push((k, Link { x: (k % nx) as u16, y: (k / nx) as u16, to: format!("b:{}:{bi}", l.index) }, "ladder", "ladder up into a cellar"));
        }
        if let Some(q) = keep_escape(t0, l, b)
            && let Some(k) = square_of(q)
        {
            joins.push((k, Link { x: (k % nx) as u16, y: (k / nx) as u16, to: format!("k:{}:{bi}", l.index) }, "tunnel", "tunnel to the keep's dungeons"));
        }
    }
    for (k, _, _, _) in &joins {
        p.cells[*k] = tunnel;
    }
    // Every street grate over this section is a way up (the first is the way in).
    let grates: Vec<usize> = l.entrances.iter().filter(|e| e.kind == UnderKind::Sewer && sewer_section(e.at) == origin).filter_map(|e| square_of(e.at)).collect();
    for &k in &grates {
        p.cells[k] = tunnel;
    }
    let arrive = match grate {
        Some(g) => {
            p.cells[g] = tunnel;
            g
        }
        None => {
            let mid = |k: usize| (k % nx).abs_diff(nx / 2).pow(2) + (k / nx).abs_diff(ny / 2).pow(2);
            (0..n).filter(|&k| p.cells[k] == tunnel).min_by_key(|&k| mid(k))?
        }
    };
    p.up = arrive;
    // Runs that meet only outside the section: each its own room for a moment, dug to the
    // grate's run by a service passage, then one sewer again.
    let mut comp = vec![-1i32; n];
    let mut comps: Vec<Vec<usize>> = Vec::new();
    for s in 0..n {
        if p.cells[s] != tunnel || comp[s] >= 0 {
            continue;
        }
        let id = comps.len() as i32;
        let mut stack = vec![s];
        comp[s] = id;
        let mut members = Vec::new();
        while let Some(k) = stack.pop() {
            members.push(k);
            for m in neighbours(k, nx, ny) {
                if p.cells[m] == tunnel && comp[m] < 0 {
                    comp[m] = id;
                    stack.push(m);
                }
            }
        }
        comps.push(members);
    }
    if comps.len() > 1 {
        let main = comp[arrive] as usize;
        let mut ids = vec![tunnel; comps.len()];
        for (c, members) in comps.iter().enumerate() {
            if c != main {
                ids[c] = p.room("sewer tunnel", 0.0);
                for &k in members {
                    p.cells[k] = ids[c];
                }
            }
        }
        let passage = p.room("service passage", 0.0);
        // Each run joins the grate's network as soon as it is dug to it (a later run may
        // reach the network only through it); rounds until no more join.
        let mut left: Vec<usize> = (0..comps.len()).filter(|&c| c != main).collect();
        loop {
            let before = left.len();
            left.retain(|&c| {
                let id = ids[c];
                let doors = p.doors.len();
                dig(&mut p, nx, ny, id, tunnel, passage, false);
                if p.doors.len() == doors {
                    return true;
                }
                for &k in &comps[c] {
                    p.cells[k] = tunnel;
                }
                false
            });
            if left.is_empty() || left.len() == before {
                break;
            }
        }
        // The doors dug between a run and its passage stay; any between two runs go.
        let doors: Vec<(usize, usize)> = p.doors.iter().copied().filter(|&(a, b)| p.cells[a] != p.cells[b]).collect();
        p.doors = doors.into_iter().collect();
    }
    let dist = p.distances(arrive, ny, None);
    for k in 0..n {
        if dist[k] < 0 {
            p.cells[k] = -1;
        }
    }
    // The drain shaft down: the farthest run from the grate, clear of the section's edge.
    if !last {
        let inner = |k: usize| (3..nx - 3).contains(&(k % nx)) && (3..ny - 3).contains(&(k / nx));
        // (Never on a grate or where a cellar or keep joins.)
        let kept = |k: usize| grates.contains(&k) || joins.iter().any(|j| j.0 == k);
        let far = |ok: &dyn Fn(usize) -> bool| (0..n).filter(|&k| dist[k] > 0 && p.cells[k] == tunnel && !kept(k) && ok(k)).max_by_key(|&k| dist[k]);
        // (Never on the section's outermost squares: the level below keeps a ring of rock.)
        let margin1 = |k: usize| (1..nx - 1).contains(&(k % nx)) && (1..ny - 1).contains(&(k / nx));
        p.down = far(&inner).or_else(|| far(&margin1));
        // A scrap of tunnel too small for a shaft: no sewer section here.
        p.down?;
    }
    // Away from the walls: floor on all four sides.
    let all_floor = |p: &Plan, k: usize| {
        let (i, j) = (k % nx, k / nx);
        i > 0 && j > 0 && i + 1 < nx && j + 1 < ny && neighbours(k, nx, ny).all(|m| p.cells[m] >= 0)
    };
    let keep = |p: &Plan, k: usize| k == p.up || Some(k) == p.down;
    // The other grates: ladders up to the street.
    let up = p.up;
    for &k in grates.iter().filter(|&&k| k != up) {
        if dist[k] >= 0 && !p.taken[k] && Some(k) != p.down {
            p.put(Item::new("exit", "ladder up to a street grate", k % nx, k / nx, 1, 1, 0, false, 0.0, None));
        }
    }
    // The ladders and tunnels where the cellars and keeps join (one per square).
    // (Two cellars facing across a street can meet at one square: the second ladder takes the
    // nearest free square of tunnel.)
    for (k, link, kind, name) in joins {
        if dist[k] < 0 {
            continue;
        }
        let mut seen = vec![false; n];
        let mut q = std::collections::VecDeque::from([k]);
        seen[k] = true;
        let mut spot = None;
        while let Some(m) = q.pop_front() {
            if !p.taken[m] && m != p.up && Some(m) != p.down {
                spot = Some(m);
                break;
            }
            for o in neighbours(m, nx, ny) {
                if !seen[o] && p.cells[o] >= 0 {
                    seen[o] = true;
                    q.push_back(o);
                }
            }
        }
        if let Some(m) = spot {
            p.put(Item::new(kind, name, m % nx, m / nx, 1, 1, 0, false, 0.0, None));
            p.links.push(Link { x: (m % nx) as u16, y: (m / nx) as u16, ..link });
        }
    }
    // Sewage down the middle of the wider runs; walkways along the walls.
    for k in 0..n {
        if p.cells[k] == tunnel && !keep(&p, k) && all_floor(&p, k) {
            p.put(Item::new("sewage", "sewage channel", k % nx, k / nx, 1, 1, 0, false, 0.0, Some("difficult terrain; swallowed: DC 11 Con save or poisoned for 1 hour")));
        }
    }
    // An outfall where a run reaches water.
    let mut outfalls = 0;
    for k in 0..n {
        if outfalls >= 2 || p.cells[k] != tunnel || p.taken[k] || keep(&p, k) {
            continue;
        }
        if neighbours(k, nx, ny).any(|m| p.cells[m] < 0 && *is_wet[m].get_or_insert_with(|| wet(t0, centre(m)))) {
            p.put(Item::new("outfall", "outfall grate", k % nx, k / nx, 1, 1, 0, false, 0.0, Some("strong current: DC 12 Str (Athletics) or swept 20 ft toward the water")));
            outfalls += 1;
        }
    }
    // Where runs leave the section (one mark per opening).
    for k in 0..n {
        let (i, j) = (k % nx, k / nx);
        let edge = i == 0 || j == 0 || i == nx - 1 || j == ny - 1;
        if !edge || p.cells[k] != tunnel || p.taken[k] || keep(&p, k) {
            continue;
        }
        let prev = if i == 0 || i == nx - 1 { (j > 0).then(|| k - nx) } else { (i > 0).then(|| k - 1) };
        if prev.is_some_and(|q| p.cells[q] >= 0) {
            continue;
        }
        p.put(Item::new("continues", "the sewer runs on (double-click to follow)", i, j, 1, 1, 0, false, 0.0, None));
    }
    Some(p)
}
