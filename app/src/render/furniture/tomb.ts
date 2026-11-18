import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { rng } from '../atlas';
import { BONE, BRASS, INK, IRON, LINE, type Light, OUT, WOOD, bone, castShadow, frame, litEdges, oval, patch, quad, ring, shade, skullTop } from './common';
import { BRONZE, GILT, candle } from './sacred';
import { rock, screenAxes } from './under';

/** Scattered bones: long bones lying every way, a jaw, ribs, a few knuckle bits; no skull (that's the
 * remains and the pile). */
export function scatteredBones(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const T = [0xa89c80, 0xcfc2a2];
  const at = (r: number): [number, number] => {
    const a = rnd() * Math.PI * 2;
    const d = r * Math.sqrt(rnd());
    return [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
  };
  // Knuckle bits.
  for (let i = 0, n = 3 + Math.floor(rnd() * 5); i < n; i++) {
    const [x, y] = at(0.42);
    g.circle(x, y, 0.03 + 0.015 * rnd()).fill(T[1]).stroke({ width: LINE, color: INK });
  }
  // Ribs: curved bars, a few or none.
  const ribs = Math.floor(rnd() * 4);
  if (ribs) {
    const [rx, ry] = at(0.2);
    const ra = rnd() * Math.PI * 2;
    const [c, s] = [Math.cos(ra), Math.sin(ra)];
    const q = (u: number, k: number): [number, number] => [rx + c * u - s * k, ry + s * u + c * k];
    for (let i = 0; i < ribs; i++) {
      const k = (i - ribs / 2) * 0.09;
      for (const [w, col] of [
        [0.082, INK],
        [0.04, T[1]],
      ] as const) g.moveTo(...q(-0.13, k)).quadraticCurveTo(...q(0, k - 0.14 * (rnd() < 0.5 ? 1 : 0.7)), ...q(0.13, k + 0.02)).stroke({ width: w, color: col, cap: 'round' });
    }
  }
  // A jawbone: a U, in half the piles.
  if (rnd() < 0.55) {
    const [jx, jy] = at(0.3);
    const ja = rnd() * Math.PI * 2;
    const [c, s] = [Math.cos(ja), Math.sin(ja)];
    const q = (u: number, k: number): [number, number] => [jx + c * u - s * k, jy + s * u + c * k];
    for (const [w, col] of [
      [0.09, INK],
      [0.046, T[1]],
    ] as const) g.moveTo(...q(-0.09, -0.07)).quadraticCurveTo(...q(0.15, 0), ...q(-0.09, 0.07)).stroke({ width: w, color: col, cap: 'round' });
  }
  const nl = 2 + Math.floor(rnd() * 4);
  for (let i = 0; i < nl; i++) {
    const [x, y] = at(0.3);
    const a = rnd() * Math.PI;
    const l = 0.08 + 0.08 * rnd();
    const w = 0.05 + 0.02 * rnd();
    castShadow(g, [x - Math.cos(a) * l, y - Math.sin(a) * l, x + Math.cos(a) * l, y + Math.sin(a) * l, x, y + 0.02], light, 0.04);
    bone(g, [x - Math.cos(a) * l, y - Math.sin(a) * l], [x + Math.cos(a) * l, y + Math.sin(a) * l], w, light, OUT, T[1]);
  }
}

/** Adventurer's remains: someone who died where they fell, life-sized (about 5.5 ft, a square is 5): a sprawled
 * skeleton — one arm flung out, the skull rolled aside, a leg bent, a bone dragged off — on a ragged cloak; a
 * rusty sword dropped by the flung hand, an open helm tipped over by the head, a satchel and a pouch spilling
 * coins. */
export function skeletonRemains(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { len, at } = frame(f);
  const o = (len - 1.15) / 2;
  const P = (u: number, s: number) => at(o + u, 0.5 + s);
  const flat = (pts: [number, number][]) => pts.flatMap(([u, s]) => P(u + (rnd() - 0.5) * 0.03, s + (rnd() - 0.5) * 0.03));
  // The cloak: ragged, spread to one side.
  const cloak = flat([[0.1, -0.2], [0.2, -0.26], [0.3, -0.2], [0.42, -0.3], [0.55, -0.24], [0.7, -0.34], [0.78, -0.22], [0.86, -0.12], [0.8, 0.02], [0.72, 0.16], [0.6, 0.22], [0.48, 0.18], [0.36, 0.26], [0.22, 0.2], [0.12, 0.14]]);
  g.poly(cloak).fill({ color: 0x3e3226, alpha: 0.85 }).stroke({ width: 0.018, color: INK, join: 'miter' });
  g.moveTo(...P(0.3, -0.18)).lineTo(...P(0.6, -0.26)).moveTo(...P(0.5, 0.15)).lineTo(...P(0.7, 0.05)).stroke({ width: 0.014, color: 0x5a4a36 });
  // Satchel on its strap, and a pouch with coins spilled from it.
  const [bx, by] = P(0.62, -0.42);
  g.moveTo(...P(0.3, -0.1)).quadraticCurveTo(...P(0.45, -0.42), bx, by).stroke({ width: 0.018, color: 0x5a3a20 });
  const bag = quad([bx - 0.1, by - 0.08, bx + 0.1, by + 0.08], rnd, 0.01);
  g.poly(bag).fill(0x7a5030).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(quad([bx - 0.1, by - 0.08, bx + 0.1, by + 0.0], rnd, 0.008)).fill(0x8e6038).stroke({ width: 0.018, color: INK });
  g.circle(bx, by + 0.0, 0.015).fill(BRASS);
  const [px, py] = P(0.98, 0.3);
  g.circle(px, py, 0.05).fill(0x6a4a2a).stroke({ width: 0.018, color: INK });
  g.circle(px + 0.02, py - 0.03, 0.018).fill(0x4a3220);
  for (let i = 0; i < 6; i++) {
    const [x, y] = P(1.02 + rnd() * 0.12, 0.2 + rnd() * 0.15);
    g.circle(x, y, 0.02).fill(GILT[1]).stroke({ width: 0.014, color: GILT[0] });
  }
  // Legs: one straight, one bent at the knee; the pelvis.
  bone(g, P(0.68, -0.07), P(0.92, -0.09), 0.04, light);
  bone(g, P(0.93, -0.09), P(1.12, -0.07), 0.034, light);
  g.poly(oval(...P(1.15, -0.07), 0.035, 0.025, 0, 8)).fill(BONE[1]).stroke({ width: LINE, color: INK });
  bone(g, P(0.68, 0.07), P(0.86, 0.2), 0.04, light);
  bone(g, P(0.87, 0.21), P(1.06, 0.12), 0.034, light);
  g.poly(oval(...P(1.09, 0.1), 0.035, 0.025, 0.6, 8)).fill(BONE[1]).stroke({ width: LINE, color: INK });
  const pel = [...P(0.58, -0.04), ...P(0.6, -0.13), ...P(0.68, -0.12), ...P(0.7, 0), ...P(0.68, 0.12), ...P(0.6, 0.13), ...P(0.58, 0.04)];
  g.poly(pel).fill(BONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const k of [-1, 1]) g.circle(...P(0.645, k * 0.07), 0.02).fill(0x2a2018);
  // Spine and ribcage, a little twisted.
  g.moveTo(...P(0.12, 0.02)).quadraticCurveTo(...P(0.35, -0.03), ...P(0.6, 0)).stroke({ width: 0.058, color: INK, cap: 'round' });
  g.moveTo(...P(0.12, 0.02)).quadraticCurveTo(...P(0.35, -0.03), ...P(0.6, 0)).stroke({ width: 0.026, color: BONE[1], cap: 'round' });
  for (let i = 0; i < 5; i++) {
    const u = 0.2 + i * 0.05;
    for (const k of [-1, 1]) {
      for (const [w, col] of [
        [0.048, INK],
        [0.02, BONE[1]],
      ] as const) {
        g.moveTo(...P(u, k * 0.015 - 0.015)).quadraticCurveTo(...P(u - 0.02, k * 0.14 - 0.015), ...P(u + 0.05, k * 0.13 - 0.015)).stroke({ width: w, color: col, cap: 'round' });
      }
    }
  }
  // Arms: one by the side, one flung out above the head; the sword by that hand.
  bone(g, P(0.17, -0.15), P(0.4, -0.19), 0.034, light);
  bone(g, P(0.41, -0.19), P(0.58, -0.22), 0.028, light);
  bone(g, P(0.15, 0.15), P(0.0, 0.33), 0.034, light);
  bone(g, P(-0.01, 0.34), P(-0.14, 0.42), 0.028, light);
  for (let j = 0; j < 3; j++) g.circle(...P(-0.18 - j * 0.01, 0.42 + j * 0.025), 0.012).fill(BONE[1]).stroke({ width: 0.018, color: INK });
  const [s0, s1] = [P(-0.3, 0.3), P(0.45, 0.42)];
  g.moveTo(...s0).lineTo(...s1).stroke({ width: 0.056, color: INK, cap: 'butt' });
  g.moveTo(...s0).lineTo(...s1).stroke({ width: 0.034, color: 0x8a7a6a, cap: 'butt' });
  g.moveTo(...P(-0.15, 0.32)).lineTo(...P(0.42, 0.415)).stroke({ width: 0.014, color: 0xb8aea0 });
  for (let i = 0; i < 5; i++) g.circle(...P(-0.1 + rnd() * 0.5, 0.33 + (rnd() * 0.08)), 0.014).fill(0x8a4a22);
  g.moveTo(...P(-0.29, 0.21)).lineTo(...P(-0.32, 0.39)).stroke({ width: 0.045, color: INK, cap: 'round' });
  g.moveTo(...P(-0.29, 0.21)).lineTo(...P(-0.32, 0.39)).stroke({ width: 0.025, color: 0x6a5a40, cap: 'round' });
  // A thigh bone dragged off.
  bone(g, P(0.95, -0.38), P(1.15, -0.28), 0.036, light);
  // The skull, rolled aside and turned; the helm tipped over beside it.
  const [hx, hy] = P(0.02, -0.08);
  const [ux, uy] = at(0, 0);
  const [vx, vy] = at(1, 0);
  skullTop(g, hx, hy, Math.atan2(uy - vy, ux - vx) + 0.9, 0.085, light);
  const [mx, my] = P(-0.08, -0.34);
  const ma = Math.atan2(my - hy, mx - hx);
  g.circle(mx, my, 0.11).fill(IRON[1]).stroke({ width: LINE, color: INK });
  g.circle(mx + light[0] * 0.03, my + light[1] * 0.03, 0.05).fill(IRON[2]);
  // Its open side: a dark crescent rimmed in iron, a nasal bar across it.
  const [ox, oy] = [mx + Math.cos(ma) * 0.05, my + Math.sin(ma) * 0.05];
  g.poly(oval(ox, oy, 0.05, 0.09, ma, 14)).fill(0x16140f).stroke({ width: 0.018, color: IRON[0] });
  g.moveTo(ox - Math.cos(ma) * 0.03, oy - Math.sin(ma) * 0.03).lineTo(ox + Math.cos(ma) * 0.05, oy + Math.sin(ma) * 0.05).stroke({ width: 0.02, color: IRON[2], cap: 'round' });
}

/** Skull pile, from a high angle so the faces show (like the headstones): skulls heaped in rows, fewer to the
 * top, each a pale cranium with dark eye sockets, a nose hole and a row of teeth; a few bones under the heap. */
export function skullPile(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt } = screenAxes(light);
  g.poly(patch(cx, cy, 0.42, 0.42, rnd, 0.1, 18)).fill({ color: 0x000000, alpha: 0.28 });
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    const [x, y] = [cx + Math.cos(a) * 0.32, cy + Math.sin(a) * 0.32];
    bone(g, [x - Math.cos(a + 1.6) * 0.1, y - Math.sin(a + 1.6) * 0.1], [x + Math.cos(a + 1.6) * 0.1, y + Math.sin(a + 1.6) * 0.1], 0.035, light);
  }
  const face = (a: number, b: number, s: number) => {
    const P = (x: number, y: number) => pt(cx, cy, a + x * s, b + y * s);
    const head = [...P(0, 1.1), ...P(0.7, 0.9), ...P(0.95, 0.35), ...P(0.85, -0.15), ...P(0.55, -0.45), ...P(0.45, -0.85), ...P(-0.45, -0.85), ...P(-0.55, -0.45), ...P(-0.85, -0.15), ...P(-0.95, 0.35), ...P(-0.7, 0.9)];
    g.poly(head).fill(BONE[1]).stroke({ width: 0.018, color: INK, join: 'round' });
    g.poly([...P(-0.5, 0.95), ...P(0.3, 1.0), ...P(0.6, 0.6), ...P(-0.2, 0.55)]).fill({ color: BONE[2], alpha: 0.9 });
    for (const k of [-1, 1]) g.poly(oval(...P(k * 0.4, 0.05), s * 0.26, s * 0.24, 0, 10)).fill(0x1a1410);
    g.poly([...P(0, -0.15), ...P(0.12, -0.38), ...P(-0.12, -0.38)]).fill(0x1a1410);
    g.moveTo(...P(-0.35, -0.62)).lineTo(...P(0.35, -0.62)).stroke({ width: 0.016, color: INK });
    for (const k of [-0.2, 0, 0.2]) g.moveTo(...P(k, -0.5)).lineTo(...P(k, -0.75));
    g.stroke({ width: 0.016, color: INK });
  };
  const s = 0.1;
  const rows: [number, number][] = [
    [1, 0.22],
    [2, 0.06],
    [3, -0.1],
    [4, -0.26],
  ];
  for (const [n, b] of rows) for (let i = 0; i < n; i++) face((i - (n - 1) / 2) * s * 1.85 + (rnd() - 0.5) * 0.02, b + (rnd() - 0.5) * 0.02, s);
}

