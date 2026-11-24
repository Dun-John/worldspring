// Dev tool: screenshot the sprite gallery (`?gallery=1`, app/src/dev/gallery.ts) into contact sheets of about 20
// sprites each, at true scale (`&scale=1&looks=1`), per family, labelled and with `&nolabels=1`.
// Usage: node scripts/galleryshot.mjs <out-dir> [base-url=http://localhost:5173/]
// Files: <family>-<n>[-nolabels].png (families: battlemap, furniture, underground, and each themed panel, `streets` too).
// Opens Windows Chrome as scripts/shot.mjs does (one window for all sheets, always closed at the end). SHOT_WIN=w,h
// sets its size (default 1620,1000); SHOT_DPR sets the device pixel ratio. `--only=a,b` (before the out dir) takes only those
// families.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { launchChrome } from './lib/chrome.mjs';

// `--compact` (before the out dir): sheets for an image reviewer, none over 1600 x 1800 px: battlemap, furniture and underground only,
// one look per item, split until each fits, only the unlabelled version with a number in each cell's corner (`&index=1`) and a
// `<sheet>.txt` beside each PNG listing `number id-or-name` (from the page's `window.__galleryOrder`).
const compact = process.argv.includes('--compact');
const only = process.argv.find((a) => a.startsWith('--only='))?.slice(7).split(',');
const inkSheet = only?.includes('ink');
const [outDir = 'sheets', base = 'http://localhost:5173/'] = process.argv.slice(2).filter((a) => a !== '--compact' && !a.startsWith('--only='));
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
const q = (s) => `${base}?gallery=1${compact ? '&scale=1&index=1&nolabels=1' : common}${s}`;
const MAX_H = 1800;
const jobs = [
  ...KINDS.map((ids, i) => ({ family: 'battlemap', n: i + 1, query: `&kinds=${ids.join(',')}` })),
  ...chunks(setOf('VARIED'), 20).map((names, i) => ({ family: 'furniture', n: i + 1, query: `&items=${names.join(',')}` })),
  ...chunks(setOf('UNDER_DRAWN'), 20).map((names, i) => ({ family: 'underground', n: i + 1, query: `&items=${names.join(',')}` })),
  ...PANELS.map((p) => ({ family: p, n: 1, query: `&items=${p}` })),
  { family: 'streets', n: 1, query: '&panels=streets' },
  { family: 'plaza', n: 1, query: '&panels=plaza' },
  { family: 'plaza', n: 2, query: '&panels=plaza&set=2' },
].filter((j) => !only || only.includes(j.family));
if (inkSheet) jobs.length = 0;

