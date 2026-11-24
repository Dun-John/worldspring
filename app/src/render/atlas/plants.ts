import { Graphics } from 'pixi.js';
import { INK, blob, oval, shade } from './common';

import { OUT } from '../pen';
/** Leafy canopy: cluster of lobes, dark base → light top-left highlights, ink rim. */
export function canopy(g: Graphics, r: number, rnd: () => number, colors: readonly number[], lobes: number) {
  const [dark, mid, light] = colors;
  const parts: [number, number, number][] = [];
  for (let i = 0; i < lobes; i++) {
    const a = (i / lobes) * Math.PI * 2 + rnd() * 0.6;
    const d = r * (0.35 + 0.2 * rnd());
    parts.push([Math.cos(a) * d, Math.sin(a) * d, r * (0.42 + 0.14 * rnd())]);
  }
  parts.push([0, 0, r * 0.55]);
  for (const [x, y, rr] of parts) g.circle(x, y, rr + OUT).fill(INK);
  for (const [x, y, rr] of parts) g.circle(x, y, rr).fill(dark);
  for (const [x, y, rr] of parts) g.circle(x - rr * 0.18, y - rr * 0.18, rr * 0.78).fill(mid);
  for (const [x, y, rr] of parts) g.circle(x - rr * 0.34, y - rr * 0.34, rr * 0.42).fill({ color: light, alpha: 0.9 });
  for (let i = 0; i < lobes * 2; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.8 * rnd();
    g.circle(Math.cos(a) * d - r * 0.1, Math.sin(a) * d - r * 0.1, r * 0.06).fill({ color: shade(light, 1.2), alpha: 0.7 });
  }
}

/** Acacia from above: a flat, wide umbrella broken into separate leafy pads with gaps between them, the
 * forked limbs (dark wood, a lit edge) showing through the gaps and through a few holes in the pads. Each
 * pad has a shadowed SE underside, a mid tone and sunlit NW tops; a soft shadow lies under the crown. */
export function acacia(g: Graphics, r: number, rnd: () => number, colors: readonly number[]) {
  const [dark, mid, light] = colors;
  const n = 5 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  const pads: [number, number, number, number][] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.4) / n) * Math.PI * 2;
    const d = r * (0.5 + 0.14 * rnd());
    pads.push([Math.cos(a) * d, Math.sin(a) * d * 0.82, r * (0.27 + 0.08 * rnd()), lit(a)]);
  }
  pads.push([(rnd() - 0.5) * r * 0.2, (rnd() - 0.5) * r * 0.2, r * 0.22, 0.4]);
  // Each pad a few flat lobes, outlined together.
  const lobes: [number, number, number, number][] = [];
  for (const [x, y, rr, l] of pads) {
    for (let j = 0; j < 3; j++) {
      const a = rnd() * Math.PI * 2;
      lobes.push([x + Math.cos(a) * rr * 0.45, y + Math.sin(a) * rr * 0.35, rr * (0.6 + 0.15 * rnd()), l]);
    }
  }
  // The crown's soft shadow on the ground, off to the SE.
  for (const [x, y, rr] of lobes) g.circle(x + r * 0.08, y + r * 0.1, rr);
  g.fill({ color: 0x3a3018, alpha: 0.14 });
  // Limbs from the trunk out to each pad, in two wood tones.
  const limb = (x0: number, y0: number, x: number, y: number, w: number) => {
    const cx = x0 + (x - x0) * 0.4 + (y - y0) * 0.15;
    const cy = y0 + (y - y0) * 0.4 - (x - x0) * 0.15;
    g.moveTo(x0, y0).quadraticCurveTo(cx, cy, x, y).stroke({ width: w + 2.5, color: INK, cap: 'round' });
    g.moveTo(x0, y0).quadraticCurveTo(cx, cy, x, y).stroke({ width: w, color: 0x5e3e26, cap: 'round' });
    g.moveTo(x0 - 0.8, y0 - 0.8).quadraticCurveTo(cx - 0.8, cy - 0.8, x - 0.8, y - 0.8).stroke({ width: w * 0.4, color: 0x9c7446, cap: 'round' });
  };
  for (const [x, y] of pads) limb(0, 0, x, y, 4);
  g.circle(0, 0, r * 0.07).fill(0x5a3e28).stroke({ width: 2, color: INK });
  for (const [x, y, rr] of lobes) g.circle(x, y, rr + OUT).fill(INK);
  for (const [x, y, rr] of lobes) g.circle(x, y, rr).fill(dark);
  for (const [x, y, rr, l] of lobes) g.circle(x - rr * 0.16, y - rr * 0.18, rr * 0.78).fill(shade(mid, 1 + 0.06 * l));
  // A few holes through the leaves with a limb across them.
  for (let i = 0; i < 3; i++) {
    const [x, y, rr] = pads[Math.floor(rnd() * n)];
    const hx = x + (rnd() - 0.5) * rr * 0.6;
    const hy = y + (rnd() - 0.5) * rr * 0.5;
    g.poly(blob(hx, hy, rr * 0.2, 7, 0.4, rnd)).fill(shade(dark, 0.62));
    const t = rnd() * Math.PI;
    g.moveTo(hx - Math.cos(t) * rr * 0.17, hy - Math.sin(t) * rr * 0.17).lineTo(hx + Math.cos(t) * rr * 0.17, hy + Math.sin(t) * rr * 0.17).stroke({ width: 2.2, color: 0x5e3e26, cap: 'round' });
  }
  for (const [x, y, rr, l] of lobes) g.circle(x - rr * 0.32, y - rr * 0.34, rr * 0.44).fill(shade(light, 1 + 0.07 * l));
}

/** A broad jungle leaf from (x, y) along `a`: widest a little before its middle, a bold outline in `line`
 * and a pale midrib. */
