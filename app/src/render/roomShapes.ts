// Rooms drawn round, octagonal, with an apse, chamfered or as a cavern breaking into a built level
// (the generator's `interior/shape.rs`: the same geometry, from the box round a room's squares),
// and buildings drawn inside a footprint that isn't the grid's rectangle (`Interior.outline`).
// Only the drawing follows them: play keeps to the squares, so a curved wall may stand up to half
// a square off them. Geometry here is in grid units, polygons wound clockwise on screen (y down).
import type { Graphics } from 'pixi.js';
import { OPEN_BELOW, ROOF_SLOPE, type Interior, type InteriorLevel } from '../gen/protocol';
import { area2, wound, dedupe, clipConvex, segOutside, entryPoint, boundaryPieces, keptRuns, doorSquares, insetIn, insetParts, clipParts, spansIn, onWall, nearestPart, normalOut, depthTo, upTo, within, subtractConvex, offsetPart, roundIsh, WALL_HALF, type Pt, type Box } from './roomGeom';
export { clipConvex, segInside, segOutside, exitPoint, entryPoint, edgeAt, wallTangent, boundaryPieces, keptRuns, doorSquares, insetIn, insetParts, exitParts, onWall, within, subtractConvex, WALL_HALF } from './roomGeom';
export type { Pt, Box } from './roomGeom';


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


// ---------------------------------------------------------------------------------------
// Buildings: a level drawn with its shaped rooms inside its outer wall as drawn.

/** Drawn as solid wall: squares of the footprint no room has (the corners of a round room fitted
 * into a square plan) and what lies between a shaped room and the outer wall. */
