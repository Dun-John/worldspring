// Auto-trace: a picture of a map turned into sketch strokes, one colour picked per kind. Pure
// (no DOM): runs in the trace worker and in node.
//
// - water: the picked colour (and transparent pixels) is water, the rest land; land masses become
//   closed hard `land` strokes (once any land is drawn, the undrawn map is sea), water enclosed by
//   a kept land mass becomes a `lake` (carved inside the land, exact, with its outlet; a `sea`
//   stroke there would make a sea-level inlet with a natural coast wandering ~35 mi off it);
// - mountain: blobby areas become `massif` outlines, long thin ones `range` lines along their
//   skeleton (branches too), chosen by elongation (spine length / mean width); strength by size;
// - forest: closed `biome` strokes of temperate forest;
// - river: thin lines of the picked colour (thick areas of it are water), gaps bridged, skeleton
//   (Zhang-Suen) split into a main stem and tributaries, each running toward its mouth: the end
//   nearest the sea (else the nearest water, else the picture's edge).
// At most `MAX_STROKES` shapes (the smallest dropped), each simplified (Douglas-Peucker) until the
// whole result fits `POINT_BUDGET`.
import type { Stroke } from '../../gen/protocol';
import { imageToWorld, TRACE_KINDS, type TraceKind, type TraceParams, type TraceRequest, type TraceResult } from './types';

const MI = 5280;
/** The longest side traced (px); larger pictures are box-downsampled first. */
export const MAX_SIDE = 2048;
/** The most points the strokes may hold in all (T0 regenerates from them). */
export const POINT_BUDGET = 3000;
/** The most strokes a trace gives (the smallest shapes beyond it are dropped). */
export const MAX_STROKES = 400;
const FOREST = 'temperate_forest';

type Mask = Uint8Array<ArrayBuffer>;
const clamp = (v: number, a: number, b: number) => (v < a ? a : v > b ? b : v);

// ---- colour -------------------------------------------------------------------------------

/** The picture at working size: each pixel's colour bin (6 bits per channel), -1 if transparent. */
interface Img {
  w: number;
  h: number;
  /** Picture px per working px (the downsampling). */
  f: number;
  keys: Int32Array;
}

function prepare(req: TraceRequest): Img {
  const { w, h, pixels } = req;
  const f = Math.max(1, Math.ceil(Math.max(w, h) / MAX_SIDE));
  const [W, H] = [Math.ceil(w / f), Math.ceil(h / f)];
  const keys = new Int32Array(W * H);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      let r = 0, g = 0, b = 0, a = 0, c = 0;
      for (let yy = y * f; yy < Math.min(h, y * f + f); yy++) {
        for (let xx = x * f; xx < Math.min(w, x * f + f); xx++) {
          const i = (yy * w + xx) * 4;
          r += pixels[i];
          g += pixels[i + 1];
          b += pixels[i + 2];
          a += pixels[i + 3];
          c++;
        }
      }
      keys[y * W + x] = a / c < 128 ? -1 : (((r / c) >> 2) << 12) | (((g / c) >> 2) << 6) | ((b / c) >> 2);
    }
  }
  return { w: W, h: H, f, keys };
}

const lin = (c: number) => ((c /= 255) <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
/** CIE L*a*b* (D65) of an sRGB colour. */
function lab(r: number, g: number, b: number): [number, number, number] {
  const [R, G, B] = [lin(r), lin(g), lin(b)];
  const f = (t: number) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  const fx = f((0.4124 * R + 0.3576 * G + 0.1805 * B) / 0.95047);
  const fy = f(0.2126 * R + 0.7152 * G + 0.0722 * B);
  const fz = f((0.0193 * R + 0.1192 * G + 0.9505 * B) / 1.08883);
  return [116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)];
}

let LAB: Float32Array | null = null;
function labBins(): Float32Array {
  if (LAB) return LAB;
  LAB = new Float32Array(262144 * 3);
  for (let k = 0; k < 262144; k++) {
    const c = lab(((k >> 12) & 63) * 4 + 2, ((k >> 6) & 63) * 4 + 2, (k & 63) * 4 + 2);
    LAB.set(c, k * 3);
  }
  return LAB;
}

/** The colour distance (CIE76 ΔE) a threshold of 0..1 allows: 4 (all but exact) to 60. */
export const deltaE = (threshold: number) => 4 + 56 * clamp(threshold, 0, 1);

/** Per colour bin, for each kind picked: within its threshold of the pick, and no nearer to
 * another kind's pick (so a river's blue close to the sea's isn't read as sea, nor the sea's
 * speckle as river; picks of the same colour both count). */
