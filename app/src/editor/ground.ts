// Ground: the battlemap's ground raised, lowered, smoothed or put back by a round brush
// (`Edits.ground`: blocks of 16 × 16 squares of change), and plateaus or pits drawn as loops
// (`Edits.plateaus`: an outline, a level and an edge). A stroke or a loop is one change (one undo
// step), made when it ends; meanwhile the ground under it is shown as it will be (the map's
// preview) and stays so while its chunks are made again.
//
// A brush square takes the strongest touch of its stroke, so scrubbing back and forth doesn't
// pile up; the values written are what `worldgen::ground::stamp_brush` would write for the same
// touches. A loop's level is the plateau's it starts on, else the ground there plus the offset.
import type { Edits, Plateau } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { Graphics } from 'pixi.js';
import type { PointerTool } from '../render/MapView';
import { added, BLOCK, blockKey, encode, MAX_ADD, MAX_BLOCKS, MAX_EDGE_FT, MAX_PLATEAU_PTS, MAX_PLATEAU_SPAN_FT, MAX_STROKE_BLOCKS, values } from './groundCodec';
import { newObjectId } from './scatter';

export type GroundMode = 'raise' | 'lower' | 'smooth' | 'restore' | 'plateau' | 'select';
export type Brush = 'raise' | 'lower' | 'smooth' | 'restore';

export interface GroundSettings {
  mode: GroundMode;
  /** Brush radius (ft). */
  radius: number;
  /** Raise and Lower: at most this far (ft). */
  height: number;
  /** 0 soft (the touch fades from the middle) to 1 hard (full out to the rim). */
  hardness: number;
  /** Smooth and Restore: how far toward their goal (0 to 1). */
  strength: number;
  /** Plateau: its level above the ground where the loop starts (ft; below: a pit). */
  offset: number;
  /** Plateau: its edge (ft; 0 a sheer cliff). */
  edge: number;
}

export const defaultGround = (): GroundSettings => ({ mode: 'raise', radius: 15, height: 5, hardness: 0.5, strength: 0.5, offset: 10, edge: 0 });

export const MAX_PLATEAUS = 256;
const SQUARE_FT = 5;

/** What the tool needs from the app. */
export interface GroundHost {
  settings(): GroundSettings;
  edits(): Edits;
  /** Whether the battlemap is showing (and we are not inside a building or site). */
  battlemap(): boolean;
  /** A square's shown ground and the water there (ft), or null where it isn't loaded. */
  ground(gx: number, gy: number): [number, number] | null;
  /** The sea's level (ft): heights are shown above it. */
  sea(): number;
  /** Show squares (global, inclusive box) as they will be: `at` gives a square's height from
   * its shown height and water, or null to leave it. */
  preview(box: [number, number, number, number], at: (gx: number, gy: number, h: number, water: number) => number | null): void;
  /** The ground as it is shown again (a stroke cancelled, or one that changed nothing). */
  unpreview(): void;
  /** Make the change (one undo step). */
  commit(ground: Record<string, string | null>, plateaus: Record<string, Plateau | null>, label: { tool: string; mode?: string; count?: number; changed?: boolean }): void;
  /** The brush ring under the pointer (world ft), or none. */
  ring(x: number, y: number, r: number | null): void;
  /** The chip by the pointer (world ft), or none. */
  chip(x: number, y: number, text: string | null): void;
  /** The plateau chosen (Select), or none. */
  select(id: string | null): void;
  selected(): string | null;
  /** Screen px per world ft. */
  ppf(): number;
  hint(text: string): void;
}

