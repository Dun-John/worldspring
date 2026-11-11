// Build › Harbour: rock islets (a click), and quays, piers and breakwaters (a drag: a pier or
// breakwater from the bank out over the water, a quay along the shore). The map shows what
// the drag will be (as wide as it will be); the generator has the last word on release
// (`Wharf::problem`, `wharf::root_problem`, through the app's `wharfcheck` op). A click on one put
// down earlier (when nothing is being drawn) picks it, to take it away. Right-drag pans meanwhile.
import type { Graphics } from 'pixi.js';
import { WHARF_WIDTH, type Edits, type Wharf, type WharfKind } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';

export type Pt = [number, number];
export type HarbourKind = 'islet' | WharfKind;

/** The Harbour strip's choices. */
export interface HarbourSettings {
  kind: HarbourKind;
  /** An islet's radius (ft). */
  radius: number;
  /** An islet's top over the water (ft). */
  rise: number;
  /** Per wharf kind (ft); a new kind gets its default. */
  width: number;
  /** A breakwater's lighthouse at its tip. */
  lighthouse: boolean;
  /** A pier's material. */
  material: 'timber' | 'stone';
}

export const defaultHarbour = (): HarbourSettings => ({ kind: 'pier', radius: 30, rise: 5, width: WHARF_WIDTH.pier[2], lighthouse: false, material: 'timber' });

/** The islet radius and rise ranges (`raise_islet`). */
export const ISLET_RADIUS: [number, number] = [10, 200];
export const ISLET_RISE: [number, number] = [1, 30];

/** Each side of a wharf at least this long, all of it at most `MAX_LENGTH_FT` (`Wharf`). */
const MIN_SIDE_FT = 10;
const MAX_LENGTH_FT = 2000;

/** Why a wharf can't be put down (mirrors `Wharf::problem`'s length and width checks), if so. */
export function wharfProblem(w: Wharf): string | null {
  const len = Math.hypot(w.pts[1][0] - w.pts[0][0], w.pts[1][1] - w.pts[0][1]);
  if (len < MIN_SIDE_FT) return `A ${w.kind} is at least ${MIN_SIDE_FT} ft long: drag farther`;
  if (len > MAX_LENGTH_FT) return `A ${w.kind} is at most ${MAX_LENGTH_FT} ft long`;
  const [lo, hi] = WHARF_WIDTH[w.kind];
  const width = w.width ?? WHARF_WIDTH[w.kind][2];
  if (!(width >= lo && width <= hi)) return `A ${w.kind}'s width: ${lo} to ${hi} ft`;
  return null;
}

/** A wharf from the strip's choices (a breakwater is stone; a pier's stone says so). */
export function wharfOf(s: HarbourSettings, a: Pt, b: Pt): Wharf {
  const kind = s.kind as WharfKind;
  return { kind, pts: [a, b], width: s.width, ...(kind === 'pier' && s.material === 'stone' ? { material: 'stone' as const } : {}) };
}

/** What the tool needs from the app. */
export interface HarbourHost {
  settings(): HarbourSettings;
  edits(): Edits;
  /** The wharf picked (its id), if any. */
  picked(): string | null;
  /** An islet clicked (world ft). */
  islet(at: Pt): void;
  /** A wharf dragged (both ends, world ft). */
  wharf(a: Pt, b: Pt): void;
  /** A wharf put down earlier was clicked. */
  pick(id: string): void;
  hint(text: string): void;
}

function segDist(p: Pt, a: Pt, b: Pt): number {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const t = Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy || 1)));
  return Math.hypot(p[0] - a[0] - dx * t, p[1] - a[1] - dy * t);
}

export class HarbourTool implements PointerTool {
  private from: Pt | null = null;
  private cursor: Pt | null = null;
  readonly hoverFollows = true;

  constructor(private readonly host: HarbourHost) {}

