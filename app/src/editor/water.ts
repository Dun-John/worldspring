// Build › Water: water drawn by hand (`Edits.waters`: `worldgen::waters`), in two shapes. Brush:
// press and drag paints water along the stroke at the width set (`band`), or click along a line
// and press Enter or double-click to end it; ending on its start (or clicking the first point)
// brings it round as a ring. Polygon: click its points, then the first again (or double-click),
// or drag round it freehand and let go near the start. A natural shore is shaped into a curve
// near the points, a dug one kept straight with rounded corners (`shore`, as the generator
// shapes it). Polygons that overlap are one water. The look follows the water (`lookOf`, as
// `waters::Form::look`): the sea's at its level, a stream along an open line, a moat round a
// ring, else a pond or a lake by its depth. A click inside water drawn earlier picks it (its
// width, depth, shore, name, or take it away) when nothing is being drawn. Escape drops the
// points. Right-drag pans meanwhile.
import type { Graphics } from 'pixi.js';
import type { Edits, Water, WaterKind } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import { dragTo, endAt } from './line';

export type Pt = [number, number];

export const MAX_WATER_PTS = 128;
export const MAX_WATER_SPAN_FT = 26_400;
export const MAX_WATERS = 256;

export interface WaterSettings {
  /** Brush (a band along a line) or polygon. */
  brush: boolean;
  /** A brushed line's width (ft, `BRUSH_WIDTHS`). */
  width: number;
  /** Its depth (ft, `DEPTHS`). */
  depth: number;
  /** Dug: straight sides, a stone edge. */
  dug: boolean;
}

/** A brushed line's width when none is given and the widths allowed, and the depths (`waters`). */
export const BRUSH_WIDTHS: [number, number, number] = [10, 3, 80];
export const DEPTHS: [number, number, number] = [4, 1, 20];
/** Still water shallower than this looks a pond (`waters::POND_BELOW_FT`). */
export const POND_BELOW_FT = 4;
/** Water within this of the sea's level is the sea's (`waters::SEA_WITHIN_FT`). */
export const SEA_WITHIN_FT = 0.25;

export const defaultWater = (): WaterSettings => ({ brush: true, width: BRUSH_WIDTHS[0], depth: DEPTHS[0], dug: false });

/** A legacy kind's depth (`WaterKind::depth`). */
const KIND_DEPTH: Record<WaterKind, number> = { inlet: 12, basin: 10, lake: 8, pond: 3, moat: 6, stream: 1.5 };
/** Its kind if it was made with one (`Water::legacy`: neither a kind nor a depth is a lake). */
export const legacy = (w: Water): WaterKind | null => w.kind ?? (w.depth === undefined ? 'lake' : null);
/** Its width's default and the widths allowed (`Water::widths`). */
export function widths(w: Water): [number, number, number] {
  const k = legacy(w);
  return !k ? BRUSH_WIDTHS : k === 'stream' ? [5, 3, 12] : [20, 8, 60];
}
/** Whether it is a line (`Water::is_line`). */
export const isLine = (w: Water) => !!w.line || legacy(w) === 'stream';
/** Whether it is an open line whose level falls along it (`Water::flows`). */
export function flows(w: Water): boolean {
  const k = legacy(w);
  return k ? k === 'stream' : !!w.line && !w.closed;
}
/** A line's width (`Water::width_ft`). */
export function widthOf(w: Water): number {
  const [d, lo, hi] = widths(w);
  const v = w.width !== undefined && Number.isFinite(w.width) ? w.width : d;
  return Math.min(hi, Math.max(lo, v));
}
const isClosed = (w: Water) => isLine(w) && !!w.closed && !flows(w);
/** Its depth (`Water::depth_ft`). */
export function depthOf(w: Water): number {
  if (w.depth !== undefined && Number.isFinite(w.depth)) return Math.min(DEPTHS[2], Math.max(DEPTHS[1], w.depth));
  return KIND_DEPTH[legacy(w) ?? 'lake'];
}
/** Whether it is dug (`Water::is_dug`). */
export function isDug(w: Water): boolean {
  if (w.dug !== undefined) return w.dug;
  const k = legacy(w);
  return k === 'basin' || k === 'moat';
}

/** What water looks like (`waters::Look`): which look, and dug (a stone edge). */
export interface WaterLook {
  kind: WaterKind;
  dug: boolean;
}

