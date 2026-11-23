// Waterfalls drawn in code, in the atlas props' cartoon look (few broad shapes, flat pale tones, ink):
//
//   drawFall(g, x, y, ux, uy, fall, sq = PX, seed = 0)
//     A fall seen from above on the battlemap, its lip at (x, y) px, the water running along (ux, uy):
//     wet rocks at the sides, the round foamy plunge pool below with its ripple rings, the falling sheet
//     (pale, streaked, as long as the face runs out), the inked lip across the channel with the glassy
//     water just above it, a line of foam puffs at the foot and a light mist over the pool.
//   drawFallCave(g, x, y, ux, uy, sq = PX, seed = 0, scale = 1)
//     The dark mouth of the cave behind a curtain at (x, y) px, in its ring of wet rock (drawn under
//     the sheet); drawFallCaveBy(g, x, y, ux, uy, fall, side, ...) puts it on the shelf behind the
//     sheet (`caveSpot`). drawFallCaveThrough: the mouth's dim shape seen through the sheet (over it).
//   drawCaveMark(g, x, y, rFt, sq = PX)
//     The DM's dashed violet ring round a hidden cave's mouth (over the fall).
//   drawFallMark(g, x, y, ux, uy, wPx, lPx, alpha = 1)
//     The map's mark in screen px: a pale curtain band `wPx` across and `lPx` along with a few streaks,
//     an inked lip on its upstream side and a foam pool below; at its smallest it is the map symbol.
//
// `sq` is px a 5-ft square (the battlemap's shapes are at `PX`); sizes and line widths scale with it.
import type { Graphics } from 'pixi.js';
import { PX } from './scale';
import { INK, OUT, LINE } from './pen';
import { rng } from './atlas/common';

/** What a fall's drawing needs (ft): the channel's width at the lip, the drop, the face's run. */
export interface FallSize {
  width: number;
  drop: number;
  face: number;
}

const WET_STONE = [0x4f4d45, 0x6d6a5a, 0x8d8a74];
const SHEET = 0xd4eaee;
const STREAK_DARK = 0x86b6c0;
const POOL = 0xa9d2d6;
const FOAM = 0xf4fafa;
const FOAM_INK = 0x7fa9b2;
const GLASS = 0x7fb1b4;
const CAVE = 0x16120f;

/** The sheet's run (ft) seen from above: the face's, at least a few feet, longer for a higher drop
 * (the water arcs out), held to the drop's half and to 45 ft. */
export function sheetRun(f: FallSize): number {
  return Math.min(Math.max(f.face, 4) + f.drop * 0.03, Math.max(8, f.drop * 0.5), 45);
}

/** How much a fall's mist and spray grow with its drop (1 for a low one, up to 3.5). */
export function mistScale(f: FallSize): number {
  return 1 + Math.min(2.5, Math.max(0, f.drop - 60) / 180);
}

/** The plunge pool's radius across (ft): about the sheet's half width, a little more for a higher
 * drop, held to the banks. */
export function poolRadius(f: FallSize): number {
  const half = Math.max(f.width, 6) / 2;
  return Math.min(half + 3, Math.max(half * 0.95, Math.min(6 + f.drop * 0.05, 30)));
}

