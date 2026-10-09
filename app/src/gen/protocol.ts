// Messages between the main thread, the coordinator worker and generator workers,
// plus the terrain payload layout (mirrors crates/worldgen/src/payload.rs).
import type { PreparedChunk } from './battlePrep';

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
  /** A building drawn by hand (`worldgen::world::Created`): its footprint (world ft), storeys,
   * what it is (a business key or a home), roof, roof colour and structure. */
  poly?: [number, number][];
  floors?: number;
  func?: string;
  roof?: RoofStyle;
  tint?: Tint;
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
  removed?: boolean;
}

/** Widest a castle may be and narrowest (ft), longest a wall (`worldgen::world`), how far past
 * its outline a castle's or wall's layout reaches (`WORKS_MARGIN_FT`). */
export const CASTLE_MAX_FT = 600;
export const CASTLE_MIN_FT = 80;
export const WALL_MAX_FT = 3000;
export const WORKS_MARGIN_FT = 40;

export type RoofStyle = 'hip' | 'battlements' | 'cone';
export const ROOFS: RoofStyle[] = ['hip', 'battlements', 'cone'];
/** Roof colours (`worldgen::world::TINTS`, as `battlePrep.ROOF_TINTS`). */
export type Tint = 'terracotta' | 'slate' | 'thatch' | 'shingle' | 'moss';
export const TINTS: Tint[] = ['terracotta', 'slate', 'thatch', 'shingle', 'moss'];
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
  dungeon: ['dungeon', 'prison', 'temple', 'wizard_lair', 'bandit_hideout', 'dwarven_hall', 'goblin_warren', 'flooded_vault'],
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
  structure?: string;
}

export type CrossingKind = 'bridge' | 'ford' | 'ferry';

/** A crossing put down by hand (`worldgen::world::Crossing`): from `a` to `b` (world ft, bank to
 * bank), `width` ft across. */
export interface Crossing {
  kind: CrossingKind;
  a: [number, number];
  b: [number, number];
  width: number;
}

/** Limits on a crossing (`Crossing::WIDTH`, `LENGTH`; a ferry needs room for two 24-ft jetties). */
export const CROSSING_WIDTH: [number, number] = [5, 40];
export const CROSSING_LENGTH: [number, number] = [10, 2000];
export const FERRY_MIN_FT = 68;

/** An underground site designed by hand (`worldgen::under::design::SiteDesign`): a copy of the
 * generated site, changed. Walls are derived; levels run bottom to top, as `Interior`'s. */
export interface SiteDesign {
  /** dungeon, crypt, catacombs, cave, mine, lava_tube. */
  kind: string;
  theme?: string;
  origin: [number, number];
  axis: [number, number];
  nx: number;
  ny: number;
  /** The way in on the top level (under the entrance). */
  entry: [number, number];
  levels: DesignLevel[];
}

