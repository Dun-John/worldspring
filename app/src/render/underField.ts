// The textures an underground level is painted from (`underground.frag`): a smooth floor field
// (4 texels a square, blurred), liquids and raised floors likewise, per-square materials, and
// terrain (water, chasms, rubble slopes) likewise.
// Pure data, so a city's sewer sections can be prepared in a worker (`underField.worker.ts`)
// while the view moves.
import type { Interior, InteriorLevel } from '../gen/protocol';
import { CAVERN, fromSquares, shapeIndex } from './roomShapes';

/** Field texels per square. */
export const FIELD_RES = 4;
/** Squares round a section of sewers its field is worked out over (`under::SEWER_RIM`): past the
 * widest blur and the walls a channel looks to, so a texel's value comes out the same on both sides
 * of a border. */
const SEWER_RIM = 5;
/** Of those, the squares kept in the texture: as far past the edge as the shader looks. */
const FIELD_PAD = 1;
/** The dry floor along every wall of a sewer (squares): sewage lies this far from a wall or more.
 * `under/themes/sewer.rs` counts squares by the same rule (`WALKWAY` there). */
const WALKWAY = 0.9;
/** How finely `sewerWalls` follows the walls (squares; `WALL_POINT` there). */
const WALL_POINT = 0.1;
/** Width (squares) of the ramp across the channel's edge, for a contour finer than a texel. */
const CHANNEL_RAMP = 0.25;

export interface UnderField {
  /** Field texture (w x h, RGBA: floor, liquid, raised, the way sewage runs (`flowAngle`); `pad`
   * squares wider than the grid each way) and per-square cells (nx x ny, RGBA:
   * material, shape (0 none, its slot + 1 in `shapes`, 254 none but one next to it, 255 a cavern),
   * room hash (a sewer's brick: its foam, `sewerFoam`), 255). A shaped room's squares are left out of the floor (its doorways stay in): the
   * shader draws it from its shape. */
  w: number;
  h: number;
  pad: number;
  data: Uint8Array;
  cells: Uint8Array;
  /** 0 none, 1 sewage, 2 lava. */
  liquidKind: number;
  /** Shaped rooms (`roomShapes.ts`), two texels each in a (count x 2) texture: x0, y0, x1, y1
   * (squares), then shape, material, room hash, 255. One blank slot when there are none. */
  shapes: Uint8Array;
  shapeCount: number;
  /** Caverns breaking in: their floor (and the squares opening into them) blurred as a cave's
   * (w x h, RGBA, red), else null. */
  cave: Uint8Array | null;
  /** Terrain (`kit::TERRAIN`) as soft masks, blurred (w x h, RGBA: shallow water (deep water
   * too: it lies in water), deep water, chasm, rubble slope), else null. */
  terrain: Uint8Array | null;
}

/** Underground terrain kinds, by their channel in `UnderField.terrain`. */
export const TERRAIN_CHANNEL: Record<string, number> = { water_shallow: 0, water_deep: 1, chasm: 2, rubble_slope: 3 };

/** Sites with square corners: built (dressed stone), unlike sewers (which follow their
 * streets) and natural rock. */
export const SQUARE_SITES = new Set(['dungeon', 'crypt', 'catacombs', 'deep dungeons']);

/** How rough a site's rock is: how far its walls jut out and bite in (the shader's `uRough`). */
export function roughness(site: string): number {
  return site === 'cave' ? 1 : site === 'lava tube' ? 0.85 : site === 'mine' ? 0.45 : SQUARE_SITES.has(site) ? 0 : 0.06;
}

/** Floor material for the shader: 0 cave rock, 1 basalt, 2 mine earth, 3 sewer brick,
 * 4 flagstones, 5 boss flagstones, 6 crypt flagstones, 7 marble, 8 hewn blocks, 9 ice, 10 the cult's dark ring flagstones. */
