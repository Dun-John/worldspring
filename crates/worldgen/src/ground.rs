//! Ground shaped by hand at battlemap scale: plateaus and pits (`Edits::plateaus`, `h:<id>`: an
//! outline, a level and an edge width; the castle-on-a-cliff tool) and brushed blocks
//! (`Edits::ground`, `g:XXXXX:YYYYY`: 16 × 16 squares of height change from the Raise, Lower,
//! Smooth and Restore brushes).
//!
//! Both are applied only where heights are read for a battlemap (`battlemap::generate`) and, at
//! the three finest levels, for the terrain texture; never to a tile's `padded` samples, so
//! terrain refinement (and every seam and zoom check) never depends on them. Evaluation is a pure
//! function of position and the edits: plateaus stacked (`stack`: the highest holds, pits dig
//! through, the deepest holds), then the brushed
//! change (at most `MAX_SHIFT_FT` of it; plateaus, pits and water beds go to their own level, with no
//! limit) and never sinking ground that was dry below the water by it.
//! The world's own islets by its cities (`t0::islets`, from T0 alone) are shaped with them
//! wherever T0 is given, and a caller can add raised ground of its own (`Shaper::with_extra`).

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::core::noise::smoothstep;
use crate::town::geom;

/// Squares per block side (80 ft).
pub const BLOCK: i64 = 16;
const CELLS: usize = (BLOCK * BLOCK) as usize;
/// Blocks a world may hold (~0.35 sq mi brushed).
pub const MAX_BLOCKS: usize = 1500;
/// Blocks one stroke may touch (one batch of ops).
pub const MAX_STROKE_BLOCKS: usize = 48;
pub const MAX_PLATEAUS: usize = 256;
pub const MAX_PLATEAU_PTS: usize = 48;
pub const MAX_PLATEAU_SPAN_FT: f64 = 1280.0;
pub const MAX_EDGE_FT: f32 = 60.0;
/// The brushed change stays within this of the ground (plateaus, pits and water beds have no limit).
pub const MAX_SHIFT_FT: f64 = 150.0;
/// A block's values are tenths of a foot, at most this far either way (150 ft).
pub const MAX_ADD: i32 = 1500;
const SQUARE_FT: f64 = 5.0;

/// 16 × 16 squares of brushed change, tenths of a foot, row-major. On the wire one string:
/// base64url (no padding) of zigzag-delta varints, a 0 delta followed by a byte counting the
/// further zeros (a uniform block is ~4 characters). `raw` is kept and written back verbatim.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundBlock {
    pub raw: String,
    pub add: Box<[i16; CELLS]>,
}

impl GroundBlock {
    pub fn new(add: Box<[i16; CELLS]>) -> Self {
        Self { raw: encode(&add), add }
    }

    pub fn is_zero(&self) -> bool {
        self.add.iter().all(|&v| v == 0)
    }
}

impl TryFrom<String> for GroundBlock {
    type Error = String;
    fn try_from(raw: String) -> Result<Self, String> {
        let add = decode(&raw)?;
        Ok(Self { raw, add })
    }
}

impl Serialize for GroundBlock {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for GroundBlock {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        GroundBlock::try_from(String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A flat top (or a pit): ground inside `pts` eases to `level`; outside, back to the ground over
/// `edge` ft (0 = a sheer step at the outline).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Plateau {
    /// World ft, 3 to `MAX_PLATEAU_PTS` points rounded to 0.5 ft, a simple polygon.
    pub pts: Vec<[f64; 2]>,
    /// Ft, the generator's datum (as `height` everywhere).
    pub level: f32,
    /// 0 to `MAX_EDGE_FT`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub edge: f32,
    /// A plateau (the default, not written) or a rock islet out in the water (`islet_for`).
    #[serde(default, skip_serializing_if = "PlateauKind::is_plateau")]
    pub kind: PlateauKind,
    /// What its top (a pit's floor) draws as: absent, the ground's own, but bare rock on a pit's
    /// floor `CHASM_FT` or more below the ground there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<Floor>,
}

/// A plateau's top or a pit's floor (`Plateau::floor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Floor {
    /// The ground's own (grass, sand, ...), however deep.
    Ground,
    /// Bare rock (scree at its foot), at every zoom.
    Rock,
}

impl Floor {
    pub fn parse(s: &str) -> Option<Floor> {
        match s {
            "ground" => Some(Floor::Ground),
            "rock" => Some(Floor::Rock),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Floor::Ground => "ground",
            Floor::Rock => "rock",
        }
    }
}

/// A pit's floor this far (ft) or more below the ground there is bare rock unless its `floor`
/// says otherwise: a chasm, not a lawn between two cliffs.
pub const CHASM_FT: f64 = 15.0;

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlateauKind {
    #[default]
    Plateau,
    /// Rock standing out of the water: its top is bare rock, and buildings, castles and towers
    /// may stand on it (`town::Shore` knows it as dry land).
    Islet,
}

impl PlateauKind {
    fn is_plateau(&self) -> bool {
        *self == PlateauKind::Plateau
    }
}

/// What the shaping makes of a square's ground (`Shaper::at_square_look`): the ground's own,
/// bare rock (an islet's top; `rim`: its edge), or made ground (a quay's or breakwater's top,
/// paved).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Ground,
    Rock { rim: bool },
    Made,
}

/// Raised ground a caller shapes besides the edits and the world's islets (`Shaper::with_extra`):
/// an outline (world ft), its top's level, the band (ft) it eases down over, and its look (made
/// ground only ever raises).
#[derive(Clone, Debug)]
pub struct Raised {
    pub pts: Vec<[f64; 2]>,
    pub level: f32,
    pub band: f32,
    pub look: Look,
}

/// A hand islet's limits: radius (ft), rise over the water (ft), the water's depth (ft).
pub const ISLET_RADIUS_FT: std::ops::RangeInclusive<f64> = 10.0..=200.0;
pub const ISLET_RISE_FT: std::ops::RangeInclusive<f64> = 1.0..=30.0;
pub const ISLET_MAX_DEPTH_FT: f64 = 120.0;
/// An islet's edge (ft): its rock eases down to the bed over this.
pub const ISLET_EDGE_FT: f32 = 6.0;

