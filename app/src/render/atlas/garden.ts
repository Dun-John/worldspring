import { Graphics } from 'pixi.js';
import { PX } from '../scale';
import { FT, INK, IRON, WOOD, oval, shade } from './common';
import { UPRIGHT_FOOT, footGrass, inked, rod, upright } from './street';
import { BARK, STONE, moss } from './stones';
import { bevel } from './yard';
import { endGrain } from './wood';
import { boughs, leaf, lit, tuft, whorl } from './plants';
import { smooth } from './water';

/** A plank from above, along x from `x0` to `x1` and `y0` to `y1`: inked, lit along its north edge, a line
 * of grain. */
export function plank(g: Graphics, x0: number, y0: number, x1: number, y1: number, tones: readonly number[], rnd: () => number) {
  g.rect(x0, y0, x1 - x0, y1 - y0).fill(tones[1]).stroke({ width: 2, color: INK, join: 'round' });
  g.rect(x0 + 1, y0 + 1, x1 - x0 - 2, Math.min(3, (y1 - y0) * 0.3)).fill(tones[2]);
  const y = y0 + (y1 - y0) * (0.55 + 0.2 * rnd());
  g.moveTo(x0 + (x1 - x0) * 0.12, y).lineTo(x0 + (x1 - x0) * (0.45 + 0.2 * rnd()), y).stroke({ width: 1, color: tones[0], alpha: 0.7, cap: 'round' });
}

/** Bench, along x, its front toward +y: two planks on timber trestles (v0); a park bench with a slatted back
 * and iron scroll ends (v1); a stone slab on two blocks with moss on it (v2); a split log on two stumps
 * (v3). */
