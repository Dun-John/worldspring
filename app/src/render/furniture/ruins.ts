import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { BONE, DARK_WOOD, INK, IRON, LINE, LINEN, type Light, OUT, WOOD, board, bone, castShadow, frame, glowPool, litEdges, oval, patch, quad, ring, shade, skullTop } from './common';
import { MARBLE, candle } from './sacred';
import { BRONZE } from './tables';
import { END_GAP, STONE, WALL_GAP } from './home';
import { inkLine } from './hall';
import { rock } from './under';

// R5 underground (item 117): a boss chamber's props.

/** Dressed stone of ruins (the collapsed wall's): shade, face, lit. */
const RUIN = [0x6e685d, 0xb9b1a0, 0xe0d9c8];
/** The same stone tumbled into rubble, in the floor's dust: a shade darker. */
const RUBBLE = RUIN.map((c) => shade(c, 0.8));
/** A worn madder cushion: the throne's one accent. */
const MADDER = [0x5e2420, 0x8e3428, 0xb05a44];
/** Pale violet crystal, for an arcane focus: shade, face, lit, glint. */
const ARCANE = [0x5a4a7a, 0x8a7ab0, 0xb8acd8, 0xe8e2f8];

/** Which way along `s` (across a frame) faces the light: +1 or -1. */
function litAcross(at: (u: number, s: number) => [number, number], light: Light): number {
  const [ax, ay] = at(0, 0);
  const [bx, by] = at(0, 1);
  return (bx - ax) * light[0] + (by - ay) * light[1] > 0 ? 1 : -1;
}

/** Stone throne: a heavy seat hewn from grey stone against `back` — a broad block seat, thick arms ending in round
 * scrolls, a high slab back with a skull boss carved in its top and knobbed corners, a worn red cushion. */
export function stoneThrone(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  castShadow(g, quad(rect(0.06, 0.04, 0.94, 0.92), rnd, 0), light, 0.8);
  board(g, rect(0.12, 0.2, 0.88, 0.92), STONE.map((c) => shade(c, 0.92)), light, rnd, true, 0.05, 0);
  // The cushion.
  const [qx0, qy0, qx1, qy1] = rect(0.3, 0.34, 0.7, 0.84);
  const [lx, ly] = light;
  g.roundRect(qx0, qy0, qx1 - qx0, qy1 - qy0, 0.06).fill(MADDER[0]).stroke({ width: LINE, color: INK });
  g.roundRect(qx0 + 0.025 + lx * 0.015, qy0 + 0.025 + ly * 0.015, qx1 - qx0 - 0.05, qy1 - qy0 - 0.05, 0.05).fill(MADDER[1]);
  g.roundRect(qx0 + 0.06 + lx * 0.03, qy0 + 0.06 + ly * 0.03, (qx1 - qx0) * 0.34, (qy1 - qy0) * 0.26, 0.03).fill({ color: MADDER[2], alpha: 0.6 });
  // Arms, their scrolled fronts.
  for (const [u0, u1] of [[0.07, 0.3], [0.7, 0.93]]) {
    board(g, rect(u0, 0.18, u1, 0.94), STONE, light, rnd, false, 0.04, 0);
    const [sx, sy] = at((u0 + u1) / 2, 0.82);
    g.circle(sx, sy, 0.06).fill(STONE[1]).stroke({ width: LINE, color: INK });
    g.circle(sx + lx * 0.015, sy + ly * 0.015, 0.03).fill(STONE[2]);
  }
  // The high back, its corner knobs and the boss.
  board(g, rect(0.03, 0.02, 0.97, 0.27), STONE.map((c) => shade(c, 1.06)), light, rnd, true, 0.05, 0);
  for (const u of [0.1, 0.9]) {
    const [kx, ky] = at(u, 0.14);
    g.circle(kx, ky, 0.07).fill(STONE[1]).stroke({ width: LINE, color: INK });
    g.circle(kx + lx * 0.02, ky + ly * 0.02, 0.038).fill(STONE[2]);
  }
  const [bx, by] = at(0.5, 0.14);
  skullTop(g, bx, by, Math.atan2(back[1], back[0]), 0.075, light);
}

/** Stone steps up a raised floor, the way up toward `back`: four treads between low cheek blocks, each higher tread
 * paler, a shadow under each nose when the light falls from above the flight, a chipped joint in each tread. */
