//! Settlement layouts, generated on demand from T0 (pure functions of world + settlement).
//!
//! Towns and cities (after watabou's Medieval Fantasy City Generator): Voronoi patches on a
//! spiral, relaxed; the inner ones are wards (plaza, castle on high ground, temples by the
//! plaza, docks on water, noble quarters by the castle, merchants on the main streets, craft
//! and common elsewhere), the outer ring is farmland with slums by the gates. Walls follow the
//! inner boundary with gates where the roads arrive; main streets run from each gate to the
//! plaza along patch edges. Each ward is cut into lots by recursive splits across the long
//! axis; buildings are lots shrunk by an alley gap. Villages grow along their roads.
//!
//! Every building gets a ground-floor level (`pad_ft`, for interiors) and a function from the
//! catalog: required ones first (an inn in every village,
//! the whole catalog in a metropolis), then residential.

pub mod belfry;
mod castle;
pub use castle::Report as CastleReport;
mod towerhouse;
pub mod catalog;
pub use crate::core::geom;
pub use crate::core::clip;
mod rows;
/// QA only (`towncheck rows`): rows of lots along a street, not used by generation.
pub use rows::{QaRow, QaRowOut, qa_bands, qa_rows};
pub mod mesh;
pub mod moat;
pub mod sites;
pub mod palace;
pub mod roof;
pub mod squares;
pub mod stairs;
pub mod temple;
pub mod wards;
pub mod hand;
pub mod handedit;
pub mod wallstyle;
pub mod outline;
mod curtain;
pub mod wallside;
pub mod gatehouse;
pub mod handwall;
pub mod harbour;
pub mod village;
// v63 countryside
pub mod fields;
// v63 countryside: hedgerows, orchards, copses, windmills, water wheels.
pub mod rural;
// R5 towns
pub mod cemetery;
pub mod culture;
pub mod members;
pub mod added;
pub mod cut;
pub mod fence;
pub use members::{JOIN_FT, Owner, Why};
pub mod horses;
pub mod yurts;
// R5 towns: districts drawn by hand, and renames kept when a patch is laid out anew.
mod districts;
mod fill;
pub mod places;
mod carry;
pub mod wallgates;
// R5 towns: a hand castle's keep and yard buildings changed by hand (`Created::pieces`).
pub mod pieces;
mod layouts;
mod ground;
mod lined;
mod plan;
mod seeds;
mod wallring;
mod build;
mod approach;
mod quays;
mod bridges;
mod quarters;
mod names;
pub use layouts::*;
pub use ground::*;
pub use lined::*;
pub(crate) use plan::*;
use build::*;
pub use quarters::*;
pub use names::*;

use std::cell::RefCell;
use std::collections::{BTreeMap, BinaryHeap};
use std::rc::Rc;

use castle::castle;
pub use castle::{DONJON_LOW, KeepKind, keep_tops, plain_keep_top};
use catalog::{CATALOG, Naming, RESIDENTIAL, Ward};
use fill::{ALLEY_HALF_FT, Block, District, Fill, build_block, farm, fill_block, park, split_field, square_kind};
pub use fill::{Front, qa_fill};
use geom::*;
use mesh::Mesh;

use crate::World;
use crate::core::hash::{FastMap, FastSet};
use crate::core::rng::{Pcg32, hash2, hash3, signed, unit};
use crate::t0::T0;
use crate::t0::names::Namer;
use crate::t0::settle::{Settlement, Tier};

#[derive(Clone, Debug)]
pub struct Building {
    /// Footprint (world ft), convex (buildings drawn by hand: any simple polygon).
    pub poly: Vec<P>,
    pub ward: Ward,
    /// Catalog index; `None` for a residence.
    pub func: Option<u16>,
    /// Index into `catalog::RESIDENTIAL` when `func` is `None`.
    pub residential: u8,
    pub name: Option<String>,
    pub floors: u8,
    /// Ground-floor pad elevation (ft).
    pub pad_ft: f32,
    pub structure: Structure,
    /// The roof drawn by hand (`None`: as its kind has it, see `interior::battlements`).
    pub roof: Option<RoofStyle>,
    /// Roof colour (index into the battlemap's tints), else picked.
    pub tint: Option<u8>,
    /// Roof material and shape set by hand (`roof::Material`, `roof::Shape` ids), else picked
    /// (`roof::look`).
    pub material: Option<u8>,
    pub shape: Option<u8>,
    /// Roof colour variant 0-4 of its material (`roof::SHADES`; 2 the usual one), else picked.
    pub shade: Option<u8>,
    /// Its id (`b:<layout>:<id>`): its place in the layout as generated, kept when buildings
    /// before it are taken away (`Layout::building`).
    pub id: u32,
    /// A keep's pieces (world ft, each convex): its body first, then wings, a forebuilding and its
    /// towers; a temple's: its body, its transept, its apse (`temple`); `poly` is their union. Empty
    /// for every other building, and once its outline is drawn by hand.
    pub parts: Vec<Part>,
    /// A castle's keep (`castle::place_keep`), of the kind it was laid out as: the castle itself,
    /// whatever its outline (with its pieces or redrawn by hand) or storeys.
    pub keep: Option<KeepKind>,
    /// (R5 M20) Standing on a deck over the water (a hand bridge's or pier's shop, a lined
    /// bridge's house): the deck's centre line (world ft), which its door faces. No cellar.
    pub deck: Option<[P; 2]>,
}

/// A piece of a building's footprint (`Building::parts`).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub poly: Vec<P>,
    pub kind: PartKind,
}

/// What a piece of a keep (or a temple: `temple`) is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    /// The main block (a great tower, a hall, a shell keep's ring).
    Body,
    /// A tower standing out from the body's walls, round or square.
    Tower { round: bool },
    /// A lower block before the entrance.
    Forebuilding,
    /// A range joined to the body (an L or a T; a temple's transept across it).
    Wing,
    /// A temple's round end (`temple`): a half circle on the body's end, a square of the body with it.
    Apse,
}

/// A mound raised under a keep (world ft): its flat top round `at` out to `top_r`, easing down to
/// the ground by `foot_r`, its top `top` ft high (the keep's pad); a path up it along `way` (unit,
/// toward the court's gate), where the shell keep has its gate.
#[derive(Clone, Copy, Debug)]
pub struct Motte {
    pub at: P,
    pub top_r: f64,
    pub foot_r: f64,
    pub top: f32,
    pub way: P,
}

/// A roof chosen for a building drawn by hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofStyle {
    Hip,
    Battlements,
    /// Sloping up from every side to the middle (a cone on a round tower).
    Cone,
}

/// What stands on a footprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    /// Walls and a roof (enterable).
    Roofed,
    /// An open-air enclosure (graveyard).
    Open,
    /// Broken walls, no roof.
    Ruin,
}

impl Building {
    pub fn label(&self) -> &'static str {
        match self.func {
            Some(f) => CATALOG[f as usize].name,
            None => RESIDENTIAL[self.residential as usize],
        }
    }
}

/// Named district kinds (watabou's DistrictType): what a district grew from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuarterKind {
    Center,
    Castle,
    Docks,
    Bridge,
    Gate,
    Bank,
    Park,
    Sprawl,
    Regular,
}

impl QuarterKind {
    pub fn name(self) -> &'static str {
        match self {
            QuarterKind::Center => "center",
            QuarterKind::Castle => "castle",
            QuarterKind::Docks => "docks",
            QuarterKind::Bridge => "bridge",
            QuarterKind::Gate => "gate",
            QuarterKind::Bank => "riverbank",
            QuarterKind::Park => "park",
            QuarterKind::Sprawl => "sprawl",
            QuarterKind::Regular => "residential",
        }
    }
}

/// A named district: a run of neighbouring patches grown from an anchor.
#[derive(Clone, Debug)]
pub struct Quarter {
    pub name: String,
    pub kind: QuarterKind,
    /// Its patches (world ft).
    pub patches: Vec<Vec<P>>,
    /// Label baseline (world ft), left to right: a gentle curve along the long axis.
    pub label: Vec<P>,
}

