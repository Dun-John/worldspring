// Procedural painterly sprite atlas for battlemap objects (top-down, light from the NW):
// layered fills with a darker ink outline and NW highlights, in the spirit of hand-painted
// battlemap assets. Drawn once into one mipmapped texture (frames padded apart so no mip level
// blends two of them) so objects render as instanced particles.
// Frames are addressed by kind id and variant, so generated art can replace them later.
import { Container, Graphics, Rectangle, Texture, type Renderer } from 'pixi.js';
import type { KindInfo } from '../gen/client';

/** Atlas resolution: pixels per 5-ft square. */
export const PX = 64;
export const VARIANTS = 4;
const ATLAS_W = 2048;
/** Mip levels kept (full size down to 1/8), and the empty gap round every frame, on a grid of the same
 * size, so that even the smallest level never blends two frames. */
const MIP_LEVELS = 4;
const PAD = 1 << (MIP_LEVELS - 1);

export interface Atlas {
  /** frames[kindId][variant] */
  frames: Texture[][];
  /** Soft round shadow blob (unit radius = half the frame). */
  shadow: Texture;
}

type Draw = (g: Graphics, r: number, rnd: () => number, v: number) => void;

function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const INK = 0x1d1a14;

function shade(color: number, f: number): number {
  const r = Math.min(255, Math.max(0, ((color >> 16) & 255) * f));
  const g = Math.min(255, Math.max(0, ((color >> 8) & 255) * f));
  const b = Math.min(255, Math.max(0, (color & 255) * f));
  return (Math.round(r) << 16) | (Math.round(g) << 8) | Math.round(b);
}

/** Lumpy blob outline: n points around radius r with jitter. */
function blob(cx: number, cy: number, r: number, n: number, jitter: number, rnd: () => number): number[] {
  const pts: number[] = [];
  const phase = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = phase + (i / n) * Math.PI * 2;
    const rr = r * (1 - jitter / 2 + jitter * rnd());
    pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
  }
  return pts;
}

/** Leafy canopy: cluster of lobes, dark base → light top-left highlights, ink rim. */
function canopy(g: Graphics, r: number, rnd: () => number, colors: readonly number[], lobes: number) {
  const [dark, mid, light] = colors;
  const parts: [number, number, number][] = [];
  for (let i = 0; i < lobes; i++) {
    const a = (i / lobes) * Math.PI * 2 + rnd() * 0.6;
    const d = r * (0.35 + 0.2 * rnd());
    parts.push([Math.cos(a) * d, Math.sin(a) * d, r * (0.42 + 0.14 * rnd())]);
  }
  parts.push([0, 0, r * 0.55]);
  for (const [x, y, rr] of parts) g.circle(x, y, rr + 2).fill(INK);
  for (const [x, y, rr] of parts) g.circle(x, y, rr).fill(dark);
  for (const [x, y, rr] of parts) g.circle(x - rr * 0.18, y - rr * 0.18, rr * 0.78).fill(mid);
  for (const [x, y, rr] of parts) g.circle(x - rr * 0.34, y - rr * 0.34, rr * 0.42).fill({ color: light, alpha: 0.9 });
  for (let i = 0; i < lobes * 2; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.8 * rnd();
    g.circle(Math.cos(a) * d - r * 0.1, Math.sin(a) * d - r * 0.1, r * 0.06).fill({ color: shade(light, 1.2), alpha: 0.7 });
  }
}

/** How much a direction faces the NW light: 1 toward it, -1 away. */
function lit(a: number): number {
  return -(Math.cos(a) + Math.sin(a)) * Math.SQRT1_2;
}

/** A whorl's bough tips: `n` (angle, radius fraction) pairs, jittered. */
function boughs(n: number, rnd: () => number): [number, number][] {
  const ph = rnd() * Math.PI * 2;
  const tips: [number, number][] = [];
  for (let i = 0; i < n; i++) tips.push([ph + ((i + (rnd() - 0.5) * 0.45) / n) * Math.PI * 2, 0.8 + 0.2 * rnd()]);
  return tips;
}

/** A ring of bough tips round (cx, cy) at radius `rr`, valleys at `inner` of it: slightly full sides
 * meeting in a soft point. */
