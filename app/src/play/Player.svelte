<script lang="ts">
  // The players' window: the map as the DM shares it (no DM-only things, no notes), with the
  // tokens, fog, rulers and pings of play mode. Drag and zoom freely unless the DM holds the
  // view; right-click (or Alt-click) to ping; F for full screen.
  import { onMount } from 'svelte';
  import type { WorldFile } from '../gen/protocol';
  import { MapView } from '../render/MapView';
  import { linked } from '../world/library';
  import { GEN_VERSION, readLink, sameWorld } from '../world/world';
  import { PlayController } from './controller';

  const view = new MapView('player');
  const play = new PlayController(view, 'player');
  if (import.meta.env.DEV) Object.assign(window, { __map: view, __play: play });
  let container: HTMLDivElement;
  let status = $state('Waiting for the DM’s window…');
  let hint = $state(true);
  /** The full-screen button shows while the pointer moves (touch screens have no F key). */
  let chrome = $state(true);
  let chromeTimer: ReturnType<typeof setTimeout> | undefined;
  function wake() {
    chrome = true;
    clearTimeout(chromeTimer);
    chromeTimer = setTimeout(() => (chrome = false), 3000);
  }
  const touch = typeof matchMedia !== 'undefined' && matchMedia('(pointer: coarse)').matches;
  /** The world shown (or being drawn). */
  let shown: WorldFile | null = null;
  let queue: Promise<void> = Promise.resolve();

  /** Show a world: drawn anew if it is another one, else only its edits. */
  async function show(w: WorldFile) {
    if (shown && sameWorld(w, shown) && JSON.stringify(w.sketch ?? null) === JSON.stringify(shown.sketch ?? null)) {
      shown = w;
      view.setEdits(w.edits ?? {});
      return;
    }
    shown = w;
    status = 'Drawing the world…';
    await view.loadWorld(w);
    view.setPlaces(play.dmPlaces);
    status = '';
  }

  play.onWorld = (w) => (queue = queue.then(() => show(w)));
  play.onEdits = (w) => {
    shown = w;
    view.setEdits(w.edits ?? {});
  };

  function fullscreen() {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void document.documentElement.requestFullscreen?.();
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'f' || e.key === 'F') fullscreen();
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('pointermove', wake);
    window.addEventListener('pointerdown', wake);
    wake();
    const t = setTimeout(() => (hint = false), 8000);
    (async () => {
      await view.mount(container);
      view.setPlaces(false);
      play.connect();
      // No DM window answering: show the world from the link.
      setTimeout(async () => {
        const w = (await readLink(location, linked)).world;
        // (A link from another generator is shown on this one.)
        if (!shown && w) void play.onWorld({ ...w, gen_version: GEN_VERSION });
      }, 1500);
    })();
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('pointermove', wake);
      window.removeEventListener('pointerdown', wake);
      clearTimeout(t);
      clearTimeout(chromeTimer);
      view.gen.dispose();
    };
  });
</script>

<div class="map" bind:this={container}></div>
{#if status}
  <div class="status">{status}</div>
{/if}
{#if hint}
  <div class="hint">{touch ? 'Tap ⛶ for full screen' : 'Right-click to ping · F: full screen'}</div>
{/if}
{#if chrome}
  <button class="full" onclick={fullscreen} aria-label="Full screen" title="Full screen (F)">⛶</button>
{/if}

<style>
  .map {
    position: fixed;
    inset: 0;
  }
  .status,
  .hint {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(243, 236, 216, 0.94);
    border: 1px solid #3a322a;
    border-radius: 4px;
    padding: 6px 12px;
    color: #2b241d;
    font: 15px/1.4 Georgia, 'Times New Roman', serif;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
  }
  .status {
    top: 40%;
  }
  .full {
    position: fixed;
    top: max(12px, env(safe-area-inset-top));
    right: 12px;
    width: 44px;
    height: 44px;
    font-size: 22px;
    line-height: 1;
    color: #2b241d;
    background: rgba(243, 236, 216, 0.94);
    border: 1px solid #3a322a;
    border-radius: 6px;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
    cursor: pointer;
  }
  .hint {
    bottom: 16px;
    font-size: 13px;
    opacity: 0.85;
  }
</style>
