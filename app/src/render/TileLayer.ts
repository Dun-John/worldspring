// Terrain tile layer: picks the LOD for the camera, requests missing tiles (all coarser
// levels covering the view too, so there is always a fallback), draws the best available
// tile for every spot (never blank), fades new tiles in, and keeps a GPU LRU.
import { BufferImageSource, Container, Geometry, GlProgram, Mesh, RenderTexture, Shader, Texture, UniformGroup, type Renderer } from 'pixi.js';
import type { GenClient } from '../gen/client';
import { TILE_N, TILE_SAMPLES, buildRiverGeometry, buildSiteGeometry, halfToFloat, tileId, type Geom, type Rect, type RiverGeometry, type TileMsg, type WantTile } from '../gen/protocol';
import type { Camera } from './camera';
import fragment from './shaders/terrain.frag?raw';
import vertex from './shaders/terrain.vert?raw';
import riverFragment from './shaders/tileriver.frag?raw';
import riverVertex from './shaders/tileriver.vert?raw';
import roadFragment from './shaders/tileroad.frag?raw';
import roadVertex from './shaders/tileroad.vert?raw';
import siteFillFragment from './shaders/sitefill.frag?raw';
import siteFillVertex from './shaders/sitefill.vert?raw';
import inkFragment from './shaders/tileink.frag?raw';
import inkVertex from './shaders/tileink.vert?raw';

/** Discharge (mm·cells) of the smallest mapped river; matches worldgen RIVER_Q. */
export const RIVER_Q = 90_000;

const FADE_MS = 100;
/**
 * New tiles per frame. Pixi uploads textures lazily at render time, so a CPU time budget
 * can't see the real cost (~0.8 MB of texture per tile, a driver stall on iGPUs); a count
 * cap keeps each frame's upload bounded.
 */
const MAX_UPLOADS_PER_FRAME = 4;
/** Received tiles no longer in view are dropped past this (the coordinator still caches them). */
const MAX_STALE_UPLOADS = 48;
const GPU_TILE_CAP = 260;
/** Eviction treats each level of coarseness as this much more recently used. */
const LEVEL_KEEP_BONUS_MS = 2000;
/** Tiles are drawn at 1–2 device px per sample. */
const TARGET_TILE_PX = TILE_N;

/**
 * Where live edits changed what tiles show (created sites), by epoch: a tile made before an
 * edit that covers it is stale (redrawn when the new one arrives; late old ones are dropped).
 */
export class Edited {
  private list: { rects: Rect[]; minLevel: number; epoch: number }[] = [];

  add(rects: Rect[], minLevel: number, epoch: number) {
    this.list.push({ rects, minLevel, epoch });
  }

  /** Whether tile (level, x, y) of size `ts` ft made at `epoch` predates an edit over it. */
  staleAt(level: number, x: number, y: number, ts: number, epoch: number): boolean {
    for (const e of this.list) {
      if (e.epoch <= epoch || level < e.minLevel) continue;
      if (e.rects.some((r) => (x + 1) * ts > r[0] && x * ts < r[2] && (y + 1) * ts > r[1] && y * ts < r[3])) return true;
    }
    return false;
  }
}

interface GpuTile {
  id: string;
  level: number;
  x: number;
  y: number;
  base: number;
  gradRms: number;
  texData: Uint16Array;
  texture: Texture;
  biomeTexture: Texture;
  pat0: Float32Array;
  pat1: Float32Array;
  mesh: Mesh<Geometry, Shader>;
  river: Mesh<Geometry, Shader> | null;
  road: Mesh<Geometry, Shader> | null;
  siteFill: Mesh<Geometry, Shader> | null;
  siteInk: Mesh<Geometry, Shader> | null;
  /** The edits epoch it was made with. */
  epoch: number;
  uniforms: UniformGroup;
  grain: Float32Array;
  arrived: number;
  lastUsed: number;
}

