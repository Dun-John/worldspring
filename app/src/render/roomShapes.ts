// Rooms drawn round, octagonal, with an apse, chamfered or as a cavern breaking into a built level
// (the generator's `interior/shape.rs`: the same geometry, from the box round a room's squares),
// and buildings drawn inside a footprint that isn't the grid's rectangle (`Interior.outline`).
// Only the drawing follows them: play keeps to the squares, so a curved wall may stand up to half
// a square off them. Geometry here is in grid units, polygons wound clockwise on screen (y down).
import type { Graphics } from 'pixi.js';
import { OPEN_BELOW, ROOF_SLOPE, type Interior, type InteriorLevel } from '../gen/protocol';

export type Pt = [number, number];
/** A box in grid units: x0, y0, x1, y1. */
export type Box = [number, number, number, number];

export const SQUARE = 0;
export const ROUND = 1;
export const OCTAGON = 2;
export const APSE_N = 3;
export const APSE_E = 4;
export const APSE_S = 5;
export const APSE_W = 6;
export const CHAMFERED = 7;
/** Natural rock breaking into a built level underground: drawn as a cave's outline. */
export const CAVERN = 255;

/** The shapes by name, as the text plan writes them (`a=sanctum (round)`). */
export const SHAPE_NAMES: Record<number, string> = {
  [ROUND]: 'round',
  [OCTAGON]: 'octagon',
  [APSE_N]: 'apse n',
  [APSE_E]: 'apse e',
  [APSE_S]: 'apse s',
  [APSE_W]: 'apse w',
  [CHAMFERED]: 'chamfered',
  [CAVERN]: 'cavern',
};

export const isApse = (s: number | undefined) => s !== undefined && s >= APSE_N && s <= APSE_W;
/** A shape with geometry (not square corners, not a cavern). */
export const geometric = (s: number | undefined) => s !== undefined && s >= ROUND && s <= CHAMFERED;

const OCTAGON_CUT = 1 - Math.SQRT1_2;
const chamfer = (s: number) => (s >= 3 ? 0.75 : 0.5);
/** Apse depth: half the side it rounds, keeping a square of the box straight behind it. */
export const apseDepth = (across: number, along: number) => {
  const r = Math.min(across / 2, along - 1);
  return r < 1 ? 0 : r;
};

/** Twice the signed area (positive: clockwise on screen). */
function area2(p: Pt[]): number {
  let a = 0;
  for (let i = 0; i < p.length; i++) {
    const [x0, y0] = p[i];
    const [x1, y1] = p[(i + 1) % p.length];
    a += x0 * y1 - x1 * y0;
  }
  return a;
}

const wound = (p: Pt[]) => (area2(p) < 0 ? p.reverse() : p);

/** Quarter-turn steps for an arc of radius `r` (grid units). */
const steps = (r: number) => Math.max(3, Math.min(16, Math.round(r * 4)));

/** A shape's outline in box `b` (clockwise on screen); null for square corners and caverns. */
export function shapePolygon(b: Box, shape: number): Pt[] | null {
  const [x0, y0, x1, y1] = b;
  const [w, h] = [x1 - x0, y1 - y0];
  if (shape === ROUND) {
    const r = Math.min(w, h) / 2;
    const out: Pt[] = [];
    const n = steps(r);
    const corners: [number, number, number][] = [
      [x1 - r, y0 + r, -Math.PI / 2],
      [x1 - r, y1 - r, 0],
      [x0 + r, y1 - r, Math.PI / 2],
      [x0 + r, y0 + r, Math.PI],
    ];
    for (const [cx, cy, a0] of corners) for (let k = 0; k <= n; k++) out.push([cx + r * Math.cos(a0 + (k / n) * (Math.PI / 2)), cy + r * Math.sin(a0 + (k / n) * (Math.PI / 2))]);
    return dedupe(out);
  }
  if (shape === OCTAGON || shape === CHAMFERED) {
    const c = shape === OCTAGON ? OCTAGON_CUT * Math.min(w, h) : chamfer(Math.min(w, h));
    return [[x0 + c, y0], [x1 - c, y0], [x1, y0 + c], [x1, y1 - c], [x1 - c, y1], [x0 + c, y1], [x0, y1 - c], [x0, y0 + c]];
  }
  if (isApse(shape)) {
    // In the apse's frame: u across it, v in from its side; then into the grid.
    const across = shape === APSE_N || shape === APSE_S ? w : h;
    const along = shape === APSE_N || shape === APSE_S ? h : w;
    const r = apseDepth(across, along);
    const local: Pt[] = [];
    if (r <= 0) local.push([0, 0], [across, 0]);
    else {
      const n = 2 * steps(Math.max(r, across / 2));
      for (let k = 0; k <= n; k++) {
        const a = Math.PI + (k / n) * Math.PI;
        local.push([across / 2 + (across / 2) * Math.cos(a), r + r * Math.sin(a)]);
      }
    }
    local.push([across, along], [0, along]);
    const to = ([u, v]: Pt): Pt => (shape === APSE_N ? [x0 + u, y0 + v] : shape === APSE_S ? [x0 + u, y1 - v] : shape === APSE_W ? [x0 + v, y0 + u] : [x1 - v, y0 + u]);
    return wound(dedupe(local.map(to)));
  }
  return null;
}

function dedupe(p: Pt[]): Pt[] {
  const out: Pt[] = [];
  for (const q of p) {
    const l = out[out.length - 1];
    if (!l || Math.abs(l[0] - q[0]) + Math.abs(l[1] - q[1]) > 1e-6) out.push(q);
  }
  while (out.length > 1 && Math.abs(out[0][0] - out[out.length - 1][0]) + Math.abs(out[0][1] - out[out.length - 1][1]) <= 1e-6) out.pop();
  return out;
}

/** The box round each room's squares (grid units), null for a room with none. */
export function roomBoxes(lv: InteriorLevel, nx: number, ny: number): (Box | null)[] {
  const out: (Box | null)[] = lv.rooms.map(() => null);
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const r = lv.cells[j * nx + i];
      if (r < 0) continue;
      const b = out[r];
      out[r] = b ? [Math.min(b[0], i), Math.min(b[1], j), Math.max(b[2], i + 1), Math.max(b[3], j + 1)] : [i, j, i + 1, j + 1];
    }
  }
  return out;
}

/** Point inside a convex polygon (wound clockwise on screen), to within `eps`. */
export function insideConvex(p: Pt, poly: Pt[], eps = 1e-6): boolean {
  for (let k = 0; k < poly.length; k++) {
    const a = poly[k];
    const b = poly[(k + 1) % poly.length];
    if ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) < -eps) return false;
  }
  return true;
}

