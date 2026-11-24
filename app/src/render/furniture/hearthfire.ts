import type { Graphics } from 'pixi.js';
import { INK, IRON, LINE, OUT } from './common';
import { trueShape, uneven } from './squash';

/** The hearth's four looks (`Item::variant` 0-3, picked by position like the other furniture; `lookName`): A logs
 * and flame, B a burnt-down ember bed, C banked (logs on firedogs over a small flame), D laid ready and cold (split
 * logs and kindling on the firedogs, swept ash, a poker and tongs). D is cold rather than a cookpot because the hearth
 * also stands in parlours, solars, salons and best chambers, where a stew pot would be wrong. */
/** Where the firebox runs across the hearth (frame `s`, drawing coordinates): from just off the wall out to
 * the surround's front, and how wide it is along it (squares). 4 ft wide, about 1.4 ft deep inside at life size. */
export const BOX_BACK = 0.3;
export const BOX_FRONT = 0.8;
const BOX_W = 0.8;

const BARK = [0x3e2a1c, 0x5e4028, 0x8a6440];
const CHAR = 0x211915;
const SOOT = 0x15110e;
const FLOOR = 0x2c2520;
const ASH = [0x5e5852, 0x7a746c, 0x948e84];
const WOOD_END = [0x9a7448, 0xc29a66];
const KINDLING = 0xa07a4c;
const FLAME = [0xc2410c, 0xf28a1e, 0xffc23a, 0xfff2c0];

type At = (u: number, s: number) => [number, number];

/** Draws the firebox and look `v` in it: a sooted box open toward the room, its floor, then the fire in true feet
 * (`trueShape` at k = 1), so logs and flames keep their shape under the hearth's life-size squash. */
export function hearthFire(g: Graphics, at: At, len: number, v: number, rnd: () => number) {
  const look = ((v % 4) + 4) % 4;
  const [u0, u1] = [len / 2 - BOX_W / 2, len / 2 + BOX_W / 2];
  const box = rectAt(at, u0, BOX_BACK, u1, BOX_FRONT);
  // The box: sooted cheeks and back, the floor inside, a little warmth on the hearth stone in front for the embers.
  g.poly(box).fill(SOOT).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(rectAt(at, u0 + 0.05, BOX_BACK + 0.06, u1 - 0.05, BOX_FRONT)).fill(FLOOR);
  if (look === 1) {
    const [ax, ay] = at(len / 2, BOX_FRONT + 0.02);
    const [bx, by] = at(len / 2, BOX_FRONT + 0.12);
    const pts: number[] = [];
    for (let i = 0; i <= 12; i++) {
      const t = (i / 12) * Math.PI;
      const [ux, uy] = [at(1, 0)[0] - at(0, 0)[0], at(1, 0)[1] - at(0, 0)[1]];
      const r = 0.34;
      pts.push(ax + Math.cos(t) * r * ux + Math.sin(t) * (bx - ax), ay + Math.cos(t) * r * uy + Math.sin(t) * (by - ay));
    }
    g.poly(pts).fill({ color: 0xf59e0b, alpha: 0.1 });
  }
  // The fire, in true squares about the box's middle: x along the hearth, y out toward the room.
  const sq = uneven();
  const [ox, oy] = at(0, 0);
  const U = [at(1, 0)[0] - ox, at(1, 0)[1] - oy];
  const S = [at(0, 1)[0] - ox, at(0, 1)[1] - oy];
  const depthK = sq ? (S[1] !== 0 ? sq.ky : sq.kx) : 1;
  const half = ((BOX_FRONT - BOX_BACK - 0.06) * depthK) / 2;
  const [cx, cy] = at(len / 2, (BOX_BACK + 0.06 + BOX_FRONT) / 2);
  trueShape(
    g,
    cx,
    cy,
    (g) => {
      const P = (x: number, y: number): [number, number] => [cx + x * U[0] + y * S[0], cy + x * U[1] + y * S[1]];
      const ctx: Ctx = { g, P, half, w: BOX_W / 2 - 0.05, rnd };
      [logsAndFlame, emberBed, banked, laid][look](ctx);
    },
    1,
  );
}

