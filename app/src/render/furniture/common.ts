import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { INK } from '../pen';
import { roundRadii } from './squash';

export { INK };
/** Ink outline and inner line widths, in squares (about 3 and 1.6 px at 64 px a square). */
export const OUT = 0.045;
export const LINE = 0.025;
/** Wood tones: shaded side, face, lit edge. Matte, close together. */
export const WOOD = [0x5e4028, 0x86603c, 0x9c7450];
export const DARK_WOOD = [0x3e2a1c, 0x5e4430, 0x725440];
export const IRON = [0x34322e, 0x5a5e64, 0x7e8288];
export const BRASS = 0xc8a45a;
export const LINEN = [0xc8bca0, 0xe4dac0, 0xf0e8d4];
/** Blanket and cloth dyes (sun-faded): cornflower, madder, moss, plum. */
export const DYES = [0x4f72a8, 0xb5523a, 0x6a8a3a, 0x7a4a6a];
export const BOOKS = [0x8a2e26, 0x2e5a6a, 0xb08a3a, 0x5a3a5e, 0x4a6a32, 0x3e5a8a, 0x7a4a2a, 0x9a6a4a, 0x6a2e2e];

export type Light = [number, number];

export function shade(color: number, f: number): number {
  const ch = (s: number) => Math.min(255, Math.max(0, Math.round(((color >> s) & 255) * f)));
  return (ch(16) << 16) | (ch(8) << 8) | ch(0);
}

/** `color` a step less saturated: pulled `k` of the way toward a warm grey of its own lightness. */
export function mute(color: number, k = 0.3): number {
  const [r, g, b] = [(color >> 16) & 255, (color >> 8) & 255, color & 255];
  const l = 0.3 * r + 0.59 * g + 0.11 * b;
  const ch = (c: number, w: number) => Math.round(c + (l * w - c) * k);
  return (ch(r, 1.04) << 16) | (ch(g, 1) << 8) | ch(b, 0.92);
}

/** An item laid out along its long side: `at(u, s)` is the grid point `u` squares along it and `s`
 * across, so one drawing serves both orientations. With `back` (the side against a wall, a unit step),
 * the item runs along that wall and `s` 0 is at the wall. */
export function frame(f: InteriorItem, back: [number, number] | null = null) {
  const along = back ? back[1] !== 0 : f.w >= f.h;
  const flip = back ? back[0] + back[1] > 0 : false;
  const len = along ? f.w : f.h;
  const wid = along ? f.h : f.w;
  const at = (u: number, s0: number): [number, number] => {
    const s = flip ? wid - s0 : s0;
    return along ? [f.x + u, f.y + s] : [f.x + s, f.y + u];
  };
  /** The grid rect spanning u0..u1 along and s0..s1 across, as [x0, y0, x1, y1]. */
  const rect = (u0: number, s0: number, u1: number, s1: number): number[] => {
    const [ax, ay] = at(u0, s0);
    const [bx, by] = at(u1, s1);
    return [Math.min(ax, bx), Math.min(ay, by), Math.max(ax, bx), Math.max(ay, by)];
  };
  return { along, len, at, rect };
}

/** A hand-drawn quad: the rect's corners nudged by up to `j` squares, so edges aren't ruler-straight. */
export function quad([x0, y0, x1, y1]: number[], rnd: () => number, j = 0.012): number[] {
  const n = () => (rnd() - 0.5) * 2 * j;
  return [x0 + n(), y0 + n(), x1 + n(), y0 + n(), x1 + n(), y1 + n(), x0 + n(), y1 + n()];
}

/** The strips along a rect's sides that face the light, `s` wide. */
export function litEdges(g: Graphics, [x0, y0, x1, y1]: number[], [lx, ly]: Light, s: number) {
  if (lx < -0.3) g.rect(x0, y0, s, y1 - y0);
  if (lx > 0.3) g.rect(x1 - s, y0, s, y1 - y0);
  if (ly < -0.3) g.rect(x0, y0, x1 - x0, s);
  if (ly > 0.3) g.rect(x0, y1 - s, x1 - x0, s);
}

/** A worn board or box from above: square corners, a darker rim on the side away from the light (the
 * face shifted toward it), a faint lit edge, a few grain lines along `along`. Returns the face. */
