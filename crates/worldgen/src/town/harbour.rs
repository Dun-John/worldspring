//! R5 M20: a port's harbour works, laid out last (`build_town`, world ft).
//!
//! - A big port (a city or metropolis with at least `BIG_PORT_PIERS` piers on a sea or lake) gets a
//!   breakwater (`Mole`): from the shore past one end of its docks, straight out along the piers' way
//!   (else curving round their heads), `MOLE_LEN_FT` long, `MOLE_WIDTH_FT` wide, in water at most `MOLE_DEPTH_FT` deep, clear
//!   of islets, piers, bridges, network roads, sea lanes and ferries. It is made ground (raised
//!   through `ground::Shaper::with_extra`, never read by created sites), with a round head and a
//!   lighthouse (`lighthouse`, 3-4 storeys) on it, appended after every other building.
//! - Boats moored along the piers (`Afloat`): in a big port one at every second pier, and one cog at
//!   the longest; elsewhere a rowboat at 1 in 3 piers; a buoy off a breakwater's head.
//! - A big port always has a harbour master's office: the docks building nearest the piers' roots
//!   that fits, when none has it.
//! Bollards and quay cranes are the battlemap's (`battlemap::dock_works`).

use super::*;

/// The fewest piers on a sea or lake that make a big port.
pub const BIG_PORT_PIERS: usize = 4;
/// A breakwater's width, its length (ft), the deepest water it is built in.
pub const MOLE_WIDTH_FT: f64 = 24.0;
pub const MOLE_LEN_FT: (f64, f64) = (200.0, 600.0);
pub const MOLE_DEPTH_FT: f64 = 40.0;
/// The radius of a breakwater's round head (ft), the lighthouse standing on it.
pub const MOLE_HEAD_FT: f64 = 18.0;
/// The steps of the fill from the bank up to a breakwater's root.
const RAMP_STEPS: usize = 4;
/// How long each of those steps is (ft).
const RAMP_STEP_FT: f64 = 3.5;

/// A big port's breakwater (world ft): its line from the bank out to its head, its width, and the
/// level of its top.
#[derive(Clone, Debug, PartialEq)]
pub struct Mole {
    pub pts: Vec<P>,
    pub width: f64,
    pub level: f32,
}

impl Mole {
    /// Its made ground: each stretch a rectangle, each joint a round, the head a wider round.
    pub fn polys(&self) -> Vec<Vec<P>> {
        let mut out: Vec<Vec<P>> = self
            .pts
            .windows(2)
            .map(|s| rect(lerp(s[0], s[1], 0.5), sub(s[1], s[0]), dist(s[0], s[1]), self.width))
            .collect();
        for &p in &self.pts[1..self.pts.len().saturating_sub(1)] {
            out.push(circle(p, 0.5 * self.width, 12));
        }
        if let Some(&tip) = self.pts.last() {
            out.push(circle(tip, MOLE_HEAD_FT, 20));
        }
        out
    }

    /// The fill up to its root from the bank behind it: four steps down 1.2 ft each, 3.5 ft long
    /// (only ever raising, so a bank higher than a step keeps its own ground).
    pub fn ramp(&self) -> Vec<(Vec<P>, f32)> {
        let (a, b) = (self.pts[0], self.pts[1]);
        let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
        (1..=RAMP_STEPS)
            .map(|k| (rect(sub(a, mul(u, RAMP_STEP_FT * (k as f64 - 0.5))), u, RAMP_STEP_FT + 0.2, self.width), self.level - 1.2 * k as f32))
            .collect()
    }

