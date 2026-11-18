import type { CastlePiece, Edits, KeepShape, Moat, RoofMaterial, RoofShape, RoofStyle, SiteSize, Tint } from './edits';

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