export function materialOf(site: string, kind: string, theme?: string): number {
  if (kind === 'ring corridor') return 10;
  if (kind === 'sewer tunnel' || kind === 'service passage') return 3;
  if (theme === 'ice') return 9;
  if (site === 'cave') return 0;
  if (theme === 'goblin_warren' || theme === 'bandit_hideout') return 2;
  if (kind === 'boss chamber') return 5;
  if (theme === 'cult_sanctum') return 6;
  if (theme === 'temple' || theme === 'sunken_temple' || theme === 'wizard_lair') return 7;
  if (theme === 'dwarven_hall' || theme === 'labyrinth') return 8;
  if (site === 'lava tube') return 1;
  if (site === 'mine') return 2;
  if (site === 'crypt' || site === 'catacombs') return 6;
  return 4;
}

/** Separable Gaussian blur of a w × h field (outside counts as 0). */
export function blur(src: Float32Array, w: number, h: number, sigma: number): Float32Array {
  const r = Math.ceil(sigma * 2.5);
  const k = new Float32Array(2 * r + 1);
  let sum = 0;
  for (let i = -r; i <= r; i++) sum += k[i + r] = Math.exp(-(i * i) / (2 * sigma * sigma));
  for (let i = 0; i < k.length; i++) k[i] /= sum;
  const tmp = new Float32Array(w * h);
  const out = new Float32Array(w * h);
  // Along rows (bounds checked only near the ends), then whole rows at a time down columns.
  for (let y = 0; y < h; y++) {
    const o = y * w;
    for (let x = 0; x < w; x++) {
      let a = 0;
      if (x >= r && x < w - r) {
        for (let i = -r; i <= r; i++) a += src[o + x + i] * k[i + r];
      } else {
        for (let i = -r; i <= r; i++) {
          const xi = x + i;
          if (xi >= 0 && xi < w) a += src[o + xi] * k[i + r];
        }
      }
      tmp[o + x] = a;
    }
  }
  for (let y = 0; y < h; y++) {
    const o = y * w;
    for (let i = -r; i <= r; i++) {
      const yi = y + i;
      if (yi < 0 || yi >= h) continue;
      const q = yi * w;
      const kk = k[i + r];
      for (let x = 0; x < w; x++) out[o + x] += tmp[q + x] * kk;
    }
  }
  return out;
}

/** The room whose floor follows the lines. */
export const LINE_ROOM = 'sewer tunnel';

/** The squares of a level with floor on or beside them: a line's floor is drawn only there (runs
 * dropped from the sewer stay rock). */
export function nearFloor(lv: InteriorLevel, nx: number, ny: number, pad = 0): Uint8Array {
  const w = nx + 2 * pad;
  const near = new Uint8Array(w * (ny + 2 * pad));
  const mark = (i: number, j: number) => {
    for (let b = Math.max(-pad, j - 1); b <= Math.min(ny + pad - 1, j + 1); b++) for (let a = Math.max(-pad, i - 1); a <= Math.min(nx + pad - 1, i + 1); a++) near[(b + pad) * w + a + pad] = 1;
  };
  for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) if (lv.cells[j * nx + i] >= 0) mark(i, j);
  for (const [i, j] of lv.rim ?? []) mark(i, j);
  return near;
}

/** How far a point is past a line's reach (below 0 inside), and the unit step from the line out to it. */
function offLine([x0, y0, x1, y1, r]: number[], px: number, py: number): [number, number, number] {
  const [dx, dy] = [x1 - x0, y1 - y0];
  const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / Math.max(dx * dx + dy * dy, 1e-9)));
  const [ex, ey] = [px - x0 - t * dx, py - y0 - t * dy];
  const e = Math.max(Math.hypot(ex, ey), 1e-9);
  return [e - r, ex / e, ey / e];
}

const WALL_RAYS = 64;
const WALL_STEP = 0.1;
/** How far a prop looks for its wall (squares). */
const WALL_REACH = 2.6;
/** How far a wall's point stays within the section: the next section's floor would cover what hangs over. */
const WALL_MARGIN = 0.5;

/** Calls `at` with points along the walls of the floor a level's lines paint: every line's outline
 * where no other line covers it, and the edges of `near` (`pad` squares past the grid) where a line
 * is cut off. Passages don't count: they open in a wall, and the next section knows nothing of them. */