export function bench(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const L = 2.45 * FT;
  if (v === 0) {
    for (const x of [-L + 0.5 * FT, L - 0.5 * FT]) g.rect(x - 0.2 * FT, -0.95 * FT, 0.4 * FT, 1.9 * FT).fill(WOOD[0]).stroke({ width: 2, color: INK });
    plank(g, -L, -0.72 * FT, L, -0.04 * FT, WOOD, rnd);
    plank(g, -L + 2, 0.04 * FT, L - 1, 0.72 * FT, WOOD, rnd);
  } else if (v === 1) {
    // The back leans away to the north: its slats seen from above, paler, its shadow on the seat; the
    // iron ends sweep from the back down into scrolled arms at the front.
    g.rect(-L, -1.05 * FT, L * 2, 0.5 * FT).fill({ color: 0x2a1a10, alpha: 0.45 });
    for (const [y0, y1] of [
      [-0.62 * FT, -0.12 * FT],
      [-0.06 * FT, 0.38 * FT],
      [0.44 * FT, 0.86 * FT],
    ]) {
      plank(g, -L, y0, L, y1, WOOD, rnd);
    }
    for (const [y0, y1] of [
      [-1.62 * FT, -1.38 * FT],
      [-1.3 * FT, -1.06 * FT],
    ]) {
      plank(g, -L, y0, L, y1, WOOD.map((c) => shade(c, 1.18)), rnd);
    }
    for (const side of [-1, 1]) {
      const x = side * (L + 1);
      rod(g, [x, -1.7 * FT], [x, 0.8 * FT], 2.6);
      inked(g, () => g.moveTo(x, 0.8 * FT).quadraticCurveTo(x, 1.15 * FT, x + side * 0.28 * FT, 1.1 * FT).quadraticCurveTo(x + side * 0.42 * FT, 0.95 * FT, x + side * 0.2 * FT, 0.86 * FT), 2.6, IRON[2]);
    }
  } else if (v === 2) {
    const st = STONE[1];
    for (const x of [-L + 0.6 * FT, L - 0.6 * FT]) bevel(g, x - 0.42 * FT, -1.0 * FT, x + 0.42 * FT, 1.0 * FT, 3, st, 2);
    bevel(g, -L, -0.78 * FT, L, 0.78 * FT, 4, st.map((c) => shade(c, 1.08)), 2.5);
    g.moveTo(-L * 0.1, -0.78 * FT + 2).lineTo(-L * 0.05, -0.2 * FT).lineTo(L * 0.05, 0.1 * FT).stroke({ width: 1.4, color: shade(st[0], 0.8), join: 'round' });
    moss(g, L * 0.62, -0.45 * FT, 0.32 * FT, rnd);
  } else {
    const bark = BARK[0];
    // Two short logs laid across under the ends, their cut ends showing front and back.
    for (const x of [-L + 0.7 * FT, L - 0.7 * FT]) {
      g.roundRect(x - 0.36 * FT, -1.0 * FT, 0.72 * FT, 2.0 * FT, 0.3 * FT).fill(bark[1]).stroke({ width: 2, color: INK });
      g.moveTo(x - 0.15 * FT, -0.75 * FT).lineTo(x - 0.15 * FT, 0.75 * FT).stroke({ width: 1.5, color: bark[2], alpha: 0.7 });
      for (const y of [-1.0 * FT, 1.0 * FT]) endGrain(g, x, y, 0.34 * FT, 0.2 * FT, bark);
    }
    // The half log: bark showing along its rounded sides, the split face flat on top.
    g.roundRect(-L, -0.5 * FT, L * 2, 1.0 * FT, 0.3 * FT).fill(bark[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let i = 0; i < 8; i++) {
      const x = -L + 0.4 * FT + rnd() * (L * 2 - 0.8 * FT);
      const y = (i % 2 ? 1 : -1) * 0.4 * FT;
      g.moveTo(x, y).lineTo(x + 5 + rnd() * 6, y).stroke({ width: 1.5, color: shade(bark[0], 0.7), cap: 'round' });
    }
    g.roundRect(-L + 3, -0.3 * FT, L * 2 - 6, 0.6 * FT, 0.2 * FT).fill(0xc89a68).stroke({ width: 1.2, color: shade(bark[0], 0.8) });
    g.moveTo(-L + 0.5 * FT, -0.05 * FT).quadraticCurveTo(0, 0.12 * FT, L - 0.6 * FT, -0.08 * FT).stroke({ width: 1, color: 0x9a6a40, alpha: 0.8 });
    g.moveTo(-L + 0.8 * FT, -0.2 * FT).lineTo(-L * 0.2, -0.18 * FT).stroke({ width: 1, color: 0xe0b47a, alpha: 0.9 });
  }
}

/** A flower seen from above: five petals inked together, lit on the NW, a small eye. */
export function flower5(g: Graphics, x: number, y: number, s: number, petal: number, eye: number, ph: number) {
  const petals = [0, 1, 2, 3, 4].map((p) => ph + (p / 5) * Math.PI * 2);
  for (const pa of petals) g.circle(x + Math.cos(pa) * s * 0.5, y + Math.sin(pa) * s * 0.5, s * 0.5 + 1).fill(INK);
  for (const pa of petals) g.circle(x + Math.cos(pa) * s * 0.5, y + Math.sin(pa) * s * 0.5, s * 0.5).fill(shade(petal, 1 + 0.15 * lit(pa)));
  g.circle(x, y, s * 0.22).fill(eye);
}

/** A rose bush from above: soft round lobes of dark leaves (lit toward the light), two or three roses on
 * it, each a curled bloom. */
function roseBush(g: Graphics, x: number, y: number, s: number, petal: number, rnd: () => number) {
  const lobes: [number, number, number][] = [[x, y, s * 0.55]];
  for (let i = 0; i < 5; i++) {
    const a = (i / 5) * Math.PI * 2 + rnd() * 0.8;
    const d = s * (0.4 + 0.15 * rnd());
    lobes.push([x + Math.cos(a) * d, y + Math.sin(a) * d, s * (0.4 + 0.12 * rnd())]);
  }
  for (const [lx, ly, lr] of lobes) g.circle(lx, ly, lr + 1.8).fill(INK);
  for (const [lx, ly, lr] of lobes) g.circle(lx, ly, lr).fill(0x2e4a24);
  for (const [lx, ly, lr] of lobes) g.circle(lx - lr * 0.18, ly - lr * 0.18, lr * 0.72).fill(0x46682e);
  for (const [lx, ly, lr] of lobes) g.circle(lx - lr * 0.32, ly - lr * 0.32, lr * 0.32).fill({ color: 0x6a8c3e, alpha: 0.85 });
  for (let i = 0; i < 2 + Math.floor(rnd() * 2); i++) {
    const a = rnd() * Math.PI * 2;
    const [rx, ry] = [x + Math.cos(a) * s * 0.42, y + Math.sin(a) * s * 0.42];
    g.circle(rx, ry, s * 0.34).fill(petal).stroke({ width: 1.3, color: INK });
    g.moveTo(rx + s * 0.18, ry).arc(rx, ry, s * 0.18, 0, Math.PI * 1.5).stroke({ width: 1.3, color: shade(petal, 0.6), cap: 'round' });
    g.circle(rx - s * 0.12, ry - s * 0.12, s * 0.08).fill({ color: 0xffffff, alpha: 0.5 });
  }
}

/** Dark soil in an outline, crumbs of lighter earth. */
function soil(g: Graphics, pts: number[], rnd: () => number, s: number) {
  g.poly(pts).fill(0x4a3626).stroke({ width: 1.5, color: 0x2a1c12, join: 'round' });
  const xs = pts.filter((_, i) => i % 2 === 0);
  const ys = pts.filter((_, i) => i % 2 === 1);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  for (let i = 0; i < 14; i++) g.circle(x0 + (x1 - x0) * (0.1 + 0.8 * rnd()), y0 + (y1 - y0) * (0.1 + 0.8 * rnd()), s * (0.02 + 0.02 * rnd())).fill({ color: 0x6a5038, alpha: 0.8 });
}

/** Flowerbed, along x: a round bed of roses inside a clipped box edging (v0); tulips in rows, red and
 * yellow, in a timber kerb (v1); a raised bed in a stone kerb, lavender behind and marigolds in front (v2);
 * a square knot of box, four beds of flowers between its diagonals, a clipped ball in the middle (v3). */
export function flowerbed(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v === 0) {
    // The edging: one band of clipped box, lumpy, lit on its NW side, leaves picked out.
    const R = r * 0.92;
    const n = 30;
    const jit = Array.from({ length: n }, () => rnd() - 0.5);
    const band = (k: number, amp: number, d = 0) => jit.flatMap((j, i) => [Math.cos((i / n) * Math.PI * 2) * (R * k + j * amp) + d, Math.sin((i / n) * Math.PI * 2) * (R * k + j * amp) + d]);
    smooth(g, band(1, 3.5));
    g.fill(0x2e4a24).stroke({ width: 2.2, color: INK, join: 'round' });
    smooth(g, band(0.96, 3, -1.2));
    g.fill(0x46682e);
    for (let i = 0; i < 26; i++) {
      const a = rnd() * Math.PI * 2;
      const d = R * (0.86 + 0.08 * rnd());
      g.circle(Math.cos(a) * d, Math.sin(a) * d, 1.6 + rnd() * 1.4).fill({ color: lit(a) > 0 ? 0x6e9440 : 0x24401c, alpha: 0.8 });
    }
    soil(g, band(0.8, 2.5), rnd, r);
    // Rose bushes at odd places and sizes, red and pink.
    const ph = rnd() * Math.PI * 2;
    for (let i = 0; i < 5; i++) {
      const a = ph + (i / 5) * Math.PI * 2 + (rnd() - 0.5) * 0.8;
      const d = R * (i === 4 ? 0.12 : 0.36 + 0.18 * rnd());
      roseBush(g, Math.cos(a) * d, Math.sin(a) * d, r * (0.17 + 0.07 * rnd()), i % 2 ? 0xe88aa0 : 0xc8323a, rnd);
    }
    return;
  }
  if (v === 3) {
    // The knot: a square of clipped box with its diagonals, four beds of one flower each between them.
    const S = r * 0.8;
    const beds: [number[], number][] = [
      [[-S, -S, S, -S, 0, 0], 0xd8323a],
      [[S, -S, S, S, 0, 0], 0xf6efe0],
      [[S, S, -S, S, 0, 0], 0xf2c94c],
      [[-S, S, -S, -S, 0, 0], 0x7a7ad8],
    ];
    for (const [tri, color] of beds) {
      soil(g, tri, rnd, r);
      const [cx, cy] = [(tri[0] + tri[2]) / 3, (tri[1] + tri[3]) / 3];
      for (let i = 0; i < 9; i++) {
        const [u, w] = [rnd(), rnd()];
        const [a, b] = u + w > 1 ? [1 - u, 1 - w] : [u, w];
        const x = tri[0] + (tri[2] - tri[0]) * a + (0 - tri[0]) * b;
        const y = tri[1] + (tri[3] - tri[1]) * a + (0 - tri[1]) * b;
        flower5(g, x * 0.86 + cx * 0.14, y * 0.86 + cy * 0.14, r * 0.1, color, color === 0xf2c94c ? 0xc8702a : 0xf2c94c, rnd());
      }
    }
    const boxLine = (pts: number[], close: boolean) => {
      for (const [width, color] of [
        [r * 0.17 + 3, INK],
        [r * 0.17, 0x2e4a24],
        [r * 0.08, 0x46682e],
      ] as const) {
        g.poly(pts, close).stroke({ width, color, join: 'round', cap: 'round' });
      }
    };
    boxLine([-S, -S, S, S], false);
    boxLine([S, -S, -S, S], false);
    boxLine([-S, -S, S, -S, S, S, -S, S], true);
    g.moveTo(-S + 2, -S - r * 0.04).lineTo(S - 2, -S - r * 0.04).moveTo(-S - r * 0.04, -S + 2).lineTo(-S - r * 0.04, S - 2).stroke({ width: 2, color: 0x6a8c3e, cap: 'round' });
    g.circle(0, 0, r * 0.17).fill(0x2e4a24).stroke({ width: 2, color: INK });
    g.circle(-r * 0.03, -r * 0.04, r * 0.12).fill(0x46682e);
    g.circle(-r * 0.06, -r * 0.07, r * 0.06).fill(0x6a8c3e);
    return;
  }
  const [W, H] = v === 1 ? [r * 0.95, r * 0.5] : [r * 0.92, r * 0.56];
  const d = v === 1 ? 4 : 7;
  bevel(g, -W, -H, W, H, d, v === 1 ? WOOD : STONE[0], 2.2);
  if (v === 2) {
    for (const x of [-W * 0.45, W * 0.15, W * 0.7]) g.moveTo(x, -H + 1).lineTo(x, -H + d).moveTo(x - 6, H - d).lineTo(x - 6, H - 1);
    g.stroke({ width: 1.2, color: shade(STONE[0][0], 0.85) });
  }
  soil(g, [-W + d, -H + d, W - d, -H + d, W - d, H - d, -W + d, H - d], rnd, r);
  if (v === 1) {
    // Tulips in loose rows (a few gone): long leaves out to the sides, the cup seen a little from the side
    // (they only read so), three petal tips at its mouth.
    const [cols, sx, sy] = [6, ((W - d) * 2) / 6, ((H - d) * 2) / 3];
    for (let row = 0; row < 3; row++) {
      for (let c = 0; c < cols; c++) {
        if (rnd() < 0.15) continue;
        const x = -W + d + (c + 0.5) * sx + (rnd() - 0.5) * 0.55 * sx;
        const y = -H + d + (row + 0.5) * sy + (rnd() - 0.5) * 0.4 * sy;
        const a = (rnd() < 0.5 ? 0 : Math.PI) + (rnd() - 0.5) * 0.9;
        leaf(g, x, y + 2, a, r * 0.2, r * 0.04, 0x5a8a3a, 0.4);
        leaf(g, x, y + 2, a + Math.PI + (rnd() - 0.5) * 0.6, r * 0.17, r * 0.04, 0x4e7a32, 0.4);
        const petal = row === 1 ? 0xf2c94c : 0xd8323a;
        const [w, h] = [r * 0.055, r * 0.085];
        g.poly([x - w, y - h * 0.15, x - w * 0.95, y - h * 0.8, x - w * 0.35, y - h * 0.45, x, y - h * 0.95, x + w * 0.35, y - h * 0.45, x + w * 0.95, y - h * 0.8, x + w, y - h * 0.15, x + w * 0.7, y + h * 0.3, x, y + h * 0.42, x - w * 0.7, y + h * 0.3])
          .fill(petal)
          .stroke({ width: 1.2, color: INK, join: 'round' });
        g.moveTo(x, y - h * 0.5).lineTo(x, y + h * 0.3).stroke({ width: 1, color: shade(petal, 0.65) });
        g.poly(oval(x - w * 0.5, y - h * 0.1, w * 0.18, h * 0.3, 0, 8)).fill({ color: 0xffffff, alpha: 0.4 });
      }
    }
    return;
  }
  // Lavender: spiky purple clumps in the back row, marigolds in front.
  for (let i = 0; i < 5; i++) {
    const [x, y] = [-W + d + ((i + 0.5) / 5) * (W - d) * 2, -H * 0.28];
    const rr = r * (0.2 + 0.03 * rnd());
    const tips = boughs(11, rnd).map(([t, k]): [number, number] => [t, k * (0.7 + 0.3 * rnd())]);
    whorl(g, x, y, rr, tips, 0.4);
    g.fill(0x5a6aa0).stroke({ width: 1.4, color: 0x2e3260, join: 'round' });
    whorl(g, x - rr * 0.1, y - rr * 0.1, rr * 0.65, tips, 0.4);
    g.fill(0x8a86c8);
    for (const [t, k] of tips.slice(0, 6)) g.circle(x + Math.cos(t) * rr * k * 0.8, y + Math.sin(t) * rr * k * 0.8, 1.6).fill(0xc0b8ec);
  }
  for (let i = 0; i < 6; i++) {
    const [x, y] = [-W + d + ((i + 0.5) / 6) * (W - d) * 2, H * 0.42];
    tuft(g, x, y, 5, r * 0.11, r * 0.03, [0x46682e, 0x5a7e36], rnd);
    flower5(g, x + (rnd() - 0.5) * 3, y - 2, r * 0.11, 0xe8902a, 0x8a4a1a, rnd());
  }
}

