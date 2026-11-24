import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { INK, IRON, LINE, type Light, OUT, WOOD, board, castShadow, frame, glowPool, oval, patch, ring, shade, waxCandle } from './common';
import { MARBLE } from './sacred';
import { GILT } from './tables';
import { STONE, masonry } from './home';

/** A fire seen from above at (x, y), about `r` across: an ember bed, then soft glowing ovals heaped on it, deep
 * orange out to a pale yellow core (as in the hearths), a few sparks. */
function fireTop(g: Graphics, x: number, y: number, r: number, rnd: () => number) {
  for (let i = 0; i < 12; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * Math.sqrt(rnd());
    g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, r * (0.1 + 0.08 * rnd())).fill([0x7a2a10, 0xc2410c, 0xf97316][Math.floor(rnd() * 3)]);
  }
  const blobs: [number, number, number][] = [];
  for (let i = 0; i < 3; i++) {
    const a = (i / 3) * Math.PI * 2 + rnd();
    blobs.push([x + Math.cos(a) * r * 0.3, y + Math.sin(a) * r * 0.3, r * (0.5 + 0.15 * rnd())]);
  }
  for (const [k, color] of [
    [1, 0xc2410c],
    [0.74, 0xf28a1e],
    [0.48, 0xffc23a],
    [0.22, 0xfff2c0],
  ] as const) {
    for (const [bx, by, br] of blobs) g.poly(oval(bx, by, br * k, br * k * 0.85, 0.4, 14)).fill(color);
  }
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * (1.1 + 0.5 * rnd());
    g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, 0.01).fill(0xffd27a);
  }
}

/** Brazier: a wide iron bowl on three clawed legs, a heaped fire of glowing coals in it, a warm glow round it. */
export function brazier(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  glowPool(g, cx, cy, 0.62);
  const r = 0.27;
  castShadow(g, ring(cx, cy, r, rnd, 0.005, 16), light, 0.45);
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < 3; i++) {
    const a = ph + (i * Math.PI * 2) / 3;
    const [fx, fy] = [cx + Math.cos(a) * 0.4, cy + Math.sin(a) * 0.4];
    for (const [w, c] of [
      [0.06, INK],
      [0.035, IRON[1]],
    ] as const) {
      g.moveTo(cx + Math.cos(a) * 0.2, cy + Math.sin(a) * 0.2).lineTo(fx, fy).stroke({ width: w, color: c, cap: 'round' });
    }
    for (const b of [-0.5, 0, 0.5]) g.moveTo(fx, fy).lineTo(fx + Math.cos(a + b) * 0.05, fy + Math.sin(a + b) * 0.05);
    g.stroke({ width: 0.018, color: IRON[0], cap: 'round' });
  }
  g.circle(cx, cy, r).fill(IRON[1]).stroke({ width: 0.035, color: INK });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la + 2.2) * r * 0.88, cy + Math.sin(la + 2.2) * r * 0.88).arc(cx, cy, r * 0.88, la + 2.2, la + 4.1).stroke({ width: 0.04, color: IRON[0] });
  g.moveTo(cx + Math.cos(la - 1) * r * 0.9, cy + Math.sin(la - 1) * r * 0.9).arc(cx, cy, r * 0.9, la - 1, la + 1).stroke({ width: 0.025, color: IRON[2] });
  g.circle(cx, cy, r * 0.78).fill(0x3a2418).stroke({ width: 0.018, color: INK });
  g.circle(cx, cy, r * 0.78).stroke({ width: 0.03, color: INK });
  fireTop(g, cx, cy, r * 0.46, rnd);
}

/** Candelabrum from straight above: an iron drip-ring about 2 ft across on its branches (the stand hidden under it),
 * the column's knob in the middle, five candles of mixed thickness round the ring as cream wax discs each with a tiny
 * flame, wax dripped on the ring and the floor, one soft warm pool of light. */
export function candelabrum(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  glowPool(g, cx, cy, 0.48, 0xfcd36a);
  castShadow(g, ring(cx, cy, 0.19, rnd, 0.004, 14), light, 0.22);
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.24 + 0.1 * rnd();
    g.poly(oval(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.024, 0.017, a, 8)).fill(0xe8dcc0).stroke({ width: 0.016, color: INK });
  }
  // The ring: an iron band, the branches under it showing as short spokes to the knob.
  const ph = rnd() * Math.PI * 2;
  const cups: [number, number, number][] = [];
  for (let i = 0; i < 5; i++) {
    const a = ph + (i / 5) * Math.PI * 2 + (rnd() - 0.5) * 0.35;
    const d = 0.165 + (rnd() - 0.5) * 0.02;
    cups.push([cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.03 + 0.012 * rnd()]);
  }
  for (const [x, y] of cups.slice(0, 3)) {
    g.moveTo(cx, cy).lineTo(x, y).stroke({ width: 0.05, color: INK, cap: 'round' });
    g.moveTo(cx, cy).lineTo(x, y).stroke({ width: 0.026, color: IRON[1], cap: 'round' });
  }
  for (const [w, c] of [
    [0.07, INK],
    [0.042, IRON[1]],
  ] as const)
    g.circle(cx, cy, 0.165).stroke({ width: w, color: c });
  g.moveTo(cx + Math.cos(Math.atan2(ly, lx) - 1) * 0.165, cy + Math.sin(Math.atan2(ly, lx) - 1) * 0.165)
    .arc(cx, cy, 0.165, Math.atan2(ly, lx) - 1, Math.atan2(ly, lx) + 1)
    .stroke({ width: 0.016, color: IRON[2] });
  g.circle(cx, cy, 0.045).fill(IRON[1]).stroke({ width: 0.016, color: INK });
  g.circle(cx + lx * 0.012, cy + ly * 0.012, 0.022).fill(IRON[2]);
  // The cups and their candles.
  for (const [x, y, r] of cups) {
    g.circle(x, y, r + 0.02).fill(IRON[0]).stroke({ width: 0.016, color: INK });
    g.poly(patch(x + 0.01, y + 0.012, r * 0.9, r * 0.7, rnd, 0.3, 8)).fill({ color: 0xe8dcc0, alpha: 0.8 });
    waxCandle(g, x, y, r, light);
  }
}