/** Polygon `subject` cut to the convex polygon `clip` (Sutherland–Hodgman). */
export function clipConvex(subject: Pt[], clip: Pt[]): Pt[] {
  let out = subject;
  for (let k = 0; k < clip.length && out.length; k++) {
    const a = clip[k];
    const b = clip[(k + 1) % clip.length];
    const side = (p: Pt) => (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    const input = out;
    out = [];
    for (let i = 0; i < input.length; i++) {
      const p = input[i];
      const q = input[(i + 1) % input.length];
      const [sp, sq] = [side(p), side(q)];
      if (sp >= 0) out.push(p);
      if (sp >= 0 !== sq >= 0) {
        const t = sp / (sp - sq);
        out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
      }
    }
  }
  return out.length >= 3 ? out : [];
}

/** The stretch of segment a-b inside a convex polygon, as parameters [t0, t1] (null: none). */
export function segInside(a: Pt, b: Pt, poly: Pt[]): [number, number] | null {
  let [t0, t1] = [0, 1];
  const d: Pt = [b[0] - a[0], b[1] - a[1]];
  for (let k = 0; k < poly.length; k++) {
    const p = poly[k];
    const q = poly[(k + 1) % poly.length];
    // Inside: (q - p) x (x - p) >= 0, linear in t.
    const s0 = (q[0] - p[0]) * (a[1] - p[1]) - (q[1] - p[1]) * (a[0] - p[0]);
    const ds = (q[0] - p[0]) * d[1] - (q[1] - p[1]) * d[0];
    if (Math.abs(ds) < 1e-12) {
      if (s0 < 0) return null;
      continue;
    }
    const t = -s0 / ds;
    if (ds > 0) t0 = Math.max(t0, t);
    else t1 = Math.min(t1, t);
    if (t0 > t1) return null;
  }
  return t1 - t0 > 1e-6 ? [t0, t1] : null;
}

/** The pieces of segment a-b left once the parts inside each of `polys` are taken out. */
export function segOutside(a: Pt, b: Pt, polys: Pt[][]): [Pt, Pt][] {
  let spans: [number, number][] = [[0, 1]];
  for (const poly of polys) {
    const cut = segInside(a, b, poly);
    if (!cut) continue;
    spans = spans.flatMap(([s, e]): [number, number][] => {
      if (cut[1] <= s || cut[0] >= e) return [[s, e]];
      const out: [number, number][] = [];
      if (cut[0] > s) out.push([s, cut[0]]);
      if (cut[1] < e) out.push([cut[1], e]);
      return out;
    });
  }
  const at = (t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
  return spans.filter(([s, e]) => e - s > 1e-4).map(([s, e]) => [at(s), at(e)]);
}

/** Where a line from `p` along `dir` leaves a convex polygon going forward (the far crossing),
 * with the polygon's edge direction there (unit); null if it misses. */
export function exitPoint(p: Pt, dir: Pt, poly: Pt[]): { at: Pt; tangent: Pt } | null {
  const far = 4;
  const a: Pt = [p[0] - dir[0] * far, p[1] - dir[1] * far];
  const b: Pt = [p[0] + dir[0] * far, p[1] + dir[1] * far];
  const span = segInside(a, b, poly);
  if (!span) return null;
  const at: Pt = [a[0] + (b[0] - a[0]) * span[1], a[1] + (b[1] - a[1]) * span[1]];
  return { at, tangent: edgeAt(at, poly) };
}

/** Where a line from `p` along `dir` enters a convex polygon (the near crossing), as `exitPoint`. */
export function entryPoint(p: Pt, dir: Pt, poly: Pt[]): { at: Pt; tangent: Pt } | null {
  return exitPoint(p, [-dir[0], -dir[1]], poly);
}

/** The direction (unit) of the polygon edge nearest `p`; within `corner` of a corner, between
 * the two edges' directions (so a door or pane there lies across the corner, not along one side). */
export function edgeAt(p: Pt, poly: Pt[], corner = 0.35): Pt {
  let best = Infinity;
  let at = 0;
  let along = 0;
  let fromEnd = false;
  const n = poly.length;
  const dirOf = (k: number): Pt => {
    const a = poly[((k % n) + n) % n];
    const b = poly[(((k + 1) % n) + n) % n];
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
    return [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
  };
  for (let k = 0; k < n; k++) {
    const a = poly[k];
    const b = poly[(k + 1) % n];
    const [ex, ey] = [b[0] - a[0], b[1] - a[1]];
    const l2 = ex * ex + ey * ey;
    if (l2 < 1e-12) continue;
    const t = Math.max(0, Math.min(1, ((p[0] - a[0]) * ex + (p[1] - a[1]) * ey) / l2));
    const d = Math.hypot(a[0] + ex * t - p[0], a[1] + ey * t - p[1]);
    if (d < best) [best, at, along, fromEnd] = [d, k, (t > 0.5 ? 1 - t : t) * Math.sqrt(l2), t > 0.5];
  }
  const dir = dirOf(at);
  // (`along`: from the edge's nearer end, `fromEnd` its far one: a point on a corner is at the far
  // end of the edge before it, between that edge and the next.)
  const other = along >= corner ? null : fromEnd ? dirOf(at + 1) : dirOf(at - 1);
  if (!other) return dir;
  const [mx, my] = [dir[0] + other[0], dir[1] + other[1]];
  const l = Math.hypot(mx, my) || 1;
  return [mx / l, my / l];
}

/** The outer wall's direction at `p` (on it): its nearest part's, across a corner there. */
export function wallTangent(p: Pt, parts: Pt[][]): Pt {
  const part = parts.reduce((best, q) => (Math.abs(insetIn(p, q)) < Math.abs(insetIn(p, best)) ? q : best), parts[0]);
  return edgeAt(p, part);
}

/** A polygon's boundary in short pieces (at most `step` long): each piece's ends and its
 * outward normal (unit; clockwise winding on screen puts the outside on the left of travel). */
export function boundaryPieces(poly: Pt[], step = 0.12): { a: Pt; b: Pt; out: Pt }[] {
  const out: { a: Pt; b: Pt; out: Pt }[] = [];
  for (let k = 0; k < poly.length; k++) {
    const a = poly[k];
    const b = poly[(k + 1) % poly.length];
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (len < 1e-9) continue;
    const n = Math.max(1, Math.ceil(len / step));
    const normal: Pt = [(b[1] - a[1]) / len, -(b[0] - a[0]) / len];
    for (let i = 0; i < n; i++) {
      const p: Pt = [a[0] + ((b[0] - a[0]) * i) / n, a[1] + ((b[1] - a[1]) * i) / n];
      const q: Pt = [a[0] + ((b[0] - a[0]) * (i + 1)) / n, a[1] + ((b[1] - a[1]) * (i + 1)) / n];
      out.push({ a: p, b: q, out: normal });
    }
  }
  return out;
}

/** Runs of consecutive pieces kept (`keep` by index), joined into polylines (closing round the
 * polygon's start when its first and last pieces are both kept). */
export function keptRuns(pieces: { a: Pt; b: Pt }[], keep: (i: number) => boolean): Pt[][] {
  const runs: Pt[][] = [];
  let cur: Pt[] | null = null;
  for (let i = 0; i < pieces.length; i++) {
    if (!keep(i)) {
      cur = null;
      continue;
    }
    if (!cur) {
      cur = [pieces[i].a];
      runs.push(cur);
    }
    cur.push(pieces[i].b);
  }
  // (A run through the start joins the one before it.)
  if (runs.length > 1 && keep(0) && keep(pieces.length - 1)) {
    const first = runs.shift()!;
    runs[runs.length - 1].push(...first.slice(1));
  }
  return runs;
}

/** The two squares either side of a door's edge (some maybe off the grid). */
export function doorSquares(d: { a: [number, number]; b: [number, number] }): [Pt, Pt] {
  const [x0, y0, x1] = [Math.min(d.a[0], d.b[0]), Math.min(d.a[1], d.b[1]), Math.max(d.a[0], d.b[0])];
  return x0 === x1 ? [[x0 - 1, y0], [x0, y0]] : [[x0, y0 - 1], [x0, y0]];
}

// ---------------------------------------------------------------------------------------
// Helpers over an outer wall of convex parts (their union the footprint).

/** How far `p` lies inside a convex polygon (its least distance to an edge's line), negative
 * outside. */
export function insetIn(p: Pt, poly: Pt[]): number {
  let m = Infinity;
  for (let k = 0; k < poly.length; k++) {
    const a = poly[k];
    const b = poly[(k + 1) % poly.length];
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (l < 1e-12) continue;
    m = Math.min(m, ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) / l);
  }
  return m;
}

/** How far `p` lies inside the parts (the deepest), negative outside them all. */
export const insetParts = (p: Pt, parts: Pt[][]) => parts.reduce((m, q) => Math.max(m, insetIn(p, q)), -Infinity);

/** A polygon cut to the parts: its piece in each. */
const clipParts = (poly: Pt[], parts: Pt[][]) => parts.map((q) => clipConvex(poly, q)).filter((p) => p.length);

/** The stretches of segment a-b inside the parts (merged), as parameters. */
function spansIn(a: Pt, b: Pt, parts: Pt[][]): [number, number][] {
  const spans = parts.map((q) => segInside(a, b, q)).filter((s): s is [number, number] => !!s);
  return merge(spans);
}

function merge(spans: [number, number][]): [number, number][] {
  spans.sort((x, y) => x[0] - y[0]);
  const out: [number, number][] = [];
  for (const s of spans) {
    const l = out[out.length - 1];
    if (l && s[0] <= l[1] + 1e-6) l[1] = Math.max(l[1], s[1]);
    else out.push([s[0], s[1]]);
  }
  return out;
}

/** Where a line from `p` along unit `dir` crosses out of the parts: the far end of the stretch
 * inside them it is on (else the nearest one behind it), as the distance along `dir` (maybe
 * negative), with the outer wall's direction there; null if the line misses them. */
export function exitParts(p: Pt, dir: Pt, parts: Pt[][]): { t: number; tangent: Pt } | null {
  const far = 4;
  const a: Pt = [p[0] - dir[0] * far, p[1] - dir[1] * far];
  const b: Pt = [p[0] + dir[0] * far, p[1] + dir[1] * far];
  const spans = spansIn(a, b, parts).map(([s, e]): [number, number] => [(s - 0.5) * 2 * far, (e - 0.5) * 2 * far]);
  if (!spans.length) return null;
  const on = spans.find(([s, e]) => s <= 1e-6 && e >= -1e-6) ?? spans.filter(([, e]) => e < 0).pop() ?? spans[0];
  const t = on[1];
  const at: Pt = [p[0] + dir[0] * t, p[1] + dir[1] * t];
  const part = parts.reduce((best, q) => (Math.abs(insetIn(at, q)) < Math.abs(insetIn(at, best)) ? q : best), parts[0]);
  return { t, tangent: edgeAt(at, part) };
}

/** Where a door or window on square edge `mid` (out along unit `dir`) stands on an outer wall of
 * parts: where the line out crosses it, or (when that runs along the wall, far off) the nearest
 * point of the wall; as the point, the distance along `dir` to it, and the wall's direction. */
export function onWall(mid: Pt, dir: Pt, parts: Pt[][]): { at: Pt; t: number; tangent: Pt } | null {
  const hit = exitParts(mid, dir, parts);
  if (hit && Math.abs(hit.t) <= 0.75) return { at: [mid[0] + dir[0] * hit.t, mid[1] + dir[1] * hit.t], t: hit.t, tangent: hit.tangent };
  let best: { at: Pt; t: number; tangent: Pt } | null = null;
  let bestD = Infinity;
  for (const part of parts) {
    for (let k = 0; k < part.length; k++) {
      const a = part[k];
      const b = part[(k + 1) % part.length];
      const [ex, ey] = [b[0] - a[0], b[1] - a[1]];
      const l2 = ex * ex + ey * ey;
      if (l2 < 1e-12) continue;
      const u = Math.max(0, Math.min(1, ((mid[0] - a[0]) * ex + (mid[1] - a[1]) * ey) / l2));
      const at: Pt = [a[0] + ex * u, a[1] + ey * u];
      // (Not on a stretch inside another part.)
      if (parts.some((o) => o !== part && insetIn(at, o) > 1e-4)) continue;
      const d = Math.hypot(at[0] - mid[0], at[1] - mid[1]);
      if (d < bestD) {
        bestD = d;
        best = { at, t: (at[0] - mid[0]) * dir[0] + (at[1] - mid[1]) * dir[1], tangent: edgeAt(at, part) };
      }
    }
  }
  return best;
}

/** The part of the outer wall nearest `p`. */
const nearestPart = (p: Pt, parts: Pt[][]) => parts.reduce((best, q) => (Math.abs(insetIn(p, q)) < Math.abs(insetIn(p, best)) ? q : best), parts[0]);

/** The wall's outward normal (unit) at a point where its direction is `t`, on the side of `out`. */
function normalOut(t: Pt, out: Pt): Pt {
  const n: Pt = [t[1], -t[0]];
  return n[0] * out[0] + n[1] * out[1] < 0 ? [-n[0], -n[1]] : n;
}

/** How far in from the outer wall at `at` (square to it, along `-n`) a shaped room's wall is
 * (else along the line out from the door or window's edge `mid`, `dir`); null if it misses it. */
function depthTo(poly: Pt[], at: Pt, n: Pt, mid: Pt, dir: Pt): number | null {
  const across = exitPoint(at, n, poly) ?? exitPoint(mid, dir, poly);
  return across ? Math.max(0, (at[0] - across.at[0]) * n[0] + (at[1] - across.at[1]) * n[1]) : null;
}

/** Segment a-b's end, brought back to where it leaves convex `poly` (b if it stays in). */
function upTo(a: Pt, b: Pt, poly: Pt[]): Pt {
  const span = segInside(a, b, poly);
  return span && span[1] < 1 - 1e-9 ? [a[0] + (b[0] - a[0]) * span[1], a[1] + (b[1] - a[1]) * span[1]] : b;
}

/** A straight piece in the outer wall (a door, a pane: `half` long each way along `t` from `at`,
 * `thick` each side) moved in, square to the wall, as far as it needs to stay off the ground
 * outside (a straight piece across a corner would reach past the wall's outer face). */
export function within(at: Pt, t: Pt, half: number, thick: number, parts: Pt[][]): Pt {
  const n: Pt = [-t[1], t[0]];
  let p = at;
  // (A few steps: at a corner a step in comes partly along the walls.)
  for (let k = 0; k < 6; k++) {
    const ends = [-1, 1].flatMap((s) => [-1, 1].map((u): Pt => [p[0] + t[0] * half * s + n[0] * thick * u, p[1] + t[1] * half * s + n[1] * thick * u]));
    const short = -WALL_HALF - Math.min(...ends.map((q) => insetParts(q, parts)));
    if (short <= 1e-4) break;
    p = [p[0] + n[0] * (short + 1e-3), p[1] + n[1] * (short + 1e-3)];
  }
  return p;
}

/** Polygon `subject` with the convex polygon `cut` taken out: convex pieces. */
export function subtractConvex(subject: Pt[], cut: Pt[]): Pt[][] {
  const out: Pt[][] = [];
  let rest = subject;
  for (let k = 0; k < cut.length && rest.length; k++) {
    const a = cut[k];
    const b = cut[(k + 1) % cut.length];
    const piece = clipHalf(rest, b, a);
    if (piece.length) out.push(piece);
    rest = clipHalf(rest, a, b);
  }
  return out;
}

/** The part of a polygon on the inner side of the line a→b (left of travel in axes with y up:
 * inside a polygon wound as these are). */
function clipHalf(poly: Pt[], a: Pt, b: Pt): Pt[] {
  const side = (p: Pt) => (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
  const out: Pt[] = [];
  for (let i = 0; i < poly.length; i++) {
    const p = poly[i];
    const q = poly[(i + 1) % poly.length];
    const [sp, sq] = [side(p), side(q)];
    if (sp >= 0) out.push(p);
    if (sp >= 0 !== sq >= 0) {
      const t = sp / (sp - sq);
      out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
    }
  }
  return out.length >= 3 && Math.abs(area2(out)) > 1e-9 ? out : [];
}

/** A part's edges on the outer wall (not running inside another part), each with its outward
 * normal (unit). */
function wallEdges(part: Pt[], parts: Pt[][]): { a: Pt; b: Pt; n: Pt }[] {
  const out: { a: Pt; b: Pt; n: Pt }[] = [];
  for (let k = 0; k < part.length; k++) {
    const a = part[k];
    const b = part[(k + 1) % part.length];
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (l < 1e-9) continue;
    const mid: Pt = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    if (parts.some((o) => o !== part && insetIn(mid, o) > 1e-4)) continue;
    out.push({ a, b, n: [(b[1] - a[1]) / l, -(b[0] - a[0]) / l] });
  }
  return out;
}

/** A part moved `d` out (negative: in) along its outer wall's edges: `-WALL_HALF` gives the outer
 * wall's inner face, `WALL_HALF` its outer face. */
function offsetPart(part: Pt[], parts: Pt[][], d: number): Pt[] {
  const xs = part.map((q) => q[0]);
  const ys = part.map((q) => q[1]);
  const m = 2 * Math.abs(d);
  // (A box round it cut by its edges' lines, the outer wall's moved: its seams stay put.)
  let p: Pt[] = [[Math.min(...xs) - m, Math.min(...ys) - m], [Math.max(...xs) + m, Math.min(...ys) - m], [Math.max(...xs) + m, Math.max(...ys) + m], [Math.min(...xs) - m, Math.max(...ys) + m]];
  const walls = new Set(wallEdges(part, parts).map((e) => e.a));
  for (let k = 0; k < part.length && p.length; k++) {
    const a = part[k];
    const b = part[(k + 1) % part.length];
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (l < 1e-9) continue;
    const s = walls.has(a) ? d : 0;
    const n: Pt = [(b[1] - a[1]) / l, -(b[0] - a[0]) / l];
    p = clipHalf(p, [a[0] + n[0] * s, a[1] + n[1] * s], [b[0] + n[0] * s, b[1] + n[1] * s]);
  }
  return p;
}

/** A footprint near enough round (as `interior::round_ish`: convex, eight corners or more, their
 * distances from their mean within 15%). */
function roundIsh(p: Pt[]): boolean {
  if (p.length < 8) return false;
  const c: Pt = [p.reduce((s, q) => s + q[0], 0) / p.length, p.reduce((s, q) => s + q[1], 0) / p.length];
  const r = p.map((q) => Math.hypot(q[0] - c[0], q[1] - c[1]));
  const hi = Math.max(...r);
  return hi > 0 && hi - Math.min(...r) <= 0.15 * hi;
}

// ---------------------------------------------------------------------------------------
// Buildings: a level drawn with its shaped rooms inside its outer wall as drawn.

/** Drawn as solid wall: squares of the footprint no room has (the corners of a round room fitted
 * into a square plan) and what lies between a shaped room and the outer wall. */
export const MASS = -2;
/** Half the outer wall's width as drawn (`interior::WALL_HALF`). */
export const WALL_HALF = 0.225;
/** How far in (a square's middle from the outer wall) a square that is floor on no level still
 * shows the room beside it: the fringe the drawn wall takes in, not a hole in the plan. */
const FRINGE = WALL_HALF + 0.5;

export interface ShapedPlan {
  nx: number;
  ny: number;
  /** The outer wall as drawn: the building's outline (convex parts), else the grid's rectangle. */
  parts: Pt[][];
  /** The building has an outline of its own; without one its outer wall stands on the squares. */
  real: boolean;
  /** What each square shows, on the grid with a ring of squares round it ((nx + 2) x (ny + 2);
   * square (i, j) at (j + 1) * (nx + 2) + i + 1): a room (a square outside the footprint but
   * inside the outer wall shows the room beside it), `MASS`, or -1 (nothing). */
  owner: Int32Array;
  /** A shaped room's square: what shows in it outside the shape (the room across its door, else
   * one beside it, else `MASS`); -1 for the rest. */
  under: Int32Array;
  /** By room: its shape, cut to the outer wall (null for square rooms). */
  polys: (Pt[] | null)[];
}

/** A door as drawn: its leaf's closed line (`a`-`b`); off its square edge (`moved`) when it stands
 * on a shaped room's wall or an outer wall drawn off the squares. A door out has a gap in the outer
 * wall at `outer`; one through a shaped room's wall a gap there at `inner`, with the floor through
 * the wall between (`threshold`, in its room's colour, `room`). */
export interface DrawnDoor {
  a: Pt;
  b: Pt;
  rooms: [number, number];
  /** A door to the outside. */
  out: boolean;
  moved: boolean;
  outer?: Pt;
  inner?: Pt;
  threshold?: Pt[];
  room?: number;
}

/** A window as drawn: a pane in the outer wall at `at` along `tangent`; through a wall at least
 * half a square thick (a shaped room's), an embrasure (`splay`) opening from `inner`. */
export interface DrawnWindow {
  at: Pt;
  tangent: Pt;
  room: number;
  inner?: Pt;
  splay?: Pt[];
}

const N4: Pt[] = [[-1, 0], [0, -1], [1, 0], [0, 1]];
/** Ways about a floor that run round an open height (`shapedPlan`'s ring). */
const GALLERIES = new Set(['corridor', "minstrels' gallery", 'mural passage']);
const N8: Pt[] = [...N4, [-1, -1], [1, -1], [1, 1], [-1, 1]];
const flat = (p: Pt[]) => p.flatMap((q) => q);
const squareAt = (i: number, j: number): Pt[] => [[i, j], [i + 1, j], [i + 1, j + 1], [i, j + 1]];

/** The squares that are floor on some level of a building (its footprint as its squares). */
export function footOf(levels: InteriorLevel[], n: number): Uint8Array {
  const foot = new Uint8Array(n);
  for (const lv of levels) for (let k = 0; k < n; k++) if (lv.cells[k] >= 0) foot[k] = 1;
  return foot;
}

/** Room `ri`'s shape polygon from the box round its squares (dev builds check it against the
 * squares the generator gave it: their middles inside, the box's others outside). */
function roomPolygon(lv: InteriorLevel, nx: number, ri: number, box: Box): Pt[] | null {
  const shape = lv.rooms[ri].shape!;
  const p = shapePolygon(box, shape);
  if (p && import.meta.env?.DEV) {
    for (let j = box[1]; j < box[3]; j++) {
      for (let i = box[0]; i < box[2]; i++) {
        const d = insetIn([i + 0.5, j + 0.5], p);
        if (Math.abs(d) > 0.03 && d > 0 !== (lv.cells[j * nx + i] === ri)) console.warn(`[roomShapes] the ${lv.rooms[ri].kind}'s ${SHAPE_NAMES[shape]} drawing disagrees with its squares at ${i},${j}`);
      }
    }
  }
  return p;
}

/** The convex hull of convex polygons `ps` (wound as they are), where they cover it (every point of it,
 * a tenth of a square apart, in one of them): their union is convex; else null. */
function convexWhole(ps: Pt[][]): Pt[] | null {
  const pts = ps.flat().slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  if (pts.length < 3) return null;
  const cross = (o: Pt, a: Pt, b: Pt) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const half = (list: Pt[]) => {
    const h: Pt[] = [];
    for (const q of list) {
      while (h.length >= 2 && cross(h[h.length - 2], h[h.length - 1], q) <= 1e-9) h.pop();
      h.push(q);
    }
    h.pop();
    return h;
  };
  const hull = wound([...half(pts), ...half(pts.slice().reverse())]);
  if (hull.length < 3) return null;
  const xs = hull.map((q) => q[0]);
  const ys = hull.map((q) => q[1]);
  for (let y = Math.min(...ys); y <= Math.max(...ys); y += 0.1) {
    for (let x = Math.min(...xs); x <= Math.max(...xs); x += 0.1) {
      if (insetIn([x, y], hull) > 0.02 && !ps.some((q) => insetIn([x, y], q) >= -0.02)) return null;
    }
  }
  return hull;
}

/** A building level's drawing plan; null when it is drawn on its squares as they are (a
 * footprint that is the grid's rectangle, no shaped rooms). `foot`: the squares that are floor on
 * some level (`footOf`): without an outline, only those are solid wall where no room is. */
export function shapedPlan(outlines: Pt[][] | undefined, lv: InteriorLevel, nx: number, ny: number, foot: Uint8Array): ShapedPlan | null {
  const real = !!outlines?.length;
  if (!lv.rooms.some((r) => geometric(r.shape)) && !real) return null;
  const parts: Pt[][] = real ? outlines!.map((o) => o.map((p) => [p[0], p[1]] as Pt)) : [[[0, 0], [nx, 0], [nx, ny], [0, ny]]];
  const W = nx + 2;
  const at = (i: number, j: number) => (j + 1) * W + i + 1;
  const cell = (i: number, j: number) => (i >= 0 && j >= 0 && i < nx && j < ny ? lv.cells[j * nx + i] : -1);
  const boxes = roomBoxes(lv, nx, ny);
  // With an outline, a shaped room ends at the outer wall's inner face (a wall round it running
  // under the outer wall would leave steps where it comes out).
  const faces = real ? parts.map((q) => offsetPart(q, parts, -WALL_HALF)) : parts;
  // The whole inner face agrees with room `ri`'s squares: theirs inside it, no other's.
  const fills = (face: Pt[], ri: number) => {
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        const d = insetIn([i + 0.5, j + 0.5], face);
        if (lv.cells[j * nx + i] === ri ? d <= 0 : d > 0.03) return false;
      }
    }
    return true;
  };
  const polys = lv.rooms.map((r, ri) => {
    const b = boxes[ri];
    const p = b && geometric(r.shape) ? roomPolygon(lv, nx, ri, b) : null;
    if (!p) return null;
    // A round room filling a round building (a wizard's tower) is its inner face: the wall an even
    // ring, whatever the grid.
    if (real && r.shape === ROUND && parts.length === 1 && roundIsh(parts[0]) && faces[0].length && fills(faces[0], ri)) return faces[0];
    // (Cut to the part holding most of it; where the parts it lies in make one convex whole (a temple's
    // hall and its apse), to that whole.)
    const pieces = faces.map((q) => clipConvex(p, q)).filter((q) => q.length).sort((x, y) => Math.abs(area2(y)) - Math.abs(area2(x)));
    if (pieces.length > 1) {
      const whole = convexWhole(faces.filter((q) => clipConvex(p, q).length));
      const cut = whole ? clipConvex(p, whole) : [];
      if (cut.length) return cut;
    }
    return pieces[0] ?? null;
  });
  // A gallery round a round open height (a shell keep's upper floor): drawn to the circle a square
  // outside the open height's, its wall to the rooms round it along that circle (its squares, and
  // play on them, as they are).
  lv.rooms.forEach((r, ri) => {
    const b = boxes[ri];
    if (r.kind !== OPEN_BELOW || r.shape !== ROUND || !b || !polys[ri]) return;
    const beside = new Map<number, number>();
    for (let j = b[1]; j < b[3]; j++) {
      for (let i = b[0]; i < b[2]; i++) {
        if (cell(i, j) !== ri) continue;
        for (const [di, dj] of N4) {
          const q = cell(i + di, j + dj);
          if (q >= 0 && q !== ri) beside.set(q, (beside.get(q) ?? 0) + 1);
        }
      }
    }
    const ring = [...beside].sort((x, y) => y[1] - x[1] || x[0] - y[0])[0]?.[0];
    if (ring === undefined || !GALLERIES.has(lv.rooms[ring].kind) || polys[ring]) return;
    const disc = shapePolygon([b[0] - 1, b[1] - 1, b[2] + 1, b[3] + 1], ROUND);
    const cut = disc ? faces.map((q) => clipConvex(disc, q)).filter((q) => q.length).sort((x, y) => Math.abs(area2(y)) - Math.abs(area2(x)))[0] : undefined;
    if (cut) polys[ring] = cut;
  });
  const isShaped = (r: number) => r >= 0 && !!polys[r]?.length;
  const owner = new Int32Array(W * (ny + 2)).fill(-1);
  for (let j = -1; j <= ny; j++) {
    for (let i = -1; i <= nx; i++) {
      const r = cell(i, j);
      if (r >= 0) {
        owner[at(i, j)] = r;
        continue;
      }
      const inGrid = i >= 0 && j >= 0 && i < nx && j < ny;
      if (!real) {
        // On the squares: the footprint's squares no room has are solid.
        if (inGrid && foot[j * nx + i]) owner[at(i, j)] = MASS;
        continue;
      }
      if (!clipParts(squareAt(i, j), parts).length) continue;
      // In the footprint: solid wall. Outside it but within the outer wall: the room beside it (a
      // shaped room ends at its shape: wall beyond).
      let o = MASS;
      // (The footprint as its squares: floor on some level. The wall drawn round it may take in
      // the middles of squares beside it, which show the room next to them; a square deeper in,
      // left empty in a design, stays solid.)
      if (!(inGrid && foot[j * nx + i]) && insetParts([i + 0.5, j + 0.5], parts) <= FRINGE) {
        for (const [di, dj] of N8) {
          const q = cell(i + di, j + dj);
          if (q >= 0) {
            o = isShaped(q) ? MASS : q;
            break;
          }
        }
      }
      owner[at(i, j)] = o;
    }
  }
  // A shaped room's squares, outside its shape: the room across its door, else one beside it.
  const under = new Int32Array(nx * ny).fill(-1);
  const doorTo = new Map<number, number>();
  for (const d of lv.doors) {
    // (Not a secret door: its corner shows as the wall it hides in.)
    if (d.kind === 'secret') continue;
    const [p, q] = doorSquares(d);
    const [rp, rq] = [cell(p[0], p[1]), cell(q[0], q[1])];
    if (rp < 0 || rq < 0 || rp === rq) continue;
    if (isShaped(rp) && !isShaped(rq)) doorTo.set(p[1] * nx + p[0], rq);
    if (isShaped(rq) && !isShaped(rp)) doorTo.set(q[1] * nx + q[0], rp);
  }
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const r = cell(i, j);
      if (!isShaped(r)) continue;
      let u = doorTo.get(j * nx + i) ?? MASS;
      for (const [di, dj] of N4) {
        const q = cell(i + di, j + dj);
        if (u !== MASS) break;
        if (q >= 0 && q !== r && !isShaped(q)) u = q;
      }
      under[j * nx + i] = u;
    }
  }
  return { nx, ny, parts, real, owner, under, polys };
}

