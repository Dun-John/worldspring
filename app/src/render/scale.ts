// The atlas's scale, apart from `atlas.ts` so the vector drawings it reuses (`walls.ts`, `bridges.ts`)
// can be imported into it without a cycle.

/** Atlas pixels per 5-ft square. */
export const PX = 64;
/** Feet in a battlemap square. */
export const SQUARE_FT = 5;
/** Mip levels kept by the sprite atlases (full size down to 1/8), and the empty gap round every frame, on a
 * grid of the same size, so that even the smallest level never blends two frames. */
export const MIP_LEVELS = 4;
export const PAD = 1 << (MIP_LEVELS - 1);