/** `a` blended `t` of the way toward `b`. */
function mix3(a: number, b: number, t: number): number {
  const ch = (k: number) => Math.round(((a >> k) & 255) * (1 - t) + ((b >> k) & 255) * t);
  return (ch(16) << 16) | (ch(8) << 8) | ch(0);
}

/** Fruit tree (an orchard's, v63 countryside): a smaller, rounder, lighter crown than the woods' trees,
 * a few fruit among the leaves (apple red v0, pear yellow v1, plum v2; v3 leaf only) and a short trunk
 * peeping out under it to the south with its own little shadow. About 11 ft across at scale 1. */
export function fruitTree(g: Graphics, r: number, rnd: () => number, v: number) {
  const [dark, mid, light] = [
    [0x3d5a26, 0x5a7e34, 0x9eb85a],
    [0x425e2a, 0x608238, 0xa6bc60],
    [0x3a5428, 0x56763a, 0x98ae62],
    [0x3e5c28, 0x5c8036, 0xa2b85a],
  ][v];
  const R = r * 1.1;
  // The trunk, short, under the crown's south side, and its shadow.
  g.moveTo(R * 0.12, R * 0.9).lineTo(R * 0.34, R * 0.98).stroke({ width: R * 0.1, color: 0x000000, alpha: 0.22, cap: 'round' });
  g.roundRect(-R * 0.06, R * 0.55, R * 0.13, R * 0.38, R * 0.04).fill(0x6a4a2e).stroke({ width: 1.8, color: INK });
  g.rect(-R * 0.035, R * 0.6, R * 0.04, R * 0.3).fill({ color: 0x8e6a44, alpha: 0.9 });
  const parts: [number, number, number][] = [];
  const n = 6;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rnd() * 0.5;
    const d = R * (0.3 + 0.08 * rnd());
    parts.push([Math.cos(a) * d, Math.sin(a) * d - R * 0.04, R * (0.48 + 0.08 * rnd())]);
  }
  parts.push([0, -R * 0.04, R * 0.62]);
  for (const [x, y, rr] of parts) g.circle(x, y, rr + 2.6).fill(INK);
  for (const [x, y, rr] of parts) g.circle(x, y, rr).fill(dark);
  for (const [x, y, rr] of parts) g.circle(x - rr * 0.16, y - rr * 0.16, rr * 0.8).fill(mid);
  for (const [x, y, rr] of parts) g.circle(x - rr * 0.34, y - rr * 0.34, rr * 0.4).fill({ color: light, alpha: 0.85 });
  // The crown's top (its north-west) a paler, yellower green, so it reads as a different tree from a round one.
  for (const [x, y, rr] of parts.slice(0, 3)) g.circle(x - rr * 0.38, y - rr * 0.4, rr * 0.3).fill({ color: mix3(light, 0xd4cc70, 0.45), alpha: 0.7 });
  // Leaf clumps: a few dark curls toward the shade, light flecks toward the light.
  for (let i = 0; i < 7; i++) {
    const a = 0.2 + rnd() * 2.2;
    const d = R * (0.35 + 0.4 * rnd());
    const [x, y] = [Math.cos(a) * d, Math.sin(a) * d];
    g.arc(x, y, R * 0.11, a - 1.2, a + 0.9).stroke({ width: 1.5, color: dark, alpha: 0.75 });
  }
  for (let i = 0; i < 8; i++) {
    const a = Math.PI + 0.3 + rnd() * 2.4;
    const d = R * (0.2 + 0.55 * rnd());
    g.circle(Math.cos(a) * d, Math.sin(a) * d, R * 0.045).fill({ color: shade(light, 1.15), alpha: 0.7 });
  }
  if (v === 3) return;
  // The fruit: dark under, the colour, a fleck of light.
  const [fd, fm, fl] = [
    [0x862a22, 0xb4402e, 0xd87a60],
    [0x8e7c30, 0xc4ae52, 0xe8d888],
    [0x3a2238, 0x5e3a5a, 0x92709a],
  ][v];
  const fr = R * 0.1;
  const count = 7 + Math.floor(rnd() * 3);
  for (let i = 0; i < count; i++) {
    const a = ((i + rnd() * 0.8) / count) * Math.PI * 2;
    const d = R * (0.25 + 0.55 * Math.sqrt(rnd()));
    const [x, y] = [Math.cos(a) * d, Math.sin(a) * d - R * 0.04];
    g.circle(x, y, fr).fill(fd);
    g.circle(x - fr * 0.2, y - fr * 0.2, fr * 0.74).fill(fm);
    g.circle(x - fr * 0.42, y - fr * 0.42, fr * 0.28).fill(fl);
  }
}

