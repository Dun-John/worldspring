//! The economy pass: what each town earns, and its size from that.
//!
//! Each town and city lives on its hinterland (the farmland nearest it by travel, found by one
//! search from all of them at once), its industry (ore, timber, fishing) and its gateway (a
//! harbour, mouth, head of navigation or crossing, where goods change hands), and on trade: each
//! pair exchanges goods in proportion to their wealth, falling with the distance between them by
//! the trial roads (or by sea between ports), and every place earns from what passes through it.
//! Ranked by wealth, they are re-tiered with each tier's count kept (a place moves a tier at
//! most), and a place's population follows its rank within its tier. The traffic sets the roads'
//! classes as built (`roads::build`): the busiest stretches become king's roads.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::biome::Biome;
use super::flood::neighbors;
use super::hydro::NO_LAKE;
use super::roads::{RoadClass, Routed};
use super::sealanes;
use super::settle::{self, Settlement, Tier};
use crate::core::hash::FastMap;
use crate::core::rng::{hash3, unit};

/// How far (miles of travel) a town's hinterland reaches.
const HINTERLAND_MI: f64 = 15.0;
/// What a market town (`Reason::Market`) earns on top of its hinterland, as a share of a mean one's.
const MARKET_TRADE: f64 = 0.2;
/// The gateway (0-1) a town needs to grow into a city.
const CITY_GATE: f64 = 0.3;
/// Trade between two places falls off past this distance (miles of travel).
const TRADE_MI: f64 = 60.0;
/// What a mile by sea (along its routed lane, `sealanes`) costs against one by road, and loading
/// and unloading (miles by road): a short way along the coast goes by road. (A link that could
/// not be routed goes `SEA_DETOUR` times farther than straight.)
const SEA_COST: f64 = 0.5;
const SEA_DETOUR: f64 = 1.3;
const SEA_LOAD: f64 = 25.0;
/// Ports linked by sea to this many of their nearest on the same sea.
const SEA_LINKS: usize = 3;

/// What each town and up earned (index into the settlements, the tier it was placed at, the
/// parts of its wealth: hinterland food relative to the mean, industry and gateway 0-1, trade
/// relative to the mean; then the wealth they add up to).
#[derive(Clone, Debug)]
pub struct Place {
    pub index: usize,
    pub was: Tier,
    pub food: f32,
    pub industry: f32,
    pub gateway: f32,
    pub trade: f32,
    pub wealth: f32,
}

/// Goods carried per step of the trial roads (cells, smaller first), and the flows that set a
/// stretch's class: at `kings` and up a king's road; under `quiet[0]` a king's road drops to a
/// road, under `quiet[1]` a road to a track.
#[derive(Clone, Debug, Default)]
pub struct Traffic {
    pub flow: FastMap<(u32, u32), f32>,
    pub kings: f32,
    pub quiet: [f32; 2],
}