export function board(g: Graphics, r: number[], t: readonly number[], light: Light, rnd: () => number, along = true, e = 0.05, grain = 3, ink = OUT): number[] {
  const [lx, ly] = light;
  g.poly(quad(r, rnd)).fill(t[0]).stroke({ width: ink, color: INK, join: 'round' });
  const [dx, dy] = [lx * e * 0.7, ly * e * 0.7];
  const face = [r[0] + e * 0.6 + dx, r[1] + e * 0.6 + dy, r[2] - e * 0.6 + dx, r[3] - e * 0.6 + dy];
  g.poly(quad(face, rnd, 0.008)).fill(t[1]);
  litEdges(g, face, light, e * 0.5);
  g.fill({ color: t[2], alpha: 0.5 });
  // Grain: short wavy lines along the board.
  const [fx0, fy0, fx1, fy1] = face;
  const L = along ? fx1 - fx0 : fy1 - fy0;
  const W = along ? fy1 - fy0 : fx1 - fx0;
  if (grain > 0) {
    for (let i = 0; i < grain * Math.max(1, Math.round(L)); i++) {
      const s0 = W * (0.15 + 0.7 * rnd());
      const u0 = L * rnd() * 0.7;
      const u1 = Math.min(u0 + L * (0.12 + 0.2 * rnd()), L - 0.02);
      const w = (rnd() - 0.5) * W * 0.08;
      const p = (u: number, s1: number): [number, number] => (along ? [fx0 + u, fy0 + s1] : [fx0 + s1, fy0 + u]);
      g.moveTo(...p(u0, s0)).quadraticCurveTo(...p((u0 + u1) / 2, s0 + w), ...p(u1, s0));
    }
    g.stroke({ width: 0.014, color: t[0], alpha: 0.7, cap: 'round' });
  }
  return face;
}

/** Seams between planks along a face, `n` planks. */
export function planks(g: Graphics, face: number[], along: boolean, n: number, color: number, rnd: () => number) {
  const [x0, y0, x1, y1] = face;
  for (let k = 1; k < n; k++) {
    const t = k / n + (rnd() - 0.5) * 0.04;
    if (along) g.moveTo(x0, y0 + (y1 - y0) * t).lineTo(x1, y0 + (y1 - y0) * t + (rnd() - 0.5) * 0.01);
    else g.moveTo(x0 + (x1 - x0) * t, y0).lineTo(x0 + (x1 - x0) * t + (rnd() - 0.5) * 0.01, y1);
  }
  g.stroke({ width: LINE, color, alpha: 0.85 });
}

/** How much a direction faces the screen-NW light in grid axes, unrotated: 1 toward it, -1 away. */
export function lit(a: number): number {
  return -(Math.cos(a) + Math.sin(a)) * Math.SQRT1_2;
}

/** An ellipse as points, turned by `rot`. */
export function oval(cx: number, cy: number, rx: number, ry: number, rot: number, n = 16): number[] {
  // Round things stay round under an uneven life-size squash (see `scaledAbout`).
  if (rx === ry) [rx, ry] = roundRadii(rx);
  const [c, sn] = [Math.cos(rot), Math.sin(rot)];
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2;
    const [x, y] = [Math.cos(t) * rx, Math.sin(t) * ry];
    pts.push(cx + x * c - y * sn, cy + x * sn + y * c);
  }
  return pts;
}

/** A rough ring of points round (cx, cy). */
export function ring(cx: number, cy: number, r: number, rnd: () => number, j: number, n = 20): number[] {
  const pts: number[] = [];
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2;
    const rr = r + (rnd() - 0.5) * j;
    const [rx, ry] = roundRadii(rr);
    pts.push(cx + Math.cos(a) * rx, cy + Math.sin(a) * ry);
  }
  return pts;
}


/** A shadow cast across the floor away from the light by something `h` squares tall: the footprint `pts`
 * swept that far and blurred (three passes, fainter outward). */
export function castShadow(g: Graphics, pts: number[], [lx, ly]: Light, h: number) {
  for (const [k, a] of [
    [1, 0.12],
    [0.75, 0.14],
    [0.5, 0.16],
  ]) {
    const [dx, dy] = [-lx * h * k, -ly * h * k];
    const n = pts.length / 2;
    // The hull of the footprint and its moved copy: good enough for the round and square bases here.
    const all: number[][] = [];
    for (let i = 0; i < n; i++) all.push([pts[i * 2], pts[i * 2 + 1]], [pts[i * 2] + dx, pts[i * 2 + 1] + dy]);
    all.sort((p, q) => p[0] - q[0] || p[1] - q[1]);
    const cross = (o: number[], p: number[], q: number[]) => (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
    const lower: number[][] = [];
    for (const p of all) {
      while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], p) <= 0) lower.pop();
      lower.push(p);
    }
    const upper: number[][] = [];
    for (let i = all.length - 1; i >= 0; i--) {
      const p = all[i];
      while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], p) <= 0) upper.pop();
      upper.push(p);
    }
    g.poly([...lower.slice(0, -1), ...upper.slice(0, -1)].flat()).fill({ color: 0x14100c, alpha: a });
  }
}

/** A rounded, rough-edged patch of points round (cx, cy). */
export function patch(cx: number, cy: number, rx: number, ry: number, rnd: () => number, j = 0.18, n = 18): number[] {
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const k = 1 + (rnd() - 0.5) * j * 2;
    pts.push(cx + Math.cos(a) * rx * k, cy + Math.sin(a) * ry * k);
  }
  return pts;
}

