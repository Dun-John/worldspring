// Play mode state (local table): tokens, shapes, fog of war, doors, per-location settings and
// the grid's scale. Every change is an `Op`, applied the same way by the DM window and the
// player window (event sourcing over a BroadcastChannel), and saved per world. Nothing here
// belongs to a game system: tokens have a size in squares, the ruler measures in the grid's
// unit, shapes are plain outlines.
//
// Positions are in the squares of a location's grid: the world's squares on the surface
// (x ft / 5), or a building's or site's own grid inside it (`Interior` grid units).

/** Token sizes (squares a side). */
export const SIZES = [0.5, 1, 2, 3, 4];

/** A character or creature, or a light source (a torch on a wall, a brazier). */
export type TokenKind = 'character' | 'light';

export interface Token {
  id: string;
  /** The location it stands in (`surface` or `<interior id>@<level>`). */
  loc: string;
  /** Centre, in squares of the location's grid. */
  x: number;
  y: number;
  name: string;
  kind: TokenKind;
  /** Squares a side. */
  size: number;
  color: number;
  /** A picture (id in the session store). */
  image?: string;
  /** Only the DM sees it. */
  hidden?: boolean;
  /** The players see what it sees (line of sight). */
  vision?: boolean;
  /** Light it gives: radius in squares (the panel shows it in the grid's unit). */
  light?: number;
}

export type ShapeKind = 'circle' | 'rect' | 'line' | 'cone';

/** A shape drawn on the map: from an origin (a grid corner) along a vector (squares): a
 * circle's radius, a rectangle's opposite corner, a line's or cone's length and direction. */
export interface Shape {
  id: string;
  loc: string;
  kind: ShapeKind;
  x: number;
  y: number;
  dx: number;
  dy: number;
  color: number;
}

export interface DoorState {
  /** Open (a portcullis: raised). */
  open?: boolean;
  /** A secret door the players know about. */
  found?: boolean;
  /** A locked door the DM has unlocked (it stays shut until opened; afterwards it is a plain door). */
  unlocked?: boolean;
}

export interface LocSettings {
  /** Fog of war: players see only what has been revealed. */
  fog: boolean;
  /** Darkness: characters see only what light reaches. */
  dark: boolean;
}

/** How the ruler counts a diagonal: as one square, as one then two alternately, as the
 * straight-line distance, or as two (one across and one along). */
export type Measure = 'chebyshev' | 'alternating' | 'euclidean' | 'manhattan';

export interface GridSettings {
  /** One square is this many `unit`s. */
  scale: number;
  unit: string;
  measure: Measure;
}

/** Fog chunks are 128 x 128 squares (a battlemap chunk on the surface). */
export const FOG_N = 128;

export interface PlayState {
  tokens: Record<string, Token>;
  shapes: Record<string, Shape>;
  /**
   * Per location, the squares the players know (by chunk, `cx,cy` → 0 unknown / 255 known):
   * revealed by the DM's fog brush, and everything their characters have seen.
   */
  fog: Record<string, Record<string, Uint8Array>>;
  /** By `<location>#<door index>`. */
  doors: Record<string, DoorState>;
  /** Locations' settings, where changed from the defaults (`locDefaults`). */
  locs: Record<string, LocSettings>;
  /** Line of sight: players see what their characters can see. */
  los: boolean;
  grid: GridSettings;
  /** When the DM last changed it (ms since 1970; missing: not since this was kept). */
  changed?: number;
  /** Carried over from another world's session when the world was remade: that world's hash,
   * and `changed` as it was then (the same now: nothing played here since). */
  came?: { from: string; changed: number };
}
// (Places are named by layout-local ids in tokens', shapes' and doors' `loc`, and the keys of
// `fog`, `doors` and `locs`: a remade world moves them (`carrySession`). A new field naming a
// place must be moved there too.)

export const DEFAULT_GRID: GridSettings = { scale: 5, unit: 'ft', measure: 'chebyshev' };

export function emptyState(): PlayState {
  return { tokens: {}, shapes: {}, fog: {}, doors: {}, locs: {}, los: false, grid: { ...DEFAULT_GRID } };
}

/** Whether a session holds anything: tokens, shapes, fog, doors, settings, line of sight, or a
 * grid changed. */
export function hasSession(s: PlayState): boolean {
  const g = s.grid;
  const grid = g.scale !== DEFAULT_GRID.scale || g.unit !== DEFAULT_GRID.unit || g.measure !== DEFAULT_GRID.measure;
  return grid || s.los || [s.tokens, s.shapes, s.fog, s.doors, s.locs].some((r) => Object.keys(r).length > 0);
}

