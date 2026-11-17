// Build › Fence: fences, hedges and walls drawn by hand (`Edits.fences`: `worldgen-model::fences`),
// gates put in one, and gaps cut with the eraser (`Edits.fence_gaps`). The generator does the
// changes (`GenClient.fenceChange`); this is the kinds' words and the pointer tool: Draw (click
// corners, or press and drag a line freehand; Enter ends it, a click on its first point closes
// it), Gate (a click on a fence) and Erase (drag over the stretch to cut). Escape drops the
// corners. Right-drag pans meanwhile.
import type { Graphics } from 'pixi.js';
import type { FenceKind, Rect } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import { dragTo, endAt } from './line';
import { simplifyOpen } from './paths';

export type Pt = [number, number];

/** What the Build menu can draw (garden walls are generated only), with the rule in a line: how it plays. */
export const FENCE_KINDS: { key: FenceKind; label: string; rule: string }[] = [
  { key: 'hedgerow', label: 'Hedgerow', rule: 'Thick and wild: hard going, half cover, seen over.' },
  { key: 'hedge', label: 'Clipped hedge', rule: 'Trim and tall: blocks the way, three-quarters cover, seen over.' },
  { key: 'dry_stone_wall', label: 'Dry-stone wall', rule: 'Waist high: hard going, half cover, seen over.' },
  { key: 'post_and_rail', label: 'Post-and-rail', rule: 'Rails only: hard going, no cover, seen through.' },
  { key: 'wattle', label: 'Wattle', rule: 'Woven twigs: hard going, half cover, seen over.' },
  { key: 'low_wall', label: 'Low stone wall', rule: 'Low: hard going, half cover, seen over.' },
  { key: 'picket', label: 'Picket', rule: 'Low: hard going, half cover, seen over.' },
  { key: 'iron_railing', label: 'Iron railing', rule: 'Bars: hard going, half cover, seen through.' },
  { key: 'stockade', label: 'Stockade', rule: 'Tall logs: blocks the way and sight, full cover.' },
  { key: 'high_wall', label: 'High wall', rule: 'Tall stone: blocks the way and sight, full cover.' },
];

/** A kind in plain words ("dry-stone wall"), also for those drawn only by the generator. */
export const fenceWord = (k: FenceKind | string | undefined): string =>
  (({ garden_wall: 'garden wall', hedgerow: 'hedgerow', hedge: 'clipped hedge', dry_stone_wall: 'dry-stone wall', post_and_rail: 'post-and-rail fence', wattle: 'wattle fence', low_wall: 'low stone wall', picket: 'picket fence', iron_railing: 'iron railing', stockade: 'stockade', high_wall: 'high wall' }) as Record<string, string>)[k ?? ''] ?? 'fence';

/** The eraser's widths (ft): the gap's width, its radius half of it. */
export const ERASER_WIDTHS = [5, 10, 20];

export type FenceMode = 'draw' | 'gate' | 'erase';

export interface FenceSettings {
  kind: FenceKind;
  mode: FenceMode;
  /** The eraser's width (ft). */
  eraser: number;
}

export const defaultFence = (): FenceSettings => ({ kind: 'hedgerow', mode: 'draw', eraser: 10 });

/** The id of the generated run at `key` in the edits of town `li`: `fr:` (a field, croft, pen... side; eight hex digits) or `fh:` (a park's). */
export const fenceId = (li: number, key: string): string => `${/^[0-9a-f]{8}$/.test(key) ? 'fr' : 'fh'}:${li}:${key}`;

/** A box round a line (world ft), `pad` ft and a little more. */
export function fenceBox(pts: readonly Pt[] | readonly number[][], pad = 0): Rect {
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  const e = pad + 10;
  return [Math.min(...xs) - e, Math.min(...ys) - e, Math.max(...xs) + e, Math.max(...ys) + e];
}

/** What the tool needs from the controller. */
export interface FenceHost {
  settings(): FenceSettings;
  /** A line ended (world ft, 0.5 ft): drawn, closed or not. */
  placed(pts: Pt[], closed: boolean): void;
  /** A click in Gate mode. */
  gate(at: Pt): void;
  /** A stroke let go in Erase mode. */
  gap(pts: Pt[]): void;
  hint(text: string): void;
}

