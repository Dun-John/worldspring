// Curtain walls, wall towers and gatehouses on the battlemap, drawn in code like the atlas and the
// bridges (assets/STYLE_GUIDE.md): warm stone in a few flat tones lit from the north-west, bold
// ink, blocks a little out of true. A wall is a flagged walk between two crenellated parapets;
// a tower a flagged fighting top inside its merlons with the spiral stair's hatch; a gate the
// passage between its two towers under an arch at each end, portcullis grooves and the doors
// swung back. A ruin's works lose merlons, crack and moss over, spill rubble where they broke.
// A palace's forecourt has a low coped garden wall instead, its gate between two solid piers with
// ball finials, the leaves (wrought iron or timber) standing open; its court's walks are gravel.
// Town walls take their culture group's look (item 104, `lookOf`): the stone's tones (`TONES`), the merlons'
// profile and rhythm, the towers' shape (round, squat, D, octagon, square, a log blockhouse, a tapering earth
// bastion), the wall's material (stone, a palisade of stakes, rammed earth) and the gatehouse's form.
import type { Graphics } from 'pixi.js';
import type { Shape } from '../gen/battlePrep';
import { PX } from './atlas';
import { FT, INK, LX, LY, mix, MOSS, PLUM, rngAt, STONE, WOOD } from './bridges';
import { gravelPatch } from './furniture';

/** The walk and a tower's top: flags between the stone's mid and light tones. */
export const WALK = 0xb0a692;
/** Iron: dark, mid, light. */
const IRON = [0x4a4e56, 0x6e7278, 0x9aa0a6];
/** A parapet's width (px): merlons stand on it. */
export const PARAPET = FT * 1.6;

/** Flags on a wall's or tower's last point (`battlemap::WALL_*`). */
const BARE_A = 1;
const BARE_B = 2;
const GATE = 1;
const RUINED = 4;
/** A square tower (a castle's): its sides run at the angle after the flags. */
const SQUARE = 8;

const flagsOf = (s: Shape, at: number) => (s.pts.length > at ? Math.round(s.pts[at] / PX) : 0);

/** Tower shapes in a tower's style code (`town::wallstyle::TowerShape`, bits 3-4 of `WallStyle::code`). */
const SHAPE_SQUARE = 1;
const SHAPE_D = 2;
const SHAPE_OCTAGON = 3;
/** A D tower's flat back stands this far in from its curtain's line (ft; `wallstyle::D_BACK`). */
const D_BACK = 4;
/** A wall's, tower's or gate's style (`WallStyle::code`: group, tower shape, material, merlons, stone,
 * outline, gatehouse), on the point after its flags; 0 (the West's round, today's look) where none. */
export const styleOf = (s: Shape, at: number) => flagsOf(s, at);
/** A tower's shape by its style. */
export const towerShape = (s: Shape) => (styleOf(s, 4) >> 3) & 3;
/** A gatehouse's form as built (`town::gatehouse::House::code`), on a gate and its towers after the
 * style: 0 none, then square tower, twin drums, arched, bent, log. */
export const GATE_SQUARE = 1;
export const GATE_ARCHED = 3;
export const GATE_BENT = 4;
export const GATE_LOG = 5;
export const gateForm = (s: Shape) => flagsOf(s, 5);
/** How much a direction faces the light, -1 .. 1. */
const lit = (x: number, y: number) => (x * LX + y * LY) / (Math.hypot(x, y) || 1);

/** Stone blocks (outlines) with a drop shadow: the block's sides in a shaded tone, its top in
 * `tone` set toward the light, a bold ink outline. */
function blocks(g: Graphics, quads: number[][], tone: () => number, shadow = FT * 0.35) {
  for (const b of quads) g.poly(b.map((v) => v + shadow));
  if (quads.length) g.fill({ color: 0x000000, alpha: 0.25 });
  for (const b of quads) {
    const t = tone();
    const n = b.length / 2;
    let [x, y] = [0, 0];
    for (let k = 0; k < n; k++) [x, y] = [x + b[k * 2] / n, y + b[k * 2 + 1] / n];
    g.poly(b).fill(mix(t, PLUM, 0.3));
    g.poly(b.map((v, k) => (k % 2 ? y + (v - y) * 0.72 + LY * FT * 0.18 : x + (v - x) * 0.72 + LX * FT * 0.18))).fill(t);
    g.poly(b).stroke({ width: 1.8, color: INK, join: 'round' });
  }
}

/** Loose rubble round (x, y): `n` blocks of mixed sizes within `spread` px. */
function rubble(x: number, y: number, n: number, spread: number, rnd: () => number, big = 1): number[][] {
  const out: number[][] = [];
  for (let k = 0; k < n; k++) {
    const [cx, cy] = [x + (rnd() - 0.5) * 2 * spread, y + (rnd() - 0.5) * 2 * spread];
    const r = FT * (0.55 + rnd() * 0.75) * big;
    const t = rnd() * Math.PI;
    const pts: number[] = [];
    for (let m = 0; m < 5; m++) {
      const a = t + (m / 5) * Math.PI * 2;
      const rr = r * (0.75 + rnd() * 0.4);
      pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    out.push(pts);
  }
  return out;
}

/** A collapse in a ruin's ring: a slope of tumbled stone over the ring where it fell (angle `ang` from (x, y),
 * `r` out to the face, `w` ft across), its outline jagged and inked, a few big blocks heaped on it. */
function breach(g: Graphics, x: number, y: number, r: number, ang: number, w: number, rnd: () => number, fallen: number[][]) {
  const half = (w * FT) / 2;
  const [ux, uy] = [Math.cos(ang), Math.sin(ang)];
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  const pts: number[] = [...at(r - PARAPET * 2.2, -half * 0.8)];
  for (let k = 0; k <= 4; k++) pts.push(...at(r + FT * (0.4 + rnd() * 1.6), -half + (k / 4) * half * 2));
  pts.push(...at(r - PARAPET * 2.2, half * 0.8));
  g.poly(pts).fill(mix(ST[0], ST[1], 0.5)).stroke({ width: LINE, color: INK, join: 'round' });
  fallen.push(...rubble(...at(r + FT * 0.2, 0), 3, half * 0.6, rnd, 2.1));
  fallen.push(...rubble(...at(r - FT * 1.6, (rnd() - 0.5) * half), 2, half * 0.3, rnd, 1.7));
}

/** The tone of a crenel's notch: the stone's dark lifted toward its mid, so no merlon reads near ink. */
const notchTone = () => mix(ST[0], ST[1], 0.5);
/** Whether the stone being drawn is rose brick (it takes coursing lines and a crisper edge). */
const brick = () => ST === TONES[3];

const stoneTone = (rnd: () => number) => () => (rnd() < 0.35 ? ST[2] : ST[1]);

/** Outlines by size (assets/STYLE_GUIDE.md): a shape over about half a square across, and smaller parts. */
const OUT = 0.045 * PX;
const LINE = 0.025 * PX;

/** A wall's, tower's or gate's look, decoded from its style (`WallStyle::code`): its culture group (0 North,
 * 1 West, 2 South, 3 Dry, 4 Steppe), tower shape, material, merlons, stone, gatehouse; rich and poor are in the
 * choices themselves. */
export interface Look {
  group: number;
  shape: number;
  material: number;
  merlons: number;
  stone: number;
  gatehouse: number;
}
/** Materials (`wallstyle::Material`). */
export const MAT_STONE = 0;
export const MAT_PALISADE = 1;
export const MAT_EARTH = 2;
/** Merlons (`wallstyle::Merlon`). */
export const M_BROAD = 0;
export const M_SQUARE = 1;
export const M_SWALLOWTAIL = 2;
export const M_STEPPED = 3;
export const M_STAKES = 4;
export const M_ROUNDED = 5;
/** The West's limestone, today's look: a shape from before styles came, or none given. */
const WEST: Look = { group: 1, shape: 0, material: MAT_STONE, merlons: M_SQUARE, stone: 1, gatehouse: 1 };
/** A shape's look by the style on point `at` (a wall's 6, a tower's 4, a gate's 4). */
export function lookOf(s: Shape, at: number): Look {
  if (s.pts.length <= at) return WEST;
  const c = flagsOf(s, at);
  return { group: c & 7, shape: (c >> 3) & 3, material: (c >> 5) & 3, merlons: (c >> 7) & 7, stone: (c >> 10) & 7, gatehouse: (c >> 15) & 7 };
}
/** Each stone's four tones (dark, mid, light, lit edge; `wallstyle::STONE_*`), matte, lit from the upper left,
 * in today's value range: blue-grey granite, buff limestone (today's), honey sandstone, rose brick, ochre-pink
 * mud brick, timber, pale tan rammed earth. */
export const TONES: number[][] = [
  [0x5c626a, 0x858c95, 0xadb3b9, 0xd3d8db],
  STONE,
  [0x7c6042, 0xad8a5d, 0xd1b07e, 0xead4a4],
  [0x7a5048, 0xa67466, 0xc69a89, 0xe2c2b2],
  [0x7e5942, 0xae8466, 0xcca27f, 0xe6c6a6],
  [0x5e4024, 0x7d5a3c, 0xa58155, 0xc6a372],
  [0x8a7658, 0xb6a17e, 0xd2c19d, 0xe8ddc1],
];
/** The walk's and a top's flags (or planks, or packed earth) by stone. */
const WALKS = [0x9da3a8, WALK, 0xc0a172, 0xb68f7f, 0xbf987c, 0x94714a, 0xc7b58e];
/** The tones, walk and merlons the works being drawn take (`use`): today's while nothing is set. */
let ST = STONE;
let WK = WALK;
let MER = M_SQUARE;
/** Takes a look's tones and merlons for what is drawn next; none, today's. */
function use(l: Look | null) {
  ST = l ? (TONES[l.stone] ?? STONE) : STONE;
  WK = l ? (WALKS[l.stone] ?? WALK) : WALK;
  MER = l ? l.merlons : M_SQUARE;
}
/** Merlons' rhythm by profile (ft): a merlon's length (least, more at random), a crenel's. Broad ones long and
 * wide apart, the Dry's stepped ones short and close. */
const RHYTHM: [number, number, number, number][] = [
  [3.8, 0.6, 2.3, 0.4],
  [2.4, 0.6, 1.4, 0.3],
  [4.0, 0.4, 2.2, 0.3],
  [3.4, 0.4, 1.8, 0.2],
  [3.0, 0.5, 2.0, 0.3],
  [2.6, 0.4, 1.6, 0.2],
];
const rhythm = () => RHYTHM[MER] ?? RHYTHM[M_SQUARE];
/** The marks a merlon's profile leaves on its top, flushed by `crenelFill`: a swallowtail's fork, a stepped
 * merlon's upper step, a rounded one's lit crown. */
const FORKS: number[][] = [];
const STEPS: number[][] = [];
const CROWNS: number[][] = [];

/** A point `a` along a straight run of parapet and `d` across it. */
type At = (a: number, d: number) => [number, number];

/** A merlon's top from `m0` to `m1` along a run, `d0` its walk side to `d1` its face, marked by its profile. */
function merlonMark(at: At, m0: number, m1: number, d0: number, d1: number) {
  if (m1 - m0 < FT * 0.9) return;
  const side = Math.sign(d1 - d0) || 1;
  const [i0, i1] = [d0 + side * 1.5, d1 - side * 1.5];
  const mid = (m0 + m1) / 2;
  if (MER === M_SWALLOWTAIL) {
    // (The fork: a V cut down the merlon's middle, wide at the face, narrow at the walk.)
    const [w0, w1] = [FT * 0.3, FT * 0.85];
    FORKS.push([...at(mid - w0, i0), ...at(mid + w0, i0), ...at(mid + w1, i1), ...at(mid - w1, i1)]);
  } else if (MER === M_STEPPED) {
    const [e, k] = [FT * 0.7, FT * 0.45];
    STEPS.push([...at(m0 + e, i0 + side * k), ...at(m1 - e, i0 + side * k), ...at(m1 - e, i1 - side * k), ...at(m0 + e, i1 - side * k)]);
  } else if (MER === M_ROUNDED) {
    const d = (d0 + d1) / 2;
    CROWNS.push([...at(m0 + FT * 0.55, d), ...at(m1 - FT * 0.55, d)]);
  }
}

/** The crenels notched through a straight run of parapet from `a0` to `a1` (`d0` its walk side,
 * `d1` its outer face), in the merlons' rhythm (`RHYTHM`) a little out of step, each merlon marked by its
 * profile; a ruin's wider here and there, a stone fallen outside. Pushed to `crenels`. */
function crenelRun(at: At, a0: number, a1: number, d0: number, d1: number, rnd: () => number, crenels: number[][], ruined = false, fallen: number[][] = []) {
  const side = Math.sign(d1 - d0);
  const [m, mr, c0, cr] = rhythm();
  let last = a0;
  for (let a = a0 + FT * (m * 0.5 + rnd() * m * 0.5); a < a1 - FT * 1.6; ) {
    let c = FT * (c0 + rnd() * cr);
    if (ruined && rnd() < 0.4) {
      c += FT * (rnd() < 0.5 ? 9 + rnd() * 9 : 2.5 + rnd() * 2);
      if (rnd() < 0.5) fallen.push(...rubble(...at(a + c / 2, d1 + side * FT * 1.4), 1, FT * 0.8, rnd, 1.8));
    }
    const b = Math.min(a + c, a1 - FT * 0.4);
    crenels.push([...at(a, d0 + side * 1.5), ...at(b, d0 + side * 1.5), ...at(b, d1 - side * 1.5), ...at(a, d1 - side * 1.5)]);
    merlonMark(at, last, a, d0, d1);
    last = b;
    a = b + FT * (m + rnd() * mr);
  }
  merlonMark(at, last, a1, d0, d1);
}

/** Crenels: dark notches through the coping, inked; then the merlons' marks (`merlonMark`). */
function crenelFill(g: Graphics, crenels: number[][]) {
  for (const q of crenels) g.poly(q);
  if (crenels.length) g.fill(notchTone()).stroke({ width: LINE * 0.8, color: INK, alpha: 0.9, join: 'round' });
  marksFill(g);
}

/** The merlons' marks pushed since last time (`merlonMark`). */
function marksFill(g: Graphics) {
  for (const q of FORKS) g.poly(q);
  if (FORKS.length) g.fill(mix(ST[1], PLUM, 0.2)).stroke({ width: LINE * 0.8, color: INK, alpha: 0.9, join: 'round' });
  for (const q of STEPS) g.poly(q);
  if (STEPS.length) g.fill(ST[3]).stroke({ width: LINE * 0.8, color: INK, alpha: 0.85, join: 'round' });
  for (const q of CROWNS) g.moveTo(q[0], q[1]).lineTo(q[2], q[3]);
  if (CROWNS.length) g.stroke({ width: FT * 0.45, color: ST[3], alpha: 0.85, cap: 'round' });
  FORKS.length = STEPS.length = CROWNS.length = 0;
}

/** A convex outline's side as a parapet run: `at` from its first corner (`d` inward), its length,
 * how far a line `d` in starts (`k0 * d`) and ends short (`k1 * d`) of its corners, whether it
 * faces the light. */
interface Side {
  /** The outline's side this runs along. */
  i: number;
  at: At;
  len: number;
  k0: number;
  k1: number;
  toLight: boolean;
}

/** The sides of a convex outline (either turn); those in `open` (by index) carry no parapet, and
 * their neighbours' parapets run square to the outline there. */
function sidesOf(pts: number[], open: number[] = []): Side[] {
  const m = pts.length / 2;
  let area = 0;
  for (let i = 0; i < m; i++) {
    const j = (i + 1) % m;
    area += pts[i * 2] * pts[j * 2 + 1] - pts[j * 2] * pts[i * 2 + 1];
  }
  const turn = area < 0 ? -1 : 1;
  const raw = Array.from({ length: m }, (_, i) => {
    const [ax, ay] = [pts[i * 2], pts[i * 2 + 1]];
    const [bx, by] = [pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const len = Math.hypot(bx - ax, by - ay);
    const [ux, uy] = len > 1e-6 ? [(bx - ax) / len, (by - ay) / len] : [1, 0];
    const [nx, ny] = [-uy * turn, ux * turn];
    return { i, ax, ay, ux, uy, nx, ny, len };
  }).filter((e) => e.len > 1e-6);
  // tan of half the turn at the corner between two sides: how far a line `d` in starts from it.
  const half = (p: (typeof raw)[0], q: (typeof raw)[0]) => (turn * (p.ux * q.uy - p.uy * q.ux)) / Math.max(1e-3, 1 + p.ux * q.ux + p.uy * q.uy);
  return raw.flatMap((e, k) => {
    if (open.includes(e.i)) return [];
    const [p, q] = [raw[(k + raw.length - 1) % raw.length], raw[(k + 1) % raw.length]];
    return [
      {
        i: e.i,
        at: (a: number, d: number): [number, number] => [e.ax + e.ux * a + e.nx * d, e.ay + e.uy * a + e.ny * d],
        len: e.len,
        k0: open.includes(p.i) ? 0 : half(p, e),
        k1: open.includes(q.i) ? 0 : half(e, q),
        toLight: lit(-e.nx, -e.ny) > 0,
      },
    ];
  });
}

/** Where a line from (px, py) along (dx, dy) runs inside the convex outline `pts` (its `t` range). */
function clipLine(pts: number[], px: number, py: number, dx: number, dy: number): [number, number] {
  const m = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
  let [t0, t1] = [-Infinity, Infinity];
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    let [nx, ny] = [-(by - ay), bx - ax];
    if ((cx - ax) * nx + (cy - ay) * ny < 0) [nx, ny] = [-nx, -ny];
    // Inside where (p + t d - a) . n >= 0.
    const num = (px - ax) * nx + (py - ay) * ny;
    const den = dx * nx + dy * ny;
    if (Math.abs(den) < 1e-9) {
      if (num < 0) return [0, 0];
    } else if (den > 0) t0 = Math.max(t0, -num / den);
    else t1 = Math.min(t1, -num / den);
  }
  return t1 > t0 ? [t0, t1] : [0, 0];
}

/**
 * A flat top's floor inside a convex outline: flags in courses along its longest side, their
 * joints staggered course to course (a tower top's).
 */
export function flagFloor(g: Graphics, pts: number[], rnd: () => number) {
  g.poly(pts).fill(WK);
  const m = pts.length / 2;
  let [ux, uy, best] = [1, 0, 0];
  for (let i = 0; i < m; i++) {
    const [dx, dy] = [pts[((i + 1) % m) * 2] - pts[i * 2], pts[((i + 1) % m) * 2 + 1] - pts[i * 2 + 1]];
    const l = Math.hypot(dx, dy);
    if (l > best) [ux, uy, best] = [dx / l, dy / l, l];
  }
  const [vx, vy] = [-uy, ux];
  let [lo, hi] = [Infinity, -Infinity];
  for (let i = 0; i < m; i++) {
    const o = pts[i * 2] * vx + pts[i * 2 + 1] * vy;
    [lo, hi] = [Math.min(lo, o), Math.max(hi, o)];
  }
  const row = FT * 2.8;
  for (let o = lo + row * (0.3 + rnd() * 0.7); o < hi; o += row) {
    const [px, py] = [vx * o, vy * o];
    const [t0, t1] = clipLine(pts, px, py, ux, uy);
    if (t1 <= t0) continue;
    g.moveTo(px + ux * t0, py + uy * t0).lineTo(px + ux * t1, py + uy * t1);
    for (let t = t0 + FT * (0.5 + rnd() * 2); t < t1; t += FT * (2.4 + rnd() * 1.4)) {
      const [qx, qy] = [px + ux * t, py + uy * t];
      const reach = Math.min(row, clipLine(pts, qx, qy, vx, vy)[1]);
      if (reach > 0) g.moveTo(qx, qy).lineTo(qx + vx * reach, qy + vy * reach);
    }
  }
  if (brick()) g.stroke({ width: LINE * 0.8, color: ST[3], alpha: 0.85 });
  else g.stroke({ width: 1.2, color: ST[0], alpha: 0.4 });
}

/**
 * Battlements round a convex outline (a keep's, a flat-topped building's), in the curtain's
 * style: inside the outline a band of coping lit on its edge toward the light, crenels notched
 * through it, inked along the walk and round the outline; the shade of the lit sides' parapets
 * on what lies inside; `walk` px of wall walk inside the parapet given, its flags' joints across
 * it. Sides in `open` (by index) have none (where a higher piece stands against them); sides in
 * `plain` a coping with no crenels (a wall between two flat tops: one wall, not a fence); a ruin's merlons
 * gone here and there, their stones to `fallen`.
 */
export function parapetRing(g: Graphics, pts: number[], rnd: () => number, walk = 0, open: number[] = [], plain: number[] = [], ruined = false, fallen: number[][] = []) {
  const sides = sidesOf(pts, open);
  const band = (s: Side, d0: number, d1: number) => [...s.at(s.k0 * d0, d0), ...s.at(s.len - s.k1 * d0, d0), ...s.at(s.len - s.k1 * d1, d1), ...s.at(s.k0 * d1, d1)];
  const P = PARAPET;
  if (walk > 0) {
    for (const s of sides) {
      const [a0, a1] = [s.k0 * (P + walk), s.len - s.k1 * (P + walk)];
      for (let a = a0 + rnd() * FT * 2; a < a1; a += FT * (2.4 + rnd() * 1.4)) {
        g.moveTo(...s.at(a, P)).lineTo(...s.at(a + (rnd() - 0.5) * FT * 0.4, P + walk));
        if (rnd() < 0.3 && walk > FT * 2) g.moveTo(...s.at(a, P + walk / 2)).lineTo(...s.at(a + FT * (1.6 + rnd()), P + walk / 2 + (rnd() - 0.5) * FT * 0.3));
      }
    }
    g.stroke({ width: 1.3, color: ST[0], alpha: 0.6 });
  }
  for (const s of sides) if (s.toLight) g.poly(band(s, P, P + FT * 0.6));
  g.fill({ color: PLUM, alpha: 0.25 });
  for (const s of sides) g.poly(band(s, 0, P));
  g.fill(ST[2]);
  for (const s of sides) {
    const e = s.toLight ? FT * 0.3 : P - FT * 0.3;
    g.moveTo(...s.at(s.k0 * e, e)).lineTo(...s.at(s.len - s.k1 * e, e));
  }
  g.stroke({ width: FT * 0.3, color: ST[3], alpha: 0.8 });
  const crenels: number[][] = [];
  for (const s of sides) if (!plain.includes(s.i)) crenelRun(s.at, s.k0 * P, s.len - s.k1 * P, P, 0, rnd, crenels, ruined, fallen);
  for (const s of sides) g.moveTo(...s.at(s.k0 * P, P)).lineTo(...s.at(s.len - s.k1 * P, P));
  g.stroke({ width: 1.8, color: INK });
  crenelFill(g, crenels);
  g.poly(pts).stroke({ width: 2.5, color: INK, join: 'round' });
}

/**
 * A stair's hatch: an opening with steps going down into the dark, its timber lid lying open
 * beside it. `hp(a, b)`: a point `a` along the hatch (`hl` long) and `b` across it (`hw` half
 * wide) from its middle; the lid lies on its +b side.
 */
export function stairHatch(g: Graphics, hp: At, hl: number, hw: number) {
  const hq = (a0: number, a1: number, b0: number, b1: number) => [...hp(a0, b0), ...hp(a1, b0), ...hp(a1, b1), ...hp(a0, b1)];
  g.poly(hq(-hl / 2, hl / 2, -hw, hw)).fill(0x221d19);
  for (let k = 0; k < 4; k++) {
    const a = hl / 2 - (k + 1) * hl * 0.21;
    g.poly(hq(a, a + hl * 0.21, -hw + 1.5, hw - 1.5)).fill(mix(ST[1], 0x221d19, k * 0.24)).stroke({ width: 1, color: 0x221d19, alpha: 0.6 });
  }
  g.poly(hq(-hl / 2, hl / 2, -hw, hw)).stroke({ width: 2.4, color: INK, join: 'round' });
  const lid = hq(-hl / 2, hl / 2, hw + FT * 0.3, hw + FT * 0.3 + hw * 2);
  g.poly(lid.map((v) => v + FT * 0.25)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(lid).fill(WOOD[1]);
  g.poly(hq(-hl / 2, hl / 2, hw + FT * 0.3, hw + FT * 0.3 + hw * 0.7)).fill({ color: WOOD[2], alpha: 0.7 });
  for (const t of [0.33, 0.66]) g.moveTo(...hp(-hl / 2, hw + FT * 0.3 + hw * 2 * t)).lineTo(...hp(hl / 2, hw + FT * 0.3 + hw * 2 * t));
  g.stroke({ width: 1.2, color: WOOD[0] });
  for (const t of [0.22, 0.78]) g.moveTo(...hp(-hl / 2 + hl * t, hw + FT * 0.3)).lineTo(...hp(-hl / 2 + hl * t, hw + FT * 0.3 + hw * 2));
  g.stroke({ width: FT * 0.28, color: IRON[0] });
  g.poly(lid).stroke({ width: 1.8, color: INK, join: 'round' });
}

/**
 * A stair hatch in a convex flat top's corner (the one at the end of the side facing most away
 * from the light), across the walk inside the parapet, its lid laid back along it; `room` how far
 * in from the outline there is flat to put it (none if too little).
 */
export function cornerHatch(g: Graphics, pts: number[], room: number) {
  const sides = sidesOf(pts);
  if (!sides.length) return;
  let s = sides[0];
  let low = Infinity;
  for (const q of sides) {
    const [x0, y0] = q.at(0, 0);
    const [x1, y1] = q.at(1, 0);
    const [x2, y2] = q.at(0, 1);
    const l = lit(-(x2 - x0), -(y2 - y0)) + lit(x1 - x0, y1 - y0) * 0.5;
    if (l < low) [s, low] = [q, l];
  }
  const hw = FT * 1.6;
  const hl = Math.min(FT * 4.5, room - PARAPET - FT * 1.6);
  if (hl < FT * 2.5 || s.len < (PARAPET + FT * 1.6 + hw * 4) * 2) return;
  const along = s.len - s.k1 * PARAPET - FT * 0.8 - hw;
  const depth = PARAPET + FT * 0.8 + hl / 2;
  stairHatch(g, (a, b) => s.at(along - b, depth - a), hl, hw);
}

/**
 * A round flat top (a wall tower's, a turret's, a wizard's tower's) at (x, y), `r` its radius: a
 * ring of masonry a little out of round, a flagged fighting top inside it, the stair's hatch, the
 * parapet with its crenels round it (a ruin's broken over a stretch, its stones to `fallen`) and
 * the ring's lit and shaded faces. Its shadow is the caller's. Returns where the hatch faces and
 * where a ruin's parapet broke.
 */
export function roundTop(g: Graphics, x: number, y: number, r: number, rnd: () => number, ruined = false, fallen: number[][] = [], smooth = false) {
  const ring: number[] = [];
  const n = 22;
  for (let k = 0; k < n; k++) {
    const t = (k / n) * Math.PI * 2;
    const rr = r * (1 + (rnd() - 0.5) * 0.035);
    ring.push(x + Math.cos(t) * rr, y + Math.sin(t) * rr);
  }
  // (`smooth`, a keep's turret or a tower's top that is no wall's: a true circle, so its coping joins the walls it stands on.)
  if (smooth) g.circle(x, y, r).fill(ST[1]).stroke({ width: 3, color: INK });
  else g.poly(ring).fill(ST[1]).stroke({ width: 3, color: INK, join: 'round' });
  // The fighting top: flags inside the parapet, their joints, the parapet's shade on the lit side.
  const rf = r - PARAPET;
  g.circle(x, y, rf).fill(WK);
  const turn = rnd() * Math.PI;
  const [cu, su] = [Math.cos(turn), Math.sin(turn)];
  // Courses of flags across the top (turned at random), their joints staggered course to course.
  const row = FT * 2.8;
  for (let o = -rf + row * (0.3 + rnd() * 0.7); o < rf; o += row) {
    const h = Math.sqrt(Math.max(0, rf * rf - o * o));
    const [px, py] = [x - su * o, y + cu * o];
    g.moveTo(px - cu * h, py - su * h).lineTo(px + cu * h, py + su * h);
    const h1 = Math.sqrt(Math.max(0, rf * rf - (o + row) * (o + row)));
    const span = Math.min(h, h1 || h);
    for (let t = -span + FT * (0.5 + rnd() * 2); t < span; t += FT * (2.4 + rnd() * 1.4)) {
      g.moveTo(px + cu * t, py + su * t).lineTo(px + cu * t - su * Math.min(row, rf - o), py + su * t + cu * Math.min(row, rf - o));
    }
  }
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.4 });
  g.moveTo(x + Math.cos(Math.PI * 0.85) * (rf - FT * 0.3), y + Math.sin(Math.PI * 0.85) * (rf - FT * 0.3));
  g.arc(x, y, rf - FT * 0.3, Math.PI * 0.85, Math.PI * 1.65);
  g.stroke({ width: FT * 0.6, color: PLUM, alpha: 0.25 });
  // The stair's hatch near the rim.
  const th = rnd() * Math.PI * 2;
  const [ex, ey] = [Math.cos(th), Math.sin(th)];
  const [hl, hw] = [Math.min(FT * 4.5, rf * 0.5), FT * 1.6];
  const hc = rf - hl / 2 - FT * 0.8;
  stairHatch(g, (a, b) => [x + ex * (hc + a) - ey * b, y + ey * (hc + a) + ex * b], hl, hw);
  const lost = roundParapet(g, x, y, r, turn, rnd, ruined, fallen);
  return { ex, ey, rf, lost };
}