/** A location's key: the surface, or a level of a building or site. */
export const locKey = (interior: string | null, level: number) => (interior ? `${interior}@${level}` : 'surface');

/** The interior id and level of a location key (null id: the surface). */
export function parseLoc(loc: string): { interior: string | null; level: number } {
  const at = loc.lastIndexOf('@');
  return at < 0 ? { interior: null, level: 0 } : { interior: loc.slice(0, at), level: Number(loc.slice(at + 1)) };
}

/** Underground sites (behind entrances, sewers, keeps' dungeons), not buildings. */
export const isUnderground = (interior: string) => /^[uwk]:/.test(interior);

/** Underground sites start fogged and dark; the surface and buildings in plain view. */
export function locDefaults(loc: string): LocSettings {
  const under = isUnderground(loc);
  return { fog: under, dark: under };
}

export const settingsOf = (s: PlayState, loc: string): LocSettings => s.locs[loc] ?? locDefaults(loc);

export type Op =
  | { t: 'token'; token: Token }
  | { t: 'untoken'; id: string }
  | { t: 'shape'; shape: Shape }
  /** `id` '*': every shape in `loc`. */
  | { t: 'unshape'; id: string; loc?: string }
  /** A fog brush stroke: a disc of radius `r` round each point (x, y, x, y, ...). */
  | { t: 'fog'; loc: string; reveal: boolean; pts: number[]; r: number }
  /** Squares set in `bits` (w x h from x0, y0) revealed or hidden (rooms, what was seen). */
  | { t: 'fogbits'; loc: string; reveal: boolean; x0: number; y0: number; w: number; h: number; bits: Uint8Array }
  /** Forget everything known here. */
  | { t: 'fogreset'; loc: string }
  | { t: 'door'; key: string; state: DoorState }
  | { t: 'loc'; loc: string; settings: LocSettings }
  | { t: 'settings'; los: boolean }
  | { t: 'grid'; grid: GridSettings }
  | { t: 'reset' };

/**
 * Apply an op in place. Returns the fog chunks it changed (`loc|cx,cy`), so the renderer
 * uploads only those.
 */
export function apply(s: PlayState, op: Op): string[] {
  switch (op.t) {
    case 'token':
      s.tokens[op.token.id] = op.token;
      return [];
    case 'untoken':
      delete s.tokens[op.id];
      return [];
    case 'shape':
      s.shapes[op.shape.id] = op.shape;
      return [];
    case 'unshape':
      if (op.id === '*') for (const [id, t] of Object.entries(s.shapes)) if (t.loc === op.loc) delete s.shapes[id];
      delete s.shapes[op.id];
      return [];
    case 'fog':
      return paintDiscs(s, op.loc, op.pts, op.r, op.reveal ? 255 : 0);
    case 'fogbits':
      return paintBits(s, op.loc, op.x0, op.y0, op.w, op.h, op.bits, op.reveal ? 255 : 0);
    case 'fogreset': {
      const keys = Object.keys(s.fog[op.loc] ?? {}).map((k) => `${op.loc}|${k}`);
      delete s.fog[op.loc];
      return keys;
    }
    case 'door':
      s.doors[op.key] = op.state;
      return [];
    case 'loc':
      s.locs[op.loc] = op.settings;
      return [];
    case 'settings':
      s.los = op.los;
      return [];
    case 'grid':
      s.grid = op.grid;
      return [];
    case 'reset': {
      const keys = Object.entries(s.fog).flatMap(([loc, chunks]) => Object.keys(chunks).map((k) => `${loc}|${k}`));
      Object.assign(s, emptyState());
      return keys;
    }
  }
}

/**
 * A saved (or sent) state as this version has it. Sessions from before carried game-system
 * fields (5e sizes and conditions, spell areas, a diagonal rule): sizes become squares, player
 * characters share their vision, spell areas become shapes, the rest is dropped.
 */
