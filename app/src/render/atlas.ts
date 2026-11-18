// The battlemap atlas's tables and its frames' shape, apart from its drawing code (`atlas/build.ts`, loaded
// with the other sprites' drawing code: render/sprites.ts).
import { PX } from './scale';
import type { Texture } from 'pixi.js';
import { FT } from './pen';
export { rng, shade, blob, oval } from './atlas/common';

/** Atlas resolution: pixels per 5-ft square. */
export { PX };
/** Looks per kind. Must divide 256: `battlePrep` packs the turned flag above an object's variant byte. */
export const VARIANTS = 4;
/** Kinds with more looks than that (by id; also dividing 256): the market stall, four awnings by four goods. */
const LOOKS: Record<number, number> = { 36: 16, 155: 3 };
/** How many looks a kind has: an object's variant picks `variant % looks(kind)`. */
export const looks = (id: number): number => LOOKS[id] ?? VARIANTS;

export interface Atlas {
  /** frames[kindId][variant] */
  frames: Texture[][];
  /** Soft round shadow blob (unit radius = half the frame). */
  shadow: Texture;
}

/** Where an upright thing drawn from a high angle stands in its frame: its foot, px below the centre, by kind
 * id (its drop shadow falls from there: `BattlemapLayer`). Things seen from straight above stand at the centre. */
export const UPRIGHT_FOOT: Record<number, number> = { 47: 1.7 * FT, 48: 1.85 * FT, 49: 1.1 * FT, 50: 2.15 * FT, 57: 1.3 * FT, 60: 2.3 * FT, 101: 2.2 * FT };
