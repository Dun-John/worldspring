// Geometry on a location's grid of squares: what the characters can see (rays cast square
// by square, stopped by walls, closed doors, rock, buildings and tall objects, and hidden by
// higher ground), lit squares in the dark, ruler paths and shapes.
import { OPEN_BELOW, ROOF_SLOPE, type Interior } from '../gen/protocol';
import type { GridSettings, Shape } from './state';

/** Square flags. */
export const OPAQUE = 1;
export const BLOCKS = 2;
export const DIFFICULT = 4;
/** No floor: rock round an underground site, outside a building. */
export const SOLID = 8;
/** Not loaded (the surface away from the battlemap chunks in memory). */
export const UNKNOWN = 16;

/** A location's grid as play mode needs it. */
export interface TacticalGrid {
  flags(i: number, j: number): number;
  /** Ground height (ft) for seeing over hills; NaN where flat (inside). */
  elev(i: number, j: number): number;
  /** A wall (or closed door) on the west edge of square (i, j), i.e. the line x = i. */
  wallW(i: number, j: number): boolean;
  /** ... on its north edge (the line y = j). */
  wallN(i: number, j: number): boolean;
  /** x0, y0, x1, y1 (squares; exclusive), or null where unbounded. */
  bounds: [number, number, number, number] | null;
  /** Fill a window's flags and heights at once (faster than square by square), if it can. */
  fill?(x0: number, y0: number, w: number, h: number, flags: Uint8Array, elev: Float32Array): void;
  /** No walls on square edges anywhere (the surface): skip asking. */
  noWalls?: boolean;
}

/** Hidden hazards (traps, sinkholes: their rules say "hidden") are for the DM's eyes only. */
export const dmOnlyHazard = (rules: string | null | undefined) => !!rules && rules.startsWith('hidden');

/** Underground terrain that blocks movement (deep water, chasms) and what one walks on over it. */
const GAPS = new Set(['water_deep', 'chasm']);
const SPANS = new Set(['rope_bridge', 'stepping_stone']);

/**
 * A level of a building or site: rooms, walls, and doors that are shut (`closed`, by index
 * into the level's doors; secret doors are shut until opened).
 */
