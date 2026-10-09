// The designer's changes to a site design (`SiteDesign`, mirroring
// crates/worldgen/src/under/design.rs): each takes the design and changes it in place. Squares
// are run-length encoded in the design; `Level` holds one level decoded while it changes.
// Walls, doors' rooms and the rules a site must keep are worked out by the generator (Ask op
// `design`), never here.
import type { DesignLevel, SiteDesign } from '../../gen/protocol';

/** Items that are ways: in from the surface, up and down a level. */
export const WAYS = ['exit', 'up', 'down'];
export const BOSS = 'boss chamber';
/** Feet a level lies below the one above. */
const LEVEL_FT = 20;

export type Item = DesignLevel['items'][number];

/** Room per square from runs. */
export function decode(runs: number[], n: number): Int16Array {
  const out = new Int16Array(n).fill(-1);
  let k = 0;
  for (let i = 0; i + 1 < runs.length && k < n; i += 2) {
    out.fill(runs[i], k, Math.min(n, k + runs[i + 1]));
    k += runs[i + 1];
  }
  return out;
}

export function encode(cells: Int16Array): number[] {
  const out: number[] = [];
  for (const c of cells) {
    if (out.length && out[out.length - 2] === c) out[out.length - 1]++;
    else out.push(c, 1);
  }
  return out;
}

/** One level's squares, changed and written back. */
export function withCells(d: SiteDesign, li: number, f: (cells: Int16Array, lv: DesignLevel) => void) {
  const lv = d.levels[li];
  const cells = decode(lv.cells, d.nx * d.ny);
  f(cells, lv);
  lv.cells = encode(cells);
}

/** Whether an item covers square (x, y). */
export const covers = (f: Item, x: number, y: number) => x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h;

/** A new room on a level (always added: an emptied room keeps its index, and its name). */
export function addRoom(d: SiteDesign, li: number, kind: string, raise_ft = 0): number {
  d.levels[li].rooms.push({ kind, raise_ft });
  return d.levels[li].rooms.length - 1;
}

/** Squares given to `room` (-1: rock). Props on squares turned to rock go with them. */
export function paint(d: SiteDesign, li: number, squares: Iterable<number>, room: number) {
  const nx = d.nx;
  const rock: number[] = [];
  withCells(d, li, (cells) => {
    for (const k of squares) {
      if (k < 0 || k >= cells.length) continue;
      cells[k] = room;
      if (room < 0) rock.push(k);
    }
  });
  if (rock.length) {
    const lv = d.levels[li];
    lv.items = lv.items.filter((f) => WAYS.includes(f.kind) || !rock.some((k) => covers(f, k % nx, Math.floor(k / nx))));
  }
}

/** The squares of a rectangle between two corners (inclusive), inside the grid. */
export function rectSquares(d: SiteDesign, x0: number, y0: number, x1: number, y1: number): number[] {
  const out: number[] = [];
  const [a, b] = [Math.max(0, Math.min(x0, x1)), Math.min(d.nx - 1, Math.max(x0, x1))];
  const [c, e] = [Math.max(0, Math.min(y0, y1)), Math.min(d.ny - 1, Math.max(y0, y1))];
  for (let j = c; j <= e; j++) for (let i = a; i <= b; i++) out.push(j * d.nx + i);
  return out;
}

/** A door on an edge: none → door → secret door → none. */
export function cycleDoor(d: SiteDesign, li: number, x: number, y: number, side: number) {
  const lv = d.levels[li];
  const at = lv.doors.findIndex((e) => e[0] === x && e[1] === y && e[2] === side);
  if (at < 0) lv.doors.push([x, y, side, 0]);
  else if (lv.doors[at][3] === 0) lv.doors[at] = [x, y, side, 1];
  else lv.doors.splice(at, 1);
}

/** The prop at a square (not a way), if any. */
export function propAt(d: SiteDesign, li: number, x: number, y: number): number {
  return d.levels[li].items.findIndex((f) => !WAYS.includes(f.kind) && covers(f, x, y));
}

/** Put a prop down where its squares are floor and free; false if they aren't. */
export function addProp(d: SiteDesign, li: number, kind: string, x: number, y: number, w: number, h: number): boolean {
  const lv = d.levels[li];
  if (x < 0 || y < 0 || x + w > d.nx || y + h > d.ny) return false;
  const cells = decode(lv.cells, d.nx * d.ny);
  for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) if (cells[j * d.nx + i] < 0 || lv.items.some((f) => covers(f, i, j))) return false;
  lv.items.push({ kind, x, y, w, h });
  return true;
}

