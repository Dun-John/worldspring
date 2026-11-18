import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { BRASS, DARK_WOOD, INK, LINE, type Light, OUT, WOOD, board, quad, ring, shade } from './common';
import { STONE } from './home';

// ---------------------------------------------------------------------------------------
// A yurt's (`interior::furnish_yurt`): drawn inside its round wall (`Fit`) where they stand by it.

/** A yurt's ring: its middle and the inner face of its wall (grid units). */
export type Fit = { c: [number, number]; r: number };

/** Felt: undyed, and the dyes a yurt's felts and cushions are worked in (madder, ochre, a dark
 * walnut brown, a faded teal), sun-faded and matte. */
const FELT = [0xa8946e, 0xc8b48c, 0xd8c8a2];
const YURT_DYES = [0x985a44, 0xb08a48, 0x6a4a34, 0x4f6e66];
const OCHRE = 0xc29a52;

/** Box `b` (x0, y0, x1, y1) pulled in from its back side and trimmed at its ends till its corners lie
 * inside the ring (the largest such box, not under `minDepth` deep); `b` as it is without a ring. */
function fitBox(b: number[], back: [number, number], fit: Fit | undefined, minDepth = 0.3): number[] {
  if (!fit) return b;
  const inside = (r: number[]) => [r[0], r[2]].every((x) => [r[1], r[3]].every((y) => Math.hypot(x - fit.c[0], y - fit.c[1]) <= fit.r - 0.03));
  const along = back[1] !== 0;
  const depth = along ? b[3] - b[1] : b[2] - b[0];
  let best = b;
  let area = -1;
  for (let t = 0; t <= depth - minDepth + 1e-9; t += 0.02) {
    for (let e = 0; e <= 0.45; e += 0.02) {
      const r = [...b];
      if (back[0] < 0) r[0] += t;
      else if (back[0] > 0) r[2] -= t;
      else if (back[1] < 0) r[1] += t;
      else r[3] -= t;
      if (along) [r[0], r[2]] = [r[0] + e, r[2] - e];
      else [r[1], r[3]] = [r[1] + e, r[3] - e];
      const a = (r[2] - r[0]) * (r[3] - r[1]);
      if (a > area && inside(r)) [best, area] = [r, a];
    }
  }
  return area < 0 ? b : best;
}

/** Points round a rounded rect (corner radius `k`), nudged by up to `j`. */
function soft([x0, y0, x1, y1]: number[], k: number, rnd: () => number, j = 0.008): number[] {
  const pts: number[] = [];
  const corners: [number, number, number][] = [
    [x1 - k, y0 + k, -Math.PI / 2],
    [x1 - k, y1 - k, 0],
    [x0 + k, y1 - k, Math.PI / 2],
    [x0 + k, y0 + k, Math.PI],
  ];
  for (const [cx, cy, a0] of corners) {
    for (let i = 0; i <= 3; i++) {
      const a = a0 + (i / 3) * (Math.PI / 2);
      pts.push(cx + Math.cos(a) * k + (rnd() - 0.5) * j, cy + Math.sin(a) * k + (rnd() - 0.5) * j);
    }
  }
  return pts;
}

/** A felt cushion seen from above, about `s` across: a soft square in its dye, lit toward the light, a
 * seam round its top and a tuft in the middle. */
function cushion(g: Graphics, cx: number, cy: number, s: number, color: number, light: Light, rnd: () => number) {
  const h = s / 2;
  g.poly(soft([cx - h, cy - h, cx + h, cy + h], s * 0.3, rnd)).fill(shade(color, 0.82)).stroke({ width: s > 0.5 ? OUT : LINE, color: INK, join: 'round' });
  const [dx, dy] = [light[0] * s * 0.07, light[1] * s * 0.07];
  g.poly(soft([cx - h * 0.78 + dx, cy - h * 0.78 + dy, cx + h * 0.7 + dx, cy + h * 0.7 + dy], s * 0.24, rnd)).fill(color);
  g.poly(soft([cx - h * 0.62 + dx, cy - h * 0.62 + dy, cx + h * 0.56 + dx, cy + h * 0.56 + dy], s * 0.2, rnd, 0.004)).stroke({ width: 0.016, color: OCHRE, alpha: 0.85 });
  g.circle(cx + dx * 0.5, cy + dy * 0.5, s * 0.06).fill(shade(color, 0.6));
}

