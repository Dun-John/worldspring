// Play mode: one per window. In the DM's window it owns the play state (changed through ops,
// saved per world), the tools (tokens, ruler, shapes, fog brush, doors, pings), what the
// characters can see, and the link to the player window; in the player window it follows the
// DM (state, location, camera, place pins) and draws the players' view.
import type { Feature, Interior, WorldFile } from '../gen/protocol';
import type { InteriorLayer } from '../render/InteriorLayer';
import type { MapView, PointerTool } from '../render/MapView';
import { SECTION_N, type Sewers } from '../render/Sewers';
import { PlayChannel, type CameraMsg, type Msg, type Role, type Ruler, type Scene, type Vision } from './channel';
import { PlayLayer } from './PlayLayer';
import { loadSession, putImage, saveSession } from './session';
import { apply, emptyState, isUnderground, locKey, newId, normalize, parseLoc, revealed, revealsNew, settingsOf, type DoorState, type GridSettings, type LocSettings, type Op, type PlayState, type Shape, type ShapeKind, type Token, type TokenKind } from './state';
import { BLOCKS, bitAt, computeVision, gridDistance, interiorGrid, measure, OPAQUE, rulerLabel, SIGHT, SOLID, UNKNOWN, type Eye, type TacticalGrid } from './vision';

export type Tool = 'select' | 'token' | 'measure' | 'shape' | 'fog';

/** A location's grid on the map: squares to world ft and back. */
export interface Place {
  loc: string;
  toWorld(gx: number, gy: number): [number, number];
  toGrid(wx: number, wy: number): [number, number];
  /** The grid's x axis on screen (radians). */
  rotation: number;
  /** The building or site and level (null on the surface); `layer` when it is the one open. */
  it: Interior | null;
  level: number;
  layer: InteriorLayer | null;
  /** A city's sewers: one place in world squares across its sections. */
  sewers?: Sewers;
}

/** The section of a building, site or the sewers under a square: its plan and level, where
 * its grid starts in the place's, and the location its doors' states are kept under. */
interface Section {
  it: Interior;
  level: number;
  ox: number;
  oy: number;
  doorLoc: string;
}

const SURFACE: Place = {
  loc: 'surface',
  toWorld: (gx, gy) => [gx * 5, gy * 5],
  toGrid: (wx, wy) => [wx / 5, wy / 5],
  rotation: 0,
  it: null,
  level: 0,
  layer: null,
};

/** A level of a building or site as a place (its grid laid on the world). */
function interiorPlace(it: Interior, level: number, layer: InteriorLayer | null): Place {
  const [ox, oy] = it.origin;
  const [ax, ay] = it.axis;
  const [bx, by] = it.across;
  return {
    loc: locKey(it.id, level),
    toWorld: (gx, gy) => [ox + (ax * gx + bx * gy) * 5, oy + (ay * gx + by * gy) * 5],
    toGrid: (wx, wy) => [((wx - ox) * ax + (wy - oy) * ay) / 5, ((wx - ox) * bx + (wy - oy) * by) / 5],
    rotation: Math.atan2(ay, ax),
    it,
    level,
    layer,
  };
}

/** The floor square nearest a grid point on a level (its centre), or null if none. */
function nearestFloor(it: Interior, level: number, gx: number, gy: number): [number, number] | null {
  const cells = it.levels[level]?.cells;
  if (!cells) return null;
  let best: [number, number] | null = null;
  let bestD = Infinity;
  for (let k = 0; k < cells.length; k++) {
    if (cells[k] < 0) continue;
    const [i, j] = [k % it.nx, Math.floor(k / it.nx)];
    const d = (i + 0.5 - gx) ** 2 + (j + 0.5 - gy) ** 2;
    if (d < bestD) [best, bestD] = [[i + 0.5, j + 0.5], d];
  }
  return best;
}

/** Whether a grid point is on a level's floor. */
function onFloor(it: Interior, level: number, gx: number, gy: number): boolean {
  const [i, j] = [Math.floor(gx), Math.floor(gy)];
  return i >= 0 && j >= 0 && i < it.nx && j < it.ny && (it.levels[level]?.cells[j * it.nx + i] ?? -1) >= 0;
}

/** Feature kinds that are places of interest (their labels are pins like businesses'). */
const POI_KINDS = new Set(['waystation', 'ruin', 'tower', 'cave', 'mine', 'lava_tube', 'camp', 'entrance', 'building', 'place']);

/** Pings: the DM's orange, the players' blue. */
const PING_DM = 0xf97316;
const PING_PLAYER = 0x38bdf8;

export const TOKEN_COLORS = [0x8b2e2e, 0x2f5d8a, 0x3f7a3a, 0x7a5a2a, 0x6b3f7a, 0x2a6b6b, 0xc2a14a, 0x4a4a4a, 0xd8d1c2];
export const SHAPE_COLORS = [0xf97316, 0x3b82f6, 0x22c55e, 0xa855f7, 0xeab308];

/** A token drawn in the view: in the location in view, outside a building one is inside
 * (`outside`, draggable in), or inside a building while the view is outside (`inside`, the
 * DM's faint marker, draggable out). */
export interface ShownToken {
  t: Token;
  place: Place;
  where: 'here' | 'outside' | 'inside';
}

export class PlayController implements PointerTool {
  state: PlayState = emptyState();
  /** Play mode on (the DM's choice; the player window follows it). */
  on = false;
  tool: Tool = 'select';
  /** The DM's selected tokens: dragged together, and brought along to wherever the DM goes
   * next (into a building or underground, up or down a floor, out to the street). */
  readonly selection = new Set<string>();
  /** The tokens being dragged by the DM (drawn where the pointer puts them). */
  readonly dragging = new Set<string>();
  /** A selection box being dragged out (world ft: x0, y0, x1, y1). */
  box: [number, number, number, number] | null = null;
  ruler: Ruler | null = null;
  draftShape: Shape | null = null;
  pings: { loc: string; x: number; y: number; t: number; color: number }[] = [];
  /** The fog brush under the pointer. */
  brush: { x: number; y: number; r: number; reveal: boolean } | null = null;
  /** What the characters see now (the DM works it out and sends it). */
  vision: Vision | null = null;
  /** New tokens. */
  draft: { name: string; kind: TokenKind; size: number; color: number; vision: boolean } = { name: 'Token', kind: 'character', size: 1, color: TOKEN_COLORS[0], vision: false };
  shapeKind: ShapeKind = 'circle';
  shapeColor = SHAPE_COLORS[0];
  fogReveal = true;
  /** Fog brush radius (squares). */
  fogRadius = 3;
  /** The fog tool reveals or hides whole rooms (inside). */
  fogRooms = false;
  /** The player window's camera follows the DM's, and can't be moved there. */
  follow = false;
  lock = false;
  /** A player window is open. */
  players = false;
  /** Player window: the DM shows business and place pins (the players see those in sight). */
  dmPlaces = false;
  /** Something the panel shows changed. */
  onChange: () => void = () => {};
  /** Something went wrong that the DM should hear about (a picture refused). */
  onError: (text: string) => void = () => {};
  /** Player window: show this world (resolves once it is loaded). */
  onWorld: (w: WorldFile) => Promise<void> = async () => {};
  /** Player window: the world's edits changed. */
  onEdits: (w: WorldFile) => void = () => {};