function whorl(g: Graphics, cx: number, cy: number, rr: number, tips: [number, number][], inner: number) {
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
function conifer(g: Graphics, r: number, rnd: () => number, v: number) {
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
    if (l === 0) g.stroke({ width: 2.5, color: INK, join: 'round' });
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
function palm(g: Graphics, r: number, rnd: () => number, v: number) {
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
function deadTree(g: Graphics, r: number, rnd: () => number, v: number) {
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

function strokes(g: Graphics, r: number, rnd: () => number, n: number, len: number, colors: number[], width: number) {
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

/** An ellipse as points, turned by `rot`. */
function oval(cx: number, cy: number, rx: number, ry: number, rot: number, n = 16): number[] {
  const c = Math.cos(rot);
  const s = Math.sin(rot);
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2;
    const x = Math.cos(t) * rx;
    const y = Math.sin(t) * ry;
    pts.push(cx + x * c - y * s, cy + x * s + y * c);
  }
  return pts;
}

/** A pointed leaf or blade from (x, y) out along `a`, widest at `at` of its length, with a lighter
 * midrib; `bend` bows it sideways. */
function leaf(g: Graphics, x: number, y: number, a: number, len: number, w: number, color: number, at = 0.35, bend = 0) {
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
function tuft(g: Graphics, x: number, y: number, n: number, len: number, w: number, colors: readonly number[], rnd: () => number) {
  const blades: [number, number][] = [];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) blades.push([ph + ((i + (rnd() - 0.5) * 0.7) / n) * Math.PI * 2, len * (0.55 + 0.45 * rnd())]);
  blades.sort((p, q) => q[1] - p[1]);
  for (const [a, l] of blades) leaf(g, x, y, a, l, w, shade(colors[Math.floor(rnd() * colors.length)], 1 + 0.18 * lit(a)), 0.3, (rnd() - 0.5) * 0.3);
}

/** Reeds: bold tufts of narrow blades like tall grass (a lighter tuft over a darker one, toned outlines),
 * with brown cattail heads standing among them. */
function reeds(g: Graphics, r: number, rnd: () => number, v: number) {
  const greens = [
    [0x5e8a48, 0x7aa456, 0x9cc070],
    [0x568646, 0x72a058, 0x94bc72],
    [0x6a8c44, 0x88a652, 0xaac26a],
    [0x52824c, 0x6e9c5e, 0x8eb878],
  ][v];
  const n = 2 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  const spots: number[][] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    const x = Math.cos(a) * r * 0.28;
    const y = Math.sin(a) * r * 0.28;
    spots.push([x, y]);
    const rr = r * (0.66 + 0.08 * rnd());
    const tips = boughs(12, rnd).map(([t, k]): [number, number] => [t, k * (0.65 + 0.35 * rnd())]);
    whorl(g, x, y, rr, tips, 0.35);
    g.fill(greens[0]).stroke({ width: 1.5, color: shade(greens[0], 0.45), join: 'round' });
    whorl(g, x - rr * 0.08, y - rr * 0.08, rr * 0.7, tips, 0.35);
    g.fill(greens[1]);
    whorl(g, x - rr * 0.14, y - rr * 0.14, rr * 0.38, boughs(8, rnd), 0.4);
    g.fill(greens[2]);
  }
  // Cattails: brown heads, lit on the NW, outlined in a dark brown.
  for (let i = 0; i < 4; i++) {
    const [x0, y0] = spots[i % n];
    const a = rnd() * Math.PI * 2;
    const x = x0 + Math.cos(a) * r * 0.2;
    const y = y0 + Math.sin(a) * r * 0.2;
    g.poly(oval(x, y, r * 0.14, r * 0.07, a)).fill(0x6e4428).stroke({ width: 1.5, color: 0x3a2414 });
    g.poly(oval(x - r * 0.02, y - r * 0.02, r * 0.09, r * 0.03, a)).fill(0x9a6a40);
  }
}

/** Tall grass: two or three bold tufts, each a fountain of narrow blades (drawn like a conifer whorl) with
 * a lighter inner tuft nearer the light; few, big shapes so it stays clean zoomed out. */
function tallGrass(g: Graphics, r: number, rnd: () => number, v: number) {
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
function wildflowers(g: Graphics, r: number, rnd: () => number, v: number) {
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
function mushroomRing(g: Graphics, r: number, rnd: () => number, v: number) {
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
function cactus(g: Graphics, r: number, rnd: () => number, v: number) {
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
function brambles(g: Graphics, r: number, rnd: () => number, v: number) {
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

/** Stone tones per variant: [dark, mid, light, sunlit]. */
const STONE = [
  [0x6e675c, 0x9a9182, 0xc4baa6, 0xe4dccb],
  [0x625f5a, 0x8c8a84, 0xb6b2a8, 0xd8d4c8],
  [0x6a6454, 0x958d76, 0xbeb498, 0xe0d6bc],
  [0x5e5a56, 0x86817a, 0xaea89c, 0xd2ccc0],
];

/** One stone from above, cut like a gem: a dark rim on the shaded side, the body, a lit top facet pushed
 * toward the NW with facet lines down to the body, a sunlit spot, a crack on the bigger ones. */
function stone(g: Graphics, x: number, y: number, s: number, tones: readonly number[], rnd: () => number, n = 8) {
  const pts = blob(0, 0, s, n, 0.28, rnd);
  const at = (k: number, dx: number) => pts.map((p, i) => p * k + dx + (i % 2 ? y : x));
  const ink = Math.max(1.2, Math.min(2.5, s * 0.07));
  g.poly(at(1, 0)).fill(tones[0]).stroke({ width: ink, color: INK, join: 'round' });
  const body = at(0.86, -s * 0.06);
  g.poly(body).fill(tones[1]);
  const top = at(0.55, -s * 0.17);
  g.poly(top).fill(tones[2]);
  if (s >= 8) {
    for (let i = 0; i < pts.length; i += 4) g.moveTo(top[i], top[i + 1]).lineTo(body[i], body[i + 1]);
    g.stroke({ width: 1.2, color: tones[0], alpha: 0.7, cap: 'round' });
    g.poly(at(0.2, -s * 0.32)).fill({ color: tones[3], alpha: 0.85 });
  }
  if (s >= 20) {
    const a = rnd() * Math.PI * 2;
    let [cx, cy] = [x + Math.cos(a) * s * 0.85, y + Math.sin(a) * s * 0.85];
    g.moveTo(cx, cy);
    for (let k = 0; k < 3; k++) {
      cx += (x - cx) * 0.3 + (rnd() - 0.5) * s * 0.15;
      cy += (y - cy) * 0.3 + (rnd() - 0.5) * s * 0.15;
      g.lineTo(cx, cy);
    }
    g.stroke({ width: 1.5, color: shade(tones[0], 0.7), cap: 'round', join: 'round' });
  }
}

/** Moss: a soft cushion of small lumps, lighter toward the light, on a stone's north side. */
function moss(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  const lumps: number[][] = [];
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = s * 0.5 * Math.sqrt(rnd());
    lumps.push([x + Math.cos(a) * d, y + Math.sin(a) * d, s * (0.38 + 0.15 * rnd())]);
  }
  for (const [lx, ly, lr] of lumps) g.circle(lx, ly, lr + 1).fill(0x3a5222);
  for (const [lx, ly, lr] of lumps) g.circle(lx, ly, lr).fill(0x5a7a32);
  for (const [lx, ly, lr] of lumps) g.circle(lx - lr * 0.25, ly - lr * 0.25, lr * 0.5).fill(0x7e9e44);
}

/** Boulder: one big faceted stone, a pebble or two at its foot, moss on some. */
function boulderStone(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = STONE[v];
  for (let i = 0; i < 2; i++) {
    const a = Math.PI * 0.25 + (rnd() - 0.5) * 2;
    stone(g, Math.cos(a) * r * 0.9, Math.sin(a) * r * 0.9, r * 0.13, tones, rnd, 6);
  }
  stone(g, -r * 0.04, -r * 0.04, r * 0.76, tones, rnd, 9);
  if (v === 2 || v === 3) {
    moss(g, -r * 0.1, -r * 0.5, r * 0.22, rnd);
    moss(g, r * 0.25, -r * 0.42, r * 0.14, rnd);
  }
}

/** Rocks: three or four small stones. */
function smallRocks(g: Graphics, r: number, rnd: () => number, v: number) {
  const n = 3 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    const d = i === 0 ? 0 : r * 0.55;
    stone(g, Math.cos(a) * d, Math.sin(a) * d, r * (i === 0 ? 0.45 : 0.28 + 0.1 * rnd()), STONE[v], rnd, 7);
  }
}

/** Rock pile: a loose, lopsided heap — stones of mixed sizes scattered over an uneven patch, the lower ones
 * drawn first and the ones resting on top last, no rings or rows. */
function rockPile(g: Graphics, r: number, rnd: () => number, v: number) {
  const rot = rnd() * Math.PI;
  const [ax, ay] = [1, 0.7 + 0.2 * rnd()];
  const parts: [number, number, number, number][] = [];
  for (let tries = 0; parts.length < 11 && tries < 200; tries++) {
    const a = rnd() * Math.PI * 2;
    const d = Math.sqrt(rnd()) * r * 0.72;
    const u = Math.cos(a) * d * ax;
    const w = Math.sin(a) * d * ay;
    const x = u * Math.cos(rot) - w * Math.sin(rot);
    const y = u * Math.sin(rot) + w * Math.cos(rot);
    const s = r * (rnd() < 0.3 ? 0.3 + 0.08 * rnd() : 0.15 + 0.1 * rnd());
    // Keep stones from sitting squarely on one another.
    if (parts.some(([px, py, ps]) => Math.hypot(px - x, py - y) < (ps + s) * 0.55)) continue;
    if (Math.hypot(x, y) + s > r * 0.98) continue;
    // Height in the heap: bigger stones and those nearer the middle sit lower or higher at random.
    parts.push([x, y, s, rnd() + (s > r * 0.25 ? -0.3 : 0)]);
  }
  parts.sort((p, q) => p[3] - q[3]);
  for (const [x, y, s] of parts) stone(g, x, y, s, STONE[v], rnd, 7);
  if (v === 2) moss(g, parts[0][0], parts[0][1] - parts[0][2] * 0.5, r * 0.14, rnd);
}

/** Loose scree: a spread of flat, low-contrast stones and gravel. */
function scree(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = STONE[v].map((c) => shade(c, 1.04));
  for (let i = 0; i < 40; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.9 * Math.sqrt(rnd());
    g.circle(Math.cos(a) * d, Math.sin(a) * d, 1.5 + rnd() * 1.5).fill({ color: tones[rnd() < 0.5 ? 0 : 2], alpha: 0.8 });
  }
  for (let i = 0; i < 22; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.85 * Math.sqrt(rnd());
    stone(g, Math.cos(a) * d, Math.sin(a) * d, r * (0.07 + 0.07 * rnd()), tones, rnd, 6);
  }
}

/** Bark tones per variant: [dark, mid, light]. */
const BARK = [
  [0x4e3424, 0x7d5a3c, 0xa8825a],
  [0x4a3a2c, 0x76604a, 0xa08a6c],
  [0x523628, 0x84603e, 0xb08a5e],
  [0x463830, 0x6e6052, 0x9a8c78],
];

/** End grain of a cut trunk: pale wood, growth rings, a crack. */
function endGrain(g: Graphics, x: number, y: number, rx: number, ry: number, bark: readonly number[]) {
  g.poly(oval(x, y, rx, ry, 0)).fill(0xc89a68).stroke({ width: 1.5, color: INK });
  for (const f of [0.68, 0.4]) g.poly(oval(x - rx * 0.05, y, rx * f, ry * f, 0)).stroke({ width: 1.2, color: shade(bark[2], 0.85), alpha: 0.9 });
  g.moveTo(x, y).lineTo(x + rx * 0.7, y - ry * 0.5).stroke({ width: 1.2, color: bark[0], alpha: 0.8 });
}

/** Fallen log along x: an uneven, slightly bent trunk that thins toward its splintered end (-x), lumpy
 * bark edges, a soft lit band along the top, furrows, a knot, a bent broken branch stub and a cut end with
 * rings (+x); moss, or a few mushrooms, on some. */
function fallenLog(g: Graphics, r: number, rnd: () => number, v: number) {
  const bark = BARK[v];
  const len = r * 1.7;
  const w = r * 0.44;
  const x0 = -len / 2;
  const x1 = len / 2 - w * 0.2;
  const bend = (rnd() - 0.5) * w * 0.5;
  // Centre line and half-width at t along the trunk (0 at the splintered end, 1 at the cut end).
  const mid = (t: number) => bend * Math.sin(Math.PI * t);
  const half = (t: number) => (w / 2) * (0.82 + 0.18 * t);
  const n = 10;
  const top: number[] = [];
  const bot: number[] = [];
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const x = x0 + w * 0.2 + (x1 - x0 - w * 0.2) * t;
    const lump = i === n ? 0 : (rnd() - 0.5) * w * 0.12;
    top.push(x, mid(t) - half(t) + lump);
    bot.push(x, mid(t) + half(t) + (i === n ? 0 : (rnd() - 0.5) * w * 0.12));
  }
  // A bent, tapering branch stub, broken off.
  const st = 0.35 + 0.3 * rnd();
  const sx = x0 + len * st;
  const side = rnd() < 0.5 ? 1 : -1;
  const sy = mid(st) + side * half(st) * 0.6;
  const kx = sx + w * 0.35 + (rnd() - 0.5) * w * 0.2;
  const ky = sy + side * w * 0.55;
  const ex = kx + w * 0.05;
  const ey = ky + side * w * 0.3;
  const bw = w * 0.23;
  const stub = [sx - bw, sy, kx - bw * 0.7, ky, ex - bw * 0.5, ey, ex + bw * 0.1, ey - side * bw * 0.4, ex + bw * 0.5, ey, kx + bw * 0.7, ky, sx + bw, sy];
  g.poly(stub).fill(bark[1]).stroke({ width: 2, color: INK, join: 'round' });
  g.circle(ex, ey - side * bw * 0.15, bw * 0.35).fill(0xb08a5a);
  // The trunk outline: the top edge, the cut end, the bottom edge back, then the splintered end.
  const outline = [...top];
  for (let i = bot.length - 2; i >= 0; i -= 2) outline.push(bot[i], bot[i + 1]);
  const ys = mid(0);
  outline.push(x0 + w * 0.1, ys + w * 0.25, x0 - w * 0.05, ys + w * 0.1, x0 + w * 0.15, ys - w * 0.02, x0, ys - w * 0.2);
  g.poly(outline).fill(bark[0]).stroke({ width: 2.5, color: INK, join: 'round' });
  // The body (a little in from the shaded side), then a soft lit band near the top edge.
  const body: number[] = [];
  const band: number[] = [];
  for (let i = 0; i <= n; i++) {
    const x = top[i * 2];
    body.push(x, top[i * 2 + 1] + w * 0.05);
    band.push(x, top[i * 2 + 1] + w * 0.12);
  }
  for (let i = n; i >= 0; i--) {
    const t = i / n;
    body.push(top[i * 2], mid(t) + half(t) * 0.55);
    band.push(top[i * 2], top[i * 2 + 1] + w * 0.24);
  }
  g.poly(body).fill(bark[1]);
  g.poly(band).fill({ color: bark[2], alpha: 0.45 });
  // Furrows along the bark, following the bend.
  for (let i = 0; i < 7; i++) {
    const t0 = 0.1 + rnd() * 0.6;
    const t1 = t0 + 0.15 + rnd() * 0.15;
    const f = (rnd() - 0.5) * 1.4;
    const p = (t: number) => [x0 + w * 0.2 + (x1 - x0 - w * 0.2) * t, mid(t) + half(t) * f];
    const [ax, ay] = p(t0);
    const [cx, cy] = p((t0 + t1) / 2);
    const [bx, by] = p(t1);
    g.moveTo(ax, ay).quadraticCurveTo(cx, cy + (rnd() - 0.5) * 3, bx, by);
  }
  g.stroke({ width: 1.3, color: bark[0], alpha: 0.85, cap: 'round' });
  const kt = 0.45 + 0.2 * rnd();
  g.poly(oval(x0 + len * kt, mid(kt) + w * 0.05, w * 0.13, w * 0.08, 0)).fill(bark[0]).stroke({ width: 1, color: INK });
  // The cut end: muted wood, rings and a crack.
  const ey0 = mid(1);
  g.poly(oval(x1, ey0, w * 0.22, half(1), 0)).fill(0xb08658).stroke({ width: 1.5, color: INK });
  for (const k of [0.66, 0.36]) g.poly(oval(x1 - w * 0.02, ey0, w * 0.22 * k, half(1) * k, 0)).stroke({ width: 1.1, color: shade(bark[1], 0.9), alpha: 0.9 });
  g.moveTo(x1, ey0).lineTo(x1 + w * 0.12, ey0 - half(1) * 0.6).stroke({ width: 1.1, color: bark[0], alpha: 0.8 });
  if (v === 1) {
    for (let i = 0; i < 3; i++) {
      const t = 0.2 + 0.25 * i;
      moss(g, x0 + len * t, mid(t) - half(t) * 0.45, w * 0.3, rnd);
    }
  }
  if (v === 3) {
    for (let i = 0; i < 3; i++) {
      const t = 0.3 + 0.15 * i;
      const mx = x0 + len * t;
      const my = mid(t) + half(t) * (0.95 + 0.15 * rnd());
      g.circle(mx, my, w * 0.16).fill(0xc8a070).stroke({ width: 1.2, color: INK });
      g.circle(mx - w * 0.04, my - w * 0.04, w * 0.07).fill(0xe8d0a0);
    }
  }
}

/** Stump: a ragged bark ring with roots spreading from it, the cut face with rings and a crack. */
function stump(g: Graphics, r: number, rnd: () => number, v: number) {
  const bark = BARK[v];
  const ph = rnd() * Math.PI * 2;
  // Roots: short rounded flares where the trunk meets the ground.
  for (let i = 0; i < 5; i++) {
    const a = ph + (i / 5) * Math.PI * 2 + rnd() * 0.4;
    const d = r * (0.55 + 0.08 * rnd());
    g.poly(oval(Math.cos(a) * d, Math.sin(a) * d, r * 0.36, r * 0.2, a, 12))
      .fill(shade(bark[1], 1 + 0.15 * lit(a)))
      .stroke({ width: 1.8, color: INK, join: 'round' });
  }
  g.poly(blob(0, 0, r * 0.6, 10, 0.15, rnd)).fill(bark[0]).stroke({ width: 2, color: INK, join: 'round' });
  g.poly(blob(-r * 0.03, -r * 0.03, r * 0.52, 10, 0.12, rnd)).fill(bark[1]);
  g.circle(-r * 0.02, -r * 0.02, r * 0.42).fill(0xc89a68);
  g.circle(-r * 0.06, -r * 0.06, r * 0.25).fill({ color: 0xe0b47a, alpha: 0.8 });
  for (const f of [0.3, 0.16]) g.circle(-r * 0.02, -r * 0.02, r * f).stroke({ width: 1, color: shade(bark[2], 0.85), alpha: 0.9 });
  const a = rnd() * Math.PI * 2;
  g.moveTo(-r * 0.02, -r * 0.02).lineTo(Math.cos(a) * r * 0.4, Math.sin(a) * r * 0.4).stroke({ width: 1.2, color: bark[0] });
  if (v === 1) moss(g, -r * 0.3, -r * 0.5, r * 0.22, rnd);
}

/** Old bones: pale bone with a dark brown outline so they show on any ground — a skull, ribs, long bones. */
function bones(g: Graphics, r: number, rnd: () => number, v: number) {
  const [mid, light] = [0xdccfae, 0xf2e8cc];
  const line = 0x4a3e30;
  const longBone = (x: number, y: number, a: number, len: number, w: number) => {
    const c = Math.cos(a);
    const s = Math.sin(a);
    const ends = [
      [x - c * len * 0.5, y - s * len * 0.5],
      [x + c * len * 0.5, y + s * len * 0.5],
    ];
    g.moveTo(ends[0][0], ends[0][1]).lineTo(ends[1][0], ends[1][1]).stroke({ width: w + 3, color: line, cap: 'round' });
    for (const [ex, ey] of ends) for (const side of [1, -1]) g.circle(ex - s * side * w * 0.45, ey + c * side * w * 0.45, w * 0.62).fill(line);
    g.moveTo(ends[0][0], ends[0][1]).lineTo(ends[1][0], ends[1][1]).stroke({ width: w, color: mid, cap: 'round' });
    for (const [ex, ey] of ends) for (const side of [1, -1]) g.circle(ex - s * side * w * 0.45, ey + c * side * w * 0.45, w * 0.45).fill(mid);
    g.moveTo(ends[0][0] - s * w * 0.2, ends[0][1] + c * w * 0.2).lineTo(ends[1][0] - s * w * 0.2, ends[1][1] + c * w * 0.2).stroke({ width: w * 0.3, color: light, cap: 'round' });
  };
  const skull = (x: number, y: number, s: number, a: number) => {
    const c = Math.cos(a);
    const sn = Math.sin(a);
    const p = (u: number, w: number) => [x + c * u - sn * w, y + sn * u + c * w];
    g.poly(oval(x, y, s, s * 0.8, a)).fill(mid).stroke({ width: 1.5, color: line });
    const [fx, fy] = p(s * 0.75, 0);
    g.poly(oval(fx, fy, s * 0.45, s * 0.42, a)).fill(mid).stroke({ width: 1.5, color: line });
    g.poly(oval(...(p(-s * 0.2, -s * 0.2) as [number, number]), s * 0.5, s * 0.35, a)).fill({ color: light, alpha: 0.9 });
    for (const side of [1, -1]) {
      const [ex, ey] = p(s * 0.55, side * s * 0.35);
      g.circle(ex, ey, s * 0.18).fill(0x2a2218);
    }
  };
  const ribs = (x: number, y: number, a: number, s: number) => {
    const c = Math.cos(a);
    const sn = Math.sin(a);
    longBone(x, y, a, s * 2, s * 0.22);
    for (let i = 0; i < 4; i++) {
      const u = -s * 0.75 + i * s * 0.5;
      for (const side of [1, -1]) {
        const bx = x + c * u;
        const by = y + sn * u;
        const tx = bx - sn * side * s * 0.9 + c * s * 0.25;
        const ty = by + c * side * s * 0.9 + sn * s * 0.25;
        const mx = bx - sn * side * s * 0.6 - c * s * 0.15;
        const my = by + c * side * s * 0.6 - sn * s * 0.15;
        g.moveTo(bx, by).quadraticCurveTo(mx, my, tx, ty).stroke({ width: 4.5, color: line, cap: 'round' });
        g.moveTo(bx, by).quadraticCurveTo(mx, my, tx, ty).stroke({ width: 2.2, color: mid, cap: 'round' });
      }
    }
  };
  const a = rnd() * Math.PI * 2;
  if (v === 0) {
    ribs(r * 0.15, r * 0.1, a, r * 0.45);
    skull(-r * 0.5, -r * 0.35, r * 0.24, a + 2.5);
  } else if (v === 1) {
    longBone(0, 0, a, r * 1.3, r * 0.14);
    longBone(r * 0.2, r * 0.35, a + 1.2, r * 0.8, r * 0.1);
    longBone(-r * 0.35, r * 0.15, a - 0.8, r * 0.6, r * 0.09);
  } else if (v === 2) {
    longBone(r * 0.1, r * 0.3, a, r * 1.1, r * 0.13);
    skull(-r * 0.25, -r * 0.3, r * 0.3, a + 1);
  } else {
    longBone(0, r * 0.1, a + 0.7, r * 1.1, r * 0.12);
    longBone(0, r * 0.1, a - 0.7, r * 1.1, r * 0.12);
    skull(0, -r * 0.2, r * 0.3, a - Math.PI / 2);
  }
}

/** Points of `pts` scaled by `k` about (cx, cy) and moved by (dx, dy). */
function scaled(pts: number[], k: number, cx = 0, cy = 0, dx = 0, dy = 0): number[] {
  return pts.map((p, i) => (i % 2 ? cy + (p - cy) * k + dy : cx + (p - cx) * k + dx));
}

/** A patch that fades into the ground: the shape drawn six times, shrinking, each very faint. */
function fade(g: Graphics, pts: number[], color: number, alpha: number) {
  for (let i = 0; i < 6; i++) g.poly(scaled(pts, 1 - i * 0.025)).fill({ color, alpha: alpha / 2.5 });
}

/** A crystal shard from (x, y) out along `a`: a long kite split down its spine, the side toward the light
 * paler, with a glint near the tip. */
function shard(g: Graphics, x: number, y: number, a: number, len: number, w: number, tones: readonly number[], line: number) {
  const c = Math.cos(a);
  const s = Math.sin(a);
  const tip = [x + c * len, y + s * len];
  const l = [x + c * len * 0.35 - s * w, y + s * len * 0.35 + c * w];
  const rt = [x + c * len * 0.35 + s * w, y + s * len * 0.35 - c * w];
  const base = [x - c * w * 0.4, y - s * w * 0.4];
  g.poly([...base, ...l, ...tip, ...rt]).fill(tones[0]).stroke({ width: 1.8, color: line, join: 'round' });
  // The half facing the light (NW) is the lit facet.
  const lSide = lit(a + Math.PI / 2) > lit(a - Math.PI / 2) ? l : rt;
  g.poly([...base, ...lSide, ...tip]).fill(tones[2]);
  g.moveTo(base[0], base[1]).lineTo(tip[0], tip[1]).stroke({ width: 1, color: tones[1], alpha: 0.9 });
  g.poly([tip[0] - c * len * 0.25, tip[1] - s * len * 0.25, lSide[0] * 0.3 + tip[0] * 0.7, lSide[1] * 0.3 + tip[1] * 0.7, tip[0], tip[1]]).fill({ color: tones[3], alpha: 0.9 });
}

/** A ring of shards round a short central point, the shaded ones first. */
function crystals(g: Graphics, r: number, rnd: () => number, n: number, tones: readonly number[], line: number) {
  const ph = rnd() * Math.PI * 2;
  const parts: [number, number, number][] = [];
  for (let i = 0; i < n; i++) parts.push([ph + ((i + (rnd() - 0.5) * 0.6) / n) * Math.PI * 2, r * (0.6 + 0.35 * rnd()), r * (0.16 + 0.06 * rnd())]);
  parts.sort((p, q) => lit(p[0]) - lit(q[0]));
  for (const [a, len, w] of parts) shard(g, Math.cos(a) * r * 0.1, Math.sin(a) * r * 0.1, a, len, w, tones, line);
  for (let i = 0; i < 4; i++) {
    const a = ph + (i / 4) * Math.PI * 2 + 0.4;
    shard(g, 0, 0, a, r * 0.32, r * 0.14, tones.map((t) => shade(t, 1.08)), line);
  }
}

/** Snowdrift: a lumpy mound with soft lavender shade on the SE, two lighter tiers toward the light and
 * wind-swept ridge lines. */
function snowdrift(g: Graphics, r: number, rnd: () => number, v: number) {
  const pts = blob(0, 0, r * 0.95, 14, 0.18, rnd);
  g.poly(pts).fill(0xb9c6dc).stroke({ width: 1.8, color: 0x8a98b4, join: 'round' });
  g.poly(scaled(pts, 0.84, 0, 0, -r * 0.06, -r * 0.06)).fill(0xdfe6ef);
  g.poly(scaled(blob(0, 0, r * 0.95, 11, 0.22, rnd), 0.55, 0, 0, -r * 0.16, -r * 0.16)).fill(0xf2f4f6);
  g.poly(blob(-r * 0.3, -r * 0.3, r * 0.14, 7, 0.3, rnd)).fill({ color: 0xfffaf0, alpha: 0.9 });
  const a = rnd() * Math.PI * 2 + v;
  for (let i = 0; i < 3; i++) {
    const d = r * (0.25 + 0.2 * i);
    const b = a + (rnd() - 0.5) * 0.6;
    g.moveTo(Math.cos(b - 0.7) * d, Math.sin(b - 0.7) * d).quadraticCurveTo(Math.cos(b) * d * 1.15, Math.sin(b) * d * 1.15, Math.cos(b + 0.7) * d, Math.sin(b + 0.7) * d);
  }
  g.stroke({ width: 1.5, color: 0xaab8d0, alpha: 0.9, cap: 'round' });
}

/** Ice spire: a cluster of pale blue crystal shards. */
function iceSpire(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = [
    [0x5a8fb8, 0x8ab8da, 0xa8d4ea, 0xeef8ff],
    [0x5a98b0, 0x86c0d4, 0xa8dae6, 0xeefaff],
    [0x6a88c0, 0x92b2e0, 0xb2cef0, 0xf2f6ff],
    [0x5290b0, 0x7cb6d2, 0x9ed2e8, 0xeaf8ff],
  ][v];
  crystals(g, r * 0.95, rnd, 6, tones, 0x2e5470);
}

/** Basalt pillar: a honeycomb of hexagonal column tops at different heights (the higher ones lighter),
 * each bevelled toward the light, cracks across some. */
function basaltPillar(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = [0x2e2a30, 0x4a4450, 0x6a6270, 0x8a7aa8].map((c) => shade(c, 1 + (v - 1.5) * 0.04));
  const s = r * 0.3;
  const cells: [number, number, number][] = [];
  const axial = [[0, 0], [1, 0], [0, 1], [-1, 1], [-1, 0], [0, -1], [1, -1]];
  const skip = Math.floor(rnd() * 6) + 1;
  for (let i = 0; i < axial.length; i++) {
    if (i === skip) continue;
    const [q, rr] = axial[i];
    cells.push([s * Math.sqrt(3) * (q + rr / 2), s * 1.5 * rr, rnd()]);
  }
  cells.sort((p, q) => p[2] - q[2]);
  for (const [x, y, h] of cells) {
    const hex = (k: number, dx = 0) => {
      const pts: number[] = [];
      for (let i = 0; i < 6; i++) pts.push(x + dx + Math.cos(Math.PI / 6 + (i / 6) * Math.PI * 2) * s * k, y + dx + Math.sin(Math.PI / 6 + (i / 6) * Math.PI * 2) * s * k);
      return pts;
    };
    const lift = 0.75 + 0.35 * h;
    g.poly(hex(0.98)).fill(shade(tones[0], lift)).stroke({ width: 2, color: INK, join: 'round' });
    g.poly(hex(0.8, -s * 0.06)).fill(shade(tones[1], lift));
    g.poly(hex(0.5, -s * 0.14)).fill(shade(tones[2], lift));
    if (h > 0.6) {
      g.moveTo(x - s * 0.5, y + s * 0.1).lineTo(x - s * 0.1, y + s * 0.05).lineTo(x + s * 0.3, y + s * 0.4).stroke({ width: 1.2, color: tones[0], cap: 'round' });
    }
  }
}

/** Obsidian outcrop: jagged black glass shards with violet glints, on a little rubble. */
function obsidian(g: Graphics, r: number, rnd: () => number, v: number) {
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    stone(g, Math.cos(a) * r * 0.7, Math.sin(a) * r * 0.7, r * 0.1, [0x2a2630, 0x3a3440, 0x4e4658, 0x7a6a98], rnd, 6);
  }
  crystals(g, r * 0.8, rnd, 5 + (v % 2), [0x1a1420, 0x2e2638, 0x4a3e5c, 0xa898d0], INK);
}

/** Bog: dark peaty water fading into the ground through a mossy margin, scum and lily pads on it, a few
 * reed tufts and bubbles. */
function bog(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 14, 0.15, rnd);
  fade(g, edge, 0x4a5a2a, 0.3);
  const water = scaled(blob(0, 0, r * 0.95, 13, 0.2, rnd), 0.78);
  g.poly(water).fill(0x3a4228).stroke({ width: 1.8, color: 0x2a3018, join: 'round' });
  g.poly(scaled(water, 0.75, 0, 0, r * 0.05, r * 0.05)).fill(0x2e3420);
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.5 * Math.sqrt(rnd());
    g.poly(blob(Math.cos(a) * d, Math.sin(a) * d, r * (0.08 + 0.06 * rnd()), 7, 0.4, rnd)).fill({ color: 0x8a9a4a, alpha: 0.75 });
  }
  // Lily pads: discs with a notch, lit on the NW.
  for (let i = 0; i < 3 + (v % 2); i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.45 * Math.sqrt(rnd());
    const x = Math.cos(a) * d;
    const y = Math.sin(a) * d;
    const s = r * 0.08;
    const n = rnd() * Math.PI * 2;
    g.moveTo(x, y).arc(x, y, s, n + 0.4, n + Math.PI * 2 - 0.4).closePath().fill(0x5e8a3a).stroke({ width: 1, color: 0x2a3a18 });
    g.circle(x - s * 0.3, y - s * 0.3, s * 0.35).fill({ color: 0x86b052, alpha: 0.8 });
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    const x = Math.cos(a) * r * 0.25;
    const y = Math.sin(a) * r * 0.25;
    g.circle(x, y, r * 0.03).stroke({ width: 1, color: 0xa8b080, alpha: 0.8 });
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    const x = Math.cos(a) * r * 0.74;
    const y = Math.sin(a) * r * 0.74;
    const tips = boughs(9, rnd);
    whorl(g, x, y, r * 0.17, tips, 0.35);
    g.fill(0x5e7a3a).stroke({ width: 1.2, color: 0x34461e, join: 'round' });
    whorl(g, x - r * 0.02, y - r * 0.02, r * 0.11, tips, 0.35);
    g.fill(0x7e9a48);
  }
}

/** Quicksand: a sandy patch fading into the ground, darker wet sand in the middle drawn into a swirl,
 * ripples and a few bubbles. */
function quicksand(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 14, 0.15, rnd);
  fade(g, edge, 0xc8a464, 0.35);
  const wet = scaled(blob(0, 0, r * 0.95, 12, 0.18, rnd), 0.72);
  g.poly(wet).fill(0xa8824c).stroke({ width: 1.5, color: 0x7a5a32, join: 'round' });
  g.poly(scaled(wet, 0.7, 0, 0, r * 0.04, r * 0.04)).fill(0x94703e);
  // The swirl: a spiral drawn in to the middle.
  const turn = v % 2 ? 1 : -1;
  g.moveTo(r * 0.6, 0);
  for (let i = 1; i <= 40; i++) {
    const t = i / 40;
    const a = turn * t * Math.PI * 4;
    const d = r * 0.6 * (1 - t);
    g.lineTo(Math.cos(a) * d, Math.sin(a) * d);
  }
  g.stroke({ width: 1.5, color: 0x6e4e2a, alpha: 0.7, cap: 'round' });
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.4 * Math.sqrt(rnd());
    g.circle(Math.cos(a) * d, Math.sin(a) * d, r * 0.025).stroke({ width: 1, color: 0xd8b878, alpha: 0.9 });
  }
  g.poly(scaled(edge, 0.5, 0, 0, -r * 0.35, -r * 0.35)).fill({ color: 0xecd29a, alpha: 0.35 });
}