    /// Its plan (`payload`, made ground): one outline round its line and head (no joints showing),
    /// and its ramp as two slabs, the steps two by two.
    pub fn plan(&self) -> Vec<Vec<P>> {
        let n = self.pts.len();
        if n < 2 {
            return self.polys();
        }
        let half = 0.5 * self.width;
        let unit_of = |k: usize| mul(sub(self.pts[k + 1], self.pts[k]), 1.0 / dist(self.pts[k], self.pts[k + 1]).max(1e-9));
        let normal = |u: P| [-u[1], u[0]];
        // (Each side mitred at the joints; the last stretch stopped where the head's rim takes over.)
        let side = |sign: f64| -> Vec<P> {
            let mut out = vec![add(self.pts[0], mul(normal(unit_of(0)), sign * half))];
            for k in 1..n - 1 {
                let (n0, n1) = (normal(unit_of(k - 1)), normal(unit_of(k)));
                let m = add(n0, n1);
                let m = mul(m, 1.0 / len(m).max(1e-9));
                out.push(add(self.pts[k], mul(m, sign * half / dot(m, n1).max(0.5))));
            }
            out
        };
        let rim = &self.edges(MOLE_HEAD_FT)[1];
        let mut outline = side(1.0);
        outline.extend_from_slice(rim);
        let mut right = side(-1.0);
        right.reverse();
        outline.extend(right);
        let mut out = vec![outline];
        let r = self.ramp();
        for pair in r.chunks(2) {
            let (near, far) = (&pair[0].0, &pair[pair.len() - 1].0);
            out.push(vec![far[0], near[1], near[2], far[3]]);
        }
        out
    }

    /// Its made ground as the ground shaper takes it, the ramp to its root with it.
    pub fn raised(&self) -> Vec<crate::ground::Raised> {
        let look = crate::ground::Look::Made;
        let mut out: Vec<crate::ground::Raised> = self
            .polys()
            .into_iter()
            .map(|pts| crate::ground::Raised {
                pts,
                level: self.level,
                band: 0.0,
                look,
            })
            .collect();
        out.extend(self.ramp().into_iter().map(|(pts, level)| crate::ground::Raised { pts, level, band: 0.0, look }));
        out
    }

    /// Its bounds (world ft: x0, y0, x1, y1), the head and width with them.
    pub fn bbox(&self) -> [f64; 4] {
        let r = MOLE_HEAD_FT.max(0.5 * self.width) + 1.0;
        let b = self.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
            [b[0].min(p[0] - r), b[1].min(p[1] - r), b[2].max(p[0] + r), b[3].max(p[1] + r)]
        });
        self.ramp()
            .iter()
            .flat_map(|r| r.0.iter())
            .fold(b, |b, p| [b[0].min(p[0] - 1.0), b[1].min(p[1] - 1.0), b[2].max(p[0] + 1.0), b[3].max(p[1] + 1.0)])
    }

    /// Its armour's lines: each long side and the head's rim (world ft polylines, a to b along the
    /// line on the left side, then back on the right, the head between).
    pub fn edges(&self, head: f64) -> Vec<Vec<P>> {
        let n = self.pts.len();
        if n < 2 {
            return Vec::new();
        }
        let half = 0.5 * self.width;
        let side = |sign: f64| -> Vec<P> {
            let mut out = Vec::new();
            for k in 0..n - 1 {
                let (a, b) = (self.pts[k], self.pts[k + 1]);
                let u = mul(sub(b, a), 1.0 / dist(a, b).max(1e-9));
                let v = mul([-u[1], u[0]], sign * half);
                out.push(add(a, v));
                // (Stopped short of the head, where its rim takes over.)
                let end = if k + 2 == n {
                    dist(a, b) - crate::core::sqrt((head * head - half * half).max(0.0))
                } else {
                    dist(a, b)
                };
                out.push(add(add(a, mul(u, end.max(0.0))), v));
            }
            out
        };
        let (tip, last) = (self.pts[n - 1], self.pts[n - 2]);
        let u = mul(sub(tip, last), 1.0 / dist(last, tip).max(1e-9));
        let a0 = libm::atan2(u[1], u[0]);
        let open = libm::asin((half / head).min(1.0));
        let rim: Vec<P> = (0..=12)
            .map(|k| {
                let a = a0 + std::f64::consts::PI - open - (std::f64::consts::TAU - 2.0 * open) * k as f64 / 12.0;
                [tip[0] + head * libm::cos(a), tip[1] + head * libm::sin(a)]
            })
            .collect();
        vec![side(1.0), rim, side(-1.0)]
    }
}

/// What floats in a harbour (`Layout::moored`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Afloat {
    Rowboat,
    FishingBoat,
    /// A moored cog (`battlemap::ShapeKind::Ship`).
    Cog,
    Buoy,
}

/// A boat moored by a pier (or a buoy): its middle (world ft), the way its bow points (radians),
/// what it is, its length (ft; a cog's, else its kind's own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moored {
    pub at: P,
    pub rot: f32,
    pub kind: Afloat,
    pub length: f32,
}