function sewerWalls(paths: number[][], near: Uint8Array, nx: number, ny: number, pad: number, at: (x: number, y: number) => void) {
  const [across, down] = [nx + 2 * pad, ny + 2 * pad];
  // The lines within reach of each square.
  const cover: (number[] | undefined)[] = new Array(across * down);
  paths.forEach((l, n) => {
    const m = l[4] + 1;
    const i0 = Math.max(-pad, Math.floor(Math.min(l[0], l[2]) - m));
    const j0 = Math.max(-pad, Math.floor(Math.min(l[1], l[3]) - m));
    const i1 = Math.min(nx + pad - 1, Math.floor(Math.max(l[0], l[2]) + m));
    const j1 = Math.min(ny + pad - 1, Math.floor(Math.max(l[1], l[3]) + m));
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) {
        if (offLine(l, i + 0.5, j + 0.5)[0] > 0.75) continue;
        const k = (j + pad) * across + i + pad;
        (cover[k] ??= []).push(n);
      }
    }
  });
  const square = (x: number, y: number) => {
    const [i, j] = [Math.floor(x) + pad, Math.floor(y) + pad];
    return i >= 0 && j >= 0 && i < across && j < down ? j * across + i : -1;
  };
  // A point of a line's outline is a wall on the band and inside no line (its end circles' far halves are inside it).
  const outline = (x: number, y: number) => {
    const k = square(x, y);
    if (k >= 0 && near[k] && !(cover[k] ?? []).some((o) => offLine(paths[o], x, y)[0] < -1e-3)) at(x, y);
  };
  paths.forEach(([x0, y0, x1, y1, r]) => {
    const len = Math.hypot(x1 - x0, y1 - y0);
    if (len > 1e-6) {
      const [ox, oy] = [(-(y1 - y0) / len) * r, ((x1 - x0) / len) * r];
      const along = Math.ceil(len / WALL_POINT);
      for (let a = 0; a <= along; a++) {
        const [x, y] = [x0 + ((x1 - x0) * a) / along, y0 + ((y1 - y0) * a) / along];
        outline(x + ox, y + oy);
        outline(x - ox, y - oy);
      }
    }
    const round = Math.ceil((2 * Math.PI * r) / WALL_POINT);
    for (let a = 0; a < round; a++) {
      const t = (a * 2 * Math.PI) / round;
      const [c, s] = [Math.cos(t) * r, Math.sin(t) * r];
      outline(x0 + c, y0 + s);
      outline(x1 + c, y1 + s);
    }
  });
  // Where the band is cut off: each side of a near square against one that isn't.
  const steps = Math.round(1 / WALL_POINT);
  for (let j = 0; j + 1 < down; j++) {
    for (let i = 0; i + 1 < across; i++) {
      const k = j * across + i;
      for (const [other, ax, ay] of [[k + 1, 1, 0], [k + across, 0, 1]]) {
        if (near[k] === near[other]) continue;
        const [x, y] = ax ? [i + 1 - pad, j - pad] : [i - pad, j + 1 - pad];
        const over = cover[near[k] ? k : other] ?? [];
        for (let a = 0; a <= steps; a++) {
          const [px, py] = [x + (ay * a) / steps, y + (ax * a) / steps];
          if (over.some((o) => offLine(paths[o], px, py)[0] < 0)) at(px, py);
        }
      }
    }
  }
}

/** How much sewage each texel of the field holds (0-1; `lined`: the lines' floor): where it lies at
 * least `WALKWAY` from every wall. Depth, not a blur: runs meeting don't build up, so the channels of
 * a crossing merge into a cross with the dry floor turning its corners. */
