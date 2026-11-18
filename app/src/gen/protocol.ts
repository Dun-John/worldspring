// Messages between the main thread, the coordinator worker and generator workers,
// plus the terrain payload layout (mirrors crates/worldgen/src/payload.rs).
import type { PreparedChunk } from './battlePrep';
import type { EditOp } from '../sync/ops';
import type { RiverGeometry, SiteGeometry, TileRivers, TileSitePolys } from './tileGeometry';

export type Wind = 'belts' | 'from_west' | 'from_east';

/** Mirrors `worldgen::WorldParams` (crates/worldgen-model/src/world.rs). */
export interface WorldParams {
  width_mi: number;
  height_mi: number;
  sea_level_ft: number;
  max_elev_ft: number;
  land_fraction: number;
  ruggedness: number;
  /** 0..1: how much of the plates' own mountain building is kept (0: only drawn mountains). */
  procedural_mountains: number;
  plate_count: number;
  erosion: number;
  lat_top: number;
  lat_bottom: number;
  wind: Wind;
  temp_offset_c: number;
  moisture: number;
  volcanoes: number;
  river_density: number;
  settlement_density: number;
  poi_density: number;
  biome_weights: Record<string, number>;
  /** Off: only the roads drawn in the sketch (with short spurs to the settlements beside them). */
  generated_roads: boolean;
}

export interface WorldFile {
  gen_version: number;
  seed: number;
  params: Partial<WorldParams>;
  /** Drawn constraints the generator follows (`worldgen::world::Sketch`). */
  sketch?: Sketch;
  /** Edits layered over the generated world (`worldgen::world::Edits`). */
  edits?: Edits;
}

export type SketchTool = 'land' | 'sea' | 'range' | 'river' | 'biome' | 'pin' | 'massif' | 'elevation' | 'lake' | 'volcano' | 'region' | 'site' | 'road';

export type VolcanoKind = 'strato' | 'shield' | 'cinder' | 'caldera';
export type VolcanoActivity = 'active' | 'dormant' | 'extinct';
/** What a pinned settlement lives by (`world::PIN_KINDS`). */
export type PinKind = 'port' | 'river' | 'mining' | 'fortress' | 'market' | 'farming' | 'fishing' | 'lumber' | 'herding' | 'oasis';
/** A site stroke's kind (`world::SITE_KINDS`). */
export type SiteKind = 'ruin' | 'tower' | 'camp' | 'waystation' | 'cave' | 'mine' | 'lava_tube' | 'entrance';
/** Road strokes' kinds (`world::ROAD_KINDS`): the road's class, or `none` (no planned road crosses it). */
export type RoadKind = 'kings_road' | 'road' | 'track' | 'none';

/** A stroke of a sketch (`worldgen::world::Stroke`); points are world ft. */
export interface Stroke {
  tool: SketchTool;
  pts: [number, number][];
  /** Land, sea, biome: the outline is filled (massif, elevation and lake strokes always are). */
  closed?: boolean;
  /** Brush radius; a massif's foothills, an elevation's edge. */
  radius_ft?: number;
  /** 0..1: a range's or massif's height, a river's or volcano's size. */
  strength?: number;
  hard?: boolean;
  biome?: string;
  tier?: 'metropolis' | 'city' | 'town' | 'village';
  /** The name of what it makes (pins, ranges, massifs, rivers, lakes, volcanoes, painted
   * biomes, coasts, regions, sites, roads). */
  name?: string;
  /** Massifs: the ridges' direction (degrees; else the outline's long axis). */
  trend?: number;
  /** Elevation: ft raised (negative: lowered). */
  delta_ft?: number;
  /** Lakes: the water's level (ft above sea level; else its shore's lowest point). */
  level_ft?: number;
  salt?: boolean;
  /** Volcanoes: VolcanoKind; pins: PinKind; sites: SiteKind; regions: what it names
   * (`world::REGION_KINDS`); roads: RoadKind. */
  kind?: string;
  activity?: VolcanoActivity;
  /** Pins: the realm's capital. */
  capital?: boolean;
  /** Pins: district names, the central one first. */
  wards?: string[];
  /** Pins: added beside the world's own settlements (Edit › Places), not in place of one of its
   * tier (it doesn't count toward their number or keep them away; smaller ones keep 10 mi off,
   * and none is placed within 3 mi of a settlement). */
  added?: boolean;
  /** Pins: in place of the world's own settlement at its spot (within 3 mi), changed: the tier it
   * was placed at (mapd's `update_settlement` makes these; nothing else moves). */
  replaces?: 'metropolis' | 'city' | 'town' | 'village';
  /** Pins: false keeps a waterside settlement where the pin stands (not moved onto its shore or
   * beside its river). */
  on_water?: boolean;
  /** Sites: beneath a ruin (dungeon, crypt, catacombs) or an entrance (any site). */
  under?: string;
}

export interface Sketch {
  strokes: Stroke[];
}

/** Something drawn that the world could not follow exactly (`t0::sketch::Conflict`). */
export interface Conflict {
  stroke: number;
  message: string;
  x: number;
  y: number;
}

/** A site added by the user or an agent (`worldgen::world::Created`). */
export interface Created {
  id: string;
  kind: string;
  x: number;
  y: number;
  name: string;
  under?: string;
  /** Its site underground (`under::SiteSize`, 1–6 levels, an `under::THEMES` key). */
  size?: SiteSize;
  levels?: number;
  theme?: string;
  /** A village or town added without a remake: its naming culture and what it lives by. */
  culture?: number;
  lives_by?: string;
  /** A deleted village or town: the layout number its edits were set aside under (a restore
   * brings them back). */
  aside?: number;
  /** A building drawn by hand (`worldgen::world::Created`): its footprint (world ft), storeys,
   * what it is (a business key or a home), roof, roof colour and structure. */
  poly?: [number, number][];
  floors?: number;
  func?: string;
  /** A row's building: the two points of the road it faces. */
  faces?: [[number, number], [number, number]];
  roof?: RoofStyle;
  tint?: Tint;
  /** Roof material and shape (`ROOF_MATERIALS`, `ROOF_SHAPES`); else the tint's / the roof's, else picked. */
  material?: RoofMaterial;
  shape?: RoofShape;
  /** Roof colour variant 0-4 of the material (`ROOF_SHADES`; 2 the usual), else picked. */
  shade?: number;
  structure?: 'roofed' | 'ruin';
  /** A castle drawn by hand (its outline in `poly`, `structure` too): the side its gate is in
   * (corner `gate` to the next; else facing the nearest road; more in `gates`, its sides), a
   * keep (its shape: square, round or drawn in `keep_poly`; else rolled), buildings round the
   * yard (both by default), its moat (none, dry or wet; else rolled). */
  gate?: number;
  keep?: boolean;
  keep_shape?: KeepShape;
  keep_poly?: [number, number][];
  yard_buildings?: boolean;
  moat?: Moat;
  /** A wall drawn by hand: its line (world ft), the corners that are gates, a ring or not. */
  pts?: [number, number][];
  gates?: number[];
  closed?: boolean;
  /** An entrance in a building's cellar (`b:<layout>:<id>`) and the way down there (`stairs` or `trapdoor`). */
  in_building?: string;
  way?: string;
  removed?: boolean;
  /** A drawn building on a deck put down by hand: a bridge (`v:<id>`) or a pier or breakwater
   * (`q:<id>`); its ground floor at the deck's level, no cellar (R5 M20). */
  on?: string;
  /** A castle's pieces changed by hand (`worldgen::world::CastlePiece`), by `keep` or a yard
   * building's number (`b:<layout>:<number>`). */
  pieces?: Record<string, CastlePiece>;
  /** A plot's fill (kind `plot`: an outline `poly` filled as a hand town's block is;
   * `worldgen::town::hand::BlockFill` as JSON). */
  fill?: Record<string, unknown>;
}

/** A stamp asked for (`placing::stamp_json`): a kind at a point, a size or width x depth, a facing (degrees, 0 = +x), the id it would get. */
export interface StampAsk {
  x: number;
  y: number;
  func: string;
  size?: 'small' | 'usual' | 'large';
  width?: number;
  depth?: number;
  facing?: number;
  clear?: boolean;
  id: string;
}
/** A building generated that a placement takes away. */
export interface Removal {
  id: string;
  name: string;
  function: string;
  at: [number, number];
}
export interface StampAnswer {
  poly: [number, number][];
  x: number;
  y: number;
  name: string;
  facing: number | null;
  width: number;
  depth: number;
  moved_ft: number;
  removed: Removal[];
}
/** A row asked for (`placing::row_json`): along a path, on a side or both, of a ward's lots or the kinds given. */
export interface RowAsk {
  line: [number, number][];
  side: 'left' | 'right' | 'both';
  ward: string;
  kinds: string[];
  depth?: number;
  gaps: number;
  clear: boolean;
  seed: number;
  first_id: number;
  /** A road along the row (kind: a `PathKind`; width 3-40 ft). */
  road?: { kind: string; width?: number };
}
export interface RowLot {
  poly: [number, number][];
  x: number;
  y: number;
  func: string;
  floors: number;
  name: string;
  existing: string | null;
  faces?: [[number, number], [number, number]];
}
/** A row's road (`placing::row`): the line as laid, and what writes it (one of `path`, `streets`, `towns`, `kept`). */
export interface RowRoadAnswer {
  as: 'road' | 'street';
  town?: number;
  line: [number, number][];
  removed: string[];
  path?: DrawnPath;
  streets?: Record<string, unknown | null>;
  wharves?: Record<string, unknown | null>;
  towns?: Record<string, TownEdit | null>;
  street_ids?: string[];
  street_id?: string;
  kept?: string;
}
export interface RowAnswer {
  road?: RowRoadAnswer | null;
  lots: RowLot[];
  removed: Removal[];
  refused: { x: number; y: number; why: string }[];
  refused_count: number;
  length_ft: number;
}

