// The sprites' drawing code: the battlemap atlas (`atlas/build.ts`) and the furniture and underground props
// (`furniture/*`), kept out of the code the page starts with. Loaded by `loadSprites` (spriteCode.ts) before
// the first battlemap or interior layer is made, and read through `sprites()`.
export { buildAtlas } from './atlas/build';
export { columnTones, stoneColumn } from './furniture/sacred';
export { drawFurniture } from './furniture/dispatch';
export { gravelPatch } from './furniture/under';
export { nicheMouth } from './furniture/tomb';
export { drawUnderProp } from './furniture/underprops';
