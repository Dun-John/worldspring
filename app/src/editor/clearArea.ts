// Clear area: drag a box (or, as a lasso, any shape) over a town; the world's own buildings
// with their middle inside it are taken away in one change (one undo step), leaving the ground
// free to draw buildings of your own. Right-drag pans meanwhile.
import type { Graphics } from 'pixi.js';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import type { Pt } from './build';

export type ClearShape = 'box' | 'lasso';

/** What the tool needs from the app. */
export interface ClearHost {
  shape(): ClearShape;
  /** An area was drawn (world ft). */
  drawn(poly: Pt[]): void;
  hint(text: string): void;
}

/** Smallest box side, and the lasso's step between points (ft). */
const MIN_FT = 5;

export class ClearAreaTool implements PointerTool {
  private path: Pt[] = [];
  private dragging = false;
  /** The area just sent, shown until the app answers. */
  pending: Pt[] | null = null;

  constructor(private readonly host: ClearHost) {}

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    this.dragging = true;
    this.path = [[x, y]];
    return true;
  }

  move(x: number, y: number) {
    if (!this.dragging) return;
    const last = this.path[this.path.length - 1];
    if (this.host.shape() === 'box') this.path = [this.path[0], [x, y]];
    else if (Math.hypot(x - last[0], y - last[1]) >= MIN_FT) this.path.push([x, y]);
  }

  up(x: number, y: number) {
    if (!this.dragging) return;
    this.dragging = false;
    this.move(x, y);
    const poly = this.area();
    this.path = [];
    if (!poly) return this.host.hint(this.host.shape() === 'box' ? 'Drag a box over the buildings to take away' : 'Draw round the buildings to take away');
    this.pending = poly;
    this.host.drawn(poly);
  }

  cancel() {
    this.dragging = false;
    this.path = [];
  }

  hover() {}

  /** The area being drawn as a polygon, if it is one. */
  private area(): Pt[] | null {
    const p = this.path;
    if (this.host.shape() === 'box') {
      if (p.length < 2) return null;
      const [[x0, y0], [x1, y1]] = [p[0], p[1]];
      if (Math.abs(x1 - x0) < MIN_FT || Math.abs(y1 - y0) < MIN_FT) return null;
      return [
        [Math.min(x0, x1), Math.min(y0, y1)],
        [Math.max(x0, x1), Math.min(y0, y1)],
        [Math.max(x0, x1), Math.max(y0, y1)],
        [Math.min(x0, x1), Math.max(y0, y1)],
      ];
    }
    return p.length >= 3 ? p.slice() : null;
  }

  draw(g: Graphics, cam: Camera) {
    const ink = 0xdc2626;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    for (const poly of [this.pending, this.dragging ? this.area() ?? (this.path.length >= 2 ? this.path : null) : null]) {
      if (!poly) continue;
      const flat = poly.flatMap((p) => scr(p));
      if (poly.length >= 3) g.poly(flat).fill({ color: ink, alpha: 0.14 });
      g.poly(flat, poly.length >= 3).stroke({ width: 2, color: ink, alpha: 0.85 });
    }
  }
}