function broadLeaf(g: Graphics, x: number, y: number, a: number, len: number, color: number, line: number, lw: number, bend: number) {
  const c = Math.cos(a);
  const s = Math.sin(a);
  const w = len * 0.36;
  const tx = x + c * len - s * len * bend;
  const ty = y + s * len + c * len * bend;
  const mx = x + c * len * 0.45 - s * len * bend * 0.5;
  const my = y + s * len * 0.45 + c * len * bend * 0.5;
  g.moveTo(x - s * w * 0.25, y + c * w * 0.25)
    .bezierCurveTo(mx - s * w * 1.25, my + c * w * 1.25, tx - c * len * 0.2 - s * w * 0.5, ty - s * len * 0.2 + c * w * 0.5, tx, ty)
    .bezierCurveTo(tx - c * len * 0.2 + s * w * 0.5, ty - s * len * 0.2 - c * w * 0.5, mx + s * w * 1.25, my - c * w * 1.25, x + s * w * 0.25, y - c * w * 0.25)
    .closePath()
    .fill(color)
    .stroke({ width: lw, color: line, join: 'round' });
  g.moveTo(x, y).quadraticCurveTo(mx, my, x + (tx - x) * 0.82, y + (ty - y) * 0.82).stroke({ width: 1.6, color: shade(color, 1.4), cap: 'round' });
}

/** Jungle tree from above: a full, round crown of big broad leaves in overlapping clumps, a dark under-layer
 * at the rim, mid clumps over it and a lighter top layer toward the NW, a few leaf tips breaking the outline. */
export function jungleTree(g: Graphics, r: number, rnd: () => number, colors: readonly number[]) {
  const [dark, mid, light] = colors;
  const parts: [number, number, number, number][] = [];
  const lobes = 9;
  for (let i = 0; i < lobes; i++) {
    const a = (i / lobes) * Math.PI * 2 + rnd() * 0.6;
    const d = r * (0.35 + 0.18 * rnd());
    parts.push([Math.cos(a) * d, Math.sin(a) * d, r * (0.42 + 0.12 * rnd()), a]);
  }
  const line = shade(dark, 0.5);
  const tone = (a: number, base: number, k: number) => shade(base, 1 + k * lit(a));
  // Leaf tips past the rim: drawn first, so the crown covers their stalks.
  const out = [0, 3, 6].map((k) => parts[(k + Math.floor(rnd() * 3)) % lobes]);
  for (const [x, y, rr, a] of out) {
    const d = Math.hypot(x, y);
    const b = a + (rnd() - 0.5) * 0.5;
    broadLeaf(g, x * 0.6, y * 0.6, b, Math.min(rr + d * 0.4 + r * 0.12, r * 1.04 - d * 0.6), tone(b, dark, 0.15), INK, OUT * 0.8, (rnd() - 0.5) * 0.3);
  }
  for (const [x, y, rr] of parts) g.circle(x, y, rr + OUT).fill(INK);
  g.circle(0, 0, r * 0.55 + OUT).fill(INK);
  for (const [x, y, rr] of parts) g.circle(x, y, rr).fill(dark);
  g.circle(0, 0, r * 0.55).fill(dark);
  // Rim clumps: three leaves fanning out from inside each lobe, the shaded side first.
  const rim = [...parts].sort((p, q) => lit(p[3]) - lit(q[3]));
  for (const [x, y, rr, a] of rim) {
    const d = Math.hypot(x, y);
    for (const f of [-0.6, 0.6, -0.2, 0.25]) {
      const b = a + f + (rnd() - 0.5) * 0.25;
      const len = (rr + d * 0.3 - r * 0.05) * (0.9 + 0.1 * rnd());
      broadLeaf(g, x * 0.7, y * 0.7, b, len, tone(b, lit(b) > 0.3 ? mid : dark, 0.14), line, 1.6, (rnd() - 0.5) * 0.35);
    }
  }
  // Mid clumps, nearer the centre and a little toward the light.
  const clump = (cx: number, cy: number, k: number, len: number, base: number, edge: number) => {
    const a0 = Math.atan2(cy, cx) + (rnd() - 0.5) * 1.2;
    for (let i = 0; i < k; i++) {
      const b = a0 + (i - (k - 1) / 2) * 0.75 + (rnd() - 0.5) * 0.3;
      broadLeaf(g, cx, cy, b, len * (0.85 + 0.25 * rnd()), tone(b, base, 0.12), edge, 1.5, (rnd() - 0.5) * 0.35);
    }
  };
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < 3; i++) {
    const a = ph + (i / 3) * Math.PI * 2 + (rnd() - 0.5) * 0.6;
    clump(Math.cos(a) * r * 0.24 - r * 0.06, Math.sin(a) * r * 0.24 - r * 0.06, 3, r * 0.46, mid, line);
  }
  clump(r * 0.02, r * 0.02, 4, r * 0.4, mid, line);
  // The sunlit top layer, toward the NW.
  const top = shade(light, 1.12);
  clump(-r * 0.12 + (rnd() - 0.5) * r * 0.08, -r * 0.2, 3, r * 0.32, top, shade(mid, 0.6));
  clump(-r * 0.22, -r * 0.04 + (rnd() - 0.5) * r * 0.08, 2, r * 0.27, top, shade(mid, 0.6));
}

/** How much a direction faces the NW light: 1 toward it, -1 away. */
export function lit(a: number): number {
  return -(Math.cos(a) + Math.sin(a)) * Math.SQRT1_2;
}