export const BONE = [0x8a7e64, 0xd8ccb0, 0xf2eada];

/** A bone from a to b, `w` thick: an inked shaft, knobbly ends (two lumps across each), a lit streak. */
export function bone(g: Graphics, a: [number, number], b: [number, number], w: number, light: Light, ink = 0.05, tone = BONE[1]) {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const l = Math.hypot(dx, dy) || 1;
  const [nx, ny] = [-dy / l, dx / l];
  const knobs = (p: [number, number]) => {
    for (const s of [-1, 1]) g.circle(p[0] + nx * w * 0.45 * s, p[1] + ny * w * 0.45 * s, w * 0.62);
  };
  knobs(a);
  knobs(b);
  g.fill(tone).stroke({ width: Math.max(ink / 2, 0.02), color: INK });
  g.moveTo(...a).lineTo(...b).stroke({ width: w + ink, color: INK, cap: 'round' });
  knobs(a);
  knobs(b);
  g.fill(tone);
  g.moveTo(...a).lineTo(...b).stroke({ width: w, color: tone, cap: 'round' });
  const o = (nx * light[0] + ny * light[1] > 0 ? 1 : -1) * w * 0.22;
  g.moveTo(a[0] + nx * o + dx * 0.15, a[1] + ny * o + dy * 0.15).lineTo(b[0] + nx * o - dx * 0.15, b[1] + ny * o - dy * 0.15).stroke({ width: w * 0.3, color: BONE[2], cap: 'round' });
}

/** A skull from above at (x, y), crown toward `dir` (radians): the round cranium lit on one side, brow and
 * cheekbones narrowing to the jaw, two dark sockets and a nose hole toward the face end. */
export function skullTop(g: Graphics, x: number, y: number, dir: number, s: number, light: Light) {
  const [c, sn] = [Math.cos(dir), Math.sin(dir)];
  const p = (u: number, k: number): [number, number] => [x + c * u - sn * k, y + sn * u + c * k];
  const shape = [...p(s, 0), ...p(s * 0.75, s * 0.68), ...p(0, s * 0.82), ...p(-s * 0.6, s * 0.6), ...p(-s * 1.05, s * 0.32), ...p(-s * 1.1, -s * 0.32), ...p(-s * 0.6, -s * 0.6), ...p(0, -s * 0.82), ...p(s * 0.75, -s * 0.68)];
  g.poly(shape).fill(BONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(x + light[0] * s * 0.25 + c * s * 0.25, y + light[1] * s * 0.25 + sn * s * 0.25, s * 0.42).fill({ color: BONE[2], alpha: 0.9 });
  for (const k of [-1, 1]) g.poly(oval(...p(-s * 0.55, k * s * 0.34), s * 0.22, s * 0.2, dir, 10)).fill(0x1a1410);
  g.poly([...p(-s * 0.8, 0), ...p(-s * 0.95, s * 0.1), ...p(-s * 0.95, -s * 0.1)]).fill(0x1a1410);
}

/** A soft warm pool of light round (x, y). */
export function glowPool(g: Graphics, x: number, y: number, r: number, color = 0xf8b048) {
  for (let i = 0; i < 5; i++) g.circle(x, y, r * (1 - i * 0.16)).fill({ color, alpha: 0.06 });
}

/** A candle from straight above at (x, y), `r` across its top: a cream wax disc lit toward the light, and on it a
 * warm flame (orange, yellow, a pale core) at least 0.05 squares across whatever the candle's size, the brightest
 * thing on whatever holds it, in a warm halo. */
export function waxCandle(g: Graphics, x: number, y: number, r: number, light: Light) {
  g.circle(x, y, Math.max(0.07, r * 2.2)).fill({ color: 0xffc23a, alpha: 0.12 });
  g.circle(x, y, Math.max(0.05, r * 1.5)).fill({ color: 0xffd46a, alpha: 0.18 });
  g.circle(x, y, r).fill(0xe4d8bc).stroke({ width: 0.016, color: INK });
  g.circle(x + light[0] * r * 0.2, y + light[1] * r * 0.2, r * 0.72).fill(0xf6eedc);
  const rot = Math.atan2(light[1], light[0]);
  const [fx, fy] = [Math.max(0.026, r * 0.55), Math.max(0.033, r * 0.72)];
  g.poly(oval(x, y, fx, fy, rot, 12)).fill(0xf28a1e);
  g.poly(oval(x + light[0] * 0.004, y + light[1] * 0.004, fx * 0.7, fy * 0.7, rot, 12)).fill(0xffc23a);
  g.poly(oval(x + light[0] * 0.006, y + light[1] * 0.006, fx * 0.38, fy * 0.4, rot, 10)).fill(0xfff6d8);
}