/** Thin ice: only the fracture, over whatever ice is already there — a punched-through spot, jagged
 * cracks running out from it and tapering, side branches, and broken rings joining the cracks. Each
 * crack is dark water with a bright broken edge on the lit side. */
function thinIce(g: Graphics, r: number, rnd: () => number, v: number) {
  // Segments [x, y, x2, y2, width].
  const segs: number[][] = [];
  const crack = (x: number, y: number, a: number, len: number, w: number, branches: number) => {
    const steps = 6;
    const pts: number[][] = [[x, y]];
    for (let k = 1; k <= steps; k++) {
      a += (rnd() - 0.5) * 0.7;
      x += Math.cos(a) * (len / steps) * (0.7 + 0.6 * rnd());
      y += Math.sin(a) * (len / steps) * (0.7 + 0.6 * rnd());
      pts.push([x, y]);
    }
    for (let k = 1; k < pts.length; k++) segs.push([...pts[k - 1], ...pts[k], w * (1 - (k - 1) / steps) + 0.6]);
    for (let b = 0; b < branches; b++) {
      const k = 2 + Math.floor(rnd() * 3);
      const [bx, by] = pts[k];
      crack(bx, by, a + (rnd() < 0.5 ? 1 : -1) * (0.5 + 0.4 * rnd()), len * (0.3 + 0.2 * rnd()), w * 0.5, 0);
    }
    return pts;
  };
  const [cx, cy] = [(rnd() - 0.5) * r * 0.3, (rnd() - 0.5) * r * 0.3];
  const n = 6 + (v % 3);
  const ph = rnd() * Math.PI * 2;
  const radials: number[][][] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.5) / n) * Math.PI * 2;
    radials.push(crack(cx, cy, a, r * (0.5 + 0.35 * rnd()), 3.2, 1 + Math.floor(rnd() * 2)));
  }
  // Rings: from a point on one crack to the matching point on the next, not all the way round.
  for (const k of [2, 4]) {
    for (let i = 0; i < n; i++) {
      if (rnd() < 0.35) continue;
      const [x0, y0] = radials[i][k];
      const [x1, y1] = radials[(i + 1) % n][k];
      const mx = (x0 + x1) / 2 + (rnd() - 0.5) * r * 0.06;
      const my = (y0 + y1) / 2 + (rnd() - 0.5) * r * 0.06;
      segs.push([x0, y0, mx, my, 1.4], [mx, my, x1, y1, 1.2]);
    }
  }
  // Keep everything inside the frame.
  const lim = r * 0.95;
  for (const sg of segs) {
    for (const o of [0, 2]) {
      const d = Math.hypot(sg[o], sg[o + 1]);
      if (d > lim) {
        sg[o] *= lim / d;
        sg[o + 1] *= lim / d;
      }
    }
  }
  for (const [x, y, x2, y2, w] of segs) g.moveTo(x, y).lineTo(x2, y2).stroke({ width: w + 1.5, color: 0x1e4660, alpha: 0.35, cap: 'round' });
  for (const [x, y, x2, y2, w] of segs) g.moveTo(x, y).lineTo(x2, y2).stroke({ width: w, color: 0x1e4660, alpha: 0.9, cap: 'round' });
  for (const [x, y, x2, y2, w] of segs) {
    g.moveTo(x - w * 0.45, y - w * 0.45).lineTo(x2 - w * 0.45, y2 - w * 0.45).stroke({ width: Math.max(0.7, w * 0.35), color: 0xf2faff, alpha: 0.85, cap: 'round' });
  }
  // The punched-through spot: dark water with a broken white rim.
  g.poly(blob(cx, cy, r * 0.06, 7, 0.5, rnd)).fill(0x163a52).stroke({ width: 1.2, color: 0xf2faff, alpha: 0.85 });
}

