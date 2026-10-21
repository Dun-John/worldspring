// Battlemap tier: painterly ground, objects and atmosphere for finest-level tiles, faded in
// over the ink map as the 5-ft grid becomes legible. Chunks arrive from the generator
// workers already decoded (`gen/battlePrep.ts`), so building one here only creates GPU objects.
import {
  BufferImageSource,
  Container,
  RenderTexture,
  Geometry,
  Graphics,
  GlProgram,
  Mesh,
  Particle,
  ParticleContainer,
  Shader,
  Sprite,
  Texture,
  UniformGroup,
  type Renderer,
} from 'pixi.js';
import { fromHalf, HS, kindRules, prepareChunk, ROOF_INK, SPRITE_KIND, SQ, toHalf, type ChunkSprite, type PreparedChunk, type Roof, type Shape } from '../gen/battlePrep';
import type { KindInfo } from '../gen/client';
import type { Geom, Placed, Rect, SpriteMeta, WantTile } from '../gen/protocol';
import { dmOnlyHazard, UNKNOWN, type TacticalGrid } from '../play/vision';
import { Edited } from './TileLayer';
import { buildAtlas, looks, PX, UPRIGHT_FOOT, type Atlas } from './atlas';
import { deckGrid, drawDeck, drawFerry, drawFord, drawRoadBridge } from './bridges';
import { banner, drawForecourtGate, drawGardenWalls, drawGate, drawGatePier, drawGravelWalk, drawWalls, drawWallTower, topGrid } from './walls';
import { customTexture, onLoaded } from './customAtlas';
import { noiseSource } from './noise';
import { sheetFor, thumbSheet, type ThumbSheet } from './thumbs';
import type { Camera } from './camera';
import atmoFragment from './shaders/atmosphere.frag?raw';
import colorMeshFragment from './shaders/colormesh.frag?raw';
import colorMeshVertex from './shaders/colormesh.vert?raw';
import groundFragment from './shaders/ground.frag?raw';
import vertex from './shaders/terrain.vert?raw';

const SQUARE_FT = 5;
const CHUNK_CAP = 24;
/** Battlemap fades in between these ft-per-pixel values (5-ft square ≈ 7 px → 17 px). */
const FADE_START = 0.7;
const FADE_END = 0.3;
/** Chunks are requested a little before they fade in. */
const REQUEST_BELOW = 1.2;

interface ChunkView {
  id: string;
  /** The edits epoch it was made with. */
  epoch: number;
  x: number;
  y: number;
  /** One holder per draw layer (ground, low, shadows, tall, atmosphere), same transform. */
  parts: Container[];
  ground: Mesh<Geometry, Shader>;
  uniforms: UniformGroup;
  textures: Texture[];
  /** Its heights texture: what it was made with (half floats less `base`), and while the ground
   * is previewed there (`previewGround`), the copy shown instead. */
  hSrc: BufferImageSource;
  hdata: Uint16Array;
  base: number;
  shown: { data: Uint16Array; dirty: boolean } | null;
  /** The roof mesh's vertex buffers (a mesh does not free its geometry). */
  roofGeometry: Geometry | null;
  /** The grids over bridge and pier decks, wall walks, tower tops and gravel walks, shown and hidden
   * with the ground's. */
  grids: Graphics[];
  /** Bridge and pier decks (and their grid): opaque whenever the ground is drawn. */
  decks: Container;
  atmo: Mesh<Geometry, Shader> | null;
  atmoUniforms: UniformGroup | null;
  atmosphere: number;
  lastUsed: number;
  /** Tactical data kept for inspection (HUD, play mode). */
  objects: number;
  /** Per square: surface, edges, tier, building (the ground texture's data). */
  data: Uint8Array;
  /** Per square: play mode's flags (`play/vision`), from the worker. */
  flags: Uint8Array;
  tier: Int16Array;
  surface: Uint8Array;
  heights: Float32Array;
  /** Per square: height (ft) of what stands on the drawn ground, and its `URBAN_*` kind. */
  raised: Uint8Array | null;
  sea: number;
  objs: ChunkObject[];
  /** Uploaded sprites its objects use. */
  sprites: ChunkSprite[];
  /** What it was built from, kept while a sprite's picture is loading (to build it again). */
  prepared: PreparedChunk | null;
}

/** An object as a chunk holds it (squares within the chunk). */
interface ChunkObject {
  kind: number;
  variant: number;
  x: number;
  y: number;
  rot: number;
  scale: number;
  /** Drawn at `rot` (`Placed.turned`). */
  turned: boolean;
}

/** The object under a point (`objectAt`): its kind (built-in id, or `s:<asset>`), centre (world ft)
 * and look. */
export interface ObjectHit {
  kind: number | string;
  x: number;
  y: number;
  variant: number;
  rot: number;
  scale: number;
  turned: boolean;
}

export interface SquareInfo {
  elevationFt: number;
  tier: number;
  surface: string;
  object: { name: string; cover: string; notes: string } | null;
}

const SURFACES = [
  'grass',
  'forest floor',
  'dry grass',
  'sand',
  'snow',
  'rock',
  'mud',
  'ash',
  'salt crust',
  'dirt',
  'shallow water',
  'deep water (swim)',
  'lava',
  'ice',
  "king's road (cobbles)",
  'road',
  'bridge deck',
  'building',
  'field (crops)',
  'stairs',
  'gravel path',
];
/** The most flights of steps' axes one chunk's ground draws treads for (`ground.frag`; `battlemap::MAX_STAIR_AXES`). */
const STAIR_AXES = 64;
/** `URBAN_*` kinds that stand on the ground (battlemap.rs), as hover labels. */
const RAISED: Record<number, string> = { 6: 'bridge deck', 7: 'wall walk', 8: 'tower top' };

const COVER = ['no cover', 'half cover', 'three-quarters cover', 'total cover'];

/** Draw layers across all chunks: objects overhanging a chunk edge must not be painted over
 * by the neighbouring chunk's ground. */
const LAYERS = 5;

export class BattlemapLayer {
  readonly container = new Container();
  private readonly layers: Container[] = Array.from({ length: LAYERS }, () => new Container());
  readonly frame = new UniformGroup({
    uTime: { value: 0, type: 'f32' },
    uGrid: { value: 1, type: 'f32' },
  });
  stats = { chunks: 0, alpha: 0 };