export function interiorGrid(it: Interior, li: number, closed: (door: number) => boolean): TacticalGrid {
  const lv = it.levels[li];
  const { nx, ny } = it;
  const flags = new Uint8Array(nx * ny);
  // (A hall's height left open is seen across and down into, not walked; a roof's slopes are
  // neither.)
  for (let k = 0; k < nx * ny; k++) {
    const kind = lv.cells[k] < 0 ? null : lv.rooms[lv.cells[k]].kind;
    if (kind === null || kind === ROOF_SLOPE) flags[k] = SOLID | OPAQUE | BLOCKS;
    else if (kind === OPEN_BELOW) flags[k] = BLOCKS;
  }
  // (Deep water and chasms block movement, not sight, unless a rope bridge or stepping stones lie over
  // them: `kit::blocked_squares`. Spans never clear anything else.)
  const gap = new Uint8Array(nx * ny);
  for (const f of lv.furniture) {
    const terrain = f.blocks_move && GAPS.has(f.kind);
    let v = 0;
    if (f.blocks_move && !terrain) v |= BLOCKS;
    if (terrain) for (let j = f.y; j < f.y + f.h && j < ny; j++) for (let i = f.x; i < f.x + f.w && i < nx; i++) gap[j * nx + i] = 1;
    if (f.blocks_move && f.height_ft >= 6) v |= OPAQUE;
    if (!f.blocks_move && (f.height_ft >= 1 || f.hazard)) v |= DIFFICULT;
    for (let j = f.y; j < f.y + f.h && j < ny; j++) for (let i = f.x; i < f.x + f.w && i < nx; i++) flags[j * nx + i] |= v;
  }
  for (const f of lv.furniture) if (SPANS.has(f.kind)) for (let j = f.y; j < f.y + f.h && j < ny; j++) for (let i = f.x; i < f.x + f.w && i < nx; i++) gap[j * nx + i] = 0;
  for (let k = 0; k < gap.length; k++) if (gap[k]) flags[k] |= BLOCKS;
  if (lv.has_stairs && !/^[uwk]:/.test(it.id)) {
    const [sx, sy, sw, sh] = it.stairs;
    for (let j = sy; j < sy + sh; j++) for (let i = sx; i < sx + sw; i++) if (i < nx && j < ny) flags[j * nx + i] |= DIFFICULT;
  }
  const wv = new Uint8Array((nx + 1) * ny);
  const wh = new Uint8Array(nx * (ny + 1));
  const line = (a: [number, number], b: [number, number]) => {
    if (a[0] === b[0]) {
      const x = Math.round(a[0]);
      for (let j = Math.round(Math.min(a[1], b[1])); j < Math.round(Math.max(a[1], b[1])); j++) if (j >= 0 && j < ny && x >= 0 && x <= nx) wv[j * (nx + 1) + x] = 1;
    } else {
      const y = Math.round(a[1]);
      for (let i = Math.round(Math.min(a[0], b[0])); i < Math.round(Math.max(a[0], b[0])); i++) if (i >= 0 && i < nx && y >= 0 && y <= ny) wh[y * nx + i] = 1;
    }
  };
  // (Railings over an open hall don't stop the eye; its open height stops a step.)
  // (A town hall's open arcade: open between its piers, which stand on the squares' corners.)
  for (const w of lv.walls) if (!w.rail && !w.arcade) line(w.a, w.b);
  // (A coaching inn's carriage arch never shut.)
  lv.doors.forEach((d, k) => d.kind !== 'arch' && closed(k) && line(d.a, d.b));
  // The street outside a carriage arch or an open arcade (a strip `APRON` squares deep in front of it, off the
  // floor): open ground, walked onto and seen across (a token put there is out in the street).
  const R = APRON;
  const [ex, ey] = [nx + 2 * R, ny + 2 * R];
  const apron = new Uint8Array(ex * ey);
  const floor = (i: number, j: number) => i >= 0 && j >= 0 && i < nx && j < ny && lv.cells[j * nx + i] >= 0;
  const open = (a: [number, number], b: [number, number]) => {
    const vert = a[0] === b[0];
    const [p, q0, q1] = vert ? [Math.round(a[0]), Math.round(Math.min(a[1], b[1])), Math.round(Math.max(a[1], b[1]))] : [Math.round(a[1]), Math.round(Math.min(a[0], b[0])), Math.round(Math.max(a[0], b[0]))];
    for (let q = q0; q < q1; q++) {
      // (The square across the edge that is off the floor.)
      const [i0, j0, i1, j1] = vert ? [p - 1, q, p, q] : [q, p - 1, q, p];
      const out0 = !floor(i0, j0);
      const [oi, oj] = out0 ? [i0, j0] : !floor(i1, j1) ? [i1, j1] : [NaN, NaN];
      if (Number.isNaN(oi)) continue;
      // (A strip APRON squares deep straight out from the opening, so it can't lie over the house
      // across a narrow alley.)
      const [ni, nj] = vert ? [out0 ? -1 : 1, 0] : [0, out0 ? -1 : 1];
      for (let k = 0; k < R; k++) {
        const [i, j] = [oi + ni * k, oj + nj * k];
        if (i >= -R && j >= -R && i < nx + R && j < ny + R && !floor(i, j)) apron[(j + R) * ex + i + R] = 1;
      }
    }
  };
  for (const d of lv.doors) if (d.kind === 'arch') open(d.a, d.b);
  for (const w of lv.walls) if (w.arcade) open(w.a, w.b);
  const aproned = apron.some((v) => v === 1);
  const at = (i: number, j: number) => {
    if (i >= -R && j >= -R && i < nx + R && j < ny + R && apron[(j + R) * ex + i + R]) return 0;
    return i < 0 || j < 0 || i >= nx || j >= ny ? SOLID | OPAQUE | BLOCKS : flags[j * nx + i];
  };
  return {
    flags: at,
    elev: () => NaN,
    fill: (x0, y0, w, h, out, elev) => {
      elev.fill(NaN);
      for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) out[j * w + i] = at(x0 + i, y0 + j);
    },
    wallW: (i, j) => i >= 0 && i <= nx && j >= 0 && j < ny && wv[j * (nx + 1) + i] === 1,
    wallN: (i, j) => i >= 0 && i < nx && j >= 0 && j <= ny && wh[j * nx + i] === 1,
    bounds: aproned ? [-R, -R, nx + R, ny + R] : [0, 0, nx, ny],
  };
}