/** Lava: a crusted basalt rim round molten rock, cooler crust plates floating with glowing seams, a hot
 * bright core and a faint glow round it all. */
function lava(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 13, 0.15, rnd);
  fade(g, edge, 0xff7a1a, 0.12);
  const rim = scaled(edge, 0.9);
  g.poly(rim).fill(0x3a2620).stroke({ width: 2.5, color: INK, join: 'round' });
  g.poly(scaled(rim, 0.94, 0, 0, -r * 0.03, -r * 0.03)).fill(0x5a3a2a);
  const pool = scaled(blob(0, 0, r * 0.95, 12, 0.22, rnd), 0.72);
  g.poly(pool).fill(0xe8561a).stroke({ width: 1.5, color: 0x8a2a10, join: 'round' });
  g.poly(scaled(pool, 0.75, 0, 0, -r * 0.03, -r * 0.03)).fill(0xff7a1a);
  g.poly(blob(-r * 0.05, -r * 0.05, r * 0.3, 9, 0.35, rnd)).fill(0xffb238);
  g.poly(blob(-r * 0.08, -r * 0.08, r * 0.12, 7, 0.4, rnd)).fill(0xfff2c0);
  for (let i = 0; i < 4 + v; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * (0.35 + 0.2 * rnd());
    g.poly(blob(Math.cos(a) * d, Math.sin(a) * d, r * (0.08 + 0.06 * rnd()), 6, 0.4, rnd)).fill(0x3a2018).stroke({ width: 1.5, color: 0xffb238, join: 'round' });
  }
}

/** Steam vent: a cracked mouth among a few sulphur-stained stones, a plume of steam drifting off it. */
function steamVent(g: Graphics, r: number, rnd: () => number, v: number) {
  const ph = rnd() * Math.PI * 2;
  const n = 4 + (v % 2);
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.6) / n) * Math.PI * 2;
    const d = r * (0.48 + 0.15 * rnd());
    stone(g, Math.cos(a) * d, Math.sin(a) * d, r * (0.14 + 0.12 * rnd()), [0x6a6450, 0x9a9070, 0xc8b878, 0xe8dc90], rnd, 6);
  }
  g.poly(blob(0, 0, r * 0.3, 8, 0.35, rnd)).fill(0x1e1a18).stroke({ width: 1.5, color: INK });
  g.poly(blob(r * 0.04, r * 0.04, r * 0.16, 7, 0.3, rnd)).fill(0x3a2a20);
  g.poly(blob(0, 0, r * 0.38, 9, 0.3, rnd)).stroke({ width: 2, color: 0xd8c050, alpha: 0.6 });
  // The plume: puffs growing and fading as they drift off to one side.
  const drift = ph + Math.PI * (0.5 + rnd());
  for (let i = 0; i < 6; i++) {
    const t = i / 5;
    const x = Math.cos(drift) * r * 0.6 * t + (rnd() - 0.5) * r * 0.12;
    const y = Math.sin(drift) * r * 0.6 * t + (rnd() - 0.5) * r * 0.12;
    const s = r * (0.16 + 0.16 * t);
    g.circle(x, y, s).fill({ color: 0xe8ecf0, alpha: 0.5 - t * 0.25 });
    g.circle(x - s * 0.3, y - s * 0.3, s * 0.45).fill({ color: 0xffffff, alpha: 0.5 - t * 0.25 });
  }
}

/** Sinkhole: broken ground falling away into a dark pit — a cracked, crumbling lip fading into the
 * ground, rings of shade going down, roots hanging over the edge. */
function sinkhole(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 13, 0.15, rnd);
  fade(g, edge, 0x6a5a40, 0.3);
  const lip = scaled(edge, 0.82);
  g.poly(lip).fill(0x5a4a36).stroke({ width: 1.8, color: 0x3a2e20, join: 'round' });
  const pit = scaled(blob(0, 0, r * 0.95, 11, 0.2, rnd), 0.62, 0, 0, r * 0.04, r * 0.04);
  g.poly(pit).fill(0x3a2e22);
  g.poly(scaled(pit, 0.7, r * 0.04, r * 0.04, r * 0.03, r * 0.03)).fill(0x241c16);
  g.poly(scaled(pit, 0.4, r * 0.04, r * 0.04, r * 0.05, r * 0.05)).fill(0x120e0c);
  // Roots hanging into the pit from the lit rim.
  for (let i = 0; i < 4; i++) {
    const a = -Math.PI * 0.75 + (rnd() - 0.5) * 2.4;
    const d0 = r * 0.6;
    const d1 = r * 0.3;
    g.moveTo(Math.cos(a) * d0, Math.sin(a) * d0)
      .quadraticCurveTo(Math.cos(a + 0.3) * r * 0.45, Math.sin(a + 0.3) * r * 0.45, Math.cos(a + 0.1) * d1, Math.sin(a + 0.1) * d1)
      .stroke({ width: 1.8, color: 0x7a5a3a, cap: 'round' });
  }
  // Cracks in the ground round the lip, and a crumbled clod or two.
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    g.moveTo(Math.cos(a) * r * 0.62, Math.sin(a) * r * 0.62).lineTo(Math.cos(a + 0.1) * r * 0.85, Math.sin(a + 0.1) * r * 0.85);
  }
  g.stroke({ width: 1.3, color: 0x2a2018, alpha: 0.8, cap: 'round' });
  for (let i = 0; i < 2 + (v % 2); i++) {
    const a = rnd() * Math.PI * 2;
    stone(g, Math.cos(a) * r * 0.5, Math.sin(a) * r * 0.5, r * 0.08, [0x4a3a2a, 0x6a5440, 0x8a7458, 0xa89070], rnd, 6);
  }
}

/** A raised box from above, lit from the NW: the outline, light top and left bevels, dark bottom and right
 * bevels `d` wide, and the face. */
function bevel(g: Graphics, x0: number, y0: number, x1: number, y1: number, d: number, tones: readonly number[], ink = 2.5) {
  g.rect(x0, y0, x1 - x0, y1 - y0).fill(tones[1]).stroke({ width: ink, color: INK, join: 'round' });
  g.poly([x0, y0, x1, y0, x1 - d, y0 + d, x0 + d, y0 + d]).fill(tones[2]);
  g.poly([x0, y0, x0 + d, y0 + d, x0 + d, y1 - d, x0, y1]).fill(shade(tones[2], 0.94));
  g.poly([x1, y0, x1, y1, x1 - d, y1 - d, x1 - d, y0 + d]).fill(tones[0]);
  g.poly([x0, y1, x1, y1, x1 - d, y1 - d, x0 + d, y1 - d]).fill(shade(tones[0], 0.92));
}

/** Barrel from above: a ring of staves, two iron hoops, a planked lid with a bung, lit on the NW. */
function barrel(g: Graphics, x: number, y: number, r: number, rnd: () => number) {
  const wood = [0x5a3a22, 0x7e5432, 0xa87a4c];
  g.circle(x, y, r).fill(wood[0]).stroke({ width: 2, color: INK });
  const n = 14;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    g.moveTo(x + Math.cos(a) * r * 0.66, y + Math.sin(a) * r * 0.66).lineTo(x + Math.cos(a) * r * 0.98, y + Math.sin(a) * r * 0.98);
  }
  g.stroke({ width: 1, color: shade(wood[0], 0.7) });
  g.moveTo(x + Math.cos(Math.PI * 0.8) * r * 0.84, y + Math.sin(Math.PI * 0.8) * r * 0.84)
    .arc(x, y, r * 0.84, Math.PI * 0.8, Math.PI * 1.75)
    .stroke({ width: r * 0.22, color: wood[1], alpha: 0.8 });
  g.circle(x, y, r * 0.9).stroke({ width: 2, color: 0x34322e });
  g.moveTo(x - r * 0.9, y).arc(x, y, r * 0.9, Math.PI, Math.PI * 1.5).stroke({ width: 1, color: 0x9a9a92 });
  g.circle(x, y, r * 0.66).fill(wood[1]).stroke({ width: 1.5, color: 0x34322e });
  for (const f of [-0.33, 0.33]) g.moveTo(x - r * 0.6, y + r * f * 0.66).lineTo(x + r * 0.6, y + r * f * 0.66);
  g.stroke({ width: 1, color: wood[0], alpha: 0.8 });
  g.circle(x - r * 0.2, y - r * 0.2, r * 0.3).fill({ color: wood[2], alpha: 0.7 });
  const b = rnd() * Math.PI * 2;
  g.circle(x + Math.cos(b) * r * 0.35, y + Math.sin(b) * r * 0.35, r * 0.08).fill(0x3a2616);
}

