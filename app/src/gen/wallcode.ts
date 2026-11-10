// A town wall's style code as the generator packs it (`town::wallstyle::WallStyle::code`): group (3 bits),
// a tower's shape (2, shifted 3), material (2, 5), merlons (3, 7), stone (3, 10). The plan's tile geometry
// (`protocol.ts`, in a worker) reads it here; `render/walls.ts` reads the same bits for the battlemap.

export const codeGroup = (code: number) => code & 7;
/** 0 round, 1 square, 2 D, 3 octagon. */
export const codeShape = (code: number) => (code >> 3) & 3;
/** 0 stone, 1 palisade, 2 rammed earth. */
export const codeMaterial = (code: number) => (code >> 5) & 3;
/** 0 granite, 1 limestone, 2 sandstone, 3 rose brick, 4 mud brick (5 timber, 6 earth). */
export const codeStone = (code: number) => (code >> 10) & 7;
