// Scatter: put battlemap objects down by hand (one at a time, or a brush that scatters the chosen
// kinds), and take them away (a click, or an eraser brush). Objects put down become
// `Edits.objects` entries; generated ones taken away, `Edits.cleared` entries. A brush or eraser
// stroke is one change (one undo step), made when the stroke ends; the objects a brush puts down
// show at once (the map's preview) while their chunks are made again.
//
// Place mode works as placing.ts describes: the chosen object's ghost follows the pointer, turned
// and sized as it will land; with nothing chosen a click picks up an object put down by hand
// (outlined under the pointer), which then follows the pointer until a click puts it down.
import type { Clear, Edits, Placed } from '../gen/protocol';
import type { KindInfo } from '../gen/client';
import type { Camera } from '../render/camera';
import type { Graphics } from 'pixi.js';
import { drawnTurned } from '../render/BattlemapLayer';
import type { PointerTool } from '../render/MapView';
import { degrees, PlacedIndex, WheelTurn, type HandState, type Placer } from './placing';

export type ScatterMode = 'place' | 'brush' | 'erase';

export interface ScatterSettings {
  mode: ScatterMode;
  /** The kinds to put down: built-in ids or `s:<asset>` (Place uses the first). */
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

export const defaultScatter = (): ScatterSettings => ({ mode: 'place', kinds: [], rotation: null, scale: 1, vary: true, radius: 15, spacing: 10, onlyChosen: false });

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
  commit(objects: Record<string, Placed | null>, cleared: Record<string, Clear>, label: { tool: string; count: number; name?: string; changed?: string[] }): void;
  /** The brush ring under the pointer (world ft), or none. */
  ring(x: number, y: number, r: number | null): void;
  /** What is in the hand drawn where it would land (red where it can't), or none. */
  ghost(o: Placed | null, ok: boolean): void;
  /** The chip by the ghost (world ft: the ghost's corner), or none. */
  chip(x: number, y: number, text: string | null): void;
  /** Whether the battlemap is showing (objects can only be seen and picked there). */
  battlemap(): boolean;
  /** Whether the battlemap is loaded at a point. */
  loaded(x: number, y: number): boolean;
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

/** A click finds an object put down by hand within this of its centre at least (ft). */
const PICK_FT = 2.5;
const rad = (deg: number) => (deg * Math.PI) / 180;
const deg = (r: number) => degrees((r * 180) / Math.PI);
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** The next object's own roll (kept until it is put down, so the ghost shows it): its turn if
 * at random, its look, and how much its size varies. */
interface Roll {
  rot: number;
  variant: number;
  vary: number;
}
const roll = (): Roll => ({ rot: Math.random() * Math.PI * 2, variant: Math.floor(Math.random() * 8), vary: 0.85 + Math.random() * 0.3 });

export class ScatterTool implements PointerTool, Placer {
  /** A brush stroke's objects so far, by id. */
  private stroke: Record<string, Placed> | null = null;
  /** An eraser stroke's circles. */
  private wipes: { x: number; y: number }[] | null = null;
  private filled = new Set<string>();
  private salt = 0;
  private last: [number, number] | null = null;
  /** The pointer (world ft), while a mouse is over the map. */
  private at: [number, number] | null = null;
  /** A piece picked up: its id, where it was, and how it is now (it follows the pointer). */
  private holding: { id: string; from: Placed; now: Placed } | null = null;
  /** With an empty hand, the object put down by hand under the pointer (and where it was looked for). */
  private under: string | null = null;
  /** Where (and in which `objects`) `under` was looked for: again once the pointer moves a foot,
   * the objects change or the hand empties. */
  private probed: { x: number; y: number; objects: Edits['objects'] } | null = null;
  private next = roll();
  private readonly wheelTurn = new WheelTurn();
  private readonly index = new PlacedIndex((o) => placedRadius(o, this.host.edits(), this.host.catalog()));
  /** The ghost and chip follow the pointer as the camera moves under it (MapView re-hovers). */
  readonly hoverFollows = true;

  constructor(private readonly host: ScatterHost) {}