/** Open coffin: a six-sided wooden coffin, wide at the shoulders, its lid shoved down over the foot end and
 * askew; inside, a faded red lining and the bones of whoever lay in it. */
export function openCoffin(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { len, at } = frame(f);
  const shape = (k: number, du = 0, ds = 0) => {
    const pts: number[] = [];
    for (const [u, s] of [[0.12, -0.24], [0.55, -0.38], [len - 0.1, -0.2], [len - 0.1, 0.2], [0.55, 0.38], [0.12, 0.24]]) {
      const [uu, ss] = [0.12 + (u - 0.12) * 1, s * k];
      pts.push(...at(uu + du + (u < 1 ? (1 - k) * 0.12 : -(1 - k) * 0.12), 0.5 + ss + ds));
    }
    return pts;
  };
  const outer = shape(1);
  castShadow(g, outer, light, 0.2);
  g.poly(outer).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(shape(0.86)).fill(WOOD[1]);
  g.poly(shape(0.72)).fill(0x1e1712).stroke({ width: 0.018, color: INK });
  g.poly(shape(0.62)).fill(0x5a2a24);
  g.poly(shape(0.62).map((q, i) => q + (i % 2 ? light[1] : light[0]) * 0.02)).fill(0x7a3a30);
  // Bones in it.
  const [ux, uy] = at(0, 0);
  const [vx, vy] = at(1, 0);
  skullTop(g, ...at(0.36, 0.5), Math.atan2(uy - vy, ux - vx), 0.08, light);
  bone(g, at(0.62, 0.42), at(1.02, 0.46), 0.035, light);
  bone(g, at(0.66, 0.6), at(1.0, 0.54), 0.032, light);
  for (let i = 0; i < 3; i++) {
    const u = 0.55 + i * 0.05;
    g.moveTo(...at(u, 0.44)).quadraticCurveTo(...at(u - 0.02, 0.32), ...at(u + 0.04, 0.3)).stroke({ width: 0.016, color: BONE[1], cap: 'round' });
  }
  // The lid: the lower part of the same shape, pushed down and off to one side.
  const sk = 0.08 + 0.04 * rnd();
  const lid: number[] = [];
  for (const [u, s] of [[1.1, -0.33], [len - 0.1, -0.2], [len - 0.1, 0.2], [1.1, 0.33]]) lid.push(...at(u + (s > 0 ? sk : -sk), 0.5 + s + 0.1));
  castShadow(g, lid, light, 0.05);
  g.poly(lid).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(lid.map((q, i) => q + (i % 2 ? light[1] : light[0]) * 0.015)).fill(WOOD[2]);
  for (const k of [-0.1, 0.1]) g.moveTo(...at(1.12, 0.56 + k * 2.4)).lineTo(...at(len - 0.12, 0.56 + k * 1.6));
  g.stroke({ width: 0.014, color: WOOD[0], alpha: 0.8 });
  g.moveTo(...at(1.4, 0.56)).lineTo(...at(1.7, 0.56)).moveTo(...at(1.5, 0.46)).lineTo(...at(1.5, 0.66)).stroke({ width: 0.025, color: WOOD[0] });
  for (const [u, s] of [[1.15, 0.3], [1.15, 0.82], [len - 0.15, 0.4], [len - 0.15, 0.72]]) g.circle(...at(u, s), 0.012).fill(IRON[0]);
}

