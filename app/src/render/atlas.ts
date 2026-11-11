// Procedural painterly sprite atlas for battlemap objects (top-down, light from the NW):
// layered fills with a darker ink outline and NW highlights, in the spirit of hand-painted
// battlemap assets. Drawn once into one mipmapped texture (frames padded apart so no mip level
// blends two of them) so objects render as instanced particles.
// Frames are addressed by kind id and variant, so generated art can replace them later.
import { Container, Graphics, Rectangle, Texture, type Renderer } from 'pixi.js';
import type { KindInfo } from '../gen/client';
import { drawHorse } from './furniture';
import { ballista, shotStones } from './fortProps';

/** Atlas resolution: pixels per 5-ft square. */
export const PX = 64;
/** Looks per kind. Must divide 256: `battlePrep` packs the turned flag above an object's variant byte. */
export const VARIANTS = 4;
/** Kinds with more looks than that (by id; also dividing 256): the market stall, four awnings by four goods. */
const LOOKS: Record<number, number> = { 36: 16 };
/** How many looks a kind has: an object's variant picks `variant % looks(kind)`. */
export const looks = (id: number): number => LOOKS[id] ?? VARIANTS;
/** Atlas width: wide enough that the height stays within 4096 too (many phones' largest texture). */
const ATLAS_W = 4096;
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

