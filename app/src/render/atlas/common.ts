import { INK, FT } from '../pen';

export { INK, FT };

export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}


export function shade(color: number, f: number): number {
  const r = Math.min(255, Math.max(0, ((color >> 16) & 255) * f));
  const g = Math.min(255, Math.max(0, ((color >> 8) & 255) * f));
  const b = Math.min(255, Math.max(0, (color & 255) * f));
  return (Math.round(r) << 16) | (Math.round(g) << 8) | Math.round(b);
}

/** Lumpy blob outline: n points around radius r with jitter. */
export function blob(cx: number, cy: number, r: number, n: number, jitter: number, rnd: () => number): number[] {
  const pts: number[] = [];
  const phase = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = phase + (i / n) * Math.PI * 2;
    const rr = r * (1 - jitter / 2 + jitter * rnd());
    pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
  }
  return pts;
}

/** An ellipse as points, turned by `rot`. */
export function oval(cx: number, cy: number, rx: number, ry: number, rot: number, n = 16): number[] {
  const c = Math.cos(rot);
  const s = Math.sin(rot);
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2;
    const x = Math.cos(t) * rx;
    const y = Math.sin(t) * ry;
    pts.push(cx + x * c - y * s, cy + x * s + y * c);
  }
  return pts;
}

/** Atlas pixels per foot. */
/** Worked wood (barrels, carts, stalls, posts) and iron: [dark, mid, light, sunlit]. */
export const WOOD = [0x5c3b22, 0x8a5a32, 0xb98552, 0xe0b47a];
export const IRON = [0x34363c, 0x4a4e56, 0x6e7278, 0x9aa0a6];