function sewerChannel(paths: number[][], near: Uint8Array, lined: Float32Array, nx: number, ny: number, pad: number, w: number, h: number): Float32Array {
  const R = FIELD_RES;
  const reach = WALKWAY + CHANNEL_RAMP / 2;
  const [reach2, span] = [reach * reach, Math.ceil(reach * R)];
  const close = new Float32Array(w * h).fill(reach2);
  sewerWalls(paths, near, nx, ny, pad, (x, y) => {
    const [cx, cy] = [(x + pad) * R - 0.5, (y + pad) * R - 0.5];
    const [tx0, ty0] = [Math.max(0, Math.round(cx) - span), Math.max(0, Math.round(cy) - span)];
    const [tx1, ty1] = [Math.min(w - 1, Math.round(cx) + span), Math.min(h - 1, Math.round(cy) + span)];
    for (let ty = ty0; ty <= ty1; ty++) {
      const dy = (ty - cy) / R;
      for (let tx = tx0; tx <= tx1; tx++) {
        const dx = (tx - cx) / R;
        const d2 = dx * dx + dy * dy;
        if (d2 < close[ty * w + tx]) close[ty * w + tx] = d2;
      }
    }
  });
  const out = new Float32Array(w * h);
  for (let k = 0; k < w * h; k++) if (lined[k]) out[k] = Math.max(0, Math.min(1, (Math.sqrt(close[k]) - WALKWAY) / CHANNEL_RAMP + 0.5));
  return out;
}

/** The nearest wall of the floor painted from a level's lines to (cx, cy), in grid squares: the point
 * on it and the unit step from it into the rock. Rays from there find where the floor painted by
 * `underField` meets rock, so a point inside one line on another's edge is no wall, and floor on squares
 * (a passage dug beside a tunnel) neither. Null if nothing within reach is a line's edge in the section. */
export function lineWall(lv: InteriorLevel, nx: number, ny: number, near: Uint8Array, cx: number, cy: number): { x: number; y: number; dx: number; dy: number } | null {
  const paths = lv.paths ?? [];
  const onFloor = (px: number, py: number) => {
    const [i, j] = [Math.floor(px), Math.floor(py)];
    if (i >= 0 && j >= 0 && i < nx && j < ny) {
      const k = j * nx + i;
      const r = lv.cells[k];
      if (r >= 0 && lv.rooms[r].kind !== LINE_ROOM) return true;
      if (!near[k]) return false;
    }
    return paths.some((l) => offLine(l, px, py)[0] <= 0);
  };
  const from = onFloor(cx, cy);
  let best: { x: number; y: number; dx: number; dy: number } | null = null;
  let reach = WALL_REACH;
  for (let a = 0; a < WALL_RAYS; a++) {
    const [ux, uy] = [Math.cos((a / WALL_RAYS) * Math.PI * 2), Math.sin((a / WALL_RAYS) * Math.PI * 2)];
    for (let t = WALL_STEP; t < reach; t += WALL_STEP) {
      if (onFloor(cx + ux * t, cy + uy * t) === from) continue;
      let [lo, hi] = [t - WALL_STEP, t];
      for (let b = 0; b < 6; b++) {
        const m = (lo + hi) / 2;
        if (onFloor(cx + ux * m, cy + uy * m) === from) lo = m;
        else hi = m;
      }
      const [x, y] = [cx + ux * hi, cy + uy * hi];
      // On a line's edge? (Else squares: not this wall.)
      let edge: [number, number, number] | null = null;
      for (const l of paths) {
        const o = offLine(l, x, y);
        if (Math.abs(o[0]) < 0.02 && (!edge || Math.abs(o[0]) < Math.abs(edge[0]))) edge = o;
      }
      if (edge && hi < reach && x >= WALL_MARGIN && y >= WALL_MARGIN && x <= nx - WALL_MARGIN && y <= ny - WALL_MARGIN) {
        reach = hi;
        best = { x, y, dx: edge[1], dy: edge[2] };
      }
      break;
    }
  }
  return best;
}

/** The way sewage runs along a run from (0, 0) toward (dx, dy), for the field's alpha (0-1): every
 * run flows toward `FLOW_TO` (grid axes), so runs meeting across sections agree; the angle from it
 * (within a quarter turn either way) over a half turn, plus 0.5. The shader turns it back
 * (`underground.frag`, the sewage). */
const FLOW_TO = [0.958, 0.287];
function flowAngle(dx: number, dy: number): number {
  const along = dx * FLOW_TO[0] + dy * FLOW_TO[1];
  const side = FLOW_TO[0] * dy - FLOW_TO[1] * dx;
  const s = along < 0 ? -1 : 1;
  return Math.max(0, Math.min(1, Math.atan2(side * s, along * s) / Math.PI + 0.5));
}