function hits(req: TraceRequest): Partial<Record<TraceKind, Uint8Array>> {
  const bins = labBins();
  const picked = TRACE_KINDS.filter((k) => req.picks[k]);
  const d2 = picked.map((k) => {
    const [L, A, B] = lab(...req.picks[k]!);
    const d = new Float32Array(262144);
    for (let b = 0; b < 262144; b++) d[b] = (bins[b * 3] - L) ** 2 + (bins[b * 3 + 1] - A) ** 2 + (bins[b * 3 + 2] - B) ** 2;
    return d;
  });
  const out: Partial<Record<TraceKind, Uint8Array>> = {};
  picked.forEach((k, i) => {
    const t2 = deltaE(req.params[k].threshold) ** 2;
    const hit = new Uint8Array(262144);
    for (let b = 0; b < 262144; b++) {
      const d = d2[i][b];
      hit[b] = d <= t2 && d2.every((o) => d <= o[b]) ? 1 : 0;
    }
    out[k] = hit;
  });
  return out;
}

/** The pixels of a kind (transparent ones count as `transparent`). */
function match(c: Ctx, kind: TraceKind, transparent: 0 | 1): Mask {
  const { keys } = c.img;
  const hit = c.hit[kind]!;
  const m = new Uint8Array(keys.length);
  for (let i = 0; i < m.length; i++) {
    const k = keys[i];
    m[i] = k < 0 ? transparent : hit[k];
  }
  return m;
}

// ---- masks --------------------------------------------------------------------------------

const MAJORITY = 0, ERODE = 1, DILATE = 2;
/** A square window of radius `r` over the mask: majority, erosion or dilation (by an integral
 * image; the window is clipped at the picture's edge). */
function box(m: Mask, w: number, h: number, r: number, mode: number): Mask {
  if (r <= 0) return m.slice();
  // Row sums over the window (clipped), then a running sum of them down each column.
  const hs = new Uint16Array(w * h);
  for (let y = 0; y < h; y++) {
    const o = y * w;
    let sum = 0;
    for (let x = 0; x < Math.min(r, w); x++) sum += m[o + x];
    for (let x = 0; x < w; x++) {
      if (x + r < w) sum += m[o + x + r];
      if (x - r - 1 >= 0) sum -= m[o + x - r - 1];
      hs[o + x] = sum;
    }
  }
  const out = new Uint8Array(w * h);
  const col = new Int32Array(w);
  const cols = new Int32Array(w);
  for (let x = 0; x < w; x++) cols[x] = Math.min(w, x + r + 1) - Math.max(0, x - r);
  for (let y = 0; y < Math.min(r, h); y++) for (let x = 0; x < w; x++) col[x] += hs[y * w + x];
  for (let y = 0; y < h; y++) {
    if (y + r < h) for (let x = 0, o = (y + r) * w; x < w; x++) col[x] += hs[o + x];
    if (y - r - 1 >= 0) for (let x = 0, o = (y - r - 1) * w; x < w; x++) col[x] -= hs[o + x];
    const rows = Math.min(h, y + r + 1) - Math.max(0, y - r);
    const o = y * w;
    if (mode === DILATE) for (let x = 0; x < w; x++) out[o + x] = col[x] > 0 ? 1 : 0;
    else if (mode === ERODE) for (let x = 0; x < w; x++) out[o + x] = col[x] === rows * cols[x] ? 1 : 0;
    else for (let x = 0; x < w; x++) out[o + x] = 2 * col[x] > rows * cols[x] ? 1 : 0;
  }
  return out;
}
const open = (m: Mask, w: number, h: number, r: number) => box(box(m, w, h, r, ERODE), w, h, r, DILATE);

interface Comp {
  area: number;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  /** Touches the picture's edge. */
  border: boolean;
  /** Its first pixel in scan order (topmost, then leftmost). */
  first: number;
}

/** Connected components (ids from 1; 0 is background), 8- or 4-connected. */
function label(m: Mask, w: number, h: number, eight: boolean): { lab: Int32Array; comps: Comp[] } {
  const lab = new Int32Array(w * h);
  const comps: Comp[] = [];
  const stack = new Int32Array(w * h);
  for (let s = 0; s < w * h; s++) {
    if (!m[s] || lab[s]) continue;
    const id = comps.length + 1;
    const c: Comp = { area: 0, x0: w, y0: h, x1: 0, y1: 0, border: false, first: s };
    let top = 0;
    stack[top++] = s;
    lab[s] = id;
    while (top) {
      const p = stack[--top];
      const x = p % w;
      const y = (p / w) | 0;
      c.area++;
      if (x < c.x0) c.x0 = x;
      if (x > c.x1) c.x1 = x;
      if (y < c.y0) c.y0 = y;
      if (y > c.y1) c.y1 = y;
      if (x === 0 || y === 0 || x === w - 1 || y === h - 1) c.border = true;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          if ((!dx && !dy) || (!eight && dx && dy)) continue;
          const nx = x + dx;
          const ny = y + dy;
          if (nx < 0 || ny < 0 || nx >= w || ny >= h) continue;
          const q = ny * w + nx;
          if (m[q] && !lab[q]) {
            lab[q] = id;
            stack[top++] = q;
          }
        }
      }
    }
    comps.push(c);
  }
  return { lab, comps };
}

