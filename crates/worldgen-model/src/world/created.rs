use super::*;

/// A battlemap object put down by hand, at a world position (ft).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Placed {
    pub kind: ObjKind,
    pub x: f64,
    pub y: f64,
    /// Radians (sprites that are drawn turned, and uploaded sprites).
    #[serde(default)]
    pub rot: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub variant: u8,
    /// Drawn at `rot` (set when it is turned by hand, or given a turn by a tool); absent: as before.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub turned: bool,
}

fn one() -> f32 {
    1.0
}

/// A built-in object kind (`battlemap::CATALOG` id) or an uploaded sprite (`s:<asset id>`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ObjKind {
    Builtin(u16),
    Sprite(String),
}

impl ObjKind {
    /// The asset id of an uploaded sprite.
    pub fn sprite(&self) -> Option<&str> {
        match self {
            ObjKind::Sprite(s) => s.strip_prefix("s:"),
            ObjKind::Builtin(_) => None,
        }
    }
}

/// Generated battlemap objects taken away: with `kind`, the one of that kind within a quarter
/// square of (x, y); else every one whose centre lies within `r` ft of it (of `kinds`, if
/// given). Objects put down by hand are removed by deleting their entry instead.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Clear {
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<u16>,
}

/// An uploaded sprite: its name and 5e tactical rules (as `battlemap::KindInfo`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SpriteMeta {
    pub name: String,
    /// Squares across at scale 1.
    pub size: f32,
    /// 0 none, 1 half, 2 three-quarters, 3 full.
    pub cover: u8,
    pub blocks_move: bool,
    pub blocks_sight: bool,
    pub difficult: bool,
    pub height_ft: f32,
}

impl Default for SpriteMeta {
    fn default() -> Self {
        Self { name: String::new(), size: 1.0, cover: 0, blocks_move: false, blocks_sight: false, difficult: false, height_ft: 3.0 }
    }
}

/// Notes on a feature.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// A point note's spot (`m:<site>:<level>:<id>`), in the location's own grid units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f64; 2]>,
}

impl Note {
    /// Two notes on one place as one: the first's text, then the second's when it differs; the
    /// tags of both, in order (as the app's `joinNotes`).
    pub fn join(a: Option<&Note>, b: Option<&Note>) -> Option<Note> {
        let (a, b) = match (a, b) {
            (Some(a), Some(b)) => (a, b),
            (a, b) => return a.or(b).cloned(),
        };
        let text = if !a.text.is_empty() && !b.text.is_empty() && a.text.trim() != b.text.trim() {
            format!("{}\n\n{}", a.text, b.text)
        } else if a.text.is_empty() {
            b.text.clone()
        } else {
            a.text.clone()
        };
        let mut tags = a.tags.clone();
        for t in &b.tags {
            if !tags.contains(t) {
                tags.push(t.clone());
            }
        }
        Some(Note { text, tags, at: a.at.or(b.at) })
    }
}