  private readonly chunks = new Map<string, ChunkView>();
  private readonly received = new Map<string, { x: number; y: number; chunk: PreparedChunk; epoch: number }>();
  private readonly edited = new Edited();
  /** Uploaded sprites' names and rules (`Edits.sprites`). */
  sprites: Record<string, SpriteMeta> = {};
  /** Objects just put down, drawn over their chunk until it comes back with them. */
  private readonly previews = new Map<string, { holder: Container; epoch: number }>();
  /** What is in the hand (`setGhost`): its sprite, where it is (world ft) and its scale (atlas px). */
  private ghost: { sprite: Sprite; at: [number, number]; scale: number } | null = null;
  private readonly unlisten: () => void;
  /** A chunk being built a stage per frame (its parts are drawn once off screen as they are
   * made, so their uploads and tessellation don't all land in one frame). */
  private building: Generator<void, void, void> | null = null;
  private warmTarget: RenderTexture | null = null;
  private atlas: Atlas | null = null;
  private readonly groundProgram = GlProgram.from({ vertex, fragment: groundFragment, name: 'battle-ground' });
  private readonly atmoProgram = GlProgram.from({ vertex, fragment: atmoFragment, name: 'battle-atmo' });
  private readonly colorMeshProgram = GlProgram.from({ vertex: colorMeshVertex, fragment: colorMeshFragment, name: 'battle-roofs' });
  private readonly quad = new Geometry({
    attributes: {
      aPosition: { buffer: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), format: 'float32x2' },
      aUV: { buffer: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), format: 'float32x2' },
    },
    indexBuffer: new Uint32Array([0, 1, 2, 0, 2, 3]),
  });

  /** `player`: the players' window (hidden hazards are left out). */
  constructor(
    private readonly geom: Geom,
    private readonly renderer: Renderer,
    readonly catalog: KindInfo[],
    private readonly player = false,
  ) {
    this.container.addChild(...this.layers);
    // A sprite's picture arrived: chunks built without it are built again.
    this.unlisten = onLoaded((asset) => {
      for (const c of this.chunks.values()) {
        if (c.prepared && c.sprites.some((s) => s.asset === asset) && !this.received.has(c.id)) this.received.set(c.id, { x: c.x, y: c.y, chunk: c.prepared, epoch: c.epoch });
      }
    });
  }

  /** Draw objects just put down (world ft) over their chunks at once, until each chunk comes
   * back from the generators with them (`epoch`: the edit that put them there). */
  preview(objects: Placed[], epoch: number) {
    const atlas = this.atlas;
    if (!atlas) return;
    const ts = this.geom.domain_ft / 2 ** this.geom.max_level;
    for (const o of objects) {
      const [cx, cy] = [Math.floor(o.x / ts), Math.floor(o.y / ts)];
      const id = `${cx}/${cy}`;
      const look = this.objectLook(o);
      if (!look) continue;
      let p = this.previews.get(id);
      if (!p) {
        const holder = new Container();
        this.layers[3].addChild(holder);
        p = { holder, epoch };
        this.previews.set(id, p);
      }
      p.epoch = Math.max(p.epoch, epoch);
      const s = new Sprite(look.texture);
      s.anchor.set(0.5);
      s.position.set(((o.x - cx * ts) / SQUARE_FT) * PX, ((o.y - cy * ts) / SQUARE_FT) * PX);
      s.rotation = look.rotation;
      s.scale.set(look.scale);
      p.holder.addChild(s);
    }
  }

  /** How an object put down by hand is drawn: its picture, turn and scale (atlas px), or null
   * while its picture isn't there. */
  private objectLook(o: Placed): { texture: Texture; rotation: number; scale: number } | null {
    const atlas = (this.atlas ??= sharedAtlas(this.renderer, this.catalog));
    if (typeof o.kind === 'string') {
      const asset = o.kind.slice(2);
      const texture = customTexture(asset);
      if (!texture) return null;
      const size = this.sprites[asset]?.size ?? 1;
      return { texture, rotation: o.rot, scale: (size * PX * o.scale) / Math.max(texture.width, texture.height) };
    }
    const texture = atlas.frames[o.kind]?.[o.variant % looks(o.kind)];
    if (!texture) return null;
    return { texture, rotation: drawnRotation(this.catalog[o.kind - 1], !!o.turned, o.rot), scale: o.scale };
  }

  /** What is in the hand, drawn where it would land (world ft): one sprite over the objects,
   * reddened where it can't go (`ok` false). Null takes it away. */
  setGhost(o: Placed | null, ok = true) {
    const look = o && this.objectLook(o);
    if (!o || !look) {
      if (this.ghost) this.ghost.sprite.visible = false;
      return;
    }
    if (!this.ghost) {
      const sprite = new Sprite(look.texture);
      sprite.anchor.set(0.5);
      this.ghost = { sprite, at: [o.x, o.y], scale: 1 };
    }
    const g = this.ghost;
    g.sprite.texture = look.texture;
    g.sprite.rotation = look.rotation;
    g.sprite.tint = ok ? 0xffffff : 0xff7a6a;
    g.sprite.alpha = ok ? 0.85 : 0.75;
    g.sprite.visible = true;
    g.scale = look.scale;
    g.at = [o.x, o.y];
    // Over the tall objects (chunks built since are added under it).
    if (this.layers[3].children[this.layers[3].children.length - 1] !== g.sprite) this.layers[3].addChild(g.sprite);
  }

  private dropPreview(id: string, epoch: number) {
    const p = this.previews.get(id);
    if (p && epoch >= p.epoch) {
      p.holder.destroy({ children: true });
      this.previews.delete(id);
    }
  }

  receive(level: number, x: number, y: number, chunk: PreparedChunk, epoch: number) {
    // One made before an edit here is dropped: a new one is on its way.
    if (level === this.geom.max_level && !this.stale(x, y, epoch)) this.received.set(`${x}/${y}`, { x, y, chunk, epoch });
  }

  /** Created sites changed within `rects`: chunks there are made again (and swapped in). */
  invalidate(rects: Rect[], epoch: number) {
    this.edited.add(rects, 0, epoch);
    for (const [id, r] of this.received) if (this.stale(r.x, r.y, r.epoch)) this.received.delete(id);
  }

  private stale(x: number, y: number, epoch: number): boolean {
    const L = this.geom.max_level;
    return this.edited.staleAt(L, x, y, this.geom.domain_ft / 2 ** L, epoch);
  }

  /**
   * Pay one-time costs up front (atlas drawing + upload, shader compilation) by rendering a
   * hidden sample chunk offscreen, instead of stalling the first time a battlemap appears.
   */
  warmup() {
    this.atlas ??= sharedAtlas(this.renderer, this.catalog);
    const n = SQ * SQ;
    const nh = HS * HS;
    // ..., one object, road halo, buildings (one, for the roof shader).
    const buf = new ArrayBuffer(32 + nh * 8 + n * 4 + 20 + nh + n * 2 + 4 + 4 + 3 * 8);
    const dv = new DataView(buf);
    dv.setUint32(16, 1, true); // one object
    dv.setUint32(20, 1, true); // mist, to compile the atmosphere shader
    const o = 32 + nh * 8 + n * 4;
    dv.setUint16(o, 1, true);
    dv.setFloat32(o + 16, 1, true);
    const r = o + 20 + nh + n * 2;
    dv.setUint32(r, 1, true);
    dv.setUint8(r + 4, 3);
    dv.setFloat32(r + 16, 1, true);
    dv.setFloat32(r + 24, 1, true);
    dv.setFloat32(r + 28, 1, true);
    // (All stages at once: nothing is on screen yet.)
    const steps = this.build('warmup', 0, 0, prepareChunk(buf, 0, 0, this.catalog), 0, 0);
    while (!steps.next().done);
    const c = this.chunks.get('warmup')!;
    const target = RenderTexture.create({ width: 16, height: 16 });
    for (const p of c.parts) this.renderer.render({ container: p, target });
    target.destroy(true);
    this.chunks.delete('warmup');
    this.destroyChunk(c);
  }

  /** Every built-in kind's picture (its first variant) on one sheet, keyed by kind id. */
  thumbs(): Promise<ThumbSheet> {
    return sheetFor(this.renderer, `battle:${this.catalog.map((k) => k.name).join('|')}`, () => {
      const atlas = (this.atlas ??= sharedAtlas(this.renderer, this.catalog));
      const tiles = this.catalog.flatMap((k) => (atlas.frames[k.id]?.[0] ? [{ key: String(k.id), node: new Sprite(atlas.frames[k.id][0]) }] : []));
      return thumbSheet(this.renderer, tiles);
    });
  }

  /** Whether the chunk under a world position is built (objects there can be seen and found). */
  loadedAt(wx: number, wy: number): boolean {
    const ts = this.geom.domain_ft / 2 ** this.geom.max_level;
    return this.chunks.has(`${Math.floor(wx / ts)}/${Math.floor(wy / ts)}`);
  }

  /** Whether a world position is under a roof (a building's or a tower's square, its chunk
   * loaded): an object put down there stands under it. */
  roofAt(wx: number, wy: number): boolean {
    const ts = this.geom.domain_ft / 2 ** this.geom.max_level;
    const cx = Math.floor(wx / ts);
    const cy = Math.floor(wy / ts);
    const c = this.chunks.get(`${cx}/${cy}`);
    if (!c) return false;
    const k = Math.floor((wy - cy * ts) / SQUARE_FT) * SQ + Math.floor((wx - cx * ts) / SQUARE_FT);
    if (k < 0 || k >= SQ * SQ) return false;
    return c.data[k * 4 + 3] !== 0 || (!!c.raised && c.raised[k * 2] > 0 && c.raised[k * 2 + 1] === 8);
  }

  /** The object whose footprint holds a world position (the nearest centre), if its chunk is
   * loaded: its kind (built-in id, or `s:<asset>`), centre (world ft) and look. */
  objectAt(wx: number, wy: number): ObjectHit | null {
    const ts = this.geom.domain_ft / 2 ** this.geom.max_level;
    let best: ObjectHit | null = null;
    let bestD = Infinity;
    // Footprints reach over chunk edges (the widest about 2.5 squares from a centre): the
    // neighbours' too, where the point is that near one.
    const [cx0, cy0] = [Math.floor(wx / ts), Math.floor(wy / ts)];
    const near = 2.5 * SQUARE_FT;
    const [fx, fy] = [wx - cx0 * ts, wy - cy0 * ts];
    const [xa, xb] = [fx < near ? -1 : 0, fx > ts - near ? 1 : 0];
    const [ya, yb] = [fy < near ? -1 : 0, fy > ts - near ? 1 : 0];
    for (let cy = cy0 + ya; cy <= cy0 + yb; cy++) {
      for (let cx = cx0 + xa; cx <= cx0 + xb; cx++) {
        const c = this.chunks.get(`${cx}/${cy}`);
        if (!c) continue;
        const [sx, sy] = [(wx - cx * ts) / SQUARE_FT, (wy - cy * ts) / SQUARE_FT];
        for (const o of c.objs) {
          const inf = kindRules(o.kind, this.catalog, c.sprites);
          if (!inf) continue;
          const d = Math.hypot(o.x - sx, o.y - sy);
          if (d <= Math.max(0.5, inf.radius * o.scale) && d < bestD) {
            bestD = d;
            const kind = o.kind >= SPRITE_KIND ? `s:${c.sprites[o.kind - SPRITE_KIND].asset}` : o.kind;
            best = { kind, x: cx * ts + o.x * SQUARE_FT, y: cy * ts + o.y * SQUARE_FT, variant: o.variant, rot: o.rot, scale: o.scale, turned: o.turned };
          }
        }
      }
    }
    return best;
  }

  /** A square's drawn ground and the water there (ft; global square), if its chunk is loaded. */
  groundAt(gx: number, gy: number): [number, number] | null {
    const [cx, cy] = [Math.floor(gx / SQ), Math.floor(gy / SQ)];
    const c = this.chunks.get(`${cx}/${cy}`);
    if (!c) return null;
    const k = (gy - cy * SQ + 1) * HS + gx - cx * SQ + 1;
    return [c.heights[k], fromHalf(c.hdata[k * 4 + 1]) + c.base];
  }

  /**
   * Show the ground of squares (global, an inclusive box) as it will be, before the chunks are
   * made again: `at` gives a square's height from its drawn height and water, or null to leave
   * it. Chunks show a copy of their heights (the halo rings too, so seams stay shut), uploaded
   * once a frame; a chunk made again shows its own.
   */
  previewGround(box: [number, number, number, number], at: (gx: number, gy: number, h: number, water: number) => number | null) {
    const [gx0, gy0, gx1, gy1] = box;
    for (let cy = Math.floor((gy0 - 1) / SQ); cy <= Math.floor((gy1 + 1) / SQ); cy++) {
      for (let cx = Math.floor((gx0 - 1) / SQ); cx <= Math.floor((gx1 + 1) / SQ); cx++) {
        const c = this.chunks.get(`${cx}/${cy}`);
        if (!c) continue;
        const [ox, oy] = [cx * SQ - 1, cy * SQ - 1];
        const [ax, bx] = [Math.max(gx0, ox), Math.min(gx1, ox + HS - 1)];
        const [ay, by] = [Math.max(gy0, oy), Math.min(gy1, oy + HS - 1)];
        for (let gy = ay; gy <= by; gy++) {
          for (let gx = ax; gx <= bx; gx++) {
            const k = (gy - oy) * HS + gx - ox;
            const v = at(gx, gy, c.heights[k], fromHalf(c.hdata[k * 4 + 1]) + c.base);
            if (v === null || !Number.isFinite(v)) continue;
            if (!c.shown) {
              c.shown = { data: c.hdata.slice(), dirty: true };
              c.hSrc.resource = c.shown.data;
            }
            c.shown.data[k * 4] = toHalf(v - c.base);
            c.shown.dirty = true;
          }
        }
      }
    }
  }

  /** Every chunk shows its own heights again. */
  unpreviewGround() {
    for (const c of this.chunks.values()) {
      if (!c.shown) continue;
      c.shown = null;
      c.hSrc.resource = c.hdata;
      c.hSrc.update();
    }
  }

  /** Tactical info for the square at a world position, if its chunk is loaded. */
  inspect(wx: number, wy: number): SquareInfo | null {
    const ts = this.geom.domain_ft / 2 ** this.geom.max_level;
    const cx = Math.floor(wx / ts);
    const cy = Math.floor(wy / ts);
    const c = this.chunks.get(`${cx}/${cy}`);
    if (!c) return null;
    const sx = (wx - cx * ts) / SQUARE_FT;
    const sy = (wy - cy * ts) / SQUARE_FT;
    const k = Math.floor(sy) * SQ + Math.floor(sx);
    if (k < 0 || k >= SQ * SQ) return null;
    let object: SquareInfo['object'] = null;
    let best = Infinity;
    for (const o of c.objs) {
      const custom = o.kind >= SPRITE_KIND ? c.sprites[o.kind - SPRITE_KIND] : null;
      const inf = custom ? { ...custom, name: this.sprites[custom.asset]?.name || 'custom object', hazard: null } : this.catalog[o.kind - 1];
      if (!inf) continue;
      const d = Math.hypot(o.x - sx, o.y - sy);
      if (d <= Math.max(0.5, inf.radius * o.scale) && d < best) {
        best = d;
        const notes = [
          inf.blocks_move ? 'blocks movement' : '',
          inf.blocks_sight ? 'blocks sight' : '',
          inf.difficult ? 'difficult terrain' : '',
          inf.height_ft >= 5 ? `${Math.round(inf.height_ft * o.scale)} ft tall` : '',
          inf.hazard ? inf.hazard.effect : '',
        ].filter(Boolean);
        object = { name: inf.name, cover: COVER[inf.cover], notes: notes.join(' · ') };
      }
    }
    // Walls, towers and decks stand on the drawn ground: report their top.
    const lift = c.raised ? c.raised[k * 2] : 0;
    const on = c.raised ? RAISED[c.raised[k * 2 + 1]] : undefined;
    return {
      elevationFt: Math.round(c.heights[(Math.floor(sy) + 1) * HS + Math.floor(sx) + 1] - c.sea + (on ? lift : 0)),
      tier: c.tier[k],
      surface: on && lift > 0 ? on : (SURFACES[c.surface[k]] ?? 'ground'),
      object,
    };
  }

  /**
   * The surface as play mode sees it, square by square (global 5-ft squares): what blocks
   * sight and movement, difficult ground and its height; squares of chunks not in memory are
   * `UNKNOWN` (and open).
   */
  grid(): TacticalGrid {
    let [lx, ly] = [NaN, NaN];
    let last: ChunkView | null = null;
    const at = (cx: number, cy: number) => {
      if (cx !== lx || cy !== ly) {
        [lx, ly] = [cx, cy];
        last = this.chunks.get(`${cx}/${cy}`) ?? null;
      }
      return last;
    };
    return {
      flags: (i, j) => {
        const [cx, cy] = [Math.floor(i / SQ), Math.floor(j / SQ)];
        const c = at(cx, cy);
        return c ? c.flags[(j - cy * SQ) * SQ + i - cx * SQ] : UNKNOWN;
      },
      elev: (i, j) => {
        const [cx, cy] = [Math.floor(i / SQ), Math.floor(j / SQ)];
        const c = at(cx, cy);
        return c ? c.heights[(j - cy * SQ + 1) * HS + i - cx * SQ + 1] : NaN;
      },
      wallW: () => false,
      wallN: () => false,
      bounds: null,
      noWalls: true,
      // Whole runs of a row from each chunk.
      fill: (x0, y0, w, h, flags, elev) => {
        for (let j = 0; j < h; j++) {
          const gj = y0 + j;
          const cy = Math.floor(gj / SQ);
          const ly = gj - cy * SQ;
          for (let i = 0; i < w; ) {
            const gi = x0 + i;
            const cx = Math.floor(gi / SQ);
            const lx = gi - cx * SQ;
            const run = Math.min(w - i, SQ - lx);
            const c = at(cx, cy);
            const o = j * w + i;
            if (c) {
              flags.set(c.flags.subarray(ly * SQ + lx, ly * SQ + lx + run), o);
              const r = (ly + 1) * HS + lx + 1;
              elev.set(c.heights.subarray(r, r + run), o);
            } else {
              flags.fill(UNKNOWN, o, o + run);
              elev.fill(NaN, o, o + run);
            }
            i += run;
          }
        }
      },
    };
  }

  /** Whether opaque battlemap chunks cover the whole view (the map beneath need not draw). */
  covering = false;
  /** Chunks built in the last update (for the frame profile). */
  lastBuilt = 0;

  /** Chunks received and not yet built (or being built). */
  get pending(): boolean {
    return !!this.building || this.received.size > 0;
  }

  get gridShown(): boolean {
    return this.frame.uniforms.uGrid > 0;
  }

  /** Show the 5-ft grid (over the ground and bridge and pier decks) or not. */
  setGrid(on: boolean) {
    this.frame.uniforms.uGrid = on ? 1 : 0;
    for (const ch of this.chunks.values()) for (const g of ch.grids) g.visible = on;
  }

  /** Returns the battlemap chunks still wanted. */
  update(cam: Camera, now: number): WantTile[] {
    const ftPerPx = 1 / cam.ppf;
    const alpha = Math.min(1, Math.max(0, (FADE_START - ftPerPx) / (FADE_START - FADE_END)));
    this.stats = { chunks: this.chunks.size, alpha };
    this.frame.uniforms.uTime = now / 1000;
    this.covering = false;
    this.lastBuilt = 0;
    if (ftPerPx > REQUEST_BELOW) {
      this.container.visible = false;
      return [];
    }
    this.container.visible = alpha > 0;

    const L = this.geom.max_level;
    const ts = this.geom.domain_ft / 2 ** L;
    const [vx0, vy0, vx1, vy1] = cam.viewRect();
    const margin = 0.25 * ts;
    const nx = Math.ceil(this.geom.map_w_ft / ts);
    const ny = Math.ceil(this.geom.map_h_ft / ts);
    const x0 = Math.max(0, Math.floor((vx0 - margin) / ts));
    const y0 = Math.max(0, Math.floor((vy0 - margin) / ts));
    const x1 = Math.min(nx - 1, Math.floor((vx1 + margin) / ts));
    const y1 = Math.min(ny - 1, Math.floor((vy1 + margin) / ts));
    const want: WantTile[] = [];
    const inView = new Set<string>();
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        const id = `${x}/${y}`;
        inView.add(id);
        const have = this.chunks.get(id);
        if ((!have || this.stale(x, y, have.epoch)) && !this.received.has(id)) {
          const d = Math.hypot(x + 0.5 - cam.cx / ts, y + 0.5 - cam.cy / ts);
          want.push({ kind: 'battlemap', level: L, x, y, pri: L * 100 + 50 + d });
        }
      }
    }

    // Build received chunks a stage per frame: one chunk at a time, about 20 ms of setup,
    // uploads and tessellation spread over up to ten frames.
    if (!this.building) {
      for (const [id, r] of this.received) {
        this.received.delete(id);
        const have = this.chunks.get(id);
        if (inView.has(id) && (!have || this.stale(r.x, r.y, have.epoch) || r.chunk === have.prepared)) {
          this.building = this.build(id, r.x, r.y, r.chunk, r.epoch, now);
          break;
        }
      }
    }
    if (this.building) {
      this.lastBuilt = 1;
      if (this.building.next().done) this.building = null;
    }

    const pxPerSq = SQUARE_FT * cam.ppf;
    for (const [id, p] of this.previews) {
      const [x, y] = id.split('/').map(Number);
      const [sx, sy] = cam.worldToScreen(x * ts, y * ts);
      p.holder.position.set(sx, sy);
      p.holder.scale.set(pxPerSq / PX);
      p.holder.alpha = alpha;
    }
    if (this.ghost?.sprite.visible) {
      const g = this.ghost;
      g.sprite.position.set(...cam.worldToScreen(g.at[0], g.at[1]));
      g.sprite.scale.set((g.scale * pxPerSq) / PX);
    }
    for (const c of this.chunks.values()) {
      if (c.shown?.dirty) {
        c.shown.dirty = false;
        c.hSrc.update();
      }
      const show = inView.has(c.id);
      for (const p of c.parts) p.visible = show;
      if (!show) continue;
      c.lastUsed = now;
      const [sx, sy] = cam.worldToScreen(c.x * ts, c.y * ts);
      for (const p of c.parts) {
        p.position.set(sx, sy);
        p.scale.set(pxPerSq / PX);
        p.alpha = p === c.decks ? 1 : alpha;
      }
      const u = c.uniforms.uniforms;
      u.uAlpha = 1;
      u.uPxPerSq = pxPerSq;
      if (c.atmoUniforms) {
        const au = c.atmoUniforms.uniforms;
        au.uAlpha = alpha;
        // Fade toward neighbours with a different atmosphere, so there is no chunk-shaped edge.
        const same = (dx: number, dy: number) => this.chunks.get(`${c.x + dx}/${c.y + dy}`)?.atmosphere === c.atmosphere;
        const e = au.uEdges as Float32Array;
        e[0] = same(-1, 0) ? 1 : 0;
        e[1] = same(1, 0) ? 1 : 0;
        e[2] = same(0, -1) ? 1 : 0;
        e[3] = same(0, 1) ? 1 : 0;
      }
    }
    // Opaque chunks over the whole view hide everything drawn beneath them.
    if (alpha >= 1) {
      let all = true;
      for (let y = Math.max(0, Math.floor(vy0 / ts)); all && y <= Math.min(ny - 1, Math.floor(vy1 / ts)); y++) {
        for (let x = Math.max(0, Math.floor(vx0 / ts)); all && x <= Math.min(nx - 1, Math.floor(vx1 / ts)); x++) {
          all = this.chunks.has(`${x}/${y}`);
        }
      }
      this.covering = all;
    }
    this.evict();
    return want;
  }

  destroy() {
    this.unlisten();
    for (const p of this.previews.values()) p.holder.destroy({ children: true });
    this.previews.clear();
    this.warmTarget?.destroy(true);
    for (const c of this.chunks.values()) this.destroyChunk(c);
    this.chunks.clear();
    this.container.destroy({ children: true });
  }

  /** Draw a freshly built part once into a tiny target: its textures and buffers upload and
   * its graphics tessellate now, not in the frame that first shows it. */
  private prewarm(...parts: (Container | null)[]) {
    this.warmTarget ??= RenderTexture.create({ width: 2, height: 2 });
    for (const p of parts) if (p) this.renderer.render({ container: p, target: this.warmTarget, clear: false });
  }

  private *build(id: string, cx: number, cy: number, c: PreparedChunk, epoch: number, now: number): Generator<void, void, void> {
    this.atlas ??= sharedAtlas(this.renderer, this.catalog);
    const atlas = this.atlas;
    const { data, hdata, base, atmosphere, nObj } = c;
    const dataSrc = new BufferImageSource({ resource: data, width: SQ, height: SQ, format: 'rgba8unorm', alphaMode: 'no-premultiply-alpha', scaleMode: 'nearest' });
    const hSrc = new BufferImageSource({ resource: hdata, width: HS, height: HS, format: 'rgba16float', alphaMode: 'no-premultiply-alpha', scaleMode: 'linear', addressMode: 'clamp-to-edge' });
    const textures = [new Texture({ source: dataSrc }), new Texture({ source: hSrc })];

    const noiseOff = new Float32Array([((cx * SQ) % 512 + 512) % 512, ((cy * SQ) % 512 + 512) % 512]);
    // Flights of steps crossing the chunk (shape 8): per axis (bottom, top) and (half width,
    // start up the flight, tread, 0) in squares, for the ground's treads.
    const stairs = new Float32Array(STAIR_AXES * 8);
    let nStairs = 0;
    for (const s of c.shapes) {
      if (s.kind !== 8 || nStairs === STAIR_AXES) continue;
      const p = s.pts.map((v) => v / PX);
      stairs.set([p[0], p[1], p[2], p[3], (0.5 * s.size) / PX, p[4], p[5], 0], nStairs++ * 8);
    }
    const uniforms = new UniformGroup({
      uBase: { value: base, type: 'f32' },
      uAlpha: { value: 1, type: 'f32' },
      uPxPerSq: { value: 16, type: 'f32' },
      uNoiseOff: { value: noiseOff, type: 'vec2<f32>' },
      uSea: { value: c.sea, type: 'f32' },
      uStairs: { value: stairs, type: 'vec4<f32>', size: STAIR_AXES * 2 },
      uStairN: { value: nStairs, type: 'f32' },
      // (Gravel here, or park ground by the edge, whose neighbour's gravel the halo may hold.)
      uGravel: { value: c.surface.includes(20) || parkEdge(c.raised) ? 1 : 0, type: 'f32' },
    });
    const ground = new Mesh({
      geometry: this.quad,
      shader: new Shader({ glProgram: this.groundProgram, resources: { uData: dataSrc, uHeights: hSrc, uNoise: noiseSource(), chunk: uniforms, battleFrame: this.frame } }),
    });
    ground.scale.set(SQ * PX);
    this.prewarm(ground);
    yield;

    // Objects (sorted in the worker): flat things and hazards, then shadows, then tall things.
    const objs: ChunkObject[] = [];
    // Uploaded sprites: a particle container each (low or tall), only where a chunk has them.
    const customs = new Map<string, ParticleContainer>();
    let waiting = false;
    // Fully static particles: the chunk moves as a whole, so nothing is re-uploaded per frame
    // (Pixi's default re-uploads every position, every frame).
    const fixed = { vertex: false, position: false, rotation: false, uvs: false, color: false };
    const low = new ParticleContainer({ texture: atlas.shadow, dynamicProperties: fixed });
    const shadows = new ParticleContainer({ texture: atlas.shadow, dynamicProperties: fixed });
    const tall = new ParticleContainer({ texture: atlas.shadow, dynamicProperties: fixed });
    for (let i = 0; i < nObj; i++) {
      const [kind, look, x, y, rot, scale] = c.objs.subarray(i * 6, i * 6 + 6);
      // The variant byte, and the turned flag above it (`battlePrep`).
      const [variant, turned] = [look & 255, look >> 8 === 1];
      const o = { kind, x, y, rot, scale };
      objs.push({ kind, variant, x, y, rot, scale, turned });
      const inf = kindRules(o.kind, this.catalog, c.sprites);
      if (!inf) continue;
      const custom = o.kind >= SPRITE_KIND ? c.sprites[o.kind - SPRITE_KIND] : null;
      const tex = custom ? customTexture(custom.asset) : atlas.frames[o.kind]?.[variant % looks(o.kind)];
      if (!tex) {
        waiting ||= !!custom;
        continue;
      }
      const hazard = 'hazard' in inf ? inf.hazard : null;
      if (this.player && dmOnlyHazard(hazard?.effect)) continue;
      const isTall = inf.height_ft >= 5;
      const rotation = custom ? o.rot : drawnRotation(inf as KindInfo, turned, o.rot);
      if (inf.height_ft >= 1.5 && !hazard && (custom || !OWN_SHADOW.has((inf as KindInfo).name))) {
        const [size, strength] = (!custom && SPARSE.get((inf as KindInfo).name)) || [1, 1];
        const r = inf.radius * PX * o.scale * size;
        // Upright things drawn from a high angle cast it from their foot; long ones a long shadow along
        // them (still falling SE, whatever their turn), a bridge's further off (it stands over the water).
        const foot = custom ? 0 : (UPRIGHT_FOOT[o.kind] ?? 0) * o.scale;
        const long = !custom && LONG_SHADOW.get((inf as KindInfo).name);
        const off = Math.min(1.3, 0.15 + inf.height_ft / 45) * PX * 0.7 * (long ? long[2] : 1);
        shadows.addParticle(
          new Particle({
            texture: atlas.shadow,
            x: o.x * PX + off,
            y: o.y * PX + foot + off,
            anchorX: 0.5,
            anchorY: 0.5,
            scaleX: long ? (long[0] * PX * o.scale) / 128 : (r * 2.1) / 128,
            scaleY: long ? (long[1] * PX * o.scale) / 128 : (r * 2.1) / 128,
            rotation: long ? rotation : 0,
            alpha: (isTall ? 0.6 : 0.35) * strength,
          }),
        );
      }
      // Lit sprites keep their NW highlights; logs, bones and wall segments follow their line,
      // market stalls and carts face their row's aisle.
      if (custom) {
        const key = `${custom.asset}/${isTall ? 1 : 0}`;
        let pc = customs.get(key);
        if (!pc) customs.set(key, (pc = new ParticleContainer({ texture: tex, dynamicProperties: fixed })));
        const k = (custom.radius * 2 * PX * o.scale) / Math.max(tex.width, tex.height);
        pc.addParticle(new Particle({ texture: tex, x: o.x * PX, y: o.y * PX, anchorX: 0.5, anchorY: 0.5, scaleX: k, scaleY: k, rotation: o.rot }));
        continue;
      }
      (isTall ? tall : low).addParticle(
        new Particle({ texture: tex, x: o.x * PX, y: o.y * PX, anchorX: 0.5, anchorY: 0.5, scaleX: o.scale, scaleY: o.scale, rotation }),
      );
    }

    yield;
    for (const layer of [low, shadows, tall]) {
      this.prewarm(layer);
      yield;
    }
    for (const pc of customs.values()) this.prewarm(pc);

    // Roof shadows and pitched roofs as one prebuilt mesh; battlemented roofs over it.
    let roofMesh: Mesh<Geometry, Shader> | null = null;
    let roofGeometry: Geometry | null = null;
    if (c.roofMesh) {
      const geometry = (roofGeometry = new Geometry({
        attributes: {
          aPos: { buffer: c.roofMesh.pos, format: 'float32x2' },
          aColor: { buffer: c.roofMesh.color, format: 'float32x4' },
        },
        indexBuffer: c.roofMesh.index,
      }));
      roofMesh = new Mesh({ geometry, shader: new Shader({ glProgram: this.colorMeshProgram, resources: {} }) });
    }
    // Daises and a court's gravel walks sit on the ground, under every prop.
    const daisG = new Graphics();
    for (const s of c.shapes) {
      if (s.kind === 5) drawDais(daisG, s);
      else if (s.kind === 13) drawGravelWalk(daisG, s);
    }
    const battleG = new Graphics();
    for (const r of c.battlements) drawBattlements(battleG, r);
    // Bridges and piers, the grid over their decks (shown with the ground's), then walls with
    // their shadows; towers stand above every chunk's walls.
    const bridgeG = new Graphics();
    const gridG = new Graphics();
    for (const s of c.shapes) {
      if (s.kind === 2) drawDeck(bridgeG, s);
      else if (s.kind === 3) drawRoadBridge(bridgeG, s);
      else if (s.kind === 6) {
        drawFord(bridgeG, s);
        continue;
      } else if (s.kind === 7) {
        drawFerry(bridgeG, s);
        continue;
      } else continue;
      deckGrid(gridG, s);
    }
    const structG = drawStructures(c.shapes.filter((s) => s.kind === 0 || s.kind === 4 || s.kind === 9 || (s.kind >= 10 && s.kind <= 12)));
    const towerG = drawStructures(c.shapes.filter((s) => s.kind === 1));
    // The grid on the wall walks (over the walls), the tower tops (over the towers) and the gravel
    // walks (over them, on the ground).
    const walkGridG = new Graphics();
    const topGridG = new Graphics();
    const pathGridG = new Graphics();
    for (const s of c.shapes) topGrid(s.kind === 1 ? topGridG : s.kind === 13 ? pathGridG : walkGridG, s);
    for (const gr of [gridG, walkGridG, topGridG, pathGridG]) gr.visible = this.frame.uniforms.uGrid > 0;
    for (const part of [roofMesh, daisG, pathGridG, battleG, bridgeG, gridG, structG, walkGridG, towerG, topGridG]) {
      if (!part) continue;
      this.prewarm(part);
      yield;
    }
    const parts = [ground, low, shadows, tall].map((o, i) => {
      const holder = new Container();
      holder.addChild(o);
      for (const [key, pc] of customs) if ((i === 1 && key.endsWith('/0')) || (i === 3 && key.endsWith('/1'))) holder.addChild(pc);
      if (i === 0) holder.addChild(daisG, pathGridG);
      if (i === 2) {
        holder.addChild(structG, walkGridG);
        if (roofMesh) holder.addChild(roofMesh);
        holder.addChild(battleG);
      }
      if (i === 3) holder.addChild(towerG, topGridG);
      this.layers[i].addChild(holder);
      return holder;
    });
    // Bridge and pier decks carry the road drawn on the ground: they show as soon as the
    // ground does (the rest fades in), under the shadows and walls.
    const decks = new Container();
    decks.addChild(bridgeG, gridG);
    this.layers[2].addChildAt(decks, this.layers[2].getChildIndex(parts[2]));
    parts.push(decks);
    let atmo: Mesh<Geometry, Shader> | null = null;
    let atmoUniforms: UniformGroup | null = null;
    if (atmosphere > 0) {
      atmoUniforms = new UniformGroup({
        uKind: { value: atmosphere, type: 'f32' },
        uEdges: { value: new Float32Array(4), type: 'vec4<f32>' },
        uAlpha: { value: 1, type: 'f32' },
        uNoiseOff: { value: noiseOff, type: 'vec2<f32>' },
      });
      atmo = new Mesh({
        geometry: this.quad,
        shader: new Shader({ glProgram: this.atmoProgram, resources: { atmo: atmoUniforms, uNoise: noiseSource(), battleFrame: this.frame } }),
      });
      atmo.scale.set(SQ * PX);
      const holder = new Container();
      holder.addChild(atmo);
      this.layers[4].addChild(holder);
      parts.push(holder);
    }
    // An edited chunk replaces the one it updates.
    const old = this.chunks.get(id);
    if (old) this.destroyChunk(old);
    this.chunks.set(id, {
      id,
      epoch,
      x: cx,
      y: cy,
      parts,
      ground,
      uniforms,
      textures,
      hSrc,
      hdata,
      base,
      shown: null,
      roofGeometry,
      grids: [gridG, walkGridG, topGridG, pathGridG],
      decks,
      atmo,
      atmoUniforms,
      atmosphere,
      lastUsed: now,
      objects: nObj,
      data,
      flags: c.flags,
      tier: c.tier,
      surface: c.surface,
      heights: c.heights,
      raised: c.raised,
      sea: c.sea,
      objs,
      sprites: c.sprites,
      prepared: waiting ? c : null,
    });
    this.dropPreview(id, epoch);
  }

  private evict() {
    if (this.chunks.size <= CHUNK_CAP) return;
    const old = [...this.chunks.values()].filter((c) => !c.parts[0].visible).sort((a, b) => a.lastUsed - b.lastUsed);
    for (const c of old) {
      if (this.chunks.size <= CHUNK_CAP) break;
      this.chunks.delete(c.id);
      this.destroyChunk(c);
    }
  }

  private destroyChunk(c: ChunkView) {
    for (const p of c.parts) p.destroy({ children: true });
    for (const t of c.textures) t.destroy(true);
    c.roofGeometry?.destroy();
  }
}