  private snap(x: number, y: number): Pt {
    return [Math.round(x * 2) / 2, Math.round(y * 2) / 2];
  }

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    this.cursor = this.snap(x, y);
    if (this.host.settings().kind !== 'islet' && !this.at(this.cursor)) this.from = this.cursor;
    else this.from = null;
    return true;
  }

  move(x: number, y: number) {
    this.cursor = this.snap(x, y);
  }

  hover(x: number, y: number) {
    this.cursor = this.snap(x, y);
  }

  up(x: number, y: number) {
    const p = this.snap(x, y);
    const s = this.host.settings();
    const a = this.from;
    this.from = null;
    if (s.kind === 'islet') return this.host.islet(p);
    if (!a) {
      const hit = this.at(p);
      if (hit) return this.host.pick(hit);
      return;
    }
    if (Math.hypot(p[0] - a[0], p[1] - a[1]) < 3) return this.host.hint(s.kind === 'quay' ? 'Drag along the shore' : `Drag from the bank out over the water`);
    this.host.wharf(a, p);
  }

  /** The wharf put down earlier under p, if any. */
  private at(p: Pt): string | null {
    let best: [string, number] | null = null;
    for (const [id, w] of Object.entries(this.host.edits().wharves ?? {})) {
      let d = Infinity;
      for (let i = 0; i + 1 < w.pts.length; i++) d = Math.min(d, segDist(p, w.pts[i], w.pts[i + 1]));
      d -= (w.width ?? 20) / 2;
      if (d < 4 && (!best || d < best[1])) best = [id, d];
    }
    return best?.[0] ?? null;
  }

  /** Drop a drag under way. True if there was one. */
  reset(): boolean {
    const had = !!this.from;
    this.from = null;
    return had;
  }

  cancel() {
    this.from = null;
  }

  draw(g: Graphics, cam: Camera) {
    const ink = 0x0ea5e9;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const picked = this.host.picked();
    for (const [id, w] of Object.entries(this.host.edits().wharves ?? {})) {
      const on = id === picked;
      for (let i = 0; i + 1 < w.pts.length; i++) {
        outline(g, w.pts[i], w.pts[i + 1], w.width ?? 20, scr);
        g.fill({ color: ink, alpha: on ? 0.25 : 0.06 }).stroke({ width: on ? 2 : 1, color: ink, alpha: on ? 1 : 0.6 });
      }
    }
    if (!this.cursor) return;
    const s = this.host.settings();
    if (s.kind === 'islet') {
      // The ring an islet would stand in (its true outline is rough; this is its radius).
      const [sx, sy] = scr(this.cursor);
      const r = Math.abs(scr([this.cursor[0] + s.radius, this.cursor[1]])[0] - sx);
      g.circle(sx, sy, r).fill({ color: ink, alpha: 0.18 }).stroke({ width: 2, color: ink });
      g.circle(sx, sy, 2.5).fill({ color: ink });
      return;
    }
    if (!this.from) {
      const [sx, sy] = scr(this.cursor);
      g.circle(sx, sy, 3).fill({ color: ink });
      return;
    }
    const [a, b] = [this.from, this.cursor];
    const ok = !wharfProblem({ kind: s.kind, pts: [a, b], width: s.width });
    outline(g, a, b, s.width, scr);
    g.fill({ color: ink, alpha: ok ? 0.22 : 0.08 }).stroke({ width: 2, color: ink, alpha: ok ? 1 : 0.4 });
    if (s.kind === 'breakwater' && s.lighthouse) {
      const [sx, sy] = scr(b);
      g.circle(sx, sy, Math.max(3, Math.abs(scr([b[0] + 9, b[1]])[0] - sx))).stroke({ width: 2, color: 0xfacc15 });
    }
    for (const p of [a, b]) {
      const [sx, sy] = scr(p);
      g.rect(sx - 3, sy - 3, 6, 6).fill({ color: 0xffffff }).stroke({ width: 1.5, color: ink });
    }
  }
}

/** A rectangle from a to b, `width` ft across, as a path (screen px). */
function outline(g: Graphics, a: Pt, b: Pt, width: number, scr: (p: Pt) => number[]) {
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
  const [vx, vy] = [(-(b[1] - a[1]) / len) * width * 0.5, ((b[0] - a[0]) / len) * width * 0.5];
  const pts: Pt[] = [
    [a[0] + vx, a[1] + vy],
    [b[0] + vx, b[1] + vy],
    [b[0] - vx, b[1] - vy],
    [a[0] - vx, a[1] - vy],
  ];
  g.poly(pts.flatMap((p) => scr(p)));
}
