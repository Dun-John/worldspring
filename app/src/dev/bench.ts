// M0 perf gate: a scripted continent → battlemap → region → continent fly-through that
// records frame times (avg fps, 1% lows) and how long detail takes to load after stops.
// Run with ?bench=1 or the Bench button; results are logged as `[bench] {...}`.
import type { PlayController } from '../play/controller';
import { newId } from '../play/state';
import { flyPath, type CameraState } from '../render/camera';
import type { MapView } from '../render/MapView';
import type { Edits, Placed, Stroke, WorldFile } from '../gen/protocol';
import { GenClient } from '../gen/client';
import { putAsset } from '../world/assets';

export interface BenchResult {
  /** The world's continent as the app generated it (ms; the gate is 15 s). */
  continentMs?: number;
  /** The same world regenerated with a sketch drawn on it, as Generate does: until every generator
   * has the new continent (ms; the gate is 10 s), and its continent alone. */
  regenerate?: { readyMs: number; continentMs: number };
  seconds: number;
  frames: number;
  avgFps: number;
  low1Fps: number;
  p95Ms: number;
  maxMs: number;
  /** Time until every target tile in view was loaded after each stop (ms). */
  detailLatencyMs: number[];
  /** Per stop: missing tiles at the stop and when they were received / uploaded / visible. */
  stops: StopTiming[];
  maxLevel: number;
  /** CPU breakdown (ms) of the slowest 1% of frames: previous frame's tiles/labels/uploads. */
  slowFrames: { tiles: number; labels: number; play: number; uploads: number; render: number; gpu: number; gap: number; seg: number; zoom: number; ev: string; before: string }[];
  /** Play mode's CPU time per frame (ms): mean and worst. */
  play: { avgMs: number; maxMs: number };
  /** Per route segment: frames and frames over 20 ms (missed vsyncs). */
  segments: { seg: string; frames: number; over20: number }[];
  /** Frames over 20 ms by camera zoom (rounded to 0.5), to see which tier stutters. */
  slowByZoom: Record<string, number>;
  /** Mean GPU ms per frame by camera zoom (with ?gpu=1). */
  gpuByZoom: Record<string, number>;
  resolution: number;
  viewport: string;
  gpu: string;
}

export interface StopTiming {
  tiles: number;
  missingAtStop: number;
  receivedMs: number;
  uploadedMs: number;
  visibleMs: number;
}

type Seg = { to: CameraState; ms: number } | { hold: number };

const BATTLEMAP_ZOOM = Math.log2(64 / 5); // 5-ft square = 64 px

/** A sketch as one draws it on a fresh world (fractions of the map): an island's coast, a range,
 * a massif, a river into a lake, a city pin and a road to it. */
function benchSketch(w: number, h: number): Stroke[] {
  const p = (x: number, y: number): [number, number] => [Math.round(x * w), Math.round(y * h)];
  const coast = Array.from({ length: 40 }, (_, k) => {
    const a = (2 * Math.PI * k) / 40;
    return p(0.5 + 0.36 * Math.cos(a) * (1 + 0.1 * Math.sin(3 * a)), 0.5 + 0.34 * Math.sin(a));
  });
  return [
    { tool: 'land', closed: true, pts: coast },
    { tool: 'range', radius_ft: 0.012 * w, strength: 0.8, name: 'Bench Ridge', pts: [p(0.3, 0.33), p(0.45, 0.36), p(0.6, 0.31)] },
    { tool: 'massif', closed: true, radius_ft: 0.007 * w, strength: 0.7, pts: [p(0.25, 0.55), p(0.35, 0.52), p(0.38, 0.62), p(0.28, 0.66)] },
    { tool: 'lake', closed: true, name: 'Bench Water', pts: [p(0.5, 0.52), p(0.53, 0.52), p(0.535, 0.56), p(0.505, 0.565)] },
    { tool: 'river', radius_ft: 0.003 * w, strength: 0.7, pts: [p(0.47, 0.39), p(0.51, 0.53), p(0.55, 0.8)] },
    { tool: 'pin', tier: 'city', name: 'Benchford', pts: [p(0.55, 0.66)] },
    { tool: 'road', kind: 'kings_road', name: 'Bench Way', pts: [p(0.3, 0.5), p(0.42, 0.62), p(0.55, 0.66)] },
  ] as Stroke[];
}