/** Line segments drawn where they lie inside convex polygons (each with its own), in one stroke. */
function strokeClipped(g: Graphics, lines: { seg: [Pt, Pt]; clip: Pt[][] }[], style: Parameters<Graphics['stroke']>[0]) {
  let any = false;
  for (const { seg, clip } of lines) {
    const [a, b] = seg;
    for (const s of spansIn(a, b, clip)) {
      g.moveTo(a[0] + (b[0] - a[0]) * s[0], a[1] + (b[1] - a[1]) * s[0]).lineTo(a[0] + (b[0] - a[0]) * s[1], a[1] + (b[1] - a[1]) * s[1]);
      any = true;
    }
  }
  if (any) g.stroke(style);
}

/** What a square shows on the floor (a shaped room's square: what lies outside its shape). */
const shownAt = (sp: ShapedPlan, i: number, j: number) =>
  i >= 0 && j >= 0 && i < sp.nx && j < sp.ny && sp.under[j * sp.nx + i] !== -1 ? sp.under[j * sp.nx + i] : sp.owner[(j + 1) * (sp.nx + 2) + i + 1];

/** A building level's floors: each square as its room (cut to the outer wall; a shaped room's
 * squares as `under` has them), the shaped rooms over them, the floors' texture (`texture`: a
 * square's pattern lines in a colour) and the 5-ft grid as far as each square's floor is drawn.
 * The solid wall comes later, over the furniture (`drawMass`). */
