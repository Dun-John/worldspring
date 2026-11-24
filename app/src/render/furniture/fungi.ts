import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { INK, LINE, type Light, OUT, castShadow, frame, glowPool, patch } from './common';

// Fungi underground (`under::fungi`), all seen from straight above, life size against the 5-ft square: each kind
// has four looks (`v % 4`). Caps are lopsided discs in three close matte tones (a darker crescent away from the
// light, a lit patch toward it), a soft shadow away from the light.

/** Tones of a cap: shaded side, face, lit patch; and its spots or freckles. */
type Tones = { t: [number, number, number]; spot?: number };

/** A cap from straight above at (x, y), `r` across: shadow (for something `h` tall), the face over a darker crescent
 * away from the light, a lit patch toward it, an ink outline (`OUT` over half a square across, else `LINE`), and on
 * request spots and the ribbed rim of a big cap. */
function cap(g: Graphics, x: number, y: number, r: number, c: Tones, light: Light, rnd: () => number, h: number, spots = 0, rim = false) {
  const [lx, ly] = light;
  const body = patch(x, y, r, r * (0.9 + 0.1 * rnd()), rnd, 0.06, 20);
  if (h > 0) castShadow(g, body, light, h);
  g.poly(body).fill(c.t[0]);
  g.poly(patch(x + lx * r * 0.12, y + ly * r * 0.12, r * 0.86, r * 0.82, rnd, 0.05, 18)).fill(c.t[1]);
  if (rim) {
    // The ribbed rim (the gills' edge showing under the cap's lip): a band of short ticks round the edge, bolder on
    // the shaded side, and the lip's line inside it.
    for (let k = 0; k < 30; k++) {
      const a = (k / 30) * Math.PI * 2 + rnd() * 0.08;
      if (Math.cos(a) * lx + Math.sin(a) * ly > 0.55) continue;
      g.moveTo(x + Math.cos(a) * r * 0.78, y + Math.sin(a) * r * 0.78).lineTo(x + Math.cos(a) * r * 0.97, y + Math.sin(a) * r * 0.97);
    }
    g.stroke({ width: 0.02, color: c.t[0], alpha: 0.95 });
    g.poly(patch(x, y, r * 0.76, r * 0.74, rnd, 0.03, 20)).stroke({ width: 0.016, color: c.t[0], alpha: 0.8 });
  }
  g.poly(patch(x + lx * r * 0.36, y + ly * r * 0.36, r * 0.38, r * 0.3, rnd, 0.12, 12)).fill({ color: c.t[2], alpha: 0.9 });
  g.poly(body).stroke({ width: r > 0.25 ? OUT : LINE, color: INK, join: 'round' });
  for (let k = 0; k < spots; k++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.62 * Math.sqrt(rnd());
    const s = r * (0.07 + 0.06 * rnd());
    g.poly(patch(x + Math.cos(a) * d, y + Math.sin(a) * d, s, s * 0.85, rnd, 0.2, 8)).fill({ color: c.spot ?? c.t[2], alpha: 0.9 });
  }
}

/** Caps of mixed sizes in a lopsided clump about `spread` across round (cx, cy): [x, y, r], biggest first. Each
 * after the first grows beside one already there, barely overlapping any, so they bunch rather than ring a centre. */
function clump(cx: number, cy: number, n: number, rmin: number, rmax: number, spread: number, rnd: () => number): [number, number, number][] {
  const a0 = rnd() * Math.PI * 2;
  const out: [number, number, number][] = [[cx + Math.cos(a0) * spread * 0.12, cy + Math.sin(a0) * spread * 0.12, rmax]];
  for (let i = 1, tries = 0; i < n && tries < 200; tries++) {
    const r = rmin + (rmax - rmin) * rnd() * rnd() * 0.9;
    const [px, py, pr] = out[Math.floor(rnd() * out.length)];
    const a = rnd() * Math.PI * 2;
    const d = pr + r * (0.55 + 0.3 * rnd());
    const [x, y] = [px + Math.cos(a) * d, py + Math.sin(a) * d];
    if (Math.hypot(x - cx, y - cy) + r > spread / 2 + 0.02) continue;
    if (out.some(([qx, qy, qr]) => Math.hypot(x - qx, y - qy) < (r + qr) * 0.72)) continue;
    out.push([x, y, r]);
    i++;
  }
  return out.sort((p, q) => q[2] - p[2]);
}

