// Vital check (perf): run the ?bench=1 fly-through in a real Chrome window on this machine and
// print the result. Uses a throwaway profile with background throttling disabled, so numbers are
// valid even if the window is covered. Requires the dev server (npm run dev).
// Usage: node scripts/bench.mjs [url]
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const url = process.argv[2] ?? 'http://localhost:5173/?seed=1&bench=1';
const port = 9333;
const candidates = [
  process.env.CHROME_PATH,
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  '/usr/bin/google-chrome',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
].filter(Boolean);
const chromePath = candidates.find((p) => existsSync(p));
if (!chromePath) throw new Error('Chrome not found; set CHROME_PATH');

const profile = mkdtempSync(join(tmpdir(), 'pfm-bench-'));
const chrome = spawn(chromePath, [
  `--user-data-dir=${profile}`,
  `--remote-debugging-port=${port}`,
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding',
  '--disable-background-timer-throttling',
  '--window-position=0,0',
  '--window-size=1920,1080',
  '--start-maximized',
  url,
]);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function pageSocket() {
  for (let i = 0; i < 60; i++) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = targets.find((t) => t.type === 'page' && t.url.startsWith('http'));
      if (page) return page.webSocketDebuggerUrl;
    } catch {}
    await sleep(500);
  }
  throw new Error('could not reach Chrome DevTools');
}

async function evaluate(ws, expression) {
  const id = Math.floor(Math.random() * 1e9);
  return new Promise((resolve) => {
    const onMsg = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id !== id) return;
      ws.removeEventListener('message', onMsg);
      resolve(msg.result?.result?.value);
    };
    ws.addEventListener('message', onMsg);
    ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true } }));
  });
}

try {
  const ws = new WebSocket(await pageSocket());
  await new Promise((r) => ws.addEventListener('open', r, { once: true }));
  const deadline = Date.now() + (url.includes('bench=heavy') ? 600_000 : 240_000);
  let result = null;
  while (!result && Date.now() < deadline) {
    await sleep(2000);
    result = await evaluate(ws, 'window.__benchResult ? JSON.stringify(window.__benchResult) : null');
  }
  ws.close();
  if (!result) throw new Error('benchmark did not finish in time');
  const r = JSON.parse(result);
  console.log(JSON.stringify(r, null, 2));
  // The stress bench: the same steps on the world as generated and loaded with edits.
  if (r.pan) {
    if (r.error) throw new Error(r.error);
    const pass = r.pan.heavy.low1Fps >= 30;
    console.log(`\nholds ${r.holds.editsKB} KB of edits (designs ${r.holds.designsKB} KB; ${r.holds.buildings} buildings edited, ${r.holds.buildingsKB} KB; ${r.holds.buildingDesigns} building designs; ${r.holds.castles} castles, ${r.holds.walls} walls); loaded in ${r.loadMs.apply} ms, settled in ${r.loadMs.settle} ms`);
    for (const [k, t] of Object.entries(r.edits.heavy)) {
      const e = r.edits.empty[k];
      console.log(`${k.padEnd(7)} main thread ${e.syncMs[0]} → ${t.syncMs[0]} ms (worst ${t.syncMs[1]}), drawn ${e.frameMs[0]} → ${t.frameMs[0]} ms, worker ${e.workerMs[0]} → ${t.workerMs[0]} ms`);
    }
    console.log(`pan 1% low ${r.pan.empty.low1Fps} → ${r.pan.heavy.low1Fps} fps, worst frame ${r.pan.empty.maxMs} → ${r.pan.heavy.maxMs} ms, ms per job ${r.pan.empty.jobMs} → ${r.pan.heavy.jobMs}`);
    console.log(`heap ${r.heapMB.empty} → ${r.heapMB.heavy} MB; a site generated ${r.siteMs.generated} ms, designed ${r.siteMs.designed} ms`);
    console.log(`\nheavy pan 1% low (gate ≥ 30): ${pass ? 'PASS' : 'FAIL'}`);
    process.exitCode = pass ? 0 : 1;
  } else {
    const pass = r.low1Fps >= 30;
    console.log(`\n1% low ${r.low1Fps.toFixed(1)} fps (gate ≥ 30): ${pass ? 'PASS' : 'FAIL'}`);
    console.log(`detail after stops: ${r.detailLatencyMs.join(' / ')} ms (target ≤ 500)`);
  }
} finally {
  chrome.kill();
  await sleep(1000);
  try {
    rmSync(profile, { recursive: true, force: true });
  } catch {}
}
