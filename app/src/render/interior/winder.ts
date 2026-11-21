// Roadmap 7 P8: a round tower's winding stair up the inside of its wall (`interior/winder.rs`). Its
// squares are `winder` items in climbing order (clockwise on screen) and, over the run below, the
// floor's `stair_well`. Drawn on the 5-ft grid so a token's square reads at a glance: two treads to a
// square, split across the way the stair climbs there, paler the higher; a chevron up each square; the
// grid over them; a rail on the run's open side. The square where the stair reaches the floor above
// (the level link, the same square on both floors) is a flat landing with an arrow: up on the floor
// below, down on the floor above. The well over the run below is a dark shaft, the treads faint in
// it, railed where it opens onto the floor.
import type { Graphics } from 'pixi.js';
import type { Interior, InteriorLevel } from '../../gen/protocol';
import { INK } from './look';

/** The floor's middle and half width (grid units): the box round the ground floor's squares, as the
 * generator finds it (`winder::frame`). */
export function winderFrame(it: Interior): { cx: number; cy: number; r: number } | null {
  const lv = it.levels[it.entry_level] ?? it.levels[0];
  if (!lv) return null;
  let [i0, i1, j0, j1] = [Infinity, -Infinity, Infinity, -Infinity];
  for (let k = 0; k < lv.cells.length; k++) {
    if (lv.cells[k] < 0) continue;
    const [i, j] = [k % it.nx, Math.floor(k / it.nx)];
    [i0, i1, j0, j1] = [Math.min(i0, i), Math.max(i1, i), Math.min(j0, j), Math.max(j1, j)];
  }
  if (i0 > i1) return null;
  return { cx: (i0 + i1 + 1) / 2, cy: (j0 + j1 + 1) / 2, r: (i1 - i0 + 1 + (j1 - j0 + 1)) / 4 };
}

const TREAD = [0xc9b79a, 0xb7a487];
const NOSE = 0xf4e8cc;
const RAIL = 0x6b4a2c;
const WELL = 0x221c17;
const PAD = 0xd9ccb0;
const MARK = 0xf2e6c8;

const shade = (c: number, k: number) => {
  const f = (s: number) => Math.max(0, Math.min(255, Math.round(((c >> s) & 255) * k)));
  return (f(16) << 16) | (f(8) << 8) | f(0);
};

type Sq = [number, number];
const key = (q: Sq) => `${q[0]},${q[1]}`;
const near = (a: Sq, b: Sq) => Math.abs(a[0] - b[0]) <= 1 && Math.abs(a[1] - b[1]) <= 1;

/** A level's winding stair runs: its `winder` squares as laid, in climbing order, one run per
 * stretch of squares each beside the last (a keep's towers each have their own). */
export function winderRuns(lv: InteriorLevel | undefined): Sq[][] {
  const runs: Sq[][] = [];
  for (const f of lv?.furniture ?? []) {
    if (f.kind !== 'winder') continue;
    const q: Sq = [f.x, f.y];
    const last = runs[runs.length - 1];
    if (last && near(last[last.length - 1], q)) last.push(q);
    else runs.push([q]);
  }
  return runs;
}

/** Where level `li`'s winding stairs change level: `up` the top of each run whose square the floor
 * above's stair goes on from, `down` the foot of each run that comes up from the floor below (keys
 * `x,y`). A token there goes up or down; the square is the same on both floors. */
export function winderLinks(it: Interior, li: number): { up: Set<string>; down: Set<string> } {
  const on = (l: number) => new Set((it.levels[l]?.furniture ?? []).filter((f) => f.kind === 'winder').map((f) => `${f.x},${f.y}`));
  const [above, below] = [on(li + 1), on(li - 1)];
  const up = new Set<string>();
  const down = new Set<string>();
  for (const run of winderRuns(it.levels[li])) {
    const [first, top] = [run[0], run[run.length - 1]];
    if (run.length >= 2 && above.has(key(top))) up.add(key(top));
    if (below.has(key(first))) down.add(key(first));
  }
  return { up, down };
}

