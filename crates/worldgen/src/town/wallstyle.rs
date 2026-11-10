//! Walls by culture group (item 104): each group's stone, tower shapes, outline, gatehouse and
//! merlons (`WallStyle`), from a settlement's group (`culture % 5`, as `roof::look` reads it: 0
//! North, 1 West, 2 South, 3 Dry, 4 Steppe), its tier and its wealth; the shape each tower of a
//! wall takes by it (`shape_towers`), and each tower's outline (`footprint`) and squares on the
//! battlemap (`raster`: a ring of wall round an open floor, a door on the town side).

use super::Layout;
use super::geom::*;
use crate::core::rng::{hash2, unit};
use crate::t0::settle::{Settlement, Tier, pop_range};

/// A tower's plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TowerShape {
    #[default]
    Round = 0,
    Square = 1,
    /// Round face out, flat back on the curtain's inner face (`D_BACK` in from its line).
    D = 2,
    /// Octagonal (a side square to the way it is set).
    Octagon = 3,
}

impl TowerShape {
    pub fn from_u8(v: u8) -> TowerShape {
        [TowerShape::Round, TowerShape::Square, TowerShape::D, TowerShape::Octagon][(v as usize).min(3)]
    }
}

/// What a wall is built of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Material {
    #[default]
    Stone = 0,
    /// A timber palisade (its towers square log blockhouses).
    Palisade = 1,
    /// Rammed earth (its towers round, tapering bastions).
    Earth = 2,
}

/// How a wall's line runs (slice 2 lays it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Outline {
    /// Straight curtains between towers, sharp corners.
    #[default]
    Straight = 0,
    /// Straight curtains with angled bastions at the corners.
    Star = 1,
    /// Rounder curtains following the ground.
    Round = 2,
}

/// A gate's form (slice 2 builds it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gatehouse {
    /// One square gate tower over the passage (North).
    SquareTower = 0,
    /// Twin round drum towers flanking the passage (West, Steppe earth walls).
    #[default]
    TwinDrums = 1,
    /// A square gate tower with an arched passage (South).
    Arched = 2,
    /// A bent entrance: the way turns between two square towers (Dry).
    Bent = 3,
    /// A log gate between two log towers (palisades).
    Log = 4,
}

/// The battlements' look (slice 3 draws it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Merlon {
    /// Broad, plain, wide apart (North).
    Broad = 0,
    /// Regular square crenels (West: today's look).
    #[default]
    Square = 1,
    /// Swallowtail, forked (South).
    Swallowtail = 2,
    /// Stepped and pointed (Dry).
    Stepped = 3,
    /// A palisade's sharpened stakes: no merlons.
    Stakes = 4,
    /// Rounded (Steppe earth walls).
    Rounded = 5,
}

/// Stone tones (`WallStyle::stone`), the renderer's index.
pub const STONE_GRANITE: u8 = 0;
pub const STONE_LIMESTONE: u8 = 1;
pub const STONE_SANDSTONE: u8 = 2;
pub const STONE_ROSE_BRICK: u8 = 3;
pub const STONE_MUD_BRICK: u8 = 4;
pub const STONE_TIMBER: u8 = 5;
pub const STONE_EARTH: u8 = 6;

/// Feet between towers along a run (`town::wall_pieces_spaced`): every group's but the Dry's.
pub const SPACING: f64 = 160.0;
pub const SPACING_DRY: f64 = 110.0;
/// A D tower's flat back stands this far in from the curtain's line (its inner face: the wall is 8 ft
/// thick on the battlemap).
pub const D_BACK: f64 = 4.0;
/// A wall's thickness by what it is built of (ft): `WallStyle::thickness`.
pub const STONE_FT: f64 = 8.0;
pub const PALISADE_FT: f64 = 3.0;
pub const EARTH_FT: f64 = 11.0;
/// A wall turning more than this (degrees) at a tower makes it a corner tower.
const CORNER_DEG: f64 = 20.0;

