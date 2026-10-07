// Scatter: put battlemap objects down by hand (a stamp, or a brush that scatters the chosen
// kinds), and take them away (a click, or an eraser brush). Objects put down become
// `Edits.objects` entries; generated ones taken away, `Edits.cleared` entries. A brush or eraser
// stroke is one change (one undo step), made when the stroke ends; the objects a brush puts down
// show at once (the map's preview) while their chunks are made again.
import type { Clear, Edits, Placed } from '../gen/protocol';
import type { KindInfo } from '../gen/client';
import type { PointerTool } from '../render/MapView';

export type ScatterMode = 'stamp' | 'brush' | 'erase';

export interface ScatterSettings {
  mode: ScatterMode;
  /** The kinds to put down: built-in ids or `s:<asset>` (a stamp uses the first). */
  kinds: (number | string)[];
  /** Degrees, or null for each at random. */
  rotation: number | null;
  scale: number;
  /** Vary each object's size a little. */
  vary: boolean;
  /** Brush and eraser radius (ft). */
  radius: number;
  /** Brush: feet between objects. */
  spacing: number;
  /** Eraser: only the chosen kinds. */
  onlyChosen: boolean;
}

export const defaultScatter = (): ScatterSettings => ({ mode: 'stamp', kinds: [], rotation: null, scale: 1, vary: true, radius: 15, spacing: 10, onlyChosen: false });

/** What the tool needs from the app. */
export interface ScatterHost {
  settings(): ScatterSettings;
  edits(): Edits;
  catalog(): KindInfo[];
  /** The object under a point on the battlemap, if one is loaded there. */
  objectAt(x: number, y: number): { kind: number | string; x: number; y: number } | null;
  /** Draw objects at once, before the change is made. */
  preview(objs: Placed[]): void;
  /** Make the change (one undo step). */
  commit(objects: Record<string, Placed | null>, cleared: Record<string, Clear>, label: { tool: string; count: number; name?: string }): void;
  /** The brush ring under the pointer (world ft), or none. */
  ring(x: number, y: number, r: number | null): void;
  /** Whether the battlemap is showing (objects can only be seen and picked there). */
  battlemap(): boolean;
  hint(text: string): void;
}

export function newObjectId(prefix: 'o' | 'x' | 'v'): string {
  const b = new Uint8Array(8);
  crypto.getRandomValues(b);
  return `${prefix}:${[...b].map((x) => '0123456789abcdefghijklmnopqrstuvwxyz'[x % 36]).join('')}`;
}

/** A placed object's footprint radius (ft). */
function placedRadius(o: Placed, e: Edits, catalog: KindInfo[]): number {
  if (typeof o.kind === 'string') return ((e.sprites?.[o.kind.slice(2)]?.size ?? 1) / 2) * 5 * o.scale;
  return (catalog[o.kind - 1]?.radius ?? 0.5) * 5 * o.scale;
}

/** A small deterministic hash of a grid cell (for the brush's jittered lattice). */
function cellHash(i: number, j: number, salt: number): number {
  let h = Math.imul(i, 0x27d4eb2d) ^ Math.imul(j, 0x165667b1) ^ salt;
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b);
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
  return (h ^ (h >>> 16)) >>> 0;
}

export class ScatterTool implements PointerTool {
  /** A brush stroke's objects so far, by id. */
  private stroke: Record<string, Placed> | null = null;
  /** An eraser stroke's circles. */
  private wipes: { x: number; y: number }[] | null = null;
  private filled = new Set<string>();
  private salt = 0;
  private last: [number, number] | null = null;

  constructor(private readonly host: ScatterHost) {}

