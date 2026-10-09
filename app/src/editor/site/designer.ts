// The designer: changes an underground site or a building's interior (a copy of the generated
// one, or the design saved before) and shows each change at once as what it builds. Changes go
// to a draft with its own undo; Save puts the draft in the world's edits, and only when it keeps
// the rules play mode needs (the generator checks them: `under::design::check`, a building's
// `interior::design::check`).
//
// On the map, underground: paint a room, dig a rectangle room or a corridor, fill squares back
// with rock; click a wall for a door (again: a secret door, again: none); put props down (click
// one to take it away); click a square for the level's way down. In a building: paint squares
// into a room, draw a wall line to split a room, click a wall to take it away (two rooms made
// one), doors as underground (in an outside wall: a back door, then the front door), furniture,
// indoor props and uploaded pictures, drag out the stair block; storeys added on top or taken
// away (the building follows on Save). Right-drag pans.
import type { Graphics } from 'pixi.js';
import type { DesignProblem, SiteDesign, UnderCatalog } from '../../gen/protocol';
import type { Camera } from '../../render/camera';
import type { MapView, PointerTool } from '../../render/MapView';
import { addLevel, addProp, addRoom, BOSS, cycleDoor, cycleOuterDoor, decode, doorPlace, type Edge, isBuilding, mergeRooms, paint, propAt, rectSquares, removeLevel, setStairs, setWayDown, splitRooms, storeysOf, cellarsOf, belowGround, wallLine, WAYS } from './model';

export type DesignMode = 'select' | 'room' | 'rect' | 'corridor' | 'rock' | 'door' | 'prop' | 'stairs' | 'wall' | 'merge';

/** The designer menu's choices. */
export interface DesignSettings {
  mode: DesignMode;
  /** The kind of room a new one is. */
  kind: string;
  /** Brush width (squares). */
  width: number;
  prop: string;
  /** The prop's size (squares), across and down. */
  pw: number;
  ph: number;
  /** Doors added wherever a room dug would be shut off. */
  autoDoors: boolean;
}

export const defaultDesign = (): DesignSettings => ({ mode: 'room', kind: 'chamber', width: 1, prop: 'chest', pw: 1, ph: 1, autoDoors: true });

/** What the designer needs from the app. */
export interface DesignHost {
  settings(): DesignSettings;
  catalog(): UnderCatalog | null;
  /** The draft changed (problems, undo, the room chosen). */
  changed(): void;
  hint(text: string): void;
}

type Sq = [number, number];

export class SiteDesigner implements PointerTool {
  draft: SiteDesign | null = null;
  problems: DesignProblem[] = [];
  /** The room chosen (on the level in view), for its settings and to paint into. */
  selected: number | null = null;
  /** The design as last saved (JSON), to tell unsaved changes. */
  private saved = '';
  private past: SiteDesign[] = [];
  private future: SiteDesign[] = [];
  private queue: Promise<void> = Promise.resolve();
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
    this.step(this.past, this.future);
  }
  redo() {
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

  move(x: number, y: number) {
    this.pointer = this.grid(x, y);
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
      case 'prop': {
        const at = propAt(d, li, sq[0], sq[1]);
        if (at >= 0) return void this.change((n) => void n.levels[li].items.splice(at, 1));
        if (cells[k] < 0) return this.host.hint('Props go on the floor');
        return void this.change((n) => {
          if (!addProp(n, li, s.prop, sq[0], sq[1], s.pw, s.ph)) {
            this.host.hint('It doesn’t fit there');
            return false;
          }
        });
      }
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
  }

  hover(x: number, y: number) {
    this.pointer = this.grid(x, y);
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
        else if (s.mode === 'prop' && propAt(d, li, i, j) < 0) g.poly(quad(i, j, i + s.pw, j + s.ph)).stroke({ width: 1.5, color: ink });
        else g.poly(quad(i, j, i + 1, j + 1)).stroke({ width: 1.5, color: ink });
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

/** A deep copy (of plain JSON data). */
function plain<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

export { BOSS };
