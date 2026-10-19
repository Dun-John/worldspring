// The designer: changes an underground site or a building's interior (a copy of the generated
// one, or the design saved before) and shows each change at once as what it builds. Changes go
// to a draft with its own undo; Save puts the draft in the world's edits, and only when it keeps
// the rules play mode needs (the generator checks them: `under::design::check`, a building's
// `interior::design::check`).
//
// On the map, underground: paint a room, dig a rectangle room or a corridor, fill squares back
// with rock; click a wall for a door (again: a secret door, again: none); put props down; click a
// square for the level's way down. In a building: paint squares into a room, draw a wall line to
// split a room, click a wall to take it away (two rooms made one), doors as underground (in an
// outside wall: a back door, then the front door), furniture, indoor props and uploaded pictures,
// drag out the stair block; storeys added on top or taken away (the building follows on Save).
// Right-drag pans.
//
// Props and furniture are placed as placing.ts describes: the chosen piece's ghost follows the
// pointer (red where it can't go, amber in a doorway); a click on a piece picks it up (also with
// another chosen: nothing can go there, so its ghost shows red under the piece's outline), a click
// puts it down again (one change); T turns a quarter, Delete takes it away. A side turned to by
// hand is kept only where the piece wouldn't face it by itself.
import type { Graphics } from 'pixi.js';
import type { DesignProblem, InteriorItem, SiteDesign, UnderCatalog } from '../../gen/protocol';
import type { Camera } from '../../render/camera';
import type { MapView, PointerTool } from '../../render/MapView';
import { APSE_E, APSE_N, APSE_S, APSE_W, isApse } from '../../render/roomShapes';
import { FINGER_PX, WheelTurn, type BarAt, type HandState, type HeldView, type Placer } from '../placing';
import { addLevel, addProp, addRoom, BOSS, cycleDoor, cycleOuterDoor, decode, doorPlace, type Edge, fits, isBuilding, type Item, mergeRooms, moveProp, paint, propAt, rectSquares, removeLevel, setStairs, setWayDown, splitRooms, storeysOf, cellarsOf, belowGround, wallLine, WAYS } from './model';

export type DesignMode = 'select' | 'room' | 'rect' | 'corridor' | 'rock' | 'door' | 'prop' | 'stairs' | 'wall' | 'merge';

/** The designer menu's choices. */
export interface DesignSettings {
  mode: DesignMode;
  /** The kind of room a new one is. */
  kind: string;
  /** Brush width (squares). */
  width: number;
  /** The piece in the hand ('' none: a click picks one up). */
  prop: string;
  /** The prop's size (squares), across and down. */
  pw: number;
  ph: number;
  /** The side it is turned to stand against (0 north 1 east 2 south 3 west); null: its own. */
  back: number | null;
  /** Doors added wherever a room dug would be shut off. */
  autoDoors: boolean;
}

export const defaultDesign = (): DesignSettings => ({ mode: 'room', kind: 'chamber', width: 1, prop: 'chest', pw: 1, ph: 1, back: null, autoDoors: true });

/** What the designer needs from the app. */
export interface DesignHost {
  settings(): DesignSettings;
  catalog(): UnderCatalog | null;
  /** The draft changed (problems, undo, the room chosen). */
  changed(): void;
  hint(text: string): void;
  /** A piece's name and whether it blocks the way (a prop, furniture or `s:<asset>`). */
  piece(kind: string): { name: string; blocks: boolean } | null;
  /** The chip by the ghost (world ft: its corner), or none. */
  chip(x: number, y: number, text: string | null): void;
  /** The place bar by the ghost (touch), or none. */
  bar(at: BarAt): void;
  /** The piece picked up, for the panel's "in hand" row, or none. */
  held(h: HeldView): void;
  /** Screen px per world ft. */
  ppf(): number;
}

/** A piece picked up: where it is in the list (on its level), as it was, as it is now (turned),
 * and the square of it the pointer took it by. */
type Held = { li: number; index: number; from: Item; now: Item; grab: [number, number] };

type Sq = [number, number];

export class SiteDesigner implements PointerTool, Placer {
  draft: SiteDesign | null = null;
  problems: DesignProblem[] = [];
  /** The room chosen (on the level in view), for its settings and to paint into. */
  selected: number | null = null;
  /** The design as last saved (JSON), to tell unsaved changes. */
  private saved = '';
  private past: SiteDesign[] = [];
  private future: SiteDesign[] = [];
  private queue: Promise<void> = Promise.resolve();
  /** The level as it was before a room was last given a shape here (`li`, `ri`), and as that left
   * it (`after`): while nothing else has changed it, the next shape for that room is fitted from
   * the squares it had before, so turning an apse round (or going back to square corners)
   * returns to where it started. */
  private shapeBase: { li: number; ri: number; level: SiteDesign['levels'][number]; after: string } | null = null;
  /** A stroke (squares painted so far) or a rectangle (its corners), being drawn. */
  private stroke: Set<number> | null = null;
  private last: Sq | null = null;
  private rect: { from: Sq; to: Sq } | null = null;
  /** A wall line being drawn (grid corners). */
  private line: { from: Sq; to: Sq } | null = null;
  /** The pointer (grid units). */
  private pointer: [number, number] | null = null;
  /** A building's storeys as they are (the draft's may differ until Save). */
  private floors = 0;
  /** Props: the piece picked up, if any. */
  private holding: Held | null = null;
  /** Props: the piece a click would pick up (its index on the level), else -1. */
  private under = -1;
  /** Props on a touch screen: the ghost stays where it was tapped or dragged to (grid). */
  private touch = false;
  private spot: [number, number] | null = null;
  /** A finger dragging the ghost: from the finger to the spot (grid). */
  private dragging: [number, number] | null = null;
  private readonly wheelTurn = new WheelTurn();
  /** The ghost as last shown, and whether it fits there (for its outline). */
  private ghostShown: { f: Item; state: 'ok' | 'doorway' | 'bad' } | null = null;
  /** The ghost and chip follow the pointer as the camera moves under it. */
  readonly hoverFollows = true;

  constructor(
    private readonly view: MapView,
    readonly id: string,
    private readonly host: DesignHost,
  ) {}

  /** The site as it is (saved design, else generated), shown as the draft. */
  async open(): Promise<boolean> {
    const reply = await this.view.gen.design(this.id);
    if ('error' in reply) {
      this.host.hint(reply.error);
      return false;
    }
    this.saved = JSON.stringify(reply.design);
    this.floors = storeysOf(reply.design);
    await this.take(reply.design, reply.problems, reply.interior);
    if (reply.set_aside) this.host.hint('Its inside was designed for the building as it was: this is the generated one (saving replaces that design)');
    return true;
  }

