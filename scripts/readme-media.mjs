// Dev tool: the README's pictures (docs/media/), from the running dev server: one zoom from the
// whole continent down to an inn's battlemap and inside it (zoom.gif, needs ffmpeg), and stills of
// the continent, a city, a street battlemap, the inn's interior and underground sites (a dungeon,
// a cave, a mine, a lava tube or a crypt: the biggest of each).
// Usage: node scripts/readme-media.mjs [url]   (default http://localhost:5173/?seed=1)
// Each frame is taken once the map has drawn everything in view, so the GIF is smooth however
// slow the generation is.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const url = process.argv[2] ?? 'http://localhost:5173/?seed=1';
const out = join(import.meta.dirname, '../docs/media');
// The frames stay in target/ (to encode them again without recording: the ffmpeg line at the end).
const frames = join(import.meta.dirname, '../target/readme-frames');
rmSync(frames, { recursive: true, force: true });
mkdirSync(frames, { recursive: true });
mkdirSync(out, { recursive: true });

const [W, H] = [1280, 800];
const port = 9335;
const chromePath = [process.env.CHROME_PATH, 'C:/Program Files/Google/Chrome/Application/chrome.exe', '/usr/bin/google-chrome'].filter(Boolean).find((p) => existsSync(p));
const profile = mkdtempSync(join(tmpdir(), 'ws-media-'));
const chrome = spawn(chromePath, [
  `--user-data-dir=${profile}`,
  `--remote-debugging-port=${port}`,
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding',
  '--disable-background-timer-throttling',
  '--window-position=0,0',
  `--window-size=${W},${H + 140}`,
  url,
]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let ws;
let nextId = 1;
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = nextId++;
    const onMsg = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id !== id) return;
      ws.removeEventListener('message', onMsg);
      resolve(msg.result);
    };
    ws.addEventListener('message', onMsg);
    ws.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }))?.result?.value;

/** Wait until everything in view is drawn (or `ms` pass). */
async function settle(ms = 10000) {
  const start = Date.now();
  // (The map asks for what the new view needs on its next frames.)
  await evaluate('new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))');
  while (Date.now() - start < ms) {
    // (Everything in view drawn; uploads of tiles round the view may linger.)
    if (await evaluate('window.__map.readiness >= 1 && !(window.__map.battle && window.__map.battle.pending)')) break;
    await sleep(120);
  }
  if (process.env.MEDIA_DEBUG) console.log(` settle ${Date.now() - start} ms`, await evaluate('JSON.stringify({ r: window.__map.readiness, s: window.__map.tiles.stats, bp: window.__map.battle && window.__map.battle.pending })'));
  await sleep(150);
}
/** Wait until the camera stops moving (a flight to a site going in), up to `ms`. */
async function still(ms = 20000) {
  const start = Date.now();
  let last = '';
  while (Date.now() - start < ms) {
    await sleep(500);
    const now = await evaluate('JSON.stringify([window.__map.cam.cx, window.__map.cam.cy, window.__map.cam.zoom].map((v) => v.toFixed(2)))');
    if (now === last) break;
    last = now;
  }
}
async function shot(file) {
  const s = await send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width: W, height: H, scale: 1 } });
  writeFileSync(file, Buffer.from(s.data, 'base64'));
}