/** Wall shackles: two bolted iron plates on the wall, each with a ring, a heavy chain hanging from it and lying on
 * the floor, ending in a thick open cuff with its hinge. */
export function shackles(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  for (const u of [0.26, 0.74]) {
    // The plate and ring.
    const plate = [...at(u - 0.08, 0), ...at(u + 0.08, 0), ...at(u + 0.08, 0.1), ...at(u - 0.08, 0.1)];
    g.poly(plate).fill(IRON[0]).stroke({ width: 0.018, color: INK, join: 'round' });
    for (const k of [-0.05, 0.05]) g.circle(...at(u + k, 0.03), 0.012).fill(IRON[2]);
    const [rx, ry] = at(u, 0.12);
    g.circle(rx, ry, 0.045).stroke({ width: 0.045, color: INK });
    g.circle(rx, ry, 0.045).stroke({ width: 0.024, color: IRON[1] });
    // The chain.
    const end = at(u + (u < 0.5 ? -0.06 : 0.06) + (rnd() - 0.5) * 0.05, 0.66 + 0.08 * rnd());
    const mid = at(u + (u < 0.5 ? 0.07 : -0.07), 0.42);
    const B = (t: number): [number, number] => [(1 - t) * (1 - t) * rx + 2 * (1 - t) * t * mid[0] + t * t * end[0], (1 - t) * (1 - t) * ry + 2 * (1 - t) * t * mid[1] + t * t * end[1]];
    const n = 7;
    castShadow(g, [...B(0), ...B(0.5), ...B(1)], light, 0.03);
    for (let i = 1; i <= n; i++) {
      const [p0, p1] = [B(i / n), B((i - 1) / n)];
      const a = Math.atan2(p0[1] - p1[1], p0[0] - p1[0]);
      const o = oval((p0[0] + p1[0]) / 2, (p0[1] + p1[1]) / 2, 0.05, i % 2 ? 0.03 : 0.012, a, 12);
      g.poly(o).stroke({ width: 0.036, color: INK });
      g.poly(o).stroke({ width: 0.018, color: IRON[1] });
      g.poly(o.map((q, k) => q + (k % 2 ? light[1] : light[0]) * 0.006)).stroke({ width: 0.014, color: IRON[2] });
    }
    // The cuff: a thick band, open, its hinge knob.
    const [ex, ey] = end;
    const a0 = Math.atan2(ey - mid[1], ex - mid[0]);
    for (const [w, c] of [
      [0.06, INK],
      [0.036, IRON[1]],
    ] as const) {
      g.moveTo(ex + Math.cos(a0 + 0.5) * 0.085, ey + Math.sin(a0 + 0.5) * 0.085).arc(ex, ey, 0.085, a0 + 0.5, a0 + 0.5 + Math.PI * 1.55).stroke({ width: w, color: c, cap: 'round' });
    }
    g.moveTo(ex + Math.cos(a0 + 1.6) * 0.085, ey + Math.sin(a0 + 1.6) * 0.085).arc(ex, ey, 0.085, a0 + 1.6, a0 + 2.6).stroke({ width: 0.014, color: IRON[2] });
    const [kx, ky] = [ex + Math.cos(a0 + Math.PI) * 0.085, ey + Math.sin(a0 + Math.PI) * 0.085];
    g.circle(kx, ky, 0.025).fill(IRON[0]).stroke({ width: 0.018, color: INK });
  }
}

/** Torch sconce: an iron bracket on the wall holding a torch angled out into the room, its head burning (glowing
 * ovals), a pool of light on the floor. */
