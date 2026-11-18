import { type Pt } from '../roomShapes';
import { Graphics } from 'pixi.js';
import { INK, hash2 } from './look';

// A yurt's (`roomShapes.yurtRing`): rugs on its felt floor, its lattice wall, its painted door.
export const MADDER = 0x8e4c38;
/** R5 towns (114): the round felt's colour by the room, so a felt temple, hall and inn tell apart:
 * a sanctuary's sky blue, a hall's moss green, the rest madder. */
export const ROOM_FELT: Record<string, number> = { 'felt sanctuary': 0x46597a, 'felt hall': 0x5f6338 };
const RUG_OCHRE = 0xb89048;
const PAINT = 0x985a44;
const WILLOW = 0xa0805a;
/** A yurt's wall's width (squares; a built outer wall is 0.45). */
export const KHANA = 0.36;
/** A circle's points round (cx, cy), a little uneven (the same for the same seed). */
export function hand(cx: number, cy: number, r: number, seed: number, n = 40): number[] {
  const out: number[] = [];
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2;
    const rr = r * (1 + 0.012 * (hash2(seed, k % (n - 1), 5) - 0.5));
    out.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
  }
  return out;
}
/** A rect `l` long along unit `u` and `w` wide, about (cx, cy). */
function band(cx: number, cy: number, u: Pt, l: number, w: number): number[] {
  const [vx, vy] = [-u[1], u[0]];
  const [hl, hw] = [l / 2, w / 2];
  return [cx - u[0] * hl - vx * hw, cy - u[1] * hl - vy * hw, cx + u[0] * hl - vx * hw, cy + u[1] * hl - vy * hw, cx + u[0] * hl + vx * hw, cy + u[1] * hl + vy * hw, cx - u[0] * hl + vx * hw, cy - u[1] * hl + vy * hw];
}
/** A yurt's rugs, under the grid: a felt rug laid across from the door under the chests, the big round
 * felt rug round the hearth (madder, an ochre border, a ring of lozenges; bare earth under the fire),
 * and a dark felt runner in from the door over its edge. */
export function yurtRugs(g: Graphics, ring: { c: Pt; r: number; south: Pt }, door: { a: Pt; b: Pt } | null, felt = MADDER) {
  const [cx, cy] = ring.c;
  const seed = Math.round(cx * 97 + cy * 31);
  let dd: Pt = ring.south;
  if (door) {
    const [mx, my] = [(door.a[0] + door.b[0]) / 2 - cx, (door.a[1] + door.b[1]) / 2 - cy];
    const l = Math.hypot(mx, my);
    if (l > 0.1) dd = [mx / l, my / l];
  }
  const t: Pt = [-dd[1], dd[0]];
  const ro = Math.max(0.9, ring.r - 0.6);
  const ri = 0.5;
  // Across from the door: a walnut-brown felt with an ochre stripe.
  const bd = ring.r - 0.75;
  const back = band(cx - dd[0] * bd, cy - dd[1] * bd, t, Math.min(1.5, ring.r), 0.7);
  g.poly(back).fill(0x6a4a34).stroke({ width: LINE_W, color: INK, join: 'round' });
  g.poly(band(cx - dd[0] * bd, cy - dd[1] * bd, t, Math.min(1.5, ring.r) - 0.16, 0.54)).stroke({ width: 0.02, color: RUG_OCHRE });
  // The round felt.
  g.poly(hand(cx, cy, ro, seed)).fill(RUG_OCHRE).stroke({ width: LINE_W, color: INK, join: 'round' });
  g.poly(hand(cx, cy, ro - 0.09, seed + 1)).fill(felt);
  g.poly(hand(cx, cy, ro - 0.16, seed + 2)).stroke({ width: 0.018, color: 0x5a3424 });
  const rm = (ro - 0.16 + ri) / 2;
  const n = Math.max(6, Math.round((Math.PI * 2 * rm) / 0.42));
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2;
    const [px, py] = [cx + Math.cos(a) * rm, cy + Math.sin(a) * rm];
    const [ux, uy] = [Math.cos(a), Math.sin(a)];
    const [s, w] = [0.11, 0.07];
    g.poly([px - ux * s, py - uy * s, px - uy * w, py + ux * w, px + ux * s, py + uy * s, px + uy * w, py - ux * w]).fill(RUG_OCHRE);
    g.circle(px, py, 0.022).fill(0x5a3424);
  }
  g.poly(hand(cx, cy, ri, seed + 3, 24)).fill(0x8f7a5e).stroke({ width: LINE_W, color: INK, join: 'round' });
  // The runner in from the door.
  const r0 = ring.r - 0.3;
  const r1 = ro - 0.25;
  const rl = r0 - r1;
  if (rl > 0.2) {
    const m = (r0 + r1) / 2;
    g.poly(band(cx + dd[0] * m, cy + dd[1] * m, dd, rl, 0.72)).fill(0x56706a).stroke({ width: LINE_W, color: INK, join: 'round' });
    g.poly(band(cx + dd[0] * m, cy + dd[1] * m, dd, rl - 0.14, 0.56)).stroke({ width: 0.02, color: RUG_OCHRE });
  }
}
const LINE_W = 0.025;
/** A yurt's wall as a lattice (khana) on a true circle about the ring's middle, the ring's radius
 * (its footprint's corners): a band of the wall's ink, a band of willow inside it, crossed by its
 * laths, even all round but where its doors out go through. `doors`: the doors through it as drawn. */