/** Distance (px, chamfer) from every pixel to the nearest set one (Infinity if none). */
function chamfer(m: Mask, w: number, h: number): Float32Array {
  const d = new Float32Array(w * h);
  for (let i = 0; i < d.length; i++) d[i] = m[i] ? 0 : Infinity;
  const D = Math.SQRT2;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      let v = d[i];
      if (x > 0) v = Math.min(v, d[i - 1] + 1);
      if (y > 0) {
        v = Math.min(v, d[i - w] + 1);
        if (x > 0) v = Math.min(v, d[i - w - 1] + D);
        if (x < w - 1) v = Math.min(v, d[i - w + 1] + D);
      }
      d[i] = v;
    }
  }
  for (let y = h - 1; y >= 0; y--) {
    for (let x = w - 1; x >= 0; x--) {
      const i = y * w + x;
      let v = d[i];
      if (x < w - 1) v = Math.min(v, d[i + 1] + 1);
      if (y < h - 1) {
        v = Math.min(v, d[i + w] + 1);
        if (x < w - 1) v = Math.min(v, d[i + w + 1] + D);
        if (x > 0) v = Math.min(v, d[i + w - 1] + D);
      }
      d[i] = v;
    }
  }
  return d;
}

// ---- outlines -----------------------------------------------------------------------------

const DX = [1, 0, -1, 0];
const DY = [0, 1, 0, -1];
const ONE_BIT = [-1, 0, 1, -1, 2, -1, -1, -1, 3];

/** A component's outer outline (marching squares: the midpoints of the edges between its pixels
 * and the rest, clockwise on screen), flat x,y in px; holes are left out. */
function outline(lab: Int32Array, w: number, id: number, c: Comp, eight: boolean): number[] {
  const [bw, bh] = [c.x1 - c.x0 + 1, c.y1 - c.y0 + 1];
  const VW = bw + 1;
  const inn = (i: number, j: number) => i >= 0 && j >= 0 && i < bw && j < bh && lab[(c.y0 + j) * w + c.x0 + i] === id;
  // Boundary edges as bits per pixel corner: 1 right, 2 down, 4 left, 8 up (the inside on
  // their right).
  const bits = new Uint8Array(VW * (bh + 1));
  for (let j = 0; j < bh; j++) {
    for (let i = 0; i < bw; i++) {
      if (!inn(i, j)) continue;
      if (!inn(i, j - 1)) bits[j * VW + i] |= 1;
      if (!inn(i + 1, j)) bits[j * VW + i + 1] |= 2;
      if (!inn(i, j + 1)) bits[(j + 1) * VW + i + 1] |= 4;
      if (!inn(i - 1, j)) bits[(j + 1) * VW + i] |= 8;
    }
  }
  // From the top edge of its first pixel (on the outer boundary, never at a saddle).
  const sv = (c.first % w) - c.x0;
  const pts: number[] = [];
  let [v, d] = [sv, 0];
  for (let guard = bits.length * 4; guard > 0; guard--) {
    const [vx, vy] = [v % VW, (v / VW) | 0];
    pts.push(c.x0 + vx + 0.5 * DX[d], c.y0 + vy + 0.5 * DY[d]);
    v = (vy + DY[d]) * VW + vx + DX[d];
    const b = bits[v];
    // At a saddle, turn left to join diagonal pixels (8-connected), else right.
    const nd = b & (b - 1) ? (eight ? (d + 3) & 3 : (d + 1) & 3) : ONE_BIT[b];
    if (v === sv && nd === 0) break;
    d = nd;
  }
  return pts;
}

// ---- skeletons ----------------------------------------------------------------------------

/** A component cut out with a 1 px empty margin. */
interface Crop {
  m: Mask;
  cw: number;
  ch: number;
  /** Picture px of crop cell (0, 0)'s corner. */
  ox: number;
  oy: number;
  /** Picture px per crop cell (1, or more for a coarse copy). */
  f: number;
}

function crop(lab: Int32Array, w: number, id: number, c: Comp): Crop {
  const [cw, ch] = [c.x1 - c.x0 + 3, c.y1 - c.y0 + 3];
  const m = new Uint8Array(cw * ch);
  for (let y = c.y0; y <= c.y1; y++) for (let x = c.x0; x <= c.x1; x++) if (lab[y * w + x] === id) m[(y - c.y0 + 1) * cw + x - c.x0 + 1] = 1;
  return { m, cw, ch, ox: c.x0 - 1, oy: c.y0 - 1, f: 1 };
}