  private make(kind: number | string, x: number, y: number): Placed {
    const s = this.host.settings();
    const rot = s.rotation === null ? Math.random() * Math.PI * 2 : (s.rotation * Math.PI) / 180;
    const scale = s.scale * (s.vary ? 0.85 + Math.random() * 0.3 : 1);
    return { kind, x, y, rot, scale: Math.round(scale * 1000) / 1000, variant: Math.floor(Math.random() * 8) };
  }

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    const s = this.host.settings();
    // Zoomed out, the map pans as usual.
    if (!this.host.battlemap()) return false;
    if (s.mode === 'brush') {
      if (!s.kinds.length) {
        this.host.hint('Choose what to scatter first');
        return true;
      }
      this.stroke = {};
      this.filled.clear();
      this.salt = (Math.random() * 0xffffffff) >>> 0;
      this.dab(x, y);
    } else if (s.mode === 'erase') {
      this.wipes = [{ x, y }];
      this.last = [x, y];
    }
    return true;
  }

  move(x: number, y: number) {
    const s = this.host.settings();
    this.host.ring(x, y, s.mode === 'stamp' ? null : s.radius);
    if (this.stroke) this.dab(x, y);
    if (this.wipes && this.last && Math.hypot(x - this.last[0], y - this.last[1]) >= s.radius * 0.6) {
      this.wipes.push({ x, y });
      this.last = [x, y];
    }
  }

  hover(x: number, y: number) {
    const s = this.host.settings();
    this.host.ring(x, y, s.mode === 'stamp' ? null : s.radius);
  }

  up(x: number, y: number) {
    const s = this.host.settings();
    if (this.stroke) {
      const objs = this.stroke;
      this.stroke = null;
      const n = Object.keys(objs).length;
      if (n) this.host.commit(objs, {}, { tool: 'place_objects', count: n });
      return;
    }
    if (this.wipes) {
      const wipes = this.wipes;
      this.wipes = null;
      if (Math.hypot(x - wipes[0].x, y - wipes[0].y) < 2 && wipes.length === 1) this.erasePoint(x, y);
      else this.eraseCircles(wipes, s);
      return;
    }
    if (s.mode === 'stamp' && this.host.battlemap()) {
      const kind = s.kinds[0];
      if (kind === undefined) return this.host.hint('Choose what to put down first');
      this.host.commit({ [newObjectId('o')]: this.make(kind, x, y) }, {}, { tool: 'place_objects', count: 1 });
    }
  }

  cancel() {
    this.stroke = null;
    this.wipes = null;
    this.host.ring(0, 0, null);
  }

  /** Brush: objects on a jittered lattice (spacing apart) within the radius, cells not yet
   * filled in this stroke and clear of objects put down before. */
  private dab(x: number, y: number) {
    const s = this.host.settings();
    const stroke = this.stroke!;
    const sp = Math.max(2.5, s.spacing);
    const r = s.radius;
    const placed = Object.values(this.host.edits().objects ?? {});
    const add: Placed[] = [];
    for (let j = Math.floor((y - r) / sp); j <= Math.floor((y + r) / sp); j++) {
      for (let i = Math.floor((x - r) / sp); i <= Math.floor((x + r) / sp); i++) {
        const key = `${i},${j}`;
        if (this.filled.has(key)) continue;
        const h = cellHash(i, j, this.salt);
        const px = (i + 0.15 + 0.7 * ((h & 0xffff) / 0xffff)) * sp;
        const py = (j + 0.15 + 0.7 * ((h >>> 16) / 0xffff)) * sp;
        if (Math.hypot(px - x, py - y) > r) continue;
        this.filled.add(key);
        if (placed.some((o) => Math.hypot(o.x - px, o.y - py) < sp * 0.6)) continue;
        const o = this.make(s.kinds[h % s.kinds.length], px, py);
        stroke[newObjectId('o')] = o;
        add.push(o);
      }
    }
    if (add.length) this.host.preview(add);
  }

  /** A click: the object put down there goes, else the generated one there is cleared. */
  private erasePoint(x: number, y: number) {
    const e = this.host.edits();
    const catalog = this.host.catalog();
    let best: [string, number] | null = null;
    for (const [id, o] of Object.entries(e.objects ?? {})) {
      const d = Math.hypot(o.x - x, o.y - y);
      if (d <= Math.max(2.5, placedRadius(o, e, catalog)) && (!best || d < best[1])) best = [id, d];
    }
    if (best) return this.host.commit({ [best[0]]: null }, {}, { tool: 'remove_objects', count: 1 });
    const hit = this.host.objectAt(x, y);
    if (!hit) return this.host.hint('Nothing there to take away');
    if (typeof hit.kind === 'string') return;
    this.host.commit({}, { [newObjectId('x')]: { x: hit.x, y: hit.y, kind: hit.kind } }, { tool: 'remove_objects', count: 1, name: catalog[hit.kind - 1]?.name });
  }

  /** An eraser stroke: objects put down inside go; a clear for each circle takes the
   * generated ones (of the chosen built-in kinds, if asked). */
  private eraseCircles(wipes: { x: number; y: number }[], s: ScatterSettings) {
    const e = this.host.edits();
    const builtin = s.kinds.filter((k): k is number => typeof k === 'number');
    const only = s.onlyChosen && s.kinds.length > 0;
    const objects: Record<string, null> = {};
    for (const [id, o] of Object.entries(e.objects ?? {})) {
      if (only && !s.kinds.includes(o.kind)) continue;
      if (wipes.some((w) => Math.hypot(o.x - w.x, o.y - w.y) <= s.radius)) objects[id] = null;
    }
    const cleared: Record<string, Clear> = {};
    // Uploaded sprites alone chosen: nothing generated is theirs.
    if (!only || builtin.length) {
      for (const w of wipes) cleared[newObjectId('x')] = { x: Math.round(w.x * 100) / 100, y: Math.round(w.y * 100) / 100, r: s.radius, ...(only ? { kinds: builtin } : {}) };
    }
    this.host.commit(objects, cleared, { tool: 'remove_objects', count: Object.keys(objects).length });
  }
}
