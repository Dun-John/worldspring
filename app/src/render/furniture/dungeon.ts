import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { INK, IRON, LINE, type Light, OUT, WOOD, board, castShadow, frame, glowPool, oval, patch, ring } from './common';
import { MARBLE } from './sacred';
import { GILT } from './tables';
import { STONE, masonry } from './home';
import { screenAxes } from './under';

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
  fireTop(g, cx, cy, r * 0.7, rnd);
}

/** Candelabrum, from a high angle like the urn and lantern (from straight above it is only a star of arms): a tall
 * iron stand on three clawed feet, five branches rising from it in pairs of curves round a middle one, a cream
 * candle with a flame in each cup, wax dripped on the floor, a soft glow. */
export function candelabrum(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt } = screenAxes(light);
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  glowPool(g, ...P(0, 0.1), 0.55, 0xfcd36a);
  castShadow(g, ring(...P(0, -0.4), 0.08, rnd, 0, 8), light, 1.1);
  for (let i = 0; i < 6; i++) g.poly(oval(...P((rnd() - 0.5) * 0.5, -0.42 + (rnd() - 0.5) * 0.08), 0.025, 0.016, 0, 8)).fill(0xf0e8d0).stroke({ width: 0.018, color: INK });
  const iron = (pts: [number, number][], w: number) => {
    for (const [ww, c] of [
      [w + 0.032, INK],
      [w, IRON[1]],
    ] as const) {
      g.moveTo(...pts[0]);
      if (pts.length === 3) g.quadraticCurveTo(...pts[1], ...pts[2]);
      else g.lineTo(...pts[1]);
      g.stroke({ width: ww, color: c, cap: 'round', join: 'round' });
    }
  };
  // Feet, the column, the branches.
  for (const k of [-1, 0, 1]) iron([P(0, -0.33), P(k * 0.14, k === 0 ? -0.44 : -0.42)], 0.04);
  iron([P(0, -0.42), P(0, 0.2)], 0.05);
  for (const k of [-1, 1]) {
    iron([P(0, -0.02), P(k * 0.3, -0.04), P(k * 0.3, 0.14)], 0.036);
    iron([P(0, 0.06), P(k * 0.16, 0.05), P(k * 0.16, 0.2)], 0.036);
  }
  g.poly(oval(...P(0, -0.32), 0.05, 0.035, 0, 10)).fill(IRON[1]).stroke({ width: 0.018, color: INK });
  // Cups, candles, flames.
  for (const [a, b] of [[-0.3, 0.14], [-0.16, 0.2], [0, 0.22], [0.16, 0.2], [0.3, 0.14]] as [number, number][]) {
    g.poly([...P(a - 0.05, b), ...P(a + 0.05, b), ...P(a + 0.035, b - 0.03), ...P(a - 0.035, b - 0.03)]).fill(IRON[1]).stroke({ width: LINE, color: INK, join: 'round' });
    const candle = [...P(a - 0.03, b + 0.1), ...P(a + 0.03, b + 0.1), ...P(a + 0.03, b), ...P(a - 0.03, b)];
    g.poly(candle).fill(0xf4ecd8).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly([...P(a - 0.025, b + 0.1), ...P(a - 0.008, b + 0.1), ...P(a - 0.008, b), ...P(a - 0.025, b)]).fill(0xfffaf0);
    g.circle(...P(a, b + 0.15), 0.05).fill({ color: 0xffc23a, alpha: 0.25 });
    g.poly(oval(...P(a, b + 0.135), 0.018, 0.03, Math.atan2(P(0, 1)[1] - P(0, 0)[1], P(0, 1)[0] - P(0, 0)[0]), 10)).fill(0xf9a23a);
    g.poly(oval(...P(a, b + 0.13), 0.009, 0.016, Math.atan2(P(0, 1)[1] - P(0, 0)[1], P(0, 1)[0] - P(0, 0)[0]), 8)).fill(0xfff2c0);
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
  g.poly([...at(0.38, 0), ...at(0.62, 0), ...at(0.58, 0.08), ...at(0.42, 0.08)]).fill(IRON[0]).stroke({ width: 0.018, color: INK, join: 'round' });
  for (const [w, c] of [
    [0.05, INK],
    [0.028, IRON[1]],
  ] as const) {
    g.moveTo(...at(0.5, 0.06)).lineTo(...at(0.5, 0.2)).stroke({ width: w, color: c, cap: 'round' });
  }
  g.poly(oval(...at(0.5, 0.2), 0.05, 0.04, 0, 10)).stroke({ width: 0.03, color: INK });
  g.poly(oval(...at(0.5, 0.2), 0.05, 0.04, 0, 10)).stroke({ width: 0.016, color: IRON[1] });
  for (const [w, c] of [
    [0.065, INK],
    [0.04, WOOD[1]],
  ] as const) {
    g.moveTo(...at(0.5, 0.12)).lineTo(...at(0.5, 0.36)).stroke({ width: w, color: c, cap: 'round' });
  }
  g.circle(hx, hy, 0.065).fill(0x3a2a1c).stroke({ width: 0.018, color: INK });
  fireTop(g, hx, hy, 0.09, rnd);
}

