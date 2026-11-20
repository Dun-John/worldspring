import type { Interior } from './interior';
import type { Edits } from './edits';

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

/** How planned a town's streets are (`set_planning_json`): 0-1 as it is now, its class, the rolled
 * value, whether set by hand; or a change's edits (`unchanged`, `set_aside`) like `CultureChange`. */
export type PlanningNow = { value: number; class: string; rolled: number; set: boolean };
export type PlanningChange = { from: number; to?: number; class?: string; unchanged?: boolean; set_aside?: number; set_aside_by?: Record<string, number>; edits?: Edits };

/** A town's street pattern (`set_patterns_json`): `auto` or one of `STREET_PATTERNS`, as it is now and whether set by hand;
 * or a change's edits like `PlanningChange` (`from`, `to`: the pattern's name). */
export const STREET_PATTERNS = ['grid', 'rings', 'radial', 'linear'] as const;
export type PatternNow = { pattern: string; set: boolean };
export type PatternChange = { from: string; to: string; unchanged?: boolean; set_aside?: number; set_aside_by?: Record<string, number>; edits?: Edits };

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