/** One of a castle's pieces changed by hand: a building's edit (`at` its middle as built; no
 * footprint) and its name. */
/** The answer of `bedit` for a castle's piece: its castle's `c:` id, its key in `pieces`, the
 * keep or not, and its edit as it is then (null: as generated). */
export interface PieceEdit {
  castle: string;
  key: string;
  keep: boolean;
  edit: CastlePiece | null;
}
export type CastlePiece = Omit<BuildingEdit, 'poly'> & { name?: string };

/** Widest a castle may be and narrowest (ft), longest a wall (`worldgen::world`), how far past
 * its outline a castle's or wall's layout reaches (`WORKS_MARGIN_FT`). */
export const CASTLE_MAX_FT = 1500;
/** A castle's keep's shapes and moats (`worldgen::world::KEEP_SHAPES`, `MOATS`). */
export type KeepShape = 'square' | 'round' | 'drawn';
export type Moat = 'none' | 'dry' | 'wet';
export const CASTLE_MIN_FT = 80;
export const WALL_MAX_FT = 3000;
/** Longest a closed wall (a ring) may be, ft round (`worldgen::world::WALL_RING_MAX_FT`). */
export const WALL_RING_MAX_FT = 24000;
export const WORKS_MARGIN_FT = 40;

export type RoofStyle = 'hip' | 'battlements' | 'cone';
export const ROOFS: RoofStyle[] = ['hip', 'battlements', 'cone'];
/** Roof colours (`worldgen::world::TINTS`, as `battlePrep.ROOF_TINTS`). */
export type Tint = 'terracotta' | 'slate' | 'thatch' | 'shingle' | 'moss';
export const TINTS: Tint[] = ['terracotta', 'slate', 'thatch', 'shingle', 'moss'];
/** Roof materials by id (`worldgen::town::roof::MATERIALS`, append-only) and their base colours (the
 * old tints' materials keep the tints' colours: `roof::TINT_MATERIAL`). */
export type RoofMaterial = 'thatch' | 'slate' | 'clay_tile' | 'shingle' | 'turf' | 'lead' | 'copper' | 'glazed_tile' | 'flat_clay' | 'hide' | 'plank' | 'whitewash' | 'stone';
export const ROOF_MATERIALS: RoofMaterial[] = ['thatch', 'slate', 'clay_tile', 'shingle', 'turf', 'lead', 'copper', 'glazed_tile', 'flat_clay', 'hide', 'plank', 'whitewash', 'stone'];
/** The materials the old tints map to (`roof::TINT_MATERIAL`). */
export const TINT_MATERIAL: Record<Tint, RoofMaterial> = { terracotta: 'clay_tile', slate: 'slate', thatch: 'thatch', shingle: 'shingle', moss: 'turf' };
export const ROOF_MATERIAL_RGB = [0xa88b5c, 0x5c6470, 0x995742, 0x7a6656, 0x7f7b54, 0x7c848c, 0x6a9282, 0x4a718b, 0xb89a72, 0x9c8a6c, 0x86684a, 0xc4baa7, 0x8d8a82];
/** Five colours per material (`Look.shade` 0-4, matte; index 2 is `ROOF_MATERIAL_RGB`, the usual one). Slate
 * keeps two hues (cool grey, warm grey-moss), glaze is dusty (saturation about 0.5), plaster a warm cream. */
export const ROOF_SHADES = [
  [0x86775f, 0x957d58, 0xa88b5c, 0xb59765, 0x765d42], // thatch: silver, dark straw, golden, bright gold, brown
  [0x5f6874, 0x71766a, 0x5c6470, 0x6a6560, 0x55575c], // slate: blue-grey, moss-grey, grey, warm grey, dark
  [0x864c40, 0xb06a44, 0x995742, 0x86614c, 0xac6350], // clay tile: dark brick, orange, red, brown, salmon
  [0x665a50, 0x6e6a5c, 0x7a6656, 0x8a7258, 0x8c8074], // shingle: dark, grey-green, brown, warm, weathered
  [0x6a7c4e, 0x727756, 0x7f7b54, 0x8c8454, 0x6a6a4b], // turf: green, moss, olive, dry, dark
  [0x6c747c, 0x727e86, 0x7c848c, 0x8a8c88, 0x9098a0], // lead: dark, blue, grey, warm, light
  [0x9a6a4c, 0x7c8e68, 0x6a9282, 0x5f877a, 0x7fa192], // copper: bare, olive, verdigris, teal, pale
  [0x4f7868, 0x476486, 0x4a718b, 0x5d8790, 0x675786], // glazed tile: green, deep blue, blue, turquoise, violet
  [0xa8885f, 0xb0a080, 0xb89a72, 0xc4a47c, 0x9a8268], // flat clay: darker, pale, sand, warm, umber
  [0x7c6a52, 0x8a7c64, 0x9c8a6c, 0xaa9a7c, 0x9a7e66], // hide: dark, grey, tan, pale, reddish
  [0x6e5a44, 0x7a6a56, 0x86684a, 0x94744e, 0x8a6a56], // plank: dark, grey, brown, warm, rosy
  [0xb6b3aa, 0xbdb399, 0xc4baa7, 0xcbc1ae, 0xbdac96], // whitewash: cool, cream, lime, bright, sand
  [0x74726c, 0x827e72, 0x8d8a82, 0x9b968a, 0x7a7770], // stone: dark, warm, grey, pale, weathered
];
/** Felt for a round roof (`Roof.shade` 0-4): cream, off-white, grey, warm cream, dusty grey (a felt hall's). */
export const FELT = [0xc9bea6, 0xc4bdad, 0xaea595, 0xc2b394, 0xa39a88];
/** A yurt's felt (`Roof.shade` 0-7, R5 towns 115 `town::yurts::felt`): cream, off-white, grey, warm cream, dusty
 * grey, smoke-browned, cream (with a red band), pale (with a sooty crown). */
export const YURT_FELT = [0xd3c6a6, 0xe0dccf, 0xa9a192, 0xcdb88f, 0xb5ad9d, 0x8a6843, 0xd0c3a2, 0xdcd6c6];
/** A roof material's colour in a variant (0-4; bad values: the usual). */
export const roofRgb = (material: number, shade = 2): number => ROOF_SHADES[material]?.[shade] ?? ROOF_MATERIAL_RGB[material] ?? ROOF_MATERIAL_RGB[3];
/** Roof shapes by id (`worldgen::town::roof::SHAPES`, append-only). */
export type RoofShape = 'hip' | 'gable' | 'cone' | 'dome' | 'flat' | 'round';
export const ROOF_SHAPES: RoofShape[] = ['hip', 'gable', 'cone', 'dome', 'flat', 'round'];
export const MAX_FLOORS = 8;
/** What a building drawn by hand can be. */
export interface BuildingFuncs {
  businesses: { key: string; name: string; category: string }[];
  homes: { key: string; name: string }[];
}

export type SiteSize = 'small' | 'medium' | 'large' | 'huge';
export const SITE_SIZES: SiteSize[] = ['small', 'medium', 'large', 'huge'];
/** Themes by kind of site underground (`worldgen::under::THEMES`; each kind's first is its usual one). */
export const THEMES: Record<string, string[]> = {
  dungeon: ['dungeon', 'prison', 'temple', 'wizard_lair', 'bandit_hideout', 'dwarven_hall', 'goblin_warren', 'flooded_vault', 'cult_sanctum', 'labyrinth', 'collapsed_fortress', 'sunken_temple'],
  crypt: ['crypt', 'tomb', 'ossuary'],
  catacombs: ['catacombs'],
  cave: ['cave', 'fungal', 'crystal', 'ice', 'flooded', 'beast_den'],
  mine: ['mine'],
  lava_tube: ['lava_tube'],
};

export type Stance = 'hostile' | 'unfriendly' | 'neutral' | 'friendly' | 'allied';
export const STANCES: Stance[] = ['hostile', 'unfriendly', 'neutral', 'friendly', 'allied'];

/** A non-player character (`worldgen::world::Npc`), authored by the user or an agent. */
export interface Npc {
  name: string;
  appearance: string;
  mannerisms: string;
  attitude: { stance: Stance; text: string };
  goals: string;
  notes: string;
  tags: string[];
  /** A picture in the asset store (`world/assets.ts`). */
  portrait?: string;
  /** Where they are: a place id; inside, a level and a point (world ft). */
  location?: { id: string; level?: number; x?: number; y?: number };
  /** Free text: alive, dead, missing... */
  status?: string;
}

export type PlotStatus = 'idea' | 'active' | 'resolved';
export const PLOT_STATUSES: PlotStatus[] = ['idea', 'active', 'resolved'];

/** A plot point (`worldgen::world::Plot`): tied to places (`anchors`) and NPCs. */
export interface Plot {
  title: string;
  text: string;
  status: PlotStatus;
  anchors: string[];
  npcs: string[];
  tags: string[];
}

/** Notes on a place (any id), a room (`r:`) or a point in a room (`m:<site>:<level>:<id>`, with
 * `at`, the point in the location's own grid units; `world.rs` `Note`). */
export interface Note {
  text: string;
  tags?: string[];
  at?: [number, number];
}

/** How a link put down by hand goes (`links::WAYS`; the first is the default). */
export const LINK_WAYS = ['stairs', 'ladder', 'passage', 'trapdoor', 'rope', 'portal'] as const;
/** One end of a link put down by hand: a level id and a square on it. */
export interface LinkEnd {
  level: string;
  x: number;
  y: number;
}
/** A paired exit put down by hand (`Edits.links`). */
export interface LinkEdit {
  a: LinkEnd;
  b: LinkEnd;
  way: string;
  name?: string;
}