/// A site added to the world: a ruin (with its dungeon or crypt), tower, camp, roadside inn,
/// cave, mine, lava tube or a bare way underground, generated like the generated ones.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Created {
    /// `c:<n>`.
    pub id: String,
    /// `ruin`, `tower`, `camp`, `waystation`, `cave`, `mine`, `lava_tube` or `entrance`; a
    /// drawn `building`, `castle`, `wall` or `plot`; a settlement (`CREATED_SETTLEMENTS`:
    /// `village`, `town`), added without remaking the world; or a `place`: a town's generated
    /// outskirts pushed out into a place of their own (`Edits.members`; no layout of its own, its
    /// buildings stay in their town's), written only by `town::members::change`.
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub name: String,
    /// What lies beneath a ruin (`dungeon`, `crypt` or `catacombs`) or an entrance (those, or
    /// `cave`, `mine`, `lava_tube`); otherwise as generated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<String>,
    /// Its site underground: `small`, `medium`, `large` or `huge` (else as generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// Levels underground, 1 to `under::MAX_LEVELS` (else as generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<u8>,
    /// Theme underground (an `under::THEMES` key for its kind, else as generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// A building's footprint (world ft): a simple polygon, its corners within
    /// `BUILDING_REACH_FT` of (x, y).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poly: Vec<[f64; 2]>,
    /// A building's storeys above ground, 1 to `MAX_FLOORS` (else 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors: Option<u8>,
    /// What a building is: a `town::catalog` function key (`inn`, `blacksmith`…) or a home
    /// (`BUILDING_HOMES`); else a house.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func: Option<String>,
    /// A building's roof (`ROOFS`): else as its kind has it (battlements on keeps and towers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<String>,
    /// A building's roof colour (`TINTS`), else picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<String>,
    /// A building's roof material (`town::roof::MATERIALS`; else the tint's, else picked) and
    /// shape (`town::roof::SHAPES`; else as `roof`, else picked).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// Roof colour variant 0-4 of its material (2 the usual), else picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade: Option<u8>,
    /// `roofed` (default) or `ruin` (broken walls, no roof).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<String>,
    /// A castle's gate: the side of `poly` it is in (corner `gate` to the next), else the side
    /// facing the nearest road (with `gates`, the first of them; `gate` the main one, first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<u32>,
    /// A castle's keep in the yard (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<bool>,
    /// (Roadmap 6 P7) A castle's keep's shape (`KEEP_SHAPES`): `square` (a keep of pieces, its
    /// kind rolled, never a round shell), `round` (a round great tower), `drawn` (`keep_poly`);
    /// else as rolled (now and then a shell keep on a motte).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_shape: Option<String>,
    /// A castle's keep drawn by hand (world ft): a simple polygon inside its outline.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_poly: Vec<[f64; 2]>,
    /// Buildings lining the inside of a castle's curtain (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yard_buildings: Option<bool>,
    /// (Roadmap 6 P7) A castle's moat (`MOATS`): `none`, `dry` (a ditch) or `wet`; else by roll
    /// (seeded by its outline), wet where water lies near and the ground (as shaped) is level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moat: Option<String>,
    /// A wall's line (world ft), 2–64 corners, at most `WALL_MAX_FT` long (a ring: 3–256,
    /// `WALL_RING_MAX_FT`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pts: Vec<[f64; 2]>,
    /// A wall's corners that are gates (indices into `pts`); more gates come where roads and
    /// streets cross it. A castle's sides with a gate (corner `g` to the next; the first the
    /// main gate), each in its side's middle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<u32>,
    /// A wall's last corner joins its first (a ring).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
    /// An entrance's building: the way down is in that building's cellar (`b:<layout>:<id>`,
    /// stored canonical, or a drawn building's `c:` id on the way in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_building: Option<String>,
    /// The way down in that cellar: `stairs` (default) or `trapdoor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub way: Option<String>,
    /// A settlement's naming culture, set where it was put (`t0::made::culture_at`) so a world
    /// remade round it names it as before; else from its nearest settlement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub culture: Option<u8>,
    /// What a settlement lives by (`PIN_KINDS` but `port` and `fishing`), set where it was put
    /// (`t0::made::kind_at`); else from its ground.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lives_by: Option<String>,
    /// Deleted (kept in the list so later sites keep their place).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
    /// (R5 122) A deleted village or town: the layout number the edits made in it were set aside
    /// under (`t0::made::set_aside_changed`), for a restore to bring them back (`t0::made::restore`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aside: Option<usize>,
    /// (R5 M20) A drawn building standing on a structure put down by hand: a bridge's deck
    /// (`v:<id>`) or a pier or breakwater (`q:<id>`). Its ground floor is at that deck's level,
    /// with no cellar; it follows its bridge (`Edits::follow_crossing`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    /// (R5 towns) A castle's pieces changed by hand, kept with the castle (not by its layout
    /// number, which moves when an earlier site is deleted): `keep` for its keep (wherever it
    /// stands), else a yard building's id in the castle's layout (`b:<layout>:<id>`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pieces: BTreeMap<String, CastlePiece>,
    /// A plot's fill (Roadmap 6: a free plot drawn by hand, its outline `poly`, filled as a
    /// hand town's block is: `town::hand::BlockFill`, not a castle; else common lots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<crate::model::town::BlockFill>,
    /// (Roadmap 6 R4) A drawn building's front: the chord (world ft) of the side of its lot that
    /// faces its row's road; its door and rooms face the side of `poly` nearest it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<[[f64; 2]; 2]>,
}