/** The way down from level `li` at (x, y), and the way up onto the level below under it (the
 * square there made floor, a landing round it, if it was rock). */
export function setWayDown(d: SiteDesign, li: number, x: number, y: number) {
  if (li <= 0) return;
  const lv = d.levels[li];
  lv.items = lv.items.filter((f) => f.kind !== 'down' && !covers(f, x, y));
  lv.items.push({ kind: 'down', x, y, w: 1, h: 1 });
  const below = d.levels[li - 1];
  below.items = below.items.filter((f) => f.kind !== 'up' && !covers(f, x, y));
  below.items.unshift({ kind: 'up', x, y, w: 1, h: 1 });
  landing(d, li - 1, x, y);
}

/** Rock round (x, y) on a level made a landing (3 × 3, inside the grid), if (x, y) is rock. */
function landing(d: SiteDesign, li: number, x: number, y: number) {
  const nx = d.nx;
  const cells = decode(d.levels[li].cells, nx * d.ny);
  if (cells[y * nx + x] >= 0) return;
  const room = addRoom(d, li, 'landing');
  paint(
    d,
    li,
    rectSquares(d, Math.max(1, x - 1), Math.max(1, y - 1), Math.min(nx - 2, x + 1), Math.min(d.ny - 2, y + 1)).filter((k) => cells[k] < 0),
    room,
  );
}

/** Its default name: "Level 2 · 40 ft down", or "· the deep" for the deepest. */
function levelName(depth: number, deepest: boolean): string {
  return deepest ? `Level ${depth} · the deep` : `Level ${depth} · ${depth * LEVEL_FT} ft down`;
}
const DEFAULT_NAME = /^Level \d+ · (the deep|\d+ ft down)$/;

/** A level dug below the deepest: a landing round its way up, under a way down from the level
 * above (on its free floor nearest its middle). */
export function addLevel(d: SiteDesign) {
  const n = d.levels.length;
  const deep = d.levels[0];
  const { nx, ny } = d;
  const cells = decode(deep.cells, nx * ny);
  const free = (k: number) => {
    const [x, y] = [k % nx, Math.floor(k / nx)];
    return cells[k] >= 0 && x > 1 && y > 1 && x < nx - 2 && y < ny - 2 && !deep.items.some((f) => covers(f, x, y)) && !deep.doors.some((e) => (e[0] === x && e[1] === y) || (e[2] === 0 ? e[0] + 1 === x && e[1] === y : e[0] === x && e[1] + 1 === y));
  };
  let [sx, sy, c] = [0, 0, 0];
  cells.forEach((r, k) => {
    if (r >= 0) [sx, sy, c] = [sx + (k % nx), sy + Math.floor(k / nx), c + 1];
  });
  const [mx, my] = c ? [sx / c, sy / c] : [nx / 2, ny / 2];
  let best = -1;
  for (let k = 0; k < cells.length; k++) {
    if (free(k) && (best < 0 || Math.hypot((k % nx) - mx, Math.floor(k / nx) - my) < Math.hypot((best % nx) - mx, Math.floor(best / nx) - my))) best = k;
  }
  if (best < 0) return false;
  if (DEFAULT_NAME.test(deep.name)) deep.name = levelName(n, false);
  d.levels.unshift({ name: levelName(n + 1, true), elevation_ft: deep.elevation_ft - LEVEL_FT, natural: deep.natural, cells: [-1, nx * ny], rooms: [], doors: [], items: [] });
  setWayDown(d, 1, best % nx, Math.floor(best / nx));
  return true;
}

/** The deepest level filled in (a site keeps one level at least). */
export function removeLevel(d: SiteDesign) {
  if (d.levels.length <= 1) return;
  d.levels.shift();
  const deep = d.levels[0];
  deep.items = deep.items.filter((f) => f.kind !== 'down');
  if (DEFAULT_NAME.test(deep.name)) deep.name = levelName(d.levels.length, true);
}

// ---------------------------------------------------------------------------------------
// Buildings (`kind: 'building'`, crates/worldgen/src/interior/design.rs): every square inside
// the walls is some room; walls are where rooms meet, so a wall line splits a room and taking a
// wall away joins two. Doors to the outside are given from their room's square, with a side
// (0 east, 1 south, 2 west, 3 north); flags 2 front, 4 back.