impl Afloat {
    /// Length and beam (ft) at scale 1 (the sprites': `render/atlas.ts`).
    pub fn size(self) -> (f64, f64) {
        match self {
            Afloat::Rowboat => (13.0, 5.0),
            Afloat::FishingBoat => (22.0, 8.0),
            Afloat::Cog => (70.0, 22.0),
            Afloat::Buoy => (4.0, 4.0),
        }
    }
}

/// Why a big port has no breakwater (`Layout::port`).
pub const NO_SHORE: &str = "no shore past the docks' ends";
pub const TOO_DEEP: &str = "water deeper than 40 ft or dry ground in the way";

/// A pier's root on the bank and its head (`rect`'s corners 0 and 3 at its root).
pub fn pier_ends(p: &[P]) -> Option<(P, P)> {
    (p.len() == 4).then(|| (lerp(p[0], p[3], 0.5), lerp(p[1], p[2], 0.5)))
}

/// The harbour works of settlement `index` (see the module). `generated`: an edited town's
/// layout as generated, whose breakwater, lighthouse and boats it keeps (ids and all) where they
/// still stand clear of its piers and bridges.
pub(super) fn harbour(site: &Site, s: &Settlement, index: usize, l: &mut Layout, generated: Option<&Layout>) {
    let c = site.center;
    let loc = |p: P| sub(p, c);
    let standing: Vec<(P, P)> = l.piers.iter().filter_map(|p| pier_ends(p)).filter(|&(_, tip)| site.wet_standing(loc(tip))).collect();
    let big = s.tier >= Tier::City && standing.len() >= BIG_PORT_PIERS;
    if let Some(g) = generated {
        l.port = g.port;
        l.moles = g.moles.clone();
        for h in g.buildings.iter().filter(|b| is_lighthouse(b)) {
            let at = l.buildings.partition_point(|b| b.id < h.id);
            if l.buildings.get(at).is_none_or(|b| b.id != h.id) && l.buildings.iter().all(|b| poly_gap(&b.poly, &h.poly) > 0.5) {
                l.buildings.insert(at, h.clone());
            }
        }
        let decks: Vec<Vec<P>> = l.piers.iter().chain(&l.bridges).cloned().collect();
        l.moored = g.moored.iter().filter(|m| decks.iter().all(|d| poly_gap(d, &hull(m)) >= 1.0)).copied().collect();
    } else {
        if big {
            l.port = Some(match breakwater(site, s, l, &standing) {
                Ok(m) => {
                    l.moles.push(m);
                    ""
                }
                Err(why) => why,
            });
            lighthouses(index, l);
        }
        moor(site, s, big, l);
    }
    if big {
        harbour_master(s, l, &standing);
    }
    nets(s, l);
}

/// Whether `b` is a breakwater's lighthouse (`catalog` "lighthouse").
pub fn is_lighthouse(b: &Building) -> bool {
    b.func.is_some_and(|f| CATALOG[f as usize].key == "lighthouse")
}

/// A boat's hull outline (world ft): a rectangle its length and beam.
pub fn hull(m: &Moored) -> Vec<P> {
    let (len, beam) = m.kind.size();
    let len = if m.kind == Afloat::Cog { m.length as f64 } else { len };
    rect(m.at, [libm::cos(m.rot as f64), libm::sin(m.rot as f64)], len, beam)
}

