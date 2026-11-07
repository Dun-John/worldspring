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

/** Outline widths (px): `OUT` round shapes over half a square, `LINE` for parts and joints. */
const OUT = 0.045 * PX;
const LINE = 0.025 * PX;

/**
 * A dressed stone block (`pts`: its corners, px, either winding): filled, a crescent of shade
 * along its edges turned away from the upper-left light, inked.
 */
function block(g: Graphics, pts: number[], tone: number, shadeTone: number, width = OUT) {
  g.poly(pts).fill(tone);
  const n = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let k = 0; k < n; k++) [cx, cy] = [cx + pts[2 * k] / n, cy + pts[2 * k + 1] / n];
  for (let k = 0; k < n; k++) {
    const [x0, y0, x1, y1] = [pts[2 * k], pts[2 * k + 1], pts[(2 * k + 2) % pts.length], pts[(2 * k + 3) % pts.length]];
    // Outward normal of the edge; shaded when it faces down and right.
    let [nx, ny] = [y1 - y0, x0 - x1];
    if (((x0 + x1) / 2 - cx) * nx + ((y0 + y1) / 2 - cy) * ny < 0) [nx, ny] = [-nx, -ny];
    const l = Math.hypot(nx, ny) || 1;
    const face = (nx + ny) / (l * Math.SQRT2);
    if (face < 0.3) continue;
    const d = 0.1 * PX * face;
    const [ix, iy] = [(-nx / l) * d, (-ny / l) * d];
    g.poly([x0, y0, x1, y1, x1 + ix, y1 + iy, x0 + ix, y0 + iy]).fill(shadeTone);
  }
  g.poly(pts).stroke({ width, color: INK, join: "miter" });
}

/** Rune strokes (`u` up the glyph 0-1, `v` across -0.5-0.5) for the lintel. */
const RUNES: [number, number][][] = [
  // ᚠ: a stem, two branches up and out.
  [[0, -0.25], [1, -0.25], [0.55, -0.25], [0.85, 0.3], [0.25, -0.25], [0.55, 0.3]],
  // ᛉ: a stem, two branches up from its middle.
  [[0, 0], [1, 0], [0.5, 0], [1, -0.4], [0.5, 0], [1, 0.4]],
  // ᛏ: an arrow pointing up.
  [[0, 0], [1, 0], [1, 0], [0.6, -0.4], [1, 0], [0.6, 0.4]],
  // ᛞ-like: two stems crossed by a saltire.
  [[0, -0.35], [1, -0.35], [0, 0.35], [1, 0.35], [0, -0.35], [1, 0.35], [1, -0.35], [0, 0.35]],
  // ᛗ-like: two stems, a chevron between them at the top.
  [[0, -0.35], [1, -0.35], [0, 0.35], [1, 0.35], [1, -0.35], [0.6, 0], [0.6, 0], [1, 0.35]],
];

/**
 * A dwarven hall's gate set into the inner face of its crag (the face and the rock behind it are
 * the ground's own), 3 squares wide: a dark doorway recessed in the cliff with steps going down
 * inside, a carved frame of big dressed blocks (a pillar either side, a lintel over the doorway
 * cut with bold runes), rock tucked against the face at its shoulders, and steps leading out.
 */
function drawRuneGate(g: Graphics, at: At, rnd: () => number) {
  const quad = (a0: number, a1: number, b0: number, b1: number) => [
    ...at(a0, b0),
    ...at(a1, b0),
    ...at(a1, b1),
    ...at(a0, b1),
  ];
  // The frame's shadow on the ground and the face.
  g.poly(shade(quad(-0.5, 1.3, -1.6, 1.6), 0.2)).fill({ color: 0x000000, alpha: 0.28 });
  // Rock tucked against the face at the frame's shoulders.
  for (const side of [-1, 1]) {
    const pts = oval(at, 0.6, 0.55, 0.45, Array.from({ length: 12 }, () => (rnd() - 0.5) * 0.2), side * 1.85);
    g.poly(shade(pts, 0.12)).fill({ color: 0x000000, alpha: 0.25 });
    g.poly(pts).fill(ROCK[1]).stroke({ width: OUT, color: INK, join: "round" });
    g.poly(inset(pts, 0.55, -0.1 * PX, -0.1 * PX)).fill(ROCK[0]);
  }
  // Steps leading out, the lower one wider.
  block(g, quad(-1.2, -0.75, -1.3, 1.3), STONE[1], STONE[2]);
  block(g, quad(-0.8, -0.35, -1.12, 1.12), STONE[0], STONE[2]);
  // The doorway: dark, recessed under the lintel, a flight going down inside.
  g.poly(quad(-0.4, 0.75, -0.82, 0.82)).fill(DARK);
  for (let k = 0; k < 3; k++)
    g.poly(quad(-0.38 + k * 0.26, -0.14 + k * 0.26, -0.7, 0.7)).fill([0x625d54, 0x4a463f, 0x34312c][k]);
  g.poly(quad(-0.4, 0.75, -0.82, 0.82)).stroke({ width: OUT, color: INK, join: "miter" });
  // The lintel's shadow falling into the doorway.
  g.poly(quad(0.32, 0.62, -0.82, 0.82)).fill({ color: 0x000000, alpha: 0.4 });
  // Pillars either side: two big blocks each.
  for (const side of [-1, 1]) {
    block(g, quad(-0.5, 0.12, side * 0.82, side * 1.58), STONE[0], STONE[2]);
    block(g, quad(0.12, 0.7, side * 0.82, side * 1.58), STONE[1], STONE[2]);
  }
  // The lintel over the doorway, its joints scored, bold runes cut in it.
  block(g, quad(0.62, 1.3, -1.7, 1.7), STONE[1], STONE[2]);
  for (const b of [-0.9, 0.9]) g.moveTo(...at(0.62, b)).lineTo(...at(1.3, b));
  g.stroke({ width: 0.018 * PX, color: INK });
  const first = Math.floor(rnd() * RUNES.length);
  for (let k = 0; k < 3; k++) {
    const rune = RUNES[(first + k * 2) % RUNES.length];
    const b0 = (k - 1) * 0.55;
    // Up the rune is up the face (into the rock).
    const p = (u: number, v: number) => at(0.74 + u * 0.44, b0 + v * 0.36);
    for (let s = 0; s < rune.length; s += 2) g.moveTo(...p(...rune[s])).lineTo(...p(...rune[s + 1]));
  }
  g.stroke({ width: 0.065 * PX, color: INK, cap: "round", join: "round" });
}

