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
import { builtinKind, FIELD, fromHalf, HS, kindRules, prepareChunk, SPRITE_KIND, SQ, toHalf, type ChunkSprite, type PreparedChunk, type Shape } from '../gen/battlePrep';
import type { KindInfo } from '../gen/client';
import type { Geom, Placed, Rect, SpriteMeta, WantTile } from '../gen/protocol';
import { dmOnlyHazard, UNKNOWN, type TacticalGrid } from '../play/vision';
import { Edited } from './TileLayer';
import { looks, PX, UPRIGHT_FOOT, type Atlas } from './atlas';
import { sprites } from './spriteCode';
import { deckGrid, drawDeck, drawFerry, drawFord, drawRoadBridge } from './bridges';
import { boundAt, isBound, SAILS } from './countryside';
import { drawBreakwaterArmour, drawCog, drawLantern, drawQuayCoping, drawRetainingWall } from './harbourShapes';
import { drawBastion, drawDitchBank, drawDrawbridge, drawDrawbridgeSill, drawGravelWalk, drawRaisedDrawbridge, topGrid } from './walls';
import { customTexture, onLoaded } from './customAtlas';
import { noiseSource } from './noise';
import { SQUARE_FT } from './scale';
import { bridgeCells, bridgeKey, drawDais, drawStructures } from './battlemap/structures';
import { drawBattlements } from './battlemap/keeps';
import { sheetFor, thumbSheet, type ThumbSheet } from './thumbs';
import type { Camera } from './camera';
import atmoFragment from './shaders/atmosphere.frag?raw';
import colorMeshFragment from './shaders/colormesh.frag?raw';
import colorMeshVertex from './shaders/colormesh.vert?raw';
import groundFragment from './shaders/ground.frag?raw';
import vertex from './shaders/terrain.vert?raw';

const CHUNK_CAP = 24;
/** Battlemap fades in between these ft-per-pixel values (5-ft square ≈ 7 px → 17 px). */
const FADE_START = 0.7;
const FADE_END = 0.3;
/** How far into that fade (0-1) the objects are already opaque. */
const OBJECTS_OPAQUE_AT = 0.3;
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
  roofMesh: Mesh<Geometry, Shader> | null;
  /** Battlemented and leaded roofs (keeps, castle ranges, towers' flats): hidden with the roof mesh. */
  battleRoofs: Graphics;
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
  /** v63 countryside: its hedgerows, dry-stone walls and fences (`countryside.ts`), for inspection. */
  bounds: Shape[];
  /** Uploaded sprites its objects use. */
  sprites: ChunkSprite[];
  /** Its castles' drawbridges: the deck with its grid (lowered) and the stub at the gate (raised),
   * shown by play's door state (`raisedOf`). */
  bridges: { key: string; deck: Container; stub: Graphics }[];
  /** Its squares' play flags as they came, where a raised drawbridge changed them. */
  bridgeOrig: Map<number, number> | null;
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
  'tundra grass',
];
/** Cobbles' tints by culture (`ground.frag` `uStone`, times the cobble grey; `Layout::culture`):
 * north cool blue-grey granite, west warm grey-buff, south warm honey limestone, dry pale ochre
 * sandstone, steppe khaki fieldstone. Each within about 8% of the grey's value, saturation <= 0.35. */
const STONE_TONES = [
  [0.96, 0.97, 1.04],
  [1.01, 1.0, 0.97],
  [1.07, 1.04, 0.89],
  [1.15, 1.08, 0.95],
  [0.99, 1.0, 0.89],
];
/** A chunk's `stone` word's tint: its culture's, varied a little by city; none (roads out of
 * town) the plain grey. */