impl Traffic {
    /// The class a stretch of `class` carrying `flow` is built as.
    pub fn class(&self, class: RoadClass, flow: f32) -> RoadClass {
        if self.kings <= 0.0 || class == RoadClass::Track {
            class
        } else if flow >= self.kings {
            RoadClass::KingsRoad
        } else if class == RoadClass::KingsRoad && flow < self.quiet[0] {
            RoadClass::Road
        } else if class == RoadClass::Road && flow < self.quiet[1] {
            RoadClass::Track
        } else {
            class
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub places: Vec<Place>,
    pub traffic: Traffic,
    /// The sea links between ports, routed, with the goods over each (`sealanes::lanes`).
    pub sea_links: Vec<sealanes::Link>,
}

#[derive(PartialEq)]
struct Node(f64, u32);
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Re-tier the towns and up of `settlements` by what they earn; `routes` are the trial roads
/// (none with generated roads off: no trade then, so a drawn road moves nothing). Pins and the
/// capital keep their tier (and a pin its population) but count in the ranking.
pub fn run(inp: &settle::Inputs, settlements: &mut [Settlement], routes: Option<&[Routed]>) -> Report {
    let (w, h, cell) = (inp.w, inp.h, inp.cell_ft);
    let n = w * h;
    // (A town added beside the world's own takes no part: adding one changes no other; nor does an
    // isolated realm's.)
    let added = |s: &Settlement| s.pin.is_some_and(|st| inp.pins.iter().any(|p| p.stroke == st as usize && p.added));
    let towns: Vec<usize> = (0..settlements.len()).filter(|&i| settlements[i].tier >= Tier::Town && !added(&settlements[i]) && !settlements[i].isolated).collect();
    let mut report = Report::default();
    if towns.is_empty() || inp.ground.get().is_none() {
        return report;
    }
    let g = inp.ground.get().expect("placed");
    let mi = cell / 5280.0;

    // --- Hinterland: from every town at once, each land cell goes to the nearest by travel
    // (uphill and down cost more), its fertility counting less the farther it is.
    let mut food = vec![0.0f64; towns.len()];
    {
        let dry = |k: usize| inp.land[k] && inp.hydro.lake_of[k] == NO_LAKE;
        let mut best = vec![f64::INFINITY; n];
        let mut owner = vec![u32::MAX; n];
        let mut heap = BinaryHeap::new();
        for (t, &i) in towns.iter().enumerate() {
            let k = settlements[i].cell;
            best[k] = 0.0;
            owner[k] = t as u32;
            heap.push(Node(0.0, k as u32));
        }
        while let Some(Node(c, k)) = heap.pop() {
            let k = k as usize;
            if c > best[k] {
                continue;
            }
            let o = owner[k];
            if dry(k) {
                let f = settle::fertility(Biome::from_u8((inp.biome[k] & 0xff) as u8));
                food[o as usize] += f * (1.0 - c / HINTERLAND_MI);
            }
            for (nb, d) in neighbors(w, h, k) {
                if !dry(nb) {
                    continue;
                }
                let run = d * mi;
                let grade = (inp.height[nb] - inp.height[k]).abs() / (d * cell);
                let nc = c + run * (1.0 + 10.0 * grade.min(0.5));
                if nc < HINTERLAND_MI && (nc < best[nb] || (nc == best[nb] && o < owner[nb])) {
                    best[nb] = nc;
                    owner[nb] = o;
                    heap.push(Node(nc, nb as u32));
                }
            }
        }
    }
    let mean_food = (food.iter().sum::<f64>() / towns.len() as f64).max(1e-9);
    // Industry and gateway: the strongest within a cell of the town.
    let around = |k: usize, v: &[f32]| neighbors(w, h, k).map(|(nb, _)| v[nb]).fold(v[k], f32::max) as f64;
    let base: Vec<f64> = towns
        .iter()
        .enumerate()
        .map(|(t, &i)| {
            let k = settlements[i].cell;
            // (A market town far from the cities trades with the farms round it.)
            let market = if settlements[i].reason == settle::Reason::Market { MARKET_TRADE } else { 0.0 };
            food[t] / mean_food + 0.7 * around(k, &g.industry) + 1.0 * around(k, &g.bulk) + market
        })
        .collect();

    // --- Trade over the trial roads (and the sea between ports).
    let mut trade = vec![0.0f64; towns.len()];
    if let Some(routes) = routes {
        // The graph: every cell the routes pass, joined along them (a step by its length, a king's
        // road a little faster).
        let mut steps: FastMap<(u32, u32), u8> = Default::default();
        for (class, path, _) in routes {
            for p in path.windows(2) {
                let key = (p[0].min(p[1]), p[0].max(p[1]));
                let e = steps.entry(key).or_insert(*class as u8);
                *e = (*e).min(*class as u8);
            }
        }
        let mut keys: Vec<((u32, u32), u8)> = steps.into_iter().collect();
        keys.sort_unstable();
        let mut cells: Vec<u32> = keys.iter().flat_map(|&((a, b), _)| [a, b]).chain(towns.iter().map(|&i| settlements[i].cell as u32)).collect();
        cells.sort_unstable();
        cells.dedup();
        let id = |c: u32| cells.binary_search(&c).expect("a graph cell") as u32;
        // (Each edge: where it goes, its cost, and `ROAD` or the sea link it is.)
        const ROAD: u32 = u32::MAX;
        let mut adj: Vec<Vec<(u32, f64, u32)>> = vec![Vec::new(); cells.len()];
        let at = |c: u32| [(c as usize % w) as f64, (c as usize / w) as f64];
        let len = |a: u32, b: u32| {
            let (p, q) = (at(a), at(b));
            crate::core::sqrt((p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1])) * mi
        };
        for &((a, b), class) in &keys {
            let c = len(a, b) * [0.8, 1.0, 1.2][class as usize];
            let (ia, ib) = (id(a), id(b));
            adj[ia as usize].push((ib, c, ROAD));
            adj[ib as usize].push((ia, c, ROAD));
        }
        // Ports on one sea trade by water with their nearest on it.
        {
            let mut sea = vec![u32::MAX; n];
            let mut next = 0u32;
            for k0 in 0..n {
                if inp.land[k0] || sea[k0] != u32::MAX {
                    continue;
                }
                let mut stack = vec![k0];
                sea[k0] = next;
                while let Some(k) = stack.pop() {
                    for (nb, _) in neighbors(w, h, k) {
                        if !inp.land[nb] && sea[nb] == u32::MAX {
                            sea[nb] = next;
                            stack.push(nb);
                        }
                    }
                }
                next += 1;
            }
            let mut size = vec![0usize; next as usize];
            for &s in sea.iter().filter(|&&s| s != u32::MAX) {
                size[s as usize] += 1;
            }
            let ports: Vec<(u32, u32, usize)> = towns
                .iter()
                .filter(|&&i| settlements[i].coastal)
                .filter_map(|&i| {
                    let k = settlements[i].cell;
                    let (x, y) = ((k % w) as i64, (k / w) as i64);
                    let r = sealanes::PORT_REACH;
                    // (The largest sea within reach: not a pocket of water cut off beside it.)
                    let water = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (x + dx, y + dy))).filter(|&(i, j)| i >= 0 && j >= 0 && i < w as i64 && j < h as i64).map(|(i, j)| sea[j as usize * w + i as usize]).filter(|&s| s != u32::MAX).min_by_key(|&s| (std::cmp::Reverse(size[s as usize]), s))?;
                    Some((k as u32, water, i))
                })
                .collect();
            // Each pair once, routed shortest first (a lane sees those before it).
            let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
            for (pa, &(a, sa, _)) in ports.iter().enumerate() {
                let mut near: Vec<(f64, u32, usize)> = ports.iter().enumerate().filter(|&(_, &(b, sb, _))| sb == sa && b != a).map(|(pb, &(b, _, _))| (len(a, b), b, pb)).collect();
                near.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
                for &(d, _, pb) in near.iter().take(SEA_LINKS) {
                    pairs.push((d, pa.min(pb), pa.max(pb)));
                }
            }
            pairs.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
            pairs.dedup_by(|x, y| x.1 == y.1 && x.2 == y.2);
            let mut router = (!pairs.is_empty()).then(|| sealanes::Router::new(w, h, cell, inp.land));
            for &(d, pa, pb) in &pairs {
                let ((a, sa, ia), (b, _, ib)) = (ports[pa], ports[pb]);
                let routed = router.as_mut().and_then(|r| {
                    let (from, into) = (r.water(a as usize, &sea, sa), r.water(b as usize, &sea, sa));
                    r.route(a as usize, &from, b as usize, &into)
                });
                let (cells, way) = routed.unwrap_or((Vec::new(), d * SEA_DETOUR));
                let c = way * SEA_COST + SEA_LOAD;
                let l = report.sea_links.len() as u32;
                adj[id(a) as usize].push((id(b), c, l));
                adj[id(b) as usize].push((id(a), c, l));
                report.sea_links.push(sealanes::Link { ports: [ia as u32, ib as u32], cells, mi: way, flow: 0.0 });
            }
        }
        for a in adj.iter_mut() {
            a.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)));
        }
        // Each pair's goods along the cheapest way between them: per source, its shortest-path
        // tree, each target's demand gathered up the tree to the source.
        let node: Vec<u32> = towns.iter().map(|&i| id(settlements[i].cell as u32)).collect();
        let m = cells.len();
        let mut through = vec![0.0f64; m];
        let mut own = vec![0.0f64; towns.len()];
        let mut cost = vec![f64::INFINITY; m];
        let mut parent = vec![u32::MAX; m];
        let mut road_up = vec![ROAD; m];
        let mut acc = vec![0.0f64; m];
        let mut order: Vec<u32> = Vec::with_capacity(m);
        let mut step_flow: FastMap<(u32, u32), f64> = Default::default();
        for (s, &src) in node.iter().enumerate() {
            cost.iter_mut().for_each(|c| *c = f64::INFINITY);
            parent.iter_mut().for_each(|p| *p = u32::MAX);
            order.clear();
            let mut heap = BinaryHeap::new();
            cost[src as usize] = 0.0;
            heap.push(Node(0.0, src));
            while let Some(Node(c, v)) = heap.pop() {
                if c > cost[v as usize] {
                    continue;
                }
                order.push(v);
                for &(u, e, road) in &adj[v as usize] {
                    let nc = c + e;
                    if nc < cost[u as usize] {
                        cost[u as usize] = nc;
                        parent[u as usize] = v;
                        road_up[u as usize] = road;
                        heap.push(Node(nc, u));
                    }
                }
            }
            for &v in &order {
                acc[v as usize] = 0.0;
            }
            for (t, &dst) in node.iter().enumerate().skip(s + 1) {
                let d = cost[dst as usize];
                if !d.is_finite() || dst == src {
                    continue;
                }
                let r = d / TRADE_MI;
                let f = base[s] * base[t] / (1.0 + r * r);
                acc[dst as usize] += f;
                own[s] += f;
                own[t] += f;
            }
            for &v in order.iter().rev() {
                let p = parent[v as usize];
                let a = acc[v as usize];
                if p == u32::MAX || a <= 0.0 {
                    continue;
                }
                acc[p as usize] += a;
                if road_up[v as usize] == ROAD {
                    let (cv, cp) = (cells[v as usize], cells[p as usize]);
                    *step_flow.entry((cv.min(cp), cv.max(cp))).or_default() += a;
                } else {
                    report.sea_links[road_up[v as usize] as usize].flow += a;
                }
            }
            // What passes a node (not ending there): the flow on from it to the places beyond.
            for &v in order.iter() {
                let p = parent[v as usize];
                if p != u32::MAX && p != src {
                    through[p as usize] += acc[v as usize];
                }
            }
        }
        for t in 0..towns.len() {
            trade[t] = own[t] + through[node[t] as usize];
        }
        let mean = trade.iter().sum::<f64>() / towns.len() as f64;
        if mean > 0.0 {
            trade.iter_mut().for_each(|v| *v /= mean);
        }
        // The flows that set the classes: a road as busy as the median step of the king's roads
        // becomes one; a king's road or road under a twentieth of its class's median drops.
        let mut by: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
        for &((a, b), class) in &keys {
            if class < 2 {
                by[class as usize].push(step_flow.get(&(a, b)).copied().unwrap_or(0.0));
            }
        }
        let median = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.total_cmp(b));
            v.get(v.len() / 2).copied().unwrap_or(0.0)
        };
        let (mk, mr) = (median(&mut by[0]), median(&mut by[1]));
        report.traffic = Traffic { flow: step_flow.into_iter().map(|(k, v)| (k, v as f32)).collect(), kings: mk as f32, quiet: [(0.05 * mk) as f32, (0.05 * mr) as f32] };
    }

    // --- Re-tier: ranked by wealth, each tier keeps its count; a place moves a tier at most, pins
    // and the capital stay.
    let wealth: Vec<f64> = (0..towns.len()).map(|t| base[t] + trade[t]).collect();
    let fixed = |i: usize| settlements[i].pin.is_some() || settlements[i].capital;
    let mut rank: Vec<usize> = (0..towns.len()).collect();
    rank.sort_by(|&a, &b| wealth[b].total_cmp(&wealth[a]).then(a.cmp(&b)));
    let was: Vec<Tier> = towns.iter().map(|&i| settlements[i].tier).collect();
    let mut now: Vec<Option<Tier>> = towns.iter().map(|&i| fixed(i).then_some(settlements[i].tier)).collect();
    // (A town grows into a city only by a gateway: one inland, however busy its market, stays a town.)
    let gate: Vec<f64> = towns.iter().map(|&i| around(settlements[i].cell, &g.bulk)).collect();
    for tier in [Tier::Metropolis, Tier::City, Tier::Town] {
        let count = was.iter().filter(|&&t| t == tier).count();
        let mut taken = now.iter().filter(|&&t| t == Some(tier)).count();
        // Those that can go no lower first (a tier above this one, not kept there), then the richest
        // within a tier of it.
        let forced: Vec<usize> = rank.iter().copied().filter(|&t| now[t].is_none() && was[t] > tier).collect();
        for t in forced {
            now[t] = Some(tier);
            taken += 1;
        }
        for &t in &rank {
            if taken >= count {
                break;
            }
            if now[t].is_none() && (was[t] as i32 - tier as i32).abs() <= 1 && (tier <= Tier::Town || was[t] >= tier || gate[t] >= CITY_GATE) {
                now[t] = Some(tier);
                taken += 1;
            }
        }
    }
    // Population from the place's rank within its tier.
    for tier in [Tier::Metropolis, Tier::City, Tier::Town] {
        let members: Vec<usize> = rank.iter().copied().filter(|&t| now[t] == Some(tier)).collect();
        let (lo, hi) = settle::pop_range(tier);
        for (r, &t) in members.iter().enumerate() {
            let i = towns[t];
            if settlements[i].pin.is_some() {
                continue;
            }
            let pos = 1.0 - (r as f64 + 0.5) / members.len() as f64;
            let jitter = unit(hash3(settlements[i].seed, 0x706f_70, 1, 0));
            let u = (0.8 * pos + 0.2 * jitter).clamp(0.0, 1.0);
            let tier_now = now[t].unwrap_or(was[t]);
            if tier_now != settlements[i].tier {
                settle::retier(inp, &mut settlements[i], tier_now);
            }
            settlements[i].population = (lo * libm::pow(hi / lo, u)) as u32;
        }
    }
    report.places = towns
        .iter()
        .enumerate()
        .map(|(t, &i)| {
            let k = settlements[i].cell;
            Place { index: i, was: was[t], food: (food[t] / mean_food) as f32, industry: around(k, &g.industry) as f32, gateway: around(k, &g.bulk) as f32, trade: trade[t] as f32, wealth: wealth[t] as f32 }
        })
        .collect();
    report
}
