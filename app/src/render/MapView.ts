// The map: Pixi application, camera, input (wheel/drag/pinch/keys with inertia), fly-to,
// and per-frame styling (zoom-adaptive hillshade and contour interval).
import { Application, Container, Graphics } from 'pixi.js';
import { GenClient } from '../gen/client';
import { type Clear, type Created, type Edits, type Feature, type Placed, type GenStats, type Geom, type Interior, type Overlay, type Rect, type WantTile, type WorldFile, WORKS_MARGIN_FT } from '../gen/protocol';
import { BattlemapLayer, type SquareInfo } from './BattlemapLayer';
import { Camera, flyPath, type CameraState } from './camera';
import { InteriorLayer, warmUnderground, type InteriorStyle, type Move } from './InteriorLayer';
import { Labels } from './Labels';
import { SketchLayer } from './SketchLayer';
import { prepareField, SECTION_FT, Sewers } from './Sewers';
import type { Sketcher } from '../editor/sketcher';
import { RIVER_Q, TileLayer, type LayerStats } from './TileLayer';

export interface InteriorState {
  id: string;
  name: string;
  levels: { name: string; z: number }[];
  level: number;
}

export interface HudState {
  zoom: number;
  ftPerPx: number;
  /** World position at the centre of the view (ft). */
  center: { x: number; y: number };
  tier: string;
  cursor: { x: number; y: number; elev: number | null; square: SquareInfo | null } | null;
  fps: number;
  layer: LayerStats;
  gen: GenStats | null;
  status: string;
  /** 0..1 while the continent is generating, else null. */
  progress: number | null;
}

const STAGES: Record<string, string> = {
  plates: 'Moving tectonic plates',
  erosion: 'Eroding mountains',
  terrain: 'Shaping terrain',
  volcanoes: 'Raising volcanoes',
  climate: 'Simulating climate',
  rivers: 'Tracing rivers and lakes',
  biomes: 'Growing biomes',
  names: 'Naming the land',
  done: 'Finishing',
};
/** Rough share of generation time per stage, for the progress bar. */
const STAGE_WEIGHT: [string, number][] = [
  ['plates', 0.2],
  ['erosion', 0.55],
  ['terrain', 0.03],
  ['volcanoes', 0.0],
  ['climate', 0.04],
  ['rivers', 0.05],
  ['biomes', 0.04],
  ['names', 0.09],
  ['done', 0.0],
];

/** Tile levels that draw site layouts (`payload::SITE_MIN_LEVEL`). */
const SITE_MIN_LEVEL = 9;
/** How far a created site's layout reaches (`town::sites::SITE_REACH_FT`), with a margin. */
const SITE_REACH_FT = 260 + 100;

/** How far (ft) from its point a created site's layout reaches, with a margin (as
 * `worldgen::world::Created::reach`: a castle's or wall's farthest corner and its towers). */
export function siteReach(c: Created): number {
  const pts = c.kind === 'castle' ? c.poly : c.kind === 'wall' ? c.pts : null;
  if (!pts?.length) return SITE_REACH_FT;
  return Math.max(...pts.map((p) => Math.hypot(p[0] - c.x, p[1] - c.y))) + WORKS_MARGIN_FT + 100;
}

/** A wall's length (ft). */
export function wallLength(pts: [number, number][], closed: boolean): number {
  let s = 0;
  const n = pts.length;
  for (let i = 0; i < (closed ? n : n - 1); i++) s += Math.hypot(pts[(i + 1) % n][0] - pts[i][0], pts[(i + 1) % n][1] - pts[i][1]);
  return s;
}

/** What a created site is (as `worldgen::agent::created_detail`). */
function createdDetail(c: Created): string {
  const words = (s: string) => s.replace(/_/g, ' ');
  if (c.kind === 'building') {
    const floors = c.floors && `${c.floors} floor${c.floors === 1 ? '' : 's'}`;
    const opts = [c.func && words(c.func), floors, c.structure === 'ruin' && 'ruined', c.roof && (c.roof === 'hip' ? 'hip roof' : c.roof), c.tint && `${c.tint} roof`].filter(Boolean);
    return `created building${opts.length ? ` (${opts.join(', ')})` : ''}`;
  }
  if (c.kind === 'castle') {
    const opts = [c.structure === 'ruin' && 'ruined', c.keep === false && 'no keep', c.yard_buildings === false && 'an empty yard', c.gate != null && `gate on side ${c.gate}`].filter(Boolean);
    return `created castle${opts.length ? ` (${opts.join(', ')})` : ''}`;
  }
  if (c.kind === 'wall') {
    const g = c.gates?.length ?? 0;
    return `created wall (${c.closed ? 'a ring, ' : ''}${Math.round(wallLength(c.pts ?? [], !!c.closed))} ft${g === 1 ? ', a gate' : g ? `, ${g} gates` : ''})`;
  }
  const opts = [c.theme && words(c.theme), c.size, c.levels && `${c.levels} level${c.levels === 1 ? '' : 's'}`].filter(Boolean);
  return `created ${words(c.kind)}${c.under ? ` over a ${words(c.under)}` : ''}${opts.length ? ` (${opts.join(', ')})` : ''}`;
}

/** A created site as a named feature (as `worldgen::agent::features` lists it). */
function createdFeature(c: Created): Feature {
  const extent = c.kind === 'building' ? 0.5 * 5280 : c.kind === 'castle' || c.kind === 'wall' ? Math.max(2 * (siteReach(c) - 100), 0.5 * 5280) : 2 * 5280;
  return { id: c.id, kind: c.kind, name: c.name, x: c.x, y: c.y, angle: 0, extent_ft: extent, detail: createdDetail(c) };
}

/** What a created site's layout depends on (not its name, but a drawn building's: it is the
 * building's own). */
const siteKey = (c: Created) => JSON.stringify({ ...c, name: c.kind === 'building' || c.kind === 'castle' ? c.name : null });

/** Screenshots for agents (`render_view`): pixels. */
const CAPTURE_W = 1536;
const CAPTURE_H = 1024;

/** A tool that takes pointer input on the map (play mode): it can claim a pointer on its way
 * down (no pan, no pick then) and follow the pointer about while nothing is pressed. */
export interface PointerTool {
  down(x: number, y: number, e: PointerEvent): boolean;
  move(x: number, y: number, e: PointerEvent): void;
  up(x: number, y: number, e: PointerEvent): void;
  cancel(): void;
  hover(x: number, y: number): void;
  /** Draw what the tool shows over the map (screen px), every frame. */
  draw?(g: Graphics, cam: Camera): void;
  /** A double click on the map: true if the tool took it (else it goes into a building). */
  dblclick?(x: number, y: number): boolean;
}

const TIERS: [number, string][] = [
  [1500, 'Continent'],
  [150, 'Region'],
  [20, 'Local'],
  [0.3, 'Site'],
  [0, 'Battlemap'],
];

