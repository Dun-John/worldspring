#!/usr/bin/env node
// mcpcheck: mapd's MCP tools checked against a baseline (QA tooling, not a test suite).
//
//   node scripts/mcpcheck.mjs [--list] [--map] [--old BIN] [--new BIN] [--port N] [--seed N] [--call TOOL JSON]
//
// Starts mapd (--new, default target/release/mapd, or $CARGO_TARGET_DIR's) on --port (default 7799) with a fresh
// worlds folder, opens a seed-only world over /ws, then:
//   --list  diffs tools/list against scripts/mcpcheck/tools-before.json (names, schemas, hints);
//   --map   runs scripts/mcpcheck/map.json: for every old tool one {old, new} pair of calls. Every tool of the
//           baseline must have an entry or still be listed. With --old BIN (a build of the baseline) the old
//           calls run there on an identical fresh world and the results must agree; without it the new calls
//           must only succeed. Then the refusals in the map (old names, arguments a view doesn't take) must be
//           refused with the text given;
//   --call  one call, its reply printed (for writing map entries).
// With neither --list nor --map nor --call, both. Prints failures and a summary only.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const argv = process.argv.slice(2);
const flag = (f) => argv.includes(f);
const opt = (f, d) => (argv.includes(f) ? argv[argv.indexOf(f) + 1] : d);
const target = process.env.CARGO_TARGET_DIR || path.join(root, 'target');
const newBin = opt('--new', path.join(target, 'release', 'mapd'));
const oldBin = opt('--old', null);
const port = Number(opt('--port', '7799'));
const seed = Number(opt('--seed', '1'));
const doCall = flag('--call');
const doList = flag('--list') || (!flag('--map') && !doCall);
const doMap = flag('--map') || (!flag('--list') && !doCall);
const here = path.join(root, 'scripts', 'mcpcheck');

function genVersion() {
  const src = fs.readFileSync(path.join(root, 'crates', 'worldgen-model', 'src', 'world', 'mod.rs'), 'utf8');
  return Number(/pub const GEN_VERSION: u32 = (\d+);/.exec(src)[1]);
}

const world = {
  gen_version: genVersion(),
  seed,
  params: {
    width_mi: 1200.0, height_mi: 900.0, sea_level_ft: 0.0, max_elev_ft: 14000.0, land_fraction: 0.45, ruggedness: 1.0, plate_count: 14,
    erosion: 1.0, lat_top: 62.0, lat_bottom: 8.0, wind: 'belts', temp_offset_c: 0.0, moisture: 1.0, volcanoes: 2, river_density: 1.0,
    settlement_density: 1.0, poi_density: 1.0, biome_weights: {},
  },
  edits: {},
};

