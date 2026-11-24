// Dev tool: screenshot the sprite gallery (`?gallery=1`, app/src/dev/gallery.ts) into contact sheets of about 20
// sprites each, at true scale (`&scale=1&looks=1`), per family, labelled and with `&nolabels=1`.
// Usage: node scripts/galleryshot.mjs <out-dir> [base-url=http://localhost:5173/]
// Files: <family>-<n>[-nolabels].png (families: battlemap, furniture, underground, and each themed panel, `streets` too).
// Opens Windows Chrome as scripts/shot.mjs does (one window for all sheets, always closed at the end). SHOT_WIN=w,h
// sets its size (default 1620,1000); SHOT_DPR sets the device pixel ratio.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const [outDir = 'sheets', base = 'http://localhost:5173/'] = process.argv.slice(2);
mkdirSync(outDir, { recursive: true });
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const tables = readFileSync(join(root, 'app/src/render/furniture/tables.ts'), 'utf8');
/** The quoted names in `export const <name> = new Set([ ... ])`. */
const setOf = (name) => {
  const m = tables.match(new RegExp(`export const ${name} = new Set\\(\\[([\\s\\S]*?)\\]\\);`));
  if (!m) throw new Error(`${name} not found in tables.ts`);
  return [...m[1].matchAll(/'([a-z0-9_]+)'/g)].map((x) => x[1]);
};
const chunks = (list, n) => Array.from({ length: Math.ceil(list.length / n) }, (_, i) => list.slice(i * n, (i + 1) * n));

// Battlemap kinds by family (BATTLE_GROUPS in editor/itemGroups.ts; the harbour batch); any others the catalog has follow as `other`.
const KINDS = [
  [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 18, 19],
  [12, 13, 14, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32],
  [33, 34, 35, 38, 40, 41, 42, 43, 44, 45, 36, 37, 46, 47, 48, 49, 50, 51],
  [52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 75, 76, 77, 78, 62, 63],
  [39, 100, 101, 102, 103, 104, 105, 106, 107, 150, 151, 152, 155, 140, 141, 142, 143, 144, 145],
];
const PANELS = ['temple', 'terrain', 'dungeon4', 'civic', 'forecourt', 'townwalls', 'cemetery', 'flatroofs', 'harbour', 'falls', 'breakwaters', 'piers', 'lighthouse', 'fences', 'shadows', 'stairs', 'countryside'];
const common = '&scale=1&looks=1';
const q = (s) => `${base}?gallery=1${common}${s}`;
const jobs = [
  ...KINDS.map((ids, i) => ({ family: 'battlemap', n: i + 1, query: `&kinds=${ids.join(',')}` })),
  ...chunks(setOf('VARIED'), 20).map((names, i) => ({ family: 'furniture', n: i + 1, query: `&items=${names.join(',')}` })),
  ...chunks(setOf('UNDER_DRAWN'), 20).map((names, i) => ({ family: 'underground', n: i + 1, query: `&items=${names.join(',')}` })),
  ...PANELS.map((p) => ({ family: p, n: 1, query: `&items=${p}` })),
  { family: 'streets', n: 1, query: '&panels=streets' },
];

const chromePath = [process.env.CHROME_PATH, '/mnt/c/Program Files/Google/Chrome/Application/chrome.exe', 'C:/Program Files/Google/Chrome/Application/chrome.exe', '/usr/bin/google-chrome'].filter(Boolean).find((p) => existsSync(p));
if (!chromePath) throw new Error('no Chrome found (set CHROME_PATH)');
// A Windows Chrome started from WSL needs its profile where Windows can see it (and as a Windows path).
const fromWsl = chromePath.startsWith('/mnt/');
const profile = mkdtempSync(join(fromWsl ? '/mnt/c/Users/Public' : tmpdir(), 'pfm-gal-'));
const profileArg = fromWsl ? execFileSync('wslpath', ['-w', profile]).toString().trim() : profile;
const chrome = spawn(chromePath, [
  `--user-data-dir=${profileArg}`,
  '--remote-debugging-port=0',
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding',
  '--disable-background-timer-throttling',
  '--window-position=0,0',
  `--window-size=${process.env.SHOT_WIN ?? '1620,1000'}`,
  ...(process.env.SHOT_DPR ? [`--force-device-scale-factor=${process.env.SHOT_DPR}`] : []),
  q(jobs[0].query),
]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let ws;
let nextId = 1;
function send(method, params = {}) {
  const id = nextId++;
  return new Promise((resolve) => {
    const onMsg = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id !== id) return;
      ws.removeEventListener('message', onMsg);
      resolve(msg.result);
    };
    ws.addEventListener('message', onMsg);
    ws.send(JSON.stringify({ id, method, params }));
  });
}
const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }))?.result?.value;