/** The mouth of a burial niche that is cut into rock: drawn over the wall line (which runs unbroken under the
 * niche), a dark floor where the wall is open between two stone jambs, so the pocket reads as reachable. */
export function nicheMouth(g: Graphics, f: InteriorItem, light: Light, wall: [number, number]) {
  const { rect } = frame(f, wall);
  const rnd = rng(f.x * 977 + f.y * 131 + 7);
  const j = (n: number) => (rnd() - 0.5) * n;
  const POCKET = [0x211c17, 0x2f2820, 0x40372c];
  const ROCK = [0x4a453d, 0x6b655b, 0x8c867a];
  const u0 = 0.15 + j(0.01);
  const u1 = 0.85 + j(0.01);
  g.poly(quad(rect(u0 + 0.005, -0.1, u1 - 0.005, 0.07), rnd, 0.004)).fill(POCKET[1]);
  for (const [a, b] of [[u0 - 0.075, u0 + 0.005], [u1 - 0.005, u1 + 0.075]]) {
    const r = rect(a, -0.09, b, 0.08);
    g.poly(quad(r, rnd, 0.006)).fill(ROCK[1]).stroke({ width: LINE, color: INK, join: 'round' });
    litEdges(g, r, light, 0.025);
    g.fill({ color: ROCK[2], alpha: 0.9 });
  }
}