  get dirty(): boolean {
    return !!this.draft && JSON.stringify(this.draft) !== this.saved;
  }
  get blocking(): DesignProblem[] {
    return this.problems.filter((p) => p.blocking);
  }
  get canUndo(): boolean {
    return this.past.length > 0;
  }
  get canRedo(): boolean {
    return this.future.length > 0;
  }
  /** The level in view (index, bottom to top). */
  get level(): number {
    return this.view.interior?.currentLevel ?? 0;
  }

  private seenLevel = -1;

  /** The view moved to another level: the room chosen was on the one before. */
  levelShown() {
    if (this.level !== this.seenLevel) this.selected = null;
    this.seenLevel = this.level;
    this.refresh();
    this.host.changed();
  }

  /** The draft was saved as it is (a building's storeys too). */
  markSaved() {
    this.saved = JSON.stringify(this.draft);
    if (this.draft && this.building) this.floors = storeysOf(this.draft);
    this.host.changed();
  }

  /** Levels the draft has dug (or filled in) below the site as it is: the others' numbers move
   * by that much. */
  get shift(): number {
    return this.draft && this.saved ? belowGround(this.draft) - this.belowSaved : 0;
  }

  /** Levels below ground of the site as it is (saved, else generated). */
  get belowSaved(): number {
    return this.saved ? belowGround(JSON.parse(this.saved)) : 0;
  }

  /** A building's storeys in the draft. */
  get storeys(): number {
    return this.draft ? storeysOf(this.draft) : 0;
  }

  /** The draft has more or fewer storeys than the building: Save changes the building too. */
  get storeysChanged(): boolean {
    return this.building && this.storeys !== this.floors;
  }

  /** What `d` builds (changed by `action`), checked as the building will be: with the draft's
   * storeys, if they differ from its own. */
  private ask(d: SiteDesign, action?: object) {
    const n = storeysOf(d);
    const fit = isBuilding(d) && n !== this.floors && !(action && 'refit' in action);
    return this.view.gen.design(this.id, d, fit ? { ...action, refit: n } : action);
  }

  /** A new draft, shown once its site is drawn. */
  private async take(d: SiteDesign, problems: DesignProblem[], interior?: Parameters<MapView['showInterior']>[0]) {
    // (Levels added or taken away at the bottom move the others' indices: the room chosen goes.)
    if (this.draft && this.draft.levels.length !== d.levels.length) this.selected = null;
    this.draft = d;
    this.problems = problems;
    if (interior) await this.view.showInterior(interior);
    this.seenLevel = this.level;
    if (this.selected !== null && !(this.selected < (d.levels[this.level]?.rooms.length ?? 0))) this.selected = null;
    // (A piece held is let go if the change took it away or moved it in the list.)
    const h = this.holding;
    if (h && !same(d.levels[h.li]?.items[h.index], h.from)) this.holding = null;
    this.refresh();
    this.host.changed();
  }

  /** Change the draft (`f` returns false to change nothing), then `action` by the generator
   * (doors where needed, a room furnished); undoable. Changes apply in order. */
  change(f: (d: SiteDesign) => boolean | void, action?: object): Promise<void> {
    this.queue = this.queue.then(async () => {
      if (!this.draft) return;
      const next = plain(this.draft);
      if (f(next) === false) return;
      const reply = await this.ask(next, action);
      if ('error' in reply) return this.host.hint(reply.error);
      this.past.push(this.draft);
      if (this.past.length > 200) this.past.shift();
      this.future = [];
      await this.take(reply.design, reply.problems, reply.interior);
    });
    return this.queue;
  }

  undo() {
    this.putBack();
    this.step(this.past, this.future);
  }
  redo() {
    this.putBack();
    this.step(this.future, this.past);
  }
  private step(from: SiteDesign[], to: SiteDesign[]) {
    this.queue = this.queue.then(async () => {
      const d = from.pop();
      if (!d || !this.draft) return;
      const reply = await this.ask(d);
      if ('error' in reply) return this.host.hint(reply.error);
      to.push(this.draft);
      await this.take(reply.design, reply.problems, reply.interior);
    });
  }

  /** The generated site again (in the draft: Save keeps it). */
  original() {
    this.putBack();
    this.queue = this.queue.then(async () => {
      const reply = await this.view.gen.design(this.id, undefined, { original: true });
      if ('error' in reply || !this.draft) return;
      this.past.push(this.draft);
      this.future = [];
      await this.take(reply.design, reply.problems, reply.interior);
    });
  }

  // ---------------------------------------------------------------------------------------
  // Rooms and levels (the menu's buttons).

  setRoom(ri: number, room: Partial<{ kind: string; raise_ft: number }>) {
    const li = this.level;
    void this.change((d) => {
      const r = d.levels[li]?.rooms[ri];
      if (!r) return false;
      Object.assign(r, room);
    });
  }

  /** The room drawn in a shape (`roomShapes.ts`), its squares fitted to it: squares outside the
   * shape go back to rock (a building's to the room beside them). An apse goes on the side away
   * from the room's doors. Refused (a hint) when the shape doesn't fit or would break a rule. */
  setShape(ri: number, shape: number) {
    const li = this.level;
    const base = this.shapeFrom(li, ri);
    if (!base || !this.draft) return;
    const d = plain(this.draft);
    d.levels[li] = plain(base);
    this.fitShape(li, ri, base, [isApse(shape) ? apseAway(d, li, ri) : shape]);
  }

  /** A room's apse moved round to the next side that takes it (north, east, south, west). */
  turnApse(ri: number) {
    const li = this.level;
    const s = this.draft?.levels[li]?.rooms[ri]?.shape;
    const base = this.shapeFrom(li, ri);
    if (!isApse(s) || !base) return;
    this.fitShape(li, ri, base, [1, 2, 3].map((k) => APSE_N + ((s! - APSE_N + k) % 4)));
  }

  /** The level to fit room `ri`'s next shape from: as it was before its last shape here, if
   * nothing else has changed since; else as it is. */
  private shapeFrom(li: number, ri: number): SiteDesign['levels'][number] | null {
    const now = this.draft?.levels[li];
    if (!now) return null;
    const b = this.shapeBase;
    return b && b.li === li && b.ri === ri && b.after === JSON.stringify(now) ? b.level : now;
  }