/** The sprite atlas depends only on the catalog: built once per page and reused by every
 * battlemap layer (worlds reload; warm-up builds it while T0 generates). */
let atlasCache: { key: string; renderer: Renderer; atlas: Atlas } | null = null;
function sharedAtlas(renderer: Renderer, catalog: KindInfo[]): Atlas {
  const key = catalog.map((k) => k.name).join('|');
  if (!atlasCache || atlasCache.key !== key || atlasCache.renderer !== renderer) atlasCache = { key, renderer, atlas: buildAtlas(renderer, catalog) };
  return atlasCache.atlas;
}

/** Whether a park's ground (`URBAN_PARK`, `URBAN_PATH`) lies along a chunk's edge (`raised`: per square, the
 * lift and the urban kind). */
function parkEdge(raised: Uint8Array | null): boolean {
  if (!raised) return false;
  for (let i = 0; i < SQ; i++) {
    for (const k of [i, (SQ - 1) * SQ + i, i * SQ, i * SQ + SQ - 1]) {
      const u = raised[k * 2 + 1];
      if (u === 11 || u === 12) return true;
    }
  }
  return false;
}

/** Props always drawn turned to their `rot` (by catalog name; also fallen logs, old bones and ruined
 * walls, by id). */
const TURNED = new Set(['market stall', 'cart', 'tent', 'campfire', 'firewood stack', 'bedroll', 'water trough', 'bench', 'flowerbed', 'hedge', 'footbridge']);
/** Turned props that look the same after a half turn, by the start of the half turn (degrees) they are
 * drawn within: the turns that keep the side drawn lit within a quarter turn of the light in the NW. A hedge is
 * lit along its north side: -135° to 45°; a flowerbed on its north and west edges: -90° to 90°; a
 * footbridge toward its west end: -45° to 135°. */
