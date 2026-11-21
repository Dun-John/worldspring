// Roadmap 7 P8 (v66b): a lighthouse's top level drawn to read at a glance. The lamp room in the middle
// is its glazed lantern: a ring of glass panes between thin iron mullions on a brass sill (in place of
// its wall), the lamp's light pooled on its floor and thrown out in rays across the gallery; the
// gallery round it a walkable ring of flags inside a low parapet with an iron railing on it.
import type { Graphics } from 'pixi.js';
import type { Interior, InteriorLevel } from '../../gen/protocol';
import type { DrawnDoor } from '../roomShapes';
import { INK } from './look';

export const LAMP_ROOM = 'lamp room';
export const LAMP_GALLERY = 'lamp gallery';
const GLASS = 0xcfe6ea;
const GLASS_LIT = 0xfff2c4;
const IRON = 0x34302b;
const BRASS = 0xc89a3c;
const LIGHT = 0xffd65a;

/** The lamp room's middle and radius (grid units): the box round its squares; null on another level. */
function lampRoom(it: Interior, lv: InteriorLevel): { ri: number; cx: number; cy: number; r: number } | null {
  const ri = lv.rooms.findIndex((r) => r.kind === LAMP_ROOM);
  if (ri < 0) return null;
  let [i0, i1, j0, j1] = [Infinity, -Infinity, Infinity, -Infinity];
  for (let k = 0; k < lv.cells.length; k++) {
    if (lv.cells[k] !== ri) continue;
    const [i, j] = [k % it.nx, Math.floor(k / it.nx)];
    [i0, i1, j0, j1] = [Math.min(i0, i), Math.max(i1, i), Math.min(j0, j), Math.max(j1, j)];
  }
  if (i0 > i1) return null;
  return { ri, cx: (i0 + i1 + 1) / 2, cy: (j0 + j1 + 1) / 2, r: Math.min(i1 - i0 + 1, j1 - j0 + 1) / 2 };
}

/** The tower's outer wall (grid units): the middle and mean radius of its drawn outline, else of the floor's box. */
function towerRing(it: Interior, lv: InteriorLevel): { cx: number; cy: number; r: number } | null {
  const o = it.outlines?.length === 1 ? it.outlines[0] : null;
  if (o && o.length >= 8) {
    const cx = o.reduce((s, p) => s + p[0], 0) / o.length;
    const cy = o.reduce((s, p) => s + p[1], 0) / o.length;
    return { cx, cy, r: o.reduce((s, p) => s + Math.hypot(p[0] - cx, p[1] - cy), 0) / o.length };
  }
  let [i0, i1, j0, j1] = [Infinity, -Infinity, Infinity, -Infinity];
  for (let k = 0; k < lv.cells.length; k++) {
    if (lv.cells[k] < 0) continue;
    const [i, j] = [k % it.nx, Math.floor(k / it.nx)];
    [i0, i1, j0, j1] = [Math.min(i0, i), Math.max(i1, i), Math.min(j0, j), Math.max(j1, j)];
  }
  return i0 > i1 ? null : { cx: (i0 + i1 + 1) / 2, cy: (j0 + j1 + 1) / 2, r: (i1 - i0 + 1 + (j1 - j0 + 1)) / 4 };
}

/** Whether a level is a lighthouse's top (its lamp room and gallery). */
export const isLampTop = (lv: InteriorLevel | undefined) => !!lv?.rooms.some((r) => r.kind === LAMP_ROOM);

/** On the floor, under the furniture: the lamp's light pooled in the lantern and its rays out over the gallery. */
export function drawLampFloor(g: Graphics, it: Interior, li: number) {
  const lv = it.levels[li];
  const room = lv && lampRoom(it, lv);
  if (!room) return;
  const { cx, cy, r } = room;
  const out = (towerRing(it, lv)?.r ?? r + 2) - 0.3;
  // (Eight rays, broad and soft, from the lamp to the parapet.)
  for (let k = 0; k < 8; k++) {
    const a = (k / 8) * Math.PI * 2 + Math.PI / 8;
    const w = 0.16;
    g.poly([cx + Math.cos(a - w) * 0.4, cy + Math.sin(a - w) * 0.4, cx + Math.cos(a - w * 0.55) * out, cy + Math.sin(a - w * 0.55) * out, cx + Math.cos(a + w * 0.55) * out, cy + Math.sin(a + w * 0.55) * out, cx + Math.cos(a + w) * 0.4, cy + Math.sin(a + w) * 0.4]).fill({ color: LIGHT, alpha: 0.22 });
  }
  for (let k = 0; k < 4; k++) g.circle(cx, cy, r * (0.95 - k * 0.18)).fill({ color: LIGHT, alpha: 0.14 });
}