/// How a place builds its walls and towers (the user's table, `docs/roadmap5/towns-104-styles.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallStyle {
    /// Culture group (0 North, 1 West, 2 South, 3 Dry, 4 Steppe).
    pub group: u8,
    /// Stone tone (`STONE_*`).
    pub stone: u8,
    /// Towers along the curtains, and at the corners (and at a wall's ends).
    pub tower: TowerShape,
    pub corner: TowerShape,
    /// Feet between towers along a run.
    pub spacing: f64,
    pub material: Material,
    pub outline: Outline,
    pub gatehouse: Gatehouse,
    pub merlons: Merlon,
    /// A city or metropolis with high wealth; a small or poor town (`of`).
    pub rich: bool,
    pub poor: bool,
}

impl Default for WallStyle {
    /// The West's, as walls were before item 104.
    fn default() -> Self {
        WallStyle::build(1, false, false, false, false)
    }
}

/// A settlement's wealth, 0..1: where its population stands in its tier's range (by logarithm), a
/// capital's 1.
pub fn wealth(s: &Settlement) -> f64 {
    if s.capital {
        return 1.0;
    }
    let (lo, hi) = pop_range(s.tier);
    ((libm::log((s.population as f64).max(1.0)) - libm::log(lo)) / (libm::log(hi) - libm::log(lo))).clamp(0.0, 1.0)
}

/// Rich: a city or metropolis in the upper half of its tier's range (or a capital). Poor: a town in its
/// tier's lowest third.
pub fn rich_poor(tier: Tier, wealth: f64) -> (bool, bool) {
    (tier >= Tier::City && wealth >= 0.5, tier == Tier::Town && wealth < 0.34)
}

impl WallStyle {
    /// A settlement's walls.
    pub fn of(s: &Settlement) -> WallStyle {
        let (rich, poor) = rich_poor(s.tier, wealth(s));
        WallStyle::with(s.culture % 5, rich, poor, s.seed)
    }

    /// A group's walls, rich or poor; `seed` keys the South's share of rose brick.
    pub fn with(group: u8, rich: bool, poor: bool, seed: u64) -> WallStyle {
        let rose = unit(hash2(seed, 0x7761_6c6c, 104)) < 0.3;
        WallStyle::build(group % 5, rich, poor, rose, false)
    }

    /// A castle's walls in this place: stone in a palisaded town (rammed earth on the steppe), the group's
    /// towers.
    pub fn castle(&self) -> WallStyle {
        if self.material != Material::Palisade {
            return *self;
        }
        WallStyle::build(self.group, self.rich, false, self.stone == STONE_ROSE_BRICK, true)
    }

    fn build(group: u8, rich: bool, poor: bool, rose: bool, castle: bool) -> WallStyle {
        use Gatehouse as G;
        use Merlon as M;
        use TowerShape as T;
        let palisade = |group: u8| WallStyle {
            group,
            stone: STONE_TIMBER,
            tower: T::Square,
            corner: T::Square,
            spacing: SPACING,
            material: Material::Palisade,
            outline: if group == 1 { Outline::Straight } else { Outline::Round },
            gatehouse: G::Log,
            merlons: M::Stakes,
            rich,
            poor,
        };
        let stone = |stone: u8, tower: T, corner: T, outline: Outline, gatehouse: G, merlons: M| WallStyle {
            group,
            stone,
            tower,
            corner,
            spacing: SPACING,
            material: Material::Stone,
            outline,
            gatehouse,
            merlons,
            rich,
            poor,
        };
        match group {
            // North: cool granite, round squat towers, rounder curtains; poor towns a palisade.
            0 if poor && !castle => palisade(0),
            0 => stone(STONE_GRANITE, T::Round, T::Round, Outline::Round, G::SquareTower, M::Broad),
            // West: buff limestone, D towers along the curtains, round at the corners; star angles in rich
            // cities; poor towns a palisade.
            1 if poor && !castle => palisade(1),
            1 => stone(STONE_LIMESTONE, T::D, T::Round, if rich { Outline::Star } else { Outline::Straight }, G::TwinDrums, M::Square),
            // South: honey sandstone (some rose brick), square towers, octagonal at a rich city's corners.
            2 => stone(
                if rose { STONE_ROSE_BRICK } else { STONE_SANDSTONE },
                T::Square,
                if rich { T::Octagon } else { T::Square },
                if rich { Outline::Star } else { Outline::Straight },
                G::Arched,
                M::Swallowtail,
            ),
            // Dry: ochre-pink mud brick, square towers set close.
            3 => WallStyle { spacing: SPACING_DRY, ..stone(STONE_MUD_BRICK, T::Square, T::Square, Outline::Round, G::Bent, M::Stepped) },
            // Steppe: a palisade with square log towers; rammed earth with round bastions in rich cities (and
            // a castle's).
            _ if !(rich || castle) => palisade(4),
            _ => WallStyle { material: Material::Earth, ..stone(STONE_EARTH, T::Round, T::Round, Outline::Round, G::TwinDrums, M::Rounded) },
        }
    }