/** A crop on cells of f x f px (a cell is set if any of its px is): thinning it costs f³ less. */
function coarse(cr: Crop, f: number): Crop {
  if (f <= 1) return cr;
  const [iw, ih] = [cr.cw - 2, cr.ch - 2];
  const [cw, ch] = [Math.ceil(iw / f) + 2, Math.ceil(ih / f) + 2];
  const m = new Uint8Array(cw * ch);
  for (let y = 0; y < ih; y++) {
    const row = ((y / f) | 0) + 1;
    for (let x = 0; x < iw; x++) if (cr.m[(y + 1) * cr.cw + x + 1]) m[row * cw + ((x / f) | 0) + 1] = 1;
  }
  return { m, cw, ch, ox: cr.ox + 1 - f, oy: cr.oy + 1 - f, f };
}

/** The crop's largest inscribed radius (px, chamfer): its half-width at the thickest. */
function maxRadius(cr: Crop): number {
  const inv = new Uint8Array(cr.m.length);
  for (let i = 0; i < inv.length; i++) inv[i] = cr.m[i] ? 0 : 1;
  const d = chamfer(inv, cr.cw, cr.ch);
  let r = 0;
  for (let i = 0; i < d.length; i++) if (cr.m[i] && d[i] > r) r = d[i];
  return r;
}

/** Zhang-Suen thinning (in place) to a one-pixel skeleton. */
function thin(cr: Crop) {
  const { m, cw } = cr;
  let fg: number[] = [];
  for (let i = 0; i < m.length; i++) if (m[i]) fg.push(i);
  const off = [-cw, -cw + 1, 1, cw + 1, cw, cw - 1, -1, -cw - 1];
  const p = new Uint8Array(8);
  for (let changed = true; changed; ) {
    changed = false;
    for (let step = 0; step < 2; step++) {
      const del: number[] = [];
      for (const i of fg) {
        for (let k = 0; k < 8; k++) p[k] = m[i + off[k]];
        const b = p[0] + p[1] + p[2] + p[3] + p[4] + p[5] + p[6] + p[7];
        if (b < 2 || b > 6) continue;
        let a = 0;
        for (let k = 0; k < 8; k++) if (!p[k] && p[(k + 1) & 7]) a++;
        if (a !== 1) continue;
        // p[0] P2 (up), p[2] P4 (right), p[4] P6 (down), p[6] P8 (left).
        if (step === 0 ? p[0] * p[2] * p[4] || p[2] * p[4] * p[6] : p[0] * p[2] * p[6] || p[0] * p[4] * p[6]) continue;
        del.push(i);
      }
      for (const i of del) m[i] = 0;
      if (del.length) {
        changed = true;
        fg = fg.filter((i) => m[i]);
      }
    }
  }
}

const N8 = (cw: number) => [-cw - 1, -cw, -cw + 1, -1, 1, cw - 1, cw, cw + 1];

/** Breadth-first over the skeleton from `root`: each cell's parent (-1 at the root, -2 unreached)
 * and the cells in visiting order. */
function bfs(cr: Crop, root: number): { parent: Int32Array; order: number[] } {
  const { m, cw } = cr;
  const parent = new Int32Array(m.length).fill(-2);
  const order = [root];
  parent[root] = -1;
  const nb = N8(cw);
  for (let h = 0; h < order.length; h++) {
    const p = order[h];
    for (const o of nb) {
      const q = p + o;
      if (m[q] && parent[q] === -2) {
        parent[q] = p;
        order.push(q);
      }
    }
  }
  return { parent, order };
}

/** The skeleton split into paths from `root`: the longest first (from its far end to the root),
 * then each branch from its tip to where it joins one already taken (crop cells). */
function branches(cr: Crop, root: number): number[][] {
  const { parent, order } = bfs(cr, root);
  const depth = new Int32Array(cr.m.length);
  const hasChild = new Uint8Array(cr.m.length);
  for (const p of order) {
    if (parent[p] >= 0) {
      depth[p] = depth[parent[p]] + 1;
      hasChild[parent[p]] = 1;
    }
  }
  const leaves = order.filter((p) => !hasChild[p] && p !== root).sort((a, b) => depth[b] - depth[a]);
  const taken = new Uint8Array(cr.m.length);
  taken[root] = 1;
  const out: number[][] = [];
  for (const leaf of leaves) {
    const path: number[] = [];
    let p = leaf;
    while (!taken[p]) {
      path.push(p);
      taken[p] = 1;
      p = parent[p];
    }
    path.push(p);
    out.push(path);
  }
  if (!out.length) out.push([root]);
  return out;
}