  readonly layer: PlayLayer;
  private readonly channel: PlayChannel;
  private world: WorldFile | null = null;
  private worldHash = '';
  private saveTimer: ReturnType<typeof setTimeout> | null = null;
  private placeCache: { key: string; place: Place } | null = null;
  /** Buildings and sites tokens stand in (null: none by that id), and their places. */
  private readonly interiors = new Map<string, Interior | null>();
  private readonly places = new Map<string, Place>();
  /** Sites designed by hand, as last seen (JSON by id). */
  private designsSeen: Record<string, { ref: unknown; json: string }> = {};
  private lastLoc = '';
  /** The place of the frame before (left when the location changes). */
  private prevPlace: Place | null = null;
  /** Ways out to the street from the sewers round the view (world ft), kept while there. */
  private sewerExits: [number, number][] = [];
  private sewerExitsAt = 0;
  private visionDirty = true;
  private visionAt = 0;
  private visionChunks = -1;
  private camKey = '';
  private camAt = 0;
  private camTarget: CameraMsg | null = null;
  private sceneSeq = 0;
  private sentPlaces: boolean | null = null;
  /** Sewer sections' grids (by section id), kept until one of their doors changes. */
  private readonly sewerGrids = new Map<string, { it: Interior; grid: TacticalGrid; doors: number }>();
  /** Changes to doors, by the location they are in. */
  private readonly doorChanges = new Map<string, number>();
  private readonly concealFn = (f: Feature) => this.conceal(f);
  private press: {
    kind: 'door' | 'token' | 'place' | 'measure' | 'shape' | 'fog' | 'ping' | 'box' | 'none';
    x: number;
    y: number;
    door?: { k: number; loc: string };
    /** Dragged tokens and their offsets from the pointer (world ft). */
    group?: { id: string; dx: number; dy: number }[];
    /** World point pressed (a selection box's corner). */
    wx?: number;
    wy?: number;
    pts?: number[];
  } | null = null;
  private rulerTimer: ReturnType<typeof setTimeout> | null = null;
  private lastDragSend = 0;
  private lastRulerSend = 0;

  constructor(
    readonly view: MapView,
    readonly role: Role,
  ) {
    this.layer = new PlayLayer(view, this);
    this.channel = new PlayChannel(role);
    this.channel.onMessage = (m) => void this.receive(m);
    view.onFrame = (now) => this.frame(now);
    if (role === 'dm') {
      // (A players' window left open while this one reloaded makes itself known.)
      this.channel.send({ t: 'dm' });
    } else {
      view.tool = this;
      window.addEventListener('beforeunload', () => this.channel.send({ t: 'bye' }));
    }
  }

  // ---------------------------------------------------------------------------------------
  // Where play is.

  /** The location in view: the building or site level open, else the surface. */
  get place(): Place | null {
    const v = this.view;
    if (!v.geom) return null;
    const layer = v.interior;
    if (!layer) return SURFACE;
    // The sewer level of a city: the whole network is one place (world squares, like the surface).
    const sewers = v.sewers;
    if (sewers && layer.currentLevel === layer.interior.entry_level) {
      const key = `w:${sewers.layout}`;
      if (this.placeCache?.key === key && this.placeCache.place.sewers === sewers) return this.placeCache.place;
      const place: Place = { ...SURFACE, loc: key, it: null, level: layer.currentLevel, layer: null, sewers };
      this.placeCache = { key, place };
      return place;
    }
    const key = locKey(layer.interior.id, layer.currentLevel);
    if (this.placeCache?.key === key && this.placeCache.place.layer === layer) return this.placeCache.place;
    const place = interiorPlace(layer.interior, layer.currentLevel, layer);
    this.placeCache = { key, place };
    this.interiors.set(layer.interior.id, layer.interior);
    return place;
  }

  /** Any location's place: the one in view, the surface, or a building known here (asked
   * for, and null meanwhile). */
  placeOf(loc: string): Place | null {
    const here = this.place;
    if (here?.loc === loc) return here;
    if (loc === 'surface') return SURFACE;
    const cached = this.places.get(loc);
    if (cached) return cached;
    const { interior, level } = parseLoc(loc);
    if (!interior) return null;
    const it = this.interiors.get(interior);
    if (it === undefined) {
      void this.fetchInterior(interior);
      return null;
    }
    if (!it) return null;
    const place = interiorPlace(it, level, null);
    this.places.set(loc, place);
    return place;
  }

  private async fetchInterior(id: string): Promise<Interior | null> {
    if (this.interiors.has(id)) return this.interiors.get(id) ?? null;
    this.interiors.set(id, null);
    const it = await this.view.gen.interior(id);
    this.interiors.set(id, it);
    return it;
  }

  /** The location's name, for the panel. */
  get placeName(): string {
    const it = this.view.interior;
    if (!it) return 'The surface';
    const lv = it.interior.levels[it.currentLevel];
    return `${it.interior.name ?? it.interior.function} · ${lv?.name ?? ''}`;
  }

  /** The one token selected (its editor shows), if exactly one is. */
  get selected(): string | null {
    return this.selection.size === 1 ? [...this.selection][0] : null;
  }

  /** Select just this token (none: nothing). */
  select(id: string | null) {
    this.selection.clear();
    if (id) this.selection.add(id);
    this.onChange();
  }

  get scene(): Scene {
    const it = this.view.interior;
    return { interior: it?.interior.id ?? null, level: it?.currentLevel ?? 0 };
  }

  door(loc: string, index: number): DoorState {
    return this.state.doors[`${loc}#${index}`] ?? {};
  }

  /** The tactical grid of the location in view. */
  gridOf(place: Place | null): TacticalGrid | null {
    if (!place) return null;
    if (place.sewers) return this.sewerGrid(place.sewers);
    if (!place.it) return this.view.battle?.grid() ?? null;
    return interiorGrid(place.it, place.level, (k) => !this.door(place.loc, k).open);
  }

  /**
   * A city's sewers as one grid of world squares: each section's sewer level from its own plan
   * (sections not loaded stop sight), without the walls the plans draw along sections' shared
   * edges where the tunnel runs on across.
   */
  private sewerGrid(sewers: Sewers): TacticalGrid {
    const N = SECTION_N;
    // The section of the square asked last (most questions are about the same one).
    let [lx, ly] = [NaN, NaN];
    let last: TacticalGrid | null | undefined;
    const section = (i: number, j: number): TacticalGrid | null | undefined => {
      const [sx, sy] = [Math.floor(i / N), Math.floor(j / N)];
      if (sx !== lx || sy !== ly) [lx, ly, last] = [sx, sy, this.sectionGrid(sewers, sx, sy)];
      return last;
    };
    const flags = (i: number, j: number) => {
      const g = section(i, j);
      return g === undefined ? UNKNOWN : g === null ? SOLID | OPAQUE | BLOCKS : g.flags(i - lx * N, j - ly * N);
    };
    const floor = (i: number, j: number) => !(flags(i, j) & (SOLID | UNKNOWN));
    return {
      flags,
      elev: () => NaN,
      wallW: (i, j) => {
        if (((i % N) + N) % N === 0) return !(floor(i - 1, j) && floor(i, j));
        const g = section(i, j);
        return !!g && g.wallW(i - lx * N, j - ly * N);
      },
      wallN: (i, j) => {
        if (((j % N) + N) % N === 0) return !(floor(i, j - 1) && floor(i, j));
        const g = section(i, j);
        return !!g && g.wallN(i - lx * N, j - ly * N);
      },
      bounds: null,
      // Whole runs of a row from each section.
      fill: (x0, y0, w, h, out, elev) => {
        elev.fill(NaN);
        const row = new Uint8Array(N);
        const dummy = new Float32Array(N);
        for (let j = 0; j < h; j++) {
          const gj = y0 + j;
          for (let i = 0; i < w; ) {
            const gi = x0 + i;
            const sx = Math.floor(gi / N);
            const run = Math.min(w - i, N - (gi - sx * N));
            const g = this.sectionGrid(sewers, sx, Math.floor(gj / N));
            const o = j * w + i;
            if (g === undefined) out.fill(UNKNOWN, o, o + run);
            else if (g === null) out.fill(SOLID | OPAQUE | BLOCKS, o, o + run);
            else {
              g.fill!(gi - sx * N, gj - Math.floor(gj / N) * N, run, 1, row, dummy);
              out.set(row.subarray(0, run), o);
            }
            i += run;
          }
        }
      },
    };
  }

  /** A sewer section's grid (kept while its plan and the doors stay the same); null where no
   * sewers run, undefined while not loaded. */
  private sectionGrid(sewers: Sewers, sx: number, sy: number): TacticalGrid | null | undefined {
    const it = sewers.interiorAt(sx, sy);
    if (!it) return it;
    const loc = locKey(it.id, it.entry_level);
    const doors = this.doorChanges.get(loc) ?? 0;
    const have = this.sewerGrids.get(it.id);
    if (have && have.it === it && have.doors === doors) return have.grid;
    const grid = interiorGrid(it, it.entry_level, (k) => !this.door(loc, k).open);
    this.sewerGrids.set(it.id, { it, grid, doors });
    if (this.sewerGrids.size > 128) this.sewerGrids.delete(this.sewerGrids.keys().next().value!);
    return grid;
  }

