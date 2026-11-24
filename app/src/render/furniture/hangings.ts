// Options for things hung on a wall (sprite audit, R7 P8b), drawn strictly from above: a cloth hanging on a wall
// is a thin strip along it. Shown side by side in the gallery's `&panels=options`; not drawn on the map yet
// (`throneDais` and `stage` take `'B'` to show theirs). Squares, through `frame(f, back)` as the furniture is.
//   B "thin strip": the rod with its finials, the cloth a band at most 0.15 deep showing its colours in order
//     along its length, fold ticks, fringe ticks at its ends, a soft narrow shadow under it.
//   C "strip plus draped foot": the same rod, the cloth sagging out to about 0.3 in a few scalloped folds, their
//     ridges lit from the NW.
import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { DARK_WOOD, INK, LINE, LINEN, type Light, OUT, WOOD, board, castShadow, frame, planks, quad, shade } from './common';
import { WALL_GAP } from './home';
import { ARMS, GILT, HOUSES } from './tables';

type At = (u: number, s: number) => [number, number];

/** The tapestries' colours along their length (border, the scene's main colours, border), by look as
 * `tapestry` draws them: the stag hunt in green woods, the unicorn on red, the ship by a castle, the rider. */
const TAPESTRY: [number, number][][] = [
  [[0x6a1e1a, 0.4], [0x3a5a2a, 1], [0x7a5a3a, 0.5], [0x5e8240, 1], [0xc8b890, 0.35], [0x46682e, 1], [0x6a1e1a, 0.4]],
  [[0x22365e, 0.4], [0x9e3428, 1], [0xe4dac0, 0.45], [0x9e3428, 0.8], [0x46682e, 0.6], [0x9e3428, 1], [0x22365e, 0.4]],
  [[0x7a2a22, 0.4], [0x34528a, 1], [0xe4dac0, 0.4], [0x4a6ca6, 0.8], [0xa49a88, 0.7], [0x34528a, 0.8], [0x7a2a22, 0.4]],
  [[0x2e4a26, 0.4], [0x6a86a0, 0.8], [0xb08a4a, 0.7], [0x5e8240, 1], [0xa49a88, 0.5], [0x46682e, 0.8], [0x2e4a26, 0.4]],
];
const TATTERED = [0x8e2a22, 0x2a4078, 0x3a6430, 0x523c66];

/** A stable small wobble from a seed. */
const jit = (k: number) => (((Math.sin(k * 12.9898) * 43758.5453) % 1) + 1) % 1 - 0.5;

/** Which way along the wall (+1 or -1 in `u`) faces the light. */
function litSide(at: At, light: Light): number {
  const [ax, ay] = at(1, 0);
  const [ox, oy] = at(0, 0);
  return (ax - ox) * light[0] + (ay - oy) * light[1] > 0 ? 1 : -1;
}

/** The rod along the wall from `u0` to `u1` at `s`, inked, with a finial at each end. */
export function hangingRod(g: Graphics, at: At, u0: number, u1: number, s: number, finial: number = GILT[1], w = 0.032) {
  for (const [ww, c] of [
    [w + 0.036, INK],
    [w, DARK_WOOD[1]],
  ] as const)
    g.moveTo(...at(u0, s)).lineTo(...at(u1, s)).stroke({ width: ww, color: c, cap: 'round' });
  for (const u of [u0, u1]) g.circle(...at(u, s), 0.034).fill(finial).stroke({ width: 0.018, color: INK });
}

/** Colour bands by weight across `u0`..`u1`: [from, to, colour]. */
function bands(u0: number, u1: number, colours: [number, number][]): [number, number, number][] {
  const total = colours.reduce((a, [, w]) => a + w, 0);
  let u = u0;
  return colours.map(([c, w]) => {
    const a = u;
    u += ((u1 - u0) * w) / total;
    return [a, u, c];
  });
}

export type ClothOpts = { fringe?: number; ragged?: number; ticks?: boolean; ink?: number };

/**
 * A hanging cloth seen from above, from the rod at `s0` out into the room: option B (`folds` 0) a band `depth`
 * deep, its edge a little uneven; option C (`folds` 3-5) sagging out to `depth` at each fold's crest. Coloured in
 * bands along its length (`colours`: colour and weight), fold ticks across it in the band's own darker and lighter
 * tones, fringe ticks at its two ends, a soft narrow shadow cast on the floor. Returns the hem's depth at `u`.
 */