export function torchSconce(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  void light;
  const [hx, hy] = at(0.5, 0.42);
  glowPool(g, hx, hy, 0.62);
  g.poly([...at(0.3, 0), ...at(0.7, 0), ...at(0.64, 0.12), ...at(0.36, 0.12)]).fill(IRON[0]).stroke({ width: 0.024, color: INK, join: 'round' });
  for (const [w, c] of [
    [0.09, INK],
    [0.056, IRON[1]],
  ] as const) {
    g.moveTo(...at(0.5, 0.08)).lineTo(...at(0.5, 0.24)).stroke({ width: w, color: c, cap: 'round' });
  }
  g.poly(oval(...at(0.5, 0.26), 0.13, 0.1, 0, 12)).stroke({ width: 0.05, color: INK });
  g.poly(oval(...at(0.5, 0.26), 0.13, 0.1, 0, 12)).stroke({ width: 0.026, color: IRON[1] });
  for (const [w, c] of [
    [0.09, INK],
    [0.06, WOOD[1]],
  ] as const) {
    g.moveTo(...at(0.5, 0.2)).lineTo(...at(0.5, 0.42)).stroke({ width: w, color: c, cap: 'round' });
  }
  g.circle(hx, hy, 0.09).fill(0x3a2a1c).stroke({ width: 0.02, color: INK });
  fireTop(g, hx, hy, 0.065, rnd);
}

/** Iron maiden from straight above, about 2.5 by 2 ft: its coffin-shaped iron lid (a rounded bulge for the head at
 * the top end, broad at the shoulders, narrowing to the foot), split down the middle where the two doors meet; the
 * left door shut, hung on two hinge straps along its outer edge, the right door just ajar so a dark wedge opens
 * along the seam with the pale tips of its spikes in it; rivets along the edge and two bands, a small blood stain
 * at the foot. */