  /** The building, site or sewer section under a square of a place. */
  private sectionAt(place: Place, gx: number, gy: number): Section | null {
    if (place.sewers) {
      const layer = place.sewers.layerAt(...place.toWorld(gx, gy));
      if (!layer) return null;
      const it = layer.interior;
      return { it, level: it.entry_level, ox: Math.round(it.origin[0] / 5), oy: Math.round(it.origin[1] / 5), doorLoc: locKey(it.id, it.entry_level) };
    }
    return place.it ? { it: place.it, level: place.level, ox: 0, oy: 0, doorLoc: place.loc } : null;
  }

  measureRuler(r: Ruler) {
    return measure(r.a, r.b, this.state.grid);
  }

  /** What the ruler reads: across, and (where the ground is known at both ends) up or down
   * and straight from end to end. */
  rulerText(r: Ruler): string {
    const g = this.state.grid;
    const place = this.place;
    const here = place?.loc === r.loc ? place : null;
    // Zoomed out: straight-line distance, heights from the map's terrain at the two points.
    const height = (p: [number, number]) => (!here ? null : r.free ? (here.it || here.sewers ? null : this.view.tiles?.heightAt(...here.toWorld(p[0], p[1])) ?? null) : this.elevationAt(here, p[0], p[1]));
    const [za, zb] = [height(r.a), height(r.b)];
    const [dx, dy] = [r.b[0] - r.a[0], r.b[1] - r.a[1]];
    const dist = r.free ? Math.hypot(dx, dy) : gridDistance(dx, dy, g.measure);
    return rulerLabel(dist, za === null || zb === null ? null : zb - za, g);
  }

  /** The height (ft above the sea) of the ground or floor under a square of the place in view:
   * terrain, a wall walk, a building's or site's floor; null where it isn't loaded or there
   * is no floor. */
  elevationAt(place: Place, i: number, j: number): number | null {
    const [wx, wy] = place.toWorld(i + 0.5, j + 0.5);
    if (place.sewers) return place.sewers.layerAt(wx, wy)?.inspect(wx, wy)?.elevationFt ?? null;
    if (place.it) return place.layer?.inspect(wx, wy)?.elevationFt ?? null;
    return this.view.battle?.inspect(wx, wy)?.elevationFt ?? null;
  }

  /** The tokens to draw: those here; inside a building, those outside it (the players see
   * only their own characters there); outside, the DM's markers of those inside buildings. */
  tokensInView(): ShownToken[] {
    const here = this.place;
    if (!here) return [];
    const player = this.role === 'player';
    const building = !!here.it && !isUnderground(here.it.id);
    const out: ShownToken[] = [];
    for (const t of Object.values(this.state.tokens)) {
      if (t.loc === here.loc) {
        out.push({ t, place: here, where: 'here' });
      } else if (building && t.loc === 'surface') {
        if (!player || (t.vision && !t.hidden)) out.push({ t, place: SURFACE, where: 'outside' });
      } else if (!here.it && !here.sewers && !player && t.loc !== 'surface' && !isUnderground(t.loc)) {
        const place = this.placeOf(t.loc);
        if (place) out.push({ t, place, where: 'inside' });
      }
    }
    return out;
  }

  /** The levels of the building a token stands in (for moving it up and down). */
  levelsOf(t: Token): { name: string; i: number }[] {
    const { interior } = parseLoc(t.loc);
    const it = interior ? this.interiors.get(interior) : null;
    return it ? it.levels.map((l, i) => ({ name: l.name, i })) : [];
  }

  // ---------------------------------------------------------------------------------------
  // The DM's side.

  /** A world was loaded (DM): its saved session comes back; the player window follows. */
  async setWorld(world: WorldFile, hash: string) {
    this.world = forPlayers(world);
    if (hash !== this.worldHash) {
      this.flushSave();
      const before = this.fogKeys();
      this.worldHash = hash;
      this.state = await loadSession(hash);
      this.interiors.clear();
      this.places.clear();
      this.layer.fogChanged([...before, ...this.fogKeys()]);
      this.selection.clear();
      this.vision = null;
      this.ruler = null;
      this.visionDirty = true;
      // The buildings tokens stand in, for the DM's markers.
      for (const t of Object.values(this.state.tokens)) {
        const id = parseLoc(t.loc).interior;
        if (id) void this.fetchInterior(id);
      }
    }
    this.snapshot();
    this.onChange();
  }

  /** The world's edits changed (DM): the player window takes them too. */
  setEdits(world: WorldFile) {
    this.world = forPlayers(world);
    this.forgetRedesigned(world);
    // To the players' window, if one is open: once a burst of changes settles (a big world's
    // edits take a while to send).
    if (!this.players) return;
    if (this.editsTimer) clearTimeout(this.editsTimer);
    this.editsTimer = setTimeout(() => {
      this.editsTimer = null;
      if (this.players && this.world) this.channel.send({ t: 'edits', world: this.world });
    }, 250);
  }
  private editsTimer: ReturnType<typeof setTimeout> | null = null;

  /** Sites designed anew since the last edits: their walls and levels are fetched again. */
  private forgetRedesigned(world: WorldFile) {
    const now = world.edits?.designs ?? {};
    for (const id of new Set([...Object.keys(this.designsSeen), ...Object.keys(now)])) {
      // (The same design object is the same design; one received anew is compared as JSON.)
      const was = this.designsSeen[id];
      if (was && was.ref === now[id]) continue;
      const json = now[id] ? JSON.stringify(now[id]) : '';
      if (was?.json === json || (!was && !json)) {
        if (was) was.ref = now[id];
        continue;
      }
      if (json) this.designsSeen[id] = { ref: now[id], json };
      else delete this.designsSeen[id];
      this.interiors.delete(id);
      for (const loc of [...this.places.keys()]) if (parseLoc(loc).interior === id) this.places.delete(loc);
      this.placeCache = null;
      this.visionDirty = true;
    }
  }

  setOn(on: boolean) {
    this.on = on;
    this.view.tool = on || this.role === 'player' ? this : null;
    this.view.interiorStyle.doors = on ? (loc, k) => this.door(loc, k) : null;
    this.view.redrawInterior();
    if (!on) {
      this.brush = null;
      this.ruler = null;
      this.draftShape = null;
    }
    this.visionDirty = true;
    if (this.role === 'dm') this.channel.send({ t: 'mode', on });
    this.onChange();
  }

  /** Change the play state (DM): here, in the player window, and in the saved session. */
  dispatch(op: Op) {
    this.applyOp(op);
    this.channel.send({ t: 'op', op });
    this.scheduleSave();
    this.onChange();
  }

  private applyOp(op: Op) {
    const keys = apply(this.state, op);
    this.layer.fogChanged(keys);
    if (op.t === 'door') {
      const loc = op.key.slice(0, op.key.lastIndexOf('#'));
      this.doorChanges.set(loc, (this.doorChanges.get(loc) ?? 0) + 1);
      this.view.redrawInterior();
    }
    if (op.t === 'reset') this.selection.clear();
    if (op.t === 'untoken') this.selection.delete(op.id);
    if (op.t !== 'fog' && op.t !== 'fogbits' && op.t !== 'fogreset' && op.t !== 'shape' && op.t !== 'unshape' && op.t !== 'grid') this.visionDirty = true;
  }

  setTool(t: Tool) {
    this.tool = t;
    if (t !== 'fog') this.brush = null;
    this.onChange();
  }