/** Burial niche: a slot cut into the rock behind the wall line, so the floor in front stays clear: a dressed stone
 * frame round a dark pocket about 0.75 of a square deep (`rock` squares of rock stand behind the wall: with none, a
 * shallow pocket is drawn on the floor). Four looks by `v`: a shrouded body, bare bones, a stone slab sealing it
 * (cracked in places), empty or holding an urn. */
export function burialNiche(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, wall: [number, number], rock: number) {
  const { rect, at } = frame(f, wall);
  const sg = rock > 0 ? -1 : 1;
  const dd = (rock >= 2 ? 0.74 : rock === 1 ? 0.56 : 0.34) + (rnd() - 0.5) * 0.06;
  // (u along the wall, k from the wall line into the pocket.)
  // With a room behind the wall, the pocket starts at the masonry band's floor face, not under it.
  const off = rock > 0 ? 0 : 0.23;
  const R = (u0: number, k0: number, u1: number, k1: number) => rect(u0, sg * (k0 + off), u1, sg * (k1 + off));
  const A = (u: number, k: number) => at(u, sg * (k + off));
  const POCKET = [0x211c17, 0x2f2820, 0x40372c];
  const ROCK = [0x4a453d, 0x6b655b, 0x8c867a];
  const jw = 0.01 + rnd() * 0.012;
  const u0 = 0.13 + rnd() * 0.02;
  const u1 = 0.87 - rnd() * 0.02;
  // The frame (lit along the sides toward the light), the pocket.
  const fr = R(u0 - 0.07, 0.07, u1 + 0.07, dd + 0.07);
  g.poly(quad(fr, rnd, 0.012)).fill(ROCK[1]).stroke({ width: OUT, color: INK, join: 'round' });
  litEdges(g, fr, light, 0.035);
  g.fill({ color: ROCK[2], alpha: 0.9 });
  g.poly(quad(R(u0, 0.0, u1, dd), rnd, jw)).fill(POCKET[1]).stroke({ width: LINE, color: INK, join: 'round' });
  const inner = R(u0 + 0.02, 0.02, u1 - 0.02, dd - 0.02);
  g.poly(quad(inner, rnd, 0.008)).fill(POCKET[2]);
  const kc = dd / 2 + 0.02;
  const head = rnd() < 0.5 ? 0 : 1;
  const U = (u: number) => (head ? 1 - u : u);
  const hw = Math.min(1, (dd / 2 - 0.06) / 0.19);
  const [ax, ay] = A(0.5, kc);
  const [bx, by] = A(0.5, kc + 1);
  const toward = (bx - ax) * light[0] + (by - ay) * light[1] > 0 ? 1 : -1;
  // The crown of a skull at the head end points out past it.
  const skull = (u: number, s: number) => {
    const [hx, hy] = A(U(u), kc);
    const [tx, ty] = A(U(0), kc);
    const [ex, ey] = A(U(1), kc);
    skullTop(g, hx, hy, Math.atan2(ty - ey, tx - ex), s, light);
  };
  switch (v % 4) {
    case 0: {
      // A body in a ragged shroud, the skull showing in some.
      const peek = rnd() < 0.5;
      const ST = [0x5e5646, 0x867c68, 0xa89e86, 0xc0b69c];
      const prof: [number, number][] = [[0.17, 0.0], [0.2, 0.09], [0.27, 0.12], [0.33, 0.1], [0.38, 0.15], [0.5, 0.19], [0.64, 0.18], [0.76, 0.13], [0.83, 0.08], [0.86, 0.0]];
      const m = (u: number) => U(peek ? 0.32 + (u - 0.17) * 0.77 : u);
      const edge = (sign: number) => prof.flatMap(([u, w]) => A(m(u), kc + sign * w * hw));
      const back = (sign: number) => [...prof].reverse().flatMap(([u, w]) => A(m(u), kc + sign * w * hw));
      g.poly([...edge(-1), ...back(1)]).fill(ST[1]).stroke({ width: LINE, color: INK, join: 'round' });
      const half = (sign: number) => [...edge(sign), ...[...prof].reverse().flatMap(([u]) => A(m(u), kc + sign * 0.01))];
      g.poly(half(toward)).fill({ color: ST[3], alpha: 0.7 });
      g.poly(half(-toward)).fill({ color: ST[0], alpha: 0.55 });
      // The wrapping, a few bands across.
      for (const u of [0.42, 0.54, 0.66, 0.77]) {
        const j = (rnd() - 0.5) * 0.03;
        g.moveTo(...A(m(u + j), kc - 0.17 * hw)).lineTo(...A(m(u + 0.025 + j), kc + 0.17 * hw));
      }
      g.stroke({ width: 0.016, color: ST[0] });
      if (peek) skull(0.22, 0.095);
      // A bone hanging out over the lip in some.
      if (rnd() < 0.4) bone(g, A(m(0.62), kc + 0.12 * hw), A(m(0.7), 0.05), 0.04, light);
      break;
    }
    case 1: {
      // Bare bones: the skull, a spine with ribs, the long bones, a pelvis, a few strays.
      bone(g, A(U(0.3), kc), A(U(0.68), kc), 0.035, light);
      for (const u of [0.38, 0.47, 0.56]) {
        const k = 0.15 * hw * (1 - Math.abs(u - 0.47) * 1.4);
        bone(g, A(U(u), kc - k), A(U(u + 0.02), kc + k), 0.03, light);
      }
      bone(g, A(U(0.72), kc - 0.1 * hw), A(U(0.86), kc - 0.13 * hw + (rnd() - 0.5) * 0.06), 0.04, light);
      bone(g, A(U(0.72), kc + 0.1 * hw), A(U(0.84), kc + 0.15 * hw), 0.04, light);
      bone(g, A(U(0.36), kc - 0.18 * hw), A(U(0.55), kc - 0.2 * hw), 0.032, light);
      g.poly(oval(...A(U(0.7), kc), 0.06, 0.09 * hw + 0.02, 0, 10)).fill(BONE[1]).stroke({ width: LINE, color: INK });
      skull(0.22, 0.1);
      for (let i = 0; i < 3; i++) g.circle(...A(0.25 + rnd() * 0.5, 0.08 + rnd() * (dd - 0.16)), 0.016).fill(BONE[0]);
      break;
    }
    case 2: {
      // Sealed with a slab: pale cut stone, a cross, a name plate or a seam, cracks and a chipped corner in some.
      const SL = [0x5e5a52, 0x8a857a, 0xa8a296];
      const sl = R(u0 - 0.01, -0.03, u1 + 0.01, dd);
      g.poly(quad(sl, rnd, 0.01)).fill(SL[1]).stroke({ width: LINE, color: INK, join: 'round' });
      litEdges(g, sl, light, 0.05);
      g.fill({ color: SL[2], alpha: 0.95 });
      const style = Math.floor(rnd() * 3);
      if (style === 0) {
        g.moveTo(...A(0.5, 0.18)).lineTo(...A(0.5, dd - 0.1));
        g.moveTo(...A(0.4, 0.3 + (dd - 0.5) * 0.3)).lineTo(...A(0.6, 0.3 + (dd - 0.5) * 0.3));
        g.stroke({ width: 0.03, color: SL[0], cap: 'round' });
      } else if (style === 1) {
        g.poly(quad(R(0.34, 0.12, 0.66, dd - 0.14), rnd, 0.008)).fill(SL[2]).stroke({ width: 0.016, color: SL[0] });
        for (const k of [0.3, 0.45, 0.6]) g.moveTo(...A(0.4, k * dd)).lineTo(...A(0.6, k * dd));
        g.stroke({ width: 0.016, color: SL[0] });
      } else {
        g.moveTo(...A(0.5 + (rnd() - 0.5) * 0.3, 0.02)).lineTo(...A(0.5 + (rnd() - 0.5) * 0.3, dd));
        g.stroke({ width: 0.018, color: SL[0] });
      }
      if (rnd() < 0.6) {
        const c0 = 0.2 + rnd() * 0.6;
        g.moveTo(...A(c0, 0.0)).lineTo(...A(c0 + 0.05, dd * 0.25)).lineTo(...A(c0 - 0.04, dd * 0.5)).lineTo(...A(c0 + 0.06, dd * 0.75)).lineTo(...A(c0, dd));
        g.moveTo(...A(c0 + 0.05, dd * 0.25)).lineTo(...A(c0 + 0.17, dd * 0.33));
        g.stroke({ width: 0.02, color: INK, alpha: 0.85, join: 'round' });
      }
      if (rnd() < 0.45) {
        const cu = rnd() < 0.5 ? u0 + 0.01 : u1 - 0.01;
        const dir = cu < 0.5 ? 1 : -1;
        g.poly([...A(cu, dd), ...A(cu + dir * 0.17, dd), ...A(cu + dir * 0.06, dd - 0.15), ...A(cu, dd - 0.1)]).fill(POCKET[0]).stroke({ width: 0.018, color: INK, join: 'round' });
      }
      return;
    }
    default: {
      // Empty (a rag, a bone, dust) or holding an urn.
      if (rnd() < 0.5) {
        const UR = [0x5a4034, 0x82604a, 0xa88466];
        const [ux, uy] = A(0.3 + rnd() * 0.4, kc);
        const r = Math.min(0.15, dd * 0.28);
        g.circle(ux - light[0] * 0.04, uy - light[1] * 0.04, r + 0.012).fill({ color: 0x000000, alpha: 0.35 });
        g.circle(ux, uy, r).fill(UR[0]).stroke({ width: LINE, color: INK });
        g.circle(ux + light[0] * r * 0.28, uy + light[1] * r * 0.28, r * 0.78).fill({ color: UR[1], alpha: 0.95 });
        g.circle(ux + light[0] * r * 0.4, uy + light[1] * r * 0.4, r * 0.4).fill({ color: UR[2], alpha: 0.9 });
        g.circle(ux, uy, r * 0.42).stroke({ width: 0.014, color: UR[0] });
        if (rnd() < 0.5) bone(g, A(0.7 + rnd() * 0.1, kc + 0.12), A(0.84, kc + 0.2), 0.03, light);
      } else {
        const RAG = [0x4e463a, 0x6e6552];
        const [rx, ry] = [0.25 + rnd() * 0.4, kc + (rnd() - 0.5) * 0.1];
        g.poly([...A(rx, ry - 0.08), ...A(rx + 0.12, ry - 0.12), ...A(rx + 0.22, ry - 0.04), ...A(rx + 0.16, ry + 0.09), ...A(rx + 0.04, ry + 0.07)]).fill(RAG[1]).stroke({ width: 0.018, color: INK, join: 'round' });
        g.moveTo(...A(rx + 0.04, ry)).lineTo(...A(rx + 0.15, ry - 0.02)).stroke({ width: 0.014, color: RAG[0] });
        for (let i = 0; i < 4; i++) g.circle(...A(0.2 + rnd() * 0.6, 0.1 + rnd() * (dd - 0.2)), 0.014 + rnd() * 0.012).fill({ color: POCKET[0], alpha: 0.9 });
        if (rnd() < 0.5) bone(g, A(0.62, kc + 0.12), A(0.78, kc + 0.16), 0.032, light);
      }
      break;
    }
  }
  // The wall on the lit side throws its shadow into the pocket.
  litEdges(g, inner, light, 0.07);
  g.fill({ color: 0x000000, alpha: 0.4 });
}