    /// The shape of a tower along a curtain, or at a corner.
    pub fn shape(&self, corner: bool) -> TowerShape {
        if corner { self.corner } else { self.tower }
    }

    /// The wall's thickness (ft): stone `STONE_FT`, a palisade's timber `PALISADE_FT`, rammed earth
    /// `EARTH_FT`.
    pub fn thickness(&self) -> f64 {
        match self.material {
            Material::Stone => STONE_FT,
            Material::Palisade => PALISADE_FT,
            Material::Earth => EARTH_FT,
        }
    }

    /// Whether any of its towers is square.
    pub fn has_square(&self) -> bool {
        self.tower == TowerShape::Square || self.corner == TowerShape::Square
    }

    /// The style packed for the battlemap's vectors (`ShapeKind::Wall`, `Tower`, `Gate`): group (3 bits),
    /// a tower's shape (2, shifted 3), material (2, 5), merlons (3, 7), stone (3, 10), outline (2, 13),
    /// gatehouse (3, 15).
    pub fn code(&self, shape: TowerShape) -> u32 {
        self.group as u32
            | (shape as u32) << 3
            | (self.material as u32) << 5
            | (self.merlons as u32) << 7
            | (self.stone as u32) << 10
            | (self.outline as u32) << 13
            | (self.gatehouse as u32) << 15
    }
}

/// The ways the walls run on from a tower at `t` (unit vectors, one per way).
pub fn wall_ways<'a>(walls: impl Iterator<Item = &'a [P]>, t: P) -> Vec<P> {
    let mut dirs: Vec<P> = Vec::new();
    let mut add_way = |d: P| {
        let l = len(d);
        if l < 1e-6 {
            return;
        }
        let u = mul(d, 1.0 / l);
        if !dirs.iter().any(|q| dot(*q, u) > 0.95) {
            dirs.push(u);
        }
    };
    for w in walls {
        for s in w.windows(2) {
            if seg_dist(t, s[0], s[1]) > 1.0 {
                continue;
            }
            for end in [s[0], s[1]] {
                if dist(end, t) > 1.0 {
                    add_way(sub(end, t));
                }
            }
        }
    }
    dirs
}

/// A tower at `t` stands at a corner of `walls`: where they turn by more than `CORNER_DEG`, at an end,
/// or where three meet.
pub fn is_corner(walls: &[Vec<P>], t: P) -> bool {
    let ways = wall_ways(walls.iter().map(|w| w.as_slice()), t);
    !(ways.len() == 2 && dot(ways[0], ways[1]) < -libm::cos(CORNER_DEG.to_radians()))
}