  hand(): HandState {
    if (this.holding) return 'holding';
    const s = this.host.settings();
    return s.mode === 'place' && s.kinds.length ? 'chosen' : 'empty';
  }

  private kindInfo(kind: number | string): KindInfo | undefined {
    return typeof kind === 'number' ? this.host.catalog()[kind - 1] : undefined;
  }

  /** Whether a kind shows a turn given by hand (props approved for it, and uploaded pictures). */
  private turns(kind: number | string): boolean {
    return typeof kind === 'string' || drawnTurned(this.kindInfo(kind), true);
  }

  /** The turn an object shows (degrees). */
  private shown(o: Placed): number {
    return typeof o.kind === 'string' || drawnTurned(this.kindInfo(o.kind), !!o.turned) ? deg(o.rot) : 0;
  }

  private nameOf(kind: number | string): string {
    if (typeof kind === 'string') return this.host.edits().sprites?.[kind.slice(2)]?.name ?? 'Your sprite';
    return cap(this.kindInfo(kind)?.name ?? 'object');
  }

  /** The next object of a kind as it would be put down at (x, y): a fixed turn sets `turned`
   * (drawn turned where the kind can be); at random, as before (drawn turned only by kinds
   * that always are). */
  private make(kind: number | string, x: number, y: number, r = this.next): Placed {
    const s = this.host.settings();
    const scale = s.scale * (s.vary ? r.vary : 1);
    const o: Placed = { kind, x, y, rot: s.rotation === null ? r.rot : rad(s.rotation), scale: Math.round(scale * 1000) / 1000, variant: r.variant };
    if (s.rotation !== null) o.turned = true;
    return o;
  }

  /** The ghost, its chip and the outline as the hand and pointer now are. */
  refresh() {
    const s = this.host.settings();
    if (this.holding && s.mode !== 'place') this.holding = null;
    const at = this.at;
    if (!at || s.mode !== 'place' || !this.host.battlemap()) {
      this.host.ghost(null, true);
      this.host.chip(0, 0, null);
      this.under = null;
      this.probed = null;
      return;
    }
    const e = this.host.edits();
    const [x, y] = at;
    let o: Placed | null = null;
    if (this.holding) o = { ...this.holding.now, x, y };
    else if (s.kinds.length) o = this.make(s.kinds[0], x, y);
    this.host.ghost(o, this.host.loaded(x, y));
    if (o) {
      const r = placedRadius(o, e, this.host.catalog()) * 0.75;
      this.host.chip(x + r, y + r, `${this.nameOf(o.kind)}${this.turns(o.kind) ? ` · ${this.shown(o)}°` : ''}`);
      this.under = null;
      this.probed = null;
      return;
    }
    // An empty hand: the object put down by hand under the pointer (looked for again only once
    // the pointer has moved a foot).
    const p = this.probed;
    if (!p || p.objects !== e.objects || Math.hypot(p.x - x, p.y - y) >= 1) {
      this.probed = { x, y, objects: e.objects };
      this.under = this.index.at(e.objects, x, y, PICK_FT);
    }
    const u = this.under && e.objects?.[this.under];
    if (u) {
      const r = Math.max(PICK_FT, placedRadius(u, e, this.host.catalog())) * 0.75;
      this.host.chip(u.x + r, u.y + r, `${this.nameOf(u.kind)} · click to pick up`);
    } else this.host.chip(0, 0, null);
  }

  /** Takes the piece under the pointer into the hand (for a key that acts on it). */
  private pickUnder(): boolean {
    if (this.holding) return true;
    if (this.hand() !== 'empty' || this.host.settings().mode !== 'place') return false;
    const o = this.under && this.host.edits().objects?.[this.under];
    if (!o || !this.under) return false;
    this.holding = { id: this.under, from: o, now: { ...o } };
    this.under = null;
    this.probed = null;
    return true;
  }