/** Crate from above: a frame of boards round three planks, nails in the corners, a cross brace on some,
 * lit on the NW. */
function crate(g: Graphics, x: number, y: number, s: number, rot: number, braced: boolean) {
  const c = Math.cos(rot);
  const si = Math.sin(rot);
  const pt = (u: number, v: number) => [x + u * c - v * si, y + u * si + v * c];
  const quad = (u0: number, v0: number, u1: number, v1: number) => [...pt(u0, v0), ...pt(u1, v0), ...pt(u1, v1), ...pt(u0, v1)];
  const wood = [0x6e4e2e, 0x9a7448, 0xc09a68];
  g.poly(quad(-s, -s, s, s)).fill(wood[0]).stroke({ width: 2.2, color: INK, join: 'round' });
  const f = s * 0.18;
  for (let i = 0; i < 3; i++) {
    const v0 = -s + f + ((2 * s - 2 * f) * i) / 3;
    g.poly(quad(-s + f, v0 + 0.8, s - f, v0 + (2 * s - 2 * f) / 3 - 0.8)).fill(i === 0 ? shade(wood[1], 1.08) : wood[1]);
  }
  g.poly(quad(-s, -s, s, -s + f)).fill(wood[2]);
  g.poly(quad(-s, -s, -s + f, s)).fill(shade(wood[2], 0.95));
  if (braced) {
    const [a, b] = [pt(-s + f, s - f), pt(s - f, -s + f)];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: f * 0.9, color: wood[0] });
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: f * 0.5, color: wood[2], alpha: 0.6 });
  }
  for (const [u, v] of [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
  ]) {
    const [nx, ny] = pt(u * (s - f / 2), v * (s - f / 2));
    g.circle(nx, ny, 1.1).fill(0x2a2a28);
  }
}

/** Barrels: one, two or three standing together. */
function barrels(g: Graphics, r: number, rnd: () => number, v: number) {
  const n = 1 + (v % 3);
  if (n === 1) return barrel(g, 0, 0, r * 0.82, rnd);
  const ph = rnd() * Math.PI * 2;
  const s = n === 2 ? r * 0.48 : r * 0.44;
  const d = n === 2 ? r * 0.47 : r * 0.5;
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    barrel(g, Math.cos(a) * d, Math.sin(a) * d, s, rnd);
  }
}

/** Crates: one, two side by side, or a small one stacked on a big one. */
function crates(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v === 0) crate(g, 0, 0, r * 0.62, (rnd() - 0.5) * 0.4, true);
  else if (v === 1) {
    crate(g, -r * 0.45, r * 0.05, r * 0.42, (rnd() - 0.5) * 0.4, false);
    crate(g, r * 0.45, -r * 0.05, r * 0.42, (rnd() - 0.5) * 0.4, true);
  } else if (v === 2) {
    crate(g, 0, 0, r * 0.65, (rnd() - 0.5) * 0.3, false);
    crate(g, -r * 0.1, -r * 0.1, r * 0.38, (rnd() - 0.5) * 0.6, true);
  } else {
    crate(g, r * 0.2, r * 0.15, r * 0.5, (rnd() - 0.5) * 0.4, true);
    // A tied sack leaning on it.
    g.poly(blob(-r * 0.5, -r * 0.35, r * 0.3, 9, 0.2, rnd)).fill(0xc8b080).stroke({ width: 2, color: INK });
    g.circle(-r * 0.56, -r * 0.42, r * 0.12).fill(0xe0cca0);
    g.circle(-r * 0.5, -r * 0.35, r * 0.06).fill(0x8a6a3a);
  }
}

/** Cart, its shafts toward +x: a box bed with thick rails (lit tops), the floor sunk inside them — shadow
 * along the inner north and west walls, the inner south and east walls catching the light — two wheels
 * on an axle, and a load casting its own shadow on the floor. */
function cart(g: Graphics, r: number, rnd: () => number, v: number) {
  const wood = [0x5e4024, 0x8a6238, 0xb08a5a];
  const [bx0, bx1, by] = [-r * 0.78, r * 0.42, r * 0.5];
  const rail = r * 0.11;
  // Shafts.
  for (const side of [-1, 1]) {
    g.moveTo(bx1, side * by * 0.5).lineTo(r * 0.96, side * by * 0.38).stroke({ width: 5.5, color: INK, cap: 'round' });
    g.moveTo(bx1, side * by * 0.5).lineTo(r * 0.96, side * by * 0.38).stroke({ width: 3, color: wood[1], cap: 'round' });
  }
  // Wheels, seen edge-on, sticking out past the bed; the axle under it.
  for (const side of [-1, 1]) {
    const wy = side * (by + r * 0.05);
    g.roundRect(-r * 0.44, wy - r * 0.08, r * 0.66, r * 0.16, r * 0.06).fill(0x3a2a1a).stroke({ width: 2, color: INK });
    g.roundRect(-r * 0.4, wy - r * 0.04 - (side < 0 ? r * 0.01 : 0), r * 0.58, r * 0.05, r * 0.02).fill(0x6a5038);
    g.circle(-r * 0.11, wy, r * 0.065).fill(0x6a6a64).stroke({ width: 1, color: INK });
  }
  // The box: outer edge, the floor sunk inside, then the rails' tops.
  g.rect(bx0, -by, bx1 - bx0, by * 2).fill(wood[0]).stroke({ width: 2.5, color: INK, join: 'round' });
  const [fx0, fx1, fy0, fy1] = [bx0 + rail, bx1 - rail, -by + rail, by - rail];
  g.rect(fx0, fy0, fx1 - fx0, fy1 - fy0).fill(shade(wood[1], 0.82));
  for (let i = 1; i < 6; i++) {
    const x = fx0 + ((fx1 - fx0) * i) / 6;
    g.moveTo(x, fy0).lineTo(x, fy1);
  }
  g.stroke({ width: 1.2, color: wood[0], alpha: 0.7 });
  // Inner walls: the south and east faces lit, a shadow cast on the floor along the north and west.
  const d = rail * 0.9;
  g.poly([fx0, fy1, fx1, fy1, fx1 - d * 0.2, fy1 - d, fx0 + d * 0.2, fy1 - d]).fill(shade(wood[1], 1.12));
  g.poly([fx1, fy0, fx1, fy1, fx1 - d, fy1 - d * 0.2, fx1 - d, fy0 + d * 0.2]).fill(shade(wood[1], 1.05));
  g.poly([fx0, fy0, fx1, fy0, fx1, fy0 + d * 1.4, fx0 + d * 1.4, fy0 + d * 1.4, fx0 + d * 1.4, fy1, fx0, fy1]).fill({ color: 0x1a120a, alpha: 0.3 });
  // Rail tops: lit along the north and west.
  g.rect(bx0, -by, bx1 - bx0, rail).fill(wood[2]);
  g.rect(bx0, -by, rail, by * 2).fill(shade(wood[2], 0.95));
  g.rect(bx0, by - rail, bx1 - bx0, rail).fill(wood[1]);
  g.rect(bx1 - rail, -by, rail, by * 2).fill(shade(wood[1], 0.92));
  g.rect(bx0, -by, bx1 - bx0, by * 2).stroke({ width: 2.5, color: INK, join: 'round' });
  g.rect(fx0, fy0, fx1 - fx0, fy1 - fy0).stroke({ width: 1.2, color: shade(wood[0], 0.8) });
  // The load, each piece with a shadow falling SE on the floor.
  const cx = (fx0 + fx1) / 2;
  const drop = (pts: number[]) => g.poly(pts.map((p, i) => p + (i % 2 ? 3 : 3))).fill({ color: 0x1a120a, alpha: 0.35 });
  if (v === 1) {
    for (let i = 0; i < 4; i++) {
      const x = cx + (i % 2 ? 0.2 : -0.2) * r;
      const y = (i < 2 ? -0.15 : 0.15) * r;
      const sack = blob(x, y, r * 0.17, 9, 0.2, rnd);
      drop(sack);
      g.poly(sack).fill(0xc8b080).stroke({ width: 1.8, color: INK });
      g.circle(x - r * 0.05, y - r * 0.05, r * 0.07).fill(0xe0cca0);
    }
  } else if (v === 2) {
    for (const [x, y] of [
      [cx - r * 0.22, -r * 0.13],
      [cx + r * 0.2, r * 0.1],
    ]) {
      g.circle(x + 3, y + 3, r * 0.18).fill({ color: 0x1a120a, alpha: 0.35 });
      barrel(g, x, y, r * 0.18, rnd);
    }
    g.rect(cx - r * 0.32 + 3, r * 0.06 + 3, r * 0.26, r * 0.26).fill({ color: 0x1a120a, alpha: 0.35 });
    crate(g, cx - r * 0.19, r * 0.19, r * 0.13, 0.15, false);
  } else if (v === 3) {
    const tips = boughs(22, rnd);
    g.circle(cx + 4, 4, r * 0.34).fill({ color: 0x1a120a, alpha: 0.3 });
    whorl(g, cx, 0, r * 0.36, tips, 0.88);
    g.fill(0xb08a3a).stroke({ width: 1.5, color: 0x6a4e1e, join: 'round' });
    whorl(g, cx - r * 0.05, -r * 0.05, r * 0.24, tips, 0.88);
    g.fill(0xd9b04f);
  }
}

/** Market stall, its front toward +y: a striped awning sloping from a ridge (the half facing the light
 * paler), a scalloped valance along the front, corner poles, goods in baskets below the front edge. */
function marketStall(g: Graphics, r: number, rnd: () => number, v: number) {
  const [stripe, cloth] = [
    [0xa83a2e, 0xece0bc],
    [0x2e5a8a, 0xece0bc],
    [0x3a7a3a, 0xece0bc],
    [0x8a6a1e, 0xf2e6c4],
  ][v];
  const w = r * 0.78;
  const h = r * 0.5;
  // Goods: baskets of produce along the front.
  const fruit = [0xc84a2a, 0xd8b040, 0x6a9a3a, 0x8a3a6a];
  for (let i = 0; i < 3; i++) {
    const x = -w * 0.62 + i * w * 0.62;
    const y = h + r * 0.2;
    g.poly(oval(x, y, r * 0.2, r * 0.13, 0)).fill(0x8a6238).stroke({ width: 1.8, color: INK });
    for (let k = 0; k < 5; k++) g.circle(x + (rnd() - 0.5) * r * 0.24, y + (rnd() - 0.5) * r * 0.12, r * 0.05).fill(fruit[(i + v) % 4]).stroke({ width: 0.8, color: INK });
  }
  for (const [px, py] of [
    [-w, -h],
    [w, -h],
    [-w, h],
    [w, h],
  ]) {
    g.circle(px, py, r * 0.05).fill(0x5a3e22).stroke({ width: 1, color: INK });
  }
  // The awning: stripes across, the back half (toward the light) paler.
  g.rect(-w, -h, w * 2, h * 2).fill(stripe).stroke({ width: 2.5, color: INK, join: 'round' });
  const stripes = 7;
  for (let i = 0; i < stripes; i++) if (i % 2) g.rect(-w + (2 * w * i) / stripes, -h, (2 * w) / stripes, h * 2).fill(cloth);
  g.rect(-w, -h, w * 2, h).fill({ color: 0xffffff, alpha: 0.16 });
  g.rect(-w, 0, w * 2, h).fill({ color: 0x2a1a10, alpha: 0.12 });
  g.moveTo(-w, 0).lineTo(w, 0).stroke({ width: 1.5, color: shade(stripe, 0.6) });
  // Valance: scallops hanging off the front edge.
  for (let i = 0; i < stripes; i++) {
    const x0 = -w + (2 * w * i) / stripes;
    const sw = (2 * w) / stripes;
    g.moveTo(x0, h).arc(x0 + sw / 2, h, sw / 2, 0, Math.PI).closePath().fill(i % 2 ? cloth : stripe).stroke({ width: 1.5, color: INK });
  }
}