export function hangingCloth(g: Graphics, at: At, light: Light, u0: number, u1: number, s0: number, depth: number, folds: number, colours: [number, number][], o: ClothOpts = {}): (u: number) => number {
  const rag = o.ragged ?? 0;
  const len = u1 - u0;
  /** The hem's depth from the rod at `u`. */
  const hem = (u: number): number => {
    const t = (u - u0) / len;
    if (!folds) return depth * (0.94 + jit(Math.round(t * 23) + s0 * 50) * 0.08) - rag * Math.abs(jit(Math.round(t * 40) * 1.7)) * 2;
    const k = Math.min(folds - 1, Math.floor(t * folds));
    const lt = t * folds - k;
    const crest = depth * (0.9 + jit(k * 5 + 1) * 0.16);
    const trough = depth * 0.62;
    return trough + (crest - trough) * Math.sin(Math.PI * lt) ** 0.7;
  };
  const N = Math.max(16, Math.round(len * 30));
  const us = Array.from({ length: N + 1 }, (_, k) => u0 + (len * k) / N);
  const outline = [...at(u0, s0), ...at(u1, s0), ...us.reverse().flatMap((u) => at(u, s0 + hem(u)))];
  us.reverse();
  castShadow(g, outline, light, folds ? 0.045 : 0.035);
  // The bands, each its slice of the outline.
  const bs = bands(u0, u1, colours);
  for (const [a, b, c] of bs) {
    const inner = us.filter((u) => u > a && u < b);
    const edge = [a, ...inner, b];
    g.poly([...at(a, s0), ...at(b, s0), ...edge.reverse().flatMap((u) => at(u, s0 + hem(u)))]).fill(c);
  }
  const bandAt = (u: number) => (bs.find(([a, b]) => u >= a && u <= b) ?? bs[bs.length - 1])[2];
  const side = litSide(at, light);
  if (folds) {
    // Each fold: its lit flank toward the light, its far flank and the troughs between folds shaded.
    for (let k = 0; k < folds; k++) {
      const a = u0 + (len * k) / folds;
      const b = u0 + (len * (k + 1)) / folds;
      const m = (a + b) / 2;
      const [lu, du] = side > 0 ? [b, a] : [a, b];
      const flank = (from: number, to: number, color: number, alpha: number) => {
        const q: number[] = [];
        for (let i = 0; i <= 6; i++) {
          const u = from + ((to - from) * i) / 6;
          q.push(...at(u, s0 + 0.03));
        }
        for (let i = 6; i >= 0; i--) {
          const u = from + ((to - from) * i) / 6;
          q.push(...at(u, s0 + hem(u) - 0.01));
        }
        g.poly(q).fill({ color, alpha });
      };
      flank(m, m + (lu - m) * 0.7, 0xf6e6c4, 0.24);
      flank(m + (du - m) * 0.25, du, 0x2a1626, 0.26);
      // The ridge, lit; the trough line, a dark tone of the cloth.
      g.moveTo(...at(m + (lu - m) * 0.12, s0 + 0.05)).lineTo(...at(m + (lu - m) * 0.08, s0 + hem(m) - 0.04));
      g.stroke({ width: 0.018, color: shade(bandAt(m), 1.32), cap: 'round' });
      if (k) {
        g.moveTo(...at(a, s0 + 0.02)).lineTo(...at(a + jit(k) * 0.02, s0 + hem(a) - 0.005));
        g.stroke({ width: 0.018, color: shade(bandAt(a), 0.55), cap: 'round' });
      }
    }
  } else if (o.ticks !== false) {
    // Fold ticks: a dark one and a lit one beside it, every so often along the band.
    for (let u = u0 + 0.1; u < u1 - 0.06; u += 0.16 + jit(u * 31) * 0.04) {
      const c = bandAt(u);
      const d = hem(u);
      g.moveTo(...at(u, s0 + 0.02)).lineTo(...at(u + jit(u * 7) * 0.02, s0 + d - 0.015)).stroke({ width: 0.016, color: shade(c, 0.62), cap: 'round' });
      g.moveTo(...at(u + side * 0.022, s0 + 0.025)).lineTo(...at(u + side * 0.022, s0 + d * 0.75)).stroke({ width: 0.014, color: shade(c, 1.3), alpha: 0.85, cap: 'round' });
    }
  }
  g.poly(outline).stroke({ width: o.ink ?? (folds ? OUT : LINE), color: INK, join: 'round' });
  // Fringe ticks out past each end.
  const fr = o.fringe ?? GILT[1];
  for (const [u, d] of [
    [u0, -1],
    [u1, 1],
  ] as const) {
    const s1 = s0 + hem(u);
    for (let i = 0; i < 3; i++) {
      const s = s0 + 0.03 + ((s1 - s0 - 0.04) * i) / 2;
      g.moveTo(...at(u + d * 0.012, s)).lineTo(...at(u + d * 0.05, s + 0.01));
    }
  }
  g.stroke({ width: 0.016, color: fr, cap: 'round' });
  return hem;
}

