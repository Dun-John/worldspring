import type { Graphics } from "pixi.js";
import { ROOF_INK } from "../gen/battlePrep";
import { PX } from "./atlas";

/**
 * Ways underground drawn by the theme they come to (v63, styles 4-6 of `size = kind + 16 × style`,
 * `under::surface`), and the cave mouth. Plan view in the battlemap's cartoon style: black ink round
 * three close matte tones, lit from the upper left (shadows fall down and right). `at(a, b)`: a
 * point `a` squares in along the passage from the opening and `b` across it.
 */
type At = (a: number, b: number) => [number, number];

const INK = ROOF_INK;
const STONE = [0xb7b1a5, 0x9e988c, 0x857f74];
const ROCK = [0x928c80, 0x7c766c, 0x655f57];
const TURF = [0x7e8d4f, 0x6d7b44, 0x5c6939];
const DARK = 0x16130f;

/** Shift a flat point list down and right by `d` squares (a drop shadow). */
const shade = (pts: number[], d: number) => pts.map((v) => v + d * PX);

/** An oval round (`ca`, 0) with radii `ra` along and `rb` across, its rim wobbling by `jit`. */
function oval(
  at: At,
  ca: number,
  ra: number,
  rb: number,
  jit: number[],
  b0 = 0,
): number[] {
  const n = jit.length;
  const pts: number[] = [];
  for (let k = 0; k < n; k++) {
    const t = (k / n) * Math.PI * 2;
    const r = 1 + jit[k];
    pts.push(...at(ca + Math.cos(t) * ra * r, b0 + Math.sin(t) * rb * r));
  }
  return pts;
}

/** The same ring of points scaled about its centre by `f` and moved by (`dx`, `dy`) px. */
function inset(pts: number[], f: number, dx: number, dy: number): number[] {
  let [cx, cy] = [0, 0];
  for (let k = 0; k < pts.length; k += 2)
    [cx, cy] = [cx + pts[k], cy + pts[k + 1]];
  [cx, cy] = [(cx * 2) / pts.length, (cy * 2) / pts.length];
  return pts.map((v, k) =>
    k % 2 ? cy + (v - cy) * f + dy : cx + (v - cx) * f + dx,
  );
}

