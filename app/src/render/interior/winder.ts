// Roadmap 7 P8: a round tower's winding stair up the inside of its wall (`interior/winder.rs`). Its
// squares are `winder` items in climbing order (clockwise on screen) and, over the run below, the
// floor's `stair_well`. Drawn as a curved flight hugging the wall (item 194): a band of the round
// wall's width in from it, wedge treads evenly spaced by angle, darker at the foot and paler up, a
// rail along the open inner side and a curved arrow up the flight; it covers each of the run's squares
// (a token stands on them). The square where the stair reaches the floor above (the level link, the
// same square on both floors) is a flat landing; on the floor above it has an arrow down. The well over
// the run below is the same band dark, the run's top treads dim in it, railed where it opens onto the floor.
import type { Graphics } from 'pixi.js';
import type { Interior, InteriorLevel } from '../../gen/protocol';
import { INK } from './look';
import { LINE, OUT } from '../pen';
import { PX } from '../scale';

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

const TREAD = 0xc9b79a;
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


type Circle = { cx: number; cy: number; r: number };
/** The flight's width in from the wall (grid units: a square is 5 ft) and its ink widths in grid units. */
const WIDE = 0.92;
const [OUT_SQ, LINE_SQ] = [OUT / PX, LINE / PX];

/** The circle (middle, wall's inner face) each square of the interior's winding stairs winds round: the
 * floor's frame where the squares lie in its band, else one fitted to that tower's squares (a round
 * tower inside a bigger building). */
function towerCircles(it: Interior): Map<string, Circle> {
  const sq = new Map<string, Sq>();
  for (const lv of it.levels) for (const f of lv.furniture) if (f.kind === 'winder' || f.kind === 'stair_well') sq.set(`${f.x},${f.y}`, [f.x, f.y]);
  const out = new Map<string, Circle>();
  const frame = winderFrame(it);
  const seen = new Set<string>();
  for (const [k0, q0] of sq) {
    if (seen.has(k0)) continue;
    // (One tower's squares: those beside each other, on any level.)
    const group: Sq[] = [];
    const todo = [q0];
    seen.add(k0);
    while (todo.length) {
      const q = todo.pop()!;
      group.push(q);
      for (let dj = -1; dj <= 1; dj++)
        for (let di = -1; di <= 1; di++) {
          const k = `${q[0] + di},${q[1] + dj}`;
          const n = sq.get(k);
          if (n && !seen.has(k)) {
            seen.add(k);
            todo.push(n);
          }
        }
    }
    const pts = group.map((q) => [q[0] + 0.5, q[1] + 0.5]);
    let c: Circle | null = null;
    if (frame && pts.every(([x, y]) => Math.hypot(x - frame.cx, y - frame.cy) >= frame.r - 1 - 1e-6 && Math.hypot(x - frame.cx, y - frame.cy) <= frame.r + 0.75)) c = frame;
    else if (pts.length >= 3) c = fitCircle(pts);
    c ??= frame;
    if (c) for (const q of group) out.set(key(q), c);
  }
  return out;
}

/** The round box a tower's stair squares lie in: a circle through their middles (least squares, Kasa),
 * its middle snapped to the half square (a box's), its radius out to the wall (the band is the squares
 * within a square of it, `winder::ring_of`). */
function fitCircle(pts: number[][]): Circle | null {
  const [mx, my] = [pts.reduce((a, p) => a + p[0], 0) / pts.length, pts.reduce((a, p) => a + p[1], 0) / pts.length];
  let [suu, svv, suv, suuu, svvv, suvv, svuu] = [0, 0, 0, 0, 0, 0, 0];
  for (const [x, y] of pts) {
    const [u, v] = [x - mx, y - my];
    [suu, svv, suv] = [suu + u * u, svv + v * v, suv + u * v];
    [suuu, svvv, suvv, svuu] = [suuu + u * u * u, svvv + v * v * v, suvv + u * v * v, svuu + v * u * u];
  }
  const det = suu * svv - suv * suv;
  if (Math.abs(det) < 1e-9) return null;
  const [bu, bv] = [0.5 * (suuu + suvv), 0.5 * (svvv + svuu)];
  const [cx, cy] = [Math.round(((bu * svv - bv * suv) / det + mx) * 2) / 2, Math.round(((bv * suu - bu * suv) / det + my) * 2) / 2];
  const far = Math.max(...pts.map((p) => Math.hypot(p[0] - cx, p[1] - cy)));
  return { cx, cy, r: Math.ceil(far * 2 - 1e-6) / 2 };
}

/** A run's flight round circle `c`: its squares' angles (climbing, unwrapped), the whole band `lo..hi`,
 * the treads' `cuts` (from the foot's edge to the top's), short of a landing at either end. */