/// A tower's shape by its walls (a corner where they turn, at an end or where three meet), the way its
/// sides run (a square's or an octagon's: along the longest stretch it stands on) or a D faces (out from
/// `inside`, square to its curtain).
pub fn form_of(style: &WallStyle, walls: &[Vec<P>], t: P, inside: P) -> (TowerShape, P) {
    let shape = style.shape(is_corner(walls, t));
    let along = walls
        .iter()
        .flat_map(|w| w.windows(2))
        .filter(|s| seg_dist(t, s[0], s[1]) < 1.0)
        .max_by(|x, y| dist(x[0], x[1]).total_cmp(&dist(y[0], y[1])))
        .map(|s| mul(sub(s[1], s[0]), 1.0 / dist(s[0], s[1]).max(1e-9)))
        .unwrap_or([1.0, 0.0]);
    match shape {
        TowerShape::D => {
            let mut n = [-along[1], along[0]];
            if dot(n, sub(t, inside)) < 0.0 {
                n = mul(n, -1.0);
            }
            (shape, n)
        }
        _ => (shape, along),
    }
}

/// Shapes the wall towers of `l` that have none yet (`Layout::tower_forms`), by `style` (`castle`: a
/// castle's), D towers facing out from `inside`; square ones also listed in `square_towers`.
pub fn shape_towers(l: &mut Layout, style: &WallStyle, towers: &[P], inside: P, castle: bool) {
    for &t in towers {
        if l.tower_forms.iter().any(|f| same(f.0, t)) {
            continue;
        }
        let (shape, dir) = form_of(style, &l.walls, t, inside);
        l.tower_forms.push((t, shape as u8 | (castle as u8) << 4, dir));
        if shape == TowerShape::Square && l.square_tower(t).is_none() {
            l.square_towers.push((t, dir));
        }
    }
}

/// The towers of `l` with no shape yet (added since, as at a river's banks), shaped by the town's style.
pub fn shape_rest(l: &mut Layout, inside: P) {
    let rest: Vec<P> = l.towers.iter().copied().chain(l.extra_towers.iter().filter(|e| !e.1).map(|e| e.0)).filter(|t| !l.tower_forms.iter().any(|f| same(f.0, *t))).collect();
    let style = l.wall_style;
    shape_towers(l, &style, &rest, inside, false);
}

fn same(a: P, b: P) -> bool {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() < 1e-6
}

/// A tower's outline (anticlockwise): `r` its radius (a square's or an octagon's half width), `dir` the
/// way its sides run (a D: the way it faces).
pub fn footprint(shape: TowerShape, at: P, r: f64, dir: P) -> Vec<P> {
    let u = dir;
    let v = [-u[1], u[0]];
    let pt = |a: f64, b: f64| add(at, add(mul(u, a), mul(v, b)));
    match shape {
        TowerShape::Round => {
            let n = super::roof::ROUND_SIDES;
            (0..n).map(|k| {
                let a = std::f64::consts::TAU * (k as f64 + 0.5) / n as f64;
                pt(r * libm::cos(a), r * libm::sin(a))
            }).collect()
        }
        TowerShape::Square => vec![pt(-r, -r), pt(r, -r), pt(r, r), pt(-r, r)],
        TowerShape::Octagon => {
            let rc = r / libm::cos(std::f64::consts::PI / 8.0);
            (0..8).map(|k| {
                let a = std::f64::consts::PI / 8.0 + std::f64::consts::TAU * k as f64 / 8.0;
                pt(rc * libm::cos(a), rc * libm::sin(a))
            }).collect()
        }
        // (`u` faces out: the flat back `D_BACK` behind the centre, the half round in front.)
        TowerShape::D => {
            let mut out = vec![pt(-D_BACK, -r)];
            for k in 0..=8 {
                let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * k as f64 / 8.0;
                out.push(pt(r * libm::cos(a), r * libm::sin(a)));
            }
            out.push(pt(-D_BACK, r));
            out
        }
    }
}

