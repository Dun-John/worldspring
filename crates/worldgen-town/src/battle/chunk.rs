//! A chunk's data (objects, sprites, vector shapes and their codes) and its packed form.

use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Object {
    pub kind: Kind,
    pub variant: u8,
    /// Position in squares within the chunk (0..128).
    pub x: f32,
    pub y: f32,
    pub rot: f32,
    pub scale: f32,
    /// An uploaded sprite: index + 1 into `Chunk::sprites` (0 = the built-in `kind`).
    pub sprite: u16,
    /// Put down by hand and turned (`Placed::turned`): drawn at `rot`.
    pub turned: bool,
}

/// An uploaded sprite used in a chunk (`Edits::sprites`).
#[derive(Clone, Debug)]
pub struct ChunkSprite {
    /// Its asset id.
    pub asset: String,
    pub name: String,
    pub info: KindInfo,
}

/// Packed kind ids from here up name `Chunk::sprites` (built-in ids stay below).
pub const SPRITE_KIND: u16 = 1000;

pub struct Chunk {
    pub key: TileKey,
    /// Ground height at square centers (ft), after micro-relief.
    pub height: Vec<f32>,
    /// Elevation tier (5-ft steps above sea level).
    pub tier: Vec<i16>,
    pub surface: Vec<Surface>,
    /// Bits: 1 cliff to the east, 2 cliff to the south, 4 slope edge east, 8 slope edge south,
    /// 16 difficult terrain, 32 a park's pond or stream or the ground by it, 64 a cliff's scree
    /// (`battlemap::cliff`; its faces are difficult terrain).
    /// 128 small closed relief, drawn soft (`soft_relief`).
    pub edges: Vec<u8>,
    pub objects: Vec<Object>,
    pub atmosphere: Atmosphere,
    pub water_level: Vec<f32>,
    /// Heights and water on the `HS`² grid with a one-square halo taken from the neighbours'
    /// (pure) terrain and micro-relief, so chunk edges shade seamlessly.
    pub halo_height: Vec<f32>,
    pub halo_water: Vec<f32>,
    /// Road class + 1 on the `HS`² grid (0 = none), bit 0x80 = bridge deck; the renderer
    /// filters it into smooth road edges.
    pub road_halo: Vec<u8>,
    /// How much of each `HS`² square's middle the road covers, 0-255 from the roads' own lines
    /// (`battlemap::road_cover`): a ramp across the edge, so the renderer's filtered coverage
    /// draws the edge where the road runs, not along the squares. 255/0 where no line explains
    /// the square (roads drawn by hand, crossings' approaches); 0 on a bridge's deck.
    pub road_cover: Vec<u8>,
    /// Building on each square: index + 1 into `buildings` (0 = none).
    pub building: Vec<u16>,
    /// (settlement index, building index) of each building touching the chunk.
    pub buildings: Vec<(u32, u32)>,
    /// Settlement ground per square (`URBAN_*`, 0 = none).
    pub urban: Vec<u8>,
    /// Footprint of each building in `buildings` (chunk-local squares) and its storeys; the
    /// renderer draws roofs from these rather than from the square grid.
    /// With each: storeys, roof (style 0 pitched, 1 battlements, 2 cone | (tint + 1) << 2) and
    /// corner-tower side (squares; a castle's keep with no pieces its top in the high two bits,
    /// `town::plain_keep_top`; 255 a keep of pieces), and its look (`look_bytes`).
    pub building_polys: Vec<(Vec<[f32; 2]>, u8, u8, u8, [u8; 3])>,
    /// A keep's pieces with its footprint in `building_polys` (`part_code`, outline; chunk-local
    /// squares); a palace's court (`COURT_PART`, its corners each by the footprint's in the same
    /// place); a manor's cross wing and main range (`CROSS_WING_PART`, `MAIN_RANGE_PART`); empty for
    /// every other building.
    pub building_parts: Vec<Vec<(u8, Vec<[f32; 2]>)>>,
    /// Height above the ground per halo square of what is drawn as a vector on top (roofs,
    /// walls, towers, bridge and pier decks; 0 elsewhere): tiers include it, the rendered
    /// ground does not.
    pub roof_lift: Vec<f32>,
    /// Walls, towers and decks drawn as vectors (chunk-local squares).
    pub structures: Vec<VectorShape>,
    /// Uploaded sprites the chunk's objects use.
    pub sprites: Vec<ChunkSprite>,
    /// A castle's yard per square (bits 1 a bailey, 2 a garden yard's, 4 on a motte above its toe;
    /// empty: none here): where the guarantee counts its walls, towers and buildings as cover. Bit 8
    /// a hedgerow's or dry-stone wall's square (v63 countryside, `town::rural::cover`): cover itself;
    /// bit 16 a village's green, which takes its own cover.
    pub yard: Vec<u8>,
    /// The settlement whose paving the chunk's cobbles are (its index + 1 in the low 24 bits, its
    /// culture + 1 in the high 8; 0 none): the highest-tier layout near the chunk, the nearest of
    /// those. The ground picks the stone's colour by them.
    pub stone: u32,
    /// v63 countryside: each field square's furrows (`town::fields::square_byte`; 0 elsewhere; empty: no
    /// field here).
    pub field: Vec<u8>,
}