  /** A new token at a world point (or the middle of the view); `as`: a character with this
   * name and picture (an NPC from the notebook) rather than the token being drafted. */
  addToken(wx?: number, wy?: number, as?: { name: string; image?: string }): Token | null {
    const place = this.place;
    if (!place) return null;
    const [gx, gy] = place.toGrid(wx ?? this.view.cam.cx, wy ?? this.view.cam.cy);
    const d = as ? { ...this.draft, name: as.name, kind: 'character' as TokenKind, vision: false } : this.draft;
    const base = d.name.trim() || 'Token';
    const same = Object.values(this.state.tokens).filter((t) => t.name.replace(/ \d+$/, '') === base).length;
    const t: Token = { id: newId(), loc: place.loc, x: 0, y: 0, name: same ? `${base} ${same + 1}` : base, kind: d.kind, size: d.kind === 'light' ? 1 : d.size, color: d.color };
    if (as?.image) t.image = as.image;
    if (d.kind === 'light') t.light = 4;
    else if (d.vision) t.vision = true;
    [t.x, t.y] = snap(gx, gy, t.size);
    this.dispatch({ t: 'token', token: t });
    this.select(t.id);
    return t;
  }

  updateToken(id: string, patch: Partial<Token>) {
    const t = this.state.tokens[id];
    if (!t) return;
    const next = { ...t, ...patch };
    if (patch.size) [next.x, next.y] = snap(next.x, next.y, next.size);
    this.dispatch({ t: 'token', token: next });
  }

  removeToken(id: string) {
    this.dispatch({ t: 'untoken', id });
  }

  /** Remove every selected token. */
  removeSelected() {
    for (const id of [...this.selection]) this.dispatch({ t: 'untoken', id });
  }

  /** An NPC from the notebook as a token here (with their portrait); null when there is no
   * place to put one. */
  async dropNpc(name: string, portrait: Blob | null, wx?: number, wy?: number): Promise<Token | null> {
    if (!this.place) return null;
    const image = portrait ? await putImage(portrait) : undefined;
    if (image) this.layer.imageChanged(image);
    return this.addToken(wx, wy, { name, image });
  }

  async setTokenImage(id: string, file: File | null) {
    if (!file) return this.updateToken(id, { image: undefined });
    let image: string;
    try {
      image = await putImage(file);
    } catch (e) {
      return this.onError(e instanceof Error ? e.message : String(e));
    }
    this.layer.imageChanged(image);
    this.updateToken(id, { image });
  }

  /** Move a token in a building to another of its levels (onto the floor nearest where it
   * stands; stairs line up between floors). */
  tokenToLevel(id: string, level: number) {
    const t = this.state.tokens[id];
    const name = t ? parseLoc(t.loc).interior : null;
    const it = name ? this.interiors.get(name) : null;
    if (!t || !it) return;
    const sq = nearestFloor(it, level, t.x, t.y);
    if (!sq) return;
    const [x, y] = snap(sq[0], sq[1], t.size);
    this.dispatch({ t: 'token', token: { ...t, loc: locKey(it.id, level), x, y } });
  }

  settingsHere(): LocSettings {
    return settingsOf(this.state, this.place?.loc ?? 'surface');
  }

  setSettingsHere(patch: Partial<LocSettings>) {
    const loc = this.place?.loc ?? 'surface';
    this.dispatch({ t: 'loc', loc, settings: { ...settingsOf(this.state, loc), ...patch } });
  }

  setGrid(patch: Partial<GridSettings>) {
    this.dispatch({ t: 'grid', grid: { ...this.state.grid, ...patch } });
  }

  /** Reveal every square of the level in view (inside only), or forget all that is known
   * here. */
  fogAll(reveal: boolean) {
    const place = this.place;
    if (!place) return;
    if (!reveal) {
      this.dispatch({ t: 'fogreset', loc: place.loc });
      return;
    }
    // (In the sewers, the section in the middle of the view.)
    const sec = this.sectionAt(place, ...place.toGrid(this.view.cam.cx, this.view.cam.cy));
    if (!sec) return;
    const { nx, ny } = sec.it;
    this.dispatch({ t: 'fogbits', loc: place.loc, reveal, x0: sec.ox, y0: sec.oy, w: nx, h: ny, bits: new Uint8Array(nx * ny).fill(1) });
  }

  clearShapes() {
    const loc = this.place?.loc;
    if (loc) this.dispatch({ t: 'unshape', id: '*', loc });
  }

  resetSession() {
    this.dispatch({ t: 'reset' });
  }

  openPlayerWindow() {
    if (!this.world) return;
    // (The DM's window keeps its world's link in its address: the players' window opens on it
    // if no DM window answers.)
    const url = `${import.meta.env.BASE_URL}player.html${location.hash}`;
    window.open(url, 'fantasy-map-player', 'popup=yes,width=1280,height=800');
  }

  /** The players' view goes to the DM's once (`follow` and `lock`: and keeps up with it). */
  sendCamera(force = false) {
    const cam = this.view.cam;
    const [x0, y0, x1, y1] = cam.viewRect();
    const camera: CameraMsg = { cx: cam.cx, cy: cam.cy, w: x1 - x0, h: y1 - y0, follow: this.follow, lock: this.lock };
    const key = `${camera.cx.toFixed(1)},${camera.cy.toFixed(1)},${camera.w.toFixed(1)},${this.follow},${this.lock}`;
    if (!force && key === this.camKey) return;
    this.camKey = key;
    this.channel.send({ t: 'camera', camera });
  }

  setFollow(follow: boolean, lock: boolean) {
    this.follow = follow || lock;
    this.lock = lock;
    this.sendCamera(true);
    this.onChange();
  }

  private snapshot() {
    if (this.role !== 'dm' || !this.world) return;
    const cam = this.view.cam;
    const [x0, y0, x1, y1] = cam.viewRect();
    this.sentPlaces = this.view.places;
    this.channel.send({
      t: 'snapshot',
      world: this.world,
      state: this.state,
      scene: this.scene,
      camera: { cx: cam.cx, cy: cam.cy, w: x1 - x0, h: y1 - y0, follow: this.follow, lock: this.lock },
      vision: this.vision,
      on: this.on,
      places: this.view.places,
    });
  }

  private fogKeys(): string[] {
    return Object.entries(this.state.fog).flatMap(([loc, chunks]) => Object.keys(chunks).map((k) => `${loc}|${k}`));
  }