/** Burial urn, from high up (from straight above it is only rings, from the side it stands too tall): seen from
 * about 60 degrees, so its round cross-sections show as fat ovals — a narrow foot, a round belly with a painted band
 * round its front, loop handles at the shoulders, a domed lid with a knob on top; lit from the upper left;
 * terracotta, bronze or grey stone. */
export function burialUrn(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const t = [
    [0x6a321c, 0xa0542e, 0xc87a4c, 0x2a1a12],
    [0x5a4220, 0x9a7438, 0xd0a85a, 0x3a2a14],
    [0x5e5a52, 0x8a857a, 0xb0aa9e, 0x3a3630],
    [0x6a321c, 0xa0542e, 0xc87a4c, 0xe8dcc0],
  ][v % 4];
  const { pt, right } = screenAxes(light);
  const rot = Math.atan2(right[1], right[0]);
  // Height shows at half, depth at nearly full: a ring of radius w at height h is an oval w by w * 0.87.
  const [hk, dk] = [0.5, 0.87];
  const at = (a: number, h: number) => pt(cx, cy, a, h * hk);
  const ell = (h: number, w: number, a = 0, k = 1) => oval(...at(a, h), w * k, w * dk * k, rot, 20);
  // The profile, foot to lip: (height, radius).
  const prof: [number, number][] = [[-0.36, 0.12], [-0.31, 0.11], [-0.27, 0.08], [-0.2, 0.17], [-0.1, 0.25], [0.0, 0.27], [0.1, 0.23], [0.18, 0.14], [0.23, 0.1], [0.26, 0.13]];
  castShadow(g, ell(-0.36, 0.14), light, 0.35);
  // Handles, behind the body's outline.
  for (const k of [-1, 1]) {
    for (const [w, c] of [
      [0.06, INK],
      [0.034, k < 0 ? t[1] : t[0]],
    ] as const) {
      g.moveTo(...at(k * 0.2, 0.14)).bezierCurveTo(...at(k * 0.34, 0.2), ...at(k * 0.37, 0.02), ...at(k * 0.26, -0.04)).stroke({ width: w, color: c, cap: 'round' });
    }
  }
  // The body as the union of its rings: an inked rim, the shaded body, the lit side shifted to the upper left.
  for (const [h, w] of prof) g.poly(ell(h, w + 0.014));
  g.fill(INK);
  for (const [h, w] of prof) g.poly(ell(h, w));
  g.fill(t[0]);
  for (const [h, w] of prof) g.poly(ell(h + 0.01, w, -w * 0.1, 0.9));
  g.fill(t[1]);
  g.poly(ell(0.04, 0.27, -0.09, 0.3)).fill({ color: t[2], alpha: 0.9 });
  // The painted band round the belly's near side, dotted.
  const band = (h: number) => {
    const pts: [number, number][] = [];
    for (let i = 0; i <= 16; i++) {
      const a = Math.PI * (i / 16);
      pts.push(pt(cx, cy, -Math.cos(a) * 0.265, h * hk - Math.sin(a) * 0.265 * dk));
    }
    return pts;
  };
  for (const h of [-0.06, 0.04]) {
    const b = band(h);
    g.moveTo(...b[0]);
    for (const q of b.slice(1)) g.lineTo(...q);
  }
  g.stroke({ width: 0.016, color: t[3] });
  const mid = band(-0.01);
  for (let i = 2; i < 16; i += 3) g.circle(...mid[i], 0.013).fill(t[3]);
  // The lid: the mouth's rim, a dome and a knob, seen mostly from above.
  g.poly(ell(0.27, 0.135)).fill(t[0]).stroke({ width: 0.018, color: INK });
  g.poly(ell(0.3, 0.115)).fill(t[1]).stroke({ width: 0.018, color: INK });
  g.poly(ell(0.32, 0.07, -0.02, 1)).fill({ color: t[2], alpha: 0.6 });
  g.poly(ell(0.36, 0.035)).fill(t[2]).stroke({ width: 0.018, color: INK });
}