/** Water wheel (v63 countryside; turned: local x along the mill's wall, the mill toward +y), seen from
 * above: a timber wheel about 14 ft across and 4 ft broad, its two rims along x and the floats between
 * them, crowded toward the ends where the wheel curves down out of sight; the axle running out toward
 * the mill to its bearing block; a short timber race bringing the water over the top; soft white
 * foam where it churns the stream on the water side. */
export function waterWheel(g: Graphics, r: number) {
  const [X, Y] = [r * 0.96, r * 0.29];
  const WD = [0x5e4024, 0x7d5a3c, 0xb08a5a];
  // Its shadow on the water, to the south-east (the layer draws no round one).
  g.roundRect(-X + X * 0.07, -Y + Y * 0.45, 2 * X, 2 * Y, Y * 0.7).fill({ color: 0x000000, alpha: 0.14 });
  // Foam on the water side and round the ends: soft, no ink.
  const foam: [number, number, number, number][] = [
    [-0.42, -1.3, 0.16, 0.3],
    [-0.08, -1.4, 0.2, 0.32],
    [0.28, -1.3, 0.16, 0.3],
    [0.52, -1.15, 0.1, 0.26],
    [-0.64, -1.1, 0.09, 0.24],
    [-0.99, -0.3, 0.05, 0.6],
    [0.99, -0.1, 0.05, 0.6],
  ];
  for (const [x, y, rx, ry] of foam) g.ellipse(x * X, y * Y, rx * X, ry * Y * 1.6).fill({ color: 0xffffff, alpha: 0.2 });
  for (let i = 0; i < 5; i++) {
    const x = (-0.5 + i * 0.24) * X;
    g.arc(x, -Y * 1.12, Y * 0.3, Math.PI * 1.15, Math.PI * 1.85).stroke({ width: 1.6, color: 0xffffff, alpha: 0.5 });
  }
  // The wheel, a drum seen from above: dark between the floats, then the floats, palest along the top
  // (its middle) and a little more on the west, darker as the wheel curves down to either end.
  const end = Y * 0.7;
  g.roundRect(-X, -Y, 2 * X, 2 * Y, end).fill(mix3(WD[0], INK, 0.3));
  const N = 14;
  for (let i = 0; i < N; i++) {
    const t = ((i + 0.5) / N) * Math.PI;
    const c = Math.cos(t);
    const x = c * X * 0.95;
    const w = 3.6 + 5.4 * Math.abs(c);
    const tone = mix3(mix3(WD[1], WD[2], Math.max(0, 0.75 - 0.9 * Math.abs(c) - 0.25 * c)), WD[0], Math.max(0, Math.abs(c) - 0.6));
    g.rect(x - w / 2, -Y + 4, w, 2 * Y - 8).fill(tone).stroke({ width: 1.3, color: INK, alpha: 0.75 });
  }
  // The rims, the north one in the light.
  const rim = Y * 0.28;
  g.roundRect(-X, -Y, 2 * X, rim, rim * 0.5).fill(WD[2]);
  g.roundRect(-X, Y - rim, 2 * X, rim, rim * 0.5).fill(WD[1]);
  for (const sx of [-1, 1]) g.roundRect(sx > 0 ? X - X * 0.12 : -X, -Y, X * 0.12, 2 * Y, end).fill({ color: WD[0], alpha: 0.45 });
  g.moveTo(-X + 6, Y - rim).lineTo(X - 6, Y - rim).moveTo(-X + 6, -Y + rim).lineTo(X - 6, -Y + rim).stroke({ width: 1.4, color: INK, alpha: 0.8 });
  g.roundRect(-X, -Y, 2 * X, 2 * Y, end).stroke({ width: 2.9, color: INK });
  // The axle out toward the mill, and its bearing block.
  const ax = r * 0.13;
  g.moveTo(0, Y).lineTo(0, Y + r * 0.42).stroke({ width: ax + 3.2, color: INK }).moveTo(0, Y + 1).lineTo(0, Y + r * 0.42).stroke({ width: ax, color: WD[1] });
  g.moveTo(-ax * 0.25, Y + 2).lineTo(-ax * 0.25, Y + r * 0.42).stroke({ width: ax * 0.3, color: WD[2], alpha: 0.8 });
  g.rect(-r * 0.12, Y + r * 0.38, r * 0.24, r * 0.16).fill(WD[1]).stroke({ width: 2, color: INK });
  g.rect(-r * 0.1, Y + r * 0.39, r * 0.2, r * 0.05).fill({ color: WD[2], alpha: 0.8 });
  // The race: a trough of boards on the mill's side, water in it, pouring over the top of the wheel.
  const [rx0, rx1, ry0, ry1] = [-0.58 * X, -0.3 * X, -Y * 0.2, r * 0.72];
  g.rect(rx0, ry0, rx1 - rx0, ry1 - ry0).fill(WD[1]).stroke({ width: 2.4, color: INK });
  const wall = (rx1 - rx0) * 0.2;
  g.rect(rx0 + wall, ry0 + 2, rx1 - rx0 - 2 * wall, ry1 - ry0 - 2).fill(0x5f808a);
  g.moveTo(rx0 + wall * 1.8, ry0 + 6).lineTo(rx0 + wall * 1.8, ry1 - 10).moveTo(rx1 - wall * 1.9, ry0 + 14).lineTo(rx1 - wall * 1.9, ry1 - 4);
  g.stroke({ width: 1.4, color: 0xd8e6e6, alpha: 0.55, cap: 'round' });
  g.moveTo(rx0 + wall * 0.5, ry0 + 3).lineTo(rx0 + wall * 0.5, ry1 - 2).stroke({ width: wall * 0.45, color: WD[2], alpha: 0.8 });
  g.moveTo(rx0 + wall, ry0 + 1).lineTo(rx1 - wall, ry0 + 1).stroke({ width: 2.4, color: 0xffffff, alpha: 0.6, cap: 'round' });
}

