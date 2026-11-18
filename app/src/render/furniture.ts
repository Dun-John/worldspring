// Building furniture, hand-drawn in the battlemap atlas's style: seen straight down, worn wood with square
// corners and slightly uneven edges, ink outlines, lit from the screen's NW (given in grid axes). Drawn
// into the item atlas once per look (InteriorLayer.itemKey), in squares at the item's grid position.
import { PLATFORMS } from '../gen/protocol';
export { type Light } from './furniture/common';
export { columnTones, stoneColumn } from './furniture/sacred';
export { HOUSES, houseOf } from './furniture/hall';
export { drawHorse } from './furniture/guild';
export { type Fit } from './furniture/yurt';
export { ORIENTED, looksOf, lockName, lookName, VARIED, IN_RING, drawFurniture } from './furniture/dispatch';
export { GOD_NAMES } from './furniture/gods';
export { gravelPatch } from './furniture/under';
export { nicheMouth } from './furniture/tomb';
export { type RailRun, railRuns } from './furniture/mine';
export { UNDER_DRAWN, drawUnderProp } from './furniture/underprops';
export { PLATFORMS };