  private scheduleSave() {
    if (this.role !== 'dm' || !this.worldHash) return;
    if (this.saveTimer) clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => this.flushSave(), 700);
  }

  private flushSave() {
    if (!this.saveTimer) return;
    clearTimeout(this.saveTimer);
    this.saveTimer = null;
    void saveSession(this.worldHash, this.state);
  }

  // ---------------------------------------------------------------------------------------
  // Pointer input (the map hands it over first while play mode is on).

  down(x: number, y: number, e: PointerEvent): boolean {
    const place = this.place;
    if (!place) return false;
    const [gx, gy] = place.toGrid(x, y);
    // Alt-click or middle button: a ping (the players' window too, with the right button).
    if ((e.button === 0 && e.altKey) || e.button === 1 || (this.role === 'player' && e.button === 2)) {
      this.press = { kind: 'ping', x: gx, y: gy };
      return true;
    }
    if (this.role === 'player' || !this.on) return false;
    if (e.button === 2) return this.rightClick(place, gx, gy);
    if (e.button !== 0) return false;
    switch (this.tool) {
      case 'select': {
        // A token first (one may stand by a door), then a door.
        const hit = this.tokenAt(x, y);
        if (hit) {
          const id = hit.t.id;
          // Shift-click: add to or take from the selection.
          if (e.shiftKey) {
            if (this.selection.has(id)) this.selection.delete(id);
            else this.selection.add(id);
            this.press = { kind: 'none', x: gx, y: gy };
            this.onChange();
            return true;
          }
          if (!this.selection.has(id)) {
            this.selection.clear();
            this.selection.add(id);
          }
          // Drag every selected token in view together.
          const group = this.tokensInView()
            .filter((s) => this.selection.has(s.t.id))
            .map((s) => {
              const [wx, wy] = s.place.toWorld(s.t.x, s.t.y);
              return { id: s.t.id, dx: wx - x, dy: wy - y };
            });
          this.dragging.clear();
          for (const g of group) this.dragging.add(g.id);
          this.press = { kind: 'token', x: gx, y: gy, group };
          this.onChange();
          return true;
        }
        const door = this.doorAt(place, gx, gy);
        if (door) {
          this.press = { kind: 'door', x: gx, y: gy, door: { k: door.k, loc: door.sec.doorLoc } };
          return true;
        }
        // Shift-drag on open ground: a selection box.
        if (e.shiftKey) {
          this.press = { kind: 'box', x: gx, y: gy, wx: x, wy: y };
          this.box = [x, y, x, y];
          return true;
        }
        if (this.selection.size) this.select(null);
        return false;
      }
      case 'token':
        this.press = { kind: 'place', x: gx, y: gy };
        return true;
      case 'measure': {
        // Zoomed out (squares a few pixels wide or less), a straight line between points.
        const free = 5 * this.view.cam.ppf < 4;
        const sq: [number, number] = free ? [gx, gy] : [Math.floor(gx), Math.floor(gy)];
        if (this.rulerTimer) clearTimeout(this.rulerTimer);
        this.ruler = { loc: place.loc, a: sq, b: sq, free };
        this.press = { kind: 'measure', x: gx, y: gy };
        this.sendRuler(true);
        return true;
      }
      case 'shape':
        this.draftShape = { id: newId(), loc: place.loc, kind: this.shapeKind, x: Math.round(gx), y: Math.round(gy), dx: 0, dy: 0, color: this.shapeColor };
        this.press = { kind: 'shape', x: gx, y: gy };
        return true;
      case 'fog': {
        const reveal = this.fogReveal !== e.shiftKey;
        if (this.fogRooms && (place.it || place.sewers)) {
          this.fogRoom(place, gx, gy, reveal);
          return true;
        }
        this.press = { kind: 'fog', x: gx, y: gy, pts: [gx, gy] };
        this.dispatch({ t: 'fog', loc: place.loc, reveal, pts: [gx, gy], r: this.fogRadius });
        return true;
      }
    }
  }

  move(x: number, y: number, e: PointerEvent) {
    const place = this.place;
    const p = this.press;
    if (!place || !p) return;
    const [gx, gy] = place.toGrid(x, y);
    if (p.kind === 'token' && p.group) {
      // Live, for the DM (each in its own grid); the players see them move a few times a second.
      const now = performance.now();
      const send = now - this.lastDragSend > 120;
      if (send) this.lastDragSend = now;
      for (const g of p.group) {
        const t = this.state.tokens[g.id];
        const tp = t && this.placeOf(t.loc);
        if (!t || !tp) continue;
        [t.x, t.y] = tp.toGrid(x + g.dx, y + g.dy);
        if (send) this.channel.send({ t: 'op', op: { t: 'token', token: { ...t } } });
      }
      this.visionDirty = true;
    } else if (p.kind === 'box') {
      this.box = [p.wx!, p.wy!, x, y];
    } else if (p.kind === 'measure' && this.ruler) {
      const b: [number, number] = this.ruler.free ? [gx, gy] : [Math.floor(gx), Math.floor(gy)];
      if (b[0] !== this.ruler.b[0] || b[1] !== this.ruler.b[1]) {
        this.ruler = { ...this.ruler, b };
        this.sendRuler(false);
      }
    } else if (p.kind === 'shape' && this.draftShape) {
      this.draftShape = aim(this.draftShape, gx, gy);
    } else if (p.kind === 'fog') {
      const pts = p.pts!;
      const [lx, ly] = [pts[pts.length - 2], pts[pts.length - 1]];
      const step = Math.max(0.5, this.fogRadius / 2);
      const d = Math.hypot(gx - lx, gy - ly);
      if (d < step) return;
      const add: number[] = [];
      for (let k = 1; k <= Math.ceil(d / step); k++) {
        const f = Math.min(1, (k * step) / d);
        add.push(lx + (gx - lx) * f, ly + (gy - ly) * f);
      }
      pts.push(...add);
      this.dispatch({ t: 'fog', loc: place.loc, reveal: this.fogReveal !== e.shiftKey, pts: add, r: this.fogRadius });
      this.brush = { x: gx, y: gy, r: this.fogRadius, reveal: this.fogReveal !== e.shiftKey };
    }
  }

  up(x: number, y: number) {
    const place = this.place;
    const p = this.press;
    this.press = null;
    if (!place || !p) return;
    const [gx, gy] = place.toGrid(x, y);
    const still = Math.hypot(gx - p.x, gy - p.y) < 0.6;
    switch (p.kind) {
      case 'ping':
        if (still) this.ping(place.loc, gx, gy);
        break;
      case 'door':
        if (still && p.door) {
          const st = this.door(p.door.loc, p.door.k);
          this.dispatch({ t: 'door', key: `${p.door.loc}#${p.door.k}`, state: { ...st, open: !st.open, found: st.found || !st.open } });
        }
        break;
      case 'token': {
        this.dragging.clear();
        for (const g of p.group ?? []) {
          const t = this.state.tokens[g.id];
          if (t) this.drop(t, place);
        }
        break;
      }
      case 'box': {
        // Every token drawn inside it joins the selection.
        const [x0, y0, x1, y1] = this.box ?? [0, 0, 0, 0];
        this.box = null;
        for (const s of this.tokensInView()) {
          const [wx, wy] = s.place.toWorld(s.t.x, s.t.y);
          if (wx >= Math.min(x0, x1) && wx <= Math.max(x0, x1) && wy >= Math.min(y0, y1) && wy <= Math.max(y0, y1)) this.selection.add(s.t.id);
        }
        this.onChange();
        break;
      }
      case 'none':
        break;
      case 'place':
        if (still) this.addToken(...place.toWorld(gx, gy));
        break;
      case 'measure':
        // The ruler stays a moment, for the table to read.
        if (this.rulerTimer) clearTimeout(this.rulerTimer);
        this.rulerTimer = setTimeout(() => {
          this.ruler = null;
          this.sendRuler(true);
        }, 4000);
        break;
      case 'shape': {
        const d = this.draftShape;
        this.draftShape = null;
        if (d && Math.hypot(d.dx, d.dy) >= 1) this.dispatch({ t: 'shape', shape: d });
        break;
      }
      case 'fog':
        break;
    }
  }

  /**
   * A dragged token lands where it is dropped. Inside a building: on this level's floor it is
   * in, off it (from any floor) out on the street. Outside: on a building it goes in, onto the
   * ground floor (the DM sees a marker of it on the roof); a marker dragged off its building's
   * floor comes out (or into the building it lands on).
   */
  private drop(t: Token, here: Place) {
    const tp = this.placeOf(t.loc) ?? here;
    const [wx, wy] = tp.toWorld(t.x, t.y);
    const it = here.it;
    const out = () => {
      const [x, y] = snap(wx / 5, wy / 5, t.size);
      const outside: Token = { ...t, loc: 'surface', x, y };
      this.dispatch({ t: 'token', token: outside });
      return outside;
    };
    if (it && !isUnderground(it.id)) {
      const [gx, gy] = here.toGrid(wx, wy);
      if (onFloor(it, here.level, gx, gy)) {
        const [x, y] = snap(gx, gy, t.size);
        this.dispatch({ t: 'token', token: { ...t, loc: here.loc, x, y } });
      } else {
        out();
      }
      return;
    }
    if (!it && !here.sewers && tp.it && !isUnderground(tp.it.id)) {
      const [gx, gy] = tp.toGrid(wx, wy);
      if (onFloor(tp.it, tp.level, gx, gy)) {
        const [x, y] = snap(gx, gy, t.size);
        this.dispatch({ t: 'token', token: { ...t, x, y } });
      } else {
        void this.enterAt(out(), wx, wy);
      }
      return;
    }
    const [x, y] = snap(t.x, t.y, t.size);
    const landed = { ...t, x, y };
    this.dispatch({ t: 'token', token: landed });
    if (!it && !here.sewers && t.loc === 'surface') void this.enterAt(landed, wx, wy);
  }

  /** A token dropped on a building on the surface goes inside, if it can be entered. */
  private async enterAt(t: Token, wx: number, wy: number) {
    const hit = await this.view.gen.query(wx, wy);
    if (hit?.kind !== 'building') return;
    const it = await this.fetchInterior(hit.id);
    const now = this.state.tokens[t.id];
    // Moved again (or removed) meanwhile: leave it.
    if (!it || !now || now.loc !== t.loc || now.x !== t.x || now.y !== t.y) return;
    const level = it.entry_level;
    const [gx, gy] = interiorPlace(it, level, null).toGrid(wx, wy);
    const sq = onFloor(it, level, gx, gy) ? [gx, gy] : nearestFloor(it, level, gx, gy);
    if (!sq) return;
    const [x, y] = snap(sq[0], sq[1], t.size);
    this.dispatch({ t: 'token', token: { ...now, loc: locKey(it.id, level), x, y } });
  }

  cancel() {
    for (const id of this.dragging) {
      const t = this.state.tokens[id];
      if (t) [t.x, t.y] = snap(t.x, t.y, t.size);
    }
    this.press = null;
    this.dragging.clear();
    this.box = null;
    this.draftShape = null;
  }

  hover(x: number, y: number) {
    const place = this.place;
    if (!place || this.role !== 'dm' || !this.on || this.tool !== 'fog' || (this.fogRooms && (place.it || place.sewers))) {
      this.brush = null;
      return;
    }
    const [gx, gy] = place.toGrid(x, y);
    this.brush = { x: gx, y: gy, r: this.fogRadius, reveal: this.fogReveal };
  }

  /** Right button: a secret door found (or not), a shape taken away. */
  private rightClick(place: Place, gx: number, gy: number): boolean {
    const door = this.doorAt(place, gx, gy);
    if (door && door.sec.it.levels[door.sec.level].doors[door.k].kind === 'secret') {
      const st = this.door(door.sec.doorLoc, door.k);
      this.dispatch({ t: 'door', key: `${door.sec.doorLoc}#${door.k}`, state: { ...st, found: !st.found, open: st.found ? false : st.open } });
      return true;
    }
    const shape = Object.values(this.state.shapes)
      .filter((s) => s.loc === place.loc)
      .reverse()
      .find((s) => shapeHas(s, gx, gy));
    if (shape) {
      this.dispatch({ t: 'unshape', id: shape.id });
      return true;
    }
    return false;
  }

  private ping(loc: string, x: number, y: number) {
    this.pings.push({ loc, x, y, t: performance.now(), color: this.role === 'dm' ? PING_DM : PING_PLAYER });
    this.channel.send({ t: 'ping', loc, x, y });
  }

  private sendRuler(force: boolean) {
    const now = performance.now();
    if (!force && now - this.lastRulerSend < 60) return;
    this.lastRulerSend = now;
    this.channel.send({ t: 'ruler', ruler: this.ruler });
  }

  /** The token under a world point that can be picked up: here, outside the building in
   * view, or (the DM's markers on the street) inside a building. */
  private tokenAt(wx: number, wy: number): ShownToken | null {
    let best: ShownToken | null = null;
    let bestD = Infinity;
    for (const s of this.tokensInView()) {
      const [gx, gy] = s.place.toGrid(wx, wy);
      const d = Math.hypot(s.t.x - gx, s.t.y - gy);
      // Here first, at the same distance (then those outside, then markers of those inside).
      const rank = d + (s.where === 'here' ? 0 : s.where === 'outside' ? 0.01 : 0.02);
      if (d <= Math.max(0.5, s.t.size / 2) && rank < bestD) [best, bestD] = [s, rank];
    }
    return best;
  }

  /** The door whose middle is near a grid point (inside), and the section it is in. */
  private doorAt(place: Place, gx: number, gy: number): { k: number; sec: Section } | null {
    const sec = this.sectionAt(place, gx, gy);
    if (!sec) return null;
    const doors = sec.it.levels[sec.level]?.doors ?? [];
    let best = -1;
    let bestD = 0.5;
    doors.forEach((d, k) => {
      const dist = Math.hypot((d.a[0] + d.b[0]) / 2 + sec.ox - gx, (d.a[1] + d.b[1]) / 2 + sec.oy - gy);
      if (dist < bestD) [best, bestD] = [k, dist];
    });
    return best >= 0 ? { k: best, sec } : null;
  }

  /** Reveal or hide the room under a point, with its walls (in the sewers, within its section). */
  private fogRoom(place: Place, gx: number, gy: number, reveal: boolean) {
    const sec = this.sectionAt(place, gx, gy);
    if (!sec) return;
    const { it, ox, oy } = sec;
    const lv = it.levels[sec.level];
    const { nx, ny } = it;
    const [i, j] = [Math.floor(gx) - ox, Math.floor(gy) - oy];
    if (i < 0 || j < 0 || i >= nx || j >= ny) return;
    const room = lv.cells[j * nx + i];
    if (room < 0) return;
    const bits = new Uint8Array(nx * ny);
    for (let k = 0; k < nx * ny; k++) {
      if (lv.cells[k] === room) {
        bits[k] = 1;
        continue;
      }
      if (lv.cells[k] >= 0 || !reveal) continue;
      // Rock or outside next to the room: its wall.
      const [a, b] = [k % nx, Math.floor(k / nx)];
      for (let dy = -1; dy <= 1 && !bits[k]; dy++) for (let dx = -1; dx <= 1; dx++) if (a + dx >= 0 && b + dy >= 0 && a + dx < nx && b + dy < ny && lv.cells[(b + dy) * nx + a + dx] === room) bits[k] = 1;
    }
    this.dispatch({ t: 'fogbits', loc: place.loc, reveal, x0: ox, y0: oy, w: nx, h: ny, bits });
  }

  // ---------------------------------------------------------------------------------------
  // Taking tokens along.

  /**
   * The DM went from one place to another: the selected tokens that were there come along.
   * Through a way (a door, stairs, a grate, a trapdoor, an entrance clicked on the map) they
   * gather round where it comes out; up or down a floor of the same building (the levels
   * list) each keeps its spot; otherwise they gather at the nearest way in or out. Returns how
   * many came.
   */
  private carry(from: Place, to: Place): number {
    const going = [...this.selection].map((id) => this.state.tokens[id]).filter((t): t is Token => !!t && t.loc === from.loc);
    if (!going.length) return 0;
    const now = performance.now();
    const way = this.view.lastWay && now - this.view.lastWay.t < 4000 ? this.view.lastWay : null;
    const same = siteOf(from.loc) === siteOf(to.loc);
    const grid = this.gridOf(to);
    const taken = new Set(Object.values(this.state.tokens).filter((t) => t.loc === to.loc && !this.selection.has(t.id)).map((t) => `${Math.floor(t.x)},${Math.floor(t.y)}`));
    const place = (t: Token, sq: [number, number]) => {
      const [x, y] = snap(sq[0] + 0.5, sq[1] + 0.5, t.size);
      this.dispatch({ t: 'token', token: { ...t, loc: to.loc, x, y } });
    };
    if (same && !way) {
      for (const t of going) {
        const [gx, gy] = to.toGrid(...from.toWorld(t.x, t.y));
        const [sq] = gather(grid, [Math.floor(gx), Math.floor(gy)], 1, taken);
        taken.add(`${sq[0]},${sq[1]}`);
        place(t, sq);
      }
    } else {
      let at: [number, number];
      if (way?.kind === 'site' && this.view.lastArrival && now - this.view.lastArrival.t < 4000) at = this.view.lastArrival.at;
      else if (way && (same || !to.it)) at = way.at;
      else if (!to.it && !to.sewers) at = nearest(this.exitsOf(from), way?.at ?? [this.view.cam.cx, this.view.cam.cy]) ?? [this.view.cam.cx, this.view.cam.cy];
      else at = nearest(this.entriesOf(to), way?.at ?? [this.view.cam.cx, this.view.cam.cy]) ?? way?.at ?? [this.view.cam.cx, this.view.cam.cy];
      const [gx, gy] = to.toGrid(...at);
      gather(grid, [Math.floor(gx), Math.floor(gy)], going.length, taken).forEach((sq, k) => place(going[k], sq));
    }
    this.selection.clear();
    for (const t of going) this.selection.add(t.id);
    this.onChange();
    return going.length;
  }

  /** Ways in from the street to a place (world ft): its ways up to the surface and its front
   * and back doors (inside them). */
  private entriesOf(to: Place): [number, number][] {
    if (to.sewers) return this.exitsOfSewers(to.sewers);
    const it = to.it;
    if (!it) return [];
    const lv = it.levels[to.level];
    const out: [number, number][] = lv.furniture.filter((f) => f.kind === 'exit').map((f) => to.toWorld(f.x + 0.5, f.y + 0.5));
    for (const d of lv.doors) {
      if (d.kind !== 'front' && d.kind !== 'back') continue;
      const [ox, oy] = this.outsideOf(it, to.level, d);
      const [mx, my] = [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
      out.push(to.toWorld(mx * 2 - ox, my * 2 - oy));
    }
    return out;
  }

  /** Ways out to the street from a place (world ft): its ways up and the street outside its
   * front and back doors. */
  private exitsOf(from: Place): [number, number][] {
    if (from.sewers) return this.sewerExits;
    const it = from.it;
    if (!it) return [];
    // Whichever floor the DM left from, the ways out are on the ground floor.
    const level = it.entry_level;
    const lv = it.levels[level];
    const ground = interiorPlace(it, level, null);
    const out: [number, number][] = lv.furniture.filter((f) => f.kind === 'exit').map((f) => ground.toWorld(f.x + 0.5, f.y + 0.5));
    for (const d of lv.doors) if (d.kind === 'front' || d.kind === 'back') out.push(ground.toWorld(...this.outsideOf(it, level, d)));
    return out;
  }

  /** The sewers' ways up to the street in the sections round the view (world ft). */
  private exitsOfSewers(sewers: Sewers): [number, number][] {
    const out: [number, number][] = [];
    const [cx, cy] = [this.view.cam.cx, this.view.cam.cy];
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const layer = sewers.layerAt(cx + dx * 480, cy + dy * 480);
        if (!layer) continue;
        const it = layer.interior;
        for (const f of it.levels[it.entry_level].furniture) if (f.kind === 'exit') out.push(layer.toWorld(f.x + 0.5, f.y + 0.5));
      }
    }
    return out;
  }

  /** The middle of the square outside a door in a building's outer wall (grid units). */
  private outsideOf(it: Interior, level: number, d: Interior['levels'][number]['doors'][number]): [number, number] {
    const [mx, my] = [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
    const [nx, ny] = d.a[0] === d.b[0] ? [1, 0] : [0, 1];
    const s = onFloor(it, level, mx + nx * 0.5, my + ny * 0.5) ? -1 : 1;
    return [mx + nx * 0.5 * s, my + ny * 0.5 * s];
  }

  // ---------------------------------------------------------------------------------------
  // Every frame.

  private frame(now: number) {
    this.pings = this.pings.filter((p) => now - p.t < 1700);
    const place = this.place;
    const loc = place?.loc ?? '';
    if (this.role === 'dm') {
      if (loc !== this.lastLoc) {
        this.lastLoc = loc;
        // The selected tokens come along; any others are let go.
        const carried = this.prevPlace && place ? this.carry(this.prevPlace, place) : 0;
        if (!carried) this.selection.clear();
        this.visionDirty = true;
        this.channel.send({ t: 'scene', scene: this.scene });
        this.onChange();
      }
      if (this.view.places !== this.sentPlaces) {
        this.sentPlaces = this.view.places;
        this.channel.send({ t: 'places', on: this.view.places });
      }
      if (this.on && (this.follow || this.lock) && now - this.camAt > 50) {
        this.camAt = now;
        this.sendCamera();
      }
      this.updateVision(now, place);
      // Where the sewers come up to the street, round the view (they are gone once left).
      if (place?.sewers && now - this.sewerExitsAt > 500) {
        this.sewerExitsAt = now;
        this.sewerExits = this.exitsOfSewers(place.sewers);
      }
      this.prevPlace = place;
    } else {
      this.followCamera();
      const labels = this.view.labels;
      if (labels && labels.conceal !== this.concealFn) labels.conceal = this.concealFn;
    }
    this.layer.update(now);
  }

  /** Work out what the characters see (when something that matters changed). */
  private updateVision(now: number, place: Place | null) {
    if (!this.on || !place) return;
    const s = this.state;
    if (!s.los) {
      if (this.vision) {
        this.vision = null;
        this.channel.send({ t: 'vision', vision: null });
      }
      return;
    }
    // On the surface, battlemap chunks arriving show more of what blocks sight (sight stops
    // where nothing is loaded; chunks dropped when zoomed out don't take sight away).
    const chunks = place.sewers ? place.sewers.loaded : place.it ? -1 : (this.view.battle?.stats.chunks ?? 0);
    if (chunks > this.visionChunks) this.visionDirty = true;
    this.visionChunks = chunks;
    if (!this.visionDirty || now - this.visionAt < (this.dragging.size ? 120 : 60)) return;
    this.visionAt = now;
    const grid = this.gridOf(place);
    if (!grid) return;
    this.visionDirty = false;
    const settings = settingsOf(s, place.loc);
    const here = Object.values(s.tokens).filter((t) => t.loc === place.loc);
    const eyes: Eye[] = here.filter((t) => t.kind === 'character' && t.vision).map((t) => ({ x: t.x, y: t.y, above: 5, range: SIGHT }));
    const lights: Eye[] = here.filter((t) => (t.light ?? 0) > 0).map((t) => ({ x: t.x, y: t.y, above: 5, range: t.light! }));
    const v = computeVision(grid, eyes, lights, settings.dark);
    this.vision = { loc: place.loc, x0: v.x0, y0: v.y0, w: v.w, h: v.h, bits: v.bits };
    this.channel.send({ t: 'vision', vision: this.vision });
    // What they have seen, they know: it stays clear (dimmed under fog) once out of sight.
    if (v.w && revealsNew(s, place.loc, v.x0, v.y0, v.w, v.h, v.bits)) {
      this.dispatch({ t: 'fogbits', loc: place.loc, reveal: true, x0: v.x0, y0: v.y0, w: v.w, h: v.h, bits: v.bits });
    }
  }

  /**
   * Players' window: labels of businesses and places of interest show only while the DM shows
   * place pins, and then only where the players can see (in sight with line of sight on; else
   * revealed under fog; else anywhere).
   */
  private conceal(f: Feature): boolean {
    if (!POI_KINDS.has(f.kind) && !f.id.startsWith('c:')) return false;
    if (!this.dmPlaces) return true;
    if (!this.on) return false;
    const fog = settingsOf(this.state, 'surface').fog;
    if (!this.state.los && !fog) return false;
    // A building's pin is at its middle, which sight never reaches (walls stop it): any
    // square of the building or just round it will do.
    const size = this.view.labels?.placeHit(f.id)?.size_ft ?? 20;
    const r = Math.min(8, Math.max(1, size / 10 + 1));
    const [ci, cj] = [f.x / 5, f.y / 5];
    const known = (i: number, j: number) => (this.state.los ? this.vision?.loc === 'surface' && bitAt(this.vision, i, j) : revealed(this.state, 'surface', i, j));
    for (let j = Math.floor(cj - r); j <= Math.floor(cj + r); j++) {
      for (let i = Math.floor(ci - r); i <= Math.floor(ci + r); i++) {
        if ((i + 0.5 - ci) ** 2 + (j + 0.5 - cj) ** 2 <= r * r && known(i, j)) return false;
      }
    }
    return true;
  }

  // ---------------------------------------------------------------------------------------
  // The player window's side.

  /** Say hello to the DM's window (it answers with everything). */
  connect() {
    this.channel.send({ t: 'hello' });
  }

  private async receive(m: Msg) {
    if (this.role === 'dm') {
      if (m.t === 'hello') {
        this.players = true;
        this.snapshot();
        this.onChange();
      } else if (m.t === 'bye') {
        this.players = false;
        this.onChange();
      } else if (m.t === 'ping') {
        this.pings.push({ loc: m.loc, x: m.x, y: m.y, t: performance.now(), color: PING_PLAYER });
      }
      return;
    }
    switch (m.t) {
      case 'snapshot': {
        const before = this.fogKeys();
        await this.onWorld(m.world);
        this.state = normalize(m.state);
        this.layer.fogChanged([...before, ...this.fogKeys()]);
        this.vision = m.vision;
        this.setOn(m.on);
        this.setDmPlaces(m.places);
        this.camTarget = m.camera && (m.camera.follow || m.camera.lock) ? m.camera : null;
        this.view.locked = !!this.camTarget?.lock;
        if (m.camera) this.jumpCamera(m.camera);
        await this.applyScene(m.scene);
        break;
      }
      case 'edits':
        this.forgetRedesigned(m.world);
        this.onEdits(m.world);
        break;
      case 'op':
        this.applyOp(m.op);
        break;
      case 'scene':
        await this.applyScene(m.scene);
        break;
      case 'camera':
        this.camTarget = m.camera.follow || m.camera.lock ? m.camera : null;
        this.view.locked = m.camera.lock;
        if (!this.camTarget) this.flyCamera(m.camera);
        break;
      case 'ping':
        this.pings.push({ loc: m.loc, x: m.x, y: m.y, t: performance.now(), color: PING_DM });
        break;
      case 'ruler':
        this.ruler = m.ruler;
        break;
      case 'vision':
        this.vision = m.vision;
        break;
      case 'mode':
        this.setOn(m.on);
        break;
      case 'places':
        this.setDmPlaces(m.on);
        break;
      case 'dm':
        this.connect();
        break;
      case 'bye':
        break;
    }
  }

  /** Player window: the DM's place pins on or off (pins show in the players' sight only). */
  setDmPlaces(on: boolean) {
    this.dmPlaces = on;
    this.view.setPlaces(on);
  }

  /** Go where the DM is: out to the surface, or into the same building or site and level. */
  private async applyScene(sc: Scene) {
    const seq = ++this.sceneSeq;
    const v = this.view;
    if (!v.geom) return;
    const cur = v.interior?.interior.id ?? null;
    if (sc.interior === null) {
      if (cur) v.exitBuilding();
      return;
    }
    if (cur !== sc.interior) {
      // Following the DM's camera: no framing flight of its own.
      const ok = await v.enterBuilding(sc.interior, this.camTarget ? { level: sc.level } : undefined);
      if (!ok || seq !== this.sceneSeq) return;
    }
    if (v.interior && v.interior.currentLevel !== sc.level) v.setInteriorLevel(sc.level);
  }

  private zoomFor(c: CameraMsg): number {
    const cam = this.view.cam;
    return Math.log2(Math.min(cam.width / Math.max(1e-6, c.w), cam.height / Math.max(1e-6, c.h)));
  }

  private jumpCamera(c: CameraMsg) {
    this.view.cam.set({ cx: c.cx, cy: c.cy, zoom: this.zoomFor(c) });
  }

  private flyCamera(c: CameraMsg) {
    this.view.flyTo({ cx: c.cx, cy: c.cy, zoom: this.zoomFor(c) });
  }

  /** Ease toward the DM's view (a jump if it is far). */
  private followCamera() {
    const c = this.camTarget;
    if (!c) return;
    const cam = this.view.cam;
    const zoom = this.zoomFor(c);
    const span = cam.width / cam.ppf;
    if (Math.abs(zoom - cam.zoom) > 3 || Math.hypot(c.cx - cam.cx, c.cy - cam.cy) > span * 3) {
      cam.set({ cx: c.cx, cy: c.cy, zoom });
      return;
    }
    // Close enough: settle exactly (a camera that never stops moving holds back detail).
    if (Math.abs(zoom - cam.zoom) < 0.002 && Math.hypot(c.cx - cam.cx, c.cy - cam.cy) * cam.ppf < 0.5) {
      if (cam.cx !== c.cx || cam.cy !== c.cy || cam.zoom !== zoom) cam.set({ cx: c.cx, cy: c.cy, zoom });
      return;
    }
    const k = 0.25;
    cam.set({ cx: cam.cx + (c.cx - cam.cx) * k, cy: cam.cy + (c.cy - cam.cy) * k, zoom: cam.zoom + (zoom - cam.zoom) * k });
  }
}

