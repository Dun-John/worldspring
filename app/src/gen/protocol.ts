// Messages between the main thread, the coordinator worker and generator workers,
// plus the terrain payload layout (mirrors crates/worldgen/src/payload.rs).
import { ruinPlanFill } from './ruinPlan';
import type { PreparedChunk } from './battlePrep';
import { codeMaterial, codeShape, codeStone } from './wallcode';
import type { EditOp } from '../sync/ops';

export type Wind = 'belts' | 'from_west' | 'from_east';

/** Mirrors `worldgen::WorldParams` (crates/worldgen/src/world.rs). */
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
  /** A building drawn by hand (`worldgen::world::Created`): its footprint (world ft), storeys,
   * what it is (a business key or a home), roof, roof colour and structure. */
  poly?: [number, number][];
  floors?: number;
  func?: string;
  roof?: RoofStyle;
  tint?: Tint;
  /** Roof material and shape (`ROOF_MATERIALS`, `ROOF_SHAPES`); else the tint's / the roof's, else picked. */
  material?: RoofMaterial;
  shape?: RoofShape;
  /** Roof colour variant 0-4 of the material (`ROOF_SHADES`; 2 the usual), else picked. */
  shade?: number;
  structure?: 'roofed' | 'ruin';
  /** A castle drawn by hand (its outline in `poly`, `structure` too): the side its gate is in
   * (corner `gate` to the next; else facing the nearest road), a keep, buildings round the yard
   * (both by default). */
  gate?: number;
  keep?: boolean;
  yard_buildings?: boolean;
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
export const CASTLE_MAX_FT = 600;
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
export type RoofMaterial = 'thatch' | 'slate' | 'clay_tile' | 'shingle' | 'turf' | 'lead' | 'copper' | 'glazed_tile' | 'flat_clay' | 'hide' | 'plank' | 'whitewash';
export const ROOF_MATERIALS: RoofMaterial[] = ['thatch', 'slate', 'clay_tile', 'shingle', 'turf', 'lead', 'copper', 'glazed_tile', 'flat_clay', 'hide', 'plank', 'whitewash'];
/** The materials the old tints map to (`roof::TINT_MATERIAL`). */
export const TINT_MATERIAL: Record<Tint, RoofMaterial> = { terracotta: 'clay_tile', slate: 'slate', thatch: 'thatch', shingle: 'shingle', moss: 'turf' };
export const ROOF_MATERIAL_RGB = [0xa88b5c, 0x5c6470, 0x995742, 0x7a6656, 0x7f7b54, 0x7c848c, 0x6a9282, 0x4a718b, 0xb89a72, 0x9c8a6c, 0x86684a, 0xc4baa7];
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

export interface Edits {
  renames?: Record<string, string>;
  notes?: Record<string, { text: string; tags?: string[] }>;
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
  /** `g:XXXXX:YYYYY` → 16 × 16 squares of ground brushed by hand (`worldgen::ground::GroundBlock`:
   * tenths of a foot, row-major, as base64url zigzag-delta varints, a 0 delta followed by a byte
   * counting further zeros). Block indices zero-padded, x first. */
  ground?: Record<string, string>;
  /** `h:<id>` → a plateau or pit drawn by hand (or a rock islet: `kind`). */
  plateaus?: Record<string, Plateau>;
  /** `q:<id>` → a quay, pier or breakwater built by hand. */
  wharves?: Record<string, Wharf>;
}

/** A flat top or a pit (`worldgen::ground::Plateau`): ground inside `pts` (world ft) eases to
 * `level` (ft); outside, back to the ground over `edge` ft (absent or 0: a sheer step). An
 * `islet` is rock standing out of the water (its outline made from a centre and radius). */
export interface Plateau {
  pts: [number, number][];
  level: number;
  edge?: number;
  kind?: 'plateau' | 'islet';
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
}