  /** Room `ri` of level `li` given the first of `codes` that the generator takes, fitted from
   * `base`; undoable. */
  private fitShape(li: number, ri: number, base: SiteDesign['levels'][number], codes: number[]) {
    this.queue = this.queue.then(async () => {
      if (!this.draft) return;
      let refusal = '';
      for (const code of codes) {
        const next = plain(this.draft);
        next.levels[li] = plain(base);
        const reply = await this.ask(next, { shape: { level: li, room: ri, shape: code } });
        if ('error' in reply) {
          refusal ||= reply.error;
          continue;
        }
        this.past.push(this.draft);
        if (this.past.length > 200) this.past.shift();
        this.future = [];
        this.shapeBase = { li, ri, level: plain(base), after: JSON.stringify(reply.design.levels[li]) };
        // (Pieces left in the rock, in a doorway or across two rooms go: say which.)
        const now = reply.design.levels[li].items;
        const gone = base.items.filter((f) => !now.some((o) => o.kind === f.kind && o.x === f.x && o.y === f.y));
        await this.take(reply.design, reply.problems, reply.interior);
        if (gone.length) this.host.hint(`Taken away as the room was fitted to its shape: ${gone.map((f) => `${this.host.piece(f.kind)?.name ?? f.kind} at ${f.x},${f.y}`).join(', ')}`);
        return;
      }
      this.host.hint(refusal);
    });
  }

  furnish(ri: number) {
    void this.change(() => {}, { furnish: { level: this.level, room: ri, seed: Math.floor(Math.random() * 2 ** 31) } });
  }

  /** Every prop (a building's furniture) standing in the room taken away. */
  clearProps(ri: number) {
    const li = this.level;
    void this.change((d) => {
      const lv = d.levels[li];
      const cells = decode(lv.cells, d.nx * d.ny);
      const n = lv.items.length;
      // (Spiral stairs stay: above the stair block they are the way up.)
      lv.items = lv.items.filter((f) => WAYS.includes(f.kind) || f.kind === 'spiral_stair' || cells[f.y * d.nx + f.x] !== ri);
      return lv.items.length !== n;
    });
  }

  get building(): boolean {
    return isBuilding(this.draft);
  }

  /** The room filled back with rock. */
  deleteRoom(ri: number) {
    const li = this.level;
    void this.change((d) => {
      const cells = decode(d.levels[li].cells, d.nx * d.ny);
      const squares = [...cells.keys()].filter((k) => cells[k] === ri);
      if (!squares.length) return false;
      paint(d, li, squares, -1);
    });
    this.selected = null;
  }

  addLevel() {
    void this.change((d) => {
      if (d.levels.length >= (this.host.catalog()?.max_levels ?? 6)) {
        this.host.hint('A site has at most 6 levels');
        return false;
      }
      if (!addLevel(d)) {
        this.host.hint('The deepest level has no free floor for a way down');
        return false;
      }
    });
    // The new level is the deepest: go there once it is shown.
    void this.queue.then(() => this.view.setInteriorLevel(0));
  }

  removeLevel() {
    void this.change((d) => {
      if (d.levels.length <= 1) return false;
      removeLevel(d);
    });
  }

  /** A building's storeys: one added on top (as generated, the stairs going on up) or the top one
   * taken away; an open roof and a keep's tower tops follow. Saved with the building. */
  setStoreys(n: number) {
    const most = this.host.catalog()?.building.max_floors ?? 8;
    if (n < 1 || n > most) return this.host.hint(n < 1 ? 'A building has at least one storey' : `A building has at most ${most} storeys`);
    void this.change(() => {}, { refit: n });
    // Up onto the new top floor (or the one left on top).
    void this.queue.then(() => {
      const top = this.draft?.levels.findIndex((l) => (l.z ?? 0) === n - 1 && !l.roof) ?? -1;
      if (top >= 0 && top !== this.level) this.view.setInteriorLevel(top);
    });
  }

  /** A building's levels below ground: one dug below the deepest (a storeroom under the whole
   * building, the stairs going on down; shown once made) or the deepest filled in. The ways to
   * the sewers or a keep's deep dungeons are on the deepest. */
  setCellars(n: number) {
    const most = this.host.catalog()?.building.max_cellars ?? 3;
    if (n < 0 || n > most) return this.host.hint(`A building has at most ${most} levels below ground`);
    const was = this.draft ? cellarsOf(this.draft) : 0;
    const at = this.level;
    void this.change(() => {}, { cellars: n });
    // Down to the new level; else the level in view stays (one lower in the list).
    void this.queue.then(() => this.view.setInteriorLevel(n > was ? 0 : Math.max(0, at - (was - n))));
  }

  /** Doors wherever a room on this level is shut off. */
  addDoors() {
    void this.change(() => {}, { doors: this.level });
  }

  // ---------------------------------------------------------------------------------------
  // The map.

  /** The square under a world point (grid units, maybe outside the grid). */
  private grid(x: number, y: number): [number, number] | null {
    const layer = this.view.interior;
    return layer ? layer.toGrid(x, y) : null;
  }

  private square(x: number, y: number): Sq | null {
    const g = this.grid(x, y);
    const d = this.draft;
    if (!g || !d) return null;
    const [i, j] = [Math.floor(g[0]), Math.floor(g[1])];
    return i >= 0 && j >= 0 && i < d.nx && j < d.ny ? [i, j] : null;
  }

  /** The brush's squares about a square. */
  private brush([i, j]: Sq): number[] {
    const d = this.draft!;
    const w = Math.max(1, Math.min(3, this.host.settings().width));
    const lo = -Math.floor((w - 1) / 2);
    return rectSquares(d, i + lo, j + lo, i + lo + w - 1, j + lo + w - 1);
  }

