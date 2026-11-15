// Quick placement tools of the Build tab: Stamp (a building of a kind and size follows the pointer
// as a ghost and goes down on a click; on a touch screen a tap moves the ghost and the panel's
// button places it), and Path (Row: a line of corners or a drag, Lasso: a closed outline drawn
// freehand or corner by corner). The ghosts are set by the app from the generator's answers; the
// tools only draw them and report where the pointer is.
import type { Graphics } from 'pixi.js';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import type { Pt } from './build';

export type StampSize = 'small' | 'usual' | 'large';
export interface StampSettings {
  size: StampSize;
  clear: boolean;
  /** Which way its front faces, in degrees (0 = east, 90 = south, turned in 90 steps); null: toward the nearest street. */
  facing: number | null;
}
export const defaultStamp = (): StampSettings => ({ size: 'usual', clear: false, facing: null });

export interface RowSettings {
  side: 'left' | 'right' | 'both';
  ward: string;
  kinds: string[];
  /** Lot depth (ft), or 0: by the ward's lot size. */
  depth: number;
  gaps: number;
  clear: boolean;
  seed: number;
}
export const defaultRow = (): RowSettings => ({ side: 'both', ward: 'common', kinds: [], depth: 0, gaps: 0.15, clear: false, seed: 0 });

export interface LassoSettings {
  fill: 'generated' | 'hand' | 'open';
  ward: string;
  /** '' the ward's usual. */
  lots: string;
  reroll: number;
  open: string;
  name: string;
}
export const defaultLasso = (): LassoSettings => ({ fill: 'generated', ward: 'common', lots: '', reroll: 0, open: 'plaza', name: '' });

const INK = 0xf97316;
const BAD = 0xdc2626;

export interface StampHost {
  /** The pointer is at (x, y): show the ghost there (`place`: a click's, not a tap's). */
  at(x: number, y: number, place: boolean): void;
  hint(text: string): void;
}

export class StampTool implements PointerTool {
  /** The ghost: the footprint, the way its front looks (degrees), whether it can stand. */
  ghost: { poly: Pt[]; facing: number | null; ok: boolean } | null = null;
  /** The generated buildings it would take away (their footprints are not known here: the middles). */
  gone: Pt[] = [];
  readonly hoverFollows = true;
  private touch = false;

  constructor(private readonly host: StampHost) {}

  down(_x: number, _y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    this.touch = e.pointerType === 'touch';
    return true;
  }
  move() {}
  up(x: number, y: number) {
    this.host.at(x, y, !this.touch);
  }
  cancel() {}
  hover(x: number, y: number) {
    this.host.at(x, y, false);
  }

  draw(g: Graphics, cam: Camera) {
    const gh = this.ghost;
    if (!gh) return;
    const ink = gh.ok ? INK : BAD;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    for (const m of this.gone) {
      const [sx, sy] = scr(m);
      g.circle(sx, sy, 7).fill({ color: BAD, alpha: 0.25 }).stroke({ width: 1.5, color: BAD });
    }
    const flat = gh.poly.flatMap((p) => scr(p));
    g.poly(flat).fill({ color: ink, alpha: 0.25 });
    g.poly(flat).stroke({ width: 2, color: ink });
    if (gh.facing != null) {
      const n = gh.poly.length;
      const mx = gh.poly.reduce((s, p) => s + p[0], 0) / n;
      const my = gh.poly.reduce((s, p) => s + p[1], 0) / n;
      const a = (gh.facing * Math.PI) / 180;
      const [ax, ay] = scr([mx, my]);
      const [bx, by] = scr([mx + Math.cos(a) * 12, my + Math.sin(a) * 12]);
      const [dx, dy] = [bx - ax, by - ay];
      const l = Math.hypot(dx, dy) || 1;
      const [ux, uy] = [dx / l, dy / l];
      g.moveTo(ax, ay).lineTo(bx, by);
      g.moveTo(bx, by).lineTo(bx - ux * 7 - uy * 4, by - uy * 7 + ux * 4);
      g.moveTo(bx, by).lineTo(bx - ux * 7 + uy * 4, by - uy * 7 - ux * 4);
      g.stroke({ width: 2, color: ink });
    }
  }
}

export interface PathHost {
  mode(): 'row' | 'lasso';
  /** The path changed (`done`: it is finished, so the app lays the row or checks the outline). */
  changed(path: Pt[], done: boolean): void;
  hint(text: string): void;
}

/** Corners further apart than this are a drag's, not a click's (ft). */
const DRAG_FT = 15;
/** A freehand path's spacing between samples (ft). */
const STEP_FT = 10;
const MAX_CORNERS = 40;