let port;
let failed = 0;
try {
  for (let i = 0; i < 60 && !port; i++) {
    try {
      port = Number(readFileSync(join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0]) || undefined;
    } catch {}
    if (!port) await sleep(250);
  }
  for (let i = 0; i < 60 && !ws; i++) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = targets.find((t) => t.type === 'page' && t.url.startsWith('http'));
      if (page) ws = new WebSocket(page.webSocketDebuggerUrl);
    } catch {}
    if (!ws) await sleep(500);
  }
  await new Promise((r) => ws.addEventListener('open', r, { once: true }));
  ws.addEventListener('message', (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.method === 'Runtime.exceptionThrown') console.error('[page exception]', msg.params.exceptionDetails?.exception?.description ?? msg.params.exceptionDetails?.text);
  });
  await send('Runtime.enable');
  await send('Page.enable');

  /** Load a gallery URL, wait for it to finish drawing, write the page's canvas as a PNG. */
  const sheet = async (url, file) => {
    await send('Page.navigate', { url });
    let ready = false;
    for (let i = 0; i < 240 && !ready; i++) {
      await sleep(250);
      ready = await evaluate('window.__galleryReady === true');
    }
    if (!ready) throw new Error('gallery not ready: ' + url);
    await sleep(500);
    const size = await evaluate(`(() => { const r = document.querySelector('canvas').getBoundingClientRect(); return [Math.ceil(r.width), Math.ceil(r.height)]; })()`);
    const [w, h] = [size[0], Math.min(size[1], 16000)];
    const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true, clip: { x: 0, y: 0, width: w, height: h, scale: 1 } });
    writeFileSync(file, Buffer.from(shot.data, 'base64'));
    console.log(file, `${w}x${size[1]}`, `${Math.round(Buffer.from(shot.data, 'base64').length / 1024)} KB`);
  };

  const run = async (job) => {
    for (const nolabels of [false, true]) {
      const file = join(outDir, `${job.family}-${job.n}${nolabels ? '-nolabels' : ''}.png`);
      try {
        await sheet(q(job.query + (nolabels ? '&nolabels=1' : '')), file);
      } catch (e) {
        failed++;
        console.error('FAILED', file, String(e.message ?? e));
      }
    }
  };
  for (const job of jobs) {
    await run(job);
    // Once the catalog is known: the kinds no family above lists, in chunks of 20.
    if (job === jobs[0]) {
      const all = (await evaluate('window.__galleryKinds')) ?? [];
      const listed = new Set(KINDS.flat());
      const rest = all.filter((id) => !listed.has(id));
      chunks(rest, 20).forEach((ids, i) => jobs.splice(KINDS.length + i, 0, { family: 'battlemap-other', n: i + 1, query: `&kinds=${ids.join(',')}` }));
    }
  }
  ws.close();
} finally {
  // Ask Chrome itself to quit first: killing the spawned process doesn't reach a Windows Chrome started from WSL.
  if (port) {
    try {
      const { webSocketDebuggerUrl } = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
      const browser = new WebSocket(webSocketDebuggerUrl);
      await new Promise((resolve, reject) => {
        browser.addEventListener('open', resolve, { once: true });
        browser.addEventListener('error', reject, { once: true });
      });
      browser.send(JSON.stringify({ id: 1, method: 'Browser.close' }));
      await sleep(500);
    } catch {}
  }
  chrome.kill();
  await sleep(800);
  try {
    rmSync(profile, { recursive: true, force: true });
  } catch {}
}
if (failed) process.exitCode = 1;