#[derive(Clone, Debug)]
pub struct Layout {
    /// Settlement index in `T0::settlements`.
    pub index: u32,
    pub center: P,
    pub radius: f64,
    pub tier: Tier,
    /// On the sea, a lake or a river (water trades are guaranteed only then).
    pub on_water: bool,
    /// City walls (open where water guards the edge) and castle curtain walls.
    pub walls: Vec<Vec<P>>,
    pub towers: Vec<P>,
    pub gates: Vec<P>,
    /// R5 towns: each of `gates`' corner on the plan when the town is walled (else none): its
    /// `gt:` id (`places`).
    pub gate_spots: Vec<u32>,
    /// R5 towns (M18b): each of `interior::towers`' `t:` number where the town's wall was edited
    /// by hand (else none: its place in the list), and what the hand edit came to.
    pub tower_ids: Vec<u32>,
    pub hand_wall: wallgates::HandWall,
    /// Main streets from the gates to the plaza.
    pub streets: Vec<Vec<P>>,
    /// Streets drawn as roads (world ft): approaches from each road's end to its gate (road
    /// class 0–2), paved streets (3: the strip between a block and its patch edge, one per
    /// side), and paved main streets and streets over bridges (4). (points, class, width ft)
    pub roads: Vec<(Vec<P>, u8, f64)>,
    /// Open spaces: market and other squares, quays, greens, yards.
    pub plazas: Vec<Plaza>,
    /// Parks: formal gardens and commons, each a block of its own.
    pub parks: Vec<Park>,
    pub fields: Vec<Vec<P>>,
    /// v63 countryside: what each of `fields` is (furrows, use, bound; `fields::assign`), in step.
    pub field_info: Vec<fields::FieldInfo>,
    /// Built-up patches (streets are the ground inside these that no building covers).
    pub districts: Vec<Vec<P>>,
    /// Built blocks (ward patches minus their streets): drawn instead of buildings when
    /// zoomed out.
    pub blocks: Vec<Vec<P>>,
    pub buildings: Vec<Building>,
    /// Bridge decks over the river (plank rectangles).
    pub bridges: Vec<Vec<P>>,
    /// Piers at the docks.
    pub piers: Vec<Vec<P>>,
    /// Towers flanking the gates (larger than wall towers).
    pub gate_towers: Vec<P>,
    /// Towers of castles set by hand on a town's ward (and whether each flanks a gate): after
    /// the others, so no other tower's `t:` id moves.
    pub extra_towers: Vec<(P, bool)>,
    /// Plaza monuments (plinth footprints).
    pub monuments: Vec<Vec<P>>,
    /// Named districts (towns and up).
    pub quarters: Vec<Quarter>,
    /// Castle curtains: centre and radius (world ft), so their towers know which way is in (an
    /// inner bailey's after its castle's outer one).
    pub castles: Vec<(P, f64)>,
    /// Towers of `towers` built square (a castle's): their centre and the way their sides run
    /// (unit); every other tower is round.
    pub square_towers: Vec<(P, P)>,
    /// Every wall tower's shape by its walls' style (item 104, `wallstyle::shape_towers`): its centre,
    /// shape (`wallstyle::TowerShape`, a castle's tower `| 16`) and the way its sides run (a D: the way
    /// it faces out). Gate towers are round.
    pub tower_forms: Vec<(P, u8, P)>,
    /// The place's walls by its culture group, tier and wealth (`wallstyle::WallStyle::of`).
    pub wall_style: wallstyle::WallStyle,
    /// A star-angled wall's bastions (`outline::bastions`: each outline from a flank's foot round its
    /// point to the other's, then the corner it stands at), and how many corners it dropped.
    pub bastions: Vec<Vec<P>>,
    pub bastions_dropped: u32,
    /// Mounds raised under shell keeps.
    pub mottes: Vec<Motte>,
    /// The town's castle patch and its market square's (the patch the main streets lead to), as
    /// built.
    pub castle_patch: Option<u32>,
    pub plaza_patch: Option<u32>,
    /// How the castle's patch was cut to its size (`castle::CUT_*`: 0 none tried, 1 cut with its
    /// leftover built on, else why not), and the free-standing buildings in its yard.
    pub castle_cut: u8,
    pub castle_yard: u8,
    /// Bounding box of everything (world ft): x0, y0, x1, y1.
    pub bbox: [f64; 4],
    /// A point-of-interest site (ruin, tower, roadside inn), not a settlement.
    pub site: bool,
    /// A hand castle's piece edits that found no building here (`pieces::apply`): their keys.
    pub pieces_aside: Vec<String>,
    /// Ways underground (see `under`).
    pub entrances: Vec<Entrance>,
    /// Bare packed earth levelled to a height (ft): a camp's clearing.
    pub yards: Vec<(Vec<P>, f32)>,
    /// Battlemap objects the layout sets out itself (a camp's tents, fire, bedrolls).
    pub props: Vec<Prop>,
    /// Flights of steps on steep streets (cities and up; `stairs`).
    pub stairs: Vec<Stair>,
    /// The temples' patches a close fits on (its temple and churchyard, ringed by houses along at
    /// least 65% of the block's streets: `squares::ringed`), for the ward editor.
    pub close_room: Vec<u32>,
    /// The capital's palace's forecourt before its front door (`palace::approach`).
    pub forecourts: Vec<palace::Forecourt>,
    /// The ground a temple's or a monastery's lots held (world ft), by its id: a temple's lot it stands
    /// on in pieces (`temple::shape_all`), a monastery's own lot and those it took in (`palace::merge`).
    /// What of it the building leaves open is its own: no other building's door opens onto it.
    pub grounds: Vec<(u32, Vec<P>)>,
    /// R5 towns (115): a steppe town's yurts, by building id: the lot each stands on, its trodden
    /// ground, a pen beside some (`yurts::finish`; their horses, carts and haystacks in `props`).
    pub yurts: Vec<yurts::Yurt>,
    /// The settlement's culture (a site's: the nearest settlement's), its roofs' seed and age
    /// (base weathering, 0-255): `roof::look`.
    pub culture: u8,
    pub roof_seed: u64,
    pub roof_age: u8,
    /// Castles' moats (v63 castles: `moat`).
    pub moats: Vec<moat::Moat>,
    /// A village's plots: its cottages' tofts and crofts, its churchyard or shrine's yard, its manor's
    /// yard (`village`).
    pub crofts: Vec<village::Plot>,
    /// v63 countryside: water mills' wheels (middle, the way the mill's wall runs, the mill to its left;
    /// `rural`).
    pub wheels: Vec<(P, P)>,
    /// R5 M20: a metropolis's lined bridge (`lined_bridge`): its widened span over the water (a
    /// deck, also in `bridges`) and the one flat level (ft) it and the houses on it stand at.
    pub lined: Option<(Vec<P>, f32)>,
    /// R5 M20: a big port's breakwaters (`harbour`), the boats moored by its piers (any place's),
    /// and whether it is a big port: `None` not one, `Some("")` with a breakwater, else why not.
    pub moles: Vec<harbour::Mole>,
    pub moored: Vec<harbour::Moored>,
    pub port: Option<&'static str>,
}

/// One street piece of a flight of steps (world ft): its centreline from the bottom up, its
/// width, and the flight's rise (ft); the pieces of one flight (a street's strips and main
/// street side by side) share `flight`.
#[derive(Clone, Debug)]
pub struct Stair {
    pub pts: Vec<P>,
    pub width: f64,
    pub rise: f32,
    pub flight: u32,
}