/** `world` regenerated with a sketch drawn on it, in a generator of its own (as Generate would:
 * new workers, the continent made in one and loaded into the rest), timed. */
async function timeRegenerate(world: WorldFile, w: number, h: number): Promise<{ readyMs: number; continentMs: number }> {
  const gen = new GenClient();
  try {
    const t = performance.now();
    const r = await gen.init({ ...world, sketch: { strokes: benchSketch(w, h) } });
    return { readyMs: Math.round(performance.now() - t), continentMs: Math.round(r.t0Ms) };
  } finally {
    gen.dispose();
  }
}

/** `route`: another fly-through than the standard one; `tick`: called every frame of it;
 * `quiet`: a run inside another bench (its result is not the one published). `world` (the
 * standard run): the world shown, regenerated with a sketch after the fly-through, timed. */
export async function runBench(view: MapView, route?: { segs: Seg[]; tick?: (now: number) => void; quiet?: boolean }, world?: WorldFile): Promise<BenchResult> {
  const geom = view.geom!;
  const cam = view.cam;
  await waitReady(view, 4000);

  const focus = findFocus(view);
  const big = view.overlay?.features.find((f) => f.kind === 'metropolis') ?? view.overlay?.features.find((f) => f.kind === 'city');
  const city: [number, number] = big ? [big.x, big.y] : focus;
  const fit = cam.fitZoom(geom.map_w_ft, geom.map_h_ft);
  const region = BATTLEMAP_ZOOM - 10;
  const segs: Seg[] = route?.segs ?? [
    { hold: 800 },
    { to: { cx: focus[0], cy: focus[1], zoom: BATTLEMAP_ZOOM }, ms: 14000 },
    { hold: 2500 },
    { to: { cx: focus[0] + 600, cy: focus[1] + 300, zoom: BATTLEMAP_ZOOM }, ms: 3000 },
    { to: { cx: focus[0] + 600, cy: focus[1] + 300, zoom: region }, ms: 4000 },
    { hold: 2500 },
    { to: { cx: focus[0] + 40 * 5280, cy: focus[1] - 15 * 5280, zoom: region }, ms: 4000 },
    { hold: 2500 },
    { to: { cx: geom.map_w_ft / 2, cy: geom.map_h_ft / 2, zoom: fit }, ms: 3500 },
    { hold: 1500 },
    // The densest battlemaps: down into the biggest city (roofs, walls, street props).
    { to: { cx: city[0], cy: city[1], zoom: BATTLEMAP_ZOOM }, ms: 14000 },
    { hold: 2500 },
    { to: { cx: city[0] + 900, cy: city[1] - 500, zoom: BATTLEMAP_ZOOM }, ms: 4000 },
    { hold: 1500 },
  ];

  const dts: number[] = [];
  const profs: { tiles: number; labels: number; play: number; uploads: number; render: number; gpu: number; seg: number; zoom: number; ev: string }[] = [];
  const latencies: number[] = [];
  const stops: StopTiming[] = [];
  let stop: StopTiming | null = null;
  let maxLevel = 0;

  await new Promise<void>((done) => {
    let i = 0;
    let segStart = -1;
    let from: CameraState = { cx: cam.cx, cy: cam.cy, zoom: cam.zoom };
    let path: ReturnType<typeof flyPath> | null = null;
    let last = -1;
    let latencyDone = false;
    view.driver = (now) => {
      route?.tick?.(now);
      if (last >= 0) {
        dts.push(now - last);
        profs.push({ ...view.prof, seg: i, zoom: +cam.zoom.toFixed(2) });
      }
      last = now;
      maxLevel = Math.max(maxLevel, view.tiles?.stats.level ?? 0);
      if (segStart < 0) {
        segStart = now;
        from = { cx: cam.cx, cy: cam.cy, zoom: cam.zoom };
        const seg = segs[i];
        path = 'to' in seg ? flyPath(from, seg.to, cam.width) : null;
        latencyDone = false;
        if ('hold' in segs[i]) {
          const s = view.tiles!.stats;
          stop = { tiles: s.targetTiles, missingAtStop: s.targetTiles - s.targetReady, receivedMs: -1, uploadedMs: -1, visibleMs: -1 };
          stops.push(stop);
        }
      }
      const seg = segs[i];
      const elapsed = now - segStart;
      let state: CameraState = from;
      if ('to' in seg) {
        const t = Math.min(1, elapsed / seg.ms);
        state = path!.at(t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2);
        if (t >= 1) next();
      } else {
        const s = view.tiles!.stats;
        if (stop && stop.receivedMs < 0 && s.targetReceived >= s.targetTiles) stop.receivedMs = Math.round(elapsed);
        if (stop && stop.uploadedMs < 0 && s.targetUploaded >= s.targetTiles) stop.uploadedMs = Math.round(elapsed);
        if (stop && stop.visibleMs < 0 && s.targetReady >= s.targetTiles) stop.visibleMs = Math.round(elapsed);
        if (!latencyDone && view.readiness >= 1) {
          latencies.push(elapsed);
          latencyDone = true;
        }
        if (elapsed >= seg.hold) {
          if (!latencyDone) latencies.push(elapsed);
          next();
        }
      }
      return state;

      function next() {
        i++;
        segStart = -1;
        if (i >= segs.length) {
          view.driver = null;
          done();
        }
      }
    };
  });

  const sorted = [...dts].sort((a, b) => a - b);
  const total = dts.reduce((a, b) => a + b, 0);
  const worst = sorted.slice(Math.floor(sorted.length * 0.99));
  const worstMean = worst.reduce((a, b) => a + b, 0) / Math.max(1, worst.length);
  const result: BenchResult = {
    seconds: total / 1000,
    frames: dts.length,
    avgFps: (dts.length * 1000) / total,
    low1Fps: 1000 / worstMean,
    p95Ms: sorted[Math.floor(sorted.length * 0.95)] ?? 0,
    maxMs: sorted[sorted.length - 1] ?? 0,
    detailLatencyMs: latencies.slice(1).map(Math.round),
    stops: stops.slice(1),
    maxLevel,
    slowFrames: dts
      .map((gap, i) => ({ gap: Math.round(gap), tiles: +profs[i].tiles.toFixed(1), labels: +profs[i].labels.toFixed(1), play: +(profs[i].play ?? 0).toFixed(1), uploads: +profs[i].uploads.toFixed(1), render: +profs[i].render.toFixed(1), gpu: +(profs[i].gpu ?? 0).toFixed(1), seg: profs[i].seg, zoom: profs[i].zoom, ev: profs[i].ev, before: profs[i - 1]?.ev ?? '' }))
      .sort((a, b) => b.gap - a.gap)
      .slice(0, 12),
    play: { avgMs: +(profs.reduce((a, p) => a + p.play, 0) / Math.max(1, profs.length)).toFixed(2), maxMs: +Math.max(0, ...profs.map((p) => p.play)).toFixed(1) },
    segments: segs.map((sg, k) => {
      const idx = profs.map((p, j) => (p.seg === k ? j : -1)).filter((j) => j >= 0);
      return { seg: 'to' in sg ? `fly to zoom ${sg.to.zoom.toFixed(1)}` : `hold ${sg.hold}`, frames: idx.length, over20: idx.filter((j) => dts[j] > 20).length };
    }),
    slowByZoom: dts.reduce<Record<string, number>>((h, gap, i) => {
      if (gap > 20) {
        const z = (Math.round(profs[i].zoom * 2) / 2).toFixed(1);
        h[z] = (h[z] ?? 0) + 1;
      }
      return h;
    }, {}),
    gpuByZoom: (() => {
      const sum: Record<string, [number, number]> = {};
      for (const p of profs) {
        const z = (Math.round(p.zoom * 2) / 2).toFixed(1);
        const e = (sum[z] ??= [0, 0]);
        e[0] += p.gpu ?? 0;
        e[1]++;
      }
      return Object.fromEntries(Object.entries(sum).map(([z, [t, n]]) => [z, +(t / n).toFixed(1)]));
    })(),
    resolution: view.app.renderer.resolution,
    viewport: `${cam.width}x${cam.height}`,
    gpu: gpuName(view),
  };
  if (route?.quiet) return result;
  if (world) {
    result.continentMs = Math.round(view.t0Ms);
    result.regenerate = await timeRegenerate(world, geom.map_w_ft, geom.map_h_ft);
  }
  console.log('[bench]', JSON.stringify(result));
  (window as unknown as { __benchResult: BenchResult }).__benchResult = result;
  return result;
}