type Ctx = { g: Graphics; P: (x: number, y: number) => [number, number]; half: number; w: number; rnd: () => number };

function rectAt(at: At, u0: number, s0: number, u1: number, s1: number): number[] {
  return [...at(u0, s0), ...at(u1, s0), ...at(u1, s1), ...at(u0, s1)];
}

/** A wobbly blob in the fire's frame, as drawing points. */
function blob({ P, rnd }: Ctx, x: number, y: number, rx: number, ry: number, j = 0.16, n = 14): number[] {
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const k = 1 + (rnd() - 0.5) * j * 2;
    pts.push(...P(x + Math.cos(a) * rx * k, y + Math.sin(a) * ry * k));
  }
  return pts;
}

/** The ash bed: a flat grey spill on the floor, a paler heart. */
function ash(c: Ctx, rx: number, ry: number, y = 0) {
  c.g.poly(blob(c, 0, y, rx, ry, 0.12, 18)).fill(ASH[0]);
  c.g.poly(blob(c, 0.02, y + ry * 0.1, rx * 0.72, ry * 0.66, 0.15, 14)).fill(ASH[1]);
}

/** A firedog: a short iron bar running back to front on the floor, a knob at its front end. */
function firedog(c: Ctx, x: number) {
  const { g, P, half } = c;
  g.moveTo(...P(x, -half * 0.8)).lineTo(...P(x, half * 0.55)).stroke({ width: 0.06, color: INK, cap: 'round' });
  g.moveTo(...P(x, -half * 0.8)).lineTo(...P(x, half * 0.55)).stroke({ width: 0.028, color: IRON[1], cap: 'round' });
  g.circle(...P(x, half * 0.62), 0.03).fill(IRON[2]).stroke({ width: LINE, color: INK });
}

/** A log seen from above, (x0, y0) to (x1, y1) in true squares, `t` thick: bark with a lit stripe and a few dark
 * bark streaks, its burning end(s) charred black with glowing cracks (`burn`: 0 neither, 1 the far end, 2 both), its
 * cut ends showing pale end grain (`ends`), or split, a pale split face along it (`split`, fresh wood). */