/** Hedge, along x: a section a little over five feet long, clipped nearly square at its ends, so sections
 * placed five feet apart overlap and read as one hedge; a lumpy top lit along its north side, leaves picked
 * out. Clipped box (v0), dark yew (v1), hawthorn in flower (v2), overgrown
 * with shoots sticking out (v3). */
export function hedge(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = [
    [0x28461e, 0x40642a, 0x64903c],
    [0x1a3628, 0x2a4e36, 0x46704c],
    [0x2c4a20, 0x46682c, 0x6a923e],
    [0x34502a, 0x557a32, 0x88a846],
  ][v];
  const [L, W] = [0.66 * PX, 0.3 * PX];
  const lump = v === 3 ? 2.6 : 1.6;
  const j = () => (rnd() - 0.5) * 2 * lump;
  const pts: number[] = [];
  for (let i = 0; i <= 10; i++) pts.push(-L + 1 + ((L - 1) * 2 * i) / 10, -W + j());
  for (const k of [-0.45, 0, 0.45]) pts.push(L + j() * 0.4, k * W);
  for (let i = 10; i >= 0; i--) pts.push(-L + 1 + ((L - 1) * 2 * i) / 10, W + j() * 0.5);
  for (const k of [0.45, 0, -0.45]) pts.push(-L + j() * 0.4, k * W);
  smooth(g, pts);
  g.fill(tones[0]).stroke({ width: 2, color: INK, join: 'round' });
  smooth(g, pts.map((p, i) => (i % 2 ? p * 0.8 - 2 : p * 0.96 - 1)));
  g.fill(tones[1]);
  smooth(g, pts.map((p, i) => (i % 2 ? p * 0.35 - W * 0.42 : p * 0.9 - 1.5)));
  g.fill({ color: tones[2], alpha: 0.85 });
  // Leaves: light ones on the lit side of the top, dark ones toward the shade.
  for (let i = 0; i < 38; i++) {
    const x = -L + 4 + rnd() * (L * 2 - 8);
    const y = -W + 4 + rnd() * (W * 2 - 8);
    const light = y < -W * 0.1 ? rnd() < 0.8 : rnd() < 0.25;
    g.circle(x, y, 1.5 + rnd() * 1.6).fill({ color: light ? shade(tones[2], 1.12) : tones[0], alpha: 0.75 });
  }
  if (v === 2) {
    // Blossom in clusters of three.
    for (let i = 0; i < 6; i++) {
      const x = -L + 8 + ((i + rnd() * 0.7) / 6) * (L * 2 - 16);
      const y = -W + 6 + rnd() * (W * 2 - 12);
      for (let k = 0; k < 3; k++) flower5(g, x + Math.cos(k * 2.1) * 3.4, y + Math.sin(k * 2.1) * 3, 3.8, i % 3 ? 0xf6efe0 : 0xf0b8c8, 0xd8b040, rnd());
    }
  }
  if (v === 3) {
    // Shoots grown out past the clipped line, uneven: two together near one end, a gap, a few more; each a
    // curved stem with two to four leaves, some lying along the hedge.
    const at = [-0.7, -0.55, 0.2 + 0.2 * rnd(), 0.65, -0.1].slice(0, 3 + Math.floor(rnd() * 3));
    at.forEach((f, i) => {
      const side = -1; // (all on the north edge: straight down, nothing hangs off a south face)
      const x0 = f * L;
      const y0 = side * (W - 3);
      const len = (0.12 + 0.12 * rnd()) * PX;
      const a = side * (Math.PI / 2) + (rnd() - 0.5) * 1.6;
      const bend = (rnd() - 0.5) * 1.2;
      const [x1, y1] = [x0 + Math.cos(a + bend) * len, y0 + Math.sin(a + bend) * len];
      const [mx, my] = [x0 + Math.cos(a) * len * 0.6, y0 + Math.sin(a) * len * 0.6];
      g.moveTo(x0, y0).quadraticCurveTo(mx, my, x1, y1).stroke({ width: 1.6, color: 0x4a3a20, cap: 'round' });
      const n = 2 + Math.floor(rnd() * 3);
      for (let k = 0; k < n; k++) {
        const t = 0.4 + (0.6 * (k + 1)) / n;
        const [lx, ly] = [x0 + (mx - x0) * t * 1.2, y0 + (my - y0) * t * 1.2];
        leaf(g, k === n - 1 ? x1 : lx, k === n - 1 ? y1 : ly, a + bend * t + (k % 2 ? 1.1 : -1.1) * (k === n - 1 ? 0 : 1), (0.08 + 0.04 * rnd()) * PX, 2.8, tones[k === n - 1 ? 2 : 1]);
      }
    });
  }
}