/** A lobed outline round (x, y): `rx` by `ry`, turned by `rot`, its edge waved by `lobes` bumps (`depth` deep) and
 * a little jitter. */
function lobed(x: number, y: number, rx: number, ry: number, rot: number, lobes: number, depth: number, rnd: () => number, n = 20): number[] {
  const ph = rnd() * Math.PI * 2;
  const [c, s] = [Math.cos(rot), Math.sin(rot)];
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const k = 1 + depth * Math.sin(a * lobes + ph) + (rnd() - 0.5) * 0.06;
    const [u, w] = [Math.cos(a) * rx * k, Math.sin(a) * ry * k];
    pts.push(x + u * c - w * s, y + u * s + w * c);
  }
  return pts;
}

/** A small cap from straight above (glowing ones, toadstools): `body` in its shaded tone, the face toward the light,
 * a pale rim along the lit edge, a darker dimple at its middle (`dimple` across, 0 for none) and an ink outline. Matte:
 * no shine. */
function smallCap(g: Graphics, body: number[], x: number, y: number, r: number, t: [number, number, number], rim: number, light: Light, rnd: () => number, h: number, dimple = 0.24) {
  const [lx, ly] = light;
  if (h > 0) castShadow(g, body, light, h);
  g.poly(body).fill(t[0]);
  // The face: the outline shrunk toward a point a little lit-ward, so it keeps the lobes.
  const [fx, fy] = [x + lx * r * 0.16, y + ly * r * 0.16];
  const face: number[] = [];
  for (let i = 0; i < body.length; i += 2) face.push(fx + (body[i] - x) * 0.8, fy + (body[i + 1] - y) * 0.8);
  g.poly(face).fill(t[1]);
  const a0 = Math.atan2(ly, lx);
  for (let i = 0; i <= 8; i++) {
    const a = a0 - 1.2 + (i / 8) * 2.4;
    const [px, py] = [x + Math.cos(a) * r * 0.74, y + Math.sin(a) * r * 0.74];
    if (i === 0) g.moveTo(px, py);
    else g.lineTo(px, py);
  }
  g.stroke({ width: Math.max(0.014, r * 0.16), color: rim, alpha: 0.85, cap: 'round' });
  if (dimple > 0) g.poly(patch(x - lx * r * 0.06, y - ly * r * 0.06, r * dimple, r * dimple * 0.9, rnd, 0.2, 9)).fill({ color: t[0], alpha: 0.95 });
  g.poly(body).stroke({ width: LINE, color: INK, join: 'round' });
}

const GIANT: Tones[] = [
  { t: [0x7a3c30, 0x9e5240, 0xbc7458], spot: 0xe8dcc2 },
  { t: [0x7a5832, 0x9c7642, 0xb99460], spot: 0xe2d2b0 },
  { t: [0x4e4458, 0x6c6078, 0x8c8298], spot: 0xc6bccc },
  { t: [0x8e7a40, 0xb49e5a, 0xd2bf7c] },
];

/** Giant mushrooms (6 ft tall, blocking): two or three broad caps overlapping, the biggest about 3.5 ft across on a
 * square (on a 2 x 2 spot, about 9 ft), each with its ribbed gill rim, a lit crown and spots or freckles; a long soft
 * shadow. Looks: brick red with cream spots, tawny, dusky violet, honey with a dark crown. */
export function giantMushrooms(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const c = GIANT[v % 4];
  // (A one-square stand is smaller than its square, the caps 3 to 3.5 ft: the shrink keeps them clear of the edge.)
  const s = Math.min(f.w, f.h) * (Math.min(f.w, f.h) === 1 ? 0.82 : 1);
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const lay: [number, number, number][][] = [
    [[-0.08, 0.06, 0.4], [0.24, -0.22, 0.24], [0.28, 0.26, 0.14]],
    [[0.06, -0.04, 0.42], [-0.26, 0.24, 0.2]],
    [[-0.12, -0.1, 0.34], [0.2, 0.14, 0.3], [-0.26, 0.3, 0.12]],
    [[0.04, 0.08, 0.38], [-0.22, -0.24, 0.22], [0.3, -0.2, 0.13]],
  ];
  // Smaller caps drawn first, so the biggest overlaps them.
  const caps = lay[v % 4].map(([x, y, r]) => [cx + (x + (rnd() - 0.5) * 0.04) * s, cy + (y + (rnd() - 0.5) * 0.04) * s, r * s * (0.94 + 0.12 * rnd())]).sort((p, q) => p[2] - q[2]);
  for (const [x, y, r] of caps) {
    // (The honey look has no spots: a darker crown at the middle instead.)
    cap(g, x, y, r, c, light, rnd, 0.32 * s, !c.spot ? 0 : r > 0.2 * s ? 5 + Math.floor(rnd() * 4) : 2, r > 0.18 * s);
    if (!c.spot) g.poly(patch(x + light[0] * r * 0.05, y + light[1] * r * 0.05, r * 0.3, r * 0.27, rnd, 0.15, 12)).fill({ color: 0x7a5a2c, alpha: 0.85 });
  }
}