/// A big port's breakwater, or why it has none.
fn breakwater(site: &Site, s: &Settlement, l: &Layout, all: &[(P, P)]) -> Result<Mole, &'static str> {
    let c = site.center;
    let loc = |p: P| sub(p, c);
    let way = |&(r, t): &(P, P)| mul(sub(t, r), 1.0 / dist(r, t).max(1e-9));
    // The piers it shelters: the most that point within 45 degrees of one of them.
    let lead = (0..all.len())
        .max_by_key(|&i| (all.iter().filter(|p| dot(way(p), way(&all[i])) > 0.7).count(), usize::MAX - i))
        .unwrap_or(0);
    let piers: Vec<(P, P)> = all.iter().copied().filter(|p| dot(way(p), way(&all[lead])) > 0.7).collect();
    if piers.len() < 2 {
        return Err(NO_SHORE);
    }
    let n = piers.len() as f64;
    let d = piers.iter().fold([0.0, 0.0], |a, p| add(a, way(p)));
    let d = mul(d, 1.0 / len(d).max(1e-9));
    let f = [-d[1], d[0]];
    let o = piers.iter().fold([0.0, 0.0], |a, &(r, _)| add(a, mul(r, 1.0 / n)));
    let (s0, s1) = piers
        .iter()
        .fold((f64::MAX, f64::MIN), |a, &(r, _)| (a.0.min(dot(sub(r, o), f)), a.1.max(dot(sub(r, o), f))));
    let reach = piers.iter().map(|&(_, t)| dot(sub(t, o), d)).fold(f64::MIN, f64::max);
    let first = unit(hash2(0x3b1e_7a77, s.seed as i64, 0)) < 0.5;
    let mut why = NO_SHORE;
    let tries = [(1.0, 50.0), (1.0, 90.0), (1.0, 30.0), (1.0, 140.0)];
    // (How far it gets: past finding a bank, past the water's depth, past what is in the way.)
    let mut stage = 0;
    let mut note = |st: usize, w: &'static str, why: &mut &'static str| {
        if st >= stage {
            stage = st;
            *why = w;
        }
    };
    // Straight out past the piers' heads, along their way (as the harbour's piers run), or
    // curving a little round them, or more: each shape tried at every place along the shore first.
    for (share, extra) in [(0.0, 60.0), (0.3, 90.0), (0.6, 50.0)] {
        for (sign, past) in tries
            .iter()
            .map(|&(k, p)| (if first { k } else { -k }, p))
            .chain(tries.iter().map(|&(k, p)| (if first { -k } else { k }, p)))
        {
            let end = if sign > 0.0 { s1 + past } else { s0 - past };
            let q = add(o, mul(f, end));
            // (Along the way of the pier at that end of the docks, the one beside it.)
            let edge = piers.iter().max_by(|x, y| (sign * dot(sub(x.0, o), f)).total_cmp(&(sign * dot(sub(y.0, o), f)))).copied().unwrap_or(piers[0]);
            let d = if dot(way(&edge), d) > 0.8 { way(&edge) } else { d };
            let f = [-d[1], d[0]];
            // The bank: where the ground out along the piers' way first meets the water, 6 ft back
            // onto land.
            let wet = |t: f64| site.wet_standing(loc(add(q, mul(d, t))));
            let Some(t) = (0..150).map(|k| -300.0 + 4.0 * k as f64).find(|&t| wet(t) && !wet(t - 4.0) && wet(t + 8.0)) else {
                continue;
            };
            let root = add(q, mul(d, t - 10.0));
            let straight = share == 0.0;
            let mut out = (reach - dot(sub(root, o), d) + extra).max(80.0);
            let mut across = share * (s1 - s0) + 60.0 * share / 0.6;
            let mut line: Vec<P> = Vec::new();
            for _ in 0..6 {
                let p2 = add(add(root, mul(d, out)), mul(f, -sign * across));
                let p1 = if straight { lerp(root, p2, 0.5) } else { add(root, mul(d, 1.15 * out)) };
                let curve: Vec<P> = (0..=24)
                    .map(|k| {
                        let t = k as f64 / 24.0;
                        add(add(mul(root, (1.0 - t) * (1.0 - t)), mul(p1, 2.0 * t * (1.0 - t))), mul(p2, t * t))
                    })
                    .collect();
                let total: f64 = curve.windows(2).map(|w| dist(w[0], w[1])).sum();
                if total < MOLE_LEN_FT.0 {
                    if straight {
                        out += MOLE_LEN_FT.0 - total + 10.0;
                    } else {
                        across += MOLE_LEN_FT.0 - total + 10.0;
                    }
                    continue;
                }
                line = trim(&curve, MOLE_LEN_FT.1);
                break;
            }
            if line.len() < 2 {
                continue;
            }
            // (Every 8 points or so: a stretch about 50-75 ft.)
            let line = resample(&line, 9);
            note(1, TOO_DEEP, &mut why);
            // In open water from 30 ft out to the head and round it, nowhere deeper than allowed.
            let total: f64 = line.windows(2).map(|w| dist(w[0], w[1])).sum();
            let mut at = 30.0;
            let mut ok = true;
            while at <= total + MOLE_HEAD_FT && ok {
                let p = along(&line, at);
                let depth = site.t0.sample_water(p[0], p[1]) as f64 - site.height(loc(p));
                ok = site.wet_standing(loc(p)) && !site.near_river(loc(p), 20.0) && depth <= MOLE_DEPTH_FT;
                at += 10.0;
            }
            if !ok {
                continue;
            }
            let level = crate::wharf::level(
                &crate::world::Wharf {
                    kind: crate::world::WharfKind::Breakwater,
                    pts: line.clone(),
                    width: MOLE_WIDTH_FT,
                    material: crate::world::CrossingMaterial::Stone,
                },
                site.t0,
            );
            let mole = Mole {
                pts: line,
                width: MOLE_WIDTH_FT,
                level,
            };
            let mut polys = mole.polys();
            polys.extend(mole.ramp().into_iter().map(|r| r.0));
            let gap = |o: &[P]| polys.iter().map(|m| poly_gap(m, o)).fold(f64::MAX, f64::min);
            let bb = mole.bbox();
            let w = if !l.piers.iter().all(|p| gap(p) >= 15.0) {
                "piers in the way"
            } else if !l.bridges.iter().all(|p| gap(p) >= 20.0) {
                "bridges in the way"
            } else if !l.buildings.iter().all(|b| gap(&b.poly) >= 3.0) {
                "buildings in the way"
            } else if !l.walls.iter().all(|w| w.windows(2).all(|e| gap(e) >= 10.0)) || !l.bastions.iter().all(|b| gap(b) >= 6.0) || !l.towers.iter().all(|t| gap(&[*t]) >= 15.0) {
                "the town wall in the way"
            } else if !crate::t0::islets::near(site.t0, [bb[0] - 30.0, bb[1] - 30.0, bb[2] + 30.0, bb[3] + 30.0]).all(|i| gap(&i.pts) >= 20.0) {
                "islets in the way"
            } else if !clear_of_ways(site, &mole) {
                "roads, sea lanes or ferries in the way"
            } else if !mole.pts.iter().all(|&p| dist(p, c) + MOLE_HEAD_FT + 10.0 < super::reach(s)) {
                "out of the town's reach"
            } else {
                return Ok(mole);
            };
            note(2, w, &mut why);
        }
    }
    Err(why)
}