export class MapView {
  readonly app = new Application();
  readonly cam = new Camera();
  readonly gen = new GenClient();
  geom: Geom | null = null;
  tiles: TileLayer | null = null;
  battle: BattlemapLayer | null = null;
  labels: Labels | null = null;
  /** Named features: the generated ones, then created sites. */
  overlay: Overlay | null = null;
  private baseOverlay: Overlay | null = null;
  /** The world file's edits as applied (labels, created sites, generators). */
  edits: Edits = {};
  private editEpoch = 0;
  /** Objects already drawn by `previewObjects` (by place), not drawn again when they arrive. */
  private previewed = new Set<string>();
  /** Sketch mode: the strokes and preview on the map, and the editor pointer input goes to. */
  sketchLayer: SketchLayer | null = null;
  private sketcher: Sketcher | null = null;
  /** The pointer drawing a stroke. */
  private sketching: number | null = null;
  /** Tiles and chunks asked for in the last frame. */
  private lastWants = 0;
  /** Frames drawn so far. */
  private frameCount = 0;
  /** The canvas size while capturing a screenshot. */
  private captureSize: [number, number] | null = null;
  /** The building being viewed inside, if any. */
  interior: InteriorLayer | null = null;
  /** In a city's sewers: the whole network round the view (`interior` is then the section
   * in the middle of it). */
  sewers: Sewers | null = null;
  /** The zoom-out limit before going down into the sewers (they are drawn near the view only). */
  private sewerMinZoom: number | null = null;
  /** Roof fade: the ground floor of the hovered building at close battlemap zoom. */
  private peek: { id: string; layer: InteriorLayer; out: boolean } | null = null;
  private peekBusy = false;
  private lastPeekCheck = 0;
  private peekCache = new Map<string, Interior | null>();
  /** Called when entering, changing level in or leaving a building. */
  onInterior: (s: InteriorState | null) => void = () => {};
  /** Renamed features and districts (the world file's edits), for labels that arrive later. */
  renames: Record<string, string> = {};
  /** Settlements (layout order) and which have had their districts asked for. */
  private towns: { x: number; y: number; r: number; index: number }[] = [];
  private districtsAsked = new Set<number>();
  private lastDistrictCheck = 0;
  /** Business pins ("Places"): shown or not, the area asked for last, and a pending ask. */
  places = false;
  onPlaces: (on: boolean) => void = () => {};
  private placesRect: [number, number, number, number] | null = null;
  /** Bumped when buildings are changed by hand: an answer asked for before is dropped. */
  private placesEpoch = 0;
  private placesBusy = false;
  onHud: (s: HudState) => void = () => {};
  /** A click (not a drag) on the map: world and screen position. */
  onPick: (x: number, y: number, sx: number, sy: number) => void = () => {};
  /** Inside a site, a click: the ways on from that square (empty elsewhere) and the screen
   * position, for a menu. */
  onMoves: (moves: Move[], sx: number, sy: number) => void = () => {};
  private tap: { x: number; y: number; t: number } | null = null;
  /** When set, called each frame to drive the camera (benchmarks); input is ignored. */
  driver: ((now: number) => CameraState | null) | null = null;
  /** Map input off (the player window while the DM holds its camera). */
  locked = false;
  /** Play mode: tokens and templates (under the labels), fog, rulers and pings (over all). */
  readonly playUnder = new Container();
  readonly playOver = new Container();
  /** Takes pointer input before panning and picking (play mode tools). */
  tool: PointerTool | null = null;
  /** What the tool draws over everything (`PointerTool.draw`). */
  private readonly toolLayer = new Graphics();
  private toolDrawn = false;
  private toolPointer: number | null = null;
  /** Called every frame after the camera moved and the map updated. */
  onFrame: ((now: number) => void) | null = null;
  /** How interiors draw doors and what they keep from the players. */
  readonly interiorStyle: InteriorStyle;
  /** The way last taken (a door, stairs, a grate, an entrance clicked on the map; world ft),
   * and where the way into another site came out: play mode brings tokens along there. */
  lastWay: { at: [number, number]; kind: string; t: number } | null = null;
  lastArrival: { at: [number, number]; t: number } | null = null;

  private el!: HTMLElement;
  private vel = { x: 0, y: 0, z: 0 };
  private drag: { id: number; x: number; y: number; t: number } | null = null;
  private pointers = new Map<number, { x: number; y: number }>();
  private lastTap: { x: number; y: number; t: number } | null = null;
  private lastPointerType = 'mouse';
  private pinchDist = 0;
  private fly: { path: ReturnType<typeof flyPath>; start: number; ms: number } | null = null;
  private cursor: { sx: number; sy: number } | null = null;
  private frameTimes: number[] = [];
  private lastFrame = performance.now();
  /** The camera at the last frame, and when it last moved (or was dragged). */
  private lastCam: [number, number, number] = [0, 0, 0];
  private movedAt = performance.now();
  /** Per-frame CPU timings (ms) of the last frame, for the benchmark's slow-frame breakdown. */
  prof = { tiles: 0, labels: 0, play: 0, uploads: 0, render: 0, gpu: 0, frame: 0, ev: '' };
  /** CPU time (ms) of play mode in the last frame (part of `labels` in the profile). */
  private playMs = 0;
  /** Worker messages (tiles, chunks) received since the last frame. */
  private received = 0;
  /** CPU time of the last Pixi render (ms). */
  private renderMs = 0;
  /** GPU time of a recent render (ms; with ?gpu=1 only). */
  private gpuMs = 0;
  /** Smoothed camera velocity (ft/ms) for predictive tile prefetch. */
  private camVel: [number, number] = [0, 0];
  private prevCenter: [number, number] = [0, 0];
  private lastHud = 0;
  private genStats: GenStats | null = null;
  private status = 'starting';
  /** How long the last continent took to generate (ms, in the worker that made it). */
  t0Ms = 0;
  private exagFactor = 1;
  private progress: number | null = null;
  private lastWantKey = '';

  /** Its own keys (zoom, pan, grid, floors…); the map's window turns them off and drives the
   * map through the methods below from its shortcuts. */
  private readonly ownKeys: boolean;
  /** The battlemap grid shown (kept across worlds). */
  private gridOn = true;

  /** `player`: the players' window at a local table (DM-only things are left out). */
  constructor(
    readonly role: 'dm' | 'player' = 'dm',
    opts: { keys?: boolean } = {},
  ) {
    this.interiorStyle = { player: role === 'player', doors: null };
    this.ownKeys = opts.keys ?? true;
  }

  /** Input is off: a script drives the camera, or the DM holds it. */
  get inputOff(): boolean {
    return !!this.driver || this.locked;
  }

  /** Zoom by a nudge (log2 px per ft a frame, eased like the wheel). */
  zoomBy(dz: number) {
    if (this.inputOff) return;
    this.fly = null;
    this.vel.z += dz;
  }

  panBy(dx: number, dy: number) {
    if (this.inputOff) return;
    this.fly = null;
    this.cam.panPixels(dx, dy);
  }

  /** Fly out to the whole map. */
  fitWorld() {
    const g = this.geom;
    if (!g || this.inputOff) return;
    this.flyTo({ cx: g.map_w_ft / 2, cy: g.map_h_ft / 2, zoom: Math.max(this.cam.minZoom, this.cam.fitZoom(g.map_w_ft, g.map_h_ft)) });
  }

  /** Up (1) or down (-1) a floor inside a building or site. */
  levelBy(d: number) {
    if (this.interior) this.setInteriorLevel(this.interior.currentLevel + d);
  }

  get grid(): boolean {
    return this.gridOn;
  }

  setGrid(on: boolean) {
    this.gridOn = on;
    this.battle?.setGrid(on);
  }

  /** Where interiors go in the stage: above the map and battlemap, under tokens and labels. */
  private get interiorIndex(): number {
    return this.playUnder.parent === this.app.stage ? this.app.stage.getChildIndex(this.playUnder) : this.app.stage.children.length;
  }

