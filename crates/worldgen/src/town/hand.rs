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
use super::catalog::{self, CATALOG, Ward};
use super::fill::{District, Fill};
use super::geom::*;
use super::{Building, Layout, ParkKind, Part, PartKind, PlazaKind, QuarterKind, RoofStyle, Site, Structure, TownPlan};
use crate::World;
use crate::core::hash::FastMap;
use crate::core::rng::{Pcg32, hash2, hash3, unit};
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
    /// Its walls, gates, bridges and squares as converted (`Works`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub works: Option<Works>,
    /// The people its homes house (`people_of`), set by the change function after each change:
    /// what its size class follows (`size`). Not read by the builder (left out of `fingerprint`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub people: Option<u32>,
    /// A converted town's population as generated and the homes it had then (`people_of`'s
    /// count): its people are that population scaled by its homes since (`count_people`), so
    /// converting it never changes its size class. None for a blank town (counted per home).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub was_people: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub was_homes: Option<u32>,
    /// The lowest block number never yet given out: a deleted block's number is never used
    /// again (ids pinned from it can't meet a new block's). None: one past the highest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_block: Option<u32>,
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
    /// Lots of a ward (`TOWN_WARDS`), their size (`LOT_SIZES`; none: the ward's usual), their
    /// storeys (`HEIGHTS`; none: the ward's usual), rolled again `reroll` times, a temple's close
    /// round its temple or not (none: where it fits).
    Generated {
        ward: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lots: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<String>,
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
        BlockFill::Generated { ward: "common".into(), lots: None, height: None, reroll: 0, close: None }
    }
}

/// A building pinned to its place (`Block::pins`): what carries something (a name, notes) keeps
/// its id through a change of lots (made by the change function).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pin {
    pub p: P,
    pub id: u32,
    /// The business it was (`CATALOG` key) and its name: the building there carries them on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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

/// What a converted town's walls, gates, bridges and squares were as laid out (world ft, each list
/// of numbers packed by `pack`), kept while what they were laid from stands: the walls, towers,
/// gates and castles' curtains while the wall lines and castle blocks hash to `of`; a square at a
/// junction while that junction stands where it was with that square; a bridge while a street or
/// a kept approach runs over it; the roads in from the country, the squares of no block (quays) and
/// the districts' kinds always. (The header's `works`; laid anew from the records without it.)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Works {
    pub of: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub towers: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gates: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gate_towers: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gate_spots: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bastions: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dropped: u32,
    /// Castle curtains (x, y, r), square towers (x, y, dx, dy), tower shapes (x, y, shape, dx,
    /// dy), castles' towers set by hand (x, y, flanks a gate), mottes (x, y, top r, foot r, top,
    /// way x, y).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub castles: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub square: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub forms: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extra: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mottes: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bridges: Vec<String>,
    /// Roads in from the country: (class, width, line).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roads: Vec<(u8, f64, String)>,
    /// Squares at junctions: (junction, kind, outline).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corners: Vec<(u32, String, String)>,
    /// Squares of no block (quays): (kind, outline).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plazas: Vec<(String, String)>,
    /// Parks of no block (a palace's garden, a cemetery beside a road out): ("kind fronts key spot
    /// cemetery", outline), their ground planned again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parks: Vec<(String, String)>,
    /// The districts' kinds by name (`QuarterKind::name`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quarters: Vec<(String, String)>,
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
    /// Which of its farmland's plots (in the order its fill cuts them) the town kept as fields:
    /// `1` or `0` each (none: all).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fields: String,
}

/// Farmland's plots as `fill_block` cuts them, and their class.
type Plots = Vec<(Vec<P>, u8)>;

/// A fill run again (`fill_block`'s): `k` 0 lots, 1 farmland, 2 a square, 3 a park; the ward
/// (`WARDS` index; a square's kind, `PLAZAS`; a park's, `PARKS`), a temple's close, the farmland's
/// class; `g` its numbers packed (`pack`): the block's `nb` corners (local ft about the header's
/// `origin`), its `nl` street lines (normal, offset; a park's ways in), then for lots and
/// farmland the stream's state (two words), for lots the district's sizes (4), the close's roll,
/// and with `nt` the outskirts' thinning (its reach and `nt` corners); for a square its monument's
/// rolls (stages 40-43), for a park its looks' key (its bits).
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
const PLAZAS: [PlazaKind; 12] = [
    PlazaKind::Market,
    PlazaKind::MarketSparse,
    PlazaKind::MarketDay,
    PlazaKind::Civic,
    PlazaKind::Quay,
    PlazaKind::Green,
    PlazaKind::Yard,
    PlazaKind::TempleClose,
    PlazaKind::WellSquare,
    PlazaKind::FountainSquare,
    PlazaKind::Crossroads,
    PlazaKind::Bailey,
];
const PARKS: [ParkKind; 4] = [ParkKind::Formal, ParkKind::Common, ParkKind::Cemetery, ParkKind::Horses];
const KEEPS: [KeepKind; 6] = [KeepKind::Great, KeepKind::Hall, KeepKind::L, KeepKind::T, KeepKind::Shell, KeepKind::Plain];
const PARTS: [PartKind; 6] = [PartKind::Body, PartKind::Tower { round: false }, PartKind::Tower { round: true }, PartKind::Forebuilding, PartKind::Wing, PartKind::Apse];

fn index_of<T: PartialEq>(list: &[T], v: &T) -> u8 {
    list.iter().position(|x| x == v).expect("every kind is listed") as u8
}

fn nth<T: Copy>(list: &[T], i: u8, what: &str) -> Result<T, String> {
    list.get(i as usize).copied().ok_or_else(|| format!("kept building: no {what} {i}"))
}

/// Bytes as base64url without padding.
fn b64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 3);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..=c.len() {
            out.push(ABC[(n >> (18 - 6 * k) & 63) as usize] as char);
        }
    }
    out
}

/// `b64` undone.
fn unb64(s: &str) -> Result<Vec<u8>, String> {
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
    Ok(bytes)
}

/// Numbers packed exactly: each f64's bits, little-endian, base64url without padding.
pub fn pack(fs: &[f64]) -> String {
    b64(&fs.iter().flat_map(|f| f.to_bits().to_le_bytes()).collect::<Vec<u8>>())
}

/// `pack` undone.
pub fn unpack(s: &str) -> Result<Vec<f64>, String> {
    let bytes = unb64(s)?;
    if bytes.len() % 8 != 0 {
        return Err("packed numbers: not whole numbers".into());
    }
    Ok(bytes.chunks(8).map(|b| f64::from_bits(u64::from_le_bytes(b.try_into().unwrap()))).collect())
}

