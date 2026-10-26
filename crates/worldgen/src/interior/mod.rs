//! Building interiors, generated on demand: a pure function of (world, settlement layout,
//! building). The footprint becomes a 5-ft grid along the building's long axis (walls stay
//! straight whatever the building's rotation); each level (cellar, ground, upper floors) is
//! split into rooms from the building's room program, rooms are joined by doors (a spanning
//! tree from the entrance plus a few loops), one stair block links every level at the same
//! spot, and rooms are furnished by rule with 5e cover values (a great hall as one composition about
//! its axis: `hall`; thrones only in castles and palaces). A castle's keep and a big barracks
//! are laid out by zones instead (`hub`): a great hall with its screens passage and dais end,
//! chambers off passages, towers, each piece as high as it rises; so are palaces, noble estates,
//! temples (a cross with its apse, or a hall ending in one) and monasteries round their cloisters.
//!
//! Guarantees (checked in tests/vital.rs): at least two levels, every room reachable from the
//! front door, stairs aligned across levels, furniture never on a doorway or the stairs.

use serde::Serialize;

pub mod design;
pub mod hall;
pub mod hub;
pub mod shape;

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
    /// Uploaded pictures standing on its floors (`Item::sprite` is index + 1).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sprites: Vec<InteriorSprite>,
    /// A building whose footprint is not the grid's rectangle (a round tower, a lot cut on the
    /// slant): its outer wall as drawn, the wall's middle on the footprint (an edge pushed out
    /// where a square inside it would come nearer than `WALL_HALF` + 0.1), as convex parts whose
    /// union is that (grid units, wound clockwise on screen), within half a square of the squares
    /// inside it (`Shell::outlines`). Empty for the rest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outlines: Vec<Vec<[f32; 2]>>,
}

/// An uploaded picture used as an item (`Edits::sprites`): its asset id and name.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct InteriorSprite {
    pub asset: String,
    pub name: String,
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
    /// Drawn round, octagonal, with an apse, chamfered or as a cavern (`shape`); 0 square. Only
    /// while its squares fit the shape (`shape::fits`).
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub shape: u8,
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct Wall {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub exterior: bool,
    /// A railing, not a wall: along a hall's height left open (`OPEN_BELOW`) where a gallery, a
    /// passage or a landing looks down into it. It stops a step but not the eye.
    #[serde(skip_serializing_if = "is_false")]
    pub rail: bool,
    /// An open arcade's street side (its outer wall along the line of its front door, `wall_runs`):
    /// arches between piers, to be drawn open, not a wall; no windows.
    #[serde(skip_serializing_if = "is_false")]
    pub arcade: bool,
}

#[derive(Serialize, Debug, Clone, Copy)]
pub struct Door {
    /// The door's square edge (grid units).
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// "front", "door", "back", "secret", "arch" (an open archway: no leaves, never shut),
    /// "portcullis" (an iron grate raised and lowered: shut, it stops a step but not the eye).
    pub kind: &'static str,
    /// Rooms on either side (-1 outside).
    pub rooms: [i16; 2],
    /// Locked (underground): 0 not, else the colour of the lock (1-7), opened by the key
    /// (`key` item) of the same colour (its `variant`).
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub lock: u8,
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
    /// An uploaded picture (kind `sprite`): index + 1 into `Interior::sprites`; 0 for the rest.
    #[serde(skip_serializing_if = "is_zero")]
    pub sprite: u16,
    /// The side its back is against, 0 north 1 east 2 south 3 west (set in a design); absent:
    /// found from the walls when drawn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back: Option<u8>,
    /// One of its looks, 0-3 (set in a design); absent: picked by position when drawn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<u8>,
}

fn is_zero(v: &u16) -> bool {
    *v == 0
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// A room kind that is the hall's height left open on the floor above it (a two-storey hall, a
/// shell keep's yard): no floor, nothing stands there, the eye goes down into the room below.
pub const OPEN_BELOW: &str = "open to below";
/// A room kind that is a roof's slopes inside a parapet walk (a keep's top roofed over).
pub const ROOF_SLOPE: &str = "roof";

/// Rooms that are ways about a floor (passages, landings, an open yard, a nave, a palace's court and
/// long gallery) rather than rooms of their own: every room beside one opens onto it (`connect`).
pub fn is_circulation(kind: &str) -> bool {
    matches!(
        kind,
        "hall"
            | "corridor"
            | "landing"
            | "screens passage"
            | "mural passage"
            | "entrance hall"
            | "cloister walk"
            | "keep yard"
            | "nave"
            | "minstrels' gallery"
            | "gate passage"
            | "courtyard"
            | "long gallery"
            | "stair hall"
            | "arcade"
            | "inn yard"
            | "yard gallery"
    )
}

/// Circulation, and the rooms people pass through on their way (a great or mess hall, a
/// guardroom, a wall walk or open battlements on a roof): what the corridor rule counts from (no
/// room more than two rooms from one).
pub fn is_way_through(kind: &str) -> bool {
    is_circulation(kind) || matches!(kind, "great hall" | "mess hall" | "guardroom" | "battlements" | "wall walk")
}

/// Rooms parted by a railing rather than a wall: a hall's height left open from a way about the
/// floor (`OPEN_BELOW`), a temple's sanctuary in its apse (drawn so) and a side chapel from its nave
/// (an altar rail), a cloister's garth from its walk (its arcade). It stops a step, not the eye.
pub fn railed(a: &Room, b: &Room) -> bool {
    let one = |a: &Room, b: &Room| {
        (a.kind == OPEN_BELOW && is_circulation(b.kind))
            || (b.kind == "nave" && (a.kind == "side chapel" || (a.kind == "sanctuary" && matches!(a.shape, shape::APSE_N..=shape::APSE_W))))
            || (a.kind == "garth" && b.kind == "cloister walk")
    };
    one(a, b) || one(b, a)
}

/// A room one can stand in: all but a hall's height left open and a roof's slopes.
pub fn walkable(kind: &str) -> bool {
    kind != OPEN_BELOW && kind != ROOF_SLOPE
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

/// A round footprint: convex, with many sides (the POI towers are 14-gons), its corners all about
/// as far from its middle (within 15%).
fn round_ish(b: &town::Building) -> bool {
    let p = &b.poly;
    let n = p.len();
    if n < 8 {
        return false;
    }
    let turns: Vec<f64> = (0..n).map(|k| geom::cross(sub(p[(k + 1) % n], p[k]), sub(p[(k + 2) % n], p[(k + 1) % n]))).collect();
    if !(turns.iter().all(|&t| t >= 0.0) || turns.iter().all(|&t| t <= 0.0)) {
        return false;
    }
    // (Measured about the corners' mean, from the first corner: world feet are millions, and an
    // area centroid's sums lose the footprint to rounding.)
    let rel: Vec<P> = p.iter().map(|q| sub(*q, p[0])).collect();
    let c = mul(rel.iter().fold([0.0, 0.0], |a, q| add(a, *q)), 1.0 / n as f64);
    let (lo, hi) = rel.iter().map(|q| geom::dist(*q, c)).fold((f64::MAX, 0.0f64), |(a, b), r| (a.min(r), b.max(r)));
    hi > 0.0 && (hi - lo) <= 0.15 * hi
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
        (Warehouse, 0) if key == Some("stables") => p("Stables", &[("stables", 6.0), ("tack room", 1.0)], None, false),
        (Warehouse, 0) => p("Floor", &[("warehouse floor", 6.0), ("office", 1.0)], None, false),
        (Warehouse, _) => p("Loft", &[("loft storage", 4.0), ("office", 1.0)], Some("loft storage"), false),
        (Temple, 0) if key == Some("shrine") => p("Shrine", &[("shrine", 1.0)], None, false),
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
        // (A keep built of pieces: its towers are pieces of its own, not the grid's corners.)
        Arch::Keep if !b.parts.is_empty() || !corner_towers(b) => Some(0.0),
        Arch::Keep => {
            let o = geom::obb(&b.poly);
            let (nx, ny) = (crate::core::round(o.long / SQUARE_FT) as usize, crate::core::round(o.short / SQUARE_FT) as usize);
            Some(keep_tower_squares(nx, ny) as f64 * SQUARE_FT)
        }
        Arch::Tower => Some(0.0),
        _ => None,
    }
}

/// A keep with no pieces of its own has a tower on each corner of its grid, but a castle's keep
/// drawn round its towers by hand (its pieces gone: a footprint far from its grid's rectangle),
/// whose corners those don't fit.
fn corner_towers(b: &town::Building) -> bool {
    let castle = b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == "castle");
    let o = geom::obb(&b.poly);
    !castle || geom::area(&b.poly).abs() >= 0.95 * o.long * o.short
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

/// Furniture: (kind, display name, cover, blocks movement, height ft).
pub const FURNITURE: &[(&str, &str, u8, bool, f32)] = &[
    ("bed", "bed", 1, false, 2.0),
    ("chest", "chest", 1, true, 2.5),
    ("wardrobe", "wardrobe", 3, true, 7.0),
    ("table", "table", 1, true, 3.0),
    ("long_table", "long table", 1, true, 3.0),
    ("desk", "desk", 1, true, 3.0),
    ("bench", "bench", 0, false, 1.5),
    ("pew", "pew", 1, false, 3.0),
    ("bar", "bar counter", 2, true, 4.0),
    ("counter", "counter", 2, true, 4.0),
    ("hearth", "hearth", 2, true, 5.0),
    ("oven", "oven", 2, true, 5.0),
    ("shelf", "shelves", 2, true, 7.0),
    ("bookcase", "bookcase", 3, true, 8.0),
    ("barrel", "barrel", 1, true, 3.5),
    ("keg_rack", "keg rack", 2, true, 5.0),
    ("crate", "crate stack", 2, true, 5.0),
    ("workbench", "workbench", 1, true, 3.0),
    ("forge", "forge", 2, true, 5.0),
    ("anvil", "anvil", 1, true, 3.0),
    ("weapon_rack", "weapon rack", 1, true, 6.0),
    ("altar", "altar", 2, true, 4.0),
    ("statue", "statue", 3, true, 9.0),
    ("sarcophagus", "sarcophagus", 2, true, 3.5),
    ("cage", "iron cage", 1, true, 7.0),
    ("throne", "throne", 1, true, 5.0),
    ("rug", "rug", 0, false, 0.0),
    ("couch", "couch", 1, true, 3.0),
    ("cauldron", "cauldron", 1, true, 3.0),
    ("alchemy_bench", "alchemy bench", 1, true, 3.5),
    ("telescope", "telescope", 1, true, 6.0),
    ("bell", "bell", 3, true, 6.0),
    ("bath", "bath", 1, true, 2.0),
    ("cot", "cot", 0, false, 1.5),
    ("pillar", "pillar", 3, true, 10.0),
    ("barricade", "barricade", 2, true, 4.0),
    ("display", "display table", 1, true, 3.0),
    ("vat", "brewing vat", 2, true, 6.0),
    ("chair", "chair", 0, false, 3.0),
    ("spiral_stair", "spiral stairs", 0, false, 0.0),
    ("rack", "rack", 1, true, 3.0),
    ("bucket", "slop bucket", 0, false, 1.0),
    ("booth_table", "booth table", 1, true, 3.0),
    ("booth_seat", "booth seat", 1, false, 3.5),
    ("trapdoor", "trapdoor (to the undercroft)", 0, false, 0.0),
    ("winch", "portcullis winch", 1, true, 4.0),
    ("sideboard", "sideboard", 1, true, 3.5),
    ("stage", "stage", 0, false, 2.0),
    // Great halls and ballrooms (`hall::furnish_hall`). The dais's steps are climbed, so it is
    // difficult ground like the underground dais; things hung on a wall or laid on the floor are
    // walked over; a standing banner is gone round but seen past.
    ("throne_dais", "throne dais", 1, false, 2.5),
    ("carpet_runner", "carpet runner", 0, false, 0.0),
    ("chair_of_office", "chair of office", 1, true, 5.0),
    ("high_table", "high table", 1, true, 3.0),
    ("tapestry", "tapestry", 0, false, 0.0),
    ("wall_banner", "wall banner", 0, false, 0.0),
    ("standing_banner", "standing banner", 0, true, 5.5),
    ("font", "font", 1, true, 3.5),
    ("cloister_well", "cloister well", 1, true, 3.5),
    ("dance_floor", "inlaid dance floor", 0, false, 0.0),
    ("harpsichord", "harpsichord", 1, true, 3.0),
    ("musicians_corner", "musicians' corner", 0, false, 3.0),
    ("refreshment_table", "refreshment table", 1, true, 3.0),
    // A palace court's gravel walks (laid on its lawn: walked over).
    ("garden_path", "gravel walk", 0, false, 0.0),
    // A low dais others stand on (`hall::is_platform`: the one piece another may overlap), climbed.
    ("low_dais", "low dais", 0, false, 1.0),
    // A temple's (`hall::Style::Nave`), each drawn for its god (`GOD_LOOKS`): the god's statue over
    // life size on its plinth, a tall stand of candles (gone round, seen past), the carved screen
    // across the nave (a wall to stand behind) and its gate (open: walked through as the floor is,
    // its sill flush, its beam overhead), a side chapel's altar.
    ("god_statue", "statue of the god", 3, true, 10.0),
    ("candle_stand", "candle stand", 0, true, 5.5),
    ("carved_screen", "carved screen", 2, true, 8.0),
    ("screen_gate", "screen gate", 0, false, 0.0),
    ("side_altar", "side altar", 2, true, 3.5),
    // A town hall's and a guild's (`hub::civic`): a strongbox, a counting desk with its ledgers, a
    // council table with its seats round it (the whole footprint blocks), hung on the wall a job board
    // and a guild's arms (`GUILD_ARMS`); a stable stall, walked into (its partitions half cover).
    ("strongbox", "strongbox", 1, true, 2.0),
    ("counting_desk", "counting desk", 1, true, 3.0),
    ("job_board", "job board", 0, false, 0.0),
    ("stable_stall", "stable stall", 1, false, 4.5),
    ("guild_arms", "guild's arms", 0, false, 0.0),
    ("council_table", "council table", 1, true, 3.0),
];

/// A guild's arms' looks (`Item::variant` of `guild_arms`): the merchants', the craftsmen's, the
/// adventurers', the mages', the town's (a town hall's, a courthouse's).
pub const GUILD_ARMS: u8 = 5;

/// The arms a building of function `key` hangs (`GUILD_ARMS`): None for one that shows none (a
/// thieves' guild's).
pub fn arms_look(key: Option<&str>) -> Option<u8> {
    match key? {
        "merchant_guild" | "auction_house" => Some(0),
        "craft_guild" => Some(1),
        "adventurers_guild" => Some(2),
        "mages_guild" => Some(3),
        "thieves_guild" => None,
        _ => Some(4),
    }
}

/// The house colours a hall's banners fly (`Item::variant` of a wall or standing banner: the same
/// index on both, so a castle's banners can share one house).
pub const HOUSES: u8 = 8;

/// The looks of a temple's pieces (the god's statue, candle stands, the carved screen and its gate,
/// a side altar): one per kind of god (`town::temple::god_look`, `town::temple::GOD_KINDS`).
pub const GOD_LOOKS: u8 = 8;

/// Furniture with other than four looks (`Item::variant` 0 to looks - 1): the throne dais's are
/// three seatings (one throne, a pair, a throne and a lesser chair) in stone and gilt (0-2), then
/// on a timber platform (3-5); a banner's are its house's colours; a low dais is of stone (0) or
/// timber (1); a temple's pieces are its god's (`GOD_LOOKS`).
const LOOKS: &[(&str, u8)] = &[
    ("throne_dais", 6),
    // (A key's look is the colour of the lock it opens, 1-7.)
    ("key", 8),
    ("wall_banner", HOUSES),
    ("standing_banner", HOUSES),
    ("low_dais", 2),
    ("god_statue", GOD_LOOKS),
    ("candle_stand", GOD_LOOKS),
    ("carved_screen", GOD_LOOKS),
    ("screen_gate", GOD_LOOKS),
    ("side_altar", GOD_LOOKS),
    ("guild_arms", GUILD_ARMS),
];

/// How many looks an item kind has (`Item::variant`): four but for `LOOKS`.
pub fn item_looks(kind: &str) -> u8 {
    LOOKS.iter().find(|l| l.0 == kind).map_or(4, |l| l.1)
}

/// (kind, display name, cover, blocks movement, height ft).
fn item_info(kind: &'static str) -> (&'static str, u8, bool, f32) {
    FURNITURE.iter().find(|f| f.0 == kind).map(|f| (f.1, f.2, f.3, f.4)).unwrap_or(("furniture", 1, true, 3.0))
}

/// Clutter a building's design can hold besides its furniture (the designer's indoor props):
/// kinds of `under::PROPS`, with their rules, drawn as they are underground.
pub const INDOOR_PROPS: &[&str] = &[
    "sacks", "rubble", "timber", "bones", "skeleton", "skulls", "urn", "coffin", "effigy", "brazier", "candles", "sconce",
    "lantern", "banner", "chains", "cobweb", "bedroll", "tools", "powder", "hoard", "offering", "glyph", "dais", "well",
    "fountain", "iron_maiden", "stocks", "rat_nest", "trap", "collapsed_wall",
];

/// An item a building's design puts down, by kind or name: furniture (`FURNITURE`), else an
/// indoor prop (`INDOOR_PROPS`, with its hazard). Not uploaded pictures (`Sprites`).
pub fn building_item(kind: &str, x: usize, y: usize, w: usize, h: usize) -> Option<Item> {
    let at = |kind, name, cover, blocks_move, height_ft, hazard| Item { kind, name, x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover, blocks_move, height_ft, hazard, sprite: 0, back: None, variant: None };
    if let Some(f) = FURNITURE.iter().find(|f| f.0 == kind || f.1 == kind) {
        return Some(at(f.0, f.1, f.2, f.3, f.4, None));
    }
    let p = crate::under::PROPS.iter().find(|p| (p.0 == kind || p.1 == kind) && INDOOR_PROPS.contains(&p.0))?;
    Some(at(p.0, p.1, p.2, p.3, p.4, p.5))
}

/// Item kind `sprite`: an uploaded picture (`Item::sprite`).
pub const SPRITE: &str = "sprite";

/// The asset id of a design's item kind that names an uploaded picture (`s:<asset id>`).
pub fn sprite_asset(kind: &str) -> Option<&str> {
    kind.strip_prefix("s:").filter(|a| !a.is_empty() && a.len() <= 128)
}

/// Uploaded pictures put down as items while a site is built: each picture's rules
/// (`Edits::sprites`) and its place in `Interior::sprites`.
pub struct Sprites<'a> {
    metas: &'a std::collections::BTreeMap<String, crate::world::SpriteMeta>,
    used: Vec<String>,
}

impl<'a> Sprites<'a> {
    pub fn new(metas: &'a std::collections::BTreeMap<String, crate::world::SpriteMeta>) -> Self {
        Sprites { metas, used: Vec::new() }
    }

    /// Picture `asset` as an item. Its rules in the terms play mode reads items by
    /// (`play/vision.ts`): what blocks movement and stands 6 ft or more blocks sight; what
    /// doesn't block movement but stands 1 ft or more is difficult ground. So the height kept is
    /// moved across those lines as the picture's own rules say (a picture that blocks sight
    /// but not movement doesn't block sight indoors).
    pub fn item(&mut self, asset: &str, x: usize, y: usize, w: usize, h: usize) -> Item {
        let k = match self.used.iter().position(|a| a == asset) {
            Some(k) => k,
            None => {
                self.used.push(asset.to_string());
                self.used.len() - 1
            }
        };
        let m = self.metas.get(asset).cloned().unwrap_or_default();
        let h0 = if m.height_ft.is_finite() { m.height_ft.clamp(0.0, 100.0) } else { 3.0 };
        let height_ft = match (m.blocks_move, m.blocks_sight, m.difficult) {
            (true, true, _) => h0.max(6.0),
            (true, false, _) => h0.min(5.5),
            (false, _, true) => h0.max(1.0),
            (false, _, false) => h0.min(0.5),
        };
        Item { kind: SPRITE, name: "custom object", x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover: m.cover.min(3), blocks_move: m.blocks_move, height_ft, hazard: None, sprite: k as u16 + 1, back: None, variant: None }
    }

    /// `Interior::sprites`: the pictures used, in the order first put down.
    pub fn table(&self) -> Vec<InteriorSprite> {
        self.used
            .iter()
            .map(|a| {
                let name = self.metas.get(a).map(|m| m.name.trim()).filter(|n| !n.is_empty()).unwrap_or("custom object");
                InteriorSprite { asset: a.clone(), name: name.to_string() }
            })
            .collect()
    }
}

