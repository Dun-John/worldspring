// Ink colour and line widths shared by the vector sprite drawings (atlas, bridges, walls, harbour, entrances).
// A leaf (no pixi): imported by workers too.
import { PX } from './scale';

/** Dev only: `?gallery=1&ink=2b2118` draws every sprite with another ink (the ink A/B sheet); never set in a build. */
const inkOverride = (): number | null => {
  if (!import.meta.env.DEV || typeof location === 'undefined') return null;
  const q = new URLSearchParams(location.search);
  const v = q.has('gallery') ? q.get('ink') : null;
  return v && /^[0-9a-f]{6}$/i.test(v) ? parseInt(v, 16) : null;
};
/** The outline ink of the cartoon sprites. */
export const INK = inkOverride() ?? 0x1d1a14;
/** Atlas pixels per foot. */
export const FT = PX / 5;
/** Ink widths by size (the sprite style rule): outlines of shapes over half a square, of smaller ones. */
export const OUT = 0.045 * PX;
export const LINE = 0.025 * PX;