const GLOW: { t: [number, number, number]; glow: number; core: number }[] = [
  { t: [0x3e5e58, 0x58807a, 0x86aaa0], glow: 0x8fdcc4, core: 0xc8f2e2 },
  { t: [0x5e4e74, 0x7e6c98, 0x9886b0], glow: 0xb49cee, core: 0xdccff8 },
  { t: [0x7a5a30, 0x9c7840, 0xc49e62], glow: 0xf4c060, core: 0xfbe4a8 },
  { t: [0x6a7a62, 0x8ea284, 0xb4c6a8], glow: 0xc8f0b8, core: 0xeaf8d8 },
];

/** Glowing fungus (dim light, 10 ft): a soft pool of its glow on the floor and over it small caps seen from above,
 * the biggest about two feet across, each with its pale glowing rim and a darker dimple, inked; a different growth in
 * each look: 0 a tight rosette of overlapping caps (sea-green), 1 a loose scatter of four or five (violet), 2 a tuft of
 * stubby fingers in two or three bunches, glowing at the tips (amber), 3 two or three flat lobed discs with a growth
 * band (ghost-pale). */
export function fungus(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const look = v % 4;
  const c = GLOW[look];
  const [cx, cy] = [f.x + 0.5 + (rnd() - 0.5) * 0.12, f.y + 0.5 + (rnd() - 0.5) * 0.12];
  glowPool(g, cx, cy, 0.62, c.glow);
  if (look === 0 || look === 1) {
    // A rosette: caps barely apart, overlapping; a scatter: each well clear of the others.
    const caps: [number, number, number][] =
      look === 0
        ? clump(cx, cy, 5 + Math.floor(rnd() * 3), 0.08, 0.2, 0.7, rnd)
        : (() => {
            const out: [number, number, number][] = [];
            for (let t = 0; t < 80 && out.length < 4 + Math.floor(rnd() * 2); t++) {
              const r = 0.09 + 0.08 * rnd();
              const [x, y] = [cx + (rnd() - 0.5) * 0.6, cy + (rnd() - 0.5) * 0.56];
              if (out.every(([qx, qy, qr]) => Math.hypot(x - qx, y - qy) > r + qr + 0.05)) out.push([x, y, r]);
            }
            return out.sort((p, q) => q[2] - p[2]);
          })();
    for (const [x, y, r] of [...caps].reverse()) smallCap(g, lobed(x, y, r, r * 0.92, rnd() * 3, 3, 0.07, rnd, 14), x, y, r, c.t, c.core, light, rnd, 0.04);
    return;
  }
  if (look === 2) {
    // Fingers: short fat lobes standing in bunches, each leaning out from its bunch's foot, a glowing tip.
    const bunches = 2 + Math.floor(rnd() * 2);
    const fingers: [number, number, number, number][] = [];
    for (let b = 0; b < bunches; b++) {
      const [bx, by] = [cx + (rnd() - 0.5) * 0.4, cy + (rnd() - 0.5) * 0.4];
      const a0 = rnd() * Math.PI * 2;
      const n = 3 + Math.floor(rnd() * 2);
      for (let i = 0; i < n; i++) {
        const a = a0 + (i - n / 2) * 0.55 + (rnd() - 0.5) * 0.3;
        const L = 0.11 + 0.08 * rnd();
        fingers.push([bx + Math.cos(a) * L * 0.7, by + Math.sin(a) * L * 0.7, L, a]);
      }
    }
    for (const [x, y, L, a] of fingers.sort((p, q) => p[2] - q[2])) {
      const body = lobed(x, y, L, 0.055 + 0.015 * rnd(), a, 2, 0.04, rnd, 14);
      castShadow(g, body, light, 0.04);
      g.poly(body).fill(c.t[0]);
      g.poly(lobed(x + light[0] * 0.01, y + light[1] * 0.01, L * 0.82, 0.04, a, 2, 0.03, rnd, 12)).fill(c.t[1]);
      g.poly(body).stroke({ width: LINE, color: INK, join: 'round' });
      g.poly(patch(x + Math.cos(a) * L * 0.6, y + Math.sin(a) * L * 0.6, 0.026, 0.024, rnd, 0.15, 8)).fill({ color: c.core, alpha: 0.9 });
    }
    return;
  }
  // Flat lobed discs, overlapping, a growth band inside the rim.
  const n = 2 + Math.floor(rnd() * 2);
  const discs: [number, number, number][] = [];
  for (let t = 0; t < 60 && discs.length < n; t++) {
    const r = 0.15 + 0.1 * rnd();
    const [x, y] = [cx + (rnd() - 0.5) * 0.4, cy + (rnd() - 0.5) * 0.4];
    if (discs.every(([qx, qy, qr]) => Math.hypot(x - qx, y - qy) > (r + qr) * 0.6)) discs.push([x, y, r]);
  }
  for (const [x, y, r] of discs.sort((p, q) => p[2] - q[2])) {
    const rot = rnd() * 3;
    smallCap(g, lobed(x, y, r, r * 0.8, rot, 4, 0.1, rnd, 22), x, y, r * 0.85, c.t, c.core, light, rnd, 0.03, 0.14);
    g.poly(lobed(x + light[0] * r * 0.1, y + light[1] * r * 0.1, r * 0.5, r * 0.4, rot, 4, 0.1, rnd, 16)).stroke({ width: 0.014, color: c.t[0], alpha: 0.85 });
  }
}