/** Over the walls: the lantern's glazing on the lamp room's wall (its door left open), the railing on the gallery's parapet. */
export function drawLampGlass(g: Graphics, it: Interior, li: number, doors: DrawnDoor[] | null) {
  const lv = it.levels[li];
  const room = lv && lampRoom(it, lv);
  if (!room) return;
  const { ri, cx, cy, r } = room;
  // The gallery's parapet: a low stone wall, capped pale, an iron railing along it on posts.
  const ring = towerRing(it, lv);
  if (ring) {
    const R = ring.r;
    g.circle(ring.cx, ring.cy, R).stroke({ width: 0.3, color: 0x8d877b });
    g.circle(ring.cx, ring.cy, R - 0.13).stroke({ width: 0.04, color: INK, alpha: 0.6 });
    g.circle(ring.cx, ring.cy, R).stroke({ width: 0.1, color: INK });
    g.circle(ring.cx, ring.cy, R).stroke({ width: 0.05, color: 0x5c5650 });
    const n = Math.max(12, Math.round((Math.PI * 2 * R) / 0.8));
    for (let k = 0; k < n; k++) {
      const a = (k / n) * Math.PI * 2;
      g.circle(ring.cx + Math.cos(a) * R, ring.cy + Math.sin(a) * R, 0.065).fill(IRON).stroke({ width: 0.025, color: INK });
    }
  }
  // The lantern: a brass sill, glass panes between iron mullions, the door's gap open.
  const gaps: [number, number][] = [];
  for (const d of doors ?? []) {
    if (d.rooms[0] !== ri && d.rooms[1] !== ri) continue;
    const [a, b] = [Math.atan2(d.a[1] - cy, d.a[0] - cx), Math.atan2(d.b[1] - cy, d.b[0] - cx)];
    let [lo, hi] = a < b ? [a, b] : [b, a];
    if (hi - lo > Math.PI) [lo, hi] = [hi, lo + Math.PI * 2];
    gaps.push([lo - 0.04, hi + 0.04]);
  }
  const inGap = (t: number) => gaps.some(([lo, hi]) => [t, t + Math.PI * 2, t - Math.PI * 2].some((x) => x >= lo && x <= hi));
  const panes = 12;
  const [r0, r1] = [r - 0.13, r + 0.13];
  for (let k = 0; k < panes; k++) {
    const a0 = (k / panes) * Math.PI * 2;
    const a1 = ((k + 1) / panes) * Math.PI * 2;
    const steps = 6;
    const ts = Array.from({ length: steps + 1 }, (_, s) => a0 + ((a1 - a0) * s) / steps).filter((t) => !inGap(t));
    if (ts.length < 2) continue;
    const pts: number[] = [];
    for (const t of ts) pts.push(cx + Math.cos(t) * (r1 + 0.05), cy + Math.sin(t) * (r1 + 0.05));
    for (const t of [...ts].reverse()) pts.push(cx + Math.cos(t) * (r0 - 0.05), cy + Math.sin(t) * (r0 - 0.05));
    g.poly(pts).fill(BRASS).stroke({ width: 0.04, color: INK });
    const glass: number[] = [];
    for (const t of ts) glass.push(cx + Math.cos(t) * r1, cy + Math.sin(t) * r1);
    for (const t of [...ts].reverse()) glass.push(cx + Math.cos(t) * r0, cy + Math.sin(t) * r0);
    // (Lit from within: the inner half warm, a glint on each pane.)
    g.poly(glass).fill(GLASS);
    const inner: number[] = [];
    for (const t of ts) inner.push(cx + Math.cos(t) * (r0 + 0.11), cy + Math.sin(t) * (r0 + 0.11));
    for (const t of [...ts].reverse()) inner.push(cx + Math.cos(t) * r0, cy + Math.sin(t) * r0);
    g.poly(inner).fill({ color: GLASS_LIT, alpha: 0.8 });
    const m = ts[Math.floor(ts.length / 2)];
    g.moveTo(cx + Math.cos(m - 0.1) * (r1 - 0.05), cy + Math.sin(m - 0.1) * (r1 - 0.05)).lineTo(cx + Math.cos(m + 0.05) * (r1 - 0.05), cy + Math.sin(m + 0.05) * (r1 - 0.05)).stroke({ width: 0.03, color: 0xffffff, alpha: 0.85 });
  }
  // (The mullions between panes, thin iron, outside the door's gap.)
  for (let k = 0; k < panes; k++) {
    const a = (k / panes) * Math.PI * 2;
    if (inGap(a)) continue;
    g.moveTo(cx + Math.cos(a) * (r0 - 0.06), cy + Math.sin(a) * (r0 - 0.06)).lineTo(cx + Math.cos(a) * (r1 + 0.06), cy + Math.sin(a) * (r1 + 0.06));
  }
  g.stroke({ width: 0.05, color: IRON, cap: 'round' });
}