export function khana(g: Graphics, ring: { c: Pt; r: number }, doors: { a: Pt; b: Pt; outer?: Pt }[]) {
  const [[cx, cy], r] = [ring.c, ring.r];
  const TAU = Math.PI * 2;
  // (Each door's gap: its angle about the middle, half as wide as its leaves.)
  const gaps = doors.map((d) => {
    const m = d.outer ?? [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
    return { at: Math.atan2(m[1] - cy, m[0] - cx), half: Math.asin(Math.min(0.95, Math.min(Math.hypot(d.b[0] - d.a[0], d.b[1] - d.a[1]), 0.95)) / 2 / r) };
  });
  // Runs of the circle between the gaps (from one gap's end round to the next's start).
  const runs: [number, number][] = [];
  if (!gaps.length) runs.push([0, TAU]);
  else {
    const g2 = gaps.map((q) => [q.at - q.half, q.at + q.half]).sort((x, y) => x[0] - y[0]);
    for (let k = 0; k < g2.length; k++) {
      const end = k + 1 < g2.length ? g2[k + 1][0] : g2[0][0] + TAU;
      if (end - g2[k][1] > 0.05) runs.push([g2[k][1], end]);
    }
  }
  const at = (t: number, rr: number): [number, number] => [cx + Math.cos(t) * rr, cy + Math.sin(t) * rr];
  const h = KHANA / 2 - 0.05;
  const w = h - 0.03;
  for (const [t0, t1] of runs) {
    const closed = t1 - t0 >= TAU - 1e-9;
    // (Points about 0.2 square apart round the run, evenly.)
    const n = Math.max(2, Math.ceil(((t1 - t0) * r) / 0.2));
    const ts = Array.from({ length: n + 1 }, (_, k) => t0 + ((t1 - t0) * k) / n);
    const arc = (rr: number) => ts.flatMap((t) => at(t, rr));
    const bandOf = (lo: number, hi: number) => (closed ? null : [...arc(hi), ...ts.slice().reverse().flatMap((t) => at(t, lo))]);
    // The wall's ink, then the willow inside it.
    for (const [lo, hi, color] of [
      [r - KHANA / 2, r + KHANA / 2, INK],
      [r - h, r + h, WILLOW],
    ] as const) {
      const b = bandOf(lo, hi);
      if (b) g.poly(b).fill(color);
      else g.poly(arc(hi)).fill(color).poly(arc(lo)).cut();
    }
    // The laths crossing it, pinned where they cross.
    for (let k = 0; k < n; k++) {
      const [p0, p1, q0, q1] = [at(ts[k], r - w), at(ts[k], r + w), at(ts[k + 1], r - w), at(ts[k + 1], r + w)];
      g.moveTo(p0[0], p0[1]).lineTo(q1[0], q1[1]);
      g.moveTo(p1[0], p1[1]).lineTo(q0[0], q0[1]);
    }
    g.stroke({ width: 0.022, color: 0x5e4028, cap: 'round' });
  }
}
/** A yurt's door through its wall: a low double door painted red ochre with an ochre border and a
 * lozenge on each leaf, between wooden posts; open, its leaves swung in. */
export function yurtDoor(g: Graphics, d: { a: Pt | number[]; b: Pt | number[] }, open: boolean, ring: { c: Pt; r: number }) {
  const c = ring.c;
  const len0 = Math.hypot(d.b[0] - d.a[0], d.b[1] - d.a[1]) || 1;
  const u: Pt = [(d.b[0] - d.a[0]) / len0, (d.b[1] - d.a[1]) / len0];
  // (On the lattice's circle, whatever side of it the doorway was found on.)
  const [ox, oy] = [(d.a[0] + d.b[0]) / 2 - c[0], (d.a[1] + d.b[1]) / 2 - c[1]];
  const k = ring.r / (Math.hypot(ox, oy) || 1);
  const [mx, my] = [c[0] + ox * k, c[1] + oy * k];
  const len = Math.min(len0, 0.95);
  const [ax, ay, bx, by] = [mx - (u[0] * len) / 2, my - (u[1] * len) / 2, mx + (u[0] * len) / 2, my + (u[1] * len) / 2];
  // (In: toward the ring's middle.)
  const side = (c[0] - mx) * -u[1] + (c[1] - my) * u[0] >= 0 ? 1 : -1;
  const n: Pt = [-u[1] * side, u[0] * side];
  for (const [px, py] of [
    [ax, ay],
    [bx, by],
  ]) {
    g.poly(band(px, py, u, 0.12, KHANA + 0.04)).fill(0x6e4a2c).stroke({ width: LINE_W, color: INK, join: 'round' });
  }
  const leaf = (len - 0.12) / 2;
  const paint = (cx: number, cy: number, along: Pt, l: number) => {
    g.poly(band(cx, cy, along, l, 0.16)).fill(PAINT).stroke({ width: LINE_W, color: INK, join: 'round' });
    g.poly(band(cx, cy, along, l - 0.08, 0.08)).stroke({ width: 0.016, color: RUG_OCHRE });
    const [s, w] = [0.06, 0.04];
    g.poly([cx - along[0] * s, cy - along[1] * s, cx - along[1] * w, cy + along[0] * w, cx + along[0] * s, cy + along[1] * s, cx + along[1] * w, cy - along[0] * w]).fill(RUG_OCHRE);
  };
  if (!open) {
    for (const k of [-1, 1]) paint(mx + u[0] * k * leaf * 0.5, my + u[1] * k * leaf * 0.5, u, leaf);
    return;
  }
  for (const [px, py] of [
    [ax + u[0] * 0.06, ay + u[1] * 0.06],
    [bx - u[0] * 0.06, by - u[1] * 0.06],
  ]) {
    paint(px + n[0] * leaf * 0.5, py + n[1] * leaf * 0.5, n, leaf);
  }
}