/** A whorl's bough tips: `n` (angle, radius fraction) pairs, jittered. */
export function boughs(n: number, rnd: () => number): [number, number][] {
  const ph = rnd() * Math.PI * 2;
  const tips: [number, number][] = [];
  for (let i = 0; i < n; i++) tips.push([ph + ((i + (rnd() - 0.5) * 0.45) / n) * Math.PI * 2, 0.8 + 0.2 * rnd()]);
  return tips;
}

/** A ring of bough tips round (cx, cy) at radius `rr`, valleys at `inner` of it: slightly full sides
 * meeting in a soft point. */
export function whorl(g: Graphics, cx: number, cy: number, rr: number, tips: [number, number][], inner: number) {
  const n = tips.length;
  const valley = (i: number) => {
    const [a0, d0] = tips[i % n];
    const [a1, d1] = tips[(i + 1) % n];
    const a = a0 + (((a1 - a0 + Math.PI * 3) % (Math.PI * 2)) - Math.PI) / 2;
    const d = rr * inner * (d0 + d1) * 0.5;
    return [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
  };
  const s = (Math.PI * 2 * 0.22) / n;
  let [x, y] = valley(n - 1);
  g.moveTo(x, y);
  for (let i = 0; i < n; i++) {
    const [a, k] = tips[i];
    const d = rr * k;
    [x, y] = valley(i);
    g.quadraticCurveTo(cx + Math.cos(a - s) * d * 0.9, cy + Math.sin(a - s) * d * 0.9, cx + Math.cos(a) * d, cy + Math.sin(a) * d);
    g.quadraticCurveTo(cx + Math.cos(a + s) * d * 0.9, cy + Math.sin(a + s) * d * 0.9, x, y);
  }
  g.closePath();
}

/** Conifer from above: stacked whorls of bough tips, each smaller, lighter and nearer the light than
 * the one below, so the crown reads as a cone; each whorl lighter on its NW side. */
export function conifer(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = [
    [0x1f3a2c, 0x2b5038, 0x3d6a45, 0x5a8a54],
    [0x223c2a, 0x2f5434, 0x436f40, 0x62904e],
    [0x1d3a30, 0x28503e, 0x3a6a4c, 0x56885c],
    [0x26402c, 0x345a36, 0x4a7642, 0x6e9a52],
  ][v];
  const layers: [number, number, number][] = [
    [1, 14, 0.8],
    [0.74, 11, 0.78],
    [0.5, 8, 0.74],
    [0.27, 6, 0.7],
  ];
  layers.forEach(([k, n, inner], l) => {
    const off = -l * r * 0.05;
    const tips = boughs(n, rnd);
    whorl(g, off, off, r * k, tips, inner);
    g.fill(tones[l]);
    if (l === 0) g.stroke({ width: OUT, color: INK, join: 'round' });
    else g.stroke({ width: 1.5, color: shade(tones[l - 1], 0.75), join: 'round' });
    // The whorl's lit side: the same tips, smaller and pushed toward the light.
    const h = r * k * 0.08;
    whorl(g, off - h, off - h, r * k * 0.84, tips, inner);
    g.fill({ color: shade(tones[l], 1.2), alpha: 0.55 });
  });
  g.circle(-r * 0.17, -r * 0.17, r * 0.06).fill(shade(tones[3], 1.35));
}

/** Palm from above: curved fronds of leaflet strokes (outlined in a dark tone of their own green, so they sit
 * with the other foliage) round a fibrous crown with a few coconuts.
 * Fronds facing the light are brighter; an old frond is dried to tan. */
export function palm(g: Graphics, r: number, rnd: () => number, v: number) {
  const green = [0x4a7a2c, 0x568a30, 0x3f7030, 0x5e8a34][v];
  const dry = 0xa8924a;
  const frond = (a: number, len: number, w: number, bend: number, color: number) => {
    // Quadratic spine from the centre, bowed sideways by `bend` (a drooping frond seen from above).
    const p2 = [Math.cos(a) * len, Math.sin(a) * len];
    const p1 = [Math.cos(a) * len * 0.5 - Math.sin(a) * len * bend, Math.sin(a) * len * 0.5 + Math.cos(a) * len * bend];
    const at = (t: number) => {
      const u = 1 - t;
      const x = 2 * u * t * p1[0] + t * t * p2[0];
      const y = 2 * u * t * p1[1] + t * t * p2[1];
      const dx = 2 * u * p1[0] + 2 * t * (p2[0] - p1[0]);
      const dy = 2 * u * p1[1] + 2 * t * (p2[1] - p1[1]);
      const m = Math.hypot(dx, dy) || 1;
      return [x, y, -dy / m, dx / m];
    };
    // Leaflets: bold strokes from the spine, swept toward the tip, longest a little before the middle.
    // All in ink first, then all in colour, so the frond has one outline.
    const steps = 11;
    const leaves: number[][] = [];
    for (let i = 2; i < steps; i++) {
      const t = i / steps;
      const [x, y, nx, ny] = at(t);
      const [tx, ty] = [ny, -nx];
      const ll = w * Math.sin(Math.PI * Math.pow(t, 0.75)) * (0.85 + 0.3 * rnd());
      for (const side of [1, -1]) {
        const dx = nx * side * 0.75 + tx * 0.66;
        const dy = ny * side * 0.75 + ty * 0.66;
        leaves.push([x, y, x + dx * ll, y + dy * ll]);
      }
    }
    for (const [width, c] of [
      [5, shade(color, 0.45)],
      [3, color],
    ]) {
      g.moveTo(0, 0).quadraticCurveTo(p1[0], p1[1], p2[0], p2[1]);
      for (const [x, y, x2, y2] of leaves) g.moveTo(x, y).lineTo(x2, y2);
      g.stroke({ width, color: c, cap: 'round', join: 'round' });
    }
    g.moveTo(0, 0).quadraticCurveTo(p1[0], p1[1], p2[0], p2[1]).stroke({ width: 1.5, color: shade(color, 1.35) });
  };
  const n = 7 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  const dead = Math.floor(rnd() * n);
  const bend = (rnd() < 0.5 ? -1 : 1) * 0.22;
  // Lower, longer fronds first, then shorter upper ones between them.
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.4) / n) * Math.PI * 2;
    const c = i === dead ? dry : shade(green, 0.85 + 0.25 * lit(a));
    frond(a, r * (0.88 + 0.12 * rnd()), r * 0.2, bend * (0.7 + 0.6 * rnd()), c);
  }
  for (let i = 0; i < 4; i++) {
    const a = ph + ((i + 0.5) / 4) * Math.PI * 2 + (rnd() - 0.5) * 0.5;
    frond(a, r * (0.5 + 0.12 * rnd()), r * 0.15, bend, shade(green, 1.1 + 0.2 * lit(a)));
  }
  g.poly(blob(0, 0, r * 0.16, 8, 0.3, rnd)).fill(0x6b4a2a).stroke({ width: 2, color: INK });
  for (let i = 0; i < 3; i++) {
    const a = ph + i * 2.1 + rnd() * 0.5;
    g.circle(Math.cos(a) * r * 0.1, Math.sin(a) * r * 0.1, r * 0.075).fill(0x7a5a2e).stroke({ width: 1.5, color: INK });
  }
  g.circle(-r * 0.04, -r * 0.05, r * 0.035).fill({ color: 0xc89a5a, alpha: 0.8 });
}