impl Chunk {
    /// An object's tactical rules (a built-in kind's, or an uploaded sprite's).
    #[inline]
    pub fn info(&self, o: &Object) -> &KindInfo {
        match o.sprite {
            0 => info(o.kind as u16),
            s => &self.sprites[s as usize - 1].info,
        }
    }

    /// An object's name.
    #[inline]
    pub fn name(&self, o: &Object) -> &str {
        match o.sprite {
            0 => info(o.kind as u16).name,
            s => &self.sprites[s as usize - 1].name,
        }
    }
}

/// A structure the renderer draws as a vector over the ground.
#[derive(Clone, Debug)]
pub struct VectorShape {
    pub kind: ShapeKind,
    /// Line width or tower radius (squares); unused for polygons.
    pub size: f32,
    pub pts: Vec<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// Curtain wall segment, then `[flags, side]` (`WALL_BARE_A`, `WALL_BARE_B`, `WALL_RUINED`; side 1
    /// where the outside of the town, or of the castle, is left of a to b, -1 right, 0 unknown:
    /// `town::wallside`), then `[style, 0]` (`town::wallstyle::WallStyle::code`).
    Wall = 0,
    /// Wall or gate tower: its centre, then `[flags, angle]` (`WALL_GATE`, `WALL_RUINED`, `WALL_SQUARE`;
    /// the way a square's or an octagon's sides run, the way a D faces out, radians), then `[style, form]`
    /// (`WallStyle::code` with its shape: round, square, D, octagon; a gate tower's gatehouse form as
    /// on its `Gate`, 0 for the rest). A square or arched gate tower's half is drawn by its `Gate`.
    Tower = 1,
    /// Bridge or pier deck outline, its first edge along the span; size is the style (0 pier,
    /// 1 timber bridge, 2 stone bridge, 3 old stone bridge).
    Deck = 2,
    /// Road bridge centre line, then `[material, 0]` (0 timber, 1 old stone).
    RoadBridge = 3,
    /// A way underground: the opening and a point one square in along the passage; size is
    /// the `UnderKind` (0 dungeon, 1 crypt, 2 cave, 3 mine, 4 lava tube, 5 sewer, 6 catacombs).
    Entrance = 4,
    /// A round feature raised a tier on paved ground: its centre and a point on its rim in the
    /// direction its steps come down (squares); size is the `PlazaFeature`.
    Dais = 5,
    /// A ford: stepping stones across shallow water, its line from bank to bank; size is the
    /// road's width (squares).
    Ford = 6,
    /// A ferry: a rope from landing to landing, a jetty at each end and the raft; size is the
    /// raft's width (squares).
    Ferry = 7,
    /// A flight of steps' axis from the bottom up, then (how far up the flight its bottom lies,
    /// the depth of a tread); size is the width it covers (squares). Every chunk it crosses has
    /// it: the ground draws the treads square across it.
    Stairs = 8,
    /// A gatehouse over the passage between two gate towers: their centres, then `[style, form]`
    /// (`WallStyle::code`; `gatehouse::House::code`, 1 + `Gatehouse`, 0 none), then `[side, exit]` (1
    /// where the town lies on the side of +y when the first tower's way to the second is +x, i.e.
    /// cross(second - first, inward) > 0, else -1; a bent
    /// entrance's way out: 1 past the second tower, -1 past the first, 0 none); size is 0 standing, 1
    /// a ruin's.
    Gate = 9,
    /// A palace forecourt's low garden wall: a straight stretch of it; size is its thickness.
    GardenWall = 10,
    /// A gate pier: its centre, then `[angle, 0]` (the way its sides run, radians); size is half its
    /// side.
    GatePier = 11,
    /// A forecourt's gate: the ends of its opening (the piers' inner faces), then a point inside,
    /// the way its leaves stand open; size is the style (0 wrought iron, 1 timber).
    ForecourtGate = 12,
    /// A gravel walk across a palace court's lawn: its four corners.
    GravelWalk = 13,
    // v63 countryside (`town::rural`): a field's or a croft's bound, a straight stretch of it (its
    // ends); size is its thickness. About 4 ft: difficult ground, cover, seen over (never sight's).
    Hedge = 14,
    DryWall = 15,
    /// A post-and-rail fence.
    Fence = 16,
    /// A windmill's cap and sails: the tower's middle, then `[angle, radius]` (the way the cap faces,
    /// radians; the tower's radius in squares), then `[pale, 0]` (1: its walls lime-washed or
    /// plastered, its cap wooden); size is the sails' reach from the hub.
    Sails = 17,
    // v63 castles
    /// A castle's drawbridge over its moat (`town::moat::Drawbridge`): the gate's middle, the far
    /// end, then `[wet, 0]` (1 over water, 0 over a dry ditch); size is its width (squares).
    Drawbridge = 18,
    /// A dry ditch's bank, inked along its top: the two ends of a stretch of it, the way down into
    /// the ditch (a unit vector; `[0, 0]` none), then `[bank, 0]` (1: its outer bank behind it).
    DitchBank = 19,
    // R5 towns
    /// A star-angled wall's bastion (`Layout::bastions`): its outline, the flank's foot on the curtain,
    /// the flank's tip, the point, the other flank's tip and foot, the corner it stands in place of;
    /// then `[style, 0]` (`WallStyle::code`). `size` is its walls' width (squares).
    Bastion = 20,
    // R5 M20
    /// A breakwater's armour, big rubble along made ground's water edge: its line, `size` the band's
    /// width (squares).
    Breakwater = 32,
    /// A moored cog: its middle, then a point toward its bow, then `[look, 0]`; `size` its length
    /// (squares).
    Ship = 33,
    /// A lighthouse's lamp gallery over its roof: its middle; `size` its radius (squares).
    Lantern = 34,
}