/**
 * A round parapet at (x, y) from `r` in to `r - PARAPET`: one ring of coping, crenels notched
 * through it (from angle `turn`; a ruin's broken over a stretch, returned), inked along the walk,
 * the ring's lit and shaded faces.
 */
export function roundParapet(g: Graphics, x: number, y: number, r: number, turn: number, rnd: () => number, ruined = false, fallen: number[][] = []) {
  const rf = r - PARAPET;
  const [rm0, rm1, rc0, rc1] = rhythm();
  const pitch = FT * (rm0 + rm1 / 2 + rc0 + rc1 / 2);
  const m = Math.max(6, Math.round((Math.PI * 2 * (r - PARAPET / 2)) / pitch));
  const step = (Math.PI * 2) / m;
  // (The crenel's share of each step, and a merlon's marks between: `at` round the ring, `a` along at mid-parapet.)
  const share = Math.min(0.5, (FT * (rc0 + rc1 / 2)) / pitch);
  const rm = r - PARAPET / 2;
  const at: At = (a, d) => [x + Math.cos(a / rm) * d, y + Math.sin(a / rm) * d];
  const lost = ruined ? rnd() * Math.PI * 2 : NaN;
  const [ri, ro] = [rf + 1.5, r - 1.5];
  g.circle(x, y, (rf + r) / 2).stroke({ width: r - rf, color: ST[2] });
  const notch = (t0: number, t1: number) => {
    const q: number[] = [];
    for (const t of [t0, (t0 + t1) / 2, t1]) q.push(x + Math.cos(t) * ri, y + Math.sin(t) * ri);
    for (const t of [t1, (t0 + t1) / 2, t0]) q.push(x + Math.cos(t) * ro, y + Math.sin(t) * ro);
    g.poly(q);
  };
  for (let k = 0; k < m; k++) {
    const t0 = k * step + turn;
    const gone = ruined && (Math.abs(Math.atan2(Math.sin(t0 - lost), Math.cos(t0 - lost))) < 0.8 || rnd() < 0.25);
    if (gone && rnd() < 0.5) fallen.push(...rubble(x + Math.cos(t0) * (rf - FT), y + Math.sin(t0) * (rf - FT), 1, FT * 0.6, rnd));
    notch(gone ? t0 - step * 0.3 : t0 + step * (0.95 - share), t0 + step * 0.95);
    if (!gone) merlonMark(at, (t0 - step * 0.05) * rm, (t0 + step * (0.95 - share)) * rm, rf, r);
  }
  g.fill(notchTone()).stroke({ width: LINE * 0.8, color: INK, alpha: 0.9, join: 'round' });
  marksFill(g);
  g.circle(x, y, rf).stroke({ width: 1.8, color: INK });
  // The ring's lit and shaded faces.
  g.moveTo(x + Math.cos(Math.PI * 0.9) * (r - 2), y + Math.sin(Math.PI * 0.9) * (r - 2));
  g.arc(x, y, r - 2, Math.PI * 0.9, Math.PI * 1.6);
  g.stroke({ width: 2.5, color: ST[3], alpha: 0.7 });
  g.moveTo(x + Math.cos(-Math.PI * 0.1) * (r - 2), y + Math.sin(-Math.PI * 0.1) * (r - 2));
  g.arc(x, y, r - 2, -Math.PI * 0.1, Math.PI * 0.6);
  g.stroke({ width: 3, color: PLUM, alpha: 0.3 });
  if (ruined) breach(g, x, y, r, lost, Math.min(14, (r / FT) * 1.3), rnd, fallen);
  return lost;
}

/**
 * A round wall walk at (x, y) from `r1` in to `r0` (a shell keep's, inside its parapet): flags,
 * their joints across it, the shade of the parapet on the lit side.
 */
export function roundWalk(g: Graphics, x: number, y: number, r0: number, r1: number, rnd: () => number) {
  g.circle(x, y, (r0 + r1) / 2).stroke({ width: r1 - r0, color: WK });
  const rm = (r0 + r1) / 2;
  for (let t = rnd() * 0.2; t < Math.PI * 2 - FT / rm; t += (FT * (2.4 + rnd() * 1.4)) / rm) {
    const t1 = t + ((rnd() - 0.5) * FT * 0.4) / rm;
    g.moveTo(x + Math.cos(t) * r0, y + Math.sin(t) * r0).lineTo(x + Math.cos(t1) * r1, y + Math.sin(t1) * r1);
  }
  g.stroke({ width: 1.3, color: ST[0], alpha: 0.6 });
  g.moveTo(x + Math.cos(Math.PI * 0.85) * (r1 - FT * 0.3), y + Math.sin(Math.PI * 0.85) * (r1 - FT * 0.3));
  g.arc(x, y, r1 - FT * 0.3, Math.PI * 0.85, Math.PI * 1.65);
  g.stroke({ width: FT * 0.6, color: PLUM, alpha: 0.25 });
}

/** A wall segment as drawn: its ends, length and frame (`at`: `a` along, `d` across toward its left, +n), its
 * half thickness, outline (a bare end broken), its own random stream and flags. */
function wallSeg(s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const l = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
  const flags = flagsOf(s, 4);
  const rnd = rngAt(x0 + x1, y0 + y1);
  const half = s.size / 2;
  const at = (a: number, d: number): [number, number] => [x0 + ux * a - uy * d, y0 + uy * a + ux * d];
  // A bare end's broken line: a few points across it, pushed in or out a little.
  const broken = (a: number, dir: number) => [-1, -0.6, -0.2, 0.2, 0.6, 1].map((t) => at(a + dir * rnd() * FT * 2.6, t * half));
  const endA = flags & BARE_A ? broken(0, 1) : [at(0, -half), at(0, half)];
  const endB = flags & BARE_B ? broken(l, -1) : [at(l, -half), at(l, half)];
  const outline = [...endA, ...endB.reverse()].flat();
  return { x0, y0, x1, y1, l, ux, uy, side: Math.sign(Math.round((s.pts[5] ?? 0) / PX)), half, at, outline, rnd, bareA: !!(flags & BARE_A), bareB: !!(flags & BARE_B), ruined: !!(flags & RUINED) };
}
type Seg = ReturnType<typeof wallSeg>;

/**
 * Curtain walls (`ShapeKind::Wall`, one segment each, `size` thick) by their look (the style on point 6):
 * stone in its stone's tones and its group's merlons (`stoneWalls`), a timber palisade (`palisades`), rammed
 * earth (`earthWalls`); each look's together, so its fills batch. `gates` (`ShapeKind::Gate`) tell a palisade
 * which way the town lies where its run doesn't turn.
 */
export function drawWalls(g: Graphics, walls: Shape[], gates: Shape[] = []) {
  const sets = new Map<number, Shape[]>();
  for (const s of walls) {
    const l = lookOf(s, 6);
    const key = l.material * 64 + l.stone * 8 + l.merlons;
    const set = sets.get(key);
    if (set) set.push(s);
    else sets.set(key, [s]);
  }
  for (const set of sets.values()) {
    const l = lookOf(set[0], 6);
    use(l);
    const segs = (l.material === MAT_STONE || l.material === MAT_EARTH ? set.flatMap(breaks) : set).map(wallSeg);
    if (l.material === MAT_PALISADE) palisades(g, segs, gates);
    else if (l.material === MAT_EARTH) earthWalls(g, segs);
    else stoneWalls(g, segs);
  }
  use(null);
}

/** A ruined curtain cut into stretches with gaps between (the walk open to the ground), each stretch's cut ends
 * bare (broken, rubble heaped); other walls whole. */
function breaks(s: Shape): Shape[] {
  if (!(flagsOf(s, 4) & RUINED)) return [s];
  const [x0, y0, x1, y1] = s.pts;
  const l = Math.hypot(x1 - x0, y1 - y0);
  if (l < FT * 16) return [s];
  const rnd = rngAt(x0 * 0.7 + x1, y0 * 0.7 + y1);
  rnd();
  const n = l > FT * 46 ? 2 : 1;
  const cuts: [number, number][] = [];
  for (let k = 0; k < n; k++) {
    const c = (l * (k + 0.3 + rnd() * 0.4)) / n;
    const w = FT * (7 + rnd() * 6);
    cuts.push([Math.max(FT * 4, c - w / 2), Math.min(l - FT * 4, c + w / 2)]);
  }
  const out: Shape[] = [];
  const flags = flagsOf(s, 4);
  let a = 0;
  const piece = (from: number, to: number, fl: number) => {
    if (to - from < FT * 3) return;
    const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
    const pts = s.pts.slice();
    [pts[0], pts[1], pts[2], pts[3], pts[4]] = [x0 + ux * from, y0 + uy * from, x0 + ux * to, y0 + uy * to, fl * PX];
    out.push({ ...s, pts });
  };
  cuts.forEach(([c0, c1], k) => {
    piece(a, c0, flags | BARE_B | (k ? BARE_A : 0));
    a = c1;
  });
  piece(a, l, flags | BARE_A);
  return out.length ? out : [s];
}