/** Bare limbs forking out from (0, 0): `n` limbs `len` long and `w` thick at the base, each bending
 * at `bends` points by up to `twist` and forking `depth` times (a third, short spur now and then with
 * `spurs`), lengths `spread` apart. Segments [x, y, x2, y2, width]. */
function bareLimbs(rnd: () => number, n: number, len: number, w: number, depth: number, twist: number, bends = 1, spurs = 0, spread = 0.2): number[][] {
  const segs: number[][] = [];
  const limb = (x: number, y: number, a: number, len: number, w: number, depth: number) => {
    let [px, py, pa] = [x, y, a];
    const parts = bends + 1;
    for (let i = 0; i < parts; i++) {
      if (i > 0) pa += (rnd() - 0.5) * twist;
      const [qx, qy] = [px + (Math.cos(pa) * len) / parts, py + (Math.sin(pa) * len) / parts];
      segs.push([px, py, qx, qy, w * (1 - (0.3 * i) / parts)]);
      [px, py] = [qx, qy];
    }
    if (depth > 0) {
      limb(px, py, pa + 0.4 + rnd() * 0.35, len * 0.62, w * 0.6, depth - 1);
      if (rnd() < 0.85) limb(px, py, pa - 0.4 - rnd() * 0.35, len * 0.56, w * 0.55, depth - 1);
      if (rnd() < spurs) limb(px, py, pa + (rnd() - 0.5) * 0.4, len * 0.4, w * 0.45, depth - 1);
    }
  };
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) limb(0, 0, ph + ((i + (rnd() - 0.5) * 0.5) / n) * Math.PI * 2, len * (1 - spread / 2 + spread * rnd()), w, depth);
  return segs;
}

/** Limbs drawn in ink, then bark, then a thin lighter line along each one's NW side (`bark`: dark, mid,
 * light; `ink` the outline's width beyond the limb). */
function drawLimbs(g: Graphics, segs: number[][], bark: readonly number[], ink = 3) {
  for (const [x, y, x2, y2, w] of segs) g.moveTo(x, y).lineTo(x2, y2).stroke({ width: w + ink, color: INK, cap: 'round' });
  for (const [x, y, x2, y2, w] of segs) g.moveTo(x, y).lineTo(x2, y2).stroke({ width: w, color: bark[1], cap: 'round' });
  for (const [x, y, x2, y2, w] of segs) {
    if (w < 2.5) continue;
    const len = Math.hypot(x2 - x, y2 - y) || 1;
    let nx = -(y2 - y) / len;
    let ny = (x2 - x) / len;
    if (nx + ny > 0) [nx, ny] = [-nx, -ny];
    const o = w * 0.22;
    g.moveTo(x + nx * o, y + ny * o).lineTo(x2 + nx * o, y2 + ny * o).stroke({ width: w * 0.3, color: bark[2], cap: 'round', alpha: 0.9 });
  }
}

/** Roots spreading over the ground from a trunk `size` across (darker than the limbs above them, in
 * shade), and the trunk's foot: a lobed base where they meet. */
function rootsAndTrunk(g: Graphics, rnd: () => number, size: number, bark: readonly number[]) {
  // Unforked and wavering, tapering to a point as they run into the ground.
  const len = size * 2.5;
  const roots = bareLimbs(rnd, 6 + Math.floor(rnd() * 2), len, size * 0.7, 0, 0.9, 2, 0, 0.5).map(([x, y, x2, y2, w]) => {
    const d = Math.hypot((x + x2) / 2, (y + y2) / 2) / len;
    return [x, y, x2, y2, w * Math.max(0.25, 1 - 0.85 * d)];
  });
  const dark = [shade(bark[0], 0.8), shade(bark[0], 1.15), shade(bark[1], 0.9)];
  drawLimbs(g, roots, dark, 2.5);
  g.poly(blob(0, 0, size, 10, 0.35, rnd)).fill(dark[1]).stroke({ width: 2, color: INK, join: 'round' });
  g.poly(blob(-size * 0.12, -size * 0.12, size * 0.6, 8, 0.3, rnd)).fill({ color: dark[2], alpha: 0.8 });
}

/** Dead tree: a few gnarled, tapering limbs forking out from where the trunk splits, over the trunk's
 * foot and its roots on the ground, bark lit along the NW side. */