export function rng(seed: number): () => number {
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

export function shade(color: number, f: number): number {
  const r = Math.min(255, Math.max(0, ((color >> 16) & 255) * f));
  const g = Math.min(255, Math.max(0, ((color >> 8) & 255) * f));
  const b = Math.min(255, Math.max(0, (color & 255) * f));
  return (Math.round(r) << 16) | (Math.round(g) << 8) | Math.round(b);
}

/** Lumpy blob outline: n points around radius r with jitter. */
export function blob(cx: number, cy: number, r: number, n: number, jitter: number, rnd: () => number): number[] {
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
export function oval(cx: number, cy: number, rx: number, ry: number, rot: number, n = 16): number[] {
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
export function stone(g: Graphics, x: number, y: number, s: number, tones: readonly number[], rnd: () => number, n = 8) {
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

/** Atlas pixels per foot. */
const FT = PX / 5;
/** Worked wood (barrels, carts, stalls, posts) and iron: [dark, mid, light, sunlit]. */
const WOOD = [0x5c3b22, 0x8a5a32, 0xb98552, 0xe0b47a];
const IRON = [0x34363c, 0x4a4e56, 0x6e7278, 0x9aa0a6];

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

/** A stall's goods along its front (+y) edge, below the awning's valance, by its look: baskets of produce
 * (v0), fish laid out on a wet board with a tub of eels (v1), bolts of dyed cloth with a length hung over
 * the counter (v2), jugs, bowls and a stack of plates (v3). */
function stallGoods(g: Graphics, r: number, w: number, h: number, rnd: () => number, v: number) {
  if (v === 0) {
    const fruit = [0xc84a2a, 0xd8b040, 0x6a9a3a, 0x8a3a6a];
    for (let i = 0; i < 3; i++) {
      const x = -w * 0.62 + i * w * 0.62;
      const y = h + r * 0.2;
      g.poly(oval(x, y, r * 0.2, r * 0.13, 0)).fill(0x8a6238).stroke({ width: 1.8, color: INK });
      for (let k = 0; k < 5; k++) g.circle(x + (rnd() - 0.5) * r * 0.24, y + (rnd() - 0.5) * r * 0.12, r * 0.05).fill(fruit[(i + v) % 4]).stroke({ width: 0.8, color: INK });
    }
    return;
  }
  // The counter: a trestle board along the front, lit along its north edge.
  const [cx0, cx1, cy0, cy1] = [-w * 0.96, w * 0.96, h - r * 0.04, h + r * 0.42];
  g.poly([cx0, cy0, cx1, cy0 + 1, cx1 - 1, cy1, cx0 + 1, cy1 - 1]).fill(v === 1 ? 0x6e5a44 : WOOD[1]).stroke({ width: 2, color: INK, join: 'round' });
  g.moveTo(cx0 + 2, cy0 + 2.5).lineTo(cx1 - 2, cy0 + 3).stroke({ width: 2, color: v === 1 ? 0x8a7458 : WOOD[2], alpha: 0.8 });
  g.moveTo(cx0 + 2, (cy0 + cy1) / 2 + 2).lineTo(cx1 - 2, (cy0 + cy1) / 2 + 2.5).stroke({ width: 1, color: WOOD[0], alpha: 0.6 });
  const y = h + r * 0.25;
  if (v === 1) {
    // Fish side by side on the wet board, heads all one way, a tub of eels at the end.
    const n = 7;
    for (let i = 0; i < n; i++) {
      const x = -w * 0.86 + (i + 0.5) * ((w * 1.4) / n);
      const a = Math.PI / 2 + 0.35 + (rnd() - 0.5) * 0.25;
      const [c, s] = [Math.cos(a), Math.sin(a)];
      const len = r * (0.13 + 0.03 * rnd());
      const at = (u: number, k: number): [number, number] => [x + c * u - s * k, y + s * u + c * k];
      const body = [...at(-len, 0), ...at(-len * 0.4, len * 0.34), ...at(len * 0.5, len * 0.3), ...at(len, 0), ...at(len * 0.5, -len * 0.3), ...at(-len * 0.4, -len * 0.34)];
      const tone = [0x9aaab2, 0xb0b6a4, 0x8e9ea8][i % 3];
      g.poly([...at(-len * 0.9, 0), ...at(-len * 1.45, len * 0.32), ...at(-len * 1.35, -len * 0.3)]).fill(shade(tone, 0.8)).stroke({ width: 1, color: INK, join: 'round' });
      g.poly(body).fill(tone).stroke({ width: 1.2, color: INK, join: 'round' });
      g.poly([...at(-len * 0.5, -len * 0.08), ...at(len * 0.6, -len * 0.06), ...at(len * 0.85, 0), ...at(len * 0.5, -len * 0.26), ...at(-len * 0.4, -len * 0.3)]).fill(0x4e6270);
      g.moveTo(...at(-len * 0.3, len * 0.16)).lineTo(...at(len * 0.5, len * 0.12)).stroke({ width: 1, color: 0xe6f0ee, alpha: 0.8 });
      g.circle(...at(len * 0.66, -len * 0.06), 1).fill(INK);
    }
    // The tub: a half-barrel of staves bound with an iron hoop, two eels writhing in its water.
    const [tx, ty, tr] = [w * 0.76, y - r * 0.03, r * 0.19];
    g.circle(tx, ty, tr).fill(WOOD[0]).stroke({ width: 2, color: INK });
    for (let k = 0; k < 10; k++) {
      const a = (k / 10) * Math.PI * 2;
      g.moveTo(tx + Math.cos(a) * tr * 0.74, ty + Math.sin(a) * tr * 0.74).lineTo(tx + Math.cos(a) * tr * 0.98, ty + Math.sin(a) * tr * 0.98);
    }
    g.stroke({ width: 1, color: shade(WOOD[0], 0.7) });
    g.circle(tx, ty, tr * 0.87).stroke({ width: 1.6, color: IRON[1] });
    g.circle(tx, ty, tr * 0.72).fill(0x2e5a66).stroke({ width: 1.2, color: INK });
    for (const [k, flip] of [
      [0, 1],
      [1, -1],
    ]) {
      const [ex, ey] = [tx + (k ? 0.12 : -0.14) * tr, ty + (k ? 0.18 : -0.2) * tr];
      const q = (u: number, v2: number): [number, number] => [ex + u * tr, ey + v2 * tr * flip];
      for (const [width, color] of [
        [3.6, 0x1e2416],
        [2.2, 0x5a5e34],
      ] as const) {
        g.moveTo(...q(-0.5, 0.05)).quadraticCurveTo(...q(-0.25, -0.3), ...q(0, 0)).quadraticCurveTo(...q(0.25, 0.3), ...q(0.45, -0.02));
        g.stroke({ width, color, cap: 'round' });
      }
      g.circle(...q(0.46, -0.03), 1.9).fill(0x5a5e34).stroke({ width: 0.8, color: 0x1e2416 });
    }
    g.circle(tx - tr * 0.36, ty - tr * 0.4, tr * 0.1).fill({ color: 0xe6f4ec, alpha: 0.8 });
  } else if (v === 2) {
    // A length of cloth hung over the counter's front, folds down it.
    const drape = 0xb5523a;
    const [dx0, dx1] = [-w * 0.3, w * 0.18];
    g.poly([dx0, cy0 + 4, dx1, cy0 + 4, dx1 + 3, cy1 + r * 0.1, (dx0 + dx1) / 2, cy1 + r * 0.06, dx0 - 2, cy1 + r * 0.1]).fill(drape).stroke({ width: 1.6, color: INK, join: 'round' });
    for (const f of [0.25, 0.5, 0.75]) {
      const x = dx0 + (dx1 - dx0) * f;
      g.moveTo(x, cy0 + 6).lineTo(x + (f - 0.5) * 3, cy1 + r * 0.06).stroke({ width: 1.2, color: shade(drape, 0.7) });
    }
    g.poly([dx0, cy0 + 4, dx1, cy0 + 4, dx1, cy0 + 8, dx0, cy0 + 8]).fill({ color: 0xfff1cf, alpha: 0.3 });
    // Bolts of dyed cloth laid across the counter, each a roll with its end turned to the light.
    const dyes = [0xd1a24c, 0x3f7f7a, 0x7a4a6a, 0x4f72a8, 0xecdcb4, 0x6a8a3a];
    const bolts: [number, number][] = [
      [-w * 0.7, y - r * 0.02],
      [-w * 0.62, y + r * 0.12],
      [w * 0.48, y - r * 0.03],
      [w * 0.66, y + r * 0.11],
    ];
    bolts.forEach(([x, by], i) => {
      const dye = dyes[(i * 2 + Math.floor(rnd() * 2)) % dyes.length];
      const [bw, bh] = [r * 0.2, r * 0.065];
      g.roundRect(x - bw, by - bh, bw * 2, bh * 2, bh * 0.9).fill(dye).stroke({ width: 1.5, color: INK });
      g.roundRect(x - bw * 0.9, by - bh * 0.7, bw * 1.8, bh * 0.55, bh * 0.3).fill({ color: shade(dye, 1.3), alpha: 0.7 });
      g.moveTo(x + bw * 0.62, by - bh * 0.9).lineTo(x + bw * 0.62, by + bh * 0.9).stroke({ width: 1, color: shade(dye, 0.62) });
    });
  } else {
    // Pots: a jug or two, bowls (their dark insides), a stack of plates, a glazed pot.
    const clay = [0x8a4a2a, 0xb5623a, 0xd8885a];
    const pot = (x: number, py: number, s: number, tones: readonly number[], mouth: number) => {
      g.circle(x, py, s).fill(tones[1]).stroke({ width: 1.6, color: INK });
      g.circle(x - s * 0.22, py - s * 0.22, s * 0.55).fill({ color: tones[2], alpha: 0.75 });
      g.circle(x, py, s * mouth).fill(shade(tones[0], 0.6)).stroke({ width: 1.2, color: tones[0] });
    };
    pot(-w * 0.76, y, r * 0.13, clay, 0.38);
    g.poly([-w * 0.76 + r * 0.11, y - r * 0.05, -w * 0.76 + r * 0.2, y - r * 0.02, -w * 0.76 + r * 0.2, y + r * 0.04, -w * 0.76 + r * 0.11, y + r * 0.05]).fill(clay[1]).stroke({ width: 1.2, color: INK, join: 'round' });
    for (const x of [-w * 0.4, -w * 0.12]) {
      g.circle(x, y, r * 0.12).fill(clay[2]).stroke({ width: 1.6, color: INK });
      g.circle(x + 1, y + 1, r * 0.085).fill(clay[0]);
      g.circle(x + 2, y + 2, r * 0.05).fill({ color: shade(clay[0], 0.8), alpha: 0.8 });
    }
    // Plates stacked a little off true.
    for (let k = 0; k < 3; k++) g.circle(w * 0.2 - k * 1.2, y - k * 1.2, r * 0.13).fill(k === 2 ? 0xe8d8b8 : 0xc8b494).stroke({ width: 1.3, color: INK });
    g.circle(w * 0.2 - 2.4, y - 2.4, r * 0.08).stroke({ width: 1, color: 0xa89474 });
    pot(w * 0.52, y - r * 0.02, r * 0.12, [0x2e5a56, 0x3f7f7a, 0x6aa6a0], 0.42);
    pot(w * 0.8, y + r * 0.02, r * 0.1, clay, 0.4);
  }
}

/** Market stall, its front toward +y: a striped awning sloping from a ridge (the half facing the light
 * paler), a scalloped valance along the front, corner poles, its goods below the front edge (`stallGoods`).
 * Sixteen looks (`LOOKS`): the awning `v & 3`, and the goods rolled apart from it, so that the first four
 * keep their own and every awning comes with every kind of goods. */
function marketStall(g: Graphics, r: number, rnd: () => number, v: number) {
  const goods = ((v >> 2) + v) & 3;
  const [stripe, cloth] = [
    [0xa83a2e, 0xece0bc],
    [0x2e5a8a, 0xece0bc],
    [0x3a7a3a, 0xece0bc],
    [0x8a6a1e, 0xf2e6c4],
  ][v & 3];
  const w = r * 0.78;
  const h = r * 0.5;
  stallGoods(g, r, w, h, rnd, goods);
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
  stripedCanopy(g, R, 12, a, b);
  // The door: a dark slit with its flap tied back.
  g.poly([R, -R * 0.16, R, R * 0.16, R * 0.72, 0]).fill(0x2a2018).stroke({ width: 2, color: INK, join: 'round' });
  g.poly([R, -R * 0.16, R * 1.1, -R * 0.36, R * 0.84, -R * 0.1]).fill(a).stroke({ width: 1.5, color: INK, join: 'round' });
  g.circle(0, 0, R * 0.11).fill(0xd9b36a).stroke({ width: 2, color: INK });
  g.circle(-R * 0.03, -R * 0.03, R * 0.045).fill(0xfff1c0);
  const f = 0.3 + rnd() * 0.3;
  g.poly([0, 0, R * 0.5, -R * 0.12 - f * 8, R * 0.42, R * 0.06]).fill(b).stroke({ width: 1.5, color: INK, join: 'round' });
}

/** A canvas canopy from above round (0, 0), radius `R`: `n` panels in two colours sagging between the ribs
 * (lit from the NW), a seam down each, inked ribs, a scalloped valance round the rim. On a canopy raised on
 * posts (a bandstand's: `drop` > 0) the panels' corners start at angle `ph`, so its eaves follow the stage's
 * polygon, and the valance is one band of darker canvas hanging `drop` px out past them, scalloped. */
function stripedCanopy(g: Graphics, R: number, n: number, a: number, b: number, drop = 0, ph = 0) {
  const at = (i: number, k: number): [number, number] => [Math.cos(ph + (i / n) * Math.PI * 2) * k, Math.sin(ph + (i / n) * Math.PI * 2) * k];
  if (drop) {
    // The valance: from corner to corner, three scallops to a side, hanging out past the eaves.
    const c0 = at(0, R + drop * 0.9);
    g.moveTo(...c0);
    for (let i = 0; i < n; i++) {
      const [p, q] = [at(i, R + drop * 0.9), at(i + 1, R + drop * 0.9)];
      const mid = ph + ((i + 0.5) / n) * Math.PI * 2;
      for (let k = 0; k < 3; k++) {
        const [u0, u1] = [k / 3, (k + 1) / 3];
        const [x1, y1] = [p[0] + (q[0] - p[0]) * u1, p[1] + (q[1] - p[1]) * u1];
        const [mx, my] = [p[0] + (q[0] - p[0]) * (u0 + u1) * 0.5, p[1] + (q[1] - p[1]) * (u0 + u1) * 0.5];
        g.quadraticCurveTo(mx + Math.cos(mid) * drop * 1.1, my + Math.sin(mid) * drop * 1.1, x1, y1);
      }
    }
    g.closePath().fill(shade(b, 0.62)).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  // (A canopy with few panels sags further between them, so its eaves curve in rather than out.)
  const sag = drop ? 0.9 : 0.94;
  for (let i = 0; i < n; i++) {
    const t0 = ph + (i / n) * Math.PI * 2;
    const t1 = ph + ((i + 1) / n) * Math.PI * 2;
    const tm = (t0 + t1) / 2;
    // Each panel sags a little between its ribs.
    g.moveTo(0, 0)
      .lineTo(Math.cos(t0) * R, Math.sin(t0) * R)
      .quadraticCurveTo(Math.cos(tm) * R * sag, Math.sin(tm) * R * sag, Math.cos(t1) * R, Math.sin(t1) * R)
      .closePath()
      .fill(shade(i % 2 ? b : a, 0.9 + 0.22 * lit(tm)));
    g.moveTo(Math.cos(tm) * R * 0.25, Math.sin(tm) * R * 0.25).lineTo(Math.cos(tm) * R * 0.85, Math.sin(tm) * R * 0.85).stroke({ width: 1, color: shade(i % 2 ? b : a, 0.75), alpha: 0.6 });
  }
  for (let i = 0; i < n; i++) {
    const t = ph + (i / n) * Math.PI * 2;
    g.moveTo(0, 0).lineTo(Math.cos(t) * R, Math.sin(t) * R);
  }
  g.stroke({ width: 1.4, color: INK, alpha: 0.6 });
  if (drop) {
    // The eaves, inked.
    for (let i = 0; i < n; i++) {
      const [t0, t1] = [ph + (i / n) * Math.PI * 2, ph + ((i + 1) / n) * Math.PI * 2];
      const tm = (t0 + t1) / 2;
      g.moveTo(Math.cos(t0) * R, Math.sin(t0) * R).quadraticCurveTo(Math.cos(tm) * R * sag, Math.sin(tm) * R * sag, Math.cos(t1) * R, Math.sin(t1) * R);
    }
    g.stroke({ width: 2, color: INK, join: 'round' });
    return;
  }
  // Scalloped valance round the rim.
  for (let i = 0; i < n * 2; i++) {
    const t = ((i + 0.5) / (n * 2)) * Math.PI * 2;
    g.circle(Math.cos(t) * R, Math.sin(t) * R, R * 0.08).fill(shade(i % 2 ? b : a, 0.85 + 0.2 * lit(t))).stroke({ width: 1, color: INK });
  }
}

/** Tent: an A-frame in three canvases, or a round pavilion. */
function tents(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v === 3) return pavilion(g, r, rnd);
  tent(g, r, rnd, v);
}

/** Flames from above: curved tongues of fire all swept the same way, in layers from dark orange out to a
 * white-hot heart, nudged toward the light. */
export function flames(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
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

/** A quad's corners as one point list, for `poly`. */
function quad4(a: [number, number], b: [number, number], c: [number, number], d: [number, number]): number[] {
  return [...a, ...b, ...c, ...d];
}

/** Where an upright thing drawn from a high angle stands in its frame: its foot, px below the centre, by kind
 * id (its drop shadow falls from there: `BattlemapLayer`). Things seen from straight above stand at the centre. */
export const UPRIGHT_FOOT: Record<number, number> = { 47: 1.7 * FT, 48: 1.85 * FT, 49: 1.1 * FT, 50: 2.15 * FT, 57: 1.3 * FT, 60: 2.3 * FT, 101: 2.2 * FT };

/** The high-angle view of upright things (as the headstone; the style guide's `hk` 0.5, `dk` 0.87): a ground
 * point `x` across and `d` deep (toward the viewer) and `h` up, in feet, to frame pixels, the foot at (0, `foot`). */
function upright(foot: number) {
  return (x: number, d: number, h: number): [number, number] => [x * FT, foot + (d * 0.87 - h * 0.5) * FT];
}

/** A few blades of grass at (x, y), as at a headstone's foot. */
function footGrass(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  whorl(g, x, y, s, boughs(7, rnd), 0.4);
  g.fill(0x5a7e36).stroke({ width: 1, color: 0x34461e, join: 'round' });
}

/** Water in a trough or basin (its inside, a rounded box): fresh water, the rim's shadow along its north and
 * west sides, two ripples and one glint. */
function troughWater(g: Graphics, x0: number, y0: number, x1: number, y1: number, round: number) {
  g.roundRect(x0, y0, x1 - x0, y1 - y0, round).fill(0x3a7f8a).stroke({ width: 1.5, color: shade(0x255a70, 0.8) });
  g.roundRect(x0, y0, x1 - x0, Math.min(4, (y1 - y0) * 0.3), round).fill({ color: 0x1e4a5c, alpha: 0.7 });
  g.roundRect(x0, y0, Math.min(4, (x1 - x0) * 0.2), y1 - y0, round).fill({ color: 0x1e4a5c, alpha: 0.5 });
  const [mx, my] = [(x0 + x1) / 2 + (x1 - x0) * 0.1, (y0 + y1) / 2 + 1];
  for (const [dx, dy, k] of [
    [0, 0, 1],
    [(x1 - x0) * 0.16, (y1 - y0) * 0.18, 0.7],
  ]) {
    const rw = (x1 - x0) * 0.09 * k;
    g.moveTo(mx + dx - rw, my + dy).quadraticCurveTo(mx + dx, my + dy - 2.5, mx + dx + rw, my + dy).stroke({ width: 1.2, color: 0x6ab0a8, alpha: 0.85, cap: 'round' });
  }
  g.poly(oval(x0 + (x1 - x0) * 0.26, y0 + (y1 - y0) * 0.42, (x1 - x0) * 0.08, (y1 - y0) * 0.1, -0.2, 10)).fill({ color: 0xe6f4ec, alpha: 0.75 });
}

/** Water trough, along x: a planked timber trough with iron bands (v0), a stone one with a chipped rim and
 * moss on its north side (v1), a hollowed log (v2), or a stone trough fed by a pump at its +x end (v3). */
function waterTrough(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const W = 1.05 * FT;
  const [x0, x1] = v === 3 ? [-3.6 * FT, 1.6 * FT] : [-3 * FT, 3 * FT];
  if (v === 2) {
    // A log split along its length and hollowed out: lumpy bark a little thicker toward +x, ridged along its
    // length and lit along the top, the pale split face round the water, a cut end with rings at each end.
    const bark = BARK[0];
    const [e0, e1] = [x0 + 0.35 * FT, x1 - 0.35 * FT];
    const half = (t: number) => W * (0.9 + 0.18 * t);
    const n = 9;
    const top: number[] = [];
    const bot: [number, number][] = [];
    for (let i = 0; i <= n; i++) {
      const t = i / n;
      const x = e0 + (e1 - e0) * t;
      const ends = i === 0 || i === n;
      top.push(x, -half(t) + (ends ? 0 : (rnd() - 0.5) * 2.6));
      bot.push([x, half(t) + (ends ? 0 : (rnd() - 0.5) * 2.6)]);
    }
    g.poly([...top, ...bot.reverse().flat()]).fill(bark[1]).stroke({ width: 2.5, color: INK, join: 'round' });
    for (let i = 0; i < 12; i++) {
      const x = e0 + 0.3 * FT + rnd() * (e1 - e0 - 0.9 * FT);
      const side = i % 2 ? 1 : -1;
      const yy = side * W * (0.72 + 0.16 * rnd());
      g.moveTo(x, yy).lineTo(x + 7 + rnd() * 9, yy + (rnd() - 0.5) * 1.5).stroke({ width: 1.8, color: shade(bark[0], 0.7), cap: 'round' });
    }
    g.moveTo(e0 + 4, -half(0) + 3).lineTo(e1 - 4, -half(1) + 3).stroke({ width: 2, color: bark[2], alpha: 0.8, cap: 'round' });
    g.roundRect(e0 + 3, -W * 0.6, e1 - e0 - 6, W * 1.2, W * 0.5).fill(0xc89a68).stroke({ width: 1.2, color: shade(bark[0], 0.8) });
    troughWater(g, e0 + 7, -W * 0.42, e1 - 7, W * 0.44, W * 0.36);
    endGrain(g, e0, 0, 0.3 * FT, half(0) * 0.96, bark);
    endGrain(g, e1, 0, 0.3 * FT, half(1) * 0.96, bark);
    return;
  }
  const tones = v === 0 ? WOOD : STONE[v === 1 ? 0 : 2];
  const rim = v === 0 ? 0.32 * FT : 0.45 * FT;
  // The box, its corners a little out of true.
  const j = () => (rnd() - 0.5) * 1.6;
  const box = [x0 + j(), -W + j(), x1 + j(), -W + j(), x1 + j(), W + j(), x0 + j(), W + j()];
  g.poly(box).fill(tones[1]).stroke({ width: 2.5, color: INK, join: 'round' });
  troughWater(g, x0 + rim, -W + rim, x1 - rim, W - rim, 2);
  // Rim tops: the north and west lit, the south and east in shade.
  g.poly([x0 + 1, -W + 1, x1 - 1, -W + 1, x1 - rim, -W + rim, x0 + rim, -W + rim]).fill(tones[2]);
  g.poly([x0 + 1, -W + 1, x0 + rim, -W + rim, x0 + rim, W - rim, x0 + 1, W - 1]).fill(shade(tones[2], 0.94));
  g.poly([x0 + 1, W - 1, x1 - 1, W - 1, x1 - rim, W - rim, x0 + rim, W - rim]).fill(tones[0]);
  g.poly([x1 - 1, -W + 1, x1 - 1, W - 1, x1 - rim, W - rim, x1 - rim, -W + rim]).fill(shade(tones[0], 0.92));
  g.poly(box).stroke({ width: 2.5, color: INK, join: 'round' });
  g.rect(x0 + rim, -W + rim, x1 - x0 - rim * 2, (W - rim) * 2).stroke({ width: 1.2, color: shade(tones[0], 0.7) });
  if (v === 0) {
    // Plank joints along the rim, an iron band round each end.
    for (const x of [x0 + (x1 - x0) * 0.36, x0 + (x1 - x0) * 0.7]) {
      g.moveTo(x, -W + 1).lineTo(x, -W + rim).moveTo(x - 3, W - rim).lineTo(x - 3, W - 1);
    }
    g.stroke({ width: 1.2, color: WOOD[0] });
    for (const x of [x0 + 0.6 * FT, x1 - 0.6 * FT]) {
      g.moveTo(x, -W - 1).lineTo(x, -W + rim).moveTo(x, W - rim).lineTo(x, W + 1).stroke({ width: 3, color: IRON[1] });
      g.moveTo(x - 1, -W).lineTo(x - 1, -W + rim - 1).stroke({ width: 1, color: IRON[3] });
    }
  } else {
    // Stone: joints between the blocks of the rim, a chip out of the south rim, moss on the north.
    for (const x of [x0 + (x1 - x0) * 0.4, x0 + (x1 - x0) * 0.78]) g.moveTo(x, -W + 1).lineTo(x + 1, -W + rim);
    g.moveTo(x0 + (x1 - x0) * 0.56, W - rim).lineTo(x0 + (x1 - x0) * 0.56, W - 1);
    g.stroke({ width: 1.2, color: shade(tones[0], 0.8) });
    const cx = x0 + (x1 - x0) * 0.24;
    g.poly([cx - 4, W - 1, cx + 5, W - 1, cx + 2, W - rim + 1, cx - 2, W - rim + 2]).fill(shade(tones[0], 0.8)).stroke({ width: 1, color: INK, join: 'round' });
    if (v === 1) {
      moss(g, x0 + (x1 - x0) * 0.66, -W + rim * 0.4, rim * 0.7, rnd);
      moss(g, x0 + rim * 0.4, -W + rim * 0.9, rim * 0.5, rnd);
    }
  }
  if (v === 3) {
    // The pump: a timber housing at the end, its handle out to the side, the spout over the water.
    const [px0, px1] = [x1 + 0.3 * FT, x1 + 1.5 * FT];
    g.moveTo(x1 - rim - 2, 0).lineTo(px0 + 2, 0).stroke({ width: 6, color: INK, cap: 'round' });
    g.moveTo(x1 - rim - 2, 0).lineTo(px0 + 2, 0).stroke({ width: 3.5, color: IRON[2], cap: 'round' });
    g.poly(oval(x1 - rim - 2, 0, 3.2, 2.4, 0, 10)).stroke({ width: 1, color: 0xe6f4ec, alpha: 0.8 });
    bevel(g, px0, -0.6 * FT, px1, 0.6 * FT, 2.5, WOOD);
    g.circle((px0 + px1) / 2, 0, 2.2).fill(IRON[1]).stroke({ width: 1, color: INK });
    const [hx, hy] = [px1 + 0.5 * FT, -1.6 * FT];
    g.moveTo((px0 + px1) / 2, 0).lineTo(hx, hy).stroke({ width: 5, color: INK, cap: 'round' });
    g.moveTo((px0 + px1) / 2, 0).lineTo(hx, hy).stroke({ width: 2.6, color: IRON[2], cap: 'round' });
    g.circle(hx, hy, 2.6).fill(WOOD[1]).stroke({ width: 1.2, color: INK });
  }
}

/** Notice board, from a high angle (it only reads from the side): a planked board on two posts, papers pinned
 * to it askew (lines of writing, a seal, a sketched face on a wanted bill), the board's top edge showing;
 * a little shingled roof over some (v1, v3), the last weathered grey. */
function noticeBoard(g: Graphics, r: number, rnd: () => number, v: number) {
  const P = upright(UPRIGHT_FOOT[47]);
  const wood = v === 3 ? [0x5e5850, 0x857c70, 0xaaa092] : WOOD;
  const ink = { width: 2, color: INK, join: 'round' as const };
  const [hw, b0, b1, top] = [2.6, 2.6, 6, v === 1 || v === 3 ? 6.6 : 6.4];
  for (const x of [-hw + 0.1, hw - 1.1]) footGrass(g, x * FT, P(0, 0.15, 0)[1], 0.13 * r + 1, rnd);
  // The posts.
  for (const x of [-hw + 0.5, hw - 0.5]) {
    g.poly(quad4(P(x - 0.25, 0, 0), P(x + 0.25, 0, 0), P(x + 0.25, 0, top), P(x - 0.25, 0, top))).fill(wood[0]).stroke(ink);
    g.poly(quad4(P(x - 0.22, 0, 0.1), P(x - 0.02, 0, 0.1), P(x - 0.02, 0, top), P(x - 0.22, 0, top))).fill(wood[1]);
  }
  // The board: its top edge seen from above, then the face, planked across.
  g.poly(quad4(P(-hw, -0.4, b1), P(hw, -0.4, b1), P(hw, 0, b1), P(-hw, 0, b1))).fill(wood[2]).stroke(ink);
  g.poly(quad4(P(-hw, 0, b0), P(hw, 0, b0), P(hw, 0, b1), P(-hw, 0, b1))).fill(wood[1]).stroke(ink);
  for (const h of [b0 + 1.15, b0 + 2.3]) g.moveTo(...P(-hw + 0.1, 0, h)).lineTo(...P(hw - 0.1, 0, h));
  g.stroke({ width: 1.2, color: wood[0], alpha: 0.8 });
  g.poly(quad4(P(-hw, 0, b1 - 0.35), P(hw, 0, b1 - 0.35), P(hw, 0, b1), P(-hw, 0, b1))).fill({ color: 0xfff1cf, alpha: 0.25 });
  // Papers: each a little askew, with writing; the biggest a bill with a face, one sealed.
  const papers: [number, number, number, number][] = v === 2 ? [[-1.5, 4.2, 1.5, 2.4], [0.6, 4.4, 1, 1.4], [1.7, 3.6, 0.8, 1.1]] : [[-1.6, 4.4, 1, 1.4], [-0.2, 4.1, 1.2, 1.7], [1.4, 4.6, 0.9, 1.2], [0.9, 3.2, 0.8, 0.9]];
  papers.forEach(([x, h, pw, ph], i) => {
    if (v === 3 && i === 3) return;
    const t = (rnd() - 0.5) * 0.25;
    const c = (u: number, k: number) => P(x + u * pw * 0.5 - k * t, 0, h + k * ph * 0.5 + u * t * 0.4);
    const paper = v === 3 && i === 2 ? 0xd8ccae : [0xf2e8cc, 0xf6efe0, 0xe8dcc0][i % 3];
    g.poly(quad4(c(-1, -1), c(1, -1), c(1, 1), c(-1, 1))).fill(paper).stroke({ width: 1.2, color: INK, join: 'round' });
    if (v === 2 && i === 0) {
      // WANTED: a bold line, a head and shoulders sketched, a line under.
      g.moveTo(...c(-0.6, 0.72)).lineTo(...c(0.6, 0.72)).stroke({ width: 2.2, color: 0x3a2a1e, cap: 'round' });
      g.poly(oval(...c(0, 0.18), 0.3 * FT, 0.2 * FT, 0, 12)).stroke({ width: 1.3, color: 0x5a4632 });
      g.moveTo(...c(-0.55, -0.42)).quadraticCurveTo(...c(0, -0.05), ...c(0.55, -0.42)).stroke({ width: 1.3, color: 0x5a4632 });
      g.moveTo(...c(-0.5, -0.7)).lineTo(...c(0.5, -0.7)).stroke({ width: 1.2, color: 0x6a5642, cap: 'round' });
    } else {
      for (let k = 0; k < 3; k++) g.moveTo(...c(-0.6, 0.5 - k * 0.42)).lineTo(...c(0.6 - 0.3 * rnd(), 0.5 - k * 0.42));
      g.stroke({ width: 1, color: 0x6a5642, cap: 'round' });
    }
    if (i === 1) g.circle(...c(0.45, -0.6), 1.8).fill(0xb5323a).stroke({ width: 0.8, color: INK });
    g.circle(...c(0, 0.85), 1.1).fill(i % 2 ? 0xc9a24a : 0x9a3a2e);
  });
  if (v === 3) g.poly(quad4(P(0.6, 0, 3.35), P(1.3, 0, 3.15), P(1.25, 0, 2.75), P(0.65, 0, 2.85))).fill(0xc8bc9e).stroke({ width: 1, color: INK, join: 'round' });
  if (v === 1 || v === 3) {
    // The roof: its far plane a lit strip behind the ridge, the near one in shade over the board's top.
    // (The slate one lighter and slighter, so the board shows under it.)
    const [eave, ridge, o, near] = v === 1 ? [b1 + 0.3, b1 + 1.8, hw + 0.45, 1] : [b1 + 0.35, b1 + 1.4, hw + 0.15, 0.6];
    const roof = v === 1 ? [0x7a3a2a, 0xa85a3e] : [0x7e7a72, 0xa29e94];
    const at = (x: number, f: number) => P(x, f * near, ridge - (ridge - eave) * Math.abs(f));
    g.poly(quad4(at(-o, -0.6), at(o, -0.6), at(o, 0), at(-o, 0))).fill(roof[1]).stroke(ink);
    g.poly(quad4(at(-o, 0), at(o, 0), at(o, 1), at(-o, 1))).fill(roof[0]).stroke(ink);
    // Shingles in staggered courses.
    for (const f of [0.34, 0.67]) g.moveTo(...at(-o + 0.05, f)).lineTo(...at(o - 0.05, f));
    for (let row = 0; row < 3; row++) {
      for (let x = -o + 0.35 + (row % 2) * 0.35; x < o - 0.2; x += 0.7) g.moveTo(...at(x, row * 0.33 + 0.03)).lineTo(...at(x, row * 0.33 + 0.31));
    }
    g.stroke({ width: 1, color: shade(roof[0], 0.68) });
    g.moveTo(...at(-o, 0)).lineTo(...at(o, 0)).stroke({ width: 2, color: shade(roof[0], 0.55) });
  }
}

/** Signpost, from a high angle: a post on a few stones, finger boards pointing off it with their lettering
 * (those pointing away behind it, those toward the viewer in front): two (v0), three (v1), a weathered post
 * with one finger broken off and hanging by a nail (v2), four painted fingers under a little cap at a
 * crossroads (v3). */
function signpost(g: Graphics, r: number, rnd: () => number, v: number) {
  const P = upright(UPRIGHT_FOOT[48]);
  const wood = v === 2 ? [0x5e5850, 0x857c70, 0xaaa092] : WOOD;
  const ink = { width: 2, color: INK, join: 'round' as const };
  const top = 8.6;
  // Fingers: [angle on the ground (0 east, +y toward the viewer), height, length].
  const looks: [number, number, number][][] = [
    [[-0.3, 7.4, 2.6], [Math.PI + 0.35, 6.6, 2.5]],
    [[-0.25, 7.6, 2.6], [Math.PI - 0.45, 6.9, 2.4], [0.5, 6.1, 2.3]],
    [[Math.PI + 0.3, 7.4, 2.5]],
    [[-0.4, 7.6, 2.5], [Math.PI + 0.4, 7.6, 2.5], [0.5, 6.8, 2.4], [Math.PI - 0.5, 6.8, 2.4]],
  ];
  const fingers = looks[v];
  const paint = v === 3 ? [0xb8ac90, 0xe8dcc0] : [wood[1], wood[2]];
  // A finger: along it `u` ft and `k` ft up its face, turned down by `droop` (radians, in its own plane) when
  // it hangs loose from its nail.
  const finger = ([a, h, len]: [number, number, number], droop = 0) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const [dc, ds] = [Math.cos(droop), Math.sin(droop)];
    const at = (u: number, k: number) => {
      const [uu, kk] = [u * dc + k * ds, k * dc - u * ds];
      return P(c * uu, s * uu, h + kk);
    };
    const th = 0.45;
    const board = [...at(0.2, th), ...at(len - 0.5, th), ...at(len, 0), ...at(len - 0.5, -th), ...at(0.2, -th)];
    if (!droop) g.poly([...at(0.2, th + 0.25), ...at(len - 0.5, th + 0.25), ...at(len - 0.42, th), ...at(0.2, th)]).fill(paint[1]).stroke({ width: 1.4, color: INK, join: 'round' });
    g.poly(board).fill(paint[0]).stroke(ink);
    for (let k = 0; k < 3; k++) {
      const u0 = 0.55 + k * 0.6;
      if (u0 + 0.4 > len - 0.5) break;
      g.moveTo(...at(u0, 0.05 * (k % 2 ? -1 : 1))).lineTo(...at(u0 + 0.35 + 0.15 * rnd(), 0.05 * (k % 2 ? -1 : 1)));
    }
    g.stroke({ width: 1.6, color: v === 3 ? 0x2a2420 : shade(wood[0], 0.7), cap: 'round' });
  };
  // Stones and grass at the foot.
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + rnd() * 0.6;
    stone(g, Math.cos(a) * 0.55 * FT, P(0, Math.sin(a) * 0.5 + 0.1, 0)[1], 0.3 * FT + rnd() * 2, STONE[i % 2], rnd, 7);
  }
  footGrass(g, -0.9 * FT, P(0, 0.3, 0)[1], 0.2 * r + 2, rnd);
  for (const f of fingers) if (Math.sin(f[0]) < 0) finger(f);
  // The post: its face with a lit west edge, its top (or cap) seen from above.
  const w = 0.3;
  g.poly(quad4(P(-w, 0, 0), P(w, 0, 0), P(w, 0, top), P(-w, 0, top))).fill(wood[0]).stroke(ink);
  g.poly(quad4(P(-w + 0.04, 0, 0.1), P(-0.02, 0, 0.1), P(-0.02, 0, top), P(-w + 0.04, 0, top))).fill(wood[1]);
  if (v === 3) g.poly([...P(-0.5, 0, top), ...P(0.5, 0, top), ...P(0, 0, top + 1)]).fill(0x7a3a2a).stroke(ink);
  else g.poly(quad4(P(-w, -0.5, top), P(w, -0.5, top), P(w, 0, top), P(-w, 0, top))).fill(wood[2]).stroke({ width: 1.5, color: INK, join: 'round' });
  for (const f of fingers) if (Math.sin(f[0]) >= 0) finger(f);
  if (v === 2) {
    // A finger broken off its peg (the splintered stub left on the post), hanging steeply by one nail at its
    // butt; lichen on the post.
    g.poly([...P(w - 0.05, 0, 7.75), ...P(0.75, 0, 7.7), ...P(0.6, 0, 7.5), ...P(0.85, 0, 7.35), ...P(w - 0.05, 0, 7.3)]).fill(paint[0]).stroke({ width: 1.5, color: INK, join: 'round' });
    finger([0, 6.9, 2.1], 1.05);
    g.circle(...P(0.32, 0, 6.9), 1.6).fill(IRON[2]).stroke({ width: 0.8, color: INK });
    for (const h of [3.2, 4.4]) g.circle(...P(-0.05, 0, h), 2.6).fill({ color: 0x9aa86a, alpha: 0.8 });
  }
}

/** Stocks, from a high angle (as the stocks indoors): two posts holding a heavy board split along its
 * middle, a head hole between two wrist holes, a hinge at one end and a padlock at the other, straw before
 * it; ankle stocks with a bench behind to sit on (v1); a pillory, the board at head height on one tall post
 * (v2); rotten fruit thrown at it (v3). */
function stocks(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const P = upright(UPRIGHT_FOOT[49]);
  const ink = { width: 2, color: INK, join: 'round' as const };
  for (let i = 0; i < 14; i++) {
    const [x, y] = P((rnd() - 0.5) * 6, 0.7 + rnd() * 1.2, 0);
    g.moveTo(x, y).lineTo(x + (rnd() - 0.5) * 7, y + (rnd() - 0.5) * 3).stroke({ width: 1.3, color: 0xc8a860, cap: 'round' });
  }
  const pillory = v === 2;
  const hw = pillory ? 2.3 : 2.9;
  const [b0, b1] = pillory ? [4.6, 5.8] : [1.1, 2.5];
  if (v === 1) {
    // The bench behind: its seat plank seen from above with a lit edge, its front edge, four stout legs.
    const [s0, s1, sh] = [-2, -1.1, 1.6];
    for (const [x, d] of [
      [-2.2, s0 + 0.15],
      [2.2, s0 + 0.15],
      [-2.2, s1 - 0.15],
      [2.2, s1 - 0.15],
    ]) {
      g.poly(quad4(P(x - 0.17, d, sh), P(x + 0.17, d, sh), P(x + 0.17, d, 0), P(x - 0.17, d, 0))).fill(WOOD[0]).stroke({ width: 1.5, color: INK, join: 'round' });
    }
    g.poly(quad4(P(-2.7, s0, sh), P(2.7, s0, sh), P(2.7, s1, sh), P(-2.7, s1, sh))).fill(WOOD[2]).stroke(ink);
    g.moveTo(...P(-2.6, s0 + 0.2, sh)).lineTo(...P(2.6, s0 + 0.2, sh)).stroke({ width: 1.5, color: WOOD[3], alpha: 0.7 });
    g.moveTo(...P(-2.6, (s0 + s1) / 2, sh)).lineTo(...P(2.6, (s0 + s1) / 2, sh)).stroke({ width: 1, color: WOOD[0], alpha: 0.7 });
    g.poly(quad4(P(-2.7, s1, sh), P(2.7, s1, sh), P(2.7, s1, sh - 0.35), P(-2.7, s1, sh - 0.35))).fill(WOOD[0]).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  if (pillory) {
    // A low platform of planks under the post.
    g.poly(quad4(P(-2, -1, 0.6), P(2, -1, 0.6), P(2, 1, 0.6), P(-2, 1, 0.6))).fill(WOOD[1]).stroke(ink);
    g.poly(quad4(P(-2, 1, 0.6), P(2, 1, 0.6), P(2, 1, 0), P(-2, 1, 0))).fill(WOOD[0]).stroke(ink);
    for (const x of [-1, 0, 1]) g.moveTo(...P(x, -1, 0.6)).lineTo(...P(x, 1, 0.6));
    g.stroke({ width: 1, color: WOOD[0] });
    g.poly(quad4(P(-0.3, 0, 0.6), P(0.3, 0, 0.6), P(0.3, 0, 7.2), P(-0.3, 0, 7.2))).fill(WOOD[0]).stroke(ink);
    g.poly(quad4(P(-0.26, 0, 0.7), P(-0.04, 0, 0.7), P(-0.04, 0, 7.2), P(-0.26, 0, 7.2))).fill(WOOD[1]);
    g.poly(quad4(P(-0.3, -0.5, 7.2), P(0.3, -0.5, 7.2), P(0.3, 0, 7.2), P(-0.3, 0, 7.2))).fill(WOOD[2]).stroke({ width: 1.5, color: INK, join: 'round' });
  } else {
    for (const x of [-hw + 0.25, hw - 0.25]) {
      g.poly(quad4(P(x - 0.3, 0, 0), P(x + 0.3, 0, 0), P(x + 0.3, 0, b1 + 0.5), P(x - 0.3, 0, b1 + 0.5))).fill(WOOD[0]).stroke(ink);
      g.poly(quad4(P(x - 0.26, 0, 0.1), P(x - 0.04, 0, 0.1), P(x - 0.04, 0, b1 + 0.5), P(x - 0.26, 0, b1 + 0.5))).fill(WOOD[1]);
      g.poly(quad4(P(x - 0.3, -0.5, b1 + 0.5), P(x + 0.3, -0.5, b1 + 0.5), P(x + 0.3, 0, b1 + 0.5), P(x - 0.3, 0, b1 + 0.5))).fill(WOOD[2]).stroke({ width: 1.5, color: INK, join: 'round' });
    }
  }
  // The board: its top seen from above, the face split along the middle, the holes cut on the split.
  const [l, rr] = pillory ? [-hw, hw] : [-hw + 0.5, hw - 0.5];
  g.poly(quad4(P(l, -0.55, b1), P(rr, -0.55, b1), P(rr, 0, b1), P(l, 0, b1))).fill(WOOD[2]).stroke(ink);
  g.poly(quad4(P(l, 0, b0), P(rr, 0, b0), P(rr, 0, b1), P(l, 0, b1))).fill(WOOD[1]).stroke(ink);
  const mid = (b0 + b1) / 2;
  g.moveTo(...P(l, 0, mid)).lineTo(...P(rr, 0, mid)).stroke({ width: 1.6, color: INK });
  const holes: [number, number][] = v === 1 ? [[-0.9, 0.24], [0.9, 0.24]] : [[0, 0.42], [-1.35, 0.24], [1.35, 0.24]];
  for (const [x, s] of holes) g.poly(oval(...P(x, 0, mid), s * FT, s * FT * 0.8, 0, 14)).fill(0x1e1712).stroke({ width: 1.2, color: INK });
  g.poly(quad4(P(rr - 0.5, 0, mid + 0.35), P(rr + 0.1, 0, mid + 0.35), P(rr + 0.1, 0, mid - 0.45), P(rr - 0.5, 0, mid - 0.45))).fill(IRON[2]).stroke({ width: 1.2, color: INK, join: 'round' });
  g.poly(oval(...P(rr - 0.2, 0, mid + 0.6), 2.2, 2.2, 0, 10)).stroke({ width: 1.3, color: IRON[3] });
  g.poly(quad4(P(l, 0, mid + 0.3), P(l + 0.4, 0, mid + 0.3), P(l + 0.4, 0, mid - 0.3), P(l, 0, mid - 0.3))).fill(IRON[1]);
  if (v === 3) {
    // Rotten fruit and an egg thrown at it: splats on the board and the ground.
    for (const [x, d, h, c] of [
      [-1.9, 0, 2.1, 0xa84a2a],
      [1.7, 0, 1.4, 0x8a9a3a],
      [0.6, 1.4, 0, 0xa84a2a],
      [-0.8, 1.1, 0, 0xe8d8a8],
      [2.4, 1.6, 0, 0x8a9a3a],
    ] as const) {
      const [sx, sy] = P(x, d, h);
      g.poly(blob(sx, sy, 3.2, 8, 0.6, rnd)).fill({ color: c, alpha: 0.85 }).stroke({ width: 1, color: shade(c, 0.6) });
      g.circle(sx - 0.8, sy - 0.8, 1).fill({ color: 0xfff1cf, alpha: 0.6 });
    }
  }
}

/** A square lantern turned corner-on, from high up (as the hanging lantern indoors): its base, the two near
 * glass sides (glowing, or plainly out: pale grey glass with a glint), corner posts, a pyramid cap,
 * a ring on top. `P` the high-angle view, `h` the lantern's foot. */
function streetLantern(g: Graphics, P: (x: number, d: number, h: number) => [number, number], x: number, h: number, lit: boolean) {
  const s = 0.5;
  const sq = (k: number, hh: number) => [P(x, k, hh), P(x + k, 0, hh), P(x, -k, hh), P(x - k, 0, hh)];
  const [bot, top] = [sq(s * 1.05, h), sq(s, h + 1)];
  const flat = (pts: [number, number][]) => pts.flat();
  if (lit) for (let i = 0; i < 5; i++) g.circle(...P(x, 0, h + 0.5), 0.9 * FT - i * 0.12 * FT).fill({ color: 0xf8c860, alpha: 0.07 });
  g.poly(flat(bot)).fill(IRON[0]).stroke({ width: 1.5, color: INK, join: 'round' });
  for (const [i, j, col] of [
    [3, 0, lit ? 0xfff0b0 : 0xc4c8c4],
    [0, 1, lit ? 0xf0c060 : 0x9ea4a6],
  ] as const) {
    g.poly(flat([bot[i], bot[j], top[j], top[i]])).fill(col).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  if (!lit) g.moveTo(...P(x - 0.32, 0.12, h + 0.8)).lineTo(...P(x - 0.22, 0.24, h + 0.55)).stroke({ width: 1.2, color: 0xf2f4f2, alpha: 0.9, cap: 'round' });
  for (const i of [3, 0, 1]) g.moveTo(...bot[i]).lineTo(...top[i]);
  g.stroke({ width: 1.6, color: IRON[0], cap: 'round' });
  const apex = P(x, 0, h + 1.7);
  const lit4 = [IRON[1], IRON[0], IRON[1], IRON[2]];
  for (let i = 0; i < 4; i++) g.poly(flat([top[i], top[(i + 1) % 4], apex])).fill(lit4[(i + 1) % 4]);
  g.poly(flat(top)).stroke({ width: 1.5, color: INK, join: 'round' });
  for (let i = 0; i < 4; i++) g.moveTo(...top[i]).lineTo(...apex);
  g.stroke({ width: 1, color: INK, alpha: 0.8 });
  g.circle(apex[0], apex[1] - 1.5, 2).stroke({ width: 2.2, color: INK });
  g.circle(apex[0], apex[1] - 1.5, 2).stroke({ width: 1, color: IRON[2] });
}

/** An iron rod from `a` to `b` (an ink line under a lit one), or one of another `color`. */
function rod(g: Graphics, a: [number, number], b: [number, number], w = 2.4, color = IRON[2]) {
  inked(g, () => g.moveTo(...a).lineTo(...b), w, color);
}

/** A stroke inked: the path `trace` draws, stroked in ink 2.4 px wider, then in `color` over it. */
function inked(g: Graphics, trace: () => void, w: number, color: number) {
  for (const [width, c] of [
    [w + 2.4, INK],
    [w, color],
  ] as const) {
    trace();
    g.stroke({ width, color: c, cap: 'round', join: 'round' });
  }
}

/** Lamp post, from a high angle: an iron post on a stone foot with a lantern on top, glowing (v0); a timber
 * post with a short iron scroll bracket hanging its lantern (v1); an iron post with two arms swept up, a
 * lantern standing on each (v2); an iron post with its lamp out and a short crossbar for the lamplighter's
 * ladder (v3). */
function lampPost(g: Graphics, r: number, rnd: () => number, v: number) {
  const P = upright(UPRIGHT_FOOT[50]);
  const ink = { width: 2, color: INK, join: 'round' as const };
  footGrass(g, 0.9 * FT, P(0, 0.4, 0)[1], 0.2 * r + 2, rnd);
  // The foot: a squat stone block, its top lit (a timber post is set straight in the ground).
  if (v !== 1) {
    const st = STONE[0];
    g.poly(quad4(P(-0.6, -0.6, 1), P(0.6, -0.6, 1), P(0.6, 0.6, 1), P(-0.6, 0.6, 1))).fill(st[2]).stroke(ink);
    g.poly(quad4(P(-0.6, 0.6, 1), P(0.6, 0.6, 1), P(0.6, 0.6, 0), P(-0.6, 0.6, 0))).fill(st[1]).stroke(ink);
  }
  const post = v === 1 ? WOOD : IRON;
  const [w, top] = [v === 1 ? 0.3 : 0.24, v === 1 ? 8 : v === 2 ? 7.4 : 8.3];
  const base = v === 1 ? 0 : 1;
  g.poly(quad4(P(-w, 0, base), P(w, 0, base), P(w, 0, top), P(-w, 0, top))).fill(post[1]).stroke(ink);
  g.poly(quad4(P(-w + 0.03, 0, base), P(-0.02, 0, base), P(-0.02, 0, top), P(-w + 0.03, 0, top))).fill(post[2]);
  if (v !== 1) {
    // A collar where the post meets its foot, and one under the lamp.
    for (const h of [1.3, top - 0.3]) g.poly(quad4(P(-0.3, 0, h - 0.25), P(0.3, 0, h - 0.25), P(0.3, 0, h + 0.25), P(-0.3, 0, h + 0.25))).fill(IRON[0]).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  if (v === 1) {
    // A short iron arm from the post's top curling up at its end, a curved brace under it, the lantern hung
    // from the arm on a short chain.
    const [ah, arm] = [top - 0.3, 1.85];
    g.poly(quad4(P(-w, -0.5, top), P(w, -0.5, top), P(w, 0, top), P(-w, 0, top))).fill(WOOD[2]).stroke({ width: 1.5, color: INK, join: 'round' });
    rod(g, P(w, 0, ah), P(arm, 0, ah));
    for (const [width, color] of [
      [4.4, INK],
      [2, IRON[2]],
    ] as const) {
      g.moveTo(...P(arm, 0, ah)).quadraticCurveTo(...P(arm + 0.4, 0, ah + 0.35), ...P(arm, 0, ah + 0.55)).quadraticCurveTo(...P(arm - 0.3, 0, ah + 0.45), ...P(arm - 0.15, 0, ah + 0.25));
      g.moveTo(...P(w, 0, ah - 1.5)).quadraticCurveTo(...P(0.4, 0, ah - 0.3), ...P(arm * 0.75, 0, ah));
      g.stroke({ width, color, cap: 'round', join: 'round' });
    }
    g.moveTo(...P(arm - 0.1, 0, ah)).lineTo(...P(arm - 0.1, 0, ah - 0.45)).stroke({ width: 1.4, color: IRON[0] });
    streetLantern(g, P, arm - 0.1, ah - 2.15, true);
  } else if (v === 2) {
    for (const side of [-1, 1]) {
      const [a, b, c]: [number, number][] = [P(0, 0, top - 0.6), P(side * 1.3, 0, top - 0.75), P(side * 1.75, 0, top + 0.2)];
      for (const [width, color] of [
        [5, INK],
        [2.6, IRON[2]],
      ] as const) {
        g.moveTo(...a).quadraticCurveTo(...b, ...c).stroke({ width, color, cap: 'round' });
      }
      streetLantern(g, P, side * 1.75, top + 0.2, true);
    }
    g.circle(...P(0, 0, top + 0.35), 2.6).fill(IRON[1]).stroke({ width: 1.4, color: INK });
  } else {
    if (v === 3) {
      rod(g, P(-0.75, 0, top - 1.2), P(0.75, 0, top - 1.2), 2.2);
    }
    streetLantern(g, P, 0, top, v === 0);
  }
}

/** A cross's shadow on the steps below it (SE, from the light in the NW), from (x, y): the shaft `len` px,
 * the arms across it near the end in proportion, a ring round the crossing on a wheel cross. */
function crossShadow(g: Graphics, x: number, y: number, len: number, wheel: boolean) {
  const sh = { color: 0x2a1a20, alpha: 0.38 };
  const [ux, uy] = [Math.SQRT1_2, Math.SQRT1_2];
  const at = (u: number, k: number) => [x + ux * u - uy * k, y + uy * u + ux * k];
  const w = len * 0.085;
  const c = len * 0.72;
  const arm = len * 0.3;
  g.poly([...at(0, -w), ...at(c - w, -w), ...at(c - w, -arm), ...at(c + w, -arm), ...at(c + w, -w), ...at(len, -w), ...at(len, w), ...at(c + w, w), ...at(c + w, arm), ...at(c - w, arm), ...at(c - w, w), ...at(0, w)]).fill(sh);
  if (wheel) {
    const [rx, ry] = at(c, 0);
    g.circle(rx, ry, len * 0.17).stroke({ width: w * 1.2, ...sh });
  }
}

/** A stone cross standing up, from a high angle (as the headstone's cross), its foot at (0, `foot`) and
 * `k` times life size: stout, so it still reads as a cross on a square seen from far up; the tops of its arms
 * and head lit behind the front face, a groove down the shaft and across the arms, the lower right in shade;
 * a ring round the crossing on a wheel cross. */
function standingCross(g: Graphics, foot: number, k: number, tones: readonly number[], wheel: boolean) {
  const P0 = upright(foot);
  const P = (x: number, d: number, h: number) => P0(x * k, d * k, h * k);
  const [sw, arm, a0, a1, head] = [0.6, 1.8, 5.9, 7.55, 9.4];
  const face = (d: number) => [
    ...P(-sw, d, 0), ...P(sw, d, 0), ...P(sw, d, a0), ...P(arm, d, a0), ...P(arm, d, a1), ...P(sw, d, a1), ...P(sw, d, head),
    ...P(-sw, d, head), ...P(-sw, d, a1), ...P(-arm, d, a1), ...P(-arm, d, a0), ...P(-sw, d, a0),
  ];
  const ink = { width: Math.max(1.2, 3 * k), color: INK, join: 'round' as const };
  g.poly(face(-0.7)).fill(tones[2]).stroke(ink);
  if (wheel) {
    // Dark between the ring and the arms, so the ring stands out.
    const [cx, cy] = P(0, 0, (a0 + a1) / 2);
    const [rx, ry] = [1.65 * FT * k, 1.65 * FT * k * 0.72];
    g.poly(oval(cx, cy, rx, ry, 0, 24)).fill(shade(tones[0], 0.45));
  }
  g.poly(face(0)).fill(tones[1]).stroke(ink);
  if (wheel) {
    // The ring, bold, standing proud of the arms: inked, lit along its upper edge.
    const [cx, cy] = P(0, 0, (a0 + a1) / 2);
    const [rx, ry] = [1.65 * FT * k, 1.65 * FT * k * 0.72];
    g.poly(oval(cx, cy, rx, ry, 0, 28)).stroke({ width: 0.3 * FT * k + 3.5, color: INK });
    g.poly(oval(cx, cy, rx, ry, 0, 28)).stroke({ width: 0.3 * FT * k, color: tones[1] });
    g.moveTo(cx - rx * 0.8, cy - ry * 0.6).quadraticCurveTo(cx, cy - ry * 1.2, cx + rx * 0.8, cy - ry * 0.6).stroke({ width: 0.1 * FT * k, color: tones[2], cap: 'round' });
  }
  if (k < 0.5) return;
  // The shade side: the east half of the shaft and the underside of the east arm.
  g.poly([...P(sw * 0.1, 0, 0.2), ...P(sw, 0, 0.2), ...P(sw, 0, a0 - 0.2), ...P(sw * 0.1, 0, a0 - 0.2)]).fill({ color: shade(tones[0], 0.8), alpha: 0.6 });
  g.poly([...P(sw, 0, a0 + 0.1), ...P(arm - 0.1, 0, a0 + 0.1), ...P(arm - 0.1, 0, (a0 + a1) / 2), ...P(sw, 0, (a0 + a1) / 2)]).fill({ color: shade(tones[0], 0.8), alpha: 0.5 });
  g.moveTo(...P(0, 0, 0.6)).lineTo(...P(0, 0, head - 0.4)).moveTo(...P(-arm + 0.35, 0, (a0 + a1) / 2)).lineTo(...P(arm - 0.35, 0, (a0 + a1) / 2));
  g.stroke({ width: 2.6, color: shade(tones[0], 0.85), cap: 'round' });
}

/** A regular polygon's corners: `n` of them round (0, 0) at radius `R`, the first at angle `ph`. */
function ngon(R: number, n: number, ph: number): number[][] {
  return Array.from({ length: n }, (_, i) => [Math.cos(ph + (i / n) * Math.PI * 2) * R, Math.sin(ph + (i / n) * Math.PI * 2) * R]);
}

/** A regular polygon's radius at angle `a` (circumradius `R`, `n` sides, a corner at `ph`), as `around` takes it. */
function polyR(R: number, n: number, ph: number) {
  const span = (Math.PI * 2) / n;
  return (a: number) => (R * Math.cos(Math.PI / n)) / Math.cos(((((a - ph) % span) + span) % span) - Math.PI / n);
}

/** Market cross: stepped octagonal treads seen from above (lit on the NW, a joint here and there), a socket
 * stone and on it a stone cross standing up, from a high angle, its shadow falling short down the steps
 * (v0); a buttercross, its octagonal roof on pillars over the steps, a little cross on its peak (v1); an old
 * one, a step cracked, moss and flowers left at its foot (v2); a wheel cross on square steps (v3). */
function marketCross(g: Graphics, r: number, rnd: () => number, v: number) {
  const st = v === 2 ? STONE[3] : STONE[0];
  const sides = v === 3 ? 4 : 8;
  const ph = v === 3 ? Math.PI / 4 + (rnd() - 0.5) * 0.1 : Math.PI / 8;
  const tier = (R: number, ink: number, dk = 1) => {
    const pts = ngon(R, sides, ph);
    g.poly(pts.flat()).fill(shade(st[1], dk)).stroke({ width: ink, color: INK, join: 'round' });
    // Each edge's band: lit toward the light, shaded away from it.
    const d = Math.min(0.42 * FT, R * 0.14);
    for (let i = 0; i < sides; i++) {
      const [a, b] = [pts[i], pts[(i + 1) % sides]];
      const k = (R - d) / R;
      const mid = Math.atan2(a[1] + b[1], a[0] + b[0]);
      g.poly([a[0], a[1], b[0], b[1], b[0] * k, b[1] * k, a[0] * k, a[1] * k]).fill(shade(st[1], dk * (0.95 + 0.17 * lit(mid))));
    }
    g.poly(pts.flat()).stroke({ width: ink, color: INK, join: 'round' });
  };
  const steps = v === 1 ? [r * 0.97, r * 0.84] : v === 3 ? [r * 1.02, r * 0.78, r * 0.55] : [r * 0.95, r * 0.73, r * 0.51];
  // The bottom step darker, so the steps stand off the paving round them.
  steps.forEach((R, i) => tier(R, i ? 2 : 3, i ? 1 : 0.8));
  // A joint across each tread here and there.
  const edge = (R: number, a: number) => polyR(R, sides, ph)(a);
  for (let i = 0; i + 1 < steps.length; i++) {
    for (let k = 0; k < 3; k++) {
      const a = ph + 0.4 + k * 2.3 + i * 1.1;
      const [r0, r1] = [edge(steps[i + 1], a) + 1.5, edge(steps[i], a) - 1.5];
      g.moveTo(Math.cos(a) * r0, Math.sin(a) * r0).lineTo(Math.cos(a) * r1, Math.sin(a) * r1);
    }
  }
  g.stroke({ width: 1.2, color: shade(st[0], 0.85) });
  const cross = [0xb8b0a0, 0xd8d1c2, 0xeee8da];
  if (v === 1) {
    // Square pillars at the corners of the upper tread, standing out past the roof's corners, the dark of the
    // open hall under the eaves; the roof over them: eight planes up to a peak, each lit as it faces the
    // light, slates lapped along the eaves; a little cross on the peak.
    const R = r * 0.62;
    g.poly(ngon(R * 1.16, 8, ph).flat()).fill(0x221a14).stroke({ width: 2, color: INK, join: 'round' });
    for (const [x, y] of ngon(r * 0.76, 8, ph)) {
      const c = 0.42 * FT;
      g.rect(x - c + 2, y - c + 2, c * 2, c * 2).fill({ color: 0x2a1a20, alpha: 0.35 });
      bevel(g, x - c, y - c, x + c, y + c, 2, st, 1.8);
    }
    const pts = ngon(R, 8, ph);
    g.poly(pts.flat().map((p) => p + 5)).fill({ color: 0x140c08, alpha: 0.55 });
    for (let i = 0; i < 8; i++) {
      const [a, b] = [pts[i], pts[(i + 1) % 8]];
      const mid = Math.atan2(a[1] + b[1], a[0] + b[0]);
      const tone = shade(0x8a7a66, 0.84 + 0.28 * lit(mid));
      g.poly([0, 0, a[0], a[1], b[0], b[1]]).fill(tone);
      for (let k = 0; k < 4; k++) {
        const f = 0.55 + 0.12 * (k % 2) + 0.1 * rnd();
        const t = 0.2 + 0.2 * k;
        const [x, y] = [(a[0] + (b[0] - a[0]) * t) * f, (a[1] + (b[1] - a[1]) * t) * f];
        const [dx, dy] = [(b[0] - a[0]) * 0.08 * f, (b[1] - a[1]) * 0.08 * f];
        g.moveTo(x - dx, y - dy).lineTo(x + dx, y + dy);
      }
      g.stroke({ width: 1.2, color: shade(tone, 0.72), cap: 'round' });
    }
    // Hips from the peak to the corners, lead along them.
    for (const [x, y] of pts) g.moveTo(0, 0).lineTo(x, y);
    g.stroke({ width: 2.4, color: shade(0x6e6a62, 0.75) });
    g.poly(pts.flat()).stroke({ width: 2.5, color: INK, join: 'round' });
    g.circle(0, 0, 0.4 * FT).fill(st[2]).stroke({ width: 1.5, color: INK });
    standingCross(g, 0, 0.32, cross, false);
    return;
  }
  if (v === 2) {
    // A crack through the lowest step with a stone come loose on the tread, moss on the north treads,
    // flowers laid on the steps.
    const a = ph + (1.3 / 8) * Math.PI * 2;
    const [r0, r1] = [edge(steps[1], a), edge(steps[0], a)];
    g.moveTo(Math.cos(a) * r0, Math.sin(a) * r0)
      .lineTo(Math.cos(a + 0.06) * (r0 + r1) * 0.5, Math.sin(a + 0.06) * (r0 + r1) * 0.5)
      .lineTo(Math.cos(a - 0.03) * r1, Math.sin(a - 0.03) * r1)
      .stroke({ width: 1.6, color: INK, join: 'round' });
    stone(g, Math.cos(a + 0.14) * (r0 + r1) * 0.5, Math.sin(a + 0.14) * (r0 + r1) * 0.5, 0.28 * FT, st, rnd, 6);
    moss(g, -steps[1] * 0.25, -steps[1] * 0.86, 0.32 * FT, rnd);
    moss(g, -steps[0] * 0.7, -steps[0] * 0.5, 0.26 * FT, rnd);
    const fa = ph + (2.5 / 8) * Math.PI * 2;
    const fr = (steps[1] + steps[2]) * 0.5;
    for (let i = 0; i < 3; i++) g.circle(Math.cos(fa) * fr + i * 4 - 4, Math.sin(fa) * fr + (i % 2) * 3 - 1, 2.4).fill([0xe0533d, 0xf2c94c, 0xf6efe0][i]).stroke({ width: 0.8, color: INK });
  }
  // The socket stone, the cross's shadow falling short down the steps, then the cross standing on it (a
  // little over life size, standing a little forward on its socket, so it reads from far up).
  const s0 = steps[steps.length - 1] * 0.5;
  bevel(g, -s0, -s0, s0, s0, 3, st.map((c) => shade(c, 1.04)), 2);
  const [foot, k] = [0.6 * FT, 1.2];
  crossShadow(g, 0, foot, r * 0.8, v === 3);
  standingCross(g, foot, k, v === 2 ? cross.map((c) => shade(c, 0.9)) : cross, v === 3);
  if (v === 2) moss(g, -0.35 * FT * k, foot - 5 * FT * k * 0.5, 0.26 * FT, rnd);
}

// Parks and gardens.

/** `n` points round (0, 0) at radius `R(a)`, from angle `ph`. */
function around(n: number, R: (a: number) => number, ph = 0): number[] {
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    pts.push(Math.cos(a) * R(a), Math.sin(a) * R(a));
  }
  return pts;
}

/** A closed curve through the middles of a polygon's edges, its corners pulling it round (a smoothed
 * outline); fill or stroke it after. */
function smooth(g: Graphics, pts: number[]) {
  const n = pts.length / 2;
  const mid = (i: number): [number, number] => {
    const [a, b] = [i % n, (i + 1) % n];
    return [(pts[a * 2] + pts[b * 2]) / 2, (pts[a * 2 + 1] + pts[b * 2 + 1]) / 2];
  };
  g.moveTo(...mid(n - 1));
  for (let i = 0; i < n; i++) g.quadraticCurveTo(pts[i * 2], pts[i * 2 + 1], ...mid(i));
  g.closePath();
}

/** The band between two outlines of as many points (a rim seen from above), each piece lit as it faces the
 * light, both outlines inked. */
function rimBand(g: Graphics, outer: number[], inner: number[], tone: number, ink = 2.2, light = 0.2) {
  g.poly(outer).fill(tone);
  const n = outer.length / 2;
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const mid = Math.atan2(outer[i * 2 + 1] + outer[j * 2 + 1], outer[i * 2] + outer[j * 2]);
    g.poly([outer[i * 2], outer[i * 2 + 1], outer[j * 2], outer[j * 2 + 1], inner[j * 2], inner[j * 2 + 1], inner[i * 2], inner[i * 2 + 1]]).fill(shade(tone, 0.9 + light * lit(mid)));
  }
  g.poly(outer).stroke({ width: ink, color: INK, join: 'round' });
  g.poly(inner).stroke({ width: Math.max(1.2, ink * 0.6), color: INK, join: 'round' });
}

/** Water seen from above inside the outline `R(a)` (a basin, a pond): deeper toward the middle, the shadow
 * of its rim or bank along the NW side, two ripples and a glint; murky water green and still. */
function water(g: Graphics, R: (a: number) => number, rnd: () => number, murky = false) {
  const t = murky ? [0x2e4a2a, 0x4a6a3c, 0x6e8a58] : [0x22566a, 0x3a7f8a, 0x6ab0a8];
  const s = (R(0) + R(Math.PI / 2) + R(Math.PI) + R(Math.PI * 1.5)) / 4;
  g.poly(around(64, R)).fill(t[1]);
  g.poly(around(48, (a) => R(a) * 0.66).map((p) => p + s * 0.05)).fill({ color: t[0], alpha: 0.4 });
  g.poly(around(36, (a) => R(a) * 0.36).map((p) => p + s * 0.08)).fill({ color: t[0], alpha: 0.35 });
  // The rim's shadow: a crescent inside its NW side, widest facing the light.
  const crescent: number[] = [];
  const [a0, a1] = [Math.PI * 0.62, Math.PI * 1.88];
  for (let i = 0; i <= 24; i++) {
    const a = a0 + ((a1 - a0) * i) / 24;
    crescent.push(Math.cos(a) * R(a), Math.sin(a) * R(a));
  }
  for (let i = 24; i >= 0; i--) {
    const a = a0 + ((a1 - a0) * i) / 24;
    const k = Math.max(0, R(a) - s * 0.13 * Math.sin(((a - a0) / (a1 - a0)) * Math.PI));
    crescent.push(Math.cos(a) * k, Math.sin(a) * k);
  }
  g.poly(crescent).fill({ color: 0x123848, alpha: murky ? 0.3 : 0.4 });
  if (murky) {
    for (let i = 0; i < 7; i++) {
      const a = rnd() * Math.PI * 2;
      const d = R(a) * 0.75 * Math.sqrt(rnd());
      g.poly(blob(Math.cos(a) * d, Math.sin(a) * d, s * (0.05 + 0.05 * rnd()), 7, 0.5, rnd)).fill({ color: 0x8a9a52, alpha: 0.7 });
    }
    return;
  }
  for (let i = 0; i < 2; i++) {
    const a = rnd() * Math.PI * 2;
    const d = R(a) * (0.3 + 0.3 * rnd());
    const [x, y, w] = [Math.cos(a) * d, Math.sin(a) * d, s * (0.12 - i * 0.03)];
    g.moveTo(x - w, y).quadraticCurveTo(x, y - w * 0.35, x + w, y).stroke({ width: 1.5, color: t[2], alpha: 0.9, cap: 'round' });
  }
  const ga = Math.PI * 1.25;
  g.poly(oval(Math.cos(ga) * R(ga) * 0.5, Math.sin(ga) * R(ga) * 0.5, s * 0.1, s * 0.045, -0.5, 10)).fill({ color: 0xe6f4ec, alpha: 0.75 });
}

/** Rings where falling water meets the pool below: short white arcs round a circle of radius `R`. */
function dripRing(g: Graphics, R: number, n: number, rnd: () => number) {
  for (let i = 0; i < n; i++) {
    const a = ((i + rnd() * 0.5) / n) * Math.PI * 2;
    g.moveTo(Math.cos(a) * R, Math.sin(a) * R).arc(0, 0, R, a, a + (Math.PI * 2 * 0.45) / n);
  }
  g.stroke({ width: 2, color: 0xe6f4ec, alpha: 0.75, cap: 'round' });
}

/** A jet from above: white spray thrown out every way from (x, y), droplets round it. */
function jet(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2 + rnd() * 0.3;
    const l = s * (0.7 + 0.3 * rnd());
    g.moveTo(x + Math.cos(a) * s * 0.15, y + Math.sin(a) * s * 0.15).lineTo(x + Math.cos(a) * l, y + Math.sin(a) * l);
  }
  g.stroke({ width: 2.2, color: 0xf2fafa, alpha: 0.85, cap: 'round' });
  for (let i = 0; i < 8; i++) {
    const a = rnd() * Math.PI * 2;
    const d = s * (0.8 + 0.5 * rnd());
    g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, 1.3).fill({ color: 0xf2fafa, alpha: 0.8 });
  }
  g.circle(x, y, s * 0.28).fill(0xf6fcfc).stroke({ width: 1.2, color: 0x6ab0a8 });
}

/** A round stone bowl from above at (0, 0): its rim (outer radius `R`) lit toward the light, water inside. */
function bowl(g: Graphics, R: number, tones: readonly number[], rnd: () => number) {
  g.circle(R * 0.12, R * 0.14, R).fill({ color: 0x0e2a34, alpha: 0.35 });
  rimBand(g, around(36, () => R), around(36, () => R * 0.74), tones[2], 2);
  water(g, () => R * 0.74, rnd);
}

/** Fountain (a garden's; a square's big raised basin is drawn with the square): a round stone basin, an
 * upper bowl on a pedestal throwing up a jet (v0); a quatrefoil basin round an urn pouring four ways (v1);
 * three tiers in an octagonal basin (v2); an old one, its water gone low, still and green below a stained
 * band, its rim cracked and a stone come away, the bowl broken off its pedestal, moss (v3). */
function fountain(g: Graphics, r: number, rnd: () => number, v: number) {
  const st = STONE[[0, 2, 0, 3][v]];
  const R0 = r * 0.93;
  const rim = r * 0.15;
  const shape = v === 1 ? (a: number) => R0 * (0.8 + 0.2 * Math.abs(Math.cos(2 * a))) : v === 2 ? polyR(R0, 8, Math.PI / 8) : () => R0;
  const inner = (a: number) => shape(a) - rim;
  rimBand(g, around(64, shape), around(64, inner), st[2], 2.5, v === 3 ? 0.4 : 0.2);
  // Joints across the rim: at the octagon's corners, the quatrefoil's cusps and lobes, else every tenth.
  const joints = v === 2 ? 8 : v === 1 ? 8 : 10;
  for (let i = 0; i < joints; i++) {
    const a = (v === 2 ? Math.PI / 8 : 0.2) + (i / joints) * Math.PI * 2;
    g.moveTo(Math.cos(a) * (inner(a) + 1.5), Math.sin(a) * (inner(a) + 1.5)).lineTo(Math.cos(a) * (shape(a) - 1.5), Math.sin(a) * (shape(a) - 1.5));
  }
  g.stroke(v === 3 ? { width: 2.2, color: shade(st[0], 0.6) } : { width: 1.3, color: shade(st[0], 0.85) });
  if (v === 3) {
    // Gone low: the basin's inside stained and dry down to the waterline, streaks down it, green water
    // lying low, and the rim's shadow falling deep across the NW side.
    g.poly(around(64, inner)).fill(st[1]);
    for (let i = 0; i < 14; i++) {
      const a = (i / 14) * Math.PI * 2 + rnd() * 0.3;
      const k = 0.88 + 0.05 * rnd();
      g.moveTo(Math.cos(a) * inner(a) * 0.99, Math.sin(a) * inner(a) * 0.99).lineTo(Math.cos(a) * inner(a) * k, Math.sin(a) * inner(a) * k);
    }
    g.stroke({ width: 1.6, color: st[0], alpha: 0.8, cap: 'round' });
    water(g, (a) => inner(a) * 0.8, rnd, true);
    g.poly(around(64, (a) => inner(a) * 0.8)).stroke({ width: 1.2, color: 0x3e3a2a, alpha: 0.8 });
    // The old waterline: an olive-brown stain round the basin a little above the water.
    g.poly(around(64, (a) => inner(a) * 0.85)).stroke({ width: 3, color: 0x6a5a2e });
    const shadow: number[] = [];
    for (let i = 0; i <= 24; i++) {
      const a = Math.PI * (0.62 + (1.26 * i) / 24);
      shadow.push(Math.cos(a) * inner(a), Math.sin(a) * inner(a));
    }
    for (let i = 24; i >= 0; i--) {
      const a = Math.PI * (0.62 + (1.26 * i) / 24);
      const k = inner(a) - r * 0.26 * Math.sin((i / 24) * Math.PI);
      shadow.push(Math.cos(a) * k, Math.sin(a) * k);
    }
    g.poly(shadow).fill({ color: 0x1a2418, alpha: 0.4 });
  } else water(g, inner, rnd);
  g.poly(around(64, inner)).stroke({ width: 1.5, color: INK, join: 'round' });
  if (v === 0) {
    g.circle(r * 0.06, r * 0.08, r * 0.38).fill({ color: 0x0e2a34, alpha: 0.25 });
    dripRing(g, r * 0.42, 9, rnd);
    bowl(g, r * 0.34, st, rnd);
    jet(g, 0, 0, r * 0.2, rnd);
  } else if (v === 1) {
    // Four streams arching out of the urn into the lobes, rings where they land.
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * Math.PI * 2;
      const [c, s] = [Math.cos(a), Math.sin(a)];
      const [x0, y0, x1, y1] = [c * r * 0.18, s * r * 0.18, c * r * 0.52, s * r * 0.52];
      const [mx, my] = [c * r * 0.36 - s * r * 0.04, s * r * 0.36 + c * r * 0.04];
      g.moveTo(x0, y0).quadraticCurveTo(mx, my, x1, y1).stroke({ width: 5, color: 0x5e9ea4, alpha: 0.7, cap: 'round' });
      g.moveTo(x0, y0).quadraticCurveTo(mx, my, x1, y1).stroke({ width: 2.4, color: 0xf2fafa, alpha: 0.9, cap: 'round' });
      g.circle(x1, y1, r * 0.06).stroke({ width: 1.5, color: 0xe6f4ec, alpha: 0.8 });
      g.circle(x1, y1, r * 0.1).stroke({ width: 1.2, color: 0xe6f4ec, alpha: 0.5 });
    }
    // The urn from above: its swell lit toward the light, the lip round its dark mouth, bubbling.
    g.circle(r * 0.05, r * 0.06, r * 0.27).fill({ color: 0x0e2a34, alpha: 0.3 });
    g.circle(0, 0, r * 0.26).fill(st[1]).stroke({ width: 2, color: INK });
    g.circle(-r * 0.05, -r * 0.05, r * 0.19).fill(st[2]);
    g.circle(0, 0, r * 0.17).fill(st[3]).stroke({ width: 1.5, color: INK });
    g.circle(0, 0, r * 0.11).fill(0x2a6070).stroke({ width: 1, color: shade(st[0], 0.7) });
    g.circle(-r * 0.02, -r * 0.02, r * 0.05).fill({ color: 0xf2fafa, alpha: 0.9 });
    // A lily pad in one lobe.
    const [lx, ly, ls] = [-r * 0.6, r * 0.08, r * 0.09];
    g.moveTo(lx, ly).arc(lx, ly, ls, 0.5, Math.PI * 2 - 0.1).closePath().fill(0x5e8a3a).stroke({ width: 1, color: 0x2a3a18 });
  } else if (v === 2) {
    g.circle(r * 0.08, r * 0.1, r * 0.5).fill({ color: 0x0e2a34, alpha: 0.2 });
    dripRing(g, r * 0.56, 11, rnd);
    bowl(g, r * 0.48, st, rnd);
    dripRing(g, r * 0.3, 7, rnd);
    bowl(g, r * 0.24, st.map((c) => shade(c, 1.05)), rnd);
    jet(g, 0, 0, r * 0.12, rnd);
  } else {
    // A stone gone from the rim (a dark gap, the stone lying outside), a crack, moss on the north rim,
    // brown leaves on the water; the pedestal still standing, broken off short, a piece of its bowl leaning
    // at its foot.
    const ga = Math.PI * 0.3;
    const gap = [ga - 0.2, ga + 0.2].flatMap((a) => [Math.cos(a) * (shape(a) - 1), Math.sin(a) * (shape(a) - 1)]);
    const gin = [ga + 0.2, ga - 0.2].flatMap((a) => [Math.cos(a) * (inner(a) + 1), Math.sin(a) * (inner(a) + 1)]);
    g.poly([...gap, ...gin]).fill(0x3e3a32).stroke({ width: 1.5, color: INK, join: 'round' });
    stone(g, Math.cos(ga) * r * 1.0, Math.sin(ga) * r * 1.0 - 4, r * 0.13, st, rnd, 6);
    const ca = Math.PI * 1.1;
    g.moveTo(Math.cos(ca) * inner(ca), Math.sin(ca) * inner(ca)).lineTo(Math.cos(ca + 0.05) * (inner(ca) + rim * 0.5), Math.sin(ca + 0.05) * (inner(ca) + rim * 0.5)).lineTo(Math.cos(ca - 0.02) * shape(ca), Math.sin(ca - 0.02) * shape(ca));
    g.stroke({ width: 1.6, color: INK, join: 'round' });
    moss(g, -r * 0.3, -r * 0.82, r * 0.16, rnd);
    moss(g, -r * 0.78, -r * 0.3, r * 0.12, rnd);
    for (let i = 0; i < 3; i++) {
      const a = Math.PI * (0.1 + i * 0.55) + rnd() * 0.3;
      leaf(g, Math.cos(a) * r * 0.48, Math.sin(a) * r * 0.48, rnd() * Math.PI * 2, r * 0.18, r * 0.06, [0x6a4a28, 0x7e5a32, 0x5e4426][i], 0.4);
    }
    // The pedestal: a round column broken off short, its shadow on the water, the east side in shade, the
    // break a jagged lit top.
    const pr = r * 0.22;
    g.poly(oval(pr * 0.7, pr * 0.8, pr * 1.1, pr * 0.8, 0.7, 14)).fill({ color: 0x0e1a14, alpha: 0.4 });
    g.circle(0, 0, pr).fill(st[1]).stroke({ width: 2.2, color: INK });
    g.moveTo(Math.cos(-Math.PI / 4) * pr, Math.sin(-Math.PI / 4) * pr).arc(0, 0, pr, -Math.PI / 4, Math.PI * 0.75).closePath().fill({ color: st[0], alpha: 0.8 });
    // The break: a jagged face, lit on its NW side, darker toward the SE, a crack across it.
    const top = Array.from({ length: 7 }, (_, i) => {
      const a = 0.3 + (i / 7) * Math.PI * 2;
      const k = pr * (i % 2 ? 0.46 + 0.06 * rnd() : 0.64 + 0.1 * rnd());
      return [Math.cos(a) * k - pr * 0.1, Math.sin(a) * k - pr * 0.1];
    }).flat();
    g.poly(top).fill(st[2]).stroke({ width: 1.4, color: INK, join: 'round' });
    g.poly(top.map((p) => p * 0.5 - pr * 0.1)).fill({ color: st[3], alpha: 0.45 });
    g.moveTo(-pr * 0.4, -pr * 0.12).lineTo(-pr * 0.1, -pr * 0.2).lineTo(pr * 0.06, -pr * 0.04).lineTo(pr * 0.28, pr * 0.02).stroke({ width: 1.3, color: shade(st[0], 0.7), join: 'round' });
    // A piece of the bowl lying at its SE foot: a curved slab of stone, its outer edge lit.
    const [bx, by, br] = [r * 0.32, r * 0.3, r * 0.2];
    const [s0, s1] = [Math.PI * 1.05, Math.PI * 1.7];
    g.moveTo(bx + Math.cos(s0) * br, by + Math.sin(s0) * br)
      .arc(bx, by, br, s0, s1)
      .lineTo(bx + Math.cos(s1) * br * 0.45, by + Math.sin(s1) * br * 0.45)
      .arc(bx, by, br * 0.45, s1, s0, true)
      .closePath()
      .fill(st[0])
      .stroke({ width: 1.8, color: INK, join: 'round' });
    g.moveTo(bx + Math.cos(s0 + 0.08) * br * 0.94, by + Math.sin(s0 + 0.08) * br * 0.94).arc(bx, by, br * 0.94, s0 + 0.08, s1 - 0.08).stroke({ width: 2.2, color: st[2], cap: 'round' });
    footGrass(g, -r * 0.88, r * 0.42, r * 0.14, rnd);
  }
}

/** A duck on the water at (x, y), heading `a`: its body, the wake spreading behind it, its head and bill; a
 * mallard drake or a white duck. */
function duck(g: Graphics, x: number, y: number, a: number, s: number, white: boolean) {
  const [c, sn] = [Math.cos(a), Math.sin(a)];
  const at = (u: number, w: number): [number, number] => [x + c * u - sn * w, y + sn * u + c * w];
  g.moveTo(...at(-s * 0.9, -s * 0.2)).lineTo(...at(-s * 2.6, -s * 1.1)).moveTo(...at(-s * 0.9, s * 0.2)).lineTo(...at(-s * 2.6, s * 1.1));
  g.stroke({ width: 1.5, color: 0xe6f4ec, alpha: 0.7, cap: 'round' });
  const body = white ? [0xd8d4c8, 0xf6f2e8] : [0x6e604c, 0x9a8a70];
  g.poly(oval(...at(-s * 0.1, 0), s, s * 0.6, a, 14)).fill(body[0]).stroke({ width: 1.5, color: INK });
  g.poly(oval(...at(-s * 0.2, -s * 0.08), s * 0.7, s * 0.36, a, 12)).fill(body[1]);
  g.poly([...at(s * 1.25, -s * 0.12), ...at(s * 1.7, 0), ...at(s * 1.25, s * 0.12)]).fill(0xe0a030).stroke({ width: 1, color: INK, join: 'round' });
  g.circle(...at(s * 0.95, 0), s * 0.38).fill(white ? 0xf6f2e8 : 0x2e6a3a).stroke({ width: 1.5, color: INK });
  if (!white) g.circle(...at(s * 0.62, 0), s * 0.36).stroke({ width: 1.2, color: 0xf6f2e8 });
}

/** Pond: a muddy bank fading into the ground, still water deeper toward the middle; lily pads with a flower
 * or two and stones on the bank (v0); reeds and cattails crowding one side (v1); a formal pool in a stone
 * coping lit along the north and west, koi under lily pads (v2); a duck pond, the ducks' wakes behind them
 * and a feather on the bank (v3). */
function pond(g: Graphics, r: number, rnd: () => number, v: number) {
  const pad = (x: number, y: number, s: number, flower: number | null) => {
    const n = rnd() * Math.PI * 2;
    g.moveTo(x, y).arc(x, y, s, n + 0.4, n + Math.PI * 2 - 0.4).closePath().fill(0x5e8a3a).stroke({ width: 1.2, color: 0x2a3a18 });
    g.circle(x - s * 0.3, y - s * 0.3, s * 0.35).fill({ color: 0x86b052, alpha: 0.8 });
    if (flower !== null) {
      for (let p = 0; p < 6; p++) {
        const pa = (p / 6) * Math.PI * 2;
        g.poly(oval(x + Math.cos(pa) * s * 0.35, y + Math.sin(pa) * s * 0.35, s * 0.36, s * 0.16, pa, 8)).fill(shade(flower, 1 + 0.12 * lit(pa))).stroke({ width: 0.8, color: shade(flower, 0.55) });
      }
      g.circle(x, y, s * 0.16).fill(0xf2c94c);
    }
  };
  if (v === 2) {
    // A formal pool: the coping's blocks lit along the north and west, clear water, koi, lily pads.
    const [W, H, d] = [r * 0.9, r * 0.6, r * 0.13];
    bevel(g, -W, -H, W, H, d, STONE[1].map((c) => shade(c, 1.08)), 2.5);
    for (let x = -W + r * 0.32; x < W - d; x += r * 0.34) g.moveTo(x, -H + 1).lineTo(x, -H + d).moveTo(x + r * 0.15, H - d).lineTo(x + r * 0.15, H - 1);
    for (let y = -H + r * 0.3; y < H - d; y += r * 0.36) g.moveTo(-W + 1, y).lineTo(-W + d, y).moveTo(W - d, y + r * 0.1).lineTo(W - 1, y + r * 0.1);
    g.stroke({ width: 1.2, color: shade(STONE[1][0], 0.85) });
    const [w, h] = [W - d, H - d];
    water(g, (a) => Math.min(w / Math.max(1e-6, Math.abs(Math.cos(a))), h / Math.max(1e-6, Math.abs(Math.sin(a)))), rnd);
    g.rect(-w, -h, w * 2, h * 2).stroke({ width: 1.5, color: INK });
    for (const [x, y, a, c] of [
      [-r * 0.35, -r * 0.1, 0.3, 0xe8742a],
      [r * 0.1, r * 0.22, Math.PI + 0.4, 0xf2efe6],
      [r * 0.45, -r * 0.18, Math.PI * 0.85, 0xe8742a],
    ] as const) {
      const [cs, sn] = [Math.cos(a), Math.sin(a)];
      const at = (u: number, k: number): [number, number] => [x + cs * u - sn * k, y + sn * u + cs * k];
      g.poly([...at(-r * 0.08, 0), ...at(-r * 0.14, -r * 0.045), ...at(-r * 0.14, r * 0.045)]).fill({ color: c, alpha: 0.85 });
      g.poly(oval(...at(0, 0), r * 0.085, r * 0.032, a, 12)).fill({ color: c, alpha: 0.9 }).stroke({ width: 1, color: shade(c, 0.55), alpha: 0.8 });
      if (c === 0xf2efe6) g.circle(...at(r * 0.02, 0), r * 0.02).fill(0xe8742a);
    }
    pad(-r * 0.55, r * 0.3, r * 0.09, 0xf6efe0);
    pad(-r * 0.4, r * 0.38, r * 0.07, null);
    pad(r * 0.62, r * 0.32, r * 0.08, null);
    return;
  }
  const [p1, p2] = [rnd() * Math.PI * 2, rnd() * Math.PI * 2];
  const R = (a: number) => r * 0.72 * (1 + 0.09 * Math.sin(2 * a + p1) + 0.06 * Math.sin(3 * a + p2)) * (v === 1 ? 1 + 0.08 * Math.cos(2 * a) : 1);
  fade(g, around(48, (a) => R(a) * 1.16), 0x4a3e26, 0.4);
  smooth(g, around(40, (a) => R(a) * 1.07));
  g.fill(0x76643e);
  smooth(g, around(40, (a) => R(a) * 1.07));
  g.stroke({ width: 1.2, color: 0x4a3e26, alpha: 0.7 });
  water(g, R, rnd);
  g.poly(around(64, R)).stroke({ width: 2, color: 0x24403c, join: 'round' });
  // Grass at the water's edge here and there.
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    footGrass(g, Math.cos(a) * R(a) * 1.08, Math.sin(a) * R(a) * 1.08, r * 0.09, rnd);
  }
  if (v === 0) {
    for (let i = 0; i < 6; i++) {
      const a = p1 + i * 1.1;
      const d = R(a) * (0.35 + 0.4 * rnd());
      pad(Math.cos(a) * d, Math.sin(a) * d, r * (0.06 + 0.03 * rnd()), i < 2 ? 0xe89ab0 : null);
    }
    for (let i = 0; i < 3; i++) {
      const a = p2 + i * 0.5;
      stone(g, Math.cos(a) * R(a) * 1.1, Math.sin(a) * R(a) * 1.1, r * (0.07 + 0.03 * rnd()), STONE[i % 2], rnd, 7);
    }
  } else if (v === 1) {
    // Reeds crowding one side, cattails standing out of them.
    const greens = [0x5e8a48, 0x7aa456, 0x9cc070];
    for (let i = 0; i < 5; i++) {
      const a = p1 + (i - 2) * 0.32;
      const [x, y] = [Math.cos(a) * R(a) * 0.98, Math.sin(a) * R(a) * 0.98];
      const rr = r * (0.2 + 0.05 * rnd());
      const tips = boughs(12, rnd).map(([t, k]): [number, number] => [t, k * (0.65 + 0.35 * rnd())]);
      whorl(g, x, y, rr, tips, 0.35);
      g.fill(greens[0]).stroke({ width: 1.5, color: shade(greens[0], 0.45), join: 'round' });
      whorl(g, x - rr * 0.08, y - rr * 0.08, rr * 0.7, tips, 0.35);
      g.fill(greens[1]);
      whorl(g, x - rr * 0.14, y - rr * 0.14, rr * 0.38, boughs(8, rnd), 0.4);
      g.fill(greens[2]);
      for (let k = 0; k < 2; k++) {
        const ca = rnd() * Math.PI * 2;
        const [cx, cy] = [x + Math.cos(ca) * rr * 0.45, y + Math.sin(ca) * rr * 0.45];
        g.poly(oval(cx, cy, r * 0.05, r * 0.024, ca)).fill(0x6e4428).stroke({ width: 1.5, color: 0x3a2414 });
        g.poly(oval(cx - 1, cy - 1, r * 0.03, r * 0.01, ca)).fill(0x9a6a40);
      }
    }
    for (let i = 0; i < 3; i++) {
      const a = p1 + Math.PI + (i - 1) * 0.7;
      const d = R(a) * 0.55;
      pad(Math.cos(a) * d, Math.sin(a) * d, r * 0.07, null);
    }
  } else {
    for (let i = 0; i < 3; i++) {
      const a = p1 + i * 2.1;
      const d = R(a) * (0.3 + 0.2 * i);
      duck(g, Math.cos(a) * d, Math.sin(a) * d, rnd() * Math.PI * 2, r * 0.07, i === 1);
    }
    const fa = p2 + 1;
    leaf(g, Math.cos(fa) * R(fa) * 1.035, Math.sin(fa) * R(fa) * 1.035, fa + Math.PI / 2, r * 0.1, r * 0.03, 0xf2efe6);
    stone(g, Math.cos(p2) * R(p2) * 1.08, Math.sin(p2) * R(p2) * 1.08, r * 0.08, STONE[2], rnd, 7);
  }
}

