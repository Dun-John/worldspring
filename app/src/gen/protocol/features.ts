import type { CrossingMaterial } from './build';
import type { Rect } from './messages';

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
  kind?: 'plateau' | 'islet' | 'hill';
  floor?: 'rock' | 'ground';
  /** A hill's form (`kind: 'hill'`: then `level` is its peak, ft over the ground). */
  hill?: Hill;
}

export type HillProfile = 'round' | 'ridge' | 'knoll' | 'mountain';

/** A hill raised by hand (`worldgen::ground::Hill`): its footprint's middle (world ft), half-axes
 * (`rx` along `angle` degrees from east toward south; a ridge's crest runs along it), a flat
 * crown's radius and its noise's seed. */
export interface Hill {
  profile: HillProfile;
  at: [number, number];
  rx: number;
  ry: number;
  angle?: number;
  crown?: number;
  seed: number;
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
  /** R7 173: a quay brushed along the shore (`wharf::shore_line`): `pts` its face along the
   * water's edge, up to 512 points ~5 ft apart, the dry side on the left; `material` timber or stone. */
  shore?: boolean;
  /** A shore quay's piers, this many ft apart (30 to 400; absent: none). */
  piers?: number;
}

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
  /** Where the roads came in when the ward editor first changed it (local ft, class, heading x, y): it is planned from these. */
  roads?: [number, number, number, number, number][];
  /** The street pattern it was laid out on then (kept with `roads`, so an auto town keeps it while the edit stands). */
  pattern?: string;
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