/** The way a run climbs through its `k`th square, snapped to the grid's eight directions. */
function dirAt(run: Sq[], k: number): Sq {
  const a = run[Math.max(0, k - 1)];
  const b = run[Math.min(run.length - 1, k + 1)];
  let [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  if (!dx && !dy) return [1, 0];
  const t = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4);
  [dx, dy] = [Math.round(Math.cos(t)), Math.round(Math.sin(t))];
  return [dx, dy];
}

/** Square (i, j) cut by the line through its middle across `d`: the lower half (toward -d) and the upper. */
function halves(i: number, j: number, d: Sq): [number[], number[]] {
  const c: Sq = [i + 0.5, j + 0.5];
  const corners: Sq[] = [[i, j], [i + 1, j], [i + 1, j + 1], [i, j + 1]];
  const side = (p: Sq) => (p[0] - c[0]) * d[0] + (p[1] - c[1]) * d[1];
  const cut = (sign: number) => {
    const out: number[] = [];
    for (let k = 0; k < 4; k++) {
      const [p, q] = [corners[k], corners[(k + 1) % 4]];
      const [sp, sq] = [side(p) * sign, side(q) * sign];
      if (sp <= 1e-9) out.push(p[0], p[1]);
      if ((sp < -1e-9 && sq > 1e-9) || (sp > 1e-9 && sq < -1e-9)) {
        const t = sp / (sp - sq);
        out.push(p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t);
      }
    }
    return out;
  };
  return [cut(1), cut(-1)];
}

/** The riser across square (i, j) (the line between its two treads), as two points. */
function riser(i: number, j: number, d: Sq): [Sq, Sq] {
  const l = Math.hypot(d[0], d[1]);
  const [ux, uy] = [-d[1] / l, d[0] / l];
  // (Half the square's width along the riser: to its edges, or its corners on a diagonal.)
  const h = d[0] && d[1] ? Math.SQRT1_2 : 0.5;
  return [[i + 0.5 - ux * h, j + 0.5 - uy * h], [i + 0.5 + ux * h, j + 0.5 + uy * h]];
}

/** Two treads on square (i, j) climbing along `d`, the lower lit `k0`, the upper `k1`; `ink` for the riser. */
function treadPair(g: Graphics, i: number, j: number, d: Sq, k0: number, k1: number, ink = 0.85) {
  const [lo, hi] = halves(i, j, d);
  g.poly(lo).fill(shade(TREAD[1], k0));
  g.poly(hi).fill(shade(TREAD[0], k1));
  const [a, b] = riser(i, j, d);
  g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: 0.06, color: INK, alpha: ink });
  // (The upper tread's nosing, lit just past the riser.)
  const l = Math.hypot(d[0], d[1]);
  const [ox, oy] = [(d[0] / l) * 0.06, (d[1] / l) * 0.06];
  g.moveTo(a[0] + ox, a[1] + oy).lineTo(b[0] + ox, b[1] + oy).stroke({ width: 0.04, color: shade(NOSE, 0.55 + 0.45 * k1), alpha: ink });
}

/** An arrow across square (i, j) along `d`: a shaft and a bold head, cream over an ink edge. */
function arrow(g: Graphics, i: number, j: number, d: Sq, len = 0.62) {
  const l = Math.hypot(d[0], d[1]);
  const [ux, uy] = [d[0] / l, d[1] / l];
  const [cx, cy] = [i + 0.5, j + 0.5];
  const [tx, ty] = [cx + ux * len * 0.5, cy + uy * len * 0.5];
  const [bx, by] = [cx - ux * len * 0.5, cy - uy * len * 0.5];
  const head = [tx, ty, tx - ux * 0.26 - uy * 0.2, ty - uy * 0.26 + ux * 0.2, tx - ux * 0.26 + uy * 0.2, ty - uy * 0.26 - ux * 0.2];
  g.moveTo(bx, by).lineTo(tx - ux * 0.2, ty - uy * 0.2).stroke({ width: 0.17, color: INK, cap: 'round' });
  g.poly(head).stroke({ width: 0.08, color: INK, join: 'round' });
  g.moveTo(bx, by).lineTo(tx - ux * 0.2, ty - uy * 0.2).stroke({ width: 0.09, color: MARK, cap: 'round' });
  g.poly(head).fill(MARK);
}