const { chrome, profile } = launchChrome('pfm-gal-', [
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding',
  '--disable-background-timer-throttling',
  '--window-position=0,0',
  `--window-size=${process.env.SHOT_WIN ?? '1620,1000'}`,
  ...(process.env.SHOT_DPR ? [`--force-device-scale-factor=${process.env.SHOT_DPR}`] : []),
  inkSheet ? base : q(jobs[0].query),
]);
// Compact mode: the lists to split (kinds are completed with the catalog's others once the page is up).
const compactLists = () => [
  ['battlemap', 'kinds', KINDS.flat(), 4],
  ['furniture', 'items', setOf('VARIED'), 8],
  ['underground', 'items', setOf('UNDER_DRAWN'), 8],
];
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
  const sheet = async (url, file, onlyMeasure = false) => {
    await send('Page.navigate', { url });
    let ready = false;
    for (let i = 0; i < 240 && !ready; i++) {
      await sleep(250);
      ready = await evaluate('window.__galleryReady === true');
    }
    if (!ready) throw new Error('gallery not ready: ' + url);
    await sleep(500);
    const size = await evaluate(`(() => { const r = document.querySelector('canvas').getBoundingClientRect(); return [Math.ceil(r.width), Math.ceil(r.height)]; })()`);
    if (onlyMeasure) return size;
    const [w, h] = [size[0], Math.min(size[1], 16000)];
    const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true, clip: { x: 0, y: 0, width: w, height: h, scale: 1 } });
    writeFileSync(file, Buffer.from(shot.data, 'base64'));
    console.log(file, `${w}x${size[1]}`, `${Math.round(Buffer.from(shot.data, 'base64').length / 1024)} KB`);
    return size;
  };

  /** Compact: shoot `list` as one sheet if it fits MAX_H, else split it in halves. Returns the next sheet number. */
  const compactSheet = async (family, param, list, n) => {
    const url = q(`&family=${family}&${param}=${list.join(',')}`);
    const size = await sheet(url, '', true);
    if (size[1] > MAX_H && list.length > 1) {
      const half = Math.ceil(list.length / 2);
      n = await compactSheet(family, param, list.slice(0, half), n);
      return compactSheet(family, param, list.slice(half), n);
    }
    const file = join(outDir, `${family}-${n}.png`);
    await sheet(url, file);
    const order = (await evaluate('window.__galleryOrder')) ?? [];
    writeFileSync(file.replace(/\.png$/, '.txt'), order.map((o, i) => `${i + 1} ${o}`).join('\n') + '\n');
    if (size[1] > MAX_H || size[0] > 1600) console.error('OVER CAP', file, size.join('x'));
    return n + 1;
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
  if (inkSheet) {
    // `--only=ink`: the sprite audit's ink A/B: the same sprites drawn with today's ink and with `&ink=2b2118`, stacked in one PNG.
    const query = '&scale=1&sq=64&items=conifer,crates,cart,well,boulder,bench,rowboat,bed,table,bookcase,chest,coffin,rock_column,ore_cart';
    const halves = [['today 0x1d1a14', ''], ['umber #2b2118', '&ink=2b2118']];
    const imgs = [];
    for (const [, extra] of halves) {
      const f = join(outDir, `ink-half${imgs.length}.png`);
      await sheet(`${base}?gallery=1${query}${extra}`, f);
      imgs.push(readFileSync(f).toString('base64'));
    }
    await send('Page.navigate', { url: 'about:blank' });
    await sleep(500);
    const out = await send('Runtime.evaluate', {
      awaitPromise: true,
      returnByValue: true,
      expression: `(async () => {
        const load = (b) => new Promise((r) => { const i = new Image(); i.onload = () => r(i); i.src = 'data:image/png;base64,' + b; });
        const ims = await Promise.all(${JSON.stringify(imgs)}.map(load));
        const labels = ${JSON.stringify(halves.map((h) => h[0]))};
        const cap = 36, w = Math.max(...ims.map((i) => i.width)), h = ims.reduce((a, i) => a + i.height + cap, 0);
        const c = document.createElement('canvas'); c.width = w; c.height = h;
        const g = c.getContext('2d'); g.fillStyle = '#fff'; g.fillRect(0, 0, w, h);
        let y = 0;
        ims.forEach((im, k) => { g.fillStyle = '#222'; g.fillRect(0, y, w, cap); g.fillStyle = '#fff'; g.font = 'bold 22px sans-serif'; g.fillText('ink: ' + labels[k], 12, y + 26); g.drawImage(im, 0, y + cap); y += im.height + cap; });
        return c.toDataURL('image/png');
      })()`,
    });
    const file = join(outDir, 'ink-ab.png');
    writeFileSync(file, Buffer.from(out.result.value.split(',')[1], 'base64'));
    console.log(file);
  }
  if (compact) {
    const lists = compactLists();
    await sheet(q(`&kinds=${KINDS[0].join(',')}`), '', true);
    const all = (await evaluate('window.__galleryKinds')) ?? [];
    const listed = new Set(KINDS.flat());
    lists[0][2] = [...KINDS.flat(), ...all.filter((id) => !listed.has(id))];
    for (const [family, param, list, per] of lists) {
      let n = 1;
      for (const c of chunks(list, per)) {
        try {
          n = await compactSheet(family, param, c, n);
        } catch (e) {
          failed++;
          console.error('FAILED', family, String(e.message ?? e));
        }
      }
    }
    jobs.length = 0;
  }
  for (const job of jobs) {
    await run(job);
    // Once the catalog is known: the kinds no family above lists, in chunks of 20.
    if (job === jobs[0] && job.family === 'battlemap') {
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