/// What an open space is for, which decides how the battlemap dresses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlazaKind {
    /// A market square: stalls in blocks round the well (busy: most of the square).
    Market,
    /// A market with few stalls and wide lanes.
    MarketSparse,
    /// A market day's huddle of stalls round the well; the rest of the square open.
    MarketDay,
    /// A square without a market: the well and the square's features only (cities and up).
    Civic,
    /// A quay along the water: cargo, no well or stalls.
    Quay,
    /// A green: a village's, round its well; in a town, a lawn where streets meet (`squares`).
    Green,
    /// A site's yard: paved, nothing set out.
    Yard,
    /// A temple's close: the middle of its block, open round the temple and its churchyard, houses
    /// along its streets.
    TempleClose,
    /// A well square where streets meet (`squares`).
    WellSquare,
    /// A fountain square where streets meet.
    FountainSquare,
    /// A crossroads widened where streets meet (a signpost, a lamp, a trough).
    Crossroads,
    /// A castle's bailey, inside its curtain: an open yard of grass (no wild trees or boulders),
    /// built on only by hand.
    Bailey,
}

impl PlazaKind {
    pub fn is_market(self) -> bool {
        matches!(self, PlazaKind::Market | PlazaKind::MarketSparse | PlazaKind::MarketDay)
    }

    /// Grass rather than paving (in towns: a green, a temple's close).
    pub fn is_lawn(self) -> bool {
        matches!(self, PlazaKind::Green | PlazaKind::TempleClose)
    }

    /// Its name as agents and the ward editor give it.
    pub fn name(self) -> &'static str {
        match self {
            PlazaKind::Market => "market",
            PlazaKind::MarketSparse => "market_sparse",
            PlazaKind::MarketDay => "market_day",
            PlazaKind::Civic => "civic",
            PlazaKind::Quay => "quay",
            PlazaKind::Green => "green",
            PlazaKind::Yard => "yard",
            PlazaKind::TempleClose => "temple_close",
            PlazaKind::WellSquare => "well",
            PlazaKind::FountainSquare => "fountain",
            PlazaKind::Crossroads => "crossroads",
            PlazaKind::Bailey => "bailey",
        }
    }
}

/// An open space (world ft): its outline and what it is for.
#[derive(Clone, Debug)]
pub struct Plaza {
    pub pts: Vec<P>,
    pub kind: PlazaKind,
    /// R5 towns: what its `sq:` id is keyed by (`places`).
    pub spot: places::Spot,
}

/// How a park is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParkKind {
    /// A formal garden: cross axes and a walk round it, a basin in the middle, hedged parterres.
    Formal,
    /// A common: winding gravel paths to a hub, a pond on the low ground, clumps of trees.
    Common,
    /// R5 towns: a cemetery (`cemetery`): a low boundary, walks, rows of graves by the culture group.
    Cemetery,
    /// R5 towns (114): horse pens and a horse market (`horses`): fenced corrals, a trodden market
    /// ground, hitching rails, troughs (a steppe town's in place of a formal garden).
    Horses,
}

impl ParkKind {
    /// Its name as agents and the ward editor give it.
    pub fn name(self) -> &'static str {
        match self {
            ParkKind::Formal => "formal",
            ParkKind::Common => "common",
            ParkKind::Cemetery => "cemetery",
            ParkKind::Horses => "horses",
        }
    }

    pub fn parse(name: &str) -> Option<ParkKind> {
        [ParkKind::Formal, ParkKind::Common, ParkKind::Cemetery, ParkKind::Horses].into_iter().find(|k| k.name() == name)
    }
}

/// A park (world ft): a whole block, no lots on it (`battlemap::park_layout` lays it out). With
/// it, which of its sides face a street (`fronts`, by edge: its ways in), the ground it is planned
/// on (where a common's pond, stream and raised tier go), and the key its looks are rolled from (its
/// patch and how many times it was laid out again by hand, so a corner moved changes no look).
#[derive(Clone, Debug)]
pub struct Park {
    pub pts: Vec<P>,
    pub kind: ParkKind,
    pub fronts: Vec<bool>,
    pub ground: ParkGround,
    pub key: u64,
    /// R5 towns: a cemetery's look (its culture group, wealth) and patch (`cemetery`).
    pub cemetery: Option<cemetery::Style>,
    /// R5 towns: what its `sq:` id is keyed by (`places`).
    pub spot: places::Spot,
}

/// The points of a 20-ft grid about the town's centre inside a polygon (local ft).
fn lattice(poly: &[P]) -> Vec<P> {
    let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
    let f = |v: f64| crate::core::floor(v / 20.0) as i64;
    let (i0, j0, i1, j1) = (f(x0), f(y0), f(x1), f(y1));
    (j0..=j1).flat_map(|j| (i0..=i1).map(move |i| [(i as f64 + 0.5) * 20.0, (j as f64 + 0.5) * 20.0])).filter(|&p| contains(poly, p)).collect()
}

/// The ground a park is planned on, every `PARK_GROUND_FT` over its bounds from `origin` (world ft):
/// heights (ft) and whether the water is there (or a river within 80 ft).
#[derive(Clone, Debug)]
pub struct ParkGround {
    pub origin: P,
    pub nx: usize,
    pub ny: usize,
    pub h: Vec<f32>,
    pub wet: Vec<bool>,
    /// On the water or a river's channel itself.
    pub water: Vec<bool>,
}

/// `ParkGround`'s spacing (ft).
pub const PARK_GROUND_FT: f64 = 20.0;

impl ParkGround {
    /// The planned ground at `p` (world ft), bilinear, clamped to the grid.
    pub fn at(&self, p: P) -> f64 {
        let u = ((p[0] - self.origin[0]) / PARK_GROUND_FT).clamp(0.0, (self.nx - 1) as f64);
        let v = ((p[1] - self.origin[1]) / PARK_GROUND_FT).clamp(0.0, (self.ny - 1) as f64);
        let (i, j) = ((crate::core::floor(u) as usize).min(self.nx.saturating_sub(2)), (crate::core::floor(v) as usize).min(self.ny.saturating_sub(2)));
        let (fu, fv) = (u - i as f64, v - j as f64);
        let at = |a: usize, b: usize| self.h[(b.min(self.ny - 1)) * self.nx + a.min(self.nx - 1)] as f64;
        let top = at(i, j) + (at(i + 1, j) - at(i, j)) * fu;
        let bottom = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu;
        top + (bottom - top) * fv
    }

    /// Whether any of the grid's points within `r` ft of `p` is on the water.
    pub fn wet_near(&self, p: P, r: f64) -> bool {
        (0..self.ny).any(|j| (0..self.nx).any(|i| self.wet[j * self.nx + i] && dist(self.point(i, j), p) <= r))
    }

    /// Whether the water itself (not a river's banks) is within `r` ft of `p` (by the grid's points).
    pub fn water_near(&self, p: P, r: f64) -> bool {
        let reach = r + PARK_GROUND_FT;
        let lo = |v: f64, o: f64| crate::core::floor((v - reach - o) / PARK_GROUND_FT).max(0.0) as usize;
        let hi = |v: f64, o: f64, n: usize| (crate::core::ceil((v + reach - o) / PARK_GROUND_FT).max(0.0) as usize).min(n.saturating_sub(1));
        let (i0, j0, i1, j1) = (lo(p[0], self.origin[0]), lo(p[1], self.origin[1]), hi(p[0], self.origin[0], self.nx), hi(p[1], self.origin[1], self.ny));
        (j0..=j1).any(|j| (i0..=i1).any(|i| self.water[j * self.nx + i] && dist(self.point(i, j), p) <= reach))
    }

    /// The grid's point (i, j) (world ft).
    pub fn point(&self, i: usize, j: usize) -> P {
        [self.origin[0] + i as f64 * PARK_GROUND_FT, self.origin[1] + j as f64 * PARK_GROUND_FT]
    }
}

/// A battlemap object placed by a layout: where (world ft), facing (rad), kind, variant, size.
#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub at: P,
    pub rot: f32,
    pub kind: crate::battle::Kind,
    pub variant: u8,
    pub scale: f32,
}

