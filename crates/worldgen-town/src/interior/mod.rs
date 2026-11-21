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
//! Guarantees (checked in tests/vital/interiors.rs): at least two levels, every room reachable from the
//! front door, stairs aligned across levels, furniture never on a doorway or the stairs.

use serde::Serialize;

pub mod design;
mod felt;
pub mod hall;
pub mod height;
pub mod hub;
pub mod shape;
mod towerplan;
mod levels;
mod catalog;
mod plans;
mod shell;
mod builder;
mod frames;
mod rooms;
mod doors;
mod furnish;
mod winder;
pub use levels::*;
pub use catalog::*;
pub use plans::*;
pub use shell::*;
use builder::*;
pub use frames::*;
use rooms::*;
pub use doors::*;
pub(crate) use furnish::*;

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
    /// A city's sewers: the walkway's stonework (`under::sewer_stone`), 0 for the rest.
    #[serde(skip_serializing_if = "no_stone")]
    pub stone: u8,
    /// A cave's look from the biome at its mouth (`under::look_of`): 0 rock, 1 green, 2 dry, 3 cold.
    #[serde(skip_serializing_if = "no_stone")]
    pub look: u8,
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
    /// (Roadmap 7, 164) A U's or a court's yard (`town::wings`): its ranges' box (four corners, grid
    /// units); the ground in it outside the footprint is the yard, shown as open ground (as on the
    /// battlemap) on and above its ground floor. Empty for the rest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub yard: Vec<[f32; 2]>,
}

fn no_stone(v: &u8) -> bool {
    *v == 0
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
    /// The floor beyond the grid's edge (sewer sections: the squares of the neighbours within
    /// `under::SEWER_RIM`, from the streets alone), so a drawn wall runs on across the edge.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rim: Vec<[i16; 2]>,
    /// Ways to other sites from this level (a cellar trapdoor into the sewers, stairs down to a
    /// keep's deep dungeons, ladders up into cellars): each has a link back on the other side.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    /// Floor height per square (`height`; underground sites): one character per square (row-major, nx x ny),
    /// `'0'` + its height above `elevation_ft` in half feet (`'5'` a 2.5-ft dais, `':'` 5 ft, `'D'` 10 ft); empty
    /// where the whole level is flat. A rise of up to `height::STEP_FT` is a step, more a sheer edge (`Floor::sheer`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub floor: String,
    /// Ramp squares (indices into `cells`): rubble slopes joining two floors, at a height between them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ramps: Vec<u32>,
    /// Where the stairs come out on this level, if set by hand (a building design's: off its stair block; a site's: its way up, which
    /// sits there instead of under the way down above): play mode steps a token onto it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing: Option<[u16; 2]>,
}

/// A way from a square of one site to another site (`to`: its id).
#[derive(Serialize, Debug, Clone)]
pub struct Link {
    pub x: u16,
    pub y: u16,
    pub to: String,
    /// A link put down by hand (`Edits::links`): its id (`j:<id>`), and where it comes out.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub far: Option<Far>,
}

/// Where a link put down by hand comes out: the far site's level (index, bottom up) and square,
/// the far site's name, the link's own name (if it has one) and its way (`links::WAYS`).
#[derive(Serialize, Debug, Clone)]
pub struct Far {
    pub level: usize,
    pub x: u16,
    pub y: u16,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub way: &'static str,
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
    /// Locked: 0 not, else the colour of the lock (1 to `design::MAX_LOCK`), opened by the key
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

/// A room kind that is a flat roof walked on inside its parapet (`terrace`).
pub const TERRACE: &str = "roof terrace";

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
    /// A one-storey home's or workplace's rooms by its size (`layout_level`): the smallest homes
    /// one room, cottages a main room and a bedroom, bigger ones a kitchen too; a shop's storeroom
    /// left out on a small floor.
    by_size: bool,
}

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
    // (R5 M20: on a deck, at the deck's level.)
    let shift = if b.deck.is_some() { 0.0 } else { crate::ground::pad_shift(&world.file.edits, t0, &b.poly, b.pad_ft) };
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
    /// Squares (grid) no door opens beside, as the stairs' (a castle keep's private spiral stair).
    pub clear: Vec<usize>,
    /// Ways about the floor that get no door between them, though they meet (a castle keep's hall
    /// and the landing or the forebuilding's entrance hall beside its low end: reached by its
    /// screens passage).
    pub apart: Vec<(i16, i16)>,
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
            clear: self.clear.clone(),
            apart: self.apart.iter().map(|&(a, b)| (m(a).min(m(b)), m(a).max(m(b)))).filter(|&(a, b)| a >= 0 && a != b).collect(),
            arch: self.arch.iter().map(|(r, e)| (m(*r), e.clone())).filter(|(r, _)| *r >= 0).collect(),
        }
    }
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

pub fn generated_id(world: &World, t0: &T0, id: &str) -> Option<Interior> {
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
    // (`k` is its `t:` number: its place in the list unless the wall was edited by hand.)
    let (at, r, gate) = *towers(&l).get(l.tower_index(k)?)?;
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
    // (A D or an octagon's inside its own outline, its grid the way it faces: `towerplan`.)
    let shaped = if square.is_some() { None } else { towerplan::outline(&l, at, r, gate) };
    let u = square.or(shaped.as_ref().map(|s| s.0)).or_else(|| dirs.first().copied()).unwrap_or([1.0, 0.0]);
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
            if let Some((_, poly)) = &shaped {
                return towerplan::covers(poly, centre(q % n, q / n));
            }
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
    // (Roadmap 7 P8: a round tower's stair winds up the inside of its wall, `winder`.)
    let round = square.is_none() && shaped.is_none();
    let stairs = if round { [0, 0, 1, 1] } else { place_stairs(&in_tower, n, n, 0, Arch::Tower) };
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
        levels.push(lvl);
    }
    // (Its winding stair laid with every level's doors known, then the furniture.)
    let wind = if round { winder::plan(&levels.iter().collect::<Vec<_>>(), 0, levels.len(), &in_tower, n, n) } else { None };
    for (li, lvl) in levels.iter_mut().enumerate() {
        lvl.has_stairs &= !round;
        if let Some(w) = &wind {
            w.lay(lvl, n, n, li);
        }
        furnish(lvl, n, n, stairs, None, &mut rng);
    }
    Some(Interior {
        id: format!("t:{}:{}", l.index, k),
        settlement: settlement as u32,
        building: k as u32,
        name: None,
        theme: None,
        stone: 0,
        look: 0,
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
        yard: Vec::new(),
    })
}
