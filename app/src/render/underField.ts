// The textures an underground level is painted from (`underground.frag`): a smooth floor field
// (4 texels a square, blurred), liquids and raised floors likewise, per-square materials, and
// terrain (water, chasms, rubble slopes) likewise.
// Pure data, so a city's sewer sections can be prepared in a worker (`underField.worker.ts`)
// while the view moves.
import { floorEdges, floorFt, floorOf } from '../gen/floor';
import type { Interior, InteriorLevel } from '../gen/protocol';
import { CAVERN, fromSquares, shapeIndex } from './roomShapes';
import { rockDistance } from './rockDistance';

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
  /** Squares to the nearest floor (`rockDistance`: r8, `DIST_PAD` squares past the grid each way). */
  dist: Uint8Array;
  /** Ledges and steps (`liftField`), else null. */
  lift: Lift | null;
}

/** The edges between floor heights (`liftField`): (nx * res) x (ny * res), RGBA: signed distance to the nearest
 * edge (squares, + on its high side; 128 + d * LIFT_SCALE), its rise (ft / LIFT_FT), how strongly it is drawn (0
 * on ramps, rising to 1 within RAMP_TAPER of one, so the way up reads); a on and a texel round ramps 1 + 254 t, t how
 * far up it (0 at the lower floor's edge; g there the ramp's rise), else 0. Rock stays rock: cave ledges
 * open onto their floors in the generator (`cave::open_ledges`), so a ledge's drop is a floor-to-floor edge. */
export interface Lift {
  data: Uint8Array;
  res: number;
}

/** The lift field's distances: to +-LIFT_REACH squares, LIFT_SCALE a square. */
const LIFT_REACH = 2;
const LIFT_SCALE = 63.5;
/** How far (squares) a cliff tapers away beside a ramp. */
const RAMP_TAPER = 0.4;
/** The highest rise it tells apart (ft). */
const LIFT_FT = 20;

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
 * 4 flagstones, 5 boss flagstones, 6 the cult's flagstones, 7 marble, 8 hewn blocks, 9 ice, 10 the cult's dark ring flagstones,
 * 11 (v63 underground) crypt flagstones, 12 a sunken arena's packed sand (`sunkenPits`). Flagstones (4-6, 10, 11) take their stone from the site's theme (`underTheme.ts`). */
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
  if (site === 'crypt' || site === 'catacombs') return 11;
  return 4;
}

/** A boss chamber's sunken arena (`under::boss`: most of its floor raised round a pit at the level's own height):
 * the pit's squares (4 or more together at the bottom, so not the doorway's square), 1 where the floor is the
 * fighting pit's packed sand (the shader's material 12). */