/// A way underground: the opening (world ft), the way the passage runs in (unit), and the
/// kind of site below.
#[derive(Clone, Copy, Debug)]
pub struct Entrance {
    pub at: P,
    pub dir: P,
    pub kind: crate::under::UnderKind,
    /// Its id (`u:<layout>:<id>`), as `Building::id`.
    pub id: u32,
}

impl Layout {
    /// Ids from list places (at the end of generating).
    fn number(&mut self) {
        self.number_buildings();
        self.number_entrances();
    }

    /// Buildings' ids from their places in the list.
    fn number_buildings(&mut self) {
        for (i, b) in self.buildings.iter_mut().enumerate() {
            b.id = i as u32;
        }
    }

    /// Ways underground's ids from their places in the list.
    fn number_entrances(&mut self) {
        for (i, e) in self.entrances.iter_mut().enumerate() {
            e.id = i as u32;
        }
    }

    /// The place in `buildings` of the building with this id (ids rise along the list).
    pub fn building_index(&self, id: usize) -> Option<usize> {
        if self.buildings.get(id).is_some_and(|b| b.id as usize == id) {
            return Some(id);
        }
        self.buildings.binary_search_by_key(&id, |b| b.id as usize).ok()
    }

    /// The building with this id.
    pub fn building(&self, id: usize) -> Option<&Building> {
        self.building_index(id).map(|i| &self.buildings[i])
    }

    /// The way the sides of the tower standing at `t` run, if it is square (`square_towers`).
    pub fn square_tower(&self, t: P) -> Option<P> {
        self.square_towers.iter().find(|s| (s.0[0] - t[0]).abs() + (s.0[1] - t[1]).abs() < 1e-6).map(|s| s.1)
    }

    /// The shape of the wall tower at `t` and the way its sides run (a D: the way it faces); round
    /// where it has none (`tower_forms`).
    pub fn tower_form(&self, t: P) -> (wallstyle::TowerShape, P) {
        let found = self.tower_forms.iter().find(|s| (s.0[0] - t[0]).abs() + (s.0[1] - t[1]).abs() < 1e-6);
        found.map_or((wallstyle::TowerShape::Round, [1.0, 0.0]), |s| (wallstyle::TowerShape::from_u8(s.1 & 15), s.2))
    }

    /// The walls' style a tower or a stretch of wall at `p` is built in: a castle's (`WallStyle::castle`)
    /// for a castle's tower, or within a castle's curtain, else the place's.
    pub fn wall_style_at(&self, p: P) -> wallstyle::WallStyle {
        let castle = match self.tower_forms.iter().find(|s| (s.0[0] - p[0]).abs() + (s.0[1] - p[1]).abs() < 1e-6) {
            Some(f) => f.1 & 16 != 0,
            None => self.castles.iter().any(|(c, r)| dist(*c, p) <= r + 1.0),
        };
        if castle { self.wall_style.castle() } else { self.wall_style }
    }

    /// A layout with nothing laid out yet.
    /// Its roofs' culture, seed and age (`roof::look`): the age by the place's own hash, a site's
    /// older.
    pub(crate) fn set_roofs(&mut self, world: &World, t0: &T0, culture: u8) {
        self.culture = culture;
        self.roof_seed = hash2(world.stream("town.roof"), rng_key(t0, self.index as usize), 0);
        self.roof_age = (unit(hash2(self.roof_seed, 1, 0)) * 90.0) as u8;
    }

    pub(crate) fn empty(index: u32, center: P, radius: f64, tier: Tier) -> Layout {
        Layout {
            index,
            center,
            radius,
            tier,
            on_water: false,
            pieces_aside: Vec::new(),
            walls: Vec::new(),
            towers: Vec::new(),
            gates: Vec::new(),
            gate_spots: Vec::new(),
            tower_ids: Vec::new(),
            hand_wall: Default::default(),
            streets: Vec::new(),
            roads: Vec::new(),
            plazas: Vec::new(),
            parks: Vec::new(),
            fields: Vec::new(),
            field_info: Vec::new(),
            districts: Vec::new(),
            blocks: Vec::new(),
            buildings: Vec::new(),
            bridges: Vec::new(),
            piers: Vec::new(),
            gate_towers: Vec::new(),
            extra_towers: Vec::new(),
            monuments: Vec::new(),
            quarters: Vec::new(),
            castles: Vec::new(),
            square_towers: Vec::new(),
            tower_forms: Vec::new(),
            wall_style: wallstyle::WallStyle::default(),
            bastions: Vec::new(),
            bastions_dropped: 0,
            mottes: Vec::new(),
            castle_patch: None,
            plaza_patch: None,
            castle_cut: 0,
            castle_yard: 0,
            bbox: [0.0; 4],
            site: false,
            entrances: Vec::new(),
            yards: Vec::new(),
            props: Vec::new(),
            stairs: Vec::new(),
            close_room: Vec::new(),
            forecourts: Vec::new(),
            yurts: Vec::new(),
            grounds: Vec::new(),
            culture: 0,
            roof_seed: 0,
            roof_age: 0,
            moats: Vec::new(),
            crofts: Vec::new(),
            wheels: Vec::new(),
            lined: None,
            moles: Vec::new(),
            moored: Vec::new(),
            port: None,
        }
    }

    /// The way underground with this id.
    pub fn entrance(&self, id: usize) -> Option<&Entrance> {
        if self.entrances.get(id).is_some_and(|e| e.id as usize == id) {
            return self.entrances.get(id);
        }
        self.entrances.iter().find(|e| e.id as usize == id)
    }
}

pub(crate) struct Site<'a> {
    t0: &'a T0,
    center: P,
    /// The ground it is planned on (a town asks for it many thousands of times).
    plan: PlanGround<'a>,
    /// Standing water (sea or lake) within the settlement's T0 neighbourhood: shore banks
    /// only matter then.
    lakeside: bool,
    /// River channel pieces near the settlement: (a, b, half width) in local coords.
    river: Vec<(P, P, f64)>,
    /// Pieces by grid cell (`RIVER_CELL` ft), each listed in every cell its bbox touches.
    river_grid: FastMap<(i64, i64), Vec<u32>>,
    river_max_hw: f64,
    /// Network roads near the settlement as ~20-ft chords (a, b, half width; local), sampled
    /// on first use.
    roads: std::cell::OnceCell<(Vec<(P, P, f64)>, FastMap<(i64, i64), Vec<u32>>)>,
    /// How far from the centre (ft) the settlement's layout can reach.
    reach: f64,
    /// The sea's level (ft): a beach is within 3 ft over it (`battlemap::beach`).
    sea: f64,
}