/** Iron maiden, from a high angle like the headstones (upright things only read from the side), big: a riveted,
 * iron-banded shell shaped like a person — a domed head with a sculpted grim face, shoulders, a taper to the foot —
 * its two doors swung wide, rows of bright spikes on their insides and in the dark hollow, dried blood on them and
 * pooled below. */
export function ironMaiden(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt } = screenAxes(light);
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  const BLOOD = 0x5a1410;
  castShadow(g, ring(...P(0, -0.44), 0.2, rnd, 0, 12), light, 1.2);
  g.poly(patch(...P(0.04, -0.46), 0.12, 0.05, rnd, 0.3, 10)).fill({ color: BLOOD, alpha: 0.8 });
  // The shell: half widths from the foot to the top of the head.
  const prof: [number, number][] = [[-0.46, 0.17], [-0.2, 0.2], [0.12, 0.22], [0.2, 0.2], [0.26, 0.12], [0.34, 0.12], [0.44, 0.08], [0.48, 0]];
  const outline: number[] = [];
  for (const [b, w] of prof) outline.push(...P(w, b));
  for (const [b, w] of [...prof].reverse()) outline.push(...P(-w, b));
  // The doors first (behind the shell's edge), hinged at its sides, swung out wide.
  const spike = (a: number, b: number, dir: number) => {
    const [tx, ty] = P(a + dir * 0.045, b);
    const [ux, uy] = P(a, b + 0.018);
    const [vx, vy] = P(a, b - 0.018);
    g.poly([ux, uy, tx, ty, vx, vy]).fill(0xe8e4dc).stroke({ width: 0.018, color: INK });
  };
  for (const k of [-1, 1]) {
    const door = [...P(k * 0.2, 0.18), ...P(k * 0.44, 0.12), ...P(k * 0.43, -0.36), ...P(k * 0.18, -0.44)];
    g.poly(door).fill(IRON[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly([...P(k * 0.22, 0.15), ...P(k * 0.41, 0.1), ...P(k * 0.4, -0.34), ...P(k * 0.2, -0.41)]).fill(k < 0 ? IRON[2] : IRON[1]);
    for (let r = 0; r < 5; r++) {
      for (const t of [0.3, 0.7]) {
        const a = k * (0.22 + 0.19 * t);
        const b = 0.08 - r * 0.1 - t * 0.04;
        spike(a, b, -k);
        if ((r + (t > 0.5 ? 1 : 0)) % 3 === 0) g.circle(...P(a - k * 0.02, b), 0.012).fill({ color: BLOOD, alpha: 0.85 });
      }
    }
    for (const b of [0.14, -0.38]) g.poly([...P(k * 0.17, b - 0.03), ...P(k * 0.23, b - 0.03), ...P(k * 0.23, b + 0.03), ...P(k * 0.17, b + 0.03)]).fill(IRON[0]).stroke({ width: 0.018, color: INK });
  }
  g.poly(outline).fill(IRON[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(outline.map((q, i) => (i % 2 ? cy : cx) + (q - (i % 2 ? cy : cx)) * 0.94 + (i % 2 ? light[1] : light[0]) * 0.02)).fill(IRON[1]);
  // The hollow, dark, spikes catching the light, blood.
  const hollow = [...P(-0.14, 0.14), ...P(0.14, 0.14), ...P(0.15, -0.38), ...P(-0.15, -0.38)];
  g.poly(hollow).fill(0x120c0a).stroke({ width: 0.018, color: INK });
  for (let r = 0; r < 5; r++) for (const a of [-0.07, 0, 0.07]) {
    const [sx, sy] = P(a + (r % 2 ? 0.035 : 0), 0.08 - r * 0.1);
    g.poly([sx, sy - 0.014, sx + 0.012, sy + 0.008, sx - 0.012, sy + 0.008]).fill(r % 2 ? IRON[2] : 0xc8c4bc);
  }
  g.poly(patch(...P(0.02, -0.3), 0.05, 0.03, rnd, 0.4, 8)).fill({ color: BLOOD, alpha: 0.8 });
  // Bands and rivets on the shell's rim.
  for (const b of [0.18, -0.12, -0.42]) {
    const w = 0.2;
    g.moveTo(...P(-w, b)).lineTo(...P(-0.15, b)).moveTo(...P(0.15, b)).lineTo(...P(w, b));
    g.stroke({ width: 0.02, color: INK });
    for (const a of [-0.18, 0.18]) g.circle(...P(a, b), 0.01).fill(IRON[2]);
  }
  // The head and its sculpted face.
  g.poly(oval(...P(0, 0.34), 0.1, 0.11, 0, 16)).fill(IRON[1]).stroke({ width: 0.018, color: INK });
  g.poly(oval(...P(-0.02, 0.36), 0.05, 0.05, 0, 10)).fill({ color: IRON[2], alpha: 0.6 });
  for (const a of [-0.04, 0.04]) {
    g.poly(oval(...P(a, 0.36), 0.022, 0.012, 0, 8)).fill(INK);
    g.moveTo(...P(a - 0.03, 0.39)).lineTo(...P(a + 0.02, 0.38)).stroke({ width: 0.016, color: INK });
  }
  g.poly([...P(0, 0.35), ...P(0.015, 0.31), ...P(-0.015, 0.31)]).fill(IRON[0]);
  g.moveTo(...P(-0.035, 0.28)).quadraticCurveTo(...P(0, 0.295), ...P(0.035, 0.28)).stroke({ width: 0.016, color: INK });
}

/** Stocks, from a high angle: two posts holding a heavy board split along its middle, a head hole between two
 * wrist holes, a hinge at one end and a padlock at the other, straw on the ground before it. */
export function stocks(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const { pt } = screenAxes(light);
  const L = Math.max(f.w, f.h) * 0.42;
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  for (let i = 0; i < 12; i++) {
    const [x, y] = P((rnd() - 0.5) * L * 1.6, -0.3 - rnd() * 0.12);
    g.moveTo(x, y).lineTo(x + (rnd() - 0.5) * 0.08, y + (rnd() - 0.5) * 0.04).stroke({ width: 0.014, color: 0xc8a860 });
  }
  castShadow(g, [...P(-L, -0.32), ...P(L, -0.32), ...P(L, -0.2), ...P(-L, -0.2)], light, 0.6);
  for (const a of [-L * 0.92, L * 0.92]) {
    const post = [...P(a - 0.06, 0.28), ...P(a + 0.06, 0.28), ...P(a + 0.06, -0.32), ...P(a - 0.06, -0.32)];
    g.poly(post).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly([...P(a - 0.04, 0.26), ...P(a, 0.26), ...P(a, -0.3), ...P(a - 0.04, -0.3)]).fill(WOOD[1]);
  }
  const board = [...P(-L, 0.18), ...P(L, 0.18), ...P(L, -0.14), ...P(-L, -0.14)];
  g.poly(board).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...P(-L + 0.02, 0.16), ...P(L - 0.02, 0.16), ...P(L - 0.02, 0.09), ...P(-L + 0.02, 0.09)]).fill(WOOD[2]);
  g.moveTo(...P(-L, 0.02)).lineTo(...P(L, 0.02)).stroke({ width: 0.018, color: INK });
  for (const [a, r] of [
    [0, 0.08],
    [-L * 0.5, 0.045],
    [L * 0.5, 0.045],
  ]) {
    g.poly(oval(...P(a, 0.02), r, r, 0, 14)).fill(0x1e1712).stroke({ width: 0.018, color: INK });
  }
  g.poly([...P(L - 0.06, 0.06), ...P(L + 0.02, 0.06), ...P(L + 0.02, -0.04), ...P(L - 0.06, -0.04)]).fill(IRON[1]).stroke({ width: 0.018, color: INK });
  g.poly(oval(...P(L - 0.02, 0.09), 0.025, 0.025, 0, 8)).stroke({ width: 0.014, color: IRON[2] });
  g.poly([...P(-L, 0.06), ...P(-L + 0.06, 0.06), ...P(-L + 0.06, -0.02), ...P(-L, -0.02)]).fill(IRON[0]);
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
