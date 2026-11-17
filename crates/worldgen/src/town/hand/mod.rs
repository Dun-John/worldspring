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
use super::wards::{PER_PATCH, ROLLS};

mod pack;
mod convert;
mod plot;
mod graph;
mod build;

pub use pack::*;
pub use convert::*;
pub use plot::*;
pub use graph::*;
pub use build::*;
pub use crate::model::town::{BlockFill, OpenKind};
use crate::model::town::{is_false, is_zero};

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
    /// (Roadmap 6 P7) The line as the generator laid it, where it came from a town's wall: kept
    /// while the line is as converted (`look.of`), whatever else changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<WallLook>,
}

/// A converted wall line's look as generated (world ft, packed): its outline by the town's style
/// ("straight", "star", "round"), its pieces with the curtains bowed out, its towers, gate towers
/// and bastions; `of` the hash of the line it was laid from (`line_of`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WallLook {
    pub of: String,
    pub outline: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub towers: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gate_towers: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bastions: Vec<String>,
}

/// A hash (hex) of what a wall line is laid from: its corners, gates and towers (not its name
/// nor its look).
pub(super) fn line_of(w: &WallLine) -> String {
    let mut h = crate::core::hash::Fnv64::default();
    for p in w.pts.iter().chain(&w.gates) {
        h.write(&p[0].to_bits().to_le_bytes());
        h.write(&p[1].to_bits().to_le_bytes());
    }
    h.write(&[w.closed as u8, 0xfd]);
    h.write(&w.spacing.to_bits().to_le_bytes());
    for t in &w.towers {
        h.write(&t.to_le_bytes());
    }
    hex(h.finish())
}

/// Each wall run's look as the generator laid it in `l` (world ft): the pieces, towers, gate
/// towers and bastions nearest it (within a curtain's bow), those of a castle's own curtain inside
/// its patch (`castled`) left out.
fn wall_looks(l: &Layout, runs: &[Vec<P>], castled: &[Vec<P>]) -> Vec<Option<WallLook>> {
    const NEAR: f64 = 75.0;
    let line_dist = |r: &[P], p: P| r.windows(2).map(|s| seg_dist(p, s[0], s[1])).fold(f64::MAX, f64::min);
    let nearest = |p: P| (0..runs.len()).map(|k| (line_dist(&runs[k], p), k)).filter(|x| x.0 <= NEAR).min_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1);
    let deep = |p: P| castled.iter().any(|c| contains(c, p) && (0..c.len()).all(|k| seg_dist(p, c[k], c[(k + 1) % c.len()]) > 3.0));
    let outline = match l.wall_style.outline {
        super::wallstyle::Outline::Straight => "straight",
        super::wallstyle::Outline::Star => "star",
        super::wallstyle::Outline::Round => "round",
    };
    let mut out: Vec<WallLook> = (0..runs.len()).map(|_| WallLook { outline: outline.into(), ..WallLook::default() }).collect();
    let (mut towers, mut gate_towers): (Vec<Vec<P>>, Vec<Vec<P>>) = (vec![Vec::new(); runs.len()], vec![Vec::new(); runs.len()]);
    for w in &l.walls {
        let mut votes = vec![0usize; runs.len()];
        for p in w.iter().filter(|p| !deep(**p)) {
            if let Some(k) = nearest(*p) {
                votes[k] += 1;
            }
        }
        if let Some((k, &v)) = votes.iter().enumerate().max_by_key(|x| (*x.1, std::cmp::Reverse(x.0)))
            && 2 * v > w.len()
        {
            out[k].walls.push(pkq(w));
        }
    }
    for (from, to) in [(&l.towers, &mut towers), (&l.gate_towers, &mut gate_towers)] {
        for t in from.iter().filter(|t| !deep(**t)) {
            if let Some(k) = nearest(*t) {
                to[k].push(*t);
            }
        }
    }
    for b in &l.bastions {
        if let Some(k) = b.last().and_then(|c| nearest(*c)) {
            out[k].bastions.push(pk(b));
        }
    }
    out.into_iter()
        .zip(towers.into_iter().zip(gate_towers))
        .map(|(mut lk, (t, g))| {
            (lk.towers, lk.gate_towers) = (pkq(&t), pkq(&g));
            (!lk.walls.is_empty()).then_some(lk)
        })
        .collect()
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
    /// The main square's block (`Layout::plaza_patch`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<u32>,
    /// The harbour as generated: piers (outlines), breakwaters (width, level, line), boats moored
    /// (x, y, bow, kind, length) and why a big port has no breakwater (`Layout::port`; "" it has
    /// one). Kept as they are, whatever the streets do.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub piers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moles: Vec<(f64, f32, String)>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub moored: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
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
    /// (Roadmap 6 P7) A castle's own works as generated (its curtains, towers, mottes), for when
    /// the town's walls are laid anew from its wall lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curtain: Option<CastleLook>,
}