export const FRONT = 2;
export const BACK = 4;

export const isBuilding = (d: SiteDesign | null | undefined) => d?.kind === 'building';

/** An edge on the grid lines: the squares either side (either maybe off the grid). */
export type Edge = { a: [number, number]; b: [number, number] };

const onGrid = (d: SiteDesign, [i, j]: [number, number]) => i >= 0 && j >= 0 && i < d.nx && j < d.ny;

/** A door's place for an edge: [x, y, side] from the square with a room (both rooms: the
 * upper or left one, side 0 or 1). */
export function doorPlace(d: SiteDesign, cells: Int16Array, e: Edge): [number, number, number] | null {
  const room = (s: [number, number]) => (onGrid(d, s) ? cells[s[1] * d.nx + s[0]] : -1);
  const [a, b] = e.a[0] + e.a[1] <= e.b[0] + e.b[1] ? [e.a, e.b] : [e.b, e.a];
  const side = (from: [number, number], to: [number, number]) => (to[0] > from[0] ? 0 : to[1] > from[1] ? 1 : to[0] < from[0] ? 2 : 3);
  if (room(a) >= 0) return [a[0], a[1], side(a, b)];
  if (room(b) >= 0) return [b[0], b[1], side(b, a)];
  return null;
}

/** The squares a door at [x, y, side] lies between. */
export function doorEdge(x: number, y: number, side: number): Edge {
  const o = [[1, 0], [0, 1], [-1, 0], [0, -1]][side] ?? [1, 0];
  return { a: [x, y], b: [x + o[0], y + o[1]] };
}

const sameEdge = (p: Edge, q: Edge) =>
  (p.a[0] === q.a[0] && p.a[1] === q.a[1] && p.b[0] === q.b[0] && p.b[1] === q.b[1]) || (p.a[0] === q.b[0] && p.a[1] === q.b[1] && p.b[0] === q.a[0] && p.b[1] === q.a[1]);

/** The door on an edge, if any (its index). */
export function doorOn(d: SiteDesign, li: number, e: Edge): number {
  return d.levels[li].doors.findIndex((x) => sameEdge(doorEdge(x[0], x[1], x[2]), e));
}

/** A door to the outside on an edge: none → back door → front door (the level's front door
 * before it becomes a back door) → none. */
export function cycleOuterDoor(d: SiteDesign, li: number, x: number, y: number, side: number) {
  const lv = d.levels[li];
  const at = doorOn(d, li, doorEdge(x, y, side));
  const flags = at >= 0 ? lv.doors[at][3] : -1;
  if (at < 0) lv.doors.push([x, y, side, BACK]);
  else if (flags & FRONT) lv.doors.splice(at, 1);
  else {
    for (const e of lv.doors) if (e[3] & FRONT) e[3] = BACK;
    lv.doors[at] = [x, y, side, FRONT];
  }
}

/** Unit edges (as square pairs) along a grid line from corner p to corner q (axis-aligned). */
function lineEdges(p: [number, number], q: [number, number]): Edge[] {
  const out: Edge[] = [];
  if (p[1] === q[1]) for (let i = Math.min(p[0], q[0]); i < Math.max(p[0], q[0]); i++) out.push({ a: [i, p[1] - 1], b: [i, p[1]] });
  else if (p[0] === q[0]) for (let j = Math.min(p[1], q[1]); j < Math.max(p[1], q[1]); j++) out.push({ a: [p[0] - 1, j], b: [p[0], j] });
  return out;
}

/** A wall line from corner p to corner q (grid points, axis-aligned): each edge on it between
 * squares of one room, and it runs on along the grid line to that room's walls. Returns the
 * edges walled. */
export function wallLine(d: SiteDesign, cells: Int16Array, p: [number, number], q: [number, number]): Edge[] {
  const room = (s: [number, number]) => (onGrid(d, s) ? cells[s[1] * d.nx + s[0]] : -1);
  const inner = (e: Edge) => room(e.a) >= 0 && room(e.a) === room(e.b);
  const edges = lineEdges(p, q).filter(inner);
  if (!edges.length) return [];
  // Run on both ways to the walls of the rooms it crosses.
  const horizontal = p[1] === q[1];
  const step = (e: Edge, k: number): Edge => (horizontal ? { a: [e.a[0] + k, e.a[1]], b: [e.b[0] + k, e.b[1]] } : { a: [e.a[0], e.a[1] + k], b: [e.b[0], e.b[1] + k] });
  const out = [...edges];
  for (const [from, k] of [[edges[0], -1], [edges[edges.length - 1], 1]] as const) {
    let e = step(from, k);
    while (inner(e) && room(e.a) === room(from.a)) {
      out.push(e);
      e = step(e, k);
    }
  }
  return out;
}