export function deadTree(g: Graphics, r: number, rnd: () => number, v: number) {
  const bark = [
    [0x4e3424, 0x7d5a3c, 0xa8825a],
    [0x4a3a2c, 0x76604a, 0xa08a6c],
    [0x523628, 0x84603e, 0xb08a5e],
    [0x463830, 0x6e6052, 0x9a8c78],
  ][v];
  if (v === 3) {
    // The first dead tree's limbs, as drawn before the others were redrawn.
    const segs: number[][] = [];
    const limb = (x: number, y: number, a: number, len: number, w: number, depth: number) => {
      const bend = (rnd() - 0.5) * 0.7;
      const xm = x + Math.cos(a) * len * 0.5;
      const ym = y + Math.sin(a) * len * 0.5;
      const x2 = xm + Math.cos(a + bend) * len * 0.5;
      const y2 = ym + Math.sin(a + bend) * len * 0.5;
      segs.push([x, y, xm, ym, w], [xm, ym, x2, y2, w * 0.75]);
      if (depth > 0) {
        limb(x2, y2, a + bend + 0.45 + rnd() * 0.3, len * 0.62, w * 0.6, depth - 1);
        if (rnd() < 0.8) limb(x2, y2, a + bend - 0.45 - rnd() * 0.3, len * 0.55, w * 0.55, depth - 1);
      }
    };
    const n = 3 + Math.floor(rnd() * 2);
    const ph = rnd() * Math.PI * 2;
    for (let i = 0; i < n; i++) limb(0, 0, ph + ((i + (rnd() - 0.5) * 0.5) / n) * Math.PI * 2, r * (0.5 + 0.08 * rnd()), r * 0.16, 2);
    rootsAndTrunk(g, rnd, r * 0.2, bark);
    drawLimbs(g, segs, bark);
    return;
  }
  rootsAndTrunk(g, rnd, r * 0.2, bark);
  // (Drawn within its frame: limbs reaching past it are pulled in.)
  const segs = bareLimbs(rnd, 3 + Math.floor(rnd() * 2), r * 0.54, r * 0.2, 2, 0.7);
  const reach = Math.max(...segs.map(([, , x2, y2]) => Math.hypot(x2, y2)));
  const k = Math.min(1, (r * 0.95) / reach);
  drawLimbs(g, segs.map(([x, y, x2, y2, w]) => [x * k, y * k, x2 * k, y2 * k, w]), bark);
}

export function strokes(g: Graphics, r: number, rnd: () => number, n: number, len: number, colors: number[], width: number) {
  for (let i = 0; i < n; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.7 * Math.sqrt(rnd());
    const x = Math.cos(a) * d;
    const y = Math.sin(a) * d;
    const b = rnd() * Math.PI * 2;
    const l = len * (0.6 + 0.6 * rnd());
    g.moveTo(x, y)
      .lineTo(x + Math.cos(b) * l, y + Math.sin(b) * l)
      .stroke({ width, color: colors[i % colors.length], cap: 'round' });
  }
}

/** A pointed leaf or blade from (x, y) out along `a`, widest at `at` of its length, with a lighter
 * midrib; `bend` bows it sideways. */
export function leaf(g: Graphics, x: number, y: number, a: number, len: number, w: number, color: number, at = 0.35, bend = 0) {
  const c = Math.cos(a);
  const s = Math.sin(a);
  const tx = x + c * len - s * len * bend;
  const ty = y + s * len + c * len * bend;
  const mx = x + c * len * at - s * len * bend * 0.5;
  const my = y + s * len * at + c * len * bend * 0.5;
  g.moveTo(x - s * w * 0.3, y + c * w * 0.3)
    .quadraticCurveTo(mx - s * w * 1.1, my + c * w * 1.1, tx, ty)
    .quadraticCurveTo(mx + s * w * 1.1, my - c * w * 1.1, x + s * w * 0.3, y - c * w * 0.3)
    .closePath()
    .fill(color)
    .stroke({ width: 1.5, color: INK, join: 'round' });
  if (len > 10) g.moveTo(x, y).quadraticCurveTo(mx, my, x + (tx - x) * 0.85, y + (ty - y) * 0.85).stroke({ width: 1.2, color: shade(color, 1.3), alpha: 0.85 });
}

/** A tuft: `n` blades fanning out from (x, y), long ones first and short upright ones on top, each lit
 * by the way it faces. */
export function tuft(g: Graphics, x: number, y: number, n: number, len: number, w: number, colors: readonly number[], rnd: () => number) {
  const blades: [number, number][] = [];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) blades.push([ph + ((i + (rnd() - 0.5) * 0.7) / n) * Math.PI * 2, len * (0.55 + 0.45 * rnd())]);
  blades.sort((p, q) => q[1] - p[1]);
  for (const [a, l] of blades) leaf(g, x, y, a, l, w, shade(colors[Math.floor(rnd() * colors.length)], 1 + 0.18 * lit(a)), 0.3, (rnd() - 0.5) * 0.3);
}

/** Reeds: two or three clumps of long thin blades radiating from their tufts (darker long ones under,
 * lighter short ones on top), and a few brown cattail heads on stalks laid outward over the blades. */
