//! Building interiors, generated on demand: a pure function of (world, settlement layout,
//! building). The footprint becomes a 5-ft grid along the building's long axis (walls stay
//! straight whatever the building's rotation); each level (cellar, ground, upper floors) is
//! split into rooms from the building's room program, rooms are joined by doors (a spanning
//! tree from the entrance plus a few loops), one stair block links every level at the same
//! spot, and rooms are furnished by rule with 5e cover values.
//!
//! Guarantees (checked in tests/vital.rs): at least two levels, every room reachable from the
//! front door, stairs aligned across levels, furniture never on a doorway or the stairs.

use serde::Serialize;

use crate::World;
use crate::core::rng::{Pcg32, hash3};
use crate::t0::T0;
use crate::town::geom::{self, P, add, dot, mul, sub};
use crate::town::{self, Layout, Structure};

/// Grid square size (ft) and storey height (ft).
pub const SQUARE_FT: f64 = 5.0;
pub const STOREY_FT: f32 = 10.0;

#[derive(Serialize, Debug, Clone)]
pub struct Interior {
    pub id: String,
    pub settlement: u32,
    pub building: u32,
    pub name: Option<String>,
    pub function: &'static str,
    /// Underground: the site's theme (`under::THEMES`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<&'static str>,
    /// World position (ft) of grid corner (0, 0); grid x runs along `axis`, grid y along
    /// `across` (both unit vectors).
    pub origin: [f64; 2],
    pub axis: [f64; 2],
    pub across: [f64; 2],
    pub nx: usize,
    pub ny: usize,
    /// Levels bottom to top; `entry_level` is the ground floor.
    pub levels: Vec<Level>,
    pub entry_level: usize,
    /// The stair block (same squares on every level): x, y, w, h.
    pub stairs: [usize; 4],
}

#[derive(Serialize, Debug, Clone)]
pub struct Level {
    /// -1 cellar, 0 ground, 1.. upper floors.
    pub z: i8,
    pub name: String,
    pub elevation_ft: f32,
    /// Room index per square (row-major, nx × ny); -1 outside the building.
    pub cells: Vec<i16>,
    pub rooms: Vec<Room>,
    /// Wall runs on square edges (grid units): x0, y0, x1, y1, and exterior or not.
    pub walls: Vec<Wall>,
    pub doors: Vec<Door>,
    /// Window openings on exterior walls (grid edges), above ground only.
    pub windows: Vec<[f32; 4]>,
    pub furniture: Vec<Item>,
    /// An open roof (battlements): the outer wall is a crenellated parapet.
    pub roof: bool,
    /// The main stair block reaches this level (tower tops above a keep's roof are reached
    /// only by the towers' spiral stairs).
    pub has_stairs: bool,
    /// Hewn or natural rock (caves, mines): walls only where floor meets rock; rooms open
    /// into each other (ledges are their raised floors).
    pub natural: bool,
    /// Lines the floor follows (sewers: the streets above), for drawing smooth outlines:
    /// x0, y0, x1, y1, reach (grid units).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<[f32; 5]>,
    /// Ways to other sites from this level (a cellar trapdoor into the sewers, stairs down to a
    /// keep's deep dungeons, ladders up into cellars): each has a link back on the other side.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
}

/// A way from a square of one site to another site (`to`: its id).
#[derive(Serialize, Debug, Clone)]
pub struct Link {
    pub x: u16,
    pub y: u16,
    pub to: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Room {
    pub kind: &'static str,
    pub squares: usize,
    /// Floor height above the level (ft): arena stands are a raised tier.
    pub raise_ft: f32,
    /// Label spot (grid units).
    pub center: [f32; 2],
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct Wall {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub exterior: bool,
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct Door {
    /// The door's square edge (grid units).
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// "front", "door", "back".
    pub kind: &'static str,
    /// Rooms on either side (-1 outside).
    pub rooms: [i16; 2],
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct Item {
    pub kind: &'static str,
    pub name: &'static str,
    /// Footprint in squares: x, y, w, h.
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// 0 none, 1 half, 2 three-quarters, 3 total.
    pub cover: u8,
    pub blocks_move: bool,
    pub height_ft: f32,
    /// Hazard rules (traps, lava, pits), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hazard: Option<&'static str>,
}

// ---------------------------------------------------------------------------------------
// Archetypes and room programs.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arch {
    House,
    Tavern,
    Inn,
    Shop,
    Forge,
    Warehouse,
    Temple,
    Hall,
    Mansion,
    Keep,
    Arena,
    Tower,
    /// Breweries, wineries, distilleries: a brewhouse and a taproom.
    Brewery,
    /// A gaol: cell blocks on every level.
    Prison,
}

fn arch_of(b: &town::Building) -> Arch {
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
    match key {
        Some("inn" | "caravanserai") => Arch::Inn,
        Some("tavern" | "alehouse" | "fine_dining" | "gambling_den" | "smugglers_den") => Arch::Tavern,
        Some("blacksmith" | "armorer" | "weaponsmith" | "glassblower" | "potter" | "tinker") => Arch::Forge,
        Some("brewery" | "winery" | "distillery") => Arch::Brewery,
        Some("warehouse" | "stables" | "shipwright" | "cartwright" | "mill" | "customs_house" | "carpenter" | "mason" | "cooper" | "black_market") => Arch::Warehouse,
        Some("temple" | "shrine" | "monastery" | "mausoleum" | "undertaker" | "catacombs") => Arch::Temple,
        Some("prison") => Arch::Prison,
        Some("castle" | "barracks" | "gatehouse") => Arch::Keep,
        Some("palace") => Arch::Mansion,
        Some("arena" | "fighting_pit" | "theater") => Arch::Arena,
        Some("observatory" | "arcane_academy" | "mages_guild") if round_ish(b) => Arch::Tower,
        Some(
            "town_hall" | "courthouse" | "guard_post" | "harbor_master" | "merchant_guild" | "craft_guild" | "adventurers_guild" | "mages_guild" | "thieves_guild" | "library"
            | "arcane_academy" | "observatory" | "bank" | "auction_house" | "bathhouse" | "orphanage" | "healer" | "bards_college" | "menagerie",
        ) => Arch::Hall,
        Some(_) => Arch::Shop,
        None => match town::catalog::RESIDENTIAL[b.residential as usize] {
            "noble estate" => Arch::Mansion,
            "wizard's tower" => Arch::Tower,
            _ => Arch::House,
        },
    }
}

/// A footprint with many sides (the POI towers are 14-gons).
fn round_ish(b: &town::Building) -> bool {
    b.poly.len() >= 8
}

/// A room program entry: kind, relative size, and whether more copies may be added to fill a
/// large floor.
type Prog = &'static [(&'static str, f64)];

struct LevelProgram {
    name: &'static str,
    rooms: Vec<(&'static str, f64)>,
    /// Repeated to fill big floors (bedrooms, guest rooms, offices, storage).
    filler: Option<&'static str>,
    /// Use a central corridor when the floor is big enough.
    corridor: bool,
    /// A cell block: the first room is a strip by the stairs, the rest is corridors lined with
    /// cells under 15 ft a side.
    cells: bool,
}

fn program(arch: Arch, z: i8, top: bool, key: Option<&str>) -> LevelProgram {
    let p = |name: &'static str, rooms: Prog, filler: Option<&'static str>, corridor: bool| LevelProgram { name, rooms: rooms.to_vec(), filler, corridor, cells: false };
    let cells = |name: &'static str, strip: &'static str| LevelProgram { name, rooms: vec![(strip, 1.0)], filler: None, corridor: false, cells: true };
    use Arch::*;
    match (arch, z) {
        (_, z) if z == ROOF => p("Battlements", &[("battlements", 1.0)], None, false),
        (_, -1) => match arch {
            Temple => p("Crypt", &[("crypt", 3.0), ("ossuary", 1.0)], Some("crypt"), false),
            Keep => cells("Dungeon", "guardroom"),
            Prison => cells("Oubliettes", "torture chamber"),
            Arena => p("Undercroft", &[("holding pen", 2.0), ("armory", 1.0), ("beast pen", 2.0)], Some("holding pen"), true),
            Tavern | Inn | Brewery => p("Cellar", &[("keg cellar", 2.0), ("storeroom", 1.0)], None, false),
            Mansion => p("Cellar", &[("wine cellar", 2.0), ("storeroom", 1.0)], Some("storeroom"), false),
            Hall if matches!(key, Some("bank" | "thieves_guild")) => p("Vault", &[("vault", 2.0), ("storeroom", 1.0)], None, false),
            Hall if matches!(key, Some("courthouse" | "guard_post")) => cells("Cells", "guardroom"),
            Tower => p("Vault", &[("vault", 1.0)], None, false),
            Warehouse => p("Cellar", &[("storeroom", 3.0)], Some("storeroom"), false),
            _ => p("Cellar", &[("storeroom", 1.0)], None, false),
        },
        (Tower, 0) => p("Entry", &[("entry hall", 1.0)], None, false),
        (Tower, _) => {
            let rooms = ["library", "laboratory", "study", "bedchamber"];
            if top {
                p("Observatory", &[("observatory", 1.0)], None, false)
            } else {
                LevelProgram { name: "Floor", rooms: vec![(rooms[(z as usize - 1) % 4], 1.0)], filler: None, corridor: false, cells: false }
            }
        }
        (House, 0) => p("Ground floor", &[("main room", 3.0), ("kitchen", 1.5)], Some("bedroom"), false),
        (House, _) => p("Upper floor", &[("bedroom", 2.0), ("bedroom", 1.5)], Some("bedroom"), false),
        (Tavern, 0) => p("Taproom", &[("common room", 5.0), ("kitchen", 1.5), ("storeroom", 1.0)], None, false),
        (Inn, 0) => p("Taproom", &[("common room", 5.0), ("kitchen", 1.5), ("storeroom", 1.0)], None, false),
        (Inn, _) => p("Guest rooms", &[("guest room", 1.0), ("guest room", 1.0)], Some("guest room"), true),
        (Tavern, _) => p("Upstairs", &[("owner's quarters", 2.0), ("storeroom", 1.0)], Some("guest room"), false),
        (Shop, 0) => p("Shop floor", &[("shop", 3.0), ("workshop", 2.0), ("storeroom", 1.0)], None, false),
        (Shop, _) => p("Living quarters", &[("living room", 1.5), ("bedroom", 1.5)], Some("bedroom"), false),
        (Forge, 0) => p("Forge", &[("forge", 4.0), ("storeroom", 1.0)], None, false),
        (Forge, _) => p("Living quarters", &[("living room", 1.5), ("bedroom", 1.5)], Some("bedroom"), false),
        (Brewery, 0) => p("Brewhouse", &[("taproom", 3.0), ("brewhouse", 4.0), ("storeroom", 1.0)], None, false),
        (Brewery, _) => p("Loft", &[("loft storage", 3.0), ("office", 1.0)], Some("loft storage"), false),
        (Warehouse, 0) => p("Floor", &[("warehouse floor", 6.0), ("office", 1.0)], None, false),
        (Warehouse, _) => p("Loft", &[("loft storage", 4.0), ("office", 1.0)], Some("loft storage"), false),
        (Temple, 0) => p("Sanctuary", &[("nave", 5.0), ("sanctuary", 2.0), ("vestry", 1.0)], None, false),
        (Temple, _) => p("Bell loft", &[("bell loft", 2.0), ("monk's cell", 1.0)], Some("monk's cell"), true),
        (Hall, 0) => {
            let main: &'static str = match key {
                Some("library") => "reading room",
                Some("bank") => "counting house",
                Some("bathhouse") => "baths",
                Some("healer") => "infirmary",
                Some("courthouse") => "courtroom",
                _ => "great hall",
            };
            LevelProgram { name: "Ground floor", rooms: vec![(main, 4.0), ("office", 1.0), ("office", 1.0)], filler: Some("office"), corridor: false, cells: false }
        }
        (Hall, _) => {
            let filler: &'static str = match key {
                Some("library" | "arcane_academy" | "bards_college") => "library",
                Some("orphanage" | "adventurers_guild") => "dormitory",
                _ => "office",
            };
            LevelProgram { name: "Upper floor", rooms: vec![("meeting room", 2.0), ("archive", 1.0), (filler, 1.0)], filler: Some(filler), corridor: true, cells: false }
        }
        (Mansion, 0) => p("Ground floor", &[("great hall", 3.0), ("dining room", 2.0), ("foyer", 1.5), ("kitchen", 1.5), ("parlor", 1.5)], Some("parlor"), false),
        (Mansion, _) => p("Upper floor", &[("master bedroom", 2.0), ("study", 1.2), ("bedroom", 1.5), ("gallery", 1.5)], Some("bedroom"), true),
        (Prison, 0) => cells("Gaol", "guardroom"),
        (Prison, 1) => cells("Cell block", "warden's office"),
        (Prison, _) => cells("Cell block", "guardroom"),
        (Keep, 0) => {
            if key == Some("barracks") {
                p("Hall", &[("mess hall", 5.0), ("guardroom", 1.5), ("kitchen", 1.5), ("pantry", 1.0)], Some("guardroom"), false)
            } else {
                p("Hall", &[("great hall", 5.0), ("guardroom", 1.5), ("kitchen", 1.5), ("pantry", 1.0), ("buttery", 1.0)], Some("guardroom"), false)
            }
        }
        (Keep, _) if key == Some("barracks") => p("Upper floor", &[("barrack room", 2.0), ("armory", 1.0), ("officers' quarters", 1.0)], Some("barrack room"), true),
        // Each floor of a keep has its own rooms: one chapel, one lord's chamber.
        (Keep, 1) => p("Lord's floor", &[("lord's chamber", 2.0), ("solar", 1.5), ("chapel", 1.0)], Some("chamber"), true),
        (Keep, 2) => p("Upper floor", &[("armory", 1.5), ("map room", 1.5), ("treasury", 1.0)], Some("guest chamber"), true),
        (Keep, 3) => p("Upper floor", &[("library", 1.5), ("study", 1.0), ("guest chamber", 1.5)], Some("guest chamber"), true),
        (Keep, _) => p("Upper floor", &[("servants' quarters", 1.5), ("wardrobe", 1.0), ("storeroom", 1.0)], Some("servants' quarters"), true),
        (Arena, 0) => p("Arena", &[("arena floor", 1.0)], None, false),
        (Arena, _) => p("Gallery", &[("gallery", 1.0)], None, false),
    }
}