/** Foam in a sewer's channel (0-255 in its squares' blue, which brick doesn't use): round where
 * runs meet (three or more ends together), where one ends, and at the outfalls, fading over
 * `FOAM_REACH` squares. */
const FOAM_REACH = 2.2;
function sewerFoam(lv: InteriorLevel, paths: number[][], nx: number, ny: number, cells: Uint8Array) {
  const ends = new Map<string, { x: number; y: number; n: number }>();
  for (const [x0, y0, x1, y1] of paths) {
    for (const [x, y] of [[x0, y0], [x1, y1]]) {
      const key = `${Math.round(x * 20)},${Math.round(y * 20)}`;
      const e = ends.get(key);
      if (e) e.n++;
      else ends.set(key, { x, y, n: 1 });
    }
  }
  const foamed = new Set<number>();
  const at = [...ends.values()].filter((e) => e.n !== 2).map((e) => [e.x, e.y]);
  for (const f of lv.furniture) if (f.kind === 'outfall') at.push([f.x + f.w / 2, f.y + f.h / 2]);
  for (const [x, y] of at) {
    const [i0, j0] = [Math.max(0, Math.floor(x - FOAM_REACH)), Math.max(0, Math.floor(y - FOAM_REACH))];
    const [i1, j1] = [Math.min(nx - 1, Math.floor(x + FOAM_REACH)), Math.min(ny - 1, Math.floor(y + FOAM_REACH))];
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) {
        const k = j * nx + i;
        if (cells[k * 4] !== 3 || cells[k * 4 + 3] === 0) continue;
        const v = Math.round(255 * Math.max(0, 1 - Math.hypot(i + 0.5 - x, j + 0.5 - y) / FOAM_REACH));
        cells[k * 4 + 2] = Math.max(v, foamed.has(k) ? cells[k * 4 + 2] : 0);
        foamed.add(k);
      }
    }
  }
  // (The rest of the brick: none.)
  for (let k = 0; k < nx * ny; k++) if (cells[k * 4] === 3 && !foamed.has(k)) cells[k * 4 + 2] = 0;
}

