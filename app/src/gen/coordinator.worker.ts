// Coordinator worker: resolves the job dependency DAG (declared in Rust, via WASM `deps`),
// keeps an LRU of results, prioritizes what the view wants and feeds generator workers.
// The main thread only ever receives render-ready tiles.
import type { PreparedChunk } from './battlePrep';
import init, { Ctx, battlemap_catalog_json } from './pkg/worldgen_wasm.js';
import {
  KIND_BATTLEMAP,
  KIND_TERRAIN,
  battleId,
  buildRiverGeometry,
  buildSiteGeometry,
  parseTerrain,
  tileId,
  type FromCoordinator,
  type FromGen,
  type Geom,
  type Rect,
  type TerrainPayload,
  type ToCoordinator,
  type ToGen,
  type WantTile,
} from './protocol';

const CACHE_MAX = 200;
const BATTLE_CACHE_MAX = 24;
/** In LRU positions: each level of coarseness counts as this many uses more recent. */
const LEVEL_KEEP_BONUS = 12;

interface Job {
  kind: number;
  level: number;
  x: number;
  y: number;
  pri: number;
  deps: string[];
}

interface GenWorker {
  w: Worker;
  busy: boolean;
}

let ctx: Ctx | null = null;
let geom: Geom | null = null;
let workers: GenWorker[] = [];
let session = 0;
let nextJobId = 1;

const cache = new Map<string, TerrainPayload>();
/** Battlemap payloads by battleId (LRU by insertion order). */
const battleCache = new Map<string, PreparedChunk>();
let wanted = new Map<string, WantTile>();
const pending = new Map<string, Job>();
const inflight = new Map<number, { id: string; worker: GenWorker }>();
const inflightIds = new Set<string>();
/** Jobs started before an edit that touches them: their results are dropped. */
const staleInflight = new Set<string>();
/** Cached tiles drawn from ground since shaped by hand: their samples (as generated) still serve
 * finer tiles and battlemaps, but they are made again when wanted. */
const retex = new Set<string>();
/** Stale jobs that only the shaped ground made stale: their samples are kept (to be made again). */
const keepSamples = new Set<string>();
/** The latest edits (by field, patches applied) and their epoch: results sent after them carry
 * it. Workers started later get them whole. */
let editFields: Record<string, unknown> = {};
let epoch = 0;
/** Cached tiles at levels below this don't draw site layouts (`payload::SITE_MIN_LEVEL`). */
const SITE_MIN_LEVEL = 9;
let jobsDone = 0;
let msTotal = 0;

const post = (msg: FromCoordinator, transfer: Transferable[] = []) =>
  (self as unknown as Worker).postMessage(msg, transfer);

function spawnWorker(): GenWorker {
  const w = new Worker(new URL('./gen.worker.ts', import.meta.url), { type: 'module' });
  return { w, busy: false };
}

/** Send a message and resolve with the first non-progress reply (progress is forwarded). */
function request(gw: GenWorker, msg: ToGen, transfer: Transferable[] = []): Promise<FromGen> {
  return new Promise((resolve) => {
    gw.w.onmessage = (e: MessageEvent<FromGen>) => {
      const m = e.data;
      if (m.type === 'progress') post({ type: 'progress', stage: m.stage, frac: m.frac });
      else resolve(m);
    };
    gw.w.postMessage(msg, transfer);
  });
}