  private brushing(): boolean {
    return ['room', 'corridor', 'rock'].includes(this.host.settings().mode);
  }

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0 || !this.draft) return false;
    const sq = this.square(x, y);
    const mode = this.host.settings().mode;
    if (mode === 'prop' && e.pointerType !== 'mouse') {
      // A finger on the ghost drags it; anywhere else it pans (a tap moves the ghost: `tap`).
      this.touch = true;
      const g = this.ghost();
      const p = this.grid(x, y);
      const reach = FINGER_PX / (this.host.ppf() * 5);
      if (!g || !p || !this.spot || p[0] < g.x - reach || p[1] < g.y - reach || p[0] > g.x + g.w + reach || p[1] > g.y + g.h + reach) return false;
      this.dragging = [this.spot[0] - p[0], this.spot[1] - p[1]];
      return true;
    }
    if (this.brushing()) {
      if (!sq) return true;
      this.stroke = new Set(this.brush(sq));
      this.last = sq;
    } else if ((mode === 'rect' || (mode === 'stairs' && this.building)) && sq) {
      this.rect = { from: sq, to: sq };
    } else if (mode === 'wall') {
      const g = this.grid(x, y);
      if (g) this.line = { from: [Math.round(g[0]), Math.round(g[1])], to: [Math.round(g[0]), Math.round(g[1])] };
    }
    return true;
  }

  move(x: number, y: number, e: PointerEvent) {
    this.pointer = this.grid(x, y);
    if (this.dragging) {
      if (this.pointer) this.spot = [this.pointer[0] + this.dragging[0], this.pointer[1] + this.dragging[1]];
      return this.refresh();
    }
    if (e.pointerType === 'mouse' && this.host.settings().mode === 'prop') {
      this.touch = false;
      this.refresh();
    }
    if (this.line && this.pointer) {
      // Along the grid line the pointer is furthest along.
      const [p, f] = [this.pointer, this.line.from];
      const [dx, dy] = [Math.round(p[0]) - f[0], Math.round(p[1]) - f[1]];
      this.line.to = Math.abs(dx) >= Math.abs(dy) ? [f[0] + dx, f[1]] : [f[0], f[1] + dy];
    }
    const sq = this.square(x, y);
    if (!sq) return;
    if (this.stroke && this.last) {
      // Every square between the last and this one: no gaps when the pointer runs.
      const [a, b] = [this.last, sq];
      const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
      for (let s = 1; s <= n; s++) {
        const p: Sq = [Math.round(a[0] + ((b[0] - a[0]) * s) / n), Math.round(a[1] + ((b[1] - a[1]) * s) / n)];
        for (const k of this.brush(p)) this.stroke.add(k);
      }
      this.last = sq;
    }
    if (this.rect) this.rect.to = sq;
  }

  up(x: number, y: number) {
    const s = this.host.settings();
    const li = this.level;
    const d = this.draft;
    if (!d) return;
    if (this.dragging) {
      this.dragging = null;
      return this.refresh();
    }
    const doors = s.autoDoors ? { doors: li } : undefined;
    if (this.line) {
      const { from, to } = this.line;
      this.line = null;
      return this.drawWall(from, to, x, y, doors);
    }
    if (this.stroke) {
      // (A building's squares stay inside its walls: only those with a room are painted.)
      const inside = this.building ? decode(d.levels[li].cells, d.nx * d.ny) : null;
      const squares = [...this.stroke].filter((k) => !inside || inside[k] >= 0);
      this.stroke = null;
      this.last = null;
      if (s.mode === 'rock') return void this.change((n) => paint(n, li, squares, -1));
      const corridor = s.mode === 'corridor' ? this.corridorKind(d) : null;
      const into = s.mode === 'room' && this.selected !== null ? this.selected : null;
      void this.change((n) => {
        const rooms = n.levels[li].rooms;
        let ri = into ?? (corridor !== null ? rooms.findIndex((r) => r.kind === corridor) : -1);
        if (ri < 0) ri = addRoom(n, li, corridor ?? s.kind);
        paint(n, li, squares, ri);
        if (corridor === null) this.selected = ri;
      }, doors);
      return;
    }
    if (this.rect) {
      const { from, to } = this.rect;
      this.rect = null;
      if (s.mode === 'stairs') {
        const [x0, y0] = [Math.min(from[0], to[0]), Math.min(from[1], to[1])];
        const [w, h] = [Math.abs(to[0] - from[0]) + 1, Math.abs(to[1] - from[1]) + 1];
        if (w > 3 || h > 3) return this.host.hint('The stairs are 1 to 3 squares each way');
        return void this.change((n) => setStairs(n, x0, y0, w, h), s.autoDoors ? { doors: null } : undefined);
      }
      const inside = this.building ? decode(d.levels[li].cells, d.nx * d.ny) : null;
      const squares = rectSquares(d, from[0], from[1], to[0], to[1]).filter((k) => !inside || inside[k] >= 0);
      if (!squares.length) return;
      void this.change((n) => {
        const ri = addRoom(n, li, s.kind);
        paint(n, li, squares, ri);
        this.selected = ri;
      }, doors);
      return;
    }
    const g = this.grid(x, y);
    const sq = this.square(x, y);
    if (!g || !sq) return;
    const cells = decode(d.levels[li].cells, d.nx * d.ny);
    const k = sq[1] * d.nx + sq[0];
    switch (s.mode) {
      case 'select':
        this.selected = cells[k] >= 0 ? cells[k] : null;
        this.host.changed();
        return;
      case 'door': {
        if (this.building) return this.buildingDoor(g, cells);
        const e = this.edge(g);
        if (!e) return this.host.hint('Click a wall between two rooms');
        const ka = e[1] * d.nx + e[0];
        const kb = ka + (e[2] === 0 ? 1 : d.nx);
        if (cells[ka] < 0 || cells[kb] < 0 || cells[ka] === cells[kb]) return this.host.hint('Doors go in walls between two rooms');
        return void this.change((n) => cycleDoor(n, li, e[0], e[1], e[2]));
      }
      case 'prop':
        return this.click(sq);
      case 'merge': {
        const e = this.edge4(g);
        if (!e) return;
        return void this.change((n) => {
          if (!mergeRooms(n, li, e)) {
            this.host.hint('Click a wall between two rooms');
            return false;
          }
        });
      }
      case 'stairs': {
        if (this.building) return;
        if (li === 0) return this.host.hint('The deepest level has no way down: add a level below first');
        if (cells[k] < 0) return this.host.hint('The way down goes on the floor');
        return void this.change((n) => setWayDown(n, li, sq[0], sq[1]), s.autoDoors ? { doors: li - 1 } : undefined);
      }
    }
  }

  /** A door in a building's wall: between two rooms none → door → secret door → none; in an
   * outside wall none → back door → front door → none. */
  private buildingDoor(g: [number, number], cells: Int16Array) {
    const d = this.draft!;
    const li = this.level;
    const e = this.edge4(g);
    if (!e) return;
    const room = (s: Sq) => (s[0] >= 0 && s[1] >= 0 && s[0] < d.nx && s[1] < d.ny ? cells[s[1] * d.nx + s[0]] : -1);
    const [ra, rb] = [room(e.a), room(e.b)];
    const at = doorPlace(d, cells, e);
    if (!at || ra === rb) return this.host.hint('Doors go in walls: between two rooms, or to the outside');
    if (ra >= 0 && rb >= 0) return void this.change((n) => cycleDoor(n, li, at[0], at[1], at[2]));
    if ((d.levels[li].z ?? 0) !== 0) return this.host.hint('Doors to the outside go on the ground floor');
    return void this.change((n) => cycleOuterDoor(n, li, at[0], at[1], at[2]));
  }

  /** A wall line from corner to corner splits the room it crosses (it runs on to the room's own
   * walls); a click puts one along the grid line nearest it. */
  private drawWall(from: Sq, to: Sq, x: number, y: number, doors?: object) {
    const d = this.draft!;
    const li = this.level;
    const cells = decode(d.levels[li].cells, d.nx * d.ny);
    let [p, q] = [from, to];
    if (p[0] === q[0] && p[1] === q[1]) {
      const g = this.grid(x, y);
      const e = g && this.edge4(g);
      if (!e) return;
      // The edge's line: between its squares.
      const [a, b] = [e.a, e.b];
      [p, q] = a[0] === b[0] ? [[a[0], Math.max(a[1], b[1])], [a[0] + 1, Math.max(a[1], b[1])]] : [[Math.max(a[0], b[0]), a[1]], [Math.max(a[0], b[0]), a[1] + 1]];
    }
    const walls = wallLine(d, cells, p, q);
    if (!walls.length) return this.host.hint('Draw the wall across a room');
    void this.change((n) => {
      const made = splitRooms(n, li, walls);
      if (!made.length) {
        this.host.hint('A wall must cut the room in two: draw it from wall to wall');
        return false;
      }
      this.selected = made[0];
    }, doors);
  }

  /** The square edge nearest a grid point, as the squares either side (one maybe off the grid:
   * a building's outside walls). */
  private edge4(g: [number, number]): Edge | null {
    const d = this.draft!;
    const [i, j] = [Math.floor(g[0]), Math.floor(g[1])];
    const [fx, fy] = [g[0] - i, g[1] - j];
    const near = Math.min(fx, 1 - fx, fy, 1 - fy);
    const e: Edge = near === fx ? { a: [i - 1, j], b: [i, j] } : near === 1 - fx ? { a: [i, j], b: [i + 1, j] } : near === fy ? { a: [i, j - 1], b: [i, j] } : { a: [i, j], b: [i, j + 1] };
    const on = (s: Sq) => s[0] >= 0 && s[1] >= 0 && s[0] < d.nx && s[1] < d.ny;
    return on(e.a) || on(e.b) ? e : null;
  }

  /** The room a corridor paints on this level: the theme's passages (caves: the open floor,
   * mines: drifts). */
  private corridorKind(d: SiteDesign): string {
    const t = this.host.catalog()?.themes.find((x) => x.key === d.theme);
    return t?.passage || (d.kind === 'mine' ? 'drift' : 'floor');
  }

  /** The square edge nearest a grid point: x, y, side (0: east of that square, 1: south). */
  private edge(g: [number, number]): [number, number, number] | null {
    const d = this.draft!;
    const [i, j] = [Math.floor(g[0]), Math.floor(g[1])];
    const [fx, fy] = [g[0] - i, g[1] - j];
    const near = Math.min(fx, 1 - fx, fy, 1 - fy);
    const e: [number, number, number] = near === fx ? [i - 1, j, 0] : near === 1 - fx ? [i, j, 0] : near === fy ? [i, j - 1, 1] : [i, j, 1];
    const ok = e[0] >= 0 && e[1] >= 0 && (e[2] === 0 ? e[0] + 1 < d.nx : e[0] < d.nx) && (e[2] === 1 ? e[1] + 1 < d.ny : e[1] < d.ny);
    return ok ? e : null;
  }

  cancel() {
    this.stroke = null;
    this.rect = null;
    this.line = null;
    this.last = null;
    this.dragging = null;
  }

  hover(x: number, y: number) {
    this.pointer = this.grid(x, y);
    this.touch = false;
    if (this.host.settings().mode === 'prop') this.refresh();
  }

  leave(e: PointerEvent) {
    // (A finger lifted leaves too: the ghost stays where it was put.)
    if (e.pointerType !== 'mouse') return;
    this.pointer = null;
    this.refresh();
  }

  /** A click the tool's `down` didn't take: a right click puts the held piece back; a tap moves
   * the ghost there (on a piece, picks it up). */
  tap(x: number, y: number, e: PointerEvent): boolean {
    if (!this.props()) return false;
    if (e.button === 2) return this.putBack();
    if (e.pointerType === 'mouse' || e.button !== 0) return false;
    this.touch = true;
    const sq = this.square(x, y);
    const at = sq && !this.holding ? propAt(this.draft!, this.level, sq[0], sq[1]) : -1;
    if (at >= 0) this.pickUp(at, sq!);
    else if (this.hand() !== 'empty') this.spot = this.grid(x, y);
    else this.host.hint('Choose a piece to put down, or tap one to pick it up');
    this.refresh();
    return true;
  }

  /** Shift+wheel turns what is in the hand. */
  wheel(e: WheelEvent): boolean {
    if (!e.shiftKey || !this.props() || this.hand() === 'empty') return false;
    const n = this.wheelTurn.steps(e);
    if (n === null) return false;
    for (let i = 0; i < Math.abs(n); i++) this.turn(n > 0 ? 1 : -1, true);
    return true;
  }

  /** The designer is shut: nothing of the hand stays on the map. */
  dispose() {
    this.holding = null;
    this.pointer = null;
    this.spot = null;
    this.view.interior?.setGhost(null);
    this.host.chip(0, 0, null);
    this.host.bar(null);
    this.host.held(null);
  }

  // ---------------------------------------------------------------------------------------
  // Props: what is in the hand.

  /** Props mode, a draft shown. */
  private props(): boolean {
    return this.host.settings().mode === 'prop' && !!this.draft && this.view.interior?.interior.id === this.id;
  }

  hand(): HandState {
    if (this.holding) return 'holding';
    return this.props() && this.host.settings().prop ? 'chosen' : 'empty';
  }

  /** Where the ghost is (grid): the pointer, or on a touch screen where it was put. */
  private ghostAt(): [number, number] | null {
    return this.touch ? this.spot : this.pointer;
  }

  /** What a click would put down, where the ghost is: the held piece (by the square it was taken
   * by), or the chosen one by its top-left square. */
  private ghost(): Item | null {
    const p = this.ghostAt();
    if (!p || !this.props()) return null;
    const [i, j] = [Math.floor(p[0]), Math.floor(p[1])];
    if (this.holding) return { ...this.holding.now, x: i - this.holding.grab[0], y: j - this.holding.grab[1] };
    const s = this.host.settings();
    if (!s.prop) return null;
    const f: Item = { kind: s.prop, x: i, y: j, w: Math.max(1, s.pw), h: Math.max(1, s.ph) };
    if (s.back !== null) f.back = s.back;
    return f;
  }

  /** A design's piece as the map draws it (an uploaded picture by its place in the site's list). */
  private shown(f: Item): InteriorItem {
    const p = this.host.piece(f.kind);
    const it: InteriorItem = { kind: f.kind, name: p?.name ?? f.kind, x: f.x, y: f.y, w: f.w, h: f.h, cover: 0, blocks_move: p?.blocks ?? false, height_ft: 0 };
    if (f.kind.startsWith('s:')) {
      it.kind = 'sprite';
      const k = this.view.interior?.interior.sprites?.findIndex((x) => x.asset === f.kind.slice(2)) ?? -1;
      if (k >= 0) it.sprite = k + 1;
    }
    if (f.back !== undefined) it.back = f.back;
    if (f.variant !== undefined) it.variant = f.variant;
    return it;
  }

  /** The side a piece would stand against where it is, by itself (an uploaded picture: its top,
   * north); null for one with no back. */
  private autoBack(f: Item): number | null {
    const { back: _, ...g } = f;
    return this.view.interior?.autoBack(this.shown(g)) ?? (f.kind.startsWith('s:') ? 0 : null);
  }

  /** The piece as stored: its side kept only where it wouldn't stand so by itself. */
  private posed(f: Item): Item {
    const { back, ...g } = f;
    return back !== undefined && back !== this.autoBack(g) ? { ...g, back } : g;
  }

  /** Whether the piece fits where it is (the held one aside). */
  private fit(f: Item): 'ok' | 'doorway' | 'bad' {
    const h = this.holding;
    return fits(this.draft!, this.level, f.x, f.y, f.w, f.h, h && h.li === this.level ? h.index : -1, this.host.piece(f.kind)?.blocks ?? true);
  }

  /** The ghost, its chip or bar, and the piece a click would pick up, as the hand and pointer are. */
  refresh() {
    const layer = this.view.interior;
    if (!layer || layer.interior.id !== this.id) return;
    const d = this.draft;
    if (this.holding && this.holding.li !== this.level) this.holding = null;
    const name = (f: Item) => this.host.piece(f.kind)?.name ?? f.kind;
    const h = this.holding;
    this.host.held(h ? { name: cap(name(h.now)), turn: '', turns: h.now.w !== h.now.h || (h.now.back ?? this.autoBack(h.now)) !== null, auto: h.now.back !== undefined, size: `${h.now.w}×${h.now.h}` } : null);
    this.under = -1;
    const g = this.ghost();
    const p = this.pointer;
    const sq = !this.touch && p && d ? [Math.floor(p[0]), Math.floor(p[1])] : null;
    // A click on a piece picks it up (an empty hand, or a piece chosen: nothing can go there).
    if (d && sq && !this.holding && this.props()) this.under = propAt(d, this.level, sq[0], sq[1]);
    const u = this.under >= 0 ? d?.levels[this.level].items[this.under] : undefined;
    this.ghostShown = null;
    if (!g) {
      layer.setGhost(null);
      this.host.bar(null);
      if (u) this.host.chip(...layer.toWorld(u.x + u.w, u.y + u.h), `${cap(name(u))} · click to pick up`);
      else this.host.chip(0, 0, null);
      return;
    }
    // (Over a piece with another chosen, the chosen one's ghost still shows, red, under the
    // piece's outline: plain what is in the hand and what a click picks up.)
    const state = this.fit(g);
    layer.setGhost(this.shown(g), state);
    this.ghostShown = { f: g, state };
    const turns = g.w !== g.h || (g.back ?? this.autoBack(g)) !== null;
    if (this.touch) {
      this.host.chip(0, 0, null);
      this.host.bar({ ...xy(layer.toWorld(g.x + g.w / 2, g.y + g.h / 2)), r: (Math.max(g.w, g.h) / 2) * 5, holding: !!this.holding, turns });
    } else {
      this.host.bar(null);
      const why = state === 'bad' ? ' · doesn’t fit here' : state === 'doorway' ? ' · in a doorway' : '';
      if (u) this.host.chip(...layer.toWorld(u.x + u.w, u.y + u.h), `${cap(name(u))} · click to pick up (${name(g)} in hand)`);
      else this.host.chip(...layer.toWorld(g.x + g.w, g.y + g.h), `${cap(name(g))}${g.w * g.h > 1 ? ` · ${g.w}×${g.h}` : ''}${why}`);
    }
  }

  /** The piece at `index` (on this level) into the hand, taken by square `sq`. */
  private pickUp(index: number, sq: Sq) {
    const f = this.draft!.levels[this.level].items[index];
    const clamp = (v: number, n: number) => Math.max(0, Math.min(n - 1, v));
    this.holding = { li: this.level, index, from: f, now: { ...f }, grab: [clamp(sq[0] - f.x, f.w), clamp(sq[1] - f.y, f.h)] };
    if (this.touch) this.spot = [sq[0] + 0.5, sq[1] + 0.5];
  }

  /** Takes the piece a click would pick up into the hand (for a key that acts on it). */
  private pickUnder(): boolean {
    if (this.holding) return true;
    const p = this.pointer;
    if (this.under < 0 || !p) return false;
    this.pickUp(this.under, [Math.floor(p[0]), Math.floor(p[1])]);
    return true;
  }

  /** A click in Props: the held piece put down, else the piece clicked picked up, else the chosen
   * piece put down. */
  private click(sq: Sq) {
    const d = this.draft!;
    if (this.holding) return void this.drop();
    const at = propAt(d, this.level, sq[0], sq[1]);
    if (at >= 0) {
      this.pickUp(at, sq);
      return this.refresh();
    }
    if (!this.host.settings().prop) return this.host.hint('Choose a piece to put down, or click one to pick it up');
    this.place();
  }

  /** One of the chosen piece put down where the ghost is. */
  private place(): boolean {
    const g = this.ghost();
    if (!g || this.holding) return false;
    if (this.fit(g) === 'bad') {
      this.host.hint(this.onFloor(g) ? 'It doesn’t fit there' : 'Props go on the floor');
      return true;
    }
    const li = this.level;
    const f = this.posed(g);
    void this.change((n) => addProp(n, li, f));
    return true;
  }

  /** Whether a piece's top-left square is floor (for the hint). */
  private onFloor(f: Item): boolean {
    const d = this.draft!;
    return f.x >= 0 && f.y >= 0 && f.x < d.nx && f.y < d.ny && decode(d.levels[this.level].cells, d.nx * d.ny)[f.y * d.nx + f.x] >= 0;
  }

  /** The held piece put down where the ghost is (in its place in the list: one change). */
  private drop(): boolean {
    const h = this.holding;
    const g = this.ghost();
    if (!h || !g) return false;
    if (this.fit(g) === 'bad') {
      this.host.hint('It doesn’t fit there');
      return true;
    }
    this.holding = null;
    const f = this.posed(g);
    if (!same(f, h.from)) void this.change((n) => same(n.levels[h.li]?.items[h.index], h.from) && moveProp(n, h.li, h.index, f));
    this.refresh();
    return true;
  }

  /** Put the held piece back where it was (before an undo, say). */
  putBack(): boolean {
    if (!this.holding) return false;
    this.holding = null;
    this.refresh();
    return true;
  }

  turn(dir: 1 | -1, _fine: boolean): boolean {
    if (!this.props() || (!this.pickUnder() && this.hand() !== 'chosen')) return false;
    const s = this.host.settings();
    const f = this.ghost() ?? this.holding?.now ?? { kind: s.prop, x: 0, y: 0, w: s.pw, h: s.ph, ...(s.back !== null ? { back: s.back } : {}) };
    const side = f.back ?? this.autoBack(f);
    if (f.w === f.h && side === null) {
      this.host.hint(`${cap(this.host.piece(f.kind)?.name ?? f.kind)}: looks the same any way round`);
      return true;
    }
    // A quarter turn: across becomes down, and the side it stands against turns with it.
    const back = side === null ? undefined : (side + dir + 4) % 4;
    if (this.holding) {
      const now = { ...this.holding.now, w: f.h, h: f.w };
      if (back === undefined) delete now.back;
      else now.back = back;
      const [gx, gy] = this.holding.grab;
      this.holding = { ...this.holding, now, grab: [Math.min(gx, now.w - 1), Math.min(gy, now.h - 1)] };
    } else {
      [s.pw, s.ph] = [f.h, f.w];
      s.back = back ?? null;
    }
    this.refresh();
    return true;
  }

  turnReset(): boolean {
    if (!this.props() || (!this.pickUnder() && this.hand() !== 'chosen')) return false;
    if (this.holding) {
      const { back: _, ...now } = this.holding.now;
      this.holding = { ...this.holding, now };
    } else this.host.settings().back = null;
    this.refresh();
    return true;
  }

  putDown(): boolean {
    return !!this.holding && this.drop();
  }

  confirm(): boolean {
    if (!this.props()) return false;
    return this.holding ? this.drop() : this.place();
  }

  remove(): boolean {
    if (!this.props()) return false;
    const h = this.holding;
    const li = this.level;
    const target = h ? { li: h.li, index: h.index, from: h.from } : this.under >= 0 ? { li, index: this.under, from: this.draft!.levels[li].items[this.under] } : null;
    if (!target) return false;
    this.holding = null;
    this.under = -1;
    void this.change((n) => {
      const items = n.levels[target.li]?.items;
      if (!items || !same(items[target.index], target.from)) return false;
      items.splice(target.index, 1);
    });
    this.refresh();
    return true;
  }

  size(): boolean {
    return false;
  }

  escape(): boolean {
    if (this.putBack()) return true;
    const s = this.host.settings();
    if (!this.props() || !s.prop) return false;
    s.prop = '';
    this.refresh();
    return true;
  }

  /** Double-clicks are the designer's (painting quickly): none takes a way out of the site,
   * which would leave the draft behind. */
  dblclick(): boolean {
    return true;
  }

  draw(g: Graphics, cam: Camera) {
    const layer = this.view.interior;
    const d = this.draft;
    if (!layer || !d || layer.interior.id !== this.id) return;
    const s = this.host.settings();
    const scr = (gx: number, gy: number) => cam.worldToScreen(...layer.toWorld(gx, gy));
    const quad = (x0: number, y0: number, x1: number, y1: number) => [...scr(x0, y0), ...scr(x1, y0), ...scr(x1, y1), ...scr(x0, y1)];
    const ink = s.mode === 'rock' ? 0x7f1d1d : 0xf97316;
    if (this.stroke) for (const k of this.stroke) g.poly(quad(k % d.nx, Math.floor(k / d.nx), (k % d.nx) + 1, Math.floor(k / d.nx) + 1)).fill({ color: ink, alpha: 0.35 });
    if (this.rect) {
      const { from, to } = this.rect;
      g.poly(quad(Math.min(from[0], to[0]), Math.min(from[1], to[1]), Math.max(from[0], to[0]) + 1, Math.max(from[1], to[1]) + 1))
        .fill({ color: ink, alpha: 0.3 })
        .stroke({ width: 2, color: ink });
    }
    if (this.line) {
      const [a, b] = [scr(...this.line.from), scr(...this.line.to)];
      g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: 4, color: ink, cap: 'round' });
    }
    // A building's stair block, while it is being moved.
    if (this.building && s.mode === 'stairs' && d.stairs && !this.rect) {
      const [sx, sy, sw, sh] = d.stairs;
      g.poly(quad(sx, sy, sx + sw, sy + sh)).stroke({ width: 2, color: 0x38bdf8 });
    }
    // The room chosen.
    const li = this.level;
    if (this.selected !== null && s.mode !== 'rock') {
      const cells = decode(d.levels[li]?.cells ?? [], d.nx * d.ny);
      for (let k = 0; k < cells.length; k++) if (cells[k] === this.selected) g.poly(quad(k % d.nx, Math.floor(k / d.nx), (k % d.nx) + 1, Math.floor(k / d.nx) + 1)).fill({ color: 0x38bdf8, alpha: 0.16 });
    }
    // What the pointer is on.
    const h = this.pointer;
    if (h && !this.stroke && !this.rect && !this.line) {
      const [i, j] = [Math.floor(h[0]), Math.floor(h[1])];
      if (this.building && (s.mode === 'door' || s.mode === 'merge' || s.mode === 'wall')) {
        const e = this.edge4(h);
        if (e && s.mode === 'wall') {
          const c = [Math.round(h[0]), Math.round(h[1])] as const;
          g.circle(...scr(c[0], c[1]), 4).fill({ color: ink });
        } else if (e) {
          // The edge between its two squares.
          const vert = e.a[1] === e.b[1];
          const [x0, y0] = [Math.max(e.a[0], e.b[0]), Math.max(e.a[1], e.b[1])];
          const [a, b] = vert ? [scr(x0, y0), scr(x0, y0 + 1)] : [scr(x0, y0), scr(x0 + 1, y0)];
          g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: 4, color: s.mode === 'merge' ? 0x7f1d1d : ink });
        }
      } else if (s.mode === 'door') {
        const e = this.edge(h);
        if (e) {
          const [a, b] = e[2] === 0 ? [scr(e[0] + 1, e[1]), scr(e[0] + 1, e[1] + 1)] : [scr(e[0], e[1] + 1), scr(e[0] + 1, e[1] + 1)];
          g.moveTo(a[0], a[1]).lineTo(b[0], b[1]).stroke({ width: 4, color: ink });
        }
      } else if (i >= 0 && j >= 0 && i < d.nx && j < d.ny) {
        if (this.brushing()) for (const k of this.brush([i, j])) g.poly(quad(k % d.nx, Math.floor(k / d.nx), (k % d.nx) + 1, Math.floor(k / d.nx) + 1)).stroke({ width: 1.5, color: ink });
        else if (s.mode !== 'prop') g.poly(quad(i, j, i + 1, j + 1)).stroke({ width: 1.5, color: ink });
      }
    }
    if (s.mode === 'prop') {
      const box = (f: Item, width: number, color: number) => g.poly(quad(f.x, f.y, f.x + f.w, f.y + f.h)).stroke({ width, color });
      // Where the held piece was (dashed), the ghost's squares (red where it can't go, amber in a
      // doorway), and on top the piece a click would pick up (gold).
      const h = this.holding;
      if (h && h.li === li) {
        const c = [scr(h.from.x, h.from.y), scr(h.from.x + h.from.w, h.from.y), scr(h.from.x + h.from.w, h.from.y + h.from.h), scr(h.from.x, h.from.y + h.from.h)];
        for (const [width, color] of [[3.5, 0x1c1917], [1.6, 0xfde047]]) {
          for (let k = 0; k < 4; k++) dash(g, c[k], c[(k + 1) % 4]);
          g.stroke({ width, color, alpha: 0.9 });
        }
      }
      if (this.ghostShown) {
        const st = this.ghostShown.state;
        box(this.ghostShown.f, st === 'ok' ? 1.5 : 2.5, st === 'bad' ? 0xdc2626 : st === 'doorway' ? 0xd97706 : ink);
      }
      const u = this.under >= 0 ? d.levels[li]?.items[this.under] : undefined;
      if (u) {
        box(u, 3.5, 0x1c1917);
        box(u, 1.6, 0xfde047);
      }
    }
    // Problems on this level.
    for (const p of this.problems) {
      if (p.level !== li || !p.at) continue;
      const [cx, cy] = scr(p.at[0] + 0.5, p.at[1] + 0.5);
      g.circle(cx, cy, Math.max(6, cam.ppf * 4)).stroke({ width: 2.5, color: p.blocking ? 0xdc2626 : 0xd97706 });
    }
  }
}

