/**
 * A star wall's bastion props as battlemap sprites (`town::wallside::KINDS`): a light ballista (108) and a pile of
 * its shot stones (109), in the atlas's hand-drawn style (`docs/agent-briefs.md` "Sprites: style rule": ink by
 * size, three close matte tones lit from the upper left, a crescent of shade away from it). Registered in
 * `atlas.ts` `DRAW`.
 */

import type { Graphics } from 'pixi.js';
import { PX } from './atlas';

const INK = 0x1d1a14;
// (Ink widths by size, set when a sprite is drawn: `atlas.ts` imports this module, so PX is read late.)
let OUT = 0;
let LINE = 0;
const inks = () => ([OUT, LINE] = [0.045 * PX, 0.025 * PX]);
/** Timber (dark, mid, light), iron (dark, mid, light), dressed stone (dark, mid, light). */
const WOOD = [0x6e4a2c, 0x8f6440, 0xb38658];
const IRON = [0x4a4e56, 0x6e7278, 0x9aa0a6];
const SHOT = [0x8f887a, 0xaea697, 0xcac2b0];
/** Rope (the skeins and the bowstring), the one accent: a bolt's fletching. */
const ROPE = [0x9a8460, 0xbca57c];
const FLETCH = 0x9b4a32;

/** A round stone at (x, y): ink, its tone, a crescent of shade away from the light, lit toward the upper left. */
function round(g: Graphics, x: number, y: number, r: number, tones: readonly number[]) {
  g.circle(x, y, r).fill(tones[0]);
  g.circle(x - r * 0.14, y - r * 0.14, r * 0.84).fill(tones[1]);
  g.circle(x - r * 0.34, y - r * 0.34, r * 0.32).fill({ color: tones[2], alpha: 0.9 });
  g.circle(x, y, r).stroke({ width: LINE, color: INK });
}

/** A beam from (ax, ay) to (bx, by), `w` wide: its tone, its upper-left edge lit, inked. */
function beam(g: Graphics, ax: number, ay: number, bx: number, by: number, w: number, tones: readonly number[], ink = LINE) {
  const l = Math.hypot(bx - ax, by - ay) || 1;
  const [nx, ny] = [(-(by - ay) / l) * (w / 2), ((bx - ax) / l) * (w / 2)];
  const q = [ax + nx, ay + ny, bx + nx, by + ny, bx - nx, by - ny, ax - nx, ay - ny];
  g.poly(q).fill(tones[1]).stroke({ width: ink, color: INK, join: 'round' });
  // (The edge toward the light: the side whose normal points up-left.)
  const s = nx + ny < 0 ? 1 : -1;
  g.moveTo(ax + nx * s * 0.45, ay + ny * s * 0.45).lineTo(bx + nx * s * 0.45, by + ny * s * 0.45).stroke({ width: w * 0.3, color: tones[2], alpha: 0.85, cap: 'round' });
}

/** A bolt along x from `x0` to `x1` at `y`: shaft, iron head forward, fletching at the back. */
function bolt(g: Graphics, x0: number, x1: number, y: number, r: number) {
  const w = r * 0.05;
  g.moveTo(x0, y).lineTo(x1, y).stroke({ width: w * 2 + LINE, color: INK, cap: 'round' });
  g.moveTo(x0, y).lineTo(x1, y).stroke({ width: w * 2, color: WOOD[2], cap: 'round' });
  g.poly([x1 - r * 0.02, y - r * 0.06, x1 + r * 0.14, y, x1 - r * 0.02, y + r * 0.06]).fill(IRON[1]).stroke({ width: LINE * 0.75, color: INK, join: 'round' });
  for (const s of [-1, 1]) g.poly([x0, y, x0 + r * 0.16, y, x0 + r * 0.06, y + s * r * 0.09, x0 - r * 0.04, y + s * r * 0.09]).fill(FLETCH).stroke({ width: LINE * 0.7, color: INK, join: 'round' });
}

/**
 * A field ballista from above, aimed along +x (about 10 ft long, its radius 5 ft): its trestle and feet, the stock with its slider,
 * the iron-banded torsion frame and its two rope skeins, the arms swept back and the bowstring to the claw, a
 * windlass at the back. Looks: 0 and 2 a bolt laid, 1 and 3 unloaded with spare bolts on the ground beside it;
 * 2 and 3 drawn (the string back on the claw).
 */
