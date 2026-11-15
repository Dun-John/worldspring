//! Towns laid out by hand as a street graph (`Edits.streets`): the records a hand town is made of,
//! and the conversion of a town as it is laid out now (generated, or laid out anew by the ward
//! editor) into them.
//!
//! Keys: `<li>` the town's header (`HandTown`); `<li>/n<id>` a junction, `/e<id>` a street between
//! two junctions, `/k<id>` a block (the face of the graph its `at` lies in), `/r<id>` a row of lots
//! along a street, `/a<id>` an area painted open, `/m<id>` a wall line. `lanes/n<id>` and
//! `lanes/e<id>` are reserved for lanes outside any town. Positions are absolute world ft.
//!
//! A converted block keeps its buildings exactly (`Kept`) until its lots next change: the fill
//! that made them (its block, street lines, district sizes and the state of the stream that cut
//! them) is run again, and what the town did to them after (ids, functions, names, storeys) is
//! written down beside it; a building that did not come from that fill as it stands now (a
//! palace, a temple in pieces, a lot trimmed by a square, a castle's) is stored whole.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::castle::KeepKind;
use super::catalog::{self, Ward};
use super::fill::{District, Fill};
use super::geom::*;
use super::{Building, Layout, ParkKind, Part, PartKind, PlazaKind, RoofStyle, Site, Structure, TownPlan};
use crate::World;
use crate::core::hash::FastMap;
use crate::core::rng::{Pcg32, hash3};
use crate::t0::T0;
use crate::t0::settle::Tier;
use crate::world::{LOT_SIZES, TownEdit};

// ---------------------------------------------------------------------------------------
// Records.

/// A hand town's header (`<li>`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HandTown {
    /// `generated` (converted from the town as it was laid out) or `blank`.
    pub from: String,
    /// The ward editor's edit the town had when converted (restored by "Back to generated").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub was: Option<TownEdit>,
    /// The town's own seed (hex): what blocks laid out anew are seeded from.
    pub seed: String,
    pub culture: u8,
    /// The fill a new block takes.
    #[serde(default)]
    pub fill: BlockFill,
    /// How far from `origin` the town reaches (ft).
    pub extent: f64,
    /// Where the town was laid out from (its centre then): kept blocks are rebuilt about it.
    pub origin: P,
    /// Its tier when converted (0 village … 3 metropolis), and how far its ground was read
    /// (`Site`'s reach, ft): both as the kept blocks were laid out.
    pub tier: u8,
    pub reach: f64,
    /// The town's roof seed (hex): what its yurts' rings are sized by.
    #[serde(default)]
    pub roofs: String,
    /// Buildings in no block (on a bridge, an islet, out in the fields), kept as they were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<Kept>,
}

/// A junction (`/n<id>`), and the square opening there.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Junction {
    pub p: P,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub square: Option<String>,
}

/// What a street is (`Street::class`).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StreetClass {
    /// A main street (from a gate to the market, over a bridge).
    Main,
    #[default]
    Street,
    /// A quay along the water.
    Quay,
    /// No street: two blocks of one district meet (both setbacks 0).
    Joint,
}

impl StreetClass {
    fn is_street(&self) -> bool {
        *self == StreetClass::Street
    }
}

/// A street (`/e<id>`) from junction `a` to `b` by its bends; its paved width, and how far the
/// blocks on its left and right (looking from `a` to `b`) stand back from its line.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Street {
    pub a: u32,
    pub b: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bends: Vec<P>,
    pub width: f64,
    pub set_l: f64,
    pub set_r: f64,
    #[serde(default, skip_serializing_if = "StreetClass::is_street")]
    pub class: StreetClass,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bridge: bool,
}

/// Open ground a block (or a painted area) is laid out as.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpenKind {
    Plaza,
    Lawn,
    Park,
    Garden,
    Market,
    Cemetery,
    Farmland,
}

/// What a block holds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BlockFill {
    /// Lots of a ward (`TOWN_WARDS`), their size (`LOT_SIZES`; none: the ward's usual), rolled
    /// again `reroll` times, a temple's close round its temple or not (none: where it fits).
    Generated {
        ward: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lots: Option<String>,
        #[serde(default, skip_serializing_if = "is_zero")]
        reroll: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        close: Option<bool>,
    },
    /// Left empty, for buildings drawn by hand.
    Hand,
    /// Open ground; `style` the square's kind (`PlazaKind::name`) or the park's (`ParkKind::name`).
    Open {
        open: OpenKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// A castle: as converted, its buildings in the block's `kept` (`hand`: one set on the patch
    /// by hand, not planned).
    Castle {
        #[serde(default, skip_serializing_if = "is_false")]
        hand: bool,
    },
}

impl Default for BlockFill {
    fn default() -> Self {
        BlockFill::Generated { ward: "common".into(), lots: None, reroll: 0, close: None }
    }
}

/// A building pinned to its place (`Block::pins`): what carries something (a name, notes) keeps
/// its id through a change of lots (made by the change function).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pin {
    pub p: P,
    pub id: u32,
}

/// A block (`/k<id>`): the face of the street graph holding `at`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub at: P,
    pub fill: BlockFill,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    /// Its own seed (hex), and how many times its lots have changed.
    pub seed: String,
    #[serde(rename = "gen", default, skip_serializing_if = "is_zero")]
    pub generation: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<Pin>,
    /// Its buildings as converted (until its lots next change).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<Kept>,
}

