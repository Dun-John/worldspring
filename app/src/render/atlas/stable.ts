import { Graphics } from 'pixi.js';
import { FT, INK, IRON, WOOD, oval, shade } from './common';
import { footGrass } from './street';
import { LINE, OUT } from './farm';
import { bevel } from './yard';
import { moss } from './stones';

// Roadmap 5 steppe towns: a horse park's horses and hitching rails.

/** Horse, standing, seen from straight above, its nose toward +x (10 ft nose to tail): broad round
 * haunches, a narrower barrel and shoulders, the neck tapering forward to a long head with two ears at the
 * poll, the mane along the top of the neck, the tail laid out behind; bay with a star (v0), dapple grey (v1),
 * chestnut with a flaxen mane and a blaze (v2), the dapple grey saddled (v3). */
export function horse(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const F = FT;
  const coat = [
    [0x4e2c18, 0x7a4a2a, 0x9e6a40],
    [0x6a6660, 0x9a9690, 0xc4c0b8],
    [0x6e3818, 0xa0582e, 0xc47e4a],
    [0x6a6660, 0x9a9690, 0xc4c0b8],
  ][v];
  const mane = [0x241a14, 0x4a4640, 0xd8c08a, 0x4a4640][v];
  // Half-width (ft) along the body from rump to nose; the head turned a little to one side.
  const prof: [number, number][] = [
    [-3.8, 0], [-3.65, 0.6], [-3.3, 1.08], [-2.7, 1.34], [-1.9, 1.34], [-1.0, 1.12], [0.0, 1.16], [0.9, 1.12],
    [1.45, 0.96], [1.85, 0.62], [2.4, 0.46], [2.9, 0.42], [3.2, 0.52], [3.7, 0.5], [4.3, 0.4], [4.8, 0.36], [5.1, 0.25], [5.25, 0],
  ];
  const bend = [0.05, -0.07, 0.08, -0.04][v];
  const mid = (x: number) => (x > 1.5 ? bend * (x - 1.5) * (x - 1.5) : 0);
  const at = (x: number, y: number): [number, number] => [x * F, (y + mid(x)) * F];
  const outline = (k: number, dx: number, dy: number): number[] => {
    const pts: number[] = [];
    for (const [x, w] of prof) pts.push(...at(x * k + dx, -w * k + dy));
    for (let i = prof.length - 2; i > 0; i--) pts.push(...at(prof[i][0] * k + dx, prof[i][1] * k + dy));
    return pts;
  };
  // The tail, laid out behind and a little to one side, under the body.
  const ts = v % 2 ? 1 : -1;
  g.moveTo(...at(-3.5, -0.3)).quadraticCurveTo(...at(-4.6, ts * 0.1 - 0.5), ...at(-5.15, ts * 0.55))
    .quadraticCurveTo(...at(-4.5, ts * 0.1 + 0.5), ...at(-3.5, 0.3)).closePath()
    .fill(mane).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(-3.8, 0)).quadraticCurveTo(...at(-4.5, ts * 0.12), ...at(-4.95, ts * 0.45)).stroke({ width: 1.2, color: shade(mane, 1.6), alpha: 0.7 });
  const body = outline(1, 0, 0);
  g.poly(body).fill(coat[0]).stroke({ width: OUT, color: INK, join: 'round' });
  // Lit toward the NW: the coat's mid over most of it, a sunlit band along the north flank and haunch.
  g.poly(outline(0.9, 0.05, -0.08)).fill(coat[1]);
  g.poly(oval(-2.2 * F, -0.45 * F, 1.0 * F, 0.45 * F, -0.1, 14)).fill({ color: coat[2], alpha: 0.75 });
  g.poly(oval(0.4 * F, -0.5 * F, 1.0 * F, 0.32 * F, 0, 14)).fill({ color: coat[2], alpha: 0.6 });
  // Spine and the cleft between the haunches.
  g.moveTo(...at(-3.75, 0)).lineTo(...at(-3.0, 0)).stroke({ width: 1.5, color: shade(coat[0], 0.75), cap: 'round' });
  if (v === 1 || v === 3) {
    for (let i = 0; i < 9; i++) g.circle((-3 + rnd() * 3.6) * F, (rnd() - 0.5) * 1.6 * F, (0.18 + rnd() * 0.12) * F).fill({ color: coat[2], alpha: 0.55 });
  }
  // The head: a paler muzzle, eyes at its sides, nostrils at the nose; a star or blaze.
  g.poly(oval(...at(4.85, 0), 0.38 * F, 0.22 * F, Math.atan(2 * bend * 3.35), 12)).fill(shade(coat[1], v === 1 || v === 3 ? 0.8 : 0.82));
  for (const sy of [-1, 1]) g.circle(...at(3.7, sy * 0.37), 1.5).fill(INK);
  for (const sy of [-1, 1]) g.circle(...at(5.05, sy * 0.1), 1.1).fill(shade(coat[0], 0.6));
  if (v === 0) g.poly(oval(...at(3.75, 0), 0.16 * F, 0.12 * F, 0, 8)).fill(0xf0e8d8);
  if (v === 2) g.poly(oval(...at(4.2, 0), 0.75 * F, 0.08 * F, Math.atan(2 * bend * 2.7), 10)).fill(0xf0e8d8);
  // Mane: falling to one side of the crest from withers to poll, its free edge wavy; a forelock between the ears.
  const ms = v % 2 ? 1 : -1;
  const mp: number[] = [...at(1.35, 0), ...at(3.15, 0)];
  for (let i = 0; i <= 8; i++) {
    const x = 3.15 - (i / 8) * 1.8;
    mp.push(...at(x, ms * (0.2 + (i % 2) * 0.1 + 0.06 * Math.sin(i * 1.7))));
  }
  g.poly(mp).fill(mane).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(1.6, ms * 0.07)).lineTo(...at(3.0, ms * 0.07)).stroke({ width: 1.2, color: shade(mane, 1.6), alpha: 0.6, cap: 'round' });
  g.poly(oval(...at(3.38, 0), 0.24 * F, 0.12 * F, 0, 8)).fill(mane).stroke({ width: 1, color: INK });
  // Ears: two pricked points at the poll, either side of the forelock, over the head.
  for (const sy of [-1, 1]) {
    const [ex, ey] = at(3.15, sy * 0.3);
    const [tx, ty] = at(3.6, sy * 0.72);
    g.poly([ex - 0.16 * F, ey - sy * 0.08 * F, tx, ty, ex + 0.2 * F, ey + sy * 0.02 * F]).fill(coat[0]).stroke({ width: LINE, color: INK, join: 'round' });
  }
  if (v === 3) {
    // Saddle: a red felt blanket square across the back, the leather seat on it (pommel forward, cantle
    // behind), the girth and stirrups hanging just past the flanks.
    const [sx, sw] = [0.15 * F, 1.35 * F];
    for (const sy of [-1, 1]) g.roundRect(sx - 0.1 * F, sy * (sw + 0.05 * F) - 0.14 * F, 0.2 * F, 0.28 * F, 1.5).fill(IRON[2]).stroke({ width: 1.2, color: INK });
    g.poly([sx - 1.0 * F, -sw, sx + 0.9 * F, -sw + 2, sx + 0.95 * F, sw - 1, sx - 1.05 * F, sw]).fill(0xa8443a).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly([sx - 0.9 * F, -sw + 3, sx + 0.8 * F, -sw + 4, sx + 0.85 * F, -sw * 0.55, sx - 0.95 * F, -sw * 0.55]).fill({ color: 0xc8644e, alpha: 0.7 });
    g.poly(oval(sx, 0, 0.8 * F, 0.58 * F, 0, 16)).fill(WOOD[0]).stroke({ width: LINE, color: INK });
    g.poly(oval(sx - 0.05 * F, -0.1 * F, 0.55 * F, 0.34 * F, 0, 14)).fill(WOOD[1]);
    g.moveTo(sx + 0.62 * F, -0.4 * F).quadraticCurveTo(sx + 0.85 * F, 0, sx + 0.62 * F, 0.4 * F).stroke({ width: 2.4, color: WOOD[2], cap: 'round' });
    g.moveTo(sx - 0.7 * F, -0.45 * F).quadraticCurveTo(sx - 0.95 * F, 0, sx - 0.7 * F, 0.45 * F).stroke({ width: 2.4, color: WOOD[2], cap: 'round' });
  }
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