/** How far the street outside a carriage arch or an open arcade is open in a building's grid
 * (squares). */
export const APRON = 2;

/** Squares set in a window of a location's grid. */
export interface Bitmap {
  x0: number;
  y0: number;
  w: number;
  h: number;
  bits: Uint8Array;
}

export const bitAt = (b: Bitmap, i: number, j: number) => {
  const [x, y] = [i - b.x0, j - b.y0];
  return x >= 0 && y >= 0 && x < b.w && y < b.h && b.bits[y * b.w + x] > 0;
};

/** Someone or something that sees or lights: centre (squares), eye height (ft), reach (squares). */
export interface Eye {
  x: number;
  y: number;
  /** Eye or flame height above the ground (ft). */
  above: number;
  range: number;
}

/** How far anyone sees in plain light (squares; 300 ft). */
export const SIGHT = 60;

/** A window of the grid copied into flat arrays (rays read these, not the grid). */
interface Local {
  x0: number;
  y0: number;
  w: number;
  h: number;
  flags: Uint8Array;
  elev: Float32Array;
  /** Walls on the line x = i (row j): (w + 1) x h; on the line y = j: w x (h + 1). */
  wv: Uint8Array;
  wh: Uint8Array;
}

function local(grid: TacticalGrid, x0: number, y0: number, w: number, h: number): Local {
  const L: Local = { x0, y0, w, h, flags: new Uint8Array(w * h), elev: new Float32Array(w * h), wv: new Uint8Array((w + 1) * h), wh: new Uint8Array(w * (h + 1)) };
  if (grid.fill) {
    grid.fill(x0, y0, w, h, L.flags, L.elev);
  } else {
    for (let j = 0; j < h; j++) {
      for (let i = 0; i < w; i++) {
        L.flags[j * w + i] = grid.flags(x0 + i, y0 + j);
        L.elev[j * w + i] = grid.elev(x0 + i, y0 + j);
      }
    }
  }
  if (grid.noWalls) return L;
  for (let j = 0; j < h; j++) for (let i = 0; i <= w; i++) L.wv[j * (w + 1) + i] = grid.wallW(x0 + i, y0 + j) ? 1 : 0;
  for (let j = 0; j <= h; j++) for (let i = 0; i < w; i++) L.wh[j * w + i] = grid.wallN(x0 + i, y0 + j) ? 1 : 0;
  return L;
}

/**
 * Cast rays from `e` across the window, setting (to 1) every square seen in `out` (the same
 * window). Squares not loaded stop sight (nothing is known of them), as does the window's edge.
 */
