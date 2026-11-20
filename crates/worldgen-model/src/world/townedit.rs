use super::*;

/// A generated building changed by hand: removed, or given another function, storeys,
/// footprint, roof, tint or structure (each left out: as generated). `at` is the generated
/// footprint's centre: when the building at that id no longer stands there (the world was
/// generated again from a changed sketch and the town laid out anew), the edit is set aside.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BuildingEdit {
    pub at: [f64; 2],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
    /// A `town::catalog` function key or a home (`BUILDING_HOMES`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors: Option<u8>,
    /// A new footprint (world ft), as a drawn building's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poly: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<String>,
    /// Roof material (`town::roof::MATERIALS`) and shape (`town::roof::SHAPES`), else picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// Roof colour variant 0-4 of its material (2 the usual), else picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade: Option<u8>,
    /// `roofed` or `ruin`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<String>,
}

impl BuildingEdit {
    /// How far (ft) the building at its id may stand from `at` and still be the one edited.
    pub const SAME_AT_FT: f64 = 3.0;

    /// Whether it changes nothing (every option as generated).
    pub fn is_noop(&self) -> bool {
        !self.removed && self.func.is_none() && self.floors.is_none() && self.poly.is_empty() && self.roof.is_none() && self.tint.is_none() && self.material.is_none() && self.shape.is_none() && self.shade.is_none() && self.structure.is_none()
    }
}

/// A town's layout changed by hand (the ward editor). Corners of its patches moved (as watabou's
/// Warp tools move them), patches given another ward, lot size, a merge with a neighbour or a
/// fresh roll, its walls on or off. `at` is the town's centre: a town laid out somewhere else (a
/// changed sketch) sets the whole edit aside; each corner and patch also remembers where it was
/// planned, and is set aside alone when that drifts.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TownEdit {
    pub at: [f64; 2],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corners: Vec<CornerMove>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patches: Vec<PatchEdit>,
    /// Walls built or not (left out: as generated). Gates stay where the roads come in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walls: Option<bool>,
    /// Squares where streets meet, set by hand.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub squares: Vec<SquareEdit>,
    /// (R5 towns) Districts made by hand, by name, in the order they were made: each is district
    /// `d:<layout>:<generated count + k>` (`town::districts`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub districts: Vec<String>,
    /// (R5 towns, M18b) Gates and towers put in or taken out of the town's own wall
    /// (`town::wallgates`).
    #[serde(default, skip_serializing_if = "WallEdit::is_empty")]
    pub wall: WallEdit,
    /// (Roadmap 6 R4) Streets added to the town (a row's road), by id (`ts:<layout>:<id>`):
    /// laid over the built town, their ends joined to its nearest streets (`town::added`). The
    /// plan never sees them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub streets: BTreeMap<u32, AddedStreet>,
    /// (Roadmap 6 R5) The town's own streets, lanes and park paths removed by hand, by their
    /// local keys (`town::cut`): `<8 hex>` a street or lane (`tr:<layout>:<8 hex>`), `<spot>:<j>`
    /// a park's path (`pp:<layout>:<spot>:<j>`). Drawn nowhere (the ground beneath shows, a park's
    /// lawn closes over); the plan and the build never see them.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed: BTreeSet<String>,
    /// (Roadmap 6 R6) The town's generated fences and hedges changed by hand (gone, another
    /// kind, gates), by their local keys (`town::fence`): `<8 hex>` a run (`fr:<layout>:<8 hex>`),
    /// `<spot>:<j>` a park's hedge or row of topiary (`fh:<layout>:<spot>:<j>`). Drawn only: the
    /// layout never sees them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fences: BTreeMap<String, crate::model::fences::FenceChange>,
    /// (Roadmap 7, 183) Where the roads came in when the ward editor first changed the town
    /// (local ft from its middle, class, heading x and y: 0, 0 none). It is planned from these
    /// while the edit stands, so a road drawn later joins its gates instead of laying it out anew.
    /// None: planned from the roads as they are (an edit made before these were kept).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roads: Option<Vec<[f64; 5]>>,
    /// (Roadmap 7, 189) The street pattern it was laid out on then (`town::pattern::NAMES`), kept
    /// with `roads`: while the edit stands a town left on auto keeps it, so a ground or water
    /// change near it can't lay its streets out anew under the edit. None: picked as it stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// (Roadmap 6 R4) A street added to a generated town (`TownEdit::streets`): its centreline
