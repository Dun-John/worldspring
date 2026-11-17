//! Quarters and their labels, wall pieces, shortest paths, shore runs, piers and wall clipping.

use super::*;

/// Named districts, after watabou's DistrictBuilder: seeds at the plaza, the castle, parks,
/// gates, bridges, the riverbank and the docks, plus as many random patches; a random
/// √(patches) of them become districts, which grow over neighbouring patches (gate, bridge
/// and castle districts slowly; never across the wall or the river; docks and parks only
/// over their own kind; not between the waterfront and inland). Leftover patches seed more.
pub(super) fn quarters(s: &Settlement, q: &QuarterInput) -> Vec<(Quarter, Vec<usize>)> {
    use QuarterKind as K;
    let total = q.cells.len();
    let in_city: Vec<bool> = (0..total).map(|i| q.cells[i].pts.len() >= 3 && q.ward[i].is_some_and(|w| w != Ward::Farm)).collect();
    let city: Vec<usize> = (0..total).filter(|&i| in_city[i]).collect();
    if city.is_empty() {
        return Vec::new();
    }
    let mut rng = Pcg32::new(hash2(s.seed, 0xd157, 2), 59);
    let kind_of = |i: usize| match q.ward[i] {
        Some(Ward::Castle) => K::Castle,
        Some(Ward::Park) => K::Park,
        Some(Ward::Docks) if q.landing[i] => K::Docks,
        _ if i < q.n_inner => K::Regular,
        _ => K::Sprawl,
    };
    let at = |p: P| -> Vec<usize> { city.iter().copied().filter(|&i| q.cells[i].pts.iter().any(|v| dist(*v, p) < 2.0)).collect() };

    // Seeds: anchors first (the centre and castle always make it), then random patches.
    let mut fixed: Vec<(Vec<usize>, K)> = Vec::new();
    if let Some(c) = q.castle.filter(|&c| in_city[c]) {
        fixed.push((vec![c], K::Castle));
    }
    let centre = q.plaza.filter(|&p| in_city[p]).unwrap_or_else(|| *city.iter().min_by(|&&a, &&b| len(centroid(&q.cells[a].pts)).total_cmp(&len(centroid(&q.cells[b].pts)))).unwrap());
    fixed.push((vec![centre], K::Center));
    let mut seeds: Vec<(Vec<usize>, K)> = Vec::new();
    for &i in &city {
        if q.ward[i] == Some(Ward::Park) {
            seeds.push((vec![i], K::Park));
        }
    }
    for &g in q.gates {
        let ps = at(g);
        if !ps.is_empty() {
            seeds.push((ps, K::Gate));
        }
    }
    for &b in q.crossings {
        let ps = at(b);
        if !ps.is_empty() {
            seeds.push((ps, K::Bridge));
        }
    }
    let banks: Vec<usize> = city.iter().copied().filter(|&i| q.landing[i] && kind_of(i) != K::Docks && q.edge_w[i].contains(&0.0)).collect();
    for _ in 0..2 {
        if !banks.is_empty() {
            seeds.push((vec![banks[rng.below(banks.len() as u32) as usize]], K::Bank));
        }
    }
    if let Some(&i) = city.iter().find(|&&i| kind_of(i) == K::Docks) {
        seeds.push((vec![i], K::Docks));
    }
    for _ in 0..seeds.len() {
        let i = city[rng.below(city.len() as u32) as usize];
        seeds.push((vec![i], kind_of(i)));
    }
    let target = crate::core::sqrt(city.len() as f64) as usize;
    for k in (1..seeds.len()).rev() {
        seeds.swap(k, rng.below(k as u32 + 1) as usize);
    }
    seeds.truncate(target.saturating_sub(fixed.len()));
    let mut selected = fixed;
    selected.extend(seeds);
    while selected.len() < target {
        let i = city[rng.below(city.len() as u32) as usize];
        selected.push((vec![i], kind_of(i)));
    }
    // One docks district (the others' patches join it or their neighbours).
    let mut seen_docks = false;
    selected.retain(|(_, k)| *k != K::Docks || !std::mem::replace(&mut seen_docks, true));

    let mut owner = vec![usize::MAX; total];
    let mut qs: Vec<(K, Vec<usize>)> = Vec::new();
    for (ps, k) in selected {
        let free: Vec<usize> = ps.iter().copied().filter(|&i| owner[i] == usize::MAX).collect();
        // A multi-patch anchor (gate, bridge) needs all its patches.
        if free.is_empty() || free.len() < ps.len() {
            continue;
        }
        for &i in &free {
            owner[i] = qs.len();
        }
        qs.push((k, free));
    }
    let grow = |id: usize, qs: &mut Vec<(K, Vec<usize>)>, owner: &mut Vec<usize>, rng: &mut Pcg32| -> bool {
        let k = qs[id].0;
        let rate = match k {
            K::Castle | K::Bridge | K::Gate => 0.1,
            K::Bank => 0.5,
            _ => 1.0,
        };
        if rng.next_f64() < 1.0 - rate {
            return true;
        }
        let mut cands: Vec<usize> = Vec::new();
        for &i in &qs[id].1 {
            for (e, &nb) in q.cells[i].labels.iter().enumerate() {
                if nb < 0 || !in_city[nb as usize] || owner[nb as usize] != usize::MAX {
                    continue;
                }
                let j = nb as usize;
                let same = match k {
                    K::Docks => kind_of(j) == K::Docks,
                    K::Park => q.ward[j] == Some(Ward::Park),
                    _ => q.landing[i] == q.landing[j],
                };
                if same && rng.next_f64() < q.edge_w[i][e] {
                    cands.push(j);
                }
            }
        }
        if cands.is_empty() {
            return false;
        }
        let j = cands[rng.below(cands.len() as u32) as usize];
        owner[j] = id;
        qs[id].1.push(j);
        true
    };
    let mut growers: Vec<usize> = (0..qs.len()).collect();
    while !growers.is_empty() && city.iter().any(|&i| owner[i] == usize::MAX) {
        let mut order = growers.clone();
        for k in (1..order.len()).rev() {
            order.swap(k, rng.below(k as u32 + 1) as usize);
        }
        for g in order {
            if !grow(g, &mut qs, &mut owner, &mut rng) {
                growers.retain(|&x| x != g);
            }
        }
    }
    loop {
        let free: Vec<usize> = city.iter().copied().filter(|&i| owner[i] == usize::MAX).collect();
        if free.is_empty() {
            break;
        }
        let i = free[rng.below(free.len() as u32) as usize];
        owner[i] = qs.len();
        qs.push((kind_of(i), vec![i]));
        let id = qs.len() - 1;
        while grow(id, &mut qs, &mut owner, &mut rng) {}
    }

    // Names (watabou's grammar) and label curves.
    let mut namer = Namer::new(hash2(s.seed, 0xd157, 3));
    let culture = s.culture as usize;
    let mut used: std::collections::BTreeSet<String> = Default::default();
    let mut out = Vec::new();
    for (k, members) in qs {
        let patches: Vec<Vec<P>> = members.iter().map(|&i| q.cells[i].pts.clone()).collect();
        let (label, mid) = label_curve(&patches);
        let size = members.len() + rng.below(3) as usize;
        let noun = if size <= 2 {
            "Quarter"
        } else if size < 6 {
            "Ward"
        } else if size < 12 {
            "District"
        } else {
            "Town"
        };
        let dir = [(1.0, 0.0, "East"), (-1.0, 0.0, "West"), (0.0, 1.0, "South"), (0.0, -1.0, "North")]
            .iter()
            .max_by(|a, b| (a.0 * mid[0] + a.1 * mid[1]).total_cmp(&(b.0 * mid[0] + b.1 * mid[1])))
            .unwrap()
            .2;
        let either = |rng: &mut Pcg32, a: &str, b: &str| if rng.next_f64() < 0.5 { a.to_string() } else { b.to_string() };
        let base = match k {
            K::Center => format!("{} {noun}", either(&mut rng, "Market", "Old")),
            K::Castle => format!("{} {noun}", either(&mut rng, "Crown", "Keep")),
            K::Docks => format!("{} {noun}", either(&mut rng, "Harbor", "Wharf")),
            K::Bridge => format!("Bridge {noun}"),
            K::Gate => format!("{dir} Gate"),
            K::Bank => format!("Riverbank {noun}"),
            // (R5 towns, 114: a steppe town's park quarter after its horse park, no garden.)
            K::Park if culture % 5 == 4 => format!("Horse {noun}"),
            K::Park => format!("Garden {noun}"),
            K::Sprawl => format!("Outer {noun}"),
            K::Regular => format!("{} {noun}", namer.word(culture, 1, 2)),
        };
        let mut name = base.clone();
        if used.contains(&name) {
            name = format!("{dir} {base}");
        }
        while used.contains(&name) {
            name = format!("{} {noun}", namer.word(culture, 1, 2));
        }
        used.insert(name.clone());
        out.push((Quarter { name, kind: k, patches, label }, members));
    }
    out
}