function stoneTone(stone: number): Float32Array {
  if (!stone) return new Float32Array([1, 1, 1]);
  const t = STONE_TONES[((stone >>> 24) + 4) % 5];
  const h = Math.imul(stone & 0xffffff, 0x9e3779b1) >>> 0;
  // (Up to 3% in value and 2% in warmth either way.)
  const v = 1 + 0.06 * ((h >>> 24) / 255 - 0.5);
  const warm = 0.04 * (((h >>> 16) & 255) / 255 - 0.5);
  return new Float32Array([t[0] * v * (1 + warm), t[1] * v, t[2] * v * (1 - warm)]);
}

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
  /** The roof patterns' scale (`colormesh.frag`: their detail fades out under ~3 px). */
  private readonly roofFrame = new UniformGroup({ uPxPerSq: { value: 16, type: 'f32' } });
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
  /** Its shader programs (compiled ahead: `precompile`). */
  get programs(): GlProgram[] {
    return [this.groundProgram, this.atmoProgram, this.colorMeshProgram];
  }
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
    return { texture, rotation: drawnRotation(builtinKind(this.catalog, o.kind), !!o.turned, o.rot), scale: o.scale };
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
    // ..., one object, road halo and cover, buildings (one, for the roof shader).
    const buf = new ArrayBuffer(32 + nh * 8 + n * 4 + 20 + 2 * nh + n * 2 + 4 + 7 + 3 * 8);
    const dv = new DataView(buf);
    dv.setUint32(16, 1, true); // one object
    dv.setUint32(20, 1, true); // mist, to compile the atmosphere shader
    const o = 32 + nh * 8 + n * 4;
    dv.setUint16(o, 1, true);
    dv.setFloat32(o + 16, 1, true);
    const r = o + 20 + 2 * nh + n * 2;
    dv.setUint32(r, 1, true);
    // (Its header: corners, floors, roof, tower, material, shape, weathering; a triangle after it.)
    dv.setUint8(r + 4, 3);
    dv.setFloat32(r + 11 + 8, 1, true);
    dv.setFloat32(r + 11 + 16, 1, true);
    dv.setFloat32(r + 11 + 20, 1, true);
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
    return (c.data[k * 4 + 3] !== 0 && c.surface[k] !== FIELD) || (!!c.raised && c.raised[k * 2] > 0 && c.raised[k * 2 + 1] === 8);
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
      const inf = custom ? { ...custom, name: this.sprites[custom.asset]?.name || 'custom object', hazard: null } : builtinKind(this.catalog, o.kind);
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
    // (A hedgerow, a dry-stone wall or a fence: cover, seen over.)
    const bound = object ? null : boundAt(c.bounds, sx * PX, sy * PX);
    if (bound) object = { name: bound.name, cover: COVER[bound.cover], notes: bound.notes };
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

  /** Play's drawbridges raised (by key, `surface#bridge:<i>:<j>`); null: all lowered (play off). */
  raisedOf: ((key: string) => boolean) | null = null;
  /** The drawbridges of chunks in memory (by their owner, the chunk holding the deck's middle): per
   * square it covers (world squares), the play flags it adds raised. */
  private readonly bridgeReg = new Map<string, { cells: number[]; owner: ChunkView }>();

  /** The drawbridge whose deck or gate square is world square (`i`, `j`), by its door key. */
  bridgeAt(i: number, j: number): string | null {
    for (const [key, b] of this.bridgeReg) for (let o = 0; o < b.cells.length; o += 3) if (b.cells[o] === i && b.cells[o + 1] === j) return key;
    return null;
  }

  /**
   * Play's drawbridges as their door states have them (on a door change, and when chunks come and
   * go): a raised one's deck hidden and its stub shown, and its squares' play flags in every chunk
   * it crosses changed (the moat's under its deck, a wall across its gate); lowered, as they came.
   */
  refreshBridges() {
    const up = (key: string) => !!this.raisedOf?.(key);
    for (const c of this.chunks.values()) {
      if (c.bridgeOrig) for (const [k, v] of c.bridgeOrig) c.flags[k] = v;
      c.bridgeOrig = null;
      for (const b of c.bridges ?? []) {
        const raised = up(b.key);
        b.deck.visible = !raised;
        b.stub.visible = raised;
      }
    }
    for (const [key, b] of this.bridgeReg) {
      if (!up(key)) continue;
      for (let o = 0; o < b.cells.length; o += 3) {
        if (!b.cells[o + 2]) continue;
        const [i, j] = [b.cells[o], b.cells[o + 1]];
        const [cx, cy] = [Math.floor(i / SQ), Math.floor(j / SQ)];
        const c = this.chunks.get(`${cx}/${cy}`);
        if (!c) continue;
        const k = (j - cy * SQ) * SQ + i - cx * SQ;
        c.bridgeOrig ??= new Map();
        if (!c.bridgeOrig.has(k)) c.bridgeOrig.set(k, c.flags[k]);
        c.flags[k] = c.bridgeOrig.get(k)! | b.cells[o + 2];
      }
    }
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
  /** Roofs shown (hidden under the city, where only the streets and ground should read). */
  private roofsOn = true;
  setRoofs(on: boolean) {
    if (on === this.roofsOn) return;
    this.roofsOn = on;
    for (const ch of this.chunks.values()) {
      if (ch.roofMesh) ch.roofMesh.visible = on;
      ch.battleRoofs.visible = on;
    }
  }

  setGrid(on: boolean) {
    this.frame.uniforms.uGrid = on ? 1 : 0;
    for (const ch of this.chunks.values()) for (const g of ch.grids) g.visible = on;
  }

  /** Returns the battlemap chunks still wanted. */
  update(cam: Camera, now: number): WantTile[] {
    const ftPerPx = 1 / cam.ppf;
    const alpha = Math.min(1, Math.max(0, (FADE_START - ftPerPx) / (FADE_START - FADE_END)));
    const objectAlpha = Math.min(1, alpha / OBJECTS_OPAQUE_AT);
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
    this.roofFrame.uniforms.uPxPerSq = pxPerSq;
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
      // (The objects' layers, low and tall, and the walls and roofs turn opaque early in the fade, so
      // the town map's paths under the battlemap's half-faded ground never show through a pond or a
      // statue, nor its grass through a roof.)
      for (const p of c.parts) {
        p.position.set(sx, sy);
        p.scale.set(pxPerSq / PX);
        p.alpha = p === c.decks ? 1 : p === c.parts[1] || p === c.parts[2] || p === c.parts[3] ? objectAlpha : alpha;
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
    // start up the flight, tread, timber) in squares, for the ground's treads.
    const stairs = new Float32Array(STAIR_AXES * 8);
    let nStairs = 0;
    for (const s of c.shapes) {
      if (s.kind !== 8 || nStairs === STAIR_AXES) continue;
      const p = s.pts.map((v) => v / PX);
      // (Timber steps on a flight 8 ft wide or less: a footpath's; dressed stone on a street's.)
      stairs.set([p[0], p[1], p[2], p[3], (0.5 * s.size) / PX, p[4], p[5], s.size / PX <= 1.6 ? 1 : 0], nStairs++ * 8);
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
      uStone: { value: stoneTone(c.stone), type: 'vec3<f32>' },
      // (Paving here, or a field: their edges with the ground drawn soft, a field's crisp.)
      uPaved: { value: c.surface.includes(14) || c.surface.includes(18) ? 1 : 0, type: 'f32' },
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
    // (The low layer: ground marks, then the short props' shadows, then the short props.)
    const flat = new ParticleContainer({ texture: atlas.shadow, dynamicProperties: fixed });
    const lowShadows = new ParticleContainer({ texture: atlas.shadow, dynamicProperties: fixed });
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
      const standing = custom ? inf.height_ft > 0 : !hazard && isStanding(inf as KindInfo);
      const sh = standing ? dropShadowOf(inf, custom ? 0 : o.kind, !custom, o.scale, rotation) : null;
      if (sh) {
        // (A short prop's shadow lies under it and the props beside it, over the ground marks; a tall
        // one's falls over the short props and walls about it.)
        (isTall ? shadows : lowShadows).addParticle(
          new Particle({
            texture: atlas.shadow,
            x: o.x * PX + sh.dx,
            y: o.y * PX + sh.dy,
            anchorX: 0.5,
            anchorY: 0.5,
            scaleX: sh.sx,
            scaleY: sh.sy,
            rotation: sh.rot,
            alpha: sh.alpha,
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
      (isTall ? tall : standing ? low : flat).addParticle(
        new Particle({ texture: tex, x: o.x * PX, y: o.y * PX, anchorX: 0.5, anchorY: 0.5, scaleX: o.scale, scaleY: o.scale, rotation }),
      );
    }

    yield;
    for (const layer of [flat, lowShadows, low, shadows, tall]) {
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
          aRoof: { buffer: c.roofMesh.roof, format: 'float32x4' },
        },
        indexBuffer: c.roofMesh.index,
      }));
      roofMesh = new Mesh({ geometry, shader: new Shader({ glProgram: this.colorMeshProgram, resources: { roofFrame: this.roofFrame } }) });
    }
    // Daises and a court's gravel walks sit on the ground, under every prop.
    const daisG = new Graphics();
    for (const s of c.shapes) {
      if (s.kind === 5) drawDais(daisG, s);
      else if (s.kind === 13) drawGravelWalk(daisG, s);
      // R5 M20: a breakwater's armour, on the ground and the water by it.
      else if (s.kind === BREAKWATER) drawArmour(daisG, s);
      // (Roadmap 7: a stone quay front's coping along its face.)
      else if (s.kind === COPING) drawCoping(daisG, s);
      // (A retaining wall round a house's levelled yard or along a flight of steps.)
      else if (s.kind === RETAINING) drawRetaining(daisG, s);
    }
    const battleG = new Graphics();
    for (const r of c.battlements) drawBattlements(battleG, r);
    // R5 M20: a lighthouse's lamp gallery over its roof (shown with the roofs).
    for (const s of c.shapes) if (s.kind === LANTERN) drawLantern(battleG, s.pts[0], s.pts[1], (s.size / PX) * 5, PX, true);
    battleG.visible = this.roofsOn;
    // Bridges and piers, the grid over their decks (shown with the ground's), then walls with
    // their shadows; towers stand above every chunk's walls.
    const bridgeG = new Graphics();
    const gridG = new Graphics();
    for (const s of c.shapes) {
      if (s.kind === 2) drawDeck(bridgeG, s);
      else if (s.kind === 3) drawRoadBridge(bridgeG, s);
      else if (s.kind === SHIP) {
        // R5 M20: a cog moored by a pier, on the water with the decks.
        drawCog(bridgeG, s.pts[0], s.pts[1], Math.atan2(s.pts[3] - s.pts[1], s.pts[2] - s.pts[0]), (s.size / PX) * 5, PX, Math.round((s.pts[4] ?? 0) / PX));
        continue;
      }
      else if (s.kind === 6) {
        drawFord(bridgeG, s);
        continue;
      } else if (s.kind === 7) {
        drawFerry(bridgeG, s);
        continue;
      } else continue;
      deckGrid(gridG, s);
    }
    // (A dry ditch's sides, then the inked tops of its banks over them all.)
    for (const ink of [false, true]) for (const s of c.shapes) if (s.kind === 19) drawDitchBank(bridgeG, s, ink);
    // A drawbridge's deck lies over the walls and under its gatehouse (which covers its inner end);
    // its deck and grid, and the stub shown when play raises it, each on their own (shown by door
    // state). Chunks without one draw their structures in one piece.
    const drawbridges = c.shapes.filter((s) => s.kind === 18);
    const lowKind = (k: number) => k === 0 || isBound(k);
    const highKind = (k: number) => k === 4 || (k >= 10 && k <= 12);
    // (Gatehouses go under the towers but over every chunk's walls (`gateG`): a curtain owned by the next
    // chunk would otherwise end on top of its gate block. The walls still learn from them which way the
    // town lies.)
    const gates = c.shapes.filter((s) => s.kind === 9);
    const structG = drawStructures(c.shapes.filter((s) => lowKind(s.kind) || (!drawbridges.length && highKind(s.kind))), cx * SQ * PX, cy * SQ * PX, gates);
    const structHigh = drawbridges.length ? drawStructures(c.shapes.filter((s) => highKind(s.kind))) : null;
    const bridges = drawbridges.map((s) => {
      drawDrawbridgeSill(structG, s);
      const deckG = new Graphics();
      drawDrawbridge(deckG, s);
      const deckGridG = new Graphics();
      deckGrid(deckGridG, s);
      deckGridG.visible = this.frame.uniforms.uGrid > 0;
      const deck = new Container();
      deck.addChild(deckG, deckGridG);
      const stub = new Graphics();
      drawRaisedDrawbridge(stub, s);
      return { key: bridgeKey(s, cx, cy), deck, stub, grid: deckGridG };
    });
    const towerG = drawStructures(c.shapes.filter((s) => s.kind === 1 || s.kind === SAILS));
    const gateG = gates.length ? drawStructures(gates) : null;
    // A star bastion's earth top: over the flat objects, under the low ones (its props) and its curtain (drawn over its edge).
    const bastionG = new Graphics();
    for (const s of c.shapes) if (s.kind === 20) drawBastion(bastionG, s);
    const gateGridG = new Graphics();
    // The grid on the wall walks (over the walls), the tower tops (over the towers) and the gravel
    // walks (over them, on the ground).
    const walkGridG = new Graphics();
    const topGridG = new Graphics();
    const pathGridG = new Graphics();
    for (const s of c.shapes) topGrid(s.kind === 1 ? topGridG : s.kind === 9 ? gateGridG : s.kind === 13 || s.kind === 20 ? pathGridG : walkGridG, s);
    for (const gr of [gridG, walkGridG, topGridG, pathGridG, gateGridG]) gr.visible = this.frame.uniforms.uGrid > 0;
    for (const part of [roofMesh, bastionG, daisG, pathGridG, battleG, bridgeG, gridG, structG, ...bridges.flatMap((b) => [b.deck, b.stub]), structHigh, walkGridG, gateG, gateGridG, towerG, topGridG]) {
      if (!part) continue;
      this.prewarm(part);
      yield;
    }
    const parts = [[ground], [flat, lowShadows, low], [shadows], [tall]].map((os, i) => {
      const holder = new Container();
      holder.addChild(...os);
      for (const [key, pc] of customs) if ((i === 1 && key.endsWith('/0')) || (i === 3 && key.endsWith('/1'))) holder.addChild(pc);
      if (i === 0) holder.addChild(bastionG, daisG, pathGridG);
      if (i === 2) {
        holder.addChild(structG);
        for (const b of bridges) holder.addChild(b.deck);
        if (structHigh) holder.addChild(structHigh);
        for (const b of bridges) holder.addChild(b.stub);
        holder.addChild(walkGridG);
        if (roofMesh) {
          roofMesh.visible = this.roofsOn;
          holder.addChild(roofMesh);
        }
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
    if (gateG) {
      // (At the bottom of the towers' layer: over every chunk's walls and roofs, under every tower.)
      const holder = new Container();
      holder.addChild(gateG, gateGridG);
      this.layers[3].addChildAt(holder, 0);
      parts.push(holder);
    }
    let atmo: Mesh<Geometry, Shader> | null = null;
    let atmoUniforms: UniformGroup | null = null;
    if (atmosphere > 0) {
      atmoUniforms = new UniformGroup({
        uKind: { value: atmosphere, type: 'f32' },
        uEdges: { value: new Float32Array(4), type: 'vec4<f32>' },
        uAlpha: { value: 1, type: 'f32' },
        uNoiseOff: { value: new Float32Array([((cx * SQ) % 65536 + 65536) % 65536, ((cy * SQ) % 65536 + 65536) % 65536]), type: 'vec2<f32>' },
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
      roofMesh,
      battleRoofs: battleG,
      grids: [gridG, walkGridG, topGridG, pathGridG, gateGridG, ...bridges.map((b) => b.grid)],
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
      bounds: c.shapes.filter((s) => isBound(s.kind)),
      sprites: c.sprites,
      bridges: bridges.map(({ key, deck, stub }) => ({ key, deck, stub })),
      bridgeOrig: null,
      prepared: waiting ? c : null,
    });
    this.dropPreview(id, epoch);
    const view = this.chunks.get(id)!;
    for (const s of drawbridges) this.bridgeReg.set(bridgeKey(s, cx, cy), { cells: bridgeCells(s, cx, cy), owner: view });
    if (this.bridgeReg.size) this.refreshBridges();
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
    let gone = false;
    for (const b of c.bridges ?? []) {
      if (this.bridgeReg.get(b.key)?.owner !== c) continue;
      this.bridgeReg.delete(b.key);
      gone = true;
    }
    if (gone) this.refreshBridges();
    // Shaders go first: they hold the textures, and PixiJS warns of one destroyed while bound.
    for (const p of c.parts) freeShaders(p);
    for (const p of c.parts) p.destroy({ children: true });
    for (const t of c.textures) t.destroy(true);
    c.roofGeometry?.destroy();
  }
}

/** Destroy the shader of every mesh under `node` (each chunk builds its own). */
function freeShaders(node: Container) {
  const shader = (node as unknown as { shader?: Shader }).shader;
  if (shader) shader.destroy();
  for (const k of node.children) freeShaders(k as Container);
}

/** The sprite atlas depends only on the catalog: built once per page and reused by every
 * battlemap layer (worlds reload; warm-up builds it while T0 generates). */
let atlasCache: { key: string; renderer: Renderer; atlas: Atlas } | null = null;
/** Forget the atlas: it lives only on the GPU, so a lost context leaves it blank. */
export function dropSharedAtlas() {
  atlasCache = null;
}
function sharedAtlas(renderer: Renderer, catalog: KindInfo[]): Atlas {
  const key = catalog.map((k) => k.name).join('|');
  if (!atlasCache || atlasCache.key !== key || atlasCache.renderer !== renderer) atlasCache = { key, renderer, atlas: sprites().buildAtlas(renderer, catalog) };
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
const TURNED = new Set(['rowboat', 'fishing boat', 'dock crane', 'net rack', 'ballista', 'market stall', 'cart', 'tent', 'campfire', 'firewood stack', 'bedroll', 'water trough', 'bench', 'flowerbed', 'hedge', 'footbridge', 'vegetable row', 'wattle fence', 'anvil', 'water wheel', 'lychgate', 'tomb slab', 'horse', 'hitching rail', 'picket fence', 'iron railing', 'stockade', 'gate']);
/** Turned props that look the same after a half turn, by the start of the half turn (degrees) they are
 * drawn within: the turns that keep the side drawn lit within a quarter turn of the light in the NW. A hedge is
 * lit along its north side: -135° to 45°; a flowerbed on its north and west edges: -90° to 90°; a
 * footbridge toward its west end: -45° to 135°. */
const HALF_TURN = new Map([
  ['hedge', -135],
  ['flowerbed', -90],
  ['footbridge', -45],
  ['vegetable row', -135],
  ['wattle fence', -135],
  ['picket fence', -135],
  ['iron railing', -135],
  ['stockade', -135],
  ['lychgate', -135],
  ['tomb slab', -90],
  ['hitching rail', -135],
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
  ['wattle fence', [1.0, 0.36, 0.6]],
  ['picket fence', [1.0, 0.3, 0.5]],
  ['iron railing', [1.0, 0.26, 0.4]],
  ['vegetable row', [1.9, 0.7, 0.5]],
  ['horse', [1.5, 0.55, 1]],
  ['hitching rail', [2.0, 0.3, 0.6]],
]);
/** Props that draw their own shadow into their picture, so get no drop shadow. */
export const OWN_SHADOW = new Set(['market cross', 'water wheel', 'kurgan', 'rowboat', 'fishing boat', 'dock crane', 'net rack']);
/** Things lying flat on the ground (marks, beds of low plants, water): no drop shadow, however tall the
 * catalog says they stand. */
export const GROUND_MARK = new Set(['pond', 'wildflowers', 'mushroom ring', 'old bones', 'bedroll', 'flowerbed']);

/** Whether a catalog kind stands up off the ground and so casts a drop shadow (not hazards, ground marks or
 * things that draw their own). */
export function isStanding(inf: KindInfo): boolean {
  return !inf.hazard && inf.height_ft > 0 && !GROUND_MARK.has(inf.name) && !OWN_SHADOW.has(inf.name);
}

/** A standing thing's drop shadow (`atlas.shadow`, 128 px across), in atlas px about its centre at `scale`:
 * offset to the SE and sized by its height (a stone 1 ft high casts a small faint one hard by it, a 5-ft
 * crate stack a full one), drawn out along long things, from the foot of upright ones. `builtin`: a
 * catalog kind (its own rules by name and id), else an uploaded sprite. */
export function dropShadowOf(inf: { name?: string; radius: number; height_ft: number }, kind: number, builtin: boolean, scale: number, rotation: number) {
  const h = inf.height_ft;
  const name = builtin ? (inf.name ?? '') : '';
  const [size, strength] = SPARSE.get(name) ?? [1, 1];
  const long = LONG_SHADOW.get(name);
  // (Short things: a tighter, darker blob set further out, so it shows past the prop and not only
  // under it, growing as they rise; from 5 ft, the tall things' shadow.)
  const short = h < 5;
  const grow = short ? 0.85 + 0.0375 * h : 1;
  const r = inf.radius * PX * scale * size * grow;
  const foot = builtin ? (UPRIGHT_FOOT[kind] ?? 0) * scale : 0;
  const off = Math.min(1.3, 0.15 + h / 45) * PX * 0.7 * (long ? long[2] : 1) * (short ? 1.8 : 1);
  return {
    dx: off,
    dy: foot + off,
    sx: long ? (long[0] * PX * scale) / 128 : (r * 2.1) / 128,
    sy: long ? (long[1] * PX * scale) / 128 : (r * 2.1) / 128,
    rot: long ? rotation : 0,
    alpha: (short ? Math.min(0.6, 0.4 + 0.05 * h) : 0.6) * strength,
  };
}

/** R5 M20 shapes (`battlemap::ShapeKind`): a breakwater's armour, a moored cog, a lighthouse's lantern. */
const BREAKWATER = 32;
const SHIP = 33;
const LANTERN = 34;
const COPING = 35;
const RETAINING = 36;

/** A stone quay front's coping along its line (px), `size` squares across. */
function drawCoping(g: Graphics, s: Shape) {
  const pts: [number, number][] = [];
  for (let i = 0; i + 1 < s.pts.length; i += 2) pts.push([s.pts[i], s.pts[i + 1]]);
  drawQuayCoping(g, pts, (s.size / PX) * 5, Math.floor(Math.abs(s.pts[0] * 7 + s.pts[1] * 13)) % 997);
}

/** A retaining wall along its line (px), then `[rise ft, 0]` (squares to px like the rest). */
function drawRetaining(g: Graphics, s: Shape) {
  const pts: [number, number][] = [];
  for (let i = 0; i + 3 < s.pts.length; i += 2) pts.push([s.pts[i], s.pts[i + 1]]);
  drawRetainingWall(g, pts, (s.size / PX) * 5, s.pts[s.pts.length - 2] / PX, Math.floor(Math.abs(s.pts[0] * 7 + s.pts[1] * 13)) % 997);
}

/** A breakwater's armour along its line (px), the band `size` squares across. */
function drawArmour(g: Graphics, s: Shape) {
  const pts: [number, number][] = [];
  for (let i = 0; i + 1 < s.pts.length; i += 2) pts.push([s.pts[i], s.pts[i + 1]]);
  drawBreakwaterArmour(g, pts, PX, (s.size / PX) * 5, Math.floor(Math.abs(s.pts[0] * 7 + s.pts[1] * 13)) % 997);
}