/// (world ft, as laid), its kind (which sets only its width: it takes the town's own street look)
/// and width (else the kind's).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AddedStreet {
    pub pts: Vec<[f64; 2]>,
    #[serde(rename = "type", default)]
    pub kind: crate::model::paths::PathKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
}

impl AddedStreet {
    /// Its width (ft): as recorded, else the kind's.
    pub fn width_ft(&self) -> f64 {
        self.width.unwrap_or_else(|| self.kind.width_ft())
    }
}

/// Gates and towers of a town's own wall set by hand, by the plan's corners: gates put in at a
/// wall corner or taken out (`at`: where the corner was planned), towers put up (each with its
/// `t:` number) and taken down (by `t:` number).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WallEdit {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates_add: Vec<WallCorner>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates_remove: Vec<WallCorner>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub towers_add: Vec<HandTower>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub towers_remove: Vec<u32>,
}

impl WallEdit {
    pub fn is_empty(&self) -> bool {
        self.gates_add.is_empty() && self.gates_remove.is_empty() && self.towers_add.is_empty() && self.towers_remove.is_empty()
    }
}

/// A corner of a town's wall (`v`, the plan's vertex; `at` where it was planned, world ft).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WallCorner {
    pub v: u32,
    pub at: [f64; 2],
}

/// A tower put up by hand on a town's wall, `t:<layout>:<id>`: `f` of the way from wall corner
/// `v` (planned at `at`) to its neighbour on the wall `w`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HandTower {
    pub id: u32,
    pub v: u32,
    pub w: u32,
    pub f: f64,
    pub at: [f64; 2],
}

/// A square set by hand at a corner where streets meet (`v`, the plan's vertex; `at` where it was
/// planned, world ft): its kind (`SQUARE_KINDS`), or "none" for no square there.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SquareEdit {
    pub v: u32,
    pub at: [f64; 2],
    pub kind: String,
}

/// A patch corner (`v`, the plan's vertex) moved from where it was planned (`from`, world ft) to
/// `to`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CornerMove {
    pub v: u32,
    pub from: [f64; 2],
    pub to: [f64; 2],
}

/// A patch (`p`, the plan's face; `at` its middle as planned) set by hand: its ward
/// (`TOWN_WARDS`), lot size (`LOT_SIZES`), merged into a neighbour's district, rolled again
/// (`reroll` counts the rolls), a temple's patch kept as a close round the temple or not, a
/// park's laid out as a formal garden or a common (`PARK_KINDS`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PatchEdit {
    pub p: u32,
    pub at: [f64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ward: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lots: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_with: Option<u32>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub reroll: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub park: Option<String>,
    /// (R5 towns) The district it is put in: a generated one's id (`d:<layout>:<q>`) or the name
    /// of one made by hand (`TownEdit::districts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// Wards a patch can be given by hand (`empty`: open ground, for buildings drawn by hand).
pub const TOWN_WARDS: [&str; 13] = ["plaza", "castle", "temple", "merchant", "craft", "noble", "common", "slum", "docks", "military", "farm", "park", "empty"];

/// Lot sizes, smallest first: each a target lot area (ft²) times the ward's own.
pub const LOT_SIZES: [&str; 4] = ["small", "medium", "large", "huge"];

/// Squares a corner where streets meet can be given by hand (`town::squares::CORNER_KINDS`).
pub const SQUARE_KINDS: [&str; 4] = ["crossroads", "well", "fountain", "green"];

/// How a park can be laid out by hand (`town::ParkKind`).
pub const PARK_KINDS: [&str; 4] = ["formal", "common", "cemetery", "horses"];

impl TownEdit {
    /// How far (ft) a town, corner or patch may stand from where its edit says it was planned.
    pub const SAME_AT_FT: f64 = 1.0;

    /// Whether it changes nothing.
    pub fn is_noop(&self) -> bool {
        self.corners.is_empty() && self.patches.iter().all(PatchEdit::is_noop) && self.walls.is_none() && self.squares.is_empty() && self.wall.is_empty()
    }
}

impl PatchEdit {
    pub fn is_noop(&self) -> bool {
        self.ward.is_none() && self.lots.is_none() && self.merge_with.is_none() && self.reroll == 0 && self.close.is_none() && self.park.is_none() && self.district.is_none()
    }
}