async function initSession(msg: Extract<ToCoordinator, { type: 'init' }>) {
  const mySession = ++session;
  workers.forEach((gw) => gw.w.terminate());
  workers = [];
  cache.clear();
  battleCache.clear();
  pending.clear();
  inflight.clear();
  inflightIds.clear();
  staleInflight.clear();
  retex.clear();
  keepSamples.clear();
  editFields = {};
  wanted = new Map();

  await init();
  post({ type: 'catalog', catalog: battlemap_catalog_json() });
  const worldJson = JSON.stringify(msg.world);
  ctx?.free();
  ctx = new Ctx(worldJson);
  geom = JSON.parse(ctx.geom_json()) as Geom;

  // One worker generates T0; the rest start now and load WASM meanwhile, then its bytes.
  const first = spawnWorker();
  const rest = Array.from({ length: Math.max(0, msg.workers - 1) }, spawnWorker);
  rest.forEach((gw) => gw.w.postMessage({ type: 'warm' } satisfies ToGen));
  const firstReply = await request(first, { type: 'init', worldJson, t0: null });
  if (firstReply.type !== 'inited' || !firstReply.t0) throw new Error(firstReply.type === 'error' ? firstReply.message : 'T0 failed');
  const t0 = firstReply.t0;
  await Promise.all(rest.map((gw) => request(gw, { type: 'init', worldJson, t0: t0.slice(0) })));
  if (mySession !== session) {
    [first, ...rest].forEach((gw) => gw.w.terminate());
    return;
  }
  workers = [first, ...rest];
  workers.forEach((gw) => (gw.w.onmessage = (e: MessageEvent<FromGen>) => onGenMessage(gw, e.data)));
  // Edits made while T0 was generating.
  if (Object.keys(editFields).length) workers.forEach((gw) => gw.w.postMessage({ type: 'edits', json: JSON.stringify(editFields), patch: null } satisfies ToGen));
  post({ type: 'ready', geom, t0Ms: firstReply.ms, overlay: firstReply.overlay ?? '{"features":[]}', catalog: battlemap_catalog_json() });
}

const jobId = (kind: number, level: number, x: number, y: number) =>
  kind === KIND_BATTLEMAP ? battleId(level, x, y) : tileId(level, x, y);

/** Dependencies are always terrain tiles (the only kind other jobs build on). */
function depsOf(kind: number, level: number, x: number, y: number): string[] {
  const flat = ctx!.deps(kind, level, x, y);
  const out: string[] = [];
  for (let i = 0; i < flat.length; i += 4) out.push(tileId(flat[i + 1] & 0xff, flat[i + 2], flat[i + 3]));
  return out;
}

const isDone = (id: string) => (cache.has(id) && !retex.has(id)) || battleCache.has(id);

function touch(id: string) {
  const v = cache.get(id);
  if (v) {
    cache.delete(id);
    cache.set(id, v);
  }
}

/** `dep`: needed only for its samples (a tile to be made again still serves). */
function schedule(kind: number, level: number, x: number, y: number, pri: number, dep = false) {
  const id = jobId(kind, level, x, y);
  if (dep ? cache.has(id) : isDone(id)) {
    touch(id);
    return;
  }
  if (inflightIds.has(id)) return;
  const existing = pending.get(id);
  if (existing) {
    existing.pri = Math.min(existing.pri, pri);
    return;
  }
  const deps = depsOf(kind, level, x, y);
  pending.set(id, { kind, level, x, y, pri, deps });
  for (const d of deps) {
    const [l, dx, dy] = d.split('/').map(Number);
    schedule(KIND_TERRAIN, l, dx, dy, pri - 0.5, true);
  }
}

function onWant(tiles: WantTile[]) {
  wanted = new Map();
  pending.clear();
  for (const t of tiles) {
    const kind = t.kind === 'battlemap' ? KIND_BATTLEMAP : KIND_TERRAIN;
    const id = jobId(kind, t.level, t.x, t.y);
    if (isDone(id)) {
      send(id);
      continue;
    }
    wanted.set(id, t);
    schedule(kind, t.level, t.x, t.y, t.pri);
  }
  pump();
}

function pump() {
  for (const gw of workers) {
    if (gw.busy) continue;
    let best: [string, Job] | null = null;
    for (const entry of pending) {
      const job = entry[1];
      if (!job.deps.every((d) => cache.has(d))) continue;
      if (!best || job.pri < best[1].pri) best = entry;
    }
    if (!best) return;
    const [id, job] = best;
    pending.delete(id);
    const parent = job.deps.length ? cache.get(job.deps[0])!.padded.slice() : null;
    const jobNo = nextJobId++;
    gw.busy = true;
    inflight.set(jobNo, { id, worker: gw });
    inflightIds.add(id);
    const msg: ToGen = { type: 'job', id: jobNo, kind: job.kind, level: job.level, x: job.x, y: job.y, parent };
    gw.w.postMessage(msg, parent ? [parent.buffer] : []);
  }
}