/** A plank from above, along x from `x0` to `x1` and `y0` to `y1`: inked, lit along its north edge, a line
 * of grain. */
function plank(g: Graphics, x0: number, y0: number, x1: number, y1: number, tones: readonly number[], rnd: () => number) {
  g.rect(x0, y0, x1 - x0, y1 - y0).fill(tones[1]).stroke({ width: 2, color: INK, join: 'round' });
  g.rect(x0 + 1, y0 + 1, x1 - x0 - 2, Math.min(3, (y1 - y0) * 0.3)).fill(tones[2]);
  const y = y0 + (y1 - y0) * (0.55 + 0.2 * rnd());
  g.moveTo(x0 + (x1 - x0) * 0.12, y).lineTo(x0 + (x1 - x0) * (0.45 + 0.2 * rnd()), y).stroke({ width: 1, color: tones[0], alpha: 0.7, cap: 'round' });
}

/** Bench, along x, its front toward +y: two planks on timber trestles (v0); a park bench with a slatted back
 * and iron scroll ends (v1); a stone slab on two blocks with moss on it (v2); a split log on two stumps
 * (v3). */
function bench(g: Graphics, r: number, rnd: () => number, v: number) {
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
function flower5(g: Graphics, x: number, y: number, s: number, petal: number, eye: number, ph: number) {
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
function flowerbed(g: Graphics, r: number, rnd: () => number, v: number) {
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
function fruitTree(g: Graphics, r: number, rnd: () => number, v: number) {
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
function waterWheel(g: Graphics, r: number) {
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
 * out, a darker foot along the south. Clipped box (v0), dark yew (v1), hawthorn in flower (v2), overgrown
 * with shoots sticking out (v3). */
function hedge(g: Graphics, r: number, rnd: () => number, v: number) {
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
  g.moveTo(-L + 4, W - 2.5).lineTo(L - 4, W - 2.5).stroke({ width: 2.4, color: shade(tones[0], 0.7), alpha: 0.9, cap: 'round' });
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
      const side = i % 3 === 1 ? 1 : -1;
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
function clippedBall(g: Graphics, x: number, y: number, s: number, tones: readonly number[], rnd: () => number) {
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
function topiary(g: Graphics, r: number, rnd: () => number, v: number) {
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

/** A roof from above over the polygon `pts` (round (0, 0)): a plane to each side up to the peak, lit as it
 * faces the light, courses of shingles lapped along it, hips from the peak to the corners. Planes left out
 * (`gone`) show the timbers under them instead: rafters down the hips, purlins across. */
function hipRoof(g: Graphics, pts: number[][], tone: number, gone: Set<number> = new Set()) {
  const n = pts.length;
  for (let i = 0; i < n; i++) {
    if (gone.has(i)) continue;
    const [a, b] = [pts[i], pts[(i + 1) % n]];
    const mid = Math.atan2(a[1] + b[1], a[0] + b[0]);
    const t = shade(tone, 0.84 + 0.28 * lit(mid));
    g.poly([0, 0, a[0], a[1], b[0], b[1]]).fill(t);
    [0.38, 0.56, 0.74, 0.92].forEach((f, row) => {
      const [x0, y0, x1, y1] = [a[0] * f, a[1] * f, b[0] * f, b[1] * f];
      g.moveTo(x0, y0).lineTo(x1, y1);
      // Shingles: short joints up from each course toward the peak, staggered row by row.
      const k = Math.max(2, Math.round(Math.hypot(x1 - x0, y1 - y0) / 9));
      for (let s = 0; s < k; s++) {
        const u = (s + 0.25 + (row % 2) * 0.5) / k;
        const [x, y] = [x0 + (x1 - x0) * u, y0 + (y1 - y0) * u];
        g.moveTo(x, y).lineTo((x * (f - 0.15)) / f, (y * (f - 0.15)) / f);
      }
    });
    g.stroke({ width: 1.1, color: shade(t, 0.7), cap: 'round' });
  }
  for (const i of gone) {
    const [a, b] = [pts[i], pts[(i + 1) % n]];
    inked(g, () => [0.45, 0.75].forEach((f) => g.moveTo(a[0] * f, a[1] * f).lineTo(b[0] * f, b[1] * f)), 1.8, WOOD[1]);
  }
  if (gone.size) {
    inked(g, () => pts.forEach(([x, y]) => g.moveTo(0, 0).lineTo(x, y)), 2.2, WOOD[1]);
  } else {
    for (const [x, y] of pts) g.moveTo(0, 0).lineTo(x, y);
    g.stroke({ width: 2.4, color: shade(tone, 0.55) });
  }
  for (let i = 0; i < n; i++) {
    if (gone.has(i)) continue;
    const [a, b] = [pts[i], pts[(i + 1) % n]];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
  }
  g.stroke({ width: 2.5, color: INK, cap: 'round' });
}

/** A finial at (x, y): a gilt ball, lit on the NW. */
function finial(g: Graphics, x: number, y: number, s: number) {
  g.circle(x, y, s).fill(0xc9a24a).stroke({ width: 1.8, color: INK });
  g.circle(x - s * 0.3, y - s * 0.3, s * 0.4).fill(0xfff1c0);
}

/** A dome from above (radius `R`): its east side in shade, a crescent of light on its NW shoulder, eight
 * straight ribs from the crown to the eaves, a painted cornice round its foot, a lantern on top (a drum
 * with windows, its cap, a gilt finial). */
function dome(g: Graphics, R: number, tones: readonly number[], cornice: number) {
  g.circle(0, 0, R + 3).fill(INK);
  g.circle(0, 0, R + 1).fill(cornice);
  g.circle(0, 0, R * 0.92).fill(tones[0]);
  g.circle(-R * 0.1, -R * 0.1, R * 0.8).fill(tones[1]);
  g.circle(-R * 0.18, -R * 0.18, R * 0.68).fill(tones[2]);
  g.circle(-R * 0.04, -R * 0.04, R * 0.62).fill(tones[1]);
  for (let i = 0; i < 8; i++) {
    const a = (i / 8) * Math.PI * 2 + Math.PI / 8;
    g.moveTo(Math.cos(a) * R * 0.26, Math.sin(a) * R * 0.26).lineTo(Math.cos(a) * R * 0.9, Math.sin(a) * R * 0.9);
  }
  g.stroke({ width: 2.2, color: shade(tones[0], 0.72), cap: 'round' });
  g.circle(0, 0, R * 0.92).stroke({ width: 1.5, color: INK });
  g.circle(R * 0.03, R * 0.04, R * 0.15).fill({ color: 0x0e1a14, alpha: 0.3 });
  g.circle(0, 0, R * 0.13).fill(cornice).stroke({ width: 1.6, color: INK });
  g.circle(0, 0, R * 0.075).fill(tones[1]).stroke({ width: 1.2, color: INK });
  finial(g, 0, 0, R * 0.04);
}

/** Steps up to a platform at its south edge (`y0`): treads `w` px wide, each a little wider and lower. */
function stepsDown(g: Graphics, y0: number, w: number, n: number, depth: number, tones: readonly number[]) {
  for (let k = n - 1; k >= 0; k--) {
    const hw = w / 2 + k * 3;
    g.rect(-hw, y0 - 4, hw * 2, depth * (k + 1) + 4).fill(shade(tones[1], 1 - k * 0.06)).stroke({ width: 1.8, color: INK, join: 'round' });
    g.moveTo(-hw + 1, y0 + depth * k + 1).lineTo(hw - 1, y0 + depth * k + 1).stroke({ width: 1.5, color: tones[2] });
  }
}

/** Gazebo, from above: an octagonal shingled roof with a gilt finial over steps at its south side (v0); a
 * white open pavilion, its balustrade round a painted floor under a copper dome gone green (v1); a pergola,
 * timber beams and rafters with climbing roses over a paved floor and a bench (v2); an old one fallen in,
 * half its roof gone to the rafters over the boards, ivy on it (v3). */
function gazebo(g: Graphics, r: number, rnd: () => number, v: number) {
  const oct = (R: number) => ngon(R, 8, Math.PI / 8);
  if (v === 2) {
    const S = r * 0.82;
    const st = STONE[1];
    g.rect(-S, -S, S * 2, S * 2).fill(st[2]).stroke({ width: 2, color: INK });
    for (let k = 1; k < 5; k++) g.moveTo(-S, -S + (S * 2 * k) / 5).lineTo(S, -S + (S * 2 * k) / 5);
    for (let k = 0; k < 5; k++) for (let i = 1; i < 4; i++) g.moveTo(-S + (S * 2 * (i + (k % 2) * 0.5)) / 4, -S + (S * 2 * k) / 5).lineTo(-S + (S * 2 * (i + (k % 2) * 0.5)) / 4, -S + (S * 2 * (k + 1)) / 5);
    g.stroke({ width: 1.2, color: shade(st[0], 0.9) });
    bench(g, r, rnd, 0);
    // The timbers' shadows on the floor, then the posts, the two beams and the rafters across them.
    const [B, rafters] = [r * 0.68, 8];
    const xs = Array.from({ length: rafters }, (_, i) => -r * 0.72 + (i * r * 1.44) / (rafters - 1));
    for (const x of xs) g.moveTo(x + 6, -r * 0.82 + 6).lineTo(x + 6, r * 0.82 + 6);
    for (const y of [-B, B]) g.moveTo(-r * 0.86 + 6, y + 6).lineTo(r * 0.86 + 6, y + 6);
    g.stroke({ width: 5, color: 0x2a1a10, alpha: 0.22 });
    for (const [x, y] of [
      [-B, -B],
      [B, -B],
      [B, B],
      [-B, B],
    ]) {
      bevel(g, x - 6, y - 6, x + 6, y + 6, 2, WOOD, 2);
    }
    for (const y of [-B, B]) plank(g, -r * 0.86, y - 4.5, r * 0.86, y + 4.5, WOOD, rnd);
    for (const x of xs) {
      g.rect(x - 3, -r * 0.82, 6, r * 1.64).fill(WOOD[2]).stroke({ width: 1.8, color: INK, join: 'round' });
      g.moveTo(x - 1.5, -r * 0.8).lineTo(x - 1.5, r * 0.8).stroke({ width: 1, color: WOOD[3], alpha: 0.7 });
    }
    // Climbing roses over two corners and along a beam: leafy clumps (as a canopy), roses in them.
    for (const [x, y, s] of [
      [-B, -B, r * 0.26],
      [B, B, r * 0.3],
      [r * 0.15, -B, r * 0.2],
    ]) {
      const parts: [number, number, number][] = [[x, y, s * 0.5]];
      for (let i = 0; i < 6; i++) {
        const a = (i / 6) * Math.PI * 2 + rnd() * 0.6;
        const d = s * (0.35 + 0.2 * rnd());
        parts.push([x + Math.cos(a) * d, y + Math.sin(a) * d, s * (0.36 + 0.12 * rnd())]);
      }
      for (const [px, py, pr] of parts) g.circle(px, py, pr + 2).fill(INK);
      for (const [px, py, pr] of parts) g.circle(px, py, pr).fill(0x2e4a24);
      for (const [px, py, pr] of parts) g.circle(px - pr * 0.18, py - pr * 0.18, pr * 0.76).fill(0x46682e);
      for (const [px, py, pr] of parts) g.circle(px - pr * 0.34, py - pr * 0.34, pr * 0.38).fill({ color: 0x6a8c3e, alpha: 0.9 });
      for (let i = 0; i < 5; i++) {
        const a = rnd() * Math.PI * 2;
        const d = s * 0.7 * Math.sqrt(rnd());
        flower5(g, x + Math.cos(a) * d, y + Math.sin(a) * d, 4.2, i % 2 ? 0xe88aa0 : 0xc8323a, 0xf2c94c, rnd());
      }
    }
    return;
  }
  if (v === 1) {
    // Hexagonal floor, its balustrade, posts at the corners, the dome over the middle.
    const hex = (R: number) => ngon(R, 6, 0);
    stepsDown(g, r * 0.76, r * 0.42, 2, r * 0.09, [0xc8c2b4, 0xe6e0d2, 0xf8f4ea]);
    const floor = hex(r * 0.9);
    g.poly(floor.flat()).fill(0xd6cfc0).stroke({ width: 2.5, color: INK, join: 'round' });
    for (let k = -6; k <= 6; k++) {
      const w = r * 0.9 - (Math.abs(k) * r * 0.12) / Math.sqrt(3) - 1;
      g.moveTo(-w, k * r * 0.12).lineTo(w, k * r * 0.12);
    }
    g.stroke({ width: 1, color: 0xb4ac9c });
    g.poly(floor.flat()).stroke({ width: 2.5, color: INK, join: 'round' });
    const rail = hex(r * 0.82);
    for (let i = 0; i < 6; i++) {
      // (The south side, corner 1 to 2, is open for the steps.)
      if (i === 1) continue;
      const [a, b] = [rail[i], rail[(i + 1) % 6]];
      for (let k = 1; k < 7; k++) {
        const [x, y] = [a[0] + ((b[0] - a[0]) * k) / 7, a[1] + ((b[1] - a[1]) * k) / 7];
        g.circle(x, y, 2.2).fill(0xf6f2e8).stroke({ width: 1, color: INK });
      }
    }
    // The rail round, open at the steps (the side facing south, from corner 1 to corner 2).
    const open = [...rail.slice(2), ...rail.slice(0, 2)].flat();
    inked(g, () => g.poly(open, false), 3.6, 0xf6f2e8);
    for (const [x, y] of rail) g.rect(x - 4, y - 4, 8, 8).fill(0xf6f2e8).stroke({ width: 1.5, color: INK });
    g.circle(r * 0.08, r * 0.1, r * 0.62).fill({ color: 0x1a2a30, alpha: 0.25 });
    dome(g, r * 0.6, [0x2e6a58, 0x4e8e76, 0x8ac4a8], 0xf2eee4);
    return;
  }
  // v0 and v3: octagonal, on a timber floor with steps at the south side.
  stepsDown(g, r * 0.74, r * 0.4, 2, r * 0.1, STONE[0]);
  if (v === 3) {
    const floor = oct(r * 0.86);
    g.poly(floor.flat()).fill(WOOD[1]).stroke({ width: 2.5, color: INK, join: 'round' });
    const apo = r * 0.86 * Math.cos(Math.PI / 8);
    for (let k = -6; k <= 6; k++) {
      const w = Math.min(apo, apo * Math.SQRT2 - Math.abs(k * r * 0.12)) - 1;
      g.moveTo(-w, k * r * 0.12).lineTo(w, k * r * 0.12);
    }
    g.stroke({ width: 1, color: WOOD[0] });
    g.poly(floor.flat()).stroke({ width: 2.5, color: INK, join: 'round' });
    // Fallen shingles and a broken rafter on the boards.
    for (let i = 0; i < 6; i++) {
      const [x, y] = [r * (0.1 + 0.4 * rnd()), r * (-0.2 + 0.6 * rnd())];
      const a = rnd() * Math.PI;
      g.poly(oval(x, y, 4.5, 2.6, a, 6)).fill(0x6a4a30).stroke({ width: 1, color: INK });
    }
    rod(g, [r * 0.15, r * 0.5], [r * 0.6, r * 0.2], 3, WOOD[1]);
  }
  hipRoof(g, oct(r * 0.88), v === 3 ? 0x6e5a42 : 0x7e5a3a, v === 3 ? new Set([0, 1, 2, 7]) : new Set());
  if (v === 3) {
    // Ivy over the broken edge, moss on the planes left.
    for (const [x, y, s] of [
      [r * 0.62, -r * 0.18, r * 0.16],
      [-r * 0.1, r * 0.58, r * 0.14],
      [-r * 0.5, -r * 0.4, r * 0.12],
    ]) {
      g.poly(blob(x, y, s, 9, 0.5, rnd)).fill(0x34502a).stroke({ width: 1.5, color: INK, join: 'round' });
      for (let i = 0; i < 5; i++) {
        const a = rnd() * Math.PI * 2;
        g.circle(x + Math.cos(a) * s * 0.5, y + Math.sin(a) * s * 0.5, s * 0.22).fill(0x5a7e36);
      }
    }
    g.circle(0, 0, r * 0.06).fill(WOOD[0]).stroke({ width: 1.5, color: INK });
    return;
  }
  finial(g, 0, 0, r * 0.07);
}

/** Bandstand, from above: a raised octagonal stage on a stone base, a railing of balusters round it open at
 * the steps on the south side; a red roof over it (v0); open to the sky, chairs in a half ring with music
 * stands, a big drum, the conductor's box, bunting on the rail (v1); a green ogee dome on iron with a gilt
 * finial and pennant (v2); a canopy of striped canvas, scalloped round its edge (v3). */
function bandstand(g: Graphics, r: number, rnd: () => number, v: number) {
  const ph = Math.PI / 8;
  const octPts = (R: number) => ngon(R, 8, ph).flat();
  const st = STONE[0];
  stepsDown(g, r * 0.8, r * 0.36, 3, r * 0.06, st);
  rimBand(g, octPts(r * 0.9), octPts(r * 0.8), st[2], 2.5);
  // The deck: boards along x, clipped to the octagon by hand.
  const deck = octPts(r * 0.8);
  g.poly(deck).fill(WOOD[2]);
  const apo = r * 0.8 * Math.cos(Math.PI / 8);
  for (let y = -apo + 7; y < apo; y += 7) {
    const w = Math.min(apo, apo * Math.SQRT2 - Math.abs(y)) - 1;
    g.moveTo(-w, y).lineTo(w, y);
  }
  g.stroke({ width: 1, color: WOOD[1] });
  g.poly(deck).stroke({ width: 1.8, color: INK, join: 'round' });
  const iron = v === 2;
  const railTone = iron ? IRON[1] : 0xf2eee4;
  const rail = octPts(r * 0.74);
  const pts: [number, number][] = Array.from({ length: 8 }, (_, i) => [rail[i * 2], rail[i * 2 + 1]]);
  // The south side (corners 1 to 2) is open for the steps. (Under a canopy's valance the rail is hidden.)
  for (let i = 0; i < 8; i++) {
    if (i === 1 || v === 3) continue;
    const [a, b] = [pts[i], pts[(i + 1) % 8]];
    for (let k = 1; k < 8; k++) {
      const [x, y] = [a[0] + ((b[0] - a[0]) * k) / 8, a[1] + ((b[1] - a[1]) * k) / 8];
      g.circle(x, y, 2).fill(railTone).stroke({ width: 1, color: INK });
    }
  }
  const run = [...pts.slice(2), ...pts.slice(0, 2)].flat();
  if (v !== 3) {
    inked(g, () => g.poly(run, false), 3.6, railTone);
    for (const [x, y] of pts) g.rect(x - 4.5, y - 4.5, 9, 9).fill(railTone).stroke({ width: 1.5, color: INK });
  }
  if (v === 1) {
    // Bunting along the rail on the north side.
    for (const i of [4, 5, 6]) {
      const [a, b] = [pts[i], pts[(i + 1) % 8]];
      for (let k = 0; k < 4; k++) {
        const [t0, t1] = [k / 4 + 0.03, (k + 1) / 4 - 0.03];
        const [x0, y0, x1, y1] = [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0, a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1];
        const [mx, my] = [(x0 + x1) / 2, (y0 + y1) / 2];
        const [nx, ny] = [-mx / Math.hypot(mx, my), -my / Math.hypot(mx, my)];
        g.poly([x0, y0, x1, y1, mx + nx * 9, my + ny * 9]).fill([0xc8323a, 0xf2c94c, 0x3a6ab0, 0xf6efe0][(k + i) % 4]).stroke({ width: 1.2, color: INK, join: 'round' });
      }
    }
    // Chairs in a half ring facing the conductor's box at the front, a stand before each, the big drum
    // at the back.
    for (let i = 0; i < 7; i++) {
      const a = Math.PI * 1.1 + (i / 6) * Math.PI * 0.8;
      const [x, y] = [Math.cos(a) * r * 0.48, Math.sin(a) * r * 0.48];
      const [c, s] = [Math.cos(a), Math.sin(a)];
      const at = (u: number, w: number) => [x + c * u - s * w, y + s * u + c * w];
      g.poly([...at(-6, -6), ...at(6, -6), ...at(6, 6), ...at(-6, 6)]).fill(WOOD[1]).stroke({ width: 1.5, color: INK, join: 'round' });
      g.poly([...at(5, -7), ...at(9, -7), ...at(9, 7), ...at(5, 7)]).fill(WOOD[0]).stroke({ width: 1.5, color: INK, join: 'round' });
      const [sx, sy] = [Math.cos(a) * r * 0.32, Math.sin(a) * r * 0.32];
      // A stand: its iron desk across, a sheet of music open on it.
      rod(g, [sx - s * 7, sy + c * 7], [sx + s * 7, sy - c * 7], 3, IRON[1]);
      g.moveTo(sx - s * 4.5, sy + c * 4.5).lineTo(sx + s * 4.5, sy - c * 4.5).stroke({ width: 2.6, color: 0xf6efe0 });
    }
    g.circle(r * 0.03, -r * 0.47, r * 0.15).fill({ color: 0x1a1008, alpha: 0.3 });
    g.circle(0, -r * 0.5, r * 0.14).fill(0x8a2e26).stroke({ width: 2, color: INK });
    g.circle(0, -r * 0.5, r * 0.11).fill(0xeee4cc).stroke({ width: 1.2, color: INK });
    g.circle(-r * 0.03, -r * 0.53, r * 0.04).fill({ color: 0xffffff, alpha: 0.6 });
    bevel(g, -r * 0.09, r * 0.14, r * 0.09, r * 0.3, 2, WOOD, 2);
    return;
  }
  g.circle(r * 0.1, r * 0.12, r * 0.66).fill({ color: 0x1a2a30, alpha: 0.25 });
  if (v === 0) {
    hipRoof(g, ngon(r * 0.66, 8, ph), 0xa04a36);
    finial(g, 0, 0, r * 0.06);
    rod(g, [-r * 0.12, -r * 0.02], [r * 0.12, -r * 0.02], 1.6);
    g.poly([r * 0.12, -r * 0.02, r * 0.07, -r * 0.06, r * 0.07, r * 0.02]).fill(IRON[2]).stroke({ width: 1, color: INK });
  } else if (v === 2) {
    dome(g, r * 0.54, [0x2e6a58, 0x4e8e76, 0x8ac4a8], 0x2e3a3a);
  } else {
    // A canvas canopy in eight broad stripes on its posts, its valance hanging out past the rim, a small
    // finial at the peak.
    stripedCanopy(g, r * 0.68, 8, 0xe2d6b4, 0xa8382e, r * 0.06, Math.PI / 8);
    finial(g, 0, 0, r * 0.05);
  }
}

/** A square block from a high angle (`P`), `w` ft each side of its middle, from `h0` to `h1` ft up: its
 * front face and its top, inked. Turned corner-on (`diamond`): two faces, the lit west one and the shaded east. */
function block(g: Graphics, P: (x: number, d: number, h: number) => [number, number], w: number, h0: number, h1: number, tones: readonly number[], diamond = false) {
  const ink = { width: 2, color: INK, join: 'round' as const };
  if (diamond) {
    g.poly(quad4(P(-w, 0, h0), P(0, w, h0), P(0, w, h1), P(-w, 0, h1))).fill(tones[1]).stroke(ink);
    g.poly(quad4(P(0, w, h0), P(w, 0, h0), P(w, 0, h1), P(0, w, h1))).fill(tones[0]).stroke(ink);
    g.poly(quad4(P(-w, 0, h1), P(0, -w, h1), P(w, 0, h1), P(0, w, h1))).fill(tones[2]).stroke(ink);
    return;
  }
  g.poly(quad4(P(-w, w, h0), P(w, w, h0), P(w, w, h1), P(-w, w, h1))).fill(tones[1]).stroke(ink);
  g.poly(quad4(P(-w, -w, h1), P(w, -w, h1), P(w, w, h1), P(-w, w, h1))).fill(tones[2]).stroke(ink);
}

/** Garden statue, from a high angle (a figure only reads from the side) on a moulded pedestal: a robed
 * figure in marble holding up a wreath (v0); a stone urn spilling flowers and ivy (v1); a warrior in bronze
 * gone green, spear and round shield (v2); an obelisk on stepped plinths (v3). */
function gardenStatue(g: Graphics, r: number, rnd: () => number, v: number) {
  // A little over life size, as statues set up to be seen are.
  const P0 = upright(UPRIGHT_FOOT[60]);
  const K = 1.3;
  const P = (x: number, d: number, h: number) => P0(x * K, d * K, h * K);
  const st = STONE[0];
  footGrass(g, ...P(-1.5, 0.9, 0), r * 0.2, rnd);
  footGrass(g, ...P(1.4, 1.2, 0), r * 0.16, rnd);
  if (v === 3) {
    block(g, P, 1.55, 0, 0.6, st, true);
    block(g, P, 1.15, 0.6, 1.3, st.map((c) => shade(c, 1.05)), true);
    // The shaft, corner-on and tapering: the lit west face, the shaded east one, carved signs down the
    // west; then the pyramid on top, all four faces seen from above.
    const [s0, s1, h0, h1, apex] = [0.72, 0.42, 1.3, 9.4, 10.6];
    const ink = { width: 2, color: INK, join: 'round' as const };
    g.poly(quad4(P(-s0, 0, h0), P(0, s0, h0), P(0, s1, h1), P(-s1, 0, h1))).fill(st[2]).stroke(ink);
    g.poly(quad4(P(0, s0, h0), P(s0, 0, h0), P(s1, 0, h1), P(0, s1, h1))).fill(st[1]).stroke(ink);
    for (let k = 0; k < 6; k++) {
      const h = h0 + 1 + k * 1.2;
      const f = 1 - (h - h0) / (h1 - h0);
      const s = s1 + (s0 - s1) * f;
      g.moveTo(...P(-s * 0.62, s * 0.38, h)).lineTo(...P(-s * 0.38, s * 0.62, h - 0.15));
    }
    g.stroke({ width: 1.6, color: st[0], cap: 'round' });
    const top = P(0, 0, apex);
    const faces: [[number, number], [number, number], number][] = [
      [P(-s1, 0, h1), P(0, -s1, h1), st[3]],
      [P(0, -s1, h1), P(s1, 0, h1), st[2]],
      [P(-s1, 0, h1), P(0, s1, h1), st[2]],
      [P(0, s1, h1), P(s1, 0, h1), st[0]],
    ];
    for (const [a, b, c] of faces) g.poly([...a, ...b, ...top]).fill(c).stroke(ink);
    moss(g, ...P(-1.1, 0.4, 0.6), 0.3 * FT * K, rnd);
    return;
  }
  // The pedestal: a base, the die, a cornice.
  block(g, P, 1.3, 0, 0.5, st);
  block(g, P, 1.0, 0.5, 2.7, st.map((c) => shade(c, 1.04)));
  g.poly(quad4(P(-0.7, 1.0, 1.0), P(0.7, 1.0, 1.0), P(0.7, 1.0, 2.2), P(-0.7, 1.0, 2.2))).stroke({ width: 1.2, color: st[0] });
  block(g, P, 1.22, 2.7, 3.1, st.map((c) => shade(c, 1.08)));
  if (v === 1) {
    // The urn: a profile turned round its axis (foot, stem, swelling body, neck, lip), handles, the earth
    // in its mouth and the flowers and ivy spilling over.
    const prof: [number, number][] = [[0.5, 3.1], [0.5, 3.35], [0.22, 3.55], [0.22, 3.85], [0.72, 4.3], [0.92, 4.9], [0.8, 5.5], [0.55, 5.85], [0.84, 6.1]];
    const out = [...prof.map(([x, h]) => P(x, 0, h)), ...prof.reverse().map(([x, h]) => P(-x, 0, h))].flat();
    const tones = st.map((c) => shade(c, 1.1));
    for (const side of [-1, 1]) {
      inked(g, () => g.moveTo(...P(side * 0.85, 0, 5.3)).quadraticCurveTo(...P(side * 1.35, 0, 5.4), ...P(side * 0.8, 0, 4.6)), 2.5, tones[1]);
    }
    g.poly(out).fill(tones[1]).stroke({ width: 2, color: INK, join: 'round' });
    g.poly([...P(-0.6, 0, 4.4), ...P(-0.8, 0, 4.9), ...P(-0.7, 0, 5.4), ...P(-0.35, 0, 5.4), ...P(-0.4, 0, 4.6)]).fill({ color: tones[2], alpha: 0.8 });
    g.poly([...P(0.4, 0, 4.5), ...P(0.85, 0, 4.9), ...P(0.75, 0, 5.4), ...P(0.5, 0, 5.4)]).fill({ color: tones[0], alpha: 0.5 });
    g.moveTo(...P(-0.85, 0, 4.9)).lineTo(...P(0.85, 0, 4.9)).stroke({ width: 1.2, color: tones[0] });
    const rim = (k: number) => Array.from({ length: 16 }, (_, i) => P(Math.cos((i / 16) * Math.PI * 2) * 0.84 * k, Math.sin((i / 16) * Math.PI * 2) * 0.84 * k, 6.1)).flat();
    g.poly(rim(1)).fill(tones[2]).stroke({ width: 1.8, color: INK, join: 'round' });
    g.poly(rim(0.78)).fill(0x3e2c1e);
    for (const [x, d] of [
      [-0.45, -0.2],
      [0.35, -0.3],
      [0, 0.25],
      [0.5, 0.2],
    ]) {
      clippedBall(g, ...P(x, d, 6.4), 0.42 * FT * K, [0x2e4a24, 0x46682e, 0x6a8c3e], rnd);
    }
    for (const [x, h] of [
      [-0.75, 5.6],
      [0.8, 5.2],
    ]) {
      for (let k = 0; k < 3; k++) leaf(g, ...P(x + (rnd() - 0.5) * 0.3, 0.3, h - k * 0.5), Math.PI / 2 + (rnd() - 0.5) * 0.8, 0.3 * FT * K, 2.2, 0x46682e);
    }
    for (let k = 0; k < 4; k++) flower5(g, ...P(-0.5 + k * 0.35, (k % 2) * 0.25 - 0.1, 6.7), 4, k % 2 ? 0xc8323a : 0xe88aa0, 0xf2c94c, rnd());
    return;
  }
  // A figure: a robe or tunic to the shoulders, its folds, the head; arms.
  const tones = v === 0 ? [0xb8b4ac, 0xdcd8d0, 0xf4f2ec] : [0x2e5e50, 0x4e8a74, 0x7ab49a];
  const ink = { width: 2, color: INK, join: 'round' as const };
  const [h0, sh, head] = [3.1, 7.0, 7.7];
  g.poly(oval(...P(0, 0, h0), 0.85 * FT * K, 0.85 * FT * K * 0.87 * 0.5, 0, 14)).fill(tones[0]).stroke(ink);
  if (v === 2) {
    // The spear behind, upright in the right hand.
    rod(g, P(0.8, 0, h0 + 0.2), P(0.8, 0, 10.4), 2, tones[1]);
    g.poly([...P(0.8, 0, 11.1), ...P(0.62, 0, 10.4), ...P(0.8, 0, 10.1), ...P(0.98, 0, 10.4)]).fill(tones[2]).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  const body = [P(-0.7, 0, h0), P(0.7, 0, h0), P(0.62, 0, 5.2), P(0.58, 0, 6.6), P(0.42, 0, sh), P(-0.42, 0, sh), P(-0.58, 0, 6.6), P(-0.62, 0, 5.2)].flat();
  g.poly(body).fill(tones[1]).stroke(ink);
  g.poly([...P(0.05, 0, h0), ...P(0.7, 0, h0), ...P(0.62, 0, 5.2), ...P(0.58, 0, 6.6), ...P(0.3, 0, sh), ...P(0.1, 0, 5.4)]).fill({ color: tones[0], alpha: 0.55 });
  for (const x of [-0.35, 0, 0.32]) g.moveTo(...P(x * 0.6, 0, 5.6)).quadraticCurveTo(...P(x + 0.08, 0, 4.4), ...P(x, 0, h0 + 0.1));
  g.stroke({ width: 1.3, color: shade(tones[0], 0.85), cap: 'round' });
  if (v === 2) g.moveTo(...P(-0.6, 0, 5.3)).lineTo(...P(0.6, 0, 5.3)).stroke({ width: 2.2, color: tones[0] });
  // The head (a crested helm on the warrior), lit on the NW.
  const [hx, hy] = P(0, 0, head);
  g.circle(hx, hy, 0.48 * FT * K).fill(tones[1]).stroke(ink);
  g.circle(hx - 1.5, hy - 1.8, 0.24 * FT * K).fill(tones[2]);
  if (v === 2) {
    inked(g, () => g.moveTo(...P(-0.35, 0, head + 0.65)).quadraticCurveTo(...P(0.1, 0, head + 1.4), ...P(0.55, 0, head + 0.5)), 2.6, 0xa8382e);
    // The shield on the left arm, its boss.
    const [sx, sy] = P(-0.55, 0.35, 5.2);
    g.poly(oval(sx, sy, 0.8 * FT * K, 0.8 * FT * K * 0.92, 0, 18)).fill(tones[1]).stroke(ink);
    g.poly(oval(sx, sy, 0.6 * FT * K, 0.6 * FT * K * 0.92, 0, 18)).stroke({ width: 1.5, color: tones[0] });
    g.circle(sx, sy, 0.2 * FT * K).fill(tones[2]).stroke({ width: 1.2, color: INK });
    rod(g, P(0.42, 0, 6.8), P(0.8, 0, 5.6), 3, tones[1]);
    for (let k = 0; k < 3; k++) g.circle(...P(-0.2 + k * 0.3, 0.05, 4 + k * 0.9), 2.2).fill({ color: 0x8ac4a8, alpha: 0.7 });
    return;
  }
  // Both arms raised, holding a laurel wreath up over the head: a thick ring of bronze leaves.
  for (const side of [-1, 1]) inked(g, () => g.moveTo(...P(side * 0.42, 0, 6.8)).quadraticCurveTo(...P(side * 0.75, 0, 8.0), ...P(side * 0.5, 0, 9.2)), 3, tones[1]);
  const [wx, wy] = P(0, 0, 9.7);
  const wr = 0.62 * FT * K;
  const laurel = [0x3e6040, 0x5e8a54, 0x8ab478];
  inked(g, () => g.circle(wx, wy, wr), 4, laurel[0]);
  for (let i = 0; i < 16; i++) {
    const a = (i / 16) * Math.PI * 2;
    const k = i % 2 ? 1.18 : 0.82;
    leaf(g, wx + Math.cos(a) * wr * k, wy + Math.sin(a) * wr * k * 0.95, a + Math.PI / 2 + (i % 2 ? 0.5 : -0.5), 6.5, 2.4, laurel[1 + (Math.cos(a) + Math.sin(a) < 0 ? 1 : 0)]);
  }
  for (const side of [-1, 1]) g.circle(...P(side * 0.5, 0, 9.25), 2.6).fill(tones[1]).stroke({ width: 1.2, color: INK });
}

/** Footbridge, along x from bank to bank, arching over the water (its shadow is cast with the battlemap's,
 * drawn out along it): the deck lit toward its western end as it rises to the light, in shade toward the
 * eastern. Timber planks with handrails on posts (v0); an old humpback in warm stone, flagged in staggered
 * rows, its parapets of blocks flaring at the ends (v1); natural boards with white-painted rails and lattice
 * (v2); two logs side by side with a rope rail (v3). */
function footbridge(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const [L, W] = [1.5 * PX, (v === 3 ? 0.3 : 0.36) * PX];
  // Light over the deck: brighter toward the west end, darker toward the east, by steps.
  const halves = (y0: number, y1: number, k = 0.2) => {
    const n = 8;
    for (let i = 0; i < n; i++) {
      const [x0, x1] = [-L + 2 + ((L * 2 - 4) * i) / n, -L + 2 + ((L * 2 - 4) * (i + 1)) / n];
      const t = ((i + 0.5) / n) * 2 - 1;
      g.rect(x0, y0, x1 - x0, y1 - y0).fill({ color: t < 0 ? 0xfff1cf : 0x1a1008, alpha: k * Math.abs(t) });
    }
  };
  if (v === 3) {
    for (const [y0, y1] of [
      [-W, -1],
      [1, W],
    ]) {
      const bark = BARK[1];
      g.roundRect(-L, y0, L * 2, y1 - y0, (y1 - y0) / 2).fill(bark[1]).stroke({ width: 2.2, color: INK });
      g.roundRect(-L + 4, y0 + 3, L * 2 - 8, (y1 - y0) * 0.3, 2).fill({ color: bark[2], alpha: 0.7 });
      for (let i = 0; i < 9; i++) {
        const x = -L + 10 + rnd() * (L * 2 - 20);
        const y = y0 + (y1 - y0) * (0.4 + 0.4 * rnd());
        g.moveTo(x, y).lineTo(x + 6 + rnd() * 8, y).stroke({ width: 1.4, color: shade(bark[0], 0.7), cap: 'round' });
      }
      for (const x of [-L + (y1 - y0) * 0.3, L - (y1 - y0) * 0.3]) endGrain(g, x, (y0 + y1) / 2, (y1 - y0) * 0.3, (y1 - y0) * 0.48, bark);
    }
    halves(-W, W);
    // A rope rail on three posts along the north side.
    const posts = [-L * 0.85, 0, L * 0.85];
    for (const x of posts) g.circle(x, -W - 4, 4).fill(WOOD[1]).stroke({ width: 1.8, color: INK });
    for (let i = 0; i < 2; i++) {
      const [a, b] = [posts[i], posts[i + 1]];
      inked(g, () => g.moveTo(a, -W - 4).quadraticCurveTo((a + b) / 2, -W + 6, b, -W - 4), 1.8, 0xc8a870);
    }
    return;
  }
  if (v === 1) {
    const st = [0x7a6448, 0xa88e68, 0xcab48a, 0xe8d6b0];
    // Flags in three rows along the deck, the joints of each row staggered from the next.
    g.rect(-L, -W, L * 2, W * 2).fill(st[2]).stroke({ width: 2, color: INK });
    const [y0, y1] = [-W + 6, W - 6];
    for (let row = 0; row < 3; row++) {
      const [a, b] = [y0 + ((y1 - y0) * row) / 3, y0 + ((y1 - y0) * (row + 1)) / 3];
      if (row) g.moveTo(-L + 2, a).lineTo(L - 2, a + (rnd() - 0.5) * 1.5);
      for (let x = -L + 6 + row * 7 + rnd() * 6; x < L - 6; x += 15 + rnd() * 8) g.moveTo(x, a + 1).lineTo(x + (rnd() - 0.5) * 2, b - 1);
    }
    g.stroke({ width: 1.3, color: shade(st[0], 0.85) });
    for (let i = 0; i < 6; i++) g.rect(-L + 8 + rnd() * (L * 2 - 24), y0 + rnd() * (y1 - y0 - 4), 9, 3).fill({ color: st[3], alpha: 0.5 });
    // The hump: the west half lit as it rises toward the light, the east half in shade, a pale crown line
    // across the top.
    g.rect(-L + 2, -W + 2, L - 2, W * 2 - 4).fill({ color: 0xfff1cf, alpha: 0.22 });
    g.rect(0, -W + 2, L - 2, W * 2 - 4).fill({ color: 0x2a1a10, alpha: 0.24 });
    g.moveTo(0, -W + 3).lineTo(0, W - 3).stroke({ width: 2.2, color: st[3], alpha: 0.9 });
    // The parapets: a low band of blocks of the same stone along each side, splaying out at the ends.
    for (const side of [-1, 1]) {
      const y = side * (W - 3.5);
      const run = (x0: number, y0: number, x1: number, y1: number, n: number) => {
        const [dx, dy] = [(x1 - x0) / n, (y1 - y0) / n];
        const len = Math.hypot(dx, dy);
        const [nx, ny] = [(-dy / len) * 3.5, (dx / len) * 3.5];
        for (let k = 0; k < n; k++) {
          const [ax, ay, bx, by] = [x0 + dx * k, y0 + dy * k, x0 + dx * (k + 1), y0 + dy * (k + 1)];
          const tone = shade(st[1], (k + (side > 0 ? 1 : 0)) % 2 ? 0.94 : 1.04);
          g.poly([ax - nx, ay - ny, bx - nx, by - ny, bx + nx, by + ny, ax + nx, ay + ny]).fill(tone).stroke({ width: 1.5, color: INK, join: 'round' });
          g.moveTo(ax - nx * 0.8, ay - ny * 0.8).lineTo(bx - nx * 0.8, by - ny * 0.8).stroke({ width: 1.2, color: st[3], alpha: 0.8 });
        }
      };
      run(-L - 2, y + side * 8, -L + 14, y, 2);
      run(-L + 14, y, L - 14, y, 10);
      run(L - 14, y, L + 2, y + side * 8, 2);
    }
    moss(g, -L * 0.55, -W, 6, rnd);
    moss(g, L * 0.7, W - 1, 5, rnd);
    return;
  }
  // Timber: planks across, rails on posts along both sides (painted white, with a lattice, on v2).
  const deck = v === 2 ? WOOD.map((c) => shade(c, 1.06)) : WOOD;
  g.rect(-L, -W, L * 2, W * 2).fill(deck[1]).stroke({ width: 2.2, color: INK });
  for (let x = -L + 8; x < L - 2; x += 8) g.moveTo(x, -W + 1).lineTo(x, W - 1);
  g.stroke({ width: 1.2, color: deck[0] });
  for (let i = 0; i < 6; i++) {
    const x = -L + 10 + rnd() * (L * 2 - 20);
    g.circle(x, -W + 6, 0.9).fill(IRON[0]);
    g.circle(x, W - 6, 0.9).fill(IRON[0]);
  }
  halves(-W, W);
  const white = 0xf6f2e8;
  const posts = v === 2 ? [-L + 5, 0, L - 5] : [-L + 4, -L / 3, L / 3, L - 4];
  for (const side of [-1, 1]) {
    const y = side * (W - 4);
    if (v === 2) {
      // The lattice between the posts, inked, then the white rail over it (its ends cut square).
      for (const [width, color] of [
        [3.6, INK],
        [1.6, white],
      ] as const) {
        for (let i = 0; i + 1 < posts.length; i++) {
          const [a, b] = [posts[i], posts[i + 1]];
          for (let k = 0; k < 3; k++) {
            const [x0, x1] = [a + 4 + ((b - a - 8) * k) / 3, a + 4 + ((b - a - 8) * (k + 1)) / 3];
            g.moveTo(x0, y - 5.5).lineTo(x1, y + 5.5).moveTo(x0, y + 5.5).lineTo(x1, y - 5.5);
          }
        }
        g.stroke({ width, color, cap: 'butt' });
      }
    }
    rod(g, [-L + 2, y], [L - 2, y], v === 2 ? 3 : 3.2, v === 2 ? white : WOOD[2]);
    for (const x of posts) {
      const ps = v === 2 ? 5 : 3.5;
      g.rect(x - ps, y - ps, ps * 2, ps * 2).fill(v === 2 ? white : WOOD[0]).stroke({ width: 1.6, color: INK });
      if (v === 2) g.circle(x, y, 2).fill(0xc9a24a).stroke({ width: 1, color: INK });
    }
  }
  for (const x of [-L - 1, L + 1]) {
    for (const y of [-W + 2, W - 2]) stone(g, x, y, 4 + rnd() * 1.5, STONE[1], rnd, 6);
  }
}

// v63 castles: a castle's kitchen garden (rows, a wattle fence, skeps) and its smithy's anvil.

/** Ink widths by size (the sprite style rule): outlines of shapes over half a square, of smaller ones. */
const OUT = 0.045 * PX;
const LINE = 0.025 * PX;

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
function vegRow(g: Graphics, r: number, rnd: () => number, v: number) {
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
function wattleFence(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const wood = v === 1 ? [0x7a5c3a, 0x9a7a50, 0xbc9c6c] : [0x8c6e46, 0xb08e5c, 0xd4b67e];
  const [L, T] = [0.5 * PX, 0.1 * PX];
  g.rect(-L, -T, L * 2, T * 2).fill(wood[1]);
  g.rect(-L, -T + 1.5, L * 2, T * 0.6).fill(wood[2]);
  // The stakes, about every 0.12 of a square, standing proud of the weave either side.
  const n = 8;
  const step = (L * 2) / n;
  for (let i = 0; i < n; i++) {
    const x = -L + (i + 0.5) * step;
    g.moveTo(x, -T - 2.2).lineTo(x, T + 2.2).stroke({ width: 3, color: INK, cap: 'round' });
    g.moveTo(x, -T - 1).lineTo(x, T + 1).stroke({ width: 1.4, color: wood[1], cap: 'round' });
  }
  // The weave: one withy zig-zagging over and under, stake to stake.
  g.moveTo(-L, 0);
  for (let i = 0; i <= n; i++) g.lineTo(Math.min(L, -L + i * step), (i % 2 ? -1 : 1) * T * 0.45);
  g.stroke({ width: 2.2, color: shade(wood[2], 1.12), join: 'round' });
  for (const y of [-T, T]) g.moveTo(-L, y).lineTo(L, y);
  g.stroke({ width: LINE, color: INK });
  if (v === 2) {
    for (let i = 0; i < 4; i++) {
      const x = -L + 6 + rnd() * (L * 2 - 12);
      leaf(g, x, (rnd() < 0.5 ? -1 : 1) * T * 0.7, (rnd() - 0.5) * Math.PI * 2, 7, 3, [0x4e7a34, 0x5e8a40][i % 2], 0.4);
    }
    flower5(g, -L * 0.3 + rnd() * L * 0.6, -T * 0.3, 4.5, 0xf2eee0, 0xd8c060, rnd());
  }
  if (v === 3) {
    const x = -L * 0.4 + rnd() * L * 0.8;
    g.moveTo(x, T * 0.4).quadraticCurveTo(x + 5, T + 4, x + 11, T + 5.5).stroke({ width: 3.6, color: INK, cap: 'round' });
    g.moveTo(x, T * 0.4).quadraticCurveTo(x + 5, T + 4, x + 11, T + 5.5).stroke({ width: 1.6, color: wood[2], cap: 'round' });
  }
}

/** A straw skep from above at (x, y), `R` round: a straw-gold dome, a crescent of shade on the SE, a
 * light on the NW shoulder, two or three coils of its straw rope as arcs, its doorway at the foot
 * (south). */
function skep(g: Graphics, x: number, y: number, R: number, rnd: () => number) {
  const straw = [0xa8884a, 0xc8a660, 0xe2c88c];
  g.circle(x, y, R).fill(straw[0]).stroke({ width: R > 0.25 * PX ? OUT : LINE, color: INK });
  g.circle(x - R * 0.12, y - R * 0.12, R * 0.84).fill(straw[1]);
  g.circle(x - R * 0.36, y - R * 0.36, R * 0.3).fill({ color: straw[2], alpha: 0.9 });
  const coils = R > 0.2 * PX ? [0.72, 0.46, 0.22] : [0.66, 0.34];
  for (const f of coils) {
    const a0 = Math.PI * (0.05 + 0.2 * rnd());
    g.moveTo(x + Math.cos(a0) * R * f, y + Math.sin(a0) * R * f).arc(x, y, R * f, a0, a0 + Math.PI * 1.25).stroke({ width: 1.8, color: 0x7e6034, cap: 'round' });
  }
  g.ellipse(x, y + R * 0.88, R * 0.24, R * 0.14).fill(0x2a1e10);
}

/** Beehives: a straw skep on a plank (v0); two on a longer plank (v1); one on a sawn stump (v2);
 * one bigger, set on the ground (v3). */
function beehive(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v === 1) {
    bevel(g, -r * 0.98, -r * 0.3, r * 0.98, r * 0.38, 3, WOOD, LINE);
    skep(g, -r * 0.47, -r * 0.02, r * 0.46, rnd);
    skep(g, r * 0.47, 0.02 * r, r * 0.44, rnd);
    return;
  }
  if (v === 2) {
    g.poly(oval(0, r * 0.06, r * 0.86, r * 0.78, 0)).fill(BARK[0][1]).stroke({ width: LINE, color: INK });
    endGrain(g, 0, 0, r * 0.76, r * 0.68, BARK[0]);
  } else if (v === 0) {
    bevel(g, -r * 0.82, -r * 0.34, r * 0.82, r * 0.42, 3, WOOD, LINE);
  }
  skep(g, 0, -r * 0.04, r * (v === 3 ? 0.72 : 0.64), rnd);
}

/** An anvil from above on a stump before a smithy, along x, its horn to +x (to -x in v3): dark iron,
 * its four feet showing past a flat face worn bright, a hardy hole at the heel. A small wooden quench
 * tub of dark water beside it (v1, v3), tongs on the stump (v1), a hammer on it (v2). */
function anvil(g: Graphics, r: number, rnd: () => number, v: number) {
  const f = v === 3 ? -1 : 1;
  const X = (x: number) => f * x;
  const flipped = (pts: number[]) => pts.map((p, i) => (i % 2 ? p : X(p)));
  if (v === 1 || v === 3) {
    // The tub: a hooped stave rim, dark water.
    const [tx, ty, T] = [X(0.55 * r), 0.48 * r, 0.26 * r];
    g.circle(tx, ty, T).fill(WOOD[1]).stroke({ width: LINE, color: INK });
    g.circle(tx - T * 0.1, ty - T * 0.1, T * 0.86).stroke({ width: 1.4, color: WOOD[2], alpha: 0.85 });
    g.circle(tx, ty, T * 0.68).fill(0x2a2620).stroke({ width: 1.2, color: INK });
    g.ellipse(tx + T * 0.18, ty + T * 0.2, T * 0.26, T * 0.1).fill({ color: 0x6a6656, alpha: 0.7 });
  }
  // The stump.
  const [sx, sy] = [X(-0.12 * r), -0.04 * r];
  g.poly(oval(sx, sy + r * 0.05, r * 0.5, r * 0.44, 0)).fill(BARK[0][1]).stroke({ width: OUT, color: INK });
  endGrain(g, sx, sy, r * 0.42, r * 0.37, BARK[0]);
  // Its feet: a waisted block under the face, flaring out past it at both ends.
  const [h0, h1] = [-0.52 * r, 0.24 * r];
  const feet = flipped([sx * f + h0 + 2, sy - 0.32 * r, sx * f - 0.14 * r, sy - 0.16 * r, sx * f + h1, sy - 0.32 * r, sx * f + h1, sy + 0.32 * r, sx * f - 0.14 * r, sy + 0.16 * r, sx * f + h0 + 2, sy + 0.32 * r]);
  g.poly(feet).fill(IRON[0]).stroke({ width: LINE, color: INK, join: 'round' });
  // The face and the horn tapering off it.
  const w = 0.2 * r;
  const face = flipped([
    sx * f + h0, sy - w,
    sx * f + h1, sy - w,
    sx * f + h1 + 0.2 * r, sy - w * 0.55,
    sx * f + h1 + 0.4 * r, sy - 0.02 * r,
    sx * f + h1 + 0.2 * r, sy + w * 0.5,
    sx * f + h1, sy + w,
    sx * f + h0, sy + w,
  ]);
  g.poly(face).fill(IRON[1]).stroke({ width: OUT, color: INK, join: 'round' });
  // Lit along its north edge, the face worn bright, the hardy hole.
  g.poly(flipped([sx * f + h0 + 2, sy - w + 2, sx * f + h1 + 0.18 * r, sy - w * 0.5 + 1, sx * f + h1 + 0.36 * r, sy - 0.02 * r, sx * f + h1, sy - w * 0.35, sx * f + h0 + 2, sy - w * 0.35])).fill(IRON[2]);
  const worn = sx * f + h0 + 0.14 * r;
  g.rect(Math.min(X(worn), X(worn + 0.4 * r)), sy - w * 0.25, 0.4 * r, w * 0.55).fill({ color: IRON[3], alpha: 0.75 });
  g.rect(X(sx * f + h0 + 0.08 * r) - 2, sy - 2, 4, 4).fill(INK);
  if (v === 1) {
    // Tongs laid over the stump's edge.
    const [a, b] = [[X(-0.3 * r), 0.36 * r], [X(0.2 * r), 0.42 * r]];
    for (const d of [-1.8, 1.8]) g.moveTo(a[0], a[1] + d).lineTo(b[0], b[1] + d * 0.3);
    g.stroke({ width: 3.4, color: INK, cap: 'round' });
    for (const d of [-1.8, 1.8]) g.moveTo(a[0], a[1] + d).lineTo(b[0], b[1] + d * 0.3);
    g.stroke({ width: 1.5, color: IRON[2], cap: 'round' });
  }
  if (v === 2) {
    // A hammer on the stump, its haft out over the edge.
    const [hx, hy] = [X(-0.32 * r), 0.3 * r];
    g.moveTo(hx, hy).lineTo(hx + X(0.36 * r), hy + 0.14 * r).stroke({ width: 4.4, color: INK, cap: 'round' });
    g.moveTo(hx, hy).lineTo(hx + X(0.36 * r), hy + 0.14 * r).stroke({ width: 2.2, color: WOOD[2], cap: 'round' });
    g.roundRect(hx - 3.5, hy - 5.5, 7, 11, 1.5).fill(IRON[1]).stroke({ width: LINE * 0.75, color: INK });
  }
  void rnd;
}

// Roadmap 5 cemeteries: cairns, standing stones, yews, a lychgate, kurgans and tomb slabs.

/** Cairn: a heap of rounded field stones about 4 ft across, the big ones round its foot, smaller ones piled
 * on them toward the middle and a capstone on top; moss on one, a lighter heap of fresh stones on another. */
function cairn(g: Graphics, r0: number, rnd: () => number, v: number) {
  // (Drawn a third over its catalog size, a darker heap on a dark ring and a shadow, so it stands out among headstones.)
  const r = r0 * 1.35;
  const tones = STONE[v].map((c) => shade(c, 0.82));
  g.ellipse(r * 0.22, r * 0.3, r * 1.12, r * 0.95).fill({ color: 0x1c1a14, alpha: 0.3 });
  g.circle(0, 0, r * 0.98).fill(tones[0]).stroke({ width: OUT, color: INK });
  const ring = (n: number, d: number, s: number, off: number) => {
    const ph = rnd() * Math.PI * 2;
    for (let i = 0; i < n; i++) {
      const a = ph + ((i + (rnd() - 0.5) * 0.4) / n) * Math.PI * 2;
      stone(g, Math.cos(a) * d * r + off, Math.sin(a) * d * r + off, r * s * (0.85 + 0.3 * rnd()), tones, rnd, 9);
    }
  };
  ring(8, 0.66, 0.3, 0);
  ring(5, 0.36, 0.28, -r * 0.04);
  stone(g, -r * 0.08, -r * 0.08, r * 0.27, v === 3 ? STONE[0] : tones, rnd, 9);
  if (v === 1) moss(g, -r * 0.42, -r * 0.4, r * 0.18, rnd);
}

/** Standing stone, from a high angle like the headstone (upright things only read from the side): a tall rough
 * slab, unworked, narrowing to a broken top (its top showing behind), lit down its left side, shaded on the
 * right, a crack or two and pale lichen; grass and a fallen chip at its foot; one leaning. */
function standingStone(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  // (A cooler, darker grey than the headstones' stone.)
  const tones = [STONE[1], STONE[3], STONE[0], STONE[2]][v].map((c) => shade(c, 0.78));
  // (Drawn well over life size, so it stands taller than the headstones do.)
  const S = 1.6;
  const P0 = upright(UPRIGHT_FOOT[101]);
  const P = (x: number, d: number, h: number) => P0(x * S, d * S, h * S);
  const H = [8, 9.2, 7, 6.4][v];
  const lean = v === 3 ? 0.12 : 0;
  // The outline across its front (x, h), from the left foot up and over the top to the right foot.
  const j = () => (rnd() - 0.5) * 0.25;
  const out: [number, number][] = [
    [-1.5, 0], [-1.6 + j(), H * 0.3], [-1.35 + j(), H * 0.62], [-1.0 + j(), H * 0.9], [-0.45, H], [0.25 + j(), H - 0.15], [0.75, H - 0.55 + j()],
    [1.05 + j(), H * 0.8], [1.3 + j(), H * 0.5], [1.5 + j(), H * 0.2], [1.45, 0],
  ];
  const face = (d: number) => out.flatMap(([x, h]) => P(x + lean * h, d, h));
  const ink = { width: 2.2, color: INK, join: 'round' as const };
  // Its shadow cast to the lower right across the grass.
  {
    const [b0, b1] = [P(-1.5, 0.4, 0), P(1.45, 0.4, 0)];
    const [dx, dy] = [H * S * FT * 0.3, H * S * FT * 0.12];
    g.poly([b0[0], b0[1], b1[0], b1[1], b1[0] + dx, b1[1] + dy, b0[0] + dx * 0.9 + 6, b0[1] + dy + 4]).fill({ color: 0x1c1a14, alpha: 0.36 });
  }
  for (const x of [-1.7, 1.6]) footGrass(g, x * S * FT, P(0, 0.2, 0)[1], 6, rnd);
  // Its top, seen from above behind the front face, then the face.
  g.poly(face(-0.7)).fill(shade(tones[1], 1.05)).stroke(ink);
  g.poly(face(0)).fill(tones[1]).stroke(ink);
  // Lit down the left, in shade on the right.
  // (Points across the front as (x, fraction of the height), leaning with it.)
  const band = (pts: [number, number][]) => pts.flatMap(([x, f]) => P(x + lean * H * f, 0, H * f));
  g.poly(band([[-1.5, 0.01], [-1.6, 0.3], [-1.3, 0.62], [-0.95, 0.88], [-0.6, 0.82], [-0.95, 0.4], [-1.05, 0.02]])).fill({ color: tones[3], alpha: 0.6 });
  g.poly(band([[0.6, 0.72], [1.05, 0.8], [1.3, 0.5], [1.45, 0.005], [0.4, 0.005], [0.75, 0.35]])).fill({ color: tones[0], alpha: 0.5 });
  // A crack down from the top, a fainter one across.
  const c0 = 0.1 + (rnd() - 0.5) * 0.5;
  g.moveTo(...P(c0 + lean * H, 0, H - 0.2)).lineTo(...P(c0 + 0.3 + lean * H * 0.75, 0, H * 0.75)).lineTo(...P(c0 + lean * H * 0.55, 0, H * 0.55));
  g.stroke({ width: 1.8, color: shade(tones[0], 0.7), cap: 'round', join: 'round' });
  g.moveTo(...P(-1.1 + lean * H * 0.3, 0, H * 0.3)).lineTo(...P(-0.3 + lean * H * 0.33, 0, H * 0.34)).stroke({ width: 1.2, color: shade(tones[0], 0.8), cap: 'round' });
  // Lichen: a few pale crusts.
  for (let i = 0; i < 3; i++) {
    const [x, h] = [-0.8 + 1.2 * rnd(), H * (0.25 + 0.55 * rnd())];
    const [px, py] = P(x + lean * h, 0, h);
    g.poly(oval(px, py, 3 + rnd() * 2.5, 2 + rnd() * 1.5, rnd(), 9)).fill({ color: i === 0 ? 0xc8c48a : 0xb4b88e, alpha: 0.85 });
  }
  if (v === 1) moss(g, P(-0.9, 0, 0.6)[0], P(-0.9, 0, 0.6)[1], 6, rnd);
  footGrass(g, P(0.2, 0.2, 0)[0], P(0.2, 0.2, 0)[1] + 1, 4.5, rnd);
}

/** Yew from above: a dark, dense evergreen, its crown rounded to a blunt cone: whorls of many short, soft
 * lobes, each smaller and nearer the light than the one below, the darkest green of the trees; red berries
 * on some. */
function yew(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = [
    [0x2a4a30, 0x3a6040, 0x4c7a50, 0x6a9a62],
    [0x2c4c2a, 0x3c6238, 0x4e7c46, 0x6c9c5a],
    [0x284a38, 0x386048, 0x4a7a5a, 0x68986e],
    [0x2e502c, 0x40683a, 0x528046, 0x729c5a],
  ][v];
  const layers: [number, number][] = [
    [1, 22],
    [0.76, 17],
    [0.53, 13],
    [0.3, 9],
  ].map(([k, n]) => [k * 0.82, n]) as [number, number][];
  // A tier's edge: small round lobes, bulging out between dips, a little uneven.
  const tier = (cx: number, cy: number, rr: number, n: number, depth: number, jit: number[]) => {
    const ph = jit[n] * Math.PI;
    const at = (i: number, k: number): [number, number] => {
      const a = ph + (i / n) * Math.PI * 2;
      return [cx + Math.cos(a) * rr * k, cy + Math.sin(a) * rr * k];
    };
    g.moveTo(...at(0, 1 - depth));
    for (let i = 0; i < n; i++) {
      const bulge = 1 + depth * (0.6 + 0.8 * jit[i]);
      g.quadraticCurveTo(...at(i + 0.5, bulge), ...at(i + 1, 1 - depth));
    }
    g.closePath();
  };
  layers.forEach(([k, n], l) => {
    const off = -l * r * 0.07;
    const jit = Array.from({ length: n + 1 }, () => rnd());
    tier(off, off, r * k, n, 0.05, jit);
    g.fill(tones[l]);
    if (l === 0) g.stroke({ width: OUT, color: INK, join: 'round' });
    else g.stroke({ width: 1.5, color: shade(tones[l - 1], 0.7), join: 'round' });
    const h = r * k * 0.07;
    tier(off - h, off - h, r * k * 0.84, n, 0.05, jit);
    g.fill({ color: shade(tones[l], 1.3), alpha: 0.5 });
    tier(off - h * 1.6, off - h * 1.6, r * k * 0.55, n, 0.05, jit);
    g.fill({ color: tones[3], alpha: 0.28 });
  });
  if (v === 1 || v === 3) {
    for (let i = 0; i < 9; i++) {
      const a = rnd() * Math.PI * 2;
      const d = r * (0.35 + 0.55 * rnd());
      g.circle(Math.cos(a) * d - r * 0.05, Math.sin(a) * d - r * 0.05, 2.2).fill(0xc8362a).stroke({ width: 1, color: 0x4a1410 });
    }
  }
}

/** Lychgate from above, its ridge along x and the way through it along y: a short gabled roof (the north
 * plane paler, rows of tiles or shingles, a dark ridge, barge boards at the gable ends), its four corner posts
 * showing past the gables, a step of flags under the eaves either side; a dark opening and a pair of gate posts at each end where the way goes through. */
function lychgate(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const roof = [
    [0x6e3424, 0x9a5038, 0xbe6e4c],
    [0x7a4020, 0xa85e30, 0xcc8048],
    [0x5a3e26, 0x84603a, 0xa8804e],
    [0x5c2e22, 0x84442e, 0xa65e40],
  ][v];
  // (12 by 7 ft over the gap: a roof, not a post, at the battlemap's middle zoom.)
  const [rx, ry, px, py] = [6 * FT, 3.5 * FT, 6.45 * FT, 2.6 * FT];
  // Flagstones under the eaves where the way goes through.
  for (const s of [-1, 1]) {
    g.rect(-1.6 * FT, s > 0 ? ry - 2 : -ry - 1.3 * FT + 2, 3.2 * FT, 1.3 * FT).fill(STONE[1][2]).stroke({ width: LINE, color: INK });
    g.moveTo(0, s > 0 ? ry - 2 : -ry + 2).lineTo(0, s > 0 ? ry + 1.3 * FT - 2 : -ry - 1.3 * FT + 2).stroke({ width: 1, color: STONE[1][0] });
  }
  // The posts, past the gable ends.
  for (const sx of [-1, 1]) for (const sy of [-1, 1]) g.rect(sx * px - 0.45 * FT, sy * py - 0.45 * FT, 0.9 * FT, 0.9 * FT).fill(WOOD[1]).stroke({ width: LINE, color: INK });
  // The roof: an eave overhang with its shadow, the south plane in shade, the north lit, tile rows along the ridge.
  const ov = 0.5 * FT;
  g.rect(-rx - ov + 3, -ry - ov + 4, rx * 2 + ov * 2, ry * 2 + ov * 2).fill({ color: 0x1c1a14, alpha: 0.32 });
  g.rect(-rx - ov, -ry - ov, rx * 2 + ov * 2, ry * 2 + ov * 2).fill(shade(roof[0], 0.8)).stroke({ width: OUT, color: INK, join: 'round' });
  g.rect(-rx, -ry, rx * 2, ry).fill(shade(roof[2], 1.12));
  g.rect(-rx, 0, rx * 2, ry).fill(shade(roof[0], 0.9));
  for (let k = 1; k < 4; k++) {
    const y = (ry * k) / 4;
    g.moveTo(-rx + 2, -y).lineTo(rx - 2, -y);
    g.moveTo(-rx + 2, y).lineTo(rx - 2, y);
  }
  g.stroke({ width: 1.4, color: shade(roof[0], 0.6), alpha: 0.85 });
  if (v === 3) for (let i = 0; i < 3; i++) moss(g, -rx * 0.6 + rnd() * rx * 1.2, -ry * 0.6 + rnd() * ry * 1.4, 5, rnd);
  // Barge boards, the eave edges and a heavy ridge: a dark line, a pale cap on its lit side.
  for (const sx of [-1, 1]) g.rect(sx * rx - (sx > 0 ? 0.35 * FT : 0), -ry, 0.35 * FT, ry * 2).fill(WOOD[0]);
  g.moveTo(-rx, -ry).lineTo(rx, -ry).moveTo(-rx, ry).lineTo(rx, ry).stroke({ width: 2.4, color: shade(roof[0], 0.45) });
  g.moveTo(-rx, 0).lineTo(rx, 0).stroke({ width: 6, color: shade(roof[0], 0.5), cap: 'round' });
  g.moveTo(-rx + 2, -2).lineTo(rx - 2, -2).stroke({ width: 3, color: shade(roof[2], 1.3), cap: 'round' });
  g.rect(-rx, -ry, rx * 2, ry * 2).stroke({ width: OUT, color: INK, join: 'round' });
  // The way through: a dark threshold cut in each eave, flags worn pale before it, between two stout gate posts.
  for (const sy of [-1, 1]) {
    const y0 = sy > 0 ? ry - 0.9 * FT : -ry - ov;
    g.rect(-1.5 * FT, y0, 3 * FT, 0.9 * FT + ov).fill({ color: 0x120c07, alpha: 0.9 }).stroke({ width: LINE, color: INK });
    for (const sx of [-1, 1]) g.rect(sx * 1.5 * FT - 0.4 * FT, sy * ry - 0.4 * FT + sy * ov * 0.6, 0.8 * FT, 0.8 * FT).fill(WOOD[2]).stroke({ width: LINE, color: INK });
  }
}

/** Kurgan: a broad burial mound under grass, drawn at its catalog size (bigger or smaller by its scale): a
 * soft rounded rise on its own shadow, a darker band of turf round its foot, in shade on its south-east side
 * and lit toward the north-west, its crown paler, tufts of grass, a few stones at its foot; on its top a
 * standing stone (a slab laid flat on one, a stone figure on another), a worn path up one, a kerb of stones
 * right round the last. */
function kurgan(g: Graphics, r: number, rnd: () => number, v: number) {
  const grass = [
    [0x4a6a2e, 0x668a40, 0x82a452, 0xa0bc6a],
    [0x52682e, 0x6e8840, 0x8ca252, 0xaab86c],
    [0x4e6a34, 0x6a8a46, 0x86a45a, 0xa2bc74],
    [0x566c30, 0x728c42, 0x90a656, 0xb0be70],
  ][v];
  const R = r * 0.9;
  const rim = blob(0, 0, R, 28, 0.06, rnd);
  // Its shadow, then the darker turf at its foot, then the shaded side: the body drawn again a little
  // smaller and toward the light.
  g.poly(rim.map((p, i) => p * 0.98 + R * (i % 2 ? 0.07 : 0.08))).fill({ color: 0x1c1a14, alpha: 0.28 });
  g.poly(rim).fill(shade(grass[0], 0.5)).stroke({ width: 3, color: shade(grass[0], 0.36), join: 'round' });
  g.poly(rim.map((p) => p * 0.88 - R * 0.02)).fill(grass[0]);
  g.poly(rim.map((p) => p * 0.8 - R * 0.07)).fill(grass[1]);
  g.poly(rim.map((p) => p * 0.66 - R * 0.12)).fill(grass[2]);
  g.poly(rim.map((p) => p * 0.4 - R * 0.17)).fill({ color: grass[3], alpha: 0.8 });
  // Tufts of grass on its flanks.
  for (let i = 0; i < 10; i++) {
    const a = rnd() * Math.PI * 2;
    const d = R * (0.3 + 0.55 * rnd());
    footGrass(g, Math.cos(a) * d, Math.sin(a) * d, 5 + rnd() * 3, rnd);
  }
  if (v === 2) {
    // A worn path up its south side to the top.
    g.moveTo(R * 0.1, R * 0.98).quadraticCurveTo(R * 0.3, R * 0.5, -R * 0.12, -R * 0.15).stroke({ width: 9, color: 0xb09a6e, alpha: 0.75, cap: 'round' });
  }
  // Stones at its foot: a few, or a kerb right round it.
  const n = v === 3 ? 22 : 6;
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = v === 3 ? ph + (i / n) * Math.PI * 2 : ph + rnd() * Math.PI * 2;
    stone(g, Math.cos(a) * R * 0.97, Math.sin(a) * R * 0.97, v === 3 ? 11 : 9 + rnd() * 5, STONE[1], rnd, 7);
  }
  const [x, y] = [-R * 0.16, -R * 0.14];
  // (What stands on top, about a sixth of the mound across: `k` px a unit below, about its crown.)
  const k = R / 80;
  const at = (pts: number[]) => pts.map((d, i) => (i % 2 ? y : x) + d * k);
  if (v === 2) {
    // A slab laid flat on its crown, where the path ends: cracked, lit on its upper edge.
    g.ellipse(x + 4 * k, y + 5 * k, 17 * k, 10 * k).fill({ color: 0x1d2a12, alpha: 0.3 });
    g.roundRect(x - 14 * k, y - 8 * k, 28 * k, 16 * k, 3 * k).fill(STONE[1][1]).stroke({ width: 2 * k, color: INK });
    g.roundRect(x - 12 * k, y - 7 * k, 24 * k, 4 * k, 2 * k).fill(STONE[1][2]);
    g.poly(at([-3, -8, 1, -1, -1, 8]), false).stroke({ width: 1.4 * k, color: STONE[1][0], cap: 'round' });
  } else if (v !== 1) {
    // A standing stone on its crown, from a high angle: a rough upright, its top showing, its shadow behind.
    const t = STONE[v === 3 ? 3 : 0];
    g.ellipse(x + 8 * k, y + 3 * k, 15 * k, 6 * k).fill({ color: 0x1d2a12, alpha: 0.32 });
    g.poly(at([-6, 2, -7, -20, -2, -26, 5, -24, 7, 2])).fill(t[1]).stroke({ width: 2 * k, color: INK, join: 'round' });
    g.poly(at([-7, -20, -2, -26, 5, -24, 1, -19])).fill(t[2]).stroke({ width: 1.4 * k, color: INK, join: 'round' });
    g.poly(at([-4, -14, -4, -2]), false).stroke({ width: 1.6 * k, color: t[2], alpha: 0.7, cap: 'round' });
  }
  if (v === 1) {
    // A stone figure on its crown, from a high angle: a worn upright block with a face.
    g.ellipse(x + 6 * k, y + 4 * k, 16 * k, 7 * k).fill({ color: 0x1d2a12, alpha: 0.3 });
    g.roundRect(x - 8 * k, y - 28 * k, 16 * k, 30 * k, 6 * k).fill(STONE[2][1]).stroke({ width: 2 * k, color: INK });
    g.roundRect(x - 8 * k, y - 32 * k, 16 * k, 9 * k, 4 * k).fill(STONE[2][2]).stroke({ width: 1.6 * k, color: INK });
    for (const [a, b, c] of [[-4, -1.5, -17], [1.5, 4, -17], [-4, 4, -6]]) g.moveTo(x + a * k, y + c * k).lineTo(x + b * k, y + c * k);
    g.stroke({ width: 1.6 * k, color: STONE[2][0], cap: 'round' });
  }
}

/** Tomb slab from above, along x (about 7 ft by 3): a ledger stone flat on the grass with a carved cross
 * (v0), a chest tomb, its lid overhanging its sides (v1), a ledger sunk at one end, cracked, an epitaph cut
 * in it and moss over it (v2), a chest tomb with a raised cross on its lid and flowers laid on it (v3). */
function tombSlab(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = [STONE[1], STONE[0], STONE[3], STONE[2]][v];
  const [L, W] = [3.5 * FT, 1.5 * FT];
  const chest = v === 1 || v === 3;
  footGrass(g, -L - 1, W - 1, 4.5, rnd);
  footGrass(g, L + 1, -W + 3, 4, rnd);
  if (chest) {
    // Its sides, in shade below the lid on the south and east.
    g.rect(-L + 3, -W + 4, L * 2, W * 2).fill(tones[0]).stroke({ width: 2, color: INK, join: 'round' });
    g.rect(-L - 2, -W - 2, L * 2 + 4, W * 2 + 2).fill(tones[2]).stroke({ width: 2, color: INK, join: 'round' });
    g.rect(-L + 2, -W + 2, L * 2 - 4, W * 2 - 6).fill(tones[1]);
    g.rect(-L + 2, -W + 2, L * 2 - 4, 3).fill({ color: tones[3], alpha: 0.7 });
  } else {
    g.rect(-L, -W, L * 2, W * 2).fill(tones[1]).stroke({ width: 2, color: INK, join: 'round' });
    // A chamfer: lit along the north and west, in shade along the south and east.
    g.poly([-L, -W, L, -W, L - 3, -W + 3, -L + 3, -W + 3]).fill(tones[2]);
    g.poly([-L, -W, -L + 3, -W + 3, -L + 3, W - 3, -L, W]).fill(tones[2]);
    g.poly([L, W, -L, W, -L + 3, W - 3, L - 3, W - 3]).fill(tones[0]);
  }
  const groove = (pts: number[], w: number) => {
    for (let i = 0; i < pts.length; i += 4) g.moveTo(pts[i], pts[i + 1]).lineTo(pts[i + 2], pts[i + 3]);
    g.stroke({ width: w, color: shade(tones[0], 0.75), cap: 'round' });
  };
  if (v === 0 || v === 1) {
    groove([-L * 0.6, 0, L * 0.45, 0, -L * 0.25, -W * 0.62, -L * 0.25, W * 0.62], 3.4);
  } else if (v === 2) {
    // An epitaph across its head end, a crack, moss; its foot end sunk (shaded).
    for (let i = 0; i < 3; i++) groove([-L * 0.7 + i * 7, -W * 0.5, -L * 0.7 + i * 7, W * (0.2 + 0.25 * rnd())], 2);
    g.poly([L * 0.35, -W + 3, L - 3, -W + 3, L - 3, W - 3, L * 0.3, W - 3]).fill({ color: tones[0], alpha: 0.35 });
    g.moveTo(L * 0.05, -W).lineTo(L * 0.15, -W * 0.2).lineTo(L * 0.02, W * 0.4).lineTo(L * 0.12, W).stroke({ width: 1.6, color: INK, join: 'round' });
    moss(g, L * 0.6, W * 0.35, 7, rnd);
  } else {
    // A raised cross: a lit north-west edge over its shadow.
    const arms = (dx: number, dy: number) => [-L * 0.6 + dx, dy, L * 0.3 + dx, dy, -L * 0.25 + dx, -W * 0.6 + dy, -L * 0.25 + dx, W * 0.5 + dy];
    for (const [pts, w, c] of [[arms(1.5, 1.5), 5, tones[0]], [arms(0, 0), 4, tones[3]]] as const) {
      for (let i = 0; i < pts.length; i += 4) g.moveTo(pts[i], pts[i + 1]).lineTo(pts[i + 2], pts[i + 3]);
      g.stroke({ width: w, color: c, cap: 'round' });
    }
    for (let i = 0; i < 3; i++) g.circle(L * 0.45 + i * 5, -W * 0.25 + (i % 2) * 6, 3).fill([0xe0533d, 0xf2c94c, 0xf6efe0][i]).stroke({ width: 1, color: INK });
  }
}

// Roadmap 5 steppe towns: a horse park's horses and hitching rails.

/** Horse, standing, seen from above, its nose toward +x (about 8 ft nose to tail): the stable stall's horse
 * (`drawHorse` in furniture.ts) with its head and neck straight forward: bay with a star (v0), dapple grey (v1),
 * chestnut with a flaxen mane and a blaze (v2), the dapple grey under a red felt saddle blanket (v3). */
function horse(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  // Drawn in squares: the stall's frame has the nose at small `s`, so `s` runs toward the tail (-x here).
  g.scale.set(PX);
  const at = (u: number, s: number): [number, number] => [1.19 - s, u];
  // (R5 towns, 114: a dark rim round body, neck and head, so a horse reads on the trodden dirt.)
  g.ellipse(-0.06, 0, 0.74, 0.31).ellipse(0.74, 0, 0.5, 0.17).fill({ color: 0x24190f, alpha: 0.6 });
  drawHorse(g, at, 1, 0, v === 2 ? -1 : 1, [-0.7071, -0.7071], rnd, v === 3 ? 2 : v + 1, { bend: 0, shadow: false, blanket: v === 3 });
}

/** Weathered wood: [dark, mid, light, sunlit]. */
const GREY_WOOD = [0x5e5a52, 0x8a857a, 0xaaa496, 0xc8c2b2];

/** Hitching rail, along x (fixed 10 ft): a rounded rail mortised through two stout posts whose tops show
 * end grain, lit along the north; an iron tie ring (v0), a lead rope wrapped round it, its end trailing
 * (v1), weathered grey with cracks, a split post and grass at the feet (v2), a water bucket by a post (v3). */
function hitchingRail(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 2 ? GREY_WOOD : WOOD;
  // (R5 towns, 114: stout enough to read at a town's battle zoom.)
  const [L, T, P, ph] = [4.2 * FT, 0.38 * FT, 4.0 * FT, 0.64 * FT];
  if (v === 2) for (const x of [-P, P]) footGrass(g, x + 0.3 * FT, ph + 2, 0.45 * FT, rnd);
  g.roundRect(-L, -T, L * 2, T * 2, T * 0.7).fill(tones[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.roundRect(-L + 2, -T + 1.2, L * 2 - 4, T * 0.62, T * 0.3).fill(tones[2]);
  g.roundRect(-L + 2, T * 0.32, L * 2 - 4, T * 0.6, T * 0.3).fill(shade(tones[1], 0.84));
  for (let k = 0; k < 4; k++) {
    const x = -L + 0.6 * FT + rnd() * (L * 2 - 2 * FT);
    g.moveTo(x, (rnd() - 0.5) * T * 0.6).lineTo(x + (0.8 + rnd()) * FT, (rnd() - 0.5) * T * 0.6);
  }
  g.stroke({ width: v === 2 ? 1.4 : 1.1, color: shade(tones[0], v === 2 ? 0.8 : 1), cap: 'round' });
  for (const x of [-P, P]) {
    bevel(g, x - ph, -ph, x + ph, ph, 2.2, tones, OUT);
    g.circle(x, 0, ph * 0.5).stroke({ width: 1.1, color: shade(tones[0], 1.05), alpha: 0.8 });
    g.circle(x, 0, ph * 0.22).stroke({ width: 1.1, color: shade(tones[0], 1.05), alpha: 0.8 });
  }
  if (v === 2) {
    g.moveTo(P - ph + 2, -2).lineTo(P + 1, 1).lineTo(P + ph - 1, 0).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(-1.5 * FT, -1).lineTo(0.6 * FT, 0.5).stroke({ width: 1.4, color: shade(tones[0], 0.75), cap: 'round' });
    moss(g, -P - ph * 0.3, -ph * 0.4, ph * 0.55, rnd);
  }
  const ink2 = (draw: () => void, w: number, color: number) => {
    draw();
    g.stroke({ width: w + 2, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: w, color, cap: 'round', join: 'round' });
  };
  if (v === 0 || v === 3) {
    const x = v === 0 ? 0.9 * FT : -1.4 * FT;
    ink2(() => g.circle(x, T + 0.3 * FT, 0.34 * FT), 2.8, IRON[2]);
  }
  if (v === 1) {
    // A lead rope: three turns round the rail, its end trailing off to the south, frayed.
    const rope = 0xc8b07a;
    const x = -1.3 * FT;
    for (let k = 0; k < 3; k++) ink2(() => g.moveTo(x + k * 6 - 3, -T - 1.5).lineTo(x + k * 6 + 3, T + 1.5), 3, rope);
    ink2(() => g.moveTo(x + 13, T).quadraticCurveTo(x + 0.9 * FT, T + 2.1 * FT, x + 2.4 * FT, T + 2.2 * FT), 3, rope);
    const [ex, ey] = [x + 2.4 * FT, T + 2.2 * FT];
    for (const a of [-0.5, 0, 0.5]) g.moveTo(ex, ey).lineTo(ex + Math.cos(a) * 6, ey + Math.sin(a) * 6);
    g.stroke({ width: 1.5, color: shade(rope, 0.7), cap: 'round' });
  }
  if (v === 3) {
    // A wooden bucket of water by the east post, its handle laid over.
    const [bx, by, R] = [P - 1.5 * FT, T + 1.35 * FT, 1.05 * FT];
    g.circle(bx, by, R).fill(WOOD[2]).stroke({ width: LINE, color: INK });
    for (let k = 0; k < 12; k++) {
      const a = (k / 12) * Math.PI * 2;
      g.moveTo(bx + Math.cos(a) * R * 0.7, by + Math.sin(a) * R * 0.7).lineTo(bx + Math.cos(a) * R * 0.98, by + Math.sin(a) * R * 0.98);
    }
    g.stroke({ width: 1, color: WOOD[0], alpha: 0.8 });
    g.circle(bx, by, R * 0.86).stroke({ width: 2.2, color: IRON[1] });
    g.circle(bx, by, R * 0.64).fill(0x4a8a90).stroke({ width: 1.2, color: shade(0x255a70, 0.8) });
    g.moveTo(bx - R * 0.64, by).arc(bx, by, R * 0.64, Math.PI, Math.PI * 1.5).stroke({ width: 2, color: 0x1e4a5c, alpha: 0.6 });
    g.poly(oval(bx - R * 0.2, by - R * 0.15, R * 0.16, R * 0.1, -0.3, 10)).fill({ color: 0xe6f4ec, alpha: 0.75 });
    ink2(() => g.moveTo(bx - R * 0.95, by + R * 0.3).quadraticCurveTo(bx, by + R * 0.55, bx + R * 0.95, by - R * 0.3), 2, IRON[2]);
  }
}

// Roadmap 5 harbours (M20): moored boats, bollards, dock cranes, net racks and buoys.

/** Half a boat's beam at `u` along it (-1 the stern, 1 the stem): full forward, a fine bow, the stern a
 * transom `tr` of the beam wide. */
export const beamAt = (u: number, B: number, tr: number) => (u > 0 ? B * (1 - Math.pow(u, 2.4)) * (1 - 0.12 * u) : B * (1 - (1 - tr) * u * u));

/** A hull's outline along x, the bow toward +x: half its length `L`, half its beam `B`, moved by (dx, dy). */
function hullPts(L: number, B: number, tr: number, dx = 0, dy = 0, n = 20): number[] {
  const pts: number[] = [];
  for (let i = 0; i <= n; i++) {
    const u = -1 + (2 * i) / n;
    pts.push(u * L + dx, -beamAt(u, B, tr) + dy);
  }
  for (let i = n - 1; i >= 0; i--) {
    const u = -1 + (2 * i) / n;
    pts.push(u * L + dx, beamAt(u, B, tr) + dy);
  }
  return pts;
}

/** Hull paint: a muted slate teal and a red ochre. */
const BOAT_PAINT = [0x5f7f80, 0x97644a];
const ROPE = 0xc8b07a;
const NET = [0x8a6e4a, 0x5e4a36, 0x6e7a52, 0xb8a27a];
const CORK = 0xd6b27a;
const WICKER = 0xb08a52;

/** A small boat's open shell, bow toward +x: the hull's side (painted or bare) round the gunwale cap, lit
 * along the north; the inside sunk, its north and west walls in shadow; floor boards and thwarts at `thwarts`
 * (along it, -1 to 1). Gives the inside's half length and beam. */
function boatShell(g: Graphics, L: number, B: number, tr: number, tones: readonly number[], paint: number | null, thwarts: number[]): [number, number] {
  const side = (paint === null ? 0.3 : 0.42) * FT;
  const cap = 0.3 * FT;
  g.poly(hullPts(L, B, tr)).fill(paint ?? tones[1]).stroke({ width: OUT, color: INK, join: 'round' });
  if (paint !== null) g.poly(hullPts(L - side * 0.6, B - side * 0.45, tr)).fill(shade(paint, 0.94));
  g.poly(hullPts(L - side * 1.3, B - side, tr)).fill(tones[2]);
  const [Li, Bi] = [L - (side + cap) * 1.3, B - side - cap];
  g.poly(hullPts(Li, Bi, tr * 0.95)).fill(shade(tones[1], 0.84)).stroke({ width: 1.2, color: INK, join: 'round' });
  g.poly(hullPts(Li - 0.32 * FT, Bi - 0.28 * FT, tr * 0.9, 0.16 * FT, 0.14 * FT)).fill(tones[1]);
  // The cap's lit north edge.
  const mid = hullPts(L - side * 1.3 - cap * 0.6, B - side - cap * 0.5, tr);
  g.moveTo(mid[4], mid[5]);
  for (let i = 6; i <= 34; i += 2) g.lineTo(mid[i], mid[i + 1]);
  g.stroke({ width: 1.6, color: tones[3], alpha: 0.85, cap: 'round', join: 'round' });
  // Floor boards.
  for (const f of [-0.34, 0.34]) g.moveTo(-Li * 0.78, f * Bi).lineTo(Li * 0.5, f * Bi);
  g.stroke({ width: 1.1, color: shade(tones[0], 1.1), alpha: 0.7 });
  for (const u of thwarts) {
    const [x, w, t] = [u * Li, beamAt(u, Bi, tr * 0.95) + 1, 0.42 * FT];
    g.rect(x - t, -w, t * 2, w * 2).fill(tones[2]).stroke({ width: LINE, color: INK, join: 'round' });
    g.rect(x - t + 1.2, -w + 1.5, t * 0.6, w * 2 - 3).fill({ color: tones[3], alpha: 0.6 });
  }
  return [Li, Bi];
}

/** An oar lying from its grip (x0, y0) to its blade's tip (x1, y1): an inked ash loom, the blade widening to it. */
function oar(g: Graphics, x0: number, y0: number, x1: number, y1: number, color = 0xc9ac7c) {
  const len = Math.hypot(x1 - x0, y1 - y0);
  const [ux, uy] = [(x1 - x0) / len, (y1 - y0) / len];
  const [nx, ny] = [-uy, ux];
  const [bl, bw, sw] = [1.8 * FT, 0.3 * FT, 0.13 * FT];
  const [bx, by] = [x1 - ux * bl, y1 - uy * bl];
  g.moveTo(x0, y0).lineTo(bx, by).stroke({ width: sw * 2 + 2.6, color: INK, cap: 'round' });
  const at = (a: number, b: number) => [bx + ux * a + nx * b, by + uy * a + ny * b];
  g.poly([...at(-0.2 * FT, sw), ...at(bl * 0.45, bw), ...at(bl - 2, bw * 0.9), ...at(bl, 0), ...at(bl - 2, -bw * 0.9), ...at(bl * 0.45, -bw), ...at(-0.2 * FT, -sw)])
    .fill(color)
    .stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(x0, y0).lineTo(bx, by).stroke({ width: sw * 2, color, cap: 'round' });
  g.moveTo(x0, y0).lineTo(x0 + ux * 0.7 * FT, y0 + uy * 0.7 * FT).stroke({ width: sw * 2, color: shade(color, 0.72), cap: 'round' });
}

/** A coil of rope from above: inked, three turns, the outer one lit on the north-west. */
function ropeCoil(g: Graphics, x: number, y: number, R: number) {
  g.circle(x, y, R).fill(shade(ROPE, 0.82)).stroke({ width: LINE, color: INK });
  for (const k of [0.7, 0.42]) g.circle(x, y, R * k).stroke({ width: 1.2, color: shade(ROPE, 0.58) });
  g.circle(x, y, R * 0.2).fill(shade(ROPE, 0.5));
  g.moveTo(x + Math.cos(Math.PI * 0.85) * R * 0.85, y + Math.sin(Math.PI * 0.85) * R * 0.85)
    .arc(x, y, R * 0.85, Math.PI * 0.85, Math.PI * 1.65)
    .stroke({ width: 1.8, color: shade(ROPE, 1.12), cap: 'round' });
}

/** A round wicker basket from above, its weave in ticks round the rim, holding fish or cork floats. */
function basketOf(g: Graphics, x: number, y: number, R: number, what: 'fish' | 'floats', rnd: () => number) {
  g.circle(x, y, R).fill(WICKER).stroke({ width: LINE, color: INK });
  for (let k = 0; k < 14; k++) {
    const a = (k / 14) * Math.PI * 2;
    g.moveTo(x + Math.cos(a) * R * 0.8, y + Math.sin(a) * R * 0.8).lineTo(x + Math.cos(a) * R * 0.98, y + Math.sin(a) * R * 0.98);
  }
  g.stroke({ width: 1.1, color: shade(WICKER, 0.66) });
  g.circle(x, y, R * 0.78).fill(shade(WICKER, 0.62));
  g.circle(x + 1, y + 1, R * 0.66).fill(shade(WICKER, 0.8));
  for (let k = 0; k < 4; k++) {
    const a = rnd() * Math.PI * 2;
    const d = R * 0.3 * Math.sqrt(rnd());
    const [cx, cy, rot] = [x + Math.cos(a) * d, y + Math.sin(a) * d, rnd() * Math.PI];
    if (what === 'fish') {
      const [c, s] = [Math.cos(rot), Math.sin(rot)];
      const tail = [cx - c * R * 0.42, cy - s * R * 0.42];
      g.poly([tail[0], tail[1], tail[0] - c * R * 0.14 - s * R * 0.12, tail[1] - s * R * 0.14 + c * R * 0.12, tail[0] - c * R * 0.14 + s * R * 0.12, tail[1] - s * R * 0.14 - c * R * 0.12])
        .fill(0x8a9894)
        .stroke({ width: 1.1, color: 0x3a4440 });
      g.poly(oval(cx, cy, R * 0.4, R * 0.13, rot, 12)).fill(0xa8b4b0).stroke({ width: 1.1, color: 0x3a4440 });
      g.poly(oval(cx - s * R * 0.03, cy + c * R * 0.03 - 0.6, R * 0.28, R * 0.05, rot, 10)).fill(0xd8e0dc);
    } else {
      g.poly(oval(cx, cy, R * 0.22, R * 0.15, rot, 10)).fill(CORK).stroke({ width: 1.1, color: INK });
    }
  }
}

/** A heap of net from above: a lumpy pile lit on the north-west, its folds and a few mesh lines, cork
 * floats caught along its edge. */
function netHeap(g: Graphics, x: number, y: number, s: number, color: number, rnd: () => number) {
  const pts = blob(x, y, s, 11, 0.3, rnd);
  g.poly(pts).fill(color).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(blob(x - s * 0.18, y - s * 0.2, s * 0.55, 9, 0.3, rnd)).fill(shade(color, 1.16));
  for (let k = 0; k < 6; k++) {
    const a = rnd() * Math.PI * 2;
    const d = s * 0.55 * rnd();
    const [cx, cy] = [x + Math.cos(a) * d, y + Math.sin(a) * d];
    const b = rnd() * Math.PI;
    g.moveTo(cx - Math.cos(b) * s * 0.3, cy - Math.sin(b) * s * 0.3).quadraticCurveTo(cx + Math.sin(b) * s * 0.12, cy - Math.cos(b) * s * 0.12, cx + Math.cos(b) * s * 0.3, cy + Math.sin(b) * s * 0.3);
  }
  g.stroke({ width: 1.2, color: shade(color, 0.62), cap: 'round' });
  for (let i = 0; i < pts.length; i += 6) g.poly(oval(pts[i] * 0.92 + x * 0.08, pts[i + 1] * 0.92 + y * 0.08, s * 0.16, s * 0.11, rnd() * 3, 10)).fill(CORK).stroke({ width: 1.1, color: INK });
}

/** Wicker lobster pot from above: a rounded box, its hoops across, the neck's mouth dark on top. */
function lobsterPot(g: Graphics, x: number, y: number, w: number, h: number, rot: number) {
  const [c, s] = [Math.cos(rot), Math.sin(rot)];
  const P = (u: number, v: number) => [x + u * c - v * s, y + u * s + v * c];
  g.poly(oval(x, y, w, h, rot, 16)).fill(WICKER).stroke({ width: LINE, color: INK, join: 'round' });
  for (const u of [-0.5, 0, 0.5]) {
    const [a, b] = [P(u * w, -h * 0.92), P(u * w, h * 0.92)];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
  }
  g.stroke({ width: 1.4, color: shade(WICKER, 0.66) });
  const [lx, ly] = P(-w * 0.3, -h * 0.35);
  g.poly(oval(lx, ly, w * 0.35, h * 0.3, rot, 10)).fill({ color: shade(WICKER, 1.18), alpha: 0.8 });
  g.poly(oval(x, y, w * 0.24, h * 0.3, rot, 10)).fill(0x4a3420).stroke({ width: 1.2, color: shade(WICKER, 0.6) });
}

/** Rowboat, bow toward +x (about 13 ft): an open clinker boat, three thwarts, a ring at the stem; bare wood
 * with its oars shipped (v0), a slate-teal hull with a rope coiled in the bow (v1), weathered grey with
 * bilge water aft and one oar left askew (v2), a basket of the day's catch (v3). */
function rowboat(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 2 ? GREY_WOOD : WOOD;
  const [L, B, tr] = [6.4 * FT, 2.25 * FT, 0.6];
  const [Li, Bi] = boatShell(g, L, B, tr, tones, v === 1 ? BOAT_PAINT[0] : null, [-0.62, 0.02, 0.5]);
  if (v === 2) {
    g.poly(oval(-Li * 0.38, Bi * 0.08, Li * 0.2, Bi * 0.5, 0, 14)).fill({ color: 0x6f9298, alpha: 0.75 });
    g.poly(oval(-Li * 0.42, -Bi * 0.06, Li * 0.06, Bi * 0.12, 0, 8)).fill({ color: 0xe2eeea, alpha: 0.7 });
    oar(g, -Li * 0.72, Bi * 0.3, Li * 0.64, -Bi * 0.22, 0xb4aa94);
  } else {
    if (v === 3) basketOf(g, -Li * 0.3, Bi * 0.05, 0.78 * FT, 'fish', rnd);
    for (const f of [-0.4, 0.4]) oar(g, -Li * 0.8, f * Bi, Li * 0.64, f * Bi * 0.86);
    if (v === 1) ropeCoil(g, Li * 0.72, 0, 0.5 * FT);
  }
  g.circle(L - 0.55 * FT, 0, 0.17 * FT).stroke({ width: 3.2, color: INK }).circle(L - 0.55 * FT, 0, 0.17 * FT).stroke({ width: 1.5, color: IRON[3] });
}

/** Fishing boat, bow toward +x (about 22 ft): half-decked, the stub of its mast stepped at the foredeck,
 * a rudder hung on the transom and its tiller; a net heaped in the well and a basket of fish (v0), red
 * ochre with crates and a coil of rope (v1), weathered grey with lobster pots (v2), the sail furled on its
 * boom laid along the boat (v3). */
function fishingBoat(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 2 ? GREY_WOOD : WOOD;
  const [L, B, tr] = [10.4 * FT, 3.7 * FT, 0.55];
  // The rudder (its blade astern of the transom).
  g.roundRect(-L - 1.0 * FT, -0.24 * FT, 1.5 * FT, 0.48 * FT, 0.2 * FT).fill(tones[1]).stroke({ width: LINE, color: INK, join: 'round' });
  const [Li, Bi] = boatShell(g, L, B, tr, tones, v === 1 ? BOAT_PAINT[1] : null, [-0.5, 0.12]);
  // Foredeck and stern deck, planked fore and aft.
  const deck = (u0: number, u1: number) => {
    const pts: number[] = [];
    const [Ld, Bd] = [Li + 0.3 * FT, Bi + 0.25 * FT];
    for (let i = 0; i <= 10; i++) {
      const u = u0 + ((u1 - u0) * i) / 10;
      pts.push(u * Ld, -beamAt(u, Bd, tr));
    }
    for (let i = 10; i >= 0; i--) {
      const u = u0 + ((u1 - u0) * i) / 10;
      pts.push(u * Ld, beamAt(u, Bd, tr));
    }
    g.poly(pts).fill(tones[2]).stroke({ width: 1.2, color: INK, join: 'round' });
    for (const f of [-0.5, -0.17, 0.17, 0.5]) {
      let [a, b] = [u0, u1];
      while (b > a && beamAt(b, Bd, tr) < Math.abs(f) * Bd + 2) b -= 0.01;
      while (a < b && beamAt(a, Bd, tr) < Math.abs(f) * Bd + 2) a += 0.01;
      if (b > a) g.moveTo(a * Ld + 1, f * Bd).lineTo(b * Ld - 1, f * Bd);
    }
    g.stroke({ width: 1.1, color: shade(tones[1], 0.8), alpha: 0.8 });
  };
  deck(0.5, 1);
  deck(-1, -0.8);
  // A shadow on the well's floor under the foredeck's edge, to the south-east.
  g.rect(0.5 * (Li + 0.3 * FT) + 1.5, -Bi * 0.62, 0.32 * FT, Bi * 1.3).fill({ color: 0x000000, alpha: 0.18 });
  // The tiller over the stern deck.
  g.moveTo(-L + 0.2 * FT, 0).lineTo(-L + 3.2 * FT, 0.35 * FT).stroke({ width: 0.3 * FT + 2.6, color: INK, cap: 'round' });
  g.moveTo(-L + 0.2 * FT, 0).lineTo(-L + 3.2 * FT, 0.35 * FT).stroke({ width: 0.3 * FT, color: tones[2], cap: 'round' });
  if (v === 0) {
    netHeap(g, -Li * 0.22, Bi * 0.05, 1.7 * FT, NET[0], rnd);
    basketOf(g, Li * 0.32, 0, 0.82 * FT, 'fish', rnd);
  } else if (v === 1) {
    crate(g, -Li * 0.26, -Bi * 0.3, 0.62 * FT, 0.12, true);
    crate(g, -Li * 0.18, Bi * 0.34, 0.56 * FT, -0.2, false);
    ropeCoil(g, Li * 0.32, 0.1 * FT, 0.7 * FT);
  } else if (v === 2) {
    lobsterPot(g, -Li * 0.25, -Bi * 0.26, 0.85 * FT, 0.6 * FT, 0.15);
    lobsterPot(g, -Li * 0.2, Bi * 0.34, 0.8 * FT, 0.58 * FT, -0.25);
    ropeCoil(g, Li * 0.3, 0, 0.62 * FT);
  }
  // The mast's stub, stepped through the foredeck in its partners.
  const mx = 0.56 * Li;
  bevel(g, mx - 0.62 * FT, -0.62 * FT, mx + 0.62 * FT, 0.62 * FT, 1.6, tones, LINE);
  if (v === 3) {
    // The sail furled on its boom, laid aft along the boat, lashed every few feet.
    const [x0, x1, w] = [mx - 0.3 * FT, -Li * 0.78, 0.55 * FT];
    g.roundRect(x1 - 0.5 * FT, -0.14 * FT, x0 - x1 + 0.6 * FT, 0.28 * FT, 0.1 * FT).fill(tones[1]).stroke({ width: LINE, color: INK });
    const roll: number[] = [];
    const n = 9;
    for (let i = 0; i <= n; i++) roll.push(x0 + ((x1 - x0) * i) / n, -w * (0.75 + 0.25 * Math.sin(i * 1.7)) - 0.15 * FT);
    for (let i = n; i >= 0; i--) roll.push(x0 + ((x1 - x0) * i) / n, w * (0.75 + 0.25 * Math.cos(i * 1.3)) - 0.15 * FT);
    g.poly(roll).fill(0xcfc2a0).stroke({ width: OUT, color: INK, join: 'round' });
    g.roundRect(x1 + 0.3 * FT, -w * 0.72 - 0.15 * FT, x0 - x1 - 0.6 * FT, w * 0.55, w * 0.25).fill(0xe6dcc2);
    for (let i = 1; i < 5; i++) {
      const x = x0 + ((x1 - x0) * i) / 5;
      g.moveTo(x, -w - 0.15 * FT).lineTo(x + 2, w - 0.1 * FT);
    }
    g.stroke({ width: 1.8, color: shade(ROPE, 0.6), cap: 'round' });
    basketOf(g, -Li * 0.2, Bi * 0.6, 0.62 * FT, 'fish', rnd);
  }
  g.circle(mx, 0, 0.78 * FT).fill(tones[1]).stroke({ width: OUT, color: INK });
  g.circle(mx - 1, -1, 0.56 * FT).fill(shade(tones[2], 1.06)).stroke({ width: 1.6, color: tones[0] });
  g.circle(mx, 0, 0.26 * FT).fill(tones[0]).stroke({ width: LINE, color: INK });
  g.circle(mx - 1.5, -1.5, 0.1 * FT).fill(tones[3]);
}

/** Bollard on a quay, seen from above (about 1.5 ft across): cast iron, its cap lit on the north-west and
 * its foot's shadow to the south-east (v0); the same with a mooring line made fast round it, its slack
 * coiled by it (v1); a weathered timber post, its end grain and a split (v2); dressed stone, worn (v3). */
function bollard(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const R = 0.95 * FT;
  if (v === 1) {
    // The line: a loop round the post, two parts running off east, the slack coiled to the south-east.
    const rope = (draw: () => void, w: number) => {
      draw();
      g.stroke({ width: w + 2.4, color: INK, cap: 'round', join: 'round' });
      draw();
      g.stroke({ width: w, color: ROPE, cap: 'round', join: 'round' });
    };
    ropeCoil(g, 1.3 * FT, 1.85 * FT, 0.85 * FT);
    rope(() => g.moveTo(0.4 * FT, R * 0.6).quadraticCurveTo(1.6 * FT, R * 0.9, 1.9 * FT, 1.2 * FT), 2.6);
    rope(() => g.moveTo(R * 1.0, -R * 0.4).quadraticCurveTo(2.2 * FT, -0.4 * FT, 2.9 * FT, 0.3 * FT), 2.6);
  }
  // Every post: a wide foot ring, a round cap with a pale rim and a groove, its top lit on the north-west.
  const [foot, cap, rim, top] = v === 2 ? [GREY_WOOD[0], GREY_WOOD[1], GREY_WOOD[3], GREY_WOOD[2]] : v === 3 ? [STONE[0][0], STONE[0][1], STONE[0][3], STONE[0][2]] : [IRON[0], IRON[2], IRON[3], IRON[1]];
  g.circle(0, 0, R * 1.3).fill(foot).stroke({ width: LINE, color: INK });
  g.circle(0, 0, R).fill(cap).stroke({ width: LINE, color: INK });
  g.circle(-1.2, -1.2, R * 0.86).stroke({ width: 2, color: rim, alpha: 0.9 });
  g.circle(0.8, 0.8, R * 0.62).fill(top).stroke({ width: 1.4, color: shade(foot, 0.8) });
  g.circle(-R * 0.22, -R * 0.24, R * 0.2).fill(rim);
  if (v === 1) g.circle(0, 0, R * 1.12).stroke({ width: 2.8, color: ROPE });
  if (v === 2) g.moveTo(R * 0.15, -R * 0.1).lineTo(R * 0.55, -R * 0.55).stroke({ width: 1.6, color: INK, cap: 'round' });
  if (v === 3) {
    // An iron mooring ring let into the stone.
    g.circle(R * 0.62, R * 0.62, R * 0.28).stroke({ width: 3.4, color: INK });
    g.circle(R * 0.62, R * 0.62, R * 0.28).stroke({ width: 1.8, color: IRON[2] });
  }
  void rnd;
}

/** Dock crane, its jib out toward +x (about 22 ft over all): two sill beams crossed under a square post on
 * footing stones, a winch drum with two cranks on the south arm and its rope up to the jib, the jib with a
 * counterweight on its tail and a block at its tip; under the tip a barrel in a sling (v0), a crate in a
 * net (v1), sacks in a cargo net (v2), or, weathered grey, the hook hanging empty (v3). */
function dockCrane(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 3 ? GREY_WOOD : WOOD;
  const px = -3.4 * FT;
  const [arm, bw] = [4.6 * FT, 0.5 * FT];
  for (const [x, y] of [
    [px - arm, 0],
    [px + arm, 0],
    [px, -arm],
    [px, arm],
  ])
    stone(g, x, y, 0.85 * FT, STONE[1], rnd, 7);
  bevel(g, px - arm, -bw, px + arm, bw, 2, tones, OUT);
  bevel(g, px - bw, -arm, px + bw, arm, 2, tones, OUT);
  // The winch: a drum along the south arm's cross, rope wound on it, a crank at each end.
  const [wy, wl, wr] = [2.5 * FT, 1.6 * FT, 0.55 * FT];
  for (const s of [-1, 1]) bevel(g, px + s * wl - 0.25 * FT, wy - 0.75 * FT, px + s * wl + 0.25 * FT, wy + 0.75 * FT, 1.2, tones, LINE);
  g.roundRect(px - wl + 0.25 * FT, wy - wr, (wl - 0.25 * FT) * 2, wr * 2, wr * 0.5).fill(tones[1]).stroke({ width: LINE, color: INK });
  g.rect(px - wl * 0.6, wy - wr + 1.2, wl * 1.2, wr * 2 - 2.4).fill(ROPE);
  for (let k = 0; k < 3; k++) {
    const x = px - wl * 0.6 + ((k + 0.5) * wl * 1.2) / 3;
    g.moveTo(x - 1, wy - wr + 1.5).lineTo(x + 1, wy + wr - 1.5);
  }
  g.stroke({ width: 1.6, color: shade(ROPE, 0.6) });
  g.rect(px - wl * 0.6, wy - wr + 1.2, wl * 1.2, wr * 0.55).fill({ color: 0xffffff, alpha: 0.22 });
  const crank = (draw: () => void) => {
    draw();
    g.stroke({ width: 4.6, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: 2.2, color: IRON[2], cap: 'round', join: 'round' });
  };
  for (const s of [-1, 1]) crank(() => g.moveTo(px + s * (wl + 0.3 * FT), wy).lineTo(px + s * (wl + 0.75 * FT), wy).lineTo(px + s * (wl + 0.75 * FT), wy + s * 1.0 * FT));
  // The hoist rope from the drum up to the jib.
  g.moveTo(px + 0.2 * FT, wy - wr).lineTo(px + 1.4 * FT, 0.3 * FT).stroke({ width: 4.2, color: INK, cap: 'round' });
  g.moveTo(px + 0.2 * FT, wy - wr).lineTo(px + 1.4 * FT, 0.3 * FT).stroke({ width: 2, color: ROPE, cap: 'round' });
  // The load under the jib's tip.
  const [tx, lx] = [9.4 * FT, 8.8 * FT];
  const sling = (R: number) => {
    for (const a of [0.6, 2.2, 3.7, 5.3]) g.moveTo(lx, 0).lineTo(lx + Math.cos(a) * R, Math.sin(a) * R);
    g.stroke({ width: 4, color: INK, cap: 'round' });
    for (const a of [0.6, 2.2, 3.7, 5.3]) g.moveTo(lx, 0).lineTo(lx + Math.cos(a) * R, Math.sin(a) * R);
    g.stroke({ width: 2, color: ROPE, cap: 'round' });
  };
  if (v === 0) {
    barrel(g, lx, 0, 1.1 * FT, rnd);
    sling(1.0 * FT);
  } else if (v === 1) {
    crate(g, lx, 0, 0.95 * FT, 0.25, true);
    sling(1.25 * FT);
  } else if (v === 2) {
    for (const [dx, dy, a] of [
      [-0.5, -0.45, 0.3],
      [0.55, -0.3, -0.4],
      [0, 0.55, 1.2],
    ]) {
      const [cx, cy] = [lx + dx * FT, dy * FT];
      g.poly(oval(cx, cy, 0.8 * FT, 0.55 * FT, a, 14)).fill(0xc8b080).stroke({ width: LINE, color: INK, join: 'round' });
      g.poly(oval(cx - 2, cy - 2, 0.45 * FT, 0.28 * FT, a, 10)).fill(0xe0cca0);
    }
    g.poly(oval(lx, 0, 1.25 * FT, 1.0 * FT, 0, 16)).stroke({ width: 2.2, color: shade(NET[0], 0.7) });
  }
  // The post (the jib over it), the block at the jib's tip.
  bevel(g, px - 0.78 * FT, -0.78 * FT, px + 0.78 * FT, 0.78 * FT, 2.4, tones, OUT);
  // The jib: two timbers spread wide at the post and drawn together at the tip, tied across and laced
  // with braces, so it reads as a crane's boom; a stone counterweight on its tail.
  bevel(g, px - 2.9 * FT, -0.95 * FT, px - 1.1 * FT, 0.95 * FT, 2, STONE[1], OUT);
  const beam = (x0: number, y0: number, x1: number, y1: number, w: number) => {
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: w + OUT * 2, color: INK, cap: 'round' });
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: w, color: tones[1], cap: 'round' });
    g.moveTo(x0 - 1, y0 - w * 0.25).lineTo(x1 - 1, y1 - w * 0.25).stroke({ width: w * 0.35, color: tones[2], cap: 'round' });
  };
  const [j0, j1] = [0.95 * FT, 0.3 * FT];
  const jy = (x: number) => j0 + ((j1 - j0) * (x - px)) / (tx - px);
  for (let k = 0; k < 2; k++) {
    const [a, b] = [px + (0.4 + k * 1.9) * ((tx - px) / 4), px + (2.0 + k * 1.9) * ((tx - px) / 4)];
    g.moveTo(a, -jy(a)).lineTo(b, jy(b));
  }
  g.stroke({ width: 0.34 * FT + 2.4, color: INK, cap: 'butt' });
  for (let k = 0; k < 2; k++) {
    const [a, b] = [px + (0.4 + k * 1.9) * ((tx - px) / 4), px + (2.0 + k * 1.9) * ((tx - px) / 4)];
    g.moveTo(a, -jy(a)).lineTo(b, jy(b));
  }
  g.stroke({ width: 0.34 * FT, color: shade(tones[1], 0.95), cap: 'butt' });
  for (const sgn of [-1, 1]) beam(px - 1.4 * FT, sgn * j0, tx, sgn * j1, 0.4 * FT);
  for (const x of [px + 2.8 * FT, px + 7.4 * FT]) {
    g.rect(x - 0.2 * FT, -jy(x), 0.4 * FT, jy(x) * 2).fill(tones[2]).stroke({ width: LINE, color: INK });
  }
  g.roundRect(tx - 0.65 * FT, -0.62 * FT, 1.2 * FT, 1.24 * FT, 0.3 * FT).fill(tones[2]).stroke({ width: LINE, color: INK });
  g.rect(tx - 0.45 * FT, -0.12 * FT, 0.8 * FT, 0.24 * FT).fill(IRON[0]);
  if (v === 3) {
    g.circle(tx + 0.9 * FT, 0, 0.42 * FT).stroke({ width: 0.3 * FT + 2.6, color: INK });
    g.circle(tx + 0.9 * FT, 0, 0.42 * FT).stroke({ width: 0.3 * FT, color: IRON[3] });
  }
}

/** Net rack, along x (10 ft), seen from high up: two forked posts, a pole across them, a net hung over it
 * to dry, hanging in scallops weighted with cork floats; one brown net (v0), a tarred net and a green one
 * side by side (v1), a pale hemp net half off with the rest heaped at the foot (v2), weathered grey posts,
 * a basket of floats by them (v3). */
function netRack(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 3 ? GREY_WOOD : WOOD;
  const [X, top, foot] = [4.3 * FT, -1.4 * FT, 1.9 * FT];
  const nets: [number, number, number][] = [[[-3.8, 3.8, NET[0]]], [[-3.9, -0.1, NET[1]], [0.15, 3.9, NET[2]]], [[-3.8, 1.1, NET[3]]], [[-3.8, 3.8, 0x857a58]]][v] as [number, number, number][];
  // A soft crescent under the net, on the side away from the light, no wider than the rack.
  g.poly(oval(0.5 * FT, top + 1.5 * FT, X + 0.2 * FT, 1.2 * FT, 0, 24)).fill({ color: 0x000000, alpha: 0.14 });
  for (const s of [-1, 1]) {
    g.poly(oval(s * X + 2, foot + 1, 0.5 * FT, 0.24 * FT, 0, 10)).fill({ color: 0x000000, alpha: 0.28 });
    g.moveTo(s * X, foot).lineTo(s * X, top).stroke({ width: 0.5 * FT + 2.8, color: INK, cap: 'round' });
    g.moveTo(s * X, foot).lineTo(s * X, top).stroke({ width: 0.5 * FT, color: tones[1], cap: 'round' });
    g.moveTo(s * X - 0.1 * FT, foot - 2).lineTo(s * X - 0.1 * FT, top + 2).stroke({ width: 1.6, color: tones[3], alpha: 0.8 });
  }
  if (v === 3) basketOf(g, 2.8 * FT, 2.6 * FT, 0.75 * FT, 'floats', rnd);
  const depth = 2.7 * FT;
  for (const [a, b, color] of nets) {
    const [x0, x1] = [a * FT, b * FT];
    const seg = (x1 - x0) / Math.max(2, Math.round((x1 - x0) / (1.7 * FT)));
    const bottom = (x: number) => top + depth - 0.32 * FT * Math.abs(Math.sin((Math.PI * (x - x0)) / seg));
    const pts = [x0, top, x1, top];
    for (let x = x1; x >= x0 - 0.01; x -= seg / 8) pts.push(x, bottom(x));
    // Soft on purpose: the net's edge is its own dark, not ink.
    g.poly(pts).fill({ color, alpha: 0.92 }).stroke({ width: LINE, color: shade(color, 0.5), join: 'round' });
    g.rect(x0, top, x1 - x0, 0.4 * FT).fill(shade(color, 1.18));
    for (let x = x0 + seg / 3; x < x1 - 2; x += seg / 3) g.moveTo(x, top + 2).lineTo(x + 0.08 * FT, bottom(x) - 1);
    for (const f of [0.36, 0.68]) {
      g.moveTo(x0 + 1, top + f * (bottom(x0) - top));
      for (let x = x0 + seg / 8; x <= x1 + 0.01; x += seg / 8) g.lineTo(x, top + f * (bottom(x) - top));
    }
    g.stroke({ width: 1.1, color: shade(color, 0.62), alpha: 0.85 });
    for (let x = x0; x <= x1 + 0.01; x += seg) g.poly(oval(x, bottom(x) - 1, 0.24 * FT, 0.16 * FT, 0, 10)).fill(CORK).stroke({ width: 1.1, color: INK });
  }
  if (v === 2) netHeap(g, 2.75 * FT, 1.35 * FT, 1.25 * FT, NET[3], rnd);
  // The pole over the posts' forks.
  g.moveTo(-X - 0.35 * FT, top).lineTo(X + 0.35 * FT, top).stroke({ width: 0.36 * FT + 2.8, color: INK, cap: 'round' });
  g.moveTo(-X - 0.35 * FT, top).lineTo(X + 0.35 * FT, top).stroke({ width: 0.36 * FT, color: tones[2], cap: 'round' });
  g.moveTo(-X - 0.2 * FT, top - 1.2).lineTo(X + 0.2 * FT, top - 1.2).stroke({ width: 1.2, color: tones[3], cap: 'round' });
  for (const s of [-1, 1]) g.circle(s * X, top, 0.3 * FT).fill(tones[2]).stroke({ width: LINE, color: INK });
}

/** Buoy afloat, from above, ripples round it: a can buoy painted red with a pale band and a ring on top
 * for its chain (v0), a tarred cask on its side, hooped, its chain going down (v1), a raft of corks with a
 * stick and a red pennant (v2), a pale ochre cone with a dark band under an iron cross top mark (v3). */
function buoy(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  // Ripples on the water: soft on purpose, no ink.
  for (const [k, a] of [
    [1.0, 0.5],
    [1.3, 0.3],
  ])
    g.poly(oval(0.12 * FT, 0.14 * FT, 1.55 * FT * k, 1.3 * FT * k, 0.2, 22)).stroke({ width: 1.6, color: 0xe2eeea, alpha: a });
  g.poly(oval(0.18 * FT, 0.2 * FT, 1.3 * FT, 1.1 * FT, 0, 18)).fill({ color: 0x1e3438, alpha: 0.22 });
  const R = 1.05 * FT;
  if (v === 1) {
    for (let k = 0; k < 4; k++) g.circle(1.35 * FT + k * 0.32 * FT, 0.1 * FT, 0.13 * FT).stroke({ width: 1.4, color: IRON[2], alpha: 0.9 - k * 0.2 });
    const tar = [0x4e3e2e, 0x6e5640, 0x8e7458, 0xb0957a];
    g.roundRect(-1.35 * FT, -0.85 * FT, 2.7 * FT, 1.7 * FT, 0.75 * FT).fill(tar[1]).stroke({ width: OUT, color: INK });
    g.roundRect(-1.2 * FT, -0.72 * FT, 2.4 * FT, 0.6 * FT, 0.3 * FT).fill(tar[2]);
    g.roundRect(-1.0 * FT, -0.62 * FT, 2.0 * FT, 0.18 * FT, 0.09 * FT).fill(tar[3]);
    for (const x of [-0.8, -0.3, 0.3, 0.8]) g.moveTo(x * FT, -0.82 * FT).lineTo(x * FT, 0.82 * FT);
    g.stroke({ width: 2, color: IRON[1] });
    g.circle(1.3 * FT, 0, 0.16 * FT).stroke({ width: 1.8, color: IRON[2] });
    return;
  }
  if (v === 2) {
    for (const [x, y, a] of [
      [-0.4, -0.25, 0.4],
      [0.45, -0.1, -0.3],
      [0, 0.45, 1.1],
    ]) {
      g.poly(oval(x * FT, y * FT, 0.7 * FT, 0.48 * FT, a, 14)).fill(0xc29a62).stroke({ width: LINE, color: INK, join: 'round' });
      g.poly(oval(x * FT - 1.5, y * FT - 1.5, 0.42 * FT, 0.25 * FT, a, 10)).fill(CORK);
    }
    g.poly([0.1 * FT, -0.05 * FT, 1.35 * FT, -0.45 * FT, 0.95 * FT, 0.15 * FT, 1.3 * FT, 0.55 * FT]).fill(0x9a4a3c).stroke({ width: 1.3, color: INK, join: 'round' });
    g.circle(0, 0, 0.22 * FT).fill(WOOD[1]).stroke({ width: LINE, color: INK });
    g.circle(-0.7, -0.7, 0.11 * FT).fill(WOOD[3]);
    void rnd;
    return;
  }
  const [dark, lit, band] = v === 0 ? [0x7e3e32, 0x9a4a3c, 0xe2d8c2] : [0x9a8450, 0xbea468, 0x3a3630];
  g.circle(0, 0, R).fill(dark).stroke({ width: LINE, color: INK });
  g.circle(-1.5, -1.5, R * 0.84).fill(lit);
  g.circle(-0.6, -0.6, R * 0.66).stroke({ width: 0.22 * FT, color: band });
  g.poly(oval(-R * 0.42, -R * 0.4, R * 0.22, R * 0.13, -0.7, 10)).fill({ color: 0xffffff, alpha: 0.35 });
  if (v === 0) {
    g.circle(-0.6, -0.6, R * 0.24).stroke({ width: 3.6, color: INK }).circle(-0.6, -0.6, R * 0.24).stroke({ width: 1.8, color: IRON[3] });
  } else {
    for (const a of [0.4, 0.4 + Math.PI / 2]) {
      const [c, s] = [Math.cos(a) * R * 0.5, Math.sin(a) * R * 0.5];
      g.moveTo(-0.6 - c, -0.6 - s).lineTo(-0.6 + c, -0.6 + s);
    }
    g.stroke({ width: 4.2, color: INK, cap: 'round' });
    for (const a of [0.4, 0.4 + Math.PI / 2]) {
      const [c, s] = [Math.cos(a) * R * 0.5, Math.sin(a) * R * 0.5];
      g.moveTo(-0.6 - c, -0.6 - s).lineTo(-0.6 + c, -0.6 + s);
    }
    g.stroke({ width: 2, color: IRON[2], cap: 'round' });
  }
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
  // Squares and streets.
  46: waterTrough,
  47: noticeBoard,
  48: signpost,
  49: stocks,
  50: lampPost,
  51: marketCross,
  // Parks and gardens.
  52: fountain,
  53: pond,
  54: bench,
  55: flowerbed,
  56: hedge,
  57: topiary,
  58: gazebo,
  59: bandstand,
  60: gardenStatue,
  61: footbridge,
  // v63 countryside: an orchard's fruit tree and a water mill's wheel.
  62: fruitTree,
  63: (g, r) => waterWheel(g, r),
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
  // v63 castles: a kitchen garden and a smithy's yard.
  75: vegRow,
  76: wattleFence,
  77: beehive,
  78: anvil,
  // Roadmap 5 cemeteries.
  100: cairn,
  101: standingStone,
  102: yew,
  103: lychgate,
  104: kurgan,
  105: tombSlab,
  // Roadmap 5 steppe towns' horse parks.
  106: horse,
  107: hitchingRail,
  // Roadmap 5 star walls' bastions (`fortProps.ts`).
  108: ballista,
  109: shotStones,
  // Roadmap 5 harbours (M20).
  140: rowboat,
  141: fishingBoat,
  142: bollard,
  143: dockCrane,
  144: netRack,
  145: buoy,
};

/** Room in the frame beyond the footprint (squares, by kind id) for tall things drawn from a high angle. */
const ROOM: Record<number, number> = { 142: 0.72, 107: 1.06, 100: 1.9, 101: 6.2, 103: 1.45, 105: 0.84, 48: 0.6, 62: 1.16, 50: 0.62, 51: 1.16, 56: 0.68, 57: 0.62, 60: 1.12 };

export function frameRadiusPx(info: KindInfo): number {
  // Radius in atlas px at scale 1 plus room for outlines.
  return Math.ceil(Math.max(info.radius, ROOM[info.id] ?? 0) * PX + 6);
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
    for (let v = 0; v < looks(info.id); v++) {
      const at = place(size);
      const g = new Graphics();
      g.position.set(at.x + rpx, at.y + rpx);
      (DRAW[info.id] ?? DRAW[12])(g, info.radius * PX, rng(info.id * 131 + v * 17 + 7), v);
      if (import.meta.env.DEV) {
        // Art past the frame is cut off flat (and could bleed into the next frame's mips).
        const b = g.getLocalBounds();
        const over = Math.max(-b.minX, b.maxX, -b.minY, b.maxY) - rpx;
        if (over > 0.5) console.warn(`[atlas] ${info.id} ${info.name} v${v}: ${over.toFixed(1)} px past its frame`);
      }
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