/** The apse for room `ri` of level `li` on the side of its box away from its doors (their middle);
 * north when it has none. */
function apseAway(d: SiteDesign, li: number, ri: number): number {
  const lv = d.levels[li];
  const cells = decode(lv.cells, d.nx * d.ny);
  const nx = d.nx;
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (let k = 0; k < cells.length; k++) {
    if (cells[k] !== ri) continue;
    const [i, j] = [k % nx, Math.floor(k / nx)];
    [x0, y0, x1, y1] = [Math.min(x0, i), Math.min(y0, j), Math.max(x1, i + 1), Math.max(y1, j + 1)];
  }
  if (x0 > x1) return APSE_N;
  const mids: [number, number][] = [];
  for (const [x, y, side] of lv.doors) {
    const other = side === 0 ? [x + 1, y] : side === 1 ? [x, y + 1] : side === 2 ? [x - 1, y] : [x, y - 1];
    const mine = cells[y * nx + x] === ri || (other[0] >= 0 && other[1] >= 0 && other[0] < nx && other[1] < d.ny && cells[other[1] * nx + other[0]] === ri);
    if (mine) mids.push(side === 0 ? [x + 1, y + 0.5] : side === 1 ? [x + 0.5, y + 1] : side === 2 ? [x, y + 0.5] : [x + 0.5, y]);
  }
  if (!mids.length) return APSE_N;
  const [mx, my] = [mids.reduce((a, m) => a + m[0], 0) / mids.length - (x0 + x1) / 2, mids.reduce((a, m) => a + m[1], 0) / mids.length - (y0 + y1) / 2];
  // (Measured against the box's size: a long room's doors are far from its middle along it.)
  const [ax, ay] = [mx / Math.max(1, x1 - x0), my / Math.max(1, y1 - y0)];
  return Math.abs(ax) > Math.abs(ay) ? (ax > 0 ? APSE_W : APSE_E) : ay > 0 ? APSE_N : APSE_S;
}

/** A deep copy (of plain JSON data). */
function plain<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

/** Whether two pieces are the same (where, how big, which way, which look). */
function same(a: Item | undefined, b: Item): boolean {
  return !!a && a.kind === b.kind && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h && a.back === b.back && a.variant === b.variant;
}

/** A dashed line from a to b (screen px), added to the path. */
function dash(g: Graphics, a: [number, number], b: [number, number]) {
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
  const n = Math.max(1, Math.round(len / 6));
  for (let i = 0; i < n; i += 2) {
    const [t0, t1] = [i / n, Math.min(1, (i + 1) / n)];
    g.moveTo(a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0).lineTo(a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1);
  }
}

const cap = (k: string) => k.charAt(0).toUpperCase() + k.slice(1);
const xy = ([x, y]: [number, number]) => ({ x, y });

export { BOSS };