/** The sites at either end of the paired exits that differ between two edits' `links`. */
export function linkSites(a: Record<string, LinkEdit> | undefined, b: Record<string, LinkEdit> | undefined): Set<string> {
  const out = new Set<string>();
  if (a === b) return out;
  const [pa, pb] = [a ?? {}, b ?? {}];
  const site = (level: string) => level.slice(2, level.lastIndexOf(':'));
  for (const id of new Set([...Object.keys(pa), ...Object.keys(pb)])) {
    if (JSON.stringify(pa[id]) === JSON.stringify(pb[id])) continue;
    for (const l of [pa[id], pb[id]]) if (l) for (const e of [l.a, l.b]) out.add(site(e.level));
  }
  return out;
}

export interface Edits {
  renames?: Record<string, string>;
  notes?: Record<string, Note>;
  hidden?: string[];
  created?: Created[];
  /** `n:<id>` → NPC. */
  npcs?: Record<string, Npc>;
  /** `p:<id>` → plot point. */
  plots?: Record<string, Plot>;
  /** `o:<id>` → an object put on the battlemap by hand. */
  objects?: Record<string, Placed>;
  /** `x:<id>` → generated objects taken away. */
  cleared?: Record<string, Clear>;
  /** Asset id → an uploaded sprite's name and rules. */
  sprites?: Record<string, SpriteMeta>;
  /** `u:<layout>:<k>` → an underground site designed by hand, built instead of generated. */
  designs?: Record<string, SiteDesign>;
  /** `v:<id>` → a bridge, ford or ferry put down by hand. */
  crossings?: Record<string, Crossing>;
  /** `b:<layout>:<id>` → one of the world's own buildings changed or taken away. */
  buildings?: Record<string, BuildingEdit>;
  /** `<layout>` → a town laid out anew by hand (the ward editor). */
  towns?: Record<string, TownEdit>;
  /** `<layout>` → one of the world's own settlements laid out in another culture (0 north, 1 west,
   * 2 south, 3 dry, 4 steppe; `worldgen::town::culture`). One added by hand keeps it in `Created.culture`. */
  cultures?: Record<string, number>;
  /** `g:XXXXX:YYYYY` → 16 × 16 squares of ground brushed by hand (`worldgen::ground::GroundBlock`:
   * tenths of a foot, row-major, as base64url zigzag-delta varints, a 0 delta followed by a byte
   * counting further zeros). Block indices zero-padded, x first. */
  ground?: Record<string, string>;
  /** `h:<id>` → a plateau or pit drawn by hand (or a rock islet: `kind`). */
  plateaus?: Record<string, Plateau>;
  /** Water drawn by hand (`e:<id>`: `worldgen::waters`). */
  waters?: Record<string, Water>;
  /** `q:<id>` → a quay, pier or breakwater built by hand. */
  wharves?: Record<string, Wharf>;
  /** `overland` → the hex grid drawn over the map for a hex crawl (`render/hex.ts`). */
  hexes?: Record<string, HexGrid>;
  /** A town laid out by hand as a street graph (`worldgen::town::hand`): `<layout>` its header,
   * `<layout>/n<id>` a junction, `/e<id>` a street, `/k<id>` a block, `/r<id>` a row, `/a<id>` a
   * painted area, `/m<id>` a wall line; `lanes/…` lanes outside any town. Positions in world ft. */
  streets?: Record<string, unknown>;
  /** Roads, streets and paths taken off the map by hand (`rd:` stretch, `mt:` track, `tr:` town street or lane, `pp:` park path): the ground shows. */
  removed_roads?: string[];
  /** Fences and hedges drawn by hand (`f:<n>`) and the gaps the eraser cut (`fg:<n>`; `worldgen-model::fences`). */
  fences?: Record<string, Fence>;
  fence_gaps?: Record<string, FenceGap>;
  /** Paired exits put down by hand (`j:<id>`; `crates/worldgen/src/links.rs`): two ends, each a
   * level (`l:<site>:<level>`, levels from the bottom: a site underground `u:`/`k:`, a sewer section
   * `w:<layout>:<sx>:<sy>`, a building's cellar `b:`) and a square on it; each shows on its own map as a
   * `link` item with a way in the level's `links` (`far`). `way` one of `LINK_WAYS`. */
  links?: Record<string, LinkEdit>;
  /** Roads drawn by hand on the ground (`rw:<id>`: `worldgen::paths`). */
  paths?: Record<string, DrawnPath>;
  /** Towns' members set by hand (`<layout>` of a town or a `place` → ids joined, `add`, or kept
   * out however near, `remove`: `worldgen::world::Members`, `town::members`): drawn things `c:<n>`,
   * and generated buildings `b:<layout>:<id>` pushed out into a place. Joins by distance are never stored. */
  members?: Record<string, { add?: string[]; remove?: string[] }>;
}

/** An overland hex grid (`worldgen::world::HexGrid`; each field left out: its default, `HEX_DEFAULTS`):
 * `size_mi` across the flats (centre to neighbouring centre), flat-topped or pointy, its lines'
 * colour (`#rrggbb`), width (px) and opacity (0-1), the lattice moved by `ox_mi`/`oy_mi`, numbers. */
export interface HexGrid {
  size_mi?: number;
  flat?: boolean;
  color?: string;
  width?: number;
  opacity?: number;
  ox_mi?: number;
  oy_mi?: number;
  numbers?: boolean;
}

/** A flat top or a pit (`worldgen::ground::Plateau`): ground inside `pts` (world ft) eases to
 * `level` (ft); outside, back to the ground over `edge` ft (absent or 0: a sheer step). An
 * `islet` is rock standing out of the water (its outline made from a centre and radius). `floor`:
 * what its top draws as (absent: rock on a pit's floor 15 ft or more below the ground). */
export interface Plateau {
  pts: [number, number][];
  level: number;
  edge?: number;
  kind?: 'plateau' | 'islet';
  floor?: 'rock' | 'ground';
}

export type WaterKind = 'inlet' | 'basin' | 'lake' | 'pond' | 'moat' | 'stream';

/** A road drawn by hand's type (`worldgen::paths::PathKind`). */
export type PathKind = 'footpath' | 'track' | 'gravel' | 'paved' | 'cobbled';

/** A road drawn by hand (`worldgen::paths::Path`): its points (world ft, 0.5 ft), type, width (ft;
 * absent: the type's) and name. */
export interface DrawnPath {
  pts: [number, number][];
  type: PathKind;
  width?: number;
  name?: string;
}

/** Water drawn by hand (`worldgen::waters::Water`): an outline (world ft, 3 to 128 points, rounded
 * to 0.5 ft, 5 miles across at most) and its surface `level` (ft, the generator's datum). Inside,
 * the ground falls under it; overlapping outlines are one water. */
export interface Water {
  pts: [number, number][];
  /** Legacy: water drawn before has a kind (its look, depth and shore); water drawn now has none,
   * its look following from its shape, depth and level (`water.ts` `lookOf`). */
  kind?: WaterKind;
  level: number;
  /** How deep its bed lies under its level (ft, 1 to 20; none: its kind's, or a lake's). */
  depth?: number;
  /** Dug: straight sides, a stone edge (none: as its kind, else natural). */
  dug?: boolean;
  /** A brushed line (`pts` its line, 2 points or more), its water the band `width` ft wide. */
  line?: boolean;
  width?: number;
  /** A brushed line come round to its start (a ring). */
  closed?: boolean;
  /** An open line's level evenly along it (`waters::stream_levels`). */
  levels?: number[];
}

export type WharfKind = 'quay' | 'pier' | 'breakwater';

/** A quay, pier or breakwater built by hand (`worldgen::world::Wharf`), along `pts` (world ft, 2 to
 * 16 points, each side ≥ 10 ft, ≤ 2,000 ft in all). A quay's line is its water edge, its paving
 * `width` ft to the dry side; a pier or breakwater runs from `pts[0]` on the bank out over the
 * water. `width` absent: 20; `material` (a pier's) absent: timber; a breakwater is stone. */
export interface Wharf {
  kind: WharfKind;
  pts: [number, number][];
  width?: number;
  material?: CrossingMaterial;
}

/** Widths (ft) by kind (`WharfKind::widths`) and the width a new one gets. */
export const WHARF_WIDTH: Record<WharfKind, [number, number, number]> = { quay: [10, 60, 20], pier: [6, 30, 10], breakwater: [12, 60, 24] };

/** A town's layout changed by hand (`worldgen::world::TownEdit`): `at` is the town's centre (the
 * edit is set aside if the town is laid out elsewhere); corners of its patches moved (`v` the
 * plan's corner, `from` where it was planned), patches set (`p` the plan's patch, `at` its middle
 * as planned; `close`: a temple's kept as a close or not), walls on or off, squares set where
 * streets meet (`v` the corner, `at` where it was planned; `kind` a `SQUARE_KINDS` or "none"). */
export interface TownEdit {
  at: [number, number];
  corners?: { v: number; from: [number, number]; to: [number, number] }[];
  patches?: { p: number; at: [number, number]; ward?: string; lots?: string; merge_with?: number; reroll?: number; close?: boolean; park?: string; district?: string }[];
  /** Districts made by hand, in the order made (a patch's `district` may name one). */
  districts?: string[];
  walls?: boolean;
  squares?: { v: number; at: [number, number]; kind: string }[];
  /** Streets added by hand (a row's road), by id (`ts:<layout>:<id>`): the line (ft), its kind and width. */
  streets?: Record<string, { pts: [number, number][]; type?: string; width?: number }>;
  /** The town's own streets, lanes and park paths taken off the map by hand (`tr:`, `pp:` ids). */
  removed?: string[];
  /** The town's generated fences and hedges changed by hand, by their local key (`fencecut`). */
  fences?: Record<string, FenceChange>;
}

/** A fence's or hedge's kind, as the world file writes it (`FenceKind`). */
export type FenceKind = 'hedgerow' | 'hedge' | 'dry_stone_wall' | 'post_and_rail' | 'wattle' | 'garden_wall' | 'low_wall' | 'picket' | 'iron_railing' | 'stockade' | 'high_wall';

/** A fence or hedge drawn by hand: its line (ft; round to its first point when `closed`) and its gates (ft along it from `pts[0]`). */
export interface Fence {
  kind: FenceKind;
  pts: [number, number][];
  closed?: boolean;
  gates?: number[];
}

