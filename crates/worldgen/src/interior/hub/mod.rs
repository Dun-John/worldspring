//! The zone engine for big interiors: a castle's keep (and a big barracks, a palace, a noble
//! estate, a temple, a monastery, a town hall, a guild, a big inn) laid out as real ones were, not as a grid of rooms. Its body is cut into zones along its
//! long axis, from the low end (where it is entered) to the high end: the service end (kitchen,
//! buttery, pantry), the screens passage across the body (the front door at one end of it), the
//! great hall (the body's full width when it is 6 to 9 squares across, else a strip of chambers
//! beside it, a mural passage along the outer wall when 4 deep), and the dais end (a parlour, the
//! landing with the main stair behind the high end of the hall, a wardrobe). Upper floors: a cross
//! passage under the landing and a mural passage on from it down one long wall (the other wall on
//! the next storey), the rest divided by cross walls into rooms of sensible proportions, each on
//! the passage or beside a room that is (`partition`); on the lord's floor the great chamber by the
//! landing, about twice as wide as it is deep, and the solar beside the landing or beyond the great
//! chamber's cross wall; the chapel, chambers, guest chambers, a treasury, an armory and a garrison
//! above, no storey on the squares of the one below. Over a hall open two storeys, its height left
//! open (`OPEN_BELOW`) with a minstrels' gallery over the screens end, reached along a mural
//! passage from the landing. Wings (an L or a T) hold chambers off a mural passage when 4 squares
//! deep or more; a forebuilding the entrance hall, a chapel above and a walled platform on top. A
//! great tower (or a small keep) has its hall on the first floor over stores, a well and the way in
//! to the stair; a shell keep is a ring of lean-tos round a round yard. Each piece rises as many
//! storeys as it has (a forebuilding 2, wings one fewer than the body, towers one more) and ends in
//! its top as the battlemap draws it (`town::keep_tops`): open battlements, or a parapet walk round
//! a roof (`ROOF_SLOPE`) that is one box, hipped. Towers are rooms of their own on every floor up
//! to theirs (guard posts and stores below, chambers and closets above, a garrison under the cone),
//! with a spiral stair at the middle (`super::hub_level`); doors go where the zones want them
//! (`Hints`). A tower keep too small for zones along it has a room or two a storey off a landing
//! across its high end (`small_plan`). Each storey above the hall is tidied (`tidy`: rooms over
//! 2.5:1 or 40 squares cut across, tails a square wide given away), its rooms of one size varied
//! (`vary`), its lord's rooms put by the main stair (`great_chamber`, `solar`, a tower's room where
//! the body has none: `lords_in_towers`), and every room given a door within a room of a way about
//! the floor (`near_links`).
//!
//! Zones along an axis, strips into wings and door hints are general (`kit`, the toolkit: cutting a
//! run into rooms of very different sizes, rooms of sound proportions, tidying a storey, doors that
//! keep every room within a room of a way about the floor); each kind of building is a programme of
//! its own on them: the hall house (`keep`: keeps of pieces and tower keeps; a barracks' mess hall,
//! `barracks`; noble estates and older palaces, `estate`), the shell keep (`shell`), the small tower
//! keep (`small`), the capital's palace, symmetric round its court (`palace`), a temple (a cross, or a
//! hall ending in its apse: `temple`), a monastery round its cloister (`monastery`), and a town hall
//! or a guildhall (its hall with its dais, the council chamber, strongroom and archive behind; a
//! city's town hall on an open arcade: `civic`), an inn or a caravanserai (its taproom across the
//! front, the kitchen, scullery and stores behind, guest floors off a corridor: `inn`).

use super::{Hints, Item, Room, Shell};
use crate::core::rng::{Pcg32, hash3};
use crate::town::{self, PartKind, geom::{self, P}};
use super::Level;