/** A token's centre snapped to the grid for its size (half-square tokens to a square's middle). */
export function snap(x: number, y: number, size: number): [number, number] {
  const n = Math.max(1, Math.round(size));
  return [Math.round(x - n / 2) + n / 2, Math.round(y - n / 2) + n / 2];
}

/** A shape aimed at a grid point: whole squares (a rectangle to the nearest corner). */
function aim(s: Shape, gx: number, gy: number): Shape {
  const [dx, dy] = [gx - s.x, gy - s.y];
  if (s.kind === 'rect') return { ...s, dx: Math.round(dx) || 1, dy: Math.round(dy) || 1 };
  const d = Math.hypot(dx, dy);
  const len = Math.max(1, Math.round(d));
  const [ux, uy] = d > 1e-6 ? [dx / d, dy / d] : [1, 0];
  return { ...s, dx: ux * len, dy: uy * len };
}

/** Whether a grid point is on a shape (for right-click removal). */
function shapeHas(s: Shape, gx: number, gy: number): boolean {
  const [vx, vy] = [gx - s.x, gy - s.y];
  const len = Math.hypot(s.dx, s.dy);
  if (Math.hypot(vx, vy) < 0.7) return true;
  switch (s.kind) {
    case 'circle':
      return vx * vx + vy * vy <= len * len;
    case 'rect':
      return vx * Math.sign(s.dx) >= 0 && vy * Math.sign(s.dy) >= 0 && Math.abs(vx) <= Math.abs(s.dx) && Math.abs(vy) <= Math.abs(s.dy);
    case 'line':
    case 'cone': {
      const along = (vx * s.dx + vy * s.dy) / len;
      const across = Math.abs(vx * s.dy - vy * s.dx) / len;
      return along > 0 && along <= len && across <= (s.kind === 'cone' ? along / 2 : 0.5);
    }
  }
}