/**
 * A wizard's tower fallen to its stump, 4 squares across: a ring of wall broken off ragged,
 * standing higher on one side (thicker, its shadow cast long over the floor and ground), the
 * doorway facing the way in and a breach behind, a flagged floor with a flight going down in it,
 * dressed blocks fallen out through the breach.
 */
function drawTowerStump(g: Graphics, at: At, rnd: () => number) {
  const [cx, cy] = at(1.0, 0);
  const [fx, fy] = at(0, 0);
  const [ux, uy] = [(cx - fx) / PX, (cy - fy) / PX];
  const door = Math.atan2(fy - cy, fx - cx);
  const breach = door + Math.PI * (0.8 + rnd() * 0.4);
  // Where the wall stands highest: to one side, between the doorway and the breach.
  const high = door + (rnd() < 0.5 ? 1 : -1) * Math.PI * (0.45 + rnd() * 0.15);
  const R = 2.0 * PX;
  const r = 1.45 * PX;
  const diff = (t: number, c: number) => Math.abs(Math.atan2(Math.sin(t - c), Math.cos(t - c)));
  // Height of the wall standing at `t` (0 a course or two, 1 the tallest piece), ragged.
  const tall = (t: number) => {
    const d = diff(t, high);
    return d < 1.1 ? 1 - d / 1.1 : 0;
  };
  g.circle(cx + 0.3 * PX, cy + 0.3 * PX, R).fill({ color: 0x000000, alpha: 0.25 });
  // The floor: flags in rough courses.
  g.circle(cx, cy, r).fill(STONE[1]);
  g.circle(cx - 0.15 * PX, cy - 0.15 * PX, r * 0.75).fill({ color: STONE[0], alpha: 0.35 });
  for (let k = -2; k <= 2; k++) {
    const o = k * 0.55 * PX;
    const w = Math.sqrt(Math.max(0, r * r - o * o)) * 0.92;
    g.moveTo(cx - uy * o - ux * w, cy + ux * o - uy * w).lineTo(cx - uy * o + ux * w, cy + ux * o + uy * w);
    for (let j = -2; j <= 2; j++) {
      const s = (j + (k & 1 ? 0.5 : 0)) * 0.6 * PX;
      if (Math.abs(s) > w * 0.95 || Math.hypot(o + 0.27 * PX, s) > r * 0.95) continue;
      g.moveTo(cx - uy * o + ux * s, cy + ux * o + uy * s).lineTo(cx - uy * (o + 0.55 * PX) + ux * s, cy + ux * (o + 0.55 * PX) + uy * s);
    }
  }
  g.stroke({ width: 0.016 * PX, color: STONE[2] });
  // The flight going down, along the way in: treads darkening to the dark at its foot.
  const quad = (a0: number, a1: number, b0: number, b1: number) => [
    ...at(a0, b0),
    ...at(a1, b0),
    ...at(a1, b1),
    ...at(a0, b1),
  ];
  g.poly(quad(0.45, 1.75, -0.42, 0.42)).fill(DARK);
  for (let k = 0; k < 4; k++)
    g.poly(quad(0.47 + k * 0.26, 0.71 + k * 0.26, -0.36, 0.36)).fill([0x8a847a, 0x6c675e, 0x504c45, 0x37342f][k]);
  g.poly(quad(0.45, 1.75, -0.42, 0.42)).stroke({ width: OUT, color: INK, join: "miter" });
  // The broken wall: runs between the doorway and the breach, broken off a course or two above
  // the ground but standing higher on one side (thicker there, a stepped and ragged top, its
  // shadow cast long), a joint scored every so often.
  const gaps = [
    [door, 0.36],
    [breach, 0.5 + rnd() * 0.25],
  ];
  const inGap = (t: number) => gaps.some(([c, w]) => diff(t, c) < w);
  const N = 72;
  const pt = (t: number, rr: number): [number, number] => [cx + Math.cos(t) * rr, cy + Math.sin(t) * rr];
  const runs: number[][] = [];
  let cur: number[] = [];
  const k0 = Array.from({ length: N }, (_, k) => k).find((k) => inGap(((k + 0.5) / N) * Math.PI * 2)) ?? 0;
  for (let j = 1; j <= N; j++) {
    const k = (k0 + j) % N;
    if (inGap(((k + 0.5) / N) * Math.PI * 2)) {
      if (cur.length) runs.push(cur);
      cur = [];
    } else cur.push(k);
  }
  if (cur.length) runs.push(cur);
  const jag = Array.from({ length: N + 1 }, () => (rnd() - 0.5) * 0.1 * PX);
  // A ring band over `ks` (segment indices) from `ri` to `ro(t)` (px).
  const band = (ks: number[], ri: (t: number) => number, ro: (t: number, k: number) => number) => {
    const out: number[] = [];
    const ts = [...ks.map((k) => (k / N) * Math.PI * 2), ((ks[ks.length - 1] + 1) / N) * Math.PI * 2];
    ts.forEach((t, i) => out.push(...pt(t, ro(t, ks[Math.min(i, ks.length - 1)] + (i === ks.length ? 1 : 0)))));
    for (let i = ts.length - 1; i >= 0; i--) out.push(...pt(ts[i], ri(ts[i])));
    return out;
  };
  const low = runs.map((ks) => band(ks, () => r, (_, k) => R - 0.08 * PX + jag[k]));
  // The standing piece: where the wall is tall, cut off ragged at its ends.
  const highKs: number[][] = [];
  for (const ks of runs) {
    let part: number[] = [];
    for (const k of ks) {
      if (tall(((k + 0.5) / N) * Math.PI * 2) > 0.25 + 0.15 * ((k * 7) % 3)) part.push(k);
      else if (part.length) [highKs[highKs.length], part] = [part, []];
    }
    if (part.length) highKs.push(part);
  }
  const standing = highKs
    .filter((ks) => ks.length > 1)
    .map((ks) => band(ks, (t) => r - 0.12 * PX * tall(t), (t, k) => R + 0.22 * PX * tall(t) + jag[k] * 0.6));
  for (const p of low) g.poly(shade(p, 0.14)).fill({ color: 0x000000, alpha: 0.28 });
  for (const p of standing) g.poly(shade(p, 0.55)).fill({ color: 0x000000, alpha: 0.3 });
  for (const p of low) g.poly(p).fill(STONE[2]).stroke({ width: OUT, color: INK, join: "round" });
  for (const p of standing) {
    g.poly(p).fill(STONE[0]);
    g.poly(inset(p, 0.9, 0.06 * PX, 0.06 * PX)).fill({ color: STONE[1], alpha: 0.6 });
    g.poly(p).stroke({ width: OUT, color: INK, join: "round" });
  }
  // Joints across the wall.
  for (const ks of runs)
    for (const k of ks) {
      if (k % 3 !== 0) continue;
      const t = (k / N) * Math.PI * 2;
      const h = tall(t);
      g.moveTo(...pt(t, r - 0.12 * PX * h)).lineTo(...pt(t, R + 0.2 * PX * h));
    }
  g.stroke({ width: 0.018 * PX, color: INK, alpha: 0.8 });
  // Dressed blocks fallen out through the breach (one inside, by the wall).
  for (let k = 0; k < 6; k++) {
    const t = k === 0 ? breach + (rnd() < 0.5 ? -1 : 1) * 0.9 : breach + (rnd() - 0.5) * 1.1;
    const d = k === 0 ? r - 0.3 * PX : R + (0.3 + rnd() * 1.3) * PX;
    const [bx, by] = [cx + Math.cos(t) * d, cy + Math.sin(t) * d];
    const [w, l, rot] = [(0.12 + rnd() * 0.06) * PX, (0.18 + rnd() * 0.08) * PX, rnd() * Math.PI];
    const [c, s] = [Math.cos(rot), Math.sin(rot)];
    const unitSq = [-1, -1, 1, -1, 1, 1, -1, 1];
    const corners: number[] = [];
    for (let i = 0; i < 4; i++) {
      const [px, py] = [unitSq[2 * i] * l, unitSq[2 * i + 1] * w];
      corners.push(bx + px * c - py * s, by + px * s + py * c);
    }
    g.poly(shade(corners, 0.1)).fill({ color: 0x000000, alpha: 0.28 });
    block(g, corners, STONE[rnd() < 0.5 ? 0 : 1], STONE[2], LINE * 1.4);
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