const smoothstep = (a: number, b: number, x: number) => {
  const t = Math.max(0, Math.min(1, (x - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

function segDist(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const [dx, dy] = [bx - ax, by - ay];
  const l = dx * dx + dy * dy;
  const t = l > 0 ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / l)) : 0;
  return Math.hypot(px - ax - t * dx, py - ay - t * dy);
}

function contains(pts: [number, number][], x: number, y: number): boolean {
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [[xi, yi], [xj, yj]] = [pts[i], pts[j]];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

function outlineDist(pts: [number, number][], x: number, y: number): number {
  let d = Infinity;
  for (let i = 0; i < pts.length; i++) {
    const [a, b] = [pts[i], pts[(i + 1) % pts.length]];
    d = Math.min(d, segDist(x, y, a[0], a[1], b[0], b[1]));
  }
  return d;
}

function area(pts: [number, number][]): number {
  let s = 0;
  for (let i = 0; i < pts.length; i++) {
    const [a, b] = [pts[i], pts[(i + 1) % pts.length]];
    s += a[0] * b[1] - b[0] * a[1];
  }
  return Math.abs(s) / 2;
}

function bbox(pts: [number, number][]): [number, number, number, number] {
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const [x, y] of pts) [x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x), Math.max(y1, y)];
  return [x0, y0, x1, y1];
}

function crossesItself(pts: [number, number][]): boolean {
  const n = pts.length;
  const side = (a: number[], b: number[], c: number[]) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  for (let i = 0; i < n; i++) {
    const [a, b] = [pts[i], pts[(i + 1) % n]];
    for (let j = i + 1; j < n; j++) {
      if (j === i + 1 || (i === 0 && j === n - 1)) continue;
      const [c, d] = [pts[j], pts[(j + 1) % n]];
      const [d1, d2, d3, d4] = [side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b)];
      if (d1 > 0 !== d2 > 0 && d3 > 0 !== d4 > 0 && d1 && d2 && d3 && d4) return true;
    }
  }
  return false;
}

/** Douglas–Peucker on a closed outline: split at the point farthest from the first (as
 * `ground::plateau_for`). */
function simplify(pts: [number, number][], tol: number): [number, number][] {
  if (pts.length <= 3) return pts.slice();
  let far = 1;
  for (let i = 1; i < pts.length; i++) if (Math.hypot(pts[i][0] - pts[0][0], pts[i][1] - pts[0][1]) > Math.hypot(pts[far][0] - pts[0][0], pts[far][1] - pts[0][1])) far = i;
  const out: [number, number][] = [];
  const dp = (p: [number, number][]) => {
    const [a, b] = [p[0], p[p.length - 1]];
    let [best, at] = [0, 0];
    for (let i = 1; i < p.length - 1; i++) {
      const d = segDist(p[i][0], p[i][1], a[0], a[1], b[0], b[1]);
      if (d > best) [best, at] = [d, i];
    }
    if (best > tol) {
      dp(p.slice(0, at + 1));
      dp(p.slice(at));
    } else out.push(a);
  };
  dp(pts.slice(0, far + 1));
  dp([...pts.slice(far), pts[0]]);
  return out;
}

/** A plateau from a loop drawn by hand, or what is wrong with it. */
export function plateauFor(outline: [number, number][], level: number, edge: number): Plateau | string {
  const pts: [number, number][] = [];
  for (const p of outline) {
    const q = pts[pts.length - 1];
    if (!q || Math.hypot(p[0] - q[0], p[1] - q[1]) >= 5) pts.push(p);
  }
  while (pts.length > 3 && Math.hypot(pts[0][0] - pts[pts.length - 1][0], pts[0][1] - pts[pts.length - 1][1]) < 5) pts.pop();
  let tol = 0.5;
  let out = simplify(pts, tol);
  while (out.length > MAX_PLATEAU_PTS) {
    tol *= 1.5;
    out = simplify(pts, tol);
  }
  const p: Plateau = { pts: out.map(([x, y]) => [Math.round(x * 2) / 2, Math.round(y * 2) / 2]), level: Math.round(level * 10) / 10 };
  const e = Math.max(0, Math.min(MAX_EDGE_FT, Math.round(edge)));
  if (e) p.edge = e;
  return plateauProblem(p) ?? p;
}

/** What is wrong with a plateau, if anything (`Plateau::problem`, as people would put it). */
export function plateauProblem(p: Plateau): string | null {
  if (p.pts.length < 3) return 'Drag a loop round the ground to raise';
  if (!p.pts.every(([x, y]) => Number.isFinite(x) && Number.isFinite(y)) || !Number.isFinite(p.level) || Math.abs(p.level) > 30000) return 'That level is out of reach';
  const [x0, y0, x1, y1] = bbox(p.pts);
  if (x1 - x0 > MAX_PLATEAU_SPAN_FT || y1 - y0 > MAX_PLATEAU_SPAN_FT) return `A plateau spans at most ${MAX_PLATEAU_SPAN_FT.toLocaleString('en')} ft`;
  if (area(p.pts) < 25) return 'That loop holds no ground';
  if (crossesItself(p.pts)) return 'The loop crosses itself: draw it round once';
  return null;
}

