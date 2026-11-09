// Floor heights inside a site's level (`InteriorLevel.floor`/`ramps`, built by `interior::height`): per square the
// floor's height above the level (ft), and the ramps. The rules: a rise of up to STEP_FT is a step (difficult
// terrain); a larger one between two squares neither of which is a ramp is a sheer edge (a climb up, a drop down);
// a ramp is difficult terrain and joins its floors without a climb.
import type { InteriorLevel } from './protocol';

/** The highest rise that is a step (a dais), not a climb (ft). */
export const STEP_FT = 3;

export interface Floor {
  nx: number;
  ny: number;
  /** Height above the level's floor (ft) per square, 0 off the floor. */
  ft: Float32Array;
  /** 1 on ramp squares. */
  ramp: Uint8Array;
}

const cache = new WeakMap<InteriorLevel, Floor | null>();

/** A level's floor heights; null where it is flat. */
export function floorOf(lv: InteriorLevel, nx: number, ny: number): Floor | null {
  let f = cache.get(lv);
  if (f !== undefined) return f;
  f = null;
  if (lv.floor && lv.floor.length === nx * ny) {
    const ft = new Float32Array(nx * ny);
    for (let k = 0; k < ft.length; k++) ft[k] = (lv.floor.charCodeAt(k) - 48) / 2;
    const ramp = new Uint8Array(nx * ny);
    for (const k of lv.ramps ?? []) ramp[k] = 1;
    f = { nx, ny, ft, ramp };
  }
  cache.set(lv, f);
  return f;
}

/** The sheer rise stepping from square a to square b (ft; negative a drop), 0 where none. */
export function sheer(f: Floor, a: number, b: number): number {
  const d = f.ft[b] - f.ft[a];
  return Math.abs(d) > STEP_FT && !f.ramp[a] && !f.ramp[b] ? d : 0;
}

/** Whether one steps between neighbouring floor squares a and b at all (the same room, or an open natural level). */
const joined = (lv: InteriorLevel, a: number, b: number) => lv.cells[a] >= 0 && lv.cells[b] >= 0 && (lv.natural || lv.cells[a] === lv.cells[b]);

/** The biggest sheer drop from square (i, j) to a square beside it (ft, positive; 0 none): the square is a lip. */
export function lipDrop(f: Floor, lv: InteriorLevel, i: number, j: number): number {
  const { nx, ny } = f;
  const k = j * nx + i;
  let drop = 0;
  for (const [di, dj] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
    const [a, b] = [i + di, j + dj];
    if (a < 0 || b < 0 || a >= nx || b >= ny || !joined(lv, k, b * nx + a)) continue;
    drop = Math.max(drop, -sheer(f, k, b * nx + a));
  }
  return drop;
}

/** The height a square's floor stands at for sight (ft): a step's rise (a dais) hides nothing, so a square raised
 * by up to STEP_FT over its room's floor is seen at the room's. */
export function sightFt(f: Floor, lv: InteriorLevel, k: number): number {
  const r = lv.cells[k];
  if (r < 0) return 0;
  const base = lv.rooms[r]?.raise_ft ?? 0;
  return !f.ramp[k] && f.ft[k] - base <= STEP_FT ? base : f.ft[k];
}

/**
 * The rise across every square edge between two floor squares stepped between, neither a ramp (ft: the east or
 * south square's height minus the west or north one's; 0 level): `v` on the lines x = i ((nx + 1) x ny), `h` on
 * y = j (nx x (ny + 1)). |rise| > STEP_FT is a sheer edge, else a step. For drawing lips, cliff faces and shadows.
 */
export function floorEdges(f: Floor, lv: InteriorLevel): { v: Float32Array; h: Float32Array } {
  const { nx, ny } = f;
  const v = new Float32Array((nx + 1) * ny);
  const h = new Float32Array(nx * (ny + 1));
  for (let j = 0; j < ny; j++)
    for (let i = 0; i < nx; i++) {
      const k = j * nx + i;
      if (i > 0 && joined(lv, k - 1, k) && !f.ramp[k - 1] && !f.ramp[k]) v[j * (nx + 1) + i] = f.ft[k] - f.ft[k - 1];
      if (j > 0 && joined(lv, k - nx, k) && !f.ramp[k - nx] && !f.ramp[k]) h[j * nx + i] = f.ft[k] - f.ft[k - nx];
    }
  return { v, h };
}

/** A square's floor height above its level (ft): from the floor heights, else its room's raise. */
export function floorFt(lv: InteriorLevel, nx: number, ny: number, i: number, j: number): number {
  const f = floorOf(lv, nx, ny);
  const k = j * nx + i;
  return f ? f.ft[k] : (lv.rooms[lv.cells[k]]?.raise_ft ?? 0);
}

/** At a lip (a sheer drop of 5 ft or more beside the square): the cover its edge gives from below (half for a
 * 5-9 ft drop, three-quarters for 10 ft or more) and the note, e.g. "ledge, 10 ft drop"; else null. */
export function lipOf(lv: InteriorLevel, nx: number, ny: number, i: number, j: number): { cover: string; note: string } | null {
  const f = floorOf(lv, nx, ny);
  const drop = f ? lipDrop(f, lv, i, j) : 0;
  if (drop < 5) return null;
  return { cover: `${drop >= 10 ? 'three-quarters' : 'half'} cover from below`, note: `ledge, ${drop} ft drop` };
}