try {
  for (let i = 0; i < 60 && !ws; i++) {
    try {
      const page = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page' && t.url.startsWith('http'));
      if (page) ws = new WebSocket(page.webSocketDebuggerUrl);
    } catch {}
    if (!ws) await sleep(500);
  }
  await new Promise((r) => ws.addEventListener('open', r, { once: true }));
  await send('Emulation.setDeviceMetricsOverride', { width: W, height: H, deviceScaleFactor: 1, mobile: false });
  for (let i = 0; i < 240 && !(await evaluate('!!(window.__map && window.__map.tiles && window.__map.readiness >= 1)')); i++) await sleep(500);

  // The inn (else tavern, else any building) nearest the middle of the biggest city.
  const target = await evaluate(`(async () => {
    const m = window.__map, f = m.overlay.features;
    const city = f.find((x) => x.kind === 'metropolis') ?? f.find((x) => x.kind === 'city');
    const r = 1500, rect = [city.x - r, city.y - r, city.x + r, city.y + r];
    for (const q of ['inn', 'tavern', '']) {
      const hits = (await m.gen.searchBuildings(q, rect)).filter((h) => h.kind === 'building');
      if (hits.length) {
        hits.sort((a, b) => Math.hypot(a.x - city.x, a.y - city.y) - Math.hypot(b.x - city.x, b.y - city.y));
        const b = hits[0];
        return { id: b.id, x: b.x, y: b.y, name: b.name, fn: b.function, city: [city.x, city.y] };
      }
    }
    return null;
  })()`);
  if (!target) throw new Error('no building found');
  console.log('target', target);
  const fit = await evaluate('(() => { const g = window.__map.geom; return [g.map_w_ft / 2, g.map_h_ft / 2, window.__map.cam.fitZoom(g.map_w_ft, g.map_h_ft)]; })()');
  const [x0, y0, z0] = fit;
  const z1 = Math.log2(64 / 5) - 0.3; // a 5-ft square a little under 64 px: the battlemap

  // The zoom: about the inn, which drifts to the middle as the map closes in.
  const N = Number(process.env.MEDIA_FRAMES ?? 170);
  const ease = (t) => t * t * (3 - 2 * t);
  let k = 0;
  const frame = async () => shot(join(frames, `f${String(k++).padStart(4, '0')}.png`));
  for (let i = 0; i <= N; i++) {
    const t = i / N;
    const z = z0 + (z1 - z0) * t;
    const s = 2 ** -(z - z0);
    const off = 1 - ease(t);
    await evaluate(`window.__map.cam.set({ cx: ${target.x + (x0 - target.x) * s * off}, cy: ${target.y + (y0 - target.y) * s * off}, zoom: ${z} })`);
    await settle();
    await frame();
    if (i === 0) await shot(join(out, 'continent.png'));
    if (i === 0) for (let h = 0; h < 15; h++) await frame();
    process.stdout.write(`\rframe ${i}/${N}`);
  }
  await shot(join(out, 'battlemap.png'));
  for (let h = 0; h < 18; h++) await frame();
  // Inside: the inn's ground floor, over the map.
  await evaluate(`window.__map.enterBuilding(${JSON.stringify(target.id)})`);
  await sleep(1000);
  await settle();
  await shot(join(out, 'interior.png'));
  for (let h = 0; h < 40; h++) await frame();
  console.log(`\n${k} frames`);

  // Stills: the city, then a dungeon.
  await evaluate('window.__map.exitBuilding()');
  await evaluate(`window.__map.cam.set({ cx: ${target.city[0]}, cy: ${target.city[1]}, zoom: -2.2 })`);
  await sleep(500);
  await settle();
  await shot(join(out, 'city.png'));
  // The biggest underground site of each kind: a dungeon, a cave, a mine, a lava tube (or a crypt).
  const unders = await evaluate(`(async () => {
    const m = window.__map, f = m.overlay.features;
    const towns = f.filter((x) => ['metropolis', 'city', 'town', 'village'].includes(x.kind)).length;
    const sites = f.filter((x) => ['ruin', 'tower', 'waystation', 'camp', 'cave', 'mine', 'lava_tube'].includes(x.kind) && !x.id.startsWith('c:'));
    const best = {};
    for (let k = 0; k < sites.length; k++) {
      const id = 'u:' + (towns + k) + ':0';
      const it = await m.gen.interior(id);
      if (it && (!best[it.function] || it.nx * it.ny > best[it.function].n)) best[it.function] = { id, n: it.nx * it.ny };
    }
    return Object.fromEntries(Object.entries(best).map(([k, v]) => [k, v.id]));
  })()`);
  console.log('underground:', JSON.stringify(unders));
  const wanted = ['dungeon', 'cave', 'mine', 'lava tube', 'crypt', 'catacombs'].filter((k) => unders[k]).slice(0, 4);
  for (const kind of wanted) {
    // (Going in frames the level.)
    await evaluate('window.__map.exitBuilding()');
    if (!(await evaluate(`window.__map.enterBuilding(${JSON.stringify(unders[kind])})`))) continue;
    await still();
    await settle();
    await shot(join(out, `${kind.replace(' ', '-')}.png`));
  }

  // Every zoom frame differs from the last, so the size goes by frames and pixels: 10 fps, 560 px
  // wide, one palette for the whole clip, keep it near 10 MB.
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-framerate', '20', '-i', join(frames, 'f%04d.png'), '-vf', 'fps=10,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle', join(out, 'zoom.gif')], { stdio: 'inherit' });
  console.log(`written to ${out}`);
  ws.close();
} finally {
  chrome.kill();
  await sleep(800);
  try {
    rmSync(profile, { recursive: true, force: true });
  } catch {}
}