/// Flags on a wall's or tower's last point (`[flags, 0]`): the wall's first or last end stands
/// bare (no tower on it), a gate tower, a ruin's works.
pub const WALL_BARE_A: u8 = 1;
pub const WALL_BARE_B: u8 = 2;
pub const WALL_GATE: u8 = 1;
pub const WALL_RUINED: u8 = 4;
/// A square tower (`Layout::square_towers`); the flags' second value is the way its sides run
/// (radians).
pub const WALL_SQUARE: u8 = 8;

/// The code of a palace's court among a building's pieces (`Chunk::building_parts`, after a keep's
/// `part_code`s): open to the sky, the roof a range along each side round it.
pub const COURT_PART: u8 = 5;
/// The codes of a manor's cross wing over its family end and its main range
/// (`interior::cross_wing_of`): roofed apart, the wing's ridge across the range's (a cross gable).
pub const CROSS_WING_PART: u8 = 6;
pub const MAIN_RANGE_PART: u8 = 7;
/// A temple's apse (`town::temple`): with its body (0, the nave and choir) and its transept (4, a
/// wing), roofed as a cross, the apse's half cone on the body's end.
pub const APSE_PART: u8 = 8;
/// A monastery's church (`interior::Court::church`): its corners from its west end along its length;
/// roofed higher than its ranges, a bell-cote on its west end.
pub const CHURCH_PART: u8 = 9;
/// A city's arcaded town hall's open front (`interior::arcade_front`), or a coaching inn's carriage
/// arch's mouth (`interior::arch_front`): its corners along the front from the first square to the
/// last, then a square in.
pub const ARCADE_PART: u8 = 10;
/// (Roadmap 7, 163) A range of an L, a U or a court (`town::wings::arms`): each hipped or gabled, the
/// ranges overlapping where they meet, valleys there.
pub const RANGE_PART: u8 = 11;