/// A tower's squares on the battlemap, on the global 5-ft lattice from (`i0`, `j0`), `nx` by `ny`:
/// 0 off it, `RING` its wall (blocking movement and sight), `FLOOR` its open floor, `DOOR` the
/// opening through its wall on the town side (or the castle yard's).
pub struct Raster {
    pub i0: i64,
    pub j0: i64,
    pub nx: usize,
    pub ny: usize,
    pub cells: Vec<u8>,
}
pub const RING: u8 = 1;
pub const FLOOR: u8 = 2;
pub const DOOR: u8 = 3;

impl Raster {
    pub fn at(&self, gi: i64, gj: i64) -> u8 {
        let (i, j) = (gi - self.i0, gj - self.j0);
        if i < 0 || j < 0 || i >= self.nx as i64 || j >= self.ny as i64 { 0 } else { self.cells[j as usize * self.nx + i as usize] }
    }
}

/// How far inside a polygon `p` lies (negative outside).
fn depth(poly: &[P], p: P) -> f64 {
    let n = poly.len();
    let d = (0..n).map(|k| seg_dist(p, poly[k], poly[(k + 1) % n])).fold(f64::MAX, f64::min);
    if contains(poly, p) { d } else { -d }
}

/// The squares of a tower whose outline is `poly`, `sq` ft each: every square whose middle is within
/// 1.5 ft of it (as walls and towers were raised before), its wall the ring of those with a square
/// beside them (corners too) off it, the rest its floor (where none is so, as in a D, those with all
/// four squares beside them on it); the door the ring's square on the floor's side and open ground's
/// (`open` at its middle) furthest toward `inward` (a unit vector). Squares are on the global lattice
/// (`floor(x / sq)`), so every chunk finds the same.
pub fn raster(poly: &[P], inward: P, sq: f64, open: &dyn Fn(P) -> bool, free: &dyn Fn(P) -> bool) -> Raster {
    let bb = poly.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let (i0, j0) = (crate::core::floor((bb[0] - 2.0) / sq) as i64 - 1, crate::core::floor((bb[1] - 2.0) / sq) as i64 - 1);
    let (i1, j1) = (crate::core::floor((bb[2] + 2.0) / sq) as i64 + 1, crate::core::floor((bb[3] + 2.0) / sq) as i64 + 1);
    let (nx, ny) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
    let mid = |i: i64, j: i64| [((i0 + i) as f64 + 0.5) * sq, ((j0 + j) as f64 + 0.5) * sq];
    let on: Vec<bool> = (0..nx * ny).map(|k| depth(poly, mid((k % nx) as i64, (k / nx) as i64)) >= -1.5).collect();
    let on_at = |i: i64, j: i64| i >= 0 && j >= 0 && i < nx as i64 && j < ny as i64 && on[j as usize * nx + i as usize];
    let mut cells = vec![0u8; nx * ny];
    for k in (0..nx * ny).filter(|&k| on[k]) {
        let (i, j) = ((k % nx) as i64, (k / nx) as i64);
        let inner = (-1..=1).all(|dy| (-1..=1).all(|dx| on_at(i + dx, j + dy)));
        cells[k] = if inner { FLOOR } else { RING };
    }
    if !cells.contains(&FLOOR) {
        for k in (0..nx * ny).filter(|&k| on[k]) {
            let (i, j) = ((k % nx) as i64, (k / nx) as i64);
            if [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dy)| on_at(i + dx, j + dy)) {
                cells[k] = FLOOR;
            }
        }
    }
    // The door: a way through the ring from the floor straight out to the ground off it (a square of
    // wall, two where the ring is thicker), its squares free (`free`: no building on them) and two
    // squares beyond it open where any are (else one, else none), the shortest, the furthest toward
    // `inward`.
    let c = centroid(poly);
    let at = |i: i64, j: i64| if i >= 0 && j >= 0 && i < nx as i64 && j < ny as i64 { cells[j as usize * nx + i as usize] } else { 0 };
    let mut best: Option<((u8, i64, f64), Vec<usize>)> = None;
    for f in (0..nx * ny).filter(|&k| cells[k] == FLOOR) {
        let (i, j) = ((f % nx) as i64, (f / nx) as i64);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let mut way: Vec<(i64, i64)> = Vec::new();
            let mut k = 1;
            while k <= 3 && at(i + k * dx, j + k * dy) == RING {
                way.push((i + k * dx, j + k * dy));
                k += 1;
            }
            if way.is_empty() || way.len() > 2 || at(i + k * dx, j + k * dy) != 0 {
                continue;
            }
            let (ox, oy) = (i + k * dx, j + k * dy);
            let level = if !way.iter().all(|&(a, b)| free(mid(a, b))) {
                1
            } else if !open(mid(ox, oy)) {
                2
            } else if !open(mid(ox + dx, oy + dy)) {
                3
            } else {
                4
            };
            let toward = dot(sub(mid(way[0].0, way[0].1), c), inward);
            let score = (level, -(way.len() as i64), toward);
            let better = best.as_ref().is_none_or(|(b, _)| score.0.cmp(&b.0).then(score.1.cmp(&b.1)).then(score.2.total_cmp(&b.2)).is_gt());
            if better {
                best = Some((score, way.iter().map(|&(a, b)| b as usize * nx + a as usize).collect()));
            }
        }
    }
    if let Some((_, way)) = best {
        for k in way {
            cells[k] = DOOR;
        }
    }
    Raster { i0, j0, nx, ny, cells }
}