export function drawShapedFloors(g: Graphics, sp: ShapedPlan, lv: InteriorLevel, color: (room: number) => number, texture: (i: number, j: number, color: number) => [Pt, Pt][]) {
  const { nx, ny, parts, polys } = sp;
  const lines: { seg: [Pt, Pt]; clip: Pt[][] }[] = [];
  for (let j = -1; j <= ny; j++) {
    for (let i = -1; i <= nx; i++) {
      const o = shownAt(sp, i, j);
      // (A room with no floor of its own, colour -1: what is under it is drawn apart.)
      if (o < 0 || color(o) < 0) continue;
      for (const region of clipParts(squareAt(i, j), parts)) g.poly(flat(region)).fill(color(o));
      for (const seg of texture(i, j, color(o))) lines.push({ seg, clip: parts });
    }
  }
  strokeClipped(g, lines, { width: 0.03, color: 0x000000, alpha: 0.25 });
  const own: { seg: [Pt, Pt]; clip: Pt[][] }[] = [];
  polys.forEach((p, r) => {
    if (!p?.length || color(r) < 0) return;
    g.poly(flat(p)).fill(color(r));
    const xs = p.map((q) => q[0]);
    const ys = p.map((q) => q[1]);
    for (let j = Math.floor(Math.min(...ys)); j < Math.ceil(Math.max(...ys)); j++) {
      for (let i = Math.floor(Math.min(...xs)); i < Math.ceil(Math.max(...xs)); i++) for (const seg of texture(i, j, color(r))) own.push({ seg, clip: [p] });
    }
  });
  strokeClipped(g, own, { width: 0.03, color: 0x000000, alpha: 0.25 });
  const grid: { seg: [Pt, Pt]; clip: Pt[][] }[] = [];
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const r = lv.cells[j * nx + i];
      if (r < 0 || color(r) < 0) continue;
      const clip = polys[r]?.length ? [polys[r]!] : parts;
      const c = squareAt(i, j);
      for (let k = 0; k < 4; k++) grid.push({ seg: [c[k], c[(k + 1) % 4]], clip });
    }
  }
  strokeClipped(g, grid, { width: 0.04, color: 0x000000, alpha: 0.35 });
}

