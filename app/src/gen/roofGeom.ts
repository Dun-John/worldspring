// Roof geometry shared by the battlemap's roofs (`battlePrep.ts`) and the roof QA script
// (`scripts/roofgeom.mjs`): footprints as convex parts, a roof's frame, where its domes go.
import earcut from 'earcut';

/** The way a polygon's longest edge runs (unit). */
export function longestDir(pts: number[]): [number, number] {
  const m = pts.length / 2;
  let [ux, uy, best] = [1, 0, 0];
  for (let i = 0; i < m; i++) {
    const dx = pts[((i + 1) % m) * 2] - pts[i * 2];
    const dy = pts[((i + 1) % m) * 2 + 1] - pts[i * 2 + 1];
    const l = Math.hypot(dx, dy);
    if (l > best) [ux, uy, best] = [dx / l, dy / l, l];
  }
  return [ux, uy];
}

export function signedArea(pts: number[]): number {
  let a = 0;
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) a += pts[i * 2] * pts[((i + 1) % m) * 2 + 1] - pts[((i + 1) % m) * 2] * pts[i * 2 + 1];
  return a / 2;
}

/** Whether a polygon turns one way only (corners straight within about a degree allowed). */
export function isConvex(pts: number[]): boolean {
  const m = pts.length / 2;
  let sign = 0;
  for (let i = 0; i < m; i++) {
    const [ax, ay] = [pts[i * 2], pts[i * 2 + 1]];
    const [bx, by] = [pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const [qx, qy] = [pts[((i + 2) % m) * 2], pts[((i + 2) % m) * 2 + 1]];
    const c = (bx - ax) * (qy - by) - (by - ay) * (qx - bx);
    if (Math.abs(c) <= STRAIGHT * Math.hypot(bx - ax, by - ay) * Math.hypot(qx - bx, qy - by)) continue;
    if (sign && Math.sign(c) !== sign) return false;
    sign = Math.sign(c);
  }
  return true;
}

/** A simple polygon as convex parts (Hertel–Mehlhorn: its triangles, merged across shared
 * edges while the union stays convex). */
export function convexParts(pts: number[]): number[][] {
  if (isConvex(pts)) return [pts];
  const tri = earcut(pts);
  const ccw = signedArea(pts) > 0;
  const at = (ids: number[]) => ids.flatMap((k) => [pts[k * 2], pts[k * 2 + 1]]);
  // Every part turned the polygon's way, so a shared edge runs opposite ways in the two.
  let parts: number[][] = [];
  for (let t = 0; t < tri.length; t += 3) {
    const ids = [tri[t], tri[t + 1], tri[t + 2]];
    if (signedArea(at(ids)) > 0 !== ccw) ids.reverse();
    parts.push(ids);
  }
  const merge = (a: number[], b: number[]): number[] | null => {
    for (let i = 0; i < a.length; i++) {
      const [p, q] = [a[i], a[(i + 1) % a.length]];
      const j = b.indexOf(q);
      if (j < 0 || b[(j + 1) % b.length] !== p) continue;
      const out: number[] = [];
      for (let k = 0; k < a.length; k++) out.push(a[(i + 1 + k) % a.length]);
      for (let k = 2; k < b.length; k++) out.push(b[(j + k) % b.length]);
      return isConvex(at(out)) ? out : null;
    }
    return null;
  };
  for (let merged = true; merged; ) {
    merged = false;
    for (let i = 0; i < parts.length && !merged; i++) {
      for (let j = i + 1; j < parts.length && !merged; j++) {
        const u = merge(parts[i], parts[j]);
        if (u) {
          parts[i] = u;
          parts.splice(j, 1);
          merged = true;
        }
      }
    }
  }
  return parts.map(at);
}

/** A convex footprint's frame: its ridge's way (unit; `dir`, else along its longest edge), centroid,
 * extent along the ridge about the centroid, the ridge's line through the middle of its width (a point
 * on it level with the centroid) and its half width. */
export function frame(pts: number[], dir?: [number, number]) {
  const m = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
  const [ux, uy] = dir ?? longestDir(pts);
  let [lo, hi, plo, phi] = [Infinity, -Infinity, Infinity, -Infinity];
  for (let i = 0; i < m; i++) {
    const a = (pts[i * 2] - cx) * ux + (pts[i * 2 + 1] - cy) * uy;
    const b = -(pts[i * 2] - cx) * uy + (pts[i * 2 + 1] - cy) * ux;
    [lo, hi, plo, phi] = [Math.min(lo, a), Math.max(hi, a), Math.min(plo, b), Math.max(phi, b)];
  }
  const mid = (plo + phi) / 2;
  return { cx, cy, ux, uy, lo, hi, halfW: (phi - plo) / 2, rx: cx - uy * mid, ry: cy + ux * mid };
}

/** A corner that turns less than this (the sine of its turn) counts as straight. */
const STRAIGHT = 0.02;

/** A footprint without its straight corners (left where a wall was split, or by rounding), which
 * would each send a hip up to the ridge, or split a roof that is all of a piece. */
export function straighten(pts: number[]): number[] {
  let out = pts;
  for (let again = true; again && out.length > 6; ) {
    again = false;
    const m = out.length / 2;
    for (let i = 0; i < m; i++) {
      const [ax, ay] = [out[((i + m - 1) % m) * 2], out[((i + m - 1) % m) * 2 + 1]];
      const [bx, by] = [out[i * 2], out[i * 2 + 1]];
      const [qx, qy] = [out[((i + 1) % m) * 2], out[((i + 1) % m) * 2 + 1]];
      const [l0, l1] = [Math.hypot(bx - ax, by - ay), Math.hypot(qx - bx, qy - by)];
      const c = (bx - ax) * (qy - by) - (by - ay) * (qx - bx);
      if (l0 < 1e-6 || l1 < 1e-6 || (Math.abs(c) <= STRAIGHT * l0 * l1 && (bx - ax) * (qx - bx) + (by - ay) * (qy - by) > 0)) {
        out = out.slice(0, i * 2).concat(out.slice(i * 2 + 2));
        again = true;
        break;
      }
    }
  }
  return out;
}

/** A footprint's roof parts (convex), each roofed apart: one if it is convex but for straight corners. */
export function roofParts(pts: number[]): { pts: number[] }[] {
  return convexParts(straighten(pts)).map((p) => ({ pts: p }));
}

/** A hip roof's ridge ends on a convex part (`frame`'s ridge shortened by its half width at each end;
 * `open`: its far end along the ridge run on by that much past the eave instead, into the roof it
 * meets, valleys from that end's corners), and which end each corner's hip runs to. */
export function ridgeEnds(pts: number[], dir?: [number, number], open = 0) {
  const f = frame(pts, dir);
  const r0 = f.lo + f.halfW;
  const r1 = Math.max(r0, f.hi - f.halfW);
  const r2 = open > 0 ? f.hi + open : r1;
  const ends = [f.rx + f.ux * r0, f.ry + f.uy * r0, f.rx + f.ux * r2, f.ry + f.uy * r2];
  // (Each corner's hip runs to the nearer end of the ridge.)
  const end = (i: number) => ((pts[i * 2] - f.cx) * f.ux + (pts[i * 2 + 1] - f.cy) * f.uy < (r0 + r1) / 2 ? 0 : 2);
  if (open > 0 || (clearance(pts, ends[0], ends[1]) >= 0.5 * f.halfW && clearance(pts, ends[2], ends[3]) >= 0.5 * f.halfW)) return { ends, end, halfW: f.halfW };
  // A footprint far from its frame's rectangle (a triangle, a skewed end), whose ridge would end near
  // or past its eaves: the ridge through its widest middle as far as the roof stays near as high (a
  // point on a triangle: hips to its middle).
  const [wx, wy, wc] = widest(pts);
  const reach = (s: number) => {
    let [a, b] = [0, f.hi - f.lo];
    for (let k = 0; k < 24; k++) {
      const t = (a + b) / 2;
      if (clearance(pts, wx + s * f.ux * t, wy + s * f.uy * t) >= 0.9 * wc) a = t;
      else b = t;
    }
    return a;
  };
  const [t0, t1] = [reach(-1), reach(1)];
  const mid = (t1 - t0) / 2;
  return {
    ends: [wx - f.ux * t0, wy - f.uy * t0, wx + f.ux * t1, wy + f.uy * t1],
    end: (i: number) => ((pts[i * 2] - wx) * f.ux + (pts[i * 2 + 1] - wy) * f.uy < mid ? 0 : 2),
    halfW: f.halfW,
  };
}

/** A hip roof part's ink: its ridge, then a hip from each corner (segments, x0 y0 x1 y1). */
export function hipLines(part: { pts: number[]; dir?: [number, number]; open?: number }): number[][] {
  const { ends, end } = ridgeEnds(part.pts, part.dir, part.open);
  const out = [ends.slice()];
  for (let i = 0; i < part.pts.length / 2; i++) out.push([ends[end(i)], ends[end(i) + 1], part.pts[i * 2], part.pts[i * 2 + 1]]);
  return out;
}

/** The part of a polygon where (x, y)·n <= c (`town::geom::clip_plain`). */
function keep(pts: number[], nx: number, ny: number, c: number): number[] {
  const out: number[] = [];
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const [da, db] = [ax * nx + ay * ny - c, bx * nx + by * ny - c];
    if (da <= 0) out.push(ax, ay);
    if (da <= 0 !== db <= 0) {
      const t = da / (da - db);
      out.push(ax + (bx - ax) * t, ay + (by - ay) * t);
    }
  }
  return out;
}