/// Whether a breakwater keeps 20 ft off network roads, 40 ft off sea lanes, 60 ft off ferries.
fn clear_of_ways(site: &Site, m: &Mole) -> bool {
    let total: f64 = m.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let samples: Vec<P> = (0..=(total / 8.0) as usize).map(|k| along(&m.pts, k as f64 * 8.0)).chain(m.pts.last().copied()).collect();
    let bb = m.bbox();
    let near = |a: P, b: P, r: f64| a[0].max(b[0]) + r >= bb[0] && a[0].min(b[0]) - r <= bb[2] && a[1].max(b[1]) + r >= bb[1] && a[1].min(b[1]) - r <= bb[3];
    let half = 0.5 * m.width;
    let roads = samples.iter().all(|&p| !site.near_network_road(sub(p, site.center), 20.0 + half));
    let lanes = site.t0.sea_lanes.iter().all(|ln| {
        ln.pts
            .windows(2)
            .filter(|w| near(w[0], w[1], 60.0))
            .all(|w| samples.iter().all(|&p| seg_dist(p, w[0], w[1]) >= 40.0 + half))
    });
    let ferries = site
        .t0
        .ferries
        .iter()
        .filter(|fy| near(fy.a, fy.b, 80.0))
        .all(|fy| samples.iter().all(|&p| seg_dist(p, fy.a, fy.b) >= 60.0 + half));
    roads && lanes && ferries
}

/// The point `at` ft along a polyline (its last past the end).
fn along(line: &[P], mut at: f64) -> P {
    for w in line.windows(2) {
        let l = dist(w[0], w[1]);
        if at <= l {
            return lerp(w[0], w[1], at / l.max(1e-9));
        }
        at -= l;
    }
    *line.last().unwrap()
}

/// A polyline cut at `max` ft along it.
fn trim(line: &[P], max: f64) -> Vec<P> {
    let mut out = vec![line[0]];
    let mut left = max;
    for w in line.windows(2) {
        let l = dist(w[0], w[1]);
        if l >= left {
            out.push(lerp(w[0], w[1], left / l.max(1e-9)));
            return out;
        }
        out.push(w[1]);
        left -= l;
    }
    out
}