/// A label baseline for a district: along the principal axis of its patch centroids
/// (area-weighted), 70% of its extent, bent toward where the patches lie. Also returns the
/// area-weighted centre.
pub(super) fn label_curve(patches: &[Vec<P>]) -> (Vec<P>, P) {
    let cs: Vec<(P, f64)> = patches.iter().map(|p| (centroid(p), area(p).abs().max(1.0))).collect();
    let wsum: f64 = cs.iter().map(|c| c.1).sum();
    let c = cs.iter().fold([0.0, 0.0], |acc, (p, w)| add(acc, mul(*p, w / wsum)));
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in patches.iter().flatten() {
        let d = sub(*p, c);
        sxx += d[0] * d[0];
        syy += d[1] * d[1];
        sxy += d[0] * d[1];
    }
    let th = 0.5 * libm::atan2(2.0 * sxy, sxx - syy);
    let mut u = [libm::cos(th), libm::sin(th)];
    if u[0] < 0.0 {
        u = mul(u, -1.0);
    }
    let nrm = [-u[1], u[0]];
    let (t0, t1) = patches.iter().flatten().fold((f64::MAX, f64::MIN), |(a, b), p| {
        let t = dot(sub(*p, c), u);
        (a.min(t), b.max(t))
    });
    let (tm, span) = (0.5 * (t0 + t1), t1 - t0);
    let h = 0.25 * span;
    let mut pts: Vec<P> = (0..5)
        .map(|k| {
            let t = tm + (k as f64 / 4.0 - 0.5) * 0.7 * span;
            let (mut num, mut den) = (0.0, 1e-9);
            for (p, w) in &cs {
                let d = sub(*p, c);
                let z = (dot(d, u) - t) / h.max(1.0);
                let g = w * libm::exp(-z * z);
                num += g * dot(d, nrm);
                den += g;
            }
            let off = (num / den).clamp(-0.15 * span, 0.15 * span);
            add(c, add(mul(u, t), mul(nrm, off)))
        })
        .collect();
    // Smooth the bend once so the text flows.
    let raw = pts.clone();
    for k in 1..4 {
        pts[k] = lerp(lerp(raw[k - 1], raw[k + 1], 0.5), raw[k], 0.5);
    }
    (pts, c)
}