  async mount(el: HTMLElement) {
    this.el = el;
    await this.app.init({
      resizeTo: el,
      background: '#8aa0a8',
      antialias: false,
      preference: 'webgl',
      resolution: Math.min(window.devicePixelRatio || 1, 2),
      autoDensity: true,
    });
    el.appendChild(this.app.canvas);
    InteriorLayer.renderer = this.app.renderer;
    // Time Pixi's render (batching, lazy Graphics tessellation, texture uploads, GL calls that
    // wait on the GPU) for the frame profile.
    // With ?gpu=1, also GPU time per render (timer queries; results arrive a few frames
    // later). Opt-in: the disjoint check is a synchronous round trip to the GPU process.
    const renderer = this.app.renderer;
    const render = renderer.render.bind(renderer);
    const gl = (renderer as unknown as { gl?: WebGL2RenderingContext }).gl;
    const timer = gl && new URLSearchParams(location.search).has('gpu') ? gl.getExtension('EXT_disjoint_timer_query_webgl2') : null;
    const queries: WebGLQuery[] = [];
    renderer.render = ((...args: Parameters<typeof render>) => {
      const t = performance.now();
      const q = timer && gl && queries.length < 6 ? gl.createQuery() : null;
      if (q) gl!.beginQuery(timer!.TIME_ELAPSED_EXT, q);
      const out = render(...args);
      if (q) {
        gl!.endQuery(timer!.TIME_ELAPSED_EXT);
        queries.push(q);
      }
      while (gl && queries.length && gl.getQueryParameter(queries[0], gl.QUERY_RESULT_AVAILABLE)) {
        const done = queries.shift()!;
        if (!gl.getParameter(timer!.GPU_DISJOINT_EXT)) this.gpuMs = gl.getQueryParameter(done, gl.QUERY_RESULT) / 1e6;
        gl.deleteQuery(done);
      }
      this.renderMs = performance.now() - t;
      return out;
    }) as typeof renderer.render;
    // Compile shaders and draw the sprite atlas while the continent generates (the main
    // thread is idle then), instead of stalling on the first tiles and battlemaps after it.
    const stub = { max_level: 0, domain_ft: 1, sea_level_ft: 0, map_w_ft: 1, map_h_ft: 1 } as Geom;
    const warmTiles = new TileLayer(stub, this.gen);
    warmTiles.warmup(this.app.renderer);
    warmTiles.destroy();
    warmUnderground(this.app.renderer);
    this.gen.onCatalog = (catalog) => {
      const warmBattle = new BattlemapLayer(stub, this.app.renderer, catalog);
      warmBattle.warmup();
      warmBattle.destroy();
    };
    this.gen.onTile = (t, epoch) => {
      this.received++;
      this.tiles?.receive(t, epoch);
    };
    this.gen.onBattlemap = (l, x, y, chunk, epoch) => {
      this.received++;
      this.battle?.receive(l, x, y, chunk, epoch);
    };
    this.gen.onStats = (s) => (this.genStats = s);
    this.gen.onProgress = (stage, frac) => {
      let done = 0;
      for (const [s, w] of STAGE_WEIGHT) {
        if (s === stage) {
          done += w * frac;
          break;
        }
        done += w;
      }
      this.progress = Math.min(1, done);
      this.status = `${STAGES[stage] ?? stage}…`;
    };
    this.gen.onError = (m) => {
      console.error('[gen]', m);
      this.status = `error: ${m}`;
    };
    this.bindInput();
    this.app.ticker.add(() => this.frame());
  }

  /** At town zoom, ask once for the districts of each town in view. */
  private askDistricts() {
    if (!this.labels || this.cam.ppf < 1 / 16) return;
    const [x0, y0, x1, y1] = this.cam.viewRect();
    const labels = this.labels;
    for (const t of this.towns) {
      if (this.districtsAsked.has(t.index) || t.x + t.r < x0 || t.x - t.r > x1 || t.y + t.r < y0 || t.y - t.r > y1) continue;
      this.districtsAsked.add(t.index);
      void this.gen.districts(t.index).then((list) => {
        if (this.labels === labels) labels.addDistricts(list, this.renames);
      });
    }
  }

  /** Show business names on the map (as pins) or not. */
  setPlaces(on: boolean) {
    this.places = on;
    this.labels?.setPlaces(on && !this.interior);
    this.onPlaces(on);
  }

  /** While Places is on at town zoom: ask for the businesses round the view once it has moved
   * off the area asked for last (the view and half again each way). */
  private askPlaces() {
    const labels = this.labels;
    if (!this.places || !labels || this.interior || this.placesBusy || this.cam.ppf < 1 / 8) return;
    const [x0, y0, x1, y1] = this.cam.viewRect();
    const r = this.placesRect;
    if (r && x0 >= r[0] && y0 >= r[1] && x1 <= r[2] && y1 <= r[3]) return;
    const [w, h] = [x1 - x0, y1 - y0];
    const rect: [number, number, number, number] = [x0 - w / 2, y0 - h / 2, x1 + w / 2, y1 + h / 2];
    this.placesBusy = true;
    const epoch = this.placesEpoch;
    void this.gen
      .inView(rect)
      .then((hits) => {
        if (this.labels !== labels || epoch !== this.placesEpoch) return;
        labels.addPlaces(
          hits.filter((h): h is Extract<typeof h, { kind: 'building' }> => h.kind === 'building'),
          this.renames,
        );
        this.placesRect = rect;
      })
      .finally(() => (this.placesBusy = false));
  }

  /** Roof fade: at battlemap zoom (a 5-ft square ≥ 8 px) the roof of the building
   * under the mouse fades away to show its ground floor. */
  private updatePeek(now: number) {
    const p = this.peek;
    if (p) {
      p.layer.update(this.cam);
      const target = p.out ? 0 : 1;
      p.layer.container.alpha += (target - p.layer.container.alpha) * 0.2;
      if (p.out && p.layer.container.alpha < 0.03) {
        p.layer.destroy();
        this.peek = null;
      }
    }
    const close = !this.interior && !!this.battle && this.battle.stats.alpha > 0.4 && this.cam.ppf * 5 >= 8 && this.lastPointerType === 'mouse';
    if (!close || !this.cursor) {
      if (this.peek) this.peek.out = true;
      return;
    }
    if (this.peekBusy || now - this.lastPeekCheck < 120) return;
    this.lastPeekCheck = now;
    this.peekBusy = true;
    const [x, y] = this.cam.screenToWorld(this.cursor.sx, this.cursor.sy);
    void (async () => {
      try {
        const hit = await this.gen.query(x, y);
        const id = hit?.kind === 'building' ? hit.id : null;
        if (id === this.peek?.id && this.peek) {
          this.peek.out = false;
          return;
        }
        if (this.peek) this.peek.out = true;
        if (!id || this.interior || !this.geom) return;
        let it = this.peekCache.get(id);
        if (it === undefined) {
          it = await this.gen.interior(id);
          this.peekCache.set(id, it);
          if (this.peekCache.size > 64) this.peekCache.delete(this.peekCache.keys().next().value!);
        }
        if (!it || this.interior) return;
        this.peek?.layer.destroy();
        const layer = new InteriorLayer(it, this.geom.sea_level_ft, true, false, this.interiorStyle);
        layer.container.alpha = 0;
        this.app.stage.addChildAt(layer.container, this.interiorIndex);
        this.peek = { id, layer, out: false };
      } finally {
        this.peekBusy = false;
      }
    })();
  }

