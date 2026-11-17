// Select: the map's pointer for choosing what is already there. A click lists what lies under
// it (`pick`); a dragged box, or a Shift-dragged lasso, lists what lies inside it. Right-drag
// pans meanwhile. The things chosen show as boxes over the map.
//
// Also the edits with things taken away (`withoutThings`): the same changes the tools that made
// them make (Build's Remove, the info card's Delete), put together as one.
import type { Graphics } from 'pixi.js';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import type { BuildingEdit, Created, Edits, Thing } from '../gen/protocol';
import type { Pt } from './build';

/** What the tool needs from the app. */
export interface SelectHost {
  /** Feet a screen pixel spans now. */
  ftPerPx(): number;
  /** A click (world ft). */
  clicked(at: Pt): void;
  /** An area dragged (world ft), a box or a lasso. */
  drawn(poly: Pt[]): void;
  /** The boxes to show (world ft): the open thing's first, brighter. */
  shown(): [number, number, number, number][];
  /** The area last chosen, shown until it is cleared. */
  area(): Pt[] | null;
}

/** Under this (screen px) a press and release is a click. */
const CLICK_PX = 5;
/** The lasso's step between points (screen px). */
const STEP_PX = 6;

export class SelectTool implements PointerTool {
  private path: Pt[] = [];
  private dragging = false;
  private lasso = false;
  readonly pinchCancels = true;

  constructor(private readonly host: SelectHost) {}

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    this.dragging = true;
    this.lasso = e.shiftKey;
    this.path = [[x, y]];
    return true;
  }

  move(x: number, y: number) {
    if (!this.dragging) return;
    const last = this.path[this.path.length - 1];
    if (!this.lasso) this.path = [this.path[0], [x, y]];
    else if (Math.hypot(x - last[0], y - last[1]) >= STEP_PX * this.host.ftPerPx()) this.path.push([x, y]);
  }

  up(x: number, y: number) {
    if (!this.dragging) return;
    this.move(x, y);
    this.dragging = false;
    const p0 = this.path[0];
    const far = this.path.some((p) => Math.hypot(p[0] - p0[0], p[1] - p0[1]) >= CLICK_PX * this.host.ftPerPx());
    const poly = far ? this.poly() : null;
    this.path = [];
    if (poly) this.host.drawn(poly);
    else this.host.clicked(p0);
  }

  cancel() {
    this.dragging = false;
    this.path = [];
  }

  hover() {}

  /** The area being drawn as a polygon, if it is one. */
  private poly(): Pt[] | null {
    const p = this.path;
    if (!this.lasso) {
      if (p.length < 2) return null;
      const [[x0, y0], [x1, y1]] = [p[0], p[1]];
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
    const ink = 0x2563eb;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    for (const poly of [this.host.area(), this.dragging ? (this.poly() ?? (this.path.length >= 2 ? this.path : null)) : null]) {
      if (!poly) continue;
      const flat = poly.flatMap((p) => scr(p));
      if (poly.length >= 3) g.poly(flat).fill({ color: ink, alpha: 0.08 });
      g.poly(flat, poly.length >= 3).stroke({ width: 1.5, color: ink, alpha: 0.8 });
    }
    this.host.shown().forEach((b, k) => {
      const flat = ([[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]] as Pt[]).flatMap((p) => scr(p));
      g.poly(flat).stroke({ width: k === 0 ? 2.5 : 1.5, color: k === 0 ? 0xf59e0b : ink, alpha: k === 0 ? 0.95 : 0.6 });
    });
  }
}

/** The ring round a box or lasso to ask `pick` over: its middle and the radius reaching every corner. */
export function ringOf(poly: Pt[]): { at: Pt; r: number } {
  const xs = poly.map((p) => p[0]);
  const ys = poly.map((p) => p[1]);
  const at: Pt = [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
  return { at, r: Math.max(...poly.map((p) => Math.hypot(p[0] - at[0], p[1] - at[1]))) };
}

function inside(poly: Pt[], x: number, y: number): boolean {
  let hit = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i];
    const [xj, yj] = poly[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) hit = !hit;
  }
  return hit;
}

/** Kinds that hold others (a town, its districts, a place): inside an area only when all of
 * them is. */
const HOLDERS = new Set<Thing['kind']>(['settlement', 'site', 'district', 'castle', 'plot']);

/** What of `things` (picked round the area) lies inside it: its box's middle inside (a holder's
 * whole box). Kept in pick's order. */
