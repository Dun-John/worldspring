import type { FenceKind, TownEdit } from './features';
import type { Rect } from './messages';
import type { Conflict } from './world';

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
  /** (Roadmap 7, 164) A U's or a court's yard: its ranges' box (four corners, grid units); the ground in
   * it outside the footprint is open yard, shown undimmed (as on the battlemap) on and above the ground floor. */
  yard?: [number, number][];
}

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
  /** Secret: the DM sees it, players don't until it is found (a cave behind a waterfall). */
  secret?: boolean;
  /** A secret place's site (`u:<layout>:0`). */
  site?: string;
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
  /** The active volcanoes' craters (`t0::volcano::Crater`): `[x, y, radius, lava radius, scorch radius]` ft and the edges' phase (`Crater::phase`); their floors hold lava. */
  craters?: number[][];
}

/** A sea lane: `p` its line (world ft, x y pairs), `c` class (0 busy, 1 normal, 2 quiet), `f`
 * flow, `e` its ends (a settlement's layout index, or -1 where lanes meet). */
export interface SeaLane {
  c: number;
  f: number;
  e: [number, number];
  p: number[];
}

/** A town's plan alone (`town::standin`), whole ft, flat point lists: wall runs (closed 0/1
 * first), gates, main streets, bridges, ward patches (`Ward as u8` first, 255 none). */
export interface TownStandin {
  w: number[][];
  g: number[];
  s: number[][];
  b: number[][];
  p: number[][];
  /** Water (sea, lake, river) over the patches: runs x0, x1, y (ft) of cells `c` ft tall. */
  c: number;
  q: number[];
}