/** Well: a ring of fitted stones round dark water with a glint, a winch on posts with a bucket (v0, v1),
 * or a little shingled roof over it (v2, v3). */
function well(g: Graphics, r: number, rnd: () => number, v: number) {
  const stone = [0x6e675c, 0x9a9182, 0xc4baa6];
  g.circle(0, 0, r * 0.95).fill(stone[0]).stroke({ width: 2.5, color: INK });
  const n = 11;
  for (let i = 0; i < n; i++) {
    const a0 = (i / n) * Math.PI * 2 + 0.05;
    const a1 = ((i + 1) / n) * Math.PI * 2 - 0.05;
    const pts: number[] = [];
    for (let k = 0; k <= 4; k++) {
      const a = a0 + ((a1 - a0) * k) / 4;
      pts.push(Math.cos(a) * r * 0.9, Math.sin(a) * r * 0.9);
    }
    for (let k = 4; k >= 0; k--) {
      const a = a0 + ((a1 - a0) * k) / 4;
      pts.push(Math.cos(a) * r * 0.62, Math.sin(a) * r * 0.62);
    }
    const mid = (a0 + a1) / 2;
    g.poly(pts).fill(shade(stone[1], 0.92 + 0.08 * rnd() + 0.12 * lit(mid)));
  }
  g.circle(0, 0, r * 0.58).fill(0x1a2830).stroke({ width: 1.8, color: INK });
  g.circle(r * 0.05, r * 0.05, r * 0.45).fill(0x22343e);
  g.poly(oval(-r * 0.18, -r * 0.16, r * 0.14, r * 0.06, -0.6)).fill({ color: 0x6a9aaa, alpha: 0.7 });
  if (v < 2) {
    for (const side of [-1, 1]) g.rect(side * r * 0.86 - r * 0.07, -r * 0.07, r * 0.14, r * 0.14).fill(0x6a4a2a).stroke({ width: 1.5, color: INK });
    g.rect(-r * 0.86, -r * 0.05, r * 1.72, r * 0.1).fill(0x8a6238).stroke({ width: 1.5, color: INK });
    g.circle(r * 0.18, r * 0.2, r * 0.13).fill(0x7e5432).stroke({ width: 1.5, color: INK });
    g.circle(r * 0.18, r * 0.2, r * 0.07).fill(0x2a3a40);
    g.moveTo(r * 0.18, 0).lineTo(r * 0.18, r * 0.08).stroke({ width: 1, color: 0xc8b080 });
  } else {
    // A little gable roof across the well: two planes, the one facing the light paler, shingle rows.
    const rw = r * 0.7;
    const rh = r * 0.52;
    const roof = v === 2 ? [0x7a3a2a, 0xa85a3e] : [0x4a5058, 0x6e7680];
    g.rect(-rw, -rh, rw * 2, rh * 2).fill(roof[0]).stroke({ width: 2.2, color: INK });
    g.rect(-rw, -rh, rw * 2, rh).fill(roof[1]);
    for (let k = 1; k < 4; k++) {
      const y = -rh + (rh * k) / 2;
      if (Math.abs(y) < 1) continue;
      g.moveTo(-rw, y).lineTo(rw, y);
    }
    g.stroke({ width: 1, color: shade(roof[0], 0.7) });
    g.moveTo(-rw, 0).lineTo(rw, 0).stroke({ width: 2, color: shade(roof[0], 0.6) });
  }
}

/** Haystack: a rounded dome of straw — a softly scalloped edge, two lighter tiers toward the light,
 * straws combed down from the top — a few loose straws round the foot, a pitchfork stuck in one. */
function haystack(g: Graphics, r: number, rnd: () => number, v: number) {
  const straw = [
    [0x9a7430, 0xc8a040, 0xe6c866],
    [0x8e7032, 0xbc9a44, 0xdcc06a],
    [0xa07a2e, 0xd0a83e, 0xecd06a],
    [0x947836, 0xc0a24a, 0xe0c870],
  ][v];
  for (let i = 0; i < 8; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * (0.84 + 0.1 * rnd());
    const b = a + (rnd() - 0.5) * 1.5;
    g.moveTo(Math.cos(a) * d, Math.sin(a) * d).lineTo(Math.cos(a) * d + Math.cos(b) * r * 0.12, Math.sin(a) * d + Math.sin(b) * r * 0.12);
  }
  g.stroke({ width: 1.5, color: straw[1], cap: 'round' });
  // The dome: many small soft bumps round the edge, so it reads as straw, not as a star.
  const tips = boughs(26, rnd);
  whorl(g, 0, 0, r * 0.88, tips, 0.9);
  g.fill(straw[0]).stroke({ width: 2.2, color: INK, join: 'round' });
  whorl(g, -r * 0.06, -r * 0.06, r * 0.72, tips, 0.9);
  g.fill(straw[1]);
  whorl(g, -r * 0.16, -r * 0.16, r * 0.38, boughs(14, rnd), 0.88);
  g.fill(straw[2]);
  // Straws combed down from the top in every direction.
  const top = [-r * 0.12, -r * 0.12];
  for (let i = 0; i < 26; i++) {
    const a = (i / 26) * Math.PI * 2 + rnd() * 0.2;
    const d0 = r * (0.2 + 0.25 * rnd());
    const d1 = d0 + r * (0.15 + 0.15 * rnd());
    g.moveTo(top[0] + Math.cos(a) * d0, top[1] + Math.sin(a) * d0).lineTo(top[0] + Math.cos(a) * d1, top[1] + Math.sin(a) * d1);
  }
  g.stroke({ width: 1.2, color: shade(straw[0], 0.85), alpha: 0.7, cap: 'round' });
  if (v === 3) {
    const a = rnd() * Math.PI * 2;
    const [x0, y0] = [Math.cos(a) * r * 0.1, Math.sin(a) * r * 0.1];
    const [x1, y1] = [Math.cos(a) * r * 0.75, Math.sin(a) * r * 0.75];
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: 4.5, color: INK, cap: 'round' });
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: 2.5, color: 0x8a6238, cap: 'round' });
  }
}

/** Headstone, seen from a high angle rather than straight down (the one exception, so it reads as a grave):
 * an upright stone with an arched top, its top edge showing as a lit strip behind the arch, its front face
 * carved (a cross or lines of an epitaph), on a base whose top is visible, grass at the foot; moss on one,
 * flowers laid at one, and a standing cross for the last. Headstones are never turned on the map. */
function headstone(g: Graphics, r: number, rnd: () => number, v: number) {
  const stone = [
    [0x6e6a62, 0x9a968c, 0xc4c0b4],
    [0x625e58, 0x8a877e, 0xb4b0a6],
    [0x726c60, 0xa8a296, 0xd0cabc],
    [0x5e5c56, 0x7e7c74, 0xa8a69c],
  ][v];
  const ink = { width: 2, color: INK, join: 'round' as const };
  // The base: a low block, its top face lit, its front face in shade.
  const [bx, by, bw, bt, bf] = [-r * 0.78, r * 0.52, r * 1.56, r * 0.16, r * 0.22];
  g.rect(bx, by, bw, bt + bf).fill(stone[0]).stroke(ink);
  g.rect(bx, by, bw, bt).fill(stone[2]);
  g.moveTo(bx, by + bt).lineTo(bx + bw, by + bt).stroke({ width: 1.2, color: INK });
  // Grass along the foot of the base.
  for (const x of [-r * 0.82, r * 0.78, -r * 0.1]) {
    const tips = boughs(7, rnd);
    whorl(g, x, by + bt + bf, r * 0.2, tips, 0.4);
    g.fill(0x5a7e36).stroke({ width: 1, color: 0x34461e, join: 'round' });
  }
  const [x0, x1, top, foot] = [-r * 0.58, r * 0.58, -r * 0.92, by + bt * 0.5];
  if (v === 3) {
    // A standing cross: the top edges of its arms and head lit, the front faces carved with a groove.
    const arm = r * 0.2;
    const shape = (dy: number) => [
      -arm, top + dy, arm, top + dy, arm, top + r * 0.42 + dy, r * 0.62, top + r * 0.42 + dy, r * 0.62, top + r * 0.82 + dy, arm, top + r * 0.82 + dy,
      arm, foot, -arm, foot, -arm, top + r * 0.82 + dy, -r * 0.62, top + r * 0.82 + dy, -r * 0.62, top + r * 0.42 + dy, -arm, top + r * 0.42 + dy,
    ];
    g.poly(shape(-r * 0.12)).fill(stone[2]).stroke(ink);
    g.poly(shape(0)).fill(stone[1]).stroke(ink);
    g.moveTo(0, top + r * 0.12).lineTo(0, foot - r * 0.1).moveTo(-r * 0.48, top + r * 0.62).lineTo(r * 0.48, top + r * 0.62);
    g.stroke({ width: 2, color: stone[0], cap: 'round' });
    return;
  }
  // The stone: its top edge (seen from above, behind the arch), then the front face.
  const face = (dy: number) => {
    const w = (x1 - x0) / 2;
    const pts: number[] = [x0, foot];
    for (let i = 0; i <= 12; i++) {
      const a = Math.PI + (i / 12) * Math.PI;
      pts.push(Math.cos(a) * w, top + w + Math.sin(a) * w * (v === 2 ? 0.55 : 1) + dy);
    }
    pts.push(x1, foot);
    return pts;
  };
  g.poly(face(-r * 0.14)).fill(stone[2]).stroke(ink);
  g.poly(face(0)).fill(stone[1]).stroke(ink);
  // The face lit from the upper left, darker toward the lower right.
  g.poly([x1 - r * 0.22, top + r * 0.6, x1 - 1, top + r * 0.6, x1 - 1, foot - 1, x0 + r * 0.3, foot - 1]).fill({ color: stone[0], alpha: 0.45 });
  if (v === 0) {
    // A carved cross: a groove with a lit lower edge.
    g.moveTo(0, top + r * 0.25).lineTo(0, top + r * 0.95).moveTo(-r * 0.22, top + r * 0.5).lineTo(r * 0.22, top + r * 0.5);
    g.stroke({ width: 3, color: shade(stone[0], 0.75), cap: 'round' });
    g.moveTo(1, top + r * 0.27).lineTo(1, top + r * 0.97).moveTo(-r * 0.2, top + r * 0.52 + 1).lineTo(r * 0.24, top + r * 0.52 + 1);
    g.stroke({ width: 1, color: stone[2], alpha: 0.8, cap: 'round' });
  } else {
    // An epitaph: short carved lines, the first a little bolder.
    for (let i = 0; i < 4; i++) {
      const y = top + r * (0.48 + i * 0.2);
      const hw = r * (i === 0 ? 0.3 : 0.18 + 0.12 * rnd());
      g.moveTo(-hw, y).lineTo(hw, y).stroke({ width: i === 0 ? 2.4 : 1.6, color: shade(stone[0], 0.75), cap: 'round' });
    }
  }
  if (v === 1) moss(g, x0 + r * 0.25, top + r * 0.4, r * 0.15, rnd);
  if (v === 2) {
    for (let i = 0; i < 3; i++) {
      const x = -r * 0.25 + i * r * 0.22;
      g.circle(x, by + r * 0.06, r * 0.09).fill([0xe0533d, 0xf2c94c, 0xf6efe0][i]).stroke({ width: 1, color: INK });
    }
  }
}

/** Statue on a plinth: a stepped plinth bevelled toward the light, a figure on it seen from above —
 * head, shoulders and an arm raised with a sword (marble, bronze gone green, or weathered stone), or a
 * winged angel. */
