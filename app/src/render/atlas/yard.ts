import { Graphics } from 'pixi.js';
import { FT, INK, IRON, WOOD, blob, oval, shade } from './common';
import { boughs, lit, whorl } from './plants';
import { moss } from './stones';
import { LINE, OUT } from '../pen';
import { blockTop, footGrass, inked, rod } from './street';

/** A raised box from above, lit from the NW: the outline, light top and left bevels, dark bottom and right
 * bevels `d` wide, and the face. */
export function bevel(g: Graphics, x0: number, y0: number, x1: number, y1: number, d: number, tones: readonly number[], ink = 2.5) {
  g.rect(x0, y0, x1 - x0, y1 - y0).fill(tones[1]).stroke({ width: ink, color: INK, join: 'round' });
  g.poly([x0, y0, x1, y0, x1 - d, y0 + d, x0 + d, y0 + d]).fill(tones[2]);
  g.poly([x0, y0, x0 + d, y0 + d, x0 + d, y1 - d, x0, y1]).fill(shade(tones[2], 0.94));
  g.poly([x1, y0, x1, y1, x1 - d, y1 - d, x1 - d, y0 + d]).fill(tones[0]);
  g.poly([x0, y1, x1, y1, x1 - d, y1 - d, x0 + d, y1 - d]).fill(shade(tones[0], 0.92));
}

/** Barrel from above: a ring of staves, two iron hoops, a planked lid with a bung, lit on the NW. */
export function barrel(g: Graphics, x: number, y: number, r: number, rnd: () => number) {
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
 * lit on the NW; bold dark ink round the box, the frame, every plank seam and the brace. */
export function crate(g: Graphics, x: number, y: number, s: number, rot: number, braced: boolean) {
  const c = Math.cos(rot);
  const si = Math.sin(rot);
  const pt = (u: number, v: number) => [x + u * c - v * si, y + u * si + v * c];
  const quad = (u0: number, v0: number, u1: number, v1: number) => [...pt(u0, v0), ...pt(u1, v0), ...pt(u1, v1), ...pt(u0, v1)];
  const wood = [0x6e4e2e, 0x9a7448, 0xc09a68];
  const ink = Math.max(1.6, Math.min(3.4, s * 0.09));
  const f = s * 0.2;
  // The box and its frame boards (lit along the north and west).
  g.poly(quad(-s, -s, s, s)).fill(wood[0]);
  g.poly(quad(-s, -s, s, -s + f)).fill(wood[2]);
  g.poly(quad(-s, -s, -s + f, s)).fill(shade(wood[2], 0.95));
  // Three planks inside the frame, the first lit, each with a grain stroke.
  const [i0, i1] = [-s + f, s - f];
  const step = (i1 - i0) / 3;
  for (let i = 0; i < 3; i++) {
    const v0 = i0 + step * i;
    g.poly(quad(i0, v0, i1, v0 + step)).fill(i === 0 ? shade(wood[1], 1.08) : wood[1]);
    const [a, b] = [pt(i0 + (i1 - i0) * (0.15 + 0.2 * i), v0 + step * 0.55), pt(i0 + (i1 - i0) * (0.55 + 0.15 * i), v0 + step * 0.5)];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
  }
  g.stroke({ width: ink * 0.45, color: shade(wood[0], 0.7), alpha: 0.8, cap: 'round' });
  // The plank seams and the frame's inner edge, inked.
  for (let i = 1; i < 3; i++) {
    const [a, b] = [pt(i0, i0 + step * i), pt(i1, i0 + step * i)];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
  }
  g.stroke({ width: ink * 0.6, color: INK, cap: 'butt' });
  g.poly(quad(i0, i0, i1, i1)).stroke({ width: ink * 0.6, color: INK, join: 'miter' });
  // The frame's mitred corners.
  for (const [u, v] of [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
  ]) {
    const [a, b] = [pt(u * s, v * s), pt(u * (s - f), v * (s - f))];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
  }
  g.stroke({ width: ink * 0.5, color: INK, cap: 'round' });
  if (braced) {
    const [a, b] = [pt(i0, i1), pt(i1, i0)];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: f * 0.9 + ink * 1.2, color: INK, cap: 'butt' });
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: f * 0.9, color: wood[1], cap: 'butt' });
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: f * 0.35, color: wood[2], alpha: 0.7, cap: 'butt' });
  }
  g.poly(quad(-s, -s, s, s)).stroke({ width: ink, color: INK, join: 'round' });
  for (const [u, v] of [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
  ]) {
    const [nx, ny] = pt(u * (s - f / 2), v * (s - f / 2));
    g.circle(nx, ny, Math.max(1.2, ink * 0.55)).fill(INK);
  }
}

