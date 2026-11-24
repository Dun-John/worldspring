// Options for the street's drain grate and the sewer manhole (sprite audit, R7 P8b), shown side by side in the
// gallery's `&panels=options`; not drawn on the map yet. Drawn in atlas pixels (`PX` a square) through the
// same `at(a, b)` / `quad` frame as `drawEntrance` (`a` along the street, `b` across it), so the pick drops
// straight into `structures.ts`.
import type { Graphics } from 'pixi.js';
import { PX } from '../scale';
import { INK } from '../pen';

type At = (a: number, b: number) => [number, number];
type Quad = (a0: number, a1: number, b0: number, b1: number) => number[];

/** The light (toward it, upper left) as the furniture has it. */
const L: [number, number] = [-Math.SQRT1_2, -Math.SQRT1_2];
/** Paving stone set round the iron: a little lighter than the cobbles, its joints and edge a dark tone of its own. */
const SET = [0x7c766a, 0x9e988c, 0xb4aea2];
const JOINT = 0x5d574c;
/** Warm mid iron: shade, face, lit. */
const IRON = [0x4a4540, 0x6a655e, 0x8a847a];
const HOLE = 0x262320;

/** Which way each frame axis faces the light: +1 when the light lies toward +a (or +b). */
function facing(at: At): [number, number] {
  const [ox, oy] = at(0, 0);
  const [ax, ay] = at(1, 0);
  const [bx, by] = at(0, 1);
  return [Math.sign((ax - ox) * L[0] + (ay - oy) * L[1]) || 1, Math.sign((bx - ox) * L[0] + (by - oy) * L[1]) || 1];
}

/** A stable small wobble from a seed (no state: the drawing is the same each time). */
const jit = (k: number) => (((Math.sin(k * 12.9898) * 43758.5453) % 1) + 1) % 1 - 0.5;

/** A hand-made quad: `quad`'s corners nudged by up to `j` squares. */
function rough(at: At, a0: number, a1: number, b0: number, b1: number, j: number, seed: number): number[] {
  return [
    ...at(a0 + jit(seed) * j, b0 + jit(seed + 1) * j),
    ...at(a1 + jit(seed + 2) * j, b0 + jit(seed + 3) * j),
    ...at(a1 + jit(seed + 4) * j, b1 + jit(seed + 5) * j),
    ...at(a0 + jit(seed + 6) * j, b1 + jit(seed + 7) * j),
  ];
}

/** An iron grating `ha` x `hb` (half sizes, squares) set in a hole at the frame's middle: the dark hole, its far
 * wall lit, a rim and bars across the street in warm mid iron at `LINE` weight, each bar lit on its NW side. */
function grating(g: Graphics, at: At, quad: Quad, ha: number, hb: number) {
  const [fa, fb] = facing(at);
  g.poly(quad(-ha, ha, -hb, hb)).fill(HOLE);
  // The hole's lit wall: the side away from the light (its near wall shades the rest).
  const w = 0.03;
  g.poly(fa > 0 ? quad(-ha, -ha + w, -hb, hb) : quad(ha - w, ha, -hb, hb)).fill({ color: 0x5a544a, alpha: 0.8 });
  g.poly(fb > 0 ? quad(-ha, ha, -hb, -hb + w) : quad(-ha, ha, hb - w, hb)).fill({ color: 0x5a544a, alpha: 0.8 });
  const n = Math.max(3, Math.round((ha * 2) / 0.055));
  for (let k = 1; k < n; k++) {
    const a = -ha + (k * 2 * ha) / n;
    g.moveTo(...at(a, -hb)).lineTo(...at(a, hb));
  }
  g.stroke({ width: 0.028 * PX, color: IRON[1] });
  for (let k = 1; k < n; k++) {
    const a = -ha + (k * 2 * ha) / n + fa * 0.007;
    g.moveTo(...at(a, -hb + 0.01)).lineTo(...at(a, hb - 0.01));
  }
  g.stroke({ width: 0.014 * PX, color: IRON[2] });
  g.poly(quad(-ha, ha, -hb, hb)).stroke({ width: 0.025 * PX, color: IRON[0], join: 'round' });
}