export interface LayerStats {
  level: number;
  /** Target-level tiles intersecting the viewport (the prefetch margin is excluded). */
  targetTiles: number;
  /** ...of which delivered by the generator (uploaded or waiting to upload). */
  targetReceived: number;
  /** ...of which uploaded to the GPU. */
  targetUploaded: number;
  /** ...of which uploaded and fully faded in. */
  targetReady: number;
  gpuTiles: number;
  queuedUploads: number;
}

const UV0 = 0.5 / TILE_SAMPLES;
const UV1 = (TILE_SAMPLES - 0.5) / TILE_SAMPLES;

export class TileLayer {
  readonly container = new Container();
  readonly frameUniforms = new UniformGroup({
    uExag: { value: 1, type: 'f32' },
    uSea: { value: 0, type: 'f32' },
    uContour: { value: 1000, type: 'f32' },
    uIndexEvery: { value: 5, type: 'f32' },
    uFtPerPx: { value: 1, type: 'f32' },
    uPatMix: { value: 0, type: 'f32' },
    uPatAlpha: { value: 1, type: 'f32' },
  });
  readonly riverFrame = new UniformGroup({
    uPpf: { value: 1, type: 'f32' },
    uMinQ: { value: RIVER_Q, type: 'f32' },
  });
  /** Symbol pattern octave cell sizes (ft); set per frame by the view. */
  patternCells: [number, number] = [1024, 2048];
  stats: LayerStats = { level: 0, targetTiles: 0, targetReceived: 0, targetUploaded: 0, targetReady: 0, gpuTiles: 0, queuedUploads: 0 };