/// Corner towers of a keep's grid: their side in squares (0 when the keep is too small).
fn keep_tower_squares(nx: usize, ny: usize) -> usize {
    if nx.min(ny) >= 8 { 3 } else { 0 }
}

/// Keeps and towers have a walkable roof: battlements instead of a pitched roof. For a keep,
/// the side of its corner towers (ft; 0 for none), as the battlemap draws them.
pub fn battlements(b: &town::Building) -> Option<f64> {
    // A roof chosen by hand: battlements on anything (a keep keeps its corner towers).
    match b.roof {
        Some(town::RoofStyle::Battlements) if arch_of(b) != Arch::Keep => return Some(0.0),
        Some(town::RoofStyle::Hip | town::RoofStyle::Cone) => return None,
        _ => {}
    }
    match arch_of(b) {
        Arch::Keep => {
            let o = geom::obb(&b.poly);
            let (nx, ny) = (crate::core::round(o.long / SQUARE_FT) as usize, crate::core::round(o.short / SQUARE_FT) as usize);
            Some(keep_tower_squares(nx, ny) as f64 * SQUARE_FT)
        }
        Arch::Tower => Some(0.0),
        _ => None,
    }
}

/// The level index of an open roof (battlements) on keeps and towers.
const ROOF: i8 = 100;

/// The level index of a keep's tower tops (above its battlements).
const TOWER_TOP: i8 = 101;

/// Rooms that stay big (they are the point of the building).
fn big_room(kind: &str) -> bool {
    matches!(
        kind,
        "common room" | "taproom" | "brewhouse" | "battlements" | "warehouse floor" | "loft storage" | "nave" | "great hall" | "mess hall" | "forge" | "reading room" | "counting house" | "baths" | "infirmary" | "courtroom" | "arena floor" | "stands" | "crypt" | "dungeon"
    )
}

// ---------------------------------------------------------------------------------------
// Furniture.

/// (kind, display name, cover, blocks movement, height ft).
fn item_info(kind: &'static str) -> (&'static str, u8, bool, f32) {
    match kind {
        "bed" => ("bed", 1, false, 2.0),
        "chest" => ("chest", 1, true, 2.5),
        "wardrobe" => ("wardrobe", 3, true, 7.0),
        "table" => ("table", 1, true, 3.0),
        "long_table" => ("long table", 1, true, 3.0),
        "desk" => ("desk", 1, true, 3.0),
        "bench" => ("bench", 0, false, 1.5),
        "pew" => ("pew", 1, false, 3.0),
        "bar" => ("bar counter", 2, true, 4.0),
        "counter" => ("counter", 2, true, 4.0),
        "hearth" => ("hearth", 2, true, 5.0),
        "oven" => ("oven", 2, true, 5.0),
        "shelf" => ("shelves", 2, true, 7.0),
        "bookcase" => ("bookcase", 3, true, 8.0),
        "barrel" => ("barrel", 1, true, 3.5),
        "keg_rack" => ("keg rack", 2, true, 5.0),
        "crate" => ("crate stack", 2, true, 5.0),
        "workbench" => ("workbench", 1, true, 3.0),
        "forge" => ("forge", 2, true, 5.0),
        "anvil" => ("anvil", 1, true, 3.0),
        "weapon_rack" => ("weapon rack", 1, true, 6.0),
        "altar" => ("altar", 2, true, 4.0),
        "statue" => ("statue", 3, true, 9.0),
        "sarcophagus" => ("sarcophagus", 2, true, 3.5),
        "cage" => ("iron cage", 1, true, 7.0),
        "throne" => ("throne", 1, true, 5.0),
        "rug" => ("rug", 0, false, 0.0),
        "couch" => ("couch", 1, true, 3.0),
        "cauldron" => ("cauldron", 1, true, 3.0),
        "alchemy_bench" => ("alchemy bench", 1, true, 3.5),
        "telescope" => ("telescope", 1, true, 6.0),
        "bell" => ("bell", 3, true, 6.0),
        "bath" => ("bath", 1, true, 2.0),
        "cot" => ("cot", 0, false, 1.5),
        "pillar" => ("pillar", 3, true, 10.0),
        "barricade" => ("barricade", 2, true, 4.0),
        "display" => ("display table", 1, true, 3.0),
        "vat" => ("brewing vat", 2, true, 6.0),
        "chair" => ("chair", 0, false, 3.0),
        "spiral_stair" => ("spiral stairs", 0, false, 0.0),
        "rack" => ("rack", 1, true, 3.0),
        "bucket" => ("slop bucket", 0, false, 1.0),
        "booth_table" => ("booth table", 1, true, 3.0),
        "booth_seat" => ("booth seat", 1, false, 3.5),
        "trapdoor" => ("trapdoor (to the undercroft)", 0, false, 0.0),
        "winch" => ("portcullis winch", 1, true, 4.0),
        "sideboard" => ("sideboard", 1, true, 3.5),
        "stage" => ("stage", 0, false, 2.0),
        _ => ("furniture", 1, true, 3.0),
    }
}

#[derive(Clone, Copy)]
enum Place {
    /// Against a wall (length along the wall, depth into the room).
    Wall(u16, u16),
    /// Free-standing, clear of the walls.
    Center(u16, u16),
    /// A one-square round table with a chair on each open side (four when the room allows).
    Seated,
    /// A table in a room corner with an L of bench seats along the two walls (2 × 2).
    Booth,
}