const HALF_TURN = new Map([
  ['hedge', -135],
  ['flowerbed', -90],
  ['footbridge', -45],
]);
/** Props drawn at their `rot` only when turned by hand (`Placed.turned`), by catalog name: those whose
 * NW-lit look still reads turned (approved in the gallery, `?gallery=1&turn=1`). Trees, plants,
 * pools and hazards never turn. */
export const TURNABLE = new Set(['barrels', 'crates', 'headstone', 'statue on a plinth', 'haystack', 'rock pile', 'well']);

/** Whether a built-in kind is drawn at its `rot`: some always, others only when turned by hand.
 * (Uploaded sprites always are.) */
export function drawnTurned(inf: KindInfo | undefined, turned: boolean): boolean {
  if (!inf) return false;
  return inf.id === 10 || inf.id === 20 || inf.id === 40 || TURNED.has(inf.name) || (turned && TURNABLE.has(inf.name));
}

/** The turn a built-in kind is drawn at (radians): none, `rot`, or `rot` folded by a half turn into its
 * `HALF_TURN` range, so the side drawn lit stays within a quarter turn of the light in the NW. (The drop
 * shadow is drawn at the same turn; play mode's footprint takes `rot` as saved, the same squares.) */
export function drawnRotation(inf: KindInfo | undefined, turned: boolean, rot: number): number {
  if (!inf || !drawnTurned(inf, turned)) return 0;
  const from = HALF_TURN.get(inf.name);
  if (from === undefined) return rot;
  const lo = (from * Math.PI) / 180;
  return ((((rot - lo) % Math.PI) + Math.PI) % Math.PI) + lo;
}
/** Sparse plants (blades or fronds with ground between them) and thin upright things (posts drawn from a
 * high angle): a smaller, fainter shadow than a solid crown, as [size, strength] factors. */
export const SPARSE = new Map([
  ['tall grass', [0.75, 0.45]],
  ['reeds', [0.75, 0.45]],
  ['palm', [0.85, 0.6]],
  ['signpost', [0.6, 0.55]],
  ['lamp post', [0.55, 0.55]],
  ['notice board', [0.85, 0.7]],
  ['topiary', [0.75, 0.8]],
  ['garden statue', [0.85, 0.8]],
  ['footbridge', [1, 1.3]],
]);
/** Long things: a shadow drawn out along them, [length, width] in squares at scale 1, and how much further
 * than usual it falls. */
export const LONG_SHADOW = new Map([
  ['hedge', [1.35, 0.72, 1]],
  ['footbridge', [3.1, 0.85, 1.6]],
]);
/** Props that draw their own shadow into their picture, so get no drop shadow. */
export const OWN_SHADOW = new Set(['market cross']);

const STONE = 0x9c968b;
const STONE_TOP = 0xb3ada2;

/**
 * Walls, gatehouses, towers and ways underground as clean vectors over the ground (their squares
 * keep their tactical height; the ground under them is drawn flat): `walls.ts` draws the first
 * three, and a forecourt's garden wall, gate piers and gate. Bridges and piers are drawn apart
 * (`bridges.ts`).
 */
function drawStructures(shapes: Shape[]): Graphics {
  const g = new Graphics();
  const walls = shapes.filter((s) => s.kind === 0);
  if (walls.length) drawWalls(g, walls);
  for (const s of shapes) if (s.kind === 9) drawGate(g, s);
  for (const s of shapes) if (s.kind === 1) drawWallTower(g, s);
  for (const s of shapes) if (s.kind === 4) drawEntrance(g, s);
  // A forecourt's low garden wall, its gate's piers, then the open leaves.
  const garden = shapes.filter((s) => s.kind === 10);
  if (garden.length) drawGardenWalls(g, garden);
  for (const s of shapes) if (s.kind === 11) drawGatePier(g, s);
  for (const s of shapes) if (s.kind === 12) drawForecourtGate(g, s);
  return g;
}

/**
 * A round feature a tier above the paving (`size`: 0 dais, 1 fountain, 2 planter, 3 market
 * cross; `pts`: the centre, then a point on the rim where its steps come down): a drop shadow,
 * a moulded stone round, and steps curving down one side.
 */