/** Its look (`waters::Form::look`), the sea at `sea` ft: a legacy kind's own; else the sea's at
 * its level (a quay when dug), a stream along an open line, a moat round a ring, else a pond
 * shallower than `POND_BELOW_FT`, a lake deeper. */
export function lookOf(w: Water, sea: number): WaterLook {
  const dug = isDug(w);
  const k = legacy(w);
  if (k) return { kind: k, dug };
  if (w.level <= sea + SEA_WITHIN_FT) return { kind: dug ? 'basin' : 'inlet', dug };
  if (flows(w)) return { kind: 'stream', dug };
  if (isClosed(w)) return { kind: 'moat', dug };
  return { kind: depthOf(w) < POND_BELOW_FT ? 'pond' : 'lake', dug };
}

/** Whether a legacy kind's level is the sea's by default (`WaterKind::of_sea`). */
export const ofSea = (w: Water) => legacy(w) === 'inlet' || legacy(w) === 'basin';

/** Why an outline can't be water (mirrors `Water::problem`), if so. */
export function waterProblem(w: Water): string | null {
  if (isLine(w)) return lineProblem(w);
  const n = w.pts.length;
  if (n < 3) return 'An outline needs 3 points or more';
  if (n > MAX_WATER_PTS) return `An outline has at most ${MAX_WATER_PTS} points`;
  const xs = w.pts.map((p) => p[0]);
  const ys = w.pts.map((p) => p[1]);
  if (Math.max(...xs) - Math.min(...xs) > MAX_WATER_SPAN_FT || Math.max(...ys) - Math.min(...ys) > MAX_WATER_SPAN_FT) return 'An outline spans 5 miles at most';
  if (Math.abs(area(w.pts)) < 25) return 'The outline holds no water';
  if (crosses(w.pts)) return 'The outline crosses itself';
  return null;
}

function lineProblem(w: Water): string | null {
  const [, lo, hi] = widths(w);
  if (w.width !== undefined && !(w.width >= lo && w.width <= hi)) return `The width is ${lo} to ${hi} ft`;
  const n = w.pts.length;
  const closed = isClosed(w);
  const least = closed ? 3 : 2;
  if (n < least) return `A line needs ${least} points or more`;
  if (n > MAX_WATER_PTS) return `A line has at most ${MAX_WATER_PTS} points`;
  const xs = w.pts.map((p) => p[0]);
  const ys = w.pts.map((p) => p[1]);
  if (Math.max(...xs) - Math.min(...xs) > MAX_WATER_SPAN_FT || Math.max(...ys) - Math.min(...ys) > MAX_WATER_SPAN_FT) return 'A line spans 5 miles at most';
  const c = centre(w);
  let len = 0;
  for (let i = 1; i < c.length; i++) len += dist(c[i - 1], c[i]);
  if (len < widthOf(w) || c.some((p, i) => i > 0 && dist(c[i - 1], p) < 0.5)) return 'The line is too short';
  if (closed && (crosses(c) || Math.abs(area(c)) < 25)) return 'The ring crosses itself';
  if (!bandOk(c, closed, widthOf(w) / 2)) return 'The water folds over itself: make the bends gentler or the water narrower';
  return null;
}

function area(p: Pt[]): number {
  let a = 0;
  for (let i = 0; i < p.length; i++) {
    const [x0, y0] = p[i];
    const [x1, y1] = p[(i + 1) % p.length];
    a += x0 * y1 - x1 * y0;
  }
  return a / 2;
}

function crosses(p: Pt[]): boolean {
  const n = p.length;
  const side = (a: Pt, b: Pt, c: Pt) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  for (let i = 0; i < n; i++) {
    const [a, b] = [p[i], p[(i + 1) % n]];
    for (let j = i + 1; j < n; j++) {
      if (j === i + 1 || (i === 0 && j === n - 1)) continue;
      const [c, d] = [p[j], p[(j + 1) % n]];
      if (side(a, b, c) * side(a, b, d) < 0 && side(c, d, a) * side(c, d, b) < 0) return true;
    }
  }
  return false;
}

// The shore shaped from an outline's points, as the generator shapes it (`waters::shape`: the
// same steps and arithmetic, so the map draws the shore the battlemap has).
const SHORE_STEP_FT = 12;
const MAX_SHORE_PTS = 384;
const PULL = 0.6;
const FILLET_FT = 6;
const SHORE_SEED = 0x5a3e91c70d2b6f48n;
const M64 = (1n << 64n) - 1n;

