// An underground level's drawn wall worked out on the CPU, for what has to stand on it: the same sums as
// `underground.frag` makes from the same bytes, walls of smooth rock being a contour of the blurred floor field
// moved about by noise.
import { NOISE_SIZE, noiseBytes } from './noise';
import { FIELD_RES, type UnderField } from './underField';

/** Signed distance to the wall (squares, + on the floor) at a grid point, and the squares whose walls are drawn
 * from something else (nonzero: a shaped room or a cavern, or beside one). */
export type WallDistance = ((x: number, y: number) => number) & { shaped: Uint8Array };

/** The shader's `vnoise`: a lattice byte per square, read through a smoothstep (linear filtering, wrapping). */
function vnoise(x: number, y: number): number {
  const data = noiseBytes();
  const [i, j] = [Math.floor(x), Math.floor(y)];
  let [u, v] = [x - i, y - j];
  u = u * u * (3 - 2 * u);
  v = v * v * (3 - 2 * v);
  const m = NOISE_SIZE - 1;
  const at = (a: number, b: number) => data[((b & m) * NOISE_SIZE + (a & m)) * 4];
  return ((at(i, j) * (1 - u) + at(i + 1, j) * u) * (1 - v) + (at(i, j + 1) * (1 - u) + at(i + 1, j + 1) * u) * v) / 255;
}

const fbm = (x: number, y: number) => 0.55 * vnoise(x, y) + 0.3 * vnoise(x * 2.13 + 7.1, y * 2.13 + 7.1) + 0.15 * vnoise(x * 4.37 + 3.3, y * 4.37 + 3.3);

/** The drawn wall of a level painted from `f`, `rough` as the shader's `uRough`. */
export function wallDistance(f: UnderField, nx: number, ny: number, rough: number): WallDistance {
  const { w, h, pad, data } = f;
  const R = FIELD_RES;
  // Red, bilinear, clamped at the edge: as the GPU reads it.
  const field = (x: number, y: number) => {
    const tx = Math.max(0, Math.min(w - 1, (x + pad) * R - 0.5));
    const ty = Math.max(0, Math.min(h - 1, (y + pad) * R - 0.5));
    const [i, j] = [Math.floor(tx), Math.floor(ty)];
    const [u, v] = [tx - i, ty - j];
    const [a, b] = [Math.min(w - 1, i + 1) - i, (Math.min(h - 1, j + 1) - j) * w];
    const k = (j * w + i) * 4;
    return ((data[k] * (1 - u) + data[k + a * 4] * u) * (1 - v) + (data[k + b * 4] * (1 - u) + data[k + (b + a) * 4] * u) * v) / 255;
  };
  const E = 0.25;
  const dist = (x: number, y: number) => {
    const [gx, gy] = [(field(x + E, y) - field(x - E, y)) / (2 * E), (field(x, y + E) - field(x, y - E)) / (2 * E)];
    const d = (field(x, y) - 0.5) / Math.max(Math.hypot(gx, gy), 0.35);
    return rough ? d + rough * ((fbm(x * 0.8 + 3, y * 0.8 + 3) - 0.5) * 0.75 + (vnoise(x * 3.7, y * 3.7) - 0.5) * 0.25 + (vnoise(x * 9, y * 9) - 0.5) * 0.08) : d;
  };
  const shaped = new Uint8Array(nx * ny);
  for (let k = 0; k < nx * ny; k++) shaped[k] = f.cells[k * 4 + 1];
  return Object.assign(dist, { shaped });
}

const RAYS = 24;
const STEP = 0.25;
/** Across a wall's bumps: how wide a stretch of it a prop's normal is taken over. */
const SPAN = 0.3;

/** The unit step from (x, y) into the rock, over `SPAN`. */
function intoRock(dist: WallDistance, x: number, y: number): [number, number] {
  const [gx, gy] = [dist(x - SPAN, y) - dist(x + SPAN, y), dist(x, y - SPAN) - dist(x, y + SPAN)];
  const n = Math.max(Math.hypot(gx, gy), 1e-9);
  return [gx / n, gy / n];
}

/** A wall near (cx, cy): the point on it and the unit step from it into the rock. Rays find where the distance
 * changes sign, up to `reach` squares; `rate` marks each find 0 fine, 1 to be avoided (the ray's unit step given; a
 * way round goes up to `detour` farther than the nearest), 2 never. The best, the nearest of those, is slid onto the
 * wall along its normal. Null if none. */
export function nearestWall(dist: WallDistance, cx: number, cy: number, reach: number, detour: number, rate: (x: number, y: number, ux: number, uy: number) => number): { x: number; y: number; dx: number; dy: number } | null {
  const start = dist(cx, cy);
  const found: { t: number; x: number; y: number; ux: number; uy: number }[] = [];
  let far = reach;
  for (let a = 0; a < RAYS; a++) {
    const [ux, uy] = [Math.cos((a / RAYS) * Math.PI * 2), Math.sin((a / RAYS) * Math.PI * 2)];
    let d0 = start;
    for (let t = STEP; t <= far + STEP / 2; t += STEP) {
      const d1 = dist(cx + ux * t, cy + uy * t);
      if (d1 >= 0 !== start >= 0) {
        const c = t - STEP + (STEP * d0) / (d0 - d1);
        found.push({ t: c, x: cx + ux * c, y: cy + uy * c, ux, uy });
        far = Math.min(far, c + detour);
        break;
      }
      d0 = d1;
    }
  }
  found.sort((p, q) => p.t - q.t);
  let best: (typeof found)[number] | null = null;
  let bestRate = 2;
  for (const c of found) {
    if (c.t > found[0].t + detour) break;
    const r = rate(c.x, c.y, c.ux, c.uy);
    if (r < bestRate) [best, bestRate] = [c, r];
    if (!r) break;
  }
  if (!best) return null;
  // (The closest try: on bumpy rock a slide can overshoot.)
  let { x, y } = best;
  let [px, py, least] = [x, y, Math.abs(dist(x, y))];
  for (let k = 0; k < 3 && least <= SPAN; k++) {
    const d = dist(px, py);
    const [nx, ny] = intoRock(dist, px, py);
    px += nx * d;
    py += ny * d;
    const e = Math.abs(dist(px, py));
    if (e < least) [x, y, least] = [px, py, e];
  }
  const [dx, dy] = intoRock(dist, x, y);
  return { x, y, dx, dy };
}