/** What a patch of a town can be given (`worldgen::world::TOWN_WARDS`; `empty`: open ground). */
export const TOWN_WARDS = ['plaza', 'castle', 'temple', 'merchant', 'craft', 'noble', 'common', 'slum', 'docks', 'military', 'farm', 'park', 'empty'] as const;
export type TownWard = (typeof TOWN_WARDS)[number];
/** Lot sizes, smallest first (`worldgen::world::LOT_SIZES`). */
export const LOT_SIZES = ['small', 'medium', 'large', 'huge'] as const;
export type LotSize = (typeof LOT_SIZES)[number];
/** Squares a corner where streets meet can be given (`worldgen::world::SQUARE_KINDS`). */
export const SQUARE_KINDS = ['crossroads', 'well', 'fountain', 'green'] as const;
export type SquareKind = (typeof SQUARE_KINDS)[number];
/** How a park can be laid out (`worldgen::world::PARK_KINDS`). */
export const PARK_KINDS = ['formal', 'common', 'cemetery', 'horses'] as const;
/** A horse park's marks on the plan (`town::horses::plan_polys`): [rgb, alpha] by the variant. */
const HORSE_PARK_PLAN: [[number, number, number], number][] = [
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

/** A town's plan for the ward editor (Ask op `townplan`). */
export interface TownPlan {
  layout: number;
  center: [number, number];
  /** How far a corner may go from where it was planned. */
  max_move_ft: number;
  walls: { built: boolean; generated: boolean; by_hand?: TownWallsByHand };
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
export const CROSSING_WIDTH: [number, number] = [5, 80];
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
   * building's door to the outside: 2 front, 4 back, 8 arch; underground, bits 4-6: `(lock & 7) << 4`, a
   * lock's colour 1-7; 128 a portcullis between two rooms). */
  doors: [number, number, number, number][];
  /** Props and the ways (`exit`, `up`, `down`), in order; a building's furniture. `back`: the
   * side it stands against (0 north 1 east 2 south 3 west), `variant`: one of its looks (0-3, more for a few);
   * absent: found from the walls, picked by position. */
  items: { kind: string; x: number; y: number; w: number; h: number; back?: number; variant?: number }[];
  /** A building's: storey (-1 cellar, 0 ground floor), an open roof, the stairs reach it. */
  z?: number;
  roof?: boolean;
  has_stairs?: boolean;
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
  /** Where a site can be created near (x, y), and a name for it (`agent::creation_spot`). */
  | { op: 'spot'; kind: string; under?: string; id: string; x: number; y: number }
  /** Whether a building drawn by hand can stand there, its point and a name
   * (`agent::building_spot`). */
  | { op: 'building'; poly: [number, number][]; func?: string; id: string }
  /** What a building can be (`agent::building_funcs_json`). */
  | { op: 'funcs' }
  /** A generated building's edit with a change made, or its removal (`agent::building_edit_json`). */
  | { op: 'bedit'; id: string; change: string }
  /** The generated buildings with their middle inside a polygon (`agent::generated_buildings_in`). */
  | { op: 'bin'; poly: [number, number][] }
  | { op: 'works'; site: Created }
  /** Shop lots along a bridge, or its shops laid out again after a change (`wharf::shop_lots`,
   * `Edits::follow_crossing`; `input` JSON). */
  | { op: 'bridgelots'; input: string }
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
  /** The villages and towns added by hand (`MadeSettlement`s). */
  | { op: 'made' }
  /** Whether a village or town can be added there (`t0::made::spot`), with what it would be. */
  | { op: 'madespot'; kind: string; id: string; x: number; y: number }
  /** Edits with those of the villages and towns added by hand that they remove, move or lay out
   * anew set aside (`t0::made::set_aside_changed`; `old`: the created sites before, JSON). */
  | { op: 'setaside'; old: string; edits: string }
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
}

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
  /** Hewn or natural rock (caves, mines): rooms open into each other; walls only at rock. */
  natural: boolean;
  /** Lines the floor follows (sewers: the streets above): x0, y0, x1, y1, reach (grid). */
  paths?: [number, number, number, number, number][];
  /** Floor beyond the grid's edge, as squares (sewer sections: `under::SEWER_RIM`). */
  rim?: [number, number][];
  /** Ways to other sites (cellar trapdoors, stairs to deep dungeons, sewer ladders): the
   * square and the site's id; the other site has a way straight back. */
  links?: { x: number; y: number; to: string }[];
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

/** Named feature from T0 (crates/worldgen/src/t0/features.rs). */
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

/** River polylines owned by one tile: vertex k is verts[4k..4k+4] = (x, y in tile units, width ft, discharge). */
export interface TileRivers {
  starts: Uint32Array;
  verts: Float32Array;
}

/** GPU-ready river capsules for one tile (4 vertices per segment), built off the main thread. */
export interface RiverGeometry {
  a: Float32Array;
  b: Float32Array;
  corner: Float32Array;
  w: Float32Array;
  q: Float32Array;
  index: Uint32Array;
}

const CORNERS = [0, -1, 1, -1, 1, 1, 0, 1];

export function buildRiverGeometry({ starts, verts }: TileRivers): RiverGeometry | null {
  let segs = 0;
  for (let l = 0; l + 1 < starts.length; l++) segs += Math.max(0, starts[l + 1] - starts[l] - 1);
  if (!segs) return null;
  const g: RiverGeometry = {
    a: new Float32Array(segs * 8),
    b: new Float32Array(segs * 8),
    corner: new Float32Array(segs * 8),
    w: new Float32Array(segs * 8),
    q: new Float32Array(segs * 8),
    index: new Uint32Array(segs * 6),
  };
  let s = 0;
  for (let l = 0; l + 1 < starts.length; l++) {
    for (let v = starts[l]; v + 1 < starts[l + 1]; v++) {
      const i = v * 4;
      for (let c = 0; c < 4; c++) {
        const o = (s * 4 + c) * 2;
        g.a[o] = verts[i];
        g.a[o + 1] = verts[i + 1];
        g.b[o] = verts[i + 4];
        g.b[o + 1] = verts[i + 5];
        g.corner[o] = CORNERS[c * 2];
        g.corner[o + 1] = CORNERS[c * 2 + 1];
        g.w[o] = verts[i + 2];
        g.w[o + 1] = verts[i + 6];
        g.q[o] = verts[i + 3];
        g.q[o + 1] = verts[i + 7];
      }
      const x = s * 6;
      const b = s * 4;
      g.index[x] = b;
      g.index[x + 1] = b + 1;
      g.index[x + 2] = b + 2;
      g.index[x + 3] = b;
      g.index[x + 4] = b + 2;
      g.index[x + 5] = b + 3;
      s++;
    }
  }
  return g;
}

/** Settlement polygons owned by one tile (see payload.rs). */
export interface TileSitePolys {
  starts: Uint32Array;
  attrs: Uint32Array;
  verts: Float32Array;
}

/** GPU-ready settlement geometry: triangulated fills (x, y, r, g, b, a) and ink capsules. */
export interface SiteGeometry {
  /** Polygons, one vertex per triangle corner; `edge` per vertex: the corner's barycentric
   * coordinate for each triangle edge that is an outlined polygon's real edge, 1 for an inner
   * diagonal or an unoutlined polygon (the fill shader inks distance-to-edge < ~1 px). */
  fill: { pos: Float32Array; color: Float32Array; edge: Float32Array; index: Uint32Array } | null;
  ink: RiverGeometry | null;
}

/** A wall's stone as the plan fills it (towers, bastions): granite, limestone, sandstone, rose brick, mud brick,
 * then timber and rammed earth. The same tones as `tileink.frag`'s wall core. */
const STONE_TONE: [number, number, number][] = [
  [0.66, 0.71, 0.78],
  [0.86, 0.8, 0.65],
  [0.9, 0.72, 0.42],
  [0.82, 0.52, 0.47],
  [0.86, 0.64, 0.52],
  [0.55, 0.39, 0.24],
  [0.8, 0.72, 0.56],
];

/** A tone a third of the way to its own grey (a bastion's broad fill, beside its wall's narrow core). */
const calm = ([r, g, b]: [number, number, number]): [number, number, number] => {
  const y = 0.3 * r + 0.59 * g + 0.11 * b;
  return [r + (y - r) * 0.35, g + (y - g) * 0.35, b + (y - b) * 0.35];
};

// Fill colours by ward (buildings), then plaza, field, block. Watabou-ish muted browns.
const WARD_RGB: [number, number, number][] = [
  [0.62, 0.56, 0.5], // plaza (unused for buildings)
  [0.5, 0.45, 0.42], // castle
  [0.6, 0.52, 0.48], // temple
  [0.64, 0.57, 0.49], // merchant
  [0.6, 0.55, 0.5], // craft
  [0.68, 0.6, 0.5], // noble
  [0.63, 0.58, 0.52], // common
  [0.56, 0.52, 0.48], // slum
  [0.58, 0.54, 0.5], // docks
  [0.52, 0.49, 0.46], // military
  [0.64, 0.58, 0.5], // farm
  [0.64, 0.57, 0.48], // rural
  [0.6, 0.62, 0.5], // park
];

/** How much larger than built a wall tower's shape is drawn on the plan, so that a D, an octagon or a square
 * reads beside the curtain from far out (a gate's towers, already wide, as built). */
const PLAN_TOWER = 1.4;
/** A D tower's flat back on the plan: this far (ft) behind its middle, past the curtain's inner face (a stone
 * curtain's half is 4.5 ft), so its square corners show inside the wall and its round face outside. */
const PLAN_D_BACK = 8;

/** A wall tower's footprint on the plan (tile units): a D (flat back, round face out along `ang`), an octagon or a
 * square (sides along `ang`) of radius `r` (ft), as `wallstyle::footprint` lays it. */
function towerPoly(x: number, y: number, r: number, ang: number, shape: number, ftTile: number): number[] {
  const [ux, uy] = [Math.cos(ang), Math.sin(ang)];
  const at = (a: number, b: number) => [x + (ux * a - uy * b) / ftTile, y + (uy * a + ux * b) / ftTile];
  const out: number[] = [];
  if (shape === 2) {
    out.push(...at(-PLAN_D_BACK, -r));
    for (let k = 0; k <= 12; k++) {
      const t = -Math.PI / 2 + (Math.PI * k) / 12;
      out.push(...at(r * Math.cos(t), r * Math.sin(t)));
    }
    out.push(...at(-PLAN_D_BACK, r));
  } else if (shape === 3) {
    const rc = r / Math.cos(Math.PI / 8);
    for (let k = 0; k < 8; k++) {
      const t = Math.PI / 8 + (Math.PI * 2 * k) / 8;
      out.push(...at(rc * Math.cos(t), rc * Math.sin(t)));
    }
  } else out.push(...at(-r, -r), ...at(r, -r), ...at(r, r), ...at(-r, r));
  return out;
}

/** A village's headstone tick on the plan (`town::village::PLAN_HEADSTONE`). */
const PLAN_HEADSTONE = 20;

/** Plan kind of a town graveyard's grave mark (not from the generator). */
const GRAVE_KIND = 0xfd;

/** Ear-clip triangulation of a simple polygon's `m` corners from `first` in `verts` (local corner indices;
 * m - 2 triangles). A degenerate ring falls back to the fan. */
function earClip(verts: Float32Array, first: number, m: number): number[][] {
  const P = (i: number) => [verts[(first + i) * 2], verts[(first + i) * 2 + 1]];
  let area = 0;
  for (let i = 0; i < m; i++) {
    const [a, b] = [P(i), P((i + 1) % m)];
    area += a[0] * b[1] - b[0] * a[1];
  }
  const sgn = area >= 0 ? 1 : -1;
  const cross = (a: number[], b: number[], c: number[]) => ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * sgn;
  const idx = Array.from({ length: m }, (_, i) => i);
  const out: number[][] = [];
  while (idx.length > 3) {
    let cut = -1;
    for (let k = 0; k < idx.length && cut < 0; k++) {
      const [ia, ib, ic] = [idx[(k + idx.length - 1) % idx.length], idx[k], idx[(k + 1) % idx.length]];
      const [a, b, c] = [P(ia), P(ib), P(ic)];
      if (cross(a, b, c) <= 0) continue;
      let ok = true;
      for (const j of idx) {
        if (j === ia || j === ib || j === ic) continue;
        const p = P(j);
        if (cross(a, b, p) >= 0 && cross(b, c, p) >= 0 && cross(c, a, p) >= 0) {
          ok = false;
          break;
        }
      }
      if (ok) cut = k;
    }
    if (cut < 0) cut = 0;
    out.push([idx[(cut + idx.length - 1) % idx.length], idx[cut], idx[(cut + 1) % idx.length]]);
    idx.splice(cut, 1);
  }
  out.push([idx[0], idx[1], idx[2]]);
  return out;
}

/** Roadmap 5 cemeteries: rows of grave marks across each graveyard (plan kind 5) that has none of its own (a
 * village's churchyards and shrine yards get theirs from `town::village::plan_ticks`): pale ticks 3.4 by 1.5 ft
 * across the rows, side by side as graves lie (a little over life size, to read at near zoom; lost in the plot's
 * green farther out), 5 ft apart in rows 8 ft apart, the rows square to the plot's longest side, 3.5 ft in from
 * its edges, off any roofed building in it (a mausoleum, a tomb house) and from under a tree's crown (a yew's,
 * plan kind 14), a walk left across the rows of a plot over 40 ft long. A pure function of the plot's corners
 * (tile units, `tileFt` ft a side). */
function withGraveRows(polys: TileSitePolys, tileFt: number): TileSitePolys {
  const n = polys.attrs.length;
  const kindOf = (i: number) => polys.attrs[i] & 0xff;
  const ring = (i: number): [number, number][] => {
    const out: [number, number][] = [];
    for (let k = polys.starts[i]; k < polys.starts[i + 1]; k++) out.push([polys.verts[k * 2] * tileFt, polys.verts[k * 2 + 1] * tileFt]);
    return out;
  };
  const inside = (p: [number, number], q: [number, number][]) => {
    let c = false;
    for (let a = 0, b = q.length - 1; a < q.length; b = a++) {
      if (q[a][1] > p[1] !== q[b][1] > p[1] && p[0] < ((q[b][0] - q[a][0]) * (p[1] - q[a][1])) / (q[b][1] - q[a][1]) + q[a][0]) c = !c;
    }
    return c;
  };
  const edgeDist = (p: [number, number], q: [number, number][]) => {
    let best = Infinity;
    for (let a = 0, b = q.length - 1; a < q.length; b = a++) {
      const [dx, dy] = [q[a][0] - q[b][0], q[a][1] - q[b][1]];
      const t = Math.max(0, Math.min(1, ((p[0] - q[b][0]) * dx + (p[1] - q[b][1]) * dy) / (dx * dx + dy * dy || 1)));
      best = Math.min(best, Math.hypot(p[0] - q[b][0] - dx * t, p[1] - q[b][1] - dy * t));
    }
    return best;
  };
  const centroid = (q: [number, number][]): [number, number] => [q.reduce((s, p) => s + p[0], 0) / q.length, q.reduce((s, p) => s + p[1], 0) / q.length];
  let ticks: [number, number][] | null = null;
  let roofed: [number, number][][] | null = null;
  const marks: number[][] = [];
  for (let i = 0; i < n; i++) {
    if (kindOf(i) !== 5 || polys.starts[i + 1] - polys.starts[i] < 3) continue;
    ticks ??= Array.from({ length: n }, (_, j) => j)
      .filter((j) => kindOf(j) === PLAN_HEADSTONE)
      .map((j) => centroid(ring(j)));
    roofed ??= Array.from({ length: n }, (_, j) => j)
      .filter((j) => kindOf(j) === 0 || kindOf(j) === 14)
      .map(ring);
    const q = ring(i);
    if (ticks.some((t) => inside(t, q))) continue;
    // The longest side: the rows run along it.
    let [ux, uy, best] = [1, 0, 0];
    for (let a = 0, b = q.length - 1; a < q.length; b = a++) {
      const d = Math.hypot(q[a][0] - q[b][0], q[a][1] - q[b][1]);
      if (d > best) [ux, uy, best] = [(q[a][0] - q[b][0]) / d, (q[a][1] - q[b][1]) / d, d];
    }
    const [o0, o1] = q[0];
    const along = q.map(([x, y]) => (x - o0) * ux + (y - o1) * uy);
    const across = q.map(([x, y]) => -(x - o0) * uy + (y - o1) * ux);
    const [u0, u1, w0, w1] = [Math.min(...along), Math.max(...along), Math.min(...across), Math.max(...across)];
    if (u1 - u0 < 8 || w1 - w0 < 8 || (u1 - u0) * (w1 - w0) > 4e6) continue;
    const inPlot = roofed.filter((b) => inside(centroid(b), q));
    const walk = u1 - u0 > 40 ? (u0 + u1) / 2 : NaN;
    // (Rows and marks centred in the plot, so the margins match on both sides.)
    const [rows, cols] = [Math.floor((w1 - w0 - 7) / 8), Math.floor((u1 - u0 - 7 - 1.5) / 5)];
    const [s0, x0] = [(w0 + w1) / 2 - (rows * 8) / 2, (u0 + u1) / 2 - (cols * 5) / 2];
    for (let r = 0; r <= rows; r++) {
      for (let c = 0; c <= cols; c++) {
        const [u, w] = [x0 + c * 5, s0 + r * 8];
        if (Math.abs(u - walk) < 4) continue;
        const p: [number, number] = [o0 + u * ux - w * uy, o1 + u * uy + w * ux];
        if (!inside(p, q) || edgeDist(p, q) < 3.5) continue;
        if (inPlot.some((b) => inside(p, b) || edgeDist(p, b) < 3)) continue;
        const [hx, hy, nx, ny] = [ux * 1.0, uy * 1.0, -uy * 2.0, ux * 2.0];
        marks.push([p[0] - hx - nx, p[1] - hy - ny, p[0] + hx - nx, p[1] + hy - ny, p[0] + hx + nx, p[1] + hy + ny, p[0] - hx + nx, p[1] - hy + ny].map((v) => v / tileFt));
      }
    }
  }
  if (!marks.length) return polys;
  const starts = Array.from(polys.starts);
  const attrs = Array.from(polys.attrs);
  const verts = Array.from(polys.verts);
  for (const m of marks) {
    attrs.push(GRAVE_KIND);
    verts.push(...m);
    starts.push(starts[starts.length - 1] + 4);
  }
  return { starts: new Uint32Array(starts), attrs: new Uint32Array(attrs), verts: new Float32Array(verts) };
}

/** Plan kind of a D, octagonal or square wall tower's footprint (not from the generator; the stone in its ward byte). */
const TOWER_KIND = 0xfe;

/**
 * A wall run drawn as one band: the capsule of each piece of a polyline has a round end with its own outline, which
 * across a joint reads as a bead. Each wall piece's `q.y` (unused by `tileink` otherwise) carries a joint code per end
 * (0 none, 1 the run turns left there, 2 right, 3 shallow; end A + 4 * end B): the shader draws a joined end's cap without its
 * rim except on the outside of the bend. A ring (first point = last) joins at its seam too.
 */
function joinWallInk(g: RiverGeometry, starts: number[], v: Float32Array, ft: number, towers: number[][]): void {
  const CUT = 24; // (ft from the tile's edge: a run's end there is probably where the tile cuts it, its neighbour carries on)
  const isWall = (i: number) => {
    const st = v[i * 4 + 3];
    return (st >= 1 && st < 2) || st === 5 || st === 6;
  };
  // (3: a shallow bend, the cap bare all round; 1 or 2: a sharp left or right turn, rim kept on its outside.)
  const turn = (a: number, b: number, c: number) => {
    const [ux, uy, wx, wy] = [v[b * 4] - v[a * 4], v[b * 4 + 1] - v[a * 4 + 1], v[c * 4] - v[b * 4], v[c * 4 + 1] - v[b * 4 + 1]];
    const cr = ux * wy - uy * wx;
    const dot = ux * wx + uy * wy;
    if (dot > 0.94 * Math.hypot(ux, uy) * Math.hypot(wx, wy)) return 3;
    return cr > 0 ? 1 : 2;
  };
  // (Vertices are tile fractions; `ft` is the tile's side. Pieces of a run that are lines of their own join where end points
  // meet, within 1.5 ft: end vertex -> the other end's vertex beside it. An end that meets a wall's side (a T), lies under a
  // tower or gate, or is where the tile cuts the run (outside, or near its edge) has no visible cap: code 3, bare.)
  const X = (i: number) => v[i * 4] * ft;
  const Y = (i: number) => v[i * 4 + 1] * ft;
  const ends: { i: number; beside: number }[] = [];
  const sides: number[][] = [];
  for (let l = 0; l + 1 < starts.length; l++) {
    const [v0, v1] = [starts[l], starts[l + 1]];
    if (v1 - v0 < 2 || !isWall(v0)) continue;
    ends.push({ i: v0, beside: v0 + 1 }, { i: v1 - 1, beside: v1 - 2 });
    for (let i = v0; i + 1 < v1; i++) sides.push([i, i + 1]);
  }
  const TOL = 1.5;
  const mate = (e: number, beside: number) => {
    const m = ends.filter((o) => o.i !== e && Math.hypot(X(o.i) - X(e), Y(o.i) - Y(e)) < TOL);
    return m.length === 1 && m[0].beside !== beside ? m[0].beside : -1;
  };
  const bare = (e: number) => {
    const [x, y] = [X(e), Y(e)];
    if (x < CUT || y < CUT || x > ft - CUT || y > ft - CUT) return true;
    if (towers.some(([tx, ty, r]) => Math.hypot(tx - x, ty - y) < r + 1)) return true;
    if (ends.some((o) => o.i !== e && Math.hypot(X(o.i) - x, Y(o.i) - y) < TOL)) return true;
    return sides.some(([p, q]) => {
      if (p === e || q === e) return false;
      const [dx, dy] = [X(q) - X(p), Y(q) - Y(p)];
      const t = Math.max(0, Math.min(1, ((x - X(p)) * dx + (y - Y(p)) * dy) / Math.max(dx * dx + dy * dy, 1e-6)));
      return Math.hypot(X(p) + dx * t - x, Y(p) + dy * t - y) < TOL;
    });
  };
  let s = 0;
  for (let l = 0; l + 1 < starts.length; l++) {
    const [v0, v1] = [starts[l], starts[l + 1]];
    const wall = v1 - v0 > 1 && isWall(v0);
    const ring = wall && v1 - v0 > 3 && v[v0 * 4] === v[(v1 - 1) * 4] && v[v0 * 4 + 1] === v[(v1 - 1) * 4 + 1];
    for (let i = v0; i + 1 < v1; i++) {
      let code = 0;
      if (wall) {
        const [ma, mb] = [i > v0 || ring ? -1 : mate(i, i + 1), i + 2 < v1 || ring ? -1 : mate(i + 1, i)];
        const a = i > v0 ? turn(i - 1, i, i + 1) : ring ? turn(v1 - 2, i, i + 1) : ma >= 0 ? turn(ma, i, i + 1) : bare(i) ? 3 : 0;
        const b = i + 2 < v1 ? turn(i, i + 1, i + 2) : ring ? turn(i, i + 1, v0 + 1) : mb >= 0 ? turn(i, i + 1, mb) : bare(i + 1) ? 3 : 0;
        code = a + 4 * b;
      }
      for (let c = 0; c < 4; c++) g.q[(s * 4 + c) * 2 + 1] = code;
      s++;
    }
  }
}

/**
 * Settlement fills and ink for one tile. `looks` (`TerrainPayload.siteLooks`) and `tileFt` (the tile's side in ft)
 * give the plan the walls' look by culture group: a stone wall's ink tinted by its stone, a palisade or rammed
 * earth wall by its material (else guessed from its width), D, octagonal and square towers as their shapes
 * (round ones stay dots). Without them (an older payload) the old drawing.
 */
export function buildSiteGeometry(polys: TileSitePolys, lines: TileRivers, looks?: TileRivers, tileFt = 0): SiteGeometry | null {
  const useLooks = !!looks && tileFt > 0 && looks.verts.length === lines.verts.length;
  // (Each tower line in a shape of its own is a polygon instead of a dot: its centre, radius, facing, shape, stone.)
  const shapes: { starts: number; verts: number[]; stone: number }[] = [];
  const skip = new Set<number>();
  if (useLooks) {
    for (let l = 0; l + 1 < lines.starts.length; l++) {
      const v = lines.starts[l];
      if (lines.verts[v * 4 + 3] !== 1) continue;
      const code = looks!.verts[v * 4 + 2];
      const shape = codeShape(code);
      // (A palisade's or an earth wall's square gate towers too: log towers, in their material's tone.)
      const mat = codeMaterial(code);
      if (!shape || (mat !== 0 && shape !== 1)) continue;
      const [x, y, w] = [lines.verts[v * 4], lines.verts[v * 4 + 1], lines.verts[v * 4 + 2]];
      const r = w < 25 ? (w / 2) * PLAN_TOWER : w / 2;
      shapes.push({ starts: 0, verts: towerPoly(x, y, r, looks!.verts[v * 4 + 3], shape, tileFt), stone: mat === 1 ? 5 : mat === 2 ? 6 : codeStone(code) });
      skip.add(l);
    }
    // (A bastion takes the look of the wall line nearest it: its stone, in its ward byte + 1.)
    const wallLooks: [number, number, number][] = [];
    for (let l = 0; l + 1 < lines.starts.length; l++) {
      const v = lines.starts[l];
      if (lines.verts[v * 4 + 3] !== 0) continue;
      const code = looks!.verts[v * 4 + 2];
      if (code > 0) for (let k = v; k < lines.starts[l + 1]; k++) wallLooks.push([lines.verts[k * 4], lines.verts[k * 4 + 1], code]);
    }
    let bastions: Uint32Array | null = null;
    if (wallLooks.length) {
      bastions = new Uint32Array(polys.attrs);
      for (let i = 0; i < bastions.length; i++) {
        if ((bastions[i] & 0xff) !== 32) continue;
        let [cx, cy] = [0, 0];
        const m = polys.starts[i + 1] - polys.starts[i];
        for (let k = polys.starts[i]; k < polys.starts[i + 1]; k++) [cx, cy] = [cx + polys.verts[k * 2] / m, cy + polys.verts[k * 2 + 1] / m];
        let best = Infinity;
        let code = 0;
        for (const w of wallLooks) {
          const d = (w[0] - cx) ** 2 + (w[1] - cy) ** 2;
          if (d < best) [best, code] = [d, w[2]];
        }
        const mat = codeMaterial(code);
        bastions[i] |= ((mat === 1 ? 5 : mat === 2 ? 6 : codeStone(code)) + 1) << 8;
      }
    }
    if (bastions) polys = { starts: polys.starts, attrs: bastions, verts: polys.verts };
    if (shapes.length) {
      const starts = Array.from(polys.starts);
      const attrs = Array.from(polys.attrs);
      const verts = Array.from(polys.verts);
      for (const sh of shapes) {
        attrs.push(TOWER_KIND | (sh.stone << 8));
        verts.push(...sh.verts);
        starts.push(starts[starts.length - 1] + sh.verts.length / 2);
      }
      polys = { starts: new Uint32Array(starts), attrs: new Uint32Array(attrs), verts: new Float32Array(verts) };
    }
  }
  if (tileFt > 0) polys = withGraveRows(polys, tileFt);
  const n = polys.attrs.length;
  let tris = 0;
  for (let i = 0; i < n; i++) tris += Math.max(0, polys.starts[i + 1] - polys.starts[i] - 2);
  const outlined = (kind: number) => kind === 0 || kind === 3 || kind === 4 || kind === 5 || kind === 6 || kind === 7 || kind === 9 || kind === 13 || kind === 14 || kind === 15 || kind === 23 || kind === 24 || kind === 25 || kind === 26 || kind === 29 || kind === 32 || kind === 33 || kind === 44 || kind === 45 || kind === TOWER_KIND;
  let fill: SiteGeometry['fill'] = null;
  if (n) {
    // Unshared corners (3 per triangle), so each carries its triangle's edge coordinates.
    const pos = new Float32Array(tris * 6);
    const color = new Float32Array(tris * 12);
    const edge = new Float32Array(tris * 9);
    const index = new Uint32Array(tris * 3);
    let v = 0;
    for (let i = 0; i < n; i++) {
      const s0 = polys.starts[i];
      const m = polys.starts[i + 1] - s0;
      const attr = polys.attrs[i];
      const kind = attr & 0xff;
      const ward = (attr >> 8) & 0xff;
      const hasFunc = (attr >> 24) & 1;
      // (Its roof's material: `payload::pack_sites`, `town::roof`.)
      const material = (attr >> 20) & 15;
      let rgb: [number, number, number];
      let a = 1;
      if (kind === TOWER_KIND) rgb = STONE_TONE[Math.min(ward, STONE_TONE.length - 1)];
      else if (kind === 1) rgb = [0.86, 0.82, 0.71];
      else if (kind === 2) {
        // Fields: alternate crop tints.
        const k = i % 3;
        rgb = k === 0 ? [0.78, 0.77, 0.55] : k === 1 ? [0.72, 0.74, 0.5] : [0.8, 0.74, 0.52];
        a = 0.55;
      } else if (kind === 3) rgb = [0.62, 0.56, 0.5];
      else if (kind === 4) {
        // Ruin: roofless shell, pale rubble.
        rgb = [0.7, 0.67, 0.6];
        a = 0.55;
      } else if (kind === 5) {
        // A graveyard or cemetery (an open-air plot): inked round, a muted green over the land's own
        // ground, its rows of grave marks over it (`withGraveRows`, or a village's own ticks).
        rgb = [0.5, 0.6, 0.42];
        a = 0.55;
      } else if (kind === 6) rgb = [0.55, 0.4, 0.26];
      else if (kind === 7) rgb = [0.72, 0.69, 0.62];
      else if (kind === 8) {
        // A camp's clearing: bare earth.
        rgb = [0.72, 0.62, 0.47];
        a = 0.7;
      } else if (kind === 9) rgb = [0.86, 0.8, 0.66];
      // A park's water (a pond fanned from its middle, a stream; not outlined), gravel, parterres
      // (hedged beds), trees' crowns and flowerbeds.
      else if (kind === 11) rgb = [0.42, 0.6, 0.64];
      // Roadmap 5 horse parks (`town::horses::PLAN_FENCE`, kind 34, the mark in the ward byte) over the
      // park's trodden ground (a camp clearing's, kind 8); not inked: a corral's post-and-rail fence (0),
      // a bold line of dark timber; a corral's ground (1) a shade greener; a horse (2) a dark bay bar; a
      // haystack (3) a straw round; a hitching rail or a cart (4) a timber tick.
      else if (kind === 34) {
        [rgb, a] = HORSE_PARK_PLAN[ward] ?? HORSE_PARK_PLAN[0];
      }
      // A kurgan mound (kind 33): a low grassy barrow, a shade darker than a cemetery plot, lightly inked.
      else if (kind === 33) {
        rgb = [0.44, 0.52, 0.3];
        a = 0.6;
      }
      // A town wall's bastion (`payload.rs` kind 32): a stone platform, outlined (its faces are the town
      // wall's lines), in its wall's stone calmed toward grey: a broad fill reads louder than the curtain's
      // narrow core in the same tone.
      else if (kind === 32) rgb = ward ? calm(STONE_TONE[Math.min(ward - 1, STONE_TONE.length - 1)]) : [0.76, 0.73, 0.65];
      else if (kind === 12) rgb = [0.85, 0.81, 0.69];
      else if (kind === 13) {
        rgb = [0.42, 0.55, 0.33];
        a = 0.7;
      } else if (kind === 14) rgb = [0.33, 0.46, 0.26];
      else if (kind === 15) rgb = [0.74, 0.5, 0.46];
      // A castle's moat (`town::moat`): a wet one water a shade deeper than the rivers' (its berm by
      // the curtain left uninked, so a pale strip shows between them), a dry ditch a band of warm
      // earth to its block's edge; inked along their banks (and their ends at a gap) by the edges
      // in the ward's place (`Moat::inks`). Its drawbridge (26) a timber deck over the gap.
      else if (kind === 24) rgb = [0.3, 0.45, 0.57];
      else if (kind === 25) rgb = [0.6, 0.48, 0.33];
      else if (kind === 26) rgb = [0.55, 0.4, 0.25];
      // R5 M20: an islet drawn by hand (44) bare rock; a quay's or breakwater's made ground (45)
      // paving stone.
      else if (kind === 44) rgb = [0.6, 0.57, 0.5];
      else if (kind === 45) rgb = [0.74, 0.71, 0.64];
      // v63 countryside: a village's headstones and a churchyard's low wall (`town::village::plan_polys`;
      // the green's and churchyard's grass is the land's own, its crofts seen on the battlemap only).
      else if (kind === 20) rgb = [0.74, 0.73, 0.69];
      // A town graveyard's grave marks (`withGraveRows`): pale stone on its green.
      else if (kind === GRAVE_KIND) rgb = [0.9, 0.9, 0.82];
      else if (kind === 22) rgb = [0.62, 0.6, 0.55];
      // A windmill's sails (`town::rural::plan_ticks`), over its roof.
      else if (kind === 23) rgb = [0.88, 0.84, 0.74];
      // v63 underground: a ruin's plan (`ruinPlan.ts`).
      else if (ruinPlanFill(kind)) [rgb, a] = ruinPlanFill(kind)!;
      else if ((attr >>> 25) & 15) {
        // A keep's piece (`payload::pack_sites`): towers and the body's hall under slate, a wing or
        // forebuilding by its top (`town::keep_tops`): open battlements stone, a roof lead or slate; a
        // monastery's church (5) its ranges' colour deepened, its bell-cote (6) slate; a town hall's
        // belfry (7) under its verdigris spire, its open bell lantern (8) pale stone, its long shadow
        // (9) down the hall's roof.
        const piece = ((attr >>> 25) & 15) - 1;
        const top = (attr >>> 29) & 3;
        // R5 towns (115): a yurt (10) its felt (`YURT_FELT`, by the shade bits), softened toward its ward's paper.
        if (piece === 10) {
          const base = WARD_RGB[ward] ?? WARD_RGB[6];
          const m = YURT_FELT[(attr >>> 29) & 7] ?? YURT_FELT[0];
          rgb = [0, 1, 2].map((c) => 0.7 * (((m >> (16 - 8 * c)) & 255) / 255) + 0.3 * base[c]) as [number, number, number];
        }
        // (R5 towns, 114: a felt hall's crown, 12, its wooden ring dark over the smoke hole.)
        else if (piece === 11) rgb = [0.38, 0.29, 0.21];
        // (R5 M20: a lighthouse's lamp (13), a warm light on its slate cone.)
        else if (piece === 12) rgb = [0.93, 0.78, 0.4];
        else if (piece === 7) rgb = [0.36, 0.6, 0.53];
        else if (piece === 8) rgb = [0.86, 0.8, 0.66];
        else if (piece === 9) {
          rgb = [0, 0, 0];
          a = 0.3;
        } else if (piece === 5) {
          const base = WARD_RGB[ward] ?? WARD_RGB[6];
          rgb = [base[0] * 0.7, base[1] * 0.58, base[2] * 0.52];
        } else rgb = piece === 3 || piece === 4 ? (top === 0 ? [0.62, 0.6, 0.55] : top === 1 ? [0.4, 0.42, 0.44] : [0.38, 0.42, 0.47]) : piece === 0 ? [0.42, 0.45, 0.49] : [0.36, 0.4, 0.45];
      } else if (kind === 0) {
        // A roofed building: its roof material's colour, softened toward its ward's paper brown
        // (watabou-ish), a business a little deeper.
        const base = WARD_RGB[ward] ?? WARD_RGB[6];
        const m = roofRgb(material, (attr >>> 29) & 7);
        const shade = (0.94 + 0.12 * (((i * 2654435761) >>> 0) / 4294967296)) * (hasFunc ? 0.88 : 1);
        rgb = [0, 1, 2].map((c) => (0.6 * (((m >> (16 - 8 * c)) & 255) / 255) + 0.4 * base[c]) * shade) as [number, number, number];
      } else {
        const base = WARD_RGB[ward] ?? WARD_RGB[6];
        const shade = 0.94 + 0.12 * (((i * 2654435761) >>> 0) / 4294967296);
        rgb = hasFunc ? [base[0] * 0.86, base[1] * 0.74, base[2] * 0.66] : [base[0] * shade, base[1] * shade, base[2] * shade];
      }
      // (A belfry's shadow not inked.)
      const ink = outlined(kind) && ((attr >>> 25) & 15) !== 10;
      const inks = kind === 24 || kind === 25 ? ward : -1;
      const inkEdge = (j: number) => ink && (j >= 8 || ((inks >> j) & 1) !== 0);
      // Fan triangles (0, k, k+1): the edge k..k+1 is always the polygon's; 0..1 only for the
      // first triangle, k+1..0 only for the last.
      // A plot (kind 5) is often concave: ear-clipped, so no wedge crosses its notches (a fan would).
      const ears = kind === 5 && m > 3 ? earClip(polys.verts, s0, m) : null;
      for (let k = 1; k + 1 < m; k++) {
        const t = ears ? ears[k - 1] : [0, k, k + 1];
        const corners = [s0 + t[0], s0 + t[1], s0 + t[2]];
        // Which edges are real, by the corner opposite each.
        const side = (x: number, y: number) => (Math.abs(x - y) === 1 ? inkEdge(Math.min(x, y)) : Math.abs(x - y) === m - 1 && inkEdge(m - 1));
        const real = ears
          ? [side(t[1], t[2]), side(t[2], t[0]), side(t[0], t[1])]
          : [inkEdge(k), k + 1 === m - 1 && inkEdge(m - 1), k === 1 && inkEdge(0)];
        for (let c = 0; c < 3; c++) {
          pos[v * 2] = polys.verts[corners[c] * 2];
          pos[v * 2 + 1] = polys.verts[corners[c] * 2 + 1];
          color.set([rgb[0], rgb[1], rgb[2], a], v * 4);
          for (let e = 0; e < 3; e++) edge[v * 3 + e] = real[e] ? (c === e ? 1 : 0) : 1;
          index[v] = v;
          v++;
        }
      }
    }
    fill = { pos, color, edge, index };
  }
  // Ink: walls and towers from the line section (building outlines are inked by the fill). A town wall
  // by its width (`payload.rs`, item 104): a palisade's (5 ft) timber brown, rammed earth's (12) tan, both
  // inked along their sides (tileink styles 5, 6); stone's ink.
  const lineVerts = lines.verts.length / 4;
  // With the looks, the material and stone come from the style code: a stone's style carries its stone + 1 in
  // sixteenths after its number (none for limestone, today's ink), a palisade is 5 and rammed earth 6.
  const styleOf = (v: number) => {
    const kind = lines.verts[v * 4 + 3];
    const w = lines.verts[v * 4 + 2];
    if (useLooks && (kind === 0 || kind === 1 || kind === 3)) {
      const code = looks!.verts[v * 4 + 2];
      if (kind === 0 && code > 0) {
        if (codeMaterial(code) === 1) return 5;
        if (codeMaterial(code) === 2) return 6;
      }
      const stone = codeMaterial(code) === 0 && code > 0 ? codeStone(code) : 1;
      return kind + 1 + (stone === 1 || stone > 4 ? 0 : (stone + 1) / 16);
    }
    return kind === 0 && w === 5 ? 5 : kind === 0 && w === 12 ? 6 : kind + 1;
  };
  const starts: number[] = [];
  const out = new Float32Array(lineVerts * 4);
  let o = 0;
  for (let l = 0; l + 1 < lines.starts.length; l++) {
    if (skip.has(l)) continue;
    starts.push(o / 4);
    for (let v = lines.starts[l]; v < lines.starts[l + 1]; v++) {
      out.set([lines.verts[v * 4], lines.verts[v * 4 + 1], lines.verts[v * 4 + 2], styleOf(v)], o);
      o += 4;
    }
  }
  starts.push(o / 4);
  const ink = buildRiverGeometry({ starts: new Uint32Array(starts), verts: out.subarray(0, o) });
  if (ink && tileFt > 0) {
    // (Towers and gates, as centre and radius in ft: wall ends under them need no cap.)
    const towers: number[][] = [];
    for (let l = 0; l + 1 < lines.starts.length; l++) {
      const v = lines.starts[l];
      const k = lines.verts[v * 4 + 3];
      if (k !== 1 && k !== 3) continue;
      const e = lines.starts[l + 1] - 1;
      const [x, y] = [(lines.verts[v * 4] + lines.verts[e * 4]) / 2, (lines.verts[v * 4 + 1] + lines.verts[e * 4 + 1]) / 2];
      towers.push([x * tileFt, y * tileFt, lines.verts[v * 4 + 2] / 2]);
    }
    joinWallInk(ink, starts, out.subarray(0, o), tileFt, towers);
  }
  if (!fill && !ink) return null;
  return { fill, ink };
}

export const TILE_N = 256;
export const TILE_SAMPLES = TILE_N + 1;
export const HALO = 4;
export const PADDED = TILE_SAMPLES + 2 * HALO;
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
   * taken away). `groundRects`: where the ground was shaped by hand: the three finest levels'
   * tiles there are packed again (their samples, as generated, still serve finer tiles). */
  | { type: 'edits'; fields: Record<string, string>; patches: Record<string, EditsPatch>; rects: Rect[]; epoch: number; battleRects?: Rect[]; groundRects?: Rect[] };

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