  /** Puts the held piece down at (x, y) (the same id: a move). */
  private drop(x: number, y: number) {
    const h = this.holding!;
    this.holding = null;
    this.probed = null;
    // (Undone or taken away meanwhile: nothing to put down.)
    if (!this.host.edits().objects?.[h.id]) return;
    const o: Placed = { ...h.now, x: Math.round(x * 100) / 100, y: Math.round(y * 100) / 100 };
    this.host.commit({ [h.id]: o }, {}, { tool: 'place_objects', count: 1, changed: [h.id] });
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

  move(x: number, y: number, e: PointerEvent) {
    const s = this.host.settings();
    this.host.ring(x, y, s.mode === 'place' ? null : s.radius);
    if (this.stroke) this.dab(x, y);
    if (this.wipes && this.last && Math.hypot(x - this.last[0], y - this.last[1]) >= s.radius * 0.6) {
      this.wipes.push({ x, y });
      this.last = [x, y];
    }
    if (e.pointerType === 'mouse' && s.mode === 'place') {
      this.at = [x, y];
      this.refresh();
    }
  }

  hover(x: number, y: number) {
    const s = this.host.settings();
    this.host.ring(x, y, s.mode === 'place' ? null : s.radius);
    this.at = [x, y];
    if (s.mode === 'place' || this.holding) this.refresh();
  }

  leave() {
    this.at = null;
    this.host.ring(0, 0, null);
    this.refresh();
  }

  up(x: number, y: number, e: PointerEvent) {
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
    if (s.mode !== 'place' || !this.host.battlemap()) return;
    if (e.pointerType === 'mouse') this.at = [x, y];
    if (this.holding) this.drop(x, y);
    else if (s.kinds.length) {
      this.host.commit({ [newObjectId('o')]: this.make(s.kinds[0], x, y) }, {}, { tool: 'place_objects', count: 1 });
      this.next = roll();
    } else {
      this.probed = null;
      if (this.at) this.refresh();
      else this.under = this.index.at(this.host.edits().objects, x, y, PICK_FT);
      if (!this.pickUnder()) {
        const hit = this.host.objectAt(x, y);
        this.host.hint(hit ? 'Only objects put down by hand can be picked up (Erase takes this one away)' : 'Choose what to put down, or click one you put down to pick it up');
      }
    }
    this.refresh();
  }

  /** A click that didn't start the tool: a right click puts the held piece back. */
  tap(_x: number, _y: number, e: PointerEvent): boolean {
    if (e.button !== 2 || !this.holding) return false;
    this.holding = null;
    this.probed = null;
    this.refresh();
    return true;
  }

  /** Shift+wheel turns what is in the hand. */
  wheel(e: WheelEvent): boolean {
    if (!e.shiftKey || this.host.settings().mode !== 'place' || (!this.holding && this.hand() !== 'chosen')) return false;
    const n = this.wheelTurn.steps(e);
    if (n === null) return false;
    for (let i = 0; i < Math.abs(n); i++) this.turn(n > 0 ? 1 : -1, true);
    return true;
  }

  cancel() {
    this.stroke = null;
    this.wipes = null;
    this.host.ring(0, 0, null);
  }

  /** The tool is put away: nothing of it stays on the map. */
  dispose() {
    this.cancel();
    this.holding = null;
    this.at = null;
    this.host.ghost(null, true);
    this.host.chip(0, 0, null);
  }

  /** Put the held piece back where it was (before an undo, say). */
  putBack(): boolean {
    if (!this.holding) return false;
    this.holding = null;
    this.probed = null;
    this.refresh();
    return true;
  }

  turn(dir: 1 | -1, fine: boolean): boolean {
    const s = this.host.settings();
    if (s.mode !== 'place' || !this.pickUnder() && this.hand() !== 'chosen') return false;
    const step = dir * (fine ? 15 : 45);
    const kind = this.holding ? this.holding.now.kind : s.kinds[0];
    if (!this.turns(kind)) {
      this.host.hint(`${this.nameOf(kind)}: looks the same any way round`);
    } else if (this.holding) {
      const now = this.holding.now;
      this.holding.now = { ...now, rot: rad(degrees(this.shown(now) + step)), turned: true };
    } else {
      // From the turn the ghost shows (an always-turned kind's roll, else upright).
      const from = s.rotation ?? (typeof kind === 'string' || drawnTurned(this.kindInfo(kind), false) ? deg(this.next.rot) : 0);
      s.rotation = degrees(from + step);
    }
    this.refresh();
    return true;
  }

  turnReset(): boolean {
    const s = this.host.settings();
    if (s.mode !== 'place' || !this.pickUnder() && this.hand() !== 'chosen') return false;
    if (this.holding) {
      const { turned: _, ...now } = this.holding.now;
      this.holding.now = { ...now, rot: Math.random() * Math.PI * 2 };
    } else s.rotation = null;
    this.refresh();
    return true;
  }

  putDown(): boolean {
    if (!this.holding || !this.at) return false;
    this.drop(...this.at);
    this.refresh();
    return true;
  }

  remove(): boolean {
    const id = this.holding?.id ?? (this.hand() === 'empty' ? this.under : null);
    if (!id) return false;
    const name = this.nameOf((this.holding?.now ?? this.host.edits().objects?.[id])?.kind ?? 0).toLowerCase();
    this.holding = null;
    this.under = null;
    this.probed = null;
    if (this.host.edits().objects?.[id]) this.host.commit({ [id]: null }, {}, { tool: 'remove_objects', count: 1, name });
    this.refresh();
    return true;
  }

  size(d: 1 | -1): boolean {
    const s = this.host.settings();
    if (s.mode !== 'place') {
      s.radius = Math.max(5, Math.min(60, s.radius + d * 2.5));
      if (this.at) this.host.ring(...this.at, s.radius);
      return true;
    }
    this.pickUnder();
    if (this.holding) {
      const now = this.holding.now;
      this.holding.now = { ...now, scale: Math.round(Math.max(0.1, Math.min(10, now.scale * (d > 0 ? 1.1 : 1 / 1.1))) * 1000) / 1000 };
    } else s.scale = Math.round(Math.max(0.5, Math.min(2.5, s.scale + d * 0.1)) * 100) / 100;
    this.refresh();
    return true;
  }

  escape(): boolean {
    if (this.putBack()) return true;
    const s = this.host.settings();
    if (s.mode !== 'place' || !s.kinds.length) return false;
    s.kinds = [];
    this.probed = null;
    this.refresh();
    return true;
  }

  /** The outline of what a click would pick up, and where the held piece was (dashed). */
  draw(g: Graphics, cam: Camera) {
    const e = this.host.edits();
    const catalog = this.host.catalog();
    const ring = (o: Placed, dashed: boolean) => {
      const [sx, sy] = cam.worldToScreen(o.x, o.y);
      const r = Math.max(6, Math.max(PICK_FT, placedRadius(o, e, catalog)) * cam.ppf);
      for (const [width, color, alpha] of [
        [3.5, 0x1c1917, 0.45],
        [1.6, 0xfde047, 0.95],
      ]) {
        if (!dashed) g.circle(sx, sy, r).stroke({ width, color, alpha });
        else {
          const n = Math.max(8, Math.min(48, Math.round(r / 4))) * 2;
          for (let i = 0; i < n; i += 2) g.moveTo(sx + r * Math.cos((i / n) * 2 * Math.PI), sy + r * Math.sin((i / n) * 2 * Math.PI)).arc(sx, sy, r, (i / n) * 2 * Math.PI, ((i + 1) / n) * 2 * Math.PI);
          g.stroke({ width, color, alpha });
        }
      }
    };
    if (this.holding) ring(this.holding.from, true);
    else if (this.under && e.objects?.[this.under] && this.at) ring(e.objects[this.under], false);
  }

  /** Brush: objects on a jittered lattice (spacing apart) within the radius, cells not yet
   * filled in this stroke and clear of objects put down before. */
  private dab(x: number, y: number) {
    const s = this.host.settings();
    const stroke = this.stroke!;
    const sp = Math.max(2.5, s.spacing);
    const r = s.radius;
    const placed = this.host.edits().objects;
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
        if (this.index.any(placed, px, py, sp * 0.6)) continue;
        const o = this.make(s.kinds[h % s.kinds.length], px, py, roll());
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
    const id = this.index.at(e.objects, x, y, PICK_FT);
    if (id) return this.host.commit({ [id]: null }, {}, { tool: 'remove_objects', count: 1 });
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