function cast(L: Local, e: Eye, out: Uint8Array) {
  const { w, h, flags, elev, wv, wh } = L;
  const ox = e.x - L.x0;
  const oy = e.y - L.y0;
  const R = e.range;
  const oi = Math.floor(ox);
  const oj = Math.floor(oy);
  if (oi < 0 || oj < 0 || oi >= w || oj >= h) return;
  out[oj * w + oi] = 1;
  const g0 = elev[oj * w + oi];
  const eye = Number.isNaN(g0) ? NaN : g0 + e.above;
  // Enough rays that neighbours at the far end are under a square apart.
  const n = Math.max(240, Math.ceil(2 * Math.PI * R * 1.6));
  for (let k = 0; k < n; k++) {
    const a = ((k + 0.5) / n) * Math.PI * 2;
    const dx = Math.cos(a);
    const dy = Math.sin(a);
    let i = oi;
    let j = oj;
    const sx = dx > 0 ? 1 : -1;
    const sy = dy > 0 ? 1 : -1;
    const ddx = Math.abs(1 / dx);
    const ddy = Math.abs(1 / dy);
    let tx = Math.abs(dx) < 1e-12 ? Infinity : (dx > 0 ? i + 1 - ox : ox - i) * ddx;
    let ty = Math.abs(dy) < 1e-12 ? Infinity : (dy > 0 ? j + 1 - oy : oy - j) * ddy;
    let horizon = -Infinity;
    for (;;) {
      let t: number;
      if (tx < ty) {
        t = tx;
        if (wv[j * (w + 1) + (sx > 0 ? i + 1 : i)]) break;
        i += sx;
        tx += ddx;
      } else {
        t = ty;
        if (wh[(sy > 0 ? j + 1 : j) * w + i]) break;
        j += sy;
        ty += ddy;
      }
      if (t > R || i < 0 || j < 0 || i >= w || j >= h) break;
      const q = j * w + i;
      const f = flags[q];
      if (f & UNKNOWN) break;
      let seen = true;
      if (!Number.isNaN(eye)) {
        // Higher ground between hides what lies lower behind it (the tangent to the horizon).
        const hq = elev[q];
        if (!Number.isNaN(hq)) {
          const s = (hq - eye) / (t + 0.5);
          seen = s >= horizon - 0.04;
          if (s > horizon) horizon = s;
        }
      }
      if (seen) out[q] = 1;
      if (f & (OPAQUE | SOLID)) break;
    }
  }
}

/** What the eyes see together; in the dark, only the squares `lights` reach. */
export function computeVision(grid: TacticalGrid, eyes: Eye[], lights: Eye[], dark: boolean): Bitmap {
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const e of eyes) {
    x0 = Math.min(x0, Math.floor(e.x - e.range));
    y0 = Math.min(y0, Math.floor(e.y - e.range));
    x1 = Math.max(x1, Math.ceil(e.x + e.range));
    y1 = Math.max(y1, Math.ceil(e.y + e.range));
  }
  const b = grid.bounds;
  if (b) [x0, y0, x1, y1] = [Math.max(x0, b[0]), Math.max(y0, b[1]), Math.min(x1, b[2]), Math.min(y1, b[3])];
  if (!eyes.length || x1 <= x0 || y1 <= y0) return { x0: 0, y0: 0, w: 0, h: 0, bits: new Uint8Array(0) };
  const [w, h] = [x1 - x0, y1 - y0];
  const L = local(grid, x0, y0, w, h);
  const bits = new Uint8Array(w * h);
  let lit: Uint8Array | null = null;
  if (dark) {
    lit = new Uint8Array(w * h);
    for (const l of lights) cast(L, l, lit);
  }
  const seen = new Uint8Array(w * h);
  for (const e of eyes) {
    seen.fill(0);
    cast(L, e, seen);
    for (let k = 0; k < w * h; k++) if (seen[k] && (!lit || lit[k])) bits[k] = 1;
  }
  return { x0, y0, w, h, bits };
}

/** Squares between two squares, the way the ruler counts them. */
export function gridDistance(dx: number, dy: number, m: GridSettings['measure']): number {
  const [a, b] = [Math.abs(dx), Math.abs(dy)];
  switch (m) {
    case 'chebyshev':
      return Math.max(a, b);
    case 'alternating':
      return Math.max(a, b) + Math.floor(Math.min(a, b) / 2);
    case 'euclidean':
      return Math.hypot(a, b);
    case 'manhattan':
      return a + b;
  }
}

/** Units that become a larger one over long distances (feet and yards to miles, metres to
 * kilometres): how many make one. */
