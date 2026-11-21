// Roadmap 7 P8: a round tower's winding stair up the inside of its wall (`interior/winder.rs`). Its
// squares are `winder` items in climbing order (clockwise on screen) and, over the run below, the
// floor's `stair_well`; drawn here as one band of wedge treads round the floor's middle, under the
// walls: two flat tones, a lit nosing and an ink line on each, darker the lower it lies, a rail on
// the open inner edge; the well shows the treads below it darker still.
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
const WELL = 0x2a231d;

const shade = (c: number, k: number) => {
  const f = (s: number) => Math.max(0, Math.min(255, Math.round(((c >> s) & 255) * k)));
  return (f(16) << 16) | (f(8) << 8) | f(0);
};

/** The arc a level's squares of one kind cover: from the first's angle to the last's, a half step
 * beyond each (radians, rising clockwise on screen). */
function arcOf(sq: [number, number][], cx: number, cy: number): [number, number] | null {
  if (sq.length < 2) return null;
  const ang: number[] = [];
  for (const [i, j] of sq) {
    let a = Math.atan2(j + 0.5 - cy, i + 0.5 - cx);
    if (ang.length) while (a < ang[ang.length - 1] - 1e-6) a += Math.PI * 2;
    ang.push(a);
  }
  const half = (ang[ang.length - 1] - ang[0]) / (ang.length - 1) / 2;
  return [ang[0] - half, ang[ang.length - 1] + half];
}

/** One tread: the band from `r0` to `r1` between angles `a` and `b`. */
function wedge(g: Graphics, cx: number, cy: number, r0: number, r1: number, a: number, b: number) {
  const n = Math.max(2, Math.ceil(((b - a) * r1) / 0.15));
  const pts: number[] = [];
  for (let s = 0; s <= n; s++) {
    const t = a + ((b - a) * s) / n;
    pts.push(cx + Math.cos(t) * r1, cy + Math.sin(t) * r1);
  }
  for (let s = n; s >= 0; s--) {
    const t = a + ((b - a) * s) / n;
    pts.push(cx + Math.cos(t) * r0, cy + Math.sin(t) * r0);
  }
  g.poly(pts);
}

function arcLine(g: Graphics, cx: number, cy: number, r: number, a: number, b: number) {
  const n = Math.max(2, Math.ceil(((b - a) * r) / 0.15));
  for (let s = 0; s <= n; s++) {
    const t = a + ((b - a) * s) / n;
    if (s === 0) g.moveTo(cx + Math.cos(t) * r, cy + Math.sin(t) * r);
    else g.lineTo(cx + Math.cos(t) * r, cy + Math.sin(t) * r);
  }
}

/** Treads from angle `a0` to `a1`, `count` of them, lit from `lo` (the lowest) to `hi` (0 dark, 1 full). */
function treads(g: Graphics, cx: number, cy: number, r0: number, r1: number, a0: number, a1: number, count: number, lo: number, hi: number) {
  const step = (a1 - a0) / count;
  for (let t = 0; t < count; t++) {
    const [a, b] = [a0 + t * step, a0 + (t + 1) * step];
    const k = lo + ((hi - lo) * (t + 0.5)) / count;
    wedge(g, cx, cy, r0, r1, a, b);
    g.fill(shade(TREAD[t % 2], k));
    // (The step up at its high end: the riser's shadow in ink, the next tread's nosing lit just past it.)
    if (t + 1 < count) {
      const n = b + 0.07 / ((r0 + r1) / 2);
      g.moveTo(cx + Math.cos(b) * r0, cy + Math.sin(b) * r0).lineTo(cx + Math.cos(b) * r1, cy + Math.sin(b) * r1);
      g.stroke({ width: 0.06, color: INK, alpha: 0.85 });
      g.moveTo(cx + Math.cos(n) * r0, cy + Math.sin(n) * r0).lineTo(cx + Math.cos(n) * r1, cy + Math.sin(n) * r1);
      g.stroke({ width: 0.05, color: shade(NOSE, 0.55 + 0.45 * k) });
    }
  }
}

/** Level `li`'s winding stair, if it has one: its run up, and the well over the run below. */
export function drawWinders(g: Graphics, it: Interior, li: number) {
  const lv: InteriorLevel | undefined = it.levels[li];
  if (!lv || !lv.furniture.some((f) => f.kind === 'winder' || f.kind === 'stair_well')) return;
  const fr = winderFrame(it);
  if (!fr) return;
  const { cx, cy, r } = fr;
  // (The wall side tucked under the wall; the open side a square in from it.)
  const [r0, r1] = [Math.max(r * 0.4, r - 1.05), r + 0.12];
  const of = (kind: string) => lv.furniture.filter((f) => f.kind === kind).map((f) => [f.x, f.y] as [number, number]);
  const well = arcOf(of('stair_well'), cx, cy);
  if (well) {
    // The run below seen through the well: its upper treads, dark in the shaft, the floor's edge inked round it.
    const [a0, a1] = well;
    wedge(g, cx, cy, r0, r1, a0, a1);
    g.fill(WELL);
    const n = Math.max(3, Math.round(((a1 - a0) * (r0 + r1)) / 2 / 0.42));
    treads(g, cx, cy, r0 + 0.04, r1, a0, a1, n, 0.32, 0.5);
    wedge(g, cx, cy, r0, r1, a0, a1);
    g.stroke({ width: 0.07, color: INK, alpha: 0.9 });
    arcLine(g, cx, cy, r0 - 0.05, a0, a1);
    g.stroke({ width: 0.06, color: RAIL });
  }
  const run = arcOf(of('winder'), cx, cy);
  if (run) {
    const [a0, a1] = run;
    // (About sixteen treads to a storey's half turn.)
    const n = Math.max(4, Math.round(((a1 - a0) * (r0 + r1)) / 2 / 0.42));
    // (A soft shadow on the floor along its open edge, then the treads, lower darker.)
    arcLine(g, cx, cy, r0 - 0.06, a0, a1);
    g.stroke({ width: 0.14, color: INK, alpha: 0.18 });
    treads(g, cx, cy, r0, r1, a0, a1, n, 0.62, 1.04);
    // The foot's first riser, the open edge in ink and the rail over it, a post at each end.
    g.moveTo(cx + Math.cos(a0) * r0, cy + Math.sin(a0) * r0).lineTo(cx + Math.cos(a0) * r1, cy + Math.sin(a0) * r1);
    g.stroke({ width: 0.07, color: INK });
    arcLine(g, cx, cy, r0, a0, a1);
    g.stroke({ width: 0.06, color: INK });
    arcLine(g, cx, cy, r0 + 0.09, a0, a1);
    g.stroke({ width: 0.07, color: RAIL });
    for (const a of [a0 + 0.04 / r0, a1 - 0.04 / r0]) g.circle(cx + Math.cos(a) * (r0 + 0.09), cy + Math.sin(a) * (r0 + 0.09), 0.075).fill(RAIL).stroke({ width: 0.03, color: INK });
  }
}