mod barracks;
mod civic;
mod estate;
mod inn;
/// The least ground-floor squares an inn is laid out by zones with (`inn::LEAST`).
pub const INN_LEAST: usize = inn::LEAST;
/// The least footprint a coaching inn stands round its yard on (`inn::YARD`).
pub const YARD: (usize, usize) = inn::YARD;
mod keep;
mod kit;
mod palace;
pub(crate) use palace::courted;
mod monastery;
mod shell;
mod small;
mod temple;

use kit::Lay;

/// The kinds of building laid out by zones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A hall keep: a long body with a forebuilding before its door, its hall on the ground floor.
    Hall,
    /// A great tower with a tower on each corner: its hall on the first floor.
    Great,
    /// A block with a wing at one end (its towers at its outer corners).
    L,
    /// A block with a wing from the middle of a long side.
    T,
    /// A round shell keep on a motte: lean-tos round an open yard.
    ShellKeep,
    /// A small tower keep of one piece, or a castle's keep redrawn by hand: as a great tower.
    Plain,
    /// A barracks of 150 squares or more: a mess hall at the heart, barrack rooms above.
    Barracks,
    /// The capital's palace: symmetric about its axis round a court (`palace`), its state rooms one
    /// into the next on the floor above.
    Palace,
    /// Any other city's palace: an older one round its great hall, a big keep without the defences
    /// (`keep`'s hall house, `estate`'s rooms).
    OldPalace,
    /// A small noble estate: a hall house, its hall between a service end and a parlour, chambers
    /// above.
    HallHouse,
    /// A big noble estate: the hall between a service wing and a cross wing for the family with its
    /// stair hall (a chapel in the richest).
    Manor,
    /// A big temple, a cross (`town::temple`): its nave with pillared aisles, the crossing, side
    /// chapels in the transept's arms, a vestry, the sanctuary in its apse (`temple`).
    Church,
    /// A small temple: a single hall with its apse (`temple`).
    TempleHall,
    /// A monastery round its cloister garth (`monastery`).
    Monastery,
    /// A town hall with its hall on the ground floor (`civic`).
    TownHall,
    /// A city's town hall on an open arcade, its council hall above (`civic`).
    ArcadeHall,
    /// The merchants', a craft, the adventurers' and the mages' guildhalls (`civic`).
    MerchantGuild,
    CraftGuild,
    AdventurersGuild,
    MagesGuild,
    /// An inn of `inn::LEAST` squares or more: its taproom across the front, the service range
    /// behind, guest floors off a corridor (`inn`).
    Inn,
    /// A caravanserai as big: an inn's programme with its stables (`inn`).
    Caravanserai,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Hall => "hall keep",
            Kind::Great => "great tower",
            Kind::L => "L keep",
            Kind::T => "T keep",
            Kind::ShellKeep => "shell keep",
            Kind::Plain => "tower keep",
            Kind::Barracks => "barracks",
            Kind::Palace => "palace",
            Kind::OldPalace => "great-hall palace",
            Kind::HallHouse => "hall house",
            Kind::Manor => "manor",
            Kind::Church => "cross-plan temple",
            Kind::TempleHall => "hall temple",
            Kind::Monastery => "monastery",
            Kind::TownHall => "town hall",
            Kind::ArcadeHall => "arcaded town hall",
            Kind::MerchantGuild => "merchants' guildhall",
            Kind::CraftGuild => "craft guildhall",
            Kind::AdventurersGuild => "adventurers' lodge",
            Kind::MagesGuild => "mages' conclave",
            Kind::Inn => "zoned inn",
            Kind::Caravanserai => "zoned caravanserai",
        }
    }

    /// A castle's keep or a barracks (the `hub` rules'), not a palace or a noble estate.
    pub fn is_keep(self) -> bool {
        !matches!(self, Kind::Palace | Kind::OldPalace | Kind::HallHouse | Kind::Manor | Kind::Church | Kind::TempleHall | Kind::Monastery) && !self.is_civic() && !self.is_inn()
    }

    /// An inn's or a caravanserai's (`inn`).
    pub fn is_inn(self) -> bool {
        matches!(self, Kind::Inn | Kind::Caravanserai)
    }

    /// A town hall's or a guild's (`civic`).
    pub fn is_civic(self) -> bool {
        matches!(self, Kind::TownHall | Kind::ArcadeHall | Kind::MerchantGuild | Kind::CraftGuild | Kind::AdventurersGuild | Kind::MagesGuild)
    }

    /// A temple's or a monastery's (`temple`, `monastery`).
    pub fn is_faith(self) -> bool {
        matches!(self, Kind::Church | Kind::TempleHall | Kind::Monastery)
    }
}