/** Barrels: one, two or three standing together. */
export function barrels(g: Graphics, r: number, rnd: () => number, v: number) {
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
export function crates(g: Graphics, r: number, rnd: () => number, v: number) {
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
export function cart(g: Graphics, r: number, rnd: () => number, v: number) {
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
export function marketStall(g: Graphics, r: number, rnd: () => number, v: number) {
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
export function well(g: Graphics, r: number, rnd: () => number, v: number) {
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
export function haystack(g: Graphics, r: number, rnd: () => number, v: number) {
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

/** Headstone, straight down: the top edge of an upright stone (about 2 ft by 0.4) at the head of a grave, its
 * plot laid out before it to the south: an arched stone (its ends rounded, its crown paler) over a grassy mound
 * (v0); a square-topped stone, moss on it, a kerb of stones round the plot (v1); a stone over a fresh mound of
 * earth, flowers laid at its foot (v2); a standing cross, its arms a bar from above, over a mound with a few
 * stones on it (v3). Its drop shadow falls from the stone (`UPRIGHT_FOOT`). */
export function headstone(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const stone = [
    [0x6e6a62, 0x9a968c, 0xc4c0b4],
    [0x625e58, 0x8a877e, 0xb4b0a6],
    [0x726c60, 0xa8a296, 0xd0cabc],
    [0x5e5c56, 0x7e7c74, 0xa8a69c],
  ][v];
  const sy = -1.35 * FT;
  // The plot: a mound (grass, or fresh earth on v2), lit toward the NW, darker to the SE, fading at its foot.
  const mound = v === 2 ? [0x5e4630, 0x7a5c3e, 0x9a7a56] : [0x4a6a2c, 0x5e823a, 0x7a9c4c];
  const [mx, my, rx, ry] = [0, 0.45 * FT, 1.0 * FT, 1.55 * FT];
  const rim = blob(mx, my, 1, 16, 0.12, rnd).map((p, i) => (i % 2 ? my + (p - my) * ry : mx + (p - mx) * rx));
  g.poly(rim).fill(mound[0]);
  g.poly(rim.map((p, i) => (i % 2 ? my + (p - my) * 0.82 - 1.5 : mx + (p - mx) * 0.84 - 1.5))).fill(mound[1]);
  g.poly(oval(mx - rx * 0.3, my - ry * 0.25, rx * 0.4, ry * 0.42, 0.15, 12)).fill({ color: mound[2], alpha: 0.7 });
  g.poly(rim).stroke({ width: 1.4, color: shade(mound[0], 0.62), join: 'round' });
  if (v === 2) {
    // Clods on the fresh earth.
    for (let i = 0; i < 5; i++) g.circle(mx + (rnd() - 0.5) * rx * 1.2, my + (rnd() - 0.2) * ry, 1.3 + rnd()).fill(shade(mound[0], 0.8));
  } else {
    for (const [x, y] of [
      [-rx * 0.8, my + ry * 0.7],
      [rx * 0.9, my - ry * 0.2],
    ]) {
      footGrass(g, x, y, 4.5, rnd);
    }
  }
  if (v === 1) {
    // A kerb of small dressed stones round the plot.
    const K = [shade(stone[0], 0.95), stone[1], stone[2]];
    for (const [x0, y0, x1, y1] of [
      [-1.25, -1.05, -0.95, 2.1],
      [0.95, -1.05, 1.25, 2.1],
      [-0.95, 1.8, 0.95, 2.1],
    ]) {
      blockTop(g, x0 * FT, y0 * FT, x1 * FT, y1 * FT, K, rnd);
    }
    for (const y of [-0.1, 0.9]) g.moveTo(-1.25 * FT + 1, y * FT).lineTo(-0.95 * FT - 1, y * FT + 1).moveTo(0.95 * FT + 1, y * FT + 0.5).lineTo(1.25 * FT - 1, y * FT);
    g.stroke({ width: 1, color: shade(stone[0], 0.8) });
  }
  if (v === 3) {
    for (let i = 0; i < 3; i++) g.circle((rnd() - 0.5) * rx, my + (rnd() - 0.3) * ry, 1.6 + rnd()).fill(stone[1]).stroke({ width: 1, color: INK });
    // The cross: the bar of its arms, its head paler at the middle.
    blockTop(g, -1.05 * FT, sy - 0.24 * FT, 1.05 * FT, sy + 0.24 * FT, stone, rnd);
    blockTop(g, -0.26 * FT, sy - 0.27 * FT, 0.26 * FT, sy + 0.27 * FT, [stone[1], stone[2], shade(stone[2], 1.08)], rnd);
    return;
  }
  // The stone's top edge.
  const [hw, hd] = [(v === 1 ? 0.95 : 1.0) * FT, (v === 2 ? 0.24 : 0.2) * FT];
  if (v === 1) {
    blockTop(g, -hw, sy - hd, hw, sy + hd, stone, rnd);
    // A corner knocked off, moss on its top.
    g.poly([hw - 4, sy - hd - 0.5, hw + 0.5, sy - hd - 0.5, hw + 0.5, sy - hd + 3]).fill(stone[0]).stroke({ width: 1, color: INK, join: 'round' });
    g.poly(oval(-hw * 0.45, sy - 0.5, 4.5, 1.8, 0.1, 10)).fill({ color: 0x6e8a3a, alpha: 0.9 });
    g.poly(oval(-hw * 0.2, sy + 0.8, 2.5, 1.2, 0, 8)).fill({ color: 0x8aa04a, alpha: 0.85 });
  } else {
    // Arched: the ends fall away, so they show rounded and darker, the crown pale along the middle.
    const pts: number[] = [];
    for (let i = 0; i <= 8; i++) {
      const a = -Math.PI / 2 + (i / 8) * Math.PI;
      pts.push(hw - hd + Math.cos(a) * hd * 1.1, sy + Math.sin(a) * hd);
    }
    for (let i = 0; i <= 8; i++) {
      const a = Math.PI / 2 + (i / 8) * Math.PI;
      pts.push(-hw + hd + Math.cos(a) * hd * 1.1, sy + Math.sin(a) * hd);
    }
    g.poly(pts).fill(stone[0]);
    g.poly([-hw * 0.72, sy - hd + 1, hw * 0.72, sy - hd + 1, hw * 0.72, sy + hd - 1, -hw * 0.72, sy + hd - 1]).fill(stone[1]);
    g.poly([-hw * 0.5, sy - hd + 1.2, hw * 0.4, sy - hd + 1.2, hw * 0.3, sy - 0.2, -hw * 0.5, sy]).fill(stone[2]);
    g.poly(pts).stroke({ width: LINE, color: INK, join: 'round' });
  }
  if (v === 2) {
    // Flowers laid at its foot: stems toward the stone, the heads on the earth.
    const fy = sy + hd + 0.45 * FT;
    for (let i = 0; i < 4; i++) g.moveTo(-0.3 * FT + i * 3.5, fy + 2).lineTo(-0.1 * FT + i * 2, sy + hd + 2);
    g.stroke({ width: 1.1, color: 0x3e6a2a, cap: 'round' });
    for (let i = 0; i < 4; i++) g.circle(-0.3 * FT + i * 3.5 + (i % 2), fy + 2 + (i % 2) * 2, 2.3).fill([0xe0533d, 0xf2c94c, 0xf6efe0, 0xe0533d][i]).stroke({ width: 1, color: INK });
  }
}

/** A figure seen straight down at (x, y), facing `a` (radians, 0 toward +x), `k` times life size, in `tones`
 * [dark, mid, light]: the shoulders an oval about 1.7 ft across, the head a circle a little forward of them, each
 * shaded away from the light. Returns `p(u, w)`: a point `u` ft forward and `w` ft to its right, in px. */
export function bust(g: Graphics, x: number, y: number, k: number, a: number, tones: readonly number[], rnd: () => number) {
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const p = (u: number, w: number): [number, number] => [x + (c * u - s * w) * FT * k, y + (s * u + c * w) * FT * k];
  const part = (cx: number, cy: number, rx: number, ry: number, rot: number) => {
    const o = oval(cx, cy, rx, ry, rot, 16).map((q) => q + (rnd() - 0.5) * 0.7);
    g.poly(o).fill(tones[0]);
    g.poly(oval(cx - Math.min(rx, ry) * 0.16, cy - Math.min(rx, ry) * 0.18, rx * 0.84, ry * 0.84, rot, 16)).fill(tones[1]);
    g.poly(oval(cx - Math.min(rx, ry) * 0.38, cy - Math.min(rx, ry) * 0.4, rx * 0.36, ry * 0.36, rot, 10)).fill({ color: tones[2], alpha: 0.85 });
    g.poly(o).stroke({ width: LINE, color: INK, join: 'round' });
  };
  part(...p(-0.05, 0), 0.46 * FT * k, 0.86 * FT * k, a);
  part(...p(0.12, 0), 0.38 * FT * k, 0.36 * FT * k, a);
  return p;
}

/** Statue on a plinth, straight down: a plinth about 4 ft square in two dressed tiers, lit on the NW, and on it a
 * figure a little over life size from above, head and shoulders, an arm out holding a sword past the plinth's
 * edge (marble v0, bronze gone green v1, weathered stone with moss v2), or a winged angel, its wings swept back
 * (v3). */
export function statue(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const plinth = [0x6e675c, 0x9a9182, 0xc4baa6];
  blockTop(g, -2.0 * FT, -2.0 * FT, 2.0 * FT, 2.0 * FT, plinth, rnd, OUT);
  blockTop(g, -1.55 * FT, -1.55 * FT, 1.55 * FT, 1.55 * FT, plinth.map((c) => shade(c, 1.06)), rnd, OUT);
  const body = [
    [0xa8a49c, 0xdcd8d0, 0xf4f2ec],
    [0x2e5e50, 0x5e9a84, 0x8ac4a8],
    [0x6e6a60, 0xa8a296, 0xc8c2b4],
    [0xa09c94, 0xd4d0c8, 0xeeece6],
  ][v];
  const K = 1.3;
  const a = -Math.PI / 2 + (rnd() - 0.5) * 0.6;
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const p = (u: number, w: number): [number, number] => [(c * u - s * w) * FT * K, (s * u + c * w) * FT * K];
  if (v === 3) {
    // Wings swept back from the shoulders, a few long feathers marked.
    for (const side of [1, -1]) {
      const pts = [...p(0.0, side * 0.35), ...p(-0.7, side * 1.15), ...p(-1.25, side * 0.9), ...p(-1.0, side * 0.45), ...p(-0.45, side * 0.15)];
      g.poly(pts).fill(body[1]).stroke({ width: LINE, color: INK, join: 'round' });
      for (const k of [0.55, 0.8]) g.moveTo(...p(-0.2, side * 0.3)).lineTo(...p(-k - 0.2, side * (0.55 + k * 0.3)));
      g.stroke({ width: 1.1, color: body[0], cap: 'round' });
    }
  } else {
    // The sword arm: from the right shoulder out forward, the blade out to the plinth's edge.
    const [sh, hand, tip] = [p(0.05, 0.62), p(0.75, 0.72), p(1.7, 0.76)];
    rod(g, hand, tip, 2.2, body[2]);
    rod(g, p(0.78, 0.5), p(0.72, 0.95), 2.4, body[0]);
    inked(g, () => g.moveTo(...sh).lineTo(...hand), 0.26 * FT * K, body[1]);
  }
  bust(g, 0, 0, K, a, body, rnd);
  if (v === 2) moss(g, ...p(-0.1, -0.5), 0.2 * FT, rnd);
}
