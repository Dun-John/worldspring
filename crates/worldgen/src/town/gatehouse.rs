//! Gatehouses by culture group (item 104): each gate's form as built, from its pair of gate towers
//! (`battlemap::gateways`) and its walls' style (`WallStyle::gatehouse`; a castle's gate its group's
//! too, `WallStyle::castle`). In a gate's frame `s` runs along the wall from tower `a` to tower `b`
//! (`len` apart, 52 ft as `wall_pieces` sets them) and `c` across it into the town (the castle yard):
//! - `SquareTower` (North), `Arched` (South): one square gate tower over the passage, `s` -4 to
//!   `len` + 4, `c` -16 to 16, the way through its ground floor 12 ft wide in the middle. On the
//!   battlemap its two halves are the gate towers (each a ring, a floor and a door); the arch over
//!   the passage is the drawing's.
//! - `TwinDrums` (West, Steppe earth): two round drum towers (radius 15) flanking the passage.
//! - `Log` (palisades): two square log towers (half side 12, sides along the wall), the gate between.
//! - `Bent` (Dry): two square towers (half 12) and a gate block between them (`s` 12 to `len` - 12,
//!   `c` -12 to 30) with the passage turning in it: 10 ft wide, in through the outer face, out through
//!   the block's side behind one tower (`exit`). Needs `len` 40 ft and clear ground for the block and
//!   the way out; else the square tower.

use super::Layout;
use super::geom::*;
use super::wallstyle::{Gatehouse as Form, RING, Raster, TowerShape};

/// Half the side of a log or bent gate's square towers (ft).
pub const SQUARE_HALF: f64 = 12.0;
/// A square gate tower's reach across the wall each way, and past each gate tower's centre along it.
pub const TOWER_DEPTH: f64 = 16.0;
pub const TOWER_OVER: f64 = 4.0;
/// The way through a square gate tower, and through a bent entrance.
pub const PASSAGE_FT: f64 = 12.0;
pub const BENT_FT: f64 = 10.0;
/// A bent gate block's reach into the town.
pub const BENT_DEPTH: f64 = 30.0;
/// The least gap between a bent entrance's towers: the way in (`s` 17 to 27) a foot clear of `b`'s
/// tower, which walls it on that side.
pub const BENT_MIN_LEN: f64 = 40.0;

/// A gatehouse: its gate towers' centres, its form, the way into the town (unit, across the wall),
/// a bent entrance's way out (1 out past `b`, -1 past `a`; 0 for the others), a castle's.
#[derive(Clone, Copy, Debug)]
pub struct House {
    pub a: P,
    pub b: P,
    pub form: Form,
    pub n: P,
    pub exit: f64,
    pub castle: bool,
}

impl House {
    pub fn len(&self) -> f64 {
        dist(self.a, self.b)
    }

    fn u(&self) -> P {
        mul(sub(self.b, self.a), 1.0 / self.len().max(1e-9))
    }

    /// A point of the frame (a bent entrance's mirrored so it always turns toward `b`).
    pub fn at(&self, s: f64, c: f64) -> P {
        let s = if self.exit < 0.0 { self.len() - s } else { s };
        add(self.a, add(mul(self.u(), s), mul(self.n, c)))
    }

    fn rect(&self, s0: f64, s1: f64, c0: f64, c1: f64) -> Vec<P> {
        let q = vec![self.at(s0, c0), self.at(s1, c0), self.at(s1, c1), self.at(s0, c1)];
        if area(&q) < 0.0 { q.into_iter().rev().collect() } else { q }
    }

    /// A gate tower's shape, the way its sides run, its radius (half side) and its outline: `b`'s
    /// where `second`.
    pub fn tower(&self, second: bool) -> (TowerShape, P, f64, Vec<P>) {
        let (t, u, l) = (if second { self.b } else { self.a }, self.u(), self.len());
        match self.form {
            Form::TwinDrums => (TowerShape::Round, [1.0, 0.0], 15.0, super::wallstyle::footprint(TowerShape::Round, t, 15.0, [1.0, 0.0])),
            Form::Log | Form::Bent => (TowerShape::Square, u, SQUARE_HALF, super::wallstyle::footprint(TowerShape::Square, t, SQUARE_HALF, u)),
            Form::SquareTower | Form::Arched => {
                let (h, d) = (0.5 * (l - PASSAGE_FT), TOWER_DEPTH);
                let half = if second { self.rect(l - h, l + TOWER_OVER, -d, d) } else { self.rect(-TOWER_OVER, h, -d, d) };
                (TowerShape::Square, u, 15.0, half)
            }
        }
    }

