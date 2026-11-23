// Waterfalls drawn in code, in the atlas props' cartoon look (few broad shapes, flat pale tones, ink):
//
//   drawFall(g, x, y, ux, uy, fall, sq = PX, seed = 0)
//     A fall seen from above on the battlemap, its lip at (x, y) px, the water running along (ux, uy):
//     wet rocks at the sides, the rounded plunge pool below with its ripple rings, the falling sheet
//     (pale, a few bold streaks, its foot in soft scallops, as long as the face runs out), the inked
//     lip across the channel with the glassy water just above it, foam in clumps at the foot and one
//     or two soft mist puffs over the pool.
//   drawFallCave(g, x, y, ux, uy, sq = PX, seed = 0, scale = 1)
//     The dark mouth of the cave behind a curtain at (x, y) px, in its ring of wet rock (drawn under
//     the sheet); drawFallCaveBy(g, x, y, ux, uy, fall, side, ...) puts it on the shelf behind the
//     sheet (`caveSpot`). drawFallCaveThrough: the mouth's dim shape seen through the sheet (over it).
//   drawCaveMark(g, x, y, rFt, sq = PX)
//     The DM's dashed violet ring round a hidden cave's mouth (over the fall).
//   drawFallMark(g, x, y, ux, uy, wPx, lPx, alpha = 1)
//     The map's mark in screen px: a pale curtain `wPx` across and `lPx` along with a few streaks and a
//     scalloped foot, an inked lip on its upstream side and a foam pool below; at its smallest (~20 px)
//     it is the map symbol.
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

