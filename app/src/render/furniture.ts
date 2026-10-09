// Building furniture, hand-drawn in the battlemap atlas's style: seen straight down, worn wood with square
// corners and slightly uneven edges, ink outlines, lit from the screen's NW (given in grid axes). Drawn
// into the item atlas once per look (InteriorLayer.itemKey), in squares at the item's grid position.
import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../gen/protocol';

const INK = 0x1d1a14;
/** Ink outline and inner line widths, in squares (about 3 and 1.6 px at 64 px a square). */
const OUT = 0.045;
const LINE = 0.025;
/** Wood tones: shaded side, face, lit edge. Matte, close together. */
const WOOD = [0x5e4028, 0x86603c, 0x9c7450];
const DARK_WOOD = [0x3e2a1c, 0x5e4430, 0x725440];
const IRON = [0x34322e, 0x5a5e64, 0x7e8288];
const BRASS = 0xc8a45a;
const LINEN = [0xc8bca0, 0xe4dac0, 0xf0e8d4];
/** Blanket and cloth dyes (sun-faded): cornflower, madder, moss, plum. */
const DYES = [0x4f72a8, 0xb5523a, 0x6a8a3a, 0x7a4a6a];
const BOOKS = [0x8a2e26, 0x2e5a6a, 0xb08a3a, 0x5a3a5e, 0x4a6a32, 0x3e5a8a, 0x7a4a2a, 0x9a6a4a, 0x6a2e2e];

export type Light = [number, number];

function shade(color: number, f: number): number {
  const ch = (s: number) => Math.min(255, Math.max(0, Math.round(((color >> s) & 255) * f)));
  return (ch(16) << 16) | (ch(8) << 8) | ch(0);
}

/** A small seeded random generator, so a look is drawn the same each time. */
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

/** An item laid out along its long side: `at(u, s)` is the grid point `u` squares along it and `s`
 * across, so one drawing serves both orientations. With `back` (the side against a wall, a unit step),
 * the item runs along that wall and `s` 0 is at the wall. */
function frame(f: InteriorItem, back: [number, number] | null = null) {
  const along = back ? back[1] !== 0 : f.w >= f.h;
  const flip = back ? back[0] + back[1] > 0 : false;
  const len = along ? f.w : f.h;
  const wid = along ? f.h : f.w;
  const at = (u: number, s0: number): [number, number] => {
    const s = flip ? wid - s0 : s0;
    return along ? [f.x + u, f.y + s] : [f.x + s, f.y + u];
  };
  /** The grid rect spanning u0..u1 along and s0..s1 across, as [x0, y0, x1, y1]. */
  const rect = (u0: number, s0: number, u1: number, s1: number): number[] => {
    const [ax, ay] = at(u0, s0);
    const [bx, by] = at(u1, s1);
    return [Math.min(ax, bx), Math.min(ay, by), Math.max(ax, bx), Math.max(ay, by)];
  };
  return { along, len, at, rect };
}

/** A hand-drawn quad: the rect's corners nudged by up to `j` squares, so edges aren't ruler-straight. */
function quad([x0, y0, x1, y1]: number[], rnd: () => number, j = 0.012): number[] {
  const n = () => (rnd() - 0.5) * 2 * j;
  return [x0 + n(), y0 + n(), x1 + n(), y0 + n(), x1 + n(), y1 + n(), x0 + n(), y1 + n()];
}

/** The strips along a rect's sides that face the light, `s` wide. */
function litEdges(g: Graphics, [x0, y0, x1, y1]: number[], [lx, ly]: Light, s: number) {
  if (lx < -0.3) g.rect(x0, y0, s, y1 - y0);
  if (lx > 0.3) g.rect(x1 - s, y0, s, y1 - y0);
  if (ly < -0.3) g.rect(x0, y0, x1 - x0, s);
  if (ly > 0.3) g.rect(x0, y1 - s, x1 - x0, s);
}

/** A worn board or box from above: square corners, a darker rim on the side away from the light (the
 * face shifted toward it), a faint lit edge, a few grain lines along `along`. Returns the face. */
function board(g: Graphics, r: number[], t: readonly number[], light: Light, rnd: () => number, along = true, e = 0.05, grain = 3): number[] {
  const [lx, ly] = light;
  g.poly(quad(r, rnd)).fill(t[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const [dx, dy] = [lx * e * 0.7, ly * e * 0.7];
  const face = [r[0] + e * 0.6 + dx, r[1] + e * 0.6 + dy, r[2] - e * 0.6 + dx, r[3] - e * 0.6 + dy];
  g.poly(quad(face, rnd, 0.008)).fill(t[1]);
  litEdges(g, face, light, e * 0.5);
  g.fill({ color: t[2], alpha: 0.5 });
  // Grain: short wavy lines along the board.
  const [fx0, fy0, fx1, fy1] = face;
  const L = along ? fx1 - fx0 : fy1 - fy0;
  const W = along ? fy1 - fy0 : fx1 - fx0;
  if (grain > 0) {
    for (let i = 0; i < grain * Math.max(1, Math.round(L)); i++) {
      const s0 = W * (0.15 + 0.7 * rnd());
      const u0 = L * rnd() * 0.7;
      const u1 = Math.min(u0 + L * (0.12 + 0.2 * rnd()), L - 0.02);
      const w = (rnd() - 0.5) * W * 0.08;
      const p = (u: number, s1: number): [number, number] => (along ? [fx0 + u, fy0 + s1] : [fx0 + s1, fy0 + u]);
      g.moveTo(...p(u0, s0)).quadraticCurveTo(...p((u0 + u1) / 2, s0 + w), ...p(u1, s0));
    }
    g.stroke({ width: 0.014, color: t[0], alpha: 0.7, cap: 'round' });
  }
  return face;
}

/** Seams between planks along a face, `n` planks. */
function planks(g: Graphics, face: number[], along: boolean, n: number, color: number, rnd: () => number) {
  const [x0, y0, x1, y1] = face;
  for (let k = 1; k < n; k++) {
    const t = k / n + (rnd() - 0.5) * 0.04;
    if (along) g.moveTo(x0, y0 + (y1 - y0) * t).lineTo(x1, y0 + (y1 - y0) * t + (rnd() - 0.5) * 0.01);
    else g.moveTo(x0 + (x1 - x0) * t, y0).lineTo(x0 + (x1 - x0) * t + (rnd() - 0.5) * 0.01, y1);
  }
  g.stroke({ width: LINE, color, alpha: 0.85 });
}

/** How much a direction faces the screen-NW light in grid axes, unrotated: 1 toward it, -1 away. */
function lit(a: number): number {
  return -(Math.cos(a) + Math.sin(a)) * Math.SQRT1_2;
}

/** An ellipse as points, turned by `rot`. */
function oval(cx: number, cy: number, rx: number, ry: number, rot: number, n = 16): number[] {
  const [c, sn] = [Math.cos(rot), Math.sin(rot)];
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2;
    const [x, y] = [Math.cos(t) * rx, Math.sin(t) * ry];
    pts.push(cx + x * c - y * sn, cy + x * sn + y * c);
  }
  return pts;
}

/** A rough ring of points round (cx, cy). */
function ring(cx: number, cy: number, r: number, rnd: () => number, j: number, n = 20): number[] {
  const pts: number[] = [];
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2;
    const rr = r + (rnd() - 0.5) * j;
    pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
  }
  return pts;
}


/** A camp cot: canvas stretched between two side poles that run past it at both ends, crossed legs
 * showing beyond the canvas at each end, a blanket thrown over the foot. */
function cot(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { len, rect, at } = frame(f);
  // Crossed legs at each end (under everything).
  for (const u of [0.1, len - 0.1]) {
    for (const [a, b] of [
      [0.16, 0.84],
      [0.84, 0.16],
    ]) {
      g.moveTo(...at(u - 0.05, a)).lineTo(...at(u + 0.05, b)).stroke({ width: 0.08, color: INK, cap: 'round' });
      g.moveTo(...at(u - 0.05, a)).lineTo(...at(u + 0.05, b)).stroke({ width: 0.04, color: WOOD[1], cap: 'round' });
    }
  }
  // The canvas, sagging (darker) down the middle, laced to the end bars.
  const [cx0, cy0, cx1, cy1] = rect(0.18, 0.24, len - 0.18, 0.76);
  g.poly(quad([cx0, cy0, cx1, cy1], rnd, 0.006)).fill(0xb8a47a).stroke({ width: LINE, color: INK });
  const [mx0, my0, mx1, my1] = rect(0.22, 0.38, len - 0.22, 0.62);
  g.rect(mx0, my0, mx1 - mx0, my1 - my0).fill({ color: 0x7a6a48, alpha: 0.3 });
  for (const u of [0.2, len - 0.2]) g.moveTo(...at(u, 0.26)).lineTo(...at(u, 0.74));
  g.stroke({ width: 0.02, color: 0x6a5a38 });
  // The poles, past the canvas at both ends.
  for (const s of [0.2, 0.8]) {
    g.moveTo(...at(0.03, s)).lineTo(...at(len - 0.03, s)).stroke({ width: 0.09, color: INK, cap: 'round' });
    g.moveTo(...at(0.03, s)).lineTo(...at(len - 0.03, s)).stroke({ width: 0.05, color: WOOD[1], cap: 'round' });
  }
  // A rough wool blanket over the foot, a rumpled edge.
  const [bx0, by0, bx1, by1] = rect(len - 0.7, 0.16, len - 0.24, 0.84);
  g.poly(quad([bx0, by0, bx1, by1], rnd, 0.03)).fill(0x6e6450).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(len - 0.55, 0.2)).quadraticCurveTo(...at(len - 0.5, 0.5), ...at(len - 0.56, 0.8)).stroke({ width: 0.018, color: 0x4a4434 });
}

function roundTable(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  g.poly(ring(cx, cy, 0.4, rnd, 0.012)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(ring(cx + lx * 0.025, cy + ly * 0.025, 0.35, rnd, 0.008)).fill(WOOD[1]);
  // Plank seams across the top, kept inside it.
  for (const d of [-0.12, 0.12]) {
    const half = Math.sqrt(0.32 * 0.32 - d * d);
    g.moveTo(cx - half, cy + d).lineTo(cx + half, cy + d + (rnd() - 0.5) * 0.01);
  }
  g.stroke({ width: LINE, color: WOOD[0], alpha: 0.85 });
}

function table(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, long: boolean) {
  const { along, len, rect, at } = frame(f);
  const face = board(g, rect(0.08, 0.1, len - 0.08, 0.9), WOOD, light, rnd, along, 0.05, 2);
  planks(g, face, along, 3, WOOD[0], rnd);
  // Board ends where planks meet, every square on long tables.
  if (long) {
    for (let u = 1; u < len; u++) {
      const d = (rnd() - 0.5) * 0.08;
      g.moveTo(...at(u + d, 0.16)).lineTo(...at(u + d + (rnd() - 0.5) * 0.015, 0.84));
    }
    g.stroke({ width: LINE, color: WOOD[0], alpha: 0.7 });
  }
}

/** A stool: legs peeking out on the shaded side, a worn round seat. */
function chair(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  for (const a of [0.4, 2.5, 4.6]) g.circle(cx + Math.cos(a) * 0.2 - lx * 0.07, cy + Math.sin(a) * 0.2 - ly * 0.07, 0.06).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  g.poly(ring(cx, cy, 0.24, rnd, 0.012, 14)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(ring(cx + lx * 0.02, cy + ly * 0.02, 0.2, rnd, 0.008, 14)).fill(WOOD[1]);
  g.moveTo(cx - 0.13, cy + 0.02).lineTo(cx + 0.13, cy - 0.01).stroke({ width: LINE, color: WOOD[0], alpha: 0.85 });
}

function bench(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect } = frame(f);
  // Legs: boards across under each end, wider than the seat.
  for (const u of [0.16, len - 0.26]) board(g, rect(u, 0.26, u + 0.1, 0.74), DARK_WOOD, light, rnd, !along, 0.02, 0);
  const face = board(g, rect(0.06, 0.32, len - 0.06, 0.68), WOOD, light, rnd, along, 0.04, 2);
  planks(g, face, along, 2, WOOD[0], rnd);
}

/** Chest, its front toward +y: a lid of planks, two iron bands over it, iron corners and a lock. */
function chest(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { x, y } = f;
  const t = v % 2 ? DARK_WOOD : WOOD;
  const face = board(g, [x + 0.14, y + 0.22, x + 0.86, y + 0.82], t, light, rnd, true, 0.05, 1);
  planks(g, face, true, 3, t[0], rnd);
  for (const bx of [x + 0.3, x + 0.62]) g.poly(quad([bx, y + 0.21, bx + 0.08, y + 0.83], rnd, 0.006)).fill(IRON[1]).stroke({ width: LINE, color: INK });
  for (const [cx, cy] of [
    [x + 0.15, y + 0.23],
    [x + 0.85, y + 0.23],
    [x + 0.15, y + 0.81],
    [x + 0.85, y + 0.81],
  ]) {
    g.rect(cx - 0.035, cy - 0.035, 0.07, 0.07).fill(IRON[0]);
  }
  g.rect(x + 0.44, y + 0.74, 0.12, 0.1).fill(BRASS).stroke({ width: LINE, color: INK });
  g.circle(x + 0.5, y + 0.8, 0.015).fill(INK);
}

/** Wardrobe, its doors toward +y: a heavy top with a moulded rim, the doors' front edge, two knobs. */
function wardrobe(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const face = board(g, [x + 0.08, y + 0.1, x + 0.92, y + 0.86], DARK_WOOD, light, rnd, true, 0.07, 1);
  g.rect(face[0] + 0.06, face[1] + 0.06, face[2] - face[0] - 0.12, face[3] - face[1] - 0.12).stroke({ width: LINE, color: DARK_WOOD[0] });
  g.poly(quad([x + 0.1, y + 0.86, x + 0.9, y + 0.92], rnd, 0.005)).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK });
  g.moveTo(x + 0.5, y + 0.86).lineTo(x + 0.5, y + 0.92).stroke({ width: LINE, color: INK });
  for (const kx of [x + 0.44, x + 0.56]) g.circle(kx, y + 0.89, 0.025).fill(BRASS);
}

/** How far shelves and bookcases stand off the wall behind them and the walls at their ends (squares):
 * clear of the walls' ink, drawn centred on the wall line (half of InteriorLayer's 0.45 outer wall). */
const WALL_GAP = 0.24;
const END_GAP = 0.12;

/** Bookcase against a wall, seen straight down: a back panel along the wall, end panels and uprights,
 * open toward the room, the books standing in it with their spines sticking out past the front, each
 * its colour with a band of gilt, the pages pale behind the spine; a gap now and then. */
function bookcase(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { len, rect } = frame(f, back);
  // Half a square deep: shallow shelves, books standing one row deep.
  const [s0, s1] = [WALL_GAP + 0.05, 0.56];
  // The floor of the case (dark, in its own shadow), then the books, then the panels over their ends.
  g.poly(quad(rect(END_GAP, s0, len - END_GAP, s1), rnd, 0.006)).fill(0x1e1610);
  let k = v * 3;
  for (let sq = 0; sq < len; sq++) {
    const u0 = sq + (sq === 0 ? END_GAP + 0.09 : 0.05);
    const u1 = sq + 1 - (sq === len - 1 ? END_GAP + 0.09 : 0.05);
    let u = u0 + 0.01;
    while (u < u1 - 0.04) {
      k++;
      const th = 0.06 + ((k * 37) % 4) * 0.012;
      if (u + th > u1 - 0.005) break;
      if (k % 9 === 0) {
        u += 0.05;
        continue;
      }
      const c = BOOKS[(k * 5) % BOOKS.length];
      // How far its spine stands out past the case.
      const out = s1 + 0.02 + ((k * 13) % 4) * 0.015;
      g.poly(quad(rect(u, s0 + 0.03, u + th, out), rnd, 0.004)).fill(c).stroke({ width: 0.012, color: INK });
      // Pages: pale between the covers, behind the spine.
      g.poly(quad(rect(u + th * 0.34, s0 + 0.05, u + th * 0.66, out - 0.08), rnd, 0.002)).fill({ color: LINEN[0], alpha: 0.75 });
      // The spine: the book's colour, a gilt band across it.
      g.poly(quad(rect(u + 0.004, out - 0.05, u + th - 0.004, out - 0.004), rnd, 0.002)).fill(shade(c, 0.85));
      g.poly(quad(rect(u + 0.006, out - 0.032, u + th - 0.006, out - 0.024), rnd, 0.001)).fill(BRASS);
      u += th + 0.004;
    }
  }
  // Back panel along the wall, end panels and uprights reaching to the open front.
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, s0), DARK_WOOD, light, rnd, true, 0.025, 0);
  for (let sq = 0; sq <= len; sq++) {
    const u = sq === 0 ? END_GAP : sq === len ? len - END_GAP - 0.09 : sq - 0.05;
    const w = sq === 0 || sq === len ? 0.09 : 0.1;
    board(g, rect(u, WALL_GAP, u + w, s1 + 0.04), DARK_WOOD, light, rnd, false, 0.02, 0);
  }
}

/** Shelf against a wall: tiers stepping down from the wall to the room (the higher, back tier lit
 * brighter, the step casting a shadow on the one below), open at the front, crocks, jars, boxes and
 * bottles on each tier, end panels cut in steps. */
function shelf(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { len, rect, at } = frame(f, back);
  const [lx, ly] = light;
  const tiers: [number, number, number][] = [
    [WALL_GAP + 0.07, 0.56, 1.1],
    [0.56, 0.9, 0.88],
  ];
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, WALL_GAP + 0.08), DARK_WOOD, light, rnd, true, 0.025, 0);
  tiers.forEach(([a, b, lift], i) => {
    const t = WOOD.map((c) => shade(c, lift));
    board(g, rect(END_GAP + 0.04, a, len - END_GAP - 0.04, b), t, light, rnd, true, 0.03, 1);
    // The tier above throws its shadow onto this one.
    if (i > 0) g.poly(quad(rect(END_GAP + 0.04, a, len - END_GAP - 0.04, a + 0.05), rnd, 0)).fill({ color: 0x1a120a, alpha: 0.35 });
  });
  const pot = (u: number, s: number, r: number, t: number[]) => {
    const [cx, cy] = at(u, s);
    g.circle(cx, cy, r).fill(t[0]).stroke({ width: LINE, color: INK });
    g.circle(cx + lx * r * 0.15, cy + ly * r * 0.15, r * 0.78).fill(t[1]);
    g.circle(cx, cy, r * 0.4).fill(t[2]);
  };
  const goods = [
    (u: number, s: number, big: boolean) => pot(u, s, big ? 0.13 : 0.1, [0x6a4028, 0x9a6038, 0x3a2414]),
    (u: number, s: number) => pot(u, s, 0.08, [0x3a5a32, 0x5a8a4a, 0x8a6a3a]),
    (u: number, s: number, big: boolean) => {
      const h = big ? 0.13 : 0.1;
      g.poly(quad(rect(u - 0.12, s - h, u + 0.12, s + h), rnd, 0.008)).fill(0xa8865a).stroke({ width: LINE, color: INK });
      g.moveTo(...at(u - 0.12, s)).lineTo(...at(u + 0.12, s)).stroke({ width: 0.012, color: 0x6a4a2a });
    },
    (u: number, s: number, big: boolean) => pot(u, s, big ? 0.11 : 0.09, [0x8a8070, 0xc8c0a8, 0x8a6a3a]),
    (u: number, s: number) => pot(u, s, 0.06, [0x2e4a5a, 0x4a7a8a, 0x5a3a22]),
  ];
  let k = v;
  tiers.forEach(([a, b], i) => {
    const mid = (a + b) / 2 + (i ? 0.02 : 0);
    for (let u = END_GAP + 0.24 + i * 0.08; u < len - END_GAP - 0.16; u += 0.28 + (k % 2) * 0.04) {
      goods[k % goods.length](u, mid, i === 1);
      k += 2 + (k % 3);
    }
  });
  // End panels, cut in steps: full depth at the wall, lower toward the room.
  for (const u of [END_GAP, len - END_GAP - 0.07]) {
    board(g, rect(u, WALL_GAP, u + 0.07, 0.56), DARK_WOOD, light, rnd, false, 0.02, 0);
    board(g, rect(u, 0.56, u + 0.07, 0.9), WOOD.map((c) => shade(c, 0.9)), light, rnd, false, 0.02, 0);
  }
}

/** Stone tones: shaded side, face, lit edge. */
const STONE = [0x5e5a52, 0x8a857a, 0xa8a296];
const BRICK = [0x6a3a2a, 0x9a5a3e, 0xb87a5a];

/** Fitted stones filling a rect, in courses along x, each its own tone, an ink line round the lot. */
function masonry(g: Graphics, [x0, y0, x1, y1]: number[], t: readonly number[], rnd: () => number, course = 0.2) {
  g.poly(quad([x0, y0, x1, y1], rnd, 0.008)).fill(t[0]).stroke({ width: OUT, color: INK, join: 'round' });
  for (let y = y0; y < y1 - 0.01; y += course) {
    const yb = Math.min(y1, y + course);
    let x = x0 + (((y - y0) / course) % 2 ? -course * 0.6 : 0);
    while (x < x1 - 0.01) {
      const w = course * (1.2 + 0.8 * rnd());
      const xa = Math.max(x0, x);
      const xb = Math.min(x1, x + w);
      if (xb - xa > 0.03) g.rect(xa + 0.012, y + 0.012, xb - xa - 0.024, yb - y - 0.024).fill(shade(t[1], 0.9 + 0.2 * rnd()));
      x += w;
    }
  }
}

/** A fire in a hearth, along `sBase` from `u0` to `u1` (frame coordinates), reaching back toward `sTop`:
 * a bed of embers, then soft glowing ovals heaped on it, deep orange out to a pale yellow core. */
function hearthFire(g: Graphics, at: (u: number, s: number) => [number, number], u0: number, u1: number, sBase: number, sTop: number, rnd: () => number) {
  for (let i = 0; i < 18; i++) g.circle(...at(u0 + (u1 - u0) * rnd(), sTop + (sBase - sTop) * (0.3 + 0.8 * rnd())), 0.018 + 0.016 * rnd()).fill([0x7a2a10, 0xc2410c, 0xf97316, 0xffc23a][Math.floor(rnd() * 4)]);
  const n = 3;
  const sm = (sBase + sTop) / 2;
  const blobs: [number, number, number, number][] = [];
  for (let i = 0; i < n; i++) {
    const u = u0 + ((u1 - u0) * (i + 0.5)) / n + (rnd() - 0.5) * 0.04;
    const big = i === 1 ? 1.25 : 0.8 + 0.15 * rnd();
    const ru = ((u1 - u0) / n) * 0.72 * big;
    const rs = (sBase - sTop) * 0.4 * big;
    blobs.push([u, sm + (rnd() - 0.5) * 0.06, ru, rs]);
  }
  for (const [k, color, a] of [
    [1, 0xc2410c, 0.9],
    [0.74, 0xf28a1e, 1],
    [0.48, 0xffc23a, 1],
    [0.22, 0xfff2c0, 1],
  ]) {
    for (const [u, sc, ru, rs] of blobs) {
      const pts: number[] = [];
      for (let t = 0; t < 16; t++) {
        const ang = (t / 16) * Math.PI * 2;
        pts.push(...at(u + Math.cos(ang) * ru * k, sc + Math.sin(ang) * rs * k));
      }
      g.poly(pts).fill({ color, alpha: a });
    }
  }
}

/** Charred logs lying in a fire, their ends glowing. */
function logs(g: Graphics, cx: number, cy: number, r: number, along: boolean) {
  for (const d of [-0.35, 0.35]) {
    const [ax, ay, bx, by] = along ? [cx - r, cy + d * r, cx + r, cy + d * r * 0.6] : [cx + d * r, cy - r, cx + d * r * 0.6, cy + r];
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: r * 0.4 + 0.03, color: INK, cap: 'round' });
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: r * 0.4, color: 0x2a1c14, cap: 'round' });
    for (const [ex, ey] of [
      [ax, ay],
      [bx, by],
    ]) {
      g.circle(ex, ey, r * 0.15).fill(0xc2410c);
      g.circle(ex, ey, r * 0.07).fill(0xffc23a);
    }
  }
}

/** Hearth against a wall: a stone surround open toward the room, the firebox with logs and flames, a
 * flagstone apron in front. */
function hearth(g: Graphics, f: InteriorItem, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  // The apron, then the surround (back and two cheeks), then the firebox.
  const apron = rect(0.14, 0.62, len - 0.14, 0.96);
  g.poly(quad(apron, rnd, 0.01)).fill(STONE[1]).stroke({ width: LINE, color: INK });
  g.moveTo(...at(len / 2, 0.62)).lineTo(...at(len / 2 + (rnd() - 0.5) * 0.1, 0.96)).stroke({ width: LINE, color: STONE[0] });
  masonry(g, rect(0.06, WALL_GAP - 0.04, len - 0.06, 0.66), STONE, rnd, 0.16);
  const box = rect(0.38, WALL_GAP + 0.08, len - 0.38, 0.66);
  g.poly(quad(box, rnd, 0.006)).fill(0x1a1410).stroke({ width: LINE, color: INK });
  g.poly(quad(rect(0.42, WALL_GAP + 0.12, len - 0.42, 0.4), rnd, 0.006)).fill({ color: 0x2e2620, alpha: 0.9 });
  // Firelight on the firebox and the apron, then the flames rising off the logs toward the chimney.
  g.poly(quad(rect(0.4, WALL_GAP + 0.1, len - 0.4, 0.92), rnd, 0)).fill({ color: 0xf59e0b, alpha: 0.12 });
  // Charred logs under the fire, only their glowing ends showing past the flames.
  const [cx, cy] = at(len / 2, 0.5);
  logs(g, cx, cy, Math.min(0.26, len * 0.15), along);
  hearthFire(g, at, 0.5, len - 0.5, 0.6, WALL_GAP + 0.1, rnd);
}

/** Bread oven against a wall: a domed brick oven, its arched mouth toward the room glowing with embers,
 * a stone sill in front. */
function oven(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { len, rect, at } = frame(f, back);
  const [lx, ly] = light;
  const [cx, cy] = at(len / 2, 0.48);
  const r = Math.min(len / 2 - 0.1, 0.42);
  // The dome: rings of brick, darker away from the light.
  g.poly(ring(cx, cy, r, rnd, 0.02, 24)).fill(BRICK[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(ring(cx + lx * r * 0.08, cy + ly * r * 0.08, r * 0.86, rnd, 0.015, 24)).fill(BRICK[1]);
  for (const k of [0.62, 0.34]) g.poly(ring(cx + lx * r * 0.1, cy + ly * r * 0.1, r * k, rnd, 0.01, 20)).stroke({ width: LINE, color: BRICK[0], alpha: 0.8 });
  for (let i = 0; i < 10; i++) {
    const a = (i / 10) * Math.PI * 2 + rnd() * 0.2;
    g.moveTo(cx + Math.cos(a) * r * 0.62, cy + Math.sin(a) * r * 0.62).lineTo(cx + Math.cos(a) * r * 0.86, cy + Math.sin(a) * r * 0.86);
  }
  g.stroke({ width: 0.015, color: BRICK[0], alpha: 0.8 });
  g.poly(ring(cx + lx * r * 0.2, cy + ly * r * 0.2, r * 0.22, rnd, 0.01, 12)).fill({ color: BRICK[2], alpha: 0.6 });
  // The mouth and its sill, toward the room.
  const mouth = rect(len / 2 - 0.16, 0.8, len / 2 + 0.16, 0.92);
  g.poly(quad(rect(len / 2 - 0.26, 0.84, len / 2 + 0.26, 0.98), rnd, 0.008)).fill(STONE[1]).stroke({ width: LINE, color: INK });
  g.poly(quad(mouth, rnd, 0.006)).fill(0x1a1008).stroke({ width: LINE, color: INK });
  const [mx, my] = at(len / 2, 0.86);
  g.circle(mx, my, 0.1).fill({ color: 0xf59e0b, alpha: 0.25 });
  for (let i = 0; i < 5; i++) g.circle(mx + (rnd() - 0.5) * 0.2, my + (rnd() - 0.5) * 0.06, 0.018).fill(rnd() < 0.5 ? 0xf97316 : 0xffc23a);
}

/** Smith's forge against a wall: a block of grey brick (the chimney breast) with a half-round stone
 * basin in front of it, opening toward the room and heaped with glowing coals; a water trough beside it. */
function forge(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const fr = frame(f, back);
  const { along, len } = fr;
  // Laid out two squares deep; a shallower forge is the same squeezed across (the wall gap kept).
  const k = (along ? f.h : f.w) / 2;
  const across = (s: number) => (s <= WALL_GAP ? s : WALL_GAP + (s - WALL_GAP) * k);
  const at = (u: number, s: number) => fr.at(u, across(s));
  const rect = (u0: number, s0: number, u1: number, s1: number) => fr.rect(u0, across(s0), u1, across(s1));
  // Trough of water at the far end.
  board(g, rect(len - 0.5, 0.7, len - 0.1, 1.55), DARK_WOOD, light, rnd, !along, 0.04, 0);
  g.poly(quad(rect(len - 0.44, 0.76, len - 0.16, 1.49), rnd, 0.004)).fill(0x2e4a5a);
  g.poly(quad(rect(len - 0.4, 0.8, len - 0.3, 1.2), rnd, 0.003)).fill({ color: 0x8ab8c8, alpha: 0.5 });
  // The basin: a half-round of stones out from the brick, the coals inside it.
  // Centred on the brick block (u 0.1 to len - 0.62).
  const [cu, cs, R] = [(0.1 + len - 0.62) / 2, 1.0, Math.min(0.62, (len - 0.72) / 2)];
  const half = (r: number, jitter: number) => {
    const pts: number[] = [];
    for (let i = 0; i <= 16; i++) {
      const t = (i / 16) * Math.PI;
      const rr = r + (rnd() - 0.5) * jitter;
      pts.push(...at(cu + Math.cos(t) * rr, cs + Math.sin(t) * rr));
    }
    return pts;
  };
  g.poly(half(R, 0.02)).fill(STONE[1]).stroke({ width: OUT, color: INK, join: 'round' });
  for (let i = 1; i < 9; i++) {
    const t = (i / 9) * Math.PI;
    g.moveTo(...at(cu + Math.cos(t) * (R - 0.12), cs + Math.sin(t) * (R - 0.12))).lineTo(...at(cu + Math.cos(t) * R, cs + Math.sin(t) * R));
  }
  g.stroke({ width: LINE, color: STONE[0] });
  const coals = half(R - 0.12, 0.015);
  g.poly(coals).fill(0x7a1e0e).stroke({ width: LINE, color: INK, join: 'round' });
  for (let i = 0; i < 70; i++) {
    const t = rnd() * Math.PI;
    const d = (R - 0.16) * Math.sqrt(rnd());
    const [x, y] = at(cu + Math.cos(t) * d, cs + Math.sin(t) * d);
    g.circle(x, y, 0.025 + 0.02 * rnd()).fill([0x9a2a12, 0xc2410c, 0xd8501a, 0xf97316, 0xffb238][Math.floor(rnd() * rnd() * 5)]);
  }
  g.poly(half((R - 0.12) * 0.5, 0.01)).fill({ color: 0xffb238, alpha: 0.25 });
  // The brick block against the wall, over the basin's straight edge.
  masonry(g, rect(0.1, WALL_GAP - 0.04, len - 0.62, cs + 0.02), STONE.map((c) => shade(c, 0.85)), rnd, 0.17);
}

/** Anvil on a stump: the stump's cut face, then the anvil from above — a flat face with a square heel,
 * the horn tapering to a point, a hardy hole — its face lit. */
function anvil(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  g.poly(ring(cx, cy, 0.36, rnd, 0.03, 16)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.circle(cx, cy, 0.29).fill(0xa8865a);
  g.circle(cx, cy, 0.18).stroke({ width: 0.015, color: WOOD[0], alpha: 0.7 });
  // Feet first (wider than the face), then the face and horn.
  g.poly([cx - 0.12, cy - 0.16, cx + 0.26, cy - 0.16, cx + 0.26, cy + 0.16, cx - 0.12, cy + 0.16]).fill(IRON[0]).stroke({ width: LINE, color: INK });
  const top = [cx - 0.12, cy - 0.1, cx + 0.3, cy - 0.1, cx + 0.3, cy + 0.1, cx - 0.12, cy + 0.1, cx - 0.2, cy + 0.06, cx - 0.4, cy, cx - 0.2, cy - 0.06];
  g.poly(top).fill(IRON[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([cx - 0.1 + lx * 0.01, cy - 0.08 + ly * 0.01, cx + 0.28, cy - 0.08 + ly * 0.01, cx + 0.28, cy - 0.04, cx - 0.1, cy - 0.04]).fill({ color: IRON[2], alpha: 0.8 });
  g.moveTo(cx - 0.12, cy - 0.1).lineTo(cx - 0.12, cy + 0.1).stroke({ width: 0.015, color: IRON[0] });
  g.rect(cx + 0.18, cy - 0.025, 0.05, 0.05).fill(INK);
}

/** Workbench against a wall: a thick plank top with dog holes, a vise at one end, a rail of tools hung on
 * the wall behind it. */
function workbench(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const face = board(g, rect(0.08, WALL_GAP + 0.04, len - 0.08, 0.92), WOOD, light, rnd, along, 0.05, 2);
  planks(g, face, along, 3, WOOD[0], rnd);
  for (let u = 0.4; u < len - 0.3; u += 0.35) g.circle(...at(u, 0.82), 0.025).fill(0x2a1c10);
  // The vise at the room-side end.
  board(g, rect(len - 0.36, 0.8, len - 0.14, 0.98), IRON, light, rnd, along, 0.025, 0);
  g.moveTo(...at(len - 0.25, 0.98)).lineTo(...at(len - 0.25, 1.06)).stroke({ width: 0.03, color: IRON[2], cap: 'round' });
  // Tools hung on the wall rail: a saw, a mallet, chisels.
  g.moveTo(...at(0.15, WALL_GAP - 0.02)).lineTo(...at(len - 0.15, WALL_GAP - 0.02)).stroke({ width: 0.04, color: DARK_WOOD[1] });
  const [sx, sy] = at(0.4, WALL_GAP + 0.02);
  const [tx, ty] = at(0.8, WALL_GAP + 0.1);
  g.poly([sx, sy, tx, ty, ...at(0.8, WALL_GAP + 0.02)]).fill(IRON[2]).stroke({ width: LINE, color: INK });
  g.circle(...at(1.15, WALL_GAP + 0.08), 0.06).fill(WOOD[1]).stroke({ width: LINE, color: INK });
  g.moveTo(...at(1.15, WALL_GAP + 0.02)).lineTo(...at(1.15, WALL_GAP + 0.03)).stroke({ width: 0.02, color: INK });
  for (const u of [len - 0.6, len - 0.5]) g.moveTo(...at(u, WALL_GAP)).lineTo(...at(u, WALL_GAP + 0.14)).stroke({ width: 0.025, color: IRON[2], cap: 'round' });
}

/** Shop counter against a wall: a long, worn top board (paler where hands rest along the front), a
 * drawer seam, the gap behind it for the keeper. */
function counter(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect } = frame(f, back);
  const face = board(g, rect(0.06, 0.48, len - 0.06, 0.94), WOOD, light, rnd, along, 0.05, 2);
  planks(g, face, along, 2, WOOD[0], rnd);
  g.poly(quad(rect(0.2, 0.8, len - 0.2, 0.88), rnd, 0.004)).fill({ color: WOOD[2], alpha: 0.5 });
  // A low shelf against the wall behind, for stock.
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, WALL_GAP + 0.14), DARK_WOOD, light, rnd, along, 0.03, 1);
}

/** Bar against a wall: a long, dark, polished counter toward the room, a back shelf of bottles along the
 * wall, the barkeep's aisle between them. */
function bar(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const shelfR = rect(END_GAP, WALL_GAP, len - END_GAP, WALL_GAP + 0.16);
  board(g, shelfR, DARK_WOOD, light, rnd, along, 0.03, 0);
  const glass = [0x3a6a3a, 0x6a3a2a, 0x2e4a6a, 0xa87a3a, 0x5a3a5e];
  for (let u = END_GAP + 0.1, k = 0; u < len - END_GAP - 0.06; u += 0.11, k++) {
    const [bx, by] = at(u, WALL_GAP + 0.08);
    g.circle(bx, by, 0.04).fill(glass[(k * 3) % glass.length]).stroke({ width: 0.012, color: INK });
    g.circle(bx - 0.012, by - 0.012, 0.012).fill({ color: 0xffffff, alpha: 0.5 });
  }
  const face = board(g, rect(0.06, 0.56, len - 0.06, 0.94), DARK_WOOD.map((c) => shade(c, 1.15)), light, rnd, along, 0.05, 2);
  planks(g, face, along, 2, DARK_WOOD[0], rnd);
  // A brass foot-rail along the front.
  g.moveTo(...at(0.1, 0.99)).lineTo(...at(len - 0.1, 0.99)).stroke({ width: 0.025, color: BRASS });
}

/** A barrel standing on end: a ring of staves, two iron hoops, a planked lid with a bung, lit on its
 * light side. */
function barrel(g: Graphics, cx: number, cy: number, r: number, light: Light, rnd: () => number) {
  const [lx, ly] = light;
  g.poly(ring(cx, cy, r, rnd, 0.01, 18)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * r * 0.7, cy + Math.sin(a) * r * 0.7).lineTo(cx + Math.cos(a) * r * 0.97, cy + Math.sin(a) * r * 0.97);
  }
  g.stroke({ width: 0.012, color: shade(WOOD[0], 0.7) });
  g.circle(cx, cy, r * 0.86).stroke({ width: 0.045, color: IRON[0] });
  const la = Math.atan2(ly, lx);
  g.moveTo(cx + Math.cos(la - 0.9) * r * 0.86, cy + Math.sin(la - 0.9) * r * 0.86)
    .arc(cx, cy, r * 0.86, la - 0.9, la + 0.9)
    .stroke({ width: 0.015, color: IRON[2] });
  g.circle(cx + lx * r * 0.06, cy + ly * r * 0.06, r * 0.68).fill(WOOD[1]).stroke({ width: LINE, color: IRON[0] });
  for (const d of [-0.3, 0.3]) g.moveTo(cx - r * 0.6, cy + d * r).lineTo(cx + r * 0.6, cy + d * r);
  g.stroke({ width: 0.012, color: WOOD[0], alpha: 0.85 });
  g.circle(cx + r * 0.3, cy - r * 0.1, r * 0.08).fill(0x2a1c10);
}

/** A keg lying on its side in a cradle, its tap toward the room: staves bulging between flat ends,
 * iron hoops round it, the side toward the light paler. */
function keg(g: Graphics, f: InteriorItem, u: number, light: Light, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  // Cradle rails across under it, at either end.
  for (const s0 of [WALL_GAP + 0.04, 0.74]) board(g, rect(u + 0.06, s0, u + 0.94, s0 + 0.1), DARK_WOOD, light, rnd, true, 0.02, 0);
  const [e0, e1] = [WALL_GAP + 0.02, 0.92];
  const [a, b] = [u + 0.2, u + 0.8];
  // The body: flat ends, sides bowed out past the ends' width.
  const body = () => {
    g.moveTo(...at(a, e0))
      .quadraticCurveTo(...at(u + 0.08, (e0 + e1) / 2), ...at(a, e1))
      .lineTo(...at(b, e1))
      .quadraticCurveTo(...at(u + 0.92, (e0 + e1) / 2), ...at(b, e0))
      .closePath();
  };
  body();
  g.fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(quad(rect(u + 0.24, e0 + 0.03, u + 0.76, e1 - 0.03), rnd, 0.004)).fill(WOOD[1]);
  g.poly(quad(rect(u + 0.38, e0 + 0.03, u + 0.6, e1 - 0.03), rnd, 0.004)).fill({ color: WOOD[2], alpha: 0.7 });
  for (const k of [0.32, 0.44, 0.56, 0.68]) g.moveTo(...at(u + k, e0 + 0.03)).lineTo(...at(u + k, e1 - 0.03));
  g.stroke({ width: 0.01, color: WOOD[0], alpha: 0.7 });
  // Hoops: bowed bands across, near each end and either side of the middle.
  for (const s0 of [e0 + 0.08, (e0 + e1) / 2 - 0.08, (e0 + e1) / 2 + 0.08, e1 - 0.08]) {
    const bulge = 0.12 * (1 - Math.abs(s0 - (e0 + e1) / 2) / ((e1 - e0) / 2)) + 0.02;
    g.moveTo(...at(a - bulge * 0.9, s0)).lineTo(...at(b + bulge * 0.9, s0));
  }
  g.stroke({ width: 0.03, color: IRON[0] });
  g.moveTo(...at(u + 0.5, e1)).lineTo(...at(u + 0.5, e1 + 0.07)).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.moveTo(...at(u + 0.5, e1)).lineTo(...at(u + 0.5, e1 + 0.07)).stroke({ width: 0.022, color: BRASS, cap: 'round' });
}

/** Crate: a frame of boards round three planks, nails in the corners, a cross brace on some. */
function crate(g: Graphics, x0: number, y0: number, sz: number, light: Light, rnd: () => number, braced: boolean) {
  const face = board(g, [x0, y0, x0 + sz, y0 + sz], [0x6e4e2e, 0x9a7448, 0xae8858], light, rnd, true, 0.04, 0);
  planks(g, face, true, 3, 0x5e4028, rnd);
  const e = sz * 0.12;
  const [fx0, fy0, fx1, fy1] = face;
  g.rect(fx0, fy0, fx1 - fx0, fy1 - fy0).stroke({ width: e, color: 0x7e5c38, alignment: 1 });
  if (braced) g.moveTo(fx0 + e, fy1 - e).lineTo(fx1 - e, fy0 + e).stroke({ width: e * 0.8, color: 0x7e5c38 });
  for (const [nx, ny] of [
    [fx0 + e / 2, fy0 + e / 2],
    [fx1 - e / 2, fy0 + e / 2],
    [fx1 - e / 2, fy1 - e / 2],
    [fx0 + e / 2, fy1 - e / 2],
  ]) {
    g.circle(nx, ny, 0.012).fill(0x2a2a28);
  }
}

/** Crates: one with a brace, or (on two by two) four in a block with one more stacked on top in the
 * middle, all the same size, a crate missing from the block on some. */
function crates(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { x, y, w, h } = f;
  if (w === 1 && h === 1) {
    crate(g, x + 0.16, y + 0.16, 0.68, light, rnd, v % 2 === 0);
    return;
  }
  const [sz, gap] = [0.84, 0.05];
  const [ox, oy] = [x + (w - 2 * sz - gap) / 2, y + (h - 2 * sz - gap) / 2];
  const skip = v % 2 ? Math.floor(v / 2) % 4 : -1;
  for (let i = 0; i < 4; i++) if (i !== skip) crate(g, ox + (i % 2) * (sz + gap), oy + Math.floor(i / 2) * (sz + gap), sz, light, rnd, (i + v) % 2 === 0);
  crate(g, x + w / 2 - sz / 2, y + h / 2 - sz / 2, sz, light, rnd, v % 2 === 1);
}

/** Upholstery: shade, cloth, lit. */
const PLUSH = [
  [0x5a2a2a, 0x8a3a34, 0xa85a4a],
  [0x2a3e5a, 0x3e5a7a, 0x5a7a9a],
  [0x3a4a28, 0x5a6a3a, 0x7a8a52],
  [0x4a2e4a, 0x6a4268, 0x8a5e86],
];

/** An item laid out lengthwise from one end: `at(u, s)` is `u` squares from the end at `head` (a unit
 * step) and `s` across. */
function lengthwise(f: InteriorItem, head: [number, number]) {
  const along = head[0] !== 0;
  const flip = head[0] + head[1] > 0;
  const len = along ? f.w : f.h;
  const wid = along ? f.h : f.w;
  const at = (u: number, s: number): [number, number] => {
    const uu = flip ? len - u : u;
    return along ? [f.x + uu, f.y + s] : [f.x + s, f.y + uu];
  };
  const rect = (u0: number, s0: number, u1: number, s1: number): number[] => {
    const [ax, ay] = at(u0, s0);
    const [bx, by] = at(u1, s1);
    return [Math.min(ax, bx), Math.min(ay, by), Math.max(ax, bx), Math.max(ay, by)];
  };
  return { along, len, wid, at, rect };
}

/** Bed, its head against the wall at `head`: a wooden frame and headboard, the sheet, a pillow each
 * (two on a double), a blanket with its top turned down and a fold or two. */
function bed(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, head: [number, number]) {
  const { along, len, wid, rect, at } = lengthwise(f, head);
  const h0 = WALL_GAP - 0.1;
  board(g, rect(h0 + 0.02, 0.08, len - 0.06, wid - 0.08), WOOD, light, rnd, along, 0.05, 1);
  board(g, rect(h0, 0.06, h0 + 0.18, wid - 0.06), DARK_WOOD, light, rnd, !along, 0.04, 1);
  const [sx0, sy0, sx1, sy1] = rect(h0 + 0.2, 0.16, len - 0.12, wid - 0.16);
  g.roundRect(sx0, sy0, sx1 - sx0, sy1 - sy0, 0.04).fill(LINEN[1]).stroke({ width: LINE, color: INK });
  const pillows = wid >= 2 ? 2 : 1;
  for (let i = 0; i < pillows; i++) {
    const s0 = 0.22 + (i * (wid - 0.44)) / pillows;
    const s1 = 0.22 + ((i + 1) * (wid - 0.44)) / pillows - (pillows > 1 ? 0.06 : 0);
    const [px0, py0, px1, py1] = rect(h0 + 0.26, s0, h0 + 0.58, s1);
    g.roundRect(px0, py0, px1 - px0, py1 - py0, 0.08).fill(LINEN[2]).stroke({ width: LINE, color: INK });
    g.moveTo(...at(h0 + 0.42, s0 + 0.12)).lineTo(...at(h0 + 0.42, s1 - 0.12)).stroke({ width: 0.015, color: LINEN[0] });
  }
  const dye = DYES[v % DYES.length];
  const top = h0 + 0.72;
  const [qx0, qy0, qx1, qy1] = rect(top, 0.12, len - 0.1, wid - 0.12);
  g.roundRect(qx0, qy0, qx1 - qx0, qy1 - qy0, 0.05).fill(dye).stroke({ width: LINE * 1.3, color: INK });
  const [tx0, ty0, tx1, ty1] = rect(top, 0.12, top + 0.2, wid - 0.12);
  g.roundRect(tx0, ty0, tx1 - tx0, ty1 - ty0, 0.04).fill(shade(dye, 1.25)).stroke({ width: LINE, color: INK });
  for (const u of [len * 0.62, len * 0.8]) {
    g.moveTo(...at(u, 0.2)).quadraticCurveTo(...at(u - 0.04, wid * 0.4), ...at(u + 0.08, wid * 0.6)).stroke({ width: 0.02, color: shade(dye, 0.7) });
  }
}

/** Couch against a wall: a wooden frame, arms at the ends, a back along the wall, seat cushions. */
function couch(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const t = PLUSH[v % PLUSH.length];
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.94), DARK_WOOD, light, rnd, along, 0.03, 0);
  const pad = (r: number[], c: readonly number[]) => {
    g.roundRect(r[0], r[1], r[2] - r[0], r[3] - r[1], 0.06).fill(c[0]).stroke({ width: LINE, color: INK });
    g.roundRect(r[0] + 0.03 + light[0] * 0.015, r[1] + 0.03 + light[1] * 0.015, r[2] - r[0] - 0.06, r[3] - r[1] - 0.06, 0.05).fill(c[1]);
  };
  pad(rect(END_GAP + 0.04, WALL_GAP + 0.03, len - END_GAP - 0.04, WALL_GAP + 0.13), t);
  for (const u of [END_GAP + 0.04, len - END_GAP - 0.2]) pad(rect(u, WALL_GAP + 0.03, u + 0.16, 0.9), t);
  const n = Math.max(2, Math.round(len));
  const [u0, u1] = [END_GAP + 0.22, len - END_GAP - 0.22];
  for (let i = 0; i < n; i++) {
    const a = u0 + ((u1 - u0) * i) / n;
    const b = u0 + ((u1 - u0) * (i + 1)) / n;
    pad(rect(a + 0.01, WALL_GAP + 0.15, b - 0.01, 0.88), t.map((c) => shade(c, 1.05)));
    g.moveTo(...at((a + b) / 2, WALL_GAP + 0.3)).lineTo(...at((a + b) / 2 + 0.01, 0.74)).stroke({ width: 0.012, color: t[0], alpha: 0.7 });
  }
}

