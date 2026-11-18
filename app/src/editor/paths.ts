// Build › Road: roads drawn by hand on the ground (`Edits.paths`: `worldgen::paths`). Click the
// line's points and press Enter or double-click to end it, or press and drag it freehand: a
// footpath, track, gravel road, paved road or cobbled street. The road runs on a smooth curve
// through the points (`curve`, as the generator shapes it), a band of its width along it. A click
// on a road drawn earlier picks it (its type, width, name, or take it away) when no line is being
// drawn. Escape drops the points. Right-drag pans meanwhile.
import type { Graphics } from 'pixi.js';
import type { DrawnPath, Edits, PathKind } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import { dragTo, endAt } from './line';
import { FREEHAND_FT } from './units';
import { segDist } from './geom';

export type Pt = [number, number];

export const MAX_PATH_PTS = 256;
export const MAX_PATH_SPAN_FT = 26_400;
export const MAX_PATHS = 256;
export const PATH_WIDTH = { min: 3, max: 40 };

export const PATH_KINDS: { key: PathKind; label: string; name: string; width: number; look: string }[] = [
  { key: 'footpath', label: 'Footpath', name: 'footpath', width: 4, look: 'A narrow worn path; fords water.' },
  { key: 'track', label: 'Track', name: 'track', width: 10, look: 'A packed-earth cart track; fords water.' },
  { key: 'gravel', label: 'Gravel', name: 'gravel road', width: 14, look: 'A gravel road; crosses water on a timber bridge.' },
  { key: 'paved', label: 'Paved', name: 'paved road', width: 18, look: 'A paved road; crosses water on a stone bridge.' },
  { key: 'cobbled', label: 'Cobbled', name: 'cobbled street', width: 20, look: 'A cobbled street; crosses water on a stone bridge.' },
];

export const kindWidth = (k: PathKind) => PATH_KINDS.find((p) => p.key === k)?.width ?? 10;
export const pathWidth = (p: DrawnPath) => Math.max(PATH_WIDTH.min, Math.min(PATH_WIDTH.max, p.width ?? kindWidth(p.type)));

export interface PathSettings {
  kind: PathKind;
  width: number;
  name: string;
}

export const defaultPath = (): PathSettings => ({ kind: 'track', width: 10, name: '' });

/** Why a line can't be a road (mirrors `Path::problem`), if so. */
export function pathProblem(p: DrawnPath): string | null {
  const n = p.pts.length;
  if (n < 2) return 'A road needs 2 points or more';
  if (n > MAX_PATH_PTS) return `A road has at most ${MAX_PATH_PTS} points`;
  if (p.width != null && !(p.width >= PATH_WIDTH.min && p.width <= PATH_WIDTH.max)) return `A road is ${PATH_WIDTH.min} to ${PATH_WIDTH.max} ft wide`;
  const xs = p.pts.map((q) => q[0]);
  const ys = p.pts.map((q) => q[1]);
  if (Math.max(...xs) - Math.min(...xs) > MAX_PATH_SPAN_FT || Math.max(...ys) - Math.min(...ys) > MAX_PATH_SPAN_FT) return 'A road spans 5 miles at most';
  for (let i = 1; i < n; i++) if (Math.hypot(p.pts[i][0] - p.pts[i - 1][0], p.pts[i][1] - p.pts[i - 1][1]) < 0.5) return 'Two points in a row are the same';
  return null;
}

const STEP_FT = 10;
const MAX_LINE_PTS = 3000;
const cache = new Map<string, Pt[]>();

/** The line through `pts` as the generator shapes it (`paths::curve`: centripetal Catmull-Rom,
 * about every 10 ft). */