export function reeds(g: Graphics, r: number, rnd: () => number, v: number) {
  const greens = [
    [0x4e7a3c, 0x6e9a4c, 0x94bc68],
    [0x48763e, 0x68964e, 0x8cb66a],
    [0x5a7c38, 0x7c9c46, 0xa2bc60],
    [0x467442, 0x629252, 0x86b070],
  ][v];
  const n = 2 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  const spots: number[][] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    spots.push([Math.cos(a) * r * 0.3, Math.sin(a) * r * 0.3]);
  }
  // Long dark blades first (all clumps), then mid, then short light ones, so the clumps knit together.
  for (let layer = 0; layer < 3; layer++) {
    for (const [x, y] of spots) {
      const m = [11, 8, 5][layer];
      const len = r * [0.62, 0.46, 0.28][layer];
      const tp = rnd() * Math.PI * 2;
      for (let j = 0; j < m; j++) {
        const a = tp + ((j + (rnd() - 0.5) * 0.6) / m) * Math.PI * 2;
        const c = shade(greens[layer], 1 + 0.15 * lit(a));
        leaf(g, x, y, a, len * (0.7 + 0.3 * rnd()), r * 0.035, c, 0.25, (rnd() - 0.5) * 0.35);
      }
    }
  }
  // Cattails: a stalk from a tuft out across the blades, the brown head near its end, laid along it.
  const heads = 3 + (v % 2);
  for (let i = 0; i < heads; i++) {
    const [x0, y0] = spots[i % n];
    const a = Math.atan2(y0, x0) + (rnd() - 0.5) * 1.6 + (i >= n ? 1.2 : 0);
    const c = Math.cos(a);
    const s = Math.sin(a);
    const l = r * (0.5 + 0.15 * rnd());
    g.moveTo(x0, y0).lineTo(x0 + c * l, y0 + s * l).stroke({ width: 2.5, color: 0x3a4a22, cap: 'round' });
    g.moveTo(x0, y0).lineTo(x0 + c * l, y0 + s * l).stroke({ width: 1.2, color: 0x7a9a4a, cap: 'round' });
    const hx = x0 + c * l * 0.72;
    const hy = y0 + s * l * 0.72;
    g.poly(oval(hx, hy, r * 0.16, r * 0.06, a)).fill(0x5e3e26).stroke({ width: 1.5, color: INK });
    g.poly(oval(hx - c * r * 0.01 - r * 0.012, hy - s * r * 0.01 - r * 0.012, r * 0.1, r * 0.022, a)).fill(0x8e6440);
  }
}

/** Tall grass: two or three bold tufts, each a fountain of narrow blades (drawn like a conifer whorl) with
 * a lighter inner tuft nearer the light; few, big shapes so it stays clean zoomed out. */
export function tallGrass(g: Graphics, r: number, rnd: () => number, v: number) {
  const greens = [
    [0x68943a, 0x84ae46, 0xa6c860],
    [0x78963a, 0x96ae48, 0xb6c662],
    [0x60903e, 0x7caa4c, 0x9cc466],
    [0x86943a, 0xa4ac48, 0xc2c262],
  ][v];
  const n = 2 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    const x = Math.cos(a) * r * 0.3;
    const y = Math.sin(a) * r * 0.3;
    const rr = r * (0.62 + 0.1 * rnd());
    // Many narrow blades of uneven length: a fountain of grass, not a leaf.
    const tips = boughs(14, rnd).map(([t, k]): [number, number] => [t, k * (0.6 + 0.4 * rnd())]);
    whorl(g, x, y, rr, tips, 0.4);
    g.fill(greens[0]).stroke({ width: 1.5, color: shade(greens[0], 0.45), join: 'round' });
    whorl(g, x - rr * 0.08, y - rr * 0.08, rr * 0.7, tips, 0.4);
    g.fill(greens[1]);
    whorl(g, x - rr * 0.14, y - rr * 0.14, rr * 0.38, boughs(10, rnd), 0.45);
    g.fill(greens[2]);
  }
}

/** Wildflowers: rosettes of pointed leaves with five-petalled flowers on top, petals lit on the NW. */
export function wildflowers(g: Graphics, r: number, rnd: () => number, v: number) {
  const [petal, eye] = [
    [0xe0533d, 0xf2c94c],
    [0xf2c94c, 0xc8702a],
    [0x9b7fd4, 0xe8d890],
    [0xf6efe0, 0xf2c94c],
  ][v];
  const greens = [0x46682e, 0x5a7e36, 0x6e9440];
  const spots: number[][] = [];
  for (let i = 0; i < 3; i++) {
    const a = (i / 3) * Math.PI * 2 + rnd();
    spots.push([Math.cos(a) * r * 0.4, Math.sin(a) * r * 0.4]);
  }
  for (const [x, y] of spots) tuft(g, x, y, 6, r * 0.55, r * 0.13, greens, rnd);
  const flowers: number[][] = [];
  for (const [x, y] of spots) {
    for (let k = 0; k < 2; k++) {
      const a = rnd() * Math.PI * 2;
      const d = r * 0.25 * rnd();
      flowers.push([x + Math.cos(a) * d, y + Math.sin(a) * d, r * (0.18 + 0.05 * rnd())]);
    }
  }
  // Each flower: its five petals outlined together in ink, then filled (lit on the NW), a small soft eye.
  for (const [x, y, s] of flowers) {
    const ph = rnd() * Math.PI;
    const petals = [0, 1, 2, 3, 4].map((p) => ph + (p / 5) * Math.PI * 2);
    for (const pa of petals) g.circle(x + Math.cos(pa) * s * 0.5, y + Math.sin(pa) * s * 0.5, s * 0.5 + 1.2).fill(INK);
    for (const pa of petals) g.circle(x + Math.cos(pa) * s * 0.5, y + Math.sin(pa) * s * 0.5, s * 0.5).fill(shade(petal, 1 + 0.15 * lit(pa)));
    g.circle(x, y, s * 0.2).fill(eye);
  }
}