function drawDais(g: Graphics, s: Shape) {
  const [cx, cy] = [s.pts[0], s.pts[1]];
  const [dx, dy] = [s.pts[2] - cx, s.pts[3] - cy];
  const r = Math.hypot(dx, dy) || PX;
  const a = Math.atan2(dy, dx);
  const kind = Math.round(s.size / PX);
  // Steps: three curved treads out from the rim.
  for (let k = 2; k >= 0; k--) {
    const ro = r + PX * 0.28 * (k + 1);
    const span = Math.min(0.9, (PX * 1.1) / r);
    g.moveTo(cx + Math.cos(a - span) * r, cy + Math.sin(a - span) * r);
    g.arc(cx, cy, ro, a - span, a + span);
    g.lineTo(cx + Math.cos(a + span) * r, cy + Math.sin(a + span) * r);
    g.closePath();
    g.fill([0xa39d92, 0x958f84, 0x878177][k]).stroke({ width: 1.5, color: ROOF_INK });
  }
  g.circle(cx + PX * 0.3, cy + PX * 0.3, r).fill({ color: 0x000000, alpha: 0.3 });
  g.circle(cx, cy, r).fill(STONE).stroke({ width: 3, color: ROOF_INK });
  g.circle(cx, cy, r - PX * 0.18).fill(STONE_TOP).stroke({ width: 1.5, color: 0x6f6a61 });
  if (kind === 1) {
    // Fountain: a basin of water inside the rim, an inner bowl and the jet.
    g.circle(cx, cy, r - PX * 0.4).fill(0x3f6f86).stroke({ width: 2, color: 0x57534b });
    for (const f of [0.55, 0.78]) g.circle(cx, cy, (r - PX * 0.4) * f).stroke({ width: 1.2, color: 0xbfe3ef, alpha: 0.45 });
    g.circle(cx + PX * 0.12, cy + PX * 0.12, Math.max(PX * 0.5, r * 0.28)).fill({ color: 0x000000, alpha: 0.25 });
    g.circle(cx, cy, Math.max(PX * 0.5, r * 0.28)).fill(STONE_TOP).stroke({ width: 2, color: ROOF_INK });
    g.circle(cx, cy, Math.max(PX * 0.3, r * 0.17)).fill(0x5d93ab);
    g.circle(cx, cy, PX * 0.12).fill(0xeaf6fa);
    return;
  }
  if (kind === 2) {
    // Planter: a ring of coping round dark soil and grass (the tree stands in it).
    g.circle(cx, cy, r - PX * 0.35).fill(0x4a3a28).stroke({ width: 1.5, color: 0x57534b });
    for (let k = 0; k < 9; k++) {
      const t = (k / 9) * Math.PI * 2 + a;
      g.circle(cx + Math.cos(t) * (r - PX * 0.7), cy + Math.sin(t) * (r - PX * 0.7), PX * 0.22).fill({ color: 0x5f7a3e, alpha: 0.85 });
    }
    return;
  }
  // Flagstones in rings.
  for (let rr = r - PX * 0.18 - PX; rr > PX * 0.4; rr -= PX) g.circle(cx, cy, rr);
  g.stroke({ width: 1, color: 0x7d776c, alpha: 0.7 });
  for (let k = 0; k < 8; k++) {
    const t = (k / 8) * Math.PI * 2 + a + 0.3;
    g.moveTo(cx + Math.cos(t) * PX * 0.4, cy + Math.sin(t) * PX * 0.4).lineTo(cx + Math.cos(t) * (r - PX * 0.18), cy + Math.sin(t) * (r - PX * 0.18));
  }
  g.stroke({ width: 1, color: 0x7d776c, alpha: 0.5 });
  if (kind === 3) {
    // Market cross: a stepped plinth and a tall shaft with its cross-head, casting long.
    const h = Math.max(PX * 0.45, r * 0.42);
    g.circle(cx, cy, h).fill(STONE).stroke({ width: 2, color: ROOF_INK });
    g.moveTo(cx, cy).lineTo(cx + PX * 2.2, cy + PX * 2.2).stroke({ width: PX * 0.3, color: 0x000000, alpha: 0.28 });
    g.rect(cx - PX * 0.2, cy - PX * 0.2, PX * 0.4, PX * 0.4).fill(0xd8d1c2).stroke({ width: 1.5, color: ROOF_INK });
    g.rect(cx - PX * 0.45, cy - PX * 0.09, PX * 0.9, PX * 0.18).fill(0xd8d1c2).stroke({ width: 1.2, color: ROOF_INK });
  }
}

/**
 * A way underground, by kind (`size`): a stairwell going down (dungeon, crypt, catacombs), a
 * cave mouth, a timber-framed mine adit with rails running out, a lava-tube skylight, a round
 * sewer grate in the street. `pts`: the opening, then a point one square in along the passage.
 */
function drawEntrance(g: Graphics, s: Shape) {
  const [x, y] = [s.pts[0], s.pts[1]];
  const l = Math.hypot(s.pts[2] - x, s.pts[3] - y) || 1;
  const [ux, uy] = [(s.pts[2] - x) / l, (s.pts[3] - y) / l];
  const [vx, vy] = [-uy, ux];
  // A point `a` squares in along the passage and `b` across it.
  const at = (a: number, b: number): [number, number] => [x + (ux * a + vx * b) * PX, y + (uy * a + vy * b) * PX];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ellipse = (a: number, ra: number, rb: number, wobble: number) => {
    const pts: number[] = [];
    for (let k = 0; k < 18; k++) {
      const t = (k / 18) * Math.PI * 2;
      const r = 1 + wobble * Math.sin(t * 3 + s.pts[0] * 0.01) * Math.cos(t * 2);
      pts.push(...at(a + Math.cos(t) * ra * r, Math.sin(t) * rb * r));
    }
    return pts;
  };
  // `size` carries kind + 16 × style (scaled by PX like every shape size when the chunk was
  // decoded): style 1 a ruin's broken stair, 2 a graveyard mausoleum over the stair.
  const code = Math.round(s.size / PX);
  // A stable random per entrance.
  let seed = (Math.round(x * 7.1) * 73856093) ^ (Math.round(y * 3.3) * 19349663);
  const rnd = () => {
    seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0;
    return ((seed ^ (seed >>> 13)) >>> 0) / 4294967296;
  };
  if (code >> 4 === 1) return drawRuinStair(g, at, quad, rnd);
  if (code >> 4 === 2) return drawMausoleum(g, at, quad, rnd);
  switch (code & 15) {
    case 0:
    case 1:
    case 6: {
      // Stairwell: steps darkening as they go down, a stone kerb round three sides.
      g.poly(quad(-0.5, 2.5, -1, 1)).fill(0x2a2520);
      for (let k = 0; k < 6; k++) {
        const a = -0.5 + k * 0.5;
        const shade = Math.round(0x9a - k * 0x14);
        g.poly(quad(a, a + 0.42, -0.95, 0.95)).fill((shade << 16) | (shade << 8) | (shade - 8));
      }
      g.poly([...at(-0.5, -1), ...at(2.5, -1), ...at(2.5, 1), ...at(-0.5, 1)], false).stroke({ width: 10, color: STONE, join: 'miter' });
      g.poly([...at(-0.5, -1.12), ...at(2.62, -1.12), ...at(2.62, 1.12), ...at(-0.5, 1.12)], false).stroke({ width: 2.5, color: ROOF_INK, join: 'miter' });
      break;
    }
    case 2: {
      // Cave mouth: rock heaped round a dark opening.
      g.poly(ellipse(0.7, 1.5, 2.1, 0.12)).fill({ color: 0x5d584f, alpha: 0.9 });
      g.poly(ellipse(0.8, 1.1, 1.6, 0.1)).fill(0x221e1a).stroke({ width: 3, color: ROOF_INK });
      g.poly(ellipse(1.0, 0.6, 1.0, 0.08)).fill(0x0f0d0b);
      break;
    }
    case 3: {
      // Mine adit: rails out of a dark opening in a timber frame.
      for (const b of [-0.35, 0.35]) g.moveTo(...at(-3, b)).lineTo(...at(0.6, b));
      g.stroke({ width: 3, color: 0x4a4440 });
      for (let a = -2.8; a < 0.6; a += 0.5) g.moveTo(...at(a, -0.55)).lineTo(...at(a, 0.55));
      g.stroke({ width: 4, color: 0x6e4a2c });
      g.poly(quad(0, 1.4, -1.1, 1.1)).fill(0x15120f);
      g.poly([...at(0, -1.2), ...at(0, 1.2)], false).stroke({ width: 9, color: 0x7a5530, cap: 'round' });
      for (const b of [-1.15, 1.15]) g.moveTo(...at(0, b)).lineTo(...at(1.3, b));
      g.stroke({ width: 7, color: 0x6a4428, cap: 'round' });
      break;
    }
    case 5: {
      // Sewer grate: an iron disc with bars over the drain.
      g.circle(x, y, PX * 0.55).fill(0x2f3236).stroke({ width: 3, color: ROOF_INK });
      for (const b of [-0.3, -0.1, 0.1, 0.3]) g.moveTo(...at(-0.45, b)).lineTo(...at(0.45, b));
      g.stroke({ width: 3, color: 0x5d6168 });
      g.circle(x, y, PX * 0.55).stroke({ width: 4, color: 0x4a4e54 });
      break;
    }
    default: {
      // Lava tube skylight: a collapsed roof onto the tube, a glow below.
      g.poly(ellipse(0.4, 1.7, 1.9, 0.18)).fill({ color: 0x3a3530, alpha: 0.95 });
      g.poly(ellipse(0.4, 1.25, 1.4, 0.15)).fill(0x120d0a).stroke({ width: 3, color: ROOF_INK });
      g.poly(ellipse(0.6, 0.6, 0.7, 0.1)).fill({ color: 0xd2691e, alpha: 0.35 });
      break;
    }
  }
}

type At = (a: number, b: number) => [number, number];
type Quad = (a0: number, a1: number, b0: number, b1: number) => number[];

/** Stone grey shaded by `k` (0 light .. 1 dark). */
const stoneShade = (k: number) => {
  const v = Math.round(0xa8 - k * 0x70);
  return (v << 16) | (v << 8) | Math.max(0, v - 10);
};

/**
 * A stair down through a ruin's floor, 3 squares wide and 4 deep: cracked steps darkening
 * into the dark, a kerb of fallen blocks with gaps, rubble and moss strewn round it.
 */