/// A row of lots along one side of a street (`/r<id>`, path fill): `side` "l" or "r" (looking
/// from the street's `a` to `b`), lots `depth` ft deep, of a ward and lot size, `gaps` the share
/// left open.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub street: u32,
    pub side: String,
    pub depth: f64,
    pub ward: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lots: Option<String>,
    #[serde(default)]
    pub gaps: f64,
    pub seed: String,
}

/// An area painted open within a block (`/a<id>`), cut out of it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Area {
    pub poly: Vec<P>,
    pub open: OpenKind,
}

/// A wall line (`/m<id>`): its corners, closed or not, towers every `spacing` ft (and at the
/// corners listed in `towers`), its gates, its name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WallLine {
    pub pts: Vec<P>,
    pub closed: bool,
    pub spacing: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub towers: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<P>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// A hand town's records, read from `Edits.streets` (those that don't read are left out).
#[derive(Clone, Debug, Default)]
pub struct HandRecords {
    pub town: Option<HandTown>,
    pub junctions: BTreeMap<u32, Junction>,
    pub streets: BTreeMap<u32, Street>,
    pub blocks: BTreeMap<u32, Block>,
    pub rows: BTreeMap<u32, Row>,
    pub areas: BTreeMap<u32, Area>,
    pub walls: BTreeMap<u32, WallLine>,
}

impl HandRecords {
    /// Town `index`'s records in `streets` (`Edits.streets`).
    pub fn of(streets: &BTreeMap<String, Value>, index: usize) -> HandRecords {
        let head = index.to_string();
        let mut out = HandRecords::default();
        let lo = format!("{head}/");
        out.town = streets.get(&head).and_then(|v| serde_json::from_value(v.clone()).ok());
        for (k, v) in streets.range(lo.clone()..).take_while(|(k, _)| k.starts_with(&lo)) {
            let rest = &k[lo.len()..];
            let Some(id) = rest.get(1..).and_then(|n| n.parse::<u32>().ok()) else { continue };
            let v = v.clone();
            match rest.as_bytes()[0] {
                b'n' => drop(serde_json::from_value(v).map(|r| out.junctions.insert(id, r))),
                b'e' => drop(serde_json::from_value(v).map(|r| out.streets.insert(id, r))),
                b'k' => drop(serde_json::from_value(v).map(|r| out.blocks.insert(id, r))),
                b'r' => drop(serde_json::from_value(v).map(|r| out.rows.insert(id, r))),
                b'a' => drop(serde_json::from_value(v).map(|r| out.areas.insert(id, r))),
                b'm' => drop(serde_json::from_value(v).map(|r| out.walls.insert(id, r))),
                _ => {}
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------
// Kept blocks.

/// A converted block's buildings, kept as they were.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Kept {
    /// The fill that laid most of them out, to run again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refill: Option<Refill>,
    /// Runs of that fill's buildings that stand: (its first building, that one's id, how many),
    /// each next building the next id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<[u32; 3]>,
    /// Their storeys, one digit each (in the runs' order).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub floors: String,
    /// What else differs from the fill's (by id).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<Over>,
    /// Buildings stored whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub full: Vec<Full>,
}

/// A fill run again (`fill_block`'s `Lots` or `Farm`): `k` 0 lots, 1 farmland; the ward (`WARDS`
/// index), a temple's close, the farmland's class; `g` its numbers packed (`pack`): the block's
/// `nb` corners (local ft about the header's `origin`), its `nl` street lines (normal, offset),
/// the stream's state (two words), then for lots the district's sizes (4), the close's roll, and
/// with `nt` the outskirts' thinning (its reach and `nt` corners).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Refill {
    pub k: u8,
    pub w: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub close: bool,
    #[serde(default, skip_serializing_if = "is_u8_zero")]
    pub class: u8,
    pub nb: u32,
    pub nl: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nt: Option<u32>,
    pub g: String,
}

fn is_u8_zero(v: &u8) -> bool {
    *v == 0
}

/// What a kept building has that its fill didn't give it (each field only where it differs; a
/// function −1: none). Its home kind otherwise follows its ward and size, as the town's.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Over {
    pub i: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub u: Option<f64>,
}

/// A building stored whole: id, footprint (`v` corners) and pieces (`q`: corners, kind) packed in
/// `g` with its pad level last (and its deck's two ends after, with `e`); ward, function (−1
/// none), home kind, name, storeys, structure, roof, tint, material, shape, shade, keep kind.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Full {
    pub i: u32,
    pub v: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub q: Vec<[u32; 2]>,
    pub g: String,
    pub w: u8,
    #[serde(default = "no_func", skip_serializing_if = "is_no_func")]
    pub f: i32,
    pub r: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<String>,
    pub fl: u8,
    #[serde(default, skip_serializing_if = "is_u8_zero")]
    pub s: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k: Option<u8>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub e: bool,
}

fn no_func() -> i32 {
    -1
}

fn is_no_func(f: &i32) -> bool {
    *f == -1
}

const WARDS: [Ward; 13] = [Ward::Plaza, Ward::Castle, Ward::Temple, Ward::Merchant, Ward::Craft, Ward::Noble, Ward::Common, Ward::Slum, Ward::Docks, Ward::Military, Ward::Farm, Ward::Rural, Ward::Park];
const STRUCTURES: [Structure; 3] = [Structure::Roofed, Structure::Open, Structure::Ruin];
const ROOFS: [RoofStyle; 3] = [RoofStyle::Hip, RoofStyle::Battlements, RoofStyle::Cone];
const KEEPS: [KeepKind; 6] = [KeepKind::Great, KeepKind::Hall, KeepKind::L, KeepKind::T, KeepKind::Shell, KeepKind::Plain];
const PARTS: [PartKind; 6] = [PartKind::Body, PartKind::Tower { round: false }, PartKind::Tower { round: true }, PartKind::Forebuilding, PartKind::Wing, PartKind::Apse];