/** Mushroom ring: capped mushrooms in a ring, darker grass where the ring runs. */
export function mushroomRing(g: Graphics, r: number, rnd: () => number, v: number) {
  const [cap, spot] = [
    [0xc8453a, 0xf6ead6],
    [0xa86a3a, 0xe8d4b0],
    [0xd8b070, 0xf6efe0],
    [0x8a5ab0, 0xd8c0f0],
  ][v];
  // Darker grass where the ring runs: uneven dabs, not a drawn circle.
  for (let i = 0; i < 14; i++) {
    const a = (i / 14) * Math.PI * 2 + rnd() * 0.3;
    const d = r * (0.6 + 0.12 * rnd());
    g.circle(Math.cos(a) * d, Math.sin(a) * d, r * (0.1 + 0.08 * rnd())).fill({ color: 0x2a3a1a, alpha: 0.12 });
  }
  const n = 7 + Math.floor(rnd() * 3);
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + (rnd() - 0.5) * 0.4;
    const d = r * (0.6 + 0.12 * rnd());
    const x = Math.cos(a) * d;
    const y = Math.sin(a) * d;
    const s = r * (0.12 + 0.07 * rnd());
    // Cap: a dark rim, the dome, a lit crown and spots; a smaller button beside some.
    if (rnd() < 0.4) {
      const b = rnd() * Math.PI * 2;
      g.circle(x + Math.cos(b) * s * 1.1, y + Math.sin(b) * s * 1.1, s * 0.45).fill(cap).stroke({ width: 1.2, color: INK });
    }
    if (v === 3) g.circle(x, y, s * 1.5).fill({ color: 0xc8a8f0, alpha: 0.18 });
    g.circle(x, y, s).fill(shade(cap, 0.7)).stroke({ width: 1.5, color: INK });
    g.circle(x - s * 0.12, y - s * 0.12, s * 0.8).fill(cap);
    g.circle(x - s * 0.32, y - s * 0.32, s * 0.32).fill({ color: shade(cap, 1.35), alpha: 0.9 });
    if (v !== 1) for (let k = 0; k < 3; k++) g.circle(x + (rnd() - 0.4) * s * 1.1, y + (rnd() - 0.4) * s * 1.1, s * 0.13).fill(spot);
  }
}

/** One ribbed cactus column from above: a fluted outline (a whorl of shallow ribs), lit on the NW, rib
 * creases and spine tufts on each rib. */
function cactusColumn(g: Graphics, x: number, y: number, s: number, ribs: number, rnd: () => number) {
  const [dark, mid, light] = [0x3f6a3a, 0x5a8a4a, 0x80ae5e];
  const tips: [number, number][] = [];
  for (let i = 0; i < ribs; i++) tips.push([(i / ribs) * Math.PI * 2, 1]);
  whorl(g, x, y, s, tips, 0.86);
  g.fill(dark).stroke({ width: 2, color: INK, join: 'round' });
  whorl(g, x - s * 0.1, y - s * 0.1, s * 0.82, tips, 0.86);
  g.fill(mid);
  for (let i = 0; i < ribs; i++) {
    const a = ((i + 0.5) / ribs) * Math.PI * 2;
    g.moveTo(x + Math.cos(a) * s * 0.2, y + Math.sin(a) * s * 0.2).lineTo(x + Math.cos(a) * s * 0.84, y + Math.sin(a) * s * 0.84);
  }
  g.stroke({ width: 1.5, color: shade(dark, 0.85), alpha: 0.8, cap: 'round' });
  g.circle(x - s * 0.28, y - s * 0.28, s * 0.3).fill({ color: light, alpha: 0.85 });
  if (s < 14) return;
  for (let i = 0; i < ribs; i++) {
    const a = (i / ribs) * Math.PI * 2;
    for (const d of [0.5, 0.82]) {
      const px = x + Math.cos(a) * s * d;
      const py = y + Math.sin(a) * s * d;
      for (const b of [a - 0.8, a, a + 0.8]) g.moveTo(px, py).lineTo(px + Math.cos(b) * 3, py + Math.sin(b) * 3);
    }
  }
  g.stroke({ width: 1, color: 0xf0e2b0, cap: 'round' });
  void rnd;
}

/** A flower of pointed petals at (x, y). */
function bloom(g: Graphics, x: number, y: number, s: number, color: number, rnd: () => number) {
  const ph = rnd() * Math.PI;
  for (let p = 0; p < 6; p++) leaf(g, x, y, ph + (p / 6) * Math.PI * 2, s, s * 0.3, shade(color, 1 + 0.12 * lit(ph + (p / 6) * Math.PI * 2)), 0.5);
  g.circle(x, y, s * 0.28).fill(0xf2c94c).stroke({ width: 1, color: INK });
}