export function ironMaiden(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const BLOOD = 0x5a1410;
  const [lx, ly] = light;
  // T(a, b): a across, b from the head (-) to the foot (+).
  const T = (a: number, b: number): [number, number] => [cx + a, cy + b];
  const lid: [number, number][] = [
    [0, -0.29], [0.065, -0.275], [0.095, -0.22], [0.09, -0.175], [0.18, -0.15], [0.21, -0.09], [0.2, 0.02],
    [0.16, 0.2], [0.125, 0.265], [0.0, 0.272], [-0.125, 0.265], [-0.16, 0.2], [-0.2, 0.02], [-0.21, -0.09], [-0.18, -0.15], [-0.09, -0.175], [-0.095, -0.22], [-0.065, -0.275],
  ];
  const pts = lid.flatMap(([a, b]) => T(a, b));
  castShadow(g, pts, light, 0.3);
  g.poly(patch(...T(0.05, 0.33), 0.08, 0.045, rnd, 0.35, 10)).fill({ color: BLOOD, alpha: 0.8 });
  g.circle(...T(0.12, 0.31), 0.016).fill({ color: BLOOD, alpha: 0.8 });
  g.poly(pts).fill(IRON[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(lid.flatMap(([a, b]) => T(a * 0.86 + lx * 0.014, b * 0.88 + ly * 0.014))).fill(IRON[1]);
  // The head's dome, lit toward the light.
  g.poly(oval(...T(0, -0.215), 0.07, 0.06, 0, 14)).fill(IRON[1]).stroke({ width: 0.018, color: shade(IRON[0], 0.8) });
  g.poly(oval(...T(lx * 0.02, -0.215 + ly * 0.02), 0.035, 0.028, 0, 10)).fill(IRON[2]);
  // Two bands across, riveted.
  for (const b of [-0.1, 0.12]) {
    const w = b < 0 ? 0.2 : 0.175;
    g.moveTo(...T(-w, b)).lineTo(...T(w, b)).stroke({ width: 0.03, color: INK });
    g.moveTo(...T(-w + 0.01, b - 0.004)).lineTo(...T(w - 0.01, b - 0.004)).stroke({ width: 0.014, color: IRON[2] });
  }
  for (const [a, b] of [[-0.17, -0.04], [-0.165, 0.06], [-0.14, 0.2], [0.17, -0.04], [0.165, 0.06], [0.14, 0.2], [-0.15, -0.13], [0.15, -0.13]] as const) {
    g.circle(...T(a, b), 0.014).fill(IRON[2]).stroke({ width: 0.016, color: INK });
  }
  // Hinge straps on the shut (left) door's outer edge, each with its knuckle.
  for (const b of [-0.05, 0.16]) {
    const w = b < 0 ? -0.205 : -0.175;
    g.poly([...T(w, b - 0.025), ...T(-0.06, b - 0.018), ...T(-0.05, b), ...T(-0.06, b + 0.018), ...T(w, b + 0.025)]).fill(IRON[0]).stroke({ width: 0.018, color: INK, join: 'round' });
    g.moveTo(...T(w + 0.01, b - 0.01)).lineTo(...T(-0.07, b - 0.008)).stroke({ width: 0.014, color: IRON[2] });
    g.poly(oval(...T(w - 0.012, b), 0.022, 0.034, 0, 10)).fill(IRON[1]).stroke({ width: 0.018, color: INK });
  }
  // The seam, and the right door ajar: a dark wedge opening toward the foot, spike tips in it.
  const gap = [...T(-0.004, -0.17), ...T(0.016, -0.17), ...T(0.07, 0.262), ...T(-0.004, 0.268)];
  g.poly(gap).fill(0x120c0a).stroke({ width: 0.018, color: INK, join: 'round' });
  g.moveTo(...T(0.018, -0.168)).lineTo(...T(0.074, 0.258)).stroke({ width: 0.016, color: IRON[2] });
  for (let i = 0; i < 6; i++) {
    const t = (i + 0.5) / 6;
    const b = -0.15 + t * 0.4;
    const a = 0.004 + t * 0.03 * (0.8 + 0.3 * rnd());
    g.poly([...T(a - 0.01, b - 0.012), ...T(a + 0.014 + t * 0.012, b), ...T(a - 0.01, b + 0.012)]).fill(0xe0dcd0);
  }
  g.circle(...T(0.03, 0.2), 0.012).fill({ color: BLOOD, alpha: 0.9 });
}

/** Stocks from straight above, about 5 ft by 0.8 ft: the heavy board's top edge as a thick plank split along its length, scalloped notches
 * where the head and wrist holes are cut through the seam, a half-foot square post top at each end, an iron hinge strap at one
 * end and a padlock at the other, straw on the floor before it. */
export function stocks(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const along = f.w >= f.h;
  const L = 0.5;
  const W = 0.08;
  // P(a, b): a along the board from its middle, b across toward the front.
  const P = (a: number, b: number): [number, number] => (along ? [cx + a, cy + b] : [cx + b, cy + a]);
  for (let i = 0; i < 16; i++) {
    const [x, y] = P((rnd() - 0.5) * L * 1.7, 0.12 + rnd() * 0.2);
    g.moveTo(x, y).lineTo(x + (rnd() - 0.5) * 0.08, y + (rnd() - 0.5) * 0.05).stroke({ width: 0.014, color: 0xc8a860 });
  }
  const plank = [...P(-L, -W), ...P(L, -W), ...P(L, W), ...P(-L, W)];
  castShadow(g, plank, light, 0.16);
  // The two boards meeting at the seam, each a little uneven.
  for (const [b0, b1, k] of [
    [-W, 0, 0.92],
    [0, W, 1],
  ] as const) {
    const r = [...P(-L, b0 + (b0 < 0 ? 0.005 : 0)), ...P(L, b0), ...P(L, b1), ...P(-L, b1 - (b1 > 0 ? 0.005 : 0))];
    g.poly(r).fill(shade(WOOD[1], k)).stroke({ width: LINE, color: INK, join: 'round' });
  }
  g.poly([...P(-L, -W + 0.005), ...P(L, -W), ...P(L, W), ...P(-L, W - 0.005)]).stroke({ width: OUT, color: INK, join: 'round' });
  const litB = (along ? light[1] : light[0]) < 0 ? -1 : 1;
  g.poly([...P(-L + 0.03, litB * 0.058), ...P(L - 0.03, litB * 0.058), ...P(L - 0.03, litB * 0.04), ...P(-L + 0.03, litB * 0.04)]).fill(WOOD[2]);
  // The grain along the boards.
  for (const b of [-0.045, 0.045]) g.moveTo(...P(-L + 0.06, b)).lineTo(...P(-0.12, b + 0.004)).moveTo(...P(0.12, b)).lineTo(...P(L - 0.06, b - 0.004));
  g.stroke({ width: 0.014, color: WOOD[0], alpha: 0.7 });
  // The holes through the seam: a big one for the neck between two for the wrists.
  for (const [a, r] of [
    [0, 0.085],
    [-0.27, 0.05],
    [0.27, 0.05],
  ]) {
    const hole: number[] = [];
    for (let i = 0; i <= 12; i++) {
      const t = (i / 12) * Math.PI * 2;
      hole.push(...P(a + Math.cos(t) * r, Math.sin(t) * Math.min(0.058, r * 0.72)));
    }
    g.poly(hole).fill(0x0e0a08).stroke({ width: LINE, color: INK });
    // Its far rim lit where the light falls into it.
    g.moveTo(...P(a - r * 0.6, -litB * Math.min(0.05, r * 0.6))).lineTo(...P(a + r * 0.6, -litB * Math.min(0.05, r * 0.6))).stroke({ width: 0.014, color: WOOD[2] });
  }
  // Posts at the ends, a hinge strap and a padlock.
  for (const s0 of [-1, 1]) {
    const a = s0 * (L + 0.045);
    const post = [...P(a - 0.05, -0.05), ...P(a + 0.05, -0.05), ...P(a + 0.05, 0.05), ...P(a - 0.05, 0.05)];
    g.poly(post).fill(WOOD[0]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly([...P(a - 0.032, -0.032), ...P(a + 0.032, -0.032), ...P(a + 0.032, 0.03), ...P(a - 0.032, 0.03)]).fill(shade(WOOD[1], 0.95));
    g.moveTo(...P(a - 0.02, 0)).lineTo(...P(a + 0.02, 0.006)).stroke({ width: 0.014, color: WOOD[0] });
  }
  g.poly([...P(-L + 0.08, -W), ...P(-L + 0.15, -W), ...P(-L + 0.15, W), ...P(-L + 0.08, W)]).fill(IRON[1]).stroke({ width: 0.018, color: INK });
  g.poly([...P(L - 0.14, 0.05), ...P(L - 0.07, 0.05), ...P(L - 0.07, 0.14), ...P(L - 0.14, 0.14)]).fill(IRON[1]).stroke({ width: 0.018, color: INK });
  g.circle(...P(L - 0.105, 0.1), 0.012).fill(INK);
}

/** Treasure hoard: a heap of gold coins spilling over the floor, a goblet and a crown on it, gems (ruby, sapphire,
 * emerald) and a sword's hilt sticking out, glints. */
export function treasureHoard(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  castShadow(g, patch(cx, cy, 0.3, 0.28, rnd, 0.2, 12), light, 0.2);
  const coin = (x: number, y: number, r: number) => {
    g.circle(x, y, r).fill(GILT[0]).stroke({ width: 0.018, color: INK });
    g.circle(x + light[0] * r * 0.15, y + light[1] * r * 0.15, r * 0.75).fill(GILT[1]);
    g.circle(x, y, r * 0.45).stroke({ width: 0.014, color: GILT[0] });
  };
  // The heap: a mound under the coins, loose coins round it, then coins piling toward the top.
  g.poly(patch(cx, cy, 0.33, 0.3, rnd, 0.2, 18)).fill(GILT[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + light[0] * 0.04, cy + light[1] * 0.04, 0.25, 0.22, rnd, 0.2, 14)).fill(GILT[1]);
  for (let i = 0; i < 12; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.34 + 0.1 * rnd();
    coin(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.028);
  }
  for (let i = 0; i < 26; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.28 * Math.sqrt(rnd());
    coin(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.032);
  }
  // A sword hilt sticking out.
  const sa = rnd() * Math.PI * 2;
  const [hx, hy] = [cx + Math.cos(sa) * 0.12, cy + Math.sin(sa) * 0.12];
  for (const [w, c] of [
    [0.045, INK],
    [0.025, 0x5a3a20],
  ] as const) {
    g.moveTo(hx, hy).lineTo(hx + Math.cos(sa) * 0.16, hy + Math.sin(sa) * 0.16).stroke({ width: w, color: c, cap: 'round' });
  }
  g.moveTo(hx - Math.sin(sa) * 0.08, hy + Math.cos(sa) * 0.08).lineTo(hx + Math.sin(sa) * 0.08, hy - Math.cos(sa) * 0.08).stroke({ width: 0.035, color: INK, cap: 'round' });
  g.moveTo(hx - Math.sin(sa) * 0.08, hy + Math.cos(sa) * 0.08).lineTo(hx + Math.sin(sa) * 0.08, hy - Math.cos(sa) * 0.08).stroke({ width: 0.018, color: GILT[2], cap: 'round' });
  g.circle(hx + Math.cos(sa) * 0.17, hy + Math.sin(sa) * 0.17, 0.025).fill(GILT[1]).stroke({ width: 0.018, color: INK });
  // A goblet (from above: a cup on its foot) and a crown.
  const [gx, gy] = [cx - 0.12, cy + 0.1];
  g.circle(gx, gy, 0.07).fill(GILT[1]).stroke({ width: 0.018, color: INK });
  g.circle(gx, gy, 0.05).fill(0x6a1e1a);
  const [kx, ky] = [cx + 0.1, cy - 0.12];
  g.circle(kx, ky, 0.08).stroke({ width: 0.04, color: INK });
  g.circle(kx, ky, 0.08).stroke({ width: 0.024, color: GILT[2] });
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    g.poly([kx + Math.cos(a - 0.15) * 0.09, ky + Math.sin(a - 0.15) * 0.09, kx + Math.cos(a) * 0.13, ky + Math.sin(a) * 0.13, kx + Math.cos(a + 0.15) * 0.09, ky + Math.sin(a + 0.15) * 0.09]).fill(GILT[2]).stroke({ width: 0.018, color: INK });
  }
  g.circle(kx, ky - 0.08, 0.016).fill(0xc02030);
  // Gems and glints.
  for (const col of [0xc02030, 0x2050c0, 0x20a060, 0xc02030, 0x8040c0]) {
    const [x, y] = [cx + (rnd() - 0.5) * 0.5, cy + (rnd() - 0.5) * 0.45];
    g.poly([x, y - 0.03, x + 0.025, y, x, y + 0.03, x - 0.025, y]).fill(col).stroke({ width: 0.018, color: INK });
    g.circle(x - 0.006, y - 0.01, 0.007).fill(0xffffff);
  }
  for (let i = 0; i < 5; i++) {
    const [x, y] = [cx + (rnd() - 0.5) * 0.5, cy + (rnd() - 0.5) * 0.5];
    g.poly([x, y - 0.035, x + 0.008, y, x, y + 0.035, x - 0.008, y]).fill(0xffffff);
    g.poly([x - 0.035, y, x, y - 0.008, x + 0.035, y, x, y + 0.008]).fill(0xffffff);
  }
}

/** Dais: the top dressing of a platform raised two steps, the upper step set back from the front. The lower step is
 * the raised floor itself (a 2.5-ft dais in the floor heights), its edge, riser and shadow drawn with every other step
 * and ledge by the underground shader (`ledges`); this draws the upper step. What stands on it (a throne, a bier's
 * sarcophagus) is an item of its own. */
export function dais(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { len, rect } = frame(f, back);
  const wid = Math.min(f.w, f.h);
  const top = board(g, rect(0.24, 0.04, len - 0.24, wid - 0.34), STONE, light, rnd, true, 0.05, 0);
  masonry(g, top, STONE, rnd, 0.4);
}

/** Collapsed wall: a few big dressed blocks tumbled out of a jagged gap in a wall (`back`, the side it fell from) into
 * the room, one or two turned over on their sides, on a flat patch of pale dust: 3-4 a square, 6-8 on 2 x 2. Each is
 * a flat light top with a lit edge on the NW, a darker side to the SE and a bold ink edge. */
export function collapsedWall(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, at } = frame(f, back);
  const wid = along ? f.h : f.w;
  const DRESSED = [0x6e685d, 0xb9b1a0, 0xe0d9c8];
  const steps = Math.max(8, Math.round(len * 8));
  // The dust: one flat pale patch, fullest by the wall.
  const fan: number[] = [];
  for (let k = 0; k <= steps; k++) fan.push(...at((k / steps) * len, 0.02));
  for (let k = steps; k >= 0; k--) {
    const e = Math.sin((k / steps) * Math.PI);
    fan.push(...at((k / steps) * len, Math.min(wid - 0.04, (0.45 + 0.5 * e) * wid * (0.9 + 0.1 * rnd()))));
  }
  g.poly(fan).fill({ color: 0xaaa392, alpha: 0.5 });
  // The breach: a jagged dark gap in the wall, a few chips run out from it.
  const breach: number[] = [];
  for (let k = 0; k <= steps; k++) breach.push(...at((k / steps) * len, 0));
  for (let k = steps; k >= 0; k--) breach.push(...at((k / steps) * len, k % 2 ? 0.04 + 0.05 * rnd() : 0.15 + 0.16 * rnd() * (k % 4 ? 1 : 1.6)));
  g.poly(breach).fill(INK);
  const [lx, ly] = light;
  for (let i = 0; i < Math.round(2 + len * wid); i++) {
    const [px, py] = at(len * (0.08 + 0.84 * rnd()), 0.14 + 0.1 * rnd());
    const r = 0.04 + 0.03 * rnd();
    g.poly([px - r, py, px, py - r * 0.9, px + r, py + r * 0.2, px + r * 0.2, py + r]).fill(DRESSED[0]).stroke({ width: 0.02, color: INK, join: 'round' });
  }
  // Pebbles.
  for (let i = 0; i < Math.round(4 + len * wid); i++) {
    const [px, py] = at(len * (0.05 + 0.9 * rnd()), wid * (0.45 + 0.5 * rnd()));
    g.circle(px, py, 0.02 + 0.015 * rnd()).fill(DRESSED[0]).stroke({ width: 0.018, color: INK });
  }
  // The blocks: spread along the wall in turn, the nearest the biggest; two of them turned well over.
  const n = Math.min(8, 3 + Math.round((len * wid - 1) * 1.3) + (rnd() < 0.5 ? 1 : 0));
  const turned = new Set([Math.floor(rnd() * n), Math.floor(rnd() * n)]);
  const order = Array.from({ length: n }, (_, k) => k);
  const cells = Math.ceil(n / 2);
  const blocks: [number, number, number, number, number][] = order.map((k) => {
    const row = k % 2;
    const u = len * ((Math.floor(k / 2) + 0.2 + 0.6 * rnd()) / cells);
    const l = 0.27 + 0.1 * rnd();
    const sv = Math.min(wid - l * 0.7, 0.28 + row * wid * 0.38 + 0.12 * rnd());
    const turn = turned.has(k) ? (rnd() < 0.5 ? 1 : -1) * (0.26 + 0.44 * rnd()) : (rnd() - 0.5) * 0.25;
    return [u, sv, l, l * (0.62 + 0.15 * rnd()), turn];
  });
  blocks.sort((p, q) => q[1] - p[1]);
  for (const [u, sv, l, w, turn] of blocks) {
    const [cx, cy] = at(Math.min(len - l * 0.6, Math.max(l * 0.6, u)), sv);
    const [c, sn] = [Math.cos(turn), Math.sin(turn)];
    const corner = (a: number, b: number, dx = 0, dy = 0): [number, number] => [cx + dx + a * c - b * sn, cy + dy + a * sn + b * c];
    // A chipped outline of 5-7 corners: one corner knocked off, the rest nudged, an edge or two notched.
    const base: [number, number][] = [[-1, -1], [1, -1], [1, 1], [-1, 1]];
    const knock = Math.floor(rnd() * 4);
    const shape: [number, number][] = [];
    base.forEach(([sa, sb], i) => {
      const [na, nb] = base[(i + 1) % 4];
      const [pa, pb] = base[(i + 3) % 4];
      const J = () => (rnd() - 0.5) * 0.14;
      if (i === knock) {
        shape.push([(sa + (pa - sa) * 0.42) * (l / 2), (sb + (pb - sb) * 0.42) * (w / 2)], [(sa + (na - sa) * 0.42) * (l / 2), (sb + (nb - sb) * 0.42) * (w / 2)]);
      } else shape.push([(sa + J()) * (l / 2), (sb + J()) * (w / 2)]);
      if (i !== knock && (i + knock) % 2 === 1 && rnd() < 0.7) shape.push([((sa + na) / 2 + J()) * (l / 2) - (sb - nb ? 0 : 0), ((sb + nb) / 2) * (w / 2) * (0.82 + 0.08 * rnd()) + (sb + nb ? 0 : 0)]);
    });
    const box = (dx = 0, dy = 0) => shape.flatMap(([a, b]) => corner(a, b, dx, dy));
    const h = w * 0.3;
    // The side, let down away from the light, then the top.
    castShadow(g, box(), light, 0.2);
    g.poly(box(-lx * h, -ly * h)).fill(DRESSED[0]).stroke({ width: OUT, color: INK, join: 'round' });
    for (const [a, b] of shape) g.moveTo(...corner(a, b)).lineTo(...corner(a, b, -lx * h, -ly * h));
    g.stroke({ width: OUT * 0.8, color: INK });
    g.poly(box()).fill(DRESSED[1]).stroke({ width: OUT, color: INK, join: 'round' });
    // The edges toward the light.
    shape.forEach(([a0, b0], k) => {
      const [a1, b1] = shape[(k + 1) % shape.length];
      const [ea, eb] = [a1 - a0, b1 - b0];
      const el = Math.hypot(ea, eb) || 1;
      const [nx, ny] = [(eb / el) * c - (-ea / el) * sn, (eb / el) * sn + (-ea / el) * c];
      if (nx * lx + ny * ly > 0.3) g.moveTo(...corner(a0 * 0.86, b0 * 0.86)).lineTo(...corner(a1 * 0.86, b1 * 0.86));
    });
    g.stroke({ width: 0.03, color: DRESSED[2], cap: 'round' });
  }
}

/** Dry fountain: a broad eight-sided stone basin, dry — its floor cracked and dusty, dead leaves blown in, a green
 * stain where the water stood — and in the middle a tiered pedestal: a chipped lower bowl, a column rising from it
 * to a small upper bowl, a finial on top, its shadow across the basin. */
export function dryFountain(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const R = Math.min(f.w, f.h) * 0.46;
  const oct = (r: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 8; i++) pts.push(cx + Math.cos(Math.PI / 8 + (i * Math.PI) / 4) * r, cy + Math.sin(Math.PI / 8 + (i * Math.PI) / 4) * r);
    return pts;
  };
  castShadow(g, oct(R), light, 0.2);
  g.poly(oct(R)).fill(MARBLE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(oct(R * 0.97).map((q, i) => q + (i % 2 ? light[1] : light[0]) * 0.02)).fill(MARBLE[1]);
  g.poly(oct(R * 0.82)).fill(0x8a7e6a).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(oct(R * 0.8).map((q, i) => (i % 2 ? cy : cx) + (q - (i % 2 ? cy : cx)) * 0.98 - (i % 2 ? light[1] : light[0]) * 0.04)).fill({ color: 0x000000, alpha: 0.2 });
  g.poly(patch(cx + 0.1, cy + 0.15, R * 0.5, R * 0.3, rnd, 0.3, 14)).fill({ color: 0x5a6a3a, alpha: 0.35 });
  // Cracks.
  for (let c = 0; c < 4; c++) {
    let a = rnd() * Math.PI * 2;
    let [x, y] = [cx + Math.cos(a) * R * 0.3, cy + Math.sin(a) * R * 0.3];
    g.moveTo(x, y);
    for (let k = 0; k < 4; k++) {
      a += (rnd() - 0.5) * 1.2;
      x += Math.cos(a) * R * 0.11;
      y += Math.sin(a) * R * 0.11;
      g.lineTo(x, y);
    }
  }
  g.stroke({ width: 0.014, color: 0x4a4234, join: 'miter' });
  // Dead leaves.
  for (let i = 0; i < 8; i++) {
    const a = rnd() * Math.PI * 2;
    const d = R * (0.35 + 0.4 * rnd());
    g.poly(oval(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.04, 0.02, rnd() * 3, 8)).fill([0x8a5a22, 0x6a4a1a, 0xa86a2a][i % 3]).stroke({ width: 0.018, color: INK });
  }
  // The tiered pedestal: its shadow, the lower bowl (dry, chipped), the upper bowl on its column, the finial.
  castShadow(g, ring(cx, cy, R * 0.18, rnd, 0, 12), light, 0.6);
  g.circle(cx, cy, R * 0.34).fill(MARBLE[0]).stroke({ width: LINE, color: INK });
  g.circle(cx + light[0] * 0.02, cy + light[1] * 0.02, R * 0.3).fill(MARBLE[1]);
  g.circle(cx, cy, R * 0.25).fill(0x9a8e78).stroke({ width: 0.018, color: INK });
  const ca = rnd() * Math.PI * 2;
  g.poly([cx + Math.cos(ca) * R * 0.34, cy + Math.sin(ca) * R * 0.34, cx + Math.cos(ca + 0.25) * R * 0.27, cy + Math.sin(ca + 0.25) * R * 0.27, cx + Math.cos(ca + 0.5) * R * 0.34, cy + Math.sin(ca + 0.5) * R * 0.34]).fill(0x9a8e78).stroke({ width: 0.018, color: INK });
  g.poly(oval(cx - light[0] * R * 0.06, cy - light[1] * R * 0.06, R * 0.12, R * 0.12, 0, 12)).fill({ color: 0x000000, alpha: 0.3 });
  g.circle(cx, cy, R * 0.17).fill(MARBLE[1]).stroke({ width: LINE, color: INK });
  g.circle(cx + light[0] * 0.015, cy + light[1] * 0.015, R * 0.13).fill(MARBLE[2]);
  g.circle(cx, cy, R * 0.11).fill(0x9a8e78).stroke({ width: 0.018, color: INK });
  g.circle(cx, cy, R * 0.07).fill(MARBLE[1]).stroke({ width: 0.018, color: INK });
  g.circle(cx + light[0] * R * 0.025, cy + light[1] * R * 0.025, R * 0.03).fill(0xffffff);
}

/** Ritual circle: drawn on the floor in chalk, blood or something that glows — a double ring with runes between
 * the rings, a seven-pointed star inside, candle stubs at its points, smudges where feet have scuffed it. */
export function ritualCircle(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  void light;
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const R = Math.min(f.w, f.h) * 0.44;
  const [ink, glow] = [
    [0xe8e4dc, 0],
    [0x8a1a14, 0],
    [0xa898d8, 0],
    [0xe8e4dc, 0],
  ][v % 4];
  if (glow) for (let i = 0; i < 4; i++) g.circle(cx, cy, R * (1.15 - i * 0.12)).fill({ color: glow, alpha: 0.06 });
  const wob = (r: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 48; i++) {
      const a = (i / 48) * Math.PI * 2;
      const rr = r * (1 + (rnd() - 0.5) * 0.015);
      pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    return pts;
  };
  const rings = [wob(R), wob(R * 0.82)];
  for (const [i, w] of [0.035, 0.025].entries()) g.poly(rings[i]).stroke({ width: w + 0.03, color: INK, alpha: 0.7 });
  for (const [i, w] of [0.035, 0.025].entries()) g.poly(rings[i]).stroke({ width: w, color: ink, alpha: 0.9 });
  // Runes between the rings.
  const rs = rnd() * 1e9;
  for (const pass of [0, 1]) {
  let sd = rs;
  const rr = () => ((sd = (sd * 16807) % 2147483647) / 2147483647);
  for (let i = 0; i < 14; i++) {
    const a = (i / 14) * Math.PI * 2;
    const [x, y] = [cx + Math.cos(a) * R * 0.91, cy + Math.sin(a) * R * 0.91];
    const [tx, ty] = [-Math.sin(a), Math.cos(a)];
    const [nx, ny] = [Math.cos(a), Math.sin(a)];
    const k = Math.floor(rr() * 4);
    const s = R * 0.05;
    g.moveTo(x - nx * s, y - ny * s).lineTo(x + nx * s, y + ny * s);
    if (k === 0) g.moveTo(x - nx * s, y - ny * s).lineTo(x + tx * s, y + ty * s);
    if (k === 1) g.moveTo(x + tx * s * 0.8 - nx * s * 0.3, y + ty * s * 0.8 - ny * s * 0.3).lineTo(x - tx * s * 0.8 + nx * s * 0.3, y - ty * s * 0.8 + ny * s * 0.3);
    if (k === 2) g.moveTo(x + nx * s, y + ny * s).lineTo(x + tx * s, y + ty * s).lineTo(x - nx * s * 0.2, y - ny * s * 0.2);
    if (k === 3) g.moveTo(x - tx * s, y - ty * s).lineTo(x + tx * s, y + ty * s);
  }
  g.stroke(pass ? { width: 0.016, color: ink, alpha: 0.9, cap: 'round', join: 'round' } : { width: 0.036, color: INK, alpha: 0.7, cap: 'round', join: 'round' });
  }
  // The star.
  const ph = -Math.PI / 2;
  const pts: [number, number][] = [];
  for (let i = 0; i < 7; i++) pts.push([cx + Math.cos(ph + (i * Math.PI * 2) / 7) * R * 0.8, cy + Math.sin(ph + (i * Math.PI * 2) / 7) * R * 0.8]);
  const hub = wob(R * 0.18);
  for (const pass of [0, 1]) {
    g.moveTo(...pts[0]);
    for (let i = 1; i <= 7; i++) g.lineTo(...pts[(i * 3) % 7]);
    g.stroke(pass ? { width: 0.022, color: ink, alpha: 0.85, join: 'miter' } : { width: 0.05, color: INK, alpha: 0.7, join: 'miter' });
    g.poly(hub).stroke(pass ? { width: 0.02, color: ink, alpha: 0.85 } : { width: 0.046, color: INK, alpha: 0.7 });
  }
  // Scuffs.
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    g.poly(patch(cx + Math.cos(a) * R * 0.85, cy + Math.sin(a) * R * 0.85, 0.07, 0.05, rnd, 0.3, 8)).fill({ color: 0x5a554c, alpha: 0.5 });
  }
  // Candle stubs at the points.
  for (const [x, y] of pts) {
    g.circle(x, y, 0.07).fill({ color: 0xffc23a, alpha: 0.15 });
    g.circle(x, y, 0.035).fill(0xf0e8d0).stroke({ width: 0.018, color: INK });
    g.circle(x, y, 0.015).fill(0xffc23a);
  }
}