export function stoneSteps(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  g.poly(quad(rect(0.04, 0.0, 0.96, 0.97), rnd, 0.008)).fill(STONE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const fromAbove = Math.max(0, back[0] * light[0] + back[1] * light[1]);
  const n = 4;
  const [s0, s1] = [0.02, 0.95];
  const d = (s1 - s0) / n;
  for (let k = 0; k < n; k++) {
    const [a, b] = [s0 + k * d, s0 + (k + 1) * d];
    const tone = shade(STONE[1], 1.22 - k * 0.12);
    g.poly(quad(rect(0.15, a, 0.85, b - 0.01), rnd, 0.006)).fill(tone);
    g.poly(quad(rect(0.15, a, 0.85, a + 0.035), rnd, 0)).fill(shade(tone, 1.08));
    if (k > 0) g.poly(quad(rect(0.15, a, 0.85, a + 0.03 + 0.05 * fromAbove), rnd, 0)).fill({ color: 0x14100c, alpha: 0.12 + 0.12 * fromAbove });
    const ju = 0.3 + 0.4 * rnd();
    g.moveTo(...at(ju, a + 0.02)).lineTo(...at(ju + (rnd() - 0.5) * 0.03, b - 0.02)).stroke({ width: 0.016, color: STONE[0] });
    if (k < n - 1) g.moveTo(...at(0.15, b)).lineTo(...at(0.85, b)).stroke({ width: LINE, color: INK });
  }
  // The cheek blocks either side.
  for (const [u0, u1] of [[0.04, 0.15], [0.85, 0.96]]) {
    g.poly(quad(rect(u0, 0.0, u1, 0.97), rnd, 0.004)).fill(STONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(quad(rect(u0 + 0.025, 0.03, u1 - 0.025, 0.4), rnd, 0.004)).fill({ color: STONE[2], alpha: 0.7 });
  }
}

/** Ladder seen from straight above, lying toward `back` (the way up): two parallel rails the same width end to end,
 * four evenly spaced rungs, the rail tops running past the last rung to the lip, a short shadow beside it. */
export function climbLadder(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { at } = frame(f, back);
  const [lx, ly] = light;
  const rail = (side: number, s: number): [number, number] => at(0.5 + side * 0.17, s);
  const [s0, s1] = [-0.04, 0.95];
  const rungs = [0.12, 0.37, 0.62, 0.87];
  // The shadow: the same lines moved a little away from the light.
  const sh = (p: [number, number]): [number, number] => [p[0] - lx * 0.07, p[1] - ly * 0.07];
  for (const side of [-1, 1]) g.moveTo(...sh(rail(side, s1))).lineTo(...sh(rail(side, s0)));
  for (const s of rungs) g.moveTo(...sh(rail(-1, s))).lineTo(...sh(rail(1, s)));
  g.stroke({ width: 0.05, color: 0x14100c, alpha: 0.22, cap: 'round' });
  const d = 0.014;
  const line = (a: [number, number], b: [number, number], w: number, color: number, ox = 0, oy = 0) =>
    g.moveTo(a[0] + ox, a[1] + oy).lineTo(b[0] + ox, b[1] + oy).stroke({ width: w, color, cap: 'round' });
  for (const side of [-1, 1]) {
    const [a, b] = [rail(side, s0), rail(side, s1)];
    line(a, b, 0.1, INK);
    line(a, b, 0.05, WOOD[1]);
    line(a, b, 0.016, WOOD[2], lx * d, ly * d);
  }
  for (const s of rungs) {
    const [a, b] = [rail(-1, s), rail(1, s)];
    line(a, b, 0.08, INK);
    line(a, b, 0.04, WOOD[2]);
  }
  // Scuffs where the feet stand.
  for (const side of [-1, 1]) g.poly(oval(...rail(side, s0), 0.06, 0.04, rnd() * Math.PI, 8)).fill({ color: 0x14100c, alpha: 0.18 });
}

/** Broken wall: a stretch of dressed masonry along the long side, its top broken down to a few heights (the highest
 * palest and set off most toward the light, an ink step where it drops), ragged ends, the stones of the top course
 * showing; rubble and dust tumbled at its foot, mostly on one side. Look 1 has a breach down to its footing. */
export function wallStub(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { len, at } = frame(f);
  const [lx, ly] = light;
  const [s0, s1] = [0.31, 0.69];
  const [u0, u1] = [0.1, len - 0.1];
  const side = rnd() < 0.5 ? -1 : 1;
  // Dust at the foot, on the side most rubble fell.
  const dust: number[] = [];
  const steps = Math.round(len * 8);
  const edge = side > 0 ? s1 : s0;
  for (let k = 0; k <= steps; k++) dust.push(...at(u0 + ((u1 - u0) * k) / steps, edge));
  for (let k = steps; k >= 0; k--) dust.push(...at(u0 + ((u1 - u0) * k) / steps, edge + side * (0.16 + 0.12 * Math.sin((k / steps) * Math.PI) * (0.7 + 0.5 * rnd()))));
  g.poly(dust).fill({ color: 0xaaa392, alpha: 0.45 });
  // The stretches of the top, each its own height.
  const cuts = [u0];
  while (cuts[cuts.length - 1] < u1 - 0.5) cuts.push(Math.min(u1 - 0.3, cuts[cuts.length - 1] + 0.38 + 0.32 * rnd()));
  cuts.push(u1);
  const nSeg = cuts.length - 1;
  const heights = Array.from({ length: nSeg }, () => [1, 0.75, 0.5][Math.floor(rnd() * 3)]);
  heights[Math.floor(rnd() * nSeg)] = 1;
  if (v % 2 === 1 && nSeg >= 3) heights[1 + Math.floor(rnd() * (nSeg - 2))] = 0.15;
  // The wall's foot: ragged ends.
  const body: number[] = [];
  const jag = (u: number, dir: number, from: number, to: number) => {
    for (const k of [0.25, 0.5, 0.75]) body.push(...at(u + dir * (rnd() - 0.4) * 0.08, from + (to - from) * k));
  };
  for (let k = 0; k <= 8; k++) body.push(...at(u0 + ((u1 - u0) * k) / 8, s0 + (k % 8 ? (rnd() - 0.5) * 0.012 : 0)));
  jag(u1, 1, s0, s1);
  for (let k = 8; k >= 0; k--) body.push(...at(u0 + ((u1 - u0) * k) / 8, s1 + (k % 8 ? (rnd() - 0.5) * 0.012 : 0)));
  jag(u0, -1, s1, s0);
  castShadow(g, body, light, 0.9);
  g.poly(body).fill(RUIN[0]).stroke({ width: OUT, color: INK, join: 'round' });
  for (let i = 0; i < nSeg; i++) {
    const h = heights[i];
    const [a, b] = [cuts[i] + (i === 0 ? 0.06 : 0.01), cuts[i + 1] - (i === nSeg - 1 ? 0.06 : 0.01)];
    const [ox, oy] = [lx * 0.035 * h, ly * 0.035 * h];
    const face = [...at(a, s0 + 0.03), ...at(b, s0 + 0.03), ...at(b, s1 - 0.03), ...at(a, s1 - 0.03)].map((p, j) => p + (j % 2 ? oy : ox));
    const tone = shade(RUIN[1], 0.8 + 0.2 * h);
    g.poly(face).fill(tone);
    if (h < 0.3) {
      for (let j = 0; j < 3; j++) rock(g, ...at(a + (b - a) * (0.2 + 0.6 * rnd()), s0 + 0.08 + 0.22 * rnd()), 0.045 + 0.02 * rnd(), RUBBLE, light, rnd, 6);
      continue;
    }
    // The top course: stones two across, joints staggered.
    const P = (u: number, s: number): [number, number] => {
      const [px, py] = at(u, s);
      return [px + ox, py + oy];
    };
    g.moveTo(...P(a + 0.02, 0.5)).lineTo(...P(b - 0.02, 0.5));
    for (let u = a + 0.12 + 0.08 * rnd(); u < b - 0.06; u += 0.2 + 0.08 * rnd()) {
      const r = rnd() < 0.5;
      g.moveTo(...P(u, r ? s0 + 0.03 : 0.5)).lineTo(...P(u, r ? 0.5 : s1 - 0.03));
    }
    g.stroke({ width: 0.016, color: RUIN[0], alpha: 0.9 });
    // A lit edge on the top.
    const sl = litAcross(at, light);
    g.moveTo(...P(a + 0.02, sl > 0 ? s1 - 0.045 : s0 + 0.045)).lineTo(...P(b - 0.02, sl > 0 ? s1 - 0.045 : s0 + 0.045)).stroke({ width: 0.02, color: RUIN[2], alpha: 0.7 });
    // Where it drops to a lower stretch, a step.
    for (const [j, u] of [[i - 1, a], [i + 1, b]] as const) {
      if (j < 0 || j >= nSeg || heights[j] >= h) continue;
      g.moveTo(...P(u, s0 + 0.03)).lineTo(...P(u, s1 - 0.03)).stroke({ width: LINE, color: INK });
    }
  }
  // Rubble at its foot: more on the dusty side.
  for (let i = 0; i < Math.round(len * 3); i++) {
    const sd = rnd() < 0.75 ? side : -side;
    const s = (sd > 0 ? s1 : s0) + sd * (0.07 + 0.17 * rnd());
    rock(g, ...at(u0 + (u1 - u0) * rnd(), s), 0.04 + 0.04 * rnd(), RUBBLE, light, rnd, 6);
  }
  for (let i = 0; i < Math.round(len * 4); i++) {
    const sd = rnd() < 0.7 ? side : -side;
    g.circle(...at(u0 + (u1 - u0) * rnd(), (sd > 0 ? s1 : s0) + sd * (0.05 + 0.25 * rnd())), 0.018 + 0.012 * rnd()).fill(RUIN[0]).stroke({ width: 0.016, color: INK });
  }
}

/** Fallen column: a toppled column lying along the long side in its drums, each pushed a little off line and
 * turned, rounded toward the light with a shaded side away from it, flutes along it; its capital at one end (the
 * square abacus and the flared echinus), the last drum broken off ragged; chips about. */
export function fallenColumn(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { len, at } = frame(f);
  const T = MARBLE;
  const r = 0.29;
  const lit = litAcross(at, light);
  const shapes: number[][] = [];
  const P = (uc: number, sc: number, t: number) => (du: number, ds: number): [number, number] => at(uc + du * Math.cos(t) - ds * Math.sin(t), sc + du * Math.sin(t) + ds * Math.cos(t));
  // The drums.
  const start = 0.5;
  const n = 3;
  const dl = (len - 0.08 - start) / n;
  const drums: [number, number, number, number][] = [];
  for (let i = 0; i < n; i++) {
    const uc = start + dl * (i + 0.5) + (i ? (rnd() - 0.3) * 0.05 : 0);
    drums.push([uc, 0.5 + (i ? (rnd() - 0.5) * 0.12 : 0), (rnd() - 0.5) * (i ? 0.16 : 0.04), dl / 2 - 0.025]);
  }
  const drumPoly = ([uc, sc, t, hl]: [number, number, number, number], broken: boolean) => {
    const p = P(uc, sc, t);
    const pts = [...p(-hl, -r), ...p(hl, -r)];
    if (broken) for (const k of [-0.5, 0, 0.5]) pts.push(...p(hl - 0.04 - 0.06 * rnd(), k * r));
    pts.push(...p(hl, r), ...p(-hl, r));
    return pts;
  };
  drums.forEach((d, i) => shapes.push(drumPoly(d, i === n - 1)));
  // The capital: echinus, then the abacus at the end.
  const cp = P(0.06, 0.5, (rnd() - 0.5) * 0.06);
  const echinus = [...cp(0.18, -0.4), ...cp(0.42, -r), ...cp(0.42, r), ...cp(0.18, 0.4)];
  const abacus = [...cp(0.0, -0.44), ...cp(0.18, -0.44), ...cp(0.18, 0.44), ...cp(0.0, 0.44)];
  for (const s of [...shapes, echinus, abacus]) castShadow(g, s, light, 0.35);
  drums.forEach((d, i) => {
    const [uc, sc, t, hl] = d;
    const p = P(uc, sc, t);
    g.poly(shapes[i]).fill(T[0]).stroke({ width: OUT, color: INK, join: 'round' });
    const end = i === n - 1 ? hl - 0.1 : hl - 0.01;
    g.poly([...p(-hl + 0.01, -lit * r * 0.62), ...p(end, -lit * r * 0.62), ...p(end, lit * r * 0.95), ...p(-hl + 0.01, lit * r * 0.95)]).fill(T[1]);
    g.poly([...p(-hl + 0.03, lit * r * 0.2), ...p(end - 0.02, lit * r * 0.2), ...p(end - 0.02, lit * r * 0.62), ...p(-hl + 0.03, lit * r * 0.62)]).fill({ color: T[2], alpha: 0.8 });
    for (const k of [-0.75, -0.4, 0, 0.4, 0.75]) g.moveTo(...p(-hl + 0.03, k * r)).lineTo(...p(end - 0.03, k * r));
    g.stroke({ width: 0.014, color: T[0], alpha: 0.7 });
    if (i === n - 1) g.poly(oval(...p(hl - 0.08, 0), 0.03, r * 0.7, t, 10)).fill({ color: T[0], alpha: 0.6 });
  });
  g.poly(echinus).fill(T[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...cp(0.19, -lit * 0.3), ...cp(0.41, -lit * r * 0.7), ...cp(0.41, lit * r * 0.95), ...cp(0.19, lit * 0.38)]).fill(T[1]);
  g.moveTo(...cp(0.3, -0.34)).lineTo(...cp(0.3, 0.34)).stroke({ width: 0.016, color: T[0] });
  g.poly(abacus).fill(T[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...cp(0.02, -lit * 0.3), ...cp(0.16, -lit * 0.3), ...cp(0.16, lit * 0.41), ...cp(0.02, lit * 0.41)]).fill(T[1]);
  g.poly([...cp(0.03, lit * 0.12), ...cp(0.15, lit * 0.12), ...cp(0.15, lit * 0.38), ...cp(0.03, lit * 0.38)]).fill({ color: T[2], alpha: 0.8 });
  // Chips.
  for (let i = 0; i < 4; i++) rock(g, ...at(0.5 + (len - 0.7) * rnd(), rnd() < 0.5 ? 0.1 + 0.06 * rnd() : 0.84 + 0.06 * rnd()), 0.035 + 0.02 * rnd(), T, light, rnd, 6);
}

/** Arcane workbench against `back`: a dark table, stoppered bottles along the wall, a copper alembic with its glass
 * head and a long neck down to a receiving flask (a violet distillate), a heavy tome lying open, a lit candle. */
export function arcaneWorkbench(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const top = rect(END_GAP, WALL_GAP, len - END_GAP, 0.92);
  castShadow(g, quad(top, rnd, 0), light, 0.35);
  board(g, top, DARK_WOOD, light, rnd, along, 0.05, 2);
  const [lx, ly] = light;
  const liquids = [0x7a5e48, 0x6a5a40, 0x8a6a3a, 0x6a6048];
  // Bottles along the wall.
  for (const [u, k] of [[END_GAP + 0.16, 0], [END_GAP + 0.32, 1], [END_GAP + 0.46, 2]] as const) {
    const [x, y] = at(u, WALL_GAP + 0.12);
    g.circle(x, y, 0.055).fill(0x7a6e5a).stroke({ width: LINE, color: INK });
    g.circle(x + lx * 0.012, y + ly * 0.012, 0.035).fill(liquids[(v + k + 1) % liquids.length]);
    g.circle(x, y, 0.02).fill(0x9a7a52).stroke({ width: 0.012, color: INK });
  }
  // The alembic: a copper pot, its glass head, the neck down to the receiver.
  const [px, py] = at(0.5, 0.62);
  const [rx, ry] = at(0.86, 0.76);
  g.moveTo(px, py).lineTo(rx, ry).stroke({ width: 0.05, color: INK, cap: 'round' });
  g.moveTo(px, py).lineTo(rx, ry).stroke({ width: 0.026, color: 0xcfc6b0, cap: 'round' });
  g.circle(px, py, 0.14).fill(BRONZE[0]).stroke({ width: LINE, color: INK });
  g.circle(px + lx * 0.02, py + ly * 0.02, 0.115).fill(BRONZE[1]);
  g.circle(px + lx * 0.05, py + ly * 0.05, 0.045).fill({ color: BRONZE[2], alpha: 0.8 });
  g.circle(px, py, 0.07).fill({ color: 0xcfc6b0, alpha: 0.75 }).stroke({ width: 0.016, color: INK });
  g.circle(px + lx * 0.025, py + ly * 0.025, 0.02).fill({ color: 0xffffff, alpha: 0.7 });
  g.circle(rx, ry, 0.075).fill({ color: 0xcfc6b0, alpha: 0.6 }).stroke({ width: LINE, color: INK });
  g.circle(rx - lx * 0.01, ry - ly * 0.01, 0.052).fill(0x7a5e48);
  g.circle(rx + lx * 0.03, ry + ly * 0.03, 0.016).fill({ color: 0xffffff, alpha: 0.7 });
  // The tome, open.
  const um = len * 0.62;
  g.poly(quad(rect(um - 0.25, 0.4, um + 0.25, 0.86), rnd, 0.006)).fill(0x4a3426).stroke({ width: LINE, color: INK, join: 'round' });
  for (const [a, b] of [[um - 0.22, um - 0.01], [um + 0.01, um + 0.22]]) {
    g.poly(quad(rect(a, 0.43, b, 0.83), rnd, 0.004)).fill(LINEN[1]).stroke({ width: 0.016, color: INK });
    for (let k = 0; k < 6; k++) {
      const s = 0.49 + k * 0.055;
      if (k === 2 && a > um) continue;
      g.moveTo(...at(a + 0.03, s)).lineTo(...at(b - 0.03 - 0.05 * rnd(), s));
    }
    g.stroke({ width: 0.014, color: 0x8a7e64, alpha: 0.8 });
  }
  g.circle(...at(um + 0.12, 0.6), 0.045).stroke({ width: 0.014, color: 0x8a7e64 });
  g.moveTo(...at(um, 0.43)).lineTo(...at(um, 0.83)).stroke({ width: 0.018, color: INK });
  g.moveTo(...at(um + 0.04, 0.83)).lineTo(...at(um + 0.06, 0.92)).stroke({ width: 0.02, color: 0x5a4a3a, cap: 'round' });
  // The candle, alight.
  candle(g, ...at(len - END_GAP - 0.16, WALL_GAP + 0.14));
}

/** Arcane focus: a big pale crystal held point up in a bronze cradle on a round stone pedestal over a square plinth,
 * its facets lit toward the light, a glint at its point, a faint violet light on the stone round it. */
export function arcaneFocus(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  castShadow(g, quad([cx - 0.36, cy - 0.36, cx + 0.36, cy + 0.36], rnd, 0), light, 0.9);
  board(g, [cx - 0.38, cy - 0.38, cx + 0.38, cy + 0.38], STONE, light, rnd, true, 0.045, 0);
  g.poly(ring(cx, cy, 0.28, rnd, 0.004, 28)).fill(MARBLE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(ring(cx + lx * 0.025, cy + ly * 0.025, 0.235, rnd, 0.004, 28)).fill(MARBLE[1]);
  glowPool(g, cx, cy, 0.26, 0xc8b8f0);
  // The cradle's three claws.
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < 3; i++) {
    const a = ph + (i * Math.PI * 2) / 3;
    inkLine(g, [[cx + Math.cos(a) * 0.2, cy + Math.sin(a) * 0.2], [cx + Math.cos(a + 0.25) * 0.14, cy + Math.sin(a + 0.25) * 0.14]], 0.03, BRONZE[1]);
  }
  // The crystal from above: six facets meeting at its point, each toned by how it faces the light.
  const R = 0.19;
  const pts: [number, number][] = [];
  for (let i = 0; i < 6; i++) {
    const a = ph + 0.3 + (i * Math.PI) / 3;
    pts.push([cx + Math.cos(a) * R, cy + Math.sin(a) * R]);
  }
  const apex: [number, number] = [cx + lx * 0.02, cy + ly * 0.02];
  for (let i = 0; i < 6; i++) {
    const [p, q] = [pts[i], pts[(i + 1) % 6]];
    const [nx, ny] = [(p[0] + q[0]) / 2 - cx, (p[1] + q[1]) / 2 - cy];
    const k = (nx * lx + ny * ly) / Math.hypot(nx, ny);
    g.poly([...apex, ...p, ...q]).fill(ARCANE[k > 0.4 ? 2 : k < -0.4 ? 0 : 1]);
  }
  for (const p of pts) g.moveTo(...apex).lineTo(...p);
  g.stroke({ width: 0.014, color: ARCANE[0], alpha: 0.9 });
  g.poly(pts.flat()).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(apex[0] + lx * 0.03, apex[1] + ly * 0.03, 0.022).fill(ARCANE[3]);
}

/** Bone mound: bones and skulls heaped in one big mound, its foot spreading out round it — inked as one shape, a
 * shaded flank away from the light and a paler crown toward it, dark gaps in the jumble, long bones lying every
 * way, skulls tumbled down its sides and one on top. Look 1 is crowned with a horned beast's skull. */
export function boneMound(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [lx, ly] = light;
  const R = Math.min(f.w, f.h) * 0.43;
  castShadow(g, patch(cx, cy, R * 0.9, R * 0.9, rnd, 0.1, 20), light, 0.4);
  g.poly(patch(cx, cy, R, R * 0.95, rnd, 0.1, 28)).fill(BONE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + lx * 0.09, cy + ly * 0.09, R * 0.84, R * 0.8, rnd, 0.08, 24)).fill(shade(BONE[1], 0.86));
  g.poly(patch(cx + lx * 0.18, cy + ly * 0.18, R * 0.48, R * 0.45, rnd, 0.1, 18)).fill({ color: BONE[1], alpha: 0.9 });
  // Gaps in the jumble.
  for (let i = 0; i < 22; i++) {
    const [a, d] = [rnd() * Math.PI * 2, R * 0.85 * Math.sqrt(rnd())];
    g.poly(oval(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.04 + 0.03 * rnd(), 0.02 + 0.015 * rnd(), rnd() * Math.PI, 8)).fill({ color: 0x3a3226, alpha: 0.55 });
  }
  // Long bones, from the foot up to the crown.
  const tone = (d: number) => shade(BONE[1], 0.84 + 0.16 * (1 - d / R));
  for (let i = 0; i < 26; i++) {
    const d = R * (0.92 - (i / 26) * 0.85) * (0.9 + 0.2 * rnd());
    const a = rnd() * Math.PI * 2;
    const [x, y] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
    const t = a + Math.PI / 2 + (rnd() - 0.5) * 1.6;
    const l = (0.1 + 0.09 * rnd()) * (d > R * 0.7 ? 0.8 : 1);
    bone(g, [x - Math.cos(t) * l, y - Math.sin(t) * l], [x + Math.cos(t) * l, y + Math.sin(t) * l], 0.035 + 0.012 * rnd(), light, 0.036, tone(d));
  }
  // Skulls down the sides, then the crown.
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2 + rnd() * 0.6;
    const d = R * (0.35 + 0.35 * rnd());
    skullTop(g, cx + Math.cos(a) * d, cy + Math.sin(a) * d, a + (rnd() - 0.5) * 1.2, 0.065 + 0.015 * rnd(), light);
  }
  const [tx, ty] = [cx + lx * 0.08, cy + ly * 0.08];
  const dir = Math.atan2(ly, lx) + (rnd() - 0.5) * 0.8;
  if (v % 2 === 1) {
    // A horned beast's skull: long-snouted, two horns sweeping back.
    const [c, sn] = [Math.cos(dir), Math.sin(dir)];
    const p = (u: number, k: number): [number, number] => [tx + c * u - sn * k, ty + sn * u + c * k];
    for (const k of [-1, 1]) {
      inkLine(g, [p(0.05, k * 0.08), p(0.2, k * 0.24), p(0.04, k * 0.34)], 0.05, BONE[0], true);
      inkLine(g, [p(0.06, k * 0.09), p(0.19, k * 0.23), p(0.06, k * 0.32)], 0.02, BONE[2], true);
    }
    g.poly([...p(0.1, 0), ...p(0.06, 0.1), ...p(-0.06, 0.09), ...p(-0.22, 0.05), ...p(-0.24, -0.05), ...p(-0.06, -0.09), ...p(0.06, -0.1)]).fill(BONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(oval(...p(0.03, 0), 0.06, 0.05, dir, 10)).fill({ color: BONE[2], alpha: 0.9 });
    for (const k of [-1, 1]) g.poly(oval(...p(-0.04, k * 0.055), 0.03, 0.022, dir, 8)).fill(0x1a1410);
    for (const k of [-1, 1]) g.circle(...p(-0.2, k * 0.02), 0.012).fill(0x1a1410);
  } else skullTop(g, tx, ty, dir, 0.095, light);
}

/** Totem, seen from straight above: the top of a carved pole in plan — a round cap painted with ochre rings and a
 * skull on it, broad wings spread to both sides, two horns curling out over the top — its shadow thrown across the floor. */
export function totem(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  const wing = (sgn: number) => [[sgn * 0.14, -0.1], [sgn * 0.46, -0.13], [sgn * 0.48, 0.1], [sgn * 0.4, 0.15], [sgn * 0.14, 0.11]].flatMap(([a, b]) => [cx + a, cy + b]);
  const horn = (sgn: number) => [[sgn * 0.1, -0.2], [sgn * 0.22, -0.44], [sgn * 0.36, -0.47], [sgn * 0.3, -0.38], [sgn * 0.27, -0.3], [sgn * 0.2, -0.2]].flatMap(([a, b]) => [cx + a, cy + b]);
  for (const sgn of [-1, 1]) {
    castShadow(g, wing(sgn), light, 0.5);
    castShadow(g, horn(sgn), light, 0.5);
  }
  g.circle(cx + lx * 0.05, cy + ly * 0.05, 0.27).fill({ color: 0x14100c, alpha: 0.25 });
  for (const sgn of [-1, 1]) {
    g.poly(wing(sgn)).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly([cx + sgn * 0.18, cy - 0.07, cx + sgn * 0.44, cy - 0.09, cx + sgn * 0.44, cy - 0.05, cx + sgn * 0.18, cy - 0.03]).fill({ color: WOOD[2], alpha: 0.8 });
    g.circle(cx + sgn * 0.33, cy + 0.01, 0.04).fill(0x9a4a32).stroke({ width: LINE, color: INK });
    g.poly(horn(sgn)).fill(BONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(cx + sgn * 0.14, cy - 0.22).lineTo(cx + sgn * 0.26, cy - 0.41).stroke({ width: 0.02, color: BONE[2], cap: 'round' });
  }
  // The round cap with its painted rings.
  g.circle(cx, cy, 0.26).fill(WOOD[1]).stroke({ width: OUT, color: INK });
  g.circle(cx, cy, 0.2).fill(0x9a4a32).stroke({ width: 0.018, color: INK });
  g.circle(cx - lx * 0.01, cy - ly * 0.01, 0.15).fill(WOOD[2]);
  skullTop(g, cx, cy + 0.01, -Math.PI / 2, 0.13, light);
  void rnd;
}

/** Scaffolding: a raised timber walkway (its height and edge drawn by the floor heights, so nothing here inks its
 * outline): planks across it with dark gaps between, a plank or two missing showing the cross-braces below, beams
 * along its long sides with posts standing up from them at every square, nail heads. */
export function scaffold(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { len, at, rect } = frame(f);
  const wid = Math.min(f.w, f.h);
  const [lx, ly] = light;
  const fq = (u0: number, s0: number, u1: number, s1: number, j = 0.004) => quad(rect(u0, s0, u1, s1), rnd, j);
  // The depth under the deck, the braces in it.
  g.poly(fq(0.02, 0.02, len - 0.02, wid - 0.02, 0)).fill(0x221a12);
  for (let u = 0; u < len - 0.5; u++) {
    g.moveTo(...at(u + 0.1, 0.1)).lineTo(...at(u + 0.9, wid - 0.1)).moveTo(...at(u + 0.9, 0.1)).lineTo(...at(u + 0.1, wid - 0.1));
  }
  g.stroke({ width: 0.06, color: DARK_WOOD[0] });
  // The planks.
  const pw = 0.2;
  const n = Math.round((len - 0.04) / pw);
  const gone = new Set<number>();
  for (let k = 0; k < Math.max(1, Math.floor(len / 2)); k++) gone.add(1 + Math.floor(rnd() * (n - 2)));
  for (let k = 0; k < n; k++) {
    if (gone.has(k)) continue;
    const u0 = 0.02 + ((len - 0.04) * k) / n + 0.012;
    const u1 = 0.02 + ((len - 0.04) * (k + 1)) / n - 0.012;
    const t = WOOD.map((c) => shade(c, 0.92 + 0.14 * rnd()));
    const short = rnd() < 0.2 ? 0.05 + 0.08 * rnd() : 0;
    const p = fq(u0, 0.03 + (rnd() < 0.5 ? short : 0), u1, wid - 0.03 - (rnd() < 0.5 ? short : 0));
    g.poly(p).fill(t[1]).stroke({ width: 0.016, color: INK, alpha: 0.85, join: 'round' });
    const [ax, ay, , , bx, by] = p;
    const [ex, ey] = [Math.min(ax, bx), Math.min(ay, by)];
    const [fx, fy] = [Math.max(ax, bx), Math.max(ay, by)];
    litEdges(g, [ex, ey, fx, fy], light, 0.03);
    g.fill({ color: t[2], alpha: 0.6 });
    g.moveTo(...at(u0 + 0.05, wid * (0.3 + 0.4 * rnd()))).lineTo(...at(u1 - 0.05, wid * (0.3 + 0.4 * rnd()))).stroke({ width: 0.014, color: t[0], alpha: 0.7 });
    for (const s of [0.1, wid - 0.1]) g.circle(...at((u0 + u1) / 2, s), 0.014).fill(IRON[0]);
  }
  // Beams along the long sides, inked on their inner edge only, and the posts on them.
  for (const [s0, s1, inner] of [[0.0, 0.09, 0.09], [wid - 0.09, wid, wid - 0.09]]) {
    g.poly(fq(0.0, s0, len, s1, 0.002)).fill(DARK_WOOD[1]);
    g.moveTo(...at(0.0, inner)).lineTo(...at(len, inner)).stroke({ width: LINE, color: INK });
  }
  for (let u = 0; u <= len + 0.01; u++) {
    for (const s of [0.07, wid - 0.07]) {
      const [px, py] = at(Math.min(len - 0.08, Math.max(0.08, u)), s);
      g.rect(px - 0.07 - lx * 0.06, py - 0.07 - ly * 0.06, 0.14, 0.14).fill({ color: 0x14100c, alpha: 0.25 });
      g.rect(px - 0.06, py - 0.06, 0.12, 0.12).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK });
      g.rect(px - 0.06 + (lx < 0 ? 0.015 : 0.045), py - 0.06 + (ly < 0 ? 0.015 : 0.045), 0.06, 0.06).fill(DARK_WOOD[2]);
    }
  }
}