export function inArea(things: Thing[], poly: Pt[]): Thing[] {
  return things.filter((t) => {
    const [x0, y0, x1, y1] = t.bbox;
    if (HOLDERS.has(t.kind)) return inside(poly, x0, y0) && inside(poly, x1, y0) && inside(poly, x1, y1) && inside(poly, x0, y1);
    return inside(poly, (x0 + x1) / 2, (y0 + y1) / 2);
  });
}

/** A kind in plain words. */
export const KIND_WORDS: Record<Thing['kind'], [string, string]> = {
  entrance: ['way underground', 'ways underground'],
  building: ['building', 'buildings'],
  drawn_building: ['building drawn by hand', 'buildings drawn by hand'],
  tower: ['tower', 'towers'],
  castle: ['castle', 'castles'],
  plot: ['plot', 'plots'],
  wall: ['wall', 'walls'],
  settlement: ['settlement', 'settlements'],
  site: ['place', 'places'],
  district: ['district', 'districts'],
  road: ['road', 'roads'],
  drawn_road: ['road drawn by hand', 'roads drawn by hand'],
  crossing: ['crossing', 'crossings'],
  water: ['water drawn by hand', 'waters drawn by hand'],
  harbour: ['harbour piece', 'harbour pieces'],
  lane: ['lane', 'lanes'],
  street: ['street', 'streets'],
  hedge: ['hedge', 'hedges'],
  field_wall: ['field wall', 'field walls'],
  fence: ['fence', 'fences'],
};

/** One of a kind or many, in words: "3 roads". */
export const countOf = (kind: Thing['kind'], n: number) => `${n} ${KIND_WORDS[kind][n === 1 ? 0 : 1]}`;

/** The same thing listed twice (a drawn building is a building and a created entry). */
export const keyOf = (t: Thing) => `${t.kind}|${t.id ?? t.bbox.join(',')}`;

/**
 * `e` with `things` taken away, as one change: a created entry (`c:`) set removed (the info
 * card's Delete), a drawn road, water, crossing or harbour piece taken out with what goes with
 * it (Build's Remove: a water's name, note and hidden mark, a road's note, a crossing's or
 * pier's shops and lighthouse), one of the world's own buildings set as `gen` answered
 * (`buildingEdit` with `remove`). `towns`: created villages or towns among them (their edits
 * inside are then set aside, as the card's Delete does).
 */
export function withoutThings(e: Edits, things: Thing[], gen: Record<string, BuildingEdit>): { next: Edits; towns: boolean } {
  const ids = new Set(things.filter((t) => t.removable && t.id).map((t) => t.id as string));
  let created: Created[] | undefined = e.created;
  const set = (f: (c: Created) => boolean) => {
    if (created?.some((c) => !c.removed && f(c))) created = created.map((c) => (!c.removed && f(c) ? { ...c, removed: true } : c));
  };
  const drop = <T>(m: Record<string, T> | undefined, keys: string[]) => {
    if (!m || !keys.some((k) => k in m)) return m;
    return Object.fromEntries(Object.entries(m).filter(([k]) => !keys.includes(k)));
  };
  const of = (p: string) => [...ids].filter((id) => id.startsWith(p));
  const [cs, rw, ws, vs, qs] = [of('c:'), of('rw:'), of('e:'), of('v:'), of('q:')];
  const towns = cs.some((id) => {
    const c = e.created?.find((x) => x.id === id);
    return c?.kind === 'village' || c?.kind === 'town';
  });
  set((c) => cs.includes(c.id) || (!!c.on && (vs.includes(c.on) || qs.includes(c.on)) && c.kind === 'building'));
  const next: Edits = { ...e };
  if (created !== e.created) next.created = created;
  if (rw.length) next.paths = drop(e.paths, rw);
  if (ws.length) {
    next.waters = drop(e.waters, ws);
    next.renames = drop(e.renames, ws);
    if (e.hidden?.some((h) => ws.includes(h))) next.hidden = e.hidden.filter((h) => !ws.includes(h));
  }
  if (rw.length || ws.length) next.notes = drop(e.notes, [...rw, ...ws]);
  if (vs.length) next.crossings = drop(e.crossings, vs);
  if (qs.length) next.wharves = drop(e.wharves, qs);
  const bs = Object.keys(gen).filter((id) => ids.has(id));
  if (bs.length) next.buildings = { ...(e.buildings ?? {}), ...Object.fromEntries(bs.map((id) => [id, gen[id]])) };
  return { next, towns };
}
