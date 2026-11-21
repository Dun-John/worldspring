// How close the map must be for a tool to act: one place for the numbers, the banner's words and where Zoom in flies.
import { TILE_N, type Geom } from '../gen/protocol';

/** Build tools (buildings, roads, water, docks, castle...) act below this many feet per pixel. */
export const BUILD_FT_PER_PX = 2;
/** The Ground brushes need the battlemap: below this many feet per pixel. */
export const GROUND_FT_PER_PX = 0.7;
/** Hill also works on the map's finest levels: this many below the top. */
export const HILL_LEVELS_BELOW_TOP = 3;

export type ReachKind = 'build' | 'ground' | 'hill';

export interface Reach {
  /** For the banner: "Zoom in to {where} to ...". */
  where: string;
  /** The camera zoom to fly to (log2 of pixels per foot). */
  zoom: number;
}

/** The camera zoom at which map level `l` starts. */
function levelZoom(geom: Pick<Geom, 'domain_ft'>, resolution: number, l: number): number {
  return Math.log2((TILE_N * 2 ** l) / (geom.domain_ft * resolution));
}

/** What a tool of `kind` needs, as the level showing at its distance (named while the map has it; the finest level
 * stays up a while as one zooms on, so "L14" means the part of it that is near enough) or the battlemap. */
export function reachOf(kind: ReachKind, geom: Pick<Geom, 'domain_ft' | 'max_level'>, resolution: number): Reach {
  const ft = kind === 'build' ? BUILD_FT_PER_PX : GROUND_FT_PER_PX;
  const minZoom = Math.log2(1 / ft);
  let level: number;
  if (kind === 'hill') level = Math.max(0, geom.max_level - HILL_LEVELS_BELOW_TOP);
  else level = Math.floor(Math.log2((geom.domain_ft * resolution) / (TILE_N * ft)));
  if (level > geom.max_level) return { where: 'a battlemap', zoom: minZoom + 0.05 };
  return { where: `L${level} or a battlemap`, zoom: Math.max(levelZoom(geom, resolution, level), kind === 'hill' ? -99 : minZoom) + 0.02 };
}
