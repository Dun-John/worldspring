// Coordinator worker: resolves the job dependency DAG (declared in Rust, via WASM `deps`),
// keeps an LRU of results, prioritizes what the view wants and feeds generator workers.
// The main thread only ever receives render-ready tiles.
import type { PreparedChunk } from './battlePrep';
import init, { Ctx, battlemap_catalog_json } from './pkg/worldgen_wasm.js';
import { buildRiverGeometry, buildSiteGeometry } from './tileGeometry';
import {
  KIND_BATTLEMAP,
  KIND_TERRAIN,
  battleId,
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
  type Ask,
  SITE_MIN_LEVEL,
} from './protocol';

const CACHE_MAX = 200;
const BATTLE_CACHE_MAX = 24;
/** In LRU positions: each level of coarseness counts as this many uses more recent. */
const LEVEL_KEEP_BONUS = 12;
/** A job that only builds a settlement's layout into a worker's memo (`warmtown`), ahead of its tiles. */
const KIND_WARM = 3;
/** Warm jobs come after every tile (`pri` is below this for all of them). */
const WARM_PRI = 1e9;
/** A second (third) worker may build a town its owners have built once the ready jobs waiting on
 * it would keep them busy this many times as long as a build of it took (both as measured here):
 * a city's cold build costs seconds, a tile over a built one a small part of that. */
const SPILL_BUILDS = 2;
const MAX_OWNERS = 3;

interface Job {
  kind: number;
  level: number;
  x: number;
  y: number;
  pri: number;
  deps: string[];
  /** The settlements whose layouts it reads (by `townBounds`), for the worker it goes to. */
  towns: number[];
}

interface GenWorker {
  w: Worker;
  busy: boolean;
  /** Its place in the debug count (`builds`). */
  no: number;
}

let ctx: Ctx | null = null;
let geom: Geom | null = null;
let workers: GenWorker[] = [];
/** Workers still loading T0 (they join `workers` as each is done). */
let joining: GenWorker[] = [];
let session = 0;
let nextJobId = 1;

const cache = new Map<string, TerrainPayload>();
/** Battlemap payloads by battleId (LRU by insertion order). */
const battleCache = new Map<string, PreparedChunk>();
let wanted = new Map<string, WantTile>();
const pending = new Map<string, Job>();
const inflight = new Map<number, { id: string; worker: GenWorker; towns: number[] }>();
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
/** The overlay's settlements and sites, for the workers that load T0 without the overlay. */
let placesJson = '[]';
let epoch = 0;
// Town affinity. Each worker keeps the layouts it built (a thread-local memo), and a metropolis
// takes seconds: every tile over a town goes to a worker that has it (or is building it), so
// one zoom builds it once, not once per cold worker.
/** Each settlement's bounds, flat (x0, y0, x1, y1), as `town::layouts_near` tests them. */
let townBounds: Float64Array = new Float64Array(0);
/** The settlements a tile's (terrain id) layouts read, worked out once per bounds. */
const townsOf = new Map<string, number[]>();
/** The workers each settlement went to (they have its layout, or are building it). */
const owners = new Map<number, Set<GenWorker>>();
/** Of those, the ones that finished a job with it (its layout made there). */
const madeBy = new Map<number, Set<GenWorker>>();
/** Debug: generated layouts built, by settlement, per worker. */
const builds = new Map<number, number[]>();
/** How long a job that built a settlement's layout took (the longest), and the jobs over it
 * that found it made ([total ms, count]), for `canTake`'s spill. */
const buildMs = new Map<number, number>();
const tileMs = new Map<number, [number, number]>();
// Stand-ins: a town's plan alone (`standin` asks), drawn while its layout is built. Each goes to
// a free worker that does not own the town (its owner is busy building it), before any tile or
// warm job, and is kept for this edit generation.
/** Plans answered (JSON), by settlement, and the epoch they were asked in. */
const standins = new Map<number, string>();
let standinEpoch = 0;
/** Asks waiting for a free worker, and those being answered (by ask id). */
const standinWait: { id: number; town: number }[] = [];
const standinRunning = new Map<number, { gw: GenWorker; town: number; epoch: number }>();
let workerNo = 0;
let jobsDone = 0;
let msTotal = 0;

const post = (msg: FromCoordinator, transfer: Transferable[] = []) =>
  (self as unknown as Worker).postMessage(msg, transfer);