/** Skeleton cells with one neighbour (its ends). */
function ends(cr: Crop): number[] {
  const nb = N8(cr.cw);
  const out: number[] = [];
  for (let i = 0; i < cr.m.length; i++) if (cr.m[i] && nb.reduce((n, o) => n + cr.m[i + o], 0) === 1) out.push(i);
  return out;
}

/** A crop path as flat picture px (pixel centres). */
function toPx(cr: Crop, path: number[]): number[] {
  const out: number[] = [];
  for (const p of path) out.push(cr.ox + ((p % cr.cw) + 0.5) * cr.f, cr.oy + (((p / cr.cw) | 0) + 0.5) * cr.f);
  return out;
}

function length(pts: number[]): number {
  let l = 0;
  for (let i = 2; i < pts.length; i += 2) l += Math.hypot(pts[i] - pts[i - 2], pts[i + 1] - pts[i - 1]);
  return l;
}

// ---- simplification -----------------------------------------------------------------------

function segD2(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const [dx, dy] = [bx - ax, by - ay];
  const l2 = dx * dx + dy * dy;
  const t = l2 > 0 ? clamp(((px - ax) * dx + (py - ay) * dy) / l2, 0, 1) : 0;
  const [ex, ey] = [ax + t * dx - px, ay + t * dy - py];
  return ex * ex + ey * ey;
}

/** Douglas-Peucker over points a..b (indices), marking those kept. */
function dpRange(pts: number[], a: number, b: number, eps2: number, keep: Uint8Array) {
  const stack = [a, b];
  keep[a] = keep[b] = 1;
  while (stack.length) {
    const j = stack.pop()!;
    const i = stack.pop()!;
    let [far, fd] = [-1, eps2];
    for (let k = i + 1; k < j; k++) {
      const d = segD2(pts[2 * k], pts[2 * k + 1], pts[2 * i], pts[2 * i + 1], pts[2 * j], pts[2 * j + 1]);
      if (d > fd) [far, fd] = [k, d];
    }
    if (far >= 0) {
      keep[far] = 1;
      stack.push(i, far, far, j);
    }
  }
}

function simplify(pts: number[], eps: number, closed: boolean): number[] {
  const n = pts.length / 2;
  if (n < 3) return pts.slice();
  const eps2 = eps * eps;
  if (!closed) {
    const keep = new Uint8Array(n);
    dpRange(pts, 0, n - 1, eps2, keep);
    return pts.filter((_, i) => keep[i >> 1]);
  }
  // A loop: split at the point farthest from the first, the first repeated at the end.
  const ext = pts.concat(pts[0], pts[1]);
  let [far, fd] = [1, -1];
  for (let k = 1; k < n; k++) {
    const d = (pts[2 * k] - pts[0]) ** 2 + (pts[2 * k + 1] - pts[1]) ** 2;
    if (d > fd) [far, fd] = [k, d];
  }
  const keep = new Uint8Array(n + 1);
  dpRange(ext, 0, far, eps2, keep);
  dpRange(ext, far, n, eps2, keep);
  return pts.filter((_, i) => keep[i >> 1]);
}

// ---- tracing ------------------------------------------------------------------------------

/** A shape found, in picture px (working resolution), before simplifying. */
interface Raw {
  kind: TraceKind;
  pts: number[];
  closed: boolean;
  /** Simplification tolerance (px) at the budget's first try. */
  eps: number;
  stroke: Omit<Stroke, 'pts'>;
}

interface Ctx {
  img: Img;
  /** Long side (working px) / 1024, at least 1: sizes in px grow with it. */
  s: number;
  /** World ft per working px. */
  ftPx: number;
  /** Each picked kind's colour bins (`hits`). */
  hit: Partial<Record<TraceKind, Uint8Array>>;
  raws: Raw[];
  /** Water as traced (sea and lakes) and the sea alone, when water is picked. */
  water: Mask | null;
  sea: Mask | null;
}

/** The thickest line (half-width, px) still counted a river rather than water. */
const thinK = (c: Ctx) => Math.max(2, Math.round(1.7 * c.s));
const smoothR = (c: Ctx, p: TraceParams) => Math.round((clamp(p.smooth, 0, 1) * c.s * 1024) / 200);
/** Smoothing: lines thinner than about the smoothing radius (lettering, grid lines)
 * opened away, then a majority filter. */
function smoothMask(c: Ctx, m: Mask, p: TraceParams): Mask {
  const { w, h } = c.img;
  const r = smoothR(c, p);
  if (r <= 0) return m;
  return box(open(m, w, h, Math.max(1, Math.round(r / 2))), w, h, r, MAJORITY);
}
const epsPx = (c: Ctx, p: TraceParams) => (0.6 + 2.4 * clamp(p.smooth, 0, 1)) * c.s;
const minArea = (c: Ctx, p: TraceParams) => Math.max(1, p.minSize / (c.img.f * c.img.f));