/** Drain option B: a grating about 2 x 1.5 ft overall (0.4 x 0.3 square) set flush in the paving: a thin frame of
 * dressed stone, lit on its NW edges, the iron inside at `LINE` weight, no heavy outline (the frame's edge a dark
 * tone of the stone, as the cobbles' joints are). */
export function drawDrainB(g: Graphics, at: At, quad: Quad) {
  const [fa, fb] = facing(at);
  const [A, B] = [0.2, 0.15];
  g.poly(rough(at, -A, A, -B, B, 0.012, 3)).fill(SET[1]).stroke({ width: 0.018 * PX, color: JOINT, join: 'round' });
  // The frame's lit edges and its shaded ones (close tones of the stone).
  g.moveTo(...at(fa * A * 0.92, -B * 0.9)).lineTo(...at(fa * A * 0.92, B * 0.9)).stroke({ width: 0.016 * PX, color: SET[2] });
  g.moveTo(...at(-A * 0.9, fb * B * 0.88)).lineTo(...at(A * 0.9, fb * B * 0.88)).stroke({ width: 0.016 * PX, color: SET[2] });
  g.moveTo(...at(-fa * A * 0.92, -B * 0.9)).lineTo(...at(-fa * A * 0.92, B * 0.9)).stroke({ width: 0.016 * PX, color: SET[0], alpha: 0.7 });
  // Joints where the frame's stones meet, at two corners.
  for (const [a, b] of [
    [-A * 0.55, -B],
    [A * 0.5, B],
  ])
    g.moveTo(...at(a, b)).lineTo(...at(a + 0.015, b * 0.72));
  g.stroke({ width: 0.016 * PX, color: JOINT });
  grating(g, at, quad, 0.145, 0.095);
}

/** Drain option C: a smaller grating, about 1.5 x 1 ft (0.3 x 0.2 square), in a shallow gutter channel of long
 * dished stones that runs along the street through it (two squares here): the channel's two edges a dark tone of
 * the stone, its dished middle a shade darker, the stones' joints across it. */
export function drawDrainC(g: Graphics, at: At, quad: Quad) {
  const [, fb] = facing(at);
  const [len, hw] = [1.0, 0.15];
  const edge: number[] = [];
  const side = (b: number, from: number, to: number, seed: number) => {
    for (let k = 0; k <= 10; k++) {
      const a = from + ((to - from) * k) / 10;
      edge.push(...at(a, b + jit(seed + k) * 0.012));
    }
  };
  side(-hw, -len, len, 11);
  side(hw, len, -len, 31);
  g.poly(edge).fill(SET[1]);
  // The dished middle, darker; the lit lip on the side away from the light (it faces the light), the near lip shaded.
  g.poly(quad(-len, len, -hw * 0.45, hw * 0.45)).fill({ color: SET[0], alpha: 0.55 });
  g.moveTo(...at(-len, -fb * hw * 0.78)).lineTo(...at(len, -fb * hw * 0.78)).stroke({ width: 0.016 * PX, color: SET[2] });
  // The stones' joints across the channel; the channel's edges.
  for (const a of [-0.72, -0.26, 0.31, 0.74]) g.moveTo(...at(a + jit(a * 7) * 0.02, -hw)).lineTo(...at(a + jit(a * 9) * 0.02, hw));
  g.stroke({ width: 0.016 * PX, color: JOINT });
  for (const b of [-hw, hw]) {
    g.moveTo(...at(-len, b));
    for (let k = 1; k <= 10; k++) g.lineTo(...at(-len + (2 * len * k) / 10, b + jit(b * 100 + k) * 0.01));
  }
  g.stroke({ width: 0.018 * PX, color: JOINT });
  // The grating, its frame stone flush with the channel.
  g.poly(rough(at, -0.17, 0.17, -0.12, 0.12, 0.01, 5)).fill(SET[1]).stroke({ width: 0.016 * PX, color: JOINT, join: 'round' });
  grating(g, at, quad, 0.13, 0.085);
}

