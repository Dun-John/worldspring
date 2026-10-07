<script lang="ts">
  // The ☰ menu: the world, its library and files, the players' window while playing, the
  // keyboard shortcuts, the Discord and the performance stats. Everything else lives in the sections.
  import type { WorldFile } from '../../gen/protocol';
  import { download } from '../../world/world';
  import Icon from '../Icon.svelte';
  import { shell } from './layout.svelte';

  interface Props {
    world: WorldFile;
    playing: boolean;
    onNewWorld: () => void;
    onLibrary: () => void;
    onPlayers: () => void;
    onClose: () => void;
  }
  let { world, playing, onNewWorld, onLibrary, onPlayers, onClose }: Props = $props();

  const size = $derived(`${(world.params.width_mi ?? 1200).toLocaleString()} × ${(world.params.height_mi ?? 900).toLocaleString()} mi`);
  const touchOnly = typeof matchMedia !== 'undefined' && matchMedia('(hover: none)').matches;

  function act(fn: () => void) {
    onClose();
    fn();
  }
</script>

<div class="scrim" role="presentation" onclick={onClose}></div>
<div class="menu ws-panel" role="menu" aria-label="Menu">
  <div class="head">
    <div class="brand">Worldspring</div>
    <div class="ws-muted">Seed {world.seed} · {size}</div>
  </div>
  <button role="menuitem" onclick={() => act(onNewWorld)}><Icon name="globe" /> New world<span class="sub">seed and settings</span></button>
  <button role="menuitem" onclick={() => act(onLibrary)}><Icon name="folder" /> Library<span class="sub">saved worlds, open a file</span></button>
  <button role="menuitem" onclick={() => act(() => download(world, `world-${world.seed}`))}><Icon name="download" /> Download this world</button>
  {#if playing}<button role="menuitem" onclick={() => act(onPlayers)}><Icon name="monitor" /> Players' window</button>{/if}
  <hr />
  <a role="menuitem" href="https://discord.gg/8ZS4nHWWVv" target="_blank" rel="noopener" onclick={onClose}><Icon name="chat" /> Discord community<span class="sub">help, ideas, bugs</span></a>
  {#if !touchOnly}<button role="menuitem" onclick={() => act(() => (shell.help = true))}><Icon name="keyboard" /> Keyboard shortcuts<kbd class="ws-kbd">?</kbd></button>{/if}
  <label class="row">
    <Icon name="activity" /> Performance stats
    <input type="checkbox" class="ws-switch" checked={shell.stats} onchange={(e) => shell.setStats(e.currentTarget.checked)} />
  </label>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: var(--z-menu);
  }
  .menu {
    position: absolute;
    z-index: var(--z-menu);
    top: calc(var(--tap) + 12px);
    left: 0;
    width: 280px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    box-shadow: var(--shadow-lg);
    background: var(--paper-solid);
  }
  .head {
    padding: 4px 8px 8px;
    border-bottom: 1px solid var(--line-faint);
    margin-bottom: 4px;
  }
  .brand {
    font-weight: bold;
    font-size: 16px;
    letter-spacing: 0.04em;
  }
  button,
  a,
  .row {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: calc(var(--tap) + 4px);
    padding: 3px 8px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    color: var(--ink);
  }
  button:hover,
  a:hover,
  .row:hover,
  button:focus-visible,
  a:focus-visible {
    background: var(--btn-hover);
  }
  .sub {
    margin-left: auto;
    font-size: 11px;
    color: var(--ink-3);
  }
  kbd,
  .row input {
    margin-left: auto;
  }
  hr {
    border: none;
    border-top: 1px solid var(--line-faint);
    margin: 4px 0;
  }
</style>
