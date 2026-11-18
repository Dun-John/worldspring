// Polygon geometry for the room shapes: clipping convex polygons, segments through them and outer walls of
// convex parts (grid units, polygons wound clockwise on screen). Moved out of `roomShapes.ts`, which re-exports it.

export type Pt = [number, number];
/** A box in grid units: x0, y0, x1, y1. */
export type Box = [number, number, number, number];
/** Twice the signed area (positive: clockwise on screen). */
export function area2(p: Pt[]): number {
  let a = 0;
  for (let i = 0; i < p.length; i++) {
    const [x0, y0] = p[i];
    const [x1, y1] = p[(i + 1) % p.length];
    a += x0 * y1 - x1 * y0;
  }
  return a;
}

export const wound = (p: Pt[]) => (area2(p) < 0 ? p.reverse() : p);

export function dedupe(p: Pt[]): Pt[] {
  const out: Pt[] = [];
  for (const q of p) {
    const l = out[out.length - 1];
    if (!l || Math.abs(l[0] - q[0]) + Math.abs(l[1] - q[1]) > 1e-6) out.push(q);
  }
  while (out.length > 1 && Math.abs(out[0][0] - out[out.length - 1][0]) + Math.abs(out[0][1] - out[out.length - 1][1]) <= 1e-6) out.pop();
  return out;
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
export const clipParts = (poly: Pt[], parts: Pt[][]) => parts.map((q) => clipConvex(poly, q)).filter((p) => p.length);

/** The stretches of segment a-b inside the parts (merged), as parameters. */
export function spansIn(a: Pt, b: Pt, parts: Pt[][]): [number, number][] {
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
export const nearestPart = (p: Pt, parts: Pt[][]) => parts.reduce((best, q) => (Math.abs(insetIn(p, q)) < Math.abs(insetIn(p, best)) ? q : best), parts[0]);

/** The wall's outward normal (unit) at a point where its direction is `t`, on the side of `out`. */
export function normalOut(t: Pt, out: Pt): Pt {
  const n: Pt = [t[1], -t[0]];
  return n[0] * out[0] + n[1] * out[1] < 0 ? [-n[0], -n[1]] : n;
}

/** How far in from the outer wall at `at` (square to it, along `-n`) a shaped room's wall is
 * (else along the line out from the door or window's edge `mid`, `dir`); null if it misses it. */
export function depthTo(poly: Pt[], at: Pt, n: Pt, mid: Pt, dir: Pt): number | null {
  const across = exitPoint(at, n, poly) ?? exitPoint(mid, dir, poly);
  return across ? Math.max(0, (at[0] - across.at[0]) * n[0] + (at[1] - across.at[1]) * n[1]) : null;
}

/** Segment a-b's end, brought back to where it leaves convex `poly` (b if it stays in). */
export function upTo(a: Pt, b: Pt, poly: Pt[]): Pt {
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
export function offsetPart(part: Pt[], parts: Pt[][], d: number): Pt[] {
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
export function roundIsh(p: Pt[]): boolean {
  if (p.length < 8) return false;
  const c: Pt = [p.reduce((s, q) => s + q[0], 0) / p.length, p.reduce((s, q) => s + q[1], 0) / p.length];
  const r = p.map((q) => Math.hypot(q[0] - c[0], q[1] - c[1]));
  const hi = Math.max(...r);
  return hi > 0 && hi - Math.min(...r) <= 0.15 * hi;
}

/** Half the outer wall's width as drawn (`interior::WALL_HALF`). */
export const WALL_HALF = 0.225;