/** Douglas-Peucker: the path with every corner dropped that lies within `tol` ft of the line between its neighbours. */
export function simplify(p: Pt[], tol: number, closed = false): Pt[] {
  if (p.length < 3) return p.slice();
  const keep = new Array(p.length).fill(false);
  keep[0] = keep[p.length - 1] = true;
  const stack: [number, number][] = [[0, p.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop()!;
    let worst = -1;
    let at = -1;
    const [ax, ay, bx, by] = [p[a][0], p[a][1], p[b][0], p[b][1]];
    const l = Math.hypot(bx - ax, by - ay) || 1e-9;
    for (let i = a + 1; i < b; i++) {
      const d = Math.abs((bx - ax) * (ay - p[i][1]) - (ax - p[i][0]) * (by - ay)) / l;
      if (d > worst) [worst, at] = [d, i];
    }
    if (worst > tol && at > 0) {
      keep[at] = true;
      stack.push([a, at], [at, b]);
    }
  }
  const out = p.filter((_, i) => keep[i]);
  return closed && out.length < 3 ? p.slice(0, 3) : out;
}

export class PathTool implements PointerTool {
  /** The corners so far. */
  path: Pt[] = [];
  /** Finished: the app has the path, and the next press starts a new one. */
  finished = false;
  /** What the app shows over it: lots of a row (footprints), slots refused, the plot's outline met, or the outline is refused. */
  lots: Pt[][] = [];
  refused: Pt[] = [];
  over: Pt[] | null = null;
  bad = false;
  private cursor: Pt | null = null;
  private drag: Pt[] | null = null;
  private from: Pt | null = null;

  constructor(private readonly host: PathHost) {}

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    if (this.finished) this.reset();
    this.from = [x, y];
    this.drag = null;
    return true;
  }

  move(x: number, y: number) {
    this.cursor = [x, y];
    if (!this.from) return;
    if (!this.drag) {
      if (Math.hypot(x - this.from[0], y - this.from[1]) < DRAG_FT) return;
      this.drag = [this.from];
    }
    const last = this.drag[this.drag.length - 1];
    if (Math.hypot(x - last[0], y - last[1]) >= STEP_FT) this.drag.push([x, y]);
  }

  up(x: number, y: number) {
    const from = this.from;
    this.from = null;
    if (!from) return;
    const row = this.host.mode() === 'row';
    if (this.drag) {
      this.drag.push([x, y]);
      const free = this.host.mode() === 'row' ? simplify(this.drag, 6) : this.limit(this.drag);
      this.drag = null;
      this.path = row ? [...this.path, ...free] : free;
      return this.finish();
    }
    const c = this.path;
    if (!row && c.length >= 3 && Math.hypot(x - c[0][0], y - c[0][1]) < DRAG_FT * 1.5) return this.finish();
    if (c.length && Math.hypot(x - c[c.length - 1][0], y - c[c.length - 1][1]) < 3) return;
    if (c.length >= 64) return this.host.hint('At most 64 corners: finish it');
    c.push([x, y]);
    if (c.length === 1) this.host.hint(row ? 'Click each corner, or drag along the street; press Enter to finish' : 'Click each corner (the first again to finish) or drag round the area');
    if (row && c.length >= 2) this.host.changed(c, false);
  }

  /** A freehand outline thinned to few enough corners. */
  private limit(p: Pt[]): Pt[] {
    let tol = 4;
    let out = simplify(p, tol, true);
    while (out.length > MAX_CORNERS && tol < 200) out = simplify(p, (tol *= 1.5), true);
    return out;
  }

  finish() {
    const row = this.host.mode() === 'row';
    if (this.path.length < (row ? 2 : 3)) return this.host.hint(row ? 'A row needs a line: two corners at least' : 'An outline needs at least 3 corners');
    this.finished = true;
    this.host.changed(this.path, true);
  }

  back() {
    if (this.finished) this.finished = false;
    this.path.pop();
    this.host.changed(this.path, false);
  }

  /** Drop what is being drawn. True if there was something. */
  reset(): boolean {
    const had = !!this.path.length || !!this.drag;
    this.path = [];
    this.drag = null;
    this.from = null;
    this.finished = false;
    this.lots = [];
    this.refused = [];
    this.over = null;
    this.bad = false;
    return had;
  }

  dblclick(): boolean {
    if (!this.finished && this.path.length >= (this.host.mode() === 'row' ? 2 : 3)) this.finish();
    return true;
  }

  cancel() {
    this.from = null;
    this.drag = null;
  }

  hover(x: number, y: number) {
    this.cursor = [x, y];
  }

  draw(g: Graphics, cam: Camera) {
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const row = this.host.mode() === 'row';
    for (const l of this.lots) {
      const flat = l.flatMap((p) => scr(p));
      g.poly(flat).fill({ color: INK, alpha: 0.28 });
      g.poly(flat).stroke({ width: 1.5, color: INK, alpha: 0.8 });
    }
    for (const r of this.refused) {
      const [sx, sy] = scr(r);
      g.circle(sx, sy, 6).fill({ color: BAD, alpha: 0.3 }).stroke({ width: 1.5, color: BAD });
    }
    if (this.over) {
      const flat = this.over.flatMap((p) => scr(p));
      g.poly(flat).fill({ color: BAD, alpha: 0.2 });
      g.poly(flat).stroke({ width: 2, color: BAD });
    }
    const pts = this.drag ? (row ? [...this.path, ...this.drag] : this.drag) : this.path;
    const live = !this.finished && !this.drag && this.cursor && pts.length ? [...pts, this.cursor] : pts;
    if (!live.length) return;
    const ink = this.bad ? BAD : INK;
    if (!row && live.length >= 3) g.poly(live.flatMap((p) => scr(p))).fill({ color: ink, alpha: 0.18 });
    for (let i = 0; i + 1 < live.length; i++) {
      const [a, b] = [scr(live[i]), scr(live[i + 1])];
      g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
    }
    if (!row && (this.finished || this.drag) && live.length >= 3) {
      const [a, b] = [scr(live[live.length - 1]), scr(live[0])];
      g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
    }
    g.stroke({ width: 2, color: ink });
    for (const c of this.path) {
      const [sx, sy] = scr(c);
      g.rect(sx - 3, sy - 3, 6, 6).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
    }
  }
}
