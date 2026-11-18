import { Graphics } from 'pixi.js';
import { INK, blob, shade } from './common';

/** Stone tones per variant: [dark, mid, light, sunlit]. */
export const STONE = [
  [0x6e675c, 0x9a9182, 0xc4baa6, 0xe4dccb],
  [0x625f5a, 0x8c8a84, 0xb6b2a8, 0xd8d4c8],
  [0x6a6454, 0x958d76, 0xbeb498, 0xe0d6bc],
  [0x5e5a56, 0x86817a, 0xaea89c, 0xd2ccc0],
];

/** One stone from above, cut like a gem: a dark rim on the shaded side, the body, a lit top facet pushed
 * toward the NW with facet lines down to the body, a sunlit spot, a crack on the bigger ones. */
export function stone(g: Graphics, x: number, y: number, s: number, tones: readonly number[], rnd: () => number, n = 8) {
  const pts = blob(0, 0, s, n, 0.28, rnd);
  const at = (k: number, dx: number) => pts.map((p, i) => p * k + dx + (i % 2 ? y : x));
  const ink = Math.max(1.2, Math.min(2.5, s * 0.07));
  g.poly(at(1, 0)).fill(tones[0]).stroke({ width: ink, color: INK, join: 'round' });
  const body = at(0.86, -s * 0.06);
  g.poly(body).fill(tones[1]);
  const top = at(0.55, -s * 0.17);
  g.poly(top).fill(tones[2]);
  if (s >= 8) {
    for (let i = 0; i < pts.length; i += 4) g.moveTo(top[i], top[i + 1]).lineTo(body[i], body[i + 1]);
    g.stroke({ width: 1.2, color: tones[0], alpha: 0.7, cap: 'round' });
    g.poly(at(0.2, -s * 0.32)).fill({ color: tones[3], alpha: 0.85 });
  }
  if (s >= 20) {
    const a = rnd() * Math.PI * 2;
    let [cx, cy] = [x + Math.cos(a) * s * 0.85, y + Math.sin(a) * s * 0.85];
    g.moveTo(cx, cy);
    for (let k = 0; k < 3; k++) {
      cx += (x - cx) * 0.3 + (rnd() - 0.5) * s * 0.15;
      cy += (y - cy) * 0.3 + (rnd() - 0.5) * s * 0.15;
      g.lineTo(cx, cy);
    }
    g.stroke({ width: 1.5, color: shade(tones[0], 0.7), cap: 'round', join: 'round' });
  }
}

/** Moss: a soft cushion of small lumps, lighter toward the light, on a stone's north side. */
export function moss(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  const lumps: number[][] = [];
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = s * 0.5 * Math.sqrt(rnd());
    lumps.push([x + Math.cos(a) * d, y + Math.sin(a) * d, s * (0.38 + 0.15 * rnd())]);
  }
  for (const [lx, ly, lr] of lumps) g.circle(lx, ly, lr + 1).fill(0x3a5222);
  for (const [lx, ly, lr] of lumps) g.circle(lx, ly, lr).fill(0x5a7a32);
  for (const [lx, ly, lr] of lumps) g.circle(lx - lr * 0.25, ly - lr * 0.25, lr * 0.5).fill(0x7e9e44);
}

/** Boulder: one big faceted stone, a pebble or two at its foot, moss on some. */
export function boulderStone(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = STONE[v];
  for (let i = 0; i < 2; i++) {
    const a = Math.PI * 0.25 + (rnd() - 0.5) * 2;
    stone(g, Math.cos(a) * r * 0.9, Math.sin(a) * r * 0.9, r * 0.13, tones, rnd, 6);
  }
  stone(g, -r * 0.04, -r * 0.04, r * 0.76, tones, rnd, 9);
  if (v === 2 || v === 3) {
    moss(g, -r * 0.1, -r * 0.5, r * 0.22, rnd);
    moss(g, r * 0.25, -r * 0.42, r * 0.14, rnd);
  }
}

/** Rocks: three or four small stones. */
export function smallRocks(g: Graphics, r: number, rnd: () => number, v: number) {
  const n = 3 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    const d = i === 0 ? 0 : r * 0.55;
    stone(g, Math.cos(a) * d, Math.sin(a) * d, r * (i === 0 ? 0.45 : 0.28 + 0.1 * rnd()), STONE[v], rnd, 7);
  }
}

/** Rock pile: a loose, lopsided heap — stones of mixed sizes scattered over an uneven patch, the lower ones
 * drawn first and the ones resting on top last, no rings or rows. */
export function rockPile(g: Graphics, r: number, rnd: () => number, v: number) {
  const rot = rnd() * Math.PI;
  const [ax, ay] = [1, 0.7 + 0.2 * rnd()];
  const parts: [number, number, number, number][] = [];
  for (let tries = 0; parts.length < 11 && tries < 200; tries++) {
    const a = rnd() * Math.PI * 2;
    const d = Math.sqrt(rnd()) * r * 0.72;
    const u = Math.cos(a) * d * ax;
    const w = Math.sin(a) * d * ay;
    const x = u * Math.cos(rot) - w * Math.sin(rot);
    const y = u * Math.sin(rot) + w * Math.cos(rot);
    const s = r * (rnd() < 0.3 ? 0.3 + 0.08 * rnd() : 0.15 + 0.1 * rnd());
    // Keep stones from sitting squarely on one another.
    if (parts.some(([px, py, ps]) => Math.hypot(px - x, py - y) < (ps + s) * 0.55)) continue;
    if (Math.hypot(x, y) + s > r * 0.98) continue;
    // Height in the heap: bigger stones and those nearer the middle sit lower or higher at random.
    parts.push([x, y, s, rnd() + (s > r * 0.25 ? -0.3 : 0)]);
  }
  parts.sort((p, q) => p[3] - q[3]);
  for (const [x, y, s] of parts) stone(g, x, y, s, STONE[v], rnd, 7);
  if (v === 2) moss(g, parts[0][0], parts[0][1] - parts[0][2] * 0.5, r * 0.14, rnd);
}

/** Loose scree: a spread of flat, low-contrast stones and gravel. */
export function scree(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = STONE[v].map((c) => shade(c, 1.04));
  for (let i = 0; i < 40; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.9 * Math.sqrt(rnd());
    g.circle(Math.cos(a) * d, Math.sin(a) * d, 1.5 + rnd() * 1.5).fill({ color: tones[rnd() < 0.5 ? 0 : 2], alpha: 0.8 });
  }
  for (let i = 0; i < 22; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.85 * Math.sqrt(rnd());
    stone(g, Math.cos(a) * d, Math.sin(a) * d, r * (0.07 + 0.07 * rnd()), tones, rnd, 6);
  }
}

/** Bark tones per variant: [dark, mid, light]. */
export const BARK = [
  [0x4e3424, 0x7d5a3c, 0xa8825a],
  [0x4a3a2c, 0x76604a, 0xa08a6c],
  [0x523628, 0x84603e, 0xb08a5e],
  [0x463830, 0x6e6052, 0x9a8c78],
];