/// A converted castle's works inside its block (world ft, packed as `Works`' own).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CastleLook {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub towers: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gate_towers: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub castles: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub square: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub forms: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mottes: String,
}

impl CastleLook {
    /// What of `l` (world ft) stands inside `cell` (3 ft in and more): None if nothing.
    fn of(l: &Layout, cell: &[P]) -> Option<CastleLook> {
        let deep = |p: P| contains(cell, p) && (0..cell.len()).all(|k| seg_dist(p, cell[k], cell[(k + 1) % cell.len()]) > 3.0);
        let flat = |v: Vec<f64>| pack(&v);
        let walls: Vec<String> = l.walls.iter().filter(|w| 2 * w.iter().filter(|p| deep(**p)).count() > w.len()).map(|w| pkq(w)).collect();
        let towers: Vec<P> = l.towers.iter().copied().filter(|t| deep(*t)).collect();
        let gate_towers: Vec<P> = l.gate_towers.iter().copied().filter(|t| deep(*t)).collect();
        if walls.is_empty() && towers.is_empty() {
            return None;
        }
        let at_tower = |p: P| towers.iter().chain(&gate_towers).any(|t| dist(*t, p) < 0.01);
        Some(CastleLook {
            walls,
            towers: pkq(&towers),
            gate_towers: pkq(&gate_towers),
            castles: flat(l.castles.iter().filter(|c| deep(c.0)).flat_map(|c| [c.0[0], c.0[1], c.1]).collect()),
            square: flat(l.square_towers.iter().filter(|t| at_tower(t.0)).flat_map(|t| [t.0[0], t.0[1], t.1[0], t.1[1]]).collect()),
            forms: forms_pk(&l.tower_forms.iter().copied().filter(|f| at_tower(f.0)).collect::<Vec<_>>()),
            mottes: flat(l.mottes.iter().filter(|m| deep(m.at)).flat_map(|m| [m.at[0], m.at[1], m.top_r, m.foot_r, m.top as f64, m.way[0], m.way[1]]).collect()),
        })
    }

    /// Put back into `l` (world ft).
    fn lay(&self, l: &mut Layout) {
        let nums = |s: &str| unpack(s).unwrap_or_default();
        l.walls.extend(self.walls.iter().map(|x| unpk(x)));
        l.towers.extend(unpk(&self.towers));
        l.gate_towers.extend(unpk(&self.gate_towers));
        l.castles.extend(nums(&self.castles).chunks_exact(3).map(|c| ([c[0], c[1]], c[2])));
        l.square_towers.extend(nums(&self.square).chunks_exact(4).map(|c| ([c[0], c[1]], [c[2], c[3]])));
        l.tower_forms.extend(forms_unpk(&self.forms));
        l.mottes.extend(nums(&self.mottes).chunks_exact(7).map(|c| super::Motte { at: [c[0], c[1]], top_r: c[2], foot_r: c[3], top: c[4] as f32, way: [c[5], c[6]] }));
    }
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
    /// (R6 P8) Lots: a docks ward on a sloping shore (`District::stepped`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub st: bool,
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
                Some(Refill { k: 1, w: 0, close: false, class: *class, nb, nl, nt: None, st: false, g: pack(&g) })
            }
            Fill::Lots { ward, dp, rng, thin, close } => {
                g.extend(words(rng));
                g.extend([dp.min_sq, dp.grid_chaos, dp.size_chaos, dp.block_size, r65]);
                if let Some((pts, reach)) = thin {
                    g.push(*reach);
                    g.extend(pts.iter().flat_map(|p| *p));
                }
                Some(Refill { k: 0, w: index_of(&WARDS, ward), close: *close, class: 0, nb, nl, nt: thin.as_ref().map(|t| t.0.len() as u32), st: dp.stepped, g: pack(&g) })
            }
            Fill::Square(kind) => {
                let mut g: Vec<f64> = block.iter().flat_map(|p| *p).collect();
                g.extend((40..44).map(roll));
                Some(Refill { k: 2, w: index_of(&PLAZAS, kind), close: false, class: 0, nb, nl: 0, nt: None, st: false, g: pack(&g) })
            }
            Fill::Park { kind, ways, key, .. } => {
                let mut g: Vec<f64> = block.iter().flat_map(|p| *p).collect();
                g.extend(ways.iter().flat_map(|(n, c)| [n[0], n[1], *c]));
                g.push(f64::from_bits(*key));
                Some(Refill { k: 3, w: index_of(&PARKS, kind), close: false, class: 0, nb, nl: ways.len() as u32, nt: None, st: false, g: pack(&g) })
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
                let dp = District { min_sq: d[0], grid_chaos: d[1], size_chaos: d[2], block_size: d[3], stepped: self.st };
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