/** Cracked floor (a hidden hazard, as the DM sees it): a slightly sunken patch of floor — a shade on its side toward
 * the light where its rim overhangs, a paler sliver opposite — cracks running out from its middle and part way round
 * it, a few loose crumbs. Kept faint. */
/** A flooded pit (a hidden hazard, seen only by the DM): a shaft full of dark still water seen from straight above,
 * in the pit's family: an inked stone rim with a wet band inside it, the water in three flat dark tones (shade on
 * the side toward the light, under the rim), one pale ripple line, a floating plank and a little scum, broken
 * stones round the rim. */
export function floodedPit(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) * 0.45;
  const [lx, ly] = light;
  const rough = (k: number, j: number, ox = 0, oy = 0) => {
    const pts: number[] = [];
    for (let i = 0; i < 20; i++) {
      const a = (i / 20) * Math.PI * 2;
      const rr = r * k * (1 + (rnd() - 0.5) * j);
      pts.push(cx + ox + Math.cos(a) * rr, cy + oy + Math.sin(a) * rr);
    }
    return pts;
  };
  g.poly(rough(1, 0.1)).fill(STONE[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(rough(0.84, 0.08)).fill(0x4a4540).stroke({ width: LINE, color: INK, join: 'round' });
  // The water: its shade toward the light (the rim's shadow), the open water, and a lit patch beyond.
  g.poly(rough(0.76, 0.06)).fill(0x1c2627);
  g.poly(rough(0.62, 0.08, lx * r * 0.12, ly * r * 0.12)).fill(0x2b3a3b);
  g.poly(rough(0.32, 0.12, lx * r * 0.3, ly * r * 0.3)).fill(0x364849);
  // One pale ripple line, curved round the lit side.
  const a0 = Math.atan2(ly, lx) - 0.7;
  g.moveTo(cx + Math.cos(a0) * r * 0.5, cy + Math.sin(a0) * r * 0.5);
  for (let k = 1; k <= 6; k++) {
    const a = a0 + (k / 6) * 1.4;
    g.lineTo(cx + Math.cos(a) * r * 0.5, cy + Math.sin(a) * r * 0.5);
  }
  g.stroke({ width: 0.018, color: 0x7f9696, alpha: 0.7, cap: 'round', join: 'round' });
  // A floating plank, a little scum.
  const pa = rnd() * Math.PI;
  const [px, py] = [cx - lx * r * 0.15 + (rnd() - 0.5) * 0.08, cy - ly * r * 0.15 + (rnd() - 0.5) * 0.08];
  const [ux, uy, vx, vy] = [Math.cos(pa) * 0.14, Math.sin(pa) * 0.14, -Math.sin(pa) * 0.035, Math.cos(pa) * 0.035];
  g.poly([px - ux - vx, py - uy - vy, px + ux - vx, py + uy - vy, px + ux + vx, py + uy + vy, px - ux + vx, py - uy + vy]).fill(0x6e5638).stroke({ width: LINE * 0.8, color: INK, join: 'round' });
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * (0.3 + 0.3 * rnd());
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.022 + 0.012 * rnd()).fill({ color: 0x5c6444, alpha: 0.85 });
  }
  // Broken stones on the rim.
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    g.poly(ring(cx + Math.cos(a) * r * 0.97, cy + Math.sin(a) * r * 0.97, 0.05 + 0.03 * rnd(), rnd, 0.03, 6)).fill(STONE[2]).stroke({ width: LINE, color: INK });
  }
}

