// Build worldgen-wasm: cargo (wasm profile) → wasm-bindgen (web + nodejs) → wasm-opt.
// Web output goes to app/src/gen/pkg; the nodejs build (for det-wasm.mjs) to target/wasm-node.
// --quick: the wasm-quick profile and no wasm-opt, for checking work in progress (never published:
// a plain build after it rebuilds, the stamp names the profile).
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const win = process.platform === 'win32';
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { cwd: root, stdio: 'inherit', shell: win, ...opts });
const sha256 = (path) => createHash('sha256').update(readFileSync(path)).digest('hex');

const quick = process.argv.includes('--quick');
const profile = quick ? 'wasm-quick' : 'wasm';
run('cargo', ['build', '-p', 'worldgen-wasm', '--profile', profile, '--target', 'wasm32-unknown-unknown']);
// Features rustc's wasm32 target emits (plus simd128 from .cargo/config.toml). Not --all-features:
// that lets wasm-opt use newer encodings some engines reject.
const WASM_FEATURES = ['--enable-simd', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--enable-reference-types', '--enable-multivalue'];
// Semantics-preserving only (no --fast-math): determinism must survive optimization.
const OPT_ARGS = ['-O3', ...WASM_FEATURES];
const wasm = join(root, `target/wasm32-unknown-unknown/${profile}/worldgen_wasm.wasm`);
const outputs = [
  ['app/src/gen/pkg', 'web'],
  ['target/wasm-node', 'nodejs'],
].map(([dir, target]) => ({ dir: join(root, dir), target, bg: join(root, dir, 'worldgen_wasm_bg.wasm') }));

// Nothing to do when cargo's wasm, wasm-bindgen and the outputs are as the last build left them
// (the stamp also keeps the dev server from reloading on a build that changed nothing).
const stampPath = join(root, 'target/wasm-pkg.stamp');
const stampOf = () => JSON.stringify({
  profile,
  wasm: sha256(wasm),
  bindgen: execFileSync('wasm-bindgen', ['--version'], { encoding: 'utf8', shell: win }).trim(),
  opt: OPT_ARGS,
  out: outputs.map((o) => (existsSync(o.bg) ? sha256(o.bg) : null)),
});
const before = stampOf();
if (existsSync(stampPath) && readFileSync(stampPath, 'utf8') === before) {
  console.log('wasm up to date');
  process.exit(0);
}

// wasm-opt from the binaryen release matching the npm package (binaryen@132, whose output it matches
// byte for byte): native and multi-threaded, about twice as fast as the npm build (one core, under
// node). Fetched once into target/tools; the npm one is the fallback (no network, other platforms).
const BINARYEN = 'version_132';
const RELEASES = {
  'win32-x64': ['x86_64-windows', '2089428ec98c899b45ee5d00636ddd6e2da8636cc473ef50b165cc25793ef7cb'],
  'linux-x64': ['x86_64-linux', '195ddc94f9bc89f45abdabb0b9eea86023d727ba90eac8b35b80f2544fc30572'],
  'linux-arm64': ['aarch64-linux', 'c58562417836c5d0493d89bdefc434933bdc097db641b483df86bcfa557a107f'],
  'darwin-x64': ['x86_64-macos', '40c3de90bb3766bd0282a895e139a6f50253dba49b4f5bb89e66faca162d832e'],
  'darwin-arm64': ['arm64-macos', '98aad827847af7ef990ed7098d885725c8e5b5aae75073403635617ae4e259aa'],
};
async function nativeWasmOpt() {
  const release = RELEASES[`${process.platform}-${process.arch}`];
  if (!release) return null;
  const tools = join(root, 'target/tools');
  const exe = join(tools, `binaryen-${BINARYEN}/bin/wasm-opt${win ? '.exe' : ''}`);
  if (existsSync(exe)) return exe;
  const name = `binaryen-${BINARYEN}-${release[0]}.tar.gz`;
  try {
    console.log(`fetching ${name} (once, ~110 MB)`);
    const res = await fetch(`https://github.com/WebAssembly/binaryen/releases/download/${BINARYEN}/${name}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = Buffer.from(await res.arrayBuffer());
    if (createHash('sha256').update(data).digest('hex') !== release[1]) throw new Error('checksum mismatch');
    mkdirSync(tools, { recursive: true });
    writeFileSync(join(tools, name), data);
    // Only wasm-opt (the archive holds ~600 MB of other tools and static libs); on macOS it links
    // to lib/libbinaryen.dylib. Windows' own tar (bsdtar): Git's GNU tar takes `C:` for a host.
    const members = [`binaryen-${BINARYEN}/bin/wasm-opt${win ? '.exe' : ''}`];
    if (process.platform === 'darwin') members.push(`binaryen-${BINARYEN}/lib/libbinaryen.dylib`);
    const tar = win ? join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe') : 'tar';
    execFileSync(tar, ['-xzf', name, ...members], { cwd: tools, stdio: 'inherit' });
    rmSync(join(tools, name));
    return existsSync(exe) ? exe : null;
  } catch (e) {
    console.warn(`native wasm-opt unavailable (${e.message}); using the npm one`);
    return null;
  }
}
const npmWasmOpt = join(root, 'app/node_modules/.bin', win ? 'wasm-opt.cmd' : 'wasm-opt');
const native = await nativeWasmOpt();
function wasmOpt(file) {
  if (native) {
    try {
      return run(native, [...OPT_ARGS, file, '-o', file], { shell: false });
    } catch {
      console.warn('native wasm-opt failed; using the npm one');
    }
  }
  run(npmWasmOpt, [...OPT_ARGS, file, '-o', file]);
}
const canOpt = !quick && (native !== null || existsSync(npmWasmOpt));

for (const o of outputs) run('wasm-bindgen', [wasm, '--out-dir', o.dir, '--target', o.target]);
// wasm-bindgen's web and nodejs modules are the same bytes (only the JS glue differs): optimize once.
const [web, node] = outputs;
const same = readFileSync(web.bg).equals(readFileSync(node.bg));
if (canOpt) {
  wasmOpt(web.bg);
  if (same) copyFileSync(web.bg, node.bg);
  else wasmOpt(node.bg);
}
// The nodejs glue is CommonJS; the repo root is "type": "module".
writeFileSync(join(root, 'target/wasm-node/package.json'), '{"type": "commonjs"}\n');
if (canOpt || quick) writeFileSync(stampPath, stampOf());
else rmSync(stampPath, { force: true });
console.log(quick ? 'wasm built quick (unoptimized; run npm run wasm before a commit)' : canOpt ? 'wasm built and optimized' : 'wasm built (wasm-opt not installed; run npm install in app/)');
