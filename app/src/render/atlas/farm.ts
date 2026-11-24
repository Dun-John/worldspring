import { PX } from '../scale';
import { OUT, LINE } from '../pen';
import { Graphics } from 'pixi.js';
import { stockadeStakes, fenceGate } from '../walls';
import { INK, WOOD, oval, shade } from './common';
import { leaf, lit, tuft } from './plants';
import { flower5 } from './garden';
import { bevel } from './yard';
import { BARK } from './stones';
import { endGrain } from './wood';

// v63 castles: a castle's kitchen garden (rows, a wattle fence, skeps) and its smithy's anvil.

export { OUT, LINE };

/** A round leafy head (a cabbage, a herb clump) at (x, y): lobes inked as one, dark under, lit toward
 * the NW, a few lobes picked out lighter. */
function leafHead(g: Graphics, x: number, y: number, s: number, n: number, tones: readonly number[], rnd: () => number) {
  const lobes: [number, number, number][] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rnd() * 0.6;
    lobes.push([x + Math.cos(a) * s * 0.5, y + Math.sin(a) * s * 0.5, s * (0.48 + 0.1 * rnd())]);
  }
  for (const [lx, ly, lr] of lobes) g.circle(lx, ly, lr + LINE).fill(INK);
  for (const [lx, ly, lr] of lobes) g.circle(lx, ly, lr).fill(tones[0]);
  for (const [lx, ly, lr] of lobes) g.circle(lx - lr * 0.16, ly - lr * 0.16, lr * 0.74).fill(tones[1]);
  for (const [lx, ly, lr] of lobes) if (lx + ly < x + y) g.circle(lx - lr * 0.3, ly - lr * 0.3, lr * 0.34).fill({ color: tones[2], alpha: 0.85 });
}

/** A bed of dug earth along x, half `L` long and `W` wide: dark soil inked round, lit along its
 * north side, a furrow's shade along its south one, a few crumbs. */
