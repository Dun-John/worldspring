import { svelte } from '@sveltejs/vite-plugin-svelte';
import { realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

/** Whether `dir` is on a network share (a UNC path, or a drive mapped to one, which resolves to
 * its UNC path): file watching fails there (`UNKNOWN: watch`), so the dev server polls. */
function onNetworkShare(dir: string): boolean {
  try {
    return /^(\\\\|\/\/)/.test(realpathSync.native(dir));
  } catch {
    return false;
  }
}
// (`WS_POLL=1` polls anywhere, e.g. a share mounted on Linux or macOS.)
const poll = process.env.WS_POLL === '1' || onNetworkShare(fileURLToPath(new URL('.', import.meta.url)));

export default defineConfig({
  // Where the site is served from: `/` locally, `/<repo>/` on GitHub Pages (scripts/publish.mjs).
  base: process.env.WS_BASE ?? '/',
  plugins: [svelte()],
  worker: { format: 'es' },
  // Two pages: the map (the DM's at a table) and the players' window.
  build: { target: 'es2022', rollupOptions: { input: { main: fileURLToPath(new URL('./index.html', import.meta.url)), player: fileURLToPath(new URL('./player.html', import.meta.url)) } } },
  server: { port: 5173, ...(poll ? { watch: { usePolling: true, interval: 300 } } : {}) },
});