fn index_of<T: PartialEq>(list: &[T], v: &T) -> u8 {
    list.iter().position(|x| x == v).expect("every kind is listed") as u8
}

fn nth<T: Copy>(list: &[T], i: u8, what: &str) -> Result<T, String> {
    list.get(i as usize).copied().ok_or_else(|| format!("kept building: no {what} {i}"))
}

/// Numbers packed exactly: each f64's bits, little-endian, base64url without padding.
pub fn pack(fs: &[f64]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes: Vec<u8> = fs.iter().flat_map(|f| f.to_bits().to_le_bytes()).collect();
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 3);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..=c.len() {
            out.push(ABC[(n >> (18 - 6 * k) & 63) as usize] as char);
        }
    }
    out
}

/// `pack` undone.
pub fn unpack(s: &str) -> Result<Vec<f64>, String> {
    let val = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(format!("packed numbers: {:?} is no digit", c as char)),
        } as u32)
    };
    let mut bytes = Vec::with_capacity(s.len() * 3 / 4);
    for c in s.as_bytes().chunks(4) {
        if c.len() < 2 {
            return Err("packed numbers: cut short".into());
        }
        let mut n = 0;
        for (k, &d) in c.iter().enumerate() {
            n |= val(d)? << (18 - 6 * k);
        }
        bytes.extend((0..c.len() - 1).map(|k| (n >> (16 - 8 * k) & 255) as u8));
    }
    if bytes.len() % 8 != 0 {
        return Err("packed numbers: not whole numbers".into());
    }
    Ok(bytes.chunks(8).map(|b| f64::from_bits(u64::from_le_bytes(b.try_into().unwrap()))).collect())
}

fn pts_of(fs: &[f64]) -> Vec<P> {
    fs.chunks(2).map(|c| [c[0], c[1]]).collect()
}

impl Refill {
    /// A fill as `build_town` hands it to `fill_block` (`Lots` and `Farm` only: the others build
    /// nothing there), `r65` its patch's close roll.
    fn of(fill: &Fill, block: &[P], lines: &[(P, f64)], r65: f64) -> Option<Refill> {
        let mut g: Vec<f64> = block.iter().flat_map(|p| *p).collect();
        g.extend(lines.iter().flat_map(|(n, c)| [n[0], n[1], *c]));
        let (nb, nl) = (block.len() as u32, lines.len() as u32);
        let words = |r: &Pcg32| {
            let (s, i) = r.state();
            [f64::from_bits(s), f64::from_bits(i)]
        };
        match fill {
            Fill::Farm { class, rng } => {
                g.extend(words(rng));
                Some(Refill { k: 1, w: 0, close: false, class: *class, nb, nl, nt: None, g: pack(&g) })
            }
            Fill::Lots { ward, dp, rng, thin, close } => {
                g.extend(words(rng));
                g.extend([dp.min_sq, dp.grid_chaos, dp.size_chaos, dp.block_size, r65]);
                if let Some((pts, reach)) = thin {
                    g.push(*reach);
                    g.extend(pts.iter().flat_map(|p| *p));
                }
                Some(Refill { k: 0, w: index_of(&WARDS, ward), close: *close, class: 0, nb, nl, nt: thin.as_ref().map(|t| t.0.len() as u32), g: pack(&g) })
            }
            _ => None,
        }
    }

    /// Its fill laid out again (local ft about the origin), as `fill_block` laid it out then.
    fn run(&self, site: &Site, meta: (u32, P, f64, Tier)) -> Result<Vec<Building>, String> {
        let g = unpack(&self.g)?;
        let (nb, nl) = (self.nb as usize, self.nl as usize);
        let need = 2 * nb + 3 * nl + 2 + if self.k == 0 { 5 + self.nt.map_or(0, |t| 1 + 2 * t as usize) } else { 0 };
        if g.len() != need {
            return Err(format!("kept block: {} numbers packed, {need} wanted", g.len()));
        }
        let block = pts_of(&g[..2 * nb]);
        let lines: Vec<(P, f64)> = g[2 * nb..2 * nb + 3 * nl].chunks(3).map(|c| ([c[0], c[1]], c[2])).collect();
        let at = 2 * nb + 3 * nl;
        let rng = Pcg32::from_state(g[at].to_bits(), g[at + 1].to_bits());
        let (fill, r65) = match self.k {
            1 => (Fill::Farm { class: self.class, rng }, 0.0),
            0 => {
                let d = &g[at + 2..at + 7];
                let dp = District { min_sq: d[0], grid_chaos: d[1], size_chaos: d[2], block_size: d[3] };
                let thin = self.nt.map(|_| (pts_of(&g[at + 8..]), g[at + 7]));
                (Fill::Lots { ward: nth(&WARDS, self.w, "ward")?, dp, rng, thin, close: self.close }, d[4])
            }
            k => return Err(format!("kept block: no fill kind {k}")),
        };
        let mut l = Layout::empty(meta.0, meta.1, meta.2, meta.3);
        let mut plots = Vec::new();
        super::fill_block(site, &block, lines, fill, &|_| r65, super::places::Spot::default(), &mut l, &mut plots);
        Ok(l.buildings)
    }
}