type Flight = { th: number[]; lo: number; hi: number; cuts: number[] };
function flight(run: Sq[], c: Circle, down: boolean, up: boolean): Flight {
  const th: number[] = [];
  for (const [i, j] of run) {
    const a = Math.atan2(j + 0.5 - c.cy, i + 0.5 - c.cx);
    const p = th[th.length - 1];
    th.push(p === undefined ? a : p + Math.atan2(Math.sin(a - p), Math.cos(a - p)));
  }
  const d = half(c);
  const n = run.length;
  const [lo, hi] = [th[0] - d, th[n - 1] + d];
  if (n < 2) return { th, lo, hi, cuts: [] };
  const [t0, t1] = [down ? th[0] + d : lo, up ? th[n - 1] - d : hi];
  if (t1 - t0 < 1e-3) return { th, lo, hi, cuts: [] };
  // (About eleven treads a quarter turn.)
  const m = Math.max(2, Math.round(((t1 - t0) / (Math.PI / 2)) * 11));
  return { th, lo, hi, cuts: Array.from({ length: m + 1 }, (_, k) => t0 + ((t1 - t0) * k) / m) };
}

/** Half a square's angle round the flight's middle line. */
const half = (c: Circle) => 0.5 / Math.max(1, c.r - WIDE / 2);
const at = (c: Circle, r: number, a: number): Sq => [c.cx + r * Math.cos(a), c.cy + r * Math.sin(a)];

/** The band between radii r0 < r1 from angle a0 to a1, as a polygon. */
function sector(c: Circle, r0: number, r1: number, a0: number, a1: number): number[] {
  const n = Math.max(2, Math.ceil(Math.abs(a1 - a0) / 0.05));
  const pts: number[] = [];
  for (let k = 0; k <= n; k++) pts.push(...at(c, r1, a0 + ((a1 - a0) * k) / n));
  for (let k = n; k >= 0; k--) pts.push(...at(c, r0, a0 + ((a1 - a0) * k) / n));
  return pts;
}

/** An arc at radius r from a0 to a1 traced into the path. */
function arc(g: Graphics, c: Circle, r: number, a0: number, a1: number) {
  const n = Math.max(2, Math.ceil(Math.abs(a1 - a0) / 0.05));
  const p = at(c, r, a0);
  g.moveTo(p[0], p[1]);
  for (let k = 1; k <= n; k++) {
    const q = at(c, r, a0 + ((a1 - a0) * k) / n);
    g.lineTo(q[0], q[1]);
  }
}

/** A curved arrow along radius r from angle a0 to a1 (its head there): cream over ink. */
function arcArrow(g: Graphics, c: Circle, r: number, a0: number, a1: number) {
  const s = Math.sign(a1 - a0) || 1;
  const hl = 0.24 / r;
  const tip = at(c, r, a1);
  const [tx, ty] = [-Math.sin(a1) * s, Math.cos(a1) * s];
  const [bx, by] = [tip[0] - tx * 0.26, tip[1] - ty * 0.26];
  const head = [tip[0], tip[1], bx - ty * 0.17, by + tx * 0.17, bx + ty * 0.17, by - tx * 0.17];
  arc(g, c, r, a0, a1 - s * hl);
  g.stroke({ width: 0.15, color: INK, cap: 'round', join: 'round' });
  g.poly(head).stroke({ width: 0.07, color: INK, join: 'round' });
  arc(g, c, r, a0, a1 - s * hl);
  g.stroke({ width: 0.075, color: MARK, cap: 'round', join: 'round' });
  g.poly(head).fill(MARK);
}

/** Flight `f`'s treads between angles a and b (all if not given), the k-th of m lit `lit(k / m)`. */
function treads(g: Graphics, c: Circle, f: Flight, lit: (t: number) => number, ink: number, a = -Infinity, b = Infinity) {
  const [r0, r1] = [c.r - WIDE, c.r];
  const m = f.cuts.length - 1;
  const rm = c.r - WIDE / 2;
  for (let k = 0; k < m; k++) {
    const [u, v] = [Math.max(a, f.cuts[k]), Math.min(b, f.cuts[k + 1])];
    if (v - u <= 1e-6) continue;
    g.poly(sector(c, r0, r1, u, v)).fill(shade(TREAD, lit((k + 0.5) / m)));
  }
  // (The risers: ink across the band at each tread's foot, the nosing of the tread above lit just past it.)
  for (let k = 1; k < m; k++) {
    const t = f.cuts[k];
    if (t <= a || t >= b) continue;
    const [p, q] = [at(c, r0, t), at(c, r1, t)];
    g.moveTo(p[0], p[1]).lineTo(q[0], q[1]);
  }
  g.stroke({ width: LINE_SQ, color: INK, alpha: ink });
  for (let k = 1; k < m; k++) {
    const t = f.cuts[k] + 0.045 / rm;
    if (t <= a || t >= b) continue;
    const [p, q] = [at(c, r0 + 0.03, t), at(c, r1, t)];
    g.moveTo(p[0], p[1]).lineTo(q[0], q[1]);
  }
  g.stroke({ width: 0.016, color: NOSE, alpha: 0.55 * ink });
}