/** Sideboard against a wall: a long cabinet top with a moulded rim and an inlaid border, the doors'
 * edge at the front with brass pulls. */
function sideboard(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const face = board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.74), DARK_WOOD, light, rnd, along, 0.05, 2);
  g.rect(face[0] + 0.06, face[1] + 0.06, face[2] - face[0] - 0.12, face[3] - face[1] - 0.12).stroke({ width: 0.015, color: DARK_WOOD[2], alpha: 0.8 });
  g.poly(quad(rect(END_GAP + 0.02, 0.74, len - END_GAP - 0.02, 0.8), rnd, 0.004)).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK });
  const doors = Math.max(2, Math.round(len) * 2);
  for (let i = 1; i < doors; i++) g.moveTo(...at(END_GAP + ((len - 2 * END_GAP) * i) / doors, 0.74)).lineTo(...at(END_GAP + ((len - 2 * END_GAP) * i) / doors, 0.8));
  g.stroke({ width: 0.012, color: INK });
  for (let i = 0; i < doors; i++) g.circle(...at(END_GAP + ((len - 2 * END_GAP) * (i + 0.5)) / doors, 0.77), 0.018).fill(BRASS);
}

/** Desk: a dark wooden top with a green leather writing surface; papers on it, an ink jar and a quill. */
function desk(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  board(g, rect(0.08, 0.12, len - 0.08, 0.88), DARK_WOOD, light, rnd, along, 0.05, 2);
  g.poly(quad(rect(0.3, 0.26, len - 0.3, 0.74), rnd, 0.004)).fill(0x3e5a3a).stroke({ width: LINE, color: 0x2a3a26 });
  // Papers: a few sheets, a little askew, lines of writing on the top one.
  const [px, py] = at(len * 0.45, 0.5);
  for (let i = 0; i < 3; i++) {
    const a = (rnd() - 0.5) * 0.5 + (along ? 0 : Math.PI / 2);
    const [c, sn] = [Math.cos(a), Math.sin(a)];
    const [hw, hh] = [0.17, 0.12];
    const ox = px + (rnd() - 0.5) * 0.08;
    const oy = py + (rnd() - 0.5) * 0.06;
    const pt = (u: number, v: number) => [ox + u * c - v * sn, oy + u * sn + v * c];
    g.poly([...pt(-hw, -hh), ...pt(hw, -hh), ...pt(hw, hh), ...pt(-hw, hh)]).fill(i === 2 ? LINEN[2] : LINEN[1]).stroke({ width: 0.012, color: INK });
    if (i === 2) {
      for (let k = 0; k < 4; k++) {
        const v = -hh * 0.6 + k * hh * 0.4;
        g.moveTo(...(pt(-hw * 0.75, v) as [number, number])).lineTo(...(pt(hw * (0.35 + 0.35 * rnd()), v) as [number, number]));
      }
      g.stroke({ width: 0.01, color: 0x5a4a3a, alpha: 0.8 });
    }
  }
  // The ink jar and a quill lying beside it.
  const [jx, jy] = at(len - 0.42, 0.36);
  g.circle(jx, jy, 0.07).fill(0x2a2a3a).stroke({ width: LINE, color: INK });
  g.circle(jx, jy, 0.035).fill(0x0e0e16);
  g.circle(jx + light[0] * 0.03, jy + light[1] * 0.03, 0.015).fill({ color: 0xffffff, alpha: 0.5 });
  const [qa, qb] = [at(len - 0.5, 0.66), at(len - 0.2, 0.46)];
  const [dx, dy] = [qb[0] - qa[0], qb[1] - qa[1]];
  const m = Math.hypot(dx, dy) || 1;
  const [nx, ny] = [-dy / m, dx / m];
  const mid = [qa[0] + dx * 0.55, qa[1] + dy * 0.55];
  g.poly([qa[0], qa[1], mid[0] + nx * 0.045, mid[1] + ny * 0.045, qb[0], qb[1], mid[0] - nx * 0.025, mid[1] - ny * 0.025]).fill(LINEN[2]).stroke({ width: 0.012, color: INK, join: 'round' });
  g.moveTo(qa[0], qa[1]).lineTo(qb[0], qb[1]).stroke({ width: 0.01, color: LINEN[0] });
}

/** Display table in a shop: a cloth over it, hanging past the edges, and wares laid out on it. */
function display(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { len, rect, at } = frame(f);
  const cloth = shade(DYES[(v + 1) % DYES.length], 0.8);
  g.poly(quad(rect(0.06, 0.08, len - 0.06, 0.92), rnd, 0.01)).fill(shade(cloth, 0.75)).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(quad(rect(0.14, 0.16, len - 0.14, 0.84), rnd, 0.006)).fill(cloth);
  for (const u of [0.3, len - 0.3]) g.moveTo(...at(u, 0.08)).lineTo(...at(u + 0.02, 0.16));
  g.stroke({ width: 0.015, color: shade(cloth, 0.6) });
  // Wares: rolls of cloth, a pot, small boxes.
  for (let u = 0.35, k = v; u < len - 0.2; u += 0.42, k++) {
    if (k % 3 === 0) {
      const r = rect(u - 0.14, 0.3, u + 0.14, 0.7);
      g.poly(quad(r, rnd, 0.004)).fill([0xc8a060, 0x6a8aa8, 0xa86a5a][k % 3 === 0 ? (k / 3) % 3 : 0]).stroke({ width: LINE, color: INK });
    } else if (k % 3 === 1) {
      const [cx, cy] = at(u, 0.5);
      g.circle(cx, cy, 0.13).fill(0x8a5a32).stroke({ width: LINE, color: INK });
      g.circle(cx + light[0] * 0.02, cy + light[1] * 0.02, 0.1).fill(0xa87040);
      g.circle(cx, cy, 0.05).fill(0x3a2414);
    } else {
      for (const s of [0.36, 0.62]) board(g, rect(u - 0.1, s - 0.1, u + 0.1, s + 0.1), [0x7a5a3a, 0xa88a5a, 0xc0a070], light, rnd, true, 0.02, 0);
    }
  }
}

/** Pew: a long seat with a high back on the side away from the altar, carved ends. */
function pew(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect } = frame(f, back);
  for (const u of [0.04, len - 0.14]) board(g, rect(u, 0.12, u + 0.1, 0.84), DARK_WOOD, light, rnd, !along, 0.02, 0);
  const seat = board(g, rect(0.12, 0.32, len - 0.12, 0.8), WOOD, light, rnd, along, 0.04, 2);
  planks(g, seat, along, 2, WOOD[0], rnd);
  board(g, rect(0.12, 0.14, len - 0.12, 0.3), DARK_WOOD, light, rnd, along, 0.03, 1);
}

/** Booth table: a square table, plain worn planks. */
function boothTable(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const face = board(g, [x + 0.08, y + 0.08, x + 0.92, y + 0.92], WOOD, light, rnd, true, 0.05, 1);
  planks(g, face, true, 3, WOOD[0], rnd);
}

/** Booth seat: an upholstered bench half a square deep, its high back on the side away from its
 * table, always in the same red. The corner seat (`back` a diagonal step) is an L along both walls,
 * so the bench runs on round the corner. */
function boothSeat(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const t = PLUSH[0];
  const pad = (r: number[], c: readonly number[]) => {
    g.roundRect(r[0], r[1], r[2] - r[0], r[3] - r[1], 0.05).fill(c[0]).stroke({ width: LINE, color: INK });
    g.roundRect(r[0] + 0.025 + light[0] * 0.012, r[1] + 0.025 + light[1] * 0.012, r[2] - r[0] - 0.05, r[3] - r[1] - 0.05, 0.04).fill(c[1]);
  };
  if (back[0] !== 0 && back[1] !== 0) {
    // (u from the wall at x's back, v from the wall at y's back.)
    const { x, y } = f;
    const px = (u: number) => (back[0] < 0 ? x + u : x + 1 - u);
    const py = (v: number) => (back[1] < 0 ? y + v : y + 1 - v);
    const r = (u0: number, v0: number, u1: number, v1: number) => [Math.min(px(u0), px(u1)), Math.min(py(v0), py(v1)), Math.max(px(u0), px(u1)), Math.max(py(v0), py(v1))];
    g.poly([px(0.06), py(0.06), px(0.94), py(0.06), px(0.94), py(0.5), px(0.5), py(0.5), px(0.5), py(0.94), px(0.06), py(0.94)])
      .fill(DARK_WOOD[1])
      .stroke({ width: LINE, color: INK, join: 'round' });
    pad(r(0.09, 0.09, 0.9, 0.22), t);
    pad(r(0.09, 0.22, 0.22, 0.9), t);
    pad(r(0.23, 0.23, 0.9, 0.47), t.map((c) => shade(c, 1.06)));
    pad(r(0.23, 0.47, 0.47, 0.9), t.map((c) => shade(c, 1.06)));
    return;
  }
  const { rect } = frame(f, back);
  board(g, rect(0.06, 0.06, 0.94, 0.5), DARK_WOOD, light, rnd, true, 0.03, 0);
  pad(rect(0.1, 0.09, 0.9, 0.22), t);
  pad(rect(0.12, 0.23, 0.88, 0.47), t.map((c) => shade(c, 1.06)));
}

/** Rug: a woven rug in two dyes, a border and a pattern of diamonds down the middle, fringed ends. */
function rug(g: Graphics, f: InteriorItem, v: number, rnd: () => number) {
  const { len, rect, at } = frame(f);
  const wid = f.w >= f.h ? f.h : f.w;
  const [ground, border, motif] = [
    [0x8a2e26, 0x2e4a5a, 0xc8a45a],
    [0x2e4a6a, 0xa8742e, 0xe0cca0],
    [0x6a4a2a, 0x8a2e26, 0xd0b070],
    [0x4a5a32, 0x6a2e3a, 0xd8c08a],
  ][v % 4];
  // Fringes at the ends.
  for (const u of [0.1, len - 0.1]) {
    for (let s = 0.2; s < wid - 0.15; s += 0.06) g.moveTo(...at(u, s)).lineTo(...at(u + (u < 1 ? -0.07 : 0.07), s));
  }
  g.stroke({ width: 0.012, color: 0xe0d0b0 });
  g.poly(quad(rect(0.12, 0.12, len - 0.12, wid - 0.12), rnd, 0.006)).fill(border).stroke({ width: LINE, color: INK });
  g.poly(quad(rect(0.22, 0.22, len - 0.22, wid - 0.22), rnd, 0.004)).fill(ground);
  g.poly(quad(rect(0.27, 0.27, len - 0.27, wid - 0.27), rnd, 0.002)).stroke({ width: 0.015, color: motif, alpha: 0.8 });
  // Diamonds down the middle.
  const n = Math.max(1, Math.round(len * 1.2));
  for (let i = 0; i < n; i++) {
    const u = 0.3 + ((len - 0.6) * (i + 0.5)) / n;
    const r = Math.min(0.2, (len - 0.6) / n / 2.4, wid * 0.18);
    g.poly([...at(u - r, wid / 2), ...at(u, wid / 2 - r), ...at(u + r, wid / 2), ...at(u, wid / 2 + r)]).fill(motif);
    g.poly([...at(u - r * 0.45, wid / 2), ...at(u, wid / 2 - r * 0.45), ...at(u + r * 0.45, wid / 2), ...at(u, wid / 2 + r * 0.45)]).fill(border);
  }
}

/** Bath: a wooden tub bound with iron hoops, oval (or round, on two by two), full of water with a glint. */
function bath(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  if (f.w !== f.h) return longBath(g, f, light, rnd);
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [rx, ry] = [f.w / 2 - 0.1, f.h / 2 - 0.1];
  const oval = (kx: number, ky: number, dx = 0, dy = 0) => {
    const pts: number[] = [];
    for (let i = 0; i < 28; i++) {
      const a = (i / 28) * Math.PI * 2;
      pts.push(cx + dx + Math.cos(a) * rx * kx, cy + dy + Math.sin(a) * ry * ky);
    }
    return pts;
  };
  g.poly(oval(1, 1)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  // The tub's wall: a thin rim of staves, lit toward the light, an iron hoop round it.
  const t = 0.07;
  const [kx, ky] = [1 - t / rx, 1 - t / ry];
  g.poly(oval(1 - (t * 0.5) / rx, 1 - (t * 0.5) / ry, light[0] * 0.015, light[1] * 0.015)).fill(WOOD[1]);
  for (let i = 0; i < 32; i++) {
    const a = (i / 32) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * rx * kx, cy + Math.sin(a) * ry * ky).lineTo(cx + Math.cos(a) * rx * 0.99, cy + Math.sin(a) * ry * 0.99);
  }
  g.stroke({ width: 0.008, color: WOOD[0] });
  g.poly(oval(1 - (t * 0.5) / rx, 1 - (t * 0.5) / ry)).stroke({ width: 0.018, color: IRON[0] });
  g.poly(oval(kx, ky)).fill(0x3e6a7a).stroke({ width: LINE, color: INK });
  g.poly(oval(kx * 0.7, ky * 0.65, light[0] * rx * 0.12, light[1] * ry * 0.12)).fill({ color: 0x5a8a9a, alpha: 0.8 });
  g.poly(oval(0.25, 0.08, light[0] * rx * 0.3, light[1] * ry * 0.3)).fill({ color: 0xffffff, alpha: 0.45 });
}

/** A long bath: a rectangular wooden tub, its thin plank walls bound with iron at the ends, full of water. */
function longBath(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect } = frame(f);
  const outer = rect(0.1, 0.1, len - 0.1, 0.9);
  board(g, outer, WOOD, light, rnd, along, 0.03, 0);
  const water = rect(0.17, 0.17, len - 0.17, 0.83);
  g.poly(quad(water, rnd, 0.004)).fill(0x3e6a7a).stroke({ width: LINE, color: INK });
  const [w0, w1, w2, w3] = water;
  g.poly(quad([w0 + 0.06, w1 + 0.06, w2 - 0.06, w3 - 0.06], rnd, 0.003)).fill({ color: 0x5a8a9a, alpha: 0.7 });
  const gw = along ? [w0 + 0.12, w1 + 0.08, w0 + 0.42, w1 + 0.12] : [w0 + 0.08, w1 + 0.12, w0 + 0.12, w1 + 0.42];
  g.rect(gw[0], gw[1], gw[2] - gw[0], gw[3] - gw[1]).fill({ color: 0xffffff, alpha: 0.4 });
  for (const u of [0.22, len - 0.28]) {
    for (const s0 of [0.1, 0.83]) g.poly(quad(rect(u, s0, u + 0.06, s0 + 0.07), rnd, 0.002)).fill(IRON[0]);
  }
}

/** Bucket: a wooden pail with a thin rim of staves, an iron hoop, a little water, and its iron handle
 * standing up across it between two lugs. */
function bucket(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.15;
  g.poly(ring(cx, cy, r, rnd, 0.006, 16)).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(cx, cy, r * 0.9).stroke({ width: 0.015, color: IRON[0] });
  g.circle(cx, cy, r * 0.78).fill(0x4e7a8a).stroke({ width: 0.01, color: INK });
  g.circle(cx + light[0] * 0.035, cy + light[1] * 0.035, 0.026).fill({ color: 0xffffff, alpha: 0.45 });
  // The handle, standing up: seen from above, a bar straight across the pail between its two lugs.
  const [ax, bx] = [cx - r * 1.02, cx + r * 1.02];
  g.moveTo(ax, cy).lineTo(bx, cy).stroke({ width: 0.05, color: INK, cap: 'round' });
  g.moveTo(ax, cy).lineTo(bx, cy).stroke({ width: 0.026, color: IRON[1], cap: 'round' });
  g.moveTo(ax + 0.015, cy - 0.005).lineTo(bx - 0.015, cy - 0.005).stroke({ width: 0.008, color: IRON[2], cap: 'round' });
  for (const x of [ax, bx]) g.circle(x, cy, 0.03).fill(IRON[1]).stroke({ width: 0.018, color: INK });
}

/** Pale stone for statues, effigies and fine work: shade, face, lit. */
const MARBLE = [0x8e8a80, 0xc4beb0, 0xe2dccd];
const GILT = [0x8a6a2a, 0xc8a04a, 0xf0d488];
const BRONZE = [0x5a4220, 0x9a7438, 0xd0a85a];

/** A candle in a holder, its flame and a little glow. */
function candle(g: Graphics, x: number, y: number) {
  g.circle(x, y, 0.09).fill({ color: 0xf59e0b, alpha: 0.18 });
  g.circle(x, y, 0.05).fill(GILT[1]).stroke({ width: 0.012, color: INK });
  g.circle(x, y, 0.03).fill(LINEN[2]);
  g.circle(x, y, 0.015).fill(0xffc23a);
}

/** Altar against a wall: a block of pale stone, a cloth laid across it and hanging over the front, a
 * candle at each end and a book open in the middle, a step before it. */
function altar(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  board(g, rect(0.14, 0.78, len - 0.14, 0.96), STONE, light, rnd, along, 0.03, 0);
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.8), MARBLE, light, rnd, along, 0.06, 0);
  const cloth = [0x8a2e26, 0x3e5a8a, 0xf0e8d4, 0x6a4a6a][v % 4];
  const [c0, c1] = [len * 0.32, len * 0.68];
  g.poly(quad(rect(c0, WALL_GAP + 0.04, c1, 0.88), rnd, 0.005)).fill(cloth).stroke({ width: LINE, color: INK });
  g.poly(quad(rect(c0 + 0.04, 0.8, c1 - 0.04, 0.86), rnd, 0.003)).fill(GILT[1]);
  const [bx0, by0, bx1, by1] = rect(len / 2 - 0.16, 0.38, len / 2 + 0.16, 0.6);
  g.rect(bx0, by0, bx1 - bx0, by1 - by0).fill(LINEN[2]).stroke({ width: 0.012, color: INK });
  g.moveTo(...at(len / 2, 0.38)).lineTo(...at(len / 2, 0.6)).stroke({ width: 0.012, color: LINEN[0] });
  for (const u of [END_GAP + 0.2, len - END_GAP - 0.2]) candle(g, ...at(u, 0.5));
}

/** Sarcophagus: the stone effigy on its tomb chest without the sword — a knight lying with head on a pillow,
 * hands together in prayer, a dog at the feet. */
function sarcophagus(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  stoneEffigy(g, f, light, rnd, false);
}

/** Bell, hung in a frame: two heavy beams across the loft, the headstock between them, the bronze bell
 * under it (seen as its crown and lip), its rope running down off one side. */
function bell(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  const wid = along ? f.h : f.w;
  const [cx, cy] = at(len / 2, wid / 2);
  const R = Math.min(len, wid) * 0.32;
  // The bell: its lip (widest), the waist, the crown, lit toward the light.
  g.poly(ring(cx, cy, R, rnd, 0.008, 32)).fill(BRONZE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.circle(cx + light[0] * R * 0.08, cy + light[1] * R * 0.08, R * 0.86).fill(BRONZE[1]);
  g.circle(cx, cy, R * 0.6).stroke({ width: 0.02, color: BRONZE[0] });
  g.circle(cx + light[0] * R * 0.25, cy + light[1] * R * 0.25, R * 0.42).fill({ color: BRONZE[2], alpha: 0.7 });
  g.circle(cx, cy, R * 0.22).fill(BRONZE[0]).stroke({ width: LINE, color: INK });
  // Beams across, the headstock over the crown.
  for (const s of [wid / 2 - R - 0.1, wid / 2 + R + 0.02]) board(g, rect(0.06, s, len - 0.06, s + 0.1), DARK_WOOD, light, rnd, true, 0.025, 1);
  board(g, rect(len / 2 - 0.12, wid / 2 - R - 0.06, len / 2 + 0.12, wid / 2 + R + 0.06), WOOD, light, rnd, false, 0.03, 1);
  // The rope, down off the wheel at one side.
  const [rx, ry] = at(len / 2 + R + 0.1, wid / 2);
  g.moveTo(...at(len / 2 + 0.12, wid / 2)).lineTo(rx, ry).stroke({ width: 0.03, color: 0xc8b080 });
  g.circle(rx, ry, 0.06).stroke({ width: 0.025, color: 0xc8b080 });
  g.circle(rx, ry, 0.03).stroke({ width: 0.02, color: 0xc8b080 });
}

/** A shadow cast across the floor away from the light by something `h` squares tall: the footprint `pts`
 * swept that far and blurred (three passes, fainter outward). */
function castShadow(g: Graphics, pts: number[], [lx, ly]: Light, h: number) {
  for (const [k, a] of [
    [1, 0.12],
    [0.75, 0.14],
    [0.5, 0.16],
  ]) {
    const [dx, dy] = [-lx * h * k, -ly * h * k];
    const n = pts.length / 2;
    // The hull of the footprint and its moved copy: good enough for the round and square bases here.
    const all: number[][] = [];
    for (let i = 0; i < n; i++) all.push([pts[i * 2], pts[i * 2 + 1]], [pts[i * 2] + dx, pts[i * 2 + 1] + dy]);
    all.sort((p, q) => p[0] - q[0] || p[1] - q[1]);
    const cross = (o: number[], p: number[], q: number[]) => (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
    const lower: number[][] = [];
    for (const p of all) {
      while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], p) <= 0) lower.pop();
      lower.push(p);
    }
    const upper: number[][] = [];
    for (let i = all.length - 1; i >= 0; i--) {
      const p = all[i];
      while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], p) <= 0) upper.pop();
      upper.push(p);
    }
    g.poly([...lower.slice(0, -1), ...upper.slice(0, -1)].flat()).fill({ color: 0x14100c, alpha: a });
  }
}

/** Statue on a plinth, seen from above: a long shadow across the floor, a plain stepped square plinth,
 * and on it a figure in bronze or warm marble — a big head on broad shoulders over a plain robe, kept
 * simple so it reads at a glance. */
function statue(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  const outer = rect(0.08, Math.max(0.08, WALL_GAP - 0.08), 0.92, 0.94);
  castShadow(g, quad(outer, rnd, 0), light, 0.9);
  board(g, outer, STONE.map((c) => shade(c, 0.85)), light, rnd, true, 0.05, 0);
  const [ox0, oy0, ox1, oy1] = outer;
  board(g, [ox0 + 0.1, oy0 + 0.1, ox1 - 0.1, oy1 - 0.1], STONE, light, rnd, true, 0.04, 0);
  // Bronze (one gone green) or warm marble.
  const t = [
    [0x6a4a22, 0xa07a3e, 0xd0aa62],
    [0x8a7a62, 0xd2c4a6, 0xf0e4c8],
    [0x3e5e52, 0x5e8a74, 0x8ab8a0],
    [0x8a7a62, 0xd2c4a6, 0xf0e4c8],
  ][v % 4];
  const [cx, cy] = at(0.5, 0.52);
  const ink = { width: OUT, color: INK, join: 'round' as const };
  // A plain robe: a rounded body round the shoulders.
  g.poly(ring(cx, cy, 0.3, rnd, 0.01, 20)).fill(t[0]).stroke(ink);
  g.poly(ring(cx + light[0] * 0.02, cy + light[1] * 0.02, 0.25, rnd, 0.008, 20)).fill(t[1]);
  // Broad shoulders across it.
  const [sx, sy] = [at(0.18, 0.52), at(0.82, 0.52)];
  g.moveTo(...sx).lineTo(...sy).stroke({ width: 0.2, color: INK, cap: 'round' });
  g.moveTo(...sx).lineTo(...sy).stroke({ width: 0.15, color: t[1], cap: 'round' });
  g.moveTo(sx[0] + light[0] * 0.02, sx[1] + light[1] * 0.03).lineTo(sy[0] + light[0] * 0.02, sy[1] + light[1] * 0.03).stroke({ width: 0.04, color: t[2], cap: 'round', alpha: 0.7 });
  // The big head.
  g.circle(cx, cy, 0.17).fill(t[1]).stroke(ink);
  g.circle(cx + light[0] * 0.05, cy + light[1] * 0.05, 0.085).fill(t[2]);
}

/** Pillar: a round column from above — its square base showing at the corners, the capital a solid,
 * flat round top (matte, a thin lit rim), so it reads as a column, not a basin. */
function pillar(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) * 0.4;
  board(g, [cx - r * 1.12, cy - r * 1.12, cx + r * 1.12, cy + r * 1.12], STONE, light, rnd, true, 0.04, 0);
  const la = Math.atan2(light[1], light[0]);
  g.poly(ring(cx, cy, r, rnd, 0.004, 32)).fill(MARBLE[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.moveTo(cx + Math.cos(la - 1.1) * r * 0.88, cy + Math.sin(la - 1.1) * r * 0.88)
    .arc(cx, cy, r * 0.88, la - 1.1, la + 1.1)
    .stroke({ width: 0.025, color: MARBLE[2] });
  g.moveTo(cx + Math.cos(la + Math.PI - 1.1) * r * 0.88, cy + Math.sin(la + Math.PI - 1.1) * r * 0.88)
    .arc(cx, cy, r * 0.88, la + Math.PI - 1.1, la + Math.PI + 1.1)
    .stroke({ width: 0.025, color: MARBLE[0], alpha: 0.8 });
}

/** Throne against a wall: a tall back with a pointed gilt crest, carved arms, and a deep red cushion
 * filling the seat, tufted, with gilt finials at the front corners. */
function throne(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  // The seat frame, then the cushion filling it.
  board(g, rect(0.12, WALL_GAP + 0.06, 0.88, 0.9), DARK_WOOD, light, rnd, true, 0.03, 0);
  const [qx0, qy0, qx1, qy1] = rect(0.24, WALL_GAP + 0.16, 0.76, 0.86);
  g.roundRect(qx0, qy0, qx1 - qx0, qy1 - qy0, 0.07).fill(0x7a2420).stroke({ width: OUT, color: INK });
  g.roundRect(qx0 + 0.035 + light[0] * 0.015, qy0 + 0.035 + light[1] * 0.015, qx1 - qx0 - 0.07, qy1 - qy0 - 0.07, 0.06).fill(0xb04436);
  g.roundRect(qx0 + 0.07 + light[0] * 0.03, qy0 + 0.07 + light[1] * 0.03, (qx1 - qx0) * 0.4, (qy1 - qy0) * 0.3, 0.04).fill({ color: 0xd8705a, alpha: 0.6 });
  for (const [u, s0] of [
    [0.4, 0.55],
    [0.6, 0.55],
    [0.5, 0.7],
  ]) {
    g.circle(...at(u, s0), 0.022).fill(GILT[1]).stroke({ width: 0.01, color: INK });
  }
  // Arms, slim.
  for (const u of [0.12, 0.76]) board(g, rect(u, WALL_GAP + 0.06, u + 0.12, 0.9), DARK_WOOD, light, rnd, false, 0.025, 0);
  for (const u of [0.18, 0.82]) g.circle(...at(u, 0.9), 0.045).fill(GILT[1]).stroke({ width: LINE, color: INK });
  // The tall back and its pointed gilt crest.
  board(g, rect(0.06, WALL_GAP - 0.06, 0.94, WALL_GAP + 0.1), DARK_WOOD, light, rnd, true, 0.03, 0);
  g.poly([...at(0.34, WALL_GAP - 0.05), ...at(0.5, WALL_GAP + 0.12), ...at(0.66, WALL_GAP - 0.05)]).fill(GILT[1]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const u of [0.08, 0.92]) g.circle(...at(u, WALL_GAP + 0.02), 0.05).fill(GILT[1]).stroke({ width: LINE, color: INK });
}

/** Stage: a raised platform of planks — its front edge (the apron) darker, a shadow on the floor along the
 * front and sides — a flight of steps up the middle of the front, and a heavy curtain hung along the back,
 * folds in light and dark, a wavy hem, tied back at both ends. */
function stage(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const wid = along ? f.h : f.w;
  const front = wid - 0.36;
  // Shadow on the floor round the front and sides.
  g.poly(quad(rect(0.02, 0.1, len - 0.02, front + 0.1), rnd, 0)).fill({ color: 0x14100c, alpha: 0.28 });
  // Steps up the middle of the front: three treads, each lower one darker.
  const sw = len * 0.3;
  for (let k = 0; k < 3; k++) {
    const s0 = front - 0.04 + k * 0.11;
    board(g, rect(len / 2 - sw - k * 0.04, s0, len / 2 + sw + k * 0.04, s0 + 0.12), WOOD.map((c) => shade(c, 0.95 - k * 0.1)), light, rnd, true, 0.02, 0);
  }
  const face = board(g, rect(0.06, 0.06, len - 0.06, front), WOOD, light, rnd, along, 0.05, 2);
  planks(g, face, along, Math.round(wid * 4), WOOD[0], rnd);
  // The apron: the front edge, darker.
  g.poly(quad(rect(0.06, front - 0.07, len - 0.06, front), rnd, 0.003)).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  // The curtain: folds along the back, a wavy hem over the boards, tied back at both ends.
  const depth = 0.34;
  const n = Math.round(len * 6);
  for (let i = 0; i < n; i++) {
    const u0 = 0.08 + ((len - 0.16) * i) / n;
    const u1 = 0.08 + ((len - 0.16) * (i + 1)) / n;
    const um = (u0 + u1) / 2;
    g.moveTo(...at(u0, 0.06)).lineTo(...at(u1, 0.06)).lineTo(...at(u1, depth)).quadraticCurveTo(...at(um, depth + 0.07), ...at(u0, depth)).closePath();
    g.fill(i % 2 ? 0x9a3428 : 0x6e2420);
  }
  g.moveTo(...at(0.08, 0.06)).lineTo(...at(len - 0.08, 0.06)).stroke({ width: LINE, color: INK });
  for (let i = 0; i < n; i++) {
    const u0 = 0.08 + ((len - 0.16) * i) / n;
    const u1 = 0.08 + ((len - 0.16) * (i + 1)) / n;
    g.moveTo(...at(u0, depth)).quadraticCurveTo(...at((u0 + u1) / 2, depth + 0.07), ...at(u1, depth));
  }
  g.stroke({ width: LINE, color: INK });
  g.poly(quad(rect(0.08, 0.06, len - 0.08, 0.1), rnd, 0.002)).fill(GILT[1]);
  // Tie-backs: the curtain gathered at each end.
  for (const u of [0.08, len - 0.3]) {
    g.poly(quad(rect(u, 0.06, u + 0.22, depth + 0.14), rnd, 0.01)).fill(0x7a2a22).stroke({ width: LINE, color: INK });
    g.moveTo(...at(u, depth - 0.02)).lineTo(...at(u + 0.22, depth - 0.02)).stroke({ width: 0.03, color: GILT[1] });
  }
}

/** A ring of wooden staves bound with iron, seen from above (vats): the outer rim, the stave joints, a
 * hoop, and the inside's edge at `inner` of the radius; the contents are drawn after. */
function staveRing(g: Graphics, cx: number, cy: number, r: number, inner: number, light: Light, rnd: () => number, n = 28) {
  g.poly(ring(cx, cy, r, rnd, 0.008, 32)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.circle(cx + light[0] * 0.01, cy + light[1] * 0.01, r * (1 + inner) / 2 + 0.01).fill(WOOD[1]);
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * r * inner, cy + Math.sin(a) * r * inner).lineTo(cx + Math.cos(a) * r * 0.98, cy + Math.sin(a) * r * 0.98);
  }
  g.stroke({ width: 0.012, color: WOOD[0] });
  g.circle(cx, cy, r * (0.5 + inner / 2)).stroke({ width: 0.03, color: IRON[0] });
  g.circle(cx, cy, r * inner).stroke({ width: LINE, color: INK });
}