/**
 * M9 perf gate: play mode at a table in the biggest city: 30 tokens (4 that see for the
 * players, 2 lights), fog of war and line of sight on, panning and zooming about the battlemap
 * while one of the 4 walks a loop (sight is worked out again at each step). Run with ?bench=play.
 */
export async function runPlayBench(view: MapView, play: PlayController): Promise<BenchResult> {
  await waitReady(view, 4000);
  const big = view.overlay?.features.find((f) => f.kind === 'metropolis') ?? view.overlay?.features.find((f) => f.kind === 'city');
  const [cx, cy] = big ? [big.x, big.y] : findFocus(view);
  view.cam.set({ cx, cy, zoom: BATTLEMAP_ZOOM });
  // Let the battlemap there load.
  const start = performance.now();
  while (performance.now() - start < 20000 && (!view.battle || view.battle.stats.chunks < 4 || view.battle.pending || view.readiness < 1)) {
    await new Promise((r) => setTimeout(r, 200));
  }
  const [gx, gy] = [Math.floor(cx / 5), Math.floor(cy / 5)];
  play.setSettingsHere({ fog: true });
  play.dispatch({ t: 'settings', los: true });
  const pcs: string[] = [];
  for (let k = 0; k < 30; k++) {
    const id = newId();
    const light = k >= 4 && k < 6;
    if (k < 4) pcs.push(id);
    play.dispatch({
      t: 'token',
      token: { id, loc: 'surface', x: gx - 10 + (k % 6) * 4 + 0.5, y: gy - 8 + Math.floor(k / 6) * 4 + 0.5, name: `Token ${k + 1}`, kind: light ? 'light' : 'character', size: k % 7 === 6 ? 2 : 1, color: 0x8b2e2e + k * 0x040404, vision: k < 4, light: light ? 4 : undefined },
    });
  }
  play.dispatch({ t: 'fog', loc: 'surface', reveal: true, pts: [gx, gy], r: 14 });
  let lastStep = 0;
  let step = 0;
  const tick = (now: number) => {
    if (now - lastStep < 400) return;
    lastStep = now;
    const t = play.state.tokens[pcs[0]];
    if (!t) return;
    step++;
    const a = (step / 24) * Math.PI * 2;
    play.updateToken(t.id, { x: gx + 0.5 + Math.round(Math.cos(a) * 8), y: gy + 0.5 + Math.round(Math.sin(a) * 8) });
  };
  const z = BATTLEMAP_ZOOM;
  return runBench(view, {
    tick,
    segs: [
      { hold: 1500 },
      { to: { cx: cx + 250, cy, zoom: z }, ms: 4000 },
      { to: { cx: cx + 250, cy: cy + 160, zoom: z - 0.8 }, ms: 3000 },
      { to: { cx, cy, zoom: z - 2 }, ms: 3000 },
      { hold: 1500 },
      { to: { cx, cy, zoom: z + 0.5 }, ms: 3000 },
      { to: { cx: cx - 200, cy: cy - 100, zoom: z }, ms: 3000 },
      { hold: 1500 },
    ],
  });
}