/// Distance from a polygon to a segment (0 if they touch or the segment is inside).
pub(super) fn poly_seg_dist(poly: &[P], a: P, b: P) -> f64 {
    if contains(poly, a) || contains(poly, b) {
        return 0.0;
    }
    let m = poly.len();
    let mut d = f64::MAX;
    for e in 0..m {
        let (c, q) = (poly[e], poly[(e + 1) % m]);
        let (o1, o2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(q, a)));
        let (o3, o4) = (cross(sub(q, c), sub(a, c)), cross(sub(q, c), sub(b, c)));
        if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
            return 0.0;
        }
        d = d.min(seg_dist(c, a, b)).min(seg_dist(a, c, q)).min(seg_dist(b, c, q));
    }
    d
}

/// A town castle's moat by roll (v63 castles, `moat`): a city's or a metropolis's citadel, the
/// town's own or one set on a ward by hand, about half of them.
pub(super) fn castle_moat(s: &Settlement, inner: bool, age: u8, roll: f64, wide: f64) -> Option<moat::Plan> {
    if s.tier < Tier::City {
        return None;
    }
    let great = s.capital || s.tier == Tier::Metropolis;
    let p = moat::odds(age as f64 / 90.0, s.kind == crate::t0::settle::SettleKind::Fortress, s.capital || inner, great);
    moat::roll(roll, wide, p, great)
}

/// (R6 P8) A docks ward on a sloping shore (its block's `Site::grade` over 0.09): lots narrow
/// enough to stand on the slope (about 7 ft of fall across each), stepped (`District::stepped`).
/// Generated and hand-laid blocks alike.
pub(super) fn docks_on_slope(dp: &mut fill::District, w: Ward, grade: f64) {
    if w == Ward::Docks && grade > 0.09 {
        let across = 7.0 / grade;
        dp.min_sq = dp.min_sq.min((across * across).max(900.0));
        dp.stepped = true;
    }
}