/** Which building, site or city's sewers a location is in (the street for the surface). */
function siteOf(loc: string): string {
  if (loc === 'surface') return loc;
  const sewer = /^w:(\d+)/.exec(loc);
  if (sewer) return `w:${sewer[1]}`;
  return parseLoc(loc).interior ?? loc;
}

/** The point nearest `p` (null if none). */
function nearest(pts: [number, number][], p: [number, number]): [number, number] | null {
  let best: [number, number] | null = null;
  let bestD = Infinity;
  for (const q of pts) {
    const d = Math.hypot(q[0] - p[0], q[1] - p[1]);
    if (d < bestD) [best, bestD] = [q, d];
  }
  return best;
}

/**
 * `n` free squares round a square, nearest first: walkable (no rock, wall mass or blocking
 * object; walls and shut doors not passed through), not `taken`. Where there aren't enough,
 * the start square again.
 */
function gather(grid: TacticalGrid | null, start: [number, number], n: number, taken: Set<string>): [number, number][] {
  const out: [number, number][] = [];
  const ok = (i: number, j: number) => !grid || !(grid.flags(i, j) & (SOLID | OPAQUE | BLOCKS));
  const seen = new Set<string>([`${start[0]},${start[1]}`]);
  const queue: [number, number][] = [start];
  for (let q = 0; q < queue.length && out.length < n && q < 4000; q++) {
    const [i, j] = queue[q];
    if (ok(i, j) && !taken.has(`${i},${j}`)) out.push([i, j]);
    for (const [di, dj] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const [a, b] = [i + di, j + dj];
      const key = `${a},${b}`;
      if (seen.has(key)) continue;
      // Not through a wall or a shut door.
      if (grid && (di === 1 ? grid.wallW(a, j) : di === -1 ? grid.wallW(i, j) : dj === 1 ? grid.wallN(i, b) : grid.wallN(i, j))) continue;
      seen.add(key);
      // Through rock and walls no further (but a start on rock may step off it).
      if (!ok(a, b) && ok(i, j)) continue;
      queue.push([a, b]);
    }
  }
  while (out.length < n) out.push(start);
  return out;
}

/** The world as the players' window gets it: without the DM's notes, NPCs and plots. */
function forPlayers(w: WorldFile): WorldFile {
  if (!w.edits) return w;
  const { notes: _, npcs: __, plots: ___, ...edits } = w.edits;
  return { ...w, edits };
}