/** Offering bowl: a wide shallow bowl on the floor heaped with offerings — coins, a red fruit, petals — two
 * candle stubs beside it, petals strewn round, a wisp of incense. */
export function offeringBowl(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const petal = [0xc84a4a, 0xe8c040, 0xe8e0f0, 0xc84a4a][v % 4];
  for (let i = 0; i < 10; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.3 + 0.12 * rnd();
    g.poly(oval(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.022, 0.012, rnd() * Math.PI, 8)).fill(petal).stroke({ width: 0.018, color: INK, alpha: 0.6 });
  }
  const r = 0.26;
  castShadow(g, ring(cx, cy, r, rnd, 0.004, 20), light, 0.12);
  g.circle(cx, cy, r).fill(BRONZE[0]).stroke({ width: OUT, color: INK });
  g.circle(cx + light[0] * 0.015, cy + light[1] * 0.015, r * 0.92).fill(BRONZE[1]);
  g.circle(cx - light[0] * 0.02, cy - light[1] * 0.02, r * 0.76).fill(BRONZE[0]).stroke({ width: 0.018, color: INK });
  // Offerings.
  for (let i = 0; i < 7; i++) {
    const [x, y] = [cx + (rnd() - 0.5) * 0.24, cy + (rnd() - 0.5) * 0.24];
    g.circle(x, y, 0.035).fill(GILT[1]).stroke({ width: 0.014, color: GILT[0] });
    g.circle(x + light[0] * 0.01, y + light[1] * 0.01, 0.015).fill(GILT[2]);
  }
  const [fx, fy] = [cx + 0.06, cy - 0.05];
  g.circle(fx, fy, 0.07).fill(0x9a2a2a).stroke({ width: 0.018, color: INK });
  g.circle(fx + light[0] * 0.025, fy + light[1] * 0.025, 0.025).fill(0xe87a6a);
  g.moveTo(fx, fy).lineTo(fx + 0.02, fy - 0.03).stroke({ width: 0.014, color: 0x3a5a22 });
  for (let i = 0; i < 4; i++) g.poly(oval(cx - 0.08 + i * 0.03, cy + 0.08 - i * 0.02, 0.025, 0.014, i, 8)).fill(petal).stroke({ width: 0.018, color: INK, alpha: 0.6 });
  for (const k of [-1, 1]) candle(g, cx + k * 0.36, cy + k * 0.12);
  // Incense: a thin curling wisp.
  g.moveTo(cx - 0.1, cy - 0.05).bezierCurveTo(cx - 0.2, cy - 0.15, cx - 0.02, cy - 0.22, cx - 0.12, cy - 0.34).stroke({ width: 0.018, color: 0xe8e8ec, alpha: 0.5, cap: 'round' });
}