/** A gap cut by the eraser: discs of radius `r` ft along its stroke. */
export interface FenceGap {
  pts: [number, number][];
  r?: number;
}

/** A generated run changed by hand: gone, another kind, gates (ft along it). */
export interface FenceChange {
  gone?: boolean;
  kind?: FenceKind;
  gates?: number[];
}

/** What changing fences does (`fencecut::ChangeOut`): set (null: delete) each `fences`, `fence_gaps` and `towns` entry in one
 * edit, draw `rects` (battlemaps) and `site_rects` (site tiles too) again. */
export interface FenceCut {
  fences: Record<string, Fence | null>;
  fence_gaps: Record<string, FenceGap | null>;
  towns: Record<string, TownEdit | null>;
  rects: Rect[];
  site_rects: Rect[];
  done: string[];
  refused: { id: string; why: string }[];
}

/** A town whose fences were changed by hand, for the Gazetteer (`fencecut::changed`). `found` false: set aside (the town moved or is by hand now). */
export interface ChangedFences {
  town: string;
  name: string | null;
  ids: string[];
  found: boolean;
}

/** What a patch of a town can be given (`worldgen::world::TOWN_WARDS`; `empty`: open ground). */
export const TOWN_WARDS = ['plaza', 'castle', 'temple', 'merchant', 'craft', 'noble', 'common', 'slum', 'docks', 'military', 'farm', 'park', 'empty'] as const;
export type TownWard = (typeof TOWN_WARDS)[number];
/** Lot sizes, smallest first (`worldgen::world::LOT_SIZES`). */
export const LOT_SIZES = ['small', 'medium', 'large', 'huge'] as const;
export type LotSize = (typeof LOT_SIZES)[number];
/** A generated block fill's storeys (`worldgen::town::hand::HEIGHTS`): one each, the ward's usual, one more. */
export const HEIGHTS = ['low', 'usual', 'tall'] as const;
export type Height = (typeof HEIGHTS)[number];
/** Squares a corner where streets meet can be given (`worldgen::world::SQUARE_KINDS`). */
export const SQUARE_KINDS = ['crossroads', 'well', 'fountain', 'green'] as const;
export type SquareKind = (typeof SQUARE_KINDS)[number];
/** How a park can be laid out (`worldgen::world::PARK_KINDS`). */
export const PARK_KINDS = ['formal', 'common', 'cemetery', 'horses'] as const;
/** A horse park's marks on the plan (`town::horses::plan_polys`): [rgb, alpha] by the variant. */
export const HORSE_PARK_PLAN: [[number, number, number], number][] = [
  [[0.33, 0.23, 0.14], 0.95],
  [[0.56, 0.56, 0.34], 0.85],
  [[0.38, 0.25, 0.16], 0.9],
  [[0.86, 0.74, 0.4], 0.95],
  [[0.42, 0.31, 0.2], 0.9],
];
export type ParkKind = (typeof PARK_KINDS)[number];

/** A patch of a town as the ward editor sees it (`town::wards::plan_json`): its corners (plan
 * corner ids), ward as it is now (and as generated, if set by hand), district, neighbours. */
export interface TownPatch {
  patch: number;
  /** Its middle as planned (world ft). */
  at: [number, number];
  corners: number[];
  ward: TownWard;
  ward_generated?: TownWard;
  /** Inside the town (else its farms and sprawl). */
  in_town: boolean;
  neighbours: number[];
  district?: { name: string; id: string };
  /** The patch whose lots it shares (its district group). */
  lots_like?: number;
  lots?: LotSize;
  reroll?: number;
  merged_with?: number;
  /** A temple's patch kept as a close round the temple (and as generated, if set by hand). */
  close?: boolean;
  close_generated?: boolean;
  /** Why a close can't be set on this temple's patch. */
  close_refused?: string;
  /** A park's patch: how it is laid out, and whether that was set by hand. */
  park?: ParkKind;
  park_by_hand?: boolean;
  /** Why the patch can't be made a park (too wet, no street to it, outside the town, too small, a cliff). */
  park_refused?: string;
  /** A park's patch built on as another ward (its block too small once its streets were cut). */
  ward_planned?: TownWard;
}

/** A corner of a town's patches: where it stands now (world ft), where it was planned (if moved). */
export interface TownCorner {
  corner: number;
  at: [number, number];
  planned?: [number, number];
  /** On the water or a river: it never moves. */
  pinned?: boolean;
  gate?: boolean;
  /** A gate put in here by hand, or a generated one taken out. */
  gate_by_hand?: boolean;
  gate_removed?: boolean;
  /** On the town's wall, and its neighbours along it (null at an open wall's end). */
  wall?: boolean;
  /** The generated tower standing at this corner was taken down by hand. */
  tower_removed?: boolean;
  wall_next?: [number | null, number | null];
  /** Streets meet here: a square can open. */
  junction?: boolean;
  /** Streets meet here but no square can open, and why. */
  square_refused?: string;
  /** The square there, and how far out it is cut (ft). */
  square?: SquareKind;
  square_ft?: number;
  /** As generated, when set by hand ('none': no square). */
  square_generated?: SquareKind | 'none';
}

/** A district in the plan (`get_town_plan`'s `districts`). */
export interface TownDistrict {
  id: string;
  name: string;
  kind: string;
  patches: number[];
  by_hand?: boolean;
  changed?: boolean;
}

/** The gates and towers of a town's wall set by hand (`walls.by_hand` of the plan). */
export interface TownWallsByHand {
  gates_added: { corner: number; id: string; name?: string | null; refused?: string }[];
  gates_removed: number[];
  towers_added: { id: string; corners: [number, number]; f: number }[];
  towers_removed: string[];
}

// ---- Hand towns (Roadmap 6): streets as records; Ask ops `streetplan` / `streetchange` ----

export type StreetClass = 'main' | 'street' | 'quay' | 'joint';
export type OpenKind = 'plaza' | 'lawn' | 'park' | 'garden' | 'market' | 'cemetery' | 'farmland';
export type BlockFill =
  | { kind: 'generated'; ward: string; lots?: string; height?: Height; reroll?: number; close?: boolean }
  | { kind: 'hand' }
  | { kind: 'open'; open: OpenKind; style?: string }
  /** A castle: as converted (its buildings kept), else built from the block's outline: gates (world ft, each on
   * the side nearest it, the main one first; none: facing the town's middle), keep and moat (none: rolled). */
  | { kind: 'castle'; hand?: boolean; gates?: P2[]; keep_shape?: 'square' | 'round'; moat?: 'none' | 'dry' | 'wet' };
export type P2 = [number, number];

/** A request to `streetchange` (`town::handedit::HandRequest`, `{op, …}`; world ft, ids the plan's numbers). */
export type StreetRequest =
  | { op: 'lay_out_by_hand' }
  | { op: 'back_to_generated' }
  | { op: 'add_street'; points: P2[]; width?: number; set_l?: number; set_r?: number; class?: StreetClass }
  | { op: 'delete_street'; street: number }
  | { op: 'set_street'; street: number; width?: number; set_l?: number; set_r?: number; class?: StreetClass; bridge?: boolean }
  | { op: 'move_junctions'; moves: { junction: number; to?: P2; by?: P2 }[] }
  | { op: 'set_square'; junction: number; square: string }
  | { op: 'set_block'; block: number; fill?: BlockFill; district?: string; seed?: string; reroll?: boolean }
  | { op: 'add_row'; street: number; side: 'l' | 'r'; depth: number; ward: string; lots?: string; gaps?: number; seed?: string }
  | { op: 'remove_row'; row: number }
  | { op: 'add_area'; poly: P2[]; open: OpenKind }
  | { op: 'remove_area'; area: number }
  | { op: 'add_wall'; pts: P2[]; closed?: boolean; spacing?: number; towers?: number[]; gates?: P2[]; name?: string }
  | { op: 'remove_wall'; wall: number }
  | { op: 'set_area'; area: number; open: OpenKind }
  | { op: 'set_wall'; wall: number; gates?: P2[]; name?: string; spacing?: number };

/** A hand town's plan (`town::handedit::plan_json`). */
export interface StreetPlan {
  layout: number;
  from: unknown;
  origin: P2;
  tier: unknown;
  fill: BlockFill;
  works: boolean;
  ward_edit: boolean;
  /** Points this close meet a junction or street (ft). */
  snap_ft: number;
  junctions: { id: number; p: P2; square: string | null; streets: number }[];
  streets: { id: number; a: number; b: number; bends?: P2[]; width: number; set_l: number; set_r: number; class: StreetClass; bridge?: boolean }[];
  blocks: { id: number; at: P2; fill: BlockFill; district: string | null; seed: string; gen: number; kept: boolean; pins: number; streets: number[] | null }[];
  rows: { id: number; street: number; side: string; depth: number; ward: string; lots: string | null; gaps: number | null }[];
  areas: { id: number; poly: P2[]; open: OpenKind }[];
  walls: { id: number; pts: P2[]; closed: boolean; spacing: number | null; gates: P2[]; name: string | null }[];
  problems: { key: string; why: string }[];
}

/** What a street change does (`HandChange::to_json`'s `change`: per Edits key, the record, null = removed) and its report. */
export interface StreetChange {
  streets?: Record<string, unknown | null>;
  wharves?: Record<string, Wharf | null>;
  towns?: Record<string, TownEdit | null>;
}
export interface StreetReport {
  op: string;
  records?: { set: unknown; removed: unknown };
  blocks?: { relaid: unknown; added: unknown; removed: unknown };
  buildings?: { before: number; after: number };
  /** Rectangles (x0, y0, x1, y1 ft) to draw again (`hintTown`). */
  rects: Rect[];
  /** What carried something (a business, rename, note, ...) and is gone. */
  lost?: { id: string; name?: string; what: string }[];
  problems?: { key: string; why: string }[];
  [more: string]: unknown;
}