/** The textures of a level of an underground site. */
export function underField(it: Interior, level: number): UnderField {
  const lv = it.levels[level];
  const { nx, ny } = it;
  const paths = lv.paths ?? [];
  const R = FIELD_RES;
  // Lines' floors are worked out past the grid, so that blurs near its edge see what lies beyond.
  const rim = paths.length ? SEWER_RIM : 0;
  const off = rim * R;
  const [w, h] = [(nx + 2 * rim) * R, (ny + 2 * rim) * R];
  const floor = new Float32Array(w * h);
  const liquid = new Float32Array(w * h);
  const raised = new Float32Array(w * h);
  // Which way the sewage runs (`flowAngle`), by the nearest run.
  const flow = new Float32Array(w * h).fill(0.5);
  const wet = new Uint8Array(nx * ny);
  let liquidKind = 0;
  for (const f of lv.furniture) {
    if (f.kind === 'sewage' || f.kind === 'lava') {
      wet[f.y * nx + f.x] = 1;
      liquidKind = f.kind === 'sewage' ? 1 : 2;
    }
  }
  const cells = new Uint8Array(nx * ny * 4);
  const { index, shapes: slots, caverns } = shapeIndex(lv, nx, ny);
  let anyRaised = false;
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const k = j * nx + i;
      const r = lv.cells[k];
      if (r < 0) continue;
      const room = lv.rooms[r];
      cells[k * 4] = room.shape === CAVERN ? 0 : materialOf(it.function, room.kind, it.theme);
      cells[k * 4 + 1] = index[k];
      cells[k * 4 + 2] = (r * 37) & 255;
      cells[k * 4 + 3] = 255;
      // Floors that follow lines (sewers under streets) are painted from the lines below; shaped
      // rooms from their shapes; anything else fills its squares.
      const square = (!paths.length || room.kind !== LINE_ROOM) && fromSquares(index[k]);
      const lift = Math.min(1, room.raise_ft / 10);
      if (lift > 0) anyRaised = true;
      for (let y = j * R; y < (j + 1) * R; y++) {
        const o = (y + off) * w + off;
        for (let x = i * R; x < (i + 1) * R; x++) {
          if (square) floor[o + x] = 1;
          liquid[o + x] = wet[k];
          raised[o + x] = lift;
        }
      }
    }
  }
  if (paths.length) {
    // Along the lines: the band within reach, where it borders real floor (runs dropped from
    // the sewer stay rock), so diagonal tunnels are smooth rather than stepped.
    const near = nearFloor(lv, nx, ny, rim);
    const across = nx + 2 * rim;
    const lined = new Float32Array(w * h);
    const closest = new Float32Array(w * h).fill(Infinity);
    for (const [x0, y0, x1, y1, r] of paths) {
      const dx = x1 - x0;
      const dy = y1 - y0;
      const len2 = Math.max(dx * dx + dy * dy, 1e-9);
      const r2 = r * r;
      const way = flowAngle(dx, dy);
      const tx0 = Math.max(0, Math.floor((Math.min(x0, x1) - r + rim) * R));
      const ty0 = Math.max(0, Math.floor((Math.min(y0, y1) - r + rim) * R));
      const tx1 = Math.min(w - 1, Math.ceil((Math.max(x0, x1) + r + rim) * R));
      const ty1 = Math.min(h - 1, Math.ceil((Math.max(y0, y1) + r + rim) * R));
      for (let ty = ty0; ty <= ty1; ty++) {
        const py = (ty + 0.5) / R - rim;
        const row = (Math.floor(py) + rim) * across + rim;
        for (let tx = tx0; tx <= tx1; tx++) {
          const px = (tx + 0.5) / R - rim;
          if (!near[row + Math.floor(px)]) continue;
          const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / len2));
          const ex = px - x0 - t * dx;
          const ey = py - y0 - t * dy;
          const e2 = ex * ex + ey * ey;
          if (e2 > r2) continue;
          lined[ty * w + tx] = 1;
          if (e2 < closest[ty * w + tx]) {
            closest[ty * w + tx] = e2;
            flow[ty * w + tx] = way;
          }
        }
      }
    }
    // The sewage channel down the middle of the runs, from the lines' floor alone (a passage is the
    // section's own, and stays dry). The squares of it that count as sewage in play follow the same
    // rule (`themes/sewer.rs`).
    liquidKind = 1;
    liquid.set(sewerChannel(paths, near, lined, nx, ny, rim, w, h));
    for (let k = 0; k < w * h; k++) if (lined[k]) floor[k] = 1;
    sewerFoam(lv, paths, nx, ny, cells);
  }
  // Natural rock is rounded; sewers follow their streets; built sites keep square corners
  // (the shader measures their walls from the squares).
  const f1 = SQUARE_SITES.has(it.function) ? floor : blur(floor, w, h, (lv.natural ? 0.42 : paths.length ? 0.22 : 0.14) * R);
  // A lava channel is one square wide, stepping diagonally: blur it wider and lift it so it
  // runs as one flow rather than beads.
  let f2: Float32Array = liquid;
  if (liquidKind === 1) f2 = blur(liquid, w, h, 0.3 * R);
  if (liquidKind === 2) f2 = blur(liquid, w, h, 0.6 * R).map((v) => Math.min(1, v * 1.7));
  const f3 = anyRaised ? blur(raised, w, h, 0.3 * R) : raised;
  // (The texture keeps what the shader reads past the grid's edge.)
  const pad = rim ? FIELD_PAD : 0;
  const skip = (rim - pad) * R;
  const [tw, th] = [w - 2 * skip, h - 2 * skip];
  const data = new Uint8Array(tw * th * 4);
  for (let y = 0; y < th; y++) {
    for (let x = 0; x < tw; x++) {
      const k = (y + skip) * w + x + skip;
      const o = (y * tw + x) * 4;
      data[o] = (f1[k] * 255 + 0.5) | 0;
      data[o + 1] = (f2[k] * 255 + 0.5) | 0;
      data[o + 2] = (f3[k] * 255 + 0.5) | 0;
      data[o + 3] = (flow[k] * 255 + 0.5) | 0;
    }
  }
  // (Rock beside a shape too: the shader looks round there, the shape reaching into it.)
  for (let k = 0; k < nx * ny; k++) cells[k * 4 + 1] = index[k];
  // Shaped rooms' boxes and looks.
  const n = Math.max(1, slots.length);
  const shapes = new Uint8Array(n * 2 * 4);
  slots.forEach(({ box, shape, room }, s) => {
    shapes.set([box[0], box[1], box[2], box[3]], s * 4);
    shapes.set([shape, materialOf(it.function, lv.rooms[room].kind, it.theme), (room * 37) & 255, 255], (n + s) * 4);
  });
  // Caverns: their floor and the squares opening into them, blurred as a cave's.
  let cave: Uint8Array | null = null;
  if (caverns) {
    const [cw, ch] = [nx * R, ny * R];
    const mask = new Float32Array(cw * ch);
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        const k = j * nx + i;
        if (lv.cells[k] < 0) continue;
        const near = (a: number, b: number) => a >= 0 && b >= 0 && a < nx && b < ny && index[b * nx + a] === CAVERN;
        if (!(index[k] === CAVERN || near(i - 1, j) || near(i + 1, j) || near(i, j - 1) || near(i, j + 1))) continue;
        for (let y = j * R; y < (j + 1) * R; y++) mask.fill(1, y * cw + i * R, y * cw + (i + 1) * R);
      }
    }
    const b = blur(mask, cw, ch, 0.42 * R);
    cave = new Uint8Array(cw * ch * 4);
    for (let k = 0; k < cw * ch; k++) cave[k * 4] = (b[k] * 255 + 0.5) | 0;
  }
  return { w: tw, h: th, pad, data, cells, liquidKind, shapes, shapeCount: slots.length, cave, terrain: terrainField(it, level) };
}