  /** Go inside a building or a site, over the map, framed; false if it can't be entered.
   * `keep`: on that level, the camera staying where it is; `from`: arriving at this site's way
   * back to that one (a trapdoor, stairs or ladder between sites). A city's sewers open as one
   * network (`Sewers`); a section's undercroft on its own. */
  async enterBuilding(id: string, keep?: { level: number }, from?: string): Promise<boolean> {
    const it = await this.gen.interior(id);
    if (!it || !this.geom) return false;
    const back = from ? linkIn(it, from) : null;
    const level = back?.level ?? keep?.level ?? it.entry_level;
    const sewer = id.startsWith('w:') && level === it.entry_level;
    // Underground, the level one arrives on is shaded in a worker first (no stall here).
    const fields = /^[uwk]:/.test(id) && !(sewer && this.sewers?.layout === Number(id.split(':')[1])) ? new Map([[level, await prepareField(it, level, sewer)]]) : undefined;
    if (!this.geom) return false;
    let layer: InteriorLayer;
    if (sewer) {
      const layout = Number(id.split(':')[1]);
      if (this.sewers?.layout === layout) {
        // Already down there: that section is the one in view.
        layer = this.sewers.layerOf(it);
        this.interior = layer;
      } else {
        this.exitBuilding();
        this.dropPeek();
        this.sewers = new Sewers(layout, this.gen, this.geom.sea_level_ft, this.interiorStyle, it, fields?.get(level));
        this.app.stage.addChildAt(this.sewers.container, this.interiorIndex);
        layer = this.interior = this.sewers.layerOf(it);
        // Only the sections round the view are drawn: no further out than a few across.
        this.sewerMinZoom = this.cam.minZoom;
        this.cam.minZoom = Math.max(this.cam.minZoom, Math.log2(Math.max(this.cam.width, this.cam.height) / (SECTION_FT * 5)));
        this.labels?.setPlaces(false);
      }
    } else {
      this.exitBuilding();
      this.dropPeek();
      layer = new InteriorLayer(it, this.geom.sea_level_ft, false, false, this.interiorStyle, false, fields, level);
      layer.prefetch(prepareField);
      this.interior = layer;
      // Above the map and battlemap, under tokens and labels.
      this.app.stage.addChildAt(layer.container, this.interiorIndex);
      // Business pins belong to the streets outside.
      this.labels?.setPlaces(false);
    }
    if (back) {
      const [wx, wy] = layer.toWorld(back.x + 0.5, back.y + 0.5);
      this.lastArrival = { at: [wx, wy], t: performance.now() };
      this.flyTo({ cx: wx, cy: wy, zoom: Math.max(this.cam.zoom, 1.5) });
      this.emitInterior();
      return true;
    }
    if (keep) {
      this.emitInterior();
      return true;
    }
    // Frame the building, or underground the floor of the level one arrives on.
    let [x0, y0, x1, y1] = [0, 0, it.nx, it.ny];
    if (layer.underground) {
      const lv = it.levels[level];
      [x0, y0, x1, y1] = [it.nx, it.ny, 0, 0];
      lv.cells.forEach((c, k) => {
        if (c < 0) return;
        const [i, j] = [k % it.nx, Math.floor(k / it.nx)];
        [x0, y0, x1, y1] = [Math.min(x0, i), Math.min(y0, j), Math.max(x1, i + 1), Math.max(y1, j + 1)];
      });
    }
    const [cx, cy] = layer.toWorld((x0 + x1) / 2, (y0 + y1) / 2);
    const size = Math.max(x1 - x0, y1 - y0) * 5 * 1.3;
    const zoom = Math.min(this.cam.maxZoom, Math.log2((Math.min(this.cam.width, this.cam.height) * 0.85) / size));
    this.flyTo({ cx, cy, zoom });
    this.emitInterior();
    return true;
  }

  private dropPeek() {
    this.peek?.layer.destroy();
    this.peek = null;
  }

  /** The building or site drawn under a world point: in the sewers, that point's section. */
  interiorAt(x: number, y: number): InteriorLayer | null {
    return this.sewers?.layerAt(x, y) ?? this.interior;
  }

  /** The site being designed (its draft is shown, `showInterior`): edits to its design don't
   * reload it, and Esc doesn't leave it. */
  designing: string | null = null;
  private shown = 0;

  /** Show `it` in place of the site we are in (the designer's draft), on the level in view
   * (counted from the top: levels come and go at the bottom), the camera staying. */
  async showInterior(it: Interior): Promise<void> {
    const layer = this.interior;
    if (!layer || this.sewers || !this.geom || layer.interior.id !== it.id) return;
    const ticket = ++this.shown;
    const level = Math.max(0, Math.min(it.levels.length - 1, layer.currentLevel + it.levels.length - layer.interior.levels.length));
    // (Underground floors are shaded in a worker first; a building's need nothing.)
    const field = /^[uwk]:/.test(it.id) ? await prepareField(it, level, false) : null;
    if (ticket !== this.shown || this.interior !== layer || !this.geom) return;
    const next = new InteriorLayer(it, this.geom.sea_level_ft, false, false, this.interiorStyle, false, field ? new Map([[level, field]]) : undefined, level);
    const at = this.app.stage.getChildIndex(layer.container);
    layer.destroy();
    this.app.stage.addChildAt(next.container, at);
    this.interior = next;
    next.update(this.cam);
    this.emitInterior();
  }

  /** Draw the building or site again (doors opened or shut, play mode on or off). */
  redrawInterior() {
    if (this.sewers) this.sewers.redraw();
    else this.interior?.redraw();
  }

  /** Take a way on from a square: to another level, out to the surface, or into another site. */
  async move(m: Move) {
    if (!this.interior) return;
    this.onMoves([], 0, 0);
    this.lastWay = { at: m.at, kind: m.kind, t: performance.now() };
    if (m.kind === 'level') {
      // In the sewers, the shaft may be in another section than the one in the middle.
      if (m.from !== this.interior.interior.id) await this.enterBuilding(m.from, { level: m.level });
      else this.setInteriorLevel(m.level);
    } else if (m.kind === 'surface') this.exitBuilding();
    else await this.enterBuilding(m.to, undefined, m.from);
  }

  setInteriorLevel(i: number) {
    const layer = this.interior;
    if (!layer) return;
    const it = layer.interior;
    const level = Math.max(0, Math.min(it.levels.length - 1, i));
    // Down from the sewers into a section's undercroft, or back up into the whole network.
    if (it.id.startsWith('w:') && level !== layer.currentLevel && (this.sewers || level === it.entry_level)) {
      void this.enterBuilding(it.id, { level });
      return;
    }
    layer.setLevel(level);
    this.emitInterior();
  }

  exitBuilding() {
    if (!this.interior) return;
    if (this.sewers) {
      this.sewers.destroy();
      this.sewers = null;
      if (this.sewerMinZoom !== null) this.cam.minZoom = this.sewerMinZoom;
      this.sewerMinZoom = null;
    } else {
      this.interior.destroy();
    }
    this.interior = null;
    this.labels?.setPlaces(this.places);
    this.onInterior(null);
  }

  private emitInterior() {
    const it = this.interior;
    if (!it) return;
    this.markNpcs();
    this.onInterior({ id: it.interior.id, name: this.renames[it.interior.id] ?? it.interior.name ?? it.interior.function, levels: it.levelInfo(), level: it.currentLevel });
  }

  /** Show the NPCs placed in the building or site we are in (the DM's map only). */
  private markNpcs() {
    const it = this.interior;
    if (!it || this.interiorStyle.player) return;
    const id = it.interior.id;
    const list = Object.entries(this.edits.npcs ?? {})
      .filter(([, n]) => n.location?.id === id)
      .map(([k, n]) => {
        const l = n.location!;
        return { id: k, name: n.name, level: l.level ?? it.interior.entry_level, at: l.x !== undefined && l.y !== undefined ? ([l.x, l.y] as [number, number]) : undefined };
      });
    it.setNpcs(list);
  }

  async loadWorld(world: WorldFile) {
    this.status = 'Generating continent…';
    this.progress = 0;
    const { geom, t0Ms, overlay, catalog } = await this.gen.init(world);
    this.tiles?.destroy();
    this.battle?.destroy();
    this.labels?.destroy();
    this.exitBuilding();
    this.peek?.layer.destroy();
    this.peek = null;
    this.peekCache.clear();
    this.lastWantKey = '';
    this.geom = geom;
    this.overlay = this.baseOverlay = overlay;
    this.tiles = new TileLayer(geom, this.gen);
    this.labels = new Labels(overlay.features);
    // The generators have the world's edits already; the labels take them now.
    this.edits = {};
    this.setEdits(world.edits ?? {}, false);
    this.labels.setPlaces(this.places);
    this.placesRect = null;
    // Towns and up get district labels once zoomed in on them (layout order = settlement order).
    const kinds = ['metropolis', 'city', 'town', 'village'];
    this.towns = overlay.features
      .filter((f) => kinds.includes(f.kind))
      .map((f, index) => ({ x: f.x, y: f.y, r: 12 * Math.sqrt(Number(/pop\. ([\d,]+)/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0)), index, kind: f.kind }))
      .filter((t) => t.kind !== 'village');
    this.districtsAsked.clear();
    this.battle = new BattlemapLayer(geom, this.app.renderer, catalog, this.role === 'player');
    this.battle.setGrid(this.gridOn);
    this.battle.sprites = this.edits.sprites ?? {};
    this.battle.warmup();
    this.app.stage.addChild(this.tiles.container);
    this.app.stage.addChild(this.battle.container);
    this.app.stage.addChild(this.playUnder);
    this.app.stage.addChild(this.labels.container);
    this.app.stage.addChild(this.playOver);
    this.app.stage.addChild(this.toolLayer);
    this.progress = null;
    this.syncSize();
    this.cam.minZoom = this.cam.fitZoom(geom.map_w_ft, geom.map_h_ft, 0.5);
    this.cam.set({ cx: geom.map_w_ft / 2, cy: geom.map_h_ft / 2, zoom: this.cam.fitZoom(geom.map_w_ft, geom.map_h_ft) });
    this.t0Ms = t0Ms;
    this.status = `Continent generated in ${(t0Ms / 1000).toFixed(1)} s`;
  }