export const BURLAP = [0x7a6440, 0xa88c5c, 0xc8ac78];

/** Cold campfire: a ring of fire-blackened stones round grey ash, charred logs crossed in it with grey-white ash on
 * their ends; a forked stick either side holding a spit across. Cold: no glow. */
export function coldCampfire(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx, cy, 0.3, 0.3, rnd, 0.1, 16)).fill(0x3a3632);
  g.poly(patch(cx, cy, 0.24, 0.24, rnd, 0.14, 14)).fill(0x8a8680);
  for (let i = 0; i < 10; i++) g.circle(cx + (rnd() - 0.5) * 0.36, cy + (rnd() - 0.5) * 0.36, 0.02 + 0.02 * rnd()).fill({ color: 0xb8b4ae, alpha: 0.7 });
  // Charred logs, crossed.
  const ph = rnd() * Math.PI;
  for (let i = 0; i < 3; i++) {
    const a = ph + (i * Math.PI) / 3;
    const [dx, dy] = [Math.cos(a) * 0.2, Math.sin(a) * 0.2];
    for (const [w, c] of [
      [0.085, INK],
      [0.06, 0x2a221c],
    ] as const) {
      g.moveTo(cx - dx, cy - dy).lineTo(cx + dx, cy + dy).stroke({ width: w, color: c, cap: 'round' });
    }
    g.moveTo(cx - dx * 0.6, cy - dy * 0.6).lineTo(cx + dx * 0.6, cy + dy * 0.6).stroke({ width: 0.014, color: 0x4a3e34, cap: 'round' });
    for (const k of [-1, 1]) g.circle(cx + dx * k, cy + dy * k, 0.026).fill(0xc8c4bc);
  }
  // The stone ring.
  for (let i = 0; i < 11; i++) {
    const a = (i / 11) * Math.PI * 2 + rnd() * 0.2;
    rock(g, cx + Math.cos(a) * 0.32, cy + Math.sin(a) * 0.32, 0.06 + 0.025 * rnd(), [0x3a3632, 0x5e5a54, 0x8a857c, 0xa8a296], light, rnd, 6);
  }
  // The spit on its forked sticks.
  const sa = ph + Math.PI / 2;
  const [ex, ey] = [Math.cos(sa) * 0.42, Math.sin(sa) * 0.42];
  for (const k of [-1, 1]) {
    g.circle(cx + ex * k, cy + ey * k, 0.04).fill(WOOD[1]).stroke({ width: 0.018, color: INK });
  }
  g.moveTo(cx - ex * 1.05, cy - ey * 1.05).lineTo(cx + ex * 1.05, cy + ey * 1.05).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.moveTo(cx - ex * 1.05, cy - ey * 1.05).lineTo(cx + ex * 1.05, cy + ey * 1.05).stroke({ width: 0.022, color: WOOD[2], cap: 'round' });
}

/** Rubble: a spill of broken rock over the whole square — a couple of big angular chunks throwing shadows,
 * medium ones, many small chips — on a dark fan of grit, a crack in the floor beside it. */
export function rubbleHeap(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const DARK = [0x3a342c, 0x564e42, 0x766c5c, 0x948874];
  g.poly(patch(cx, cy, 0.48, 0.44, rnd, 0.25, 22)).fill({ color: 0x2e2a24, alpha: 0.35 });
  const a = rnd() * Math.PI * 2;
  g.moveTo(cx + Math.cos(a) * 0.46, cy + Math.sin(a) * 0.46)
    .lineTo(cx + Math.cos(a) * 0.3 + 0.04, cy + Math.sin(a) * 0.3 - 0.03)
    .lineTo(cx + Math.cos(a) * 0.2, cy + Math.sin(a) * 0.2)
    .stroke({ width: 0.016, color: INK, alpha: 0.7, join: 'miter' });
  for (let i = 0; i < 36; i++) {
    const b = rnd() * Math.PI * 2;
    const d = 0.45 * Math.sqrt(rnd());
    const [x, y] = [cx + Math.cos(b) * d, cy + Math.sin(b) * d];
    const r = 0.01 + 0.012 * rnd();
    g.poly([x - r, y, x, y - r * 0.8, x + r * 0.9, y + r * 0.2, x, y + r]).fill(i % 3 ? DARK[1] : DARK[2]);
  }
  const chunks: [number, number, number][] = [];
  for (let i = 0; i < 2; i++) chunks.push([cx + (rnd() - 0.5) * 0.3, cy + (rnd() - 0.5) * 0.3, 0.13 + 0.05 * rnd()]);
  for (let i = 0; i < 5; i++) {
    const b = rnd() * Math.PI * 2;
    const d = 0.18 + 0.18 * rnd();
    chunks.push([cx + Math.cos(b) * d, cy + Math.sin(b) * d, 0.06 + 0.035 * rnd()]);
  }
  for (let i = 0; i < 10; i++) {
    const b = rnd() * Math.PI * 2;
    const d = 0.1 + 0.32 * rnd();
    chunks.push([cx + Math.cos(b) * d, cy + Math.sin(b) * d, 0.025 + 0.02 * rnd()]);
  }
  for (const [x, y, s] of chunks) if (s > 0.1) castShadow(g, patch(x, y, s, s, rnd, 0.2, 6), light, 0.25);
  chunks.sort((p, q) => p[2] - q[2]);
  for (const [x, y, s] of chunks) rock(g, x, y, s, DARK, light, rnd, s > 0.1 ? 5 : 4 + Math.floor(rnd() * 2));
}

/** Flotsam heap: what the water left — two broken planks, a split barrel hoop, a sodden rag, a bottle, sticks — in
 * a slick of greenish mud. */