function spawnWorker(): GenWorker {
  const w = new Worker(new URL('./gen.worker.ts', import.meta.url), { type: 'module' });
  return { w, busy: false, no: workerNo++ };
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
  [...workers, ...joining].forEach((gw) => gw.w.terminate());
  workers = [];
  joining = [];
  cache.clear();
  battleCache.clear();
  pending.clear();
  inflight.clear();
  inflightIds.clear();
  staleInflight.clear();
  retex.clear();
  keepSamples.clear();
  editFields = {};
  placesJson = '[]';
  standins.clear();
  standinWait.length = 0;
  standinRunning.clear();
  wanted = new Map();
  townBounds = new Float64Array(0);
  townsOf.clear();
  owners.clear();
  madeBy.clear();
  builds.clear();
  buildMs.clear();
  tileMs.clear();
  workerNo = 0;

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
  // (Listed from now: a world started meanwhile stops them all, the one making T0 too.)
  joining = [first, ...rest];
  const firstReply = await request(first, { type: 'init', worldJson, t0: null });
  if (mySession !== session) {
    first.w.terminate();
    return;
  }
  if (firstReply.type !== 'inited' || !firstReply.t0) throw new Error(firstReply.type === 'error' ? firstReply.message : 'T0 failed');
  // The map is ready once the worker that made T0 has it: it starts on the first tiles while
  // the rest load T0 (each its own copy, handed over, not copied again on the way) and join in
  // as each is done (a few hundred ms in a worker that has only just compiled the WASM).
  // (The names of this world's places first: each worker gets them as it joins, before any ask.)
  const feats = (JSON.parse(firstReply.overlay ?? '{"features":[]}') as { features: { id: string; kind: string; name: string }[] }).features;
  placesJson = JSON.stringify(feats.map((f) => ({ id: f.id, kind: f.kind, name: f.name })));
  townBounds = firstReply.bounds ?? new Float64Array(0);
  join(first);
  const t0 = firstReply.t0;
  rest.forEach((gw, k) => {
    const bytes = k === rest.length - 1 ? t0 : t0.slice(0);
    void request(gw, { type: 'init', worldJson, t0: bytes }, [bytes]).then((m) => {
      if (mySession !== session) return;
      if (m.type === 'inited') join(gw);
      else {
        joining = joining.filter((j) => j !== gw);
        gw.w.terminate();
      }
    });
  });
  post({ type: 'ready', geom, t0Ms: firstReply.ms, overlay: firstReply.overlay ?? '{"features":[]}', catalog: battlemap_catalog_json() });
}

/** A worker with T0 loaded takes jobs: first the edits as they are now (those made while it was
 * loading included), then whatever is waiting. */
function join(gw: GenWorker) {
  joining = joining.filter((j) => j !== gw);
  gw.w.onmessage = (e: MessageEvent<FromGen>) => onGenMessage(gw, e.data);
  gw.w.postMessage({ type: 'places', json: placesJson } satisfies ToGen);
  if (Object.keys(editFields).length) gw.w.postMessage({ type: 'edits', json: JSON.stringify(editFields), patch: null } satisfies ToGen);
  workers.push(gw);
  pump();
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
  pending.set(id, { kind, level, x, y, pri, deps, towns: jobTowns(kind, id, deps) });
  for (const d of deps) {
    const [l, dx, dy] = d.split('/').map(Number);
    schedule(KIND_TERRAIN, l, dx, dy, pri - 0.5, true);
  }
}

/** The settlements whose layouts a terrain tile reads (none above site levels). */
function tileTowns(id: string): number[] {
  const known = townsOf.get(id);
  if (known) return known;
  const [l, x, y] = id.split('/').map(Number);
  const out: number[] = [];
  if (l >= SITE_MIN_LEVEL && geom) {
    const ts = geom.domain_ft / 2 ** l;
    out.push(...biggestFirst(townsIn([x * ts, y * ts, (x + 1) * ts, (y + 1) * ts])));
  }
  townsOf.set(id, out);
  return out;
}

/** A job's settlements (a battlemap reads the layouts over its terrain tile). */
const jobTowns = (kind: number, id: string, deps: string[]): number[] => (kind === KIND_BATTLEMAP ? (deps.length ? tileTowns(deps[0]) : []) : tileTowns(id));

/** Settlements biggest first (by bounds): a job goes by the first. */
function biggestFirst(towns: number[]): number[] {
  const b = townBounds;
  const area = (i: number) => (b[i * 4 + 2] - b[i * 4]) * (b[i * 4 + 3] - b[i * 4 + 1]);
  return towns.sort((p, q) => area(q) - area(p) || p - q);
}