function sunkenPits(lv: InteriorLevel, nx: number, ny: number): Uint8Array {
  const out = new Uint8Array(nx * ny);
  if (!floorOf(lv, nx, ny)) return out;
  for (let r = 0; r < lv.rooms.length; r++) {
    if (lv.rooms[r].kind !== 'boss chamber') continue;
    let all = 0;
    let raised = 0;
    for (let k = 0; k < nx * ny; k++) {
      if (lv.cells[k] !== r) continue;
      all++;
      if (floorFt(lv, nx, ny, k % nx, Math.floor(k / nx)) > 3) raised++;
    }
    if (raised * 5 <= all * 2) continue;
    const low = (k: number) => lv.cells[k] === r && !out[k] && floorFt(lv, nx, ny, k % nx, Math.floor(k / nx)) < 0.5;
    const seen = new Uint8Array(nx * ny);
    for (let k0 = 0; k0 < nx * ny; k0++) {
      if (seen[k0] || !low(k0)) continue;
      const part = [k0];
      seen[k0] = 1;
      for (let n = 0; n < part.length; n++) {
        const k = part[n];
        const i = k % nx;
        for (const m of [i > 0 ? k - 1 : -1, i + 1 < nx ? k + 1 : -1, k - nx, k + nx]) {
          if (m >= 0 && m < nx * ny && !seen[m] && low(m)) {
            seen[m] = 1;
            part.push(m);
          }
        }
      }
      if (part.length >= 4) for (const k of part) out[k] = 1;
    }
  }
  return out;
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

/** `offLine(...)[0] <= 0` without the array (the same sums: called a great many times a section). */
function inLine(l: number[], px: number, py: number): boolean {
  const x0 = l[0];
  const y0 = l[1];
  const dx = l[2] - x0;
  const dy = l[3] - y0;
  const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / Math.max(dx * dx + dy * dy, 1e-9)));
  const ex = px - x0 - t * dx;
  const ey = py - y0 - t * dy;
  return Math.max(Math.hypot(ex, ey), 1e-9) - l[4] <= 0;
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
  // Only the lines within reach: every point looked at lies within WALL_REACH of (cx, cy), so a
  // line farther than that past its own reach (plus the edge test's 0.02) can't hold it or edge it.
  // (In their order, so the nearest edge wins the same ties.)
  const paths = (lv.paths ?? []).filter((l) => offLine(l, cx, cy)[0] <= WALL_REACH + 0.05);
  const onFloor = (px: number, py: number) => {
    const [i, j] = [Math.floor(px), Math.floor(py)];
    if (i >= 0 && j >= 0 && i < nx && j < ny) {
      const k = j * nx + i;
      const r = lv.cells[k];
      if (r >= 0 && lv.rooms[r].kind !== LINE_ROOM) return true;
      if (!near[k]) return false;
    }
    for (const l of paths) if (inLine(l, px, py)) return true;
    return false;
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

/** Foam in a sewer's channel (in its squares' blue, which brick doesn't use: 0-248 in steps of 8,
 * the city's stonework, `Interior.stone`, in the low 3 bits, on the rock round it too): round where
 * runs meet (three or more ends together), where one ends, and at the outfalls, fading over
 * `FOAM_REACH` squares. */
const FOAM_REACH = 2.2;
function sewerFoam(lv: InteriorLevel, paths: number[][], nx: number, ny: number, cells: Uint8Array, stone: number) {
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
  for (let k = 0; k < nx * ny; k++) {
    if (cells[k * 4 + 3] !== 0 && cells[k * 4] !== 3) continue;
    cells[k * 4 + 2] = ((foamed.has(k) ? cells[k * 4 + 2] : 0) & 0xf8) | (stone & 7);
  }
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
  const sand = sunkenPits(lv, nx, ny);
  const { index, shapes: slots, caverns } = shapeIndex(lv, nx, ny);
  let anyRaised = false;
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const k = j * nx + i;
      const r = lv.cells[k];
      if (r < 0) continue;
      const room = lv.rooms[r];
      cells[k * 4] = room.shape === CAVERN ? 0 : sand[k] ? 12 : materialOf(it.function, room.kind, it.theme);
      cells[k * 4 + 1] = index[k];
      cells[k * 4 + 2] = (r * 37) & 255;
      cells[k * 4 + 3] = 255;
      // Floors that follow lines (sewers under streets) are painted from the lines below; shaped
      // rooms from their shapes; anything else fills its squares.
      const square = (!paths.length || room.kind !== LINE_ROOM) && fromSquares(index[k]);
      const lift = Math.min(1, floorFt(lv, nx, ny, i, j) / LIFT_FT);
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
    sewerFoam(lv, paths, nx, ny, cells, it.stone ?? 0);
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
  // How far the rock lies from the floor (hatched beside the walls, fading out beyond).
  const dist = rockDistance(f1, w, h, R, rim, nx, ny, (i, j) => lv.cells[j * nx + i] >= 0 && !fromSquares(index[j * nx + i]));
  return { w: tw, h: th, pad, data, cells, liquidKind, shapes, shapeCount: slots.length, cave, terrain: terrainField(it, level), dist, lift: liftField(it, level) };
}

/**
 * Ledges and steps (`UnderField.lift`): every edge between floor heights (`floorEdges`; ramps have none) as a signed
 * distance field, so the shader inks the high side's lip along it, the cliff face below it (as wide as the drop is
 * high) and the shadow beyond. Built sites measure square by square (Chebyshev: square corners, at 8 texels a
 * square); natural rock by the straight distance, blurred as its walls are (4 a square). Null where it is flat and
 * in sewers.
 */
function liftField(it: Interior, level: number): Lift | null {
  const lv = it.levels[level];
  const { nx, ny } = it;
  if (lv.paths?.length) return null;
  const f = floorOf(lv, nx, ny);
  if (!f) return null;
  const { v, h } = floorEdges(f, lv);
  // (Only rubble slopes are drawn as slopes; steps and ladders are plain ramps, under their own sprites.)
  const rubble = new Uint8Array(nx * ny);
  for (const g of lv.furniture) if (g.kind === 'rubble_slope') for (let y = g.y; y < Math.min(ny, g.y + g.h); y++) for (let x = g.x; x < Math.min(nx, g.x + g.w); x++) rubble[y * nx + x] = 1;
  const slope = (k: number) => f.ramp[k] && rubble[k] === 1;
  // (i, j, 1 on the line x = i over row j / 0 on y = j over column i, rise: the east or south side's minus the other's,
  // the west or north side's height.)
  const segs: [number, number, number, number, number][] = [];
  for (let j = 0; j < ny; j++) for (let i = 0; i <= nx; i++) if (v[j * (nx + 1) + i]) segs.push([i, j, 1, v[j * (nx + 1) + i], f.ft[j * nx + i - 1]]);
  for (let j = 0; j <= ny; j++) for (let i = 0; i < nx; i++) if (h[j * nx + i]) segs.push([i, j, 0, h[j * nx + i], f.ft[(j - 1) * nx + i]]);
  const square = SQUARE_SITES.has(it.function);
  const R = square ? 8 : 4;
  const [w, hh] = [nx * R, ny * R];
  if (!segs.length && !f.ramp.some((r) => r)) return null;
  const best = new Float32Array(w * hh).fill(Infinity);
  const dist = new Float32Array(w * hh);
  const rise = new Float32Array(w * hh);
  for (const [i, j, vert, r, ft0] of segs) {
    const [x0, y0] = [i - LIFT_REACH, j - LIFT_REACH];
    const [x1, y1] = vert ? [i + LIFT_REACH, j + 1 + LIFT_REACH] : [i + 1 + LIFT_REACH, j + LIFT_REACH];
    for (let ty = Math.max(0, Math.floor(y0 * R)); ty < Math.min(hh, Math.ceil(y1 * R)); ty++) {
      for (let tx = Math.max(0, Math.floor(x0 * R)); tx < Math.min(w, Math.ceil(x1 * R)); tx++) {
        const [px, py] = [(tx + 0.5) / R, (ty + 0.5) / R];
        const [perp, a, a0] = vert ? [px - i, py, j] : [py - j, px, i];
        const over = Math.max(0, a0 - a, a - a0 - 1);
        const d = square ? Math.max(Math.abs(perp), over) : Math.hypot(perp, over);
        // (Ties go to the edge alongside: past an edge's end its side may not be the texel's.)
        const key = d + 1e-3 * over;
        const k = ty * w + tx;
        if (d >= LIFT_REACH || key >= best[k]) continue;
        best[k] = key;
        // (Its side: a floor texel's by its own square's height where that is the edge's high or low floor, so one
        // past the edge's end, on a floor of the low side's height, is never taken for the high side; else by the line.
        // Taken for the high side, it would carry that side over its whole floor (below), and where that met the low
        // side, out of reach of any edge, a lip was drawn across the floor with no drop.)
        const s = Math.min(ny - 1, Math.floor(py)) * nx + Math.min(nx - 1, Math.floor(px));
        let high = perp > 0 === r > 0;
        if (lv.cells[s] >= 0 && !f.ramp[s]) {
          if (f.ft[s] >= Math.max(ft0, ft0 + r)) high = true;
          else if (f.ft[s] <= Math.min(ft0, ft0 + r)) high = false;
        }
        dist[k] = high ? d : -d;
        rise[k] = Math.abs(r);
      }
    }
  }
  // Texels out of reach take their square's side (so no edge shows inside a floor): a square's from its middle texel
  // where that is in reach, else from the floor of the same height it joins (4 ways), then the rock from what is
  // beside it; else low.
  const sq = new Int8Array(nx * ny);
  const todo: number[] = [];
  for (let k = 0; k < nx * ny; k++) {
    const t = (Math.floor(k / nx) * R + (R >> 1)) * w + (k % nx) * R + (R >> 1);
    if (best[t] < Infinity) {
      sq[k] = dist[t] > 0 ? 1 : -1;
      todo.push(k);
    }
  }
  for (const floors of [true, false]) {
    const queue = floors ? todo : Array.from(sq.keys()).filter((k) => sq[k]);
    for (let q = 0; q < queue.length; q++) {
      const k = queue[q];
      const [i, j] = [k % nx, Math.floor(k / nx)];
      for (const m of [i > 0 ? k - 1 : -1, i + 1 < nx ? k + 1 : -1, j > 0 ? k - nx : -1, j + 1 < ny ? k + nx : -1]) {
        if (m < 0 || sq[m]) continue;
        if (floors ? lv.cells[k] >= 0 && lv.cells[m] >= 0 && f.ft[m] === f.ft[k] : lv.cells[m] < 0) {
          sq[m] = sq[k];
          queue.push(m);
        }
      }
    }
  }
  for (let k = 0; k < w * hh; k++) if (best[k] === Infinity) dist[k] = (sq[Math.floor(Math.floor(k / w) / R) * nx + Math.floor((k % w) / R)] || -1) * LIFT_REACH;
  // Ramps: no lip, and the cliff beside one tapers to it (0 on the ramp, 1 from RAMP_TAPER squares off it), so it
  // ends cleanly where the way up starts. (Unblurred: the taper is the soft part.)
  const strength = new Float32Array(w * hh).fill(1);
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      if (!f.ramp[j * nx + i]) continue;
      for (let ty = Math.max(0, Math.floor((j - RAMP_TAPER) * R)); ty < Math.min(hh, Math.ceil((j + 1 + RAMP_TAPER) * R)); ty++) {
        for (let tx = Math.max(0, Math.floor((i - RAMP_TAPER) * R)); tx < Math.min(w, Math.ceil((i + 1 + RAMP_TAPER) * R)); tx++) {
          const [px, py] = [(tx + 0.5) / R, (ty + 0.5) / R];
          const d = Math.hypot(Math.max(0, i - px, px - i - 1), Math.max(0, j - py, py - j - 1));
          const k = ty * w + tx;
          strength[k] = Math.min(strength[k], Math.min(1, d / RAMP_TAPER));
        }
      }
    }
  }
  // Ramps, so the shader draws each as a slope: how far up it a texel is (a, 1 + 254 t: t 0 on the edge it meets the
  // lower floor at, 1 on the higher's; spread a texel past the ramp, so filtering inside it reads only ramp texels),
  // and on it (g) the rise between its two floors, its low 3 bits the way up (one of eight, 45 degrees each from +x,
  // y down; one per run of ramp squares), read exactly by texelFetch (off by at most 0.6 ft as a rise).
  const segsOf = (i: number, j: number, k: number) => {
    const out: [number, number, number, number, number][] = [];
    for (const [di, dj] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
      const [a, b] = [i + di, j + dj];
      if (a < 0 || b < 0 || a >= nx || b >= ny) continue;
      const m = b * nx + a;
      if (lv.cells[m] < 0 || f.ramp[m] || f.ft[m] === f.ft[k]) continue;
      // (The side shared with that square, and that square's height.)
      const [x0, y0] = [di > 0 ? i + 1 : i, dj > 0 ? j + 1 : j];
      out.push([x0, y0, di ? x0 : x0 + 1, dj ? y0 : y0 + 1, f.ft[m]]);
    }
    return out;
  };
  const rampSides: [number, number, number, number, number, number][] = [];
  for (let k = 0; k < nx * ny; k++) if (slope(k)) for (const sg of segsOf(k % nx, Math.floor(k / nx), k)) rampSides.push([...sg, f.ft[k]]);
  const rampT = new Float32Array(w * hh).fill(-1);
  const rampCode = new Uint8Array(w * hh);
  const onRamp = new Uint8Array(w * hh);
  // (Each run: its way up from the sides it meets higher floors on, less those it meets lower ones on; its rise.)
  const runCode = new Uint8Array(nx * ny);
  const seen = new Uint8Array(nx * ny);
  for (let k0 = 0; k0 < nx * ny; k0++) {
    if (!f.ramp[k0] || seen[k0]) continue;
    const run = [k0];
    seen[k0] = 1;
    let [vx, vy, lo, hi] = [0, 0, Infinity, -Infinity];
    for (let r = 0; r < run.length; r++) {
      const k = run[r];
      const [i, j] = [k % nx, Math.floor(k / nx)];
      for (const [di, dj] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
        const [a, b] = [i + di, j + dj];
        if (a < 0 || b < 0 || a >= nx || b >= ny) continue;
        const m = b * nx + a;
        if (lv.cells[m] < 0) continue;
        if (f.ramp[m]) {
          if (!seen[m]) {
            seen[m] = 1;
            run.push(m);
          }
          continue;
        }
        const sgn = Math.sign(f.ft[m] - f.ft[k]);
        [vx, vy] = [vx + sgn * di, vy + sgn * dj];
        if (sgn < 0) lo = Math.min(lo, f.ft[m]);
        if (sgn > 0) hi = Math.max(hi, f.ft[m]);
      }
    }
    const way = (Math.round(Math.atan2(vy, vx) / (Math.PI / 4)) + 8) % 8;
    const riseFt = lo < Infinity && hi > -Infinity ? hi - lo : lo < Infinity || hi > -Infinity ? 2 * Math.abs(f.ft[k0] - (lo < Infinity ? lo : hi)) : 0;
    const code = Math.min(248, Math.round((riseFt / LIFT_FT) * 255 / 8) * 8) + way;
    for (const k of run) runCode[k] = code;
  }
  if (rampSides.length) {
    const segDist = (px: number, py: number, s: number[]) => Math.hypot(px - Math.min(Math.max(px, s[0]), s[2]), py - Math.min(Math.max(py, s[1]), s[3]));
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        if (!slope(j * nx + i)) continue;
        const near = rampSides.filter((s) => s[0] >= i - 3 && s[2] <= i + 4 && s[1] >= j - 3 && s[3] <= j + 4);
        for (let ty = Math.max(0, j * R - 1); ty < Math.min(hh, (j + 1) * R + 1); ty++) {
          for (let tx = Math.max(0, i * R - 1); tx < Math.min(w, (i + 1) * R + 1); tx++) {
            const [px, py] = [(tx + 0.5) / R, (ty + 0.5) / R];
            let [dLo, dHi] = [Infinity, Infinity];
            for (const s of near) {
              const d = segDist(px, py, s);
              if (s[4] < s[5]) dLo = Math.min(dLo, d);
              if (s[4] > s[5]) dHi = Math.min(dHi, d);
            }
            if (dLo === Infinity && dHi === Infinity) continue;
            const k = ty * w + tx;
            const inside = tx >= i * R && tx < (i + 1) * R && ty >= j * R && ty < (j + 1) * R;
            if (!inside && onRamp[k]) continue;
            if (inside) onRamp[k] = 1;
            rampT[k] = dHi === Infinity ? 1 : dLo === Infinity ? 0 : dLo / (dLo + dHi);
            if (inside) rampCode[k] = runCode[j * nx + i];
          }
        }
      }
    }
  }
  let [dd, rr]: Float32Array[] = [dist, rise];
  if (!square) {
    const sigma = 0.36 * R;
    [dd, rr] = [blur(dist, w, hh, sigma), blur(rise, w, hh, sigma)];
  }
  const data = new Uint8Array(w * hh * 4);
  for (let k = 0; k < w * hh; k++) {
    data[k * 4] = Math.max(1, Math.min(255, Math.round(128 + dd[k] * LIFT_SCALE)));
    data[k * 4 + 1] = Math.min(255, Math.round((rr[k] / LIFT_FT) * 255));
    data[k * 4 + 2] = Math.round(strength[k] * 255);
    if (rampT[k] >= 0) data[k * 4 + 3] = 1 + Math.round(rampT[k] * 254);
    if (onRamp[k]) data[k * 4 + 1] = rampCode[k];
  }
  return { data, res: R };
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