function drawRuinStair(g: Graphics, at: At, quad: Quad, rnd: () => number) {
  // A shadowed pit, then the steps.
  g.poly(quad(-0.6, 3.6, -1.6, 1.6)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(quad(-0.5, 3.5, -1.5, 1.5)).fill(0x1b1714);
  for (let k = 0; k < 7; k++) {
    const a = -0.5 + k * 0.5;
    const col = stoneShade(Math.min(1, k / 6.5));
    // Each step a few slabs, some cracked or missing.
    for (let b = -1.45; b < 1.4; b += 0.72) {
      if (rnd() < 0.12 && k > 1) continue;
      const w = 0.66 + rnd() * 0.04;
      g.poly(quad(a + 0.02, a + 0.42 - rnd() * 0.06, b, b + w)).fill(col);
      if (rnd() < 0.3) g.moveTo(...at(a + 0.05, b + w * rnd())).lineTo(...at(a + 0.38, b + w * rnd())).stroke({ width: 1.5, color: 0x2a2520, alpha: 0.7 });
    }
    g.moveTo(...at(a + 0.42, -1.45)).lineTo(...at(a + 0.42, 1.45)).stroke({ width: 1.2, color: 0x2a2520, alpha: 0.6 });
  }
  // The kerb: worn blocks along both sides and the back, some fallen out of line.
  const block = (a: number, b: number, la: number, lb: number) => {
    const j = (rnd() - 0.5) * 0.12;
    const p = [...at(a + j, b - j), ...at(a + la + j, b - j), ...at(a + la - j, b + lb + j), ...at(a - j, b + lb + j)];
    g.poly(p.map((v) => v + 3)).fill({ color: 0x000000, alpha: 0.3 });
    g.poly(p).fill(rnd() < 0.5 ? STONE : STONE_TOP).stroke({ width: 1.8, color: ROOF_INK });
  };
  for (let a = -0.5; a < 3.5; a += 0.62) {
    for (const side of [-1, 1]) {
      if (rnd() < 0.25) continue;
      block(a, side < 0 ? -1.85 : 1.5, 0.56, 0.34);
    }
  }
  for (let b = -1.85; b < 1.8; b += 0.62) if (rnd() > 0.3) block(3.5, b, 0.34, 0.56);
  // Rubble and moss about the mouth.
  for (let k = 0; k < 10; k++) {
    const a = -1.6 + rnd() * 5.6;
    const b = (rnd() < 0.5 ? -1 : 1) * (1.9 + rnd() * 1.1);
    const r = 0.1 + rnd() * 0.16;
    const [cx, cy] = at(a, b);
    g.circle(cx + 2, cy + 2, r * PX).fill({ color: 0x000000, alpha: 0.25 });
    g.circle(cx, cy, r * PX).fill(stoneShade(0.15 + rnd() * 0.3)).stroke({ width: 1.2, color: ROOF_INK, alpha: 0.8 });
  }
  for (let k = 0; k < 4; k++) {
    const [cx, cy] = at(rnd() * 3.5, (rnd() - 0.5) * 3.4);
    g.circle(cx, cy, (0.15 + rnd() * 0.15) * PX).fill({ color: 0x55703a, alpha: 0.45 });
  }
}

/**
 * A graveyard mausoleum over the catacomb stair, 3 squares wide and 4 deep: a pale stone
 * plinth, heavy walls on three sides, columns either side of the open door, the stair going
 * down inside; a slab path out of the door, urns at its corners.
 */
function drawMausoleum(g: Graphics, at: At, quad: Quad, rnd: () => number) {
  // Shadow (light from the NW), plinth, the threshold slabs out front.
  g.poly(quad(-1.5, 2.5, -1.5, 1.5).map((v) => v + 0.35 * PX)).fill({ color: 0x000000, alpha: 0.3 });
  g.poly(quad(-1.5, 2.5, -1.5, 1.5)).fill(0xc4bdae).stroke({ width: 2.5, color: ROOF_INK });
  for (let k = 0; k < 2; k++) g.poly(quad(-2.4 + k * 0.45, -2.4 + k * 0.45 + 0.4, -0.55, 0.55)).fill(0xa8a294).stroke({ width: 1.2, color: ROOF_INK, alpha: 0.8 });
  // Walls on the sides and back.
  const wall = 0x8f897d;
  g.poly(quad(-1.2, 2.5, -1.5, -1.05)).fill(wall).stroke({ width: 2, color: ROOF_INK });
  g.poly(quad(-1.2, 2.5, 1.05, 1.5)).fill(wall).stroke({ width: 2, color: ROOF_INK });
  g.poly(quad(2.05, 2.5, -1.05, 1.05)).fill(wall).stroke({ width: 2, color: ROOF_INK });
  // Coping along the wall tops.
  for (const b of [-1.28, 1.28]) g.moveTo(...at(-1.15, b)).lineTo(...at(2.45, b));
  g.moveTo(...at(2.28, -1.25)).lineTo(...at(2.28, 1.25));
  g.stroke({ width: 1.5, color: 0xd6cfbf, alpha: 0.8 });
  // The stair inside, darkening as it goes down toward the back.
  g.poly(quad(-0.6, 2.05, -1.05, 1.05)).fill(0x1b1714);
  for (let k = 0; k < 5; k++) {
    const a = -0.55 + k * 0.52;
    g.poly(quad(a, a + 0.44, -0.98, 0.98)).fill(stoneShade(0.1 + k * 0.2));
  }
  // Columns either side of the door, a lintel slab across.
  for (const b of [-1.25, 1.25]) {
    const [cx, cy] = at(-1.25, b);
    g.circle(cx + 2.5, cy + 2.5, 0.3 * PX).fill({ color: 0x000000, alpha: 0.3 });
    g.circle(cx, cy, 0.3 * PX).fill(0xd8d1c2).stroke({ width: 2, color: ROOF_INK });
    g.circle(cx - 1.5, cy - 1.5, 0.12 * PX).fill({ color: 0xffffff, alpha: 0.35 });
  }
  // Urns at the front corners of the path.
  for (const b of [-0.85, 0.85]) {
    const [cx, cy] = at(-2.15, b);
    g.circle(cx + 2, cy + 2, 0.18 * PX).fill({ color: 0x000000, alpha: 0.3 });
    g.circle(cx, cy, 0.18 * PX).fill(0x6f675b).stroke({ width: 1.5, color: ROOF_INK });
    g.circle(cx, cy, 0.08 * PX).fill(0x2a2520);
  }
  // Moss and lichen on the stone.
  for (let k = 0; k < 4; k++) {
    const [cx, cy] = at(-1.1 + rnd() * 3.5, (rnd() < 0.5 ? -1.3 : 1.3) + (rnd() - 0.5) * 0.2);
    g.circle(cx, cy, (0.08 + rnd() * 0.1) * PX).fill({ color: 0x5f7a3e, alpha: 0.55 });
  }
}

/** Merlons along a closed outline (inset toward its centre). */
function merlons(g: Graphics, pts: number[], cx: number, cy: number, size: number) {
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) {
    const ax = pts[i * 2];
    const ay = pts[i * 2 + 1];
    const bx = pts[((i + 1) % m) * 2];
    const by = pts[((i + 1) % m) * 2 + 1];
    const len = Math.hypot(bx - ax, by - ay);
    if (len < 1e-6) continue;
    const [ux, uy] = [(bx - ax) / len, (by - ay) / len];
    // Inward normal.
    let [nx, ny] = [-uy, ux];
    if ((cx - ax) * nx + (cy - ay) * ny < 0) [nx, ny] = [-nx, -ny];
    for (let t = size; t + size * 0.5 < len; t += size * 2) {
      const x = ax + ux * t + nx * size * 0.5;
      const y = ay + uy * t + ny * size * 0.5;
      g.rect(x - size * 0.5, y - size * 0.5, size, size);
    }
  }
}

/** `c` scaled by `k` per channel (shading a lit or shadowed face). */
function shadeColor(c: number, k: number) {
  const f = (v: number) => Math.max(0, Math.min(255, Math.round(v * k)));
  return (f(c >> 16) << 16) | (f((c >> 8) & 255) << 8) | f(c & 255);
}

/** How lit a face sloping down toward (nx, ny) is under the NW light. */
const faceLight = (nx: number, ny: number) => 0.86 + 0.24 * (-nx - ny) * Math.SQRT1_2;

const SLATE = 0x5d6670;

/**
 * A round tower seen from above: its shadow, a stone ring, and a conical slate roof in lit and
 * shadowed wedges with a finial (or a crenellated top when `flat`).
 */
function roundTower(g: Graphics, x: number, y: number, r: number, lift: number, flat: boolean) {
  g.circle(x + lift, y + lift, r).fill({ color: 0x000000, alpha: 0.32 });
  g.circle(x, y, r).fill(0x8f897d).stroke({ width: 2.5, color: ROOF_INK });
  if (flat) {
    g.circle(x, y, r * 0.72).fill(0x9a958a).stroke({ width: 1.5, color: 0x57534b });
    for (let k = 0; k < 10; k++) {
      const a = (k / 10) * Math.PI * 2;
      g.circle(x + Math.cos(a) * r * 0.86, y + Math.sin(a) * r * 0.86, r * 0.12);
    }
    g.fill(0x57534b);
    return;
  }
  const n = 16;
  const rr = r * 0.92;
  for (let k = 0; k < n; k++) {
    const a0 = (k / n) * Math.PI * 2;
    const a1 = ((k + 1) / n) * Math.PI * 2;
    const am = (a0 + a1) / 2;
    g.poly([x, y, x + Math.cos(a0) * rr, y + Math.sin(a0) * rr, x + Math.cos(a1) * rr, y + Math.sin(a1) * rr]).fill(shadeColor(SLATE, faceLight(Math.cos(am), Math.sin(am))));
  }
  // Slate courses round the cone.
  for (const f of [0.35, 0.6, 0.82]) g.circle(x, y, rr * f).stroke({ width: 1, color: 0x2f353b, alpha: 0.45 });
  g.circle(x, y, rr).stroke({ width: 2, color: ROOF_INK });
  g.circle(x, y, r * 0.1).fill(0xc9a227).stroke({ width: 1, color: ROOF_INK });
}

/** Dark lead (flats and lead roofs), and its rolls. */
const LEAD = 0x5f656b;
/** In a keep body's top code (`town::DONJON_LOW`): its donjon at its first corner's end. */
const DONJON_LOW = 4;
const LEAD_ROLL = 0x80878d;

/**
 * The top of a four-cornered piece of a keep (`town::keep_tops`), from corner 0 along its longer
 * side: `top` 0 open battlements, a wall walk of stone flags inside the merlons (a broad one's
 * middle a flat of dark lead with its rolls), a stair hatch in a corner or (`cap`, the donjon) a
 * cap house under a slate pyramid in the middle; 1 or 2 a hipped roof of lead (its rolls up the
 * slopes) or slate (its courses) inside the wall walk. Its merlons and outline last.
 */
function keepTop(g: Graphics, pts: number[], top: number, cap: boolean) {
  const [p0x, p0y] = [pts[0], pts[1]];
  let [ax, ay] = [pts[2] - p0x, pts[3] - p0y];
  let [bx, by] = [pts[6] - p0x, pts[7] - p0y];
  if (Math.hypot(bx, by) > Math.hypot(ax, ay)) [ax, ay, bx, by] = [bx, by, ax, ay];
  const L = Math.hypot(ax, ay) || 1;
  const W = Math.hypot(bx, by) || 1;
  const [ux, uy] = [ax / L, ay / L];
  const [vx, vy] = [bx / W, by / W];
  const P = (s: number, t: number): [number, number] => [p0x + ux * s + vx * t, p0y + uy * s + vy * t];
  const poly = (...st: [number, number][]) => st.flatMap(([s, t]) => P(s, t));
  const rect = (s0: number, s1: number, t0: number, t1: number) => poly([s0, t0], [s1, t0], [s1, t1], [s0, t1]);
  const [cx, cy] = P(L / 2, W / 2);
  const ww = PX * 1.15;
  if (top === 0) {
    // Stone flags in courses along the walk, their joints staggered.
    g.poly(rect(0, L, 0, W)).fill(0x9d988c);
    const f = PX * 0.7;
    for (let t = f; t < W - PX * 0.1; t += f) g.moveTo(...P(0, t)).lineTo(...P(L, t));
    for (let t = 0, row = 0; t < W; t += f, row++) for (let s = (row % 2) * f * 0.5 + f; s < L - PX * 0.1; s += f * 1.4) g.moveTo(...P(s, t)).lineTo(...P(s, Math.min(W, t + f)));
    g.stroke({ width: 1, color: 0x6f6a61, alpha: 0.5 });
    const pw = PX * 1.6;
    if (L > PX * 7 && W > PX * 7) {
      const lead = rect(pw, L - pw, pw, W - pw);
      g.poly(lead).fill(LEAD).stroke({ width: 1.5, color: 0x3d4146 });
      for (let s = pw + PX * 1.2; s < L - pw - PX * 0.3; s += PX * 1.2) g.moveTo(...P(s, pw)).lineTo(...P(s, W - pw));
      g.stroke({ width: 2, color: LEAD_ROLL, alpha: 0.6 });
    }
    // (The parapet's shadow on the walk along its north-west sides.)
    g.moveTo(...P(L - PX * 0.5, PX * 0.5)).lineTo(...P(PX * 0.5, PX * 0.5)).lineTo(...P(PX * 0.5, W - PX * 0.5));
    g.stroke({ width: PX * 0.4, color: 0x000000, alpha: 0.22 });
    if (cap) {
      const c = Math.min(L, W) * 0.17;
      const [cs, ct] = [L / 2, W / 2];
      const box = rect(cs - c, cs + c, ct - c, ct + c);
      g.poly(box.map((v) => v + PX * 0.6)).fill({ color: 0x000000, alpha: 0.32 });
      g.poly(box).fill(0x8f897d).stroke({ width: 2, color: ROOF_INK });
      const r = c * 0.85;
      const apex = P(cs, ct);
      const pyr: [[number, number], [number, number], number, number][] = [
        [[cs - r, ct - r], [cs + r, ct - r], -vx, -vy],
        [[cs + r, ct - r], [cs + r, ct + r], ux, uy],
        [[cs + r, ct + r], [cs - r, ct + r], vx, vy],
        [[cs - r, ct + r], [cs - r, ct - r], -ux, -uy],
      ];
      for (const [e0, e1, nx, ny] of pyr) g.poly([...P(...e0), ...P(...e1), ...apex]).fill(shadeColor(SLATE, faceLight(nx, ny))).stroke({ width: 1.2, color: 0x2f353b, alpha: 0.7 });
      g.circle(...apex, PX * 0.14).fill(0xc9a227).stroke({ width: 1, color: ROOF_INK });
    } else {
      // A stair hatch in its far corner: a dark opening, its lid of boards laid back.
      const h = PX * 0.8;
      const [hs, ht] = [L - ww - PX * 0.9, W - ww - PX * 0.9];
      g.poly(rect(hs - h / 2, hs + h / 2, ht - h / 2, ht + h / 2)).fill(0x2a2621).stroke({ width: 1, color: ROOF_INK });
      g.poly(rect(hs - h / 2 - h, hs - h / 2, ht - h / 2, ht + h / 2)).fill(0x7a5a3a).stroke({ width: 1, color: ROOF_INK });
    }
  } else {
    // The wall walk, and a hipped roof inside it along the piece's length.
    g.poly(rect(0, L, 0, W)).fill(0x9a958a);
    for (let s = PX; s < L; s += PX) g.moveTo(...P(s, 0)).lineTo(...P(s, ww)).moveTo(...P(s, W - ww)).lineTo(...P(s, W));
    g.stroke({ width: 1, color: 0x6f6a61, alpha: 0.55 });
    const base = top === 1 ? LEAD : SLATE;
    const [s0, s1, a, b] = [ww, L - ww, ww, W - ww];
    const h = Math.min((b - a) / 2, (s1 - s0) / 2);
    const tm = (a + b) / 2;
    const r0: [number, number] = [s0 + h, tm];
    const r1: [number, number] = [s1 - h, tm];
    const faces: [[number, number][], number, number][] = [
      [[[s0, a], [s1, a], r1, r0], -vx, -vy],
      [[[s0, b], [s1, b], r1, r0], vx, vy],
      [[[s0, a], r0, [s0, b]], -ux, -uy],
      [[[s1, a], r1, [s1, b]], ux, uy],
    ];
    for (const [fc, nx, ny] of faces) g.poly(poly(...fc)).fill(shadeColor(base, faceLight(nx, ny)));
    if (top === 1) {
      // Lead rolls from the eaves up the long slopes.
      for (let s = s0 + PX * 1.1; s < s1 - PX * 0.3; s += PX * 1.1) {
        const k = Math.min(s - s0, s1 - s, h);
        g.moveTo(...P(s, a)).lineTo(...P(s, a + k)).moveTo(...P(s, b)).lineTo(...P(s, b - k));
      }
      g.stroke({ width: 2, color: LEAD_ROLL, alpha: 0.55 });
    } else {
      for (let d = PX * 0.45; d < h - PX * 0.1; d += PX * 0.45) {
        g.moveTo(...P(s0 + d, a + d)).lineTo(...P(s1 - d, a + d)).lineTo(...P(s1 - d, b - d)).lineTo(...P(s0 + d, b - d)).lineTo(...P(s0 + d, a + d));
      }
      g.stroke({ width: 1, color: 0x2f353b, alpha: 0.35 });
    }
    g.moveTo(...P(s0, a)).lineTo(...P(...r0)).lineTo(...P(s0, b)).moveTo(...P(s1, a)).lineTo(...P(...r1)).lineTo(...P(s1, b));
    g.stroke({ width: 1.5, color: 0x8a9099, alpha: 0.8 });
    g.moveTo(...P(...r0)).lineTo(...P(...r1)).stroke({ width: 3, color: 0x9aa0a8 });
    g.poly(rect(s0, s1, a, b)).stroke({ width: 1.8, color: ROOF_INK });
    // (The walk's inner north-west edges shade the roof set below it.)
    g.moveTo(...P(s1, a)).lineTo(...P(s0, a)).lineTo(...P(s0, b));
    g.stroke({ width: PX * 0.35, color: 0x000000, alpha: 0.22 });
  }
  merlons(g, pts, cx, cy, PX * 0.45);
  g.fill(0x57534b);
  g.poly(pts).stroke({ width: 2.5, color: ROOF_INK });
}