  /**
   * Apply the world file's edits live: names, hidden labels, created sites (labels, and the
   * generators, which make the tiles and battlemaps around changed sites again).
   * `send`: false when the generators already have them. `changed`: the keys each field's
   * changes touch since the edits applied last (from their ops), so a field of thousands of
   * entries isn't searched for the few that changed.
   */
  setEdits(next: Edits, send = true, changed?: Record<string, string[]>) {
    const prev = this.edits;
    this.edits = next;
    /** The ids in a keyed field that may have changed. */
    const keys = (field: string, a: object, b: object): Iterable<string> => changed?.[field] ?? (changed ? [] : new Set([...Object.keys(a), ...Object.keys(b)]));
    const before = prev.created ?? [];
    const after = next.created ?? [];
    const rects: Rect[] = [];
    for (let k = 0; k < Math.max(before.length, after.length); k++) {
      const a = before[k];
      const b = after[k];
      if (a && b && siteKey(a) === siteKey(b)) continue;
      for (const c of [a, b]) {
        if (!c) continue;
        const r = siteReach(c);
        rects.push([c.x - r, c.y - r, c.x + r, c.y + r]);
      }
    }
    const createdChanged = JSON.stringify(before) !== JSON.stringify(after);
    if (createdChanged && this.baseOverlay) {
      const created = after.filter((c) => !c.removed).map(createdFeature);
      this.overlay = { ...this.baseOverlay, features: [...this.baseOverlay.features, ...created] };
      this.labels?.setCreated(created);
    }
    // Changed names, and created sites' (their labels are new).
    const pr = prev.renames ?? {};
    const nr = next.renames ?? {};
    // (Only names drawn inside, levels, rooms and sites, redraw the building or site we are in.)
    let renamed = false;
    // (Fields that are the same object are unchanged: edits are never changed in place.)
    if (pr !== nr || createdChanged) for (const id of createdChanged ? new Set([...Object.keys(pr), ...Object.keys(nr)]) : keys('renames', pr, nr)) {
      if (pr[id] !== nr[id] && /^[lrbtuwk]:/.test(id)) renamed = true;
      if (pr[id] !== nr[id] || (createdChanged && id.startsWith('c:'))) this.labels?.rename(id, nr[id] ?? null);
    }
    this.renames = nr;
    InteriorLayer.names = nr;
    // Inside: the title, level and room names as renamed.
    if (renamed && this.interior) {
      this.redrawInterior();
      this.emitInterior();
    }
    if (prev.hidden !== next.hidden) this.labels?.setHidden(new Set(next.hidden ?? []));
    if (prev.npcs !== next.npcs) this.markNpcs();
    // A site designed anew while we are in it (by undo, an agent): shown as it is now.
    const here = this.interior?.interior.id;
    if (here && here !== this.designing && prev.designs?.[here] !== next.designs?.[here] && JSON.stringify(prev.designs?.[here]) !== JSON.stringify(next.designs?.[here])) {
      void this.enterBuilding(here, { level: this.interior!.currentLevel });
    }
    if (this.battle) this.battle.sprites = next.sprites ?? {};
    // Objects put down or taken away: only the battlemaps holding them are made again.
    const battleRects: Rect[] = [];
    const added: NonNullable<Edits['objects']>[string][] = [];
    const po = prev.objects ?? {};
    const no = next.objects ?? {};
    if (po !== no) for (const id of keys('objects', po, no)) {
      const [a, b] = [po[id], no[id]];
      if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
      for (const o of [a, b]) if (o) battleRects.push([o.x - 1, o.y - 1, o.x + 1, o.y + 1]);
      if (b && !this.previewed.delete(`${b.x},${b.y}`)) added.push(b);
    }
    const pc = prev.cleared ?? {};
    const nc = next.cleared ?? {};
    const reach = (c: Clear) => (c.r ?? 1.25) + 1;
    if (pc !== nc) for (const id of keys('cleared', pc, nc)) {
      const [a, b] = [pc[id], nc[id]];
      if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
      for (const c of [a, b]) if (c) battleRects.push([c.x - reach(c), c.y - reach(c), c.x + reach(c), c.y + reach(c)]);
    }
    // A sprite's rules changed: every chunk using it.
    const ps = prev.sprites ?? {};
    const ns = next.sprites ?? {};
    if (ps !== ns) for (const asset of new Set([...Object.keys(ps), ...Object.keys(ns)])) {
      if (JSON.stringify(ps[asset]) === JSON.stringify(ns[asset])) continue;
      for (const o of Object.values(no)) if (o.kind === `s:${asset}`) battleRects.push([o.x - 1, o.y - 1, o.x + 1, o.y + 1]);
    }
    // Crossings put down by hand: the battlemaps and site tiles along them (decks are drawn on
    // both; a ferry's jetties reach past its ends).
    const pv = prev.crossings ?? {};
    const nv = next.crossings ?? {};
    if (pv !== nv) for (const id of keys('crossings', pv, nv)) {
      const [a, b] = [pv[id], nv[id]];
      if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
      for (const c of [a, b]) {
        if (!c) continue;
        const r = c.width / 2 + 40;
        const box: Rect = [Math.min(c.a[0], c.b[0]) - r, Math.min(c.a[1], c.b[1]) - r, Math.max(c.a[0], c.b[0]) + r, Math.max(c.a[1], c.b[1]) + r];
        rects.push(box);
      }
    }
    // The world's own buildings changed or taken away: round their middle as generated (a
    // castle keep reaches ~150 ft) and any new footprint.
    const pb = prev.buildings ?? {};
    const nb = next.buildings ?? {};
    let reenter = false;
    const replaced: string[] = [];
    if (pb !== nb) for (const id of keys('buildings', pb, nb)) {
      const [a, b] = [pb[id], nb[id]];
      if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
      replaced.push(id);
      for (const e of [a, b]) {
        if (!e) continue;
        const pts: [number, number][] = [[e.at[0] - 150, e.at[1] - 150], [e.at[0] + 150, e.at[1] + 150], ...(e.poly ?? [])];
        rects.push([Math.min(...pts.map((p) => p[0])) - 5, Math.min(...pts.map((p) => p[1])) - 5, Math.max(...pts.map((p) => p[0])) + 5, Math.max(...pts.map((p) => p[1])) + 5]);
      }
      // Inside it: shown as it is now (unless it is being designed: the designer shows it).
      if (id === here && b && !b.removed && id !== this.designing) reenter = true;
    }
    // Their business pins: dropped, and asked for again (as they are now).
    if (replaced.length) {
      this.labels?.forgetPlaces(replaced);
      this.placesRect = null;
      this.placesEpoch++;
    }
    if (!send) return;
    if (rects.length || battleRects.length) {
      const epoch = ++this.editEpoch;
      this.gen.setEdits(next, rects, epoch, battleRects, changed && { prev, changed });
      if (rects.length) this.tiles?.invalidate(rects, SITE_MIN_LEVEL, epoch);
      this.battle?.invalidate([...rects, ...battleRects], epoch);
      if (added.length) this.battle?.preview(added, epoch);
    } else {
      this.gen.setEdits(next, [], this.editEpoch, [], changed && { prev, changed });
    }
    if (reenter && this.interior) void this.enterBuilding(this.interior.interior.id, { level: this.interior.currentLevel });
  }