    /// A bent entrance's gate block.
    pub fn block(&self) -> Option<Vec<P>> {
        let h = SQUARE_HALF;
        (self.form == Form::Bent).then(|| self.rect(h, self.len() - h, -h, BENT_DEPTH))
    }

    /// The open way through (a bent entrance's two legs), each reaching 3 ft past the faces it opens on.
    pub fn passage(&self) -> Vec<Vec<P>> {
        let l = self.len();
        match self.form {
            Form::Bent => {
                let (s0, w, c1) = (SQUARE_HALF + 5.0, BENT_FT, BENT_DEPTH - 5.0);
                vec![self.rect(s0, s0 + w, -SQUARE_HALF - 3.0, c1), self.rect(s0, l - SQUARE_HALF + 3.0, c1 - w, c1)]
            }
            Form::SquareTower | Form::Arched => vec![self.rect(0.5 * (l - PASSAGE_FT), 0.5 * (l + PASSAGE_FT), -TOWER_DEPTH - 3.0, TOWER_DEPTH + 3.0)],
            Form::TwinDrums => vec![self.rect(15.0, l - 15.0, -18.0, 18.0)],
            Form::Log => vec![self.rect(SQUARE_HALF, l - SQUARE_HALF, -15.0, 15.0)],
        }
    }

    /// Where the way through starts outside and ends inside (4 ft past its mouths).
    pub fn ends(&self) -> (P, P) {
        let l = self.len();
        let straight = |c: f64| (self.at(0.5 * l, -c - 4.0), self.at(0.5 * l, c + 4.0));
        match self.form {
            Form::Bent => (self.at(SQUARE_HALF + 5.0 + 0.5 * BENT_FT, -SQUARE_HALF - 4.0), self.at(l - SQUARE_HALF + 4.0, BENT_DEPTH - 5.0 - 0.5 * BENT_FT)),
            Form::SquareTower | Form::Arched => straight(TOWER_DEPTH),
            Form::TwinDrums => straight(15.0),
            Form::Log => straight(SQUARE_HALF),
        }
    }

    /// Everything built: the towers' outlines and a bent entrance's block.
    pub fn solids(&self) -> Vec<Vec<P>> {
        let mut out = vec![self.tower(false).3, self.tower(true).3];
        out.extend(self.block());
        out
    }

    /// The form's code on the vectors (`ShapeKind::Gate` and its towers': the point after the style),
    /// 1 + `Form`; 0 is no gatehouse.
    pub fn code(&self) -> f32 {
        1.0 + self.form as u8 as f32
    }
}

/// The ground a bent entrance's block and the way out of it toward `exit` need, clear of buildings.
fn bent_fits(l: &Layout, h: &House) -> bool {
    let near = |q: &P| dist(*q, h.a) < h.len() + 80.0;
    !l.buildings.iter().filter(|b| b.poly.iter().any(near)).any(|b| bent_on(h, &b.poly))
}

/// Whether `poly` stands on a bent entrance's block or the way out of it.
fn bent_on(h: &House, poly: &[P]) -> bool {
    let len = h.len();
    let apron = h.rect(len - SQUARE_HALF, len - SQUARE_HALF + 14.0, SQUARE_HALF + 1.5, BENT_DEPTH);
    let block = h.block().unwrap_or_default();
    [&block, &apron].iter().any(|z| poly.iter().any(|p| contains(z, *p)) || z.iter().any(|p| contains(poly, *p)))
}

/// Whether a castle's bent entrance at `gate` (the middle of its passage, as `castle::curtain` sets it;
/// `inward` into the yard, unit) has the ground for its block and a way out (either side) with `poly`
/// (its keep) standing: its towers `wall_pieces`' 52 ft apart, a foot to spare.
pub(super) fn bent_room(gate: P, inward: P, poly: &[P]) -> bool {
    let w = mul([-inward[1], inward[0]], 27.0);
    let h = House { a: sub(gate, w), b: add(gate, w), form: Form::Bent, n: mul(inward, (BENT_DEPTH + 1.0) / BENT_DEPTH), exit: 1.0, castle: true };
    [1.0, -1.0].into_iter().any(|e| !bent_on(&House { exit: e, ..h }, poly))
}