export function drawFall(g: Graphics, x: number, y: number, ux: number, uy: number, f: FallSize, sq = PX, seed = 0) {
  const k = sq / 5;
  const [nx, ny] = [-uy, ux];
  // (a along the stream from the lip, b across it, both ft.)
  const at = (a: number, b: number): [number, number] => [x + (ux * a + nx * b) * k, y + (uy * a + ny * b) * k];
  const poly = (pts: [number, number][]) => pts.flat();
  const rnd = rng(seed * 7919 + 17);
  const half = Math.max(f.width, 6) / 2;
  const L = sheetRun(f);
  const R = poolRadius(f);
  const Ra = R * 0.8;
  const pc = L + Ra * 0.75;
  const ink = (w: number) => Math.max(1, (w * sq) / PX);
  const stoneR = Math.min(4.5, 1.6 + Math.sqrt(half) * 0.35);

  // The plunge pool: an uneven round of pale churned water, two ripple rings round it.
  const ring = (r: number, ra: number, n: number, wob: number) => {
    const pts: [number, number][] = [];
    for (let i = 0; i < n; i++) {
      const t = (i / n) * Math.PI * 2;
      const w = 1 + wob * Math.sin(t * 3 + seed) + wob * 0.6 * Math.sin(t * 5 + seed * 2);
      pts.push(at(pc + Math.cos(t) * ra * w, Math.sin(t) * r * w));
    }
    return poly(pts);
  };
  g.poly(ring(R, Ra, 20, 0.05)).fill({ color: POOL, alpha: 0.45 });
  g.poly(ring(R * 0.6, Ra * 0.6, 18, 0.07)).fill({ color: FOAM, alpha: 0.3 });
  for (const s of [0.86, 1.04]) g.poly(ring(R * s, Ra * s, 22, 0.04)).stroke({ width: ink(LINE * 1.3), color: 0xffffff, alpha: s < 1 ? 0.55 : 0.35 });

  // Wet rocks along both sides of the sheet and round the pool's upper rim.
  const stone = (cx: number, cy: number, r: number) => {
    const pts: [number, number][] = [];
    const turn = rnd() * Math.PI;
    for (let j = 0; j < 7; j++) {
      const t = turn + (j / 7) * Math.PI * 2;
      const rr = r * (0.8 + 0.2 * Math.sin(j * 2.3 + turn * 3)) * k;
      pts.push([cx + Math.cos(t) * rr, cy + Math.sin(t) * rr * 0.85]);
    }
    g.poly(poly(pts).map((v, i) => v + (i % 2 ? 0.3 : 0.25) * r * k)).fill({ color: 0x000000, alpha: 0.14 });
    g.poly(poly(pts)).fill(WET_STONE[1]).stroke({ width: ink(OUT), color: INK, alpha: 0.8 });
    g.ellipse(cx - 0.25 * r * k, cy - 0.25 * r * k, 0.45 * r * k, 0.3 * r * k).fill({ color: WET_STONE[2], alpha: 0.7 });
    g.ellipse(cx - 0.15 * r * k, cy - 0.35 * r * k, 0.14 * r * k, 0.08 * r * k).fill({ color: 0xffffff, alpha: 0.5 });
  };
  for (const s of [-1, 1]) {
    const n = 2 + Math.floor(L / 10);
    for (let i = 0; i < n; i++) {
      const r = stoneR * (0.7 + rnd() * 0.5);
      stone(...at(-1 + (i / Math.max(1, n - 1)) * (L + 1), s * (half + r * 0.55 + rnd() * 0.8)), r);
    }
    stone(...at(pc - Ra * 0.35, s * (R * 0.98)), stoneR * (0.8 + rnd() * 0.3));
  }

  // The glassy water just above the lip, then the falling sheet: pale, wider toward the foot.
  g.poly(poly([at(-3.2, -half), at(-3.2, half), at(0, half), at(0, -half)])).fill({ color: GLASS, alpha: 0.45 });
  const sheet = poly([at(0, -half), at(0, half), at(L, half * 1.05), at(L, -half * 1.05)]);
  g.poly(sheet).fill({ color: SHEET, alpha: 0.93 });
  g.poly(poly([at(L * 0.55, -half * 1.03), at(L * 0.55, half * 1.03), at(L, half * 1.05), at(L, -half * 1.05)])).fill({ color: 0xffffff, alpha: 0.35 });
  // Streaks down the sheet: white, with a darker one between, broken toward the foot.
  const nStreak = Math.max(3, Math.min(40, Math.round(f.width / 3)));
  for (let i = 0; i < nStreak; i++) {
    const b = -half + ((i + 0.5 + (rnd() - 0.5) * 0.5) / nStreak) * 2 * half;
    const a0 = rnd() * 0.15 * L;
    const a1 = L * (0.6 + rnd() * 0.35);
    g.moveTo(...at(a0, b)).lineTo(...at(a1, b * (1 + 0.05 * (a1 / L))));
    g.stroke({ width: ink(LINE * 2.2), color: 0xffffff, alpha: 0.95, cap: 'round' });
    if (i < nStreak - 1 && rnd() < 0.6) {
      const bd = b + (half / nStreak) * (0.8 + rnd() * 0.4);
      g.moveTo(...at(L * 0.1, bd)).lineTo(...at(L * (0.35 + rnd() * 0.3), bd));
      g.stroke({ width: ink(LINE * 1.4), color: STREAK_DARK, alpha: 0.85, cap: 'round' });
    }
  }
  g.poly(sheet).stroke({ width: ink(LINE), color: FOAM_INK, alpha: 0.6 });
  // The lip: an ink line across the channel where the smooth water breaks.
  const lip: [number, number][] = [];
  for (let i = 0; i <= 8; i++) lip.push(at(0.25 * Math.sin(i * 1.7 + seed), -half - 0.5 + (i / 8) * (2 * half + 1)));
  g.moveTo(...lip[0]);
  for (const p of lip.slice(1)) g.lineTo(...p);
  g.stroke({ width: ink(OUT * 2), color: INK, alpha: 0.9, cap: 'round', join: 'round' });

  // Foam at the foot in uneven clumps (a few puffs each, one outline round each clump), some
  // breaking off downstream; bigger and more churned under a higher drop.
  const m = mistScale(f);
  const puffR = Math.min(1.8, 0.8 + half / 40) * Math.min(1.6, 0.85 + 0.15 * m);
  const puffs: [number, number, number][] = [];
  let b = -half * 1.02;
  while (b < half * 1.02) {
    const n = 2 + Math.floor(rnd() * 4);
    const span = (1.2 + rnd() * 1.8) * puffR * n * 0.5;
    const a0 = L + (rnd() - 0.4) * 1.5 * m;
    for (let j = 0; j < n; j++) {
      const r = puffR * (0.6 + rnd() * 0.8);
      puffs.push([a0 + (rnd() - 0.3) * 1.4 * m, Math.min(half * 1.05, b + (j / n) * span + (rnd() - 0.5) * puffR), r]);
    }
    b += span + puffR * (0.4 + rnd() * 1.8);
  }
  for (let i = 0; i < Math.round(2 + half / 8); i++) {
    puffs.push([L + Ra * (0.25 + rnd() * 0.6), (rnd() - 0.5) * 1.6 * half * 0.8, puffR * (0.35 + rnd() * 0.35)]);
  }
  for (const [a, bb, r] of puffs) g.circle(...at(a, bb), (r + 0.35) * k).fill({ color: FOAM_INK, alpha: 0.75 });
  for (const [a, bb, r] of puffs) g.circle(...at(a, bb), r * k).fill(FOAM);
  for (const [a, bb, r] of puffs) if (r > puffR * 0.9) g.circle(...at(a - r * 0.3, bb - r * 0.25), r * 0.4 * k).fill({ color: 0xffffff, alpha: 0.9 });
  // Mist over the pool, spreading past the banks under a high drop.
  const nMist = 2 + Math.round(m);
  for (let i = 0; i < nMist; i++) {
    const [mx, my] = at(L + Ra * (0.1 + 0.35 * i), (rnd() - 0.5) * half * 0.8);
    const s = m * (1 - i / (nMist + 1));
    g.ellipse(mx, my, (half * 0.55 + 2) * s * k, (Ra * 0.3 + 1) * s * k).fill({ color: 0xffffff, alpha: 0.12 + 0.04 / (i + 1) });
  }
}

