// The textures an underground level is painted from (`underground.frag`): a smooth floor field
// (4 texels a square, blurred), liquids and raised floors likewise, and per-square materials.
// Pure data, so a city's sewer sections can be prepared in a worker (`underField.worker.ts`)
// while the view moves.
import type { Interior } from '../gen/protocol';
import { CAVERN, fromSquares, shapeIndex } from './roomShapes';

/** Field texels per square. */
export const FIELD_RES = 4;

export interface UnderField {
  /** Field texture (w x h, RGBA: floor, liquid, raised) and per-square cells (nx x ny, RGBA:
   * material, shape (0 none, its slot + 1 in `shapes`, 254 none but one next to it, 255 a cavern),
   * room hash, 255). A shaped room's squares are left out of the floor (its doorways stay in): the
   * shader draws it from its shape. */
  w: number;
  h: number;
  data: Uint8Array;
  cells: Uint8Array;
  /** 0 none, 1 sewage, 2 lava. */
  liquidKind: number;
  /** Shaped rooms (`roomShapes.ts`), two texels each in a (count x 2) texture: x0, y0, x1, y1
   * (squares), then shape, material, room hash, 255. One blank slot when there are none. */
  shapes: Uint8Array;
  shapeCount: number;
  /** Caverns breaking in: their floor (and the squares opening into them) blurred as a cave's
   * (w x h, RGBA, red), else null. */
  cave: Uint8Array | null;
}

/** Sites with square corners: built (dressed stone), unlike sewers (which follow their
 * streets) and natural rock. */
export const SQUARE_SITES = new Set(['dungeon', 'crypt', 'catacombs', 'deep dungeons']);

/** Floor material for the shader: 0 cave rock, 1 basalt, 2 mine earth, 3 sewer brick,
 * 4 flagstones, 5 boss flagstones, 6 crypt flagstones, 7 marble, 8 hewn blocks, 9 ice. */
export function materialOf(site: string, kind: string, theme?: string): number {
  if (kind === 'sewer tunnel' || kind === 'service passage') return 3;
  if (theme === 'ice') return 9;
  if (site === 'cave') return 0;
  if (theme === 'goblin_warren' || theme === 'bandit_hideout') return 2;
  if (kind === 'boss chamber') return 5;
  if (theme === 'temple' || theme === 'wizard_lair') return 7;
  if (theme === 'dwarven_hall') return 8;
  if (site === 'lava tube') return 1;
  if (site === 'mine') return 2;
  if (site === 'crypt' || site === 'catacombs') return 6;
  return 4;
}

/** Separable Gaussian blur of a w × h field (outside counts as 0, or as the nearest edge
 * value with `clamp`: a sewer section's tunnels then run right up to its edge, to meet the
 * next section's). */
export function blur(src: Float32Array, w: number, h: number, sigma: number, clamp = false): Float32Array {
  const r = Math.ceil(sigma * 2.5);
  const k = new Float32Array(2 * r + 1);
  let sum = 0;
  for (let i = -r; i <= r; i++) sum += k[i + r] = Math.exp(-(i * i) / (2 * sigma * sigma));
  for (let i = 0; i < k.length; i++) k[i] /= sum;
  const tmp = new Float32Array(w * h);
  const out = new Float32Array(w * h);
  // Along rows (bounds checked only near the ends), then whole rows at a time down columns.
  for (let y = 0; y < h; y++) {
    const o = y * w;
    for (let x = 0; x < w; x++) {
      let a = 0;
      if (x >= r && x < w - r) {
        for (let i = -r; i <= r; i++) a += src[o + x + i] * k[i + r];
      } else {
        for (let i = -r; i <= r; i++) {
          const xi = clamp ? Math.min(w - 1, Math.max(0, x + i)) : x + i;
          if (xi >= 0 && xi < w) a += src[o + xi] * k[i + r];
        }
      }
      tmp[o + x] = a;
    }
  }
  for (let y = 0; y < h; y++) {
    const o = y * w;
    for (let i = -r; i <= r; i++) {
      const yi = clamp ? Math.min(h - 1, Math.max(0, y + i)) : y + i;
      if (yi < 0 || yi >= h) continue;
      const q = yi * w;
      const kk = k[i + r];
      for (let x = 0; x < w; x++) out[o + x] += tmp[q + x] * kk;
    }
  }
  return out;
}