async function start(bin, p) {
  if (!fs.existsSync(bin)) throw new Error(`no mapd at ${bin} (build it, or give --new / --old)`);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mcpcheck-'));
  const proc = spawn(bin, ['--port', String(p), '--dir', dir], { stdio: ['ignore', 'ignore', 'pipe'] });
  let log = '';
  proc.stderr.on('data', (d) => (log = (log + d).slice(-2000)));
  const url = `http://127.0.0.1:${p}`;
  let id = 0;
  const rpc = async (method, params) => {
    const r = await fetch(`${url}/mcp`, { method: 'POST', headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream' }, body: JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params }) });
    const j = await r.json();
    if (j.error) throw new Error(`${method}: ${j.error.message}`);
    return j.result;
  };
  for (let i = 0; ; i++) {
    try {
      await rpc('ping', {});
      break;
    } catch (e) {
      if (i > 100 || proc.exitCode !== null) throw new Error(`mapd ${bin} did not start: ${log}`);
      await new Promise((r) => setTimeout(r, 100));
    }
  }
  // The world, opened as a tab would (then the tab goes: render_view and focus_view find no app).
  await new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${p}/ws`);
    const t = setTimeout(() => reject(new Error('no world from /ws in 120 s')), 120000);
    ws.onopen = () => ws.send(JSON.stringify({ type: 'hello', world, take: true }));
    ws.onmessage = (m) => {
      const v = JSON.parse(m.data);
      if (v.type === 'world' || v.type === 'error') {
        clearTimeout(t);
        ws.close();
        v.type === 'world' ? resolve() : reject(new Error(v.message));
      }
    };
    ws.onerror = () => reject(new Error('/ws failed'));
  });
  const call = async (name, args) => {
    const r = await rpc('tools/call', { name, arguments: args ?? {} });
    const parts = r.content ?? [];
    const first = parts.find((c) => c.type === 'text')?.text ?? '';
    if (r.isError) return { error: first };
    let v;
    try {
      v = JSON.parse(first);
    } catch {
      v = first;
    }
    return { ok: v, parts: parts.map((c) => c.type) };
  };
  const stop = () => {
    proc.kill();
    fs.rmSync(dir, { recursive: true, force: true });
  };
  return { call, list: async () => (await rpc('tools/list', {})).tools, stop };
}

const fails = [];
const fail = (s) => fails.push(s);
const cut = (v) => {
  const s = typeof v === 'string' ? v : JSON.stringify(v);
  return s.length > 160 ? s.slice(0, 157) + '...' : s;
};

// Results compared with the change labels' and timings' keys left out (they may differ by design).
const SKIP = new Set(['tool', 'ms', 'elapsed_ms', 'took_ms']);
function norm(v) {
  if (Array.isArray(v)) return v.map(norm);
  if (v && typeof v === 'object') return Object.fromEntries(Object.keys(v).filter((k) => !SKIP.has(k)).sort().map((k) => [k, norm(v[k])]));
  return v;
}
const same = (a, b) => JSON.stringify(norm(a)) === JSON.stringify(norm(b));
// Where two results first differ: a path and both values.
function where(a, b, at = '') {
  if (JSON.stringify(norm(a)) === JSON.stringify(norm(b))) return null;
  if (a && b && typeof a === 'object' && typeof b === 'object') {
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
      if (SKIP.has(k)) continue;
      const w = where(a[k], b[k], `${at}.${k}`);
      if (w) return w;
    }
  }
  return `${at || '.'}: old ${cut(a)} new ${cut(b)}`;
}
// `$name` in a call's arguments: the id an earlier entry with `as: name` made (each build its own).
function fill(v, vars) {
  if (typeof v === 'string') return v.replace(/\$(\w+)/g, (m, k) => vars[k] ?? m);
  if (Array.isArray(v)) return v.map((x) => fill(x, vars));
  if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, fill(x, vars)]));
  return v;
}
// A new result with the new build's ids of the entries' makings swapped for the old build's.
function swap(v, pairs) {
  let s = JSON.stringify(v);
  for (const [n, o] of pairs) s = s.split(JSON.stringify(n).slice(1, -1)).join(JSON.stringify(o).slice(1, -1));
  return JSON.parse(s);
}
const drop = (v, keys) => (v && typeof v === 'object' && !Array.isArray(v) ? Object.fromEntries(Object.entries(v).filter(([k]) => !keys.includes(k))) : v);

const servers = [];
try {
  const now = await start(newBin, port);
  servers.push(now);
  if (doCall) {
    const i = argv.indexOf('--call');
    console.log(JSON.stringify(await now.call(argv[i + 1], JSON.parse(argv[i + 2] ?? '{}')), null, 1).slice(0, 6000));
  }
  const tools = await now.list();
  const names = new Set(tools.map((t) => t.name));
  const before = JSON.parse(fs.readFileSync(path.join(here, 'tools-before.json'), 'utf8'));
  const old = before.result?.tools ?? before.tools ?? before;
  let summary = [`tools now ${tools.length} (baseline ${old.length})`];
  if (doList) {
    const gone = old.filter((t) => !names.has(t.name)).map((t) => t.name);
    const oldNames = new Set(old.map((t) => t.name));
    const added = tools.filter((t) => !oldNames.has(t.name)).map((t) => t.name);
    const changed = [];
    for (const t of tools) {
      const o = old.find((x) => x.name === t.name);
      if (o && !(same(o.inputSchema, t.inputSchema) && same(o.annotations, t.annotations))) changed.push(t.name);
    }
    summary.push(`list: ${gone.length} gone, ${added.length} new (${added.join(', ')}), ${changed.length} changed${changed.length ? ` (${changed.join(', ')})` : ''}`);
  }
  if (doMap) {
    const map = JSON.parse(fs.readFileSync(path.join(here, 'map.json'), 'utf8'));
    const covered = new Set(map.entries.map((e) => e.old.tool));
    const missing = old.map((t) => t.name).filter((n) => !covered.has(n) && !names.has(n));
    for (const n of missing) fail(`map: no entry for ${n}, and it is no longer listed`);
    const then = oldBin ? await start(oldBin, port + 1) : null;
    if (then) servers.push(then);
    let agree = 0;
    const bothRefused = [];
    const vars = { old: {}, new: {} };
    const pairs = [];
    for (const e of map.entries) {
      const b = await now.call(e.new.tool, fill(e.new.arguments, vars.new));
      if (e.as && b.ok?.id) vars.new[e.as] = b.ok.id;
      if (!then) {
        if (b.error && !e.refused) fail(`${e.old.tool} -> ${e.new.tool}: ${cut(b.error)}`);
        else agree++;
        continue;
      }
      const a = await then.call(e.old.tool, fill(e.old.arguments, vars.old));
      if (e.as && a.ok?.id) {
        vars.old[e.as] = a.ok.id;
        if (b.ok?.id) pairs.push([b.ok.id, a.ok.id]);
      }
      if (b.ok) b.ok = drop(swap(b.ok, pairs), e.ignore ?? []);
      if (a.ok) a.ok = drop(a.ok, e.ignore ?? []);
      if (a.error && b.error) {
        agree++;
        bothRefused.push(e.old.tool);
      }
      else if (a.error || b.error) fail(`${e.old.tool} -> ${e.new.tool}: ${a.error ? 'only the old refused' : 'only the new refused'}: ${cut(a.error ?? b.error)}`);
      else if (!same(a.ok, b.ok) || String(a.parts) !== String(b.parts)) fail(`${e.old.tool} -> ${e.new.tool}: results differ at ${where(a.ok, b.ok)}`);
      else agree++;
    }
    let refused = 0;
    for (const r of map.refuse ?? []) {
      const b = await now.call(r.tool, r.arguments);
      if (b.error && b.error.includes(r.says)) refused++;
      else fail(`refuse ${r.tool} ${cut(r.arguments)}: wanted "${r.says}", got ${cut(b.error ?? b.ok)}`);
    }
    const stillListed = old.filter((t) => !covered.has(t.name) && names.has(t.name)).length;
    summary.push(`map: ${agree}/${map.entries.length} ${then ? 'agree with the old build' : 'run'}, ${stillListed} old tools still listed, ${missing.length} missing; refusals ${refused}/${(map.refuse ?? []).length}${bothRefused.length ? `; refused by both: ${bothRefused.join(', ')}` : ''}`);
  }
  for (const f of fails.slice(0, 40)) console.log('FAIL', f);
  if (fails.length > 40) console.log(`... and ${fails.length - 40} more`);
  console.log(summary.join('\n'));
  console.log(fails.length ? `${fails.length} failures` : 'ok');
} finally {
  for (const s of servers) s.stop();
}
process.exit(fails.length ? 1 : 0);