  /** Draw objects about to be put down (a brush stroke) at once, before the change is made. */
  previewObjects(objs: Placed[]) {
    if (!this.battle) return;
    this.battle.preview(objs, this.editEpoch + 1);
    for (const o of objs) this.previewed.add(`${o.x},${o.y}`);
  }

  /**
   * A PNG (data URL) of the map, `CAPTURE_W` x `CAPTURE_H` px, centred on (cx, cy) with
   * `sizeFt` of ground across it, once everything there has loaded (or after `timeoutMs`).
   * The canvas takes that size meanwhile; then the view goes back to where it was. Works
   * with the page in the background too (frames are driven by hand then).
   */
  async capture(cx: number, cy: number, sizeFt: number, timeoutMs = 20000): Promise<string> {
    const back = { cx: this.cam.cx, cy: this.cam.cy, zoom: this.cam.zoom };
    const resolution = this.app.renderer.resolution;
    this.fly = null;
    this.captureSize = [CAPTURE_W, CAPTURE_H];
    this.app.renderer.resize(CAPTURE_W, CAPTURE_H, 1);
    this.syncSize();
    this.cam.set({ cx, cy, zoom: Math.max(this.cam.minZoom, Math.min(this.cam.maxZoom, Math.log2(CAPTURE_W / Math.max(20, sizeFt)))) });
    let driving = true;
    const channel = new MessageChannel();
    // Without animation frames (a hidden page), tick by hand: messages aren't throttled.
    channel.port1.onmessage = () => {
      if (!driving) return;
      const now = performance.now();
      if (now - this.lastFrame > 100) this.app.ticker.update(now);
      setTimeout(() => channel.port2.postMessage(0), 16);
    };
    channel.port2.postMessage(0);
    try {
      const end = performance.now() + timeoutMs;
      const start = this.frameCount;
      let calm = 0;
      while (calm < 5 && performance.now() < end) {
        await new Promise((r) => setTimeout(r, 50));
        // Only frames drawn at the new place count.
        const quiet = this.frameCount > start + 1 && this.lastWants === 0 && this.readiness >= 1 && !this.battle?.pending && !this.labels?.lastCreated;
        calm = quiet ? calm + 1 : 0;
      }
      // Draw and read back in one go (the drawing buffer is not kept after it is shown).
      this.app.ticker.update(performance.now());
      return this.app.canvas.toDataURL('image/png');
    } finally {
      driving = false;
      channel.port1.close();
      this.captureSize = null;
      this.app.renderer.resize(this.el.clientWidth, this.el.clientHeight, resolution);
      this.syncSize();
      this.cam.set(back);
    }
  }

  /** Sketch mode on: the primary button (or one finger) draws with `sketcher`; the right or
   * middle button (or two fingers) pans. Returns the layer showing strokes and the preview. */
  enterSketch(sketcher: Sketcher): SketchLayer | null {
    this.exitSketch();
    if (!this.geom) return null;
    this.sketcher = sketcher;
    this.sketchLayer = new SketchLayer(this.geom, sketcher);
    this.app.stage.addChild(this.sketchLayer.container);
    return this.sketchLayer;
  }

  exitSketch() {
    this.sketcher?.cancel();
    this.sketcher = null;
    this.sketching = null;
    this.sketchLayer?.destroy();
    this.sketchLayer = null;
  }

  flyTo(target: CameraState, speed = 1) {
    // Snapshot the start: the path reads it every frame, and the camera moves along it.
    const from = { cx: this.cam.cx, cy: this.cam.cy, zoom: this.cam.zoom };
    const path = flyPath(from, target, this.cam.width);
    this.fly = { path, start: performance.now(), ms: Math.max(300, (path.duration * 1000) / speed) };
  }

  /** How long (ms) the map has been still: no pan, zoom, fly or drag. Work that costs a frame
   * (the address bar's link) waits for it, so the frame it drops is one nobody sees move. */
  stillMs(): number {
    return performance.now() - this.movedAt;
  }

  private noteMotion(now: number) {
    const [x, y, z] = this.lastCam;
    const c = this.cam;
    if (this.drag || this.fly || c.cx !== x || c.cy !== y || c.zoom !== z) this.movedAt = now;
    this.lastCam = [c.cx, c.cy, c.zoom];
  }

  /** Fraction of target-level tiles in view that are loaded and faded in. */
  get readiness(): number {
    const s = this.tiles?.stats;
    return s && s.targetTiles ? s.targetReady / s.targetTiles : 0;
  }

  private syncSize() {
    [this.cam.width, this.cam.height] = this.captureSize ?? [this.el.clientWidth, this.el.clientHeight];
  }