/**
 * The sewers of the biggest city as one network: down a grate, then panning across several
 * sections and zooming out and in while they load round the view. Run with ?bench=sewer.
 */
export async function runSewerBench(view: MapView): Promise<BenchResult> {
  await waitReady(view, 4000);
  const big = view.overlay?.features.find((f) => f.kind === 'metropolis') ?? view.overlay?.features.find((f) => f.kind === 'city');
  const [cx, cy] = big ? [big.x, big.y] : findFocus(view);
  const index = view.overlay!.features.filter((f) => ['metropolis', 'city', 'town', 'village'].includes(f.kind)).findIndex((f) => f === big);
  // A section with sewers near the middle of the city.
  let entered = false;
  for (let r = 0; r < 4 && !entered; r++) {
    for (let dy = -r; dy <= r && !entered; dy++) {
      for (let dx = -r; dx <= r && !entered; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
        entered = await view.enterBuilding(`w:${index}:${Math.floor(cx / 480) + dx}:${Math.floor(cy / 480) + dy}`);
      }
    }
  }
  if (!entered) throw new Error('no sewers found');
  const it = view.interior!.interior;
  const [x0, y0] = [it.origin[0] + 240, it.origin[1] + 240];
  const z = 1.6;
  view.cam.set({ cx: x0, cy: y0, zoom: z });
  // Let the sections round the view load.
  const start = performance.now();
  while (performance.now() - start < 15000 && (view.sewers?.loaded ?? 0) < 6) await new Promise((r) => setTimeout(r, 200));
  await new Promise((r) => setTimeout(r, 2000));
  return runBench(view, {
    segs: [
      { hold: 1000 },
      { to: { cx: x0 + 1440, cy: y0, zoom: z }, ms: 6000 },
      { to: { cx: x0 + 1440, cy: y0 + 960, zoom: z }, ms: 4000 },
      { to: { cx: x0 + 720, cy: y0 + 480, zoom: view.cam.minZoom }, ms: 3000 },
      { hold: 1000 },
      { to: { cx: x0, cy: y0, zoom: z + 1.5 }, ms: 4000 },
      { hold: 1000 },
    ],
  });
}