/** Brewing vat: a great wooden tun, the mash inside under a head of froth, a wooden paddle across it. */
function vat(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) / 2 - 0.08;
  staveRing(g, cx, cy, r, 0.84, light, rnd);
  g.circle(cx, cy, r * 0.84).fill(0x7a5a2a);
  g.circle(cx + light[0] * r * 0.1, cy + light[1] * r * 0.1, r * 0.6).fill(0x9a7434);
  for (let i = 0; i < 40; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.78 * Math.sqrt(rnd());
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.02 + 0.03 * rnd()).fill({ color: 0xf0e2b8, alpha: 0.85 });
  }
  const a = rnd() * Math.PI;
  const [px, py] = [Math.cos(a) * r * 1.05, Math.sin(a) * r * 1.05];
  g.moveTo(cx - px, cy - py).lineTo(cx + px, cy + py).stroke({ width: 0.08, color: INK, cap: 'round' });
  g.moveTo(cx - px, cy - py).lineTo(cx + px, cy + py).stroke({ width: 0.05, color: WOOD[2], cap: 'round' });
  g.poly(ring(cx + px * 0.7, cy + py * 0.7, 0.1, rnd, 0.01, 10)).fill(WOOD[2]).stroke({ width: LINE, color: INK });
}

/** Cauldron: a round black iron pot on three short legs, the embers of a fire glowing round its foot, a
 * bubbling brew inside. */
function cauldron(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.34;
  g.circle(cx, cy, r + 0.1).fill({ color: 0xf59e0b, alpha: 0.12 });
  for (let i = 0; i < 14; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r + 0.02 + 0.06 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.02 + 0.015 * rnd()).fill([0xc2410c, 0xf97316, 0xffc23a][Math.floor(rnd() * 3)]);
  }
  for (const a of [0.5, 2.6, 4.7]) g.circle(cx + Math.cos(a) * (r + 0.04), cy + Math.sin(a) * (r + 0.04), 0.045).fill(IRON[0]).stroke({ width: LINE, color: INK });
  g.circle(cx, cy, r).fill(0x24221e).stroke({ width: OUT, color: INK });
  g.circle(cx, cy, r * 0.86).stroke({ width: 0.035, color: 0x4a4844 });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la - 0.9) * r * 0.93, cy + Math.sin(la - 0.9) * r * 0.93)
    .arc(cx, cy, r * 0.93, la - 0.9, la + 0.9)
    .stroke({ width: 0.02, color: 0x8a8884 });
  const brew = [
    [0x3a6a2a, 0x6aa84a, 0xb0e080],
    [0x5a2a6a, 0x8a4aa8, 0xd0a0f0],
    [0x7a3a1a, 0xb0602a, 0xf0b070],
    [0x2a5a6a, 0x4a8aa8, 0x9ad0e0],
  ][v % 4];
  g.circle(cx, cy, r * 0.76).fill(brew[0]);
  g.circle(cx + light[0] * r * 0.1, cy + light[1] * r * 0.1, r * 0.56).fill(brew[1]);
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.55 * Math.sqrt(rnd());
    const br = 0.025 + 0.025 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, br).fill(brew[1]).stroke({ width: 0.012, color: brew[2] });
  }
}

/** Alchemy bench against a wall: a work top with glassware — round flasks of coloured liquid, a retort,
 * a little burner under a flask, a mortar and pestle, a scatter of herbs — and jars along the wall. */
function alchemyBench(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.92), DARK_WOOD, light, rnd, along, 0.05, 2);
  const liquids = [0x6aa84a, 0x8a4aa8, 0xc84a3a, 0x4a8aa8, 0xd8b040];
  const flask = (u: number, s0: number, r: number, c: number) => {
    const [x, y] = at(u, s0);
    g.circle(x, y, r).fill({ color: 0xd8e8e8, alpha: 0.5 }).stroke({ width: LINE, color: INK });
    g.circle(x, y, r * 0.75).fill(c);
    g.circle(x - r * 0.3, y - r * 0.3, r * 0.25).fill({ color: 0xffffff, alpha: 0.7 });
  };
  // Jars along the wall.
  for (let u = END_GAP + 0.14, k = v; u < len - END_GAP - 0.08; u += 0.17, k++) {
    const [x, y] = at(u, WALL_GAP + 0.1);
    g.circle(x, y, 0.06).fill(0x8a7a62).stroke({ width: 0.012, color: INK });
    g.circle(x, y, 0.03).fill(liquids[(k * 3) % liquids.length]);
  }
  // A retort: a round belly with a long neck reaching out.
  const [rx, ry] = at(len * 0.3, 0.56);
  const [nx, ny] = at(len * 0.3 + 0.36, 0.74);
  g.moveTo(rx, ry).lineTo(nx, ny).stroke({ width: 0.05, color: INK, cap: 'round' });
  g.moveTo(rx, ry).lineTo(nx, ny).stroke({ width: 0.03, color: 0xc8dcdc, cap: 'round' });
  flask(len * 0.3, 0.56, 0.13, liquids[v % liquids.length]);
  // A flask on a little burner, its flame glowing.
  const [bx, by] = at(len * 0.62, 0.5);
  g.circle(bx, by, 0.11).fill({ color: 0xf59e0b, alpha: 0.2 });
  for (const a of [0, 2.1, 4.2]) g.moveTo(bx, by).lineTo(bx + Math.cos(a) * 0.12, by + Math.sin(a) * 0.12);
  g.stroke({ width: 0.02, color: IRON[1] });
  flask(len * 0.62, 0.5, 0.09, liquids[(v + 2) % liquids.length]);
  // Mortar and pestle, a scatter of herbs.
  const [mx, my] = at(len - END_GAP - 0.24, 0.72);
  g.circle(mx, my, 0.09).fill(STONE[1]).stroke({ width: LINE, color: INK });
  g.circle(mx, my, 0.055).fill(STONE[0]);
  g.moveTo(mx, my).lineTo(mx + 0.1, my - 0.08).stroke({ width: 0.03, color: STONE[2], cap: 'round' });
  for (let i = 0; i < 6; i++) g.poly(oval(...at(len * 0.45 + rnd() * 0.3, 0.74 + rnd() * 0.12), 0.03, 0.012, rnd() * Math.PI, 6)).fill(0x5a8a3a);
}

/** A weapon lying on the rack's pegs along `u` from u0 to u1, its point toward u1: a sword, a spear, an axe
 * or a mace. */
function weapon(g: Graphics, at: (u: number, s: number) => [number, number], u0: number, u1: number, s: number, kind: number) {
  const steel = 0xc8ccd0;
  const shaft = (a: number, b: number, w: number, c: number) => {
    g.moveTo(...at(a, s)).lineTo(...at(b, s)).stroke({ width: w + 0.025, color: INK, cap: 'round' });
    g.moveTo(...at(a, s)).lineTo(...at(b, s)).stroke({ width: w, color: c, cap: 'round' });
  };
  if (kind === 0) {
    // Sword: grip, crossguard, blade.
    shaft(u0, u0 + 0.14, 0.03, 0x5a3a22);
    g.moveTo(...at(u0 + 0.14, s - 0.07)).lineTo(...at(u0 + 0.14, s + 0.07)).stroke({ width: 0.035, color: INK, cap: 'round' });
    g.moveTo(...at(u0 + 0.14, s - 0.07)).lineTo(...at(u0 + 0.14, s + 0.07)).stroke({ width: 0.018, color: BRASS, cap: 'round' });
    g.poly([...at(u0 + 0.15, s - 0.025), ...at(u1 - 0.06, s - 0.02), ...at(u1, s), ...at(u1 - 0.06, s + 0.02), ...at(u0 + 0.15, s + 0.025)]).fill(steel).stroke({ width: 0.012, color: INK });
  } else if (kind === 1) {
    // Spear: a long haft, a leaf-shaped head.
    shaft(u0, u1 - 0.12, 0.025, WOOD[2]);
    g.poly([...at(u1 - 0.14, s), ...at(u1 - 0.08, s - 0.04), ...at(u1, s), ...at(u1 - 0.08, s + 0.04)]).fill(steel).stroke({ width: 0.012, color: INK });
  } else if (kind === 2) {
    // Axe: a haft, a crescent blade on one side.
    shaft(u0, u1, 0.03, WOOD[2]);
    g.poly([...at(u1 - 0.18, s), ...at(u1 - 0.22, s - 0.14), ...at(u1 - 0.06, s - 0.14), ...at(u1 - 0.04, s)]).fill(steel).stroke({ width: 0.012, color: INK });
  } else {
    // Mace: a haft, a flanged iron head.
    shaft(u0, u1 - 0.08, 0.03, WOOD[1]);
    const [hx, hy] = at(u1 - 0.06, s);
    g.circle(hx, hy, 0.06).fill(IRON[1]).stroke({ width: 0.012, color: INK });
    for (let i = 0; i < 6; i++) {
      const a = (i / 6) * Math.PI * 2;
      g.moveTo(hx, hy).lineTo(hx + Math.cos(a) * 0.08, hy + Math.sin(a) * 0.08);
    }
    g.stroke({ width: 0.02, color: IRON[0] });
  }
}

/** Weapon rack against a wall: a frame of two rails with pegs, weapons laid along it — swords, spears,
 * axes, a mace — and a round shield hung at one end. */
function weaponRack(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, WALL_GAP + 0.08), DARK_WOOD, light, rnd, along, 0.02, 0);
  for (const u of [END_GAP + 0.04, len - END_GAP - 0.1]) board(g, rect(u, WALL_GAP, u + 0.06, 0.88), DARK_WOOD, light, rnd, !along, 0.015, 0);
  // Weapons at their own lengths (a sword about 3.5 ft, a spear 6), as many as fit along each row.
  const lengths = [0.72, 1.2, 0.6, 0.55];
  const rows = [0.4, 0.55, 0.7, 0.84];
  let k = v;
  for (const s0 of rows) {
    let u = END_GAP + 0.06;
    const end = len - END_GAP - 0.04;
    while (true) {
      let kind = k % 4;
      if (u + lengths[kind] > end) kind = [2, 3, 0].find((c) => u + lengths[c] <= end) ?? -1;
      if (kind < 0) break;
      weapon(g, at, u, u + lengths[kind], s0, kind);
      u += lengths[kind] + 0.12;
      k++;
    }
    k += 3;
  }
  // A shield hung at one end, its boss and rim.
  const [sx, sy] = at(len - END_GAP - 0.22, WALL_GAP + 0.14);
  const dye = DYES[(v + 1) % DYES.length];
  g.circle(sx, sy, 0.13).fill(dye).stroke({ width: OUT, color: INK });
  g.circle(sx, sy, 0.11).stroke({ width: 0.02, color: IRON[1] });
  g.circle(sx, sy, 0.04).fill(IRON[2]).stroke({ width: 0.012, color: INK });
}

/** The rack (torture): a heavy wooden frame, a roller across each end with ropes to the bed, a big wheel
 * at the head to turn it, the bed's boards stained. */
function tortureRack(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  for (const s of [0.14, 0.78]) board(g, rect(0.06, s, len - 0.06, s + 0.08), DARK_WOOD, light, rnd, along, 0.02, 0);
  const bed = board(g, rect(0.24, 0.24, len - 0.24, 0.76), WOOD, light, rnd, along, 0.03, 1);
  planks(g, bed, along, 3, WOOD[0], rnd);
  g.poly(ring(...at(len * 0.55, 0.5), 0.1, rnd, 0.05, 10)).fill({ color: 0x5a1a14, alpha: 0.5 });
  for (const u of [0.12, len - 0.12]) {
    g.moveTo(...at(u, 0.12)).lineTo(...at(u, 0.88)).stroke({ width: 0.12, color: INK, cap: 'round' });
    g.moveTo(...at(u, 0.12)).lineTo(...at(u, 0.88)).stroke({ width: 0.08, color: WOOD[1], cap: 'round' });
    for (const s of [0.36, 0.64]) g.moveTo(...at(u, s)).lineTo(...at(u < 1 ? 0.34 : len - 0.34, s));
    g.stroke({ width: 0.022, color: 0xc8b080 });
  }
  // The wheel at the head.
  const [wx, wy] = at(0.12, 0.94);
  g.circle(wx, wy, 0.1).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2;
    g.moveTo(wx, wy).lineTo(wx + Math.cos(a) * 0.14, wy + Math.sin(a) * 0.14);
  }
  g.stroke({ width: 0.025, color: WOOD[2] });
}

/** Iron cage: dark straw on its floor between thick bars running one way across its top, cross-straps, a
 * heavy frame with corner posts, and a door in its right-hand side, hinged, a padlock hanging outside. */
function cage(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y, w, h } = f;
  const [x0, y0, x1, y1] = [x + 0.08, y + 0.08, x + w - 0.08, y + h - 0.08];
  g.rect(x0, y0, x1 - x0, y1 - y0).fill(0x2e261c);
  for (let i = 0; i < 14 * w * h; i++) {
    const [px, py] = [x0 + rnd() * (x1 - x0), y0 + rnd() * (y1 - y0)];
    const a = rnd() * Math.PI;
    g.moveTo(px, py).lineTo(px + Math.cos(a) * 0.1, py + Math.sin(a) * 0.1);
  }
  g.stroke({ width: 0.014, color: 0x8a7440, alpha: 0.8 });
  const bar = (ax: number, ay: number, bx: number, by: number, wdt: number) => {
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: wdt + 0.025, color: INK });
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: wdt, color: IRON[1] });
    g.moveTo(ax - 0.01, ay).lineTo(bx - 0.01, by).stroke({ width: wdt * 0.3, color: IRON[2] });
  };
  // Thick bars one way, about a fifth of a square apart, then the cross-straps.
  const n = Math.max(3, Math.round((x1 - x0) / 0.19));
  for (let i = 1; i < n; i++) bar(x0 + ((x1 - x0) * i) / n, y0, x0 + ((x1 - x0) * i) / n, y1, 0.05);
  const straps = h >= 2 ? 3 : 2;
  for (let j = 1; j <= straps; j++) bar(x0, y0 + ((y1 - y0) * j) / (straps + 1), x1, y0 + ((y1 - y0) * j) / (straps + 1), 0.04);
  g.rect(x0, y0, x1 - x0, y1 - y0).stroke({ width: 0.09, color: INK });
  g.rect(x0, y0, x1 - x0, y1 - y0).stroke({ width: 0.055, color: IRON[1] });
  for (const [cx, cy] of [
    [x0, y0],
    [x1, y0],
    [x0, y1],
    [x1, y1],
  ]) {
    g.rect(cx - 0.05, cy - 0.05, 0.1, 0.1).fill(IRON[0]).stroke({ width: LINE, color: INK });
  }
  // The door, in the right-hand side: a heavier stretch of the frame there, hinge knuckles at its ends,
  // and a big padlock hanging outside the bars at its middle.
  const [dy0, dy1] = [y0 + (y1 - y0) * 0.3, y0 + (y1 - y0) * 0.7];
  g.moveTo(x1, dy0).lineTo(x1, dy1).stroke({ width: 0.13, color: INK });
  g.moveTo(x1, dy0).lineTo(x1, dy1).stroke({ width: 0.08, color: IRON[2] });
  for (const hy of [dy0, dy1]) g.rect(x1 - 0.07, hy - 0.035, 0.14, 0.07).fill(IRON[0]).stroke({ width: 0.012, color: INK });
  const ly = (dy0 + dy1) / 2;
  g.moveTo(x1 + 0.04, ly - 0.05).arc(x1 + 0.08, ly - 0.05, 0.04, Math.PI, Math.PI * 2).stroke({ width: 0.022, color: IRON[2] });
  g.rect(x1 + 0.02, ly - 0.05, 0.12, 0.11).fill(BRASS).stroke({ width: LINE, color: INK });
  g.circle(x1 + 0.08, ly + 0.005, 0.015).fill(INK);
}

/** Portcullis winch: a drum on an axle between two posts, shaded as a cylinder, its chain wound round it in
 * rows of links and running off into a dark slot in the floor; a big iron crank at each end, a toothed
 * ratchet with its pawl. */