/// What kind of keep a castle's keep built of pieces is (`Building::parts`), as `town::castle`
/// laid it out (`Building::keep`); None for a building without pieces.
pub fn keep_kind(b: &town::Building) -> Option<Kind> {
    if b.parts.is_empty() {
        return None;
    }
    use town::KeepKind as K;
    Some(match b.keep? {
        K::Hall => Kind::Hall,
        K::Great => Kind::Great,
        K::L => Kind::L,
        K::T => Kind::T,
        K::Shell => Kind::ShellKeep,
        K::Plain => Kind::Plain,
    })
}

/// What kind of building `plan` lays out by zones, if any (`sh`: its grid):
/// a castle's keep (of pieces, else as a tower keep), a barracks of 150 squares or more, a palace (the
/// capital's, `capital`, symmetric round a court; another an older one round its great hall) and a
/// noble estate of `estate::LEAST` squares or more outside a castle (a hall house, a manor from
/// `estate::MANOR`) on a lot near enough a rectangle (`estate::FILL` of its grid: zones run along it),
/// a temple in pieces (`town::temple`: a cross, else a hall with its apse; a town's shrine too) and a monastery on a
/// rectangle big enough for its cloister (`monastery::fits`); a town hall or a guild big enough
/// (`civic::kind`; `city`: in a city or a metropolis), an inn or a caravanserai big enough
/// (`inn::kind`); `sh`: its grid.
pub fn kind_of(b: &town::Building, key: Option<&str>, sh: &Shell, capital: bool, city: bool) -> Option<Kind> {
    if matches!(key, Some("town_hall" | "merchant_guild" | "craft_guild" | "adventurers_guild" | "mages_guild")) {
        return civic::kind(b, key, sh, city);
    }
    if matches!(key, Some("inn" | "caravanserai")) {
        return inn::kind(b, key, sh);
    }
    let inside = &sh.inside;
    if matches!(key, Some("temple" | "shrine")) {
        return town::temple::shaped(b).then(|| if b.parts.iter().any(|p| p.kind == PartKind::Wing) { Kind::Church } else { Kind::TempleHall });
    }
    if key == Some("monastery") {
        return monastery::fits(b).then_some(Kind::Monastery);
    }
    let (n, inside) = (inside.len().max(1), inside.iter().filter(|&&i| i).count());
    let castle = key == Some("castle") || b.keep.is_some();
    // (An estate long enough for its zones along it (a service end, the screens, the hall, the dais
    // end), 8 squares by 5.)
    let fits = || {
        let o = geom::obb(&b.poly);
        o.long >= 8.0 * super::SQUARE_FT - 1.0 && o.short >= 5.0 * super::SQUARE_FT - 1.0
    };
    let estate = key.is_none() && town::catalog::RESIDENTIAL[b.residential as usize] == "noble estate" && b.ward != town::catalog::Ward::Castle && !castle && fits() && inside as f64 >= estate::FILL * n as f64;
    match keep_kind(b) {
        Some(k) => Some(k),
        None if key == Some("barracks") && !castle && inside >= 150 => Some(Kind::Barracks),
        None if castle => Some(Kind::Plain),
        None if key == Some("palace") => Some(if capital { Kind::Palace } else { Kind::OldPalace }),
        None if estate && inside >= estate::LEAST => Some(if inside >= estate::MANOR { Kind::Manor } else { Kind::HallHouse }),
        None => None,
    }
}

/// What a grid square is part of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Piece {
    Out,
    Body,
    Wing(u8),
    Fore,
    Tower,
}