/** Wall banner and tapestry options (`opt` 'B' or 'C'), as `tapestry` and `wallBanner` lay them out. */
export function tapestryOpt(g: Graphics, f: InteriorItem, light: Light, v: number, back: [number, number], opt: 'B' | 'C') {
  const { len, at } = frame(f, back);
  const top = WALL_GAP - 0.02;
  hangingCloth(g, at, light, 0.14, len - 0.14, top + 0.015, opt === 'B' ? 0.13 : 0.3, opt === 'B' ? 0 : Math.max(3, Math.min(5, Math.round(len * 1.5))), TAPESTRY[v % 4]);
  hangingRod(g, at, 0.08, len - 0.08, top);
}

export function wallBannerOpt(g: Graphics, f: InteriorItem, light: Light, v: number, back: [number, number], opt: 'B' | 'C') {
  const { at } = frame(f, back);
  const top = WALL_GAP - 0.02;
  const h = HOUSES[v % HOUSES.length];
  const cols: [number, number][] = [[h.field[1], 1], [h.charge, 0.45], [h.field[1], 1]];
  hangingCloth(g, at, light, 0.24, 0.76, top + 0.015, opt === 'B' ? 0.12 : 0.28, opt === 'B' ? 0 : 3, cols);
  hangingRod(g, at, 0.16, 0.84, top);
}

/** The dungeon's tattered banner: ragged along its hem, a gilt emblem band in the middle. */
export function tatteredBannerOpt(g: Graphics, f: InteriorItem, light: Light, v: number, wall: [number, number], opt: 'B' | 'C') {
  const { at } = frame(f, wall);
  const c = TATTERED[v % 4];
  const cols: [number, number][] = [[shade(c, 1.1), 0.6], [c, 0.6], [GILT[1], 0.3], [c, 0.6], [shade(c, 0.85), 0.6]];
  hangingCloth(g, at, light, 0.22, 0.78, 0.075, opt === 'B' ? 0.13 : 0.28, opt === 'B' ? 0 : 3, cols, { ragged: 0.02, fringe: GILT[2] });
  hangingRod(g, at, 0.12, 0.88, 0.06);
}

/** Guild arms on their banner (two squares): the cloth as B or C, the shield on it seen edge-on from above, a
 * shallow curved slab of its rim with the field's colour inside, standing a little proud of the cloth. */
export function guildArmsOpt(g: Graphics, f: InteriorItem, light: Light, v: number, back: [number, number], opt: 'B' | 'C') {
  const { len, at } = frame(f, back);
  const top = WALL_GAP - 0.02;
  const c = len / 2;
  const arms = ARMS[v % ARMS.length];
  const [k0, k1, k2] = arms.cloth;
  const cols: [number, number][] = [[GILT[1], 0.1], [k2, 0.5], [k1, 1.6], [k0, 0.5], [GILT[1], 0.1]];
  hangingCloth(g, at, light, c - 0.5, c + 0.5, top + 0.015, opt === 'B' ? 0.12 : 0.28, opt === 'B' ? 0 : 3, cols);
  // The shield's top edge: a curved slab, deepest in the middle.
  const hw = 0.3;
  const s0 = top + 0.035;
  const edge = (d: number) => Array.from({ length: 13 }, (_, k) => {
    const t = k / 12;
    return at(c - hw + t * 2 * hw, s0 + d * Math.sin(Math.PI * t));
  });
  const outer = edge(0.12);
  const inner = edge(0.08);
  const slab = [...outer, ...[...edge(0.02)].reverse()].flat();
  castShadow(g, slab, light, 0.05);
  g.poly(slab).fill(arms.rim).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly([...inner, ...[...edge(0.045)].reverse()].flat()).fill(arms.field[1]);
  g.moveTo(...inner[2]);
  for (const p of inner.slice(3, 10)) g.lineTo(...p);
  g.stroke({ width: 0.014, color: arms.field[2] });
  hangingRod(g, at, c - 0.62, c + 0.62, top);
}

/** Job board options from above. B: the board seen on its top edge, a thin plank hung on two pegs, the papers'
 * corners sticking out beyond it as tiny light ticks. C: the board leaning out at its foot, so its face shows
 * foreshortened, the papers as thin slivers on it. */