function winch(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const [cy, dy0, dy1] = [y + 0.42, y + 0.24, y + 0.6];
  const link = (lx: number, ly: number, rx: number, ry: number) => {
    g.poly(oval(lx, ly, rx, ry, 0, 10)).stroke({ width: 0.028, color: INK });
    g.poly(oval(lx, ly, rx, ry, 0, 10)).stroke({ width: 0.014, color: IRON[2] });
  };
  // The slot in the floor, and the slack chain running into it.
  g.poly(quad([x + 0.38, y + 0.84, x + 0.62, y + 0.94], rnd, 0.004)).fill(0x0e0c0a).stroke({ width: LINE, color: INK });
  for (let k = 0; k < 4; k++) link(x + 0.5, dy1 + 0.04 + k * 0.06, 0.03, 0.04);
  // Posts.
  for (const px of [x + 0.1, x + 0.76]) board(g, [px, y + 0.14, px + 0.14, y + 0.7], DARK_WOOD, light, rnd, false, 0.02, 0);
  // The drum, a cylinder along x: dark at its edges, a light band down the middle.
  g.rect(x + 0.24, dy0, 0.52, dy1 - dy0).fill(WOOD[0]).stroke({ width: OUT, color: INK });
  g.rect(x + 0.24, dy0 + 0.06, 0.52, dy1 - dy0 - 0.12).fill(WOOD[1]);
  g.rect(x + 0.24, cy - 0.04 + light[1] * 0.02, 0.52, 0.06).fill({ color: WOOD[2], alpha: 0.9 });
  // Chain wound round it: rows of links across the drum.
  for (let k = 0; k < 6; k++) for (let j = 0; j < 3; j++) link(x + 0.29 + k * 0.085, dy0 + 0.07 + j * 0.11, 0.03, 0.05);
  // Big cranks: an L-handle out past each post.
  for (const [ax, dir] of [
    [x + 0.06, -1],
    [x + 0.94, 1],
  ]) {
    g.moveTo(ax, cy).lineTo(ax, cy + 0.3).stroke({ width: 0.07, color: INK, cap: 'round' });
    g.moveTo(ax, cy).lineTo(ax, cy + 0.3).stroke({ width: 0.04, color: IRON[1], cap: 'round' });
    g.moveTo(ax, cy + 0.3).lineTo(ax + dir * 0.06, cy + 0.3).stroke({ width: 0.07, color: INK, cap: 'round' });
    g.moveTo(ax, cy + 0.3).lineTo(ax + dir * 0.06, cy + 0.3).stroke({ width: 0.045, color: WOOD[1], cap: 'round' });
    g.circle(ax, cy, 0.05).fill(IRON[1]).stroke({ width: LINE, color: INK });
  }
  // The ratchet: a toothed wheel on the axle by the right post, its pawl hooked in.
  const [rx, ry] = [x + 0.83, y + 0.08];
  const teeth: number[] = [];
  for (let i = 0; i < 16; i++) {
    const a = (i / 16) * Math.PI * 2;
    const r = i % 2 ? 0.07 : 0.095;
    teeth.push(rx + Math.cos(a) * r, ry + Math.sin(a) * r);
  }
  g.poly(teeth).fill(IRON[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(rx, ry, 0.03).fill(IRON[0]);
  g.moveTo(x + 0.66, y + 0.06).lineTo(rx - 0.08, ry + 0.01).stroke({ width: 0.035, color: INK, cap: 'round' });
  g.moveTo(x + 0.66, y + 0.06).lineTo(rx - 0.08, ry + 0.01).stroke({ width: 0.018, color: IRON[2], cap: 'round' });
}

/** Telescope: a long brass tube on a wooden tripod, pointed up and away across the room — its lens end
 * wide, the eyepiece narrow — a stool by the eyepiece. */
function telescope(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const s = Math.min(f.w, f.h);
  for (const a of [0.6, 2.7, 4.8]) {
    g.moveTo(cx, cy).lineTo(cx + Math.cos(a) * s * 0.36, cy + Math.sin(a) * s * 0.36).stroke({ width: 0.06, color: INK, cap: 'round' });
    g.moveTo(cx, cy).lineTo(cx + Math.cos(a) * s * 0.36, cy + Math.sin(a) * s * 0.36).stroke({ width: 0.035, color: WOOD[1], cap: 'round' });
  }
  const a = -Math.PI / 4 + (rnd() - 0.5) * 0.6;
  const [ux, uy] = [Math.cos(a), Math.sin(a)];
  const [wx, wy] = [-uy, ux];
  const L = s * 0.42;
  const tube = [cx - ux * L * 0.6 - wx * 0.05, cy - uy * L * 0.6 - wy * 0.05, cx + ux * L - wx * 0.1, cy + uy * L - wy * 0.1, cx + ux * L + wx * 0.1, cy + uy * L + wy * 0.1, cx - ux * L * 0.6 + wx * 0.05, cy - uy * L * 0.6 + wy * 0.05];
  g.poly(tube).fill(BRONZE[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.moveTo(cx - ux * L * 0.55 - wx * 0.02 * Math.sign(light[0] + light[1] || 1), cy - uy * L * 0.55).lineTo(cx + ux * L * 0.95, cy + uy * L * 0.95).stroke({ width: 0.025, color: BRONZE[2] });
  for (const k of [0.1, 0.55]) g.moveTo(cx + ux * L * k - wx * 0.08, cy + uy * L * k - wy * 0.08).lineTo(cx + ux * L * k + wx * 0.08, cy + uy * L * k + wy * 0.08);
  g.stroke({ width: 0.03, color: BRONZE[0] });
  g.poly(oval(cx + ux * L, cy + uy * L, 0.04, 0.11, a, 12)).fill(0x6a9ab0).stroke({ width: LINE, color: INK });
  g.circle(cx - ux * L * 0.62, cy - uy * L * 0.62, 0.035).fill(BRONZE[0]).stroke({ width: 0.012, color: INK });
  // A stool by the eyepiece.
  const [tx, ty] = [cx - ux * L * 1.15, cy - uy * L * 1.15];
  g.poly(ring(tx, ty, 0.13, rnd, 0.01, 12)).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  g.circle(tx + light[0] * 0.015, ty + light[1] * 0.015, 0.1).fill(WOOD[1]);
}

/** Barricade (cheval de frise): a thick log along its length, sharpened stakes through it sticking straight
 * out on both sides in turn, each a tapering wedge with a dark iron point, lashed where it meets the log. */
function barricade(g: Graphics, f: InteriorItem) {
  const { len, at } = frame(f);
  const n = Math.max(3, Math.round(len * 3));
  const us = Array.from({ length: n }, (_, i) => 0.22 + ((len - 0.44) * i) / (n - 1));
  us.forEach((u, i) => {
    const side = i % 2 ? 1 : -1;
    const tip = 0.5 + side * 0.44;
    // The wedge: wide at the log, narrowing to the point.
    g.poly([...at(u - 0.05, 0.5), ...at(u - 0.02, tip - side * 0.1), ...at(u + 0.02, tip - side * 0.1), ...at(u + 0.05, 0.5)]).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(...at(u - 0.02, 0.5)).lineTo(...at(u - 0.008, tip - side * 0.12)).stroke({ width: 0.014, color: WOOD[2] });
    g.poly([...at(u - 0.025, tip - side * 0.11), ...at(u, tip), ...at(u + 0.025, tip - side * 0.11)]).fill(IRON[0]).stroke({ width: 0.012, color: INK, join: 'round' });
    g.circle(...at(u - 0.006, tip - side * 0.07), 0.01).fill(IRON[2]);
  });
  // The log, thick, its round end grain at both ends.
  g.moveTo(...at(0.12, 0.5)).lineTo(...at(len - 0.12, 0.5)).stroke({ width: 0.24, color: INK, cap: 'round' });
  g.moveTo(...at(0.12, 0.5)).lineTo(...at(len - 0.12, 0.5)).stroke({ width: 0.19, color: WOOD[0], cap: 'round' });
  g.moveTo(...at(0.16, 0.46)).lineTo(...at(len - 0.16, 0.46)).stroke({ width: 0.06, color: WOOD[1], cap: 'round' });
  for (const u of [0.1, len - 0.1]) {
    g.circle(...at(u, 0.5), 0.1).fill(0xb08a58).stroke({ width: LINE, color: INK });
    g.circle(...at(u, 0.5), 0.05).stroke({ width: 0.01, color: WOOD[0] });
  }
  // Lashings where the stakes pass through.
  for (const u of us) g.moveTo(...at(u - 0.06, 0.4)).lineTo(...at(u + 0.06, 0.6)).moveTo(...at(u + 0.06, 0.4)).lineTo(...at(u - 0.06, 0.6));
  g.stroke({ width: 0.022, color: 0xc8b080 });
}

/** Trapdoor: a hatch of planks set into the floor — a dark gap round its edge, iron strap hinges along one
 * side, an iron ring to lift it by. */
function trapdoor(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  g.poly(quad([x + 0.1, y + 0.1, x + 0.9, y + 0.9], rnd, 0.004)).fill(0x14100c);
  const face = board(g, [x + 0.14, y + 0.14, x + 0.86, y + 0.86], WOOD, light, rnd, false, 0.04, 1);
  planks(g, face, false, 4, WOOD[0], rnd);
  // Strap hinges along the top edge, nails in them.
  for (const hx of [x + 0.28, x + 0.64]) {
    g.poly(quad([hx, y + 0.13, hx + 0.07, y + 0.3], rnd, 0.002)).fill(IRON[1]).stroke({ width: 0.012, color: INK });
    g.circle(hx + 0.035, y + 0.24, 0.01).fill(IRON[0]);
  }
  // The ring.
  g.circle(x + 0.5, y + 0.74, 0.07).stroke({ width: 0.04, color: INK });
  g.circle(x + 0.5, y + 0.74, 0.07).stroke({ width: 0.022, color: IRON[2] });
  g.rect(x + 0.46, y + 0.64, 0.08, 0.05).fill(IRON[0]).stroke({ width: 0.012, color: INK });
}

/** Spiral stair from above: wedge-shaped stone treads winding down round a central post, each outlined
 * in ink, each step down darker than the one above and shaded across itself (lit at its front edge, dark
 * where the next one drops away); the outer edge rough, each tread ending at its own length. */
function spiralStair(g: Graphics, f: InteriorItem, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) * 0.46;
  const n = 10;
  const a0 = -Math.PI / 2;
  const sweep = Math.PI * 2 * 0.92;
  const reach = Array.from({ length: n }, () => r * (0.84 + 0.14 * rnd()));
  // The well under the treads, its edge as rough as theirs.
  const well: number[] = [];
  for (let i = 0; i < 36; i++) {
    const a = (i / 36) * Math.PI * 2;
    const rr = r * (0.97 + 0.06 * rnd());
    well.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
  }
  g.poly(well).fill(0x1a1612).stroke({ width: OUT, color: INK, join: 'round' });
  for (let i = n - 1; i >= 0; i--) {
    const t0 = a0 + (i / n) * sweep;
    const t1 = a0 + ((i + 1) / n) * sweep;
    const tone = shade(STONE[2], 1.15 - (i / n) * 0.85);
    const pts = [cx + Math.cos(t0) * r * 0.16, cy + Math.sin(t0) * r * 0.16];
    for (let k = 0; k <= 4; k++) {
      const t = t0 + ((t1 - t0) * k) / 4;
      const rr = reach[i] * (1 + (rnd() - 0.5) * 0.04);
      pts.push(cx + Math.cos(t) * rr, cy + Math.sin(t) * rr);
    }
    pts.push(cx + Math.cos(t1) * r * 0.16, cy + Math.sin(t1) * r * 0.16);
    // Shaded across the tread in thin slices, lit at its front edge, darker toward the back where the next
    // step drops away; one ink outline round the whole tread.
    const slices = 6;
    for (let k = 0; k < slices; k++) {
      const ta = t0 + ((t1 - t0) * k) / slices;
      const tb = t0 + ((t1 - t0) * (k + 1)) / slices;
      const rr = reach[i];
      const c = shade(tone, 1.3 - (0.68 * k) / (slices - 1));
      g.poly([cx + Math.cos(ta) * r * 0.16, cy + Math.sin(ta) * r * 0.16, cx + Math.cos(ta) * rr, cy + Math.sin(ta) * rr, cx + Math.cos(tb) * rr, cy + Math.sin(tb) * rr, cx + Math.cos(tb) * r * 0.16, cy + Math.sin(tb) * r * 0.16]).fill(c);
    }
    g.poly(pts).stroke({ width: 0.035, color: INK, join: 'round' });
  }
  // The newel post.
  g.circle(cx, cy, r * 0.17).fill(STONE[1]).stroke({ width: OUT, color: INK });
  g.circle(cx - r * 0.04, cy - r * 0.04, r * 0.07).fill(STONE[2]);
}

/** Stairs down to the deep dungeons: an opening in the floor framed in stone, steps going down into it,
 * each darker than the last until the dark swallows them, a pale arrow showing the way down. */
function linkDown(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y } = f;
  masonry(g, [x + 0.04, y + 0.04, x + 0.96, y + 0.96], STONE, rnd, 0.16);
  g.poly(quad([x + 0.16, y + 0.16, x + 0.84, y + 0.84], rnd, 0.004)).fill(0x0c0a08).stroke({ width: LINE, color: INK });
  const n = 5;
  for (let k = 0; k < n; k++) {
    const y0 = y + 0.16 + k * 0.12;
    g.rect(x + 0.18, y0, 0.64, 0.1).fill(shade(STONE[2], 1 - k * 0.19));
    g.moveTo(x + 0.18, y0).lineTo(x + 0.82, y0).stroke({ width: 0.014, color: shade(STONE[2], 1.15 - k * 0.19) });
  }
  g.moveTo(x + 0.5, y + 0.26).lineTo(x + 0.5, y + 0.66).moveTo(x + 0.38, y + 0.54).lineTo(x + 0.5, y + 0.68).lineTo(x + 0.62, y + 0.54);
  g.stroke({ width: 0.06, color: 0xf2e6c8, cap: 'round', join: 'round' });
}

/** Furniture with a back (InteriorLayer.backSide): to the wall, a bed's head to the wall at its end, a
 * pew's away from the altar, a booth seat's away from its table. */
export const ORIENTED = new Set(['shelf', 'bookcase', 'hearth', 'oven', 'forge', 'workbench', 'counter', 'bar', 'keg_rack', 'bed', 'couch', 'sideboard', 'pew', 'booth_seat', 'altar', 'statue', 'throne', 'alchemy_bench', 'weapon_rack', 'stage']);

/** Furniture drawn here: one of four looks each, picked by position (InteriorLayer.itemKey). */
export const VARIED = new Set([
  'bed', 'cot', 'table', 'long_table', 'chair', 'bench', 'chest', 'wardrobe', 'shelf', 'bookcase',
  'hearth', 'oven', 'forge', 'anvil', 'workbench', 'counter', 'bar', 'barrel', 'keg_rack', 'crate',
  'sideboard', 'desk', 'display', 'couch', 'pew', 'booth_table', 'booth_seat', 'rug', 'bath', 'bucket',
  'altar', 'statue', 'pillar', 'sarcophagus', 'bell', 'throne', 'stage',
  'vat', 'cauldron', 'alchemy_bench', 'weapon_rack', 'rack', 'cage', 'winch', 'telescope', 'barricade',
  'trapdoor', 'spiral_stair', 'link_down',
]);

/** Draws `f` if it is building furniture drawn here; false otherwise. */
export function drawFurniture(g: Graphics, f: InteriorItem, light: Light, v: number, back: [number, number] | null = null): boolean {
  if (!VARIED.has(f.kind)) return false;
  const rnd = rng(v * 7919 + f.w * 131 + f.h * 17 + f.kind.length * 1009 + f.kind.charCodeAt(0));
  switch (f.kind) {
    case 'bed':
      bed(g, f, light, v, rnd, back ?? (f.w > f.h ? [-1, 0] : [0, -1]));
      break;
    case 'couch':
      couch(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'sideboard':
      sideboard(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'desk':
      desk(g, f, light, rnd);
      break;
    case 'display':
      display(g, f, light, v, rnd);
      break;
    case 'pew':
      pew(g, f, light, rnd, back ?? (f.w >= f.h ? [0, 1] : [1, 0]));
      break;
    case 'booth_table':
      boothTable(g, f, light, rnd);
      break;
    case 'booth_seat':
      boothSeat(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'rug':
      rug(g, f, v, rnd);
      break;
    case 'bath':
      bath(g, f, light, rnd);
      break;
    case 'bucket':
      bucket(g, f, light, rnd);
      break;
    case 'altar':
      altar(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'statue':
      statue(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'pillar':
      pillar(g, f, light, rnd);
      break;
    case 'sarcophagus':
      sarcophagus(g, f, light, rnd);
      break;
    case 'bell':
      bell(g, f, light, rnd);
      break;
    case 'throne':
      throne(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'stage':
      stage(g, f, light, rnd, back ?? (f.w >= f.h ? [0, -1] : [-1, 0]));
      break;
    case 'vat':
      vat(g, f, light, rnd);
      break;
    case 'cauldron':
      cauldron(g, f, light, v, rnd);
      break;
    case 'alchemy_bench':
      alchemyBench(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'weapon_rack':
      weaponRack(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'rack':
      tortureRack(g, f, light, rnd);
      break;
    case 'cage':
      cage(g, f, rnd);
      break;
    case 'winch':
      winch(g, f, light, rnd);
      break;
    case 'telescope':
      telescope(g, f, light, rnd);
      break;
    case 'barricade':
      barricade(g, f);
      break;
    case 'trapdoor':
      trapdoor(g, f, light, rnd);
      break;
    case 'spiral_stair':
      spiralStair(g, f, rnd);
      break;
    case 'link_down':
      linkDown(g, f, rnd);
      break;
    case 'cot':
      cot(g, f, rnd);
      break;
    case 'table':
      if (f.w === 1 && f.h === 1) roundTable(g, f, light, rnd);
      else table(g, f, light, rnd, false);
      break;
    case 'long_table':
      table(g, f, light, rnd, true);
      break;
    case 'chair':
      chair(g, f, light, rnd);
      break;
    case 'bench':
      bench(g, f, light, rnd);
      break;
    case 'chest':
      chest(g, f, light, v, rnd);
      break;
    case 'wardrobe':
      wardrobe(g, f, light, rnd);
      break;
    case 'shelf':
      shelf(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'bookcase':
      bookcase(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'hearth':
      hearth(g, f, rnd, back ?? [0, -1]);
      break;
    case 'oven':
      oven(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'forge':
      forge(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'anvil':
      anvil(g, f, light, rnd);
      break;
    case 'workbench':
      workbench(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'counter':
      counter(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'bar':
      bar(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'barrel':
      barrel(g, f.x + f.w / 2, f.y + f.h / 2, Math.min(f.w, f.h) * 0.4, light, rnd);
      break;
    case 'keg_rack':
      for (let u = 0; u < Math.max(f.w, f.h); u++) keg(g, f, u, light, rnd, back ?? [0, -1]);
      break;
    case 'crate':
      crates(g, f, light, v, rnd);
      break;
  }
  return true;
}

/** Stairs up out of a level: a stone-framed flight rising toward the top of its square, each step higher
 * and lighter than the one below, a pale arrow up; out to the surface, daylight falling down them. */
function stairsUp(g: Graphics, f: InteriorItem, rnd: () => number, daylight: boolean) {
  const { x, y } = f;
  if (daylight) for (const [k, a] of [[0.75, 0.1], [0.55, 0.12], [0.35, 0.14]]) g.circle(x + 0.5, y + 0.3, k).fill({ color: 0xfff3d0, alpha: a });
  masonry(g, [x + 0.04, y + 0.04, x + 0.96, y + 0.96], STONE, rnd, 0.16);
  g.poly(quad([x + 0.16, y + 0.16, x + 0.84, y + 0.84], rnd, 0.004)).fill(STONE[0]).stroke({ width: LINE, color: INK });
  const n = 5;
  for (let k = 0; k < n; k++) {
    // The top step (k = 0) at y + 0.16, lightest.
    const y0 = y + 0.16 + k * 0.136;
    const c = shade(daylight ? 0xd8cfbd : STONE[2], 1.12 - k * 0.12);
    g.rect(x + 0.18, y0, 0.64, 0.12).fill(c);
    g.moveTo(x + 0.18, y0 + 0.12).lineTo(x + 0.82, y0 + 0.12).stroke({ width: 0.02, color: shade(c, 0.6) });
  }
  g.moveTo(x + 0.5, y + 0.72).lineTo(x + 0.5, y + 0.3).moveTo(x + 0.38, y + 0.42).lineTo(x + 0.5, y + 0.28).lineTo(x + 0.62, y + 0.42);
  g.stroke({ width: 0.06, color: daylight ? 0xfff6d8 : 0xf2e6c8, cap: 'round', join: 'round' });
}

/** A tunnel leaving through the rock wall: a straight passage about a square wide running away into the
 * rock, fading smoothly from the floor's colour at its mouth to black, rough rock faces either side. */
function tunnel(g: Graphics, f: InteriorItem, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const [mouth, end] = [0.2, -0.75];
  const half = (s0: number) => 0.4 - (mouth - s0) * 0.04;
  // The passage: many thin slices, each a shade darker, so the fade reads as one smooth run.
  const n = 24;
  for (let k = 0; k < n; k++) {
    const s0 = mouth + ((end - mouth) * k) / n;
    const s1 = mouth + ((end - mouth) * (k + 1)) / n + (k < n - 1 ? -0.004 : 0);
    const t = (k + 0.5) / n;
    const c = shade(STONE[1], Math.max(0.05, 1 - Math.pow(t, 0.8) * 0.95));
    g.poly([...at(0.5 - half(s0), s0), ...at(0.5 + half(s0), s0), ...at(0.5 + half(s1), s1), ...at(0.5 - half(s1), s1)]).fill(c);
  }
  // Rough rock faces either side, running straight back.
  for (const side of [-1, 1]) {
    const inner: number[] = [];
    for (let k = 0; k <= 6; k++) {
      const sk = mouth + 0.04 + ((end - mouth - 0.04) * k) / 6;
      inner.push(...at(0.5 + side * (half(sk) + (rnd() - 0.5) * 0.03), sk));
    }
    const pts = [...inner];
    for (let k = 6; k >= 0; k--) {
      const sk = mouth + 0.04 + ((end - mouth - 0.04) * k) / 6;
      pts.push(...at(0.5 + side * (0.5 + (rnd() - 0.5) * 0.03), sk));
    }
    g.poly(pts).fill(0x4a443c);
    for (let k = 0; k < 6; k++) g.moveTo(inner[k * 2], inner[k * 2 + 1]).lineTo(inner[k * 2 + 2], inner[k * 2 + 3]);
    g.stroke({ width: OUT, color: INK, cap: 'round' });
  }
}

/** A ladder against the wall, up to a grate in the street above: two rails, rungs, a square of grey light
 * falling through the grate onto its foot. */
function ladder(g: Graphics, f: InteriorItem, rnd: () => number, wall: [number, number]) {
  const { at, rect } = frame(f, wall);
  // The grate's light on the floor below it.
  g.poly(quad(rect(0.2, 0.02, 0.8, 0.62), rnd, 0)).fill({ color: 0xd8dce0, alpha: 0.18 });
  for (let k = 1; k < 4; k++) g.moveTo(...at(0.2 + k * 0.15, 0.02)).lineTo(...at(0.2 + k * 0.15, 0.62));
  g.stroke({ width: 0.03, color: 0x14100c, alpha: 0.25 });
  for (const u of [0.32, 0.68]) {
    g.moveTo(...at(u, -0.22)).lineTo(...at(u, 0.7)).stroke({ width: 0.07, color: INK, cap: 'round' });
    g.moveTo(...at(u, -0.22)).lineTo(...at(u, 0.7)).stroke({ width: 0.04, color: WOOD[1], cap: 'round' });
  }
  for (const s0 of [-0.08, 0.14, 0.32, 0.5]) {
    g.moveTo(...at(0.32, s0)).lineTo(...at(0.68, s0)).stroke({ width: 0.05, color: INK, cap: 'round' });
    g.moveTo(...at(0.32, s0)).lineTo(...at(0.68, s0)).stroke({ width: 0.028, color: WOOD[2], cap: 'round' });
  }
}

/** A drain outlet in the wall: a short, wide iron pipe standing out of the rock, its back rounded where it
 * goes into the wall, its sides the width of its mouth, shaded as a cylinder with a band near the end; dark
 * inside, a trickle of muck running from it to a puddle on the floor. */
function pipe(g: Graphics, f: InteriorItem, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const [r, back, mouth] = [0.25, -0.02, 0.16];
  g.poly(ring(...at(0.5, 0.66), 0.16, rnd, 0.05, 12)).fill({ color: 0x4a5a32, alpha: 0.8 });
  g.moveTo(...at(0.5, mouth + 0.04)).quadraticCurveTo(...at(0.46, 0.4), ...at(0.5, 0.56)).stroke({ width: 0.06, color: 0x5a6a3a, cap: 'round' });
  // The body: straight sides the mouth's width, the back a curve bulging into the wall.
  const body = (k: number) => {
    const pts: number[] = [...at(0.5 - r * k, mouth)];
    for (let i = 0; i <= 12; i++) {
      const t = Math.PI - (i / 12) * Math.PI;
      pts.push(...at(0.5 + Math.cos(t) * r * k, back - Math.sin(t) * 0.12 * k));
    }
    pts.push(...at(0.5 + r * k, mouth));
    return pts;
  };
  g.poly(body(1)).fill(IRON[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(body(0.72)).fill(IRON[1]);
  g.poly([...at(0.5 - r * 0.6, back), ...at(0.5 - r * 0.3, back), ...at(0.5 - r * 0.3, mouth), ...at(0.5 - r * 0.6, mouth)]).fill({ color: IRON[2], alpha: 0.7 });
  // A band just behind the mouth.
  g.poly([...at(0.5 - r * 1.04, mouth - 0.07), ...at(0.5 + r * 1.04, mouth - 0.07), ...at(0.5 + r * 1.04, mouth - 0.03), ...at(0.5 - r * 1.04, mouth - 0.03)]).fill(IRON[0]).stroke({ width: 0.012, color: INK });
  // The mouth: an ellipse the body's width, dark inside.
  const rot = wall[0] !== 0 ? Math.PI / 2 : 0;
  g.poly(oval(...at(0.5, mouth), r, 0.1, rot, 18)).fill(IRON[1]).stroke({ width: OUT, color: INK });
  g.poly(oval(...at(0.5, mouth), r * 0.72, 0.07, rot, 18)).fill(0x0e0c0a);
  g.poly(oval(...at(0.5, mouth + 0.03), r * 0.45, 0.025, rot, 12)).fill({ color: 0x5a6a3a, alpha: 0.9 });
}

/** An old well: a ring of fitted stones round dark water with a glint, a rope down into it from a beam
 * across, a bucket on the rim. */
function well(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) * 0.42;
  g.poly(ring(cx, cy, r, rnd, 0.01, 24)).fill(STONE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const n = 10;
  for (let i = 0; i < n; i++) {
    const a0 = (i / n) * Math.PI * 2 + 0.05;
    const a1 = ((i + 1) / n) * Math.PI * 2 - 0.05;
    const pts: number[] = [];
    for (let k = 0; k <= 3; k++) pts.push(cx + Math.cos(a0 + ((a1 - a0) * k) / 3) * r * 0.95, cy + Math.sin(a0 + ((a1 - a0) * k) / 3) * r * 0.95);
    for (let k = 3; k >= 0; k--) pts.push(cx + Math.cos(a0 + ((a1 - a0) * k) / 3) * r * 0.66, cy + Math.sin(a0 + ((a1 - a0) * k) / 3) * r * 0.66);
    g.poly(pts).fill(shade(STONE[1], 0.95 + 0.15 * lit((a0 + a1) / 2)));
  }
  g.circle(cx, cy, r * 0.62).fill(0x14222a).stroke({ width: LINE, color: INK });
  g.poly(oval(cx + light[0] * r * 0.25, cy + light[1] * r * 0.25, r * 0.18, r * 0.07, -0.6)).fill({ color: 0x6a9aaa, alpha: 0.7 });
  g.moveTo(cx - r * 1.02, cy).lineTo(cx + r * 1.02, cy).stroke({ width: 0.08, color: INK, cap: 'round' });
  g.moveTo(cx - r * 1.02, cy).lineTo(cx + r * 1.02, cy).stroke({ width: 0.05, color: WOOD[1], cap: 'round' });
  g.circle(cx, cy, 0.03).fill(0xc8b080).stroke({ width: 0.01, color: INK });
  const [bx, by] = [cx + r * 0.7, cy + r * 0.7];
  g.circle(bx, by, 0.09).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  g.circle(bx, by, 0.06).fill(0x3e5a6a);
}

/** A pit: a hole in the floor with a broken, rocky rim, dropping away into darkness in rough rings. */
function pit(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) * 0.46;
  const rough = (k: number, j: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 22; i++) {
      const a = (i / 22) * Math.PI * 2;
      const rr = r * k * (1 + (rnd() - 0.5) * j);
      pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    return pts;
  };
  g.poly(rough(1, 0.12)).fill(STONE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(rough(0.88, 0.14)).fill(0x3a322a);
  g.poly(rough(0.72, 0.16).map((p, i) => p - (i % 2 ? light[1] : light[0]) * r * 0.06)).fill(0x221c16);
  g.poly(rough(0.52, 0.18).map((p, i) => p - (i % 2 ? light[1] : light[0]) * r * 0.1)).fill(0x100c0a);
  g.poly(rough(0.3, 0.2).map((p, i) => p - (i % 2 ? light[1] : light[0]) * r * 0.12)).fill(0x050404);
  // Broken stones on the rim, a crack or two running out from it.
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI * 2;
    g.poly(ring(cx + Math.cos(a) * r * 0.95, cy + Math.sin(a) * r * 0.95, 0.06 + 0.04 * rnd(), rnd, 0.03, 6)).fill(STONE[1]).stroke({ width: 0.012, color: INK });
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * r, cy + Math.sin(a) * r).lineTo(cx + Math.cos(a + 0.1) * r * 1.12, cy + Math.sin(a + 0.1) * r * 1.12);
  }
  g.stroke({ width: LINE, color: INK });
}

/** A hidden pressure plate (seen only by the DM): a bold red-tinted square with a thick dashed red border
 * and a warning glyph (!) on the plate, so it stands out at a glance. */
function trap(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y } = f;
  g.poly(quad([x + 0.1, y + 0.1, x + 0.9, y + 0.9], rnd, 0)).fill({ color: 0xd8301e, alpha: 0.28 });
  g.poly(quad([x + 0.26, y + 0.26, x + 0.74, y + 0.74], rnd, 0.004)).fill(shade(STONE[1], 0.9)).stroke({ width: 0.02, color: INK });
  const d = 0.1;
  for (const [ax, ay, bx, by] of [
    [0.1, 0.1, 0.9, 0.1],
    [0.9, 0.1, 0.9, 0.9],
    [0.9, 0.9, 0.1, 0.9],
    [0.1, 0.9, 0.1, 0.1],
  ]) {
    const len = Math.hypot(bx - ax, by - ay);
    for (let t = 0; t < len; t += d * 2) {
      const [t0, t1] = [t / len, Math.min(1, (t + d) / len)];
      g.moveTo(x + ax + (bx - ax) * t0, y + ay + (by - ay) * t0).lineTo(x + ax + (bx - ax) * t1, y + ay + (by - ay) * t1);
    }
  }
  g.stroke({ width: 0.06, color: 0xd8301e, cap: 'round' });
  g.moveTo(x + 0.5, y + 0.36).lineTo(x + 0.5, y + 0.54).stroke({ width: 0.07, color: 0xd8301e, cap: 'round' });
  g.circle(x + 0.5, y + 0.64, 0.04).fill(0xd8301e);
}

/** An unstable ceiling (players see it too): grit and small stones fallen on the floor beneath it, cracks
 * in the floor where bigger ones struck. */
function caveIn(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y, w, h } = f;
  const [cx, cy] = [x + w / 2, y + h / 2];
  for (let i = 0; i < 40; i++) g.circle(x + 0.15 + rnd() * (w - 0.3), y + 0.15 + rnd() * (h - 0.3), 0.012 + 0.012 * rnd()).fill({ color: 0x8a8070, alpha: 0.8 });
  for (let i = 0; i < 5; i++) {
    const [px, py] = [x + 0.2 + rnd() * (w - 0.4), y + 0.2 + rnd() * (h - 0.4)];
    g.poly(ring(px, py, 0.05 + 0.04 * rnd(), rnd, 0.03, 6)).fill(STONE[1]).stroke({ width: 0.012, color: INK });
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    let [px, py] = [cx, cy];
    g.moveTo(px, py);
    for (let k = 0; k < 3; k++) {
      px += Math.cos(a + (rnd() - 0.5)) * 0.1;
      py += Math.sin(a + (rnd() - 0.5)) * 0.1;
      g.lineTo(px, py);
    }
  }
  g.stroke({ width: 0.015, color: INK, alpha: 0.8 });
}

/** A faceted stone from above: a dark rim on the shaded side, its face, a lit top facet toward the light
 * with facet lines down to it, a sunlit spot, a crack on big ones. */
function rock(g: Graphics, x: number, y: number, s: number, t: readonly number[], light: Light, rnd: () => number, n = 8) {
  const base: number[] = [];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    const rr = s * (0.82 + 0.3 * rnd());
    base.push(Math.cos(a) * rr, Math.sin(a) * rr);
  }
  const at = (k: number, dx: number, dy: number) => base.map((p, i) => p * k + (i % 2 ? y + dy : x + dx));
  g.poly(at(1, 0, 0)).fill(t[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const body = at(0.86, light[0] * s * 0.06, light[1] * s * 0.06);
  g.poly(body).fill(t[1]);
  const top = at(0.52, light[0] * s * 0.18, light[1] * s * 0.18);
  g.poly(top).fill(t[2]);
  for (let i = 0; i < base.length; i += 4) g.moveTo(top[i], top[i + 1]).lineTo(body[i], body[i + 1]);
  g.stroke({ width: 0.014, color: t[0], alpha: 0.7, cap: 'round' });
  if (s > 0.3) {
    const a = rnd() * Math.PI * 2;
    let [cx, cy] = [x + Math.cos(a) * s * 0.8, y + Math.sin(a) * s * 0.8];
    g.moveTo(cx, cy);
    for (let k = 0; k < 3; k++) {
      cx += (x - cx) * 0.3 + (rnd() - 0.5) * s * 0.15;
      cy += (y - cy) * 0.3 + (rnd() - 0.5) * s * 0.15;
      g.lineTo(cx, cy);
    }
    g.stroke({ width: 0.02, color: shade(t[0], 0.7), cap: 'round', join: 'round' });
  }
}

/** A crystal shard from (x, y) out along `a`: a kite split down its spine, its half toward the light paler,
 * a glint at the tip. */
function shard(g: Graphics, x: number, y: number, a: number, len: number, w: number, t: readonly number[], line: number, light: Light) {
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const tip = [x + c * len, y + s * len];
  const l = [x + c * len * 0.35 - s * w, y + s * len * 0.35 + c * w];
  const r = [x + c * len * 0.35 + s * w, y + s * len * 0.35 - c * w];
  const base = [x - c * w * 0.4, y - s * w * 0.4];
  g.poly([...base, ...l, ...tip, ...r]).fill(t[0]).stroke({ width: 0.02, color: line, join: 'round' });
  const litSide = -s * light[0] + c * light[1] > 0 ? l : r;
  g.poly([...base, ...litSide, ...tip]).fill(t[2]);
  g.moveTo(base[0], base[1]).lineTo(tip[0], tip[1]).stroke({ width: 0.01, color: t[1] });
  g.circle(tip[0] - c * len * 0.12, tip[1] - s * len * 0.12, w * 0.25).fill({ color: t[3], alpha: 0.9 });
}

/** A cluster of shards round a point, shaded ones first, a soft glow round it. */
function shards(g: Graphics, cx: number, cy: number, r: number, n: number, t: readonly number[], line: number, glow: number, light: Light, rnd: () => number) {
  for (const [k, a] of [
    [1.25, 0.08],
    [0.9, 0.1],
  ]) {
    g.circle(cx, cy, r * k).fill({ color: glow, alpha: a });
  }
  const ph = rnd() * Math.PI * 2;
  const parts: [number, number, number][] = [];
  for (let i = 0; i < n; i++) parts.push([ph + ((i + (rnd() - 0.5) * 0.6) / n) * Math.PI * 2, r * (0.6 + 0.35 * rnd()), r * (0.18 + 0.06 * rnd())]);
  parts.sort((p, q) => -(Math.cos(p[0]) * light[0] + Math.sin(p[0]) * light[1]) + (Math.cos(q[0]) * light[0] + Math.sin(q[0]) * light[1]));
  for (const [a, len, w] of parts) shard(g, cx + Math.cos(a) * r * 0.08, cy + Math.sin(a) * r * 0.08, a, len, w, t, line, light);
  for (let i = 0; i < 3; i++) shard(g, cx, cy, ph + i * 2.1 + 0.5, r * 0.4, r * 0.15, t.map((c) => shade(c, 1.08)), line, light);
}

/** A rounded, rough-edged patch of points round (cx, cy). */
function patch(cx: number, cy: number, rx: number, ry: number, rnd: () => number, j = 0.18, n = 18): number[] {
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const k = 1 + (rnd() - 0.5) * j * 2;
    pts.push(cx + Math.cos(a) * rx * k, cy + Math.sin(a) * ry * k);
  }
  return pts;
}

const CAVE_ROCK = [0x5a5246, 0x857a6a, 0xa89c88, 0xc8bca6];

/** Stalagmite: a cone of wet rock rising from the floor — rings of flowstone climbing to a pale tip set toward
 * the light, a drip glint on it, a shadow away from the light. */
function stalagmite(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.36;
  castShadow(g, patch(cx, cy, r, r, rnd, 0.1, 14), light, 0.5);
  const rings = 4;
  for (let k = 0; k < rings; k++) {
    const kk = 1 - k * 0.22;
    const off = (k / rings) * r * 0.45;
    g.poly(patch(cx + light[0] * off, cy + light[1] * off, r * kk, r * kk, rnd, 0.12, 16)).fill(shade(CAVE_ROCK[1], 0.8 + k * 0.14)).stroke({ width: k === 0 ? OUT : 0.016, color: k === 0 ? INK : CAVE_ROCK[0], join: 'round' });
  }
  const [tx, ty] = [cx + light[0] * r * 0.5, cy + light[1] * r * 0.5];
  g.circle(tx, ty, r * 0.12).fill(CAVE_ROCK[3]).stroke({ width: 0.012, color: CAVE_ROCK[0] });
  g.circle(tx + light[0] * 0.02, ty + light[1] * 0.02, 0.025).fill({ color: 0xffffff, alpha: 0.7 });
}

/** Rock column: a thick pillar of flowstone joining floor and ceiling — a lumpy outline, ripples of flowstone
 * round it, lit down one side, a long shadow. */
function rockColumn(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.44;
  castShadow(g, patch(cx, cy, r, r, rnd, 0.1, 16), light, 1.1);
  const out = patch(cx, cy, r, r, rnd, 0.1, 20);
  g.poly(out).fill(CAVE_ROCK[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + light[0] * 0.03, cy + light[1] * 0.03, r * 0.86, r * 0.86, rnd, 0.08, 20)).fill(CAVE_ROCK[1]);
  for (const k of [0.68, 0.48, 0.28]) g.poly(patch(cx + light[0] * (0.86 - k) * 0.1, cy + light[1] * (0.86 - k) * 0.1, r * k, r * k, rnd, 0.1, 16)).stroke({ width: 0.018, color: CAVE_ROCK[0], alpha: 0.8 });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la - 0.9) * r * 0.78, cy + Math.sin(la - 0.9) * r * 0.78)
    .arc(cx, cy, r * 0.78, la - 0.9, la + 0.9)
    .stroke({ width: 0.04, color: CAVE_ROCK[2], alpha: 0.8 });
}

/** Boulder: a big faceted stone, a pebble or two at its foot, a shadow away from the light. */
function caveBoulder(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const s = Math.min(f.w, f.h) * 0.38;
  castShadow(g, patch(cx, cy, s, s, rnd, 0.15, 10), light, 0.5);
  for (let i = 0; i < 2; i++) {
    const a = Math.PI * 0.25 + (rnd() - 0.5) * 2;
    rock(g, cx + Math.cos(a) * s * 1.15, cy + Math.sin(a) * s * 1.15, s * 0.16, CAVE_ROCK, light, rnd, 6);
  }
  rock(g, cx, cy, s, CAVE_ROCK, light, rnd, 9);
}

/** Crystal cluster (after a reference picture): long faceted shards of uneven length radiating outward from a shared
 * heart in different directions, each split into flat facets in light and dark tones with a highlight streak, and a
 * few stubby upright prisms standing in the middle, seen from a high angle with their flat six-sided tops showing. */
function crystal(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const t = [
    [0x2a6a9a, 0x4a9ac8, 0x8acaea, 0xd8f4ff],
    [0x4a2a8a, 0x7a5ac0, 0xb08ae8, 0xece0ff],
    [0x1e6a62, 0x3a9a8e, 0x7ad0c4, 0xd8fff8],
    [0x2a6a9a, 0x4a9ac8, 0x8acaea, 0xd8f4ff],
  ][v % 4];
  const line = 0x14202a;
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt, up } = screenAxes(light);
  for (let i = 0; i < 4; i++) g.circle(cx, cy, 0.3 + i * 0.05).fill({ color: t[2], alpha: 0.05 });
  // One long faceted prism lying outward along angle `a`: two long faces split by an off-centre ridge, a pointed
  // tip of its own two facets, a crack across each long face; the face turned to the light is pale.
  const shard = (a: number, len: number, w: number) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const p = (u: number, k: number): [number, number] => [cx + c * u - s * k, cy + s * u + c * k];
    const lit = -s * light[0] + c * light[1] > 0 ? 1 : -1;
    const r = w * 0.2 * (rnd() - 0.5);
    const sh = len * (0.6 + 0.15 * rnd());
    const [B, BL, BR] = [p(-0.04, r), p(-0.04, w), p(-0.04, -w)];
    const [SL, SR, SM] = [p(sh, w * 0.92), p(sh, -w * 0.92), p(sh + w * 0.3, r)];
    const T = p(len, r * 0.5);
    const tone = (k: number, tip: boolean) => (k === lit ? (tip ? t[3] : t[2]) : tip ? t[1] : t[0]);
    g.poly([...B, ...BL, ...SL, ...SM]).fill(tone(1, false));
    g.poly([...B, ...BR, ...SR, ...SM]).fill(tone(-1, false));
    g.poly([...SL, ...T, ...SM]).fill(tone(1, true));
    g.poly([...SR, ...T, ...SM]).fill(tone(-1, true));
    const u1 = sh * (0.3 + 0.35 * rnd());
    const u2 = sh * (0.3 + 0.35 * rnd());
    g.moveTo(...B).lineTo(...SM).lineTo(...T).moveTo(...SL).lineTo(...SM).lineTo(...SR);
    g.moveTo(...p(u1, r)).lineTo(...p(u1 + w * 0.5, w)).moveTo(...p(u2, r)).lineTo(...p(u2 - w * 0.4, -w));
    g.stroke({ width: 0.012, color: line, alpha: 0.75 });
    g.poly([...BL, ...SL, ...T, ...SR, ...BR]).stroke({ width: 0.026, color: line, join: 'round' });
    const hk = lit * w * 0.55;
    g.moveTo(...p(sh * 0.15, hk)).lineTo(...p(sh * 0.85, hk * 0.9)).stroke({ width: 0.018, color: 0xffffff, alpha: 0.6, cap: 'round' });
  };
  // Eight uneven prisms all round, the ones pointing away (screen up) first so the near ones overlap them.
  const n = 8;
  const ph = rnd() * Math.PI * 2;
  const outs: [number, number, number][] = [];
  for (let i = 0; i < n; i++) outs.push([ph + ((i + (rnd() - 0.5) * 0.8) / n) * Math.PI * 2, 0.34 + 0.16 * rnd(), 0.085 + 0.035 * rnd()]);
  outs.sort((p, q) => (Math.cos(q[0]) * up[0] + Math.sin(q[0]) * up[1]) - (Math.cos(p[0]) * up[0] + Math.sin(p[0]) * up[1]));
  for (const [a, len, w] of outs) shard(a, len, w);
  // Stubby upright hexagonal prisms in the middle, seen from high up: three side faces, a flat hexagonal top.
  const stubs: [number, number, number, number][] = [
    [0.02, 0.05, 0.16, 0.09],
    [-0.11, -0.02, 0.11, 0.08],
    [0.12, -0.06, 0.08, 0.07],
  ];
  for (const [a, b, h, w] of stubs) {
    const base = pt(cx, cy, a, b);
    const at = (x: number, y: number, z: number) => pt(base[0], base[1], x * w, y * w * 0.45 + z);
    const ring = (z: number) => [at(-1, 0, z), at(-0.5, -1, z), at(0.5, -1, z), at(1, 0, z), at(0.5, 1, z), at(-0.5, 1, z)];
    const [lo, hi] = [ring(0), ring(h)];
    const face = (i: number, j: number, col: number) => g.poly([...lo[i], ...lo[j], ...hi[j], ...hi[i]]).fill(col);
    face(0, 1, t[2]);
    face(1, 2, t[1]);
    face(2, 3, t[0]);
    g.poly(hi.flat()).fill(t[3]);
    g.moveTo(...lo[1]).lineTo(...hi[1]).moveTo(...lo[2]).lineTo(...hi[2]).stroke({ width: 0.01, color: line, alpha: 0.75 });
    g.poly([...lo[0], ...lo[1], ...lo[2], ...lo[3], ...hi[3], ...hi[4], ...hi[5], ...hi[0]]).stroke({ width: 0.024, color: line, join: 'round' });
    g.poly(hi.flat()).stroke({ width: 0.012, color: line, alpha: 0.75, join: 'round' });
    g.moveTo(...at(-0.75, -0.3, h * 0.2)).lineTo(...at(-0.75, -0.3, h * 0.85)).stroke({ width: 0.014, color: 0xffffff, alpha: 0.7, cap: 'round' });
  }
}

/** Ice formation: a rounded, melted mound of pale ice with stubby icicles standing out of it, a frosted fringe
 * round its foot, cracks and bubbles inside; cold, not glowing. */
function iceFormation(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  for (let i = 0; i < 30; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.38 + 0.1 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.012 + 0.01 * rnd()).fill({ color: 0xf0f8ff, alpha: 0.8 });
  }
  const mound = patch(cx, cy, 0.38, 0.34, rnd, 0.12, 20);
  g.poly(mound).fill(0xa8cce0).stroke({ width: OUT, color: 0x3e6a88, join: 'round' });
  g.poly(patch(cx + light[0] * 0.04, cy + light[1] * 0.04, 0.3, 0.26, rnd, 0.1, 18)).fill(0xcae4f2);
  g.poly(patch(cx + light[0] * 0.1, cy + light[1] * 0.1, 0.14, 0.11, rnd, 0.1, 12)).fill({ color: 0xffffff, alpha: 0.75 });
  // Stubby icicles standing out of the mound: short rounded cones.
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.08 + 0.12 * rnd();
    const [px, py] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
    g.circle(px, py, 0.06).fill(0xe0f0fa).stroke({ width: 0.014, color: 0x5a8aa8 });
    g.circle(px + light[0] * 0.02, py + light[1] * 0.02, 0.025).fill(0xffffff);
  }
  g.moveTo(cx - 0.2, cy + 0.05).lineTo(cx - 0.05, cy + 0.12).lineTo(cx + 0.05, cy + 0.04).stroke({ width: 0.012, color: 0x5a8aa8, alpha: 0.8 });
  for (let i = 0; i < 4; i++) g.circle(cx + (rnd() - 0.5) * 0.4, cy + (rnd() - 0.5) * 0.3, 0.012).stroke({ width: 0.006, color: 0xffffff });
}

/** For things drawn upright from a high angle (glowing fungus, crystals): screen up and screen right in grid axes,
 * from the light (screen NW, so screen up is the light turned 45 degrees), and a point `a` right and `b` up of
 * (x, y). */
function screenAxes(light: Light) {
  const la = Math.atan2(light[1], light[0]) + Math.PI / 4;
  const up: [number, number] = [Math.cos(la), Math.sin(la)];
  const right: [number, number] = [Math.cos(la + Math.PI / 2), Math.sin(la + Math.PI / 2)];
  const pt = (x: number, y: number, a: number, b: number): [number, number] => [x + right[0] * a + up[0] * b, y + right[1] * a + up[1] * b];
  return { up, right, pt };
}

/** Glowing fungus, seen from a high bird's-eye angle (upright things foreshortened, their tops showing): a clump of
 * luminous mushrooms standing out of a dark patch of ground — pale stems curving gently, caps seen mostly from above
 * (a glowing top, the darker gilled rim showing below), the tallest at the back — a wide soft glow on the floor, a
 * few spores drifting. */
function fungus(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt, right } = screenAxes(light);
  const rot = Math.atan2(right[1], right[0]);
  const lift = 0.85;
  const c = v % 2 ? [0x2e1458, 0x7a44c8, 0xb48ae8, 0xf0e4ff] : [0x083e38, 0x12a090, 0x6ee0cc, 0xe0fff8];
  for (const [k, al] of [
    [0.95, 0.05],
    [0.75, 0.07],
    [0.55, 0.09],
    [0.35, 0.1],
  ]) {
    g.circle(cx, cy, k).fill({ color: c[1], alpha: al });
  }
  g.poly(oval(...pt(cx, cy, 0, -0.22), 0.3, 0.12, rot, 18)).fill({ color: 0x14100c, alpha: 0.7 });
  // [right, back (up the screen), height, cap radius, bend]: back ones first.
  const shrooms: [number, number, number, number, number][] = [
    [-0.04, 0.06, 0.5, 0.18, 0.08],
    [0.2, 0.0, 0.36, 0.14, 0.1],
    [-0.24, -0.04, 0.3, 0.12, -0.1],
    [0.04, -0.18, 0.22, 0.11, 0.06],
    [0.26, -0.22, 0.14, 0.08, 0.08],
  ].map(([a, d, h, r, bend]) => [a + (rnd() - 0.5) * 0.04, d, h * (0.9 + 0.2 * rnd()), r, bend * (0.8 + 0.4 * rnd())]);
  for (const [a, d, h, r, bend] of shrooms) {
    const base = pt(cx, cy, a, d * 0.5 - 0.22);
    const hh = h * lift;
    // The stem: a tapering curve bowed sideways by `bend`, built from points along it.
    const sw = r * 0.26;
    const along = (t: number): [number, number] => pt(base[0], base[1], bend * Math.sin(Math.PI * t * 0.8) + bend * t * 0.3, hh * t);
    const left: number[] = [];
    const rightSide: number[] = [];
    for (let k = 0; k <= 8; k++) {
      const t = k / 8;
      const [px, py] = along(t);
      const w = sw * (1 - 0.3 * t);
      left.push(...pt(px, py, -w, 0));
      rightSide.push(...pt(px, py, w, 0));
    }
    const stem = [...left];
    for (let k = 8; k >= 0; k--) stem.push(rightSide[k * 2], rightSide[k * 2 + 1]);
    g.poly(stem).fill(0xe0d8c8).stroke({ width: 0.014, color: INK, join: 'round' });
    const top = along(1);
    // The cap from above: a wide oval, the gilled underside showing as a darker crescent below the glowing top.
    g.poly(oval(...pt(top[0], top[1], 0, -r * 0.08), r, r * 0.66, rot, 22)).fill(c[0]).stroke({ width: 0.016, color: INK });
    for (let k = 0; k < 9; k++) {
      const t = Math.PI * (0.15 + (0.7 * k) / 8);
      g.moveTo(...pt(top[0], top[1], Math.cos(t) * r * 0.55, -r * 0.08 - Math.sin(t) * r * 0.35)).lineTo(...pt(top[0], top[1], Math.cos(t) * r * 0.92, -r * 0.08 - Math.sin(t) * r * 0.6));
    }
    g.stroke({ width: 0.008, color: c[2], alpha: 0.7 });
    g.poly(oval(...pt(top[0], top[1], 0, r * 0.06), r * 0.94, r * 0.58, rot, 22)).fill(c[1]).stroke({ width: 0.014, color: INK });
    g.poly(oval(...pt(top[0], top[1], -r * 0.22, r * 0.2), r * 0.42, r * 0.24, rot, 14)).fill({ color: c[2], alpha: 0.9 });
    g.circle(...pt(top[0], top[1], -r * 0.28, r * 0.26), r * 0.09).fill(c[3]);
  }
  for (let i = 0; i < 8; i++) {
    const [px, py] = pt(cx, cy, (rnd() - 0.5) * 0.8, -0.3 + rnd() * 0.65);
    g.circle(px, py, 0.01 + 0.008 * rnd()).fill({ color: c[3], alpha: 0.9 });
  }
}

/** Giant mushrooms: two or three broad caps, each with a darker rim, a lit crown and pale spots, gills
 * showing under the edge on the shaded side. */
function giantMushrooms(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const caps = [
    [0x5a2a4a, 0x8a4a6e, 0xb06a90],
    [0x6a3a1a, 0x9a5a2c, 0xc07a44],
    [0x3a4a6a, 0x5a6a9a, 0x8a9ac0],
    [0x5a2a4a, 0x8a4a6e, 0xb06a90],
  ][v % 4];
  const parts: [number, number, number][] = [
    [0.38, 0.58, 0.28],
    [0.68, 0.36, 0.22],
    [0.7, 0.72, 0.15],
  ];
  for (const [px, py, r] of parts) {
    const [x, y] = [f.x + px, f.y + py];
    g.circle(x - light[0] * r * 0.25, y - light[1] * r * 0.25, r).fill({ color: 0x14100c, alpha: 0.3 });
    g.circle(x, y, r).fill(caps[0]).stroke({ width: OUT, color: INK });
    for (let k = 0; k < 10; k++) {
      const a = (k / 10) * Math.PI * 2;
      if (Math.cos(a) * light[0] + Math.sin(a) * light[1] > 0.2) continue;
      g.moveTo(x + Math.cos(a) * r * 0.78, y + Math.sin(a) * r * 0.78).lineTo(x + Math.cos(a) * r * 0.97, y + Math.sin(a) * r * 0.97);
    }
    g.stroke({ width: 0.012, color: 0xd8c8b0, alpha: 0.7 });
    g.circle(x + light[0] * r * 0.12, y + light[1] * r * 0.12, r * 0.78).fill(caps[1]);
    g.circle(x + light[0] * r * 0.35, y + light[1] * r * 0.35, r * 0.32).fill({ color: caps[2], alpha: 0.85 });
    for (let k = 0; k < 4; k++) {
      const a = rnd() * Math.PI * 2;
      const d = r * 0.55 * Math.sqrt(rnd());
      g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, r * (0.08 + 0.05 * rnd())).fill({ color: 0xf5ecd6, alpha: 0.85 });
    }
  }
}

/** Thick webs: a sheet of web strung across the square — radial threads from a centre, a spiral round them,
 * thicker sagging strands, a wrapped bundle caught in it. */
function web(g: Graphics, f: InteriorItem, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5 + (rnd() - 0.5) * 0.1, f.y + 0.5 + (rnd() - 0.5) * 0.1];
  const n = 9;
  const ends: number[][] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + (rnd() - 0.5) * 0.3;
    const r = 0.45 + 0.08 * rnd();
    ends.push([Math.cos(a) * r, Math.sin(a) * r]);
  }
  g.poly(ends.flatMap(([ex, ey]) => [cx + ex, cy + ey])).fill({ color: 0xf5f5f4, alpha: 0.12 });
  for (const [ex, ey] of ends) g.moveTo(cx, cy).lineTo(cx + ex, cy + ey);
  g.stroke({ width: 0.022, color: 0xf5f5f4, alpha: 0.85 });
  for (const k of [0.22, 0.38, 0.54, 0.7, 0.86]) {
    g.moveTo(cx + ends[0][0] * k, cy + ends[0][1] * k);
    for (let i = 1; i <= n; i++) {
      const [px, py] = ends[(i - 1) % n];
      const [qx, qy] = ends[i % n];
      const sag = 0.85;
      g.quadraticCurveTo(cx + ((px + qx) / 2) * k * sag, cy + ((py + qy) / 2) * k * sag, cx + qx * k, cy + qy * k);
    }
  }
  g.stroke({ width: 0.014, color: 0xf5f5f4, alpha: 0.75 });
  // A wrapped bundle caught in it.
  const [bx, by] = [cx + ends[2][0] * 0.5, cy + ends[2][1] * 0.5];
  g.poly(oval(bx, by, 0.09, 0.05, rnd() * Math.PI, 12)).fill(0xe8e4d8).stroke({ width: 0.012, color: 0x8a8478 });
  g.moveTo(bx - 0.06, by - 0.02).lineTo(bx + 0.06, by + 0.02).stroke({ width: 0.008, color: 0x8a8478 });
}

/** Cobwebs in the angle of the wall: a fan of threads from the wall with sagging cross-strands. */
function cobweb(g: Graphics, f: InteriorItem, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const [ox, oy] = at(0.5, 0);
  const n = 7;
  const ends: number[][] = [];
  for (let i = 0; i < n; i++) {
    const a = Math.PI * (0.08 + (0.84 * i) / (n - 1)) + (rnd() - 0.5) * 0.1;
    const r = 0.5 + 0.1 * rnd();
    ends.push(at(0.5 + Math.cos(a) * r, Math.sin(a) * r));
  }
  g.poly([ox, oy, ...ends.flat()]).fill({ color: 0xf5f5f4, alpha: 0.1 });
  for (const [ex, ey] of ends) g.moveTo(ox, oy).lineTo(ex, ey);
  g.stroke({ width: 0.018, color: 0xf5f5f4, alpha: 0.7 });
  for (const k of [0.3, 0.55, 0.8]) {
    g.moveTo(ox + (ends[0][0] - ox) * k, oy + (ends[0][1] - oy) * k);
    for (let i = 1; i < n; i++) {
      const [px, py] = ends[i - 1];
      const [qx, qy] = ends[i];
      g.quadraticCurveTo(ox + ((px + qx) / 2 - ox) * k * 0.85, oy + ((py + qy) / 2 - oy) * k * 0.85, ox + (qx - ox) * k, oy + (qy - oy) * k);
    }
  }
  g.stroke({ width: 0.012, color: 0xf5f5f4, alpha: 0.6 });
}

/** Bat guano: one soft splattered pile of dirty brown-ochre droppings, blotched chalky white, small splats
 * sprayed out round it, a faint damp stain under it. */
function guano(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y, w, h } = f;
  const [cx, cy] = [x + w / 2 + (rnd() - 0.5) * 0.2, y + h / 2 + (rnd() - 0.5) * 0.2];
  g.poly(patch(cx, cy, w * 0.42, h * 0.38, rnd, 0.15, 22)).fill({ color: 0x3a3020, alpha: 0.25 });
  // Splats sprayed out round the pile.
  for (let i = 0; i < 18; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.45 + 0.4 * rnd();
    const r = 0.025 + 0.035 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.9, r, r, rnd, 0.3, 8)).fill(rnd() < 0.3 ? 0xe6e0cc : 0x6a5430);
  }
  // The pile: a soft mound in rounded heaps, lighter where it rises.
  g.poly(patch(cx, cy, 0.42, 0.36, rnd, 0.14, 24)).fill(0x5a4628).stroke({ width: 0.02, color: 0x2e2414 });
  g.poly(patch(cx - 0.04, cy - 0.04, 0.3, 0.25, rnd, 0.16, 20)).fill(0x7a6236);
  g.poly(patch(cx - 0.08, cy - 0.08, 0.16, 0.13, rnd, 0.2, 14)).fill(0x947a46);
  // Chalky white blotches over it.
  for (let i = 0; i < 9; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.3 * Math.sqrt(rnd());
    const r = 0.03 + 0.04 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.85, r, r * 0.8, rnd, 0.35, 9)).fill({ color: 0xece6d4, alpha: 0.9 });
  }
}