/** Rooms on level `li` cut by `walls` (edges inside a room) split: each piece but the biggest a
 * room of its own, of the same kind. The new rooms' indices. */
export function splitRooms(d: SiteDesign, li: number, walls: Edge[]): number[] {
  const { nx, ny } = d;
  const lv = d.levels[li];
  const cut = new Set(walls.flatMap((e) => [`${e.a[0]},${e.a[1]}|${e.b[0]},${e.b[1]}`, `${e.b[0]},${e.b[1]}|${e.a[0]},${e.a[1]}`]));
  const made: number[] = [];
  withCells(d, li, (cells) => {
    const rooms = new Set(walls.map((e) => cells[e.a[1] * nx + e.a[0]]));
    for (const r of rooms) {
      const piece = new Int32Array(nx * ny).fill(-1);
      const sizes: number[] = [];
      for (let k = 0; k < nx * ny; k++) {
        if (cells[k] !== r || piece[k] >= 0) continue;
        const id = sizes.length;
        const stack = [k];
        piece[k] = id;
        let n = 0;
        while (stack.length) {
          const s = stack.pop()!;
          n++;
          const [i, j] = [s % nx, Math.floor(s / nx)];
          for (const [a, b] of [[i + 1, j], [i - 1, j], [i, j + 1], [i, j - 1]]) {
            if (a < 0 || b < 0 || a >= nx || b >= ny) continue;
            const t = b * nx + a;
            if (cells[t] !== r || piece[t] >= 0 || cut.has(`${i},${j}|${a},${b}`)) continue;
            piece[t] = id;
            stack.push(t);
          }
        }
        sizes.push(n);
      }
      if (sizes.length < 2) continue;
      const keep = sizes.indexOf(Math.max(...sizes));
      const index = sizes.map((_, p) => (p === keep ? r : (lv.rooms.push({ kind: lv.rooms[r].kind, raise_ft: lv.rooms[r].raise_ft }), lv.rooms.length - 1)));
      for (const p of index) if (p !== r) made.push(p);
      for (let k = 0; k < nx * ny; k++) if (piece[k] >= 0) cells[k] = index[piece[k]];
    }
  });
  return made;
}

/** The wall on an edge between two rooms taken away: the smaller room joins the bigger (the
 * doors between them go). False if the edge is not between two rooms. */
export function mergeRooms(d: SiteDesign, li: number, e: Edge): boolean {
  const { nx } = d;
  const lv = d.levels[li];
  let done = false;
  withCells(d, li, (cells) => {
    const room = (s: [number, number]) => (onGrid(d, s) ? cells[s[1] * nx + s[0]] : -1);
    const [ra, rb] = [room(e.a), room(e.b)];
    if (ra < 0 || rb < 0 || ra === rb) return;
    const size = (r: number) => cells.reduce((n, c) => n + (c === r ? 1 : 0), 0);
    const [keep, gone] = size(ra) >= size(rb) ? [ra, rb] : [rb, ra];
    for (let k = 0; k < cells.length; k++) if (cells[k] === gone) cells[k] = keep;
    lv.doors = lv.doors.filter((x) => {
      const de = doorEdge(x[0], x[1], x[2]);
      return !(room(de.a) === keep && room(de.b) === keep);
    });
    done = true;
  });
  return done;
}

/** A building's storeys above ground in a design: its levels from the ground floor up, the
 * open roof and tower tops aside. */
export function storeysOf(d: SiteDesign): number {
  return d.levels.filter((l) => (l.z ?? 0) >= 0 && !l.roof).length;
}

/** The stair block put at x, y (w × h squares); furniture on its squares, on every level it
 * reaches, goes. */
export function setStairs(d: SiteDesign, x: number, y: number, w: number, h: number) {
  d.stairs = [x, y, w, h];
  const on = (f: Item) => f.x < x + w && f.x + f.w > x && f.y < y + h && f.y + f.h > y;
  for (const lv of d.levels) if (lv.has_stairs) lv.items = lv.items.filter((f) => !on(f));
}