impl Plateau {
    /// What is wrong with it, if anything (checked when one is made; the generator only skips
    /// one it can't evaluate).
    pub fn problem(&self) -> Option<String> {
        let n = self.pts.len();
        if !(3..=MAX_PLATEAU_PTS).contains(&n) {
            return Some(format!("an outline has 3 to {MAX_PLATEAU_PTS} points, not {n}"));
        }
        if !self.pts.iter().all(|p| p[0].is_finite() && p[1].is_finite()) || !self.level.is_finite() || self.level.abs() > 30_000.0 {
            return Some("points and level must be finite (level ±30,000 ft at most)".into());
        }
        if !(0.0..=MAX_EDGE_FT).contains(&self.edge) {
            return Some(format!("edge is 0 to {MAX_EDGE_FT} ft"));
        }
        let b = bbox(&self.pts);
        if b[2] - b[0] > MAX_PLATEAU_SPAN_FT || b[3] - b[1] > MAX_PLATEAU_SPAN_FT {
            return Some(format!("an outline spans at most {MAX_PLATEAU_SPAN_FT} ft"));
        }
        if geom::area(&self.pts).abs() < 25.0 {
            return Some("the outline holds no ground".into());
        }
        if crosses_itself(&self.pts) {
            return Some("the outline crosses itself".into());
        }
        None
    }
}

pub(crate) fn bbox(pts: &[[f64; 2]]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
}

pub(crate) fn crosses_itself(pts: &[[f64; 2]]) -> bool {
    let n = pts.len();
    let side = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        for j in i + 1..n {
            // Neighbouring edges share a corner.
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (c, d) = (pts[j], pts[(j + 1) % n]);
            let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
            if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) && d1 != 0.0 && d2 != 0.0 && d3 != 0.0 && d4 != 0.0 {
                return true;
            }
        }
    }
    false
}

// ---- the block string ----

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let v = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..c.len() + 1 {
            out.push(B64[(v >> (18 - 6 * k)) as usize & 63] as char);
        }
    }
    out
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(format!("a ground block is base64url, not {:?}", c as char)),
        } as u32)
    };
    let s = s.as_bytes();
    if s.len() % 4 == 1 {
        return Err("a ground block's length is wrong".into());
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for c in s.chunks(4) {
        let mut v = 0u32;
        for (k, &ch) in c.iter().enumerate() {
            v |= val(ch)? << (18 - 6 * k);
        }
        for k in 0..c.len() - 1 {
            out.push((v >> (16 - 8 * k)) as u8);
        }
    }
    Ok(out)
}

/// A block's values as its string.
pub fn encode(add: &[i16; CELLS]) -> String {
    let mut out = Vec::new();
    let (mut prev, mut i) = (0i32, 0);
    while i < CELLS {
        let d = add[i] as i32 - prev;
        prev = add[i] as i32;
        if d == 0 {
            let mut n = 0;
            while i + 1 + n < CELLS && n < 255 && add[i + 1 + n] as i32 == prev {
                n += 1;
            }
            out.extend([0, n as u8]);
            i += 1 + n;
        } else {
            let mut z = ((d << 1) ^ (d >> 31)) as u32;
            while z >= 0x80 {
                out.push(z as u8 | 0x80);
                z >>= 7;
            }
            out.push(z as u8);
            i += 1;
        }
    }
    b64_encode(&out)
}

/// A block's string as its values: exactly 256 of them, each within `MAX_ADD`, nothing after.
pub fn decode(raw: &str) -> Result<Box<[i16; CELLS]>, String> {
    let bytes = b64_decode(raw)?;
    let mut add = Box::new([0i16; CELLS]);
    let (mut prev, mut i, mut at) = (0i32, 0usize, 0usize);
    let bad = || "a ground block holds 256 values of at most ±1500".to_string();
    while at < bytes.len() {
        let mut z = 0u32;
        let mut shift = 0;
        loop {
            let b = *bytes.get(at).ok_or_else(bad)?;
            at += 1;
            z |= ((b & 0x7f) as u32) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 14 {
                return Err(bad());
            }
        }
        let d = (z >> 1) as i32 ^ -((z & 1) as i32);
        let v = prev + d;
        if v.abs() > MAX_ADD || i >= CELLS {
            return Err(bad());
        }
        add[i] = v as i16;
        prev = v;
        i += 1;
        if d == 0 {
            let n = *bytes.get(at).ok_or_else(bad)? as usize;
            at += 1;
            if i + n > CELLS {
                return Err(bad());
            }
            add[i..i + n].fill(v as i16);
            i += n;
        }
    }
    if i != CELLS {
        return Err(bad());
    }
    Ok(add)
}

/// (Roadmap 6 P7) A hash of the shaping that meets `rect` (world ft): the brushed blocks over it and
/// the plateaus meeting it (with their edges), for a layout that reads the ground as shaped
/// (a castle drawn by hand, its moat's roll) to be laid out again when that changes.
pub fn shaped_fp(edits: &crate::world::Edits, rect: [f64; 4]) -> u64 {
    use crate::core::hash::fnv64;
    use crate::core::rng::hash2;
    let b = 16.0 * SQUARE_FT;
    let k = |v: f64| crate::core::floor(v / b) as i64;
    let mut h = 0;
    if !edits.ground.is_empty() {
        for bx in k(rect[0]) - 1..=k(rect[2]) + 1 {
            for (key, g) in edits.ground.range(block_key(bx, k(rect[1]) - 1)..=block_key(bx, k(rect[3]) + 1)) {
                h = hash2(h, fnv64(key.as_bytes()) as i64, fnv64(g.raw.as_bytes()) as i64);
            }
        }
    }
    for (key, p) in &edits.plateaus {
        let e = p.edge as f64 + b;
        let pb = p.pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, q| [r[0].min(q[0]), r[1].min(q[1]), r[2].max(q[0]), r[3].max(q[1])]);
        if pb[2] + e >= rect[0] && pb[0] - e <= rect[2] && pb[3] + e >= rect[1] && pb[1] - e <= rect[3] {
            h = hash2(h, fnv64(key.as_bytes()) as i64, fnv64(serde_json::to_string(p).unwrap_or_default().as_bytes()) as i64);
        }
    }
    h
}

/// A block's key: block indices zero-padded, x first (one column of blocks is one key range).
pub fn block_key(bx: i64, by: i64) -> String {
    format!("g:{bx:05}:{by:05}")
}