export interface DesignLevel {
  name: string;
  elevation_ft: number;
  natural: boolean;
  /** Room per square, run-length encoded: room (-1 rock), count, … */
  cells: number[];
  /** By index (empty ones stay, so room names hold). */
  rooms: { kind: string; raise_ft: number }[];
  /** x, y, side (0 east, 1 south), secret (0/1). */
  doors: [number, number, number, number][];
  /** Props and the ways (`exit`, `up`, `down`), in order. */
  items: { kind: string; x: number; y: number; w: number; h: number }[];
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
export type DesignReply = { design: SiteDesign; interior: Interior; problems: DesignProblem[] } | { error: string };

/** What the designer offers (`worldgen::under::design::catalog_json`). */
export interface UnderCatalog {
  props: { kind: string; name: string; cover: number; blocks: boolean; height_ft: number; hazard: string | null; w: number; h: number }[];
  /** Every room kind. */
  rooms: string[];
  /** Each theme's own room kinds (its first room, its passages, its ledges first). */
  themes: { key: string; kind: string; first: string; passage: string; rooms: string[] }[];
  boss: string;
  max_levels: number;
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
  | { op: 'inview'; rect: [number, number, number, number] }
  /** `b:<settlement>:<building>` or `t:<settlement>:<tower>`. */
  | { op: 'interior'; id: string }
  /** Any place id: its name and position (`worldgen::agent::place`). */
  | { op: 'place'; id: string }
  /** What can be renamed in a layout (its index) or a building or site (its id). */
  | { op: 'names'; scope: string }
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
  /** An underground site as a design (the one given, else its own), changed by `action`, with
   * what it builds and its problems (`under::design::design_json`). */
  | { op: 'design'; id: string; design?: string; action?: string }
  /** What the designer offers (`under::design::catalog_json`). */
  | { op: 'undercat' };

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
}

export interface InteriorLevel {
  z: number;
  name: string;
  elevation_ft: number;
  cells: number[];
  rooms: { kind: string; squares: number; raise_ft: number; center: [number, number] }[];
  walls: { a: [number, number]; b: [number, number]; exterior: boolean }[];
  doors: { a: [number, number]; b: [number, number]; kind: string; rooms: [number, number] }[];
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
  /** Ways to other sites (cellar trapdoors, stairs to deep dungeons, sewer ladders): the
   * square and the site's id; the other site has a way straight back. */
  links?: { x: number; y: number; to: string }[];
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
      district: string | null;
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

export function buildSiteGeometry(polys: TileSitePolys, lines: TileRivers): SiteGeometry | null {
  const n = polys.attrs.length;
  let tris = 0;
  for (let i = 0; i < n; i++) tris += Math.max(0, polys.starts[i + 1] - polys.starts[i] - 2);
  const outlined = (kind: number) => kind === 0 || kind === 3 || kind === 4 || kind === 6 || kind === 7 || kind === 9;
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
      let rgb: [number, number, number];
      let a = 1;
      if (kind === 1) rgb = [0.86, 0.82, 0.71];
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
      } else if (kind === 5) rgb = [0.6, 0.66, 0.47];
      else if (kind === 6) rgb = [0.55, 0.4, 0.26];
      else if (kind === 7) rgb = [0.72, 0.69, 0.62];
      else if (kind === 8) {
        // A camp's clearing: bare earth.
        rgb = [0.72, 0.62, 0.47];
        a = 0.7;
      } else if (kind === 9) rgb = [0.86, 0.8, 0.66];
      else {
        const base = WARD_RGB[ward] ?? WARD_RGB[6];
        const shade = 0.94 + 0.12 * (((i * 2654435761) >>> 0) / 4294967296);
        rgb = hasFunc ? [base[0] * 0.86, base[1] * 0.74, base[2] * 0.66] : [base[0] * shade, base[1] * shade, base[2] * shade];
      }
      const ink = outlined(kind);
      // Fan triangles (0, k, k+1): the edge k..k+1 is always the polygon's; 0..1 only for the
      // first triangle, k+1..0 only for the last.
      for (let k = 1; k + 1 < m; k++) {
        const corners = [s0, s0 + k, s0 + k + 1];
        // Which edges are real, by the corner opposite each.
        const real = [ink, ink && k + 1 === m - 1, ink && k === 1];
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
  // Ink: walls and towers from the line section (building outlines are inked by the fill).
  const lineVerts = lines.verts.length / 4;
  const starts: number[] = [];
  const out = new Float32Array(lineVerts * 4);
  let o = 0;
  for (let l = 0; l + 1 < lines.starts.length; l++) {
    starts.push(o / 4);
    for (let v = lines.starts[l]; v < lines.starts[l + 1]; v++) {
      out.set([lines.verts[v * 4], lines.verts[v * 4 + 1], lines.verts[v * 4 + 2], lines.verts[v * 4 + 3] + 1], o);
      o += 4;
    }
  }
  starts.push(o / 4);
  const ink = buildRiverGeometry({ starts: new Uint32Array(starts), verts: out.subarray(0, o) });
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
   * taken away). */
  | { type: 'edits'; fields: Record<string, string>; patches: Record<string, EditsPatch>; rects: Rect[]; epoch: number; battleRects?: Rect[] };

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
  | { type: 'edits'; json: string | null; patch: string | null };

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
  const [siteLines] = lines(polyVerts + nPolyVerts * 8);
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