const LONG: Record<string, [number, string]> = { ft: [5280, 'mi'], feet: [5280, 'mi'], foot: [5280, 'mi'], yd: [1760, 'mi'], yds: [1760, 'mi'], m: [1000, 'km'], meters: [1000, 'km'], metres: [1000, 'km'] };

/** A distance in the grid's unit, for labels ("30 ft", "4.5 m"), in miles or kilometres from
 * one of those up ("12.4 mi", "3 km"). */
export function formatDistance(squares: number, g: GridSettings): string {
  let v = squares * g.scale;
  let unit = g.unit;
  const long = LONG[unit.toLowerCase()];
  if (long && v >= long[0]) [v, unit] = [v / long[0], long[1]];
  const shown = v >= 100 ? Math.round(v) : Math.round(v * 10) / 10;
  return `${shown.toLocaleString()}${unit ? ` ${unit}` : ''}`;
}

/**
 * A ruler's reading: the distance across the grid (`dist`, squares) and, where the ground
 * at both ends is known, the rise from start to end (`rise`, ft; a square is 5 ft): then
 * also how far up or down, and the straight line between the two (the hypotenuse).
 */
export function rulerLabel(dist: number, rise: number | null, g: GridSettings): string {
  const across = formatDistance(dist, g);
  if (rise === null) return across;
  if (Math.abs(rise) < 1) return `${across} · level`;
  const up = Math.abs(rise) / 5;
  return `${across} horizontal
${formatDistance(up, g)} ${rise > 0 ? 'up' : 'down'}
${formatDistance(Math.hypot(dist, up), g)} diagonal`;
}

/** A ruler from square `a` to square `b`: the squares crossed and the distance (squares). */
export function measure(a: [number, number], b: [number, number], g: GridSettings): { squares: [number, number][]; dist: number } {
  const [dx, dy] = [Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1])];
  // The squares along the line (Bresenham).
  const squares: [number, number][] = [];
  let [x, y] = a;
  const [sx, sy] = [Math.sign(b[0] - a[0]), Math.sign(b[1] - a[1])];
  let err = dx - dy;
  for (;;) {
    squares.push([x, y]);
    if (x === b[0] && y === b[1]) break;
    const e2 = 2 * err;
    if (e2 > -dy) {
      err -= dy;
      x += sx;
    }
    if (e2 < dx) {
      err += dx;
      y += sy;
    }
  }
  return { squares, dist: gridDistance(dx, dy, g.measure) };
}

/** A shape's outline (squares, relative to its origin): a polygon, or a polyline for lines. */
export function shapeOutline(s: Shape): number[] {
  const len = Math.hypot(s.dx, s.dy);
  switch (s.kind) {
    case 'circle': {
      const pts: number[] = [];
      for (let k = 0; k < 64; k++) pts.push(Math.cos((k / 64) * Math.PI * 2) * len, Math.sin((k / 64) * Math.PI * 2) * len);
      return pts;
    }
    case 'rect':
      return [0, 0, s.dx, 0, s.dx, s.dy, 0, s.dy];
    case 'line':
      return [0, 0, s.dx, s.dy];
    case 'cone': {
      // As wide as it is long at its far end, with a rounded front.
      if (len < 1e-6) return [];
      const a = Math.atan2(s.dy, s.dx);
      const half = Math.atan(0.5);
      const pts = [0, 0];
      for (let k = 0; k <= 16; k++) {
        const t = a - half + (2 * half * k) / 16;
        pts.push(Math.cos(t) * len, Math.sin(t) * len);
      }
      return pts;
    }
  }
}

/** A shape's size, for its label ("20 ft radius", "15 × 10 ft", "30 ft"). */
export function shapeLabel(s: Shape, g: GridSettings): string {
  const len = Math.hypot(s.dx, s.dy);
  if (s.kind === 'circle') return `${formatDistance(len, g)} radius`;
  if (s.kind === 'rect') return `${formatDistance(Math.abs(s.dx), g)} × ${formatDistance(Math.abs(s.dy), g)}`;
  return formatDistance(len, g);
}
