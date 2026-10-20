// Placing things by hand, the same way in Edit › Objects (the battlemap) and Edit › Rooms
// (inside): what is in the hand, the keys (and on touch screens the place bar's buttons) that act
// on it, turning by Shift+wheel, and an index of the objects put down by hand for finding the one
// under the pointer.
//
// With a mouse the ghost follows the pointer and a click puts it down. On a touch screen a tap
// moves the ghost (or picks up the piece tapped), a drag from the ghost moves it, any other drag
// pans, and the bar beside the ghost turns it, puts it down (✓, the ghost stays for the next) or
// puts it away (✕).
import type { Placed } from '../gen/protocol';

/** What is in the hand: nothing (a click picks up what it is on), an item chosen (its ghost
 * follows the pointer; a click puts one down), or a piece picked up (a click puts it down). */
export type HandState = 'empty' | 'chosen' | 'holding';

/** What the keys do to what the hand shows: the piece held, the piece a click would pick up
 * (the key picks it up first), or the chosen item's ghost. Each says whether it did anything. */
export interface Placer {
  hand(): HandState;
  /** T / Shift+T (`fine` false), `.` / `,` (true): clockwise for 1. */
  turn(dir: 1 | -1, fine: boolean): boolean;
  /** Z: back to the turn it takes by itself (at random, or facing its wall). */
  turnReset(): boolean;
  /** Enter: put the held piece down where it is. */
  putDown(): boolean;
  /** Delete: the held piece, or the one under the pointer (also with an item chosen), goes. */
  remove(): boolean;
  /** `]` / `[`: bigger (1) or smaller (-1). */
  size(d: 1 | -1): boolean;
  /** Escape: put the held piece back, else empty the hand. */
  escape(): boolean;
  /** ✓ on the place bar: the held piece, or one of the chosen item, put down where the ghost is. */
  confirm(): boolean;
}

/** Where the place bar goes (beside the ghost on a touch screen; world ft, `r` the ghost's
 * radius) and which buttons it shows: `remove` while a piece is held, `turns` for what shows a
 * turn. Null: no bar (a mouse, or nothing in the hand). */
export type BarAt = { x: number; y: number; r: number; holding: boolean; turns: boolean } | null;

/** A piece picked up, as the panel's "in hand" row shows it: its name, its turn ('45°'; '' where
 * none is shown), whether it shows a turn, whether its turn was given by hand (Auto shown), and its
 * size ('1.10×', '2×1'); inside also its kind and the look given it (null: its own). */
export type HeldView = { name: string; turn: string; turns: boolean; auto: boolean; size: string; kind?: string; variant?: number | null } | null;

/** A finger's reach on the map (screen px): a tap or drag this near the ghost or a piece is on it. */
export const FINGER_PX = 22;

/** Degrees in [0, 360), whole. */
export function degrees(d: number): number {
  return ((Math.round(d) % 360) + 360) % 360;
}

/** Shift+wheel as turning: one step per 100 px scrolled (lines count 40 px; Windows sends
 * Shift+wheel sideways). Pinch-zoom (Ctrl) is never taken. */
export class WheelTurn {
  private acc = 0;
  private last = 0;

  /** Steps to turn for this event (clockwise positive), or null if it isn't a turn. */
  steps(e: WheelEvent): number | null {
    if (!e.shiftKey || e.ctrlKey || e.metaKey) return null;
    const now = performance.now();
    // A pause starts a new gesture.
    if (now - this.last > 400) this.acc = 0;
    this.last = now;
    this.acc += (e.deltaY || e.deltaX) * (e.deltaMode === 1 ? 40 : 1);
    const n = Math.trunc(this.acc / 100);
    this.acc -= n * 100;
    return n;
  }
}

const CELL = 40;

/** The objects put down by hand (`Edits.objects`) in 40-ft buckets by centre, for the one
 * under the pointer and the brush's spacing. Kept for one `objects` record (edits are never
 * changed in place): a new one is indexed in idle time, meanwhile every object is looked at. */
export class PlacedIndex {
  private src: Record<string, Placed> | undefined;
  private cells = new Map<number, string[]>();
  private pending: Record<string, Placed> | undefined;
  /** The widest footprint indexed (ft). */
  private widest = 0;

  constructor(private readonly radius: (o: Placed) => number) {}

  /** Calls `f` for each object whose centre is within `r` of (x, y) (and maybe a few more). */
  around(objects: Record<string, Placed> | undefined, x: number, y: number, r: number, f: (id: string, o: Placed) => void) {
    if (!objects) return;
    if (objects !== this.src) {
      this.schedule(objects);
      for (const id in objects) f(id, objects[id]);
      return;
    }
    for (let j = Math.floor((y - r) / CELL); j <= Math.floor((y + r) / CELL); j++) {
      for (let i = Math.floor((x - r) / CELL); i <= Math.floor((x + r) / CELL); i++) {
        const ids = this.cells.get(key(i, j));
        if (ids) for (const id of ids) f(id, objects[id]);
      }
    }
  }

  /** The object whose footprint (at least `min` ft round) holds (x, y), the nearest centre. */
  at(objects: Record<string, Placed> | undefined, x: number, y: number, min: number): string | null {
    let best: string | null = null;
    let bestD = Infinity;
    const reach = Math.max(min, objects === this.src ? this.widest : 0);
    this.around(objects, x, y, reach, (id, o) => {
      const d = Math.hypot(o.x - x, o.y - y);
      if (d <= Math.max(min, this.radius(o)) && d < bestD) [best, bestD] = [id, d];
    });
    return best;
  }

  /** Whether any object's centre is within `r` of (x, y). */
  any(objects: Record<string, Placed> | undefined, x: number, y: number, r: number): boolean {
    let hit = false;
    this.around(objects, x, y, r, (_, o) => (hit ||= Math.hypot(o.x - x, o.y - y) < r));
    return hit;
  }

  private schedule(objects: Record<string, Placed>) {
    if (this.pending === objects) return;
    this.pending = objects;
    const run = () => {
      if (this.pending !== objects) return;
      this.pending = undefined;
      this.build(objects);
    };
    if ('requestIdleCallback' in window) requestIdleCallback(run, { timeout: 500 });
    else setTimeout(run, 0);
  }

  private build(objects: Record<string, Placed>) {
    const cells = new Map<number, string[]>();
    let widest = 0;
    for (const id in objects) {
      const o = objects[id];
      const k = key(Math.floor(o.x / CELL), Math.floor(o.y / CELL));
      const ids = cells.get(k);
      if (ids) ids.push(id);
      else cells.set(k, [id]);
      widest = Math.max(widest, this.radius(o));
    }
    [this.src, this.cells, this.widest] = [objects, cells, widest];
  }
}

/** A bucket's key (cells to ±2^20 either way: 40 million ft). */
const key = (i: number, j: number) => (i + 0x100000) * 0x200000 + (j + 0x100000);
