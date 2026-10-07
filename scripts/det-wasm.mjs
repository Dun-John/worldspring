// Vital check: the WASM build must produce byte-identical output to the native build.
// Usage: node scripts/det-wasm.mjs   (run `npm run wasm` first)
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(import.meta.url);
const wasm = require(join(root, 'target/wasm-node/worldgen_wasm.js'));

// A generated world, and a sketched one (coastline, range, river, painted biome, pins).
const MI = 5280;
const mi = (x, y) => [Math.round(x * MI), Math.round(y * MI)];
const outline = Array.from({ length: 40 }, (_, k) => {
  const a = (2 * Math.PI * k) / 40;
  return mi(600 + 420 * Math.cos(a) * (1 + 0.1 * Math.sin(3 * a)), 450 + 290 * Math.sin(a));
});
const sketched = JSON.parse(wasm.default_world_json(77));
sketched.sketch = {
  strokes: [
    { tool: 'land', closed: true, pts: outline },
    { tool: 'range', radius_ft: 15 * MI, strength: 0.8, pts: [mi(350, 300), mi(500, 320), mi(650, 280)] },
    { tool: 'river', radius_ft: 3 * MI, strength: 0.7, pts: [mi(500, 360), mi(520, 500), mi(560, 760)] },
    { tool: 'biome', biome: 'jungle', closed: true, pts: [mi(800, 400), mi(900, 420), mi(880, 560), mi(780, 520)] },
    { tool: 'pin', tier: 'city', name: 'Sketchford', pts: [mi(540, 600)] },
    { tool: 'pin', tier: 'village', pts: [mi(300, 450)] },
  ],
};

// A world with buildings drawn by hand (an L-shaped inn, a round tower) on a sampled tile.
const built = JSON.parse(wasm.default_world_json(424242));
{
  const [x, y] = [Math.round(0.37 * built.params.width_mi * MI), Math.round(0.41 * built.params.height_mi * MI)].map((v) => Math.round(v / 5) * 5);
  const ell = [[0, 0], [50, 0], [50, 20], [20, 20], [20, 45], [0, 45]].map(([a, b]) => [x + a, y + b]);
  const tower = Array.from({ length: 16 }, (_, k) => [x + 90 + 15 * Math.cos((k / 16) * 2 * Math.PI), y + 20 + 15 * Math.sin((k / 16) * 2 * Math.PI)]);
  built.edits = {
    created: [
      { id: 'c:0', kind: 'building', x: x + 20, y: y + 18, name: '', poly: ell, func: 'inn', floors: 2, roof: 'battlements', tint: 'slate' },
      { id: 'c:1', kind: 'building', x: x + 90, y: y + 20, name: 'The Needle', poly: tower, func: 'wizard_tower', floors: 4, roof: 'cone' },
    ],
    // Crossings put down by hand, across the sampled tile.
    crossings: {
      'v:bridge': { kind: 'bridge', a: [x - 120, y + 70], b: [x + 160.5, y + 130], width: 12 },
      'v:ford': { kind: 'ford', a: [x - 100, y + 160], b: [x + 140, y + 200.25], width: 9 },
      'v:ferry': { kind: 'ferry', a: [x - 200, y - 60], b: [x + 210, y - 20], width: 14 },
    },
  };
}

let total = 0;
for (const worldJson of [wasm.default_world_json(424242), JSON.stringify(sketched), JSON.stringify(built)]) {
  const native = execFileSync('cargo', ['run', '-q', '--release', '-p', 'worldgen', '--example', 'dethash', '-'], {
    cwd: root,
    input: worldJson,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  const web = wasm.det_report(worldJson);
  const a = native.trim().split(/\r?\n/);
  const b = web.trim().split(/\r?\n/);
  const diffs = a.filter((line, i) => line !== b[i]);
  if (a.length !== b.length || diffs.length) {
    console.error(`DETERMINISM FAILURE: ${diffs.length} of ${a.length} artifacts differ`);
    diffs.slice(0, 10).forEach((d) => console.error('  native:', d));
    process.exit(1);
  }
  total += a.length;
}
console.log(`determinism ok: ${total} artifacts identical (native == wasm), with a sketched world, drawn buildings, crossings and a site redesigned`);