function onGenMessage(gw: GenWorker, m: FromGen) {
  if (m.type === 'inited' || m.type === 'progress') return;
  if (m.type === 'answer') {
    post({ type: 'answer', id: m.id, json: m.json });
    return;
  }
  gw.busy = false;
  const job = m.id !== undefined ? inflight.get(m.id) : undefined;
  if (m.id !== undefined) inflight.delete(m.id);
  if (job) inflightIds.delete(job.id);
  if (m.type === 'error') {
    post({ type: 'error', message: `generation failed for ${job?.id}: ${m.message}` });
  } else if (m.type === 'done' && job && staleInflight.delete(job.id)) {
    // Made before an edit here: make it again if it is still wanted (a tile drawn from ground
    // since shaped still serves finer ones with its samples).
    if (keepSamples.delete(job.id) && m.buf) {
      cache.set(job.id, parseTerrain(m.buf));
      retex.add(job.id);
      evict();
    }
    const want = wanted.get(job.id);
    if (want) {
      const [l, x, y] = job.id.split('/').filter((p) => p !== 'b').map(Number);
      schedule(job.id.startsWith('b/') ? KIND_BATTLEMAP : KIND_TERRAIN, l, x, y, want.pri);
    }
  } else if (m.type === 'done' && job) {
    jobsDone++;
    msTotal += m.ms;
    if (job.id.startsWith('b/')) {
      battleCache.set(job.id, m.chunk!);
      while (battleCache.size > BATTLE_CACHE_MAX) battleCache.delete(battleCache.keys().next().value!);
    } else {
      cache.delete(job.id);
      retex.delete(job.id);
      cache.set(job.id, parseTerrain(m.buf!));
      evict();
    }
    if (wanted.has(job.id)) send(job.id);
  }
  pump();
}

function send(id: string) {
  if (id.startsWith('b/')) {
    // Copied (structured clone): the cache keeps its own.
    const chunk = battleCache.get(id)!;
    wanted.delete(id);
    const [, l, x, y] = id.split('/').map(Number);
    post({ type: 'battlemap', level: l, x, y, chunk, epoch });
  } else {
    sendTile(id);
  }
}

function sendTile(id: string) {
  const r = cache.get(id)!;
  wanted.delete(id);
  const tex = r.tex.slice();
  const biome = r.biome.slice();
  const rivers = buildRiverGeometry(r.rivers);
  const transfer: Transferable[] = [tex.buffer, biome.buffer];
  const roads = buildRiverGeometry(r.roads);
  const sites = buildSiteGeometry(r.sitePolys, r.siteLines);
  for (const g of [rivers, roads, sites?.ink]) if (g) transfer.push(g.a.buffer, g.b.buffer, g.corner.buffer, g.w.buffer, g.q.buffer, g.index.buffer);
  if (sites?.fill) transfer.push(sites.fill.pos.buffer, sites.fill.color.buffer, sites.fill.edge.buffer, sites.fill.index.buffer);
  post(
    {
      type: 'tile',
      tile: { level: r.level, x: r.x, y: r.y, base: r.base, min: r.min, max: r.max, gradRms: r.gradRms, tex, biome, rivers, roads, sites },
      epoch,
    },
    transfer,
  );
}

/** New edits: every generator gets them; results they change (site-level tiles and battlemaps
 * where created sites changed) are dropped from the cache, or when being made, discarded. */
/** Edits fields (each JSON) as one JSON object. */
const fieldsJson = (f: Record<string, string>) => `{${Object.entries(f).map(([k, v]) => `${JSON.stringify(k)}:${v}`).join(',')}}`;