/** `blur` worked out at 1/`f` of the resolution and sampled back up (bilinear): for wide,
 * soft masks, at a fraction of the cost. */
function blurCoarse(src: Float32Array, w: number, h: number, sigma: number, f: number, clamp = false): Float32Array {
  const [cw, ch] = [Math.ceil(w / f), Math.ceil(h / f)];
  const small = new Float32Array(cw * ch);
  const inv = 1 / (f * f);
  for (let y = 0; y < h; y++) {
    const row = Math.floor(y / f) * cw;
    for (let x = 0; x < w; x++) small[row + ((x / f) | 0)] += src[y * w + x] * inv;
  }
  const b = blur(small, cw, ch, sigma / f, clamp);
  const out = new Float32Array(w * h);
  // Where each column samples from (the same for every row).
  const x0s = new Int32Array(w);
  const fxs = new Float32Array(w);
  for (let x = 0; x < w; x++) {
    const sx = Math.min(cw - 1, Math.max(0, (x + 0.5) / f - 0.5));
    x0s[x] = Math.floor(sx);
    fxs[x] = sx - x0s[x];
  }
  for (let y = 0; y < h; y++) {
    const sy = Math.min(ch - 1, Math.max(0, (y + 0.5) / f - 0.5));
    const y0 = Math.floor(sy);
    const r0 = y0 * cw;
    const r1 = Math.min(ch - 1, y0 + 1) * cw;
    const fy = sy - y0;
    for (let x = 0; x < w; x++) {
      const x0 = x0s[x];
      const x1 = Math.min(cw - 1, x0 + 1);
      const fx = fxs[x];
      const top = b[r0 + x0] * (1 - fx) + b[r0 + x1] * fx;
      const bot = b[r1 + x0] * (1 - fx) + b[r1 + x1] * fx;
      out[y * w + x] = top * (1 - fy) + bot * fy;
    }
  }
  return out;
}

/** The textures of a level of an underground site. `clamp`: a section of a city's sewers (its
 * tunnels run to the edge, to meet the next section's). */
