// Building furniture, hand-drawn in the battlemap atlas's style: seen straight down, worn wood with square
// corners and slightly uneven edges, ink outlines, lit from the screen's NW (given in grid axes). Drawn
// into the item atlas once per look (InteriorLayer.itemKey), in squares at the item's grid position.
// This module is its tables (`furniture/tables.ts`); the drawing code is loaded later (render/sprites.ts).
export { type Light } from './furniture/common';
export { type Fit } from './furniture/yurt';
export { HOUSES, houseOf, ORIENTED, looksOf, lockName, lookName, VARIED, IN_RING, GOD_NAMES, UNDER_DRAWN, PLATFORMS } from './furniture/tables';