/// (R5 towns) One of a castle's pieces changed by hand (`Created::pieces`): its looks, function
/// and storeys as a town building's edit (`BuildingEdit`, without a footprint: `at` its middle as
/// built, a yard building's edit held only while one stands there), and its name.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CastlePiece {
    #[serde(flatten)]
    pub edit: BuildingEdit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl CastlePiece {
    /// The key of the castle's keep in `Created::pieces`.
    pub const KEEP: &'static str = "keep";

    /// Whether it changes nothing.
    pub fn is_noop(&self) -> bool {
        self.edit.is_noop() && self.name.is_none()
    }
}

/// A non-player character. Every field is optional on the way in, so a partial entry from an
/// agent or an older app still reads.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Npc {
    pub name: String,
    pub appearance: String,
    pub mannerisms: String,
    pub attitude: Attitude,
    pub goals: String,
    pub notes: String,
    pub tags: Vec<String>,
    /// A picture in the asset store (its id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub portrait: Option<String>,
    /// Where they are found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<NpcPlace>,
    /// Free text: alive, dead, missing, imprisoned...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// How an NPC feels about the players.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Attitude {
    pub stance: Stance,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Hostile,
    Unfriendly,
    #[default]
    Neutral,
    Friendly,
    Allied,
}

pub const STANCES: [&str; 5] = ["hostile", "unfriendly", "neutral", "friendly", "allied"];

/// Where an NPC is: a feature, building, district or site id; inside, a level and a point (ft).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NpcPlace {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

/// A plot point: what is going on, where (anchors: any ids) and who is involved (NPC ids).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Plot {
    pub title: String,
    pub text: String,
    pub status: PlotStatus,
    pub anchors: Vec<String>,
    pub npcs: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotStatus {
    #[default]
    Idea,
    Active,
    Resolved,
}

pub const PLOT_STATUSES: [&str; 3] = ["idea", "active", "resolved"];

/// The kinds of site that can be created.
pub const CREATABLE: [&str; 12] = ["ruin", "tower", "camp", "waystation", "cave", "mine", "lava_tube", "entrance", "building", "castle", "wall", "plot"];
/// Settlements that can be created without remaking the world (cities keep the remake).
pub const CREATED_SETTLEMENTS: [&str; 2] = ["village", "town"];
/// Kinds drawn by hand as a shape (a footprint, an outline, a line), not put at a point.
pub const DRAWN_KINDS: [&str; 4] = ["building", "castle", "wall", "plot"];
/// Widest a plot may be, its point to its farthest corner (ft).
pub const PLOT_REACH_FT: f64 = 750.0;
/// Widest a castle may be across: the long side of the box that fits it closest (ft).
pub const CASTLE_MAX_FT: f64 = 1500.0;
/// A hand castle's outline drawn in this far (ft) stays one piece: its necks 30 ft across at least.
pub const CASTLE_NECK_FT: f64 = 15.0;
/// Most gates a castle may have.
pub const CASTLE_MAX_GATES: usize = 8;
/// A castle's keep's shapes (`Created::keep_shape`).
pub const KEEP_SHAPES: [&str; 3] = ["square", "round", "drawn"];
/// A castle's moats (`Created::moat`).
pub const MOATS: [&str; 3] = ["none", "dry", "wet"];
/// Narrowest a castle may be (ft): room for a curtain, a yard and a keep.
pub const CASTLE_MIN_FT: f64 = 80.0;
/// Longest a wall may be (ft).
pub const WALL_MAX_FT: f64 = 3000.0;
/// (R5 towns, M18) Longest a closed wall (a ring) may be (ft): round a city's old town or upper
/// city, or a metropolis' whole old town (17,500-19,900 ft round).
pub const WALL_RING_MAX_FT: f64 = 24000.0;
/// Most corners of a wall's line, open and closed (a ring round patches takes their corners:
/// `town::handwall::RING_MAX_CORNERS`).
pub const WALL_MAX_CORNERS: usize = 64;
pub const WALL_RING_MAX_CORNERS: usize = 256;
/// How far a castle's or wall's layout reaches past its outline (towers stand astride it).
pub const WORKS_MARGIN_FT: f64 = 40.0;