const SMALL: Tones[] = [
  { t: [0x7e4434, 0xa05a42, 0xbe7c5c], spot: 0xeadcc4 },
  { t: [0x8a6232, 0xae8244, 0xcaa466], spot: 0x6e4c2a },
];
const PUFF: [number, number, number][] = [
  [0xb8ae98, 0xd8d0bc, 0xeee8d8],
  [0x7e6448, 0x9e8262, 0xbaa080],
];

/** Toadstools (walkable, under a foot tall): looks 0 and 1 three or four caps of mixed sizes, each with its gill
 * rim and darker crown (russet with pale spots; honey-brown); 2 and 3 puffballs, round and pale with a few flecks (2 fresh white, 3
 * old and brown, some burst open at the top, a puff of spores about them). */
export function toadstools(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5 + (rnd() - 0.5) * 0.16, f.y + 0.5 + (rnd() - 0.5) * 0.16];
  const look = v % 4;
  if (look < 2) {
    // Three or four caps, the biggest near a foot across: a pale gill rim showing at the lip on the shaded side, the
    // crown (a darker boss at the middle; russet ones with a few cream spots).
    const c = SMALL[look];
    const caps = clump(cx, cy, 3 + Math.floor(rnd() * 2), 0.09, 0.19, 0.7, rnd);
    for (const [x, y, r] of [...caps].reverse()) {
      cap(g, x, y, r, c, light, rnd, 0.06, look === 0 ? 3 : 0, r > 0.1);
      g.poly(patch(x + light[0] * r * 0.04, y + light[1] * r * 0.04, r * 0.3, r * 0.27, rnd, 0.15, 10)).fill({ color: look === 0 ? c.t[0] : 0x5c3e22, alpha: 0.9 });
    }
    return;
  }
  const t = PUFF[look - 2];
  if (look === 3) {
    // Old puffballs: three or four big brown balls, most burst at the top (a dark ragged mouth, its torn lip pale), a
    // dusting of spores about them.
    const balls = clump(cx, cy, 3 + Math.floor(rnd() * 2), 0.1, 0.2, 0.72, rnd);
    for (let i = 0; i < 3; i++) g.poly(patch(cx + (rnd() - 0.5) * 0.4, cy + (rnd() - 0.5) * 0.4, 0.14, 0.11, rnd, 0.3, 10)).fill({ color: 0x8a7458, alpha: 0.12 });
    for (const [x, y, r] of [...balls].reverse()) {
      cap(g, x, y, r, { t }, light, rnd, 0.08);
      if (r > 0.11 || rnd() < 0.5) {
        const mouth = patch(x - light[0] * r * 0.08, y - light[1] * r * 0.08, r * 0.36, r * 0.32, rnd, 0.28, 9);
        g.poly(mouth).fill(0x3e3024).stroke({ width: 0.02, color: 0xd2c0a0, alpha: 0.9, join: 'round' });
      } else g.circle(x, y, r * 0.16).fill({ color: t[0], alpha: 0.9 });
    }
    return;
  }
  const balls = clump(cx, cy, 4 + Math.floor(rnd() * 3), 0.07, 0.19, 0.7, rnd);
  for (const [x, y, r] of [...balls].reverse()) {
    cap(g, x, y, r, { t }, light, rnd, 0.07);
    for (let k = 0; k < 3; k++) g.circle(x + (rnd() - 0.5) * r, y + (rnd() - 0.5) * r, 0.012 + 0.006 * rnd()).fill({ color: t[0], alpha: 0.9 });
  }
}