/** Where a fall's cave mouth stands (ft from the lip: along the stream, across it on `side`), as
 * the generator puts it (`lod::rivers::behind_mouth`): midway across the shelf behind the falling
 * sheet, a quarter of the half width out from the middle. (The battlemap draws it where `falls_json`
 * says; this is for drawings without one.) */
export function caveSpot(f: FallSize, side: number): [number, number] {
  const back = Math.max(f.face - 9, 0.2 * f.face);
  return [0.5 * (back + f.face + 1), side * 0.25 * (f.width / 2)];
}

/** How much `drawFallCave` shrinks a mouth `off` ft across from the middle so it stays within the
 * sheet of a fall `width` ft wide. */
export function caveScale(width: number, off: number): number {
  return Math.max(0.35, Math.min(1, (Math.max(width, 6) / 2 - Math.abs(off) - 0.5) / 6.4));
}

/** `drawFallCave` at its spot by the fall whose lip is at (x, y) px (`caveSpot`; draw it before
 * the fall, so the sheet lies over it, then `drawFallCaveThrough` over the fall). */
export function drawFallCaveBy(g: Graphics, x: number, y: number, ux: number, uy: number, f: FallSize, side: number, sq = PX, seed = 0) {
  const [a, b] = caveSpot(f, side);
  const k = sq / 5;
  drawFallCave(g, x + (ux * a - uy * b) * k, y + (uy * a + ux * b) * k, ux, uy, sq, seed, caveScale(f.width, b));
}