  private frame() {
    const now = performance.now();
    const dt = Math.min(100, now - this.lastFrame);
    this.lastFrame = now;
    this.frameCount++;
    this.frameTimes.push(dt);
    if (this.frameTimes.length > 120) this.frameTimes.shift();
    this.syncSize();

    this.noteMotion(now);
    const driven = this.driver?.(now);
    if (driven) {
      this.cam.set(driven);
    } else if (this.fly) {
      const t = Math.min(1, (now - this.fly.start) / this.fly.ms);
      const e = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2;
      this.cam.set(this.fly.path.at(e));
      if (t >= 1) this.fly = null;
    } else if (!this.drag) {
      // Inertia.
      const decay = Math.exp(-dt / 180);
      if (Math.abs(this.vel.x) + Math.abs(this.vel.y) > 0.01) {
        this.cam.panPixels(this.vel.x * dt, this.vel.y * dt);
        this.vel.x *= decay;
        this.vel.y *= decay;
      }
      if (Math.abs(this.vel.z) > 1e-4) {
        const cx = this.cursor?.sx ?? this.cam.width / 2;
        const cy = this.cursor?.sy ?? this.cam.height / 2;
        this.cam.zoomAt(this.vel.z * dt, cx, cy);
        this.vel.z *= Math.exp(-dt / 90);
      }
    }

    if (this.tiles) {
      this.applyStyle();
      const [vx0, , vx1] = this.cam.viewRect();
      const viewW = vx1 - vx0;
      const jx = this.cam.cx - this.prevCenter[0];
      const jy = this.cam.cy - this.prevCenter[1];
      this.prevCenter = [this.cam.cx, this.cam.cy];
      if (Math.hypot(jx, jy) > viewW) {
        // A jump (fly-to, search, reload, script), not motion: no look-ahead.
        this.camVel = [0, 0];
      } else {
        const d = Math.max(1, dt);
        this.camVel = [this.camVel[0] * 0.8 + (jx / d) * 0.2, this.camVel[1] * 0.8 + (jy / d) * 0.2];
      }
      // Look ahead at most one view width, whatever the speed.
      const LEAD_MS = Math.min(500, viewW / Math.max(1e-9, Math.hypot(this.camVel[0], this.camVel[1])));
      const t0 = performance.now();
      const want: WantTile[] = this.tiles.update(this.cam, this.app.renderer.resolution, now, [this.camVel[0] * LEAD_MS, this.camVel[1] * LEAD_MS]);
      if (this.battle) want.push(...this.battle.update(this.cam, now));
      this.lastWants = want.length;
      // Under a fully built battlemap the tile layer is invisible, and in the sewers (their
      // rock covers the view) the whole street map: skip drawing them.
      this.tiles.container.visible = !this.battle?.covering && !this.sewers;
      if (this.battle) this.battle.container.visible &&= !this.sewers;
      // Safety net: a screenful needs a few hundred tiles at most; never flood the generator.
      const MAX_WANT = 600;
      if (want.length > MAX_WANT) {
        want.sort((a, b) => a.pri - b.pri);
        want.length = MAX_WANT;
      }
      // A hash of the wanted tiles (two 32-bit FNV-style lanes), not a string of every id: this
      // runs every frame and a long joined string is a lot of garbage.
      let h1 = 0x811c9dc5;
      let h2 = 0x9e3779b9;
      for (const w of want) {
        for (const v of [w.kind === 'battlemap' ? 1 : 0, w.level, w.x, w.y]) {
          h1 = Math.imul(h1 ^ v, 0x01000193);
          h2 = Math.imul(h2 ^ v, 0x85ebca6b) ^ (h2 >>> 13);
        }
      }
      const wantKey = `${h1 >>> 0}.${h2 >>> 0}.${want.length}`;
      if (wantKey !== this.lastWantKey) {
        this.lastWantKey = wantKey;
        this.gen.want(want);
      }
      const t1 = performance.now();
      this.labels?.update(this.cam, now);
      this.sketchLayer?.update(this.cam);
      if (this.sewers) {
        this.sewers.update(this.cam, now);
        // The section in the middle of the view is the one the levels list speaks of.
        const mid = this.sewers.layerAt(this.cam.cx, this.cam.cy);
        if (mid && mid !== this.interior) {
          this.interior = mid;
          this.emitInterior();
        }
      } else {
        this.interior?.update(this.cam);
      }
      this.updatePeek(now);
      const tp = performance.now();
      this.onFrame?.(now);
      this.playMs = performance.now() - tp;
      if (this.tool?.draw || this.toolDrawn) {
        this.toolLayer.clear();
        this.tool?.draw?.(this.toolLayer, this.cam);
        this.toolDrawn = !!this.tool?.draw;
      }
      if (now - this.lastDistrictCheck > 300) {
        this.lastDistrictCheck = now;
        this.askDistricts();
        this.askPlaces();
      }
      const t2 = performance.now();
      // What happened this frame: tiles uploaded, chunks built, labels lettered, messages in.
      // (Only while the benchmark drives the camera: a string a frame is garbage otherwise.)
      const ev = this.driver ? `u${this.tiles.lastUploads} b${this.battle?.lastBuilt ?? 0} l${this.labels?.lastCreated ?? 0} m${this.received}` : '';
      this.received = 0;
      this.prof = { tiles: t1 - t0, labels: t2 - t1 - this.playMs, play: this.playMs, uploads: this.tiles.lastUploadMs, render: this.renderMs, gpu: this.gpuMs, frame: dt, ev };
    }

    if (now - this.lastHud > 100) {
      this.lastHud = now;
      this.emitHud();
    }
  }

  private applyStyle() {
    const tiles = this.tiles!;
    const ftPerPx = 1 / this.cam.ppf;
    // Relief is weaker at coarse scales: exaggerate more when zoomed out, then partially
    // normalize to the view's measured relief so hills read up close and ranges dominate far out.
    const base = Math.min(25, Math.max(1, (ftPerPx / 2) ** 0.35));
    const target = Math.min(1.6, Math.max(0.6, Math.sqrt(0.22 / Math.max(1e-4, tiles.reliefRms * base))));
    this.exagFactor += (target - this.exagFactor) * 0.08;
    const u = tiles.frameUniforms.uniforms;
    u.uExag = base * this.exagFactor;
    u.uFtPerPx = ftPerPx;
    const r = tiles.riverFrame.uniforms;
    r.uPpf = this.cam.ppf;
    // Only big rivers when zoomed out; every mapped river from region zoom in.
    r.uMinQ = Math.min(5e6, Math.max(RIVER_Q, RIVER_Q * (ftPerPx / 60)));
    // Map symbols ~11 px apart at any zoom: two power-of-two octaves, crossfaded.
    const k = Math.log2(11 * ftPerPx);
    const k0 = Math.floor(k);
    const f = k - k0;
    tiles.patternCells = [2 ** k0, 2 ** (k0 + 1)];
    // The crossfade between octaves is kept to the middle of each octave, so most of the time
    // only one octave's symbols are evaluated (each costs about half a millisecond a frame).
    const fm = Math.min(1, Math.max(0, (f - 0.3) / 0.4));
    u.uPatMix = fm * fm * (3 - 2 * fm);
    // Symbols are a cartographic abstraction: fade them out at site/battlemap zoom.
    u.uPatAlpha = Math.min(1, Math.max(0, (ftPerPx - 0.8) / 3.2));
    if (ftPerPx > 300) [u.uContour, u.uIndexEvery] = [1000, 5];
    else if (ftPerPx > 30) [u.uContour, u.uIndexEvery] = [200, 5];
    else if (ftPerPx > 3) [u.uContour, u.uIndexEvery] = [50, 5];
    else [u.uContour, u.uIndexEvery] = [5, 5];
  }

  private emitHud() {
    const sorted = [...this.frameTimes].sort((a, b) => a - b);
    const avg = sorted.reduce((a, b) => a + b, 0) / Math.max(1, sorted.length);
    const ftPerPx = 1 / this.cam.ppf;
    let cursor: HudState['cursor'] = null;
    if (this.cursor && this.tiles) {
      const [x, y] = this.cam.screenToWorld(this.cursor.sx, this.cursor.sy);
      const inside = this.interiorAt(x, y)?.inspect(x, y) ?? null;
      const square = inside ?? (this.battle && this.battle.stats.alpha > 0.5 ? this.battle.inspect(x, y) : null);
      cursor = { x, y, elev: this.tiles.heightAt(x, y), square };
    }
    this.onHud({
      zoom: this.cam.zoom,
      ftPerPx,
      center: { x: this.cam.cx, y: this.cam.cy },
      tier: TIERS.find(([t]) => ftPerPx > t)?.[1] ?? 'Battlemap',
      cursor,
      fps: avg ? 1000 / avg : 0,
      layer: this.tiles?.stats ?? { level: 0, targetTiles: 0, targetReceived: 0, targetUploaded: 0, targetReady: 0, gpuTiles: 0, queuedUploads: 0 },
      gen: this.genStats,
      status: this.status,
      progress: this.progress,
    });
  }