/// The ground a layout is built on, as building lots see it: whether a footprint can stand
/// there, and the level it is built at.
pub(super) trait Ground {
    fn buildable(&self, poly: &[P]) -> bool;
    /// `buildable` for a docks lot on a sloping shore (grade over 0.09): the ground rising up to
    /// 10 ft across it, so it stands on two pads a step apart (`fill::step_pads`).
    fn stepped(&self, poly: &[P]) -> bool {
        self.buildable(poly)
    }
    fn pad(&self, poly: &[P]) -> f32;
    /// The ground's height at a point (ft).
    fn height(&self, p: P) -> f64;
    /// On the water or a river's channel.
    fn wet(&self, p: P) -> bool;
    /// On the water or within `margin` ft of a river's channel.
    fn near_water(&self, p: P, margin: f64) -> bool;
}

impl Ground for Site<'_> {
    fn buildable(&self, poly: &[P]) -> bool {
        Site::buildable(self, poly)
    }
    fn stepped(&self, poly: &[P]) -> bool {
        self.terraced(poly)
    }
    fn pad(&self, poly: &[P]) -> f32 {
        Site::pad(self, poly)
    }
    fn height(&self, p: P) -> f64 {
        Site::height(self, p)
    }
    fn wet(&self, p: P) -> bool {
        Site::wet(self, p)
    }
    fn near_water(&self, p: P, margin: f64) -> bool {
        self.wet_standing(p) || self.near_river(p, margin)
    }
}

/// Split a wall line at its gates: a 22 ft passage through the wall at each gate corner
/// between two gate towers, towers on every other corner, and more towers along long runs.
/// Input: corners with a gate flag; `closed` joins the last corner back to the first.
/// (The spacing where each stretch's towers are given.)
pub(super) const SET: f64 = wallstyle::SPACING;

pub(super) fn wall_pieces(corners: &[(P, bool)], closed: bool, spacing: f64) -> (Vec<Vec<P>>, Vec<P>, Vec<P>) {
    let (pieces, towers, gate_towers, _) = wall_pieces_spaced(corners, closed, None, spacing);
    (pieces, towers, gate_towers)
}

/// `wall_pieces`, with how many towers stand along each stretch between corners given (in
/// order; else from its length, one per `spacing` ft: `WallStyle::spacing`), so a town whose corners
/// moved keeps its towers (and their `t:` ids). Also returns the counts used.
pub(super) fn wall_pieces_spaced(corners: &[(P, bool)], closed: bool, along: Option<&[usize]>, spacing: f64) -> (Vec<Vec<P>>, Vec<P>, Vec<P>, Vec<usize>) {
    let mut used: Vec<usize> = Vec::new();
    // Gate towers (15 ft radius) stand 26 ft either side of the gate: a 22 ft passage.
    const GATE_HALF: f64 = 26.0;
    let mut pts: Vec<(P, bool)> = corners.to_vec();
    if closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    let (mut pieces, mut towers, mut gate_towers) = (Vec::new(), Vec::new(), Vec::new());
    let mut cur: Vec<P> = Vec::new();
    for k in 0..pts.len() {
        let (p, gate) = pts[k];
        if gate {
            let prev = if k > 0 { Some(pts[k - 1].0) } else { None };
            let next = if k + 1 < pts.len() { Some(pts[k + 1].0) } else { None };
            if let Some(q) = prev {
                let e = add(p, mul(sub(q, p), GATE_HALF / dist(p, q).max(1e-9)));
                cur.push(e);
                gate_towers.push(e);
            }
            if cur.len() >= 2 {
                pieces.push(std::mem::take(&mut cur));
            }
            cur.clear();
            if let Some(q) = next {
                let e = add(p, mul(sub(q, p), GATE_HALF / dist(p, q).max(1e-9)));
                cur.push(e);
                gate_towers.push(e);
            }
            continue;
        }
        // Towers along a long run since the last corner.
        if let Some(&last) = cur.last() {
            let d = dist(last, p);
            let extra = along.and_then(|a| a.get(used.len()).copied()).unwrap_or((d / spacing).floor() as usize);
            used.push(extra);
            for j in 1..=extra {
                towers.push(lerp(last, p, j as f64 / (extra + 1) as f64));
            }
        }
        cur.push(p);
        if !(closed && k + 1 == pts.len()) {
            towers.push(p);
        }
    }
    if cur.len() >= 2 {
        pieces.push(cur);
    }
    (pieces, towers, gate_towers, used)
}