/**
 * A keep from above (a rectangle with corner towers): a tall square donjon at one end (a flat
 * leaded roof inside its own crenellated parapet, a stair turret with a banner) and a lower hall
 * range along the rest: a crenellated wall walk round hipped slate roofs (one or two ridges,
 * lead gutters, chimneys). Round corner towers with conical roofs stand a little proud of the
 * walls, bartizans along long walls; the whole casts a long shadow. A keep's body among its other
 * pieces (`drawKeepParts`: `avoid`, their outlines) has no corner towers of its own, no bartizan
 * where a piece meets its wall and its stair turret at the donjon's corner farthest from them (none
 * where no corner is clear).
 */
function drawKeep(g: Graphics, pts: number[], tower: number, cornerTowers = true, avoid: number[][] = [], top = 0, low?: boolean) {
  // How far a point stands clear of the other pieces (negative inside one).
  const clearOf = (x: number, y: number) => Math.min(Infinity, ...avoid.map((a) => polyClear(a, x, y)));
  // Long axis u from corner 0, short axis v.
  const [p0x, p0y] = [pts[0], pts[1]];
  let [ax, ay] = [pts[2] - p0x, pts[3] - p0y];
  let [bx, by] = [pts[6] - p0x, pts[7] - p0y];
  if (Math.hypot(bx, by) > Math.hypot(ax, ay)) [ax, ay, bx, by] = [bx, by, ax, ay];
  const L = Math.hypot(ax, ay);
  const W = Math.hypot(bx, by);
  const [ux, uy] = [ax / L, ay / L];
  const [vx, vy] = [bx / W, by / W];
  const P = (s: number, t: number): [number, number] => [p0x + ux * s + vx * t, p0y + uy * s + vy * t];
  const poly = (...st: [number, number][]) => st.flatMap(([s, t]) => P(s, t));
  const rect = (s0: number, s1: number, t0: number, t1: number) => poly([s0, t0], [s1, t0], [s1, t1], [s0, t1]);
  let seed = (Math.round(p0x * 13.7) * 73856093) ^ (Math.round(p0y * 7.3) * 19349663);
  const rnd = () => {
    seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0;
    return ((seed ^ (seed >>> 13)) >>> 0) / 4294967296;
  };
  const lift = PX * 0.35;
  // The donjon: a square at one end (none on a short keep).
  const D = L > W * 1.35 ? Math.min(W, L * 0.42) : 0;
  // (A keep built of pieces: at the end its top code names, as its interior has it.)
  const rolled = rnd() < 0.5;
  const donjonLow = low ?? rolled;
  const [d0, d1] = donjonLow ? [0, D] : [L - D, L];
  const [h0, h1] = donjonLow ? [D, L] : [0, L - D];
  // Shadow of the whole mass, longer from the donjon.
  g.poly(pts.map((v) => v + PX * 1.1)).fill({ color: 0x000000, alpha: 0.28 });
  if (D > 0) g.poly(rect(d0, d1, 0, W).map((v) => v + PX * 2.2)).fill({ color: 0x000000, alpha: 0.22 });
  // Wall walk round the hall range.
  const ww = PX * 1.15;
  g.poly(rect(h0, h1, 0, W)).fill(0x9a958a);
  for (let s = h0 + PX; s < h1; s += PX) g.moveTo(...P(s, 0)).lineTo(...P(s, ww)).moveTo(...P(s, W - ww)).lineTo(...P(s, W));
  g.stroke({ width: 1, color: 0x6f6a61, alpha: 0.55 });
  // Hipped slate roofs inside the walk.
  const [s0, s1, t0, t1] = [h0 + (donjonLow ? 0 : ww), h1 - (donjonLow ? ww : 0), ww, W - ww];
  const bands = Math.max(1, Math.min(2, Math.round((t1 - t0) / (PX * 10))));
  const bw = (t1 - t0) / bands;
  for (let k = 0; k < bands; k++) {
    const [a, b] = [t0 + k * bw, t0 + (k + 1) * bw];
    const h = Math.min(bw / 2, (s1 - s0) / 2);
    const tm = (a + b) / 2;
    const r0: [number, number] = [s0 + h, tm];
    const r1: [number, number] = [s1 - h, tm];
    const faces: [[number, number][], number, number][] = [
      [[[s0, a], [s1, a], r1, r0], -vx, -vy],
      [[[s0, b], [s1, b], r1, r0], vx, vy],
      [[[s0, a], r0, [s0, b]], -ux, -uy],
      [[[s1, a], r1, [s1, b]], ux, uy],
    ];
    for (const [f, nx, ny] of faces) g.poly(poly(...f)).fill(shadeColor(SLATE, faceLight(nx, ny)));
    // Slate courses parallel to the eaves.
    for (let d = PX * 0.45; d < h - PX * 0.1; d += PX * 0.45) {
      g.moveTo(...P(s0 + d, a + d)).lineTo(...P(s1 - d, a + d)).lineTo(...P(s1 - d, b - d)).lineTo(...P(s0 + d, b - d)).lineTo(...P(s0 + d, a + d));
    }
    g.stroke({ width: 1, color: 0x2f353b, alpha: 0.35 });
    g.moveTo(...P(s0, a)).lineTo(...P(...r0)).lineTo(...P(s0, b)).moveTo(...P(s1, a)).lineTo(...P(...r1)).lineTo(...P(s1, b));
    g.stroke({ width: 1.5, color: 0x8a9099, alpha: 0.8 });
    g.moveTo(...P(...r0)).lineTo(...P(...r1)).stroke({ width: 3, color: 0x9aa0a8 });
    g.poly(rect(s0, s1, a, b)).stroke({ width: 1.8, color: ROOF_INK });
    // Chimneys astride the ridge.
    const nch = 1 + Math.floor(rnd() * 2);
    for (let c = 0; c < nch; c++) {
      const s = r0[0] + (r1[0] - r0[0]) * ((c + 1) / (nch + 1)) + (rnd() - 0.5) * PX;
      const q = rect(s - PX * 0.6, s + PX * 0.6, tm - PX * 0.4, tm + PX * 0.4);
      g.poly(q.map((v) => v + PX * 0.45)).fill({ color: 0x000000, alpha: 0.35 });
      g.poly(q).fill(0x8a7f70).stroke({ width: 1.8, color: ROOF_INK });
      for (const o of [-0.28, 0.28]) g.circle(...P(s + o * PX, tm), PX * 0.16).fill(0x1d1a17);
    }
  }
  for (let k = 1; k < bands; k++) g.moveTo(...P(s0, t0 + k * bw)).lineTo(...P(s1, t0 + k * bw));
  g.stroke({ width: 3, color: 0x6b7077 });
  // The walk's inner NW edges shade the roofs set below it.
  g.moveTo(...P(s1, t0)).lineTo(...P(s0, t0)).lineTo(...P(s0, t1));
  g.stroke({ width: PX * 0.35, color: 0x000000, alpha: 0.22 });
  const range = rect(h0, h1, 0, W);
  merlons(g, range, ...P((h0 + h1) / 2, W / 2), PX * 0.45);
  g.fill(0x57534b);
  g.poly(range).stroke({ width: 2.5, color: ROOF_INK });
  // Bartizans midway along the range's long walls (none where another piece meets the wall).
  if (h1 - h0 > PX * 18) {
    for (const t of [0, W]) {
      const at = P((h0 + h1) / 2, t);
      if (clearOf(...at) > PX * 3) roundTower(g, ...at, PX * 1.1, lift * 0.8, false);
    }
  }
  // The donjon, a storey or two above the range: its shadow falls across the range roofs.
  if (D > 0) {
    const dj = rect(d0, d1, 0, W);
    g.poly(dj).fill(0x8f897d);
    // Its top (`keepTop`): open battlements round a flat of dark lead with a cap house, or a roof
    // (the banner on its ridge).
    keepTop(g, dj, top, true);
    const pw = PX * 1.3;
    if (top !== 0) banner(g, ...P((d0 + d1) / 2, W / 2));
    // The stair turret on the roof, the banner above it (among other pieces, at the flats' corner
    // farthest from them, if one is clear).
    const inner = donjonLow ? d1 - pw - PX * 1.6 : d0 + pw + PX * 1.6;
    const outer = donjonLow ? d0 + pw + PX * 1.6 : d1 - pw - PX * 1.6;
    const spots: [number, number][] = avoid.length
      ? [P(inner, pw + PX * 1.6), P(inner, W - pw - PX * 1.6), P(outer, pw + PX * 1.6), P(outer, W - pw - PX * 1.6)]
      : [P(inner, pw + PX * 1.6)];
    let best = spots[0];
    for (const q of spots) if (clearOf(...q) > clearOf(...best)) best = q;
    if (top === 0 && (!avoid.length || clearOf(...best) > PX * 1.25 + PX * 1.5)) {
      roundTower(g, ...best, PX * 1.25, lift * 1.6, false);
      banner(g, ...best);
    }
  }
  // Corner towers above all; with no donjon the great one carries the banner. (A keep built of
  // pieces draws its own towers: `drawKeepParts`.)
  if (!cornerTowers) return;
  const rt = Math.max(tower * 0.6, Math.min(PX * 2.6, W * 0.11));
  const inset = rt * 0.55;
  const corners: [number, number][] = [
    [inset, inset],
    [L - inset, inset],
    [L - inset, W - inset],
    [inset, W - inset],
  ];
  const great = Math.floor(rnd() * 4);
  corners.forEach(([s, t], i) => {
    const onDonjon = D > 0 && s >= d0 && s <= d1;
    roundTower(g, ...P(s, t), onDonjon ? rt * 1.15 : rt, lift * (onDonjon ? 2.2 : 1.3), onDonjon);
  });
  if (D === 0) banner(g, ...P(...corners[great]));
}

