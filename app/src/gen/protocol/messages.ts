import type { RiverGeometry, SiteGeometry } from '../tileGeometry';
import type { WorldFile } from './world';
import type { Ask } from './ask';
import type { Geom } from './interior';
import type { PreparedChunk } from '../battlePrep';

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
  /** Generated layouts built so far by settlement index: per generator worker, how many times
   * (a debug count: `window.__ws.layoutBuilds()` in dev). */
  builds?: Record<number, number[]>;
  /** The generator workers each settlement's jobs go to (`no`s as in `builds`; debug). */
  owners?: Record<number, number[]>;
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
   * tiles there are packed again (their samples, as generated, still serve finer tiles); `groundLevels`, by rect, the
   * coarsest level packed again (default the third finest: a big hill shows on coarser ones). */
  | { type: 'edits'; fields: Record<string, string>; patches: Record<string, EditsPatch>; rects: Rect[]; epoch: number; battleRects?: Rect[]; groundRects?: Rect[]; allRects?: Rect[]; groundLevels?: number[] };

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
  | { type: 'places'; json: string }
  /** Each settlement's bounds again (after edits): answered with `bounds`. */
  | { type: 'bounds' }
  /** Build settlement `town`'s layout into this worker's memo (`Ctx::warm_town`): answered with `done`. */
  | { type: 'warmtown'; id: number; town: number };

/** `built`: the layouts this worker built as generated meanwhile (`Ctx::take_built`, layout indices). */
export type FromGen =
  /** `bounds`: each settlement's (`Ctx::town_bounds`, x0, y0, x1, y1 flat), from the worker that made T0. */
  | { type: 'inited'; t0: ArrayBuffer | null; overlay: string | null; ms: number; bounds?: Float64Array }
  | { type: 'progress'; stage: string; frac: number }
  | { type: 'done'; id: number; buf: ArrayBuffer | null; chunk?: PreparedChunk; ms: number; built?: number[] }
  | { type: 'answer'; id: number; json: string; built?: number[] }
  | { type: 'bounds'; bounds: Float64Array }
  | { type: 'error'; id?: number; message: string; built?: number[] };