/**
 * The biggest built site underground (a ruin's dungeon, crypt or catacombs: square corners,
 * walls measured from the squares in the shader): panning its deepest level, zooming out to the
 * whole level and in again, changing level every few seconds. Run with ?bench=dungeon.
 */
export async function runDungeonBench(view: MapView): Promise<BenchResult> {
  await waitReady(view, 4000);
  const features = view.overlay!.features;
  const towns = features.filter((f) => ['metropolis', 'city', 'town', 'village'].includes(f.kind)).length;
  const sites = features.filter((f) => ['ruin', 'tower', 'waystation', 'camp', 'cave', 'mine', 'lava_tube'].includes(f.kind) && !f.id.startsWith('c:'));
  let best: { id: string; squares: number } | null = null;
  for (let k = 0; k < sites.length; k++) {
    if (sites[k].kind !== 'ruin') continue;
    const id = `u:${towns + k}:0`;
    const it = await view.gen.interior(id);
    if (it && ['dungeon', 'crypt', 'catacombs'].includes(it.function) && (!best || it.nx * it.ny * it.levels.length > best.squares)) best = { id, squares: it.nx * it.ny * it.levels.length };
  }
  if (!best || !(await view.enterBuilding(best.id))) throw new Error('no dungeon found');
  const layer = view.interior!;
  const it = layer.interior;
  view.setInteriorLevel(0);
  const at = (i: number, j: number) => layer.toWorld(i, j);
  const [c, a, b] = [at(it.nx / 2, it.ny / 2), at(it.nx * 0.2, it.ny * 0.25), at(it.nx * 0.8, it.ny * 0.75)];
  const z = 1.6;
  view.cam.set({ cx: a[0], cy: a[1], zoom: z });
  await new Promise((r) => setTimeout(r, 2000));
  let last = 0;
  const tick = (now: number) => {
    if (now - last < 3000) return;
    last = now;
    view.setInteriorLevel((layer.currentLevel + 1) % it.levels.length);
  };
  console.log('[bench] dungeon', best.id, it.function, it.theme ?? '', `${it.nx}x${it.ny}`, `${it.levels.length} levels`);
  return runBench(view, {
    tick,
    segs: [
      { hold: 1000 },
      { to: { cx: b[0], cy: b[1], zoom: z }, ms: 6000 },
      { to: { cx: c[0], cy: c[1], zoom: z - 1.6 }, ms: 3000 },
      { hold: 1500 },
      { to: { cx: a[0], cy: a[1], zoom: z + 1.2 }, ms: 4000 },
      { to: { cx: c[0], cy: c[1], zoom: z }, ms: 4000 },
      { hold: 1000 },
    ],
  });
}