/** A closed smooth curve through the midpoints of `pts` (each point its control): a hand-drawn blob. */
function blob(g: Graphics, pts: [number, number][]) {
  const n = pts.length;
  const mid = (i: number): [number, number] => {
    const [a, b] = [pts[i % n], pts[(i + 1) % n]];
    return [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
  };
  g.moveTo(...mid(n - 1));
  for (let i = 0; i < n; i++) g.quadraticCurveTo(pts[i][0], pts[i][1], ...mid(i));
  g.closePath();
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
  // A bold stroke: the house weight up close, at least `ft` feet wide on a big fall (so it isn't a
  // hairline when the battlemap is seen from farther out).
  const bold = (w: number, ft: number) => Math.max(ink(w), ft * k);
  const stoneR = Math.min(4.5, 1.6 + Math.sqrt(half) * 0.35);
  const m = mistScale(f);

  // The plunge pool: one rounded, uneven blob of pale churned water with an ink-blue rim, a
  // ripple ring inside it and a broken one outside.
  const ringPts = (r: number, ra: number, n: number, wob: number, ph: number) => {
    const pts: [number, number][] = [];
    for (let i = 0; i < n; i++) {
      const t = (i / n) * Math.PI * 2;
      const w = 1 + wob * Math.sin(t * 2 + seed + ph) + wob * 0.7 * Math.sin(t * 3 + seed * 2 + ph);
      pts.push(at(pc + Math.cos(t) * ra * w, Math.sin(t) * r * w));
    }
    return pts;
  };
  blob(g, ringPts(R, Ra, 9, 0.09, 0));
  g.fill({ color: POOL, alpha: 0.85 }).stroke({ width: bold(OUT, 0.3), color: 0x5d8f9a, alpha: 0.9 });
  blob(g, ringPts(R * 0.62, Ra * 0.62, 7, 0.1, 1.3));
  g.fill({ color: 0xc4e2e4, alpha: 0.9 });
  blob(g, ringPts(R * 0.8, Ra * 0.8, 8, 0.08, 2.1));
  g.stroke({ width: bold(LINE * 1.6, 0.35), color: 0xffffff, alpha: 0.85 });
  // (The outer ripple in two arcs downstream of the pool.)
  for (const [t0, t1] of [[-1.1, -0.25], [0.3, 1.0]]) {
    for (let j = 0; j <= 8; j++) {
      const t = t0 + ((t1 - t0) * j) / 8;
      const p = at(pc + Math.cos(t) * Ra * 1.18, Math.sin(t) * R * 1.12);
      if (j === 0) g.moveTo(...p);
      else g.lineTo(...p);
    }
    g.stroke({ width: bold(LINE * 1.4, 0.3), color: 0xffffff, alpha: 0.6, cap: 'round', join: 'round' });
  }

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

  // The glassy water just above the lip, its upstream edge soft.
  const bow = Math.min(1.6, half * 0.08);
  g.moveTo(...at(-2.4, -half)).quadraticCurveTo(...at(-2.4 - bow, 0), ...at(-2.4, half));
  g.lineTo(...at(0, half)).quadraticCurveTo(...at(bow, 0), ...at(0, -half)).closePath();
  g.fill({ color: 0xa6d0d2, alpha: 0.3 });
  // The falling sheet: the lip bowed a little downstream, the sides flaring out a touch, the foot
  // in a few soft scallops (the foam hides most of it); pale, outlined in ink.
  const ns = Math.max(3, Math.min(9, Math.round(half / 4)));
  const scal = Math.min(2.2, 0.6 + L * 0.08);
  const bulge = Array.from({ length: ns }, () => [0.7 + 0.6 * rnd(), 0.2 * rnd()]);
  const sheet = () => {
    g.moveTo(...at(0, -half)).quadraticCurveTo(...at(bow, 0), ...at(0, half));
    g.quadraticCurveTo(...at(L * 0.6, half * 1.01), ...at(L, half * 1.14));
    for (let j = 1; j <= ns; j++) {
      const b0 = half * 1.14 - ((j - 0.5) / ns) * 2 * half * 1.14;
      const b1 = half * 1.14 - (j / ns) * 2 * half * 1.14;
      g.quadraticCurveTo(...at(L + scal * bulge[j - 1][0], b0), ...at(L - scal * bulge[j - 1][1], b1));
    }
    g.quadraticCurveTo(...at(L * 0.6, -half * 1.01), ...at(0, -half));
    g.closePath();
  };
  sheet();
  g.fill({ color: SHEET, alpha: 0.96 });
  // A paler foot, as one soft band over the sheet's lower half.
  g.moveTo(...at(L * 0.5, -half)).quadraticCurveTo(...at(L * 0.62, 0), ...at(L * 0.5, half));
  g.lineTo(...at(L * 0.95, half * 1.1)).lineTo(...at(L * 0.95, -half * 1.1)).closePath();
  g.fill({ color: 0xffffff, alpha: 0.4 });
  // A few bold streaks down the sheet, white, a blue one between some; tapering off toward the foot.
  const nStreak = Math.max(3, Math.min(10, Math.round(f.width / 7)));
  const sw = bold(LINE * 2.6, Math.min(1.5, 0.55 + half * 0.03));
  for (let i = 0; i < nStreak; i++) {
    const b = -half + ((i + 0.5 + (rnd() - 0.5) * 0.4) / nStreak) * 2 * half;
    const a0 = (0.06 + rnd() * 0.12) * L;
    const a1 = L * (0.55 + rnd() * 0.35);
    g.moveTo(...at(a0, b)).quadraticCurveTo(...at((a0 + a1) / 2, b * 1.02), ...at(a1, b * (1 + 0.05 * (a1 / L))));
    g.stroke({ width: sw, color: 0xffffff, alpha: 0.95, cap: 'round' });
    if (i < nStreak - 1 && rnd() < 0.55) {
      const bd = b + (half / nStreak) * (0.85 + rnd() * 0.3);
      g.moveTo(...at(L * 0.12, bd)).lineTo(...at(L * (0.35 + rnd() * 0.25), bd));
      g.stroke({ width: sw * 0.7, color: STREAK_DARK, alpha: 0.9, cap: 'round' });
    }
  }
  sheet();
  g.stroke({ width: ink(OUT), color: INK, alpha: 0.55, join: 'round' });
  // The lip: a bold ink line across the channel where the smooth water breaks.
  g.moveTo(...at(0, -half - 0.6)).quadraticCurveTo(...at(bow, 0), ...at(0, half + 0.6));
  g.stroke({ width: bold(OUT * 2, 0.6), color: INK, alpha: 0.9, cap: 'round' });

  // Foam at the foot in a few irregular clumps (puffs inside one outline each), a couple breaking
  // off downstream; bigger and more churned under a higher drop.
  const puffR = Math.min(3.2, 1.3 + half / 18) * Math.min(1.5, 0.85 + 0.12 * m);
  const clumps = Math.max(2, Math.min(6, Math.round(half / 5)));
  const puffs: [number, number, number][] = [];
  for (let c = 0; c < clumps; c++) {
    const bc = -half + ((c + 0.5 + (rnd() - 0.5) * 0.5) / clumps) * 2 * half;
    const ac = L + (rnd() - 0.3) * 1.2 * m;
    const n = 3 + Math.floor(rnd() * 3);
    const span = Math.min((2 * half) / clumps, puffR * 2.6);
    for (let j = 0; j < n; j++) puffs.push([ac + (rnd() - 0.4) * puffR * 1.1, bc + (j / (n - 1) - 0.5) * span + (rnd() - 0.5) * puffR * 0.4, puffR * (0.65 + rnd() * 0.55)]);
  }
  for (let i = 0; i < 2; i++) puffs.push([L + Ra * (0.4 + rnd() * 0.5), (rnd() - 0.5) * half * 0.9, puffR * (0.45 + rnd() * 0.25)]);
  for (const [a, bb, r] of puffs) g.circle(...at(a, bb), r * k + ink(OUT)).fill({ color: FOAM_INK, alpha: 1 });
  for (const [a, bb, r] of puffs) g.circle(...at(a, bb), r * k).fill(FOAM);
  for (const [a, bb, r] of puffs) if (r > puffR * 0.9) g.circle(...at(a - r * 0.3, bb - r * 0.25), r * 0.35 * k).fill({ color: 0xffffff, alpha: 1 });
  // Mist: one or two soft round puffs over the pool (each a pale disc with a fainter rim round it),
  // kept to about the pool's size; denser, not wider, under a high drop.
  const nMist = m > 1.6 ? 2 : 1;
  for (let i = 0; i < nMist; i++) {
    const r = Math.min(R * 0.6, half * 0.5 + 1.5) * (1 - 0.3 * i);
    const [mx, my] = at(L + Ra * (0.35 + 0.5 * i), (i ? 1 : -1) * half * 0.18);
    const al = Math.min(0.24, 0.12 + 0.04 * m);
    g.circle(mx, my, r * 1.25 * k).fill({ color: 0xffffff, alpha: al * 0.45 });
    g.circle(mx, my, r * k).fill({ color: 0xffffff, alpha: al });
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
  // The foam pool: a rounded blob below the curtain.
  const pool: [number, number][] = [];
  for (let i = 0; i < 8; i++) {
    const t = (i / 8) * Math.PI * 2;
    const w = 1 + 0.08 * Math.sin(t * 3 + 1);
    pool.push(at(pc + Math.cos(t) * r * 0.75 * w, Math.sin(t) * r * w));
  }
  blob(g, pool);
  g.fill({ color: FOAM, alpha: 0.9 * alpha }).stroke({ width: Math.max(1.2, Math.min(2, wPx / 14)), color: FOAM_INK, alpha: 0.95 * alpha });
  // The curtain: pale, the lip bowed a little downstream, flaring to a foot in soft scallops.
  const bow = Math.min(3, lPx * 0.12);
  const ns = Math.max(2, Math.min(6, Math.round(wPx / 9)));
  const curtain = () => {
    g.moveTo(...at(0, -half)).quadraticCurveTo(...at(bow, 0), ...at(0, half));
    g.lineTo(...at(lPx, half * 1.1));
    for (let j = 1; j <= ns; j++) {
      const b0 = half * 1.1 - ((j - 0.5) / ns) * 2.2 * half;
      const b1 = half * 1.1 - (j / ns) * 2.2 * half;
      g.quadraticCurveTo(...at(lPx + Math.max(1.5, lPx * 0.18), b0), ...at(lPx, b1));
    }
    g.closePath();
  };
  curtain();
  g.fill({ color: 0xf2f9fa, alpha: 0.97 * alpha }).stroke({ width: Math.max(1.3, Math.min(2.2, wPx / 12)), color: 0x2f4f58, alpha: 0.95 * alpha, join: 'round' });
  // A few bold streaks down it.
  const n = Math.max(2, Math.min(6, Math.round(wPx / 7)));
  for (let i = 0; i < n; i++) {
    const b = -half + ((i + 0.5) / n) * 2 * half;
    g.moveTo(...at(lPx * 0.25, b)).lineTo(...at(lPx * (i % 2 ? 0.65 : 0.85), b * 1.04));
  }
  g.stroke({ width: Math.max(1.5, Math.min(2.6, wPx / 9)), color: 0x6fa6b4, alpha: 0.9 * alpha, cap: 'round' });
  // The ink lip across the channel.
  g.moveTo(...at(0, -half - 2)).quadraticCurveTo(...at(bow, 0), ...at(0, half + 2));
  g.stroke({ width: Math.max(2.2, Math.min(3.4, wPx / 8)), color: 0x3a3026, alpha: 0.95 * alpha, cap: 'round' });
}