/** The solid wall: the footprint's squares no room has, and the parts of shaped rooms' squares
 * outside their shapes where nothing else shows (over the furniture, as walls are). */
export function drawMass(g: Graphics, sp: ShapedPlan, color: number) {
  const pieces = massPieces(sp);
  for (const piece of pieces) g.poly(flat(piece));
  if (pieces.length) g.fill(color);
}

/** The solid wall's pieces (convex). */
export function massPieces(sp: ShapedPlan): Pt[][] {
  const { nx, ny, parts, polys } = sp;
  // (The shapes reach past their own squares, up to half a square: none is covered.)
  const shapes = polys.filter((p): p is Pt[] => !!p?.length).map((p) => ({ p, box: [Math.min(...p.map((q) => q[0])), Math.min(...p.map((q) => q[1])), Math.max(...p.map((q) => q[0])), Math.max(...p.map((q) => q[1]))] }));
  const out: Pt[][] = [];
  for (let j = -1; j <= ny; j++) {
    for (let i = -1; i <= nx; i++) {
      if (shownAt(sp, i, j) !== MASS) continue;
      let pieces = clipParts(squareAt(i, j), parts);
      for (const { p, box } of shapes) if (box[0] < i + 1 && box[2] > i && box[1] < j + 1 && box[3] > j) pieces = pieces.flatMap((q) => subtractConvex(q, p));
      out.push(...pieces);
    }
  }
  return out;
}