/** A rail along the inner edge from a0 to a1, a post at each end. */
function rail(g: Graphics, c: Circle, a0: number, a1: number) {
  if (a1 - a0 <= 1e-3) return;
  const r = c.r - WIDE + 0.045;
  arc(g, c, r, a0, a1);
  g.stroke({ width: 0.11, color: INK, cap: 'round', join: 'round' });
  arc(g, c, r, a0, a1);
  g.stroke({ width: 0.055, color: RAIL, cap: 'round', join: 'round' });
  for (const a of [a0, a1]) {
    const p = at(c, r, a);
    g.circle(p[0], p[1], 0.065).fill(RAIL).stroke({ width: LINE_SQ, color: INK });
  }
}

/** Level `li`'s winding stairs, if it has any: each flight up, its landings, and the well over the run below. */
export function drawWinders(g: Graphics, it: Interior, li: number) {
  const lv: InteriorLevel | undefined = it.levels[li];
  if (!lv) return;
  const wells: Sq[] = lv.furniture.filter((f) => f.kind === 'stair_well').map((f) => [f.x, f.y]);
  const runs = winderRuns(lv);
  if (!runs.length && !wells.length) return;
  const circles = towerCircles(it);
  const { up, down } = winderLinks(it, li);

  // The well: the band dark over the upper part of the run below, its top treads dim down it.
  if (wells.length) {
    const left = new Set(wells.map(key));
    const links = winderLinks(it, li - 1);
    for (const run of winderRuns(it.levels[li - 1])) {
      const ks = run.map((q, k) => (left.has(key(q)) ? k : -1)).filter((k) => k >= 0);
      const c = circles.get(key(run[0]));
      if (!ks.length || !c) continue;
      for (const k of ks) left.delete(key(run[k]));
      const n = run.length;
      const isUp = n >= 2 && links.up.has(key(run[n - 1]));
      const f = flight(run, c, links.down.has(key(run[0])), isUp);
      const d = half(c);
      const a = Math.min(...ks.map((k) => f.th[k])) - d;
      const b = isUp && f.cuts.length ? f.cuts[f.cuts.length - 1] : Math.max(...ks.map((k) => f.th[k])) + d;
      g.poly(sector(c, c.r - WIDE, c.r, a, b)).fill(WELL);
      treads(g, c, f, (t) => 0.22 + 0.4 * t, 0.55, a, b);
      g.poly(sector(c, c.r - WIDE, c.r, a, b)).fill({ color: WELL, alpha: 0.3 }).stroke({ width: OUT_SQ, color: INK, join: 'round' });
      // (Railed along the open inner side and across its low end; open toward the landing.)
      rail(g, c, a, b);
      const [p, q] = [at(c, c.r - WIDE + 0.045, a), at(c, c.r, a)];
      g.moveTo(p[0], p[1]).lineTo(q[0], q[1]).stroke({ width: 0.11, color: INK, cap: 'round' });
      g.moveTo(p[0], p[1]).lineTo(q[0], q[1]).stroke({ width: 0.055, color: RAIL, cap: 'round' });
    }
    // (Any well square not over a run below: a dark square.)
    for (const s of left) {
      const [i, j] = s.split(',').map(Number);
      g.rect(i, j, 1, 1).fill(WELL).stroke({ width: OUT_SQ, color: INK, alpha: 0.7 });
    }
  }

  for (const run of runs) {
    const c = circles.get(key(run[0]));
    if (!c) continue;
    const n = run.length;
    const isDown = down.has(key(run[0]));
    const isUp = n >= 2 && up.has(key(run[n - 1]));
    const f = flight(run, c, isDown, isUp);
    const d = half(c);
    const rm = c.r - WIDE / 2;
    // (The landings flat: where the run comes up from below, and where it goes on to the floor above.)
    const pads: [number, number][] = [];
    if (isDown || n < 2) pads.push([f.th[0] - d, f.th[0] + d]);
    if (isUp) pads.push([f.th[n - 1] - d, f.th[n - 1] + d]);
    for (const [a, b] of pads) g.poly(sector(c, c.r - WIDE, c.r, a, b)).fill(PAD).stroke({ width: LINE_SQ, color: INK, alpha: 0.7, join: 'round' });
    if (f.cuts.length) {
      const [t0, t1] = [f.cuts[0], f.cuts[f.cuts.length - 1]];
      treads(g, c, f, (t) => 0.46 + 0.6 * t, 0.9);
      // (The flight's edge: ink round the band, the wall side under the wall.)
      g.poly(sector(c, c.r - WIDE, c.r, t0, t1)).stroke({ width: OUT_SQ, color: INK, join: 'round' });
      // (The rail on the open side, from a tread up from the foot where the run starts on this floor.)
      rail(g, c, isDown ? t0 : t0 + Math.min(0.5 / rm, (t1 - t0) * 0.25), t1);
      // A curved arrow up the flight's middle.
      const span = Math.min((t1 - t0) * 0.6, 2.4 / rm);
      const mid = (t0 + t1) / 2;
      arcArrow(g, c, rm + 0.04, mid - span / 2, mid + span / 2);
    }
    // (Where it comes up from the floor below: an arrow down on the landing, back the way it came.)
    if (isDown) arcArrow(g, c, rm, f.th[0] + d * 0.7, f.th[0] - d * 0.7);
  }
}