export function curve(pts: Pt[]): Pt[] {
  const n = pts.length;
  if (n < 2) return pts.slice();
  const key = pts.join(';');
  const hit = cache.get(key);
  if (hit) return hit;
  let total = 0;
  for (let i = 1; i < n; i++) total += Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
  const step = Math.max(STEP_FT, total / MAX_LINE_PTS);
  const ext = (a: Pt, b: Pt): Pt => [2 * a[0] - b[0], 2 * a[1] - b[1]];
  const knot = (a: Pt, b: Pt) => Math.max(Math.sqrt(Math.hypot(a[0] - b[0], a[1] - b[1])), 1e-3);
  const lerp = (a: Pt, b: Pt, ta: number, tb: number, t: number): Pt => {
    const [u, v] = [(tb - t) / (tb - ta), (t - ta) / (tb - ta)];
    return [a[0] * u + b[0] * v, a[1] * u + b[1] * v];
  };
  const out: Pt[] = [];
  for (let i = 0; i < n - 1; i++) {
    const [p1, p2] = [pts[i], pts[i + 1]];
    const p0 = i === 0 ? ext(p1, p2) : pts[i - 1];
    const p3 = i + 2 < n ? pts[i + 2] : ext(p2, p1);
    const t1 = knot(p0, p1);
    const t2 = t1 + knot(p1, p2);
    const t3 = t2 + knot(p2, p3);
    const k = Math.max(1, Math.ceil(Math.hypot(p2[0] - p1[0], p2[1] - p1[1]) / step));
    for (let j = 0; j < k; j++) {
      const t = t1 + ((t2 - t1) * j) / k;
      const a1 = lerp(p0, p1, 0, t1, t);
      const a2 = lerp(p1, p2, t1, t2, t);
      const a3 = lerp(p2, p3, t2, t3, t);
      const b1 = lerp(a1, a2, 0, t2, t);
      const b2 = lerp(a2, a3, t1, t3, t);
      out.push(lerp(b1, b2, t1, t2, t));
    }
  }
  out.push(pts[n - 1]);
  if (cache.size > 256) cache.clear();
  cache.set(key, out);
  return out;
}

/** An open line's points thinned (Douglas-Peucker, both ends kept) to at most `max`. */
export function simplifyOpen(pts: Pt[], max: number): Pt[] {
  if (pts.length <= 2) return pts;
  for (let tol = 1.5; ; tol *= 1.5) {
    const keep = pts.map((_, i) => i === 0 || i === pts.length - 1);
    const run = (a: number, b: number) => {
      let [best, at] = [-1, -1];
      for (let i = a + 1; i < b; i++) {
        const d = segDist(pts[i], pts[a], pts[b]);
        if (d > best) [best, at] = [d, i];
      }
      if (at < 0 || best <= tol) return;
      keep[at] = true;
      run(a, at);
      run(at, b);
    };
    run(0, pts.length - 1);
    const out = pts.filter((_, i) => keep[i]);
    if (out.length <= max) return out;
  }
}

/** The road drawn by hand nearest `p` within its half width and `slack` ft, if any. */
export function pathAt(edits: Edits, p: Pt, slack: number): string | null {
  let best: [string, number] | null = null;
  for (const [id, r] of Object.entries(edits.paths ?? {})) {
    const line = curve(r.pts);
    const reach = pathWidth(r) / 2 + slack;
    for (let i = 1; i < line.length; i++) {
      const d = segDist(p, line[i - 1], line[i]);
      if (d <= reach && (!best || d < best[1])) best = [id, d];
    }
  }
  return best?.[0] ?? null;
}

/** What the tool needs from the app. */
export interface PathHost {
  edits(): Edits;
  picked(): string | null;
  /** The width being drawn (ft), for the preview. */
  width(): number;
  /** A line ended (world ft, rounded to 0.5). */
  placed(pts: Pt[]): void;
  /** A road drawn earlier was clicked. */
  pick(id: string): void;
  hint(text: string): void;
}

const round = (p: Pt): Pt => [Math.round(p[0] * 2) / 2, Math.round(p[1] * 2) / 2];

export class PathDrawTool implements PointerTool {
  private pts: Pt[] = [];
  private cursor: Pt | null = null;
  /** ft per px, as last drawn. */
  private fpp = 1;
  private press: Pt | null = null;
  private stroke: Pt[] | null = null;