const shores = new WeakMap<Water, Pt[]>();

/** A water's shore (`Water::shore`: a line's `band`), kept per outline object. */
export function shore(w: Water): Pt[] {
  let s = shores.get(w);
  if (!s) shores.set(w, (s = isLine(w) ? band(centre(w), isClosed(w), widthOf(w) / 2).pts : shapeShore(w.pts, !isDug(w))));
  return s;
}

/** The shore lines to ink: an outline's shore, or a ring's outer and inner edges. */
export function shoreRings(w: Water): Pt[][] {
  if (!isLine(w) || !isClosed(w)) return [shore(w)];
  const c = centre(w);
  const h = widthOf(w) / 2;
  return [side(c, h, true), side(c, -h, true)];
}

/** A line's middle (`Water::centre`): an open line's (a stream's) smoothed twice. */
export function centre(w: Water): Pt[] {
  return flows(w) ? chaikin(chaikin(w.pts)) : w.pts.slice();
}

function chaikin(c: Pt[]): Pt[] {
  if (c.length < 3) return c.slice();
  const out: Pt[] = [c[0]];
  for (let i = 1; i < c.length; i++) {
    const [a, b] = [c[i - 1], c[i]];
    out.push([a[0] + (b[0] - a[0]) * 0.25, a[1] + (b[1] - a[1]) * 0.25], [a[0] + (b[0] - a[0]) * 0.75, a[1] + (b[1] - a[1]) * 0.75]);
  }
  out.push(c[c.length - 1]);
  return out;
}

const CAP_STEPS = 8;

/** `waters::band`: the band `h` ft either side of line `c` (a ring's outer edge, a bridge, its
 * inner edge backwards and back: `bridge` the first bridge edge). */
export function band(c: Pt[], closed: boolean, h: number): { pts: Pt[]; bridge: number | null } {
  if (c.length < 2) return { pts: c.slice(), bridge: null };
  if (closed) {
    const [outer, inner] = [side(c, h, true), side(c, -h, true)];
    const pts = [...outer, outer[0], inner[0], ...inner.slice(1).reverse(), inner[0]];
    return { pts, bridge: outer.length };
  }
  const [l, r] = [side(c, h, false), side(c, -h, false)];
  const n = c.length;
  const ang = (a: Pt, b: Pt) => Math.atan2(b[1] - a[1], b[0] - a[0]) + Math.PI / 2;
  const out = l;
  const cap = (p: Pt, a0: number) => {
    for (let j = 1; j < CAP_STEPS; j++) {
      const a = a0 - (Math.PI * j) / CAP_STEPS;
      out.push([p[0] + h * Math.cos(a), p[1] + h * Math.sin(a)]);
    }
  };
  cap(c[n - 1], ang(c[n - 2], c[n - 1]));
  out.push(...r.reverse());
  cap(c[0], ang(c[0], c[1]) + Math.PI);
  return { pts: out, bridge: null };
}

/** `waters::side`: one edge of line `c`, `h` ft to its left (right when negative). */
export function side(c: Pt[], h: number, closed: boolean): Pt[] {
  const n = c.length;
  const norm = (a: Pt, b: Pt): Pt => {
    const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
    const l = Math.max(Math.sqrt(dx * dx + dy * dy), 1e-9);
    return [-dy / l, dx / l];
  };
  const out: Pt[] = [];
  for (let i = 0; i < n; i++) {
    const p = c[i];
    const prev = i > 0 ? c[i - 1] : closed ? c[n - 1] : null;
    const next = i + 1 < n ? c[i + 1] : closed ? c[0] : null;
    if (!prev && !next) continue;
    const m0 = prev ? norm(prev, p) : norm(p, next!);
    const m1 = next ? norm(p, next) : norm(prev!, p);
    const cross = m0[0] * m1[1] - m0[1] * m1[0];
    const dot = m0[0] * m1[0] + m0[1] * m1[1];
    if (cross * h < 0 && dot < 0.995) {
      const [a0, a1] = [Math.atan2(m0[1], m0[0]), Math.atan2(m1[1], m1[0])];
      let da = a1 - a0;
      if (da > Math.PI) da -= 2 * Math.PI;
      else if (da < -Math.PI) da += 2 * Math.PI;
      const k = Math.max(1, Math.ceil(Math.abs(da) / (Math.PI / 8)));
      for (let j = 0; j <= k; j++) {
        const a = a0 + (da * j) / k;
        out.push([p[0] + h * Math.cos(a), p[1] + h * Math.sin(a)]);
      }
    } else {
      let [bx, by] = [m0[0] + m1[0], m0[1] + m1[1]];
      const bl = Math.max(Math.sqrt(bx * bx + by * by), 1e-9);
      [bx, by] = [bx / bl, by / bl];
      const l = h / Math.max(bx * m0[0] + by * m0[1], 0.25);
      out.push([p[0] + bx * l, p[1] + by * l]);
    }
  }
  return out;
}