export function normalize(raw: unknown): PlayState {
  const r = (raw ?? {}) as Record<string, unknown> & Partial<PlayState>;
  const s: PlayState = { ...emptyState(), fog: r.fog ?? {}, doors: r.doors ?? {}, locs: r.locs ?? {}, los: !!r.los };
  const old: Record<string, number> = { tiny: 0.5, small: 1, medium: 1, large: 2, huge: 3, gargantuan: 4 };
  for (const t of Object.values((r.tokens ?? {}) as unknown as Record<string, Record<string, unknown>>)) {
    const kind = t.kind === 'light' ? 'light' : 'character';
    const size = typeof t.size === 'number' ? t.size : (old[String(t.size)] ?? 1);
    const token: Token = { id: String(t.id), loc: String(t.loc), x: Number(t.x), y: Number(t.y), name: String(t.name ?? ''), kind, size, color: Number(t.color ?? 0x8b2e2e) };
    if (t.image) token.image = String(t.image);
    if (t.hidden) token.hidden = true;
    if (t.vision || t.kind === 'pc') token.vision = true;
    // Older sessions gave light in feet.
    if (Number(t.light) > 0) token.light = r.grid ? Number(t.light) : Number(t.light) / 5;
    s.tokens[token.id] = token;
  }
  const kinds: Record<string, ShapeKind> = { sphere: 'circle', cube: 'rect', line: 'line', cone: 'cone', circle: 'circle', rect: 'rect' };
  const shapes = { ...((r as { templates?: Record<string, Record<string, unknown>> }).templates ?? {}), ...((r.shapes ?? {}) as unknown as Record<string, Record<string, unknown>>) };
  for (const v of Object.values(shapes)) {
    const kind = kinds[String(v.kind ?? v.shape)];
    if (!kind) continue;
    let [dx, dy] = [Number(v.dx), Number(v.dy)];
    // A spell cube's vector was its side along the diagonal: the corner opposite.
    if (v.shape === 'cube') [dx, dy] = [Math.sign(dx || 1) * Math.max(Math.abs(dx), Math.abs(dy)), Math.sign(dy || 1) * Math.max(Math.abs(dx), Math.abs(dy))];
    s.shapes[String(v.id)] = { id: String(v.id), loc: String(v.loc), kind, x: Number(v.x), y: Number(v.y), dx, dy, color: Number(v.color) };
  }
  const g = r.grid;
  if (g && g.scale > 0) s.grid = { scale: g.scale, unit: String(g.unit ?? ''), measure: g.measure ?? 'chebyshev' };
  else if ((r as { diagonal?: string }).diagonal === '5-10') s.grid.measure = 'alternating';
  if (typeof r.changed === 'number') s.changed = r.changed;
  if (r.came && typeof r.came.from === 'string') s.came = { from: r.came.from, changed: Number(r.came.changed) || 0 };
  return s;
}

/** The fog chunk holding a square, made (all hidden) if `make`. */
function chunk(s: PlayState, loc: string, cx: number, cy: number, make: boolean): Uint8Array | null {
  const key = `${cx},${cy}`;
  const fog = (s.fog[loc] ??= {});
  return fog[key] ?? (make ? (fog[key] = new Uint8Array(FOG_N * FOG_N)) : null);
}

/** Whether the players know a square. */
export function revealed(s: PlayState, loc: string, i: number, j: number): boolean {
  const cx = Math.floor(i / FOG_N);
  const cy = Math.floor(j / FOG_N);
  const c = s.fog[loc]?.[`${cx},${cy}`];
  return !!c && c[(j - cy * FOG_N) * FOG_N + (i - cx * FOG_N)] > 0;
}

function set(s: PlayState, loc: string, i: number, j: number, v: number, touched: Set<string>) {
  const cx = Math.floor(i / FOG_N);
  const cy = Math.floor(j / FOG_N);
  const c = chunk(s, loc, cx, cy, v > 0);
  if (!c) return;
  const k = (j - cy * FOG_N) * FOG_N + (i - cx * FOG_N);
  if (c[k] === v) return;
  c[k] = v;
  touched.add(`${loc}|${cx},${cy}`);
}

function paintDiscs(s: PlayState, loc: string, pts: number[], r: number, v: number): string[] {
  const touched = new Set<string>();
  for (let p = 0; p + 1 < pts.length; p += 2) {
    const [x, y] = [pts[p], pts[p + 1]];
    for (let j = Math.floor(y - r); j <= Math.ceil(y + r); j++) {
      for (let i = Math.floor(x - r); i <= Math.ceil(x + r); i++) {
        if ((i + 0.5 - x) ** 2 + (j + 0.5 - y) ** 2 <= r * r) set(s, loc, i, j, v, touched);
      }
    }
  }
  return [...touched];
}

function paintBits(s: PlayState, loc: string, x0: number, y0: number, w: number, h: number, bits: Uint8Array, v: number): string[] {
  const touched = new Set<string>();
  for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) if (bits[j * w + i]) set(s, loc, x0 + i, y0 + j, v, touched);
  return [...touched];
}

/** Whether `bits` would reveal any square still unknown. */
export function revealsNew(s: PlayState, loc: string, x0: number, y0: number, w: number, h: number, bits: Uint8Array): boolean {
  for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) if (bits[j * w + i] && !revealed(s, loc, x0 + i, y0 + j)) return true;
  return false;
}

/** A short random id. */
export const newId = () => Math.random().toString(36).slice(2, 10);