/// A key's block indices, if it is one as `block_key` writes them.
pub fn parse_key(k: &str) -> Option<(i64, i64)> {
    let (x, y) = k.strip_prefix("g:")?.split_once(':')?;
    let (bx, by) = (x.parse().ok()?, y.parse().ok()?);
    (k.len() == 13 && block_key(bx, by) == k).then_some((bx, by))
}

/// The brushed change of global square (gx, gy), ft.
pub fn added(ground: &BTreeMap<String, GroundBlock>, gx: i64, gy: i64) -> f64 {
    if gx < 0 || gy < 0 {
        return 0.0;
    }
    ground.get(&block_key(gx / BLOCK, gy / BLOCK)).map_or(0.0, |b| b.add[((gy % BLOCK) * BLOCK + gx % BLOCK) as usize] as f64 * 0.1)
}

// ---- evaluation ----

struct Kept<'a> {
    pts: std::borrow::Cow<'a, [[f64; 2]]>,
    level: f64,
    band: f64,
    b: [f64; 4],
    /// Only ever raises the ground (made ground: a quay's or breakwater's strip).
    raise: bool,
    look: Look,
    /// A hand plateau's `floor`.
    floor: Option<Floor>,
}

/// One step of the shaping (`Shaper::shape`): the pieces `at` (indices into `order`) of one
/// level, raising (none lowers the ground) or digging (pits below the ground, none raises it).
struct Step {
    dig: bool,
    level: f64,
    at: std::ops::Range<usize>,
}

/// A pit's floor eases from levelling only the ground (a piece at the ground's own height) to
/// cutting through raised ground over its first this many ft below the ground (so the shaping
/// stays continuous where a piece's level crosses the ground).
const DIG_EASE_FT: f64 = 5.0;

impl Kept<'_> {
    /// Its weight at (x, y): 1 inside, easing to 0 over its band outside; `None` beyond.
    fn weight(&self, x: f64, y: f64) -> Option<f64> {
        if x < self.b[0] - self.band || x > self.b[2] + self.band || y < self.b[1] - self.band || y > self.b[3] + self.band {
            return None;
        }
        if geom::contains(&self.pts, [x, y]) {
            return Some(1.0);
        }
        let n = self.pts.len();
        let d = (0..n).map(|i| geom::seg_dist([x, y], self.pts[i], self.pts[(i + 1) % n])).fold(f64::MAX, f64::min);
        (d < self.band).then(|| 1.0 - smoothstep(0.0, self.band, d))
    }
}

/// Whether any edits shape the ground: brushed blocks, plateaus, water outlines, quays or breakwaters.
pub fn shapes(edits: &crate::world::Edits) -> bool {
    !edits.ground.is_empty() || !edits.plateaus.is_empty() || !edits.waters.is_empty() || edits.wharves.values().any(|w| w.kind != crate::world::WharfKind::Pier)
}

/// The shaping that reaches one rectangle (a battlemap chunk with its halo, a tile), looked up
/// once: the blocks over it (a dense grid) and the plateaus that meet it.
pub struct Shaper<'a> {
    bx0: i64,
    by0: i64,
    bw: i64,
    bh: i64,
    blocks: Vec<Option<&'a GroundBlock>>,
    plats: Vec<Kept<'a>>,
    /// Indices into `plats`, step by step (`steps`).
    order: Vec<u16>,
    steps: Vec<Step>,
    reach: [f64; 4],
    /// Water drawn by hand meeting the rectangle (`waters`), and the bank's least width.
    waters: Vec<crate::waters::Near>,
    band: f64,
}

