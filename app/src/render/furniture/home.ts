import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { BOOKS, BRASS, DARK_WOOD, DYES, INK, IRON, LINE, LINEN, type Light, OUT, WOOD, board, castShadow, frame, mute, oval, patch, planks, quad, ring, shade } from './common';

/** A camp cot: canvas stretched between two side poles that run past it at both ends, crossed legs
 * showing beyond the canvas at each end, a blanket thrown over the foot. */
export function cot(g: Graphics, f: InteriorItem, rnd: () => number) {
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

export function roundTable(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
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

export function table(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, long: boolean) {
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
export function chair(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  for (const a of [0.4, 2.5, 4.6]) g.circle(cx + Math.cos(a) * 0.2 - lx * 0.07, cy + Math.sin(a) * 0.2 - ly * 0.07, 0.06).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  g.poly(ring(cx, cy, 0.24, rnd, 0.012, 14)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(ring(cx + lx * 0.02, cy + ly * 0.02, 0.2, rnd, 0.008, 14)).fill(WOOD[1]);
  g.moveTo(cx - 0.13, cy + 0.02).lineTo(cx + 0.13, cy - 0.01).stroke({ width: LINE, color: WOOD[0], alpha: 0.85 });
}

export function bench(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect } = frame(f);
  // Legs: boards across under each end, wider than the seat.
  for (const u of [0.16, len - 0.26]) board(g, rect(u, 0.26, u + 0.1, 0.74), DARK_WOOD, light, rnd, !along, 0.02, 0);
  const face = board(g, rect(0.06, 0.32, len - 0.06, 0.68), WOOD, light, rnd, along, 0.04, 2);
  planks(g, face, along, 2, WOOD[0], rnd);
  // A lit band along the back half of the seat, so a bench this shallow still reads as a seat.
  { const r = rect(0.1, 0.35, len - 0.1, 0.47); g.rect(r[0], r[1], r[2] - r[0], r[3] - r[1]).fill({ color: WOOD[2], alpha: 0.85 }); }
}

/** Chest, its front toward +y: a lid of planks, two iron bands over it, iron corners and a lock. */
export function chest(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
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
export function wardrobe(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const face = board(g, [x + 0.08, y + 0.1, x + 0.92, y + 0.86], DARK_WOOD, light, rnd, true, 0.07, 1);
  g.rect(face[0] + 0.06, face[1] + 0.06, face[2] - face[0] - 0.12, face[3] - face[1] - 0.12).stroke({ width: LINE, color: DARK_WOOD[0] });
  g.poly(quad([x + 0.1, y + 0.86, x + 0.9, y + 0.92], rnd, 0.005)).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK });
  g.moveTo(x + 0.5, y + 0.86).lineTo(x + 0.5, y + 0.92).stroke({ width: LINE, color: INK });
  for (const kx of [x + 0.44, x + 0.56]) g.circle(kx, y + 0.89, 0.025).fill(BRASS);
}

/** How far shelves and bookcases stand off the wall behind them and the walls at their ends (squares):
 * clear of the walls' ink, drawn centred on the wall line (half of InteriorLayer's 0.45 outer wall). */
export const WALL_GAP = 0.24;
export const END_GAP = 0.12;

/** Bookcase against a wall, seen straight down: a back panel along the wall, end panels and uprights,
 * open toward the room, the books standing in it with their spines sticking out past the front, each
 * its colour with a band of gilt, the pages pale behind the spine; a gap now and then. */
export function bookcase(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
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
      g.poly(quad(rect(u, s0 + 0.03, u + th, out), rnd, 0.004)).fill(c).stroke({ width: 0.016, color: INK });
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
export function shelf(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
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
      g.moveTo(...at(u - 0.12, s)).lineTo(...at(u + 0.12, s)).stroke({ width: 0.014, color: 0x6a4a2a });
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
export const STONE = [0x5e5a52, 0x8a857a, 0xa8a296];
const BRICK = [0x6a3a2a, 0x9a5a3e, 0xb87a5a];

/** Fitted stones filling a rect, in courses along x, each its own tone, an ink line round the lot. */
export function masonry(g: Graphics, [x0, y0, x1, y1]: number[], t: readonly number[], rnd: () => number, course = 0.2) {
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
export function hearth(g: Graphics, f: InteriorItem, rnd: () => number, back: [number, number]) {
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
export function oven(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { len, rect, at } = frame(f, back);
  const [lx, ly] = light;
  const r = Math.min(len / 2 - 0.1, 0.4);
  const [cx, cy] = at(len / 2, 0.08 + r);
  // A flat hearth lip of stone slabs in front, toward the room.
  const lip = quad(rect(len / 2 - r * 1.1, 0.08 + r * 1.7, len / 2 + r * 1.1, 0.97), rnd, 0.01);
  g.poly(lip).fill(STONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(len / 2 - r * 0.7, 0.08 + r * 2.05)).lineTo(...at(len / 2 - r * 0.72, 0.96)).stroke({ width: 0.016, color: STONE[0] });
  // The dome: one smooth clay mound in three flat tones, lit from the light, its short shadow.
  const dome = ring(cx, cy, r, rnd, 0.025, 26);
  castShadow(g, dome, light, 0.14);
  g.poly(dome).fill(BRICK[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(ring(cx + lx * r * 0.12, cy + ly * r * 0.12, r * 0.8, rnd, 0.02, 22)).fill(BRICK[1]);
  g.poly(ring(cx + lx * r * 0.34, cy + ly * r * 0.34, r * 0.32, rnd, 0.02, 14)).fill(BRICK[2]);
  // A few bricks showing through the clay, laid round the dome.
  for (const [a, d] of [
    [-2.4, 0.72],
    [-1.2, 0.8],
    [0.1, 0.74],
    [2.6, 0.78],
    [-0.4, 0.42],
  ]) {
    const [bx, by] = [cx + Math.cos(a) * r * d, cy + Math.sin(a) * r * d];
    g.poly(oval(bx, by, r * 0.16, r * 0.09, a + Math.PI / 2, 8)).fill(shade(BRICK[1], 1.08)).stroke({ width: 0.016, color: BRICK[0] });
  }
  // The mouth seen from above: a dark notch cut into the dome's rim on the room side, a brick rim round it, the
  // hearth lip in front of it, embers deep inside.
  const [mu, ms, mw] = [len / 2, 0.08 + r * 2, r * 0.46];
  const notch = (w: number, d: number, k = 10) => {
    const pts: number[] = [];
    for (let i = 0; i <= k; i++) {
      const t = (i / k) * Math.PI;
      pts.push(...at(mu + Math.cos(t) * w, ms + 0.02 - Math.sin(t) * d));
    }
    return pts;
  };
  g.poly(notch(mw + 0.05, r * 0.38 + 0.05)).fill(BRICK[2]).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(notch(mw, r * 0.38)).fill(0x1a1008).stroke({ width: LINE, color: INK, join: 'round' });
  const [ex, ey] = at(mu, ms - r * 0.16);
  g.poly(oval(ex, ey, mw * 0.55, r * 0.09, 0, 10)).fill({ color: 0xc2501a, alpha: 0.8 });
  g.circle(ex, ey, 0.022).fill(0xe8902a);
  // One oven in two has its peel or the morning's loaves on the lip.
  if (rnd() < 0.5) {
    for (const [du, ds] of [
      [-r * 0.92, 0.9],
      [r * 0.95, 0.88],
    ]) {
      const [px, py] = at(mu + du, ds);
      g.poly(oval(px, py, 0.075, 0.055, rnd() * 0.6, 12)).fill(0xa8703a).stroke({ width: LINE, color: INK });
      g.poly(oval(px + lx * 0.015, py + ly * 0.015, 0.05, 0.035, 0, 10)).fill(0xc89458);
      g.moveTo(px - 0.03, py + 0.01).lineTo(px + 0.03, py - 0.01).stroke({ width: 0.014, color: 0x6a4020 });
    }
  } else {
    const [a, b] = [at(mu - r * 1.0, 0.9), at(mu + r * 0.3, 0.9)];
    g.moveTo(...a).lineTo(...b).stroke({ width: 0.05, color: INK, cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: 0.026, color: WOOD[2], cap: 'round' });
    const [px, py] = at(mu + r * 0.45, 0.9);
    g.poly(oval(px, py, 0.1, 0.065, Math.atan2(b[1] - a[1], b[0] - a[0]), 12)).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(oval(px + lx * 0.015, py + ly * 0.015, 0.06, 0.035, Math.atan2(b[1] - a[1], b[0] - a[0]), 10)).fill(WOOD[2]);
  }
}

/** Smith's forge against a wall: a block of grey brick (the chimney breast) with a half-round stone
 * basin in front of it, opening toward the room and heaped with glowing coals; a water trough beside it. */
export function forge(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
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
  for (let i = 0; i < 22; i++) {
    const t = rnd() * Math.PI;
    const d = (R - 0.2) * Math.sqrt(rnd());
    const [x, y] = at(cu + Math.cos(t) * d, cs + Math.sin(t) * d);
    g.circle(x, y, 0.04 + 0.025 * rnd()).fill([0x6a2414, 0x8e3418, 0xa8481e, 0xc0642a, 0xd88a3a][Math.floor(rnd() * rnd() * 5)]);
  }
  g.poly(half((R - 0.12) * 0.5, 0.01)).fill({ color: 0xd88a3a, alpha: 0.3 });
  // The brick block against the wall, over the basin's straight edge.
  masonry(g, rect(0.1, WALL_GAP - 0.04, len - 0.62, cs + 0.02), STONE.map((c) => shade(c, 0.85)), rnd, 0.17);
}

/** Anvil on a stump: the stump's cut face, then the anvil from above — a flat face with a square heel,
 * the horn tapering to a point, a hardy hole — its face lit. */
export function anvil(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
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
export function workbench(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
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
export function counter(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect } = frame(f, back);
  const face = board(g, rect(0.06, 0.48, len - 0.06, 0.94), WOOD, light, rnd, along, 0.05, 2);
  planks(g, face, along, 2, WOOD[0], rnd);
  g.poly(quad(rect(0.2, 0.8, len - 0.2, 0.88), rnd, 0.004)).fill({ color: WOOD[2], alpha: 0.5 });
  // A low shelf against the wall behind, for stock.
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, WALL_GAP + 0.14), DARK_WOOD, light, rnd, along, 0.03, 1);
}

/** Bar against a wall: a long, dark, polished counter toward the room, a back shelf of bottles along the
 * wall, the barkeep's aisle between them. */
export function bar(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const shelfR = rect(END_GAP, WALL_GAP, len - END_GAP, WALL_GAP + 0.16);
  board(g, shelfR, DARK_WOOD, light, rnd, along, 0.03, 0);
  const glass = [0x3a6a3a, 0x6a3a2a, 0x2e4a6a, 0xa87a3a, 0x5a3a5e];
  for (let u = END_GAP + 0.1, k = 0; u < len - END_GAP - 0.06; u += 0.11, k++) {
    const [bx, by] = at(u, WALL_GAP + 0.08);
    g.circle(bx, by, 0.04).fill(glass[(k * 3) % glass.length]).stroke({ width: 0.016, color: INK });
    g.circle(bx - 0.012, by - 0.012, 0.012).fill({ color: 0xffffff, alpha: 0.5 });
  }
  const face = board(g, rect(0.06, 0.56, len - 0.06, 0.94), DARK_WOOD.map((c) => shade(c, 1.15)), light, rnd, along, 0.05, 2);
  planks(g, face, along, 2, DARK_WOOD[0], rnd);
  // A brass foot-rail along the front.
  g.moveTo(...at(0.1, 0.99)).lineTo(...at(len - 0.1, 0.99)).stroke({ width: 0.025, color: BRASS });
}

/** A barrel standing on end: a ring of staves, two iron hoops, a planked lid with a bung, lit on its
 * light side. */
export function barrel(g: Graphics, cx: number, cy: number, r: number, light: Light, rnd: () => number, out = OUT) {
  const [lx, ly] = light;
  g.poly(ring(cx, cy, r, rnd, 0.01, 18)).fill(WOOD[0]).stroke({ width: out, color: INK, join: 'round' });
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * r * 0.7, cy + Math.sin(a) * r * 0.7).lineTo(cx + Math.cos(a) * r * 0.97, cy + Math.sin(a) * r * 0.97);
  }
  g.stroke({ width: 0.014, color: shade(WOOD[0], 0.7) });
  g.circle(cx, cy, r * 0.86).stroke({ width: out < OUT ? 0.03 : 0.045, color: IRON[0] });
  const la = Math.atan2(ly, lx);
  g.moveTo(cx + Math.cos(la - 0.9) * r * 0.86, cy + Math.sin(la - 0.9) * r * 0.86)
    .arc(cx, cy, r * 0.86, la - 0.9, la + 0.9)
    .stroke({ width: 0.015, color: IRON[2] });
  g.circle(cx + lx * r * 0.06, cy + ly * r * 0.06, r * 0.68).fill(WOOD[1]).stroke({ width: LINE, color: IRON[0] });
  for (const d of [-0.3, 0.3]) g.moveTo(cx - r * 0.6, cy + d * r).lineTo(cx + r * 0.6, cy + d * r);
  g.stroke({ width: 0.014, color: WOOD[0], alpha: 0.85 });
  g.circle(cx + r * 0.3, cy - r * 0.1, r * 0.08).fill(0x2a1c10);
}

/** A keg lying on its side in a cradle, its tap toward the room: staves bulging between flat ends,
 * iron hoops round it, the side toward the light paler. */
export function keg(g: Graphics, f: InteriorItem, u: number, light: Light, rnd: () => number, back: [number, number]) {
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
  g.stroke({ width: 0.016, color: WOOD[0], alpha: 0.7 });
  // Hoops: bowed bands across, near each end and either side of the middle.
  for (const s0 of [e0 + 0.08, (e0 + e1) / 2 - 0.08, (e0 + e1) / 2 + 0.08, e1 - 0.08]) {
    const bulge = 0.12 * (1 - Math.abs(s0 - (e0 + e1) / 2) / ((e1 - e0) / 2)) + 0.02;
    g.moveTo(...at(a - bulge * 0.9, s0)).lineTo(...at(b + bulge * 0.9, s0));
  }
  g.stroke({ width: 0.03, color: IRON[0] });
  g.moveTo(...at(u + 0.5, e1)).lineTo(...at(u + 0.5, e1 + 0.07)).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.moveTo(...at(u + 0.5, e1)).lineTo(...at(u + 0.5, e1 + 0.07)).stroke({ width: 0.022, color: BRASS, cap: 'round' });
}

/** Crate: a frame of boards round three planks, nails in the corners, a cross brace on some; bold dark
 * ink round the box, the frame, the plank seams and the brace. */
function crate(g: Graphics, x0: number, y0: number, sz: number, light: Light, rnd: () => number, braced: boolean) {
  const face = board(g, [x0, y0, x0 + sz, y0 + sz], [0x6e4e2e, 0x9a7448, 0xae8858], light, rnd, true, 0.04, 0);
  const e = sz * 0.13;
  const [fx0, fy0, fx1, fy1] = face;
  const ink = 0.034;
  g.rect(fx0, fy0, fx1 - fx0, fy1 - fy0).stroke({ width: e, color: 0x7e5c38, alignment: 1 });
  // The frame's inner edge and the plank seams, inked.
  const [ix0, iy0, ix1, iy1] = [fx0 + e, fy0 + e, fx1 - e, fy1 - e];
  for (let k = 1; k < 3; k++) {
    const t = iy0 + ((iy1 - iy0) * k) / 3;
    g.moveTo(ix0, t).lineTo(ix1, t);
  }
  g.stroke({ width: ink * 0.8, color: INK });
  g.rect(ix0, iy0, ix1 - ix0, iy1 - iy0).stroke({ width: ink * 0.8, color: INK });
  for (const [ax, ay, bx, by] of [
    [x0, y0, ix0, iy0],
    [x0 + sz, y0, ix1, iy0],
    [x0 + sz, y0 + sz, ix1, iy1],
    [x0, y0 + sz, ix0, iy1],
  ]) {
    g.moveTo(ax, ay).lineTo(bx, by);
  }
  g.stroke({ width: ink * 0.6, color: INK, cap: 'round' });
  if (braced) {
    g.moveTo(ix0, iy1).lineTo(ix1, iy0).stroke({ width: e * 0.8 + ink * 1.4, color: INK });
    g.moveTo(ix0, iy1).lineTo(ix1, iy0).stroke({ width: e * 0.8, color: 0x8a6640 });
    g.moveTo(ix0, iy1).lineTo(ix1, iy0).stroke({ width: e * 0.3, color: 0xae8858, alpha: 0.7 });
  }
  g.rect(x0, y0, sz, sz).stroke({ width: ink * 1.3, color: INK, join: 'round' });
  for (const [nx, ny] of [
    [fx0 + e / 2, fy0 + e / 2],
    [fx1 - e / 2, fy0 + e / 2],
    [fx1 - e / 2, fy1 - e / 2],
    [fx0 + e / 2, fy1 - e / 2],
  ]) {
    g.circle(nx, ny, 0.018).fill(INK);
  }
}

/** Crates: one with a brace, or (on two by two) four in a block with one more stacked on top in the
 * middle, all the same size, a crate missing from the block on some. */
export function crates(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
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
export const PLUSH = [
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
export function bed(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, head: [number, number]) {
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
export function couch(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
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
    g.moveTo(...at((a + b) / 2, WALL_GAP + 0.3)).lineTo(...at((a + b) / 2 + 0.01, 0.74)).stroke({ width: 0.014, color: t[0], alpha: 0.7 });
  }
}

/** Sideboard against a wall: a long cabinet top with a moulded rim and an inlaid border, the doors'
 * edge at the front with brass pulls. */
export function sideboard(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const face = board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.74), DARK_WOOD, light, rnd, along, 0.05, 2);
  g.rect(face[0] + 0.06, face[1] + 0.06, face[2] - face[0] - 0.12, face[3] - face[1] - 0.12).stroke({ width: 0.015, color: DARK_WOOD[2], alpha: 0.8 });
  g.poly(quad(rect(END_GAP + 0.02, 0.74, len - END_GAP - 0.02, 0.8), rnd, 0.004)).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK });
  const doors = Math.max(2, Math.round(len) * 2);
  for (let i = 1; i < doors; i++) g.moveTo(...at(END_GAP + ((len - 2 * END_GAP) * i) / doors, 0.74)).lineTo(...at(END_GAP + ((len - 2 * END_GAP) * i) / doors, 0.8));
  g.stroke({ width: 0.016, color: INK });
  for (let i = 0; i < doors; i++) g.circle(...at(END_GAP + ((len - 2 * END_GAP) * (i + 0.5)) / doors, 0.77), 0.018).fill(BRASS);
}

/** Desk: a dark wooden top with a green leather writing surface; papers on it, an ink jar and a quill. */
export function desk(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
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
    g.poly([...pt(-hw, -hh), ...pt(hw, -hh), ...pt(hw, hh), ...pt(-hw, hh)]).fill(i === 2 ? LINEN[2] : LINEN[1]).stroke({ width: 0.016, color: INK });
    if (i === 2) {
      for (let k = 0; k < 4; k++) {
        const v = -hh * 0.6 + k * hh * 0.4;
        g.moveTo(...(pt(-hw * 0.75, v) as [number, number])).lineTo(...(pt(hw * (0.35 + 0.35 * rnd()), v) as [number, number]));
      }
      g.stroke({ width: 0.016, color: 0x5a4a3a, alpha: 0.8 });
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
  g.poly([qa[0], qa[1], mid[0] + nx * 0.045, mid[1] + ny * 0.045, qb[0], qb[1], mid[0] - nx * 0.025, mid[1] - ny * 0.025]).fill(LINEN[2]).stroke({ width: 0.016, color: INK, join: 'round' });
  g.moveTo(qa[0], qa[1]).lineTo(qb[0], qb[1]).stroke({ width: 0.016, color: LINEN[0] });
}

/** Display table in a shop: a cloth over it, hanging past the edges, and wares laid out on it. */
export function display(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { len, rect, at } = frame(f);
  const cloth = mute(shade(DYES[(v + 1) % DYES.length], 0.8), 0.5);
  g.poly(quad(rect(0.06, 0.08, len - 0.06, 0.92), rnd, 0.01)).fill(shade(cloth, 0.75)).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(quad(rect(0.14, 0.16, len - 0.14, 0.84), rnd, 0.006)).fill(cloth);
  for (const u of [0.3, len - 0.3]) g.moveTo(...at(u, 0.08)).lineTo(...at(u + 0.02, 0.16));
  g.stroke({ width: 0.015, color: shade(cloth, 0.6) });
  // Wares: rolls of cloth, a pot, small boxes.
  for (let u = 0.35, k = v; u < len - 0.2; u += 0.42, k++) {
    if (k % 3 === 0) {
      const r = rect(u - 0.14, 0.3, u + 0.14, 0.7);
      g.poly(quad(r, rnd, 0.004)).fill(mute([0xc8a060, 0x6a8aa8, 0xa86a5a][k % 3 === 0 ? (k / 3) % 3 : 0], 0.3)).stroke({ width: LINE, color: INK });
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
export function pew(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect } = frame(f, back);
  const box = (r: number[]) => [r[0], r[1], r[0], r[3], r[2], r[3], r[2], r[1]];
  castShadow(g, box(rect(0.06, 0.12, len - 0.06, 0.8)), light, 0.1);
  // The seat: one broad pale board toward the front, its front edge lit.
  board(g, rect(0.12, 0.3, len - 0.12, 0.8), WOOD, light, rnd, along, 0.03, 1, LINE);
  // The back's shadow falling on the seat just in front of it: the back stands up.
  { const r = rect(0.13, 0.3, len - 0.13, 0.4); g.rect(r[0], r[1], r[2] - r[0], r[3] - r[1]).fill({ color: 0x2a1a10, alpha: 0.35 }); }
  // The back rail: a narrow dark top rail along the back side, its capping lit.
  board(g, rect(0.08, 0.12, len - 0.08, 0.3), DARK_WOOD, light, rnd, along, 0.03, 0, OUT);
  { const r = rect(0.12, 0.17, len - 0.12, 0.21); g.rect(r[0], r[1], r[2] - r[0], r[3] - r[1]).fill({ color: DARK_WOOD[2], alpha: 0.9 }); }
  // Bench ends: a small square block at each end, standing past the seat and the back.
  for (const u of [0.04, len - 0.18]) board(g, rect(u, 0.1, u + 0.14, 0.82), DARK_WOOD, light, rnd, !along, 0.025, 0, OUT);
}

/** Booth table: a square table, plain worn planks. */
export function boothTable(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const face = board(g, [x + 0.08, y + 0.08, x + 0.92, y + 0.92], WOOD, light, rnd, true, 0.05, 1);
  planks(g, face, true, 3, WOOD[0], rnd);
}

/** Booth seat: an upholstered bench half a square deep, its high back on the side away from its
 * table, always in the same red. The corner seat (`back` a diagonal step) is an L along both walls,
 * so the bench runs on round the corner. */
export function boothSeat(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
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
export function rug(g: Graphics, f: InteriorItem, v: number, rnd: () => number) {
  const { len, rect, at } = frame(f);
  const wid = f.w >= f.h ? f.h : f.w;
  const [ground, border, motif] = [
    [0x8a2e26, 0x2e4a5a, 0xc8a45a],
    [0x2e4a6a, 0xa8742e, 0xe0cca0],
    [0x6a4a2a, 0x8a2e26, 0xd0b070],
    [0x4a5a32, 0x6a2e3a, 0xd8c08a],
  ][v % 4].map((c) => mute(c, 0.3));
  // Fringes at the ends.
  for (const u of [0.1, len - 0.1]) {
    for (let s = 0.2; s < wid - 0.15; s += 0.06) g.moveTo(...at(u, s)).lineTo(...at(u + (u < 1 ? -0.07 : 0.07), s));
  }
  g.stroke({ width: 0.014, color: 0xe0d0b0 });
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
export function bath(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
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
  g.stroke({ width: 0.014, color: WOOD[0] });
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

/** Bucket: a wooden pail from above: a ring of staves round a dark inside with a little water (a flat pale
 * glint on it), an iron hoop, and its iron handle tipped over across the top as an arc between two lugs. */
export function bucket(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  const r = 0.15;
  const rim = ring(cx, cy, r, rnd, 0.006, 16);
  castShadow(g, rim, light, 0.06);
  g.poly(rim).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(cx + lx * 0.008, cy + ly * 0.008, r * 0.9).stroke({ width: 0.016, color: WOOD[2] });
  // The staves: seams round the rim and down into the dark.
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2 + 0.3;
    g.moveTo(cx + Math.cos(a) * r * 0.7, cy + Math.sin(a) * r * 0.7).lineTo(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  }
  g.stroke({ width: 0.016, color: WOOD[0] });
  // The dark inside, one small dull glint.
  g.circle(cx, cy, r * 0.7).fill(0x1c140e).stroke({ width: 0.016, color: INK });
  g.circle(cx + lx * r * 0.3, cy + ly * r * 0.3, r * 0.45).fill(0x2a2018);
  g.circle(cx - lx * r * 0.28, cy - ly * r * 0.28, 0.014).fill(0x6a6a5a);
  // The handle tipped over: an iron arc from lug to lug across the top.
  const [ax, bx] = [cx - r * 1.02, cx + r * 1.02];
  const by = cy + r * 0.95;
  g.moveTo(ax, cy).quadraticCurveTo(cx, by, bx, cy).stroke({ width: 0.045, color: INK, cap: 'round' });
  g.moveTo(ax, cy).quadraticCurveTo(cx, by, bx, cy).stroke({ width: 0.022, color: IRON[1], cap: 'round' });
  for (const x of [ax, bx]) g.circle(x, cy, 0.026).fill(IRON[1]).stroke({ width: 0.016, color: INK });
}

/** A roof terrace's dome (`interior::furnish_terrace`) from above on its drum: warm plaster in three
 * flat tones lit from the light, a few bold segment lines up to a small finial, its shadow beside it. */
export function roofDome(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) / 2 - OUT / 2;
  const [lx, ly] = light;
  const body = ring(cx, cy, r, rnd, 0.006, 32);
  castShadow(g, body, light, 0.3);
  g.poly(body).fill(0x9c8a6e).stroke({ width: OUT, color: INK, join: 'round' });
  g.circle(cx + lx * r * 0.12, cy + ly * r * 0.12, r * 0.86).fill(0xb8a684);
  g.circle(cx + lx * r * 0.36, cy + ly * r * 0.36, r * 0.46).fill(0xcebe9c);
  for (let j = 0; j < 6; j++) {
    const a = (Math.PI * j) / 3 + 0.5;
    g.moveTo(cx + Math.cos(a) * r * 0.1, cy + Math.sin(a) * r * 0.1).lineTo(cx + Math.cos(a) * r * 0.97, cy + Math.sin(a) * r * 0.97);
  }
  g.stroke({ width: LINE, color: INK, cap: 'round' });
  g.circle(cx, cy, 0.07).fill(0x9c8a6e).stroke({ width: LINE, color: INK });
}

/** A water jar on a roof terrace from above: a round clay belly lit on the light side, shaded in a
 * crescent away from it, its mouth (whitewashed on some) open on dark water. */
export function waterJar(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [lx, ly] = light;
  const r = 0.27;
  const body = ring(cx, cy, r, rnd, 0.006, 20);
  castShadow(g, body, light, 0.24);
  g.poly(body).fill(0x7a5640).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(cx + lx * 0.045, cy + ly * 0.045, r * 0.82).fill(0xa47a58);
  g.circle(cx + lx * 0.12, cy + ly * 0.12, r * 0.3).fill(0xc09a76);
  const lip = v % 2 ? 0xe0d6bc : 0xb88a64;
  g.circle(cx, cy, 0.12).fill(lip).stroke({ width: LINE, color: INK });
  g.circle(cx - lx * 0.012, cy - ly * 0.012, 0.075).fill(0x34403e).stroke({ width: 0.018, color: INK });
}

/** A planter on a roof terrace: a clay (or plank) trough of earth, a few big clumps of herbs and
 * greens, a few flowers on some. */
export function planter(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const r = [f.x + 0.12, f.y + 0.12, f.x + f.w - 0.12, f.y + f.h - 0.12];
  castShadow(g, [r[0], r[1], r[2], r[1], r[2], r[3], r[0], r[3]], light, 0.2);
  const box = v % 2 ? [0x8a5a3e, 0xa87252, 0xc0906c] : [0x7a5a3a, 0x9a7448, 0xae8858];
  const face = board(g, r, box, light, rnd, f.w >= f.h, 0.07, 0);
  const soil = [face[0] + 0.04, face[1] + 0.04, face[2] - 0.04, face[3] - 0.04];
  g.poly(quad(soil, rnd, 0.01)).fill(0x6a4c34).stroke({ width: 0.018, color: INK });
  const greens = [0x4a6a34, 0x6a8a44, 0x8ca85c];
  const along = f.w >= f.h;
  const [L, W] = along ? [soil[2] - soil[0], soil[3] - soil[1]] : [soil[3] - soil[1], soil[2] - soil[0]];
  const n = Math.max(1, Math.round(L / 0.42));
  const spots: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    const u = (L * (i + 0.5)) / n + (rnd() - 0.5) * 0.06;
    const s = W / 2 + (rnd() - 0.5) * 0.06;
    spots.push(along ? [soil[0] + u, soil[1] + s] : [soil[0] + s, soil[1] + u]);
  }
  for (const [px, py] of spots) {
    const s = Math.min(W * 0.46, 0.21) * (0.9 + rnd() * 0.15);
    g.poly(patch(px, py, s, s * 0.92, rnd, 0.3, 9)).fill(greens[0]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(patch(px + light[0] * s * 0.18, py + light[1] * s * 0.18, s * 0.7, s * 0.64, rnd, 0.25, 8)).fill(greens[1]);
    g.circle(px + light[0] * s * 0.4, py + light[1] * s * 0.4, s * 0.26).fill(greens[2]);
  }
  if (v % 4 >= 2) {
    const c = [0xb04a36, 0xd0b048, 0xe8dcc4][v % 3];
    for (const [px, py] of spots) g.circle(px - light[0] * 0.07, py - light[1] * 0.07, 0.045).fill(c).stroke({ width: 0.018, color: INK });
  }
}

/** A washing line along its squares between two posts, sagging a little, linen and undyed cloths
 * pegged along it (one dyed blue) and hanging to the side, their shadows on the terrace. */
export function washingLine(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const along = f.w >= f.h;
  const L = along ? f.w : f.h;
  const p = (t: number, s: number): [number, number] => (along ? [f.x + t, f.y + f.h / 2 + s] : [f.x + f.w / 2 + s, f.y + t]);
  const [lx, ly] = light;
  const colors = [LINEN[1], 0xc4b08a, LINEN[2], 0xb0a084];
  const cloths: [number, number, number, number][] = [];
  for (let t = 0.4; ; ) {
    const w = 0.32 + rnd() * 0.14;
    if (t + w > L - 0.34) break;
    cloths.push([t, w, 0.3 + rnd() * 0.1, colors[(v + cloths.length) % colors.length]]);
    t += w + 0.06 + rnd() * 0.1;
  }
  // (One small accent: a blue cloth.)
  if (cloths.length) cloths[(v + 1) % cloths.length][3] = 0x4e6a84;
  const sag = (t: number) => 0.05 * Math.sin((Math.PI * (t - 0.25)) / (L - 0.5));
  const cloth = (t: number, w: number, d: number, ox = 0, oy = 0) => {
    // (Seen from above, hung over the line: as much of it either side.)
    const [a, b, c, e] = [p(t, sag(t) - d * 0.45), p(t + w, sag(t + w) - d * 0.45), p(t + w - 0.02, sag(t + w) + d * 0.55), p(t + 0.02, sag(t) + d * 0.55 + 0.02)];
    return [a[0] + ox, a[1] + oy, b[0] + ox, b[1] + oy, c[0] + ox, c[1] + oy, e[0] + ox, e[1] + oy];
  };
  for (const [t, w, d] of cloths) g.poly(cloth(t, w, d, -lx * 0.2, -ly * 0.2)).fill({ color: 0x000000, alpha: 0.16 });
  const [a, b] = [p(0.25, 0), p(L - 0.25, 0)];
  const m = p(L / 2, 0.1);
  g.moveTo(a[0] - lx * 0.2, a[1] - ly * 0.2).quadraticCurveTo(m[0] - lx * 0.2, m[1] - ly * 0.2, b[0] - lx * 0.2, b[1] - ly * 0.2).stroke({ width: 0.02, color: 0x000000, alpha: 0.18 });
  g.moveTo(...a).quadraticCurveTo(...m, ...b).stroke({ width: LINE, color: INK });
  for (const [t, w, d, c] of cloths) {
    g.poly(cloth(t, w, d)).fill(c).stroke({ width: LINE, color: INK, join: 'round' });
    // (The fold where it hangs over the line, a crease down it.)
    g.moveTo(...p(t + 0.03, sag(t))).lineTo(...p(t + w - 0.03, sag(t + w))).stroke({ width: 0.018, color: shade(c, 0.72) });
    const [q0, q1] = [p(t + w * 0.5, sag(t + w * 0.5) + 0.04), p(t + w * 0.5 + (rnd() - 0.5) * 0.06, sag(t + w * 0.5) + d * 0.48)];
    g.moveTo(...q0).lineTo(...q1).stroke({ width: 0.016, color: shade(c, 0.78) });
    for (const u of [t + 0.04, t + w - 0.04]) g.circle(...p(u, sag(u)), 0.024).fill(WOOD[0]).stroke({ width: 0.016, color: INK });
  }
  for (const q of [a, b]) {
    g.circle(q[0] - lx * 0.06, q[1] - ly * 0.06, 0.08).fill({ color: 0x000000, alpha: 0.25 });
    g.circle(q[0], q[1], 0.075).fill(WOOD[0]).stroke({ width: LINE, color: INK });
    g.circle(q[0] + lx * 0.02, q[1] + ly * 0.02, 0.045).fill(WOOD[2]);
  }
}