/** Damp moss: a low olive mat with a broken, feathery edge of short strokes, floor showing through gaps in it,
 * darker damp patches, a fine fuzz over it, a darker wet halo on the stone round it and a few water beads. */
function caveMoss(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx, cy, 0.46, 0.42, rnd, 0.2, 22)).fill({ color: 0x1a1a10, alpha: 0.2 });
  // Patchy clumps rather than one sheet, so the floor shows between them.
  const clumps: [number, number, number][] = [];
  for (let i = 0; i < 11; i++) {
    const a = (i / 11) * Math.PI * 2 + rnd() * 0.5;
    const d = i < 3 ? 0.08 * rnd() : 0.2 + 0.1 * rnd();
    clumps.push([cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.9, 0.11 + 0.07 * rnd()]);
  }
  for (const [px, py, r] of clumps) g.poly(patch(px, py, r, r * 0.85, rnd, 0.3, 12)).fill(0x4a5a2a);
  for (const [px, py, r] of clumps) if (rnd() < 0.5) g.poly(patch(px, py, r * 0.5, r * 0.4, rnd, 0.3, 9)).fill({ color: 0x2e3a1c, alpha: 0.8 });
  // The feathery edge: short strokes fanning out from every clump's rim.
  for (const [px, py, r] of clumps) {
    for (let k = 0; k < 14; k++) {
      const a = (k / 14) * Math.PI * 2 + rnd() * 0.3;
      const [ex, ey] = [px + Math.cos(a) * r * 0.85, py + Math.sin(a) * r * 0.75];
      const L = 0.03 + 0.03 * rnd();
      g.moveTo(ex, ey).lineTo(ex + Math.cos(a + (rnd() - 0.5)) * L, ey + Math.sin(a + (rnd() - 0.5)) * L);
    }
  }
  g.stroke({ width: 0.014, color: 0x5e6e32, cap: 'round' });
  // Fuzz over the clumps.
  for (let i = 0; i < 60; i++) {
    const [px, py, r] = clumps[Math.floor(rnd() * clumps.length)];
    const a = rnd() * Math.PI * 2;
    const d = r * 0.7 * Math.sqrt(rnd());
    const b = rnd() * Math.PI;
    const [qx, qy] = [px + Math.cos(a) * d, py + Math.sin(a) * d];
    g.moveTo(qx, qy).lineTo(qx + Math.cos(b) * 0.025, qy + Math.sin(b) * 0.025);
  }
  g.stroke({ width: 0.011, color: 0x7a8a42, alpha: 0.85, cap: 'round' });
  for (let i = 0; i < 5; i++) {
    const [px, py] = [cx + (rnd() - 0.5) * 0.45, cy + (rnd() - 0.5) * 0.4];
    g.circle(px, py, 0.015).fill({ color: 0xc8e0e8, alpha: 0.8 });
    g.circle(px + light[0] * 0.005, py + light[1] * 0.005, 0.006).fill(0xffffff);
  }
}

/** Pool: still blue water with a lobed edge like the glassy pool's, no rim of rock — a darker wet margin on the
 * floor round it, pale shallows along the edge, deepening to the middle, ripple rings and a glint. */
function cavePool(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [rx, ry] = [f.w * 0.42, f.h * 0.4];
  const deep = v % 2 ? [0x1e4a62, 0x2e6a86, 0x4a8eaa, 0x7ab8cc] : [0x1e3e6a, 0x2e5a8a, 0x4a7eaa, 0x7aaad0];
  const [p3, p5, p2] = [rnd() * 6.3, rnd() * 6.3, rnd() * 6.3];
  const N = 44;
  const lobe = (k: number, j = 0, dx = 0, dy = 0): number[] => {
    const pts: number[] = [];
    for (let i = 0; i < N; i++) {
      const a = (i / N) * Math.PI * 2;
      const r = k * (1 + 0.12 * Math.sin(3 * a + p3) + 0.06 * Math.sin(5 * a + p5) + 0.05 * Math.sin(2 * a + p2) + (rnd() - 0.5) * j);
      pts.push(cx + dx + Math.cos(a) * rx * r, cy + dy + Math.sin(a) * ry * r);
    }
    return pts;
  };
  g.poly(lobe(1.1, 0.06)).fill({ color: 0x000000, alpha: 0.18 });
  g.poly(lobe(1)).fill(deep[3]);
  g.poly(lobe(0.9, 0, -light[0] * 0.02, -light[1] * 0.02)).fill(deep[2]);
  g.poly(lobe(0.7, 0, -light[0] * 0.05, -light[1] * 0.05)).fill(deep[1]);
  g.poly(lobe(0.42, 0, -light[0] * 0.08, -light[1] * 0.08)).fill(deep[0]);
  g.poly(lobe(0.97)).stroke({ width: 0.014, color: 0xd8f0f8, alpha: 0.7 });
  for (const [x, y, k] of [
    [cx + rx * 0.25, cy + ry * 0.2, 1],
    [cx - rx * 0.3, cy - ry * 0.1, 0.6],
  ]) {
    g.poly(oval(x, y, rx * 0.24 * k, ry * 0.07 * k, 0, 18)).stroke({ width: 0.014, color: 0xc8e4f0, alpha: 0.5 });
  }
  const la = Math.atan2(light[1], light[0]) + Math.PI / 2;
  g.poly(oval(cx + light[0] * rx * 0.42, cy + light[1] * ry * 0.42, rx * 0.24, 0.02, la, 14)).fill({ color: 0xffffff, alpha: 0.6 });
}

/** A heap of straw, twigs and rags (rat nest: a messy spiky heap of shredded straw with frayed striped rags
 * in it, a chewed candle and droppings round it, rats with long pink tails) or a big ring of crossed sticks
 * and fur tufts round a hollow with a bone in it (a beast's nest). */
function nestHeap(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, beast: boolean) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = beast ? 0.42 : 0.3;
  const spiky = (rr: number, n: number) => {
    const pts: number[] = [];
    for (let i = 0; i < n; i++) {
      const a = (i / n) * Math.PI * 2;
      const k = i % 2 ? 0.8 + 0.1 * rnd() : 1.05 + 0.15 * rnd();
      pts.push(cx + Math.cos(a) * rr * k, cy + Math.sin(a) * rr * k * 0.92);
    }
    return pts;
  };
  if (!beast) {
    for (let i = 0; i < 10; i++) g.poly(oval(cx + (rnd() - 0.5) * 0.8, cy + (rnd() - 0.5) * 0.8, 0.012, 0.007, rnd() * Math.PI, 6)).fill(0x2a2018);
    // A chewed candle stub at the edge.
    const [kx, ky] = [cx + r * 1.1, cy - r * 0.6];
    g.poly(oval(kx, ky, 0.07, 0.03, 0.5, 10)).fill(0xe8dcb8).stroke({ width: 0.01, color: INK });
    g.circle(kx + 0.05, ky + 0.02, 0.012).fill(0x2a2018);
  }
  g.poly(spiky(r, beast ? 30 : 26)).fill(beast ? 0x5e4a30 : 0x8a7444).stroke({ width: OUT, color: INK, join: 'round' });
  // Straws and sticks crossing, their ends sticking out of the outline.
  for (let i = 0; i < (beast ? 18 : 24); i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * (0.5 + 0.5 * rnd());
    const b = a + Math.PI / 2 + (rnd() - 0.5) * 1.2;
    const L = r * (beast ? 0.5 : 0.35) * (0.7 + 0.6 * rnd());
    const [px, py] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
    g.moveTo(px - (Math.cos(b) * L) / 2, py - (Math.sin(b) * L) / 2).lineTo(px + (Math.cos(b) * L) / 2, py + (Math.sin(b) * L) / 2);
  }
  g.stroke({ width: beast ? 0.035 : 0.014, color: beast ? 0x3a2a18 : 0x5a4a2a, cap: 'round' });
  if (beast) {
    // Fur tufts on the ring.
    for (let i = 0; i < 5; i++) {
      const a = rnd() * Math.PI * 2;
      g.poly(patch(cx + Math.cos(a) * r * 0.78, cy + Math.sin(a) * r * 0.78, 0.07, 0.05, rnd, 0.4, 10)).fill(0x8a7a6a);
    }
    // The hollow, then a bone across the rim and a shard.
    g.poly(patch(cx, cy, r * 0.5, r * 0.45, rnd, 0.15, 14)).fill(0x2a2018).stroke({ width: 0.014, color: INK });
    g.poly(patch(cx, cy, r * 0.38, r * 0.33, rnd, 0.2, 12)).fill(0x5a4a3a);
    const a = rnd() * Math.PI;
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const ends = [
      [cx - c * 0.2, cy - s * 0.2],
      [cx + c * 0.2, cy + s * 0.2],
    ];
    g.moveTo(ends[0][0], ends[0][1]).lineTo(ends[1][0], ends[1][1]).stroke({ width: 0.075, color: INK, cap: 'round' });
    for (const [ex, ey] of ends) for (const k of [1, -1]) g.circle(ex - s * k * 0.035, ey + c * k * 0.035, 0.045).fill(INK);
    g.moveTo(ends[0][0], ends[0][1]).lineTo(ends[1][0], ends[1][1]).stroke({ width: 0.045, color: 0xece2c8, cap: 'round' });
    for (const [ex, ey] of ends) for (const k of [1, -1]) g.circle(ex - s * k * 0.035, ey + c * k * 0.035, 0.03).fill(0xece2c8);
    g.poly(oval(cx + 0.1, cy + 0.14, 0.05, 0.02, 1.2, 8)).fill(0xece2c8).stroke({ width: 0.01, color: INK });
    return;
  }
  // One rag: a frayed, striped patch of cloth.
  {
    const a = rnd() * Math.PI * 2;
    const [px, py] = [cx + Math.cos(a) * r * 0.35, cy + Math.sin(a) * r * 0.35];
    const [co, si] = [Math.cos(a + 0.6), Math.sin(a + 0.6)];
    const q = (u: number, k: number): [number, number] => [px + co * u - si * k, py + si * u + co * k];
    g.poly([...q(-0.1, -0.07), ...q(0.1, -0.07), ...q(0.1, 0.07), ...q(-0.1, 0.07)]).fill(0x8a4a3a).stroke({ width: 0.01, color: INK });
    g.moveTo(...q(-0.1, 0)).lineTo(...q(0.1, 0)).stroke({ width: 0.018, color: 0xc07a62 });
    for (let k = -0.06; k <= 0.06; k += 0.03) g.moveTo(...q(0.1, k)).lineTo(...q(0.13, k + 0.01));
    g.stroke({ width: 0.008, color: 0x8a4a3a });
  }
  // Rats: bigger teardrop bodies with long pink tails curling away from the nest.
  for (let i = 0; i < 2; i++) {
    const a = i * 2.8 + rnd() * 0.6;
    const [rx, ry] = [cx + Math.cos(a) * r * 1.05, cy + Math.sin(a) * r * 1.05];
    const [c, s] = [Math.cos(a), Math.sin(a)];
    g.moveTo(rx + c * 0.08, ry + s * 0.08).quadraticCurveTo(rx + c * 0.26 - s * 0.1, ry + s * 0.26 + c * 0.1, rx + c * 0.3 + s * 0.06, ry + s * 0.3 - c * 0.06).stroke({ width: 0.024, color: 0xd09a8a, cap: 'round' });
    g.poly([rx - c * 0.13, ry - s * 0.13, rx - s * 0.065, ry + c * 0.065, rx + c * 0.1, ry + s * 0.1, rx + s * 0.065, ry - c * 0.065]).fill(0x6a5e52).stroke({ width: 0.014, color: INK, join: 'round' });
    for (const k of [1, -1]) g.circle(rx - c * 0.07 - s * k * 0.035, ry - s * 0.07 + c * k * 0.035, 0.018).fill(0xc89a8a);
    g.circle(rx - c * 0.12, ry - s * 0.12, 0.01).fill(INK);
  }
}

/** Sheet ice: a pale, glassy sheet over the floor with a soft irregular edge, frosted rim, cracks and glints. */
function iceSheet(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [rx, ry] = [f.w * 0.46, f.h * 0.44];
  const out = patch(cx, cy, rx, ry, rnd, 0.12, 26);
  g.poly(out).fill({ color: 0xe8f4fc, alpha: 0.85 }).stroke({ width: 0.025, color: 0x8ab0c8, join: 'round' });
  g.poly(patch(cx - light[0] * 0.05, cy - light[1] * 0.05, rx * 0.82, ry * 0.82, rnd, 0.1, 22)).fill({ color: 0xbcdcf0, alpha: 0.7 });
  for (let i = 0; i < 4; i++) {
    let [px, py] = [cx + (rnd() - 0.5) * rx, cy + (rnd() - 0.5) * ry];
    const a = rnd() * Math.PI * 2;
    g.moveTo(px, py);
    for (let k = 0; k < 4; k++) {
      px += Math.cos(a + (rnd() - 0.5)) * 0.15;
      py += Math.sin(a + (rnd() - 0.5)) * 0.15;
      g.lineTo(px, py);
    }
  }
  g.stroke({ width: 0.016, color: 0x6a9ab8, alpha: 0.85 });
  for (let i = 0; i < 4; i++) {
    const [px, py] = [cx + (rnd() - 0.5) * rx * 1.2, cy + (rnd() - 0.5) * ry * 1.2];
    g.moveTo(px - 0.07, py + 0.03).lineTo(px + 0.07, py - 0.03).stroke({ width: 0.025, color: 0xffffff, alpha: 0.75, cap: 'round' });
  }
}

const OBSIDIAN = [0x121018, 0x2a2634, 0x4a4560, 0x8a84a8];
const BASALT = [0x1c1a1c, 0x2e2b2e, 0x46424a, 0x66606a];

/** One flat glassy chip lying on the floor round (x, y) along `a`: a sharp irregular blade split by a ridge, its
 * half toward the light catching a pale sheen, curved ripples (glass breaks in shells), a bright cutting edge. */
function glassChip(g: Graphics, x: number, y: number, a: number, len: number, w: number, light: Light, rnd: () => number) {
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const p = (u: number, k: number): [number, number] => [x + c * u - s * k, y + s * u + c * k];
  const T = p(len, (rnd() - 0.5) * w * 0.4);
  const L = p(len * (0.05 + 0.2 * rnd()), w * (0.8 + 0.3 * rnd()));
  const Bl = p(-len * (0.45 + 0.2 * rnd()), w * (0.3 + 0.3 * rnd()));
  const Br = p(-len * (0.3 + 0.3 * rnd()), -w * (0.4 + 0.3 * rnd()));
  const R = p(len * (0.15 + 0.2 * rnd()), -w * (0.7 + 0.3 * rnd()));
  const M = p(-len * 0.1, (rnd() - 0.5) * w * 0.3);
  const lit = -s * light[0] + c * light[1] > 0;
  const sd = lit ? -1 : 1;
  castShadow(g, [...T, ...L, ...Bl, ...Br, ...R], light, 0.06);
  g.poly([...T, ...L, ...Bl, ...M]).fill(lit ? OBSIDIAN[2] : OBSIDIAN[0]);
  g.poly([...T, ...M, ...Br, ...R]).fill(lit ? OBSIDIAN[0] : OBSIDIAN[2]);
  g.poly([...M, ...Bl, ...Br]).fill(OBSIDIAN[1]);
  // Shell ripples on the dark half, the ridge, the outline.
  for (const k of [0.35, 0.6]) {
    g.moveTo(...p(len * k, sd * w * 0.15)).quadraticCurveTo(...p(len * (k - 0.05), sd * w * 0.55), ...p(len * (k - 0.25), sd * w * 0.65));
  }
  g.stroke({ width: 0.01, color: OBSIDIAN[2], alpha: 0.8 });
  g.moveTo(...T).lineTo(...M).lineTo(...Bl).moveTo(...M).lineTo(...Br).stroke({ width: 0.01, color: INK, alpha: 0.6 });
  g.poly([...T, ...L, ...Bl, ...Br, ...R]).stroke({ width: 0.022, color: INK, join: 'miter' });
  g.moveTo(...(lit ? L : R)).lineTo(...T).stroke({ width: 0.014, color: OBSIDIAN[3], cap: 'round' });
  g.moveTo(...p(len * 0.3, -sd * w * 0.3)).lineTo(...p(len * 0.7, -sd * w * 0.12)).stroke({ width: 0.012, color: 0xffffff, alpha: 0.8, cap: 'round' });
}

/** Obsidian shards: a scatter of broken black volcanic glass, a few big blades and small chips, glossy and sharp. */
function obsidianShards(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < 9; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.2 + 0.24 * rnd();
    glassChip(g, cx + Math.cos(a) * d, cy + Math.sin(a) * d, rnd() * Math.PI * 2, 0.04 + 0.03 * rnd(), 0.025, light, rnd);
  }
  const big: [number, number, number, number, number][] = [
    [-0.16, -0.12, ph, 0.3, 0.11],
    [0.18, 0.06, ph + 2.2, 0.26, 0.1],
    [-0.06, 0.22, ph + 4.1, 0.22, 0.09],
    [0.16, -0.22, ph + 1.1, 0.15, 0.065],
    [-0.24, 0.1, ph + 3, 0.12, 0.05],
  ];
  for (const [dx, dy, a, len, w] of big) glassChip(g, cx + dx, cy + dy, a, len, w, light, rnd);
}

/** Basalt column: a clump of tight six-sided columns seen from above, standing to different heights: each a
 * flat hexagonal top with a lit rim, its side showing away from the light, the tallest throwing the longest
 * shadow; cracks across the tops. */
function basaltColumns(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.13;
  const sp = r * Math.sqrt(3) * 1.02;
  const ph = rnd() * 0.5;
  const gap = Math.floor(rnd() * 6);
  const cols: [number, number, number][] = [[cx, cy, 1]];
  for (let i = 0; i < 6; i++) {
    if (i === gap) continue;
    const a = ph + Math.PI / 6 + (i * Math.PI) / 3;
    cols.push([cx + Math.cos(a) * sp, cy + Math.sin(a) * sp, 0.3 + 0.6 * rnd()]);
  }
  const hex = (x: number, y: number, k: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 6; i++) pts.push(x + Math.cos(ph + (i * Math.PI) / 3) * r * k, y + Math.sin(ph + (i * Math.PI) / 3) * r * k);
    return pts;
  };
  for (const [x, y, h] of cols) castShadow(g, hex(x, y, 1), light, 0.12 + h * 0.25);
  cols.sort((p, q) => p[2] - q[2]);
  for (const [x, y, h] of cols) {
    const off = 0.03 + h * 0.08;
    const top = hex(x, y, 1);
    const low = hex(x - light[0] * off, y - light[1] * off, 1);
    // The side: the top swept away from the light, dark, its edges ruled.
    g.poly(low).fill(BASALT[0]).stroke({ width: OUT, color: INK, join: 'round' });
    for (let i = 0; i < 6; i++) {
      const j = (i + 1) % 6;
      g.poly([top[i * 2], top[i * 2 + 1], top[j * 2], top[j * 2 + 1], low[j * 2], low[j * 2 + 1], low[i * 2], low[i * 2 + 1]]).fill(BASALT[0]);
    }
    for (let i = 0; i < 6; i++) g.moveTo(top[i * 2], top[i * 2 + 1]).lineTo(low[i * 2], low[i * 2 + 1]);
    g.stroke({ width: 0.012, color: INK, alpha: 0.8 });
    const tone = shade(BASALT[2], 0.7 + h * 0.6);
    g.poly(top).fill(tone).stroke({ width: 0.022, color: INK, join: 'round' });
    g.poly(hex(x + light[0] * 0.012, y + light[1] * 0.012, 0.72)).fill(shade(tone, 1.12));
    // The rim toward the light catches it.
    for (let i = 0; i < 6; i++) {
      const j = (i + 1) % 6;
      const [mx, my] = [(top[i * 2] + top[j * 2]) / 2 - x, (top[i * 2 + 1] + top[j * 2 + 1]) / 2 - y];
      if (mx * light[0] + my * light[1] > 0.02) g.moveTo(top[i * 2], top[i * 2 + 1]).lineTo(top[j * 2], top[j * 2 + 1]);
    }
    g.stroke({ width: 0.016, color: BASALT[3], cap: 'round' });
    const a = rnd() * Math.PI * 2;
    g.moveTo(x + Math.cos(a) * r * 0.8, y + Math.sin(a) * r * 0.8).lineTo(x + (rnd() - 0.5) * r * 0.4, y + (rnd() - 0.5) * r * 0.4).stroke({ width: 0.01, color: INK, alpha: 0.7 });
  }
}

/** Steam vent: a dark wet hole in a low cone of rock, white salt crust round its lip, the floor damp round it, a
 * big plume of white steam rolling off to one side over the rim. */
function steamVent(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  // The damp floor: a darker glossy patch, droplets.
  g.poly(patch(cx, cy, 0.44, 0.42, rnd, 0.12, 20)).fill({ color: 0x000000, alpha: 0.2 });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la - 0.7) * 0.38, cy + Math.sin(la - 0.7) * 0.38).arc(cx, cy, 0.38, la - 0.7, la + 0.7).stroke({ width: 0.02, color: 0xffffff, alpha: 0.15, cap: 'round' });
  for (let i = 0; i < 10; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.32 + 0.12 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.01 + 0.008 * rnd()).fill({ color: 0xd8e4ec, alpha: 0.6 });
  }
  g.poly(patch(cx, cy, 0.29, 0.27, rnd, 0.14, 16)).fill(BASALT[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + light[0] * 0.03, cy + light[1] * 0.03, 0.22, 0.2, rnd, 0.14, 14)).fill(BASALT[2]);
  // Salt crust on the lip, then the hole: dark, a wet sheen on its far edge.
  g.poly(patch(cx, cy, 0.16, 0.14, rnd, 0.22, 14)).fill(0xe8e4d8).stroke({ width: 0.014, color: 0x8a8478, join: 'round' });
  for (let i = 0; i < 8; i++) {
    const a = rnd() * Math.PI * 2;
    g.circle(cx + Math.cos(a) * 0.17, cy + Math.sin(a) * 0.15, 0.012 + 0.01 * rnd()).fill(0xf4f0e6);
  }
  const hole = patch(cx, cy, 0.085, 0.07, rnd, 0.2, 12);
  g.poly(hole).fill(0x0a0a0c).stroke({ width: 0.016, color: INK, join: 'round' });
  g.moveTo(cx - light[0] * 0.06 - light[1] * 0.04, cy - light[1] * 0.06 + light[0] * 0.04)
    .lineTo(cx - light[0] * 0.06 + light[1] * 0.04, cy - light[1] * 0.06 - light[0] * 0.04)
    .stroke({ width: 0.012, color: 0x8aa4b4, alpha: 0.8, cap: 'round' });
  // The plume: rolling clusters of puffs, growing and fading as they drift, the first over the hole and rim.
  const drift = la + Math.PI + (rnd() - 0.5) * 0.8;
  const [dc, ds] = [Math.cos(drift), Math.sin(drift)];
  for (let i = 0; i < 5; i++) {
    const k = i / 4;
    const d = 0.08 + k * 0.36;
    const bend = Math.sin(k * 3.5 + 1) * 0.07;
    const [px, py] = [cx + dc * d - ds * bend, cy + ds * d + dc * bend];
    const rr = 0.07 + k * 0.08;
    const a = 0.6 - k * 0.3;
    for (const [ox, oy, kk] of [
      [-0.6, 0.9, 0.7],
      [0.5, -0.9, 0.65],
      [0, 0, 1],
    ]) {
      const [qx, qy] = [px + (dc * ox - ds * oy) * rr, py + (ds * ox + dc * oy) * rr];
      g.circle(qx, qy, rr * kk).fill({ color: 0xe8ecf0, alpha: a });
    }
    g.circle(px + light[0] * rr * 0.3, py + light[1] * rr * 0.3, rr * 0.55).fill({ color: 0xffffff, alpha: a * 0.8 });
  }
}

/** Sulfur crust: one flat ragged patch of yellow mineral crust grown round a dark vent crack, needle crystals
 * in lemon and ochre on it, rust-orange stains at its edge, a faint haze over the crack. */
function sulfurCrust(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.3 + 0.08 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.05 + 0.03 * rnd(), 0.04 + 0.02 * rnd(), rnd, 0.3, 9)).fill({ color: 0xc06a20, alpha: 0.45 });
  }
  g.poly(patch(cx, cy, 0.36, 0.33, rnd, 0.12, 34)).fill(0xb8982a).stroke({ width: 0.016, color: 0x6a5418, join: 'round' });
  g.poly(patch(cx + light[0] * 0.02, cy + light[1] * 0.02, 0.29, 0.27, rnd, 0.12, 28)).fill(0xd8c040);
  for (let i = 0; i < 3; i++) {
    const a = Math.atan2(light[1], light[0]) + (rnd() - 0.5) * 2.4;
    const d = 0.1 + 0.1 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.08 + 0.04 * rnd(), 0.06 + 0.03 * rnd(), rnd, 0.3, 10)).fill(0xf0e070);
  }
  // The vent crack: a dark slit, a raised ring of crust round it.
  const ca = rnd() * Math.PI;
  const [kc, ks] = [Math.cos(ca), Math.sin(ca)];
  const q = (u: number, k: number): [number, number] => [cx + kc * u - ks * k, cy + ks * u + kc * k];
  g.poly([...q(-0.13, 0), ...q(-0.05, 0.035), ...q(0.03, 0.02), ...q(0.12, 0.03), ...q(0.06, -0.02), ...q(-0.04, -0.03)]).fill(0xf8f0b0).stroke({ width: 0.012, color: 0x8a7420, join: 'round' });
  g.poly([...q(-0.1, 0), ...q(-0.04, 0.018), ...q(0.03, 0.008), ...q(0.09, 0.014), ...q(0.05, -0.01), ...q(-0.03, -0.014)]).fill(0x1a1608);
  // Needle crystals: thin pale blades with a dark side, densest round the crack.
  for (let i = 0; i < 46; i++) {
    const a = rnd() * Math.PI * 2;
    const d = i < 16 ? 0.07 + 0.06 * rnd() : 0.08 + 0.22 * Math.sqrt(rnd());
    const [x, y] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.94];
    const b = i < 16 ? a + (rnd() - 0.5) * 0.6 : rnd() * Math.PI * 2;
    const L = 0.035 + 0.035 * rnd();
    const [ex, ey] = [x + Math.cos(b) * L, y + Math.sin(b) * L];
    g.moveTo(x - light[0] * 0.006, y - light[1] * 0.006).lineTo(ex - light[0] * 0.006, ey - light[1] * 0.006).stroke({ width: 0.012, color: 0x8a7420, cap: 'round' });
    g.moveTo(x, y).lineTo(ex, ey).stroke({ width: 0.009, color: 0xfff6a8, cap: 'round' });
  }
  g.circle(cx, cy, 0.08).fill({ color: 0xf4f8e0, alpha: 0.18 });
  g.circle(cx - light[0] * 0.08, cy - light[1] * 0.08, 0.11).fill({ color: 0xf4f8e0, alpha: 0.12 });
}

/** Scorched remains: someone burnt where they fell — a black soot burst on the floor round the body, grey ash
 * over it, charred bones (a blackened skull, ribs, long bones), a few embers still glowing. */
function scorchedRemains(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const long = f.h >= f.w;
  // A body about 5.5 ft long (a square is 5 ft): B scales the positions along it.
  const B = 1.4;
  // Along the body (head end first) and across it.
  const p = (u: number, k: number): [number, number] => (long ? [cx + k, cy + u] : [cx + u, cy + k]);
  const burst: number[] = [];
  const n = 28;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const spike = i % 2 ? 0.75 + 0.2 * rnd() : 1 + 0.3 * rnd();
    burst.push(...p(Math.sin(a) * B * 0.5 * spike, Math.cos(a) * 0.4 * spike));
  }
  g.poly(burst).fill({ color: 0x0e0a08, alpha: 0.5 });
  g.poly(patch(cx, cy, long ? 0.3 : B * 0.44, long ? B * 0.44 : 0.3, rnd, 0.18, 22)).fill({ color: 0x0e0a08, alpha: 0.75 });
  // Grey ash in drifts over it.
  for (let i = 0; i < 6; i++) {
    const [x, y] = p((rnd() - 0.5) * B * 0.6, (rnd() - 0.5) * 0.4);
    g.poly(patch(x, y, 0.07 + 0.05 * rnd(), 0.05 + 0.04 * rnd(), rnd, 0.3, 10)).fill({ color: 0x8a8480, alpha: 0.55 });
  }
  const bone = 0x6a5e50;
  const ash = 0xd0c8b8;
  const limb = (a: [number, number], b: [number, number], w: number) => {
    g.moveTo(...a).lineTo(...b).stroke({ width: w, color: INK, cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: w - 0.025, color: bone, cap: 'round' });
    g.circle(...a, w * 0.75).fill(bone).stroke({ width: 0.012, color: INK });
    g.circle(...b, w * 0.75).fill(bone).stroke({ width: 0.012, color: INK });
    g.moveTo(...a).lineTo(...b).stroke({ width: 0.012, color: ash, alpha: 0.7, cap: 'round' });
  };
  limb(p(B * 0.12, 0.06), p(B * 0.36, 0.1), 0.045);
  limb(p(B * 0.1, -0.08), p(B * 0.32, -0.14), 0.042);
  limb(p(-B * 0.18, 0.2), p(-B * 0.02, 0.24), 0.035);
  // Ribs: curved bars either side of a spine.
  g.moveTo(...p(-B * 0.24, 0)).lineTo(...p(B * 0.06, 0)).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.moveTo(...p(-B * 0.24, 0)).lineTo(...p(B * 0.06, 0)).stroke({ width: 0.02, color: bone, cap: 'round' });
  for (let i = 0; i < 4; i++) {
    const u = -B * 0.2 + i * B * 0.055;
    for (const sd of [-1, 1]) {
      for (const [w, c] of [
        [0.032, INK],
        [0.015, bone],
      ]) {
        g.moveTo(...p(u, sd * 0.02)).quadraticCurveTo(...p(u - 0.02, sd * 0.14), ...p(u + 0.03, sd * 0.17));
        g.stroke({ width: w, color: c, cap: 'round' });
      }
    }
  }
  // The skull at the head end: blackened, ash-grey on top, dark sockets.
  const [hx, hy] = p(-B * 0.36, 0.02);
  g.poly(oval(hx, hy, 0.085, 0.095, 0, 16)).fill(bone).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(oval(hx + light[0] * 0.025, hy + light[1] * 0.025, 0.05, 0.05, 0, 12)).fill({ color: ash, alpha: 0.8 });
  g.circle(...p(-B * 0.36 + 0.025, -0.025), 0.022).fill(0x080604);
  g.circle(...p(-B * 0.36 + 0.025, 0.055), 0.022).fill(0x080604);
  g.circle(...p(-B * 0.36 + 0.06, 0.015), 0.01).fill(0x080604);
  // Embers.
  for (let i = 0; i < 7; i++) {
    const [x, y] = p((rnd() - 0.5) * B * 0.7, (rnd() - 0.5) * 0.5);
    g.circle(x, y, 0.03).fill({ color: 0xf08030, alpha: 0.25 });
    g.circle(x, y, 0.012 + 0.008 * rnd()).fill(i % 2 ? 0xf8a040 : 0xe05818);
  }
}

/** Cooled glassy pool: a flat sheet of black volcanic glass where lava spread and set, lobed like a spill: a
 * lumpy basalt rim merging into the floor, ropy folds frozen near its edge, angular cracks (one still faintly
 * red), crisp mirror glints, obsidian chips at the rim. */
function glassPool(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [rx, ry] = [f.w * 0.42, f.h * 0.4];
  const [p3, p5, p2] = [rnd() * 6.3, rnd() * 6.3, rnd() * 6.3];
  const N = 44;
  const lobe = (k: number, j = 0): number[] => {
    const pts: number[] = [];
    for (let i = 0; i < N; i++) {
      const a = (i / N) * Math.PI * 2;
      const r = k * (1 + 0.12 * Math.sin(3 * a + p3) + 0.06 * Math.sin(5 * a + p5) + 0.05 * Math.sin(2 * a + p2) + (rnd() - 0.5) * j);
      pts.push(cx + Math.cos(a) * rx * r, cy + Math.sin(a) * ry * r);
    }
    return pts;
  };
  g.poly(lobe(1.12, 0.1)).fill({ color: BASALT[1], alpha: 0.45 });
  g.poly(lobe(1.02, 0.12)).fill(BASALT[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(lobe(0.98, 0.1)).fill(BASALT[2]);
  const glass = lobe(0.84, 0.03);
  g.poly(glass).fill(0x0c0a10).stroke({ width: LINE, color: INK, join: 'round' });
  // Ropy folds: partial rings following the edge, bunched near it.
  for (const k of [0.76, 0.68, 0.6]) {
    const ring = lobe(k, 0.02);
    const start = Math.floor(rnd() * N);
    const len = Math.floor(N * (0.3 + 0.25 * rnd()));
    for (const [w, c, a, o] of [
      [0.026, OBSIDIAN[1], 0.9, 0],
      [0.01, OBSIDIAN[2], 0.7, 0.012],
    ]) {
      for (let s = 0; s < 2; s++) {
        const i0 = (start + s * Math.floor(N / 2)) % N;
        g.moveTo(ring[i0 * 2] + light[0] * o, ring[i0 * 2 + 1] + light[1] * o);
        for (let i = 1; i < len; i++) {
          const ii = (i0 + i) % N;
          const wob = Math.sin(i * 1.9) * 0.012;
          g.lineTo(ring[ii * 2] + light[0] * o + wob, ring[ii * 2 + 1] + light[1] * o + wob);
        }
        g.stroke({ width: w, color: c, alpha: a, cap: 'round', join: 'round' });
      }
    }
  }
  // Angular cracks; the first glows dull red deep down.
  const away = Math.atan2(-light[1], -light[0]);
  for (let c = 0; c < 3; c++) {
    const a = away + (c - 1) * 1.4 + (rnd() - 0.5) * 0.5;
    let [x, y] = [cx + Math.cos(a) * rx * 0.2, cy + Math.sin(a) * ry * 0.2];
    const pts: number[] = [x, y];
    for (let k = 0; k < 4; k++) {
      const b = a + (rnd() - 0.5) * 1.2;
      x += Math.cos(b) * rx * 0.13;
      y += Math.sin(b) * ry * 0.13;
      pts.push(x, y);
    }
    const line = () => {
      g.moveTo(pts[0], pts[1]);
      for (let i = 2; i < pts.length; i += 2) g.lineTo(pts[i], pts[i + 1]);
    };
    if (c === 0) {
      line();
      g.stroke({ width: 0.05, color: 0xa02a10, alpha: 0.35, cap: 'round', join: 'miter' });
      line();
      g.stroke({ width: 0.022, color: 0xc03a14, alpha: 0.7, cap: 'round', join: 'miter' });
    }
    line();
    g.stroke({ width: 0.012, color: c === 0 ? 0x3a0a04 : OBSIDIAN[3], alpha: c === 0 ? 1 : 0.55, cap: 'round', join: 'miter' });
  }
  // Mirror glints: two crisp streaks toward the light and a star.
  const la = Math.atan2(light[1], light[0]) + Math.PI / 2;
  g.poly(oval(cx + light[0] * rx * 0.42, cy + light[1] * ry * 0.42, rx * 0.26, 0.022, la, 14)).fill({ color: 0xffffff, alpha: 0.85 });
  g.poly(oval(cx + light[0] * rx * 0.3 + Math.cos(la) * 0.12, cy + light[1] * ry * 0.3 + Math.sin(la) * 0.12, rx * 0.12, 0.014, la, 10)).fill({ color: 0xffffff, alpha: 0.7 });
  const [sx, sy] = [cx - light[0] * rx * 0.35 + Math.cos(la) * 0.1, cy - light[1] * ry * 0.35 + Math.sin(la) * 0.1];
  g.poly([sx, sy - 0.05, sx + 0.012, sy, sx, sy + 0.05, sx - 0.012, sy]).fill({ color: 0xffffff, alpha: 0.8 });
  g.poly([sx - 0.05, sy, sx, sy - 0.012, sx + 0.05, sy, sx, sy + 0.012]).fill({ color: 0xffffff, alpha: 0.8 });
  // Lumps on the rim, obsidian chips by it.
  for (let i = 0; i < 7; i++) {
    const a = ((i + rnd() * 0.7) / 7) * Math.PI * 2;
    rock(g, cx + Math.cos(a) * rx * 1.0, cy + Math.sin(a) * ry * 1.0, 0.06 + 0.05 * rnd(), BASALT, light, rnd, 6);
  }
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    glassChip(g, cx + Math.cos(a) * rx * 1.12, cy + Math.sin(a) * ry * 1.12, rnd() * Math.PI * 2, 0.06 + 0.04 * rnd(), 0.03, light, rnd);
  }
}

const BONE = [0x8a7e64, 0xd8ccb0, 0xf2eada];

/** A bone from a to b, `w` thick: an inked shaft, knobbly ends (two lumps across each), a lit streak. */
function bone(g: Graphics, a: [number, number], b: [number, number], w: number, light: Light) {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const l = Math.hypot(dx, dy) || 1;
  const [nx, ny] = [-dy / l, dx / l];
  const knobs = (p: [number, number]) => {
    for (const s of [-1, 1]) g.circle(p[0] + nx * w * 0.45 * s, p[1] + ny * w * 0.45 * s, w * 0.62);
  };
  knobs(a);
  knobs(b);
  g.fill(BONE[1]).stroke({ width: 0.016, color: INK });
  g.moveTo(...a).lineTo(...b).stroke({ width: w + 0.03, color: INK, cap: 'round' });
  knobs(a);
  knobs(b);
  g.fill(BONE[1]);
  g.moveTo(...a).lineTo(...b).stroke({ width: w, color: BONE[1], cap: 'round' });
  const o = (nx * light[0] + ny * light[1] > 0 ? 1 : -1) * w * 0.22;
  g.moveTo(a[0] + nx * o + dx * 0.15, a[1] + ny * o + dy * 0.15).lineTo(b[0] + nx * o - dx * 0.15, b[1] + ny * o - dy * 0.15).stroke({ width: w * 0.3, color: BONE[2], cap: 'round' });
}

/** A skull from above at (x, y), crown toward `dir` (radians): the round cranium lit on one side, brow and
 * cheekbones narrowing to the jaw, two dark sockets and a nose hole toward the face end. */
function skullTop(g: Graphics, x: number, y: number, dir: number, s: number, light: Light) {
  const [c, sn] = [Math.cos(dir), Math.sin(dir)];
  const p = (u: number, k: number): [number, number] => [x + c * u - sn * k, y + sn * u + c * k];
  const shape = [...p(s, 0), ...p(s * 0.75, s * 0.68), ...p(0, s * 0.82), ...p(-s * 0.6, s * 0.6), ...p(-s * 1.05, s * 0.32), ...p(-s * 1.1, -s * 0.32), ...p(-s * 0.6, -s * 0.6), ...p(0, -s * 0.82), ...p(s * 0.75, -s * 0.68)];
  g.poly(shape).fill(BONE[1]).stroke({ width: 0.02, color: INK, join: 'round' });
  g.circle(x + light[0] * s * 0.25 + c * s * 0.25, y + light[1] * s * 0.25 + sn * s * 0.25, s * 0.42).fill({ color: BONE[2], alpha: 0.9 });
  for (const k of [-1, 1]) g.poly(oval(...p(-s * 0.55, k * s * 0.34), s * 0.22, s * 0.2, dir, 10)).fill(0x1a1410);
  g.poly([...p(-s * 0.8, 0), ...p(-s * 0.95, s * 0.1), ...p(-s * 0.95, -s * 0.1)]).fill(0x1a1410);
}

/** Scattered bones: long bones lying every way, a jaw, ribs, a few knuckle bits; no skull (that's the
 * remains and the pile). */
function scatteredBones(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  for (let i = 0; i < 7; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.1 + 0.3 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.02 + 0.012 * rnd()).fill(BONE[1]).stroke({ width: 0.01, color: INK });
  }
  // Ribs: curved bars.
  for (let i = 0; i < 3; i++) {
    const [x, y] = [cx - 0.22 + i * 0.07, cy + 0.18 + i * 0.02];
    for (const [w, col] of [
      [0.04, INK],
      [0.022, BONE[1]],
    ] as const) {
      g.moveTo(x, y).quadraticCurveTo(x + 0.12, y - 0.1, x + 0.2, y + 0.02).stroke({ width: w, color: col, cap: 'round' });
    }
  }
  // A jawbone: a U.
  const [jx, jy] = [cx + 0.22, cy + 0.2];
  for (const [w, col] of [
    [0.05, INK],
    [0.03, BONE[1]],
  ] as const) {
    g.moveTo(jx - 0.08, jy - 0.06).quadraticCurveTo(jx, jy + 0.14, jx + 0.08, jy - 0.06).stroke({ width: w, color: col, cap: 'round' });
  }
  const ph = rnd() * Math.PI;
  const longs: [number, number, number, number, number][] = [
    [-0.04, -0.2, ph, 0.13, 0.05],
    [0.18, 0.0, ph + 1.4, 0.12, 0.045],
    [-0.2, 0.04, ph + 2.3, 0.09, 0.04],
    [0.04, 0.06, ph + 0.4, 0.08, 0.035],
  ];
  for (const [dx, dy, a, l, w] of longs) {
    const [x, y] = [cx + dx, cy + dy];
    castShadow(g, [x - Math.cos(a) * l, y - Math.sin(a) * l, x + Math.cos(a) * l, y + Math.sin(a) * l, x, y + 0.02], light, 0.04);
    bone(g, [x - Math.cos(a) * l, y - Math.sin(a) * l], [x + Math.cos(a) * l, y + Math.sin(a) * l], w, light);
  }
}