/** A chevron on square (i, j) pointing along `d`. */
function chevron(g: Graphics, i: number, j: number, d: Sq, size: number, width: number, color: number, alpha: number) {
  const l = Math.hypot(d[0], d[1]);
  const [ux, uy] = [d[0] / l, d[1] / l];
  const [cx, cy] = [i + 0.5 + ux * size * 0.3, j + 0.5 + uy * size * 0.3];
  g.moveTo(cx - ux * size - uy * size, cy - uy * size + ux * size)
    .lineTo(cx, cy)
    .lineTo(cx - ux * size + uy * size, cy - uy * size - ux * size)
    .stroke({ width, color, alpha, cap: 'round', join: 'round' });
}

/** A flat landing on square (i, j): paving inset in an ink frame. */
function landing(g: Graphics, i: number, j: number) {
  g.rect(i, j, 1, 1).fill(shade(PAD, 0.92));
  g.rect(i + 0.09, j + 0.09, 0.82, 0.82).fill(PAD).stroke({ width: 0.05, color: INK, alpha: 0.7 });
}

/** Rails along the sides of squares `sq` that open onto squares `open` says are floor beside them. */
function rails(g: Graphics, sq: Sq[], open: (i: number, j: number) => boolean, inset: number, width: number) {
  const segs: [number, number, number, number][] = [];
  for (const [i, j] of sq) {
    if (open(i, j - 1)) segs.push([i, j + inset, i + 1, j + inset]);
    if (open(i + 1, j)) segs.push([i + 1 - inset, j, i + 1 - inset, j + 1]);
    if (open(i, j + 1)) segs.push([i, j + 1 - inset, i + 1, j + 1 - inset]);
    if (open(i - 1, j)) segs.push([i + inset, j, i + inset, j + 1]);
  }
  if (!segs.length) return;
  for (const s of segs) g.moveTo(s[0], s[1]).lineTo(s[2], s[3]);
  g.stroke({ width: width + 0.05, color: INK, cap: 'round' });
  for (const s of segs) g.moveTo(s[0], s[1]).lineTo(s[2], s[3]);
  g.stroke({ width, color: RAIL, cap: 'round' });
  for (const s of segs) for (const [x, y] of [[s[0], s[1]], [s[2], s[3]]]) g.circle(x, y, width * 0.75).fill(RAIL).stroke({ width: 0.025, color: INK });
}