/// The body's frame on the grid: square (s, t), s along its long axis from its low end, t across
/// it from its front side, is grid square `o + s * ds + t * dt`.
#[derive(Clone, Copy, Debug)]
struct Frame {
    o: (isize, isize),
    ds: (isize, isize),
    dt: (isize, isize),
    len: usize,
    wid: usize,
}

impl Frame {
    fn grid(&self, s: isize, t: isize) -> (isize, isize) {
        (self.o.0 + self.ds.0 * s + self.dt.0 * t, self.o.1 + self.ds.1 * s + self.dt.1 * t)
    }

    fn local(&self, i: isize, j: isize) -> (isize, isize) {
        let (di, dj) = (i - self.o.0, j - self.o.1);
        (di * self.ds.0 + dj * self.ds.1, di * self.dt.0 + dj * self.dt.1)
    }
}

/// A floor laid out by zones, before its towers and doors (`super::hub_level`).
pub struct Laid {
    pub cells: Vec<i16>,
    pub rooms: Vec<Room>,
    pub hints: Hints,
    /// The room kind that serves as circulation besides the passages (the hall on its floor).
    pub public: Option<&'static str>,
    pub name: &'static str,
    /// Open to the sky (the body's top): its outer wall a parapet.
    pub roof: bool,
    /// Pieces put down before the furniture (a shell keep's well).
    pub items: Vec<Item>,
    /// The keep's towers standing on this floor (indices into `Shell::towers`), with their rooms'
    /// kinds.
    pub towers: Vec<(usize, &'static str)>,
    /// The kinds of the rooms in a grid's corner towers (a barracks'), in `super::hub_level`'s order.
    pub corners: Vec<&'static str>,
    /// The frames of the floor's halls for their furnishing (`super::hall`): each from its low end to
    /// its far end (a hall's dais end, a throne room's dais).
    pub halls: Vec<super::hall::Frame>,
}

/// A piece of the body's top (s from `s0` to `s1`): its code (`town::keep_tops`: 0 open
/// battlements, else roofed inside a parapet walk) and whether its walk runs along its ends too.
#[derive(Clone, Copy, Debug)]
struct Top {
    s0: usize,
    s1: usize,
    code: u8,
    walk_lo: bool,
    walk_hi: bool,
}

/// A wing: its squares, along the joint `s0..s1` (body frame), outward `b` squares deep, on the
/// body's far side (t past its width) or its front side.
#[derive(Clone, Debug)]
struct Wing {
    squares: Vec<usize>,
    s0: usize,
    s1: usize,
    deep: usize,
    far: bool,
    /// Its rows (t, body frame): `t0..t1`.
    t0: isize,
    t1: isize,
}

/// A shell keep's ring of lean-tos round its round yard: the yard (the squares whose middles lie
/// in the circle of its box, `shape::ROUND`), the gate passage, the lean-tos' outer and inner
/// rows, the stairs' landing.
#[derive(Clone, Debug)]
struct Ring {
    yard: Vec<bool>,
    gate: Vec<bool>,
    outer: Vec<bool>,
    inner: Vec<bool>,
    landing: Vec<bool>,
    /// Each ring square's angle round from the gate (0 to 1).
    turn: Vec<f64>,
    centre: P,
    front: Vec<(usize, usize, usize)>,
}

/// How a building's floors are laid out by zones (`plan`).
pub struct Plan {
    pub kind: Kind,
    nx: usize,
    ny: usize,
    f: Frame,
    piece: Vec<Piece>,
    /// The service end's depth (rows from the low end); the screens passage after it, if any.
    service: usize,
    screens: bool,
    /// The dais end from this row (the landing, the main stair).
    dz: usize,
    hall_t: (usize, usize),
    land_t: (usize, usize),
    /// The main stair block (grid: x, y, w, h).
    pub stairs: [usize; 4],
    /// A castle keep's private stair at the dais end (grid block, as `stairs`): the lord's way from
    /// the dais end of the hall up to the great chamber and the solar, a spiral stair on its last
    /// square (`private_stair`); the lord's rooms go by it. None elsewhere: by the main stair.
    pub lord: Option<[usize; 4]>,
    /// A castle keep's landing at the low end (body frame: s0, s1, t0, t1), the main stair in it,
    /// opening onto the screens passage's far end: the hall entered at its low end from the stair as
    /// from the front door.
    pub low: Option<[usize; 4]>,
    /// The floor the hall is on; whether it is open two storeys (a void above it).
    pub hall_z: i8,
    pub open: bool,
    /// The body's storeys; a wing's and a forebuilding's.
    pub floors: i8,
    wing_floors: i8,
    fore_floors: i8,
    /// Built of pieces: the body's top is a level of its own (`floors`), towers have no tops.
    pub pieces: bool,
    top: Vec<Top>,
    wing_tops: Vec<u8>,
    wings: Vec<Wing>,
    fore: Vec<usize>,
    /// Each tower's top floor (`Shell::towers`' order).
    tower_top: Vec<i8>,
    /// Each tower's room's squares (`Shell::towers`' order).
    tower_sq: Vec<Vec<usize>>,
    /// A wing's or the forebuilding's squares in the body's box (where it overlaps the body's
    /// edge), what they are and the piece's top: the body's above it.
    overlap: Vec<(usize, Piece, i8)>,
    ring: Option<Ring>,
    seed: u64,
    /// Rooms each upper floor must still have, in order (a treasury, an armory...).
    pending: Vec<&'static str>,
    chapel: bool,
    /// The low end's and the front side's order of preference for the front door (nearer the
    /// street first).
    ends: [bool; 2],
    /// The long wall the last storey's mural passage ran along (t), -1 none yet.
    side: isize,
    /// A tower room is the oratory.
    oratory: bool,
    /// The last upper storey's body squares by room (`layout_of`): the next is laid out otherwise.
    last: Vec<i16>,
    /// A barracks entered from its low end (its long sides shut in): the mess hall reaches the
    /// end wall in a bay between the kitchen and the pantry, at this column (t), with open ground
    /// before it, that leaves the kitchen the most room.
    bay: Option<usize>,
    /// A small tower keep (`small_plan`): each storey a room or two off the landing at its high end.
    small: bool,
    /// The capital's palace's zones (`palace`).
    pal: Option<palace::Zones>,
    /// A rich manor: its chapel in the cross wing.
    rich: bool,
    /// A temple's zones (`temple`).
    tem: Option<temple::Zones>,
    /// A monastery's zones (`monastery`).
    mon: Option<monastery::Zones>,
    /// A town hall's or a guild's zones (`civic`).
    civ: Option<civic::Zones>,
    /// An inn's or a caravanserai's zones (`inn`).
    inn: Option<inn::Zones>,
    /// A town hall's belfry's spiral stair (`civic`; its squares `tower_sq`, its top `tower_top`).
    spiral: Option<usize>,
}

/// Room kinds by relative size, for a floor's rooms to take biggest first.
type Kinds = Vec<(&'static str, f64)>;

/// A building's plan, if it is laid out by zones: a castle's keep built of pieces, a castle's keep
/// redrawn by hand (as a tower keep), a barracks of 150 squares or more, a palace and a noble estate
/// (`kind_of`; `capital`: in the capital). `corner`: the grid's
/// corner towers' side (a keep without pieces, a barracks); `tops`: `town::keep_tops`; `near`: how
/// near the street a grid point is (lower nearer); `ground`: whether a grid square outside is open
/// ground (`Shell::ext_free`); `front`: the middle of a capital's palace's front as the town laid it
/// out (grid units: `town::palace::approach`).
#[allow(clippy::too_many_arguments)]
pub fn plan(b: &town::Building, sh: &Shell, key: Option<&str>, corner: usize, tops: &[u8], seed: u64, near: &dyn Fn(P) -> f64, ground: &dyn Fn(isize, isize) -> bool, (capital, city): (bool, bool), front: Option<P>) -> Option<Plan> {
    let (nx, ny) = (sh.nx, sh.ny);
    let kind = kind_of(b, key, sh, capital, city)?;
    if kind.is_civic() {
        return civic::plan(sh, kind, b.floors.max(1) as i8, seed, near, ground);
    }
    if kind.is_inn() {
        return inn::plan(sh, kind, b.floors.max(1) as i8, seed, near, ground);
    }
    // Each square's piece.
    let mut piece = vec![Piece::Out; nx * ny];
    let mut wing_of: Vec<usize> = Vec::new();
    for k in 0..nx * ny {
        if !sh.inside[k] {
            continue;
        }
        piece[k] = if b.parts.is_empty() {
            Piece::Body
        } else {
            match b.parts.get(sh.owner[k]).map(|p| p.kind) {
                Some(PartKind::Body) => Piece::Body,
                Some(PartKind::Wing) => {
                    let w = match wing_of.iter().position(|&p| p == sh.owner[k]) {
                        Some(w) => w,
                        None => {
                            wing_of.push(sh.owner[k]);
                            wing_of.len() - 1
                        }
                    };
                    Piece::Wing(w as u8)
                }
                Some(PartKind::Forebuilding) => Piece::Fore,
                Some(PartKind::Tower { .. }) => Piece::Tower,
                Some(PartKind::Apse) => Piece::Body,
                None => Piece::Out,
            }
        };
    }
    for (sq, _, _) in &sh.towers {
        for &k in sq {
            piece[k] = Piece::Tower;
        }
    }
    // (A grid's corner towers: theirs.)
    if corner > 0 && nx > 2 * corner && ny > 2 * corner {
        for (cx, cy) in [(0, 0), (nx - corner, 0), (0, ny - corner), (nx - corner, ny - corner)] {
            for j in cy..cy + corner {
                for i in cx..cx + corner {
                    if piece[j * nx + i] != Piece::Out {
                        piece[j * nx + i] = Piece::Tower;
                    }
                }
            }
        }
    }
    let grid_p = |p: P| {
        let d = geom::sub(p, sh.origin);
        [geom::dot(d, sh.u) / super::SQUARE_FT, geom::dot(d, sh.v) / super::SQUARE_FT]
    };
    // (Each programme lays out its own: `shell`, `palace` (else as an older palace), else the hall
    // house of `keep`.)
    if kind == Kind::ShellKeep {
        return shell::plan(b, sh, piece, seed, &grid_p);
    }
    if matches!(kind, Kind::Church | Kind::TempleHall) {
        return temple::plan(b, sh, kind, seed, &grid_p);
    }
    if kind == Kind::Monastery {
        return monastery::plan(b, sh, seed, near, ground);
    }
    if kind == Kind::Palace
        && let Some(p) = palace::plan(b, sh, piece.clone(), seed, near, ground, front)
    {
        return Some(p);
    }
    let kind = if kind == Kind::Palace { Kind::OldPalace } else { kind };
    keep::plan(b, sh, kind, piece, wing_of, tops, seed, near, ground, &grid_p)
}

impl Plan {
    /// What it is laid out as.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Its frame on the grid (as a hall's: s along it from its low end, a palace's from its front,
    /// t across it).
    pub fn frame(&self) -> super::hall::Frame {
        super::hall::Frame { o: self.f.o, ds: self.f.ds, dt: self.f.dt, len: self.f.len, wid: self.f.wid }
    }

    /// A manor's frame and the row its dais end (the family's cross wing) begins at; None for every
    /// other kind.
    pub fn cross_wing(&self) -> Option<(super::hall::Frame, usize)> {
        (self.kind == Kind::Manor).then(|| (self.frame(), self.dz))
    }

    /// The capital's palace's zones in its frame: the front range's depth, the back range's first
    /// row, the side ranges' width, whether there is a court between them.
    pub fn palace(&self) -> Option<(usize, usize, usize, bool)> {
        self.pal.as_ref().map(|z| (z.front, z.back, z.side, z.court))
    }

    /// A plan by zones with none of a keep's: its kind, grid, frame, pieces, stairs, storeys.
    #[allow(clippy::too_many_arguments)]
    fn bare(kind: Kind, nx: usize, ny: usize, f: Frame, piece: Vec<Piece>, stairs: [usize; 4], floors: i8, seed: u64) -> Plan {
        Plan {
            kind,
            nx,
            ny,
            f,
            piece,
            service: 0,
            screens: false,
            dz: 0,
            hall_t: (0, f.wid),
            land_t: (0, 0),
            stairs,
            lord: None,
            low: None,
            hall_z: 0,
            open: false,
            floors,
            wing_floors: 1,
            fore_floors: 1,
            pieces: false,
            top: Vec::new(),
            wing_tops: Vec::new(),
            wings: Vec::new(),
            fore: Vec::new(),
            tower_top: Vec::new(),
            tower_sq: Vec::new(),
            overlap: Vec::new(),
            ring: None,
            seed,
            pending: Vec::new(),
            chapel: false,
            ends: [true, true],
            side: -1,
            oratory: false,
            last: Vec::new(),
            bay: None,
            small: false,
            pal: None,
            rich: false,
            tem: None,
            mon: None,
            civ: None,
            inn: None,
            spiral: None,
        }
    }

    /// The levels: z, its storeys and its top (a keep of pieces), else its storeys (the top and
    /// tower tops as other buildings have them: `None` from `level`).
    pub fn reaches(&self) -> i8 {
        if self.pieces {
            self.floors
        } else if let Some((_, top)) = self.belfry() {
            top.max(self.floors - 1)
        } else {
            self.floors - 1
        }
    }


    /// Floor `z` laid out by zones, or None where the shared code lays it out (the cellar; a
    /// barracks' and a redrawn keep's roof).
    pub fn level(&mut self, z: i8) -> Option<Laid> {
        if z < 0 || z > self.reaches() {
            return None;
        }
        let rng = Pcg32::new(hash3(self.seed, z as i64, 0x5a, 0), 83);
        // (Above a wing's or the forebuilding's top, its squares on the body's edge are the body's.)
        for &(k, p, top) in &self.overlap {
            self.piece[k] = if z > top { Piece::Body } else { p };
        }
        let blank = Lay { cells: vec![-1; self.nx * self.ny], rooms: Vec::new(), hints: Hints::default(), items: Vec::new(), entry: None };
        if self.kind == Kind::ShellKeep {
            return self.shell_level(z, blank);
        }
        if self.small {
            return Some(self.small_level(z, blank));
        }
        if self.pal.is_some() {
            return Some(self.palace_level(z, blank));
        }
        if self.tem.is_some() {
            return self.temple_level(z, blank);
        }
        if self.mon.is_some() {
            return self.monastery_level(z, blank);
        }
        if self.civ.is_some() {
            return self.civic_level(z, blank, rng);
        }
        if self.inn.is_some() {
            return self.inn_level(z, blank, rng);
        }
        self.hall_level(z, blank, rng)
    }

    /// A level the shared code laid out (`level` gave none: a barracks' roof), finished as the zones'
    /// are: its corner towers' storey a guardroom, stores, an armory and a garrison, as on its floors.
    pub fn finish_shared(&self, lvl: &mut Level) {
        let mut n = 0;
        for room in lvl.rooms.iter_mut().filter(|r| r.kind == "tower room") {
            room.kind = ["guardroom", "storeroom", "armory", "garrison"][n % 4];
            n += 1;
        }
    }

    /// Nothing stands on a level's open tops (a platform, a walk) but a spiral stair.
    pub fn clear_tops(&self, lvl: &mut Level) {
        let (cells, rooms, nx) = (&lvl.cells, &lvl.rooms, self.nx);
        lvl.furniture.retain(|f| {
            let r = cells[f.y as usize * nx + f.x as usize];
            let kind = if r >= 0 { rooms[r as usize].kind } else { "" };
            f.kind == "spiral_stair" || !matches!(kind, "battlements" | "wall walk")
        });
    }
}