/** How far a point is from a polygon's nearest side, negative outside it. */
function clearance(pts: number[], x: number, y: number): number {
  const m = pts.length / 2;
  let [d, inside] = [Infinity, false];
  for (let i = 0, j = m - 1; i < m; j = i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[j * 2], pts[j * 2 + 1]];
    if (ay > y !== by > y && x < ((bx - ax) * (y - ay)) / (by - ay) + ax) inside = !inside;
    const [ex, ey] = [bx - ax, by - ay];
    const t = Math.max(0, Math.min(1, ((x - ax) * ex + (y - ay) * ey) / Math.max(ex * ex + ey * ey, 1e-12)));
    const [dx, dy] = [x - ax - ex * t, y - ay - ey * t];
    d = Math.min(d, Math.sqrt(dx * dx + dy * dy));
  }
  return inside ? d : -d;
}

const STEPS = [1, 0, -1, 0, 0, 1, 0, -1, 1, 1, 1, -1, -1, 1, -1, -1];

/** The middle of the biggest circle in a polygon (the nearest such to its corners' mean): x, y and
 * its radius. A coarse grid over it, then steps that halve (`town::roof::widest`). */
function widest(pts: number[]): [number, number, number] {
  const m = pts.length / 2;
  let [cx, cy, x0, y0, x1, y1] = [0, 0, Infinity, Infinity, -Infinity, -Infinity];
  for (let i = 0; i < m; i++) {
    const [x, y] = [pts[i * 2], pts[i * 2 + 1]];
    [cx, cy] = [cx + x / m, cy + y / m];
    [x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x), Math.max(y1, y)];
  }
  const size = Math.max(x1 - x0, y1 - y0);
  const score = (x: number, y: number) => clearance(pts, x, y) - 0.02 * Math.sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
  let [bx, by, bs] = [cx, cy, score(cx, cy)];
  for (let i = 0; i <= 8; i++) {
    for (let j = 0; j <= 8; j++) {
      const [x, y] = [x0 + ((x1 - x0) * i) / 8, y0 + ((y1 - y0) * j) / 8];
      const s = score(x, y);
      if (s > bs) [bx, by, bs] = [x, y, s];
    }
  }
  let step = size / 16;
  for (let it = 0; it < 200 && step > size * 1e-4; it++) {
    let moved = false;
    for (let k = 0; k < 16; k += 2) {
      const [x, y] = [bx + STEPS[k] * step, by + STEPS[k + 1] * step];
      const s = score(x, y);
      if (s > bs) [bx, by, bs, moved] = [x, y, s, true];
    }
    if (!moved) step /= 2;
  }
  return [bx, by, clearance(pts, bx, by)];
}