/** Terrain's masks (`UnderField.terrain`): each kind's squares, spread into the rock next to
 * them (so water and chasms run right up to the walls), blurred for soft banks. */
function terrainField(it: Interior, level: number): Uint8Array | null {
  const lv = it.levels[level];
  const { nx, ny } = it;
  const items = lv.furniture.filter((f) => TERRAIN_CHANNEL[f.kind] !== undefined);
  if (!items.length) return null;
  const sq = new Uint8Array(nx * ny * 4);
  for (const f of items) {
    const c = TERRAIN_CHANNEL[f.kind];
    for (let j = Math.max(0, f.y); j < Math.min(ny, f.y + f.h); j++) {
      for (let i = Math.max(0, f.x); i < Math.min(nx, f.x + f.w); i++) {
        sq[(j * nx + i) * 4 + c] = 1;
        // (Deep water lies in water: the shallows' mask runs under it.)
        if (c === 1) sq[(j * nx + i) * 4] = 1;
      }
    }
  }
  // Rock squares take what the floor beside them has.
  const spread = sq.slice();
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      if (lv.cells[j * nx + i] >= 0) continue;
      for (let b = Math.max(0, j - 1); b <= Math.min(ny - 1, j + 1); b++) {
        for (let a = Math.max(0, i - 1); a <= Math.min(nx - 1, i + 1); a++) {
          if (lv.cells[b * nx + a] < 0) continue;
          for (let c = 0; c < 4; c++) spread[(j * nx + i) * 4 + c] |= sq[(b * nx + a) * 4 + c];
        }
      }
    }
  }
  const R = FIELD_RES;
  const [w, h] = [nx * R, ny * R];
  const out = new Uint8Array(w * h * 4);
  for (let c = 0; c < 4; c++) {
    if (!items.some((f) => TERRAIN_CHANNEL[f.kind] === c || (c === 0 && f.kind === 'water_deep'))) continue;
    const mask = new Float32Array(w * h);
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        if (!spread[(j * nx + i) * 4 + c]) continue;
        for (let y = j * R; y < (j + 1) * R; y++) mask.fill(1, y * w + i * R, y * w + (i + 1) * R);
      }
    }
    const b = blur(mask, w, h, (c === 2 ? 0.24 : 0.3) * R);
    for (let k = 0; k < w * h; k++) out[k * 4 + c] = (b[k] * 255 + 0.5) | 0;
  }
  return out;
}