/** Adventurer's remains: someone who died where they fell, life-sized (about 5.5 ft, a square is 5): a sprawled
 * skeleton — one arm flung out, the skull rolled aside, a leg bent, a bone dragged off — on a ragged cloak; a
 * rusty sword dropped by the flung hand, an open helm tipped over by the head, a satchel and a pouch spilling
 * coins. */
function skeletonRemains(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { len, at } = frame(f);
  const o = (len - 1.15) / 2;
  const P = (u: number, s: number) => at(o + u, 0.5 + s);
  const flat = (pts: [number, number][]) => pts.flatMap(([u, s]) => P(u + (rnd() - 0.5) * 0.03, s + (rnd() - 0.5) * 0.03));
  // The cloak: ragged, spread to one side.
  const cloak = flat([[0.1, -0.2], [0.2, -0.26], [0.3, -0.2], [0.42, -0.3], [0.55, -0.24], [0.7, -0.34], [0.78, -0.22], [0.86, -0.12], [0.8, 0.02], [0.72, 0.16], [0.6, 0.22], [0.48, 0.18], [0.36, 0.26], [0.22, 0.2], [0.12, 0.14]]);
  g.poly(cloak).fill({ color: 0x3e3226, alpha: 0.85 }).stroke({ width: 0.014, color: 0x1e1812, join: 'miter' });
  g.moveTo(...P(0.3, -0.18)).lineTo(...P(0.6, -0.26)).moveTo(...P(0.5, 0.15)).lineTo(...P(0.7, 0.05)).stroke({ width: 0.01, color: 0x5a4a36 });
  // Satchel on its strap, and a pouch with coins spilled from it.
  const [bx, by] = P(0.62, -0.42);
  g.moveTo(...P(0.3, -0.1)).quadraticCurveTo(...P(0.45, -0.42), bx, by).stroke({ width: 0.018, color: 0x5a3a20 });
  const bag = quad([bx - 0.1, by - 0.08, bx + 0.1, by + 0.08], rnd, 0.01);
  g.poly(bag).fill(0x7a5030).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(quad([bx - 0.1, by - 0.08, bx + 0.1, by + 0.0], rnd, 0.008)).fill(0x8e6038).stroke({ width: 0.012, color: INK });
  g.circle(bx, by + 0.0, 0.015).fill(BRASS);
  const [px, py] = P(0.98, 0.3);
  g.circle(px, py, 0.05).fill(0x6a4a2a).stroke({ width: 0.016, color: INK });
  g.circle(px + 0.02, py - 0.03, 0.018).fill(0x4a3220);
  for (let i = 0; i < 6; i++) {
    const [x, y] = P(1.02 + rnd() * 0.12, 0.2 + rnd() * 0.15);
    g.circle(x, y, 0.02).fill(GILT[1]).stroke({ width: 0.008, color: GILT[0] });
  }
  // Legs: one straight, one bent at the knee; the pelvis.
  bone(g, P(0.68, -0.07), P(0.92, -0.09), 0.04, light);
  bone(g, P(0.93, -0.09), P(1.12, -0.07), 0.034, light);
  g.poly(oval(...P(1.15, -0.07), 0.035, 0.025, 0, 8)).fill(BONE[1]).stroke({ width: 0.01, color: INK });
  bone(g, P(0.68, 0.07), P(0.86, 0.2), 0.04, light);
  bone(g, P(0.87, 0.21), P(1.06, 0.12), 0.034, light);
  g.poly(oval(...P(1.09, 0.1), 0.035, 0.025, 0.6, 8)).fill(BONE[1]).stroke({ width: 0.01, color: INK });
  const pel = [...P(0.58, -0.04), ...P(0.6, -0.13), ...P(0.68, -0.12), ...P(0.7, 0), ...P(0.68, 0.12), ...P(0.6, 0.13), ...P(0.58, 0.04)];
  g.poly(pel).fill(BONE[1]).stroke({ width: 0.016, color: INK, join: 'round' });
  for (const k of [-1, 1]) g.circle(...P(0.645, k * 0.07), 0.02).fill(0x2a2018);
  // Spine and ribcage, a little twisted.
  g.moveTo(...P(0.12, 0.02)).quadraticCurveTo(...P(0.35, -0.03), ...P(0.6, 0)).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.moveTo(...P(0.12, 0.02)).quadraticCurveTo(...P(0.35, -0.03), ...P(0.6, 0)).stroke({ width: 0.022, color: BONE[1], cap: 'round' });
  for (let i = 0; i < 5; i++) {
    const u = 0.2 + i * 0.05;
    for (const k of [-1, 1]) {
      for (const [w, col] of [
        [0.03, INK],
        [0.015, BONE[1]],
      ] as const) {
        g.moveTo(...P(u, k * 0.015 - 0.015)).quadraticCurveTo(...P(u - 0.02, k * 0.14 - 0.015), ...P(u + 0.05, k * 0.13 - 0.015)).stroke({ width: w, color: col, cap: 'round' });
      }
    }
  }
  // Arms: one by the side, one flung out above the head; the sword by that hand.
  bone(g, P(0.17, -0.15), P(0.4, -0.19), 0.034, light);
  bone(g, P(0.41, -0.19), P(0.58, -0.22), 0.028, light);
  bone(g, P(0.15, 0.15), P(0.0, 0.33), 0.034, light);
  bone(g, P(-0.01, 0.34), P(-0.14, 0.42), 0.028, light);
  for (let j = 0; j < 3; j++) g.circle(...P(-0.18 - j * 0.01, 0.42 + j * 0.025), 0.012).fill(BONE[1]).stroke({ width: 0.008, color: INK });
  const [s0, s1] = [P(-0.3, 0.3), P(0.45, 0.42)];
  g.moveTo(...s0).lineTo(...s1).stroke({ width: 0.075, color: INK, cap: 'butt' });
  g.moveTo(...s0).lineTo(...s1).stroke({ width: 0.05, color: 0x8a7a6a, cap: 'butt' });
  g.moveTo(...P(-0.15, 0.32)).lineTo(...P(0.42, 0.415)).stroke({ width: 0.012, color: 0xb8aea0 });
  for (let i = 0; i < 5; i++) g.circle(...P(-0.1 + rnd() * 0.5, 0.33 + (rnd() * 0.08)), 0.014).fill(0x8a4a22);
  g.moveTo(...P(-0.29, 0.21)).lineTo(...P(-0.32, 0.39)).stroke({ width: 0.045, color: INK, cap: 'round' });
  g.moveTo(...P(-0.29, 0.21)).lineTo(...P(-0.32, 0.39)).stroke({ width: 0.025, color: 0x6a5a40, cap: 'round' });
  // A thigh bone dragged off.
  bone(g, P(0.95, -0.38), P(1.15, -0.28), 0.036, light);
  // The skull, rolled aside and turned; the helm tipped over beside it.
  const [hx, hy] = P(0.02, -0.08);
  const [ux, uy] = at(0, 0);
  const [vx, vy] = at(1, 0);
  skullTop(g, hx, hy, Math.atan2(uy - vy, ux - vx) + 0.9, 0.085, light);
  const [mx, my] = P(-0.08, -0.34);
  const ma = Math.atan2(my - hy, mx - hx);
  g.circle(mx, my, 0.11).fill(IRON[1]).stroke({ width: OUT, color: INK });
  g.circle(mx + light[0] * 0.03, my + light[1] * 0.03, 0.05).fill(IRON[2]);
  // Its open side: a dark crescent rimmed in iron, a nasal bar across it.
  const [ox, oy] = [mx + Math.cos(ma) * 0.05, my + Math.sin(ma) * 0.05];
  g.poly(oval(ox, oy, 0.05, 0.09, ma, 14)).fill(0x16140f).stroke({ width: 0.018, color: IRON[0] });
  g.moveTo(ox - Math.cos(ma) * 0.03, oy - Math.sin(ma) * 0.03).lineTo(ox + Math.cos(ma) * 0.05, oy + Math.sin(ma) * 0.05).stroke({ width: 0.02, color: IRON[2], cap: 'round' });
}

/** Skull pile, from a high angle so the faces show (like the headstones): skulls heaped in rows, fewer to the
 * top, each a pale cranium with dark eye sockets, a nose hole and a row of teeth; a few bones under the heap. */
function skullPile(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt } = screenAxes(light);
  g.poly(patch(cx, cy, 0.42, 0.42, rnd, 0.1, 18)).fill({ color: 0x000000, alpha: 0.28 });
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    const [x, y] = [cx + Math.cos(a) * 0.32, cy + Math.sin(a) * 0.32];
    bone(g, [x - Math.cos(a + 1.6) * 0.1, y - Math.sin(a + 1.6) * 0.1], [x + Math.cos(a + 1.6) * 0.1, y + Math.sin(a + 1.6) * 0.1], 0.035, light);
  }
  const face = (a: number, b: number, s: number) => {
    const P = (x: number, y: number) => pt(cx, cy, a + x * s, b + y * s);
    const head = [...P(0, 1.1), ...P(0.7, 0.9), ...P(0.95, 0.35), ...P(0.85, -0.15), ...P(0.55, -0.45), ...P(0.45, -0.85), ...P(-0.45, -0.85), ...P(-0.55, -0.45), ...P(-0.85, -0.15), ...P(-0.95, 0.35), ...P(-0.7, 0.9)];
    g.poly(head).fill(BONE[1]).stroke({ width: 0.018, color: INK, join: 'round' });
    g.poly([...P(-0.5, 0.95), ...P(0.3, 1.0), ...P(0.6, 0.6), ...P(-0.2, 0.55)]).fill({ color: BONE[2], alpha: 0.9 });
    for (const k of [-1, 1]) g.poly(oval(...P(k * 0.4, 0.05), s * 0.26, s * 0.24, 0, 10)).fill(0x1a1410);
    g.poly([...P(0, -0.15), ...P(0.12, -0.38), ...P(-0.12, -0.38)]).fill(0x1a1410);
    g.moveTo(...P(-0.35, -0.62)).lineTo(...P(0.35, -0.62)).stroke({ width: 0.01, color: INK });
    for (const k of [-0.2, 0, 0.2]) g.moveTo(...P(k, -0.5)).lineTo(...P(k, -0.75));
    g.stroke({ width: 0.008, color: INK });
  };
  const s = 0.1;
  const rows: [number, number][] = [
    [1, 0.22],
    [2, 0.06],
    [3, -0.1],
    [4, -0.26],
  ];
  for (const [n, b] of rows) for (let i = 0; i < n; i++) face((i - (n - 1) / 2) * s * 1.85 + (rnd() - 0.5) * 0.02, b + (rnd() - 0.5) * 0.02, s);
}

/** Open coffin: a six-sided wooden coffin, wide at the shoulders, its lid shoved down over the foot end and
 * askew; inside, a faded red lining and the bones of whoever lay in it. */
function openCoffin(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { len, at } = frame(f);
  const shape = (k: number, du = 0, ds = 0) => {
    const pts: number[] = [];
    for (const [u, s] of [[0.12, -0.24], [0.55, -0.38], [len - 0.1, -0.2], [len - 0.1, 0.2], [0.55, 0.38], [0.12, 0.24]]) {
      const [uu, ss] = [0.12 + (u - 0.12) * 1, s * k];
      pts.push(...at(uu + du + (u < 1 ? (1 - k) * 0.12 : -(1 - k) * 0.12), 0.5 + ss + ds));
    }
    return pts;
  };
  const outer = shape(1);
  castShadow(g, outer, light, 0.2);
  g.poly(outer).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(shape(0.86)).fill(WOOD[1]);
  g.poly(shape(0.72)).fill(0x1e1712).stroke({ width: 0.014, color: INK });
  g.poly(shape(0.62)).fill(0x5a2a24);
  g.poly(shape(0.62).map((q, i) => q + (i % 2 ? light[1] : light[0]) * 0.02)).fill(0x7a3a30);
  // Bones in it.
  const [ux, uy] = at(0, 0);
  const [vx, vy] = at(1, 0);
  skullTop(g, ...at(0.36, 0.5), Math.atan2(uy - vy, ux - vx), 0.08, light);
  bone(g, at(0.62, 0.42), at(1.02, 0.46), 0.035, light);
  bone(g, at(0.66, 0.6), at(1.0, 0.54), 0.032, light);
  for (let i = 0; i < 3; i++) {
    const u = 0.55 + i * 0.05;
    g.moveTo(...at(u, 0.44)).quadraticCurveTo(...at(u - 0.02, 0.32), ...at(u + 0.04, 0.3)).stroke({ width: 0.016, color: BONE[1], cap: 'round' });
  }
  // The lid: the lower part of the same shape, pushed down and off to one side.
  const sk = 0.08 + 0.04 * rnd();
  const lid: number[] = [];
  for (const [u, s] of [[1.1, -0.33], [len - 0.1, -0.2], [len - 0.1, 0.2], [1.1, 0.33]]) lid.push(...at(u + (s > 0 ? sk : -sk), 0.5 + s + 0.1));
  castShadow(g, lid, light, 0.05);
  g.poly(lid).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(lid.map((q, i) => q + (i % 2 ? light[1] : light[0]) * 0.015)).fill(WOOD[2]);
  for (const k of [-0.1, 0.1]) g.moveTo(...at(1.12, 0.56 + k * 2.4)).lineTo(...at(len - 0.12, 0.56 + k * 1.6));
  g.stroke({ width: 0.01, color: WOOD[0], alpha: 0.8 });
  g.moveTo(...at(1.4, 0.56)).lineTo(...at(1.7, 0.56)).moveTo(...at(1.5, 0.46)).lineTo(...at(1.5, 0.66)).stroke({ width: 0.025, color: WOOD[0] });
  for (const [u, s] of [[1.15, 0.3], [1.15, 0.82], [len - 0.15, 0.4], [len - 0.15, 0.72]]) g.circle(...at(u, s), 0.012).fill(IRON[0]);
}

/** Stone effigy: a knight carved in the round lying on a tall tomb chest — the figure stands up off the slab and
 * throws its own shadow on it: head in a mail coif on a carved pillow, sloping shoulders, hands together on the
 * hilt of a sword laid down the body, a belt, pointed feet resting on a dog; moss and a crack. */
function stoneEffigy(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, sword = true) {
  const { len, rect, at } = frame(f);
  const ST = [0x4e4a44, 0x7e796e, 0xaaa496, 0xd2ccbe];
  const k = len / 2;
  const U = (u: number) => u * k;
  // The chest: its shaded sides, then the top face set toward the light, a moulding round it.
  const outer = rect(0.06, 0.08, len - 0.06, 0.92);
  castShadow(g, quad(outer, rnd, 0), light, 0.6);
  g.poly(quad(outer, rnd, 0.006)).fill(ST[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const [x0, y0, x1, y1] = outer;
  const top = [x0 + 0.03 + light[0] * 0.03, y0 + 0.03 + light[1] * 0.03, x1 - 0.03 + light[0] * 0.03, y1 - 0.03 + light[1] * 0.03];
  g.poly(quad(top, rnd, 0.004)).fill(ST[1]).stroke({ width: 0.014, color: INK });
  g.poly(quad([top[0] + 0.04, top[1] + 0.04, top[2] - 0.04, top[3] - 0.04], rnd, 0.003)).stroke({ width: 0.012, color: ST[0] });
  // The pillow.
  const pil = quad(rect(U(0.12), 0.28, U(0.46), 0.72), rnd, 0.008);
  g.poly(pil.map((q, i) => q - (i % 2 ? light[1] : light[0]) * 0.025)).fill({ color: 0x000000, alpha: 0.35 });
  g.poly(pil).fill(ST[2]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const [u, s] of [[0.14, 0.3], [0.44, 0.3], [0.14, 0.7], [0.44, 0.7]]) g.circle(...at(U(u), s), 0.018).fill(ST[1]);
  // The figure: one outline, head to toes, mirrored about the middle.
  const prof: [number, number][] = [
    [0.19, 0.0], [0.2, 0.05], [0.24, 0.09], [0.3, 0.1], [0.37, 0.09], [0.42, 0.065], [0.46, 0.07], [0.5, 0.17], [0.56, 0.23], [0.66, 0.22],
    [0.84, 0.19], [1.0, 0.17], [1.1, 0.18], [1.3, 0.15], [1.5, 0.12], [1.62, 0.1], [1.66, 0.13], [1.74, 0.12], [1.72, 0.03], [1.68, 0.0],
  ];
  const fig: number[] = [];
  for (const [u, w] of prof) fig.push(...at(U(u), 0.5 - w));
  for (const [u, w] of [...prof].reverse()) fig.push(...at(U(u), 0.5 + w));
  g.poly(fig.map((q, i) => q - (i % 2 ? light[1] : light[0]) * 0.05)).fill({ color: 0x000000, alpha: 0.35 });
  g.poly(fig).fill(ST[2]).stroke({ width: OUT, color: INK, join: 'round' });
  // Lit along the side toward the light, shaded along the other: the figure's roundness.
  const half = (sign: number) => {
    const pts: number[] = [];
    for (const [u, w] of prof) pts.push(...at(U(u), 0.5 + sign * w));
    for (const [u, w] of [...prof].reverse()) pts.push(...at(U(u), 0.5 + sign * w * 0.35));
    return pts;
  };
  const [ax, ay] = at(1, 0.2);
  const [bx, by] = at(1, 0.8);
  const toward = (bx - ax) * light[0] + (by - ay) * light[1] > 0 ? 1 : -1;
  g.poly(half(toward)).fill({ color: ST[3], alpha: 0.75 });
  g.poly(half(-toward)).fill({ color: ST[1], alpha: 0.7 });
  // The face in its coif, the shoulder line, the belt, the split between the legs.
  g.poly(oval(...at(U(0.31), 0.5), 0.055 * k, 0.055, 0, 12)).fill(ST[3]).stroke({ width: 0.012, color: ST[0] });
  g.moveTo(...at(U(0.52), 0.32)).quadraticCurveTo(...at(U(0.47), 0.5), ...at(U(0.52), 0.68)).stroke({ width: 0.014, color: ST[0] });
  g.moveTo(...at(U(1.04), 0.33)).lineTo(...at(U(1.04), 0.67)).stroke({ width: 0.03, color: ST[0] });
  g.moveTo(...at(U(1.15), 0.5)).lineTo(...at(U(1.64), 0.5)).stroke({ width: 0.014, color: ST[0] });
  // The sword down the body (not on a sarcophagus), the hands together on its hilt or in prayer.
  if (sword) {
    g.moveTo(...at(U(0.72), 0.5)).lineTo(...at(U(1.55), 0.5)).stroke({ width: 0.06, color: INK, cap: 'round' });
    g.moveTo(...at(U(0.72), 0.5)).lineTo(...at(U(1.55), 0.5)).stroke({ width: 0.034, color: ST[3], cap: 'round' });
    g.moveTo(...at(U(0.8), 0.39)).lineTo(...at(U(0.8), 0.61)).stroke({ width: 0.055, color: INK, cap: 'round' });
    g.moveTo(...at(U(0.8), 0.39)).lineTo(...at(U(0.8), 0.61)).stroke({ width: 0.03, color: ST[3], cap: 'round' });
  }
  const hands = [...at(U(0.6), 0.5), ...at(U(0.66), 0.44), ...at(U(0.76), 0.45), ...at(U(0.78), 0.5), ...at(U(0.76), 0.55), ...at(U(0.66), 0.56)];
  g.poly(hands).fill(ST[3]).stroke({ width: 0.016, color: INK, join: 'round' });
  g.moveTo(...at(U(0.61), 0.5)).lineTo(...at(U(0.77), 0.5)).stroke({ width: 0.01, color: ST[0] });
  // The dog the feet rest on, lying across the chest: a long body, its head up at one side with ears and a
  // snout, its tail curled at the other.
  const dog: number[] = [];
  for (let i = 0; i < 16; i++) {
    const t = (i / 16) * Math.PI * 2;
    dog.push(...at(U(1.84) + Math.sin(t) * 0.085, 0.53 + Math.cos(t) * 0.2));
  }
  g.poly(dog.map((q, i) => q - (i % 2 ? light[1] : light[0]) * 0.03)).fill({ color: 0x000000, alpha: 0.3 });
  g.poly(dog).fill(ST[2]).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(U(1.83), 0.38)).quadraticCurveTo(...at(U(1.87), 0.52), ...at(U(1.83), 0.66)).stroke({ width: 0.012, color: ST[0] });
  const [hx, hy] = at(U(1.8), 0.28);
  g.poly([...at(U(1.75), 0.24), ...at(U(1.71), 0.2), ...at(U(1.77), 0.21)]).fill(ST[2]).stroke({ width: 0.012, color: INK, join: 'round' });
  g.poly([...at(U(1.75), 0.33), ...at(U(1.71), 0.37), ...at(U(1.77), 0.36)]).fill(ST[2]).stroke({ width: 0.012, color: INK, join: 'round' });
  g.circle(hx, hy, 0.075).fill(ST[2]).stroke({ width: LINE, color: INK });
  g.poly(oval(...at(U(1.86), 0.24), 0.035, 0.035, 0, 10)).fill(ST[3]).stroke({ width: 0.012, color: INK });
  g.circle(...at(U(1.89), 0.23), 0.012).fill(INK);
  g.moveTo(...at(U(1.88), 0.72)).quadraticCurveTo(...at(U(1.98), 0.78), ...at(U(1.92), 0.66)).stroke({ width: 0.022, color: INK, cap: 'round' });
  g.moveTo(...at(U(1.88), 0.72)).quadraticCurveTo(...at(U(1.98), 0.78), ...at(U(1.92), 0.66)).stroke({ width: 0.01, color: ST[2], cap: 'round' });
  // Weathering.
  g.moveTo(...at(U(1.25), 0.1)).lineTo(...at(U(1.3), 0.2)).lineTo(...at(U(1.24), 0.26)).stroke({ width: 0.012, color: INK, alpha: 0.7 });
  for (const [u, s] of [[0.1, 0.12], [len - 0.12, 0.86], [U(1.4), 0.88]]) g.poly(patch(...at(u, s), 0.07, 0.05, rnd, 0.3, 10)).fill({ color: 0x5a7a3a, alpha: 0.6 });
}

/** Burial niche: a slot cut back into the rock wall — rock jambs either side coming out of the wall, the hollow
 * darkest at the back and lighter toward its mouth, a lit stone sill along the front; in it a body in a ragged
 * shroud, the skull showing, a bone hanging out over the sill; a cobweb in the corner. */
function burialNiche(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number]) {
  const { rect, at } = frame(f, wall);
  const ROCK = [0x2e2a26, 0x45413b, 0x5a554d];
  const d = 0.56;
  // The hollow: bands from black at the back to dark brown at the mouth.
  for (let i = 0; i < 6; i++) g.poly(quad(rect(0.14, (i * d) / 6, 0.86, ((i + 1) * d) / 6 + 0.01), rnd, 0)).fill(shade(0x2a241e, 0.25 + i * 0.15));
  // The shroud and its body.
  const shroud = [...at(0.32, 0.1), ...at(0.4, 0.07), ...at(0.6, 0.1), ...at(0.78, 0.14), ...at(0.84, 0.2), ...at(0.85, 0.3), ...at(0.79, 0.37), ...at(0.6, 0.42), ...at(0.4, 0.45), ...at(0.32, 0.42)];
  g.poly(shroud).fill(0x7a746a).stroke({ width: 0.016, color: INK, join: 'round' });
  g.poly(shroud.map((q, i) => q + (i % 2 ? light[1] : light[0]) * 0.012)).fill({ color: 0x9a948a, alpha: 0.6 });
  for (const u of [0.42, 0.52, 0.62, 0.72]) g.moveTo(...at(u - 0.02, 0.09 + (u - 0.3) * 0.12)).lineTo(...at(u + 0.02, 0.43 - (u - 0.3) * 0.18));
  g.stroke({ width: 0.014, color: 0x4a463e });
  const [ux, uy] = at(0, 0.26);
  const [vx, vy] = at(1, 0.26);
  skullTop(g, ...at(0.25, 0.26), Math.atan2(uy - vy, ux - vx), 0.1, light);
  // Rock jambs either side, out of the wall, their faces toward the hollow in shadow.
  for (const [u0, u1] of [
    [0, 0.14],
    [0.86, 1],
  ]) {
    g.poly(quad(rect(u0, 0, u1, d + 0.04), rnd, 0.01)).fill(ROCK[1]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(quad(rect(u0 + 0.02, 0, u1 - 0.02, d), rnd, 0.01)).fill(ROCK[2]);
  }
  // The sill: a lit stone lip along the mouth.
  g.poly(quad(rect(0.1, d - 0.02, 0.9, d + 0.08), rnd, 0.004)).fill(STONE[2]).stroke({ width: 0.018, color: INK });
  g.moveTo(...at(0.12, d + 0.065)).lineTo(...at(0.88, d + 0.065)).stroke({ width: 0.012, color: shade(STONE[2], 1.15) });
  // A bone hanging out over the sill.
  bone(g, at(0.62, 0.44), at(0.68, 0.72), 0.042, light);
  // Cobweb in the back corner.
  const [kx, ky] = at(0.14, 0);
  const [ax, ay] = at(0.36, 0);
  const [bx, by] = at(0.14, 0.24);
  for (const t of [0.35, 0.7, 1]) g.moveTo(kx + (ax - kx) * t, ky + (ay - ky) * t).quadraticCurveTo(kx + ((ax + bx) / 2 - kx) * t * 0.6, ky + ((ay + by) / 2 - ky) * t * 0.6, kx + (bx - kx) * t, ky + (by - ky) * t);
  g.moveTo(kx, ky).lineTo((ax + bx) / 2, (ay + by) / 2);
  g.stroke({ width: 0.008, color: 0xe8e4dc, alpha: 0.7 });
}

/** Burial urn, from high up (from straight above it is only rings, from the side it stands too tall): seen from
 * about 60 degrees, so its round cross-sections show as fat ovals — a narrow foot, a round belly with a painted band
 * round its front, loop handles at the shoulders, a domed lid with a knob on top; lit from the upper left;
 * terracotta, bronze or grey stone. */
function burialUrn(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const t = [
    [0x6a321c, 0xa0542e, 0xc87a4c, 0x2a1a12],
    [0x5a4220, 0x9a7438, 0xd0a85a, 0x3a2a14],
    [0x5e5a52, 0x8a857a, 0xb0aa9e, 0x3a3630],
    [0x6a321c, 0xa0542e, 0xc87a4c, 0xe8dcc0],
  ][v % 4];
  const { pt, right } = screenAxes(light);
  const rot = Math.atan2(right[1], right[0]);
  // Height shows at half, depth at nearly full: a ring of radius w at height h is an oval w by w * 0.87.
  const [hk, dk] = [0.5, 0.87];
  const at = (a: number, h: number) => pt(cx, cy, a, h * hk);
  const ell = (h: number, w: number, a = 0, k = 1) => oval(...at(a, h), w * k, w * dk * k, rot, 20);
  // The profile, foot to lip: (height, radius).
  const prof: [number, number][] = [[-0.36, 0.12], [-0.31, 0.11], [-0.27, 0.08], [-0.2, 0.17], [-0.1, 0.25], [0.0, 0.27], [0.1, 0.23], [0.18, 0.14], [0.23, 0.1], [0.26, 0.13]];
  castShadow(g, ell(-0.36, 0.14), light, 0.35);
  // Handles, behind the body's outline.
  for (const k of [-1, 1]) {
    for (const [w, c] of [
      [0.06, INK],
      [0.034, k < 0 ? t[1] : t[0]],
    ] as const) {
      g.moveTo(...at(k * 0.2, 0.14)).bezierCurveTo(...at(k * 0.34, 0.2), ...at(k * 0.37, 0.02), ...at(k * 0.26, -0.04)).stroke({ width: w, color: c, cap: 'round' });
    }
  }
  // The body as the union of its rings: an inked rim, the shaded body, the lit side shifted to the upper left.
  for (const [h, w] of prof) g.poly(ell(h, w + 0.022));
  g.fill(INK);
  for (const [h, w] of prof) g.poly(ell(h, w));
  g.fill(t[0]);
  for (const [h, w] of prof) g.poly(ell(h + 0.02, w, -w * 0.2, 0.74));
  g.fill(t[1]);
  g.poly(ell(0.04, 0.27, -0.09, 0.3)).fill({ color: t[2], alpha: 0.9 });
  // The painted band round the belly's near side, dotted.
  const band = (h: number) => {
    const pts: [number, number][] = [];
    for (let i = 0; i <= 16; i++) {
      const a = Math.PI * (i / 16);
      pts.push(pt(cx, cy, -Math.cos(a) * 0.265, h * hk - Math.sin(a) * 0.265 * dk));
    }
    return pts;
  };
  for (const h of [-0.06, 0.04]) {
    const b = band(h);
    g.moveTo(...b[0]);
    for (const q of b.slice(1)) g.lineTo(...q);
  }
  g.stroke({ width: 0.016, color: t[3] });
  const mid = band(-0.01);
  for (let i = 2; i < 16; i += 3) g.circle(...mid[i], 0.013).fill(t[3]);
  // The lid: the mouth's rim, a dome and a knob, seen mostly from above.
  g.poly(ell(0.27, 0.135)).fill(t[0]).stroke({ width: 0.016, color: INK });
  g.poly(ell(0.3, 0.115)).fill(t[1]).stroke({ width: 0.014, color: INK });
  g.poly(ell(0.32, 0.07, -0.02, 1)).fill({ color: t[2], alpha: 0.6 });
  g.poly(ell(0.36, 0.035)).fill(t[2]).stroke({ width: 0.014, color: INK });
}

/** Offering bowl: a wide shallow bowl on the floor heaped with offerings — coins, a red fruit, petals — two
 * candle stubs beside it, petals strewn round, a wisp of incense. */
function offeringBowl(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const petal = [0xc84a4a, 0xe8c040, 0xe8e0f0, 0xc84a4a][v % 4];
  for (let i = 0; i < 10; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.3 + 0.12 * rnd();
    g.poly(oval(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.022, 0.012, rnd() * Math.PI, 8)).fill(petal).stroke({ width: 0.006, color: INK, alpha: 0.6 });
  }
  const r = 0.26;
  castShadow(g, ring(cx, cy, r, rnd, 0.004, 20), light, 0.12);
  g.circle(cx, cy, r).fill(BRONZE[0]).stroke({ width: OUT, color: INK });
  g.circle(cx + light[0] * 0.015, cy + light[1] * 0.015, r * 0.92).fill(BRONZE[1]);
  g.circle(cx - light[0] * 0.02, cy - light[1] * 0.02, r * 0.76).fill(BRONZE[0]).stroke({ width: 0.012, color: INK });
  // Offerings.
  for (let i = 0; i < 7; i++) {
    const [x, y] = [cx + (rnd() - 0.5) * 0.24, cy + (rnd() - 0.5) * 0.24];
    g.circle(x, y, 0.035).fill(GILT[1]).stroke({ width: 0.01, color: GILT[0] });
    g.circle(x + light[0] * 0.01, y + light[1] * 0.01, 0.015).fill(GILT[2]);
  }
  const [fx, fy] = [cx + 0.06, cy - 0.05];
  g.circle(fx, fy, 0.07).fill(0x9a2a2a).stroke({ width: 0.014, color: INK });
  g.circle(fx + light[0] * 0.025, fy + light[1] * 0.025, 0.025).fill(0xe87a6a);
  g.moveTo(fx, fy).lineTo(fx + 0.02, fy - 0.03).stroke({ width: 0.012, color: 0x3a5a22 });
  for (let i = 0; i < 4; i++) g.poly(oval(cx - 0.08 + i * 0.03, cy + 0.08 - i * 0.02, 0.025, 0.014, i, 8)).fill(petal).stroke({ width: 0.006, color: INK, alpha: 0.6 });
  for (const k of [-1, 1]) candle(g, cx + k * 0.36, cy + k * 0.12);
  // Incense: a thin curling wisp.
  g.moveTo(cx - 0.1, cy - 0.05).bezierCurveTo(cx - 0.2, cy - 0.15, cx - 0.02, cy - 0.22, cx - 0.12, cy - 0.34).stroke({ width: 0.018, color: 0xe8e8ec, alpha: 0.5, cap: 'round' });
}

const BURLAP = [0x7a6440, 0xa88c5c, 0xc8ac78];

/** Cold campfire: a ring of fire-blackened stones round grey ash, charred logs crossed in it with grey-white ash on
 * their ends; a forked stick either side holding a spit across. Cold: no glow. */
function coldCampfire(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx, cy, 0.3, 0.3, rnd, 0.1, 16)).fill(0x3a3632);
  g.poly(patch(cx, cy, 0.24, 0.24, rnd, 0.14, 14)).fill(0x8a8680);
  for (let i = 0; i < 10; i++) g.circle(cx + (rnd() - 0.5) * 0.36, cy + (rnd() - 0.5) * 0.36, 0.02 + 0.02 * rnd()).fill({ color: 0xb8b4ae, alpha: 0.7 });
  // Charred logs, crossed.
  const ph = rnd() * Math.PI;
  for (let i = 0; i < 3; i++) {
    const a = ph + (i * Math.PI) / 3;
    const [dx, dy] = [Math.cos(a) * 0.2, Math.sin(a) * 0.2];
    for (const [w, c] of [
      [0.085, INK],
      [0.06, 0x2a221c],
    ] as const) {
      g.moveTo(cx - dx, cy - dy).lineTo(cx + dx, cy + dy).stroke({ width: w, color: c, cap: 'round' });
    }
    g.moveTo(cx - dx * 0.6, cy - dy * 0.6).lineTo(cx + dx * 0.6, cy + dy * 0.6).stroke({ width: 0.012, color: 0x4a3e34, cap: 'round' });
    for (const k of [-1, 1]) g.circle(cx + dx * k, cy + dy * k, 0.026).fill(0xc8c4bc);
  }
  // The stone ring.
  for (let i = 0; i < 11; i++) {
    const a = (i / 11) * Math.PI * 2 + rnd() * 0.2;
    rock(g, cx + Math.cos(a) * 0.32, cy + Math.sin(a) * 0.32, 0.06 + 0.025 * rnd(), [0x3a3632, 0x5e5a54, 0x8a857c, 0xa8a296], light, rnd, 6);
  }
  // The spit on its forked sticks.
  const sa = ph + Math.PI / 2;
  const [ex, ey] = [Math.cos(sa) * 0.42, Math.sin(sa) * 0.42];
  for (const k of [-1, 1]) {
    g.circle(cx + ex * k, cy + ey * k, 0.04).fill(WOOD[1]).stroke({ width: 0.016, color: INK });
  }
  g.moveTo(cx - ex * 1.05, cy - ey * 1.05).lineTo(cx + ex * 1.05, cy + ey * 1.05).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.moveTo(cx - ex * 1.05, cy - ey * 1.05).lineTo(cx + ex * 1.05, cy + ey * 1.05).stroke({ width: 0.022, color: WOOD[2], cap: 'round' });
}