/** Which side of each segment faces out of the town (+1 its +n, -1 its -n): a run of segments end to end turns
 * toward the town it rings, so its outside is the side it turns from; a straight run's away from the way the
 * nearest gate leads in (`gates`), or +n with none. */
function outsides(segs: Seg[], gates: Shape[] = []): number[] {
  const n = segs.length;
  const root = segs.map((_, i) => i);
  const find = (i: number): number => (root[i] === i ? i : (root[i] = find(root[i])));
  const links: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      if (i !== j && Math.abs(segs[i].x1 - segs[j].x0) + Math.abs(segs[i].y1 - segs[j].y0) < 2) {
        links.push([i, j]);
        root[find(i)] = find(j);
      }
    }
  }
  const turn = new Array(n).fill(0);
  for (const [i, j] of links) turn[find(i)] += segs[i].ux * segs[j].uy - segs[i].uy * segs[j].ux;
  return segs.map((s, i) => {
    // (The generator's own word where it has one: `[flags, side]`, side +1 its +n.)
    if (s.side) return s.side;
    const t = turn[find(i)];
    if (Math.abs(t) > 1e-3 || !gates.length) return t > 1e-3 ? -1 : 1;
    // (The nearest gate's way in: its side of the line between its towers.)
    const [mx, my] = [(s.x0 + s.x1) / 2, (s.y0 + s.y1) / 2];
    let [best, nx, ny] = [Infinity, 0, 0];
    for (const q of gates) {
      const [x0, y0, x1, y1] = q.pts;
      const d = Math.hypot((x0 + x1) / 2 - mx, (y0 + y1) / 2 - my);
      const l = Math.hypot(x1 - x0, y1 - y0) || 1;
      const sd = flagsOf(q, 6) < 0 ? -1 : 1;
      if (d < best) [best, nx, ny] = [d, (-(y1 - y0) / l) * sd, ((x1 - x0) / l) * sd];
    }
    return -s.uy * nx + s.ux * ny > 0 ? -1 : 1;
  });
}

/** Where the light falls on a segment's side `side` (+1 its +n). */
const sideLit = (s: Seg, side: number) => lit(-s.uy * side, s.ux * side);

/**
 * Stone curtains: a soft shadow, ink and the stone body (round joints where no end is bare), a flagged walk
 * down the middle, a parapet with merlons along each side, the shade of the parapet facing the light across
 * the walk; moss and cracks; rubble where an end stands bare; a ruin's with merlons gone and stones fallen.
 */
function stoneWalls(g: Graphics, segs: Seg[]) {
  const joints = (fn: (x: number, y: number) => void) => {
    for (const s of segs) {
      if (!s.bareA) fn(s.x0, s.y0);
      if (!s.bareB) fn(s.x1, s.y1);
    }
  };
  // Shadows, in two soft slices.
  for (const [off, alpha] of [
    [PX * 0.85, 0.12],
    [PX * 0.5, 0.18],
  ]) {
    for (const s of segs) g.poly(s.outline.map((v) => v + off));
    joints((x, y) => g.circle(x + off, y + off, segs[0].half));
    g.fill({ color: 0x000000, alpha });
  }
  // Ink, then the stone body.
  for (const s of segs) g.poly(s.outline);
  g.stroke({ width: 5, color: INK, join: 'round' });
  joints((x, y) => g.circle(x, y, segs[0].half + 2.5));
  g.fill(INK);
  for (const s of segs) g.poly(s.outline);
  joints((x, y) => g.circle(x, y, segs[0].half));
  g.fill(ST[1]);
  // The walk between the parapets, its flags' joints and the shade of the lit side's parapet.
  for (const s of segs) {
    const w = s.half - PARAPET;
    const [a0, a1] = [s.bareA ? FT * 2.8 : 0, s.bareB ? s.l - FT * 2.8 : s.l];
    if (a1 > a0) g.poly([...s.at(a0, -w), ...s.at(a1, -w), ...s.at(a1, w), ...s.at(a0, w)]);
  }
  joints((x, y) => g.circle(x, y, segs[0].half - PARAPET));
  g.fill(WK);
  for (const s of segs) {
    const w = s.half - PARAPET;
    for (let a = s.rnd() * FT * 2; a < s.l; a += FT * (2.4 + s.rnd() * 1.4)) {
      g.moveTo(...s.at(a, -w)).lineTo(...s.at(a + (s.rnd() - 0.5) * FT * 0.4, w));
      if (s.rnd() < 0.3) g.moveTo(...s.at(a, 0)).lineTo(...s.at(a + FT * (1.6 + s.rnd()), (s.rnd() - 0.5) * FT * 0.3));
    }
  }
  g.stroke({ width: 1.3, color: ST[0], alpha: 0.6 });
  if (brick()) {
    // Rose brick: a few bold light courses along the walk, their joints staggered.
    for (const s of segs) {
      const w = s.half - PARAPET;
      const [a0, a1] = [s.bareA ? FT * 2.8 : 0, s.bareB ? s.l - FT * 2.8 : s.l];
      for (const d of [-w * 0.5, w * 0.5]) g.moveTo(...s.at(a0, d)).lineTo(...s.at(a1, d));
      for (let a = a0 + FT * 1.5, j = 0; a < a1; a += FT * 4.5, j++) g.moveTo(...s.at(a, j % 2 ? 0 : -w)).lineTo(...s.at(a, j % 2 ? w : 0));
    }
    g.stroke({ width: LINE * 0.8, color: ST[3], alpha: 0.85 });
  }
  for (const s of segs) {
    const w = s.half - PARAPET;
    // The parapet on the side the light comes from shades the walk beside it.
    const side = lit(-s.uy, s.ux) > 0 ? 1 : -1;
    g.poly([...s.at(0, side * w), ...s.at(s.l, side * w), ...s.at(s.l, side * (w - FT * 0.6)), ...s.at(0, side * (w - FT * 0.6))]);
  }
  g.fill({ color: PLUM, alpha: 0.25 });
  // Each parapet one band of coping (lit on its edge toward the light), the crenels notches
  // through it (a ruin's wider, merlons gone), inked along the walk.
  const crenels: number[][] = [];
  const fallen: number[][] = [];
  for (const s of segs) {
    const [a0, a1] = [s.bareA ? FT * 3 : 0, s.bareB ? s.l - FT * 3 : s.l];
    if (a1 <= a0) continue;
    for (const side of [-1, 1]) {
      const [d0, d1] = [side * (s.half - PARAPET), side * s.half];
      g.poly([...s.at(a0, d0), ...s.at(a1, d0), ...s.at(a1, d1), ...s.at(a0, d1)]).fill(ST[2]);
      const toLight = lit(-s.uy * side, s.ux * side) > 0;
      const e = toLight ? d1 - side * FT * 0.3 : d0 + side * FT * 0.3;
      g.moveTo(...s.at(a0, e)).lineTo(...s.at(a1, e)).stroke({ width: FT * 0.3, color: ST[3], alpha: 0.8 });
      crenelRun(s.at, a0, a1, d0, d1, s.rnd, crenels, s.ruined, fallen);
      g.moveTo(...s.at(a0, d0)).lineTo(...s.at(a1, d0));
    }
  }
  g.stroke({ width: brick() ? 2.6 : 1.8, color: INK, join: 'miter' });
  crenelFill(g, crenels);
  // Moss on the stone and the odd crack; a ruin's more of both.
  for (const s of segs) {
    const n = Math.floor((s.l / (FT * 24)) * (s.ruined ? 1.5 : 1) + s.rnd());
    for (let k = 0; k < n; k++) {
      const [mx, my] = s.at(s.rnd() * s.l, (s.rnd() < 0.5 ? -1 : 1) * (s.half - PARAPET * 0.5));
      const r = FT * (0.3 + s.rnd() * 0.25);
      g.ellipse(mx, my, r * 1.3, r).fill({ color: MOSS[0], alpha: 0.85 });
      g.ellipse(mx - r * 0.3, my - r * 0.25, r * 0.6, r * 0.45).fill({ color: MOSS[1], alpha: 0.85 });
    }
    for (let k = 0; k < (s.ruined ? 2 : 1); k++) {
      if (s.rnd() < 0.5) continue;
      const a = s.rnd() * s.l;
      const d = (s.rnd() - 0.5) * s.half;
      g.moveTo(...s.at(a, d)).lineTo(...s.at(a + FT * 0.8, d + FT * 0.5)).lineTo(...s.at(a + FT * 1.5, d + FT * 0.2));
    }
  }
  g.stroke({ width: 1.4, color: ST[0], alpha: 0.9, join: 'round' });
  // Rubble spilled where a stretch broke off, and below a ruin's walls.
  for (const s of segs) {
    if (s.bareA) fallen.push(...rubble(...s.at(-FT * 2, 0), 3, s.half * 0.7, s.rnd, 2));
    if (s.bareB) fallen.push(...rubble(...s.at(s.l + FT * 2, 0), 3, s.half * 0.7, s.rnd, 2));
    if (s.ruined) for (let a = s.rnd() * FT * 20; a < s.l; a += FT * (22 + s.rnd() * 16)) fallen.push(...rubble(...s.at(a, (s.rnd() < 0.5 ? -1 : 1) * (s.half + FT * 1.5)), 1, FT * 1.2, s.rnd, 1.8));
  }
  blocks(g, fallen, stoneTone(segs[0]?.rnd ?? Math.random));
}

/** Stake tops' radius on a palisade (px). */
const STAKE = FT * 0.85;

/**
 * A timber palisade (3 ft): a row of stakes along its outer face seen from above, each sharpened to a point
 * (its lit facet toward the light), inked; behind them, on the town side, a walk of planks laid across, its
 * inner edge inked; the stakes' shadow cast long. A ruin's has stakes gone and some lying fallen.
 */
function palisades(g: Graphics, segs: Seg[], gates: Shape[]) {
  const out = outsides(segs, gates);
  // (Each stake: its middle, kept; a ruin's gaps left out.)
  const stakes: [number, number][] = [];
  const fallen: [Seg, number, number][] = [];
  segs.forEach((s, i) => {
    const d = out[i] * (s.half - STAKE);
    const [a0, a1] = [s.bareA ? FT * 2 : 0, s.bareB ? s.l - FT * 2 : s.l];
    for (let a = a0 + STAKE * 0.6; a < a1; a += STAKE * 2.05) {
      if (s.ruined && s.rnd() < 0.3) {
        if (s.rnd() < 0.3) fallen.push([s, a, -out[i]]);
        continue;
      }
      stakes.push(s.at(a + (s.rnd() - 0.5) * FT * 0.12, d + (s.rnd() - 0.5) * FT * 0.14));
    }
  });
  // Shadows: the stakes' long, the walk's short.
  for (const [x, y] of stakes) g.circle(x + PX * 0.7, y + PX * 0.7, STAKE);
  g.fill({ color: 0x000000, alpha: 0.2 });
  // The walk: planks across it, the shade of the stakes on it where the light comes over them.
  const walk = (s: Seg, i: number) => {
    const [b0, b1] = [-out[i] * s.half, out[i] * (s.half - STAKE * 1.6)];
    return { b0, b1, q: [...s.at(0, b0), ...s.at(s.l, b0), ...s.at(s.l, b1), ...s.at(0, b1)] };
  };
  segs.forEach((s, i) => g.poly(walk(s, i).q.map((v) => v + PX * 0.25)));
  g.fill({ color: 0x000000, alpha: 0.15 });
  segs.forEach((s, i) => g.poly(walk(s, i).q));
  g.fill(WK);
  segs.forEach((s, i) => {
    const { b0, b1 } = walk(s, i);
    for (let a = FT * (0.4 + s.rnd() * 0.6); a < s.l; a += FT * (1.0 + s.rnd() * 0.25)) g.moveTo(...s.at(a, b0)).lineTo(...s.at(a, b1));
  });
  g.stroke({ width: 1.1, color: ST[0], alpha: 0.6 });
  segs.forEach((s, i) => {
    if (sideLit(s, out[i]) > 0) {
      const b1 = out[i] * (s.half - STAKE * 1.6);
      g.poly([...s.at(0, b1), ...s.at(s.l, b1), ...s.at(s.l, b1 - out[i] * FT * 0.5), ...s.at(0, b1 - out[i] * FT * 0.5)]);
    }
  });
  g.fill({ color: PLUM, alpha: 0.22 });
  segs.forEach((s, i) => {
    const b0 = -out[i] * s.half;
    g.moveTo(...s.at(0, b0)).lineTo(...s.at(s.l, b0));
  });
  g.stroke({ width: OUT, color: INK, cap: 'round' });
  // The stakes: each a cone from above.
  stakeTops(g, stakes, STAKE);
  // A ruin's fallen stakes, lying out from the line.
  const lying: number[][] = [];
  for (const [s, a, side] of fallen) {
    const t = (s.rnd() - 0.5) * 1.2;
    const [cx, cy] = s.at(a, side * (s.half + FT * 2.5));
    const [vx, vy] = [Math.cos(Math.atan2(s.uy, s.ux) + Math.PI / 2 + t), Math.sin(Math.atan2(s.uy, s.ux) + Math.PI / 2 + t)];
    lying.push(logPoly((p, q) => [cx + vx * p - vy * q, cy + vy * p + vx * q], -FT * 3, FT * 3, -STAKE, STAKE));
  }
  logs(g, lying, []);
}

/** A rammed earth wall's battered foot each side (ft). */
const BATTER = 1.6;

/**
 * A rammed earth wall: one smooth tan body, its battered faces each side (the one toward the light lit, the
 * other in shade), a parapet with rounded merlons along each, a packed walk between; few big shapes. A ruin's
 * crumbles: merlons gone, lumps of earth fallen.
 */
function earthWalls(g: Graphics, segs: Seg[]) {
  const joints = (fn: (x: number, y: number) => void) => {
    for (const s of segs) {
      if (!s.bareA) fn(s.x0, s.y0);
      if (!s.bareB) fn(s.x1, s.y1);
    }
  };
  const half = segs[0].half;
  for (const s of segs) g.poly(s.outline.map((v) => v + PX * 0.6));
  joints((x, y) => g.circle(x + PX * 0.6, y + PX * 0.6, half));
  g.fill({ color: 0x000000, alpha: 0.2 });
  for (const s of segs) g.poly(s.outline);
  g.stroke({ width: OUT * 2, color: INK, join: 'round' });
  joints((x, y) => g.circle(x, y, half + OUT));
  g.fill(INK);
  for (const s of segs) g.poly(s.outline);
  joints((x, y) => g.circle(x, y, half));
  g.fill(ST[1]);
  const B = FT * BATTER;
  const P = PARAPET;
  const band = (s: Seg, d0: number, d1: number) => [...s.at(0, d0), ...s.at(s.l, d0), ...s.at(s.l, d1), ...s.at(0, d1)];
  // The battered faces: lit toward the light, shaded away.
  for (const s of segs) for (const side of [-1, 1]) if (sideLit(s, side) > 0) g.poly(band(s, side * (half - B), side * half));
  g.fill(ST[2]);
  for (const s of segs) for (const side of [-1, 1]) if (sideLit(s, side) <= 0) g.poly(band(s, side * (half - B), side * half));
  g.fill(mix(ST[1], PLUM, 0.22));
  // The walk, packed: one tone, the shade of the lit parapet across it.
  const w = half - B - P;
  for (const s of segs) g.poly(band(s, -w, w));
  joints((x, y) => g.circle(x, y, w));
  g.fill(WK);
  for (const s of segs) {
    const side = sideLit(s, 1) > 0 ? 1 : -1;
    g.poly(band(s, side * w, side * (w - FT * 0.7)));
  }
  g.fill({ color: PLUM, alpha: 0.2 });
  // The parapets: coping, rounded merlons, inked along the walk and along the top of the batter.
  const crenels: number[][] = [];
  const fallen: number[][] = [];
  for (const s of segs) {
    const [a0, a1] = [s.bareA ? FT * 3 : 0, s.bareB ? s.l - FT * 3 : s.l];
    if (a1 <= a0) continue;
    for (const side of [-1, 1]) {
      const [d0, d1] = [side * w, side * (w + P)];
      g.poly([...s.at(a0, d0), ...s.at(a1, d0), ...s.at(a1, d1), ...s.at(a0, d1)]).fill(ST[2]);
      crenelRun(s.at, a0, a1, d0, d1, s.rnd, crenels, s.ruined, fallen);
      g.moveTo(...s.at(a0, d0)).lineTo(...s.at(a1, d0));
    }
  }
  g.stroke({ width: 1.8, color: INK });
  for (const s of segs) for (const side of [-1, 1]) g.moveTo(...s.at(0, side * (w + P))).lineTo(...s.at(s.l, side * (w + P)));
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.7 });
  crenelFill(g, crenels);
  for (const s of segs) {
    if (s.bareA) fallen.push(...rubble(...s.at(-FT * 2, 0), 3, half * 0.7, s.rnd, 2));
    if (s.bareB) fallen.push(...rubble(...s.at(s.l + FT * 2, 0), 3, half * 0.7, s.rnd, 2));
    if (s.ruined) for (let a = s.rnd() * FT * 20; a < s.l; a += FT * (24 + s.rnd() * 16)) fallen.push(...rubble(...s.at(a, (s.rnd() < 0.5 ? -1 : 1) * (half + FT * 1.5)), 1, FT * 1.4, s.rnd, 1.8));
  }
  blocks(g, fallen, stoneTone(segs[0].rnd));
}

/** Packed earth on a bastion's top: dark, mid, light (three close matte tones). */
const EARTH_TOP = [0x8d7658, 0xa68e6a, 0xbca57f];
/** The ramp up onto a bastion from the town (`town::wallside::RAMP_FT`, `RAMP_HALF`): ft out from the corner, half
 * its width. */
const RAMP_FT = 10;
const RAMP_HALF = 5;

/** A bastion's ramp (px): its foot along the open back (on either side of the corner `v`), its head out along the
 * middle line: foot on one side, the corner, foot on the other side, head, head. */
export function bastionRamp(pts: number[]): number[] {
  const [vx, vy] = [pts[10], pts[11]];
  let [ox, oy] = [pts[4] - vx, pts[5] - vy];
  const lo = Math.hypot(ox, oy) || 1;
  [ox, oy] = [ox / lo, oy / lo];
  const [sx, sy] = [-oy, ox];
  const [hw, len] = [RAMP_HALF * FT, RAMP_FT * FT];
  // (Where the ramp's side, `side` of the middle line, meets the back edge on that side: v to f1 or v to f2.)
  const foot = (side: number): [number, number] => {
    for (const f of [0, 8]) {
      const [dx, dy] = [pts[f] - vx, pts[f + 1] - vy];
      const lat = dx * sx + dy * sy;
      if (lat * side <= 0) continue;
      const t = Math.min(1, hw / Math.abs(lat));
      return [vx + dx * t, vy + dy * t];
    }
    return [vx + sx * hw * side, vy + sy * hw * side];
  };
  const [a, b] = [foot(-1), foot(1)];
  return [...a, vx, vy, ...b, vx + ox * len + sx * hw, vy + oy * len + sy * hw, vx + ox * len - sx * hw, vy + oy * len - sy * hw];
}