/** A town's plan for the ward editor (Ask op `townplan`). */
export interface TownPlan {
  layout: number;
  center: [number, number];
  /** How far a corner may go from where it was planned. */
  max_move_ft: number;
  /** `towers`: the wall's towers as built now, gate towers (`gate_towers`) apart; `gates` as built. */
  walls: { built: boolean; generated: boolean; towers?: number; gate_towers?: number; gates?: number; by_hand?: TownWallsByHand };
  edited: boolean;
  patches: TownPatch[];
  /** The town's districts (a hand-made one: `by_hand`; one changed by a patch moved in or out: `changed`). */
  districts?: TownDistrict[];
  corners: TownCorner[];
  /** What of the town's edit no longer applies. */
  set_aside: string[];
}

/** Gates and towers by corner (`wallgates::request`): a tower by two corners and where between (f), or one corner. */
export interface TownWallsRequest {
  built?: boolean;
  gates_add?: number[];
  gates_remove?: number[];
  towers_add?: (number | [number, number] | [number, number, number])[];
  towers_remove?: (number | string)[];
}

/** A change asked of a town (`town::wards::TownRequest`). */
export interface TownRequest {
  moves?: { corner: number; to?: [number, number]; by?: [number, number]; as_generated?: boolean }[];
  equalize?: number[];
  relax?: { at: [number, number]; radius_ft: number; amount?: number };
  patches?: { patch: number; ward?: TownWard | 'auto'; lots?: LotSize | 'auto'; merge_with?: number | 'none'; reroll?: boolean; close?: boolean | 'auto'; park?: ParkKind | 'auto'; district?: string; as_generated?: boolean }[];
  squares?: { corner: number; kind: SquareKind | 'none' | 'auto' }[];
  walls?: boolean | 'auto' | TownWallsRequest;
  reset?: 'all' | 'corners' | 'patches' | 'squares';
}

/** What a change to a town does (`town::wards::change`). */
export interface TownReport {
  corners: { asked: number; moved: number; part_way: number; stayed: number };
  buildings: { before: number; after: number; added: number; taken_away: number };
  walls: { built: boolean; towers: number; gates: number };
  /** What a gate or tower asked for came to (only when one was asked). */
  wall?: {
    gates_added?: { corner: number; id: string; name?: string; towers?: string[]; way_out_ft?: number; built?: boolean }[];
    gates_removed?: { corner: number; id: string; name?: string }[];
    towers_added?: { id: string; x_ft: number; y_ft: number }[];
    towers_removed?: string[];
    refused?: { gate_add?: number; gate_remove?: number; tower_add?: unknown; tower_remove?: unknown; why: string }[];
  };
  /** Its open spaces' kinds (quays aside), and its parks'. */
  squares: string[];
  parks: ParkKind[];
  /** The rectangles (x0, y0, x1, y1 ft) to draw again. */
  rects: Rect[];
  functions_lost?: string[];
  unmerged?: number[];
  /** Patches that changed district (`from`/`to` are district ids). */
  districts_changed?: { patch: number; from: string; to: string }[];
  /** A renamed or noted building gone, handed to the new one that does its work (move the name and note). */
  renames_carried?: { from: string; to: string; name: string }[];
  /** A renamed building gone with no successor: its name stays on the old id. */
  renames_lost?: { id: string; name: string }[];
}

/** A generated building changed by hand (`worldgen::world::BuildingEdit`): `at` is its middle
 * as generated (the edit is set aside if another building stands at its id after the town is
 * laid out anew); every option left out is as generated. */
/** One of the world's own islets (`t0::islets`): rock in a river or off a coast; a watchtower may stand on it. */
export interface GenIslet {
  id: string;
  x: number;
  y: number;
  r: number;
  river: boolean;
  level: number;
  tower: { id: string; x: number; y: number } | null;
}

export interface BuildingEdit {
  at: [number, number];
  removed?: boolean;
  func?: string;
  floors?: number;
  poly?: [number, number][];
  roof?: string;
  tint?: string;
  material?: string;
  shape?: string;
  shade?: number;
  structure?: string;
}

export type CrossingKind = 'bridge' | 'ford' | 'ferry';
export type CrossingMaterial = 'timber' | 'stone';

/** A crossing put down by hand (`worldgen::world::Crossing`): from `a` to `b` (world ft, bank to
 * bank), `width` ft across; a bridge of old stone has `material` (absent: timber). */
export interface Crossing {
  kind: CrossingKind;
  a: [number, number];
  b: [number, number];
  width: number;
  material?: CrossingMaterial;
}

/** Limits on a crossing (`Crossing::WIDTH`, `LENGTH`; a ferry needs room for two 24-ft jetties). */
export const CROSSING_WIDTH: [number, number] = [5, 120];
/** A ford's or a ferry's widest (a bridge goes to `CROSSING_WIDTH[1]`). */
export const NARROW_CROSSING_MAX = 40;
export const CROSSING_LENGTH: [number, number] = [10, 2000];
export const FERRY_MIN_FT = 68;

/** An underground site or a building's interior designed by hand
 * (`worldgen::under::design::SiteDesign`): a copy of the generated one, changed. Walls are
 * derived; levels run bottom to top, as `Interior`'s. */
export interface SiteDesign {
  /** dungeon, crypt, catacombs, cave, mine, lava_tube; `building` (`b:` ids). */
  kind: string;
  theme?: string;
  origin: [number, number];
  axis: [number, number];
  nx: number;
  ny: number;
  /** The way in on the top level (under the entrance). */
  entry: [number, number];
  levels: DesignLevel[];
  /** A building's stair block: x, y, w, h (the same squares on every level it reaches). */
  stairs?: [number, number, number, number];
  /** A building's: what it was made for (its grid, footprint and storeys). */
  fingerprint?: string;
}

export interface DesignLevel {
  name: string;
  elevation_ft: number;
  natural: boolean;
  /** Room per square, run-length encoded: room (-1 rock), count, … */
  cells: number[];
  /** By index (empty ones stay, so room names hold). `shape`: drawn round, octagonal… (`render/
   * roomShapes.ts` `SHAPES`; absent or 0 square), while its squares fit the shape. */
  rooms: { kind: string; raise_ft: number; shape?: number }[];
  /** x, y, side (0 east, 1 south; a building's also 2 west, 3 north), flags (1 secret; a
   * building's door to the outside: 2 front, 4 back, 8 arch; a lock's colour 1-31: its low three bits in bits
   * 4-6 (`(lock & 7) << 4`), the rest in bits 8-11 (`(lock >> 3) << 8`); 128 a portcullis between two rooms). */
  doors: [number, number, number, number][];
  /** Props and the ways (`exit`, `up`, `down`), in order; a building's furniture. `back`: the
   * side it stands against (0 north 1 east 2 south 3 west), `variant`: one of its looks (0-3, more for a few);
   * absent: found from the walls, picked by position. */
  items: { kind: string; x: number; y: number; w: number; h: number; back?: number; variant?: number }[];
  /** A building's: storey (-1 cellar, 0 ground floor), an open roof, the stairs reach it. */
  z?: number;
  roof?: boolean;
  has_stairs?: boolean;
  /** Where the stairs come out here (a building's: off its stair block; a site's: its way up, else under the way down above). */
  landing?: [number, number];
}

/** Something wrong with a design (`worldgen::under::design::Problem`); `blocking` ones keep it
 * from being saved. `level`: index, bottom to top. */
export interface DesignProblem {
  level: number;
  at?: [number, number];
  text: string;
  blocking: boolean;
}

/** A design with the site it builds and its problems (Ask op `design`). */
/** A settlement's culture changed (`setculture`, `town::culture::Changed`): the edits to apply,
 * the cultures by name (`north`, `west`, `south`, `dry`, `steppe`), and the edits inside it set
 * aside, by field (`buildings`, `towns`, `designs`, `renames`, `notes`, `hidden`, `npcs`, `plots`).
 * Without a culture asked, only `from`. */
export type CultureChange = { from: string; to?: string; unchanged?: boolean; set_aside?: number; set_aside_by?: Record<string, number>; edits?: Edits };

export type DesignReply = { design: SiteDesign; interior: Interior; problems: DesignProblem[]; set_aside?: boolean } | { error: string };

/** What the designer offers (`worldgen::under::design::catalog_json`). */
export interface UnderCatalog {
  props: { kind: string; name: string; cover: number; blocks: boolean; height_ft: number; hazard: string | null; w: number; h: number }[];
  /** Every room kind. */
  rooms: string[];
  /** Each theme's own room kinds (its first room, its passages, its ledges first). */
  themes: { key: string; kind: string; first: string; passage: string; rooms: string[] }[];
  boss: string;
  max_levels: number;
  /** What a building's design can be given: furniture, indoor props (kinds of `props`) and room kinds. */
  building: { furniture: { kind: string; name: string; cover: number; blocks: boolean; height_ft: number; w: number; h: number }[]; props: UnderCatalog['props']; rooms: string[]; max_floors: number; max_cellars: number };
}

/** A battlemap object put down by hand (`worldgen::world::Placed`): a built-in kind (catalog
 * id) or an uploaded sprite (`s:<asset id>`), at a world position (ft). */
export interface Placed {
  kind: number | string;
  x: number;
  y: number;
  /** Radians. */
  rot: number;
  scale: number;
  variant: number;
  /** Drawn at `rot` (turned by hand or by a tool); absent: as before. */
  turned?: true;
}

/** Generated objects taken away (`worldgen::world::Clear`): with `kind`, the one of that kind
 * within a quarter square of (x, y); else all within `r` ft (of `kinds`, if given). */
export interface Clear {
  x: number;
  y: number;
  kind?: number;
  r?: number;
  kinds?: number[];
}

/** An uploaded sprite (`worldgen::world::SpriteMeta`). */
export interface SpriteMeta {
  name: string;
  /** Squares across at scale 1. */
  size: number;
  /** 0 none, 1 half, 2 three-quarters, 3 full. */
  cover: number;
  blocks_move: boolean;
  blocks_sight: boolean;
  difficult: boolean;
  height_ft: number;
}

