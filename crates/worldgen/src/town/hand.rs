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
            Some(w) => BlockFill::Generated { ward: w.name().into(), lots: lot_name(i), reroll: ch.reroll.get(&i).copied().unwrap_or(0), close: ch.close.get(&i).copied() },
        };
        let open = !matches!(fill, BlockFill::Hand);
        let block = Block {
            at: add(inside_point(&cap.cells[i]), center),
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
struct Graph {
    pts: Vec<P>,
    q: Vec<(i64, i64)>,
    /// (from, to, street id): from `a` toward `b`.
    segs: Vec<(usize, usize, u32)>,
}

/// A face of the graph: its ring (corners, anticlockwise), and per side the street piece and
/// whether the face is on its left.
struct Face {
    ring: Vec<usize>,
    side: Vec<(usize, bool)>,
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
    fn of(recs: &HandRecords, origin: P, li: &str, problems: &mut Problems) -> Graph {
        let mut g = Graph { pts: Vec::new(), q: Vec::new(), segs: Vec::new() };
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
                kept.remove(&id);
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
    static BUILT: RefCell<(u64, FastMap<u32, Rc<Layout>>)> = RefCell::new((0, FastMap::default()));
}

/// Layout `index` as its hand records lay it out, if it has a header (`base_layout`).
pub(super) fn built(world: &World, t0: &T0, index: usize) -> Option<Rc<Layout>> {
    let streets = &world.file.edits.streets;
    if streets.is_empty() || !streets.contains_key(&index.to_string()) {
        return None;
    }
    let key = index as u32;
    if let Some(l) = BUILT.with(|c| {
        let c = c.borrow();
        if c.0 == world.hash { c.1.get(&key).cloned() } else { None }
    }) {
        return Some(l);
    }
    let l = Rc::new(build_hand(world, t0, index)?.layout);
    BUILT.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != world.hash {
            *c = (world.hash, FastMap::default());
        }
        c.1.insert(key, l.clone());
    });
    Some(l)
}