const MAX_POINTS = 400;
const FREEHAND_FT = 15;
const round = (p: Pt): Pt => [Math.round(p[0] * 2) / 2, Math.round(p[1] * 2) / 2];

export class FenceTool implements PointerTool {
  private pts: Pt[] = [];
  private cursor: Pt | null = null;
  /** ft per px, as last drawn. */
  private fpp = 1;
  private press: Pt | null = null;
  private stroke: Pt[] | null = null;

  constructor(private readonly host: FenceHost) {}

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    this.press = [x, y];
    this.stroke = null;
    return true;
  }

  move(x: number, y: number) {
    this.cursor = [x, y];
    const press = this.press;
    if (!press) return;
    const mode = this.host.settings().mode;
    if (mode === 'gate' || this.pts.length) return;
    // (The eraser takes a point every few feet, a fence a long step.)
    this.stroke = dragTo(this.stroke, press, [x, y], 6 * this.fpp, mode === 'erase' ? Math.max(2, this.fpp * 2) : Math.max(FREEHAND_FT, 3 * this.fpp));
  }

  up(x: number, y: number) {
    this.press = null;
    const mode = this.host.settings().mode;
    if (this.stroke) {
      const s = endAt(this.stroke, [x, y]);
      this.stroke = null;
      if (mode === 'erase') return this.host.gap(s.map(round));
      const pts: Pt[] = [];
      for (const p of simplifyOpen(s, MAX_POINTS).map(round)) {
        const q = pts[pts.length - 1];
        if (!q || Math.hypot(p[0] - q[0], p[1] - q[1]) >= 1) pts.push(p);
      }
      if (pts.length < 2) return;
      this.pts = pts;
      this.finish();
      return;
    }
    const p = round([x, y]);
    if (mode === 'gate') return this.host.gate(p);
    if (mode === 'erase') return this.host.gap([p]);
    if (!this.pts.length) {
      this.pts = [p];
      return this.host.hint('Click its next corners; press Enter or double-click to end it');
    }
    const first = this.pts[0];
    // A click back on its first corner closes it.
    if (this.pts.length >= 3 && Math.hypot(p[0] - first[0], p[1] - first[1]) < 8 * this.fpp) {
      this.finish(true);
      return;
    }
    const last = this.pts[this.pts.length - 1];
    if (Math.hypot(p[0] - last[0], p[1] - last[1]) < 1) return;
    if (this.pts.length >= MAX_POINTS) return this.host.hint(`A fence has at most ${MAX_POINTS} points: press Enter to end it`);
    this.pts.push(p);
  }

  /** End the line (Enter). True if there was one to end. */
  finish(closed = false): boolean {
    if (this.pts.length < 2) return false;
    const pts = this.pts;
    this.pts = [];
    this.host.placed(pts, closed);
    return true;
  }

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
    const s = this.host.settings();
    const ink = s.mode === 'erase' ? 0xdc2626 : 0x2563eb;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const line = (pts: Pt[], width: number, alpha: number) => {
      if (pts.length < 2) return;
      const s0 = scr(pts[0]);
      g.moveTo(s0[0], s0[1]);
      for (const p of pts.slice(1)) {
        const q = scr(p);
        g.lineTo(q[0], q[1]);
      }
      g.stroke({ width, color: ink, alpha, cap: 'round', join: 'round' });
    };
    if (this.stroke) {
      const pts = this.cursor ? [...this.stroke, this.cursor] : this.stroke;
      line(pts, s.mode === 'erase' ? Math.max(3, s.eraser * cam.ppf) : 2, s.mode === 'erase' ? 0.3 : 1);
      return;
    }
    if (this.cursor && !this.pts.length) {
      const [sx, sy] = scr(this.cursor);
      if (s.mode === 'erase') g.circle(sx, sy, Math.max(3, (s.eraser / 2) * cam.ppf)).stroke({ width: 1.5, color: ink, alpha: 0.8 });
      else g.circle(sx, sy, 3).fill({ color: ink });
    }
    if (!this.pts.length) return;
    const pts = this.cursor ? [...this.pts, this.cursor] : this.pts;
    line(pts, 2, 1);
    for (const p of this.pts) {
      const [sx, sy] = scr(p);
      g.rect(sx - 3, sy - 3, 6, 6).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
    }
  }
}