/// Two convex polygons overlap (a corner of one inside the other).
pub(super) fn overlaps(a: &[P], b: &[P]) -> bool {
    a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p))
}

/// `overlaps`, or their edges cross (two strips laid across each other, no corner in the other).
pub(super) fn overlaps_or_crosses(a: &[P], b: &[P]) -> bool {
    let cross = |p: P, q: P, r: P| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let edges = |v: &[P]| (0..v.len()).map(|i| (v[i], v[(i + 1) % v.len()])).collect::<Vec<_>>();
    overlaps(a, b)
        || edges(a).iter().any(|&(p, q)| {
            edges(b).iter().any(|&(r, t)| {
                let (d1, d2, d3, d4) = (cross(p, q, r), cross(p, q, t), cross(r, t, p), cross(r, t, q));
                d1 * d2 < 0.0 && d3 * d4 < 0.0
            })
        })
}

pub(super) fn rotate(v: P, ang: f64) -> P {
    let (c, s) = (libm::cos(ang), libm::sin(ang));
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

/// Every edge of a convex polygon as a street line (outward normal, offset).
pub(super) fn edge_lines(poly: &[P]) -> Vec<(P, f64)> {
    let m = poly.len();
    let ccw = area(poly) > 0.0;
    (0..m)
        .filter_map(|e| {
            let (a, b) = (poly[e], poly[(e + 1) % m]);
            let ev = sub(b, a);
            let el = len(ev);
            (el > 1e-9).then(|| {
                let nrm = if ccw { [ev[1] / el, -ev[0] / el] } else { [-ev[1] / el, ev[0] / el] };
                (nrm, dot(a, nrm))
            })
        })
        .collect()
}

/// Dijkstra over the street graph from `start` to any of `targets`.
/// Street graph: quantized vertex → (neighbour, cost).
pub(super) type Graph = FastMap<(i64, i64), Vec<((i64, i64), f64)>>;

pub(super) fn shortest(adj: &Graph, start: (i64, i64), targets: &std::collections::BTreeSet<(i64, i64)>) -> Option<Vec<(i64, i64)>> {
    #[derive(PartialEq)]
    struct N(f64, (i64, i64));
    impl Eq for N {}
    impl Ord for N {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
        }
    }
    impl PartialOrd for N {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    if !adj.contains_key(&start) {
        return None;
    }
    let mut g: FastMap<(i64, i64), f64> = FastMap::default();
    let mut came: FastMap<(i64, i64), (i64, i64)> = FastMap::default();
    let mut heap = BinaryHeap::new();
    g.insert(start, 0.0);
    heap.push(N(0.0, start));
    while let Some(N(d, k)) = heap.pop() {
        if targets.contains(&k) {
            let mut path = vec![k];
            let mut cur = k;
            while let Some(&p) = came.get(&cur) {
                path.push(p);
                cur = p;
            }
            path.reverse();
            return Some(path);
        }
        if d > *g.get(&k).unwrap_or(&f64::MAX) {
            continue;
        }
        let mut nbs = adj[&k].clone();
        nbs.sort_by_key(|a| a.0);
        for (nb, w) in nbs {
            let nd = d + w;
            if nd < *g.get(&nb).unwrap_or(&f64::MAX) {
                g.insert(nb, nd);
                came.insert(nb, k);
                heap.push(N(nd, nb));
            }
        }
    }
    None
}

/// Trim every corner: a chord between points `t` ft along both edges from the corner.
pub(super) fn chamfer(poly: &[P], t: f64) -> Vec<P> {
    let m = poly.len();
    let mut out = poly.to_vec();
    for i in 0..m {
        let (p, a, b) = (poly[i], poly[(i + m - 1) % m], poly[(i + 1) % m]);
        let (la, lb) = (dist(p, a), dist(p, b));
        if la < 1e-6 || lb < 1e-6 {
            continue;
        }
        let tt = t.min(0.3 * la.min(lb));
        let (u, v) = (mul(sub(a, p), 1.0 / la), mul(sub(b, p), 1.0 / lb));
        let bis = add(u, v);
        let bl = len(bis);
        if bl < 1e-6 {
            continue;
        }
        let n = mul(bis, -1.0 / bl);
        out = clip_plain(&out, n, dot(add(p, mul(u, tt)), n));
        if out.len() < 3 {
            return out;
        }
    }
    out
}

/// Rectangle centred at `c`, long side `w` along `dir`, depth `d`.
pub(crate) fn rect(c: P, dir: P, w: f64, d: f64) -> Vec<P> {
    let dl = len(dir).max(1e-9);
    let u = mul(dir, 1.0 / dl);
    let v = [-u[1], u[0]];
    let (hu, hv) = (mul(u, 0.5 * w), mul(v, 0.5 * d));
    vec![sub(sub(c, hu), hv), sub(add(c, hu), hv), add(add(c, hu), hv), add(sub(c, hu), hv)]
}

// ---------------------------------------------------------------------------------------
// Villages.

pub(super) fn village(site: &Site, s: &Settlement, r: f64, _road_ends: &[(P, u8)], rng: &mut Pcg32, l: &mut Layout) {
    village::village(site, s, r, rng, l);
}

/// The shore a village faces: rays from the centre find the water's edge; the longest run
/// of consecutive hits (in angle) is the shoreline seen from the village. Returned as a lane
/// 14 ft inland of it, smoothed and resampled every 25 ft. `None` if there's no water near.
pub(super) fn shore_run(site: &Site, reach: f64) -> Option<Vec<P>> {
    const RAYS: usize = 72;
    let mut hits: Vec<Option<P>> = vec![None; RAYS];
    for (a, hit) in hits.iter_mut().enumerate() {
        let ang = std::f64::consts::TAU * a as f64 / RAYS as f64;
        let u = [libm::cos(ang), libm::sin(ang)];
        let mut t = 20.0;
        while t < reach {
            let q = mul(u, t);
            if site.wet(q) {
                // Refine the edge to a few feet, then step back inland.
                let (mut lo, mut hi) = (t - 20.0, t);
                for _ in 0..5 {
                    let m = 0.5 * (lo + hi);
                    if site.wet(mul(u, m)) { hi = m } else { lo = m }
                }
                *hit = Some(mul(u, (lo - 14.0).max(10.0)));
                break;
            }
            t += 20.0;
        }
    }
    // Longest circular run of hits.
    let mut best: (usize, usize) = (0, 0);
    for start in 0..RAYS {
        if hits[start].is_none() || hits[(start + RAYS - 1) % RAYS].is_some() && hits.iter().any(|h| h.is_none()) {
            continue;
        }
        let mut n = 0;
        while n < RAYS && hits[(start + n) % RAYS].is_some() {
            n += 1;
        }
        if n > best.1 {
            best = (start, n);
        }
    }
    if best.1 < 5 {
        return None;
    }
    let raw: Vec<P> = (0..best.1).map(|k| hits[(best.0 + k) % RAYS].unwrap()).collect();
    // Smooth (Chaikin) and resample.
    let mut pts = raw;
    for _ in 0..2 {
        let mut next = vec![pts[0]];
        for w in pts.windows(2) {
            next.push(lerp(w[0], w[1], 0.25));
            next.push(lerp(w[0], w[1], 0.75));
        }
        next.push(*pts.last().unwrap());
        pts = next;
    }
    let total: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    if total < 120.0 {
        return None;
    }
    let n = (total / 25.0).ceil() as usize;
    Some((0..=n).map(|k| point_along(&pts, total * k as f64 / n as f64).0).collect())
}

/// Piers along a village's wharf: every 150–260 ft (about one in three left out), square to the
/// shore, 25–45 ft into the water (a river pier reaches at most a third of the way across). Built
/// only where the water starts right at the wharf and the whole pier stands over water.
pub(super) fn piers_along(site: &Site, wharf: &[P], rng: &mut Pcg32, l: &mut Layout) {
    let islets = site.islets();
    let total: f64 = wharf.windows(2).map(|w| dist(w[0], w[1])).sum();
    let mut at = rng.range(40.0, 120.0);
    while at < total - 20.0 {
        if rng.next_f64() < 0.33 {
            at += rng.range(150.0, 260.0);
            continue;
        }
        let (p, u) = point_along(wharf, at);
        let mut out = [-u[1], u[0]];
        if !site.wet(add(p, mul(out, 30.0))) {
            out = mul(out, -1.0);
        }
        let mut length = rng.range(25.0, 45.0);
        if let Some((_, _, hw)) = site.nearest_river(p)
            && site.near_river(add(p, mul(out, 30.0)), 0.0)
        {
            length = length.min(0.66 * hw);
        }
        // Anchor on the real water's edge along the pier line: the first wet point out from
        // the wharf (backing up if the wharf point is already wet), refined to a foot.
        let wet_at = |t: f64| site.wet(add(p, mul(out, t)));
        let mut t = 0.0;
        while wet_at(t) && t > -40.0 {
            t -= 4.0;
        }
        while !wet_at(t) && t < 80.0 {
            t += 4.0;
        }
        if wet_at(t) && !wet_at(t - 4.0) {
            let (mut lo, mut hi) = (t - 4.0, t);
            for _ in 0..3 {
                let m = 0.5 * (lo + hi);
                if wet_at(m) { hi = m } else { lo = m }
            }
            // The pier starts 6 ft on land and runs out over the water.
            let (base, end) = (lo - 6.0, hi + length);
            let over_water = (0..=8).all(|k| wet_at(hi + 2.0 + (length - 2.0) * k as f64 / 8.0));
            let pier = rect(add(p, mul(out, 0.5 * (base + end))), out, end - base, 8.0);
            // Never on a road (or its bridge) or another deck.
            let clear = !site.deck_on_road(&pier) && !l.bridges.iter().chain(&l.piers).any(|d| overlaps_or_crosses(d, &pier)) && islets.iter().all(|i| poly_gap(i, &pier) >= 20.0);
            if length >= 12.0 && over_water && !wet_at(base) && clear {
                l.piers.push(pier);
            }
        }
        at += rng.range(150.0, 260.0);
    }
}

/// Where the straight line a→b crosses the river centreline (a and b on opposite banks).
pub(super) fn river_crossing(site: &Site, a: P, b: P) -> P {
    let side = |p: P| site.nearest_river(p).map_or(0.0, |(c, t, _)| cross(t, sub(p, c)));
    let sa = side(a);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let m = 0.5 * (lo + hi);
        if side(lerp(a, b, m)) * sa > 0.0 { lo = m } else { hi = m }
    }
    lerp(a, b, 0.5 * (lo + hi))
}