/** Where a domed roof's domes stand, each (x, y, its drum's radius), all inside the footprint with a
 * foot of roof round them (`ft`: px per ft): one over its widest middle, two along a long one (if
 * each half holds one near as big); none if it can't hold a dome 5 ft across (`town::roof::domes`). */
export function domeSpots(pts: number[], ft: number): [number, number, number][] {
  const { ux, uy, lo, hi, halfW, rx, ry } = frame(pts);
  const len = hi - lo;
  const size = (w: number, c: number) => Math.min(0.448 * w, c - ft);
  const [x, y, c] = widest(pts);
  let out: [number, number, number][] = [[x, y, size(Math.min(len, 2 * halfW), c)]];
  if (len > 1.75 * 2 * halfW) {
    const k = (rx + ux * (lo + hi) * 0.5) * ux + (ry + uy * (lo + hi) * 0.5) * uy;
    const [a, b] = [keep(pts, ux, uy, k), keep(pts, -ux, -uy, -k)];
    if (a.length >= 6 && b.length >= 6) {
      const [ax, ay, ac] = widest(a);
      const [bx, by, bc] = widest(b);
      const two = Math.min(size(Math.min(len * 0.5, 2 * halfW), ac), size(Math.min(len * 0.5, 2 * halfW), bc));
      if (two >= 0.7 * out[0][2]) out = [[ax, ay, two], [bx, by, two]];
    }
  }
  return out[0][2] >= 2.5 * ft ? out : [];
}

/** A round roof's eaves (`town::roof::Shape::Round`) on a convex footprint of one piece: the footprint
 * itself, a ring of `town::roof::ROUND_SIDES` as generated (`town::round_off`); a footprint drawn by
 * hand, or a lot too small for a ring, roofed round over all of it (never walls out past the eaves). */
export function roundOutline(pts: number[]): number[] {
  return pts;
}