/// Points packed on a 1/64-ft grid (`~`, then each number's step from the last, zigzag varints,
/// base64url): a quarter of `pk`'s size, for long lines whose places need no more (walls, towers).
pub(super) fn pkq(pts: &[P]) -> String {
    if pts.is_empty() {
        return String::new();
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(pts.len() * 4);
    let mut last = [0i64; 2];
    for p in pts {
        for k in 0..2 {
            let v = crate::core::round(p[k] * 64.0) as i64;
            let d = v - last[k];
            last[k] = v;
            let mut z = ((d << 1) ^ (d >> 63)) as u64;
            loop {
                let b = (z & 127) as u8;
                z >>= 7;
                if z == 0 {
                    bytes.push(b);
                    break;
                }
                bytes.push(b | 128);
            }
        }
    }
    format!("~{}", b64(&bytes))
}

/// Towers' forms packed short (`~` centres `|` shapes, two hex digits each `|` ways, each on a
/// 1/4096 grid), or as `pack` (five numbers each) without the `~`.
fn forms_pk(forms: &[(P, u8, P)]) -> String {
    if forms.is_empty() {
        return String::new();
    }
    let at: Vec<P> = forms.iter().map(|f| f.0).collect();
    let ways: Vec<P> = forms.iter().map(|f| mul(f.2, 64.0)).collect();
    let shapes: String = forms.iter().map(|f| format!("{:02x}", f.1)).collect();
    format!("{}|{shapes}|{}", pkq(&at), pkq(&ways))
}

fn forms_unpk(s: &str) -> Vec<(P, u8, P)> {
    if !s.starts_with('~') {
        return unpack(s).unwrap_or_default().chunks_exact(5).map(|c| ([c[0], c[1]], c[2] as u8, [c[3], c[4]])).collect();
    }
    let mut it = s.split('|');
    let (at, shapes, ways) = (unpk(it.next().unwrap_or("")), it.next().unwrap_or(""), unpk(it.next().unwrap_or("")));
    let shapes: Vec<u8> = shapes.as_bytes().chunks(2).map(|c| std::str::from_utf8(c).ok().and_then(|h| u8::from_str_radix(h, 16).ok()).unwrap_or(0)).collect();
    at.into_iter().zip(shapes).zip(ways).map(|((a, k), w)| (a, k, mul(w, 1.0 / 64.0))).collect()
}

fn unpkq(s: &str) -> Vec<P> {
    let Ok(bytes) = unb64(s) else { return Vec::new() };
    let (mut nums, mut z, mut shift, mut last) = (Vec::new(), 0u64, 0, [0i64; 2]);
    for b in bytes {
        z |= ((b & 127) as u64) << shift;
        shift += 7;
        if b & 128 == 0 {
            let k = nums.len() % 2;
            last[k] += (z >> 1) as i64 ^ -((z & 1) as i64);
            nums.push(last[k] as f64 / 64.0);
            (z, shift) = (0, 0);
        }
    }
    nums.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
}

fn pts_of(fs: &[f64]) -> Vec<P> {
    fs.chunks(2).map(|c| [c[0], c[1]]).collect()
}

impl Refill {
    /// A fill as `build_town` hands it to `fill_block`, `roll` its patch's rolls.
    fn of(fill: &Fill, block: &[P], lines: &[(P, f64)], roll: &dyn Fn(i64) -> f64) -> Option<Refill> {
        let r65 = roll(65);
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
            Fill::Square(kind) => {
                let mut g: Vec<f64> = block.iter().flat_map(|p| *p).collect();
                g.extend((40..44).map(roll));
                Some(Refill { k: 2, w: index_of(&PLAZAS, kind), close: false, class: 0, nb, nl: 0, nt: None, g: pack(&g) })
            }
            Fill::Park { kind, ways, key, .. } => {
                let mut g: Vec<f64> = block.iter().flat_map(|p| *p).collect();
                g.extend(ways.iter().flat_map(|(n, c)| [n[0], n[1], *c]));
                g.push(f64::from_bits(*key));
                Some(Refill { k: 3, w: index_of(&PARKS, kind), close: false, class: 0, nb, nl: ways.len() as u32, nt: None, g: pack(&g) })
            }
        }
    }

    /// Its fill's buildings laid out again (local ft about the origin), as `fill_block` laid them
    /// out then.
    fn run(&self, site: &Site, meta: (u32, P, f64, Tier)) -> Result<Vec<Building>, String> {
        if self.k >= 2 {
            return Ok(Vec::new());
        }
        Ok(self.lay(site, meta, None)?.0.buildings)
    }

    /// Its block (local ft about the origin).
    pub(super) fn block(&self) -> Result<Vec<P>, String> {
        let g = unpack(&self.g)?;
        g.get(..2 * self.nb as usize).map(pts_of).ok_or_else(|| "kept block: corners cut short".to_string())
    }

    /// Its fill laid out again (local ft about the origin), as `fill_block` laid it out then: the
    /// layout it made and farmland's plots; a cemetery's style (its town's, `cemetery::Style::of`).
    pub(super) fn lay(&self, site: &Site, meta: (u32, P, f64, Tier), cemetery: Option<super::cemetery::Style>) -> Result<(Layout, Plots), String> {
        let g = unpack(&self.g)?;
        let (nb, nl) = (self.nb as usize, self.nl as usize);
        let need = 2 * nb + 3 * nl + match self.k {
            0 => 7 + self.nt.map_or(0, |t| 1 + 2 * t as usize),
            1 => 2,
            2 => 4,
            _ => 1,
        };
        if g.len() != need {
            return Err(format!("kept block: {} numbers packed, {need} wanted", g.len()));
        }
        let block = pts_of(&g[..2 * nb]);
        let lines: Vec<(P, f64)> = g[2 * nb..2 * nb + 3 * nl].chunks(3).map(|c| ([c[0], c[1]], c[2])).collect();
        let at = 2 * nb + 3 * nl;
        let rng = || Pcg32::from_state(g[at].to_bits(), g[at + 1].to_bits());
        let mut rolls = [0.0; 4];
        let (fill, r65) = match self.k {
            1 => (Fill::Farm { class: self.class, rng: rng() }, 0.0),
            2 => {
                rolls.copy_from_slice(&g[at..at + 4]);
                (Fill::Square(nth(&PLAZAS, self.w, "square")?), 0.0)
            }
            3 => {
                let kind = nth(&PARKS, self.w, "park")?;
                (Fill::Park { kind, ways: lines.clone(), key: g[at].to_bits(), cemetery: if kind == ParkKind::Cemetery { cemetery } else { None } }, 0.0)
            }
            0 => {
                let d = &g[at + 2..at + 7];
                let dp = District { min_sq: d[0], grid_chaos: d[1], size_chaos: d[2], block_size: d[3] };
                let thin = self.nt.map(|_| (pts_of(&g[at + 8..]), g[at + 7]));
                (Fill::Lots { ward: nth(&WARDS, self.w, "ward")?, dp, rng: rng(), thin, close: self.close }, d[4])
            }
            k => return Err(format!("kept block: no fill kind {k}")),
        };
        let mut l = Layout::empty(meta.0, meta.1, meta.2, meta.3);
        let mut plots = Vec::new();
        // (The open ground past a town's farmland: its plots alone.)
        if let Fill::Farm { class: 2, mut rng } = fill {
            super::fill::farm(site, &block, &mut rng, &mut l, &mut plots, 2, false);
            return Ok((l, plots));
        }
        let roll = |k: i64| if (40..44).contains(&k) { rolls[(k - 40) as usize] } else { r65 };
        super::fill_block(site, &block, lines, fill, &roll, super::places::Spot::default(), &mut l, &mut plots);
        Ok((l, plots))
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
        KeptGround::at(world, t0, index, town.origin, town.reach, Tier::from_u8(town.tier), u64::from_str_radix(&town.roofs, 16).unwrap_or(0))
    }

    fn at(world: &World, t0: &'a T0, index: usize, origin: P, reach: f64, tier: Tier, roof_seed: u64) -> KeptGround<'a> {
        let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
        let site = Site::new(t0, origin, lattice, reach, super::river_pieces(t0, origin, reach), world.params().sea_level_ft);
        let meta = (index as u32, origin, 0.0, tier);
        KeptGround { site, meta, roofs: roofs_of(meta, roof_seed) }
    }

    /// Farmland's plots laid out again by `r` (local ft), as the town's fields are cut from them
    /// (`fields::off_sand`), with what is left of each (none: all gone).
    fn plots(&self, r: &Refill) -> Vec<Option<Vec<P>>> {
        match r.lay(&self.site, self.meta, None) {
            Ok((_, plots)) => plots.iter().map(|(f, _)| super::fields::off_sand(&self.site, f)).collect(),
            Err(_) => Vec::new(),
        }
    }
}

fn roofs_of(meta: (u32, P, f64, Tier), roof_seed: u64) -> Layout {
    let mut l = Layout::empty(meta.0, meta.1, meta.2, meta.3);
    l.roof_seed = roof_seed;
    l
}

/// A kept block's buildings (world ft), in id order, from its record alone.
pub fn kept_on(ground: &KeptGround, kept: &Kept) -> Result<Vec<Building>, String> {
    let raw = match &kept.refill {
        Some(r) => r.run(&ground.site, ground.meta)?,
        None => Vec::new(),
    };
    kept_from(ground, kept, &raw)
}

/// `kept_on` with its fill's buildings laid out already (`raw`, local ft).
fn kept_from(ground: &KeptGround, kept: &Kept, raw: &[Building]) -> Result<Vec<Building>, String> {
    let origin = ground.meta.1;
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

pub(super) fn cap_fill(i: usize, fill: &Fill, block: &[P], lines: &[(P, f64)], roll: &dyn Fn(i64) -> f64) {
    with_cap(|c| c.pending = Refill::of(fill, block, lines, roll).map(|r| (i, r)))
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

/// A point on a 1/64-ft grid (written short, read back the same).
fn q64(p: P) -> P {
    [crate::core::round(p[0] * 64.0) / 64.0, crate::core::round(p[1] * 64.0) / 64.0]
}

pub(super) fn hex(v: u64) -> String {
    format!("{v:016x}")
}

/// A point well inside a polygon: its centroid if inside, else the middle of the widest chord
/// across it from one of its corners (its mean, failing those).
pub(super) fn inside_point(poly: &[P]) -> P {
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
    // (Every fill, though it keeps no building: a square's ground and monument, a park's, which of
    // farmland's plots are fields, laid out again.)
    let ground = KeptGround::at(world, t0, index, center, super::reach(&s) + 500.0, s.tier, base.roof_seed);
    let mut fields: FastMap<Vec<u64>, u32> = FastMap::default();
    for f in &base.fields {
        *fields.entry(f.iter().flat_map(|p| [p[0].to_bits(), p[1].to_bits()]).collect()).or_default() += 1;
    }
    for (&i, (r, _)) in &cap.fills {
        let kp = kept.entry(i).or_default();
        kp.refill = Some(r.clone());
        if r.k == 1 {
            for p in ground.plots(r) {
                let bits: Option<Vec<u64>> = p.map(|p| p.iter().flat_map(|q| { let w = add(*q, center); [w[0].to_bits(), w[1].to_bits()] }).collect());
                let hit = bits.and_then(|b| fields.get_mut(&b)).filter(|n| **n > 0).map(|n| *n -= 1).is_some();
                kp.fields.push(if hit { '1' } else { '0' });
            }
            if !kp.fields.contains('0') {
                kp.fields.clear();
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
            Some(w) => BlockFill::Generated { ward: w.name().into(), lots: lot_name(i), height: None, reroll: ch.reroll.get(&i).copied().unwrap_or(0), close: ch.close.get(&i).copied() },
        };
        let open = !matches!(fill, BlockFill::Hand);
        let block = Block {
            at: q64(add(inside_point(&cap.cells[i]), center)),
            fill,
            district: district(i),
            seed: hex(hash3(s.seed, index as i64, i as i64, 0x4a4d)),
            generation: 0,
            pins: Vec::new(),
            // (Nothing kept but what its fill made: it stays so until its lots next change.)
            kept: kept.remove(&i).or_else(|| open.then(Kept::default)),
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
        works: None,
        people: Some(s.population),
        was_people: Some(s.population),
        was_homes: Some(people_of(&base)),
        next_block: None,
    };
    debug_assert!(kept.is_empty(), "every block's buildings go with its block");
    // (And its walls, gates, bridges and squares as they are, while what they stand on stands.)
    let recs: BTreeMap<String, Value> = out.iter().cloned().collect();
    let town = HandTown { works: Some(works_from(&base, works_of(&HandRecords::of(&recs, index)), &corner)), ..town };
    out.push((li.clone(), serde_json::to_value(town).map_err(|e| e.to_string())?));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// A plot drawn by hand (`Created` kind `plot`, layout `index`): its outline, about its point,
/// filled as a hand town's block is by its fill (`Created::fill`; none: common lots) fronting no
/// street, seeded by its id; businesses named as a hand town's are. Its buildings number from 0
/// as a site's do. The nearest settlement gives a cemetery its looks.
pub fn plot_layout(world: &World, t0: &T0, index: usize, c: &crate::world::Created) -> Layout {
    use super::fill::{Block as Region, build_block};
    let origin = [c.x, c.y];
    let reach = c.reach();
    let tier = Tier::Town;
    let ground = KeptGround::at(world, t0, index, origin, reach, tier, 0);
    let site = &ground.site;
    let mut l = Layout::empty(index as u32, origin, reach, tier);
    l.set_roofs(world, t0, c.culture.unwrap_or_else(|| crate::t0::made::culture_at(t0, c.x, c.y)));
    let poly: Vec<P> = c.poly.iter().map(|p| sub(*p, origin)).collect();
    let g = super::clip::region(&poly);
    let seed = crate::core::hash::fnv64(c.id.as_bytes()) ^ world.seed;
    let near = |s: &&crate::t0::settle::Settlement| dist([s.x, s.y], origin);
    let Some(s) = t0.settlements.iter().min_by(|a, b| near(a).total_cmp(&near(b))) else { return l };
    match c.fill.clone().unwrap_or_default() {
        BlockFill::Hand | BlockFill::Castle { .. } => {}
        BlockFill::Generated { ward, lots, height, reroll, .. } => {
            let Some(w) = ward_of(&ward) else { return l };
            let salt = |h: u64| if reroll == 0 { h } else { hash3(h, reroll as i64, 0x7e70, 1) };
            let mut dp = District::roll(w, salt(hash2(seed, 0, 0xd15)));
            if let Some(k) = lots.as_deref().and_then(|n| LOT_SIZES.iter().position(|x| *x == n)) {
                dp.min_sq = District::base(w) * super::wards::LOT_FACTOR[k];
            }
            let mut rng = Pcg32::new(salt(hash2(seed, 0, 0x107)), 47);
            l.blocks.extend(g.iter().map(|x| x.outer.clone()));
            build_block(site, Region::Region(&g), &[], w, &dp, &mut rng, &mut l, None);
            home_storeys(&mut l, 0, w, tier, hash2(salt(seed), 0, 0x5707), &[]);
            businesses(&mut l, 0, w, tier, false, hash2(salt(seed), 0, 0xb05), &[]);
            height_storeys(&mut l, 0, height.as_deref(), &[]);
        }
        BlockFill::Open { open, style } => {
            let mut pieces: Vec<(f64, usize)> = g.iter().enumerate().map(|(k, x)| (x.area(), k)).filter(|x| x.0 >= 200.0).collect();
            pieces.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
            for (pi, &(_, k)) in pieces.iter().enumerate() {
                let salt = |h: u64| if pi == 0 { h } else { hash2(h, pi as i64, 0x91ec) };
                let roll = |j: i64| unit(salt(hash3(seed, 0, j, 0x09e)));
                let cem = || super::cemetery::Style::of(s, index, false);
                let fill = open_fill(open, style.as_deref(), &[], salt(hash2(seed, 0, 0x9a4c)), cem, &mut || Pcg32::new(salt(hash2(seed, 0, 0xfa)), 61), tier);
                let mut plots = Vec::new();
                super::fill_block(site, &g[k].outer, Vec::new(), fill, &roll, super::places::Spot::None, &mut l, &mut plots);
            }
        }
    }
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (k, b) in l.buildings.iter_mut().enumerate() {
        b.id = k as u32;
        if b.func.is_some() && b.name.is_none() {
            for salt in 0..8 {
                let mut rng = Pcg32::new(hash3(seed, k as i64, 0x4e + salt, 0xb7), 43);
                b.name = super::sites::trade_name(b.func, &mut rng, t0, origin, names.iter().map(String::as_str));
                if b.name.as_ref().is_none_or(|n| !names.contains(n)) {
                    break;
                }
            }
            names.extend(b.name.clone());
        }
    }
    l
}

/// People a home of each kind (`catalog::RESIDENTIAL`: hovel, house, townhouse, tenement, noble
/// estate, farmhouse, wizard's tower) houses, counting servants, lodgers and the trades that live
/// over their shops.
const PEOPLE_PER_HOME: [u32; 7] = [4, 5, 8, 23, 14, 5, 2];

/// The people a layout's homes house (a hand town's population: `HandTown::people`).
pub fn people_of(l: &Layout) -> u32 {
    l.buildings.iter().filter(|b| b.func.is_none()).map(|b| PEOPLE_PER_HOME.get(b.residential as usize).copied().unwrap_or(5)).sum()
}

/// A hand town's people as layout `l` (its homes now) houses them: a converted town's population
/// as generated scaled by its homes now against its homes then; a blank town's per home.
pub fn count_people(town: &HandTown, l: &Layout) -> u32 {
    let now = people_of(l);
    match (town.was_people, town.was_homes) {
        (Some(p), Some(h)) if h > 0 => ((p as u64 * now as u64 + h as u64 / 2) / h as u64).min(u32::MAX as u64) as u32,
        _ => now,
    }
}

/// The size class a population makes (`t0::settle::pop_range`'s floors).
pub fn tier_of_people(people: u32) -> Tier {
    let floor = |t: Tier| crate::t0::settle::pop_range(t).0 as u32;
    [Tier::Metropolis, Tier::City, Tier::Town].into_iter().find(|&t| people >= floor(t)).unwrap_or(Tier::Village)
}

/// A hand town's population (its homes', as last counted) and the size class it makes: what its
/// label, icon and lists show at once (T0's roads and trade catch up at the next remake). None
/// for a layout not laid out by hand or not yet counted.
pub fn size(world: &World, index: usize) -> Option<(u32, Tier)> {
    let people = world.file.edits.streets.get(&index.to_string())?.get("people")?.as_u64()? as u32;
    Some((people, tier_of_people(people)))
}

/// The records of settlement `index` laid out blank (Roadmap 6, the user's call): a header (`from`
/// "blank", common lots for the blocks its streets will make) and the roads that reach it running
/// in as streets to one junction at its centre, nothing else: no blocks, no buildings. Streets
/// drawn later (`add_street`) make its blocks.
pub fn blank(world: &World, t0: &T0, index: usize) -> Result<Vec<(String, Value)>, String> {
    let s = super::culture::settlement(world, t0, index).ok_or_else(|| format!("layout {index}: not a settlement"))?;
    let li = index.to_string();
    let center = [s.x, s.y];
    let mut out: Vec<(String, Value)> = Vec::new();
    out.push((format!("{li}/n0"), serde_json::to_value(Junction { p: center, square: None }).map_err(|e| e.to_string())?));
    for (k, (e, class)) in super::ends_of(t0, index).into_iter().enumerate() {
        let p = add(e, center);
        if dist(p, center) < 20.0 {
            continue;
        }
        let n = k as u32 + 1;
        // (A road's street as wide as a generated town's main streets run, a track's narrower.)
        let w = if class >= crate::t0::roads::RoadClass::Road as u8 { 14.0 } else { 10.0 };
        out.push((format!("{li}/n{n}"), serde_json::to_value(Junction { p, square: None }).map_err(|e| e.to_string())?));
        let st = Street { a: n, b: 0, bends: Vec::new(), width: w, set_l: 0.5 * w, set_r: 0.5 * w, class: StreetClass::Main, bridge: false };
        out.push((format!("{li}/e{n}"), serde_json::to_value(st).map_err(|e| e.to_string())?));
    }
    let town = HandTown {
        from: "blank".into(),
        was: None,
        seed: hex(s.seed),
        culture: s.culture,
        fill: BlockFill::default(),
        extent: super::urban_radius(s.tier, s.population),
        origin: center,
        tier: s.tier as u8,
        reach: super::reach(&s) + 500.0,
        roofs: String::new(),
        kept: None,
        works: None,
        people: Some(0),
        was_people: None,
        was_homes: None,
        next_block: None,
    };
    out.push((li, serde_json::to_value(town).map_err(|e| e.to_string())?));
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

// ---------------------------------------------------------------------------------------
// Walls, gates, bridges and squares.

const QUARTERS: [QuarterKind; 9] =
    [QuarterKind::Center, QuarterKind::Castle, QuarterKind::Docks, QuarterKind::Bridge, QuarterKind::Gate, QuarterKind::Bank, QuarterKind::Park, QuarterKind::Sprawl, QuarterKind::Regular];

pub(super) fn pk(pts: &[P]) -> String {
    pack(&pts.iter().flat_map(|p| [p[0], p[1]]).collect::<Vec<f64>>())
}

pub(super) fn unpk(s: &str) -> Vec<P> {
    if let Some(q) = s.strip_prefix('~') {
        return unpkq(q);
    }
    unpack(s).map(|f| pts_of(&f)).unwrap_or_default()
}

pub(super) fn plaza_kind(name: &str) -> Option<PlazaKind> {
    PLAZAS.iter().copied().find(|k| k.name() == name)
}

/// A hash (hex) of what a town's walls are laid from: its wall lines and its castle blocks.
pub(super) fn works_of(recs: &HandRecords) -> String {
    let mut h = crate::core::hash::Fnv64::default();
    for (id, w) in &recs.walls {
        h.write(&id.to_le_bytes());
        h.write(serde_json::to_string(w).unwrap_or_default().as_bytes());
    }
    for (id, b) in recs.blocks.iter().filter(|(_, b)| matches!(b.fill, BlockFill::Castle { .. })) {
        h.write(&id.to_le_bytes());
        h.write(serde_json::to_string(&b.fill).unwrap_or_default().as_bytes());
        h.write(b.seed.as_bytes());
        h.write(&[b.generation as u8, b.kept.is_some() as u8, 0xfe]);
    }
    hex(h.finish())
}

/// A town's works as laid out in `l` (world ft), with junction `corner[v]` where each square at a
/// junction opens.
fn works_from(l: &Layout, of: String, corner: &BTreeMap<usize, P>) -> Works {
    use super::places::Spot;
    let flat = |it: &mut dyn Iterator<Item = f64>| pack(&it.collect::<Vec<f64>>());
    Works {
        of,
        walls: l.walls.iter().map(|w| pkq(w)).collect(),
        towers: pkq(&l.towers),
        gates: pk(&l.gates),
        gate_towers: pkq(&l.gate_towers),
        gate_spots: l.gate_spots.clone(),
        bastions: l.bastions.iter().map(|b| pk(b)).collect(),
        dropped: l.bastions_dropped,
        castles: flat(&mut l.castles.iter().flat_map(|c| [c.0[0], c.0[1], c.1])),
        square: flat(&mut l.square_towers.iter().flat_map(|t| [t.0[0], t.0[1], t.1[0], t.1[1]])),
        forms: forms_pk(&l.tower_forms),
        extra: flat(&mut l.extra_towers.iter().flat_map(|t| [t.0[0], t.0[1], t.1 as u8 as f64])),
        mottes: flat(&mut l.mottes.iter().flat_map(|m| [m.at[0], m.at[1], m.top_r, m.foot_r, m.top as f64, m.way[0], m.way[1]])),
        bridges: l.bridges.iter().map(|b| pkq(b)).collect(),
        roads: l.roads.iter().filter(|r| r.1 <= 2).map(|r| (r.1, r.2, pkq(&r.0))).collect(),
        corners: l
            .plazas
            .iter()
            .filter_map(|p| match p.spot {
                // (The junction's point first: the square stands while the junction does.)
                Spot::Corner(v) => corner.get(&(v as usize)).map(|j| (v, p.kind.name().to_string(), pk(&[&[*j][..], &p.pts[..]].concat()))),
                _ => None,
            })
            .collect(),
        plazas: l.plazas.iter().filter(|p| !matches!(p.spot, Spot::Patch(_) | Spot::Corner(_))).map(|p| (p.kind.name().to_string(), pk(&p.pts))).collect(),
        parks: l
            .parks
            .iter()
            .filter_map(|p| {
                let spot = match p.spot {
                    Spot::Garden(g) => format!("g{g}"),
                    Spot::Road(r) => format!("r{r}"),
                    Spot::None => "n".into(),
                    _ => return None,
                };
                let fronts: String = p.fronts.iter().map(|f| if *f { '1' } else { '0' }).collect();
                let cem = p.cemetery.map_or("-".into(), |c| format!("{}.{}.{}.{}.{}", c.group, c.rich as u8, c.city as u8, c.patch, c.by_road as u8));
                Some((format!("{} {} {:x} {spot} {cem}", p.kind.name(), if fronts.is_empty() { "-".into() } else { fronts }, p.key), pk(&p.pts)))
            })
            .collect(),
        quarters: l.quarters.iter().map(|q| (q.name.clone(), q.kind.name().to_string())).collect(),
    }
}

/// A park of no block as `works_from` wrote it down, planned again on `site` about `origin`.
fn park_of(site: &Site, origin: P, meta: &str, pts: &str) -> Option<super::Park> {
    use super::places::Spot;
    let f: Vec<&str> = meta.split(' ').collect();
    let (kind, fronts, key, spot, cem) = (ParkKind::parse(f.first()?)?, *f.get(1)?, u64::from_str_radix(f.get(2)?, 16).ok()?, *f.get(3)?, *f.get(4)?);
    let fronts: Vec<bool> = if fronts == "-" { Vec::new() } else { fronts.bytes().map(|c| c == b'1').collect() };
    let world_pts = unpk(pts);
    let local: Vec<P> = world_pts.iter().map(|p| sub(*p, origin)).collect();
    let mut pk = super::park(site, &local, kind, fronts, key);
    pk.pts = world_pts;
    pk.ground.origin = add(pk.ground.origin, origin);
    pk.spot = match (spot.as_bytes().first(), spot.get(1..).and_then(|n| n.parse::<u32>().ok())) {
        (Some(b'g'), Some(n)) => Spot::Garden(n),
        (Some(b'r'), Some(n)) => Spot::Road(n),
        _ => Spot::None,
    };
    if cem != "-" {
        let c: Vec<u32> = cem.split('.').filter_map(|x| x.parse().ok()).collect();
        if c.len() == 5 {
            pk.cemetery = Some(super::cemetery::Style { group: c[0] as u8, rich: c[1] != 0, city: c[2] != 0, patch: c[3], by_road: c[4] != 0 });
        }
    }
    Some(pk)
}

/// Whether the line `a`-`b` runs over `poly` (crossing it, or an end in it).
pub(super) fn runs_over(poly: &[P], a: P, b: P) -> bool {
    super::overlaps_or_crosses(poly, &[a, b])
}

/// Where the street `c`-`d` crosses the wall's stretch `a`-`b` (its share along the stretch), if
/// it crosses it outright.
fn crossing(a: P, b: P, c: P, d: P) -> Option<f64> {
    let (d1, d2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
    let (d3, d4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
    (d1 * d2 < 0.0 && d3 * d4 < 0.0).then(|| d3 / (d3 - d4))
}

/// The wall lines laid anew (town-local ft): each line's corners, a gate where a street crosses it
/// or one of its `gates` stands (at a corner within 30 ft, else on the stretch), towers by
/// `wall_pieces_spaced` (its record's counts per stretch while they fit, else every `spacing`
/// ft); broken at the river. Returns (pieces, towers, gate towers, gates).
fn lay_walls(site: &Site, recs: &HandRecords, streets: &[(P, P)], origin: P, spacing: f64) -> (Vec<Vec<P>>, Vec<P>, Vec<P>, Vec<P>) {
    const SNAP: f64 = 30.0;
    let mut tmp = Layout::empty(0, [0.0, 0.0], 0.0, Tier::Town);
    let mut gates_at: Vec<P> = Vec::new();
    for w in recs.walls.values() {
        let pts: Vec<P> = w.pts.iter().map(|p| sub(*p, origin)).collect();
        if pts.len() < 2 {
            continue;
        }
        let gates: Vec<P> = w.gates.iter().map(|p| sub(*p, origin)).collect();
        let n = pts.len();
        let stretches = if w.closed { n } else { n - 1 };
        let mut at_corner = vec![false; n];
        let mut on: Vec<Vec<f64>> = vec![Vec::new(); stretches];
        let put = |e: usize, t: f64, at_corner: &mut Vec<bool>, on: &mut Vec<Vec<f64>>| {
            let (a, b) = (pts[e], pts[(e + 1) % n]);
            let el = dist(a, b);
            if t * el < SNAP {
                at_corner[e] = true;
            } else if (1.0 - t) * el < SNAP {
                at_corner[(e + 1) % n] = true;
            } else {
                on[e].push(t);
            }
        };
        for g in &gates {
            if let Some(k) = pts.iter().position(|p| dist(*p, *g) < SNAP) {
                at_corner[k] = true;
            } else if let Some(e) = (0..stretches).find(|&e| seg_dist(*g, pts[e], pts[(e + 1) % n]) < 2.0) {
                let (a, b) = (pts[e], pts[(e + 1) % n]);
                put(e, dot(sub(*g, a), sub(b, a)) / dot(sub(b, a), sub(b, a)).max(1e-9), &mut at_corner, &mut on);
            }
        }
        for e in 0..stretches {
            let (a, b) = (pts[e], pts[(e + 1) % n]);
            for &(c, d) in streets {
                if let Some(t) = crossing(a, b, c, d) {
                    put(e, t, &mut at_corner, &mut on);
                }
            }
        }
        // (A closed ring starts at a plain corner, so no gate falls on its seam.)
        let start = if w.closed { (0..n).find(|&k| !at_corner[k]).unwrap_or(0) } else { 0 };
        let mut corners: Vec<(P, bool)> = Vec::new();
        let mut plain = true;
        for j in 0..n {
            let k = (start + j) % n;
            corners.push((pts[k], at_corner[k]));
            if k < stretches && (w.closed || j + 1 < n) {
                let mut ts = std::mem::take(&mut on[k]);
                ts.sort_by(f64::total_cmp);
                ts.dedup_by(|x, y| (*x - *y) * dist(pts[k], pts[(k + 1) % n]) < SNAP);
                plain &= ts.is_empty();
                corners.extend(ts.iter().map(|&t| (lerp(pts[k], pts[(k + 1) % n], t), true)));
            }
        }
        gates_at.extend(corners.iter().filter(|c| c.1).map(|c| c.0));
        let counts: Vec<usize> = w.towers.iter().map(|&t| t as usize).collect();
        let along = (plain && start == 0 && !counts.is_empty()).then_some(counts.as_slice());
        let (pieces, towers, gate_towers, _) = super::wall_pieces_spaced(&corners, w.closed, along, if w.spacing > 0.0 { w.spacing } else { spacing });
        tmp.walls.extend(pieces);
        tmp.towers.extend(towers);
        tmp.gate_towers.extend(gate_towers);
    }
    super::walls_off_river(site, &mut tmp, &[]);
    (tmp.walls, tmp.towers, tmp.gate_towers, gates_at)
}

/// A square opening at a junction laid anew (town-local ft): an octagon reaching past the
/// setbacks of the streets that meet there.
fn junction_square(at: P, reach: f64) -> Vec<P> {
    let r = reach + 24.0;
    (0..8)
        .map(|k| {
            let a = (k as f64 + 0.5) * std::f64::consts::FRAC_PI_4;
            add(at, [r * libm::cos(a), r * libm::sin(a)])
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// The builder: a hand town laid out from its records.

/// Ids of the buildings a hand town's blocks lay out: `HAND_ID + (block × ROLLS + gen mod ROLLS)
/// × PER_PATCH + k` (k in the order the block builds them), above every id a kept block keeps
/// (`wards::ROLLED_ID`'s range); a row's take block number `ROW_BLOCK + row`. Blocks under
/// `ROW_BLOCK` and rows under `MAX_ROWS` have room.
pub const HAND_ID: u32 = 2_000_000_000;
pub const ROW_BLOCK: u32 = 3_000;
pub const MAX_ROWS: u32 = 350;
use super::wards::{PER_PATCH, ROLLS};

/// What of a hand town's records could not be laid out: (key, why).
pub type Problems = Vec<(String, &'static str)>;

/// The street graph: corners (junctions and bends, local ft) and the street pieces between them.
pub(super) struct Graph {
    pub(super) pts: Vec<P>,
    q: Vec<(i64, i64)>,
    /// (from, to, street id): from `a` toward `b`.
    pub(super) segs: Vec<(usize, usize, u32)>,
    /// The pieces of streets left out as dead ends (no block beside them), in the same form: drawn
    /// all the same (a blank town's first streets), never a block's side.
    pub(super) loose: Vec<(usize, usize, u32)>,
}

/// A face of the graph: its ring (corners, anticlockwise), and per side the street piece and
/// whether the face is on its left.
pub(super) struct Face {
    pub(super) ring: Vec<usize>,
    pub(super) side: Vec<(usize, bool)>,
}

fn grid(p: P) -> (i64, i64) {
    (crate::core::round(p[0] * 256.0) as i64, crate::core::round(p[1] * 256.0) as i64)
}

fn orient(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> i128 {
    (b.0 - a.0) as i128 * (c.1 - a.1) as i128 - (b.1 - a.1) as i128 * (c.0 - a.0) as i128
}

/// Whether c lies on segment a-b strictly between its ends (exactly, by the grid).
fn between(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> bool {
    orient(a, b, c) == 0 && c != a && c != b && (c.0 - a.0) as i128 * (c.0 - b.0) as i128 + (c.1 - a.1) as i128 * (c.1 - b.1) as i128 <= 0
}

impl Graph {
    /// The graph of `recs`' streets (local ft about `origin`), the streets that can't stand in it
    /// left out and said why: a missing junction, no length, crossing a street of a lower id (or
    /// touching it, or running along it), or ending nowhere (a dead end: no block beside it).
    pub(super) fn of(recs: &HandRecords, origin: P, li: &str, problems: &mut Problems) -> Graph {
        let mut g = Graph { pts: Vec::new(), q: Vec::new(), segs: Vec::new(), loose: Vec::new() };
        let mut at: FastMap<(i64, i64), usize> = FastMap::default();
        let mut corner = |g: &mut Graph, p: P| {
            let k = grid(p);
            *at.entry(k).or_insert_with(|| {
                g.pts.push(p);
                g.q.push(k);
                g.pts.len() - 1
            })
        };
        // Each street's pieces, in id order.
        let mut pieces: BTreeMap<u32, Vec<(usize, usize)>> = BTreeMap::new();
        for (&id, st) in &recs.streets {
            let (Some(a), Some(b)) = (recs.junctions.get(&st.a), recs.junctions.get(&st.b)) else {
                problems.push((format!("{li}/e{id}"), "no such junction"));
                continue;
            };
            let line: Vec<usize> = std::iter::once(a.p).chain(st.bends.iter().copied()).chain([b.p]).map(|p| corner(&mut g, sub(p, origin))).collect();
            let mut ps: Vec<(usize, usize)> = line.windows(2).filter(|w| w[0] != w[1]).map(|w| (w[0], w[1])).collect();
            // (A piece doubling straight back on the last is no street.)
            if ps.is_empty() || ps.windows(2).any(|w| w[0].0 == w[1].1) {
                problems.push((format!("{li}/e{id}"), "no length"));
                ps.clear();
            }
            if !ps.is_empty() {
                pieces.insert(id, ps);
            }
        }
        // Crossings: every pair of pieces of different streets meeting other than at a shared corner,
        // found by a sweep along x; the street of the higher id goes.
        let all: Vec<(usize, usize, u32)> = pieces.iter().flat_map(|(&id, ps)| ps.iter().map(move |&(u, v)| (u, v, id))).collect();
        let bx = |s: &(usize, usize, u32)| (g.q[s.0].0.min(g.q[s.1].0), g.q[s.0].0.max(g.q[s.1].0));
        let mut order: Vec<usize> = (0..all.len()).collect();
        order.sort_by_key(|&k| (bx(&all[k]).0, k));
        let mut meets: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (oi, &i) in order.iter().enumerate() {
            let (x1, (u, v, si)) = (bx(&all[i]).1, all[i]);
            let (a, b) = (g.q[u], g.q[v]);
            for &j in &order[oi + 1..] {
                if bx(&all[j]).0 > x1 {
                    break;
                }
                let (c, d, sj) = all[j];
                if si == sj {
                    continue;
                }
                let (pc, pd) = (g.q[c], g.q[d]);
                if a.1.max(b.1) < pc.1.min(pd.1) || pc.1.max(pd.1) < a.1.min(b.1) {
                    continue;
                }
                if conflict((u, v), (c, d), a, b, pc, pd) {
                    meets.entry(si.max(sj)).or_default().push(si.min(sj));
                }
            }
        }
        let mut kept: BTreeMap<u32, Vec<(usize, usize)>> = BTreeMap::new();
        for (id, ps) in pieces {
            if meets.get(&id).is_some_and(|m| m.iter().any(|o| kept.contains_key(o))) {
                problems.push((format!("{li}/e{id}"), "crosses another street"));
            } else {
                kept.insert(id, ps);
            }
        }
        // Dead ends, until none is left.
        loop {
            let mut deg = vec![0u32; g.pts.len()];
            for ps in kept.values() {
                for &(u, v) in ps {
                    deg[u] += 1;
                    deg[v] += 1;
                }
            }
            let dead: Vec<u32> = kept.iter().filter(|(_, ps)| ps.iter().any(|&(u, v)| deg[u] < 2 || deg[v] < 2)).map(|(&id, _)| id).collect();
            if dead.is_empty() {
                break;
            }
            for id in dead {
                if let Some(ps) = kept.remove(&id) {
                    g.loose.extend(ps.into_iter().map(|(u, v)| (u, v, id)));
                }
                problems.push((format!("{li}/e{id}"), "a dead end: no block beside it"));
            }
        }
        g.segs = kept.iter().flat_map(|(&id, ps)| ps.iter().map(move |&(u, v)| (u, v, id))).collect();
        g
    }

    /// Its faces by a walk round each half piece (each corner's pieces in exact angle order),
    /// the bounded ones (anticlockwise) and each part's outer edge (clockwise).
    fn faces(&self) -> (Vec<Face>, Vec<Face>) {
        let n = self.pts.len();
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        // (Half piece 2k runs from the piece's first corner, 2k + 1 back.)
        let from = |h: usize| if h.is_multiple_of(2) { self.segs[h / 2].0 } else { self.segs[h / 2].1 };
        let to = |h: usize| from(h ^ 1);
        for h in 0..2 * self.segs.len() {
            out[from(h)].push(h);
        }
        let dir = |h: usize| {
            let (a, b) = (self.q[from(h)], self.q[to(h)]);
            (b.0 - a.0, b.1 - a.1)
        };
        let half = |d: (i64, i64)| if d.1 > 0 || (d.1 == 0 && d.0 > 0) { 0 } else { 1 };
        for o in &mut out {
            o.sort_by(|&x, &y| {
                let (dx, dy) = (dir(x), dir(y));
                half(dx).cmp(&half(dy)).then_with(|| 0.cmp(&((dx.0 as i128) * (dy.1 as i128) - (dx.1 as i128) * (dy.0 as i128)))).then(x.cmp(&y))
            });
        }
        let mut pos = vec![0usize; 2 * self.segs.len()];
        for o in &out {
            for (k, &h) in o.iter().enumerate() {
                pos[h] = k;
            }
        }
        let mut seen = vec![false; 2 * self.segs.len()];
        let (mut bounded, mut outer) = (Vec::new(), Vec::new());
        for h0 in 0..2 * self.segs.len() {
            if seen[h0] {
                continue;
            }
            let mut f = Face { ring: Vec::new(), side: Vec::new() };
            let mut h = h0;
            let mut twice = 0i128;
            while !seen[h] {
                seen[h] = true;
                f.ring.push(from(h));
                f.side.push((h / 2, h.is_multiple_of(2)));
                let (a, b) = (self.q[from(h)], self.q[to(h)]);
                twice += a.0 as i128 * b.1 as i128 - b.0 as i128 * a.1 as i128;
                let o = &out[to(h)];
                h = o[(pos[h ^ 1] + o.len() - 1) % o.len()];
            }
            if twice > 0 { bounded.push(f) } else { outer.push(f) }
        }
        (bounded, outer)
    }
}

/// The graph of a town's streets with its faces: their rings (local ft), the parts held inside
/// each as holes (`outers`' indices), and the block each face is (the lowest-id block whose `at` it
/// holds, outside its holes, not taken by another face).
pub(super) struct Faces {
    pub(super) graph: Graph,
    pub(super) faces: Vec<Face>,
    pub(super) outers: Vec<Face>,
    pub(super) rings: Vec<Vec<P>>,
    pub(super) holes: Vec<Vec<usize>>,
    pub(super) face_block: Vec<Option<u32>>,
}

impl Faces {
    pub(super) fn of(recs: &HandRecords, origin: P, li: &str, problems: &mut Problems) -> Faces {
        let graph = Graph::of(recs, origin, li, problems);
        let (faces, outers) = graph.faces();
        let ring_pts = |f: &Face| f.ring.iter().map(|&v| graph.pts[v]).collect::<Vec<P>>();
        // Each part's outer edge is a hole in the smallest face of another part round it.
        let mut part = (0..graph.pts.len()).collect::<Vec<usize>>();
        fn root(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for &(u, v, _) in &graph.segs {
            let (a, b) = (root(&mut part, u), root(&mut part, v));
            part[a.max(b)] = a.min(b);
        }
        let rings: Vec<Vec<P>> = faces.iter().map(ring_pts).collect();
        let areas: Vec<f64> = rings.iter().map(|r| area(r)).collect();
        let mut holes: Vec<Vec<usize>> = vec![Vec::new(); faces.len()];
        for (k, o) in outers.iter().enumerate() {
            let p0 = graph.pts[o.ring[0]];
            let me = root(&mut part, o.ring[0]);
            let around = (0..faces.len()).filter(|&f| root(&mut part, faces[f].ring[0]) != me && contains(&rings[f], p0)).min_by(|&x, &y| areas[x].total_cmp(&areas[y]).then(x.cmp(&y)));
            if let Some(f) = around {
                holes[f].push(k);
            }
        }
        // Faces to blocks.
        let mut taken: BTreeMap<u32, usize> = BTreeMap::new();
        let mut face_block: Vec<Option<u32>> = vec![None; faces.len()];
        for (f, ring) in rings.iter().enumerate() {
            let inside = |p: P| contains(ring, p) && holes[f].iter().all(|&h| !contains(&ring_pts(&outers[h]), p));
            if let Some((&id, _)) = recs.blocks.iter().find(|(id, b)| !taken.contains_key(id) && inside(sub(b.at, origin))) {
                taken.insert(id, f);
                face_block[f] = Some(id);
            } else {
                problems.push((format!("{li}/e{}", graph.segs[faces[f].side[0].0].2), "a block with no record beside this street"));
            }
        }
        for &id in recs.blocks.keys() {
            if !taken.contains_key(&id) {
                problems.push((format!("{li}/k{id}"), "no block of the streets holds its point"));
            }
        }
        Faces { graph, faces, outers, rings, holes, face_block }
    }

    /// Face `f`'s ring as points (local ft).
    pub(super) fn ring_of(&self, f: &Face) -> Vec<P> {
        f.ring.iter().map(|&v| self.graph.pts[v]).collect()
    }
}

/// Whether two pieces meet anywhere but a corner they share.
fn conflict(s: (usize, usize), t: (usize, usize), a: (i64, i64), b: (i64, i64), c: (i64, i64), d: (i64, i64)) -> bool {
    let shared = [s.0, s.1].iter().filter(|x| **x == t.0 || **x == t.1).count();
    if shared == 2 {
        return true;
    }
    if shared == 1 {
        // (Meeting at their shared corner only, unless they run along each other from it.)
        let (p, q, r) = if s.0 == t.0 { (a, b, d) } else if s.0 == t.1 { (a, b, c) } else if s.1 == t.0 { (b, a, d) } else { (b, a, c) };
        return orient(p, q, r) == 0 && (q.0 - p.0) as i128 * (r.0 - p.0) as i128 + (q.1 - p.1) as i128 * (r.1 - p.1) as i128 > 0;
    }
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1.signum() * o2.signum() < 0 && o3.signum() * o4.signum() < 0 {
        return true;
    }
    between(a, b, c) || between(a, b, d) || between(c, d, a) || between(c, d, b) || a == c || a == d || b == c || b == d
}

/// A hand town laid out, with each block's ground (world ft) and what could not be laid out.
pub struct HandBuilt {
    pub layout: Layout,
    pub blocks: Vec<(u32, Vec<super::clip::Shape>)>,
    pub problems: Problems,
}

thread_local! {
    /// Hand towns as built, by layout: the fingerprint of all they were built from (`fingerprint`).
    static BUILT: RefCell<FastMap<u32, (u64, Rc<Layout>)>> = RefCell::new(FastMap::default());
    /// Blocks as filled, by a hash of all a fill reads (`block_key`): so a change to one block
    /// fills only that one again, and what comes out never hangs on what was asked for first.
    static FILLED: RefCell<FastMap<u64, Rc<Filled>>> = RefCell::new(FastMap::default());
}

/// Whether layout `index` is a town laid out by hand (it has a header record).
pub fn is_hand(world: &World, index: usize) -> bool {
    let streets = &world.file.edits.streets;
    !streets.is_empty() && streets.contains_key(&index.to_string())
}

fn hash_value(h: &mut crate::core::hash::Fnv64, v: &Value) {
    match v {
        Value::Null => h.write(&[0]),
        Value::Bool(b) => h.write(&[1, *b as u8]),
        Value::Number(n) => {
            h.write(&[2]);
            h.write(&n.as_f64().unwrap_or(0.0).to_bits().to_le_bytes());
        }
        Value::String(t) => {
            h.write(&[3]);
            h.write(t.as_bytes());
            h.write(&[0xff]);
        }
        Value::Array(a) => {
            h.write(&[4]);
            h.write(&(a.len() as u64).to_le_bytes());
            a.iter().for_each(|x| hash_value(h, x));
        }
        Value::Object(o) => {
            // (serde_json's map is ordered by key: the same object hashes the same.)
            h.write(&[5]);
            h.write(&(o.len() as u64).to_le_bytes());
            for (k, x) in o {
                h.write(k.as_bytes());
                h.write(&[0xfe]);
                hash_value(h, x);
            }
        }
    }
}

/// The ground a hand town's blocks are laid out on: the world, the settlement (its seed, size,
/// culture as laid out) and the header's origin, reach, tier and roof seed.
fn ground_key(world: &World, t0: &T0, index: usize, town: &HandTown, reach: f64) -> u64 {
    let mut h = hash3(world.hash, super::layout_identity(t0, index) as i64, super::culture::mark(world, t0, index), 0x6a4d);
    // (And what a cemetery's look reads of the settlement.)
    if let Some(s) = super::settlement(t0, index) {
        h = hash3(h, super::wallstyle::wealth(s).to_bits() as i64, s.capital as i64, s.culture as i64);
    }
    h = hash3(h, town.origin[0].to_bits() as i64, town.origin[1].to_bits() as i64, reach.to_bits() as i64);
    hash3(h, town.tier as i64, town.culture as i64, u64::from_str_radix(&town.roofs, 16).unwrap_or(0) as i64)
}

/// The rectangle a hand town covers (world ft: x0, y0, x1, y1): its junctions' and the header's
/// extent round its origin, and a margin. No cap by its size: a town laid by hand reaches as far
/// as its streets.
pub fn bounds(streets: &BTreeMap<String, Value>, index: usize) -> Option<[f64; 4]> {
    const MARGIN: f64 = 300.0;
    let head = index.to_string();
    let h = streets.get(&head)?;
    let num = |v: Option<&Value>| v.and_then(Value::as_f64);
    let o = h.get("origin")?;
    let (ox, oy, e) = (num(o.get(0))?, num(o.get(1))?, num(h.get("extent")).unwrap_or(0.0));
    let mut b = [ox - e, oy - e, ox + e, oy + e];
    let lo = format!("{head}/n");
    for (_, v) in streets.range(lo.clone()..).take_while(|(k, _)| k.starts_with(&lo)) {
        if let (Some(x), Some(y)) = (num(v.get("p").and_then(|p| p.get(0))), num(v.get("p").and_then(|p| p.get(1)))) {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    // (And the bends of its streets.)
    let le = format!("{head}/e");
    for (_, v) in streets.range(le.clone()..).take_while(|(k, _)| k.starts_with(&le)) {
        for p in v.get("bends").and_then(Value::as_array).into_iter().flatten() {
            if let (Some(x), Some(y)) = (num(p.get(0)), num(p.get(1))) {
                b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            }
        }
    }
    Some([b[0] - MARGIN, b[1] - MARGIN, b[2] + MARGIN, b[3] + MARGIN])
}

/// A hash of all a hand town is built from: its ground (`ground_key`), its records, and the
/// buildings drawn by hand and wharves within its bounds. None for a layout not laid by hand.
pub fn fingerprint(world: &World, t0: &T0, index: usize) -> Option<u64> {
    if !is_hand(world, index) {
        return None;
    }
    let edits = &world.file.edits;
    let b = bounds(&edits.streets, index)?;
    let mut h = crate::core::hash::Fnv64::default();
    h.write(&hash3(world.hash, super::layout_identity(t0, index) as i64, super::culture::mark(world, t0, index), 0x6a4e).to_le_bytes());
    let head = index.to_string();
    let lo = format!("{head}/");
    // (The header but its count of people, which the builder doesn't read.)
    let mut header = edits.streets[&head].clone();
    if let Some(o) = header.as_object_mut() {
        for k in ["people", "was_people", "was_homes"] {
            o.remove(k);
        }
    }
    for (k, v) in std::iter::once((&head, &header)).chain(edits.streets.range(lo.clone()..).take_while(|(k, _)| k.starts_with(&lo))) {
        h.write(k.as_bytes());
        hash_value(&mut h, v);
    }
    for c in edits.created.iter().filter(|c| drawn_obstacle(c, &b)) {
        h.write(c.id.as_bytes());
        c.poly.iter().for_each(|p| h.write(&[p[0].to_bits().to_le_bytes(), p[1].to_bits().to_le_bytes()].concat()));
    }
    for (k, w) in edits.wharves.iter().filter(|(_, w)| wharf_obstacle(w, &b)) {
        h.write(k.as_bytes());
        h.write(&w.width.to_bits().to_le_bytes());
        w.pts.iter().for_each(|p| h.write(&[p[0].to_bits().to_le_bytes(), p[1].to_bits().to_le_bytes()].concat()));
    }
    Some(h.finish())
}

/// Whether points `pts`, grown by `pad` ft, overlap rectangle `b` (x0, y0, x1, y1).
fn box_meets(pts: &[P], pad: f64, b: &[f64; 4]) -> bool {
    let Some(first) = pts.first() else { return false };
    let r = pts.iter().fold([first[0], first[1], first[0], first[1]], |r, p| [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])]);
    r[0] - pad <= b[2] && r[2] + pad >= b[0] && r[1] - pad <= b[3] && r[3] + pad >= b[1]
}

/// A building or plot drawn by hand that a hand town within `b` builds round (`build_hand`'s
/// obstacles, `fingerprint`'s: the same test, so all the builder reads is hashed).
fn drawn_obstacle(c: &crate::world::Created, b: &[f64; 4]) -> bool {
    (c.kind == "building" || (c.kind == "plot" && !c.removed)) && c.poly.len() >= 3 && box_meets(&c.poly, 4.0, b)
}

/// A wharf a hand town within `b` builds round (as `drawn_obstacle`).
fn wharf_obstacle(w: &crate::world::Wharf, b: &[f64; 4]) -> bool {
    w.pts.len() >= 2 && box_meets(&w.pts, 0.5 * w.width + 4.0, b)
}

/// Layout `index` as its hand records lay it out, if it has a header (`base_layout`).
pub(super) fn built(world: &World, t0: &T0, index: usize) -> Option<Rc<Layout>> {
    let fp = fingerprint(world, t0, index)?;
    let key = index as u32;
    if let Some(l) = BUILT.with(|c| c.borrow().get(&key).filter(|e| e.0 == fp).map(|e| e.1.clone())) {
        return Some(l);
    }
    let l = Rc::new(build_hand(world, t0, index)?.layout);
    BUILT.with(|c| c.borrow_mut().insert(key, (fp, l.clone())));
    Some(l)
}

/// `build_hand`, remembered as `built` would (the change function's after: the app asks for it next),
/// with what could not be laid out.
pub(super) fn build_kept(world: &World, t0: &T0, index: usize) -> Option<(Rc<Layout>, Problems)> {
    let fp = fingerprint(world, t0, index)?;
    let b = build_hand(world, t0, index)?;
    let l = Rc::new(b.layout);
    BUILT.with(|c| c.borrow_mut().insert(index as u32, (fp, l.clone())));
    Some((l, b.problems))
}

/// What one block's fill laid out (town-local ft but its kept buildings, world ft).
struct Filled {
    layout: Layout,
    kept: Vec<Building>,
    plots: Vec<(Vec<P>, u8)>,
    problems: Problems,
}

/// A hash of all a block's fill reads: the ground (`ground_key`), its record, its ground and the
/// lines it fronts.
fn block_key(ground: u64, id: u32, rec: &Value, g: &[super::clip::Shape], fronts: &[super::fill::Front]) -> u64 {
    let mut h = crate::core::hash::Fnv64::default();
    h.write(&ground.to_le_bytes());
    h.write(&id.to_le_bytes());
    hash_value(&mut h, rec);
    let mut pts = |v: &[P]| {
        h.write(&(v.len() as u64).to_le_bytes());
        v.iter().for_each(|p| h.write(&[p[0].to_bits().to_le_bytes(), p[1].to_bits().to_le_bytes()].concat()));
    };
    for sh in g {
        pts(&sh.outer);
        sh.holes.iter().for_each(|x| pts(x));
    }
    for f in fronts {
        pts(&[f.n, [f.c, 0.0], f.a, f.b]);
    }
    h.finish()
}

/// What a block is laid out as, once its ground is known.
pub fn ward_of(name: &str) -> Option<Ward> {
    WARDS.iter().copied().find(|w| w.name() == name)
}

/// Hand town `index` laid out from its records (`Edits.streets`): the street graph's faces, each
/// the block whose `at` it holds (the lowest id if several), cut back from its streets by their
/// setbacks, from painted areas and from what is drawn or built over it (buildings drawn by hand,
/// wharves); each block filled as its record says (a kept block's buildings as converted),
/// then the rows along streets. None without a header or a settlement.
pub fn build_hand(world: &World, t0: &T0, index: usize) -> Option<HandBuilt> {
    use super::clip::{self, Ends, Join, Shape};
    use super::fill::{Block as Region, Front, build_block};
    let mut recs = HandRecords::of(&world.file.edits.streets, index);
    let header = recs.town.clone()?;
    let town = &header;
    let s = super::culture::settlement(world, t0, index)?;
    let li = index.to_string();
    let origin = town.origin;
    let ground = KeptGround::of(world, t0, index, town);
    let tier = Tier::from_u8(town.tier);
    // (The ground read as far as its streets reach, no cap by its size: kept blocks on the ground
    // as converted, all else on this.)
    let b = bounds(&world.file.edits.streets, index)?;
    let far = [[b[0], b[1]], [b[2], b[1]], [b[0], b[3]], [b[2], b[3]]].iter().map(|p| dist(*p, origin)).fold(0.0, f64::max);
    let wide = (far > town.reach).then(|| KeptGround::at(world, t0, index, origin, far, tier, 0));
    let kept_site = &ground.site;
    let site = wide.as_ref().map_or(kept_site, |g| &g.site);
    let mut l = Layout::empty(index as u32, origin, if wide.is_some() { super::urban_radius(tier, s.population).max(far - 300.0) } else { super::urban_radius(tier, s.population) }, tier);
    l.set_roofs(world, t0, town.culture);
    l.roof_seed = u64::from_str_radix(&town.roofs, 16).unwrap_or(l.roof_seed);
    l.wall_style = super::wallstyle::WallStyle::of(&s);
    l.on_water = s.coastal || s.river || !site.river.is_empty();
    // (What each block's fill starts from.)
    let template = l.clone();
    let gkey = ground_key(world, t0, index, town, far.max(town.reach));
    let mut problems: Problems = Vec::new();
    let Faces { graph, faces, outers, rings, holes, mut face_block } = Faces::of(&recs, origin, &li, &mut problems);
    // A face with no block record takes the header's fill (a blank town's first blocks, before
    // the change function gives them records): numbered on from the records, in face order, and
    // seeded as the change function seeds a new block.
    let mut unrecorded: BTreeMap<u32, Value> = BTreeMap::new();
    if face_block.iter().any(Option::is_none) {
        let hseed = u64::from_str_radix(&town.seed, 16).unwrap_or(0);
        let mut next = recs.blocks.keys().next_back().map_or(0, |k| k + 1).max(town.next_block.unwrap_or(0));
        for f in 0..faces.len() {
            if face_block[f].is_some() {
                continue;
            }
            let at = add(inside_point(&rings[f]), origin);
            let b = Block { at, fill: town.fill.clone(), district: None, seed: hex(hash3(hseed, next as i64, 0xb10c, 1)), generation: 0, pins: Vec::new(), kept: None };
            unrecorded.insert(next, serde_json::to_value(&b).unwrap_or(Value::Null));
            recs.blocks.insert(next, b);
            face_block[f] = Some(next);
            next += 1;
        }
        problems.retain(|p| p.1 != "a block with no record beside this street");
    }
    let recs = recs;
    let street = |k: usize| &recs.streets[&graph.segs[k].2];
    let set = |k: usize, left: bool| if left { street(k).set_l } else { street(k).set_r };
    let ring_pts = |f: &Face| f.ring.iter().map(|&v| graph.pts[v]).collect::<Vec<P>>();

    // What no block is built over: buildings drawn by hand, wharves, painted areas.
    let pad = |poly: &[P], d: f64| clip::offset(&clip::region(poly), d, Join::Miter(2.0));
    let mut obstacles: Vec<Shape> = Vec::new();
    let tb = bounds(&world.file.edits.streets, index)?;
    for c in world.file.edits.created.iter().filter(|c| drawn_obstacle(c, &tb)) {
        let poly: Vec<P> = c.poly.iter().map(|p| sub(*p, origin)).collect();
        obstacles.extend(pad(&poly, 2.0));
    }
    for w in world.file.edits.wharves.values().filter(|w| wharf_obstacle(w, &tb)) {
        let pts: Vec<P> = w.pts.iter().map(|p| sub(*p, origin)).collect();
        obstacles.extend(clip::buffer(&pts, 0.5 * w.width + 2.0, 0.5 * w.width + 2.0, Join::Miter(2.0), Ends::Flat));
    }
    let painted: Vec<(u32, Vec<P>, OpenKind)> = recs.areas.iter().filter(|(_, a)| a.poly.len() >= 3).map(|(&k, a)| (k, a.poly.iter().map(|p| sub(*p, origin)).collect(), a.open)).collect();
    let mut cut: Vec<Shape> = obstacles.clone();
    for (_, poly, _) in &painted {
        cut.extend(clip::region(poly));
    }
    // Squares at junctions (as converted while the junction stands so, else laid anew), cut out
    // of the blocks round them.
    let works = town.works.as_ref();
    let mut corner_squares: Vec<(u32, PlazaKind, Vec<P>)> = Vec::new();
    for (&j, jr) in &recs.junctions {
        let Some(kind) = jr.square.as_deref().and_then(plaza_kind) else { continue };
        let kept = works.and_then(|w| w.corners.iter().find(|c| c.0 == j && c.1 == kind.name())).map(|c| unpk(&c.2)).filter(|v| v.len() >= 4 && v[0] == jr.p);
        let pts = match kept {
            Some(v) => v[1..].iter().map(|p| sub(*p, origin)).collect(),
            None => {
                let reach = recs.streets.values().filter(|st| st.a == j || st.b == j).map(|st| st.set_l.max(st.set_r)).fold(0.0, f64::max);
                junction_square(sub(jr.p, origin), reach)
            }
        };
        cut.extend(clip::region(&pts));
        corner_squares.push((j, kind, pts));
    }
    let bbox_of = |pts: &mut dyn Iterator<Item = P>| pts.fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    let cut_boxes: Vec<[f64; 4]> = cut.iter().map(|sh| bbox_of(&mut sh.outer.iter().copied())).collect();

    // Each block's ground: its face cut back by each street's setback on its side (mitred, as a
    // generated town's patch is), less the parts inside it, painted areas and obstacles.
    let mut grounds: Vec<Vec<Shape>> = Vec::with_capacity(faces.len());
    for (f, face) in faces.iter().enumerate() {
        let ring = &rings[f];
        let d: Vec<f64> = face.side.iter().map(|&(k, left)| set(k, left)).collect();
        let simple = {
            let mut v = face.ring.clone();
            v.sort_unstable();
            v.windows(2).all(|w| w[0] != w[1])
        };
        let mut g = if simple {
            clip::inset_edges(ring, &d)
        } else {
            // (A street with the face on both sides: the face less each street's buffer.)
            let mut sub_: Vec<Shape> = Vec::new();
            for e in 0..ring.len() {
                sub_.extend(clip::buffer(&[ring[e], ring[(e + 1) % ring.len()]], d[e], 0.0, Join::Miter(2.0), Ends::Square));
            }
            clip::difference(&clip::region(ring), &sub_)
        };
        // (And each side's own strip: a mitre takes the nearer setback where two sides run straight
        // on with different ones.)
        let mut minus: Vec<Shape> = Vec::new();
        for e in 0..ring.len() {
            if d[e] > 0.0 {
                minus.extend(clip::buffer(&[ring[e], ring[(e + 1) % ring.len()]], d[e], 0.0, Join::Miter(2.0), Ends::Flat));
            }
        }
        for &h in &holes[f] {
            let o = &outers[h];
            let hr = ring_pts(o);
            minus.extend(clip::region(&hr));
            for (e, &(k, left)) in o.side.iter().enumerate() {
                minus.extend(clip::buffer(&[hr[e], hr[(e + 1) % hr.len()]], set(k, left), 0.0, Join::Miter(2.0), Ends::Square));
            }
        }
        let fb = bbox_of(&mut ring.iter().copied());
        for (sh, b) in cut.iter().zip(&cut_boxes) {
            if b[0] <= fb[2] && fb[0] <= b[2] && b[1] <= fb[3] && fb[1] <= b[3] {
                minus.push(sh.clone());
            }
        }
        if !minus.is_empty() {
            g = clip::difference(&g, &minus);
        }
        grounds.push(g);
    }

    // The streets: each side's paved strip (as a generated town paves its patches' sides), main
    // streets drawn whole too.
    let near_water = |a: P, b: P, w: f64| {
        let n = (dist(a, b) / 5.0).ceil() as usize;
        site.near_river(lerp(a, b, 0.5), 2.0) || (0..=n).any(|j| site.near_river(lerp(a, b, j as f64 / n.max(1) as f64), 0.5 * w))
    };
    // (Paved on the sides of blocks built on or laid out as parks, as a generated town paves its
    // patches but squares and farmland.)
    let paves = |b: &Block| match &b.fill {
        BlockFill::Generated { .. } | BlockFill::Castle { .. } => true,
        BlockFill::Open { open, .. } => matches!(open, OpenKind::Park | OpenKind::Garden | OpenKind::Cemetery),
        BlockFill::Hand => false,
    };
    let mut paved_side: crate::core::hash::FastSet<(usize, bool)> = Default::default();
    for (f, face) in faces.iter().enumerate() {
        if face_block[f].is_some_and(|id| paves(&recs.blocks[&id])) {
            paved_side.extend(face.side.iter().copied());
        }
    }
    let mut lines: BTreeMap<u32, Vec<P>> = BTreeMap::new();
    for (k, &(u, v, id)) in graph.segs.iter().enumerate() {
        let line = lines.entry(id).or_insert_with(|| vec![graph.pts[u]]);
        line.push(graph.pts[v]);
        let st = &recs.streets[&id];
        let (a, b) = (graph.pts[u], graph.pts[v]);
        let el = dist(a, b);
        if st.class == StreetClass::Joint || el < 1.0 {
            continue;
        }
        let ev = mul(sub(b, a), 1.0 / el);
        for (d, left) in [(st.set_l, true), (st.set_r, false)] {
            if d <= 0.0 || !paved_side.contains(&(k, left)) {
                continue;
            }
            let o = mul(if left { [-ev[1], ev[0]] } else { [ev[1], -ev[0]] }, 0.5 * d);
            let (p, q) = (add(a, o), add(b, o));
            if !near_water(p, q, d) {
                l.roads.push((vec![p, q], 3, d + 1.0));
            }
        }
    }
    for (id, line) in &lines {
        if recs.streets[id].class == StreetClass::Main {
            l.streets.push(line.clone());
            l.roads.push((line.clone(), 4, recs.streets[id].width.clamp(8.0, 18.0)));
        }
    }
    // (Dead ends, with no block beside them to show where they run: each paved as a main street
    // is, whatever its class, but a joint's.)
    let mut loose: BTreeMap<u32, Vec<P>> = BTreeMap::new();
    for &(u, v, id) in &graph.loose {
        loose.entry(id).or_insert_with(|| vec![graph.pts[u]]).push(graph.pts[v]);
    }
    for (id, line) in &loose {
        let st = &recs.streets[id];
        if st.class == StreetClass::Joint {
            continue;
        }
        if st.class == StreetClass::Main {
            l.streets.push(line.clone());
        }
        l.roads.push((line.clone(), 4, st.width.clamp(8.0, 18.0)));
    }

    // Bridges: a deck wherever a street runs over the river, set or not (as converted where a
    // street or a road in from the country still runs over one), as a generated town spans it.
    let local = |v: Vec<P>| v.into_iter().map(|p| sub(p, origin)).collect::<Vec<P>>();
    let kept_decks: Vec<Vec<P>> = works.map(|w| w.bridges.iter().map(|b| local(unpk(b))).collect()).unwrap_or_default();
    let approaches: Vec<(u8, f64, Vec<P>)> = works.map(|w| w.roads.iter().map(|r| (r.0, r.1, local(unpk(&r.2)))).collect()).unwrap_or_default();
    let mut deck_kept = vec![false; kept_decks.len()];
    for (k, d) in kept_decks.iter().enumerate() {
        deck_kept[k] = approaches.iter().any(|r| r.2.windows(2).any(|s| runs_over(d, s[0], s[1])));
    }
    let mut decks: Vec<Vec<P>> = Vec::new();
    for (k, &(u, v, id)) in graph.segs.iter().enumerate() {
        let st = &recs.streets[&id];
        let (a, b) = (graph.pts[u], graph.pts[v]);
        let el = dist(a, b);
        // (A street, not the edge of farmland or of a square: a block beside it built on.)
        if st.class == StreetClass::Joint || el < 1.0 || !(st.class == StreetClass::Main || paved_side.contains(&(k, true)) || paved_side.contains(&(k, false))) {
            continue;
        }
        let mut over = false;
        for (k, d) in kept_decks.iter().enumerate() {
            if runs_over(d, a, b) {
                deck_kept[k] = true;
                over = true;
            }
        }
        let n = (el / 5.0).ceil() as usize;
        let wet = |p: P| site.near_river(p, 0.0);
        if over || site.river.is_empty() || wet(a) || wet(b) || !(1..n).any(|j| wet(lerp(a, b, j as f64 / n as f64))) {
            continue;
        }
        let m = super::river_crossing(site, a, b);
        let ud = mul(sub(b, a), 1.0 / el);
        let (fwd, back) = (site.dry_reach(m, ud, 0.0, 1.0, 0.0) + 14.0, site.dry_reach(m, ud, 0.0, -1.0, 0.0) + 14.0);
        // (Far wider than the river: the street runs along the water, not over it.)
        let hw = site.nearest_river(m).map_or(0.0, |r| r.2);
        // (Or no water across it to speak of.)
        if fwd + back > 4.0 * hw + 80.0 || fwd + back < 28.0 + 2.0 {
            continue;
        }
        let width = if tier >= Tier::City { 24.0 } else if st.class == StreetClass::Main { 16.0 } else { 12.0 };
        let (fwd, back) = (site.dry_reach(m, ud, width, 1.0, fwd - 14.0) + 14.0, site.dry_reach(m, ud, width, -1.0, back - 14.0) + 14.0);
        decks.push(super::rect(add(m, mul(ud, 0.5 * (fwd - back))), ud, fwd + back, width));
        if st.class != StreetClass::Main {
            l.roads.push((vec![a, m, b], 4, 12.0));
        }
    }
    let decks: Vec<Vec<P>> = kept_decks.into_iter().zip(&deck_kept).filter(|x| *x.1).map(|x| x.0).chain(decks).collect();
    l.bridges = decks;
    for (class, w, pts) in approaches {
        l.roads.push((pts, class, w));
    }

    // The blocks' fills.
    let meta = (index as u32, origin, 0.0, tier);
    let mut kept_bs: Vec<Building> = Vec::new();
    let mut plots: Vec<(Vec<P>, u8)> = Vec::new();
    let mut built_ground: Vec<Shape> = Vec::new();
    let cem = |id: u32| super::cemetery::Style::of(&s, id as usize, false);
    let mut out: Vec<(u32, Vec<Shape>)> = Vec::new();
    let mut named: Vec<(u32, String, Vec<P>)> = Vec::new();
    let seed_of = |hexs: &str| u64::from_str_radix(hexs, 16).unwrap_or_else(|_| hash2(0x4a4d, index as i64, 0));
    let shift = |sh: &[Shape]| -> Vec<Shape> {
        sh.iter().map(|x| Shape { outer: x.outer.iter().map(|p| add(*p, origin)).collect(), holes: x.holes.iter().map(|h| h.iter().map(|p| add(*p, origin)).collect()).collect() }).collect()
    };
    // (Every id pinned in the town, by block: no block's fresh ids take one pinned in another.)
    let pinned_all: Vec<(u32, u32)> = recs.blocks.iter().flat_map(|(&b, r)| r.pins.iter().map(move |p| (b, p.id))).collect();
    for (f, face) in faces.iter().enumerate() {
        let Some(id) = face_block[f] else { continue };
        let rec = &recs.blocks[&id];
        let g = &grounds[f];
        out.push((id, shift(g)));
        if let Some(d) = &rec.district {
            named.push((id, d.clone(), rings[f].clone()));
        }
        // (What a row keeps clear of: every block but one left for buildings drawn by hand.)
        if !matches!(rec.fill, BlockFill::Hand) {
            built_ground.extend(g.iter().cloned());
        }
        let spot = super::places::Spot::Patch(id);
        let bseed = seed_of(&rec.seed);
        let gn = rec.generation as i64;
        let ring = &rings[f];
        // (Its streets' lines, outward, where a lot may front them.)
        let fronts: Vec<Front> = face
            .side
            .iter()
            .enumerate()
            .filter_map(|(e, &(k, left))| {
                let d = set(k, left);
                let (a, b) = (ring[e], ring[(e + 1) % ring.len()]);
                let el = dist(a, b);
                (d > 0.0 && el > 1e-9 && street(k).class != StreetClass::Joint).then(|| {
                    let n = [(b[1] - a[1]) / el, -(b[0] - a[0]) / el];
                    Front { n, c: dot(a, n) - d, a: sub(a, mul(n, d)), b: sub(b, mul(n, d)) }
                })
            })
            .collect();
        let biggest = || g.iter().max_by(|x, y| x.area().total_cmp(&y.area())).map(|x| x.outer.clone());
        let built_on = !matches!(rec.fill, BlockFill::Hand | BlockFill::Open { .. });
        // (Filled once for all it reads: the same block comes out the same whatever else changed.)
        let base = HAND_ID as u64 + ((id as u64 * ROLLS as u64 + gn as u64 % ROLLS as u64) * PER_PATCH as u64);
        let foreign: Vec<u32> = pinned_all.iter().filter(|x| x.0 != id && (x.1 as u64) >= base && (x.1 as u64) < base + PER_PATCH as u64).map(|x| x.1).collect();
        let mut key = block_key(gkey, id, world.file.edits.streets.get(&format!("{li}/k{id}")).or_else(|| unrecorded.get(&id)).unwrap_or(&Value::Null), g, &fronts);
        for &p in &foreign {
            key = hash2(key, p as i64, 0x4e1d);
        }
        let filled = FILLED.with(|c| c.borrow().get(&key).cloned()).unwrap_or_else(|| {
            let mut l = template.clone();
            let mut kept_bs: Vec<Building> = Vec::new();
            let mut plots: Vec<(Vec<P>, u8)> = Vec::new();
            let mut problems: Problems = Vec::new();
            let first = 0;
            let mut pinned: Vec<Option<u32>> = Vec::new();
            'fill: {
                if built_on {
                    l.districts.push(ring.clone());
                }
                if let Some(kept) = &rec.kept {
                    // (Its fill laid out once: its buildings to keep, its open ground, its plots.)
                    let laid = kept.refill.as_ref().map(|r| r.lay(kept_site, meta, Some(cem(id))));
                    let raw = match &laid {
                        Some(Ok((lay, _))) if kept.refill.as_ref().is_some_and(|r| r.k < 2) => lay.buildings.as_slice(),
                        _ => &[],
                    };
                    match kept_from(&ground, kept, raw) {
                        Ok(bs) => kept_bs.extend(bs),
                        Err(_) => problems.push((format!("{li}/k{id}"), "its kept buildings could not be laid out again")),
                    }
                    if let (Some(r), Some(laid)) = (&kept.refill, laid) {
                        match laid {
                            Ok((mut lay, pl)) => {
                                if r.k == 0 {
                                    l.blocks.push(r.block().unwrap_or_default());
                                }
                                let mask = kept.fields.as_bytes();
                                plots.extend(pl.into_iter().enumerate().filter(|(k, _)| mask.get(*k).is_none_or(|c| *c == b'1')).map(|x| x.1));
                                lay.plazas.iter_mut().for_each(|p| p.spot = spot);
                                lay.parks.iter_mut().for_each(|p| p.spot = spot);
                                l.plazas.append(&mut lay.plazas);
                                l.parks.append(&mut lay.parks);
                                l.monuments.append(&mut lay.monuments);
                                l.roads.append(&mut lay.roads);
                            }
                            Err(_) => problems.push((format!("{li}/k{id}"), "its kept fill could not be laid out again")),
                        }
                    } else if built_on {
                        l.blocks.extend(g.iter().map(|x| x.outer.clone()));
                    }
                    break 'fill;
                }
                if g.is_empty() {
                    break 'fill;
                }
                match &rec.fill {
                    BlockFill::Hand | BlockFill::Castle { .. } => {}
                    BlockFill::Generated { ward, lots, height, reroll, close } => {
                        let Some(w) = ward_of(ward) else {
                            problems.push((format!("{li}/k{id}"), "no such ward"));
                            break 'fill;
                        };
                        let salt = |h: u64| if *reroll == 0 { h } else { hash3(h, *reroll as i64, 0x7e70, 1) };
                        let mut dp = District::roll(w, salt(hash2(bseed, gn, 0xd15)));
                        if let Some(k) = lots.as_deref().and_then(|n| LOT_SIZES.iter().position(|x| *x == n)) {
                            dp.min_sq = District::base(w) * super::wards::LOT_FACTOR[k];
                        }
                        let mut rng = Pcg32::new(salt(hash2(bseed, gn, 0x107)), 47);
                        l.blocks.extend(g.iter().map(|x| x.outer.clone()));
                        let alleys = l.roads.len();
                        build_block(site, Region::Region(g), &fronts, w, &dp, &mut rng, &mut l, None);
                        // A temple's close round its temple on its largest piece (where it fits,
                        // unless asked for none), else a temple alone where no lot was cut.
                        if w == Ward::Temple
                            && let Some(b) = biggest()
                        {
                            let lines: Vec<(P, f64)> = fronts.iter().map(|f| (f.n, f.c)).collect();
                            let t = unit(hash2(bseed, gn, 65));
                            let laid = *close != Some(false) && l.buildings.len() > first && super::squares::close(site, &b, &lines, t, first, alleys, &mut l);
                            if !laid && *close == Some(true) {
                                problems.push((format!("{li}/k{id}"), "no room for a close round its temple"));
                            }
                            if l.buildings.len() == first {
                                super::squares::lone_temple(site, &b, t, &mut l);
                            }
                        }
                        pinned = pin_on(&mut l, first, &rec.pins, origin);
                        home_storeys(&mut l, first, w, tier, hash2(salt(bseed), gn, 0x5707), &pinned);
                        businesses(&mut l, first, w, tier, s.coastal || s.river || !site.river.is_empty(), hash2(salt(bseed), gn, 0xb05), &pinned);
                        height_storeys(&mut l, first, height.as_deref(), &pinned);
                    }
                    BlockFill::Open { open, style } => {
                        // (Every piece of its ground, the largest first: each but the first with
                        // looks of its own.)
                        let mut pieces: Vec<(f64, usize)> = g.iter().enumerate().map(|(k, x)| (x.area(), k)).filter(|x| x.0 >= 200.0).collect();
                        pieces.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
                        for (pi, &(_, k)) in pieces.iter().enumerate() {
                            let salt = |h: u64| if pi == 0 { h } else { hash2(h, pi as i64, 0x91ec) };
                            let roll = |j: i64| unit(salt(hash3(bseed, gn, j, 0x09e)));
                            let fill = open_fill(*open, style.as_deref(), &fronts, salt(hash2(bseed, gn, 0x9a4c)), || cem(id), &mut || Pcg32::new(salt(hash2(bseed, gn, 0xfa)), 61), tier);
                            let mut pl = Vec::new();
                            let (pz, pk) = (l.plazas.len(), l.parks.len());
                            super::fill_block(site, &g[k].outer, Vec::new(), fill, &roll, spot, &mut l, &mut pl);
                            l.plazas[pz..].iter_mut().for_each(|p| p.spot = spot);
                            l.parks[pk..].iter_mut().for_each(|p| p.spot = spot);
                            plots.append(&mut pl);
                        }
                    }
                }
                // (Its own ids, by the order it laid them out, but those pinned: those keep theirs,
                // and no other takes one of them, nor one pinned anywhere else in the town. Past
                // its range (PER_PATCH ids, blocks under ROW_BLOCK) the rest are left out.)
                let held: Vec<u32> = pinned.iter().flatten().copied().chain(foreign.iter().copied()).collect();
                let end = if id < ROW_BLOCK { base + PER_PATCH as u64 } else { base };
                let mut next = base;
                let mut keep = vec![true; l.buildings.len() - first];
                for (k, b) in l.buildings[first..].iter_mut().enumerate() {
                    if let Some(Some(pid)) = pinned.get(k) {
                        b.id = *pid;
                        continue;
                    }
                    while next < end && held.contains(&(next as u32)) {
                        next += 1;
                    }
                    if next >= end {
                        keep[k] = false;
                        continue;
                    }
                    b.id = next as u32;
                    next += 1;
                }
                if keep.contains(&false) {
                    let mut k = 0;
                    l.buildings.retain(|_| {
                        k += 1;
                        k <= first || keep[k - 1 - first]
                    });
                    problems.push((format!("{li}/k{id}"), if id >= ROW_BLOCK { "block number too high for its ids" } else { "more buildings than a block has ids for" }));
                }
            }
            let fl = Rc::new(Filled { layout: l, kept: kept_bs, plots, problems });
            FILLED.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() >= 40_000 {
                    c.clear();
                }
                c.insert(key, fl.clone());
            });
            fl
        });
        let fl = &filled.layout;
        l.buildings.extend(fl.buildings.iter().cloned());
        l.plazas.extend(fl.plazas.iter().cloned());
        l.parks.extend(fl.parks.iter().cloned());
        l.monuments.extend(fl.monuments.iter().cloned());
        l.roads.extend(fl.roads.iter().cloned());
        l.blocks.extend(fl.blocks.iter().cloned());
        l.districts.extend(fl.districts.iter().cloned());
        l.crofts.extend(fl.crofts.iter().cloned());
        l.grounds.extend(fl.grounds.iter().cloned());
        kept_bs.extend(filled.kept.iter().cloned());
        plots.extend(filled.plots.iter().cloned());
        problems.extend(filled.problems.iter().cloned());
    }
    // Painted areas: open ground of their kind.
    for (k, poly, open) in &painted {
        let key = hash2(seed_of(&town.seed), *k as i64, 0xa4ea);
        let fill = open_fill(*open, None, &[], key, || cem(*k + ROW_BLOCK), &mut || Pcg32::new(key, 61), tier);
        let mut pl = Vec::new();
        super::fill_block(site, poly, Vec::new(), fill, &|j| unit(hash2(key, j, 3)), super::places::Spot::None, &mut l, &mut pl);
        plots.append(&mut pl);
    }
    // Rows of lots along streets, each clear of what stands already.
    let mut avoid: Vec<Shape> = built_ground;
    avoid.extend(obstacles.iter().cloned());
    avoid.extend(painted.iter().flat_map(|a| clip::region(&a.1)));
    avoid.extend(corner_squares.iter().flat_map(|c| clip::region(&c.2)));
    // (And what stands in no block: kept by the header.)
    let loose = match &town.kept {
        Some(k) => kept_on(&ground, k).map_err(|_| problems.push((li.clone(), "its kept buildings could not be laid out again"))).unwrap_or_default(),
        None => Vec::new(),
    };
    for b in &loose {
        avoid.extend(clip::region(&b.poly.iter().map(|p| sub(*p, origin)).collect::<Vec<P>>()));
    }
    for (id, line) in &lines {
        let w = recs.streets[id].width;
        avoid.extend(clip::buffer(line, 0.5 * w, 0.5 * w, Join::Miter(2.0), Ends::Flat));
    }
    for (&r, row) in &recs.rows {
        let (Some(line), Some(w)) = (lines.get(&row.street), ward_of(&row.ward)) else {
            problems.push((format!("{li}/r{r}"), "no such street or ward"));
            continue;
        };
        if r >= MAX_ROWS {
            problems.push((format!("{li}/r{r}"), "row number too high for its ids"));
            continue;
        }
        let rseed = seed_of(&row.seed);
        let mut dp = District::roll(w, hash2(rseed, r as i64, 0xd15));
        if let Some(k) = row.lots.as_deref().and_then(|n| LOT_SIZES.iter().position(|x| *x == n)) {
            dp.min_sq = District::base(w) * super::wards::LOT_FACTOR[k];
        }
        let mut rng = Pcg32::new(hash2(rseed, r as i64, 0x107), 1000 + r as u64);
        let side = super::rows::Side { line, left: row.side == "l", setback: 0.5 * recs.streets[&row.street].width + 2.0, depth: row.depth, gaps: row.gaps.clamp(0.0, 0.9) };
        let first = l.buildings.len();
        let lots = super::rows::build_row(site, &side, w, &dp, &mut rng, &avoid, &mut l);
        l.blocks.extend(super::rows::bands(&side));
        for lot in &lots {
            avoid.extend(clip::region(lot));
        }
        let base = HAND_ID as u64 + ((ROW_BLOCK + r) as u64 * ROLLS as u64) * PER_PATCH as u64;
        // (A row's ids run over all ROLLS of its number; past them the rest are left out.)
        let room = (ROLLS * PER_PATCH) as usize;
        if l.buildings.len() - first > room {
            l.buildings.truncate(first + room);
            problems.push((format!("{li}/r{r}"), "more buildings than a row has ids for"));
        }
        for (k, b) in l.buildings[first..].iter_mut().enumerate() {
            b.id = (base + k as u64) as u32;
        }
    }
    // Farmland's plots, off the sand: the town's fields.
    l.fields.extend(plots.into_iter().filter_map(|(f, _)| super::fields::off_sand(site, &f)));

    // Local → world, then what was kept (world ft already).
    let tw = |p: &mut P| *p = add(*p, origin);
    for b in &mut l.buildings {
        b.poly.iter_mut().chain(b.parts.iter_mut().flat_map(|p| p.poly.iter_mut())).for_each(tw);
    }
    for (pts, _, _) in &mut l.roads {
        pts.iter_mut().for_each(tw);
    }
    for poly in l.streets.iter_mut().chain(l.plazas.iter_mut().map(|p| &mut p.pts)).chain(&mut l.fields).chain(&mut l.districts).chain(&mut l.blocks).chain(&mut l.monuments).chain(&mut l.bridges) {
        poly.iter_mut().for_each(tw);
    }
    for pk in &mut l.parks {
        pk.pts.iter_mut().chain([&mut pk.ground.origin]).for_each(tw);
    }
    for c in &mut l.crofts {
        c.pts.iter_mut().for_each(tw);
    }
    for (_, g) in &mut l.grounds {
        g.iter_mut().for_each(tw);
    }
    l.buildings.append(&mut kept_bs);
    l.buildings.extend(loose);
    l.buildings.sort_by_key(|b| b.id);
    // Businesses laid out anew named unlike any other in the town, each by its id.
    {
        let mut names: std::collections::BTreeSet<String> = l.buildings.iter().filter(|b| b.func.is_some()).filter_map(|b| b.name.clone()).collect();
        let tseed = seed_of(&town.seed);
        for b in l.buildings.iter_mut().filter(|b| b.id >= HAND_ID && b.func.is_some() && b.name.is_none()) {
            for salt in 0..8 {
                let mut rng = Pcg32::new(hash3(tseed, b.id as i64, 0x4e + salt, 0xb7), 43);
                b.name = super::sites::trade_name(b.func, &mut rng, t0, origin, names.iter().map(String::as_str));
                if b.name.as_ref().is_none_or(|n| !names.contains(n)) {
                    break;
                }
            }
            names.extend(b.name.clone());
        }
    }
    super::fields::assign(site, s.seed, &mut l);

    // Walls, towers and gates: as converted while the wall lines and castle blocks are as they
    // were, else laid anew from the wall lines (castles' curtains then go with them).
    let wkey = works_of(&recs);
    match works.filter(|w| w.of == wkey) {
        Some(w) => {
            let nums = |s: &str| unpack(s).unwrap_or_default();
            l.walls = w.walls.iter().map(|x| unpk(x)).collect();
            l.towers = unpk(&w.towers);
            l.gates = unpk(&w.gates);
            l.gate_towers = unpk(&w.gate_towers);
            l.gate_spots = w.gate_spots.clone();
            l.bastions = w.bastions.iter().map(|x| unpk(x)).collect();
            l.bastions_dropped = w.dropped;
            l.castles = nums(&w.castles).chunks_exact(3).map(|c| ([c[0], c[1]], c[2])).collect();
            l.square_towers = nums(&w.square).chunks_exact(4).map(|c| ([c[0], c[1]], [c[2], c[3]])).collect();
            l.tower_forms = forms_unpk(&w.forms);
            l.extra_towers = nums(&w.extra).chunks_exact(3).map(|c| ([c[0], c[1]], c[2] != 0.0)).collect();
            l.mottes = nums(&w.mottes).chunks_exact(7).map(|c| super::Motte { at: [c[0], c[1]], top_r: c[2], foot_r: c[3], top: c[4] as f32, way: [c[5], c[6]] }).collect();
        }
        None => {
            let segs: Vec<(P, P)> = graph.segs.iter().filter(|g| recs.streets[&g.2].class != StreetClass::Joint).map(|g| (graph.pts[g.0], graph.pts[g.1])).collect();
            let (pieces, towers, gate_towers, gates) = lay_walls(site, &recs, &segs, origin, l.wall_style.spacing);
            let w = |v: Vec<P>| v.into_iter().map(|p| add(p, origin)).collect::<Vec<P>>();
            l.walls = pieces.into_iter().map(w).collect();
            l.towers = w(towers);
            l.gate_towers = w(gate_towers);
            l.gates = w(gates);
            let (style, laid) = (l.wall_style, l.towers.clone());
            super::wallstyle::shape_towers(&mut l, &style, &laid, origin, false);
        }
    }
    // Squares at junctions, and those of no block as converted (quays).
    for (j, kind, pts) in corner_squares {
        l.plazas.push(super::Plaza { pts: pts.into_iter().map(|p| add(p, origin)).collect(), kind, spot: super::places::Spot::Corner(j) });
    }
    for (kind, pts) in works.map(|w| w.plazas.as_slice()).unwrap_or_default() {
        if let Some(kind) = plaza_kind(kind) {
            l.plazas.push(super::Plaza { pts: unpk(pts), kind, spot: super::places::Spot::None });
        }
    }
    for (meta, pts) in works.map(|w| w.parks.as_slice()).unwrap_or_default() {
        l.parks.extend(park_of(kept_site, origin, meta, pts));
    }
    // Districts: the blocks of each name (in the order of their lowest block), labelled along them.
    named.sort_by_key(|x| x.0);
    let mut by_name: BTreeMap<String, Vec<Vec<P>>> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    for (_, name, ring) in named {
        if !by_name.contains_key(&name) {
            names.push(name.clone());
        }
        by_name.entry(name).or_default().push(ring);
    }
    for name in names {
        let patches = by_name.remove(&name).unwrap_or_default();
        let label = super::label_curve(&patches).0;
        let kind = works.and_then(|w| w.quarters.iter().find(|q| q.0 == name)).and_then(|q| QUARTERS.iter().copied().find(|k| k.name() == q.1)).unwrap_or(QuarterKind::Regular);
        let w = |v: Vec<P>| v.into_iter().map(|p| add(p, origin)).collect::<Vec<P>>();
        l.quarters.push(super::Quarter { name, kind, patches: patches.into_iter().map(w).collect(), label: w(label) });
    }
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in l.buildings.iter().flat_map(|b| b.poly.iter()).chain(l.walls.iter().flatten()).chain(l.fields.iter().flatten()).chain(l.plazas.iter().flat_map(|p| p.pts.iter())).chain(l.parks.iter().flat_map(|p| p.pts.iter())).chain(l.districts.iter().flatten()) {
        bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
    }
    l.bbox = if bb[0] <= bb[2] { bb } else { [origin[0], origin[1], origin[0], origin[1]] };
    // Sewers and catacombs under cities, from its streets as laid by hand.
    if tier >= Tier::City {
        l.entrances = crate::under::city_entrances(&l, t0, s.seed);
    }
    l.number_entrances();
    Some(HandBuilt { layout: l, blocks: out, problems })
}

/// Pins on a block laid out anew (`Block::pins`, world ft; `l` local about `origin`): the building
/// standing on each pin's point takes its id, and its business with the name it had (each building
/// one pin, the pins in order); a pin no building stands on then goes to the nearest building left
/// within `PIN_REACH` ft (a lot moved a little), else it is lost. Per building laid out from
/// `first`: the id pinned to it.
fn pin_on(l: &mut Layout, first: usize, pins: &[Pin], origin: P) -> Vec<Option<u32>> {
    const PIN_REACH: f64 = 12.0;
    let mut out = vec![None; l.buildings.len() - first];
    let mut on: Vec<Option<usize>> = vec![None; pins.len()];
    for (i, pin) in pins.iter().enumerate() {
        let at = sub(pin.p, origin);
        on[i] = (0..out.len()).find(|&k| out[k].is_none() && contains(&l.buildings[first + k].poly, at));
        if let Some(k) = on[i] {
            out[k] = Some(pin.id);
        }
    }
    for (i, pin) in pins.iter().enumerate() {
        if on[i].is_some() {
            continue;
        }
        let at = sub(pin.p, origin);
        let edge = |poly: &[P]| (0..poly.len()).map(|e| seg_dist(at, poly[e], poly[(e + 1) % poly.len()])).fold(f64::MAX, f64::min);
        on[i] = (0..out.len()).filter(|&k| out[k].is_none()).map(|k| (edge(&l.buildings[first + k].poly), k)).filter(|x| x.0 <= PIN_REACH).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))).map(|x| x.1);
        if let Some(k) = on[i] {
            out[k] = Some(pin.id);
        }
    }
    for (pin, k) in pins.iter().zip(on) {
        let Some(k) = k else { continue };
        if let Some(fi) = pin.func.as_deref().and_then(catalog::index_of) {
            let b = &mut l.buildings[first + k];
            (b.func, b.name) = (Some(fi as u16), pin.name.clone());
            if CATALOG[fi].key == "graveyard" {
                b.structure = Structure::Open;
            }
        }
    }
    out
}

/// What a block laid out anew may hold as businesses: shops, workshops, inns, taverns and the like
/// (not what a town has one of: its hall, castle, temples, guilds, mills).
fn trade(f: &catalog::Func) -> bool {
    let shops = ["alchemist", "apothecary", "magic_shop", "scribe", "bookseller", "cartographer", "fortune_teller", "bathhouse", "curiosities", "gambling_den", "undertaker", "guard_post", "pawnbroker"];
    (matches!(f.category, "food & lodging" | "arms & gear" | "luxury & finance" | "crafts & transport") && !matches!(f.key, "mill" | "caravanserai" | "auction_house" | "bank")) || shops.contains(&f.key)
}

/// About this share of a block's homes are businesses when it is laid out anew, by its ward.
fn business_share(w: Ward) -> f64 {
    match w {
        Ward::Merchant | Ward::Plaza => 0.14,
        Ward::Craft => 0.10,
        Ward::Docks => 0.08,
        Ward::Military => 0.06,
        Ward::Common => 0.04,
        Ward::Noble | Ward::Slum => 0.03,
        _ => 0.0,
    }
}

/// What a generated fill's storeys can be set to (`BlockFill::Generated::height`): one storey
/// each, the ward's usual, one more than usual.
pub const HEIGHTS: [&str; 3] = ["low", "usual", "tall"];

/// The storeys of the homes of a block of ward `w` laid out anew (`l.buildings[first..]`, but
/// those pinned), as a town of that size rolls them per ward (`assign_functions`).
fn home_storeys(l: &mut Layout, first: usize, w: Ward, tier: Tier, key: u64, pinned: &[Option<u32>]) {
    for k in first..l.buildings.len() {
        let b = &mut l.buildings[k];
        if pinned.get(k - first).is_some_and(|p| p.is_some()) || b.func.is_some() || b.keep.is_some() || b.structure != Structure::Roofed {
            continue;
        }
        let r = unit(hash2(key, k as i64, 1));
        b.floors = match w {
            Ward::Noble | Ward::Merchant | Ward::Castle => 2 + (r < 0.4) as u8,
            Ward::Slum | Ward::Common | Ward::Craft | Ward::Docks => 1 + (tier >= Tier::City && r < 0.5) as u8,
            _ => 1,
        };
    }
}

/// A block's storeys as its fill's `height` asks (after `businesses`): "low" one storey each,
/// "tall" one more than rolled (to 4); none or "usual" as rolled. Those pinned are left be.
fn height_storeys(l: &mut Layout, first: usize, height: Option<&str>, pinned: &[Option<u32>]) {
    let set = |f: u8| match height {
        Some("low") => 1,
        Some("tall") => (f + 1).min(4),
        _ => f,
    };
    for k in first..l.buildings.len() {
        let b = &mut l.buildings[k];
        if pinned.get(k - first).is_none_or(|p| p.is_none()) && b.keep.is_none() && b.structure == Structure::Roofed {
            b.floors = set(b.floors);
        }
    }
}

/// Businesses for a block of ward `w` laid out anew (`l.buildings[first..]`, `pinned` as `pin_on`
/// gave): about `business_share` of its homes become a business of that ward the town's size has
/// (more likely the more of it a town of that size has), on the lot that suits it best (as
/// `assign_functions` picks), with its storeys; a pinned building is left as pinned. Named after,
/// with the town's others (`build_hand`).
fn businesses(l: &mut Layout, first: usize, w: Ward, tier: Tier, on_water: bool, key: u64, pinned: &[Option<u32>]) {
    let free = |k: usize| pinned.get(k - first).is_none_or(|p| p.is_none());
    let homes: Vec<usize> = (first..l.buildings.len()).filter(|&k| free(k) && l.buildings[k].func.is_none() && l.buildings[k].keep.is_none() && l.buildings[k].structure == Structure::Roofed).collect();
    let have = l.buildings[first..].iter().filter(|b| b.func.is_some()).count();
    let want = (crate::core::floor(homes.len() as f64 * business_share(w) + unit(hash2(key, 0, 1))) as usize).saturating_sub(have);
    let menu: Vec<(usize, f64)> = CATALOG
        .iter()
        .enumerate()
        .filter(|(_, f)| tier >= f.min_tier && (!f.water || on_water) && trade(f))
        .filter_map(|(fi, f)| f.wards.iter().position(|x| *x == w).map(|pos| (fi, (1 + f.counts[tier as usize]) as f64 / (1 + pos) as f64)))
        .collect();
    if menu.is_empty() || want == 0 {
        return;
    }
    let mut rng = Pcg32::new(key, 53);
    let areas: Vec<f64> = homes.iter().map(|&k| area(&l.buildings[k].poly).abs()).collect();
    let max_area = areas.iter().cloned().fold(1.0, f64::max);
    let mut taken = vec![false; homes.len()];
    let total: f64 = menu.iter().map(|m| m.1).sum();
    for _ in 0..want.min(homes.len()) {
        let mut r = rng.next_f64() * total;
        let fi = menu.iter().find(|m| {
            r -= m.1;
            r < 0.0
        });
        let fi = fi.map_or(menu[menu.len() - 1].0, |m| m.0);
        let f = &CATALOG[fi];
        let mut best: Option<(f64, usize)> = None;
        for j in (0..homes.len()).filter(|&j| !taken[j]) {
            let size = areas[j] / max_area;
            let score = if f.big { 6.0 * size } else { -2.0 * size } - if areas[j] < 500.0 { 100.0 } else { 0.0 } + 1.5 * rng.next_f64();
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, j));
            }
        }
        let Some((_, j)) = best else { break };
        taken[j] = true;
        let b = &mut l.buildings[homes[j]];
        b.func = Some(fi as u16);
        b.floors = match f.key {
            "inn" | "tavern" => 2 + (rng.next_f64() < 0.5) as u8,
            "warehouse" | "shrine" => 1,
            _ => 1 + (tier >= Tier::Town && rng.next_f64() < 0.6) as u8,
        };
    }
}

/// The fill of open ground of kind `open` (its style by name, else the kind's usual): its ways in
/// along `fronts`, its looks' key, a cemetery's style, farmland's stream.
fn open_fill(open: OpenKind, style: Option<&str>, fronts: &[super::fill::Front], key: u64, cem: impl Fn() -> super::cemetery::Style, rng: &mut dyn FnMut() -> Pcg32, tier: Tier) -> Fill {
    let plaza = style.and_then(|n| PLAZAS.iter().copied().find(|k| k.name() == n));
    let parkk = style.and_then(ParkKind::parse);
    let ways: Vec<(P, f64)> = fronts.iter().map(|f| (f.n, f.c)).collect();
    let park = |kind: ParkKind| Fill::Park { kind, ways: ways.clone(), key, cemetery: (kind == ParkKind::Cemetery).then(&cem) };
    match open {
        OpenKind::Plaza => Fill::Square(plaza.unwrap_or(if tier >= Tier::City { PlazaKind::Civic } else { PlazaKind::MarketDay })),
        OpenKind::Market => Fill::Square(plaza.filter(|k| k.is_market()).unwrap_or(PlazaKind::Market)),
        OpenKind::Lawn => Fill::Square(PlazaKind::Green),
        OpenKind::Park => park(parkk.unwrap_or(ParkKind::Common)),
        OpenKind::Garden => park(parkk.unwrap_or(ParkKind::Formal)),
        OpenKind::Cemetery => park(ParkKind::Cemetery),
        OpenKind::Farmland => Fill::Farm { class: 0, rng: rng() },
    }
}