function ridge(g: Graphics, L: number, W: number, rnd: () => number) {
  const earth = [0x3e2a1c, 0x563c28, 0x6e5038];
  g.roundRect(-L, -W, L * 2, W * 2, W * 0.7).fill(earth[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.roundRect(-L + 4, -W + 3, L * 2 - 8, W * 0.6, W * 0.3).fill({ color: earth[2], alpha: 0.85 });
  g.roundRect(-L + 4, W * 0.3, L * 2 - 8, W * 0.55, W * 0.27).fill({ color: earth[0], alpha: 0.8 });
  for (let i = 0; i < 8; i++) g.circle(-L + 6 + rnd() * (L * 2 - 12), (rnd() - 0.5) * W * 1.3, 1.2 + rnd()).fill({ color: rnd() < 0.5 ? earth[2] : earth[0], alpha: 0.9 });
}

/** A kitchen garden's row, along x (two squares long): a bed of dark earth, a few big crops along it.
 * Cabbages, one red among them (v0); leeks in fans (v1); herbs, sage, parsley and chives in lilac
 * flower (v2); carrots' feathery tops, orange shoulders showing (v3). */
export function vegRow(g: Graphics, r: number, rnd: () => number, v: number) {
  const [L, W] = [r * 0.94, 0.34 * PX];
  ridge(g, L, W, rnd);
  const at = (i: number, n: number, j = 2) => [-L + ((i + 0.5) / n) * L * 2 + (rnd() - 0.5) * j, (rnd() - 0.5) * j] as const;
  if (v === 0) {
    const n = 4;
    const red = Math.floor(rnd() * n);
    for (let i = 0; i < n; i++) {
      const [x, y] = at(i, n);
      const tones = i === red ? [0x5a3448, 0x7a4a62, 0x9a6a80] : [0x4e7a3e, 0x6c9c50, 0x94bc6a];
      leafHead(g, x, y, 0.25 * PX, 6, tones, rnd);
      const h = 0.12 * PX;
      g.circle(x - 1, y - 1, h).fill(i === red ? 0xae7e94 : 0xa6c47e).stroke({ width: LINE * 0.75, color: INK });
      g.moveTo(x - h * 0.5, y - h * 0.1).quadraticCurveTo(x - h * 0.1, y - h * 0.6, x + h * 0.4, y - h * 0.4).stroke({ width: 1.2, color: tones[0], cap: 'round' });
    }
    return;
  }
  if (v === 1) {
    const n = 5;
    const tones = [0x4a7a44, 0x62944e, 0x80ae66];
    for (let i = 0; i < n; i++) {
      const [x, y] = at(i, n);
      for (let k = 0; k < 4; k++) {
        const a = (k % 2 ? Math.PI / 2 : -Math.PI / 2) + (k < 2 ? -0.4 : 0.4) + (rnd() - 0.5) * 0.3;
        leaf(g, x, y, a, 0.36 * PX * (0.85 + 0.2 * rnd()), 6.5, shade(tones[k % 3], 1 + 0.12 * lit(a)), 0.4, (rnd() - 0.5) * 0.25);
      }
      g.circle(x, y, 3.4).fill(0xe0e0c8).stroke({ width: LINE * 0.75, color: INK });
    }
    return;
  }
  if (v === 2) {
    const n = 3;
    for (let i = 0; i < n; i++) {
      const [x, y] = at(i, n, 3);
      const kind = (i + Math.floor(rnd() * 2)) % 3;
      if (kind === 0) {
        // Sage: soft grey-green leaves, rounded.
        leafHead(g, x, y, 0.28 * PX, 7, [0x627a56, 0x80986e, 0xa4b48c], rnd);
      } else if (kind === 1) {
        // Parsley: a tight curly clump.
        leafHead(g, x, y, 0.27 * PX, 8, [0x3e6e2a, 0x5a903a, 0x7eac56], rnd);
      } else {
        // Chives: fine blades, a few lilac heads.
        tuft(g, x, y, 9, 0.3 * PX, 4.6, [0x4e7e3a, 0x60904a, 0x74a456], rnd);
        for (let k = 0; k < 3; k++) {
          const a = rnd() * Math.PI * 2;
          const [fx, fy] = [x + Math.cos(a) * 0.1 * PX, y + Math.sin(a) * 0.1 * PX];
          g.circle(fx, fy, 3.6).fill(0xb49ccc).stroke({ width: LINE * 0.75, color: INK });
          g.circle(fx - 1, fy - 1, 1.4).fill(0xd6c8e4);
        }
      }
    }
    return;
  }
  const n = 5;
  const show = new Set([Math.floor(rnd() * 2), 2 + Math.floor(rnd() * 3)]);
  for (let i = 0; i < n; i++) {
    const [x, y] = at(i, n);
    if (show.has(i)) g.ellipse(x + 2, y + 3, 4.6, 3.8).fill(0xc8743a).stroke({ width: LINE * 0.75, color: INK });
    tuft(g, x, y, 7, 0.3 * PX, 5.6, [0x5e923f, 0x72a44c, 0x90b862], rnd);
  }
}

/** A piece of wattle fence along x, five feet long so pieces set end to end read as one fence: pale
 * withies inked along both sides (not across its ends), a stake ticked across it every few inches,
 * one light zig-zag of weave between them. Fresh hazel (v0), older darker withies (v1), bindweed in
 * flower on it (v2), a broken withy sticking out (v3). */
export function wattleFence(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const wood = v === 1 ? [0x7a5c3a, 0x9a7a50, 0xbc9c6c] : [0x8c6e46, 0xb08e5c, 0xd4b67e];
  const [L, T] = [0.5 * PX, 0.2 * (PX / 5)];
  g.rect(-L, -T, L * 2, T * 2).fill(wood[1]);
  g.rect(-L, -T + 0.8, L * 2, T * 0.6).fill(wood[2]);
  // The stakes, about every 0.12 of a square, standing proud of the weave either side.
  const n = 8;
  const step = (L * 2) / n;
  for (let i = 0; i < n; i++) {
    const x = -L + (i + 0.5) * step;
    g.moveTo(x, -T - 1).lineTo(x, T + 1).stroke({ width: 2.6, color: INK, cap: 'round' });
    g.moveTo(x, -T - 0.2).lineTo(x, T + 0.2).stroke({ width: 1.2, color: wood[1], cap: 'round' });
  }
  // The weave: one withy zig-zagging over and under, stake to stake.
  g.moveTo(-L, 0);
  for (let i = 0; i <= n; i++) g.lineTo(Math.min(L, -L + i * step), (i % 2 ? -1 : 1) * T * 0.45);
  g.stroke({ width: 1.4, color: shade(wood[2], 1.12), join: 'round' });
  for (const y of [-T, T]) g.moveTo(-L, y).lineTo(L, y);
  g.stroke({ width: LINE, color: INK });
  if (v === 2) {
    for (let i = 0; i < 4; i++) {
      const x = -L + 6 + rnd() * (L * 2 - 12);
      leaf(g, x, (rnd() < 0.5 ? -1 : 1) * T * 0.7, (rnd() - 0.5) * Math.PI * 2, 5, 2.2, [0x4e7a34, 0x5e8a40][i % 2], 0.4);
    }
    flower5(g, -L * 0.3 + rnd() * L * 0.6, -T * 0.3, 3.4, 0xf2eee0, 0xd8c060, rnd());
  }
  if (v === 3) {
    const x = -L * 0.4 + rnd() * L * 0.8;
    g.moveTo(x, T * 0.4).quadraticCurveTo(x + 3, T + 2, x + 7, T + 3).stroke({ width: 3, color: INK, cap: 'round' });
    g.moveTo(x, T * 0.4).quadraticCurveTo(x + 3, T + 2, x + 7, T + 3).stroke({ width: 1.4, color: wood[2], cap: 'round' });
  }
}

// R6 fences: each a five-foot piece along x, set end to end like `wattleFence`, seen straight down. (A low
// stone wall is drawn as the countryside's dry-stone wall, a high wall as a manor's garden wall; the
// stockade's stakes and the gate's leaves are the palisade's and the forecourt gate's, `walls.ts`.)

/** A straight line inked twice: a dark line `w + 2.4` wide under one of `color`, so a rail or bar has an
 * outline along both sides but none across its ends (pieces end to end read as one rail). */
function railLine(g: Graphics, x0: number, y0: number, x1: number, y1: number, w: number, color: number, ink = 2.4) {
  g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: w + ink, color: INK });
  g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: w, color });
}

