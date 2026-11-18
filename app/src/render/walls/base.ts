import { FT, LX, LY, mix, PLUM, INK, STONE } from '../bridges';
import type { Shape } from '../../gen/battlePrep';
import { PX } from '../scale';
import { OUT, LINE } from '../pen';
import type { Graphics } from 'pixi.js';

/** The walk and a tower's top: flags between the stone's mid and light tones. */
export const WALK = 0xb0a692;
/** Iron: dark, mid, light. */
export const IRON = [0x4a4e56, 0x6e7278, 0x9aa0a6];
/** A parapet's width (px): merlons stand on it. */
export const PARAPET = FT * 1.6;

/** Flags on a wall's or tower's last point (`battlemap::WALL_*`). */
export const BARE_A = 1;
export const BARE_B = 2;
export const GATE = 1;
export const RUINED = 4;
/** A square tower (a castle's): its sides run at the angle after the flags. */
export const SQUARE = 8;

export const flagsOf = (s: Shape, at: number) => (s.pts.length > at ? Math.round(s.pts[at] / PX) : 0);

/** Tower shapes in a tower's style code (`town::wallstyle::TowerShape`, bits 3-4 of `WallStyle::code`). */
export const SHAPE_SQUARE = 1;
export const SHAPE_D = 2;
export const SHAPE_OCTAGON = 3;
/** A D tower's flat back stands this far in from its curtain's line (ft; `wallstyle::D_BACK`). */
export const D_BACK = 4;
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
export const lit = (x: number, y: number) => (x * LX + y * LY) / (Math.hypot(x, y) || 1);

/** Stone blocks (outlines) with a drop shadow: the block's sides in a shaded tone, its top in
 * `tone` set toward the light, a bold ink outline. */
export function blocks(g: Graphics, quads: number[][], tone: () => number, shadow = FT * 0.35) {
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
export function rubble(x: number, y: number, n: number, spread: number, rnd: () => number, big = 1): number[][] {
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
export function breach(g: Graphics, x: number, y: number, r: number, ang: number, w: number, rnd: () => number, fallen: number[][]) {
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
export const notchTone = () => mix(ST[0], ST[1], 0.5);
/** Whether the stone being drawn is rose brick (it takes coursing lines and a crisper edge). */
export const brick = () => ST === TONES[3];

export const stoneTone = (rnd: () => number) => () => (rnd() < 0.35 ? ST[2] : ST[1]);

/** Outlines by size (assets/STYLE_GUIDE.md): a shape over about half a square across, and smaller parts. */
export { OUT, LINE };

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
export let ST = STONE;
export let WK = WALK;
let MER = M_SQUARE;
/** Takes a look's tones and merlons for what is drawn next; none, today's. */
export function use(l: Look | null) {
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
export const rhythm = () => RHYTHM[MER] ?? RHYTHM[M_SQUARE];
/** The marks a merlon's profile leaves on its top, flushed by `crenelFill`: a swallowtail's fork, a stepped
 * merlon's upper step, a rounded one's lit crown. */
const FORKS: number[][] = [];
const STEPS: number[][] = [];
const CROWNS: number[][] = [];

/** A point `a` along a straight run of parapet and `d` across it. */
export type At = (a: number, d: number) => [number, number];

/** A merlon's top from `m0` to `m1` along a run, `d0` its walk side to `d1` its face, marked by its profile. */
export function merlonMark(at: At, m0: number, m1: number, d0: number, d1: number) {
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
export function crenelRun(at: At, a0: number, a1: number, d0: number, d1: number, rnd: () => number, crenels: number[][], ruined = false, fallen: number[][] = []) {
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
export function crenelFill(g: Graphics, crenels: number[][]) {
  for (const q of crenels) g.poly(q);
  if (crenels.length) g.fill(notchTone()).stroke({ width: LINE * 0.8, color: INK, alpha: 0.9, join: 'round' });
  marksFill(g);
}

/** The merlons' marks pushed since last time (`merlonMark`). */
export function marksFill(g: Graphics) {
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
export interface Side {
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
export function sidesOf(pts: number[], open: number[] = []): Side[] {
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
export function clipLine(pts: number[], px: number, py: number, dx: number, dy: number): [number, number] {
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

/** Stake tops' radius on a palisade (px). */
export const STAKE = FT * 0.85;

/** A rammed earth wall's battered foot each side (ft). */
export const BATTER = 1.6;

/** A shape's points as pairs. */
export const pairs = (pts: number[]): [number, number][] => Array.from({ length: pts.length >> 1 }, (_, k) => [pts[k * 2], pts[k * 2 + 1]]);

/** The convex hull of points, as a flat list round it. */
export function hull(p: [number, number][]): number[] {
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

/** Sets the stone's tones (a stockade piece draws in timber); callers restore the old ones. */
export function setStone(t: number[]) {
  ST = t;
}