/** The settlement a question reads the layout of, if it is about a place (biggest there). */
function askTown(a: Ask): number | undefined {
  if (a.op === 'districts') return a.settlement < townBounds.length / 4 ? a.settlement : undefined;
  const r: Rect | null =
    a.op === 'inview' ? a.rect : a.op === 'query' ? [a.x, a.y, a.x, a.y] : a.op === 'pick' ? [a.x - a.radius, a.y - a.radius, a.x + a.radius, a.y + a.radius] : a.op === 'fencesnear' ? [a.x - a.r, a.y - a.r, a.x + a.r, a.y + a.r] : null;
  return r ? biggestFirst(townsIn(r))[0] : undefined;
}

function townsIn(r: Rect): number[] {
  const out: number[] = [];
  const b = townBounds;
  for (let i = 0; i < b.length; i += 4) if (b[i + 2] >= r[0] && b[i] <= r[2] && b[i + 3] >= r[1] && b[i + 1] <= r[3]) out.push(i / 4);
  return out;
}

/** Zooming from region to local (the two levels above site levels): the settlements in view or
 * within about a screen of it get built ahead (`warmtown`), biggest and nearest first, each by
 * the worker that is to own it, after every tile. */
function scheduleWarm(tiles: WantTile[]) {
  let finest = -1;
  for (const t of tiles) if (t.kind !== 'battlemap') finest = Math.max(finest, t.level);
  if (!geom || finest < SITE_MIN_LEVEL - 2 || finest >= SITE_MIN_LEVEL) return;
  const ts = geom.domain_ft / 2 ** finest;
  const r: Rect = [Infinity, Infinity, -Infinity, -Infinity];
  for (const t of tiles) {
    if (t.kind === 'battlemap' || t.level !== finest) continue;
    r[0] = Math.min(r[0], t.x * ts);
    r[1] = Math.min(r[1], t.y * ts);
    r[2] = Math.max(r[2], (t.x + 1) * ts);
    r[3] = Math.max(r[3], (t.y + 1) * ts);
  }
  const [cx, cy, hw, hh] = [(r[0] + r[2]) / 2, (r[1] + r[3]) / 2, r[2] - r[0], r[3] - r[1]];
  for (const i of townsIn([cx - hw, cy - hh, cx + hw, cy + hh])) {
    const id = `w/${i}`;
    if (owners.get(i)?.size || inflightIds.has(id)) continue;
    const b = townBounds.subarray(i * 4, i * 4 + 4);
    const reach = Math.max(1, (b[2] - b[0]) / 2);
    const d = Math.hypot((b[0] + b[2]) / 2 - cx, (b[1] + b[3]) / 2 - cy);
    pending.set(id, { kind: KIND_WARM, level: 0, x: i, y: 0, pri: WARM_PRI + d / reach, deps: [], towns: [i] });
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
  scheduleWarm(tiles);
  pump();
}

const ownersOf = (t: number) => owners.get(t);

/** Whether `gw` may take `job` (town affinity, by the biggest settlement it reads: the others
 * cost little to build again): that town is unowned or owned by `gw`, or it has a long backlog
 * (`SPILL_BUILDS`) and an owner that has built it (never a second cold build while the first is
 * still going). */
function canTake(gw: GenWorker, job: Job, backlog: Map<number, number>): boolean {
  const t = job.towns[0];
  const o = t === undefined ? undefined : ownersOf(t);
  if (!o?.size || o.has(gw)) return true;
  const build = buildMs.get(t);
  const tile = tileMs.get(t);
  if (o.size >= MAX_OWNERS || !madeBy.get(t)?.size || build === undefined || !tile) return false;
  return ((backlog.get(t) ?? 0) * (tile[0] / tile[1])) / o.size > SPILL_BUILDS * build;
}

/** Warm jobs running at once: they leave two workers (one of two) to the tiles in view. */
const warmCap = () => (workers.length >= 3 ? workers.length - 2 : workers.length - 1);

function claim(gw: GenWorker, t: number | undefined) {
  if (t !== undefined) {
    let o = owners.get(t);
    if (!o) owners.set(t, (o = new Set()));
    o.add(gw);
  }
}

/** Stand-in asks to free workers, a town's non-owners first; one per town at a time. */
function pumpStandins() {
  for (let k = 0; k < standinWait.length; ) {
    const { id, town } = standinWait[k];
    if ([...standinRunning.values()].some((r) => r.town === town)) {
      k++;
      continue;
    }
    const free = workers.filter((w) => !w.busy);
    const gw = free.find((w) => !owners.get(town)?.has(w)) ?? free[0];
    if (!gw) return;
    standinWait.splice(k, 1);
    gw.busy = true;
    standinRunning.set(id, { gw, town, epoch });
    gw.w.postMessage({ type: 'ask', id, ask: { op: 'standin', town } } satisfies ToGen);
  }
}

/** A stand-in answered: kept (if no edit came since it was asked), and sent to every ask for it. */
function onStandin(gw: GenWorker, id: number, json: string) {
  const r = standinRunning.get(id)!;
  standinRunning.delete(id);
  gw.busy = false;
  if (r.epoch === epoch) standins.set(r.town, json);
  post({ type: 'answer', id, json });
  for (let k = standinWait.length - 1; k >= 0; k--) {
    if (standinWait[k].town === r.town && r.epoch === epoch) {
      post({ type: 'answer', id: standinWait[k].id, json });
      standinWait.splice(k, 1);
    }
  }
}

function pump() {
  pumpStandins();
  const ready: [string, Job][] = [];
  for (const entry of pending) if (entry[1].deps.every((d) => cache.has(d))) ready.push(entry);
  if (!ready.length) return;
  const backlog = new Map<number, number>();
  for (const [, { towns }] of ready) if (towns.length) backlog.set(towns[0], (backlog.get(towns[0]) ?? 0) + 1);
  let warming = 0;
  for (const { id } of inflight.values()) if (id.startsWith('w/')) warming++;
  const taken = new Set<string>();
  for (const gw of workers) {
    if (gw.busy) continue;
    let best: [string, Job] | null = null;
    for (const entry of ready) {
      const job = entry[1];
      if (taken.has(entry[0]) || (best && job.pri >= best[1].pri)) continue;
      if (job.kind === KIND_WARM ? warming >= warmCap() || !!ownersOf(job.x)?.size : !canTake(gw, job, backlog)) continue;
      best = entry;
    }
    if (!best) continue;
    const [id, job] = best;
    taken.add(id);
    pending.delete(id);
    if (job.towns.length) backlog.set(job.towns[0], (backlog.get(job.towns[0]) ?? 1) - 1);
    claim(gw, job.towns[0]);
    const jobNo = nextJobId++;
    gw.busy = true;
    inflight.set(jobNo, { id, worker: gw, towns: job.towns });
    inflightIds.add(id);
    if (job.kind === KIND_WARM) {
      warming++;
      gw.w.postMessage({ type: 'warmtown', id: jobNo, town: job.x } satisfies ToGen);
      continue;
    }
    const parent = job.deps.length ? cache.get(job.deps[0])!.padded.slice() : null;
    const msg: ToGen = { type: 'job', id: jobNo, kind: job.kind, level: job.level, x: job.x, y: job.y, parent };
    gw.w.postMessage(msg, parent ? [parent.buffer] : []);
  }
}

/** Debug count of the layouts a worker reports built. */
function countBuilt(gw: GenWorker, built: number[] | undefined) {
  const n = townBounds.length / 4;
  for (const t of built ?? []) {
    if (t >= n) continue;
    let c = builds.get(t);
    if (!c) builds.set(t, (c = []));
    c[gw.no] = (c[gw.no] ?? 0) + 1;
  }
}

function onGenMessage(gw: GenWorker, m: FromGen) {
  if (m.type === 'inited' || m.type === 'progress') return;
  if (m.type === 'bounds') {
    townBounds = m.bounds;
    townsOf.clear();
    for (const [id, job] of pending) if (job.kind !== KIND_WARM) job.towns = jobTowns(job.kind, id, job.deps);
    return;
  }
  countBuilt(gw, m.built);
  if (m.type === 'answer') {
    if (standinRunning.has(m.id)) {
      onStandin(gw, m.id, m.json);
      pump();
      return;
    }
    post({ type: 'answer', id: m.id, json: m.json });
    return;
  }
  gw.busy = false;
  const job = m.id !== undefined ? inflight.get(m.id) : undefined;
  if (m.id !== undefined) inflight.delete(m.id);
  if (job) inflightIds.delete(job.id);
  if (job && m.type === 'done') {
    const key = job.towns[0];
    if (m.built?.includes(key)) buildMs.set(key, Math.max(buildMs.get(key) ?? 0, m.ms));
    else if (key !== undefined && !job.id.startsWith('w/')) {
      const a = tileMs.get(key) ?? [0, 0];
      tileMs.set(key, [a[0] + m.ms, a[1] + 1]);
    }
    for (const t of job.towns) {
      let o = madeBy.get(t);
      if (!o) madeBy.set(t, (o = new Set()));
      o.add(gw);
    }
  }
  if (job?.id.startsWith('w/')) {
    if (m.type === 'error') console.warn(`[gen] warming ${job.id} failed: ${m.message}`);
  } else if (m.type === 'error') {
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
  const sites = buildSiteGeometry(r.sitePolys, r.siteLines, r.siteLooks, geom ? geom.domain_ft / 2 ** r.level : 0);
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
  if (standinEpoch !== epoch) {
    standins.clear();
    standinEpoch = epoch;
  }
  const json = Object.keys(m.fields).length ? fieldsJson(m.fields) : null;
  const patch = Object.keys(m.patches).length ? JSON.stringify(m.patches) : null;
  if (json || patch) for (const gw of workers) gw.w.postMessage({ type: 'edits', json, patch } satisfies ToGen);
  // (A town laid out by hand or given streets reaches elsewhere: the tiles it reads go by its bounds.)
  if ((json || patch) && workers.length) workers[0].w.postMessage({ type: 'bounds' } satisfies ToGen);
  const battleRects = m.battleRects ?? [];
  const groundRects = m.groundRects ?? [];
  const allRects = m.allRects ?? [];
  if (!geom || (!m.rects.length && !battleRects.length && !groundRects.length && !allRects.length)) return;
  const touched = (id: string) => {
    const battle = id.startsWith('b/');
    const [l, x, y] = (battle ? id.slice(2) : id).split('/').map(Number);
    const ts = geom!.domain_ft / 2 ** l;
    const hit = (r: Rect) => (x + 1) * ts > r[0] && x * ts < r[2] && (y + 1) * ts > r[1] && y * ts < r[3];
    if (!battle && allRects.some(hit)) return true;
    if (!battle && l < SITE_MIN_LEVEL) return false;
    return m.rects.some(hit) || (battle && (battleRects.some(hit) || allRects.some(hit)));
  };
  // Ground shaped by hand: the three finest levels' tiles over it are drawn from it (a big hill:
  // coarser ones too, `groundLevels`).
  const reshaped = (id: string) => {
    if (id.startsWith('b/') || !groundRects.length) return false;
    const [l, x, y] = id.split('/').map(Number);
    const ts = geom!.domain_ft / 2 ** l;
    const fine = geom!.max_level - 2;
    return groundRects.some((r, i) => l >= (m.groundLevels?.[i] ?? fine) && (x + 1) * ts > r[0] && x * ts < r[2] && (y + 1) * ts > r[1] && y * ts < r[3]);
  };
  for (const id of [...cache.keys()]) {
    if (touched(id)) {
      cache.delete(id);
      retex.delete(id);
    } else if (reshaped(id)) retex.add(id);
  }
  for (const id of [...battleCache.keys()]) if (touched(id)) battleCache.delete(id);
  for (const { id } of inflight.values()) {
    if (id.startsWith('w/')) continue;
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
      builds: Object.fromEntries(builds),
      owners: Object.fromEntries([...owners].map(([t, o]) => [t, [...o].map((w) => w.no)])),
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
  } else if (m.type === 'ask' && m.ask.op === 'standin') {
    const kept = standins.get(m.ask.town);
    if (kept !== undefined) post({ type: 'answer', id: m.id, json: kept });
    else {
      standinWait.push({ id: m.id, town: m.ask.town });
      pump();
    }
  } else if (m.type === 'ask') {
    // Questions go to the last worker, so its settlement layouts stay warm between them; a
    // town's districts to a worker that has its layout (or is to build it: then it owns it).
    // (So with the other questions about a place in a town: what is here, what is in view.)
    let gw = workers[workers.length - 1];
    const t = gw ? askTown(m.ask) : undefined;
    if (t !== undefined) {
      const made = [...(madeBy.get(t) ?? [])];
      gw = made.find((w) => !w.busy) ?? made[0] ?? [...(owners.get(t) ?? [])][0] ?? gw;
      claim(gw, t);
    }
    if (gw) gw.w.postMessage({ type: 'ask', id: m.id, ask: m.ask });
    else post({ type: 'answer', id: m.id, json: 'null' });
  }
};