/** A small square top seen from above at (x, y), `w` by `d`, turned `t`: `tones` dark, mid, light; the
 * light in its north-west corner, inked. */
function topBlock(g: Graphics, x: number, y: number, w: number, d: number, t: number, tones: readonly number[], ink: number) {
  const [c, s] = [Math.cos(t), Math.sin(t)];
  const at = (a: number, b: number) => [x + a * c - b * s, y + a * s + b * c];
  const quad = (a0: number, b0: number, a1: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  g.poly(quad(-w / 2, -d / 2, w / 2, d / 2)).fill(tones[1]);
  g.poly(quad(-w / 2, -d / 2, w * 0.1, -d * 0.05)).fill(tones[2]);
  g.poly(quad(-w / 2, -d / 2, w / 2, d / 2)).stroke({ width: ink, color: INK, join: 'round' });
}

/** Picket fence along x from above: the rail along its inner face, six palings' tops in a row along it and a
 * post's square top at the middle. Whitewashed (v0), weathered cream with the paint flaking (v1), sage
 * green (v2), white with one paling gone and the next knocked askew (v3). */
export function picketFence(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const paint = [
    [0xb4b0a2, 0xeae6d8, 0xfdfbf2],
    [0xa49a80, 0xd6ccae, 0xece2c4],
    [0x627e5e, 0x92b088, 0xc0d8b2],
    [0xb4b0a2, 0xeae6d8, 0xfdfbf2],
  ][v];
  const L = 0.5 * PX;
  // The rail runs the whole piece under the palings, so the pieces read as one continuous fence.
  railLine(g, -L, 0, L, 0, 2.4, v === 2 ? 0x7a6a4a : WOOD[1], LINE);
  const n = 7;
  const step = (L * 2) / n;
  const gone = v === 3 ? 1 + Math.floor(rnd() * 5) : -1;
  for (let i = 0; i < n; i++) {
    if (i === gone) continue;
    const x = -L + (i + 0.5) * step;
    const askew = v === 3 && i === gone + 1 ? 0.5 : 0;
    topBlock(g, x, 0, step - 3.6, 3.8, askew, paint, LINE);
    if (v === 1 && rnd() < 0.5) g.circle(x + (rnd() - 0.5) * 2, -0.4, 0.7).fill(WOOD[2]);
  }
  topBlock(g, 0, 0, 5.4, 5.4, 0, paint.map((c) => shade(c, 0.9)), LINE);
}

/** Iron railing along x from above: its top rail a dark bar, six finials' round tops along it (the bars'
 * spear heads seen end on) and a post's square cap with its ball at the middle, in blue-grey iron. Plain
 * (v0), weathered paler (v1), brass-capped finials (v2), one bar bent out of line (v3). */
export function ironRailing(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const iron = v === 1 ? [0x4a5058, 0x7a828c, 0xb0b8c0] : [0x30343c, 0x5e6672, 0x98a2ae];
  const tip = v === 2 ? [0x5e5434, 0x9a8a5a, 0xc4b688] : iron;
  const L = 0.5 * PX;
  // Two rails the whole piece long (the top one broad), a round finial on every bar, a post cap at the middle.
  railLine(g, -L, -1.1, L, -1.1, 1.2, iron[1], LINE);
  railLine(g, -L, 0.4, L, 0.4, 2.0, iron[1], LINE);
  g.moveTo(-L, 0).lineTo(L, 0).stroke({ width: 0.6, color: iron[2] });
  const n = 8;
  const step = (L * 2) / n;
  const bent = v === 3 ? Math.floor(rnd() * 2) : -1;
  for (let i = 0; i < n; i++) {
    const x = -L + (i + 0.5) * step;
    const y = i === bent ? 2.6 : 0;
    if (y) g.moveTo(x, 0).lineTo(x, y).stroke({ width: 2.4, color: INK, cap: 'round' });
    if (i % 2 && i !== bent) continue;
    g.circle(x, y, 1.9).fill(tip[1]).stroke({ width: 1.2, color: INK });
    g.circle(x - 0.5, y - 0.5, 0.6).fill(tip[2]);
  }
  topBlock(g, 0, 0, 5, 5, 0, iron, LINE);
  g.circle(0, 0, 2).fill(tip[1]).stroke({ width: 1.1, color: INK });
  g.circle(-0.5, -0.5, 0.6).fill(tip[2]);
}

/** Stockade along x from above: a palisade's stakes (`stockadeStakes`). */
export function stockade(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  void v;
  stockadeStakes(g, rnd);
}

/** Gate, five feet along x from above: the forecourt gate's leaves swung open on two posts (`fenceGate`):
 * timber (v0), wrought iron (v1), heavier timber (v2, a stockade's). */
export function gate(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  fenceGate(g, v % 3, rnd);
}

/** A straw skep from above at (x, y), `R` round: a straw-gold dome, a crescent of shade on the SE, a
 * light on the NW shoulder, two or three coils of its straw rope as arcs, its doorway at the foot
 * (south). */
function skep(g: Graphics, x: number, y: number, R: number, rnd: () => number) {
  const straw = [0xb8964a, 0xe6cc84, 0xf5e4a8];
  const toned = 0x8a6a30;
  // From straight above: a pale straw dome, three bold coil rings as toned lines with stitch ticks across
  // them, a knob at the crown, the doorway bulging at one rim, bees about it.
  const notch = Math.PI * 0.5 + (rnd() - 0.5) * 0.7;
  g.circle(x, y, R).fill(straw[1]).stroke({ width: LINE, color: INK });
  g.circle(x + R * 0.14, y + R * 0.14, R * 0.92).fill({ color: straw[0], alpha: 0.5 });
  g.circle(x - R * 0.08, y - R * 0.08, R * 0.84).fill(straw[1]);
  g.circle(x - R * 0.26, y - R * 0.26, R * 0.46).fill({ color: straw[2], alpha: 0.7 });
  const radii = [0.8, 0.56, 0.32];
  radii.forEach((f, i) => g.circle(x - R * 0.03 * i, y - R * 0.03 * i, R * f).stroke({ width: Math.max(1.4, R * 0.1), color: toned }));
  for (let i = 0; i < 3; i++) {
    const n = 9 - i * 2;
    const r0 = R * [0.64, 0.41, 0.2][i];
    const r1 = R * [0.74, 0.51, 0.28][i];
    for (let k = 0; k < n; k++) {
      const a = ((k + (i % 2) * 0.5) / n) * Math.PI * 2;
      const [c, s] = [Math.cos(a), Math.sin(a)];
      g.moveTo(x + c * r0, y + s * r0).lineTo(x + c * r1, y + s * r1);
    }
  }
  g.stroke({ width: 1.1, color: toned, alpha: 0.9, cap: 'round' });
  g.circle(x, y, R * 0.17).fill(straw[2]).stroke({ width: 1.4, color: toned });
  g.circle(x - R * 0.04, y - R * 0.04, R * 0.07).fill(straw[0]);
  const [nx, ny] = [x + Math.cos(notch) * R * 0.97, y + Math.sin(notch) * R * 0.97];
  g.poly(oval(nx, ny, R * 0.3, R * 0.18, notch + Math.PI / 2, 10)).fill(0x1c1208).stroke({ width: 1.2, color: INK });
  for (let i = 0; i < 2; i++) {
    const a = notch + (rnd() - 0.5) * 1.2;
    const d = R * (1.15 + 0.2 * rnd());
    g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, 1.2).fill(0x3a2a10);
  }
}

/** Beehives: a straw skep on a plank (v0); two on a longer plank (v1); one on a sawn stump (v2);
 * one bigger, set on the ground (v3). */
export function beehive(g: Graphics, r: number, rnd: () => number, v: number) {
  const board = [0x5e5a52, 0x8a857a, 0xaaa496];
  if (v === 1) {
    bevel(g, -r * 1.0, -r * 0.55, r * 1.0, r * 0.55, 2, board, LINE);
    skep(g, -r * 0.48, 0, r * 0.44, rnd);
    skep(g, r * 0.48, 0, r * 0.44, rnd);
    return;
  }
  // A square grey-wood board showing on all sides, its planks lined.
  const h = r * (v === 3 ? 1.0 : 0.9);
  bevel(g, -h, -h, h, h, 2, board, LINE);
  g.moveTo(-h * 0.98, -h * 0.34).lineTo(h * 0.98, -h * 0.34).moveTo(-h * 0.98, h * 0.34).lineTo(h * 0.98, h * 0.34);
  g.stroke({ width: 1.1, color: board[0], alpha: 0.8 });
  skep(g, 0, 0, r * (v === 3 ? 0.78 : v === 2 ? 0.6 : 0.68), rnd);
}