function traceWater(c: Ctx, p: TraceParams, rivers: boolean) {
  const { w, h } = c.img;
  let m = match(c, 'water', 1);
  // Rivers drawn in the water's colour are not water.
  if (rivers) m = open(m, w, h, thinK(c));
  let land = new Uint8Array(m.length);
  for (let i = 0; i < m.length; i++) land[i] = m[i] ? 0 : 1;
  land = smoothMask(c, land, p);
  for (let i = 0; i < m.length; i++) m[i] = land[i] ? 0 : 1;
  const L = label(land, w, h, true);
  const W = label(m, w, h, false);
  const min = minArea(c, p);
  const eps = epsPx(c, p);
  const kept = L.comps.map((k) => k.area >= min);
  L.comps.forEach((k, i) => {
    if (kept[i]) c.raws.push({ kind: 'water', pts: outline(L.lab, w, i + 1, k, true), closed: true, eps, stroke: { tool: 'land', closed: true, hard: true } });
  });
  const sea = new Uint8Array(m.length);
  const seaId = new Uint8Array(W.comps.length + 1);
  W.comps.forEach((k, i) => {
    if (k.border) {
      seaId[i + 1] = 1;
      return;
    }
    // A lake: inside a kept land mass (the pixel above its first is that land's).
    const around = L.lab[k.first - w];
    if (k.area >= min && around > 0 && kept[around - 1]) {
      c.raws.push({ kind: 'water', pts: outline(W.lab, w, i + 1, k, false), closed: true, eps, stroke: { tool: 'lake', closed: true } });
    }
  });
  for (let i = 0; i < m.length; i++) sea[i] = seaId[W.lab[i]];
  [c.water, c.sea] = [m, sea];
}

/** Mountains or forests: the picked areas, smoothed, specks dropped. */
function areas(c: Ctx, kind: TraceKind, p: TraceParams) {
  const { w, h } = c.img;
  const m = smoothMask(c, match(c, kind, 0), p);
  const L = label(m, w, h, true);
  const min = minArea(c, p);
  return { L, keep: L.comps.map((k, i) => [k, i + 1] as const).filter(([k]) => k.area >= min) };
}

function traceForest(c: Ctx, p: TraceParams) {
  const { L, keep } = areas(c, 'forest', p);
  const eps = epsPx(c, p);
  const radius = Math.round(clamp(4 * eps * c.ftPx, 2 * MI, 20 * MI));
  for (const [k, id] of keep) {
    c.raws.push({ kind: 'forest', pts: outline(L.lab, c.img.w, id, k, true), closed: true, eps, stroke: { tool: 'biome', closed: true, biome: FOREST, radius_ft: radius } });
  }
}

/** How elongated a mountain area must be (spine length / mean width) to be a range. */
const RANGE_ELONGATION = 4;
/** Below this (area / thickest width²) a shape can't reach RANGE_ELONGATION, bar knots: a massif. */
const RANGE_PLAUSIBLE = 0.9;
/** Thinning's budget per shape (area x half-width, px³; ~40 ms): bigger ones are thinned coarser. */
const THIN_WORK = 4e6;

function traceMountains(c: Ctx, p: TraceParams) {
  const { L, keep } = areas(c, 'mountain', p);
  const eps = epsPx(c, p);
  for (const [k, id] of keep) {
    // Decided before thinning (its cost is area x half-width): a shape no longer than
    // RANGE_PLAUSIBLE times its thickest width is a massif; others are thinned on cells of up to
    // a size that bounds the work (THIN_WORK), then judged by their spine.
    let cr = crop(L.lab, c.img.w, id, k);
    const r = Math.max(1, maxRadius(cr));
    const plausible = k.area / (4 * r * r) >= RANGE_PLAUSIBLE;
    let any = -1;
    if (plausible) {
      cr = coarse(cr, Math.max(1, Math.ceil(Math.cbrt((k.area * r) / THIN_WORK))));
      thin(cr);
      any = cr.m.indexOf(1);
    }
    let paths: number[][] = [];
    let spine = 0;
    if (any >= 0) {
      // The spine: from the skeleton's far end (farthest from any of its cells) outward.
      const { order } = bfs(cr, any);
      paths = branches(cr, order[order.length - 1]);
      spine = length(toPx(cr, paths[0]));
    }
    const width = k.area / Math.max(1, spine);
    if (spine / width >= RANGE_ELONGATION) {
      const strength = clamp(0.3 + (0.7 * width * c.ftPx) / (100 * MI), 0.3, 1);
      const radius = Math.round(0.5 * width * c.ftPx);
      paths.forEach((path, i) => {
        const pts = toPx(cr, path);
        if (i === 0 || length(pts) >= 2 * width) c.raws.push({ kind: 'mountain', pts, closed: false, eps: Math.max(eps, 0.75 * cr.f), stroke: { tool: 'range', radius_ft: radius, strength } });
      });
    } else {
      const diam = 2 * Math.sqrt(k.area / Math.PI) * c.ftPx;
      const strength = clamp(0.3 + (0.7 * diam) / (200 * MI), 0.3, 1);
      const foot = Math.round(clamp(0.15 * diam, 2 * MI, 15 * MI));
      c.raws.push({ kind: 'mountain', pts: outline(L.lab, c.img.w, id, k, true), closed: true, eps, stroke: { tool: 'massif', closed: true, radius_ft: foot, strength } });
    }
  }
}