function bandOk(c: Pt[], closed: boolean, h: number): boolean {
  if (!closed) return !crosses(band(c, false, h).pts);
  const [outer, inner] = [side(c, h, true), side(c, -h, true)];
  const [big, small] = Math.abs(area(outer)) >= Math.abs(area(inner)) ? [outer, inner] : [inner, outer];
  if (area(big) * area(small) <= 0 || Math.abs(area(small)) < 25 || crosses(big) || crosses(small)) return false;
  const sd = (a: Pt, b: Pt, q: Pt) => (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
  for (let i = 0; i < big.length; i++) {
    const [a, b] = [big[i], big[(i + 1) % big.length]];
    for (let j = 0; j < small.length; j++) {
      const [q, r] = [small[j], small[(j + 1) % small.length]];
      if (sd(a, b, q) * sd(a, b, r) < 0 && sd(q, r, a) * sd(q, r, b) < 0) return false;
    }
  }
  return contains(big, small[0][0], small[0][1]);
}

const STREAM_STEP_FT = 20;
const MAX_STREAM_LEVELS = 512;

/** `waters::stream_samples`: where a stream's level is read along its middle. */
export function streamSamples(c: Pt[]): Pt[] {
  let total = 0;
  for (let i = 1; i < c.length; i++) total += dist(c[i - 1], c[i]);
  if (c.length < 2 || total <= 0) return c.slice(0, 1);
  const n = Math.min(MAX_STREAM_LEVELS, Math.max(2, Math.ceil(total / STREAM_STEP_FT) + 1));
  const out: Pt[] = [];
  let [seg, s0] = [0, 0];
  for (let k = 0; k < n; k++) {
    const s = (total * k) / (n - 1);
    while (seg + 2 < c.length && s0 + dist(c[seg], c[seg + 1]) < s) {
      s0 += dist(c[seg], c[seg + 1]);
      seg++;
    }
    const l = dist(c[seg], c[seg + 1]);
    const t = l > 0 ? Math.min(1, Math.max(0, (s - s0) / l)) : 0;
    out.push([c[seg][0] + (c[seg + 1][0] - c[seg][0]) * t, c[seg][1] + (c[seg + 1][1] - c[seg][1]) * t]);
  }
  return out;
}

/** `waters::stream_levels`: water's level where wet, else 1 ft under the ground; never rising
 * from the higher end; at least `floor`; to 0.1 ft. */
export function streamLevels(at: [number, number | null][], floor: number): number[] {
  const h = at.map(([g, wet]) => wet ?? g - 1);
  if (!h.length) return [];
  const down = h[0] >= h[h.length - 1];
  const out = new Array<number>(h.length).fill(0);
  let low = Infinity;
  for (let k = 0; k < h.length; k++) {
    const i = down ? k : h.length - 1 - k;
    low = Math.min(low, h[i]);
    out[i] = Math.round(Math.max(low, floor) * 10) / 10;
  }
  return out;
}

/** `waters::shape`; `wobbly` false leaves the wobble out (for the outline being drawn). */
export function shapeShore(pts: Pt[], natural: boolean, wobbly = true): Pt[] {
  const n = pts.length;
  const ok = (s: Pt[]) => s.length >= 3 && s.every((p) => Number.isFinite(p[0]) && Number.isFinite(p[1])) && Math.abs(area(s)) >= 25 && !crosses(s);
  if (n < 3 || !pts.every((p) => Number.isFinite(p[0]) && Number.isFinite(p[1]))) return pts.slice();
  if (!natural) {
    const f = fillet(pts);
    return ok(f) ? f : pts.slice();
  }
  const curve = spline(pts);
  const a = Math.abs(area(pts));
  const per = perimeter(pts);
  if (per > 0 && wobbly) {
    const hw = (2 * a) / per;
    const amp = Math.min(Math.min(0.1 * Math.sqrt(a), 0.2 * hw), 40);
    const wave = Math.min(Math.max(1.2 * hw, 25), 400);
    for (const f of [1, 0.5]) {
      if (amp * f < 0.5) break;
      const s = wobble(curve, amp * f, wave);
      if (ok(s)) return s;
    }
  }
  return ok(curve) ? curve : pts.slice();
}

const dist = (a: Pt, b: Pt) => {
  const [dx, dy] = [a[0] - b[0], a[1] - b[1]];
  return Math.sqrt(dx * dx + dy * dy);
};

function perimeter(pts: Pt[]): number {
  let s = 0;
  for (let i = 0; i < pts.length; i++) s += dist(pts[i], pts[(i + 1) % pts.length]);
  return s;
}

function spline(pts: Pt[]): Pt[] {
  const n = pts.length;
  const step = Math.max(SHORE_STEP_FT, perimeter(pts) / (MAX_SHORE_PTS - Math.min(n, MAX_SHORE_PTS - 1)));
  const q: Pt[] = pts.map((p, i) => {
    const [a, b] = [pts[(i + n - 1) % n], pts[(i + 1) % n]];
    const on = (k: 0 | 1) => (a[k] + 4 * p[k] + b[k]) / 6;
    return [p[0] + (p[0] - on(0)) * PULL, p[1] + (p[1] - on(1)) * PULL];
  });
  const out: Pt[] = [];
  for (let i = 0; i < n; i++) {
    const [q0, q1, q2, q3] = [q[(i + n - 1) % n], q[i], q[(i + 1) % n], q[(i + 2) % n]];
    const k = Math.max(Math.ceil(dist(pts[i], pts[(i + 1) % n]) / step), 1);
    for (let j = 0; j < k; j++) {
      const u = j / k;
      const [u2, u3, v] = [u * u, u * u * u, 1 - u];
      const [b0, b1, b2, b3] = [(v * v * v) / 6, (3 * u3 - 6 * u2 + 4) / 6, (-3 * u3 + 3 * u2 + 3 * u + 1) / 6, u3 / 6];
      out.push([q0[0] * b0 + q1[0] * b1 + q2[0] * b2 + q3[0] * b3, q0[1] * b0 + q1[1] * b1 + q2[1] * b2 + q3[1] * b3]);
    }
  }
  return out;
}

function wobble(curve: Pt[], amp: number, wave: number): Pt[] {
  const n = curve.length;
  return curve.map((p, i) => {
    const [a, b] = [curve[(i + n - 1) % n], curve[(i + 1) % n]];
    const [tx, ty] = [b[0] - a[0], b[1] - a[1]];
    const l = Math.sqrt(tx * tx + ty * ty);
    if (l < 1e-9) return p;
    const d = amp * shoreNoise(p[0] / wave, p[1] / wave);
    return [p[0] + (ty / l) * d, p[1] - (tx / l) * d];
  });
}

const shoreNoise = (x: number, y: number) => (value(x, y) + 0.5 * value(x * 2.5 + 17.25, y * 2.5 - 9.5)) / 1.5;

/** `core::rng::mix64`, `hash2` and `signed`. */
function mix64(z: bigint): bigint {
  z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & M64;
  z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & M64;
  return z ^ (z >> 31n);
}
function signed(x: number, y: number): number {
  const h = mix64(SHORE_SEED ^ mix64((BigInt(x) & M64) ^ mix64((BigInt(y) & M64) ^ 0x9e3779b97f4a7c15n)));
  return Number(h >> 11n) * 2 ** -53 * 2 - 1;
}

function value(x: number, y: number): number {
  const [fx, fy] = [Math.floor(x), Math.floor(y)];
  let [u, v] = [x - fx, y - fy];
  [u, v] = [u * u * (3 - 2 * u), v * v * (3 - 2 * v)];
  const [h00, h10, h01, h11] = [signed(fx, fy), signed(fx + 1, fy), signed(fx, fy + 1), signed(fx + 1, fy + 1)];
  const [top, bottom] = [h00 + (h10 - h00) * u, h01 + (h11 - h01) * u];
  return top + (bottom - top) * v;
}

function fillet(pts: Pt[]): Pt[] {
  const n = pts.length;
  const out: Pt[] = [];
  for (let i = 0; i < n; i++) {
    const [a, b, c] = [pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]];
    const [la, lc] = [dist(a, b), dist(b, c)];
    const r = Math.min(Math.min(FILLET_FT, 0.25 * la), 0.25 * lc);
    if (r < 0.5) {
      out.push(b);
      continue;
    }
    const [ua, uc]: Pt[] = [[(a[0] - b[0]) / la, (a[1] - b[1]) / la], [(c[0] - b[0]) / lc, (c[1] - b[1]) / lc]];
    if (ua[0] * uc[0] + ua[1] * uc[1] < -0.985) {
      out.push(b);
      continue;
    }
    const [p0, p2]: Pt[] = [[b[0] + ua[0] * r, b[1] + ua[1] * r], [b[0] + uc[0] * r, b[1] + uc[1] * r]];
    for (let j = 0; j <= 4; j++) {
      const t = j / 4;
      const [w0, w1, w2] = [(1 - t) * (1 - t), 2 * (1 - t) * t, t * t];
      out.push([p0[0] * w0 + b[0] * w1 + p2[0] * w2, p0[1] * w0 + b[1] * w1 + p2[1] * w2]);
    }
  }
  return out;
}