/// A polyline as `n` points evenly along it.
fn resample(line: &[P], n: usize) -> Vec<P> {
    let total: f64 = line.windows(2).map(|w| dist(w[0], w[1])).sum();
    (0..n).map(|k| along(line, total * k as f64 / (n - 1) as f64)).collect()
}

/// A round lighthouse on each breakwater's head: 18-20 ft across, 3-4 storeys, its ground floor
/// at the breakwater's level. Ids follow every other building's.
fn lighthouses(index: usize, l: &mut Layout) {
    let Some(func) = catalog::index_of("lighthouse") else { return };
    let mut id = l.buildings.iter().map(|b| b.id + 1).max().unwrap_or(0);
    for m in l.moles.clone() {
        let tip = *m.pts.last().unwrap();
        let h = hash3(0x11e7_4a05, crate::core::round(tip[0]) as i64, crate::core::round(tip[1]) as i64, index as i64);
        let rad = 9.0 + unit(h);
        let a0 = unit(h ^ 1) * std::f64::consts::TAU;
        let mut poly: Vec<P> = (0..rural::MILL_SIDES)
            .map(|k| {
                let a = a0 + std::f64::consts::TAU * k as f64 / rural::MILL_SIDES as f64;
                [tip[0] + rad * libm::cos(a), tip[1] + rad * libm::sin(a)]
            })
            .collect();
        if area(&poly) < 0.0 {
            poly.reverse();
        }
        let floors = 3 + (unit(h ^ 3) < 0.5) as u8;
        l.buildings.push(Building {
            poly,
            ward: Ward::Docks,
            func: Some(func as u16),
            residential: 1,
            name: None,
            floors,
            pad_ft: m.level,
            structure: Structure::Roofed,
            roof: None,
            tint: None,
            material: None,
            shape: None,
            shade: None,
            id,
            parts: Vec::new(),
            keep: None,
            deck: None,
        });
        id += 1;
    }
}