/** Level `li`'s winding stairs, if it has any: each run up, its landings, and the well over the run below. */
export function drawWinders(g: Graphics, it: Interior, li: number) {
  const lv: InteriorLevel | undefined = it.levels[li];
  if (!lv) return;
  const wells: Sq[] = lv.furniture.filter((f) => f.kind === 'stair_well').map((f) => [f.x, f.y]);
  const runs = winderRuns(lv);
  if (!runs.length && !wells.length) return;
  const { nx, ny } = it;
  const stair = new Set([...runs.flat(), ...wells].map(key));
  const floor = (i: number, j: number) => i >= 0 && j >= 0 && i < nx && j < ny && lv.cells[j * nx + i] >= 0 && !stair.has(`${i},${j}`);
  const { up, down } = winderLinks(it, li);

  // The well: the run below seen down the shaft, its treads dim, a rail where it opens onto the floor.
  if (wells.length) {
    const below = winderRuns(it.levels[li - 1]);
    const isWell = new Set(wells.map(key));
    for (const [i, j] of wells) {
      g.rect(i, j, 1, 1).fill(WELL);
      const run = below.find((r) => r.some((q) => q[0] === i && q[1] === j));
      if (run) {
        const k = run.findIndex((q) => q[0] === i && q[1] === j);
        const t = k / Math.max(1, run.length - 1);
        treadPair(g, i, j, dirAt(run, k), 0.26 + 0.12 * t, 0.3 + 0.12 * t, 0.5);
      }
      g.rect(i, j, 1, 1).fill({ color: WELL, alpha: 0.35 });
    }
    // (Its edge inked; the rail on the sides onto the floor; open toward the landing the run reaches.)
    for (const [i, j] of wells) g.rect(i, j, 1, 1);
    g.stroke({ width: 0.04, color: INK, alpha: 0.55 });
    const segs: number[][] = [];
    for (const [i, j] of wells) {
      for (const [di, dj, s] of [[0, -1, [i, j, i + 1, j]], [1, 0, [i + 1, j, i + 1, j + 1]], [0, 1, [i, j + 1, i + 1, j + 1]], [-1, 0, [i, j, i, j + 1]]] as [number, number, number[]][]) {
        if (!isWell.has(`${i + di},${j + dj}`)) segs.push(s);
      }
    }
    for (const s of segs) g.moveTo(s[0], s[1]).lineTo(s[2], s[3]);
    g.stroke({ width: 0.09, color: INK, cap: 'round' });
    rails(g, wells, floor, 0.1, 0.08);
  }

  for (const run of runs) {
    const n = run.length;
    // (Treads: two a square, the lowest darkest; the landings flat.)
    const steps = run.map((q, k) => !(k === 0 && down.has(key(q))) && !(k === n - 1 && up.has(key(q))) && !(n === 1));
    const total = steps.filter(Boolean).length * 2;
    let t = 0;
    for (let k = 0; k < n; k++) {
      const [i, j] = run[k];
      if (!steps[k]) {
        landing(g, i, j);
        continue;
      }
      const d = dirAt(run, k);
      const lit = (s: number) => 0.62 + (0.42 * (s + 0.5)) / Math.max(1, total);
      treadPair(g, i, j, d, lit(t), lit(t + 1));
      t += 2;
    }
    // The grid over every stair square, each a square a token stands on.
    for (const [i, j] of run) g.rect(i, j, 1, 1);
    g.stroke({ width: 0.045, color: INK, alpha: 0.5 });
    // A chevron up each tread square; the foot's bold, the way up.
    const first = steps.indexOf(true);
    for (let k = 0; k < n; k++) {
      if (!steps[k]) continue;
      const d = dirAt(run, k);
      // (The foot of a run that starts on this floor: an arrow up it. Where the run comes up from below, the
      // landing's arrow down marks it instead.)
      if (k === first && k === 0) arrow(g, run[k][0], run[k][1], d, 0.5);
      else chevron(g, run[k][0], run[k][1], d, 0.17, 0.075, INK, 0.6);
    }
    // The rail along the run's open side (not at its foot, where you step on).
    rails(g, run.filter((_, k) => steps[k] && k !== first), floor, 0.07, 0.07);
    // The landings: up on the top of the run, down where it comes up from the floor below.
    if (n >= 2 && up.has(key(run[n - 1]))) arrow(g, run[n - 1][0], run[n - 1][1], dirAt(run, n - 1));
    if (down.has(key(run[0]))) {
      // (Back the way the run below came up: from the well, or along this run's start.)
      const below = winderRuns(it.levels[li - 1]).find((r) => r.length >= 2 && key(r[r.length - 1]) === key(run[0]));
      const d = below ? dirAt(below, below.length - 1) : dirAt(run, 0);
      arrow(g, run[0][0], run[0][1], [-d[0], -d[1]]);
    }
  }
}