/// Homes a building can be (`func`), as `town::catalog::RESIDENTIAL`: hovel, house, townhouse,
/// tenement, noble estate, farmhouse, wizard's tower.
pub const BUILDING_HOMES: [&str; 7] = ["hovel", "house", "townhouse", "tenement", "noble_estate", "farmhouse", "wizard_tower"];
/// A building's roof: pitched with hips, a walkable roof behind battlements, or a cone (a
/// pyramid on square plans).
pub const ROOFS: [&str; 3] = ["hip", "battlements", "cone"];
/// Roof colours (as the battlemap's `ROOF_TINTS`).
pub const TINTS: [&str; 5] = ["terracotta", "slate", "thatch", "shingle", "moss"];
pub const STRUCTURES: [&str; 2] = ["roofed", "ruin"];
pub const MAX_FLOORS: u8 = 8;
/// How far a building's corners may lie from its point (ft): its layout reaches
/// `town::sites::SITE_REACH_FT`.
pub const BUILDING_REACH_FT: f64 = 200.0;

impl Created {
    /// A site created again within this of one of its kind (with the same beneath) is that one.
    pub const SAME_SPOT_FT: f64 = 300.0;

    /// Whether `self`, being created (asked for at `asked`, put at its own point), is `other`, a
    /// live site already there: the same kind and beneath, within `SAME_SPOT_FT` of either point;
    /// a building, the same footprint (each corner within 2 ft), function and name (if `self`
    /// has one). Creating a site again (a script run twice, a world file opened again) then adds
    /// nothing; another building on that footprint is refused where it overlaps the first.
    pub fn same_site(&self, other: &Created, asked: [f64; 2]) -> bool {
        if other.removed || other.kind != self.kind {
            return false;
        }
        let same = |a: &[[f64; 2]], b: &[[f64; 2]]| a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a[0] - b[0]).abs() <= 2.0 && (a[1] - b[1]).abs() <= 2.0);
        let named = self.name.trim().is_empty() || self.name.trim() == other.name.trim();
        match self.kind.as_str() {
            "building" => return other.func == self.func && named && same(&other.poly, &self.poly),
            "plot" => return named && same(&other.poly, &self.poly),
            "castle" => return named && same(&other.poly, &self.poly),
            "wall" => return named && other.closed == self.closed && same(&other.pts, &self.pts),
            _ => {}
        }
        if self.in_building.is_some() || other.in_building.is_some() {
            return self.in_building == other.in_building && other.under_kind() == self.under_kind();
        }
        let near = |p: [f64; 2]| (other.x - p[0]).hypot(other.y - p[1]) <= Self::SAME_SPOT_FT;
        other.under_kind() == self.under_kind() && (near([self.x, self.y]) || near(asked))
    }

    /// The kind of site underground (none for towers, camps and inns).
    pub fn under_kind(&self) -> Option<crate::model::under::UnderKind> {
        use crate::model::under::UnderKind;
        match self.kind.as_str() {
            "ruin" => Some(self.under.as_deref().and_then(UnderKind::parse).unwrap_or(UnderKind::Dungeon)),
            "entrance" => Some(self.under.as_deref().and_then(UnderKind::parse).unwrap_or(UnderKind::Dungeon)),
            "cave" => Some(UnderKind::Cave),
            "mine" => Some(UnderKind::Mine),
            "lava_tube" => Some(UnderKind::LavaTube),
            _ => None,
        }
    }

    /// Whether this is a settlement added by hand (`CREATED_SETTLEMENTS`).
    pub fn is_settlement(&self) -> bool {
        CREATED_SETTLEMENTS.contains(&self.kind.as_str())
    }
}

impl Created {
    /// A castle's gates: the sides with one (`gate` first, then `gates`), each once.
    pub fn castle_gates(&self) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for g in self.gate.iter().chain(&self.gates) {
            if !out.contains(g) {
                out.push(*g);
            }
        }
        out
    }

    /// How far (ft) from its point a created site's layout reaches: a castle's or wall's
    /// farthest corner and its towers; any other site `town::sites::SITE_REACH_FT`.
    pub fn reach(&self) -> f64 {
        let far = |pts: &[[f64; 2]]| pts.iter().map(|q| (q[0] - self.x).hypot(q[1] - self.y)).fold(0.0, f64::max);
        match self.kind.as_str() {
            "castle" => far(&self.poly) + WORKS_MARGIN_FT,
            "wall" => far(&self.pts) + WORKS_MARGIN_FT,
            "plot" => far(&self.poly) + WORKS_MARGIN_FT,
            _ => crate::model::consts::SITE_REACH_FT,
        }
    }
}