/** Rubble: a spill of broken rock over the whole square — a couple of big angular chunks throwing shadows,
 * medium ones, many small chips — on a dark fan of grit, a crack in the floor beside it. */
function rubbleHeap(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const DARK = [0x3a342c, 0x564e42, 0x766c5c, 0x948874];
  g.poly(patch(cx, cy, 0.48, 0.44, rnd, 0.25, 22)).fill({ color: 0x2e2a24, alpha: 0.35 });
  const a = rnd() * Math.PI * 2;
  g.moveTo(cx + Math.cos(a) * 0.46, cy + Math.sin(a) * 0.46)
    .lineTo(cx + Math.cos(a) * 0.3 + 0.04, cy + Math.sin(a) * 0.3 - 0.03)
    .lineTo(cx + Math.cos(a) * 0.2, cy + Math.sin(a) * 0.2)
    .stroke({ width: 0.014, color: INK, alpha: 0.7, join: 'miter' });
  for (let i = 0; i < 36; i++) {
    const b = rnd() * Math.PI * 2;
    const d = 0.45 * Math.sqrt(rnd());
    const [x, y] = [cx + Math.cos(b) * d, cy + Math.sin(b) * d];
    const r = 0.01 + 0.012 * rnd();
    g.poly([x - r, y, x, y - r * 0.8, x + r * 0.9, y + r * 0.2, x, y + r]).fill(i % 3 ? DARK[1] : DARK[2]);
  }
  const chunks: [number, number, number][] = [];
  for (let i = 0; i < 2; i++) chunks.push([cx + (rnd() - 0.5) * 0.3, cy + (rnd() - 0.5) * 0.3, 0.13 + 0.05 * rnd()]);
  for (let i = 0; i < 5; i++) {
    const b = rnd() * Math.PI * 2;
    const d = 0.18 + 0.18 * rnd();
    chunks.push([cx + Math.cos(b) * d, cy + Math.sin(b) * d, 0.06 + 0.035 * rnd()]);
  }
  for (let i = 0; i < 10; i++) {
    const b = rnd() * Math.PI * 2;
    const d = 0.1 + 0.32 * rnd();
    chunks.push([cx + Math.cos(b) * d, cy + Math.sin(b) * d, 0.025 + 0.02 * rnd()]);
  }
  for (const [x, y, s] of chunks) if (s > 0.1) castShadow(g, patch(x, y, s, s, rnd, 0.2, 6), light, 0.25);
  chunks.sort((p, q) => p[2] - q[2]);
  for (const [x, y, s] of chunks) rock(g, x, y, s, DARK, light, rnd, s > 0.1 ? 5 : 4 + Math.floor(rnd() * 2));
}

/** Flotsam heap: what the water left — two broken planks, a split barrel hoop, a sodden rag, a bottle, sticks — in
 * a slick of greenish mud. */
function flotsamHeap(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx, cy, 0.42, 0.38, rnd, 0.2, 18)).fill({ color: 0x3a3e24, alpha: 0.45 });
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI;
    const [x, y] = [cx + (rnd() - 0.5) * 0.6, cy + (rnd() - 0.5) * 0.6];
    const l = 0.1 + 0.1 * rnd();
    for (const [w, c] of [
      [0.04, INK],
      [0.022, 0x5a4428],
    ] as const) {
      g.moveTo(x - Math.cos(a) * l, y - Math.sin(a) * l).lineTo(x + Math.cos(a) * l, y + Math.sin(a) * l).stroke({ width: w, color: c, cap: 'round' });
    }
  }
  // A rag.
  const rag = patch(cx - 0.2, cy + 0.18, 0.13, 0.09, rnd, 0.35, 10);
  g.poly(rag).fill(0x7a6e5a).stroke({ width: 0.016, color: INK, join: 'round' });
  g.moveTo(cx - 0.28, cy + 0.16).lineTo(cx - 0.12, cy + 0.22).stroke({ width: 0.01, color: 0x5a5040 });
  // Broken planks, a jagged end each.
  const plank = (x: number, y: number, a: number, l: number, w: number) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const P = (u: number, k: number): [number, number] => [x + c * u - s * k, y + s * u + c * k];
    const pts = [...P(-l, -w), ...P(l * 0.7, -w), ...P(l * 0.85, -w * 0.4), ...P(l, -w * 0.1), ...P(l * 0.8, w * 0.3), ...P(l * 0.92, w), ...P(-l, w)];
    castShadow(g, pts, light, 0.05);
    g.poly(pts).fill(0x7a5a3c).stroke({ width: 0.02, color: INK, join: 'miter' });
    g.poly([...P(-l + 0.02, -w + 0.02), ...P(l * 0.6, -w + 0.02), ...P(l * 0.6, -w * 0.1), ...P(-l + 0.02, -w * 0.1)]).fill(0x8e6c48);
    g.moveTo(...P(-l * 0.9, w * 0.3)).lineTo(...P(l * 0.6, w * 0.35)).stroke({ width: 0.01, color: 0x4a3828 });
    for (const k of [-0.5, 0.5]) g.circle(...P(-l * 0.8, w * k), 0.014).fill(IRON[0]);
  };
  plank(cx - 0.04, cy - 0.06, rnd() * Math.PI, 0.3, 0.06);
  plank(cx + 0.08, cy + 0.1, rnd() * Math.PI, 0.24, 0.055);
  // A split hoop.
  const [ox, oy] = [cx + 0.22, cy - 0.22];
  g.moveTo(ox + 0.13, oy).arc(ox, oy, 0.13, 0, Math.PI * 1.5).stroke({ width: 0.04, color: INK });
  g.moveTo(ox + 0.13, oy).arc(ox, oy, 0.13, 0, Math.PI * 1.5).stroke({ width: 0.022, color: IRON[1] });
  // A bottle.
  const ba = rnd() * Math.PI * 2;
  const [bx, by] = [cx - 0.24, cy - 0.22];
  g.poly(oval(bx, by, 0.09, 0.045, ba, 12)).fill(0x3a6a4a).stroke({ width: 0.016, color: INK });
  for (const [w, c] of [
    [0.045, INK],
    [0.028, 0x3a6a4a],
  ] as const) {
    g.moveTo(bx + Math.cos(ba) * 0.08, by + Math.sin(ba) * 0.08).lineTo(bx + Math.cos(ba) * 0.15, by + Math.sin(ba) * 0.15).stroke({ width: w, color: c, cap: 'round' });
  }
  g.poly(oval(bx - Math.cos(ba) * 0.02 + light[0] * 0.015, by - Math.sin(ba) * 0.02 + light[1] * 0.015, 0.04, 0.012, ba, 8)).fill({ color: 0xffffff, alpha: 0.6 });
}

/** Timber prop: a mine set seen from above — two squared posts, their ends showing either side of the rough-hewn
 * cap beam laid across them, axe facets along the cap, wedges driven in over the posts, a long shadow (it holds
 * up the roof). */
function timberProp(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const ba = Math.floor(rnd() * 2) * (Math.PI / 2);
  const [c, s] = [Math.cos(ba), Math.sin(ba)];
  const P = (u: number, k: number): [number, number] => [cx + c * u - s * k, cy + s * u + c * k];
  const sq = (u: number, k: number, h: number) => [...P(u - h, k - h), ...P(u + h, k - h), ...P(u + h, k + h), ...P(u - h, k + h)];
  const cap = [...P(-0.47, -0.06), ...P(0.47, -0.065), ...P(0.48, 0.06), ...P(-0.46, 0.065)];
  for (const u of [-0.32, 0.32]) castShadow(g, sq(u, 0, 0.15), light, 0.6);
  castShadow(g, cap, light, 0.3);
  // The posts.
  for (const u of [-0.32, 0.32]) {
    const post = sq(u, 0, 0.15);
    g.poly(post).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
    for (const r of [0.04, 0.08, 0.12]) g.poly(oval(...P(u, 0), r, r, ba, 14)).stroke({ width: 0.01, color: WOOD[0] });
    g.circle(...P(u, 0), 0.015).fill(WOOD[0]);
  }
  // The cap: hewn, with axe facets down its length.
  g.poly(cap).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...P(-0.45, -0.045), ...P(0.45, -0.05), ...P(0.46, 0.015), ...P(-0.44, 0.02)]).fill(WOOD[2]);
  for (let i = 0; i < 7; i++) {
    const u = -0.4 + i * 0.13 + (rnd() - 0.5) * 0.04;
    g.moveTo(...P(u, -0.045)).lineTo(...P(u + 0.04, 0.015));
  }
  g.stroke({ width: 0.01, color: WOOD[0], alpha: 0.8 });
  g.moveTo(...P(-0.44, 0.035)).lineTo(...P(0.45, 0.03)).stroke({ width: 0.01, color: shade(WOOD[0], 0.8) });
  // Wedges over the posts.
  for (const u of [-0.32, 0.32]) {
    g.poly([...P(u - 0.06, -0.06), ...P(u + 0.06, -0.06), ...P(u + 0.04, -0.12), ...P(u - 0.04, -0.12)]).fill(WOOD[2]).stroke({ width: 0.012, color: INK, join: 'round' });
  }
}

/** Rails: two iron rails across the square, edge to edge so neighbours join, on two wooden sleepers in a strip of
 * gravel. */
function rails(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  g.rect(x, y + 0.14, 1, 0.72).fill({ color: 0x5a554c, alpha: 0.35 });
  for (let i = 0; i < 14; i++) g.circle(x + rnd(), y + 0.16 + rnd() * 0.68, 0.01 + 0.01 * rnd()).fill({ color: 0x8a857a, alpha: 0.6 });
  for (const u of [0.25, 0.75]) {
    const sl = quad([x + u - 0.07, y + 0.16, x + u + 0.07, y + 0.84], rnd, 0.006);
    g.poly(sl).fill(DARK_WOOD[1]).stroke({ width: 0.016, color: INK });
    g.moveTo(x + u - 0.02, y + 0.2).lineTo(x + u - 0.015, y + 0.8).stroke({ width: 0.008, color: DARK_WOOD[0] });
  }
  for (const s of [0.3, 0.7]) {
    g.rect(x, y + s - 0.035, 1, 0.07).fill(INK);
    g.rect(x, y + s - 0.022, 1, 0.044).fill(IRON[1]);
    g.rect(x, y + s - 0.022 + (light[1] > 0 ? 0.026 : 0), 1, 0.018).fill(IRON[2]);
    for (const u of [0.25, 0.75]) g.circle(x + u, y + s + 0.05, 0.012).fill(IRON[0]);
  }
}

/** Ore cart: a deep tapered iron-strapped tub (wider at the rim than the floor), its inside walls showing and
 * the near wall's shadow across the ore lying in the bottom, glinting gold; four wheels showing at its corners, a
 * push bar at the back and a coupling hook at the front. */
function oreCart(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [hw, hh] = [0.33, 0.24];
  castShadow(g, [cx - hw, cy - hh, cx + hw, cy - hh, cx + hw, cy + hh, cx - hw, cy + hh], light, 0.4);
  // Wheels at the corners, poking out of the long sides.
  for (const [dx, dy] of [[-0.21, -1], [0.21, -1], [-0.21, 1], [0.21, 1]]) {
    const r = [cx + dx - 0.1, cy + dy * (hh + 0.035) - 0.035, cx + dx + 0.1, cy + dy * (hh + 0.035) + 0.035];
    g.roundRect(r[0], r[1], r[2] - r[0], r[3] - r[1], 0.03).fill(IRON[0]).stroke({ width: 0.016, color: INK });
    g.moveTo(r[0] + 0.03, (r[1] + r[3]) / 2).lineTo(r[2] - 0.03, (r[1] + r[3]) / 2).stroke({ width: 0.01, color: IRON[2] });
  }
  // Push bar at the back, coupling at the front.
  g.moveTo(cx - hw, cy - 0.14).lineTo(cx - hw - 0.1, cy - 0.14).lineTo(cx - hw - 0.1, cy + 0.14).lineTo(cx - hw, cy + 0.14).stroke({ width: 0.05, color: INK, join: 'round' });
  g.moveTo(cx - hw, cy - 0.14).lineTo(cx - hw - 0.1, cy - 0.14).lineTo(cx - hw - 0.1, cy + 0.14).lineTo(cx - hw, cy + 0.14).stroke({ width: 0.026, color: WOOD[1], join: 'round' });
  g.moveTo(cx + hw, cy).lineTo(cx + hw + 0.08, cy).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.circle(cx + hw + 0.1, cy, 0.03).stroke({ width: 0.018, color: IRON[1] });
  // The tub: the rim, sloping sides in, the floor.
  g.roundRect(cx - hw, cy - hh, hw * 2, hh * 2, 0.05).fill(WOOD[0]).stroke({ width: OUT, color: INK });
  const [iw, ih] = [hw - 0.1, hh - 0.1];
  const lit = (x: number, y: number) => (x * light[0] + y * light[1] > 0 ? WOOD[2] : WOOD[0]);
  for (const [ax, ay, bx2, by2, nx, ny] of [
    [-1, -1, 1, -1, 0, -1],
    [1, -1, 1, 1, 1, 0],
    [1, 1, -1, 1, 0, 1],
    [-1, 1, -1, -1, -1, 0],
  ]) {
    g.poly([cx + ax * (hw - 0.03), cy + ay * (hh - 0.03), cx + bx2 * (hw - 0.03), cy + by2 * (hh - 0.03), cx + bx2 * iw, cy + by2 * ih, cx + ax * iw, cy + ay * ih]).fill(lit(-nx, -ny));
  }
  g.rect(cx - iw, cy - ih, iw * 2, ih * 2).fill(0x2e2a26).stroke({ width: 0.012, color: INK });
  // Iron straps: round the rim, down each corner.
  g.roundRect(cx - hw + 0.015, cy - hh + 0.015, hw * 2 - 0.03, hh * 2 - 0.03, 0.04).stroke({ width: 0.022, color: IRON[0] });
  for (const [kx, ky] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
    g.moveTo(cx + kx * (hw - 0.02), cy + ky * (hh - 0.02)).lineTo(cx + kx * iw, cy + ky * ih).stroke({ width: 0.03, color: IRON[0] });
    g.circle(cx + kx * (hw - 0.035), cy + ky * (hh - 0.035), 0.01).fill(IRON[2]);
  }
  // The load, down in the bottom: lumps kept clear of the walls.
  const ORE = [0x3e3a36, 0x6a645c, 0x8e877c, 0xa8a090];
  for (let i = 0; i < 12; i++) {
    const sz = 0.03 + 0.02 * rnd();
    rock(g, cx + (rnd() * 2 - 1) * (iw - sz - 0.01), cy + (rnd() * 2 - 1) * (ih - sz - 0.01), sz, ORE, light, rnd, 5);
  }
  for (let i = 0; i < 6; i++) {
    const [x, y] = [cx + (rnd() * 2 - 1) * (iw - 0.03), cy + (rnd() * 2 - 1) * (ih - 0.03)];
    g.poly([x, y - 0.018, x + 0.011, y, x, y + 0.018, x - 0.011, y]).fill(0xf0c850);
  }
  // The walls on the light's side shade the bottom along their foot.
  const sh = 0.06;
  if (light[0] < 0) g.rect(cx - iw, cy - ih, sh, ih * 2).fill({ color: 0x000000, alpha: 0.35 });
  if (light[0] > 0) g.rect(cx + iw - sh, cy - ih, sh, ih * 2).fill({ color: 0x000000, alpha: 0.35 });
  if (light[1] < 0) g.rect(cx - iw, cy - ih, iw * 2, sh).fill({ color: 0x000000, alpha: 0.35 });
  if (light[1] > 0) g.rect(cx - iw, cy + ih - sh, iw * 2, sh).fill({ color: 0x000000, alpha: 0.35 });
  g.rect(cx - iw, cy - ih, iw * 2, ih * 2).stroke({ width: 0.012, color: INK });
}

/** Ore vein (after a reference picture): a pale grey lump of rock against the wall, speckled, with chunky rounded
 * nuggets of ore bursting out of it — glowing orange gold, copper green or silver — each shaded dark at its root
 * and bright at its tip, a few chips knocked off onto the floor. */
function oreVein(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const ore = [
    [0x6a2a0a, 0xc8601a, 0xf0a030, 0xffe08a],
    [0x1e4a3a, 0x2e8a6a, 0x5ac8a0, 0xc8ffe8],
    [0x3a4250, 0x7a8698, 0xb8c4d4, 0xffffff],
    [0x6a2a0a, 0xc8601a, 0xf0a030, 0xffe08a],
  ][v % 4];
  const ROCK = [0x5e6468, 0x8a9296, 0xaab2b4, 0xc8ced0];
  const [cx, cy] = at(0.5, 0.3);
  // The rock: a lumpy pale mass out from the wall.
  const lump: number[] = [];
  const n = 13;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const r = 0.29 * (0.8 + 0.35 * rnd());
    lump.push(cx + Math.cos(a) * r * 1.15, cy + Math.sin(a) * r);
  }
  castShadow(g, lump, light, 0.3);
  const nug = (a: number, d: number, len: number, w: number) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const P = (u: number, k: number): [number, number] => [cx + c * (d + u) - s * k, cy + s * (d + u) + c * k];
    const shape = oval(...P(len * 0.5, 0), len * 0.55, w, a, 16);
    g.poly(shape).fill(ore[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(oval(...P(len * 0.56, w * 0.1), len * 0.44, w * 0.72, a, 14)).fill(ore[1]);
    g.poly(oval(...P(len * 0.66, w * 0.2), len * 0.3, w * 0.45, a, 12)).fill(ore[2]);
    g.poly(oval(...P(len * 0.78, w * 0.28), len * 0.1, w * 0.16, a, 8)).fill(ore[3]);
  };
  // The rock, then nuggets bursting out of it at uneven angles, bunched to one side, different sizes.
  g.poly(lump).fill(ROCK[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(lump.map((q, i) => (i % 2 ? cy : cx) + (q - (i % 2 ? cy : cx)) * 0.86 + (i % 2 ? light[1] : light[0]) * 0.03)).fill(ROCK[1]);
  g.poly(patch(cx + light[0] * 0.08, cy + light[1] * 0.08, 0.14, 0.11, rnd, 0.2, 10)).fill(ROCK[2]);
  for (let i = 0; i < 9; i++) {
    const [x, y] = [cx + (rnd() - 0.5) * 0.44, cy + (rnd() - 0.5) * 0.36];
    g.circle(x, y, 0.012 + 0.01 * rnd()).fill(i % 2 ? ROCK[0] : ROCK[3]);
  }
  const [wx, wy] = at(0.5, 0);
  const away = Math.atan2(cy - wy, cx - wx);
  const ph = away + (rnd() < 0.5 ? -1 : 1) * (0.5 + 0.3 * rnd());
  const nugs: [number, number, number, number][] = [
    [ph - 0.9, 0.13, 0.26, 0.09],
    [ph + 0.25, 0.16, 0.22, 0.08],
    [ph + 1.5 + 0.4 * rnd(), 0.15, 0.17, 0.065],
  ];
  for (const [a, d, len, w] of nugs) nug(a, d, len, w);
  // Chips on the floor in front.
  for (let i = 0; i < 4; i++) {
    const [x, y] = at(0.2 + rnd() * 0.6, 0.66 + rnd() * 0.2);
    rock(g, x, y, 0.025 + 0.015 * rnd(), ROCK, light, rnd, 5);
    if (i % 2 === 0) g.circle(x + 0.012, y - 0.005, 0.012).fill(ore[2]).stroke({ width: 0.006, color: INK });
  }
}

/** Mining tools laid side by side: a pickaxe (a curved two-pointed iron head across the top of a long haft), a
 * shovel (D-grip, a flat pointed spade blade) and a sledgehammer (a heavy block head), each a little askew. */
function miningTools(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const haft = (a: [number, number], b: [number, number], w: number) => {
    g.moveTo(...a).lineTo(...b).stroke({ width: w + 0.025, color: INK, cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: w, color: WOOD[1], cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: w * 0.3, color: WOOD[2], cap: 'round' });
  };
  const tool = (ox: number, a: number) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    return (u: number, k: number): [number, number] => [x + ox + c * k - s * u, y + 0.5 + s * k + c * u];
  };
  // The pickaxe: u runs down the haft, k across.
  const K = tool(0.22, (rnd() - 0.5) * 0.25);
  castShadow(g, [...K(-0.34, -0.2), ...K(-0.34, 0.2), ...K(0.36, 0.03), ...K(0.36, -0.03)], light, 0.04);
  haft(K(-0.3, 0), K(0.38, 0), 0.04);
  for (const [w, col] of [
    [0.08, INK],
    [0.052, IRON[1]],
  ] as const) {
    g.moveTo(...K(-0.24, -0.2)).quadraticCurveTo(...K(-0.36, 0), ...K(-0.24, 0.2)).stroke({ width: w, color: col, cap: 'round' });
  }
  g.moveTo(...K(-0.28, -0.12)).quadraticCurveTo(...K(-0.355, 0), ...K(-0.28, 0.12)).stroke({ width: 0.015, color: IRON[2], cap: 'round' });
  g.poly([...K(-0.36, -0.05), ...K(-0.26, -0.05), ...K(-0.26, 0.05), ...K(-0.36, 0.05)]).fill(IRON[0]).stroke({ width: 0.012, color: INK });
  // The shovel.
  const S = tool(0.5, (rnd() - 0.5) * 0.2);
  haft(S(-0.32, 0), S(0.08, 0), 0.034);
  g.poly([...S(-0.4, -0.06), ...S(-0.34, -0.06), ...S(-0.34, 0.06), ...S(-0.4, 0.06)]).stroke({ width: 0.026, color: INK });
  g.moveTo(...S(-0.4, 0)).lineTo(...S(-0.34, 0)).stroke({ width: 0.02, color: WOOD[1] });
  const blade = [...S(0.06, -0.1), ...S(0.06, 0.1), ...S(0.32, 0.1), ...S(0.42, 0), ...S(0.32, -0.1)];
  castShadow(g, blade, light, 0.03);
  g.poly(blade).fill(IRON[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...S(0.09, -0.06), ...S(0.09, 0.06), ...S(0.3, 0.06), ...S(0.36, 0), ...S(0.3, -0.06)]).fill(IRON[2]);
  g.moveTo(...S(0.07, 0)).lineTo(...S(0.22, 0)).stroke({ width: 0.016, color: IRON[0] });
  // The sledgehammer.
  const H = tool(0.79, (rnd() - 0.5) * 0.25);
  haft(H(-0.18, 0), H(0.36, 0), 0.034);
  const head = [...H(-0.32, -0.11), ...H(-0.32, 0.11), ...H(-0.16, 0.11), ...H(-0.16, -0.11)];
  castShadow(g, head, light, 0.08);
  g.poly(head).fill(IRON[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...H(-0.3, -0.09), ...H(-0.3, 0.06), ...H(-0.18, 0.06), ...H(-0.18, -0.09)]).fill(IRON[1]);
  g.moveTo(...H(-0.3, -0.09)).lineTo(...H(-0.3, 0.06)).stroke({ width: 0.012, color: IRON[2] });
}

/** Hanging lantern, seen from high up (about 60 degrees, like the urn): a square iron lantern turned corner-on, its
 * pyramid cap and ring on top, the two near glass sides glowing, a short run of chain above, a pool of warm light on
 * the floor under it. */
function hangingLantern(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  void rnd;
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  for (let i = 0; i < 5; i++) g.circle(cx, cy, 0.4 - i * 0.06).fill({ color: 0xf8c860, alpha: 0.06 });
  const { pt } = screenAxes(light);
  const [hk, dk] = [0.5, 0.87];
  // (x across, y depth, h height) to the grid.
  const Q = (x: number, y: number, h: number) => pt(cx, cy, x, h * hk + y * dk);
  const sq = (s: number, h: number) => [Q(0, s, h), Q(s, 0, h), Q(0, -s, h), Q(-s, 0, h)];
  const s = 0.1;
  const [bot, top] = [sq(s * 1.05, -0.12), sq(s, 0.08)];
  const flat = (pts: [number, number][]) => pts.flat();
  // Base, the near glass sides, iron corner posts, the cap.
  g.poly(flat(bot)).fill(IRON[0]).stroke({ width: 0.015, color: INK, join: 'round' });
  for (const [i, j, col] of [
    [3, 2, 0xfff0b0],
    [2, 1, 0xf0c060],
  ] as const) {
    g.poly(flat([bot[i], bot[j], top[j], top[i]])).fill(col).stroke({ width: 0.015, color: INK, join: 'round' });
  }
  for (const i of [3, 2, 1]) g.moveTo(...bot[i]).lineTo(...top[i]);
  g.stroke({ width: 0.016, color: IRON[0], cap: 'round' });
  const apex = Q(0, 0, 0.18);
  const lit = [IRON[1], IRON[0], IRON[1], IRON[2]];
  for (let i = 0; i < 4; i++) g.poly(flat([top[i], top[(i + 1) % 4], apex])).fill(lit[i]);
  g.poly(flat(top)).stroke({ width: 0.015, color: INK, join: 'round' });
  for (let i = 0; i < 4; i++) g.moveTo(...top[i]).lineTo(...apex);
  g.stroke({ width: 0.009, color: INK, alpha: 0.8 });
  // The ring on top and the chain above it.
  const [ax, ay] = apex;
  g.poly(oval(ax, ay, 0.028, 0.025, 0, 12)).stroke({ width: 0.022, color: INK });
  g.poly(oval(ax, ay, 0.028, 0.025, 0, 12)).stroke({ width: 0.011, color: IRON[1] });
  for (let i = 1; i <= 3; i++) {
    const [lx, ly] = Q(0, 0, 0.18 + i * 0.045);
    g.poly(oval(lx, ly, 0.01, 0.015, 0, 8)).stroke({ width: 0.009, color: IRON[0] });
  }
}

/** Blasting powder kegs: three small kegs standing together, a red flame painted on each lid, grainy black powder
 * spilled by them, a tarred fuse running from one keg's bung across the floor to a coil, its end frayed. */
function powderKegs(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  // Spilled powder: a grainy black drift.
  for (let i = 0; i < 40; i++) {
    const [x, y] = [cx + 0.22 + (rnd() - 0.5) * 0.24 * (1 + rnd()), cy + 0.3 + (rnd() - 0.5) * 0.14];
    g.circle(x, y, 0.008 + 0.01 * rnd()).fill(i % 4 ? 0x1a1816 : 0x4a4640);
  }
  const kegs: [number, number][] = [
    [-0.17, -0.14],
    [0.17, -0.12],
    [0.02, 0.15],
  ];
  for (const [dx, dy] of kegs) castShadow(g, ring(cx + dx, cy + dy, 0.17, rnd, 0.005, 14), light, 0.35);
  for (const [dx, dy] of kegs) {
    const [x, y] = [cx + dx, cy + dy];
    barrel(g, x, y, 0.17, light, rnd);
    g.poly([x, y - 0.075, x + 0.04, y + 0.0, x + 0.035, y + 0.045, x, y + 0.06, x - 0.035, y + 0.045, x - 0.04, y]).fill(0xc0301c).stroke({ width: 0.01, color: INK });
    g.poly([x, y - 0.02, x + 0.017, y + 0.02, x, y + 0.04, x - 0.017, y + 0.02]).fill(0xf0a030);
  }
  // The fuse: from the front keg's bung down over its side and across the floor to a coil.
  const [bx, by] = [cx + 0.02 + 0.17 * 0.3, cy + 0.15 - 0.17 * 0.1];
  const [kx, ky] = [cx - 0.3, cy + 0.36];
  const path = () => g.moveTo(bx, by).bezierCurveTo(bx - 0.02, by + 0.2, kx + 0.2, ky + 0.06, kx + 0.09, ky);
  const coil = () => {
    g.moveTo(kx + 0.09, ky);
    for (let i = 1; i <= 40; i++) {
      const t = (i / 40) * Math.PI * 2 * 2.5;
      const r = 0.09 - (i / 40) * 0.06;
      g.lineTo(kx + Math.cos(t) * r, ky + Math.sin(t) * r * 0.8);
    }
  };
  for (const [w, c] of [
    [0.03, INK],
    [0.016, 0x4a3420],
  ] as const) {
    path();
    g.stroke({ width: w, color: c, cap: 'round' });
    coil();
    g.stroke({ width: w, color: c, cap: 'round', join: 'round' });
  }
  const [ex, ey] = [kx + Math.cos(Math.PI * 5) * 0.03, ky];
  for (const b of [-0.5, 0, 0.5]) g.moveTo(ex, ey).lineTo(ex + Math.cos(Math.PI + b) * 0.03, ey + Math.sin(Math.PI + b) * 0.03);
  g.stroke({ width: 0.008, color: 0x8a6a40, cap: 'round' });
}

/** Bedroll: a ground pad with a wool blanket over it, turned back at the head, a rolled cloak for a pillow, a strap
 * and buckle lying loose; rumpled. */
function bedroll(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { len, rect, at } = frame(f);
  const wool = [
    [0x4a5a3a, 0x6a7a4a, 0x8a9a62],
    [0x6a3a2a, 0x8a4e36, 0xa86a48],
    [0x3a4a6a, 0x4e6288, 0x6a80a8],
    [0x6a5a3a, 0x8a7650, 0xa89068],
  ][v % 4];
  const pad = quad(rect(0.14, 0.14, len - 0.1, 0.86), rnd, 0.02);
  castShadow(g, pad, light, 0.05);
  g.poly(pad).fill(0x8a7650).stroke({ width: LINE, color: INK, join: 'round' });
  // Blanket, rumpled, turned back at the head end.
  const bl: number[] = [];
  for (const [u, s] of [[0.62, 0.17], [len * 0.6, 0.15], [len - 0.16, 0.18], [len - 0.14, 0.5], [len - 0.17, 0.83], [len * 0.6, 0.86], [0.62, 0.84]]) bl.push(...at(u + (rnd() - 0.5) * 0.03, s + (rnd() - 0.5) * 0.03));
  g.poly(bl).fill(wool[1]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const s of [0.36, 0.64]) g.moveTo(...at(0.66, s)).lineTo(...at(len - 0.18, s));
  g.stroke({ width: 0.02, color: wool[0] });
  g.moveTo(...at(0.66, 0.5)).lineTo(...at(len - 0.18, 0.5)).stroke({ width: 0.012, color: wool[2] });
  g.moveTo(...at(1.1, 0.2)).quadraticCurveTo(...at(1.18, 0.5), ...at(1.08, 0.8)).stroke({ width: 0.016, color: wool[0] });
  const turn = quad(rect(0.56, 0.16, 0.74, 0.84), rnd, 0.015);
  g.poly(turn).fill(wool[2]).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(0.65, 0.2)).lineTo(...at(0.65, 0.8)).stroke({ width: 0.012, color: wool[1] });
  // The rolled cloak for a pillow.
  const pil = quad(rect(0.18, 0.24, 0.48, 0.76), rnd, 0.02);
  g.poly(pil).fill(0x6a5e4c).stroke({ width: LINE, color: INK, join: 'round' });
  for (const s of [0.36, 0.5, 0.64]) g.moveTo(...at(0.2, s)).quadraticCurveTo(...at(0.33, s + 0.03), ...at(0.46, s));
  g.stroke({ width: 0.012, color: 0x4a4234 });
  // A loose strap.
  g.moveTo(...at(len - 0.5, 0.88)).quadraticCurveTo(...at(len - 0.35, 0.96), ...at(len - 0.18, 0.9)).stroke({ width: 0.03, color: 0x4a3020, cap: 'round' });
  g.rect(...at(len - 0.2, 0.88), 0.04, 0.04).stroke({ width: 0.01, color: BRASS });
}

/** Grain sacks: three plump burlap sacks, each gathered at the neck and tied with cord, the cloth pulled into creases
 * running in to the tie; the weave and a stencilled mark showing; one tipped over with grain spilled from it. */
function grainSacks(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx + 0.26, cy + 0.3, 0.13, 0.08, rnd, 0.3, 12)).fill(0xd8b868).stroke({ width: 0.01, color: 0xa08040 });
  for (let i = 0; i < 16; i++) g.poly(oval(cx + 0.14 + rnd() * 0.3, cy + 0.2 + rnd() * 0.2, 0.014, 0.008, rnd() * 3, 6)).fill(0xc8a050);
  const sack = (x: number, y: number, rx: number, ry: number, a: number, tipped: boolean) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const body = oval(x, y, rx, ry, a, 20).map((q) => q + (rnd() - 0.5) * 0.02);
    castShadow(g, body, light, 0.3);
    g.poly(body).fill(BURLAP[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(oval(x + light[0] * 0.03, y + light[1] * 0.03, rx * 0.84, ry * 0.82, a, 16)).fill(BURLAP[1]);
    g.poly(oval(x + light[0] * 0.07, y + light[1] * 0.07, rx * 0.42, ry * 0.36, a, 12)).fill({ color: BURLAP[2], alpha: 0.6 });
    // Where the neck is: the middle on a standing sack, one end on the tipped one.
    const [nx, ny] = tipped ? [x + c * rx * 0.9, y + s * rx * 0.9] : [x - light[0] * 0.015, y - light[1] * 0.015];
    // Creases pulled in to the tie.
    for (let i = 0; i < 7; i++) {
      const b = (i / 7) * Math.PI * 2 + rnd() * 0.3;
      const [ex, ey] = [x + (Math.cos(b) * c - Math.sin(b) * s * (ry / rx)) * rx * 0.85, y + (Math.cos(b) * s + Math.sin(b) * c * (ry / rx)) * rx * 0.85];
      if (tipped && (ex - nx) * c + (ey - ny) * s > -0.02) continue;
      g.moveTo(nx, ny).quadraticCurveTo((nx + ex) / 2 + (rnd() - 0.5) * 0.04, (ny + ey) / 2 + (rnd() - 0.5) * 0.04, ex, ey);
    }
    g.stroke({ width: 0.014, color: BURLAP[0], cap: 'round' });
    // The gathered neck: a small rumpled tuft past the cord.
    const tuft = tipped ? [nx + c * 0.06, ny + s * 0.06] : [nx, ny];
    g.poly(patch(tuft[0], tuft[1], 0.05, 0.05, rnd, 0.45, 9)).fill(BURLAP[2]).stroke({ width: 0.014, color: INK, join: 'round' });
    g.circle(nx, ny, 0.028).stroke({ width: 0.018, color: 0x5a3a20 });
    g.moveTo(nx, ny).lineTo(nx + 0.05, ny + 0.06).stroke({ width: 0.012, color: 0x5a3a20, cap: 'round' });
  };
  sack(cx - 0.17, cy - 0.15, 0.18, 0.17, 0.2, false);
  sack(cx + 0.18, cy - 0.12, 0.17, 0.16, -0.3, false);
  g.poly(oval(cx - 0.24, cy - 0.04, 0.05, 0.03, 0.2, 10)).stroke({ width: 0.014, color: 0x5a3a20, alpha: 0.8 });
  sack(cx - 0.06, cy + 0.2, 0.24, 0.14, 0.3, true);
}

/** A fire seen from above at (x, y), about `r` across: an ember bed, then soft glowing ovals heaped on it, deep
 * orange out to a pale yellow core (as in the hearths), a few sparks. */
function fireTop(g: Graphics, x: number, y: number, r: number, rnd: () => number) {
  for (let i = 0; i < 12; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * Math.sqrt(rnd());
    g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, r * (0.1 + 0.08 * rnd())).fill([0x7a2a10, 0xc2410c, 0xf97316][Math.floor(rnd() * 3)]);
  }
  const blobs: [number, number, number][] = [];
  for (let i = 0; i < 3; i++) {
    const a = (i / 3) * Math.PI * 2 + rnd();
    blobs.push([x + Math.cos(a) * r * 0.3, y + Math.sin(a) * r * 0.3, r * (0.5 + 0.15 * rnd())]);
  }
  for (const [k, color] of [
    [1, 0xc2410c],
    [0.74, 0xf28a1e],
    [0.48, 0xffc23a],
    [0.22, 0xfff2c0],
  ] as const) {
    for (const [bx, by, br] of blobs) g.poly(oval(bx, by, br * k, br * k * 0.85, 0.4, 14)).fill(color);
  }
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * (1.1 + 0.5 * rnd());
    g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, 0.01).fill(0xffd27a);
  }
}

/** A soft warm pool of light round (x, y). */
function glowPool(g: Graphics, x: number, y: number, r: number, color = 0xf8b048) {
  for (let i = 0; i < 5; i++) g.circle(x, y, r * (1 - i * 0.16)).fill({ color, alpha: 0.06 });
}

/** Brazier: a wide iron bowl on three clawed legs, a heaped fire of glowing coals in it, a warm glow round it. */
function brazier(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  glowPool(g, cx, cy, 0.62);
  const r = 0.27;
  castShadow(g, ring(cx, cy, r, rnd, 0.005, 16), light, 0.45);
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < 3; i++) {
    const a = ph + (i * Math.PI * 2) / 3;
    const [fx, fy] = [cx + Math.cos(a) * 0.4, cy + Math.sin(a) * 0.4];
    for (const [w, c] of [
      [0.06, INK],
      [0.035, IRON[1]],
    ] as const) {
      g.moveTo(cx + Math.cos(a) * 0.2, cy + Math.sin(a) * 0.2).lineTo(fx, fy).stroke({ width: w, color: c, cap: 'round' });
    }
    for (const b of [-0.5, 0, 0.5]) g.moveTo(fx, fy).lineTo(fx + Math.cos(a + b) * 0.05, fy + Math.sin(a + b) * 0.05);
    g.stroke({ width: 0.018, color: IRON[0], cap: 'round' });
  }
  g.circle(cx, cy, r).fill(IRON[0]).stroke({ width: OUT, color: INK });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la - 1) * r * 0.9, cy + Math.sin(la - 1) * r * 0.9).arc(cx, cy, r * 0.9, la - 1, la + 1).stroke({ width: 0.025, color: IRON[2] });
  g.circle(cx, cy, r * 0.8).fill(0x2a1a12).stroke({ width: 0.014, color: INK });
  fireTop(g, cx, cy, r * 0.7, rnd);
}

/** Candelabrum, from a high angle like the urn and lantern (from straight above it is only a star of arms): a tall
 * iron stand on three clawed feet, five branches rising from it in pairs of curves round a middle one, a cream
 * candle with a flame in each cup, wax dripped on the floor, a soft glow. */