export const MASS = -2;
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
  /** A round wall tower's outer face (`t:` interiors, no outline): its middle, the shape's radius and the face's. */
  disc?: { c: Pt; r0: number; r: number };
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
export function drawShapedFloors(g: Graphics, sp: ShapedPlan, lv: InteriorLevel, color: (room: number) => number, texture: (i: number, j: number, color: number) => [Pt, Pt][], decor?: () => void) {
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
  // (What lies on the floor under the grid: a yurt's rugs.)
  decor?.();
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
export function drawMass(g: Graphics, sp: ShapedPlan, color: number, lv?: InteriorLevel) {
  const pieces = [...massPieces(sp), ...(lv ? discRing(sp, lv) : [])];
  for (const piece of pieces) g.poly(flat(piece));
  if (pieces.length) g.fill(color);
}

/** Wall thickness of a round wall tower's outer face past its round room's shape (grid units). */
const TOWER_FACE = 0.58;
const DISC_SIDES = 48;

/** A round wall tower's outer face: the ring from the room's shape out to the circle, as convex
 * pieces; none where a floor of another room lies (a wall walk's squares leaving the tower). */
function discRing(sp: ShapedPlan, lv: InteriorLevel): Pt[][] {
  const d = sp.disc;
  if (!d) return [];
  const out: Pt[][] = [];
  const pt = (a: number, r: number): Pt => [d.c[0] + Math.cos(a) * r, d.c[1] + Math.sin(a) * r];
  for (let k = 0; k < DISC_SIDES; k++) {
    const [a0, a1] = [(k / DISC_SIDES) * Math.PI * 2, ((k + 1) / DISC_SIDES) * Math.PI * 2];
    const am = (a0 + a1) / 2;
    const m = pt(am, (d.r0 + d.r) / 2);
    const [i, j] = [Math.floor(m[0]), Math.floor(m[1])];
    const room = i >= 0 && j >= 0 && i < sp.nx && j < sp.ny ? lv.cells[j * sp.nx + i] : -1;
    if (room > 0 || (room === 0 && !sp.polys[0])) continue;
    // (Inner edge on the circle through the shape's corners; its chord's slivers are the squares' mass.)
    const rin = d.r0 * 0.995;
    out.push([pt(a0, rin), pt(a1, rin), pt(a1, d.r), pt(a0, d.r)]);
  }
  return out;
}

/** The solid wall's pieces (convex). */
export function massPieces(sp: ShapedPlan): Pt[][] {
  const { nx, ny, parts, polys } = sp;
  // (The shapes reach past their own squares, up to half a square: none is covered.)
  const shapes = polys.filter((p): p is Pt[] => !!p?.length).map((p) => ({ p, box: [Math.min(...p.map((q) => q[0])), Math.min(...p.map((q) => q[1])), Math.max(...p.map((q) => q[0])), Math.max(...p.map((q) => q[1]))] }));
  const out: Pt[][] = [];
  const d = sp.disc;
  const face: Pt[] | null = d ? Array.from({ length: DISC_SIDES }, (_, k): Pt => [d.c[0] + Math.cos((k / DISC_SIDES) * Math.PI * 2) * d.r, d.c[1] + Math.sin((k / DISC_SIDES) * Math.PI * 2) * d.r]) : null;
  for (let j = -1; j <= ny; j++) {
    for (let i = -1; i <= nx; i++) {
      if (shownAt(sp, i, j) !== MASS) continue;
      let pieces = clipParts(squareAt(i, j), parts);
      for (const { p, box } of shapes) if (box[0] < i + 1 && box[2] > i && box[1] < j + 1 && box[3] > j) pieces = pieces.flatMap((q) => subtractConvex(q, p));
      // (A round tower's own squares, outside its shape: only within the outer face.)
      if (face && i >= 0 && j >= 0 && i < nx && j < ny && sp.under[j * nx + i] !== -1) pieces = pieces.map((q) => clipConvex(q, face)).filter((q) => q.length);
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
  // The outer wall: each part's edges but where they run inside another part, open at the doors out
  // and along a town hall's open arcade (its piers drawn apart: `InteriorLayer.drawArcade`).
  const gaps = doors.filter((d) => d.out && d.outer).map((d) => d.outer!);
  const arcades = lv.walls.filter((w) => w.arcade);
  const onArcade = (m: Pt) =>
    arcades.some((w) => {
      const [ex, ey] = [w.b[0] - w.a[0], w.b[1] - w.a[1]];
      const l2 = ex * ex + ey * ey || 1;
      const t = ((m[0] - w.a[0]) * ex + (m[1] - w.a[1]) * ey) / l2;
      return t > 0 && t < 1 && Math.abs((m[0] - w.a[0]) * ey - (m[1] - w.a[1]) * ex) / Math.sqrt(l2) < 0.6;
    });
  const kept: { a: Pt; b: Pt; out: Pt }[] = [];
  parts.forEach((part, k) => {
    const pieces = boundaryPieces(part);
    const keep = (i: number) => !near(midOf(pieces[i]), gaps) && !onArcade(midOf(pieces[i])) && !parts.some((o, m) => m !== k && insetIn(midOf(pieces[i]), o) > 1e-4);

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
    // (A round wall tower: its outer face a circle, not the squares round its round room.)
    const rp = plan && !plan.real && it.id.startsWith('t:') ? plan.polys[0] : null;
    if (plan && rp && lv.rooms[0]?.shape === ROUND && rp.length >= 8) {
      const xs = rp.map((q) => q[0]);
      const ys = rp.map((q) => q[1]);
      const r0 = (Math.max(...xs) - Math.min(...xs) + Math.max(...ys) - Math.min(...ys)) / 4;
      plan.disc = { c: [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2], r0, r: r0 + TOWER_FACE };
    }
    const doors = plan ? lv.doors.map((d) => doorSegment(plan, lv, d)) : null;
    // (A yurt's door on the south point of its ring, as the battlemap draws it.)
    const ring = yurtRing(it, level);
    doors?.forEach((d, k) => {
      if (!ring || lv.doors[k].kind !== 'front') return;
      const m: Pt = [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
      const [sx, sy] = ring.south;
      const len = Math.hypot(m[0] - ring.c[0], m[1] - ring.c[1]) || 1;
      if (((m[0] - ring.c[0]) * sx + (m[1] - ring.c[1]) * sy) / len < 0.6) return;
      const p: Pt = [ring.c[0] + sx * ring.r, ring.c[1] + sy * ring.r];
      const [dx, dy] = [p[0] - m[0], p[1] - m[1]];
      const half = Math.hypot(d.b[0] - d.a[0], d.b[1] - d.a[1]) / 2;
      const t: Pt = (d.b[0] - d.a[0]) * -sy + (d.b[1] - d.a[1]) * sx >= 0 ? [-sy, sx] : [sy, -sx];
      const move = (q: Pt): Pt => [q[0] + dx, q[1] + dy];
      doors[k] = { ...d, a: [p[0] - t[0] * half, p[1] - t[1] * half], b: [p[0] + t[0] * half, p[1] + t[1] * half], outer: d.outer && move(d.outer), inner: d.inner && move(d.inner), threshold: d.threshold?.map(move), moved: true };
    });
    out = { plan, doors };
  }
  byLevel.set(level, out);
  return out;
}

/** A yurt's ring (a level holding a yurt, drawn inside one outline): its middle and mean radius (the
 * wall's line, grid units), and the way south on the map (unit, grid axes). Null for the rest. */
/** A yurt's room, and (R5 towns) a steppe town's round felt hall's (`interior::felt::ROOMS`). */
export const YURT_ROOMS = new Set(['yurt', 'felt hall', 'felt sanctuary', 'felt common room']);

export function yurtRing(it: Interior, level: number): { c: Pt; r: number; south: Pt } | null {
  const lv = it.levels[level];
  const o = it.outlines;
  if (!lv || !o || o.length !== 1 || o[0].length < 8 || !lv.rooms.some((r) => YURT_ROOMS.has(r.kind))) return null;
  const n = o[0].length;
  const c: Pt = [o[0].reduce((a, p) => a + p[0], 0) / n, o[0].reduce((a, p) => a + p[1], 0) / n];
  const r = o[0].reduce((a, p) => a + Math.hypot(p[0] - c[0], p[1] - c[1]), 0) / n;
  const s = Math.hypot(it.axis[1], it.across[1]) || 1;
  return { c, r, south: [it.axis[1] / s, it.across[1] / s] };
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
