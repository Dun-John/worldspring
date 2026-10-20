// Dev tool: screenshot the running app at given camera views (unthrottled Chrome window).
// Usage: node scripts/shot.mjs <out-prefix> [url] [views-json]
//   views-json: [{"name":"continent"}, {"name":"region","fx":0.5,"fy":0.5,"zoom":-7}, ...]
//   fx/fy are fractions of the map; zoom is log2(px per ft); omitted = fit whole map.
//   Optional "eval": a page expression whose (JSON-able) value is printed after the shot;
//   "do": an expression run after positioning the camera (then "wait" ms, default 1500).
// Env: SHOT_WIN=w,h sets the window size, SHOT_DPR=2 renders at that device pixel ratio (sharper art),
// SHOT_PHONE=w,h[,dpr] emulates a phone (that viewport, touch input, mobile layout; e.g. 390,844,3).
// Each run gets its own Chrome on a free DevTools port (SHOT_PORT=n to pin one), so runs can go side by side.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [prefix = 'shot', url = 'http://localhost:5173/', viewsJson = '[{"name":"continent"}]'] = process.argv.slice(2);
const views = JSON.parse(viewsJson);
const chromePath = [process.env.CHROME_PATH, 'C:/Program Files/Google/Chrome/Application/chrome.exe', '/usr/bin/google-chrome']
  .filter(Boolean)
  .find((p) => existsSync(p));
const profile = mkdtempSync(join(tmpdir(), 'pfm-shot-'));
const chrome = spawn(chromePath, [
  `--user-data-dir=${profile}`,
  `--remote-debugging-port=${process.env.SHOT_PORT ?? 0}`,
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding',
  '--disable-background-timer-throttling',
  '--window-position=0,0',
  `--window-size=${process.env.SHOT_WIN ?? '1600,1000'}`,
  ...(process.env.SHOT_DPR ? [`--force-device-scale-factor=${process.env.SHOT_DPR}`] : []),
  url,
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
const evaluate = async (expression) =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }))?.result?.value;

try {
  // Chrome writes the port it took to DevToolsActivePort in its profile.
  let port;
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
  // Echo page errors and console errors/warnings (shader compile failures, worker errors).
  ws.addEventListener('message', (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.method === 'Runtime.exceptionThrown') console.error('[page exception]', msg.params.exceptionDetails?.exception?.description ?? msg.params.exceptionDetails?.text);
    if (msg.method === 'Runtime.consoleAPICalled' && ['error', 'warning'].includes(msg.params.type)) {
      console.error(`[console.${msg.params.type}]`, msg.params.args.map((a) => a.value ?? a.description ?? '').join(' ').slice(0, 2000));
    }
  });
  await send('Runtime.enable');
  // A phone: its viewport and touch input, then the page loaded again so it starts in that layout.
  if (process.env.SHOT_PHONE) {
    const [width, height, dpr = 3] = process.env.SHOT_PHONE.split(',').map(Number);
    await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: dpr, mobile: true });
    await send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
    await send('Page.reload', { ignoreCache: false });
    await sleep(1500);
  }
  // Wait for the world to generate.
  for (let i = 0; i < 120; i++) {
    if (await evaluate('!!(window.__map && window.__map.tiles)')) break;
    await sleep(500);
  }
  for (const v of views) {
    // A view with only a "do" action keeps the camera where the previous step left it.
    if (v.fx !== undefined || v.zoom !== undefined || !v.do)
      await evaluate(`(() => {
      const m = window.__map, g = m.geom;
      const zoom = ${v.zoom ?? 'null'} ?? m.cam.fitZoom(g.map_w_ft, g.map_h_ft);
      m.cam.set({ cx: g.map_w_ft * ${v.fx ?? 0.5}, cy: g.map_h_ft * ${v.fy ?? 0.5}, zoom });
    })()`);
    if (v.do) {
      await evaluate(v.do);
      await sleep(v.wait ?? 1500);
    }
    for (let i = 0; i < 60; i++) {
      await sleep(250);
      if (await evaluate('window.__map.readiness >= 1 && window.__map.tiles.stats.queuedUploads === 0')) break;
    }
    await sleep(400);
    const shot = await send('Page.captureScreenshot', { format: 'png' });
    const file = `${prefix}-${v.name}.png`;
    writeFileSync(file, Buffer.from(shot.data, 'base64'));
    const info = await evaluate('JSON.stringify({status: window.__map.status, level: window.__map.tiles.stats.level})');
    console.log(file, info);
    if (v.eval) console.log(await evaluate(v.eval));
  }
  ws.close();
} finally {
  chrome.kill();
  await sleep(800);
  try {
    rmSync(profile, { recursive: true, force: true });
  } catch {}
}
