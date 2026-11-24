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

/** Where the standing part of a thing stands in its frame when it is off the middle: px below the centre (above
 * when negative), by kind id, before the frame is turned (its drop shadow falls from there: `BattlemapLayer`).
 * The headstone stands at the head of its grave; the rest stand at the centre. */
export const UPRIGHT_FOOT: Record<number, number> = { 39: -1.35 * FT };

/** Each frame's drawn size: about half its art's width and height averaged, atlas px at scale 1, by kind id and
 * look (filled by `buildAtlas`). A small prop's drop shadow is sized to it, not to its placement radius
 * (`dropShadowOf`). */
export const DRAWN_R: number[][] = [];
