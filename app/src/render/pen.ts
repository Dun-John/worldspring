// Ink colour and line widths shared by the vector sprite drawings (atlas, bridges, walls, harbour, entrances).
// A leaf (no pixi): imported by workers too.
import { PX } from './scale';

/** The outline ink of the cartoon sprites. */
export const INK = 0x1d1a14;
/** Atlas pixels per foot. */
export const FT = PX / 5;
/** Ink widths by size (the sprite style rule): outlines of shapes over half a square, of smaller ones. */
export const OUT = 0.045 * PX;
export const LINE = 0.025 * PX;