impl<'a> Site<'a> {
    fn new(t0: &'a T0, center: P, lattice_ft: f64, layout_reach: f64, river: Vec<(P, P, f64)>, sea: f64) -> Site<'a> {
        let mut river_grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
        let mut river_max_hw = 0.0f64;
        for (i, &(a, b, hw)) in river.iter().enumerate() {
            river_max_hw = river_max_hw.max(hw);
            let (x0, x1) = (crate::core::floor((a[0].min(b[0]) - hw) / RIVER_CELL) as i64, crate::core::floor((a[0].max(b[0]) + hw) / RIVER_CELL) as i64);
            let (y0, y1) = (crate::core::floor((a[1].min(b[1]) - hw) / RIVER_CELL) as i64, crate::core::floor((a[1].max(b[1]) + hw) / RIVER_CELL) as i64);
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    river_grid.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        // Shore banks reach at most about a T0 cell from wet cells.
        let reach = 12_000.0 + 2.0 * t0.cell_ft;
        let step = 0.25 * t0.cell_ft;
        let n = (2.0 * reach / step).ceil() as i64;
        let lakeside = (0..=n).any(|j| (0..=n).any(|i| t0.sample_lake(center[0] - reach + i as f64 * step, center[1] - reach + j as f64 * step).0 > crate::t0::hydro::DRY));
        // Ground nodes out to where shore banks matter; the valley over the layout's reach and
        // a little past it.
        let e = layout_reach + 1_000.0;
        let plan = PlanGround::new(
            t0,
            [center[0] - reach, center[1] - reach, center[0] + reach, center[1] + reach],
            [center[0] - e, center[1] - e, center[0] + e, center[1] + e],
            lattice_ft,
            VALLEY_CELL,
        );
        Site { t0, center, plan, lakeside, river, river_grid, river_max_hw, roads: std::cell::OnceCell::new(), reach: layout_reach, sea }
    }
}

impl Site<'_> {
    fn world(&self, p: P) -> P {
        add(p, self.center)
    }
    /// How far from `c` along `u` (by `sign`) a deck `width` wide first reaches ground that is dry
    /// all the way across it (clear of standing water and 2 ft clear of the river), searching from
    /// `from` in 5-ft steps: its corners stand on the banks.
    fn dry_reach(&self, c: P, u: P, width: f64, sign: f64, from: f64) -> f64 {
        let v = [-u[1], u[0]];
        let wet_across = |t: f64| {
            (0..=4).any(|k| {
                let p = add(add(c, mul(u, sign * t)), mul(v, (k as f64 / 4.0 - 0.5) * width));
                self.wet_standing(p) || self.near_river(p, 2.0)
            })
        };
        let mut t = from;
        while t < 600.0 && wet_across(t) {
            t += 5.0;
        }
        t
    }
    /// The ground the settlement is planned on: the T0 surface in its river valleys, as the
    /// terrain under its pad is reset to (`PlanGround`).
    fn height(&self, p: P) -> f64 {
        let w = self.world(p);
        self.plan.ground(w[0], w[1], self.lakeside) + self.plan.depth(w[0], w[1])
    }
    /// The world's islets in reach (`t0::islets`), local: piers keep off them.
    fn islets(&self) -> Vec<Vec<P>> {
        let (c, r) = (self.center, self.reach);
        crate::t0::islets::near(self.t0, [c[0] - r, c[1] - r, c[0] + r, c[1] + r]).map(|i| i.pts.iter().map(|&p| sub(p, c)).collect()).collect()
    }
    /// Sea or lake (not the river channel): inside the drawn shoreline (the same water mask
    /// the terrain renders), so buildings, quays and piers agree with the visible shore.
    fn wet_standing(&self, p: P) -> bool {
        let w = self.world(p);
        // The water lookup gives a level wherever a nearby cell is wet; it is only water
        // where that level is above the ground (as the terrain draws it).
        let level = self.t0.sample_water(w[0], w[1]);
        level > crate::t0::hydro::DRY && (level as f64) > self.height(p)
    }
    fn wet(&self, p: P) -> bool {
        self.wet_standing(p) || self.near_river(p, 0.0)
    }
    /// Planned below the level of the sea or lake beside it (in the shore band, where the cell
    /// isn't wet): the drawn ground there is banked up to the shore, not the ground planned.
    fn below_shore(&self, p: P) -> bool {
        let w = self.world(p);
        let (level, _) = self.t0.sample_lake(w[0], w[1]);
        level > crate::t0::hydro::DRY && (level as f64) > self.height(p)
    }
    fn near_river(&self, p: P, margin: f64) -> bool {
        if self.river.is_empty() {
            return false;
        }
        let r = self.river_max_hw + margin;
        let (x0, x1) = (crate::core::floor((p[0] - r) / RIVER_CELL) as i64, crate::core::floor((p[0] + r) / RIVER_CELL) as i64);
        let (y0, y1) = (crate::core::floor((p[1] - r) / RIVER_CELL) as i64, crate::core::floor((p[1] + r) / RIVER_CELL) as i64);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if let Some(ids) = self.river_grid.get(&(cx, cy)) {
                    for &i in ids {
                        let (a, b, hw) = self.river[i as usize];
                        if seg_dist(p, a, b) < hw + margin {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    /// A deck (pier) within 6 ft of a network road anywhere round its outline (every 5 ft).
    fn deck_on_road(&self, poly: &[P]) -> bool {
        let m = poly.len();
        (0..m).any(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            let n = (dist(a, b) / 5.0).ceil().max(1.0) as usize;
            (0..n).any(|j| self.near_network_road(lerp(a, b, j as f64 / n as f64), 6.0))
        }) || self.near_network_road(centroid(poly), 6.0)
    }
    /// Within `margin` ft of a network road's edge (`p` local; the curve every ~20 ft).
    fn near_network_road(&self, p: P, margin: f64) -> bool {
        let roads = self.roads.get_or_init(|| {
            let (c, r) = (self.center, self.reach);
            let mut out = Vec::new();
            for (ri, k) in self.t0.roads.segments_near([c[0] - r, c[1] - r, c[0] + r, c[1] + r], 0.0) {
                let rc = &self.t0.roads.roads[ri as usize];
                let n = ((rc.s[k as usize + 1] - rc.s[k as usize]) / 20.0).ceil().clamp(1.0, 64.0) as usize;
                let pts: Vec<P> = (0..=n).map(|j| sub(rc.eval(k as usize, j as f64 / n as f64, 5.0, self.t0.cell_ft).p, c)).collect();
                out.extend(pts.windows(2).map(|q| (q[0], q[1], 0.5 * rc.class.width_ft())));
            }
            // By `ROAD_CELL`-ft cell, padded by the widest road's half width and a deck's margin.
            let mut grid: FastMap<(i64, i64), Vec<u32>> = FastMap::default();
            for (i, &(a, b, hw)) in out.iter().enumerate() {
                let m = hw + 10.0;
                for cy in crate::core::floor((a[1].min(b[1]) - m) / ROAD_CELL) as i64..=crate::core::floor((a[1].max(b[1]) + m) / ROAD_CELL) as i64 {
                    for cx in crate::core::floor((a[0].min(b[0]) - m) / ROAD_CELL) as i64..=crate::core::floor((a[0].max(b[0]) + m) / ROAD_CELL) as i64 {
                        grid.entry((cx, cy)).or_default().push(i as u32);
                    }
                }
            }
            (out, grid)
        });
        let cell = (crate::core::floor(p[0] / ROAD_CELL) as i64, crate::core::floor(p[1] / ROAD_CELL) as i64);
        roads.1.get(&cell).into_iter().flatten().any(|&i| {
            let (a, b, hw) = roads.0[i as usize];
            seg_dist(p, a, b) < hw + margin
        })
    }
    /// Direction (unit, local) a network road travels as it arrives at its end `e` (local).
    fn road_heading(&self, e: P) -> Option<P> {
        let w = self.world(e);
        for (ri, k) in self.t0.roads.segments_near([w[0] - 5.0, w[1] - 5.0, w[0] + 5.0, w[1] + 5.0], 0.0) {
            let rc = &self.t0.roads.roads[ri as usize];
            let n = rc.pts.len();
            for (kk, t, back) in [(0usize, 0.0, 0.08), (n.saturating_sub(2), 1.0, 0.92)] {
                if k as usize != kk {
                    continue;
                }
                let end = rc.eval(kk, t, 5.0, self.t0.cell_ft).p;
                if dist(end, w) < 1.0 {
                    let before = rc.eval(kk, back, 5.0, self.t0.cell_ft).p;
                    let d = sub(end, before);
                    let l = len(d);
                    return (l > 1e-6).then(|| mul(d, 1.0 / l));
                }
            }
        }
        None
    }
    /// Nearest point of the river centreline within ~400 ft: (point, unit tangent, half width).
    fn nearest_river(&self, p: P) -> Option<(P, P, f64)> {
        let r = self.river_max_hw + 400.0;
        let (x0, x1) = (crate::core::floor((p[0] - r) / RIVER_CELL) as i64, crate::core::floor((p[0] + r) / RIVER_CELL) as i64);
        let (y0, y1) = (crate::core::floor((p[1] - r) / RIVER_CELL) as i64, crate::core::floor((p[1] + r) / RIVER_CELL) as i64);
        let mut best: Option<(f64, P, P, f64)> = None;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                for &i in self.river_grid.get(&(cx, cy)).into_iter().flatten() {
                    let (a, b, hw) = self.river[i as usize];
                    let ab = sub(b, a);
                    let l2 = dot(ab, ab).max(1e-12);
                    let t = (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0);
                    let q = add(a, mul(ab, t));
                    let d = dist(p, q);
                    if best.is_none_or(|x| d < x.0) {
                        best = Some((d, q, mul(ab, 1.0 / crate::core::sqrt(l2)), hw));
                    }
                }
            }
        }
        best.map(|(_, q, t, hw)| (q, t, hw))
    }

    /// Buildable: dry, off the channel, not too steep.
    fn buildable(&self, poly: &[P]) -> bool {
        let c = centroid(poly);
        if self.wet(c) || self.near_river(c, 12.0) {
            return false;
        }
        let (lo, hi) = self.relief(poly);
        hi - lo < 6.0 && self.dry_outline(poly, 10.0)
    }
    /// `buildable` for a big building raised on a terrace (a monastery round its cloister): its ground
    /// rising up to 10 ft across it.
    fn terraced(&self, poly: &[P]) -> bool {
        let c = centroid(poly);
        if self.wet(c) || self.near_river(c, 12.0) {
            return false;
        }
        let (lo, hi) = self.relief(poly);
        hi - lo < 10.0 && self.dry_outline(poly, 10.0)
    }
    /// No point of the outline (every `step` ft or less) on water: a channel at least `step`
    /// wide cannot cross the polygon unseen.
    fn dry_outline(&self, poly: &[P], step: f64) -> bool {
        let m = poly.len();
        (0..m).all(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            let n = (dist(a, b) / step).ceil().max(1.0) as usize;
            (0..n).all(|j| !self.wet(lerp(a, b, j as f64 / n as f64)))
        })
    }
    /// Lowest and highest ground over a polygon (corners, edge midpoints, centre).
    fn relief(&self, poly: &[P]) -> (f64, f64) {
        let m = poly.len();
        let pts = (0..m).flat_map(|k| [poly[k], lerp(poly[k], poly[(k + 1) % m], 0.5)]).chain(std::iter::once(centroid(poly)));
        pts.map(|p| self.height(p)).fold((f64::MAX, f64::MIN), |(lo, hi), h| (lo.min(h), hi.max(h)))
    }
    /// The share of a polygon on the water or a river's channel (points every 20 ft inside it).
    /// (The points are on a 20-ft grid about the town's centre: a corner moved shifts none of them.)
    fn wet_share(&self, poly: &[P]) -> f64 {
        let pts = lattice(poly);
        let wet = pts.iter().filter(|&&p| self.wet(p) || self.near_river(p, 10.0)).count();
        if pts.is_empty() { 1.0 } else { wet as f64 / pts.len() as f64 }
    }
    /// The share of a polygon within 80 ft of a river (its drawn banks, which the plan's ground doesn't
    /// see, may stand well over it).
    fn river_share(&self, poly: &[P]) -> f64 {
        let pts = lattice(poly);
        let near = pts.iter().filter(|&&p| self.near_river(p, 80.0)).count();
        if pts.is_empty() { 0.0 } else { near as f64 / pts.len() as f64 }
    }
    /// The grade from the lowest to the highest dry point of a 20-ft grid inside a polygon (as a
    /// common's layout judges a slope).
    fn fall(&self, poly: &[P]) -> f64 {
        let hs: Vec<(P, f64)> = lattice(poly).into_iter().filter(|&p| !self.wet(p) && !self.near_river(p, 10.0)).map(|p| (p, self.height(p))).collect();
        let lo = hs.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
        let hi = hs.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1));
        match (lo, hi) {
            (Some(lo), Some(hi)) if dist(lo.0, hi.0) > 20.0 => (hi.1 - lo.1) / dist(lo.0, hi.0),
            _ => 0.0,
        }
    }
    /// The steepest grade between neighbouring dry points of a 20-ft grid about the town's centre
    /// inside a polygon (a cliff across it, where its mean grade may be gentle).
    fn max_grade(&self, poly: &[P]) -> f64 {
        let at = |i: i64, j: i64| -> Option<f64> {
            let p = [(i as f64 + 0.5) * 20.0, (j as f64 + 0.5) * 20.0];
            (contains(poly, p) && !self.wet(p) && !self.near_river(p, 10.0)).then(|| self.height(p))
        };
        let (x0, y0, x1, y1) = poly.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, p| (a.0.min(p[0]), a.1.min(p[1]), a.2.max(p[0]), a.3.max(p[1])));
        let f = |v: f64| crate::core::floor(v / 20.0) as i64;
        let (i0, j0, i1, j1) = (f(x0), f(y0), f(x1), f(y1));
        let mut worst: f64 = 0.0;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let Some(h) = at(i, j) else { continue };
                for (di, dj) in [(1, 0), (0, 1)] {
                    if let Some(g) = at(i + di, j + dj) {
                        worst = worst.max((g - h).abs() / 20.0);
                    }
                }
            }
        }
        worst
    }
    /// Lowest and highest dry ground over a polygon (as `relief`, the water's points left out).
    fn dry_relief(&self, poly: &[P]) -> (f64, f64) {
        let m = poly.len();
        (0..m)
            .flat_map(|k| [poly[k], lerp(poly[k], poly[(k + 1) % m], 0.5)])
            .chain(std::iter::once(centroid(poly)))
            .filter(|&p| !self.wet(p) && !self.near_river(p, 10.0))
            .map(|p| self.height(p))
            .fold((f64::MAX, f64::MIN), |(lo, hi), h| (lo.min(h), hi.max(h)))
    }
    /// `grade` from the dry points only (a sea's floor or a river's channel by a waterfront block is
    /// no slope).
    fn dry_grade(&self, poly: &[P]) -> f64 {
        let m = poly.len();
        let c = centroid(poly);
        let width = 2.0 * poly.iter().map(|p| dist(*p, c)).fold(0.0, f64::max);
        let hs: Vec<f64> = (0..m).flat_map(|k| [poly[k], lerp(poly[k], poly[(k + 1) % m], 0.5)]).chain(std::iter::once(c)).filter(|&p| !self.wet(p) && !self.near_river(p, 10.0)).map(|p| self.height(p)).collect();
        if hs.len() < 3 {
            return 0.0;
        }
        let (lo, hi) = hs.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
        (hi - lo) / width.max(1.0)
    }
    /// Mean grade across a polygon: its relief over its width.
    fn grade(&self, poly: &[P]) -> f64 {
        let c = centroid(poly);
        let width = 2.0 * poly.iter().map(|p| dist(*p, c)).fold(0.0, f64::max);
        let (lo, hi) = self.relief(poly);
        (hi - lo) / width.max(1.0)
    }
    fn pad(&self, poly: &[P]) -> f32 {
        let c = centroid(poly);
        let hs = poly.iter().map(|p| self.height(*p)).chain(std::iter::once(self.height(c)));
        let (sum, n) = hs.fold((0.0, 0.0), |(s, n), h| (s + h, n + 1.0));
        (sum / n) as f32
    }
}