/// Binary payload for the renderer (little-endian):
///   0 magic "BATL" | 4 level | 8 x | 12 y | 16 n_objects | 20 atmosphere | 24 sea f32 | 28 stone u32
///   (`Chunk::stone`: whose paving the cobbles are, for their colour)
///   32 height f32[HS²] | tier i16[SQ²] | surface u8[SQ²] | edges u8[SQ²] | water f32[HS²]
///   (height and water include the one-square halo ring)
///   objects: (kind u16, variant u8, turned u8 (1: put down by hand and drawn at rot,
///   `Placed::turned`; 0 otherwise), x f32, y f32, rot f32, scale f32) × n
///   road halo u8[HS²]: road class + 1 (0 = none), 0x80 = bridge deck
///   road cover u8[HS²]: the road's coverage of each square's middle, 0-255 (`Chunk::road_cover`)
///   building u16[SQ²]: index + 1 of the building on each square (0 = none)
///   buildings: count u32, then per building: n u8, floors u8, roof u8 (style: 0 hip,
///   1 battlements, 2 cone; | (tint + 1) << 2, 0 = picked),
///   corner-tower side u8 (squares; 255: a keep of pieces), roof material u8, shape u8 (| 0x10
///   smoke-darkened), weathering u8 (`town::roof`), then n × (x, y) f32 in
///   chunk squares (heights above are the ground; roofs are drawn from these outlines); a keep
///   of pieces then has its pieces: count u8, then per piece `part_code` u8, n u8, n × (x, y) f32
///   structures: count u32, then per shape: kind u8 (`ShapeKind`), n u8, 0, 0, size f32,
///   then n × (x, y) f32 in chunk squares (drawn as vectors; decks are not in the road halo; a
///   flight of steps' third point is its start up the flight and its tread, in squares)
///   raised u8[SQ² × 2]: per square, height (ft, capped 255) of what stands on the drawn
///   ground (roof, wall walk, tower top, deck) and its `URBAN_*` kind
///   sprites: count u32, then per uploaded sprite (object kind `SPRITE_KIND` + index): id length
///   u8, cover u8, flags u8 (1 blocks movement, 2 blocks sight, 4 difficult), 0, radius f32
///   (squares), height f32 (ft), then the asset id (ASCII)
pub fn pack(world: &World, c: &Chunk) -> Vec<u8> {
    let n = SQ * SQ;
    let mut out = Vec::with_capacity(32 + n * 12 + c.objects.len() * 20);
    for v in [0x4241_544cu32, c.key.level as u32, c.key.x, c.key.y, c.objects.len() as u32, c.atmosphere as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&(world.params().sea_level_ft as f32).to_le_bytes());
    out.extend_from_slice(&c.stone.to_le_bytes());
    // Rendered ground: roofs are drawn as vectors, so their squares show the ground below.
    for (v, l) in c.halo_height.iter().zip(&c.roof_lift) {
        out.extend_from_slice(&(v - l).to_le_bytes());
    }
    for v in &c.tier {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend(c.surface.iter().map(|s| *s as u8));
    out.extend_from_slice(&c.edges);
    for v in &c.halo_water {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for o in &c.objects {
        let kind = if o.sprite == 0 { o.kind as u16 } else { SPRITE_KIND + o.sprite - 1 };
        out.extend_from_slice(&kind.to_le_bytes());
        out.push(o.variant);
        // Drawn at `rot` (`Placed::turned`); 0 for generated objects, so their bytes are as before.
        out.push(o.turned as u8);
        for v in [o.x, o.y, o.rot, o.scale] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend(c.road_halo.iter().map(|&r| if r & 0x80 != 0 { 0 } else { r }));
    out.extend_from_slice(&c.road_cover);
    for b in &c.building {
        out.extend_from_slice(&b.to_le_bytes());
    }
    out.extend_from_slice(&(c.building_polys.len() as u32).to_le_bytes());
    for (k, (poly, floors, roof, tower, look)) in c.building_polys.iter().enumerate() {
        let parts = c.building_parts.get(k).filter(|p| !p.is_empty());
        out.extend_from_slice(&[poly.len().min(255) as u8, *floors, *roof, if parts.is_some() { 255 } else { *tower }]);
        out.extend_from_slice(look);
        for p in poly.iter().take(255) {
            out.extend_from_slice(&p[0].to_le_bytes());
            out.extend_from_slice(&p[1].to_le_bytes());
        }
        // A keep's pieces (its corner-tower byte 255): count u8, then per piece code u8, n u8, n ×
        // (x, y) f32.
        if let Some(parts) = parts {
            out.push(parts.len().min(255) as u8);
            for (code, pts) in parts.iter().take(255) {
                out.extend_from_slice(&[*code, pts.len().min(255) as u8]);
                for p in pts.iter().take(255) {
                    out.extend_from_slice(&p[0].to_le_bytes());
                    out.extend_from_slice(&p[1].to_le_bytes());
                }
            }
        }
    }
    out.extend_from_slice(&(c.structures.len() as u32).to_le_bytes());
    for sh in &c.structures {
        out.extend_from_slice(&[sh.kind as u8, sh.pts.len().min(255) as u8, 0, 0]);
        out.extend_from_slice(&sh.size.to_le_bytes());
        for p in sh.pts.iter().take(255) {
            out.extend_from_slice(&p[0].to_le_bytes());
            out.extend_from_slice(&p[1].to_le_bytes());
        }
    }
    for k in 0..n {
        let lift = c.roof_lift[(k / SQ + 1) * HS + k % SQ + 1];
        out.push(lift.round().clamp(0.0, 255.0) as u8);
        out.push(c.urban[k]);
    }
    // v63 countryside: the field squares' furrows, before the sprite table, which ends the payload (`Chunk::field`): count u32 (0 or SQ²), bytes.
    out.extend_from_slice(&(c.field.len() as u32).to_le_bytes());
    out.extend_from_slice(&c.field);
    out.extend_from_slice(&(c.sprites.len() as u32).to_le_bytes());
    for sp in &c.sprites {
        let i = &sp.info;
        let flags = i.blocks_move as u8 | (i.blocks_sight as u8) << 1 | (i.difficult as u8) << 2;
        out.extend_from_slice(&[sp.asset.len().min(255) as u8, i.cover, flags, 0]);
        out.extend_from_slice(&i.radius.to_le_bytes());
        out.extend_from_slice(&i.height_ft.to_le_bytes());
        out.extend_from_slice(&sp.asset.as_bytes()[..sp.asset.len().min(255)]);
    }
    out
}