export function ballista(g: Graphics, r: number, rnd: () => number, v: number) {
  inks();
  const drawn = v >= 2;
  const loaded = v % 2 === 0;
  // The trestle: a crossbar under the stock on two splayed legs, each on its foot block.
  for (const s of [-1, 1]) {
    beam(g, -r * 0.05, 0, -r * 0.42, s * r * 0.62, r * 0.15, WOOD);
    g.roundRect(-r * 0.56, s * r * 0.62 - r * 0.1, r * 0.28, r * 0.2, r * 0.04).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  }
  beam(g, -r * 0.1, -r * 0.5, -r * 0.1, r * 0.5, r * 0.14, WOOD);
  // Spare bolts on the ground (unloaded looks).
  if (!loaded) for (const y of [r * 0.78, r * 0.88]) bolt(g, -r * 0.55, r * 0.25 + rnd() * r * 0.06, y, r);
  // The stock along the middle, the slider in its groove.
  beam(g, -r * 0.9, 0, r * 0.64, 0, r * 0.26, WOOD, OUT);
  g.moveTo(-r * 0.7, 0).lineTo(r * 0.55, 0).stroke({ width: LINE * 0.8, color: WOOD[0] });
  const claw = drawn ? -r * 0.58 : r * 0.08;
  g.roundRect(claw - r * 0.08, -r * 0.08, r * 0.2, r * 0.16, r * 0.03).fill(IRON[1]).stroke({ width: LINE, color: INK });
  // The windlass at the back: a drum across with its spokes.
  beam(g, -r * 0.8, -r * 0.26, -r * 0.8, r * 0.26, r * 0.13, WOOD);
  for (const s of [-1, 1]) {
    g.moveTo(-r * 0.8, s * r * 0.26).lineTo(-r * 0.8, s * r * 0.4).stroke({ width: r * 0.08 + LINE, color: INK, cap: 'round' });
    g.moveTo(-r * 0.8, s * r * 0.26).lineTo(-r * 0.8, s * r * 0.4).stroke({ width: r * 0.08, color: WOOD[2], cap: 'round' });
  }
  // The arms, swept back from the frame, and the string from their tips to the claw.
  const [ax, tipX, tipY] = [r * 0.42, drawn ? r * 0.12 : r * 0.24, r * 0.86];
  g.moveTo(tipX, -tipY).lineTo(claw, 0).lineTo(tipX, tipY).stroke({ width: LINE * 1.6, color: INK, join: 'round' });
  g.moveTo(tipX, -tipY).lineTo(claw, 0).lineTo(tipX, tipY).stroke({ width: LINE * 0.9, color: ROPE[1], join: 'round' });
  for (const s of [-1, 1]) {
    const pts = [ax + r * 0.02, s * r * 0.3, ax + r * 0.1, s * r * 0.58, tipX + r * 0.07, s * tipY, tipX - r * 0.08, s * (tipY - r * 0.06), ax - r * 0.13, s * r * 0.58, ax - r * 0.1, s * r * 0.3];
    g.poly(pts).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
    g.moveTo(ax - r * 0.02, s * r * 0.34).lineTo(tipX - r * 0.02, s * (tipY - r * 0.08)).stroke({ width: r * 0.05, color: WOOD[2], alpha: 0.85, cap: 'round' });
    g.circle(tipX, s * tipY, r * 0.055).fill(IRON[1]).stroke({ width: LINE * 0.7, color: INK });
  }
  // The torsion frame across the stock: a box banded with iron, a rope skein in each half.
  g.roundRect(ax - r * 0.17, -r * 0.4, r * 0.34, r * 0.8, r * 0.05).fill(WOOD[0]).stroke({ width: OUT, color: INK });
  for (const s of [-1, 1]) {
    g.circle(ax, s * r * 0.2, r * 0.12).fill(ROPE[0]).stroke({ width: LINE, color: INK });
    g.circle(ax - r * 0.03, s * r * 0.2 - r * 0.03, r * 0.065).fill(ROPE[1]);
  }
  for (const y of [-r * 0.4, 0, r * 0.4]) g.rect(ax - r * 0.18, y - r * 0.03, r * 0.36, r * 0.06).fill(IRON[0]);
  g.moveTo(ax - r * 0.15, -r * 0.37).lineTo(ax + r * 0.15, -r * 0.37).stroke({ width: LINE * 0.8, color: IRON[2], alpha: 0.8 });
  // The bolt laid on the stock.
  if (loaded) bolt(g, claw + r * 0.1, r * 0.82, 0, r);
}

/**
 * A pile of round shot stones (about 5 ft across): a ring of stones on the ground and fewer on them, each lit to
 * the upper left. Looks: the heap's stones and how they lie; 3 in a low timber crib.
 */
export function shotStones(g: Graphics, r: number, rnd: () => number, v: number) {
  inks();
  const s = r * 0.27;
  if (v === 3) {
    // The crib: four logs round the heap.
    for (const [ax, ay, bx, by] of [[-0.9, -0.82, 0.9, -0.82], [-0.9, 0.82, 0.9, 0.82], [-0.82, -0.9, -0.82, 0.9], [0.82, -0.9, 0.82, 0.9]]) {
      beam(g, ax * r, ay * r, bx * r, by * r, r * 0.16, WOOD);
    }
  }
  // (The bottom ring, then the stones on them, back to front so the nearer overlap; a little scatter.)
  const ring = v === 1 ? 6 : 7;
  const turn = rnd() * Math.PI * 2;
  const stones: [number, number, number][] = [];
  for (let k = 0; k < ring; k++) {
    const a = turn + (k / ring) * Math.PI * 2;
    stones.push([Math.cos(a) * r * 0.56, Math.sin(a) * r * 0.56, s * (0.9 + rnd() * 0.15)]);
  }
  const upper = v === 2 ? 4 : 3;
  for (let k = 0; k < upper; k++) {
    const a = turn + 0.5 + (k / upper) * Math.PI * 2;
    stones.push([Math.cos(a) * r * 0.24, Math.sin(a) * r * 0.24, s * (0.92 + rnd() * 0.1)]);
  }
  if (v !== 2) stones.push([0, 0, s]);
  // (A stone or two rolled off.)
  if (v !== 3) stones.unshift([Math.cos(turn + 2.4) * r * 0.95, Math.sin(turn + 2.4) * r * 0.95, s * 0.8]);
  // Shadow under the heap, then the stones, the lower ones first.
  g.circle(r * 0.06, r * 0.08, r * 0.72).fill({ color: INK, alpha: 0.16 });
  const lower = stones.slice(0, stones.length - upper - (v !== 2 ? 1 : 0));
  const top = stones.slice(lower.length);
  for (const [x, y, rr] of lower.sort((a, b) => a[0] + a[1] - (b[0] + b[1]))) round(g, x, y, rr, SHOT);
  for (const [x, y, rr] of top.sort((a, b) => a[0] + a[1] - (b[0] + b[1]))) round(g, x - r * 0.03, y - r * 0.05, rr, SHOT);
}