  constructor(private readonly host: PathHost) {}

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    this.press = [x, y];
    this.stroke = null;
    return true;
  }

  move(x: number, y: number) {
    this.cursor = [x, y];
    const press = this.press;
    if (!press || this.pts.length) return;
    this.stroke = dragTo(this.stroke, press, [x, y], 6 * this.fpp, Math.max(FREEHAND_FT, 3 * this.fpp));
  }

  up(x: number, y: number) {
    this.press = null;
    if (this.stroke) return this.endStroke(x, y);
    const p = round([x, y]);
    if (!this.pts.length) {
      const hit = pathAt(this.host.edits(), p, 4 * this.fpp);
      if (hit && hit !== this.host.picked()) return this.host.pick(hit);
      this.pts = [p];
      return this.host.hint('Click its next points; press Enter or double-click to end it');
    }
    const last = this.pts[this.pts.length - 1];
    if (Math.hypot(p[0] - last[0], p[1] - last[1]) < 1) return;
    if (this.pts.length >= MAX_PATH_PTS) return this.host.hint(`A road has at most ${MAX_PATH_PTS} points: press Enter to end it`);
    this.pts.push(p);
  }

  /** A freehand line let go: the road, simplified to its points. */
  private endStroke(x: number, y: number) {
    const s = this.stroke!;
    this.stroke = null;
    endAt(s, [x, y]);
    const pts: Pt[] = [];
    for (const p of simplifyOpen(s, MAX_PATH_PTS).map(round)) {
      const q = pts[pts.length - 1];
      if (!q || Math.hypot(p[0] - q[0], p[1] - q[1]) >= 1) pts.push(p);
    }
    if (pts.length < 2) return;
    this.pts = pts;
    this.finish();
  }

  /** End the line (Enter). True if there was one to end. */
  finish(): boolean {
    if (this.pts.length < 2) return false;
    const pts = this.pts;
    this.pts = [];
    this.host.placed(pts);
    return true;
  }

  /** Whether a line is under way. */
  drawing(): boolean {
    return this.pts.length > 0;
  }

  /** Take the last point back. */
  back(): boolean {
    if (!this.pts.length) return false;
    this.pts.pop();
    return true;
  }

  dblclick(): boolean {
    if (this.pts.length < 2) return this.pts.length > 0;
    this.finish();
    return true;
  }

  /** Drop the points clicked. True if there were some. */
  reset(): boolean {
    const had = this.pts.length > 0 || !!this.stroke;
    this.pts = [];
    this.stroke = null;
    this.press = null;
    return had;
  }

  cancel() {
    this.stroke = null;
    this.press = null;
  }

  hover(x: number, y: number) {
    this.cursor = [x, y];
  }

  draw(g: Graphics, cam: Camera) {
    this.fpp = 1 / cam.ppf;
    const ink = 0x2563eb;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const picked = this.host.picked();
    const line = (pts: Pt[], width: number, alpha: number) => {
      if (pts.length < 2) return;
      const s0 = scr(pts[0]);
      g.moveTo(s0[0], s0[1]);
      for (const p of pts.slice(1)) {
        const s = scr(p);
        g.lineTo(s[0], s[1]);
      }
      g.stroke({ width, color: ink, alpha, cap: 'round', join: 'round' });
    };
    for (const [id, r] of Object.entries(this.host.edits().paths ?? {})) {
      const on = id === picked;
      const c = curve(r.pts);
      if (on) line(c, Math.max(3, pathWidth(r) * cam.ppf + 4), 0.25);
      line(c, on ? 2 : 1, on ? 1 : 0.5);
    }
    if (this.stroke) {
      line(this.cursor ? [...this.stroke, this.cursor] : this.stroke, 2, 1);
      return;
    }
    if (this.cursor && !this.pts.length) {
      const [sx, sy] = scr(this.cursor);
      g.circle(sx, sy, 3).fill({ color: ink });
    }
    if (!this.pts.length) return;
    const pts = this.cursor ? [...this.pts, this.cursor] : this.pts;
    line(pts, 1, 0.45);
    const c = curve(pts);
    line(c, Math.max(2, this.host.width() * cam.ppf), 0.25);
    line(c, 2, 1);
    for (const p of this.pts) {
      const [sx, sy] = scr(p);
      g.rect(sx - 3, sy - 3, 6, 6).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
    }
  }
}