export function crackedFloor(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  g.poly(patch(cx, cy, 0.5, 0.47, rnd, 0.1, 16)).fill({ color: 0x14100c, alpha: 0.14 });
  g.poly(patch(cx + lx * 0.06, cy + ly * 0.06, 0.42, 0.38, rnd, 0.12, 16)).fill({ color: 0x14100c, alpha: 0.2 });
  g.poly(patch(cx - lx * 0.12, cy - ly * 0.12, 0.2, 0.17, rnd, 0.2, 12)).fill({ color: 0xf0e8d8, alpha: 0.12 });
  const crack = (x: number, y: number, a: number, l: number, w: number) => {
    const pts: [number, number][] = [[x, y]];
    for (let k = 1; k <= 4; k++) {
      a += (rnd() - 0.5) * 0.9;
      pts.push([pts[k - 1][0] + Math.cos(a) * (l / 4), pts[k - 1][1] + Math.sin(a) * (l / 4)]);
    }
    g.moveTo(pts[0][0] - lx * 0.016, pts[0][1] - ly * 0.016);
    for (const p of pts.slice(1)) g.lineTo(p[0] - lx * 0.016, p[1] - ly * 0.016);
    g.stroke({ width: 0.02, color: 0xf0e8d8, alpha: 0.3, join: 'round' });
    g.moveTo(...pts[0]);
    for (const p of pts.slice(1)) g.lineTo(...p);
    g.stroke({ width: w, color: INK, alpha: 0.95, cap: 'round', join: 'round' });
    return pts;
  };
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < 6; i++) {
    const a = ph + (i / 6) * Math.PI * 2 + (rnd() - 0.5) * 0.5;
    const pts = crack(cx, cy, a, 0.56 + 0.14 * rnd(), 0.034);
    for (const j of [1, 2, 3]) if (rnd() < 0.6) crack(...pts[j], a + (rnd() < 0.5 ? 0.9 : -0.9), 0.12 + 0.08 * rnd(), 0.022);
  }
  // Part way round the rim.
  const a0 = rnd() * Math.PI * 2;
  g.moveTo(cx + Math.cos(a0) * 0.4, cy + Math.sin(a0) * 0.37);
  for (let k = 1; k <= 8; k++) {
    const a = a0 + (k / 8) * Math.PI * 1.2;
    const r = 0.4 + (rnd() - 0.5) * 0.04;
    g.lineTo(cx + Math.cos(a) * r, cy + Math.sin(a) * r * 0.9);
  }
  g.stroke({ width: 0.026, color: INK, alpha: 0.85, join: 'round' });
  for (let i = 0; i < 6; i++) g.circle(cx + (rnd() - 0.5) * 0.88, cy + (rnd() - 0.5) * 0.88, 0.016 + 0.012 * rnd()).fill({ color: 0x3a342a, alpha: 0.7 });
}
