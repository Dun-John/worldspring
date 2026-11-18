//! Ground shaped by hand, as the world file holds it: brushed blocks (`GroundBlock`, their
//! string, keys and limits) and plateaus and pits (`Plateau`); `ground` applies them.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::core::geom;

/// Squares per block side (80 ft).
pub const BLOCK: i64 = 16;
pub const CELLS: usize = (BLOCK * BLOCK) as usize;
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
pub const SQUARE_FT: f64 = 5.0;

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

pub use crate::core::geom::bbox;

pub fn crosses_itself(pts: &[[f64; 2]]) -> bool {
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
#[inline]
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
#[inline]
pub fn added(ground: &BTreeMap<String, GroundBlock>, gx: i64, gy: i64) -> f64 {
    if gx < 0 || gy < 0 {
        return 0.0;
    }
    ground.get(&block_key(gx / BLOCK, gy / BLOCK)).map_or(0.0, |b| b.add[((gy % BLOCK) * BLOCK + gx % BLOCK) as usize] as f64 * 0.1)
}