/** Gazetteer questions answered by a generator worker (see `worldgen::gazetteer`). */
export type Ask =
  | { op: 'query'; x: number; y: number }
  /** Everything within `radius` ft of (x, y), topmost first (`pick::pick`). */
  | { op: 'pick'; x: number; y: number; radius: number }
  /** `rect` (x0, y0, x1, y1 ft) limits the search to that area. */
  | { op: 'search'; q: string; rect?: [number, number, number, number] }
  | { op: 'districts'; settlement: number }
  /** The roads someone named and the crossings put down by hand (`roadids::named_json`). */
  | { op: 'named' }
  /** Underground sites by name, kind, theme and what is attached (`undersearch::search_json`; `filters` JSON). */
  | { op: 'under'; q: string; filters: string }
  | { op: 'inview'; rect: [number, number, number, number] }
  /** `b:<settlement>:<building>` or `t:<settlement>:<tower>`. */
  | { op: 'interior'; id: string }
  /** Any place id: its name and position (`worldgen::agent::place`). */
  | { op: 'place'; id: string }
  /** What can be renamed in a layout (its index) or a building or site (its id). */
  | { op: 'names'; scope: string }
  /** The sites standing in settlements (`agent::parents_json`). */
  | { op: 'parents' }
  /** Town layout `town`'s members changed by hand, as `members` entries and created places (`town::members::change`); ids as JSON arrays. */
  | { op: 'members'; town: number; add: string; remove: string }
  /** The towns things could join and whether a generated building may leave its town (`town::members::join_reach`); ids as a JSON array. */
  | { op: 'reach'; ids: string; within: number }
  /** Where a site can be created near (x, y), and a name for it (`agent::creation_spot`). */
  | { op: 'spot'; kind: string; under?: string; id: string; x: number; y: number }
  /** Whether a building drawn by hand can stand there, its point and a name
   * (`agent::building_spot`). */
  | { op: 'building'; poly: [number, number][]; func?: string; id: string }
  /** A building of a kind set down at a point (`placing::stamp_json`). */
  | { op: 'stamp'; input: string }
  /** A row of buildings along a path (`placing::row_json`). */
  | { op: 'row'; input: string }
  /** Whether an outline can be a plot (`placing::parcel_check_json`). */
  | { op: 'parcel'; input: string }
  /** What a building can be (`agent::building_funcs_json`). */
  | { op: 'funcs' }
  /** A generated building's edit with a change made, or its removal (`agent::building_edit_json`). */
  | { op: 'bedit'; id: string; change: string }
  /** The same for many buildings at once: the answers in a list, in order. */
  | { op: 'bedits'; ids: string[]; change: string }
  /** The generated buildings with their middle inside a polygon (`agent::generated_buildings_in`). */
  | { op: 'bin'; poly: [number, number][] }
  | { op: 'works'; site: Created }
  /** Shop lots along a bridge, or its shops laid out again after a change (`wharf::shop_lots`,
   * `Edits::follow_crossing`; `input` JSON). */
  | { op: 'bridgelots'; input: string }
  /** The world's generated islets (`t0::islets`). */
  | { op: 'islets' }
  /** A hand islet round a point (`wharf::islet_at`). */
  | { op: 'islet'; key: string; x: number; y: number; radius: number; rise: number }
  /** Whether a wharf can be put down, with its lighthouse (`wharf::root_problem`). */
  | { op: 'wharfcheck'; wharf: string; lighthouse: boolean }
  /** An underground site as a design (the one given, else its own), changed by `action`, with
   * what it builds and its problems (`under::design::design_json`). */
  | { op: 'design'; id: string; design?: string; action?: string }
  /** What the designer offers (`under::design::catalog_json`). */
  | { op: 'undercat' }
  /** A town's plan for the ward editor (`town::wards::plan_json`). */
  | { op: 'townplan'; layout: number }
  /** A town changed as asked, without changing the edits (`town::wards::change_json`). */
  | { op: 'townchange'; layout: number; request: string }
  /** A hand town's street plan (`town::handedit::plan_json`). */
  | { op: 'streetplan'; layout: number }
  /** A hand town's streets changed as asked, without changing the edits (`town::handedit::change_json`); `request` is a `StreetRequest` as JSON. */
  | { op: 'streetchange'; layout: number; request: string }
  /** The villages and towns added by hand (`MadeSettlement`s). */
  | { op: 'made' }
  /** Whether a village or town can be added there (`t0::made::spot`), with what it would be. */
  | { op: 'madespot'; kind: string; id: string; x: number; y: number }
  /** Roads taken off the map or brought back, the edits left alone (`roadcut::cut`; `input` JSON `{ids, restore?, dry?}`). */
  | { op: 'roadcut'; input: string }
  /** What was taken off the map by hand, for the Gazetteer (`roadcut::removed`). */
  | { op: 'removedroads' }
  /** Fences changed, drawn or cut, the edits left alone (`fencecut::change`; `input` JSON as `fence_change_json`). */
  | { op: 'fencechange'; input: string }
  /** The fences within `r` ft of a point (`fencecut::near`). */
  | { op: 'fencesnear'; x: number; y: number; r: number; limit: number }
  /** The towns whose fences were changed by hand (`fencecut::changed`). */
  | { op: 'changedfences' }
  /** The overlay's bridge marks without the stretches taken off the map (`bridge_marks_json`). */
  | { op: 'bridgemarks' }
  /** One of the world's own settlements as its sketch pin would take it (`agent::settlement_facts`). */
  | { op: 'settlefacts'; id: string }
  /** Edits with those of the villages and towns added by hand that they remove, move or lay out
   * anew set aside (`t0::made::set_aside_changed`; `old`: the created sites before, JSON). */
  | { op: 'setaside'; old: string; edits: string }
  /** Edits with settlement `layout` laid out in `culture` (0 north … 4 steppe) and the edits inside
   * it set aside (`town::culture::change`; nothing changed in the worker); without `culture`, the
   * culture it is laid out in now. */
  | { op: 'setculture'; layout: number; culture?: number; edits: string }
  /** Edits with deleted village or town `id` restored, if its spot is free, and what was set aside
   * when it was deleted brought back where it fits (`t0::made::restore`). */
  | { op: 'restoremade'; id: string; edits: string }
  /** The fingerprints a remake of this world needs (`t0::remap::prints_before`; `edits` JSON). */
  | { op: 'remakeprints'; edits: string; played: number[] }
  /** Edits made on the world before following their places into this one, remade
   * (`t0::remap::follow_into`; `input` JSON: `RemakeInput`). */
  | { op: 'follow'; input: string };

/** What a world remade from a changed sketch needs to move the edits made on the world before
 * (`follow_json`): the edits, that world's places as its overlay has them, its layouts'
 * fingerprints (`remakePrints`), the settlements taken away on purpose and a play session's
 * layouts (`played`). */
export interface RemakeInput {
  edits: Edits;
  before: { features: Feature[]; first_created?: number; layout_ids?: number[] };
  was: [number, number][];
  gone: { id: string; name: string; li: number }[];
  played: number[];
}

/** The edits followed into the world remade (`followed`: null if nothing moved; `ops` turn the
 * edits given into them; `kept`, `parked`, `removed`: how many entries followed their places,
 * were set aside, or were set aside with a place taken away), and where each of the play
 * session's layouts goes (`targets`, null: it came out differently or is gone). */
export interface Remade {
  followed: { ops: EditOp[]; kept: number; parked: number; removed: number; moved: [string, string][] } | null;
  targets: (number | null)[];
}

/** A village or town added by hand, as the generator has it (`made_json`): its layout index,
 * `reach` (its layout's), `built` (built-up radius), `rect` (all its battlemaps change over: the
 * pad and the track) and its track's points (ft) from its edge to the road. */
/** One of the world's own settlements as its sketch pin would take it (`agent::settlement_facts`). */
export interface SettlementFacts {
  layout: number;
  /** The pin's stroke that made it (else null: generated). */
  pin: number | null;
  tier: 'metropolis' | 'city' | 'town' | 'village';
  /** The tier it was placed at (a pin replacing it says so). */
  planned: 'metropolis' | 'city' | 'town' | 'village';
  /** What it lives by (a pin kind). */
  kind: string;
  name: string;
  spot: [number, number];
  capital: boolean;
  port_ok: boolean;
  fishing_ok: boolean;
}

export interface MadeSettlement {
  id: string;
  layout: number;
  tier: string;
  population: number;
  x: number;
  y: number;
  reach: number;
  built: number;
  rect: Rect;
  track: [number, number][];
  /** The link's road class (`RoadClass`: 2 a village's track, 1 a town's road) and bed width (ft). */
  class: number;
  width: number;
  /** Its culture (0 north … 4 steppe) and what it lives by (`SettleKind::name`): a change to
   * either lays it out anew. */
  culture: number;
  lives: string;
}

/** A name to rename (`worldgen::agent::name_entry`): `generated` is the name before renames. */
export interface NameEntry {
  id: string;
  kind: string;
  name: string;
  generated: string;
  x_ft?: number;
  y_ft?: number;
  /** A building or site with levels and rooms. */
  enter?: boolean;
  hidden?: boolean;
}

/** The sites standing in settlements (`worldgen::agent::parents_json`): `[site layout, settlement]`
 * pairs, with the counts of settlements (`n`) and of sites not created by hand (`base`) the
 * layout indices assume. */
export interface Parents {
  n: number;
  base: number;
  pairs: [number, number][];
  /** Each drawn thing's town (a layout index, past `n` for a town or place made by hand) and why: by the
   * 300 ft rule (`gap`, with the distance in ft) or by hand. */
  owners?: [number, number, 'gap' | 'hand', number | null][];
  /** Generated buildings pushed out of their town: `[place, town, building ids]`. */
  pushed?: [number, number, string[]][];
  /** Settlements and places by feature id: `[id, kind, extent ft, people]`. */
  sizes?: [string, string, number, number][];
}

