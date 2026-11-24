import type { Graphics } from 'pixi.js';

/** The life-size squash in force while `scaledAbout` draws (kx, ky, and the translation that maps drawing
 * coordinates to the world), or null outside one. Kept apart so `common` (rings, ovals) can read it without
 * importing `lifesize`. */
export type Squash = { kx: number; ky: number; tx: number; ty: number };
let cur: Squash | null = null;

export function setSquash(s: Squash | null): Squash | null {
  const old = cur;
  cur = s;
  return old;
}

/** The squash when it is uneven (the two factors differ), else null. */
export function uneven(): Squash | null {
  return cur && Math.abs(cur.kx - cur.ky) > 1e-3 ? cur : null;
}

/** The size a round thing keeps under an uneven squash: the mean of the two factors, so an ink pot or a jar
 * stays round and about life size while the body it sits on gets shallower. */
export function trueK(s: Squash): number {
  return Math.sqrt(s.kx * s.ky);
}

/** Radii (rx, ry) in drawing coordinates that come out as a circle of radius `r` after the squash. */
export function roundRadii(r: number): [number, number] {
  const s = uneven();
  if (!s) return [r, r];
  const t = trueK(s) * r;
  return [t / s.kx, t / s.ky];
}

/** Draws `fn` (a shape centred on (cx, cy) in drawing coordinates) at the mean life-size scale, uniformly,
 * about its squashed centre, so it stays true when the body under it is squashed. `k` overrides the scale (1 keeps
 * a weapon's true length when the body is only shallower). Outside an uneven squash it just calls `fn`. */
export function trueShape(g: Graphics, cx: number, cy: number, fn: (g: Graphics) => void, k?: number) {
  const s = uneven();
  if (!s) return fn(g);
  const u = k ?? trueK(s);
  const old = setSquash(null);
  g.save().setTransform(u, 0, 0, u, s.kx * cx + s.tx - u * cx, s.ky * cy + s.ty - u * cy);
  fn(g);
  g.restore();
  setSquash(old);
}