export function underField(it: Interior, level: number, clamp: boolean): UnderField {
  const lv = it.levels[level];
  const { nx, ny } = it;
  const R = FIELD_RES;
  const [w, h] = [nx * R, ny * R];
  const floor = new Float32Array(w * h);
  const liquid = new Float32Array(w * h);
  const raised = new Float32Array(w * h);
  const wet = new Uint8Array(nx * ny);
  let liquidKind = 0;
  for (const f of lv.furniture) {
    if (f.kind === 'sewage' || f.kind === 'lava') {
      wet[f.y * nx + f.x] = 1;
      liquidKind = f.kind === 'sewage' ? 1 : 2;
    }
  }
  const cells = new Uint8Array(nx * ny * 4);
  const paths = lv.paths ?? [];
  const { index, shapes: slots, caverns } = shapeIndex(lv, nx, ny);
  let anyRaised = false;
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const k = j * nx + i;
      const r = lv.cells[k];
      if (r < 0) continue;
      const room = lv.rooms[r];
      cells[k * 4] = room.shape === CAVERN ? 0 : materialOf(it.function, room.kind, it.theme);
      cells[k * 4 + 1] = index[k];
      cells[k * 4 + 2] = (r * 37) & 255;
      cells[k * 4 + 3] = 255;
      // Floors that follow lines (sewers under streets) are painted from the lines below; shaped
      // rooms from their shapes; anything else fills its squares.
      const square = (!paths.length || room.kind !== 'sewer tunnel') && fromSquares(index[k]);
      const lift = Math.min(1, room.raise_ft / 10);
      if (lift > 0) anyRaised = true;
      for (let y = j * R; y < (j + 1) * R; y++) {
        const o = y * w;
        for (let x = i * R; x < (i + 1) * R; x++) {
          if (square) floor[o + x] = 1;
          liquid[o + x] = wet[k];
          raised[o + x] = lift;
        }
      }
    }
  }
  if (paths.length) {
    // Along the lines: the band within reach, where it borders real floor (runs dropped from
    // the sewer stay rock), so diagonal tunnels are smooth rather than stepped.
    const near = new Uint8Array(nx * ny);
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        if (lv.cells[j * nx + i] < 0) continue;
        for (let b = Math.max(0, j - 1); b <= Math.min(ny - 1, j + 1); b++) for (let a = Math.max(0, i - 1); a <= Math.min(nx - 1, i + 1); a++) near[b * nx + a] = 1;
      }
    }
    for (const [x0, y0, x1, y1, r] of paths) {
      const dx = x1 - x0;
      const dy = y1 - y0;
      const len2 = Math.max(dx * dx + dy * dy, 1e-9);
      const r2 = r * r;
      const tx0 = Math.max(0, Math.floor((Math.min(x0, x1) - r) * R));
      const ty0 = Math.max(0, Math.floor((Math.min(y0, y1) - r) * R));
      const tx1 = Math.min(w - 1, Math.ceil((Math.max(x0, x1) + r) * R));
      const ty1 = Math.min(h - 1, Math.ceil((Math.max(y0, y1) + r) * R));
      for (let ty = ty0; ty <= ty1; ty++) {
        const py = (ty + 0.5) / R;
        const row = Math.floor(py) * nx;
        for (let tx = tx0; tx <= tx1; tx++) {
          const px = (tx + 0.5) / R;
          if (!near[row + Math.floor(px)]) continue;
          const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / len2));
          const ex = px - x0 - t * dx;
          const ey = py - y0 - t * dy;
          if (ex * ex + ey * ey <= r2) floor[ty * w + tx] = 1;
        }
      }
    }
    // The sewage channel: the middle of the wider runs, from a wide blur of the floor.
    liquidKind = 1;
    const wide = blurCoarse(floor, w, h, 1.1 * R, 2, clamp);
    for (let k = 0; k < w * h; k++) liquid[k] = floor[k] * Math.max(0, Math.min(1, (wide[k] - 0.66) / 0.08));
  }
  // Natural rock is rounded; sewers follow their streets; built sites keep square corners
  // (the shader measures their walls from the squares).
  const f1 = SQUARE_SITES.has(it.function) ? floor : blur(floor, w, h, (lv.natural ? 0.42 : paths.length ? 0.22 : 0.14) * R, clamp);
  // A lava channel is one square wide, stepping diagonally: blur it wider and lift it so it
  // runs as one flow rather than beads.
  let f2: Float32Array = liquid;
  if (liquidKind === 1) f2 = blur(liquid, w, h, 0.3 * R, clamp);
  if (liquidKind === 2) f2 = blur(liquid, w, h, 0.6 * R, clamp).map((v) => Math.min(1, v * 1.7));
  const f3 = anyRaised ? blur(raised, w, h, 0.3 * R, clamp) : raised;
  const data = new Uint8Array(w * h * 4);
  for (let k = 0; k < w * h; k++) {
    const o = k * 4;
    data[o] = (f1[k] * 255 + 0.5) | 0;
    data[o + 1] = (f2[k] * 255 + 0.5) | 0;
    data[o + 2] = (f3[k] * 255 + 0.5) | 0;
    data[o + 3] = 255;
  }
  // (Rock beside a shape too: the shader looks round there, the shape reaching into it.)
  for (let k = 0; k < nx * ny; k++) cells[k * 4 + 1] = index[k];
  // Shaped rooms' boxes and looks.
  const n = Math.max(1, slots.length);
  const shapes = new Uint8Array(n * 2 * 4);
  slots.forEach(({ box, shape, room }, s) => {
    shapes.set([box[0], box[1], box[2], box[3]], s * 4);
    shapes.set([shape, materialOf(it.function, lv.rooms[room].kind, it.theme), (room * 37) & 255, 255], (n + s) * 4);
  });
  // Caverns: their floor and the squares opening into them, blurred as a cave's.
  let cave: Uint8Array | null = null;
  if (caverns) {
    const mask = new Float32Array(w * h);
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        const k = j * nx + i;
        if (lv.cells[k] < 0) continue;
        const near = (a: number, b: number) => a >= 0 && b >= 0 && a < nx && b < ny && index[b * nx + a] === CAVERN;
        if (!(index[k] === CAVERN || near(i - 1, j) || near(i + 1, j) || near(i, j - 1) || near(i, j + 1))) continue;
        for (let y = j * R; y < (j + 1) * R; y++) mask.fill(1, y * w + i * R, y * w + (i + 1) * R);
      }
    }
    const b = blur(mask, w, h, 0.42 * R, clamp);
    cave = new Uint8Array(w * h * 4);
    for (let k = 0; k < w * h; k++) cave[k * 4] = (b[k] * 255 + 0.5) | 0;
  }
  return { w, h, data, cells, liquidKind, shapes, shapeCount: slots.length, cave };
}