pub fn generate(world: &World, t0: &T0, index: usize) -> Layout {
    generate_with(world, t0, index, None, None)
}

/// A settlement's layout; a town with the ward editor's changes (`Edits.towns`) applied between
/// planning it and building it, its buildings and ways underground matched to the generated
/// layout's (`wards::inherit`).
/// `survey`: every street judged for steps, towns' too (`stairs::survey`).
fn generate_with(world: &World, t0: &T0, index: usize, edit: Option<&crate::world::TownEdit>, survey: Option<&mut Vec<stairs::Profile>>) -> Layout {
    let s = culture::settlement(world, t0, index).expect("layout of a settlement");
    generate_in(world, t0, index, &s, edit, survey)
}

/// `generate_with` settlement `s` (layout `index`'s, in the culture it is laid out in).
fn generate_in(world: &World, t0: &T0, index: usize, s: &Settlement, edit: Option<&crate::world::TownEdit>, survey: Option<&mut Vec<stairs::Profile>>) -> Layout {
    let center = [s.x, s.y];
    let r = urban_radius(s.tier, s.population);
    let (site, road_ends, mut rng) = prepare(world, t0, index);
    let mut l = Layout::empty(index as u32, center, r, s.tier);
    l.set_roofs(world, t0, s.culture);
    l.wall_style = wallstyle::WallStyle::of(s);
    // (How far apart the plan's patches are: how far a function the edit lost may move.)
    let mut spacing = 0.0;
    if s.tier == Tier::Village {
        village(&site, s, r, &road_ends, &mut rng, &mut l);
    } else {
        let plan = match edit {
            Some(_) => wards::plan(world, index, s.culture, || plan_town(&site, s, r, &road_ends, &mut rng)),
            None => Rc::new(plan_town(&site, s, r, &road_ends, &mut rng)),
        };
        let mut ch = edit.map(|e| wards::changes(&plan, e, center).0).unwrap_or_default();
        // (R5 towns, M18: the ground of a gate taken out keeps off the wall as it was generated.)
        if !ch.wall.remove.is_empty() {
            wallgates::kept_off(&plan, &mut ch.wall, &generated_layout(world, t0, index), center);
        }
        spacing = plan.spacing;
        let hand = build_town(&site, s, r, &plan, &ch, &mut l, survey);
        // (Read by a conversion to a hand town alone: `hand::convert`.)
        hand::cap_plan(index, &plan);
        walls_off_river(&site, &mut l, &hand);
        // (A castle's curtain meeting the town's wall, or a run cut at a river: stopped at the
        // line it meets, as the town's own pieces, `outline::tidy`.)
        outline::tidy(&mut l.walls, &mut l.towers);
    }
    // A pin's ward names go to its districts in order, the central one first.
    if let Some(pin) = s.pin.and_then(|i| world.file.sketch.strokes.get(i as usize)) {
        // (Not to districts made by hand: R5 towns, `districts`.)
        let made = edit.map_or(0, |e| e.districts.len());
        let mut order: Vec<usize> = (0..l.quarters.len().saturating_sub(made)).collect();
        order.sort_by_key(|&q| (l.quarters[q].kind != QuarterKind::Center, q));
        for (&q, name) in order.iter().zip(&pin.wards) {
            if !name.trim().is_empty() {
                l.quarters[q].name = name.trim().to_string();
            }
        }
    }
    drop_walled_in(&mut l);
    // (Towers added since the walls were laid, as at a river's banks, shaped too.)
    wallstyle::shape_rest(&mut l, [0.0, 0.0]);
    let on_water = s.coastal || s.river || !site.river.is_empty();
    l.on_water = on_water;
    if edit.is_none() {
        assign_functions(&site, s, on_water, &mut rng, &mut l);
        // (Ids from their places now: the lots a palace takes in leave theirs unused, every other
        // building keeps its own.)
        l.number_buildings();
        palace::merge(&site, &mut l, [0.0, 0.0], s.capital, s.tier, &[], None);
        palace::approach(&site, &mut l, [0.0, 0.0], s.capital, &[], None);
        palace::gardens(&site, &mut l, [0.0, 0.0]);
    }
    // Local → world.
    let tw = |p: &mut P| *p = add(*p, center);
    for b in &mut l.buildings {
        b.poly.iter_mut().chain(b.parts.iter_mut().flat_map(|p| p.poly.iter_mut())).for_each(tw);
    }
    for (pts, _, _) in &mut l.roads {
        pts.iter_mut().for_each(tw);
    }
    for st in &mut l.stairs {
        st.pts.iter_mut().for_each(tw);
    }
    for poly in l.walls.iter_mut().chain(&mut l.bastions).chain(&mut l.streets).chain(l.plazas.iter_mut().map(|p| &mut p.pts)).chain(&mut l.fields).chain(&mut l.districts).chain(&mut l.blocks).chain(&mut l.bridges).chain(&mut l.piers) {
        poly.iter_mut().for_each(tw);
    }
    l.towers.iter_mut().chain(&mut l.gates).chain(&mut l.gate_towers).for_each(tw);
    for poly in &mut l.monuments {
        poly.iter_mut().for_each(tw);
    }
    for q in &mut l.quarters {
        q.patches.iter_mut().flatten().chain(&mut q.label).for_each(tw);
    }
    for c in &mut l.castles {
        tw(&mut c.0);
    }
    for t in &mut l.extra_towers {
        tw(&mut t.0);
    }
    for t in &mut l.square_towers {
        tw(&mut t.0);
    }
    for t in &mut l.tower_forms {
        tw(&mut t.0);
    }
    for m in &mut l.mottes {
        tw(&mut m.at);
    }
    for pk in &mut l.parks {
        pk.pts.iter_mut().chain([&mut pk.ground.origin]).for_each(tw);
    }
    for f in &mut l.forecourts {
        f.front.iter_mut().chain(&mut f.yard).chain(&mut f.way).for_each(tw);
    }
    for (_, g) in &mut l.grounds {
        g.iter_mut().for_each(tw);
    }
    for m in &mut l.moats {
        m.shift(center);
    }
    for c in &mut l.crofts {
        c.pts.iter_mut().for_each(tw);
    }
    if let Some((d, _)) = &mut l.lined {
        d.iter_mut().for_each(tw);
    }
    // Temples on their lots in pieces (world ft, so that a town laid out anew shapes them alike), a shrine
    // shut in a block's middle moved first to a home with a way out.
    if edit.is_none() {
        temple::reach_streets(&mut l, None);
        temple::shape_all(&mut l, None);
        belfry::raise_all(&mut l, None);
    }
    if let Some(e) = edit {
        let generated = generated_layout(world, t0, index);
        let kept = palace::follow(&site, &mut l, &generated, center, s.capital);
        wards::inherit(world, t0, e, &generated, spacing, &mut l);
        palace::merge(&site, &mut l, center, s.capital, s.tier, &kept, Some(&generated));
        palace::approach(&site, &mut l, center, s.capital, &kept, Some(&generated));
        palace::gardens(&site, &mut l, center);
        temple::reach_streets(&mut l, Some(&generated));
        temple::shape_all(&mut l, Some(&generated));
        belfry::raise_all(&mut l, Some(&generated));
    }
    round_off(&mut l, Some(&site));
    fields::crofts_off_sand(&site, &mut l);
    fields::assign(&site, s.seed, &mut l);
    rural::place(&site, s, &mut l);
    // (A town edited by hand keeps its windmills as generated, ids and all, where they still stand
    // clear of its buildings: in their places by id, before the ones the edit made.)
    if edit.is_some() {
        let generated = generated_layout(world, t0, index);
        l.buildings.retain(|b| !rural::windmill(b));
        for w in generated.buildings.iter().filter(|b| rural::windmill(b)) {
            let c = geom::centroid(&w.poly);
            let at = l.buildings.partition_point(|b| b.id < w.id);
            if l.buildings.get(at).is_none_or(|b| b.id != w.id) && l.buildings.iter().all(|b| geom::dist(geom::centroid(&b.poly), c) > 40.0) {
                l.buildings.insert(at, w.clone());
            }
        }
    }
    // R5 towns: the cemeteries' tombs, after every other building (`cemetery::raise`).
    cemetery::raise(&site, &mut l, edit.map(|_| generated_layout(world, t0, index)).as_deref());
    // Sewers and catacombs under cities.
    if s.tier >= Tier::City {
        l.entrances = crate::under::city_entrances(&l, t0, s.seed);
    }
    // R5 M20: the houses on a lined bridge, after every other building (an edited town with the
    // same lined bridge keeps them as generated, ids and all).
    match edit {
        Some(_) if l.lined.is_some() && generated_layout(world, t0, index).lined == l.lined => {
            let generated = generated_layout(world, t0, index);
            for h in generated.buildings.iter().filter(|b| b.deck.is_some()) {
                let at = l.buildings.partition_point(|b| b.id < h.id);
                let hb = bbox(&h.poly);
                if l.buildings.get(at).is_none_or(|b| b.id != h.id) && l.buildings.iter().all(|b| !boxes_meet(grow(bbox(&b.poly), 1.5), hb) || poly_gap(&b.poly, &h.poly) > 0.5) {
                    l.buildings.insert(at, h.clone());
                }
            }
        }
        _ => bridge_houses(index, &mut l),
    }
    // R5 M20: the watchtowers on its islets, after every other building (an edited town keeps
    // them as generated, ids and all, where they stand clear of its bridges).
    match edit {
        None => islet_towers(t0, index, &mut l),
        Some(_) => {
            let generated = generated_layout(world, t0, index);
            for w in generated.buildings.iter().filter(|b| is_watchtower(b)) {
                let at = l.buildings.partition_point(|b| b.id < w.id);
                if l.buildings.get(at).is_none_or(|b| b.id != w.id) && l.bridges.iter().all(|d| poly_gap(d, &w.poly) >= 4.0) {
                    l.buildings.insert(at, w.clone());
                }
            }
        }
    }
    // R5 M20: a port's breakwater and its lighthouse (after every other building), its boats, its
    // harbour master's office.
    harbour::harbour(&site, s, index, &mut l, edit.map(|_| generated_layout(world, t0, index)).as_deref());
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in l.buildings.iter().flat_map(|b| b.poly.iter()).chain(l.fields.iter().flatten()).chain(l.walls.iter().flatten()).chain(l.plazas.iter().flat_map(|p| p.pts.iter())).chain(l.parks.iter().flat_map(|p| p.pts.iter())).chain(l.crofts.iter().flat_map(|c| c.pts.iter())) {
        bb = [bb[0].min(p[0]), bb[1].min(p[1]), bb[2].max(p[0]), bb[3].max(p[1])];
    }
    for m in &l.moats {
        bb = [bb[0].min(m.bbox[0]), bb[1].min(m.bbox[1]), bb[2].max(m.bbox[2]), bb[3].max(m.bbox[3])];
    }
    l.bbox = if bb[0] <= bb[2] { bb } else { [center[0], center[1], center[0], center[1]] };
    match edit {
        Some(_) => wards::inherit_entrances(&generated_layout(world, t0, index), &mut l),
        None => l.number_entrances(),
    }
    if let Some(e) = edit {
        wallgates::finish(world, t0, index, e, &mut l);
    }
    l
}