const SHELF: Tones[] = [
  { t: [0x96602e, 0xbc8444, 0xd8aa6a], spot: 0xe6cc8e },
  { t: [0x5c4636, 0x7c6450, 0xa48e76], spot: 0xc8bca6 },
  { t: [0x9a8866, 0xbea982, 0xdac8a0], spot: 0xece0c0 },
  { t: [0x7a4030, 0x9a5a42, 0xb87a5e], spot: 0xd8b28e },
];

/** Shelf fungus (bracket fungus on the rock wall, `wall` the rock side): two to four half-round brackets growing out
 * of the wall, overlapping in tiers along it, the biggest about 1.5 ft out; growth bands round each, a pale margin at
 * its lip, each rooted a little into the wall's line so it touches the drawn wall. Looks: sulphur-ochre, turkey-tail
 * brown, warm cream, rust. */
export function shelfFungus(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, wall: [number, number]) {
  const c = SHELF[v % 4];
  const { at } = frame(f, wall);
  const n = 2 + Math.floor(rnd() * 3);
  const shelves: [number, number, number][] = [];
  let u = 0.16 + rnd() * 0.1;
  for (let i = 0; i < n && u < 0.86; i++) {
    const r = 0.13 + rnd() * 0.13;
    shelves.push([u, r, r * (0.85 + 0.35 * rnd())]);
    u += r * (1.1 + 0.5 * rnd());
  }
  // A bracket: a half-disc `r` along the wall and `d` out, from 0.04 inside the wall's line.
  const half = (u: number, r: number, d: number, k: number) => {
    const pts: number[] = [];
    for (let i = 0; i <= 14; i++) {
      const a = (i / 14) * Math.PI;
      const w = 1 + (i > 0 && i < 14 ? (rnd() - 0.5) * 0.1 : 0);
      pts.push(...at(u - Math.cos(a) * r * k * w, -0.04 + Math.sin(a) * (d * k + 0.04) * w));
    }
    return pts;
  };
  // The farthest out first (the top tier last, over them).
  for (const [u, r, d] of shelves.sort((p, q) => q[2] - p[2])) {
    const body = half(u, r, d, 1);
    castShadow(g, body, light, 0.05);
    g.poly(body).fill(c.t[0]);
    g.poly(half(u, r, d, 0.84)).fill(c.t[1]);
    g.poly(body).stroke({ width: LINE, color: INK, join: 'round' });
    // Growth bands, and the pale lip.
    for (const k of [0.62, 0.38]) {
      const pts = half(u, r, d, k);
      g.moveTo(pts[0], pts[1]);
      for (let i = 2; i < pts.length; i += 2) g.lineTo(pts[i], pts[i + 1]);
      g.stroke({ width: 0.016, color: c.t[0], alpha: 0.85 });
    }
    const lip = half(u, r, d, 0.93);
    g.moveTo(lip[4], lip[5]);
    for (let i = 6; i < lip.length - 4; i += 2) g.lineTo(lip[i], lip[i + 1]);
    g.stroke({ width: 0.018, color: c.spot!, alpha: 0.8 });
  }
}

/** Mycelium carpet as a prop (the palette, the gallery and above ground): one flat soft patch of the fungal lawn, a
 * thin darker rim, a few small pale caps. Underground the shader pools a level's carpet into one patch a grove
 * instead (`fungalMat` in underground.frag, from `underField` `matMask`), in the same tones. */
export function myceliumCarpet(g: Graphics, f: InteriorItem, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx, cy, 0.44, 0.42, rnd, 0.18, 14)).fill(0x8c9473).stroke({ width: 0.02, color: 0x666652, alpha: 0.8 });
  for (let i = 0; i < 3; i++) g.circle(cx + (rnd() - 0.5) * 0.5, cy + (rnd() - 0.5) * 0.5, 0.04 + 0.02 * rnd()).fill(0xd6d1b3);
}