/// Boats along the piers: in a big port one at every second pier (a rowboat or a fishing boat),
/// one cog at the longest pier it fits by, and a buoy off each breakwater's head; elsewhere a
/// rowboat at 1 in 3 piers. Each on open water, clear of every deck, islet and breakwater.
fn moor(site: &Site, s: &Settlement, big: bool, l: &mut Layout) {
    let c = site.center;
    let mut taken: Vec<Vec<P>> = l.piers.iter().chain(&l.bridges).cloned().collect();
    taken.extend(l.moles.iter().flat_map(|m| m.polys()));
    let bb = l.piers.iter().flatten().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    });
    taken.extend(crate::t0::islets::near(site.t0, [bb[0] - 100.0, bb[1] - 100.0, bb[2] + 100.0, bb[3] + 100.0]).map(|i| i.pts.clone()));
    let afloat = |m: &Moored, taken: &[Vec<P>], own: Option<usize>| {
        let h = hull(m);
        let pts = h.iter().copied().chain(std::iter::once(m.at)).chain((0..4).map(|k| lerp(h[k], h[(k + 1) % 4], 0.5)));
        pts.into_iter().all(|p| site.wet(sub(p, c))) && taken.iter().enumerate().all(|(i, t)| poly_gap(t, &h) >= if Some(i) == own { 1.0 } else { 2.0 })
    };
    let mut out: Vec<Moored> = Vec::new();
    let mut place = |m: Moored, own: Option<usize>, taken: &mut Vec<Vec<P>>| {
        if afloat(&m, taken, own) {
            taken.push(hull(&m));
            out.push(m);
            true
        } else {
            false
        }
    };
    // The cog first, at the longest pier on the sea or lake that has room on a side.
    if big {
        let mut by_len: Vec<usize> = (0..l.piers.len())
            .filter(|&i| pier_ends(&l.piers[i]).is_some_and(|(_, t)| site.wet_standing(sub(t, c))))
            .collect();
        let plen = |i: usize| pier_ends(&l.piers[i]).map_or(0.0, |(r, t)| dist(r, t));
        by_len.sort_by(|&a, &b| plen(b).total_cmp(&plen(a)).then(a.cmp(&b)));
        let length = 60.0 + 20.0 * unit(hash2(0xc0_9e5e, s.seed as i64, 1)) as f32;
        'cog: for &i in by_len.iter().take(4) {
            let (r, t) = pier_ends(&l.piers[i]).unwrap();
            let u = mul(sub(t, r), 1.0 / dist(r, t).max(1e-9));
            let half_w = 0.5 * dist(l.piers[i][0], l.piers[i][3]);
            for side in [1.0, -1.0] {
                let at = add(add(r, mul(u, dist(r, t) - 0.5 * length as f64 - 4.0)), mul([-u[1], u[0]], side * (half_w + 11.0 + 3.0)));
                let rot = libm::atan2(u[1], u[0]) as f32;
                if place(
                    Moored {
                        at,
                        rot,
                        kind: Afloat::Cog,
                        length,
                    },
                    Some(i),
                    &mut taken,
                ) {
                    break 'cog;
                }
            }
        }
    }
    for i in 0..l.piers.len() {
        let Some((r, t)) = pier_ends(&l.piers[i]) else { continue };
        let h = hash3(0x30a7_b0a7, s.seed as i64, crate::core::round(r[0]) as i64, crate::core::round(r[1]) as i64);
        let standing = site.wet_standing(sub(t, c));
        let kind = if big && standing {
            if i % 2 != 0 {
                continue;
            }
            if unit(h) < 0.5 { Afloat::Rowboat } else { Afloat::FishingBoat }
        } else if unit(h) < 1.0 / 3.0 {
            Afloat::Rowboat
        } else {
            continue;
        };
        let pl = dist(r, t);
        let u = mul(sub(t, r), 1.0 / pl.max(1e-9));
        let half_w = 0.5 * dist(l.piers[i][0], l.piers[i][3]);
        let (blen, beam) = kind.size();
        // (Bow out or in, along the pier, on either side, a little way along; 4 ft off its edge planks.)
        let bow = if unit(h ^ 2) < 0.5 { 0.0 } else { std::f64::consts::PI };
        let first = if unit(h ^ 3) < 0.5 { 1.0 } else { -1.0 };
        'tries: for along in [0.6, 0.8, 0.4] {
            let m = (along * pl).clamp(0.5 * blen + 6.0, (pl - 0.5 * blen - 2.0).max(0.5 * blen + 6.0));
            for side in [first, -first] {
                let at = add(add(r, mul(u, m)), mul([-u[1], u[0]], side * (half_w + 0.5 * beam + 4.0)));
                let rot = (libm::atan2(u[1], u[0]) + bow) as f32;
                if place(
                    Moored {
                        at,
                        rot,
                        kind,
                        length: blen as f32,
                    },
                    Some(i),
                    &mut taken,
                ) {
                    break 'tries;
                }
            }
        }
    }
    // A buoy 30-50 ft off each breakwater's head, on its outer side.
    for (k, mo) in l.moles.iter().enumerate() {
        let n = mo.pts.len();
        let (tip, last) = (mo.pts[n - 1], mo.pts[n - 2]);
        let u = mul(sub(tip, last), 1.0 / dist(last, tip).max(1e-9));
        let at = add(tip, mul(u, MOLE_HEAD_FT + 30.0 + 20.0 * unit(hash2(0xb0_0e, s.seed as i64, k as i64))));
        place(
            Moored {
                at,
                rot: 0.0,
                kind: Afloat::Buoy,
                length: 4.0,
            },
            None,
            &mut taken,
        );
    }
    l.moored = out;
}

/// A big port's harbour master's office, when no building has it: the docks building nearest the
/// piers' roots that fits (roofed, of one piece, a lot of at least 500 sq ft), a warehouse or a home.
fn harbour_master(s: &Settlement, l: &mut Layout, piers: &[(P, P)]) {
    let Some(fi) = catalog::index_of("harbor_master") else { return };
    if l.buildings.iter().any(|b| b.func == Some(fi as u16)) || piers.is_empty() {
        return;
    }
    let warehouse = catalog::index_of("warehouse").map(|w| w as u16);
    let o = piers.iter().fold([0.0, 0.0], |a, &(r, _)| add(a, mul(r, 1.0 / piers.len() as f64)));
    let fits = |b: &Building| {
        b.ward == Ward::Docks
            && b.structure == Structure::Roofed
            && b.parts.is_empty()
            && b.keep.is_none()
            && b.deck.is_none()
            && b.shape.is_none()
            && area(&b.poly).abs() >= 500.0
            && (b.func.is_none() || b.func == warehouse)
    };
    let near = |b: &Building| dist(centroid(&b.poly), o);
    let Some(i) = (0..l.buildings.len())
        .filter(|&i| fits(&l.buildings[i]))
        .min_by(|&a, &b| near(&l.buildings[a]).total_cmp(&near(&l.buildings[b])).then(a.cmp(&b)))
    else {
        return;
    };
    let mut namer = Namer::new(hash2(s.seed, 0x4a7b, 1));
    let mut loc = Locale::of(s, s.river, &mut namer);
    let b = &mut l.buildings[i];
    b.func = Some(fi as u16);
    b.floors = b.floors.max(2);
    b.name = business_name(&CATALOG[fi], &mut namer, &mut loc, Some(b.ward));
}