/** Where a door is drawn: on its square edge, or on the wall as drawn there (a shaped room's; a
 * door out through an outline), with the floor through a shaped room's thick wall to it. */
export function doorSegment(sp: ShapedPlan, lv: InteriorLevel, d: InteriorLevel['doors'][number]): DrawnDoor {
  const { nx, ny } = sp;
  const cell = (p: Pt) => (p[0] >= 0 && p[1] >= 0 && p[0] < nx && p[1] < ny ? lv.cells[p[1] * nx + p[0]] : -1);
  const [p, q] = doorSquares(d);
  const [rp, rq] = [cell(p), cell(q)];
  const mid: Pt = [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
  const pq: Pt = [q[0] - p[0], q[1] - p[1]];
  const half = Math.hypot(d.b[0] - d.a[0], d.b[1] - d.a[1]) * 0.45;
  const base = { rooms: d.rooms, out: rp < 0 || rq < 0, a: d.a, b: d.b, moved: false };
  const along = (at: Pt, t: Pt): { a: Pt; b: Pt } => ({ a: [at[0] - t[0] * half, at[1] - t[1] * half], b: [at[0] + t[0] * half, at[1] + t[1] * half] });
  if (base.out) {
    // Out through the outer wall, from the room's square: its middle `outerT` along `out`.
    const room = rp >= 0 ? rp : rq;
    const out: Pt = rp >= 0 ? pq : [-pq[0], -pq[1]];
    const edge: Pt = [Math.abs(out[1]), Math.abs(out[0])];
    const hit = sp.real ? onWall(mid, out, sp.parts) : { at: mid, t: 0, tangent: edge };
    if (!hit) return base;
    const T = hit.tangent;
    const outer = hit.at;
    const n = normalOut(T, out);
    const depth = sp.polys[room]?.length ? depthTo(sp.polys[room]!, outer, n, mid, out) : null;
    // (With an outline a shaped room ends at the outer wall's inner face or short of it: always
    // through the wall to it.)
    if (depth !== null && (sp.real || depth > WALL_HALF + 0.02)) {
      // Through a thick wall: the floor from the shaped room's wall to the outer face, square to
      // the wall (kept off the ground outside), the leaf on the inner face swinging in.
      const inner: Pt = [outer[0] - n[0] * depth, outer[1] - n[1] * depth];
      const outerFace: Pt = [outer[0] + n[0] * WALL_HALF, outer[1] + n[1] * WALL_HALF];
      const s = along(inner, T);
      const f = along(outerFace, T);
      if (sp.real) {
        // (Across a corner the outer face turns: the sides end where they reach it.)
        const face = offsetPart(nearestPart(outer, sp.parts), sp.parts, WALL_HALF);
        [f.a, f.b] = [upTo(s.a, f.a, face), upTo(s.b, f.b, face)];
      }
      return { ...base, ...s, moved: true, outer, inner, threshold: [s.a, s.b, f.b, f.a], room };
    }
    if (Math.hypot(outer[0] - mid[0], outer[1] - mid[1]) < 1e-6) return { ...base, outer };
    return { ...base, ...along(within(outer, T, half, 0.06, sp.parts), T), moved: true, outer };
  }
  const shaped = onShape(sp.polys, lv, d, nx, ny);
  return shaped ? { ...base, ...shaped, moved: true, inner: [(shaped.a[0] + shaped.b[0]) / 2, (shaped.a[1] + shaped.b[1]) / 2] } : base;
}

/** A door between two rooms, one of them shaped: on that room's wall, where the line in from the
 * door's edge meets it; null if neither is shaped. */
export function onShape(polys: (Pt[] | null)[], lv: InteriorLevel, d: { a: [number, number]; b: [number, number] }, nx: number, ny: number): { a: Pt; b: Pt } | null {
  const cell = (p: Pt) => (p[0] >= 0 && p[1] >= 0 && p[0] < nx && p[1] < ny ? lv.cells[p[1] * nx + p[0]] : -1);
  const [p, q] = doorSquares(d);
  const [rp, rq] = [cell(p), cell(q)];
  if (rp < 0 || rq < 0) return null;
  const mid: Pt = [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
  const pq: Pt = [q[0] - p[0], q[1] - p[1]];
  const half = Math.hypot(d.b[0] - d.a[0], d.b[1] - d.a[1]) * 0.45;
  for (const [r, into] of [[rq, pq], [rp, [-pq[0], -pq[1]]]] as [number, Pt][]) {
    const poly = polys[r];
    const hit = poly?.length ? entryPoint(mid, into, poly) : null;
    if (hit) return { a: [hit.at[0] - hit.tangent[0] * half, hit.at[1] - hit.tangent[1] * half], b: [hit.at[0] + hit.tangent[0] * half, hit.at[1] + hit.tangent[1] * half] };
  }
  return null;
}

/** A level's windows as drawn: panes in the outer wall as drawn; through a wall at least half a
 * square thick (a shaped room's), embrasures from its inner face. */
export function drawnWindows(sp: ShapedPlan, lv: InteriorLevel): DrawnWindow[] {
  const { nx, ny } = sp;
  const cellAt = (c: Pt) => (c[0] >= 0 && c[1] >= 0 && c[0] < nx && c[1] < ny ? lv.cells[c[1] * nx + c[0]] : -1);
  const out: DrawnWindow[] = [];
  for (const [x0, y0, x1, y1] of lv.windows) {
    const [p, q] = doorSquares({ a: [x0, y0], b: [x1, y1] });
    const [room, dir] = cellAt(p) >= 0 ? [cellAt(p), [q[0] - p[0], q[1] - p[1]] as Pt] : [cellAt(q), [p[0] - q[0], p[1] - q[1]] as Pt];
    if (room < 0) continue;
    const mid: Pt = [(x0 + x1) / 2, (y0 + y1) / 2];
    const hit = sp.real ? onWall(mid, dir, sp.parts) : { at: mid, t: 0, tangent: [Math.abs(dir[1]), Math.abs(dir[0])] as Pt };
    if (!hit) continue;
    const at = hit.at;
    const T = hit.tangent;
    const n = normalOut(T, dir);
    const poly = sp.polys[room];
    const depth = Math.max(WALL_HALF, (poly?.length ? depthTo(poly, at, n, mid, dir) : null) ?? WALL_HALF);
    // (Thick enough for an embrasure: from the inner face to the outer one, half a square or more.)
    if (depth + WALL_HALF < 0.5) {
      out.push({ at: within(at, T, 0.3, 0.09, sp.parts), tangent: T, room });
      continue;
    }
    // (Square to the wall, from the shaped room's wall to the outer wall's inner face.)
    const inner: Pt = [at[0] - n[0] * depth, at[1] - n[1] * depth];
    const face: Pt = [at[0] - n[0] * WALL_HALF, at[1] - n[1] * WALL_HALF];
    const splay: Pt[] = [
      [inner[0] - T[0] * 0.32, inner[1] - T[1] * 0.32],
      [inner[0] + T[0] * 0.32, inner[1] + T[1] * 0.32],
      [face[0] + T[0] * 0.16, face[1] + T[1] * 0.16],
      [face[0] - T[0] * 0.16, face[1] - T[1] * 0.16],
    ];
    out.push({ at, tangent: T, room, inner, splay });
  }
  return out;
}

/** A building level's walls as drawn: between rooms on the squares (cut to the outer wall, clear
 * of the shaped rooms), round each shaped room (open at its doors and embrasures) and, with an
 * outline, the outer wall (open at the doors out), `interior` and `exterior` wide. Returns the
 * outer wall's pieces as drawn (a parapet's merlons stand on them; none without an outline). */
export function drawShapedWalls(g: Graphics, sp: ShapedPlan, lv: InteriorLevel, doors: DrawnDoor[], windows: DrawnWindow[], interior: number, exterior: number, color: number): { a: Pt; b: Pt; out: Pt }[] {
  const { nx, ny, owner, polys, parts } = sp;
  const W = nx + 2;
  const own = (i: number, j: number) => owner[(j + 1) * W + i + 1];
  const shapedSq = (i: number, j: number) => i >= 0 && j >= 0 && i < nx && j < ny && lv.cells[j * nx + i] >= 0 && !!polys[lv.cells[j * nx + i]]?.length;
  const doorEdge = new Set(lv.doors.map((d) => `${Math.min(d.a[0], d.b[0])},${Math.min(d.a[1], d.b[1])},${d.a[0] === d.b[0] ? 'v' : 'h'}`));
  // (A railing over an open hall is drawn apart, not as a wall.)
  for (const w of lv.walls) {
    if (!w.rail) continue;
    const vert = w.a[0] === w.b[0];
    const [lo, hi] = vert ? [Math.min(w.a[1], w.b[1]), Math.max(w.a[1], w.b[1])] : [Math.min(w.a[0], w.b[0]), Math.max(w.a[0], w.b[0])];
    for (let t = lo; t < hi; t++) doorEdge.add(vert ? `${w.a[0]},${t},v` : `${t},${w.a[1]},h`);
  }
  const cut = polys.filter((p): p is Pt[] => !!p?.length);
  // (A roof's slopes inside a parapet walk end at their eaves, not a wall.)
  const eave = (o: number) => o >= 0 && lv.rooms[o]?.kind === ROOF_SLOPE;
  // Unit edges between two rooms' squares (the ring's too: the rooms reach the outer wall there),
  // not a shaped room's, not a doorway, not a roof's; in runs along each line.
  const runs: [Pt, Pt][] = [];
  for (const vertical of [false, true]) {
    const span = vertical ? ny : nx;
    for (let line = 0; line <= (vertical ? nx : ny); line++) {
      let start: number | null = null;
      for (let t = -1; t <= span + 1; t++) {
        const [a, b]: [Pt, Pt] = vertical ? [[line - 1, t], [line, t]] : [[t, line - 1], [t, line]];
        const key = vertical ? `${line},${t},v` : `${t},${line},h`;
        const wall =
          t <= span &&
          (() => {
            const [oa, ob] = [own(a[0], a[1]), own(b[0], b[1])];
            return oa >= 0 && ob >= 0 && oa !== ob && !eave(oa) && !eave(ob) && !shapedSq(a[0], a[1]) && !shapedSq(b[0], b[1]) && !doorEdge.has(key);
          })();
        if (wall) start ??= t;
        else if (start !== null) {
          runs.push(vertical ? [[line, start], [line, t]] : [[start, line], [t, line]]);
          start = null;
        }
      }
    }
  }
  // (A stroke with nothing new drawn would go round the last shape again: only when there is one.)
  let drawn = 0;
  for (const [a, b] of runs) {
    const at = (t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    for (const s of spansIn(a, b, parts))
      for (const [p, q] of segOutside(at(s[0]), at(s[1]), cut)) {
        g.moveTo(p[0], p[1]).lineTo(q[0], q[1]);
        drawn++;
      }
  }
  if (drawn) g.stroke({ width: interior, color, cap: 'square' });
  const near = (m: Pt, gaps: Pt[]) => gaps.some((c) => Math.hypot(c[0] - m[0], c[1] - m[1]) < 0.45);
  const midOf = (p: { a: Pt; b: Pt }): Pt => [(p.a[0] + p.b[0]) / 2, (p.a[1] + p.b[1]) / 2];
  const polyline = (run: Pt[]) => {
    g.moveTo(run[0][0], run[0][1]);
    for (const q of run.slice(1)) g.lineTo(q[0], q[1]);
    drawn++;
  };
  drawn = 0;
  // Round each shaped room, but where its doors and embrasures go through it and beyond the outer
  // wall (a hall's height left open: its railing is drawn apart).
  polys.forEach((p, r) => {
    if (!p?.length || lv.rooms[r].kind === OPEN_BELOW) return;
    // (Open as wide as the door's leaf, the embrasure's mouth.)
    const gaps: { at: Pt; half: number }[] = [
      ...doors.filter((d) => (d.rooms[0] === r || d.rooms[1] === r) && d.inner).map((d) => ({ at: d.inner!, half: Math.hypot(d.b[0] - d.a[0], d.b[1] - d.a[1]) / 2 })),
      ...windows.filter((w) => w.room === r && w.inner).map((w) => ({ at: w.inner!, half: 0.32 })),
    ];
    const open = (m: Pt) => gaps.some((c) => Math.hypot(c.at[0] - m[0], c.at[1] - m[1]) < c.half);
    const pieces = boundaryPieces(p);
    // (With an outline, all round: a shaped room ends at the outer wall's inner face, its wall
    // there along that face. On the squares, not along the grid's edge: the outer wall stands there.)
    const keep = (i: number) => !open(midOf(pieces[i])) && (sp.real || insetParts(midOf(pieces[i]), parts) > 0.02);
    if (pieces.every((_, i) => keep(i))) {
      g.poly(flat(p), true);
      drawn++;
    } else for (const run of keptRuns(pieces, keep)) polyline(run);
  });
  if (drawn) g.stroke({ width: interior, color, cap: 'butt', join: 'round' });
  if (!sp.real) return [];
  drawn = 0;
  // The outer wall: each part's edges but where they run inside another part, open at the doors out.
  const gaps = doors.filter((d) => d.out && d.outer).map((d) => d.outer!);
  const kept: { a: Pt; b: Pt; out: Pt }[] = [];
  parts.forEach((part, k) => {
    const pieces = boundaryPieces(part);
    const keep = (i: number) => !near(midOf(pieces[i]), gaps) && !parts.some((o, m) => m !== k && insetIn(midOf(pieces[i]), o) > 1e-4);
    const all = pieces.every((_, i) => keep(i));
    // (Whole: drawn closed, its corners joined.)
    if (all) {
      g.poly(flat(part), true);
      drawn++;
    } else for (const run of keptRuns(pieces, keep)) polyline(run);
    kept.push(...pieces.filter((_, i) => keep(i)));
  });
  if (drawn) g.stroke({ width: exterior, color, cap: 'butt', join: 'miter' });
  return kept;
}

/** Floor through a thick wall (a door's threshold, a window's embrasure), with inked sides. */
export function drawOpening(g: Graphics, quad: Pt[], fill: number, ink: number) {
  g.poly(flat(quad)).fill(fill);
  g.moveTo(quad[1][0], quad[1][1]).lineTo(quad[2][0], quad[2][1]).moveTo(quad[3][0], quad[3][1]).lineTo(quad[0][0], quad[0][1]).stroke({ width: 0.08, color: ink, cap: 'butt' });
}

// ---------------------------------------------------------------------------------------
// Underground: which squares the shader draws from a shape.

/** `uCells.g`: no shape here or next to it. */
export const SHAPE_NONE = 0;
/** `uCells.g`: no shape here, one next to it (the shader looks round). */
export const SHAPE_NEAR = 254;
/** Most shaped rooms a level's texture holds (slots 1-253). */
export const SHAPE_SLOTS = 253;

/** A level's shaped rooms for the underground shader: per square, 0 (drawn from the squares, with
 * no shape next to it), its room's slot + 1 in `shapes` (1-253), 254 (drawn from the squares, a
 * shape next to it: a shaped room's doorways too, so the way in meets it; never a secret door's,
 * which stays hidden in the curve), or 255 (a cavern breaking in); and each slot's box, shape and
 * outline (`polys`, by room). */
export function shapeIndex(lv: InteriorLevel, nx: number, ny: number): { index: Uint8Array; shapes: { box: Box; shape: number; room: number }[]; caverns: boolean; polys: (Pt[] | null)[] } {
  const index = new Uint8Array(nx * ny);
  const shapes: { box: Box; shape: number; room: number }[] = [];
  const polys: (Pt[] | null)[] = lv.rooms.map(() => null);
  if (!lv.rooms.some((r) => r.shape)) return { index, shapes, caverns: false, polys };
  const boxes = roomBoxes(lv, nx, ny);
  const slot = new Map<number, number>();
  lv.rooms.forEach((r, ri) => {
    const b = boxes[ri];
    if (b && geometric(r.shape) && shapes.length < SHAPE_SLOTS) {
      slot.set(ri, shapes.length + 1);
      shapes.push({ box: b, shape: r.shape!, room: ri });
      polys[ri] = roomPolygon(lv, nx, ri, b);
    } else if (b && r.shape === CAVERN) slot.set(ri, CAVERN);
  });
  let caverns = false;
  for (let k = 0; k < nx * ny; k++) {
    const r = lv.cells[k];
    const s = r >= 0 ? slot.get(r) : undefined;
    if (s === undefined) continue;
    index[k] = s;
    caverns ||= s === CAVERN;
  }
  for (const d of lv.doors) {
    if (d.kind === 'secret') continue;
    for (const [i, j] of doorSquares(d)) {
      if (i >= 0 && j >= 0 && i < nx && j < ny && index[j * nx + i] !== CAVERN) index[j * nx + i] = SHAPE_NONE;
    }
  }
  // Squares drawn from the squares beside a shape (or a cavern): the shader looks round only there.
  const near = new Uint8Array(nx * ny);
  for (let k = 0; k < nx * ny; k++) {
    if (index[k] === SHAPE_NONE) continue;
    const [i, j] = [k % nx, Math.floor(k / nx)];
    for (let b = Math.max(0, j - 1); b <= Math.min(ny - 1, j + 1); b++) for (let a = Math.max(0, i - 1); a <= Math.min(nx - 1, i + 1); a++) near[b * nx + a] = 1;
  }
  for (let k = 0; k < nx * ny; k++) if (near[k] && index[k] === SHAPE_NONE) index[k] = SHAPE_NEAR;
  return { index, shapes, caverns, polys };
}

/** Drawn from the squares (no shape, or one next to it). */
export const fromSquares = (g: number) => g === SHAPE_NONE || g === SHAPE_NEAR;

// ---------------------------------------------------------------------------------------
// Per interior and level, worked out once (the renderer and play's door clicks share them).

const plans = new WeakMap<Interior, Map<number, { plan: ShapedPlan | null; doors: DrawnDoor[] | null }>>();

function planOf(it: Interior, level: number): { plan: ShapedPlan | null; doors: DrawnDoor[] | null } {
  let byLevel = plans.get(it);
  if (!byLevel) plans.set(it, (byLevel = new Map()));
  let out = byLevel.get(level);
  if (out) return out;
  const lv = it.levels[level];
  const underground = /^[uwk]:/.test(it.id);
  if (underground) {
    // Underground, only a secret door in a shaped room's wall is drawn off its edge: on the curve
    // (the other doorways are drawn as squares, the way in reaching the shape).
    const { polys } = shapeIndex(lv, it.nx, it.ny);
    const moved = polys.some((p) => p) ? lv.doors.map((d) => (d.kind === 'secret' ? onShape(polys, lv, d, it.nx, it.ny) : null)) : [];
    const doors = moved.some((m) => m) ? lv.doors.map((d, k): DrawnDoor => {
          const m = moved[k];
          return m ? { a: m.a, b: m.b, rooms: d.rooms, out: false, moved: true, inner: [(m.a[0] + m.b[0]) / 2, (m.a[1] + m.b[1]) / 2] } : { a: d.a, b: d.b, rooms: d.rooms, out: false, moved: false };
        }) : null;
    out = { plan: null, doors };
  } else {
    // (A keep's pieces that have ended below this level (a forebuilding, a wing) aren't drawn on it:
    // only the outline parts holding some of its squares no other part holds; a wing overlapping the
    // body by a row has only squares the body holds too once it has ended.)
    let outlines = it.outlines;
    if (it.outlines && it.outlines.length > 1) {
      const parts = it.outlines.map((o) => o.map((p) => [p[0], p[1]] as Pt));
      const holds = new Set<number>();
      for (let k = 0; k < it.nx * it.ny; k++) {
        if (lv.cells[k] < 0) continue;
        const c: Pt = [(k % it.nx) + 0.5, Math.floor(k / it.nx) + 0.5];
        const inside = parts.flatMap((p, n) => (insetIn(c, p) > 0 ? [n] : []));
        if (inside.length === 1) holds.add(inside[0]);
      }
      outlines = it.outlines.filter((_, p) => holds.has(p));
    }
    const plan = shapedPlan(outlines, lv, it.nx, it.ny, footOf(it.levels, it.nx * it.ny));
    out = { plan, doors: plan ? lv.doors.map((d) => doorSegment(plan, lv, d)) : null };
  }
  byLevel.set(level, out);
  return out;
}

/** A building level's drawing plan (`shapedPlan`), null when drawn on its squares as they are. */
export const shapedPlanOf = (it: Interior, level: number) => planOf(it, level).plan;

/** A level's doors as drawn (`DrawnDoor`), by door; null when all stand on their square edges. */
export const drawnDoorsOf = (it: Interior, level: number) => planOf(it, level).doors;

/** An underground level's walls between rooms (masonry), as segments: where two shaped rooms
 * meet, only the stretch along their shapes (not on into the rock round them). */
export function undergroundWalls(lv: InteriorLevel, nx: number, ny: number): [Pt, Pt][] {
  const inner = lv.walls.filter((w) => !w.exterior);
  if (!lv.rooms.some((r) => geometric(r.shape))) return inner.map((w) => [w.a, w.b]);
  const { index, polys } = shapeIndex(lv, nx, ny);
  const shapedAt = (i: number, j: number) => (i >= 0 && j >= 0 && i < nx && j < ny && !fromSquares(index[j * nx + i]) && index[j * nx + i] !== CAVERN ? polys[lv.cells[j * nx + i]] : null);
  const out: [Pt, Pt][] = [];
  for (const w of inner) {
    const vertical = w.a[0] === w.b[0];
    const [s, e] = vertical ? [Math.min(w.a[1], w.b[1]), Math.max(w.a[1], w.b[1])] : [Math.min(w.a[0], w.b[0]), Math.max(w.a[0], w.b[0])];
    for (let t = s; t < e; t++) {
      const [a, b]: [Pt, Pt] = vertical ? [[w.a[0], t], [w.a[0], t + 1]] : [[t, w.a[1]], [t + 1, w.a[1]]];
      const [p, q] = vertical ? [shapedAt(w.a[0] - 1, t), shapedAt(w.a[0], t)] : [shapedAt(t, w.a[1] - 1), shapedAt(t, w.a[1])];
      if (!p || !q) {
        out.push([a, b]);
        continue;
      }
      for (const [u, v] of spansIn(a, b, [p, q])) out.push([[a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u], [a[0] + (b[0] - a[0]) * v, a[1] + (b[1] - a[1]) * v]]);
    }
  }
  return out;
}