/** A square tower's roof from above: its shadow, the stone below the eaves and a pyramid of slate
 * in lit and shaded faces, its ridges and a finial. */
function squareTowerRoof(g: Graphics, pts: number[], lift: number) {
  let [cx, cy] = [0, 0];
  for (let i = 0; i < 4; i++) [cx, cy] = [cx + pts[i * 2] / 4, cy + pts[i * 2 + 1] / 4];
  g.poly(pts.map((v) => v + lift)).fill({ color: 0x000000, alpha: 0.32 });
  g.poly(pts).fill(0x8f897d).stroke({ width: 2.5, color: ROOF_INK });
  const eave = pts.map((v, k) => (k % 2 ? cy + (v - cy) * 0.9 : cx + (v - cx) * 0.9));
  for (let i = 0; i < 4; i++) {
    const [ax, ay, bx, by] = [eave[i * 2], eave[i * 2 + 1], eave[((i + 1) % 4) * 2], eave[((i + 1) % 4) * 2 + 1]];
    const [mx, my] = [(ax + bx) / 2 - cx, (ay + by) / 2 - cy];
    const l = Math.hypot(mx, my) || 1;
    g.poly([ax, ay, bx, by, cx, cy]).fill(shadeColor(SLATE, faceLight(mx / l, my / l)));
  }
  // Slate courses parallel to the eaves, the hips to the apex.
  for (const f of [0.3, 0.55, 0.78]) g.poly(eave.map((v, k) => (k % 2 ? cy + (v - cy) * f : cx + (v - cx) * f))).stroke({ width: 1, color: 0x2f353b, alpha: 0.45 });
  for (let i = 0; i < 4; i++) g.moveTo(eave[i * 2], eave[i * 2 + 1]).lineTo(cx, cy);
  g.stroke({ width: 1.5, color: 0x8a9099, alpha: 0.8 });
  g.poly(eave).stroke({ width: 2, color: ROOF_INK });
  g.circle(cx, cy, PX * 0.14).fill(0xc9a227).stroke({ width: 1, color: ROOF_INK });
}

/**
 * A shell keep from above (`drawKeepParts`): a round curtain with its wall walk and merlons, lean-to
 * slate roofs against its inside round an open, flagged yard with a well; its gate at its first
 * corner (toward the bailey's gate, where the path comes up the motte).
 */
function drawShellKeep(g: Graphics, pts: number[]) {
  const m = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
  const R = Math.hypot(pts[0] - cx, pts[1] - cy);
  const ring = (r: number) => {
    const out: number[] = [];
    for (let k = 0; k < 40; k++) out.push(cx + Math.cos((k / 40) * Math.PI * 2) * r, cy + Math.sin((k / 40) * Math.PI * 2) * r);
    return out;
  };
  g.poly(ring(R).map((v) => v + PX * 1.2)).fill({ color: 0x000000, alpha: 0.3 });
  g.poly(ring(R)).fill(0x9a958a);
  const walk = R - PX * 1.2;
  const lean = walk - PX * 3.2;
  // Lean-to roofs: sloping in from the walk to the yard, shaded as they face.
  const n = 16;
  for (let k = 0; k < n; k++) {
    const [a0, a1] = [(k / n) * Math.PI * 2, ((k + 1) / n) * Math.PI * 2];
    const am = (a0 + a1) / 2;
    const q = [cx + Math.cos(a0) * walk, cy + Math.sin(a0) * walk, cx + Math.cos(a1) * walk, cy + Math.sin(a1) * walk, cx + Math.cos(a1) * lean, cy + Math.sin(a1) * lean, cx + Math.cos(a0) * lean, cy + Math.sin(a0) * lean];
    // (The face slopes down toward the yard: it faces inward.)
    g.poly(q).fill(shadeColor(SLATE, faceLight(-Math.cos(am), -Math.sin(am))));
  }
  for (const f of [0.33, 0.66]) g.poly(ring(lean + (walk - lean) * f)).stroke({ width: 1, color: 0x2f353b, alpha: 0.4 });
  for (let k = 0; k < n; k += 2) {
    const a = (k / n) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * lean, cy + Math.sin(a) * lean).lineTo(cx + Math.cos(a) * walk, cy + Math.sin(a) * walk);
  }
  g.stroke({ width: 1.5, color: 0x6b7077, alpha: 0.8 });
  // The yard: flags, its edge in the roofs' shadow, a well.
  g.poly(ring(lean)).fill(0xb0a692).stroke({ width: 2, color: ROOF_INK });
  for (let x = cx - lean; x < cx + lean; x += PX * 0.8) {
    const h = Math.sqrt(Math.max(0, lean * lean - (x - cx) ** 2));
    g.moveTo(x, cy - h).lineTo(x, cy + h);
  }
  g.stroke({ width: 1, color: 0x6e675c, alpha: 0.35 });
  g.circle(cx - PX * 0.3, cy - PX * 0.3, lean).stroke({ width: PX * 0.6, color: 0x000000, alpha: 0.15 });
  g.circle(cx, cy, PX * 0.55).fill(0x5d6670).stroke({ width: 2, color: ROOF_INK });
  g.circle(cx, cy, PX * 0.3).fill(0x1d2a33);
  // The curtain's walk and merlons.
  merlons(g, ring(R), cx, cy, PX * 0.45);
  g.fill(0x57534b);
  g.poly(ring(R)).stroke({ width: 3, color: ROOF_INK });
  g.poly(ring(walk)).stroke({ width: 1.5, color: 0x6f6a61 });
  // The gate: a passage through the wall and the lean-tos to the yard, its doors at the outer face,
  // a turret either side.
  const ga = Math.atan2(pts[1] - cy, pts[0] - cx);
  const [ux, uy] = [Math.cos(ga), Math.sin(ga)];
  const [vx, vy] = [-uy, ux];
  const hw = PX * 0.7;
  const at = (r: number, s: number): [number, number] => [cx + ux * r + vx * s, cy + uy * r + vy * s];
  g.poly([...at(lean - PX * 0.2, -hw), ...at(R + PX * 0.3, -hw), ...at(R + PX * 0.3, hw), ...at(lean - PX * 0.2, hw)])
    .fill(0x9c917e)
    .stroke({ width: 2, color: ROOF_INK });
  for (let q = lean + PX * 0.4; q < R; q += PX * 0.5) g.moveTo(...at(q, -hw)).lineTo(...at(q, hw));
  g.stroke({ width: 1, color: 0x6e675c, alpha: 0.5 });
  g.moveTo(...at(R + PX * 0.1, -hw))
    .lineTo(...at(R + PX * 0.1, hw))
    .stroke({ width: PX * 0.25, color: 0x5a4030 });
  for (const s of [-1, 1]) roundTower(g, ...at(R - PX * 0.3, s * (hw + PX * 0.75)), PX * 0.85, PX * 0.3, false);
  // (The banner over the wall across from the gate.)
  banner(g, cx - ux * R * 0.72, cy - uy * R * 0.72);
}

/**
 * A keep built of pieces (`Roof.parts`): its wings and forebuilding under flat leaded roofs inside
 * their parapets, the body over them (a rectangle as `drawKeep` draws a keep, without its corner
 * towers; a round one as a shell keep), its towers over all: round ones under conical slate roofs,
 * square ones under slate pyramids.
 */
function drawKeepParts(g: Graphics, r: Roof) {
  // A flat roof of dark grey lead inside its parapet walk, lead rolls across it along its length.
  const flat = (pts: number[]) => {
    const m = pts.length / 2;
    let [cx, cy] = [0, 0];
    for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
    g.poly(pts).fill(0x9a958a);
    g.poly(pts).stroke({ width: PX * 0.72, color: 0x6f6a61, alignment: 1 });
    const lead = pts.map((v, k) => (k % 2 ? cy + (v - cy) * 0.7 : cx + (v - cx) * 0.7));
    g.poly(lead).fill(LEAD).stroke({ width: 1.5, color: 0x3d4146 });
    if (m === 4) {
      // (Along its longer side, from its first corner.)
      let [ux, uy] = [lead[2] - lead[0], lead[3] - lead[1]];
      let [vx, vy] = [lead[6] - lead[0], lead[7] - lead[1]];
      if (Math.hypot(vx, vy) > Math.hypot(ux, uy)) [ux, uy, vx, vy] = [vx, vy, ux, uy];
      const L = Math.hypot(ux, uy) || 1;
      for (let s = PX * 1.2; s < L - PX * 0.3; s += PX * 1.2) {
        const [x, y] = [lead[0] + (ux * s) / L, lead[1] + (uy * s) / L];
        g.moveTo(x, y).lineTo(x + vx, y + vy);
      }
      g.stroke({ width: 2, color: LEAD_ROLL, alpha: 0.6 });
      // (Its north-west edges in the parapet's shadow.)
      g.moveTo(lead[2], lead[3]).lineTo(lead[0], lead[1]).lineTo(lead[6], lead[7]);
      g.stroke({ width: PX * 0.4, color: 0x000000, alpha: 0.25 });
    }
    merlons(g, pts, cx, cy, PX * 0.45);
    g.fill(0x57534b);
    g.poly(pts).stroke({ width: 2.5, color: ROOF_INK });
  };
  for (const p of r.parts) if (p.kind === 3 || p.kind === 4) (p.pts.length === 8 ? keepTop(g, p.pts, p.top & 3, false) : flat(p.pts));
  for (const p of r.parts) {
    if (p.kind !== 0) continue;
    if (p.pts.length === 8)
      drawKeep(
        g,
        p.pts,
        0,
        false,
        r.parts.filter((o) => o !== p).map((o) => o.pts),
        p.top & 3,
        (p.top & DONJON_LOW) !== 0,
      );
    else drawShellKeep(g, p.pts);
  }
  const lift = PX * 0.35;
  for (const p of r.parts) {
    if (p.kind !== 1 && p.kind !== 2) continue;
    const m = p.pts.length / 2;
    let [cx, cy] = [0, 0];
    for (let i = 0; i < m; i++) [cx, cy] = [cx + p.pts[i * 2] / m, cy + p.pts[i * 2 + 1] / m];
    if (p.kind === 1) roundTower(g, cx, cy, Math.hypot(p.pts[0] - cx, p.pts[1] - cy), lift * 1.6, false);
    else squareTowerRoof(g, p.pts, lift * 1.6);
  }
}

/** How far (x, y) stands outside a convex polygon `pts` (px; negative inside, by its nearest edge). */
function polyClear(pts: number[], x: number, y: number): number {
  const m = pts.length / 2;
  let inside = true;
  let sign = 0;
  let d = Infinity;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const [ex, ey] = [bx - ax, by - ay];
    const c = ex * (y - ay) - ey * (x - ax);
    if (c !== 0) {
      if (sign === 0) sign = Math.sign(c);
      else if (Math.sign(c) !== sign) inside = false;
    }
    const l2 = ex * ex + ey * ey || 1;
    const t = Math.max(0, Math.min(1, ((x - ax) * ex + (y - ay) * ey) / l2));
    d = Math.min(d, Math.hypot(ax + ex * t - x, ay + ey * t - y));
  }
  return inside ? -d : d;
}

/** A flat stone roof with a parapet and merlons (a keep: `drawKeep`). */
function drawBattlements(g: Graphics, r: Roof) {
  const { pts, tower } = r;
  const m = pts.length / 2;
  if (r.parts.length) return drawKeepParts(g, r);
  if (tower > 0 && m === 4) return drawKeep(g, pts, tower, true, [], r.top);
  // (A castle's keep with no corner towers, roofed: `keepTop`, its shadow under it.)
  if (r.top !== 0 && m === 4) {
    g.poly(pts.map((v) => v + PX * 1.1)).fill({ color: 0x000000, alpha: 0.28 });
    return keepTop(g, pts, r.top, true);
  }
  let cx = 0;
  let cy = 0;
  for (let i = 0; i < m; i++) {
    cx += pts[i * 2] / m;
    cy += pts[i * 2 + 1] / m;
  }
  const merlon = PX * 0.45;
  g.poly(pts).fill(0x9a958a);
  // The parapet: a darker band just inside the outline, merlons on it.
  g.poly(pts).stroke({ width: merlon * 1.6, color: 0x6f6a61, alignment: 1 });
  merlons(g, pts, cx, cy, merlon);
  g.fill(0x57534b);
  g.poly(pts).stroke({ width: 2.5, color: ROOF_INK });
}
