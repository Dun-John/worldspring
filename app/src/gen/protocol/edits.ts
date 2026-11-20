import type { DrawnPath, Fence, FenceGap, HexGrid, Plateau, TownEdit, Water, Wharf } from './features';
import type { BuildingEdit } from './town';
import type { Created } from './world';
import type { Clear, Crossing, Placed, SiteDesign, SpriteMeta } from './build';

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
/** A castle's keep's shapes and moats (`worldgen::world::KEEP_SHAPES`, `MOATS`). */
export type KeepShape = 'square' | 'round' | 'drawn';
export type Moat = 'none' | 'dry' | 'wet';

export type RoofStyle = 'hip' | 'battlements' | 'cone';
/** Roof colours (`worldgen::world::TINTS`, as `battlePrep.ROOF_TINTS`). */
export type Tint = 'terracotta' | 'slate' | 'thatch' | 'shingle' | 'moss';
/** Roof materials by id (`worldgen::town::roof::MATERIALS`, append-only) and their base colours (the
 * old tints' materials keep the tints' colours: `roof::TINT_MATERIAL`). */
export type RoofMaterial = 'thatch' | 'slate' | 'clay_tile' | 'shingle' | 'turf' | 'lead' | 'copper' | 'glazed_tile' | 'flat_clay' | 'hide' | 'plank' | 'whitewash' | 'stone';
/** Roof shapes by id (`worldgen::town::roof::SHAPES`, append-only). */
export type RoofShape = 'hip' | 'gable' | 'cone' | 'dome' | 'flat' | 'round';
/** What a building drawn by hand can be. */
export interface BuildingFuncs {
  businesses: { key: string; name: string; category: string }[];
  homes: { key: string; name: string }[];
}

export type SiteSize = 'small' | 'medium' | 'large' | 'huge';

export type Stance = 'hostile' | 'unfriendly' | 'neutral' | 'friendly' | 'allied';

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
  /** `<layout>` → one of the world's own towns or cities planned otherwise than rolled: 0-100, how
   * planned its streets are (0-29 grown, 30-69 a planned quarter beside a grown core, 70-100 planned
   * throughout; `worldgen::town::planning`). Villages and towns laid out by hand ignore it. */
  planning?: Record<string, number>;
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