export function contains(pts: Pt[], x: number, y: number): boolean {
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i];
    const [xj, yj] = pts[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/** Freehand outlines are sampled this far apart (ft) at least. */
const FREEHAND_FT = 15;

/** An open line with points dropped while it stays within `tol` ft of them, the tolerance raised
 * until at most `max` (Douglas-Peucker). */
function simplifyOpen(pts: Pt[], max: number): Pt[] {
  if (pts.length <= 2) return pts;
  for (let tol = 1.5; ; tol *= 1.5) {
    const keep = pts.map((_, i) => i === 0 || i === pts.length - 1);
    const run = (a: number, b: number) => {
      const [p, q] = [pts[a], pts[b]];
      const [dx, dy] = [q[0] - p[0], q[1] - p[1]];
      const l2 = dx * dx + dy * dy;
      let [best, at] = [-1, -1];
      for (let i = a + 1; i < b; i++) {
        const r = pts[i];
        const t = l2 > 0 ? Math.max(0, Math.min(1, ((r[0] - p[0]) * dx + (r[1] - p[1]) * dy) / l2)) : 0;
        const d = Math.hypot(r[0] - p[0] - dx * t, r[1] - p[1] - dy * t);
        if (d > best) [best, at] = [d, i];
      }
      if (at < 0 || best <= tol) return;
      keep[at] = true;
      run(a, at);
      run(at, b);
    };
    run(0, pts.length - 1);
    const out = pts.filter((_, i) => keep[i]);
    if (out.length <= max) return out;
  }
}

/** A closed outline with points dropped while it stays within `tol` ft of them (Douglas-Peucker,
 * from its first point and the one farthest from it), the tolerance raised until at most `max`. */
export function simplify(pts: Pt[], max: number): Pt[] {
  if (pts.length <= 3) return pts;
  let far = 1;
  for (let i = 1; i < pts.length; i++) if (Math.hypot(pts[i][0] - pts[0][0], pts[i][1] - pts[0][1]) > Math.hypot(pts[far][0] - pts[0][0], pts[far][1] - pts[0][1])) far = i;
  const run = (a: number, b: number, tol: number, keep: boolean[]) => {
    const [p, q] = [pts[a], pts[b % pts.length]];
    const [dx, dy] = [q[0] - p[0], q[1] - p[1]];
    const l2 = dx * dx + dy * dy;
    let [best, at] = [-1, -1];
    for (let i = a + 1; i < b; i++) {
      const r = pts[i];
      const t = l2 > 0 ? Math.max(0, Math.min(1, ((r[0] - p[0]) * dx + (r[1] - p[1]) * dy) / l2)) : 0;
      const d = Math.hypot(r[0] - p[0] - dx * t, r[1] - p[1] - dy * t);
      if (d > best) [best, at] = [d, i];
    }
    if (at < 0 || best <= tol) return;
    keep[at] = true;
    run(a, at, tol, keep);
    run(at, b, tol, keep);
  };
  for (let tol = 1.5; ; tol *= 1.5) {
    const keep = pts.map((_, i) => i === 0 || i === far);
    run(0, far, tol, keep);
    run(far, pts.length, tol, keep);
    const out = pts.filter((_, i) => keep[i]);
    if (out.length <= max) return out;
  }
}

/** What the tool needs from the app. */
export interface WaterHost {
  edits(): Edits;
  picked(): string | null;
  /** Brushing a line (else a polygon). */
  brush(): boolean;
  /** Drawing a dug shore (its shape in the preview). */
  dug(): boolean;
  /** An outline closed, or a line ended (`closed`: come round as a ring) (world ft, rounded to 0.5). */
  placed(pts: Pt[], closed?: boolean): void;
  /** A line's width (ft), for its preview. */
  width(): number;
  /** Water drawn earlier was clicked. */
  pick(id: string): void;
  hint(text: string): void;
}

const round = (p: Pt): Pt => [Math.round(p[0] * 2) / 2, Math.round(p[1] * 2) / 2];

export class WaterTool implements PointerTool {
  private pts: Pt[] = [];
  private cursor: Pt | null = null;
  /** ft per px, as last drawn. */
  private fpp = 1;
  /** Where the button went down, and the freehand outline once it is dragged (no points yet). */
  private press: Pt | null = null;
  private stroke: Pt[] | null = null;

  constructor(private readonly host: WaterHost) {}

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    this.press = [x, y];
    this.stroke = null;
    return true;
  }

  move(x: number, y: number) {
    this.cursor = [x, y];
    const press = this.press;
    if (!press || this.pts.length) return;
    this.stroke = dragTo(this.stroke, press, [x, y], 6 * this.fpp, Math.max(FREEHAND_FT, 3 * this.fpp));
  }

  up(x: number, y: number) {
    this.press = null;
    if (this.stroke) return this.endStroke(x, y);
    const p = round([x, y]);
    const line = this.host.brush();
    if (!this.pts.length) {
      const hit = this.at(p);
      if (hit && hit !== this.host.picked()) return this.host.pick(hit);
      this.pts = [p];
      return this.host.hint(line ? 'Click along it; press Enter or double-click to end it, or click the first point to bring it round' : 'Click its next points; click the first again to close it');
    }
    const first = this.pts[0];
    if (this.pts.length >= 3 && Math.hypot(p[0] - first[0], p[1] - first[1]) <= 8 * this.fpp) return this.close(line);
    const last = this.pts[this.pts.length - 1];
    if (Math.hypot(p[0] - last[0], p[1] - last[1]) < 1) return;
    if (this.pts.length >= MAX_WATER_PTS) return this.host.hint(`An outline has at most ${MAX_WATER_PTS} points: click the first to close it`);
    this.pts.push(p);
  }

  /** A freehand outline let go: closed when let go near its start (or it comes round to it),
   * else its points kept to click on from. */
  private endStroke(x: number, y: number) {
    const s = this.stroke!;
    this.stroke = null;
    const lastPt = s[s.length - 1];
    endAt(s, [x, y]);
    let length = 0;
    for (let i = 1; i < s.length; i++) length += Math.hypot(s[i][0] - s[i - 1][0], s[i][1] - s[i - 1][1]);
    const gap = Math.hypot(x - s[0][0], y - s[0][1]);
    const line = this.host.brush();
    const closes = gap <= Math.max(16 * this.fpp, 0.1 * length);
    if (closes && Math.hypot(lastPt[0] - s[0][0], lastPt[1] - s[0][1]) < 1e-9) s.pop();
    if (line) {
      // (A line drawn freehand is done when let go: a ring when let go near its start.)
      const pts: Pt[] = [];
      for (const p of simplifyOpen(s, MAX_WATER_PTS).map(round)) {
        const q = pts[pts.length - 1];
        if (!q || Math.hypot(p[0] - q[0], p[1] - q[1]) >= 1) pts.push(p);
      }
      if (closes && pts.length > 3 && Math.hypot(pts[pts.length - 1][0] - pts[0][0], pts[pts.length - 1][1] - pts[0][1]) <= Math.max(16 * this.fpp, 0.1 * length)) pts.pop();
      this.pts = pts;
      if (pts.length >= (closes ? 3 : 2)) return this.close(closes);
      this.pts = [];
      return;
    }
    const pts: Pt[] = [];
    for (const p of simplify(s, closes ? MAX_WATER_PTS : MAX_WATER_PTS - 1).map(round)) {
      const q = pts[pts.length - 1];
      if (!q || Math.hypot(p[0] - q[0], p[1] - q[1]) >= 1) pts.push(p);
    }
    if (closes && pts.length >= 3) {
      this.pts = pts;
      return this.close();
    }
    this.pts = pts;
    this.host.hint('Let go near the start to close it, or click on and click the first point');
  }

  dblclick(): boolean {
    if (this.host.brush()) {
      // (The double click's second press added a point where the first did.)
      if (this.pts.length < 2) return this.pts.length > 0;
      this.close();
      return true;
    }
    if (this.pts.length < 3) return this.pts.length > 0;
    this.close();
    return true;
  }

  /** Enter: a line ended where it is, an outline closed. */
  finish() {
    if (this.pts.length >= (this.host.brush() ? 2 : 3)) this.close();
  }

  private close(ring = false) {
    const pts = this.pts;
    this.pts = [];
    this.host.placed(pts, ring);
  }

  /** The water drawn earlier under p (the smallest holding it), if any. */
  private at(p: Pt): string | null {
    let best: [string, number] | null = null;
    for (const [id, w] of Object.entries(this.host.edits().waters ?? {})) {
      const sh = shore(w);
      if (!contains(sh, p[0], p[1])) continue;
      const a = Math.abs(area(sh));
      if (!best || a < best[1]) best = [id, a];
    }
    return best?.[0] ?? null;
  }

  /** Drop the points clicked. True if there were some. */
  reset(): boolean {
    const had = this.pts.length > 0 || !!this.stroke;
    this.pts = [];
    this.stroke = null;
    this.press = null;
    return had;
  }

  cancel() {
    this.stroke = null;
    this.press = null;
  }

  hover(x: number, y: number) {
    this.cursor = [x, y];
  }

  draw(g: Graphics, cam: Camera) {
    this.fpp = 1 / cam.ppf;
    const ink = 0x2563eb;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const picked = this.host.picked();
    for (const [id, w] of Object.entries(this.host.edits().waters ?? {})) {
      const on = id === picked;
      g.poly(shore(w).flatMap((p) => scr(p)), true).fill({ color: ink, alpha: on ? 0.2 : 0.04 });
      for (const r of shoreRings(w)) g.poly(r.flatMap((p) => scr(p)), true).stroke({ width: on ? 2 : 1, color: ink, alpha: on ? 1 : 0.5 });
    }
    if (this.stroke) {
      const line = this.cursor ? [...this.stroke, this.cursor] : this.stroke;
      const s0 = scr(line[0]);
      g.moveTo(s0[0], s0[1]);
      for (const p of line.slice(1)) {
        const s = scr(p);
        g.lineTo(s[0], s[1]);
      }
      g.stroke({ width: 2, color: ink });
      g.rect(s0[0] - 5, s0[1] - 5, 10, 10).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
      return;
    }
    if (this.cursor && !this.pts.length) {
      const [sx, sy] = scr(this.cursor);
      g.circle(sx, sy, 3).fill({ color: ink });
    }
    if (!this.pts.length) return;
    const line = this.cursor ? [...this.pts, this.cursor] : this.pts;
    // The shore it will have (without its wobble) once there are three points, the points' line
    // faint; a line's band as soon as it has two.
    const asLine = this.host.brush();
    const shaped = asLine
      ? line.length >= 2
        ? band(chaikin(chaikin(line)), false, this.host.width() / 2).pts
        : null
      : line.length >= 3
        ? shapeShore(line, !this.host.dug(), false)
        : null;
    const s0 = scr(line[0]);
    g.moveTo(s0[0], s0[1]);
    for (const p of line.slice(1)) {
      const s = scr(p);
      g.lineTo(s[0], s[1]);
    }
    g.stroke({ width: shaped ? 1 : 2, color: ink, alpha: shaped ? 0.45 : 1 });
    if (shaped) g.poly(shaped.flatMap((p) => scr(p)), true).fill({ color: ink, alpha: 0.08 }).stroke({ width: 2, color: ink });
    for (const [k, p] of this.pts.entries()) {
      const [sx, sy] = scr(p);
      const r = k === 0 ? 5 : 3;
      g.rect(sx - r, sy - r, 2 * r, 2 * r).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
    }
  }
}