/** A clipped ball of leaves at (x, y): inked, lit toward the light, leaves picked out. */
export function clippedBall(g: Graphics, x: number, y: number, s: number, tones: readonly number[], rnd: () => number) {
  g.circle(x, y, s).fill(tones[0]).stroke({ width: 2, color: INK });
  g.circle(x - s * 0.1, y - s * 0.12, s * 0.84).fill(tones[1]);
  g.circle(x - s * 0.3, y - s * 0.32, s * 0.42).fill({ color: tones[2], alpha: 0.85 });
  for (let i = 0; i < Math.round(s * 0.8); i++) {
    const a = rnd() * Math.PI * 2;
    const d = s * 0.82 * Math.sqrt(rnd());
    const [dx, dy] = [Math.cos(a) * d, Math.sin(a) * d];
    g.circle(x + dx, y + dy, 1.3 + rnd() * 1.2).fill({ color: dx + dy < 0 ? shade(tones[2], 1.12) : tones[0], alpha: 0.7 });
  }
}

/** Topiary, from a high angle (it reads by its shape from the side): a clipped ball on a stem in a
 * terracotta pot (v0); a cone (v1); a cone clipped into a spiral (v2); three balls on one stem (v3). */
export function topiary(g: Graphics, r: number, rnd: () => number, v: number) {
  const P = upright(UPRIGHT_FOOT[57]);
  const tones = [0x22401c, 0x386026, 0x5c8a34];
  // A ground point round the foot: on a circle of radius `R` ft, at angle `t` (+y toward the viewer).
  const ring = (R: number, t: number, h: number) => P(Math.cos(t) * R, Math.sin(t) * R, h);
  const half = (R: number, h: number, t0: number, t1: number) => Array.from({ length: 13 }, (_, i) => ring(R, t0 + ((t1 - t0) * i) / 12, h)).flat();
  if (v === 0) {
    // The pot: its body down to the front of its foot, the rim seen from above with the earth in it.
    const pot = [0x8a4a2e, 0xb8683e, 0xd88a5a];
    g.poly([...ring(0.95, Math.PI, 1.6), ...half(0.72, 0, Math.PI, 0), ...ring(0.95, 0, 1.6), ...half(0.95, 1.6, 0, Math.PI)]).fill(pot[1]).stroke({ width: 2, color: INK, join: 'round' });
    g.poly([...ring(0.95, Math.PI, 1.6), ...half(0.72, 0, Math.PI, Math.PI * 0.62), ...half(0.95, 1.6, Math.PI * 0.62, Math.PI)]).fill({ color: pot[2], alpha: 0.6 });
    g.poly(half(0.95, 1.6, 0, Math.PI * 2)).fill(pot[2]).stroke({ width: 1.8, color: INK, join: 'round' });
    g.poly(half(0.75, 1.6, 0, Math.PI * 2)).fill(0x3e2c1e);
    rod(g, P(0, 0, 1.6), P(0, 0, 3.4), 2.4, BARK[0][1]);
    clippedBall(g, ...P(0, 0, 4.5), 1.4 * FT, tones, rnd);
    return;
  }
  // Earth round the foot, grass at its edge.
  g.poly(half(1.8, 0, 0, Math.PI * 2)).fill(0x4a3828).stroke({ width: 1.2, color: 0x2a1c12 });
  footGrass(g, ...P(-1.6, 0.4, 0), r * 0.2, rnd);
  footGrass(g, ...P(1.5, 0.7, 0), r * 0.16, rnd);
  if (v === 3) {
    rod(g, P(0, 0, 0), P(0, 0, 5.8), 2.6, BARK[0][1]);
    for (const [h, s] of [
      [1.4, 1.3],
      [3.7, 1.0],
      [5.6, 0.7],
    ]) {
      clippedBall(g, ...P(0, 0, h), s * FT, tones, rnd);
    }
    return;
  }
  // A cone: its shaded east side, its lit west side, leaves picked out; on a spiral, the groove winding up.
  const [R, top] = [1.55, 6.6];
  const apex = P(0, 0, top);
  const at = (t: number, h: number) => ring(R * (1 - h / top), t, h);
  g.poly([...apex, ...half(R, 0.1, 0, Math.PI)]).fill(tones[1]).stroke({ width: 2, color: INK, join: 'round' });
  g.poly([...apex, ...half(R, 0.1, 0, Math.PI * 0.4)]).fill({ color: tones[0], alpha: 0.75 });
  g.poly([...apex, ...half(R, 0.1, Math.PI * 0.72, Math.PI)]).fill({ color: tones[2], alpha: 0.7 });
  for (let i = 0; i < 26; i++) {
    const h = rnd() * top * 0.85;
    const t = rnd() * Math.PI;
    const [x, y] = at(t, h);
    g.circle(x, y, 1.3 + rnd() * 1.1).fill({ color: t > Math.PI * 0.55 ? shade(tones[2], 1.12) : tones[0], alpha: 0.7 });
  }
  if (v === 2) {
    for (let k = 0; k < 5; k++) {
      const h0 = 0.4 + k * 1.25;
      const path = Array.from({ length: 13 }, (_, i) => {
        const t = Math.PI - (Math.PI * i) / 12;
        return at(t, h0 + (i / 12) * 0.75);
      });
      g.poly(path.flat(), false).stroke({ width: 3.6, color: shade(tones[0], 0.7), cap: 'round', join: 'round' });
      g.poly(path.map(([x, y]) => [x, y - 2.6]).flat(), false).stroke({ width: 1.6, color: tones[2], cap: 'round', join: 'round' });
    }
  }
  g.poly([...apex, ...half(R, 0.1, 0, Math.PI)]).stroke({ width: 2, color: INK, join: 'round' });
}