/// What a town decides before anything is built on it (`plan_town`): its patch mesh (smoothed
/// along the wall and the main streets), walls, gates, main streets, bridges, wards and named
/// districts. `build_town` may move the mesh's corners first, as watabou's Warp tools do:
/// walls, streets and blocks follow the corners they run along, while what was decided (wards,
/// district groups, names, the towers along each wall run) stays as planned on the unmoved mesh,
/// so a moved corner changes only the patches round it.
struct TownPlan {
    /// Patch spacing (ft): how far a corner may be moved from where it was planned.
    spacing: f64,
    mesh: Mesh,
    /// Faces round each vertex.
    vf: Vec<Vec<usize>>,
    /// The town's patches are the first `n_inner` faces; the rest are its outskirts.
    n_inner: usize,
    /// Faces on the water (or too small to keep).
    wet: Vec<bool>,
    face_ok: Vec<bool>,
    /// Town patch sides facing a patch on the water with water beyond them (corner pairs, lower
    /// first): a quay on the shore, not the wall.
    shore: FastSet<(usize, usize)>,
    /// Town patches on the water amid the town (no run of them to the outskirts): no wall round them.
    enclosed: Vec<bool>,
    /// Corners that never move (on the water or a river).
    pinned: Vec<bool>,
    walled: bool,
    /// Wall runs (a closed ring starts at a plain corner): corners, closed, and the towers along
    /// each stretch (`wall_pieces_spaced`). Planned for unwalled towns too (walls set by hand).
    wall_runs: Vec<(Vec<usize>, bool, Vec<usize>)>,
    gates: Vec<usize>,
    /// Where roads arrive (class), the way each comes in, and its gate.
    ends: Vec<(P, u8)>,
    heading_of: Vec<Option<P>>,
    gate_of: Vec<Option<usize>>,
    /// The town's patches as first laid out (bbox, polygon): where an approach road becomes a
    /// town street.
    town_faces: Vec<([f64; 4], Vec<P>)>,
    /// The market square's face (a rhombus plaza), and the face the main streets lead to.
    plaza_face: Option<usize>,
    plaza: Option<usize>,
    /// Ways over the river: (bank corner, river corner, bank corner).
    crossing_links: Vec<(usize, usize, usize)>,
    /// Main streets: corners from each gate to the plaza.
    paths: Vec<Vec<usize>>,
    /// The crossings bridged, and whether a main street uses each.
    bridged: Vec<((usize, usize, usize), bool)>,
    /// Patch edges along main streets and bridges (corner pairs, lower first).
    main_edges: FastSet<(usize, usize)>,
    ward: Vec<Option<Ward>>,
    /// A patch's district group (its first patch), else `usize::MAX`.
    group: Vec<usize>,
    /// Per-patch random streams (`roll`), and the seed of block and lot parameters.
    pseed: u64,
    base_seed: u64,
    /// Named districts and their patches.
    quarters: Vec<(Quarter, Vec<usize>)>,
    /// The town's other open spaces (`squares::plan`): squares where streets meet (corner,
    /// kind), and temple patches kept as a close round their temple.
    squares: Vec<(usize, PlazaKind)>,
    closes: Vec<usize>,
    /// Patches too steep to build on (parks), and a rich city's palace garden.
    steep: Vec<bool>,
    palace: Option<usize>,
    /// Why a patch can't be a park (`PARK_WET`, `PARK_SHUT`, `PARK_OUT`, `PARK_SMALL`, `PARK_CLIFF`),
    /// where it can't.
    park_refused: Vec<Option<&'static str>>,
}