function statue(g: Graphics, r: number, rnd: () => number, v: number) {
  const plinth = [0x6e675c, 0x9a9182, 0xc4baa6];
  bevel(g, -r * 0.95, -r * 0.95, r * 0.95, r * 0.95, r * 0.12, plinth);
  bevel(g, -r * 0.66, -r * 0.66, r * 0.66, r * 0.66, r * 0.1, plinth.map((c) => shade(c, 1.06)), 2);
  const body = [
    [0xb8b4ac, 0xdcd8d0, 0xf4f2ec],
    [0x3e6e60, 0x5e9a84, 0x8ac4a8],
    [0x7e7a70, 0xa8a296, 0xc8c2b4],
    [0xb0aca4, 0xd4d0c8, 0xeeece6],
  ][v];
  const a = -Math.PI / 2 + (rnd() - 0.5) * 0.6;
  const c = Math.cos(a);
  const s = Math.sin(a);
  const p = (u: number, w: number): [number, number] => [c * u - s * w, s * u + c * w];
  if (v === 3) {
    // Wings swept back from the shoulders.
    for (const side of [1, -1]) {
      const pts = [...p(-r * 0.05, side * r * 0.2), ...p(-r * 0.5, side * r * 0.58), ...p(-r * 0.6, side * r * 0.25), ...p(-r * 0.32, side * r * 0.06)];
      g.poly(pts).fill(body[1]).stroke({ width: 1.8, color: INK, join: 'round' });
      for (const k of [0.3, 0.42]) g.moveTo(...p(-r * 0.1, side * r * 0.15)).lineTo(...p(-r * k - r * 0.1, side * r * 0.35));
      g.stroke({ width: 1, color: body[0] });
    }
  }
  // Shoulders, the raised arm holding a sword (blade, crossguard), the head.
  g.poly(oval(...p(-r * 0.04, 0), r * 0.42, r * 0.24, a + Math.PI / 2)).fill(body[1]).stroke({ width: 2, color: INK });
  g.poly(oval(...p(r * 0.0, -r * 0.08), r * 0.24, r * 0.12, a + Math.PI / 2)).fill(body[2]);
  if (v !== 3) {
    const [sx, sy] = p(0, r * 0.32);
    const [hx, hy] = p(r * 0.3, r * 0.36);
    g.moveTo(sx, sy).lineTo(hx, hy).stroke({ width: r * 0.13 + 3, color: INK, cap: 'round' });
    g.moveTo(sx, sy).lineTo(hx, hy).stroke({ width: r * 0.13, color: body[1], cap: 'round' });
    const [tx, ty] = p(r * 0.82, r * 0.36);
    g.moveTo(hx, hy).lineTo(tx, ty).stroke({ width: 5.5, color: INK, cap: 'round' });
    g.moveTo(hx, hy).lineTo(tx, ty).stroke({ width: 3, color: body[2], cap: 'round' });
    g.moveTo(...p(r * 0.36, r * 0.24)).lineTo(...p(r * 0.36, r * 0.48)).stroke({ width: 5, color: INK, cap: 'round' });
    g.moveTo(...p(r * 0.36, r * 0.24)).lineTo(...p(r * 0.36, r * 0.48)).stroke({ width: 2.5, color: body[0], cap: 'round' });
  }
  const [lx, ly] = p(r * 0.06, 0);
  g.circle(lx, ly, r * 0.19).fill(body[1]).stroke({ width: 2, color: INK });
  g.circle(lx - r * 0.05, ly - r * 0.05, r * 0.09).fill(body[2]);
  if (v === 2) moss(g, r * 0.45, -r * 0.5, r * 0.14, rnd);
}

/** A-frame tent, its door at +x: two canvas planes sagging between the poles (the north one lit), seams
 * down them, a ridge pole with its ends showing, guy ropes curving out to pegs, the doorway with its flaps
 * tied back; a patch sewn on some. */
function tent(g: Graphics, r: number, rnd: () => number, v: number) {
  const canvas = [0xd8cba8, 0xbfa36a, 0x9a5442][v];
  const [L, W] = [r * 0.78, r * 0.6];
  // Guy ropes and pegs first, under the canvas.
  for (const [sx, sy] of [
    [-1, -1],
    [1, -1],
    [-1, 1],
    [1, 1],
  ]) {
    const [x0, y0] = [sx * L * 0.85, sy * W * 0.95];
    const [x1, y1] = [sx * L * 1.12, sy * W * 1.45];
    g.moveTo(x0, y0).quadraticCurveTo(x0 + sx * L * 0.05, (y0 + y1) / 2 + sy * 3, x1, y1).stroke({ width: 1.3, color: 0x8a7a5a });
    g.circle(x1, y1, 2.3).fill(0x5a4028).stroke({ width: 1, color: INK });
  }
  // The canvas: the eaves sag between the corners.
  const sag = W * 0.1;
  const plane = (sy: number) => {
    g.moveTo(-L, 0).lineTo(L, 0).lineTo(L, sy * W).quadraticCurveTo(0, sy * (W - sag), -L, sy * W).closePath();
  };
  plane(-1);
  g.fill(shade(canvas, 1.08));
  plane(1);
  g.fill(shade(canvas, 0.74));
  // Seams down each plane, and the shade the ridge throws on the lit side's lower edge.
  for (let i = 1; i < 4; i++) {
    const x = -L + (2 * L * i) / 4 + (rnd() - 0.5) * 4;
    g.moveTo(x, -1).lineTo(x + (rnd() - 0.5) * 3, -W + sag * 0.6).stroke({ width: 1.2, color: shade(canvas, 0.85), alpha: 0.9 });
    g.moveTo(x, 1).lineTo(x + (rnd() - 0.5) * 3, W - sag * 0.6).stroke({ width: 1.2, color: shade(canvas, 0.58), alpha: 0.9 });
  }
  g.moveTo(-L, -W).quadraticCurveTo(0, -W + sag, L, -W).lineTo(L, -W + W * 0.14).quadraticCurveTo(0, -W + sag + W * 0.14, -L, -W + W * 0.14).closePath().fill({ color: 0xffffff, alpha: 0.12 });
  if (v === 1) {
    const [px, py] = [-L * 0.35, W * 0.45];
    g.rect(px, py, L * 0.25, W * 0.25).fill(shade(0x8a6a3a, 0.9)).stroke({ width: 1, color: shade(canvas, 0.45) });
  }
  // Outline, the ridge pole (its ends past the canvas), the doorway at +x.
  plane(-1);
  g.stroke({ width: 2.5, color: INK, join: 'round' });
  plane(1);
  g.stroke({ width: 2.5, color: INK, join: 'round' });
  g.moveTo(-L - 4, 0).lineTo(L + 4, 0).stroke({ width: 3.5, color: INK, cap: 'round' });
  g.moveTo(-L - 4, 0).lineTo(L + 4, 0).stroke({ width: 1.8, color: 0x8a6238, cap: 'round' });
  g.poly([L, -W * 0.5, L, W * 0.5, L - W * 0.4, 0]).fill(0x2a2018).stroke({ width: 1.8, color: INK, join: 'round' });
  g.poly([L, -W * 0.5, L + r * 0.12, -W * 0.72, L - r * 0.02, -W * 0.18]).fill(shade(canvas, 1.15)).stroke({ width: 1.5, color: INK, join: 'round' });
  g.poly([L, W * 0.5, L + r * 0.12, W * 0.72, L - r * 0.02, W * 0.18]).fill(shade(canvas, 0.8)).stroke({ width: 1.5, color: INK, join: 'round' });
}

/** Round pavilion (a camp leader's): panels in two colours sagging between the ribs (lit from the NW),
 * a scalloped valance round the rim, a gilt finial with a pennant, the door at +x with its flap tied
 * back. */
function pavilion(g: Graphics, r: number, rnd: () => number) {
  const [a, b] = [0xe2d6b4, 0x8a2e26];
  const R = r * 0.86;
  const n = 12;
  for (let i = 0; i < n; i++) {
    const t0 = (i / n) * Math.PI * 2;
    const t1 = ((i + 1) / n) * Math.PI * 2;
    const tm = (t0 + t1) / 2;
    // Each panel sags a little between its ribs.
    g.moveTo(0, 0)
      .lineTo(Math.cos(t0) * R, Math.sin(t0) * R)
      .quadraticCurveTo(Math.cos(tm) * R * 0.94, Math.sin(tm) * R * 0.94, Math.cos(t1) * R, Math.sin(t1) * R)
      .closePath()
      .fill(shade(i % 2 ? b : a, 0.9 + 0.22 * lit(tm)));
    g.moveTo(Math.cos(tm) * R * 0.25, Math.sin(tm) * R * 0.25).lineTo(Math.cos(tm) * R * 0.85, Math.sin(tm) * R * 0.85).stroke({ width: 1, color: shade(i % 2 ? b : a, 0.75), alpha: 0.6 });
  }
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2;
    g.moveTo(0, 0).lineTo(Math.cos(t) * R, Math.sin(t) * R);
  }
  g.stroke({ width: 1.4, color: INK, alpha: 0.6 });
  // Scalloped valance round the rim.
  for (let i = 0; i < n * 2; i++) {
    const t = ((i + 0.5) / (n * 2)) * Math.PI * 2;
    g.circle(Math.cos(t) * R, Math.sin(t) * R, R * 0.08).fill(shade(i % 2 ? b : a, 0.85 + 0.2 * lit(t))).stroke({ width: 1, color: INK });
  }
  // The door: a dark slit with its flap tied back.
  g.poly([R, -R * 0.16, R, R * 0.16, R * 0.72, 0]).fill(0x2a2018).stroke({ width: 2, color: INK, join: 'round' });
  g.poly([R, -R * 0.16, R * 1.1, -R * 0.36, R * 0.84, -R * 0.1]).fill(a).stroke({ width: 1.5, color: INK, join: 'round' });
  g.circle(0, 0, R * 0.11).fill(0xd9b36a).stroke({ width: 2, color: INK });
  g.circle(-R * 0.03, -R * 0.03, R * 0.045).fill(0xfff1c0);
  const f = 0.3 + rnd() * 0.3;
  g.poly([0, 0, R * 0.5, -R * 0.12 - f * 8, R * 0.42, R * 0.06]).fill(b).stroke({ width: 1.5, color: INK, join: 'round' });
}

/** Tent: an A-frame in three canvases, or a round pavilion. */
function tents(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v === 3) return pavilion(g, r, rnd);
  tent(g, r, rnd, v);
}

/** Flames from above: curved tongues of fire all swept the same way, in layers from dark orange out to a
 * white-hot heart, nudged toward the light. */
function flames(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  const swirl = rnd() < 0.5 ? 1 : -1;
  const layers: [number, number, number][] = [
    [1, 0xd8461a, 6],
    [0.72, 0xf28a1e, 5],
    [0.48, 0xffc23a, 4],
  ];
  layers.forEach(([k, color, n], l) => {
    const cx = x - l * s * 0.05;
    const cy = y - l * s * 0.05;
    const ph = rnd() * Math.PI * 2;
    for (let i = 0; i < n; i++) {
      const a = ph + (i / n) * Math.PI * 2 + (rnd() - 0.5) * 0.4;
      const len = s * k * (0.75 + 0.25 * rnd());
      const w = s * k * 0.45;
      const c = Math.cos(a);
      const sn = Math.sin(a);
      // A tongue: wide at the base, bowed sideways, hooking at the tip.
      const tip = [cx + c * len - sn * len * 0.35 * swirl, cy + sn * len + c * len * 0.35 * swirl];
      const m = [cx + c * len * 0.5 - sn * len * 0.1 * swirl, cy + sn * len * 0.5 + c * len * 0.1 * swirl];
      g.moveTo(cx - sn * w, cy + c * w)
        .quadraticCurveTo(m[0] - sn * w * 0.9, m[1] + c * w * 0.9, tip[0], tip[1])
        .quadraticCurveTo(m[0] + sn * w * 0.5, m[1] - c * w * 0.5, cx + sn * w, cy - c * w)
        .closePath()
        .fill(color);
      if (l === 0) g.stroke({ width: 1.2, color: 0x7a2a0e, join: 'round' });
    }
  });
  g.circle(x - s * 0.12, y - s * 0.12, s * 0.2).fill(0xfff2c0);
}

/** Campfire: a warm glow, a ring of faceted stones round the ash, barked logs crossed in it (cut ends
 * showing), and flames — or, on the last, glowing embers with a wisp of smoke. */
function campfire(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v !== 3) for (let i = 0; i < 5; i++) g.circle(0, 0, r * (0.98 - i * 0.08)).fill({ color: 0xf59e0b, alpha: 0.05 });
  g.circle(0, 0, r * 0.62).fill(0x2a2420).stroke({ width: 2, color: INK });
  g.circle(r * 0.06, r * 0.06, r * 0.46).fill({ color: 0x6a645c, alpha: 0.6 });
  const n = 9;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rnd() * 0.3;
    stone(g, Math.cos(a) * r * 0.7, Math.sin(a) * r * 0.7, r * (0.15 + 0.05 * rnd()), STONE[i % 2], rnd, 7);
  }
  const bark = BARK[0];
  for (const a of [0.4, 2.5, 4.4].map((t) => t + (rnd() - 0.5) * 0.3)) {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const [x0, y0, x1, y1] = [-c * r * 0.48, -s * r * 0.48, c * r * 0.48, s * r * 0.48];
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: r * 0.16 + 3, color: INK, cap: 'round' });
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: r * 0.16, color: bark[1], cap: 'round' });
    g.moveTo(x0 - s * r * 0.03, y0 + c * r * 0.03).lineTo(x1 - s * r * 0.03, y1 + c * r * 0.03).stroke({ width: r * 0.05, color: bark[2], alpha: 0.6, cap: 'round' });
    g.circle(x1, y1, r * 0.07).fill(0xc89a68).stroke({ width: 1, color: INK });
  }
  if (v === 3) {
    for (let i = 0; i < 12; i++) {
      const a = rnd() * Math.PI * 2;
      const d = r * 0.32 * Math.sqrt(rnd());
      g.circle(Math.cos(a) * d, Math.sin(a) * d, 1.8 + rnd() * 2).fill(rnd() < 0.5 ? 0xc2410c : 0xf97316);
    }
    for (let i = 0; i < 3; i++) g.circle(-r * 0.12 - i * r * 0.1, -r * 0.12 - i * r * 0.12, r * (0.12 + i * 0.05)).fill({ color: 0xd8d4cc, alpha: 0.32 - i * 0.08 });
  } else {
    flames(g, 0, 0, r * 0.52, rnd);
  }
}