function log(c: Ctx, x0: number, y0: number, x1: number, y1: number, t: number, burn: 0 | 1 | 2, ends = false, split = false) {
  const { g, P, rnd } = c;
  const [a, b] = [P(x0, y0), P(x1, y1)];
  const seg = (p: [number, number], q: [number, number], width: number, color: number, alpha = 1, cap: 'round' | 'butt' = 'round') => g.moveTo(...p).lineTo(...q).stroke({ width, color, alpha, cap });
  const mix = (k: number): [number, number] => [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
  const [nx, ny] = [-(b[1] - a[1]), b[0] - a[0]];
  const nl = Math.hypot(nx, ny) || 1;
  const side = (p: [number, number], d: number): [number, number] => [p[0] + (nx / nl) * d, p[1] + (ny / nl) * d];
  seg(a, b, t + OUT, INK);
  seg(a, b, t, split ? BARK[0] : BARK[1]);
  // The lit side (toward the NW, as the house light): a stripe of bark, or the split face of fresh wood.
  const lit = ny > 0 ? -1 : 1;
  if (split) seg(side(mix(0.06), lit * t * 0.12), side(mix(0.94), lit * t * 0.12), t * 0.5, 0xb08a5c);
  else seg(side(mix(0.12), lit * t * 0.2), side(mix(0.88), lit * t * 0.2), t * 0.28, BARK[2], 0.55);
  // Bark streaks on the shaded side.
  for (let i = 0; i < 2; i++) {
    const k = 0.22 + 0.4 * i + rnd() * 0.12;
    seg(side(mix(k), -lit * t * 0.24), side(mix(k + 0.16), -lit * t * 0.22), 0.018, INK, 0.7);
  }
  const burnt = burn === 2 ? [[0, 0.32], [0.68, 1]] : burn === 1 ? [[0.62, 1]] : [];
  for (const [k0, k1] of burnt) {
    seg(mix(k0), mix(k1), t * 0.86, CHAR);
    // Glowing cracks across the char.
    for (let i = 0; i < 2; i++) {
      const p = mix(k0 + (k1 - k0) * (0.3 + 0.4 * i + (rnd() - 0.5) * 0.12));
      g.moveTo(...side(p, -t * 0.32)).lineTo(...side(p, t * 0.2)).stroke({ width: 0.018, color: 0xe0601c, cap: 'round' });
    }
  }
  // End grain on the unburnt cut ends: a pale disc with a ring and its heart.
  if (ends) {
    for (const [k, p] of [[0, a], [1, b]] as const) {
      if (burnt.some(([k0, k1]) => k >= k0 && k <= k1)) continue;
      g.circle(...p, t * 0.42).fill(WOOD_END[1]).stroke({ width: 0.016, color: INK });
      g.circle(...p, t * 0.2).stroke({ width: 0.016, color: WOOD_END[0] });
    }
  }
}

/** Nested flat flame shapes seen from above, deep orange out to a pale core; `layers` how many of the four tones. */
function flame(c: Ctx, x: number, y: number, rx: number, ry: number, layers = 4) {
  const ks = [1, 0.72, 0.46, 0.22];
  for (let l = 0; l < layers; l++) {
    const k = ks[l];
    // Each inner tone sits a little toward the back, where the flame stands tallest.
    c.g.poly(blob(c, x + (c.rnd() - 0.5) * rx * 0.1, y - (1 - k) * ry * 0.25, rx * k, ry * k, 0.14, 16)).fill(FLAME[l]);
  }
}

function embersAt(c: Ctx, n: number, rx: number, ry: number, y = 0, dim = false) {
  for (let i = 0; i < n; i++) {
    const a = c.rnd() * Math.PI * 2;
    const d = 0.55 + 0.45 * c.rnd();
    c.g.circle(...c.P(Math.cos(a) * rx * d, y + Math.sin(a) * ry * d), 0.016 + 0.01 * c.rnd()).fill(dim ? (c.rnd() < 0.6 ? 0xa8401a : 0xd2601e) : c.rnd() < 0.5 ? 0xf97316 : 0xffc23a);
  }
}

/** A: two logs crossed over the firedogs on an ash bed, a compact flame where they cross, a few embers. */
function logsAndFlame(c: Ctx) {
  const { half, w } = c;
  ash(c, w * 0.9, half * 0.85);
  firedog(c, -w * 0.55);
  firedog(c, w * 0.55);
  embersAt(c, 6, w * 0.8, half * 0.75);
  log(c, -w * 0.72, -half * 0.5, w * 0.36, half * 0.24, 0.095, 1);
  log(c, w * 0.7, -half * 0.42, -w * 0.32, half * 0.3, 0.085, 1);
  flame(c, w * 0.02, -half * 0.08, w * 0.42, half * 0.64);
}

/** A live coal: a big lump outlined in ink, glowing or going dark, a hot fleck on the glowing ones. */
function coal(c: Ctx, x: number, y: number, r: number, hot: boolean) {
  c.g.poly(blob(c, x, y, r, r * 0.78, 0.2, 9)).fill(hot ? 0xd2541a : 0x3a261c).stroke({ width: 0.016, color: INK, join: 'round' });
  c.g.poly(blob(c, x - r * 0.15, y - r * 0.15, r * 0.42, r * 0.32, 0.2, 7)).fill(hot ? 0xffb02e : 0x6a3a22);
}

/** B: the logs burnt down to stubs at either end, a bed of a few big coals between, one clear lick of flame. */
function emberBed(c: Ctx) {
  const { g, half, w } = c;
  ash(c, w * 0.95, half * 0.85);
  // One flat glow under the coals, then five big coals on it (three live, two going dark), not a scatter of specks.
  g.poly(blob(c, 0, 0, w * 0.6, half * 0.62, 0.12, 16)).fill(0xa8401a);
  const coals: [number, number, number, boolean][] = [
    [-w * 0.36, -half * 0.22, 0.085, true],
    [w * 0.3, -half * 0.28, 0.08, false],
    [w * 0.36, half * 0.3, 0.09, true],
    [-w * 0.3, half * 0.34, 0.075, false],
    [w * 0.02, half * 0.36, 0.07, true],
  ];
  for (const [x, y, r, hot] of coals) coal(c, x, y, r, hot);
  // The stubs: short charred ends of the two logs, cooling at the sides of the bed.
  log(c, -w * 0.98, -half * 0.25, -w * 0.66, -half * 0.05, 0.085, 1, true);
  log(c, w * 0.96, half * 0.05, w * 0.68, -half * 0.2, 0.08, 1, true);
  flame(c, -w * 0.02, -half * 0.12, w * 0.22, half * 0.44);
}

/** C: banked and quiet: two logs of uneven length on the firedogs, a third laid across them, a small flame
 * between, grey ash. */
function banked(c: Ctx) {
  const { half, w } = c;
  ash(c, w * 0.92, half * 0.85);
  embersAt(c, 4, w * 0.6, half * 0.5, 0, true);
  firedog(c, -w * 0.4);
  firedog(c, w * 0.4);
  log(c, -w * 0.7, -half * 0.58, w * 0.5, -half * 0.5, 0.09, 0, true);
  log(c, -w * 0.48, half * 0.54, w * 0.66, half * 0.44, 0.08, 0, true);
  log(c, w * 0.34, -half * 0.86, w * 0.58, half * 0.82, 0.075, 1, true);
  flame(c, -w * 0.12, -half * 0.02, w * 0.3, half * 0.4);
}

/** D: cold and laid ready: swept ash, two split logs on the firedogs, kindling across them and a twist of straw,
 * a poker and tongs stood in the corner against the right cheek. */
function laid(c: Ctx) {
  const { g, P, half, w } = c;
  c.g.poly(blob(c, 0, half * 0.1, w * 0.7, half * 0.6, 0.1, 16)).fill(ASH[0]);
  firedog(c, -w * 0.42);
  firedog(c, w * 0.42);
  log(c, -w * 0.72, -half * 0.52, w * 0.48, -half * 0.5, 0.1, 0, true, true);
  log(c, -w * 0.6, half * 0.5, w * 0.56, half * 0.44, 0.09, 0, true, true);
  // Kindling: a twist of straw under three split sticks all leant the same way, from log to log.
  g.poly(blob(c, -w * 0.08, half * 0.02, w * 0.24, half * 0.3, 0.22, 12)).fill(0xd8c48a).stroke({ width: 0.016, color: INK, join: 'round' });
  for (const x of [-w * 0.3, -w * 0.06, w * 0.18]) {
    const [p, q] = [P(x - w * 0.1, -half * 0.6), P(x + w * 0.1, half * 0.56)];
    g.moveTo(...p).lineTo(...q).stroke({ width: 0.034 + OUT * 0.7, color: INK, cap: 'round' });
    g.moveTo(...p).lineTo(...q).stroke({ width: 0.034, color: KINDLING, cap: 'round' });
  }
  // The poker and the tongs, iron, their ring handles toward the room.
  for (const [x0, x1] of [[w * 0.86, w * 0.78], [w * 0.98, w * 0.94]]) {
    g.moveTo(...P(x0, -half * 0.8)).lineTo(...P(x1, half * 0.78)).stroke({ width: 0.03 + OUT, color: INK, cap: 'round' });
    g.moveTo(...P(x0, -half * 0.8)).lineTo(...P(x1, half * 0.78)).stroke({ width: 0.03, color: IRON[1], cap: 'round' });
  }
  g.circle(...P(w * 0.78, half * 0.9), 0.04).stroke({ width: 0.04, color: INK });
  g.circle(...P(w * 0.78, half * 0.9), 0.04).stroke({ width: 0.018, color: IRON[2] });
}