/// The gatehouse of gate towers `a` and `b` as its walls' style has it (a bent entrance not yet fitted),
/// `centre` the town's middle (the origin while it is laid out).
fn framed(l: &Layout, a: P, b: P, centre: P) -> House {
    let m = lerp(a, b, 0.5);
    let castle = l.castles.iter().find(|(c, rad)| dist(*c, a) < rad * 1.3);
    let style = l.wall_style_at(a);
    let inside = castle.map_or(centre, |c| c.0);
    let u = sub(b, a);
    let v = mul([-u[1], u[0]], 1.0 / len(u).max(1e-9));
    let n = if dot(sub(inside, m), v) >= 0.0 { v } else { mul(v, -1.0) };
    House { a, b, form: style.gatehouse, n, exit: 0.0, castle: castle.is_some() }
}

/// Whether `poly` stands on one of `blocks` (`bent_blocks`).
pub(super) fn on_bent_block(blocks: &[Vec<P>], poly: &[P]) -> bool {
    blocks.iter().any(|z| poly.iter().any(|p| contains(z, *p)) || z.iter().any(|p| contains(poly, *p)))
}

/// The gate blocks of `l`'s town gates built as bent entrances (none for another style), while it is
/// laid out (about the origin), on its walls as laid (`walls`, `gate_towers`: standing or taken
/// down, so the town is the same either way): the buildings on them give way, as on the ground
/// before each gate.
pub(super) fn bent_blocks(l: &Layout, walls: &[Vec<P>], gate_towers: &[P]) -> Vec<Vec<P>> {
    if l.wall_style.gatehouse != Form::Bent {
        return Vec::new();
    }
    let towers: Vec<(P, f64, bool)> = gate_towers.iter().map(|&t| (t, 15.0, true)).collect();
    crate::battlemap::gateways_on(walls, &towers).into_iter().map(|(a, b)| framed(l, a, b, [0.0, 0.0])).filter(|h| !h.castle && h.form == Form::Bent && h.len() >= BENT_MIN_LEN).filter_map(|h| h.block()).collect()
}

/// The gatehouses of `l`, in the order `battlemap::gateways` pairs their towers: a pure function of the
/// layout, so every chunk finds the same.
pub fn houses(l: &Layout) -> Vec<House> {
    let towers = crate::interior::towers(l);
    crate::battlemap::gateways(l, &towers)
        .into_iter()
        .map(|(a, b)| {
            let mut h = framed(l, a, b, l.center);
            if h.form == Form::Bent {
                // (Out behind `b`'s tower where the ground is clear, else `a`'s; else a square tower.)
                let fit = [1.0, -1.0].into_iter().find(|&e| h.len() >= BENT_MIN_LEN && bent_fits(l, &House { exit: e, ..h }));
                match fit {
                    Some(e) => h.exit = e,
                    None => h.form = Form::SquareTower,
                }
            }
            h
        })
        .collect()
}

/// A gate tower's gatehouse, and whether it is the gatehouse's second (`b`).
pub fn of_tower(houses: &[House], t: P) -> Option<(&House, bool)> {
    let at = |p: P| (p[0] - t[0]).abs() + (p[1] - t[1]).abs() < 1e-6;
    houses.iter().find_map(|h| if at(h.a) { Some((h, false)) } else if at(h.b) { Some((h, true)) } else { None })
}

/// The squares of each bent entrance's gate block on the battlemap (`sq` ft each, on the global
/// lattice): `RING` (blocking) every square within 1.5 ft of the block whose middle is off the way
/// through; the way's squares 0 (the ground as laid: the street).
pub fn rasters(houses: &[House], sq: f64) -> Vec<Raster> {
    let mut out = Vec::new();
    for h in houses {
        let Some(block) = h.block() else { continue };
        let legs = h.passage();
        let bb = block.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        let (i0, j0) = (crate::core::floor((bb[0] - 2.0) / sq) as i64, crate::core::floor((bb[1] - 2.0) / sq) as i64);
        let (i1, j1) = (crate::core::floor((bb[2] + 2.0) / sq) as i64, crate::core::floor((bb[3] + 2.0) / sq) as i64);
        let (nx, ny) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
        let n = block.len();
        let cells = (0..nx * ny)
            .map(|k| {
                let p = [((i0 + (k % nx) as i64) as f64 + 0.5) * sq, ((j0 + (k / nx) as i64) as f64 + 0.5) * sq];
                let on = contains(&block, p) || (0..n).any(|e| seg_dist(p, block[e], block[(e + 1) % n]) <= 1.5);
                if on && !legs.iter().any(|g| contains(g, p)) { RING } else { 0 }
            })
            .collect();
        out.push(Raster { i0, j0, nx, ny, cells });
    }
    out
}