/// Furniture rules per room kind: (item, placement, how many: fixed plus one per `per`
/// squares of room, 0 = none).
fn furnishing(kind: &str) -> &'static [(&'static str, Place, u8, u16)] {
    use Place::*;
    match kind {
        "common room" | "taproom" => &[("bar", Wall(3, 1), 1, 0), ("table", Seated, 2, 12), ("booth_table", Booth, 0, 24), ("hearth", Wall(2, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "brewhouse" => &[("vat", Center(2, 2), 1, 16), ("barrel", Wall(1, 1), 2, 8), ("workbench", Wall(2, 1), 1, 0)],
        "battlements" => &[("crate", Wall(1, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "kitchen" => &[("oven", Wall(2, 1), 1, 0), ("table", Center(2, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0)],
        "storeroom" | "loft storage" => &[("crate", Wall(1, 1), 2, 6), ("barrel", Wall(1, 1), 1, 8), ("shelf", Wall(2, 1), 1, 0)],
        "keg cellar" | "wine cellar" => &[("keg_rack", Wall(2, 1), 2, 8), ("barrel", Wall(1, 1), 2, 6)],
        "bedroom" | "guest room" | "owner's quarters" | "chamber" => &[("bed", Wall(1, 2), 1, 0), ("chest", Wall(1, 1), 1, 0), ("wardrobe", Wall(1, 1), 1, 0)],
        "master bedroom" | "lord's chamber" | "bedchamber" => &[("bed", Wall(2, 2), 1, 0), ("wardrobe", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        "main room" | "living room" => &[("hearth", Wall(2, 1), 1, 0), ("table", Center(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("bench", Wall(2, 1), 1, 0)],
        "shop" => &[("counter", Wall(3, 1), 1, 0), ("shelf", Wall(2, 1), 2, 10), ("display", Center(2, 1), 0, 12)],
        "workshop" => &[("workbench", Wall(2, 1), 1, 8), ("shelf", Wall(2, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "forge" => &[("forge", Wall(2, 2), 1, 0), ("anvil", Center(1, 1), 1, 0), ("barrel", Wall(1, 1), 1, 0), ("weapon_rack", Wall(2, 1), 1, 0), ("workbench", Wall(2, 1), 1, 0)],
        "warehouse floor" => &[("crate", Center(2, 2), 1, 10), ("shelf", Wall(2, 1), 1, 10), ("barrel", Wall(1, 1), 2, 0)],
        "office" | "archive" => &[("desk", Center(2, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 8), ("chest", Wall(1, 1), 1, 0)],
        "study" => &[("desk", Center(2, 1), 1, 0), ("bookcase", Wall(2, 1), 2, 0), ("rug", Center(2, 2), 1, 0)],
        "meeting room" => &[("long_table", Center(3, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 0)],
        "library" | "reading room" => &[("bookcase", Wall(2, 1), 2, 6), ("table", Center(2, 1), 1, 16)],
        "laboratory" => &[("alchemy_bench", Wall(2, 1), 2, 0), ("cauldron", Center(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "observatory" => &[("telescope", Center(2, 2), 1, 0), ("desk", Wall(2, 1), 1, 0), ("bookcase", Wall(1, 1), 1, 0)],
        "entry hall" | "foyer" => &[("rug", Center(2, 2), 1, 0), ("statue", Wall(1, 1), 1, 0)],
        "nave" => &[("pew", Center(3, 1), 2, 6), ("pillar", Wall(1, 1), 2, 0)],
        "sanctuary" | "chapel" => &[("altar", Wall(2, 1), 1, 0), ("statue", Wall(1, 1), 1, 0), ("pew", Center(2, 1), 0, 10)],
        "vestry" => &[("wardrobe", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("table", Center(1, 1), 1, 0)],
        "crypt" | "ossuary" => &[("sarcophagus", Center(1, 2), 1, 8), ("statue", Wall(1, 1), 1, 0)],
        "bell loft" => &[("bell", Center(2, 2), 1, 0)],
        "monk's cell" | "cell" | "holding cell" => &[("cot", Wall(1, 2), 1, 0), ("bucket", Wall(1, 1), 1, 0)],
        "great hall" | "mess hall" => &[("long_table", Center(4, 1), 1, 18), ("hearth", Wall(2, 1), 1, 0), ("throne", Wall(1, 1), 1, 0), ("pillar", Wall(1, 1), 0, 16)],
        "dining room" => &[("long_table", Center(3, 1), 1, 0), ("sideboard", Wall(2, 1), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "parlor" => &[("couch", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "gallery" => &[("statue", Wall(1, 1), 2, 8), ("bench", Center(2, 1), 1, 0)],
        "guardroom" | "armory" => &[("weapon_rack", Wall(2, 1), 1, 6), ("table", Center(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "barrack room" | "dormitory" => &[("cot", Wall(1, 2), 2, 5), ("chest", Wall(1, 1), 1, 8)],
        "dungeon" | "holding pen" | "beast pen" => &[("cage", Center(2, 2), 1, 12), ("chest", Wall(1, 1), 0, 0), ("barrel", Wall(1, 1), 1, 0)],
        "vault" => &[("chest", Wall(1, 1), 3, 4), ("shelf", Wall(2, 1), 1, 0)],
        "counting house" | "courtroom" => &[("counter", Wall(3, 1), 1, 0), ("desk", Center(2, 1), 1, 12), ("bench", Center(3, 1), 0, 12)],
        "baths" => &[("bath", Center(2, 2), 1, 12), ("bench", Wall(2, 1), 1, 0)],
        "infirmary" => &[("cot", Wall(1, 2), 2, 6), ("table", Center(2, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "arena floor" => &[("pillar", Center(1, 1), 2, 40), ("barricade", Center(2, 1), 2, 30), ("trapdoor", Center(1, 1), 1, 30), ("cage", Center(1, 1), 1, 0)],
        "stands" => &[("bench", Wall(3, 1), 2, 6)],
        "tower room" => &[("weapon_rack", Wall(1, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "winch room" => &[("winch", Center(1, 1), 1, 0), ("weapon_rack", Wall(1, 1), 1, 0)],
        "wall walk" => &[],
        "hall" | "corridor" => &[],
        "solar" => &[("couch", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("table", Center(1, 1), 1, 0)],
        "map room" => &[("long_table", Center(3, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "treasury" => &[("chest", Wall(1, 1), 3, 4), ("shelf", Wall(2, 1), 1, 0)],
        "guest chamber" => &[("bed", Wall(1, 2), 1, 0), ("wardrobe", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 1), 1, 0)],
        "servants' quarters" | "officers' quarters" => &[("cot", Wall(1, 2), 2, 6), ("chest", Wall(1, 1), 1, 8)],
        "pantry" | "buttery" => &[("shelf", Wall(2, 1), 1, 6), ("barrel", Wall(1, 1), 2, 0), ("crate", Wall(1, 1), 1, 0)],
        "wardrobe" => &[("wardrobe", Wall(1, 1), 2, 0), ("chest", Wall(1, 1), 1, 0)],
        "guard post" => &[("weapon_rack", Wall(1, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "tower top" => &[("crate", Wall(1, 1), 1, 0)],
        "torture chamber" => &[("rack", Center(2, 1), 1, 0), ("cage", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "prison block" => &[("cage", Wall(2, 2), 2, 3), ("cot", Wall(1, 2), 1, 2), ("bucket", Wall(1, 1), 1, 2)],
        "well room" => &[("bucket", Wall(1, 1), 2, 2), ("barrel", Wall(1, 1), 1, 2)],
        "warden's office" => &[("desk", Center(2, 1), 1, 0), ("bookcase", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        _ => &[("chest", Wall(1, 1), 1, 0)],
    }
}

// ---------------------------------------------------------------------------------------
// Generation.

/// Interior of a layout's building, or `None` for what cannot be entered (ruins, graveyards).
pub fn generate(world: &World, t0: &T0, settlement: usize, building: usize) -> Option<Interior> {
    if settlement >= town::layout_count(t0) {
        return None;
    }
    let l = town::layout(world, t0, settlement);
    let b = l.building(building)?;
    if b.structure != Structure::Roofed {
        return None;
    }
    Some(build(world, t0, &l, settlement, b))
}

/// A keep big enough for deep dungeons below its cellar (`under::keep_dungeon`).
pub fn has_deep_dungeon(b: &town::Building) -> bool {
    b.structure == Structure::Roofed && arch_of(b) == Arch::Keep && geom::area(&b.poly).abs() >= 150.0 * SQUARE_FT * SQUARE_FT
}

/// Put a link item on the free floor square of `lvl` nearest grid point `at` (off the stairs
/// and doorways); false if there is none.
fn place_link(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], at: [f64; 2], kind: &'static str, name: &'static str, to: String) -> bool {
    let mut taken = vec![false; nx * ny];
    for f in &lvl.furniture {
        for j in f.y..f.y + f.h {
            for i in f.x..f.x + f.w {
                taken[j as usize * nx + i as usize] = true;
            }
        }
    }
    for j in st[1]..st[1] + st[3] {
        for i in st[0]..st[0] + st[2] {
            taken[j * nx + i] = true;
        }
    }
    for d in &lvl.doors {
        for (x, y) in [(d.a[0].min(d.b[0]), d.a[1].min(d.b[1])), (d.a[0].min(d.b[0]) - if d.a[0] == d.b[0] { 1.0 } else { 0.0 }, d.a[1].min(d.b[1]) - if d.a[1] == d.b[1] { 1.0 } else { 0.0 })] {
            if x >= 0.0 && y >= 0.0 && (x as usize) < nx && (y as usize) < ny {
                taken[y as usize * nx + x as usize] = true;
            }
        }
    }
    let best = (0..nx * ny).filter(|&k| lvl.cells[k] >= 0 && !taken[k]).min_by(|&a, &b| {
        let d = |k: usize| ((k % nx) as f64 + 0.5 - at[0]).powi(2) + ((k / nx) as f64 + 0.5 - at[1]).powi(2);
        d(a).total_cmp(&d(b))
    });
    let Some(k) = best else { return false };
    let (x, y) = (k % nx, k / nx);
    lvl.furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: 1, h: 1, cover: 0, blocks_move: false, height_ft: 0.0, hazard: None });
    lvl.links.push(Link { x: x as u16, y: y as u16, to });
    true
}

fn build(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building) -> Interior {
    let bi = b.id as usize;
    let mut rng = Pcg32::new(hash3(world.stream("interior"), settlement as i64, bi as i64, 0x1a7), 71);
    let arch = arch_of(b);
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);

    // Grid along the footprint's long axis.
    let o = geom::obb(&b.poly);
    let (u, v) = (o.axis, [-o.axis[1], o.axis[0]]);
    let (a0, a1) = geom::extent(&b.poly, u);
    let (b0, b1) = geom::extent(&b.poly, v);
    let nx = (crate::core::round((a1 - a0) / SQUARE_FT) as usize).max(2);
    let ny = (crate::core::round((b1 - b0) / SQUARE_FT) as usize).max(2);
    let (u0, v0) = (0.5 * (a0 + a1) - 0.5 * nx as f64 * SQUARE_FT, 0.5 * (b0 + b1) - 0.5 * ny as f64 * SQUARE_FT);
    let origin = add(mul(u, u0), mul(v, v0));
    let world_at = |x: f64, y: f64| add(origin, add(mul(u, x * SQUARE_FT), mul(v, y * SQUARE_FT)));
    // Squares inside the footprint (a tolerance keeps rectangle edges whole).
    let grown = grow(&b.poly, 1.5);
    let inside: Vec<bool> = (0..nx * ny).map(|k| geom::contains(&grown, world_at((k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5))).collect();

    // Squares just outside the walls that are open ground (not inside a neighbouring
    // building): a door must open onto one of these.
    let c0 = geom::centroid(&b.poly);
    let reach = b.poly.iter().map(|p| geom::dist(*p, c0)).fold(0.0, f64::max) + 40.0;
    let neighbours: Vec<&Vec<P>> = l
        .buildings
        .iter()
        .filter(|o| o.id != b.id && o.structure == Structure::Roofed && o.poly.iter().any(|p| geom::dist(*p, c0) < reach + 60.0))
        .map(|o| &o.poly)
        .collect();
    let ext_free = |i: isize, j: isize| {
        let p = world_at(i as f64 + 0.5, j as f64 + 0.5);
        !neighbours.iter().any(|poly| geom::contains(poly, p))
    };
    let is_in = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && inside[j as usize * nx + i as usize];
    // Open exterior squares along each side (0 top, 1 right, 2 bottom, 3 left).
    let open_on = |side: usize| -> usize {
        let (di, dj) = [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)][side];
        (0..ny as isize).flat_map(|j| (0..nx as isize).map(move |i| (i, j))).filter(|&(i, j)| is_in(i, j) && !is_in(i + di, j + dj) && ext_free(i + di, j + dj)).count()
    };

    // The front: the open grid side nearest a street (else nearest the settlement centre).
    let sides = [[0.5 * nx as f64, 0.0], [nx as f64, 0.5 * ny as f64], [0.5 * nx as f64, ny as f64], [0.0, 0.5 * ny as f64]];
    let street_d = |p: P| {
        l.roads
            .iter()
            .flat_map(|(pts, _, w)| pts.windows(2).map(move |s| geom::seg_dist(p, s[0], s[1]) - 0.5 * w))
            .fold(f64::MAX, f64::min)
    };
    let open: Vec<usize> = (0..4).map(open_on).collect();
    let any_open = open.iter().any(|&n| n > 0);
    let front = (0..4)
        .filter(|&k| !any_open || open[k] > 0)
        .min_by(|&x, &y| {
            let (px, py) = (world_at(sides[x][0], sides[x][1]), world_at(sides[y][0], sides[y][1]));
            let score = |p: P| {
                let d = street_d(p);
                if d < 80.0 { d } else { 1e4 + geom::dist(p, l.center) }
            };
            score(px).total_cmp(&score(py))
        })
        .unwrap_or(0);

    // Levels: a cellar, the ground floor and the storeys above; at least two levels.
    let floors = b.floors.max(1) as i8;
    let mut zs: Vec<i8> = vec![-1];
    zs.extend(0..floors);
    // Keeps and towers (or any roof drawn so): walk the battlements on the roof.
    let open_roof = battlements(b).is_some();
    if open_roof {
        zs.push(ROOF);
    }
    // A keep's corner towers rise a storey above its battlements.
    let t = keep_tower_squares(nx, ny);
    if arch == Arch::Keep && t > 0 && open_roof {
        zs.push(TOWER_TOP);
    }
    let entry_level = 1;

    // The stair block: at the back of the building, in the middle of the short side (towers:
    // near the centre), on inside squares. Inns and taverns lay out the ground floor first
    // and put the stairs in the taproom, where guests can reach them.
    let mut ground_pre: Option<Level> = None;
    let mut stairs = place_stairs(&inside, nx, ny, front, arch);
    if is_pub(arch) {
        let prog = program(arch, 0, floors == 1, key);
        let lvl = layout_level(&inside, nx, ny, &prog, [0, 0, 0, 0], front, true, arch, &ext_free, &mut rng);
        if let Some(st) = pub_stairs(&lvl, nx, ny) {
            stairs = st;
            ground_pre = Some(lvl);
        }
    }

    let mut levels = Vec::new();
    for (li, &z) in zs.iter().enumerate() {
        let top = li + 1 == zs.len() || matches!(zs.get(li + 1), Some(&ROOF) | Some(&TOWER_TOP));
        let prog = program(arch, z.min(ROOF), top, key);
        let mut lvl = match (&ground_pre, z) {
            (Some(g), 0) => g.clone(),
            (_, TOWER_TOP) => tower_tops(&inside, nx, ny, keep_tower_squares(nx, ny), stairs, &mut rng),
            _ => layout_level(&inside, nx, ny, &prog, stairs, front, z == 0, arch, &ext_free, &mut rng),
        };
        // The roof is one storey above the top floor, the tower tops one above that.
        let zf = match z {
            ROOF => floors,
            TOWER_TOP => floors + 1,
            _ => z,
        };
        lvl.z = zf;
        lvl.name = if z == TOWER_TOP { "Tower tops".to_string() } else { prog.name.to_string() };
        lvl.elevation_ft = b.pad_ft + STOREY_FT * zf as f32;
        if z == ROOF || z == TOWER_TOP {
            // Open to the sky: the outer wall is a parapet (no windows).
            lvl.roof = true;
            lvl.windows.clear();
        }
        // Keeps: a spiral stair in each corner tower, from the ground floor to the tower top.
        if arch == Arch::Keep && t > 0 && zf >= 0 {
            for (i, j) in [(0, 0), (nx - 1, 0), (0, ny - 1), (nx - 1, ny - 1)] {
                if lvl.cells[j * nx + i] >= 0 {
                    push_item(&mut lvl.furniture, "spiral_stair", i, j);
                }
            }
        }
        furnish(&mut lvl, nx, ny, stairs, &mut rng);
        if z == -1 {
            let grid = |p: P| {
                let d = sub(p, origin);
                [dot(d, u) / SQUARE_FT, dot(d, v) / SQUARE_FT]
            };
            // A trapdoor down into the sewers (on the street side).
            if let Some(q) = crate::under::sewer_link_of(t0, l, b) {
                place_link(&mut lvl, nx, ny, stairs, grid(q), "trapdoor", "trapdoor to the sewers", crate::under::sewer_id(l.index as usize, q));
            }
            // Stairs down to a keep's deep dungeons, in the middle of the far end.
            if has_deep_dungeon(b) {
                place_link(&mut lvl, nx, ny, stairs, [nx as f64 * 0.5, ny as f64 * 0.5], "link_down", "stairs down to the deep dungeons", format!("k:{}:{}", l.index, bi));
            }
        }
        levels.push(lvl);
    }
    Interior {
        id: format!("b:{}:{}", l.index, bi),
        settlement: settlement as u32,
        building: bi as u32,
        name: b.name.clone(),
        theme: None,
        function: b.label(),
        origin,
        axis: u,
        across: v,
        nx,
        ny,
        levels,
        entry_level,
        stairs,
    }
}

/// Stairs for an inn or tavern: a 2 × 1 block (else 1 × 1) in the public room against one of
/// its walls, off every doorway, as far from the front door as the room allows.
fn pub_stairs(lvl: &Level, nx: usize, ny: usize) -> Option<[usize; 4]> {
    let front = lvl.doors.iter().find(|d| d.kind == "front")?;
    let public = front.rooms[0];
    let fd = [(front.a[0] + front.b[0]) * 0.5, (front.a[1] + front.b[1]) * 0.5];
    // Squares next to any door stay clear.
    let mut near_door = vec![false; nx * ny];
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        for (i, j) in sq {
            for (di, dj) in [(0isize, 0isize), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i + di, j + dj);
                if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                    near_door[b as usize * nx + a as usize] = true;
                }
            }
        }
    }
    let ok = |i: usize, j: usize| i < nx && j < ny && lvl.cells[j * nx + i] == public && !near_door[j * nx + i];
    let wall = |i: usize, j: usize| [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| {
        let (a, b) = (i as isize + di, j as isize + dj);
        a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || lvl.cells[b as usize * nx + a as usize] != public
    });
    let mut best: Option<(f32, [usize; 4])> = None;
    for (w, h) in [(2usize, 1usize), (1, 2), (1, 1)] {
        for j in 0..ny {
            for i in 0..nx {
                let sq: Vec<(usize, usize)> = (j..j + h).flat_map(|b| (i..i + w).map(move |a| (a, b))).collect();
                if !sq.iter().all(|&(a, b)| ok(a, b)) || !sq.iter().all(|&(a, b)| wall(a, b)) {
                    continue;
                }
                let c = [i as f32 + w as f32 * 0.5, j as f32 + h as f32 * 0.5];
                let d = (c[0] - fd[0]).abs() + (c[1] - fd[1]).abs();
                if best.is_none_or(|b| d > b.0) {
                    best = Some((d, [i, j, w, h]));
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|b| b.1)
}

/// The tops of a keep's four corner towers: a battlemented square each (no main stairs), on
/// the footprint (a corner the footprint cuts off has no tower, nor its spiral stair).
fn tower_tops(inside: &[bool], nx: usize, ny: usize, t: usize, st: [usize; 4], rng: &mut Pcg32) -> Level {
    let mut cells = vec![-1i16; nx * ny];
    let mut rooms = Vec::new();
    for ((cx, cy), (ki, kj)) in [(0, 0), (nx - t, 0), (0, ny - t), (nx - t, ny - t)].into_iter().zip([(0, 0), (nx - 1, 0), (0, ny - 1), (nx - 1, ny - 1)]) {
        if !inside[kj * nx + ki] {
            continue;
        }
        let id = rooms.len() as i16;
        rooms.push(Room { kind: "tower top", squares: 0, raise_ft: 0.0, center: [0.0; 2] });
        for j in cy..cy + t {
            for i in cx..cx + t {
                if inside[j * nx + i] {
                    cells[j * nx + i] = id;
                }
            }
        }
    }
    finish_rooms(&mut rooms, &cells, nx);
    let mut lvl = connect(cells, rooms, nx, ny, st, 0, false, None, &|_, _| true, rng);
    lvl.has_stairs = false;
    lvl
}

/// A convex polygon pushed outward by `d` ft (about its centroid).
fn grow(poly: &[P], d: f64) -> Vec<P> {
    let c = geom::centroid(poly);
    poly.iter()
        .map(|p| {
            let r = sub(*p, c);
            let l = geom::len(r).max(1e-9);
            add(*p, mul(r, d / l))
        })
        .collect()
}

fn place_stairs(inside: &[bool], nx: usize, ny: usize, front: usize, arch: Arch) -> [usize; 4] {
    let fits = |x: usize, y: usize, w: usize, h: usize| x + w <= nx && y + h <= ny && (y..y + h).all(|j| (x..x + w).all(|i| inside[j * nx + i]));
    // Towers: a 2×2 spiral near the centre.
    if arch == Arch::Tower {
        let (cx, cy) = (nx / 2, ny / 2);
        for r in 0..nx.max(ny) {
            for (x, y) in [(cx + r, cy), (cx.saturating_sub(r), cy), (cx, cy + r), (cx, cy.saturating_sub(r))] {
                if fits(x, y, 2, 2) {
                    return [x, y, 2, 2];
                }
            }
        }
    }
    // Two squares along the long axis, one wide, at the back end, mid short side.
    let (w, h) = if nx >= 3 { (2, 1) } else { (1, 2.min(ny)) };
    let back_x = if front == 3 { nx.saturating_sub(w) } else { 0 };
    let back_x = if front == 1 { 0 } else { back_x.max(if front == 3 { 0 } else { nx.saturating_sub(w) }) };
    let y_mid = ny / 2;
    let ys: Vec<usize> = (0..ny).map(|d| if d % 2 == 0 { y_mid + d / 2 } else { y_mid.saturating_sub(d / 2 + 1) }).filter(|&y| y + h <= ny).collect();
    let xs: Vec<usize> = if back_x == 0 { (0..nx).collect() } else { (0..=nx - w).rev().collect() };
    for &x in &xs {
        for &y in &ys {
            if fits(x, y, w, h) {
                return [x, y, w, h];
            }
        }
    }
    // Any inside square.
    let k = inside.iter().position(|&i| i).unwrap_or(0);
    [k % nx, k / nx, 1, 1]
}

#[derive(Clone, Copy, Debug)]
struct R {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

#[allow(clippy::too_many_arguments)]
fn layout_level(inside: &[bool], nx: usize, ny: usize, prog: &LevelProgram, st: [usize; 4], front: usize, ground: bool, arch: Arch, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
    // A large keep's dungeon floor: guardrooms, a prison block, stores and the stair down to
    // the deep dungeons, on corridors (a small keep keeps its cell block).
    let keep_dungeon;
    let prog = if prog.cells && arch == Arch::Keep && inside.iter().filter(|&&i| i).count() >= 150 {
        keep_dungeon = LevelProgram {
            name: "Dungeon",
            rooms: vec![("guardroom", 1.5), ("prison block", 1.5), ("torture chamber", 1.0), ("armory", 1.0), ("wine cellar", 1.0), ("well room", 0.8), ("vault", 1.0), ("storeroom", 1.0), ("prison block", 1.0)],
            filler: Some("storeroom"),
            corridor: true,
            cells: false,
        };
        &keep_dungeon
    } else {
        prog
    };
    let mut kinds: Vec<(&'static str, f64)> = prog.rooms.clone();
    let area = inside.iter().filter(|&&i| i).count();
    // Rooms per floor: at least 6 squares each (small houses keep fewer rooms), fillers to
    // keep ordinary rooms under ~30 squares.
    while kinds.len() > 1 && area / kinds.len() < 6 {
        kinds.pop();
    }
    // A small inn or tavern is all taproom (the bar and tables need the space).
    if ground && matches!(arch, Arch::Tavern | Arch::Inn) && area < 40 {
        kinds.truncate(1);
    }
    if let Some(f) = prog.filler {
        let fixed_big: f64 = kinds.iter().filter(|k| big_room(k.0)).map(|k| k.1).sum();
        let total: f64 = kinds.iter().map(|k| k.1).sum();
        let ordinary_area = area as f64 * (1.0 - fixed_big / total.max(1e-9));
        let ordinary = kinds.iter().filter(|k| !big_room(k.0)).count().max(1) as f64;
        // Guest rooms are about 15 ft square (a bed, a chest, a wardrobe); other fillers up to
        // ~30 squares.
        let size = if f == "guest room" { 13.0 } else { 30.0 };
        let mut extra = ((ordinary_area / size) - ordinary).max(0.0) as usize;
        extra = extra.min(if f == "guest room" { 20 } else { 10 });
        for _ in 0..extra {
            kinds.push((f, 1.0));
        }
    }

    let mut cells = vec![-1i16; nx * ny];
    let full = R { x: 0, y: 0, w: nx, h: ny };
    let mut leaves: Vec<(R, usize)> = Vec::new();
    let mut hall: Option<R> = None;

    if arch == Arch::Arena && ground {
        // A pit in the middle, stands around it.
        let inset = (nx.min(ny) / 4).clamp(1, 4);
        let pit = R { x: inset, y: inset, w: nx.saturating_sub(2 * inset).max(1), h: ny.saturating_sub(2 * inset).max(1) };
        let mut rooms = vec![Room { kind: "stands", squares: 0, raise_ft: 5.0, center: [0.0; 2] }, Room { kind: "arena floor", squares: 0, raise_ft: 0.0, center: [0.0; 2] }];
        for j in 0..ny {
            for i in 0..nx {
                if inside[j * nx + i] {
                    let in_pit = i >= pit.x && i < pit.x + pit.w && j >= pit.y && j < pit.y + pit.h;
                    let on_stairs = i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
                    cells[j * nx + i] = if in_pit && !on_stairs { 1 } else { 0 };
                }
            }
        }
        // On an odd footprint the pit can cut the stands in two (or the stands the pit): each
        // smaller piece joins the other room, so both stay one room each.
        for _ in 0..2 {
            let mut comp = vec![usize::MAX; nx * ny];
            let mut sizes: Vec<(i16, usize)> = Vec::new();
            for k0 in 0..nx * ny {
                if cells[k0] < 0 || comp[k0] != usize::MAX {
                    continue;
                }
                let (id, room) = (sizes.len(), cells[k0]);
                let (mut stack, mut n) = (vec![k0], 0);
                comp[k0] = id;
                while let Some(k) = stack.pop() {
                    n += 1;
                    let (i, j) = (k % nx, k / nx);
                    for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                        if a < nx && b < ny && cells[b * nx + a] == room && comp[b * nx + a] == usize::MAX {
                            comp[b * nx + a] = id;
                            stack.push(b * nx + a);
                        }
                    }
                }
                sizes.push((room, n));
            }
            let main = |room: i16| (0..sizes.len()).filter(|&c| sizes[c].0 == room).max_by_key(|&c| (sizes[c].1, std::cmp::Reverse(c)));
            let keep = [main(0), main(1)];
            for k in 0..nx * ny {
                if cells[k] >= 0 && Some(comp[k]) != keep[cells[k] as usize] {
                    cells[k] = 1 - cells[k];
                }
            }
        }
        finish_rooms(&mut rooms, &cells, nx);
        let mut lvl = connect(cells, rooms, nx, ny, st, front, ground, None, ext_free, rng);
        windows(&mut lvl, nx, ny, ground);
        return lvl;
    }

    if prog.cells
        && nx >= 7
        && ny >= 5
        && let Some((cells, mut rooms)) = cell_block(inside, nx, ny, st, prog.rooms[0].0)
    {
        let (cells, mut rooms2) = compact(cells, std::mem::take(&mut rooms));
        finish_rooms(&mut rooms2, &cells, nx);
        let public = if ground { Some(prog.rooms[0].0) } else { None };
        let mut lvl = connect(cells, rooms2, nx, ny, st, front, ground, public, ext_free, rng);
        windows(&mut lvl, nx, ny, ground);
        return lvl;
    }
    // (A floor too small for a cell block is a guardroom with a holding cell off it.)
    if prog.cells {
        kinds = if area >= 40 {
            vec![(prog.rooms[0].0, 2.0), ("holding cell", 1.0), ("storeroom", 1.0)]
        } else if area >= 12 {
            vec![(prog.rooms[0].0, 2.0), ("holding cell", 1.0)]
        } else {
            vec![(prog.rooms[0].0, 1.0)]
        };
    }
    // Large floors: a grid of corridors, every room opening onto one.
    let large = !prog.cells && area >= 150 && nx.min(ny) >= 12 && prog.rooms.len() >= 2 && !(ground && matches!(arch, Arch::Warehouse | Arch::Temple)) && !matches!(arch, Arch::Tower | Arch::Arena);
    let mut rooms: Vec<Room> = if large {
        let (gc, gr) = grid_rooms(inside, nx, ny, prog, st, front, ground, rng);
        cells = gc;
        gr
    } else {
    let corridor_ok = prog.corridor && kinds.len() >= 3 && ny >= 7 && nx >= 6;
    if corridor_ok {
        // A 1-square corridor down the long axis through the stair row.
        let cy = st[1].clamp(1, ny - 2);
        let h = R { x: 0, y: cy, w: nx, h: 1 };
        hall = Some(h);
        let top = R { x: 0, y: 0, w: nx, h: cy };
        let bot = R { x: 0, y: cy + 1, w: nx, h: ny - cy - 1 };
        let (mut a, mut bb): (Vec<(&str, f64)>, Vec<(&str, f64)>) = (Vec::new(), Vec::new());
        let (mut wa, mut wb) = (0.0, 0.0);
        for k in kinds.iter() {
            if wa * (bot.h as f64) <= wb * (top.h as f64) {
                a.push(*k);
                wa += k.1;
            } else {
                bb.push(*k);
                wb += k.1;
            }
        }
        // One row of rooms each side, every one on the corridor.
        strip(top, &a, &mut leaves, 0);
        let off = a.len();
        let mut lb = Vec::new();
        strip(bot, &bb, &mut lb, 0);
        leaves.extend(lb.into_iter().map(|(r, i)| (r, i + off)));
        kinds = a.into_iter().chain(bb).collect();
    } else {
        bsp(full, &kinds, st, rng, &mut leaves, 0);
    }

    // Assign program kinds to leaves: the first (public) room to the biggest leaf on the
    // front side (on the ground floor), the rest by size.
    let leaf_area = |r: &R| (r.y..r.y + r.h).map(|j| (r.x..r.x + r.w).filter(|&i| inside[j * nx + i]).count()).sum::<usize>();
    let touches_front = |r: &R| match front {
        0 => r.y == 0,
        1 => r.x + r.w == nx,
        2 => r.y + r.h == ny,
        _ => r.x == 0,
    };
    let mut order: Vec<usize> = (0..leaves.len()).collect();
    order.sort_by(|&p, &q| leaf_area(&leaves[q].0).cmp(&leaf_area(&leaves[p].0)).then(p.cmp(&q)));
    let mut by_weight: Vec<usize> = (0..kinds.len()).collect();
    by_weight.sort_by(|&p, &q| kinds[q].1.total_cmp(&kinds[p].1).then(p.cmp(&q)));
    let mut assign = vec![usize::MAX; leaves.len()];
    // The public room (the first of the program) on the ground floor: the biggest leaf with
    // open ground outside it (a door can be put there), on the street side if possible.
    let open_out = |r: &R| {
        (r.y..r.y + r.h).any(|j| {
            (r.x..r.x + r.w).any(|i| {
                inside[j * nx + i]
                    && [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| {
                        let (a, b) = (i as isize + di, j as isize + dj);
                        let out = a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || !inside[b as usize * nx + a as usize];
                        out && ext_free(a, b)
                    })
            })
        })
    };
    if ground && hall.is_none() && !order.is_empty() {
        // The biggest leaf with a way outside; the street side wins only a near tie.
        let open: Vec<usize> = order.iter().copied().filter(|&li| open_out(&leaves[li].0)).collect();
        let first = match open.first() {
            Some(&big) => open.iter().copied().find(|&li| touches_front(&leaves[li].0) && 4 * leaf_area(&leaves[li].0) >= 3 * leaf_area(&leaves[big].0)).unwrap_or(big),
            None => order[0],
        };
        assign[first] = 0;
    }
    let pre: Vec<usize> = assign.iter().copied().filter(|&k| k != usize::MAX).collect();
    let mut next_kind = by_weight.iter().copied().filter(|k| !pre.contains(k));
    for &li in &order {
        if assign[li] == usize::MAX {
            assign[li] = next_kind.next().unwrap_or(by_weight[0]);
        }
    }
    let mut rooms: Vec<Room> = Vec::new();
    for (li, (r, _)) in leaves.iter().enumerate() {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: kinds[assign[li]].0, squares: 0, raise_ft: 0.0, center: [0.0; 2] });
        for j in r.y..r.y + r.h {
            for i in r.x..r.x + r.w {
                if inside[j * nx + i] {
                    cells[j * nx + i] = id;
                }
            }
        }
    }
    if let Some(h) = hall {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: "hall", squares: 0, raise_ft: 0.0, center: [0.0; 2] });
        for i in h.x..h.x + h.w {
            if inside[h.y * nx + i] {
                cells[h.y * nx + i] = id;
            }
        }
    }
    rooms
    };
    // Keeps: a tower in each corner, a room of its own on every floor above the cellar.
    let t = if arch == Arch::Keep { keep_tower_squares(nx, ny) } else { 0 };
    if t > 0 {
        let kind: &'static str = if ground { "guard post" } else { "tower room" };
        for (cx, cy) in [(0, 0), (nx - t, 0), (0, ny - t), (nx - t, ny - t)] {
            let id = rooms.len() as i16;
            rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2] });
            for j in cy..cy + t {
                for i in cx..cx + t {
                    if inside[j * nx + i] {
                        cells[j * nx + i] = id;
                    }
                }
            }
        }
    }
    // Rooms split into pieces by the footprint shape keep their largest piece; the others
    // join the neighbour they share the longest wall with. Empty rooms are dropped.
    let mut cells = merge_fragments(cells, nx, ny);
    // A room that is nothing but the stair block is the stairwell of its neighbour.
    for r in 0..rooms.len() as i16 {
        let mine: Vec<usize> = (0..nx * ny).filter(|&k| cells[k] == r).collect();
        let all_stairs = !mine.is_empty() && mine.iter().all(|&k| { let (i, j) = (k % nx, k / nx); i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3] });
        if all_stairs {
            let nb = mine.iter().flat_map(|&k| {
                let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)]
            }).filter(|&(a, b)| a >= 0 && b >= 0 && a < nx as isize && b < ny as isize).map(|(a, b)| cells[b as usize * nx + a as usize]).find(|&c| c >= 0 && c != r);
            if let Some(nb) = nb {
                for &k in &mine {
                    cells[k] = nb;
                }
            }
        }
    }
    let (cells, mut rooms) = compact(cells, rooms);
    finish_rooms(&mut rooms, &cells, nx);
    let public = if ground { prog.rooms.first().map(|r| r.0) } else { None };
    let mut lvl = connect(cells, rooms, nx, ny, st, front, ground, public, ext_free, rng);
    windows(&mut lvl, nx, ny, ground);
    lvl
}

/// Cut `r` along x into one room per kind, widths by weight (at least 2 squares each).
fn strip(r: R, kinds: &[(&'static str, f64)], out: &mut Vec<(R, usize)>, base: usize) {
    if kinds.is_empty() || r.w == 0 || r.h == 0 {
        return;
    }
    let n = kinds.len().min(r.w / 2).max(1);
    let total: f64 = kinds[..n].iter().map(|k| k.1).sum();
    let mut x = r.x;
    let mut acc = 0.0;
    for (k, kind) in kinds[..n].iter().enumerate() {
        acc += kind.1;
        let x1 = if k + 1 == n { r.x + r.w } else { (r.x + ((r.w as f64) * acc / total).round() as usize).clamp(x + 2, r.x + r.w - 2 * (n - k - 1)) };
        out.push((R { x, y: r.y, w: x1 - x, h: r.h }, base + k));
        x = x1;
    }
}

/// Large floors: corridor rows about every 8 squares along the long axis (one through the
/// stairs where it can), cross corridors near both ends and every ~22 squares joining them,
/// and a single row of rooms in each band between, so every room opens onto a corridor. On
/// the ground floor the public room (the great hall) spans two bands by the front.
#[allow(clippy::too_many_arguments)]
fn grid_rooms(inside: &[bool], nx: usize, ny: usize, prog: &LevelProgram, st: [usize; 4], front: usize, ground: bool, rng: &mut Pcg32) -> (Vec<i16>, Vec<Room>) {
    // (u, v): u along the long axis, v across it.
    let long_x = nx >= ny;
    let (w, h) = if long_x { (nx, ny) } else { (ny, nx) };
    let idx = |u: usize, v: usize| if long_x { v * nx + u } else { u * nx + v };
    let (sv0, sv1) = if long_x { (st[1], st[1] + st[3]) } else { (st[0], st[0] + st[2]) };
    let m = ((h as f64 - 1.0) / 8.5).round().max(1.0) as usize;
    let mut rows: Vec<usize> = (0..m).map(|k| ((k + 1) * (h + 1) / (m + 1)).saturating_sub(1).clamp(3, h - 4)).collect();
    let mid = (sv0 + sv1) / 2;
    if let Some(r) = rows.iter_mut().min_by_key(|r| r.abs_diff(mid))
        && r.abs_diff(mid) <= 3
    {
        *r = mid.clamp(3, h - 4);
    }
    rows.sort();
    rows.dedup();
    let end = 3usize.min(w / 4);
    let mut cols = vec![end, w - 1 - end];
    let span = w - 1 - 2 * end;
    for k in 1..=span / 22 {
        cols.push(end + k * span / (span / 22 + 1));
    }
    cols.sort();
    cols.dedup();
    let (r0, r1) = (rows[0], *rows.last().unwrap());
    let mut cells = vec![-1i16; nx * ny];
    let mut rooms = vec![Room { kind: "hall", squares: 0, raise_ft: 0.0, center: [0.0; 2] }];
    let set = |cells: &mut Vec<i16>, u: usize, v: usize, id: i16| {
        if inside[idx(u, v)] {
            cells[idx(u, v)] = id;
        }
    };
    for &v in &rows {
        for u in 0..w {
            set(&mut cells, u, v, 0);
        }
    }
    // Cross corridors join the rows; the two at the ends run wall to wall (past the corner
    // towers, which open onto them).
    for &u in &cols {
        let (a, b) = if u == cols[0] || u == *cols.last().unwrap() { (0, h - 1) } else { (r0, r1) };
        for v in a..=b {
            set(&mut cells, u, v, 0);
        }
    }
    // On an irregular footprint some corridor runs end up apart: join each to the rest by the
    // shortest way across the floor.
    loop {
        let mut comp = vec![usize::MAX; nx * ny];
        let mut sizes: Vec<usize> = Vec::new();
        for s0 in 0..nx * ny {
            if cells[s0] != 0 || comp[s0] != usize::MAX {
                continue;
            }
            let c = sizes.len();
            let mut stack = vec![s0];
            comp[s0] = c;
            let mut size = 0;
            while let Some(k) = stack.pop() {
                size += 1;
                let (i, j) = (k % nx, k / nx);
                for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                    let (a, b) = (i as isize + di, j as isize + dj);
                    if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                        let m = b as usize * nx + a as usize;
                        if cells[m] == 0 && comp[m] == usize::MAX {
                            comp[m] = c;
                            stack.push(m);
                        }
                    }
                }
            }
            sizes.push(size);
        }
        if sizes.len() <= 1 {
            break;
        }
        let main = (0..sizes.len()).max_by_key(|&c| sizes[c]).unwrap();
        let other = (0..sizes.len()).find(|&c| c != main).unwrap();
        let mut prev = vec![usize::MAX; nx * ny];
        let mut q: std::collections::VecDeque<usize> = (0..nx * ny).filter(|&k| comp[k] == other).collect();
        for &k in &q {
            prev[k] = k;
        }
        let mut hit = None;
        while let Some(k) = q.pop_front() {
            if comp[k] == main {
                hit = Some(k);
                break;
            }
            let (i, j) = (k % nx, k / nx);
            for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i as isize + di, j as isize + dj);
                if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                    let m = b as usize * nx + a as usize;
                    if inside[m] && prev[m] == usize::MAX {
                        prev[m] = k;
                        q.push_back(m);
                    }
                }
            }
        }
        match hit {
            Some(mut k) => {
                while prev[k] != k {
                    cells[k] = 0;
                    k = prev[k];
                }
            }
            // Unreachable across the floor: let it go to the rooms.
            None => {
                for k in 0..nx * ny {
                    if comp[k] == other {
                        cells[k] = -1;
                    }
                }
            }
        }
    }
    // Bands between the rows (and the walls): (v0, v1) half-open.
    let mut edges = vec![0usize];
    for &r in &rows {
        edges.push(r);
        edges.push(r + 1);
    }
    edges.push(h);
    let bands: Vec<(usize, usize)> = edges.chunks(2).map(|c| (c[0], c[1])).filter(|c| c.1 > c.0).collect();
    // The public room on the ground floor: two bands and the corridor between, on the front
    // side, over the middle half (or from the front end when the front is a short side).
    let mut public: Option<(usize, usize, usize, usize)> = None;
    if ground && big_room(prog.rooms[0].0) && bands.len() >= 2 {
        let front_v_low = if long_x { front == 0 } else { front == 3 };
        let front_v = if long_x { matches!(front, 0 | 2) } else { matches!(front, 1 | 3) };
        let pair = if !front_v || front_v_low { 0 } else { bands.len() - 2 };
        let (v0, v1) = (bands[pair].0, bands[pair + 1].1);
        let (u0, u1) = if front_v {
            (w / 4, w - w / 4)
        } else if (long_x && front == 3) || (!long_x && front == 0) {
            (0, w / 2)
        } else {
            (w - w / 2, w)
        };
        public = Some((u0, u1, v0, v1));
        let id = rooms.len() as i16;
        rooms.push(Room { kind: prog.rooms[0].0, squares: 0, raise_ft: 0.0, center: [0.0; 2] });
        for v in v0..v1 {
            for u in u0..u1 {
                set(&mut cells, u, v, id);
            }
        }
    }
    let in_public = |u: usize, v: usize| public.is_some_and(|(u0, u1, v0, v1)| u >= u0 && u < u1 && v >= v0 && v < v1);
    // Rooms: each band's runs between cross corridors (inner bands) cut along u.
    let mut pieces: Vec<(usize, usize, usize, usize)> = Vec::new();
    for &(v0, v1) in &bands {
        let inner = v0 > r0 && v1 <= r1 + 1;
        let mut cuts: Vec<usize> = vec![0];
        for &c in &cols {
            if inner || c == cols[0] || c == *cols.last().unwrap() {
                cuts.push(c);
                cuts.push(c + 1);
            }
        }
        cuts.push(w);
        for seg in cuts.chunks(2) {
            let (mut a, b) = (seg[0], seg[1]);
            // Skip the public room's columns.
            while a < b {
                if (v0..v1).all(|v| in_public(a, v)) {
                    a += 1;
                    continue;
                }
                let mut e = a;
                while e < b && !(v0..v1).all(|v| in_public(e, v)) {
                    e += 1;
                }
                // Cut [a, e) into rooms about 40 squares each.
                let depth = v1 - v0;
                let target = ((40.0 / depth as f64).round() as usize).clamp(3, 9);
                let mut x = a;
                while x < e {
                    let wdt = (target + rng.below(3) as usize).saturating_sub(1).max(3);
                    let x1 = if e - x < wdt + 3 { e } else { x + wdt };
                    pieces.push((x, x1, v0, v1));
                    x = x1;
                }
                a = e;
            }
        }
    }
    // Program rooms (after the public room) to the biggest pieces, fillers to the rest.
    let mut kinds: Vec<(&'static str, f64)> = prog.rooms.iter().copied().skip(if public.is_some() { 1 } else { 0 }).collect();
    kinds.sort_by(|a, b| b.1.total_cmp(&a.1));
    let filler = prog.filler.unwrap_or(prog.rooms[0].0);
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    let size = |p: &(usize, usize, usize, usize)| (p.1 - p.0) * (p.3 - p.2);
    order.sort_by(|&a, &b| size(&pieces[b]).cmp(&size(&pieces[a])).then(a.cmp(&b)));
    let mut kind_of = vec![filler; pieces.len()];
    for (k, &pi) in order.iter().enumerate() {
        if let Some(&(kk, _)) = kinds.get(k) {
            kind_of[pi] = kk;
        }
    }
    for (pi, &(u0, u1, v0, v1)) in pieces.iter().enumerate() {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: kind_of[pi], squares: 0, raise_ft: 0.0, center: [0.0; 2] });
        for v in v0..v1 {
            for u in u0..u1 {
                if !in_public(u, v) && cells[idx(u, v)] != 0 {
                    set(&mut cells, u, v, id);
                }
            }
        }
    }
    (cells, rooms)
}

/// A cell block: a strip beside the stairs (full height, the stairs' end of the building) for
/// the guards, a spine corridor along it, and corridors off the spine every five rows with a
/// row of 10 × 10 ft cells on each side. Every cell touches a corridor; rows left over at the
/// far end become corridor.
///
/// Irregular footprints: cells only in full-height columns (their corridor rows are whole);
/// partial columns on the stairs' side join the strip, those at the far end are corridor.
/// `None` when fewer than two full columns are left for cells.
fn cell_block(inside: &[bool], nx: usize, ny: usize, st: [usize; 4], strip_kind: &'static str) -> Option<(Vec<i16>, Vec<Room>)> {
    let full: Vec<bool> = (0..nx).map(|i| (0..ny).all(|j| inside[j * nx + i])).collect();
    let west = st[0] + st[2] / 2 < nx / 2;
    let sw = if west { (st[0] + st[2]).max(3) } else { (nx - st[0]).max(3) };
    let sw = sw.min(nx - 3);
    // The spine: the first full column past the strip.
    let spine = if west { (sw..nx).find(|&i| full[i])? } else { (0..nx - sw).rev().find(|&i| full[i])? };
    let beyond = |i: usize| if west { i > spine } else { i < spine };
    if (0..nx).filter(|&i| beyond(i) && full[i]).count() < 2 {
        return None;
    }
    let in_strip = |i: usize| if west { i < spine } else { i > spine };
    let mut cells = vec![-1i16; nx * ny];
    let mut rooms = vec![Room { kind: strip_kind, squares: 0, raise_ft: 0.0, center: [0.0; 2] }, Room { kind: "corridor", squares: 0, raise_ft: 0.0, center: [0.0; 2] }];
    // Row roles: 2 = corridor (every fifth row from 2, and the rows the pattern leaves without
    // one at the far end); 0/1 = cell rows.
    let role = |j: usize| -> u8 {
        let m = j % 5;
        let block_end = (j / 5) * 5 + 5;
        if m == 2 {
            2
        } else if m < 2 && block_end > ny && ny - (j / 5) * 5 <= 2 {
            // The last block has no corridor row: its last row is one.
            if j + 1 == ny { 2 } else { 0 }
        } else {
            0
        }
    };
    let cell_cols: Vec<usize> = (0..nx).filter(|&i| beyond(i) && full[i]).collect();
    for j in 0..ny {
        for i in 0..nx {
            if !inside[j * nx + i] {
                continue;
            }
            cells[j * nx + i] = if in_strip(i) {
                0
            } else if i == spine || role(j) == 2 || !full[i] {
                1
            } else {
                -2
            };
        }
    }
    // Cells: 2 columns wide (the last may be 1) and the cell rows between corridors (at most 2).
    let mut j = 0;
    while j < ny {
        if role(j) == 2 {
            j += 1;
            continue;
        }
        let mut h = 1;
        while h < 2 && j + h < ny && role(j + h) != 2 {
            h += 1;
        }
        let mut k = 0;
        while k < cell_cols.len() {
            let w = if cell_cols.len() - k == 3 { 1 } else { 2.min(cell_cols.len() - k) };
            let id = rooms.len() as i16;
            let mut any = false;
            for b in j..j + h {
                for &i in &cell_cols[k..k + w] {
                    if cells[b * nx + i] == -2 {
                        cells[b * nx + i] = id;
                        any = true;
                    }
                }
            }
            if any {
                rooms.push(Room { kind: "cell", squares: 0, raise_ft: 0.0, center: [0.0; 2] });
            }
            k += w;
        }
        j += h;
    }
    for c in cells.iter_mut() {
        if *c == -2 {
            *c = 1;
        }
    }
    Some((cells, rooms))
}

/// Recursive split of a rectangle among rooms (weights), never cutting through the stairs.
fn bsp(r: R, rooms: &[(&'static str, f64)], st: [usize; 4], rng: &mut Pcg32, out: &mut Vec<(R, usize)>, base: usize) {
    if rooms.len() <= 1 || r.w * r.h < 8 {
        // Leftover rooms (no space) merge into this leaf.
        out.push((r, base));
        if rooms.len() > 1 {
            // Mark the rest as leaves of zero size so indices stay aligned.
            for k in 1..rooms.len() {
                out.push((R { x: r.x, y: r.y, w: 0, h: 0 }, base + k));
            }
        }
        return;
    }
    let total: f64 = rooms.iter().map(|k| k.1).sum();
    // Split the list where the prefix weight is closest to half.
    let mut acc = 0.0;
    let mut k = 1;
    let mut best = f64::MAX;
    for i in 1..rooms.len() {
        acc += rooms[i - 1].1;
        let d = (acc / total - 0.5).abs();
        if d < best {
            best = d;
            k = i;
        }
    }
    let frac = rooms[..k].iter().map(|x| x.1).sum::<f64>() / total;
    let horizontal = r.w >= r.h; // cut across the long side
    let span = if horizontal { r.w } else { r.h };
    let jitter = (rng.next_f64() - 0.5) * 0.12;
    let mut cut = crate::core::round(span as f64 * (frac + jitter)) as usize;
    // Sides at least 2 squares where the span allows it.
    let edge = if span >= 4 { 2 } else { 1 };
    cut = cut.clamp(edge, span - edge);
    // Keep the stair block whole.
    let (s0, s1, o0, o1, r0, r1) = if horizontal { (st[0], st[0] + st[2], st[1], st[1] + st[3], r.y, r.y + r.h) } else { (st[1], st[1] + st[3], st[0], st[0] + st[2], r.x, r.x + r.w) };
    let base_pos = if horizontal { r.x } else { r.y };
    let pos = base_pos + cut;
    if o0 < r1 && o1 > r0 && pos > s0 && pos < s1 {
        let (lo, hi) = (s0 - base_pos, s1 - base_pos);
        let cand = if cut - lo <= hi - cut { lo } else { hi };
        if cand >= 2 && span - cand >= 2 {
            cut = cand;
        } else if hi >= 2 && span - hi >= 2 {
            cut = hi;
        } else if lo >= 2 && span - lo >= 2 {
            cut = lo;
        }
    }
    let (a, b) = if horizontal {
        (R { x: r.x, y: r.y, w: cut, h: r.h }, R { x: r.x + cut, y: r.y, w: r.w - cut, h: r.h })
    } else {
        (R { x: r.x, y: r.y, w: r.w, h: cut }, R { x: r.x, y: r.y + cut, w: r.w, h: r.h - cut })
    };
    bsp(a, &rooms[..k], st, rng, out, base);
    bsp(b, &rooms[k..], st, rng, out, base + k);
}

fn merge_fragments(mut cells: Vec<i16>, nx: usize, ny: usize) -> Vec<i16> {
    loop {
        // Connected pieces of every room.
        let mut piece = vec![usize::MAX; nx * ny];
        let mut pieces: Vec<(i16, Vec<usize>)> = Vec::new();
        for k in 0..nx * ny {
            if cells[k] < 0 || piece[k] != usize::MAX {
                continue;
            }
            let id = pieces.len();
            let mut members = vec![k];
            piece[k] = id;
            let mut i = 0;
            while i < members.len() {
                let q = members[i];
                i += 1;
                let (x, y) = ((q % nx) as isize, (q / nx) as isize);
                for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                    let (a, b) = (x + dx, y + dy);
                    if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize {
                        continue;
                    }
                    let r = b as usize * nx + a as usize;
                    if cells[r] == cells[k] && piece[r] == usize::MAX {
                        piece[r] = id;
                        members.push(r);
                    }
                }
            }
            pieces.push((cells[k], members));
        }
        // A piece that is not its room's largest.
        let largest = |room: i16| pieces.iter().filter(|p| p.0 == room).map(|p| p.1.len()).max().unwrap_or(0);
        let Some(p) = pieces.iter().find(|p| p.1.len() < largest(p.0) || (p.1.len() == largest(p.0) && pieces.iter().any(|q| q.0 == p.0 && q.1.len() == p.1.len() && q.1[0] < p.1[0]))) else {
            return cells;
        };
        // Its longest shared wall with another room.
        let mut shared: std::collections::BTreeMap<i16, usize> = Default::default();
        for &q in &p.1 {
            let (x, y) = ((q % nx) as isize, (q / nx) as isize);
            for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (x + dx, y + dy);
                if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize {
                    continue;
                }
                let r = cells[b as usize * nx + a as usize];
                if r >= 0 && r != p.0 {
                    *shared.entry(r).or_insert(0) += 1;
                }
            }
        }
        let Some((&to, _)) = shared.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))) else {
            // An isolated piece (the footprint itself is in pieces): leave it.
            return cells;
        };
        for &q in &p.1 {
            cells[q] = to;
        }
    }
}

fn compact(cells: Vec<i16>, rooms: Vec<Room>) -> (Vec<i16>, Vec<Room>) {
    let mut used = vec![false; rooms.len()];
    for &c in &cells {
        if c >= 0 {
            used[c as usize] = true;
        }
    }
    let mut map = vec![-1i16; rooms.len()];
    let mut out = Vec::new();
    for (i, r) in rooms.into_iter().enumerate() {
        if used[i] {
            map[i] = out.len() as i16;
            out.push(r);
        }
    }
    (cells.into_iter().map(|c| if c >= 0 { map[c as usize] } else { -1 }).collect(), out)
}

fn finish_rooms(rooms: &mut [Room], cells: &[i16], nx: usize) {
    let mut sum = vec![(0.0f64, 0.0f64, 0usize); rooms.len()];
    for (k, &c) in cells.iter().enumerate() {
        if c >= 0 {
            let s = &mut sum[c as usize];
            s.0 += (k % nx) as f64 + 0.5;
            s.1 += (k / nx) as f64 + 0.5;
            s.2 += 1;
        }
    }
    for (r, s) in rooms.iter_mut().zip(sum) {
        r.squares = s.2;
        if s.2 > 0 {
            r.center = [(s.0 / s.2 as f64) as f32, (s.1 / s.2 as f64) as f32];
        }
    }
}

/// Doors (a spanning tree over room adjacency from the entrance or stairs, plus a few loops),
/// the front door, and the wall runs.
#[allow(clippy::too_many_arguments)]
fn connect(cells: Vec<i16>, rooms: Vec<Room>, nx: usize, ny: usize, st: [usize; 4], front: usize, ground: bool, public: Option<&str>, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
    let n = rooms.len();
    let at = |i: isize, j: isize| if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { cells[j as usize * nx + i as usize] };
    let on_stairs = |i: usize, j: usize| i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3];
    // Shared edges per room pair: (a, b) -> edges (x, y, vertical) where the edge lies
    // between square (x, y) and its right (vertical) or lower neighbour.
    let mut shared: std::collections::BTreeMap<(i16, i16), Vec<(usize, usize, bool)>> = Default::default();
    for j in 0..ny {
        for i in 0..nx {
            let a = cells[j * nx + i];
            if a < 0 {
                continue;
            }
            for (di, dj, vert) in [(1isize, 0isize, true), (0, 1, false)] {
                let bnb = at(i as isize + di, j as isize + dj);
                // Cells open only onto corridors and guardrooms, never through one another.
                if bnb >= 0 && bnb != a && !(rooms[a as usize].kind == "cell" && rooms[bnb as usize].kind == "cell") {
                    shared.entry((a.min(bnb), a.max(bnb))).or_default().push((i, j, vert));
                }
            }
        }
    }
    let edge_seg = |x: usize, y: usize, vert: bool| -> ([f32; 2], [f32; 2]) {
        if vert { ([(x + 1) as f32, y as f32], [(x + 1) as f32, (y + 1) as f32]) } else { ([x as f32, (y + 1) as f32], [(x + 1) as f32, (y + 1) as f32]) }
    };
    let mut doors: Vec<Door> = Vec::new();
    let mut door_edges: std::collections::BTreeSet<(usize, usize, bool)> = Default::default();
    // Spanning tree (BFS) from the stairs room.
    let stair_room = cells[st[1] * nx + st[0]].max(0) as usize;
    let mut seen = vec![false; n];
    let mut queue = std::collections::VecDeque::new();
    if n > 0 {
        seen[stair_room] = true;
        queue.push_back(stair_room);
    }
    let mut tree: Vec<(i16, i16)> = Vec::new();
    while let Some(r) = queue.pop_front() {
        // Neighbours in a stable order, the widest shared wall first.
        let mut nbs: Vec<(usize, i16)> = shared.iter().filter_map(|(&(a, b), e)| if a as usize == r { Some((e.len(), b)) } else if b as usize == r { Some((e.len(), a)) } else { None }).collect();
        nbs.sort_by(|p, q| q.0.cmp(&p.0).then(p.1.cmp(&q.1)));
        for (_, s) in nbs {
            if !seen[s as usize] {
                seen[s as usize] = true;
                queue.push_back(s as usize);
                tree.push(((r as i16).min(s), (r as i16).max(s)));
            }
        }
    }
    // Circulation (corridors, halls, landings, the public room): every room beside it opens
    // onto it, and passages between two such rooms are dropped (no walking through one room
    // to reach the next). Tree edges still join rooms with no corridor of their own.
    let circ = |r: usize| matches!(rooms[r].kind, "hall" | "corridor" | "landing") || public.is_some_and(|k| rooms[r].kind == k);
    let mut onto: std::collections::BTreeSet<(i16, i16)> = Default::default();
    let mut served = vec![false; n];
    if (0..n).any(circ) {
        for r in 0..n {
            if circ(r) {
                continue;
            }
            let best = shared.iter().filter(|&(&(a, b), _)| (a as usize == r && circ(b as usize)) || (b as usize == r && circ(a as usize))).max_by_key(|(_, e)| e.len()).map(|(k, _)| *k);
            if let Some(k) = best {
                onto.insert(k);
                served[r] = true;
            }
        }
        // Circulation spaces open into each other.
        for &(a, b) in shared.keys() {
            if circ(a as usize) && circ(b as usize) {
                onto.insert((a, b));
            }
        }
    }
    for (&(a, b), edges) in &shared {
        let in_tree = tree.contains(&(a, b)) && !(served[a as usize] && served[b as usize]);
        let loops = if served.iter().any(|&s| s) { 0.05 } else { 0.2 };
        if !onto.contains(&(a, b)) && !in_tree && rng.next_f64() > loops {
            continue;
        }
        // A doorway off the stairs where the wall allows it.
        let clear: Vec<&(usize, usize, bool)> = edges
            .iter()
            .filter(|&&(x, y, vert)| {
                let (nx2, ny2) = if vert { (x + 1, y) } else { (x, y + 1) };
                !on_stairs(x, y) && !on_stairs(nx2, ny2)
            })
            .collect();
        let &(x, y, vert) = if clear.is_empty() { &edges[edges.len() / 2] } else { clear[clear.len() / 2] };
        door_edges.insert((x, y, vert));
        let (p, q) = edge_seg(x, y, vert);
        doors.push(Door { a: p, b: q, kind: "door", rooms: [a, b] });
    }
    // A room still more than one room from the circulation (here a great or mess hall, and a
    // guardroom, the guards' way through a keep, count too) gets a door to its neighbour
    // nearest it (rooms between two others, corner towers).
    let hub = |r: usize| circ(r) || matches!(rooms[r].kind, "great hall" | "mess hall" | "guardroom");
    if (0..n).any(hub) {
        for _ in 0..n {
            let mut hops = vec![usize::MAX; n];
            let mut q = std::collections::VecDeque::new();
            for r in (0..n).filter(|&r| hub(r)) {
                hops[r] = 0;
                q.push_back(r);
            }
            while let Some(r) = q.pop_front() {
                for d in &doors {
                    let [a, b] = d.rooms;
                    let o = if a == r as i16 { b } else if b == r as i16 { a } else { continue };
                    if o >= 0 && hops[o as usize] == usize::MAX {
                        hops[o as usize] = hops[r] + 1;
                        q.push_back(o as usize);
                    }
                }
            }
            let deep = (0..n).filter(|&r| hops[r] != usize::MAX && hops[r] >= 3).find_map(|r| {
                shared
                    .iter()
                    .filter_map(|(&(a, b), e)| {
                        let o = if a as usize == r { b } else if b as usize == r { a } else { return None };
                        (hops[o as usize] + 1 < hops[r]).then_some(((a, b), hops[o as usize], e))
                    })
                    .min_by_key(|x| x.1)
            });
            let Some(((a, b), _, edges)) = deep else { break };
            let &(x, y, vert) = &edges[edges.len() / 2];
            if door_edges.insert((x, y, vert)) {
                let (p, q) = edge_seg(x, y, vert);
                doors.push(Door { a: p, b: q, kind: "door", rooms: [a, b] });
            } else {
                break;
            }
        }
    }
    // The front door on the ground floor: onto open ground (never against a neighbour's wall),
    // into the public room if it has an outside wall, on the front side if possible, at the
    // middle of that room's run of such walls; off the stairs.
    if ground {
        // (side, i, j, room): side 0 top, 1 right, 2 bottom, 3 left.
        let mut cands: Vec<(usize, usize, usize, i16)> = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let c = cells[j * nx + i];
                if c < 0 || on_stairs(i, j) {
                    continue;
                }
                for (side, (di, dj)) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
                    let (a, b) = (i as isize + di, j as isize + dj);
                    if at(a, b) < 0 && ext_free(a, b) {
                        cands.push((side, i, j, c));
                    }
                }
            }
        }
        let rank = |c: &(usize, usize, usize, i16)| {
            let is_public = public.is_some_and(|k| rooms[c.3 as usize].kind == k);
            (is_public, c.0 == front, rooms[c.3 as usize].squares)
        };
        if let Some(best) = cands.iter().max_by(|x, y| rank(x).cmp(&rank(y)).then(y.1.cmp(&x.1)).then(y.2.cmp(&x.2))).copied() {
            let mine: Vec<&(usize, usize, usize, i16)> = cands.iter().filter(|c| c.3 == best.3 && c.0 == best.0).collect();
            let &&(side, i, j, r) = &mine[mine.len() / 2];
            let (p, q) = match side {
                0 => ([i as f32, j as f32], [(i + 1) as f32, j as f32]),
                1 => ([(i + 1) as f32, j as f32], [(i + 1) as f32, (j + 1) as f32]),
                2 => ([i as f32, (j + 1) as f32], [(i + 1) as f32, (j + 1) as f32]),
                _ => ([i as f32, j as f32], [i as f32, (j + 1) as f32]),
            };
            doors.push(Door { a: p, b: q, kind: "front", rooms: [r, -1] });
        }
    }
    // Wall runs: every square edge between different rooms (or a room and the outside) that
    // is not a door, merged along straight lines.
    let mut h_edges: Vec<(usize, usize, bool)> = Vec::new(); // (x, y line, exterior) horizontal unit edges at y
    let mut v_edges: Vec<(usize, usize, bool)> = Vec::new(); // (x line, y, exterior)
    let is_door = |a: [f32; 2], b: [f32; 2]| doors.iter().any(|d| (d.a == a && d.b == b) || (d.a == b && d.b == a));
    for j in 0..=ny {
        for i in 0..nx {
            let (up, dn) = (at(i as isize, j as isize - 1), at(i as isize, j as isize));
            if up != dn && (up >= 0 || dn >= 0) && !is_door([i as f32, j as f32], [(i + 1) as f32, j as f32]) {
                h_edges.push((i, j, up < 0 || dn < 0));
            }
        }
    }
    for i in 0..=nx {
        for j in 0..ny {
            let (lf, rt) = (at(i as isize - 1, j as isize), at(i as isize, j as isize));
            if lf != rt && (lf >= 0 || rt >= 0) && !is_door([i as f32, j as f32], [i as f32, (j + 1) as f32]) {
                v_edges.push((i, j, lf < 0 || rt < 0));
            }
        }
    }
    let mut walls: Vec<Wall> = Vec::new();
    h_edges.sort_by_key(|e| (e.1, e.0));
    for e in &h_edges {
        if let Some(w) = walls.last_mut().filter(|w| w.a[1] == w.b[1] && w.b[1] == e.1 as f32 && w.b[0] == e.0 as f32 && w.exterior == e.2) {
            w.b[0] += 1.0;
        } else {
            walls.push(Wall { a: [e.0 as f32, e.1 as f32], b: [(e.0 + 1) as f32, e.1 as f32], exterior: e.2 });
        }
    }
    v_edges.sort_by_key(|e| (e.0, e.1));
    for e in &v_edges {
        if let Some(w) = walls.last_mut().filter(|w| w.a[0] == w.b[0] && w.b[0] == e.0 as f32 && w.b[1] == e.1 as f32 && w.exterior == e.2) {
            w.b[1] += 1.0;
        } else {
            walls.push(Wall { a: [e.0 as f32, e.1 as f32], b: [e.0 as f32, (e.1 + 1) as f32], exterior: e.2 });
        }
    }
    Level { z: 0, name: String::new(), elevation_ft: 0.0, cells, rooms, walls, doors, windows: Vec::new(), furniture: Vec::new(), roof: false, has_stairs: true, natural: false, paths: Vec::new(), links: Vec::new() }
}

/// Windows on exterior walls above ground: every third square along a run, off doors.
fn windows(lvl: &mut Level, _nx: usize, _ny: usize, _ground: bool) {
    let mut out = Vec::new();
    for w in &lvl.walls {
        if !w.exterior {
            continue;
        }
        let len = ((w.b[0] - w.a[0]).abs() + (w.b[1] - w.a[1]).abs()) as usize;
        // (f32::signum(0.0) is 1.0: compare explicitly.)
        let dir = |d: f32| if d > 0.0 { 1.0 } else if d < 0.0 { -1.0 } else { 0.0 };
        let (dx, dy) = (dir(w.b[0] - w.a[0]), dir(w.b[1] - w.a[1]));
        let mut k = 1;
        while k + 1 < len {
            let p = [w.a[0] + dx * k as f32, w.a[1] + dy * k as f32];
            out.push([p[0], p[1], p[0] + dx, p[1] + dy]);
            k += 3;
        }
    }
    lvl.windows = out;
}

/// Pieces a room must have (drinking rooms: a bar and two tables), tried in smaller sizes
/// and anywhere in the room when the preferred spot is not free.
fn required(kind: &str, item: &str) -> bool {
    matches!((kind, item), ("common room" | "taproom", "bar" | "table"))
}

/// Inns, taverns and breweries: the public room (taproom) is where guests come in and go up.
fn is_pub(arch: Arch) -> bool {
    matches!(arch, Arch::Tavern | Arch::Inn | Arch::Brewery)
}

/// Furniture by room rules: wall pieces along walls, free pieces clear of them; never on the
/// stairs or a doorway (the squares either side of every door stay open), and never cutting
/// a room's doors and stairs off from each other.
fn furnish(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], rng: &mut Pcg32) {
    let st = if lvl.has_stairs { st } else { [0, 0, 0, 0] };
    let mut taken = vec![false; nx * ny];
    for j in st[1]..st[1] + st[3] {
        for i in st[0]..st[0] + st[2] {
            taken[j * nx + i] = true;
        }
    }
    // Stairs need a free approach square at each end.
    for (i, j) in [(st[0] as isize - 1, st[1] as isize), ((st[0] + st[2]) as isize, st[1] as isize), (st[0] as isize, st[1] as isize - 1), (st[0] as isize, (st[1] + st[3]) as isize)] {
        if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
            taken[j as usize * nx + i as usize] = true;
        }
    }
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        for (i, j) in sq {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
                taken[j as usize * nx + i as usize] = true;
            }
        }
    }
    let cells = lvl.cells.clone();
    let room_at = |i: isize, j: isize| if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { cells[j as usize * nx + i as usize] };
    let mut blocked = vec![false; nx * ny];
    // Pieces already in place (spiral stairs) stay, with a clear square around them.
    let mut furniture: Vec<Item> = std::mem::take(&mut lvl.furniture);
    for f in &furniture {
        for j in (f.y as usize).saturating_sub(1)..((f.y + f.h) as usize + 1).min(ny) {
            for i in (f.x as usize).saturating_sub(1)..((f.x + f.w) as usize + 1).min(nx) {
                taken[j * nx + i] = true;
            }
        }
    }
    for (ri, room) in lvl.rooms.iter().enumerate() {
        let ri = ri as i16;
        let mine: Vec<(usize, usize)> = (0..nx * ny).filter(|&k| cells[k] == ri).map(|k| (k % nx, k / nx)).collect();
        if mine.is_empty() {
            continue;
        }
        let touches = |i: usize, j: usize| [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| room_at(i as isize + di, j as isize + dj) != ri);
        // A room whose required pieces don't all fit is furnished again from scratch with
        // other random choices (a bar across the only aisle leaves no room for tables).
        let (taken0, blocked0, n0) = (taken.clone(), blocked.clone(), furniture.len());
        for attempt in 0..8 {
        if attempt > 0 {
            taken.clone_from(&taken0);
            blocked.clone_from(&blocked0);
            furniture.truncate(n0);
        }
        let mut missing = false;
        for &(kind, place, fixed, per) in furnishing(room.kind) {
            let must = required(room.kind, kind);
            let mut count = fixed as usize + if per > 0 { room.squares / per as usize } else { 0 };
            // Guest rooms over 15 ft each way take more beds (one per ~10 squares).
            if kind == "bed" && room.kind == "guest room" {
                let (x0, x1) = mine.iter().fold((usize::MAX, 0), |a, &(x, _)| (a.0.min(x), a.1.max(x)));
                let (y0, y1) = mine.iter().fold((usize::MAX, 0), |a, &(_, y)| (a.0.min(y), a.1.max(y)));
                count = if (x1 - x0 + 1).min(y1 - y0 + 1) > 3 { (room.squares / 10).max(2) } else { 1 };
            }
            let (name, cover, blocks, height) = item_info(kind);
            for n_placed in 0..count.min(12) {
                if matches!(place, Place::Seated | Place::Booth) {
                    let ok = if matches!(place, Place::Seated) {
                        seat_table(&cells, nx, ny, ri, must, &mut taken, &mut blocked, &mut furniture, rng, |b| room_connected(lvl, nx, ny, st, ri, b))
                    } else {
                        booth(&cells, nx, ny, ri, &mut taken, &mut blocked, &mut furniture, rng, |b| room_connected(lvl, nx, ny, st, ri, b))
                    };
                    if !ok {
                        if must && n_placed < fixed as usize {
                            missing = true;
                        }
                        break;
                    }
                    continue;
                }
                let (a, b) = match place {
                    Place::Wall(l, d) | Place::Center(l, d) => (l as usize, d as usize),
                    Place::Seated | Place::Booth => unreachable!(),
                };
                // Sizes to try: the preferred one, then (for required pieces) smaller.
                let mut sizes = vec![(a, b)];
                if must {
                    for s in [(2, 1), (1, 1)] {
                        if !sizes.contains(&s) && s.0 * s.1 < a * b {
                            sizes.push(s);
                        }
                    }
                    // Small rooms: compact pieces first (a short bar, stools-and-table).
                    if mine.len() < 16 {
                        let small = if kind == "bar" { 2 } else { 1 };
                        sizes.retain(|s| s.0 * s.1 <= small);
                    }
                }
                let strict = [true, false];
                let mut placed = false;
                'search: for &(pa, pb) in &sizes {
                    for &strict in strict.iter().take(if must { 2 } else { 1 }) {
                        let mut cands: Vec<(usize, usize, usize, usize)> = Vec::new();
                        for &(x, y) in &mine {
                            let orients: &[(usize, usize)] = if pa == pb { &[(pa, pb)] } else { &[(pa, pb), (pb, pa)] };
                            for &(w, h) in orients {
                                if x + w > nx || y + h > ny {
                                    continue;
                                }
                                if !(y..y + h).all(|j| (x..x + w).all(|i| cells[j * nx + i] == ri && !taken[j * nx + i])) {
                                    continue;
                                }
                                let edge_n = (y..y + h).flat_map(|j| (x..x + w).map(move |i| (i, j))).filter(|&(i, j)| touches(i, j)).count();
                                let ok = !strict
                                    || match place {
                                        Place::Wall(..) => edge_n >= w.max(h).min(w * h),
                                        Place::Center(..) => edge_n == 0 || mine.len() < 12,
                                        Place::Seated | Place::Booth => true,
                                    };
                                if ok {
                                    cands.push((x, y, w, h));
                                }
                            }
                        }
                        // Random order; the first that keeps the room passable wins.
                        for k in (1..cands.len()).rev() {
                            cands.swap(k, rng.below(k as u32 + 1) as usize);
                        }
                        for (x, y, w, h) in cands {
                            if blocks {
                                for j in y..y + h {
                                    for i in x..x + w {
                                        blocked[j * nx + i] = true;
                                    }
                                }
                                if !room_connected(lvl, nx, ny, st, ri, &blocked) {
                                    for j in y..y + h {
                                        for i in x..x + w {
                                            blocked[j * nx + i] = false;
                                        }
                                    }
                                    continue;
                                }
                            }
                            for j in y..y + h {
                                for i in x..x + w {
                                    taken[j * nx + i] = true;
                                }
                            }
                            // Free pieces keep a one-square aisle around them in bigger rooms.
                            if matches!(place, Place::Center(..)) && mine.len() >= 12 && strict {
                                for j in y.saturating_sub(1)..(y + h + 1).min(ny) {
                                    for i in x.saturating_sub(1)..(x + w + 1).min(nx) {
                                        if cells[j * nx + i] == ri {
                                            taken[j * nx + i] = true;
                                        }
                                    }
                                }
                            }
                            furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover, blocks_move: blocks, height_ft: height, hazard: None });
                            placed = true;
                            break 'search;
                        }
                    }
                }
                if !placed {
                    if must && n_placed < fixed as usize {
                        missing = true;
                    }
                    break;
                }
            }
        }
        if !missing {
            break;
        }
        }
    }
    lvl.furniture = furniture;
}

fn push_item(furniture: &mut Vec<Item>, kind: &'static str, x: usize, y: usize) {
    let (name, cover, blocks, height) = item_info(kind);
    furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: 1, h: 1, cover, blocks_move: blocks, height_ft: height, hazard: None });
}

/// A one-square round table with chairs on its open sides: four chairs and a clear ring around
/// it where the room allows, else fewer (a required table takes any free square).
#[allow(clippy::too_many_arguments)]
fn seat_table(
    cells: &[i16],
    nx: usize,
    ny: usize,
    ri: i16,
    must: bool,
    taken: &mut [bool],
    blocked: &mut [bool],
    furniture: &mut Vec<Item>,
    rng: &mut Pcg32,
    passable: impl Fn(&[bool]) -> bool,
) -> bool {
    let free = |taken: &[bool], i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && cells[j as usize * nx + i as usize] == ri && !taken[j as usize * nx + i as usize];
    let needs: &[usize] = if must { &[4, 3, 2, 1, 0] } else { &[4, 3] };
    for &need in needs {
        let mut cands: Vec<(usize, usize)> = (0..nx * ny)
            .filter(|&k| cells[k] == ri && !taken[k])
            .map(|k| (k % nx, k / nx))
            .filter(|&(i, j)| [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)].iter().filter(|&&(di, dj)| free(taken, i as isize + di, j as isize + dj)).count() >= need)
            .collect();
        for k in (1..cands.len()).rev() {
            cands.swap(k, rng.below(k as u32 + 1) as usize);
        }
        for (i, j) in cands {
            blocked[j * nx + i] = true;
            if !passable(blocked) {
                blocked[j * nx + i] = false;
                continue;
            }
            let chairs: Vec<(usize, usize)> = [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .map(|&(di, dj)| (i as isize + di, j as isize + dj))
                .filter(|&(a, b)| free(taken, a, b))
                .map(|(a, b)| (a as usize, b as usize))
                .collect();
            push_item(furniture, "table", i, j);
            // A full table keeps its 3 × 3 to itself (the next table's chairs don't crowd it).
            let ring = if need == 4 { 1 } else { 0 };
            for b in j.saturating_sub(ring)..(j + ring + 1).min(ny) {
                for a in i.saturating_sub(ring)..(i + ring + 1).min(nx) {
                    if cells[b * nx + a] == ri {
                        taken[b * nx + a] = true;
                    }
                }
            }
            for (a, b) in chairs {
                push_item(furniture, "chair", a, b);
                taken[b * nx + a] = true;
            }
            taken[j * nx + i] = true;
            return true;
        }
    }
    false
}

/// A booth in a room corner: the table on the inner square, bench seats on the three squares
/// along the two walls.
#[allow(clippy::too_many_arguments)]
fn booth(cells: &[i16], nx: usize, ny: usize, ri: i16, taken: &mut [bool], blocked: &mut [bool], furniture: &mut Vec<Item>, rng: &mut Pcg32, passable: impl Fn(&[bool]) -> bool) -> bool {
    let out = |i: isize, j: isize| i < 0 || j < 0 || i >= nx as isize || j >= ny as isize || cells[j as usize * nx + i as usize] != ri;
    let mut cands: Vec<(usize, usize, usize, usize)> = Vec::new();
    for j in 0..ny.saturating_sub(1) {
        for i in 0..nx.saturating_sub(1) {
            let sq = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)];
            if !sq.iter().all(|&(a, b)| cells[b * nx + a] == ri && !taken[b * nx + a]) {
                continue;
            }
            let (x, y) = (i as isize, j as isize);
            let left = out(x - 1, y) && out(x - 1, y + 1);
            let right = out(x + 2, y) && out(x + 2, y + 1);
            let top = out(x, y - 1) && out(x + 1, y - 1);
            let bottom = out(x, y + 2) && out(x + 1, y + 2);
            // The table sits on the square away from both walls.
            let tx = if left { i + 1 } else if right { i } else { continue };
            let ty = if top { j + 1 } else if bottom { j } else { continue };
            cands.push((i, j, tx, ty));
        }
    }
    for k in (1..cands.len()).rev() {
        cands.swap(k, rng.below(k as u32 + 1) as usize);
    }
    for (i, j, tx, ty) in cands {
        blocked[ty * nx + tx] = true;
        if !passable(blocked) {
            blocked[ty * nx + tx] = false;
            continue;
        }
        for (a, b) in [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)] {
            push_item(furniture, if (a, b) == (tx, ty) { "booth_table" } else { "booth_seat" }, a, b);
            taken[b * nx + a] = true;
        }
        return true;
    }
    false
}

/// The squares inside a room next to its doorways and stairs (its entry points).
fn entry_squares(lvl: &Level, nx: usize, ny: usize, st: [usize; 4], ri: i16) -> Vec<usize> {
    let mut out = Vec::new();
    for d in &lvl.doors {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let sq: [(isize, isize); 2] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        for (i, j) in sq {
            if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && lvl.cells[j as usize * nx + i as usize] == ri {
                out.push(j as usize * nx + i as usize);
            }
        }
    }
    for j in st[1]..st[1] + st[3] {
        for i in st[0]..st[0] + st[2] {
            if lvl.cells[j * nx + i] == ri {
                out.push(j * nx + i);
            }
        }
    }
    out
}

/// Whether all of a room's entry squares connect over squares not blocked by furniture.
fn room_connected(lvl: &Level, nx: usize, ny: usize, st: [usize; 4], ri: i16, blocked: &[bool]) -> bool {
    let entries = entry_squares(lvl, nx, ny, st, ri);
    let Some(&start) = entries.first() else { return true };
    let mut seen = vec![false; nx * ny];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(k) = stack.pop() {
        let (i, j) = ((k % nx) as isize, (k / nx) as isize);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (a, b) = (i + di, j + dj);
            if a < 0 || b < 0 || a >= nx as isize || b >= ny as isize {
                continue;
            }
            let q = b as usize * nx + a as usize;
            if !seen[q] && lvl.cells[q] == ri && !blocked[q] {
                seen[q] = true;
                stack.push(q);
            }
        }
    }
    entries.iter().all(|&e| seen[e])
}

/// World position (ft) of a grid point.
pub fn to_world(it: &Interior, x: f64, y: f64) -> P {
    add(it.origin, add(mul(it.axis, x * SQUARE_FT), mul(it.across, y * SQUARE_FT)))
}

/// Grid coordinates of a world position.
pub fn to_grid(it: &Interior, p: P) -> [f64; 2] {
    let d = sub(p, it.origin);
    [dot(d, it.axis) / SQUARE_FT, dot(d, it.across) / SQUARE_FT]
}

/// Interior by id: `b:<settlement>:<building>` or `t:<settlement>:<tower>` (wall towers,
/// then gate towers, of that layout).
pub fn generate_id(world: &World, t0: &T0, id: &str) -> Option<Interior> {
    let mut parts = id.split(':');
    let kind = parts.next()?;
    let s: usize = parts.next()?.parse().ok()?;
    let k: usize = parts.next()?.parse().ok()?;
    match kind {
        "b" => generate(world, t0, s, k),
        "t" => tower(world, t0, s, k),
        // (As designed by hand, if it was: `under::design`.)
        "u" => match world.file.edits.designs.get(id) {
            Some(d) => crate::under::design::site(world, t0, id, d),
            None => crate::under::generate(world, t0, s, k),
        },
        // A keep's deep dungeons: `k:<layout>:<building>`.
        "k" => crate::under::keep_dungeon(world, t0, s, k),
        // Sewer sections: `w:<layout>:<sx>:<sy>`.
        "w" => crate::under::sewer_site(world, t0, s, k as i64, parts.next()?.parse().ok()?),
        _ => None,
    }
}

pub fn interior_json(world: &World, t0: &T0, id: &str) -> String {
    serde_json::to_string(&generate_id(world, t0, id)).expect("serializable")
}

/// Height of the wall walk and of a tower's top above the ground (as on the battlemap).
const WALL_WALK_FT: f32 = 20.0;
const TOWER_TOP_FT: f32 = 30.0;

/// Wall and gate towers of a layout: (centre, radius ft, gate tower).
pub fn towers(l: &Layout) -> Vec<(P, f64, bool)> {
    l.towers.iter().map(|&t| (t, 11.0, false)).chain(l.gate_towers.iter().map(|&t| (t, 15.0, true))).collect()
}

/// A wall or gate tower: a guardroom at the foot (its door toward the town, or the castle yard
/// for a curtain tower), a room at wall-walk height opening onto the wall walk each way the
/// wall runs (a gate tower's holds the portcullis winch), and battlements on top.
pub fn tower(world: &World, t0: &T0, settlement: usize, k: usize) -> Option<Interior> {
    if settlement >= town::layout_count(t0) {
        return None;
    }
    let l = town::layout(world, t0, settlement);
    let (at, r, gate) = *towers(&l).get(k)?;
    let mut rng = Pcg32::new(hash3(world.stream("interior.tower"), settlement as i64, k as i64, 0x70), 73);
    // The ways the wall runs from here.
    let mut dirs: Vec<P> = Vec::new();
    for w in &l.walls {
        for s in w.windows(2) {
            if geom::seg_dist(at, s[0], s[1]) > r + 2.0 {
                continue;
            }
            for end in [s[0], s[1]] {
                let d = sub(end, at);
                let len = geom::len(d);
                if len > r + 3.0 {
                    let u = mul(d, 1.0 / len);
                    if !dirs.iter().any(|q| dot(*q, u) > 0.9) {
                        dirs.push(u);
                    }
                }
            }
        }
    }
    let u = dirs.first().copied().unwrap_or([1.0, 0.0]);
    let v = [-u[1], u[0]];
    // A square grid centred on the tower (its centre in the middle of a square), reaching three
    // squares of wall walk beyond the tower each side.
    let half = (crate::core::ceil(r / SQUARE_FT) as usize) + 3;
    let n = 2 * half + 1;
    let origin = sub(at, mul(add(u, v), (half as f64 + 0.5) * SQUARE_FT));
    let centre = |i: usize, j: usize| add(origin, add(mul(u, (i as f64 + 0.5) * SQUARE_FT), mul(v, (j as f64 + 0.5) * SQUARE_FT)));
    let in_tower: Vec<bool> = (0..n * n).map(|q| geom::dist(centre(q % n, q / n), at) <= r + 1.5).collect();
    // Wall-walk squares: a 4-connected run of squares along each way the wall goes, traced
    // from the tower's centre (so it always meets the tower along an edge), outside the tower.
    let mut walk_of: Vec<i16> = vec![-1; n * n];
    let to_sq = |p: P| {
        let d = sub(p, origin);
        (crate::core::floor(dot(d, u) / SQUARE_FT) as isize, crate::core::floor(dot(d, v) / SQUARE_FT) as isize)
    };
    for (wi, w) in dirs.iter().enumerate() {
        let mut prev = to_sq(at);
        let mut t = 0.0;
        while t < r + 3.2 * SQUARE_FT {
            let sq = to_sq(add(at, mul(*w, t)));
            if sq != prev {
                // A diagonal step goes through a side square first.
                let steps = if sq.0 != prev.0 && sq.1 != prev.1 { vec![(sq.0, prev.1), sq] } else { vec![sq] };
                for (i, j) in steps {
                    if i < 0 || j < 0 || i >= n as isize || j >= n as isize {
                        continue;
                    }
                    let q = j as usize * n + i as usize;
                    if !in_tower[q] && walk_of[q] < 0 {
                        walk_of[q] = wi as i16;
                    }
                }
                prev = sq;
            }
            t += 1.0;
        }
    }
    let stairs = place_stairs(&in_tower, n, n, 0, Arch::Tower);
    // The door faces in: the castle yard for a curtain tower, else the town.
    let inward = l.castles.iter().find(|(c, rad)| geom::dist(*c, at) < rad * 1.3).map_or(l.center, |c| c.0);
    let di = sub(inward, at);
    let (gx, gy) = (dot(di, u), dot(di, v));
    let front = if gx.abs() > gy.abs() { if gx > 0.0 { 1 } else { 3 } } else if gy > 0.0 { 2 } else { 0 };
    let lattice = world.geom.spacing_ft(world.geom.first_refine_level.saturating_sub(1));
    let pad = t0.ground_at(at[0], at[1], lattice) as f32;
    // The door opens onto open ground, never into a building built against the tower.
    let near: Vec<&Vec<P>> = l.buildings.iter().filter(|o| o.structure == town::Structure::Roofed && o.poly.iter().any(|p| geom::dist(*p, at) < r + 120.0)).map(|o| &o.poly).collect();
    let ext_free = |i: isize, j: isize| {
        let p = add(origin, add(mul(u, (i as f64 + 0.5) * SQUARE_FT), mul(v, (j as f64 + 0.5) * SQUARE_FT)));
        !near.iter().any(|poly| geom::contains(poly, p))
    };

    let mut levels = Vec::new();
    for (z, name, elev) in [(0i8, "Guardroom", 0.0f32), (1, "Wall walk", WALL_WALK_FT), (2, "Battlements", TOWER_TOP_FT)] {
        let mut cells = vec![-1i16; n * n];
        let main: &'static str = match z {
            0 => "guardroom",
            1 if gate => "winch room",
            1 => "tower room",
            _ => "battlements",
        };
        let mut rooms = vec![Room { kind: main, squares: 0, raise_ft: 0.0, center: [0.0; 2] }];
        for q in 0..n * n {
            if in_tower[q] {
                cells[q] = 0;
            }
        }
        if z == 1 {
            for wi in 0..dirs.len() {
                let id = rooms.len() as i16;
                if walk_of.iter().any(|&w| w == wi as i16) {
                    rooms.push(Room { kind: "wall walk", squares: 0, raise_ft: 0.0, center: [0.0; 2] });
                    for q in 0..n * n {
                        if walk_of[q] == wi as i16 {
                            cells[q] = id;
                        }
                    }
                }
            }
        }
        finish_rooms(&mut rooms, &cells, n);
        let mut lvl = connect(cells, rooms, n, n, stairs, front, z == 0, None, &ext_free, &mut rng);
        windows(&mut lvl, n, n, z == 0);
        lvl.z = z;
        lvl.name = name.to_string();
        lvl.elevation_ft = pad + elev;
        if z >= 1 {
            // Arrow slits below; open to the sky on the walk and the top.
            lvl.windows.clear();
        }
        if z == 2 {
            for w in &mut lvl.walls {
                w.exterior = true;
            }
            lvl.roof = true;
        }
        furnish(&mut lvl, n, n, stairs, &mut rng);
        levels.push(lvl);
    }
    Some(Interior {
        id: format!("t:{}:{}", l.index, k),
        settlement: settlement as u32,
        building: k as u32,
        name: None,
        theme: None,
        function: if gate { "Gate tower" } else { "Wall tower" },
        origin,
        axis: u,
        across: v,
        nx: n,
        ny: n,
        levels,
        entry_level: 0,
        stairs,
    })
}
