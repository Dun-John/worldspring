import type { Height, LotSize, ParkKind, SquareKind, TownWard } from '../protocol';
import type { TownEdit, Wharf } from './features';
import type { Rect } from './messages';

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