/** How a plateau's level reads (ft above the sea). */
const above = (level: number, sea: number) => `${Math.round(level - sea).toLocaleString('en')} ft`;
const signed = (v: number) => `${v >= 0 ? '+' : '−'}${Math.abs(Math.round(v))}`;
const BIG = 1 << 21;

export class GroundTool implements PointerTool {
  /** A brush stroke: each square's strongest touch (by `gx * BIG + gy`), and its blocks. */
  private touch: Map<number, number> | null = null;
  private blocks = new Set<string>();
  private full = false;
  private last: [number, number] | null = null;
  /** A loop being drawn, and the level it will have. */
  private loop: { pts: [number, number][]; level: number; on: string | null } | null = null;
  private at: [number, number] | null = null;
  /** Select: the plateau under the pointer. */
  private under: string | null = null;
  /** The ground shown differs from the edits (a stroke under way, or one just made). */
  private previewing = false;
  readonly hoverFollows = true;
  readonly pinchCancels = true;

  constructor(private readonly host: GroundHost) {}

  private brush(): Brush | null {
    const m = this.host.settings().mode;
    return m === 'plateau' || m === 'select' ? null : m;
  }

  /** The plateau at a point: the smallest holding it, else one whose outline is within `near` ft. */
  plateauAt(x: number, y: number, near = 0): string | null {
    let best: string | null = null;
    let score = Infinity;
    for (const [id, p] of Object.entries(this.host.edits().plateaus ?? {})) {
      const pts = p.pts as [number, number][];
      const [x0, y0, x1, y1] = bbox(pts);
      if (x < x0 - near || x > x1 + near || y < y0 - near || y > y1 + near) continue;
      const s = contains(pts, x, y) ? area(pts) : outlineDist(pts, x, y) <= near ? 1e12 + outlineDist(pts, x, y) : Infinity;
      if (s < score) [best, score] = [id, s];
    }
    return best;
  }

  /** The level a loop started at (x, y) would have, and the plateau it extends. */
  private levelAt(x: number, y: number): { level: number; on: string | null } | null {
    const on = this.plateauAt(x, y);
    const p = on ? this.host.edits().plateaus?.[on] : null;
    if (p) return { level: p.level, on };
    const g = this.host.ground(Math.floor(x / SQUARE_FT), Math.floor(y / SQUARE_FT));
    return g ? { level: g[0] + this.host.settings().offset, on: null } : null;
  }

  private levelChip(x: number, y: number) {
    const s = this.host.settings();
    const l = this.loop ?? this.levelAt(x, y);
    const r = 12 / this.host.ppf();
    if (!l) return this.host.chip(x + r, y + r, null);
    this.host.chip(x + r, y + r, l.on ? `Level ${above(l.level, this.host.sea())} · on the plateau` : `Level ${above(l.level, this.host.sea())} (${signed(s.offset)})`);
  }

  /** The settings changed (the panel): the ring and chip as they now are. */
  refresh() {
    const at = this.at;
    const s = this.host.settings();
    if (!at || !this.host.battlemap()) {
      this.host.ring(0, 0, null);
      this.host.chip(0, 0, null);
      return;
    }
    this.host.ring(at[0], at[1], this.brush() ? s.radius : null);
    if (s.mode === 'plateau') this.levelChip(at[0], at[1]);
    else if (s.mode === 'select') this.selectChip(at[0], at[1]);
    else this.host.chip(0, 0, null);
  }

