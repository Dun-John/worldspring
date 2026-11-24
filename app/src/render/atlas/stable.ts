import { Graphics } from 'pixi.js';
import { PX } from '../scale';
import { drawHorse } from '../furniture/guild';
import { FT, INK, IRON, WOOD, oval, shade } from './common';
import { footGrass } from './street';
import { LINE, OUT } from './farm';
import { bevel } from './yard';
import { moss } from './stones';

// Roadmap 5 steppe towns: a horse park's horses and hitching rails.

/** Horse, standing, seen from above, its nose toward +x (about 8 ft nose to tail): the stable stall's horse
 * (`drawHorse` in furniture.ts) with its head and neck straight forward: bay with a star (v0), dapple grey (v1),
 * chestnut with a flaxen mane and a blaze (v2), the dapple grey under a red felt saddle blanket (v3). */
export function horse(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  // Drawn in squares: the stall's frame has the nose at small `s`, so `s` runs toward the tail (-x here).
  g.scale.set(PX);
  const at = (u: number, s: number): [number, number] => [1.19 - s, u];
  // (R5 towns, 114: a dark rim round body, neck and head, so a horse reads on the trodden dirt.)
  g.ellipse(-0.06, 0, 0.74, 0.31).ellipse(0.74, 0, 0.5, 0.17).fill({ color: 0x24190f, alpha: 0.6 });
  drawHorse(g, at, 1, 0, v === 2 ? -1 : 1, [-0.7071, -0.7071], rnd, v === 3 ? 2 : v + 1, { bend: 0, shadow: false, blanket: v === 3 });
}

/** Weathered wood: [dark, mid, light, sunlit]. */
export const GREY_WOOD = [0x5e5a52, 0x8a857a, 0xaaa496, 0xc8c2b2];

/** Hitching rail, along x (fixed 10 ft): a rounded rail mortised through two stout posts whose tops show
 * end grain, lit along the north; an iron tie ring (v0), a lead rope wrapped round it, its end trailing
 * (v1), weathered grey with cracks, a split post and grass at the feet (v2), a water bucket by a post (v3). */
export function hitchingRail(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 2 ? GREY_WOOD : WOOD;
  // (R5 towns, 114: stout enough to read at a town's battle zoom.)
  const [L, T, P, ph] = [4.2 * FT, 0.26 * FT, 4.0 * FT, 0.42 * FT];
  if (v === 2) for (const x of [-P, P]) footGrass(g, x + 0.3 * FT, ph + 2, 0.45 * FT, rnd);
  g.roundRect(-L, -T, L * 2, T * 2, T * 0.7).fill(tones[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.roundRect(-L + 2, -T + 1.2, L * 2 - 4, T * 0.62, T * 0.3).fill(tones[2]);
  g.roundRect(-L + 2, T * 0.32, L * 2 - 4, T * 0.6, T * 0.3).fill(shade(tones[1], 0.84));
  for (let k = 0; k < 4; k++) {
    const x = -L + 0.6 * FT + rnd() * (L * 2 - 2 * FT);
    g.moveTo(x, (rnd() - 0.5) * T * 0.6).lineTo(x + (0.8 + rnd()) * FT, (rnd() - 0.5) * T * 0.6);
  }
  g.stroke({ width: v === 2 ? 1.4 : 1.1, color: shade(tones[0], v === 2 ? 0.8 : 1), cap: 'round' });
  for (const x of [-P, P]) {
    bevel(g, x - ph, -ph, x + ph, ph, 2.2, tones, OUT);
    g.circle(x, 0, ph * 0.5).stroke({ width: 1.1, color: shade(tones[0], 1.05), alpha: 0.8 });
    g.circle(x, 0, ph * 0.22).stroke({ width: 1.1, color: shade(tones[0], 1.05), alpha: 0.8 });
  }
  if (v === 2) {
    g.moveTo(P - ph + 2, -2).lineTo(P + 1, 1).lineTo(P + ph - 1, 0).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(-1.5 * FT, -1).lineTo(0.6 * FT, 0.5).stroke({ width: 1.4, color: shade(tones[0], 0.75), cap: 'round' });
    moss(g, -P - ph * 0.3, -ph * 0.4, ph * 0.55, rnd);
  }
  const ink2 = (draw: () => void, w: number, color: number) => {
    draw();
    g.stroke({ width: w + 2, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: w, color, cap: 'round', join: 'round' });
  };
  if (v === 0 || v === 3) {
    const x = v === 0 ? 0.9 * FT : -1.4 * FT;
    ink2(() => g.circle(x, T + 0.22 * FT, 0.24 * FT), 2.4, IRON[2]);
  }
  if (v === 1) {
    // A lead rope: three turns round the rail, its end trailing off to the south, frayed.
    const rope = 0xc8b07a;
    const x = -1.3 * FT;
    for (let k = 0; k < 3; k++) ink2(() => g.moveTo(x + k * 5 - 2.5, -T - 1.2).lineTo(x + k * 5 + 2.5, T + 1.2), 2.6, rope);
    ink2(() => g.moveTo(x + 13, T).quadraticCurveTo(x + 0.6 * FT, T + 1.2 * FT, x + 1.8 * FT, T + 1.3 * FT), 2.6, rope);
    const [ex, ey] = [x + 1.8 * FT, T + 1.3 * FT];
    for (const a of [-0.5, 0, 0.5]) g.moveTo(ex, ey).lineTo(ex + Math.cos(a) * 6, ey + Math.sin(a) * 6);
    g.stroke({ width: 1.5, color: shade(rope, 0.7), cap: 'round' });
  }
  if (v === 3) {
    // A wooden bucket of water by the east post, its handle laid over.
    const [bx, by, R] = [P - 1.2 * FT, T + 0.85 * FT, 0.6 * FT];
    g.circle(bx, by, R).fill(WOOD[2]).stroke({ width: LINE, color: INK });
    for (let k = 0; k < 12; k++) {
      const a = (k / 12) * Math.PI * 2;
      g.moveTo(bx + Math.cos(a) * R * 0.7, by + Math.sin(a) * R * 0.7).lineTo(bx + Math.cos(a) * R * 0.98, by + Math.sin(a) * R * 0.98);
    }
    g.stroke({ width: 1, color: WOOD[0], alpha: 0.8 });
    g.circle(bx, by, R * 0.86).stroke({ width: 2.2, color: IRON[1] });
    g.circle(bx, by, R * 0.64).fill(0x4a8a90).stroke({ width: 1.2, color: shade(0x255a70, 0.8) });
    g.moveTo(bx - R * 0.64, by).arc(bx, by, R * 0.64, Math.PI, Math.PI * 1.5).stroke({ width: 2, color: 0x1e4a5c, alpha: 0.6 });
    g.poly(oval(bx - R * 0.2, by - R * 0.15, R * 0.16, R * 0.1, -0.3, 10)).fill({ color: 0xe6f4ec, alpha: 0.75 });
    ink2(() => g.moveTo(bx - R * 0.95, by + R * 0.3).quadraticCurveTo(bx, by + R * 0.55, bx + R * 0.95, by - R * 0.3), 2, IRON[2]);
  }
}