pub(super) fn polar_pt(d: f64, a: f64) -> P {
    [d * libm::cos(a), d * libm::sin(a)]
}

/// The street from a road's end `e` to its gate `g` (local coords): a cubic that carries on
/// the way the road was heading and arrives square to the town edge, each only where that
/// leads towards the other end (else it heads straight there: no loops or doubling back).
pub(super) fn approach_curve(e: P, g: P, heading: Option<P>) -> Vec<P> {
    let span = dist(e, g);
    let into = mul(sub(g, e), 1.0 / span.max(1e-9));
    let start = heading.filter(|h| dot(*h, into) > 0.3).unwrap_or(into);
    let inward = mul(g, -1.0 / len(g).max(1e-9));
    let end = if dot(inward, into) > 0.3 { inward } else { into };
    let (c1, c2) = (add(e, mul(start, 0.4 * span)), sub(g, mul(end, 0.35 * span)));
    let n = (span / 40.0).ceil().clamp(2.0, 24.0) as usize;
    (0..=n)
        .map(|j| {
            let t = j as f64 / n as f64;
            let u = 1.0 - t;
            add(add(mul(e, u * u * u), mul(c1, 3.0 * u * u * t)), add(mul(c2, 3.0 * u * t * t), mul(g, t * t * t)))
        })
        .collect()
}