/// Why a patch can't be laid out as a park (generated or set by hand).
pub(super) const PARK_WET: &str = "too much of that patch is on the water for a park (more than 15%)";
pub(super) const PARK_SHUT: &str = "no street comes to that patch: a park needs a way in from a street";
pub(super) const PARK_OUT: &str = "parks are laid out inside the town";
pub(super) const PARK_SMALL: &str = "that patch is too small for a park (its block under 9,000 sq ft or 60 ft across)";
pub(super) const PARK_CLIFF: &str = "a cliff crosses that patch: too steep for a park";
pub(super) const PARK_SLIVER: &str = "too small for a park once its streets are cut: built on as a common ward's";

impl TownPlan {
    fn usable(&self, i: usize) -> bool {
        i < self.n_inner && !self.wet[i] && self.face_ok[i]
    }

    fn inner(&self, j: i32) -> bool {
        j >= 0 && (j as usize) < self.n_inner
    }

    fn in_town(&self, p: P) -> bool {
        self.town_faces.iter().any(|(bb, poly)| p[0] >= bb[0] && p[0] <= bb[2] && p[1] >= bb[1] && p[1] <= bb[3] && contains(poly, p))
    }

    fn main_edge(&self, a: usize, b: usize) -> bool {
        self.main_edges.contains(&(a.min(b), a.max(b)))
    }

    fn roll(&self, i: usize, stage: i64) -> f64 {
        unit(hash3(self.pseed, i as i64, stage, 0x9a7c))
    }
}

struct QuarterInput<'a> {
    cells: &'a [Labeled],
    ward: &'a [Option<Ward>],
    n_inner: usize,
    plaza: Option<usize>,
    castle: Option<usize>,
    gates: &'a [P],
    /// Bridge crossing points (river vertices).
    crossings: &'a [P],
    /// Per patch edge: how readily a district grows across it (0 wall or river, 0.9 main
    /// street, 1 otherwise).
    edge_w: &'a [Vec<f64>],
    /// Patches on the water.
    landing: &'a [bool],
}