function traceRivers(c: Ctx, p: TraceParams) {
  const { w, h } = c.img;
  const k = thinK(c);
  const m = match(c, 'river', 0);
  // Thin lines only: what survives an opening of the river's colour is water.
  const thick = box(box(m, w, h, k, ERODE), w, h, k + 1, DILATE);
  const wet = c.water ? box(c.water, w, h, 1, DILATE) : null;
  const line = new Uint8Array(m.length);
  for (let i = 0; i < m.length; i++) line[i] = m[i] && !thick[i] && !(wet && wet[i]) ? 1 : 0;
  // Gaps of up to 2g px bridged (dilated, then thinned back to the middle).
  const g = 1 + Math.round(2 * clamp(p.smooth, 0, 1));
  const bridged = box(line, w, h, g, DILATE);
  if (wet) for (let i = 0; i < m.length; i++) if (wet[i]) bridged[i] = 0;
  const L = label(bridged, w, h, true);
  const count = new Int32Array(L.comps.length + 1);
  for (let i = 0; i < m.length; i++) if (line[i]) count[L.lab[i]]++;
  const seaD = c.sea ? chamfer(c.sea, w, h) : null;
  const wetD = c.water ? chamfer(c.water, w, h) : null;
  // A mouth this close to water reaches it.
  const reach = 2 * g + k + 3;
  const spur = 2 * g + 4 + 8 * clamp(p.smooth, 0, 1) * c.s;
  const eps = epsPx(c, p);
  const min = minArea(c, p);
  L.comps.forEach((comp, ci) => {
    if (count[ci + 1] < min) return;
    const cr = crop(L.lab, w, ci + 1, comp);
    thin(cr);
    const at = (q: number) => (cr.oy + ((q / cr.cw) | 0)) * w + cr.ox + (q % cr.cw);
    // The mouth: the end nearest the sea within reach, else the nearest water, else the edge.
    const score = (q: number) => {
      const i = at(q);
      if (seaD && seaD[i] <= reach) return seaD[i];
      if (wetD && wetD[i] < Infinity) return 1e6 + wetD[i];
      const [x, y] = [i % w, (i / w) | 0];
      return 2e6 + Math.min(x, y, w - 1 - x, h - 1 - y);
    };
    let cand = ends(cr);
    if (!cand.length) cand = [...cr.m.keys()].filter((q) => cr.m[q]);
    if (!cand.length) return;
    const mouth = cand.reduce((a, b) => (score(b) < score(a) ? b : a));
    const paths = branches(cr, mouth);
    paths.forEach((path, i) => {
      const pts = toPx(cr, path);
      if (length(pts) < (i === 0 ? 2 * spur : spur)) return;
      if (i === 0 && wetD) {
        // Into the water: on to its nearest pixel.
        const mi = at(mouth);
        if (wetD[mi] <= reach) {
          const [mx, my] = [mi % w, (mi / w) | 0];
          const r = Math.ceil(wetD[mi]) + 1;
          let [best, bd] = [-1, Infinity];
          for (let y = Math.max(0, my - r); y <= Math.min(h - 1, my + r); y++) {
            for (let x = Math.max(0, mx - r); x <= Math.min(w - 1, mx + r); x++) {
              const d = (x - mx) ** 2 + (y - my) ** 2;
              if (c.water![y * w + x] && d < bd) [best, bd] = [y * w + x, d];
            }
          }
          if (best >= 0) pts.push((best % w) + 0.5, ((best / w) | 0) + 0.5);
        }
      }
      const strength = clamp(0.3 + (0.7 * length(pts) * c.ftPx) / (500 * MI), 0.3, 1);
      c.raws.push({ kind: 'river', pts, closed: false, eps, stroke: { tool: 'river', radius_ft: 3 * MI, strength } });
    });
  });
}