/// A bank tower no nearer another town tower than this (ft, centre to centre).
const BANK_CLEAR: f64 = 22.0;

/// Walls end at the bank: wall runs are cut where they cross a river channel or run out into
/// the sea or a lake (the water is the defence there), each cut end gets a tower on the bank, and
/// towers standing in the water go. The walls of castles set by hand (`hand`, places in `l.walls`)
/// put their bank towers after every other, as their own towers are (no `t:` id moves).
pub(super) fn walls_off_river(site: &Site, l: &mut Layout, hand: &[usize]) {
    let river = !site.river.is_empty();
    clip_walls(l, |p| (river && site.near_river(p, 3.0)) || site.wet_standing(p), |t| (river && site.near_river(t, 2.0)) || site.wet_standing(t), hand);
}

/// Break walls where they would stand in water (`wet`, every 5 ft; a tower on each bank), and
/// drop the towers standing in it (`tower_wet`). The bank towers of the walls at `hand` go with
/// the towers of castles set by hand (`l.extra_towers`).
pub(super) fn clip_walls(l: &mut Layout, wet: impl Fn(P) -> bool, tower_wet: impl Fn(P) -> bool, hand: &[usize]) {
    let mut walls: Vec<Vec<P>> = Vec::new();
    for (wi, w) in std::mem::take(&mut l.walls).into_iter().enumerate() {
        let by_hand = hand.contains(&wi);
        // (In a castle, none beside a tower that stays, nor on one of its ranges: they would stand
        // on each other. A town's castle's range there goes instead, but its keep: no curtain
        // ending in the open by the water.)
        let fill = !by_hand && l.castle_patch.is_some();
        let crowded = |l: &Layout, t: P, ranges: bool| {
            l.castles.iter().any(|c| dist(t, c.0) <= c.1 + 30.0)
                && (l.towers.iter().chain(&l.gate_towers).copied().chain(l.extra_towers.iter().map(|e| e.0)).any(|o| dist(o, t) < 34.0 && !tower_wet(o))
                    || l.buildings.iter().any(|b| b.ward == Ward::Castle && (ranges || b.keep.is_some()) && poly_seg_dist(&b.poly, t, t) < 13.0))
        };
        let bank = |l: &mut Layout, t: P| {
            if crowded(l, t, !fill) {
                return;
            }
            // (Nor beside a town tower that stays, as where a run's corner or another cut end stands
            // on the bank: one tower there.)
            if !by_hand && l.towers.iter().chain(&l.gate_towers).any(|o| dist(*o, t) < BANK_CLEAR && !tower_wet(*o)) {
                return;
            }
            if crowded(l, t, true) {
                l.buildings.retain(|b| !(b.ward == Ward::Castle && b.keep.is_none() && poly_seg_dist(&b.poly, t, t) < 13.0));
            }
            if by_hand { l.extra_towers.push((t, false)) } else { l.towers.push(t) }
        };
        let mut cur: Vec<P> = Vec::new();
        for (k, seg) in w.windows(2).enumerate() {
            let (a, b) = (seg[0], seg[1]);
            if k == 0 && !wet(a) {
                cur.push(a);
            }
            let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
            let mut prev = a;
            for j in 1..=n {
                let p = lerp(a, b, j as f64 / n as f64);
                let (was, is) = (wet(prev), wet(p));
                if was != is {
                    // The bank between the two samples, by bisection.
                    let (mut lo, mut hi) = (prev, p);
                    for _ in 0..6 {
                        let m = lerp(lo, hi, 0.5);
                        if wet(m) == was { lo = m } else { hi = m }
                    }
                    let edge = if was { hi } else { lo };
                    if is {
                        cur.push(edge);
                        if cur.len() >= 2 {
                            bank(l, edge);
                            walls.push(std::mem::take(&mut cur));
                        }
                        cur.clear();
                    } else {
                        bank(l, edge);
                        cur.push(edge);
                    }
                }
                if j == n && !is {
                    cur.push(b);
                }
                prev = p;
            }
        }
        if cur.len() >= 2 {
            walls.push(cur);
        }
    }
    walls.retain(|w| w.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>() > 10.0);
    l.walls = walls;
    l.towers.retain(|t| !tower_wet(*t));
    l.gate_towers.retain(|t| !tower_wet(*t));
    l.extra_towers.retain(|t| !tower_wet(t.0));
    // (A square tower gone with the rest.)
    let (towers, extra) = (&l.towers, &l.extra_towers);
    l.square_towers.retain(|s| towers.iter().chain(extra.iter().map(|e| &e.0)).any(|t| (t[0] - s.0[0]).abs() + (t[1] - s.0[1]).abs() < 1e-6));
    l.tower_forms.retain(|s| towers.iter().chain(extra.iter().map(|e| &e.0)).any(|t| (t[0] - s.0[0]).abs() + (t[1] - s.0[1]).abs() < 1e-6));
}

/// Point and unit direction `at` ft along a polyline.
pub(super) fn point_along(pts: &[P], at: f64) -> (P, P) {
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let d = dist(w[0], w[1]);
        if acc + d >= at && d > 1e-9 {
            let u = mul(sub(w[1], w[0]), 1.0 / d);
            return (lerp(w[0], w[1], (at - acc) / d), u);
        }
        acc += d;
    }
    let n = pts.len();
    let d = dist(pts[n - 2], pts[n - 1]).max(1e-9);
    (pts[n - 1], mul(sub(pts[n - 1], pts[n - 2]), 1.0 / d))
}

// ---------------------------------------------------------------------------------------
// Functions, names, floors.
