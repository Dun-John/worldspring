import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { WALL_GAP } from './home';

/** Life size for furniture. Boxes come from the generator and the drawings fill about 90% of them, so a
 * 4 ft stool read as 9 ft. Each entry here scales the finished drawing (points only: strokes keep their
 * width, so the ink stays at the rule) toward real size, about the wall for wall pieces and about the centre
 * for free ones. `wall`: [a, p], the factors along the back axis (depth, or the length of a bed) and across
 * it, anchored on the wall side, centred across. `free`: [l, s] along the box's long side (x when square)
 * and across it, about the centre. A single number scales both alike. The factors are real over drawn
 * size (`docs/roadmap7/sprite-audit/scale.md`, feet). */
type Life = { wall: [number, number]; def?: 'long' | 'wide' | 'up' } | { free: number | [number, number] };

export const LIFE: Record<string, Life | ((f: InteriorItem) => Life)> = {
  shelf: { wall: [0.49, 1] },
  bookcase: { wall: [0.8, 1] },
  weapon_rack: { wall: [0.66, 1] },
  workbench: { wall: [0.45, 1] },
  sideboard: { wall: [0.58, 1] },
  counter: { wall: [0.52, 1] },
  bar: { wall: [0.5, 1] },
  hearth: { wall: [0.62, 1] },
  bed: (f) => ({ wall: [0.68, Math.min(f.w, f.h) >= 2 ? 0.55 : 0.64], def: 'long' }),
  cot: { wall: [0.6, 0.66], def: 'long' },
  yurt_pallet: { wall: [0.65, 0.67], def: 'wide' },
  desk: { free: [1, 0.5] },
  bench: { free: [1, 0.52] },
  pew: { free: [1, 0.36] },
  table: (f) => ({ free: f.w === 1 && f.h === 1 ? 0.67 : [1, 0.7] }),
  long_table: { free: [1, 0.7] },
  bath: (f) => ({ free: f.w === f.h ? 0.45 : 1 }),
  barrel: { free: 0.51 },
  crate: { free: 0.68 },
  bucket: { free: 0.8 },
  chest: { free: [0.77, 0.55] },
  strongbox: { free: [0.95, 0.8] },
  painted_chest: { free: [0.84, 0.66] },
  wardrobe: { free: [0.89, 0.47] },
  chair: { free: 0.64 },
  floor_cushion: { free: 0.8 },
  water_jar: { free: 0.64 },
  anvil: { free: 0.62 },
  cauldron: { free: 0.7 },
  font: { free: 0.68 },
  bell: { free: 0.7 },
};

/** The same for underground props (`drawUnderProp`): all free ones, drawn about their centre. */
export const LIFE_UNDER: Record<string, Life> = {
  offering: { free: 0.3 },
  brazier: { free: 0.92 },
  ore_cart: { free: 1.25 },
  arcane_focus: { free: 0.9 },
  boulder: { free: 0.6 },
  coffin: { free: [0.72, 0.54] },
  effigy: { free: [0.6, 0.38] },
  pit: { free: 0.82 },
};

/** The side a piece set against a wall has its back to, when the room did not say (as `dispatch` defaults). */
export function lifeBack(f: InteriorItem, back: [number, number] | null): [number, number] {
  if (back) return back;
  const l = LIFE[f.kind];
  const life = typeof l === 'function' ? l(f) : l;
  const def = life && 'wall' in life ? life.def : undefined;
  if (def === 'long') return f.w > f.h ? [-1, 0] : [0, -1];
  if (def === 'wide') return f.w >= f.h ? [0, -1] : [-1, 0];
  return [0, -1];
}

/** Draws with `fn` scaled by (kx, ky) about the world point (ax, ay). Pixi's matrix moves points, not an
 * arc's radius, so that is scaled here. */
export function scaledAbout(g: Graphics, kx: number, ky: number, ax: number, ay: number, fn: (g: Graphics) => void) {
  const rk = Math.sqrt(kx * ky);
  const thin: Graphics = new Proxy(g, {
    get(target, prop, receiver) {
      const m = Reflect.get(target, prop, receiver);
      if (typeof m !== 'function') return m;
      return (...args: unknown[]) => {
        const a = prop === 'arc' ? [args[0], args[1], (args[2] as number) * rk, ...args.slice(3)] : args;
        const out = (m as (...x: unknown[]) => unknown).apply(target, a);
        return out === target ? thin : out;
      };
    },
  });
  g.save().setTransform(kx, 0, 0, ky, ax * (1 - kx), ay * (1 - ky));
  fn(thin);
  g.restore();
}

/** Draws `f` with `fn` at life size, if it has an entry; `across` overrides the centre across the back axis
 * (a keg in a rack scales about itself). Returns false when it has none (the caller draws as is). */
export function lifeSize(g: Graphics, f: InteriorItem, back: [number, number] | null, fn: (g: Graphics) => void, across?: number, table = LIFE): boolean {
  const l = table[f.kind];
  const life = typeof l === 'function' ? l(f) : l;
  if (!life) return false;
  const cx = f.x + f.w / 2;
  const cy = f.y + f.h / 2;
  if ('free' in life) {
    const [kl, ks] = typeof life.free === 'number' ? [life.free, life.free] : life.free;
    const [kx, ky] = f.w >= f.h ? [kl, ks] : [ks, kl];
    if (kx === 1 && ky === 1) return false;
    scaledAbout(g, kx, ky, cx, cy, fn);
    return true;
  }
  const [bx, by] = lifeBack(f, back);
  const [a, p] = life.wall;
  const wx = bx < 0 ? f.x + WALL_GAP : bx > 0 ? f.x + f.w - WALL_GAP : (across ?? cx);
  const wy = by < 0 ? f.y + WALL_GAP : by > 0 ? f.y + f.h - WALL_GAP : (across ?? cy);
  scaledAbout(g, bx ? a : p, by ? a : p, wx, wy, fn);
  return true;
}