/** A rough boulder or fallen block at `p` (px), radius `r` squares. */
function boulder(
  g: Graphics,
  p: [number, number],
  r: number,
  tone: number,
  rnd: () => number,
) {
  const pts: number[] = [];
  const n = 6 + Math.floor(rnd() * 3);
  const a0 = rnd() * Math.PI;
  for (let k = 0; k < n; k++) {
    const t = a0 + (k / n) * Math.PI * 2;
    const rr = r * PX * (0.8 + rnd() * 0.3);
    pts.push(p[0] + Math.cos(t) * rr, p[1] + Math.sin(t) * rr * 0.85);
  }
  g.poly(shade(pts, 0.08)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(pts).fill(tone).stroke({ width: 1.8, color: INK, join: "round" });
  g.poly(inset(pts, 0.5, -r * PX * 0.25, -r * PX * 0.25)).fill({
    color: 0xffffff,
    alpha: 0.12,
  });
}

/** Style 4, 5 or 6: the barrow, the rune gate or the tower stump. */
export function drawThemedEntrance(
  g: Graphics,
  style: number,
  at: At,
  rnd: () => number,
) {
  if (style === 4) drawBarrow(g, at, rnd);
  else if (style === 5) drawRuneGate(g, at, rnd);
  else drawTowerStump(g, at, rnd);
}

/**
 * A long barrow over a tomb or crypt, 6 squares long and 4 across: a turf mound lit from the upper
 * left, a few grass tufts, a façade of standing stones either side of the stone doorway at its
 * front end (two jambs and a capstone over the dark passage going down).
 */
function drawBarrow(g: Graphics, at: At, rnd: () => number) {
  const jit = Array.from({ length: 28 }, () => (rnd() - 0.5) * 0.06);
  const mound = oval(at, 2.4, 3.0, 2.05, jit);
  g.poly(shade(mound, 0.35)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(mound).fill(TURF[2]);
  g.poly(inset(mound, 0.9, -0.1 * PX, -0.1 * PX)).fill(TURF[1]);
  g.poly(inset(mound, 0.5, -0.4 * PX, -0.3 * PX)).fill(TURF[0]);
  g.poly(mound).stroke({ width: 2.5, color: INK, join: "round" });
  // Grass tufts: sparse ink ticks on the turf.
  for (let k = 0; k < 9; k++) {
    const [x, y] = at(0.6 + rnd() * 3.8, (rnd() - 0.5) * 2.8);
    g.moveTo(x - 4, y + 3)
      .lineTo(x - 1, y - 3)
      .moveTo(x + 1, y + 3)
      .lineTo(x + 3, y - 4);
  }
  g.stroke({ width: 1.3, color: TURF[2] - 0x101008, alpha: 0.9 });
  // The façade: standing stones curving out from the door like horns.
  for (const side of [-1, 1]) {
    for (const [b, a] of [
      [1.05, -0.62],
      [1.5, -0.5],
      [1.92, -0.25],
    ]) {
      const [x, y] = at(a, side * b);
      const pts: number[] = [];
      for (let k = 0; k < 6; k++) {
        const t = (k / 6) * Math.PI * 2;
        const [ox, oy] = [Math.cos(t) * 0.22, Math.sin(t) * 0.3];
        const [px, py] = at(a + ox, side * b + oy);
        pts.push(
          x + (px - x) * (0.9 + rnd() * 0.2),
          y + (py - y) * (0.9 + rnd() * 0.2),
        );
      }
      g.poly(shade(pts, 0.12)).fill({ color: 0x000000, alpha: 0.3 });
      g.poly(pts)
        .fill(STONE[1])
        .stroke({ width: 1.8, color: INK, join: "round" });
      g.poly(inset(pts, 0.5, -3, -3)).fill(STONE[0]);
    }
  }
  // The doorway: a dark passage into the mound, steps going down, jambs and the capstone.
  const quad = (a0: number, a1: number, b0: number, b1: number) => [
    ...at(a0, b0),
    ...at(a1, b0),
    ...at(a1, b1),
    ...at(a0, b1),
  ];
  g.poly(quad(-0.75, 0.5, -0.42, 0.42)).fill(DARK);
  for (let k = 0; k < 3; k++)
    g.poly(quad(-0.7 + k * 0.17, -0.58 + k * 0.17, -0.38, 0.38)).fill(
      [0x5d574e, 0x48433c, 0x35312b][k],
    );
  for (const side of [-1, 1]) {
    const j = quad(-0.85, 0.35, side * 0.42, side * 0.8);
    g.poly(shade(j, 0.1)).fill({ color: 0x000000, alpha: 0.3 });
    g.poly(j).fill(STONE[1]).stroke({ width: 2, color: INK, join: "round" });
  }
  const cap = quad(-0.18, 0.62, -0.95, 0.95);
  g.poly(shade(cap, 0.14)).fill({ color: 0x000000, alpha: 0.35 });
  g.poly(cap).fill(STONE[0]).stroke({ width: 2.2, color: INK, join: "round" });
  g.moveTo(...at(0.02, -0.5))
    .lineTo(...at(0.25, -0.1))
    .lineTo(...at(0.12, 0.35))
    .stroke({ width: 1.2, color: STONE[2] });
}

/**
 * A dwarven hall's gate cut into a rock face 3 squares wide: rock shoulders either side (the crag
 * rising behind is the ground's own), a dressed façade with square pillars carved with runes, broad
 * steps up to the dark gateway, its stone leaves swung in against the passage walls.
 */
function drawRuneGate(g: Graphics, at: At, rnd: () => number) {
  const quad = (a0: number, a1: number, b0: number, b1: number) => [
    ...at(a0, b0),
    ...at(a1, b0),
    ...at(a1, b1),
    ...at(a0, b1),
  ];
  // Rock shoulders: rounded masses of the crag either side of the façade.
  for (const side of [-1, 1]) {
    const pts = oval(
      at,
      0.85,
      0.85,
      0.75,
      Array.from({ length: 14 }, () => (rnd() - 0.5) * 0.16),
      side * 2.2,
    );
    g.poly(shade(pts, 0.2)).fill({ color: 0x000000, alpha: 0.25 });
    g.poly(pts).fill(ROCK[1]).stroke({ width: 2.5, color: INK, join: "round" });
    g.poly(inset(pts, 0.55, -0.12 * PX, -0.12 * PX)).fill(ROCK[0]);
    // A crack.
    const [x, y] = at(0.7 + rnd() * 0.4, side * (2.0 + rnd() * 0.4));
    g.moveTo(x, y)
      .lineTo(x + (rnd() - 0.5) * 22, y + (rnd() - 0.5) * 22)
      .stroke({ width: 1.3, color: ROCK[2] });
  }
  // The façade: one dressed slab, its joints scored.
  const face = quad(0.05, 0.8, -1.6, 1.6);
  g.poly(face).fill(STONE[1]).stroke({ width: 2.5, color: INK, join: "miter" });
  for (const b of [-1.15, 1.15]) g.moveTo(...at(0.05, b)).lineTo(...at(0.8, b));
  g.stroke({ width: 1.2, color: STONE[2] });
  // The gateway: dark, steps going down inside, the leaves open against its walls.
  g.poly(quad(0.05, 1.7, -0.72, 0.72))
    .fill(DARK)
    .stroke({ width: 2, color: INK });
  for (let k = 0; k < 4; k++)
    g.poly(quad(0.1 + k * 0.32, 0.36 + k * 0.32, -0.6, 0.6)).fill(
      [0x6a645a, 0x534e46, 0x3e3a34, 0x2b2824][k],
    );
  for (const side of [-1, 1])
    g.poly(quad(0.12, 1.0, side * 0.58, side * 0.72))
      .fill(ROCK[2])
      .stroke({ width: 1.5, color: INK });
  // Broad steps up to it.
  for (const [a0, a1, w, c] of [
    [-1.05, -0.6, 1.1, STONE[1]],
    [-0.6, 0.05, 0.9, STONE[0]],
  ] as const) {
    g.poly(quad(a0, a1, -w, w))
      .fill(c)
      .stroke({ width: 1.8, color: INK, join: "miter" });
  }
  // Square pillars either side, a rune cut in each.
  const RUNE = 0xe1cd8a;
  for (const side of [-1, 1]) {
    const p = quad(-0.55, 0.3, side * 0.85, side * 1.5);
    g.poly(shade(p, 0.14)).fill({ color: 0x000000, alpha: 0.35 });
    g.poly(p).fill(STONE[0]).stroke({ width: 2.2, color: INK, join: "miter" });
    const c = -0.12;
    const m = side * 1.17;
    g.moveTo(...at(c - 0.22, m)).lineTo(...at(c + 0.22, m));
    g.moveTo(...at(c - 0.22, m - 0.15))
      .lineTo(...at(c, m + 0.12))
      .lineTo(...at(c + 0.22, m - 0.15));
    g.stroke({ width: 2, color: RUNE, cap: "round", join: "round" });
  }
  // A rune band across the façade's foot.
  for (let k = 0; k < 6; k++) {
    const b = -0.95 + k * 0.38;
    if (Math.abs(b) < 0.75) continue;
    g.moveTo(...at(0.62, b - 0.08))
      .lineTo(...at(0.45, b))
      .lineTo(...at(0.62, b + 0.08));
  }
  g.stroke({ width: 1.5, color: RUNE, alpha: 0.9, cap: "round" });
  for (let k = 0; k < 4; k++)
    boulder(
      g,
      at(-1.4 - rnd() * 0.8, (rnd() < 0.5 ? -1 : 1) * (1.6 + rnd() * 1.2)),
      0.18 + rnd() * 0.14,
      ROCK[rnd() < 0.5 ? 0 : 1],
      rnd,
    );
}

/**
 * A wizard's tower fallen to its stump, 4 squares across: a ring of broken wall with the doorway
 * facing the way in and a breach behind, a worn floor, a spiral stair going down in the middle, a
 * faint chalk circle round it, fallen blocks outside.
 */
function drawTowerStump(g: Graphics, at: At, rnd: () => number) {
  const [cx, cy] = at(1.0, 0);
  const [fx, fy] = at(0, 0);
  const door = Math.atan2(fy - cy, fx - cx);
  const breach = door + Math.PI * (0.8 + rnd() * 0.4);
  const R = 2.0 * PX;
  const r = 1.45 * PX;
  g.circle(cx + 0.35 * PX, cy + 0.35 * PX, R).fill({
    color: 0x000000,
    alpha: 0.25,
  });
  // The floor, its flags in rings.
  g.circle(cx, cy, r).fill(STONE[2]);
  g.circle(cx - 0.12 * PX, cy - 0.12 * PX, r * 0.8).fill({
    color: STONE[1],
    alpha: 0.6,
  });
  g.circle(cx, cy, r * 0.72).stroke({ width: 1, color: 0x5f5a52, alpha: 0.6 });
  // A chalk circle round the stair, a few glyphs on it.
  g.circle(cx, cy, 1.05 * PX).stroke({
    width: 1.5,
    color: 0xb9a7e0,
    alpha: 0.55,
  });
  for (let k = 0; k < 5; k++) {
    const t = (k / 5) * Math.PI * 2 + door;
    const [x, y] = [cx + Math.cos(t) * 1.05 * PX, cy + Math.sin(t) * 1.05 * PX];
    g.moveTo(x - 4, y - 4)
      .lineTo(x + 4, y + 4)
      .moveTo(x + 4, y - 4)
      .lineTo(x, y);
  }
  g.stroke({ width: 1.3, color: 0xb9a7e0, alpha: 0.6 });
  // The spiral stair: wedges darkening as they wind down round a newel.
  const sr = 0.75 * PX;
  g.circle(cx, cy, sr).fill(DARK);
  for (let k = 0; k < 7; k++) {
    const t0 = door + Math.PI / 2 + k * 0.62;
    const v = Math.round(0xa0 - k * 0x14);
    g.moveTo(cx, cy)
      .arc(cx, cy, sr * 0.95, t0, t0 + 0.55)
      .closePath()
      .fill((v << 16) | (v << 8) | (v - 10));
  }
  g.circle(cx, cy, sr).stroke({ width: 2, color: INK });
  g.circle(cx, cy, 0.16 * PX)
    .fill(STONE[0])
    .stroke({ width: 1.5, color: INK });
  // The broken wall: ring pieces between the doorway and the breach, of uneven height (tone).
  const gaps = [
    [door, 0.36],
    [breach, 0.55 + rnd() * 0.3],
  ];
  const inGap = (t: number) =>
    gaps.some(
      ([c, w]) => Math.abs(Math.atan2(Math.sin(t - c), Math.cos(t - c))) < w,
    );
  const step = 0.21;
  for (let t = 0; t < Math.PI * 2 - 1e-6; t += step) {
    if (inGap(t + step / 2)) continue;
    const ro = R - (rnd() < 0.3 ? 0.08 * PX : 0);
    const pts = [
      cx + Math.cos(t) * r,
      cy + Math.sin(t) * r,
      cx + Math.cos(t) * ro,
      cy + Math.sin(t) * ro,
    ];
    for (const tt of [t + step * 0.5, t + step])
      pts.push(cx + Math.cos(tt) * ro, cy + Math.sin(tt) * ro);
    pts.push(cx + Math.cos(t + step) * r, cy + Math.sin(t + step) * r);
    // Lit on the side facing the upper left.
    const lit = Math.cos(t + step / 2 + Math.PI * 0.75);
    g.poly(pts)
      .fill(lit > 0.3 ? STONE[0] : lit > -0.3 ? STONE[1] : STONE[2])
      .stroke({ width: 1.6, color: INK, join: "round" });
  }
  // Fallen blocks out through the breach and by the door.
  for (let k = 0; k < 6; k++) {
    const t =
      (k < 4 ? breach : door + Math.PI * 0.35 * (k === 4 ? 1 : -1)) +
      (rnd() - 0.5) * 0.9;
    const d = R + (0.3 + rnd() * 1.1) * PX;
    boulder(
      g,
      [cx + Math.cos(t) * d, cy + Math.sin(t) * d],
      0.16 + rnd() * 0.14,
      STONE[rnd() < 0.5 ? 1 : 2],
      rnd,
    );
  }
}

/**
 * A cave mouth in a hillside: heaped rock round a dark opening, a lip of rock overhanging its back
 * half (its underside in shade), boulders either side. The passage runs in under the overhang.
 */
export function drawCaveMouth(g: Graphics, at: At, rnd: () => number) {
  const jit = Array.from({ length: 22 }, () => (rnd() - 0.5) * 0.18);
  const heap = oval(at, 1.0, 2.1, 2.7, jit);
  g.poly(shade(heap, 0.3)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(heap).fill(ROCK[2]);
  g.poly(inset(heap, 0.85, -0.12 * PX, -0.12 * PX)).fill(ROCK[1]);
  g.poly(heap).stroke({ width: 2.5, color: INK, join: "round" });
  // The opening.
  const mouth = oval(
    at,
    0.5,
    0.85,
    1.45,
    jit.map((v) => v * 0.4),
  );
  g.poly(mouth).fill(DARK).stroke({ width: 2.5, color: INK, join: "round" });
  // The overhang: its front edge bowed out over the opening, the back running into the heap.
  const lip: number[] = [];
  const n = 8;
  for (let k = 0; k <= n; k++) {
    const b = -1.95 + (3.9 * k) / n;
    lip.push(...at(0.45 + 0.3 * (b / 1.95) ** 2 + (rnd() - 0.5) * 0.08, b));
  }
  const under = lip.slice();
  for (let k = n; k >= 0; k--) {
    const b = -1.95 + (3.9 * k) / n;
    lip.push(
      ...at(1.9 + (rnd() - 0.5) * 0.4 - 0.25 * (b / 1.95) ** 2, b * 0.95),
    );
  }
  // Its underside: a band of shade just in front of the lip's edge.
  const band: number[] = [];
  for (let k = 0; k <= n; k++) {
    const b = -1.9 + (3.8 * k) / n;
    band.push(...at(0.28 + 0.3 * (b / 1.9) ** 2, b));
  }
  for (let k = under.length - 2; k >= 0; k -= 2)
    band.push(under[k], under[k + 1]);
  g.poly(band).fill({ color: ROCK[2], alpha: 0.9 });
  g.poly(shade(lip, 0.12)).fill({ color: 0x000000, alpha: 0.3 });
  g.poly(lip).fill(ROCK[1]).stroke({ width: 2.5, color: INK, join: "round" });
  g.poly(inset(lip, 0.6, -0.12 * PX, -0.12 * PX)).fill(ROCK[0]);
  // Cracks on the lip, boulders either side of the mouth.
  for (let k = 0; k < 2; k++) {
    const [x, y] = at(0.9 + rnd() * 0.6, (rnd() - 0.5) * 2.2);
    g.moveTo(x, y).lineTo(x + (rnd() - 0.5) * 26, y + (rnd() - 0.5) * 26);
  }
  g.stroke({ width: 1.3, color: ROCK[2] });
  for (let k = 0; k < 4; k++)
    boulder(
      g,
      at(-0.3 + rnd() * 1.2, (k % 2 ? -1 : 1) * (1.9 + rnd() * 0.9)),
      0.18 + rnd() * 0.15,
      ROCK[rnd() < 0.5 ? 0 : 1],
      rnd,
    );
}