/** What `members` answers: the `Edits.members` entries to set (`{}` takes one away), the places to append to
 * `Edits.created`, and what was left as it was (`why` in plain words). */
export type MembersChange =
  | { members: Record<string, { add?: string[]; remove?: string[] }>; created: Created[]; refused: { id: string; why: string }[]; report?: unknown }
  | { error: string };

export interface PlaceInfo {
  id: string;
  name: string | null;
  /** The name before renames. */
  generated?: string | null;
  x: number;
  y: number;
}

/** A building's interior (`worldgen::interior::Interior`); grid units are 5-ft squares. */
export interface Interior {
  id: string;
  settlement: number;
  building: number;
  name: string | null;
  function: string;
  /** Underground: the site's theme (`THEMES`). */
  theme?: string;
  /** A city's sewers: the walkway's stonework (`under::sewer_stone`: bits 0-1 brick, dressed flags,
   * rubble; bit 2 warm stone). */
  stone?: number;
  /** A cave's look from the biome at its mouth (`under::look_of`): 1 green, 2 dry, 3 cold; absent for rock. */
  look?: number;
  /** World ft of grid corner (0, 0); grid x runs along `axis`, y along `across`. */
  origin: [number, number];
  axis: [number, number];
  across: [number, number];
  nx: number;
  ny: number;
  levels: InteriorLevel[];
  entry_level: number;
  /** Stair block: x, y, w, h (same on every level). */
  stairs: [number, number, number, number];
  /** Uploaded pictures standing on its floors (`InteriorItem.sprite` is index + 1). */
  sprites?: { asset: string; name: string }[];
  /** A building whose footprint is not the grid's rectangle: its outer wall as drawn (its middle on
   * the footprint), convex parts whose union is the footprint (grid units, clockwise on screen),
   * within half a square of its squares. */
  outlines?: [number, number][][];
}

/** Room kinds with no floor (`interior::OPEN_BELOW`, `ROOF_SLOPE`): a hall's height left open over
 * it (the room below shows through; it stops a step, not the eye) and a roof's slopes. */
export const OPEN_BELOW = 'open to below';
export const ROOF_SLOPE = 'roof';
/** A flat roof walked on inside its parapet (`interior::TERRACE`). */
export const TERRACE = 'roof terrace';
/** Pieces others stand on (`interior::hall::is_platform`): the kinds another piece may overlap (a low dais, a court's gravel walk). */
export const PLATFORMS = new Set(['low_dais', 'garden_path']);

export interface InteriorLevel {
  z: number;
  name: string;
  elevation_ft: number;
  cells: number[];
  /** `shape`: drawn round, octagonal, with an apse, chamfered or as a cavern (`roomShapes.ts`;
   * absent square). Only ever set while the room's squares are the shape's. */
  rooms: { kind: string; squares: number; raise_ft: number; center: [number, number]; shape?: number }[];
  /** `rail`: a railing where a gallery or passage looks down into a hall open two storeys (it
   * stops a step, not the eye); `arcade`: a town hall's open arcade along its front (piers with the
   * way open between them: `InteriorLayer.drawArcade`). */
  walls: { a: [number, number]; b: [number, number]; exterior: boolean; rail?: boolean; arcade?: boolean }[];

  /** `kind`: door, secret, front, back, arch, portcullis (an iron grate: shut, it stops a step but not the eye). */
  doors: { a: [number, number]; b: [number, number]; kind: string; rooms: [number, number]; lock?: number }[];
  windows: [number, number, number, number][];
  furniture: InteriorItem[];
  /** An open roof (battlements). */
  roof: boolean;
  /** The main stair block reaches this level (not a keep's tower tops). */
  has_stairs: boolean;
  /** Where the stairs come out, if set by hand (`DesignLevel.landing`). */
  landing?: [number, number];
  /** Hewn or natural rock (caves, mines): rooms open into each other; walls only at rock. */
  natural: boolean;
  /** Lines the floor follows (sewers: the streets above): x0, y0, x1, y1, reach (grid). */
  paths?: [number, number, number, number, number][];
  /** Floor beyond the grid's edge, as squares (sewer sections: `under::SEWER_RIM`). */
  rim?: [number, number][];
  /** Ways to other sites (cellar trapdoors, stairs to deep dungeons, sewer ladders): the
   * square and the site's id; the other site has a way straight back. */
  links?: {
    x: number;
    y: number;
    to: string;
    /** A link put down by hand (`Edits.links`): its id, and where it comes out (the far site `to`'s
     * level index, from the bottom, and square; that site's name; the link's own name; its way). */
    id?: string;
    far?: { level: number; x: number; y: number; name: string; label?: string; way: string };
  }[];
  /** Floor height per square (underground sites; `interior::height`): a character per square (row-major), its
   * code - 48 the height above `elevation_ft` in half feet; absent where the level is flat. Read it with
   * `gen/floor.ts`. */
  floor?: string;
  /** Ramp squares (indices into `cells`): rubble slopes joining two floors, at a height between them. */
  ramps?: number[];
}

export interface InteriorItem {
  kind: string;
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  cover: number;
  blocks_move: boolean;
  height_ft: number;
  /** Hazard rules (traps, lava, pits), if any. */
  hazard?: string;
  /** An uploaded picture (kind `sprite`): index + 1 into `Interior.sprites`. */
  sprite?: number;
  /** Set in a design: the side its back is against (0 north 1 east 2 south 3 west); absent: found
   * from the walls. */
  back?: number;
  /** Set in a design: one of its looks (0-3; more for a few: `looksOf`); absent: picked by position.
   * A `key`'s is its colour (1-7, the lock's `lock`). */
  variant?: number;
}

/** What taking roads away (or bringing them back) does (`roadcut::CutOut`): apply by setting `removed_roads` when
 * not null, dropping the `paths` (drawn roads, with their notes), and setting or deleting each `towns` key. */
export interface RoadCut {
  removed_roads: string[] | null;
  paths: string[];
  towns: Record<string, TownEdit | null>;
  /** Site-level tiles and battlemaps to draw again; `all_rects`: tiles at every level. */
  rects: Rect[];
  all_rects: Rect[];
  done: string[];
  /** The approaches to towns taken (or brought back) with a stretch. */
  also: string[];
  refused: { id: string; why: string }[];
}

/** A road, street or path taken off the map by hand, for the Gazetteer. `found` false: it is not there any more. */
export interface RemovedRoad {
  id: string;
  name: string | null;
  what: string;
  at: string | null;
  found: boolean;
}

/** What `pick` finds under or near a point (`worldgen::pick`): topmost first, then nearest. `id` is null
 * for kinds without ids yet (lane, street); `town` the owning settlement's
 * feature id; `removable` where a delete edit exists today; `bbox` world ft x0, y0, x1, y1. */
export interface Thing {
  kind:
    | 'entrance' | 'building' | 'drawn_building' | 'tower' | 'castle' | 'plot' | 'wall' | 'settlement' | 'site' | 'district'
    | 'road' | 'drawn_road' | 'crossing' | 'water' | 'harbour' | 'lane' | 'street' | 'path' | 'fence' | 'drawn_fence' | 'fence_gap';
  id: string | null;
  name: string | null;
  town: string | null;
  removable: boolean;
  bbox: [number, number, number, number];
  /** How it belongs to `town`: the 300 ft rule, by hand, or inside its streets; `gap_ft` is the distance. */
  why?: 'gap' | 'hand' | 'inside';
  gap_ft?: number;
  /** A line's points near the pick (road, street, lane, path, drawn road), for areas: `editor/select.ts inArea`. */
  line?: [number, number][];
  /** A fence or hedge's kind (not for topiary rows and gaps). */
  fence?: FenceKind;
}

/** What a point query or building search returns. */
export type Hit =
  | {
      kind: 'building';
      id: string;
      settlement: number;
      name: string | null;
      function: string;
      category: string | null;
      ward: string;
      /** The district it stands in: its generated name and id (`d:<layout>:<i>`, for renames). */
      district: string | null;
      district_id: string | null;
      floors: number;
      x: number;
      y: number;
      size_ft: number;
    }
  | { kind: 'district'; id: string; settlement: number; name: string; district_kind: string; x: number; y: number; size_ft: number }
  | { kind: 'settlement'; settlement: number; x: number; y: number; radius_ft: number };

/** A settlement's named district, for its curved map label (`worldgen::gazetteer::DistrictLabel`). */
export interface DistrictLabel {
  id: string;
  name: string;
  kind: string;
  /** Label baseline (world ft), left to right. */
  path: [number, number][];
}

/** A road someone named: its stretches (`rd:` ids, in order along it) named alike and meeting
 * end to end; `id` is the first. */
export interface NamedRoad {
  id: string;
  ids: string[];
  name: string;
  length_ft: number;
}

/** The roads someone named and the crossings put down by hand (`worldgen::roadids::named_json`). */
export interface NamedPlaces {
  /** What the map letters (hidden stretches left out): each road's line (world ft). */
  labels: (NamedRoad & { pts: [number, number][] })[];
  /** Every named road, hidden ones too, with its box (x0, y0, x1, y1 ft). */
  roads: (NamedRoad & { bbox: [number, number, number, number]; generated: string })[];
  /** `name`: given, else `generated` ("Stone bridge", "Ford", "Ferry"); x, y its middle. */
  crossings: { id: string; name: string; generated: string; x: number; y: number; length_ft: number }[];
  /** The same for quays, piers and breakwaters (`q:`). */
  wharves?: { id: string; name: string; generated: string; x: number; y: number; length_ft: number }[];
  /** The walls someone named (`c:` drawn, `<layout>/m<id>` a hand town's), lettered along their lines (world ft). */
  walls?: { id: string; name: string; pts: [number, number][] }[];
}

