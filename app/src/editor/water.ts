// Build › Water: water drawn by hand (`Edits.waters`: `worldgen::waters`). Click the outline's
// points, then click its first point again (or double-click) to close it: an inlet or basin of
// the sea, a lake, pond or moat. Outlines that overlap are one water. A click inside water drawn
// earlier picks it (its kind, name, or take it away) when no outline is being drawn. Escape drops
// the points. Right-drag pans meanwhile.
import type { Graphics } from 'pixi.js';
import type { Edits, Water, WaterKind } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';

export type Pt = [number, number];

export const MAX_WATER_PTS = 128;
export const MAX_WATER_SPAN_FT = 26_400;
export const MAX_WATERS = 256;

export interface WaterSettings {
  kind: WaterKind;
}

export const defaultWater = (): WaterSettings => ({ kind: 'pond' });

/** Whether its level is the sea's by default (`WaterKind::of_sea`). */
export const ofSea = (k: WaterKind) => k === 'inlet' || k === 'basin';

/** Why an outline can't be water (mirrors `Water::problem`), if so. */
export function waterProblem(w: Water): string | null {
  const n = w.pts.length;
  if (n < 3) return 'An outline needs 3 points or more';
  if (n > MAX_WATER_PTS) return `An outline has at most ${MAX_WATER_PTS} points`;
  const xs = w.pts.map((p) => p[0]);
  const ys = w.pts.map((p) => p[1]);
  if (Math.max(...xs) - Math.min(...xs) > MAX_WATER_SPAN_FT || Math.max(...ys) - Math.min(...ys) > MAX_WATER_SPAN_FT) return 'An outline spans 5 miles at most';
  if (Math.abs(area(w.pts)) < 25) return 'The outline holds no water';
  if (crosses(w.pts)) return 'The outline crosses itself';
  return null;
}

function area(p: Pt[]): number {
  let a = 0;
  for (let i = 0; i < p.length; i++) {
    const [x0, y0] = p[i];
    const [x1, y1] = p[(i + 1) % p.length];
    a += x0 * y1 - x1 * y0;
  }
  return a / 2;
}

function crosses(p: Pt[]): boolean {
  const n = p.length;
  const side = (a: Pt, b: Pt, c: Pt) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  for (let i = 0; i < n; i++) {
    const [a, b] = [p[i], p[(i + 1) % n]];
    for (let j = i + 1; j < n; j++) {
      if (j === i + 1 || (i === 0 && j === n - 1)) continue;
      const [c, d] = [p[j], p[(j + 1) % n]];
      if (side(a, b, c) * side(a, b, d) < 0 && side(c, d, a) * side(c, d, b) < 0) return true;
    }
  }
  return false;
}

export function contains(pts: Pt[], x: number, y: number): boolean {
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i];
    const [xj, yj] = pts[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/** What the tool needs from the app. */
export interface WaterHost {
  edits(): Edits;
  picked(): string | null;
  /** An outline closed (world ft, rounded to 0.5). */
  placed(pts: Pt[]): void;
  /** Water drawn earlier was clicked. */
  pick(id: string): void;
  hint(text: string): void;
}

export class WaterTool implements PointerTool {
  private pts: Pt[] = [];
  private cursor: Pt | null = null;
  /** ft per px, as last drawn. */
  private fpp = 1;

  constructor(private readonly host: WaterHost) {}

  down(_x: number, _y: number, e: PointerEvent): boolean {
    return e.button === 0;
  }

  move(x: number, y: number) {
    this.cursor = [x, y];
  }

  up(x: number, y: number) {
    const p: Pt = [Math.round(x * 2) / 2, Math.round(y * 2) / 2];
    if (!this.pts.length) {
      const hit = this.at(p);
      if (hit && hit !== this.host.picked()) return this.host.pick(hit);
      this.pts = [p];
      return this.host.hint('Click its next points; click the first again to close it');
    }
    const first = this.pts[0];
    if (this.pts.length >= 3 && Math.hypot(p[0] - first[0], p[1] - first[1]) <= 8 * this.fpp) return this.close();
    const last = this.pts[this.pts.length - 1];
    if (Math.hypot(p[0] - last[0], p[1] - last[1]) < 1) return;
    if (this.pts.length >= MAX_WATER_PTS) return this.host.hint(`An outline has at most ${MAX_WATER_PTS} points: click the first to close it`);
    this.pts.push(p);
  }

  dblclick(): boolean {
    if (this.pts.length < 3) return this.pts.length > 0;
    this.close();
    return true;
  }

  private close() {
    const pts = this.pts;
    this.pts = [];
    this.host.placed(pts);
  }

  /** The water drawn earlier under p (the smallest holding it), if any. */
  private at(p: Pt): string | null {
    let best: [string, number] | null = null;
    for (const [id, w] of Object.entries(this.host.edits().waters ?? {})) {
      if (!contains(w.pts, p[0], p[1])) continue;
      const a = Math.abs(area(w.pts));
      if (!best || a < best[1]) best = [id, a];
    }
    return best?.[0] ?? null;
  }

  /** Drop the points clicked. True if there were some. */
  reset(): boolean {
    const had = this.pts.length > 0;
    this.pts = [];
    return had;
  }

  cancel() {}

  hover(x: number, y: number) {
    this.cursor = [x, y];
  }

  draw(g: Graphics, cam: Camera) {
    this.fpp = 1 / cam.ppf;
    const ink = 0x2563eb;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const picked = this.host.picked();
    for (const [id, w] of Object.entries(this.host.edits().waters ?? {})) {
      const on = id === picked;
      g.poly(w.pts.flatMap((p) => scr(p)), true)
        .fill({ color: ink, alpha: on ? 0.2 : 0.04 })
        .stroke({ width: on ? 2 : 1, color: ink, alpha: on ? 1 : 0.5 });
    }
    if (this.cursor && !this.pts.length) {
      const [sx, sy] = scr(this.cursor);
      g.circle(sx, sy, 3).fill({ color: ink });
    }
    if (!this.pts.length) return;
    const line = this.cursor ? [...this.pts, this.cursor] : this.pts;
    const s0 = scr(line[0]);
    g.moveTo(s0[0], s0[1]);
    for (const p of line.slice(1)) {
      const s = scr(p);
      g.lineTo(s[0], s[1]);
    }
    g.stroke({ width: 2, color: ink });
    for (const [k, p] of this.pts.entries()) {
      const [sx, sy] = scr(p);
      const r = k === 0 ? 5 : 3;
      g.rect(sx - r, sy - r, 2 * r, 2 * r).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
    }
  }
}