/**
 * Editing a battlemap while moving over it: at battlemap zoom over the high ground, every 1.5 s
 * a batch of objects is put down ahead of the camera (trees, rocks and an uploaded sprite), or
 * an area is cleared, so the chunks there are made again and swapped in while panning. The
 * world's edits are put back afterwards. Run with ?bench=edit.
 */
export async function runEditBench(view: MapView, edits: () => Edits, apply: (e: Edits) => void): Promise<BenchResult> {
  await waitReady(view, 4000);
  const before = structuredClone(edits());
  // A sprite drawn here, uploaded as the user would.
  const c = document.createElement('canvas');
  c.width = c.height = 128;
  const g = c.getContext('2d')!;
  g.fillStyle = '#7a5a3a';
  g.beginPath();
  g.arc(64, 64, 56, 0, Math.PI * 2);
  g.fill();
  g.fillStyle = '#d8c48a';
  g.fillRect(40, 40, 48, 48);
  const blob = await new Promise<Blob>((r) => c.toBlob((b) => r(b!), 'image/png'));
  const asset = await putAsset(blob, 'bench crate');
  apply({ ...edits(), sprites: { ...(edits().sprites ?? {}), [asset]: { name: 'Bench crate', size: 2, cover: 2, blocks_move: true, blocks_sight: false, difficult: false, height_ft: 6 } } });
  const [fx, fy] = findFocus(view);
  const z = BATTLEMAP_ZOOM - 1;
  view.cam.set({ cx: fx, cy: fy, zoom: z });
  await new Promise((r) => setTimeout(r, 3000));
  let last = 0;
  let n = 0;
  const tick = (now: number) => {
    if (now - last < 1500) return;
    last = now;
    n++;
    const [cx, cy] = [view.cam.cx + 60, view.cam.cy];
    const e = structuredClone(edits());
    if (n % 3 === 0) {
      e.cleared = { ...(e.cleared ?? {}), [`x:bench${n}`]: { x: cx, y: cy, r: 30 } };
    } else {
      const objects = { ...(e.objects ?? {}) };
      for (let k = 0; k < 40; k++) {
        const a = k * 2.399;
        const r = 6 * Math.sqrt(k + 1);
        const kind: Placed['kind'] = k % 4 === 0 ? `s:${asset}` : [1, 2, 8, 12, 14][k % 5];
        objects[`o:bench${n}_${k}`] = { kind, x: cx + r * Math.cos(a), y: cy + r * Math.sin(a), rot: a, scale: 1, variant: k & 7 };
      }
      e.objects = objects;
    }
    apply(e);
  };
  try {
    return await runBench(view, {
      tick,
      segs: [
        { hold: 1000 },
        { to: { cx: fx + 900, cy: fy, zoom: z }, ms: 7000 },
        { to: { cx: fx + 900, cy: fy + 600, zoom: z + 0.5 }, ms: 4000 },
        { to: { cx: fx, cy: fy, zoom: z - 0.5 }, ms: 5000 },
        { hold: 1500 },
      ],
    });
  } finally {
    apply(before);
  }
}

async function waitReady(view: MapView, timeoutMs: number) {
  const start = performance.now();
  while (view.readiness < 1 && performance.now() - start < timeoutMs) {
    await new Promise((r) => setTimeout(r, 100));
  }
}

/** Highest loaded point in the central part of the map: mountains stress detail the most. */
function findFocus(view: MapView): [number, number] {
  const g = view.geom!;
  let best: [number, number] = [g.map_w_ft / 2, g.map_h_ft / 2];
  let bestH = -Infinity;
  for (let j = 0; j <= 30; j++) {
    for (let i = 0; i <= 40; i++) {
      const x = g.map_w_ft * (0.2 + (0.6 * i) / 40);
      const y = g.map_h_ft * (0.2 + (0.6 * j) / 30);
      const h = view.tiles?.heightAt(x, y) ?? -Infinity;
      if (h > bestH) [bestH, best] = [h, [x, y]];
    }
  }
  return best;
}

function gpuName(view: MapView): string {
  const r = view.app.renderer as unknown as { gl?: WebGL2RenderingContext };
  const gl = r.gl;
  if (!gl) return 'unknown';
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  return ext ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : String(gl.getParameter(gl.RENDERER));
}