/// A net rack on the bank by a village's first pier (`Layout::props`).
fn nets(s: &Settlement, l: &mut Layout) {
    if s.tier != Tier::Village {
        return;
    }
    let Some((r, t)) = l.piers.first().and_then(|p| pier_ends(p)) else { return };
    let u = mul(sub(t, r), 1.0 / dist(r, t).max(1e-9));
    let side = if unit(hash2(0x4e7_5ac, s.seed as i64, 0)) < 0.5 { 1.0 } else { -1.0 };
    let at = add(sub(r, mul(u, 10.0)), mul([-u[1], u[0]], side * 12.0));
    if l.buildings.iter().all(|b| poly_gap(&b.poly, &rect(at, [-u[1], u[0]], 12.0, 6.0)) > 2.0) {
        let rot = libm::atan2(u[0], -u[1]) as f32;
        l.props.push(Prop {
            at,
            rot,
            kind: crate::battlemap::Kind::NetRack,
            variant: (hash2(s.seed, 0x4e7, 2) >> 20) as u8 & 3,
            scale: 1.0,
        });
    }
}

/// The harbour things' rules on the battlemap (ids 140-145, M20's range).
pub(crate) const KINDS: [(crate::battlemap::Kind, crate::battlemap::KindInfo); 6] = {
    use crate::battlemap::{Kind, KindInfo};
    const fn k(id: Kind, name: &'static str, radius: f32, blocks_move: bool, cover: u8, difficult: bool, height_ft: f32) -> KindInfo {
        KindInfo {
            id: id as u16,
            name,
            radius,
            blocks_move,
            blocks_sight: false,
            cover,
            difficult,
            height_ft,
            hazard: None,
            feature: true,
        }
    }
    [
        // (Boats along +x, the bow at +x: stepped into, rocking underfoot.)
        (Kind::Rowboat, k(Kind::Rowboat, "rowboat", 1.4, false, 1, true, 3.0)),
        (Kind::FishingBoat, k(Kind::FishingBoat, "fishing boat", 2.4, false, 1, true, 4.0)),
        (Kind::Bollard, k(Kind::Bollard, "bollard", 0.3, false, 1, false, 2.0)),
        // (Its jib along +x, over the water.)
        (Kind::DockCrane, k(Kind::DockCrane, "dock crane", 2.2, true, 2, false, 16.0)),
        (Kind::NetRack, k(Kind::NetRack, "net rack", 1.1, true, 1, false, 6.0)),
        (Kind::Buoy, k(Kind::Buoy, "buoy", 0.45, false, 0, false, 1.0)),
    ]
};

/// The breakwaters' made ground meeting `rect` (world ft), as the battlemap and the finest terrain
/// tiles shape it (`ground::Shaper::with_extra`): only cities' and metropolises' layouts are
/// asked, in settlement order.
pub fn raised_near(world: &World, t0: &T0, rect: [f64; 4]) -> Vec<crate::ground::Raised> {
    let hit = |x: f64, y: f64, r: f64| x + r >= rect[0] && x - r <= rect[2] && y + r >= rect[1] && y - r <= rect[3];
    let mut out = Vec::new();
    for (i, s) in t0.settlements.iter().enumerate() {
        if s.tier < Tier::City || !hit(s.x, s.y, reach(s)) {
            continue;
        }
        let l = layout(world, t0, i);
        for m in &l.moles {
            let b = m.bbox();
            if b[2] + 10.0 >= rect[0] && b[0] - 10.0 <= rect[2] && b[3] + 10.0 >= rect[1] && b[1] - 10.0 <= rect[3] {
                out.extend(m.raised());
            }
        }
    }
    out
}
