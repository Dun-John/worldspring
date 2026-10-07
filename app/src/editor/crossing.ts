// Build › Crossing: put a bridge, ford or ferry down by clicking one bank, then the other (the
// crossing follows the pointer between the clicks, as wide as it will be). A click on a
// crossing put down earlier picks it (to change or take away) when nothing is being placed.
// Escape drops the first click. Right-drag pans meanwhile.
import type { Graphics } from 'pixi.js';
import { CROSSING_LENGTH, CROSSING_WIDTH, FERRY_MIN_FT, type Crossing, type CrossingKind, type Edits } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';

export type Pt = [number, number];

/** The crossing menu's choices. */
export interface CrossSettings {
  kind: CrossingKind;
  /** ft. */
  width: number;
}

export const defaultCross = (): CrossSettings => ({ kind: 'bridge', width: 12 });

/** A ferry's jetties (ft), out from each end (`battlemap::JETTY_FT`, `JETTY_WIDTH_FT`). */
const JETTY_FT = 24;
const JETTY_WIDTH_FT = 9;

/** Why a crossing can't be put down (mirrors `Crossing::problem`), if so. */
export function crossingProblem(c: Crossing): string | null {
  const len = Math.hypot(c.b[0] - c.a[0], c.b[1] - c.a[1]);
  const min = c.kind === 'ferry' ? FERRY_MIN_FT : CROSSING_LENGTH[0];
  if (len < min) return `A ${c.kind} is at least ${min} ft long: click farther apart`;
  if (len > CROSSING_LENGTH[1]) return `A ${c.kind} is at most ${CROSSING_LENGTH[1]} ft long`;
  if (!(c.width >= CROSSING_WIDTH[0] && c.width <= CROSSING_WIDTH[1])) return `Width: ${CROSSING_WIDTH[0]} to ${CROSSING_WIDTH[1]} ft`;
  return null;
}

/** Distance (ft) from p to the segment a–b. */
function segDist(p: Pt, a: Pt, b: Pt): number {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const t = Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy || 1)));
  return Math.hypot(p[0] - a[0] - dx * t, p[1] - a[1] - dy * t);
}

/** What the tool needs from the app. */
export interface CrossHost {
  settings(): CrossSettings;
  edits(): Edits;
  /** The crossing picked (being changed), if any. */
  picked(): string | null;
  /** Both ends clicked (world ft). */
  placed(a: Pt, b: Pt): void;
  /** A crossing put down earlier was clicked. */
  pick(id: string): void;
  hint(text: string): void;
}

export class CrossingTool implements PointerTool {
  private first: Pt | null = null;
  private cursor: Pt | null = null;

  constructor(private readonly host: CrossHost) {}

  down(_x: number, _y: number, e: PointerEvent): boolean {
    return e.button === 0;
  }

  move(x: number, y: number) {
    this.cursor = [x, y];
  }

  up(x: number, y: number) {
    const p: Pt = [Math.round(x * 10) / 10, Math.round(y * 10) / 10];
    if (!this.first) {
      // On a crossing put down earlier (and not moving the one picked): pick it.
      const hit = this.host.picked() ? null : this.at(p);
      if (hit) return this.host.pick(hit);
      this.first = p;
      return this.host.hint('Now click the other bank');
    }
    const a = this.first;
    this.first = null;
    this.host.placed(a, p);
  }

  /** The crossing put down earlier under p, if any. */
  private at(p: Pt): string | null {
    let best: [string, number] | null = null;
    for (const [id, c] of Object.entries(this.host.edits().crossings ?? {})) {
      const d = segDist(p, c.a, c.b) - c.width / 2;
      if (d < 4 && (!best || d < best[1])) best = [id, d];
    }
    return best?.[0] ?? null;
  }

  /** Drop the first click. True if there was one. */
  reset(): boolean {
    const had = !!this.first;
    this.first = null;
    return had;
  }

  cancel() {}

  hover(x: number, y: number) {
    this.cursor = [x, y];
  }

  draw(g: Graphics, cam: Camera) {
    const ink = 0xf97316;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    // Crossings put down earlier: outlined, the picked one filled.
    const picked = this.host.picked();
    for (const [id, c] of Object.entries(this.host.edits().crossings ?? {})) {
      const on = id === picked;
      outline(g, c.a, c.b, c.width, scr);
      g.fill({ color: ink, alpha: on ? 0.25 : 0.06 }).stroke({ width: on ? 2 : 1, color: ink, alpha: on ? 1 : 0.6 });
    }
    if (this.cursor && !this.first) {
      const [sx, sy] = scr(this.cursor);
      g.circle(sx, sy, 3).fill({ color: ink });
    }
    if (!this.first || !this.cursor) return;
    const { kind, width } = this.host.settings();
    const [a, b] = [this.first, this.cursor];
    const ok = !crossingProblem({ kind, a, b, width });
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (kind === 'ferry' && len > 1) {
      // Its jetties out from each end, the rope between.
      const u: Pt = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
      const ja: Pt = [a[0] + u[0] * Math.min(JETTY_FT, len / 2), a[1] + u[1] * Math.min(JETTY_FT, len / 2)];
      const jb: Pt = [b[0] - u[0] * Math.min(JETTY_FT, len / 2), b[1] - u[1] * Math.min(JETTY_FT, len / 2)];
      for (const [p, q] of [
        [a, ja],
        [jb, b],
      ] as [Pt, Pt][]) {
        outline(g, p, q, JETTY_WIDTH_FT, scr);
        g.fill({ color: ink, alpha: 0.22 }).stroke({ width: 2, color: ink });
      }
      const [s0, s1] = [scr(ja), scr(jb)];
      g.moveTo(s0[0], s0[1]).lineTo(s1[0], s1[1]).stroke({ width: 2, color: ink, alpha: ok ? 1 : 0.4 });
    } else {
      outline(g, a, b, width, scr);
      g.fill({ color: ink, alpha: ok ? 0.22 : 0.08 }).stroke({ width: 2, color: ink, alpha: ok ? 1 : 0.4 });
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