impl Full {
    fn of(b: &Building) -> Full {
        let mut g: Vec<f64> = b.poly.iter().flat_map(|p| *p).collect();
        let q = b.parts.iter().map(|p| [p.poly.len() as u32, index_of(&PARTS, &p.kind) as u32]).collect();
        g.extend(b.parts.iter().flat_map(|p| p.poly.iter().flat_map(|p| *p)));
        g.push(b.pad_ft as f64);
        if let Some(d) = b.deck {
            g.extend(d.iter().flat_map(|p| *p));
        }
        Full {
            i: b.id,
            v: b.poly.len() as u32,
            q,
            g: pack(&g),
            w: index_of(&WARDS, &b.ward),
            f: b.func.map_or(-1, |f| f as i32),
            r: b.residential,
            n: b.name.clone(),
            fl: b.floors,
            s: index_of(&STRUCTURES, &b.structure),
            o: b.roof.map(|r| index_of(&ROOFS, &r)),
            t: b.tint,
            m: b.material,
            h: b.shape,
            d: b.shade,
            k: b.keep.map(|k| index_of(&KEEPS, &k)),
            e: b.deck.is_some(),
        }
    }

    fn building(&self) -> Result<Building, String> {
        let g = unpack(&self.g)?;
        let parts_n: usize = self.q.iter().map(|q| q[0] as usize).sum();
        let need = 2 * (self.v as usize + parts_n) + 1 + if self.e { 4 } else { 0 };
        if g.len() != need {
            return Err(format!("kept building {}: {} numbers packed, {need} wanted", self.i, g.len()));
        }
        let mut at = 2 * self.v as usize;
        let mut parts = Vec::new();
        for q in &self.q {
            let n = 2 * q[0] as usize;
            parts.push(Part { poly: pts_of(&g[at..at + n]), kind: nth(&PARTS, q[1] as u8, "piece")? });
            at += n;
        }
        let pad_ft = g[at] as f32;
        let deck = self.e.then(|| [[g[at + 1], g[at + 2]], [g[at + 3], g[at + 4]]]);
        Ok(Building {
            poly: pts_of(&g[..2 * self.v as usize]),
            ward: nth(&WARDS, self.w, "ward")?,
            func: (self.f >= 0).then_some(self.f as u16),
            residential: self.r,
            name: self.n.clone(),
            floors: self.fl,
            pad_ft,
            structure: nth(&STRUCTURES, self.s, "structure")?,
            roof: self.o.map(|o| nth(&ROOFS, o, "roof")).transpose()?,
            tint: self.t,
            material: self.m,
            shape: self.h,
            shade: self.d,
            id: self.i,
            parts,
            keep: self.k.map(|k| nth(&KEEPS, k, "keep")).transpose()?,
            deck,
        })
    }
}

/// A fill's building moved to world ft (as `generate_in` moves every building).
fn to_world(mut b: Building, origin: P) -> Building {
    b.poly.iter_mut().chain(b.parts.iter_mut().flat_map(|p| p.poly.iter_mut())).for_each(|p| *p = add(*p, origin));
    b
}

/// The home kind a building of no function has in a town (`assign_functions`).
fn home_kind(b: &Building) -> u8 {
    catalog::residential_for(b.ward, area(&b.poly).abs() > 2500.0) as u8
}

/// A fill's building `raw` (world ft) as the town made it: its id and storeys, what `o` sets
/// and, with `o.y`, on the round footprint `yurts::ring` stands a steppe home on in its lot,
/// turned `y` corners to its door (and drawn up `o.u` ft toward it, as `yurts::extra` draws
/// one up beside a pen or a store).
fn rebuild(raw: &Building, id: u32, floors: u8, o: &Over, roofs: &Layout) -> Result<Building, String> {
    let mut b = raw.clone();
    b.id = id;
    b.floors = floors;
    apply(o, &mut b)?;
    if let Some(y) = o.y {
        let mut ring = super::yurts::ring(roofs, &b).filter(|r| (y as usize) < r.len()).ok_or("kept block: no yurt on its lot")?;
        ring.rotate_left(y as usize);
        if let Some(t) = o.u {
            let n = super::yurts::frame(&ring).2;
            ring.iter_mut().for_each(|p| *p = add(*p, mul(n, t)));
        }
        b.poly = ring;
    }
    Ok(b)
}

/// What the town gave a fill's building `raw` (world ft) to make it `b`, if only what `Over` and
/// the storeys hold differs: `rebuild` makes `b` again exactly (else it is stored whole).
fn over(raw: &Building, b: &Building, roofs: &Layout) -> Option<Over> {
    if !(1..=9).contains(&b.floors) {
        return None;
    }
    let mut o = Over { i: b.id, ..Over::default() };
    if b.ward != raw.ward {
        o.w = Some(index_of(&WARDS, &b.ward));
    }
    if b.func != raw.func {
        o.f = Some(b.func.map_or(-1, |f| f as i32));
    }
    if b.name != raw.name {
        o.n = b.name.clone();
    }
    if b.pad_ft.to_bits() != raw.pad_ft.to_bits() {
        o.p = Some(b.pad_ft as f64);
    }
    if b.structure != raw.structure {
        o.s = Some(index_of(&STRUCTURES, &b.structure));
    }
    if b.residential != rebuild(raw, b.id, b.floors, &o, roofs).ok()?.residential {
        o.r = Some(b.residential);
    }
    if raw.poly != b.poly {
        let ring = super::yurts::ring(roofs, &rebuild(raw, b.id, b.floors, &o, roofs).ok()?)?;
        let n = ring.len();
        if b.poly.len() != n {
            return None;
        }
        // (Turned to its door; or drawn up toward it too, by a quarter foot or so: the step
        // `yurts::extra` takes, its door's way out from its middle.)
        let (y, t) = (0..n).find_map(|y| {
            let turned: Vec<P> = (0..n).map(|k| ring[(k + y) % n]).collect();
            if turned == b.poly {
                return Some((y, None));
            }
            let dir = super::yurts::frame(&turned).2;
            let t = crate::core::round(4.0 * dot(sub(b.poly[0], turned[0]), dir)) / 4.0;
            (t > 0.0 && turned.iter().zip(&b.poly).all(|(p, q)| add(*p, mul(dir, t)) == *q)).then_some((y, Some(t)))
        })?;
        (o.y, o.u) = (Some(y as u8), t);
    }
    differs(&rebuild(raw, b.id, b.floors, &o, roofs).ok()?, b).is_empty().then_some(o)
}