/**
 * A star bastion's top (`Layout::bastions`: f1, g1, tip, g2, f2, the corner, px, then its style): an earth terreplein
 * (packed dirt, a worn track across it, a few stones), held up along its open back by a low stone revetment that
 * casts its shadow on the town, with a ramp cut up into it from the town at the corner. Its curtain (drawn over its
 * edge by `drawWalls`) keeps its walk and merlons; its props (a ballista, shot, crates) are battlemap objects.
 */
export function drawBastion(g: Graphics, s: Shape) {
  const pts = s.pts.slice(0, 12);
  use(lookOf(s, 12));
  const rnd = rngAt(pts[4], pts[5]);
  const [vx, vy] = [pts[10], pts[11]];
  let [ox, oy] = [pts[4] - vx, pts[5] - vy];
  const depth = Math.hypot(ox, oy) || 1;
  [ox, oy] = [ox / depth, oy / depth];
  const [sx, sy] = [-oy, ox];
  const at = (a: number, l: number): [number, number] => [vx + ox * a + sx * l, vy + oy * a + sy * l];
  // The earth, and a few broad patches of it lighter and darker (no lines across it).
  g.poly(pts).fill(EARTH_TOP[1]);
  const patch = (cx: number, cy: number, r: number, tone: number, alpha: number) => {
    const n = 9;
    const q: number[] = [];
    for (let k = 0; k < n; k++) {
      const t = (k / n) * Math.PI * 2;
      const rr = r * (0.75 + rnd() * 0.4);
      q.push(cx + Math.cos(t) * rr * 1.3, cy + Math.sin(t) * rr);
    }
    g.poly(q).fill({ color: tone, alpha });
  };
  for (let k = 0; k < 3; k++) {
    const [px, py] = at(depth * (0.35 + rnd() * 0.4), (rnd() - 0.5) * depth * 0.5);
    patch(px, py, FT * (3 + rnd() * 2.5), k === 0 ? EARTH_TOP[0] : EARTH_TOP[2], k === 0 ? 0.35 : 0.45);
  }
  // A worn track from the ramp's head toward the point, lighter.
  const [t0x, t0y] = at(RAMP_FT * FT, 0);
  const [t1x, t1y] = at(depth * 0.72, (rnd() - 0.5) * FT * 2);
  g.moveTo(t0x, t0y).quadraticCurveTo((t0x + t1x) / 2 + sx * FT, (t0y + t1y) / 2 + sy * FT, t1x, t1y).stroke({ width: FT * 3.2, color: EARTH_TOP[2], alpha: 0.55, cap: 'round' });
  // Shadow of the curtain's parapet on the earth, along the faces away from the light.
  for (let i = 0; i < 4; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[i * 2 + 2], pts[i * 2 + 3]];
    const [mx, my] = [(ax + bx) / 2 - vx, (ay + by) / 2 - vy];
    if (lit(mx, my) < 0.1) continue;
    const l = Math.hypot(bx - ax, by - ay) || 1;
    let [nx, ny] = [-(by - ay) / l, (bx - ax) / l];
    if (nx * mx + ny * my > 0) [nx, ny] = [-nx, -ny];
    const w = s.size / 2 + FT * 1.2;
    g.poly([ax, ay, bx, by, bx + nx * w, by + ny * w, ax + nx * w, ay + ny * w]).fill({ color: EARTH_TOP[0], alpha: 0.3 });
  }
  // A few stones in the earth.
  for (let k = 0; k < 4; k++) {
    const [px, py] = at(depth * (0.3 + rnd() * 0.5), (rnd() - 0.5) * depth * 0.7);
    if (!inside(pts, px, py)) continue;
    const r = FT * (0.35 + rnd() * 0.25);
    g.ellipse(px, py, r * 1.2, r).fill(ST[2]).stroke({ width: LINE * 0.8, color: INK });
  }
  // The ramp: the cut's floor, its treads square to the way up, lit on their upper edge; the cut's side toward the
  // light shades the floor beside it.
  const ramp = bastionRamp(pts);
  g.poly(ramp).fill(mix(EARTH_TOP[1], EARTH_TOP[0], 0.45));
  const hw = RAMP_HALF * FT;
  const [fa, fb] = [ramp.slice(0, 2), ramp.slice(4, 6)];
  // (Which foot stands on the back edge from the corner to f1, which on the one to f2.)
  const onF1 = Math.abs((fa[0] - vx) * (pts[1] - vy) - (fa[1] - vy) * (pts[0] - vx)) < Math.abs((fb[0] - vx) * (pts[1] - vy) - (fb[1] - vy) * (pts[0] - vx));
  const [foot1, foot2] = onF1 ? [fa, fb] : [fb, fa];
  const footAx = (p: number[]) => (p[0] - vx) * ox + (p[1] - vy) * oy;
  const start = Math.min(footAx(fa), footAx(fb));
  for (let a = start + FT * 2.4; a < RAMP_FT * FT - FT * 0.5; a += FT * 2.4) {
    // (Clipped to the ramp: its sides, and the back edges near its foot.)
    const lat = (side: number) => {
      const f = side < 0 ? fa : fb;
      const fAx = footAx(f);
      if (a >= fAx) return side * hw;
      const r = Math.max(0, a) / Math.max(1e-6, fAx);
      return side * hw * r;
    };
    const [l0, l1] = [lat(-1), lat(1)];
    if (l1 - l0 < FT) continue;
    g.moveTo(...at(a, l0 + FT * 0.3)).lineTo(...at(a, l1 - FT * 0.3));
  }
  g.stroke({ width: LINE, color: INK, alpha: 0.75, cap: 'round' });
  for (let a = start + FT * 2.4; a < RAMP_FT * FT - FT * 0.5; a += FT * 2.4) {
    g.moveTo(...at(a + FT * 0.35, -hw * 0.55)).lineTo(...at(a + FT * 0.35, hw * 0.55));
  }
  g.stroke({ width: FT * 0.4, color: EARTH_TOP[2], alpha: 0.7, cap: 'round' });
  // The revetment: a stone coping along the open back and down the ramp's sides (the earth stands up behind it),
  // its shadow on the town where it faces away from the light, inked on its outer edge.
  const cope = FT * 1.5;
  const runs: [number, number, number, number][] = [
    [pts[8], pts[9], foot2[0], foot2[1]],
    [foot1[0], foot1[1], pts[0], pts[1]],
    [fb[0], fb[1], ramp[6], ramp[7]],
    [fa[0], fa[1], ramp[8], ramp[9]],
  ];
  const [cx, cy] = at(depth * 0.55, 0);
  runs.forEach(([ax, ay, bx, by], idx) => {
    const l = Math.hypot(bx - ax, by - ay);
    if (l < FT * 0.5) return;
    // (Into the earth: toward the top's middle for the back; away from the ramp's middle line for its sides.)
    let [nx, ny] = [-(by - ay) / l, (bx - ax) / l];
    const away = idx >= 2 ? Math.sign(((ax + bx) / 2 - vx) * sx + ((ay + by) / 2 - vy) * sy) : 0;
    const [rx, ry] = idx >= 2 ? [sx * away, sy * away] : [cx - ax, cy - ay];
    if (nx * rx + ny * ry < 0) [nx, ny] = [-nx, -ny];
    // (Its shadow out over the lower ground, where the face looks away from the light.)
    if (-(nx * LX + ny * LY) < 0.2) g.poly([ax, ay, bx, by, bx - nx * FT * 1.6, by - ny * FT * 1.6, ax - nx * FT * 1.6, ay - ny * FT * 1.6]).fill({ color: PLUM, alpha: 0.22 });
    const band = [ax, ay, bx, by, bx + nx * cope, by + ny * cope, ax + nx * cope, ay + ny * cope];
    g.poly(band).fill(nx * LX + ny * LY < 0 ? ST[2] : ST[1]);
    g.moveTo(ax + nx * cope, ay + ny * cope).lineTo(bx + nx * cope, by + ny * cope).stroke({ width: LINE * 0.75, color: ST[0], alpha: 0.8 });
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: LINE, color: INK, cap: 'round' });
  });
  use(null);
}

/** Whether (x, y) falls inside a polygon of flat px pairs. */
function inside(q: number[], x: number, y: number): boolean {
  let hit = false;
  for (let i = 0, j = q.length / 2 - 1; i < q.length / 2; j = i++) {
    const [xi, yi, xj, yj] = [q[i * 2], q[i * 2 + 1], q[j * 2], q[j * 2 + 1]];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) hit = !hit;
  }
  return hit;
}

/**
 * A wall or gate tower's top (`ShapeKind::Tower`: its centre, `size` its radius, then flags): a
 * long soft shadow, a ring of masonry a little out of round with merlons on it, lit on the
 * north-west; a flagged fighting top with the spiral stair's hatch; a gate tower's banner. A
 * ruin's has merlons gone, a stretch of its parapet broken and rubble on the top.
 */
export function drawWallTower(g: Graphics, s: Shape) {
  const look = lookOf(s, 4);
  use(look);
  towerBody(g, s, look);
  use(null);
}

/** A tower by its look: a square or arched gate tower is half its gatehouse (`drawGate` draws it whole); a
 * palisade's a log blockhouse, the rest by their shape; a round one squat in the North (a broad splayed foot,
 * a short shadow), a tapering bastion in rammed earth (its battered face round it). */
function towerBody(g: Graphics, s: Shape, look: Look) {
  const [x, y, r] = [s.pts[0], s.pts[1], s.size];
  const flags = flagsOf(s, 2);
  const shape = look.shape;
  if (flags & GATE && (gateForm(s) === GATE_SQUARE || gateForm(s) === GATE_ARCHED)) return;
  if (look.material === MAT_PALISADE && (flags & SQUARE || shape === SHAPE_SQUARE)) return drawLogTower(g, s);
  if (flags & SQUARE || shape === SHAPE_SQUARE) return drawSquareTower(g, s);
  if (shape === SHAPE_D || shape === SHAPE_OCTAGON) return drawShapedTower(g, s, shape);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  rnd();
  rnd();
  const earth = look.material === MAT_EARTH;
  const squat = !earth && look.group === 0;
  // (The foot round it: a squat tower's splayed plinth, an earth bastion's battered face, wider.)
  const foot = earth ? FT * 2.2 : squat ? FT * 1.1 : 0;
  // Shadow: the round swept away from the light (one outline, so it doesn't darken twice); a low one's short.
  const D = PX * (squat ? 0.7 : earth ? 0.8 : 1.1);
  const rs = r + foot;
  const shadow: number[] = [];
  for (let k = 0; k <= 12; k++) {
    const t = Math.PI * 0.75 + (k / 12) * Math.PI;
    shadow.push(x + Math.cos(t) * rs, y + Math.sin(t) * rs);
  }
  for (let k = 0; k <= 12; k++) {
    const t = -Math.PI * 0.25 + (k / 12) * Math.PI;
    shadow.push(x + D + Math.cos(t) * rs, y + D + Math.sin(t) * rs);
  }
  g.poly(shadow).fill({ color: 0x000000, alpha: 0.28 });
  if (foot) {
    // The foot in its shaded tone, a crescent lit toward the light, inked.
    g.circle(x, y, rs).fill(mix(ST[1], PLUM, 0.2)).stroke({ width: OUT, color: INK });
    g.moveTo(x + Math.cos(Math.PI * 0.8) * (r + foot / 2), y + Math.sin(Math.PI * 0.8) * (r + foot / 2));
    g.arc(x, y, r + foot / 2, Math.PI * 0.8, Math.PI * 1.7);
    g.stroke({ width: foot * 0.8, color: ST[2], alpha: 0.9, cap: 'round' });
  }
  const fallen: number[][] = [];
  const { ex, ey, rf, lost } = roundTop(g, x, y, r, rnd, ruined, fallen, earth);
  if (ruined) {
    fallen.push(...rubble(x + Math.cos(lost) * rf * 0.5, y + Math.sin(lost) * rf * 0.5, 2, rf * 0.3, rnd, 1.6));
    for (let k = 0; k < 4; k++) {
      const [mx, my] = [x + (rnd() - 0.5) * rf * 1.4, y + (rnd() - 0.5) * rf * 1.4];
      g.ellipse(mx, my, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  }
  blocks(g, fallen, stoneTone(rnd));
  // A gate tower flies the banner.
  if (gate && !ruined) banner(g, x - ex * rf * 0.4, y - ey * rf * 0.4);
}

/** A D's or an octagon's outline (px), as `wallstyle::footprint` lays it: `r` its radius, (`ux`, `uy`)
 * the way a D faces out or an octagon's sides run. */
function shapedOutline(x: number, y: number, r: number, ux: number, uy: number, shape: number): number[] {
  const at = (a: number, b: number) => [x + ux * a - uy * b, y + uy * a + ux * b];
  const out: number[] = [];
  if (shape === SHAPE_D) {
    out.push(...at(-D_BACK * FT, -r));
    for (let k = 0; k <= 8; k++) {
      const t = -Math.PI / 2 + (Math.PI * k) / 8;
      out.push(...at(r * Math.cos(t), r * Math.sin(t)));
    }
    out.push(...at(-D_BACK * FT, r));
  } else {
    const rc = r / Math.cos(Math.PI / 8);
    for (let k = 0; k < 8; k++) {
      const t = Math.PI / 8 + (Math.PI * 2 * k) / 8;
      out.push(...at(rc * Math.cos(t), rc * Math.sin(t)));
    }
  }
  return out;
}

/** A convex outline drawn in by `d` (px), corner for corner. */
function inset(pts: number[], d: number): number[] {
  const n = pts.length / 2;
  let area = 0;
  for (let i = 0; i < n; i++) area += pts[i * 2] * pts[((i + 1) % n) * 2 + 1] - pts[((i + 1) % n) * 2] * pts[i * 2 + 1];
  const sg = area > 0 ? 1 : -1;
  const normal = (i: number, j: number) => {
    const [dx, dy] = [pts[j * 2] - pts[i * 2], pts[j * 2 + 1] - pts[i * 2 + 1]];
    const l = Math.hypot(dx, dy) || 1;
    return [(-dy / l) * sg, (dx / l) * sg];
  };
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    const [ax, ay] = normal((i + n - 1) % n, i);
    const [bx, by] = normal(i, (i + 1) % n);
    const k = d / Math.max(0.3, 1 + ax * bx + ay * by);
    out.push(pts[i * 2] + (ax + bx) * k, pts[i * 2 + 1] + (ay + by) * k);
  }
  return out;
}

/** A parapet between a convex outline and the same drawn in (`inner`, corner for corner): one band of coping,
 * crenels notched through it side by side in the merlons' rhythm (a corner always a merlon; a ruin's wider,
 * stones to `fallen`), each side's lit edge toward the light and a shaded crescent away from it, inked. */
function ringParapet(g: Graphics, outer: number[], inner: number[], rnd: () => number, ruined = false, fallen: number[][] = []) {
  const n = outer.length / 2;
  g.poly(outer);
  g.poly(inner).cut();
  g.fill(ST[2]);
  const crenels: number[][] = [];
  const P = (i: number, k: number): [number, number] => [outer[i * 2] + (inner[i * 2] - outer[i * 2]) * k, outer[i * 2 + 1] + (inner[i * 2 + 1] - outer[i * 2 + 1]) * k];
  let [cx, cy] = [0, 0];
  for (let i = 0; i < n; i++) [cx, cy] = [cx + outer[i * 2] / n, cy + outer[i * 2 + 1] / n];
  const lits: number[] = [];
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const [mx, my] = [(outer[i * 2] + outer[j * 2]) / 2, (outer[i * 2 + 1] + outer[j * 2 + 1]) / 2];
    lits.push(lit(mx - cx, my - cy));
    const len = Math.hypot(outer[j * 2] - outer[i * 2], outer[j * 2 + 1] - outer[i * 2 + 1]) * 0.5 + Math.hypot(inner[j * 2] - inner[i * 2], inner[j * 2 + 1] - inner[i * 2 + 1]) * 0.5;
    // (`a` along the side at mid-parapet, `d` in from the outline: 0 at its face, PARAPET at the walk.)
    const at: At = (a, d) => {
      const [t, k] = [a / len, d / PARAPET];
      const [p, q] = [P(i, k), P(j, k)];
      return [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
    };
    if (len > FT * 3.4) crenelRun(at, FT * 0.3, len - FT * 0.3, PARAPET, 0, rnd, crenels, ruined, fallen);
  }
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    if (lits[i] > 0.1) g.moveTo(...P(i, 0.18)).lineTo(...P(j, 0.18));
  }
  g.stroke({ width: FT * 0.3, color: ST[3], alpha: 0.8, cap: 'round' });
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    if (lits[i] < -0.1) g.moveTo(...P(i, 0.15)).lineTo(...P(j, 0.15));
  }
  g.stroke({ width: 3, color: PLUM, alpha: 0.3, cap: 'round' });
  g.poly(inner).stroke({ width: 1.8, color: INK, join: 'round' });
  crenelFill(g, crenels);
  g.poly(outer).stroke({ width: OUT, color: INK, join: 'round' });
}

/** A flat top's floor inside its parapet (`inner`): flags, the shade of the lit sides' parapets, the stair's
 * hatch in from the side facing most away from the light (at `cx`, `cy` its middle). */
function shapedTop(g: Graphics, inner: number[], cx: number, cy: number, rnd: () => number) {
  flagFloor(g, inner, rnd);
  const n = inner.length / 2;
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const [mx, my] = [(inner[i * 2] + inner[j * 2]) / 2, (inner[i * 2 + 1] + inner[j * 2 + 1]) / 2];
    if (lit(mx - cx, my - cy) > 0.2) g.moveTo(inner[i * 2], inner[i * 2 + 1]).lineTo(inner[j * 2], inner[j * 2 + 1]);
  }
  g.stroke({ width: FT * 1.2, color: PLUM, alpha: 0.2 });
  // (The hatch: its long side across the way from the middle to the shaded side.)
  let reach = Infinity;
  for (let i = 0; i < n; i++) reach = Math.min(reach, Math.hypot(inner[i * 2] - cx, inner[i * 2 + 1] - cy));
  const [ex, ey] = [-LX, -LY];
  const [hl, hw] = [Math.min(FT * 4.5, reach * 0.45), FT * 1.6];
  if (hl < FT * 2.5) return;
  const hc = reach * 0.85 - hl / 2 - FT * 0.8;
  stairHatch(g, (a, b) => [cx + ex * (hc + a) - ey * b, cy + ey * (hc + a) + ex * b], hl, hw);
}

/** A D or an octagonal tower: its shadow, the stone outline, a flagged top with the stair's hatch inside its
 * parapet of merlons; a ruin's with merlons gone, rubble and moss on the top. */