  private selectChip(x: number, y: number) {
    this.under = this.plateauAt(x, y, 6 / this.host.ppf());
    const p = this.under ? this.host.edits().plateaus?.[this.under] : null;
    const r = 12 / this.host.ppf();
    this.host.chip(x + r, y + r, p ? `Level ${above(p.level, this.host.sea())}${this.under === this.host.selected() ? '' : ' · click to choose'}` : null);
  }

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0 || !this.host.battlemap()) return false;
    const s = this.host.settings();
    this.at = [x, y];
    // (Choose picks by a tap: a drag pans the map.)
    if (s.mode === 'select') return false;
    if (s.mode === 'plateau') {
      const l = this.levelAt(x, y);
      if (!l) {
        this.host.hint('Wait for the battlemap to load here');
        return true;
      }
      if (!l.on && Object.keys(this.host.edits().plateaus ?? {}).length >= MAX_PLATEAUS) {
        this.host.hint(`This world has as many plateaus as it can hold (${MAX_PLATEAUS})`);
        return true;
      }
      this.loop = { pts: [[x, y]], ...l };
      this.levelChip(x, y);
      return true;
    }
    this.touch = new Map();
    this.blocks.clear();
    this.full = false;
    this.last = [x, y];
    this.dab(x, y, x, y);
    return true;
  }

  move(x: number, y: number, e: PointerEvent) {
    this.hover(x, y);
    if (this.loop) {
      const q = this.loop.pts[this.loop.pts.length - 1];
      if (Math.hypot(x - q[0], y - q[1]) >= 2.5) this.loop.pts.push([x, y]);
      return;
    }
    if (this.touch && this.last) {
      const [lx, ly] = this.last;
      // (A segment at a time, so a fast stroke leaves no gaps.)
      if (Math.hypot(x - lx, y - ly) >= 1) {
        this.dab(lx, ly, x, y);
        this.last = [x, y];
      }
    }
    void e;
  }

  hover(x: number, y: number) {
    this.at = [x, y];
    this.refresh();
  }

  leave(e?: PointerEvent) {
    if (e && e.pointerType !== 'mouse') return;
    this.at = null;
    this.under = null;
    this.refresh();
  }

  up(x: number, y: number, e?: PointerEvent) {
    // (A finger lifted: no ring or chip left behind.)
    if (e && e.pointerType !== 'mouse') {
      this.at = null;
      this.refresh();
    }
    const s = this.host.settings();
    if (this.loop) {
      const loop = this.loop;
      this.loop = null;
      if (loop.pts.length < 3) return this.host.hint('Drag a loop round the ground to raise (or, below it, to dig)');
      const p = plateauFor(loop.pts, loop.level, s.edge);
      if (typeof p === 'string') return this.host.hint(p);
      const id = newObjectId('h');
      this.previewPlateau(p);
      this.host.commit({}, { [id]: p }, { tool: 'set_plateau' });
      this.host.select(id);
      return;
    }
    if (this.touch) {
      const touch = this.touch;
      this.touch = null;
      this.last = null;
      this.finish(touch, s);
    }
  }

  /** Choose: a click or tap picks the plateau there (or lets the chosen one go). */
  tap(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0 || this.host.settings().mode !== 'select' || !this.host.battlemap()) return false;
    const id = this.plateauAt(x, y, Math.max(6, e.pointerType === 'mouse' ? 0 : 16) / this.host.ppf());
    this.host.select(id);
    if (!id) this.host.hint('Click a plateau or pit to change it');
    this.refresh();
    return true;
  }

  cancel() {
    this.touch = null;
    this.loop = null;
    this.last = null;
    if (this.previewing) {
      this.previewing = false;
      this.host.unpreview();
    }
  }

  /** Escape: a stroke or loop under way is dropped, then the chosen plateau let go. */
  escape(): boolean {
    if (this.touch || this.loop) {
      this.cancel();
      return true;
    }
    if (this.host.selected()) {
      this.host.select(null);
      return true;
    }
    return false;
  }

  /** The tool is put away: nothing of it stays on the map (a preview stays until its chunks
   * come back). */
  dispose() {
    this.touch = null;
    this.loop = null;
    this.at = null;
    this.host.ring(0, 0, null);
    this.host.chip(0, 0, null);
  }

  /** `[` and `]`: the brush narrower or wider. */
  size(d: 1 | -1) {
    const s = this.host.settings();
    s.radius = Math.max(5, Math.min(60, s.radius + d * 2.5));
    this.refresh();
  }

  /** Touch every square the brush passes over from (ax, ay) to (bx, by), and show them. */
  private dab(ax: number, ay: number, bx: number, by: number) {
    const s = this.host.settings();
    const touch = this.touch!;
    const r = s.radius;
    const inner = r * Math.max(0, Math.min(1, s.hardness));
    const sq = (v: number) => Math.floor(v / SQUARE_FT);
    const [gx0, gy0, gx1, gy1] = [Math.max(0, sq(Math.min(ax, bx) - r)), Math.max(0, sq(Math.min(ay, by) - r)), sq(Math.max(ax, bx) + r), sq(Math.max(ay, by) + r)];
    let changed = false;
    for (let gy = gy0; gy <= gy1; gy++) {
      for (let gx = gx0; gx <= gx1; gx++) {
        const d = segDist((gx + 0.5) * SQUARE_FT, (gy + 0.5) * SQUARE_FT, ax, ay, bx, by);
        if (d >= r) continue;
        const f = d <= inner ? 1 : 1 - smoothstep(inner, r, d);
        const k = gx * BIG + gy;
        if (f <= 0 || f <= (touch.get(k) ?? 0)) continue;
        const key = blockKey(Math.floor(gx / BLOCK), Math.floor(gy / BLOCK));
        if (!this.blocks.has(key)) {
          if (this.blocks.size >= MAX_STROKE_BLOCKS) {
            if (!this.full) this.host.hint('A stroke reaches so far: let go and brush on with another');
            this.full = true;
            continue;
          }
          this.blocks.add(key);
        }
        touch.set(k, f);
        changed = true;
      }
    }
    if (!changed) return;
    const e = this.host.edits();
    const brush = this.brush()!;
    this.previewing = true;
    this.host.preview([gx0, gy0, gx1, gy1], (gx, gy, h, water) => {
      const f = touch.get(gx * BIG + gy);
      if (f === undefined) return null;
      const d = added(e.ground, gx, gy);
      const next = this.value(brush, s, gx, gy, f, d);
      if (next === null) return null;
      const out = h + (next - d);
      // Ground that is dry stays above the water by it.
      return h >= water ? Math.max(out, Math.min(h, water + 0.5)) : out;
    });
  }

  /** A square's new brushed change (ft, in tenths) from its touch `f` and change `d`; null where
   * its neighbours aren't loaded (Smooth). */
  private value(brush: Brush, s: GroundSettings, gx: number, gy: number, f: number, d: number): number | null {
    let v: number;
    if (brush === 'raise') v = d + s.height * f;
    else if (brush === 'lower') v = d - s.height * f;
    else if (brush === 'restore') v = d * (1 - s.strength * f);
    else {
      const h = this.host.ground(gx, gy)?.[0];
      if (h === undefined) return null;
      let [sum, n] = [0, 0];
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const g = this.host.ground(gx + dx, gy + dy);
          if (g) [sum, n] = [sum + g[0], n + 1];
        }
      }
      v = d + s.strength * f * (sum / n - h);
    }
    return Math.max(-MAX_ADD, Math.min(MAX_ADD, Math.round(v * 10))) / 10;
  }

  /** The stroke's blocks written anew (blocks that come out all zero removed), as one change. */
  private finish(touch: Map<number, number>, s: GroundSettings) {
    const e = this.host.edits();
    const ground = e.ground ?? {};
    const brush = this.brush();
    if (!brush || !touch.size) return this.unpreviewIfIdle();
    const next = new Map<string, Int16Array>();
    for (const [k, f] of touch) {
      const [gx, gy] = [Math.floor(k / BIG), k % BIG];
      const key = blockKey(Math.floor(gx / BLOCK), Math.floor(gy / BLOCK));
      let a = next.get(key);
      if (!a) {
        const raw = ground[key];
        a = raw ? (values(raw)?.slice() ?? new Int16Array(BLOCK * BLOCK)) : new Int16Array(BLOCK * BLOCK);
        next.set(key, a);
      }
      const v = this.value(brush, s, gx, gy, f, added(ground, gx, gy));
      if (v !== null) a[(gy % BLOCK) * BLOCK + (gx % BLOCK)] = Math.round(v * 10);
    }
    const out: Record<string, string | null> = {};
    let grows = 0;
    for (const [key, a] of next) {
      const zero = a.every((v) => v === 0);
      if (zero) {
        if (ground[key] !== undefined) out[key] = null;
        continue;
      }
      const raw = encode(a);
      if (raw === ground[key]) continue;
      if (ground[key] === undefined) grows++;
      out[key] = raw;
    }
    if (Object.keys(ground).length + grows > MAX_BLOCKS) {
      this.cancel();
      return this.host.hint('This world has as much brushed ground as it can hold: smooth or put back some first');
    }
    if (!Object.keys(out).length) return this.unpreviewIfIdle();
    this.host.commit(out, {}, { tool: 'shape_ground', mode: brush, count: Object.keys(out).length });
  }

  private unpreviewIfIdle() {
    if (this.previewing) {
      this.previewing = false;
      this.host.unpreview();
    }
  }

  /** Show a new plateau's ground at once (until its chunks come back). */
  private previewPlateau(p: Plateau) {
    const pts = p.pts as [number, number][];
    const band = Math.max(p.edge ?? 0, SQUARE_FT);
    const [x0, y0, x1, y1] = bbox(pts);
    const sq = (v: number) => Math.floor(v / SQUARE_FT);
    this.previewing = true;
    this.host.preview([Math.max(0, sq(x0 - band)), Math.max(0, sq(y0 - band)), sq(x1 + band), sq(y1 + band)], (gx, gy, h, water) => {
      const [x, y] = [(gx + 0.5) * SQUARE_FT, (gy + 0.5) * SQUARE_FT];
      let w = 1;
      if (!contains(pts, x, y)) {
        const d = outlineDist(pts, x, y);
        if (d >= band) return null;
        w = 1 - smoothstep(0, band, d);
      }
      const out = h + w * (p.level - h);
      return h >= water ? Math.max(out, Math.min(h, water + 0.5)) : out;
    });
  }

  /** Plateaus' outlines (the chosen one bold, the one a click would choose lit), and the loop
   * being drawn. */
  draw(g: Graphics, cam: Camera) {
    if (!this.host.battlemap()) return;
    const plateaus = this.host.edits().plateaus ?? {};
    const sel = this.host.selected();
    const [vx0, vy0] = cam.screenToWorld(0, 0);
    const [vx1, vy1] = cam.screenToWorld(cam.width, cam.height);
    const line = (pts: [number, number][], closed: boolean, width: number, color: number, alpha: number) => {
      pts.forEach(([x, y], i) => {
        const [sx, sy] = cam.worldToScreen(x, y);
        if (i) g.lineTo(sx, sy);
        else g.moveTo(sx, sy);
      });
      if (closed) g.closePath();
      g.stroke({ width, color, alpha });
    };
    for (const [id, p] of Object.entries(plateaus)) {
      const pts = p.pts as [number, number][];
      const [x0, y0, x1, y1] = bbox(pts);
      if (x1 < vx0 || x0 > vx1 || y1 < vy0 || y0 > vy1) continue;
      const on = id === sel;
      const lit = !on && id === this.under && this.host.settings().mode === 'select';
      line(pts, true, on ? 4 : 3, 0x1c1917, on ? 0.55 : 0.35);
      line(pts, true, on ? 2 : 1.2, on || lit ? 0xfde047 : 0xf5efe0, on ? 0.95 : lit ? 0.9 : 0.6);
    }
    if (this.loop && this.loop.pts.length > 1) {
      const pts = this.at ? [...this.loop.pts, this.at] : this.loop.pts;
      line(pts, false, 3.5, 0x1c1917, 0.45);
      line(pts, false, 1.6, 0xfde047, 0.95);
      // (Where it will close.)
      line([pts[pts.length - 1], pts[0]], false, 1, 0xfde047, 0.5);
    }
  }

  /** A plateau's level or edge changed (Select), as one change. */
  change(id: string, p: Plateau) {
    const problem = plateauProblem(p);
    if (problem) return this.host.hint(problem);
    this.host.commit({}, { [id]: p }, { tool: 'set_plateau', changed: true });
  }

  /** The chosen plateau taken away. */
  remove(): boolean {
    const id = this.host.selected();
    if (!id || !this.host.edits().plateaus?.[id]) return false;
    this.host.select(null);
    this.host.commit({}, { [id]: null }, { tool: 'delete_plateau' });
    return true;
  }
}