/** A yurt's hearth in the middle of its floor: a ring of field stones round grey ash, charred sticks
 * and a warm fire; on a floor of two squares or more each way, cushions round it but on the door's
 * side (`back`). */
export function yurtHearth(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const R = 0.4;
  if (f.w >= 2 && f.h >= 2) {
    for (const [k, [dx, dy]] of ([[0, -1], [1, 0], [0, 1], [-1, 0]] as [number, number][]).entries()) {
      if (dx === back[0] && dy === back[1]) continue;
      cushion(g, cx + dx * 0.74, cy + dy * 0.74, 0.36, YURT_DYES[(v + k) % 4], light, rnd);
    }
  }
  g.poly(ring(cx, cy, R * 0.92, rnd, 0.03, 18)).fill(0x7a7064);
  g.poly(ring(cx + 0.02, cy + 0.02, R * 0.7, rnd, 0.03, 16)).fill(0x8e8478);
  // The stones, lit on their upper left.
  const n = 11;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rnd() * 0.2;
    const [sx, sy] = [cx + Math.cos(a) * R, cy + Math.sin(a) * R];
    const r = 0.07 + rnd() * 0.025;
    g.poly(ring(sx, sy, r, rnd, 0.025, 8)).fill(STONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(ring(sx + light[0] * r * 0.3, sy + light[1] * r * 0.3, r * 0.55, rnd, 0.015, 7)).fill(STONE[2]);
  }
  // Sticks laid to the middle, their ends charred; then the fire over them.
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + 0.4 + rnd() * 0.3;
    const [ax, ay, bx, by] = [cx + Math.cos(a) * 0.3, cy + Math.sin(a) * 0.3, cx + Math.cos(a) * 0.05, cy + Math.sin(a) * 0.05];
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.075, color: INK, cap: 'round' });
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.045, color: WOOD[0], cap: 'round' });
    g.circle(bx + (ax - bx) * 0.35, by + (ay - by) * 0.35, 0.022).fill(0x2a1c14);
  }
  for (const [r, color] of [
    [0.19, 0xc2410c],
    [0.14, 0xe8781e],
    [0.085, 0xf6b23a],
    [0.04, 0xfde6a8],
  ] as [number, number][]) {
    g.poly(ring(cx - 0.01, cy - 0.01, r, rnd, r * 0.35, 12)).fill(color);
  }
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.2 + rnd() * 0.06;
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.016).fill(0xe8781e);
  }
}

/** A low pallet along the wall, its bedding on it: a felt mattress quilted in lines, a dyed blanket
 * over most of it with a border, a rolled blanket at the head. */
export function yurtPallet(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number], fit?: Fit) {
  const along = back[1] !== 0;
  const box = fitBox([f.x + 0.06, f.y + 0.06, f.x + f.w - 0.06, f.y + f.h - 0.06], back, fit, 0.42);
  const pal = board(g, box, DARK_WOOD, light, rnd, along, 0.05, 0);
  const [x0, y0, x1, y1] = [pal[0] + 0.03, pal[1] + 0.03, pal[2] - 0.03, pal[3] - 0.03];
  const L = along ? x1 - x0 : y1 - y0;
  const at = (u: number, s: number): [number, number] => (along ? [x0 + u, y0 + s] : [x0 + s, y0 + u]);
  const W = along ? y1 - y0 : x1 - x0;
  const r = (u0: number, s0: number, u1: number, s1: number) => {
    const [a, b] = [at(u0, s0), at(u1, s1)];
    return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
  };
  g.poly(soft(r(0, 0, L, W), 0.05, rnd)).fill(FELT[1]).stroke({ width: LINE, color: INK, join: 'round' });
  for (let u = 0.12; u < L * 0.3; u += 0.12) g.moveTo(...at(u, 0.04)).lineTo(...at(u, W - 0.04));
  g.stroke({ width: 0.016, color: FELT[0], alpha: 0.9 });
  // (Its head at either end, by its look.)
  const head = v % 2 === 0;
  const [b0, b1] = head ? [L * 0.3, L - 0.03] : [0.03, L * 0.7];
  const dye = YURT_DYES[v % 4];
  g.poly(soft(r(b0, -0.02, b1, W + 0.02), 0.06, rnd, 0.012)).fill(dye).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(soft(r(b0 + 0.06, 0.06, b1 - 0.06, W - 0.06), 0.04, rnd, 0.006)).stroke({ width: 0.02, color: OCHRE, alpha: 0.9 });
  g.moveTo(...at(b0 + 0.02, W * 0.3)).quadraticCurveTo(...at(b0 + 0.1, W * 0.5), ...at(b0 + 0.03, W * 0.72)).stroke({ width: 0.018, color: shade(dye, 0.7) });
  // The rolled blanket across the head.
  const [h0, h1] = head ? [0.02, 0.2] : [L - 0.2, L - 0.02];
  const roll = YURT_DYES[(v + 2) % 4];
  g.poly(soft(r(h0, 0.01, h1, W - 0.01), 0.07, rnd)).fill(roll).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at((h0 + h1) / 2, 0.05)).lineTo(...at((h0 + h1) / 2 + (rnd() - 0.5) * 0.02, W - 0.05)).stroke({ width: 0.016, color: shade(roll, 0.7) });
}