export function flotsamHeap(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx, cy, 0.42, 0.38, rnd, 0.2, 18)).fill({ color: 0x3a3e24, alpha: 0.45 });
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI;
    const [x, y] = [cx + (rnd() - 0.5) * 0.6, cy + (rnd() - 0.5) * 0.6];
    const l = 0.1 + 0.1 * rnd();
    for (const [w, c] of [
      [0.04, INK],
      [0.022, 0x5a4428],
    ] as const) {
      g.moveTo(x - Math.cos(a) * l, y - Math.sin(a) * l).lineTo(x + Math.cos(a) * l, y + Math.sin(a) * l).stroke({ width: w, color: c, cap: 'round' });
    }
  }
  // A rag.
  const rag = patch(cx - 0.2, cy + 0.18, 0.13, 0.09, rnd, 0.35, 10);
  g.poly(rag).fill(0x7a6e5a).stroke({ width: 0.018, color: INK, join: 'round' });
  g.moveTo(cx - 0.28, cy + 0.16).lineTo(cx - 0.12, cy + 0.22).stroke({ width: 0.014, color: 0x5a5040 });
  // Broken planks, a jagged end each.
  const plank = (x: number, y: number, a: number, l: number, w: number) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const P = (u: number, k: number): [number, number] => [x + c * u - s * k, y + s * u + c * k];
    const pts = [...P(-l, -w), ...P(l * 0.7, -w), ...P(l * 0.85, -w * 0.4), ...P(l, -w * 0.1), ...P(l * 0.8, w * 0.3), ...P(l * 0.92, w), ...P(-l, w)];
    castShadow(g, pts, light, 0.05);
    g.poly(pts).fill(0x7a5a3c).stroke({ width: 0.02, color: INK, join: 'miter' });
    g.poly([...P(-l + 0.02, -w + 0.02), ...P(l * 0.6, -w + 0.02), ...P(l * 0.6, -w * 0.1), ...P(-l + 0.02, -w * 0.1)]).fill(0x8e6c48);
    g.moveTo(...P(-l * 0.9, w * 0.3)).lineTo(...P(l * 0.6, w * 0.35)).stroke({ width: 0.014, color: 0x4a3828 });
    for (const k of [-0.5, 0.5]) g.circle(...P(-l * 0.8, w * k), 0.014).fill(IRON[0]);
  };
  plank(cx - 0.04, cy - 0.06, rnd() * Math.PI, 0.3, 0.06);
  plank(cx + 0.08, cy + 0.1, rnd() * Math.PI, 0.24, 0.055);
  // A split hoop.
  const [ox, oy] = [cx + 0.22, cy - 0.22];
  g.moveTo(ox + 0.13, oy).arc(ox, oy, 0.13, 0, Math.PI * 1.5).stroke({ width: 0.04, color: INK });
  g.moveTo(ox + 0.13, oy).arc(ox, oy, 0.13, 0, Math.PI * 1.5).stroke({ width: 0.022, color: IRON[1] });
  // A bottle.
  const ba = rnd() * Math.PI * 2;
  const [bx, by] = [cx - 0.24, cy - 0.22];
  g.poly(oval(bx, by, 0.09, 0.045, ba, 12)).fill(0x3a6a4a).stroke({ width: 0.018, color: INK });
  for (const [w, c] of [
    [0.045, INK],
    [0.028, 0x3a6a4a],
  ] as const) {
    g.moveTo(bx + Math.cos(ba) * 0.08, by + Math.sin(ba) * 0.08).lineTo(bx + Math.cos(ba) * 0.15, by + Math.sin(ba) * 0.15).stroke({ width: w, color: c, cap: 'round' });
  }
  g.poly(oval(bx - Math.cos(ba) * 0.02 + light[0] * 0.015, by - Math.sin(ba) * 0.02 + light[1] * 0.015, 0.04, 0.012, ba, 8)).fill({ color: 0xffffff, alpha: 0.6 });
}

/** Timber prop: a mine set seen from above — two squared posts, their ends showing either side of the rough-hewn
 * cap beam laid across them, axe facets along the cap, wedges driven in over the posts, a long shadow (it holds
 * up the roof). */
export function timberProp(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const ba = Math.floor(rnd() * 2) * (Math.PI / 2);
  const [c, s] = [Math.cos(ba), Math.sin(ba)];
  const P = (u: number, k: number): [number, number] => [cx + c * u - s * k, cy + s * u + c * k];
  const sq = (u: number, k: number, h: number) => [...P(u - h, k - h), ...P(u + h, k - h), ...P(u + h, k + h), ...P(u - h, k + h)];
  const cap = [...P(-0.47, -0.06), ...P(0.47, -0.065), ...P(0.48, 0.06), ...P(-0.46, 0.065)];
  for (const u of [-0.32, 0.32]) castShadow(g, sq(u, 0, 0.15), light, 0.6);
  castShadow(g, cap, light, 0.3);
  // The posts.
  for (const u of [-0.32, 0.32]) {
    const post = sq(u, 0, 0.15);
    g.poly(post).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
    for (const r of [0.04, 0.08, 0.12]) g.poly(oval(...P(u, 0), r, r, ba, 14)).stroke({ width: 0.014, color: WOOD[0] });
    g.circle(...P(u, 0), 0.015).fill(WOOD[0]);
  }
  // The cap: hewn, with axe facets down its length.
  g.poly(cap).fill(WOOD[0]).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly([...P(-0.45, -0.045), ...P(0.45, -0.05), ...P(0.46, 0.015), ...P(-0.44, 0.02)]).fill(WOOD[2]);
  for (let i = 0; i < 7; i++) {
    const u = -0.4 + i * 0.13 + (rnd() - 0.5) * 0.04;
    g.moveTo(...P(u, -0.045)).lineTo(...P(u + 0.04, 0.015));
  }
  g.stroke({ width: 0.014, color: WOOD[0], alpha: 0.8 });
  g.moveTo(...P(-0.44, 0.035)).lineTo(...P(0.45, 0.03)).stroke({ width: 0.014, color: shade(WOOD[0], 0.8) });
  // Wedges over the posts.
  for (const u of [-0.32, 0.32]) {
    g.poly([...P(u - 0.06, -0.06), ...P(u + 0.06, -0.06), ...P(u + 0.04, -0.12), ...P(u - 0.04, -0.12)]).fill(WOOD[2]).stroke({ width: 0.018, color: INK, join: 'round' });
  }
}