impl<'a> Shaper<'a> {
    /// `None` when nothing shaped meets `rect` (world ft: x0, y0, x1, y1). `spacing` is the
    /// sample spacing (ft): a sheer edge is eased over one sample. (T0 places a quay's strip on
    /// its dry side and finds a wharf's level: `wharf`.)
    pub fn new(edits: &'a crate::world::Edits, t0: &crate::t0::T0, rect: [f64; 4], spacing: f64) -> Option<Shaper<'a>> {
        Self::build(edits, Some(t0), rect, spacing)
    }

    /// `new` with raised ground of the caller's own besides (R5 M20: a big port's breakwater).
    pub fn with_extra(edits: &'a crate::world::Edits, t0: &crate::t0::T0, extra: &[Raised], rect: [f64; 4], spacing: f64) -> Option<Shaper<'a>> {
        Self::gather(edits, Some(t0), extra, rect, spacing)
    }

    /// `new`, with quays, breakwaters and the world's islets too only when `t0` is given (a
    /// brush's own change is measured without them).
    pub fn build(edits: &'a crate::world::Edits, t0: Option<&crate::t0::T0>, rect: [f64; 4], spacing: f64) -> Option<Shaper<'a>> {
        Self::gather(edits, t0, &[], rect, spacing)
    }

    fn gather(edits: &'a crate::world::Edits, t0: Option<&crate::t0::T0>, extra: &[Raised], rect: [f64; 4], spacing: f64) -> Option<Shaper<'a>> {
        let near = |b: [f64; 4], band: f64| !(b[2] + band < rect[0] || b[0] - band > rect[2] || b[3] + band < rect[1] || b[1] - band > rect[3]);
        let islet_band = (ISLET_EDGE_FT as f64).max(spacing);
        let world_islets: Vec<(usize, &crate::t0::islets::Islet)> = t0.map_or(Vec::new(), |t0| crate::t0::islets::islets(t0).iter().enumerate().filter(|(_, i)| near(i.bbox, islet_band) && !edits.hidden.contains(&i.id)).collect());
        if !shapes(edits) && world_islets.is_empty() && extra.is_empty() {
            return None;
        }
        // Blocks under the rectangle and one square round it (points between square centres).
        let sq = |v: f64| crate::core::floor(v / SQUARE_FT) as i64;
        let (sx0, sy0, sx1, sy1) = (sq(rect[0]) - 1, sq(rect[1]) - 1, sq(rect[2]) + 1, sq(rect[3]) + 1);
        let (bx0, by0) = (sx0.max(0) / BLOCK, sy0.max(0) / BLOCK);
        let (bx1, by1) = (sx1 / BLOCK, sy1 / BLOCK);
        let (bw, bh) = ((bx1 - bx0 + 1).max(0), (by1 - by0 + 1).max(0));
        let mut blocks = vec![None; (bw * bh) as usize];
        let mut any = false;
        let mut reach = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        let mut grow = |r: [f64; 4]| reach = [reach[0].min(r[0]), reach[1].min(r[1]), reach[2].max(r[2]), reach[3].max(r[3])];
        if !edits.ground.is_empty() && sx1 >= 0 && sy1 >= 0 {
            for bx in bx0..=bx1 {
                for (k, b) in edits.ground.range(block_key(bx, by0)..=block_key(bx, by1)) {
                    if let Some((x, y)) = parse_key(k)
                        && x == bx
                        && (by0..=by1).contains(&y)
                    {
                        blocks[((y - by0) * bw + x - bx0) as usize] = Some(b);
                        any = true;
                        let side = (BLOCK as f64) * SQUARE_FT;
                        grow([x as f64 * side - SQUARE_FT, y as f64 * side - SQUARE_FT, (x + 1) as f64 * side + SQUARE_FT, (y + 1) as f64 * side + SQUARE_FT]);
                    }
                }
            }
        }
        let mut plats: Vec<(std::borrow::Cow<str>, Kept)> = Vec::new();
        for (k, p) in &edits.plateaus {
            if !usable(p) {
                continue;
            }
            let b = bbox(&p.pts);
            if b[2] - b[0] > MAX_PLATEAU_SPAN_FT || b[3] - b[1] > MAX_PLATEAU_SPAN_FT {
                continue;
            }
            let band = (p.edge.clamp(0.0, MAX_EDGE_FT) as f64).max(spacing);
            if b[2] + band < rect[0] || b[0] - band > rect[2] || b[3] + band < rect[1] || b[1] - band > rect[3] {
                continue;
            }
            grow([b[0] - band, b[1] - band, b[2] + band, b[3] + band]);
            let look = if p.kind == PlateauKind::Islet { Look::Rock { rim: false } } else { Look::Ground };
            plats.push((std::borrow::Cow::Borrowed(k.as_str()), Kept { pts: std::borrow::Cow::Borrowed(&p.pts), level: p.level as f64, band, b, raise: false, look, floor: p.floor }));
        }
        // The world's islets (rock, like a hand islet) and the caller's raised ground.
        for (i, isl) in world_islets {
            grow([isl.bbox[0] - islet_band, isl.bbox[1] - islet_band, isl.bbox[2] + islet_band, isl.bbox[3] + islet_band]);
            let kept = Kept { pts: std::borrow::Cow::Owned(isl.pts.clone()), level: isl.level as f64, band: islet_band, b: isl.bbox, raise: false, look: Look::Rock { rim: false }, floor: None };
            plats.push((std::borrow::Cow::Owned(format!("~t0:{i:05}")), kept));
        }
        for (i, r) in extra.iter().enumerate() {
            if r.pts.len() < 3 || !r.level.is_finite() {
                continue;
            }
            let (b, band) = (bbox(&r.pts), (r.band.max(0.0) as f64).max(spacing));
            if !near(b, band) {
                continue;
            }
            grow([b[0] - band, b[1] - band, b[2] + band, b[3] + band]);
            let kept = Kept { pts: std::borrow::Cow::Owned(r.pts.clone()), level: r.level as f64, band, b, raise: r.look == Look::Made, look: r.look, floor: None };
            plats.push((std::borrow::Cow::Owned(format!("~x:{i:05}")), kept));
        }
        // Quays and breakwaters: made ground, raised (never dug) to their level.
        let band = spacing;
        for (k, w) in edits.wharves.iter().filter(|_| t0.is_some()) {
            let wb = crate::wharf::bounds(w);
            if w.kind == crate::world::WharfKind::Pier || wb[2] + band < rect[0] || wb[0] - band > rect[2] || wb[3] + band < rect[1] || wb[1] - band > rect[3] {
                continue;
            }
            // (`problem` without the map's size: the generator only skips what it can't build.)
            if w.problem(f64::MAX, f64::MAX).is_some() {
                continue;
            }
            let t0 = t0.unwrap();
            let level = crate::wharf::level(w, t0) as f64;
            for strip in crate::wharf::strips(w, t0) {
                let b = bbox(&strip);
                if b[2] + band < rect[0] || b[0] - band > rect[2] || b[3] + band < rect[1] || b[1] - band > rect[3] {
                    continue;
                }
                grow([b[0] - band, b[1] - band, b[2] + band, b[3] + band]);
                plats.push((std::borrow::Cow::Borrowed(k.as_str()), Kept { pts: std::borrow::Cow::Owned(strip), level, band, b, raise: true, look: Look::Made, floor: None }));
            }
        }
        let waters = crate::waters::near(edits, rect);
        if let Some(b) = crate::waters::bounds(&waters) {
            grow(b);
        }
        if !any && plats.is_empty() && waters.is_empty() {
            return None;
        }
        let (order, steps) = stack(edits, &plats);
        Some(Shaper { bx0, by0, bw, bh, blocks: if any { blocks } else { Vec::new() }, plats: plats.into_iter().map(|p| p.1).collect(), order, steps, reach, waters, band: spacing })
    }

    /// Where the shaping it holds can change the ground (world ft: x0, y0, x1, y1); outside it
    /// every point is the ground as given.
    pub fn reach(&self) -> [f64; 4] {
        self.reach
    }

    /// The brushed change of global square (gx, gy), ft.
    pub fn added(&self, gx: i64, gy: i64) -> f64 {
        if self.blocks.is_empty() || gx < 0 || gy < 0 {
            return 0.0;
        }
        let (bx, by) = (gx / BLOCK - self.bx0, gy / BLOCK - self.by0);
        if bx < 0 || by < 0 || bx >= self.bw || by >= self.bh {
            return 0.0;
        }
        self.blocks[(by * self.bw + bx) as usize].map_or(0.0, |b| b.add[((gy % BLOCK) * BLOCK + gx % BLOCK) as usize] as f64 * 0.1)
    }

    /// The brushed change at a point: bilinear between square centres.
    fn added_at(&self, x: f64, y: f64) -> f64 {
        if self.blocks.is_empty() {
            return 0.0;
        }
        let (fx, fy) = (x / SQUARE_FT - 0.5, y / SQUARE_FT - 0.5);
        let (i, j) = (crate::core::floor(fx), crate::core::floor(fy));
        let (tx, ty) = (fx - i, fy - j);
        let (i, j) = (i as i64, j as i64);
        let top = self.added(i, j) * (1.0 - tx) + self.added(i + 1, j) * tx;
        let bottom = self.added(i, j + 1) * (1.0 - tx) + self.added(i + 1, j + 1) * tx;
        top * (1.0 - ty) + bottom * ty
    }

    /// Ground `g` (water `hw`) at (x, y) shaped, with `d` brushed on: (height, the plateaus'
    /// weight there, 0 to 1, and what the ground there looks like). Step by step (`stack`): a
    /// raising step eases the ground up to its level (`w * (level - h)`, never down), a digging
    /// step eases it down; so in one stack the highest plateau holds and the deepest pit, and
    /// each edge eases toward the ground left round it by the steps before.
    fn shape(&self, x: f64, y: f64, d: f64, g: f32, hw: f32) -> (f32, f32, Look, bool) {
        // Water drawn by hand first: its bed is the ground the plateaus stand on (an islet or a
        // plateau over it stays dry).
        let (g, hw) = match self.outline_water(x, y) {
            Some((l, bed)) => ((g as f64).min(bed) as f32, hw.max(l as f32)),
            None => (g, hw),
        };
        let g0 = g as f64;
        let (mut h, mut wmax) = (g0, 0.0f64);
        let (mut look, mut floor) = (Look::Ground, false);
        // Each piece's weight here (-1: out of its reach), once.
        let n = self.plats.len();
        let (mut stack, mut heap) = ([0f64; 32], Vec::new());
        let ws: &mut [f64] = if n <= stack.len() {
            &mut stack[..n]
        } else {
            heap.resize(n, 0.0);
            &mut heap
        };
        for (i, p) in self.plats.iter().enumerate() {
            ws[i] = p.weight(x, y).unwrap_or(-1.0);
            wmax = wmax.max(ws[i]);
        }
        for st in &self.steps {
            // Pieces of one level are one ground: the greatest weight of them.
            let Some((w, i)) = self.order[st.at.clone()].iter().map(|&i| (ws[i as usize], i as usize)).filter(|a| a.0 >= 0.0).reduce(|a, b| if b.0 > a.0 { b } else { a }) else {
                continue;
            };
            let (p, l) = (&self.plats[i], st.level);
            let deep = if st.dig {
                // (Only a pit below the ground here digs.)
                if l >= g0 {
                    continue;
                }
                let holds = l <= h;
                let s = ((g0 - l) / DIG_EASE_FT).clamp(0.0, 1.0);
                h -= w * ((h.min(g0) - l).max(0.0) + s * (h - g0).max(0.0));
                if !holds {
                    continue;
                }
                g0 - l
            } else {
                let holds = l >= h || p.raise;
                if l > h {
                    h += w * (l - h);
                }
                if !holds {
                    continue;
                }
                0.0
            };
            if w >= 0.5 {
                let rock = Look::Rock { rim: w < 1.0 };
                (look, floor) = match (p.look, p.floor) {
                    (Look::Ground, Some(Floor::Rock)) => (rock, true),
                    (Look::Ground, None) if deep >= CHASM_FT => (rock, true),
                    (Look::Rock { .. }, _) => (rock, false),
                    (l, _) => (l, false),
                };
            }
        }
        h += d.clamp(-MAX_SHIFT_FT, MAX_SHIFT_FT);
        // Ground that was dry stays above the water by it (raising wet ground makes land).
        if g >= hw {
            h = h.max(g0.min(hw as f64 + 0.5));
        }
        (h as f32, wmax as f32, look, floor)
    }

    /// The water drawn by hand at (x, y): its level and the bed under it (`waters::at`).
    fn outline_water(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if self.waters.is_empty() {
            return None;
        }
        crate::waters::at(&self.waters, x, y, self.band)
    }

    /// The water's surface at (x, y) with water drawn by hand (`hw` as given elsewhere).
    pub fn water(&self, x: f64, y: f64, hw: f32) -> f32 {
        self.outline_water(x, y).map_or(hw, |(l, _)| hw.max(l as f32))
    }

    /// The level of the water drawn by hand holding (x, y), if any.
    pub fn outline_level(&self, x: f64, y: f64) -> Option<f32> {
        self.outline_water(x, y).map(|(l, _)| l as f32)
    }

    /// Whether any water drawn by hand meets what it covers.
    pub fn has_waters(&self) -> bool {
        !self.waters.is_empty()
    }

    /// Global square (gx, gy) with ground `g` and water `hw` shaped: (height, plateau weight).
    pub fn at_square(&self, gx: i64, gy: i64, g: f32, hw: f32) -> (f32, f32) {
        let (h, w, _, _) = self.shape((gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT, self.added(gx, gy), g, hw);
        (h, w)
    }

    /// `at_square` with what the shaping makes of the square's ground: the look of the last
    /// plateau, pit, islet or made ground holding it (at least one half there): an islet's rock,
    /// made ground, a rock floor (`Plateau::floor`, `CHASM_FT`); and whether that is a rock floor.
    pub fn at_square_look(&self, gx: i64, gy: i64, g: f32, hw: f32) -> (f32, f32, Look, bool) {
        self.shape((gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT, self.added(gx, gy), g, hw)
    }

    /// Whether any islet, made ground or plateau that may draw as rock is among what it holds
    /// (`at_square_look` can be other than `Ground`).
    pub fn has_looks(&self) -> bool {
        self.plats.iter().any(|p| p.look != Look::Ground || p.floor != Some(Floor::Ground))
    }

    /// Any point (world ft) with ground `g` and water `hw` shaped.
    pub fn at_point(&self, x: f64, y: f64, g: f32, hw: f32) -> f32 {
        self.shape(x, y, self.added_at(x, y), g, hw).0
    }

    /// `at_point`, and whether a plateau's rock floor (`Plateau::floor`, `CHASM_FT`) holds the
    /// point (the map draws it as rock).
    pub fn at_point_floor(&self, x: f64, y: f64, g: f32, hw: f32) -> (f32, bool) {
        let (h, _, _, floor) = self.shape(x, y, self.added_at(x, y), g, hw);
        (h, floor)
    }

    /// Whether a plateau that may draw a rock floor is among what it holds.
    pub fn has_floors(&self) -> bool {
        self.plats.iter().any(|p| p.look == Look::Ground && !p.raise && p.floor != Some(Floor::Ground))
    }
}

/// Whether the generator can shape with it (`Plateau::problem` checks more when one is made).
fn usable(p: &Plateau) -> bool {
    (3..=MAX_PLATEAU_PTS).contains(&p.pts.len()) && p.pts.iter().all(|q| q[0].is_finite() && q[1].is_finite()) && p.level.is_finite() && p.edge.is_finite()
}

/// The order the pieces shape the ground in (`Shaper::shape`): by how many hand plateaus hold
/// each one (its whole outline inside theirs, counted over the world's plateaus, not only those
/// near, so every rectangle shapes alike), then at each depth the raising steps from the lowest
/// level up and the digging steps from the highest down (made ground never digs). Pieces of one
/// level at one depth make one step. A pit inside a plateau digs it, a pillar inside a pit
/// stands in it, a lower plateau inside a higher one changes nothing, and a higher pit inside a
/// deeper one neither.
fn stack(edits: &crate::world::Edits, plats: &[(std::borrow::Cow<str>, Kept)]) -> (Vec<u16>, Vec<Step>) {
    let hand: Vec<(&str, &[[f64; 2]], [f64; 4])> =
        edits.plateaus.iter().filter(|(_, p)| usable(p)).map(|(k, p)| (k.as_str(), p.pts.as_slice(), bbox(&p.pts))).filter(|(_, _, b)| b[2] - b[0] <= MAX_PLATEAU_SPAN_FT && b[3] - b[1] <= MAX_PLATEAU_SPAN_FT).collect();
    let depth = |k: &str, p: &Kept| -> usize {
        hand.iter()
            .filter(|(hk, pts, b)| *hk != k && b[0] <= p.b[0] && b[1] <= p.b[1] && b[2] >= p.b[2] && b[3] >= p.b[3] && p.pts.iter().all(|q| geom::contains(pts, *q)))
            .count()
    };
    let depths: Vec<usize> = plats.iter().map(|(k, p)| if hand.is_empty() { 0 } else { depth(k, p) }).collect();
    let mut idx: Vec<usize> = (0..plats.len()).collect();
    // (Stable: strips of one quay keep their order.)
    idx.sort_by(|&a, &b| depths[a].cmp(&depths[b]).then_with(|| plats[a].1.level.total_cmp(&plats[b].1.level)).then_with(|| plats[a].0.cmp(&plats[b].0)));
    let (mut order, mut steps): (Vec<u16>, Vec<Step>) = (Vec::with_capacity(2 * idx.len()), Vec::new());
    // A run of one depth's raising (or digging) steps: pieces of one level after another join one.
    let mut run = |dig: bool, pieces: &mut dyn Iterator<Item = usize>| {
        let first = steps.len();
        for i in pieces {
            let level = plats[i].1.level;
            let joins = steps.len() > first;
            match steps.last_mut() {
                Some(st) if joins && st.level == level => st.at.end += 1,
                _ => steps.push(Step { dig, level, at: order.len()..order.len() + 1 }),
            }
            order.push(i as u16);
        }
    };
    let mut s = 0;
    while s < idx.len() {
        let e = s + idx[s..].iter().take_while(|&&i| depths[i] == depths[idx[s]]).count();
        run(false, &mut idx[s..e].iter().copied());
        run(true, &mut idx[s..e].iter().rev().copied().filter(|&i| !plats[i].1.raise));
        s = e;
    }
    (order, steps)
}

/// How far the shaping moves a building standing at `pad` ft (its ground floor): the mean change
/// at its outline's corners and centroid with the ground there taken as `pad` (so on a plateau it
/// stands at the plateau's level), the water as T0's (a pad that was dry stays above it). It reads
/// only the edits round the building, so it is the same in every chunk the building crosses and in
/// its interior. 0 when nothing shaped reaches it.
pub fn pad_shift(edits: &crate::world::Edits, t0: &crate::t0::T0, poly: &[[f64; 2]], pad: f32) -> f32 {
    if poly.is_empty() {
        return 0.0;
    }
    let Some(s) = Shaper::new(edits, t0, bbox(poly), SQUARE_FT) else {
        return 0.0;
    };
    let pts = poly.iter().copied().chain([geom::centroid(poly)]);
    let sum: f64 = pts.map(|p| (s.at_point(p[0], p[1], pad, t0.sample_water(p[0], p[1])) - pad) as f64).sum();
    (sum / (poly.len() + 1) as f64) as f32
}

/// `pad_shift` at one point (a wall tower's centre).
pub fn shift_at(edits: &crate::world::Edits, t0: &crate::t0::T0, at: [f64; 2], pad: f32) -> f32 {
    let Some(s) = Shaper::new(edits, t0, [at[0], at[1], at[0], at[1]], SQUARE_FT) else {
        return 0.0;
    };
    s.at_point(at[0], at[1], pad, t0.sample_water(at[0], at[1])) - pad
}

// ---- making edits ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Brush {
    Raise,
    Lower,
    Smooth,
    Restore,
}

/// One brush stroke: a round brush (`radius_ft`) along `path` (one point: a dab). Each square
/// takes the strongest touch of the stroke, so scrubbing back and forth doesn't pile up.
#[derive(Clone, Debug)]
pub struct Stroke<'p> {
    pub mode: Brush,
    pub path: &'p [[f64; 2]],
    pub radius_ft: f64,
    /// Raise and Lower: at most this far (ft).
    pub height_ft: f64,
    /// 0 (soft: the touch fades from the middle) to 1 (hard: full out to the rim).
    pub hardness: f64,
    /// Smooth and Restore: how far toward their goal (0 to 1).
    pub strength: f64,
}

/// Brush a stroke into `ground`. `height(gx, gy)` is the ground as shown (Smooth eases each
/// square toward its neighbours' mean). Returns the keys changed: set, or removed where a block
/// comes out all zero.
pub fn stamp_brush(ground: &mut BTreeMap<String, GroundBlock>, s: &Stroke, height: &dyn Fn(i64, i64) -> f64) -> Result<Vec<String>, String> {
    let ok = |v: f64| v.is_finite();
    if s.path.is_empty() || !s.path.iter().all(|p| ok(p[0]) && ok(p[1]) && p[0] >= 0.0 && p[1] >= 0.0) {
        return Err("a stroke needs points on the map".into());
    }
    if !(ok(s.radius_ft) && (2.5..=200.0).contains(&s.radius_ft)) || !(ok(s.height_ft) && (0.0..=50.0).contains(&s.height_ft)) {
        return Err("radius 2.5 to 200 ft, height 0 to 50 ft".into());
    }
    let (hard, strength) = (s.hardness.clamp(0.0, 1.0), s.strength.clamp(0.0, 1.0));
    let b = bbox(s.path);
    let sq = |v: f64| crate::core::floor(v / SQUARE_FT) as i64;
    let (sx0, sy0, sx1, sy1) = (sq(b[0] - s.radius_ft).max(0), sq(b[1] - s.radius_ft).max(0), sq(b[2] + s.radius_ft), sq(b[3] + s.radius_ft));
    // Each square's strongest touch.
    let inner = s.radius_ft * hard;
    let mut touch: Vec<(i64, i64, f64)> = Vec::new();
    for gy in sy0..=sy1 {
        for gx in sx0..=sx1 {
            let p = [(gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT];
            let d = if s.path.len() == 1 { geom::dist(p, s.path[0]) } else { s.path.windows(2).map(|w| geom::seg_dist(p, w[0], w[1])).fold(f64::MAX, f64::min) };
            if d >= s.radius_ft {
                continue;
            }
            let f = if d <= inner { 1.0 } else { 1.0 - smoothstep(inner, s.radius_ft, d) };
            if f > 0.0 {
                touch.push((gx, gy, f));
            }
        }
    }
    let mut keys: Vec<String> = touch.iter().map(|&(gx, gy, _)| block_key(gx / BLOCK, gy / BLOCK)).collect();
    keys.sort();
    keys.dedup();
    if keys.len() > MAX_STROKE_BLOCKS {
        return Err(format!("a stroke reaches at most {MAX_STROKE_BLOCKS} blocks of 80 ft (this one {})", keys.len()));
    }
    let win = (s.radius_ft * 0.5).max(2.0 * SQUARE_FT);
    let heights: BTreeMap<(i64, i64), f64> = if s.mode == Brush::Smooth { touch.iter().map(|&(gx, gy, _)| ((gx, gy), height(gx, gy))).collect() } else { BTreeMap::new() };
    let mut next: BTreeMap<String, Box<[i16; CELLS]>> = keys.iter().map(|k| (k.clone(), ground.get(k).map_or_else(|| Box::new([0; CELLS]), |b| b.add.clone()))).collect();
    for &(gx, gy, f) in &touch {
        let d = added(ground, gx, gy);
        let v = match s.mode {
            Brush::Raise => d + s.height_ft * f,
            Brush::Lower => d - s.height_ft * f,
            Brush::Smooth => {
                // The mean of the stroke's own squares within `win` (not the ground beyond the rim,
                // which would pull the stroke up or down toward it each time): a repeated stroke
                // settles instead of piling up.
                let h = height(gx, gy);
                let (mut sum, mut n) = (0.0, 0.0);
                let r = (win / SQUARE_FT) as i64 + 1;
                for dy in -r..=r {
                    for dx in -r..=r {
                        let (x, y) = (gx + dx, gy + dy);
                        if x < sx0 || y < sy0 || x > sx1 || y > sy1 || ((dx * dx + dy * dy) as f64) * SQUARE_FT * SQUARE_FT > win * win {
                            continue;
                        }
                        if let Some(&hh) = heights.get(&(x, y)) {
                            sum += hh;
                            n += 1.0;
                        }
                    }
                }
                let mean = if n > 0.0 { sum / n } else { h };
                d + strength * f * (mean - h)
            }
            Brush::Restore => d * (1.0 - strength * f),
        };
        let q = (crate::core::round(v * 10.0) as i32).clamp(-MAX_ADD, MAX_ADD) as i16;
        next.get_mut(&block_key(gx / BLOCK, gy / BLOCK)).expect("a touched block")[((gy % BLOCK) * BLOCK + gx % BLOCK) as usize] = q;
    }
    let mut changed = Vec::new();
    let grows = next.iter().filter(|(k, a)| !ground.contains_key(*k) && a.iter().any(|&v| v != 0)).count();
    if ground.len() + grows > MAX_BLOCKS {
        return Err(format!("a world holds at most {MAX_BLOCKS} blocks of brushed ground"));
    }
    for (k, add) in next {
        if add.iter().all(|&v| v == 0) {
            if ground.remove(&k).is_some() {
                changed.push(k);
            }
        } else if ground.get(&k).is_none_or(|b| *b.add != *add) {
            ground.insert(k.clone(), GroundBlock::new(add));
            changed.push(k);
        }
    }
    Ok(changed)
}

/// A plateau from an outline drawn by hand: simplified to at most `MAX_PLATEAU_PTS` points at
/// least 5 ft apart, rounded to 0.5 ft, and checked (`Plateau::problem`).
pub fn plateau_for(outline: &[[f64; 2]], level: f64, edge: f64) -> Result<Plateau, String> {
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for &p in outline {
        if pts.last().is_none_or(|q| geom::dist(*q, p) >= 5.0) {
            pts.push(p);
        }
    }
    while pts.len() > 3 && geom::dist(pts[0], pts[pts.len() - 1]) < 5.0 {
        pts.pop();
    }
    let mut tol = 0.5;
    let mut out = simplify(&pts, tol);
    while out.len() > MAX_PLATEAU_PTS {
        tol *= 1.5;
        out = simplify(&pts, tol);
    }
    let p = Plateau {
        pts: out.iter().map(|q| [crate::core::round(q[0] * 2.0) * 0.5, crate::core::round(q[1] * 2.0) * 0.5]).collect(),
        level: (crate::core::round(level * 10.0) * 0.1) as f32,
        edge: edge.clamp(0.0, MAX_EDGE_FT as f64) as f32,
        kind: PlateauKind::Plateau,
        floor: None,
    };
    match p.problem() {
        Some(e) => Err(e),
        None => Ok(p),
    }
}

/// A rock islet for `h:<id>` (`key`) round `at` (world ft): a rough ring of 12 to 20 corners
/// about `radius_ft` from the middle, shaped by the id, its top `rise_ft` over the water there
/// (standing water or a river), easing down to the bed over `ISLET_EDGE_FT`. Refused on dry land
/// and where the water is deeper than `ISLET_MAX_DEPTH_FT`.
pub fn islet_for(key: &str, at: [f64; 2], radius_ft: f64, rise_ft: f64, t0: &crate::t0::T0) -> Result<Plateau, String> {
    islet_for_fine(key, at, radius_ft, rise_ft, t0, None)
}

/// `islet_for` where the water is as the battlemap draws it (`fine`: the water level and the ground at the point, 5-ft
/// squares): T0's shore is coarser, so a spot it calls dry but the fine shore floods is wet (the level and depth are the
/// fine ones), and a spot it floods but the fine shore dries (not a river) is dry land.
pub fn islet_for_fine(key: &str, at: [f64; 2], radius_ft: f64, rise_ft: f64, t0: &crate::t0::T0, fine: Option<(f32, f32)>) -> Result<Plateau, String> {
    use crate::core::rng::stream;
    if !radius_ft.is_finite() || !ISLET_RADIUS_FT.contains(&radius_ft) {
        return Err(format!("radius_ft: {:.0} to {:.0}", ISLET_RADIUS_FT.start(), ISLET_RADIUS_FT.end()));
    }
    if !rise_ft.is_finite() || !ISLET_RISE_FT.contains(&rise_ft) {
        return Err(format!("rise_ft: {:.0} to {:.0}", ISLET_RISE_FT.start(), ISLET_RISE_FT.end()));
    }
    if !at[0].is_finite() || !at[1].is_finite() {
        return Err("at: a point on the map".into());
    }
    let still = t0.sample_water(at[0], at[1]);
    let river = t0.rivers.crossing_level(at, at, 0.0, t0.cell_ft, &crate::lod::rivers::CrossingCache::default()).map_or(crate::t0::hydro::DRY, |(z, _)| z as f32);
    let mut water = still.max(river);
    let mut ground = t0.ground_at(at[0], at[1], 5.0);
    if let Some((hw, g)) = fine {
        if hw > g {
            // (The fine shore floods it: its level over its ground.)
            water = water.max(hw);
            ground = if river > crate::t0::hydro::DRY || ground >= hw as f64 { g as f64 } else { ground.min(g as f64) };
        } else if river <= crate::t0::hydro::DRY {
            water = crate::t0::hydro::DRY;
        }
    }
    // (A river's channel is water whatever the valley's ground there: it never dips below the water.)
    if water <= crate::t0::hydro::DRY || ((water as f64) <= ground && river <= crate::t0::hydro::DRY) {
        return Err(format!("at: ({:.0}, {:.0}) is on dry land: an islet stands in a river, lake or the sea", at[0], at[1]));
    }
    let depth = (water as f64 - ground).max(0.0);
    if depth > ISLET_MAX_DEPTH_FT {
        return Err(format!("at: the water there is {depth:.0} ft deep: at most {ISLET_MAX_DEPTH_FT:.0}"));
    }
    let pts = islet_ring(stream(0x15_1e7, key), at, radius_ft);
    let p = Plateau { pts, level: (crate::core::round((water as f64 + rise_ft) * 10.0) * 0.1) as f32, edge: ISLET_EDGE_FT, kind: PlateauKind::Islet, floor: None };
    match p.problem() {
        Some(e) => Err(e),
        None => Ok(p),
    }
}

/// An islet's rough rock ring round `at` (world ft), shaped by the hash `h`: 12 to 20 corners
/// 0.82 to 1.12 of `radius_ft` from the middle, on the half foot (hand islets and the world's).
pub fn islet_ring(h: u64, at: [f64; 2], radius_ft: f64) -> Vec<[f64; 2]> {
    use crate::core::rng::{hash2, unit};
    let n = 12 + (hash2(h, 0, 0) % 9) as usize;
    (0..n)
        .map(|k| {
            let a = std::f64::consts::TAU * (k as f64 + 0.35 * (unit(hash2(h, k as i64, 1)) - 0.5)) / n as f64;
            let r = radius_ft * (0.82 + 0.3 * unit(hash2(h, k as i64, 2)));
            [crate::core::round((at[0] + r * libm::cos(a)) * 2.0) * 0.5, crate::core::round((at[1] + r * libm::sin(a)) * 2.0) * 0.5]
        })
        .collect()
}

/// The islets (`PlateauKind::Islet`) whose outlines meet `rect` (world ft), fit to shape
/// (`Plateau::problem`): (outline, level, bounds).
pub fn islets_in(edits: &crate::world::Edits, rect: [f64; 4]) -> Vec<(Vec<[f64; 2]>, f32, [f64; 4])> {
    edits
        .plateaus
        .values()
        .filter(|p| p.kind == PlateauKind::Islet && p.problem().is_none())
        .map(|p| (p, bbox(&p.pts)))
        .filter(|(_, b)| b[2] >= rect[0] && b[0] <= rect[2] && b[3] >= rect[1] && b[1] <= rect[3])
        .map(|(p, b)| (p.pts.clone(), p.level, b))
        .collect()
}

/// Douglas–Peucker on a closed outline: split at the point farthest from the first.
fn simplify(pts: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    if pts.len() <= 3 {
        return pts.to_vec();
    }
    let far = (1..pts.len()).max_by(|&a, &b| geom::dist(pts[0], pts[a]).total_cmp(&geom::dist(pts[0], pts[b]))).unwrap_or(1);
    fn dp(pts: &[[f64; 2]], tol: f64, out: &mut Vec<[f64; 2]>) {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let (mut best, mut at) = (0.0, 0);
        for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
            let d = geom::seg_dist(*p, a, b);
            if d > best {
                (best, at) = (d, i);
            }
        }
        if best > tol {
            dp(&pts[..=at], tol, out);
            dp(&pts[at..], tol, out);
        } else {
            out.push(a);
        }
    }
    let mut out = Vec::new();
    dp(&pts[..=far], tol, &mut out);
    let mut back: Vec<[f64; 2]> = pts[far..].to_vec();
    back.push(pts[0]);
    dp(&back, tol, &mut out);
    out
}