/** A shape's size for the stroke cap: its bounding box's diagonal (px), lines and areas alike. */
function extent(pts: number[]): number {
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (let i = 0; i < pts.length; i += 2) {
    x0 = Math.min(x0, pts[i]);
    x1 = Math.max(x1, pts[i]);
    y0 = Math.min(y0, pts[i + 1]);
    y1 = Math.max(y1, pts[i + 1]);
  }
  return pts.length ? Math.hypot(x1 - x0, y1 - y0) : 0;
}

/**
 * The colour a pick takes from an n x n RGBA window of the picture centred on the click. Areas:
 * the 3x3 pixel most like the rest (noise, a hatching stroke or an edge pixel isn't taken). Rivers:
 * the 5x5 pixel most unlike the land round it (the window's rim), so a thin line picks its own
 * colour even when the click lands beside it. Null if the click is on a transparent pixel.
 */
export function snapPick(px: Uint8ClampedArray, n: number, river: boolean): [number, number, number] | null {
  const m = (n - 1) >> 1;
  const at = (x: number, y: number) => (y * n + x) * 4;
  const d2 = (i: number, c: number[]) => (px[i] - c[0]) ** 2 + (px[i + 1] - c[1]) ** 2 + (px[i + 2] - c[2]) ** 2;
  if (px[at(m, m) + 3] < 128) return null;
  const r = river ? Math.min(2, m) : Math.min(1, m);
  const cells: number[] = [];
  for (let y = m - r; y <= m + r; y++) for (let x = m - r; x <= m + r; x++) if (px[at(x, y) + 3] >= 128) cells.push(at(x, y));
  let best = at(m, m);
  if (river) {
    // The land: the rim's median, channel by channel.
    const rim: number[][] = [[], [], []];
    for (let k = 0; k < n; k++) {
      for (const i of [at(k, 0), at(k, n - 1), at(0, k), at(n - 1, k)]) if (px[i + 3] >= 128) for (let ch = 0; ch < 3; ch++) rim[ch].push(px[i + ch]);
    }
    if (rim[0].length) {
      const land = rim.map((v) => v.sort((a, b) => a - b)[v.length >> 1]);
      for (const i of cells) if (d2(i, land) > d2(best, land)) best = i;
    }
  } else {
    let bs = Infinity;
    for (const i of cells) {
      const s = cells.reduce((t, j) => t + d2(i, [px[j], px[j + 1], px[j + 2]]), 0);
      if (s < bs) [bs, best] = [s, i];
    }
  }
  return [px[best], px[best + 1], px[best + 2]];
}

const now = () => (globalThis.performance ? globalThis.performance.now() : Date.now());

/** Trace the picture: the strokes (world ft) for every kind picked. */
export function trace(req: TraceRequest): TraceResult {
  const t0 = now();
  const img = prepare(req);
  const c: Ctx = { img, s: Math.max(1, Math.max(img.w, img.h) / 1024), ftPx: req.xform.ftPerPx * img.f, hit: hits(req), raws: [], water: null, sea: null };
  const { picks, params } = req;
  if (picks.water) traceWater(c, params.water, !!picks.river);
  if (picks.mountain) traceMountains(c, params.mountain);
  if (picks.forest) traceForest(c, params.forest);
  if (picks.river) traceRivers(c, params.river);

  // At most MAX_STROKES (each costs the generator a pass): the smallest dropped first.
  let dropped = 0;
  if (c.raws.length > MAX_STROKES) {
    const size = c.raws.map((r) => extent(r.pts));
    const keep = c.raws.map((_, i) => i).sort((a, b) => size[b] - size[a] || a - b).slice(0, MAX_STROKES).sort((a, b) => a - b);
    dropped = c.raws.length - keep.length;
    c.raws = keep.map((i) => c.raws[i]);
  }
  // Simplify until the whole fits the budget.
  let out: number[][] = [];
  for (let scale = 1, tries = 0; tries < 16; tries++, scale *= 1.35) {
    out = c.raws.map((r) => simplify(r.pts, r.eps * scale, r.closed));
    if (out.reduce((n, p) => n + p.length / 2, 0) <= POINT_BUDGET) break;
  }
  const strokes: Stroke[] = [];
  const kinds: TraceKind[] = [];
  const counts = Object.fromEntries(TRACE_KINDS.map((k) => [k, 0])) as Record<TraceKind, number>;
  c.raws.forEach((r, i) => {
    const pts = out[i];
    if (pts.length / 2 < (r.closed ? 3 : 2)) return;
    const world: [number, number][] = [];
    for (let j = 0; j < pts.length; j += 2) {
      const [x, y] = imageToWorld(req.w, req.h, req.xform, pts[j] * img.f, pts[j + 1] * img.f);
      world.push([Math.round(x), Math.round(y)]);
    }
    strokes.push({ ...r.stroke, pts: world } as Stroke);
    kinds.push(r.kind);
    counts[r.kind]++;
  });
  return { strokes, kinds, counts, dropped, ms: now() - t0 };
}