/** The mouth of the cave behind a curtain at (x, y) px, in its ring of wet rock, `scale` times
 * its full size (12 ft across). */
export function drawFallCave(g: Graphics, x: number, y: number, ux: number, uy: number, sq = PX, seed = 0, scale = 1) {
  const k = (sq / 5) * scale;
  const rnd = rng(seed * 104729 + 3);
  const rot = Math.atan2(uy, ux) + Math.PI / 2;
  const [c, s] = [Math.cos(rot), Math.sin(rot)];
  const pt = (a: number, b: number): [number, number] => [x + (a * c - b * s) * k, y + (a * s + b * c) * k];
  const rim: number[] = [];
  for (let i = 0; i < 10; i++) {
    const t = (i / 10) * Math.PI * 2;
    const r = 1 + 0.12 * Math.sin(i * 2.7 + seed) + (rnd() - 0.5) * 0.1;
    rim.push(...pt(Math.cos(t) * 6.2 * r, Math.sin(t) * 4.6 * r));
  }
  const ink = (w: number) => Math.max(1, (w * sq) / PX);
  g.poly(rim.map((v, i) => v + (i % 2 ? 0.8 : 0.6) * k)).fill({ color: 0x000000, alpha: 0.15 });
  g.poly(rim).fill(WET_STONE[0]).stroke({ width: ink(OUT), color: INK, alpha: 0.85 });
  // (Sunlit on its upper-left lip.)
  g.ellipse(...pt(-1.5, -2.6), 3.6 * k, 1.2 * k).fill({ color: WET_STONE[2], alpha: 0.5 });
  const mouth: number[] = [];
  for (let i = 0; i < 12; i++) {
    const t = (i / 12) * Math.PI * 2;
    mouth.push(...pt(Math.cos(t) * 4.1, Math.sin(t) * 2.7 + (Math.sin(t) > 0 ? 0.4 : 0)));
  }
  g.poly(mouth).fill(CAVE).stroke({ width: ink(OUT), color: INK, alpha: 0.9 });
  g.ellipse(...pt(0, 0.5), 2.6 * k, 1.5 * k).fill({ color: 0x000000, alpha: 0.6 });
}

/** The cave's mouth as it shows through the falling sheet over it (draw over the fall): a dim dark
 * shape, soft-edged, at (x, y) px, `scale` as `drawFallCave`. */