  private bindInput() {
    const c = this.app.canvas;
    c.style.touchAction = 'none';
    c.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault();
        if (this.inputOff) return;
        this.fly = null;
        const r = c.getBoundingClientRect();
        this.cursor = { sx: e.clientX - r.left, sy: e.clientY - r.top };
        const lines = e.deltaMode === 1 ? 40 : 1;
        this.vel.z += (-e.deltaY * lines) * 0.00004;
        this.vel.z = Math.max(-0.02, Math.min(0.02, this.vel.z));
      },
      { passive: false },
    );
    c.addEventListener('pointerdown', (e) => {
      this.lastPointerType = e.pointerType;
      if (this.inputOff) return;
      this.fly = null;
      try {
        c.setPointerCapture(e.pointerId);
      } catch {
        // Not a live pointer (synthetic input): no capture needed.
      }
      if (this.tool && this.pointers.size === 0 && this.toolPointer === null) {
        const r = c.getBoundingClientRect();
        const [x, y] = this.cam.screenToWorld(e.clientX - r.left, e.clientY - r.top);
        if (this.tool.down(x, y, e)) {
          this.toolPointer = e.pointerId;
          this.vel.x = this.vel.y = 0;
          return;
        }
      }
      if (this.sketcher) {
        if (e.button === 0 && this.pointers.size === 0 && this.sketching === null) {
          const r = c.getBoundingClientRect();
          const [x, y] = this.cam.screenToWorld(e.clientX - r.left, e.clientY - r.top);
          this.sketching = e.pointerId;
          this.vel.x = this.vel.y = 0;
          this.sketcher.down(x, y, this.cam.ppf);
          return;
        }
        // A second finger: not a stroke after all, but a pan or pinch.
        if (this.sketching !== null) {
          this.sketcher.cancel();
          this.sketching = null;
        }
      }
      this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (this.pointers.size === 1) {
        this.tap = { x: e.clientX, y: e.clientY, t: performance.now() };
        this.drag = { id: e.pointerId, x: e.clientX, y: e.clientY, t: performance.now() };
        this.vel.x = this.vel.y = 0;
      } else if (this.pointers.size === 2) {
        this.tap = null;
        const [a, b] = [...this.pointers.values()];
        this.pinchDist = Math.hypot(a.x - b.x, a.y - b.y);
      }
    });
    c.addEventListener('pointermove', (e) => {
      const r = c.getBoundingClientRect();
      this.cursor = { sx: e.clientX - r.left, sy: e.clientY - r.top };
      if (e.pointerId === this.sketching && this.sketcher) {
        const [x, y] = this.cam.screenToWorld(e.clientX - r.left, e.clientY - r.top);
        this.sketcher.move(x, y);
        return;
      }
      if (this.tool) {
        const [x, y] = this.cam.screenToWorld(e.clientX - r.left, e.clientY - r.top);
        if (e.pointerId === this.toolPointer) {
          this.tool.move(x, y, e);
          return;
        }
        if (!this.pointers.size) this.tool.hover(x, y);
      }
      if (!this.pointers.has(e.pointerId) || this.inputOff) return;
      this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        const d = Math.hypot(a.x - b.x, a.y - b.y);
        if (this.pinchDist > 0) this.cam.zoomAt(Math.log2(d / this.pinchDist), (a.x + b.x) / 2 - r.left, (a.y + b.y) / 2 - r.top);
        this.pinchDist = d;
      } else if (this.drag && this.drag.id === e.pointerId) {
        const now = performance.now();
        const dx = e.clientX - this.drag.x;
        const dy = e.clientY - this.drag.y;
        this.cam.panPixels(dx, dy);
        const dt = Math.max(1, now - this.drag.t);
        this.vel.x = 0.8 * (dx / dt) + 0.2 * this.vel.x;
        this.vel.y = 0.8 * (dy / dt) + 0.2 * this.vel.y;
        this.drag = { id: e.pointerId, x: e.clientX, y: e.clientY, t: now };
      }
    });
    const end = (e: PointerEvent) => {
      if (e.pointerId === this.toolPointer) {
        this.toolPointer = null;
        const r = c.getBoundingClientRect();
        const [x, y] = this.cam.screenToWorld(e.clientX - r.left, e.clientY - r.top);
        if (e.type === 'pointerup') this.tool?.up(x, y, e);
        else this.tool?.cancel();
        return;
      }
      if (e.pointerId === this.sketching) {
        this.sketching = null;
        if (e.type === 'pointerup') this.sketcher?.up();
        else this.sketcher?.cancel();
        return;
      }
      this.pointers.delete(e.pointerId);
      const tap = this.tap;
      this.tap = null;
      if (tap && e.type === 'pointerup' && this.pointers.size === 0 && Math.hypot(e.clientX - tap.x, e.clientY - tap.y) < 5 && performance.now() - tap.t < 400) {
        const r = c.getBoundingClientRect();
        const [x, y] = this.cam.screenToWorld(e.clientX - r.left, e.clientY - r.top);
        // Inside a site, a click offers the ways on from that square; outside, it picks.
        const site = this.interiorAt(x, y);
        if (site) this.onMoves(site.movesAt(x, y), e.clientX - r.left, e.clientY - r.top);
        else this.onPick(x, y, e.clientX - r.left, e.clientY - r.top);
        // Touch screens: a second tap nearby soon after is a double-tap.
        if (e.pointerType !== 'mouse') {
          const now = performance.now();
          const last = this.lastTap;
          if (last && now - last.t < 350 && Math.hypot(e.clientX - last.x, e.clientY - last.y) < 30) {
            this.lastTap = null;
            void this.doubleTap(e.clientX - r.left, e.clientY - r.top, false);
          } else {
            this.lastTap = { x: e.clientX, y: e.clientY, t: now };
          }
        }
      }
      if (this.drag?.id === e.pointerId) {
        if (performance.now() - this.drag.t > 80) this.vel.x = this.vel.y = 0;
        this.drag = null;
      }
      if (this.pointers.size < 2) this.pinchDist = 0;
    };
    c.addEventListener('pointerup', end);
    c.addEventListener('pointercancel', end);
    c.addEventListener('pointerleave', () => (this.cursor = null));
    // Sketching pans with the right button, play tools use it: no menu.
    c.addEventListener('contextmenu', (e) => {
      if (this.sketcher || this.tool) e.preventDefault();
    });
    c.addEventListener('dblclick', (e) => {
      // Touch double-taps are handled on pointerup (browsers differ on synthesizing these).
      if (this.lastPointerType !== 'mouse' || this.sketcher || this.inputOff) return;
      const r = c.getBoundingClientRect();
      if (this.tool?.dblclick?.(...this.cam.screenToWorld(e.clientX - r.left, e.clientY - r.top))) return;
      void this.doubleTap(e.clientX - r.left, e.clientY - r.top, e.shiftKey);
    });
    if (!this.ownKeys) return;
    window.addEventListener('keydown', (e) => {
      if (this.inputOff || (e.target as HTMLElement)?.closest?.('input, textarea, select')) return;
      const step = 120;
      if (e.key === '+' || e.key === '=') this.zoomBy(0.006);
      else if (e.key === '-' || e.key === '_') this.zoomBy(-0.006);
      else if (e.key === 'ArrowLeft') this.panBy(step, 0);
      else if (e.key === 'ArrowRight') this.panBy(-step, 0);
      else if (e.key === 'ArrowUp') this.panBy(0, step);
      else if (e.key === 'ArrowDown') this.panBy(0, -step);
      else if (e.key === 'g' || e.key === 'G') this.setGrid(!this.gridOn);
      else if (e.key === 'p' || e.key === 'P') this.setPlaces(!this.places);
      else if (this.interior && e.key === 'Escape' && !this.designing) this.exitBuilding();
      else if (e.key === 'PageUp' || e.key === ']') this.levelBy(1);
      else if (e.key === 'PageDown' || e.key === '[') this.levelBy(-1);
    });
  }

  /** Double-click / double-tap: go into a building at battlemap zoom, else zoom in there. */
  private async doubleTap(sx: number, sy: number, shift: boolean) {
    if (this.inputOff) return;
    const [x, y] = this.cam.screenToWorld(sx, sy);
    // Stairs, ladders, the way out, trapdoors, tunnels: take the first way on from the square
    // (into another site, arriving at its way back here).
    const moves = this.interiorAt(x, y)?.movesAt(x, y) ?? [];
    if (moves.length) {
      await this.move(moves[0]);
      return;
    }
    if (!this.interior && this.battle && this.battle.stats.alpha > 0.5) {
      const hit = await this.gen.query(x, y);
      if (hit?.kind === 'building') {
        this.lastWay = { at: [x, y], kind: 'enter', t: performance.now() };
        if (await this.enterBuilding(hit.id)) return;
      }
    }
    this.flyTo({ cx: x, cy: y, zoom: Math.min(this.cam.maxZoom, this.cam.zoom + (shift ? -1.5 : 1.5)) }, 1.5);
  }
}

/** Where a site's way back to `from` is: its level and square. */
function linkIn(it: Interior, from: string): { level: number; x: number; y: number } | null {
  for (let li = 0; li < it.levels.length; li++) {
    const k = it.levels[li].links?.find((l) => l.to === from);
    if (k) return { level: li, x: k.x, y: k.y };
  }
  return null;
}