/** Cactus: a saguaro with arms (v0, v3), a barrel cactus in flower (v1), or a prickly pear (v2). */
export function cactus(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v === 2) {
    // Prickly pear: overlapping oval pads, the top ones lighter, dotted with spine tufts, a fruit or two.
    const pads: [number, number, number, number][] = [];
    const ph = rnd() * Math.PI * 2;
    for (let i = 0; i < 4; i++) {
      const a = ph + (i / 4) * Math.PI * 2 + rnd() * 0.6;
      pads.push([Math.cos(a) * r * 0.45, Math.sin(a) * r * 0.45, a, 0]);
    }
    pads.push([0, 0, ph + 0.8, 1]);
    for (const [x, y, a, top] of pads) {
      const c = top ? 0x6a9a50 : 0x56864a;
      g.poly(oval(x, y, r * 0.42, r * 0.27, a)).fill(c).stroke({ width: 2, color: INK });
      g.poly(oval(x - r * 0.06, y - r * 0.06, r * 0.26, r * 0.14, a)).fill(shade(c, 1.2));
      for (let k = 0; k < 6; k++) {
        const t = rnd() * Math.PI * 2;
        const px = x + Math.cos(t) * r * 0.25 * rnd();
        const py = y + Math.sin(t) * r * 0.16 * rnd();
        g.moveTo(px - 2, py).lineTo(px + 2, py).moveTo(px, py - 2).lineTo(px, py + 2);
      }
      g.stroke({ width: 1, color: 0xf0e2b0 });
    }
    for (let k = 0; k < 2; k++) {
      const [x, y, a] = pads[k];
      const fx = x + Math.cos(a) * r * 0.36;
      const fy = y + Math.sin(a) * r * 0.36;
      g.poly(oval(fx, fy, r * 0.11, r * 0.08, a)).fill(0xb8305a).stroke({ width: 1.2, color: INK });
      g.circle(fx - 1.5, fy - 1.5, 1.5).fill({ color: 0xffffff, alpha: 0.5 });
    }
    return;
  }
  if (v === 1) {
    cactusColumn(g, 0, 0, r * 0.85, 14, rnd);
    bloom(g, -r * 0.05, -r * 0.05, r * 0.24, 0xf2c94c, rnd);
    return;
  }
  const arms = v === 0 ? 2 : 1;
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < arms; i++) {
    const a = ph + i * (2.2 + rnd());
    const x = Math.cos(a) * r * 0.58;
    const y = Math.sin(a) * r * 0.58;
    g.moveTo(0, 0).lineTo(x, y).stroke({ width: r * 0.34 + 3, color: INK, cap: 'round' });
    g.moveTo(0, 0).lineTo(x, y).stroke({ width: r * 0.34, color: 0x4a7a40, cap: 'round' });
    cactusColumn(g, x, y, r * 0.38, 8, rnd);
  }
  cactusColumn(g, 0, 0, r * 0.55, 10, rnd);
  if (v === 3) bloom(g, -r * 0.04, -r * 0.04, r * 0.18, 0xe88aa0, rnd);
}

/** Brambles: a low mound of leafy clumps, each a whorl of short leaf tips (between tall grass and a tree
 * canopy), stacked so the lit ones sit on top; a few thin thorny canes arch over it, berries. */
export function brambles(g: Graphics, r: number, rnd: () => number, v: number) {
  const greens = [
    [0x3a5a2c, 0x547a36, 0x749a46],
    [0x3c5a30, 0x587c3c, 0x7a9c4c],
    [0x445a2c, 0x607a34, 0x829a44],
    [0x365630, 0x50763c, 0x70964c],
  ][v];
  const outline = shade(greens[0], 0.5);
  const x0y0 = (a: number, d: number) => [Math.cos(a) * d, Math.sin(a) * d];
  const ph = rnd() * Math.PI * 2;
  // Leafy clumps: a ring round a middle one, the shaded side first.
  const parts: [number, number, number][] = [];
  for (let i = 0; i < 6; i++) {
    const a = ph + 0.4 + ((i + (rnd() - 0.5) * 0.5) / 6) * Math.PI * 2;
    parts.push([Math.cos(a) * r * 0.42, Math.sin(a) * r * 0.42, r * (0.36 + 0.06 * rnd())]);
  }
  parts.sort((p, q) => lit(Math.atan2(p[1], p[0])) - lit(Math.atan2(q[1], q[0])));
  parts.push([-r * 0.05, -r * 0.05, r * 0.42]);
  for (const [x, y, rr] of parts) {
    const tips = boughs(11, rnd);
    whorl(g, x, y, rr, tips, 0.62);
    g.fill(greens[0]).stroke({ width: 1.5, color: outline, join: 'round' });
    whorl(g, x - rr * 0.1, y - rr * 0.1, rr * 0.72, tips, 0.62);
    g.fill(greens[1]);
    whorl(g, x - rr * 0.2, y - rr * 0.2, rr * 0.36, boughs(7, rnd), 0.6);
    g.fill(greens[2]);
  }
  // A few thin thorny canes arching over the leaves.
  for (let i = 0; i < 4; i++) {
    const a0 = ph + (i / 4) * Math.PI * 2 + rnd() * 0.6;
    const a1 = a0 + 1.6 + rnd() * 0.8;
    const d0 = r * (0.55 + 0.25 * rnd());
    const d1 = r * (0.55 + 0.25 * rnd());
    const c = [x0y0(a0, d0), x0y0((a0 + a1) / 2, r * 0.25), x0y0(a1, d1)];
    for (const [w, col] of [
      [3, 0x3a1e14],
      [1.6, 0x9a5236],
    ]) {
      g.moveTo(c[0][0], c[0][1]).quadraticCurveTo(c[1][0], c[1][1], c[2][0], c[2][1]).stroke({ width: w, color: col, cap: 'round' });
    }
    for (const t of [0.3, 0.55, 0.8]) {
      const u = 1 - t;
      const x = u * u * c[0][0] + 2 * u * t * c[1][0] + t * t * c[2][0];
      const y = u * u * c[0][1] + 2 * u * t * c[1][1] + t * t * c[2][1];
      g.poly([x - 1.5, y, x + 1.5, y, x, y - 3.5]).fill(0xe0c090);
    }
  }
  const berry = [0x3a1e3a, 0xa8302a, 0x3a1e3a, 0x5a2a4a][v];
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.65 * Math.sqrt(rnd());
    const x = Math.cos(a) * d;
    const y = Math.sin(a) * d;
    for (let k = 0; k < 3; k++) {
      const bx = x + Math.cos(k * 2.1) * 3.2;
      const by = y + Math.sin(k * 2.1) * 3.2;
      g.circle(bx, by, 3).fill(berry).stroke({ width: 1, color: shade(berry, 0.5) });
      g.circle(bx - 1, by - 1, 0.9).fill({ color: 0xffffff, alpha: 0.6 });
    }
  }
}