/** Firewood: split logs stacked side by side, each a little uneven, the cut ends (+x) pale with rings,
 * some split to show the pale wood; a chopping block with an axe bitten into it on some. */
function firewood(g: Graphics, r: number, rnd: () => number, v: number) {
  const n = 4;
  const lw = (r * 1.05) / n;
  const shift = v % 2 ? -r * 0.2 : 0;
  for (let i = 0; i < n; i++) {
    const bark = BARK[(i + v) % 4];
    const y = -r * 0.52 + (i + 0.5) * lw;
    const len = r * (1.05 + 0.15 * rnd());
    const x = shift + (rnd() - 0.5) * r * 0.12;
    const [x0, x1] = [x - len / 2, x + len / 2];
    const h = lw / 2;
    const wob = () => (rnd() - 0.5) * h * 0.25;
    const body = [x0 + h * 0.5, y - h + wob(), x0 + len * 0.5, y - h + wob(), x1, y - h, x1, y + h, x0 + len * 0.5, y + h + wob(), x0 + h * 0.5, y + h + wob(), x0, y + wob()];
    g.poly(body).fill(bark[0]).stroke({ width: 2, color: INK, join: 'round' });
    g.rect(x0 + h * 0.5, y - h * 0.7, len - h, h * 0.9).fill(bark[1]);
    g.rect(x0 + h * 0.6, y - h * 0.65, len - h * 1.4, h * 0.3).fill({ color: bark[2], alpha: 0.5 });
    if (i % 2 === 1) g.rect(x0 + h * 0.6, y + h * 0.1, len - h * 1.6, h * 0.55).fill(0xc89a68);
    g.poly(oval(x1, y, h * 0.42, h * 0.95, 0)).fill(0xc89a68).stroke({ width: 1.5, color: INK });
    g.poly(oval(x1 - 0.5, y, h * 0.22, h * 0.5, 0)).stroke({ width: 1, color: shade(bark[1], 0.9) });
  }
  if (v % 2) {
    const [sx, sy] = [r * 0.6, r * 0.48];
    const sr = r * 0.3;
    g.poly(blob(sx, sy, sr, 9, 0.12, rnd)).fill(BARK[0][0]).stroke({ width: 2, color: INK });
    g.circle(sx, sy, sr * 0.78).fill(0xc89a68);
    g.circle(sx - sr * 0.15, sy - sr * 0.15, sr * 0.35).fill({ color: 0xe0b47a, alpha: 0.8 });
    g.circle(sx, sy, sr * 0.45).stroke({ width: 1, color: 0x8a6440 });
    // The axe: a handle reaching up-right from the blade bitten into the block.
    const [hx, hy] = [sx + sr * 1.05, sy - sr * 1.1];
    g.moveTo(sx, sy).lineTo(hx, hy).stroke({ width: 5, color: INK, cap: 'round' });
    g.moveTo(sx, sy).lineTo(hx, hy).stroke({ width: 2.8, color: 0x9a7448, cap: 'round' });
    g.poly([sx - sr * 0.42, sy + sr * 0.05, sx + sr * 0.1, sy - sr * 0.3, sx + sr * 0.2, sy + sr * 0.1, sx - sr * 0.2, sy + sr * 0.35]).fill(0x8a8f96).stroke({ width: 1.5, color: INK, join: 'round' });
    g.moveTo(sx - sr * 0.38, sy + sr * 0.08).lineTo(sx - sr * 0.18, sy + sr * 0.3).stroke({ width: 1.2, color: 0xd8dce0 });
  }
}

/** Bedroll: a blanket laid along x with a stripe woven across it, the top corner turned back to show
 * the lining, a crease or two, and the rolled head end at +x tied with a strap. */
function bedroll(g: Graphics, r: number, rnd: () => number, v: number) {
  const wool = [0x8a3a2e, 0x4a6a3a, 0x7a5a3a, 0x4a5a6e][v];
  const [L, W] = [r * 0.88, r * 0.42];
  g.roundRect(-L, -W, 2 * L, 2 * W, 5).fill(wool).stroke({ width: 2, color: INK });
  g.roundRect(-L + 3, -W + 2, 2 * L - 6, W * 0.45, 4).fill({ color: shade(wool, 1.3), alpha: 0.55 });
  // Stripes woven across.
  for (const x of [-L * 0.62, -L * 0.52]) g.rect(x, -W + 1, L * 0.05, 2 * W - 2).fill(shade(wool, 1.5));
  // Creases.
  for (let i = 0; i < 2; i++) {
    const x = -L * 0.2 + i * L * 0.35 + (rnd() - 0.5) * 6;
    g.moveTo(x, -W + 2).quadraticCurveTo(x + (rnd() - 0.5) * 8, 0, x - 3, W - 2).stroke({ width: 1.4, color: shade(wool, 0.62) });
  }
  // The corner turned back: a triangle of the pale lining.
  g.poly([-L + 2, -W + 2, -L + W * 1.1, -W + 2, -L + 2, -W + W * 1.1]).fill(0xe8dcc0).stroke({ width: 1.5, color: INK, join: 'round' });
  // The rolled head end: a cylinder seen from above with its spiral end.
  g.roundRect(L * 0.42, -W * 1.08, L * 0.58, W * 2.16, W * 0.5).fill(0xd8cfb8).stroke({ width: 2, color: INK });
  g.roundRect(L * 0.46, -W * 0.98, L * 0.5, W * 0.6, W * 0.3).fill({ color: 0xf2ead6, alpha: 0.8 });
  g.moveTo(L * 0.68, -W * 1.08).lineTo(L * 0.68, W * 1.08).stroke({ width: 3, color: 0x6a4a2a });
  g.moveTo(L * 0.96, -W * 0.5).quadraticCurveTo(L * 0.86, 0, L * 0.96, W * 0.5).stroke({ width: 1.2, color: 0xa89878 });
}

const DRAW: Record<number, Draw> = {
  1: (g, r, rnd, v) => canopy(g, r, rnd, [[0x2f4a22, 0x4d6e2d, 0x7a9a3c], [0x34502a, 0x557a32, 0x88a846], [0x3a4a20, 0x62702c, 0x9aa24a], [0x2c4426, 0x46683a, 0x6f9450]][v], 7),
  2: conifer,
  3: palm,
  4: (g, r, rnd, v) => canopy(g, r, rnd, [[0x1e4a24, 0x2e6a2e, 0x4f8f3a], [0x1c4230, 0x2b6240, 0x4a8850], [0x224a1e, 0x357028, 0x5a9a38], [0x1a3e22, 0x2a5a2e, 0x468040]][v], 9),
  5: (g, r, rnd) => {
    canopy(g, r, rnd, [0x5a6a26, 0x7d8a34, 0xa9ad54], 6);
  },
  6: deadTree,
  7: (g, r, rnd) => {
    canopy(g, r, rnd, [0x3e5e44, 0x5f8058, 0x8aaa78], 8);
    strokes(g, r * 0.9, rnd, 14, r * 0.35, [0x9ab888, 0x6a8a5a], 1.5);
  },
  8: (g, r, rnd, v) => canopy(g, r, rnd, [[0x3f5f2a, 0x5f7f35, 0x86a048], [0x44602e, 0x66843a, 0x8ea650], [0x4a5a2a, 0x6e7a34, 0x98a04a], [0x385a30, 0x587c40, 0x7ea058]][v], 5),
  9: (g, r, rnd) => {
    canopy(g, r, rnd, [0x2e4a22, 0x46642c, 0x6a8a3c], 11);
    strokes(g, r, rnd, 10, r * 0.25, [0x3a2e20], 1.5);
  },
  10: fallenLog,
  11: stump,
  12: boulderStone,
  // Town props.
  33: barrels,
  34: crates,
  35: cart,
  36: marketStall,
  37: well,
  39: headstone,
  40: (g, r, rnd, v) => {
    // Ruined wall segment: a run of fitted stones along x, broken ends, moss.
    const len = r * 1.75;
    const t = r * 0.95;
    const stones = 4 + (v % 3);
    for (let i = 0; i < stones; i++) {
      const x0 = -len / 2 + (len * i) / stones;
      const sw = len / stones - 1.5;
      const jitter = (rnd() - 0.5) * 3;
      const tone = [0x8e877a, 0x9a9384, 0x807a6e][Math.floor(rnd() * 3)];
      g.rect(x0, -t / 2 + jitter, sw, t).fill(tone).stroke({ width: 2, color: INK });
      g.rect(x0 + 2, -t / 2 + jitter + 2, sw - 4, t * 0.25).fill({ color: 0xb8b0a0, alpha: 0.6 });
    }
    for (let i = 0; i < 3; i++) g.circle((rnd() - 0.5) * len, t / 2 + 2 + rnd() * 4, 2 + rnd() * 3).fill(0x7e776a).stroke({ width: 1, color: INK });
    if (rnd() < 0.6) g.circle((rnd() - 0.5) * len * 0.6, (rnd() - 0.5) * t * 0.5, t * 0.3).fill({ color: 0x5a7a3a, alpha: 0.6 });
  },
  41: statue,
  42: tents,
  43: campfire,
  44: firewood,
  45: bedroll,
  38: haystack,
  13: smallRocks,
  14: rockPile,
  15: reeds,
  16: cactus,
  17: tallGrass,
  18: wildflowers,
  19: mushroomRing,
  20: bones,
  21: snowdrift,
  22: iceSpire,
  23: basaltPillar,
  24: obsidian,
  25: brambles,
  26: bog,
  27: quicksand,
  28: thinIce,
  29: lava,
  30: steamVent,
  31: scree,
  32: sinkhole,
};

export function frameRadiusPx(info: KindInfo): number {
  // Radius in atlas px at scale 1 plus room for outlines.
  return Math.ceil(info.radius * PX + 6);
}

export function buildAtlas(renderer: Renderer, catalog: KindInfo[]): Atlas {
  const root = new Container();
  const placements: { id: number; v: number; x: number; y: number; size: number }[] = [];
  let x = 0;
  let y = 0;
  let rowH = 0;
  // Frames start on the PAD grid with at least PAD empty pixels after them.
  const step = (size: number) => Math.ceil(size / PAD) * PAD + PAD;
  const place = (size: number) => {
    if (x + step(size) > ATLAS_W) {
      x = 0;
      y += rowH;
      rowH = 0;
    }
    const at = { x, y };
    x += step(size);
    rowH = Math.max(rowH, step(size));
    return at;
  };
  for (const info of catalog) {
    const rpx = frameRadiusPx(info);
    const size = rpx * 2;
    for (let v = 0; v < VARIANTS; v++) {
      const at = place(size);
      const g = new Graphics();
      g.position.set(at.x + rpx, at.y + rpx);
      (DRAW[info.id] ?? DRAW[12])(g, info.radius * PX, rng(info.id * 131 + v * 17 + 7), v);
      root.addChild(g);
      placements.push({ id: info.id, v, x: at.x, y: at.y, size });
    }
  }
  // Shadow blob: stacked translucent discs approximate a soft falloff.
  const shadowAt = place(128);
  const sg = new Graphics();
  sg.position.set(shadowAt.x + 64, shadowAt.y + 64);
  for (let i = 0; i < 12; i++) sg.circle(0, 0, 62 - i * 4).fill({ color: 0x000000, alpha: 0.09 });
  root.addChild(sg);

  const height = y + rowH;
  const rt = renderer.generateTexture({
    target: root,
    frame: new Rectangle(0, 0, ATLAS_W, height),
    resolution: 1,
    textureSourceOptions: { autoGenerateMipmaps: true, mipLevelCount: MIP_LEVELS },
  });
  root.destroy({ children: true });
  const frames: Texture[][] = [];
  for (const p of placements) {
    (frames[p.id] ??= [])[p.v] = new Texture({ source: rt.source, frame: new Rectangle(p.x, p.y, p.size, p.size) });
  }
  const shadow = new Texture({ source: rt.source, frame: new Rectangle(shadowAt.x, shadowAt.y, 128, 128) });
  return { frames, shadow };
}