/// A tower of `l` (`interior::towers`: centre, radius, gate tower) on the battlemap (`raster`): its
/// shape and squares, its door onto ground no building, wall or other tower stands on.
pub fn tower_raster(l: &Layout, t: P, r: f64, gate: bool, sq: f64) -> (TowerShape, Raster) {
    let (shape, _, poly, inward) = tower_outline(l, t, r, gate);
    let near = |q: &P| dist(*q, t) < r + 120.0;
    let houses: Vec<&Vec<P>> = l.buildings.iter().filter(|b| b.poly.iter().any(near)).map(|b| &b.poly).collect();
    let walls: Vec<(P, P)> = l.walls.iter().flat_map(|w| w.windows(2)).filter(|s| seg_dist(t, s[0], s[1]) < r + 60.0).map(|s| (s[0], s[1])).collect();
    let others: Vec<(P, f64)> =
        crate::interior::towers(l).into_iter().filter(|o| dist(o.0, t) > 1e-6 && dist(o.0, t) < r + 60.0).map(|o| (o.0, o.1 * std::f64::consts::SQRT_2)).collect();
    let free = |p: P| !houses.iter().any(|h| contains(h, p));
    // (A wall's squares: within half its thickness and 1.5 ft, as `battlemap` raises it.)
    let reach = (0.5 * l.wall_style_at(t).thickness()).max(2.1) + 1.5;
    let open = |p: P| {
        free(p) && !walls.iter().any(|&(a, b)| seg_dist(p, a, b) <= reach) && !others.iter().any(|&(o, ro)| dist(p, o) <= ro + 1.5)
    };
    (shape, raster(&poly, inward, sq, &open, &free))
}

/// A tower of `l` (`interior::towers`: centre, radius, gate tower) as it is built: its shape, the way it
/// is set (`footprint`), its outline, and the way in to its door (the castle yard for a castle's tower,
/// else the town's middle, as its interior's guardroom door faces).
pub fn tower_outline(l: &Layout, t: P, r: f64, gate: bool) -> (TowerShape, P, Vec<P>, P) {
    let (shape, dir) = if gate { (TowerShape::Round, [1.0, 0.0]) } else { l.tower_form(t) };
    let inside = l.castles.iter().find(|(c, rad)| dist(*c, t) < rad * 1.3).map_or(l.center, |c| c.0);
    let d = sub(inside, t);
    let inward = if len(d) < 1e-6 { [0.0, 1.0] } else { mul(d, 1.0 / len(d)) };
    (shape, dir, footprint(shape, t, r, dir), inward)
}
