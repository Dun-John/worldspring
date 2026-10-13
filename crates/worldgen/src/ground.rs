//! Ground shaped by hand at battlemap scale: plateaus and pits (`Edits::plateaus`, `h:<id>`: an
//! outline, a level and an edge width; the castle-on-a-cliff tool) and brushed blocks
//! (`Edits::ground`, `g:XXXXX:YYYYY`: 16 × 16 squares of height change from the Raise, Lower,
//! Smooth and Restore brushes).
//!
//! Both are applied only where heights are read for a battlemap (`battlemap::generate`) and, at
//! the three finest levels, for the terrain texture; never to a tile's `padded` samples, so
//! terrain refinement (and every seam and zoom check) never depends on them. Evaluation is a pure
//! function of position and the edits: plateaus largest first (then by key), then the brushed
//! change, clamped to `MAX_SHIFT_FT` and never sinking ground that was dry below the water by it.

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
/// Shaped ground stays within this of the ground as generated.
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
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

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

fn bbox(pts: &[[f64; 2]]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
}

fn crosses_itself(pts: &[[f64; 2]]) -> bool {
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
    pts: &'a [[f64; 2]],
    level: f64,
    band: f64,
    b: [f64; 4],
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
}

impl<'a> Shaper<'a> {
    /// `None` when nothing shaped meets `rect` (world ft: x0, y0, x1, y1). `spacing` is the
    /// sample spacing (ft): a sheer edge is eased over one sample.
    pub fn new(edits: &'a crate::world::Edits, rect: [f64; 4], spacing: f64) -> Option<Shaper<'a>> {
        if edits.ground.is_empty() && edits.plateaus.is_empty() {
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
        if !edits.ground.is_empty() && sx1 >= 0 && sy1 >= 0 {
            for bx in bx0..=bx1 {
                for (k, b) in edits.ground.range(block_key(bx, by0)..=block_key(bx, by1)) {
                    if let Some((x, y)) = parse_key(k)
                        && x == bx
                        && (by0..=by1).contains(&y)
                    {
                        blocks[((y - by0) * bw + x - bx0) as usize] = Some(b);
                        any = true;
                    }
                }
            }
        }
        let mut plats: Vec<(f64, &String, Kept)> = Vec::new();
        for (k, p) in &edits.plateaus {
            let n = p.pts.len();
            if !(3..=MAX_PLATEAU_PTS).contains(&n) || !p.pts.iter().all(|q| q[0].is_finite() && q[1].is_finite()) || !p.level.is_finite() || !p.edge.is_finite() {
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
            plats.push((geom::area(&p.pts).abs(), k, Kept { pts: &p.pts, level: p.level as f64, band, b }));
        }
        if !any && plats.is_empty() {
            return None;
        }
        // Largest first, so terraces and pits inside a plateau come after it.
        plats.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        Some(Shaper { bx0, by0, bw, bh, blocks: if any { blocks } else { Vec::new() }, plats: plats.into_iter().map(|p| p.2).collect() })
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
    /// weight there, 0 to 1).
    fn shape(&self, x: f64, y: f64, d: f64, g: f32, hw: f32) -> (f32, f32) {
        let g0 = g as f64;
        let (mut h, mut wmax) = (g0, 0.0f64);
        for p in &self.plats {
            if x < p.b[0] - p.band || x > p.b[2] + p.band || y < p.b[1] - p.band || y > p.b[3] + p.band {
                continue;
            }
            let w = if geom::contains(p.pts, [x, y]) {
                1.0
            } else {
                let n = p.pts.len();
                let d = (0..n).map(|i| geom::seg_dist([x, y], p.pts[i], p.pts[(i + 1) % n])).fold(f64::MAX, f64::min);
                if d >= p.band {
                    continue;
                }
                1.0 - smoothstep(0.0, p.band, d)
            };
            h += w * (p.level - h);
            wmax = wmax.max(w);
        }
        h = (h + d).clamp(g0 - MAX_SHIFT_FT, g0 + MAX_SHIFT_FT);
        // Ground that was dry stays above the water by it (raising wet ground makes land).
        if g >= hw {
            h = h.max(g0.min(hw as f64 + 0.5));
        }
        (h as f32, wmax as f32)
    }

    /// Global square (gx, gy) with ground `g` and water `hw` shaped: (height, plateau weight).
    pub fn at_square(&self, gx: i64, gy: i64, g: f32, hw: f32) -> (f32, f32) {
        self.shape((gx as f64 + 0.5) * SQUARE_FT, (gy as f64 + 0.5) * SQUARE_FT, self.added(gx, gy), g, hw)
    }

    /// Any point (world ft) with ground `g` and water `hw` shaped.
    pub fn at_point(&self, x: f64, y: f64, g: f32, hw: f32) -> f32 {
        self.shape(x, y, self.added_at(x, y), g, hw).0
    }
}

/// How far the shaping moves a building standing at `pad` ft (its ground floor): the mean change
/// at its outline's corners and centroid with the ground there taken as `pad` (so on a plateau it
/// stands at the plateau's level), the water as T0's (a pad that was dry stays above it). It reads
/// only the edits round the building, so it is the same in every chunk the building crosses and in
/// its interior. 0 when nothing shaped reaches it.
pub fn pad_shift(edits: &crate::world::Edits, t0: &crate::t0::T0, poly: &[[f64; 2]], pad: f32) -> f32 {
    if (edits.ground.is_empty() && edits.plateaus.is_empty()) || poly.is_empty() {
        return 0.0;
    }
    let Some(s) = Shaper::new(edits, bbox(poly), SQUARE_FT) else {
        return 0.0;
    };
    let pts = poly.iter().copied().chain([geom::centroid(poly)]);
    let sum: f64 = pts.map(|p| (s.at_point(p[0], p[1], pad, t0.sample_water(p[0], p[1])) - pad) as f64).sum();
    (sum / (poly.len() + 1) as f64) as f32
}

/// `pad_shift` at one point (a wall tower's centre).
pub fn shift_at(edits: &crate::world::Edits, t0: &crate::t0::T0, at: [f64; 2], pad: f32) -> f32 {
    if edits.ground.is_empty() && edits.plateaus.is_empty() {
        return 0.0;
    }
    let Some(s) = Shaper::new(edits, [at[0], at[1], at[0], at[1]], SQUARE_FT) else {
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
    let mut next: BTreeMap<String, Box<[i16; CELLS]>> = keys.iter().map(|k| (k.clone(), ground.get(k).map_or_else(|| Box::new([0; CELLS]), |b| b.add.clone()))).collect();
    for &(gx, gy, f) in &touch {
        let d = added(ground, gx, gy);
        let v = match s.mode {
            Brush::Raise => d + s.height_ft * f,
            Brush::Lower => d - s.height_ft * f,
            Brush::Smooth => {
                let h = height(gx, gy);
                let mean = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| height(gx + dx, gy + dy)).sum::<f64>() / 9.0;
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
    };
    match p.problem() {
        Some(e) => Err(e),
        None => Ok(p),
    }
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
