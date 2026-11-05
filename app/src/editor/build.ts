// Build: draw a building's footprint on the map, snapped to the 5-ft grid. A rectangle is
// dragged corner to corner; a polygon is clicked corner by corner (finished on its first
// corner, a double click or Enter; Backspace takes the last corner back); a round tower is
// dragged from its middle out. Right-drag pans meanwhile. The finished footprint goes to the
// app, which checks it with the generator (dry land, clear of buildings, roads and walls).
import type { Graphics } from 'pixi.js';
import { TINT_MATERIAL, type Created, type RoofMaterial, type RoofShape, type RoofStyle } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';

export type BuildShape = 'rect' | 'poly' | 'tower';
export type Pt = [number, number];

/** The build menu's choices: the shape drawn and the building's options. */
export interface BuildSettings {
  shape: BuildShape;
  /** A business key or a home (`BuildingFuncs`). */
  func: string;
  floors: number;
  /** '' as its kind has it. */
  roof: RoofStyle | '';
  /** Roof material and shape; '' picked. */
  material: RoofMaterial | '';
  rshape: RoofShape | '';
  /** Colour variant 0-4 of the material (`ROOF_SHADES`); '' picked. */
  shade: number | '';
  ruin: boolean;
  /** '' named for its trade. */
  name: string;
}

export const defaultBuild = (): BuildSettings => ({ shape: 'rect', func: 'house', floors: 1, roof: '', material: '', rshape: '', shade: '', ruin: false, name: '' });

/** A building's options from the menu's choices (as `Created` holds them). */
export function buildOptions(s: BuildSettings): Pick<Created, 'func' | 'floors' | 'roof' | 'material' | 'shape' | 'shade' | 'structure'> {
  const o: Pick<Created, 'func' | 'floors' | 'roof' | 'material' | 'shape' | 'shade' | 'structure'> = { func: s.func, floors: s.floors };
  if (s.roof) o.roof = s.roof;
  if (s.material) o.material = s.material;
  if (s.rshape) o.shape = s.rshape;
  if (s.shade !== '') o.shade = s.shade;
  if (s.ruin) o.structure = 'ruin';
  return o;
}

/** The menu's choices for a building drawn earlier. */
export function settingsOf(c: Created, shape: BuildShape): BuildSettings {
  return { shape, func: c.func ?? 'house', floors: c.floors ?? 1, roof: c.roof ?? '', material: c.material ?? (c.tint ? TINT_MATERIAL[c.tint] : ''), rshape: c.shape ?? '', shade: c.shade ?? '', ruin: c.structure === 'ruin', name: c.name };
}

const GRID = 5;
/** Corners of a round tower. */
const TOWER_SIDES = 16;
/** Smallest side (ft). */
const MIN_FT = 10;
const MAX_CORNERS = 64;

const snap = (v: number) => Math.round(v / GRID) * GRID;

/** What the tool needs from the app. */
export interface BuildHost {
  shape(): BuildShape;
  /** A footprint was drawn (world ft). */
  drawn(poly: Pt[]): void;
  hint(text: string): void;
}

/** A round tower's footprint about (cx, cy). */
export function towerPoly(cx: number, cy: number, r: number): Pt[] {
  const pts: Pt[] = [];
  for (let k = 0; k < TOWER_SIDES; k++) {
    const a = (k / TOWER_SIDES) * Math.PI * 2;
    pts.push([Math.round((cx + r * Math.cos(a)) * 100) / 100, Math.round((cy + r * Math.sin(a)) * 100) / 100]);
  }
  return pts;
}

export class BuildTool implements PointerTool {
  /** A polygon's corners so far. */
  private corners: Pt[] = [];
  /** A rectangle's or tower's drag: from, to (snapped). */
  private drag: { from: Pt; to: Pt } | null = null;
  private cursor: Pt | null = null;
  /** The footprint just sent, shown until the app answers. */
  pending: Pt[] | null = null;

  constructor(private readonly host: BuildHost) {}