export function drawFallCaveThrough(g: Graphics, x: number, y: number, ux: number, uy: number, sq = PX, scale = 1) {
  const k = (sq / 5) * scale;
  const rot = Math.atan2(uy, ux) + Math.PI / 2;
  const [c, s] = [Math.cos(rot), Math.sin(rot)];
  const mouth = (r: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 12; i++) {
      const t = (i / 12) * Math.PI * 2;
      const [a, b] = [Math.cos(t) * 4.1 * r, (Math.sin(t) * 2.7 + (Math.sin(t) > 0 ? 0.4 : 0)) * r];
      pts.push(x + (a * c - b * s) * k, y + (a * s + b * c) * k);
    }
    return pts;
  };
  g.poly(mouth(1.25)).fill({ color: 0x1f3a40, alpha: 0.16 });
  g.poly(mouth(1)).fill({ color: CAVE, alpha: 0.3 });
}

/** The DM's mark for a hidden cave (draw over the fall): a dashed violet ring of radius `rFt` ft
 * round its mouth at (x, y) px, a dot at its middle. */
export function drawCaveMark(g: Graphics, x: number, y: number, rFt: number, sq = PX) {
  const r = (rFt * sq) / 5;
  const n = 14;
  const w = Math.max(2, (0.7 * sq) / 5);
  g.circle(x, y, r).stroke({ width: w * 2.2, color: 0x2a1640, alpha: 0.35 });
  for (let i = 0; i < n; i++) {
    const a0 = (i / n) * Math.PI * 2;
    const a1 = a0 + ((Math.PI * 2) / n) * 0.55;
    g.moveTo(x + Math.cos(a0) * r, y + Math.sin(a0) * r).arc(x, y, r, a0, a1);
  }
  g.stroke({ width: w, color: 0xc58bff, alpha: 1, cap: 'round' });
  g.circle(x, y, Math.max(2, (0.9 * sq) / 5)).fill({ color: 0xc58bff, alpha: 1 }).stroke({ width: w * 0.5, color: 0x2a1640, alpha: 0.8 });
}

export function drawFallMark(g: Graphics, x: number, y: number, ux: number, uy: number, wPx: number, lPx: number, alpha = 1) {
  const [nx, ny] = [-uy, ux];
  const at = (a: number, b: number): [number, number] => [x + ux * a + nx * b, y + uy * a + ny * b];
  const half = wPx / 2;
  const r = Math.max(half * 1.05, 3);
  const pc = lPx + r * 0.55;
  const pool: number[] = [];
  for (let i = 0; i < 14; i++) {
    const t = (i / 14) * Math.PI * 2;
    pool.push(...at(pc + Math.cos(t) * r * 0.75, Math.sin(t) * r));
  }
  g.poly(pool).fill({ color: FOAM, alpha: 0.75 * alpha }).stroke({ width: 1, color: FOAM_INK, alpha: 0.8 * alpha });
  const band = [...at(0, -half), ...at(0, half), ...at(lPx, half * 1.08), ...at(lPx, -half * 1.08)];
  g.poly(band).fill({ color: 0xf2f9fa, alpha: 0.95 * alpha }).stroke({ width: 1.2, color: 0x2f4f58, alpha: 0.9 * alpha });
  const n = Math.max(2, Math.min(8, Math.round(wPx / 4)));
  for (let i = 0; i < n; i++) {
    const b = -half + ((i + 0.5) / n) * 2 * half;
    g.moveTo(...at(lPx * 0.15, b)).lineTo(...at(lPx * (i % 2 ? 0.7 : 0.9), b));
  }
  g.stroke({ width: Math.max(1, Math.min(2, wPx / 12)), color: 0x6fa6b4, alpha: 0.9 * alpha, cap: 'round' });
  g.moveTo(...at(0, -half - 1.5)).lineTo(...at(0, half + 1.5));
  g.stroke({ width: Math.max(1.6, Math.min(3, wPx / 10)), color: 0x3a3026, alpha: 0.95 * alpha, cap: 'round' });
}