/// A design's item kind for item `f` of an interior: an uploaded picture as `s:<asset id>`.
pub fn design_kind(it: &Interior, f: &Item) -> String {
    match it.sprites.get((f.sprite as usize).wrapping_sub(1)) {
        Some(s) if f.sprite > 0 => format!("s:{}", s.asset),
        _ => f.kind.to_string(),
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
        "oratory" => &[("altar", Wall(2, 1), 1, 0), ("statue", Wall(1, 1), 1, 0)],
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
        "vault" => &[("chest", Wall(1, 1), 3, 4), ("strongbox", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "counting house" => &[("counter", Wall(3, 1), 1, 0), ("counting_desk", Center(2, 1), 1, 12), ("strongbox", Wall(1, 1), 1, 0), ("bench", Wall(2, 1), 0, 16)],
        "courtroom" => &[("counter", Wall(3, 1), 1, 0), ("desk", Center(2, 1), 1, 12), ("bench", Center(3, 1), 0, 12)],
        "baths" => &[("bath", Center(2, 2), 1, 12), ("bench", Wall(2, 1), 1, 0)],
        "infirmary" => &[("cot", Wall(1, 2), 2, 6), ("table", Center(2, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "arena floor" => &[("pillar", Center(1, 1), 2, 40), ("barricade", Center(2, 1), 2, 30), ("trapdoor", Center(1, 1), 1, 30), ("cage", Center(1, 1), 1, 0)],
        "stands" => &[("bench", Wall(3, 1), 2, 6)],
        "tower room" => &[("weapon_rack", Wall(1, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "winch room" => &[("winch", Center(1, 1), 1, 0), ("weapon_rack", Wall(1, 1), 1, 0)],
        "wall walk" => &[],
        "hall" | "corridor" => &[],
        "screens passage" | "mural passage" | "landing" | "gate passage" | "open to below" | "roof" => &[],
        "entrance hall" => &[("bench", Wall(2, 1), 1, 0)],
        "minstrels' gallery" => &[("bench", Wall(2, 1), 1, 0)],
        "keep yard" => &[("barrel", Wall(1, 1), 2, 0), ("crate", Wall(1, 1), 1, 0)],
        "great chamber" => &[("bed", Wall(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("wardrobe", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        "garrison" => &[("cot", Wall(1, 2), 2, 5), ("weapon_rack", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 8)],
        "solar" => &[("couch", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("table", Center(1, 1), 1, 0)],
        "map room" => &[("long_table", Center(3, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0)],
        "treasury" => &[("chest", Wall(1, 1), 3, 4), ("strongbox", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
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
        // A palace's and an estate's (its throne room, banqueting hall and ballroom furnished as
        // wholes: `hall`; these where that fails).
        "throne room" => &[("throne", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("pillar", Wall(1, 1), 0, 16)],
        "banqueting hall" => &[("long_table", Center(4, 1), 1, 18), ("hearth", Wall(2, 1), 1, 0), ("sideboard", Wall(2, 1), 1, 0)],
        "ballroom" => &[("bench", Wall(2, 1), 1, 12), ("statue", Wall(1, 1), 1, 0)],
        "audience chamber" => &[("chair_of_office", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("tapestry", Wall(2, 1), 1, 12), ("bench", Wall(2, 1), 1, 12)],
        "long gallery" => &[("statue", Wall(1, 1), 1, 8), ("tapestry", Wall(2, 1), 0, 8), ("bench", Wall(2, 1), 0, 10)],
        "royal chapel" => &[("altar", Wall(2, 1), 1, 0), ("statue", Wall(1, 1), 1, 0), ("pew", Center(3, 1), 1, 8), ("font", Center(1, 1), 1, 0)],
        // A temple's and a monastery's (their naves, sanctuaries, side chapels, shrines and refectories
        // furnished as wholes: `hall`; these where that fails).
        "side chapel" => &[("side_altar", Wall(2, 1), 1, 0), ("candle_stand", Wall(1, 1), 1, 0)],
        "shrine" => &[("altar", Wall(2, 1), 1, 0), ("god_statue", Wall(1, 1), 1, 0), ("candle_stand", Wall(1, 1), 1, 0)],
        "refectory" => &[("long_table", Center(4, 1), 1, 18), ("bench", Wall(2, 1), 1, 8), ("hearth", Wall(2, 1), 1, 0)],
        "chapter house" => &[("bench", Wall(2, 1), 2, 6), ("chair_of_office", Wall(1, 1), 1, 0)],
        "dorter" => &[("cot", Wall(1, 2), 2, 5), ("chest", Wall(1, 1), 1, 8)],
        "scriptorium" => &[("desk", Center(2, 1), 1, 8), ("bookcase", Wall(2, 1), 1, 8), ("shelf", Wall(2, 1), 1, 0)],
        "cloister walk" | "garth" | "inn yard" | "yard gallery" => &[],
        // (A palace's court: its fountain, walks and benches laid out with it, `hub::Plan::court_pieces`.)
        "courtyard" => &[],
        "antechamber" => &[("bench", Wall(2, 1), 1, 0), ("tapestry", Wall(2, 1), 1, 0), ("chair", Wall(1, 1), 1, 0)],
        "servants' hall" => &[("long_table", Center(3, 1), 1, 0), ("bench", Wall(2, 1), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "salon" => &[("couch", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("harpsichord", Wall(2, 1), 1, 0)],
        "stair hall" => &[("bench", Wall(2, 1), 1, 0)],
        // A town hall's and a guild's (`hub::civic`): the guild's arms (`arms_look`) where it shows
        // itself; a belfry's stair and its bell loft (the bell put down with the tower: `hub::civic`).
        "arcade" => &[("crate", Wall(1, 1), 1, 14), ("barrel", Wall(1, 1), 1, 18)],
        "council chamber" => &[("council_table", Center(4, 2), 1, 0), ("long_table", Center(3, 1), 1, 0), ("chair_of_office", Wall(1, 1), 1, 0), ("guild_arms", Wall(2, 1), 1, 0), ("bookcase", Wall(2, 1), 1, 0)],
        "strongroom" | "warded vault" => &[("strongbox", Wall(1, 1), 2, 4), ("chest", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "sample room" => &[("display", Center(2, 1), 1, 10), ("shelf", Wall(2, 1), 1, 6), ("guild_arms", Wall(1, 1), 1, 0), ("crate", Wall(1, 1), 1, 0)],
        "masterpiece gallery" => &[("display", Center(2, 1), 1, 10), ("statue", Wall(1, 1), 1, 0), ("guild_arms", Wall(1, 1), 1, 0), ("bench", Wall(2, 1), 1, 0)],
        "job-board room" => &[("job_board", Wall(2, 1), 1, 0), ("table", Center(2, 1), 0, 12), ("bench", Wall(2, 1), 1, 8), ("guild_arms", Wall(1, 1), 1, 0)],
        "trophy hall" => &[("statue", Wall(1, 1), 2, 8), ("guild_arms", Wall(2, 1), 1, 0), ("weapon_rack", Wall(2, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        "belfry stair" | "belfry" => &[],
        // A stables' stalls in a row along its walls, their mangers to the wall.
        "stables" => &[("stable_stall", Wall(1, 2), 2, 4), ("barrel", Wall(1, 1), 1, 0)],
        "tack room" => &[("rack", Wall(2, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "bunk room" => &[("cot", Wall(1, 2), 2, 4), ("chest", Wall(1, 1), 1, 8)],
        // A big inn's (`hub::inn`): the snug a drinking room of its own, the private dining room's
        // table for a party, the scullery's tubs and buckets, the best chamber's big bed by its
        // hearth, the innkeeper's takings in a strongbox.
        "snug" => &[("table", Seated, 1, 0), ("hearth", Wall(2, 1), 1, 0), ("bench", Wall(2, 1), 1, 0)],
        "private dining room" => &[("long_table", Center(3, 1), 1, 0), ("sideboard", Wall(2, 1), 1, 0), ("hearth", Wall(2, 1), 1, 0)],
        "scullery" => &[("bucket", Wall(1, 1), 2, 0), ("barrel", Wall(1, 1), 1, 0), ("shelf", Wall(2, 1), 1, 0)],
        "best chamber" => &[("bed", Wall(2, 2), 1, 0), ("hearth", Wall(2, 1), 1, 0), ("wardrobe", Wall(1, 1), 1, 0), ("chest", Wall(1, 1), 1, 0), ("rug", Center(2, 2), 1, 0)],
        "innkeeper's quarters" => &[("bed", Wall(1, 2), 1, 0), ("desk", Wall(2, 1), 1, 0), ("strongbox", Wall(1, 1), 1, 0), ("wardrobe", Wall(1, 1), 1, 0)],
        _ => &[("chest", Wall(1, 1), 1, 0)],
    }
}

/// Every kind of room in a building (those with furnishing rules), for the designer.
pub const ROOM_KINDS: &[&str] = &[
    "common room", "taproom", "brewhouse", "battlements", "kitchen", "storeroom", "loft storage", "keg cellar",
    "wine cellar", "bedroom", "guest room", "owner's quarters", "chamber", "master bedroom", "lord's chamber",
    "bedchamber", "main room", "living room", "shop", "workshop", "forge", "warehouse floor", "office", "archive",
    "study", "meeting room", "library", "reading room", "laboratory", "observatory", "entry hall", "foyer", "nave",
    "sanctuary", "chapel", "vestry", "crypt", "ossuary", "bell loft", "monk's cell", "cell", "holding cell",
    "great hall", "mess hall", "dining room", "parlor", "gallery", "guardroom", "armory", "barrack room", "dormitory",
    "dungeon", "holding pen", "beast pen", "vault", "counting house", "courtroom", "baths", "infirmary",
    "arena floor", "stands", "tower room", "winch room", "wall walk", "hall", "corridor", "solar", "map room",
    "treasury", "guest chamber", "servants' quarters", "officers' quarters", "pantry", "buttery", "wardrobe",
    "guard post", "tower top", "torture chamber", "prison block", "well room", "warden's office",
    "screens passage", "mural passage", "landing", "entrance hall", "minstrels' gallery", "open to below",
    "roof", "keep yard", "gate passage", "great chamber", "garrison", "oratory", "throne room", "ballroom",
    "audience chamber", "long gallery", "banqueting hall", "royal chapel", "courtyard", "antechamber",
    "servants' hall", "salon", "stair hall", "side chapel", "shrine", "refectory", "chapter house", "dorter",
    "scriptorium", "cloister walk", "garth", "arcade", "council chamber", "strongroom", "sample room",
    "masterpiece gallery", "job-board room", "trophy hall", "bunk room", "warded vault", "belfry stair", "belfry",
    "stables", "tack room", "snug", "private dining room", "scullery", "best chamber", "innkeeper's quarters",
    "inn yard", "yard gallery",
];

/// A kind of room in a building, by name.
pub fn room_kind(s: &str) -> Option<&'static str> {
    ROOM_KINDS.iter().copied().find(|k| *k == s)
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
    // Its floors ride on ground shaped by hand, as its roof does on the battlemap.
    let shift = crate::ground::pad_shift(&world.file.edits, t0, &b.poly, b.pad_ft);
    let moved;
    let b = if shift != 0.0 {
        moved = town::Building { pad_ft: b.pad_ft + shift, ..b.clone() };
        &moved
    } else {
        b
    };
    // (As designed by hand, while the design fits the building: `design`.)
    if !world.file.edits.designs.is_empty()
        && let Some(d) = world.file.edits.designs.get(&format!("b:{}:{}", l.index, b.id))
        && let Some(it) = design::designed(world, t0, &l, settlement, b, d)
    {
        return Some(it);
    }
    Some(build(world, t0, &l, settlement, b))
}

/// What a building laid out by zones (`hub`) is: a castle's keep, a barracks of 150 squares or
/// more, a palace, a noble estate big enough, a temple, a monastery, a town hall or a guild big
/// enough (`hub::civic::LEAST`); None for the rest.
pub fn hub_kind(t0: &T0, l: &Layout, b: &town::Building) -> Option<hub::Kind> {
    if b.structure != Structure::Roofed || !zoned_arch(b) {
        return None;
    }
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
    let sh = Shell::of(l, b);
    hub::kind_of(b, key, &sh, capital_of(t0, l.index as usize), city_of(t0, l.index as usize))
}

/// Whether a building may be laid out by zones (`hub::kind_of`): a keep, a palace or an estate, a
/// temple, a town hall or a guild (not the thieves' guild), an inn or a caravanserai.
fn zoned_arch(b: &town::Building) -> bool {
    match arch_of(b) {
        Arch::Keep | Arch::Mansion | Arch::Temple | Arch::Inn => true,
        Arch::Hall => matches!(b.func.map(|f| town::catalog::CATALOG[f as usize].key), Some("town_hall" | "merchant_guild" | "craft_guild" | "adventurers_guild" | "mages_guild")),
        _ => false,
    }
}

/// Whether layout `settlement` is the capital's.
fn capital_of(t0: &T0, settlement: usize) -> bool {
    t0.settlements.get(settlement).is_some_and(|s| s.capital)
}

/// Whether layout `settlement` is a city's or a metropolis's.
fn city_of(t0: &T0, settlement: usize) -> bool {
    t0.settlements.get(settlement).is_some_and(|s| s.tier >= crate::t0::settle::Tier::City)
}

/// The floors (z) of a building laid out by zones that the zones didn't hold together on, laid out
/// as other buildings' are instead (every floor of it if it has no plan at all): for the checks,
/// none should be. As generated (no design, no ground shaped by hand).
pub fn hub_fallbacks(world: &World, t0: &T0, settlement: usize, building: usize) -> Vec<i8> {
    let l = town::layout(world, t0, settlement);
    let Some(b) = l.building(building).filter(|b| hub_kind(t0, &l, b).is_some()) else { return Vec::new() };
    let mut fell = Vec::new();
    build_noting(world, t0, &l, settlement, b, &mut fell, &mut Vec::new());
    fell
}

/// The court the capital's palace stands round (`hub::Plan::palace`), open to the sky (world ft, its
/// corners each by the footprint's corner in the same place in `Building::poly`), as the battlemap
/// and the map draw it; None for every other building.
pub fn court_of(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Vec<P>> {
    court_plan(world, t0, l, b).map(|c| c.court)
}

/// The capital's palace's court as the battlemap draws it (`court_of`) and what stands in it, as
/// its ground floor has it (`hub::Plan::court_pieces`).
pub struct Court {
    /// Its outline (world ft), each corner by the footprint's corner in the same place.
    pub court: Vec<P>,
    /// Each piece: its kind (the fountain, a gravel walk, a bench), its box's corners (world ft),
    /// the way its back faces (unit, world), if any.
    pub pieces: Vec<(&'static str, [P; 4], Option<P>)>,
    /// The walled forecourt before its front door, where the ground there allows one (`Forecourt`).
    pub forecourt: Option<Forecourt>,
    /// A monastery's church along its side (world ft): its corners from its west end's (the way in's)
    /// along its length, then back along its side by the walk; roofed higher than the ranges, a
    /// bell-cote on its west end.
    pub church: Option<[P; 4]>,
    /// Cobbled (a coaching inn's yard), not a lawn.
    pub paved: bool,
}

/// The capital's palace's forecourt (`town::palace::Forecourt`): paved ground before its front, a
/// low garden wall round it with a gate on the palace's axis between two piers (world ft).
pub struct Forecourt {
    pub yard: Vec<P>,
    /// The wall's centre lines, `FORE_WALL_FT` thick, along the middle of its squares at the edge:
    /// from the palace's front round to the piers.
    pub walls: Vec<(P, P)>,
    /// The gate's piers' middles, `FORE_PIER_FT` square, their sides along `along` (the front,
    /// unit).
    pub piers: Vec<P>,
    pub along: P,
    /// The gate: its opening's ends (the piers' inner faces, `town::palace::GATE_SQ` squares apart)
    /// and a point inside, where its leaves swing open to.
    pub gate: [P; 3],
}

/// A forecourt's wall's thickness and its gate's piers' side (ft).
pub const FORE_WALL_FT: f64 = 2.0;
pub const FORE_PIER_FT: f64 = 7.0;

/// A manor's outline cut across where its dais end begins (`hub::Plan::cross_wing`): its main
/// range's part and its cross wing's over the family end (world ft), as the map and the battlemap roof
/// them (the cross wing's ridge across the main range's: a cross gable). None for every other
/// building.
pub fn cross_wing_of(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<(Vec<P>, Vec<P>)> {
    // (Cheap tests first: a noble estate big enough for a manor, `hub::kind_of`.)
    if b.structure != Structure::Roofed || !b.parts.is_empty() || b.func.is_some() || b.keep.is_some() || town::catalog::RESIDENTIAL[b.residential as usize] != "noble estate" || geom::area(&b.poly).abs() < 120.0 * SQUARE_FT * SQUARE_FT {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (f, dz) = plan.cross_wing()?;
    // (The line between the rows dz - 1 and dz, the wing beyond it.)
    let (gi, gj) = f.grid(dz as isize, 0);
    let (cx, cy) = (gi as f64 + 0.5 - 0.5 * f.ds.0 as f64, gj as f64 + 0.5 - 0.5 * f.ds.1 as f64);
    let n = add(mul(sh.u, f.ds.0 as f64), mul(sh.v, f.ds.1 as f64));
    let c = dot(sh.world_at(cx, cy), n);
    let main = geom::clip_plain(&b.poly, n, c);
    let wing = geom::clip_plain(&b.poly, mul(n, -1.0), -c);
    (main.len() >= 3 && wing.len() >= 3).then_some((main, wing))
}

/// A city's town hall on its arcade (`hub::civic`): the arcade's open front (world ft) from the outer
/// corner of its first square along the front to its last's (the stair hall's corner left out), then
/// a square in; None for every other building.
pub fn arcade_front(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Vec<P>> {
    // (Cheap tests first: a city's town hall of two storeys or more, big enough.)
    if !town::belfry::is_town_hall(b) || b.structure != Structure::Roofed || b.floors < 2 || !city_of(t0, l.index as usize) || geom::area(&b.poly).abs() < 100.0 * SQUARE_FT * SQUARE_FT {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    if !plan.civic()?.0 {
        return None;
    }
    let f = plan.frame();
    let w = f.wid as isize;
    let corner = |t: isize, ds: f64, dt: f64| {
        let (i, j) = f.grid(0, t);
        sh.world_at(i as f64 + 0.5 + ds * f.ds.0 as f64 + dt * f.dt.0 as f64, j as f64 + 0.5 + ds * f.ds.1 as f64 + dt * f.dt.1 as f64)
    };
    Some(vec![corner(0, -0.5, -0.5), corner(w - 4, -0.5, 0.5), corner(w - 4, 0.5, 0.5), corner(0, 0.5, -0.5)])
}

/// A coaching inn's carriage arch's mouth on the street (`hub::inn`'s `Yard`), as a town hall's arcade
/// front is given (`arcade_front`: the outer corner of its first square along the front to its
/// last's, then a square in; the battlemap draws its piers either side); None for every other building.
pub fn arch_front(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Vec<P>> {
    if !town::palace::is_coaching(l, b) {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (_, (a0, a1)) = plan.yard()?;
    let f = plan.frame();
    let corner = |t: isize, ds: f64, dt: f64| {
        let (i, j) = f.grid(0, t);
        sh.world_at(i as f64 + 0.5 + ds * f.ds.0 as f64 + dt * f.dt.0 as f64, j as f64 + 0.5 + ds * f.ds.1 as f64 + dt * f.dt.1 as f64)
    };
    let (a0, a1) = (a0 as isize, a1 as isize - 1);
    Some(vec![corner(a0, -0.5, -0.5), corner(a1, -0.5, 0.5), corner(a1, 0.5, 0.5), corner(a0, 0.5, -0.5)])
}

/// `court_of` with the court's pieces (`Court`); a monastery's garth (`garth_plan`); a coaching inn's
/// yard (`yard_plan`); None for every

/// other building.
pub fn court_plan(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Court> {
    if town::palace::is_monastery(b) {
        return garth_plan(world, t0, l, b);
    }
    if town::palace::is_coaching(l, b) {
        return yard_plan(world, t0, l, b);
    }
    if !town::palace::is_palace(b) || !capital_of(t0, l.index as usize) || b.poly.len() != 4 || b.structure != Structure::Roofed || !b.parts.is_empty() {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (front, back, side, court) = plan.palace()?;
    if !court {
        return None;
    }
    let f = plan.frame();
    // (A box of the frame's squares (s0..s1, t0..t1) in the world.)
    let corners = |s0: usize, s1: usize, t0: usize, t1: usize| {
        let (a, c) = (f.grid(s0 as isize, t0 as isize), f.grid(s1 as isize - 1, t1 as isize - 1));
        let (x0, x1, y0, y1) = (a.0.min(c.0) as f64, a.0.max(c.0) as f64 + 1.0, a.1.min(c.1) as f64, a.1.max(c.1) as f64 + 1.0);
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|g| sh.world_at(g[0], g[1]))
    };
    let pts = corners(front, back, side, f.wid - side);
    let court = b.poly.iter().map(|q| *pts.iter().min_by(|p, r| geom::dist(**p, *q).total_cmp(&geom::dist(**r, *q))).expect("four corners")).collect();
    let pieces = plan
        .court_pieces()
        .into_iter()
        .map(|(kind, (s0, s1, t0, t1), back)| {
            let dir = back.map(|d| {
                let (gi, gj) = (f.ds.0 * d.0 + f.dt.0 * d.1, f.ds.1 * d.0 + f.dt.1 * d.1);
                add(mul(sh.u, gi as f64), mul(sh.v, gj as f64))
            });
            (kind, corners(s0, s1, t0, t1), dir)
        })
        .collect();
    // The forecourt laid out with the town (`town::palace::approach`): its wall along its outer
    // squares' middles, the piers either side of the gate outside its opening.
    let forecourt = l.forecourts.iter().find(|fc| fc.palace == b.id && fc.yard.len() == 4).map(|fc| {
        let w = geom::dist(fc.front[0], fc.front[1]) / SQUARE_FT;
        let along = mul(sub(fc.front[1], fc.front[0]), 1.0 / (w * SQUARE_FT).max(1e-9));
        let at = |o: f64, t: f64| add(fc.front[0], add(mul(along, t * SQUARE_FT), mul(fc.out, o * SQUARE_FT)));
        let (i, e, p) = (fc.inset, fc.depth - 0.5, 0.5 * FORE_PIER_FT / SQUARE_FT);
        let (g0, g1) = (0.5 * (w - town::palace::GATE_SQ), 0.5 * (w + town::palace::GATE_SQ));
        let walls = vec![(at(0.0, i + 0.5), at(e, i + 0.5)), (at(e, i + 0.5), at(e, g0 - 2.0 * p)), (at(e, g1 + 2.0 * p), at(e, w - i - 0.5)), (at(e, w - i - 0.5), at(0.0, w - i - 0.5))];
        Forecourt { yard: fc.yard.clone(), walls, piers: vec![at(e, g0 - p), at(e, g1 + p)], along, gate: [at(e, g0), at(e, g1), at(e - 1.0, 0.5 * w)] }
    });
    Some(Court { court, pieces, forecourt, church: None, paved: false })
}

/// A monastery's cloister garth as the battlemap and the map draw it (`Court`: open to the sky, its
/// corners each by the footprint's corner in the same place; its gravel walks and its cloister well as
/// its ground floor has them, `hub::Plan::garth_pieces`); None for a monastery not round a cloister.
fn garth_plan(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Court> {
    if b.structure != Structure::Roofed || hub_kind(t0, l, b) != Some(hub::Kind::Monastery) {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let (_, (s0, s1, g0, g1)) = plan.monastery()?;
    let f = plan.frame();
    let corners = |s0: usize, s1: usize, t0: usize, t1: usize| {
        let (a, c) = (f.grid(s0 as isize, t0 as isize), f.grid(s1 as isize - 1, t1 as isize - 1));
        let (x0, x1, y0, y1) = (a.0.min(c.0) as f64, a.0.max(c.0) as f64 + 1.0, a.1.min(c.1) as f64, a.1.max(c.1) as f64 + 1.0);
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|g| sh.world_at(g[0], g[1]))
    };
    let pts = corners(s0, s1, g0, g1);
    let court = b.poly.iter().map(|q| *pts.iter().min_by(|p, r| geom::dist(**p, *q).total_cmp(&geom::dist(**r, *q))).expect("four corners")).collect();
    let pieces = plan.garth_pieces().into_iter().map(|(kind, (s0, s1, t0, t1))| (kind, corners(s0, s1, t0, t1), None)).collect();
    // (The church: the frame's corner (s, t), grid units, to world ft.)
    let ((cw, ..), _) = plan.monastery()?;
    let (i, j) = f.grid(0, 0);
    let o = [i as f64 + 0.5 - 0.5 * (f.ds.0 + f.dt.0) as f64, j as f64 + 0.5 - 0.5 * (f.ds.1 + f.dt.1) as f64];
    let corner = |s: usize, t: usize| sh.world_at(o[0] + (f.ds.0 * s as isize + f.dt.0 * t as isize) as f64, o[1] + (f.ds.1 * s as isize + f.dt.1 * t as isize) as f64);
    let church = Some([corner(0, 0), corner(f.len, 0), corner(f.len, cw), corner(0, cw)]);
    Some(Court { court, pieces, forecourt: None, church, paved: false })
}

/// A coaching inn's yard as the battlemap and the map draw it (`Court`: cobbled, open to the sky but
/// for the squares the gallery stands over, its corners each by the footprint's corner in the same
/// place; a water trough, barrels and a cart as `hub::Plan::yard_pieces` puts them); None for an inn
/// not round a yard.
fn yard_plan(world: &World, t0: &T0, l: &Layout, b: &town::Building) -> Option<Court> {
    if !hub_kind(t0, l, b).is_some_and(|k| k.is_inn()) {
        return None;
    }
    let sh = Shell::of(l, b);
    let plan = zone_plan(world, t0, l, l.index as usize, b, &sh, 0)?;
    let ((s0, s1, g0, g1), (a0, a1)) = plan.yard()?;
    let f = plan.frame();
    let corners = |s0: usize, s1: usize, t0: usize, t1: usize| {
        let (a, c) = (f.grid(s0 as isize, t0 as isize), f.grid(s1 as isize - 1, t1 as isize - 1));
        let (x0, x1, y0, y1) = (a.0.min(c.0) as f64, a.0.max(c.0) as f64 + 1.0, a.1.min(c.1) as f64, a.1.max(c.1) as f64 + 1.0);
        [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|g| sh.world_at(g[0], g[1]))
    };
    let pts = corners(s0, s1, g0, g1);
    let court = b.poly.iter().map(|q| *pts.iter().min_by(|p, r| geom::dist(**p, *q).total_cmp(&geom::dist(**r, *q))).expect("four corners")).collect();
    let mut pieces: Vec<(&'static str, [P; 4], Option<P>)> = plan.yard_pieces().into_iter().map(|(kind, (s0, s1, t0, t1))| (kind, corners(s0, s1, t0, t1), None)).collect();
    // (The carriage arch's mouth, "arch": from just inside the front wall to a square out on the
    // street, across its squares.)
    let at = |t: isize, ds: f64, dt: f64| {
        let (i, j) = f.grid(0, t);
        sh.world_at(i as f64 + 0.5 + ds * f.ds.0 as f64 + dt * f.dt.0 as f64, j as f64 + 0.5 + ds * f.ds.1 as f64 + dt * f.dt.1 as f64)
    };
    let (a0, a1) = (a0 as isize, a1 as isize - 1);
    pieces.push(("arch", [at(a0, -1.5, -0.5), at(a1, -1.5, 0.5), at(a1, -0.05, 0.5), at(a0, -0.05, -0.5)], None));
    Some(Court { court, pieces, forecourt: None, church: None, paved: true })
}

/// The plan a building laid out by zones is laid out on (`hub::plan`): its frame, its zones (a
/// palace's: `hub::Plan::palace`); as generated (no design, no ground shaped by hand). For the checks.
pub fn zone_plan_of(world: &World, t0: &T0, settlement: usize, building: usize) -> Option<hub::Plan> {
    let l = town::layout(world, t0, settlement);
    let b = l.building(building).filter(|b| b.structure == Structure::Roofed && zoned_arch(b))?;
    let sh = Shell::of(&l, b);
    let t = if b.parts.is_empty() && corner_towers(b) { keep_tower_squares(sh.nx, sh.ny) } else { 0 };
    zone_plan(world, t0, &l, settlement, b, &sh, if arch_of(b) == Arch::Keep { t } else { 0 })
}

/// A building's plan by zones (`hub::plan`), its front toward the nearest street (else the town's
/// middle); `corner`: a keep's grid's corner towers' side.
fn zone_plan(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, sh: &Shell, corner: usize) -> Option<hub::Plan> {
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
    let street_d = |p: P| {
        l.roads
            .iter()
            .flat_map(|(pts, _, w)| pts.windows(2).map(move |s| geom::seg_dist(p, s[0], s[1]) - 0.5 * w))
            .fold(f64::MAX, f64::min)
    };
    let near = |p: P| {
        let w = sh.world_at(p[0], p[1]);
        let d = street_d(w);
        if d < 80.0 { d } else { 1e4 + geom::dist(w, l.center) }
    };
    let ext_free = |i: isize, j: isize| sh.ext_free(i, j);
    let seed = hash3(world.stream("interior.hub"), town::rng_key(t0, settlement), b.id as i64, 0x4b);
    // (The capital's palace's front as the town laid it out with its forecourt: its middle, grid
    // units.)
    let front = l.forecourts.iter().find(|f| f.palace == b.id).map(|f| {
        let d = sub(mul(add(f.front[0], f.front[1]), 0.5), sh.origin);
        [dot(d, sh.u) / SQUARE_FT, dot(d, sh.v) / SQUARE_FT]
    });
    hub::plan(b, sh, key, corner, &town::keep_tops(b, world.stream("keep.tops")), seed, &near, &ext_free, (capital_of(t0, settlement), city_of(t0, settlement)), front)
}

/// A great hall furnished as a whole (`hall`): its level (index into `Interior::levels`), its room,
/// the frame it was laid out in, how.
#[derive(Clone, Copy, Debug)]
pub struct HallPlan {
    pub level: usize,
    pub room: i16,
    pub frame: hall::Frame,
    pub style: hall::Style,
}

/// The great halls of a building furnished as wholes, as generated (no design, no ground shaped by
/// hand): for the checks.
pub fn hall_plans(world: &World, t0: &T0, settlement: usize, building: usize) -> Vec<HallPlan> {
    let l = town::layout(world, t0, settlement);
    let Some(b) = l.building(building).filter(|b| b.structure == Structure::Roofed) else { return Vec::new() };
    let mut out = Vec::new();
    build_noting(world, t0, &l, settlement, b, &mut Vec::new(), &mut out);
    out
}

/// A keep big enough for deep dungeons below its cellar (`under::keep_dungeon`).
pub fn has_deep_dungeon(b: &town::Building) -> bool {
    b.structure == Structure::Roofed && arch_of(b) == Arch::Keep && geom::area(&b.poly).abs() >= 150.0 * SQUARE_FT * SQUARE_FT
}

/// The ways from a cellar to other sites: a trapdoor down into the sewers (on the street side),
/// stairs down to a keep's deep dungeons (in the middle of the far end). Which ones it should
/// have (their names), put on the free floor of `lvl` (the deepest level below ground; fewer if
/// there is none; none without a level).
fn cellar_links(mut lvl: Option<&mut Level>, t0: &T0, l: &Layout, b: &town::Building, sh: &Shell, stairs: [usize; 4]) -> Vec<&'static str> {
    let (nx, ny) = (sh.nx, sh.ny);
    let grid = |p: P| {
        let d = sub(p, sh.origin);
        [dot(d, sh.u) / SQUARE_FT, dot(d, sh.v) / SQUARE_FT]
    };
    let mut want = Vec::new();
    if let Some(q) = crate::under::sewer_link_of(t0, l, b) {
        want.push("the trapdoor to the sewers");
        if let Some(lvl) = lvl.as_deref_mut() {
            place_link(lvl, nx, ny, stairs, grid(q), "trapdoor", "trapdoor to the sewers", crate::under::sewer_id(l.index as usize, q));
        }
    }
    if has_deep_dungeon(b) {
        want.push("the stairs down to the deep dungeons");
        if let Some(lvl) = lvl {
            place_link(lvl, nx, ny, stairs, [nx as f64 * 0.5, ny as f64 * 0.5], "link_down", "stairs down to the deep dungeons", format!("k:{}:{}", l.index, b.id));
        }
    }
    want
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
    lvl.furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: 1, h: 1, cover: 0, blocks_move: false, height_ft: 0.0, hazard: None, sprite: 0, back: None, variant: None });
    lvl.links.push(Link { x: x as u16, y: y as u16, to });
    true
}

/// A building's grid: along its footprint's long axis, the squares inside it, and which squares
/// just outside are open ground (not inside a neighbouring building): a door must open onto one.
pub struct Shell<'a> {
    pub nx: usize,
    pub ny: usize,
    /// World ft of grid corner (0, 0); grid x runs along `u`, y along `v`.
    pub origin: P,
    pub u: P,
    pub v: P,
    pub inside: Vec<bool>,
    /// The outer wall as drawn (`Interior::outlines`), grid units; empty when the footprint is
    /// the grid's rectangle (or not convex: drawn on the squares).
    pub outline: Vec<P>,
    /// Squares the outer wall as drawn doesn't cut: inside its inner face (all of them without an
    /// outline). Stairs stand on these.
    pub whole: Vec<bool>,
    /// A keep's outer wall as drawn, one convex part per piece of it (`Building::parts`), grid
    /// units; then `outline` is empty.
    pub outline_parts: Vec<Vec<P>>,
    /// A keep's towers: the squares of each one's room (filling the tower: round about its middle,
    /// or square, its wall as thin as any outer wall), whether round, and the square in its middle
    /// (its spiral stair).
    pub towers: Vec<(Vec<usize>, bool, usize)>,
    /// Each tower's ways in (`towers`' order): the squares of the building's under its wall beside
    /// its room, the one way in on each floor kept by the room it opens onto (`layout_level`).
    pub tower_doors: Vec<Vec<usize>>,
    /// A keep's piece each square is drawn inside (index into `Building::parts`; `usize::MAX` none);
    /// empty for every other building.
    pub owner: Vec<usize>,
    neighbours: Vec<&'a Vec<P>>,
    /// A keep's footprint (world ft): the squares in it no room has are its walls, not open ground.
    own: Vec<P>,
    /// A temple's, a shrine's or a monastery's: the battlemap's squares round it (`town::temple::REACH_PAD`
    /// beyond it each way) whose open ground reaches a street (`town::temple::street_reach`), else the box's
    /// edge (`reach_of`); None for another building, or one no open square beside it reaches either from.
    reach: Option<town::temple::Reach>,
    /// A coaching inn's (`town::palace::is_coaching`: its lots merged round a yard).
    pub coaching: bool,
}


/// Half the outer wall's width as drawn (squares): a square is whole when it lies this far inside
/// the outline.
pub const WALL_HALF: f64 = 0.225;

impl<'a> Shell<'a> {
    pub fn of(l: &'a Layout, b: &town::Building) -> Shell<'a> {
        if !b.parts.is_empty() {
            return Shell::of_parts(l, b);
        }
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
        let mut inside: Vec<bool> = (0..nx * ny).map(|k| geom::contains(&grown, world_at((k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5))).collect();
        // A round tower's floors are round: its squares those of a circle in the box round them,
        // so its rooms keep their round shape (`shape::fits`).
        if arch_of(b) == Arch::Tower && round_ish(b) {
            let all: Vec<i16> = inside.iter().map(|&i| if i { 0 } else { -1 }).collect();
            if let Some(bx) = shape::room_box(&all, nx, 0) {
                let round = shape::shape_cells(bx, shape::ROUND);
                for (k, c) in round.iter().enumerate() {
                    let q = (bx[1] + k / bx[2]) * nx + bx[0] + k % bx[2];
                    inside[q] &= *c;
                }
            }
        }
        let neighbours = neighbours_of(l, b);
        let outline = outline_of(&b.poly, &inside, nx, origin, u, v);
        let mut sh = Shell { nx, ny, origin, u, v, inside, outline, whole: Vec::new(), outline_parts: Vec::new(), towers: Vec::new(), tower_doors: Vec::new(), owner: Vec::new(), neighbours, own: Vec::new(), reach: None, coaching: town::palace::is_coaching(l, b) };
        sh.whole = sh.whole_by(WALL_HALF);
        sh.reach = sh.reach_of(l, b);
        sh
    }

    /// A keep's grid (`Building::parts`): along its body, its squares whole squares from the
    /// body's first corner (the pieces are laid on them: `town::castle`), reaching over every
    /// piece. Inside: the squares whose middle lies in the body, a wing or a forebuilding, and each
    /// tower's room, filling its box (round in a round tower: the squares whose middle its circle
    /// holds), so its wall is as thin as any outer wall; any square of the building left under a
    /// tower beside its room is a way in (`tower_doors`, one kept on each floor), else wall. Its
    /// outer wall as drawn is a part per piece, each edge pushed out as far as the squares drawn
    /// inside that piece need (`part_outline`).
    fn of_parts(l: &'a Layout, b: &town::Building) -> Shell<'a> {
        let body = &b.parts[0].poly;
        let o = geom::obb(body);
        let (u, v) = (o.axis, [-o.axis[1], o.axis[0]]);
        let (a0, a1) = geom::extent(&b.poly, u);
        let (b0, b1) = geom::extent(&b.poly, v);
        let (ru, rv) = (dot(body[0], u), dot(body[0], v));
        let from = |r: f64, lo: f64| r - crate::core::ceil((r - lo) / SQUARE_FT - 1e-6) * SQUARE_FT;
        let (u0, v0) = (from(ru, a0), from(rv, b0));
        let nx = (crate::core::ceil((a1 - u0) / SQUARE_FT - 1e-6) as usize).max(2);
        let ny = (crate::core::ceil((b1 - v0) / SQUARE_FT - 1e-6) as usize).max(2);
        let origin = add(mul(u, u0), mul(v, v0));
        let grid = |p: P| {
            let d = sub(p, origin);
            [dot(d, u) / SQUARE_FT, dot(d, v) / SQUARE_FT]
        };
        // Each piece in grid units, wound anticlockwise in grid axes.
        let parts: Vec<Vec<P>> = b
            .parts
            .iter()
            .map(|p| {
                let mut g: Vec<P> = p.poly.iter().map(|q| grid(*q)).collect();
                if geom::area(&g) < 0.0 {
                    g.reverse();
                }
                g
            })
            .collect();
        let middle = |k: usize| [(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5];
        let mut inside = vec![false; nx * ny];
        // The piece each square is drawn inside (the one it lies deepest in; a tower's room its
        // tower).
        let mut owner = vec![usize::MAX; nx * ny];
        for k in 0..nx * ny {
            let best = (0..parts.len())
                .filter(|&pi| !matches!(b.parts[pi].kind, town::PartKind::Tower { .. }))
                .map(|pi| (pi, inset_by(&parts[pi], middle(k))))
                .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)));
            if let Some((pi, _)) = best.filter(|x| x.1 > 0.0) {
                inside[k] = true;
                owner[k] = pi;
            }
        }
        let mut towers = Vec::new();
        let mut tower_doors = Vec::new();
        let in_tower = |k: usize| b.parts.iter().enumerate().any(|(pi, p)| matches!(p.kind, town::PartKind::Tower { .. }) && geom::contains(&parts[pi], middle(k)));
        for (pi, part) in b.parts.iter().enumerate() {
            let town::PartKind::Tower { round } = part.kind else { continue };
            let g = &parts[pi];
            let c = mul(g.iter().fold([0.0, 0.0], |a, q| add(a, *q)), 1.0 / g.len() as f64);
            let (x0, x1) = g.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q[0]), a.1.max(q[0])));
            let d = crate::core::round(x1 - x0) as i64;
            let (bx, by) = (crate::core::round(c[0] - 0.5 * d as f64) as i64, crate::core::round(c[1] - 0.5 * d as f64) as i64);
            let w = d.max(1) as usize;
            if bx < 0 || by < 0 || bx as usize + w > nx || by as usize + w > ny {
                continue;
            }
            let rb = [bx as usize, by as usize, w, w];
            let cells = if round { shape::shape_cells(rb, shape::ROUND) } else { vec![true; w * w] };
            let squares: Vec<usize> = (0..w * w).filter(|&k| cells[k]).map(|k| (rb[1] + k / w) * nx + rb[0] + k % w).collect();
            for &k in &squares {
                inside[k] = true;
                owner[k] = pi;
            }
            // Its wall: the building's squares under it, but those beside its room and beside the
            // building outside every tower (its ways in, one kept on each floor).
            let side = |k: usize| {
                let (i, j) = (k % nx, k / nx);
                [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().filter(|&(a, b)| a < nx && b < ny).map(move |(a, b)| b * nx + a)
            };
            let mut doors = Vec::new();
            for k in 0..nx * ny {
                if !inside[k] || squares.contains(&k) || !geom::contains(&parts[pi], middle(k)) {
                    continue;
                }
                if side(k).any(|q| squares.contains(&q)) && side(k).any(|q| inside[q] && !in_tower(q)) {
                    doors.push(k);
                } else {
                    inside[k] = false;
                }
            }
            // Its spiral stair: the square nearest its middle that no door can open beside (none of
            // its sides on a square of the building outside the tower).
            let mid = [rb[0] as f64 + 0.5 * w as f64, rb[1] as f64 + 0.5 * w as f64];
            let inner = |k: usize| {
                let (i, j) = (k % nx, k / nx);
                [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].iter().all(|&(a, b)| a >= nx || b >= ny || !inside[b * nx + a] || squares.contains(&(b * nx + a)))
            };
            let near = |k: usize| geom::dist(middle(k), mid);
            let best = |all: bool| squares.iter().copied().filter(|&k| all || inner(k)).min_by(|&a, &b| near(a).total_cmp(&near(b)).then(a.cmp(&b)));
            let Some(spiral) = best(false).or_else(|| best(true)) else { continue };
            towers.push((squares, round, spiral));
            tower_doors.push(doors);
        }
        let outline_parts: Vec<Vec<P>> = (0..parts.len())
            .filter_map(|pi| {
                let own: Vec<P> = (0..nx * ny).filter(|&k| inside[k] && owner[k] == pi).map(middle).collect();
                let reach: Vec<usize> = (0..nx * ny).filter(|&k| geom::contains(&parts[pi], middle(k))).collect();
                part_outline(&parts[pi], &own, &reach, nx)
            })
            .collect();
        let mut sh = Shell { nx, ny, origin, u, v, inside, outline: Vec::new(), whole: Vec::new(), outline_parts, towers, tower_doors, owner, neighbours: neighbours_of(l, b), own: b.poly.clone(), reach: None, coaching: false };
        sh.whole = sh.whole_by(WALL_HALF);
        sh.reach = sh.reach_of(l, b);
        sh
    }

    /// How far grid point `c` lies inside the outer wall as drawn (its parts' best; f64::MAX
    /// without one).
    fn inset(&self, c: P) -> f64 {
        if !self.outline.is_empty() {
            return inset_by(&self.outline, c);
        }
        self.outline_parts.iter().map(|o| inset_by(o, c)).fold(if self.outline_parts.is_empty() { f64::MAX } else { f64::MIN }, f64::max)
    }

    /// Squares inside, lying at least `t` (squares) inside the outer wall as drawn.
    pub fn whole_by(&self, t: f64) -> Vec<bool> {
        let walled = !self.outline.is_empty() || !self.outline_parts.is_empty();
        (0..self.nx * self.ny)
            .map(|k| {
                let (i, j) = ((k % self.nx) as f64, (k / self.nx) as f64);
                self.inside[k] && (!walled || [[i, j], [i + 1.0, j], [i + 1.0, j + 1.0], [i, j + 1.0]].iter().all(|c| self.inset(*c) >= t))
            })
            .collect()
    }

    /// The outer wall as drawn (`Interior::outlines`): its convex parts, whose union is the
    /// footprint (one, a keep's one per piece, or none for the grid's rectangle), grid units, wound
    /// clockwise on screen.
    pub fn outlines(&self) -> Vec<Vec<[f32; 2]>> {
        if !self.outline_parts.is_empty() {
            return self.outline_parts.iter().map(|o| o.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()).collect();
        }
        if self.outline.is_empty() {
            return Vec::new();
        }
        vec![self.outline.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()]
    }

    /// World position (ft) of a grid point.
    pub fn world_at(&self, x: f64, y: f64) -> P {
        add(self.origin, add(mul(self.u, x * SQUARE_FT), mul(self.v, y * SQUARE_FT)))
    }

    /// Square (i, j) (maybe off the grid) is open ground: inside no neighbouring building (nor, for
    /// a keep, in its own walls).
    /// A temple's, a shrine's or a monastery's: ground that reaches a street (`reach`) where any
    /// beside it does.
    pub fn ext_free(&self, i: isize, j: isize) -> bool {
        self.open_ground(i, j) && self.reach.as_ref().is_none_or(|r| r.at(self.world_at(i as f64 + 0.5, j as f64 + 0.5)))
    }

    /// Square (i, j) is inside no neighbouring building (nor, for a keep, in its own walls): open
    /// ground, whether or not it reaches a street.
    pub fn open_ground(&self, i: isize, j: isize) -> bool {
        let p = self.world_at(i as f64 + 0.5, j as f64 + 0.5);
        !self.neighbours.iter().any(|poly| geom::contains(poly, p)) && (self.own.is_empty() || !geom::contains(&self.own, p))
    }

    /// `reach` for building `b` of layout `l`: a temple's, a shrine's or a monastery's, where an open
    /// square beside it reaches a street; else (no street near: a hamlet's shrine), where one reaches
    /// the open beyond the houses round it (`Reach::to_edge`), so that its door never opens onto a
    /// battlemap square of a neighbour's.
    fn reach_of(&self, l: &Layout, b: &town::Building) -> Option<town::temple::Reach> {
        if !town::temple::is_faith(b) || b.structure != Structure::Roofed {
            return None;
        }
        let (nx, ny) = (self.nx, self.ny);
        let r = town::temple::street_reach(l, b.id, &b.poly, &b.poly, town::temple::REACH_PAD);
        let is_in = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && self.inside[j as usize * nx + i as usize];
        let beside = |r: &town::temple::Reach| {
            (0..nx * ny).filter(|&k| self.inside[k]).any(|k| {
                let (i, j) = ((k % nx) as isize, (k / nx) as isize);
                [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| !is_in(i + di, j + dj) && self.open_ground(i + di, j + dj) && r.at(self.world_at((i + di) as f64 + 0.5, (j + dj) as f64 + 0.5)))
            })
        };
        if beside(&r) {
            return Some(r);
        }
        let r = r.to_edge();
        beside(&r).then_some(r)
    }
}

/// The roofed buildings near `b` (a door must not open into one), and the ground of another's lots
/// (`Layout::grounds`: a temple's or a monastery's own) but one `b` stands on.
fn neighbours_of<'a>(l: &'a Layout, b: &town::Building) -> Vec<&'a Vec<P>> {
    let c0 = geom::centroid(&b.poly);
    let reach = b.poly.iter().map(|p| geom::dist(*p, c0)).fold(0.0, f64::max) + 40.0;
    let near = |poly: &Vec<P>| poly.iter().any(|p| geom::dist(*p, c0) < reach + 60.0);
    let roofed = |id: u32| l.building(id as usize).is_some_and(|o| o.structure == Structure::Roofed);
    l.buildings
        .iter()
        .filter(|o| o.id != b.id && o.structure == Structure::Roofed && near(&o.poly))
        .map(|o| &o.poly)
        .chain(l.grounds.iter().filter(|g| g.0 != b.id && near(&g.1) && roofed(g.0) && !town::on_lot(&g.1, &b.poly)).map(|g| &g.1))
        .collect()
}

/// A piece of a keep's outer wall as drawn (`pts`: convex, grid units, anticlockwise in grid axes):
/// each edge pushed out as far as the squares drawn inside this piece (`own`: their middles) need
/// to lie `WALL_HALF` + 0.1 in from it, cut back to within half a square of the squares whose middle
/// it holds (`reach`), as `outline_of` does for a whole footprint. None for a piece holding none.
fn part_outline(pts: &[P], own: &[P], reach: &[usize], nx: usize) -> Option<Vec<P>> {
    if reach.is_empty() {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
    }
    let mut out: Vec<P> = vec![[x0 - 5.0, y0 - 5.0], [x1 + 5.0, y0 - 5.0], [x1 + 5.0, y1 + 5.0], [x0 - 5.0, y1 + 5.0]];
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        let e = sub(b, a);
        let l = geom::len(e);
        if l < 1e-9 {
            continue;
        }
        let o = [e[1] / l, -e[0] / l];
        let least = own.iter().map(|c| geom::cross(e, sub(*c, a)) / l).fold(f64::MAX, f64::min);
        let push = (WALL_HALF + 0.1 - least).max(0.0);
        out = geom::clip_plain(&out, o, dot(a, o) + push);
    }
    let mut corners: Vec<P> = Vec::new();
    for &k in reach {
        let (i, j) = ((k % nx) as f64, (k / nx) as f64);
        corners.extend([[i - 0.5, j - 0.5], [i + 1.5, j - 0.5], [i + 1.5, j + 1.5], [i - 0.5, j + 1.5]]);
    }
    let hull = geom::hull(corners);
    for k in 0..hull.len() {
        let (a, b) = (hull[k], hull[(k + 1) % hull.len()]);
        let e = sub(b, a);
        out = geom::clip_plain(&out, [e[1], -e[0]], dot(a, [e[1], -e[0]]));
    }
    let mut pts: Vec<P> = Vec::with_capacity(out.len());
    for p in out {
        if pts.last().is_none_or(|l| (l[0] - p[0]).abs() + (l[1] - p[1]).abs() > 1e-4) {
            pts.push(p);
        }
    }
    while pts.len() > 1 && (pts[0][0] - pts[pts.len() - 1][0]).abs() + (pts[0][1] - pts[pts.len() - 1][1]).abs() <= 1e-4 {
        pts.pop();
    }
    (pts.len() >= 3).then_some(pts)
}

/// A footprint's outer wall as drawn (grid units, the wall's middle on the footprint, each edge
/// pushed out as far as the squares inside need to lie `WALL_HALF` + 0.1 in from it), for one that
/// isn't the grid's rectangle: the footprint when it is convex (else none: drawn on the squares),
/// cut back to within half a square of the squares inside it (an acute corner holds none), wound
/// anticlockwise in grid axes (clockwise on screen), points the cuts left together merged.
fn outline_of(poly: &[P], inside: &[bool], nx: usize, origin: P, u: P, v: P) -> Vec<P> {
    if inside.iter().all(|&i| i) || !inside.iter().any(|&i| i) {
        return Vec::new();
    }
    let mut pts: Vec<P> = poly
        .iter()
        .map(|p| {
            let d = sub(*p, origin);
            [dot(d, u) / SQUARE_FT, dot(d, v) / SQUARE_FT]
        })
        .collect();
    let n = pts.len();
    let turns: Vec<f64> = (0..n).map(|k| geom::cross(sub(pts[(k + 1) % n], pts[k]), sub(pts[(k + 2) % n], pts[(k + 1) % n]))).collect();
    if n < 3 || !(turns.iter().all(|&t| t >= -1e-6) || turns.iter().all(|&t| t <= 1e-6)) {
        return Vec::new();
    }
    if turns.iter().sum::<f64>() < 0.0 {
        pts.reverse();
    }
    // Each edge pushed out as far as its squares need: every square's middle at least a tenth of
    // a square inside the wall's inner face (squares count as inside up to 1.5 ft past the
    // footprint); an edge no square reaches past stays on the footprint.
    let middles: Vec<P> = inside.iter().enumerate().filter(|(_, i)| **i).map(|(k, _)| [(k % nx) as f64 + 0.5, (k / nx) as f64 + 0.5]).collect();
    let ny = inside.len() / nx;
    let mut grown: Vec<P> = vec![[-5.0, -5.0], [nx as f64 + 5.0, -5.0], [nx as f64 + 5.0, ny as f64 + 5.0], [-5.0, ny as f64 + 5.0]];
    for k in 0..pts.len() {
        let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
        let e = sub(b, a);
        let l = geom::len(e);
        if l < 1e-9 {
            continue;
        }
        let out = [e[1] / l, -e[0] / l];
        let least = middles.iter().map(|c| geom::cross(e, sub(*c, a)) / l).fold(f64::MAX, f64::min);
        let push = (WALL_HALF + 0.1 - least).max(0.0);
        grown = geom::clip_plain(&grown, out, dot(a, out) + push);
    }
    pts = grown;
    // Within the squares grown by half a square each way.
    let mut corners: Vec<P> = Vec::new();
    for (k, _) in inside.iter().enumerate().filter(|(_, i)| **i) {
        let (i, j) = ((k % nx) as f64, (k / nx) as f64);
        corners.extend([[i - 0.5, j - 0.5], [i + 1.5, j - 0.5], [i + 1.5, j + 1.5], [i - 0.5, j + 1.5]]);
    }
    let reach = geom::hull(corners);
    for k in 0..reach.len() {
        let (a, b) = (reach[k], reach[(k + 1) % reach.len()]);
        let e = sub(b, a);
        let out = [e[1], -e[0]];
        pts = geom::clip_plain(&pts, out, dot(a, out));
    }
    let mut out: Vec<P> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_none_or(|l| (l[0] - p[0]).abs() + (l[1] - p[1]).abs() > 1e-4) {
            out.push(p);
        }
    }
    while out.len() > 1 && (out[0][0] - out[out.len() - 1][0]).abs() + (out[0][1] - out[out.len() - 1][1]).abs() <= 1e-4 {
        out.pop();
    }
    if out.len() < 3 { Vec::new() } else { out }
}

/// How far point `p` lies inside convex polygon `poly` (wound anticlockwise in grid axes): its
/// least distance to an edge's line, negative outside.
fn inset_by(poly: &[P], p: P) -> f64 {
    (0..poly.len())
        .map(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            let e = sub(b, a);
            geom::cross(e, sub(p, a)) / geom::len(e).max(1e-12)
        })
        .fold(f64::MAX, f64::min)
}

fn build(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building) -> Interior {
    build_noting(world, t0, l, settlement, b, &mut Vec::new(), &mut Vec::new())
}

/// `build`, noting the floors a building laid out by zones fell back on the shared layout for
/// (`hub_fallbacks`) and its halls furnished as wholes (`hall_plans`).
fn build_noting(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, fell: &mut Vec<i8>, plans: &mut Vec<HallPlan>) -> Interior {
    let bi = b.id as usize;
    let mut rng = Pcg32::new(hash3(world.stream("interior"), town::rng_key(t0, settlement), bi as i64, 0x1a7), 71);
    let arch = arch_of(b);
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);

    // Grid along the footprint's long axis.
    let sh = Shell::of(l, b);
    let (nx, ny, origin, u, v) = (sh.nx, sh.ny, sh.origin, sh.u, sh.v);
    let world_at = |x: f64, y: f64| sh.world_at(x, y);
    let inside = &sh.inside;
    let ext_free = |i: isize, j: isize| sh.ext_free(i, j);
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
    // (A temple's, a shrine's or a monastery's with no street near: a side open all along before one a
    // neighbour stands against, so that its door doesn't open into the gap between them.)
    let edge_on = |side: usize| -> usize {
        let (di, dj) = [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)][side];
        (0..ny as isize).flat_map(|j| (0..nx as isize).map(move |i| (i, j))).filter(|&(i, j)| is_in(i, j) && !is_in(i + di, j + dj)).count()
    };
    let faith = town::temple::is_faith(b);
    let front = (0..4)
        .filter(|&k| !any_open || open[k] > 0)
        .min_by(|&x, &y| {
            let score = |k: usize| {
                let p = world_at(sides[k][0], sides[k][1]);
                let d = street_d(p);
                let shut = if faith && open[k] < edge_on(k) { 1e5 } else { 0.0 };
                if d < 80.0 { d } else { 1e4 + shut + geom::dist(p, l.center) }
            };
            score(x).total_cmp(&score(y))
        })
        .unwrap_or(0);

    // Levels: a cellar, the ground floor and the storeys above; at least two levels.
    let floors = b.floors.max(1) as i8;
    let mut zs: Vec<i8> = vec![-1];
    zs.extend(0..floors);
    // Keeps and towers (or any roof drawn so): walk the battlements on the roof.
    let open_roof = battlements(b).is_some();
    // A keep's corner towers rise a storey above its battlements (a keep built of pieces: its own
    // towers, `Shell::towers`).
    let t = if b.parts.is_empty() && corner_towers(b) { keep_tower_squares(nx, ny) } else { 0 };
    // A castle's keep, a big barracks, a palace and a noble estate are laid out by zones (`hub`): a
    // keep of pieces up to its body's top, each piece as high as it rises (its towers under cones: no
    // tops of their own).
    let zoned = zoned_arch(b);
    // (Only a keep has the grid's corner towers.)
    let corner = if arch == Arch::Keep { t } else { 0 };
    let capital = capital_of(t0, settlement);
    let mut plan = if zoned { zone_plan(world, t0, l, settlement, b, &sh, corner) } else { None };
    if plan.is_none() && zoned && hub::kind_of(b, key, &sh, capital, city_of(t0, settlement)).is_some() {
        fell.extend(0..floors);
    }
    // (A town hall's belfry: its storeys above the hall's, `hub::Plan::belfry`.)
    let belfry = plan.as_ref().and_then(|p| p.belfry());
    if let Some(p) = plan.as_ref().filter(|p| p.pieces || belfry.is_some()) {
        zs = (-1..=p.reaches()).collect();
    } else if open_roof {
        zs.push(ROOF);
    }
    let towers: Option<Vec<Vec<usize>>> = (arch == Arch::Keep && !b.parts.is_empty()).then(|| sh.towers.iter().map(|t| t.0.clone()).collect());
    // (With their ways in.)
    let rings: Option<Vec<(Vec<usize>, Vec<usize>)>> = towers.as_ref().map(|t| t.iter().cloned().zip(sh.tower_doors.iter().cloned()).collect());
    // (A castle's keep reshaped by hand off its grid: no towers on its grid's corners either.)
    let rings = rings.or_else(|| (arch == Arch::Keep && !corner_towers(b)).then(Vec::new));
    if arch == Arch::Keep && (t > 0 || towers.as_ref().is_some_and(|t| !t.is_empty())) && open_roof && !plan.as_ref().is_some_and(|p| p.pieces) {
        zs.push(TOWER_TOP);
    }
    let entry_level = 1;

    // The stair block: at the back of the building, in the middle of the short side (towers:
    // near the centre), on inside squares. Inns and taverns lay out the ground floor first
    // and put the stairs in the taproom, where guests can reach them.
    let mut ground_pre: Option<Level> = None;
    // (On squares the outer wall as drawn doesn't cut; in a building too small for that, as far
    // in from it as they go.)
    // (Off a keep's towers and their ways in: their spiral stairs are theirs.)
    let off_towers = |mut m: Vec<bool>| {
        for k in towers.iter().flatten().flatten().chain(sh.tower_doors.iter().flatten()) {
            m[*k] = false;
        }
        m
    };
    let whole = off_towers(sh.whole.clone());
    let mut stairs = match &plan {
        Some(p) => p.stairs,
        None if key == Some("shrine") => shrine_stairs(&whole, nx, ny, front).unwrap_or_else(|| place_stairs(&whole, nx, ny, front, arch)),
        None => place_stairs(&whole, nx, ny, front, arch),
    };
    let on = |st: [usize; 4], m: &[bool]| (st[1]..st[1] + st[3]).all(|j| (st[0]..st[0] + st[2]).all(|i| m[j * nx + i]));
    if plan.is_none() && !on(stairs, &whole) {
        for t in [0.1, 0.0, f64::MIN] {
            let m = off_towers(sh.whole_by(t));
            stairs = place_stairs(&m, nx, ny, front, arch);
            if on(stairs, &m) {
                break;
            }
        }
        // (Where no two squares side by side are clear of the wall but one is: a ladder's square
        // there, the clear one nearest the stairs' place.)
        if !on(stairs, &whole) {
            let c = [stairs[0] as f64 + 0.5 * stairs[2] as f64, stairs[1] as f64 + 0.5 * stairs[3] as f64];
            let d = |k: usize| ((k % nx) as f64 + 0.5 - c[0]).abs() + ((k / nx) as f64 + 0.5 - c[1]).abs();
            let clear = off_towers(sh.whole_by(WALL_HALF - 1e-3));
            if let Some(k) = (0..nx * ny).filter(|&k| clear[k]).min_by(|&a, &b| d(a).total_cmp(&d(b)).then(a.cmp(&b))) {
                stairs = [k % nx, k / nx, 1, 1];
            }
        }
    }
    // (An inn laid out by zones has its stairs in its taproom already: `hub::inn`.)
    if is_pub(arch) && plan.is_none() {
        let prog = program(arch, 0, floors == 1, key);
        let lvl = layout_level(&inside, nx, ny, &prog, [0, 0, 0, 0], front, true, arch, rings.as_deref(), &ext_free, &mut rng);
        // (Clear of the outer wall as drawn if they can be; in the taproom whatever.)
        // (Then on squares the wall cuts if it leaves them clear enough (or none is), else, in a
        // taproom of 12 squares or more, a square beside a door clear of it, else on the cut squares;
        // where the taproom has room for them nowhere, the ground floor all taproom, as a small inn's
        // is, and a square beside a door if need be.)
        let clear = sh.whole_by(WALL_HALF - 1e-3);
        let cut = pub_stairs(&lvl, nx, ny, inside, true, false);
        let roomy = lvl.doors.iter().find(|d| d.kind == "front").is_some_and(|d| lvl.cells.iter().filter(|&&c| c == d.rooms[0]).count() >= 12);
        let st = pub_stairs(&lvl, nx, ny, &sh.whole, true, false)
            .or_else(|| pub_stairs(&lvl, nx, ny, &sh.whole, false, false))
            .or_else(|| cut.filter(|&s| on(s, &clear) || !clear.iter().any(|&c| c)))
            .or_else(|| if roomy { pub_stairs(&lvl, nx, ny, &sh.whole, false, true) } else { None })
            .or(cut);
        if let Some(st) = st {
            stairs = st;
            ground_pre = Some(lvl);
        } else {
            let all = LevelProgram { rooms: prog.rooms[..1].to_vec(), filler: None, ..prog };
            let lvl = layout_level(&inside, nx, ny, &all, [0, 0, 0, 0], front, true, arch, rings.as_deref(), &ext_free, &mut rng);
            let st = pub_stairs(&lvl, nx, ny, &sh.whole, true, false)
                .or_else(|| pub_stairs(&lvl, nx, ny, &sh.whole, false, false))
                .or_else(|| pub_stairs(&lvl, nx, ny, inside, true, false))
                .or_else(|| pub_stairs(&lvl, nx, ny, &sh.whole, false, true))
                .or_else(|| pub_stairs(&lvl, nx, ny, inside, false, true));
            if let Some(st) = st {
                stairs = st;
                ground_pre = Some(lvl);
            }
        }
    }

    // (Furniture stands clear of the outer wall as drawn: in a building laid out by zones (a keep's
    // round tower rooms), off any square its middle line cuts more than a tenth of a square into;
    // elsewhere off a square it cuts deeply, its far corner 0.4 of a square beyond it.)
    let floor = sh.whole_by(if plan.is_some() { -0.1 } else { -0.4 });
    // Great halls furnished as wholes (`hall`): a castle's, a town's or a guild's, a noble estate's,
    // a barracks' mess hall; thrones only in a castle or a palace.
    let (_, thrones, styled) = hall_kind(world, t0, settlement, b);
    // (A temple's, a shrine's or a monastery's rooms open to every square: `furnish_rooms`, `faith`.)
    let mut levels = Vec::new();
    for (li, &z) in zs.iter().enumerate() {
        let top = li + 1 == zs.len() || matches!(zs.get(li + 1), Some(&ROOF) | Some(&TOWER_TOP));
        // (A keep of pieces' top as the shared code would have it, if its zones won't do.)
        let pieces_top = plan.as_ref().is_some_and(|p| p.pieces && z == p.reaches() && p.kind != hub::Kind::ShellKeep);
        let prog = program(arch, if pieces_top { ROOF } else { z.min(ROOF) }, top, key);
        // (Laid out by zones, if it holds together: every room in one piece and reached.)
        let laid = plan.as_mut().and_then(|p| p.level(z));
        let tried = laid.is_some();
        // (A belfry's storey above the hall's: reached by its spiral stair alone.)
        let tower_only = belfry.is_some_and(|_| z >= floors);
        let reach_st = match belfry {
            Some((sp, _)) if tower_only => [sp % nx, sp / nx, 1, 1],
            _ => stairs,
        };
        let hubbed = laid.and_then(|h| {
            let (name, roof, items, halls) = (h.name, h.roof, h.items.clone(), h.halls.clone());
            // (Its stair hall's squares all the belfry's room: no main stairs there.)
            let mut lvl = hub_level(h, &sh, nx, ny, if tower_only { [0, 0, 0, 0] } else { stairs }, corner, front, z == 0, &ext_free, &mut rng);
            if tower_only {
                lvl.has_stairs = false;
            }
            (rooms_whole(&lvl.cells, nx, ny) && reaches_all(&lvl, nx, ny, reach_st)).then_some((lvl, name, roof, items, halls))
        });
        if tried && hubbed.is_none() {
            fell.push(z);
        }
        let hall_f: Vec<hall::Frame> = hubbed.as_ref().map(|h| h.4.clone()).unwrap_or_default();
        let (mut lvl, hubbed) = match (&ground_pre, z, hubbed) {
            (_, _, Some((lvl, name, roof, items, _))) => (lvl, Some((name, roof, items))),
            (Some(g), 0, None) => (g.clone(), None),
            (_, TOWER_TOP, None) => (tower_tops(&inside, nx, ny, t, towers.as_deref().unwrap_or_default(), stairs, &mut rng), None),
            _ => {
                // (A monastery's crypt under its church only: `hub::Plan::crypt`.)
                let crypt = if z < 0 { plan.as_ref().and_then(|p| p.crypt()) } else { None };
                (layout_level(crypt.as_deref().unwrap_or(inside), nx, ny, &prog, stairs, front, z == 0, arch, rings.as_deref(), &ext_free, &mut rng), None)
            }
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
        if let Some((name, roof, items)) = &hubbed {
            lvl.name = name.to_string();
            lvl.furniture.extend(items.iter().copied());
            if *roof {
                lvl.roof = true;
                lvl.windows.clear();
            }
        } else if pieces_top {
            lvl.name = "Battlements".to_string();
            lvl.roof = true;
            lvl.windows.clear();
        }
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
        // A town hall's belfry: its spiral stair from the top floor's landing up.
        if let Some((sp, _)) = belfry
            && z >= floors - 1
            && lvl.cells[sp] >= 0
        {
            push_item(&mut lvl.furniture, "spiral_stair", sp % nx, sp / nx);
        }
        // A keep built of pieces: a spiral stair in the middle of each of its towers; their rooms
        // round in round towers.
        if arch == Arch::Keep && zf >= 0 {


            for (sq, round, mid) in &sh.towers {
                let r = lvl.cells[*mid];
                if r >= 0 {
                    push_item(&mut lvl.furniture, "spiral_stair", mid % nx, mid / nx);
                    if *round && sq.iter().all(|&k| lvl.cells[k] == r) && shape::fits(&lvl.cells, nx, r, shape::ROUND) {
                        lvl.rooms[r as usize].shape = shape::ROUND;
                    }
                }
            }
        }
        // A round tower's rooms are round (while their squares fit the shape).
        if arch == Arch::Tower && round_ish(b) {
            for ri in 0..lvl.rooms.len() {
                if lvl.rooms[ri].squares > 0 && shape::fits(&lvl.cells, nx, ri as i16, shape::ROUND) {
                    lvl.rooms[ri].shape = shape::ROUND;
                }
            }
        }
        // (A temple's apse, a chapter house's octagon: drawn so while its squares are the shape's.)
        for ri in 0..lvl.rooms.len() {
            if lvl.rooms[ri].shape != 0 && !shape::fits(&lvl.cells, nx, ri as i16, lvl.rooms[ri].shape) {
                lvl.rooms[ri].shape = 0;
            }
        }
        // (A building laid out by zones, on a level the shared code laid out (a barracks' roof): its
        // corner towers' storey a guardroom, stores, an armory and a garrison, as on its floors.)
        if let Some(p) = plan.as_ref().filter(|_| hubbed.is_none()) {
            p.finish_shared(&mut lvl);
        }
        // (Its great hall furnished as a whole, in the frame its zones gave it or one found in its
        // squares; a castle's minstrels' gallery over an open hall, its musicians' corner.)
        let mut halls: Vec<(i16, hall::Frame, bool, hall::Style)> = Vec::new();
        for &(kind, style) in &styled {
            for ri in (0..lvl.rooms.len()).filter(|&ri| lvl.rooms[ri].kind == kind && lvl.rooms[ri].squares > 0) {
                let ours = |f: &hall::Frame| {
                    let (i, j) = f.grid(f.len as isize / 2, f.wid as isize / 2);
                    i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && lvl.cells[j as usize * nx + i as usize] == ri as i16
                };
                // (A chapel along its length, its altar toward the east where it can be.)
                let found = if style == hall::Style::Chapel { chapel_frame(&lvl, nx, ny, ri as i16, stairs, u, v, Some(&floor)) } else { hall_f.iter().copied().find(ours).map(|f| (f, false)).or_else(|| hall_frame(&lvl, nx, ny, ri as i16, stairs)) };
                if let Some((f, turns)) = found {
                    halls.push((ri as i16, f, turns, style));
                }
            }
        }
        let open_hall = lvl.rooms.iter().any(|r| r.kind == OPEN_BELOW && r.squares > 0);
        let galleries: Vec<i16> = (0..lvl.rooms.len() as i16).filter(|&ri| open_hall && lvl.rooms[ri as usize].kind == "minstrels' gallery").collect();
        let skip: Vec<i16> = halls.iter().map(|h| h.0).chain(galleries.iter().copied()).collect();
        furnish_rooms(&mut lvl, nx, ny, stairs, None, Some(&floor), &skip, thrones, faith, &mut rng);
        for &(ri, f, turns, style) in &halls {
            let seed = hash3(world.stream("interior.hall"), town::rng_key(t0, settlement), bi as i64, zf as i64);
            let n = lvl.furniture.len();
            let mut f = furnish_styled(&mut lvl, nx, ny, stairs, ri, (f, turns), style, Some(&floor), seed);
            // (A chapel with no altar, a door on its axis at the end: `chapel_door`.)
            if style == hall::Style::Chapel
                && !lvl.furniture[n..].iter().any(|it| it.kind == "altar")
                && let Some(g) = chapel_door(&mut lvl, nx, ny, stairs, ri, f, Some(&floor), seed, n)
            {
                f = g;
            }
            // (A shrine whose god's statue fits nowhere in its frame (a slanted lot's): furnished by its
            // rules instead, its pieces in the god's look.)
            if let hall::Style::Shrine { god } = style
                && !lvl.furniture[n..].iter().any(|it| it.kind == "god_statue")
            {
                lvl.furniture.truncate(n);
                let mut frng = Pcg32::new(hash3(seed, ri as i64, 0x5f1, 0), 77);
                furnish_rooms(&mut lvl, nx, ny, stairs, Some(ri), Some(&floor), &[], thrones, faith, &mut frng);
                for it in lvl.furniture[n..].iter_mut().filter(|it| matches!(it.kind, "god_statue" | "candle_stand")) {
                    it.variant = Some(god);
                }
            }
            // (A noble estate's hall whose zones' frame holds no dais (a slanted lot's end cut across
            // it): in the largest box of its squares.)
            if matches!(style, hall::Style::Estate { .. }) && !lvl.furniture[n..].iter().any(|it| it.kind == "low_dais")
                && let Some(alt) = hall_frame(&lvl, nx, ny, ri, stairs).filter(|a| a.0 != f)
            {
                let kept = lvl.furniture.split_off(n);
                let g = furnish_styled(&mut lvl, nx, ny, stairs, ri, alt, style, Some(&floor), seed);
                if lvl.furniture[n..].iter().any(|it| it.kind == "low_dais") {
                    f = g;
                } else {
                    lvl.furniture.truncate(n);
                    lvl.furniture.extend(kept);
                }
            }
            plans.push(HallPlan { level: li, room: ri, frame: f, style });
        }
        for &ri in &galleries {
            hall::furnish_gallery(&mut lvl, nx, ny, ri);
        }
        // (Laid out by zones: nothing stands on an open top (a platform, a walk).)
        if let Some(p) = &plan {
            p.clear_tops(&mut lvl);
        }
        if z == -1 {
            cellar_links(Some(&mut lvl), t0, l, b, &sh, stairs);
        }
        // (A guild's arms in its own look; a thieves' guild hangs none.)
        match arms_look(key) {
            Some(a) => lvl.furniture.iter_mut().filter(|it| it.kind == "guild_arms" && it.variant.is_none()).for_each(|it| it.variant = Some(a)),
            None => lvl.furniture.retain(|it| it.kind != "guild_arms"),
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
        sprites: Vec::new(),
        outlines: sh.outlines(),
    }
}

/// Room `ri` furnished as a hall (`hall::furnish_hall`) in frame `f`; where it may be turned (`f.1`,
/// `hall_frame`: entered on a long wall) and holds no dais at the far end, the other way round. The
/// frame it is furnished in.
#[allow(clippy::too_many_arguments)]
fn furnish_styled(lvl: &mut Level, nx: usize, ny: usize, stairs: [usize; 4], ri: i16, f: (hall::Frame, bool), style: hall::Style, whole: Option<&[bool]>, seed: u64) -> hall::Frame {
    let (f, turns) = f;
    let n = lvl.furniture.len();
    if hall::furnish_hall(lvl, nx, ny, stairs, ri, f, style, whole, seed) || !turns {
        return f;
    }
    lvl.furniture.truncate(n);
    let back = hall::Frame { o: f.grid(f.len as isize - 1, 0), ds: (-f.ds.0, -f.ds.1), ..f };
    if hall::furnish_hall(lvl, nx, ny, stairs, ri, back, style, whole, seed) {
        return back;
    }
    lvl.furniture.truncate(n);
    hall::furnish_hall(lvl, nx, ny, stairs, ri, f, style, whole, seed);
    f
}

/// A chapel furnished with no altar at either end of frame `f` where a door opens in that end's wall
/// on its axis (as generated): that door moved along the wall off the axis, between the same rooms,
/// where the altar then has room, the chapel furnished again (its frame; its furniture first cleared
/// from `n` on), the level's walls run again. None, nothing changed, where no such move gives it one.
#[allow(clippy::too_many_arguments)]
fn chapel_door(lvl: &mut Level, nx: usize, ny: usize, stairs: [usize; 4], ri: i16, f: hall::Frame, whole: Option<&[bool]>, seed: u64, n: usize) -> Option<hall::Frame> {
    let style = hall::Style::Chapel;
    let back = hall::Frame { o: f.grid(f.len as isize - 1, 0), ds: (-f.ds.0, -f.ds.1), ..f };
    let kept = lvl.furniture.split_off(n);
    for fr in [f, back] {
        let (l, w) = (fr.len as isize, fr.wid as isize);
        let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
        let room_at = |lvl: &Level, p: (isize, isize)| (p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < nx && (p.1 as usize) < ny).then(|| lvl.cells[p.1 as usize * nx + p.0 as usize]);
        // (A door's two squares, as `pub_stairs` reads its edge.)
        let squares = |d: &Door| {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
            if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] }
        };
        let on_axis: Vec<usize> = (0..lvl.doors.len())
            .filter(|&i| {
                let sq = squares(&lvl.doors[i]);
                (cl..=cr).any(|t| {
                    let (p, q) = (fr.grid(l - 1, t), fr.grid(l, t));
                    (sq[0] == p && sq[1] == q) || (sq[0] == q && sq[1] == p)
                })
            })
            .collect();
        for i in on_axis {
            let (d0, sq) = (lvl.doors[i].clone(), squares(&lvl.doors[i]));
            let pair = (room_at(lvl, sq[0]), room_at(lvl, sq[1]));
            for k in [1isize, -1, 2, -2, 3, -3] {
                let (dx, dy) = (fr.dt.0 * k, fr.dt.1 * k);
                let moved = [(sq[0].0 + dx, sq[0].1 + dy), (sq[1].0 + dx, sq[1].1 + dy)];
                let t = fr.local(moved[0].0, moved[0].1).1.min(fr.local(moved[1].0, moved[1].1).1);
                // (Not where a piece or the stairs already stand in the room beyond.)
                let on = |p: (isize, isize), x: u16, y: u16, w: u16, h: u16| (x as isize..(x + w) as isize).contains(&p.0) && (y as isize..(y + h) as isize).contains(&p.1);
                let held = |p: (isize, isize)| lvl.furniture.iter().any(|it| on(p, it.x, it.y, it.w, it.h)) || on(p, stairs[0] as u16, stairs[1] as u16, stairs[2] as u16, stairs[3] as u16);
                if (cl..=cr).contains(&t) || (room_at(lvl, moved[0]), room_at(lvl, moved[1])) != pair || held(moved[0]) || held(moved[1]) || lvl.doors.iter().enumerate().any(|(j, e)| j != i && squares(e) == moved) {
                    continue;
                }
                (lvl.doors[i].a, lvl.doors[i].b) = ([d0.a[0] + dx as f32, d0.a[1] + dy as f32], [d0.b[0] + dx as f32, d0.b[1] + dy as f32]);
                if hall::furnish_hall(lvl, nx, ny, stairs, ri, fr, style, whole, seed) {
                    // (The walls run past where it was, open where it is.)
                    lvl.walls = wall_runs(&lvl.cells, &lvl.rooms, nx, ny, &lvl.doors);
                    return Some(fr);
                }
                lvl.furniture.truncate(n);
                lvl.doors[i] = d0.clone();
            }
        }
    }
    lvl.furniture.extend(kept);
    None
}

/// What a building's halls are (as generated, and furnished in the designer): whether it is one of
/// a castle's buildings, whether thrones go in it (a castle's or a palace's only) and how its great
/// hall is furnished as a whole (`hall_style`).
pub(crate) fn hall_kind(world: &World, t0: &T0, settlement: usize, b: &town::Building) -> (bool, bool, Vec<(&'static str, hall::Style)>) {
    let key = b.func.map(|f| town::catalog::CATALOG[f as usize].key);
    let castle = key == Some("castle") || b.keep.is_some() || b.ward == town::catalog::Ward::Castle;
    (castle, castle || key == Some("palace"), hall_style(world, t0, settlement, b, arch_of(b), key, castle))
}

/// How a building's halls are furnished as wholes (`hall`), with the kinds of room they are: a
/// barracks' mess hall (a castle's flying its house); a castle keep's great hall, its throne dais of
/// stone and gilt in a capital (a pair of thrones) and rich cities, of timber elsewhere, now and then
/// with a lesser chair; a town's or a guild's hall, a harbor master's, an auction house's (a stone
/// dais in a city); any other hall's tables only, as a mess hall's (a guard post's, an orphanage's);
/// a noble estate's; the capital's palace's throne room (a pair of thrones, stone and gilt),
/// banqueting hall and ballroom (and a great hall, where it is laid out as an older palace); another
/// palace's great hall, its throne dais as a castle's. A palace's banners fly the town's house, as
/// its castle's. `castle`: one of a castle's buildings (its banners fly the castle's house).
fn hall_style(world: &World, t0: &T0, settlement: usize, b: &town::Building, arch: Arch, key: Option<&str>, castle: bool) -> Vec<(&'static str, hall::Style)> {
    use crate::t0::settle::Tier;
    let roll = |salt: i64| hash3(world.stream("interior.hall"), town::rng_key(t0, settlement), salt, 0);
    let house = |own: bool| (if own { hash3(world.stream("interior.house"), town::rng_key(t0, settlement), b.id as i64, 1) } else { roll(0x40) } % HOUSES as u64) as u8;
    let set = t0.settlements.get(settlement);
    if key == Some("barracks") {
        return vec![("mess hall", hall::Style::Mess { house: castle.then(|| house(false)) })];
    }
    let capital = set.is_some_and(|s| s.capital);
    if b.keep.is_some() || key == Some("castle") {
        let rich = capital || set.is_some_and(|s| s.tier == Tier::Metropolis || (s.tier == Tier::City && roll(0x41) % 2 == 0));
        let seating = if capital { 1 } else if roll(0x42) % 3 == 0 { 2 } else { 0 };
        return vec![("great hall", hall::Style::Castle { dais: seating + if rich { 0 } else { 3 }, house: house(false), banquet: false })];
    }
    match arch {
        // (A low dais and a chair of office in a town hall, a guild's, a harbor master's and an
        // auction house; the tables only in every other hall: a guard post's, an orphanage's...)
        Arch::Hall if matches!(key, Some("town_hall" | "craft_guild" | "merchant_guild" | "adventurers_guild" | "thieves_guild" | "mages_guild" | "harbor_master" | "auction_house")) => {
            vec![("great hall", hall::Style::Civic { stone: set.is_some_and(|s| s.tier >= Tier::City), arms: arms_look(key) })]
        }
        Arch::Hall => vec![("great hall", hall::Style::Mess { house: None })],
        Arch::Mansion if key == Some("palace") && capital => {
            let house = house(false);
            vec![
                ("throne room", hall::Style::Throne { dais: 1, house }),
                ("banqueting hall", hall::Style::Banquet { house }),
                ("ballroom", hall::Style::Ballroom),
                ("great hall", hall::Style::Castle { dais: 1, house, banquet: true }),
                ("audience chamber", hall::Style::Presence { house }),
                ("long gallery", hall::Style::Gallery),
                ("royal chapel", hall::Style::Chapel),
                ("chapel", hall::Style::Chapel),
            ]
        }
        Arch::Mansion if key == Some("palace") => {
            let rich = set.is_some_and(|s| s.tier == Tier::Metropolis || (s.tier == Tier::City && roll(0x43) % 3 != 0));
            let seating = if roll(0x44) % 3 == 0 { 2 } else { 0 };
            let house = house(false);
            vec![
                ("great hall", hall::Style::Castle { dais: seating + if rich { 0 } else { 3 }, house, banquet: true }),
                ("audience chamber", hall::Style::Presence { house }),
                ("long gallery", hall::Style::Gallery),
                ("chapel", hall::Style::Chapel),
            ]
        }
        Arch::Mansion => vec![("great hall", hall::Style::Estate { house: house(!castle) })],
        // (A temple's god's pieces by the god in its name; a big one's screen at its crossing.)
        Arch::Temple if key == Some("temple") && town::temple::shaped(b) => {
            let god = town::temple::god_look(b);
            match town::temple::cross_of(b) {
                Some(x) => vec![
                    ("nave", hall::Style::Nave { god, cross: Some((x.nave as u8, (x.nave + x.transept - 1) as u8)) }),
                    ("sanctuary", hall::Style::Sanctuary { god }),
                    ("side chapel", hall::Style::SideChapel { god }),
                ],
                None => vec![("nave", hall::Style::Nave { god, cross: None })],
            }
        }
        Arch::Temple if key == Some("shrine") => vec![("shrine", hall::Style::Shrine { god: town::temple::god_look(b) })],
        Arch::Temple if key == Some("monastery") => vec![("nave", hall::Style::Nave { god: town::temple::god_look(b), cross: None }), ("refectory", hall::Style::Refectory)],
        _ => Vec::new(),
    }
}

/// A hall's frame (`hall::Frame`) where its zones gave it none: the largest box of its squares, along
/// the axis from the wall its main door (the one nearest the way in: the front door, else the stairs)
/// is on to the opposite one, or, where that wall is the long one, along its length away from the
/// door's end (then the other way too, where the far end holds no dais: whether it may be turned).
/// None for a room with no box 3 by 2.
fn hall_frame(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4]) -> Option<(hall::Frame, bool)> {
    let mine = |i: usize, j: usize| lvl.cells[j * nx + i] == ri;
    let sq: Vec<usize> = (0..nx * ny).filter(|&k| lvl.cells[k] == ri).collect();
    if sq.is_empty() {
        return None;
    }
    let (x0, x1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
    let (y0, y1) = sq.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
    let main = main_door(lvl, nx, ny, ri, st);
    // The largest box (a summed table over the room's bounding box), then the one holding the door.
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut sum = vec![0usize; (bw + 1) * (bh + 1)];
    for j in 0..bh {
        for i in 0..bw {
            sum[(j + 1) * (bw + 1) + i + 1] = sum[j * (bw + 1) + i + 1] + sum[(j + 1) * (bw + 1) + i] - sum[j * (bw + 1) + i] + mine(x0 + i, y0 + j) as usize;
        }
    }
    let full = |a0: usize, a1: usize, b0: usize, b1: usize| sum[b1 * (bw + 1) + a1] + sum[b0 * (bw + 1) + a0] - sum[b0 * (bw + 1) + a1] - sum[b1 * (bw + 1) + a0] == (a1 - a0) * (b1 - b0);
    let holds = |b: [usize; 4], p: (isize, isize)| p.0 >= (x0 + b[0]) as isize && p.0 < (x0 + b[1]) as isize && p.1 >= (y0 + b[2]) as isize && p.1 < (y0 + b[3]) as isize;
    let mut best: Option<((usize, bool), [usize; 4])> = None;
    for b0 in 0..bh {
        for b1 in b0 + 2..=bh {
            for a0 in 0..bw {
                for a1 in a0 + 2..=bw {
                    if (a1 - a0).max(b1 - b0) < 3 || !full(a0, a1, b0, b1) {
                        continue;
                    }
                    let score = ((a1 - a0) * (b1 - b0), main.is_some_and(|m| holds([a0, a1, b0, b1], m.1)));
                    if best.as_ref().is_none_or(|b| score > b.0) {
                        best = Some((score, [a0, a1, b0, b1]));
                    }
                }
            }
        }
    }
    let (_, [a0, a1, b0, b1]) = best?;
    let (bx0, bx1, by0, by1) = ((x0 + a0) as isize, (x0 + a1) as isize - 1, (y0 + b0) as isize, (y0 + b1) as isize - 1);
    let (w, h) = ((bx1 - bx0 + 1) as usize, (by1 - by0 + 1) as usize);
    // The door's side of the box: the one it lies nearest (0 top, 1 right, 2 bottom, 3 left).
    let c = main.map_or([(bx0 + bx1) as f64 * 0.5, (by0 + by1) as f64 * 0.5], |m| [m.1.0 as f64, m.1.1 as f64]);
    let gaps = [c[1] - by0 as f64, bx1 as f64 - c[0], by1 as f64 - c[1], c[0] - bx0 as f64];
    let door_side = (0..4).min_by(|&a, &b| gaps[a].total_cmp(&gaps[b]).then(a.cmp(&b))).unwrap_or(3);
    let (across, deep) = if door_side % 2 == 0 { (w, h) } else { (h, w) };
    // (Entered on a long wall: along the length, from the end nearer the door.)
    let from = if main.is_none() || across <= deep {
        door_side
    } else if door_side % 2 == 0 {
        if c[0] - bx0 as f64 <= bx1 as f64 - c[0] { 3 } else { 1 }
    } else if c[1] - by0 as f64 <= by1 as f64 - c[1] {
        0
    } else {
        2
    };
    let f = match from {
        0 => hall::Frame { o: (bx0, by0), ds: (0, 1), dt: (1, 0), len: h, wid: w },
        2 => hall::Frame { o: (bx0, by1), ds: (0, -1), dt: (1, 0), len: h, wid: w },
        3 => hall::Frame { o: (bx0, by0), ds: (1, 0), dt: (0, 1), len: w, wid: h },
        _ => hall::Frame { o: (bx1, by0), ds: (-1, 0), dt: (0, 1), len: w, wid: h },
    };
    Some((f, from != door_side))
}

/// A room's main door (`hall_frame`): rooms by doors from the front door's room (or the stairs'),
/// room `ri`'s door to the nearest (the front door itself first); its square in the room (grid) and
/// the door (its index in `lvl.doors`).
pub fn main_door(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4]) -> Option<(usize, (isize, isize), usize)> {
    let mine = |i: usize, j: usize| lvl.cells[j * nx + i] == ri;
    let n = lvl.rooms.len();
    let start = lvl.doors.iter().find(|d| d.kind == "front").map(|d| d.rooms[0]).or_else(|| lvl.has_stairs.then(|| lvl.cells[st[1] * nx + st[0]])).unwrap_or(ri);
    let mut hops = vec![usize::MAX; n];
    if start >= 0 {
        hops[start as usize] = 0;
        let mut q = std::collections::VecDeque::from([start]);
        while let Some(a) = q.pop_front() {
            for d in lvl.doors.iter().filter(|d| d.rooms.contains(&a)) {
                let o = if d.rooms[0] == a { d.rooms[1] } else { d.rooms[0] };
                if o >= 0 && hops[o as usize] == usize::MAX {
                    hops[o as usize] = hops[a as usize] + 1;
                    q.push_back(o);
                }
            }
        }
    }
    let door_sq = |d: &Door| {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let pair = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        pair.into_iter().find(|&(i, j)| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && mine(i as usize, j as usize))
    };
    lvl.doors
        .iter()
        .enumerate()
        .filter(|(_, d)| d.rooms.contains(&ri))
        .filter_map(|(i, d)| {
            let o = if d.rooms[0] == ri { d.rooms[1] } else { d.rooms[0] };
            let rank = if d.kind == "front" { 0 } else if o >= 0 { hops[o as usize].saturating_add(1) } else { usize::MAX };
            door_sq(d).map(|p| (rank, p, i))
        })
        .min_by_key(|x| (x.0, x.1.1, x.1.0))
}

/// A chapel's frame (`hall::Frame`): the largest box of its squares (as `hall_frame` finds it), along
/// its length, its far end (the altar's) the one away from its way in (`way_in`), else the one more
/// to the east (`u`, `v`: the grid's axes in the world), either where it runs north and south, run on
/// to the far wall where the room does past the box on the axis (on `whole` squares), else short of
/// a far row the outer wall cuts into there; it may be turned (the altar at the other end).
#[allow(clippy::too_many_arguments)]
fn chapel_frame(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4], u: P, v: P, whole: Option<&[bool]>) -> Option<(hall::Frame, bool)> {
    let (f, _) = hall_frame(lvl, nx, ny, ri, st)?;
    let (a, c) = (f.grid(0, 0), f.grid(f.len as isize - 1, f.wid as isize - 1));
    let (bx0, bx1, by0, by1) = (a.0.min(c.0), a.0.max(c.0), a.1.min(c.1), a.1.max(c.1));
    let (w, h) = ((bx1 - bx0 + 1) as usize, (by1 - by0 + 1) as usize);
    let east = |d: (isize, isize)| d.0 as f64 * u[0] + d.1 as f64 * v[0];
    let f = match (w >= h, w >= h && east((1, 0)) >= 0.0 || w < h && east((0, 1)) >= 0.0) {
        (true, true) => hall::Frame { o: (bx0, by0), ds: (1, 0), dt: (0, 1), len: w, wid: h },
        (true, false) => hall::Frame { o: (bx1, by0), ds: (-1, 0), dt: (0, 1), len: w, wid: h },
        (false, true) => hall::Frame { o: (bx0, by0), ds: (0, 1), dt: (1, 0), len: h, wid: w },
        (false, false) => hall::Frame { o: (bx0, by1), ds: (0, -1), dt: (1, 0), len: h, wid: w },
    };
    // (Its way in nearer the far end: the other way round.)
    let mut f = f;
    if way_in(lvl, nx, ny, ri, st, f).is_some_and(|m| m > 0.5 * (f.len as f64 - 1.0) + 0.01) {
        f = hall::Frame { o: f.grid(f.len as isize - 1, 0), ds: (-f.ds.0, -f.ds.1), ..f };
    }
    // (On to its far wall where the room runs on past the box on the axis, a row or two, on squares
    // the outer wall leaves whole (`whole`), so the altar stands against it.)
    let w = f.wid as isize;
    let (cl, cr) = if w % 2 == 1 { (w / 2, w / 2) } else { (w / 2 - 1, w / 2) };
    let ours = |p: (isize, isize)| p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < nx && (p.1 as usize) < ny && {
        let k = p.1 as usize * nx + p.0 as usize;
        lvl.cells[k] == ri && whole.is_none_or(|w| w[k])
    };
    let mut ran = false;
    for _ in 0..2 {
        if !(cl..=cr).all(|t| ours(f.grid(f.len as isize, t))) {
            break;
        }
        (f.len, ran) = (f.len + 1, true);
    }
    // (Else short of its far row where the outer wall cuts into it on the axis: against that.)
    let mine = |p: (isize, isize)| p.0 >= 0 && p.1 >= 0 && (p.0 as usize) < nx && (p.1 as usize) < ny && lvl.cells[p.1 as usize * nx + p.0 as usize] == ri;
    while !ran && f.len > 3 && (cl..=cr).all(|t| mine(f.grid(f.len as isize - 1, t)) && !ours(f.grid(f.len as isize - 1, t))) {
        f.len -= 1;
    }
    Some((f, true))
}

/// Where along frame `f` room `ri` is entered (its `s`: a door in a side wall at its square's, one
/// in an end wall half a square beyond it): at the end whose wall a door opens in, where only one
/// end's does; else at its main door (`main_door`); else (none, or that midway along it) at its
/// doors' on the mean. None for a room with no door.
pub fn way_in(lvl: &Level, nx: usize, ny: usize, ri: i16, st: [usize; 4], f: hall::Frame) -> Option<f64> {
    let at = |d: &Door| {
        let (x0, y0, x1) = (d.a[0].min(d.b[0]) as isize, d.a[1].min(d.b[1]) as isize, d.a[0].max(d.b[0]) as isize);
        let [p, q] = if x0 == x1 { [(x0 - 1, y0), (x0, y0)] } else { [(x0, y0 - 1), (x0, y0)] };
        0.5 * (f.local(p.0, p.1).0 + f.local(q.0, q.1).0) as f64
    };
    let (l, mid) = (f.len as f64, 0.5 * (f.len as f64 - 1.0));
    let ends = |e: f64| lvl.doors.iter().any(|d| d.rooms.contains(&ri) && (at(d) - e).abs() < 0.01);
    match (ends(-0.5), ends(l - 0.5)) {
        (true, false) => return Some(-0.5),
        (false, true) => return Some(l - 0.5),
        _ => {}
    }
    if let Some(s) = main_door(lvl, nx, ny, ri, st).map(|m| at(&lvl.doors[m.2])).filter(|s| (s - mid).abs() > 0.01) {
        return Some(s);
    }
    let s: Vec<f64> = lvl.doors.iter().filter(|d| d.rooms.contains(&ri)).map(at).collect();
    (!s.is_empty()).then(|| s.iter().sum::<f64>() / s.len() as f64)
}

/// The frame room `room` of level `z` of building `b` was furnished in as a hall when generated
/// (where it is laid out by zones and its frame's middle square is still that room's in `lvl`: its
/// zones' frame, not one found in its squares), with the squares its furniture keeps to as then
/// (`Shell::whole_by`): the designer's Furnish gives what generation gives.
#[allow(clippy::too_many_arguments)]
pub(crate) fn zone_hall_frame(world: &World, t0: &T0, l: &Layout, settlement: usize, b: &town::Building, z: i8, lvl: &Level, room: i16, kind: &str) -> Option<(hall::Frame, Vec<bool>)> {
    hub_kind(t0, l, b)?;
    let sh = Shell::of(l, b);
    let (nx, ny) = (sh.nx, sh.ny);
    if lvl.cells.len() != nx * ny {
        return None;
    }
    let mut plans = Vec::new();
    let it = build_noting(world, t0, l, settlement, b, &mut Vec::new(), &mut plans);
    let ours = |f: &hall::Frame| {
        let (i, j) = f.grid(f.len as isize / 2, f.wid as isize / 2);
        i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && lvl.cells[j as usize * nx + i as usize] == room
    };
    let p = plans.iter().find(|p| it.levels[p.level].z == z && it.levels[p.level].rooms.get(p.room as usize).is_some_and(|r| r.kind == kind) && ours(&p.frame))?;
    Some((p.frame, sh.whole_by(-0.1)))
}

/// Stairs for an inn or tavern: a 2 × 1 block (else 1 × 1) in the public room (against one of its
/// walls, if `against`), on `whole` squares, off every doorway, as far from the front door as the
/// room allows.
fn pub_stairs(lvl: &Level, nx: usize, ny: usize, whole: &[bool], against: bool, by_doors: bool) -> Option<[usize; 4]> {
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
                // (`by_doors`: only the door's own squares.)
                if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny && (!by_doors || (di, dj) == (0, 0)) {
                    near_door[b as usize * nx + a as usize] = true;
                }
            }
        }
    }
    let ok = |i: usize, j: usize| i < nx && j < ny && lvl.cells[j * nx + i] == public && !near_door[j * nx + i] && whole[j * nx + i];
    let wall = |i: usize, j: usize| [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| {
        let (a, b) = (i as isize + di, j as isize + dj);
        a < 0 || b < 0 || a >= nx as isize || b >= ny as isize || lvl.cells[b as usize * nx + a as usize] != public
    });
    let mut best: Option<(f32, [usize; 4])> = None;
    let sizes: &[(usize, usize)] = if by_doors { &[(1, 1)] } else { &[(2, 1), (1, 2), (1, 1)] };
    for &(w, h) in sizes {
        for j in 0..ny {
            for i in 0..nx {
                let sq: Vec<(usize, usize)> = (j..j + h).flat_map(|b| (i..i + w).map(move |a| (a, b))).collect();
                if !sq.iter().all(|&(a, b)| ok(a, b)) || (against && !sq.iter().all(|&(a, b)| wall(a, b))) {
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

/// A floor laid out by zones (`hub::Plan::level`), finished: a room in each of the keep's towers
/// standing on it and in the grid's corner towers (`corner`: their side), of the kinds the plan
/// gave them, doors where its zones want them (`Hints`), windows (none onto open battlements,
/// walks or roofs).
#[allow(clippy::too_many_arguments)]
fn hub_level(laid: hub::Laid, sh: &Shell, nx: usize, ny: usize, st: [usize; 4], corner: usize, front: usize, ground: bool, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
    let hub::Laid { mut cells, mut rooms, hints, public, towers, corners, .. } = laid;
    let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
    for &(ti, kind) in &towers {
        let id = rooms.len() as i16;
        rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for &k in &sh.towers[ti].0 {
            if sh.inside[k] && !on_stairs(k) {
                cells[k] = id;
            }
        }
    }
    if corner > 0 && nx > 2 * corner && ny > 2 * corner {
        for (n, (cx, cy)) in [(0, 0), (nx - corner, 0), (0, ny - corner), (nx - corner, ny - corner)].into_iter().enumerate() {
            let id = rooms.len() as i16;
            let kind = corners.get(n).copied().unwrap_or(if ground { "guard post" } else { "tower room" });
            rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
            for j in cy..cy + corner {
                for i in cx..cx + corner {
                    if sh.inside[j * nx + i] && !on_stairs(j * nx + i) {
                        cells[j * nx + i] = id;
                    }
                }
            }
        }
    }
    let cells = merge_fragments(cells, nx, ny);
    let (cells, mut rooms, map) = compact_map(cells, rooms);
    finish_rooms(&mut rooms, &cells, nx);
    let hints = hints.renumbered(&map);
    let mut lvl = connect_with(cells, rooms, nx, ny, st, front, ground, public, ext_free, Some(&hints), rng);
    windows(&mut lvl, nx, ny, ground);
    lvl
}

/// Every room one can stand in reached from the stairs, crossing from room to room only through
/// doors.
fn reaches_all(lvl: &Level, nx: usize, ny: usize, st: [usize; 4]) -> bool {
    let k0 = st[1] * nx + st[0];
    if lvl.cells.get(k0).is_none_or(|&c| c < 0) {
        return false;
    }
    let door_between = |a: usize, b: usize| {
        lvl.doors.iter().any(|d| {
            let (x0, y0, x1) = (d.a[0].min(d.b[0]) as usize, d.a[1].min(d.b[1]) as usize, d.a[0].max(d.b[0]) as usize);
            let sq = if x0 == x1 { [(x0.wrapping_sub(1), y0), (x0, y0)] } else { [(x0, y0.wrapping_sub(1)), (x0, y0)] };
            let (pa, pb) = ((a % nx, a / nx), (b % nx, b / nx));
            (sq[0] == pa && sq[1] == pb) || (sq[0] == pb && sq[1] == pa)
        })
    };
    let mut seen = vec![false; nx * ny];
    let mut stack = vec![k0];
    seen[k0] = true;
    while let Some(k) = stack.pop() {
        let (i, j) = (k % nx, k / nx);
        for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
            if a >= nx || b >= ny {
                continue;
            }
            let q = b * nx + a;
            if seen[q] || lvl.cells[q] < 0 || (lvl.cells[q] != lvl.cells[k] && !door_between(k, q)) {
                continue;
            }
            seen[q] = true;
            stack.push(q);
        }
    }
    (0..lvl.rooms.len()).all(|r| !walkable(lvl.rooms[r].kind) || lvl.rooms[r].squares == 0 || (0..nx * ny).any(|k| lvl.cells[k] == r as i16 && seen[k]))
}

/// The tops of a keep's four corner towers: a battlemented square each (no main stairs), on
/// the footprint (a corner the footprint cuts off has no tower, nor its spiral stair); a keep
/// built of pieces, its own towers' (`towers`: their squares).
fn tower_tops(inside: &[bool], nx: usize, ny: usize, t: usize, towers: &[Vec<usize>], st: [usize; 4], rng: &mut Pcg32) -> Level {
    let mut cells = vec![-1i16; nx * ny];
    let mut rooms = Vec::new();
    for sq in towers {
        let id = rooms.len() as i16;
        rooms.push(Room { kind: "tower top", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for &k in sq {
            cells[k] = id;
        }
    }
    if !towers.is_empty() {
        finish_rooms(&mut rooms, &cells, nx);
        let mut lvl = connect(cells, rooms, nx, ny, st, 0, false, None, &|_, _| true, rng);
        lvl.has_stairs = false;
        return lvl;
    }
    for ((cx, cy), (ki, kj)) in [(0, 0), (nx - t, 0), (0, ny - t), (nx - t, ny - t)].into_iter().zip([(0, 0), (nx - 1, 0), (0, ny - 1), (nx - 1, ny - 1)]) {
        if !inside[kj * nx + ki] {
            continue;
        }
        let id = rooms.len() as i16;
        rooms.push(Room { kind: "tower top", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
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

/// A shrine's stairs down to its crypt: two squares along a side wall from the front wall (a corner
/// first, then a square or two in), so its far end holds the god's statue and the altar; None where
/// they fit nowhere there.
fn shrine_stairs(inside: &[bool], nx: usize, ny: usize, front: usize) -> Option<[usize; 4]> {
    let fits = |st: [usize; 4]| st[0] + st[2] <= nx && st[1] + st[3] <= ny && (st[1]..st[1] + st[3]).all(|j| (st[0]..st[0] + st[2]).all(|i| inside[j * nx + i]));
    // (Along the side walls: across the front's way in from it by `d`, at each end of it.)
    (0..3usize).flat_map(|d| [false, true].map(move |far| (d, far))).find_map(|(d, far)| {
        let st = match front {
            0 => [if far { nx.checked_sub(1)? } else { 0 }, d, 1, 2],
            1 => [nx.checked_sub(2 + d)?, if far { ny.checked_sub(1)? } else { 0 }, 2, 1],
            2 => [if far { nx.checked_sub(1)? } else { 0 }, ny.checked_sub(2 + d)?, 1, 2],
            _ => [d, if far { ny.checked_sub(1)? } else { 0 }, 2, 1],
        };
        fits(st).then_some(st)
    })
}

#[derive(Clone, Copy, Debug)]
struct R {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

#[allow(clippy::too_many_arguments)]
fn layout_level(inside: &[bool], nx: usize, ny: usize, prog: &LevelProgram, st: [usize; 4], front: usize, ground: bool, arch: Arch, towers: Option<&[(Vec<usize>, Vec<usize>)]>, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
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
        let mut rooms = vec![Room { kind: "stands", squares: 0, raise_ft: 5.0, center: [0.0; 2], shape: 0 }, Room { kind: "arena floor", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }];
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
        && let Some((cells, mut rooms)) = cell_block(inside, nx, ny, st, prog.rooms[0].0).filter(|(c, _)| rooms_whole(c, nx, ny))
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
        rooms.push(Room { kind: kinds[assign[li]].0, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
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
        rooms.push(Room { kind: "hall", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for i in h.x..h.x + h.w {
            if inside[h.y * nx + i] {
                cells[h.y * nx + i] = id;
            }
        }
    }
    rooms
    };
    // Keeps: a tower in each corner, a room of its own on every floor above the cellar (a keep
    // built of pieces: in each of its towers).
    let t = if arch == Arch::Keep && towers.is_none() { keep_tower_squares(nx, ny) } else { 0 };
    let ways: Vec<usize> = towers.unwrap_or_default().iter().flat_map(|t| t.1.iter().copied()).collect();
    let first_tower = rooms.len();
    let mut tower_ids: Vec<(i16, &Vec<usize>, &Vec<usize>)> = Vec::new();
    for (sq, doors) in towers.unwrap_or_default() {
        let kind: &'static str = if ground { "guard post" } else { "tower room" };
        let id = rooms.len() as i16;
        rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
        for &k in sq {
            let (i, j) = (k % nx, k / nx);
            if inside[k] && !(i >= st[0] && i < st[0] + st[2] && j >= st[1] && j < st[1] + st[3]) {
                cells[k] = id;
            }
        }
        // Its ways in are wall until the rooms round it are settled (below).
        for &k in doors {
            cells[k] = -1;
        }
        tower_ids.push((id, sq, doors));
    }
    if t > 0 {
        let kind: &'static str = if ground { "guard post" } else { "tower room" };
        for (cx, cy) in [(0, 0), (nx - t, 0), (0, ny - t), (nx - t, ny - t)] {
            let id = rooms.len() as i16;
            rooms.push(Room { kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
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
    // A keep built of pieces: a sliver a square wide left between its pieces' walls (and its
    // towers') is part of the room it opens off, not a room of its own.
    if towers.is_some() {
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        for r in 0..first_tower.min(rooms.len()) as i16 {
            let mine: Vec<usize> = (0..nx * ny).filter(|&k| cells[k] == r).collect();
            if mine.is_empty() || mine.len() > 8 || mine.iter().any(|&k| on_stairs(k)) {
                continue;
            }
            let (i0, i1) = mine.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k % nx), a.1.max(k % nx)));
            let (j0, j1) = mine.iter().fold((usize::MAX, 0), |a, &k| (a.0.min(k / nx), a.1.max(k / nx)));
            if i0 != i1 && j0 != j1 {
                continue;
            }
            let mut shared: Vec<(i16, usize)> = Vec::new();
            for &k in &mine {
                let (i, j) = (k % nx, k / nx);
                for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                    if a < nx && b < ny {
                        let o = cells[b * nx + a];
                        if o >= 0 && o != r && (o as usize) < first_tower {
                            match shared.iter_mut().find(|x| x.0 == o) {
                                Some(x) => x.1 += 1,
                                None => shared.push((o, 1)),
                            }
                        }
                    }
                }
            }
            if let Some(&(o, _)) = shared.iter().max_by(|x, y| x.1.cmp(&y.1).then(y.0.cmp(&x.0))) {
                for &k in &mine {
                    cells[k] = o;
                }
            }
        }
    }
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
    // A keep's towers: each one's way in on this floor, through its wall from the room beside it
    // nearest the circulation (a hall or corridor, else a room beside one); where that room is no
    // nearer than that, a passage on from it to the nearest hall or corridor through the rooms
    // between (the way in theirs). Its other ways are wall here.
    if !tower_ids.is_empty() {
        let side = |k: usize| {
            let (i, j) = (k % nx, k / nx);
            [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().filter(|&(a, b)| a < nx && b < ny).map(move |(a, b)| b * nx + a)
        };
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        let mut carved = false;
        for &(id, sq, doors) in &tower_ids {
            let circ = |r: i16| r >= 0 && is_way_through(rooms[r as usize].kind);
            // How many rooms from the circulation each room is, by the walls they share.
            let mut hops: Vec<usize> = vec![usize::MAX; rooms.len()];
            let mut q: std::collections::VecDeque<usize> = (0..rooms.len()).filter(|&r| circ(r as i16) && cells.contains(&(r as i16))).collect();
            for &r in &q {
                hops[r] = 0;
            }
            while let Some(r) = q.pop_front() {
                for k in (0..nx * ny).filter(|&k| cells[k] == r as i16) {
                    for n in side(k) {
                        let o = cells[n];
                        if o >= 0 && (o as usize) < first_tower && hops[o as usize] == usize::MAX {
                            hops[o as usize] = hops[r] + 1;
                            q.push_back(o as usize);
                        }
                    }
                }
            }
            let opens = |k: usize| side(k).filter(|q| !ways.contains(q) && !sq.contains(q)).map(|q| cells[q]).find(|&r| r >= 0 && (r as usize) < first_tower);
            let rank = |k: usize| opens(k).map_or(usize::MAX, |r| hops[r as usize]);
            let Some(k) = doors.iter().copied().filter(|&k| side(k).any(|q| cells[q] == id) && opens(k).is_some()).min_by_key(|&k| (rank(k), k)) else { continue };
            let r = opens(k).unwrap_or(-1);
            let passage = (rank(k) >= 2).then(|| {
                let mut from = vec![usize::MAX; nx * ny];
                let mut q = std::collections::VecDeque::from([k]);
                from[k] = k;
                while let Some(c) = q.pop_front() {
                    if c != k && circ(cells[c]) {
                        let mut path = Vec::new();
                        let mut x = from[c];
                        while x != k {
                            path.push(x);
                            x = from[x];
                        }
                        return (path.len() <= 8).then_some((cells[c], path));
                    }
                    for n in side(c) {
                        if from[n] == usize::MAX && cells[n] >= 0 && (cells[n] as usize) < first_tower && !ways.contains(&n) && !on_stairs(n) {
                            from[n] = c;
                            q.push_back(n);
                        }
                    }
                }
                None
            });
            match passage.flatten() {
                Some((hall, path)) => {
                    for c in path.into_iter().chain([k]) {
                        cells[c] = hall;
                    }
                    carved = true;
                }
                None => cells[k] = r,
            }
        }
        if carved {
            cells = merge_fragments(cells, nx, ny);
        }
    }
    // On a floor of eight rooms or more with a hall or corridor (not a keep's), no room more than two
    // rooms from it by the walls they share: a passage carved to such a room from the nearest hall or
    // corridor square through the rooms between (theirs the hall's, 16 squares at most), the
    // farthest room first; a room the passage cuts in two keeps its largest piece, and any other
    // piece of 12 squares or more is a room of its own of the same kind (the rest made whole again).
    if arch != Arch::Keep && first_tower == rooms.len() {
        let circ = |rooms: &[Room], r: i16| r >= 0 && is_way_through(rooms[r as usize].kind);
        let side = |k: usize| {
            let (i, j) = (k % nx, k / nx);
            [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().filter(|&(a, b)| a < nx && b < ny).map(move |(a, b)| b * nx + a)
        };
        let on_stairs = |k: usize| (k % nx) >= st[0] && (k % nx) < st[0] + st[2] && (k / nx) >= st[1] && (k / nx) < st[1] + st[3];
        let mut carved = false;
        for _ in 0..8 {
            let live: Vec<bool> = (0..rooms.len()).map(|r| cells.contains(&(r as i16))).collect();
            if live.iter().filter(|&&l| l).count() < 8 || !(0..rooms.len()).any(|r| live[r] && circ(&rooms, r as i16)) {
                break;
            }
            let mut hops: Vec<usize> = vec![usize::MAX; rooms.len()];
            let mut q: std::collections::VecDeque<usize> = (0..rooms.len()).filter(|&r| live[r] && circ(&rooms, r as i16)).collect();
            for &r in &q {
                hops[r] = 0;
            }
            while let Some(r) = q.pop_front() {
                for k in (0..nx * ny).filter(|&k| cells[k] == r as i16) {
                    for n in side(k) {
                        let o = cells[n];
                        if o >= 0 && hops[o as usize] == usize::MAX {
                            hops[o as usize] = hops[r] + 1;
                            q.push_back(o as usize);
                        }
                    }
                }
            }
            let Some(far) = (0..rooms.len()).filter(|&r| hops[r] != usize::MAX && hops[r] >= 3).max_by_key(|&r| (hops[r], std::cmp::Reverse(r))) else { break };
            // From the far room's squares to the nearest square of a hall or corridor, turning as
            // little as it can (a step 10, a turn 3 more: straight runs, not a staircase).
            let state = |k: usize, d: usize| k * 5 + d;
            let mut cost = vec![u32::MAX; nx * ny * 5];
            let mut back = vec![usize::MAX; nx * ny * 5];
            let mut heap = std::collections::BinaryHeap::new();
            for k in (0..nx * ny).filter(|&k| cells[k] == far as i16) {
                cost[state(k, 4)] = 0;
                heap.push(std::cmp::Reverse((0u32, state(k, 4))));
            }
            let mut found = None;
            while let Some(std::cmp::Reverse((c, st_))) = heap.pop() {
                if c > cost[st_] {
                    continue;
                }
                let (k, d) = (st_ / 5, st_ % 5);
                if circ(&rooms, cells[k]) {
                    found = Some(st_);
                    break;
                }
                let (i, j) = (k % nx, k / nx);
                for (nd, (a, b)) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)].into_iter().enumerate() {
                    if a >= nx || b >= ny {
                        continue;
                    }
                    let n = b * nx + a;
                    if cells[n] < 0 || on_stairs(n) || cells[n] == far as i16 {
                        continue;
                    }
                    let nc = c + 10 + if d != 4 && d != nd { 3 } else { 0 };
                    if nc < cost[state(n, nd)] {
                        cost[state(n, nd)] = nc;
                        back[state(n, nd)] = st_;
                        heap.push(std::cmp::Reverse((nc, state(n, nd))));
                    }
                }
            }
            let Some(end_state) = found else { break };
            let end = end_state / 5;
            let mut path = Vec::new();
            let mut x = back[end_state];
            while x != usize::MAX && cells[x / 5] != far as i16 {
                path.push(x / 5);
                x = back[x];
            }
            if path.len() > 16 {
                break;
            }
            let hall = cells[end];
            let mut cut: Vec<i16> = Vec::new();
            for c in path {
                if !cut.contains(&cells[c]) {
                    cut.push(cells[c]);
                }
                cells[c] = hall;
            }
            // (Each room cut: its pieces, the largest kept; the others, with those of other rooms
            // they touch, a room of their own of the biggest one's kind where 12 squares or more.)
            let mut off: Vec<(i16, Vec<usize>)> = Vec::new();
            for r in cut {
                let mut pieces: Vec<Vec<usize>> = Vec::new();
                let mut seen = vec![false; nx * ny];
                for k0 in (0..nx * ny).filter(|&k| cells[k] == r) {
                    if seen[k0] {
                        continue;
                    }
                    seen[k0] = true;
                    let mut piece = vec![k0];
                    let mut i = 0;
                    while i < piece.len() {
                        for n in side(piece[i]) {
                            if cells[n] == r && !seen[n] {
                                seen[n] = true;
                                piece.push(n);
                            }
                        }
                        i += 1;
                    }
                    pieces.push(piece);
                }
                pieces.sort_by_key(|p| (std::cmp::Reverse(p.len()), p[0]));
                off.extend(pieces.into_iter().skip(1).map(|p| (r, p)));
            }
            let mut group: Vec<usize> = (0..off.len()).collect();
            let touch = |a: &[usize], b: &[usize]| a.iter().any(|&k| side(k).any(|n| b.contains(&n)));
            for i in 0..off.len() {
                for j in i + 1..off.len() {
                    if touch(&off[i].1, &off[j].1) {
                        let (gi, gj) = (group[i], group[j]);
                        for g in group.iter_mut() {
                            if *g == gj {
                                *g = gi;
                            }
                        }
                    }
                }
            }
            for g in 0..off.len() {
                let members: Vec<usize> = (0..off.len()).filter(|&i| group[i] == g).collect();
                if members.iter().map(|&i| off[i].1.len()).sum::<usize>() < 12 {
                    continue;
                }
                let big = members.iter().copied().max_by_key(|&i| (off[i].1.len(), std::cmp::Reverse(i))).unwrap_or(g);
                let id = rooms.len() as i16;
                let from = off[big].0 as usize;
                rooms.push(Room { kind: rooms[from].kind, squares: 0, raise_ft: rooms[from].raise_ft, center: [0.0; 2], shape: 0 });
                for &i in &members {
                    for &k in &off[i].1 {
                        cells[k] = id;
                    }
                }
            }
            carved = true;
        }
        if carved {
            cells = merge_fragments(cells, nx, ny);
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
    let mut rooms = vec![Room { kind: "hall", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }];
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
        rooms.push(Room { kind: prog.rooms[0].0, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
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
        rooms.push(Room { kind: kind_of[pi], squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
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
    let mut rooms = vec![Room { kind: strip_kind, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }, Room { kind: "corridor", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }];
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
                rooms.push(Room { kind: "cell", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
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

/// Every room is one piece (its squares joined side to side): a cell block laid on a footprint it
/// doesn't fit (a keep drawn round its towers by hand) can leave a corridor in two.
fn rooms_whole(cells: &[i16], nx: usize, ny: usize) -> bool {
    let mut seen = vec![false; nx * ny];
    let mut rooms_seen: Vec<i16> = Vec::new();
    for k0 in 0..nx * ny {
        if cells[k0] < 0 || seen[k0] {
            continue;
        }
        if rooms_seen.contains(&cells[k0]) {
            return false;
        }
        rooms_seen.push(cells[k0]);
        let mut stack = vec![k0];
        seen[k0] = true;
        while let Some(k) = stack.pop() {
            let (i, j) = (k % nx, k / nx);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < nx && b < ny && !seen[b * nx + a] && cells[b * nx + a] == cells[k0] {
                    seen[b * nx + a] = true;
                    stack.push(b * nx + a);
                }
            }
        }
    }
    true
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
    let (cells, rooms, _) = compact_map(cells, rooms);
    (cells, rooms)
}

/// `compact`, with each old room's new index (-1 gone).
fn compact_map(cells: Vec<i16>, rooms: Vec<Room>) -> (Vec<i16>, Vec<Room>, Vec<i16>) {
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
    (cells.into_iter().map(|c| if c >= 0 { map[c as usize] } else { -1 }).collect(), out, map)
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

/// Where a floor laid out by zones (`hub`) wants its doors (rooms by index into the floor's rooms).
#[derive(Clone, Debug, Default)]
pub struct Hints {
    /// A door between two rooms on this edge (x, y, vertical: the edge between square (x, y) and
    /// the one to its right, or below it), where they share it.
    pub doors: Vec<((i16, i16), (usize, usize, bool))>,
    /// A room opening onto this circulation room, not onto the one it shares the most wall with.
    pub onto: Vec<(i16, i16)>,
    /// The front door: into this room, through the first of these outside edges (side 0 top, 1
    /// right, 2 bottom, 3 left; square x, y) onto open ground, else anywhere on its outside walls.
    pub front: Option<(i16, Vec<(usize, usize, usize)>)>,
    /// Rooms that get a door between them whatever else does (the way to a passage that meets no
    /// other).
    pub link: Vec<(i16, i16)>,
    /// Open archways onto the street (doors of kind "arch", no leaves): from this room through these
    /// outside edges (as `front`'s), where they open onto open ground (a coaching inn's carriage arch).
    pub arch: Vec<(i16, Vec<(usize, usize, usize)>)>,
}

impl Hints {
    /// The hints with rooms renumbered (`map`: old index to new, -1 gone).
    fn renumbered(&self, map: &[i16]) -> Hints {
        let m = |r: i16| if r >= 0 && (r as usize) < map.len() { map[r as usize] } else { -1 };
        Hints {
            doors: self.doors.iter().map(|&((a, b), e)| ((m(a).min(m(b)), m(a).max(m(b))), e)).filter(|((a, _), _)| *a >= 0).collect(),
            onto: self.onto.iter().map(|&(a, b)| (m(a), m(b))).filter(|&(a, b)| a >= 0 && b >= 0).collect(),
            front: self.front.as_ref().map(|(r, e)| (m(*r), e.clone())).filter(|(r, _)| *r >= 0),
            link: self.link.iter().map(|&(a, b)| (m(a).min(m(b)), m(a).max(m(b)))).filter(|&(a, b)| a >= 0 && a != b).collect(),
            arch: self.arch.iter().map(|(r, e)| (m(*r), e.clone())).filter(|(r, _)| *r >= 0).collect(),
        }
    }
}

/// Doors (a spanning tree over room adjacency from the entrance or stairs, plus a few loops),
/// the front door, and the wall runs. Rooms one can't stand in (`walkable`) get no doors.
#[allow(clippy::too_many_arguments)]
fn connect(cells: Vec<i16>, rooms: Vec<Room>, nx: usize, ny: usize, st: [usize; 4], front: usize, ground: bool, public: Option<&str>, ext_free: &dyn Fn(isize, isize) -> bool, rng: &mut Pcg32) -> Level {
    connect_with(cells, rooms, nx, ny, st, front, ground, public, ext_free, None, rng)
}

/// `connect`, with door hints.
#[allow(clippy::too_many_arguments)]
fn connect_with(cells: Vec<i16>, rooms: Vec<Room>, nx: usize, ny: usize, st: [usize; 4], front: usize, ground: bool, public: Option<&str>, ext_free: &dyn Fn(isize, isize) -> bool, hints: Option<&Hints>, rng: &mut Pcg32) -> Level {
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
                if bnb >= 0 && bnb != a && !(rooms[a as usize].kind == "cell" && rooms[bnb as usize].kind == "cell") && walkable(rooms[a as usize].kind) && walkable(rooms[bnb as usize].kind) {
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
    let circ = |r: usize| is_circulation(rooms[r].kind) || public.is_some_and(|k| rooms[r].kind == k);
    let mut onto: std::collections::BTreeSet<(i16, i16)> = Default::default();
    let mut served = vec![false; n];
    if (0..n).any(circ) {
        for r in 0..n {
            if circ(r) {
                continue;
            }
            let best = shared.iter().filter(|&(&(a, b), _)| (a as usize == r && circ(b as usize)) || (b as usize == r && circ(a as usize))).max_by_key(|(_, e)| e.len()).map(|(k, _)| *k);
            // (Onto the one its hint names, where they meet.)
            let hinted = hints.and_then(|h| h.onto.iter().find(|o| o.0 as usize == r)).map(|o| (o.0.min(o.1), o.0.max(o.1))).filter(|k| shared.contains_key(k));
            if let Some(k) = hinted.or(best) {
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
        let forced = hints.is_some_and(|h| h.link.contains(&(a, b)));
        if !onto.contains(&(a, b)) && !in_tree && !forced && rng.next_f64() > loops {
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
        // (Where its hint puts it, if the rooms meet there off the stairs.)
        let hinted = hints.and_then(|h| h.doors.iter().find(|d| d.0 == (a, b) && clear.contains(&&d.1))).map(|d| d.1);
        let (x, y, vert) = hinted.unwrap_or((x, y, vert));
        door_edges.insert((x, y, vert));
        let (p, q) = edge_seg(x, y, vert);
        doors.push(Door { a: p, b: q, kind: "door", rooms: [a, b], lock: 0 });
    }
    // A room still more than one room from the circulation (here a great or mess hall, and a
    // guardroom, the guards' way through a keep, count too) gets a door to its neighbour
    // nearest it (rooms between two others, corner towers).
    let hub = |r: usize| circ(r) || is_way_through(rooms[r].kind);
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
            // (A room that can't be brought nearer itself has the room it is reached through brought
            // nearer, or the one that is reached through, on toward the way in.)
            let nearer = |r: usize| {
                shared
                    .iter()
                    .filter_map(|(&(a, b), e)| {
                        let o = if a as usize == r { b } else if b as usize == r { a } else { return None };
                        (hops[o as usize] + 1 < hops[r]).then_some(((a, b), hops[o as usize], e))
                    })
                    .min_by_key(|x| x.1)
            };
            let deep = (0..n).filter(|&r| hops[r] != usize::MAX && hops[r] >= 3).find_map(|r| {
                nearer(r).or_else(|| {
                    let mut x = r;
                    while hops[x] >= 3 {
                        x = doors
                            .iter()
                            .filter_map(|d| {
                                let [a, b] = d.rooms;
                                let o = if a == x as i16 { b } else if b == x as i16 { a } else { return None };
                                (o >= 0 && hops[o as usize] + 1 == hops[x]).then_some(o as usize)
                            })
                            .min()?;
                        if let Some(e) = nearer(x) {
                            return Some(e);
                        }
                    }
                    None
                })
            });
            let Some(((a, b), _, edges)) = deep else { break };
            let &(x, y, vert) = &edges[edges.len() / 2];
            if door_edges.insert((x, y, vert)) {
                let (p, q) = edge_seg(x, y, vert);
                doors.push(Door { a: p, b: q, kind: "door", rooms: [a, b], lock: 0 });
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
        // (Into the room its hint names, through the first edge it names that is open ground, if
        // it can.)
        let into = hints.and_then(|h| h.front.as_ref()).filter(|(r, _)| cands.iter().any(|c| c.3 == *r));
        let named = into.and_then(|(r, edges)| edges.iter().find_map(|e| cands.iter().find(|c| c.3 == *r && (c.0, c.1, c.2) == *e).copied()));
        let cands: Vec<(usize, usize, usize, i16)> = match into {
            Some((r, _)) => cands.into_iter().filter(|c| c.3 == *r).collect(),
            None => cands,
        };
        if let Some(best) = cands.iter().max_by(|x, y| rank(x).cmp(&rank(y)).then(y.1.cmp(&x.1)).then(y.2.cmp(&x.2))).copied() {
            let mine: Vec<&(usize, usize, usize, i16)> = cands.iter().filter(|c| c.3 == best.3 && c.0 == best.0).collect();
            let &(side, i, j, r) = named.as_ref().unwrap_or(mine[mine.len() / 2]);
            let (p, q) = match side {
                0 => ([i as f32, j as f32], [(i + 1) as f32, j as f32]),
                1 => ([(i + 1) as f32, j as f32], [(i + 1) as f32, (j + 1) as f32]),
                2 => ([i as f32, (j + 1) as f32], [(i + 1) as f32, (j + 1) as f32]),
                _ => ([i as f32, j as f32], [i as f32, (j + 1) as f32]),
            };
            doors.push(Door { a: p, b: q, kind: "front", rooms: [r, -1], lock: 0 });
        }
        // (Open archways onto the street: each edge named, its room's, onto open ground.)
        for (r, edges) in hints.map_or(&[][..], |h| &h.arch[..]) {
            for &(side, i, j) in edges {
                let (di, dj) = [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)][side.min(3)];
                if at(i as isize, j as isize) != *r || at(i as isize + di, j as isize + dj) >= 0 || !ext_free(i as isize + di, j as isize + dj) {
                    continue;
                }
                let (p, q) = match side {
                    0 => ([i as f32, j as f32], [(i + 1) as f32, j as f32]),
                    1 => ([(i + 1) as f32, j as f32], [(i + 1) as f32, (j + 1) as f32]),
                    2 => ([i as f32, (j + 1) as f32], [(i + 1) as f32, (j + 1) as f32]),
                    _ => ([i as f32, j as f32], [i as f32, (j + 1) as f32]),
                };
                doors.push(Door { a: p, b: q, kind: "arch", rooms: [*r, -1], lock: 0 });
            }
        }
    }
    let walls = wall_runs(&cells, &rooms, nx, ny, &doors);
    Level { z: 0, name: String::new(), elevation_ft: 0.0, cells, rooms, walls, doors, windows: Vec::new(), furniture: Vec::new(), roof: false, has_stairs: true, natural: false, paths: Vec::new(), links: Vec::new() }
}

/// Wall runs: every square edge between different rooms (or a room and the outside) that is not
/// a door, merged along straight lines; where a hall's height left open (`OPEN_BELOW`) meets
/// circulation (a gallery, a passage, a landing), a railing; none along a roof's slopes inside
/// the walls (their eaves).
pub fn wall_runs(cells: &[i16], rooms: &[Room], nx: usize, ny: usize, doors: &[Door]) -> Vec<Wall> {
    let at = |i: isize, j: isize| if i < 0 || j < 0 || i >= nx as isize || j >= ny as isize { -1 } else { cells[j as usize * nx + i as usize] };
    let kind = |r: i16| rooms.get(r as usize).map_or("", |r| r.kind);
    // (A coaching inn's gallery railed onto its yard open to the sky.)
    let rail = |a: i16, b: i16| (a >= 0 && b >= 0 && railed(&rooms[a as usize], &rooms[b as usize])) || ((a < 0) != (b < 0) && kind(a.max(b)) == "yard gallery");
    let eave = |a: i16, b: i16| a >= 0 && b >= 0 && (kind(a) == ROOF_SLOPE || kind(b) == ROOF_SLOPE);
    let mut h_edges: Vec<(usize, usize, bool, bool)> = Vec::new(); // (x, y line, exterior, rail) horizontal unit edges at y
    let mut v_edges: Vec<(usize, usize, bool, bool)> = Vec::new(); // (x line, y, exterior, rail)
    let is_door = |a: [f32; 2], b: [f32; 2]| doors.iter().any(|d| (d.a == a && d.b == b) || (d.a == b && d.b == a));
    for j in 0..=ny {
        for i in 0..nx {
            let (up, dn) = (at(i as isize, j as isize - 1), at(i as isize, j as isize));
            if up != dn && (up >= 0 || dn >= 0) && !eave(up, dn) && !is_door([i as f32, j as f32], [(i + 1) as f32, j as f32]) {
                h_edges.push((i, j, up < 0 || dn < 0, rail(up, dn)));
            }
        }
    }
    for i in 0..=nx {
        for j in 0..ny {
            let (lf, rt) = (at(i as isize - 1, j as isize), at(i as isize, j as isize));
            if lf != rt && (lf >= 0 || rt >= 0) && !eave(lf, rt) && !is_door([i as f32, j as f32], [i as f32, (j + 1) as f32]) {
                v_edges.push((i, j, lf < 0 || rt < 0, rail(lf, rt)));
            }
        }
    }
    let mut walls: Vec<Wall> = Vec::new();
    h_edges.sort_by_key(|e| (e.1, e.0));
    for e in &h_edges {
        if let Some(w) = walls.last_mut().filter(|w| w.a[1] == w.b[1] && w.b[1] == e.1 as f32 && w.b[0] == e.0 as f32 && w.exterior == e.2 && w.rail == e.3) {
            w.b[0] += 1.0;
        } else {
            walls.push(Wall { a: [e.0 as f32, e.1 as f32], b: [(e.0 + 1) as f32, e.1 as f32], exterior: e.2, rail: e.3, arcade: false });
        }
    }
    v_edges.sort_by_key(|e| (e.0, e.1));
    for e in &v_edges {
        if let Some(w) = walls.last_mut().filter(|w| w.a[0] == w.b[0] && w.b[0] == e.0 as f32 && w.b[1] == e.1 as f32 && w.exterior == e.2 && w.rail == e.3) {
            w.b[1] += 1.0;
        } else {
            walls.push(Wall { a: [e.0 as f32, e.1 as f32], b: [e.0 as f32, (e.1 + 1) as f32], exterior: e.2, rail: e.3, arcade: false });
        }
    }
    // (An arcade's street side: its outer wall along the line of its front door, wherever the
    // arcade is inside it, arches between piers (`Wall::arcade`): a run there cut where the arcade
    // ends along it, a stair hall's or a tower's wall beside it kept.)
    for d in doors.iter().filter(|d| d.kind == "front" && kind(d.rooms[0]) == "arcade") {
        let (r, flat) = (d.rooms[0], d.a[1] == d.b[1]);
        let mut out = Vec::with_capacity(walls.len());
        for w in walls.drain(..) {
            let on = w.exterior && !w.rail && if flat { w.a[1] == d.a[1] && w.b[1] == d.a[1] } else { w.a[0] == d.a[0] && w.b[0] == d.a[0] };
            if !on {
                out.push(w);
                continue;
            }
            let (x0, x1, y0, y1) = (w.a[0].min(w.b[0]) as isize, w.a[0].max(w.b[0]) as isize, w.a[1].min(w.b[1]) as isize, w.a[1].max(w.b[1]) as isize);
            let (lo, hi) = if flat { (x0, x1) } else { (y0, y1) };
            let open = |k: isize| if flat { at(k, y0 - 1) == r || at(k, y0) == r } else { at(x0 - 1, k) == r || at(x0, k) == r };
            let mut k = lo;
            while k < hi {
                let a = open(k);
                let mut e = k + 1;
                while e < hi && open(e) == a {
                    e += 1;
                }
                let (p, q) = if flat { ([k as f32, w.a[1]], [e as f32, w.a[1]]) } else { ([w.a[0], k as f32], [w.a[0], e as f32]) };
                out.push(Wall { a: p, b: q, exterior: true, rail: false, arcade: a });
                k = e;
            }
        }
        walls = out;
    }
    walls

}

/// Windows on exterior walls above ground: every third square along a run, off doors; none onto
/// open battlements, a walk or a roof (a keep's lower pieces' tops beside a storey).
fn windows(lvl: &mut Level, nx: usize, ny: usize, _ground: bool) {
    let mut out = Vec::new();
    let open = |x: isize, y: isize| {
        x >= 0 && y >= 0 && (x as usize) < nx && (y as usize) < ny && lvl.cells[y as usize * nx + x as usize] >= 0 && matches!(lvl.rooms[lvl.cells[y as usize * nx + x as usize] as usize].kind, "battlements" | "wall walk" | ROOF_SLOPE)
    };
    for w in &lvl.walls {
        if !w.exterior || w.arcade {
            continue;
        }
        let len = ((w.b[0] - w.a[0]).abs() + (w.b[1] - w.a[1]).abs()) as usize;
        // (f32::signum(0.0) is 1.0: compare explicitly.)
        let dir = |d: f32| if d > 0.0 { 1.0 } else if d < 0.0 { -1.0 } else { 0.0 };
        let (dx, dy) = (dir(w.b[0] - w.a[0]), dir(w.b[1] - w.a[1]));
        let mut k = 1;
        while k + 1 < len {
            let p = [w.a[0] + dx * k as f32, w.a[1] + dy * k as f32];
            // (Not where the square inside is an open top: a run may go on from a storey's wall to
            // a platform's parapet.)
            let (x, y) = (p[0].min(p[0] + dx) as isize, p[1].min(p[1] + dy) as isize);
            let sq = if dx == 0.0 { [(x - 1, y), (x, y)] } else { [(x, y - 1), (x, y)] };
            if !sq.iter().any(|&(i, j)| open(i, j)) {
                out.push([p[0], p[1], p[0] + dx, p[1] + dy]);
            }
            k += 3;
        }
    }
    lvl.windows = out;
}

/// Pieces a room must have (drinking rooms: a bar and two tables), tried in smaller sizes
/// and anywhere in the room when the preferred spot is not free.
fn required(kind: &str, item: &str) -> bool {
    matches!(
        (kind, item),
        ("common room" | "taproom", "bar" | "table")
            | ("council chamber", "council_table")
            | ("job-board room", "job_board")
            | ("counting house", "counting_desk")
            | ("strongroom" | "warded vault", "strongbox")
            | ("stables", "stable_stall")
            | ("snug", "table")
            | ("private dining room", "long_table")
            | ("best chamber", "bed")
    )
}

/// Inns, taverns and breweries: the public room (taproom) is where guests come in and go up.
fn is_pub(arch: Arch) -> bool {
    matches!(arch, Arch::Tavern | Arch::Inn | Arch::Brewery)
}

/// Furniture by room rules: wall pieces along walls, free pieces clear of them; never on the
/// stairs or a doorway (the squares either side of every door stay open), and never cutting
/// a room's doors and stairs (a spiral stair's too, by anything but a rug) off from each other. With `whole` (`Shell::whole_by`), nothing on a
/// square it leaves out, one the outer wall as drawn cuts (but a drinking room's bar and tables
/// where they fit nowhere else).
fn furnish(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], whole: Option<&[bool]>, rng: &mut Pcg32) {
    furnish_rooms(lvl, nx, ny, st, None, whole, &[], true, false, rng);
}

/// `furnish` for every room, or `only` one (round what is already there), but those in `skip` (halls
/// furnished on their own: `hall`); without `thrones` (a building neither a castle nor a palace), no
/// throne where a room's rules have one; with `open` (a temple's, a shrine's, a monastery's), every
/// square of a room no piece blocks reached from its ways in (`room_connected`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn furnish_rooms(lvl: &mut Level, nx: usize, ny: usize, st: [usize; 4], only: Option<i16>, whole: Option<&[bool]>, skip: &[i16], thrones: bool, open: bool, rng: &mut Pcg32) {
    let cut = |k: usize| whole.is_some_and(|w| !w[k]);
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
    // Pieces already in place (spiral stairs) stay, with a clear square around them (and a way to
    // them: `entry_squares`).
    let mut furniture: Vec<Item> = lvl.furniture.clone();
    for f in &furniture {
        for j in (f.y as usize).saturating_sub(1)..((f.y + f.h) as usize + 1).min(ny) {
            for i in (f.x as usize).saturating_sub(1)..((f.x + f.w) as usize + 1).min(nx) {
                taken[j * nx + i] = true;
            }
        }
    }
    for (ri, room) in lvl.rooms.iter().enumerate() {
        let ri = ri as i16;
        if only.is_some_and(|o| o != ri) || skip.contains(&ri) {
            continue;
        }
        let mine: Vec<(usize, usize)> = (0..nx * ny).filter(|&k| cells[k] == ri).map(|k| (k % nx, k / nx)).collect();
        if mine.is_empty() {
            continue;
        }
        let touches = |i: usize, j: usize| [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)].iter().any(|&(di, dj)| room_at(i as isize + di, j as isize + dj) != ri);
        let spiral = lvl.furniture.iter().any(|f| f.kind == "spiral_stair" && cells[f.y as usize * nx + f.x as usize] == ri);
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
        for &(kind, place, fixed, per) in furnishing(room.kind).iter().filter(|r| thrones || r.0 != "throne") {
            let must = required(room.kind, kind);
            // (A council chamber too small for the council table: a long table and the chair of office.)
            if room.kind == "council chamber" && matches!(kind, "long_table" | "chair_of_office") && furniture[n0..].iter().any(|f| f.kind == "council_table") {
                continue;
            }
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
                    // (Off the squares the wall cuts, but a table a room must have.)
                    let none = |_: usize| false;
                    let skip: &dyn Fn(usize) -> bool = if must { &none } else { &cut };
                    let ok = if matches!(place, Place::Seated) {
                        seat_table(&cells, nx, ny, ri, must, &mut taken, &mut blocked, &mut furniture, rng, |b| room_connected(lvl, nx, ny, st, ri, b, open), skip)
                    } else {
                        booth(&cells, nx, ny, ri, &mut taken, &mut blocked, &mut furniture, rng, |b| room_connected(lvl, nx, ny, st, ri, b, open), skip)
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
                // (Pieces drawn whole at their own sizes: a council table with its seats 6 long in a
                // big room, 3 at the least; a desk, a board, a stall not shrunk.)
                let whole = matches!(kind, "council_table" | "counting_desk" | "job_board" | "stable_stall");
                if kind == "council_table" {
                    sizes = if mine.len() >= 40 { vec![(6, 2), (5, 2), (4, 2), (3, 2)] } else { vec![(4, 2), (3, 2)] };
                }
                if must && !whole {
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
                        // Random order; the first that keeps the room passable wins (off the squares
                        // the wall cuts, but a piece a room must have).
                        for k in (1..cands.len()).rev() {
                            cands.swap(k, rng.below(k as u32 + 1) as usize);
                        }
                        for (x, y, w, h) in cands {
                            if !must && (y..y + h).any(|j| (x..x + w).any(|i| cut(j * nx + i))) {
                                continue;
                            }
                            // (In a room with a spiral stair, nothing but a rug in the way to it, not
                            // even a bed one could step over.)
                            if blocks || (spiral && kind != "rug") {
                                for j in y..y + h {
                                    for i in x..x + w {
                                        blocked[j * nx + i] = true;
                                    }
                                }
                                if !room_connected(lvl, nx, ny, st, ri, &blocked, open) {
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
                            furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover, blocks_move: blocks, height_ft: height, hazard: None, sprite: 0, back: None, variant: None });
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
    furniture.push(Item { kind, name, x: x as u16, y: y as u16, w: 1, h: 1, cover, blocks_move: blocks, height_ft: height, hazard: None, sprite: 0, back: None, variant: None });
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
    skip: &dyn Fn(usize) -> bool,
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
            if skip(j * nx + i) {
                continue;
            }
            blocked[j * nx + i] = true;
            if !passable(blocked) {
                blocked[j * nx + i] = false;
                continue;
            }
            let chairs: Vec<(usize, usize)> = [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .map(|&(di, dj)| (i as isize + di, j as isize + dj))
                .filter(|&(a, b)| free(taken, a, b) && !skip(b as usize * nx + a as usize))
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
fn booth(cells: &[i16], nx: usize, ny: usize, ri: i16, taken: &mut [bool], blocked: &mut [bool], furniture: &mut Vec<Item>, rng: &mut Pcg32, passable: impl Fn(&[bool]) -> bool, skip: &dyn Fn(usize) -> bool) -> bool {
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
        if [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)].iter().any(|&(a, b)| skip(b * nx + a)) {
            continue;
        }
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

/// The squares inside a room next to its doorways, its stairs and a spiral stair's (its entry
/// points).
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
    // (A spiral stair in it: the way on to the floors above and below.)
    for f in lvl.furniture.iter().filter(|f| f.kind == "spiral_stair") {
        let k = f.y as usize * nx + f.x as usize;
        if lvl.cells[k] == ri {
            out.push(k);
        }
    }
    out
}

/// Whether all of a room's entry squares connect over squares not blocked by furniture (with `open`,
/// every square of it not blocked too).
fn room_connected(lvl: &Level, nx: usize, ny: usize, st: [usize; 4], ri: i16, blocked: &[bool], open: bool) -> bool {
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
    entries.iter().all(|&e| seen[e]) && (!open || (0..nx * ny).all(|k| seen[k] || lvl.cells[k] != ri || blocked[k]))
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
    let extra = l.extra_towers.iter().map(|&(t, gate)| (t, if gate { 15.0 } else { 11.0 }, gate));
    l.towers.iter().map(|&t| (t, 11.0, false)).chain(l.gate_towers.iter().map(|&t| (t, 15.0, true))).chain(extra).collect()
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
    let mut rng = Pcg32::new(hash3(world.stream("interior.tower"), town::rng_key(t0, settlement), k as i64, 0x70), 73);
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
    // (A square tower's grid along its sides.)
    let square = if gate { None } else { l.square_tower(at) };
    let u = square.or_else(|| dirs.first().copied()).unwrap_or([1.0, 0.0]);
    let v = [-u[1], u[0]];
    // A square grid centred on the tower (its centre in the middle of a square), reaching three
    // squares of wall walk beyond the tower each side.
    let half = (crate::core::ceil(r / SQUARE_FT) as usize) + 3;
    let n = 2 * half + 1;
    let origin = sub(at, mul(add(u, v), (half as f64 + 0.5) * SQUARE_FT));
    let centre = |i: usize, j: usize| add(origin, add(mul(u, (i as f64 + 0.5) * SQUARE_FT), mul(v, (j as f64 + 0.5) * SQUARE_FT)));
    let in_tower: Vec<bool> = (0..n * n)
        .map(|q| {
            let d = sub(centre(q % n, q / n), at);
            if square.is_some() { dot(d, u).abs() <= r && dot(d, v).abs() <= r } else { geom::dist(centre(q % n, q / n), at) <= r + 1.5 }
        })
        .collect();
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
    let pad = pad + crate::ground::shift_at(&world.file.edits, t0, at, pad);
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
        let mut rooms = vec![Room { kind: main, squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 }];
        for q in 0..n * n {
            if in_tower[q] {
                cells[q] = 0;
            }
        }
        if z == 1 {
            for wi in 0..dirs.len() {
                let id = rooms.len() as i16;
                if walk_of.iter().any(|&w| w == wi as i16) {
                    rooms.push(Room { kind: "wall walk", squares: 0, raise_ft: 0.0, center: [0.0; 2], shape: 0 });
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
        furnish(&mut lvl, n, n, stairs, None, &mut rng);
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
        sprites: Vec::new(),
        outlines: Vec::new(),
    })
}