  /** The footprint being drawn, if any. */
  private shapeNow(): Pt[] | null {
    const shape = this.host.shape();
    if (shape === 'poly') {
      if (!this.corners.length) return null;
      const c = this.cursor;
      return c && !same(c, this.corners[this.corners.length - 1]) ? [...this.corners, c] : this.corners;
    }
    if (!this.drag) return null;
    const { from, to } = this.drag;
    if (shape === 'rect') return [from, [to[0], from[1]], to, [from[0], to[1]]];
    return towerPoly(from[0], from[1], towerRadius(from, to));
  }

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    const p: Pt = [snap(x), snap(y)];
    if (this.host.shape() !== 'poly') this.drag = { from: p, to: p };
    return true;
  }

  move(x: number, y: number) {
    this.cursor = [snap(x), snap(y)];
    if (this.drag) this.drag.to = this.cursor;
  }

  up(x: number, y: number) {
    const p: Pt = [snap(x), snap(y)];
    const shape = this.host.shape();
    if (shape === 'poly') return this.corner(p);
    const d = this.drag;
    this.drag = null;
    if (!d) return;
    d.to = p;
    if (shape === 'rect') {
      const [w, h] = [Math.abs(d.to[0] - d.from[0]), Math.abs(d.to[1] - d.from[1])];
      if (w < MIN_FT || h < MIN_FT) return this.host.hint(`Drag out a rectangle at least ${MIN_FT} ft a side`);
      const [x0, y0, x1, y1] = [Math.min(d.from[0], d.to[0]), Math.min(d.from[1], d.to[1]), Math.max(d.from[0], d.to[0]), Math.max(d.from[1], d.to[1])];
      return this.send([
        [x0, y0],
        [x1, y0],
        [x1, y1],
        [x0, y1],
      ]);
    }
    const r = towerRadius(d.from, d.to);
    if (r * 2 < MIN_FT) return this.host.hint(`Drag from the middle out: a tower at least ${MIN_FT} ft across`);
    this.send(towerPoly(d.from[0], d.from[1], r));
  }

  /** A click while drawing a polygon: a corner, or on the first corner, the end. */
  private corner(p: Pt) {
    const c = this.corners;
    if (c.length >= 3 && same(p, c[0])) return this.finish();
    if (c.length && same(p, c[c.length - 1])) return;
    if (c.length >= MAX_CORNERS) return this.host.hint(`At most ${MAX_CORNERS} corners: click the first corner to finish`);
    c.push(p);
    if (c.length === 1) this.host.hint('Click each corner; click the first again (or press Enter) to finish');
  }

  /** Finish the polygon being drawn. */
  finish() {
    const c = this.corners;
    if (this.host.shape() !== 'poly') return;
    if (c.length < 3) return this.host.hint('A building needs at least 3 corners');
    this.corners = [];
    this.send(c);
  }

  /** Take the polygon's last corner back. */
  back() {
    this.corners.pop();
  }

  /** Drop what is being drawn. True if there was something. */
  reset(): boolean {
    const had = !!this.corners.length || !!this.drag;
    this.corners = [];
    this.drag = null;
    return had;
  }

  dblclick(): boolean {
    if (this.host.shape() === 'poly' && this.corners.length >= 3) this.finish();
    return true;
  }

  cancel() {
    this.drag = null;
  }

  hover(x: number, y: number) {
    this.cursor = [snap(x), snap(y)];
  }

  private send(poly: Pt[]) {
    this.pending = poly;
    this.host.drawn(poly);
  }

  draw(g: Graphics, cam: Camera) {
    const ink = 0xf97316;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    if (this.pending) {
      g.poly(this.pending.flatMap((p) => scr(p))).fill({ color: ink, alpha: 0.18 });
      g.poly(this.pending.flatMap((p) => scr(p))).stroke({ width: 1.5, color: ink, alpha: 0.6 });
    }
    // The grid point under the pointer.
    if (this.cursor) {
      const [sx, sy] = scr(this.cursor);
      g.circle(sx, sy, 3).fill({ color: ink });
    }
    const pts = this.shapeNow();
    if (!pts) return;
    const flat = pts.flatMap((p) => scr(p));
    const poly = this.host.shape() === 'poly';
    if (pts.length >= 3) g.poly(flat).fill({ color: ink, alpha: 0.22 });
    if (poly) {
      for (let i = 0; i + 1 < pts.length; i++) {
        const [a, b] = [scr(pts[i]), scr(pts[i + 1])];
        g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
      }
      g.stroke({ width: 2, color: ink });
      for (const c of this.corners) {
        const [sx, sy] = scr(c);
        g.rect(sx - 3, sy - 3, 6, 6).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
      }
    } else {
      g.poly(flat).stroke({ width: 2, color: ink });
    }
  }
}

function same(a: Pt, b: Pt): boolean {
  return a[0] === b[0] && a[1] === b[1];
}

/** A tower's radius from its middle to the pointer: whole 5-ft diameters. */
function towerRadius(from: Pt, to: Pt): number {
  return Math.round(Math.hypot(to[0] - from[0], to[1] - from[1]) / (GRID / 2)) * (GRID / 2);
}