  /** Per level: terrain, then that level's rivers, then roads (bridges cross rivers). */
  private readonly levels: { terrain: Container; rivers: Container; roads: Container; sites: Container }[] = [];
  private readonly tiles = new Map<string, GpuTile>();
  private readonly uploads = new Map<string, TileMsg>();
  private readonly edited = new Edited();
  private readonly program = GlProgram.from({ vertex, fragment, name: 'terrain-tile' });
  private readonly riverProgram = GlProgram.from({ vertex: riverVertex, fragment: riverFragment, name: 'tile-river' });
  private readonly roadProgram = GlProgram.from({ vertex: roadVertex, fragment: roadFragment, name: 'tile-road' });
  private readonly siteFillProgram = GlProgram.from({ vertex: siteFillVertex, fragment: siteFillFragment, name: 'site-fill' });
  private readonly inkProgram = GlProgram.from({ vertex: inkVertex, fragment: inkFragment, name: 'tile-ink' });
  private readonly geometry = new Geometry({
    attributes: {
      aPosition: { buffer: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), format: 'float32x2' },
      aUV: { buffer: new Float32Array([UV0, UV0, UV1, UV0, UV1, UV1, UV0, UV1]), format: 'float32x2' },
    },
    indexBuffer: new Uint32Array([0, 1, 2, 0, 2, 3]),
  });
  /** Time spent uploading textures in the last update (ms). */
  lastUploadMs = 0;
  /** Tiles uploaded in the last update (for the frame profile). */
  lastUploads = 0;
  private viewRms = 0.3;

  constructor(
    private readonly geom: Geom,
    private readonly gen: GenClient,
  ) {
    for (let l = 0; l <= geom.max_level; l++) {
      const level = { terrain: new Container(), rivers: new Container(), roads: new Container(), sites: new Container() };
      this.levels.push(level);
      this.container.addChild(level.terrain, level.rivers, level.roads, level.sites);
    }
    this.frameUniforms.uniforms.uSea = geom.sea_level_ft;
  }

  receive(t: TileMsg, epoch: number) {
    t.epoch = epoch;
    // Made before an edit here (it crossed paths with it): a new one is on its way.
    if (this.stale(t)) return;
    this.uploads.set(tileId(t.level, t.x, t.y), t);
  }

  /** Created sites changed within `rects`: tiles there from `minLevel` in are made again
   * (and swapped in when they arrive). */
  invalidate(rects: Rect[], minLevel: number, epoch: number) {
    this.edited.add(rects, minLevel, epoch);
    for (const [id, m] of this.uploads) if (this.stale(m)) this.uploads.delete(id);
  }

  private stale(t: { level: number; x: number; y: number; epoch?: number }): boolean {
    return this.edited.staleAt(t.level, t.x, t.y, this.geom.domain_ft / 2 ** t.level, t.epoch ?? 0);
  }

  targetLevel(cam: Camera, resolution: number): number {
    const tilePx = this.geom.domain_ft * cam.ppf * resolution;
    const l = Math.floor(Math.log2(tilePx / TARGET_TILE_PX));
    return Math.max(0, Math.min(this.geom.max_level, l));
  }

  /**
   * @param lead world offset (ft) the camera is heading toward (~0.5 s of motion): tiles
   * there are requested now, so they are mostly ready when the camera stops.
   */
  /** Returns the terrain tiles still wanted (the view merges and sends all wants). */
  update(cam: Camera, resolution: number, now: number, lead: [number, number] = [0, 0]): WantTile[] {
    const L = this.targetLevel(cam, resolution);
    const [vx0, vy0, vx1, vy1] = cam.viewRect();
    const want: WantTile[] = [];
    /** Needed ids in upload priority order: target level first (nearest center), then coarser. */
    const needed: string[] = [];
    const target: { x: number; y: number; inView: boolean }[] = [];

    for (let l = L; l >= 0; l--) {
      const ts = this.geom.domain_ft / 2 ** l;
      const margin = l === L ? 0.25 * ts : 0;
      const [nx, ny] = this.tilesAcross(l);
      const x0 = Math.max(0, Math.floor((Math.min(vx0, vx0 + lead[0]) - margin) / ts));
      const y0 = Math.max(0, Math.floor((Math.min(vy0, vy0 + lead[1]) - margin) / ts));
      const x1 = Math.min(nx - 1, Math.floor((Math.max(vx1, vx1 + lead[0]) + margin) / ts));
      const y1 = Math.min(ny - 1, Math.floor((Math.max(vy1, vy1 + lead[1]) + margin) / ts));
      const ccx = cam.cx / ts - 0.5;
      const ccy = cam.cy / ts - 0.5;
      const level: { id: string; d: number }[] = [];
      for (let y = y0; y <= y1; y++) {
        for (let x = x0; x <= x1; x++) {
          const id = tileId(l, x, y);
          const d = Math.hypot(x - ccx, y - ccy);
          if (l === L) {
            const inView = (x + 1) * ts > vx0 && x * ts < vx1 && (y + 1) * ts > vy0 && y * ts < vy1;
            target.push({ x, y, inView });
          }
          level.push({ id, d });
          const have = this.tiles.get(id);
          if ((!have || this.stale(have)) && !this.uploads.has(id)) want.push({ level: l, x, y, pri: l * 100 + d });
        }
      }
      level.sort((a, b) => a.d - b.d);
      for (const e of level) needed.push(e.id);
    }

    this.drainUploads(needed, now);

    // Draw set: every target tile if present (fading or not), plus its nearest ready
    // ancestor wherever the target tile is missing or still fading in.
    const visible = new Set<GpuTile>();
    let inView = 0;
    let received = 0;
    let uploaded = 0;
    let ready = 0;
    let rmsSum = 0;
    for (const { x, y, inView: iv } of target) {
      const id = tileId(L, x, y);
      const t = this.tiles.get(id);
      const faded = !!t && now - t.arrived >= FADE_MS;
      if (iv) {
        inView++;
        if (t || this.uploads.has(id)) received++;
        if (t) uploaded++;
        if (faded) ready++;
      }
      if (t) {
        visible.add(t);
        rmsSum += t.gradRms;
      }
      if (faded) continue;
      for (let l = L - 1, ax = x >> 1, ay = y >> 1; l >= 0; l--, ax >>= 1, ay >>= 1) {
        const a = this.tiles.get(tileId(l, ax, ay));
        if (a) {
          visible.add(a);
          break;
        }
      }
    }

    if (visible.size) {
      const measured = rmsSum / Math.max(1, target.length);
      if (measured > 0) this.viewRms += (measured - this.viewRms) * 0.1;
    }

    for (const t of this.tiles.values()) {
      const show = visible.has(t);
      t.mesh.visible = show;
      if (t.river) t.river.visible = show;
      if (t.road) t.road.visible = show;
      if (t.siteFill) t.siteFill.visible = show;
      if (t.siteInk) t.siteInk.visible = show;
      if (!show) continue;
      t.lastUsed = now;
      const ts = this.geom.domain_ft / 2 ** t.level;
      const [sx, sy] = cam.worldToScreen(t.x * ts, t.y * ts);
      const px = ts * cam.ppf;
      t.mesh.position.set(sx, sy);
      t.mesh.scale.set(px, px);
      for (const line of [t.river, t.road, t.siteFill, t.siteInk]) {
        if (!line) continue;
        line.position.set(sx, sy);
        line.scale.set(px, px);
      }
      const u = t.uniforms.uniforms;
      u.uAlpha = Math.min(1, (now - t.arrived) / FADE_MS);
      u.uTilePx = px;
      t.grain[0] = mod(sx, 512);
      t.grain[1] = mod(sy, 512);
      // World-anchored pattern octaves: origin in cells (mod the shader's 256 period).
      for (const [pat, cellFt] of [
        [t.pat0, this.patternCells[0]],
        [t.pat1, this.patternCells[1]],
      ] as const) {
        pat[0] = mod((t.x * ts) / cellFt, 256);
        pat[1] = mod((t.y * ts) / cellFt, 256);
        pat[2] = ts / cellFt;
      }
    }

    this.evict(now);
    this.stats = {
      level: L,
      targetTiles: inView,
      targetReceived: received,
      targetUploaded: uploaded,
      targetReady: ready,
      gpuTiles: this.tiles.size,
      queuedUploads: this.uploads.size,
    };
    return want;
  }

  /** Mean gradient RMS over the current view, for adaptive hillshade exaggeration. */
  get reliefRms(): number {
    return this.viewRms;
  }

  /** Elevation (ft) at a world point from the finest loaded tile, or null. */
  heightAt(wx: number, wy: number): number | null {
    for (let l = this.geom.max_level; l >= 0; l--) {
      const ts = this.geom.domain_ft / 2 ** l;
      const x = Math.floor(wx / ts);
      const y = Math.floor(wy / ts);
      const t = this.tiles.get(tileId(l, x, y));
      if (!t) continue;
      const fx = ((wx - x * ts) / ts) * TILE_N;
      const fy = ((wy - y * ts) / ts) * TILE_N;
      const i = Math.min(TILE_N, Math.max(0, Math.round(fx)));
      const j = Math.min(TILE_N, Math.max(0, Math.round(fy)));
      return t.base + halfToFloat(t.texData[(j * TILE_SAMPLES + i) * 4]);
    }
    return null;
  }

  destroy() {
    for (const t of this.tiles.values()) this.destroyTile(t);
    this.tiles.clear();
    this.container.destroy({ children: true });
  }

  private tilesAcross(level: number): [number, number] {
    const ts = this.geom.domain_ft / 2 ** level;
    return [Math.max(1, Math.ceil(this.geom.map_w_ft / ts)), Math.max(1, Math.ceil(this.geom.map_h_ft / ts))];
  }

  /** Upload received tiles in view-priority order within a frame-time budget. */
  private drainUploads(needed: string[], now: number) {
    const start = performance.now();
    this.lastUploadMs = 0;
    let n = 0;
    for (const id of needed) {
      const m = this.uploads.get(id);
      if (!m) continue;
      if (n >= MAX_UPLOADS_PER_FRAME) break;
      this.uploads.delete(id);
      this.upload(m, now);
      n++;
    }
    this.lastUploads = n;
    this.lastUploadMs = performance.now() - start;
    // Tiles for views already passed: keep a few (panning back is common), drop the rest.
    if (this.uploads.size > MAX_STALE_UPLOADS) {
      const keep = new Set(needed);
      for (const id of this.uploads.keys()) {
        if (this.uploads.size <= MAX_STALE_UPLOADS) break;
        if (!keep.has(id)) this.uploads.delete(id);
      }
    }
  }

  private upload(m: TileMsg, now: number) {
    const id = tileId(m.level, m.x, m.y);
    // An edited tile replaces the one it updates at once (no fade).
    const old = this.tiles.get(id);
    if (old) {
      if (!this.stale(old)) return;
      this.tiles.delete(id);
      this.destroyTile(old);
    }
    const source = new BufferImageSource({
      resource: m.tex,
      width: TILE_SAMPLES,
      height: TILE_SAMPLES,
      format: 'rgba16float',
      // Data, not color: premultiply-on-upload is also invalid for typed-array uploads.
      alphaMode: 'no-premultiply-alpha',
      scaleMode: 'linear',
      addressMode: 'clamp-to-edge',
    });
    const texture = new Texture({ source });
    const biomeSource = new BufferImageSource({
      resource: m.biome,
      width: TILE_SAMPLES,
      height: TILE_SAMPLES,
      format: 'rgba8unorm',
      alphaMode: 'no-premultiply-alpha',
      // Linear for the coast-distance channel; biome ids are read with texelFetch.
      scaleMode: 'linear',
      addressMode: 'clamp-to-edge',
    });
    const biomeTexture = new Texture({ source: biomeSource });
    const grain = new Float32Array(2);
    const pat0 = new Float32Array(4);
    const pat1 = new Float32Array(4);
    // Where the map ends, in tile units: an edge tile's part past it is not drawn (its data
    // there is the edge drawn out into streaks).
    const ts = this.geom.domain_ft / 2 ** m.level;
    const mapEnd = new Float32Array([this.geom.map_w_ft / ts - m.x, this.geom.map_h_ft / ts - m.y]);
    const uniforms = new UniformGroup({
      uBase: { value: m.base, type: 'f32' },
      uAlpha: { value: 0, type: 'f32' },
      uMapEnd: { value: mapEnd, type: 'vec2<f32>' },
      uGrainOrigin: { value: grain, type: 'vec2<f32>' },
      uTilePx: { value: 256, type: 'f32' },
      uPat0: { value: pat0, type: 'vec4<f32>' },
      uPat1: { value: pat1, type: 'vec4<f32>' },
    });
    const shader = new Shader({
      glProgram: this.program,
      resources: { uTex: source, uBio: biomeSource, tileUniforms: uniforms, frameUniforms: this.frameUniforms },
    });
    const mesh = new Mesh({ geometry: this.geometry, shader });
    mesh.visible = false;
    this.levels[m.level].terrain.addChild(mesh);
    const river = this.lineMesh(m.rivers, this.riverProgram, uniforms);
    if (river) this.levels[m.level].rivers.addChild(river);
    const road = this.lineMesh(m.roads, this.roadProgram, uniforms);
    if (road) this.levels[m.level].roads.addChild(road);
    const siteFill = this.fillMesh(m.sites?.fill ?? null, uniforms);
    if (siteFill) this.levels[m.level].sites.addChild(siteFill);
    const siteInk = this.lineMesh(m.sites?.ink ?? null, this.inkProgram, uniforms);
    if (siteInk) this.levels[m.level].sites.addChild(siteInk);
    this.tiles.set(id, {
      id,
      level: m.level,
      x: m.x,
      y: m.y,
      base: m.base,
      gradRms: m.gradRms,
      texData: m.tex,
      texture,
      biomeTexture,
      pat0,
      pat1,
      mesh,
      river,
      road,
      siteFill,
      siteInk,
      uniforms,
      grain,
      epoch: m.epoch ?? 0,
      arrived: old ? now - FADE_MS : now,
      lastUsed: now,
    });
  }

  private evict(now: number) {
    if (this.tiles.size <= GPU_TILE_CAP) return;
    // LRU, but coarse tiles count as more recent: they cover more ground and are the
    // fallback everywhere, so zooming back out should find them still resident.
    const keepScore = (t: GpuTile) => t.lastUsed + (this.geom.max_level - t.level) * LEVEL_KEEP_BONUS_MS - now;
    const candidates = [...this.tiles.values()]
      .filter((t) => !t.mesh.visible && t.level > 1)
      .sort((a, b) => keepScore(a) - keepScore(b));
    for (const t of candidates) {
      if (this.tiles.size <= GPU_TILE_CAP) break;
      this.tiles.delete(t.id);
      this.destroyTile(t);
    }
  }

  /** Triangulated settlement polygons (built in the coordinator). */
  private fillMesh(g: NonNullable<TileMsg['sites']>['fill'], tileUniforms: UniformGroup): Mesh<Geometry, Shader> | null {
    if (!g) return null;
    const geometry = new Geometry({
      attributes: {
        aPos: { buffer: g.pos, format: 'float32x2' },
        aColor: { buffer: g.color, format: 'float32x4' },
        aEdge: { buffer: g.edge, format: 'float32x3' },
      },
      indexBuffer: g.index,
    });
    const shader = new Shader({ glProgram: this.siteFillProgram, resources: { tileUniforms, riverFrame: this.riverFrame } });
    const mesh = new Mesh({ geometry, shader });
    mesh.visible = false;
    return mesh;
  }

  /** One capsule quad per river/road segment the tile owns (geometry built in the coordinator). */
  private lineMesh(g: RiverGeometry | null, glProgram: GlProgram, tileUniforms: UniformGroup): Mesh<Geometry, Shader> | null {
    if (!g) return null;
    const geometry = new Geometry({
      attributes: {
        aA: { buffer: g.a, format: 'float32x2' },
        aB: { buffer: g.b, format: 'float32x2' },
        aCorner: { buffer: g.corner, format: 'float32x2' },
        aW: { buffer: g.w, format: 'float32x2' },
        aQ: { buffer: g.q, format: 'float32x2' },
      },
      indexBuffer: g.index,
    });
    const shader = new Shader({
      glProgram,
      resources: { tileUniforms, riverFrame: this.riverFrame },
    });
    const mesh = new Mesh({ geometry, shader });
    mesh.visible = false;
    return mesh;
  }

  /**
   * Compile every tile shader by drawing one synthetic tile (terrain, river, road, settlement
   * fill and ink) offscreen, so the first real tiles don't stall a frame. Programs are shared
   * by source, so a throwaway layer warms them for every later one.
   */
  warmup(renderer: Renderer) {
    const n = TILE_SAMPLES * TILE_SAMPLES;
    const line = () => buildRiverGeometry({ starts: new Uint32Array([0, 2]), verts: new Float32Array([0.1, 0.1, 20, RIVER_Q * 4, 0.9, 0.9, 20, RIVER_Q * 4]) });
    const sites = buildSiteGeometry(
      { starts: new Uint32Array([0, 3]), attrs: new Uint32Array([0]), verts: new Float32Array([0.1, 0.1, 0.9, 0.1, 0.5, 0.9]) },
      { starts: new Uint32Array([0, 2]), verts: new Float32Array([0.1, 0.1, 9, 0, 0.9, 0.9, 9, 0]) },
    );
    this.upload({ level: 0, x: 0, y: 0, base: 0, min: 0, max: 0, gradRms: 0, tex: new Uint16Array(n * 4), biome: new Uint8Array(n * 4), rivers: line(), roads: line(), sites }, 0);
    const t = this.tiles.get(tileId(0, 0, 0));
    if (!t) return;
    for (const o of [t.mesh, t.river, t.road, t.siteFill, t.siteInk]) {
      if (!o) continue;
      o.visible = true;
      o.scale.set(16);
    }
    const target = RenderTexture.create({ width: 16, height: 16 });
    renderer.render({ container: this.container, target });
    target.destroy(true);
    this.tiles.delete(t.id);
    this.destroyTile(t);
  }

  private destroyTile(t: GpuTile) {
    t.river?.destroy({ children: true });
    t.road?.destroy({ children: true });
    t.siteFill?.destroy({ children: true });
    t.siteInk?.destroy({ children: true });
    t.mesh.destroy();
    t.texture.destroy(true);
    t.biomeTexture.destroy(true);
  }
}

function mod(a: number, n: number): number {
  return ((a % n) + n) % n;
}
