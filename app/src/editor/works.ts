// Build › Castle and Wall: a castle's outline is dragged as a rectangle or clicked corner by
// corner (a convex polygon), then a click on one of its sides puts the gate there (Enter: the
// side facing the nearest road). A wall is clicked corner by corner and finished with a double
// click or Enter (on its first corner too when it is a ring); a click on a corner as it is drawn
// makes that corner a gate. With a castle or wall picked to change, a click on its line moves
// the castle's gate there, or adds a gate to the wall (on a gate, takes it away). Everything
// snaps to the 5-ft grid; right-drag pans meanwhile. The app checks the result with the
// generator (dry land, clear of walls and drawn buildings; the town's own buildings in the way
// can go with it).
import type { Graphics } from 'pixi.js';
import { CASTLE_MAX_FT, CASTLE_MIN_FT, WALL_MAX_FT, type Created } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import type { Pt } from './build';

export type CastleShape = 'rect' | 'poly';

/** The Castle and Wall modes' choices. */
export interface WorksSettings {
  castleShape: CastleShape;
  keep: boolean;
  yardBuildings: boolean;
  ruin: boolean;
  /** A wall drawn as a ring. */
  closed: boolean;
  /** '' named for the place. */
  name: string;
}

export const defaultWorks = (): WorksSettings => ({ castleShape: 'rect', keep: true, yardBuildings: true, ruin: false, closed: false, name: '' });

const GRID = 5;
const MAX_CASTLE_CORNERS = 32;
const MAX_WALL_CORNERS = 64;
/** How near (screen px) a click must be to a line or corner to pick it. */
const PICK_PX = 12;

const snap = (v: number) => Math.round(v / GRID) * GRID;
const same = (a: Pt, b: Pt) => a[0] === b[0] && a[1] === b[1];

/** What the tool needs from the app. */
export interface WorksHost {
  mode(): 'castle' | 'wall';
  settings(): WorksSettings;
  /** The castle or wall being changed, if any. */
  editing(): Created | null;
  /** A castle's outline and its gate side (null: facing the road). */
  castle(poly: Pt[], gate: number | null): void;
  /** A wall's line and gate corners. */
  wall(pts: Pt[], gates: number[], closed: boolean): void;
  /** The castle or wall being changed gets a new gate (a castle's side) or gates (a wall's). */
  gates(change: { gate: number } | { pts: Pt[]; gates: number[] }): void;
  hint(text: string): void;
}

/** Whether a polygon turns the same way at every corner. */
export function convex(p: Pt[]): boolean {
  let sign = 0;
  for (let i = 0; i < p.length; i++) {
    const [a, b, c] = [p[i], p[(i + 1) % p.length], p[(i + 2) % p.length]];
    const z = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
    if (Math.abs(z) < 1e-6) continue;
    if (sign && Math.sign(z) !== sign) return false;
    sign = Math.sign(z);
  }
  return true;
}

/** A castle outline's problem, if any (as `Created::check_castle` has them). */
export function castleProblem(p: Pt[]): string | null {
  if (p.length < 3) return 'A castle needs at least 3 corners';
  if (!convex(p)) return 'A castle’s outline must be convex: no corner turning inward';
  let across = 0;
  for (const a of p) for (const b of p) across = Math.max(across, Math.hypot(a[0] - b[0], a[1] - b[1]));
  if (across > CASTLE_MAX_FT) return `A castle is at most ${CASTLE_MAX_FT} ft across (this is ${Math.round(across)})`;
  // Narrowest: the least width over every side's direction.
  let narrow = Infinity;
  for (let i = 0; i < p.length; i++) {
    const [a, b] = [p[i], p[(i + 1) % p.length]];
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (l < 1e-9) continue;
    const n = [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
    const ds = p.map((q) => (q[0] - a[0]) * n[0] + (q[1] - a[1]) * n[1]);
    narrow = Math.min(narrow, Math.max(...ds) - Math.min(...ds));
  }
  if (narrow < CASTLE_MIN_FT) return `A castle is at least ${CASTLE_MIN_FT} ft across: room for its walls, a yard and a keep`;
  return null;
}

/** Distance from p to the segment a–b, and how far along it (0–1). */
function toSeg(p: Pt, a: Pt, b: Pt): [number, number] {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const l2 = dx * dx + dy * dy;
  const t = l2 > 0 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2)) : 0;
  return [Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy), t];
}