export interface Geom {
  map_w_ft: number;
  map_h_ft: number;
  max_level: number;
  domain_ft: number;
  t0_w: number;
  t0_h: number;
  t0_cell_ft: number;
  first_refine_level: number;
  tile_n: number;
  halo: number;
  sea_level_ft: number;
  world_hash: string;
}

/** Named feature from T0 (crates/worldgen-terrain/src/t0/features.rs). */
export interface Feature {
  id: string;
  kind: string;
  name: string;
  x: number;
  y: number;
  angle: number;
  extent_ft: number;
  elev_ft?: number;
  detail?: string;
}

export interface Overlay {
  features: Feature[];
  /** The sketch's conflicts, when the world was drawn. */
  conflicts?: Conflict[];
  /** The layout index of the first created site (`Edits.created[0]`), as the generator counts. */
  first_created?: number;
  /** What each layout is drawn from (`town::layout_identity`, 32 bits), by layout index: a
   * remade world lays out alike the places whose number here is the same. */
  layout_ids?: number[];
  /** The settlements added from Edit › Places: `[layout index, sketch stroke index]` (the pin each
   * stands for). */
  added?: [number, number][];
  /** Shipping between ports (`t0::SeaLane`), segments split at ports and where lanes meet. */
  sea_lanes?: SeaLane[];
  /** Roads over rivers, for region zoom's bridge marks: `[x, y, dir x, dir y, road class, kind (0 bridge, 1 ford, 2 ferry), river width ft]`. */
  bridges?: number[][];
}

/** A sea lane: `p` its line (world ft, x y pairs), `c` class (0 busy, 1 normal, 2 quiet), `f`
 * flow, `e` its ends (a settlement's layout index, or -1 where lanes meet). */
export interface SeaLane {
  c: number;
  f: number;
  e: [number, number];
  p: number[];
}

export const TILE_N = 256;
export const TILE_SAMPLES = TILE_N + 1;
export const HALO = 4;
export const KIND_TERRAIN = 1;
export const KIND_BATTLEMAP = 2;

export const tileId = (level: number, x: number, y: number) => `${level}/${x}/${y}`;
export const battleId = (level: number, x: number, y: number) => `b/${level}/${x}/${y}`;

export interface WantTile {
  /** Default terrain. */
  kind?: 'terrain' | 'battlemap';
  level: number;
  x: number;
  y: number;
  /** Lower runs first. */
  pri: number;
}

export interface TileMsg {
  level: number;
  x: number;
  y: number;
  base: number;
  min: number;
  max: number;
  gradRms: number;
  /** f16 RGBA per sample: (h - base, dh/dx, dh/dy, water - base). */
  tex: Uint16Array;
  /** u8 RGBA per sample: (primary biome, secondary biome, blend, coast distance). */
  biome: Uint8Array;
  rivers: RiverGeometry | null;
  /** Same capsule layout; w = bed width (ft), q = road class. */
  roads: RiverGeometry | null;
  sites: SiteGeometry | null;
  /** The edits epoch it was made with (set on receipt). */
  epoch?: number;
}

export interface GenStats {
  pending: number;
  inflight: number;
  cached: number;
  jobs: number;
  avgMs: number;
}

export type ToCoordinator =
  | { type: 'init'; world: WorldFile; workers: number }
  | { type: 'want'; tiles: WantTile[] }
  | { type: 'ask'; id: number; ask: Ask }
  /** New edits for every generator: the fields that changed, each whole as JSON (a big world's
   * edits are not sent whole on every change); `rects` (x0, y0, x1, y1 ft) are where created
   * sites changed: cached site-level tiles and battlemaps there are dropped. Results sent
   * afterwards carry `epoch`. `battleRects`: where only battlemaps change (objects put down or
   * taken away). `allRects`: where roads were taken away (tiles at every level). `groundRects`: where the ground was shaped by hand: the three finest levels'
   * tiles there are packed again (their samples, as generated, still serve finer tiles). */
  | { type: 'edits'; fields: Record<string, string>; patches: Record<string, EditsPatch>; rects: Rect[]; epoch: number; battleRects?: Rect[]; groundRects?: Rect[]; allRects?: Rect[] };

/** Some entries of a keyed edits field changed: those set (whole) and those taken out. */
export interface EditsPatch {
  set: Record<string, unknown>;
  unset: string[];
}

/** World ft: x0, y0, x1, y1. */
export type Rect = [number, number, number, number];

export type FromCoordinator =
  /** Sent as soon as the coordinator's WASM is up (long before T0), for renderer warm-up. */
  | { type: 'catalog'; catalog: string }
  | { type: 'ready'; geom: Geom; t0Ms: number; overlay: string; catalog: string }
  | { type: 'battlemap'; level: number; x: number; y: number; chunk: PreparedChunk; epoch: number }
  | { type: 'progress'; stage: string; frac: number }
  | { type: 'tile'; tile: TileMsg; epoch: number }
  | { type: 'stats'; stats: GenStats }
  | { type: 'answer'; id: number; json: string }
  | { type: 'error'; message: string };

export type ToGen =
  /** Load the WASM module now (while another worker builds T0). */
  | { type: 'warm' }
  | { type: 'init'; worldJson: string; t0: ArrayBuffer | null }
  | { type: 'job'; id: number; kind: number; level: number; x: number; y: number; parent: Float32Array | null }
  | { type: 'ask'; id: number; ask: Ask }
  /** Some of the edits' fields, each whole (a JSON object): `Ctx::set_edit_fields`; then
   * entries of others (`{field: EditsPatch}`): `Ctx::patch_edits`. */
  | { type: 'edits'; json: string | null; patch: string | null }
  /** The overlay's settlements and sites (`[{id, kind, name}]`) for a T0 loaded without it: `Ctx::set_place_names`. */
  | { type: 'places'; json: string };

export type FromGen =
  | { type: 'inited'; t0: ArrayBuffer | null; overlay: string | null; ms: number }
  | { type: 'progress'; stage: string; frac: number }
  | { type: 'done'; id: number; buf: ArrayBuffer | null; chunk?: PreparedChunk; ms: number }
  | { type: 'answer'; id: number; json: string }
  | { type: 'error'; id?: number; message: string };

export interface TerrainPayload {
  level: number;
  x: number;
  y: number;
  base: number;
  min: number;
  max: number;
  gradRms: number;
  padded: Float32Array;
  tex: Uint16Array;
  biome: Uint8Array;
  rivers: TileRivers;
  roads: TileRivers;
  sitePolys: TileSitePolys;
  siteLines: TileRivers;
  /**
   * Each site line's look (item 104; `payload.rs` "site looks"), in the same layout as `siteLines`, line
   * for line and point for point: (x, y, a, b). A wall's a its style code (`town::wallstyle::WallStyle::code`:
   * group, stone, material), a tower's a its style code with its shape (round, square, D, octagon) and b
   * the way a D faces (an octagon's or a square's sides run), radians; 0, 0 for the rest. Absent from an
   * older payload.
   */
  siteLooks?: TileRivers;
}

const TERRAIN_MAGIC = 0x5445_5252;
const HEADER_BYTES = 48;

export function parseTerrain(buf: ArrayBuffer): TerrainPayload {
  const dv = new DataView(buf);
  if (dv.getUint32(0, true) !== TERRAIN_MAGIC) throw new Error('bad terrain payload');
  const paddedLen = dv.getUint32(32, true);
  const texLen = dv.getUint32(36, true);
  const biomeLen = dv.getUint32(40, true);
  const texOffset = HEADER_BYTES + paddedLen * 4;
  const lines = (offset: number): [TileRivers, number] => {
    const nLines = dv.getUint32(offset, true);
    const nVerts = dv.getUint32(offset + 4, true);
    const startsOffset = offset + 8;
    const vertsOffset = startsOffset + (nLines + 1) * 4;
    return [
      { starts: new Uint32Array(buf, startsOffset, nLines + 1), verts: new Float32Array(buf, vertsOffset, nVerts * 4) },
      vertsOffset + nVerts * 16,
    ];
  };
  const [rivers, roadOffset] = lines(texOffset + texLen * 2 + biomeLen);
  const [roads, siteOffset] = lines(roadOffset);
  const nPolys = dv.getUint32(siteOffset, true);
  const nPolyVerts = dv.getUint32(siteOffset + 4, true);
  const polyStarts = siteOffset + 8;
  const polyAttrs = polyStarts + (nPolys + 1) * 4;
  const polyVerts = polyAttrs + nPolys * 4;
  const sitePolys = {
    starts: new Uint32Array(buf, polyStarts, nPolys + 1),
    attrs: new Uint32Array(buf, polyAttrs, nPolys),
    verts: new Float32Array(buf, polyVerts, nPolyVerts * 2),
  };
  const [siteLines, looksOffset] = lines(polyVerts + nPolyVerts * 8);
  const siteLooks = looksOffset + 8 <= buf.byteLength ? lines(looksOffset)[0] : undefined;
  return {
    level: dv.getUint32(4, true),
    x: dv.getUint32(8, true),
    y: dv.getUint32(12, true),
    base: dv.getFloat32(16, true),
    min: dv.getFloat32(20, true),
    max: dv.getFloat32(24, true),
    gradRms: dv.getFloat32(28, true),
    padded: new Float32Array(buf, HEADER_BYTES, paddedLen),
    tex: new Uint16Array(buf, texOffset, texLen),
    biome: new Uint8Array(buf, texOffset + texLen * 2, biomeLen),
    rivers,
    roads,
    sitePolys,
    siteLines,
    siteLooks,
  };
}

/** IEEE half → float (for CPU-side reads such as elevation under the cursor). */
export function halfToFloat(h: number): number {
  const s = h & 0x8000 ? -1 : 1;
  const e = (h >> 10) & 0x1f;
  const f = h & 0x3ff;
  if (e === 0) return s * f * 2 ** -24;
  if (e === 31) return f ? NaN : s * Infinity;
  return s * (1 + f / 1024) * 2 ** (e - 15);
}