/// `over` applied to a fill's building (its storeys already set), but for its footprint.
fn apply(o: &Over, b: &mut Building) -> Result<(), String> {
    if let Some(w) = o.w {
        b.ward = nth(&WARDS, w, "ward")?;
    }
    if let Some(f) = o.f {
        b.func = (f >= 0).then_some(f as u16);
    }
    if let Some(n) = &o.n {
        b.name = Some(n.clone());
    }
    if let Some(p) = o.p {
        b.pad_ft = p as f32;
    }
    if let Some(s) = o.s {
        b.structure = nth(&STRUCTURES, s, "structure")?;
    }
    b.residential = match (o.r, b.func) {
        (Some(r), _) => r,
        (None, None) => home_kind(b),
        (None, Some(_)) => b.residential,
    };
    Ok(())
}

/// Where and on what ground a hand town's kept blocks are rebuilt: the layout's index, origin,
/// radius and tier as `fill_block` saw them.
pub struct KeptGround<'a> {
    site: Site<'a>,
    meta: (u32, P, f64, Tier),
    /// What a yurt's ring is read from (`yurts::ring`: the town's roof seed).
    roofs: Layout,
}

impl<'a> KeptGround<'a> {
    /// The ground about the header's origin, as read when the town was converted.
    pub fn of(world: &World, t0: &'a T0, index: usize, town: &HandTown) -> KeptGround<'a> {
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
        let site = Site::new(t0, town.origin, lattice, town.reach, super::river_pieces(t0, town.origin, town.reach), world.params().sea_level_ft);
        let meta = (index as u32, town.origin, 0.0, Tier::from_u8(town.tier));
        KeptGround { site, meta, roofs: roofs_of(meta, u64::from_str_radix(&town.roofs, 16).unwrap_or(0)) }
    }
}

fn roofs_of(meta: (u32, P, f64, Tier), roof_seed: u64) -> Layout {
    let mut l = Layout::empty(meta.0, meta.1, meta.2, meta.3);
    l.roof_seed = roof_seed;
    l
}

/// A kept block's buildings (world ft), in id order, from its record alone.
pub fn kept_on(ground: &KeptGround, kept: &Kept) -> Result<Vec<Building>, String> {
    let origin = ground.meta.1;
    let raw = match &kept.refill {
        Some(r) => r.run(&ground.site, ground.meta)?,
        None => Vec::new(),
    };
    let overs: FastMap<u32, &Over> = kept.set.iter().map(|o| (o.i, o)).collect();
    let mut floors = kept.floors.bytes();
    let mut out = Vec::new();
    for &[k, id, n] in &kept.runs {
        for j in 0..n {
            let b = raw.get((k + j) as usize).ok_or_else(|| format!("kept block: its fill laid out {} buildings, not {}", raw.len(), k + j + 1))?;
            let fl = floors.next().filter(u8::is_ascii_digit).ok_or("kept block: storeys cut short")? - b'0';
            let none = Over::default();
            let o = overs.get(&(id + j)).copied().unwrap_or(&none);
            out.push(rebuild(&to_world(b.clone(), origin), id + j, fl, o, &ground.roofs)?);
        }
    }
    for f in &kept.full {
        out.push(f.building()?);
    }
    out.sort_by_key(|b| b.id);
    Ok(out)
}

/// Block `block` of hand town `index`: its kept buildings (world ft, id order), from its record
/// alone (none if it keeps none).
pub fn kept_buildings(world: &World, t0: &T0, index: usize, block: u32) -> Result<Vec<Building>, String> {
    let recs = HandRecords::of(&world.file.edits.streets, index);
    let town = recs.town.as_ref().ok_or("not a hand town")?;
    let b = recs.blocks.get(&block).ok_or("no such block")?;
    match &b.kept {
        Some(k) => kept_on(&KeptGround::of(world, t0, index, town), k),
        None => Ok(Vec::new()),
    }
}

/// The fields two buildings differ in (exactly: every float by its bits).
pub fn differs(a: &Building, b: &Building) -> Vec<&'static str> {
    let mut out = Vec::new();
    let d = |x: &dyn std::fmt::Debug, y: &dyn std::fmt::Debug| format!("{x:?}") != format!("{y:?}");
    for (name, diff) in [
        ("poly", d(&a.poly, &b.poly)),
        ("ward", a.ward != b.ward),
        ("func", a.func != b.func),
        ("residential", a.residential != b.residential),
        ("name", a.name != b.name),
        ("floors", a.floors != b.floors),
        ("pad_ft", a.pad_ft.to_bits() != b.pad_ft.to_bits()),
        ("structure", a.structure != b.structure),
        ("roof", a.roof != b.roof),
        ("look", (a.tint, a.material, a.shape, a.shade) != (b.tint, b.material, b.shape, b.shade)),
        ("id", a.id != b.id),
        ("parts", d(&a.parts, &b.parts)),
        ("keep", a.keep != b.keep),
        ("deck", d(&a.deck, &b.deck)),
    ] {
        if diff {
            out.push(name);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Capture: what `build_town` decided per patch, read once by a conversion.

#[derive(Default)]
struct Capture {
    index: usize,
    plan: Option<Rc<TownPlan>>,
    towns: u32,
    faces: Vec<Vec<usize>>,
    cells: Vec<Vec<P>>,
    labels: Vec<Vec<i32>>,
    insets: Vec<Vec<f64>>,
    ward: Vec<Option<Ward>>,
    group: Vec<usize>,
    walled: bool,
    gates: Vec<P>,
    pending: Option<(usize, Refill)>,
    fills: BTreeMap<usize, (Refill, Vec<Building>)>,
    squares: Vec<(usize, PlazaKind)>,
}

thread_local! {
    static CAP: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

fn with_cap(f: impl FnOnce(&mut Capture)) {
    CAP.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            f(c)
        }
    })
}

/// Whether layout `index` is being converted (`build_town` then records its patches).
pub(super) fn capturing(index: u32) -> bool {
    CAP.with(|c| c.borrow().as_ref().is_some_and(|c| c.index == index as usize))
}

pub(super) fn cap_plan(index: usize, plan: &Rc<TownPlan>) {
    with_cap(|c| {
        if c.index == index {
            c.plan = Some(plan.clone());
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn cap_town(faces: &[Vec<usize>], cells: &[Labeled], insets: Vec<Vec<f64>>, ward: &[Option<Ward>], group: &[usize], walled: bool, gates: &[P]) {
    with_cap(|c| {
        c.towns += 1;
        c.faces = faces.to_vec();
        c.cells = cells.iter().map(|l| l.pts.clone()).collect();
        c.labels = cells.iter().map(|l| l.labels.clone()).collect();
        c.insets = insets;
        c.ward = ward.to_vec();
        c.group = group.to_vec();
        c.walled = walled;
        c.gates = gates.to_vec();
    })
}

pub(super) fn cap_fill(i: usize, fill: &Fill, block: &[P], lines: &[(P, f64)], r65: f64) {
    with_cap(|c| c.pending = Refill::of(fill, block, lines, r65).map(|r| (i, r)))
}

pub(super) fn cap_built(i: usize, built: &[Building]) {
    with_cap(|c| {
        if let Some((p, r)) = c.pending.take()
            && p == i
        {
            c.fills.insert(i, (r, built.to_vec()));
        }
    })
}

pub(super) fn cap_square(v: usize, kind: PlazaKind) {
    with_cap(|c| c.squares.push((v, kind)))
}

// ---------------------------------------------------------------------------------------
// Conversion.

fn hex(v: u64) -> String {
    format!("{v:016x}")
}

/// A point well inside a polygon: its centroid if inside, else the middle of the widest chord
/// across it from one of its corners (its mean, failing those).
fn inside_point(poly: &[P]) -> P {
    let c = centroid(poly);
    if contains(poly, c) {
        return c;
    }
    let n = poly.len();
    let mut best: Option<(f64, P)> = None;
    for i in 0..n {
        for j in i + 2..n {
            let m = lerp(poly[i], poly[j], 0.5);
            if contains(poly, m) && best.is_none_or(|(d, _)| dist(poly[i], poly[j]) > d) {
                best = Some((dist(poly[i], poly[j]), m));
            }
        }
    }
    best.map_or_else(|| mean(poly), |b| b.1)
}

/// Town `index` as laid out now (generated, or by its ward edit), as hand town records (key,
/// value), in key order. A village has no streets plan to convert.
pub fn convert(world: &World, t0: &T0, index: usize) -> Result<Vec<(String, Value)>, String> {
    let s = super::culture::settlement(world, t0, index).ok_or("not a settlement")?;
    if s.tier == Tier::Village {
        return Err("a village has no streets plan to lay out by hand".into());
    }
    if HandRecords::of(&world.file.edits.streets, index).town.is_some() {
        return Err("already laid out by hand".into());
    }
    let edit = super::wards::edit_of(world, t0, index);
    // (The layouts it reads made first, so the one run here is the only one recorded.)
    let base = super::base_layout(world, t0, index);
    super::generated_layout(world, t0, index);
    CAP.with(|c| *c.borrow_mut() = Some(Capture { index, ..Capture::default() }));
    let again = super::generate_with(world, t0, index, edit, None);
    let cap = CAP.with(|c| c.borrow_mut().take()).expect("the capture just set");
    let plan = cap.plan.clone().ok_or("no plan laid out")?;
    if cap.towns != 1 {
        return Err(format!("the town was built {} times converting it", cap.towns));
    }
    if again.buildings.len() != base.buildings.len() {
        return Err("the town came out otherwise laid out again".into());
    }
    let center = [s.x, s.y];
    let ch = edit.map(|e| super::wards::changes(&plan, e, center).0).unwrap_or_default();
    let li = index.to_string();
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut put = |key: String, v: Value| out.push((key, v));

    // Junctions: every corner of a patch.
    let mut corner: BTreeMap<usize, P> = BTreeMap::new();
    for (f, pts) in cap.faces.iter().zip(&cap.cells) {
        for (&v, &p) in f.iter().zip(pts) {
            corner.entry(v).or_insert(add(p, center));
        }
    }
    let squares: BTreeMap<usize, PlazaKind> = cap.squares.iter().copied().collect();
    for (&v, &p) in &corner {
        let j = Junction { p, square: squares.get(&v).map(|k| k.name().to_string()) };
        put(format!("{li}/n{v}"), serde_json::to_value(j).map_err(|e| e.to_string())?);
    }

    // Streets: every patch side, the setback of the patch on each side of it.
    let bridged: Vec<(usize, usize)> = plan.bridged.iter().flat_map(|&((a, r, b), _)| [(a.min(r), a.max(r)), (r.min(b), r.max(b))]).collect();
    let mut sides: BTreeMap<(usize, usize), (Option<(usize, f64)>, Option<(usize, f64)>)> = BTreeMap::new();
    for (i, f) in cap.faces.iter().enumerate() {
        let m = f.len();
        if m < 3 {
            continue;
        }
        let ccw = area(&cap.cells[i]) > 0.0;
        for e in 0..m {
            let (va, vb) = (f[e], f[(e + 1) % m]);
            let key = (va.min(vb), va.max(vb));
            let set = (i, cap.insets[i].get(e).copied().unwrap_or(0.0));
            let slot = sides.entry(key).or_default();
            // (A patch is on its side's left looking from the lower corner when it runs round
            // anticlockwise that way.)
            if ccw == (va == key.0) { slot.0 = Some(set) } else { slot.1 = Some(set) }
        }
    }
    for (k, (&(a, b), &(l, r))) in sides.iter().enumerate() {
        let (set_l, set_r) = (l.map_or(0.0, |x| x.1), r.map_or(0.0, |x| x.1));
        let joint = matches!((l, r), (Some((p, _)), Some((q, _))) if cap.group[p] != usize::MAX && cap.group[p] == cap.group[q]) && set_l == 0.0 && set_r == 0.0;
        let class = if joint {
            StreetClass::Joint
        } else if plan.main_edge(a, b) {
            StreetClass::Main
        } else if plan.shore.contains(&(a, b)) {
            StreetClass::Quay
        } else {
            StreetClass::Street
        };
        let st = Street { a: a as u32, b: b as u32, bends: Vec::new(), width: set_l + set_r, set_l, set_r, class, bridge: bridged.contains(&(a, b)) };
        put(format!("{li}/e{k}"), serde_json::to_value(st).map_err(|e| e.to_string())?);
    }

    // Blocks: every patch, its fill as its ward and settings have it, its buildings kept.
    let quarters = super::districts::of(&plan, &ch);
    let district = |i: usize| quarters.iter().position(|q| q.1.contains(&i)).and_then(|q| base.quarters.get(q)).map(|q| q.name.clone());
    let lot_name = |i: usize| ch.lots.get(&i).and_then(|f| super::wards::LOT_FACTOR.iter().position(|x| x == f)).map(|k| LOT_SIZES[k].to_string());
    let park_of = |i: usize| ch.park.get(&i).copied().or_else(|| base.parks.iter().find(|pk| pk.spot == super::places::Spot::Patch(i as u32)).map(|pk| pk.kind));
    // Which block each building stands in: those a fill laid out by the fill, the rest by where
    // they stand (else the town's own).
    let mut from_fill: FastMap<Vec<u64>, (usize, usize)> = FastMap::default();
    for (&i, (_, raw)) in &cap.fills {
        for (k, b) in raw.iter().enumerate() {
            from_fill.entry(to_world(b.clone(), center).poly.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]).collect()).or_insert((i, k));
        }
    }
    let roofs = roofs_of((index as u32, center, 0.0, s.tier), base.roof_seed);
    let patch_at = |p: P| (0..cap.cells.len()).find(|&i| cap.cells[i].len() >= 3 && contains(&cap.cells[i], sub(p, center)));
    let mut kept: BTreeMap<usize, Kept> = BTreeMap::new();
    let mut runs: BTreeMap<usize, Vec<(u32, u32)>> = BTreeMap::new();
    let mut used: crate::core::hash::FastSet<(usize, usize)> = Default::default();
    for b in &base.buildings {
        let bits: Vec<u64> = b.poly.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]).collect();
        let try_one = |i: usize, k: usize| (!used.contains(&(i, k))).then(|| over(&to_world(cap.fills[&i].1[k].clone(), center), b, &roofs).map(|o| (i, k, o))).flatten();
        // (Else a yurt's ring on the lot it stands in.)
        let c = mean(&b.poly);
        let matched = from_fill.get(&bits).and_then(|&(i, k)| try_one(i, k)).or_else(|| {
            let i = patch_at(c)?;
            let raw = &cap.fills.get(&i)?.1;
            (0..raw.len()).filter(|&k| contains(&raw[k].poly, sub(c, center))).find_map(|k| try_one(i, k))
        });
        match matched {
            Some((i, k, o)) => {
                used.insert((i, k));
                let kp = kept.entry(i).or_default();
                kp.floors.push((b'0' + b.floors) as char);
                if o != (Over { i: b.id, ..Over::default() }) {
                    kp.set.push(o);
                }
                runs.entry(i).or_default().push((k as u32, b.id));
            }
            None => {
                kept.entry(patch_at(c).unwrap_or(usize::MAX)).or_default().full.push(Full::of(b));
            }
        }
    }
    for (i, list) in runs {
        let kp = kept.entry(i).or_default();
        kp.refill = Some(cap.fills[&i].0.clone());
        for (k, id) in list {
            match kp.runs.last_mut() {
                Some(r) if r[0] + r[2] == k && r[1] + r[2] == id => r[2] += 1,
                _ => kp.runs.push([k, id, 1]),
            }
        }
    }
    for i in 0..cap.cells.len() {
        if cap.cells[i].len() < 3 {
            continue;
        }
        let w = cap.ward[i];
        let fill = match w {
            None => BlockFill::Hand,
            Some(Ward::Plaza) => {
                let k = super::fill::square_kind(plan.roll(i, 44), s.tier >= Tier::City);
                BlockFill::Open { open: if k.is_market() { OpenKind::Market } else { OpenKind::Plaza }, style: Some(k.name().into()) }
            }
            Some(Ward::Farm) => BlockFill::Open { open: OpenKind::Farmland, style: None },
            Some(Ward::Park) => {
                let k = park_of(i).unwrap_or(ParkKind::Common);
                let open = match k {
                    ParkKind::Formal => OpenKind::Garden,
                    ParkKind::Cemetery => OpenKind::Cemetery,
                    ParkKind::Common | ParkKind::Horses => OpenKind::Park,
                };
                BlockFill::Open { open, style: Some(k.name().into()) }
            }
            Some(Ward::Castle) => BlockFill::Castle { hand: plan.ward[i] != Some(Ward::Castle) },
            Some(w) => BlockFill::Generated { ward: w.name().into(), lots: lot_name(i), reroll: ch.reroll.get(&i).copied().unwrap_or(0), close: ch.close.get(&i).copied() },
        };
        let block = Block {
            at: add(inside_point(&cap.cells[i]), center),
            fill,
            district: district(i),
            seed: hex(hash3(s.seed, index as i64, i as i64, 0x4a4d)),
            generation: 0,
            pins: Vec::new(),
            kept: kept.remove(&i),
        };
        put(format!("{li}/k{i}"), serde_json::to_value(block).map_err(|e| e.to_string())?);
    }

    // Wall lines: each run of the town's wall, where it stands.
    if cap.walled {
        let pos = |v: usize| corner.get(&v).copied().unwrap_or_else(|| add(plan.mesh.pos[v], center));
        for (k, (chain, closed, towers)) in plan.wall_runs.iter().enumerate() {
            let pts: Vec<P> = chain.iter().map(|&v| pos(v)).collect();
            let gates: Vec<P> = cap.gates.iter().map(|g| add(*g, center)).filter(|g| chain.windows(2).any(|w| seg_dist(*g, pos(w[0]), pos(w[1])) < 1.0) || pts.iter().any(|p| dist(*p, *g) < 1.0)).collect();
            let wall = WallLine { pts, closed: *closed, spacing: base.wall_style.spacing, towers: towers.iter().map(|&t| t as u32).collect(), gates, name: None };
            put(format!("{li}/m{k}"), serde_json::to_value(wall).map_err(|e| e.to_string())?);
        }
    }

    // The header, with what stands in no block.
    let extent = base.buildings.iter().flat_map(|b| b.poly.iter()).chain(corner.values()).map(|p| dist(*p, center)).fold(0.0, f64::max);
    let town = HandTown {
        from: "generated".into(),
        was: edit.cloned(),
        seed: hex(s.seed),
        culture: s.culture,
        fill: BlockFill::default(),
        extent,
        origin: center,
        tier: s.tier as u8,
        reach: super::reach(&s) + 500.0,
        roofs: hex(base.roof_seed),
        kept: kept.remove(&usize::MAX),
    };
    debug_assert!(kept.is_empty(), "every block's buildings go with its block");
    put(li.clone(), serde_json::to_value(town).map_err(|e| e.to_string())?);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The harbour pieces a converted town's records leave to `Edits.wharves`: its piers (each a
/// rectangle: the middle of its short ends, its width) and breakwaters, as laid out now (key
/// `q:h<layout>-<n>`).
pub fn wharves(world: &World, t0: &T0, index: usize) -> Vec<(String, crate::world::Wharf)> {
    use crate::world::{CrossingMaterial, Wharf, WharfKind};
    let l = super::base_layout(world, t0, index);
    let mut out = Vec::new();
    for p in &l.piers {
        if p.len() != 4 {
            continue;
        }
        let (e0, e1) = (dist(p[0], p[1]), dist(p[1], p[2]));
        let (a, b, w) = if e0 <= e1 { (lerp(p[0], p[1], 0.5), lerp(p[2], p[3], 0.5), e0) } else { (lerp(p[1], p[2], 0.5), lerp(p[3], p[0], 0.5), e1) };
        out.push(Wharf { kind: WharfKind::Pier, pts: vec![a, b], width: w, material: CrossingMaterial::default() });
    }
    for m in &l.moles {
        out.push(Wharf { kind: WharfKind::Breakwater, pts: m.pts.clone(), width: m.width, material: CrossingMaterial::default() });
    }
    out.into_iter().enumerate().map(|(k, w)| (format!("q:h{index}-{k}"), w)).collect()
}

/// A town's records as they would be stored (`Edits.streets`, compact JSON): its size in bytes.
pub fn size_of(records: &[(String, Value)]) -> usize {
    records.iter().map(|(k, v)| k.len() + 4 + serde_json::to_string(v).map_or(0, |s| s.len())).sum()
}