function onEdits(m: Extract<ToCoordinator, { type: 'edits' }>) {
  for (const [f, json] of Object.entries(m.fields)) editFields[f] = JSON.parse(json);
  for (const [f, p] of Object.entries(m.patches)) {
    const field = ((editFields[f] as Record<string, unknown> | undefined) ??= {}) as Record<string, unknown>;
    Object.assign(field, p.set);
    for (const k of p.unset) delete field[k];
  }
  epoch = m.epoch;
  const json = Object.keys(m.fields).length ? fieldsJson(m.fields) : null;
  const patch = Object.keys(m.patches).length ? JSON.stringify(m.patches) : null;
  if (json || patch) for (const gw of workers) gw.w.postMessage({ type: 'edits', json, patch } satisfies ToGen);
  const battleRects = m.battleRects ?? [];
  const groundRects = m.groundRects ?? [];
  if (!geom || (!m.rects.length && !battleRects.length && !groundRects.length)) return;
  const touched = (id: string) => {
    const battle = id.startsWith('b/');
    const [l, x, y] = (battle ? id.slice(2) : id).split('/').map(Number);
    if (!battle && l < SITE_MIN_LEVEL) return false;
    const ts = geom!.domain_ft / 2 ** l;
    const hit = (r: Rect) => (x + 1) * ts > r[0] && x * ts < r[2] && (y + 1) * ts > r[1] && y * ts < r[3];
    return m.rects.some(hit) || (battle && battleRects.some(hit));
  };
  // Ground shaped by hand: the three finest levels' tiles over it are drawn from it.
  const reshaped = (id: string) => {
    if (id.startsWith('b/') || !groundRects.length) return false;
    const [l, x, y] = id.split('/').map(Number);
    if (l < geom!.max_level - 2) return false;
    const ts = geom!.domain_ft / 2 ** l;
    return groundRects.some((r) => (x + 1) * ts > r[0] && x * ts < r[2] && (y + 1) * ts > r[1] && y * ts < r[3]);
  };
  for (const id of [...cache.keys()]) {
    if (touched(id)) {
      cache.delete(id);
      retex.delete(id);
    } else if (reshaped(id)) retex.add(id);
  }
  for (const id of [...battleCache.keys()]) if (touched(id)) battleCache.delete(id);
  for (const { id } of inflight.values()) {
    if (touched(id)) {
      staleInflight.add(id);
      keepSamples.delete(id);
    } else if (reshaped(id) && !staleInflight.has(id)) {
      staleInflight.add(id);
      keepSamples.add(id);
    }
  }
}

function evict() {
  if (cache.size <= CACHE_MAX) return;
  const pinned = new Set<string>();
  for (const job of pending.values()) job.deps.forEach((d) => pinned.add(d));
  // LRU order (Map insertion order, refreshed by touch), but coarse tiles get a bonus:
  // every finer tile depends on them, so losing one forces a regeneration chain.
  const maxLevel = geom?.max_level ?? 0;
  const order = [...cache.values()]
    .map((t, i) => ({ id: tileId(t.level, t.x, t.y), score: i + (maxLevel - t.level) * LEVEL_KEEP_BONUS }))
    .filter((e) => !pinned.has(e.id))
    .sort((a, b) => a.score - b.score);
  for (const e of order) {
    if (cache.size <= CACHE_MAX) break;
    cache.delete(e.id);
    retex.delete(e.id);
  }
}

setInterval(() => {
  if (!geom) return;
  post({
    type: 'stats',
    stats: {
      pending: pending.size,
      inflight: inflight.size,
      cached: cache.size + battleCache.size,
      jobs: jobsDone,
      avgMs: jobsDone ? msTotal / jobsDone : 0,
    },
  });
}, 250);

self.onmessage = (e: MessageEvent<ToCoordinator>) => {
  const m = e.data;
  if (m.type === 'init') {
    initSession(m).catch((err) => post({ type: 'error', message: String(err) }));
  } else if (m.type === 'want') {
    if (ctx) onWant(m.tiles);
  } else if (m.type === 'edits') {
    onEdits(m);
  } else if (m.type === 'ask') {
    // Questions go to the last worker, so its settlement layouts stay warm between them.
    const gw = workers[workers.length - 1];
    if (gw) gw.w.postMessage({ type: 'ask', id: m.id, ask: m.ask });
    else post({ type: 'answer', id: m.id, json: 'null' });
  }
};