/** Manhole options B (r 0.25 square, 2.5 ft across) and C (r 0.3, 3 ft): a warm mid iron lid with a thin ink ring,
 * a few raised checker treads (lit on their NW sides, shaded on the SE) and a pry slot near its edge, lit along
 * its NW and shaded along its SE edge, ringed by a collar of a few dressed stones flush with the paving. */
export function drawManholeB(g: Graphics, at: At, r: number) {
  const [x, y] = at(0, 0);
  const R = r * PX;
  const c = 0.075;
  // The collar: six or seven stones round the lid, lopsided, joints between them.
  const out: number[] = [];
  const n = 22;
  for (let k = 0; k < n; k++) {
    const t = (k / n) * Math.PI * 2;
    const rr = (r + c + jit(k + r * 10) * 0.025) * PX;
    out.push(x + Math.cos(t) * rr, y + Math.sin(t) * rr);
  }
  g.poly(out).fill(SET[1]).stroke({ width: 0.018 * PX, color: JOINT, join: 'round' });
  const stones = r > 0.27 ? 7 : 6;
  for (let k = 0; k < stones; k++) {
    const t = ((k + 0.3 + jit(k * 3) * 0.3) / stones) * Math.PI * 2;
    g.moveTo(x + Math.cos(t) * R, y + Math.sin(t) * R).lineTo(x + Math.cos(t + 0.06) * (R + c * PX * 1.05), y + Math.sin(t + 0.06) * (R + c * PX * 1.05));
  }
  g.stroke({ width: 0.016 * PX, color: JOINT });
  // The collar's lit outer edge to the NW.
  g.moveTo(x + Math.cos(Math.PI * 1.0) * (R + c * PX * 0.75), y + Math.sin(Math.PI * 1.0) * (R + c * PX * 0.75));
  g.arc(x, y, R + c * PX * 0.75, Math.PI * 1.0, Math.PI * 1.5);
  g.stroke({ width: 0.016 * PX, color: SET[2] });
  // The lid, its lit and shaded crescents.
  g.circle(x, y, R).fill(IRON[1]);
  g.moveTo(x + Math.cos(-Math.PI * 0.2) * R * 0.9, y + Math.sin(-Math.PI * 0.2) * R * 0.9);
  g.arc(x, y, R * 0.9, -Math.PI * 0.2, Math.PI * 0.7);
  g.stroke({ width: R * 0.12, color: IRON[0], alpha: 0.7 });
  g.moveTo(x + Math.cos(Math.PI * 1.05) * R * 0.9, y + Math.sin(Math.PI * 1.05) * R * 0.9);
  g.arc(x, y, R * 0.9, Math.PI * 1.05, Math.PI * 1.45);
  g.stroke({ width: R * 0.08, color: IRON[2], alpha: 0.75 });
  // Checker treads: small raised squares inside the rim, turned a little off the street.
  const step = 0.075 * PX;
  const half = 0.02 * PX;
  const rot = 0.32;
  const [cs, sn] = [Math.cos(rot), Math.sin(rot)];
  for (let i = -4; i <= 4; i++)
    for (let j = -4; j <= 4; j++) {
      if ((i + j) % 2) continue;
      const [u, v] = [i * step, j * step];
      if (Math.hypot(u, v) > R * 0.7) continue;
      const [px, py] = [x + u * cs - v * sn, y + u * sn + v * cs];
      const sq = [px - half, py - half, px + half, py - half, px + half, py + half, px - half, py + half];
      // The tread's SE side shaded (the square moved toward the SE under it), its top lit.
      g.poly(sq.map((p) => p + 0.008 * PX)).fill({ color: IRON[0], alpha: 0.8 });
      g.poly(sq).fill(IRON[2]);
    }
  // A pry slot near the edge, on the shaded side.
  const t = Math.PI * 0.3;
  g.moveTo(x + Math.cos(t - 0.12) * R * 0.82, y + Math.sin(t - 0.12) * R * 0.82).lineTo(x + Math.cos(t + 0.12) * R * 0.82, y + Math.sin(t + 0.12) * R * 0.82);
  g.stroke({ width: 0.022 * PX, color: HOLE, cap: 'round' });
  // The thin ink ring (lid and collar meet).
  g.circle(x, y, R).stroke({ width: 0.025 * PX, color: INK });
}