export class WorksTool implements PointerTool {
  /** Corners so far (a polygon castle, a wall). */
  private corners: Pt[] = [];
  /** A wall's corners marked gates as it is drawn. */
  private gateMarks = new Set<number>();
  /** A rectangle castle's drag. */
  private drag: { from: Pt; to: Pt } | null = null;
  /** A castle outline drawn, waiting for its gate side. */
  private outline: Pt[] | null = null;
  private cursor: Pt | null = null;
  /** What was just sent, shown until the app answers. */
  pending: Pt[] | null = null;
  pendingClosed = false;
  private scale = 1;

  constructor(private readonly host: WorksHost) {}

  private get castleMode() {
    return this.host.mode() === 'castle';
  }

  /** Feet per screen pixel, from the last frame. */
  private pickFt() {
    return PICK_PX * this.scale;
  }

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    if (this.castleMode && !this.outline && this.host.settings().castleShape === 'rect' && !this.pickOnEditing([x, y], true)) {
      const p: Pt = [snap(x), snap(y)];
      this.drag = { from: p, to: p };
    }
    return true;
  }

  move(x: number, y: number) {
    this.cursor = [snap(x), snap(y)];
    if (this.drag) this.drag.to = this.cursor;
  }

  up(x: number, y: number) {
    const raw: Pt = [x, y];
    const p: Pt = [snap(x), snap(y)];
    if (this.castleMode) {
      if (this.outline) return this.pickGate(raw);
      const d = this.drag;
      this.drag = null;
      if (this.host.settings().castleShape === 'rect') {
        // (No drag: the press was on the castle being changed.)
        if (!d) return void this.pickOnEditing(raw, false);
        const [w, h] = [Math.abs(p[0] - d.from[0]), Math.abs(p[1] - d.from[1])];
        if (w < 1 && h < 1) return this.host.hint('Drag from corner to corner to draw the castle’s walls');
        const [x0, y0, x1, y1] = [Math.min(d.from[0], p[0]), Math.min(d.from[1], p[1]), Math.max(d.from[0], p[0]), Math.max(d.from[1], p[1])];
        return this.outlined([
          [x0, y0],
          [x1, y0],
          [x1, y1],
          [x0, y1],
        ]);
      }
      if (!this.corners.length && this.pickOnEditing(raw, false)) return;
      return this.corner(p, MAX_CASTLE_CORNERS);
    }
    if (!this.corners.length && this.pickOnEditing(raw, false)) return;
    this.corner(p, MAX_WALL_CORNERS);
  }

  /** A click on the castle or wall being changed (not while drawing a new one): its gate moved,
   * or a gate added or taken away. `test`: only say whether it is on it. */
  private pickOnEditing(p: Pt, test: boolean): boolean {
    const c = this.host.editing();
    if (!c) return false;
    const reach = this.pickFt();
    if (c.kind === 'castle' && this.castleMode && c.poly) {
      const side = this.nearestSide(c.poly, p);
      if (!side || side[1] > reach) return false;
      if (!test) this.host.gates({ gate: side[0] });
      return true;
    }
    if (c.kind === 'wall' && !this.castleMode && c.pts) {
      const pts = c.pts.map((q) => [q[0], q[1]] as Pt);
      const gates = new Set(c.gates ?? []);
      const k = pts.findIndex((q) => Math.hypot(q[0] - p[0], q[1] - p[1]) <= reach);
      if (k >= 0) {
        if (test) return true;
        if (gates.has(k)) gates.delete(k);
        else gates.add(k);
        this.host.gates({ pts, gates: [...gates].sort((a, b) => a - b) });
        return true;
      }
      const side = this.nearestSide(pts, p, !c.closed);
      if (!side || side[1] > reach) return false;
      if (test) return true;
      // A new corner on the line, a gate.
      const [i, , t] = side;
      const [a, b] = [pts[i], pts[(i + 1) % pts.length]];
      const at: Pt = [snap(a[0] + (b[0] - a[0]) * t), snap(a[1] + (b[1] - a[1]) * t)];
      if (same(at, a) || same(at, b)) return true;
      pts.splice(i + 1, 0, at);
      const moved = [...gates].map((g) => (g > i ? g + 1 : g));
      this.host.gates({ pts, gates: [...moved, i + 1].sort((x, y) => x - y) });
      return true;
    }
    return false;
  }

  /** The side of a ring (or line) nearest p: its index, distance and place along it. */
  private nearestSide(pts: Pt[], p: Pt, open = false): [number, number, number] | null {
    let best: [number, number, number] | null = null;
    const n = pts.length;
    for (let i = 0; i < (open ? n - 1 : n); i++) {
      const [d, t] = toSeg(p, pts[i], pts[(i + 1) % n]);
      if (!best || d < best[1]) best = [i, d, t];
    }
    return best;
  }

  /** A click while drawing corners: a corner; on the first (a castle, a ring wall), the end; on
   * the last of a wall's, a gate there. */
  private corner(p: Pt, max: number) {
    const c = this.corners;
    const ring = this.castleMode || this.host.settings().closed;
    if (c.length >= 3 && ring && same(p, c[0])) return this.finish();
    if (c.length && same(p, c[c.length - 1])) {
      if (!this.castleMode) {
        const k = c.length - 1;
        if (this.gateMarks.has(k)) this.gateMarks.delete(k);
        else this.gateMarks.add(k);
      }
      return;
    }
    if (c.length >= max) return this.host.hint(`At most ${max} corners: ${ring ? 'click the first corner' : 'double-click or press Enter'} to finish`);
    if (!this.castleMode && c.length && wallLen([...c, p], false) > WALL_MAX_FT) return this.host.hint(`A wall is at most ${WALL_MAX_FT} ft long`);
    c.push(p);
    if (c.length === 1)
      this.host.hint(
        this.castleMode
          ? 'Click each corner; click the first again (or press Enter) to close the walls'
          : `Click each corner (click one again to make it a gate); ${ring ? 'click the first again,' : ''} double-click or press Enter to finish`,
      );
  }

  /** Finish what is being drawn (Enter): a polygon castle's outline, a wall; a castle waiting
   * for its gate gets the side facing the road. */
  finish() {
    if (this.outline) return this.sendCastle(this.outline, null);
    const c = this.corners;
    if (this.castleMode) {
      if (c.length < 3) return this.host.hint('A castle needs at least 3 corners');
      this.corners = [];
      return this.outlined(c);
    }
    if (c.length < 2) return this.host.hint('A wall needs at least 2 corners');
    const closed = this.host.settings().closed;
    if (closed && c.length < 3) return this.host.hint('A ring of wall needs at least 3 corners');
    const gates = [...this.gateMarks].sort((a, b) => a - b);
    this.corners = [];
    this.gateMarks.clear();
    this.pending = c;
    this.pendingClosed = closed;
    this.host.wall(c, gates, closed);
  }

  /** A castle's outline is drawn: checked, then its gate side is asked for. */
  private outlined(poly: Pt[]) {
    const problem = castleProblem(poly);
    if (problem) return this.host.hint(problem);
    this.outline = poly;
    this.host.hint('Click the side the gate is in (Enter: the side facing the road)');
  }

  private pickGate(p: Pt) {
    const o = this.outline;
    if (!o) return;
    const side = this.nearestSide(o, p);
    if (!side || side[1] > Math.max(this.pickFt(), 40)) return this.host.hint('Click one of the sides for the gate, or press Enter for the side facing the road');
    this.sendCastle(o, side[0]);
  }

  private sendCastle(poly: Pt[], gate: number | null) {
    this.outline = null;
    this.pending = poly;
    this.pendingClosed = true;
    this.host.castle(poly, gate);
  }

  /** Take the last corner back. */
  back() {
    if (this.outline) {
      this.outline = null;
      return;
    }
    this.gateMarks.delete(this.corners.length - 1);
    this.corners.pop();
  }

  /** Drop what is being drawn. True if there was something. */
  reset(): boolean {
    const had = !!this.corners.length || !!this.drag || !!this.outline;
    this.corners = [];
    this.gateMarks.clear();
    this.drag = null;
    this.outline = null;
    return had;
  }

  dblclick(): boolean {
    if (this.castleMode ? this.corners.length >= 3 : this.corners.length >= 2) this.finish();
    return true;
  }

  cancel() {
    this.drag = null;
  }

  hover(x: number, y: number) {
    this.cursor = [snap(x), snap(y)];
  }

  draw(g: Graphics, cam: Camera) {
    const ink = 0x7c3aed;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const [ax] = cam.worldToScreen(0, 0);
    const [bx] = cam.worldToScreen(100, 0);
    this.scale = 100 / Math.max(1e-6, Math.abs(bx - ax));
    const line = (pts: Pt[], closed: boolean, width: number, alpha = 1) => {
      if (pts.length < 2) return;
      const s = pts.map(scr);
      g.moveTo(s[0][0], s[0][1]);
      for (const q of s.slice(1)) g.lineTo(q[0], q[1]);
      if (closed) g.lineTo(s[0][0], s[0][1]);
      g.stroke({ width, color: ink, alpha });
    };
    if (this.pending) {
      if (this.pendingClosed && this.pending.length >= 3) g.poly(this.pending.flatMap(scr)).fill({ color: ink, alpha: 0.12 });
      line(this.pending, this.pendingClosed, 3, 0.6);
    }
    // The castle or wall being changed: its line, gates marked.
    const ed = this.host.editing();
    if (ed && !this.corners.length && !this.outline && !this.drag) {
      const pts = (ed.kind === 'castle' ? ed.poly : ed.pts) ?? [];
      line(pts, ed.kind === 'castle' || !!ed.closed, 2, 0.5);
      if (ed.kind === 'castle' && ed.gate != null && pts.length) {
        const [a, b] = [pts[ed.gate], pts[(ed.gate + 1) % pts.length]];
        const [sx, sy] = scr([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]);
        g.rect(sx - 5, sy - 5, 10, 10).fill({ color: ink });
      }
      for (const k of ed.kind === 'wall' ? (ed.gates ?? []) : []) {
        if (!pts[k]) continue;
        const [sx, sy] = scr(pts[k]);
        g.rect(sx - 5, sy - 5, 10, 10).fill({ color: ink });
      }
    }
    if (this.cursor) {
      const [sx, sy] = scr(this.cursor);
      g.circle(sx, sy, 3).fill({ color: ink });
    }
    if (this.outline) {
      g.poly(this.outline.flatMap(scr)).fill({ color: ink, alpha: 0.18 });
      line(this.outline, true, 4);
      // The side the pointer is over would take the gate.
      const side = this.cursor && this.nearestSide(this.outline, this.cursor);
      if (side) line([this.outline[side[0]], this.outline[(side[0] + 1) % this.outline.length]], false, 8, 0.8);
      return;
    }
    if (this.drag) {
      const { from, to } = this.drag;
      const r: Pt[] = [from, [to[0], from[1]], to, [from[0], to[1]]];
      g.poly(r.flatMap(scr)).fill({ color: ink, alpha: 0.18 });
      line(r, true, 3);
      return;
    }
    const c = this.corners;
    if (!c.length) return;
    const pts = this.cursor && !same(this.cursor, c[c.length - 1]) ? [...c, this.cursor] : c;
    const ring = this.castleMode || this.host.settings().closed;
    if (this.castleMode && pts.length >= 3) g.poly(pts.flatMap(scr)).fill({ color: ink, alpha: 0.18 });
    line(pts, false, 3);
    if (ring && pts.length >= 3) line([pts[pts.length - 1], pts[0]], false, 1.5, 0.5);
    c.forEach((q, k) => {
      const [sx, sy] = scr(q);
      if (this.gateMarks.has(k)) g.rect(sx - 5, sy - 5, 10, 10).fill({ color: ink });
      else g.rect(sx - 3, sy - 3, 6, 6).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
    });
  }
}

function wallLen(pts: Pt[], closed: boolean): number {
  let s = 0;
  for (let i = 0; i < (closed ? pts.length : pts.length - 1); i++) s += Math.hypot(pts[(i + 1) % pts.length][0] - pts[i][0], pts[(i + 1) % pts.length][1] - pts[i][1]);
  return s;
}