/** A painted chest by the wall, its front toward the room: a lid in red ochre (one look faded teal), an
 * ochre border and a lozenge with curling horns painted on it, a dark front edge, a brass hasp. */
export function paintedChest(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number], fit?: Fit) {
  const along = back[1] !== 0;
  const pad = along ? [0.12, 0.2] : [0.2, 0.12];
  const box = fitBox([f.x + pad[0], f.y + pad[1], f.x + f.w - pad[0], f.y + f.h - pad[1]], back, fit, 0.38);
  const paint = v % 4 === 3 ? [0x3c524c, 0x4f6e66, 0x628078] : [0x74422f, 0x985a44, 0xa86a52];
  const face = board(g, box, paint, light, rnd, along, 0.05, 0);
  // The front edge, toward the room.
  const [fx0, fy0, fx1, fy1] = face;
  const e = 0.06;
  const front = back[0] < 0 ? [fx1 - e, fy0, fx1, fy1] : back[0] > 0 ? [fx0, fy0, fx0 + e, fy1] : back[1] < 0 ? [fx0, fy1 - e, fx1, fy1] : [fx0, fy0, fx1, fy0 + e];
  g.rect(front[0], front[1], front[2] - front[0], front[3] - front[1]).fill(paint[0]);
  const inner = [fx0 + 0.05, fy0 + 0.05, fx1 - 0.05, fy1 - 0.05];
  g.poly(quad(inner, rnd, 0.004)).stroke({ width: 0.02, color: OCHRE });
  const [mx, my] = [(fx0 + fx1) / 2, (fy0 + fy1) / 2];
  const [rx, ry] = [Math.min(0.1, (fx1 - fx0) * 0.22), Math.min(0.1, (fy1 - fy0) * 0.22)];
  g.poly([mx - rx, my, mx, my - ry, mx + rx, my, mx, my + ry]).fill(OCHRE).stroke({ width: 0.016, color: INK });
  // Horns curling off the lozenge's points, along the lid.
  for (const s of [-1, 1]) {
    if (along) g.moveTo(mx + s * rx, my).quadraticCurveTo(mx + s * rx * 2.2, my - ry * 1.4, mx + s * rx * 2.4, my - ry * 0.2);
    else g.moveTo(mx, my + s * ry).quadraticCurveTo(mx - rx * 1.4, my + s * ry * 2.2, mx - rx * 0.2, my + s * ry * 2.4);
  }
  g.stroke({ width: 0.02, color: OCHRE, cap: 'round' });
  const [hx, hy] = [(front[0] + front[2]) / 2, (front[1] + front[3]) / 2];
  g.rect(hx - 0.04, hy - 0.04, 0.08, 0.08).fill(BRASS).stroke({ width: 0.016, color: INK });
}

/** A floor cushion or two by the wall (or by the hearth). */
export function floorCushion(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number], fit?: Fit) {
  const box = fitBox([f.x + 0.14, f.y + 0.14, f.x + f.w - 0.14, f.y + f.h - 0.14], back, fit, 0.4);
  const [cx, cy] = [(box[0] + box[2]) / 2, (box[1] + box[3]) / 2];
  const s = Math.min(0.52, box[2] - box[0], box[3] - box[1]);
  cushion(g, cx, cy, s, YURT_DYES[v % 4], light, rnd);
}