function drawShapedTower(g: Graphics, s: Shape, shape: number) {
  const t = s.pts.length > 3 ? s.pts[3] / PX : 0;
  const [x, y, r, ux, uy] = [s.pts[0], s.pts[1], s.size, Math.cos(t), Math.sin(t)];
  const flags = flagsOf(s, 2);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  const outer = shapedOutline(x, y, r, ux, uy, shape);
  const D = PX * 1.1;
  g.poly(outer.map((v) => v + D)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(outer).fill(ST[1]);
  const inner = inset(outer, PARAPET);
  // (Its middle: a D's between its back and its face.)
  const [cx, cy] = shape === SHAPE_D ? [x + ux * ((r - D_BACK * FT) / 2), y + uy * ((r - D_BACK * FT) / 2)] : [x, y];
  shapedTop(g, inner, cx, cy, rnd);
  const fallen: number[][] = [];
  ringParapet(g, outer, inner, rnd, ruined, fallen);
  if (ruined) {
    breach(g, x, y, r * 0.95, Math.atan2(uy, ux) + (rnd() - 0.5) * 2.4, Math.min(12, (r / FT) * 1.2), rnd, fallen);
    fallen.push(...rubble(cx + (rnd() - 0.5) * r * 0.6, cy + (rnd() - 0.5) * r * 0.6, 2, r * 0.25, rnd, 1.5));
    for (let k = 0; k < 4; k++) g.ellipse(cx + (rnd() - 0.5) * r * 1.1, cy + (rnd() - 0.5) * r * 1.1, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
  }
  blocks(g, fallen, stoneTone(rnd));
  if (gate && !ruined) banner(g, cx + LX * r * 0.3, cy + LY * r * 0.3);
}

/** A rounded log (a capsule) from `a0` to `a1` along and `b0` to `b1` across in `at`'s frame. */
function logPoly(at: At, a0: number, a1: number, b0: number, b1: number): number[] {
  const [rb, mb] = [(b1 - b0) / 2, (b0 + b1) / 2];
  const out: number[] = [];
  for (let k = 0; k <= 6; k++) {
    const t = -Math.PI / 2 + (Math.PI * k) / 6;
    out.push(...at(a1 - rb + Math.cos(t) * rb * 0.7, mb + Math.sin(t) * rb));
  }
  for (let k = 0; k <= 6; k++) {
    const t = Math.PI / 2 + (Math.PI * k) / 6;
    out.push(...at(a0 + rb + Math.cos(t) * rb * 0.7, mb + Math.sin(t) * rb));
  }
  return out;
}

/** Logs (`logPoly` outlines) in timber: mid tone, the half toward the light lit, the ink round each. `toLight`
 * whether each log's +b side faces the light. */
function logs(g: Graphics, polys: number[][], lits: number[][]) {
  for (const p of polys) g.poly(p);
  if (polys.length) g.fill(ST[1]);
  for (const p of lits) g.poly(p);
  if (lits.length) g.fill(ST[2]);
  for (const p of polys) g.poly(p);
  if (polys.length) g.stroke({ width: LINE * 1.3, color: INK, join: 'round' });
}

/**
 * A palisade's tower, a log blockhouse seen from above (square, `SQUARE`'s frame): its shadow, a log laid
 * along each side, the two crossing the others at the corners with their ends standing out; inside them a
 * plank fighting top and the stair's hatch. A ruin's has a side's log fallen and planks gone.
 */
function drawLogTower(g: Graphics, s: Shape) {
  const { x, y, h, ux, uy } = squareFrame(s);
  const flags = flagsOf(s, 2);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const D = PX * 0.9;
  g.poly(quad(-h, h, -h, h).map((v) => v + D)).fill({ color: 0x000000, alpha: 0.28 });
  // The plank top inside the logs.
  const f = h - PARAPET;
  g.poly(quad(-h + FT * 0.4, h - FT * 0.4, -h + FT * 0.4, h - FT * 0.4)).fill(WK);
  for (let b = -f + FT * 1.1; b < f; b += FT * 1.1) g.moveTo(...at(-f, b)).lineTo(...at(f, b));
  for (let b = -f; b < f; b += FT * 1.1) {
    const a = (rnd() - 0.5) * f * 1.2;
    g.moveTo(...at(a, b)).lineTo(...at(a, Math.min(f, b + FT * 1.1)));
  }
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.6 });
  const lx = ux * LX + uy * LY;
  const ly = -uy * LX + ux * LY;
  g.poly(lx < 0 ? quad(-f, -f + FT * 0.8, -f, f) : quad(f - FT * 0.8, f, -f, f)).fill({ color: PLUM, alpha: 0.2 });
  g.poly(ly < 0 ? quad(-f, f, -f, -f + FT * 0.8) : quad(-f, f, f - FT * 0.8, f)).fill({ color: PLUM, alpha: 0.2 });
  const [hl, hw] = [Math.min(FT * 4.5, f * 0.6), FT * 1.6];
  const [ca, cb] = [f - hl / 2 - FT * 0.6, f - hw - FT * 0.6];
  if (!ruined) stairHatch(g, (a, b) => at(ca + a, cb - b), hl, hw);
  // The logs: along a (the sides at b = -h, h) under, along b over, each a log's thickness in from the face.
  const L = PARAPET;
  const e = FT * 0.9;
  const lost = ruined ? Math.floor(rnd() * 4) : -1;
  const under: number[][] = [];
  const over: number[][] = [];
  const litU: number[][] = [];
  const litO: number[][] = [];
  const fallen: number[][] = [];
  for (const [k, sg] of [
    [0, -1],
    [1, 1],
  ]) {
    const [b0, b1] = sg < 0 ? [-h, -h + L] : [h - L, h];
    const a1 = k * 2 === lost ? h * (0.1 + rnd() * 0.4) : h + e;
    const mb = (b0 + b1) / 2;
    under.push(logPoly(at, -h - e, a1, b0, b1));
    litU.push(ly < 0 ? logPoly(at, -h - e, a1, b0, mb) : logPoly(at, -h - e, a1, mb, b1));
    const atB: At = (a, b) => at(b, a);
    const a2 = k * 2 + 1 === lost ? h * (0.1 + rnd() * 0.4) : h + e;
    over.push(logPoly(atB, -h - e, a2, b0, b1));
    litO.push(lx < 0 ? logPoly(atB, -h - e, a2, b0, mb) : logPoly(atB, -h - e, a2, mb, b1));
    if (k * 2 === lost || k * 2 + 1 === lost) fallen.push(...rubble(...at(sg * (h + FT * 2), sg * h * 0.6), 2, FT * 1.4, rnd));
  }
  // (Each log's half toward the light lit: `ly` and `lx` how much +b and +a face it.)
  logs(g, under, litU);
  logs(g, over, litO);
  // End grain where the logs stand out at the corners: a ring on each end.
  for (const sa of [-1, 1]) {
    for (const sb of [-1, 1]) {
      g.circle(...at(sa * (h + e * 0.4), sb * (h - L / 2)), L * 0.2);
      g.circle(...at(sb * (h - L / 2), sa * (h + e * 0.4)), L * 0.2);
    }
  }
  g.stroke({ width: 1.1, color: ST[0], alpha: 0.7 });
  blocks(g, fallen, () => ST[1]);
  if (gate && !ruined) banner(g, ...at(-f * 0.4, -f * 0.4));
}

/** A square tower's frame: its middle, half side and the way its sides run (`SQUARE`). */
function squareFrame(s: Shape) {
  const t = s.pts.length > 3 ? s.pts[3] / PX : 0;
  return { x: s.pts[0], y: s.pts[1], h: s.size, ux: Math.cos(t), uy: Math.sin(t) };
}

/**
 * A square wall tower seen from above (a castle's: `SQUARE`): its shadow, the stone block in its
 * shaded and lit faces, a flagged fighting top inside a parapet of merlons, the stair's hatch with
 * its lid; a ruin's with merlons gone, cracks, moss and fallen stones.
 */