/// What a block is laid out as, once its ground is known.
fn ward_of(name: &str) -> Option<Ward> {
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
    let recs = HandRecords::of(&world.file.edits.streets, index);
    let town = recs.town.as_ref()?;
    let s = super::culture::settlement(world, t0, index)?;
    let li = index.to_string();
    let origin = town.origin;
    let ground = KeptGround::of(world, t0, index, town);
    let site = &ground.site;
    let tier = Tier::from_u8(town.tier);
    let mut l = Layout::empty(index as u32, origin, super::urban_radius(tier, s.population), tier);
    l.set_roofs(world, t0, town.culture);
    l.roof_seed = u64::from_str_radix(&town.roofs, 16).unwrap_or(l.roof_seed);
    l.wall_style = super::wallstyle::WallStyle::of(&s);
    l.on_water = s.coastal || s.river || !site.river.is_empty();
    let mut problems: Problems = Vec::new();
    let graph = Graph::of(&recs, origin, &li, &mut problems);
    let (faces, outers) = graph.faces();
    let street = |k: usize| &recs.streets[&graph.segs[k].2];
    let set = |k: usize, left: bool| if left { street(k).set_l } else { street(k).set_r };
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

    // What no block is built over: buildings drawn by hand, wharves, painted areas.
    let pad = |poly: &[P], d: f64| clip::offset(&clip::region(poly), d, Join::Miter(2.0));
    let mut obstacles: Vec<Shape> = Vec::new();
    for c in world.file.edits.created.iter().filter(|c| c.kind == "building" && c.poly.len() >= 3) {
        let poly: Vec<P> = c.poly.iter().map(|p| sub(*p, origin)).collect();
        obstacles.extend(pad(&poly, 2.0));
    }
    for w in world.file.edits.wharves.values().filter(|w| w.pts.len() >= 2) {
        let pts: Vec<P> = w.pts.iter().map(|p| sub(*p, origin)).collect();
        obstacles.extend(clip::buffer(&pts, 0.5 * w.width + 2.0, 0.5 * w.width + 2.0, Join::Miter(2.0), Ends::Flat));
    }
    let painted: Vec<(u32, Vec<P>, OpenKind)> = recs.areas.iter().filter(|(_, a)| a.poly.len() >= 3).map(|(&k, a)| (k, a.poly.iter().map(|p| sub(*p, origin)).collect(), a.open)).collect();
    let mut cut: Vec<Shape> = obstacles.clone();
    for (_, poly, _) in &painted {
        cut.extend(clip::region(poly));
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
    let mut lines: BTreeMap<u32, Vec<P>> = BTreeMap::new();
    for &(u, v, id) in &graph.segs {
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
            if d <= 0.0 {
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

    // The blocks' fills.
    let meta = (index as u32, origin, 0.0, tier);
    let mut kept_bs: Vec<Building> = Vec::new();
    let mut plots: Vec<(Vec<P>, u8)> = Vec::new();
    let mut built_ground: Vec<Shape> = Vec::new();
    let cem = |id: u32| super::cemetery::Style::of(&s, id as usize, false);
    let mut out: Vec<(u32, Vec<Shape>)> = Vec::new();
    let seed_of = |hexs: &str| u64::from_str_radix(hexs, 16).unwrap_or_else(|_| hash2(0x4a4d, index as i64, 0));
    let shift = |sh: &[Shape]| -> Vec<Shape> {
        sh.iter().map(|x| Shape { outer: x.outer.iter().map(|p| add(*p, origin)).collect(), holes: x.holes.iter().map(|h| h.iter().map(|p| add(*p, origin)).collect()).collect() }).collect()
    };
    for (f, face) in faces.iter().enumerate() {
        let Some(id) = face_block[f] else { continue };
        let rec = &recs.blocks[&id];
        let g = &grounds[f];
        out.push((id, shift(g)));
        // (What a row keeps clear of: every block but one left for buildings drawn by hand.)
        if !matches!(rec.fill, BlockFill::Hand) {
            built_ground.extend(g.iter().cloned());
        }
        let spot = super::places::Spot::Patch(id);
        let bseed = seed_of(&rec.seed);
        let gn = rec.generation as i64;
        let first = l.buildings.len();
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
        if built_on {
            l.districts.push(ring.clone());
        }
        if let Some(kept) = &rec.kept {
            // (Its fill laid out once: its buildings to keep, its open ground, its plots.)
            let laid = kept.refill.as_ref().map(|r| r.lay(site, meta, Some(cem(id))));
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
            continue;
        }
        if g.is_empty() {
            continue;
        }
        match &rec.fill {
            BlockFill::Hand | BlockFill::Castle { .. } => {}
            BlockFill::Generated { ward, lots, reroll, close: _ } => {
                let Some(w) = ward_of(ward) else {
                    problems.push((format!("{li}/k{id}"), "no such ward"));
                    continue;
                };
                let salt = |h: u64| if *reroll == 0 { h } else { hash3(h, *reroll as i64, 0x7e70, 1) };
                let mut dp = District::roll(w, salt(hash2(bseed, gn, 0xd15)));
                if let Some(k) = lots.as_deref().and_then(|n| LOT_SIZES.iter().position(|x| *x == n)) {
                    dp.min_sq = District::base(w) * super::wards::LOT_FACTOR[k];
                }
                let mut rng = Pcg32::new(salt(hash2(bseed, gn, 0x107)), 47);
                l.blocks.extend(g.iter().map(|x| x.outer.clone()));
                build_block(site, Region::Region(g), &fronts, w, &dp, &mut rng, &mut l, None);
                if w == Ward::Temple && l.buildings.len() == first
                    && let Some(b) = biggest()
                {
                    super::squares::lone_temple(site, &b, unit(hash2(bseed, gn, 65)), &mut l);
                }
            }
            BlockFill::Open { open, style } => {
                let Some(b) = biggest() else { continue };
                let roll = |k: i64| unit(hash3(bseed, gn, k, 0x09e));
                let fill = open_fill(*open, style.as_deref(), &fronts, hash2(bseed, gn, 0x9a4c), || cem(id), &mut || Pcg32::new(hash2(bseed, gn, 0xfa), 61), tier);
                let mut pl = Vec::new();
                let (pz, pk) = (l.plazas.len(), l.parks.len());
                super::fill_block(site, &b, Vec::new(), fill, &roll, spot, &mut l, &mut pl);
                l.plazas[pz..].iter_mut().for_each(|p| p.spot = spot);
                l.parks[pk..].iter_mut().for_each(|p| p.spot = spot);
                plots.append(&mut pl);
            }
        }
        // (Its own ids, by the order it laid them out.)
        let base = HAND_ID as u64 + ((id as u64 * ROLLS as u64 + gn as u64 % ROLLS as u64) * PER_PATCH as u64);
        for (k, b) in l.buildings[first..].iter_mut().enumerate() {
            b.id = (base + k as u64).min(u32::MAX as u64) as u32;
        }
        if id >= ROW_BLOCK {
            problems.push((format!("{li}/k{id}"), "block number too high for its ids"));
        }
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
        let side = super::rows::Side { line, left: row.side == "l", setback: 0.5 * recs.streets[&row.street].width + 2.0, depth: row.depth };
        let first = l.buildings.len();
        let lots = super::rows::build_row(site, &side, w, &dp, &mut rng, &avoid, &mut l);
        l.blocks.extend(super::rows::bands(&side));
        for lot in &lots {
            avoid.extend(clip::region(lot));
        }
        let base = HAND_ID as u64 + ((ROW_BLOCK + r) as u64 * ROLLS as u64) * PER_PATCH as u64;
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
    for poly in l.streets.iter_mut().chain(l.plazas.iter_mut().map(|p| &mut p.pts)).chain(&mut l.fields).chain(&mut l.districts).chain(&mut l.blocks).chain(&mut l.monuments) {
        poly.iter_mut().for_each(tw);
    }
    for pk in &mut l.parks {
        pk.pts.iter_mut().chain([&mut pk.ground.origin]).for_each(tw);
    }
    l.buildings.append(&mut kept_bs);
    l.buildings.extend(loose);
    l.buildings.sort_by_key(|b| b.id);
    super::fields::assign(site, s.seed, &mut l);
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in l.buildings.iter().flat_map(|b| b.poly.iter()).chain(l.fields.iter().flatten()).chain(l.plazas.iter().flat_map(|p| p.pts.iter())).chain(l.parks.iter().flat_map(|p| p.pts.iter())).chain(l.districts.iter().flatten()) {
        bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
    }
    l.bbox = if bb[0] <= bb[2] { bb } else { [origin[0], origin[1], origin[0], origin[1]] };
    Some(HandBuilt { layout: l, blocks: out, problems })
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