export function jobBoardOpt(g: Graphics, f: InteriorItem, light: Light, v: number, back: [number, number], opt: 'B' | 'C', rnd: () => number) {
  const { along, len, rect, at } = frame(f, back);
  const top = WALL_GAP - 0.02;
  const wood = v === 2 ? DARK_WOOD : v === 3 ? [0x8a6a44, 0xb08a5a, 0xc8a274] : WOOD;
  const depth = opt === 'B' ? 0.07 : 0.18;
  const r = rect(0.08, top, len - 0.08, top + depth);
  castShadow(g, quad(r, rnd, 0), light, opt === 'B' ? 0.06 : 0.04);
  // The pegs: stubs from the wall at each end, past the board.
  for (const u of [0.24, len - 0.24]) g.moveTo(...at(u, top - 0.04)).lineTo(...at(u, top + depth + 0.035));
  g.stroke({ width: 0.05, color: INK, cap: 'round' });
  for (const u of [0.24, len - 0.24]) g.moveTo(...at(u, top - 0.04)).lineTo(...at(u, top + depth + 0.035));
  g.stroke({ width: 0.03, color: DARK_WOOD[2], cap: 'round' });
  const face = board(g, r, wood, light, rnd, along, opt === 'B' ? 0.02 : 0.035, 0);
  const PAPER = [LINEN[2], LINEN[1], 0xe8d8a8, 0xd8c8a0];
  if (opt === 'B') {
    // Paper corners: short light ticks off the board's room edge, at odd angles.
    for (let i = 0; i < 9; i++) {
      const u = 0.18 + ((len - 0.36) * (i + 0.2 + rnd() * 0.6)) / 9;
      const s = top + depth;
      const a = (rnd() - 0.5) * 0.06;
      g.moveTo(...at(u, s - 0.008)).lineTo(...at(u + a, s + 0.03 + rnd() * 0.025));
      g.stroke({ width: 0.022, color: PAPER[i % 4], cap: 'round' });
    }
    // And the odd one curling over the top edge.
    for (const u of [len * 0.38, len * 0.71]) g.poly(quad(rect(u, top + 0.012, u + 0.09, top + 0.04), rnd, 0.004)).fill(LINEN[2]).stroke({ width: 0.014, color: INK });
  } else {
    planks(g, face, along, 2, wood[0], rnd);
    // Papers on the face, foreshortened to slivers, a pin at each top.
    for (let i = 0; i < 6; i++) {
      const u = 0.2 + ((len - 0.4) * (i + rnd() * 0.3)) / 6;
      const w = 0.16 + rnd() * 0.06;
      const s = top + 0.04 + rnd() * 0.04;
      const p = quad(rect(u, s, u + w, s + 0.05 + rnd() * 0.04), rnd, 0.006);
      g.poly(p).fill(PAPER[i % 4]).stroke({ width: 0.014, color: INK, join: 'round' });
    }
  }
  g.poly(quad(r, rnd, 0)).stroke({ width: LINE, color: INK, join: 'round' });
}

/** The stage's curtain, option B: a thin gathered band along the back from the rod, its folds in two reds. */
export function stageCurtainB(g: Graphics, at: At, light: Light, len: number) {
  const n = Math.round(len * 7);
  const cols: [number, number][] = Array.from({ length: n }, (_, i) => [i % 2 ? 0x9a3428 : 0x6e2420, 1]);
  hangingCloth(g, at, light, 0.1, len - 0.1, 0.075, 0.12, 0, cols, { ticks: false });
  // Tie-backs: the cloth gathered into a knot at each end, a gilt cord round it.
  for (const u of [0.16, len - 0.16]) {
    g.circle(...at(u, 0.14), 0.065).fill(0x7a2a22).stroke({ width: LINE, color: INK });
    g.moveTo(...at(u - 0.06, 0.14)).lineTo(...at(u + 0.06, 0.14)).stroke({ width: 0.022, color: GILT[1] });
  }
  hangingRod(g, at, 0.07, len - 0.07, 0.065);
}

/** The throne dais's cloth of estate, option B: a thin gathered band of blue powdered with gilt behind the seats. */
export function clothOfEstateB(g: Graphics, at: At, light: Light, c0: number, c1: number, top: number) {
  const hem = hangingCloth(g, at, light, c0, c1, top + 0.015, 0.13, 0, [[0x2e4a82, 1]], { ink: OUT * 0.75 });
  for (let u = c0 + 0.12; u < c1 - 0.08; u += 0.2) {
    const [x, y] = at(u + 0.08, top + 0.015 + hem(u) * 0.5);
    g.poly([x, y - 0.022, x + 0.018, y, x, y + 0.022, x - 0.018, y]).fill(GILT[2]);
  }
  hangingRod(g, at, c0 - 0.06, c1 + 0.06, top);
}