function drawSquareTower(g: Graphics, s: Shape) {
  const { x, y, h, ux, uy } = squareFrame(s);
  const ruined = !!(flagsOf(s, 2) & RUINED);
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  // Shadow, cast SE.
  const D = PX * 1.1;
  g.poly(quad(-h, h, -h, h).map((v) => v + D)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(quad(-h, h, -h, h)).fill(ST[1]).stroke({ width: 3, color: INK, join: 'round' });
  // The fighting top: flags in courses inside the parapet.
  const f = h - PARAPET;
  g.poly(quad(-f, f, -f, f)).fill(WK);
  const row = FT * 2.8;
  for (let b = -f + row * (0.3 + rnd() * 0.7); b < f; b += row) {
    g.moveTo(...at(-f, b)).lineTo(...at(f, b));
    for (let a = -f + FT * (0.5 + rnd() * 2); a < f; a += FT * (2.4 + rnd() * 1.4)) g.moveTo(...at(a, b)).lineTo(...at(a, Math.min(f, b + row)));
  }
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.4 });
  // The parapet's shade across the top on the side away from the light.
  const lx = ux * LX + uy * LY;
  const ly = -uy * LX + ux * LY;
  g.poly(lx < 0 ? quad(-f, -f + FT * 0.8, -f, f) : quad(f - FT * 0.8, f, -f, f)).fill({ color: PLUM, alpha: 0.18 });
  g.poly(ly < 0 ? quad(-f, f, -f, -f + FT * 0.8) : quad(-f, f, f - FT * 0.8, f)).fill({ color: PLUM, alpha: 0.18 });
  // The stair's hatch in a corner, its lid lying open beside it.
  const [hl, hw] = [Math.min(FT * 4.5, f * 0.6), FT * 1.6];
  const ca = f - hl / 2 - FT * 0.8;
  const cb = f - hw - FT * 0.8;
  g.poly(quad(ca - hl / 2, ca + hl / 2, cb - hw, cb + hw)).fill(0x221d19);
  for (let k = 0; k < 4; k++) {
    const a = ca + hl / 2 - (k + 1) * hl * 0.21;
    g.poly(quad(a, a + hl * 0.21, cb - hw + 1.5, cb + hw - 1.5)).fill(mix(ST[1], 0x221d19, k * 0.24)).stroke({ width: 1, color: 0x221d19, alpha: 0.6 });
  }
  g.poly(quad(ca - hl / 2, ca + hl / 2, cb - hw, cb + hw)).stroke({ width: 2.4, color: INK, join: 'round' });
  const lid = quad(ca - hl / 2, ca + hl / 2, cb - hw - FT * 0.3 - hw * 2, cb - hw - FT * 0.3);
  g.poly(lid.map((v) => v + FT * 0.25)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(lid).fill(WOOD[1]).stroke({ width: 1.8, color: INK, join: 'round' });
  // The parapet: coping round the top, crenels notched through it in the merlons' rhythm (a ruin's broken in places).
  const fallen: number[][] = [];
  ringParapet(g, quad(-h, h, -h, h), quad(-f, f, -f, f), rnd, ruined, fallen);
  if (ruined) {
    breach(g, x, y, h, Math.atan2(uy, ux) + Math.floor(rnd() * 4) * (Math.PI / 2), Math.min(12, (h / FT) * 1.2), rnd, fallen);
    fallen.push(...rubble(x, y, 2, f * 0.4, rnd, 1.5));
    for (let k = 0; k < 4; k++) {
      const [mx, my] = at((rnd() - 0.5) * f * 1.4, (rnd() - 0.5) * f * 1.4);
      g.ellipse(mx, my, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  }
  blocks(g, fallen, stoneTone(rnd));
}

/** A dashed line from `p` to `q` (px): `on` drawn, `off` left, for the caller to stroke. */
function dashes(g: Graphics, p: [number, number], q: [number, number], on: number, off: number) {
  const l = Math.hypot(q[0] - p[0], q[1] - p[1]) || 1;
  const [ux, uy] = [(q[0] - p[0]) / l, (q[1] - p[1]) / l];
  for (let t = 0; t < l; t += on + off) g.moveTo(p[0] + ux * t, p[1] + uy * t).lineTo(p[0] + ux * Math.min(l, t + on), p[1] + uy * Math.min(l, t + on));
}

/**
 * A gatehouse of one block (`town::gatehouse`, in the gate's frame: `a` along from the first tower to the
 * second, `b` across into the town, a bent entrance's mirrored to turn toward the second tower): a square gate
 * tower over its passage (the North's; the South's with an arch at each end of the passage), or the Dry's bent
 * entrance, its way turning inside a square block between its two square towers (those are its `Tower`s).
 * The block's top flagged inside its parapet of merlons, the stair's hatch in a corner; the way through under
 * it shown through the top, its sides dashed, a lintel or an arch over each mouth, a portcullis slot inside the
 * outer one, the doors standing half open. A ruin's has no doors, merlons gone, rubble.
 */
/** A one-block gatehouse's frame (ft; see `drawGateBlock`) and its block's outline (px). */
function blockFrame(s: Shape, form: number) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const side = flagsOf(s, 6) < 0 ? -1 : 1;
  const bent = form === GATE_BENT;
  const mirror = bent && flagsOf(s, 7) < 0;
  const at = (a: number, b: number): [number, number] => {
    const [p, q] = [mirror ? L - a * FT : a * FT, b * FT * side];
    return [x0 + ux * p - uy * q, y0 + uy * p + ux * q];
  };
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const l = L / FT;
  return { at, quad, l, bent, solid: bent ? quad(12, l - 12, -12, 30) : quad(-4, l + 4, -16, 16) };
}

function drawGateBlock(g: Graphics, s: Shape, form: number) {
  const { at, l, bent, solid } = blockFrame(s, form);
  const atPx: At = (a, b) => at(a / FT, b / FT);
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(s.pts[0] + s.pts[2], s.pts[1] + s.pts[3]);
  // The way through (ft), its mouths by edge: the first edge the way in, the other the way out.
  const m = l / 2;
  const way: [number, number][] = bent
    ? [
        [17, -12],
        [27, -12],
        [27, 15],
        [l - 12, 15],
        [l - 12, 25],
        [17, 25],
      ]
    : [
        [m - 6, -16],
        [m + 6, -16],
        [m + 6, 16],
        [m - 6, 16],
      ];
  const mouths = bent ? [0, 3] : [0, 2];
  g.poly(solid.map((v) => v + PX * 1.1)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(solid).fill(ST[1]);
  flagFloor(g, inset(solid, PARAPET), rnd);
  const fallen: number[][] = [];
  parapetRing(g, solid, rnd, 0, [], [], ruined, fallen);
  if (!bent && !ruined) cornerHatch(g, solid, FT * 10);
  // The way under the top, seen through it: paved, its sides dashed.
  const wayPts = way.flatMap(([a, b]) => at(a, b));
  // (A bent way is drawn plain and walled: the turn must read, so it is paved clear of the block's top, its sides inked.)
  g.poly(wayPts).fill(bent ? { color: mix(ST[0], 0xb8ae9a, 0.6), alpha: 0.96 } : { color: mix(ST[0], 0xa39a8a, 0.45), alpha: 0.6 });
  for (let i = 0; i < way.length; i++) {
    if (mouths.includes(i)) continue;
    const j = (i + 1) % way.length;
    if (bent) g.moveTo(...at(...way[i])).lineTo(...at(...way[j]));
    else dashes(g, at(...way[i]), at(...way[j]), FT * 0.9, FT * 0.6);
  }
  g.stroke(bent ? { width: 3.2, color: INK, alpha: 0.95, join: 'round', cap: 'round' } : { width: 2.2, color: INK, alpha: 0.85 });
  if (bent) {
    // A chevron along the way: in, across, out.
    const mid = (p: [number, number], q: [number, number]) => at((p[0] + q[0]) / 2, (p[1] + q[1]) / 2);
    g.moveTo(...mid(way[0], way[1])).lineTo(...at(22, 20)).lineTo(...mid(way[3], way[4])).stroke({ width: 1.4, color: INK, alpha: 0.35 });
  }
  // Over each mouth a lintel, or the South's arch (voussoirs, the keystone lit); a portcullis slot inside the outer.
  mouths.forEach((e, k) => {
    const [p, q] = [way[e], way[(e + 1) % way.length]];
    const n = way[(e + 2) % way.length];
    // (Inward: square to the mouth, toward the way's next corner.)
    const [ex, ey] = [q[0] - p[0], q[1] - p[1]];
    const el = Math.hypot(ex, ey) || 1;
    let [ix, iy] = [-ey / el, ex / el];
    if ((n[0] - q[0]) * ix + (n[1] - q[1]) * iy < 0) [ix, iy] = [-ix, -iy];
    const pt = (t: number, d: number) => at(p[0] + ex * t + ix * d, p[1] + ey * t + iy * d);
    const band: number[] = [];
    const steps = 8;
    for (let i = 0; i <= steps; i++) band.push(...pt(i / steps, 0));
    for (let i = steps; i >= 0; i--) band.push(...pt(i / steps, form === GATE_ARCHED ? 1 + 1.8 * Math.sin((Math.PI * i) / steps) : 2));
    g.poly(band).fill(ST[2]).stroke({ width: LINE * 1.2, color: INK, join: 'round' });
    for (let i = 1; i < steps; i++) {
      if (form === GATE_ARCHED && i === steps / 2) continue;
      const d = form === GATE_ARCHED ? 1 + 1.8 * Math.sin((Math.PI * i) / steps) : 2;
      g.moveTo(...pt(i / steps, 0.2)).lineTo(...pt(i / steps, d - 0.2));
    }
    g.stroke({ width: 1.2, color: ST[0], alpha: 0.8 });
    if (form === GATE_ARCHED) g.poly([...pt(0.44, 0), ...pt(0.56, 0), ...pt(0.56, 2.6), ...pt(0.44, 2.6)]).fill(ST[3]).stroke({ width: LINE, color: INK, join: 'round' });
    if (k === 0) {
      const d = form === GATE_ARCHED ? 3.6 : 3;
      g.poly([...pt(0, d), ...pt(1, d), ...pt(1, d + 0.7), ...pt(0, d + 0.7)]).fill(0x2a2520);
      if (!ruined) g.moveTo(...pt(0.03, d + 0.35)).lineTo(...pt(0.97, d + 0.35)).stroke({ width: FT * 0.22, color: IRON[1] });
    }
  });
  // The doors inside the way in, standing half open.
  if (!ruined) {
    const [a0, a1] = bent ? [17, 27] : [m - 6, m + 6];
    const b0 = bent ? -12 : -16;
    gateDoors(g, atPx, a0 * FT, a1 * FT, (b0 + 5.2) * FT, rnd);
    banner(g, ...at(bent ? (12 + l - 12) / 2 + 4 : m - 10, bent ? 8 : 4));
  } else {
    fallen.push(...rubble(...at(m + (rnd() - 0.5) * 20, (rnd() - 0.5) * 12), 6, FT * 3, rnd));
    for (let k = 0; k < 5; k++) g.ellipse(...at(m + (rnd() - 0.5) * l, (rnd() - 0.5) * 24), FT * 0.7, FT * 0.5).fill({ color: MOSS[1], alpha: 0.75 });
  }
  blocks(g, fallen, stoneTone(rnd));
}

/** Stake tops (`r` px across half), each a cone from above: mid body, its facet toward the light lit, inked. */
function stakeTops(g: Graphics, stakes: [number, number][], r: number) {
  for (const [x, y] of stakes) g.circle(x, y, r);
  if (!stakes.length) return;
  g.fill(ST[1]);
  for (const [x, y] of stakes) {
    g.moveTo(x, y);
    g.arc(x, y, r * 0.92, Math.PI * 0.75, Math.PI * 1.75);
    g.closePath();
  }
  g.fill(ST[2]);
  for (const [x, y] of stakes) g.circle(x + LX * r * 0.3, y + LY * r * 0.3, r * 0.3);
  g.fill(ST[3]);
  for (const [x, y] of stakes) g.circle(x, y, r);
  g.stroke({ width: OUT * 0.8, color: INK });
  for (const [x, y] of stakes) g.moveTo(x - LX * r * 0.15, y - LY * r * 0.15).lineTo(x + LX * r * 0.7, y + LY * r * 0.7);
  g.stroke({ width: 1, color: ST[0], alpha: 0.6 });
}

/**
 * A palisade's log gate (`GATE_LOG`) between its two log towers: packed earth through the gap, a great post at
 * each side, the two leaves (stakes lashed to rails, their tops seen from above) swung open into the town with
 * their swings marked, a plank walk over the gap between the towers, seen through. A ruin's has no leaves or walk,
 * a post gone and logs lying.
 */
function logGate(g: Graphics, s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const side = flagsOf(s, 6) < 0 ? -1 : 1;
  const at: At = (a, b) => [x0 + ux * a - uy * b * side, y0 + uy * a + ux * b * side];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(x0 + x1, y0 + y1);
  const [p0, p1] = [FT * 12, L - FT * 12];
  const D = FT * 6;
  g.poly(quad(p0 - FT, p1 + FT, -D, D)).fill(0xa08c6a);
  const mid = (p0 + p1) / 2;
  for (const d of [-FT * 2.4, FT * 2.4]) g.moveTo(...at(mid + d, -D)).lineTo(...at(mid + d, D));
  g.stroke({ width: FT * 1.1, color: PLUM, alpha: 0.12 });
  // The posts, and the leaves hung on them.
  const R = FT * 0.9;
  const posts: [number, number][] = [];
  if (!ruined || rnd() < 0.5) posts.push(at(p0 + R, 0));
  posts.push(at(p1 - R, 0));
  const len = (p1 - p0) / 2 - R;
  const stakes: [number, number][] = [];
  if (!ruined) {
    const open = 1.05 + rnd() * 0.3;
    for (const dir of [1, -1]) {
      const hinge = dir > 0 ? p0 + R : p1 - R;
      const [du, dv] = [dir * Math.cos(open), Math.sin(open)];
      for (let k = 0; k < 7; k++) {
        const [t0, t1] = [(open * k) / 7, (open * (k + 0.55)) / 7];
        g.moveTo(...at(hinge + dir * Math.cos(t0) * len, Math.sin(t0) * len)).lineTo(...at(hinge + dir * Math.cos(t1) * len, Math.sin(t1) * len));
      }
      g.stroke({ width: 2, color: INK, alpha: 0.6 });
      // The rail behind the stakes, then the stakes along it.
      const rail = [...at(hinge + du * R, dv * R + FT * 0.3), ...at(hinge + du * len, dv * len + FT * 0.3)];
      g.moveTo(rail[0], rail[1]).lineTo(rail[2], rail[3]).stroke({ width: FT * 0.5, color: ST[0], cap: 'round' });
      for (let t = R + STAKE; t < len; t += STAKE * 1.9) stakes.push(at(hinge + du * t, dv * t));
    }
    // The walk over the gap, between the towers: planks, see-through, its edges dashed.
    g.poly(quad(p0 - FT * 0.5, p1 + FT * 0.5, -FT * 3.5, -FT * 1)).fill({ color: WK, alpha: 0.5 });
    for (let a = p0 + FT; a < p1; a += FT * 1.1) g.moveTo(...at(a, -FT * 3.5)).lineTo(...at(a, -FT * 1));
    g.stroke({ width: 1.1, color: ST[0], alpha: 0.4 });
    for (const b of [-FT * 3.5, -FT * 1]) dashes(g, at(p0 - FT * 0.5, b), at(p1 + FT * 0.5, b), FT * 0.9, FT * 0.6);
    g.stroke({ width: 2.2, color: INK, alpha: 0.85 });
  }
  for (const [x, y] of posts) g.circle(x + PX * 0.5, y + PX * 0.5, R);
  g.fill({ color: 0x000000, alpha: 0.22 });
  stakeTops(g, posts, R);
  stakeTops(g, stakes, STAKE);
  if (ruined) {
    const lying: number[][] = [];
    for (let k = 0; k < 3; k++) {
      const [cx, cy] = at(p0 + rnd() * (p1 - p0), (rnd() - 0.5) * D * 2);
      const t = rnd() * Math.PI;
      const [vx, vy] = [Math.cos(t), Math.sin(t)];
      lying.push(logPoly((p, q) => [cx + vx * p - vy * q, cy + vy * p + vx * q], -FT * 3, FT * 3, -STAKE, STAKE));
    }
    logs(g, lying, []);
  }
}

/** How far a gatehouse reaches out each side of its curtain's line (ft): where a drawbridge's leaf hangs
 * when play raises it. */
export const GATE_DEPTH = 11;

/** A gatehouse (`ShapeKind::Gate`) by its look and form: one block (`drawGateBlock`), a palisade's log gate
 * (`logGate`), or twin drum towers (`drumGate`), in its stone's tones. */
export function drawGate(g: Graphics, s: Shape) {
  const form = gateForm(s);
  use(lookOf(s, 4));
  if (form === GATE_SQUARE || form === GATE_ARCHED || form === GATE_BENT) drawGateBlock(g, s, form);
  else if (form === GATE_LOG) logGate(g, s);
  else drumGate(g, s);
  use(null);
}

/**
 * A gatehouse (`ShapeKind::Gate`: the two gate towers' centres; `size` 1 for a ruin's): the
 * passage between the towers paved in setts, masonry jambs down its sides, a stone arch across
 * each end casting its shadow in, a portcullis slot inside each arch, the two doors standing
 * half open with their swings marked; over it all the wall walk, see-through, a winch for the
 * portcullis on some. A ruin's has no doors, walk or portcullis, and one arch broken.
 */
function drumGate(g: Graphics, s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const at = (a: number, b: number): [number, number] => [x0 + ux * a - uy * b, y0 + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(x0 + x1, y0 + y1);
  const R = FT * 15;
  const D = FT * GATE_DEPTH;
  const ab = FT * 2.5;
  const [p0, p1] = [R, L - R];
  // The setts of the passage, rows across the way through, and worn ruts.
  g.poly(quad(p0 - FT, p1 + FT, -D, D)).fill(0xa39a8a);
  for (let b = -D + FT * 2; b < D; b += FT * 2) {
    g.moveTo(...at(p0 - FT, b)).lineTo(...at(p1 + FT, b));
    for (let a = p0 - FT + rnd() * FT * 2.5; a < p1 + FT; a += FT * (2.6 + rnd() * 1.2)) g.moveTo(...at(a, b - FT * 2)).lineTo(...at(a, b));
  }
  g.stroke({ width: 1.2, color: 0x7a7266, alpha: 0.8 });
  for (const d of [-FT * 2.4, FT * 2.4]) g.moveTo(...at((p0 + p1) / 2 + d, -D)).lineTo(...at((p0 + p1) / 2 + d, D));
  g.stroke({ width: FT * 1.1, color: PLUM, alpha: 0.1 });
  // The arches' shadows falling into the passage.
  const sh = FT * 1.4;
  for (const b0 of [-D, D - ab]) g.poly(quad(p0 - FT, p1 + FT, b0, b0 + ab).map((v) => v + sh));
  g.fill({ color: 0x000000, alpha: 0.25 });
  // Jambs down the sides, coursed, lit on the side toward the light.
  for (const [a0, a1] of [
    [p0 - FT * 2, p0 + FT * 0.8],
    [p1 - FT * 0.8, p1 + FT * 2],
  ]) {
    g.poly(quad(a0, a1, -D, D)).fill(ST[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let b = -D + FT * 1.8; b < D; b += FT * 1.8) g.moveTo(...at(a0, b)).lineTo(...at(a1, b));
    g.stroke({ width: 1.2, color: ST[0], alpha: 0.8 });
  }
  // Portcullis grooves inside each arch: the slot the raised gate hangs in, its iron foot showing.
  for (const b of [-D + ab + FT * 0.7, D - ab - FT * 0.7]) {
    g.poly(quad(p0, p1, b - FT * 0.35, b + FT * 0.35)).fill(0x2a2520);
    if (!ruined) g.moveTo(...at(p0 + FT * 0.3, b)).lineTo(...at(p1 - FT * 0.3, b)).stroke({ width: FT * 0.22, color: IRON[1] });
  }
  // The arches: voussoirs across the way, the keystone lighter, a sunlit coping edge; a ruin's
  // has one broken partway, its stones fallen in.
  const broken = ruined ? (rnd() < 0.5 ? 0 : 1) : -1;
  const fallen: number[][] = [];
  [-D, D - ab].forEach((b0, k) => {
    const end = k === broken ? p0 - FT + (p1 - p0 + FT * 2) * (0.35 + rnd() * 0.3) : p1 + FT;
    g.poly(quad(p0 - FT, end, b0, b0 + ab)).fill(ST[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let a = p0 - FT + FT * 1.6; a < end - FT * 0.4; a += FT * 1.6) g.moveTo(...at(a, b0)).lineTo(...at(a, b0 + ab));
    g.stroke({ width: 1.2, color: ST[0], alpha: 0.85 });
    const mid = (p0 + p1) / 2;
    if (mid < end) g.poly(quad(mid - FT * 0.7, mid + FT * 0.7, b0, b0 + ab)).fill(ST[2]).stroke({ width: 1.4, color: INK });
    // The coping's lit edge: the long side toward the light.
    const litSide = lit(-uy, ux) > 0 ? b0 + ab - FT * 0.25 : b0 + FT * 0.25;
    g.moveTo(...at(p0 - FT, litSide)).lineTo(...at(end, litSide)).stroke({ width: FT * 0.3, color: ST[3], alpha: 0.7 });
    if (k === broken) fallen.push(...rubble(...at(end + FT * 2, b0 + ab / 2 + (k ? -FT * 2 : FT * 2)), 6, FT * 2, rnd));
  });
  // The wall walk carried over the gate, between the towers: overhead, so shown see-through with
  // its edges dashed; on some gates the portcullis's winch stands on it, chains to the slot.
  if (!ruined) {
    const w = FT * 4;
    const [a0, a1] = [p0 - FT, p1 + FT];
    g.poly(quad(a0, a1, -w, w)).fill({ color: WK, alpha: 0.5 });
    for (const side of [-1, 1]) {
      const [d0, d1] = side < 0 ? [-w, -w + PARAPET] : [w - PARAPET, w];
      g.poly(quad(a0, a1, d0, d1)).fill({ color: ST[2], alpha: 0.6 });
      for (let a = a0 + FT * 1.6; a < a1 - FT * 1.6; a += FT * 3.9) g.poly(quad(a, a + FT * 1.4, d0 + 1.5, d1 - 1.5));
    }
    g.fill({ color: mix(ST[0], PLUM, 0.3), alpha: 0.75 });
    for (const b of [-w, w]) for (let a = a0; a < a1; a += FT * 1.5) g.moveTo(...at(a, b)).lineTo(...at(Math.min(a + FT * 0.9, a1), b));
    g.stroke({ width: 2.5, color: INK, alpha: 0.9 });
    if (rnd() < 0.5) {
      const mid = (p0 + p1) / 2;
      const vb = -w + PARAPET + FT * 1.1;
      const slot = -D + ab + FT * 0.7;
      // Chains down to the portcullis.
      for (const o of [-FT * 1.8, FT * 1.8]) {
        for (let b = vb; b > slot; b -= FT * 0.7) g.moveTo(...at(mid + o, b)).lineTo(...at(mid + o, Math.max(slot, b - FT * 0.45)));
      }
      g.stroke({ width: FT * 0.25, color: IRON[0] });
      // The drum, rope wound on it, a post at each end and a spoked wheel to turn it.
      const drum = quad(mid - FT * 3, mid + FT * 3, vb - FT * 0.7, vb + FT * 0.7);
      g.poly(drum.map((v) => v + FT * 0.3)).fill({ color: 0x000000, alpha: 0.25 });
      g.poly(drum).fill(WOOD[1]).stroke({ width: 1.8, color: INK, join: 'round' });
      g.poly(quad(mid - FT * 3, mid + FT * 3, vb - FT * 0.7, vb - FT * 0.2)).fill(WOOD[2]);
      for (let a = mid - FT * 2.4; a < mid + FT * 2.5; a += FT * 0.6) g.moveTo(...at(a, vb - FT * 0.7)).lineTo(...at(a, vb + FT * 0.7));
      g.stroke({ width: 1.2, color: WOOD[0] });
      for (const a of [mid - FT * 3.5, mid + FT * 3.5]) g.poly(quad(a - FT * 0.5, a + FT * 0.5, vb - FT * 0.6, vb + FT * 0.6)).fill(WOOD[0]).stroke({ width: 1.6, color: INK });
      const [wx, wy] = at(mid + FT * 4.6, vb);
      for (let k = 0; k < 4; k++) {
        const t = (k / 4) * Math.PI + rnd() * 0.3;
        g.moveTo(wx - Math.cos(t) * FT * 1.3, wy - Math.sin(t) * FT * 1.3).lineTo(wx + Math.cos(t) * FT * 1.3, wy + Math.sin(t) * FT * 1.3);
      }
      g.stroke({ width: FT * 0.3, color: WOOD[1], cap: 'round' });
      g.circle(wx, wy, FT * 1.1).stroke({ width: FT * 0.25, color: WOOD[0] });
      g.circle(wx, wy, FT * 0.35).fill(IRON[1]).stroke({ width: 1.2, color: INK });
    }
  }
  // The doors (drawn over the walk, to be read).
  if (!ruined) gateDoors(g, at, p0, p1, -D + ab + FT * 1.7, rnd);
  if (ruined) {
    for (let k = 0; k < 4; k++) {
      const [mx, my] = at(p0 + rnd() * (p1 - p0), (rnd() - 0.5) * 2 * D);
      g.ellipse(mx, my, FT * 0.7, FT * 0.5).fill({ color: MOSS[1], alpha: 0.75 });
    }
    blocks(g, fallen, stoneTone(rnd));
  }
}

/** A gate's doors: two timber leaves hung on the jambs (`p0`, `p1` along, `bh` across in `at`'s frame) just
 * inside the outer portcullis, standing half open into the passage, each with its swing marked. */
function gateDoors(g: Graphics, at: At, p0: number, p1: number, bh: number, rnd: () => number) {
  const len = (p1 - p0) / 2 - FT * 0.2;
  const open = 0.95 + rnd() * 0.3;
  const th = FT * 1.4;
  for (const dir of [1, -1]) {
    const hinge = dir > 0 ? p0 : p1;
    const [du, dv] = [dir * Math.cos(open), Math.sin(open)];
    // Its thickness on the outer side (toward the portcullis).
    const [nu, nv] = [dir * dv, -dir * du];
    const pt = (t: number, k: number) => at(hinge + du * t + nu * k, bh + dv * t + nv * k);
    // The swing: an arc from where the leaf shuts to where it stands, in dashes.
    for (let k = 0; k < 7; k++) {
      const [t0, t1] = [(open * k) / 7, (open * (k + 0.55)) / 7];
      g.moveTo(...at(hinge + dir * Math.cos(t0) * len, bh + Math.sin(t0) * len)).lineTo(...at(hinge + dir * Math.cos(t1) * len, bh + Math.sin(t1) * len));
    }
    g.stroke({ width: 2, color: INK, alpha: 0.6 });
    const leaf = [...pt(0, 0), ...pt(len, 0), ...pt(len, th), ...pt(0, th)];
    g.poly(leaf.map((v) => v + FT * 0.35)).fill({ color: 0x000000, alpha: 0.28 });
    g.poly(leaf).fill(WOOD[1]);
    g.poly([...pt(0, 0), ...pt(len, 0), ...pt(len, th * 0.35), ...pt(0, th * 0.35)]).fill(WOOD[2]);
    // Its boards, end on along the leaf.
    for (const k of [0.35, 0.68]) g.moveTo(...pt(0, th * k)).lineTo(...pt(len, th * k));
    g.stroke({ width: 1.2, color: WOOD[0] });
    // Iron bands across the leaf and the studs along it.
    for (const t of [0.18, 0.5, 0.82]) g.moveTo(...pt(len * t, -1)).lineTo(...pt(len * t, th + 1));
    g.stroke({ width: FT * 0.28, color: IRON[0] });
    g.poly(leaf).stroke({ width: 2, color: INK, join: 'round' });
    g.circle(...pt(0, th * 0.5), FT * 0.35).fill(IRON[1]).stroke({ width: 1.2, color: INK });
  }
}

/** A garden wall's height (ft): its shadow's reach. */
const GARDEN_FT = 4;
/** A gate pier's half side (px; `battlemap`'s, 7-ft piers): where a forecourt gate's leaves hang. */
const PIER_HALF = PX * 0.7;

/** A shape's points as pairs. */
const pairs = (pts: number[]): [number, number][] => Array.from({ length: pts.length >> 1 }, (_, k) => [pts[k * 2], pts[k * 2 + 1]]);

/** A coping stone's tone as its face meets the light (`l`, -1 .. 1): sunlit toward cream, shaded toward plum. */
const coping = (l: number, rnd: () => number) => {
  const t = l >= 0 ? mix(STONE[2], STONE[3], l * 0.7) : mix(STONE[2], mix(STONE[1], PLUM, 0.25), -l * 0.85);
  return mix(t, rnd() < 0.5 ? STONE[1] : STONE[3], rnd() * 0.18);
};

/**
 * A forecourt's low garden wall (`ShapeKind::GardenWall`, one straight segment each, `size` thick):
 * a short soft shadow (it stands about 4 ft), a saddle-backed coping of flat stones along its top,
 * the face toward the light sunlit and the other shaded, joints across it, the odd tuft of moss; a
 * bold ink outline. Segments meeting end to end are mitred into one neat corner; a free end (at a
 * gate pier or the building's wall) is cut square.
 */
export function drawGardenWalls(g: Graphics, walls: Shape[]) {
  const segs = walls.map((s) => {
    const [x0, y0, x1, y1] = s.pts;
    const l = Math.hypot(x1 - x0, y1 - y0) || 1;
    return { x0, y0, x1, y1, l, ux: (x1 - x0) / l, uy: (y1 - y0) / l, half: s.size / 2, rnd: rngAt(x0 * 0.7 + x1, y0 * 0.7 + y1) };
  });
  type Seg = (typeof segs)[number];
  // Each end's two corners (on the segment's left, +n, and right) and whether it is mitred.
  const ends = segs.map((s) => {
    const n = [-s.uy, s.ux];
    const out: { left: [number, number]; right: [number, number]; mitred: boolean }[] = [];
    for (const end of [0, 1]) {
      const [px, py] = end ? [s.x1, s.y1] : [s.x0, s.y0];
      let corner = { left: [px + n[0] * s.half, py + n[1] * s.half] as [number, number], right: [px - n[0] * s.half, py - n[1] * s.half] as [number, number], mitred: false };
      const o = segs.find((t) => t !== s && (Math.hypot(t.x0 - px, t.y0 - py) < 1.5 || Math.hypot(t.x1 - px, t.y1 - py) < 1.5));
      if (o) {
        // Heading into the corner along this one, out of it along the other.
        const din = end ? [s.ux, s.uy] : [-s.ux, -s.uy];
        const dout = Math.hypot(o.x0 - px, o.y0 - py) < 1.5 ? [o.ux, o.uy] : [-o.ux, -o.uy];
        const [ax, ay, bx, by] = [-din[1], din[0], -dout[1], dout[0]];
        const ml = Math.hypot(ax + bx, ay + by);
        if (ml > 0.2) {
          const [mx, my] = [(ax + bx) / ml, (ay + by) / ml];
          const k = Math.min(s.half * 3, s.half / (mx * ax + my * ay));
          const q: [number, number] = [px + mx * k, py + my * k];
          const q2: [number, number] = [px - mx * k, py - my * k];
          corner = end ? { left: q, right: q2, mitred: true } : { left: q2, right: q, mitred: true };
        }
      }
      out.push(corner);
    }
    return out;
  });
  // A point `a` along a segment and `d` across it (toward its left).
  const at = (s: Seg, a: number, d: number): [number, number] => [s.x0 + s.ux * a - s.uy * d, s.y0 + s.uy * a + s.ux * d];
  const outline = (k: number) => {
    const [A, B] = ends[k];
    return [...A.right, ...B.right, ...B.left, ...A.left];
  };
  // The shadow, short and soft (two slices).
  const reach = GARDEN_FT * FT * 0.55;
  for (const [f, alpha] of [
    [1, 0.12],
    [0.55, 0.16],
  ]) {
    for (let k = 0; k < segs.length; k++) g.poly(outline(k).map((v) => v + reach * f));
    g.fill({ color: 0x000000, alpha });
  }
  // The coping in blocks, each in two faces either side of the ridge, lit as they face.
  segs.forEach((s, k) => {
    const [A, B] = ends[k];
    const l = lit(-s.uy, s.ux);
    // The joints, clear of the ends' mitres.
    const margin = s.half * 1.4;
    const cuts: number[] = [0];
    for (let a = margin + s.rnd() * FT * 1.6; a < s.l - margin; a += FT * (2.2 + s.rnd() * 1.4)) cuts.push(a);
    cuts.push(s.l);
    const edge = (i: number, side: 1 | -1): [number, number] => {
      if (i === 0) return side > 0 ? A.left : A.right;
      if (i === cuts.length - 1) return side > 0 ? B.left : B.right;
      return at(s, cuts[i] + (s.rnd() - 0.5) * FT * 0.25, side * s.half);
    };
    const lefts = cuts.map((_, i) => edge(i, 1));
    const rights = cuts.map((_, i) => edge(i, -1));
    const ridge = cuts.map((a) => at(s, a, 0));
    for (let i = 0; i + 1 < cuts.length; i++) {
      g.poly([...ridge[i], ...ridge[i + 1], ...lefts[i + 1], ...lefts[i]]).fill(coping(l, s.rnd));
      g.poly([...ridge[i], ...ridge[i + 1], ...rights[i + 1], ...rights[i]]).fill(coping(-l, s.rnd));
    }
    // The sunlit arris along the edge toward the light.
    const sun = l >= 0 ? 1 : -1;
    g.moveTo(...at(s, A.mitred ? s.half : 0, sun * (s.half - FT * 0.28))).lineTo(...at(s, B.mitred ? s.l - s.half : s.l, sun * (s.half - FT * 0.28)));
    g.stroke({ width: FT * 0.35, color: STONE[3], alpha: 0.55 * Math.abs(l) });
    // The ridge and the joints across it, a mitred end's joint too.
    g.moveTo(...ridge[0]).lineTo(...ridge[ridge.length - 1]);
    for (let i = 1; i + 1 < cuts.length; i++) g.moveTo(...lefts[i]).lineTo(...ridge[i]).lineTo(...rights[i]);
    g.stroke({ width: 1.6, color: STONE[0], alpha: 0.95, join: 'round' });
    for (const [E, P] of [
      [A, [s.x0, s.y0]],
      [B, [s.x1, s.y1]],
    ] as const) {
      if (E.mitred) g.moveTo(...E.left).lineTo(...P).lineTo(...E.right);
    }
    g.stroke({ width: 1.6, color: STONE[0], alpha: 0.9, join: 'round' });
    // The odd tuft of moss on the shaded face, or at the wall's foot.
    const tufts = Math.floor(s.l / (FT * 30) + s.rnd() * 0.6);
    for (let t = 0; t < tufts; t++) {
      const a = s.half * 1.5 + s.rnd() * Math.max(0, s.l - s.half * 3);
      const [mx, my] = at(s, a, -sun * s.half * (0.35 + s.rnd() * 0.25));
      const r = FT * (0.22 + s.rnd() * 0.14);
      g.ellipse(mx, my, r * 1.5, r).fill({ color: MOSS[0], alpha: 0.75 });
      g.ellipse(mx - r * 0.35, my - r * 0.3, r * 0.75, r * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  });
  // Bold ink round it all: the long sides and the square ends, not across a mitre.
  segs.forEach((s, k) => {
    const [A, B] = ends[k];
    g.moveTo(...A.left).lineTo(...B.left);
    if (!B.mitred) g.lineTo(...B.right);
    else g.moveTo(...B.right);
    g.lineTo(...A.right);
    if (!A.mitred) g.lineTo(...A.left);
  });
  g.stroke({ width: 2.6, color: INK, join: 'round', cap: 'round' });
}

/** The convex hull of points, as a flat list round it. */
function hull(p: [number, number][]): number[] {
  const s = [...p].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const cross = (o: number[], a: number[], b: number[]) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const half = (list: [number, number][]) => {
    const h: [number, number][] = [];
    for (const q of list) {
      while (h.length >= 2 && cross(h[h.length - 2], h[h.length - 1], q) <= 0) h.pop();
      h.push(q);
    }
    h.pop();
    return h;
  };
  return [...half(s), ...half([...s].reverse())].flat();
}

/**
 * A forecourt gate's pier (`ShapeKind::GatePier`: its middle, then its sides' angle; `size` half its
 * side): a solid square of stone seen from above, its shadow to the south-east; the cap's fillet,
 * then its weathered slopes stepping in (lit on the faces toward the light, shaded on the others),
 * its flat top and a ball finial on a little plinth, the ball shaded round and casting its own shadow.
 */
export function drawGatePier(g: Graphics, s: Shape) {
  const [x, y, h] = [s.pts[0], s.pts[1], s.size];
  const t = s.pts.length > 2 ? s.pts[2] / PX : 0;
  const [ux, uy] = [Math.cos(t), Math.sin(t)];
  const rnd = rngAt(x * 1.7 + y * 0.23, y * 0.9 - x * 0.31);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  // A square `r` from the middle, its corners a little out of true.
  const square = (r: number, j = 0.6) => {
    const out: number[] = [];
    for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) out.push(...at(a * r + (rnd() - 0.5) * j, b * r + (rnd() - 0.5) * j));
    return out;
  };
  // The shadow: the block swept away from the light.
  const D = PX * 0.38;
  const foot = square(h, 0);
  g.poly(hull(pairs([...foot, ...foot.map((v) => v + D)]))).fill({ color: 0x000000, alpha: 0.22 });
  // The fillet, the widest of the cap.
  g.poly(square(h)).fill(STONE[1]).stroke({ width: 3, color: INK, join: 'round' });
  g.poly(square(h * 0.9)).fill(STONE[2]);
  // The weathering: four slopes from the fillet up to the top, each as it faces the light.
  const a0 = square(h * 0.84, 0.4);
  const a1 = square(h * 0.58, 0.4);
  for (let k = 0; k < 4; k++) {
    const m = (k + 1) % 4;
    const face = [a0[k * 2], a0[k * 2 + 1], a0[m * 2], a0[m * 2 + 1], a1[m * 2], a1[m * 2 + 1], a1[k * 2], a1[k * 2 + 1]];
    // Its outward way: from the middle to its outer edge's midpoint.
    const l = lit((a0[k * 2] + a0[m * 2]) / 2 - x, (a0[k * 2 + 1] + a0[m * 2 + 1]) / 2 - y);
    g.poly(face).fill(l >= 0 ? mix(STONE[2], STONE[3], 0.3 + l * 0.6) : mix(STONE[1], PLUM, 0.12 - l * 0.28));
  }
  g.poly(a0).stroke({ width: 1.6, color: INK, alpha: 0.85, join: 'round' });
  for (let k = 0; k < 4; k++) g.moveTo(a0[k * 2], a0[k * 2 + 1]).lineTo(a1[k * 2], a1[k * 2 + 1]);
  g.stroke({ width: 1.2, color: STONE[0], alpha: 0.8 });
  // The flat top, and the finial's plinth on it.
  g.poly(a1).fill(STONE[2]).stroke({ width: 1.6, color: INK, alpha: 0.9, join: 'round' });
  const pl = square(h * 0.36, 0.3);
  g.poly(pl.map((v) => v + h * 0.12)).fill({ color: PLUM, alpha: 0.25 });
  g.poly(pl).fill(STONE[1]).stroke({ width: 1.4, color: INK, join: 'round' });
  g.poly(square(h * 0.28, 0.2)).fill(mix(STONE[2], STONE[1], 0.3));
  // The ball: its shadow on the top, a shaded round lit toward the light, a soft sheen (stone: matte).
  const r = h * 0.29;
  g.circle(x + r * 0.45, y + r * 0.45, r).fill({ color: PLUM, alpha: 0.3 });
  const ball = (k: number, ox: number, oy: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 18; i++) {
      const a = (i / 18) * Math.PI * 2;
      const rr = r * k * (1 + (rnd() - 0.5) * 0.05);
      pts.push(x + ox + Math.cos(a) * rr, y + oy + Math.sin(a) * rr);
    }
    return pts;
  };
  const round = ball(1, 0, 0);
  g.poly(round).fill(mix(STONE[1], PLUM, 0.18));
  g.poly(ball(0.78, LX * r * 0.2, LY * r * 0.2)).fill(STONE[2]);
  g.poly(ball(0.4, LX * r * 0.38, LY * r * 0.38)).fill({ color: STONE[3], alpha: 0.85 });
  g.poly(round).stroke({ width: 2, color: INK, join: 'round' });
}

/**
 * A forecourt's gate (`ShapeKind::ForecourtGate`: the opening's two ends at the piers' inner faces,
 * then a point inside; `size` its style, 0 wrought iron, 1 timber), seen from above as the map's other
 * doors and gates are: two leaves standing open, each hung on its pier's inner corner and swung into
 * the forecourt a little past square, the pair mirrored about the gate's axis and turned with it,
 * drawn over the piers' edges, each with its swing marked in dashes. A timber leaf is the top of its
 * boards, their joints across it, iron straps from the hinge along its faces; a wrought-iron one its
 * top rail, a spear finial on every bar and a C-scroll on each face, a stile post at the hinge and a
 * knob at the meeting end. Soft shadows to the south-east, an iron leaf's faint (it is open work).
 */
export function drawForecourtGate(g: Graphics, s: Shape) {
  const [ax, ay, bx, by, ix, iy] = s.pts;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  // Into the forecourt.
  const inward = (ix - ax) * -uy + (iy - ay) * ux >= 0 ? 1 : -1;
  const [vx, vy] = [-uy * inward, ux * inward];
  const timber = Math.round(s.size / PX) === 1;
  const rnd = rngAt(ax + bx, ay + by);
  // Its thickness from above, and its length (the two shut meet in the middle).
  const th = FT * (timber ? 1 : 0.8);
  const len = L / 2 - FT * 0.2;
  // Open a little past square, both alike.
  const open = (104 * Math.PI) / 180;
  for (const end of [0, 1]) {
    const dir = end ? -1 : 1;
    // Across the opening toward the other leaf (`dir`), into the forecourt (`v`).
    const way = (t: number): [number, number] => [ux * dir * Math.cos(t) + vx * Math.sin(t), uy * dir * Math.cos(t) + vy * Math.sin(t)];
    // The hinge on the pier's inner corner, the leaf's face flush with the pier's face to the opening.
    const [px, py] = end ? [bx, by] : [ax, ay];
    const [hx, hy] = [px + vx * (PIER_HALF - th * 0.5) + ux * dir * th * 0.5, py + vy * (PIER_HALF - th * 0.5) + uy * dir * th * 0.5];
    const [dx, dy] = way(open);
    // Across the leaf, from face to face.
    const [nx, ny] = [-dy, dx];
    // A point `p` of the way along the leaf (0 at the hinge) and `q` across it (-1 .. 1, face to face).
    const P = (p: number, q: number): [number, number] => [hx + dx * len * p + nx * th * 0.5 * q, hy + dy * len * p + ny * th * 0.5 * q];
    // The swing: an arc from where the leaf shuts to where it stands, in dashes.
    for (let k = 0; k < 8; k++) {
      const [t0, t1] = [(open * k) / 8, (open * (k + 0.5)) / 8];
      const [c0, c1] = [way(t0), way(t1)];
      g.moveTo(hx + c0[0] * len, hy + c0[1] * len).lineTo(hx + c1[0] * len, hy + c1[1] * len);
    }
    g.stroke({ width: 1.6, color: INK, alpha: 0.45 });
    // Its shadow, swept away from the light.
    const outline = [...P(0, -1), ...P(1, -1), ...P(1, 1), ...P(0, 1)];
    g.poly(hull(pairs([...outline, ...outline.map((v) => v + FT * (timber ? 1.5 : 1.1))]))).fill({ color: 0x000000, alpha: timber ? 0.2 : 0.08 });
    // (Which face is toward the light: +1 or -1.)
    const sun = lit(nx, ny) >= 0 ? 1 : -1;
    if (timber) timberLeaf(g, P, len, sun, rnd);
    else ironLeaf(g, P, len, th, sun);
  }
}

/** A timber gate leaf from above (`P(p, q)`: along it from the hinge, across it face to face; `sun`
 * the face toward the light): the top of its boards, a lit band along the sunlit face, the joints
 * between the boards across it, iron straps from the hinge along both faces, nailed; its hinge pin. */
function timberLeaf(g: Graphics, P: (p: number, q: number) => [number, number], len: number, sun: number, rnd: () => number) {
  // A little out of true: hand-made.
  const j = () => (rnd() - 0.5) * 0.12;
  const panel = [...P(0, -1), ...P(0.5, -1 + j()), ...P(1, -1), ...P(1, 1), ...P(0.5, 1 + j()), ...P(0, 1)];
  g.poly(panel).fill(WOOD[1]);
  g.poly([...P(0.02, sun * 0.95), ...P(0.98, sun * 0.95), ...P(0.98, sun * 0.25), ...P(0.02, sun * 0.25)]).fill({ color: WOOD[2], alpha: 0.85 });
  // The boards, their joints across the leaf, some a shade lighter.
  const n = Math.max(4, Math.round(len / (FT * 1.05)));
  for (let k = 0; k < n; k++) if (rnd() < 0.35) g.poly([...P(k / n, -0.9), ...P((k + 1) / n, -0.9), ...P((k + 1) / n, 0.9), ...P(k / n, 0.9)]).fill({ color: WOOD[2], alpha: 0.3 });
  for (let k = 1; k < n; k++) g.moveTo(...P(k / n + j() * 0.2, -0.95)).lineTo(...P(k / n + j() * 0.2, 0.95));
  g.stroke({ width: 1.3, color: WOOD[0] });
  // The straps along both faces from the hinge, pointed at their ends, a nail or two.
  for (const q of [-1, 1]) {
    g.poly([...P(0, q * 1.08), ...P(0.6, q * 1.05), ...P(0.66, q * 0.95), ...P(0, q * 0.82)]).fill(IRON[0]);
    for (const p of [0.2, 0.45]) g.circle(...P(p, q * 0.95), 1.2).fill(IRON[2]);
  }
  g.poly(panel).stroke({ width: 2.2, color: INK, join: 'round' });
  g.circle(...P(0, 0), FT * 0.28).fill(IRON[1]).stroke({ width: 1.2, color: INK });
}

/** A wrought-iron gate leaf from above (`P(p, q)`: along it from the hinge, across it face to face;
 * `th` its depth, `sun` the face toward the light): the top rail, lit along its sunlit edge; a spear
 * finial on every bar, a lozenge across the rail; a C-scroll on each face, a third and two thirds of
 * the way along; a square stile post at the hinge with its pin, a knob on the meeting stile. Iron in
 * ink, each line drawn over a wider one of ink. */
function ironLeaf(g: Graphics, P: (p: number, q: number) => [number, number], len: number, th: number, sun: number) {
  const iron = (draw: () => void, w: number, c: number) => {
    draw();
    g.stroke({ width: w + 1.8, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: w, color: c, cap: 'round', join: 'round' });
  };
  const n = Math.max(5, Math.round(len / (FT * 0.95)));
  // The scrolls: a C-scroll on each face, a third of the way along on one and two thirds on the
  // other, curling from the rail out and back toward the hinge.
  iron(() => {
    for (const [p0, side] of [
      [0.36, -1],
      [0.68, 1],
    ]) {
      const r = th * 0.42;
      const [cx, cy] = P(p0, side * (1 + (2 * r) / th));
      const a0 = Math.atan2(...((): [number, number] => {
        const [ox, oy] = P(p0, side);
        return [oy - cy, ox - cx];
      })());
      for (let i = 0; i <= 14; i++) {
        const a = a0 + side * (i / 14) * Math.PI * 1.7;
        const rr = r * (1 - i * 0.04);
        if (i) g.lineTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
        else g.moveTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
      }
    }
  }, 1.3, IRON[1]);
  // The top rail, hinge to meeting stile.
  iron(() => {
    g.moveTo(...P(0, 0)).lineTo(...P(1, 0));
  }, th * 0.5, IRON[0]);
  g.moveTo(...P(0.04, sun * 0.2)).lineTo(...P(0.96, sun * 0.2)).stroke({ width: 0.9, color: IRON[2], alpha: 0.85 });
  // The bars' spear finials: lozenges across the rail.
  for (let k = 1; k < n; k++) {
    const p = k / n;
    const d = Math.min(0.06, 0.3 / n);
    g.poly([...P(p - d, 0), ...P(p, -1.1), ...P(p + d, 0), ...P(p, 1.1)]).fill(IRON[1]).stroke({ width: 1, color: INK, join: 'round' });
    g.circle(...P(p, 0), Math.max(1, th * 0.12)).fill(IRON[2]);
  }
  // The hinge stile's square post and pin, the meeting stile's knob.
  g.poly([...P(-0.03, -1.1), ...P(0.07, -1.1), ...P(0.07, 1.1), ...P(-0.03, 1.1)]).fill(IRON[0]).stroke({ width: 1.4, color: INK, join: 'round' });
  g.circle(...P(0.02, 0), FT * 0.2).fill(IRON[2]).stroke({ width: 1, color: INK });
  g.circle(...P(1, 0), FT * 0.28).fill(IRON[1]).stroke({ width: 1.2, color: INK });
}

/**
 * A castle's drawbridge, lowered (`ShapeKind::Drawbridge`: the gate's middle on the curtain's line,
 * the far end on the moat's far bank, then `[wet, 0]`; `size` its width): a heavy timber deck out
 * from under the gatehouse, its planks laid across it between two stout side beams (the sunlit one
 * lit along its top), iron bands across it and an iron-shod lifting edge, nailed; the lifting
 * chains from rings at its far corners back along its beams into the gatehouse (drawn over its
 * inner end); its shadow on the water, or further down on a dry ditch's floor, to the south-east.
 * Inked bold. Its landing on the far bank (`drawDrawbridgeSill`) is drawn apart: it stays when play
 * raises the deck (`drawRaisedDrawbridge`).
 */
export function drawDrawbridge(g: Graphics, s: Shape) {
  const [ax, ay, bx, by] = s.pts;
  const wet = flagsOf(s, 4) === 1;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  const [vx, vy] = [-uy, ux];
  const half = s.size * 0.5;
  const rnd = rngAt(ax + bx, ay + by);
  // `p` along it from the gate (0) to the lifting edge (1), `q` across it (-1 .. 1).
  const P = (p: number, q: number): [number, number] => [ax + ux * L * p + vx * half * q, ay + uy * L * p + vy * half * q];
  const deck = [...P(0, -1), ...P(1, -1), ...P(1, 1), ...P(0, 1)];
  const drop = FT * (wet ? 1.3 : 3.2);
  g.poly(hull(pairs([...deck, ...deck.map((v) => v + drop)]))).fill({ color: 0x000000, alpha: wet ? 0.24 : 0.32 });
  // The planks across it, some a shade lighter, their joints a little out of true.
  const beam = 0.2;
  g.poly(deck).fill(WOOD[1]);
  const n = Math.max(6, Math.round(L / (FT * 1.1)));
  const j = () => (rnd() - 0.5) * 0.25;
  for (let k = 0; k < n; k++) if (rnd() < 0.3) g.poly([...P(k / n, -1), ...P((k + 1) / n, -1), ...P((k + 1) / n, 1), ...P(k / n, 1)]).fill({ color: WOOD[2], alpha: 0.32 });
  for (let k = 1; k < n; k++) {
    const t = k / n + j() / n;
    g.moveTo(...P(t, -1 + beam)).lineTo(...P(t + j() / n, 1 - beam));
  }
  g.stroke({ width: 1.3, color: WOOD[0] });
  // The side beams, the sunlit one's top lit.
  const sun = lit(vx, vy) >= 0 ? 1 : -1;
  for (const q of [-1, 1]) {
    g.poly([...P(0, q), ...P(1, q), ...P(1, q * (1 - beam)), ...P(0, q * (1 - beam))]).fill(q === sun ? WOOD[1] : WOOD[0]);
    g.moveTo(...P(0, q * (1 - beam))).lineTo(...P(1, q * (1 - beam)));
  }
  g.stroke({ width: 1.4, color: INK, alpha: 0.7 });
  g.poly([...P(0, sun * 0.97), ...P(1, sun * 0.97), ...P(1, sun * 0.9), ...P(0, sun * 0.9)]).fill({ color: WOOD[2], alpha: 0.9 });
  // Iron bands across it, and the shod lifting edge; nail heads along them.
  const band = (p0: number, p1: number) => {
    g.poly([...P(p0, -1), ...P(p1, -1), ...P(p1, 1), ...P(p0, 1)]).fill(IRON[0]);
    const pm = (p0 + p1) / 2;
    for (const q of [-0.62, -0.2, 0.2, 0.62]) g.circle(...P(pm, q), 1.2).fill(IRON[2]);
  };
  const t = (FT * 0.5) / L;
  band(0.34 - t * 0.5, 0.34 + t * 0.5);
  band(0.68 - t * 0.5, 0.68 + t * 0.5);
  band(1 - t * 1.4, 1);
  // The chains: from a ring at each far corner back along the beam, links alternately flat and
  // edge-on.
  const ring = 0.86;
  for (const q of [-1, 1]) {
    const [rx, ry] = P(ring, q * (1 - beam * 0.5));
    const links = Math.max(3, Math.round((L * ring) / (FT * 0.9)));
    for (let k = 0; k < links; k++) {
      const [p0, p1] = [ring * (1 - k / links), ring * (1 - (k + 0.8) / links)];
      const [c0, c1] = [P(p0, q * (1 - beam * 0.5)), P(p1, q * (1 - beam * 0.5))];
      if (k % 2) {
        g.moveTo(...c0).lineTo(...c1).stroke({ width: 3.4, color: INK, cap: 'round' });
        g.moveTo(...c0).lineTo(...c1).stroke({ width: 1.6, color: IRON[1], cap: 'round' });
      } else {
        const [mx, my] = [(c0[0] + c1[0]) / 2, (c0[1] + c1[1]) / 2];
        g.circle(mx, my, FT * 0.3).stroke({ width: 2.6, color: INK }).circle(mx, my, FT * 0.3).stroke({ width: 1.2, color: IRON[2] });
      }
    }
    g.circle(rx, ry, FT * 0.42).stroke({ width: 3, color: INK }).circle(rx, ry, FT * 0.42).stroke({ width: 1.4, color: IRON[1] });
  }
  g.poly(deck).stroke({ width: 2.6, color: INK, join: 'round' });
}

/** A drawbridge's landing on the far bank: a timber sill a little wider than the deck, its boards along it. */
export function drawDrawbridgeSill(g: Graphics, s: Shape) {
  const [ax, ay, bx, by] = s.pts;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  const [vx, vy] = [-uy, ux];
  const half = s.size * 0.5;
  const P = (p: number, q: number): [number, number] => [ax + ux * L * p + vx * half * q, ay + uy * L * p + vy * half * q];
  const sill = (FT * 2.5) / L;
  const land = [...P(1, -1.15), ...P(1 + sill, -1.15), ...P(1 + sill, 1.15), ...P(1, 1.15)];
  g.poly(land).fill(WOOD[0]);
  for (const q of [-0.4, 0.4]) g.moveTo(...P(1 + sill * 0.08, q)).lineTo(...P(1 + sill * 0.92, q));
  g.stroke({ width: 1.1, color: WOOD[2], alpha: 0.6 });
  g.poly(land).stroke({ width: 2.2, color: INK, join: 'round' });
}

/**
 * A drawbridge raised (play: the DM shut it as a door): its leaf stands upright against the gatehouse's
 * outer arch (`GATE_DEPTH` out from the curtain's line), seen from above as a short timber stub across the gateway a little wider than the deck: its outer face
 * toned by the light from the north-west, the planks' ends along its top, the iron-shod lifting edge
 * nailed along it, a chain ring at each end; its tall shadow falls on the moat to the south-east.
 */
export function drawRaisedDrawbridge(g: Graphics, s: Shape) {
  const [ax, ay, bx, by] = s.pts;
  const wet = flagsOf(s, 4) === 1;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  const [vx, vy] = [-uy, ux];
  const half = s.size * 0.5 * 1.08;
  const th = FT * 2.75;
  // `p` out from the gatehouse's outer arch (ft), `q` across it (-1 .. 1).
  const out = FT * GATE_DEPTH;
  const P = (p: number, q: number): [number, number] => [ax + ux * (out + p) + vx * half * q, ay + uy * (out + p) + vy * half * q];
  const slab = [...P(0, -1), ...P(th, -1), ...P(th, 1), ...P(0, 1)];
  const drop = FT * (wet ? 5 : 7);
  g.poly(hull(pairs([...slab, ...slab.map((v) => v + drop)]))).fill({ color: 0x000000, alpha: wet ? 0.26 : 0.32 });
  // The top, then the outer face's strip in the light's tone.
  g.poly(slab).fill(WOOD[1]);
  const face = lit(ux, uy);
  g.poly([...P(th * 0.62, -1), ...P(th, -1), ...P(th, 1), ...P(th * 0.62, 1)]).fill(face > 0.3 ? WOOD[2] : face < -0.3 ? WOOD[0] : WOOD[1]);
  // The planks' ends along the top.
  const n = Math.max(4, Math.round((half * 2) / (FT * 1.1)));
  const rnd = rngAt(ax + bx, ay + by);
  for (let k = 1; k < n; k++) {
    const q = -1 + (2 * k) / n + (rnd() - 0.5) * 0.04;
    g.moveTo(...P(th * 0.08, q)).lineTo(...P(th * 0.62, q));
  }
  g.stroke({ width: 1.2, color: WOOD[0] });
  // The iron-shod lifting edge along the top, nailed.
  g.poly([...P(th * 0.24, -0.97), ...P(th * 0.46, -0.97), ...P(th * 0.46, 0.97), ...P(th * 0.24, 0.97)]).fill(IRON[0]);
  for (let k = 0; k < n; k++) g.circle(...P(th * 0.35, -1 + (2 * k + 1) / n), 1.1).fill(IRON[2]);
  g.poly(slab).stroke({ width: 2.6, color: INK, join: 'round' });
  // A chain ring at each end, back toward the gatehouse.
  for (const q of [-1, 1]) {
    const [rx, ry] = P(-FT * 0.15, q * 0.9);
    g.circle(rx, ry, FT * 0.38).stroke({ width: 3, color: INK }).circle(rx, ry, FT * 0.38).stroke({ width: 1.4, color: IRON[1] });
  }
}

/** A dry ditch's sides (ft down from a bank's top to the floor) and its outer bank's lip (ft wide). */
const DITCH_SIDE = 3.2;
const DITCH_LIP = 3;
/** Earth on a ditch's side turned from the light, toward it, and on its outer bank's lip. */
const CUT_DARK = 0x5a4430;
const CUT_LIT = 0xb09468;
const CUT_LIP = 0xd9c79c;
/**
 * A dry ditch's bank along its top (`ShapeKind::DitchBank`: the two ends of a stretch, the way
 * down into the ditch, whether its outer bank lies behind): its side a band of earth down to the
 * floor, dark where it is turned from the light (NW) and light toward it, the outer bank's lip
 * pale behind it (`ink` false); the top inked by hand, a little wavering (`ink`: over every
 * stretch's earth).
 */
export function drawDitchBank(g: Graphics, s: Shape, ink: boolean) {
  const [ax, ay, bx, by] = s.pts;
  const L = Math.hypot(bx - ax, by - ay);
  if (!(L > 0)) return;
  const rnd = rngAt(ax + bx, ay + by);
  const [nx, ny] = [-(by - ay) / L, (bx - ax) / L];
  const dl = Math.hypot(s.pts[4] ?? 0, s.pts[5] ?? 0);
  if (!ink) {
    if (!(dl > 0)) return;
    const [dx, dy] = [s.pts[4] / dl, s.pts[5] / dl];
    // (Overlapping a little at the ends: a stretch's band meets the next's without a seam.)
    const [ux, uy] = [((bx - ax) / L) * FT * 0.4, ((by - ay) / L) * FT * 0.4];
    const band = (from: number, to: number, color: number, alpha: number) => {
      const [f, t] = [from * FT, to * FT];
      g.poly([ax - ux + dx * f, ay - uy + dy * f, bx + ux + dx * f, by + uy + dy * f, bx + ux + dx * t, by + uy + dy * t, ax - ux + dx * t, ay - uy + dy * t]).fill({ color, alpha });
    };
    const lit = 0.5 + 0.5 * (dx * LX + dy * LY);
    const side = mix(CUT_DARK, CUT_LIT, lit);
    band(0, DITCH_SIDE * 0.7, side, 1);
    band(DITCH_SIDE * 0.7, DITCH_SIDE, side, 0.5);
    // (Its foot: a dark line where the side meets the floor, under the lit side only faint.)
    band(DITCH_SIDE * 0.62, DITCH_SIDE * 0.7, CUT_DARK, 0.5 - 0.35 * lit);
    if ((s.pts[6] ?? 0) > 0) band(-DITCH_LIP, 0, CUT_LIP, 0.45);
    return;
  }
  const n = Math.max(1, Math.round(L / (FT * 3)));
  g.moveTo(ax, ay);
  for (let k = 1; k < n; k++) {
    const t = k / n;
    const w = (rnd() - 0.5) * FT * 0.3;
    g.lineTo(ax + (bx - ax) * t + nx * w, ay + (by - ay) * t + ny * w);
  }
  g.lineTo(bx, by).stroke({ width: OUT, color: INK, cap: 'round', join: 'round' });
}

/** A gravel walk on a court's lawn (`ShapeKind::GravelWalk`: its rectangle's corners), drawn as a
 * building's court draws it (`gravelPatch`), along its longer side. */
export function drawGravelWalk(g: Graphics, s: Shape) {
  const q = pairs(s.pts.slice(0, 8));
  if (q.length < 4) return;
  const [e1, e3] = [Math.hypot(q[1][0] - q[0][0], q[1][1] - q[0][1]), Math.hypot(q[3][0] - q[0][0], q[3][1] - q[0][1])];
  if (!(e1 > 0 && e3 > 0)) return;
  const [o, a, b] = e1 >= e3 ? [q[0], q[1], q[3]] : [q[0], q[3], q[1]];
  const [la, lb] = [Math.max(e1, e3), Math.min(e1, e3)];
  const [ux, uy, vx, vy] = [(a[0] - o[0]) / la, (a[1] - o[1]) / la, (b[0] - o[0]) / lb, (b[1] - o[1]) / lb];
  const at = (u: number, t: number): [number, number] => [o[0] + (ux * u + vx * t) * PX, o[1] + (uy * u + vy * t) * PX];
  gravelPatch(g, at, la / PX, lb / PX, PX, rngAt((q[0][0] + q[2][0]) / 2, (q[0][1] + q[2][1]) / 2), [LX, LY]);
}

/** A banner streaming east on the wind from a pole at (x, y). */
export function banner(g: Graphics, x: number, y: number) {
  g.moveTo(x, y).lineTo(x + PX * 0.2, y - PX * 1.8).stroke({ width: 2.5, color: 0x3b2f25 });
  g.poly([x + PX * 0.2, y - PX * 1.8, x + PX * 1.7, y - PX * 1.5, x + PX * 1.4, y - PX * 1.25, x + PX * 1.75, y - PX * 0.95, x + PX * 0.15, y - PX * 1.05])
    .fill(0x9b2226)
    .stroke({ width: 1.5, color: INK });
}

/** The 5-ft grid over a wall's walk or a tower's top (shown with the ground's): lines on the square
 * boundaries, clipped to the walk (a quad) or the top (a round); over a court's gravel walk too, as
 * faint as the ground's own; over a bastion's earth top. None over a forecourt's low wall, piers or gate. */
export function topGrid(g: Graphics, s: Shape) {
  if (s.kind === 20) {
    // A bastion's top, as two halves either side of its middle line (each convex): f1, g1, tip, v and v, tip, g2, f2.
    const p = s.pts;
    gridIn(g, [p[0], p[1], p[2], p[3], p[4], p[5], p[10], p[11]]);
    gridIn(g, [p[10], p[11], p[4], p[5], p[6], p[7], p[8], p[9]]);
    g.stroke({ width: 1, color: INK, alpha: 0.32, pixelLine: true });
    return;
  }
  if (s.kind === 13) {
    if (s.pts.length < 8) return;
    gridIn(g, s.pts.slice(0, 8));
    g.stroke({ width: 1, color: INK, alpha: 0.28, pixelLine: true });
    return;
  }
  if (s.kind === 9) {
    // A one-block gatehouse's top, inside its parapet.
    const form = gateForm(s);
    if (form !== GATE_SQUARE && form !== GATE_ARCHED && form !== GATE_BENT) return;
    gridIn(g, inset(blockFrame(s, form).solid, PARAPET));
    g.stroke({ width: 1, color: INK, alpha: 0.45, pixelLine: true });
    return;
  }
  // (A square or arched gate tower's half: its gatehouse's top has the grid.)
  if (s.kind === 1 && flagsOf(s, 2) & GATE && (gateForm(s) === GATE_SQUARE || gateForm(s) === GATE_ARCHED)) return;
  const shape = s.kind === 1 && !(flagsOf(s, 2) & SQUARE) ? towerShape(s) : -1;
  if (shape === SHAPE_D || shape === SHAPE_OCTAGON) {
    const t = s.pts.length > 3 ? s.pts[3] / PX : 0;
    gridIn(g, inset(shapedOutline(s.pts[0], s.pts[1], s.size, Math.cos(t), Math.sin(t), shape), PARAPET));
  } else if (s.kind === 1 && (flagsOf(s, 2) & SQUARE || towerShape(s) === SHAPE_SQUARE)) {
    // A square tower's top: its square's lines, clipped to it.
    const { x, y, h, ux, uy } = squareFrame(s);
    const f = h - PARAPET;
    const q = [-1, -1, 1, -1, 1, 1, -1, 1].map((v) => v * f);
    const pts: number[] = [];
    for (let k = 0; k < 4; k++) pts.push(x + ux * q[k * 2] - uy * q[k * 2 + 1], y + uy * q[k * 2] + ux * q[k * 2 + 1]);
    gridIn(g, pts);
  } else if (s.kind === 1) {
    const [cx, cy] = [s.pts[0], s.pts[1]];
    const r = s.size - PARAPET;
    for (const axis of [0, 1]) {
      const c = axis === 0 ? cx : cy;
      for (let k = Math.ceil((c - r) / PX); k * PX <= c + r; k++) {
        const h = Math.sqrt(Math.max(0, r * r - (k * PX - c) ** 2));
        if (h <= 0) continue;
        if (axis === 0) g.moveTo(k * PX, cy - h).lineTo(k * PX, cy + h);
        else g.moveTo(cx - h, k * PX).lineTo(cx + h, k * PX);
      }
    }
  } else if (s.kind === 0) {
    const [x0, y0, x1, y1] = s.pts;
    const len = Math.hypot(x1 - x0, y1 - y0) || 1;
    // (No grid on a palisade: no walk to stand on; rammed earth's walk inside its battered faces.)
    const mat = lookOf(s, 6).material;
    if (mat === MAT_PALISADE) return;
    const w = s.size / 2 - PARAPET - (mat === MAT_EARTH ? FT * BATTER : 0);
    const [vx, vy] = [(-(y1 - y0) / len) * w, ((x1 - x0) / len) * w];
    gridIn(g, [x0 - vx, y0 - vy, x1 - vx, y1 - vy, x1 + vx, y1 + vy, x0 + vx, y0 + vy]);
  } else return;
  g.stroke({ width: 1, color: INK, alpha: 0.45, pixelLine: true });
}

/** The 5-ft grid's lines where they cross a convex outline. */
function gridIn(g: Graphics, q: number[]) {
  const n = q.length / 2;
  // Where the line `axis` = `at` crosses the walk's outline, as the span along the other axis.
  for (const axis of [0, 1]) {
    const vals = Array.from({ length: n }, (_, i) => q[i * 2 + axis]);
    for (let k = Math.ceil(Math.min(...vals) / PX); k * PX <= Math.max(...vals); k++) {
      let [lo, hi] = [Infinity, -Infinity];
      for (let i = 0; i < n; i++) {
        const [a, b] = [i * 2, ((i + 1) % n) * 2];
        const [p, r] = [q[a + axis], q[b + axis]];
        if ((p - k * PX) * (r - k * PX) > 0 || p === r) continue;
        const o = q[a + 1 - axis] + ((k * PX - p) / (r - p)) * (q[b + 1 - axis] - q[a + 1 - axis]);
        [lo, hi] = [Math.min(lo, o), Math.max(hi, o)];
      }
      if (!(hi > lo)) continue;
      if (axis === 0) g.moveTo(k * PX, lo).lineTo(k * PX, hi);
      else g.moveTo(lo, k * PX).lineTo(hi, k * PX);
    }
  }
}