function candelabrum(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt } = screenAxes(light);
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  glowPool(g, ...P(0, 0.1), 0.55, 0xfcd36a);
  castShadow(g, ring(...P(0, -0.4), 0.08, rnd, 0, 8), light, 1.1);
  for (let i = 0; i < 6; i++) g.poly(oval(...P((rnd() - 0.5) * 0.5, -0.42 + (rnd() - 0.5) * 0.08), 0.025, 0.016, 0, 8)).fill(0xf0e8d0).stroke({ width: 0.006, color: 0xa89878 });
  const iron = (pts: [number, number][], w: number) => {
    for (const [ww, c] of [
      [w + 0.022, INK],
      [w, IRON[1]],
    ] as const) {
      g.moveTo(...pts[0]);
      if (pts.length === 3) g.quadraticCurveTo(...pts[1], ...pts[2]);
      else g.lineTo(...pts[1]);
      g.stroke({ width: ww, color: c, cap: 'round', join: 'round' });
    }
  };
  // Feet, the column, the branches.
  for (const k of [-1, 0, 1]) iron([P(0, -0.33), P(k * 0.14, k === 0 ? -0.44 : -0.42)], 0.03);
  iron([P(0, -0.42), P(0, 0.2)], 0.035);
  for (const k of [-1, 1]) {
    iron([P(0, -0.02), P(k * 0.3, -0.04), P(k * 0.3, 0.14)], 0.026);
    iron([P(0, 0.06), P(k * 0.16, 0.05), P(k * 0.16, 0.2)], 0.026);
  }
  g.poly(oval(...P(0, -0.32), 0.05, 0.035, 0, 10)).fill(IRON[1]).stroke({ width: 0.014, color: INK });
  // Cups, candles, flames.
  for (const [a, b] of [[-0.3, 0.14], [-0.16, 0.2], [0, 0.22], [0.16, 0.2], [0.3, 0.14]] as [number, number][]) {
    g.poly([...P(a - 0.05, b), ...P(a + 0.05, b), ...P(a + 0.035, b - 0.03), ...P(a - 0.035, b - 0.03)]).fill(IRON[1]).stroke({ width: 0.012, color: INK, join: 'round' });
    const candle = [...P(a - 0.025, b + 0.1), ...P(a + 0.025, b + 0.1), ...P(a + 0.025, b), ...P(a - 0.025, b)];
    g.poly(candle).fill(0xf4ecd8).stroke({ width: 0.012, color: INK, join: 'round' });
    g.poly([...P(a - 0.025, b + 0.1), ...P(a - 0.008, b + 0.1), ...P(a - 0.008, b), ...P(a - 0.025, b)]).fill(0xfffaf0);
    g.circle(...P(a, b + 0.15), 0.05).fill({ color: 0xffc23a, alpha: 0.25 });
    g.poly(oval(...P(a, b + 0.135), 0.018, 0.03, Math.atan2(P(0, 1)[1] - P(0, 0)[1], P(0, 1)[0] - P(0, 0)[0]), 10)).fill(0xf9a23a);
    g.poly(oval(...P(a, b + 0.13), 0.009, 0.016, Math.atan2(P(0, 1)[1] - P(0, 0)[1], P(0, 1)[0] - P(0, 0)[0]), 8)).fill(0xfff2c0);
  }
}

/** Wall shackles: two bolted iron plates on the wall, each with a ring, a heavy chain hanging from it and lying on
 * the floor, ending in a thick open cuff with its hinge. */
function shackles(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  for (const u of [0.26, 0.74]) {
    // The plate and ring.
    const plate = [...at(u - 0.08, 0), ...at(u + 0.08, 0), ...at(u + 0.08, 0.1), ...at(u - 0.08, 0.1)];
    g.poly(plate).fill(IRON[0]).stroke({ width: 0.018, color: INK, join: 'round' });
    for (const k of [-0.05, 0.05]) g.circle(...at(u + k, 0.03), 0.012).fill(IRON[2]);
    const [rx, ry] = at(u, 0.12);
    g.circle(rx, ry, 0.045).stroke({ width: 0.045, color: INK });
    g.circle(rx, ry, 0.045).stroke({ width: 0.024, color: IRON[1] });
    // The chain.
    const end = at(u + (u < 0.5 ? -0.06 : 0.06) + (rnd() - 0.5) * 0.05, 0.66 + 0.08 * rnd());
    const mid = at(u + (u < 0.5 ? 0.07 : -0.07), 0.42);
    const B = (t: number): [number, number] => [(1 - t) * (1 - t) * rx + 2 * (1 - t) * t * mid[0] + t * t * end[0], (1 - t) * (1 - t) * ry + 2 * (1 - t) * t * mid[1] + t * t * end[1]];
    const n = 7;
    castShadow(g, [...B(0), ...B(0.5), ...B(1)], light, 0.03);
    for (let i = 1; i <= n; i++) {
      const [p0, p1] = [B(i / n), B((i - 1) / n)];
      const a = Math.atan2(p0[1] - p1[1], p0[0] - p1[0]);
      const o = oval((p0[0] + p1[0]) / 2, (p0[1] + p1[1]) / 2, 0.05, i % 2 ? 0.03 : 0.012, a, 12);
      g.poly(o).stroke({ width: 0.036, color: INK });
      g.poly(o).stroke({ width: 0.018, color: IRON[1] });
      g.poly(o.map((q, k) => q + (k % 2 ? light[1] : light[0]) * 0.006)).stroke({ width: 0.006, color: IRON[2] });
    }
    // The cuff: a thick band, open, its hinge knob.
    const [ex, ey] = end;
    const a0 = Math.atan2(ey - mid[1], ex - mid[0]);
    for (const [w, c] of [
      [0.06, INK],
      [0.036, IRON[1]],
    ] as const) {
      g.moveTo(ex + Math.cos(a0 + 0.5) * 0.085, ey + Math.sin(a0 + 0.5) * 0.085).arc(ex, ey, 0.085, a0 + 0.5, a0 + 0.5 + Math.PI * 1.55).stroke({ width: w, color: c, cap: 'round' });
    }
    g.moveTo(ex + Math.cos(a0 + 1.6) * 0.085, ey + Math.sin(a0 + 1.6) * 0.085).arc(ex, ey, 0.085, a0 + 1.6, a0 + 2.6).stroke({ width: 0.01, color: IRON[2] });
    const [kx, ky] = [ex + Math.cos(a0 + Math.PI) * 0.085, ey + Math.sin(a0 + Math.PI) * 0.085];
    g.circle(kx, ky, 0.025).fill(IRON[0]).stroke({ width: 0.012, color: INK });
  }
}

/** Torch sconce: an iron bracket on the wall holding a torch angled out into the room, its head burning (glowing
 * ovals), a pool of light on the floor. */
function torchSconce(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  void light;
  const [hx, hy] = at(0.5, 0.42);
  glowPool(g, hx, hy, 0.62);
  g.poly([...at(0.38, 0), ...at(0.62, 0), ...at(0.58, 0.08), ...at(0.42, 0.08)]).fill(IRON[0]).stroke({ width: 0.016, color: INK, join: 'round' });
  for (const [w, c] of [
    [0.05, INK],
    [0.028, IRON[1]],
  ] as const) {
    g.moveTo(...at(0.5, 0.06)).lineTo(...at(0.5, 0.2)).stroke({ width: w, color: c, cap: 'round' });
  }
  g.poly(oval(...at(0.5, 0.2), 0.05, 0.04, 0, 10)).stroke({ width: 0.03, color: INK });
  g.poly(oval(...at(0.5, 0.2), 0.05, 0.04, 0, 10)).stroke({ width: 0.016, color: IRON[1] });
  for (const [w, c] of [
    [0.065, INK],
    [0.04, WOOD[1]],
  ] as const) {
    g.moveTo(...at(0.5, 0.12)).lineTo(...at(0.5, 0.36)).stroke({ width: w, color: c, cap: 'round' });
  }
  g.circle(hx, hy, 0.065).fill(0x3a2a1c).stroke({ width: 0.014, color: INK });
  fireTop(g, hx, hy, 0.09, rnd);
}

/** Tattered banner: hung from a pole along the wall and drawn falling out into the room — a long cloth with a gilt
 * emblem, a fringe at the top, its swallowtail end ragged and torn. */
function tatteredBanner(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const cloth = [
    [0x6a1e1a, 0x8e2a22, 0xb03a2e],
    [0x1e2e5a, 0x2a4078, 0x3e5898],
    [0x2a4a22, 0x3a6430, 0x507e42],
    [0x3a2a4a, 0x523c66, 0x6c5482],
  ][v % 4];
  const pts: number[] = [...at(0.22, 0.06), ...at(0.78, 0.06), ...at(0.79, 0.62)];
  // A ragged swallowtail edge.
  for (const [u, s] of [[0.72, 0.7], [0.66, 0.62], [0.6, 0.72], [0.54, 0.6], [0.5, 0.52], [0.46, 0.6], [0.4, 0.7], [0.34, 0.6], [0.28, 0.72]]) pts.push(...at(u + (rnd() - 0.5) * 0.03, s + (rnd() - 0.5) * 0.04));
  pts.push(...at(0.21, 0.62));
  castShadow(g, pts, light, 0.04);
  g.poly(pts).fill(cloth[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...at(0.24, 0.08), ...at(0.4, 0.08), ...at(0.38, 0.6), ...at(0.25, 0.6)]).fill(cloth[2]);
  g.poly([...at(0.62, 0.08), ...at(0.76, 0.08), ...at(0.77, 0.58), ...at(0.64, 0.6)]).fill(cloth[0]);
  // A tear, a gilt border, the emblem.
  g.moveTo(...at(0.66, 0.66)).lineTo(...at(0.62, 0.5)).stroke({ width: 0.014, color: INK });
  g.poly([...at(0.26, 0.1), ...at(0.74, 0.1), ...at(0.75, 0.56), ...at(0.25, 0.56)]).stroke({ width: 0.014, color: GILT[1], alpha: 0.8 });
  const [ex, ey] = at(0.5, 0.3);
  g.poly([ex, ey - 0.09, ex + 0.08, ey - 0.03, ex + 0.06, ey + 0.07, ex, ey + 0.1, ex - 0.06, ey + 0.07, ex - 0.08, ey - 0.03]).fill(GILT[1]).stroke({ width: 0.014, color: INK });
  g.poly([ex, ey - 0.05, ex + 0.03, ey + 0.02, ex, ey + 0.05, ex - 0.03, ey + 0.02]).fill(cloth[0]);
  // The pole and its finials.
  for (const [w, c] of [
    [0.06, INK],
    [0.035, DARK_WOOD[1]],
  ] as const) {
    g.moveTo(...at(0.12, 0.06)).lineTo(...at(0.88, 0.06)).stroke({ width: w, color: c, cap: 'round' });
  }
  for (const u of [0.12, 0.88]) g.circle(...at(u, 0.06), 0.035).fill(GILT[1]).stroke({ width: 0.012, color: INK });
  for (let i = 0; i < 9; i++) g.moveTo(...at(0.24 + i * 0.065, 0.09)).lineTo(...at(0.24 + i * 0.065, 0.13));
  g.stroke({ width: 0.012, color: GILT[2] });
}

/** Iron maiden, from a high angle like the headstones (upright things only read from the side), big: a riveted,
 * iron-banded shell shaped like a person — a domed head with a sculpted grim face, shoulders, a taper to the foot —
 * its two doors swung wide, rows of bright spikes on their insides and in the dark hollow, dried blood on them and
 * pooled below. */
function ironMaiden(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt } = screenAxes(light);
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  const BLOOD = 0x5a1410;
  castShadow(g, ring(...P(0, -0.44), 0.2, rnd, 0, 12), light, 1.2);
  g.poly(patch(...P(0.04, -0.46), 0.12, 0.05, rnd, 0.3, 10)).fill({ color: BLOOD, alpha: 0.8 });
  // The shell: half widths from the foot to the top of the head.
  const prof: [number, number][] = [[-0.46, 0.17], [-0.2, 0.2], [0.12, 0.22], [0.2, 0.2], [0.26, 0.12], [0.34, 0.12], [0.44, 0.08], [0.48, 0]];
  const outline: number[] = [];
  for (const [b, w] of prof) outline.push(...P(w, b));
  for (const [b, w] of [...prof].reverse()) outline.push(...P(-w, b));
  // The doors first (behind the shell's edge), hinged at its sides, swung out wide.
  const spike = (a: number, b: number, dir: number) => {
    const [tx, ty] = P(a + dir * 0.045, b);
    const [ux, uy] = P(a, b + 0.018);
    const [vx, vy] = P(a, b - 0.018);
    g.poly([ux, uy, tx, ty, vx, vy]).fill(0xe8e4dc).stroke({ width: 0.006, color: INK });
  };
  for (const k of [-1, 1]) {
    const door = [...P(k * 0.2, 0.18), ...P(k * 0.44, 0.12), ...P(k * 0.43, -0.36), ...P(k * 0.18, -0.44)];
    g.poly(door).fill(IRON[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly([...P(k * 0.22, 0.15), ...P(k * 0.41, 0.1), ...P(k * 0.4, -0.34), ...P(k * 0.2, -0.41)]).fill(k < 0 ? IRON[2] : IRON[1]);
    for (let r = 0; r < 5; r++) {
      for (const t of [0.3, 0.7]) {
        const a = k * (0.22 + 0.19 * t);
        const b = 0.08 - r * 0.1 - t * 0.04;
        spike(a, b, -k);
        if ((r + (t > 0.5 ? 1 : 0)) % 3 === 0) g.circle(...P(a - k * 0.02, b), 0.012).fill({ color: BLOOD, alpha: 0.85 });
      }
    }
    for (const b of [0.14, -0.38]) g.poly([...P(k * 0.17, b - 0.03), ...P(k * 0.23, b - 0.03), ...P(k * 0.23, b + 0.03), ...P(k * 0.17, b + 0.03)]).fill(IRON[0]).stroke({ width: 0.01, color: INK });
  }
  g.poly(outline).fill(IRON[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(outline.map((q, i) => (i % 2 ? cy : cx) + (q - (i % 2 ? cy : cx)) * 0.9 + (i % 2 ? light[1] : light[0]) * 0.012)).fill(IRON[1]);
  // The hollow, dark, spikes catching the light, blood.
  const hollow = [...P(-0.14, 0.14), ...P(0.14, 0.14), ...P(0.15, -0.38), ...P(-0.15, -0.38)];
  g.poly(hollow).fill(0x120c0a).stroke({ width: 0.016, color: INK });
  for (let r = 0; r < 5; r++) for (const a of [-0.07, 0, 0.07]) {
    const [sx, sy] = P(a + (r % 2 ? 0.035 : 0), 0.08 - r * 0.1);
    g.poly([sx, sy - 0.014, sx + 0.012, sy + 0.008, sx - 0.012, sy + 0.008]).fill(r % 2 ? IRON[2] : 0xc8c4bc);
  }
  g.poly(patch(...P(0.02, -0.3), 0.05, 0.03, rnd, 0.4, 8)).fill({ color: BLOOD, alpha: 0.8 });
  // Bands and rivets on the shell's rim.
  for (const b of [0.18, -0.12, -0.42]) {
    const w = 0.2;
    g.moveTo(...P(-w, b)).lineTo(...P(-0.15, b)).moveTo(...P(0.15, b)).lineTo(...P(w, b));
    g.stroke({ width: 0.02, color: INK });
    for (const a of [-0.18, 0.18]) g.circle(...P(a, b), 0.01).fill(IRON[2]);
  }
  // The head and its sculpted face.
  g.poly(oval(...P(0, 0.34), 0.1, 0.11, 0, 16)).fill(IRON[1]).stroke({ width: 0.016, color: INK });
  g.poly(oval(...P(-0.02, 0.36), 0.05, 0.05, 0, 10)).fill({ color: IRON[2], alpha: 0.6 });
  for (const a of [-0.04, 0.04]) {
    g.poly(oval(...P(a, 0.36), 0.022, 0.012, 0, 8)).fill(INK);
    g.moveTo(...P(a - 0.03, 0.39)).lineTo(...P(a + 0.02, 0.38)).stroke({ width: 0.01, color: INK });
  }
  g.poly([...P(0, 0.35), ...P(0.015, 0.31), ...P(-0.015, 0.31)]).fill(IRON[0]);
  g.moveTo(...P(-0.035, 0.28)).quadraticCurveTo(...P(0, 0.295), ...P(0.035, 0.28)).stroke({ width: 0.012, color: INK });
}

/** Stocks, from a high angle: two posts holding a heavy board split along its middle, a head hole between two
 * wrist holes, a hinge at one end and a padlock at the other, straw on the ground before it. */
function stocks(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const { pt } = screenAxes(light);
  const L = Math.max(f.w, f.h) * 0.42;
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  for (let i = 0; i < 12; i++) {
    const [x, y] = P((rnd() - 0.5) * L * 1.6, -0.3 - rnd() * 0.12);
    g.moveTo(x, y).lineTo(x + (rnd() - 0.5) * 0.08, y + (rnd() - 0.5) * 0.04).stroke({ width: 0.012, color: 0xc8a860 });
  }
  castShadow(g, [...P(-L, -0.32), ...P(L, -0.32), ...P(L, -0.2), ...P(-L, -0.2)], light, 0.6);
  for (const a of [-L * 0.92, L * 0.92]) {
    const post = [...P(a - 0.06, 0.28), ...P(a + 0.06, 0.28), ...P(a + 0.06, -0.32), ...P(a - 0.06, -0.32)];
    g.poly(post).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly([...P(a - 0.04, 0.26), ...P(a, 0.26), ...P(a, -0.3), ...P(a - 0.04, -0.3)]).fill(WOOD[1]);
  }
  const board = [...P(-L, 0.18), ...P(L, 0.18), ...P(L, -0.14), ...P(-L, -0.14)];
  g.poly(board).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...P(-L + 0.02, 0.16), ...P(L - 0.02, 0.16), ...P(L - 0.02, 0.09), ...P(-L + 0.02, 0.09)]).fill(WOOD[2]);
  g.moveTo(...P(-L, 0.02)).lineTo(...P(L, 0.02)).stroke({ width: 0.018, color: INK });
  for (const [a, r] of [
    [0, 0.08],
    [-L * 0.5, 0.045],
    [L * 0.5, 0.045],
  ]) {
    g.poly(oval(...P(a, 0.02), r, r, 0, 14)).fill(0x1e1712).stroke({ width: 0.014, color: INK });
  }
  g.poly([...P(L - 0.06, 0.06), ...P(L + 0.02, 0.06), ...P(L + 0.02, -0.04), ...P(L - 0.06, -0.04)]).fill(IRON[1]).stroke({ width: 0.012, color: INK });
  g.poly(oval(...P(L - 0.02, 0.09), 0.025, 0.025, 0, 8)).stroke({ width: 0.012, color: IRON[2] });
  g.poly([...P(-L, 0.06), ...P(-L + 0.06, 0.06), ...P(-L + 0.06, -0.02), ...P(-L, -0.02)]).fill(IRON[0]);
}

/** Treasure hoard: a heap of gold coins spilling over the floor, a goblet and a crown on it, gems (ruby, sapphire,
 * emerald) and a sword's hilt sticking out, glints. */
function treasureHoard(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  glowPool(g, cx, cy, 0.5, 0xf8d860);
  castShadow(g, patch(cx, cy, 0.3, 0.28, rnd, 0.2, 12), light, 0.2);
  const coin = (x: number, y: number, r: number) => {
    g.circle(x, y, r).fill(GILT[0]).stroke({ width: 0.008, color: 0x5a3e10 });
    g.circle(x + light[0] * r * 0.15, y + light[1] * r * 0.15, r * 0.75).fill(GILT[1]);
    g.circle(x, y, r * 0.45).stroke({ width: 0.005, color: GILT[0] });
  };
  // The heap: a mound under the coins, loose coins round it, then coins piling toward the top.
  g.poly(patch(cx, cy, 0.33, 0.3, rnd, 0.2, 18)).fill(GILT[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + light[0] * 0.04, cy + light[1] * 0.04, 0.25, 0.22, rnd, 0.2, 14)).fill(GILT[1]);
  for (let i = 0; i < 12; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.34 + 0.1 * rnd();
    coin(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.028);
  }
  for (let i = 0; i < 26; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.28 * Math.sqrt(rnd());
    coin(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.032);
  }
  // A sword hilt sticking out.
  const sa = rnd() * Math.PI * 2;
  const [hx, hy] = [cx + Math.cos(sa) * 0.12, cy + Math.sin(sa) * 0.12];
  for (const [w, c] of [
    [0.045, INK],
    [0.025, 0x5a3a20],
  ] as const) {
    g.moveTo(hx, hy).lineTo(hx + Math.cos(sa) * 0.16, hy + Math.sin(sa) * 0.16).stroke({ width: w, color: c, cap: 'round' });
  }
  g.moveTo(hx - Math.sin(sa) * 0.08, hy + Math.cos(sa) * 0.08).lineTo(hx + Math.sin(sa) * 0.08, hy - Math.cos(sa) * 0.08).stroke({ width: 0.035, color: INK, cap: 'round' });
  g.moveTo(hx - Math.sin(sa) * 0.08, hy + Math.cos(sa) * 0.08).lineTo(hx + Math.sin(sa) * 0.08, hy - Math.cos(sa) * 0.08).stroke({ width: 0.018, color: GILT[2], cap: 'round' });
  g.circle(hx + Math.cos(sa) * 0.17, hy + Math.sin(sa) * 0.17, 0.025).fill(GILT[1]).stroke({ width: 0.01, color: INK });
  // A goblet (from above: a cup on its foot) and a crown.
  const [gx, gy] = [cx - 0.12, cy + 0.1];
  g.circle(gx, gy, 0.07).fill(GILT[1]).stroke({ width: 0.014, color: INK });
  g.circle(gx, gy, 0.05).fill(0x6a1e1a);
  const [kx, ky] = [cx + 0.1, cy - 0.12];
  g.circle(kx, ky, 0.08).stroke({ width: 0.04, color: INK });
  g.circle(kx, ky, 0.08).stroke({ width: 0.024, color: GILT[2] });
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    g.poly([kx + Math.cos(a - 0.15) * 0.09, ky + Math.sin(a - 0.15) * 0.09, kx + Math.cos(a) * 0.13, ky + Math.sin(a) * 0.13, kx + Math.cos(a + 0.15) * 0.09, ky + Math.sin(a + 0.15) * 0.09]).fill(GILT[2]).stroke({ width: 0.008, color: INK });
  }
  g.circle(kx, ky - 0.08, 0.016).fill(0xc02030);
  // Gems and glints.
  for (const col of [0xc02030, 0x2050c0, 0x20a060, 0xc02030, 0x8040c0]) {
    const [x, y] = [cx + (rnd() - 0.5) * 0.5, cy + (rnd() - 0.5) * 0.45];
    g.poly([x, y - 0.03, x + 0.025, y, x, y + 0.03, x - 0.025, y]).fill(col).stroke({ width: 0.008, color: INK });
    g.circle(x - 0.006, y - 0.01, 0.007).fill(0xffffff);
  }
  for (let i = 0; i < 5; i++) {
    const [x, y] = [cx + (rnd() - 0.5) * 0.5, cy + (rnd() - 0.5) * 0.5];
    g.poly([x, y - 0.035, x + 0.008, y, x, y + 0.035, x - 0.008, y]).fill(0xffffff);
    g.poly([x - 0.035, y, x, y - 0.008, x + 0.035, y, x, y + 0.008]).fill(0xffffff);
  }
}

/** Dais and throne: a stone platform raised two steps, the lower step showing round its front and sides, a red runner
 * up the steps to a throne set at the back against the wall. */
function daisThrone(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { len, rect, at } = frame(f, back);
  const wid = Math.min(f.w, f.h);
  castShadow(g, quad(rect(0.04, 0.02, len - 0.04, wid - 0.04), rnd, 0), light, 0.25);
  board(g, rect(0.04, 0.02, len - 0.04, wid - 0.04), STONE.map((c) => shade(c, 0.85)), light, rnd, true, 0.05, 0);
  const top = board(g, rect(0.24, 0.04, len - 0.24, wid - 0.34), STONE, light, rnd, true, 0.05, 0);
  masonry(g, top, STONE, rnd, 0.4);
  // The runner, down from the throne over both steps.
  const run = quad(rect(len / 2 - 0.26, 0.7, len / 2 + 0.26, wid - 0.04), rnd, 0.004);
  g.poly(run).fill(0x7a2420).stroke({ width: 0.016, color: INK });
  g.poly(quad(rect(len / 2 - 0.2, 0.72, len / 2 + 0.2, wid - 0.06), rnd, 0.003)).stroke({ width: 0.014, color: GILT[1] });
  g.moveTo(...at(len / 2 - 0.26, wid - 0.34)).lineTo(...at(len / 2 + 0.26, wid - 0.34)).stroke({ width: 0.014, color: 0x4a1410 });
  // The throne, on the top at the back.
  const [tx, ty] = at(len / 2 - 0.5, 0);
  const [ux, uy] = at(len / 2 + 0.5, 1);
  const sub: InteriorItem = { ...f, kind: 'throne', x: Math.min(tx, ux), y: Math.min(ty, uy), w: 1, h: 1 };
  throne(g, sub, light, rnd, back);
}

/** Dry fountain: a broad eight-sided stone basin, dry — its floor cracked and dusty, dead leaves blown in, a green
 * stain where the water stood — and in the middle a tiered pedestal: a chipped lower bowl, a column rising from it
 * to a small upper bowl, a finial on top, its shadow across the basin. */
function dryFountain(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const R = Math.min(f.w, f.h) * 0.46;
  const oct = (r: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 8; i++) pts.push(cx + Math.cos(Math.PI / 8 + (i * Math.PI) / 4) * r, cy + Math.sin(Math.PI / 8 + (i * Math.PI) / 4) * r);
    return pts;
  };
  castShadow(g, oct(R), light, 0.2);
  g.poly(oct(R)).fill(MARBLE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(oct(R * 0.97).map((q, i) => q + (i % 2 ? light[1] : light[0]) * 0.02)).fill(MARBLE[1]);
  g.poly(oct(R * 0.82)).fill(0x8a7e6a).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(oct(R * 0.8).map((q, i) => (i % 2 ? cy : cx) + (q - (i % 2 ? cy : cx)) * 0.98 - (i % 2 ? light[1] : light[0]) * 0.04)).fill({ color: 0x000000, alpha: 0.2 });
  g.poly(patch(cx + 0.1, cy + 0.15, R * 0.5, R * 0.3, rnd, 0.3, 14)).fill({ color: 0x5a6a3a, alpha: 0.35 });
  // Cracks.
  for (let c = 0; c < 4; c++) {
    let a = rnd() * Math.PI * 2;
    let [x, y] = [cx + Math.cos(a) * R * 0.3, cy + Math.sin(a) * R * 0.3];
    g.moveTo(x, y);
    for (let k = 0; k < 4; k++) {
      a += (rnd() - 0.5) * 1.2;
      x += Math.cos(a) * R * 0.11;
      y += Math.sin(a) * R * 0.11;
      g.lineTo(x, y);
    }
  }
  g.stroke({ width: 0.012, color: 0x4a4234, join: 'miter' });
  // Dead leaves.
  for (let i = 0; i < 8; i++) {
    const a = rnd() * Math.PI * 2;
    const d = R * (0.35 + 0.4 * rnd());
    g.poly(oval(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.04, 0.02, rnd() * 3, 8)).fill([0x8a5a22, 0x6a4a1a, 0xa86a2a][i % 3]).stroke({ width: 0.008, color: INK });
  }
  // The tiered pedestal: its shadow, the lower bowl (dry, chipped), the upper bowl on its column, the finial.
  castShadow(g, ring(cx, cy, R * 0.18, rnd, 0, 12), light, 0.6);
  g.circle(cx, cy, R * 0.34).fill(MARBLE[0]).stroke({ width: OUT, color: INK });
  g.circle(cx + light[0] * 0.02, cy + light[1] * 0.02, R * 0.3).fill(MARBLE[1]);
  g.circle(cx, cy, R * 0.25).fill(0x9a8e78).stroke({ width: 0.014, color: INK });
  const ca = rnd() * Math.PI * 2;
  g.poly([cx + Math.cos(ca) * R * 0.34, cy + Math.sin(ca) * R * 0.34, cx + Math.cos(ca + 0.25) * R * 0.27, cy + Math.sin(ca + 0.25) * R * 0.27, cx + Math.cos(ca + 0.5) * R * 0.34, cy + Math.sin(ca + 0.5) * R * 0.34]).fill(0x9a8e78).stroke({ width: 0.01, color: INK });
  g.poly(oval(cx - light[0] * R * 0.06, cy - light[1] * R * 0.06, R * 0.12, R * 0.12, 0, 12)).fill({ color: 0x000000, alpha: 0.3 });
  g.circle(cx, cy, R * 0.17).fill(MARBLE[1]).stroke({ width: OUT, color: INK });
  g.circle(cx + light[0] * 0.015, cy + light[1] * 0.015, R * 0.13).fill(MARBLE[2]);
  g.circle(cx, cy, R * 0.11).fill(0x9a8e78).stroke({ width: 0.012, color: INK });
  g.circle(cx, cy, R * 0.07).fill(MARBLE[1]).stroke({ width: 0.014, color: INK });
  g.circle(cx + light[0] * R * 0.025, cy + light[1] * R * 0.025, R * 0.03).fill(0xffffff);
}

/** Ritual circle: drawn on the floor in chalk, blood or something that glows — a double ring with runes between
 * the rings, a seven-pointed star inside, candle stubs at its points, smudges where feet have scuffed it. */
function ritualCircle(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  void light;
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const R = Math.min(f.w, f.h) * 0.44;
  const [ink, glow] = [
    [0xe8e4dc, 0],
    [0x8a1a14, 0],
    [0xb89aff, 0x8a5aff],
    [0xe8e4dc, 0],
  ][v % 4];
  if (glow) for (let i = 0; i < 4; i++) g.circle(cx, cy, R * (1.15 - i * 0.12)).fill({ color: glow, alpha: 0.06 });
  const wob = (r: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 48; i++) {
      const a = (i / 48) * Math.PI * 2;
      const rr = r * (1 + (rnd() - 0.5) * 0.015);
      pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    return pts;
  };
  g.poly(wob(R)).stroke({ width: 0.035, color: ink, alpha: 0.9 });
  g.poly(wob(R * 0.82)).stroke({ width: 0.025, color: ink, alpha: 0.9 });
  // Runes between the rings.
  for (let i = 0; i < 14; i++) {
    const a = (i / 14) * Math.PI * 2;
    const [x, y] = [cx + Math.cos(a) * R * 0.91, cy + Math.sin(a) * R * 0.91];
    const [tx, ty] = [-Math.sin(a), Math.cos(a)];
    const [nx, ny] = [Math.cos(a), Math.sin(a)];
    const k = Math.floor(rnd() * 4);
    const s = R * 0.05;
    g.moveTo(x - nx * s, y - ny * s).lineTo(x + nx * s, y + ny * s);
    if (k === 0) g.moveTo(x - nx * s, y - ny * s).lineTo(x + tx * s, y + ty * s);
    if (k === 1) g.moveTo(x + tx * s * 0.8 - nx * s * 0.3, y + ty * s * 0.8 - ny * s * 0.3).lineTo(x - tx * s * 0.8 + nx * s * 0.3, y - ty * s * 0.8 + ny * s * 0.3);
    if (k === 2) g.moveTo(x + nx * s, y + ny * s).lineTo(x + tx * s, y + ty * s).lineTo(x - nx * s * 0.2, y - ny * s * 0.2);
    if (k === 3) g.moveTo(x - tx * s, y - ty * s).lineTo(x + tx * s, y + ty * s);
  }
  g.stroke({ width: 0.016, color: ink, alpha: 0.9, cap: 'round', join: 'round' });
  // The star.
  const ph = -Math.PI / 2;
  const pts: [number, number][] = [];
  for (let i = 0; i < 7; i++) pts.push([cx + Math.cos(ph + (i * Math.PI * 2) / 7) * R * 0.8, cy + Math.sin(ph + (i * Math.PI * 2) / 7) * R * 0.8]);
  g.moveTo(...pts[0]);
  for (let i = 1; i <= 7; i++) g.lineTo(...pts[(i * 3) % 7]);
  g.stroke({ width: 0.022, color: ink, alpha: 0.85, join: 'miter' });
  g.poly(wob(R * 0.18)).stroke({ width: 0.02, color: ink, alpha: 0.85 });
  // Scuffs.
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    g.poly(patch(cx + Math.cos(a) * R * 0.85, cy + Math.sin(a) * R * 0.85, 0.07, 0.05, rnd, 0.3, 8)).fill({ color: 0x5a554c, alpha: 0.5 });
  }
  // Candle stubs at the points.
  for (const [x, y] of pts) {
    g.circle(x, y, 0.07).fill({ color: 0xffc23a, alpha: 0.15 });
    g.circle(x, y, 0.035).fill(0xf0e8d0).stroke({ width: 0.012, color: INK });
    g.circle(x, y, 0.015).fill(0xffc23a);
  }
}

/** Underground props drawn here (InteriorLayer: ways in and out, hazards): one of four looks by position;
 * those against the rock wall take its side. */
export const UNDER_DRAWN = new Set([
  'exit', 'up', 'down', 'tunnel', 'ladder', 'pipe', 'well', 'pit', 'trap', 'cave_in',
  'stalagmite', 'rock_column', 'boulder', 'crystal', 'fungus', 'mushroom', 'web', 'cobweb', 'guano', 'moss', 'pool', 'nest', 'rat_nest', 'ice', 'ice_sheet',
  'obsidian', 'basalt', 'vent', 'sulfur', 'scorched', 'glass_pool',
  'bones', 'skeleton', 'skulls', 'coffin', 'effigy', 'niche', 'urn', 'offering',
  'campfire', 'rubble', 'debris', 'timber', 'rail', 'ore_cart', 'ore_vein', 'tools', 'lantern', 'powder', 'bedroll', 'sacks',
  'brazier', 'candles', 'chains', 'sconce', 'banner', 'iron_maiden', 'stocks', 'hoard', 'dais', 'fountain', 'glyph',
]);

/** Draws underground prop `f` if it is drawn here; false otherwise. `wall` is the rock side for props
 * against a wall, and the back for those that have one (the dais). */
export function drawUnderProp(g: Graphics, f: InteriorItem, light: Light, v: number, wall: [number, number]): boolean {
  if (!UNDER_DRAWN.has(f.kind)) return false;
  const rnd = rng(v * 7919 + f.w * 131 + f.h * 17 + f.kind.length * 1009 + f.kind.charCodeAt(0) * 3);
  const w: [number, number] = wall[0] || wall[1] ? wall : [0, -1];
  switch (f.kind) {
    case 'exit':
      stairsUp(g, f, rnd, true);
      break;
    case 'up':
      stairsUp(g, f, rnd, false);
      break;
    case 'down':
      linkDown(g, f, rnd);
      break;
    case 'tunnel':
      tunnel(g, f, rnd, w);
      break;
    case 'ladder':
      ladder(g, f, rnd, w);
      break;
    case 'pipe':
      pipe(g, f, rnd, w);
      break;
    case 'well':
      well(g, f, light, rnd);
      break;
    case 'pit':
      pit(g, f, light, rnd);
      break;
    case 'trap':
      trap(g, f, rnd);
      break;
    case 'cave_in':
      caveIn(g, f, rnd);
      break;
    case 'stalagmite':
      stalagmite(g, f, light, rnd);
      break;
    case 'rock_column':
      rockColumn(g, f, light, rnd);
      break;
    case 'boulder':
      caveBoulder(g, f, light, rnd);
      break;
    case 'crystal':
      crystal(g, f, light, v, rnd);
      break;
    case 'fungus':
      fungus(g, f, light, v, rnd);
      break;
    case 'mushroom':
      giantMushrooms(g, f, light, v, rnd);
      break;
    case 'web':
      web(g, f, rnd);
      break;
    case 'cobweb':
      cobweb(g, f, rnd, w);
      break;
    case 'guano':
      guano(g, f, rnd);
      break;
    case 'moss':
      caveMoss(g, f, light, rnd);
      break;
    case 'pool':
      cavePool(g, f, light, v, rnd);
      break;
    case 'nest':
      nestHeap(g, f, light, rnd, true);
      break;
    case 'rat_nest':
      nestHeap(g, f, light, rnd, false);
      break;
    case 'ice':
      iceFormation(g, f, light, rnd);
      break;
    case 'ice_sheet':
      iceSheet(g, f, light, rnd);
      break;
    case 'obsidian':
      obsidianShards(g, f, light, rnd);
      break;
    case 'basalt':
      basaltColumns(g, f, light, rnd);
      break;
    case 'vent':
      steamVent(g, f, light, rnd);
      break;
    case 'sulfur':
      sulfurCrust(g, f, light, rnd);
      break;
    case 'scorched':
      scorchedRemains(g, f, light, rnd);
      break;
    case 'glass_pool':
      glassPool(g, f, light, rnd);
      break;
    case 'bones':
      scatteredBones(g, f, light, rnd);
      break;
    case 'skeleton':
      skeletonRemains(g, f, light, rnd);
      break;
    case 'skulls':
      skullPile(g, f, light, rnd);
      break;
    case 'coffin':
      openCoffin(g, f, light, rnd);
      break;
    case 'effigy':
      stoneEffigy(g, f, light, rnd);
      break;
    case 'niche':
      burialNiche(g, f, light, rnd, w);
      break;
    case 'urn':
      burialUrn(g, f, light, v, rnd);
      break;
    case 'offering':
      offeringBowl(g, f, light, v, rnd);
      break;
    case 'campfire':
      coldCampfire(g, f, light, rnd);
      break;
    case 'rubble':
      rubbleHeap(g, f, light, rnd);
      break;
    case 'debris':
      flotsamHeap(g, f, light, rnd);
      break;
    case 'timber':
      timberProp(g, f, light, rnd);
      break;
    case 'rail':
      rails(g, f, light, rnd);
      break;
    case 'ore_cart':
      oreCart(g, f, light, rnd);
      break;
    case 'ore_vein':
      oreVein(g, f, light, v, rnd, w);
      break;
    case 'tools':
      miningTools(g, f, light, rnd);
      break;
    case 'lantern':
      hangingLantern(g, f, light, rnd);
      break;
    case 'powder':
      powderKegs(g, f, light, rnd);
      break;
    case 'bedroll':
      bedroll(g, f, light, v, rnd);
      break;
    case 'sacks':
      grainSacks(g, f, light, rnd);
      break;
    case 'brazier':
      brazier(g, f, light, rnd);
      break;
    case 'candles':
      candelabrum(g, f, light, rnd);
      break;
    case 'chains':
      shackles(g, f, light, rnd, w);
      break;
    case 'sconce':
      torchSconce(g, f, light, rnd, w);
      break;
    case 'banner':
      tatteredBanner(g, f, light, v, rnd, w);
      break;
    case 'iron_maiden':
      ironMaiden(g, f, light, rnd);
      break;
    case 'stocks':
      stocks(g, f, light, rnd);
      break;
    case 'hoard':
      treasureHoard(g, f, light, rnd);
      break;
    case 'dais':
      daisThrone(g, f, light, rnd, w);
      break;
    case 'fountain':
      dryFountain(g, f, light, rnd);
      break;
    case 'glyph':
      ritualCircle(g, f, light, v, rnd);
      break;
  }
  return true;
}
